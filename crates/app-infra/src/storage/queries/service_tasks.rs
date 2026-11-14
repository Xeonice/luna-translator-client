//! Genuine codec3 task admission. No offline target is represented by an HTTP model.
use super::*;
use app_core::{
    application::service_request_plan::{
        self, ComposedServicePlan, ServicePlanInput, TargetLanguageOrigin,
    },
    domain::{
        model_config::{service_draft::materialize_base, service_graph::*},
        service_dispatch::{PreparedWorkTranslation, ServiceTaskDispatch, WorkDispatch},
        service_execution::ServiceExecutionTarget,
    },
};
use serde::{Deserialize, Serialize};
mod admission;
mod authority;
pub(in crate::storage) use admission::{TaskPreparation, prepare};
pub(in crate::storage) use authority::validate_dispatch;
pub(in crate::storage) use authority::validate_google_prelease;
pub(super) use authority::{snapshot, validate};
pub(super) const CACHE_TTL_MS: i64 = 86_400_000;
pub(super) const CACHE_POLICY: &str = "typed-cache-v1";
/// Current runtime has no installed cross-service router. This is a fail-closed
/// gap guard, not a declaration that configured fallback has been implemented.
pub(super) fn validate_primary_route(fallback: &[SavedServiceRef]) -> AppResult<()> {
    if fallback.is_empty() {
        Ok(())
    } else {
        Err(failure("service_fallback_not_ready"))
    }
}
fn failure(code: &str) -> AppError {
    AppError::new(code, "service_task", "服务任务、固定对象或当前授权已改变")
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SelectionAuthority {
    pub requested_scope: String,
    pub requested_revision: u32,
    pub selection_scope: String,
    pub selection_revision: u32,
    pub effective_scope: String,
    pub effective_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CredentialFact {
    credential_id: String,
    pool_id: String,
    pool_revision: u32,
    member_id: String,
    lifecycle_revision: u32,
    auth_kind: String,
    envelope_schema: Option<u32>,
    version_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AdmissionSnapshot {
    pub schema: String,
    pub service_plan: ComposedServicePlan,
    pub plan_sha256: String,
    pub target_sha256: String,
    pub resource_semantics_sha256: String,
    pub cache_input: serde_json::Value,
    pub cache_policy_version: String,
    pub operation_id: Option<String>,
    pub selection: Option<LexicalToken>,
    pub source_hash: String,
    pub selection_authority: SelectionAuthority,
    credential_authority: Option<CredentialFact>,
    pub cache_read_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_status_lineage:
        Option<app_core::domain::translation_status::TranslationStatusLineage>,
}
fn selection_revision(
    conn: &Connection,
    revision: &str,
    scope: &str,
    purpose: &str,
) -> AppResult<Option<u32>> {
    conn.query_row("SELECT selection_revision FROM purpose_bindings WHERE revision_id=?1 AND scope_key=?2 AND purpose=?3",params![revision,scope,purpose],|r|r.get(0)).optional().map_err(err)
}
fn selected_authority(
    conn: &Connection,
    current: &ConfigurationV3,
    unit: &ReadingUnit,
    binding: &PurposeBindingV3,
) -> AppResult<SelectionAuthority> {
    let p = purpose_name(binding.purpose);
    let requested = unit
        .game_id
        .as_ref()
        .map(|g| format!("game:{g}"))
        .unwrap_or_else(|| "global".into());
    let requested_revision =
        selection_revision(conn, &current.revision_id, &requested, p)?.unwrap_or(0);
    let effective_revision = selection_revision(conn, &current.revision_id, &binding.scope_key, p)?
        .ok_or_else(|| failure("missing_effective_selection"))?;
    if effective_revision == 0 {
        return Err(failure("unversioned_service_selection"));
    }
    Ok(SelectionAuthority {
        selection_scope: binding.scope_key.clone(),
        selection_revision: effective_revision,
        requested_scope: requested,
        requested_revision,
        effective_scope: binding.scope_key.clone(),
        effective_revision,
    })
}
fn purpose_name(p: CatalogPurpose) -> &'static str {
    match p {
        CatalogPurpose::Translation => "translation",
        CatalogPurpose::Grammar => "grammar",
        CatalogPurpose::ContextWord => "context_word",
    }
}
fn current_graph(conn: &Connection) -> AppResult<Option<ConfigurationV3>> {
    let revision: Option<String> = conn
        .query_row(
            "SELECT revision_id FROM config_heads WHERE head_kind='applied'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    revision
        .map(|r| configuration::load_document(conn, &r))
        .transpose()?
        .map(|d| materialize_base(&d))
        .transpose()
}
fn typed_mode(db: &Database, uid: &str, revision: u32, p: &str) -> AppResult<bool> {
    let unit = unit(&db.conn, uid, revision)?;
    let Some(graph) = current_graph(&db.conn)? else {
        return Ok(false);
    };
    let p = app_core::domain::model_config::catalog::parse_purpose(p)?;
    Ok(
        effective_service_binding(&graph, unit.game_id.as_deref(), p)
            .is_some_and(|b| b.semantics_version == PurposeSemanticsVersion::TypedV3),
    )
}
pub(in crate::storage) fn prepared_translation(
    db: &mut Database,
    uid: &str,
    rev: u32,
    op: &str,
    now: i64,
) -> AppResult<PreparedWorkTranslation> {
    let dispatches = prepare(db, TaskPreparation { unit_id: uid, text_revision: rev, purpose: "translation", operation_id: Some(op), selected: None, bypass_cache: true, now })?;
    let sql = format!(
        "SELECT {TASK_VIEW_COLUMNS} {TASK_VIEW_JOINS} WHERE json_extract(q.input_snapshot_json,'$.operationId')=?1 ORDER BY q.task_id"
    );
    let mut q = db.conn.prepare(&sql).map_err(err)?;
    let task_views = q
        .query_map([op], task_view_row)
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(PreparedWorkTranslation {
        task_ids: task_views.iter().map(|t| t.task_id.clone()).collect(),
        dispatches,
        task_views,
    })
}
pub(super) fn retry(
    db: &mut Database,
    task: &str,
    attempt: &str,
    code: &str,
    now: i64,
) -> AppResult<String> {
    let handler = super::google_authorization::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    validate(&tx, &handler, task, Some(attempt))?;
    let snapshot = snapshot(&tx, task)?;
    let number:u32=tx.query_row("SELECT attempt_number FROM request_attempts WHERE attempt_id=?1 AND task_id=?2 AND state='started' AND dispatch_kind IN('initial','retry') AND NOT EXISTS(SELECT 1 FROM request_attempts newer WHERE newer.task_id=?2 AND newer.attempt_number>request_attempts.attempt_number)",params![attempt,task],|r|r.get(0)).map_err(|_|failure("retry_not_authorized"))?;
    if number > u32::from(snapshot.service_plan.policy.execution.retries) {
        return Err(failure("retry_limit"));
    }
    let next = number
        .checked_add(1)
        .ok_or_else(|| failure("retry_limit"))?;
    let next_id = id("attempt", &format!("{task}:{next}"));
    tx.execute("UPDATE request_attempts SET state='failed',error_code=?2,finished_at_ms=?3 WHERE attempt_id=?1 AND state='started'",params![attempt,code,now]).map_err(err)?;
    tx.execute("INSERT INTO request_attempts(attempt_id,task_id,attempt_number,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,dispatch_kind,state,error_code,finish_reason,usage_json,started_at_ms,finished_at_ms) SELECT ?1,task_id,?2,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,'retry','started',NULL,NULL,'{}',?3,NULL FROM request_attempts WHERE attempt_id=?4",params![next_id,next,now,attempt]).map_err(err)?;
    let inserted=tx.execute("INSERT INTO attempt_execution_targets(attempt_id,task_id,config_revision_id,binding_id,engine_id,target_kind,http_model_id,http_connection_id,offline_engine_config_id,snapshot_id,route_id,observation_id,observation_epoch,runtime_build_sha256,worker_instance_id,worker_protocol_revision,provenance_sha256) SELECT ?1,task_id,config_revision_id,binding_id,engine_id,target_kind,http_model_id,http_connection_id,offline_engine_config_id,snapshot_id,route_id,observation_id,observation_epoch,runtime_build_sha256,worker_instance_id,worker_protocol_revision,provenance_sha256 FROM attempt_execution_targets WHERE attempt_id=?2",params![next_id,attempt]).map_err(err)?;
    if inserted != 1 {
        return Err(failure("service_target_mismatch"));
    }
    bump(&tx, now)?;
    tx.commit().map_err(err)?;
    Ok(next_id)
}
