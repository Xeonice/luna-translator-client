use app_core::{
    application::runtime::ApplicationRuntime,
    contracts::*,
    ports::{IdentitySource, ProductRepository, runtime::CredentialVault},
};
use app_infra::{
    credentials::SystemVault, hook_backend::CoreHookBackend,
    language_services::OfflineLanguageServices, platform::SystemIdentity,
    providers::gateway::HttpModelGateway, storage::SqliteStore,
};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::{App, Manager};
#[path = "platform/runtime_paths.rs"]
mod runtime_paths;
pub struct PendingExport {
    pub plan: SessionExportPlan,
    pub confirming: bool,
    pub cancelled: Arc<app_infra::platform::session_export::ExportControl>,
}
pub struct DesktopState {
    pub provider_registry: Arc<app_infra::providers::registry::ProviderRegistry>,
    pub main_navigation: Mutex<Option<MainPageRequest>>,
    pub settings_navigation: Mutex<Option<SettingsPageRequest>>,
    pub word_cards: Arc<crate::platform::word_card::WordCards>,
    pub runtime: Arc<ApplicationRuntime>,
    pub hook: Arc<CoreHookBackend>,
    pub subscriptions: Mutex<BTreeMap<String, (String, tauri::async_runtime::JoinHandle<()>)>>,
    pub session_previews:
        Arc<crate::platform::session_preview_coordinator::SessionPreviewCoordinator>,
    pub pending_exports: Mutex<BTreeMap<String, PendingExport>>,
    pub export_closing: Arc<std::sync::atomic::AtomicBool>,
}
fn profile_id(root: &Path) -> AppResult<String> {
    let path = root.join("profiles.json");
    if path.is_file() {
        let bytes = std::fs::read(&path)
            .map_err(|_| AppError::new("profile_index", "storage", "配置档索引无法读取"))?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("profile_index", "storage", "配置档索引无效，原件已保留"))?;
        if value["schemaVersion"] != 1 {
            return Err(AppError::new(
                "profile_index_version",
                "storage",
                "不支持的配置档索引版本，原件已保留",
            ));
        }
        let id = value["activeProfileId"]
            .as_str()
            .filter(|id| {
                !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            .ok_or_else(|| AppError::new("profile_index", "storage", "配置档身份无效"))?;
        return Ok(id.into());
    }
    std::fs::create_dir_all(root)
        .map_err(|_| AppError::new("profile_index", "storage", "无法创建应用数据目录"))?;
    let id = uuid_id();
    let bytes=serde_json::to_vec_pretty(&serde_json::json!({"schemaVersion":1,"activeProfileId":id,"profiles":[{"profileId":id,"displayName":"默认配置档"}]})).map_err(|_|AppError::new("profile_index","storage","无法编码配置档索引"))?;
    let tmp = root.join(format!("profiles-{}.pending", uuid_id()));
    std::fs::write(&tmp, bytes)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .map_err(|_| AppError::new("profile_index", "storage", "无法保存配置档索引"))?;
    Ok(id)
}
fn uuid_id() -> String {
    SystemIdentity.new_id("profile")
}
pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = runtime_paths::RuntimePaths::from_environment(
        app.path().app_data_dir()?,
        app.path().app_log_dir()?,
    )?;
    let root = paths.data_root.clone();
    let resources = app.path().resource_dir()?;
    let profile = profile_id(&root)?;
    paths.bind_profile(&profile)?;
    let clock: Arc<dyn IdentitySource> = Arc::new(SystemIdentity);
    let instance = clock.new_id("server");
    let logs = paths.session_logs_for(&instance)?;
    if let Err(error) = crate::platform::session_preview_coordinator::binding_observation::initialize(
        &logs.with_file_name(format!("binding-journal-{instance}.jsonl")),
        &resources, &profile, &instance,
    ) {
        // Unsealed dev runs may lack package provenance. Observation failures
        // cannot authorize or prevent a product operation and are not a pass.
        eprintln!("Binding observation unavailable: {error}");
    }
    crate::platform::session_preview_coordinator::binding_observation::transition(
        &instance,
        crate::platform::session_preview_coordinator::binding_observation::Context::default(),
    );
    crate::platform::runtime_observation::initialize(
        logs.with_file_name("runtime-observation.jsonl"),
        &instance,
        &app_core::domain::content_hash(profile.as_bytes()),
    );
    let provider_registry = Arc::new(app_infra::providers::register::builtin_registry()?);
    let store = Arc::new(SqliteStore::open(
        root.join("profiles").join(&profile).join("reader.sqlite3"),
        &profile,
        &instance,
        clock.now_ms(),
    )?);
    store.configure_service_directory(provider_registry.clone())?;
    let repository: Arc<dyn ProductRepository> = store.clone();
    paths.write_diagnostic(&profile, &resources, &instance)?;
    let vault: Arc<dyn CredentialVault> = Arc::new(SystemVault::new(&profile));
    let hook = Arc::new(CoreHookBackend::new(
        resources.clone(),
        logs,
        paths.target_locks.clone(),
    ));
    let google_test_operations = Arc::new(app_core::application::google_test_operations::LiveGoogleTestOperations::default());
    let google_authorization = Arc::new(app_infra::providers::google_authorization::GoogleAuthorizationService::new(
        store.google_authorization_repository(google_test_operations.clone()),
        vault.clone(),
        Arc::new(app_infra::providers::google_token::FixedGoogleTokenTransport::new()?),
        Arc::new(app_infra::providers::google_authorization::SystemGoogleClock),
        google_test_operations.clone(),
    ));
    let gateway = Arc::new(HttpModelGateway::with_registry(repository.clone(), vault.clone(), provider_registry.clone())
        .with_google_authorization(google_authorization.clone())?);
    let language = Arc::new(OfflineLanguageServices::new(resources));
    let runtime =
        ApplicationRuntime::new(repository, clock.clone(), hook.clone(), gateway, language, vault.clone())?;
    runtime.configure_service_testing(
        store.service_testing_repository(google_test_operations.clone(), google_test_operations.clone())?,
        google_test_operations.clone(),
    )?;
    runtime.configure_service_configuration(Arc::new(
        app_core::application::service_configuration::ServiceConfigurationController::new(
            store.clone(), provider_registry.clone(), clock.clone(),
        ),
    ))?;
    let google_interactive = Arc::new(app_infra::providers::google_interactive::SqliteGoogleInteractiveBackend::new(store.clone(), google_authorization.clone(), clock.clone(), vault.clone()));
    runtime.configure_google_interactive(Arc::new(app_core::application::google_interactive::GoogleInteractiveController::new(
        google_interactive.clone(),
        Arc::new(crate::platform::system_browser::NativeGoogleSystemBrowser),
    )))?;
    #[cfg(target_os = "macos")]
    match app_infra::platform::game_assets::ManagedGameAssets::new(
        root.join("profiles").join(&profile),
        Arc::new(crate::platform::macos::NativeCoverCodec),
    ) {
        Ok(assets) => runtime.configure_game_assets(Arc::new(assets))?,
        Err(error) => runtime.report_native_effect_failure(error),
    }
    runtime.recover_credentials()?;
    let ready = runtime.clone();
    tauri::async_runtime::block_on(async move { ready.start()?; ready.start_google_material_recovery() })?;
    app.manage(crate::platform::window_manager::WindowCoordinator::new(
        app.handle().clone(),
        root.clone(),
    ));
    let previews = crate::platform::window_preview::WindowPreviewService::new();
    #[cfg(target_os = "macos")]
    {
        let collector =
            crate::platform::window_collector::CollectorSupervisor::new(Arc::downgrade(&previews));
        previews.install_collector(collector.clone())?;
        let proof: Arc<dyn app_infra::hook_backend::PreviewProofProvider> = collector.clone();
        hook.configure_preview_proof(Arc::downgrade(&proof))?;
        app.manage(collector);
    }
    let session_previews =
        crate::platform::session_preview_coordinator::SessionPreviewCoordinator::new(
            runtime.clone(),
            hook.clone(),
            previews.clone(),
        );
    let export_closing = Arc::new(std::sync::atomic::AtomicBool::new(false));
    session_previews.start_current_session_owner(export_closing.clone())?;
    let cover_dependencies = session_previews.cover_dependencies(export_closing.clone());
    let cover_consumer = session_previews.cover_consumer();
    app.manage(previews);
    let shortcut_app = app.handle().clone();
    let coordinator =
        crate::platform::shortcuts::ShortcutCoordinator::new(Arc::new(move |intent| {
            let app = shortcut_app.clone();
            tauri::async_runtime::block_on(async move {
                let state = app.state::<DesktopState>();
                let result = match intent {
                    ShortcutIntent::ToggleHud => {
                        if let Some(hud) = app.get_webview_window("hud") {
                            match hud.is_visible() {
                                Ok(true) => hud.hide().map_err(|_| {
                                    AppError::new("hud_hide", "desktop", "字幕窗口暂时无法隐藏")
                                }),
                                Ok(false) => {
                                    crate::platform::window_manager::show(&app, WindowRole::Hud)
                                }
                                Err(_) => Err(AppError::new(
                                    "hud_visibility",
                                    "desktop",
                                    "无法检查字幕窗口",
                                )),
                            }
                        } else {
                            crate::platform::window_manager::show(&app, WindowRole::Hud)
                        }
                    }
                    ShortcutIntent::CycleReadingDisplayMode => {
                        crate::ipc::reading::cycle_display(&state).await
                    }
                    ShortcutIntent::ToggleCapturePause => match state.runtime.snapshot() {
                        Ok(snapshot) => {
                            if let Some(session) = snapshot
                                .session
                                .filter(|s| s.source_kind == "hook" && s.state == "active")
                            {
                                state
                                    .runtime
                                    .dispatch(CommandEnvelope {
                                        operation_id: SystemIdentity.new_id("pause"),
                                        expected_revision: None,
                                        command: Command::SetCapturePaused {
                                            paused: !session.paused,
                                        },
                                    })
                                    .await
                                    .map(|_| ())
                            } else {
                                Ok(())
                            }
                        }
                        Err(error) => Err(error),
                    },
                };
                if let Err(error) = result {
                    state.runtime.report_native_effect_failure(error);
                }
            });
        }));
    app.manage(coordinator);
    let watch_runtime = runtime.clone();
    app.manage(DesktopState {
        provider_registry,
        main_navigation: Mutex::new(None),
        settings_navigation: Mutex::new(None),
        word_cards: Default::default(),
        runtime,
        hook,
        subscriptions: Mutex::new(BTreeMap::new()),
        session_previews,
        pending_exports: Mutex::new(BTreeMap::new()),
        export_closing: export_closing.clone(),
    });
    crate::platform::session_cover_coordinator::SessionCoverCoordinator::spawn(
        cover_dependencies,
        cover_consumer,
    );
    app.set_menu(crate::platform::menus::build(app.handle())?)?;
    let menu_app = app.handle().clone();
    let menu_runtime = watch_runtime.clone();
    let mut menu_updates = watch_runtime.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut previous_request = None;
        loop {
            let enabled = {
                let update = menu_updates.borrow_and_update();
                let session = update.snapshot.session.as_ref();
                crate::platform::menus::capture_pause_enabled(
                    session.map(|s| s.source_kind.as_str()),
                    session.map(|s| s.state.as_str()),
                )
            };
            if previous_request != Some(enabled) {
                let app = menu_app.clone();
                let result = crate::ipc::blocking(move || {
                    crate::platform::menus::sync_capture_pause(&app, enabled)
                })
                .await;
                previous_request = Some(enabled);
                if let Err(error) = result {
                    let runtime = menu_runtime.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        runtime.report_native_effect_failure(error)
                    });
                }
            }
            if menu_updates.changed().await.is_err() {
                break;
            }
        }
    });
    let startup_app = app.handle().clone();
    let startup_runtime = watch_runtime.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) =
            crate::ipc::blocking(move || crate::ipc::reading::sync_native_preferences(&startup_app))
                .await
        {
            startup_runtime.report_native_effect_failure(error);
        }
    });
    // Reconcile game/global inheritance after runtime-originated session transitions.
    // IPC preference edits use their own native prepare / durable commit transaction.
    let watch_app = app.handle().clone();
    let mut updates = watch_runtime.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut previous = updates.borrow().snapshot.preferences.clone();
        while updates.changed().await.is_ok() {
            let preferences = updates.borrow_and_update().snapshot.preferences.clone();
            if preferences == previous {
                continue;
            }
            loop {
                let app = watch_app.clone();
                let result = crate::ipc::blocking(move || {
                    crate::ipc::reading::sync_native_preferences(&app)
                })
                .await;
                match result {
                    Ok(applied) => {
                        previous = applied;
                        if updates.borrow().snapshot.preferences == previous {
                            break;
                        }
                    }
                    Err(error) if error.code == "shortcut_apply_in_progress" => {
                        // A prepared IPC transaction may publish before commit. Retry
                        // even if no subsequent projection arrives after its commit.
                        let _ = tauri::async_runtime::spawn_blocking(|| {
                            std::thread::sleep(std::time::Duration::from_millis(100))
                        })
                        .await;
                    }
                    Err(error) => {
                        previous = updates.borrow().snapshot.preferences.clone();
                        watch_runtime.report_native_effect_failure(error);
                        break;
                    }
                }
            }
        }
    });
    if let Some(acceptance_lease) = paths.take_acceptance_lease() {
        app.manage(acceptance_lease);
    }
    Ok(())
}
