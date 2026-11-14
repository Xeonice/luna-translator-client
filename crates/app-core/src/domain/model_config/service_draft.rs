//! Compiles a form into a new immutable draft. This module cannot activate a service,
//! write credentials, prepare an installation, or prove a claimed source revision.
use super::{catalog::parse_purpose, service_codec::validate_service_graph, service_graph::*};
use crate::{
    contracts::*,
    domain::require_id,
    ports::{
        IdentitySource,
        service_configuration::{CompiledServiceDraft, ServiceDirectory},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
};

/// Every registered field has an explicit persistence owner or a protected purpose.
/// New directory fields fail closed until their owner is implemented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceFieldOwner {
    Connection,
    HttpOptions,
    Model,
    Template,
    RequestDefaults,
    Offline,
    PreparedResource,
    Protected,
}
pub fn service_field_owner(id: &str) -> Option<ServiceFieldOwner> {
    use ServiceFieldOwner::*;
    Some(match id {
        "name" | "connection_mode" | "endpoint" | "auth_mode" | "provider_site" | "proxy_mode" | "proxy_url" => Connection,
        "organization_id" | "openai_project_id" | "workspace_id" | "resource_type" | "region"
        | "plan" | "project_id" | "location" | "google_auth_method" => HttpOptions,
        "model" | "temperature_mode" | "temperature" | "max_tokens" => Model,
        "template" => Template,
        "source_language" | "target_language" | "context_count" | "timeout" | "concurrency"
        | "retries" | "fallback" | "cache" => RequestDefaults,
        "route_policy" => Offline,
        "key_pool" | "app_id" | "app_key" | "oauth_client_id" | "oauth_client_mode"
        | "installation_path" | "language_package" | "runtime" | "bottle" => PreparedResource,
        "purpose"
        | "api_key"
        | "app_secret"
        | "service_account_json"
        | "availability"
        | "protocol_details"
        | "oauth_login"
        | "application_oauth_status"
        | "system_status"
        | "language_status"
        | "prepare_language"
        | "check_local_conditions"
        | "sample_test"
        | "resource_status"
        | "runtime_status"
        | "package_detected"
        | "engine_architecture"
        | "resource_version" => Protected,
        _ => return None,
    })
}

/// Internal, non-serializable receipts from preparation ports. These carry no secret.
/// Repository writers must independently verify the referenced immutable resources.
#[derive(Debug, Clone, Default)]
pub struct PreparedServiceDraftResources {
    pub credential: Option<PreparedCredentialDraft>,
    pub google_client: Option<PreparedGoogleClientDraft>,
    pub offline: Option<PreparedOfflineDraft>,
}
#[derive(Debug, Clone)]
pub struct PreparedCredentialDraft {
    pub pool_id: String,
    pub pool_revision: NonZeroU32,
    pub multiple_keys: bool,
    /// Only public typed-material identifiers, never AppSecret/APIKey.
    pub public_identifiers: BTreeMap<String, String>,
}
#[derive(Debug, Clone)]
pub struct PreparedGoogleClientDraft {
    pub mode: String,
    pub client_id: Option<String>,
    pub authorization: GoogleAuthorizationRef,
}
#[derive(Debug, Clone)]
pub struct PreparedOfflineDraft {
    pub engine: OfflineEngineKey,
    pub pair: LanguagePair,
    /// Exact form values whose paths/runtime were persisted by the installation port.
    pub prepared_values: BTreeMap<String, CatalogScalar>,
    pub installation: LocalInstallationRef,
    pub resolution: OfflineConfigResolution,
    pub pivot_consent_revision: Option<NonZeroU32>,
}

fn error(code: &str, message: impl Into<String>) -> AppError {
    AppError::new(code, "service_draft", message)
}
fn field_error(code: &str, id: &str) -> AppError {
    error(
        code,
        format!("字段 {id} 无法保存；请核对字段或完成对应资源准备"),
    )
}
/// Stable operation identity excludes freshly generated graph IDs. All resource
/// references are non-secret immutable preparation facts, never credential values.
pub fn service_draft_request_hash(
    input: &SaveServiceDraftInput,
    prepared: &PreparedServiceDraftResources,
) -> AppResult<String> {
    crate::domain::canonical_hash(&(
        "service-draft-input-v1",
        input.expected_profile_generation,
        input.expected_draft_head_revision,
        &input.editing,
        &input.target,
        input.purpose,
        &input.values,
        &input.authentication,
        prepared.credential.as_ref().map(|credential| {
            (
                &credential.pool_id,
                credential.pool_revision,
                credential.multiple_keys,
                &credential.public_identifiers,
            )
        }),
        prepared
            .google_client
            .as_ref()
            .map(|client| (&client.mode, &client.client_id, &client.authorization)),
        prepared.offline.as_ref().map(|offline| {
            (
                &offline.engine,
                &offline.pair,
                &offline.prepared_values,
                &offline.installation,
                &offline.resolution,
                offline.pivot_consent_revision,
            )
        }),
    ))
}
fn validate_field_applicability(values: &Values<'_>, protocol: Option<&str>) -> AppResult<()> {
    for row in &values.0.fields {
        if row.resolved.value.is_none()
            || service_field_owner(&row.descriptor.id) == Some(ServiceFieldOwner::Protected)
        {
            continue;
        }
        let id = row.descriptor.id.as_str();
        let supported = match id {
            "name" | "source_language" | "target_language" | "context_count" | "timeout"
            | "concurrency" | "retries" | "fallback" | "cache" => true,
            "route_policy" | "installation_path" | "language_package" | "runtime" | "bottle" => {
                protocol.is_none()
            }
            "organization_id" | "openai_project_id" => {
                matches!(protocol, Some("openai_compatible" | "openai_responses"))
            }
            "workspace_id" => protocol == Some("anthropic"),
            "resource_type" | "region" => protocol == Some("azure_translator"),
            "plan" => protocol == Some("deepl_v2"),
            "project_id" | "location" | "google_auth_method" | "oauth_client_id"
            | "oauth_client_mode" => protocol == Some("google_translate_v3"),
            "app_id" => protocol == Some("baidu_vip"),
            "app_key" => protocol == Some("youdao_v3"),
            // Google Translation Advanced has an optional real model resource.
            // Applicability of model input comes from the exact directory descriptor.
            "model" => protocol.is_some(),
            "template" | "temperature_mode" | "temperature" | "max_tokens" => {
                protocol.is_some_and(|p| !super::is_translation_protocol(p))
            }
            "connection_mode" | "endpoint" | "auth_mode" | "provider_site" | "key_pool" | "proxy_mode" | "proxy_url" => {
                protocol.is_some()
            }
            _ => false,
        };
        if !supported {
            return Err(field_error("inapplicable_service_field", id));
        }
    }
    Ok(())
}
struct Values<'a>(&'a ServiceFormProjection);
impl Values<'_> {
    fn value(&self, id: &str) -> Option<&CatalogScalar> {
        self.0
            .fields
            .iter()
            .find(|f| f.descriptor.id == id)
            .and_then(|f| f.resolved.value.as_ref())
    }
    fn text(&self, id: &str) -> Option<String> {
        match self.value(id) {
            Some(CatalogScalar::Text(s)) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        }
    }
    fn number(&self, id: &str) -> Option<f64> {
        match self.value(id) {
            Some(CatalogScalar::Number(n)) => Some(*n),
            _ => None,
        }
    }
    fn boolean(&self, id: &str) -> Option<bool> {
        match self.value(id) {
            Some(CatalogScalar::Boolean(b)) => Some(*b),
            _ => None,
        }
    }
    fn input(&self, id: &str) -> bool {
        self.0
            .fields
            .iter()
            .any(|f| f.descriptor.id == id && f.input_value.is_some())
    }
    fn can_edit(&self, id: &str) -> bool {
        self.0.fields.iter().any(|f| {
            f.descriptor.id == id && f.input_allowed && f.visibility == CapabilityState::Supported
        })
    }
    fn input_text(&self, id: &str) -> Option<&str> {
        self.0
            .fields
            .iter()
            .find(|f| f.descriptor.id == id)
            .and_then(|f| f.input_value.as_ref())
            .and_then(|v| match v {
                CatalogScalar::Text(text) => Some(text.as_str()),
                _ => None,
            })
    }
}

