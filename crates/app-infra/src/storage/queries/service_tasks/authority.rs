//! Revalidates frozen dispatch identity against current SQL authority.
use super::*;
pub(in crate::storage) fn snapshot(conn: &Connection, task: &str) -> AppResult<AdmissionSnapshot> {
    let text: String = conn
        .query_row(
            "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
            [task],
            |r| r.get(0),
        )
        .map_err(err)?;
    app_core::domain::model_config::service_codec::validate_unique_service_json(text.as_bytes())?;
    let snapshot: AdmissionSnapshot = parse(text)?;
    if snapshot.schema != "service-admission-v1"
        || snapshot.cache_policy_version != CACHE_POLICY
        || canonical_hash(&snapshot.service_plan)? != snapshot.plan_sha256
    {
        return Err(failure("invalid_service_snapshot"));
    }
    Ok(snapshot)
}
pub(in crate::storage) fn validate(
    conn: &Connection,
    handler: &super::super::google_authorization::GoogleHandlerContext,
    task: &str,
    attempt: Option<&str>,
) -> AppResult<()> {
    validate_inner(conn, handler, task, attempt, AuthorityPhase::Execution).map(|_| ())
}
/// Google material acquisition reuses every frozen task/source/control check.
/// The only deferred fact is the token provider, whose own repository supplies
/// grant/material authority before it can mint or record an execution lease.
pub(in crate::storage) fn validate_google_prelease(
    conn: &Connection,
    handler: &super::super::google_authorization::GoogleHandlerContext,
    task: &str,
    attempt: &str,
) -> AppResult<ComposedServicePlan> {
    validate_inner(conn, handler, task, Some(attempt), AuthorityPhase::GoogleMint)
        .map(|snapshot| snapshot.service_plan)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthorityPhase {
    Execution,
    GoogleMint,
}
fn validate_inner(
    conn: &Connection,
    handler: &super::super::google_authorization::GoogleHandlerContext,
    task: &str,
    attempt: Option<&str>,
    phase: AuthorityPhase,
) -> AppResult<AdmissionSnapshot> {
    let (revision,binding_id,engine_id,p,uid,text_revision,epoch,generation,server,request):(String,String,String,String,String,u32,u32,u32,String,String)=conn.query_row("SELECT config_revision_id,binding_id,engine_id,purpose,unit_id,text_revision,authorization_epoch,profile_generation,server_instance_id,request_semantics_sha256 FROM request_tasks WHERE task_id=?1 AND state='running'",[task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).map_err(|_|failure("task_not_authorized"))?;
    let (current_attempt,credential):(String,Option<String>)=conn.query_row("SELECT attempt_id,credential_id FROM request_attempts WHERE task_id=?1 AND state='started' ORDER BY attempt_number DESC LIMIT 1",[task],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|failure("attempt_not_authorized"))?;
    if attempt.is_some_and(|a| a != current_attempt) {
        return Err(failure("attempt_not_authorized"));
    }
    let snapshot = snapshot(conn, task)?;
    let frozen = &snapshot.service_plan;
    let identity = &frozen.identity;
    if identity.configuration_revision != revision
        || identity.binding_id != binding_id
        || identity.engine_id != engine_id
        || purpose_name(identity.purpose) != p
        || identity.unit_id != uid
        || identity.text_revision != text_revision
    {
        return Err(failure("service_identity_mismatch"));
    }
    let profile:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM profile_meta WHERE singleton=1 AND runtime_state='active' AND runtime_generation=?1 AND server_instance_id=?2)",params![generation,server],|r|r.get(0)).map_err(err)?;
    if !profile {
        return Err(failure("task_not_authorized"));
    }
    let actual_unit = unit(conn, &uid, text_revision)?;
    if actual_unit.session_id != identity.session_id
        || actual_unit.game_id != identity.game_id
        || actual_unit.source_hash != identity.source_hash
    {
        return Err(failure("stale_input_identity"));
    }
    let current = current_graph(conn)?.ok_or_else(|| failure("configuration_not_applied"))?;
    let current_binding =
        effective_service_binding(&current, actual_unit.game_id.as_deref(), identity.purpose)
            .ok_or_else(|| failure("missing_effective_binding"))?;
    if current_binding.binding_id != binding_id
        || !current_binding.enabled
        || selected_authority(conn, &current, &actual_unit, current_binding)?
            != snapshot.selection_authority
    {
        return Err(failure("service_selection_changed"));
    }
    let runtime:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2 AND binding_id=?3 AND config_revision_id=?4 AND authorization_epoch=?5 AND is_enabled=1)",params![current_binding.scope_key,p,binding_id,revision,epoch],|r|r.get(0)).map_err(err)?;
    if !runtime {
        return Err(failure("task_not_authorized"));
    }
    let graph = materialize_base(&configuration::load_document(conn, &revision)?)?;
    if service_request_plan::purpose_service_semantics_hash_for_scope(
        &graph,
        &binding_id,
        &snapshot.selection_authority.requested_scope,
    )? != snapshot.resource_semantics_sha256
        || service_request_plan::purpose_service_semantics_hash_for_scope(
            &current,
            &binding_id,
            &snapshot.selection_authority.requested_scope,
        )? != snapshot.resource_semantics_sha256
    {
        return Err(failure("service_semantics_changed"));
    }
    let plan_hash = canonical_hash(&(
        "service-target-v1",
        &identity.configuration_revision,
        &identity.binding_id,
        &identity.engine_id,
        &frozen.target,
    ))?;
    if snapshot.target_sha256 != plan_hash {
        return Err(failure("service_target_mismatch"));
    }
    let ServiceExecutionTarget::Http { plan: http } = &frozen.target else {
        return Err(failure("offline_runtime_unavailable"));
    };
    let google = http.connection.protocol == "google_translate_v3"
        && matches!(http.connection.auth_mode.as_str(), "google_service_account" | "google_oauth");
    if phase == AuthorityPhase::GoogleMint && !google {
        return Err(failure("google_target_mismatch"));
    }
    if snapshot.selection_authority.effective_scope != current_binding.scope_key {
        return Err(failure("service_selection_changed"));
    }
    let expected_request = canonical_hash(&(
        "service-request-v3",
        CACHE_POLICY,
        &frozen.target,
        &frozen.policy,
        &snapshot.cache_input,
        &snapshot.resource_semantics_sha256,
        &snapshot.credential_authority,
    ))?;
    if expected_request != request {
        return Err(failure("service_request_mismatch"));
    }
    let target:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM task_execution_targets t JOIN attempt_execution_targets a ON a.task_id=t.task_id AND a.config_revision_id=t.config_revision_id AND a.binding_id=t.binding_id AND a.engine_id=t.engine_id AND a.target_kind=t.target_kind AND a.http_model_id=t.http_model_id AND a.http_connection_id=t.http_connection_id WHERE t.task_id=?1 AND a.attempt_id=?2 AND t.target_sha256=?3 AND t.config_revision_id=?4 AND t.binding_id=?5 AND t.engine_id=?6 AND t.target_kind='http' AND t.http_model_id=?7 AND t.http_connection_id=?8 AND t.selection_scope=?9 AND t.selection_revision=?10 AND t.effective_scope=?11 AND t.effective_selection_revision=?12)",params![task,current_attempt,plan_hash,revision,binding_id,engine_id,http.model.model_id,http.connection.connection_id,snapshot.selection_authority.selection_scope,snapshot.selection_authority.selection_revision,snapshot.selection_authority.effective_scope,snapshot.selection_authority.effective_revision],|r|r.get(0)).map_err(err)?;
    if !target {
        return Err(failure("service_target_mismatch"));
    }
    if !crate::providers::adapters::dispatch::installed(&http.connection.protocol)
        || !(crate::providers::gateway::authentication_implemented(&http.connection.auth_mode)
            || phase == AuthorityPhase::GoogleMint && google)
    {
        return Err(failure("service_runtime_unavailable"));
    }
    if google && !super::super::google_authorization::handler_ready(conn, handler)? {
        return Err(failure("service_runtime_unavailable"));
    }
    let gate =
        service_selection::connection_control(conn, &revision, &http.connection.connection_id)?;
    let allowed:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM connection_runtime_states WHERE connection_id=?1 AND is_suspended=0)",[gate],|r|r.get(0)).map_err(err)?;
    if !allowed || !http.connection.availability_intent {
        return Err(failure("connection_suspended"));
    }
    match &snapshot.credential_authority {
        None => {
            let owned_google = google && http.connection.auth_mode == "google_oauth"
                && http.connection.google_authorization.is_some()
                && http.connection.credential_pool_id.is_none()
                && http.connection.credential_pool_revision.is_none()
                && super::super::google_authorization::connection_authorized(conn, &http.connection)?;
            if credential.is_some() || !(http.connection.auth_mode == "none" || owned_google) {
                return Err(failure("credential_identity_mismatch"));
            }
        }
        Some(fact) => {
            if http.connection.credential_pool_id.as_deref() != Some(fact.pool_id.as_str())
                || http.connection.credential_pool_revision.map(|v| v.get())
                    != Some(fact.pool_revision)
                || http.connection.auth_mode != fact.auth_kind
            {
                return Err(failure("credential_identity_mismatch"));
            }
            if credential.as_ref() != Some(&fact.credential_id) {
                return Err(failure("credential_identity_mismatch"));
            }
            let row:Option<(String,u32,String,Option<u32>)>=conn.query_row("SELECT c.vault_ref,c.lifecycle_revision,c.auth_kind,c.envelope_schema FROM credential_refs c JOIN credential_pool_members m ON m.credential_id=c.credential_id WHERE c.credential_id=?1 AND m.pool_id=?2 AND m.pool_revision=?3 AND m.member_id=?4 AND m.enabled=1 AND c.state='active'",params![fact.credential_id,fact.pool_id,fact.pool_revision,fact.member_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
            let Some((vault, lifecycle, kind, envelope)) = row else {
                return Err(failure("credential_unavailable"));
            };
            if content_hash(&vault) != fact.version_sha256
                || lifecycle != fact.lifecycle_revision
                || kind != fact.auth_kind
                || envelope != fact.envelope_schema
            {
                return Err(failure("credential_identity_mismatch"));
            }
        }
    }
    Ok(snapshot)
}
pub(in crate::storage) fn validate_dispatch(
    db: &Database,
    dispatch: &ServiceTaskDispatch,
) -> AppResult<()> {
    let conn = &db.conn;
    let handler = super::super::google_authorization::handler_context(db);
    validate(conn, &handler, &dispatch.task_id, Some(&dispatch.attempt_id))?;
    let stored = snapshot(conn, &dispatch.task_id)?;
    if canonical_hash(&dispatch.plan)? != stored.plan_sha256
        || dispatch.unit.unit_id != stored.service_plan.identity.unit_id
        || dispatch.unit.text_revision != stored.service_plan.identity.text_revision
        || dispatch.unit.source_hash != stored.source_hash
    {
        return Err(failure("service_dispatch_mismatch"));
    }
    let actual_unit = unit(conn, &dispatch.unit.unit_id, dispatch.unit.text_revision)?;
    if dispatch.unit.source_text != actual_unit.source_text
        || dispatch.unit.source_language != actual_unit.source_language
        || dispatch.unit.session_id != actual_unit.session_id
        || dispatch.unit.game_id != actual_unit.game_id
    {
        return Err(failure("service_dispatch_mismatch"));
    }
    let allowed:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM request_tasks WHERE task_id=?1 AND request_semantics_sha256=?2 AND authorization_epoch=?3 AND profile_generation=?4 AND server_instance_id=?5)",params![dispatch.task_id,dispatch.request_semantics_hash,dispatch.authorization_epoch,dispatch.profile_generation,dispatch.server_instance_id],|r|r.get(0)).map_err(err)?;
    if !allowed {
        return Err(failure("service_dispatch_mismatch"));
    }
    let expected:Option<String>=conn.query_row("SELECT c.vault_ref FROM request_attempts a LEFT JOIN credential_refs c ON c.credential_id=a.credential_id WHERE a.attempt_id=?1",[&dispatch.attempt_id],|r|r.get(0)).map_err(err)?;
    if expected != dispatch.credential_vault_ref {
        return Err(failure("credential_identity_mismatch"));
    }
    Ok(())
}
