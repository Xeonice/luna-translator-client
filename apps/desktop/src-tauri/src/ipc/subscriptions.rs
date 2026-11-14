use crate::{bootstrap::DesktopState, ipc::role};
use app_core::contracts::*;
use tauri::{State, WebviewWindow, ipc::Channel};
#[tauri::command]
pub fn application_subscribe(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    channel: Channel<ProjectionUpdate>,
) -> AppResult<String> {
    role(&window, &["main", "settings", "hud", "learning"])?;
    let mut receiver = state.runtime.subscribe();
    let id = format!(
        "{}-{}",
        window.label(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|v| v.as_nanos())
            .unwrap_or(0)
    );
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            let update = receiver.borrow_and_update().clone();
            if channel.send(update).is_err() {
                break;
            }
            if receiver.changed().await.is_err() {
                break;
            }
        }
    });
    state
        .subscriptions
        .lock()
        .map_err(|_| AppError::new("subscriptions", "ipc", "订阅状态不可用"))?
        .insert(id.clone(), (window.label().into(), handle));
    Ok(id)
}
#[tauri::command]
pub fn application_unsubscribe(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    subscription_id: String,
) -> AppResult<()> {
    let mut subscriptions = state
        .subscriptions
        .lock()
        .map_err(|_| AppError::new("subscriptions", "ipc", "订阅状态不可用"))?;
    if subscriptions
        .get(&subscription_id)
        .is_some_and(|(owner, _)| owner != window.label())
    {
        return Err(AppError::new(
            "forbidden_subscription",
            "ipc",
            "不能关闭其他窗口的订阅",
        ));
    }
    if let Some((_, handle)) = subscriptions.remove(&subscription_id) {
        handle.abort();
    }
    Ok(())
}
