//! Typed codec3 planning only. Repository authority and secret/resource leases remain external.
mod test_plan;
pub use test_plan::{
    SavedServiceTestPlanInput, TestReadingInput, compose_saved_service_test_plan,
    saved_service_test_evidence_domain, saved_service_test_request_semantics,
    saved_service_test_source_semantics,
};

use super::request_plan::{
    ReferenceMaterialInput, RequestPlan, RequestTextInput, compose_reference_material,
    compose_request_text, purpose_system_prompt, request_system_safety_suffix,
    request_system_structured_suffix,
};
use crate::{
    contracts::*,
    domain::{
        content_hash,
        model_config::{
            catalog, service_codec::validate_service_graph, service_graph::*, templates,
            validate_template, wire_parameters::generation_wire_parameters,
        },
        require_id,
        service_execution::*,
        utf16_span,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub type ComposedServicePlan = ServiceExecutionPlan<RequestPlan>;
/// This is a provenance statement, never permission to read preferences or to
/// dispatch a request. The caller freezes the actual resolved language first.
pub enum TargetLanguageOrigin {
    BuiltinDefault,
    StoredReadingPreference {
        revision_id: String,
        scope_key: String,
    },
}
pub fn compose_service_plan_with_target_origin(
    input: ServicePlanInput<'_>,
    origin: TargetLanguageOrigin,
) -> AppResult<ComposedServicePlan> {
    let source = match origin {
        TargetLanguageOrigin::BuiltinDefault => {
            if input.target_language != "zh-Hans" {
                return Err(error(
                    "target_language_origin_mismatch",
                    "产品默认目标语言与实际值不一致",
                ));
            }
            PlanFieldSource {
                kind: PlanSourceKind::Builtin,
                revision_id: None,
                entity_id: None,
                field: "defaultTargetLanguageV1".into(),
            }
        }
        TargetLanguageOrigin::StoredReadingPreference {
            revision_id,
            scope_key,
        } => {
            require_id(&revision_id)?;
            require_id(&scope_key)?;
            PlanFieldSource {
                kind: PlanSourceKind::ReadingPreference,
                revision_id: Some(revision_id),
                entity_id: Some(scope_key),
                field: "targetLanguage".into(),
            }
        }
    };
    let mut plan = compose_service_plan(input)?;
    if let Some(current) = plan.provenance.get_mut("targetLanguage")
        && current.kind == PlanSourceKind::ReadingPreference
    {
        *current = source;
    }
    Ok(plan)
}
/// Exact selected identities, already resolved reading preferences and the real immutable directory.
/// No boolean in this input claims that a credential or installed resource is authorized.
pub struct ServicePlanInput<'a> {
    pub graph: &'a ConfigurationV3,
    pub expected_revision_id: &'a str,
    pub binding_id: &'a str,
    pub engine_id: &'a str,
    pub unit: &'a ReadingUnit,
    pub selected_token: Option<&'a LexicalToken>,
    pub history: &'a [ReadingUnit],
    pub source_language: &'a str,
    pub target_language: &'a str,
    pub catalog: &'a ServiceCatalog,
}
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "service_request_plan", message)
}
fn purpose_name(purpose: CatalogPurpose) -> &'static str {
    match purpose {
        CatalogPurpose::Translation => "translation",
        CatalogPurpose::Grammar => "grammar",
        CatalogPurpose::ContextWord => "context_word",
    }
}
fn source(
    kind: PlanSourceKind,
    revision: Option<&str>,
    entity: Option<&str>,
    field: &str,
) -> PlanFieldSource {
    PlanFieldSource {
        kind,
        revision_id: revision.map(str::to_owned),
        entity_id: entity.map(str::to_owned),
        field: field.into(),
    }
}
fn entity_source(
    graph: &ConfigurationV3,
    kind: PlanSourceKind,
    id: &str,
    field: &str,
) -> PlanFieldSource {
    source(kind, Some(&graph.revision_id), Some(id), field)
}
fn validate_input(input: &ServicePlanInput<'_>) -> AppResult<()> {
    if input.graph.revision_id != input.expected_revision_id {
        return Err(error(
            "service_revision_mismatch",
            "所选服务版本已变化，请重新核对",
        ));
    }
    validate_service_graph(input.graph)?;
    validate_reading_input(
        input.unit,
        input.selected_token,
        input.source_language,
        input.target_language,
    )
}
fn validate_reading_input(
    unit: &ReadingUnit,
    selected_token: Option<&LexicalToken>,
    source_language: &str,
    target_language: &str,
) -> AppResult<()> {
    require_id(&unit.unit_id)?;
    require_id(&unit.session_id)?;
    require_id(source_language)?;
    require_id(target_language)?;
    if unit.text_revision == 0
        || unit.source_text.trim().is_empty()
        || unit.source_text.contains('\0')
        || !unit.created_at_ms.is_finite()
        || unit.source_hash != content_hash(&unit.source_text)
    {
        return Err(error(
            "invalid_reading_identity",
            "阅读对象版本或原文指纹无效",
        ));
    }
    if let Some(token) = selected_token {
        let mapped = utf16_span(&unit.source_text, token.byte_start, token.byte_end)?;
        if token.byte_start >= token.byte_end
            || unit
                .source_text
                .get(token.byte_start as usize..token.byte_end as usize)
                != Some(token.surface.as_str())
            || mapped != (token.utf16_start, token.utf16_end)
        {
            return Err(error("selected_token_mismatch", "选词不属于当前固定原文"));
        }
    }
    Ok(())
}
fn validate_endpoint(endpoint: Option<&str>) -> AppResult<&str> {
    let endpoint = endpoint
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| error("missing_endpoint", "尚未配置服务地址"))?;
    let authority = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next());
    if endpoint.len() > 4096
        || endpoint
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
        || endpoint.contains(['@', '?', '#'])
        || authority.is_none_or(str::is_empty)
    {
        return Err(error(
            "invalid_endpoint",
            "服务地址需要不含凭据、查询和片段的HTTP(S)地址",
        ));
    }
    Ok(endpoint)
}
fn auth_kind(connection: &ConnectionV3) -> AppResult<CatalogAuthKind> {
    let mode = connection.auth_mode.as_str();
    let kind = match mode {
        "none" => CatalogAuthKind::None,
        "google_oauth" => {
            if connection.google_authorization.is_none() {
                return Err(error(
                    "missing_google_authorization",
                    "尚未选择Google授权版本",
                ));
            }
            CatalogAuthKind::GoogleOauth
        }
        "google_service_account" => CatalogAuthKind::GoogleServiceAccount,
        "baidu_app_secret" => CatalogAuthKind::BaiduAppSecret,
        "youdao_app_secret" => CatalogAuthKind::YoudaoAppSecret,
        "api_key" => match connection.protocol.as_str() {
            "anthropic" => CatalogAuthKind::AnthropicApiKey,
            "gemini_generate_content" | "google_translate_v2" => CatalogAuthKind::GoogleApiKey,
            "azure_translator" => CatalogAuthKind::AzureSubscriptionKey,
            "deepl_v2" => CatalogAuthKind::DeeplAuthKey,
            _ => CatalogAuthKind::BearerApiKey,
        },
        _ => return Err(error("unsupported_auth_kind", "未知认证材料类型")),
    };
    if !matches!(kind, CatalogAuthKind::None | CatalogAuthKind::GoogleOauth)
        && (connection
            .credential_pool_id
            .as_deref()
            .is_none_or(str::is_empty)
            || connection.credential_pool_revision.is_none())
    {
        return Err(error(
            "missing_credential_reference",
            "尚未选择凭据及其精确版本",
        ));
    }
    Ok(kind)
}
fn validate_http_binding<'a>(directory: &'a ServiceCatalog, connection: &ConnectionV3, purpose: CatalogPurpose) -> AppResult<&'a ProtocolDescriptor> {
    let _ = validate_endpoint(connection.endpoint.as_deref())?;
    let binding = connection
        .adapter_binding
        .as_ref()
        .ok_or_else(|| error("missing_adapter_binding", "尚未指定精确接口契约"))?;
    let protocol = directory
        .protocols
        .iter()
        .find(|p| p.key == binding.protocol)
        .ok_or_else(|| error("unknown_protocol_version", "未注册所选精确协议版本"))?;
    let profile = directory
        .profiles
        .iter()
        .find(|p| p.key == binding.profile)
        .ok_or_else(|| error("unknown_profile_version", "未注册所选精确兼容规则版本"))?;
    catalog::validate_protocol(protocol)?;
    catalog::validate_profile(profile, protocol)?;
    if protocol.key.protocol_id != connection.protocol
        || !protocol.allowed_purposes.contains(&purpose)
        || !profile.allowed_purposes.contains(&purpose)
    {
        return Err(error(
            "service_purpose_mismatch",
            "接口不支持该用途或精确协议身份不一致",
        ));
    }
    let kind = auth_kind(connection)?;
    if !protocol.auth_kinds.contains(&kind) || !profile.auth_kinds.contains(&kind) {
        return Err(error(
            "authentication_protocol_mismatch",
            "认证材料与精确协议不匹配",
        ));
    }
    Ok(protocol)
}
fn validate_http_options<'a>(directory: &ServiceCatalog, connection: &'a ConnectionV3, actual: Option<&str>, purpose: CatalogPurpose) -> AppResult<(&'a HttpServiceOptionsV1, &'a ConnectionOrigin)> {
    let endpoint=validate_endpoint(connection.endpoint.as_deref())?;
    let binding=connection.adapter_binding.as_ref().ok_or_else(||error("missing_adapter_binding", "尚未指定精确接口契约"))?;
    let options = connection
        .http_options
        .as_ref()
        .ok_or_else(|| error("missing_http_options", "缺少精确HTTP接口选项"))?;
    let origin = connection
        .origin
        .as_ref()
        .ok_or_else(|| error("missing_service_origin", "缺少服务来源身份"))?;
    let option_text = |value: Option<&str>| -> AppResult<()> {
        if value.is_some_and(|v| v.trim().is_empty() || v != v.trim()) {
            Err(error(
                "invalid_http_options",
                "明确的HTTP选项不能是空白或包含首尾空白",
            ))
        } else {
            Ok(())
        }
    };
    match options {
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id,
            project_id,
        }
        | HttpServiceOptionsV1::OpenaiResponses {
            organization_id,
            project_id,
        } => {
            option_text(organization_id.as_deref())?;
            option_text(project_id.as_deref())?;
        }
        HttpServiceOptionsV1::Anthropic { workspace_id } => {
            option_text(workspace_id.as_deref())?;
        }
        HttpServiceOptionsV1::AzureTranslator {
            resource_type,
            region,
        } => {
            let resource = resource_type
                .as_ref()
                .ok_or_else(|| error("missing_azure_resource_type", "需要明确选择Azure资源类型"))?;
            if !matches!(resource, AzureResourceType::Global)
                && region.as_deref().is_none_or(|v| v.trim().is_empty())
            {
                return Err(error("missing_azure_region", "所选Azure资源类型需要区域"));
            }
            option_text(region.as_deref())?;
        }
        HttpServiceOptionsV1::DeeplV2 { plan: None } => {
            return Err(error("missing_deepl_plan", "需要明确选择DeepL Free或Pro"));
        }
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id,
            location,
        } => {
            if !endpoint.starts_with("https://") {
                return Err(error(
                    "google_endpoint_requires_https",
                    "Google短期授权仅发送到明确保存的HTTPS目标",
                ));
            }
            if project_id.trim().is_empty() || location.trim().is_empty() {
                return Err(error(
                    "missing_google_route",
                    "Google Advanced需要项目和位置",
                ));
            }
            if !google_resource_segment(project_id) || !google_resource_segment(location) {
                return Err(error(
                    "invalid_google_route",
                    "Google项目和位置不是有效的资源路径片段",
                ));
            }
            if let Some(model) = actual {
                validate_google_model_resource(model, project_id, location)?;
            }
        }
        _ => {}
    }
    if let ConnectionOrigin::Preset { key, .. } = origin {
        let preset = directory
            .presets
            .iter()
            .find(|p| p.key == *key)
            .ok_or_else(|| error("unknown_preset_version", "未注册所选精确预设版本"))?;
        if preset.binding != *binding || !preset.allowed_purposes.contains(&purpose) {
            return Err(error(
                "preset_binding_mismatch",
                "预设与精确接口或用途不匹配",
            ));
        }
    }
    // Intentionally do not read implementation/eligibility as resource authorization.
    Ok((options, origin))
}
fn validate_http<'a>(directory: &ServiceCatalog, connection: &'a ConnectionV3, model: &ModelV3, purpose: CatalogPurpose) -> AppResult<(&'a HttpServiceOptionsV1, &'a ConnectionOrigin)> {
    let protocol=validate_http_binding(directory, connection, purpose)?;
    let actual = model.actual_model_id.as_deref();
    if actual.is_some_and(|m| m.trim().is_empty() || m.chars().any(char::is_control)) {
        return Err(error("invalid_model_identity", "模型身份为空或含控制字符"));
    }
    match protocol.model_identity {
        CatalogModelIdentity::Required if actual.is_none() => {
            return Err(error("missing_model", "所选接口要求真实模型ID"));
        }
        CatalogModelIdentity::NotApplicable if actual.is_some() => {
            return Err(error("model_not_applicable", "此接口不接受模型ID"));
        }
        _ => {}
    }
    validate_http_options(directory,connection,actual,purpose)
}
/// Connection-only static preflight. No fictitious model/plan or material access.
pub fn validate_http_connection<'a>(directory: &ServiceCatalog, connection: &'a ConnectionV3, purpose: CatalogPurpose) -> AppResult<(&'a HttpServiceOptionsV1, &'a ConnectionOrigin)> {
    validate_http_binding(directory,connection,purpose)?;
    let result=validate_http_options(directory,connection,None,purpose)?;
    if result.0.protocol_id()!=connection.protocol { return Err(error("invalid_http_options", "接口选项与精确协议不一致")); }
    Ok(result)
}
fn google_resource_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn validate_google_model_resource(model: &str, project: &str, location: &str) -> AppResult<()> {
    // Keep the saved route's exact resource identity; never normalize mismatched paths.
    let parts = model.split('/').collect::<Vec<_>>();
    let tail = match parts.as_slice() {
        ["projects", p, "locations", l, "models", tail @ ..] if *p == project && *l == location => {
            tail
        }
        _ => {
            return Err(error(
                "invalid_google_model",
                "Google模型必须属于明确的项目和位置",
            ));
        }
    };
    let valid = match tail {
        [id] => google_resource_segment(id) && !matches!(*id, "general" | "translation-llm-custom"),
        ["general", "nmt" | "translation-llm"] => true,
        ["translation-llm-custom", id] => google_resource_segment(id),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(error("invalid_google_model", "Google模型资源路径结构无效"))
    }
}
fn generation_parameters(
    graph: &ConfigurationV3,
    binding: &PurposeBindingV3,
    engine: &EngineV3,
    model: &ModelV3,
    provenance: &mut BTreeMap<String, PlanFieldSource>,
) -> AppResult<BTreeMap<String, Value>> {
    let mut layers = vec![(PlanSourceKind::Service, &model.model_id, &model.parameters)];
    if let Some(global) = inherited_global(graph, binding) {
        layers.push((
            PlanSourceKind::Scope,
            &global.binding_id,
            &global.parameters,
        ));
        let exact = global
            .engines
            .iter()
            .find(|e| e.engine_id == engine.engine_id && e.target == engine.target);
        let candidates: Vec<_> = global
            .engines
            .iter()
            .filter(|e| e.enabled && e.target == engine.target)
            .collect();
        if exact.is_none() && candidates.len() > 1 {
            return Err(error(
                "ambiguous_parameter_inheritance",
                "同目标存在多个全局引擎，需要明确继承身份",
            ));
        }
        if let Some(global_engine) = exact.or_else(|| candidates.first().copied()) {
            layers.push((
                PlanSourceKind::Engine,
                &global_engine.engine_id,
                &global_engine.http_parameters,
            ));
        }
    }
    layers.push((
        PlanSourceKind::Scope,
        &binding.binding_id,
        &binding.parameters,
    ));
    layers.push((
        PlanSourceKind::Engine,
        &engine.engine_id,
        &engine.http_parameters,
    ));
    resolve_parameter_layers(graph, layers, provenance)
}
fn resolve_parameter_layers(
    graph: &ConfigurationV3,
    layers: Vec<(
        PlanSourceKind,
        &String,
        &BTreeMap<String, GenerationParameter>,
    )>,
    provenance: &mut BTreeMap<String, PlanFieldSource>,
) -> AppResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    for (kind, id, layer) in layers {
        for (key, parameter) in layer {
            match parameter {
                GenerationParameter::Inherit {} => {}
                GenerationParameter::Omit {} => {
                    values.remove(key);
                    provenance.insert(
                        format!("parameters.{key}"),
                        entity_source(graph, kind.clone(), id, &format!("parameters.{key}.omit")),
                    );
                }
                GenerationParameter::Set { value } => {
                    values.insert(key.clone(), value.clone());
                    provenance.insert(
                        format!("parameters.{key}"),
                        entity_source(graph, kind.clone(), id, &format!("parameters.{key}.set")),
                    );
                }
            }
        }
    }
    for key in values.keys() {
        if matches!(
            key.as_str(),
            "model" | "messages" | "stream" | "tools" | "tool_choice" | "api_key" | "authorization"
        ) {
            return Err(error(
                "reserved_parameter",
                "参数不能覆盖协议身份、凭据或引入工具调用",
            ));
        }
    }
    Ok(values)
}
fn inherited_global<'a>(
    graph: &'a ConfigurationV3,
    binding: &PurposeBindingV3,
) -> Option<&'a PurposeBindingV3> {
    (binding.scope_key != "global")
        .then(|| {
            graph
                .bindings
                .iter()
                .find(|b| b.scope_key == "global" && b.purpose == binding.purpose)
        })
        .flatten()
}
fn select_template(
    graph: &ConfigurationV3,
    binding: &PurposeBindingV3,
    engine: &EngineV3,
    model: &ModelV3,
) -> AppResult<(Option<Template>, Option<PlanFieldSource>)> {
    let mut candidates = vec![
        (
            engine.http_template_override.as_deref(),
            PlanSourceKind::Engine,
            engine.engine_id.as_str(),
        ),
        (
            binding.template_id.as_deref(),
            PlanSourceKind::Scope,
            binding.binding_id.as_str(),
        ),
    ];
    if let Some(global) = inherited_global(graph, binding) {
        let exact = global
            .engines
            .iter()
            .find(|e| e.engine_id == engine.engine_id && e.target == engine.target);
        let matching: Vec<_> = global
            .engines
            .iter()
            .filter(|e| e.enabled && e.target == engine.target)
            .collect();
        let distinct: std::collections::BTreeSet<_> = matching
            .iter()
            .map(|e| e.http_template_override.as_deref())
            .collect();
        if exact.is_none() && distinct.len() > 1 && candidates.iter().all(|c| c.0.is_none()) {
            return Err(error(
                "ambiguous_template_inheritance",
                "全局模板继承存在歧义",
            ));
        }
        if let Some(e) = exact.or_else(|| {
            (distinct.len() <= 1)
                .then(|| matching.first().copied())
                .flatten()
        }) {
            candidates.push((
                e.http_template_override.as_deref(),
                PlanSourceKind::Engine,
                e.engine_id.as_str(),
            ));
        }
        candidates.push((
            global.template_id.as_deref(),
            PlanSourceKind::Scope,
            global.binding_id.as_str(),
        ));
    }
    candidates.push((
        model
            .default_templates
            .get(&binding.purpose)
            .map(String::as_str),
        PlanSourceKind::Service,
        model.model_id.as_str(),
    ));
    choose_template(graph, model, binding.purpose, candidates)
}
fn choose_template(
    graph: &ConfigurationV3,
    model: &ModelV3,
    purpose: CatalogPurpose,
    candidates: Vec<(Option<&str>, PlanSourceKind, &str)>,
) -> AppResult<(Option<Template>, Option<PlanFieldSource>)> {
    let Some((id, kind, owner)) = candidates
        .into_iter()
        .find_map(|(id, kind, owner)| id.map(|id| (id, kind, owner)))
    else {
        return Ok((None, None));
    };
    let selected = checked_template(graph, model, purpose, id)?;
    Ok((
        Some(selected),
        Some(entity_source(graph, kind, owner, "template")),
    ))
}
fn checked_template(
    graph: &ConfigurationV3,
    model: &ModelV3,
    purpose: CatalogPurpose,
    id: &str,
) -> AppResult<Template> {
    let t = graph
        .templates
        .iter()
        .find(|t| t.template_id == id && t.purpose == purpose)
        .ok_or_else(|| error("missing_template", "所选模板不存在或用途不匹配"))?;
    let legacy = Template {
        template_id: t.template_id.clone(),
        purpose: purpose_name(t.purpose).into(),
        display_name: t.display_name.clone(),
        source_kind: t.source_kind.clone(),
        source_version: t.source_version.clone(),
        body: t.body.clone(),
        variables: t.variables.clone(),
        compatibility: t.compatibility.clone(),
    };
    if templates::owner(&legacy)?.is_some_and(|owner| owner != model.model_id) {
        return Err(error("template_model_scope", "模板属于另一模型配置"));
    }
    for (key, actual) in [
        (
            "protocols",
            Some(
                graph
                    .connections
                    .iter()
                    .find(|c| c.connection_id == model.connection_id)
                    .ok_or_else(|| error("missing_connection", "连接不存在"))?
                    .protocol
                    .as_str(),
            ),
        ),
        ("actualModelIds", model.actual_model_id.as_deref()),
    ] {
        if let Some(list) = legacy.compatibility.get(key) {
            let values = list
                .as_array()
                .filter(|v| v.iter().all(Value::is_string))
                .ok_or_else(|| {
                    error("invalid_template_compatibility", "模板兼容声明须为文字列表")
                })?;
            if actual.is_none_or(|a| !values.iter().any(|v| v.as_str() == Some(a))) {
                return Err(error("template_incompatible", "模板与所选协议或模型不匹配"));
            }
        }
    }
    validate_template(&legacy)?;
    if crate::domain::model_config::is_translation_protocol(
        &graph
            .connections
            .iter()
            .find(|c| c.connection_id == model.connection_id)
            .ok_or_else(|| error("missing_connection", "连接不存在"))?
            .protocol,
    ) {
        return Err(error(
            "unsupported_template_protocol",
            "传统翻译不接受模型提示词模板",
        ));
    }
    Ok(legacy)
}

