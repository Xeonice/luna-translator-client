//! Authenticated wire dispatch. Configuration and Vault authority stay in the gateway.
use super::{
    llm,
    translation::{self, TranslationWireOptions},
};
use crate::providers::{self, ModelOutput, failure};
use app_core::{
    application::request_plan::RequestPlan,
    contracts::{AppResult, CatalogPurpose, Connection},
    domain::model_config::credentials::ProviderAuthLease,
};

pub fn installed(protocol: &str) -> bool {
    matches!(
        protocol,
        "openai_compatible" | "anthropic" | "azure_translator"
    ) || llm::installed(protocol)
        || translation::installed(protocol)
}

/// Called before a credential is selected or read. It never inspects secret bytes.
pub fn preflight(
    plan: &RequestPlan,
    connection: &Connection,
    purpose: CatalogPurpose,
    options: &TranslationWireOptions,
) -> AppResult<()> {
    if !installed(&plan.protocol)
        || plan.protocol != connection.protocol
        || connection.endpoint.as_deref() != Some(plan.endpoint.as_str())
    {
        return Err(failure("protocol_mismatch", "请求与已选连接身份不一致"));
    }
    providers::plan_generation_parameters(plan)?;
    if translation::installed(&plan.protocol) {
        return translation::preflight(plan, connection, purpose, options);
    }
    if options.google_v3.is_some() || options.tokenhub.is_some() {
        return Err(failure(
            "unsupported_route_options",
            "当前协议不接受其他服务的路由选项",
        ));
    }
    if plan.protocol == "azure_translator" {
        if purpose != CatalogPurpose::Translation || plan.structured {
            return Err(failure("unsupported_purpose", "传统翻译仅支持翻译用途"));
        }
    } else if plan.model.as_deref().is_none_or(|id| id.trim().is_empty()) {
        return Err(failure("missing_model", "请填写实际模型ID"));
    }
    providers::endpoint(&plan.endpoint, "")?;
    if !matches!(connection.auth_mode.as_str(), "none" | "api_key") {
        return Err(failure(
            "credential_kind_mismatch",
            "模型协议不接受此认证材料类型",
        ));
    }
    if connection.auth_mode == "none"
        && !matches!(
            plan.protocol.as_str(),
            "openai_compatible" | "ollama_native" | "lmstudio_native_v1"
        )
    {
        return Err(failure(
            "unsupported_authentication",
            "所选协议需要明确凭据",
        ));
    }
    if llm::installed(&plan.protocol) {
        llm::preflight(plan, connection)?;
    }
    Ok(())
}

pub async fn execute(
    plan: &RequestPlan,
    connection: &Connection,
    purpose: CatalogPurpose,
    auth: &ProviderAuthLease<'_>,
    options: &TranslationWireOptions,
    on_delta: impl Fn(&str),
) -> AppResult<ModelOutput> {
    execute_with_headers(
        plan,
        connection,
        purpose,
        auth,
        options,
        &reqwest::header::HeaderMap::new(),
        on_delta,
    )
    .await
}
pub(crate) async fn execute_with_headers(
    plan: &RequestPlan,
    connection: &Connection,
    purpose: CatalogPurpose,
    auth: &ProviderAuthLease<'_>,
    options: &TranslationWireOptions,
    headers: &reqwest::header::HeaderMap,
    on_delta: impl Fn(&str),
) -> AppResult<ModelOutput> {
    preflight(plan, connection, purpose, options)?;
    if translation::installed(&plan.protocol) {
        return translation::execute_with_headers(
            plan, connection, purpose, auth, options, headers, on_delta,
        )
        .await;
    }
    let key = match (connection.auth_mode.as_str(), auth) {
        ("none", ProviderAuthLease::None) => None,
        ("api_key", ProviderAuthLease::ApiKey { source, key }) => {
            source.validate()?;
            if key.expose().bytes().any(|byte| byte.is_ascii_control()) {
                return Err(failure("invalid_credential", "认证材料包含无效控制字符"));
            }
            Some(key.expose())
        }
        _ => {
            return Err(failure(
                "credential_kind_mismatch",
                "认证租约类型与连接不一致",
            ));
        }
    };
    providers::execute_with_headers(plan, connection, key, headers, on_delta).await
}
