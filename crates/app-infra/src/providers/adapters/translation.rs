//! Traditional translation wire contracts. No configuration, Vault access or
//! OAuth token exchange occurs here; the application gateway supplies a lease.
use super::{
    super::{
        MAX_RESPONSE_BYTES, ModelOutput, bounded_json, checked, endpoint, failure, network_error,
    },
    stream::{self, Fragment},
};
use app_core::{
    application::request_plan::RequestPlan,
    contracts::{AppResult, CatalogPurpose, Connection},
    domain::model_config::credentials::{ProviderAuthLease, SignedAppLease},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// Non-secret service route data. It is not inferred from networkMetadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleV3Route {
    pub project_id: String,
    pub location: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Sentence,
    Term,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationReference {
    pub kind: ReferenceKind,
    pub text: String,
    pub translation: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenHubTranslationOptions {
    pub context: Option<String>,
    pub references: Vec<TranslationReference>,
    pub glossary_ids: Vec<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationWireOptions {
    pub google_v3: Option<GoogleV3Route>,
    pub tokenhub: Option<TokenHubTranslationOptions>,
}

pub fn installed(protocol: &str) -> bool {
    matches!(
        protocol,
        "deepl_v2"
            | "google_translate_v2"
            | "google_translate_v3"
            | "baidu_vip"
            | "youdao_v3"
            | "tencent_tokenhub_translation"
    )
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn resource_segment(value: &str) -> bool {
    valid_id(value) && !value.contains(':') && !matches!(value, "." | "..")
}
fn validate_google_model(model: &str, route: &GoogleV3Route) -> AppResult<()> {
    // The four documented model-resource forms have distinct fixed tails.
    // Do not normalize URIs, encoded separators or dot segments into a valid resource.
    let parts = model.split('/').collect::<Vec<_>>();
    let tail = match parts.as_slice() {
        [
            "projects",
            project,
            "locations",
            location,
            "models",
            tail @ ..,
        ] if *project == route.project_id && *location == route.location => tail,
        _ => {
            return Err(failure(
                "invalid_model",
                "Google模型必须属于当前项目和地区的完整资源",
            ));
        }
    };
    let valid = match tail {
        [id] => resource_segment(id) && !matches!(*id, "general" | "translation-llm-custom"),
        ["general", "nmt" | "translation-llm"] => true,
        ["translation-llm-custom", id] => resource_segment(id),
        _ => false,
    };
    if !valid {
        return Err(failure("invalid_model", "Google模型资源的路径结构无效"));
    }
    Ok(())
}
fn language(protocol: &str, value: &str, source: bool) -> AppResult<Option<String>> {
    if source && matches!(value, "" | "auto") {
        return Ok(None);
    }
    if value.is_empty()
        || value == "auto"
        || value.len() > 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(failure(
            "invalid_language",
            "请明确选择有效的源语言与目标语言",
        ));
    }
    let normalized = match value {
        "ja-JP" => "ja",
        "ko-KR" => "ko",
        "zh-CN" | "zh-Hans" => "zh-Hans",
        "zh-TW" | "zh-Hant" => "zh-Hant",
        _ => value,
    };
    let mapped = match (protocol, normalized, source) {
        ("deepl_v2", "zh-Hans" | "zh-Hant" | "zh", true) => "ZH".into(),
        ("deepl_v2", "zh-Hans" | "zh", false) => "ZH-HANS".into(),
        ("deepl_v2", "zh-Hant", false) => "ZH-HANT".into(),
        ("deepl_v2", _, _) => normalized.to_ascii_uppercase(),
        ("baidu_vip", "zh-Hans" | "zh", _) => "zh".into(),
        ("baidu_vip", "zh-Hant", _) => "cht".into(),
        ("baidu_vip", "ja", _) => "jp".into(),
        ("baidu_vip", "ko", _) => "kor".into(),
        ("baidu_vip", "fr", _) => "fra".into(),
        ("baidu_vip", "es", _) => "spa".into(),
        ("baidu_vip", "ar", _) => "ara".into(),
        ("baidu_vip", "vi", _) => "vie".into(),
        ("youdao_v3", "zh-Hans" | "zh", _) => "zh-CHS".into(),
        ("youdao_v3", "zh-Hant", _) => "zh-CHT".into(),
        ("tencent_tokenhub_translation", "zh-Hans" | "zh", _) => "zh".into(),
        ("tencent_tokenhub_translation", "zh-Hant", _) => "zh-TR".into(),
        ("google_translate_v2" | "google_translate_v3", "zh-Hans", _) => "zh-CN".into(),
        ("google_translate_v2" | "google_translate_v3", "zh-Hant", _) => "zh-TW".into(),
        (_, _, _) => normalized.to_owned(),
    };
    Ok(Some(mapped))
}

fn key<'a>(auth: &'a ProviderAuthLease<'a>) -> AppResult<&'a str> {
    let ProviderAuthLease::ApiKey { source, key } = auth else {
        return Err(failure(
            "credential_kind_mismatch",
            "所选协议需要明确的API Key材料",
        ));
    };
    source.validate()?;
    if key.expose().bytes().any(|b| b.is_ascii_control()) {
        return Err(failure("invalid_credential", "API Key不能包含控制字符"));
    }
    Ok(key.expose())
}

/// Shared admission is intentionally usable before an outer gateway reads a secret.
/// It checks public request/route facts, not runtime authorization.
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
        return Err(failure("protocol_mismatch", "请求和已选传统翻译连接不一致"));
    }
    if purpose != CatalogPurpose::Translation {
        return Err(failure(
            "purpose_not_supported",
            "传统翻译服务仅用于翻译，不能执行文法或语境查词",
        ));
    }
    if plan.structured {
        return Err(failure(
            "unsupported_structured_output",
            "此翻译接口不接受模型结构化输出参数",
        ));
    }
    if plan.stream && plan.protocol != "tencent_tokenhub_translation" {
        return Err(failure(
            "stream_not_supported",
            "所选翻译接口不支持流式输出",
        ));
    }
    if !plan.parameters.is_empty() {
        return Err(failure(
            "unsupported_parameter",
            "此翻译接口不接受通用模型生成参数",
        ));
    }
    if plan.source_text.trim().is_empty() || plan.source_text.contains('\0') {
        return Err(failure("invalid_input", "待翻译原文为空或包含无效字符"));
    }
    if plan.source_text.len() > MAX_RESPONSE_BYTES {
        return Err(failure("request_too_large", "翻译原文超过请求上限"));
    }
    if plan.protocol == "baidu_vip" && plan.source_text.len() > 6000 {
        return Err(failure("request_too_large", "百度单次原文超过6000字节"));
    }
    if plan.protocol == "youdao_v3" && plan.source_text.chars().count() > 5000 {
        return Err(failure("request_too_large", "有道单次原文超过5000字符"));
    }
    if plan.protocol == "google_translate_v3" && plan.source_text.chars().count() > 30_000 {
        return Err(failure(
            "request_too_large",
            "Google Advanced单次原文超过30000个Unicode码点",
        ));
    }
    language(&plan.protocol, &plan.source_language, true)?;
    language(&plan.protocol, &plan.target_language, false)?;
    endpoint(&plan.endpoint, "")?;
    if plan.protocol == "google_translate_v2" {
        // Basic limits the complete encoded request, not only the original text.
        // This pure check is intentionally available before the Gateway reads Vault.
        bounded_body(&google_v2_body(plan)?, 100_000)?;
    }
    if plan.protocol != "google_translate_v3" && options.google_v3.is_some()
        || plan.protocol != "tencent_tokenhub_translation" && options.tokenhub.is_some()
    {
        return Err(failure("unsupported_parameter", "服务选项不属于所选协议"));
    }
    if plan.protocol == "google_translate_v3" {
        let route = options
            .google_v3
            .as_ref()
            .ok_or_else(|| failure("missing_project", "Google Advanced需要明确的项目和地区"))?;
        if !valid_id(&route.project_id)
            || !valid_id(&route.location)
            || matches!(route.project_id.as_str(), "." | "..")
            || matches!(route.location.as_str(), "." | "..")
            || route.project_id.contains(':')
            || route.location.contains(':')
        {
            return Err(failure("invalid_project", "Google项目或地区标识无效"));
        }
        if let Some(model) = &plan.model {
            validate_google_model(model, route)?;
        }
    }
    if plan.protocol == "tencent_tokenhub_translation" {
        if plan.model.as_deref().is_none_or(|m| !valid_id(m)) {
            return Err(failure("missing_model", "请填写专用翻译接口的实际模型ID"));
        }
        if let Some(options) = &options.tokenhub {
            if options.references.len() > 10
                || options.glossary_ids.len() > 10
                || options.glossary_ids.iter().any(|id| !valid_id(id))
            {
                return Err(failure(
                    "invalid_reference",
                    "翻译参考或术语库数量及标识无效",
                ));
            }
            if options
                .context
                .as_ref()
                .is_some_and(|s| s.len() > MAX_RESPONSE_BYTES || s.contains('\0'))
                || options.references.iter().any(|r| {
                    r.text.is_empty()
                        || r.translation.is_empty()
                        || r.text.contains('\0')
                        || r.translation.contains('\0')
                        || r.text.len() > MAX_RESPONSE_BYTES
                        || r.translation.len() > MAX_RESPONSE_BYTES
                })
            {
                return Err(failure("invalid_reference", "翻译上下文或参考文本无效"));
            }
        }
    }
    Ok(())
}

fn bounded_body(body: &Value, limit: usize) -> AppResult<()> {
    if serde_json::to_vec(body)
        .map_err(|_| failure("invalid_request", "无法编码翻译请求"))?
        .len()
        > limit
    {
        return Err(failure(
            "request_too_large",
            "翻译请求超过所选服务的容量上限",
        ));
    }
    Ok(())
}
fn google_v2_body(plan: &RequestPlan) -> AppResult<Value> {
    let mut body = json!({"q":[plan.source_text],"target":language(&plan.protocol, &plan.target_language, false)?,"format":"text"});
    if let Some(source) = language(&plan.protocol, &plan.source_language, true)? {
        body["source"] = json!(source);
    }
    Ok(body)
}

async fn checked_google(response: reqwest::Response) -> AppResult<reqwest::Response> {
    let status = response.status().as_u16();
    if !matches!(status, 403 | 429) {
        return checked(response).await;
    }
    let value = match bounded_json(response).await {
        Ok(value) => value,
        // Unknown/malformed 403 bodies still describe a permission failure. Bounds
        // and transport failures remain explicit; no server text enters diagnostics.
        Err(error) if error.code == "malformed_response" => Value::Null,
        Err(error) => return Err(error),
    };
    let mut quota =
        value.pointer("/error/status").and_then(Value::as_str) == Some("RESOURCE_EXHAUSTED");
    let mut rate = false;
    let legacy = value
        .pointer("/error/errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let detailed = value
        .pointer("/error/details")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|detail| {
            detail.get("@type").and_then(Value::as_str)
                == Some("type.googleapis.com/google.rpc.ErrorInfo")
        });
    for reason in legacy
        .chain(detailed)
        .filter_map(|entry| entry.get("reason").and_then(Value::as_str))
    {
        match reason {
            "dailyLimitExceeded" | "quotaExceeded" | "QUOTA_EXCEEDED" => quota = true,
            "userRateLimitExceeded" | "rateLimitExceeded" | "RATE_LIMIT_EXCEEDED" => rate = true,
            _ => {}
        }
    }
    let code = if rate {
        "rate_limited"
    } else if quota {
        "quota_exceeded"
    } else if status == 429 {
        "rate_limited"
    } else {
        "authentication_failed"
    };
    Err(failure(
        code,
        "Google翻译拒绝本次请求；请检查权限、配额或请求频率",
    ))
}
fn output(text: String) -> AppResult<ModelOutput> {
    if text.trim().is_empty() {
        return Err(failure("empty_response", "翻译服务未返回译文"));
    }
    if text.contains('\0') {
        return Err(failure("malformed_response", "翻译服务返回无效字符"));
    }
    if text.len() > MAX_RESPONSE_BYTES {
        return Err(failure("response_too_large", "译文超过上限"));
    }
    Ok(ModelOutput { text })
}
fn one_translation(value: &Value, array_path: &str, text_key: &str) -> AppResult<String> {
    let items = value
        .pointer(array_path)
        .and_then(Value::as_array)
        .filter(|items| items.len() == 1)
        .ok_or_else(|| failure("malformed_response", "翻译服务未返回与请求对应的单条译文"))?;
    items[0]
        .get(text_key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| failure("malformed_response", "翻译服务返回的译文格式无效"))
}
fn service_error(value: &Value) -> AppResult<()> {
    if value.get("error").is_some_and(|v| !v.is_null()) {
        return Err(failure(
            "translation_error",
            "翻译服务拒绝或无法处理本次请求",
        ));
    }
    Ok(())
}
fn code(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
fn baidu_text(value: &Value) -> AppResult<String> {
    if let Some(c) = value
        .get("error_code")
        .and_then(code)
        .filter(|s| !matches!(s.as_str(), "0" | "52000"))
    {
        let kind = match c.as_str() {
            "52003" | "54001" | "58000" | "90107" => "authentication_failed",
            "54003" | "54005" => "rate_limited",
            "54004" => "quota_exceeded",
            "58001" => "unsupported_language",
            "52001" => "timeout",
            _ => "translation_error",
        };
        return Err(failure(kind, "百度翻译拒绝或无法处理本次请求"));
    }
    let items = value["trans_result"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| failure("malformed_response", "百度翻译未返回译文"))?;
    items
        .iter()
        .map(|item| {
            item["dst"]
                .as_str()
                .ok_or_else(|| failure("malformed_response", "百度译文格式无效"))
        })
        .collect::<AppResult<Vec<_>>>()
        .map(|parts| parts.join("\n"))
}
fn youdao_text(value: &Value) -> AppResult<String> {
    let c = value
        .get("errorCode")
        .and_then(code)
        .ok_or_else(|| failure("malformed_response", "有道响应缺少结果状态"))?;
    if c != "0" {
        let kind = match c.as_str() {
            "108" | "110" | "111" | "112" | "202" | "203" | "205" | "206" | "207" => {
                "authentication_failed"
            }
            "411" | "412" => "rate_limited",
            "401" => "quota_exceeded",
            "102" => "unsupported_language",
            "103" => "request_too_large",
            _ => "translation_error",
        };
        return Err(failure(kind, "有道翻译拒绝或无法处理本次请求"));
    }
    let items = value["translation"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| failure("malformed_response", "有道未返回译文"))?;
    items
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| failure("malformed_response", "有道译文格式无效"))
        })
        .collect::<AppResult<Vec<_>>>()
        .map(|parts| parts.join("\n"))
}

