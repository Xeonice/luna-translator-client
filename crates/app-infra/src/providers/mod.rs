//! Async protocol adapters. No application configuration is resolved here.
pub mod google_interactive;
pub mod adapters;
pub mod gateway;
pub mod google_auth;
pub mod google_token;
pub mod google_oauth;
pub mod google_authorization;
pub mod presets;
pub mod profiles;
pub mod register;
pub mod registry;
mod typed_http;
mod service_testing;
mod operation_io;
pub mod service_discovery;
mod google_execution;
use app_core::{
    application::request_plan::RequestPlan,
    contracts::{AppError, AppResult, Connection, ProtocolKey},
};
use eventsource_stream::{EventStreamError, Eventsource};
use futures_util::StreamExt;
pub use register::builtin_registry;
use reqwest::{Client, Response, Url};
use serde_json::{Value, json};
use std::time::Duration;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct ModelOutput {
    pub text: String,
}

fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "provider", message)
}
fn plan_generation_parameters(
    plan: &RequestPlan,
) -> AppResult<std::collections::BTreeMap<String, Value>> {
    // The original Azure-only legacy endpoint never sent model generation
    // parameters. Its frozen grammar remains distinct from explicit bindings.
    if plan.protocol == "azure_translator" && plan.adapter_binding.is_none() {
        return Ok(std::collections::BTreeMap::new());
    }
    let protocol = plan
        .adapter_binding
        .as_ref()
        .map(|binding| binding.protocol.clone())
        .unwrap_or_else(|| ProtocolKey {
            protocol_id: plan.protocol.clone(),
            contract_revision: 1,
        });
    if protocol.protocol_id != plan.protocol {
        return Err(failure(
            "protocol_mismatch",
            "生成参数的协议身份与请求不一致",
        ));
    }
    app_core::domain::model_config::wire_parameters::generation_wire_parameters(
        &protocol,
        &plan.parameters,
    )
}
fn validate_output_text(text: &str) -> AppResult<()> {
    if text.contains('\0') {
        return Err(failure("invalid_output", "服务正文包含无效NUL字符"));
    }
    if text.len() > MAX_RESPONSE_BYTES {
        return Err(failure("response_too_large", "模型输出超过上限"));
    }
    Ok(())
}
pub fn endpoint(base: &str, route: &str) -> AppResult<Url> {
    let mut url = Url::parse(base).map_err(|_| failure("invalid_endpoint", "服务地址无效"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(failure(
            "invalid_endpoint",
            "服务地址只能包含HTTP(S)主机与路径，凭据及参数需独立配置",
        ));
    }
    let path: Vec<_> = url.path().split('/').filter(|s| !s.is_empty()).collect();
    let route: Vec<_> = route.split('/').filter(|s| !s.is_empty()).collect();
    if route
        .iter()
        .any(|part| matches!(*part, "." | "..") || part.contains(['?', '#']))
    {
        return Err(failure("invalid_route", "协议路径无效"));
    }
    // Defaults may already include /api, /api/v1 or the complete endpoint.
    // Compare whole path segments, never suffixes of unrelated segment names.
    let overlap = (0..=path.len().min(route.len()))
        .rev()
        .find(|&count| path[path.len() - count..] == route[..count])
        .unwrap_or(0);
    let joined = path
        .iter()
        .copied()
        .chain(route[overlap..].iter().copied())
        .collect::<Vec<_>>()
        .join("/");
    url.set_path(&format!("/{joined}"));
    Ok(url)
}
fn client_with_headers(
    connection: &Connection,
    timeout: u64,
    headers: &reqwest::header::HeaderMap,
) -> AppResult<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_millis(timeout))
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .default_headers(headers.clone());
    match connection.proxy_mode.as_str() {
        "disabled" => builder = builder.no_proxy(),
        "override" => {
            let proxy = connection
                .network_metadata
                .get("proxyUrl")
                .and_then(Value::as_str)
                .ok_or_else(|| failure("invalid_proxy", "独立代理地址尚未填写"))?;
            let parsed = Url::parse(proxy).map_err(|_| failure("invalid_proxy", "代理地址无效"))?;
            if !parsed.username().is_empty() || parsed.password().is_some() {
                return Err(failure("secret_in_proxy", "代理地址不能包含明文凭据"));
            }
            builder = builder.proxy(
                reqwest::Proxy::all(proxy).map_err(|_| failure("invalid_proxy", "代理协议无效"))?,
            );
        }
        "inherit" => {}
        _ => return Err(failure("invalid_proxy", "未知代理模式")),
    }
    builder
        .build()
        .map_err(|_| failure("client_initialization", "无法初始化网络客户端"))
}
fn azure_language(value: &str) -> &str {
    match value {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        _ => value,
    }
}
fn network_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        failure("timeout", "服务请求超时，可检查网络后重试")
    } else if error.is_connect() {
        failure("connection_failed", "无法连接服务，请检查地址、代理与网络")
    } else {
        failure("network_error", "服务连接中断")
    }
}
async fn checked(response: Response) -> AppResult<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    // Server bodies and full URLs may contain keys or source text. Do not echo them to diagnostics.
    let status = response.status().as_u16();
    Err(failure(
        match status {
            401 | 403 => "authentication_failed",
            429 => "rate_limited",
            500..=599 => "service_unavailable",
            _ => "http_error",
        },
        &format!("服务返回HTTP {status}；请检查该连接设置"),
    ))
}
async fn bounded_json(mut response: Response) -> AppResult<Value> {
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if data.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(failure("response_too_large", "服务响应超过上限"));
        }
        data.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&data).map_err(|_| failure("malformed_response", "服务返回无效JSON"))
}
fn authorize(
    request: reqwest::RequestBuilder,
    protocol: &str,
    key: Option<&str>,
) -> reqwest::RequestBuilder {
    match (protocol, key) {
        ("anthropic", Some(key)) => request.header("x-api-key", key),
        ("azure_translator", Some(key)) => request.header("Ocp-Apim-Subscription-Key", key),
        (_, Some(key)) => request.bearer_auth(key),
        _ => request,
    }
}

