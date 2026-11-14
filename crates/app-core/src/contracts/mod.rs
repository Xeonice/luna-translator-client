pub mod binding_journey;
pub use binding_journey::*;
pub mod google_authorization_ui;
pub use google_authorization_ui::*;
mod desktop;
pub mod service_catalog;
pub mod service_form;
pub mod service_configuration;
pub use service_configuration::*;
pub mod service_discovery;
pub use service_discovery::*;
pub use service_form::*;
pub use service_catalog::*;
pub mod game_assets;
pub use game_assets::*;
pub mod session_records;
pub use session_records::*;
pub mod window_capabilities;
pub use window_capabilities::*;
pub mod scoped_preferences;
pub use scoped_preferences::*;
pub mod game_window;
pub use game_window::*;
mod word_card;
pub use desktop::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;
pub use word_card::*;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub stage: String,
    pub message: String,
    pub recoverable: bool,
}
impl AppError {
    pub fn new(code: &str, stage: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            stage: stage.into(),
            message: message.into(),
            recoverable: true,
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    pub operation_id: String,
    pub expected_revision: Option<u32>,
    pub command: Command,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    SaveGame(Box<GameInput>),
    SaveGames {
        items: Vec<GameBatchItem>,
    },
    ArchiveGame {
        game_id: String,
        row_revision: u32,
    },
    ManualInput {
        text: String,
        source_language: String,
    },
    SelectSource {
        source_id: String,
    },
    SetCapturePaused {
        paused: bool,
    },
    Disconnect,
    UnbindWindow(GameWindowActivationRequest),
    RevokeCredential {
        credential_id: String,
        expected_lifecycle_revision: u32,
        expected_impact_hash: String,
    },
    SaveCredentialPool {
        pool: CredentialPoolInput,
    },
    SetTranslationEnabled {
        enabled: bool,
    },
    CopyModelConfiguration {
        model_id: String,
        new_display_name: String,
        expected_head_revision: u32,
    },
    SaveConfiguration {
        configuration: Configuration,
        expected_head_revision: u32,
    },
    ApplyConfiguration {
        revision_id: String,
        expected_head_revision: u32,
    },
    SetConnectionSuspended {
        connection_id: String,
        suspended: bool,
        expected_control_revision: u32,
    },
    SetPurposeRuntimeEnabled {
        scope_key: String,
        purpose: String,
        enabled: bool,
        expected_authorization_epoch: u32,
    },
    SavePreferences {
        preferences: Preferences,
    },
    SaveScopedPreferences {
        preferences: Preferences,
        expected_head_revision: Option<u32>,
    },
    CycleReadingDisplayMode,
    SetReadingAnnotations {
        show_ruby: bool,
        show_pos: bool,
    },
    SetHudPreference {
        preference: HudPreference,
        enabled: bool,
    },
    SaveCorrection {
        translation_id: String,
        base_revision: u32,
        text: String,
    },
    CancelTask {
        task_id: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandReceipt {
    #[serde(default)]
    pub replayed: bool,
    pub request_id: String,
    pub revision: u32,
    pub result_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub game_results: Option<Vec<GameSaveResult>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameBatchItem {
    pub item_id: String,
    pub game: GameInput,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameSaveResult {
    pub item_id: String,
    pub game_id: String,
    pub row_revision: u32,
    pub reused: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameInput {
    pub game_id: Option<String>,
    pub row_revision: Option<u32>,
    pub display_name: String,
    pub original_title: Option<String>,
    pub source_language: Option<String>,
    pub cover_asset_ref: Option<String>,
    pub target: GameTarget,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameTarget {
    pub environment_kind: String,
    pub environment_key: String,
    pub bottle_locator: Option<String>,
    pub launch_path: Option<String>,
    pub canonical_exe_path: Option<String>,
    pub target_path_key: Option<String>,
    pub working_directory: Option<String>,
    pub arguments: Vec<String>,
    pub architecture: Option<String>,
    pub verification_state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub game_id: String,
    pub display_name: String,
    pub original_title: Option<String>,
    pub source_language: Option<String>,
    pub cover_asset_ref: Option<String>,
    pub row_revision: u32,
    pub target: GameTarget,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSource {
    pub source_id: String,
    pub hook_name: String,
    pub hook_code: String,
    pub sample: String,
    pub selected: bool,
    pub encoding: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub session_id: String,
    pub game_id: Option<String>,
    pub source_kind: String,
    pub state: String,
    pub paused: bool,
    pub translation_enabled: bool,
    pub sources: Vec<CaptureSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LexicalToken {
    pub surface: String,
    pub lemma: String,
    pub reading: String,
    pub part_of_speech: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub utf16_start: u32,
    pub utf16_end: u32,
    pub known: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub translation_id: String,
    pub task_id: String,
    pub text: String,
    pub provider_label: String,
    pub target_language: String,
    pub correction: Option<Correction>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Correction {
    pub revision: u32,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadingUnit {
    #[serde(default)]
    pub source_summary: Option<HistorySessionSummary>,
    pub unit_id: String,
    pub session_id: String,
    pub game_id: Option<String>,
    pub text_revision: u32,
    pub source_text: String,
    pub source_hash: String,
    pub source_language: String,
    pub created_at_ms: f64,
    pub translations: Vec<Translation>,
    pub tokens: Vec<LexicalToken>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub units: Vec<ReadingUnit>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistorySourceKind {
    Hook,
    Manual,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub game_id: Option<String>,
    pub source_kind: Option<HistorySourceKind>,
    pub session_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub filter: HistoryFilter,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionFilter {
    pub game_id: Option<String>,
    pub source_kind: Option<HistorySourceKind>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionsQuery {
    pub filter: HistorySessionFilter,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionSummary {
    pub session_id: String,
    pub game_id: Option<String>,
    pub source_kind: HistorySourceKind,
    pub state: String,
    pub started_at_ms: f64,
    pub ended_at_ms: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionsPage {
    pub sessions: Vec<HistorySessionSummary>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterMode {
    Inherit,
    Set,
    Omit,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub mode: ParameterMode,
    pub value: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub connection_id: String,
    pub display_name: String,
    pub protocol: String,
    pub deployment: String,
    pub auth_mode: String,
    pub endpoint: Option<String>,
    pub credential_pool_id: Option<String>,
    pub credential_pool_revision: Option<u32>,
    pub proxy_mode: String,
    pub network_metadata: BTreeMap<String, serde_json::Value>,
    pub availability_intent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub adapter_binding: Option<AdapterBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preset_source: Option<ProviderPresetKey>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[serde(default)]
    pub default_templates: BTreeMap<String, String>,
    pub model_id: String,
    pub connection_id: String,
    pub actual_model_id: Option<String>,
    pub display_name: String,
    pub selection_source: String,
    pub capabilities: BTreeMap<String, serde_json::Value>,
    pub parameters: BTreeMap<String, Parameter>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    pub engine_id: String,
    pub model_id: String,
    pub enabled: bool,
    pub template_id: Option<String>,
    pub parameters: BTreeMap<String, Parameter>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PurposeBinding {
    pub binding_id: String,
    pub scope_key: String,
    pub game_id: Option<String>,
    pub purpose: String,
    pub binding_mode: String,
    pub enabled: bool,
    pub output_mode: Option<String>,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    pub template_id: Option<String>,
    pub context: BTreeMap<String, serde_json::Value>,
    pub schedule: BTreeMap<String, serde_json::Value>,
    pub parameters: BTreeMap<String, Parameter>,
    pub engines: Vec<Engine>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub template_id: String,
    pub purpose: String,
    pub display_name: String,
    pub source_kind: String,
    pub source_version: Option<String>,
    pub body: BTreeMap<String, serde_json::Value>,
    pub variables: Vec<String>,
    pub compatibility: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Term {
    pub term_id: String,
    pub source_text: Option<String>,
    pub target_text: Option<String>,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TermSet {
    pub term_set_id: String,
    pub scope_key: String,
    pub game_id: Option<String>,
    pub display_name: String,
    pub enabled: bool,
    pub terms: Vec<Term>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub revision_id: String,
    pub connections: Vec<Connection>,
    pub models: Vec<Model>,
    pub bindings: Vec<PurposeBinding>,
    pub templates: Vec<Template>,
    pub term_sets: Vec<TermSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub codec_version: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationState {
    pub draft: ConfigurationDocument,
    pub applied: Option<ConfigurationDocument>,
    pub draft_head_revision: u32,
    pub applied_head_revision: u32,
    pub runtime_connections: Vec<ConnectionRuntimeView>,
    pub runtime_purposes: Vec<PurposeRuntimeView>,
}

/// These are local authorization gates, not a claim of remote service availability.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRuntimeView {
    pub connection_id: String,
    pub config_revision_id: String,
    pub control_revision: u32,
    pub suspended: bool,
    pub reason_code: Option<String>,
    pub availability_intent: bool,
    pub affected_purposes: Vec<PurposeImpact>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PurposeImpact {
    pub scope_key: String,
    pub purpose: String,
    pub binding_id: String,
    pub enabled_intent: bool,
    pub runtime_enabled: bool,
    pub affected_game_ids: Vec<String>,
    pub engine_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PurposeRuntimeView {
    pub scope_key: String,
    pub purpose: String,
    pub binding_id: String,
    pub config_revision_id: String,
    pub authorization_epoch: u32,
    pub enabled_intent: bool,
    /// Independent safety permission; enabled intent and connection/credential gates also apply.
    pub runtime_enabled: bool,
    pub blocked_connection_ids: Vec<String>,
    pub authorized_engine_ids: Vec<String>,
    pub affected_game_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRevocationRequest {
    pub operation_id: String,
    pub credential_id: String,
    pub expected_lifecycle_revision: u32,
    pub expected_impact_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRevocationImpact {
    pub config_revision_id: Option<String>,
    pub impact_hash: String,
    pub credential_id: String,
    pub lifecycle_revision: u32,
    pub state: String,
    pub affected_connection_ids: Vec<String>,
    pub affected_purposes: Vec<PurposeImpact>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadingDisplayMode {
    #[default]
    Both,
    Source,
    Translation,
}
impl ReadingDisplayMode {
    pub fn next(self) -> Self {
        match self {
            Self::Both => Self::Source,
            Self::Source => Self::Translation,
            Self::Translation => Self::Both,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LexicalPresentationState {
    Pending,
    Ready,
    ResourceUnavailable,
    WorkerFailed,
    AlignmentInvalid,
    ReadingUnavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LexicalState {
    pub unit_id: String,
    pub text_revision: u32,
    pub source_hash: String,
    pub state: LexicalPresentationState,
    pub error_code: Option<String>,
}
fn default_segmentation_enabled() -> bool {
    true
}
fn default_hud_background_opacity() -> f64 {
    0.70
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    #[serde(default = "default_segmentation_enabled")]
    pub segmentation_enabled: bool,
    #[serde(default)]
    pub reading_display_mode: ReadingDisplayMode,
    #[serde(default = "default_hud_background_opacity")]
    pub hud_background_opacity: f64,
    pub scope_key: String,
    pub theme: String,
    pub transparency_mode: String,
    pub show_ruby: bool,
    pub show_pos: bool,
    pub source_font_pt: f64,
    pub translation_font_pt: f64,
    pub hud_width_pt: f64,
    pub shortcuts: BTreeMap<String, String>,
    pub overrides: BTreeMap<String, bool>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            segmentation_enabled: true,
            reading_display_mode: ReadingDisplayMode::Both,
            hud_background_opacity: 0.70,
            scope_key: "global".into(),
            theme: "system".into(),
            transparency_mode: "material".into(),
            show_ruby: true,
            show_pos: false,
            source_font_pt: 18.,
            translation_font_pt: 24.,
            hud_width_pt: 560.,
            shortcuts: BTreeMap::new(),
            overrides: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub stage: String,
    pub message: String,
    pub recoverable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    #[serde(default)]
    pub lexical_states: Vec<LexicalState>,
    pub profile_id: String,
    pub server_instance_id: String,
    pub profile_generation: u32,
    pub revision: u32,
    pub commit_revision: u32,
    pub games: Vec<Game>,
    pub session: Option<Session>,
    pub current: Option<ReadingUnit>,
    pub history: HistoryPage,
    pub configuration: ConfigurationState,
    pub preferences: Preferences,
    pub diagnostics: Vec<Diagnostic>,
    pub tasks: Vec<TaskView>,
    pub study_results: Vec<StudyResult>,
    pub credentials: Vec<CredentialMetadata>,
    pub credential_pools: Vec<CredentialPool>,
    pub configuration_tests: Vec<ConfigurationTestView>,
    pub learning_object: Option<LearningObject>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionUpdate {
    pub projection_id: String,
    pub server_instance_id: String,
    pub profile_generation: u32,
    pub base_revision: u32,
    pub revision: u32,
    pub snapshot: AppSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationTestCoverage {
    LegacyCompared,
    TypedCompared,
    NotCovered,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationTestView {
    pub test_id: String,
    pub revision_id: String,
    pub object_kind: String,
    pub object_id: String,
    pub semantics_hash: String,
    pub status: String,
    pub completed_at_ms: f64,
    pub error_code: Option<String>,
    pub credential_ids: Vec<String>,
    pub stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub comparison_coverage: Option<ConfigurationTestCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialMetadata {
    pub credential_id: String,
    pub masked_label: String,
    pub state: String,
    pub lifecycle_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPoolMemberInput {
    pub member_id: Option<String>,
    pub credential_id: String,
    pub enabled: bool,
    pub weight: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPoolInput {
    pub pool_id: Option<String>,
    pub expected_revision: u32,
    pub policy: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub members: Vec<CredentialPoolMemberInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPoolMember {
    pub member_id: String,
    pub credential_id: String,
    pub position: u32,
    pub enabled: bool,
    pub weight: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPool {
    pub pool_id: String,
    pub revision: u32,
    pub policy: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub members: Vec<CredentialPoolMember>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionExportStatus {
    Completed,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionExportReceipt {
    pub export_id: String,
    pub status: SessionExportStatus,
    pub record_count: String,
    pub destination_label: Option<String>,
    pub content_sha256: Option<String>,
}
pub fn typescript_declarations() -> String {
    let declarations = [
        GameCoverOrigin::decl(&ts_rs::Config::default()),
        GameCoverStatus::decl(&ts_rs::Config::default()),
        GameCoverRequest::decl(&ts_rs::Config::default()),
        GameCoverResult::decl(&ts_rs::Config::default()),
        <serde_json::Value as TS>::decl(&ts_rs::Config::default()),
        CredentialMetadata::decl(&ts_rs::Config::default()),
        CredentialPoolMemberInput::decl(&ts_rs::Config::default()),
        CredentialPoolInput::decl(&ts_rs::Config::default()),
        CredentialPoolMember::decl(&ts_rs::Config::default()),
        CredentialPool::decl(&ts_rs::Config::default()),
        ConfigurationTestCoverage::decl(&ts_rs::Config::default()),
        ConfigurationTestView::decl(&ts_rs::Config::default()),
        ShortcutIntent::decl(&ts_rs::Config::default()),
        ShortcutState::decl(&ts_rs::Config::default()),
        GeometryState::decl(&ts_rs::Config::default()),
        MaterialEffect::decl(&ts_rs::Config::default()),
        PreviewConsumer::decl(&ts_rs::Config::default()),
        PreviewControl::decl(&ts_rs::Config::default()),
        PreviewPermission::decl(&ts_rs::Config::default()),
        PreviewState::decl(&ts_rs::Config::default()),
        ShortcutBindingProjection::decl(&ts_rs::Config::default()),
        ShortcutProjection::decl(&ts_rs::Config::default()),
        LogicalBounds::decl(&ts_rs::Config::default()),
        GeometryProjection::decl(&ts_rs::Config::default()),
        MaterialProjection::decl(&ts_rs::Config::default()),
        WindowCapabilities::decl(&ts_rs::Config::default()),
        PreviewStartRequest::decl(&ts_rs::Config::default()),
        PreviewControlRequest::decl(&ts_rs::Config::default()),
        PreviewCapability::decl(&ts_rs::Config::default()),
        PreviewProjection::decl(&ts_rs::Config::default()),
        PreviewFrame::decl(&ts_rs::Config::default()),
        PreviewUpdate::decl(&ts_rs::Config::default()),
        SessionDataset::decl(&ts_rs::Config::default()),
        SessionRecordOrder::decl(&ts_rs::Config::default()),
        SessionRecordFilter::decl(&ts_rs::Config::default()),
        SessionRecordQuery::decl(&ts_rs::Config::default()),
        SequenceRange::decl(&ts_rs::Config::default()),
        SessionCompleteness::decl(&ts_rs::Config::default()),
        SessionRangeSnapshot::decl(&ts_rs::Config::default()),
        BodySourceReference::decl(&ts_rs::Config::default()),
        SessionRecord::decl(&ts_rs::Config::default()),
        BodyRecordReference::decl(&ts_rs::Config::default()),
        SessionRecordPage::decl(&ts_rs::Config::default()),
        SessionExportRequest::decl(&ts_rs::Config::default()),
        SessionExportPlan::decl(&ts_rs::Config::default()),
        SessionExportStatus::decl(&ts_rs::Config::default()),
        SessionExportReceipt::decl(&ts_rs::Config::default()),
        AppError::decl(&ts_rs::Config::default()),
        CommandEnvelope::decl(&ts_rs::Config::default()),
        Command::decl(&ts_rs::Config::default()),
        CommandReceipt::decl(&ts_rs::Config::default()),
        GameBatchItem::decl(&ts_rs::Config::default()),
        GameSaveResult::decl(&ts_rs::Config::default()),
        GameInput::decl(&ts_rs::Config::default()),
        GameTarget::decl(&ts_rs::Config::default()),
        Game::decl(&ts_rs::Config::default()),
        CaptureSource::decl(&ts_rs::Config::default()),
        Session::decl(&ts_rs::Config::default()),
        LexicalToken::decl(&ts_rs::Config::default()),
        Translation::decl(&ts_rs::Config::default()),
        Correction::decl(&ts_rs::Config::default()),
        ReadingUnit::decl(&ts_rs::Config::default()),
        HistoryPage::decl(&ts_rs::Config::default()),
        HistorySourceKind::decl(&ts_rs::Config::default()),
        HistoryFilter::decl(&ts_rs::Config::default()),
        HistoryQuery::decl(&ts_rs::Config::default()),
        HistorySessionFilter::decl(&ts_rs::Config::default()),
        HistorySessionsQuery::decl(&ts_rs::Config::default()),
        HistorySessionSummary::decl(&ts_rs::Config::default()),
        HistorySessionsPage::decl(&ts_rs::Config::default()),
        ParameterMode::decl(&ts_rs::Config::default()),
        Parameter::decl(&ts_rs::Config::default()),
        Connection::decl(&ts_rs::Config::default()),
        Model::decl(&ts_rs::Config::default()),
        Engine::decl(&ts_rs::Config::default()),
        PurposeBinding::decl(&ts_rs::Config::default()),
        Template::decl(&ts_rs::Config::default()),
        Term::decl(&ts_rs::Config::default()),
        TermSet::decl(&ts_rs::Config::default()),
        Configuration::decl(&ts_rs::Config::default()),
        CredentialRevocationRequest::decl(&ts_rs::Config::default()),
        CredentialRevocationImpact::decl(&ts_rs::Config::default()),
        PurposeImpact::decl(&ts_rs::Config::default()),
        ConnectionRuntimeView::decl(&ts_rs::Config::default()),
        PurposeRuntimeView::decl(&ts_rs::Config::default()),
        ConfigurationState::decl(&ts_rs::Config::default()),
        ReadingDisplayMode::decl(&ts_rs::Config::default()),
        LexicalPresentationState::decl(&ts_rs::Config::default()),
        LexicalState::decl(&ts_rs::Config::default()),
        Preferences::decl(&ts_rs::Config::default()),
        PreferenceField::decl(&ts_rs::Config::default()),
        ScopedPreferences::decl(&ts_rs::Config::default()),
        GameWindowBindingState::decl(&ts_rs::Config::default()),
        GameWindowBindingProjection::decl(&ts_rs::Config::default()),
        GameWindowActivationState::decl(&ts_rs::Config::default()),
        GameWindowActivationReceipt::decl(&ts_rs::Config::default()),
        GameWindowActivationRequest::decl(&ts_rs::Config::default()),
        Diagnostic::decl(&ts_rs::Config::default()),
        AppSnapshot::decl(&ts_rs::Config::default()),
        ProjectionUpdate::decl(&ts_rs::Config::default()),
    ];
    format!(
        "// Generated by app-core. Do not edit.\n{}\n{}{}",
        declarations
            .iter()
            .map(|x| format!("export {x}"))
            .collect::<Vec<_>>()
            .join("\n\n"),
        desktop::typescript_declarations(),
        google_authorization_ui::typescript_declarations() + &word_card::typescript_declarations() + &service_configuration::typescript_declarations() + &crate::domain::model_config::service_graph::typescript_declarations() + &service_catalog::typescript_declarations() + &service_form::typescript_declarations() + &service_discovery::typescript_declarations()
    )
}