/// MD5 is required solely by the Baidu wire contract; it is not an integrity or credential-protection primitive.
fn baidu_sign(app_id: &str, source: &str, salt: &str, secret: &str) -> String {
    let mut hash = md5::Context::new();
    for part in [app_id, source, salt, secret] {
        hash.consume(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn youdao_input(source: &str) -> String {
    let count = source.chars().count();
    if count <= 20 {
        return source.to_owned();
    }
    let first = source.chars().take(10).collect::<String>();
    let last = source.chars().skip(count - 10).collect::<String>();
    format!("{first}{count}{last}")
}
fn youdao_sign(app_key: &str, source: &str, salt: &str, time: &str, secret: &str) -> String {
    let mut hash = Sha256::new();
    for part in [app_key, &youdao_input(source), salt, time, secret] {
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn tokenhub_fragment(value: &Value, streaming: bool) -> AppResult<Fragment> {
    service_error(value)?;
    let Some(choices) = value.get("choices").and_then(Value::as_array) else {
        if streaming && value.get("usage").is_some() {
            return Ok(Fragment::Ignore);
        }
        return Err(failure("malformed_response", "专用翻译接口缺少输出"));
    };
    if streaming && choices.is_empty() && value.get("usage").is_some() {
        return Ok(Fragment::Ignore);
    }
    let choice = choices
        .first()
        .filter(|_| choices.len() == 1)
        .ok_or_else(|| failure("malformed_response", "专用翻译接口返回的候选数量无效"))?;
    if choice.pointer("/delta/tool_calls").is_some()
        || choice.pointer("/message/tool_calls").is_some()
    {
        return Err(failure("output_incomplete", "专用翻译接口不能执行工具调用"));
    }
    let stop = choice.get("finish_reason").and_then(Value::as_str);
    if stop.is_some_and(|s| s != "stop") {
        return Err(failure("output_incomplete", "专用翻译输出被截断或拒绝"));
    }
    let text = choice.pointer(if streaming {
        "/delta/content"
    } else {
        "/message/content"
    });
    let text = match text {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None if streaming => String::new(),
        _ => return Err(failure("malformed_response", "专用翻译正文格式无效")),
    };
    // Reject invalid text before the shared stream invokes a UI-facing callback.
    if text.contains('\0') {
        return Err(failure("malformed_response", "专用翻译正文包含无效字符"));
    }
    if stop == Some("stop") {
        Ok(Fragment::LastText(text))
    } else if streaming {
        Ok(Fragment::Text(text))
    } else {
        Err(failure("output_incomplete", "专用翻译接口未完整结束"))
    }
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
    // Validate material kind and metadata before constructing a network request.
    match (plan.protocol.as_str(), auth) {
        (
            "deepl_v2" | "google_translate_v2" | "tencent_tokenhub_translation",
            ProviderAuthLease::ApiKey { .. },
        ) => {
            key(auth)?;
        }
        (
            "baidu_vip",
            ProviderAuthLease::AppSecret {
                source,
                material: SignedAppLease::Baidu { app_id, .. },
            },
        ) => {
            source.validate()?;
            if !valid_id(app_id) {
                return Err(failure("invalid_credential", "百度应用ID无效"));
            }
        }
        (
            "youdao_v3",
            ProviderAuthLease::AppSecret {
                source,
                material: SignedAppLease::Youdao { app_key, .. },
            },
        ) => {
            source.validate()?;
            if !valid_id(app_key) {
                return Err(failure("invalid_credential", "有道应用ID无效"));
            }
        }
        ("google_translate_v3", ProviderAuthLease::GoogleAccess { token, identity }) => {
            identity.validate()?;
            if token.expose().bytes().any(|b| b.is_ascii_control()) {
                return Err(failure("invalid_credential", "Google短期授权材料无效"));
            }
        }
        (
            "google_translate_v3",
            ProviderAuthLease::GoogleServiceAccountAccess { token, identity },
        ) => {
            identity.validate()?;
            if token.expose().bytes().any(|b| b.is_ascii_control()) {
                return Err(failure("invalid_credential", "Google短期授权材料无效"));
            }
        }
        _ => {
            return Err(failure(
                "credential_kind_mismatch",
                "认证材料类型不适用于所选翻译协议",
            ));
        }
    }
    let source = language(&plan.protocol, &plan.source_language, true)?;
    let target = language(&plan.protocol, &plan.target_language, false)?
        .ok_or_else(|| failure("invalid_language", "请选择目标语言"))?;
    let transport = crate::providers::client_with_headers(connection, plan.timeout_ms, headers)?;
    let mut request = match plan.protocol.as_str() {
        "deepl_v2" => {
            let mut body = json!({"text":[plan.source_text],"target_lang":target});
            if let Some(source) = source {
                body["source_lang"] = json!(source);
            }
            bounded_body(&body, 128 * 1024)?;
            transport
                .post(endpoint(&plan.endpoint, "v2/translate")?)
                .header("Authorization", format!("DeepL-Auth-Key {}", key(auth)?))
                .json(&body)
        }
        "google_translate_v2" => {
            let body = google_v2_body(plan)?;
            transport
                .post(endpoint(&plan.endpoint, "language/translate/v2")?)
                .header("x-goog-api-key", key(auth)?)
                .json(&body)
        }
        "google_translate_v3" => {
            let route = options
                .google_v3
                .as_ref()
                .ok_or_else(|| failure("missing_project", "请选择Google项目"))?;
            let token = match auth {
                ProviderAuthLease::GoogleAccess { token, .. }
                | ProviderAuthLease::GoogleServiceAccountAccess { token, .. } => token,
                _ => {
                    return Err(failure(
                        "credential_kind_mismatch",
                        "Google Advanced需要OAuth授权",
                    ));
                }
            };
            let mut body = json!({"contents":[plan.source_text],"targetLanguageCode":target,"mimeType":"text/plain"});
            if let Some(source) = source {
                body["sourceLanguageCode"] = json!(source);
            }
            if let Some(model) = &plan.model {
                body["model"] = json!(model);
            }
            bounded_body(&body, MAX_RESPONSE_BYTES)?;
            let route = format!(
                "v3/projects/{}/locations/{}:translateText",
                route.project_id, route.location
            );
            transport
                .post(endpoint(&plan.endpoint, &route)?)
                .bearer_auth(token.expose())
                .json(&body)
        }
        "baidu_vip" => {
            let ProviderAuthLease::AppSecret {
                material: SignedAppLease::Baidu { app_id, secret },
                ..
            } = auth
            else {
                return Err(failure("credential_kind_mismatch", "百度需要应用签名材料"));
            };
            let salt = uuid::Uuid::new_v4().to_string();
            let sign = baidu_sign(app_id, &plan.source_text, &salt, secret.expose());
            let form = [
                ("q", plan.source_text.as_str()),
                ("from", source.as_deref().unwrap_or("auto")),
                ("to", target.as_str()),
                ("appid", app_id),
                ("salt", salt.as_str()),
                ("sign", sign.as_str()),
            ];
            transport
                .post(endpoint(&plan.endpoint, "api/trans/vip/translate")?)
                .form(&form)
        }
        "youdao_v3" => {
            let ProviderAuthLease::AppSecret {
                material: SignedAppLease::Youdao { app_key, secret },
                ..
            } = auth
            else {
                return Err(failure("credential_kind_mismatch", "有道需要应用签名材料"));
            };
            let salt = uuid::Uuid::new_v4().to_string();
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| failure("invalid_clock", "系统时间无效，无法签名"))?
                .as_secs()
                .to_string();
            let sign = youdao_sign(app_key, &plan.source_text, &salt, &time, secret.expose());
            let form = [
                ("q", plan.source_text.as_str()),
                ("from", source.as_deref().unwrap_or("auto")),
                ("to", target.as_str()),
                ("appKey", app_key),
                ("salt", salt.as_str()),
                ("signType", "v3"),
                ("curtime", time.as_str()),
                ("sign", sign.as_str()),
                ("strict", "true"),
            ];
            transport.post(endpoint(&plan.endpoint, "api")?).form(&form)
        }
        "tencent_tokenhub_translation" => {
            let mut body = json!({"model":plan.model,"text":plan.source_text,"target":target,"stream":plan.stream});
            if let Some(source) = source {
                body["source"] = json!(source);
            }
            if let Some(options) = &options.tokenhub {
                if let Some(context) = &options.context {
                    body["context"] = json!(context);
                }
                if !options.references.is_empty() {
                    body["references"] = json!(options.references.iter().map(|r|json!({"type":match r.kind{ReferenceKind::Sentence=>"sentence",ReferenceKind::Term=>"term"},"text":r.text,"translation":r.translation})).collect::<Vec<_>>());
                }
                if !options.glossary_ids.is_empty() {
                    body["glossary_ids"] = json!(options.glossary_ids);
                }
            }
            bounded_body(&body, MAX_RESPONSE_BYTES)?;
            transport
                .post(endpoint(&plan.endpoint, "api/translations")?)
                .bearer_auth(key(auth)?)
                .json(&body)
        }
        _ => return Err(failure("unsupported_protocol", "未安装此翻译协议")),
    };
    if plan.stream {
        request = request.header("Accept", "text/event-stream");
    }
    let response = request.send().await.map_err(network_error)?;
    if plan.protocol == "deepl_v2" && response.status().as_u16() == 456 {
        return Err(failure("quota_exceeded", "DeepL翻译额度已用完"));
    }
    let response = if matches!(
        plan.protocol.as_str(),
        "google_translate_v2" | "google_translate_v3"
    ) {
        checked_google(response).await?
    } else {
        checked(response).await?
    };
    if plan.stream {
        let text = stream::sse(response, |_, v| tokenhub_fragment(v, true), &on_delta).await?;
        return output(text);
    }
    let value = bounded_json(response).await?;
    service_error(&value)?;
    let text = match plan.protocol.as_str() {
        "deepl_v2" => one_translation(&value, "/translations", "text")?,
        "google_translate_v2" => one_translation(&value, "/data/translations", "translatedText")?,
        "google_translate_v3" => one_translation(&value, "/translations", "translatedText")?,
        "baidu_vip" => baidu_text(&value)?,
        "youdao_v3" => youdao_text(&value)?,
        "tencent_tokenhub_translation" => match tokenhub_fragment(&value, false)? {
            Fragment::LastText(text) => text,
            _ => return Err(failure("output_incomplete", "专用翻译接口未完整结束")),
        },
        _ => return Err(failure("unsupported_protocol", "未安装此翻译协议")),
    };
    let result = output(text)?;
    on_delta(&result.text);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signing_vectors_preserve_original_utf8_before_form_encoding() {
        assert_eq!(
            baidu_sign("2015063000000001", "apple", "1435660288", "12345678"),
            "f89f9594663708c1605f3d736d01d2d4"
        );
        assert_eq!(youdao_input("0123456789abcdefghij"), "0123456789abcdefghij");
        assert_eq!(
            youdao_input("0123456789😀abcdefghij"),
            "012345678921abcdefghij"
        );
        assert_eq!(
            youdao_input("😀😀😀😀😀😀😀😀😀😀日😀😀😀😀😀😀😀😀😀😀"),
            "😀😀😀😀😀😀😀😀😀😀21😀😀😀😀😀😀😀😀😀😀"
        );
    }
}
