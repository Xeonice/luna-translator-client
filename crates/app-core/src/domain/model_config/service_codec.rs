//! Explicit codec3 candidates; production Store dispatch remains codecs1/2.
use super::{catalog, service_graph::*};
use crate::{
    contracts::{AppError, AppResult, CatalogPurpose},
    domain::require_id,
};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use std::{collections::BTreeSet, fmt};

pub const MAX_SERVICE_GRAPH_BYTES: usize = 16 * 1024 * 1024;
fn error(code: &str) -> AppError {
    AppError::new(code, "service_graph_codec", "服务配置结构、身份或关系无效")
}
fn unique<T>(items: &[T], id: impl Fn(&T) -> &str) -> AppResult<()> {
    let mut ids = BTreeSet::new();
    for item in items {
        let value = id(item);
        require_id(value)?;
        if !ids.insert(value) {
            return Err(error("duplicate_service_identity"));
        }
    }
    Ok(())
}
fn optional_id(value: Option<&str>) -> AppResult<()> {
    value.map(require_id).transpose().map(|_| ())
}
fn valid_scope(scope: &str, game_id: Option<&str>) -> AppResult<bool> {
    match game_id {
        None => Ok(scope == "global"),
        Some(id) => {
            require_id(id)?;
            Ok(scope == format!("game:{id}"))
        }
    }
}
fn optional_text(value: Option<&str>) -> AppResult<()> {
    if value.is_some_and(|text| text.contains('\0')) {
        return Err(error("invalid_service_text"));
    }
    Ok(())
}
fn source(source: &SavedServiceRef) -> AppResult<()> {
    match source {
        SavedServiceRef::Http { source } => {
            require_id(&source.revision_id)?;
            require_id(&source.model_id)
        }
        SavedServiceRef::Offline { source } => {
            require_id(&source.revision_id)?;
            require_id(&source.config_id)
        }
    }
}
fn context(value: &ServiceContextConsent) -> AppResult<()> {
    if value.max_sentences > 50 {
        Err(error("invalid_service_context"))
    } else {
        Ok(())
    }
}
fn execution(value: &ServiceExecutionDefaults) -> AppResult<()> {
    if !(1000..=300000).contains(&value.timeout_ms)
        || !(1..=8).contains(&value.concurrency)
        || value.retries > 3
    {
        Err(error("invalid_service_execution"))
    } else {
        Ok(())
    }
}
fn overrides(value: &PolicyOverrides) -> AppResult<()> {
    if let PolicyOverride::Set { value } = &value.context {
        context(value)?;
    }
    let valid = ServiceExecutionDefaults {
        timeout_ms: match value.execution.timeout_ms {
            PolicyOverride::Set { value } => value,
            _ => 60000,
        },
        concurrency: match value.execution.concurrency {
            PolicyOverride::Set { value } => value,
            _ => 1,
        },
        retries: match value.execution.retries {
            PolicyOverride::Set { value } => value,
            _ => 0,
        },
    };
    execution(&valid)
}
fn defaults(
    value: &ServiceRequestDefaults,
    purpose: CatalogPurpose,
    offline: bool,
) -> AppResult<()> {
    if value.purpose != purpose
        || matches!(
            value.languages,
            ServiceLanguageDefaults::OfflineUseConfigPair {}
        ) != offline
    {
        return Err(error("service_defaults_target_mismatch"));
    }
    context(&value.context)?;
    execution(&value.execution)?;
    if let ServiceLanguageDefaults::Http { source, target } = &value.languages {
        if let SourceLanguageChoice::Explicit { language } = source {
            require_id(language)?;
        }
        if let TargetLanguageChoice::Explicit { language } = target {
            require_id(language)?;
        }
    }
    let mut seen = BTreeSet::new();
    for fallback in &value.fallback {
        source(fallback)?;
        let key = serde_json::to_string(fallback).map_err(|_| error("invalid_fallback"))?;
        if !seen.insert(key) {
            return Err(error("duplicate_fallback_target"));
        }
    }
    Ok(())
}
fn option_identifier(value: &str, path_segment: bool) -> AppResult<()> {
    // Empty means an unfinished draft, not an executable route. Runtime admission checks required fields.
    if value.len() > 512
        || value.chars().any(char::is_control)
        || (path_segment && value.contains(['/', '\\', '?', '#']))
    {
        Err(error("invalid_http_options"))
    } else {
        Ok(())
    }
}
fn http_options(value: &HttpServiceOptionsV1) -> AppResult<()> {
    match value {
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id,
            project_id,
        }
        | HttpServiceOptionsV1::OpenaiResponses {
            organization_id,
            project_id,
        } => {
            for v in [organization_id, project_id].into_iter().flatten() {
                option_identifier(v, false)?;
            }
        }
        HttpServiceOptionsV1::Anthropic { workspace_id: Some(value) }
        | HttpServiceOptionsV1::AzureTranslator { region: Some(value), .. } => {
            option_identifier(value, false)?;
        }
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id,
            location,
        } => {
            option_identifier(project_id, true)?;
            option_identifier(location, true)?;
        }
        _ => {}
    }
    Ok(())
}

