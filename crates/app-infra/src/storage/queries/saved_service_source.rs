//! Shared actor-local saved-source/pool metadata. No test, plan, Vault or transport authority.
use super::*;
use app_core::{
    domain::model_config::{credentials::*, service_draft::materialize_base, service_graph::*},
    ports::service_configuration::SavedServiceSourceProof,
};
use std::num::NonZeroU32;
pub(super) fn parts(source: &SavedServiceRef) -> (&str, &str, &str) {
    match source {
        SavedServiceRef::Http { source } => ("http", &source.revision_id, &source.model_id),
        SavedServiceRef::Offline { source } => ("offline", &source.revision_id, &source.config_id),
    }
}
pub(super) fn frame(
    conn: &Connection,
    generation: NonZeroU32,
    server: Option<&str>,
    fail: fn(&str) -> AppError,
) -> AppResult<(String, NonZeroU32, String)> {
    let (id,g,s):(String,u32,String)=conn.query_row("SELECT profile_id,runtime_generation,server_instance_id FROM profile_meta WHERE singleton=1 AND runtime_state='active'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
    if g != generation.get() || server.is_some_and(|v| v != s) {
        return Err(fail("profile_generation_conflict"));
    }
    Ok((id, generation, s))
}
pub(super) fn saved(
    conn: &Connection,
    source: &SavedServiceRef,
    fail: fn(&str) -> AppError,
) -> AppResult<(ConfigurationDocument, SavedServiceSourceProof)> {
    let (kind, revision, resource) = parts(source);
    require_id(revision)?;
    require_id(resource)?;
    let row:Option<(u32,String,String,String)>=conn.query_row("SELECT c.codec_version,c.content_hash,s.content_hash,c.revision_role FROM config_revisions c JOIN config_revision_seals s USING(revision_id) WHERE c.revision_id=?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=c.revision_id)",[revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
    let Some((codec, content, seal, role)) = row else {
        return Err(fail("saved_service_source_missing"));
    };
    if role != "user" {
        return Err(fail("activation_source_not_user"));
    }
    let document = configuration::load_document(conn, revision)?;
    let graph = materialize_base(&document)?;
    let control = match source {
        SavedServiceRef::Http { source } => {
            let m = graph
                .models
                .iter()
                .find(|m| m.model_id == source.model_id)
                .ok_or_else(|| fail("missing_model"))?;
            Some(service_selection::connection_control(
                conn,
                revision,
                &m.connection_id,
            )?)
        }
        SavedServiceRef::Offline { source } => {
            if !graph
                .offline_engine_configs
                .iter()
                .any(|o| o.id == source.config_id)
            {
                return Err(fail("missing_offline_configuration"));
            }
            None
        }
    };
    Ok((
        document,
        SavedServiceSourceProof {
            source: source.clone(),
            logical_resource_id: format!("{kind}:{resource}"),
            codec_version: codec,
            content_sha256: content,
            seal_sha256: seal,
            canonical_control_id: control,
        },
    ))
}
pub(super) fn check_control(
    conn: &Connection,
    proof: &SavedServiceSourceProof,
    fail: fn(&str) -> AppError,
) -> AppResult<()> {
    if let Some(control) = &proof.canonical_control_id {
        let stopped: Option<bool> = conn
            .query_row(
                "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                [control],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if stopped == Some(true) {
            return Err(fail("connection_suspended"));
        }
    }
    Ok(())
}
pub(super) fn pool_witness(
    conn: &Connection,
    pool: &str,
    revision: u32,
    fail: fn(&str) -> AppError,
) -> AppResult<String> {
    let p = credentials::pool(conn, pool, revision)?;
    let seal: String = conn
        .query_row(
            "SELECT content_hash FROM credential_pool_seals WHERE pool_id=?1 AND pool_revision=?2",
            params![pool, revision],
            |r| r.get(0),
        )
        .map_err(err)?;
    if p.content_hash != seal {
        return Err(fail("credential_pool_seal_mismatch"));
    }
    Ok(seal)
}

pub(super) struct SavedPoolMember {
    pub selection: app_core::domain::credentials::CredentialSelection,
    pub storage: CredentialStorageMetadata,
    pub lifecycle_revision: u32,
    pub pool_policy_sha256: String,
}
/// Read the real current-policy member without advancing the writer's cursor.
pub(super) fn pool_member(
    conn:&Connection, pool:&str, revision:u32, cursor:u64,
    kind:CredentialMaterialKind, fail:fn(&str)->AppError,
) -> AppResult<SavedPoolMember> {
    let (selection, _)=credentials::select_at_for_kind(conn,pool,revision,cursor,kind)?;
    let (actual,schema,lifecycle):(String,Option<u32>,u32)=conn.query_row("SELECT auth_kind,envelope_schema,lifecycle_revision FROM credential_refs WHERE credential_id=?1 AND state='active'",[&selection.credential_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
    let storage=CredentialStorageMetadata::from_stored(&actual,schema)?;
    let pool_policy_sha256=pool_witness(conn,pool,revision,fail)?;
    Ok(SavedPoolMember {selection,storage,lifecycle_revision:lifecycle,pool_policy_sha256})
}