fn term_sets(graph: &ConfigurationV3) -> Vec<TermSet> {
    graph
        .term_sets
        .iter()
        .map(|s| TermSet {
            term_set_id: s.term_set_id.clone(),
            scope_key: s.scope_key.clone(),
            game_id: s.game_id.clone(),
            display_name: s.display_name.clone(),
            enabled: s.enabled,
            terms: s
                .terms
                .iter()
                .map(|t| Term {
                    term_id: t.term_id.clone(),
                    source_text: t.source_text.clone(),
                    target_text: t.target_text.clone(),
                    source_language: t.source_language.clone(),
                    target_language: t.target_language.clone(),
                    enabled: t.enabled,
                })
                .collect(),
        })
        .collect()
}
fn mode_name(mode: &HistoryInputMode) -> &'static str {
    match mode {
        HistoryInputMode::Original => "original",
        HistoryInputMode::Translation => "translation",
        HistoryInputMode::Both => "both",
    }
}
fn policy_sources(
    graph: &ConfigurationV3,
    scope: &RequestScope<'_>,
    service_id: &str,
    policy: &EffectiveServicePolicy,
    provenance: &mut BTreeMap<String, PlanFieldSource>,
) {
    for (field, layer) in &policy.sources {
        let (kind, id) = match layer {
            PolicySource::Service => (PlanSourceKind::Service, service_id),
            PolicySource::Scope => (
                PlanSourceKind::Scope,
                scope.binding_id().expect("scope policy source"),
            ),
            PolicySource::Engine => (
                PlanSourceKind::Engine,
                scope.engine_id().expect("engine policy source"),
            ),
        };
        let fields: &[&str] = match field.as_str() {
            "context" => &["context.enabled", "context.maxSentences", "context.mode"],
            "cache" => &["cache.enabled"],
            "timeoutMs" => &["execution.timeoutMs"],
            "concurrency" => &["execution.concurrency"],
            "retries" => &["execution.retries"],
            _ => &[],
        };
        for name in fields {
            provenance.insert(
                (*name).into(),
                entity_source(graph, kind.clone(), id, field),
            );
        }
    }
}
/// Compile one exact slot. This never chooses another slot, applies configuration, unlocks secrets,
/// verifies an installation, or changes the semantics-v2 hash of a legacy dispatch.
pub fn compose_service_plan(input: ServicePlanInput<'_>) -> AppResult<ComposedServicePlan> {
    validate_input(&input)?;
    let graph = input.graph;
    let binding = graph
        .bindings
        .iter()
        .find(|b| b.binding_id == input.binding_id)
        .ok_or_else(|| error("missing_binding", "用途绑定不存在"))?;
    if binding.semantics_version != PurposeSemanticsVersion::TypedV3 {
        return Err(error(
            "legacy_dispatch_required",
            "此绑定保留LegacyV2语义，必须读取真实旧来源并走原compose",
        ));
    }
    for language in [
        binding.source_language.as_deref(),
        binding.target_language.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if language.trim().is_empty() || language.chars().any(char::is_control) {
            return Err(error(
                "invalid_scope_language",
                "显式用途语言为空或包含控制字符，不能当作继承",
            ));
        }
    }
    if !binding.enabled
        || binding.binding_mode != "override"
        || (binding.game_id.is_some() && binding.game_id != input.unit.game_id)
    {
        return Err(error(
            "inactive_service_binding",
            "用途未有效启用或不属于该游戏",
        ));
    }
    let engine = binding
        .engines
        .iter()
        .find(|e| e.engine_id == input.engine_id)
        .ok_or_else(|| error("missing_engine", "所选引擎槽不存在"))?;
    if !engine.enabled {
        return Err(error("inactive_service_engine", "所选引擎槽未启用"));
    }
    service_config_preflight(graph, &engine.target, binding.purpose, input.catalog)?;
    let identity = ServicePlanIdentity {
        configuration_revision: graph.revision_id.clone(),
        binding_id: binding.binding_id.clone(),
        engine_id: engine.engine_id.clone(),
        purpose: binding.purpose,
        unit_id: input.unit.unit_id.clone(),
        text_revision: input.unit.text_revision,
        source_hash: input.unit.source_hash.clone(),
        session_id: input.unit.session_id.clone(),
        game_id: input.unit.game_id.clone(),
    };
    let mut provenance = BTreeMap::new();
    for field in [
        "sourceText",
        "sourceHash",
        "unitId",
        "textRevision",
        "sessionId",
        "gameId",
    ] {
        provenance.insert(
            field.into(),
            source(
                PlanSourceKind::ReadingUnit,
                None,
                Some(&input.unit.unit_id),
                field,
            ),
        );
    }
    for (field, id) in [
        ("bindingId", binding.binding_id.as_str()),
        ("engineId", engine.engine_id.as_str()),
    ] {
        provenance.insert(
            field.into(),
            entity_source(graph, PlanSourceKind::Scope, id, field),
        );
    }
    provenance.insert(
        "purpose".into(),
        entity_source(graph, PlanSourceKind::Scope, &binding.binding_id, "purpose"),
    );
    let compiled = compile_target(
        RequestContext::from(&input),
        &engine.target,
        RequestScope::Active { binding, engine },
        provenance,
    )?;
    Ok(ServiceExecutionPlan {
        identity,
        policy: compiled.policy,
        fallback: compiled.configured_fallback,
        provenance: compiled.provenance,
        context_units: compiled.context_units,
        term_sources: compiled.term_sources,
        target: compiled.target,
    })
}
struct RequestContext<'a> {
    graph: &'a ConfigurationV3,
    unit: &'a ReadingUnit,
    selected_token: Option<&'a LexicalToken>,
    history: &'a [ReadingUnit],
    source_language: &'a str,
    target_language: &'a str,
    catalog: &'a ServiceCatalog,
}
impl<'a> From<&ServicePlanInput<'a>> for RequestContext<'a> {
    fn from(input: &ServicePlanInput<'a>) -> Self {
        Self {
            graph: input.graph,
            unit: input.unit,
            selected_token: input.selected_token,
            history: input.history,
            source_language: input.source_language,
            target_language: input.target_language,
            catalog: input.catalog,
        }
    }
}
/// Borrows only real active entities, or an explicit saved-service test purpose.
/// ServiceOnly is not an activation and has no binding/engine identity.
enum RequestScope<'a> {
    Active {
        binding: &'a PurposeBindingV3,
        engine: &'a EngineV3,
    },
    ServiceOnly {
        purpose: CatalogPurpose,
        output_mode: ServiceTestOutputMode,
        output_mode_explicit: bool,
    },
}
impl RequestScope<'_> {
    fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }
    fn purpose(&self) -> CatalogPurpose {
        match self {
            Self::Active { binding, .. } => binding.purpose,
            Self::ServiceOnly { purpose, .. } => *purpose,
        }
    }
    fn output_mode(&self) -> Option<&str> {
        match self {
            Self::Active { binding, .. } => binding.output_mode.as_deref(),
            Self::ServiceOnly { output_mode, .. } => Some(match output_mode {
                ServiceTestOutputMode::Text => "text",
                ServiceTestOutputMode::Structured => "structured",
            }),
        }
    }
    fn source_language(&self) -> Option<&str> {
        match self {
            Self::Active { binding, .. } => binding.source_language.as_deref(),
            Self::ServiceOnly { .. } => None,
        }
    }
    fn target_language(&self) -> Option<&str> {
        match self {
            Self::Active { binding, .. } => binding.target_language.as_deref(),
            Self::ServiceOnly { .. } => None,
        }
    }
    fn binding_id(&self) -> Option<&str> {
        match self {
            Self::Active { binding, .. } => Some(&binding.binding_id),
            Self::ServiceOnly { .. } => None,
        }
    }
    fn engine_id(&self) -> Option<&str> {
        match self {
            Self::Active { engine, .. } => Some(&engine.engine_id),
            Self::ServiceOnly { .. } => None,
        }
    }
    fn has_scope_template(&self) -> bool {
        matches!(self, Self::Active { binding, .. } if binding.template_id.is_some())
    }
    fn has_scope_parameters(&self) -> bool {
        matches!(self, Self::Active { binding, .. } if !binding.parameters.is_empty())
    }
    fn policy(&self, defaults: &ServiceRequestDefaults) -> EffectiveServicePolicy {
        match self {
            Self::Active { binding, engine } => {
                effective_policy(defaults, &binding.scope_overrides, &engine.policy_overrides)
            }
            Self::ServiceOnly { .. } => effective_policy(
                defaults,
                &PolicyOverrides::default(),
                &PolicyOverrides::default(),
            ),
        }
    }
    fn policy_sources(
        &self,
        graph: &ConfigurationV3,
        service: &str,
        policy: &EffectiveServicePolicy,
        provenance: &mut BTreeMap<String, PlanFieldSource>,
    ) {
        policy_sources(graph, self, service, policy, provenance);
    }
    fn parameters(
        &self,
        graph: &ConfigurationV3,
        model: &ModelV3,
        provenance: &mut BTreeMap<String, PlanFieldSource>,
    ) -> AppResult<BTreeMap<String, Value>> {
        match self {
            Self::Active { binding, engine } => {
                generation_parameters(graph, binding, engine, model, provenance)
            }
            Self::ServiceOnly { .. } => resolve_parameter_layers(
                graph,
                vec![(PlanSourceKind::Service, &model.model_id, &model.parameters)],
                provenance,
            ),
        }
    }
    fn template(
        &self,
        graph: &ConfigurationV3,
        model: &ModelV3,
    ) -> AppResult<(Option<Template>, Option<PlanFieldSource>)> {
        match self {
            Self::Active { binding, engine } => select_template(graph, binding, engine, model),
            Self::ServiceOnly { purpose, .. } => choose_template(
                graph,
                model,
                *purpose,
                vec![(
                    model.default_templates.get(purpose).map(String::as_str),
                    PlanSourceKind::Service,
                    &model.model_id,
                )],
            ),
        }
    }
    fn output_source(&self, graph: &ConfigurationV3) -> PlanFieldSource {
        match self {
            Self::Active { binding, .. } => entity_source(
                graph,
                PlanSourceKind::Scope,
                &binding.binding_id,
                "outputMode",
            ),
            Self::ServiceOnly {
                output_mode_explicit,
                ..
            } => source(
                if *output_mode_explicit {
                    PlanSourceKind::TestRequest
                } else {
                    PlanSourceKind::Builtin
                },
                None,
                None,
                if *output_mode_explicit {
                    "savedServiceTest.outputMode"
                } else {
                    "savedServiceTest.defaultTextV1"
                },
            ),
        }
    }
}
struct CompiledTarget {
    policy: EffectiveServicePolicy,
    configured_fallback: Vec<SavedServiceRef>,
    provenance: BTreeMap<String, PlanFieldSource>,
    context_units: Vec<(String, u32)>,
    term_sources: Vec<(String, String)>,
    target: ServiceExecutionTarget<RequestPlan>,
}
fn resolve_http_languages(
    graph: &ConfigurationV3,
    scope: &RequestScope<'_>,
    defaults: &ServiceRequestDefaults,
    service_id: &str,
    input_source_language: &str,
    input_target_language: &str,
    provenance: &mut BTreeMap<String, PlanFieldSource>,
) -> AppResult<(String, String)> {
    let ServiceLanguageDefaults::Http {
        source: source_choice,
        target: target_choice,
    } = &defaults.languages
    else {
        return Err(error(
            "service_defaults_target_mismatch",
            "HTTP目标需要HTTP语言默认值",
        ));
    };
    let (source_language, source_provenance) = if let Some(language) = scope.source_language() {
        (
            language.to_owned(),
            entity_source(
                graph,
                PlanSourceKind::Scope,
                scope.binding_id().expect("active scope language override"),
                "sourceLanguage",
            ),
        )
    } else {
        match source_choice {
            SourceLanguageChoice::InheritSource {} => (
                input_source_language.to_owned(),
                source(
                    PlanSourceKind::ReadingPreference,
                    None,
                    None,
                    "sourceLanguage",
                ),
            ),
            SourceLanguageChoice::Explicit { language } => (
                language.clone(),
                entity_source(
                    graph,
                    PlanSourceKind::Service,
                    service_id,
                    "requestDefaults.languages.source",
                ),
            ),
        }
    };
    let (target_language, target_provenance) = if let Some(language) = scope.target_language() {
        (
            language.to_owned(),
            entity_source(
                graph,
                PlanSourceKind::Scope,
                scope.binding_id().expect("active scope language override"),
                "targetLanguage",
            ),
        )
    } else {
        match target_choice {
            TargetLanguageChoice::InheritPreference {} => (
                input_target_language.to_owned(),
                source(
                    PlanSourceKind::ReadingPreference,
                    None,
                    None,
                    "targetLanguage",
                ),
            ),
            TargetLanguageChoice::Explicit { language } => (
                language.clone(),
                entity_source(
                    graph,
                    PlanSourceKind::Service,
                    service_id,
                    "requestDefaults.languages.target",
                ),
            ),
        }
    };
    provenance.insert("sourceLanguage".into(), source_provenance);
    provenance.insert("targetLanguage".into(), target_provenance);
    Ok((source_language, target_language))
}
fn compile_target(
    input: RequestContext<'_>,
    target: &EngineTarget,
    scope: RequestScope<'_>,
    mut provenance: BTreeMap<String, PlanFieldSource>,
) -> AppResult<CompiledTarget> {
    let graph = input.graph;
    let terms = term_sets(graph);
    let (defaults, service_id) = match target {
        EngineTarget::Http { model_id } => {
            let model = graph
                .models
                .iter()
                .find(|m| m.model_id == *model_id)
                .ok_or_else(|| error("missing_model_configuration", "所选模型配置不存在"))?;
            (
                model
                    .request_defaults
                    .get(&scope.purpose())
                    .ok_or_else(|| error("missing_request_defaults", "该用途缺少服务请求默认值"))?,
                model.model_id.as_str(),
            )
        }
        EngineTarget::Offline { offline_config_id } => {
            let config = graph
                .offline_engine_configs
                .iter()
                .find(|c| c.id == *offline_config_id)
                .ok_or_else(|| error("missing_offline_configuration", "所选离线配置不存在"))?;
            (&config.request_defaults, config.id.as_str())
        }
    };
    let policy = scope.policy(defaults);
    scope.policy_sources(graph, service_id, &policy, &mut provenance);
    provenance.insert(
        "fallback".into(),
        entity_source(
            graph,
            PlanSourceKind::Service,
            service_id,
            "requestDefaults.fallback",
        ),
    );
    let context_units;
    let term_sources;
    let target = match target {
        EngineTarget::Http { model_id } => {
            let model = graph
                .models
                .iter()
                .find(|m| m.model_id == *model_id)
                .ok_or_else(|| error("missing_model_configuration", "模型配置不存在"))?;
            let connection = graph
                .connections
                .iter()
                .find(|c| c.connection_id == model.connection_id)
                .ok_or_else(|| error("missing_connection", "连接不存在"))?;
            if scope.is_active() && !connection.availability_intent {
                return Err(error("connection_disabled", "连接的可用意图已关闭"));
            }
            let (options, origin) =
                validate_http(input.catalog, connection, model, scope.purpose())?;
            let (source_language, target_language) = resolve_http_languages(
                graph,
                &scope,
                defaults,
                service_id,
                input.source_language,
                input.target_language,
                &mut provenance,
            )?;
            for field in [
                "protocol",
                "endpoint",
                "adapterBinding",
                "httpOptions",
                "origin",
                "credentialPoolId",
                "credentialPoolRevision",
                "googleAuthorization",
                "proxyMode",
                "networkMetadata",
            ] {
                provenance.insert(
                    field.into(),
                    entity_source(
                        graph,
                        PlanSourceKind::Service,
                        &connection.connection_id,
                        field,
                    ),
                );
            }
            for (prefix, value) in [
                ("httpOptions", serde_json::to_value(options)),
                ("origin", serde_json::to_value(origin)),
            ] {
                if let Ok(Value::Object(fields)) = value {
                    for field in fields.keys() {
                        provenance.insert(
                            format!("{prefix}.{field}"),
                            entity_source(
                                graph,
                                PlanSourceKind::Service,
                                &connection.connection_id,
                                &format!("{prefix}.{field}"),
                            ),
                        );
                    }
                }
            }
            provenance.insert(
                "model".into(),
                entity_source(
                    graph,
                    PlanSourceKind::Service,
                    &model.model_id,
                    "actualModelId",
                ),
            );
            let (template, template_source) = scope.template(graph, model)?;
            provenance.insert(
                "template".into(),
                template_source.unwrap_or_else(|| {
                    source(PlanSourceKind::Builtin, None, None, "purposePrompt")
                }),
            );
            let frozen_template = template
                .as_ref()
                .and_then(|t| {
                    graph
                        .templates
                        .iter()
                        .find(|v| v.template_id == t.template_id)
                })
                .cloned();
            if let Some(t) = &frozen_template {
                for field in [
                    "templateId",
                    "purpose",
                    "sourceKind",
                    "sourceVersion",
                    "body",
                    "variables",
                    "compatibility",
                ] {
                    provenance.insert(
                        format!("template.{field}"),
                        entity_source(graph, PlanSourceKind::Template, &t.template_id, field),
                    );
                }
            }
            let structured = scope.purpose() != CatalogPurpose::Translation
                && scope.output_mode() == Some("structured");
            let text = compose_request_text(
                RequestTextInput {
                    unit: input.unit,
                    selected_token: input.selected_token,
                    purpose: purpose_name(scope.purpose()),
                    source: &source_language,
                    target: &target_language,
                    context_enabled: policy.context.enabled,
                    context_count: policy.context.max_sentences as usize,
                    context_mode: mode_name(&policy.context.mode),
                    term_sets: &terms,
                    history: input.history,
                    structured,
                },
                || Ok(template),
            )?;
            context_units = text.context_units;
            term_sources = text.term_sources;
            let parameters = scope.parameters(graph, model, &mut provenance)?;
            generation_wire_parameters(
                &connection
                    .adapter_binding
                    .as_ref()
                    .ok_or_else(|| error("missing_adapter_binding", "缺少精确协议绑定"))?
                    .protocol,
                &parameters,
            )?;
            let stream = model
                .capabilities
                .get("streaming")
                .and_then(Value::as_bool)
                .unwrap_or(!crate::domain::model_config::is_translation_protocol(
                    &connection.protocol,
                ));
            provenance.insert(
                "stream".into(),
                entity_source(
                    graph,
                    PlanSourceKind::Service,
                    &model.model_id,
                    if model.capabilities.contains_key("streaming") {
                        "capabilities.streaming"
                    } else {
                        "protocolDefault.streaming"
                    },
                ),
            );
            provenance.insert("structured".into(), scope.output_source(graph));
            let request = RequestPlan {
                protocol: connection.protocol.clone(),
                endpoint: validate_endpoint(connection.endpoint.as_deref())?.into(),
                model: model.actual_model_id.clone(),
                source_text: input.unit.source_text.clone(),
                source_language,
                target_language,
                messages: text.messages,
                parameters,
                structured,
                stream,
                timeout_ms: policy.execution.timeout_ms as u64,
                adapter_binding: connection.adapter_binding.clone(),
                purpose: Some(scope.purpose()),
            };
            ServiceExecutionTarget::Http {
                plan: Box::new(FrozenHttpServicePlan {
                    connection: connection.clone(),
                    model: model.clone(),
                    http_options: options.clone(),
                    origin: origin.clone(),
                    request,
                    template: frozen_template,
                    reference_material: text.reference_material,
                }),
            }
        }
        EngineTarget::Offline { offline_config_id } => {
            let config = graph
                .offline_engine_configs
                .iter()
                .find(|c| c.id == *offline_config_id)
                .ok_or_else(|| error("missing_offline_configuration", "离线配置不存在"))?;
            if scope.purpose() != CatalogPurpose::Translation
                || scope.has_scope_template()
                || scope.has_scope_parameters()
                || scope.output_mode().is_some_and(|mode| mode != "text")
            {
                return Err(error(
                    "offline_http_field_forbidden",
                    "离线翻译不接受HTTP模板、生成参数或结构化解释",
                ));
            }
            if !matches!(
                defaults.languages,
                ServiceLanguageDefaults::OfflineUseConfigPair {}
            ) || config.pair.source != scope.source_language().unwrap_or(input.source_language)
                || config.pair.target != scope.target_language().unwrap_or(input.target_language)
            {
                return Err(error(
                    "offline_language_pair_mismatch",
                    "离线配置的语言对必须与实际请求方向完全一致",
                ));
            }
            for field in [
                "sourceLanguage",
                "targetLanguage",
                "engine",
                "backend",
                "pair",
                "routePolicy",
                "installation",
                "resolution",
            ] {
                provenance.insert(
                    field.into(),
                    entity_source(graph, PlanSourceKind::Service, &config.id, field),
                );
            }
            for (field, explicit) in [
                ("sourceLanguage", scope.source_language()),
                ("targetLanguage", scope.target_language()),
            ] {
                if explicit.is_some() {
                    provenance.insert(
                        field.into(),
                        entity_source(
                            graph,
                            PlanSourceKind::Scope,
                            scope.binding_id().expect("active scope language override"),
                            field,
                        ),
                    );
                }
            }
            let material = compose_reference_material(ReferenceMaterialInput {
                unit: input.unit,
                context_enabled: policy.context.enabled,
                context_count: policy.context.max_sentences as usize,
                context_mode: mode_name(&policy.context.mode),
                term_sets: &terms,
                history: input.history,
            })?;
            context_units = material.context_units;
            term_sources = material.term_sources;
            ServiceExecutionTarget::Offline {
                config: Box::new(config.clone()),
                payload: Box::new(OfflineRequestPayload {
                    source_text: input.unit.source_text.clone(),
                    source_language: config.pair.source.clone(),
                    target_language: config.pair.target.clone(),
                    context: material.context,
                    glossary: material.glossary,
                    selected_token: input.selected_token.cloned(),
                }),
            }
        }
    };
    for (index, (unit, revision)) in context_units.iter().enumerate() {
        provenance.insert(
            format!("context.units.{index}"),
            source(
                PlanSourceKind::ReadingUnit,
                None,
                Some(unit),
                &format!("textRevision:{revision}"),
            ),
        );
    }
    for (index, (set, term)) in term_sources.iter().enumerate() {
        provenance.insert(
            format!("glossary.{index}"),
            entity_source(graph, PlanSourceKind::Term, set, term),
        );
    }
    if input.selected_token.is_some() {
        provenance.insert(
            "selectedToken".into(),
            source(
                PlanSourceKind::ReadingUnit,
                None,
                Some(&input.unit.unit_id),
                "selectedToken",
            ),
        );
    }
    Ok(CompiledTarget {
        policy,
        configured_fallback: defaults.fallback.clone(),
        provenance,
        context_units,
        term_sources,
        target,
    })
}