/// Structural validation only: references claiming saved resources are not trusted execution leases.
pub fn validate_service_graph(value: &ConfigurationV3) -> AppResult<()> {
    if value.codec_version != 3 {
        return Err(error("unsupported_service_graph_codec"));
    }
    require_id(&value.revision_id)?;
    unique(&value.connections, |v| &v.connection_id)?;
    unique(&value.models, |v| &v.model_id)?;
    unique(&value.bindings, |v| &v.binding_id)?;
    unique(&value.templates, |v| &v.template_id)?;
    unique(&value.term_sets, |v| &v.term_set_id)?;
    unique(&value.offline_engine_configs, |v| &v.id)?;
    for connection in &value.connections {
        require_id(&connection.display_name)?;
        if connection.credential_pool_id.is_some() != connection.credential_pool_revision.is_some()
        {
            return Err(error("incomplete_pool_reference"));
        }
        optional_id(connection.credential_pool_id.as_deref())?;
        if !["cloud", "remote", "local"].contains(&connection.deployment.as_str())
            || !["inherit", "disabled", "override"].contains(&connection.proxy_mode.as_str())
        {
            return Err(error("invalid_connection_policy"));
        }
        let pool = connection.credential_pool_id.is_some();
        let grant = connection.google_authorization.is_some();
        match connection.auth_mode.as_str() {
            "none" if !pool && !grant => {}
            "google_oauth" if !pool => {}
            "api_key" | "baidu_app_secret" | "youdao_app_secret" | "google_service_account"
                if !grant => {}
            _ => return Err(error("authentication_source_mismatch")),
        }
        if let Some(grant) = &connection.google_authorization {
            require_id(&grant.grant_id)?;
            if connection.protocol != "google_translate_v3" {
                return Err(error("authentication_protocol_mismatch"));
            }
        }
        if let Some(origin) = &connection.materialization_source {
            require_id(&origin.revision_id)?;
            require_id(&origin.connection_id)?;
        }
        match (
            &connection.adapter_binding,
            &connection.http_options,
            &connection.origin,
        ) {
            (Some(binding), Some(options), Some(origin)) => {
                catalog::validate_binding(binding)?;
                http_options(options)?;
                if binding.protocol.protocol_id != connection.protocol
                    || options.protocol_id() != connection.protocol
                {
                    return Err(error("http_options_protocol_mismatch"));
                }
                match origin {
                    ConnectionOrigin::Preset { key, site } => {
                        optional_text(site.as_deref())?;
                        let ns = if key.preset_id.starts_with("builtin.") {
                            crate::contracts::CatalogNamespace::Builtin
                        } else {
                            crate::contracts::CatalogNamespace::User
                        };
                        catalog::validate_preset_key(key, ns)?;
                        if connection.preset_source.as_ref() != Some(key) {
                            return Err(error("preset_origin_mismatch"));
                        }
                    }
                    ConnectionOrigin::Custom { copied_from, site } => {
                        optional_text(site.as_deref())?;
                        if copied_from.as_ref() != connection.preset_source.as_ref() {
                            return Err(error("preset_origin_mismatch"));
                        }
                    }
                }
            }
            (binding, None, None) if connection.materialization_source.is_some() => {
                // Schema3/codec2 already permitted additional exact HTTP contracts,
                // and legacy rows may deliberately describe an unsupported service.
                // Preserve that source; repository proof and runtime admission are separate.
                catalog::validate_catalog_id(&connection.protocol)?;
                if let Some(binding) = binding {
                    catalog::validate_binding(binding)?;
                    if binding.protocol.protocol_id != connection.protocol {
                        return Err(error("adapter_protocol_mismatch"));
                    }
                }
            }
            _ => return Err(error("incomplete_service_adapter_identity")),
        }
    }
    for model in &value.models {
        require_id(&model.display_name)?;
        if !value
            .connections
            .iter()
            .any(|c| c.connection_id == model.connection_id)
        {
            return Err(error("orphan_model_connection"));
        }
        if let Some(origin) = &model.legacy_eligibility_origin {
            source(&SavedServiceRef::Http {
                source: origin.clone(),
            })?;
        }
        if let Some(purpose) = model.template_purpose {
            if model.request_defaults.len() != 1 || !model.request_defaults.contains_key(&purpose) {
                return Err(error("model_purpose_defaults_mismatch"));
            }
        } else if model.legacy_eligibility_origin.is_none() {
            return Err(error("model_purpose_missing"));
        }
        for (&purpose, default) in &model.request_defaults {
            defaults(default, purpose, false)?;
        }
        for (&purpose, id) in &model.default_templates {
            if !value
                .templates
                .iter()
                .any(|t| t.template_id == *id && t.purpose == purpose)
            {
                return Err(error("orphan_model_template"));
            }
        }
    }
    for offline in &value.offline_engine_configs {
        require_id(&offline.display_name)?;
        catalog::validate_catalog_id(&offline.engine.engine_id)?;
        require_id(&offline.pair.source)?;
        require_id(&offline.pair.target)?;
        if let Some(installation) = &offline.installation {
            require_id(&installation.installation_id)?;
        }
        match &offline.resolution {
            OfflineConfigResolution::Draft {
                observed_resource,
                proposed_route,
            } => {
                optional_id(observed_resource.as_deref())?;
                optional_id(proposed_route.as_deref())?;
            }
            OfflineConfigResolution::Resolved {
                resource_snapshot_id,
                route_id,
            } => {
                require_id(resource_snapshot_id)?;
                require_id(route_id)?;
            }
        }
        defaults(&offline.request_defaults, CatalogPurpose::Translation, true)?;
    }
    let mut scopes = BTreeSet::new();
    for binding in &value.bindings {
        if binding.semantics_version == PurposeSemanticsVersion::TypedV3
            && (!binding.context.is_empty() || !binding.schedule.is_empty())
        {
            // A typed policy has one source of intent. Legacy maps are preserved only
            // for the explicit LegacyV2 grammar; silently ignoring an opt-out is unsafe.
            return Err(error("legacy_policy_shadow_forbidden"));
        }
        if !scopes.insert((&binding.scope_key, binding.purpose)) {
            return Err(error("duplicate_service_scope"));
        }
        if !valid_scope(&binding.scope_key, binding.game_id.as_deref())?
            || !["override", "inherit"].contains(&binding.binding_mode.as_str())
        {
            return Err(error("invalid_service_scope"));
        }
        if binding.scope_key == "global" && binding.binding_mode == "inherit" {
            return Err(error("global_cannot_inherit"));
        }
        if let Some(id) = &binding.template_id
            && !value
                .templates
                .iter()
                .any(|t| t.template_id == *id && t.purpose == binding.purpose)
            {
                return Err(error("orphan_binding_template"));
            }
        unique(&binding.engines, |e| &e.engine_id)?;
        overrides(&binding.scope_overrides)?;
        for engine in &binding.engines {
            overrides(&engine.policy_overrides)?;
            match &engine.target {
                EngineTarget::Http { model_id } => {
                    let model = value
                        .models
                        .iter()
                        .find(|m| m.model_id == *model_id)
                        .ok_or_else(|| error("orphan_http_target"))?;
                    if model.template_purpose.is_some_and(|p| p != binding.purpose) {
                        return Err(error("target_purpose_mismatch"));
                    }
                    if let Some(id) = &engine.http_template_override
                        && !value
                            .templates
                            .iter()
                            .any(|t| t.template_id == *id && t.purpose == binding.purpose)
                        {
                            return Err(error("orphan_engine_template"));
                        }
                }
                EngineTarget::Offline { offline_config_id } => {
                    if !value
                        .offline_engine_configs
                        .iter()
                        .any(|v| v.id == *offline_config_id)
                    {
                        return Err(error("orphan_offline_target"));
                    }
                    if binding.purpose != CatalogPurpose::Translation
                        || engine.http_template_override.is_some()
                        || !engine.http_parameters.is_empty()
                    {
                        return Err(error("offline_http_field_forbidden"));
                    }
                }
            }
        }
    }
    for template in &value.templates {
        require_id(&template.display_name)?;
        optional_text(template.source_version.as_deref())?;
        for variable in &template.variables {
            require_id(variable)?;
        }
    }
    for set in &value.term_sets {
        require_id(&set.display_name)?;
        if !valid_scope(&set.scope_key, set.game_id.as_deref())? {
            return Err(error("invalid_service_scope"));
        }
        unique(&set.terms, |t| &t.term_id)?;
        for term in &set.terms {
            optional_text(term.source_text.as_deref())?;
            optional_text(term.target_text.as_deref())?;
            optional_id(term.source_language.as_deref())?;
            optional_id(term.target_language.as_deref())?;
        }
    }
    Ok(())
}

