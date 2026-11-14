//! Bounded, profile-owned cover references. Filesystem paths never cross this contract.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameCoverOrigin {
    SteamLibrary,
    GameResource,
    SessionFrame,
}
impl GameCoverOrigin {
    pub fn key(self) -> &'static str {
        match self {
            Self::SteamLibrary => "steam_library",
            Self::GameResource => "game_resource",
            Self::SessionFrame => "session_frame",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameCoverStatus {
    Ready,
    Missing,
    Unavailable,
    ReferenceChanged,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameCoverRequest {
    pub game_id: String,
    pub expected_asset_ref: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameCoverResult {
    pub game_id: String,
    pub asset_ref: Option<String>,
    pub status: GameCoverStatus,
    pub origin: Option<GameCoverOrigin>,
    pub content_hash: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mime_type: Option<String>,
    pub payload: Option<Vec<u8>>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone)]
pub struct PreparedGameCover {
    pub asset_ref: String,
    pub origin: GameCoverOrigin,
    pub content_hash: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone)]
pub struct GameCoverBytes {
    pub prepared: PreparedGameCover,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone)]
pub struct CoverSessionGuard {
    pub session_id: String,
    pub process_identity_sha256: String,
    pub binding_identity_sha256: String,
}
#[derive(Debug, Clone)]
pub struct CoverCommitGuard {
    pub profile_id: String,
    pub profile_generation: u32,
    pub server_instance_id: String,
    pub game_id: String,
    pub row_revision: u32,
    pub environment_key: String,
    pub target_path_key: Option<String>,
    pub session: Option<CoverSessionGuard>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverInstallOutcome {
    Installed,
    AlreadyPresent,
    RevisionConflict,
    StaleSession,
    TargetChanged,
    Archived,
}
#[derive(Debug, Clone)]
pub struct PreparedSessionCover {
    pub guard: CoverCommitGuard,
    pub cover: PreparedGameCover,
}
