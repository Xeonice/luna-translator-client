//! Read-only projection of an exact saved resource into its registered ordinary form.
//! Source seals, roles and external facts are loaded and verified by the repository;
//! this pure projection neither manufactures a preset identity nor grants execution.
use super::{
    service_draft::{PreparedServiceDraftResources, materialize_base},
    service_graph::*,
};
use crate::{contracts::*, domain::require_id, ports::service_configuration::ServiceDirectory};
use std::collections::BTreeMap;

/// Non-Serde, non-secret facts prepared from exact immutable pool/grant/installation
/// rows. Reuse the compiler's fact shape so read and write share the same identities.
pub type ServiceEditorFacts = PreparedServiceDraftResources;
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "service_editor", message)
}
fn revision(document: &ConfigurationDocument) -> &str {
    match document {
        ConfigurationDocument::Legacy { configuration } => &configuration.revision_id,
        ConfigurationDocument::ServiceGraph { configuration } => &configuration.revision_id,
    }
}
fn source_revision(source: &SavedServiceRef) -> &str {
    match source {
        SavedServiceRef::Http { source } => &source.revision_id,
        SavedServiceRef::Offline { source } => &source.revision_id,
    }
}
fn source_at(source: &SavedServiceRef, revision: &str) -> SavedServiceRef {
    match source {
        SavedServiceRef::Http { source } => SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: revision.into(),
                model_id: source.model_id.clone(),
            },
        },
        SavedServiceRef::Offline { source } => SavedServiceRef::Offline {
            source: OfflineConfigRef {
                revision_id: revision.into(),
                config_id: source.config_id.clone(),
            },
        },
    }
}
#[derive(Clone, PartialEq)]
enum Closure {
    Http {
        model: ModelV3,
        connection: Box<ConnectionV3>,
        templates: Vec<TemplateV3>,
    },
    Offline(OfflineEngineConfigV1),
}
/// Pure source-purpose membership shared with explicit model-list preparation.
/// This grants neither repository provenance nor execution permission.
pub fn validate_http_source_purpose(graph: &ConfigurationV3, model: &ModelV3, purpose: CatalogPurpose) -> AppResult<()> {
    if model.template_purpose.is_some_and(|p|p!=purpose) || model.template_purpose.is_none() && !model.default_templates.contains_key(&purpose) && !graph.bindings.iter().any(|b|b.purpose==purpose && b.engines.iter().any(|e|matches!(&e.target,EngineTarget::Http{model_id} if model_id==&model.model_id))) {
        return Err(error("service_editor_purpose_mismatch","来源服务与所查询用途不一致或无法核验"));
    }
    Ok(())
}
fn closure(
    graph: &ConfigurationV3,
    source: &SavedServiceRef,
    purpose: CatalogPurpose,
) -> AppResult<Closure> {
    match source {
        SavedServiceRef::Http { source } => {
            require_id(&source.model_id)?;
            let model = graph
                .models
                .iter()
                .find(|m| m.model_id == source.model_id)
                .ok_or_else(|| {
                    error("service_editor_resource_missing", "来源版本中不存在该服务")
                })?;
            validate_http_source_purpose(graph, model, purpose)?;
            let connection = graph
                .connections
                .iter()
                .find(|c| c.connection_id == model.connection_id)
                .ok_or_else(|| error("service_editor_connection_missing", "来源连接不存在"))?;
            let templates = model
                .default_templates
                .values()
                .map(|id| {
                    graph
                        .templates
                        .iter()
                        .find(|t| &t.template_id == id)
                        .cloned()
                        .ok_or_else(|| error("service_editor_template_missing", "来源模板不存在"))
                })
                .collect::<AppResult<Vec<_>>>()?;
            Ok(Closure::Http {
                model: model.clone(),
                connection: Box::new(connection.clone()),
                templates,
            })
        }
        SavedServiceRef::Offline { source } => {
            require_id(&source.config_id)?;
            if purpose != CatalogPurpose::Translation {
                return Err(error(
                    "service_editor_purpose_mismatch",
                    "离线服务仅可查询翻译用途",
                ));
            }
            Ok(Closure::Offline(
                graph
                    .offline_engine_configs
                    .iter()
                    .find(|c| c.id == source.config_id)
                    .cloned()
                    .ok_or_else(|| {
                        error(
                            "service_editor_resource_missing",
                            "来源版本中不存在该离线配置",
                        )
                    })?,
            ))
        }
    }
}
fn put(values: &mut BTreeMap<String, CatalogScalar>, id: &str, value: Option<String>) {
    if let Some(value) = value {
        values.insert(id.into(), CatalogScalar::Text(value));
    }
}
fn common_defaults(
    values: &mut BTreeMap<String, CatalogScalar>,
    defaults: &ServiceRequestDefaults,
) {
    if let ServiceLanguageDefaults::Http { source, target } = &defaults.languages {
        if let SourceLanguageChoice::Explicit { language } = source {
            put(values, "source_language", Some(language.clone()));
        }
        if let TargetLanguageChoice::Explicit { language } = target {
            put(values, "target_language", Some(language.clone()));
        }
    }
    for (id, value) in [
        ("context_count", defaults.context.max_sentences as f64),
        ("timeout", defaults.execution.timeout_ms as f64 / 1000.0),
        ("concurrency", defaults.execution.concurrency as f64),
        ("retries", defaults.execution.retries as f64),
    ] {
        values.insert(id.into(), CatalogScalar::Number(value));
    }
    values.insert(
        "cache".into(),
        CatalogScalar::Boolean(defaults.cache.enabled),
    );
}
fn options(values: &mut BTreeMap<String, CatalogScalar>, options: &HttpServiceOptionsV1) {
    use HttpServiceOptionsV1::*;
    match options {
        OpenaiCompatible {
            organization_id,
            project_id,
        }
        | OpenaiResponses {
            organization_id,
            project_id,
        } => {
            put(values, "organization_id", organization_id.clone());
            put(values, "openai_project_id", project_id.clone());
        }
        Anthropic { workspace_id } => put(values, "workspace_id", workspace_id.clone()),
        AzureTranslator {
            resource_type,
            region,
        } => {
            put(
                values,
                "resource_type",
                resource_type.as_ref().map(|r| {
                    match r {
                        AzureResourceType::Global => "global",
                        AzureResourceType::Regional => "regional",
                        AzureResourceType::MultiService => "multi_service",
                    }
                    .into()
                }),
            );
            put(values, "region", region.clone());
        }
        DeeplV2 { plan } => put(
            values,
            "plan",
            plan.as_ref().map(|p| {
                match p {
                    DeeplPlan::Free => "free",
                    DeeplPlan::Pro => "pro",
                }
                .into()
            }),
        ),
        GoogleTranslateV3 {
            project_id,
            location,
        } => {
            put(values, "project_id", Some(project_id.clone()));
            put(values, "location", Some(location.clone()));
        }
        _ => {}
    }
}
fn exact_target(
    resource: &Closure,
    catalog: &ServiceCatalog,
    purpose: CatalogPurpose,
) -> Option<ServiceFormTarget> {
    match resource {
        Closure::Http {
            model, connection, ..
        } => {
            if model.template_purpose != Some(purpose) {
                return None;
            }
            let (Some(key), Some(binding), Some(options), Some(origin)) = (
                &connection.preset_source,
                &connection.adapter_binding,
                &connection.http_options,
                &connection.origin,
            ) else {
                return None;
            };
            let origin_matches = match origin {
                ConnectionOrigin::Preset { key: origin, .. } => origin == key,
                ConnectionOrigin::Custom { copied_from, .. } => copied_from.as_ref() == Some(key),
            };
            if !origin_matches || options.protocol_id() != connection.protocol {
                return None;
            }
            catalog
                .presets
                .iter()
                .find(|p| {
                    &p.key == key
                        && &p.binding == binding
                        && p.binding.protocol.protocol_id == connection.protocol
                        && p.allowed_purposes.contains(&purpose)
                })
                .map(|p| ServiceFormTarget::Http {
                    preset: p.key.clone(),
                })
        }
        Closure::Offline(config) => catalog
            .offline_engines
            .iter()
            .find(|e| {
                e.engine_id == config.engine.engine_id
                    && e.contract_revision == config.engine.contract_revision.get()
                    && e.allowed_purposes.contains(&purpose)
            })
            .map(|e| ServiceFormTarget::Offline {
                engine_id: e.engine_id.clone(),
                contract_revision: e.contract_revision,
            }),
    }
}
pub fn project_service_editor(
    query: &ServiceDraftQuery,
    source_document: &ConfigurationDocument,
    current: &ServiceConfigurationSnapshot,
    directory: &dyn ServiceDirectory,
    facts: &ServiceEditorFacts,
) -> AppResult<ServiceDraftEditorProjection> {
    if query.expected_profile_generation != current.profile_generation {
        return Err(error(
            "profile_generation_changed",
            "配置档案已切换，请重新打开服务",
        ));
    }
    require_id(source_revision(&query.source))?;
    if source_revision(&query.source) != revision(source_document) {
        return Err(error(
            "service_editor_source_mismatch",
            "查询来源与已加载的精确版本不一致",
        ));
    }
    let graph = materialize_base(source_document)?;
    let resource = closure(&graph, &query.source, query.purpose)?;
    let target = exact_target(&resource, &directory.catalog(), query.purpose);
    let mut reasons = Vec::<String>::new();
    if target.is_none() {
        reasons.push(
            "此来源没有可核验的精确表单身份；请使用原高级配置入口查看，不能猜测预设后覆盖".into(),
        );
    }
    let current_graph = materialize_base(&current.draft)?;
    let current_source = source_at(&query.source, &current_graph.revision_id);
    let equivalent = closure(&current_graph, &current_source, query.purpose)
        .is_ok_and(|current| current == resource);
    if !equivalent {
        reasons.push(
            "当前草稿中的服务及相关模板已变更或不存在；本次只读展示原版本，请重新打开当前服务"
                .into(),
        );
    }
    let mut values = BTreeMap::new();
    let mut authentication = ServiceAuthenticationSelection::None {};
    let mut affected_shared_model_ids = vec![];
    let mut fallback = vec![];
    match &resource {
        Closure::Http {
            model,
            connection,
            templates,
        } => {
            if facts.offline.is_some() {
                return Err(error(
                    "service_editor_fact_mismatch",
                    "HTTP服务不能接收离线安装事实",
                ));
            }
            if let Some(fact) = &facts.credential
                && (connection.credential_pool_id.as_deref() != Some(&fact.pool_id)
                    || connection.credential_pool_revision != Some(fact.pool_revision))
                {
                    return Err(error(
                        "service_editor_fact_mismatch",
                        "凭据事实不属于来源引用",
                    ));
                }
            if let Some(fact) = &facts.google_client
                && connection.google_authorization.as_ref() != Some(&fact.authorization) {
                    return Err(error(
                        "service_editor_fact_mismatch",
                        "Google客户端事实不属于来源授权引用",
                    ));
                }
            put(&mut values, "name", Some(model.display_name.clone()));
            put(&mut values, "model", model.actual_model_id.clone());
            // An explicitly unfinished custom endpoint must not regain a trusted
            // preset URL merely because the editor omitted its empty field.
            put(
                &mut values,
                "endpoint",
                Some(connection.endpoint.clone().unwrap_or_default()),
            );
            put(&mut values, "auth_mode", Some(connection.auth_mode.clone()));
            put(&mut values, "proxy_mode", Some(connection.proxy_mode.clone()));
            if connection.proxy_mode == "override" {
                put(&mut values, "proxy_url", connection.network_metadata.get("proxyUrl").and_then(serde_json::Value::as_str).map(str::to_owned));
            }
            if let Some(origin) = &connection.origin {
                match origin {
                    ConnectionOrigin::Preset { site, .. } => {
                        put(&mut values, "connection_mode", Some("preset".into()));
                        put(&mut values, "provider_site", site.clone());
                    }
                    ConnectionOrigin::Custom { site, .. } => {
                        put(&mut values, "connection_mode", Some("custom".into()));
                        put(&mut values, "provider_site", site.clone());
                    }
                }
            }
            if let Some(option) = &connection.http_options {
                options(&mut values, option);
            }
            match connection.auth_mode.as_str() {
                "google_oauth" => put(&mut values, "google_auth_method", Some("oauth".into())),
                "google_service_account" => put(
                    &mut values,
                    "google_auth_method",
                    Some("service_account".into()),
                ),
                _ => {}
            }
            if let Some(defaults) = model.request_defaults.get(&query.purpose) {
                common_defaults(&mut values, defaults);
                fallback = defaults.fallback.clone();
            }
            if let Some(parameter) = model.parameters.get("temperature") {
                match parameter {
                    GenerationParameter::Inherit {} => {
                        put(&mut values, "temperature_mode", Some("inherit".into()))
                    }
                    GenerationParameter::Omit {} => {
                        put(&mut values, "temperature_mode", Some("omit".into()))
                    }
                    GenerationParameter::Set { value } => {
                        put(&mut values, "temperature_mode", Some("set".into()));
                        if let Some(n) = value.as_f64() {
                            values.insert("temperature".into(), CatalogScalar::Number(n));
                        } else {
                            reasons.push("保存的temperature值无法用当前数值控件无损表示".into());
                        }
                    }
                }
            }
            if let Some(GenerationParameter::Set { value }) = model.parameters.get("max_tokens") {
                if let Some(n) = value.as_f64() {
                    values.insert("max_tokens".into(), CatalogScalar::Number(n));
                } else {
                    reasons.push("保存的max_tokens值无法用当前数值控件无损表示".into());
                }
            }
            if let Some(id) = model.default_templates.get(&query.purpose) {
                let template = templates
                    .iter()
                    .find(|t| &t.template_id == id)
                    .ok_or_else(|| error("service_editor_template_missing", "服务模板不存在"))?;
                if let Some(body) = template.body.get("user").and_then(|v| v.as_str()) {
                    put(&mut values, "template", Some(body.into()));
                } else {
                    reasons.push("保存的模板没有可无损编辑的user正文，保留原模板引用".into());
                }
            }
            if let (Some(pool_id), Some(pool_revision)) = (
                &connection.credential_pool_id,
                connection.credential_pool_revision,
            ) {
                authentication = ServiceAuthenticationSelection::ExistingPool {
                    pool_id: pool_id.clone(),
                    pool_revision,
                };
                if let Some(fact) = &facts.credential {
                    if &fact.pool_id != pool_id || fact.pool_revision != pool_revision {
                        return Err(error(
                            "service_editor_fact_mismatch",
                            "凭据事实与来源引用不一致",
                        ));
                    }
                    values.insert(
                        "key_pool".into(),
                        CatalogScalar::Boolean(fact.multiple_keys),
                    );
                    for id in ["app_id", "app_key"] {
                        put(&mut values, id, fact.public_identifiers.get(id).cloned());
                    }
                } else {
                    reasons.push(
                        "原凭据组引用已保留；尚未加载凭据组模式及公开标识，不能推测后覆盖".into(),
                    );
                }
            }
            if let Some(authorization) = &connection.google_authorization {
                authentication = ServiceAuthenticationSelection::GoogleOwnedGrant {
                    authorization: authorization.clone(),
                };
                if let Some(fact) = &facts.google_client {
                    if &fact.authorization != authorization {
                        return Err(error(
                            "service_editor_fact_mismatch",
                            "Google客户端事实与原授权引用不一致",
                        ));
                    }
                    put(&mut values, "oauth_client_mode", Some(fact.mode.clone()));
                    put(&mut values, "oauth_client_id", fact.client_id.clone());
                } else {
                    reasons.push(
                        "原Google授权引用已保留；尚未加载客户端公开身份，不能推测客户端模式".into(),
                    );
                }
            }
            affected_shared_model_ids = current_graph
                .models
                .iter()
                .filter(|m| {
                    m.connection_id == connection.connection_id && m.model_id != model.model_id
                })
                .map(|m| m.model_id.clone())
                .collect();
        }
        Closure::Offline(config) => {
            if facts.credential.is_some() || facts.google_client.is_some() {
                return Err(error(
                    "service_editor_fact_mismatch",
                    "离线服务不能接收HTTP认证事实",
                ));
            }
            if let Some(fact) = &facts.offline
                && config.installation.as_ref() != Some(&fact.installation) {
                    return Err(error(
                        "service_editor_fact_mismatch",
                        "本机安装事实不属于来源引用",
                    ));
                }
            put(&mut values, "name", Some(config.display_name.clone()));
            put(
                &mut values,
                "source_language",
                Some(config.pair.source.clone()),
            );
            put(
                &mut values,
                "target_language",
                Some(config.pair.target.clone()),
            );
            put(
                &mut values,
                "route_policy",
                Some(
                    match config.route_policy {
                        OfflineRoutePolicy::DirectOnly {} => "direct_only",
                        OfflineRoutePolicy::ExplicitPivot { .. } => "pivot",
                    }
                    .into(),
                ),
            );
            common_defaults(&mut values, &config.request_defaults);
            fallback = config.request_defaults.fallback.clone();
            if let Some(installation) = &config.installation {
                if let Some(fact) = &facts.offline {
                    if &fact.installation != installation
                        || fact.engine != config.engine
                        || fact.pair != config.pair
                        || fact.resolution != config.resolution
                    {
                        return Err(error(
                            "service_editor_fact_mismatch",
                            "本机资源事实与保存的精确引用不一致",
                        ));
                    }
                    for id in ["installation_path", "language_package", "runtime", "bottle"] {
                        if let Some(value) = fact.prepared_values.get(id) {
                            values.insert(id.into(), value.clone());
                        }
                    }
                } else {
                    reasons.push(
                        "原安装与资源引用已保留；尚未加载本机路径和运行环境事实，不能推测后覆盖"
                            .into(),
                    );
                }
            }
        }
    }
    if let Some(target) = &target {
        // Ask Rust for input permission first, then project candidate values. Do not
        // reproduce visibility/default/authentication rules in this reverse mapping.
        let initial = directory.form(&ServiceFormRequest {
            target: target.clone(),
            purpose: query.purpose,
            values: BTreeMap::new(),
        })?;
        values.retain(|id, _| {
            id != "fallback"
                && initial
                    .fields
                    .iter()
                    .any(|row| &row.descriptor.id == id && row.input_allowed)
        });
        let projected = directory.form(&ServiceFormRequest {
            target: target.clone(),
            purpose: query.purpose,
            values: values.clone(),
        })?;
        if projected.target != *target || projected.purpose != query.purpose {
            return Err(error("directory_projection_mismatch", "目录投影身份不一致"));
        }
        for row in &projected.fields {
            if values.contains_key(&row.descriptor.id) && row.visibility == CapabilityState::Unknown
            {
                reasons.push(format!(
                    "字段 {} 需要可信运行条件，当前无法核验并无损编辑",
                    row.descriptor.id
                ));
            }
            if row.issues.iter().any(|i| {
                matches!(
                    i,
                    ServiceFormIssue::InvalidChoice
                        | ServiceFormIssue::InvalidNumber
                        | ServiceFormIssue::TypeMismatch
                        | ServiceFormIssue::InvalidUrl
                        | ServiceFormIssue::DependencyCycle
                )
            }) {
                reasons.push(format!(
                    "字段 {} 与当前精确表单约束不一致，保留原值只读",
                    row.descriptor.id
                ));
            }
        }
        values.retain(|id, _| {
            projected.fields.iter().any(|row| {
                &row.descriptor.id == id
                    && row.input_allowed
                    && row.visibility == CapabilityState::Supported
            })
        });
        // Retained opaque references are checked against the authoritative capture
        // kind, rather than interpreting provider authentication a second time.
        match (&authentication, &projected.credential_capture) {
            (ServiceAuthenticationSelection::None {}, _) => {}
            (
                ServiceAuthenticationSelection::ExistingPool { .. },
                ServiceCredentialCapture::ApiKey
                | ServiceCredentialCapture::BaiduAppSecret
                | ServiceCredentialCapture::YoudaoAppSecret
                | ServiceCredentialCapture::GoogleServiceAccountFile,
            ) => {}
            (
                ServiceAuthenticationSelection::GoogleOwnedGrant { .. },
                ServiceCredentialCapture::GoogleOwnedGrant,
            ) => {}
            _ => reasons.push("原认证引用与当前目录的材料接入方式无法精确核对".into()),
        }
    } else {
        values.clear();
    }
    affected_shared_model_ids.sort();
    affected_shared_model_ids.dedup();
    let readonly_reason = (!reasons.is_empty()).then(|| reasons.join("；"));
    Ok(ServiceDraftEditorProjection {
        profile_generation: current.profile_generation,
        draft_head_revision: current.draft_head_revision,
        source: query.source.clone(),
        editing: readonly_reason.is_none().then_some(current_source),
        target,
        purpose: query.purpose,
        values,
        authentication,
        readonly_reason,
        affected_shared_model_ids,
        fallback,
    })
}
