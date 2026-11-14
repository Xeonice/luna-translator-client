use crate::{bootstrap::DesktopState, ipc::*};
use app_core::contracts::*;
use tauri::{Manager, State, WebviewWindow};

// Private diagnostic projections deliberately omit reading text, samples,
// configuration and credential data. The public query responses are unchanged.
fn snapshot_journal_projection(snapshot: &AppSnapshot) -> serde_json::Value {
serde_json::json!({
            "profileId": snapshot.profile_id,
            "serverInstanceId": snapshot.server_instance_id,
            "profileGeneration": snapshot.profile_generation,
            "commitRevision": snapshot.commit_revision,
            "session": snapshot.session.as_ref().map(|session| serde_json::json!({
                "sessionId": session.session_id, "gameId": session.game_id,
                "sourceKind": session.source_kind, "state": session.state,
            })),
        })
}
fn source_selection_journal_projection(selection: &SourceSelectionProjection) -> serde_json::Value {
serde_json::json!({
            "profile": selection.profile, "sessionId": selection.session_id,
            "current": selection.current,
            "sources": selection.sources.iter().map(|source| serde_json::json!({
                "sourceId":source.source_id, "streamIdentitySha256":source.stream_identity_sha256,
                "hookName":source.hook_name, "hookCode":source.hook_code, "encoding":source.encoding,
                "address":source.address, "context":source.context, "context2":source.context2,
                "identityExtensionsJson":source.identity_extensions_json,
            })).collect::<Vec<_>>(),
        })
}
#[tauri::command]
pub fn application_service_catalog(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<ServiceCatalog> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    // This immutable view contains descriptors and material requirements only.
    // A read neither retrieves credentials nor starts a provider request.
    Ok(state.provider_registry.catalog())
}
#[tauri::command]
pub fn application_service_form(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: ServiceFormRequest,
) -> AppResult<ServiceFormProjection> {
    role(&window, &["settings"])?;
    // Conditions, defaults and input restrictions are pure Rust policy.
    // This query neither retrieves secrets nor grants saved-service eligibility.
    state.provider_registry.project_form(&request)
}
#[tauri::command]
pub async fn application_login_startup(window: WebviewWindow) -> AppResult<LoginStartupState> {
    role(&window, crate::platform::login_startup::IPC_ROLES)?;
    let app = window.app_handle().clone();
    blocking(move || crate::platform::login_startup::query(&app)).await
}
#[tauri::command]
pub fn application_main_page(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<Option<MainPageRequest>> {
    role(&window, &["main"])?;
    state
        .main_navigation
        .lock()
        .map(|request| request.clone())
        .map_err(|_| AppError::new("navigation_state", "desktop", "无法读取主窗口状态"))
}
#[tauri::command]
pub fn application_settings_page(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<Option<SettingsPageRequest>> {
    role(&window, &["settings"])?;
    state
        .settings_navigation
        .lock()
        .map(|request| request.clone())
        .map_err(|_| AppError::new("navigation_state", "desktop", "无法读取设置窗口状态"))
}
#[tauri::command]
pub async fn application_snapshot(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<AppSnapshot> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    let runtime = state.runtime.clone();
    let correlation = binding_observation::correlation();
    binding_observation::event("port_call", &correlation, serde_json::json!({"method":"snapshot","args":{}}));
    let result = blocking(move || runtime.snapshot()).await;
    // The private observation journal contains identity/revision facts only,
    // never game text, provider configuration or credentials from the snapshot.
    let response = match &result {
        Ok(snapshot) => snapshot_journal_projection(snapshot),
        Err(error) => serde_json::json!({"errorCode":error.code}),
    };
    binding_observation::event("port_return", &correlation, serde_json::json!({"method":"snapshot","response":response}));
    result
}
#[tauri::command]
pub async fn application_targets(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<TargetCandidates> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.targets()).await
}
#[tauri::command]
pub async fn application_review_association(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: AssociationReviewQuery,
) -> AppResult<AssociationReview> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    let correlation = binding_observation::correlation();
    binding_observation::event("port_call", &correlation, serde_json::json!({"method":"reviewAssociation","args":query}));
    let result = blocking(move || runtime.review_association(query)).await;
    if let Ok(review) = &result {
        binding_observation::transition(&correlation, binding_observation::Context {
            review_id:Some(review.review_id.clone()),session_id:review.active_session_id.clone(),
            active_target_identity_sha256:review.active_target_identity_sha256.clone(),
            candidate_target_identity_sha256:Some(review.candidate.target_identity_sha256.clone()),
            ..Default::default()
        });
    }
    binding_observation::event("port_return", &correlation, serde_json::json!({"method":"reviewAssociation","response":match &result {
        Ok(review)=>serde_json::json!({"reviewId":review.review_id,"relation":review.relation,"activeSessionId":review.active_session_id}),
        Err(error)=>serde_json::json!({"errorCode":error.code}),
    }}));
    result
}
#[tauri::command]
pub async fn application_association_progress(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: AssociationProgressQuery,
) -> AppResult<AssociationProgress> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.association_progress(query)).await
}
#[tauri::command]
pub async fn application_source_selection(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: SourceSelectionQuery,
) -> AppResult<SourceSelectionProjection> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    let correlation = binding_observation::correlation();
    binding_observation::event("port_call", &correlation, serde_json::json!({"method":"sourceSelection","args":query}));
    let result = blocking(move || runtime.source_selection(query)).await;
    let response = match &result {
        Ok(selection) => source_selection_journal_projection(selection),
        Err(error) => serde_json::json!({"errorCode":error.code}),
    };
    binding_observation::event("port_return", &correlation, serde_json::json!({"method":"sourceSelection","response":response}));
    result
}
#[tauri::command]
pub async fn application_pick_window(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    game_id: Option<String>,
) -> AppResult<TargetCandidates> {
    role(&window, &["main"])?;
    let hook = state.hook.clone();
    let key = game_id
        .map(|id| state.runtime.game_environment(&id))
        .transpose()?;
    blocking(move || hook.pick_window(key)).await
}
#[tauri::command]
pub async fn application_history(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    game_id: Option<String>,
    cursor: Option<String>,
    limit: u32,
) -> AppResult<HistoryPage> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.history(game_id, cursor, limit)).await
}
#[tauri::command]
pub async fn application_history_page(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: HistoryQuery,
) -> AppResult<HistoryPage> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.history_page(query)).await
}
#[tauri::command]
pub async fn application_history_sessions(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    query: HistorySessionsQuery,
) -> AppResult<HistorySessionsPage> {
    role(&window, &["main"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.history_sessions(query)).await
}
#[tauri::command]
pub async fn application_reading_unit(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
) -> AppResult<ReadingUnit> {
    role(&window, &["main", "hud", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.unit(unit_id, text_revision)).await
}
#[tauri::command]
pub async fn application_reading_unit_projection(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
) -> AppResult<ReadingUnitProjection> {
    role(&window, &["main", "hud", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.reading_unit_projection(unit_id, text_revision)).await
}
#[tauri::command]
pub async fn application_lookup(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    unit_id: String,
    text_revision: u32,
    token_index: u32,
) -> AppResult<WordLookup> {
    role(&window, &["main", "hud", "learning"])?;
    if text_revision == 0 {
        return Err(AppError::new(
            "invalid_revision",
            "dictionary",
            "请选择固定文本版本",
        ));
    }
    let runtime = state.runtime.clone();
    blocking(move || runtime.lookup(unit_id, text_revision, token_index)).await
}
#[tauri::command]
pub async fn application_resources(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<Vec<ResourceInfo>> {
    role(&window, &["main", "settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || Ok(runtime.resources())).await
}
#[tauri::command]
pub async fn application_pick_executables(
    window: WebviewWindow,
) -> AppResult<Vec<ExecutableSelection>> {
    role(&window, &["main"])?;
    blocking(app_infra::platform::executables::pick).await
}
#[tauri::command]
pub async fn application_inspect_executables(
    window: WebviewWindow,
    paths: Vec<String>,
) -> AppResult<Vec<ExecutableSelection>> {
    role(&window, &["main"])?;
    blocking(move || app_infra::platform::executables::inspect(paths)).await
}

#[cfg(test)]
mod binding_query_observation_tests {
    use super::*;
    use app_core::ports::ProductRepository;
    use std::collections::BTreeSet;

    #[test]
    fn snapshot_journal_keeps_profile_revision_and_stopped_identity_without_product_data() {
        let directory = std::env::temp_dir().join(format!("luna-query-journal-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&directory).unwrap();
        let store = app_infra::storage::SqliteStore::open(&directory.join("profile.sqlite3"), "observed-profile", "observed-server", 1).unwrap();
        let mut snapshot = store.snapshot().unwrap();
        snapshot.session = Some(Session { session_id:"stopped-session".into(), game_id:Some("game".into()), source_kind:"hook".into(),
            state:"stopped".into(), paused:false, translation_enabled:false, sources:vec![] });
        let value = snapshot_journal_projection(&snapshot);
        let keys = value.as_object().unwrap().keys().map(String::as_str).collect::<BTreeSet<_>>();
        assert_eq!(keys, BTreeSet::from(["profileId", "serverInstanceId", "profileGeneration", "commitRevision", "session"]));
        assert_eq!(value["profileId"], snapshot.profile_id);
        assert_eq!(value["commitRevision"], snapshot.commit_revision);
        assert_eq!(value["session"]["state"], "stopped");
        assert_eq!(value["session"].as_object().unwrap().len(), 4);
        drop(store);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_journal_excludes_samples_and_preserves_full_decimal_identity() {
        let projection = SourceSelectionProjection {
            profile:BindingProfileContext { profile_id:"profile".into(),server_instance_id:"server".into(),profile_generation:1 },
            session_id:"session".into(),state:SourceSelectionState::Unselected,current:None,saved:None,remember_allowed:false,member_limit:64,
            sources:vec![SourceDetailView { source_id:"source".into(),hook_name:"TextOutW".into(),hook_code:"hook-code".into(),
                address:Some("18446744073709551615".into()),context:Some("9007199254740993".into()),context2:Some("0".into()),
                encoding:"utf-16".into(),latest_sample:"private dialogue must not enter journal".into(),stream_identity_sha256:"a".repeat(64),identity_extensions_json:"{}".into() }],
        };
        let value = source_selection_journal_projection(&projection);
        let text = serde_json::to_string(&value).unwrap();
        assert!(!text.contains(&projection.sources[0].latest_sample));
        assert!(value["sources"][0].get("latestSample").is_none());
        assert_eq!(value["sources"][0]["address"], "18446744073709551615");
        assert_eq!(value["sources"][0]["context"], "9007199254740993");
        assert_eq!(value["profile"], serde_json::to_value(&projection.profile).unwrap());
        assert!(value.get("saved").is_none());
    }
}
