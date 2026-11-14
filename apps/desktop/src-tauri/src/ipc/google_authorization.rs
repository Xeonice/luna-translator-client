//! Settings-only safe Google authorization commands. Core owns every operation.
use crate::{
    bootstrap::DesktopState,
    ipc::{blocking, role},
};
use app_core::contracts::*;
use tauri::{State, WebviewWindow};
#[tauri::command]
pub async fn application_google_authorization_inspect(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    context: GoogleInteractiveContext,
) -> AppResult<GoogleAuthorizationProjection> {
    role(&window, &["settings"])?;
    state.runtime.inspect_google_authorization(context).await
}
#[tauri::command]
pub async fn application_google_authorization_start(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    input: StartGoogleLoginInput,
) -> AppResult<GoogleLoginOperationProjection> {
    role(&window, &["settings"])?;
    state.runtime.start_google_login(input).await
}
#[tauri::command]
pub async fn application_google_authorization_status(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: GoogleLoginOperationQuery,
) -> AppResult<GoogleLoginOperationProjection> {
    role(&window, &["settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.google_login_status(query)).await
}
#[tauri::command]
pub async fn application_google_authorization_cancel(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: GoogleLoginOperationQuery,
) -> AppResult<GoogleLoginOperationProjection> {
    role(&window, &["settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.cancel_google_login(query)).await
}
#[tauri::command]
pub async fn application_google_authorization_revoke(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    input: RevokeGoogleAuthorizationInput,
) -> AppResult<GoogleRevokeProjection> {
    role(&window, &["settings"])?;
    state.runtime.revoke_google_authorization(input).await
}
