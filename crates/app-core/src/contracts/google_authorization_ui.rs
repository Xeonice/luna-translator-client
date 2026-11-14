//! Safe settings projections. Browser intents and credential materials are never IPC values.
use super::{AppError, AppResult};
use crate::domain::model_config::service_graph::GoogleAuthorizationRef;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use ts_rs::TS;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoogleInteractiveClientMode {
    Application,
    Custom,
}
#[derive(Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleInteractiveProxy {
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proxy_url: Option<String>,
}
impl std::fmt::Debug for GoogleInteractiveProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleInteractiveProxy")
            .field("mode", &self.mode)
            .field("proxy_url", &self.proxy_url.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleInteractiveContext {
    pub expected_profile_generation: NonZeroU32,
    pub client_mode: GoogleInteractiveClientMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub public_client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub authorization: Option<GoogleAuthorizationRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proxy: Option<GoogleInteractiveProxy>,
}
impl GoogleInteractiveContext {
    pub fn validate(&self) -> AppResult<()> {
        if let Some(reference) = &self.authorization {
            crate::domain::require_id(&reference.grant_id)?;
        }
        if self
            .public_client_id
            .as_ref()
            .is_some_and(|id| id.len() > 512 || id.bytes().any(|b| b.is_ascii_control()))
        {
            return Err(AppError::new(
                "google_client_invalid",
                "google_interactive",
                "公开客户端ID格式无效",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoogleInteractiveState {
    Unconfigured,
    Ready,
    Authorizing,
    Authorized,
    Revoked,
    NeedsReauth,
    Failed,
    Cancelled,
    TimedOut,
    CleanupPending,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleAuthorizationProjection {
    pub state: GoogleInteractiveState,
    pub can_login: bool,
    pub can_logout: bool,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub authorization: Option<GoogleAuthorizationRef>,
    pub affected_service_count: u32,
}
#[derive(Debug, Clone, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartGoogleLoginInput {
    pub operation_id: String,
    pub context: GoogleInteractiveContext,
}
#[derive(Debug, Clone, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleLoginOperationQuery {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleLoginOperationProjection {
    pub operation_id: String,
    pub state: GoogleInteractiveState,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub authorization: Option<GoogleAuthorizationRef>,
    pub can_cancel: bool,
}
#[derive(Debug, Clone, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeGoogleAuthorizationInput {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
    pub authorization: GoogleAuthorizationRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proxy: Option<GoogleInteractiveProxy>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleRevokeProjection {
    pub local_revoked: bool,
    pub remote_revocation_confirmed: bool,
    pub message: String,
}
pub(super) fn typescript_declarations() -> String {
    [
        GoogleInteractiveProxy::decl(&ts_rs::Config::default()),
        GoogleInteractiveClientMode::decl(&ts_rs::Config::default()),
        GoogleInteractiveContext::decl(&ts_rs::Config::default()),
        GoogleInteractiveState::decl(&ts_rs::Config::default()),
        GoogleAuthorizationProjection::decl(&ts_rs::Config::default()),
        StartGoogleLoginInput::decl(&ts_rs::Config::default()),
        GoogleLoginOperationQuery::decl(&ts_rs::Config::default()),
        GoogleLoginOperationProjection::decl(&ts_rs::Config::default()),
        RevokeGoogleAuthorizationInput::decl(&ts_rs::Config::default()),
        GoogleRevokeProjection::decl(&ts_rs::Config::default()),
    ]
    .map(|v| format!("export {v}\n"))
    .join("\n")
}
