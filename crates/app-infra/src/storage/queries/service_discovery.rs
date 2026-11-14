//! Genuine saved-source discovery reads on the original writer. No business rows are written.
use super::*;
use crate::storage::SqliteStore;
use app_core::{
    application::{
        service_discovery::{model_discovery_result, validate_discovered_models},
        service_request_plan::validate_http_connection,
    },
    domain::{
        credentials::CredentialLeaseMetadata,
        model_config::{
            credentials::*, service_editor::validate_http_source_purpose, service_graph::*,
        },
    },
    ports::{ProductRepository, service_discovery::*},
};
use std::sync::Arc;

struct SqliteDiscoveryProof {
    // Storage-private reuse of the existing actual writer token, not Google authorization.
    frame: app_core::ports::google_authorization::GoogleRuntimeFrame,
}
struct SqliteServiceModelDiscoveryRepository {
    store: SqliteStore,
    live: Arc<dyn ServiceModelDiscoveryAuthority>,
}
fn fail(code: &str) -> AppError {
    AppError::new(
        code,
        "service_model_discovery",
        "所保存服务的来源、连接或授权不可核验；原输入已保留",
    )
}
fn actor_frame(
    db: &Database,
) -> AppResult<app_core::ports::google_authorization::GoogleRuntimeFrame> {
    google_authorization::bound_runtime_frame(db)
        .map_err(|_| fail("discovery_source_frame_unavailable"))
}
fn source_connection(
    db: &Database,
    op: &Arc<ServiceModelDiscoveryOperation>,
) -> AppResult<(
    app_core::ports::service_configuration::SavedServiceSourceProof,
    ConnectionV3,
)> {
    let (document, source) =
        saved_service_source::saved(&db.conn, &op.input().request.source, fail)?;
    saved_service_source::check_control(&db.conn, &source, fail)?;
    let SavedServiceRef::Http { source: reference } = &op.input().request.source else {
        return Err(fail("discovery_unsupported"));
    };
    let ConfigurationDocument::ServiceGraph {
        configuration: graph,
    } = document
    else {
        return Err(fail("discovery_typed_source_required"));
    };
    let model = graph
        .models
        .iter()
        .find(|m| m.model_id == reference.model_id)
        .ok_or_else(|| fail("missing_model_configuration"))?;
    validate_http_source_purpose(&graph, model, op.input().request.purpose)?;
    let connection = graph
        .connections
        .iter()
        .find(|c| c.connection_id == model.connection_id)
        .ok_or_else(|| fail("missing_connection"))?;
    if !connection.availability_intent {
        return Err(fail("connection_disabled"));
    }
    if !matches!(
        connection.protocol.as_str(),
        "openai_compatible"
            | "anthropic"
            | "openai_responses"
            | "gemini_generate_content"
            | "ollama_native"
            | "lmstudio_native_v1"
    ) {
        return Err(fail("discovery_unsupported"));
    }
    let directory = db
        .service_directory
        .as_ref()
        .ok_or_else(|| fail("service_directory_unavailable"))?;
    validate_http_connection(&directory.catalog(), connection, op.input().request.purpose)?;
    if connection.adapter_binding.is_none()
        || connection.http_options.is_none()
        || connection.origin.is_none()
    {
        return Err(fail("discovery_typed_source_required"));
    }
    if connection.auth_mode != "none" && connection.auth_mode != "api_key" {
        return Err(fail("discovery_unsupported"));
    }
    Ok((source, connection.clone()))
}
fn authentication(db: &Database, c: &ConnectionV3) -> AppResult<ServiceDiscoveryAuthentication> {
    let Some(kind) =
        app_core::domain::model_config::catalog::credential_kind_for_auth_mode(&c.auth_mode)?
    else {
        return Ok(ServiceDiscoveryAuthentication::None);
    };
    if kind != CredentialMaterialKind::ApiKey {
        return Err(fail("discovery_unsupported"));
    }
    let pool = c
        .credential_pool_id
        .as_ref()
        .ok_or_else(|| fail("missing_credential"))?;
    let revision = c
        .credential_pool_revision
        .ok_or_else(|| fail("missing_credential"))?
        .get();
    let member = saved_service_source::pool_member(
        &db.conn,
        pool,
        revision,
        *db.pool_cursors.get(&(pool.clone(), revision)).unwrap_or(&0),
        kind,
        fail,
    )?;
    let selection = member.selection;
    Ok(ServiceDiscoveryAuthentication::PoolMember {
        metadata: Box::new(CredentialLeaseMetadata {
            credential: CredentialVersionRef {
                credential_id: selection.credential_id,
                credential_version_id: selection.vault_ref.clone(),
            },
            vault_ref: selection.vault_ref,
            storage: member.storage,
            lifecycle_revision: member.lifecycle_revision,
            pool_id: selection.pool_id,
            pool_revision: selection.pool_revision,
            member_id: selection.member_id,
        }),
        pool_policy_sha256: member.pool_policy_sha256,
    })
}
fn validate_sql(db: &mut Database, p: &Arc<PreparedServiceModelDiscovery>) -> AppResult<()> {
    let proof = p
        .repository_witness
        .downcast_ref::<SqliteDiscoveryProof>()
        .ok_or_else(|| fail("discovery_source_witness_invalid"))?;
    let current = actor_frame(db)?;
    if current != proof.frame
        || current.profile_id != p.profile_id
        || current.profile_generation != p.profile_generation
        || current.server_instance_id != p.server_instance_id
        || p.profile_generation != p.operation.input().request.expected_profile_generation
    {
        return Err(fail("discovery_source_frame_mismatch"));
    }
    let (source, connection) = source_connection(db, &p.operation)?;
    if source != p.source
        || connection != p.connection
        || connection.http_options.as_ref() != Some(&p.http_options)
        || connection.origin.as_ref() != Some(&p.origin)
    {
        return Err(fail("discovery_source_changed"));
    }
    match &p.authentication {
        ServiceDiscoveryAuthentication::None if connection.auth_mode == "none" => {}
        ServiceDiscoveryAuthentication::PoolMember {
            metadata,
            pool_policy_sha256,
        } if connection.auth_mode == "api_key" => {
            if connection.credential_pool_id.as_ref() != Some(&metadata.pool_id)
                || connection
                    .credential_pool_revision
                    .map(std::num::NonZeroU32::get)
                    != Some(metadata.pool_revision)
            {
                return Err(fail("discovery_credential_changed"));
            }
            let current = credentials::credential_lease_metadata(
                db,
                &metadata.pool_id,
                metadata.pool_revision,
                &metadata.vault_ref,
                CredentialMaterialKind::ApiKey,
            )?;
            if current != **metadata
                || saved_service_source::pool_witness(
                    &db.conn,
                    &metadata.pool_id,
                    metadata.pool_revision,
                    fail,
                )? != *pool_policy_sha256
            {
                return Err(fail("discovery_credential_changed"));
            }
        }
        _ => return Err(fail("discovery_credential_changed")),
    }
    Ok(())
}
impl SqliteStore {
    pub fn service_model_discovery_repository(
        &self,
        live: Arc<dyn ServiceModelDiscoveryAuthority>,
    ) -> AppResult<Arc<dyn ServiceModelDiscoveryRepository>> {
        let install = live.clone();
        self.call(move |db| {
            if let Some(old) = &db.service_model_discovery_authority {
                if !Arc::ptr_eq(old, &install) {
                    return Err(fail("discovery_authority_conflict"));
                }
            } else {
                db.service_model_discovery_authority = Some(install);
            }
            Ok(())
        })?;
        Ok(Arc::new(SqliteServiceModelDiscoveryRepository {
            store: self.clone(),
            live,
        }))
    }
}
impl ServiceModelDiscoveryRepository for SqliteServiceModelDiscoveryRepository {
    fn prepare(
        &self,
        op: &Arc<ServiceModelDiscoveryOperation>,
    ) -> AppResult<Arc<PreparedServiceModelDiscovery>> {
        let budget = self.live.operation_budget(op)?;
        if !budget.shares_with(&op.budget()) {
            return Err(fail("discovery_budget_mismatch"));
        }
        budget.check()?;
        let op = op.clone();
        let live = self.live.clone();
        self.store.call(move |db| {
            let current = live.operation_budget(&op)?;
            if !current.shares_with(&budget) {
                return Err(fail("discovery_budget_mismatch"));
            }
            current.check()?;
            let frame = actor_frame(db)?;
            if frame.profile_generation != op.input().request.expected_profile_generation {
                return Err(fail("profile_generation_conflict"));
            }
            let (source, connection) = source_connection(db, &op)?;
            let http_options = connection
                .http_options
                .clone()
                .ok_or_else(|| fail("discovery_typed_source_required"))?;
            let origin = connection
                .origin
                .clone()
                .ok_or_else(|| fail("discovery_typed_source_required"))?;
            let authentication = authentication(db, &connection)?;
            let prepared = Arc::new(PreparedServiceModelDiscovery {
                operation: op.clone(),
                profile_id: frame.profile_id.clone(),
                profile_generation: frame.profile_generation,
                server_instance_id: frame.server_instance_id.clone(),
                source,
                connection,
                http_options,
                origin,
                authentication,
                repository_witness: Arc::new(SqliteDiscoveryProof { frame }),
            });
            current.check()?;
            live.attach_prepared(&op, prepared.clone())?;
            Ok(prepared)
        })
    }
    fn validate(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        _: ServiceDiscoveryValidationPhase,
    ) -> AppResult<()> {
        let budget = self.live.prepared_budget(p)?;
        if !budget.shares_with(&p.operation.budget()) {
            return Err(fail("discovery_budget_mismatch"));
        }
        budget.check()?;
        let p = p.clone();
        let live = self.live.clone();
        self.store.call(move |db| {
            let original = live.prepared_budget(&p)?;
            if !original.shares_with(&budget) {
                return Err(fail("discovery_budget_mismatch"));
            }
            original.check()?;
            validate_sql(db, &p)?;
            original.check()
        })
    }
    fn validate_gateway_owner(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        gateway: &Arc<dyn ProductRepository>,
    ) -> AppResult<()> {
        let budget = self.live.prepared_budget(p)?;
        if !budget.shares_with(&p.operation.budget()) {
            return Err(fail("discovery_budget_mismatch"));
        }
        budget.check()?;
        // Never call another repository while holding this actor.
        let actual = gateway
            .google_runtime_frame()
            .map_err(|_| fail("discovery_source_frame_unavailable"))?;
        budget.check()?;
        let p = p.clone();
        let live = self.live.clone();
        self.store.call(move |db| {
            let original = live.prepared_budget(&p)?;
            if !original.shares_with(&budget) {
                return Err(fail("discovery_budget_mismatch"));
            }
            original.check()?;
            validate_sql(db, &p)?;
            let witness = p
                .repository_witness
                .downcast_ref::<SqliteDiscoveryProof>()
                .ok_or_else(|| fail("discovery_source_witness_invalid"))?;
            if actual != witness.frame {
                return Err(fail("discovery_gateway_owner_mismatch"));
            }
            original.check()
        })
    }
    fn complete(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        outcome: ServiceModelDiscoveryOutcome,
    ) -> AppResult<ServiceModelDiscoveryResult> {
        let budget = self.live.prepared_budget(p)?;
        if !budget.shares_with(&p.operation.budget()) {
            return Err(fail("discovery_budget_mismatch"));
        }
        let p = p.clone();
        let live = self.live.clone();
        budget.with_terminal_fence(|| {
            self.store.call(move |db| {
                let original = live.prepared_budget(&p)?;
                if !original.shares_with(&p.operation.budget()) {
                    return Err(fail("discovery_budget_mismatch"));
                }
                original.check()?;
                validate_sql(db, &p)?;
                let ServiceModelDiscoveryOutcome::Available { models } = &outcome else {
                    return Err(fail("discovery_terminal_invalid"));
                };
                validate_discovered_models(models)?;
                original.check()?;
                let result = model_discovery_result(p.operation.input(), outcome);
                // The Core accepts under the original fence and this same actor's
                // final metadata order. No SQL/Vault/network follows this decision.
                live.accept_validated_terminal(&p, result.clone())?;
                Ok(result)
            })
        })
    }
}
