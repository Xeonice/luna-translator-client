//! Immutable selection sets share the original per-source lineage authority.
use super::*;
use app_core::ports::runtime::SourceSelectionCommit;

pub(in crate::storage) fn profile(conn: &Connection) -> AppResult<BindingProfileContext> {
    conn.query_row("SELECT profile_id,server_instance_id,runtime_generation FROM profile_meta WHERE singleton=1 AND runtime_state='active'", [], |row| Ok(BindingProfileContext {
        profile_id: row.get(0)?, server_instance_id: row.get(1)?, profile_generation: row.get(2)?,
    })).map_err(err)
}

fn require_profile(conn: &Connection, expected: &BindingProfileContext) -> AppResult<()> {
    if &profile(conn)? != expected {
        return Err(AppError::new("stale_binding_profile", "capture", "配置档或服务运行代次已变化，请重新核对"));
    }
    Ok(())
}

fn require_active(conn: &Connection, session: &str) -> AppResult<()> {
    require_id(session)?;
    let valid: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id=?1 AND source_kind='hook' AND state IN ('opening','active')) AND NOT EXISTS(SELECT 1 FROM sessions WHERE source_kind='hook' AND state IN ('opening','active') AND session_id<>?1)", [session], |row| row.get(0)).map_err(err)?;
    if !valid {
        return Err(AppError::new("stale_capture_session", "capture", "当前取文会话已变化，请重新核对正文来源"));
    }
    Ok(())
}

fn persistence(value: &str) -> AppResult<SelectionPersistence> {
    match value {
        "temporary" => Ok(SelectionPersistence::Temporary),
        "saved_snapshot" => Ok(SelectionPersistence::SavedSnapshot),
        _ => Err(AppError::new("selection_set_corrupt", "storage", "未知的正文保存状态")),
    }
}

