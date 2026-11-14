use super::database::{Database, err};
use app_core::{contracts::*, domain::*};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::collections::BTreeMap;
mod cache;
mod capture;
pub(super) mod source_selection;
mod configuration;
pub(in crate::storage) mod configuration_codec;
mod configuration_lifecycle;
mod service_selection;
pub(in crate::storage) mod service_graph;
mod credentials;
mod google_authorization;
pub(in crate::storage) mod offline_resources;
pub(in crate::storage) use google_authorization::{bound_runtime_frame as google_runtime_frame, handler_context as google_handler_context};
pub(super) mod game_assets;
mod history;
mod model_tests;
mod service_tests;
mod saved_service_source;
mod service_discovery;
pub(in crate::storage) mod provider_presets;
mod retention;
pub(in crate::storage) mod session_records;
pub(super) use retention::maintain_storage;
mod retries;
mod tasks;
pub(super) mod service_tasks;
mod translation_status;
pub(super) use capture::*;
pub(super) use credentials::*;
pub(super) use model_tests::{credential_test_reference, record_model_test};
pub(super) use retries::retry_attempt;
pub(super) use tasks::{
    complete_task, fail_task, prepare_study_tasks, prepare_tasks, prepare_tasks_for_operation,
    prepare_uncached_translation_for_operation, validate_task_authority,
};
pub(super) use translation_status::reading_unit_projection;
fn json<T: serde::Serialize>(v: &T) -> AppResult<String> {
    serde_json::to_string(v).map_err(err)
}
fn parse<T: serde::de::DeserializeOwned>(v: String) -> AppResult<T> {
    serde_json::from_str(&v).map_err(err)
}
fn id(prefix: &str, operation: &str) -> String {
    format!("{prefix}-{}", content_hash(operation))
}
fn revision(conn: &Connection) -> AppResult<u32> {
    conn.query_row(
        "SELECT commit_revision FROM profile_meta WHERE singleton=1",
        [],
        |r| r.get(0),
    )
    .map_err(err)
}
fn bump(conn: &Connection, now: i64) -> AppResult<u32> {
    conn.execute("UPDATE profile_meta SET commit_revision=commit_revision+1,updated_at_ms=?1 WHERE singleton=1",[now]).map_err(err)?;
    revision(conn)
}
fn receipt(
    tx: &Transaction,
    e: &CommandEnvelope,
    hash: &str,
    kind: &str,
    result: (Option<String>, Vec<GameSaveResult>),
    now: i64,
) -> AppResult<CommandReceipt> {
    let rev = bump(tx, now)?;
    let r = CommandReceipt {
        replayed: false,
        request_id: e.operation_id.clone(),
        revision: rev,
        result_id: result.0,
        game_results: (!result.1.is_empty()).then_some(result.1),
    };
    tx.execute(
        "INSERT INTO operation_receipts VALUES(?1,?2,?3,?4,?5,?6)",
        params![e.operation_id, hash, kind, rev, json(&r)?, now],
    )
    .map_err(err)?;
    Ok(r)
}
fn existing_receipt(conn: &Connection, op: &str, hash: &str) -> AppResult<Option<CommandReceipt>> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT request_sha256,result_refs_json FROM operation_receipts WHERE operation_id=?1",
            [op],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    match row {
        Some((h, _)) if h != hash => Err(AppError::new(
            "operation_conflict",
            "command",
            "相同操作身份用于不同请求",
        )),
        Some((_, r)) => {
            let mut receipt: CommandReceipt = parse(r)?;
            receipt.replayed = true;
            Ok(Some(receipt))
        }
        None => Ok(None),
    }
}
fn window_unbind_request(command: &Command) -> AppResult<&GameWindowActivationRequest> {
    let Command::UnbindWindow(request) = command else {
        return Err(AppError::new(
            "invalid_command_preflight",
            "command",
            "只允许核对窗口解绑操作",
        ));
    };
    require_id(&request.expected_session_id)?;
    if request.expected_target_identity_sha256.len() != 64
        || !request
            .expected_target_identity_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(AppError::new(
            "invalid_window_identity",
            "command",
            "窗口身份必须为完整的规范 SHA-256",
        ));
    }
    Ok(request)
}
fn require_unbind_session(
    conn: &Connection,
    request: &GameWindowActivationRequest,
) -> AppResult<()> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id=?1 AND source_kind='hook')",
            [&request.expected_session_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if !exists {
        return Err(AppError::new(
            "game_window_unbind_stale",
            "command",
            "解绑请求所属的取文会话不存在或不是 Hook 会话",
        ));
    }
    Ok(())
}
pub(super) fn preflight_window_unbind(
    db: &Database,
    e: CommandEnvelope,
) -> AppResult<Option<CommandReceipt>> {
    require_id(&e.operation_id)?;
    let request = window_unbind_request(&e.command)?;
    if let Some(receipt) =
        existing_receipt(&db.conn, &e.operation_id, &canonical_hash(&e.command)?)?
    {
        return Ok(Some(receipt));
    }
    // Unrelated text/model commits cannot invalidate a frozen window identity.
    // Current native identity is checked inside the adapter's same lease.
    require_unbind_session(&db.conn, request)?;
    Ok(None)
}
pub(super) fn commit_window_unbind(
    db: &mut Database,
    e: CommandEnvelope,
    now: i64,
) -> AppResult<CommandReceipt> {
    window_unbind_request(&e.command)?;
    dispatch_after_native_effect(db, e, now, true)
}
pub(super) fn preflight_disconnect(
    db: &Database,
    e: CommandEnvelope,
) -> AppResult<Option<CommandReceipt>> {
    require_id(&e.operation_id)?;
    if !matches!(e.command, Command::Disconnect) {
        return Err(AppError::new(
            "invalid_command_preflight",
            "command",
            "只允许核对断开操作",
        ));
    }
    if let Some(receipt) =
        existing_receipt(&db.conn, &e.operation_id, &canonical_hash(&e.command)?)?
    {
        return Ok(Some(receipt));
    }
    if let Some(expected) = e.expected_revision
        && revision(&db.conn)? != expected
    {
        return Err(AppError::new(
            "revision_conflict",
            "command",
            "其他窗口已修改数据，请保留编辑并重新核对",
        ));
    }
    Ok(None)
}
/// Shared single-record write. The caller owns the transaction and operation receipt.
fn save_game(
    tx: &Transaction,
    input: &GameInput,
    fallback_id: &str,
    now: i64,
) -> AppResult<String> {
    validate_game(input)?;
    let gid = input.game_id.as_deref().unwrap_or(fallback_id);
    let target_id = id("target", gid);
    if input.game_id.is_some() {
        let current: u32 = tx
            .query_row(
                "SELECT row_revision FROM games WHERE game_id=?1 AND archived_at_ms IS NULL",
                [gid],
                |r| r.get(0),
            )
            .map_err(err)?;
        if input.row_revision != Some(current) {
            return Err(AppError::new(
                "revision_conflict",
                "library",
                "游戏资料已更改，请保留编辑并重新核对",
            ));
        }
        tx.execute("UPDATE games SET display_name=?2,original_title=?3,source_language=?4,cover_asset_ref=?5,row_revision=row_revision+1,updated_at_ms=?6 WHERE game_id=?1",params![gid,input.display_name,input.original_title,input.source_language,input.cover_asset_ref,now]).map_err(err)?;
    } else {
        tx.execute(
            "INSERT INTO games VALUES(?1,?2,?3,?4,?5,1,?6,?6,NULL)",
            params![
                gid,
                input.display_name,
                input.original_title,
                input.source_language,
                input.cover_asset_ref,
                now
            ],
        )
        .map_err(err)?;
    }
    let t = &input.target;
    tx.execute("INSERT INTO game_targets VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,1,?13,NULL) ON CONFLICT(game_id) DO UPDATE SET environment_kind=excluded.environment_kind,environment_key=excluded.environment_key,bottle_locator=excluded.bottle_locator,launch_path=excluded.launch_path,canonical_exe_path=excluded.canonical_exe_path,target_path_key=excluded.target_path_key,working_directory=excluded.working_directory,arguments_json=excluded.arguments_json,architecture=excluded.architecture,verification_state=excluded.verification_state,row_revision=game_targets.row_revision+1,updated_at_ms=excluded.updated_at_ms",params![target_id,gid,t.environment_kind,t.environment_key,t.bottle_locator,t.launch_path,t.canonical_exe_path,t.target_path_key,t.working_directory,json(&t.arguments)?,t.architecture,t.verification_state,now]).map_err(err)?;
    Ok(gid.to_owned())
}
pub(super) fn dispatch(
    db: &mut Database,
    e: CommandEnvelope,
    now: i64,
) -> AppResult<CommandReceipt> {
    if matches!(e.command, Command::UnbindWindow(_)) {
        return Err(AppError::new(
            "native_effect_required",
            "command",
            "窗口解绑必须经受控接入事务执行",
        ));
    }
    dispatch_after_native_effect(db, e, now, false)
}
fn dispatch_after_native_effect(
    db: &mut Database,
    mut e: CommandEnvelope,
    now: i64,
    window_unbound: bool,
) -> AppResult<CommandReceipt> {
    require_id(&e.operation_id)?;
    match &mut e.command {
        Command::SaveGame(input) => normalize_game_input(input)?,
        Command::SaveGames { items } => {
            app_core::domain::library::normalize_game_batch(items)?;
            if e.expected_revision.is_none() {
                return Err(AppError::new(
                    "revision_required",
                    "library",
                    "批量添加需要当前游戏库版本，请重新核对后保存",
                ));
            }
        }
        _ => {}
    }
    if now < 0 {
        return Err(AppError::new("invalid_time", "command", "时间不得为负"));
    }
    let hash = canonical_hash(&e.command)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if let Some(r) = existing_receipt(&tx, &e.operation_id, &hash)? {
        return Ok(r);
    }
    if let Command::UnbindWindow(_) = &e.command {
        if !window_unbound {
            return Err(AppError::new(
                "native_effect_required",
                "command",
                "窗口解绑必须经受控接入事务执行",
            ));
        }
        require_unbind_session(&tx, window_unbind_request(&e.command)?)?;
    }
    if !matches!(
        e.command,
        Command::SaveScopedPreferences { .. } | Command::UnbindWindow(_)
    ) && let Some(expected) = e.expected_revision
        && revision(&tx)? != expected
    {
        return Err(AppError::new(
            "revision_conflict",
            "command",
            "其他窗口已修改数据，请保留编辑并重新核对",
        ));
    }
    let mut game_results = Vec::new();
    let (kind, result) = match &e.command {
        Command::SaveGame(input) => {
            let gid = save_game(&tx, input, &id("game", &e.operation_id), now)?;
            ("save_game", Some(gid))
        }
        Command::SaveGames { items } => {
            for item in items {
                let existing = if item.game.target.environment_kind == "crossover"
                    && item.game.target.target_path_key.is_some()
                {
                    tx.query_row(
                        "SELECT g.game_id,g.row_revision FROM games g JOIN game_targets t USING(game_id) WHERE g.archived_at_ms IS NULL AND t.archived_at_ms IS NULL AND t.environment_key=?1 AND t.target_path_key=?2",
                        params![item.game.target.environment_key, item.game.target.target_path_key],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)),
                    ).optional().map_err(err)?
                } else {
                    None
                };
                let (game_id, row_revision, reused) =
                    if let Some((game_id, row_revision)) = existing {
                        (game_id, row_revision, true)
                    } else {
                        let fallback = id("game", &format!("{}\0{}", e.operation_id, item.item_id));
                        (save_game(&tx, &item.game, &fallback, now)?, 1, false)
                    };
                game_results.push(GameSaveResult {
                    item_id: item.item_id.clone(),
                    game_id,
                    row_revision,
                    reused,
                });
            }
            ("save_games", None)
        }
        Command::ArchiveGame {
            game_id,
            row_revision,
        } => {
            if tx.execute("UPDATE games SET archived_at_ms=?3,updated_at_ms=?3,row_revision=row_revision+1 WHERE game_id=?1 AND row_revision=?2 AND archived_at_ms IS NULL",params![game_id,row_revision,now]).map_err(err)?!=1{return Err(AppError::new("revision_conflict","library","游戏已更改或移出"));}
            tx.execute(
                "UPDATE game_targets SET archived_at_ms=?2 WHERE game_id=?1",
                params![game_id, now],
            )
            .map_err(err)?;
            ("archive_game", Some(game_id.clone()))
        }
        Command::ManualInput {
            text,
            source_language,
        } => {
            if text.trim().is_empty() || text.contains('\0') {
                return Err(AppError::new(
                    "invalid_text",
                    "reading",
                    "原文不能为空或包含空字符",
                ));
            }
            let sid = id("manual", &e.operation_id);
            let uid = id("unit", &e.operation_id);
            tx.execute("INSERT INTO sessions(session_id,source_kind,state,started_at_ms) VALUES(?1,'manual','active',?2)",params![sid,now]).map_err(err)?;
            tx.execute(
                "INSERT INTO reading_units VALUES(?1,?2,1,'finalized',?3)",
                params![uid, sid, now],
            )
            .map_err(err)?;
            tx.execute("INSERT INTO reading_unit_revisions VALUES(?1,1,?2,?3,?4,?5,?6,'manual-v1','{}',NULL,?7)",params![uid,sid,text,text.len() as i64,content_hash(text),source_language,now]).map_err(err)?;
            ("manual_input", Some(uid))
        }
        Command::CopyModelConfiguration {
            model_id,
            new_display_name,
            expected_head_revision,
        } => {
            configuration::check_head(&tx, "draft", *expected_head_revision)?;
            let draft: String = tx
                .query_row(
                    "SELECT revision_id FROM config_heads WHERE head_kind='draft'",
                    [],
                    |r| r.get(0),
                )
                .map_err(err)?;
            let original = configuration::load(&tx, &draft)?;
            let (copied, new_model) = app_core::domain::model_config::templates::copy_model(
                &original,
                model_id,
                new_display_name,
                &e.operation_id,
            )?;
            configuration::save(&tx, &copied, now)?;
            ("copy_model_configuration", Some(new_model))
        }
        Command::SaveConfiguration {
            configuration,
            expected_head_revision,
        } => {
            configuration::check_head(&tx, "draft", *expected_head_revision)?;
            validate_configuration(configuration, false)?;
            let mut c = configuration.clone();
            c.revision_id = id("config", &e.operation_id);
            configuration::save(&tx, &c, now)?;
            ("save_configuration", Some(c.revision_id))
        }
        Command::ApplyConfiguration {
            revision_id,
            expected_head_revision,
        } => {
            configuration::check_head(&tx, "applied", *expected_head_revision)?;
            configuration::apply(&tx, revision_id, now)?;
            ("apply_configuration", Some(revision_id.clone()))
        }
        Command::SetConnectionSuspended {
            connection_id,
            suspended,
            expected_control_revision,
        } => {
            configuration_lifecycle::set_connection_suspended(
                &tx,
                connection_id,
                *suspended,
                *expected_control_revision,
                now,
            )?;
            ("set_connection_suspended", Some(connection_id.clone()))
        }
        Command::SetPurposeRuntimeEnabled {
            scope_key,
            purpose,
            enabled,
            expected_authorization_epoch,
        } => {
            configuration_lifecycle::set_purpose_enabled(
                &tx,
                scope_key,
                purpose,
                *enabled,
                *expected_authorization_epoch,
                now,
            )?;
            (
                "set_purpose_runtime_enabled",
                Some(format!("{scope_key}:{purpose}")),
            )
        }
        Command::SavePreferences { preferences: p } => {
            let pid = save_preferences(&tx, p, &e.operation_id, now)?;
            ("save_preferences", Some(pid))
        }
        Command::SaveScopedPreferences {
            preferences: p,
            expected_head_revision,
        } => {
            validate_preference_scope(&tx, &p.scope_key)?;
            let actual: Option<u32> = tx
                .query_row(
                    "SELECT head_revision FROM preference_heads WHERE scope_key=?1",
                    [&p.scope_key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            if actual != *expected_head_revision {
                return Err(AppError::new(
                    "preference_head_conflict",
                    "preferences",
                    "该作用范围的偏好已被其他窗口修改，请重新读取后核对",
                ));
            }
            let pid = save_preferences(&tx, p, &e.operation_id, now)?;
            ("save_scoped_preferences", Some(pid))
        }
        Command::SetHudPreference {
            preference,
            enabled,
        } => {
            let mut preferences = effective_preferences(&tx)?;
            if matches!(preference, HudPreference::Segmentation) {
                preferences.segmentation_enabled = *enabled;
                preferences.overrides.remove("reading.segmentationEnabled");
                if preferences.scope_key != "global" {
                    preferences
                        .overrides
                        .insert("segmentationEnabled".into(), true);
                }
            } else {
                preferences
                    .overrides
                    .insert(preference.key().into(), *enabled);
            }
            let pid = save_preferences(&tx, &preferences, &e.operation_id, now)?;
            ("set_hud_preference", Some(pid))
        }
        Command::SetReadingAnnotations {
            show_ruby,
            show_pos,
        } => {
            // Load within this writer transaction, never from a stale HUD snapshot.
            let mut preferences = effective_preferences(&tx)?;
            if preferences.scope_key != "global" {
                preferences.overrides.insert("showRuby".into(), true);
                preferences.overrides.insert("showPos".into(), true);
            }
            preferences.show_ruby = *show_ruby;
            preferences.show_pos = *show_pos;
            let pid = save_preferences(&tx, &preferences, &e.operation_id, now)?;
            ("set_reading_annotations", Some(pid))
        }
        Command::CycleReadingDisplayMode => {
            let mut preferences = effective_preferences(&tx)?;
            preferences.reading_display_mode = preferences.reading_display_mode.next();
            if preferences.scope_key != "global" {
                preferences
                    .overrides
                    .insert("readingDisplayMode".into(), true);
            }
            let pid = save_preferences(&tx, &preferences, &e.operation_id, now)?;
            ("cycle_reading_display_mode", Some(pid))
        }
        Command::SaveCorrection {
            translation_id,
            base_revision,
            text,
        } => {
            if text.trim().is_empty() {
                return Err(AppError::new(
                    "empty_correction",
                    "history",
                    "人工修正不能为空",
                ));
            }
            let current:u32=tx.query_row("SELECT coalesce(max(correction_revision),0) FROM corrections WHERE translation_id=?1",[translation_id],|r|r.get(0)).map_err(err)?;
            if current != *base_revision {
                return Err(AppError::new(
                    "revision_conflict",
                    "history",
                    "人工修正已更新，请保留本地编辑",
                ));
            }
            let cid = id("correction", &e.operation_id);
            tx.execute(
                "INSERT INTO corrections VALUES(?1,?2,?3,?4,?5,?6)",
                params![cid, translation_id, current + 1, text, base_revision, now],
            )
            .map_err(err)?;
            ("save_correction", Some(cid))
        }
        Command::CancelTask { task_id } => {
            tx.execute("UPDATE request_tasks SET state='cancelled',invalidation_code='user_cancelled',finished_at_ms=?2 WHERE task_id=?1 AND state IN ('queued','running')",params![task_id,now]).map_err(err)?;
            tx.execute("UPDATE request_attempts SET state='cancelled',error_code='user_cancelled',finished_at_ms=?2 WHERE task_id=?1 AND state='started'",params![task_id,now]).map_err(err)?;
            ("cancel_task", Some(task_id.clone()))
        }
        Command::Disconnect => {
            capture::seal_capture_units(&tx, None, now, false)?;
            tx.execute("UPDATE sessions SET state='stopped',ended_at_ms=?1 WHERE source_kind='hook' AND state IN ('opening','active')",[now]).map_err(err)?;
            ("disconnect", None)
        }
        Command::SelectSource { source_id } => {
            let sid: String = tx
                .query_row(
                    "SELECT session_id FROM capture_sources WHERE source_id=?1",
                    [source_id],
                    |r| r.get(0),
                )
                .map_err(err)?;
            source_selection::insert_set(&tx, &sid, &e.operation_id, std::slice::from_ref(source_id), "temporary", "{}", now)?;
            ("select_source", Some(source_id.clone()))
        }
        Command::SetCapturePaused { paused } => {
            if *paused {
                capture::seal_capture_units(&tx, None, now, false)?;
            }
            ("set_capture_paused", None)
        }
        Command::SetTranslationEnabled { .. } => ("set_translation_enabled", None),
        Command::RevokeCredential {
            credential_id,
            expected_lifecycle_revision,
            expected_impact_hash,
        } => {
            configuration_lifecycle::revoke_credential(
                &tx,
                credential_id,
                *expected_lifecycle_revision,
                expected_impact_hash,
                now,
            )?;
            ("revoke_credential", Some(credential_id.clone()))
        }
        Command::SaveCredentialPool { pool } => {
            let (pool_id, revision) = credentials::save_pool(&tx, pool, &e.operation_id, now)?;
            (
                "save_credential_pool",
                Some(format!("{pool_id}@{revision}")),
            )
        }
        Command::UnbindWindow(request) => (
            "unbind_window",
            Some(request.expected_target_identity_sha256.clone()),
        ),
    };
    session_records::record_command_event(&tx, &e.command, &e.operation_id, now)?;
    let r = receipt(&tx, &e, &hash, kind, (result, game_results), now)?;
    tx.commit().map_err(err)?;
    Ok(r)
}
fn games(conn: &Connection) -> AppResult<Vec<Game>> {
    let mut stmt=conn.prepare("SELECT g.game_id,g.display_name,g.original_title,g.source_language,g.cover_asset_ref,g.row_revision,t.environment_kind,t.environment_key,t.bottle_locator,t.launch_path,t.canonical_exe_path,t.target_path_key,t.working_directory,t.arguments_json,t.architecture,t.verification_state FROM games g JOIN game_targets t USING(game_id) WHERE g.archived_at_ms IS NULL ORDER BY g.updated_at_ms DESC,g.game_id").map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                Game {
                    game_id: r.get(0)?,
                    display_name: r.get(1)?,
                    original_title: r.get(2)?,
                    source_language: r.get(3)?,
                    cover_asset_ref: r.get(4)?,
                    row_revision: r.get(5)?,
                    target: GameTarget {
                        environment_kind: r.get(6)?,
                        environment_key: r.get(7)?,
                        bottle_locator: r.get(8)?,
                        launch_path: r.get(9)?,
                        canonical_exe_path: r.get(10)?,
                        target_path_key: r.get(11)?,
                        working_directory: r.get(12)?,
                        arguments: vec![],
                        architecture: r.get(14)?,
                        verification_state: r.get(15)?,
                    },
                },
                r.get::<_, String>(13)?,
            ))
        })
        .map_err(err)?;
    rows.map(|r| {
        let (mut g, args) = r.map_err(err)?;
        g.target.arguments = parse(args)?;
        Ok(g)
    })
    .collect()
}
pub(super) fn unit(conn: &Connection, uid: &str, rev: u32) -> AppResult<ReadingUnit> {
    let rev = if rev == 0 {
        conn.query_row(
            "SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=?1",
            [uid],
            |r| r.get(0),
        )
        .map_err(err)?
    } else {
        rev
    };
    let mut u=conn.query_row("SELECT r.unit_id,r.session_id,s.game_id,r.text_revision,r.source_text,r.source_sha256,r.source_language,r.created_at_ms,s.session_id,s.game_id,s.source_kind,s.state,s.started_at_ms,s.ended_at_ms FROM reading_unit_revisions r JOIN sessions s USING(session_id) WHERE r.unit_id=?1 AND r.text_revision=?2",params![uid,rev],|r|Ok(ReadingUnit{source_summary:Some(history::session_summary(r,8)?),unit_id:r.get(0)?,session_id:r.get(1)?,game_id:r.get(2)?,text_revision:r.get(3)?,source_text:r.get(4)?,source_hash:r.get(5)?,source_language:r.get(6)?,created_at_ms:r.get::<_,i64>(7)? as f64,translations:vec![],tokens:vec![]})).map_err(err)?;
    let mut stmt=conn.prepare("SELECT t.translation_id,t.task_id,t.translated_text,t.provider_label_snapshot,t.target_language,(SELECT corrected_text FROM corrections c WHERE c.translation_id=t.translation_id ORDER BY correction_revision DESC LIMIT 1),(SELECT max(correction_revision) FROM corrections c WHERE c.translation_id=t.translation_id) FROM translations t JOIN request_tasks q USING(task_id) WHERE q.unit_id=?1 AND q.text_revision=?2 ORDER BY t.created_at_ms,t.translation_id").map_err(err)?;
    u.translations = stmt
        .query_map(params![uid, rev], |r| {
            let correction: Option<String> = r.get(5)?;
            Ok(Translation {
                translation_id: r.get(0)?,
                task_id: r.get(1)?,
                text: r.get(2)?,
                provider_label: r.get(3)?,
                target_language: r.get(4)?,
                correction: correction.map(|text| Correction {
                    revision: r.get(6).unwrap_or(1),
                    text,
                }),
            })
        })
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    let mut q=conn.prepare("SELECT t.surface,t.lemma,t.reading_kana,t.pos_major,t.byte_start,t.byte_end,t.is_known FROM lexical_tokens t JOIN lexical_snapshots s USING(lexical_id) WHERE s.lexical_id=(SELECT lexical_id FROM lexical_snapshots WHERE unit_id=?1 AND text_revision=?2 ORDER BY created_at_ms DESC,lexical_id DESC LIMIT 1) ORDER BY t.token_index").map_err(err)?;
    u.tokens = q
        .query_map(params![uid, rev], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u32>(4)?,
                r.get::<_, u32>(5)?,
                r.get::<_, bool>(6)?,
            ))
        })
        .map_err(err)?
        .map(|r| {
            let (surface, lemma, reading, part_of_speech, byte_start, byte_end, known) =
                r.map_err(err)?;
            let (utf16_start, utf16_end) = utf16_span(&u.source_text, byte_start, byte_end)?;
            Ok(LexicalToken {
                surface,
                lemma,
                reading: reading.unwrap_or_default(),
                part_of_speech,
                byte_start,
                byte_end,
                utf16_start,
                utf16_end,
                known,
            })
        })
        .collect::<AppResult<_>>()?;
    Ok(u)
}
pub(super) fn history_page(conn: &Connection, query: HistoryQuery) -> AppResult<HistoryPage> {
    history::page(conn, query)
}
pub(super) fn history_sessions(
    conn: &Connection,
    query: HistorySessionsQuery,
) -> AppResult<HistorySessionsPage> {
    history::sessions(conn, query)
}
pub(super) fn history(
    conn: &Connection,
    game: Option<&str>,
    cursor: Option<&str>,
    limit: u32,
) -> AppResult<HistoryPage> {
    let boundary = if let Some(cursor) = cursor {
        let parsed: (i64, String, u32) = serde_json::from_str(cursor)
            .map_err(|_| AppError::new("invalid_cursor", "history", "历史游标无效"))?;
        parsed
    } else {
        (i64::MAX, "~".into(), u32::MAX)
    };
    let mut stmt=conn.prepare("SELECT r.unit_id,r.text_revision,r.created_at_ms FROM reading_unit_revisions r JOIN sessions s USING(session_id) WHERE (coalesce(json_extract(r.processing_snapshot_json,'$.unitProjection'),'all')<>'latest' OR r.text_revision=(SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=r.unit_id)) AND (?1 IS NULL OR s.game_id=?1) AND (r.created_at_ms,r.unit_id,r.text_revision)<(?2,?3,?4) ORDER BY r.created_at_ms DESC,r.unit_id DESC,r.text_revision DESC LIMIT ?5").map_err(err)?;
    let rows: Vec<(String, u32, i64)> = stmt
        .query_map(
            params![game, boundary.0, boundary.1, boundary.2, limit + 1],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    let has_more = rows.len() > limit as usize;
    let rows = &rows[..rows.len().min(limit as usize)];
    let units = rows
        .iter()
        .map(|(id, rev, _)| unit(conn, id, *rev))
        .collect::<AppResult<Vec<_>>>()?;
    let next_cursor = if has_more {
        rows.last()
            .map(|(id, rev, t)| json(&(*t, id, *rev)))
            .transpose()?
    } else {
        None
    };
    Ok(HistoryPage { units, next_cursor })
}
fn preferences_for_scope(conn: &Connection, scope: &str) -> AppResult<Option<Preferences>> {
    let row=conn.query_row("SELECT p.scope_key,p.theme,p.transparency_mode,p.show_ruby,p.show_pos,p.source_font_pt,p.translation_font_pt,p.hud_width_pt,p.shortcuts_json,p.overrides_json,n.segmentation_enabled,n.reading_display_mode,n.hud_background_opacity FROM preference_revisions p JOIN preference_heads h USING(preference_revision_id) LEFT JOIN reading_preference_settings n USING(preference_revision_id) WHERE h.scope_key=?1",[scope],|r|Ok((Preferences{scope_key:r.get(0)?,theme:r.get(1)?,transparency_mode:r.get(2)?,show_ruby:r.get(3)?,show_pos:r.get(4)?,source_font_pt:r.get(5)?,translation_font_pt:r.get(6)?,hud_width_pt:r.get(7)?,..Preferences::default()},r.get::<_,String>(8)?,r.get::<_,String>(9)?,r.get::<_,Option<bool>>(10)?,r.get::<_,Option<String>>(11)?,r.get::<_,Option<f64>>(12)?))).optional().map_err(err)?;
    row.map(
        |(mut p, shortcuts, overrides, segmentation, mode, opacity)| {
            p.shortcuts = parse(shortcuts)?;
            p.overrides = parse(overrides)?;
            p.segmentation_enabled = segmentation.unwrap_or_else(|| {
                p.overrides
                    .get("reading.segmentationEnabled")
                    .copied()
                    .unwrap_or(true)
            });
            p.reading_display_mode = match mode.as_deref() {
                None | Some("both") => ReadingDisplayMode::Both,
                Some("source") => ReadingDisplayMode::Source,
                Some("translation") => ReadingDisplayMode::Translation,
                _ => {
                    return Err(AppError::new(
                        "invalid_preferences",
                        "storage",
                        "未知阅读显示模式",
                    ));
                }
            };
            p.hud_background_opacity = opacity.unwrap_or(0.70);
            Ok(p)
        },
    )
    .transpose()
}
fn global_preferences(conn: &Connection) -> AppResult<Preferences> {
    Ok(preferences_for_scope(conn, "global")?.unwrap_or_default())
}
fn validate_preference_scope(conn: &Connection, scope: &str) -> AppResult<()> {
    if scope == "global" {
        return Ok(());
    }
    let game = scope
        .strip_prefix("game:")
        .filter(|g| !g.is_empty())
        .ok_or_else(|| AppError::new("invalid_scope", "preferences", "无效偏好范围"))?;
    require_id(game)?;
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM games WHERE game_id=?1 AND archived_at_ms IS NULL)",
            [game],
            |r| r.get(0),
        )
        .map_err(err)?;
    if !exists {
        return Err(AppError::new(
            "game_missing",
            "preferences",
            "所选游戏不存在或已移出游戏库",
        ));
    }
    Ok(())
}
pub(super) fn preferences_scope(conn: &Connection, scope: &str) -> AppResult<ScopedPreferences> {
    validate_preference_scope(conn, scope)?;
    let global = global_preferences(conn)?;
    let saved = preferences_for_scope(conn, scope)?;
    let head: Option<(String, u32)> = conn
        .query_row(
            "SELECT preference_revision_id,head_revision FROM preference_heads WHERE scope_key=?1",
            [scope],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let effective = if scope == "global" {
        global.clone()
    } else {
        merge_preferences(global.clone(), saved.clone())
    };
    let inherited_draft = if scope == "global" {
        None
    } else {
        // Restore is a Rust-derived draft whose visible scalar values already
        // match the inherited source. Older local revisions remain immutable.
        let mut p = global.clone();
        p.scope_key = scope.into();
        p.overrides.clear();
        Some(p)
    };
    let mut overridden_fields = Vec::new();
    if scope != "global"
        && let Some(local) = &saved
    {
        for (key, field) in [
            ("theme", PreferenceField::Theme),
            ("transparencyMode", PreferenceField::TransparencyMode),
            ("showRuby", PreferenceField::ShowRuby),
            ("showPos", PreferenceField::ShowPos),
            ("sourceFontPt", PreferenceField::SourceFontPt),
            ("translationFontPt", PreferenceField::TranslationFontPt),
            ("hudWidthPt", PreferenceField::HudWidthPt),
            ("shortcuts", PreferenceField::Shortcuts),
            ("segmentationEnabled", PreferenceField::SegmentationEnabled),
            ("readingDisplayMode", PreferenceField::ReadingDisplayMode),
            (
                "hudBackgroundOpacity",
                PreferenceField::HudBackgroundOpacity,
            ),
        ] {
            if local.overrides.get(key) == Some(&true)
                || (field == PreferenceField::SegmentationEnabled
                    && local.overrides.contains_key("reading.segmentationEnabled"))
            {
                overridden_fields.push(field);
            }
        }
        if local.overrides.contains_key("hud.compact") {
            overridden_fields.push(PreferenceField::HudCompact);
        }
        if local.overrides.contains_key("hud.alwaysOnTop") {
            overridden_fields.push(PreferenceField::HudAlwaysOnTop);
        }
    }
    Ok(ScopedPreferences {
        scope_key: scope.into(),
        saved,
        effective,
        global,
        head_revision: head.as_ref().map(|h| h.1),
        preference_revision_id: head.map(|h| h.0),
        overridden_fields,
        inherited_draft,
    })
}
fn effective_preferences(conn: &Connection) -> AppResult<Preferences> {
    effective_preferences_with_patch(conn, None)
}
pub(super) fn preview_preferences(
    conn: &Connection,
    preferences: &Preferences,
) -> AppResult<Preferences> {
    validate_preferences(preferences)?;
    effective_preferences_with_patch(conn, Some(preferences))
}
fn effective_preferences_with_patch(
    conn: &Connection,
    patch: Option<&Preferences>,
) -> AppResult<Preferences> {
    let global = if let Some(p) = patch.filter(|p| p.scope_key == "global") {
        p.clone()
    } else {
        global_preferences(conn)?
    };
    let active:Option<String>=conn.query_row("SELECT game_id FROM sessions WHERE source_kind='hook' AND state IN ('opening','active') ORDER BY started_at_ms DESC,session_id DESC LIMIT 1",[],|r|r.get(0)).optional().map_err(err)?.flatten();
    let game: Option<String> = if active.is_some() {
        active
    } else {
        conn.query_row("SELECT s.game_id FROM reading_unit_revisions r JOIN sessions s USING(session_id) ORDER BY r.created_at_ms DESC,r.unit_id DESC,r.text_revision DESC LIMIT 1",[],|r|r.get(0)).optional().map_err(err)?.flatten()
    };
    let Some(game) = game else { return Ok(global) };
    let scope = format!("game:{game}");
    let local = if let Some(p) = patch.filter(|p| p.scope_key == scope) {
        Some(p.clone())
    } else {
        preferences_for_scope(conn, &scope)?
    };
    Ok(merge_preferences(global, local))
}
fn merge_preferences(global: Preferences, local: Option<Preferences>) -> Preferences {
    let Some(local) = local else { return global };
    let mut effective = global.clone();
    effective.scope_key = local.scope_key.clone();
    macro_rules! merge {
        ($field:ident,$key:literal) => {
            if local.overrides.get($key) == Some(&true) {
                effective.$field = local.$field.clone();
            }
        };
    }
    merge!(theme, "theme");
    merge!(transparency_mode, "transparencyMode");
    merge!(show_ruby, "showRuby");
    merge!(show_pos, "showPos");
    merge!(source_font_pt, "sourceFontPt");
    merge!(translation_font_pt, "translationFontPt");
    merge!(hud_width_pt, "hudWidthPt");
    merge!(segmentation_enabled, "segmentationEnabled");
    merge!(reading_display_mode, "readingDisplayMode");
    merge!(hud_background_opacity, "hudBackgroundOpacity");
    merge!(shortcuts, "shortcuts");
    if local.overrides.contains_key("reading.segmentationEnabled") {
        effective.segmentation_enabled = local.segmentation_enabled;
    }
    effective.overrides.extend(local.overrides);
    effective
}
fn save_preferences(
    tx: &Transaction<'_>,
    p: &Preferences,
    operation: &str,
    now: i64,
) -> AppResult<String> {
    validate_preferences(p)?;
    let pid = id("preference", operation);
    let game = p.scope_key.strip_prefix("game:");
    let previous: Option<String> = tx
        .query_row(
            "SELECT preference_revision_id FROM preference_heads WHERE scope_key=?1",
            [&p.scope_key],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    tx.execute(
        "INSERT INTO preference_revisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            pid,
            p.scope_key,
            game,
            previous,
            p.theme,
            p.transparency_mode,
            p.show_ruby,
            p.show_pos,
            p.source_font_pt,
            p.translation_font_pt,
            p.hud_width_pt,
            json(&p.shortcuts)?,
            json(&p.overrides)?,
            now
        ],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT INTO reading_preference_settings VALUES(?1,?2,?3,?4)",
        params![
            pid,
            p.segmentation_enabled,
            match p.reading_display_mode {
                ReadingDisplayMode::Both => "both",
                ReadingDisplayMode::Source => "source",
                ReadingDisplayMode::Translation => "translation",
            },
            p.hud_background_opacity
        ],
    )
    .map_err(err)?;
    tx.execute("INSERT INTO preference_heads VALUES(?1,?2,1,?3) ON CONFLICT(scope_key) DO UPDATE SET preference_revision_id=excluded.preference_revision_id,head_revision=preference_heads.head_revision+1,updated_at_ms=excluded.updated_at_ms",params![p.scope_key,pid,now]).map_err(err)?;
    Ok(pid)
}
pub(super) fn snapshot(db: &mut Database) -> AppResult<AppSnapshot> {
    let conn = &db.conn;
    let history = history(conn, None, None, 50)?;
    let preferences = effective_preferences(conn)?;
    let session=conn.query_row("SELECT session_id,game_id,source_kind,state FROM sessions WHERE source_kind='hook' ORDER BY started_at_ms DESC,session_id DESC LIMIT 1",[],|r|Ok(Session{session_id:r.get(0)?,game_id:r.get(1)?,source_kind:r.get(2)?,state:r.get(3)?,paused:false,translation_enabled:true,sources:vec![]})).optional().map_err(err)?;
    let mut session = session;
    if let Some(s) = &mut session {
        let mut q=conn.prepare("SELECT c.source_id,c.hook_name,c.hook_code,c.encoding,EXISTS(SELECT 1 FROM current_source_selection_members m WHERE m.session_id=c.session_id AND m.source_id=c.source_id),coalesce((SELECT CAST(p.payload_bytes AS TEXT) FROM raw_events r JOIN raw_event_payloads p USING(event_id) WHERE r.source_id=c.source_id ORDER BY r.ingest_sequence DESC LIMIT 1),'') FROM capture_sources c WHERE c.session_id=?1 ORDER BY c.discovered_at_ms,c.source_id").map_err(err)?;
        s.sources = q
            .query_map([&s.session_id], |r| {
                Ok(CaptureSource {
                    source_id: r.get(0)?,
                    hook_name: r.get(1)?,
                    hook_code: r.get(2)?,
                    encoding: r.get(3)?,
                    sample: r.get(5)?,
                    selected: r.get(4)?,
                })
            })
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
    }
    // The live reading object belongs to the running capture session. Global
    // history also contains previous games and independent manual input, and
    // its first page may not contain this session's latest visible revision.
    // Keep the existing global-last-sentence restoration when capture ends.
    let current = if let Some(s) = session
        .as_ref()
        .filter(|s| matches!(s.state.as_str(), "opening" | "active"))
    {
        history::page(
            conn,
            HistoryQuery {
                filter: HistoryFilter {
                    session_id: Some(s.session_id.clone()),
                    ..Default::default()
                },
                cursor: None,
                limit: 1,
            },
        )?
        .units
        .into_iter()
        .next()
    } else {
        history.units.first().cloned()
    };
    let generation: u32 = conn
        .query_row(
            "SELECT runtime_generation FROM profile_meta WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    Ok(AppSnapshot {
        profile_id: db.profile_id.clone(),
        server_instance_id: db.server_id.clone(),
        profile_generation: generation,
        revision: revision(conn)?,
        commit_revision: revision(conn)?,
        games: games(conn)?,
        session,
        current,
        history,
        configuration: configuration::state(conn)?,
        preferences,
        diagnostics: db.diagnostics.clone(),
        tasks: task_views(conn)?,
        study_results: study_results(conn)?,
        learning_object: None,
        lexical_states: vec![],
        credentials: credentials::metadata(conn)?,
        credential_pools: credentials::pools(conn)?,
        configuration_tests: model_tests::document_views(conn, &configuration::state(conn)?.draft)?,
    })
}

