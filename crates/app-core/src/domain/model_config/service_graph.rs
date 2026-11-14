//! Version-three service graph. Pure data and policy; no storage or runtime authority.
//! Existing Configuration/Engine grammars remain separate and unchanged.
use crate::contracts::{AdapterBinding, CatalogPurpose, ProviderPresetKey};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, num::NonZeroU32};
use ts_rs::TS;

#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpConfigRef {
    pub revision_id: String,
    pub model_id: String,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineConfigRef {
    pub revision_id: String,
    pub config_id: String,
}
/// A saved source version is never a local engine target or proof of eligibility.
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SavedServiceRef {
    Http { source: HttpConfigRef },
    Offline { source: OfflineConfigRef },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EngineTarget {
    Http { model_id: String },
    Offline { offline_config_id: String },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AzureResourceType {
    Global,
    Regional,
    MultiService,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeeplPlan {
    Free,
    Pro,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "protocol",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HttpServiceOptionsV1 {
    OpenaiCompatible {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        organization_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        project_id: Option<String>,
    },
    Anthropic {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        workspace_id: Option<String>,
    },
    AzureTranslator {
        /// Missing is an unfinished draft; execution must require an explicit resource type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        resource_type: Option<AzureResourceType>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        region: Option<String>,
    },
    OpenaiResponses {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        organization_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        project_id: Option<String>,
    },
    GeminiGenerateContent {},
    OllamaNative {},
    LmstudioNativeV1 {},
    DeeplV2 {
        /// No first-option fallback: Free/Pro remains unchosen until the user selects it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        plan: Option<DeeplPlan>,
    },
    GoogleTranslateV2 {},
    GoogleTranslateV3 {
        project_id: String,
        location: String,
    },
    BaiduVip {},
    YoudaoV3 {},
    TencentTokenhubTranslation {},
}
impl HttpServiceOptionsV1 {
    pub fn protocol_id(&self) -> &'static str {
        match self {
            Self::OpenaiCompatible { .. } => "openai_compatible",
            Self::Anthropic { .. } => "anthropic",
            Self::AzureTranslator { .. } => "azure_translator",
            Self::OpenaiResponses { .. } => "openai_responses",
            Self::GeminiGenerateContent { .. } => "gemini_generate_content",
            Self::OllamaNative { .. } => "ollama_native",
            Self::LmstudioNativeV1 { .. } => "lmstudio_native_v1",
            Self::DeeplV2 { .. } => "deepl_v2",
            Self::GoogleTranslateV2 { .. } => "google_translate_v2",
            Self::GoogleTranslateV3 { .. } => "google_translate_v3",
            Self::BaiduVip { .. } => "baidu_vip",
            Self::YoudaoV3 { .. } => "youdao_v3",
            Self::TencentTokenhubTranslation { .. } => "tencent_tokenhub_translation",
        }
    }
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConnectionOrigin {
    Preset {
        key: ProviderPresetKey,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        site: Option<String>,
    },
    Custom {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        copied_from: Option<ProviderPresetKey>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        site: Option<String>,
    },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleAuthorizationRef {
    pub grant_id: String,
    pub authorization_epoch: NonZeroU32,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionSourceRef {
    pub revision_id: String,
    pub connection_id: String,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionV3 {
    pub connection_id: String,
    pub display_name: String,
    pub protocol: String,
    pub deployment: String,
    pub auth_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub credential_pool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub credential_pool_revision: Option<NonZeroU32>,
    pub proxy_mode: String,
    pub network_metadata: BTreeMap<String, serde_json::Value>,
    pub availability_intent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub adapter_binding: Option<AdapterBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preset_source: Option<ProviderPresetKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub http_options: Option<HttpServiceOptionsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub origin: Option<ConnectionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub google_authorization: Option<GoogleAuthorizationRef>,
    /// A claimed internal origin requires repository proof; imports cannot grant it to themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub materialization_source: Option<ConnectionSourceRef>,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicyOverride<T> {
    Inherit {},
    Set { value: T },
}
impl<T> Default for PolicyOverride<T> {
    fn default() -> Self {
        Self::Inherit {}
    }
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryInputMode {
    Original,
    Translation,
    Both,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceContextConsent {
    pub enabled: bool,
    pub max_sentences: u8,
    pub mode: HistoryInputMode,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceExecutionDefaults {
    pub timeout_ms: u32,
    pub concurrency: u8,
    pub retries: u8,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceCachePolicy {
    pub enabled: bool,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SourceLanguageChoice {
    InheritSource {},
    Explicit { language: String },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TargetLanguageChoice {
    InheritPreference {},
    Explicit { language: String },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServiceLanguageDefaults {
    Http {
        source: SourceLanguageChoice,
        target: TargetLanguageChoice,
    },
    OfflineUseConfigPair {},
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRequestDefaults {
    pub purpose: CatalogPurpose,
    pub languages: ServiceLanguageDefaults,
    pub context: ServiceContextConsent,
    pub execution: ServiceExecutionDefaults,
    pub cache: ServiceCachePolicy,
    pub fallback: Vec<SavedServiceRef>,
}
#[derive(TS, Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionPolicyOverrides {
    pub timeout_ms: PolicyOverride<u32>,
    pub concurrency: PolicyOverride<u8>,
    pub retries: PolicyOverride<u8>,
}
#[derive(TS, Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyOverrides {
    pub context: PolicyOverride<ServiceContextConsent>,
    pub execution: ExecutionPolicyOverrides,
    pub cache: PolicyOverride<ServiceCachePolicy>,
}
#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    Service,
    Scope,
    Engine,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectiveServicePolicy {
    pub context: ServiceContextConsent,
    pub execution: ServiceExecutionDefaults,
    pub cache: ServiceCachePolicy,
    pub sources: BTreeMap<String, PolicySource>,
}
/// Explicit false/zero remain Set values. Validation is separate from precedence calculation.
pub fn effective_policy(
    defaults: &ServiceRequestDefaults,
    scope: &PolicyOverrides,
    engine: &PolicyOverrides,
) -> EffectiveServicePolicy {
    fn value<T: Clone>(
        key: &str,
        default: &T,
        scope: &PolicyOverride<T>,
        engine: &PolicyOverride<T>,
        sources: &mut BTreeMap<String, PolicySource>,
    ) -> T {
        let (value, source) = match engine {
            PolicyOverride::Set { value } => (value, PolicySource::Engine),
            PolicyOverride::Inherit {} => match scope {
                PolicyOverride::Set { value } => (value, PolicySource::Scope),
                PolicyOverride::Inherit {} => (default, PolicySource::Service),
            },
        };
        sources.insert(key.into(), source);
        value.clone()
    }
    let mut sources = BTreeMap::new();
    EffectiveServicePolicy {
        context: value(
            "context",
            &defaults.context,
            &scope.context,
            &engine.context,
            &mut sources,
        ),
        execution: ServiceExecutionDefaults {
            timeout_ms: value(
                "timeoutMs",
                &defaults.execution.timeout_ms,
                &scope.execution.timeout_ms,
                &engine.execution.timeout_ms,
                &mut sources,
            ),
            concurrency: value(
                "concurrency",
                &defaults.execution.concurrency,
                &scope.execution.concurrency,
                &engine.execution.concurrency,
                &mut sources,
            ),
            retries: value(
                "retries",
                &defaults.execution.retries,
                &scope.execution.retries,
                &engine.execution.retries,
                &mut sources,
            ),
        },
        cache: value(
            "cache",
            &defaults.cache,
            &scope.cache,
            &engine.cache,
            &mut sources,
        ),
        sources,
    }
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum GenerationParameter {
    Inherit {},
    Omit {},
    Set { value: serde_json::Value },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelV3 {
    pub default_templates: BTreeMap<CatalogPurpose, String>,
    pub model_id: String,
    pub connection_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub actual_model_id: Option<String>,
    pub display_name: String,
    pub selection_source: String,
    pub capabilities: BTreeMap<String, serde_json::Value>,
    pub parameters: BTreeMap<String, GenerationParameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub template_purpose: Option<CatalogPurpose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub legacy_eligibility_origin: Option<HttpConfigRef>,
    pub request_defaults: BTreeMap<CatalogPurpose, ServiceRequestDefaults>,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineV3 {
    pub engine_id: String,
    pub target: EngineTarget,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub http_template_override: Option<String>,
    pub http_parameters: BTreeMap<String, GenerationParameter>,
    pub policy_overrides: PolicyOverrides,
}
#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PurposeSemanticsVersion {
    LegacyV2,
    TypedV3,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PurposeBindingV3 {
    pub binding_id: String,
    pub scope_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub game_id: Option<String>,
    pub purpose: CatalogPurpose,
    pub binding_mode: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub output_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub template_id: Option<String>,
    pub context: BTreeMap<String, serde_json::Value>,
    pub schedule: BTreeMap<String, serde_json::Value>,
    pub parameters: BTreeMap<String, GenerationParameter>,
    pub engines: Vec<EngineV3>,
    pub semantics_version: PurposeSemanticsVersion,
    pub scope_overrides: PolicyOverrides,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateV3 {
    pub template_id: String,
    pub purpose: CatalogPurpose,
    pub display_name: String,
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_version: Option<String>,
    pub body: BTreeMap<String, serde_json::Value>,
    pub variables: Vec<String>,
    pub compatibility: BTreeMap<String, serde_json::Value>,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermV3 {
    pub term_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_language: Option<String>,
    pub enabled: bool,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermSetV3 {
    pub term_set_id: String,
    pub scope_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub game_id: Option<String>,
    pub display_name: String,
    pub enabled: bool,
    pub terms: Vec<TermV3>,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineEngineKey {
    pub engine_id: String,
    pub contract_revision: NonZeroU32,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfflineBackend {
    PlatformNative,
    LocalWorker,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OfflineRoutePolicy {
    DirectOnly {},
    ExplicitPivot { consent_revision: NonZeroU32 },
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguagePair {
    pub source: String,
    pub target: String,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalInstallationRef {
    pub installation_id: String,
    pub revision: NonZeroU32,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OfflineConfigResolution {
    Draft {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        observed_resource: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        proposed_route: Option<String>,
    },
    Resolved {
        resource_snapshot_id: String,
        route_id: String,
    },
}
impl OfflineConfigResolution {
    pub fn is_structurally_resolved(&self) -> bool {
        matches!(self, Self::Resolved { .. })
    }
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineEngineConfigV1 {
    pub id: String,
    pub engine: OfflineEngineKey,
    pub backend: OfflineBackend,
    pub display_name: String,
    pub pair: LanguagePair,
    pub route_policy: OfflineRoutePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub installation: Option<LocalInstallationRef>,
    pub resolution: OfflineConfigResolution,
    pub request_defaults: ServiceRequestDefaults,
}
#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationV3 {
    pub revision_id: String,
    pub connections: Vec<ConnectionV3>,
    pub models: Vec<ModelV3>,
    pub bindings: Vec<PurposeBindingV3>,
    pub templates: Vec<TemplateV3>,
    pub term_sets: Vec<TermSetV3>,
    pub offline_engine_configs: Vec<OfflineEngineConfigV1>,
    pub codec_version: u32,
}
/// Resolve explicit game override or global inheritance without changing intent.
pub fn effective_service_binding<'a>(
    graph: &'a ConfigurationV3,
    game_id: Option<&str>,
    purpose: CatalogPurpose,
) -> Option<&'a PurposeBindingV3> {
    if let Some(game) = game_id {
        let scope = format!("game:{game}");
        if let Some(binding) = graph.bindings.iter().find(|binding| {
            binding.scope_key == scope && binding.purpose == purpose && binding.binding_mode == "override"
        }) {
            return Some(binding);
        }
    }
    graph.bindings.iter().find(|binding| {
        binding.scope_key == "global" && binding.purpose == purpose && binding.binding_mode == "override"
    })
}

/// Non-secret DTO declarations, generated from the same Rust grammar used by storage.
pub(crate) fn typescript_declarations() -> String {
    [
        HttpConfigRef::decl(&ts_rs::Config::default()),
        OfflineConfigRef::decl(&ts_rs::Config::default()),
        SavedServiceRef::decl(&ts_rs::Config::default()),
        EngineTarget::decl(&ts_rs::Config::default()),
        AzureResourceType::decl(&ts_rs::Config::default()),
        DeeplPlan::decl(&ts_rs::Config::default()),
        HttpServiceOptionsV1::decl(&ts_rs::Config::default()),
        ConnectionOrigin::decl(&ts_rs::Config::default()),
        GoogleAuthorizationRef::decl(&ts_rs::Config::default()),
        ConnectionSourceRef::decl(&ts_rs::Config::default()),
        ConnectionV3::decl(&ts_rs::Config::default()),
        PolicyOverride::<String>::decl(&ts_rs::Config::default()),
        HistoryInputMode::decl(&ts_rs::Config::default()),
        ServiceContextConsent::decl(&ts_rs::Config::default()),
        ServiceExecutionDefaults::decl(&ts_rs::Config::default()),
        ServiceCachePolicy::decl(&ts_rs::Config::default()),
        SourceLanguageChoice::decl(&ts_rs::Config::default()),
        TargetLanguageChoice::decl(&ts_rs::Config::default()),
        ServiceLanguageDefaults::decl(&ts_rs::Config::default()),
        ServiceRequestDefaults::decl(&ts_rs::Config::default()),
        ExecutionPolicyOverrides::decl(&ts_rs::Config::default()),
        PolicyOverrides::decl(&ts_rs::Config::default()),
        PolicySource::decl(&ts_rs::Config::default()),
        EffectiveServicePolicy::decl(&ts_rs::Config::default()),
        GenerationParameter::decl(&ts_rs::Config::default()),
        ModelV3::decl(&ts_rs::Config::default()),
        EngineV3::decl(&ts_rs::Config::default()),
        PurposeSemanticsVersion::decl(&ts_rs::Config::default()),
        PurposeBindingV3::decl(&ts_rs::Config::default()),
        TemplateV3::decl(&ts_rs::Config::default()),
        TermV3::decl(&ts_rs::Config::default()),
        TermSetV3::decl(&ts_rs::Config::default()),
        OfflineEngineKey::decl(&ts_rs::Config::default()),
        OfflineBackend::decl(&ts_rs::Config::default()),
        OfflineRoutePolicy::decl(&ts_rs::Config::default()),
        LanguagePair::decl(&ts_rs::Config::default()),
        LocalInstallationRef::decl(&ts_rs::Config::default()),
        OfflineConfigResolution::decl(&ts_rs::Config::default()),
        OfflineEngineConfigV1::decl(&ts_rs::Config::default()),
        ConfigurationV3::decl(&ts_rs::Config::default()),
    ]
    .map(|value| format!("export {value}\n"))
    .join("\n")
}
