//! Real task/attempt admission in the existing SQLite transaction.
use super::*;
pub(in crate::storage) struct TaskPreparation<'a> {
    pub(in crate::storage) unit_id: &'a str,
    pub(in crate::storage) text_revision: u32,
    pub(in crate::storage) purpose: &'a str,
    pub(in crate::storage) operation_id: Option<&'a str>,
    pub(in crate::storage) selected: Option<LexicalToken>,
    pub(in crate::storage) bypass_cache: bool,
    pub(in crate::storage) now: i64,
}
pub(in crate::storage) fn prepare(
    db: &mut Database,
    input: TaskPreparation<'_>,
) -> AppResult<Vec<WorkDispatch>> {
    let TaskPreparation {
        unit_id: uid, text_revision: revision, purpose: p, operation_id: op,
        selected, bypass_cache: bypass, now,
    } = input;
    if !typed_mode(db, uid, revision, p)? {
        return if let Some(selected) = selected {
            let unit = unit(&db.conn, uid, revision)?;
            let index = unit
                .tokens
                .iter()
                .position(|t| t == &selected)
                .ok_or_else(|| failure("stale_token"))?;
            tasks::prepare_study_tasks(
                db,
                uid,
                revision,
                p,
                Some(index as u32),
                op.ok_or_else(|| failure("missing_operation"))?,
                now,
            )
            .map(|v| v.into_iter().map(Into::into).collect())
        } else if p != "translation" {
            tasks::prepare_study_tasks(
                db,
                uid,
                revision,
                p,
                None,
                op.ok_or_else(|| failure("missing_operation"))?,
                now,
            )
            .map(|v| v.into_iter().map(Into::into).collect())
        } else if bypass {
            tasks::prepare_uncached_translation_for_operation(
                db,
                uid,
                revision,
                op.ok_or_else(|| failure("missing_operation"))?,
                now,
            )
            .map(|v| v.dispatches.into_iter().map(Into::into).collect())
        } else {
            tasks::prepare_tasks_for_operation(db, uid, revision, p, op, now)
                .map(|v| v.into_iter().map(Into::into).collect())
        };
    }
    if now < 0 {
        return Err(failure("invalid_time"));
    }
    let directory = db
        .service_directory
        .clone()
        .ok_or_else(|| failure("service_directory_unavailable"))?;
    let handler = super::super::google_authorization::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let unit = unit(&tx, uid, revision)?;
    if let Some(op) = op {
        require_id(op)?;
        let existing:Option<(String,u32,String,Option<String>,String)>=tx.query_row("SELECT unit_id,text_revision,purpose,json_extract(input_snapshot_json,'$.selection'),coalesce(json_extract(input_snapshot_json,'$.cacheReadPolicy'),'use') FROM request_tasks WHERE json_extract(input_snapshot_json,'$.operationId')=?1 LIMIT 1",[op],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(err)?;
        if let Some((old_uid, old_revision, old_p, old_selection, old_policy)) = existing {
            if old_uid != uid
                || old_revision != revision
                || old_p != p
                || old_selection.map(parse::<LexicalToken>).transpose()? != selected
                || old_policy != if bypass { "bypass" } else { "use" }
            {
                return Err(failure("operation_conflict"));
            }
            return Ok(vec![]);
        }
    }
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM request_tasks WHERE unit_id=?1 AND text_revision=?2 AND purpose=?3 AND state IN('queued','running') AND coalesce(json_extract(input_snapshot_json,'$.selection.byteStart'),-1)=?4)",params![uid,revision,p,selected.as_ref().map(|t|i64::from(t.byte_start)).unwrap_or(-1)],|r|r.get(0)).map_err(err)?;
    if active {
        return Ok(vec![]);
    }
    let current = current_graph(&tx)?.ok_or_else(|| failure("configuration_not_applied"))?;
    let purpose = app_core::domain::model_config::catalog::parse_purpose(p)?;
    let current_binding = effective_service_binding(&current, unit.game_id.as_deref(), purpose)
        .ok_or_else(|| failure("purpose_not_configured"))?;
    let (runtime_revision,binding_id,epoch,enabled,resource_hash):(String,String,u32,bool,String)=tx.query_row("SELECT config_revision_id,binding_id,authorization_epoch,is_enabled,config_semantics_sha256 FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2",params![current_binding.scope_key,p],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(err)?;
    if !enabled || !current_binding.enabled {
        return Err(failure("purpose_suspended"));
    }
    let graph = materialize_base(&configuration::load_document(&tx, &runtime_revision)?)?;
    let binding = graph
        .bindings
        .iter()
        .find(|b| {
            b.binding_id == binding_id && b.purpose == purpose && b.binding_mode == "override"
        })
        .ok_or_else(|| failure("missing_runtime_binding"))?;
    if binding != current_binding
        || resource_hash
            != service_request_plan::purpose_service_semantics_hash(&graph, &binding_id)?
    {
        return Err(failure("runtime_configuration_mismatch"));
    }
    let authority = selected_authority(&tx, &current, &unit, current_binding)?;
    let (generation,server):(u32,String)=tx.query_row("SELECT runtime_generation,server_instance_id FROM profile_meta WHERE singleton=1 AND runtime_state='active'",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    let history = history(&tx, unit.game_id.as_deref(), None, 100)?.units;
    let cache_input = cache::input_identity(&tx, &unit)?;
    let scoped_resource_hash = service_request_plan::purpose_service_semantics_hash_for_scope(
        &graph,
        &binding_id,
        &authority.requested_scope,
    )?;
    let mut output = Vec::new();
    let mut cursors = db.pool_cursors.clone();
    let mut last_error = None;
    for engine in binding.engines.iter().filter(|e| e.enabled) {
        let plan = match service_request_plan::compose_service_plan_with_target_origin(
            ServicePlanInput {
                graph: &graph,
                expected_revision_id: &runtime_revision,
                binding_id: &binding_id,
                engine_id: &engine.engine_id,
                unit: &unit,
                selected_token: selected.as_ref(),
                history: &history,
                source_language: &unit.source_language,
                target_language: "zh-Hans",
                catalog: &directory.catalog(),
            },
            TargetLanguageOrigin::BuiltinDefault,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        if let Err(error) = validate_primary_route(&plan.fallback) {
            last_error = Some(error);
            continue;
        }
        let ServiceExecutionTarget::Http { plan: http } = &plan.target else {
            last_error = Some(failure("offline_runtime_unavailable"));
            continue;
        };
        if !crate::providers::adapters::dispatch::installed(&http.connection.protocol)
            || !crate::providers::gateway::authentication_implemented(&http.connection.auth_mode)
        {
            last_error = Some(failure("service_runtime_unavailable"));
            continue;
        }
        if http.connection.protocol == "google_translate_v3"
            && !super::super::google_authorization::handler_ready(&tx, &handler)? {
            last_error = Some(failure("service_runtime_unavailable"));
            continue;
        }
        let control = service_selection::connection_control(
            &tx,
            &runtime_revision,
            &http.connection.connection_id,
        )?;
        let suspended: bool = tx
            .query_row(
                "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                [control],
                |r| r.get(0),
            )
            .map_err(err)?;
        if suspended || !http.connection.availability_intent {
            last_error = Some(failure("connection_suspended"));
            continue;
        }
        let mut selected_credential = None;
        let mut credential_fact = None;
        let material_kind = if http.connection.auth_mode == "google_oauth"
            && http.connection.protocol == "google_translate_v3"
        {
            if !super::super::google_authorization::connection_authorized(&tx, &http.connection)? {
                last_error = Some(failure("google_grant_inactive"));
                continue;
            }
            None
        } else {
            app_core::domain::model_config::catalog::credential_kind_for_auth_mode(
                &http.connection.auth_mode,
            )?
        };
        if let Some(kind) = material_kind {
            let pool = http
                .connection
                .credential_pool_id
                .as_deref()
                .ok_or_else(|| failure("missing_credential"))?;
            let rev = http
                .connection
                .credential_pool_revision
                .ok_or_else(|| failure("missing_credential"))?
                .get();
            let cursor = *cursors.entry((pool.into(), rev)).or_default();
            let (selection, next) =
                match credentials::select_at_for_kind(&tx, pool, rev, cursor, kind) {
                    Ok(value) => value,
                    Err(error) => {
                        last_error = Some(error);
                        continue;
                    }
                };
            cursors.insert((pool.into(), rev), next);
            let (lifecycle,auth_kind,envelope):(u32,String,Option<u32>)=tx.query_row("SELECT lifecycle_revision,auth_kind,envelope_schema FROM credential_refs WHERE credential_id=?1 AND state='active'",[&selection.credential_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
            app_core::domain::model_config::credentials::CredentialStorageMetadata::from_stored(
                &auth_kind, envelope,
            )?;
            credential_fact = Some(CredentialFact {
                credential_id: selection.credential_id.clone(),
                pool_id: pool.into(),
                pool_revision: rev,
                member_id: selection.member_id.clone(),
                lifecycle_revision: lifecycle,
                auth_kind,
                envelope_schema: envelope,
                version_sha256: content_hash(&selection.vault_ref),
            });
            selected_credential = Some(selection);
        }
        let target_hash = canonical_hash(&(
            "service-target-v1",
            &plan.identity.configuration_revision,
            &plan.identity.binding_id,
            &plan.identity.engine_id,
            &plan.target,
        ))?;
        let request_hash = canonical_hash(&(
            "service-request-v3",
            CACHE_POLICY,
            &plan.target,
            &plan.policy,
            &cache_input,
            &scoped_resource_hash,
            &credential_fact,
        ))?;
        let cached = if !bypass && plan.policy.cache.enabled {
            cache::lookup_service(
                &tx,
                p,
                &request_hash,
                &scoped_resource_hash,
                &target_hash,
                now,
            )?
        } else {
            (None, false)
        };
        if cached.1 {
            db.diagnostics.push(Diagnostic {
                code: "cache_integrity_failure".into(),
                stage: "cache".into(),
                message: "派生缓存摘要或结构无效，已删除并重新执行".into(),
                recoverable: true,
            });
        }
        let count: i64 = tx
            .query_row("SELECT count(*) FROM request_tasks", [], |r| r.get(0))
            .map_err(err)?;
        let task = id(
            "task",
            &format!(
                "{}:{uid}:{revision}:{p}:{}:{now}:{count}",
                db.server_id, engine.engine_id
            ),
        );
        let attempt = id("attempt", &task);
        let lineage = if purpose == CatalogPurpose::Translation {
            Some(translation_status::for_admission(
                &tx,
                &translation_status::TranslationLane {
                    unit_id: uid.into(),
                    text_revision: revision,
                    scope_key: binding.scope_key.clone(),
                    binding_id: binding_id.clone(),
                    engine_id: engine.engine_id.clone(),
                },
            )?)
        } else {
            None
        };
        let snapshot = AdmissionSnapshot {
            schema: "service-admission-v1".into(),
            plan_sha256: canonical_hash(&plan)?,
            service_plan: plan.clone(),
            target_sha256: target_hash.clone(),
            resource_semantics_sha256: scoped_resource_hash.clone(),
            cache_input: cache_input.clone(),
            cache_policy_version: CACHE_POLICY.into(),
            operation_id: op.map(str::to_owned),
            selection: selected.clone(),
            source_hash: unit.source_hash.clone(),
            selection_authority: authority.clone(),
            credential_authority: credential_fact,
            cache_read_policy: if bypass { "bypass" } else { "use" }.into(),
            translation_status_lineage: lineage,
        };
        tx.execute("INSERT INTO request_tasks VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'queued',NULL,?15,NULL)",params![task,uid,revision,unit.session_id,binding.scope_key,p,runtime_revision,binding_id,engine.engine_id,generation,server,epoch,request_hash,json(&snapshot)?,now]).map_err(err)?;
        tx.execute("INSERT INTO task_execution_targets(task_id,config_revision_id,binding_id,engine_id,purpose,target_kind,http_model_id,http_connection_id,selection_scope,selection_revision,effective_scope,effective_selection_revision,target_sha256) VALUES(?1,?2,?3,?4,?5,'http',?6,?7,?8,?9,?10,?11,?12)",params![task,runtime_revision,binding_id,engine.engine_id,p,http.model.model_id,http.connection.connection_id,authority.selection_scope,authority.selection_revision,authority.effective_scope,authority.effective_revision,target_hash]).map_err(err)?;
        tx.execute(
            "UPDATE request_tasks SET state='running' WHERE task_id=?1",
            [&task],
        )
        .map_err(err)?;
        tx.execute("INSERT INTO request_attempts(attempt_id,task_id,attempt_number,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,dispatch_kind,state,error_code,finish_reason,usage_json,started_at_ms,finished_at_ms) VALUES(?1,?2,1,?3,?4,?5,?6,?7,?8,'started',NULL,NULL,'{}',?9,NULL)",params![attempt,task,runtime_revision,binding_id,p,engine.engine_id,selected_credential.as_ref().map(|s|&s.credential_id),if cached.0.is_some(){"cache"}else{"initial"},now]).map_err(err)?;
        tx.execute("INSERT INTO attempt_execution_targets(attempt_id,task_id,config_revision_id,binding_id,engine_id,target_kind,http_model_id,http_connection_id) VALUES(?1,?2,?3,?4,?5,'http',?6,?7)",params![attempt,task,runtime_revision,binding_id,engine.engine_id,http.model.model_id,http.connection.connection_id]).map_err(err)?;
        output.push(WorkDispatch::from(ServiceTaskDispatch {
            task_id: task,
            attempt_id: attempt,
            unit: unit.clone(),
            plan,
            request_semantics_hash: request_hash,
            authorization_epoch: epoch,
            profile_generation: generation,
            server_instance_id: server.clone(),
            credential_vault_ref: selected_credential.map(|s| s.vault_ref),
            cached_result: cached.0,
        }));
    }
    if output.is_empty() {
        return Err(last_error.unwrap_or_else(|| failure("missing_engine")));
    }
    bump(&tx, now)?;
    tx.commit().map_err(err)?;
    db.pool_cursors = cursors;
    Ok(output)
}
