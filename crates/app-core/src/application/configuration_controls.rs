//! Non-secret impact computation shared by previews and transaction-time CAS validation.
use crate::{contracts::*, domain::canonical_hash};
pub fn credential_impact(
    configuration: &ConfigurationState,
    credentials: &[CredentialMetadata],
    pools: &[CredentialPool],
    credential_id: &str,
) -> AppResult<CredentialRevocationImpact> {
    let credential = credentials
        .iter()
        .find(|c| c.credential_id == credential_id)
        .ok_or_else(|| AppError::new("credential_not_found", "credentials", "凭据不存在"))?;
    let connections: Vec<(&str, Option<&str>, Option<u32>)> = configuration
        .applied
        .as_ref()
        .map(|document| match document {
            ConfigurationDocument::Legacy { configuration } => configuration
                .connections.iter()
                .map(|c| (c.connection_id.as_str(), c.credential_pool_id.as_deref(), c.credential_pool_revision))
                .collect(),
            ConfigurationDocument::ServiceGraph { configuration } => configuration
                .connections.iter()
                .map(|c| (c.connection_id.as_str(), c.credential_pool_id.as_deref(), c.credential_pool_revision.map(|revision| revision.get())))
                .collect(),
        })
        .unwrap_or_default();
    let affected_connection_ids: Vec<String> = connections
        .into_iter()
        .filter(|(_, pool_id, pool_revision)| {
            pools.iter().any(|pool| {
                Some(pool.pool_id.as_str()) == *pool_id
                    && Some(pool.revision) == *pool_revision
                    && pool
                        .members
                        .iter()
                        .any(|m| m.credential_id == credential_id)
            })
        })
        .map(|(connection_id, _, _)| connection_id.to_owned())
        .collect();
    let mut affected_purposes = Vec::<PurposeImpact>::new();
    for connection in configuration
        .runtime_connections
        .iter()
        .filter(|c| affected_connection_ids.contains(&c.connection_id))
    {
        for impact in &connection.affected_purposes {
            if let Some(existing) = affected_purposes
                .iter_mut()
                .find(|p| p.binding_id == impact.binding_id)
            {
                for engine in &impact.engine_ids {
                    if !existing.engine_ids.contains(engine) {
                        existing.engine_ids.push(engine.clone());
                    }
                }
            } else {
                affected_purposes.push(impact.clone());
            }
        }
    }
    let mut impact = CredentialRevocationImpact {
        config_revision_id: configuration
            .applied
            .as_ref()
            .map(|c| c.revision_id().to_owned()),
        impact_hash: String::new(),
        credential_id: credential_id.into(),
        lifecycle_revision: credential.lifecycle_revision,
        state: credential.state.clone(),
        affected_connection_ids,
        affected_purposes,
    };
    impact.impact_hash = canonical_hash(&impact)?;
    Ok(impact)
}
