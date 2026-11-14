use super::*;
use app_core::domain::capture::*;
struct StoredHookSession {
    target: String,
    broker: String,
    core: String,
    state: String,
    game_id: Option<String>,
    environment: String,
    path: String,
    pid: u32,
    created: String,
    architecture: String,
}
fn resource(tx: &Transaction, r: &ResourceSnapshotInput, now: i64) -> AppResult<()> {
    tx.execute("INSERT INTO resource_snapshots VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(resource_snapshot_id) DO NOTHING",params![r.snapshot_id,r.kind,r.version,r.manifest_sha256,r.contract_version,r.metadata_json,now]).map_err(err)?;
    let existing: (String, String, String, String, String) = tx
        .query_row(
            "SELECT kind,version_label,manifest_sha256,contract_version,metadata_json FROM resource_snapshots WHERE resource_snapshot_id=?1",
            [&r.snapshot_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(err)?;
    if existing.0 != r.kind
        || existing.1 != r.version
        || existing.2 != r.manifest_sha256
        || existing.3 != r.contract_version
        || parse::<serde_json::Value>(existing.4)?
            != parse::<serde_json::Value>(r.metadata_json.clone())?
    {
        return Err(AppError::new(
            "resource_identity_conflict",
            "resources",
            "资源身份被不同内容复用",
        ));
    }
    Ok(())
}
pub(in crate::storage) fn open_hook_session(
    db: &mut Database,
    i: HookSessionInput,
    now: i64,
) -> AppResult<()> {
    canonical_u64(&i.process_created_u64)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let existing:Option<StoredHookSession>=tx.query_row("SELECT target_identity_sha256,broker_instance_id,core_resource_snapshot_id,state,game_id,environment_key,target_exe_path,windows_pid,process_created_u64,architecture FROM sessions WHERE session_id=?1",[&i.session_id],|r|Ok(StoredHookSession { target:r.get(0)?,broker:r.get(1)?,core:r.get(2)?,state:r.get(3)?,game_id:r.get(4)?,environment:r.get(5)?,path:r.get(6)?,pid:r.get(7)?,created:r.get(8)?,architecture:r.get(9)? })).optional().map_err(err)?;
    let mut supplement_attribution = false;
    if let Some(StoredHookSession { target, broker, core, state, game_id, environment, path, pid, created, architecture }) = existing {
        if target == i.target_identity_sha256
            && broker == i.broker_instance_id
            && core == i.resource.snapshot_id
            && matches!(state.as_str(), "opening" | "active")
            && environment == i.environment_key && path == i.target_exe_path
            && pid == i.windows_pid && created == i.process_created_u64 && architecture == i.architecture
        {
            // An idempotent session retry still declares the immutable Core
            // resource identity; the early return cannot bypass that check.
            resource(&tx, &i.resource, now)?;
            if game_id.is_some() && i.game_id.is_some() && game_id != i.game_id {
                return Err(AppError::new("session_game_conflict", "capture", "当前会话已有不同的游戏库归属，不能改写"));
            }
            if game_id.is_some() || i.game_id.is_none() { return Ok(()); }
            supplement_attribution = true;
        } else {
            return Err(AppError::new(
                "session_identity_conflict",
                "capture",
                "会话身份不能被新目标或已结束进程复用",
            ));
        }
    }
    if let Some(game_id) = &i.game_id {
        let game = super::games(&tx)?
            .into_iter()
            .find(|g| &g.game_id == game_id)
            .ok_or_else(|| {
                AppError::new(
                    "game_not_active",
                    "capture",
                    "游戏已移出或不存在，请重新选择关联对象",
                )
            })?;
        let mut input = GameInput {
            game_id: Some(game.game_id.clone()),
            row_revision: Some(game.row_revision),
            display_name: game.display_name,
            original_title: game.original_title,
            source_language: game.source_language,
            cover_asset_ref: game.cover_asset_ref,
            target: game.target,
        };
        let declared_path = input.target.canonical_exe_path.as_deref().map(app_core::domain::normalize_windows_path).transpose()?.map(|(_, key)| key);
        normalize_game_input(&mut input)?;
        let saved_target = input.target.clone();
        input.target.environment_kind = "crossover".into();
        input.target.environment_key = i.environment_key.clone();
        input.target.bottle_locator = i
            .environment_key
            .strip_prefix("crossover:")
            .map(str::to_owned);
        input.target.canonical_exe_path = Some(i.target_exe_path.clone());
        input.target.architecture = Some(i.architecture.clone());
        input.target.verification_state = "verified".into();
        normalize_game_input(&mut input)?;
        validate_game(&input)?;
        if saved_target.environment_kind == "crossover" && saved_target.environment_key != input.target.environment_key
            || declared_path.is_some() && declared_path != input.target.target_path_key
            || declared_path.is_some() && saved_target.architecture.is_some() && saved_target.architecture != input.target.architecture
        { return Err(AppError::new("game_target_conflict", "capture", "游戏资料与接入目标的环境、完整路径或架构不一致")); }
        // Connecting an already verified target does not edit the game library.
        // Only newly observed verification facts warrant a revision update.
        if saved_target.environment_kind != input.target.environment_kind
            || saved_target.environment_key != input.target.environment_key
            || saved_target.bottle_locator != input.target.bottle_locator
            || saved_target.canonical_exe_path != input.target.canonical_exe_path
            || saved_target.target_path_key != input.target.target_path_key
            || saved_target.architecture != input.target.architecture
            || saved_target.verification_state != input.target.verification_state
        {
        tx.execute("UPDATE game_targets SET environment_kind='crossover',environment_key=?2,bottle_locator=?3,canonical_exe_path=?4,target_path_key=?5,architecture=?6,verification_state='verified',row_revision=row_revision+1,updated_at_ms=?7 WHERE game_id=?1",params![game_id,input.target.environment_key,input.target.bottle_locator,input.target.canonical_exe_path,input.target.target_path_key,input.target.architecture,now]).map_err(err)?;
        tx.execute(
            "UPDATE games SET row_revision=row_revision+1,updated_at_ms=?2 WHERE game_id=?1",
            params![game_id, now],
        )
        .map_err(err)?;
        }
    }
    if supplement_attribution {
        // Only the current session's previously empty attribution changes. Existing
        // text units, language snapshots, source choice and native identity stay immutable.
        let changed = tx.execute("UPDATE sessions SET game_id=?2 WHERE session_id=?1 AND game_id IS NULL AND state IN ('opening','active')", params![i.session_id, i.game_id]).map_err(err)?;
        if changed != 1 { return Err(AppError::new("session_identity_conflict", "capture", "会话归属已变化，请重新核对")); }
        bump(&tx, now)?;
        return tx.commit().map_err(err);
    }
    resource(&tx, &i.resource, now)?;
    seal_capture_units(&tx, None, now, false)?;
    tx.execute("UPDATE sessions SET state='stopped',ended_at_ms=?1 WHERE source_kind='hook' AND state IN ('opening','active')",[now]).map_err(err)?;
    tx.execute(
        "INSERT INTO sessions VALUES(?1,?2,'hook','opening',?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL)",
        params![
            i.session_id,
            i.game_id,
            i.environment_key,
            i.target_exe_path,
            i.windows_pid,
            i.process_created_u64,
            i.architecture,
            i.broker_instance_id,
            i.resource.snapshot_id,
            i.target_identity_sha256,
            now
        ],
    )
    .map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn ingest_hook_text(
    db: &mut Database,
    i: HookTextInput,
    now: i64,
) -> AppResult<CaptureOutcome> {
    ingest_hook_text_policy(db, i, true, false, now)
}
pub(in crate::storage) fn ingest_hook_text_with_policy(
    db: &mut Database,
    i: HookTextInput,
    consume_body: bool,
    now: i64,
) -> AppResult<CaptureOutcome> {
    ingest_hook_text_policy(db, i, consume_body, true, now)
}
fn ingest_hook_text_policy(
    db: &mut Database,
    i: HookTextInput,
    consume_body: bool,
    allow_received_terminal: bool,
    now: i64,
) -> AppResult<CaptureOutcome> {
    canonical_u64(&i.sequence_u64)?;
    canonical_u64(&i.source.hook_address_u64)?;
    canonical_u64(&i.source.context_u64)?;
    canonical_u64(&i.source.subcontext_u64)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id=?1 AND source_kind='hook' AND (state IN ('opening','active') OR ?2=1))",params![i.session_id,allow_received_terminal],|r|r.get(0)).map_err(err)?;
    if !active {
        return Err(AppError::new(
            "stale_capture_session",
            "capture",
            "已失效会话拒绝新事件",
        ));
    }
    let duplicate: Option<(String,String,Option<String>,String)> = tx.query_row("SELECT payload_sha256,session_id,source_id,wire_sequence_u64 FROM raw_events WHERE event_id=?1",[&i.event_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
    if let Some((hash, session, source, sequence)) = duplicate {
        if hash != content_hash(&i.text)
            || session != i.session_id
            || source.as_deref() != Some(&i.source.source_id)
            || sequence != i.sequence_u64
        {
            return Err(AppError::new(
                "event_identity_conflict",
                "capture",
                "事件身份被不同内容或来源复用",
            ));
        }
        ensure_source(&tx, &i.session_id, &i.source, now)?;
        return Ok(CaptureOutcome::default());
    }
    tx.execute(
        "UPDATE sessions SET state='active' WHERE session_id=?1 AND state='opening'",
        [&i.session_id],
    )
    .map_err(err)?;
    let s = &i.source;
    ensure_source(&tx, &i.session_id, s, now)?;
    let previous_sequence: i64 = tx
        .query_row(
            "SELECT coalesce(max(ingest_sequence),0) FROM raw_events WHERE session_id=?1",
            [&i.session_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let sequence = previous_sequence.checked_add(1).ok_or_else(|| {
        AppError::new(
            "sequence_exhausted",
            "capture",
            "本地接收序号已耗尽，拒绝丢精度或覆盖原始事实",
        )
    })?;
    let session_state: String = tx
        .query_row(
            "SELECT state FROM sessions WHERE session_id=?1",
            [&i.session_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let consume_body = consume_body && matches!(session_state.as_str(), "opening" | "active");
    let integrity = if matches!(session_state.as_str(), "opening" | "active") {
        "intact"
    } else {
        let lost:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM hook_events e JOIN capture_operations o USING(operation_id) WHERE coalesce(e.session_id,o.session_id)=?1 AND e.integrity='lost')",[&i.session_id],|r|r.get(0)).map_err(err)?;
        if lost { "lost" } else { "unknown" }
    };
    tx.execute(
        "INSERT INTO raw_events VALUES(?1,?2,?3,?4,?5,'text',?6,?7,?8,?9)",
        params![
            i.event_id,
            i.session_id,
            s.source_id,
            i.sequence_u64,
            sequence,
            i.text.len() as i64,
            content_hash(&i.text),
            now,
            integrity
        ],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT INTO raw_event_payloads VALUES(?1,?2,?3)",
        params![i.event_id, i.text.as_bytes(), now],
    )
    .map_err(err)?;
    let selected:Option<(String,u32)>=tx.query_row("SELECT source_id,selection_revision FROM current_source_selection_members WHERE session_id=?1 AND source_id=?2",params![i.session_id,s.source_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
    let mut outcome = CaptureOutcome::default();
    if let Some((source, selection)) = &selected
        && consume_body
        && source == &s.source_id
        && !i.text.trim().is_empty()
    {
        let language:String=tx.query_row("SELECT coalesce(nullif(g.source_language,''),'und') FROM sessions s LEFT JOIN games g USING(game_id) WHERE s.session_id=?1",[&i.session_id],|r|r.get(0)).map_err(err)?;
        let mut start = 0;
        while start < i.text.len() {
            let mut end = start.saturating_add(CAPTURE_MAX_BYTES).min(i.text.len());
            while !i.text.is_char_boundary(end) {
                end -= 1;
            }
            let fragment = &i.text[start..end];
            ingest_fragment(
                &tx,
                &i,
                *selection,
                &language,
                fragment,
                start,
                end,
                now,
                &mut outcome,
            )?;
            start = end;
        }
    }
    if !consume_body
        || selected
            .as_ref()
            .is_none_or(|(source, _)| source != &s.source_id)
        || i.text.trim().is_empty()
    {
        let reason = if !matches!(session_state.as_str(), "opening" | "active") {
            "text_inactive"
        } else if !consume_body {
            "text_paused"
        } else if i.text.trim().is_empty() {
            "text_empty"
        } else {
            "text_unselected"
        };
        let op = super::session_records::operation_for_session(&tx, &i.session_id)?;
        tx.execute("INSERT INTO hook_events(event_id,operation_id,session_id,source_id,observed_at_ms,level,stage,code,message,integrity,metadata_json) VALUES(?1,?2,?3,?4,?5,'info','capture',?6,?7,'intact','{}')",params![format!("disposition:{}",i.event_id),op,i.session_id,s.source_id,now,reason,match reason{"text_inactive"=>"会话正文消费已停止，已收到的原始回调仍保存","text_paused"=>"暂停期间已保存原始回调，未生成正文","text_empty"=>"已保存空白回调，未生成正文",_=>"已保存未选正文流的原始回调"}]).map_err(err)?;
    }
    bump(&tx, now)?;
    tx.commit().map_err(err)?;
    Ok(outcome)
}

pub(in crate::storage) fn ensure_source(
    tx: &Transaction<'_>,
    session: &str,
    s: &HookSourceInput,
    now: i64,
) -> AppResult<()> {
    require_id(&s.source_id)?;
    require_id(session)?;
    canonical_u64(&s.hook_address_u64)?;
    canonical_u64(&s.context_u64)?;
    canonical_u64(&s.subcontext_u64)?;
    tx.execute("INSERT INTO capture_sources VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(source_id) DO NOTHING",params![s.source_id,session,s.host_instance_id,s.hook_address_u64,s.context_u64,s.subcontext_u64,s.hook_code,s.hook_name,s.encoding,s.stream_identity_sha256,s.identity_extensions_json,now]).map_err(err)?;
    let identity:(String,String,String,String,String,String,String,String)=tx.query_row("SELECT session_id,host_instance_id,hook_address_u64,context_u64,subcontext_u64,hook_code,encoding,stream_identity_sha256 FROM capture_sources WHERE source_id=?1",[&s.source_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).map_err(err)?;
    if identity
        != (
            session.to_string(),
            s.host_instance_id.clone(),
            s.hook_address_u64.clone(),
            s.context_u64.clone(),
            s.subcontext_u64.clone(),
            s.hook_code.clone(),
            s.encoding.clone(),
            s.stream_identity_sha256.clone(),
        )
    {
        return Err(AppError::new(
            "source_identity_conflict",
            "capture",
            "文本流身份不得原地更换",
        ));
    }
    Ok(())
}

// New writes always use V3; persisted policy is read before any continuation.
const ACTIVE_CAPTURE_POLICY: CapturePolicy = CapturePolicy::V3;
type OpenRow = (String, u32, String, i64, i64, usize, String);
fn current_open(tx: &Transaction, session: &str, selection: u32) -> AppResult<Option<OpenRow>> {
    tx.query_row("SELECT u.unit_id,r.text_revision,r.source_text,u.created_at_ms,r.created_at_ms,(SELECT count(*) FROM source_refs f WHERE f.unit_id=r.unit_id AND f.text_revision=r.text_revision),r.processing_revision FROM reading_units u JOIN reading_unit_revisions r USING(unit_id) WHERE u.session_id=?1 AND u.state='open' AND r.selection_revision=?2 AND r.text_revision=(SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=u.unit_id) ORDER BY u.unit_ordinal DESC LIMIT 1",params![session,selection],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get::<_,u32>(5)? as usize,r.get(6)?))).optional().map_err(err)
}
fn seal_one(tx: &Transaction, unit: &str, revision: u32) -> AppResult<CaptureVersion> {
    tx.execute(
        "UPDATE reading_units SET state='finalized' WHERE unit_id=?1 AND state='open'",
        [unit],
    )
    .map_err(err)?;
    Ok(CaptureVersion {
        unit_id: unit.into(),
        text_revision: revision,
    })
}
fn capture_provenance_error() -> AppError {
    // Existing Repository error classification records storage_unconfirmed.
    AppError::new(
        "storage_failure",
        "capture",
        "正文来源跨度缺失、越界或与固定原文不一致，拒绝生成错误修订",
    )
}
struct CaptureRefSlice {
    event_id: String,
    start: usize,
    end: usize,
    bytes: Vec<u8>,
}
fn capture_ref_slices(
    tx: &Transaction<'_>,
    unit: &str,
    revision: u32,
    expected: &str,
) -> AppResult<Vec<CaptureRefSlice>> {
    let mut statement = tx.prepare("SELECT f.ref_ordinal,f.event_id,f.byte_start,f.byte_end,CASE WHEN f.byte_end-f.byte_start BETWEEN 0 AND ?4 THEN substr(p.payload_bytes,f.byte_start+1,f.byte_end-f.byte_start) END,length(p.payload_bytes) FROM source_refs f LEFT JOIN raw_event_payloads p USING(event_id) WHERE f.unit_id=?1 AND f.text_revision=?2 ORDER BY f.ref_ordinal LIMIT ?3").map_err(err)?;
    let mut rows = statement
        .query(params![
            unit,
            revision,
            (CAPTURE_MAX_REFS + 1) as i64,
            CAPTURE_MAX_BYTES as i64
        ])
        .map_err(err)?;
    let mut result = Vec::new();
    let mut reconstructed = Vec::new();
    while let Some(row) = rows.next().map_err(err)? {
        let ordinal = usize::try_from(row.get::<_, i64>(0).map_err(err)?)
            .map_err(|_| capture_provenance_error())?;
        let start = usize::try_from(row.get::<_, i64>(2).map_err(err)?)
            .map_err(|_| capture_provenance_error())?;
        let end = usize::try_from(row.get::<_, i64>(3).map_err(err)?)
            .map_err(|_| capture_provenance_error())?;
        let bytes = row
            .get::<_, Option<Vec<u8>>>(4)
            .map_err(err)?
            .ok_or_else(capture_provenance_error)?;
        let payload_len = row
            .get::<_, Option<i64>>(5)
            .map_err(err)?
            .and_then(|len| usize::try_from(len).ok())
            .ok_or_else(capture_provenance_error)?;
        if ordinal != result.len()
            || result.len() >= CAPTURE_MAX_REFS
            || end < start
            || end > payload_len
            || end - start != bytes.len()
            || std::str::from_utf8(&bytes).is_err()
            || reconstructed
                .len()
                .checked_add(bytes.len())
                .is_none_or(|len| len > CAPTURE_MAX_BYTES)
        {
            return Err(capture_provenance_error());
        }
        reconstructed.extend_from_slice(&bytes);
        result.push(CaptureRefSlice {
            event_id: row.get(1).map_err(err)?,
            start,
            end,
            bytes,
        });
    }
    if result.is_empty() || reconstructed != expected.as_bytes() {
        return Err(capture_provenance_error());
    }
    Ok(result)
}
fn quoted_prefix_refs(
    tx: &Transaction<'_>,
    unit: &str,
    revision: u32,
    old: &str,
    prefix_bytes: usize,
) -> AppResult<Vec<CaptureRefSlice>> {
    if !old.is_char_boundary(prefix_bytes) || prefix_bytes > old.len() {
        return Err(capture_provenance_error());
    }
    let refs = capture_ref_slices(tx, unit, revision, old)?;
    let mut remaining = prefix_bytes;
    let mut retained = Vec::new();
    for mut source in refs {
        if remaining == 0 {
            break;
        }
        let count = source.bytes.len().min(remaining);
        if count == 0 {
            continue;
        }
        if std::str::from_utf8(&source.bytes[..count]).is_err() {
            return Err(capture_provenance_error());
        }
        source.end = source
            .start
            .checked_add(count)
            .ok_or_else(capture_provenance_error)?;
        source.bytes.truncate(count);
        remaining = remaining
            .checked_sub(count)
            .ok_or_else(capture_provenance_error)?;
        retained.push(source);
    }
    if remaining != 0
        || retained
            .len()
            .checked_add(1)
            .is_none_or(|len| len > CAPTURE_MAX_REFS)
    {
        return Err(capture_provenance_error());
    }
    Ok(retained)
}
#[allow(clippy::too_many_arguments)]
fn ingest_fragment(
    tx: &Transaction,
    input: &HookTextInput,
    selection: u32,
    language: &str,
    fragment: &str,
    start: usize,
    end: usize,
    now: i64,
    outcome: &mut CaptureOutcome,
) -> AppResult<()> {
    let open = current_open(tx, &input.session_id, selection)?;
    let fold = open
        .as_ref()
        .map(|(_, revision, text, created, changed, refs, processing)| {
            // Never revise an older/unknown policy unit with V3 semantics.
            if CapturePolicy::from_processing_revision(processing) != Some(ACTIVE_CAPTURE_POLICY) {
                return CaptureFold::NewUnit;
            }
            ACTIVE_CAPTURE_POLICY.fold(
                &OpenCapture {
                    text,
                    created_at_ms: *created,
                    changed_at_ms: *changed,
                    text_revision: *revision,
                    ref_count: *refs,
                },
                fragment,
                now,
            )
        })
        .unwrap_or(CaptureFold::NewUnit);
    let (unit, revision, text) = match (&open, fold) {
        (Some((unit, revision, old, _, _, _, _)), CaptureFold::Append) => {
            (unit.clone(), revision + 1, format!("{old}{fragment}"))
        }
        (Some((unit, revision, _, _, _, _, _)), CaptureFold::GrowingSnapshot) => {
            (unit.clone(), revision + 1, fragment.to_owned())
        }
        (
            Some((unit, revision, old, _, _, _, _)),
            CaptureFold::GrowingQuotedSnapshot { prefix_bytes },
        ) => {
            let prefix = old
                .get(..prefix_bytes)
                .ok_or_else(capture_provenance_error)?;
            if prefix_bytes
                .checked_add(fragment.len())
                .is_none_or(|len| len > CAPTURE_MAX_BYTES)
            {
                return Err(capture_provenance_error());
            }
            (unit.clone(), revision + 1, format!("{prefix}{fragment}"))
        }
        _ => {
            if let Some((unit, revision, _, _, _, _, _)) = open.as_ref() {
                outcome
                    .finalized_versions
                    .push(seal_one(tx, unit, *revision)?);
            }
            // Equal-time fragments from one callback must sort by source span,
            // not unrelated hashes, while preserving opaque history cursors.
            let unit = format!("{}-{start:08x}", id("unit", &input.event_id));
            let ordinal: u32 = tx
                .query_row(
                    "SELECT coalesce(max(unit_ordinal),0)+1 FROM reading_units WHERE session_id=?1",
                    [&input.session_id],
                    |r| r.get(0),
                )
                .map_err(err)?;
            tx.execute(
                "INSERT INTO reading_units VALUES(?1,?2,?3,'open',?4)",
                params![unit, input.session_id, ordinal, now],
            )
            .map_err(err)?;
            (unit, 1, fragment.to_owned())
        }
    };
    let prefix_bytes = match fold {
        CaptureFold::GrowingQuotedSnapshot { prefix_bytes } => Some(prefix_bytes),
        _ => None,
    };
    let retained = match (&open, prefix_bytes) {
        (Some((old_unit, old_revision, old, _, _, _, _)), Some(prefix)) => {
            quoted_prefix_refs(tx, old_unit, *old_revision, old, prefix)?
        }
        _ => Vec::new(),
    };
    let fold_name = if prefix_bytes.is_some() {
        "GrowingQuotedSnapshot".to_owned()
    } else {
        format!("{fold:?}")
    };
    let policy = serde_json::json!({"policy":"bounded-quoted-growth-v3","quotedGrowth":"unique-open-quote-prefix-growth-v1","prefixBytes":prefix_bytes,"idleMs":CAPTURE_IDLE_MS,"idleFinalizesIncomplete":false,"completion":"balanced-terminal-v2","maxAgeMs":CAPTURE_MAX_AGE_MS,"maxBytes":CAPTURE_MAX_BYTES,"maxRevisions":CAPTURE_MAX_REVISIONS,"maxRefs":CAPTURE_MAX_REFS,"fold":fold_name,"join":"exact-ref-concatenation","unitProjection":"latest"});
    tx.execute(
        "INSERT INTO reading_unit_revisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            unit,
            revision,
            input.session_id,
            text,
            text.len() as i64,
            content_hash(&text),
            language,
            ACTIVE_CAPTURE_POLICY.processing_revision(),
            json(&policy)?,
            selection,
            now
        ],
    )
    .map_err(err)?;
    if fold == CaptureFold::Append {
        tx.execute("INSERT INTO source_refs SELECT unit_id,?2,session_id,ref_ordinal,event_id,byte_start,byte_end FROM source_refs WHERE unit_id=?1 AND text_revision=?3",params![unit,revision,revision-1]).map_err(err)?;
    }
    for (ordinal, source) in retained.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).map_err(|_| capture_provenance_error())?;
        let start = i64::try_from(source.start).map_err(|_| capture_provenance_error())?;
        let end = i64::try_from(source.end).map_err(|_| capture_provenance_error())?;
        tx.execute(
            "INSERT INTO source_refs VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                unit,
                revision,
                input.session_id,
                ordinal,
                source.event_id,
                start,
                end
            ],
        )
        .map_err(err)?;
    }
    let ordinal: u32 = tx
        .query_row(
            "SELECT count(*) FROM source_refs WHERE unit_id=?1 AND text_revision=?2",
            params![unit, revision],
            |r| r.get(0),
        )
        .map_err(err)?;
    tx.execute(
        "INSERT INTO source_refs VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            unit,
            revision,
            input.session_id,
            ordinal,
            input.event_id,
            start as i64,
            end as i64
        ],
    )
    .map_err(err)?;
    if prefix_bytes.is_some() {
        // Verify the actual new refs/payload slices, not a second text heuristic.
        capture_ref_slices(tx, &unit, revision, &text)?;
    }
    let visible = CaptureVersion {
        unit_id: unit.clone(),
        text_revision: revision,
    };
    if ACTIVE_CAPTURE_POLICY.ends_sentence(&text)
        || text.len() >= CAPTURE_MAX_BYTES
        || revision >= CAPTURE_MAX_REVISIONS
    {
        outcome
            .finalized_versions
            .push(seal_one(tx, &unit, revision)?);
    }
    outcome.visible_version = Some(visible);
    Ok(())
}

pub(in crate::storage) fn seal_capture_units(
    tx: &Transaction,
    session: Option<&str>,
    now: i64,
    due_only: bool,
) -> AppResult<Vec<CaptureVersion>> {
    let mut query=tx.prepare("SELECT u.unit_id,r.text_revision,r.source_text,u.created_at_ms,r.created_at_ms,r.processing_revision FROM reading_units u JOIN reading_unit_revisions r USING(unit_id) JOIN sessions s ON s.session_id=u.session_id WHERE u.state='open' AND (?1 IS NULL OR u.session_id=?1) AND (?2=0 OR s.state IN ('opening','active')) AND r.text_revision=(SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=u.unit_id)").map_err(err)?;
    let rows = query
        .query_map(params![session, due_only], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut result = Vec::new();
    for (unit, revision, text, created, changed, processing) in rows {
        if !due_only
            || CapturePolicy::from_processing_revision(&processing).is_none_or(|policy| {
                policy.due(
                    &OpenCapture {
                        text: &text,
                        created_at_ms: created,
                        changed_at_ms: changed,
                        text_revision: revision,
                        ref_count: 0,
                    },
                    now,
                )
            })
        {
            result.push(seal_one(tx, &unit, revision)?);
        }
    }
    Ok(result)
}
pub(in crate::storage) fn flush_hook_capture(
    db: &mut Database,
    request: CaptureFlush,
    now: i64,
) -> AppResult<Vec<CaptureVersion>> {
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let result = seal_capture_units(
        &tx,
        request.session_id.as_deref(),
        now,
        request.reason == CaptureFlushReason::Due,
    )?;
    if !result.is_empty() {
        bump(&tx, now)?;
    }
    tx.commit().map_err(err)?;
    Ok(result)
}
pub(in crate::storage) fn set_tokens(
    db: &mut Database,
    i: TokenSnapshotInput,
    now: i64,
) -> AppResult<()> {
    set_tokens_with_replay(db, i, now, false)
}
pub(in crate::storage) fn set_tokens_for_retry(
    db: &mut Database,
    i: TokenSnapshotInput,
    now: i64,
) -> AppResult<()> {
    set_tokens_with_replay(db, i, now, true)
}
fn set_tokens_with_replay(
    db: &mut Database,
    i: TokenSnapshotInput,
    now: i64,
    allow_identical_retry: bool,
) -> AppResult<()> {
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let u = unit(&tx, &i.unit_id, i.text_revision)?;
    app_core::domain::require_id(&i.analysis_revision)?;
    let mut last = 0;
    for t in &i.tokens {
        let span = utf16_span(&u.source_text, t.byte_start, t.byte_end)?;
        if t.byte_start < last
            || t.byte_end <= t.byte_start
            || u.source_text
                .get(t.byte_start as usize..t.byte_end as usize)
                != Some(t.surface.as_str())
            || span != (t.utf16_start, t.utf16_end)
        {
            return Err(AppError::new(
                "invalid_token_alignment",
                "language",
                "词法输出与固定原文不一致",
            ));
        }
        last = t.byte_end;
    }
    resource(&tx, &i.resource, now)?;
    let lid = id(
        "lexical",
        &format!(
            "{}:{}:{}:language-v1:{}",
            i.unit_id, i.text_revision, i.resource.manifest_sha256, i.analysis_revision
        ),
    );
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM lexical_snapshots WHERE lexical_id=?1)",
            [&lid],
            |r| r.get(0),
        )
        .map_err(err)?;
    if exists && allow_identical_retry {
        let mut q=tx.prepare("SELECT surface,lemma,coalesce(reading_kana,''),pos_major,byte_start,byte_end,is_known FROM lexical_tokens WHERE lexical_id=?1 ORDER BY token_index").map_err(err)?;
        let actual = q
            .query_map([&lid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, u32>(4)?,
                    r.get::<_, u32>(5)?,
                    r.get::<_, bool>(6)?,
                ))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        let expected = i
            .tokens
            .iter()
            .map(|t| {
                (
                    t.surface.clone(),
                    t.lemma.clone(),
                    t.reading.clone(),
                    t.part_of_speech.clone(),
                    t.byte_start,
                    t.byte_end,
                    t.known,
                )
            })
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(AppError::new(
                "lexical_identity_conflict",
                "language",
                "同一词法资源与分析版本不能覆盖不同结果",
            ));
        }
        return Ok(());
    }
    tx.execute(
        "INSERT INTO lexical_snapshots VALUES(?1,?2,?3,?4,?5,'language-v1',?6,?7,?8)",
        params![
            lid,
            i.unit_id,
            i.text_revision,
            u.source_hash,
            i.resource.snapshot_id,
            i.analysis_revision,
            i.tokens.len() as i64,
            now
        ],
    )
    .map_err(err)?;
    for (index, t) in i.tokens.iter().enumerate() {
        tx.execute(
            "INSERT INTO lexical_tokens VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'{}',?9,?10)",
            params![
                lid,
                index as i64,
                t.byte_start,
                t.byte_end,
                t.surface,
                t.lemma,
                if t.reading.is_empty() {
                    None
                } else {
                    Some(&t.reading)
                },
                t.part_of_speech,
                if t.surface.chars().all(char::is_whitespace) {
                    "whitespace"
                } else if !t.known {
                    "unknown"
                } else if t
                    .surface
                    .chars()
                    .any(|c| c.is_alphabetic() || c.is_numeric())
                {
                    "word"
                } else {
                    "punctuation"
                },
                t.known
            ],
        )
        .map_err(err)?;
    }
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
