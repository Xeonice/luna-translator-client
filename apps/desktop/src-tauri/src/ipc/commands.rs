use crate::{bootstrap::DesktopState, ipc::*, platform::window_manager};
use app_core::contracts::*;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
#[tauri::command]
pub async fn application_set_login_startup(
    window: WebviewWindow,
    enabled: bool,
) -> AppResult<LoginStartupState> {
    role(&window, crate::platform::login_startup::IPC_ROLES)?;
    let app = window.app_handle().clone();
    blocking(move || crate::platform::login_startup::set(&app, enabled)).await
}
async fn settle_shortcuts(
    coordinator: std::sync::Arc<crate::platform::shortcuts::ShortcutCoordinator>,
    token: String,
    commit: bool,
) -> AppResult<ShortcutProjection> {
    blocking(move || super::reading::settle_native_shortcuts(&coordinator, &token, commit)).await
}
#[tauri::command]
pub async fn application_dispatch(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    envelope: CommandEnvelope,
) -> AppResult<CommandReceipt> {
    role(&window, command_roles(&envelope.command))?;
    let disconnect_correlation = matches!(envelope.command, Command::Disconnect)
        .then(|| envelope.operation_id.clone());
    if let Some(correlation)=&disconnect_correlation {
        binding_observation::event("intent",correlation,serde_json::json!({}));
        binding_observation::event("port_call",correlation,serde_json::json!({"method":"disconnect","args":{}}));
    }
    let preferences_changed = matches!(
        envelope.command,
        Command::SavePreferences { .. }
            | Command::SaveScopedPreferences { .. }
            | Command::SetHudPreference { .. }
            | Command::SetReadingAnnotations { .. }
            | Command::CycleReadingDisplayMode
    );
    let unbind_preview = if let Command::UnbindWindow(request) = &envelope.command {
        let epoch = state.session_previews.epoch().map_err(|_| {
            AppError::new("preview_state", "preview", "窗口预览状态暂不可用，请重试")
        })?;
        Some((request.expected_target_identity_sha256.clone(), epoch))
    } else {
        None
    };
    let coordinator = window
        .app_handle()
        .state::<std::sync::Arc<crate::platform::shortcuts::ShortcutCoordinator>>();
    let prepared = if let Command::SavePreferences { preferences }
    | Command::SaveScopedPreferences { preferences, .. } = &envelope.command
    {
        let runtime = state.runtime.clone();
        let preferences = preferences.clone();
        let effective = blocking(move || runtime.preview_preferences(preferences)).await?;
        let token = coordinator.prepare(&effective.shortcuts, &effective.scope_key, None)?;
        Some((token, effective))
    } else {
        None
    };
    let result = match state.runtime.dispatch(envelope).await {
        Ok(result) => result,
        Err(error) => {
            if let Some(correlation)=&disconnect_correlation {
                binding_observation::event("port_return",correlation,serde_json::json!({"method":"disconnect","response":{"errorCode":error.code}}));
            }
            if let Some((token, _)) = &prepared
                && let Err(cleanup) =
                    settle_shortcuts(coordinator.inner().clone(), token.clone(), false).await
            {
                state.runtime.report_native_effect_failure(cleanup);
            }
            return Err(error);
        }
    };
    if let Some(correlation)=&disconnect_correlation {
        if let Ok(context)=binding_context(&state) {binding_observation::transition(correlation,context);}
        binding_observation::event("port_return",correlation,serde_json::json!({"method":"disconnect","response":result}));
    }
    if let Some((token, effective)) = prepared {
        let commit = match state.runtime.snapshot() {
            Ok(snapshot) => {
                snapshot.preferences.shortcuts == effective.shortcuts
                    && snapshot.preferences.scope_key == effective.scope_key
            }
            Err(error) => {
                state.runtime.report_native_effect_failure(error);
                false
            }
        };
        if let Err(error) =
            settle_shortcuts(coordinator.inner().clone(), token.clone(), commit).await
        {
            state.runtime.report_native_effect_failure(error);
            // A failure after durable commit is a native effect failure. Always
            // attempt token cleanup rather than leave future applies pending.
            if commit
                && let Err(cleanup) =
                    settle_shortcuts(coordinator.inner().clone(), token, false).await
                && cleanup.code != "shortcut_transaction"
            {
                state.runtime.report_native_effect_failure(cleanup);
            }
        }
        if !commit && let Err(error) = super::reading::sync_native_preferences(window.app_handle())
        {
            state.runtime.report_native_effect_failure(error);
        }
        if let Err(error) = coordinator.projection().and_then(|projection| {
            crate::platform::menus::sync_shortcuts(window.app_handle(), &projection)
        }) {
            state.runtime.report_native_effect_failure(error);
        }
    }

    if let Some((expected_identity, epoch)) = unbind_preview
        && !result.replayed
        && result.result_id.as_deref() == Some(expected_identity.as_str())
    {
        super::reading::stop_window_previews_if_epoch(window.app_handle(), epoch);
    }
    if preferences_changed {
        let effect = state.runtime.snapshot().and_then(|snapshot| {
            window_manager::sync_hud_preferences(window.app_handle(), &snapshot.preferences)
        });
        if let Err(error) = effect {
            // Persistence already committed. Report the separate native effect
            // without turning a successful durable operation into a false error.
            state.runtime.report_native_effect_failure(error);
        }
    }
    Ok(result)
}
#[tauri::command]
pub fn application_open_main_page(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    intent: MainPageIntent,
) -> AppResult<()> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    let request = {
        let mut pending = state
            .main_navigation
            .lock()
            .map_err(|_| AppError::new("navigation_state", "desktop", "无法读取主窗口状态"))?;
        let revision = pending
            .as_ref()
            .map_or(Some(1), |old| old.revision.checked_add(1))
            .ok_or_else(|| AppError::new("navigation_revision", "desktop", "请重启客户端后重试"))?;
        let request = MainPageRequest { revision, intent };
        *pending = Some(request.clone());
        request
    };
    window_manager::show(&app, WindowRole::Main)?;
    app.emit_to("main", "application-main-page", request)
        .map_err(|_| AppError::new("navigation_delivery", "desktop", "主窗口已打开，请重试导航"))
}
fn record_settings_navigation(
    pending: &mut Option<SettingsPageRequest>,
    intent: SettingsPageIntent,
) -> AppResult<SettingsPageRequest> {
    let revision = pending
        .as_ref()
        .map_or(Some(1), |old| old.revision.checked_add(1))
        .ok_or_else(|| AppError::new("navigation_revision", "desktop", "请重启客户端后重试"))?;
    let request = SettingsPageRequest { revision, intent };
    *pending = Some(request.clone());
    Ok(request)
}
#[tauri::command]
pub fn application_open_settings_page(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    intent: SettingsPageIntent,
) -> AppResult<()> {
    role(&window, &["main"])?;
    let request = {
        let mut pending = state
            .settings_navigation
            .lock()
            .map_err(|_| AppError::new("navigation_state", "desktop", "无法读取设置窗口状态"))?;
        record_settings_navigation(&mut pending, intent)?
    };
    window_manager::show(&app, WindowRole::Settings)?;
    app.emit_to("settings", "application-settings-page", request)
        .map_err(|_| AppError::new("navigation_delivery", "desktop", "设置窗口已打开，请重试导航"))
}
#[tauri::command]
pub async fn application_associate(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: AssociateRequest,
) -> AppResult<CommandReceipt> {
    role(&window, &["main"])?;
    // A boolean switch flag cannot carry the reviewed target/session witness.
    // Reject before touching Core or any preview/window lifecycle owner.
    let _ = (state, request);
    Err(AppError::new(
        "reviewed_association_required",
        "capture",
        "请重新核对运行目标后，通过关联并接入继续",
    ))
}
#[tauri::command]
pub async fn application_associate_reviewed(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: ReviewedAssociateRequest,
) -> AppResult<AssociationOperationReceipt> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    // Register only the exact original candidate metadata for a possible
    // handoff. Core still validates and commits the association; preparation
    // neither advances the preview epoch nor performs capture/window effects.
    let epoch = state.session_previews.prepare_reviewed_association(request.clone()).await?;
    let correlation=request.operation_id.clone();
    binding_observation::event("intent", &correlation, serde_json::json!({}));
    binding_observation::event("port_call", &correlation, serde_json::json!({"method":"associate","args":request}));
    let result = blocking(move || runtime.associate_reviewed(request)).await;
    binding_observation::event("port_return", &correlation, serde_json::json!({"method":"associate","response":match &result {
        Ok(receipt)=>serde_json::to_value(receipt).unwrap_or_default(),
        Err(error)=>serde_json::json!({"errorCode":error.code}),
    }}));
    let result=result?;
    if result.replayed { return Ok(result); }
    if let Err(error) = state.session_previews.finish_reviewed_association(result.clone(),epoch).await {
        state.runtime.report_native_effect_failure(error);
    }
    // Session preference reconciliation is owned by the runtime subscription;
    // an old association continuation must not apply effects to a new session.
    Ok(result)
}