fn static_parameters(model: &ModelV3) -> BTreeMap<String, Value> {
    model
        .parameters
        .iter()
        .filter_map(|(key, value)| match value {
            GenerationParameter::Set { value } => Some((key.clone(), value.clone())),
            GenerationParameter::Inherit {} | GenerationParameter::Omit {} => None,
        })
        .collect()
}
fn known_direction(
    template: &Template,
    source: Option<&str>,
    target: Option<&str>,
) -> AppResult<()> {
    for (key, actual) in [("sourceLanguages", source), ("targetLanguages", target)] {
        if let Some(value) = template.compatibility.get(key) {
            let list = value
                .as_array()
                .filter(|v| v.iter().all(Value::is_string))
                .ok_or_else(|| {
                    error("invalid_template_compatibility", "模板语言条件须为文字列表")
                })?;
            if actual.is_some_and(|a| !list.iter().any(|v| v.as_str() == Some(a))) {
                return Err(error(
                    "template_incompatible",
                    "模板与明确配置的语言方向不匹配",
                ));
            }
        }
    }
    Ok(())
}
fn request_default_languages(defaults: &ServiceRequestDefaults) -> (Option<&str>, Option<&str>) {
    match &defaults.languages {
        ServiceLanguageDefaults::Http { source, target } => (
            match source {
                SourceLanguageChoice::Explicit { language } => Some(language.as_str()),
                _ => None,
            },
            match target {
                TargetLanguageChoice::Explicit { language } => Some(language.as_str()),
                _ => None,
            },
        ),
        ServiceLanguageDefaults::OfflineUseConfigPair {} => (None, None),
    }
}
fn static_material_values(
    connection: &ConnectionV3,
    model: &ModelV3,
) -> BTreeMap<String, CatalogScalar> {
    let mut values = BTreeMap::new();
    let mut text = |key: &str, value: Option<&str>| {
        if let Some(value) = value {
            values.insert(key.into(), CatalogScalar::Text(value.into()));
        }
    };
    text("endpoint", connection.endpoint.as_deref());
    text("model", model.actual_model_id.as_deref());
    text("auth_mode", Some(&connection.auth_mode));
    if let Some(origin) = &connection.origin {
        match origin {
            ConnectionOrigin::Preset { site, .. } => {
                text("connection_mode", Some("preset"));
                text("provider_site", site.as_deref());
            }
            ConnectionOrigin::Custom { site, .. } => {
                text("connection_mode", Some("custom"));
                text("provider_site", site.as_deref());
            }
        }
    }
    match connection.http_options.as_ref() {
        Some(HttpServiceOptionsV1::AzureTranslator {
            resource_type,
            region,
        }) => {
            let resource = resource_type.as_ref().map(|v| match v {
                AzureResourceType::Global => "global",
                AzureResourceType::Regional => "regional",
                AzureResourceType::MultiService => "multi_service",
            });
            text("resource_type", resource);
            text("region", region.as_deref());
        }
        Some(HttpServiceOptionsV1::DeeplV2 { plan }) => text(
            "plan",
            plan.as_ref().map(|v| match v {
                DeeplPlan::Free => "free",
                DeeplPlan::Pro => "pro",
            }),
        ),
        Some(HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id,
            location,
        }) => {
            text("project_id", Some(project_id));
            text("location", Some(location));
        }
        _ => {}
    }
    match connection.auth_mode.as_str() {
        "google_oauth" => text("google_auth_method", Some("oauth")),
        "google_service_account" => text("google_auth_method", Some("service_account")),
        _ => {}
    }
    values
}
fn material_preflight(
    connection: &ConnectionV3,
    model: &ModelV3,
    directory: &ServiceCatalog,
    requirements: &mut Vec<ServicePreflightRequirement>,
) -> AppResult<()> {
    let Some(ConnectionOrigin::Preset { key, .. }) = &connection.origin else {
        return Ok(());
    };
    let preset = directory
        .presets
        .iter()
        .find(|p| p.key == *key)
        .ok_or_else(|| error("unknown_preset_version", "精确预设版本不存在"))?;
    let values = static_material_values(connection, model);
    // Choice identity is user intent. Never infer the first displayed site or plan.
    for field in &preset.fields {
        if !matches!(
            field.id.as_str(),
            "provider_site" | "resource_type" | "plan"
        ) {
            continue;
        }
        let condition = catalog::evaluate_condition(&field.condition, &values);
        if condition == CapabilityState::Unsupported {
            continue;
        }
        let value = values.get(&field.id);
        let required = field.required_when_visible
            || field.classification == ServiceFieldClassification::Required;
        if condition == CapabilityState::Supported && required && value.is_none() {
            return Err(error(
                "required_service_choice_missing",
                "必要服务选项尚未明确选择",
            ));
        }
        if condition == CapabilityState::Unknown && required && value.is_none() {
            requirements.push(ServicePreflightRequirement::MaterialCondition {
                reason: CatalogReason::ConditionUnresolved,
            });
        }
        if let Some(value) = value
            && field.field_type == ServiceFieldType::Select
            && !field.choices.iter().any(|c| &c.value == value)
        {
            return Err(error(
                "invalid_service_choice",
                "已保存服务选项不属于精确预设合同",
            ));
        }
    }
    for blocker in &preset.material_blockers {
        match catalog::evaluate_condition(&blocker.condition, &values) {
            CapabilityState::Supported => {
                return Err(error(
                    match blocker.reason {
                        CatalogReason::PresetSiteMaterialIncomplete => "preset_material_incomplete",
                        CatalogReason::ApplicationOauthSetupUnverified => {
                            "application_oauth_setup_unverified"
                        }
                        _ => "service_material_blocked",
                    },
                    "精确预设资料条件已明确阻断",
                ));
            }
            CapabilityState::Unknown => {
                requirements.push(ServicePreflightRequirement::MaterialCondition {
                    reason: blocker.reason,
                })
            }
            CapabilityState::Unsupported => {}
        }
    }
    Ok(())
}
fn offline_static_preflight(
    config: &OfflineEngineConfigV1,
    purpose: CatalogPurpose,
    directory: &ServiceCatalog,
) -> AppResult<()> {
    if purpose != CatalogPurpose::Translation
        || config.request_defaults.purpose != purpose
        || !matches!(
            config.request_defaults.languages,
            ServiceLanguageDefaults::OfflineUseConfigPair {}
        )
    {
        return Err(error(
            "service_defaults_target_mismatch",
            "离线目标的用途和语言对合同不匹配",
        ));
    }
    let descriptor = directory
        .offline_engines
        .iter()
        .find(|d| {
            d.engine_id == config.engine.engine_id
                && d.contract_revision == config.engine.contract_revision.get()
        })
        .ok_or_else(|| error("unknown_offline_contract", "精确离线契约不存在"))?;
    let backend = match config.backend {
        OfflineBackend::PlatformNative => OfflineExecutionBackend::PlatformNative,
        OfflineBackend::LocalWorker => OfflineExecutionBackend::LocalWorker,
    };
    if descriptor.backend != backend
        || !descriptor.allowed_purposes.contains(&purpose)
        || descriptor.model_identity != CatalogModelIdentity::NotApplicable
    {
        return Err(error(
            "offline_contract_mismatch",
            "离线引擎后端或用途不匹配",
        ));
    }
    if matches!(config.backend, OfflineBackend::LocalWorker) && config.installation.is_none() {
        return Err(error(
            "missing_offline_installation",
            "本地工作进程缺少明确安装版本引用",
        ));
    }
    if !matches!(config.resolution, OfflineConfigResolution::Resolved { .. }) {
        return Err(error("offline_route_unresolved", "离线资源或路线尚未解析"));
    }
    Ok(())
}
/// Validate only a saved target's static configuration. No ReadingUnit, sample, secret,
/// installed-state assertion or automatic network action is manufactured here. Scope/engine
/// overrides require validation again after effective resolution in compose_service_plan.
pub fn service_config_preflight(
    graph: &ConfigurationV3,
    target: &EngineTarget,
    purpose: CatalogPurpose,
    directory: &ServiceCatalog,
) -> AppResult<ServiceConfigPreflight> {
    validate_service_graph(graph)?;
    let mut requirements = vec![ServicePreflightRequirement::InstalledAdapter];
    match target {
        EngineTarget::Http { model_id } => {
            let model = graph
                .models
                .iter()
                .find(|m| m.model_id == *model_id)
                .ok_or_else(|| error("missing_model_configuration", "模型配置不存在"))?;
            let connection = graph
                .connections
                .iter()
                .find(|c| c.connection_id == model.connection_id)
                .ok_or_else(|| error("missing_connection", "连接不存在"))?;
            validate_http(directory, connection, model, purpose)?;
            let defaults = model
                .request_defaults
                .get(&purpose)
                .ok_or_else(|| error("missing_request_defaults", "所选用途缺少服务默认值"))?;
            if defaults.purpose != purpose
                || !matches!(defaults.languages, ServiceLanguageDefaults::Http { .. })
            {
                return Err(error(
                    "service_defaults_target_mismatch",
                    "HTTP目标的用途或语言默认值不匹配",
                ));
            }
            let protocol = &connection
                .adapter_binding
                .as_ref()
                .ok_or_else(|| error("missing_adapter_binding", "缺少精确协议版本"))?
                .protocol;
            generation_wire_parameters(protocol, &static_parameters(model))?;
            if let Some(value) = model.capabilities.get("streaming")
                && !value.is_boolean()
            {
                return Err(error(
                    "invalid_stream_capability",
                    "明确的streaming能力必须为布尔值",
                ));
            }
            if crate::domain::model_config::is_translation_protocol(&connection.protocol)
                && connection.protocol != "tencent_tokenhub_translation"
                && model.capabilities.get("streaming").and_then(Value::as_bool) == Some(true)
            {
                return Err(error(
                    "stream_not_supported",
                    "此传统翻译接口不支持流式输出",
                ));
            }
            let (source, target) = request_default_languages(defaults);
            if let Some(id) = model.default_templates.get(&purpose) {
                let template = checked_template(graph, model, purpose, id)?;
                known_direction(&template, source, target)?;
            }
            if source.is_none() || target.is_none() {
                requirements.push(ServicePreflightRequirement::EffectiveRequestLanguages);
            }
            if connection.auth_mode == "google_oauth" {
                requirements.push(ServicePreflightRequirement::GoogleAuthorizationAuthority);
            } else if connection.auth_mode != "none" {
                requirements.push(ServicePreflightRequirement::CredentialAuthority);
            }
            if model.actual_model_id.is_some() {
                requirements.push(ServicePreflightRequirement::ModelCapabilities);
            }
            requirements.push(ServicePreflightRequirement::NetworkAvailability);
            material_preflight(connection, model, directory, &mut requirements)?;
        }
        EngineTarget::Offline { offline_config_id } => {
            let config = graph
                .offline_engine_configs
                .iter()
                .find(|c| c.id == *offline_config_id)
                .ok_or_else(|| error("missing_offline_configuration", "离线配置不存在"))?;
            offline_static_preflight(config, purpose, directory)?;
            requirements.push(ServicePreflightRequirement::OfflineResourceAuthority);
        }
    }
    Ok(ServiceConfigPreflight {
        target: target.clone(),
        purpose,
        runtime_requirements: requirements,
    })
}

