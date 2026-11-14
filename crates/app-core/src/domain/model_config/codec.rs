//! Explicit configuration codecs. Legacy bytes stay untouched; codec2 normalizes unordered entities.
use crate::{contracts::*, domain::require_id};
use std::collections::BTreeSet;

/// Preserve the original serializer's struct field order and every Vec's caller-provided order.
/// Old database rows did not persist some Vec positions; do not rehash their sorted load result.
pub fn encode_legacy_configuration(configuration: &Configuration) -> AppResult<Vec<u8>> {
    serde_json::to_vec(configuration)
        .map_err(|_| AppError::new("invalid_json", "configuration_codec", "无法编码原配置格式"))
}
fn sort_unique<T>(items: &mut [T], id: impl Fn(&T) -> &str) -> AppResult<()> {
    let mut seen = BTreeSet::new();
    for item in items.iter() {
        let identity = id(item);
        require_id(identity)?;
        if !seen.insert(identity.to_owned()) {
            return Err(AppError::new(
                "duplicate_configuration_identity",
                "configuration_codec",
                "配置规范化遇到重复实体身份",
            ));
        }
    }
    items.sort_by(|a, b| id(a).cmp(id(b)));
    Ok(())
}
fn unique_in_order<'a>(ids: impl Iterator<Item = &'a str>) -> AppResult<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        require_id(id)?;
        if !seen.insert(id) {
            return Err(AppError::new(
                "duplicate_configuration_identity",
                "configuration_codec",
                "有序配置内容含重复身份",
            ));
        }
    }
    Ok(())
}
/// Canonicalize only unordered top-level entities of the current legacy-shaped DTO subset.
/// Engines and terms retain meaningful position. This does not authorize a saved codec2 format.
pub fn normalize_service_graph_configuration(
    configuration: &Configuration,
) -> AppResult<Configuration> {
    let mut result = configuration.clone();
    sort_unique(&mut result.connections, |v| &v.connection_id)?;
    sort_unique(&mut result.models, |v| &v.model_id)?;
    sort_unique(&mut result.bindings, |v| &v.binding_id)?;
    sort_unique(&mut result.templates, |v| &v.template_id)?;
    sort_unique(&mut result.term_sets, |v| &v.term_set_id)?;
    let mut scopes = BTreeSet::new();
    for binding in &result.bindings {
        if !scopes.insert((&binding.scope_key, &binding.purpose)) {
            return Err(AppError::new(
                "duplicate_configuration_scope",
                "configuration_codec",
                "同一范围用途含重复绑定",
            ));
        }
        unique_in_order(binding.engines.iter().map(|e| e.engine_id.as_str()))?;
    }
    for set in &result.term_sets {
        unique_in_order(set.terms.iter().map(|t| t.term_id.as_str()))?;
    }
    Ok(result)
}
/// Deterministic candidate bytes for the current DTO subset only; never verify old seals with it.
pub fn encode_normalized_service_graph_configuration(
    configuration: &Configuration,
) -> AppResult<Vec<u8>> {
    encode_legacy_configuration(&normalize_service_graph_configuration(configuration)?)
}

/// None is the frozen legacy grammar. Numeric one is deliberately not an alias.
pub fn validate_configuration_codec(c: &Configuration) -> AppResult<()> {
    if c.codec_version.is_some_and(|v| v != 2) {
        return Err(AppError::new(
            "unsupported_configuration_codec",
            "configuration_codec",
            "不支持此配置编码版本",
        ));
    }
    for con in &c.connections {
        if c.codec_version.is_none() {
            if con.adapter_binding.is_some()
                || con.preset_source.is_some()
                || ![
                    "openai_compatible",
                    "anthropic",
                    "azure_translator",
                    "unsupported",
                ]
                .contains(&con.protocol.as_str())
                || !["none", "api_key"].contains(&con.auth_mode.as_str())
            {
                return Err(AppError::new(
                    "legacy_codec_extension_forbidden",
                    "configuration_codec",
                    "旧编码不能承载新协议身份",
                ));
            }
        } else {
            super::catalog::validate_catalog_id(&con.protocol)?;
            if let Some(binding) = &con.adapter_binding {
                super::catalog::validate_binding(binding)?;
                if binding.protocol.protocol_id != con.protocol {
                    return Err(AppError::new(
                        "adapter_protocol_mismatch",
                        "configuration_codec",
                        "连接协议与适配身份不一致",
                    ));
                }
            }
            if let Some(key) = &con.preset_source {
                let ns = if key.preset_id.starts_with("builtin.") {
                    CatalogNamespace::Builtin
                } else {
                    CatalogNamespace::User
                };
                super::catalog::validate_preset_key(key, ns)?;
                if con.adapter_binding.is_none() {
                    return Err(AppError::new(
                        "preset_binding_missing",
                        "configuration_codec",
                        "来源预设必须关联精确适配身份",
                    ));
                }
            }
            // Metadata remains network-only; it cannot shadow an explicit authentication identity.
            for key in con.network_metadata.keys() {
                if ![
                    "proxyUrl",
                    "timeoutMs",
                    "region",
                    "project",
                    "location",
                    "category",
                    "formality",
                    "tagHandling",
                    "splitSentences",
                    "preserveFormatting",
                    "apiVersion",
                ]
                .contains(&key.as_str())
                {
                    return Err(AppError::new(
                        "protected_network_metadata",
                        "configuration_codec",
                        "新配置网络字段不在明确白名单中",
                    ));
                }
            }
        }
    }
    Ok(())
}
/// Actual bytes used by save/reopen, separate from the historic generic canonical_hash.
pub fn encode_configuration(c: &Configuration) -> AppResult<Vec<u8>> {
    validate_configuration_codec(c)?;
    match c.codec_version {
        None => encode_legacy_configuration(c),
        Some(2) => encode_normalized_service_graph_configuration(c),
        _ => unreachable!("validated codec"),
    }
}
