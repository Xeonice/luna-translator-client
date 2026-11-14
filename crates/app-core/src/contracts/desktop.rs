//! Product-facing desktop and language DTOs. Platform/SQL/FFI representations never cross IPC.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetCandidate {
    pub candidate_id: String,
    pub window_title: String,
    pub executable_path: String,
    pub windows_pid: u32,
    pub process_created_u64: String,
    pub architecture: String,
    pub environment_key: String,
    pub bottle_label: String,
    pub has_window: bool,
    pub window_handle: Option<String>,
    pub macos_host_pid: Option<u32>,
    pub macos_window_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetCandidates {
    pub candidates: Vec<TargetCandidate>,
    pub diagnostics: Vec<super::Diagnostic>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssociateRequest {
    pub operation_id: String,
    pub game_id: Option<String>,
    pub candidate_id: String,
    pub confirm_switch: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableSelection {
    pub path: String,
    pub display_name: String,
    pub architecture: Option<String>,
    pub environment_key: Option<String>,
    pub bottle_locator: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WindowRole {
    Main,
    Hud,
    Settings,
    Learning,
}
/// Current operating-system registration, independent of profile preferences.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LoginStartupStatus {
    NotRegistered,
    Enabled,
    RequiresApproval,
    NotFound,
    Unsupported,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoginStartupState {
    pub status: LoginStartupStatus,
    /// False for unsupported systems, unsigned/unbundled and acceptance builds.
    pub can_change: bool,
    pub error_code: Option<String>,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MainPageIntent {
    History,
    Session,
    Capture,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MainPageRequest {
    pub revision: u32,
    pub intent: MainPageIntent,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsPageIntent {
    Translation,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPageRequest {
    pub revision: u32,
    pub intent: SettingsPageIntent,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HudPreference {
    Segmentation,
    Compact,
    AlwaysOnTop,
}
impl HudPreference {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Segmentation => "reading.segmentationEnabled",
            Self::Compact => "hud.compact",
            Self::AlwaysOnTop => "hud.alwaysOnTop",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DictionarySense {
    pub language: String,
    pub text: String,
    pub usage_notes: Option<String>,
    pub source_locator: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    pub entry_id: String,
    pub lemma: String,
    pub reading: Option<String>,
    pub part_of_speech: Vec<String>,
    pub content_kind: String,
    pub source_label: String,
    pub source_version: String,
    pub license_ref: String,
    pub attribution: String,
    pub senses: Vec<DictionarySense>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WordLookup {
    pub unit_id: String,
    pub text_revision: u32,
    pub token_index: u32,
    pub token: super::LexicalToken,
    pub entries: Vec<DictionaryEntry>,
    pub resource_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReference {
    pub byte_start: u32,
    pub byte_end: u32,
    pub quote: String,
    pub explanation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StudyResult {
    pub task_id: String,
    pub unit_id: String,
    pub text_revision: u32,
    pub purpose: String,
    pub mode: String,
    pub text: String,
    pub references: Vec<AnalysisReference>,
    pub selection_byte_start: Option<u32>,
    pub selection_byte_end: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LearningObject {
    pub unit: super::ReadingUnit,
    pub token_index: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    pub task_id: String,
    pub unit_id: String,
    pub text_revision: u32,
    pub purpose: String,
    pub status: String,
    pub error_code: Option<String>,
    pub partial_text: Option<String>,
    pub provider_label: String,
    /// Actual latest attempt identity. Older serialized views remain readable.
    #[serde(default)]
    pub attempt_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationHeadSummary {
    pub head_count: String,
    pub queued_count: String,
    pub running_count: String,
    pub completed_count: String,
    pub failed_count: String,
    pub cancelled_count: String,
    pub invalidated_count: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TranslationEngineStatus {
    Unrequested,
    Current {
        task: TaskView,
    },
    Unordered {
        summary: TranslationHeadSummary,
        #[serde(rename = "activeTask")]
        active_task: Option<TaskView>,
    },
    Unavailable {
        error: super::Diagnostic,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationEngineProjection {
    pub scope_key: String,
    pub binding_id: String,
    pub engine_id: String,
    pub provider_label: String,
    pub status: TranslationEngineStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadingTranslationState {
    pub unit_id: String,
    pub text_revision: u32,
    pub source_hash: String,
    pub engines: Vec<TranslationEngineProjection>,
}

/// Read composition only: never persisted into a ReadingUnit or frozen object.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadingUnitProjection {
    pub profile_id: String,
    pub server_instance_id: String,
    pub profile_generation: u32,
    pub commit_revision: u32,
    pub unit: super::ReadingUnit,
    pub translation_state: ReadingTranslationState,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationTestRequest {
    pub revision_id: String,
    pub object_kind: String,
    pub object_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationTestResult {
    pub revision_id: String,
    pub object_kind: String,
    pub object_id: String,
    pub status: String,
    pub message: String,
    pub model_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInput {
    pub label: String,
    pub secret: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialReference {
    pub credential_id: String,
    pub credential_version_id: String,
    pub pool_id: String,
    pub pool_revision: u32,
    pub label: String,
    pub masked_hint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub kind: String,
    pub version: String,
    pub manifest_sha256: String,
    pub available: bool,
    pub detail: String,
}

/// A language request is private to the native worker; stdin/stdout carry bounded JSONL only.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageRequest {
    pub v: u32,
    pub request_id: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResponse {
    pub v: u32,
    pub request_id: String,
    pub resource_version: String,
    pub tokens: Vec<super::LexicalToken>,
    pub error: Option<String>,
}

pub fn typescript_declarations() -> String {
    [
        super::binding_journey::binding_journey_typescript_declarations(),
        TargetCandidate::decl(&ts_rs::Config::default()),
        TargetCandidates::decl(&ts_rs::Config::default()),
        AssociateRequest::decl(&ts_rs::Config::default()),
        ExecutableSelection::decl(&ts_rs::Config::default()),
        WindowRole::decl(&ts_rs::Config::default()),
        LoginStartupStatus::decl(&ts_rs::Config::default()),
        LoginStartupState::decl(&ts_rs::Config::default()),
        MainPageIntent::decl(&ts_rs::Config::default()),
        MainPageRequest::decl(&ts_rs::Config::default()),
        SettingsPageIntent::decl(&ts_rs::Config::default()),
        SettingsPageRequest::decl(&ts_rs::Config::default()),
        HudPreference::decl(&ts_rs::Config::default()),
        DictionarySense::decl(&ts_rs::Config::default()),
        DictionaryEntry::decl(&ts_rs::Config::default()),
        WordLookup::decl(&ts_rs::Config::default()),
        AnalysisReference::decl(&ts_rs::Config::default()),
        StudyResult::decl(&ts_rs::Config::default()),
        LearningObject::decl(&ts_rs::Config::default()),
        TaskView::decl(&ts_rs::Config::default()),
        TranslationHeadSummary::decl(&ts_rs::Config::default()),
        TranslationEngineStatus::decl(&ts_rs::Config::default()),
        TranslationEngineProjection::decl(&ts_rs::Config::default()),
        ReadingTranslationState::decl(&ts_rs::Config::default()),
        ReadingUnitProjection::decl(&ts_rs::Config::default()),
        ConfigurationTestRequest::decl(&ts_rs::Config::default()),
        ConfigurationTestResult::decl(&ts_rs::Config::default()),
        CredentialInput::decl(&ts_rs::Config::default()),
        CredentialReference::decl(&ts_rs::Config::default()),
        ResourceInfo::decl(&ts_rs::Config::default()),
        LanguageRequest::decl(&ts_rs::Config::default()),
        LanguageResponse::decl(&ts_rs::Config::default()),
    ]
    .iter()
    .map(|s| format!("\nexport {s}\n"))
    .collect()
}

#[cfg(test)]
mod main_page_navigation_tests {
    use super::{MainPageIntent, MainPageRequest};

    #[test]
    fn destinations_round_trip_without_changing_existing_wire_values() {
        for (intent, wire_value) in [
            (MainPageIntent::History, "history"),
            (MainPageIntent::Session, "session"),
            (MainPageIntent::Capture, "capture"),
        ] {
            let wire = serde_json::json!(wire_value);
            assert_eq!(serde_json::to_value(&intent).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<MainPageIntent>(wire).unwrap(),
                intent
            );
            for revision in [1, u32::MAX] {
                let request = MainPageRequest {
                    revision,
                    intent: intent.clone(),
                };
                let wire = serde_json::json!({ "revision": revision, "intent": wire_value });
                assert_eq!(serde_json::to_value(&request).unwrap(), wire);
                assert_eq!(
                    serde_json::from_value::<MainPageRequest>(wire).unwrap(),
                    request
                );
            }
        }
    }

    #[test]
    fn unknown_destinations_are_rejected_in_intents_and_requests() {
        for unknown in ["library", "unknown", "Session", "session ", ""] {
            assert!(serde_json::from_value::<MainPageIntent>(serde_json::json!(unknown)).is_err());
            let wire = serde_json::json!({ "revision": 1, "intent": unknown });
            assert!(serde_json::from_value::<MainPageRequest>(wire).is_err());
        }
    }
}