fn set(conn: &Connection, session: &str, revision: i64) -> AppResult<SourceSelectionSet> {
    let (operation_id, kind, selected_at): (Option<String>, String, i64) = conn.query_row("SELECT operation_id,persistence_kind,selected_at_ms FROM source_selection_sets WHERE session_id=?1 AND selection_set_revision=?2 AND state='sealed'", params![session,revision], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(err)?;
    let mut statement = conn.prepare("SELECT source_id,selection_revision,member_ordinal FROM source_selection_members WHERE session_id=?1 AND selection_set_revision=?2 ORDER BY member_ordinal").map_err(err)?;
    let members = statement.query_map(params![session,revision], |row| Ok(SourceSelectionMember { source_id: row.get(0)?, selection_revision: row.get(1)?, ordinal: row.get(2)? })).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    Ok(SourceSelectionSet { session_id: session.into(), set_revision: revision.to_string(), operation_id,
        persistence: persistence(&kind)?, selected_at_ms: selected_at.to_string(), members })
}

fn head(conn: &Connection, session: &str) -> AppResult<Option<i64>> {
    conn.query_row("SELECT max(selection_set_revision) FROM source_selection_sets WHERE session_id=?1 AND state='sealed'", [session], |row| row.get(0)).map_err(err)
}

fn details(conn: &Connection, session: &str) -> AppResult<Vec<SourceDetailView>> {
    let mut statement = conn.prepare("SELECT c.source_id,c.hook_name,c.hook_code,c.hook_address_u64,c.context_u64,c.subcontext_u64,c.encoding,c.stream_identity_sha256,c.identity_extensions_json,coalesce((SELECT CAST(p.payload_bytes AS TEXT) FROM raw_events e JOIN raw_event_payloads p USING(event_id) WHERE e.source_id=c.source_id ORDER BY e.ingest_sequence DESC LIMIT 1),'') FROM capture_sources c WHERE c.session_id=?1 ORDER BY c.discovered_at_ms,c.source_id").map_err(err)?;
    let values = statement.query_map([session], |row| Ok(SourceDetailView { source_id: row.get(0)?, hook_name: row.get(1)?, hook_code: row.get(2)?, address: Some(row.get(3)?), context: Some(row.get(4)?), context2: Some(row.get(5)?), encoding: row.get(6)?, stream_identity_sha256: row.get(7)?, identity_extensions_json: row.get(8)?, latest_sample: row.get(9)? })).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    for source in &values {
        for value in [&source.address, &source.context, &source.context2].into_iter().flatten() { canonical_u64(value)?; }
    }
    Ok(values)
}

pub(in crate::storage) fn source_selection(db: &mut Database, query: SourceSelectionQuery) -> AppResult<SourceSelectionProjection> {
    let frame = profile(&db.conn)?;
    if frame.profile_id != db.profile_id || frame.server_instance_id != db.server_id || frame.profile_generation != query.expected_profile_generation {
        return Err(AppError::new("stale_binding_profile", "capture", "配置档运行代次已变化"));
    }
    require_active(&db.conn, &query.session_id)?;
    let current = head(&db.conn, &query.session_id)?.map(|revision| set(&db.conn, &query.session_id, revision)).transpose()?;
    let (game, environment, path, architecture): (Option<String>, String, String, String) = db.conn.query_row("SELECT game_id,environment_key,target_exe_path,architecture FROM sessions WHERE session_id=?1", [&query.session_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).map_err(err)?;
    let mut saved = None;
    if let Some(game_id) = &game {
        let (_, path_key) = normalize_windows_path(&path)?;
        let mut statement = db.conn.prepare("SELECT s.session_id,s.selection_set_revision,s.match_snapshot_json,h.target_exe_path FROM source_selection_sets s JOIN sessions h ON h.session_id=s.session_id WHERE h.game_id=?1 AND h.environment_key=?2 AND h.architecture=?3 AND s.persistence_kind='saved_snapshot' AND s.state='sealed' ORDER BY s.selected_at_ms DESC,s.session_id,s.selection_set_revision DESC").map_err(err)?;
        let candidates = statement.query_map(params![game_id,environment,architecture], |row| Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        for (session, revision, snapshot, old_path) in candidates {
            if normalize_windows_path(&old_path).is_ok_and(|(_, key)| key == path_key) {
                let selection = set(&db.conn, &session, revision)?;
                let ids = selection.members.iter().map(|member| &member.source_id).collect::<std::collections::BTreeSet<_>>();
                let sources = details(&db.conn, &session)?.into_iter().filter(|source| ids.contains(&source.source_id)).collect();
                let matcher_version = serde_json::from_str::<serde_json::Value>(&snapshot).ok().and_then(|value| value.get("schemaVersion").and_then(serde_json::Value::as_u64)).and_then(|value| u32::try_from(value).ok());
                saved = Some(SavedSourceSelection { set: selection, matcher_version, state: SourceSelectionState::RequiresReview,
                    reason: "上次来源只有会话身份和绝对地址，尚无可移植匹配证明，请重新核对后开始阅读".into(), sources, match_snapshot_json: snapshot });
                break;
            }
        }
    }
    let state = if current.is_some() { SourceSelectionState::Current } else if saved.is_some() { SourceSelectionState::RequiresReview } else { SourceSelectionState::Unselected };
    Ok(SourceSelectionProjection { profile: frame, session_id: query.session_id.clone(), state, current,
        sources: details(&db.conn, &query.session_id)?, saved, remember_allowed: game.is_some(), member_limit: SOURCE_SELECTION_MEMBER_LIMIT })
}

fn requested_head(value: &Option<String>) -> AppResult<Option<i64>> {
    value.as_ref().map(|value| {
        let revision = canonical_binding_decimal(value)?;
        if revision == 0 { return Err(AppError::new("invalid_selection_revision", "capture", "空集合头必须明确使用null")); }
        i64::try_from(revision).map_err(|_| AppError::new("invalid_selection_revision", "capture", "集合版本超出有效整数范围"))
    }).transpose()
}

fn validate_snapshot(conn: &Connection, input: &SourceSelectionCommit) -> AppResult<()> {
    let actual = details(conn, &input.request.session_id)?.into_iter().map(|source| (source.source_id.clone(), source)).collect::<BTreeMap<_,_>>();
    let selected = input.request.source_ids.iter().map(|source| actual.get(source).cloned().ok_or_else(|| AppError::new("source_session_mismatch", "capture", "正文来源不属于当前会话"))).collect::<AppResult<Vec<_>>>()?;
    app_core::application::binding_journey::validate_selection_match_snapshot(
        &input.match_snapshot_json, input.request.remember, &input.active_session.target_identity_sha256, &selected,
    )
}

pub(in crate::storage) fn commit(db: &mut Database, input: SourceSelectionCommit, now: i64) -> AppResult<SelectionCommitReceipt> {
    require_id(&input.request.operation_id)?;
    let hash = canonical_hash(&input.request)?;
    if hash != input.request_sha256 { return Err(AppError::new("selection_request_hash_mismatch", "capture", "正文选择请求摘要不一致")); }
    let tx = db.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
    let previous: Option<(String,String,String)> = tx.query_row("SELECT request_sha256,command_kind,result_refs_json FROM operation_receipts WHERE operation_id=?1", [&input.request.operation_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(err)?;
    if let Some((previous_hash, kind, result)) = previous {
        if previous_hash != hash || kind != "begin_reading" { return Err(AppError::new("operation_conflict", "capture", "同一操作身份不能用于不同请求")); }
        let mut receipt: SelectionCommitReceipt = parse(result)?;
        receipt.replayed = true;
        return Ok(receipt);
    }
    if input.request.source_ids.is_empty() || input.request.source_ids.len() > SOURCE_SELECTION_MEMBER_LIMIT as usize {
        return Err(AppError::new("invalid_source_selection", "capture", "请选择1～64个正文来源"));
    }
    require_profile(&tx, &input.profile)?;
    if input.profile.profile_generation != input.request.expected_profile_generation || input.profile.profile_id != db.profile_id || input.profile.server_instance_id != db.server_id || input.active_session.session_id != input.request.session_id {
        return Err(AppError::new("stale_binding_profile", "capture", "正文提交不属于当前配置档或取文会话"));
    }
    require_active(&tx, &input.request.session_id)?;
    let witness = &input.active_session;
    let exact: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id=?1 AND game_id IS ?2 AND environment_key=?3 AND target_exe_path=?4 AND windows_pid=?5 AND process_created_u64=?6 AND architecture=?7 AND broker_instance_id=?8 AND core_resource_snapshot_id=?9 AND target_identity_sha256=?10)", params![witness.session_id,witness.game_id,witness.environment_key,witness.target_exe_path,witness.windows_pid,witness.process_created_u64,witness.architecture,witness.broker_instance_id,witness.resource.snapshot_id,witness.target_identity_sha256], |row| row.get(0)).map_err(err)?;
    if !exact { return Err(AppError::new("stale_capture_target", "capture", "取文目标身份已变化，不能提交旧选择")); }
    if head(&tx, &input.request.session_id)? != requested_head(&input.request.expected_set_revision)? {
        return Err(AppError::new("selection_head_conflict", "capture", "正文选择已被其他窗口更新，请保留草稿并重新核对"));
    }
    // The typed saved explanation is checked below against the actual source
    // rows; the Core-provided word "validated" never bypasses this writer.
    validate_snapshot(&tx, &input)?;
    let kind = if input.request.remember { "saved_snapshot" } else { "temporary" };
    let committed = insert_set(&tx, &input.request.session_id, &input.request.operation_id, &input.request.source_ids, kind, &input.match_snapshot_json, now)?;
    let selection = set(&tx, &input.request.session_id, committed.revision)?;
    debug_assert_eq!(selection.members.len(), committed.members.len());
    let effect_authority = ReadingWindowEffectAuthority { profile: input.profile.clone(), session_id: input.request.session_id.clone(), set_revision: selection.set_revision.clone(), operation_id: input.request.operation_id.clone() };
    let receipt = SelectionCommitReceipt { operation_id: input.request.operation_id.clone(), profile: input.profile, selection, replayed: false, effect_authority };
    let revision = bump(&tx, now)?;
    tx.execute("INSERT INTO operation_receipts VALUES(?1,?2,'begin_reading',?3,?4,?5)", params![input.request.operation_id,hash,revision,json(&receipt)?,now]).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(receipt)
}

pub(in crate::storage) fn validate_effect(db: &mut Database, authority: ReadingWindowEffectAuthority) -> AppResult<SelectionCommitReceipt> {
    require_profile(&db.conn, &authority.profile)?;
    require_active(&db.conn, &authority.session_id)?;
    if head(&db.conn, &authority.session_id)? != requested_head(&Some(authority.set_revision.clone()))? {
        return Err(AppError::new("stale_reading_window_effect", "capture", "窗口操作所属正文集合已变化"));
    }
    let result: Option<String> = db.conn.query_row("SELECT o.result_refs_json FROM operation_receipts o JOIN source_selection_sets s ON s.operation_id=o.operation_id WHERE o.operation_id=?1 AND o.command_kind='begin_reading' AND s.session_id=?2 AND CAST(s.selection_set_revision AS TEXT)=?3 AND s.state='sealed'", params![authority.operation_id,authority.session_id,authority.set_revision], |row| row.get(0)).optional().map_err(err)?;
    let receipt: SelectionCommitReceipt = parse(result.ok_or_else(|| AppError::new("stale_reading_window_effect", "capture", "窗口操作缺少对应的成功持久化回执"))?)?;
    if receipt.effect_authority != authority { return Err(AppError::new("stale_reading_window_effect", "capture", "窗口操作身份与成功回执不一致")); }
    Ok(receipt)
}

pub(super) struct CommittedSet {
    pub revision: i64,
    pub members: Vec<(String, u32)>,
}

/// Both the new explicit begin-reading operation and legacy SelectSource use
/// this transaction body. Its caller writes the one operation receipt before
/// COMMIT, satisfying the deferred header FK atomically.
pub(super) fn insert_set(
    tx: &Transaction<'_>,
    session: &str,
    operation: &str,
    sources: &[String],
    persistence: &str,
    match_snapshot: &str,
    now: i64,
) -> AppResult<CommittedSet> {
    require_id(session)?;
    require_id(operation)?;
    if now < 0 || sources.is_empty() || sources.len() > SOURCE_SELECTION_MEMBER_LIMIT as usize || !matches!(persistence, "temporary" | "saved_snapshot") {
        return Err(AppError::new("invalid_source_selection", "capture", "请选择1～64个正文来源并保留明确的保存意图"));
    }
    let mut unique = std::collections::BTreeSet::new();
    for source in sources {
        require_id(source)?;
        if !unique.insert(source) {
            return Err(AppError::new("duplicate_source_selection", "capture", "同一正文来源不能重复提交"));
        }
    }
    let state: Option<(String, String, Option<String>)> = tx.query_row(
        "SELECT source_kind,state,game_id FROM sessions WHERE session_id=?1", [session],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(err)?;
    let Some((kind, state, game)) = state else {
        return Err(AppError::new("stale_capture_session", "capture", "取文会话不存在，请重新核对"));
    };
    require_active(tx, session)?;
    if kind != "hook" || !matches!(state.as_str(), "opening" | "active") {
        return Err(AppError::new("stale_capture_session", "capture", "此取文会话已停止，请重新核对"));
    }
    if persistence == "saved_snapshot" && game.is_none() {
        return Err(AppError::new("remember_requires_game", "capture", "仅本次接入没有游戏归属，不能记住选择"));
    }
    let snapshot: serde_json::Value = serde_json::from_str(match_snapshot).map_err(err)?;
    if !snapshot.is_object() || match_snapshot.contains('\0') {
        return Err(AppError::new("invalid_match_snapshot", "capture", "保存说明必须是合法的版本化对象"));
    }
    for source in sources {
        let owned: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM capture_sources WHERE source_id=?1 AND session_id=?2)", params![source,session], |row| row.get(0)).map_err(err)?;
        if !owned {
            return Err(AppError::new("source_session_mismatch", "capture", "正文来源不属于当前会话"));
        }
    }
    let head: i64 = tx.query_row("SELECT coalesce(max(selection_set_revision),0) FROM source_selection_sets WHERE session_id=?1", [session], |row| row.get(0)).map_err(err)?;
    let revision = head.checked_add(1).ok_or_else(|| AppError::new("selection_revision_exhausted", "capture", "正文集合版本已耗尽"))?;
    let lineage: i64 = tx.query_row("SELECT coalesce(max(selection_revision),0) FROM source_selections WHERE session_id=?1", [session], |row| row.get(0)).map_err(err)?;
    let lineage = u32::try_from(lineage).map_err(|_| AppError::new("selection_revision_exhausted", "capture", "正文来源版本超出有效范围"))?;
    lineage.checked_add(u32::try_from(sources.len()).map_err(err)?).ok_or_else(|| AppError::new("selection_revision_exhausted", "capture", "正文来源版本已耗尽"))?;
    capture::seal_capture_units(tx, Some(session), now, false)?;
    tx.execute("INSERT INTO source_selection_sets VALUES(?1,?2,?3,'explicit_set',?4,?5,?6,?7,'building')", params![session,revision,operation,persistence,match_snapshot,now,sources.len() as i64]).map_err(err)?;
    let mut members = Vec::with_capacity(sources.len());
    for (ordinal, source) in sources.iter().enumerate() {
        let member_revision = lineage + u32::try_from(ordinal + 1).map_err(err)?;
        tx.execute("INSERT INTO source_selections VALUES(?1,?2,?3,?4,?5,?6)", params![session,member_revision,source,persistence,match_snapshot,now]).map_err(err)?;
        tx.execute("INSERT INTO source_selection_members VALUES(?1,?2,?3,?4,?5)", params![session,revision,source,member_revision,ordinal as i64]).map_err(err)?;
        members.push((source.clone(), member_revision));
    }
    tx.execute("UPDATE source_selection_sets SET state='sealed' WHERE session_id=?1 AND selection_set_revision=?2", params![session,revision]).map_err(err)?;
    Ok(CommittedSet { revision, members })
}

/// Reject a business-unrepresentable old profile before any writable open or
/// backup. SQL's INTEGER carrier is wider than the retained u32 lineage API.
pub(in crate::storage) fn validate_database(conn: &Connection, version: i64) -> AppResult<()> {
    let invalid: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM source_selections WHERE selection_revision<1 OR selection_revision>4294967295 OR length(session_id)=0 OR instr(session_id,char(0))<>0 OR length(source_id)=0 OR instr(source_id,char(0))<>0)",
        [], |row| row.get(0),
    ).map_err(err)?;
    if invalid {
        return Err(AppError::new("selection_lineage_corrupt", "storage", "正文选择血缘超出已评审类型范围；原配置档未修改"));
    }
    let exhausted: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profile_meta WHERE runtime_generation>=4294967295)", [], |row| row.get(0),
    ).map_err(err)?;
    if exhausted {
        return Err(AppError::new("runtime_generation_exhausted", "storage", "运行代次已耗尽；原配置档未修改"));
    }
    if version >= 5 {
        let invalid: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM source_selection_sets s WHERE s.state<>'sealed' OR s.member_count<>(SELECT count(*) FROM source_selection_members m WHERE m.session_id=s.session_id AND m.selection_set_revision=s.selection_set_revision) OR 0<>(SELECT min(member_ordinal) FROM source_selection_members m WHERE m.session_id=s.session_id AND m.selection_set_revision=s.selection_set_revision) OR s.member_count-1<>(SELECT max(member_ordinal) FROM source_selection_members m WHERE m.session_id=s.session_id AND m.selection_set_revision=s.selection_set_revision)) OR EXISTS(SELECT 1 FROM source_selection_members m JOIN source_selections r ON r.session_id=m.session_id AND r.selection_revision=m.selection_revision JOIN source_selection_sets s ON s.session_id=m.session_id AND s.selection_set_revision=m.selection_set_revision WHERE m.source_id<>r.source_id OR s.persistence_kind<>r.persistence_kind) OR EXISTS(SELECT 1 FROM source_selections r WHERE NOT EXISTS(SELECT 1 FROM source_selection_members m WHERE m.session_id=r.session_id AND m.selection_revision=r.selection_revision))",
            [], |row| row.get(0),
        ).map_err(err)?;
        if invalid {
            return Err(AppError::new("selection_set_corrupt", "storage", "正文集合、成员与原血缘不完整；原配置档未修改"));
        }
    }
    Ok(())
}