fn semantics_sources(
    mut values: BTreeMap<String, PlanFieldSource>,
) -> BTreeMap<String, PlanFieldSource> {
    for value in values.values_mut() {
        value.revision_id = None;
    }
    values
}
fn semantic_connection(connection: &ConnectionV3) -> Value {
    json!({"connectionId":connection.connection_id,"protocol":connection.protocol,"deployment":connection.deployment,
        "endpoint":connection.endpoint,"authMode":connection.auth_mode,"credentialPoolId":connection.credential_pool_id,
        "credentialPoolRevision":connection.credential_pool_revision,"googleAuthorization":connection.google_authorization,
        "proxyMode":connection.proxy_mode,"networkMetadata":connection.network_metadata,"availabilityIntent":connection.availability_intent,
        "adapterBinding":connection.adapter_binding,"httpOptions":connection.http_options,"origin":connection.origin,
        "presetSource":connection.preset_source,"materializationSource":connection.materialization_source})
}
fn semantic_template(template: &Template) -> Value {
    json!({"templateId":template.template_id,"purpose":template.purpose,"body":template.body,"variables":template.variables,
        "compatibility":template.compatibility,"sourceKind":template.source_kind,"sourceVersion":template.source_version})
}
fn semantic_languages(binding: &PurposeBindingV3, defaults: &ServiceRequestDefaults) -> Value {
    match &defaults.languages {
        ServiceLanguageDefaults::Http { source, target } => json!({
            "source":match &binding.source_language {Some(value)=>json!({"source":"scope","value":value}),None=>json!({"source":"service","value":source})},
            "target":match &binding.target_language {Some(value)=>json!({"source":"scope","value":value}),None=>json!({"source":"service","value":target})}}),
        ServiceLanguageDefaults::OfflineUseConfigPair {} => {
            json!({"kind":"offline_use_config_pair","scopeSource":binding.source_language,"scopeTarget":binding.target_language})
        }
    }
}
fn semantic_scope_terms(graph: &ConfigurationV3, requested_scope: &str) -> Vec<Value> {
    let terms: Vec<_> = graph
        .term_sets
        .iter()
        .filter(|set| {
            set.enabled && (set.scope_key == "global" || set.scope_key == requested_scope)
        })
        .filter_map(|set| {
            let entries: Vec<_> = set
                .terms
                .iter()
                .filter(|t| t.enabled)
                .filter_map(|term| {
                    let source = term.source_text.as_ref().filter(|s| !s.is_empty())?;
                    let target = term.target_text.as_ref()?;
                    Some(json!({"termId":term.term_id,"source":source,"target":target}))
                })
                .collect();
            (!entries.is_empty()).then(
                || json!({"termSetId":set.term_set_id,"scopeKey":set.scope_key,"terms":entries}),
            )
        })
        .collect();
    terms
}
/// Purpose-runtime identity, not a per-reading-unit request/cache identity. The v3 domain
/// marker and effective field sources are part of the contract. Graph revision IDs and
/// display labels are excluded; exact credential/resource/fallback revisions are meaningful.
pub fn purpose_service_semantics_hash(
    graph: &ConfigurationV3,
    binding_id: &str,
) -> AppResult<String> {
    let binding = graph
        .bindings
        .iter()
        .find(|b| b.binding_id == binding_id)
        .ok_or_else(|| error("missing_binding", "用途绑定不存在"))?;
    purpose_service_semantics_hash_for_scope(graph, binding_id, &binding.scope_key)
}
/// A global binding can be evaluated for a real game scope without including every other
/// game's terms. Callers must use the actual task scope, not substitute a global gate hash.
pub fn purpose_service_semantics_hash_for_scope(
    graph: &ConfigurationV3,
    binding_id: &str,
    requested_scope: &str,
) -> AppResult<String> {
    validate_service_graph(graph)?;
    let binding = graph
        .bindings
        .iter()
        .find(|b| b.binding_id == binding_id)
        .ok_or_else(|| error("missing_binding", "用途绑定不存在"))?;
    if binding.semantics_version != PurposeSemanticsVersion::TypedV3 {
        return Err(error(
            "legacy_dispatch_required",
            "LegacyV2用途必须使用原Configuration语义哈希",
        ));
    }
    if requested_scope != "global" {
        let game = requested_scope
            .strip_prefix("game:")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| error("invalid_service_scope", "用途运行范围无效"))?;
        require_id(game)?;
    }
    if binding.binding_mode != "override"
        || (binding.scope_key != "global" && binding.scope_key != requested_scope)
    {
        return Err(error(
            "purpose_scope_mismatch",
            "显式绑定与实际请求范围不匹配",
        ));
    }
    let mut engines = vec![];
    for engine in binding.engines.iter().filter(|e| e.enabled) {
        let mut fields = BTreeMap::new();
        match &engine.target {
            EngineTarget::Http { model_id } => {
                let model = graph
                    .models
                    .iter()
                    .find(|m| m.model_id == *model_id)
                    .ok_or_else(|| error("missing_model_configuration", "模型配置不存在"))?;
                let connection = graph
                    .connections
                    .iter()
                    .find(|c| c.connection_id == model.connection_id)
                    .ok_or_else(|| error("missing_connection", "连接不存在"))?;
                let defaults = model
                    .request_defaults
                    .get(&binding.purpose)
                    .ok_or_else(|| error("missing_request_defaults", "所选用途缺少服务默认值"))?;
                let policy =
                    effective_policy(defaults, &binding.scope_overrides, &engine.policy_overrides);
                let parameters = generation_parameters(graph, binding, engine, model, &mut fields)?;
                let (template, template_source) = select_template(graph, binding, engine, model)?;
                let stream = model
                    .capabilities
                    .get("streaming")
                    .and_then(Value::as_bool)
                    .unwrap_or(!crate::domain::model_config::is_translation_protocol(
                        &connection.protocol,
                    ));
                engines.push(json!({"engineId":engine.engine_id,"target":engine.target,"modelId":model.model_id,
                    "connection":semantic_connection(connection),"actualModelId":model.actual_model_id,
                    "languages":semantic_languages(binding,defaults),"policy":policy,"fallback":defaults.fallback,
                    "parameters":parameters,"parameterSources":semantics_sources(fields),
                    "template":template.as_ref().map(semantic_template),"templateSource":template_source.map(|mut v|{v.revision_id=None;v}),
                    "stream":stream,"streamSource":if model.capabilities.contains_key("streaming"){"model_capability"}else{"protocol_default"}}));
            }
            EngineTarget::Offline { offline_config_id } => {
                let config = graph
                    .offline_engine_configs
                    .iter()
                    .find(|c| c.id == *offline_config_id)
                    .ok_or_else(|| error("missing_offline_configuration", "离线配置不存在"))?;
                let policy = effective_policy(
                    &config.request_defaults,
                    &binding.scope_overrides,
                    &engine.policy_overrides,
                );
                engines.push(json!({"engineId":engine.engine_id,"target":engine.target,"configId":config.id,"engine":config.engine,
                    "backend":config.backend,"pair":config.pair,"routePolicy":config.route_policy,"installation":config.installation,
                    "resolution":config.resolution,"languages":semantic_languages(binding,&config.request_defaults),"policy":policy,"fallback":config.request_defaults.fallback}));
            }
        }
    }
    // No sentence exists at this layer. Include only potentially matching, enabled complete
    // terms from the actual requested scope; per-sentence filtering remains shared composition.
    let terms = semantic_scope_terms(graph, requested_scope);
    crate::domain::canonical_hash(&(
        "purpose-service-semantics-v3",
        json!({
        "bindingId":binding.binding_id,"scopeKey":binding.scope_key,"requestedScope":requested_scope,"gameId":binding.game_id,
        "purpose":binding.purpose,"enabled":binding.enabled,"structured":binding.purpose!=CatalogPurpose::Translation && binding.output_mode.as_deref()==Some("structured"),
        "engines":engines,"terms":terms}),
    ))
}
