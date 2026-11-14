pub mod wire_parameters;
use super::require_id;
use crate::contracts::*;
use std::collections::{BTreeMap, BTreeSet};
pub mod catalog;
pub mod codec;
pub mod credentials;
pub mod service_graph;
pub mod service_codec;
pub mod form;
pub mod service_draft;
pub mod service_editor;
pub mod templates;
pub mod testing;

/// Protocol capability is independent of a model's user-editable display name.
pub fn is_translation_protocol(protocol: &str) -> bool {
    matches!(
        protocol,
        "azure_translator"
            | "deepl_v2"
            | "google_translate_v2"
            | "google_translate_v3"
            | "baidu_vip"
            | "youdao_v3"
            | "tencent_tokenhub_translation"
    )
}
/// Bound translation contracts are non-streaming unless the saved model explicitly requests it.
/// The legacy None grammar preserves its historical default; gateway preflight validates overrides.
pub fn default_request_streaming(connection: &Connection) -> bool {
    connection.adapter_binding.is_none() || !is_translation_protocol(&connection.protocol)
}
pub fn validate_protocol_purpose(protocol: &str, purpose: &str) -> AppResult<()> {
    if is_translation_protocol(protocol) && purpose != "translation" {
        return Err(AppError::new(
            "unsupported_protocol_purpose",
            "configuration",
            "传统文本翻译协议仅支持翻译用途，不能用于文法或语境查词",
        ));
    }
    Ok(())
}

/// These bounds match the runtime's bounded purpose queues and provider deadline.
/// Missing keys retain defaults; explicit malformed values never become defaults.
pub fn validate_schedule(schedule: &BTreeMap<String, serde_json::Value>) -> AppResult<()> {
    for (key, min, max) in [
        ("retries", 0, 3),
        ("concurrency", 1, 8),
        ("queueCapacity", 1, 128),
        ("timeoutMs", 1000, 300000),
    ] {
        if let Some(value) = schedule.get(key)
            && value
                .as_u64()
                .is_none_or(|value| value < min || value > max)
        {
            return Err(AppError::new(
                "invalid_schedule",
                "configuration",
                format!("调度字段 {key} 必须为 {min} 至 {max} 的整数"),
            ));
        }
    }
    Ok(())
}

fn is_template_variable(variable: &str) -> bool {
    matches!(
        variable,
        "source"
            | "word"
            | "lemma"
            | "source_language"
            | "target_language"
            | "history"
            | "terms"
            | "dictionary"
    )
}

pub fn template_variables(text: &str) -> AppResult<Vec<&str>> {
    let mut variables = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        let end = remaining.find("}}").ok_or_else(|| {
            AppError::new(
                "invalid_template_variable",
                "configuration",
                "模板变量缺少结束标记",
            )
        })?;
        let variable = &remaining[..end];
        if !is_template_variable(variable) {
            return Err(AppError::new(
                "unknown_template_variable",
                "configuration",
                "模板包含未知变量",
            ));
        }
        variables.push(variable);
        remaining = &remaining[end + 2..];
    }
    Ok(variables)
}

