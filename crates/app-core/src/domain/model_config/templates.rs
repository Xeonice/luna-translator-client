//! Template ownership uses stable configuration identities, never display names/model-name heuristics.
use crate::{
    contracts::*,
    domain::{canonical_hash, content_hash, require_id},
};
use serde_json::Value;
use std::collections::BTreeSet;
const PURPOSES: [&str; 3] = ["translation", "grammar", "context_word"];
/// Convention v1 in templates.compatibility_json; an absent owner denotes a shared template.
pub fn owner(template: &Template) -> AppResult<Option<&str>> {
    template
        .compatibility
        .get("modelConfigurationId")
        .map(|v| {
            v.as_str().filter(|s| !s.is_empty()).ok_or_else(|| {
                AppError::new(
                    "invalid_template_owner",
                    "configuration",
                    "模板专属模型身份必须是稳定配置ID",
                )
            })
        })
        .transpose()
}
fn check_list(template: &Template, key: &str, actual: &str) -> AppResult<()> {
    if let Some(value) = template.compatibility.get(key) {
        let list = value.as_array().ok_or_else(|| {
            AppError::new(
                "invalid_template_compatibility",
                "configuration",
                "模板兼容声明须为文字列表",
            )
        })?;
        if list.iter().any(|v| v.as_str().is_none()) {
            return Err(AppError::new(
                "invalid_template_compatibility",
                "configuration",
                "模板兼容声明须为文字列表",
            ));
        }
        if !list.iter().any(|v| v.as_str() == Some(actual)) {
            return Err(AppError::new(
                "template_incompatible",
                "configuration",
                "模板与所选模型、协议或语言方向不匹配",
            ));
        }
    }
    Ok(())
}
fn validate_reference<'a>(
    c: &'a Configuration,
    model: &Model,
    purpose: &str,
    id: &str,
) -> AppResult<&'a Template> {
    let t = c
        .templates
        .iter()
        .find(|t| t.template_id == id)
        .ok_or_else(|| AppError::new("missing_template", "configuration", "引用的模板不存在"))?;
    if t.purpose != purpose {
        return Err(AppError::new(
            "template_purpose",
            "configuration",
            "模板用途不匹配",
        ));
    }
    if owner(t)?.is_some_and(|id| id != model.model_id) {
        return Err(AppError::new(
            "template_model_scope",
            "configuration",
            "不能使用另一模型配置的专属模板",
        ));
    }
    if let Some(con) = c
        .connections
        .iter()
        .find(|con| con.connection_id == model.connection_id)
    {
        if con.protocol == "azure_translator" {
            return Err(AppError::new(
                "unsupported_template_protocol",
                "configuration",
                "传统翻译协议不支持提示词模板，请恢复内置处理",
            ));
        }
        check_list(t, "protocols", &con.protocol)?;
    }
    if let Some(actual) = &model.actual_model_id {
        check_list(t, "actualModelIds", actual)?;
    }
    Ok(t)
}
pub fn validate_model_templates(c: &Configuration) -> AppResult<()> {
    let mut names = BTreeSet::new();
    for t in &c.templates {
        let owning = owner(t)?;
        if owning.is_some_and(|id| !c.models.iter().any(|m| m.model_id == id)) {
            return Err(AppError::new(
                "invalid_template_owner",
                "configuration",
                "模板所属模型配置不存在",
            ));
        }
        if !PURPOSES.contains(&t.purpose.as_str()) || t.display_name.trim().is_empty() {
            return Err(AppError::new(
                "invalid_template",
                "configuration",
                "模板用途或名称无效",
            ));
        }
        if !names.insert((
            owning,
            t.purpose.as_str(),
            t.display_name.trim().to_lowercase(),
        )) {
            return Err(AppError::new(
                "duplicate_template_name",
                "configuration",
                "同一模型作用域和用途的模板名称不能重复",
            ));
        }
    }
    for model in &c.models {
        for (purpose, id) in &model.default_templates {
            if !PURPOSES.contains(&purpose.as_str()) {
                return Err(AppError::new(
                    "template_purpose",
                    "configuration",
                    "模型默认模板用途无效",
                ));
            }
            validate_reference(c, model, purpose, id)?;
        }
    }
    Ok(())
}
pub fn resolve_template<'a>(
    c: &'a Configuration,
    model: &Model,
    b: &PurposeBinding,
    e: &Engine,
) -> AppResult<Option<&'a Template>> {
    let mut ids = vec![e.template_id.as_deref(), b.template_id.as_deref()];
    if b.scope_key != "global"
        && let Some(global) = c
            .bindings
            .iter()
            .find(|v| v.scope_key == "global" && v.purpose == b.purpose)
    {
        let exact = global
            .engines
            .iter()
            .find(|v| v.engine_id == e.engine_id && v.model_id == e.model_id);
        let matching: Vec<_> = global
            .engines
            .iter()
            .filter(|v| v.enabled && v.model_id == e.model_id)
            .collect();
        let template_values: BTreeSet<_> =
            matching.iter().map(|v| v.template_id.as_deref()).collect();
        let inherited = exact.or_else(|| {
            (template_values.len() <= 1)
                .then(|| matching.first().copied())
                .flatten()
        });
        if exact.is_none()
            && template_values.len() > 1
            && e.template_id.is_none()
            && b.template_id.is_none()
        {
            return Err(AppError::new(
                "ambiguous_template_inheritance",
                "configuration",
                "全局引擎模板继承存在歧义，请明确引擎身份或用途模板",
            ));
        }
        ids.push(inherited.and_then(|v| v.template_id.as_deref()));
        ids.push(global.template_id.as_deref());
    }
    ids.push(model.default_templates.get(&b.purpose).map(String::as_str));
    let selected = ids
        .into_iter()
        .flatten()
        .next()
        .map(|id| validate_reference(c, model, &b.purpose, id))
        .transpose()?;
    if let Some(t) = selected {
        if let Some(source) = b.source_language.as_deref() {
            check_list(t, "sourceLanguages", source)?;
        }
        if let Some(target) = b.target_language.as_deref() {
            check_list(t, "targetLanguages", target)?;
        }
    }
    Ok(selected)
}
pub fn copy_model(
    c: &Configuration,
    source: &str,
    name: &str,
    operation: &str,
) -> AppResult<(Configuration, String)> {
    require_id(source)?;
    require_id(name)?;
    let name = name.trim();
    if name.is_empty()
        || c.models
            .iter()
            .any(|m| m.display_name.trim().to_lowercase() == name.to_lowercase())
    {
        return Err(AppError::new(
            "duplicate_model_name",
            "configuration",
            "模型副本名称为空或与现有模型重名",
        ));
    }
    let mut model = c
        .models
        .iter()
        .find(|m| m.model_id == source)
        .cloned()
        .ok_or_else(|| AppError::new("missing_model", "configuration", "复制的模型配置不存在"))?;
    let new_id = format!("model-{}", content_hash(operation));
    model.model_id = new_id.clone();
    model.display_name = name.into();
    let mut copied = c.clone();
    for template in &c.templates {
        if owner(template)? == Some(source) {
            let mut t = template.clone();
            let id = format!("template-{}", canonical_hash(&(operation, &t.template_id))?);
            for value in model.default_templates.values_mut() {
                if *value == t.template_id {
                    *value = id.clone();
                }
            }
            t.template_id = id;
            t.source_kind = "copied".into();
            t.compatibility
                .insert("modelConfigurationId".into(), Value::String(new_id.clone()));
            copied.templates.push(t);
        }
    }
    copied.models.push(model);
    copied.revision_id = format!("config-{}", content_hash(operation));
    super::validate_configuration(&copied, false)?;
    Ok((copied, new_id))
}

/// Dispatch validates actual resolved language direction; unknown draft languages are never guessed.
pub fn validate_direction(template: &Template, source: &str, target: &str) -> AppResult<()> {
    check_list(template, "sourceLanguages", source)?;
    check_list(template, "targetLanguages", target)
}