// Shared raw task view: history/receipts remain independent of current reading.
pub(super) const TASK_VIEW_COLUMNS: &str = "q.task_id,q.unit_id,q.text_revision,q.purpose,q.state,q.invalidation_code,CASE WHEN e.target_kind='http' THEN c.display_name WHEN e.target_kind='offline' THEN oc.display_name END,(SELECT a.attempt_id FROM request_attempts a WHERE a.task_id=q.task_id ORDER BY a.attempt_number DESC LIMIT 1)";
pub(super) const TASK_VIEW_JOINS: &str = "FROM request_tasks q JOIN binding_engines e ON e.revision_id=q.config_revision_id AND e.binding_id=q.binding_id AND e.engine_id=q.engine_id LEFT JOIN models m ON e.target_kind='http' AND m.revision_id=e.revision_id AND m.model_id=e.model_id LEFT JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id LEFT JOIN offline_engine_configs oc ON e.target_kind='offline' AND oc.revision_id=e.revision_id AND oc.engine_config_id=e.offline_engine_config_id";
pub(super) fn task_view_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskView> {
    Ok(TaskView {
        task_id: row.get(0)?,
        unit_id: row.get(1)?,
        text_revision: row.get(2)?,
        purpose: row.get(3)?,
        status: row.get(4)?,
        error_code: row.get(5)?,
        partial_text: None,
        provider_label: row.get(6)?,
        attempt_id: row.get(7)?,
    })
}
fn task_views(conn: &Connection) -> AppResult<Vec<TaskView>> {
    // Preserve the original activity/100-terminal history inventory and order.
    // Reading state is queried separately for its exact object, never here.
    let sql = format!(
        "SELECT {TASK_VIEW_COLUMNS} {TASK_VIEW_JOINS} WHERE q.state IN ('queued','running') OR q.task_id IN (SELECT task_id FROM request_tasks WHERE state NOT IN ('queued','running') ORDER BY created_at_ms DESC,task_id DESC LIMIT 100) ORDER BY q.created_at_ms DESC,q.task_id DESC"
    );
    let mut statement = conn.prepare(&sql).map_err(err)?;
    statement
        .query_map([], task_view_row)
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)
}

fn study_results(conn: &Connection) -> AppResult<Vec<StudyResult>> {
    let mut q=conn.prepare("SELECT q.task_id,q.unit_id,q.text_revision,q.purpose,s.output_mode,s.explanation_text,s.structured_json,s.selection_byte_start,s.selection_byte_end FROM study_results s JOIN request_tasks q USING(task_id) ORDER BY s.created_at_ms DESC LIMIT 100").map_err(err)?;
    q.query_map([], |r| {
        Ok((
            StudyResult {
                task_id: r.get(0)?,
                unit_id: r.get(1)?,
                text_revision: r.get(2)?,
                purpose: r.get(3)?,
                mode: r.get(4)?,
                text: r.get(5)?,
                references: vec![],
                selection_byte_start: r.get(7)?,
                selection_byte_end: r.get(8)?,
            },
            r.get::<_, String>(6)?,
        ))
    })
    .map_err(err)?
    .map(|row| {
        let (mut s, v) = row.map_err(err)?;
        let v: serde_json::Value = parse(v)?;
        if let Some(refs) = v.get("references") {
            s.references = serde_json::from_value(refs.clone()).map_err(err)?;
        }
        Ok(s)
    })
    .collect()
}
