//! Authoritative typed session-record contracts. Rust generates the IPC TypeScript types.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionDataset {
    Body,
    Extraction,
    HookEvents,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordOrder {
    #[default]
    OldestFirst,
    NewestFirst,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRecordFilter {
    pub search: Option<String>, // literal Unicode substring over authoritative full dataset
    pub source_id: Option<String>, // extraction only
    pub level: Option<String>,  // hook_events only: info/warning/error
    pub stage: Option<String>,  // hook_events only
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRecordQuery {
    pub session_id: String,
    pub dataset: SessionDataset,
    #[serde(default)]
    pub order: SessionRecordOrder,
    pub filter: SessionRecordFilter,
    pub cursor: Option<String>,
    pub page_size: u32, // default caller50, validated1..200, not dataset cap
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SequenceRange {
    pub first: Option<String>,
    pub last: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompleteness {
    pub status: String, // complete_received_range / incomplete / unknown / cleaned
    pub scope: String,  // always received_range_only
    pub known_loss: bool,
    pub unknown_range: bool,
    pub cleaned_payload_count: String,
    pub notes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionRangeSnapshot {
    pub snapshot_id: String,
    pub refresh_cursor: String, // replay frozen first page / poll newerCount without decoding cursor
    pub high_watermark: String,
    pub first_available: Option<String>,
    pub last_received: Option<String>,
    pub last_persisted: Option<String>,
    pub retained_from: Option<String>,
    pub matching_count: String,
    pub newer_count: String,
    pub database_revision: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BodySourceReference {
    pub event_id: String,
    pub byte_start: u32,
    pub byte_end: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SessionRecord {
    Body {
        sequence: String,
        unit: crate::contracts::ReadingUnit,
        source_refs: Vec<BodySourceReference>,
    },
    Extraction {
        sequence: String,
        event_id: String,
        source_id: String,
        wire_sequence: String,
        text: Option<String>,
        observed_at_ms: f64,
        reason: String,
        body_refs: Vec<BodyRecordReference>,
        payload_retained: bool,
    },
    HookEvent {
        sequence: String,
        event_id: String,
        operation_id: String,
        session_id: Option<String>,
        source_id: Option<String>,
        wire_sequence: Option<String>,
        observed_at_ms: f64,
        level: String,
        stage: String,
        code: String,
        message: String,
        lost_count: Option<String>,
        integrity: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BodyRecordReference {
    pub unit_id: String,
    pub text_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecordPage {
    pub records: Vec<SessionRecord>,
    pub next_cursor: Option<String>,
    pub snapshot: SessionRangeSnapshot,
    pub loaded_range: SequenceRange,
    pub completeness: SessionCompleteness,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionExportRequest {
    pub query: SessionRecordQuery,
    pub privacy_confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionExportPlan {
    pub export_id: String,
    pub session_id: String,
    pub dataset: SessionDataset,
    pub order: SessionRecordOrder,
    pub filter: SessionRecordFilter,
    pub snapshot: SessionRangeSnapshot,
    pub completeness: SessionCompleteness,
    pub contains_game_text: bool,
    pub contains_target_identity: bool,
}