fn window_step(result: AppResult<()>) -> NativeWindowStepResult {
    match result {
        Ok(()) => NativeWindowStepResult { state: NativeWindowStepState::Succeeded, error: None },
        Err(error) => NativeWindowStepResult { state: NativeWindowStepState::Failed, error: Some(error) },
    }
}

/// Called only inside the Core authority guard. A failed show never hides the
/// main window. Durable selection and fallible native effects remain separate.
fn reading_window_steps(
    authority: ReadingWindowEffectAuthority,
    show: impl FnOnce() -> AppResult<()>,
    hide: impl FnOnce() -> AppResult<()>,
) -> ReadingWindowEffectsResult {
    let show_hud = window_step(show());
    binding_observation::event(if show_hud.state==NativeWindowStepState::Succeeded {"show_hud_completed"} else {"window_effect_failed"},
        &authority.operation_id, serde_json::json!({"effect":"show_hud","error_code":show_hud.error.as_ref().map(|e|&e.code)}));
    let hide_main = if show_hud.state == NativeWindowStepState::Succeeded {
        window_step(hide())
    } else {
        NativeWindowStepResult { state: NativeWindowStepState::NotAttempted, error: None }
    };
    let retryable = show_hud.state != NativeWindowStepState::Succeeded
        || hide_main.state != NativeWindowStepState::Succeeded;
    if hide_main.state!=NativeWindowStepState::NotAttempted {
        binding_observation::event(if hide_main.state==NativeWindowStepState::Succeeded {"hide_main_completed"} else {"window_effect_failed"},
            &authority.operation_id, serde_json::json!({"effect":"hide_main","error_code":hide_main.error.as_ref().map(|e|&e.code)}));
    }
    ReadingWindowEffectsResult {
        authority, show_hud, hide_main, retryable,
        detail: if retryable { "正文选择已保存，窗口操作未完成，可重试打开阅读窗口" }
            else { "正文选择已保存，字幕浮窗已显示" }.into(),
    }
}

