//! Versioned, non-secret service descriptions. Catalog support is not saved-service eligibility.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolKey {
    pub protocol_id: String,
    pub contract_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityProfileKey {
    pub profile_id: String,
    pub profile_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderPresetKey {
    pub preset_id: String,
    pub preset_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdapterBinding {
    pub protocol: ProtocolKey,
    pub profile: CompatibilityProfileKey,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CatalogPurpose {
    Translation,
    Grammar,
    ContextWord,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Unsupported,
    Unknown,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogReason {
    AdapterNotImplemented,
    ExistingAdapterRequiresRegistryMigration,
    RequiresSavedConfiguration,
    OfflineRuntimeNotImplemented,
    PurposeNotSupported,
    ConditionUnresolved,
    PresetSiteMaterialIncomplete,
    ApplicationOauthSetupUnverified,
    UnsupportedField,
    RequiredFieldMissing,
    UnknownFieldValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogDecision {
    pub state: CapabilityState,
    pub reason: Option<CatalogReason>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogFamily {
    Llm,
    Translation,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogModelIdentity {
    Required,
    Optional,
    NotApplicable,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAuthKind {
    None,
    BearerApiKey,
    AnthropicApiKey,
    GoogleApiKey,
    AzureSubscriptionKey,
    DeeplAuthKey,
    GoogleOauth,
    GoogleServiceAccount,
    BaiduAppSecret,
    YoudaoAppSecret,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogPurposeCapability {
    pub purpose: CatalogPurpose,
    pub support: CatalogDecision,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogWireFormat {
    Json,
    Sse,
    TypedSse,
    Ndjson,
    FormJson,
    TranslationDelta,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolDescriptor {
    pub key: ProtocolKey,
    pub label: String,
    pub family: CatalogFamily,
    pub auth_kinds: Vec<CatalogAuthKind>,
    pub model_identity: CatalogModelIdentity,
    pub wire_formats: Vec<CatalogWireFormat>,
    pub allowed_purposes: Vec<CatalogPurpose>,
    pub capabilities: Vec<CatalogPurposeCapability>,
    pub implementation: CatalogDecision,
    pub source_refs: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityProfileDescriptor {
    pub key: CompatibilityProfileKey,
    pub protocol: ProtocolKey,
    pub allowed_purposes: Vec<CatalogPurpose>,
    pub auth_kinds: Vec<CatalogAuthKind>,
    pub option_field_ids: Vec<String>,
    pub source_refs: Vec<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogNamespace {
    Builtin,
    User,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFieldType {
    Text,
    Url,
    Secret,
    Select,
    Path,
    Number,
    Boolean,
    Textarea,
    Status,
    Action,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFieldClassification {
    Required,
    Conditional,
    Default,
    Advanced,
    Readonly,
    NotApplicable,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFieldGroup {
    Necessary,
    Advanced,
    Details,
    Guidance,
    Internal,
    Purpose,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(untagged)]
pub enum CatalogScalar {
    Text(String),
    Boolean(bool),
    Number(f64),
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceConditionOperator {
    Eq,
    In,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceConditionClause {
    pub field: String,
    pub operator: ServiceConditionOperator,
    pub values: Vec<CatalogScalar>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFieldCondition {
    pub all: Vec<ServiceConditionClause>,
    pub any: Vec<ServiceConditionClause>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFieldChoice {
    pub value: CatalogScalar,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceConditionalDefault {
    pub selector_value: CatalogScalar,
    pub value: CatalogScalar,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFieldDefaultBy {
    pub field: String,
    pub values: Vec<ServiceConditionalDefault>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFieldInheritance {
    SourceLanguage,
    TargetLanguage,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogDefaultEvidence {
    ReviewedOfficialSource,
    ReviewedSiteDefaultsOnly,
    ReviewedPlanDefaultsOnly,
    ApplicationOauthClientNotVerified,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFieldDescriptor {
    pub id: String,
    pub label: String,
    pub field_type: ServiceFieldType,
    pub classification: ServiceFieldClassification,
    pub group: Option<ServiceFieldGroup>,
    pub condition: ServiceFieldCondition,
    pub choices: Vec<ServiceFieldChoice>,
    pub default_value: Option<CatalogScalar>,
    pub default_by: Option<ServiceFieldDefaultBy>,
    pub default_evidence: Option<CatalogDefaultEvidence>,
    pub inherit_from: Option<ServiceFieldInheritance>,
    pub required_when_visible: bool,
    pub masked: bool,
    pub observed_only: bool,
    pub help: Option<String>,
    pub source_refs: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionalCatalogBlocker {
    pub condition: ServiceFieldCondition,
    pub reason: CatalogReason,
    pub message: String,
    pub source_refs: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderPresetDescriptor {
    pub key: ProviderPresetKey,
    pub namespace: CatalogNamespace,
    pub provider_id: String,
    pub brand_id: String,
    pub brand_label: String,
    pub label: String,
    pub binding: AdapterBinding,
    pub allowed_purposes: Vec<CatalogPurpose>,
    pub interface_label: String,
    pub interface_description: String,
    pub interface_choice_required: bool,
    pub default_auth_kind: CatalogAuthKind,
    pub default_endpoint: Option<String>,
    pub fields: Vec<ServiceFieldDescriptor>,
    pub material_blockers: Vec<ConditionalCatalogBlocker>,
    pub source_refs: Vec<String>,
    pub implementation: CatalogDecision,
    pub eligibility: CatalogDecision,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfflineExecutionBackend {
    PlatformNative,
    LocalWorker,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineEngineDescriptor {
    pub engine_id: String,
    pub contract_revision: u32,
    pub label: String,
    pub brand_id: String,
    pub brand_label: String,
    pub backend: OfflineExecutionBackend,
    pub allowed_purposes: Vec<CatalogPurpose>,
    pub model_identity: CatalogModelIdentity,
    pub fields: Vec<ServiceFieldDescriptor>,
    pub source_refs: Vec<String>,
    pub implementation: CatalogDecision,
    pub eligibility: CatalogDecision,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSourceKind {
    Official,
    OfficialSearchIndex,
    LocalContract,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogSource {
    pub source_id: String,
    pub kind: CatalogSourceKind,
    pub reference: String,
    pub checked_at: String,
    pub scope: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceCatalog {
    pub brands: Vec<CatalogBrandDescriptor>,
    pub schema_version: u32,
    pub protocols: Vec<ProtocolDescriptor>,
    pub profiles: Vec<CompatibilityProfileDescriptor>,
    pub presets: Vec<ProviderPresetDescriptor>,
    pub offline_engines: Vec<OfflineEngineDescriptor>,
    pub sources: Vec<CatalogSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogBrandDescriptor {
    pub brand_id: String,
    pub label: String,
}
/// Declarative import only. No secret, vault, headers, scripts or wire mapping exists in this shape.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserProviderPresetInput {
    pub provider_label: String,
    pub schema_version: u32,
    pub key: ProviderPresetKey,
    pub provider_id: String,
    pub display_name: String,
    pub binding: AdapterBinding,
    pub endpoint: Option<String>,
    pub auth_kind: CatalogAuthKind,
    pub options: BTreeMap<String, CatalogScalar>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedProviderDescriptor {
    pub protocol: ProtocolDescriptor,
    pub profile: CompatibilityProfileDescriptor,
    pub implementation: CatalogDecision,
}

pub(super) fn typescript_declarations() -> String {
    [
        StoredProviderPreset::decl(&ts_rs::Config::default()),
        ProtocolKey::decl(&ts_rs::Config::default()),
        CompatibilityProfileKey::decl(&ts_rs::Config::default()),
        ProviderPresetKey::decl(&ts_rs::Config::default()),
        AdapterBinding::decl(&ts_rs::Config::default()),
        CatalogPurpose::decl(&ts_rs::Config::default()),
        CapabilityState::decl(&ts_rs::Config::default()),
        CatalogReason::decl(&ts_rs::Config::default()),
        CatalogDecision::decl(&ts_rs::Config::default()),
        CatalogFamily::decl(&ts_rs::Config::default()),
        CatalogModelIdentity::decl(&ts_rs::Config::default()),
        CatalogAuthKind::decl(&ts_rs::Config::default()),
        CatalogPurposeCapability::decl(&ts_rs::Config::default()),
        CatalogWireFormat::decl(&ts_rs::Config::default()),
        ProtocolDescriptor::decl(&ts_rs::Config::default()),
        CompatibilityProfileDescriptor::decl(&ts_rs::Config::default()),
        CatalogNamespace::decl(&ts_rs::Config::default()),
        ServiceFieldType::decl(&ts_rs::Config::default()),
        ServiceFieldClassification::decl(&ts_rs::Config::default()),
        ServiceFieldGroup::decl(&ts_rs::Config::default()),
        CatalogScalar::decl(&ts_rs::Config::default()),
        ServiceConditionOperator::decl(&ts_rs::Config::default()),
        ServiceConditionClause::decl(&ts_rs::Config::default()),
        ServiceFieldCondition::decl(&ts_rs::Config::default()),
        ServiceFieldChoice::decl(&ts_rs::Config::default()),
        ServiceConditionalDefault::decl(&ts_rs::Config::default()),
        ServiceFieldDefaultBy::decl(&ts_rs::Config::default()),
        ServiceFieldInheritance::decl(&ts_rs::Config::default()),
        CatalogDefaultEvidence::decl(&ts_rs::Config::default()),
        ServiceFieldDescriptor::decl(&ts_rs::Config::default()),
        ConditionalCatalogBlocker::decl(&ts_rs::Config::default()),
        ProviderPresetDescriptor::decl(&ts_rs::Config::default()),
        OfflineExecutionBackend::decl(&ts_rs::Config::default()),
        OfflineEngineDescriptor::decl(&ts_rs::Config::default()),
        CatalogSourceKind::decl(&ts_rs::Config::default()),
        CatalogSource::decl(&ts_rs::Config::default()),
        CatalogBrandDescriptor::decl(&ts_rs::Config::default()),
        ServiceCatalog::decl(&ts_rs::Config::default()),
        UserProviderPresetInput::decl(&ts_rs::Config::default()),
        ResolvedProviderDescriptor::decl(&ts_rs::Config::default()),
    ]
    .iter()
    .map(|s| format!("\nexport {s}\n"))
    .collect()
}

/// Immutable storage source, not a copy of the UI presentation descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredProviderPreset {
    pub key: ProviderPresetKey,
    pub namespace: CatalogNamespace,
    pub provider_id: String,
    pub provider_label: String,
    pub display_name: String,
    pub binding: AdapterBinding,
    pub endpoint: Option<String>,
    pub auth_kind: String,
    pub options: BTreeMap<String, CatalogScalar>,
    pub source_refs: Vec<String>,
}