pub async fn execute(
    plan: &RequestPlan,
    connection: &Connection,
    key: Option<&str>,
    on_delta: impl Fn(&str),
) -> AppResult<ModelOutput> {
    execute_with_headers(
        plan,
        connection,
        key,
        &reqwest::header::HeaderMap::new(),
        on_delta,
    )
    .await
}
async fn execute_with_headers(
    plan: &RequestPlan,
    connection: &Connection,
    key: Option<&str>,
    headers: &reqwest::header::HeaderMap,
    on_delta: impl Fn(&str),
) -> AppResult<ModelOutput> {
    let wire_parameters = plan_generation_parameters(plan)?;
    if adapters::llm::installed(&plan.protocol) {
        return adapters::llm::execute_with_headers(plan, connection, key, headers, on_delta).await;
    }
    if plan.protocol != connection.protocol {
        return Err(failure("protocol_mismatch", "请求与连接协议不一致"));
    }
    if connection.auth_mode == "api_key" && key.is_none() {
        return Err(failure("missing_credential", "连接凭据不可用，请重新授权"));
    }
    let client = client_with_headers(connection, plan.timeout_ms, headers)?;
    if plan.protocol == "azure_translator" {
        let mut url = endpoint(&plan.endpoint, "translate")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("api-version", "3.0")
                .append_pair("to", azure_language(&plan.target_language));
            if !plan.source_language.is_empty() && plan.source_language != "auto" {
                pairs.append_pair("from", azure_language(&plan.source_language));
            }
        }
        let mut request = authorize(client.post(url), &plan.protocol, key)
            .json(&json!([{"Text":plan.source_text}]));
        if let Some(region) = connection
            .network_metadata
            .get("region")
            .and_then(Value::as_str)
        {
            request = request.header("Ocp-Apim-Subscription-Region", region);
        }
        let json =
            bounded_json(checked(request.send().await.map_err(network_error)?).await?).await?;
        let text = json
            .pointer("/0/translations/0/text")
            .and_then(Value::as_str)
            .ok_or_else(|| failure("malformed_response", "翻译服务未返回译文"))?
            .to_owned();
        validate_output_text(&text)?;
        if text.trim().is_empty() {
            return Err(failure("empty_response", "翻译服务未返回译文"));
        }
        on_delta(&text);
        return Ok(ModelOutput { text });
    }
    let mut body = serde_json::Map::new();
    for (key, value) in wire_parameters {
        body.insert(key, value);
    }
    body.insert(
        "model".into(),
        json!(
            plan.model
                .as_ref()
                .ok_or_else(|| failure("missing_model", "请填写实际模型ID"))?
        ),
    );
    body.insert("stream".into(), json!(plan.stream));
    let route = match plan.protocol.as_str() {
        "openai_compatible" => "chat/completions",
        "anthropic" => "messages",
        _ => return Err(failure("unsupported_protocol", "未支持的模型协议")),
    };
    if plan.protocol == "anthropic" {
        body.entry("max_tokens").or_insert(json!(2048));
        body.insert(
            "system".into(),
            plan.messages
                .first()
                .and_then(|m| m.get("content"))
                .cloned()
                .unwrap_or(json!("")),
        );
        body.insert(
            "messages".into(),
            json!(plan.messages.iter().skip(1).collect::<Vec<_>>()),
        );
    } else {
        body.insert("messages".into(), json!(plan.messages));
    }
    let mut request = authorize(
        client.post(endpoint(&plan.endpoint, route)?),
        &plan.protocol,
        key,
    )
    .header("Accept", "text/event-stream")
    .json(&body);
    if plan.protocol == "anthropic" {
        request = request.header("anthropic-version", "2023-06-01");
    }
    let response = checked(request.send().await.map_err(network_error)?).await?;
    if !plan.stream {
        let value = bounded_json(response).await?;
        if value.get("error").is_some() {
            return Err(failure("model_error", "模型返回错误"));
        }
        let stop = if plan.protocol == "anthropic" {
            value.get("stop_reason")
        } else {
            value.pointer("/choices/0/finish_reason")
        }
        .and_then(Value::as_str);
        if matches!(
            stop,
            Some("length" | "max_tokens" | "content_filter" | "tool_calls")
        ) {
            return Err(failure(
                "output_incomplete",
                "模型输出被截断、拒绝或需要未支持的工具调用",
            ));
        }
        let text = if plan.protocol == "anthropic" {
            value.get("content").and_then(Value::as_array).map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
        } else {
            value
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }
        .ok_or_else(|| failure("malformed_response", "模型未返回文字内容"))?;
        validate_output_text(&text)?;
        if text.trim().is_empty() {
            return Err(failure("empty_response", "模型未返回文字内容"));
        }
        on_delta(&text);
        return Ok(ModelOutput { text });
    }
    if !response
        .headers()
        .get("content-type")
        .and_then(|x| x.to_str().ok())
        .is_some_and(|x| x.starts_with("text/event-stream"))
    {
        return Err(failure(
            "stream_not_supported",
            "服务未返回SSE流；请核对协议和地址",
        ));
    }
    let mut received = 0usize;
    let bytes = response.bytes_stream().map(move |chunk| {
        chunk.map_err(network_error).and_then(|bytes| {
            received = received.saturating_add(bytes.len());
            if received > MAX_RESPONSE_BYTES * 4 {
                Err(failure("response_too_large", "模型流数据超过上限"))
            } else {
                Ok(bytes)
            }
        })
    });
    let mut stream = bytes.eventsource();
    let mut text = String::new();
    let mut terminal = false;
    while let Some(event) = stream.next().await {
        let event = event.map_err(|error| match error {
            EventStreamError::Transport(error) => error,
            EventStreamError::Utf8(_) | EventStreamError::Parser(_) => {
                failure("stream_interrupted", "模型流解析失败或连接中断")
            }
        })?;
        if event.data == "[DONE]" {
            terminal = true;
            break;
        }
        let value: Value = serde_json::from_str(&event.data)
            .map_err(|_| failure("malformed_stream", "模型流包含无效JSON"))?;
        let stop = value
            .pointer("/choices/0/finish_reason")
            .or_else(|| value.pointer("/delta/stop_reason"))
            .and_then(Value::as_str);
        if matches!(
            stop,
            Some("length" | "max_tokens" | "content_filter" | "tool_calls")
        ) || value.pointer("/choices/0/delta/tool_calls").is_some()
        {
            return Err(failure(
                "output_incomplete",
                "模型输出被截断、拒绝或需要未支持的工具调用",
            ));
        }
        if value.get("error").is_some() || event.event == "error" {
            return Err(failure("model_error", "模型服务在流中返回错误"));
        }
        if event.event == "message_stop" {
            terminal = true;
            break;
        }
        let delta = if plan.protocol == "anthropic" {
            if value.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta") {
                value.pointer("/delta/text").and_then(Value::as_str)
            } else {
                None
            }
        } else {
            value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
        };
        if let Some(delta) = delta {
            validate_output_text(delta)?;
            if text.len() + delta.len() > MAX_RESPONSE_BYTES {
                return Err(failure("response_too_large", "模型输出超过上限"));
            }
            text.push_str(delta);
            on_delta(&text);
        }
    }
    if !terminal {
        return Err(failure(
            "stream_incomplete",
            "模型流提前结束，保留原文，可重试",
        ));
    }
    if text.trim().is_empty() {
        return Err(failure("empty_response", "模型未返回文字内容"));
    }
    Ok(ModelOutput { text })
}

