//! Conservative reference inspection; O3A installs no complete live runtime lease Owner.
use super::*;
#[derive(Debug, Clone)]
pub struct OfflineRetentionReport {
    pub inspected_snapshots: u32,
    pub referenced_snapshots: Vec<String>,
    pub removed_rows: u32,
    pub deletion_blocker: String,
}
pub(super) fn inspect(db: &Database, ids: &[String]) -> AppResult<OfflineRetentionReport> {
    if ids.len() > 128 {
        return Err(fail("offline_retention_budget_exceeded"));
    }
    let mut referenced = Vec::new();
    for id in ids {
        require_id(id)?;
        let used:bool=db.conn.query_row("SELECT EXISTS(SELECT 1 FROM offline_engine_configs WHERE snapshot_id=?1 UNION ALL SELECT 1 FROM offline_translation_route_hops WHERE snapshot_id=?1 UNION ALL SELECT 1 FROM offline_resource_observations WHERE snapshot_id=?1 UNION ALL SELECT 1 FROM task_execution_targets WHERE snapshot_id=?1 UNION ALL SELECT 1 FROM attempt_execution_targets WHERE snapshot_id=?1 UNION ALL SELECT 1 FROM offline_test_targets WHERE snapshot_id=?1)",[id],|r|r.get(0)).map_err(err)?;
        if used {
            referenced.push(id.clone());
        }
    }
    Ok(OfflineRetentionReport {
        inspected_snapshots: ids.len() as u32,
        referenced_snapshots: referenced,
        removed_rows: 0,
        deletion_blocker: "offline_retention_owner_unavailable".into(),
    })
}