/// Canonicalize unordered entities only; engine/fallback/term order retains its meaning.
pub fn encode_service_graph(value: &ConfigurationV3) -> AppResult<Vec<u8>> {
    validate_service_graph(value)?;
    let mut normalized = value.clone();
    normalized
        .connections
        .sort_by(|a, b| a.connection_id.cmp(&b.connection_id));
    normalized
        .models
        .sort_by(|a, b| a.model_id.cmp(&b.model_id));
    normalized
        .bindings
        .sort_by(|a, b| a.binding_id.cmp(&b.binding_id));
    normalized
        .templates
        .sort_by(|a, b| a.template_id.cmp(&b.template_id));
    normalized
        .term_sets
        .sort_by(|a, b| a.term_set_id.cmp(&b.term_set_id));
    normalized
        .offline_engine_configs
        .sort_by(|a, b| a.id.cmp(&b.id));
    let bytes = serde_json::to_vec(&normalized).map_err(|_| error("invalid_service_graph_json"))?;
    if bytes.len() > MAX_SERVICE_GRAPH_BYTES {
        return Err(error("service_graph_too_large"));
    }
    Ok(bytes)
}

struct UniqueJson;
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded JSON without duplicate keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut seen = BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !seen.insert(key) {
                        return Err(de::Error::custom("duplicate JSON field"));
                    }
                    map.next_value::<UniqueJson>()?;
                }
                Ok(UniqueJson)
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                while seq.next_element::<UniqueJson>()?.is_some() {}
                Ok(UniqueJson)
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                if v.is_finite() {
                    Ok(UniqueJson)
                } else {
                    Err(E::custom("nonfinite"))
                }
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
        }
        deserializer.deserialize_any(V)
    }
}
/// Explicit candidate decoder; never called by production's schema1/2 dispatch.
pub fn decode_service_graph_internal(bytes: &[u8]) -> AppResult<ConfigurationV3> {
    validate_unique_service_json(bytes)?;
    let value: ConfigurationV3 =
        serde_json::from_slice(bytes).map_err(|_| error("invalid_service_graph_json"))?;
    validate_service_graph(&value)?;
    Ok(value)
}

/// Apply this before parsing any typed SQL JSON carrier into Value or a map.
/// Reassembling a graph after duplicate keys have been discarded cannot recover that evidence.
pub fn validate_unique_service_json(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() > MAX_SERVICE_GRAPH_BYTES {
        return Err(error("service_graph_too_large"));
    }
    serde_json::from_slice::<UniqueJson>(bytes).map_err(|_| error("invalid_service_graph_json"))?;
    Ok(())
}
/// Public imports cannot manufacture internal provenance or LegacyV2 semantic authority.
pub fn decode_service_graph_import(bytes: &[u8]) -> AppResult<ConfigurationV3> {
    let value = decode_service_graph_internal(bytes)?;
    if value
        .connections
        .iter()
        .any(|c| c.materialization_source.is_some())
        || value
            .models
            .iter()
            .any(|m| m.legacy_eligibility_origin.is_some())
        || value
            .bindings
            .iter()
            .any(|b| b.semantics_version == PurposeSemanticsVersion::LegacyV2)
    {
        return Err(error("internal_service_origin_not_importable"));
    }
    Ok(value)
}