fn authorized_reading_windows(
    app: &AppHandle,
    runtime: &app_core::application::runtime::ApplicationRuntime,
    authority: ReadingWindowEffectAuthority,
) -> AppResult<ReadingWindowEffectsResult> {
    let mut effects = None;
    runtime.with_reading_window_effect_authority(authority.clone(), &mut || {
        effects = Some(reading_window_steps(authority.clone(), || {
            window_manager::show(app, WindowRole::Hud)?;
            let hud = app.get_webview_window("hud").ok_or_else(|| {
                AppError::new("hud_show", "desktop", "字幕浮窗尚未打开，请重试")
            })?;
            match hud.is_visible() {
                Ok(true) => Ok(()),
                _ => Err(AppError::new("hud_show", "desktop", "字幕浮窗尚未显示，请重试")),
            }
        }, || {
            let main = app.get_webview_window("main").ok_or_else(|| {
                AppError::new("main_hide", "desktop", "主窗口暂不可用，请重试")
            })?;
            main.hide().map_err(|_| AppError::new("main_hide", "desktop", "主窗口尚未隐藏，请重试"))?;
            match main.is_visible() {
                Ok(false) => Ok(()),
                _ => Err(AppError::new("main_hide", "desktop", "主窗口尚未隐藏，请重试")),
            }
        }));
        Ok(())
    })?;
    effects.ok_or_else(|| AppError::new("reading_window_effect", "desktop", "窗口操作未执行，请重试"))
}

