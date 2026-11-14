#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bootstrap;
mod ipc;
mod platform;
mod visual_verification;
use ipc::{commands::*, game_assets::*, queries::*, reading::*, service_configuration::*, google_authorization::*, subscriptions::*, word_card::*};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use tauri::Manager;

#[derive(Debug, PartialEq, Eq)]
enum QuitDecision {
    BeginCleanup,
    AlreadyClosing,
    AllowExit,
}
#[derive(Default)]
struct QuitCoordinator {
    // 0 = running, 1 = cleanup in progress, 2 = durable cleanup completed.
    phase: AtomicU8,
}
impl QuitCoordinator {
    fn request(&self) -> QuitDecision {
        match self
            .phase
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => QuitDecision::BeginCleanup,
            Err(2) => QuitDecision::AllowExit,
            Err(_) => QuitDecision::AlreadyClosing,
        }
    }
    fn ready(&self) {
        self.phase.store(2, Ordering::Release);
    }
    fn retry(&self) {
        self.phase.store(0, Ordering::Release);
    }
}
async fn reject_quit(
    app: &tauri::AppHandle,
    quit: Arc<QuitCoordinator>,
    runtime: Arc<app_core::application::runtime::ApplicationRuntime>,
) {
    #[cfg(target_os = "macos")]
    {
        let retry = quit.clone();
        if let Err(error) =
            platform::macos::reply_pending_quit(app, false, move || retry.retry()).await
        {
            quit.retry();
            report_menu_failure(runtime, error);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, runtime);
        quit.retry();
    }
}
fn report_menu_failure(
    runtime: Arc<app_core::application::runtime::ApplicationRuntime>,
    error: app_core::contracts::AppError,
) {
    // Diagnostics use the SQLite actor. Never make the native menu callback
    // wait for that actor while reporting a rejected or busy window action.
    tauri::async_runtime::spawn_blocking(move || runtime.report_native_effect_failure(error));
}
fn main() {
    let visual_request = match visual_verification::request() {
        Ok(request) => request,
        Err(error) => {
            eprintln!("General visual verification rejected: {error}");
            std::process::exit(64);
        }
    };
    #[cfg(target_os = "macos")]
    if visual_request.is_none() {
        if let Some(code) = platform::window_collector::dispatch_worker() {
            std::process::exit(code);
        }
    }
    let context = tauri::generate_context!();
    if let Some(request) = visual_request {
        std::process::exit(visual_verification::run_request(request, context));
    }
    let app = tauri::Builder::default()
        .setup(|app| {
            bootstrap::setup(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let role = match event.id().as_ref() {
                "main" => Some(app_core::contracts::WindowRole::Main),
                "hud" => Some(app_core::contracts::WindowRole::Hud),
                "settings" => Some(app_core::contracts::WindowRole::Settings),
                "reading_toggle_hud" | "reading_toggle_pause" | "reading_cycle_display" => {
                    use app_core::contracts::ShortcutIntent;
                    let intent = match event.id().as_ref() {
                        "reading_toggle_hud" => ShortcutIntent::ToggleHud,
                        "reading_toggle_pause" => ShortcutIntent::ToggleCapturePause,
                        _ => ShortcutIntent::CycleReadingDisplayMode,
                    };
                    if let Err(error) = app
                        .state::<Arc<platform::shortcuts::ShortcutCoordinator>>()
                        .trigger_action(intent)
                    {
                        report_menu_failure(
                            app.state::<bootstrap::DesktopState>().runtime.clone(),
                            error,
                        );
                    }
                    None
                }
                "show_game" => {
                    let runtime = app.state::<bootstrap::DesktopState>().runtime.clone();
                    // Freeze the trusted association at the user's click. A
                    // queued effect must never raise a later association.
                    let request = match runtime.game_window_activation_request() {
                        Ok(request) => request,
                        Err(error) => {
                            report_menu_failure(runtime, error);
                            return;
                        }
                    };
                    tauri::async_runtime::spawn(async move {
                        let command_runtime = runtime.clone();
                        let result =
                            ipc::blocking(move || command_runtime.show_associated_game(request))
                                .await;
                        match result {
                            Ok(receipt) if receipt.foreground_after => {}
                            Ok(_) => report_menu_failure(
                                runtime,
                                app_core::contracts::AppError::new(
                                    "game_not_foreground",
                                    "desktop",
                                    "游戏窗口已显示，系统未允许切到前台；请直接点击游戏窗口",
                                ),
                            ),
                            Err(error) => report_menu_failure(runtime, error),
                        }
                    });
                    None
                }
                "hud_reset" => {
                    if let Err(error) =
                        platform::window_manager::show(app, app_core::contracts::WindowRole::Hud)
                            .and_then(|_| {
                                app.state::<Arc<platform::window_manager::WindowCoordinator>>()
                                    .reset_hud_position()
                                    .map(|_| ())
                            })
                    {
                        report_menu_failure(
                            app.state::<bootstrap::DesktopState>().runtime.clone(),
                            error,
                        );
                    }
                    None
                }
                "quit" => {
                    app.exit(0);
                    None
                }
                _ => None,
            };
            if let Some(role) = role {
                let _ = platform::window_manager::show(app, role);
            }
        })
        .on_window_event(|window, event| {
            let decision = if let Some(coordinator) = window
                .app_handle()
                .try_state::<Arc<platform::window_manager::WindowCoordinator>>()
            {
                coordinator.on_window_event(window, event)
            } else {
                platform::window_manager::OwnerGeometryDecision {
                    invalidation: None,
                    classification: "coordinator_unavailable",
                }
            };
            ipc::word_card::on_window_event(window, event, &decision);
            if window.label().starts_with("word-card-") {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "main" {
                    ipc::reading::stop_candidate_window_previews(window.app_handle());
                }
                if window.label() == "settings" {
                    let _ = window
                        .app_handle()
                        .state::<Arc<platform::shortcuts::ShortcutCoordinator>>()
                        .suppress_dispatch(false);
                }
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            application_game_cover,
            application_preview_session,
            application_word_card_open,
            application_word_card_snapshot,
            application_word_card_lookup,
            application_word_card_close,
            application_word_card_action,
            application_snapshot,
            application_service_catalog,
            application_service_form,
            application_service_configuration,
            application_google_authorization_inspect,
            application_google_authorization_start,
            application_google_authorization_status,
            application_google_authorization_cancel,
            application_google_authorization_revoke,
            application_service_draft,
            application_save_service_draft,
            application_saved_services,
            application_select_saved_services,
            application_restore_service_inheritance,
            application_set_service_purpose_enabled,
            application_store_service_credential,
            application_login_startup,
            application_set_login_startup,
            application_dispatch,
            application_subscribe,
            application_unsubscribe,
            application_targets,
            application_open_main_page,
            application_main_page,
            application_open_settings_page,
            application_settings_page,
            application_pick_window,
            application_associate,
            application_review_association,
            application_associate_reviewed,
            application_association_progress,
            application_source_selection,
            application_begin_reading,
            application_retry_reading_window_effects,
            application_window,
            application_pick_executables,
            application_inspect_executables,
            application_lookup,
            application_study,
            application_test_configuration,
            application_test_saved_service,
            application_cancel_saved_service_test,
            application_store_credential,
            application_credential_revocation_impact,
            application_revoke_credential,
            application_resources,
            application_learning_object,
            application_history,
            application_history_page,
            application_history_sessions,
            application_reading_unit,
            application_reading_unit_projection,
            application_retranslate,
            application_window_capabilities,
            application_preferences_scope,
            application_game_window_binding,
            application_show_associated_game,
            application_begin_hud_drag,
            application_fit_hud_content,
            application_reset_hud_position,
            application_shortcut_recording,
            application_preview_capability,
            application_preview_start,
            observed_preview_select_window,
            application_preview_control,
            application_preview_latest,
            application_session_records,
            application_session_export_prepare,
            application_session_export_confirm,
            application_session_export_cancel,
            application_cycle_reading_display_mode,
            application_copy_reading_unit,
            application_retry_lexical
        ])
        .build(context)
        .expect("无法启动Luna，请保留原数据并检查诊断");
    #[cfg(target_os = "macos")]
    platform::macos::initialize_quit(app.handle().clone())
        .expect("无法安全协调系统退出，请保留原数据并检查诊断");
    let quit = Arc::new(QuitCoordinator::default());
    app.run(move |app, event| {
        if matches!(event, tauri::RunEvent::Exit)
            && let Some(coordinator) =
                app.try_state::<Arc<platform::window_manager::WindowCoordinator>>()
        {
            // Best-effort diagnostics only, after the actual exit decision.
            // Never wait for file I/O or change the shutdown authority.
            coordinator.request_trace_drain();
        }
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            let _ = platform::window_manager::show(app, app_core::contracts::WindowRole::Main);
        }
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            let decision = quit.request();
            if decision == QuitDecision::AllowExit {
                return;
            }
            api.prevent_exit();
            if decision == QuitDecision::AlreadyClosing {
                return;
            }
            let app = app.clone();
            let quit = quit.clone();
            let runtime = app.state::<bootstrap::DesktopState>().runtime.clone();
            tauri::async_runtime::spawn(async move {
                let cleanup_app = app.clone();
                if let Err(error) =
                    ipc::blocking(move || ipc::reading::cancel_exports_for_shutdown(&cleanup_app))
                        .await
                {
                    runtime.report_native_effect_failure(error);
                    app.state::<bootstrap::DesktopState>()
                        .export_closing
                        .store(false, Ordering::Release);
                    reject_quit(&app, quit, runtime.clone()).await;
                    let _ =
                        platform::window_manager::show(&app, app_core::contracts::WindowRole::Main);
                    return;
                }
                ipc::reading::stop_window_previews(&app);
                match runtime.shutdown().await {
                    Ok(()) => {
                        let preview = app
                            .state::<Arc<platform::window_preview::WindowPreviewService>>()
                            .inner()
                            .clone();
                        if let Err(error) = ipc::blocking(move || preview.stop_all()).await {
                            runtime.report_native_effect_failure(error);
                        }
                        if let Err(error) = app
                            .state::<Arc<platform::window_manager::WindowCoordinator>>()
                            .flush()
                        {
                            runtime.report_native_effect_failure(error);
                        }
                        if let Err(error) = app
                            .state::<Arc<platform::shortcuts::ShortcutCoordinator>>()
                            .shutdown()
                        {
                            runtime.report_native_effect_failure(error);
                        }
                        #[cfg(target_os = "macos")]
                        if let Some(collector) =
                            app.try_state::<Arc<platform::window_collector::CollectorSupervisor>>()
                        {
                            let collector = collector.inner().clone();
                            let draining = collector.clone();
                            if let Err(error) =
                                ipc::blocking(move || draining.shutdown_and_wait()).await
                            {
                                runtime.report_native_effect_failure(error);
                                reject_quit(&app, quit, runtime.clone()).await;
                                return;
                            }
                        }
                        quit.ready();
                        #[cfg(target_os = "macos")]
                        match platform::macos::reply_pending_quit(&app, true, || {}).await {
                            Ok(true) => {}
                            Ok(false) => app.exit(0),
                            Err(error) => {
                                report_menu_failure(runtime.clone(), error);
                                app.state::<bootstrap::DesktopState>()
                                    .export_closing
                                    .store(false, Ordering::Release);
                                reject_quit(&app, quit, runtime.clone()).await;
                                let _ = platform::window_manager::show(
                                    &app,
                                    app_core::contracts::WindowRole::Main,
                                );
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        app.exit(0);
                    }
                    Err(_) => {
                        app.state::<bootstrap::DesktopState>()
                            .export_closing
                            .store(false, Ordering::Release);
                        reject_quit(&app, quit, runtime.clone()).await;
                        let _ = platform::window_manager::show(
                            &app,
                            app_core::contracts::WindowRole::Main,
                        );
                    }
                }
            });
        }
    });
}

#[cfg(test)]
mod quit_tests {
    use super::*;

    #[test]
    fn quit_never_allows_exit_before_cleanup_is_ready() {
        let quit = QuitCoordinator::default();
        assert_eq!(quit.request(), QuitDecision::BeginCleanup);
        for _ in 0..100 {
            assert_eq!(quit.request(), QuitDecision::AlreadyClosing);
        }
        quit.ready();
        assert_eq!(quit.request(), QuitDecision::AllowExit);
        assert_eq!(quit.request(), QuitDecision::AllowExit);
    }

    #[test]
    fn rejected_quit_can_retry_without_reusing_success_authority() {
        let quit = QuitCoordinator::default();
        assert_eq!(quit.request(), QuitDecision::BeginCleanup);
        quit.retry();
        assert_eq!(quit.request(), QuitDecision::BeginCleanup);
        assert_eq!(quit.request(), QuitDecision::AlreadyClosing);
        quit.ready();
        // A failed native reply also revokes the temporary exit authority.
        quit.retry();
        assert_eq!(quit.request(), QuitDecision::BeginCleanup);
        quit.ready();
        assert_eq!(quit.request(), QuitDecision::AllowExit);
    }

    #[test]
    fn simultaneous_menu_and_native_quit_start_only_one_cleanup() {
        let quit = Arc::new(QuitCoordinator::default());
        let barrier = Arc::new(std::sync::Barrier::new(32));
        let workers = (0..32)
            .map(|_| {
                let quit = quit.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    quit.request()
                })
            })
            .collect::<Vec<_>>();
        let decisions = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == QuitDecision::BeginCleanup)
                .count(),
            1
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == QuitDecision::AlreadyClosing)
                .count(),
            31
        );
        quit.ready();
        assert_eq!(quit.request(), QuitDecision::AllowExit);
    }
}
