//! Explicit maintenance policies. No default destructive retention is inferred from UI state.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicy {
    pub max_rows_per_batch: u32,
    pub raw_payload_before_ms: Option<i64>,
    pub raw_payload_byte_limit: Option<u64>,
    pub cache_byte_limit: Option<u64>,
    pub remove_expired_cache: bool,
    pub history_unit_ids: Vec<String>,
    pub config_revision_ids: Vec<String>,
    pub credential_pool_versions: Vec<(String, u32)>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedObjects {
    pub unit_ids: Vec<String>,
    pub config_revision_ids: Vec<String>,
    pub credential_pool_versions: Vec<(String, u32)>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceReceipt {
    pub more_work: bool,
    pub operation_id: String,
    pub commit_revision: u32,
    pub removed_raw_payloads: u32,
    pub removed_cache_entries: u32,
    pub removed_history_units: u32,
    pub collected_config_snapshots: u32,
    pub collected_pool_snapshots: u32,
    pub replayed: bool,
}
