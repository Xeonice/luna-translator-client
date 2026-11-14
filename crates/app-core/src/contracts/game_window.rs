//! Receipts for the current verified association, never caller-provided HWNDs.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameWindowActivationRequest {
    pub expected_session_id: String,
    /// Full bound-window identity (process + HWND/class), not the DB process hash.
    pub expected_target_identity_sha256: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameWindowBindingState {
    Bound,
    Unbound,
    NoWindow,
    Stale,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameWindowBindingProjection {
    pub state: GameWindowBindingState,
    pub session_id: Option<String>,
    /// Includes HWND/class. Busy may retain a trusted cached binding identity;
    /// that is not a fresh verification and verified_at_ms remains None.
    pub target_identity_sha256: Option<String>,
    pub window_title: Option<String>,
    pub verified_at_ms: Option<String>,
    pub error_code: Option<String>,
    pub detail: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameWindowActivationState {
    Activated,
    VisibleNotForeground,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameWindowActivationReceipt {
    pub session_id: String,
    /// Same full bound-window identity frozen in the activation request.
    pub target_identity_sha256: String,
    pub state: GameWindowActivationState,
    pub was_minimized: bool,
    pub show_was_visible: bool,
    pub set_foreground_accepted: bool,
    pub visible_after: bool,
    pub foreground_after: bool,
    pub observed_at_ms: String,
}