/// The input is a full desired form, not a partial JSON patch. Missing optional fields
/// retain their directory defaults/inheritance. Saving never changes purpose bindings.
pub fn compile_service_draft(
    base: &ConfigurationDocument,
    input: &SaveServiceDraftInput,
    directory: &dyn ServiceDirectory,
    ids: &dyn IdentitySource,
    prepared: &PreparedServiceDraftResources,
) -> AppResult<CompiledServiceDraft> {
    require_id(&input.operation_id)?;
    let projection = directory.form(&ServiceFormRequest {
        target: input.target.clone(),
        purpose: input.purpose,
        values: input.values.clone(),
    })?;
    if projection.target != input.target || projection.purpose != input.purpose {
        return Err(error(
            "directory_projection_mismatch",
            "目录返回了不同的服务或用途",
        ));
    }
    for row in &projection.fields {
        let owner = service_field_owner(&row.descriptor.id)
            .ok_or_else(|| field_error("unmapped_service_field", &row.descriptor.id))?;
        if row.input_value.is_some() && owner == ServiceFieldOwner::Protected {
            return Err(field_error("protected_form_field", &row.descriptor.id));
        }
        if row.issues.iter().any(|i| {
            matches!(
                i,
                ServiceFormIssue::TypeMismatch
                    | ServiceFormIssue::InvalidChoice
                    | ServiceFormIssue::InvalidNumber
                    | ServiceFormIssue::InvalidUrl
                    | ServiceFormIssue::DependencyCycle
            )
        }) {
            return Err(field_error("invalid_service_field", &row.descriptor.id));
        }
        // A caller-supplied inactive/unresolved value cannot be silently discarded.
        if row.input_value.is_some() && row.visibility != CapabilityState::Supported {
            return Err(field_error("inactive_service_field", &row.descriptor.id));
        }
    }
    let catalog = directory.catalog();
    let mut graph = materialize_base(base)?;
    let old_revision = graph.revision_id.clone();
    if let Some(editing) = &input.editing {
        let revision = match editing {
            SavedServiceRef::Http { source } => &source.revision_id,
            SavedServiceRef::Offline { source } => &source.revision_id,
        };
        if revision != &old_revision {
            return Err(error(
                "stale_service_edit",
                "编辑源版本已过期；请重新打开当前服务配置",
            ));
        }
    }
    let values = Values(&projection);
    let resource = match &input.target {
        ServiceFormTarget::Http { preset } => {
            let descriptor = catalog
                .presets
                .iter()
                .find(|p| &p.key == preset)
                .ok_or_else(|| error("unknown_preset_version", "精确服务预设不存在"))?;
            if !descriptor.allowed_purposes.contains(&input.purpose) {
                return Err(error("unsupported_service_purpose", "服务不支持此用途"));
            }
            compile_http(&mut graph, input, descriptor, &values, ids, prepared)?
        }
        ServiceFormTarget::Offline {
            engine_id,
            contract_revision,
        } => {
            let descriptor = catalog
                .offline_engines
                .iter()
                .find(|e| &e.engine_id == engine_id && e.contract_revision == *contract_revision)
                .ok_or_else(|| error("unknown_offline_version", "精确离线引擎版本不存在"))?;
            if input.purpose != CatalogPurpose::Translation
                || !descriptor.allowed_purposes.contains(&input.purpose)
            {
                return Err(error(
                    "unsupported_service_purpose",
                    "离线引擎仅支持已声明的翻译用途",
                ));
            }
            compile_offline(&mut graph, input, descriptor, &values, ids, prepared)?
        }
    };
    graph.revision_id = fresh_id(ids, "config", &graph)?;
    validate_service_graph(&graph)?;
    Ok(CompiledServiceDraft {
        operation_id: input.operation_id.clone(),
        request_hash: service_draft_request_hash(input, prepared)?,
        expected_profile_generation: input.expected_profile_generation,
        expected_draft_head_revision: input.expected_draft_head_revision,
        graph,
        resource,
    })
}

