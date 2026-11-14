//! Immediate safety controls are independent of immutable draft/applied configuration intent.
use super::*;

fn conflict() -> AppError {
    AppError::new(
        "revision_conflict",
        "configuration",
        "运行授权已改变，请重新核对影响后操作",
    )
}
fn applied(conn: &Connection) -> AppResult<ConfigurationDocument> {
    let revision = conn
        .query_row(
            "SELECT revision_id FROM config_heads WHERE head_kind='applied'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(err)?
        .ok_or_else(|| {
            AppError::new(
                "configuration_not_applied",
                "configuration",
                "请先明确应用配置",
            )
        })?;
    super::configuration::load_document(conn, &revision)
}
fn connection_authorized(
    conn: &Connection,
    c: &app_core::contracts::Connection,
) -> AppResult<bool> {
    if !c.availability_intent {
        return Ok(false);
    }
    if c.auth_mode == "none" {
        return Ok(true);
    }
    let Some(pool) = c.credential_pool_id.as_deref() else {
        return Ok(false);
    };
    let Some(revision) = c.credential_pool_revision else {
        return Ok(false);
    };
    let kind =
        app_core::domain::model_config::catalog::credential_kind_for_auth_mode(&c.auth_mode)?
            .ok_or_else(|| {
                AppError::new("invalid_auth_kind", "configuration", "认证材料类型无效")
            })?;
    Ok(super::credentials::select_at_for_kind(conn, pool, revision, 0, kind).is_ok())
}

fn set_connection_suspended_typed(
    tx: &Transaction,
    connection: &str,
    suspended: bool,
    expected: u32,
    now: i64,
) -> AppResult<()> {
    require_id(connection)?;
    let document = applied(tx)?;
    let c = app_core::domain::model_config::service_draft::materialize_base(&document)?;
    let con = c
        .connections
        .iter()
        .find(|candidate| {
            super::service_selection::connection_control(
                tx,
                &c.revision_id,
                &candidate.connection_id,
            )
            .is_ok_and(|id| id == connection)
        })
        .ok_or_else(|| {
            AppError::new(
                "connection_not_applied",
                "configuration",
                "连接不在当前已应用版本中",
            )
        })?;
    let (current, old): (u32, bool) = tx.query_row(
        "SELECT control_revision,is_suspended FROM connection_runtime_states WHERE connection_id=?1",
        [connection], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(err)?;
    if current != expected {
        return Err(conflict());
    }
    if !suspended && !typed_connection_authorized(tx, con)? {
        return Err(AppError::new(
            "connection_restore_blocked",
            "configuration",
            "已应用连接未启用或凭据已失效，请先修复并明确应用相关版本",
        ));
    }
    if old == suspended {
        return Ok(());
    }
    current.checked_add(1).ok_or_else(|| {
        AppError::new("revision_exhausted", "configuration", "连接控制版本已耗尽")
    })?;
    tx.execute("UPDATE connection_runtime_states SET is_suspended=?2,control_revision=control_revision+1,reason_code=?3,updated_at_ms=?4 WHERE connection_id=?1",params![connection,suspended,if suspended {"user_suspended"} else {"user_restored"},now]).map_err(err)?;
    if suspended {
        // Match each task's frozen revision/engine, rather than current draft models or just purpose.
        let mut query=tx.prepare("SELECT DISTINCT t.task_id,m.revision_id,m.connection_id FROM request_tasks t JOIN binding_engines e ON e.revision_id=t.config_revision_id AND e.binding_id=t.binding_id AND e.engine_id=t.engine_id JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id WHERE t.state IN('queued','running') OR (t.state='invalidated' AND t.invalidation_code='connection_suspended') ORDER BY t.task_id").map_err(err)?;
        let tasks = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        drop(query);
        for (task, revision, target_connection) in tasks {
            // Resolve the entire proven source set rather than an arbitrary row.
            if super::service_selection::connection_control(tx, &revision, &target_connection)?
                != connection
            {
                continue;
            }
            tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='connection_suspended',finished_at_ms=?2 WHERE task_id=?1 AND state IN('queued','running')",params![task,now]).map_err(err)?;
            tx.execute("UPDATE request_attempts SET state='cancelled',error_code='connection_suspended',finished_at_ms=?2 WHERE task_id=?1 AND state='started'",params![task,now]).map_err(err)?;
        }
    }
    Ok(())
}

fn set_purpose_enabled_typed(
    tx: &Transaction,
    scope: &str,
    purpose: &str,
    enabled: bool,
    expected: u32,
    now: i64,
) -> AppResult<()> {
    let document = applied(tx)?;
    let c = app_core::domain::model_config::service_draft::materialize_base(&document)?;
    let binding = c
        .bindings
        .iter()
        .find(|b| {
            b.scope_key == scope
                && purpose_name(b.purpose) == purpose
                && b.binding_mode == "override"
        })
        .ok_or_else(|| {
            AppError::new(
                "purpose_not_applied",
                "configuration",
                "请选择当前已应用的实际用途绑定；继承用途由全局绑定控制",
            )
        })?;
    let (epoch, old): (u32, bool) = tx.query_row(
        "SELECT authorization_epoch,is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2 AND binding_id=?3",
        params![scope,purpose,binding.binding_id], |r| Ok((r.get(0)?,r.get(1)?)),
    ).map_err(err)?;
    if epoch != expected {
        return Err(conflict());
    }
    if enabled && !binding.enabled {
        return Err(AppError::new(
            "purpose_restore_blocked",
            "configuration",
            "当前已应用配置未启用此用途，请先明确应用启用意图",
        ));
    }
    if old == enabled {
        return Ok(());
    }
    epoch.checked_add(1).ok_or_else(|| {
        AppError::new("revision_exhausted", "configuration", "用途授权版本已耗尽")
    })?;
    tx.execute("UPDATE purpose_runtime_states SET is_enabled=?3,authorization_epoch=authorization_epoch+1,updated_at_ms=?4 WHERE scope_key=?1 AND purpose=?2",params![scope,purpose,enabled,now]).map_err(err)?;
    if !enabled {
        tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='purpose_suspended',finished_at_ms=?3 WHERE scope_key=?1 AND purpose=?2 AND state IN ('queued','running')",params![scope,purpose,now]).map_err(err)?;
        tx.execute("UPDATE request_attempts SET state='cancelled',error_code='purpose_suspended',finished_at_ms=?3 WHERE state='started' AND task_id IN (SELECT task_id FROM request_tasks WHERE scope_key=?1 AND purpose=?2 AND state='invalidated' AND invalidation_code='purpose_suspended')",params![scope,purpose,now]).map_err(err)?;
    }
    Ok(())
}

pub(super) fn views(
    conn: &Connection,
    c: Option<&Configuration>,
) -> AppResult<(Vec<ConnectionRuntimeView>, Vec<PurposeRuntimeView>)> {
    let Some(c) = c else {
        return Ok((vec![], vec![]));
    };
    let mut q = conn
        .prepare("SELECT game_id FROM games WHERE archived_at_ms IS NULL ORDER BY game_id")
        .map_err(err)?;
    let games = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut purposes = Vec::new();
    for binding in c.bindings.iter().filter(|b| b.binding_mode == "override") {
        let (epoch, runtime_enabled): (u32,bool) = conn.query_row(
            "SELECT authorization_epoch,is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2",
            params![binding.scope_key,binding.purpose], |r| Ok((r.get(0)?,r.get(1)?)),
        ).map_err(err)?;
        let mut blocked = Vec::new();
        let mut authorized = Vec::new();
        for engine in binding.engines.iter().filter(|e| e.enabled) {
            let model = c
                .models
                .iter()
                .find(|m| m.model_id == engine.model_id)
                .ok_or_else(|| {
                    AppError::new("missing_model", "configuration", "已应用引擎缺少模型")
                })?;
            let con = c
                .connections
                .iter()
                .find(|con| con.connection_id == model.connection_id)
                .ok_or_else(|| {
                    AppError::new("missing_connection", "configuration", "已应用模型缺少连接")
                })?;
            let suspended: bool = conn
                .query_row(
                    "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                    [&con.connection_id],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if suspended || !connection_authorized(conn, con)? {
                if !blocked.contains(&con.connection_id) {
                    blocked.push(con.connection_id.clone());
                }
            } else if binding.enabled && runtime_enabled {
                authorized.push(engine.engine_id.clone());
            }
        }
        let affected_game_ids = games
            .iter()
            .filter(|g| {
                effective_binding(c, Some(g), &binding.purpose)
                    .is_some_and(|b| b.binding_id == binding.binding_id)
            })
            .cloned()
            .collect();
        purposes.push(PurposeRuntimeView {
            scope_key: binding.scope_key.clone(),
            purpose: binding.purpose.clone(),
            binding_id: binding.binding_id.clone(),
            config_revision_id: c.revision_id.clone(),
            authorization_epoch: epoch,
            enabled_intent: binding.enabled,
            runtime_enabled,
            blocked_connection_ids: blocked,
            authorized_engine_ids: authorized,
            affected_game_ids,
        });
    }
    let mut connections = Vec::new();
    for con in &c.connections {
        let (control_revision, suspended, reason_code): (u32,bool,Option<String>) = conn.query_row(
            "SELECT control_revision,is_suspended,reason_code FROM connection_runtime_states WHERE connection_id=?1", [&con.connection_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).map_err(err)?;
        let affected_purposes = purposes
            .iter()
            .filter_map(|p| {
                let b = c.bindings.iter().find(|b| b.binding_id == p.binding_id)?;
                let engine_ids: Vec<_> = b
                    .engines
                    .iter()
                    .filter(|e| {
                        c.models.iter().any(|m| {
                            m.model_id == e.model_id && m.connection_id == con.connection_id
                        })
                    })
                    .map(|e| e.engine_id.clone())
                    .collect();
                (!engine_ids.is_empty()).then(|| PurposeImpact {
                    scope_key: p.scope_key.clone(),
                    purpose: p.purpose.clone(),
                    binding_id: p.binding_id.clone(),
                    enabled_intent: p.enabled_intent,
                    runtime_enabled: p.runtime_enabled,
                    affected_game_ids: p.affected_game_ids.clone(),
                    engine_ids,
                })
            })
            .collect();
        connections.push(ConnectionRuntimeView {
            connection_id: con.connection_id.clone(),
            config_revision_id: c.revision_id.clone(),
            control_revision,
            suspended,
            reason_code,
            availability_intent: con.availability_intent,
            affected_purposes,
        });
    }
    Ok((connections, purposes))
}

pub(super) fn revoke_credential(
    tx: &Transaction,
    credential: &str,
    expected_lifecycle_revision: u32,
    expected_impact_hash: &str,
    now: i64,
) -> AppResult<()> {
    let impact = app_core::application::configuration_controls::credential_impact(
        &super::configuration::state(tx)?,
        &super::credentials::metadata(tx)?,
        &super::credentials::pools(tx)?,
        credential,
    )?;
    if impact.lifecycle_revision != expected_lifecycle_revision
        || impact.impact_hash != expected_impact_hash
    {
        return Err(AppError::new(
            "revision_conflict",
            "credentials",
            "凭据状态或共享影响已改变，请重新核对后撤销",
        ));
    }
    super::credentials::revoke_authority(tx, credential, now)?;
    Ok(())
}

fn purpose_name(p: CatalogPurpose) -> &'static str {
    match p {
        CatalogPurpose::Translation => "translation",
        CatalogPurpose::Grammar => "grammar",
        CatalogPurpose::ContextWord => "context_word",
    }
}
pub(super) fn typed_connection_authorized(
    conn: &Connection,
    c: &app_core::domain::model_config::service_graph::ConnectionV3,
) -> AppResult<bool> {
    if !c.availability_intent {
        return Ok(false);
    }
    if c.auth_mode == "none" {
        return Ok(true);
    }
    if c.auth_mode == "google_oauth" {
        return super::google_authorization::connection_authorized(conn, c);
    }
    let (Some(pool), Some(revision)) = (&c.credential_pool_id, c.credential_pool_revision) else {
        return Ok(false);
    };
    let Some(kind) =
        app_core::domain::model_config::catalog::credential_kind_for_auth_mode(&c.auth_mode)?
    else {
        return Ok(false);
    };
    Ok(super::credentials::select_at_for_kind(conn, pool, revision.get(), 0, kind).is_ok())
}
pub(super) fn document_views(
    conn: &Connection,
    document: Option<&ConfigurationDocument>,
) -> AppResult<(Vec<ConnectionRuntimeView>, Vec<PurposeRuntimeView>)> {
    let Some(document) = document else {
        return Ok((vec![], vec![]));
    };
    if let Some(legacy) = document.as_legacy() {
        return views(conn, Some(legacy));
    }
    use app_core::domain::model_config::service_graph::EngineTarget;
    let graph = app_core::domain::model_config::service_draft::materialize_base(document)?;
    let mut purposes = Vec::new();
    let mut q=conn.prepare("SELECT scope_key,purpose,binding_id,config_revision_id,authorization_epoch,is_enabled FROM purpose_runtime_states ORDER BY scope_key,purpose").map_err(err)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u32>(4)?,
                r.get::<_, bool>(5)?,
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    for (scope, purpose, binding_id, revision, epoch, enabled) in rows {
        // A restored game scope can retain a disabled historical control row
        // because completed tasks still reference it. It is not a current
        // override and must not appear as an independently runnable purpose.
        if !graph.bindings.iter().any(|b| {
            b.scope_key == scope
                && match b.purpose {
                    app_core::contracts::CatalogPurpose::Translation => "translation",
                    app_core::contracts::CatalogPurpose::Grammar => "grammar",
                    app_core::contracts::CatalogPurpose::ContextWord => "context_word",
                } == purpose
                && b.binding_id == binding_id
                && b.binding_mode == "override"
        }) {
            continue;
        }
        // A scope not changed by selection keeps its actual old runtime revision.
        let actual = super::configuration::load_document(conn, &revision)?;
        let g = app_core::domain::model_config::service_draft::materialize_base(&actual)?;
        let Some(binding) = g.bindings.iter().find(|b| b.binding_id == binding_id) else {
            return Err(AppError::new(
                "missing_binding",
                "configuration",
                "运行指针缺少实际绑定",
            ));
        };
        let mut blocked = Vec::new();
        let mut authorized = Vec::new();
        for e in binding.engines.iter().filter(|e| e.enabled) {
            let ready = match &e.target {
                EngineTarget::Http { model_id } => {
                    let m = g
                        .models
                        .iter()
                        .find(|m| &m.model_id == model_id)
                        .ok_or_else(|| {
                            AppError::new("missing_model", "configuration", "运行引擎模型缺失")
                        })?;
                    let c = g
                        .connections
                        .iter()
                        .find(|c| c.connection_id == m.connection_id)
                        .ok_or_else(|| {
                            AppError::new("missing_connection", "configuration", "运行连接缺失")
                        })?;
                    let control = super::service_selection::connection_control(
                        conn,
                        &g.revision_id,
                        &c.connection_id,
                    )?;
                    let suspended:bool=conn.query_row("SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",[&control],|r|r.get(0)).map_err(err)?;
                    if suspended || !typed_connection_authorized(conn, c)? {
                        if !blocked.contains(&control) {
                            blocked.push(control)
                        }
                        false
                    } else {
                        true
                    }
                }
                EngineTarget::Offline { .. } => false,
            };
            if ready && enabled && binding.enabled {
                authorized.push(e.engine_id.clone())
            }
        }
        let mut games = conn
            .prepare("SELECT game_id FROM games WHERE archived_at_ms IS NULL ORDER BY game_id")
            .map_err(err)?;
        let games = games
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        let affected = games
            .into_iter()
            .filter(|game| {
                scope == format!("game:{game}")
                    || (scope == "global"
                        && !graph.bindings.iter().any(|b| {
                            b.scope_key == format!("game:{game}")
                                && purpose_name(b.purpose) == purpose
                                && b.binding_mode == "override"
                        }))
            })
            .collect();
        purposes.push(PurposeRuntimeView {
            scope_key: scope,
            purpose,
            binding_id,
            config_revision_id: revision,
            authorization_epoch: epoch,
            enabled_intent: binding.enabled,
            runtime_enabled: enabled,
            blocked_connection_ids: blocked,
            authorized_engine_ids: authorized,
            affected_game_ids: affected,
        });
    }
    let mut connections = Vec::new();
    let mut seen_controls = std::collections::BTreeSet::new();
    for c in &graph.connections {
        let control = super::service_selection::connection_control(
            conn,
            &graph.revision_id,
            &c.connection_id,
        )?;
        if !seen_controls.insert(control.clone()) {
            continue;
        }
        let mut affected_purposes = Vec::new();
        for runtime in &purposes {
            let actual = super::configuration::load_document(conn, &runtime.config_revision_id)?;
            let frozen = app_core::domain::model_config::service_draft::materialize_base(&actual)?;
            let Some(binding) = frozen
                .bindings
                .iter()
                .find(|b| b.binding_id == runtime.binding_id)
            else {
                continue;
            };
            let mut engine_ids = Vec::new();
            for engine in &binding.engines {
                if let EngineTarget::Http { model_id } = &engine.target
                    && let Some(model) = frozen.models.iter().find(|m| &m.model_id == model_id)
                        && super::service_selection::connection_control(
                            conn,
                            &frozen.revision_id,
                            &model.connection_id,
                        )? == control
                        {
                            engine_ids.push(engine.engine_id.clone())
                        }
            }
            if !engine_ids.is_empty() {
                affected_purposes.push(PurposeImpact {
                    scope_key: runtime.scope_key.clone(),
                    purpose: runtime.purpose.clone(),
                    binding_id: runtime.binding_id.clone(),
                    enabled_intent: runtime.enabled_intent,
                    runtime_enabled: runtime.runtime_enabled,
                    affected_game_ids: runtime.affected_game_ids.clone(),
                    engine_ids,
                })
            }
        }
        let row:Option<(u32,bool,Option<String>)>=conn.query_row("SELECT control_revision,is_suspended,reason_code FROM connection_runtime_states WHERE connection_id=?1",[&control],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
        if let Some((revision, suspended, reason)) = row {
            connections.push(ConnectionRuntimeView {
                connection_id: control,
                config_revision_id: graph.revision_id.clone(),
                control_revision: revision,
                suspended,
                reason_code: reason,
                availability_intent: c.availability_intent,
                affected_purposes,
            })
        }
    }
    Ok((connections, purposes))
}

// Keep existing legacy control semantics and validation obligations unchanged.
fn legacy_applied(conn: &Connection) -> AppResult<Configuration> {
    applied(conn)?.as_legacy().cloned().ok_or_else(|| {
        AppError::new(
            "legacy_configuration_required",
            "configuration",
            "此控制路径只接受旧配置",
        )
    })
}
pub(super) fn set_connection_suspended(
    tx: &Transaction,
    connection: &str,
    suspended: bool,
    expected: u32,
    now: i64,
) -> AppResult<()> {
    if applied(tx)?.as_legacy().is_some() {
        set_connection_suspended_legacy(tx, connection, suspended, expected, now)
    } else {
        set_connection_suspended_typed(tx, connection, suspended, expected, now)
    }
}
pub(super) fn set_purpose_enabled(
    tx: &Transaction,
    scope: &str,
    purpose: &str,
    enabled: bool,
    expected: u32,
    now: i64,
) -> AppResult<()> {
    if applied(tx)?.as_legacy().is_some() {
        set_purpose_enabled_legacy(tx, scope, purpose, enabled, expected, now)
    } else {
        set_purpose_enabled_typed(tx, scope, purpose, enabled, expected, now)
    }
}
fn set_connection_suspended_legacy(
    tx: &Transaction,
    connection: &str,
    suspended: bool,
    expected: u32,
    now: i64,
) -> AppResult<()> {
    require_id(connection)?;
    let c = legacy_applied(tx)?;
    let con = c
        .connections
        .iter()
        .find(|c| c.connection_id == connection)
        .ok_or_else(|| {
            AppError::new(
                "connection_not_applied",
                "configuration",
                "连接不在当前已应用版本中",
            )
        })?;
    let (current, old): (u32, bool) = tx.query_row(
        "SELECT control_revision,is_suspended FROM connection_runtime_states WHERE connection_id=?1",
        [connection], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(err)?;
    if current != expected {
        return Err(conflict());
    }
    if !suspended && !connection_authorized(tx, con)? {
        return Err(AppError::new(
            "connection_restore_blocked",
            "configuration",
            "已应用连接未启用或凭据已失效，请先修复并明确应用相关版本",
        ));
    }
    if old == suspended {
        return Ok(());
    }
    current.checked_add(1).ok_or_else(|| {
        AppError::new("revision_exhausted", "configuration", "连接控制版本已耗尽")
    })?;
    tx.execute("UPDATE connection_runtime_states SET is_suspended=?2,control_revision=control_revision+1,reason_code=?3,updated_at_ms=?4 WHERE connection_id=?1",params![connection,suspended,if suspended {"user_suspended"} else {"user_restored"},now]).map_err(err)?;
    if suspended {
        // Match each task's frozen revision/engine, rather than current draft models or just purpose.
        tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='connection_suspended',finished_at_ms=?2 WHERE state IN ('queued','running') AND EXISTS(SELECT 1 FROM binding_engines e JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id WHERE e.revision_id=request_tasks.config_revision_id AND e.binding_id=request_tasks.binding_id AND e.engine_id=request_tasks.engine_id AND m.connection_id=?1)",params![connection,now]).map_err(err)?;
        tx.execute("UPDATE request_attempts SET state='cancelled',error_code='connection_suspended',finished_at_ms=?2 WHERE state='started' AND task_id IN (SELECT task_id FROM request_tasks WHERE state='invalidated' AND invalidation_code='connection_suspended' AND EXISTS(SELECT 1 FROM binding_engines e JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id WHERE e.revision_id=request_tasks.config_revision_id AND e.binding_id=request_tasks.binding_id AND e.engine_id=request_tasks.engine_id AND m.connection_id=?1))",params![connection,now]).map_err(err)?;
    }
    Ok(())
}

fn set_purpose_enabled_legacy(
    tx: &Transaction,
    scope: &str,
    purpose: &str,
    enabled: bool,
    expected: u32,
    now: i64,
) -> AppResult<()> {
    let c = legacy_applied(tx)?;
    let binding = c
        .bindings
        .iter()
        .find(|b| b.scope_key == scope && b.purpose == purpose && b.binding_mode == "override")
        .ok_or_else(|| {
            AppError::new(
                "purpose_not_applied",
                "configuration",
                "请选择当前已应用的实际用途绑定；继承用途由全局绑定控制",
            )
        })?;
    let (epoch, old): (u32, bool) = tx.query_row(
        "SELECT authorization_epoch,is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2 AND binding_id=?3 AND config_revision_id=?4",
        params![scope,purpose,binding.binding_id,c.revision_id], |r| Ok((r.get(0)?,r.get(1)?)),
    ).map_err(err)?;
    if epoch != expected {
        return Err(conflict());
    }
    if enabled && !binding.enabled {
        return Err(AppError::new(
            "purpose_restore_blocked",
            "configuration",
            "当前已应用配置未启用此用途，请先明确应用启用意图",
        ));
    }
    if old == enabled {
        return Ok(());
    }
    epoch.checked_add(1).ok_or_else(|| {
        AppError::new("revision_exhausted", "configuration", "用途授权版本已耗尽")
    })?;
    tx.execute("UPDATE purpose_runtime_states SET is_enabled=?3,authorization_epoch=authorization_epoch+1,updated_at_ms=?4 WHERE scope_key=?1 AND purpose=?2",params![scope,purpose,enabled,now]).map_err(err)?;
    if !enabled {
        tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='purpose_suspended',finished_at_ms=?3 WHERE scope_key=?1 AND purpose=?2 AND state IN ('queued','running')",params![scope,purpose,now]).map_err(err)?;
        tx.execute("UPDATE request_attempts SET state='cancelled',error_code='purpose_suspended',finished_at_ms=?3 WHERE state='started' AND task_id IN (SELECT task_id FROM request_tasks WHERE scope_key=?1 AND purpose=?2 AND state='invalidated' AND invalidation_code='purpose_suspended')",params![scope,purpose,now]).map_err(err)?;
    }
    Ok(())
}
