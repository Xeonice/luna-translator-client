//! Non-Serde operation capabilities. No bearer material or browser URL is a settings projection.
use crate::{contracts::*, ports::google_authorization::GoogleBudget};
use std::{future::Future, pin::Pin};
pub type GoogleInteractiveFuture<'a, T> = Pin<Box<dyn Future<Output = AppResult<T>> + Send + 'a>>;
/// Minted by the trusted broker adapter, consumed only by the native browser port.
/// Never place this value in IPC, exports, logs, or diagnostics.
pub struct GoogleSystemBrowserIntent {
    url: String,
}
impl GoogleSystemBrowserIntent {
    pub fn from_broker_url(url: String) -> Self {
        Self { url }
    }
    pub fn native_url(&self) -> &str {
        &self.url
    }
}
impl std::fmt::Debug for GoogleSystemBrowserIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleSystemBrowserIntent([native-only])")
    }
}
pub trait GoogleSystemBrowser: Send + Sync {
    fn open<'a>(
        &'a self,
        intent: &'a GoogleSystemBrowserIntent,
        budget: &'a GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, ()>;
}
/// Success is provisional until the Core terminal fence wins. A cancelled or
/// expired operation must discard this exact new grant before reporting stopped.
pub trait CompletedGoogleLogin: Send {
    fn authorization(&self) -> crate::domain::model_config::service_graph::GoogleAuthorizationRef;
    fn discard(self: Box<Self>) -> GoogleInteractiveFuture<'static, ()>;
}
pub trait OwnedGoogleLogin: Send {
    fn browser_intent(&self) -> GoogleSystemBrowserIntent;
    fn finish(self: Box<Self>) -> GoogleInteractiveFuture<'static, Box<dyn CompletedGoogleLogin>>;
}
pub trait GoogleInteractiveBackend: Send + Sync {
    /// Startup-only inactive material recovery; never grants login authority.
    fn recover_abandoned<'a>(&'a self) -> GoogleInteractiveFuture<'a, u32> {
        Box::pin(async {
            Err(AppError::new(
                "google_recovery_unavailable",
                "google_interactive",
                "授权材料恢复适配器未安装",
            ))
        })
    }

    fn inspect<'a>(
        &'a self,
        context: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection>;
    fn begin<'a>(
        &'a self,
        input: StartGoogleLoginInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>>;
    fn revoke<'a>(
        &'a self,
        input: RevokeGoogleAuthorizationInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection>;
}
/// Actor-proved internal state, never serialized or supplied by a webview.
pub struct GoogleInteractiveStoredState {
    pub frame: super::google_authorization::GoogleRuntimeFrame,
    pub handler: std::sync::Arc<super::google_authorization::GoogleHandlerRegistration>,
    pub grant: Option<super::google_authorization::GoogleOwnedGrant>,
    pub affected_service_count: u32,
}
