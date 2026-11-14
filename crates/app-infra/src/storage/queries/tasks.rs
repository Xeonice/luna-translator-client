use super::*;
#[derive(Clone, Copy)]
enum CacheReadPolicy {
    Use,
    Bypass,
}
struct RequestIntent<'a> {
    operation: Option<&'a str>,
    selected_token: Option<LexicalToken>,
    cache_read: CacheReadPolicy,
}
pub(in crate::storage) fn prepare_tasks(
    db: &mut Database,
    uid: &str,
    text_revision: u32,
    purpose: &str,
    now: i64,
) -> AppResult<Vec<TaskDispatch>> {
    prepare_tasks_with_selection(
        db,
        uid,
        text_revision,
        purpose,
        RequestIntent {
            operation: None,
            selected_token: None,
            cache_read: CacheReadPolicy::Use,
        },
        now,
    )
}
pub(in crate::storage) fn prepare_tasks_for_operation(
    db: &mut Database,
    uid: &str,
    text_revision: u32,
    purpose: &str,
    operation: Option<&str>,
    now: i64,
) -> AppResult<Vec<TaskDispatch>> {
    prepare_tasks_with_selection(
        db,
        uid,
        text_revision,
        purpose,
        RequestIntent {
            operation,
            selected_token: None,
            cache_read: CacheReadPolicy::Use,
        },
        now,
    )
}
pub(in crate::storage) fn prepare_uncached_translation_for_operation(
    db: &mut Database,
    uid: &str,
    text_revision: u32,
    operation: &str,
    now: i64,
) -> AppResult<PreparedTranslation> {
    let dispatches = prepare_tasks_with_selection(
        db,
        uid,
        text_revision,
        "translation",
        RequestIntent {
            operation: Some(operation),
            selected_token: None,
            cache_read: CacheReadPolicy::Bypass,
        },
        now,
    )?;
    // Operation receipts are a durable lookup, not a slice of snapshot.tasks.
    let sql = format!(
        "SELECT {TASK_VIEW_COLUMNS} {TASK_VIEW_JOINS} WHERE json_extract(q.input_snapshot_json,'$.operationId')=?1 ORDER BY q.task_id"
    );
    let mut statement = db.conn.prepare(&sql).map_err(err)?;
    let task_views = statement
        .query_map([operation], task_view_row)
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let task_ids = task_views.iter().map(|task| task.task_id.clone()).collect();
    Ok(PreparedTranslation {
        dispatches,
        task_ids,
        task_views,
    })
}
pub(in crate::storage) fn prepare_study_tasks(
    db: &mut Database,
    uid: &str,
    text_revision: u32,
    purpose: &str,
    token_index: Option<u32>,
    operation: &str,
    now: i64,
) -> AppResult<Vec<TaskDispatch>> {
    if text_revision == 0 || !["grammar", "context_word"].contains(&purpose) {
        return Err(AppError::new(
            "invalid_study_object",
            "study",
            "学习必须固定确切原文版本与用途",
        ));
    }
    let source = unit(&db.conn, uid, text_revision)?;
    let selected = if purpose == "context_word" {
        Some(
            source
                .tokens
                .get(token_index.ok_or_else(|| {
                    AppError::new("missing_selection", "study", "语境查词必须明确选择词语")
                })? as usize)
                .ok_or_else(|| AppError::new("stale_token", "study", "所选词语已失效"))?
                .clone(),
        )
    } else {
        None
    };
    prepare_tasks_with_selection(
        db,
        uid,
        text_revision,
        purpose,
        RequestIntent {
            operation: Some(operation),
            selected_token: selected,
            cache_read: CacheReadPolicy::Use,
        },
        now,
    )
}
fn prepare_tasks_with_selection(
    db: &mut Database,
    uid: &str,
    text_revision: u32,
    purpose: &str,
    intent: RequestIntent<'_>,
    now: i64,
) -> AppResult<Vec<TaskDispatch>> {
    let RequestIntent {
        operation,
        selected_token,
        cache_read,
    } = intent;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let unit = unit(&tx, uid, text_revision)?;
    if let Some(op) = operation {
        require_id(op)?;
        let duplicate:Option<(String,u32,String,Option<String>)>=tx.query_row("SELECT unit_id,text_revision,purpose,json_extract(input_snapshot_json,'$.selection') FROM request_tasks WHERE json_extract(input_snapshot_json,'$.operationId')=?1 LIMIT 1",[op],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
        if let Some((old_unit, old_revision, old_purpose, old_selection)) = duplicate {
            if old_unit != uid
                || old_revision != text_revision
                || old_purpose != purpose
                || old_selection
                    .as_ref()
                    .map(|j| parse::<LexicalToken>(j.clone()))
                    .transpose()?
                    != selected_token
            {
                return Err(AppError::new(
                    "operation_conflict",
                    "models",
                    "相同请求操作身份用于不同固定对象或用途",
                ));
            }
            return Ok(vec![]);
        }
    }
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM request_tasks WHERE unit_id=?1 AND text_revision=?2 AND purpose=?3 AND state IN ('queued','running') AND coalesce(json_extract(input_snapshot_json,'$.selection.byteStart'),-1)=?4)",params![uid,text_revision,purpose,selected_token.as_ref().map(|t|i64::from(t.byte_start)).unwrap_or(-1)],|r|r.get(0)).map_err(err)?;
    if active {
        return Ok(vec![]);
    }
    let state = configuration::state(&tx)?;
    let c = state.applied.ok_or_else(|| {
        AppError::new(
            "model_not_configured",
            "models",
            "请先配置并应用翻译或学习用途",
        )
    })?;
    // This entry retains the frozen legacy request grammar. Typed service
    // admission has its own genuine HTTP/offline target and cannot be projected
    // into a fabricated legacy model/connection.
    let c = match c {
        ConfigurationDocument::Legacy { configuration } => configuration,
        ConfigurationDocument::ServiceGraph { configuration } => {
            use app_core::domain::model_config::{
                service_draft::materialize_base,
                service_graph::{PurposeSemanticsVersion, effective_service_binding},
            };
            let catalog_purpose = app_core::domain::model_config::catalog::parse_purpose(purpose)?;
            let current =
                effective_service_binding(&configuration, unit.game_id.as_deref(), catalog_purpose)
                    .ok_or_else(|| {
                        AppError::new("purpose_not_configured", "models", "此用途尚未配置")
                    })?;
            if current.semantics_version != PurposeSemanticsVersion::LegacyV2 {
                return Err(AppError::new(
                    "versioned_task_entry_required",
                    "models",
                    "此配置需使用对应版本的服务任务入口",
                ));
            }
            // An unrelated activation must not replace the immutable execution
            // source of this unchanged legacy purpose with the new whole graph.
            let revision: String = tx.query_row(
                "SELECT config_revision_id FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2 AND binding_id=?3",
                params![current.scope_key, purpose, current.binding_id], |row| row.get(0),
            ).map_err(err)?;
            let source = configuration::load_document(&tx, &revision)?;
            let materialized = materialize_base(&source)?;
            if materialized
                .bindings
                .iter()
                .find(|binding| binding.binding_id == current.binding_id)
                != Some(current)
            {
                return Err(AppError::new(
                    "legacy_dispatch_source_mismatch",
                    "models",
                    "旧用途与其冻结执行来源已改变，拒绝推定旧请求",
                ));
            }
            let ConfigurationDocument::Legacy {
                configuration: source,
            } = source
            else {
                return Err(AppError::new(
                    "legacy_dispatch_source_required",
                    "models",
                    "旧用途需要其真实的旧版本执行来源",
                ));
            };
            source
        }
    };
    let b = effective_binding(&c, unit.game_id.as_deref(), purpose)
        .ok_or_else(|| AppError::new("purpose_not_configured", "models", "此用途尚未配置"))?
        .clone();
    if !b.enabled {
        return Err(AppError::new(
            "purpose_disabled",
            "models",
            "此用途尚未启用",
        ));
    }
    let scope = b.scope_key.clone();
    let (epoch, runtime_enabled): (u32, bool) = tx.query_row("SELECT authorization_epoch,is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2 AND binding_id=?3 AND config_revision_id=?4",params![scope,purpose,b.binding_id,c.revision_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    if !runtime_enabled {
        return Err(AppError::new(
            "purpose_suspended",
            "models",
            "此用途已即时停用，需明确恢复",
        ));
    }
    let (generation,server):(u32,String)=tx.query_row("SELECT runtime_generation,server_instance_id FROM profile_meta WHERE singleton=1 AND runtime_state='active'",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    let mut result = vec![];
    let mut blocked_connections = false;
    for e in b.engines.iter().filter(|e| e.enabled) {
        let m = c
            .models
            .iter()
            .find(|m| m.model_id == e.model_id)
            .ok_or_else(|| AppError::new("missing_model", "models", "模型不存在"))?
            .clone();
        let con = c
            .connections
            .iter()
            .find(|con| con.connection_id == m.connection_id)
            .ok_or_else(|| AppError::new("missing_connection", "models", "连接不存在"))?
            .clone();
        let suspended: bool = tx
            .query_row(
                "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                [&con.connection_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if suspended || !con.availability_intent {
            blocked_connections = true;
            continue;
        }
        let selected_credential = if let Some(kind) =
            app_core::domain::model_config::catalog::credential_kind_for_auth_mode(&con.auth_mode)?
        {
            let pool = con.credential_pool_id.as_ref().ok_or_else(|| {
                AppError::new("missing_credential", "credentials", "连接未选择凭据池")
            })?;
            let rev = con.credential_pool_revision.ok_or_else(|| {
                AppError::new("missing_credential", "credentials", "缺少凭据池版本")
            })?;
            let cursor = *db.pool_cursors.entry((pool.clone(), rev)).or_default();
            let (selected, next) =
                super::credentials::select_at_for_kind(&tx, pool, rev, cursor, kind)?;
            db.pool_cursors.insert((pool.clone(), rev), next);
            Some(selected)
        } else {
            None
        };
        let frozen_history = history(&tx, unit.game_id.as_deref(), None, 100)?.units;
        let mut prepared = TaskDispatch {
            selected_token: selected_token.clone(),
            credential_vault_ref: selected_credential.as_ref().map(|c| c.vault_ref.clone()),
            cached_result: None,
            task_id: String::new(),
            attempt_id: String::new(),
            unit: unit.clone(),
            configuration: c.clone(),
            binding: b.clone(),
            engine: e.clone(),
            model: m.clone(),
            connection: con.clone(),
            request_semantics_hash: String::new(),
            authorization_epoch: epoch,
            profile_generation: generation,
            server_instance_id: server.clone(),
            frozen_history,
        };
        let plan =
            app_core::application::request_plan::compose(&prepared, &prepared.frozen_history)?;
        let cache_input = super::cache::input_identity(&tx, &unit)?;
        let hash = canonical_hash(&(
            "model-request-semantics-v2",
            &plan,
            &cache_input,
            &con.credential_pool_id,
            &con.credential_pool_revision,
            selected_credential.as_ref().map(|c| &c.credential_id),
        ))?;
        let (cached, corrupt) = match cache_read {
            CacheReadPolicy::Use => super::cache::lookup(&tx, &c, &b, &hash, now)?,
            CacheReadPolicy::Bypass => (None, false),
        };
        prepared.cached_result = cached;
        if corrupt {
            db.diagnostics.push(Diagnostic {
                code: "cache_integrity_failure".into(),
                stage: "cache".into(),
                message: "派生缓存摘要或结构无效，已移除并重新请求服务".into(),
                recoverable: true,
            });
        }

        let next: i64 = tx
            .query_row("SELECT count(*) FROM request_tasks", [], |r| r.get(0))
            .map_err(err)?;
        let task_id = id(
            "task",
            &format!(
                "{}:{uid}:{text_revision}:{purpose}:{}:{now}:{next}",
                db.server_id, e.engine_id
            ),
        );
        let attempt_id = id("attempt", &task_id);
        let mut input_snapshot = serde_json::json!({"requestPlan":plan,"cacheInput":cache_input,"cacheReadPolicy":match cache_read {CacheReadPolicy::Use=>"use",CacheReadPolicy::Bypass=>"bypass"},"sourceHash":unit.source_hash,"operationId":operation,"selection":selected_token,"contextRefs":prepared.frozen_history.iter().map(|u|serde_json::json!({"unitId":u.unit_id,"textRevision":u.text_revision,"sourceHash":u.source_hash})).collect::<Vec<_>>()});
        if purpose == "translation" {
            let lineage = translation_status::for_admission(
                &tx,
                &translation_status::TranslationLane {
                    unit_id: uid.into(),
                    text_revision,
                    scope_key: scope.clone(),
                    binding_id: b.binding_id.clone(),
                    engine_id: e.engine_id.clone(),
                },
            )?;
            input_snapshot["translationStatusLineage"] =
                serde_json::to_value(lineage).map_err(err)?;
        }
        tx.execute("INSERT INTO request_tasks VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'queued',NULL,?15,NULL)",params![task_id,uid,text_revision,unit.session_id,scope,purpose,c.revision_id,b.binding_id,e.engine_id,generation,server,epoch,hash,json(&input_snapshot)?,now]).map_err(err)?;
        let credential = selected_credential
            .as_ref()
            .map(|c| c.credential_id.clone());
        tx.execute(
            "UPDATE request_tasks SET state='running' WHERE task_id=?1",
            [&task_id],
        )
        .map_err(err)?;
        tx.execute("INSERT INTO request_attempts VALUES(?1,?2,1,?3,?4,?5,?6,?8,?9,'started',NULL,NULL,'{}',?7,NULL)",params![attempt_id,task_id,c.revision_id,b.binding_id,purpose,e.engine_id,now,credential,if prepared.cached_result.is_some(){"cache"}else{"initial"}]).map_err(err)?;
        prepared.task_id = task_id;
        prepared.attempt_id = attempt_id;
        prepared.request_semantics_hash = hash;
        result.push(prepared);
    }
    if result.is_empty() && blocked_connections {
        return Err(AppError::new(
            "connection_suspended",
            "models",
            "连接已被安全停用，需明确恢复后才能请求",
        ));
    }
    bump(&tx, now)?;
    tx.commit().map_err(err)?;
    Ok(result)
}
pub(in crate::storage) fn complete_task(
    db: &mut Database,
    r: TaskCompletion,
    now: i64,
) -> AppResult<CommandReceipt> {
    if r.text.trim().is_empty() {
        return Err(AppError::new(
            "empty_result",
            "models",
            "服务没有返回有效内容",
        ));
    }
    let handler = super::google_authorization::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let codec:Option<u32>=tx.query_row("SELECT c.codec_version FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=?1",[&r.task_id],|r|r.get(0)).optional().map_err(err)?;
    if codec == Some(3) {
        service_tasks::validate(&tx, &handler, &r.task_id, Some(&r.attempt_id))?;
    }
    let purpose: String = tx
        .query_row(
            "SELECT purpose FROM request_tasks WHERE task_id=?1 AND state='running'",
            [&r.task_id],
            |row| row.get(0),
        )
        .map_err(|_| {
            AppError::new(
                "task_not_authorized",
                "models",
                "任务已取消、失效或完成；拒绝迟到结果",
            )
        })?;
    let current_attempt:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM request_attempts WHERE attempt_id=?1 AND task_id=?2 AND state='started')",params![r.attempt_id,r.task_id],|row|row.get(0)).map_err(err)?;
    if !current_attempt {
        return Err(AppError::new(
            "attempt_not_authorized",
            "models",
            "此尝试已结束；拒绝旧尝试的迟到结果",
        ));
    }
    let selection:Option<String>=tx.query_row("SELECT json_extract(input_snapshot_json,'$.selection') FROM request_tasks WHERE task_id=?1",[&r.task_id],|row|row.get(0)).map_err(err)?;
    let selection = selection.map(parse::<LexicalToken>).transpose()?;
    if purpose != "translation" && r.output_mode == "structured" {
        let source:String=tx.query_row("SELECT u.source_text FROM request_tasks t JOIN reading_unit_revisions u USING(unit_id,text_revision) WHERE t.task_id=?1",[&r.task_id],|row|row.get(0)).map_err(err)?;
        let refs = r
            .structured
            .get("references")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                AppError::new(
                    "invalid_study_references",
                    "study",
                    "结构化解释缺少引用列表",
                )
            })?;
        for item in refs {
            let reference: AnalysisReference = serde_json::from_value(item.clone()).map_err(err)?;
            if reference.byte_start >= reference.byte_end
                || utf16_span(&source, reference.byte_start, reference.byte_end).is_err()
                || source.get(reference.byte_start as usize..reference.byte_end as usize)
                    != Some(reference.quote.as_str())
            {
                return Err(AppError::new(
                    "invalid_study_references",
                    "study",
                    "结构化引用未对应固定原文",
                ));
            }
        }
    }
    let result_id = id(
        if purpose == "translation" {
            "translation"
        } else {
            "study"
        },
        &r.task_id,
    );
    tx.execute("UPDATE request_attempts SET state='completed',finish_reason='stop',finished_at_ms=?2 WHERE attempt_id=?1 AND task_id=?3 AND state='started'",params![r.attempt_id,now,r.task_id]).map_err(err)?;
    tx.execute("UPDATE request_tasks SET state='completed',finished_at_ms=?2 WHERE task_id=?1 AND state='running'",params![r.task_id,now]).map_err(err)?;
    if purpose == "translation" {
        tx.execute(
            "INSERT INTO translations VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                result_id,
                r.task_id,
                r.attempt_id,
                r.text,
                r.target_language,
                r.provider_label,
                now
            ],
        )
        .map_err(err)?;
    } else {
        if !["plain_text", "structured"].contains(&r.output_mode.as_str())
            || !r.structured.is_object()
        {
            return Err(AppError::new(
                "invalid_study_result",
                "study",
                "学习结果结构无效",
            ));
        }
        tx.execute(
            "INSERT INTO study_results VALUES(?1,?2,?3,?4,?5,?6,?7,?9,?10,'[]',?8)",
            params![
                result_id,
                r.task_id,
                r.attempt_id,
                r.output_mode,
                r.text,
                json(&r.structured)?,
                if r.output_mode == "structured" {
                    Some("study-v1")
                } else {
                    None
                },
                now,
                selection.as_ref().map(|t| t.byte_start),
                selection.as_ref().map(|t| t.byte_end)
            ],
        )
        .map_err(err)?;
    }
    super::cache::store(&tx, &r, now)?;
    let revision = bump(&tx, now)?;
    tx.commit().map_err(err)?;
    Ok(CommandReceipt {
        replayed: false,
        request_id: r.task_id,
        revision,
        result_id: Some(result_id),
        game_results: None,
    })
}

