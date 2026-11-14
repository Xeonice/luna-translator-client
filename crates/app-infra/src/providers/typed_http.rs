//! Projection of genuine codec3 HTTP data into the existing HTTP transport.
//! This never constructs a legacy configuration, binding or model. Offline
//! targets cannot enter this adapter and options remain explicitly typed.
use super::{
    adapters::translation::{
        GoogleV3Route, ReferenceKind, TokenHubTranslationOptions, TranslationReference,
        TranslationWireOptions,
    },
    failure,
};
use app_core::{
    application::request_plan::RequestPlan,
    contracts::{AppResult, Connection},
    domain::{
        model_config::service_graph::{AzureResourceType, HttpServiceOptionsV1},
        service_execution::FrozenHttpServicePlan,
    },
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

pub(super) struct HttpTransport {
    pub connection: Connection,
    pub options: TranslationWireOptions,
    pub headers: HeaderMap,
}
fn header(headers: &mut HeaderMap, name: &'static str, value: Option<&str>) -> AppResult<()> {
    if let Some(value) = value {
        if value.is_empty() || value.len() > 512 || value != value.trim() {
            return Err(failure(
                "invalid_http_options",
                "接口选项为空、过长或含首尾空白",
            ));
        }
        let value = HeaderValue::from_str(value)
            .map_err(|_| failure("invalid_http_options", "接口选项包含无效HTTP字符"))?;
        headers.insert(HeaderName::from_static(name), value);
    }
    Ok(())
}
pub(super) fn project(plan: &FrozenHttpServicePlan<RequestPlan>) -> AppResult<HttpTransport> {
    let c = &plan.connection;
    if plan.http_options.protocol_id() != c.protocol
        || c.http_options.as_ref() != Some(&plan.http_options)
        || c.origin.as_ref() != Some(&plan.origin)
        || plan.model.connection_id != c.connection_id
        || plan.model.actual_model_id != plan.request.model
        || plan.request.protocol != c.protocol
        || plan.request.adapter_binding != c.adapter_binding
        || c.adapter_binding.is_none()
        || c.endpoint.as_deref() != Some(plan.request.endpoint.as_str())
    {
        return Err(failure(
            "service_plan_identity_mismatch",
            "冻结HTTP服务的连接、模型或选项身份不一致",
        ));
    }
    let mut transport = project_connection(c, &plan.http_options, &plan.origin)?;
    let material = &plan.reference_material;
    if c.protocol == "tencent_tokenhub_translation" {
        let references = material
            .glossary
            .iter()
            .map(|value| {
                let text = value
                    .get("source")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| failure("invalid_reference", "冻结术语缺少原文"))?;
                let translation = value
                    .get("target")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| failure("invalid_reference", "冻结术语缺少译文"))?;
                Ok(TranslationReference {
                    kind: ReferenceKind::Term,
                    text: text.into(),
                    translation: translation.into(),
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        transport.options.tokenhub = Some(TokenHubTranslationOptions {
            context: if material.context.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&material.context)
                        .map_err(|_| failure("invalid_reference", "上下文不能编码"))?,
                )
            },
            references,
            // Remote glossary resource IDs are a separate contract; local term
            // IDs must never be mistaken for provider-owned glossary resources.
            glossary_ids: vec![],
        });
    } else if app_core::domain::model_config::is_translation_protocol(&c.protocol)
        && (!material.context.is_empty() || !material.glossary.is_empty())
    {
        return Err(failure(
            "reference_not_supported",
            "此翻译接口不能执行当前明确选择的上下文或术语材料",
        ));
    }
    Ok(transport)
}
/// A genuine typed connection projection; no fabricated model/request plan.
pub(super) fn project_connection(
    c: &app_core::domain::model_config::service_graph::ConnectionV3,
    http_options: &HttpServiceOptionsV1,
    origin: &app_core::domain::model_config::service_graph::ConnectionOrigin,
) -> AppResult<HttpTransport> {
    if http_options.protocol_id() != c.protocol
        || c.http_options.as_ref() != Some(http_options)
        || c.origin.as_ref() != Some(origin)
        || c.adapter_binding.is_none()
    {
        return Err(failure(
            "discovery_connection_identity_mismatch",
            "保存的连接、接口选项或来源不一致",
        ));
    }
    let mut headers = HeaderMap::new();
    let mut options = TranslationWireOptions::default();
    match http_options {
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id,
            project_id,
        }
        | HttpServiceOptionsV1::OpenaiResponses {
            organization_id,
            project_id,
        } => {
            header(
                &mut headers,
                "openai-organization",
                organization_id.as_deref(),
            )?;
            header(&mut headers, "openai-project", project_id.as_deref())?;
        }
        HttpServiceOptionsV1::Anthropic { workspace_id } => {
            header(
                &mut headers,
                "anthropic-workspace-id",
                workspace_id.as_deref(),
            )?;
        }
        HttpServiceOptionsV1::AzureTranslator {
            resource_type,
            region,
        } => {
            let kind = resource_type
                .as_ref()
                .ok_or_else(|| failure("missing_azure_resource_type", "需要明确Azure资源类型"))?;
            if !matches!(kind, AzureResourceType::Global) && region.is_none() {
                return Err(failure("missing_azure_region", "所选Azure资源类型需要区域"));
            }
            header(
                &mut headers,
                "ocp-apim-subscription-region",
                region.as_deref(),
            )?;
        }
        HttpServiceOptionsV1::DeeplV2 { plan: None } => {
            return Err(failure("missing_deepl_plan", "需要明确DeepL Free或Pro"));
        }
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id,
            location,
        } => {
            options.google_v3 = Some(GoogleV3Route {
                project_id: project_id.clone(),
                location: location.clone(),
            });
        }
        _ => {}
    }
    let mut network_metadata = c.network_metadata.clone();
    // Old materialized connections may retain their original region metadata.
    // The new request contract has a single authority: the typed option above.
    if matches!(http_options, HttpServiceOptionsV1::AzureTranslator { .. }) {
        network_metadata.remove("region");
    }
    Ok(HttpTransport {
        connection: Connection {
            connection_id: c.connection_id.clone(),
            display_name: c.display_name.clone(),
            protocol: c.protocol.clone(),
            deployment: c.deployment.clone(),
            auth_mode: c.auth_mode.clone(),
            endpoint: c.endpoint.clone(),
            credential_pool_id: c.credential_pool_id.clone(),
            credential_pool_revision: c.credential_pool_revision.map(std::num::NonZeroU32::get),
            proxy_mode: c.proxy_mode.clone(),
            network_metadata,
            availability_intent: c.availability_intent,
            adapter_binding: c.adapter_binding.clone(),
            preset_source: c.preset_source.clone(),
        },
        options,
        headers,
    })
}

#[cfg(test)]
#[path = "typed_http_tests.rs"]
mod tests;