#[tauri::command]
pub async fn application_begin_reading(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: BeginReadingRequest,
) -> AppResult<BeginReadingResult> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    let correlation=request.operation_id.clone();
    binding_observation::event("intent",&correlation,serde_json::json!({}));
    binding_observation::event("port_call",&correlation,serde_json::json!({"method":"beginReading","args":request}));
    let committed = blocking(move || runtime.begin_reading(request)).await;
    if let Err(error)=&committed {
        binding_observation::event("port_return",&correlation,serde_json::json!({"method":"beginReading","response":{"errorCode":error.code}}));
    }
    let selection=committed?;
    let current_context=binding_context(&state).ok()
        .filter(|context|context.session_id.as_deref()==Some(selection.selection.session_id.as_str()));
    let mut context=current_context.unwrap_or_default();
    context.session_id=Some(selection.selection.session_id.clone());
    context.operation_id=Some(selection.operation_id.clone());
    context.selection_set_revision=Some(selection.selection.set_revision.clone());
    binding_observation::transition(&correlation,context);
    binding_observation::event("persist_commit",&correlation,serde_json::json!({"response":selection}));
    let authority = selection.effect_authority.clone();
    let app = window.app_handle().clone();
    let runtime = state.runtime.clone();
    let effects = blocking(move || authorized_reading_windows(&app, &runtime, authority)).await
        .unwrap_or_else(|error| ReadingWindowEffectsResult {
            authority: selection.effect_authority.clone(),
            show_hud: NativeWindowStepResult { state: NativeWindowStepState::NotAttempted, error: Some(error) },
            hide_main: NativeWindowStepResult { state: NativeWindowStepState::NotAttempted, error: None },
            retryable: false,
            detail: "正文选择已保存，当前会话或选择已变化，请重新核对后进入阅读".into(),
        });
    let result=BeginReadingResult { selection, effects };
    binding_observation::event("port_return",&correlation,serde_json::json!({"method":"beginReading","response":result}));
    Ok(result)
}

#[tauri::command]
pub async fn application_retry_reading_window_effects(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    authority: ReadingWindowEffectAuthority,
) -> AppResult<ReadingWindowEffectsResult> {
    role(&window, &["main"])?;
    let app = window.app_handle().clone();
    let runtime = state.runtime.clone();
    let correlation=authority.operation_id.clone();
    binding_observation::event("intent",&correlation,serde_json::json!({}));
    binding_observation::event("port_call",&correlation,serde_json::json!({"method":"retryReadingWindowEffects","args":authority}));
    let result=blocking(move || authorized_reading_windows(&app, &runtime, authority)).await;
    binding_observation::event("port_return",&correlation,serde_json::json!({"method":"retryReadingWindowEffects","response":match &result {
        Ok(effects)=>serde_json::to_value(effects).unwrap_or_default(),
        Err(error)=>serde_json::json!({"errorCode":error.code}),
    }}));
    result
}

