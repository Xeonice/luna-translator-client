//! Native capability receipts are distinct from saved preferences and product snapshots.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! states { ($name:ident { $($value:ident),+ $(,)? }) => {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum $name { $($value),+ }
}; }
states!(ShortcutIntent {
    ToggleHud,
    ToggleCapturePause,
    CycleReadingDisplayMode
});
states!(ShortcutState {
    Draft,
    Validating,
    Registering,
    Active,
    Failed,
    CleanupPending
});
states!(GeometryState {
    Unavailable,
    Restored,
    Moved,
    NotSaved
});
states!(MaterialEffect {
    NativeMaterial,
    OpaqueFallback,
    Unsupported
});
states!(PreviewConsumer { Candidate, Session });
states!(PreviewControl {
    Pause,
    Resume,
    Stop
});
states!(PreviewPermission {
    NotRequested,
    Granted,
    Denied,
    Unavailable
});
states!(PreviewState {
    Off,
    RequestingPermission,
    Starting,
    Live,
    Paused,
    Stale,
    Minimized,
    TargetClosed,
    PermissionDenied,
    PermissionRevoked,
    Unavailable,
    Stopped
});
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBindingProjection {
    pub intent: ShortcutIntent,
    pub desired: Option<String>,
    pub effective: Option<String>,
    pub scope_key: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutProjection {
    pub state: ShortcutState,
    pub registration_generation: String,
    pub bindings: Vec<ShortcutBindingProjection>,
    pub dispatch_suppressed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogicalBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeometryProjection {
    pub state: GeometryState,
    pub logical_bounds: Option<LogicalBounds>,
    pub monitor_id: Option<String>,
    pub scale_factor: Option<f64>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MaterialProjection {
    pub requested: String,
    pub effective: MaterialEffect,
    pub system_reduce_transparency: bool,
    pub effective_reduce_transparency: bool,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowCapabilities {
    pub geometry: GeometryProjection,
    pub material: MaterialProjection,
    pub shortcuts: ShortcutProjection,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewStartRequest {
    pub candidate_id: String,
    pub consumer: PreviewConsumer,
    pub consumer_id: String,
    pub expected_generation: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewControlRequest {
    pub lease_id: String,
    pub generation: String,
    pub action: PreviewControl,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCapability {
    pub supported: bool,
    pub permission: PreviewPermission,
    pub error_code: Option<String>,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProjection {
    pub lease_id: Option<String>,
    pub generation: String,
    pub consumer_id: String,
    pub state: PreviewState,
    pub permission: PreviewPermission,
    pub target_identity_hash: Option<String>,
    pub attempted_at_ms: Option<String>,
    pub captured_at_ms: Option<String>,
    pub content_changed_at_ms: Option<String>,
    pub permission_checked_at_ms: Option<String>,
    pub error_code: Option<String>,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFrame {
    pub lease_id: String,
    pub generation: String,
    pub sequence: String,
    pub target_identity_hash: String,
    pub captured_at_ms: String,
    pub width: u32,
    pub height: u32,
    pub content_hash: String,
    pub encoding: String,
    pub payload: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewUpdate {
    pub projection: PreviewProjection,
    pub frame: Option<PreviewFrame>,
}
