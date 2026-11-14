//! Client-facing service operations. Mutation input never includes a whole graph,
//! internal activation role, legacy proof, secret, arbitrary header or token URL.
use super::*;
use crate::domain::model_config::service_graph::{
    ConfigurationV3, GoogleAuthorizationRef, SavedServiceRef,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, num::NonZeroU32};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfigurationDocument {
    Legacy { configuration: Configuration },
    ServiceGraph { configuration: ConfigurationV3 },
}
impl ConfigurationDocument {
    pub fn revision_id(&self) -> &str {
        match self {
            Self::Legacy { configuration } => &configuration.revision_id,
            Self::ServiceGraph { configuration } => &configuration.revision_id,
        }
    }
    pub fn as_legacy(&self) -> Option<&Configuration> {
        match self {
            Self::Legacy { configuration } => Some(configuration),
            Self::ServiceGraph { .. } => None,
        }
    }
    pub fn as_service_graph(&self) -> Option<&ConfigurationV3> {
        match self {
            Self::ServiceGraph { configuration } => Some(configuration),
            Self::Legacy { .. } => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServiceScope {
    Global {},
    Game { game_id: String },
}
impl ServiceScope {
    pub fn scope_key(&self) -> AppResult<String> {
        match self {
            Self::Global {} => Ok("global".into()),
            Self::Game { game_id } => {
                crate::domain::require_id(game_id)?;
                Ok(format!("game:{game_id}"))
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceConfigurationSnapshot {
    pub profile_generation: NonZeroU32,
    pub draft_head_revision: u32,
    pub applied_head_revision: u32,
    pub draft: ConfigurationDocument,
    pub applied: Option<ConfigurationDocument>,
    pub runtime_purposes: Vec<PurposeRuntimeView>,
    pub runtime_connections: Vec<ConnectionRuntimeView>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServiceAuthenticationSelection {
    None {},
    ExistingPool {
        pool_id: String,
        pool_revision: NonZeroU32,
    },
    GoogleOwnedGrant {
        authorization: GoogleAuthorizationRef,
    },
}
/// Untrusted mutation input is deserialized and validated, never an export/snapshot payload.
#[derive(Clone, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveServiceDraftInput {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
    pub expected_draft_head_revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub editing: Option<SavedServiceRef>,
    pub target: ServiceFormTarget,
    pub purpose: CatalogPurpose,
    #[serde(deserialize_with = "super::service_form::unique_values")]
    pub values: BTreeMap<String, CatalogScalar>,
    pub authentication: ServiceAuthenticationSelection,
}
impl std::fmt::Debug for SaveServiceDraftInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveServiceDraftInput")
            .field("operation_id", &self.operation_id)
            .field("target", &self.target)
            .field("purpose", &self.purpose)
            .field("field_ids", &self.values.keys().collect::<Vec<_>>())
            .finish()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceDraftReceipt {
    pub operation_id: String,
    pub revision_id: String,
    pub draft_head_revision: u32,
    pub saved_resource: SavedServiceRef,
    pub affected_shared_model_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceDraftQuery {
    pub expected_profile_generation: NonZeroU32,
    pub source: SavedServiceRef,
    pub purpose: CatalogPurpose,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceDraftEditorProjection {
    pub profile_generation: NonZeroU32,
    pub draft_head_revision: u32,
    pub source: SavedServiceRef,
    pub editing: Option<SavedServiceRef>,
    pub target: Option<ServiceFormTarget>,
    pub purpose: CatalogPurpose,
    pub values: BTreeMap<String, CatalogScalar>,
    pub authentication: ServiceAuthenticationSelection,
    pub readonly_reason: Option<String>,
    pub affected_shared_model_ids: Vec<String>,
    pub fallback: Vec<SavedServiceRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ServiceTestInputScope {
    Sample {},
    SelectedUnit { unit_id: String, text_revision: NonZeroU32, source_hash: String },
}
impl Default for ServiceTestInputScope {
    fn default() -> Self { Self::Sample {} }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedServiceTestRequest {
    pub expected_profile_generation: NonZeroU32,
    pub source: SavedServiceRef,
    pub purpose: CatalogPurpose,
    #[serde(default)]
    #[ts(optional)]
    pub input_scope: Option<ServiceTestInputScope>,
    /// Explicit output requirement; omission preserves the text test contract.
    /// It never borrows another active purpose's output mode.
    #[serde(default)]
    #[ts(optional)]
    pub output_mode: Option<ServiceTestOutputMode>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTestOutputMode { #[default] Text, Structured }
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTestStatus { Passed, Failed, Cancelled, TimedOut, Unsupported }
/// Safe application diagnosis, never a raw provider response or credential.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceTestFailureDetail {
    pub code: String,
    pub stage: Option<String>,
    pub recovery: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceTestResult {
    pub source: SavedServiceRef,
    pub purpose: CatalogPurpose,
    pub test_id: String,
    pub status: ServiceTestStatus,
    pub stage: String,
    pub message: String,
    pub model_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure: Option<ServiceTestFailureDetail>,
}
/// Dedicated secret mutation. Never an ordinary form, snapshot or export value.
#[derive(Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ServiceCredentialInput {
    ApiKey { label: String, key: String },
    BaiduAppSecret { label: String, app_id: String, secret: String },
    YoudaoAppSecret { label: String, app_key: String, secret: String },
    GoogleServiceAccountFile { label: String, file_contents: String },
}
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceCredentialRequest {
    pub expected_profile_generation: NonZeroU32,
    pub material: ServiceCredentialInput,
}
impl std::fmt::Debug for ServiceCredentialRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceCredentialRequest")
            .field("expected_profile_generation", &self.expected_profile_generation)
            .field("material", &self.material)
            .finish()
    }
}
impl std::fmt::Debug for ServiceCredentialInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::ApiKey { .. } => "api_key",
            Self::BaiduAppSecret { .. } => "baidu_app_secret",
            Self::YoudaoAppSecret { .. } => "youdao_app_secret",
            Self::GoogleServiceAccountFile { .. } => "google_service_account_file",
        };
        f.debug_struct("ServiceCredentialInput").field("kind", &kind).finish()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedServiceQuery {
    pub expected_profile_generation: NonZeroU32,
    pub scope: ServiceScope,
    pub purpose: CatalogPurpose,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceBackendKind {
    HttpLlm,
    HttpTranslation,
    PlatformNative,
    LocalWorker,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceEligibilityIssue {
    pub code: String,
    pub message: String,
    pub field_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedServiceEligibility {
    Selectable {},
    Blocked {
        reasons: Vec<ServiceEligibilityIssue>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServiceVerification {
    /// One used member does not qualify every credential in the selected pool.
    PartiallyTested { test_id: String, passed_credential_count: u32, total_credential_count: u32, tested_at_ms: i64 },
    Untested {},
    Stale {
        test_id: String,
    },
    Passed {
        test_id: String,
        tested_at_ms: i64,
    },
    Failed {
        test_id: String,
        code: String,
        tested_at_ms: i64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedServiceRow {
    pub source: SavedServiceRef,
    pub logical_resource_id: String,
    pub display_name: String,
    pub backend: ServiceBackendKind,
    pub purpose: CatalogPurpose,
    pub eligibility: SavedServiceEligibility,
    pub verification: ServiceVerification,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectiveServiceSelection {
    pub requested_scope: ServiceScope,
    pub effective_scope: ServiceScope,
    pub selection_revision: u32,
    pub effective_selection_revision: u32,
    pub main: Option<SavedServiceRef>,
    pub comparisons: Vec<SavedServiceRef>,
    pub purpose_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedServiceProjection {
    pub profile_generation: NonZeroU32,
    pub applied_head_revision: u32,
    pub selection: EffectiveServiceSelection,
    pub rows: Vec<SavedServiceRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectSavedServicesInput {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
    pub scope: ServiceScope,
    pub purpose: CatalogPurpose,
    pub expected_selection_revision: u32,
    pub expected_applied_head_revision: u32,
    pub main: SavedServiceRef,
    pub comparisons: Vec<SavedServiceRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreServiceInheritanceInput {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
    pub game_id: String,
    pub purpose: CatalogPurpose,
    pub expected_selection_revision: u32,
    pub expected_applied_head_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetServicePurposeEnabledInput {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
    pub scope: ServiceScope,
    pub purpose: CatalogPurpose,
    pub expected_selection_revision: u32,
    pub expected_applied_head_revision: u32,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceSelectionReceipt {
    pub operation_id: String,
    pub revision_id: String,
    pub applied_head_revision: u32,
    pub selection_revision: u32,
    pub affected_task_ids: Vec<String>,
    pub semantic_change: bool,
}

pub(super) fn typescript_declarations() -> String {
    [
        ConfigurationDocument::decl(&ts_rs::Config::default()),
        ServiceScope::decl(&ts_rs::Config::default()),
        ServiceConfigurationSnapshot::decl(&ts_rs::Config::default()),
        ServiceAuthenticationSelection::decl(&ts_rs::Config::default()),
        SaveServiceDraftInput::decl(&ts_rs::Config::default()),
        ServiceDraftReceipt::decl(&ts_rs::Config::default()),
        ServiceDraftQuery::decl(&ts_rs::Config::default()),
        ServiceDraftEditorProjection::decl(&ts_rs::Config::default()),
        ServiceTestInputScope::decl(&ts_rs::Config::default()),
        ServiceTestOutputMode::decl(&ts_rs::Config::default()),
        SavedServiceTestRequest::decl(&ts_rs::Config::default()),
        ServiceTestStatus::decl(&ts_rs::Config::default()),
        ServiceTestFailureDetail::decl(&ts_rs::Config::default()),
        ServiceTestResult::decl(&ts_rs::Config::default()),
        ServiceCredentialInput::decl(&ts_rs::Config::default()),
        ServiceCredentialRequest::decl(&ts_rs::Config::default()),
        SavedServiceQuery::decl(&ts_rs::Config::default()),
        ServiceBackendKind::decl(&ts_rs::Config::default()),
        ServiceEligibilityIssue::decl(&ts_rs::Config::default()),
        SavedServiceEligibility::decl(&ts_rs::Config::default()),
        ServiceVerification::decl(&ts_rs::Config::default()),
        SavedServiceRow::decl(&ts_rs::Config::default()),
        EffectiveServiceSelection::decl(&ts_rs::Config::default()),
        SavedServiceProjection::decl(&ts_rs::Config::default()),
        SelectSavedServicesInput::decl(&ts_rs::Config::default()),
        RestoreServiceInheritanceInput::decl(&ts_rs::Config::default()),
        SetServicePurposeEnabledInput::decl(&ts_rs::Config::default()),
        ServiceSelectionReceipt::decl(&ts_rs::Config::default()),
    ]
    .map(|value| format!("export {value}\n"))
    .join("\n")
}