/// Incomplete templates can be saved as drafts, but cannot be applied or sent.
pub fn validate_template(template: &Template) -> AppResult<()> {
    let mut has_source = false;
    for (role, body) in &template.body {
        if !matches!(role.as_str(), "system" | "user") {
            return Err(AppError::new(
                "unsupported_template_role",
                "configuration",
                "当前协议模板仅支持 system 与 user，尚不支持 prefill",
            ));
        }
        let text = body.as_str().ok_or_else(|| {
            AppError::new(
                "invalid_template_body",
                "configuration",
                "模板内容必须是文字",
            )
        })?;
        has_source |= template_variables(text)?.contains(&"source");
    }
    for variable in &template.variables {
        if !is_template_variable(variable) {
            return Err(AppError::new(
                "unknown_template_variable",
                "configuration",
                "模板变量目录包含未知变量",
            ));
        }
    }
    if !has_source {
        return Err(AppError::new(
            "missing_source_variable",
            "configuration",
            "模板须包含 {{source}} 原句变量",
        ));
    }
    Ok(())
}
pub fn validate_parameter(p: &Parameter) -> AppResult<()> {
    match (&p.mode, &p.value) {
        (ParameterMode::Set, Some(_)) | (ParameterMode::Inherit | ParameterMode::Omit, None) => {
            Ok(())
        }
        _ => Err(AppError::new(
            "invalid_parameter",
            "configuration",
            "参数set必须有值，inherit/omit不得带值",
        )),
    }
}
pub fn validate_configuration(c: &Configuration, applying: bool) -> AppResult<()> {
    codec::validate_configuration_codec(c)?;
    let mut ids = BTreeSet::new();
    for con in &c.connections {
        require_id(&con.connection_id)?;
        if !ids.insert(&con.connection_id) {
            return Err(AppError::new(
                "duplicate_connection",
                "configuration",
                "连接身份重复",
            ));
        }
        if let Some(endpoint) = &con.endpoint {
            let lower = endpoint.to_ascii_lowercase();
            let authority = endpoint
                .split("://")
                .nth(1)
                .unwrap_or("")
                .split('/')
                .next()
                .unwrap_or("");
            if endpoint.contains('\0')
                || endpoint.contains('?')
                || endpoint.contains('#')
                || authority.contains('@')
                || lower.contains("api_key=")
                || lower.contains("access_token=")
                || lower.contains("token=")
            {
                return Err(AppError::new(
                    "secret_in_endpoint",
                    "configuration",
                    "服务地址不得携带查询参数、片段或内嵌凭据，请分开配置",
                ));
            }
            if applying && !lower.starts_with("https://") && !lower.starts_with("http://") {
                return Err(AppError::new(
                    "invalid_endpoint",
                    "configuration",
                    "服务地址需要http或https协议",
                ));
            }
        }
        if let Some(proxy) = con.network_metadata.get("proxyUrl") {
            let proxy = proxy.as_str().ok_or_else(|| {
                AppError::new("invalid_proxy", "configuration", "代理地址必须是文字")
            })?;
            let authority = proxy
                .split("://")
                .nth(1)
                .unwrap_or("")
                .split('/')
                .next()
                .unwrap_or("");
            if proxy.contains(['?', '#', '\0']) || authority.contains('@') {
                return Err(AppError::new(
                    "secret_in_proxy",
                    "configuration",
                    "代理地址不得携带查询参数、片段或内嵌凭据",
                ));
            }
        }
        if catalog::credential_kind_for_auth_mode(&con.auth_mode).is_err()
            || !["local", "remote", "cloud"].contains(&con.deployment.as_str())
            || !["inherit", "override", "disabled"].contains(&con.proxy_mode.as_str())
        {
            return Err(AppError::new(
                "invalid_connection",
                "configuration",
                "连接选项无效",
            ));
        }
        if con.credential_pool_id.is_some() != con.credential_pool_revision.is_some() {
            return Err(AppError::new(
                "invalid_credential_pool",
                "configuration",
                "凭据池身份与版本必须同时指定",
            ));
        }
    }
    let mut model_ids = BTreeSet::new();
    for model in &c.models {
        require_id(&model.model_id)?;
        if !model_ids.insert(&model.model_id)
            || !c
                .connections
                .iter()
                .any(|x| x.connection_id == model.connection_id)
        {
            return Err(AppError::new(
                "invalid_model_reference",
                "configuration",
                "模型身份重复或连接不存在",
            ));
        }
        for p in model.parameters.values() {
            validate_parameter(p)?;
        }
    }
    let mut keys = BTreeSet::new();
    let mut binding_ids = BTreeSet::new();
    for b in &c.bindings {
        require_id(&b.binding_id)?;
        if applying {
            validate_schedule(&b.schedule)?;
        }
        if !binding_ids.insert(&b.binding_id) || !keys.insert((&b.scope_key, &b.purpose)) {
            return Err(AppError::new(
                "duplicate_binding",
                "configuration",
                "同范围用途不得重复",
            ));
        }
        if !["translation", "grammar", "context_word"].contains(&b.purpose.as_str())
            || !["inherit", "override"].contains(&b.binding_mode.as_str())
            || (b.scope_key == "global" && (b.game_id.is_some() || b.binding_mode != "override"))
            || (b.scope_key != "global"
                && b.game_id
                    .as_ref()
                    .is_none_or(|g| b.scope_key != format!("game:{g}")))
        {
            return Err(AppError::new(
                "invalid_binding",
                "configuration",
                "用途范围或继承定义无效",
            ));
        }
        if b.purpose != "translation" && b.engines.len() > 1 {
            return Err(AppError::new(
                "single_study_engine",
                "configuration",
                "学习用途至多分配一个引擎",
            ));
        }
        let mut engines = BTreeSet::new();
        for e in &b.engines {
            if !engines.insert(&e.engine_id) || !c.models.iter().any(|m| m.model_id == e.model_id) {
                return Err(AppError::new(
                    "invalid_engine",
                    "configuration",
                    "引擎身份重复或模型不存在",
                ));
            }
            for p in e.parameters.values() {
                validate_parameter(p)?;
            }
        }
        for p in b.parameters.values() {
            validate_parameter(p)?;
        }
        for e in &b.engines {
            let model = c
                .models
                .iter()
                .find(|m| m.model_id == e.model_id)
                .expect("validated model");
            templates::resolve_template(c, model, b, e)?;
        }
        if applying && b.binding_mode == "override" && b.enabled {
            if !b.engines.iter().any(|e| e.enabled) {
                return Err(AppError::new(
                    "missing_engine",
                    "configuration",
                    "启用用途需要至少一个引擎",
                ));
            }
            for e in b.engines.iter().filter(|e| e.enabled) {
                let m = c
                    .models
                    .iter()
                    .find(|m| m.model_id == e.model_id)
                    .expect("validated model");
                let con = c
                    .connections
                    .iter()
                    .find(|x| x.connection_id == m.connection_id)
                    .expect("validated connection");
                validate_protocol_purpose(&con.protocol, &b.purpose)?;
                if let Some(template_id) = e.template_id.as_ref().or(b.template_id.as_ref()) {
                    let template = c
                        .templates
                        .iter()
                        .find(|template| &template.template_id == template_id)
                        .ok_or_else(|| {
                            AppError::new(
                                "missing_template",
                                "configuration",
                                "用途引用的模板不存在",
                            )
                        })?;
                    if template.purpose != b.purpose {
                        return Err(AppError::new(
                            "template_purpose",
                            "configuration",
                            "模板用途不匹配",
                        ));
                    }
                }
                if !con.availability_intent
                    || con.protocol == "unsupported"
                    || con.endpoint.as_deref().is_none_or(str::is_empty)
                    || (!is_translation_protocol(&con.protocol)
                        && m.actual_model_id.as_deref().is_none_or(str::is_empty))
                    || (con.auth_mode != "none" && con.credential_pool_id.is_none())
                    || (c.codec_version == Some(2) && con.adapter_binding.is_none())
                {
                    return Err(AppError::new(
                        "configuration_incomplete",
                        "configuration",
                        "启用用途的连接、模型或凭据未配置完整",
                    ));
                }
            }
        }
    }
    let mut template_ids = BTreeSet::new();
    for template in &c.templates {
        require_id(&template.template_id)?;
        if !template_ids.insert(&template.template_id) {
            return Err(AppError::new(
                "duplicate_template",
                "configuration",
                "模板身份重复",
            ));
        }
        if applying {
            validate_template(template)?;
        }
    }
    templates::validate_model_templates(c)?;
    Ok(())
}
pub fn effective_binding<'a>(
    c: &'a Configuration,
    game: Option<&str>,
    purpose: &str,
) -> Option<&'a PurposeBinding> {
    if let Some(game) = game
        && let Some(b) = c
            .bindings
            .iter()
            .find(|b| b.scope_key == format!("game:{game}") && b.purpose == purpose)
        && b.binding_mode == "override"
    {
        return Some(b);
    }

    c.bindings
        .iter()
        .find(|b| b.scope_key == "global" && b.purpose == purpose)
}
pub fn resolve_parameters(
    layers: &[&BTreeMap<String, Parameter>],
) -> BTreeMap<String, serde_json::Value> {
    let mut result = BTreeMap::new();
    for layer in layers {
        for (k, p) in *layer {
            match p.mode {
                ParameterMode::Inherit => {}
                ParameterMode::Set => {
                    if let Some(v) = &p.value {
                        result.insert(k.clone(), v.clone());
                    }
                }
                ParameterMode::Omit => {
                    result.remove(k);
                }
            }
        }
    }
    result
}
