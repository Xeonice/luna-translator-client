use super::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Cursor {
    Units {
        version: u8,
        profile_id: String,
        filter: HistoryFilter,
        boundary: (i64, String, u32),
    },
    Sessions {
        version: u8,
        profile_id: String,
        filter: HistorySessionFilter,
        boundary: (i64, String),
    },
}
const MAX_CURSOR_BYTES: usize = 65_536;
fn decode_cursor(text: &str) -> AppResult<Cursor> {
    if text.len() > MAX_CURSOR_BYTES {
        return Err(invalid_cursor());
    }
    serde_json::from_str(text).map_err(|_| invalid_cursor())
}
fn valid_boundary_id(id: &str) -> bool {
    app_core::domain::require_id(id).is_ok()
}
fn invalid_cursor() -> AppError {
    AppError::new(
        "invalid_cursor",
        "history",
        "历史分页范围已变化，请重新加载记录",
    )
}
fn source(kind: Option<&HistorySourceKind>) -> Option<&'static str> {
    kind.map(|kind| match kind {
        HistorySourceKind::Hook => "hook",
        HistorySourceKind::Manual => "manual",
    })
}
fn profile(conn: &Connection) -> AppResult<String> {
    conn.query_row(
        "SELECT profile_id FROM profile_meta WHERE singleton=1",
        [],
        |row| row.get(0),
    )
    .map_err(err)
}
pub(super) fn page(conn: &Connection, query: HistoryQuery) -> AppResult<HistoryPage> {
    app_core::application::history::validate_filter(&query.filter)?;
    let profile_id = profile(conn)?;
    let boundary = match query.cursor.as_deref() {
        None => None,
        Some(text) => match decode_cursor(text)? {
            Cursor::Units {
                version: 1,
                profile_id: owner,
                filter,
                boundary,
            } if owner == profile_id
                && filter == query.filter
                && boundary.0 >= 0
                && valid_boundary_id(&boundary.1)
                && boundary.2 > 0 =>
            {
                Some(boundary)
            }
            _ => return Err(invalid_cursor()),
        },
    };
    let limit = query.limit.clamp(1, 100);
    let mut stmt = conn.prepare("SELECT r.unit_id,r.text_revision,r.created_at_ms FROM reading_unit_revisions r JOIN sessions s USING(session_id) WHERE (coalesce(json_extract(r.processing_snapshot_json,'$.unitProjection'),'all')<>'latest' OR r.text_revision=(SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=r.unit_id)) AND (?1 IS NULL OR s.game_id=?1) AND (?2 IS NULL OR s.source_kind=?2) AND (?3 IS NULL OR s.session_id=?3) AND (?4 IS NULL OR (r.created_at_ms,r.unit_id,r.text_revision)<(?4,?5,?6)) ORDER BY r.created_at_ms DESC,r.unit_id DESC,r.text_revision DESC LIMIT ?7").map_err(err)?;
    let rows: Vec<(String, u32, i64)> = stmt
        .query_map(
            params![
                query.filter.game_id,
                source(query.filter.source_kind.as_ref()),
                query.filter.session_id,
                boundary.as_ref().map(|v| v.0),
                boundary.as_ref().map(|v| v.1.as_str()),
                boundary.as_ref().map(|v| v.2),
                limit + 1
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    let has_more = rows.len() > limit as usize;
    let rows = &rows[..rows.len().min(limit as usize)];
    let units = rows
        .iter()
        .map(|(id, revision, _)| unit(conn, id, *revision))
        .collect::<AppResult<Vec<_>>>()?;
    let next_cursor = if has_more {
        rows.last()
            .map(|(id, revision, time)| {
                json(&Cursor::Units {
                    version: 1,
                    profile_id,
                    filter: query.filter,
                    boundary: (*time, id.clone(), *revision),
                })
            })
            .transpose()?
    } else {
        None
    };
    Ok(HistoryPage { units, next_cursor })
}
/// Both record details and the bounded session chooser read the same authoritative row.
pub(super) fn session_summary(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<HistorySessionSummary> {
    let kind: String = row.get(offset + 2)?;
    let kind = match kind.as_str() {
        "hook" => HistorySourceKind::Hook,
        "manual" => HistorySourceKind::Manual,
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                offset + 2,
                "source_kind".into(),
                rusqlite::types::Type::Text,
            ));
        }
    };
    Ok(HistorySessionSummary {
        session_id: row.get(offset)?,
        game_id: row.get(offset + 1)?,
        source_kind: kind,
        state: row.get(offset + 3)?,
        started_at_ms: row.get::<_, i64>(offset + 4)? as f64,
        ended_at_ms: row.get::<_, Option<i64>>(offset + 5)?.map(|v| v as f64),
    })
}
pub(super) fn sessions(
    conn: &Connection,
    query: HistorySessionsQuery,
) -> AppResult<HistorySessionsPage> {
    app_core::application::history::validate_session_filter(&query.filter)?;
    let profile_id = profile(conn)?;
    let boundary = match query.cursor.as_deref() {
        None => None,
        Some(text) => match decode_cursor(text)? {
            Cursor::Sessions {
                version: 1,
                profile_id: owner,
                filter,
                boundary,
            } if owner == profile_id
                && filter == query.filter
                && boundary.0 >= 0
                && valid_boundary_id(&boundary.1) =>
            {
                Some(boundary)
            }
            _ => return Err(invalid_cursor()),
        },
    };
    let limit = query.limit.clamp(1, 100);
    let mut stmt = conn.prepare("SELECT s.session_id,s.game_id,s.source_kind,s.state,s.started_at_ms,s.ended_at_ms FROM sessions s WHERE (?1 IS NULL OR s.game_id=?1) AND (?2 IS NULL OR s.source_kind=?2) AND EXISTS(SELECT 1 FROM reading_units u WHERE u.session_id=s.session_id) AND (?3 IS NULL OR (s.started_at_ms,s.session_id)<(?3,?4)) ORDER BY s.started_at_ms DESC,s.session_id DESC LIMIT ?5").map_err(err)?;
    let rows: Vec<(HistorySessionSummary, i64)> = stmt
        .query_map(
            params![
                query.filter.game_id,
                source(query.filter.source_kind.as_ref()),
                boundary.as_ref().map(|v| v.0),
                boundary.as_ref().map(|v| v.1.as_str()),
                limit + 1
            ],
            |row| Ok((session_summary(row, 0)?, row.get::<_, i64>(4)?)),
        )
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    let has_more = rows.len() > limit as usize;
    let rows = &rows[..rows.len().min(limit as usize)];
    let next_cursor = if has_more {
        rows.last()
            .map(|(session, time)| {
                json(&Cursor::Sessions {
                    version: 1,
                    profile_id,
                    filter: query.filter,
                    boundary: (*time, session.session_id.clone()),
                })
            })
            .transpose()?
    } else {
        None
    };
    Ok(HistorySessionsPage {
        sessions: rows.iter().map(|(session, _)| session.clone()).collect(),
        next_cursor,
    })
}
