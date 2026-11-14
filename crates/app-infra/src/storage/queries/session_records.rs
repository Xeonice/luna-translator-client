//! Three typed, full-set queries over authoritative data; cursors pin a server-side range.
use super::*;
use app_core::ports::runtime::HookEventInput;
use std::time::{Duration, Instant};

pub(in crate::storage) fn association_progress_facts(
    db: &mut Database,
    operation: &str,
) -> AppResult<app_core::ports::runtime::AssociationOperationFacts> {
    require_id(operation)?;
    let profile = super::source_selection::profile(&db.conn)?;
    if profile.profile_id != db.profile_id || profile.server_instance_id != db.server_id {
        return Err(AppError::new("stale_binding_profile", "capture", "接入进度不属于当前运行配置档"));
    }
    let (request_sha256, session_id, started_at_ms, finished_at_ms): (String, Option<String>, i64, Option<i64>) = db.conn.query_row(
        "SELECT request_sha256,session_id,started_at_ms,ended_at_ms FROM capture_operations WHERE operation_id=?1", [operation],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).optional().map_err(err)?.ok_or_else(|| AppError::new("association_operation_missing", "capture", "未找到该次实际接入操作"))?;
    let mut statement = db.conn.prepare("SELECT event_id,event_sequence,session_id,observed_at_ms,level,stage,code,message FROM (SELECT * FROM hook_events WHERE operation_id=?1 ORDER BY event_sequence DESC LIMIT 512) ORDER BY event_sequence").map_err(err)?;
    let events = statement.query_map([operation], |row| Ok(app_core::ports::runtime::AssociationJournalFact {
        event_id: row.get(0)?, ingest_sequence: row.get::<_,i64>(1)?.to_string(), session_id: row.get(2)?, observed_at_ms: row.get(3)?,
        level: row.get(4)?, stage: row.get(5)?, code: row.get(6)?, message: row.get(7)?,
    })).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    Ok(app_core::ports::runtime::AssociationOperationFacts { profile, operation_id: operation.into(), request_sha256, session_id, started_at_ms, finished_at_ms, events })
}
#[derive(Clone)]
pub(in crate::storage) struct FrozenRange {
    id: String,
    session: String,
    dataset: SessionDataset,
    order: SessionRecordOrder,
    filter: SessionRecordFilter,
    high: i64,
    hook_high: i64,
    raw_high: i64,
    revision: u32,
    matched: i64,
    cleaned: i64,
    created: Instant,
    export_pin: bool,
    issued_boundaries: std::collections::BTreeSet<i64>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    snapshot: String,
    after: String,
}
fn invalid_cursor() -> AppError {
    AppError::new(
        "invalid_cursor",
        "session_records",
        "分页范围、会话或筛选条件已变化，请重新加载",
    )
}
fn dataset_name(value: &SessionDataset) -> &'static str {
    match value {
        SessionDataset::Body => "body",
        SessionDataset::Extraction => "extraction",
        SessionDataset::HookEvents => "hook_events",
    }
}
fn cursor(range: &FrozenRange, after: i64) -> AppResult<String> {
    json(&Cursor {
        snapshot: range.id.clone(),
        after: after.to_string(),
    })
}
fn decode(text: &str) -> AppResult<Cursor> {
    if text.len() > 4096 {
        return Err(invalid_cursor());
    }
    serde_json::from_str(text).map_err(|_| invalid_cursor())
}
fn parse_seq(text: &str) -> AppResult<i64> {
    let n = canonical_u64(text).map_err(|_| invalid_cursor())?;
    i64::try_from(n).map_err(|_| invalid_cursor())
}
fn session_exists(db: &Database, session: &str) -> AppResult<()> {
    let exists: bool = db
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id=?1)",
            [session],
            |r| r.get(0),
        )
        .map_err(err)?;
    if !exists {
        return Err(AppError::new(
            "session_missing",
            "session_records",
            "会话不存在",
        ));
    }
    Ok(())
}
fn max_sequence(conn: &Connection, session: &str, dataset: &SessionDataset) -> AppResult<i64> {
    let sql = match dataset {
        SessionDataset::Body => {
            "SELECT coalesce(max(record_sequence),0) FROM body_record_positions WHERE session_id=?1"
        }
        SessionDataset::Extraction => {
            "SELECT coalesce(max(ingest_sequence),0) FROM raw_events WHERE session_id=?1 AND event_kind='text'"
        }
        SessionDataset::HookEvents => {
            "SELECT coalesce(max(e.event_sequence),0) FROM hook_events e JOIN capture_operations o USING(operation_id) WHERE coalesce(e.session_id,o.session_id)=?1"
        }
    };
    conn.query_row(sql, [session], |r| r.get(0)).map_err(err)
}
fn first_sequence(
    conn: &Connection,
    session: &str,
    dataset: &SessionDataset,
    retained: bool,
) -> AppResult<Option<i64>> {
    let sql = match dataset {
        SessionDataset::Body => {
            "SELECT min(record_sequence) FROM body_record_positions WHERE session_id=?1"
        }
        SessionDataset::Extraction if retained => {
            "SELECT min(e.ingest_sequence) FROM raw_events e JOIN raw_event_payloads p USING(event_id) WHERE e.session_id=?1 AND e.event_kind='text'"
        }
        SessionDataset::Extraction => {
            "SELECT min(ingest_sequence) FROM raw_events WHERE session_id=?1 AND event_kind='text'"
        }
        SessionDataset::HookEvents => {
            "SELECT min(e.event_sequence) FROM hook_events e JOIN capture_operations o USING(operation_id) WHERE coalesce(e.session_id,o.session_id)=?1"
        }
    };
    conn.query_row(sql, [session], |r| r.get(0)).map_err(err)
}
fn filtered_sql(dataset: &SessionDataset, count: bool, order: SessionRecordOrder) -> String {
    let (select, from, seq, filters) = match dataset {
        SessionDataset::Body => (
            "b.record_sequence,b.unit_id,b.text_revision",
            "body_record_positions b JOIN reading_unit_revisions r USING(unit_id,text_revision,session_id)",
            "b.record_sequence",
            "b.session_id=?1 AND (?4 IS NULL OR instr(r.source_text,?4)>0)",
        ),
        SessionDataset::Extraction => (
            "e.ingest_sequence,e.event_id,e.source_id,e.wire_sequence_u64,p.payload_bytes,e.observed_at_ms",
            "raw_events e LEFT JOIN raw_event_payloads p USING(event_id)",
            "e.ingest_sequence",
            "e.session_id=?1 AND e.event_kind='text' AND (?4 IS NULL OR instr(CAST(p.payload_bytes AS TEXT),?4)>0) AND (?5 IS NULL OR e.source_id=?5)",
        ),
        SessionDataset::HookEvents => (
            "e.event_sequence,e.event_id,e.operation_id,coalesce(e.session_id,o.session_id),e.source_id,e.wire_sequence_u64,e.observed_at_ms,e.level,e.stage,e.code,e.message,e.lost_count_u64,e.integrity",
            "hook_events e JOIN capture_operations o USING(operation_id)",
            "e.event_sequence",
            "coalesce(e.session_id,o.session_id)=?1 AND (?4 IS NULL OR instr(e.message,?4)>0 OR instr(e.code,?4)>0) AND (?6 IS NULL OR e.level=?6) AND (?7 IS NULL OR (?7='diagnostics' AND NOT (e.level='info' AND e.stage='capture' AND e.integrity='intact' AND e.lost_count_u64 IS NULL AND e.code IN ('text_unselected','text_paused','text_empty','text_inactive'))) OR e.stage=?7)",
        ),
    };
    // All variants intentionally bind the same seven placeholders.
    let select = if count { "count(*)" } else { select };
    let boundary = match order {
        SessionRecordOrder::OldestFirst => format!("{seq}>?2"),
        SessionRecordOrder::NewestFirst => format!("(?2=0 OR {seq}<?2)"),
    };
    let direction = if order == SessionRecordOrder::NewestFirst {
        "DESC"
    } else {
        "ASC"
    };
    format!(
        "SELECT {select} FROM {from} WHERE {filters} AND {boundary} AND {seq}<=?3 AND (?5 IS NULL OR ?5 IS NOT NULL) AND (?6 IS NULL OR ?6 IS NOT NULL) AND (?7 IS NULL OR ?7 IS NOT NULL){}",
        if count {
            String::new()
        } else {
            format!(" ORDER BY {seq} {direction} LIMIT ?8")
        }
    )
}
fn count(conn: &Connection, range: &FrozenRange, after: i64, high: i64) -> AppResult<i64> {
    conn.query_row(
        &filtered_sql(&range.dataset, true, SessionRecordOrder::OldestFirst),
        params![
            range.session,
            after,
            high,
            range.filter.search,
            range.filter.source_id,
            range.filter.level,
            range.filter.stage
        ],
        |r| r.get(0),
    )
    .map_err(err)
}
fn cleaned(conn: &Connection, session: &str) -> AppResult<i64> {
    conn.query_row("SELECT count(*) FROM raw_events e LEFT JOIN raw_event_payloads p USING(event_id) WHERE e.session_id=?1 AND e.event_kind='text' AND p.event_id IS NULL",[session],|r|r.get(0)).map_err(err)
}
fn new_range(db: &mut Database, query: &SessionRecordQuery) -> AppResult<FrozenRange> {
    session_exists(db, &query.session_id)?;
    db.record_ranges
        .retain(|_, r| r.export_pin || r.created.elapsed() < Duration::from_secs(1800));
    if db.record_ranges.len() >= 4096 {
        return Err(AppError::new(
            "snapshot_capacity",
            "session_records",
            "读取范围数量达到上限，请关闭旧范围或稍后重试",
        ));
    }
    db.record_range_counter = db.record_range_counter.checked_add(1).ok_or_else(|| {
        AppError::new("sequence_exhausted", "session_records", "读取范围序号耗尽")
    })?;
    let mut range = FrozenRange {
        id: content_hash(format!(
            "{}:{}:{}:{}",
            db.server_id,
            db.profile_id,
            db.record_range_counter,
            dataset_name(&query.dataset)
        )),
        session: query.session_id.clone(),
        dataset: query.dataset.clone(),
        order: query.order,
        filter: query.filter.clone(),
        high: max_sequence(&db.conn, &query.session_id, &query.dataset)?,
        hook_high: max_sequence(&db.conn, &query.session_id, &SessionDataset::HookEvents)?,
        raw_high: max_sequence(&db.conn, &query.session_id, &SessionDataset::Extraction)?,
        revision: revision(&db.conn)?,
        matched: 0,
        cleaned: cleaned(&db.conn, &query.session_id)?,
        created: Instant::now(),
        export_pin: false,
        issued_boundaries: std::collections::BTreeSet::from([0]),
    };
    range.matched = count(&db.conn, &range, 0, range.high)?;
    db.record_ranges.insert(range.id.clone(), range.clone());
    Ok(range)
}
fn resolve(db: &mut Database, query: &SessionRecordQuery) -> AppResult<(FrozenRange, i64)> {
    app_core::application::session_records::validate_query(query)?;
    match &query.cursor {
        None => Ok((new_range(db, query)?, 0)),
        Some(token) => {
            let c = decode(token)?;
            let range = db
                .record_ranges
                .get(&c.snapshot)
                .cloned()
                .ok_or_else(invalid_cursor)?;
            if range.session != query.session_id
                || range.dataset != query.dataset
                || range.order != query.order
                || range.filter != query.filter
            {
                return Err(invalid_cursor());
            }
            if !range.export_pin && range.created.elapsed() >= Duration::from_secs(1800) {
                return Err(AppError::new(
                    "snapshot_expired",
                    "session_records",
                    "读取范围已过期，请重新加载",
                ));
            }
            let after = parse_seq(&c.after)?;
            if after > range.high || !range.issued_boundaries.contains(&after) {
                return Err(invalid_cursor());
            }
            if cleaned(&db.conn, &range.session)? != range.cleaned {
                return Err(AppError::new(
                    "cursor_range_changed",
                    "session_records",
                    "读取期间原始内容保留范围发生变化，请重新核对",
                ));
            }
            if count(&db.conn, &range, 0, range.high)? != range.matched {
                return Err(AppError::new(
                    "cursor_range_changed",
                    "session_records",
                    "固定范围中的记录已被清理，请重新核对",
                ));
            }
            Ok((range, after))
        }
    }
}
fn completeness(db: &Database, range: &FrozenRange) -> AppResult<SessionCompleteness> {
    let loss:bool=db.conn.query_row("SELECT EXISTS(SELECT 1 FROM hook_events e JOIN capture_operations o USING(operation_id) WHERE coalesce(e.session_id,o.session_id)=?1 AND e.event_sequence<=?2 AND e.integrity='lost') OR EXISTS(SELECT 1 FROM raw_events WHERE session_id=?1 AND ingest_sequence<=?3 AND integrity_state='lost')",params![range.session,range.hook_high,range.raw_high],|r|r.get(0)).map_err(err)?;
    let unknown:bool=db.conn.query_row("SELECT EXISTS(SELECT 1 FROM capture_operations WHERE session_id=?1 AND legacy_unknown_start=1) OR EXISTS(SELECT 1 FROM sessions WHERE session_id=?1 AND state='crashed') OR EXISTS(SELECT 1 FROM hook_events e JOIN capture_operations o USING(operation_id) WHERE coalesce(e.session_id,o.session_id)=?1 AND (e.event_sequence<=?2 OR e.code='storage_unconfirmed') AND e.integrity='unknown') OR EXISTS(SELECT 1 FROM raw_events WHERE session_id=?1 AND ingest_sequence<=?3 AND integrity_state='unknown')",params![range.session,range.hook_high,range.raw_high],|r|r.get(0)).map_err(err)?;

    let status = if range.cleaned > 0 {
        "cleaned"
    } else if loss {
        "incomplete"
    } else if unknown {
        "unknown"
    } else {
        "complete_received_range"
    };
    Ok(SessionCompleteness {
        status: status.into(),
        scope: "received_range_only".into(),
        known_loss: loss,
        unknown_range: unknown,
        cleaned_payload_count: range.cleaned.to_string(),
        notes: vec!["仅表示客户端已接收范围，不保证上游剧情完整".into()],
    })
}
pub(in crate::storage) fn page(
    db: &mut Database,
    query: SessionRecordQuery,
) -> AppResult<SessionRecordPage> {
    let (range, after) = resolve(db, &query)?;
    let mut stmt = db
        .conn
        .prepare(&filtered_sql(&range.dataset, false, range.order))
        .map_err(err)?;
    let mut rows = stmt
        .query(params![
            range.session,
            after,
            range.high,
            range.filter.search,
            range.filter.source_id,
            range.filter.level,
            range.filter.stage,
            query.page_size + 1
        ])
        .map_err(err)?;
    let mut records = Vec::new();
    let mut positions = Vec::new();
    while let Some(row) = rows.next().map_err(err)? {
        let sequence: i64 = row.get(0).map_err(err)?;
        positions.push(sequence);
        let record = match range.dataset {
            SessionDataset::Body => {
                let uid: String = row.get(1).map_err(err)?;
                let rev: u32 = row.get(2).map_err(err)?;
                let mut refs=db.conn.prepare("SELECT event_id,byte_start,byte_end FROM source_refs WHERE unit_id=?1 AND text_revision=?2 ORDER BY ref_ordinal").map_err(err)?;
                let refs = refs
                    .query_map(params![uid, rev], |r| {
                        Ok(BodySourceReference {
                            event_id: r.get(0)?,
                            byte_start: r.get(1)?,
                            byte_end: r.get(2)?,
                        })
                    })
                    .map_err(err)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(err)?;
                SessionRecord::Body {
                    sequence: sequence.to_string(),
                    unit: unit(&db.conn, &uid, rev)?,
                    source_refs: refs,
                }
            }
            SessionDataset::Extraction => {
                let event_id: String = row.get(1).map_err(err)?;
                let bytes: Option<Vec<u8>> = row.get(4).map_err(err)?;
                let text = bytes.map(String::from_utf8).transpose().map_err(err)?;
                let mut refs=db.conn.prepare("SELECT DISTINCT unit_id,text_revision FROM source_refs WHERE event_id=?1 ORDER BY unit_id,text_revision").map_err(err)?;
                let refs = refs
                    .query_map([&event_id], |r| {
                        Ok(BodyRecordReference {
                            unit_id: r.get(0)?,
                            text_revision: r.get(1)?,
                        })
                    })
                    .map_err(err)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(err)?;
                let reason = if !refs.is_empty() {
                    "body".into()
                } else {
                    db.conn
                        .query_row(
                            "SELECT code FROM hook_events WHERE event_id=?1",
                            [format!("disposition:{event_id}")],
                            |r| r.get(0),
                        )
                        .optional()
                        .map_err(err)?
                        .unwrap_or_else(|| "not_consumed".into())
                };
                SessionRecord::Extraction {
                    sequence: sequence.to_string(),
                    event_id,
                    source_id: row.get(2).map_err(err)?,
                    wire_sequence: row.get(3).map_err(err)?,
                    payload_retained: text.is_some(),
                    text,
                    observed_at_ms: row.get::<_, i64>(5).map_err(err)? as f64,
                    reason,
                    body_refs: refs,
                }
            }
            SessionDataset::HookEvents => SessionRecord::HookEvent {
                sequence: sequence.to_string(),
                event_id: row.get(1).map_err(err)?,
                operation_id: row.get(2).map_err(err)?,
                session_id: row.get(3).map_err(err)?,
                source_id: row.get(4).map_err(err)?,
                wire_sequence: row.get(5).map_err(err)?,
                observed_at_ms: row.get::<_, i64>(6).map_err(err)? as f64,
                level: row.get(7).map_err(err)?,
                stage: row.get(8).map_err(err)?,
                code: row.get(9).map_err(err)?,
                message: row.get(10).map_err(err)?,
                lost_count: row.get(11).map_err(err)?,
                integrity: row.get(12).map_err(err)?,
            },
        };
        records.push(record);
    }
    drop(rows);
    drop(stmt);
    let has_more = records.len() > query.page_size as usize;
    records.truncate(query.page_size as usize);
    positions.truncate(query.page_size as usize);
    let boundary = positions.last().copied();
    if range.order == SessionRecordOrder::NewestFirst {
        records.reverse();
        positions.reverse();
    }
    let next_cursor = if has_more {
        boundary.map(|p| cursor(&range, p)).transpose()?
    } else {
        None
    };
    if has_more
        && let (Some(boundary), Some(saved)) = (boundary, db.record_ranges.get_mut(&range.id))
    {
        saved.issued_boundaries.insert(boundary);
    }
    let latest = max_sequence(&db.conn, &range.session, &range.dataset)?;
    let first: Option<i64> = positions.first().copied();
    let last = positions.last().copied();
    let first_available =
        first_sequence(&db.conn, &range.session, &range.dataset, false)?.map(|n| n.to_string());
    let retained_from =
        first_sequence(&db.conn, &range.session, &range.dataset, true)?.map(|n| n.to_string());
    Ok(SessionRecordPage {
        records,
        next_cursor,
        loaded_range: SequenceRange {
            first: first.map(|n| n.to_string()),
            last: last.map(|n| n.to_string()),
        },
        completeness: completeness(db, &range)?,
        snapshot: SessionRangeSnapshot {
            snapshot_id: range.id.clone(),
            refresh_cursor: cursor(&range, 0)?,
            high_watermark: range.high.to_string(),
            first_available,
            last_received: if latest > 0 {
                Some(latest.to_string())
            } else {
                None
            },
            last_persisted: if latest > 0 {
                Some(latest.to_string())
            } else {
                None
            },
            retained_from,
            matching_count: range.matched.to_string(),
            newer_count: count(&db.conn, &range, range.high, latest)?.to_string(),
            database_revision: range.revision.to_string(),
        },
    })
}
pub(in crate::storage) fn prepare_export(
    db: &mut Database,
    request: SessionExportRequest,
) -> AppResult<SessionExportPlan> {
    if !request.privacy_confirmed {
        return Err(AppError::new(
            "export_confirmation_required",
            "session_records",
            "导出前请确认台词与目标信息范围",
        ));
    }
    if db.record_exports.len() >= 32 {
        return Err(AppError::new(
            "snapshot_capacity",
            "session_records",
            "进行中的导出过多",
        ));
    }
    let original = page(db, request.query.clone())?;
    let mut range = db
        .record_ranges
        .get(&original.snapshot.snapshot_id)
        .cloned()
        .ok_or_else(invalid_cursor)?;
    if db.record_ranges.len() >= 4096 {
        return Err(AppError::new(
            "snapshot_capacity",
            "session_records",
            "读取范围数量达到上限",
        ));
    }
    db.record_range_counter = db
        .record_range_counter
        .checked_add(1)
        .ok_or_else(|| AppError::new("sequence_exhausted", "session_records", "导出序号耗尽"))?;
    range.id = content_hash(format!(
        "export:{}:{}:{}",
        db.server_id, original.snapshot.snapshot_id, db.record_range_counter
    ));
    range.order = SessionRecordOrder::OldestFirst;
    range.export_pin = true;
    range.issued_boundaries = std::collections::BTreeSet::from([0]);
    let id = range.id.clone();
    let initial = cursor(&range, 0)?;
    db.record_ranges.insert(id.clone(), range);
    let p = page(
        db,
        SessionRecordQuery {
            session_id: request.query.session_id.clone(),
            dataset: request.query.dataset.clone(),
            order: SessionRecordOrder::OldestFirst,
            filter: request.query.filter.clone(),
            cursor: Some(initial),
            page_size: request.query.page_size,
        },
    )?;
    let export_id = format!("export:{id}");
    db.record_exports.insert(export_id.clone(), id);
    Ok(SessionExportPlan {
        export_id,
        session_id: request.query.session_id,
        dataset: request.query.dataset.clone(),
        order: SessionRecordOrder::OldestFirst,
        filter: request.query.filter,
        snapshot: p.snapshot,
        completeness: p.completeness,
        contains_game_text: request.query.dataset != SessionDataset::HookEvents,
        contains_target_identity: request.query.dataset == SessionDataset::HookEvents,
    })
}
pub(in crate::storage) fn export_page(
    db: &mut Database,
    export_id: &str,
    token: Option<String>,
    size: u32,
) -> AppResult<SessionRecordPage> {
    let id = db
        .record_exports
        .get(export_id)
        .ok_or_else(|| AppError::new("export_missing", "session_records", "导出已取消或不存在"))?
        .clone();
    let range = db
        .record_ranges
        .get(&id)
        .ok_or_else(invalid_cursor)?
        .clone();
    if let Some(ref t) = token
        && decode(t)?.snapshot != id
    {
        return Err(invalid_cursor());
    }
    let token = token.unwrap_or(cursor(&range, 0)?);
    page(
        db,
        SessionRecordQuery {
            session_id: range.session,
            dataset: range.dataset,
            order: range.order,
            filter: range.filter,
            cursor: Some(token),
            page_size: size,
        },
    )
}
pub(in crate::storage) fn export_session(db: &Database, id: &str) -> AppResult<String> {
    let range = db
        .record_exports
        .get(id)
        .and_then(|key| db.record_ranges.get(key))
        .ok_or_else(|| AppError::new("export_missing", "session_records", "导出已取消或不存在"))?;
    Ok(range.session.clone())
}
pub(in crate::storage) fn close_export(db: &mut Database, id: &str) -> AppResult<()> {
    if let Some(snapshot) = db.record_exports.remove(id)
        && let Some(range) = db.record_ranges.get_mut(&snapshot)
    {
        range.export_pin = false;
    }
    Ok(())
}
pub(in crate::storage) fn session_pinned(db: &Database, session: &str) -> bool {
    db.record_ranges
        .values()
        .any(|r| r.export_pin && r.session == session)
}
pub(in crate::storage) fn begin_operation(
    db: &mut Database,
    operation: &str,
    hash: &str,
    now: i64,
) -> AppResult<()> {
    require_id(operation)?;
    if now < 0
        || hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(AppError::new(
            "invalid_capture_operation",
            "capture",
            "接入操作身份或时间无效",
        ));
    }
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let prior: Option<(String, String)> = tx
        .query_row(
            "SELECT request_sha256,state FROM capture_operations WHERE operation_id=?1",
            [operation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    if let Some((prior, state)) = prior {
        if prior != hash {
            return Err(AppError::new(
                "operation_conflict",
                "capture",
                "操作身份已用于其他目标",
            ));
        }
        if state != "opening" {
            return Err(AppError::new(
                "operation_terminal",
                "capture",
                "该接入操作已完成，请为重试创建新操作",
            ));
        }
        return Ok(());
    }
    tx.execute(
        "INSERT INTO capture_operations VALUES(?1,?2,NULL,'opening',?3,NULL,0)",
        params![operation, hash, now],
    )
    .map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn finish_operation(
    db: &mut Database,
    operation: &str,
    session: Option<&str>,
    now: i64,
) -> AppResult<()> {
    if now < 0 {
        return Err(AppError::new("invalid_time", "capture", "时间不得为负"));
    }
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let prior: Option<(Option<String>, String)> = tx
        .query_row(
            "SELECT session_id,state FROM capture_operations WHERE operation_id=?1",
            [operation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((old_session, state)) = prior else {
        return Err(AppError::new(
            "operation_missing",
            "capture",
            "接入操作不存在",
        ));
    };
    if state != "opening" {
        if old_session.as_deref() == session {
            return Ok(());
        }
        return Err(AppError::new(
            "operation_conflict",
            "capture",
            "接入操作已关联其他会话",
        ));
    }
    tx.execute("UPDATE capture_operations SET session_id=?2,state=CASE WHEN ?2 IS NULL THEN 'failed' ELSE 'active' END,ended_at_ms=CASE WHEN ?2 IS NULL THEN ?3 ELSE NULL END WHERE operation_id=?1 AND state='opening'",params![operation,session,now]).map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn operation_for_session(
    conn: &Connection,
    session: &str,
) -> AppResult<String> {
    let found:Option<String>=conn.query_row("SELECT o.operation_id FROM capture_operations o LEFT JOIN hook_events e ON e.operation_id=o.operation_id AND e.code='association_ready' WHERE o.session_id=?1 GROUP BY o.operation_id ORDER BY max(e.event_sequence) DESC,o.started_at_ms DESC,o.operation_id DESC LIMIT 1",[session],|r|r.get(0)).optional().map_err(err)?;
    if let Some(id) = found {
        return Ok(id);
    }
    let id = format!("legacy-session:{session}");
    conn.execute("INSERT INTO capture_operations SELECT ?1,?2,session_id,CASE WHEN state IN ('opening','active') THEN 'active' ELSE 'recovered' END,started_at_ms,ended_at_ms,1 FROM sessions WHERE session_id=?3 AND source_kind='hook'",params![id,content_hash(&id),session]).map_err(err)?;
    Ok(id)
}
pub(in crate::storage) fn record_event(db: &mut Database, input: HookEventInput) -> AppResult<()> {
    require_id(&input.event_id)?;
    if input.observed_at_ms < 0
        || !["info", "warning", "error"].contains(&input.detail.level.as_str())
        || !app_core::application::session_records::valid_stage(&input.detail.stage)
        || !["intact", "lost", "unknown"].contains(&input.integrity.as_str())
        || input.detail.code.is_empty()
        || !input
            .detail
            .code
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        || input.detail.message.contains('\0')
        || input.detail.message.len() > 4096
    {
        return Err(AppError::new(
            "invalid_hook_event",
            "capture",
            "Hook事件类型或安全字段无效",
        ));
    }
    if let Some(w) = &input.wire_sequence {
        canonical_u64(w)?;
    }
    if let Some(w) = &input.lost_count {
        canonical_u64(w)?;
    }
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let operation = match input.operation_id {
        Some(id) => id,
        None => operation_for_session(
            &tx,
            input.session_id.as_deref().ok_or_else(|| {
                AppError::new("operation_missing", "capture", "过程事件缺少操作身份")
            })?,
        )?,
    };
    if let Some(ref s) = input.source {
        capture::ensure_source(
            &tx,
            input
                .session_id
                .as_deref()
                .ok_or_else(|| AppError::new("session_missing", "capture", "文本流缺少会话"))?,
            s,
            input.observed_at_ms,
        )?;
    }
    type HookEventFacts = (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    );
    let previous:Option<HookEventFacts>=tx.query_row("SELECT operation_id,session_id,source_id,wire_sequence_u64,observed_at_ms,level,stage,code,message,lost_count_u64,integrity FROM hook_events WHERE event_id=?1",[&input.event_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?))).optional().map_err(err)?;
    if let Some(p) = previous {
        let expected = (
            operation,
            input.session_id,
            input.source.map(|s| s.source_id),
            input.wire_sequence,
            input.observed_at_ms,
            input.detail.level,
            input.detail.stage,
            input.detail.code,
            input.detail.message,
            input.lost_count,
            input.integrity,
        );
        if p != expected {
            return Err(AppError::new(
                "event_identity_conflict",
                "capture",
                "过程事件身份被不同事实复用",
            ));
        }
        return Ok(());
    }
    tx.execute("INSERT INTO hook_events(event_id,operation_id,session_id,source_id,wire_sequence_u64,observed_at_ms,level,stage,code,message,lost_count_u64,integrity,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'{}')",params![input.event_id,operation,input.session_id,input.source.map(|s|s.source_id),input.wire_sequence,input.observed_at_ms,input.detail.level,input.detail.stage,input.detail.code,input.detail.message,input.lost_count,input.integrity]).map_err(err)?;
    bump(&tx, input.observed_at_ms)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn record_command_event(
    conn: &Connection,
    command: &Command,
    operation: &str,
    now: i64,
) -> AppResult<()> {
    let (stage, code, message) = match command {
        Command::SelectSource { .. } => ("selection", "source_selected", "正文文本流选择已变更"),
        Command::SetCapturePaused { paused: true } => (
            "capture",
            "capture_paused",
            "正文消费已暂停，已收到的原始回调继续保存",
        ),
        Command::SetCapturePaused { paused: false } => (
            "capture",
            "capture_resumed",
            "正文消费已恢复，不补造暂停期间正文",
        ),
        Command::Disconnect => (
            "disconnect",
            "disconnect_requested",
            "用户已请求断开，历史记录保留",
        ),
        Command::UnbindWindow(_) => (
            "association",
            "window_unlinked",
            "窗口关联已解除，文本接收继续保留",
        ),
        _ => return Ok(()),
    };
    let source = if let Command::SelectSource { source_id } = command {
        Some(source_id.clone())
    } else {
        None
    };
    let session: Option<String> = if let Command::UnbindWindow(request) = command {
        Some(request.expected_session_id.clone())
    } else if let Some(source) = &source {
        conn.query_row(
            "SELECT session_id FROM capture_sources WHERE source_id=?1",
            [source],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
    } else {
        conn.query_row("SELECT session_id FROM sessions WHERE source_kind='hook' ORDER BY started_at_ms DESC,session_id DESC LIMIT 1",[],|r|r.get(0)).optional().map_err(err)?
    };
    if let Some(session) = session {
        let owner = operation_for_session(conn, &session)?;
        conn.execute("INSERT INTO hook_events(event_id,operation_id,session_id,source_id,observed_at_ms,level,stage,code,message,integrity,metadata_json) VALUES(?1,?2,?3,?4,?5,'info',?6,?7,?8,'intact','{}')",params![format!("command-hook:{operation}"),owner,session,source,now,stage,code,message]).map_err(err)?;
        if matches!(command, Command::Disconnect) {
            conn.execute("UPDATE capture_operations SET state='stopped',ended_at_ms=?1 WHERE session_id=?2 AND state='active'",params![now,session]).map_err(err)?;
        }
    }
    Ok(())
}
