//! Reviewed association and atomic reading selection. Legacy desktop DTOs stay unchanged.
use super::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SOURCE_SELECTION_MEMBER_LIMIT: u32 = 64;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingProfileContext {
    pub profile_id: String,
    pub server_instance_id: String,
    pub profile_generation: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssociationAttributionIntent {
    AutoMatch,
    ExplicitGame,
    SessionOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssociationReviewQuery {
    pub candidate_id: String,
    pub game_id: Option<String>,
    pub expected_profile_generation: u32,
    pub expected_active_session_id: Option<String>,
    /// Required when continuing a reviewed new-library-item flow after saving.
    #[serde(default)]
    #[ts(optional)]
    pub prior_review_id: Option<String>,
    /// Only explicit auto_match may infer a unique existing library owner.
    #[serde(default)]
    #[ts(optional)]
    pub attribution_intent: Option<AssociationAttributionIntent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssociationRelation {
    None,
    Reuse,
    Switch,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssociationTargetIdentity {
    pub candidate_id: String,
    pub environment_key: String,
    pub executable_path: String,
    pub windows_pid: u32,
    pub process_created_u64: String,
    pub architecture: String,
    pub target_identity_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssociationReview {
    pub review_id: String,
    pub profile: BindingProfileContext,
    pub relation: AssociationRelation,
    pub candidate: AssociationTargetIdentity,
    pub game_id: Option<String>,
    pub active_session_id: Option<String>,
    pub active_target_identity_sha256: Option<String>,
    pub detail: String,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedAssociateRequest {
    pub operation_id: String,
    pub review_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssociationStage {
    Verifying,
    PreparingResources,
    RequestingHook,
    WaitingHandshake,
    WaitingText,
    CandidatesReady,
    Failed,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssociationOperationReceipt {
    pub operation_id: String,
    pub review_id: String,
    pub profile: BindingProfileContext,
    pub relation: AssociationRelation,
    pub session_id: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssociationProgressQuery {
    pub operation_id: String,
    pub expected_profile_generation: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssociationProgressFact {
    pub event_id: String,
    pub sequence: String,
    pub observed_at_ms: String,
    pub stage: AssociationStage,
    pub level: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssociationProgress {
    pub operation_id: String,
    pub profile: BindingProfileContext,
    pub session_id: Option<String>,
    pub generation: String,
    pub stage: AssociationStage,
    pub facts: Vec<AssociationProgressFact>,
    pub superseded: bool,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSelectionQuery {
    pub session_id: String,
    pub expected_profile_generation: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionPersistence {
    Temporary,
    SavedSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceSelectionState {
    Unselected,
    Current,
    RequiresReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSelectionMember {
    pub source_id: String,
    pub selection_revision: u32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSelectionSet {
    pub session_id: String,
    /// Canonical decimal SQLite revision; never converted through a JS number.
    pub set_revision: String,
    pub operation_id: Option<String>,
    pub persistence: SelectionPersistence,
    pub selected_at_ms: String,
    pub members: Vec<SourceSelectionMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetailView {
    pub source_id: String,
    pub hook_name: String,
    pub hook_code: String,
    pub address: Option<String>,
    pub context: Option<String>,
    pub context2: Option<String>,
    pub encoding: String,
    pub latest_sample: String,
    pub stream_identity_sha256: String,
    pub identity_extensions_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedSourceSelection {
    pub set: SourceSelectionSet,
    pub matcher_version: Option<u32>,
    pub state: SourceSelectionState,
    pub reason: String,
    pub sources: Vec<SourceDetailView>,
    /// Preserve unknown/legacy matcher bytes; this is not an automatic-match grant.
    pub match_snapshot_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSelectionProjection {
    pub profile: BindingProfileContext,
    pub session_id: String,
    pub state: SourceSelectionState,
    pub current: Option<SourceSelectionSet>,
    pub sources: Vec<SourceDetailView>,
    pub saved: Option<SavedSourceSelection>,
    pub remember_allowed: bool,
    pub member_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BeginReadingRequest {
    pub operation_id: String,
    pub session_id: String,
    pub expected_profile_generation: u32,
    /// None is a CAS expectation of no sealed set, not an omitted check.
    pub expected_set_revision: Option<String>,
    pub source_ids: Vec<String>,
    pub remember: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingWindowEffectAuthority {
    pub profile: BindingProfileContext,
    pub session_id: String,
    pub set_revision: String,
    pub operation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCommitReceipt {
    pub operation_id: String,
    pub profile: BindingProfileContext,
    pub selection: SourceSelectionSet,
    pub replayed: bool,
    pub effect_authority: ReadingWindowEffectAuthority,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeWindowStepState {
    NotAttempted,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowStepResult {
    pub state: NativeWindowStepState,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadingWindowEffectsResult {
    pub authority: ReadingWindowEffectAuthority,
    pub show_hud: NativeWindowStepResult,
    pub hide_main: NativeWindowStepResult,
    pub retryable: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BeginReadingResult {
    pub selection: SelectionCommitReceipt,
    pub effects: ReadingWindowEffectsResult,
}

/// Typed saved carrier. It records descriptors, never portable automatic matching proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedSelectionSnapshotV1 {
    pub schema_version: u32,
    pub match_capability: SavedSelectionMatchCapability,
    pub target_identity_sha256: String,
    pub sources: Vec<SavedSourceDescriptor>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SavedSelectionMatchCapability {
    RequiresReview,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedSourceDescriptor {
    pub source_id: String,
    pub stream_identity_sha256: String,
    pub hook_name: String,
    pub hook_code: String,
    pub encoding: String,
    pub address: Option<String>,
    pub context: Option<String>,
    pub context2: Option<String>,
    pub identity_extensions_json: String,
}

pub fn binding_journey_unavailable() -> AppError {
    AppError::new(
        "binding_journey_unavailable",
        "capture",
        "当前适配器尚未提供完整绑定旅程",
    )
}

pub fn canonical_binding_decimal(value: &str) -> AppResult<u64> {
    if value.is_empty()
        || value.len() > 20
        || value != "0" && value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AppError::new(
            "invalid_decimal_identity",
            "capture",
            "身份或版本必须使用规范十进制字符串",
        ));
    }
    value
        .parse()
        .map_err(|_| AppError::new("invalid_decimal_identity", "capture", "身份或版本超出范围"))
}

pub fn binding_journey_typescript_declarations() -> String {
    [
        BindingProfileContext::decl(&ts_rs::Config::default()),
        AssociationAttributionIntent::decl(&ts_rs::Config::default()),
        AssociationReviewQuery::decl(&ts_rs::Config::default()),
        AssociationRelation::decl(&ts_rs::Config::default()),
        AssociationTargetIdentity::decl(&ts_rs::Config::default()),
        AssociationReview::decl(&ts_rs::Config::default()),
        ReviewedAssociateRequest::decl(&ts_rs::Config::default()),
        AssociationStage::decl(&ts_rs::Config::default()),
        AssociationOperationReceipt::decl(&ts_rs::Config::default()),
        AssociationProgressQuery::decl(&ts_rs::Config::default()),
        AssociationProgressFact::decl(&ts_rs::Config::default()),
        AssociationProgress::decl(&ts_rs::Config::default()),
        SourceSelectionQuery::decl(&ts_rs::Config::default()),
        SelectionPersistence::decl(&ts_rs::Config::default()),
        SourceSelectionState::decl(&ts_rs::Config::default()),
        SourceSelectionMember::decl(&ts_rs::Config::default()),
        SourceSelectionSet::decl(&ts_rs::Config::default()),
        SourceDetailView::decl(&ts_rs::Config::default()),
        SavedSourceSelection::decl(&ts_rs::Config::default()),
        SourceSelectionProjection::decl(&ts_rs::Config::default()),
        BeginReadingRequest::decl(&ts_rs::Config::default()),
        ReadingWindowEffectAuthority::decl(&ts_rs::Config::default()),
        SelectionCommitReceipt::decl(&ts_rs::Config::default()),
        NativeWindowStepState::decl(&ts_rs::Config::default()),
        NativeWindowStepResult::decl(&ts_rs::Config::default()),
        ReadingWindowEffectsResult::decl(&ts_rs::Config::default()),
        BeginReadingResult::decl(&ts_rs::Config::default()),
    ]
    .join("\n\nexport ")
}