fn fresh_id(ids: &dyn IdentitySource, prefix: &str, graph: &ConfigurationV3) -> AppResult<String> {
    let id = ids.new_id(prefix);
    require_id(&id)?;
    if graph.revision_id == id
        || graph.connections.iter().any(|x| x.connection_id == id)
        || graph.models.iter().any(|x| x.model_id == id)
        || graph.templates.iter().any(|x| x.template_id == id)
        || graph.offline_engine_configs.iter().any(|x| x.id == id)
    {
        return Err(error(
            "generated_identity_collision",
            "身份源返回了已有身份",
        ));
    }
    Ok(id)
}
fn fallback(values: &Values<'_>) -> AppResult<Vec<SavedServiceRef>> {
    let Some(text) = values.text("fallback") else {
        return Ok(vec![]);
    };
    if matches!(text.as_str(), "off" | "none") {
        return Ok(vec![]);
    }
    super::service_codec::validate_unique_service_json(text.as_bytes())
        .map_err(|_| field_error("invalid_fallback_sources", "fallback"))?;
    let sources = serde_json::from_str::<Vec<SavedServiceRef>>(&text)
        .map_err(|_| field_error("invalid_fallback_sources", "fallback"))?;
    let mut seen = BTreeSet::new();
    for source in &sources {
        let (revision, resource) = match source {
            SavedServiceRef::Http { source } => (&source.revision_id, &source.model_id),
            SavedServiceRef::Offline { source } => (&source.revision_id, &source.config_id),
        };
        require_id(revision)
            .and_then(|_| require_id(resource))
            .map_err(|_| field_error("invalid_fallback_sources", "fallback"))?;
        if !seen.insert(
            serde_json::to_string(source)
                .map_err(|_| field_error("invalid_fallback_sources", "fallback"))?,
        ) {
            return Err(field_error("duplicate_fallback_sources", "fallback"));
        }
    }
    Ok(sources)
}
fn identifier(values: &Values<'_>, id: &str) -> AppResult<Option<String>> {
    let value = values.text(id);
    if let Some(value) = &value {
        require_id(value).map_err(|_| field_error("invalid_service_field", id))?;
    }
    Ok(value)
}
fn defaults(
    values: &Values<'_>,
    purpose: CatalogPurpose,
    offline: bool,
) -> AppResult<ServiceRequestDefaults> {
    // Application defaults, not the first option of any descriptor. Timeout is seconds in the form.
    let context = values.number("context_count").unwrap_or(0.0) as u8;
    Ok(ServiceRequestDefaults {
        purpose,
        languages: if offline {
            ServiceLanguageDefaults::OfflineUseConfigPair {}
        } else {
            ServiceLanguageDefaults::Http {
                source: identifier(values, "source_language")?
                    .map_or(SourceLanguageChoice::InheritSource {}, |language| {
                        SourceLanguageChoice::Explicit { language }
                    }),
                target: identifier(values, "target_language")?
                    .map_or(TargetLanguageChoice::InheritPreference {}, |language| {
                        TargetLanguageChoice::Explicit { language }
                    }),
            }
        },
        context: ServiceContextConsent {
            enabled: context > 0,
            max_sentences: context,
            mode: HistoryInputMode::Original,
        },
        execution: ServiceExecutionDefaults {
            timeout_ms: values
                .number("timeout")
                .map_or(60000, |n| (n * 1000.0).round() as u32),
            concurrency: values.number("concurrency").unwrap_or(1.0) as u8,
            retries: values.number("retries").unwrap_or(0.0) as u8,
        },
        cache: ServiceCachePolicy {
            enabled: values.boolean("cache").unwrap_or(true),
        },
        fallback: fallback(values)?,
    })
}
fn retain_unexposed_defaults(
    current: &mut ServiceRequestDefaults,
    old: Option<&ServiceRequestDefaults>,
    values: &Values<'_>,
) {
    let Some(old) = old else {
        return;
    };
    // The form exposes a count, not the stored history mode or a separate consent
    // switch. An unchanged count cannot silently turn an existing opt-out on.
    current.context.mode = old.context.mode.clone();
    if !values.can_edit("context_count") {
        current.context = old.context.clone();
    } else if current.context.max_sentences == old.context.max_sentences {
        current.context.enabled = old.context.enabled;
    }
    if let (
        ServiceLanguageDefaults::Http { source, target },
        ServiceLanguageDefaults::Http {
            source: old_source,
            target: old_target,
        },
    ) = (&mut current.languages, &old.languages)
    {
        if !values.can_edit("source_language") {
            *source = old_source.clone();
        }
        if !values.can_edit("target_language") {
            *target = old_target.clone();
        }
    }
    if !values.can_edit("timeout") {
        current.execution.timeout_ms = old.execution.timeout_ms;
    }
    if !values.can_edit("concurrency") {
        current.execution.concurrency = old.execution.concurrency;
    }
    if !values.can_edit("retries") {
        current.execution.retries = old.execution.retries;
    }
    if !values.can_edit("cache") {
        current.cache = old.cache.clone();
    }
    if !values.can_edit("fallback") {
        current.fallback = old.fallback.clone();
    }
}
fn retain_unexposed_options(
    current: &mut HttpServiceOptionsV1,
    old: Option<&HttpServiceOptionsV1>,
    values: &Values<'_>,
) {
    let Some(old) = old else {
        return;
    };
    use HttpServiceOptionsV1::*;
    match (current, old) {
        (
            OpenaiCompatible {
                organization_id,
                project_id,
            },
            OpenaiCompatible {
                organization_id: old_org,
                project_id: old_project,
            },
        )
        | (
            OpenaiResponses {
                organization_id,
                project_id,
            },
            OpenaiResponses {
                organization_id: old_org,
                project_id: old_project,
            },
        ) => {
            if !values.can_edit("organization_id") {
                *organization_id = old_org.clone();
            }
            if !values.can_edit("openai_project_id") {
                *project_id = old_project.clone();
            }
        }
        (Anthropic { workspace_id }, Anthropic { workspace_id: old }) => {
            if !values.can_edit("workspace_id") {
                *workspace_id = old.clone();
            }
        }
        (
            AzureTranslator {
                resource_type,
                region,
            },
            AzureTranslator {
                resource_type: old_resource,
                region: old_region,
            },
        ) => {
            if !values.can_edit("resource_type") {
                *resource_type = old_resource.clone();
            }
            if !values.can_edit("region") {
                *region = old_region.clone();
            }
        }
        (DeeplV2 { plan }, DeeplV2 { plan: old }) => {
            if !values.can_edit("plan") {
                *plan = old.clone();
            }
        }
        (
            GoogleTranslateV3 {
                project_id,
                location,
            },
            GoogleTranslateV3 {
                project_id: old_project,
                location: old_location,
            },
        ) => {
            if !values.can_edit("project_id") {
                *project_id = old_project.clone();
            }
            if !values.can_edit("location") {
                *location = old_location.clone();
            }
        }
        _ => {}
    }
}
fn http_options(protocol: &str, values: &Values<'_>) -> AppResult<HttpServiceOptionsV1> {
    for id in [
        "organization_id",
        "openai_project_id",
        "workspace_id",
        "region",
        "project_id",
        "location",
    ] {
        if let Some(value) = values.text(id)
            && (value.len() > 512
                || value.chars().any(char::is_control)
                || matches!(id, "project_id" | "location") && value.contains(['/', '\\', '?', '#']))
            {
                return Err(field_error("invalid_service_field", id));
            }
    }
    use HttpServiceOptionsV1::*;
    Ok(match protocol {
        "openai_compatible" => OpenaiCompatible {
            organization_id: values.text("organization_id"),
            project_id: values.text("openai_project_id"),
        },
        "openai_responses" => OpenaiResponses {
            organization_id: values.text("organization_id"),
            project_id: values.text("openai_project_id"),
        },
        "anthropic" => Anthropic {
            workspace_id: values.text("workspace_id"),
        },
        "azure_translator" => AzureTranslator {
            resource_type: values
                .text("resource_type")
                .map(|s| match s.as_str() {
                    "global" => Ok(AzureResourceType::Global),
                    "regional" => Ok(AzureResourceType::Regional),
                    "multi_service" => Ok(AzureResourceType::MultiService),
                    _ => Err(field_error("invalid_service_field", "resource_type")),
                })
                .transpose()?,
            region: values.text("region"),
        },
        "gemini_generate_content" => GeminiGenerateContent {},
        "ollama_native" => OllamaNative {},
        "lmstudio_native_v1" => LmstudioNativeV1 {},
        "deepl_v2" => DeeplV2 {
            plan: values
                .text("plan")
                .map(|s| match s.as_str() {
                    "free" => Ok(DeeplPlan::Free),
                    "pro" => Ok(DeeplPlan::Pro),
                    _ => Err(field_error("invalid_service_field", "plan")),
                })
                .transpose()?,
        },
        "google_translate_v2" => GoogleTranslateV2 {},
        "google_translate_v3" => GoogleTranslateV3 {
            project_id: values.text("project_id").unwrap_or_default(),
            location: values.text("location").unwrap_or_default(),
        },
        "baidu_vip" => BaiduVip {},
        "youdao_v3" => YoudaoV3 {},
        "tencent_tokenhub_translation" => TencentTokenhubTranslation {},
        _ => {
            return Err(error(
                "unsupported_service_protocol",
                "此协议尚无草稿字段编译器",
            ));
        }
    })
}
fn deployment(endpoint: Option<&str>, protocol: &str) -> String {
    let authority = endpoint
        .and_then(|s| {
            s.split_once("://")
                .map(|(_, x)| x.split('/').next().unwrap_or(""))
        })
        .unwrap_or("");
    let host = authority.split(':').next().unwrap_or("");
    if matches!(host, "localhost" | "127.0.0.1") || authority.starts_with("[::1]") {
        "local"
    } else if host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| {
        ip.is_private()
            || ip.is_loopback()
            || ip.is_link_local()
            || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
    }) || matches!(protocol, "ollama_native" | "lmstudio_native_v1")
    {
        "remote"
    } else {
        "cloud"
    }
    .into()
}
type PreparedAuthenticationParts = (
    String,
    Option<String>,
    Option<NonZeroU32>,
    Option<GoogleAuthorizationRef>,
);
fn authentication(
    input: &SaveServiceDraftInput,
    descriptor: &ProviderPresetDescriptor,
    values: &Values<'_>,
    prepared: &PreparedServiceDraftResources,
) -> AppResult<PreparedAuthenticationParts> {
    let mode = super::catalog::service_form_auth_mode(
        &descriptor.binding.protocol.protocol_id,
        &descriptor.default_auth_kind,
        values.text("auth_mode").as_deref(),
        values.text("google_auth_method").as_deref(),
    )?;
    let (pool, revision, grant) = match &input.authentication {
        ServiceAuthenticationSelection::None {} => (None, None, None),
        ServiceAuthenticationSelection::ExistingPool {
            pool_id,
            pool_revision,
        } if !matches!(mode, "none" | "google_oauth") => {
            require_id(pool_id)?;
            (Some(pool_id.clone()), Some(*pool_revision), None)
        }
        ServiceAuthenticationSelection::GoogleOwnedGrant { authorization }
            if mode == "google_oauth" =>
        {
            require_id(&authorization.grant_id)?;
            (None, None, Some(authorization.clone()))
        }
        _ => {
            return Err(error(
                "authentication_selection_mismatch",
                "所选凭据来源与服务验证方式不一致",
            ));
        }
    };
    for id in ["key_pool", "app_id", "app_key"] {
        if values.value(id).is_some() {
            let proof = prepared
                .credential
                .as_ref()
                .ok_or_else(|| field_error("credential_preparation_required", id))?;
            if pool.as_deref() != Some(&proof.pool_id) || revision != Some(proof.pool_revision) {
                return Err(field_error("credential_preparation_mismatch", id));
            }
            if id == "key_pool" {
                if values.boolean(id) != Some(proof.multiple_keys) {
                    return Err(field_error("credential_preparation_mismatch", id));
                }
            } else if values.text(id).as_ref() != proof.public_identifiers.get(id) {
                return Err(field_error("credential_preparation_mismatch", id));
            }
        }
    }
    if mode == "google_oauth" {
        let client_mode = values.text("oauth_client_mode");
        let client_id = values.text("oauth_client_id");
        if client_mode.as_deref() == Some("custom") || client_id.is_some() {
            let proof = prepared.google_client.as_ref().ok_or_else(|| {
                field_error("google_client_preparation_required", "oauth_client_id")
            })?;
            if client_mode.as_ref() != Some(&proof.mode)
                || client_id != proof.client_id
                || grant.as_ref() != Some(&proof.authorization)
            {
                return Err(field_error(
                    "google_client_preparation_mismatch",
                    "oauth_client_id",
                ));
            }
        }
    }
    Ok((mode.into(), pool, revision, grant))
}
fn compile_http(
    graph: &mut ConfigurationV3,
    input: &SaveServiceDraftInput,
    descriptor: &ProviderPresetDescriptor,
    values: &Values<'_>,
    ids: &dyn IdentitySource,
    prepared: &PreparedServiceDraftResources,
) -> AppResult<EngineTarget> {
    validate_field_applicability(values, Some(&descriptor.binding.protocol.protocol_id))?;
    identifier(values, "name")?;
    identifier(values, "model")?;
    let (index, connection_id, model_id) = match &input.editing {
        None => (
            None,
            fresh_id(ids, "connection", graph)?,
            fresh_id(ids, "model", graph)?,
        ),
        Some(SavedServiceRef::Http { source }) => {
            let index = graph
                .models
                .iter()
                .position(|m| m.model_id == source.model_id)
                .ok_or_else(|| error("unknown_service_edit", "编辑服务不存在"))?;
            let old = &graph.models[index];
            let old_connection = graph
                .connections
                .iter()
                .find(|c| c.connection_id == old.connection_id)
                .ok_or_else(|| error("missing_service_connection", "服务连接不存在"))?;
            if old_connection.preset_source.as_ref() != Some(&descriptor.key)
                || old.template_purpose.is_some_and(|p| p != input.purpose)
            {
                return Err(error(
                    "service_edit_target_mismatch",
                    "编辑源与服务预设或用途不一致；请创建新服务",
                ));
            }
            if old.template_purpose.is_none() {
                return Err(error(
                    "legacy_service_edit_requires_copy",
                    "旧服务按原语义保留；请复制为新的明确用途服务后编辑",
                ));
            }
            (Some(index), old.connection_id.clone(), old.model_id.clone())
        }
        _ => {
            return Err(error(
                "service_edit_kind_mismatch",
                "不能用HTTP表单覆盖离线配置",
            ));
        }
    };
    let (auth_mode, credential_pool_id, credential_pool_revision, google_authorization) =
        authentication(input, descriptor, values, prepared)?;
    let previous_connection = graph
        .connections
        .iter()
        .find(|c| c.connection_id == connection_id)
        .cloned();
    let previous_model = index.map(|i| graph.models[i].clone());
    let previous_site = previous_connection
        .as_ref()
        .and_then(|c| c.origin.as_ref())
        .and_then(|o| match o {
            ConnectionOrigin::Custom { site, .. } | ConnectionOrigin::Preset { site, .. } => {
                site.clone()
            }
        });
    let previous_custom = previous_connection
        .as_ref()
        .is_some_and(|c| matches!(c.origin, Some(ConnectionOrigin::Custom { .. })));
    let custom = if values.can_edit("connection_mode") {
        values.text("connection_mode").as_deref() == Some("custom")
    } else {
        previous_custom
    };
    let site = if values.can_edit("provider_site") {
        values.text("provider_site")
    } else {
        previous_site.clone()
    };
    let mut endpoint = values.text("endpoint");
    if !values.can_edit("endpoint") && custom == previous_custom && site == previous_site
        && let Some(old) = &previous_connection {
            endpoint = old.endpoint.clone();
        }
    let mut options = http_options(&descriptor.binding.protocol.protocol_id, values)?;
    retain_unexposed_options(
        &mut options,
        previous_connection
            .as_ref()
            .and_then(|c| c.http_options.as_ref()),
        values,
    );
    let mut network_metadata = previous_connection.as_ref().map_or_else(BTreeMap::new, |c| c.network_metadata.clone());
    let proxy_mode = if values.can_edit("proxy_mode") {
        let mode = values.text("proxy_mode").ok_or_else(|| field_error("invalid_proxy_mode", "proxy_mode"))?;
        network_metadata.remove("proxyUrl");
        if mode == "override"
            && let Some(url) = values.text("proxy_url") {
                network_metadata.insert("proxyUrl".into(), serde_json::Value::String(url));
            }
        crate::ports::google_authorization::GoogleNetworkPolicy::from_connection(&mode, &network_metadata)?;
        mode
    } else {
        previous_connection.as_ref().map_or_else(|| "inherit".into(), |c| c.proxy_mode.clone())
    };
    let connection = ConnectionV3 {
        connection_id: connection_id.clone(),
        display_name: previous_connection.as_ref().map_or_else(
            || {
                values
                    .text("name")
                    .unwrap_or_else(|| descriptor.label.clone())
            },
            |c| c.display_name.clone(),
        ),
        protocol: descriptor.binding.protocol.protocol_id.clone(),
        deployment: previous_connection
            .as_ref()
            .filter(|c| c.endpoint == endpoint)
            .map_or_else(
                || {
                    deployment(
                        endpoint.as_deref(),
                        &descriptor.binding.protocol.protocol_id,
                    )
                },
                |c| c.deployment.clone(),
            ),
        auth_mode,
        endpoint,
        credential_pool_id,
        credential_pool_revision,
        proxy_mode,
        network_metadata,
        availability_intent: previous_connection
            .as_ref()
            .is_none_or(|c| c.availability_intent),
        adapter_binding: Some(descriptor.binding.clone()),
        preset_source: Some(descriptor.key.clone()),
        http_options: Some(options),
        origin: Some(if custom {
            ConnectionOrigin::Custom {
                copied_from: Some(descriptor.key.clone()),
                site: site.clone(),
            }
        } else {
            ConnectionOrigin::Preset {
                key: descriptor.key.clone(),
                site,
            }
        }),
        google_authorization,
        materialization_source: previous_connection
            .as_ref()
            .and_then(|c| c.materialization_source.clone()),
    };
    if let Some(existing) = graph
        .connections
        .iter_mut()
        .find(|c| c.connection_id == connection_id)
    {
        if *existing != connection
            && graph
                .models
                .iter()
                .filter(|m| m.connection_id == connection_id)
                .count()
                > 1
        {
            return Err(error(
                "shared_connection_review_required",
                "连接被多个服务共享；更改连接前须显式核对共享影响",
            ));
        }
        *existing = connection;
    } else {
        graph.connections.push(connection);
    }
    let mut parameters = index.map_or_else(BTreeMap::new, |i| graph.models[i].parameters.clone());
    if values.can_edit("temperature_mode") {
        parameters.remove("temperature");
        match values.text("temperature_mode").as_deref() {
            Some("set") => {
                let value = values
                    .number("temperature")
                    .ok_or_else(|| field_error("generation_value_required", "temperature"))?;
                let parameter=previous_model.as_ref().and_then(|m|m.parameters.get("temperature")).filter(|p|matches!(p,GenerationParameter::Set{value:old} if old.as_f64()==Some(value))).cloned().unwrap_or(GenerationParameter::Set{value:serde_json::json!(value)});
                parameters.insert("temperature".into(), parameter);
            }
            Some("omit") => {
                parameters.insert("temperature".into(), GenerationParameter::Omit {});
            }
            Some("inherit") => {
                if previous_model
                    .as_ref()
                    .is_none_or(|m| m.parameters.contains_key("temperature"))
                    || values.input("temperature_mode")
                {
                    parameters.insert("temperature".into(), GenerationParameter::Inherit {});
                }
            }
            None => {
                if values.input("temperature") {
                    return Err(field_error("generation_mode_required", "temperature_mode"));
                }
            }
            _ => return Err(field_error("invalid_service_field", "temperature_mode")),
        }
    }
    if values.can_edit("max_tokens") {
        if let Some(n) = values.number("max_tokens") {
            let parameter = previous_model
                .as_ref()
                .and_then(|m| m.parameters.get("max_tokens"))
                .filter(
                    |p| matches!(p,GenerationParameter::Set{value:old} if old.as_f64()==Some(n)),
                )
                .cloned()
                .unwrap_or(GenerationParameter::Set {
                    value: serde_json::json!(n as u32),
                });
            parameters.insert("max_tokens".into(), parameter);
        } else if matches!(
            parameters.get("max_tokens"),
            Some(GenerationParameter::Set { .. })
        ) {
            parameters.remove("max_tokens");
        }
    }
    let mut default_templates =
        index.map_or_else(BTreeMap::new, |i| graph.models[i].default_templates.clone());
    if values.can_edit("template") {
        let old_template = default_templates
            .get(&input.purpose)
            .and_then(|id| graph.templates.iter().find(|t| &t.template_id == id))
            .cloned();
        let body = values.text("template");
        let unchanged = old_template.as_ref().is_some_and(|t| {
            let old = t.body.get("user").and_then(|v| v.as_str());
            old.is_some() && (old == body.as_deref() || old == values.input_text("template"))
        });
        if !unchanged {
            default_templates.remove(&input.purpose);
            if let Some(body) = body {
                let id = fresh_id(ids, "template", graph)?;
                // Template syntax/required variables are execution checks. Preserve unfinished drafts.
                let variables = super::template_variables(&body)
                    .unwrap_or_default()
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                let mut template = old_template.unwrap_or(TemplateV3 {
                    template_id: id.clone(),
                    purpose: input.purpose,
                    display_name: "自定义模板".into(),
                    source_kind: "custom".into(),
                    source_version: None,
                    body: BTreeMap::new(),
                    variables,
                    compatibility: BTreeMap::new(),
                });
                template.template_id = id.clone();
                template.source_kind = "custom".into();
                template.source_version = None;
                template
                    .body
                    .insert("user".into(), serde_json::Value::String(body));
                template.variables = template
                    .body
                    .values()
                    .filter_map(|v| v.as_str())
                    .flat_map(|text| {
                        super::template_variables(text)
                            .unwrap_or_default()
                            .into_iter()
                            .map(str::to_owned)
                    })
                    .collect();
                template.variables.sort();
                template.variables.dedup();
                graph.templates.push(template);
                default_templates.insert(input.purpose, id);
            }
        }
    }
    let mut defaults = defaults(values, input.purpose, false)?;
    retain_unexposed_defaults(
        &mut defaults,
        previous_model
            .as_ref()
            .and_then(|m| m.request_defaults.get(&input.purpose)),
        values,
    );
    if defaults
        .fallback
        .iter()
        .any(|s| matches!(s, SavedServiceRef::Http{source} if source.model_id == model_id))
    {
        return Err(field_error("self_fallback_forbidden", "fallback"));
    }
    let model = ModelV3 {
        default_templates,
        model_id: model_id.clone(),
        connection_id,
        actual_model_id: if values.can_edit("model") {
            values.text("model")
        } else {
            previous_model
                .as_ref()
                .and_then(|m| m.actual_model_id.clone())
        },
        display_name: if values.can_edit("name") {
            values
                .text("name")
                .unwrap_or_else(|| descriptor.label.clone())
        } else {
            previous_model
                .as_ref()
                .map_or_else(|| descriptor.label.clone(), |m| m.display_name.clone())
        },
        selection_source: previous_model
            .as_ref()
            .map_or_else(|| "manual".into(), |m| m.selection_source.clone()),
        capabilities: index.map_or_else(BTreeMap::new, |i| graph.models[i].capabilities.clone()),
        parameters,
        template_purpose: Some(input.purpose),
        legacy_eligibility_origin: previous_model
            .as_ref()
            .and_then(|m| m.legacy_eligibility_origin.clone()),
        request_defaults: BTreeMap::from([(input.purpose, defaults)]),
    };
    if let Some(index) = index {
        graph.models[index] = model;
    } else {
        graph.models.push(model);
    }
    Ok(EngineTarget::Http { model_id })
}
fn compile_offline(
    graph: &mut ConfigurationV3,
    input: &SaveServiceDraftInput,
    descriptor: &OfflineEngineDescriptor,
    values: &Values<'_>,
    ids: &dyn IdentitySource,
    prepared: &PreparedServiceDraftResources,
) -> AppResult<EngineTarget> {
    validate_field_applicability(values, None)?;
    identifier(values, "name")?;
    if input.authentication != (ServiceAuthenticationSelection::None {}) {
        return Err(error(
            "offline_authentication_forbidden",
            "离线引擎不能携带HTTP凭据",
        ));
    }
    let (index, id) = match &input.editing {
        None => (None, fresh_id(ids, "offline", graph)?),
        Some(SavedServiceRef::Offline { source }) => {
            let index = graph
                .offline_engine_configs
                .iter()
                .position(|c| c.id == source.config_id)
                .ok_or_else(|| error("unknown_service_edit", "离线配置不存在"))?;
            let old = &graph.offline_engine_configs[index];
            if old.engine.engine_id != descriptor.engine_id
                || old.engine.contract_revision.get() != descriptor.contract_revision
            {
                return Err(error(
                    "service_edit_target_mismatch",
                    "离线引擎版本与编辑源不一致",
                ));
            }
            (Some(index), old.id.clone())
        }
        _ => {
            return Err(error(
                "service_edit_kind_mismatch",
                "不能用离线表单覆盖HTTP服务",
            ));
        }
    };
    let engine = OfflineEngineKey {
        engine_id: descriptor.engine_id.clone(),
        contract_revision: NonZeroU32::new(descriptor.contract_revision)
            .ok_or_else(|| error("invalid_offline_version", "离线引擎版本必须非零"))?,
    };
    let previous = index.map(|i| graph.offline_engine_configs[i].clone());
    let pair = LanguagePair {
        source: identifier(values, "source_language")?
            .ok_or_else(|| field_error("offline_language_pair_required", "source_language"))?,
        target: identifier(values, "target_language")?
            .ok_or_else(|| field_error("offline_language_pair_required", "target_language"))?,
    };
    let path_fields = ["installation_path", "language_package", "runtime", "bottle"];
    let entered = path_fields
        .iter()
        .filter_map(|id| values.value(id).map(|v| ((*id).to_owned(), v.clone())))
        .collect::<BTreeMap<_, _>>();
    let proof = prepared.offline.as_ref();
    if !entered.is_empty() && proof.is_none() {
        return Err(error(
            "offline_preparation_required",
            "本机路径、软件运行环境或语言资源须先由安装端口持久化准备",
        ));
    }
    if let Some(proof) = proof
        && (proof.engine != engine || proof.pair != pair || entered != proof.prepared_values) {
            return Err(error(
                "offline_preparation_mismatch",
                "安装准备回执与引擎或本次输入不一致",
            ));
        }
    let route_policy = if let Some(previous) = previous.as_ref().filter(|_| !values.can_edit("route_policy")) {
        previous.route_policy.clone()
    } else {
        match values.text("route_policy").as_deref() {
            None | Some("direct_only") => OfflineRoutePolicy::DirectOnly {},
            Some("pivot") => OfflineRoutePolicy::ExplicitPivot {
                consent_revision: proof
                    .and_then(|p| p.pivot_consent_revision)
                    .or_else(|| {
                        previous.as_ref().filter(|p| p.pair == pair).and_then(|p| {
                            match p.route_policy {
                                OfflineRoutePolicy::ExplicitPivot { consent_revision } => {
                                    Some(consent_revision)
                                }
                                _ => None,
                            }
                        })
                    })
                    .ok_or_else(|| field_error("offline_pivot_consent_required", "route_policy"))?,
            },
            _ => return Err(field_error("invalid_service_field", "route_policy")),
        }
    };
    let mut defaults = defaults(values, input.purpose, true)?;
    retain_unexposed_defaults(
        &mut defaults,
        previous.as_ref().map(|p| &p.request_defaults),
        values,
    );
    if defaults
        .fallback
        .iter()
        .any(|s| matches!(s, SavedServiceRef::Offline{source} if source.config_id == id))
    {
        return Err(field_error("self_fallback_forbidden", "fallback"));
    }
    let retain_resolution = previous
        .as_ref()
        .filter(|p| p.pair == pair && p.route_policy == route_policy);
    let config = OfflineEngineConfigV1 {
        id: id.clone(),
        engine,
        backend: match descriptor.backend {
            OfflineExecutionBackend::PlatformNative => OfflineBackend::PlatformNative,
            OfflineExecutionBackend::LocalWorker => OfflineBackend::LocalWorker,
        },
        display_name: values
            .text("name")
            .unwrap_or_else(|| descriptor.label.clone()),
        pair,
        route_policy,
        installation: proof
            .map(|p| p.installation.clone())
            .or_else(|| previous.as_ref().and_then(|p| p.installation.clone())),
        resolution: proof
            .map(|p| p.resolution.clone())
            .or_else(|| retain_resolution.map(|p| p.resolution.clone()))
            .unwrap_or(OfflineConfigResolution::Draft {
                observed_resource: None,
                proposed_route: None,
            }),
        request_defaults: defaults,
    };
    if let Some(index) = index {
        graph.offline_engine_configs[index] = config;
    } else {
        graph.offline_engine_configs.push(config);
    }
    Ok(EngineTarget::Offline {
        offline_config_id: id,
    })
}

