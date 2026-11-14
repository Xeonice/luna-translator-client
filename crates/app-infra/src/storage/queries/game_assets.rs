use super::*;
struct StoredCoverTarget {
    revision: u32,
    reference: Option<String>,
    archived: Option<i64>,
    environment: String,
    path: Option<String>,
}

pub(in crate::storage) fn set_game_cover_if_empty(
    db: &mut Database,
    guard: CoverCommitGuard,
    cover: PreparedGameCover,
    now: i64,
) -> AppResult<CoverInstallOutcome> {
    use CoverInstallOutcome::*;
    let valid_hash = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if !valid_hash(&cover.content_hash)
        || cover.asset_ref != format!("cover:v1:{}:{}", cover.origin.key(), cover.content_hash)
        || cover.width < 160
        || cover.height < 90
        || cover.width > 960
        || cover.height > 960
        || now < 0
    {
        return Err(AppError::new(
            "game_cover_invalid",
            "library",
            "托管封面身份无效",
        ));
    }
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let profile: (String, String, u32, String) = tx.query_row("SELECT profile_id,server_instance_id,runtime_generation,runtime_state FROM profile_meta WHERE singleton=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(err)?;
    if profile.0 != guard.profile_id
        || profile.1 != guard.server_instance_id
        || profile.2 != guard.profile_generation
        || profile.3 != "active"
    {
        return Ok(StaleSession);
    }
    let current: Option<StoredCoverTarget> = tx.query_row("SELECT g.row_revision,g.cover_asset_ref,g.archived_at_ms,t.environment_key,t.target_path_key FROM games g JOIN game_targets t USING(game_id) WHERE g.game_id=?1 AND t.archived_at_ms IS NULL", [&guard.game_id], |r| Ok(StoredCoverTarget { revision:r.get(0)?,reference:r.get(1)?,archived:r.get(2)?,environment:r.get(3)?,path:r.get(4)? })).optional().map_err(err)?;
    let Some(StoredCoverTarget { revision, reference, archived, environment, path }) = current else {
        return Ok(Archived);
    };
    if archived.is_some() {
        return Ok(Archived);
    }
    if reference.is_some() {
        return Ok(AlreadyPresent);
    }
    if revision != guard.row_revision {
        return Ok(RevisionConflict);
    }
    if environment != guard.environment_key || path != guard.target_path_key {
        return Ok(TargetChanged);
    }
    if let Some(session) = &guard.session {
        let matches: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id=?1 AND game_id=?2 AND source_kind='hook' AND state IN ('opening','active') AND environment_key=?3 AND target_identity_sha256=?4)", params![session.session_id,guard.game_id,guard.environment_key,session.process_identity_sha256], |r| r.get(0)).map_err(err)?;
        if !matches || !valid_hash(&session.binding_identity_sha256) {
            return Ok(StaleSession);
        }
    }
    let changed = tx.execute("UPDATE games SET cover_asset_ref=?1,row_revision=row_revision+1,updated_at_ms=?2 WHERE game_id=?3 AND row_revision=?4 AND cover_asset_ref IS NULL AND archived_at_ms IS NULL", params![cover.asset_ref,now,guard.game_id,guard.row_revision]).map_err(err)?;
    if changed != 1 {
        return Ok(RevisionConflict);
    }
    bump(&tx, now)?;
    tx.commit().map_err(err)?;
    Ok(Installed)
}
