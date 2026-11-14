//! Input validation for the three authoritative session datasets.
use crate::{contracts::*, domain::require_id};
pub fn validate_query(query: &SessionRecordQuery) -> AppResult<()> {
    require_id(&query.session_id)?;
    if !(1..=200).contains(&query.page_size)
        || query
            .filter
            .search
            .as_ref()
            .is_some_and(|s| s.len() > 4096 || s.contains('\0'))
        || query.cursor.as_ref().is_some_and(|s| s.len() > 4096)
    {
        return Err(invalid_query());
    }
    if let Some(source) = &query.filter.source_id {
        require_id(source)?;
    }
    match query.dataset {
        SessionDataset::Body
            if query.filter.source_id.is_some()
                || query.filter.level.is_some()
                || query.filter.stage.is_some() =>
        {
            return Err(invalid_query());
        }
        SessionDataset::Extraction
            if query.filter.level.is_some() || query.filter.stage.is_some() =>
        {
            return Err(invalid_query());
        }
        SessionDataset::HookEvents if query.filter.source_id.is_some() => {
            return Err(invalid_query());
        }
        _ => {}
    }
    if query
        .filter
        .level
        .as_ref()
        .is_some_and(|s| !["info", "warning", "error"].contains(&s.as_str()))
        || query.filter.stage.as_ref().is_some_and(|s| {
            !(valid_stage(s) || query.dataset == SessionDataset::HookEvents && s == "diagnostics")
        })
    {
        return Err(invalid_query());
    }
    Ok(())
}
pub fn valid_stage(stage: &str) -> bool {
    // "diagnostics" is a query-only view, never a stored event stage.
    [
        "association",
        "resource",
        "identity",
        "injection",
        "handshake",
        "stream",
        "selection",
        "capture",
        "disconnect",
        "recovery",
    ]
    .contains(&stage)
}
pub fn invalid_query() -> AppError {
    AppError::new(
        "invalid_session_query",
        "session_records",
        "会话数据集、筛选条件或分页大小无效",
    )
}
