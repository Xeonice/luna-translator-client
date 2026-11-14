//! Narrow service IPC. Business policy and resource authority remain in Rust Core/store.
use crate::{bootstrap::DesktopState, ipc::{blocking, role}};
use app_core::contracts::*;
use tauri::{State, WebviewWindow};

#[tauri::command]
pub async fn application_service_draft(window: WebviewWindow, state: State<'_, DesktopState>, query: ServiceDraftQuery) -> AppResult<ServiceDraftEditorProjection> {
    role(&window, &["settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.service_draft(query)).await
}

#[tauri::command]
pub async fn application_service_configuration(window: WebviewWindow, state: State<'_, DesktopState>) -> AppResult<ServiceConfigurationSnapshot> {
    role(&window, &["settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.service_configuration()).await
}
#[tauri::command]
pub async fn application_save_service_draft(window: WebviewWindow, state: State<'_, DesktopState>, input: SaveServiceDraftInput) -> AppResult<ServiceDraftReceipt> {
    role(&window, &["settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.save_service_draft(input)).await
}
#[tauri::command]
pub async fn application_saved_services(window: WebviewWindow, state: State<'_, DesktopState>, query: SavedServiceQuery) -> AppResult<SavedServiceProjection> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.saved_services(query)).await
}
#[tauri::command]
pub async fn application_select_saved_services(window: WebviewWindow, state: State<'_, DesktopState>, input: SelectSavedServicesInput) -> AppResult<ServiceSelectionReceipt> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.select_saved_services(input)).await
}
#[tauri::command]
pub async fn application_restore_service_inheritance(window: WebviewWindow, state: State<'_, DesktopState>, input: RestoreServiceInheritanceInput) -> AppResult<ServiceSelectionReceipt> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.restore_service_inheritance(input)).await
}
#[tauri::command]
pub async fn application_set_service_purpose_enabled(window: WebviewWindow, state: State<'_, DesktopState>, input: SetServicePurposeEnabledInput) -> AppResult<ServiceSelectionReceipt> {
    role(&window, &["main", "hud", "settings", "learning"])?;
    let runtime = state.runtime.clone();
    blocking(move || runtime.set_service_purpose_enabled(input)).await
}
#[tauri::command]
pub async fn application_store_service_credential(window: WebviewWindow, state: State<'_, DesktopState>, input: ServiceCredentialRequest) -> AppResult<CredentialReference> {
    role(&window, &["settings"])?;
    let runtime = state.runtime.clone();
    blocking(move || {
        use app_core::domain::model_config::credentials::{CredentialMaterial, SecretText};
        runtime.ensure_service_profile_generation(input.expected_profile_generation)?;
        let (label, material) = match input.material {
            ServiceCredentialInput::ApiKey { label, key } => (label, CredentialMaterial::ApiKey { key: SecretText::from_bytes(key.into_bytes())? }),
            ServiceCredentialInput::BaiduAppSecret { label, app_id, secret } => (label, CredentialMaterial::BaiduAppSecret { app_id, secret: SecretText::from_bytes(secret.into_bytes())? }),
            ServiceCredentialInput::YoudaoAppSecret { label, app_key, secret } => (label, CredentialMaterial::YoudaoAppSecret { app_key, secret: SecretText::from_bytes(secret.into_bytes())? }),
            ServiceCredentialInput::GoogleServiceAccountFile { label, file_contents } => (label, app_infra::providers::google_auth::import_service_account_owned(file_contents.into_bytes())?),
        };
        runtime.store_credential_material(label, material)
    }).await
}

#[tauri::command]
pub async fn application_test_saved_service(window:WebviewWindow,state:State<'_,DesktopState>,input:SavedServiceTestRequest,operation_id:Option<String>,selected_token_index:Option<u32>)->AppResult<ServiceTestResult>{
    role(&window,&["settings"])?;
    state.runtime.test_saved_service(operation_id,input,selected_token_index).await
}
#[tauri::command]
pub async fn application_cancel_saved_service_test(window:WebviewWindow,state:State<'_,DesktopState>,operation_id:String,expected_profile_generation:std::num::NonZeroU32)->AppResult<()>{
    role(&window,&["settings"])?;let runtime=state.runtime.clone();
    blocking(move||runtime.cancel_saved_service_test(&operation_id,expected_profile_generation)).await
}
