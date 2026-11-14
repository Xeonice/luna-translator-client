//! The main UI may read only the current game's opaque managed cover reference.
use crate::{
    bootstrap::DesktopState,
    ipc::{blocking, role},
};
use app_core::contracts::*;
use tauri::{State, WebviewWindow};
#[tauri::command]
pub async fn application_game_cover(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: GameCoverRequest,
) -> AppResult<GameCoverResult> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.game_cover(request)).await
}