#[tauri::command(rename = "application_preview_select_window")]
pub async fn observed_preview_select_window(
    window: WebviewWindow,
    app: AppHandle,
    request: PreviewStartRequest,
) -> AppResult<PreviewProjection> {
    role(&window,&["main"])?;
    let correlation=binding_observation::correlation();
    binding_observation::event("intent",&correlation,serde_json::json!({}));
    binding_observation::event("port_call",&correlation,serde_json::json!({"method":"selectWindowPreview","args":request}));
    let result=super::reading::application_preview_select_window(window,app,request).await;
    binding_observation::event("port_return",&correlation,serde_json::json!({"method":"selectWindowPreview","response":match &result {
        Ok(projection)=>serde_json::json!({"state":projection.state,"errorCode":projection.error_code,"capturedAtMs":projection.captured_at_ms,"generation":projection.generation,"leaseId":projection.lease_id}),
        Err(error)=>serde_json::json!({"errorCode":error.code}),
    }}));
    result
}
#[tauri::command]
pub fn application_window(
    window: WebviewWindow,
    app: AppHandle,
    role: WindowRole,
) -> AppResult<()> {
    crate::ipc::role(&window, &["main", "hud", "settings", "learning"])?;
    window_manager::show(&app, role)
}
#[tauri::command]
pub async fn application_learning_object(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
    token_index: Option<u32>,
) -> AppResult<LearningObject> {
    role(&window, &["main", "hud", "learning"])?;
    if text_revision == 0 {
        return Err(AppError::new(
            "invalid_revision",
            "study",
            "请选择固定文本版本",
        ));
    }
    let runtime = state.runtime.clone();
    let object =
        blocking(move || runtime.freeze_learning(unit_id, text_revision, token_index)).await?;
    window_manager::show(&app, WindowRole::Learning)?;
    Ok(object)
}
#[tauri::command]
pub async fn application_study(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
    purpose: String,
    token_index: Option<u32>,
) -> AppResult<TaskView> {
    role(&window, &["main", "hud", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.study(unit_id, text_revision, purpose, token_index)).await
}
#[tauri::command]
pub async fn application_test_configuration(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: ConfigurationTestRequest,
) -> AppResult<ConfigurationTestResult> {
    role(&window, &["settings", "main"])?;
    state.runtime.test_configuration(request).await
}
#[tauri::command]
pub async fn application_store_credential(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    input: CredentialInput,
) -> AppResult<CredentialReference> {
    role(&window, &["settings", "main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.store_credential(input)).await
}
#[tauri::command]
pub async fn application_credential_revocation_impact(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    credential_id: String,
) -> AppResult<CredentialRevocationImpact> {
    role(&window, &["settings", "main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.credential_revocation_impact(credential_id)).await
}
#[tauri::command]
pub async fn application_revoke_credential(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: CredentialRevocationRequest,
) -> AppResult<CommandReceipt> {
    role(&window, &["settings", "main"])?;
    state.runtime.revoke_credential(request).await
}
#[tauri::command]
pub async fn application_retranslate(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
    operation_id: String,
) -> AppResult<Vec<TaskView>> {
    role(&window, &["main", "hud"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.retranslate(unit_id, text_revision, operation_id)).await
}

#[cfg(test)]
mod settings_navigation_tests {
    use super::*;

    #[test]
    fn settings_navigation_retains_latest_request_for_late_and_repeated_reads() {
        let mut pending = None;
        for revision in 1..=3 {
            let request = record_settings_navigation(&mut pending, SettingsPageIntent::Translation)
                .expect("navigation request");
            assert_eq!(request.revision, revision);
            assert_eq!(request.intent, SettingsPageIntent::Translation);
            assert_eq!(pending.as_ref(), Some(&request));
            assert_eq!(pending.clone(), Some(request));
        }
    }

    #[test]
    fn settings_navigation_overflow_preserves_the_last_recoverable_request() {
        let last = SettingsPageRequest {
            revision: u32::MAX,
            intent: SettingsPageIntent::Translation,
        };
        let mut pending = Some(last.clone());
        let error = record_settings_navigation(&mut pending, SettingsPageIntent::Translation)
            .expect_err("overflow must not wrap or clear pending navigation");
        assert_eq!(error.code, "navigation_revision");
        assert_eq!(pending, Some(last));
    }
}

#[cfg(test)]
mod reading_window_effect_tests {
    use super::*;
    use std::cell::RefCell;
    fn authority() -> ReadingWindowEffectAuthority {
        ReadingWindowEffectAuthority {
            profile:BindingProfileContext {profile_id:"profile".into(),server_instance_id:"server".into(),profile_generation:1},
            session_id:"session".into(),set_revision:"9007199254740993".into(),operation_id:"operation".into(),
        }
    }
    #[test]
    fn failed_show_preserves_main_and_exact_authority() {
        let calls=RefCell::new(Vec::new());
        let result=reading_window_steps(authority(),||{calls.borrow_mut().push("show");Err(AppError::new("show_failed","desktop","failed"))},||{calls.borrow_mut().push("hide");Ok(())});
        assert_eq!(*calls.borrow(),vec!["show"]);
        assert_eq!(result.authority,authority());
        assert_eq!(result.show_hud.state,NativeWindowStepState::Failed);
        assert_eq!(result.hide_main.state,NativeWindowStepState::NotAttempted);
        assert!(result.retryable);
    }
    #[test]
    fn show_must_complete_before_hide_and_hide_failure_can_retry_effects() {
        let calls=RefCell::new(Vec::new());
        let result=reading_window_steps(authority(),||{calls.borrow_mut().push("show");Ok(())},||{calls.borrow_mut().push("hide");Err(AppError::new("hide_failed","desktop","failed"))});
        assert_eq!(*calls.borrow(),vec!["show","hide"]);
        assert_eq!(result.show_hud.state,NativeWindowStepState::Succeeded);
        assert_eq!(result.hide_main.state,NativeWindowStepState::Failed);
        assert!(result.retryable);
    }
    #[test]
    fn successful_effects_retain_decimal_revision_and_have_no_retry() {
        let result=reading_window_steps(authority(),||Ok(()),||Ok(()));
        assert_eq!(result.authority.set_revision,"9007199254740993");
        assert_eq!(result.show_hud.state,NativeWindowStepState::Succeeded);
        assert_eq!(result.hide_main.state,NativeWindowStepState::Succeeded);
        assert!(!result.retryable);
    }
}