pub(in crate::storage) fn validate_task_authority(
    conn: &Connection,
    handler: &super::google_authorization::GoogleHandlerContext,
    task_id: &str,
) -> AppResult<()> {
    let codec:Option<u32>=conn.query_row("SELECT c.codec_version FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=?1",[task_id],|r|r.get(0)).optional().map_err(err)?;
    if codec == Some(3) {
        return service_tasks::validate(conn, handler, task_id, None);
    }
    let allowed:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id JOIN profile_meta p ON p.singleton=1 JOIN purpose_runtime_states s ON s.scope_key=t.scope_key AND s.purpose=t.purpose JOIN purpose_bindings b ON b.revision_id=t.config_revision_id AND b.binding_id=t.binding_id JOIN binding_engines e ON e.revision_id=t.config_revision_id AND e.binding_id=t.binding_id AND e.engine_id=a.actual_engine_id JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN connection_runtime_states r ON r.connection_id=c.connection_id WHERE t.task_id=?1 AND t.state='running' AND a.state='started' AND p.runtime_state='active' AND p.runtime_generation=t.profile_generation AND p.server_instance_id=t.server_instance_id AND s.authorization_epoch=t.authorization_epoch AND s.binding_id=t.binding_id AND s.is_enabled=1 AND b.enabled=1 AND b.binding_mode='override' AND e.enabled=1 AND c.availability_intent=1 AND r.is_suspended=0 AND ((c.auth_mode='none' AND a.credential_id IS NULL) OR (c.auth_mode IN ('api_key','baidu_app_secret','youdao_app_secret','google_service_account') AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr USING(credential_id) WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.enabled=1 AND cr.credential_id=a.credential_id AND cr.state='active' AND cr.auth_kind=c.auth_mode AND ((cr.envelope_schema IS NULL AND cr.auth_kind='api_key') OR cr.envelope_schema=1)))))",[task_id],|r|r.get(0)).map_err(err)?;
    if !allowed {
        return Err(AppError::new(
            "task_not_authorized",
            "models",
            "任务授权已失效；禁止发送网络请求",
        ));
    }
    Ok(())
}

pub(in crate::storage) fn fail_task(
    db: &mut Database,
    task_id: String,
    code: String,
    now: i64,
) -> AppResult<()> {
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let changed = tx.execute("UPDATE request_tasks SET state='failed',invalidation_code=?2,finished_at_ms=?3 WHERE task_id=?1 AND state IN ('queued','running')",params![task_id,code,now]).map_err(err)?;
    let attempts = tx.execute("UPDATE request_attempts SET state='failed',error_code=?2,finished_at_ms=?3 WHERE task_id=?1 AND state='started'",params![task_id,code,now]).map_err(err)?;
    if changed > 0 || attempts > 0 {
        bump(&tx, now)?;
    }
    tx.commit().map_err(err)
}
