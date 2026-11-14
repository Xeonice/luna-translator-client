use crate::{contracts::*, domain::require_id};

pub fn validate_filter(filter: &HistoryFilter) -> AppResult<()> {
    if let Some(id) = &filter.game_id {
        require_id(id)?;
    }
    if let Some(id) = &filter.session_id {
        require_id(id)?;
    }
    Ok(())
}

pub fn validate_session_filter(filter: &HistorySessionFilter) -> AppResult<()> {
    if let Some(id) = &filter.game_id {
        require_id(id)?;
    }
    Ok(())
}