pub async fn discover_models(connection:&Connection,key:Option<&str>) -> AppResult<Vec<String>> {
    discover_models_with_options(connection,key,&reqwest::header::HeaderMap::new(),false).await
}
async fn discover_models_with_options(connection:&Connection,key:Option<&str>,headers:&reqwest::header::HeaderMap,strict:bool) -> AppResult<Vec<String>> {
    if connection.protocol == "azure_translator" {
        return Ok(vec![]);
    }
    if !matches!(
        connection.protocol.as_str(),
        "openai_compatible" | "anthropic"
    ) {
        return Err(failure(
            "discovery_unsupported",
            "所选协议没有已安装的模型发现策略；请明确填写模型ID",
        ));
    }
    let request = authorize(
        client_with_headers(connection, 15000, headers)?.get(endpoint(
            connection
                .endpoint
                .as_deref()
                .ok_or_else(|| failure("missing_endpoint", "请填写服务地址"))?,
            "models",
        )?),
        &connection.protocol,
        key,
    );
    let request = if connection.protocol == "anthropic" {
        request.header("anthropic-version", "2023-06-01")
    } else {
        request
    };
    let result = bounded_json(checked(request.send().await.map_err(network_error)?).await?).await?;
    let values = result
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            failure(
                "discovery_unsupported",
                "服务未提供标准模型列表；可手动填写模型ID",
            )
        })?;
    if strict {
        if result.get("error").is_some_and(|v|!v.is_null()) || values.len()>4096
            || ["has_more","hasMore"].iter().any(|k|result.get(*k).is_some_and(|v|v!=false))
            || ["next_page","nextPage","next_page_token","nextPageToken","next_cursor","nextCursor"].iter().any(|k|result.get(*k).is_some_and(|v|!v.is_null() && v!="")) {
            return Err(failure("invalid_model_list","模型列表不完整或超过上限；原输入已保留"));
        }
        let models=values.iter().map(|v|v.get("id").and_then(Value::as_str).map(|id|app_core::contracts::DiscoveredServiceModel {id:id.into()}).ok_or_else(||failure("invalid_model_list","模型列表格式无效；原输入已保留"))).collect::<AppResult<Vec<_>>>()?;
        app_core::application::service_discovery::validate_discovered_models(&models)?;
        return Ok(models.into_iter().map(|m|m.id).collect());
    }
    Ok(values
        .iter()
        .filter_map(|v| v.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_preserve_base_prefix_without_duplicate_routes() {
        assert_eq!(
            endpoint("https://example.org/v1/", "chat/completions")
                .unwrap()
                .as_str(),
            "https://example.org/v1/chat/completions"
        );
        assert_eq!(
            endpoint("https://example.org/v1/messages", "messages")
                .unwrap()
                .as_str(),
            "https://example.org/v1/messages"
        );
    }
    #[test]
    fn credentials_and_non_http_schemes_rejected() {
        for s in [
            "https://key:secret@example.org",
            "https://example.org?key=secret",
            "file:///x",
        ] {
            assert!(endpoint(s, "models").is_err());
        }
        assert!(endpoint("http://127.0.0.1:8080/v1", "models").is_ok());
    }
}