fn parameters(
    values: &BTreeMap<String, Parameter>,
) -> AppResult<BTreeMap<String, GenerationParameter>> {
    values
        .iter()
        .map(|(k, v)| {
            super::validate_parameter(v)?;
            Ok((
                k.clone(),
                match &v.mode {
                    ParameterMode::Inherit => GenerationParameter::Inherit {},
                    ParameterMode::Omit => GenerationParameter::Omit {},
                    ParameterMode::Set => GenerationParameter::Set {
                        value: v
                            .value
                            .clone()
                            .ok_or_else(|| error("invalid_parameter", "set参数必须有值"))?,
                    },
                },
            ))
        })
        .collect()
}
/// Preserve frozen legacy values and sequence semantics. Source claims are declarations,
/// not credentials: the private writer must prove them against the original stored row.
pub fn materialize_base(base: &ConfigurationDocument) -> AppResult<ConfigurationV3> {
    match base {
        ConfigurationDocument::ServiceGraph { configuration } => {
            validate_service_graph(configuration)?;
            Ok(configuration.clone())
        }
        ConfigurationDocument::Legacy { configuration: c } => {
            // A fresh profile has no saved revision. Its exact empty legacy
            // snapshot is a draft seed, never a claimed stored source.
            if c != &Configuration::default() {
                super::codec::validate_configuration_codec(c)?;
            }
            let mut graph = ConfigurationV3 {
                revision_id: c.revision_id.clone(),
                connections: vec![],
                models: vec![],
                bindings: vec![],
                templates: vec![],
                term_sets: vec![],
                offline_engine_configs: vec![],
                codec_version: 3,
            };
            for x in &c.connections {
                graph.connections.push(ConnectionV3 {
                    connection_id: x.connection_id.clone(),
                    display_name: x.display_name.clone(),
                    protocol: x.protocol.clone(),
                    deployment: x.deployment.clone(),
                    auth_mode: x.auth_mode.clone(),
                    endpoint: x.endpoint.clone(),
                    credential_pool_id: x.credential_pool_id.clone(),
                    credential_pool_revision: x
                        .credential_pool_revision
                        .map(|v| {
                            NonZeroU32::new(v).ok_or_else(|| {
                                error("invalid_credential_revision", "凭据版本必须非零")
                            })
                        })
                        .transpose()?,
                    proxy_mode: x.proxy_mode.clone(),
                    network_metadata: x.network_metadata.clone(),
                    availability_intent: x.availability_intent,
                    adapter_binding: x.adapter_binding.clone(),
                    preset_source: x.preset_source.clone(),
                    http_options: None,
                    origin: None,
                    google_authorization: None,
                    materialization_source: Some(ConnectionSourceRef {
                        revision_id: c.revision_id.clone(),
                        connection_id: x.connection_id.clone(),
                    }),
                });
            }
            for x in &c.models {
                graph.models.push(ModelV3 {
                    default_templates: x
                        .default_templates
                        .iter()
                        .map(|(p, t)| Ok((parse_purpose(p)?, t.clone())))
                        .collect::<AppResult<_>>()?,
                    model_id: x.model_id.clone(),
                    connection_id: x.connection_id.clone(),
                    actual_model_id: x.actual_model_id.clone(),
                    display_name: x.display_name.clone(),
                    selection_source: x.selection_source.clone(),
                    capabilities: x.capabilities.clone(),
                    parameters: parameters(&x.parameters)?,
                    template_purpose: None,
                    legacy_eligibility_origin: Some(HttpConfigRef {
                        revision_id: c.revision_id.clone(),
                        model_id: x.model_id.clone(),
                    }),
                    request_defaults: BTreeMap::new(),
                });
            }
            for x in &c.bindings {
                graph.bindings.push(PurposeBindingV3 {
                    binding_id: x.binding_id.clone(),
                    scope_key: x.scope_key.clone(),
                    game_id: x.game_id.clone(),
                    purpose: parse_purpose(&x.purpose)?,
                    binding_mode: x.binding_mode.clone(),
                    enabled: x.enabled,
                    output_mode: x.output_mode.clone(),
                    source_language: x.source_language.clone(),
                    target_language: x.target_language.clone(),
                    template_id: x.template_id.clone(),
                    context: x.context.clone(),
                    schedule: x.schedule.clone(),
                    parameters: parameters(&x.parameters)?,
                    engines: x
                        .engines
                        .iter()
                        .map(|e| {
                            Ok(EngineV3 {
                                engine_id: e.engine_id.clone(),
                                target: EngineTarget::Http {
                                    model_id: e.model_id.clone(),
                                },
                                enabled: e.enabled,
                                http_template_override: e.template_id.clone(),
                                http_parameters: parameters(&e.parameters)?,
                                policy_overrides: PolicyOverrides::default(),
                            })
                        })
                        .collect::<AppResult<_>>()?,
                    semantics_version: PurposeSemanticsVersion::LegacyV2,
                    scope_overrides: PolicyOverrides::default(),
                });
            }
            for x in &c.templates {
                graph.templates.push(TemplateV3 {
                    template_id: x.template_id.clone(),
                    purpose: parse_purpose(&x.purpose)?,
                    display_name: x.display_name.clone(),
                    source_kind: x.source_kind.clone(),
                    source_version: x.source_version.clone(),
                    body: x.body.clone(),
                    variables: x.variables.clone(),
                    compatibility: x.compatibility.clone(),
                });
            }
            for x in &c.term_sets {
                graph.term_sets.push(TermSetV3 {
                    term_set_id: x.term_set_id.clone(),
                    scope_key: x.scope_key.clone(),
                    game_id: x.game_id.clone(),
                    display_name: x.display_name.clone(),
                    enabled: x.enabled,
                    terms: x
                        .terms
                        .iter()
                        .map(|t| TermV3 {
                            term_id: t.term_id.clone(),
                            source_text: t.source_text.clone(),
                            target_text: t.target_text.clone(),
                            source_language: t.source_language.clone(),
                            target_language: t.target_language.clone(),
                            enabled: t.enabled,
                        })
                        .collect(),
                });
            }
            if c != &Configuration::default() {
                validate_service_graph(&graph)?;
            }
            Ok(graph)
        }
    }
}
