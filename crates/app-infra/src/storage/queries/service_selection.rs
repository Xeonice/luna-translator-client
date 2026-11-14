//! Saved versions, proved immutable closure copies and atomic scoped activation.
use super::*;
use crate::storage::SqliteStore;
use app_core::{
    domain::model_config::{service_draft::materialize_base, service_graph::*},
    ports::{
        ServiceConfigurationRepository, ServiceDirectory,
        service_configuration::CompiledServiceDraft,
    },
};
use std::{collections::BTreeSet, num::NonZeroU32, sync::Arc};
mod activation;
mod catalog;
mod draft;
use activation::{inherit, select, set_enabled};
use catalog::{binding, effective, eligibility, logical, projection, query_saved, source_for};
pub(super) use draft::ensure_controls;
use draft::{ensure_presets, save_compiled};
fn failure(code: &str) -> AppError {
    AppError::new(
        code,
        "service_configuration",
        "服务版本、授权或并发状态已改变，请重新核对",
    )
}
fn purpose(p: CatalogPurpose) -> &'static str {
    match p {
        CatalogPurpose::Translation => "translation",
        CatalogPurpose::Grammar => "grammar",
        CatalogPurpose::ContextWord => "context_word",
    }
}
fn generation(conn: &Connection) -> AppResult<NonZeroU32> {
    let n:u32=conn.query_row("SELECT runtime_generation FROM profile_meta WHERE singleton=1 AND runtime_state='active'",[],|r|r.get(0)).map_err(err)?;
    NonZeroU32::new(n).ok_or_else(|| failure("invalid_profile_generation"))
}
fn check_generation(conn: &Connection, expected: NonZeroU32) -> AppResult<()> {
    if generation(conn)? != expected {
        return Err(failure("profile_generation_conflict"));
    }
    Ok(())
}
fn head(conn: &Connection, kind: &str) -> AppResult<Option<(String, u32)>> {
    conn.query_row(
        "SELECT revision_id,head_revision FROM config_heads WHERE head_kind=?1",
        [kind],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(err)
}
fn replay<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    op: &str,
    hash: &str,
    kind: &str,
) -> AppResult<Option<T>> {
    require_id(op)?;
    let row:Option<(String,String,String)>=conn.query_row("SELECT request_sha256,command_kind,result_refs_json FROM operation_receipts WHERE operation_id=?1",[op],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
    match row {
        None => Ok(None),
        Some((h, k, _)) if h != hash || k != kind => Err(failure("operation_conflict")),
        Some((_, _, v)) => Ok(Some(parse(v)?)),
    }
}
fn record<T: serde::Serialize>(
    tx: &Transaction,
    op: &str,
    hash: &str,
    kind: &str,
    value: &T,
    now: i64,
) -> AppResult<()> {
    let rev = bump(tx, now)?;
    tx.execute(
        "INSERT INTO operation_receipts VALUES(?1,?2,?3,?4,?5,?6)",
        params![op, hash, kind, rev, json(value)?, now],
    )
    .map_err(err)?;
    Ok(())
}
fn directory(db: &Database) -> AppResult<Arc<dyn ServiceDirectory>> {
    db.service_directory
        .clone()
        .ok_or_else(|| failure("service_directory_unavailable"))
}
pub(super) fn connection_control(
    conn: &Connection,
    revision: &str,
    connection: &str,
) -> AppResult<String> {
    Ok(
        service_graph::connection_materialization(conn, revision, connection)?
            .map(|source| source.control_connection_id)
            .unwrap_or_else(|| connection.to_owned()),
    )
}
fn selections(conn: &Connection, revision: &str) -> AppResult<BTreeMap<String, u32>> {
    let mut q = conn
        .prepare("SELECT binding_id,selection_revision FROM purpose_bindings WHERE revision_id=?1")
        .map_err(err)?;
    q.query_map([revision], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)
}
fn aliases(conn: &Connection, revision: &str) -> AppResult<Vec<service_graph::Materialization>> {
    let mut q=conn.prepare("SELECT effective_model_id,effective_offline_config_id,source_revision_id,source_model_id,source_offline_config_id,reason FROM activation_resource_materializations WHERE effective_revision_id=?1 AND target_kind IN('http','offline') ORDER BY materialization_id").map_err(err)?;
    q.query_map([revision], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, String>(5)?,
        ))
    })
    .map_err(err)?
    .map(|r| {
        let (m, o, rev, sm, so, reason) = r.map_err(err)?;
        let source = if let Some(model_id) = sm {
            SavedServiceRef::Http {
                source: HttpConfigRef {
                    revision_id: rev,
                    model_id,
                },
            }
        } else {
            SavedServiceRef::Offline {
                source: OfflineConfigRef {
                    revision_id: rev,
                    config_id: so.ok_or_else(|| failure("invalid_materialization_source"))?,
                },
            }
        };
        Ok(service_graph::Materialization {
            effective_model: m,
            effective_offline: o,
            source,
            reason,
        })
    })
    .collect()
}
fn resource_ref(revision: &str, target: &EngineTarget) -> SavedServiceRef {
    match target {
        EngineTarget::Http { model_id } => SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: revision.into(),
                model_id: model_id.clone(),
            },
        },
        EngineTarget::Offline { offline_config_id } => SavedServiceRef::Offline {
            source: OfflineConfigRef {
                revision_id: revision.into(),
                config_id: offline_config_id.clone(),
            },
        },
    }
}
fn source_revision(source: &SavedServiceRef) -> &str {
    match source {
        SavedServiceRef::Http { source } => &source.revision_id,
        SavedServiceRef::Offline { source } => &source.revision_id,
    }
}
impl ServiceConfigurationRepository for SqliteStore {
    fn google_client_draft(
        &self,
        authorization: &app_core::domain::model_config::service_graph::GoogleAuthorizationRef,
        expected_profile_generation: std::num::NonZeroU32,
    ) -> AppResult<app_core::domain::model_config::service_draft::PreparedGoogleClientDraft> {
        let authorization = authorization.clone();
        self.call(move |db| super::google_authorization::prepared_google_client_draft(
            &db.conn, &authorization, expected_profile_generation, &db.server_id,
        ))
    }
    fn service_configuration(&self) -> AppResult<ServiceConfigurationSnapshot> {
        self.call(|db| {
            let s = configuration::state(&db.conn)?;
            Ok(ServiceConfigurationSnapshot {
                profile_generation: generation(&db.conn)?,
                draft_head_revision: s.draft_head_revision,
                applied_head_revision: s.applied_head_revision,
                draft: s.draft,
                applied: s.applied,
                runtime_purposes: s.runtime_purposes,
                runtime_connections: s.runtime_connections,
            })
        })
    }
    fn service_configuration_revision(&self, revision: &str) -> AppResult<ConfigurationDocument> {
        let revision = revision.to_owned();
        self.call(move |db| configuration::load_document(&db.conn, &revision))
    }
    fn service_draft_replay(
        &self,
        operation_id: &str,
        request_hash: &str,
        expected: NonZeroU32,
    ) -> AppResult<Option<ServiceDraftReceipt>> {
        let (op, hash) = (operation_id.to_owned(), request_hash.to_owned());
        self.call(move |db| {
            check_generation(&db.conn, expected)?;
            replay(&db.conn, &op, &hash, "service_draft")
        })
    }
    fn save_service_configuration(
        &self,
        command: &CompiledServiceDraft,
        now: i64,
    ) -> AppResult<ServiceDraftReceipt> {
        let command = CompiledServiceDraft {
            operation_id: command.operation_id.clone(),
            request_hash: command.request_hash.clone(),
            expected_profile_generation: command.expected_profile_generation,
            expected_draft_head_revision: command.expected_draft_head_revision,
            graph: command.graph.clone(),
            resource: command.resource.clone(),
        };
        self.call(move |db| save_compiled(db, &command, now))
    }
    fn saved_services(&self, query: &SavedServiceQuery) -> AppResult<SavedServiceProjection> {
        let query = query.clone();
        self.call(move |db| query_saved(db, &query))
    }
    fn select_saved_services(
        &self,
        input: &SelectSavedServicesInput,
        now: i64,
    ) -> AppResult<ServiceSelectionReceipt> {
        let input = input.clone();
        self.call(move |db| {
            
            select(db, &input, now)
        })
    }
    fn set_service_purpose_enabled(
        &self,
        input: &SetServicePurposeEnabledInput,
        now: i64,
    ) -> AppResult<ServiceSelectionReceipt> {
        let input = input.clone();
        self.call(move |db| set_enabled(db, &input, now))
    }
    fn restore_service_inheritance(
        &self,
        input: &RestoreServiceInheritanceInput,
        now: i64,
    ) -> AppResult<ServiceSelectionReceipt> {
        let input = input.clone();
        self.call(move |db| inherit(db, &input, now))
    }
}
