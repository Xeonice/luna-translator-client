//! Narrow native effects and bounded application queries. Business rules remain in app-core.
use crate::{
    bootstrap::{DesktopState, PendingExport},
    ipc::{blocking, role},
    platform::{
        shortcuts::ShortcutCoordinator,
        window_manager::{self, WindowCoordinator},
        window_preview::WindowPreviewService,
    },
};
use app_core::{contracts::*, ports::IdentitySource};
use app_infra::platform::SystemIdentity;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, State, WebviewWindow};

fn state_error() -> AppError {
    AppError::new("desktop_state", "desktop", "窗口状态暂不可用，请重试")
}
pub fn sync_native_preferences(app: &AppHandle) -> AppResult<Preferences> {
    let state = app.state::<DesktopState>();
    let preferences = state.runtime.snapshot()?.preferences;
    let shortcuts = app.state::<Arc<ShortcutCoordinator>>();
    let token = shortcuts.prepare(&preferences.shortcuts, &preferences.scope_key, None)?;
    if let Err(error) = settle_native_shortcuts(&shortcuts, &token, true) {
        if let Err(cleanup) = settle_native_shortcuts(&shortcuts, &token, false)
            && cleanup.code != "shortcut_transaction"
        {
            state.runtime.report_native_effect_failure(cleanup);
        }
        return Err(error);
    }
    crate::platform::menus::sync_shortcuts(app, &shortcuts.projection()?)?;
    window_manager::sync_hud_preferences(app, &preferences)?;
    let _ = window_manager::material_projection(app, &preferences);
    Ok(preferences)
}
/// Runs on the blocking executor. Never call a retrying AppKit effect from the
/// native main loop: a concurrent owner may itself be waiting for that loop.
pub fn settle_native_shortcuts(
    coordinator: &ShortcutCoordinator,
    token: &str,
    commit: bool,
) -> AppResult<ShortcutProjection> {
    loop {
        let result = if commit {
            coordinator.commit(token)
        } else {
            coordinator.rollback(token)
        };
        match result {
            Err(error) if error.code == "shortcut_apply_in_progress" => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            result => return result,
        }
    }
}
pub fn stop_window_previews(app: &AppHandle) {
    app.state::<DesktopState>().session_previews.stop_all(None);
}
/// Hiding the main window releases only candidate preview ownership.
pub fn stop_candidate_window_previews(app: &AppHandle) {
    std::mem::drop(
        app.state::<DesktopState>()
            .session_previews
            .stop_candidates(None),
    );
}
pub fn begin_association_preview(app: &AppHandle) -> AppResult<u64> {
    app.state::<DesktopState>()
        .session_previews
        .begin_association()
}
pub fn stop_association_candidate_previews(app: &AppHandle, epoch: u64) {
    std::mem::drop(
        app.state::<DesktopState>()
            .session_previews
            .stop_candidates(Some(epoch)),
    );
}
pub async fn transfer_candidate_window_preview(
    app: AppHandle,
    candidate: String,
    expected_epoch: u64,
) -> AppResult<()> {
    let coordinator = app.state::<DesktopState>().session_previews.clone();
    coordinator
        .transfer_candidate(candidate, expected_epoch)
        .await
}
pub fn stop_window_previews_if_epoch(app: &AppHandle, epoch: u64) {
    app.state::<DesktopState>()
        .session_previews
        .stop_all(Some(epoch));
}
pub fn cancel_exports_for_shutdown(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<DesktopState>();
    let prepared = {
        let mut entries = state.pending_exports.lock().map_err(|_| state_error())?;
        state.export_closing.store(true, Ordering::Release);
        for entry in entries.values() {
            entry.cancelled.request_cancel();
        }
        let prepared: Vec<_> = entries
            .iter()
            .filter(|(_, entry)| !entry.confirming)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &prepared {
            if let Some(entry) = entries.get_mut(id) {
                entry.confirming = true;
            }
        }
        prepared
    };
    for id in prepared {
        if let Err(error) = state.runtime.cancel_session_export(id.clone()) {
            if let Some(entry) = state
                .pending_exports
                .lock()
                .map_err(|_| state_error())?
                .get_mut(&id)
            {
                entry.confirming = false;
            }
            return Err(error);
        }
        state
            .pending_exports
            .lock()
            .map_err(|_| state_error())?
            .remove(&id);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !state
        .pending_exports
        .lock()
        .map_err(|_| state_error())?
        .is_empty()
    {
        if std::time::Instant::now() >= deadline {
            return Err(AppError::new(
                "export_cleanup_pending",
                "session_export",
                "请先取消保存对话框或等待导出清理，再退出 Luna",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    Ok(())
}
#[tauri::command]
pub async fn application_preferences_scope(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    scope_key: String,
) -> AppResult<ScopedPreferences> {
    role(&window, &["main", "settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.preferences_scope(scope_key)).await
}
#[tauri::command]
pub async fn application_game_window_binding(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> AppResult<app_core::contracts::GameWindowBindingProjection> {
    role(&window, &["main"])?;
    let _ = app;
    state.session_previews.binding_projection()
}
#[tauri::command]
pub async fn application_show_associated_game(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: GameWindowActivationRequest,
) -> AppResult<GameWindowActivationReceipt> {
    role(&window, &["main", "hud"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.show_associated_game(request)).await
}
#[tauri::command]
pub async fn application_window_capabilities(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> AppResult<WindowCapabilities> {
    role(&window, &["main", "hud", "settings"])?;
    let runtime = state.runtime.clone();
    let preferences = blocking(move || Ok(runtime.snapshot()?.preferences)).await?;
    Ok(WindowCapabilities {
        geometry: app.state::<Arc<WindowCoordinator>>().geometry_projection(),
        material: window_manager::material_projection(&app, &preferences),
        shortcuts: app.state::<Arc<ShortcutCoordinator>>().projection()?,
    })
}
#[tauri::command]
pub fn application_begin_hud_drag(window: WebviewWindow, app: AppHandle) -> AppResult<()> {
    role(&window, &["hud"])?;
    app.state::<Arc<WindowCoordinator>>()
        .begin_hud_drag(window.label())
}
#[tauri::command]
pub async fn application_fit_hud_content(
    window: WebviewWindow,
    app: AppHandle,
    height: f64,
) -> AppResult<GeometryProjection> {
    role(&window, &["hud"])?;
    let coordinator = app.state::<Arc<WindowCoordinator>>().inner().clone();
    coordinator.fit_hud_content(window.label(), height).await
}
#[tauri::command]
pub fn application_reset_hud_position(
    window: WebviewWindow,
    app: AppHandle,
) -> AppResult<GeometryProjection> {
    role(&window, &["main", "hud", "settings"])?;
    app.state::<Arc<WindowCoordinator>>().reset_hud_position()
}
#[tauri::command]
pub fn application_shortcut_recording(
    window: WebviewWindow,
    app: AppHandle,
    recording: bool,
) -> AppResult<ShortcutProjection> {
    role(&window, &["settings"])?;
    app.state::<Arc<ShortcutCoordinator>>()
        .suppress_dispatch(recording)
}
#[tauri::command]
pub fn application_preview_capability(
    window: WebviewWindow,
    app: AppHandle,
) -> AppResult<PreviewCapability> {
    role(&window, &["main"])?;
    Ok(app.state::<Arc<WindowPreviewService>>().capability())
}
#[tauri::command]
pub async fn application_preview_start(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: PreviewStartRequest,
) -> AppResult<PreviewProjection> {
    role(&window, &["main"])?;
    let _ = app;
    state.session_previews.start(request).await
}
#[tauri::command]
pub async fn application_preview_select_window(
    window: WebviewWindow,
    app: AppHandle,
    request: PreviewStartRequest,
) -> AppResult<PreviewProjection> {
    role(&window, &["main"])?;
    let coordinator = app.state::<DesktopState>().session_previews.clone();
    coordinator.select(request).await
}
pub async fn ensure_associated_preview(app: AppHandle) -> AppResult<()> {
    let coordinator = app.state::<DesktopState>().session_previews.clone();
    coordinator.ensure_associated().await
}
#[tauri::command]
pub fn application_preview_session(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> AppResult<PreviewProjection> {
    role(&window, &["main"])?;
    let _ = app;
    state.session_previews.session_projection()
}

/// Native/application lifecycle owns session capture. UI attaches through latest-frame queries only.
#[cfg(test)]
pub(crate) use crate::platform::session_preview_coordinator::automatic_permission_resume_request;
#[tauri::command]
pub async fn application_preview_control(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: PreviewControlRequest,
) -> AppResult<PreviewProjection> {
    role(&window, &["main"])?;
    let _ = app;
    state.session_previews.control(request).await
}
#[tauri::command]
pub fn application_preview_latest(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    lease_id: String,
    generation: String,
    after_sequence: Option<String>,
) -> AppResult<PreviewUpdate> {
    role(&window, &["main"])?;
    let _ = app;
    let result = state
        .session_previews
        .latest(&lease_id, &generation, after_sequence.as_deref());
    if let Ok(update) = &result {
        crate::platform::runtime_observation::record(
            crate::platform::runtime_observation::Fact::delivered(
                window.label(),
                &lease_id,
                &generation,
                after_sequence.as_deref(),
                update,
            ),
        );
    }
    result
}
#[tauri::command]
pub async fn application_session_records(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: SessionRecordQuery,
) -> AppResult<SessionRecordPage> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.session_records(query)).await
}
#[tauri::command]
pub async fn application_session_export_prepare(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: SessionExportRequest,
) -> AppResult<SessionExportPlan> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    let plan = blocking(move || runtime.prepare_session_export(request)).await?;
    let mut pending = state.pending_exports.lock().map_err(|_| state_error())?;
    if state.export_closing.load(Ordering::Acquire) || pending.len() >= 4 {
        drop(pending);
        state
            .runtime
            .cancel_session_export(plan.export_id.clone())?;
        return Err(AppError::new(
            "export_limit",
            "session_export",
            "请先完成或取消其他导出",
        ));
    }
    pending.insert(
        plan.export_id.clone(),
        PendingExport {
            plan: plan.clone(),
            confirming: false,
            cancelled: Arc::new(app_infra::platform::session_export::ExportControl::default()),
        },
    );
    Ok(plan)
}
#[tauri::command]
pub async fn application_session_export_confirm(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    export_id: String,
) -> AppResult<SessionExportReceipt> {
    role(&window, &["main"])?;
    let (plan, cancelled) = {
        let mut pending = state.pending_exports.lock().map_err(|_| state_error())?;
        if state.export_closing.load(Ordering::Acquire) {
            return Err(AppError::new(
                "export_closing",
                "session_export",
                "Luna正在退出，请稍后重试",
            ));
        }
        let entry = pending.get_mut(&export_id).ok_or_else(|| {
            AppError::new(
                "export_expired",
                "session_export",
                "导出确认已过期，请重新检查范围",
            )
        })?;
        if entry.confirming {
            return Err(AppError::new(
                "export_in_progress",
                "session_export",
                "该导出正在处理",
            ));
        }
        entry.confirming = true;
        (entry.plan.clone(), entry.cancelled.clone())
    };
    let runtime = state.runtime.clone();
    let id = export_id.clone();
    let result = blocking(move || {
        #[cfg(target_os = "macos")]
        let selected =
            crate::platform::macos::choose_export_path(&format!("Luna-{}.jsonl", plan.export_id))?;
        #[cfg(not(target_os = "macos"))]
        let selected: Option<std::path::PathBuf> = return Err(AppError::new(
            "export_unsupported",
            "session_export",
            "当前平台尚不支持原生导出",
        ));
        if let Some(path) = selected {
            app_infra::platform::session_export::deliver(&runtime, &plan, &path, &cancelled)
        } else {
            Ok(SessionExportReceipt {
                export_id: plan.export_id,
                status: SessionExportStatus::Cancelled,
                record_count: "0".into(),
                destination_label: None,
                content_sha256: None,
            })
        }
    })
    .await;
    // Release the frozen GC pin after delivery, cancellation or failure, never a fake success on cleanup failure.
    let cleanup = state.runtime.finish_session_export(id.clone());
    let mut entries = state.pending_exports.lock().map_err(|_| state_error())?;
    if cleanup.is_ok() {
        entries.remove(&id);
    } else if let Some(entry) = entries.get_mut(&id) {
        entry.confirming = false;
    }
    match (result, cleanup) {
        (Ok(receipt), Ok(())) => Ok(receipt),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}
#[tauri::command]
pub async fn application_session_export_cancel(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    export_id: String,
) -> AppResult<()> {
    role(&window, &["main"])?;
    let confirming = {
        let mut entries = state.pending_exports.lock().map_err(|_| state_error())?;
        if let Some(entry) = entries.get_mut(&export_id) {
            let cancelled = entry.cancelled.request_cancel();
            if entry.confirming {
                if !cancelled {
                    return Err(AppError::new(
                        "export_delivery_in_progress",
                        "session_export",
                        "文件正在原子交付，请等待完成结果",
                    ));
                }
                true
            } else {
                entry.confirming = true;
                false
            }
        } else {
            false
        }
    };
    if confirming {
        return Ok(());
    }
    let runtime = state.runtime.clone();
    let id = export_id.clone();
    let result = blocking(move || runtime.cancel_session_export(export_id)).await;
    let mut entries = state.pending_exports.lock().map_err(|_| state_error())?;
    if result.is_ok() {
        entries.remove(&id);
    } else if let Some(entry) = entries.get_mut(&id) {
        entry.confirming = false;
    }
    result
}
pub async fn cycle_display(state: &DesktopState) -> AppResult<()> {
    state
        .runtime
        .dispatch(CommandEnvelope {
            operation_id: SystemIdentity.new_id("display"),
            expected_revision: None,
            command: Command::CycleReadingDisplayMode,
        })
        .await
        .map(|_| ())
}
#[tauri::command]
pub async fn application_cycle_reading_display_mode(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<()> {
    role(&window, &["main", "hud", "settings"])?;
    cycle_display(&state).await
}
#[tauri::command]
pub async fn application_copy_reading_unit(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
) -> AppResult<()> {
    role(&window, &["main", "hud"])?;
    let runtime = state.runtime.clone();
    blocking(move || {
        let unit = runtime.unit(unit_id, text_revision)?;
        #[cfg(target_os = "macos")]
        {
            crate::platform::macos::write_clipboard(&unit.source_text)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = unit;
            Err(AppError::new(
                "clipboard_unsupported",
                "desktop",
                "当前平台尚不支持该复制入口",
            ))
        }
    })
    .await
}
#[tauri::command]
pub async fn application_retry_lexical(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
) -> AppResult<()> {
    role(&window, &["main", "hud", "learning"])?;
    state.runtime.retry_lexical(unit_id, text_revision).await
}
