//! Exact native model-list routes. Discovery is never a translation, resource
//! installation, model load, or qualification of a configured purpose.
use crate::providers::{bounded_json, checked, endpoint, failure, network_error};
use app_core::contracts::{AppResult, Connection};
use serde_json::Value;
use std::{collections::BTreeSet, time::Duration};

const MAX_MODELS: usize = 4096;
const MAX_PAGES: usize = 32;
const MAX_ID_BYTES: usize = 512;
const MAX_CURSOR_BYTES: usize = 4096;

pub(in crate::providers) fn installed(protocol: &str) -> bool {
    matches!(
        protocol,
        "openai_responses" | "gemini_generate_content" | "ollama_native" | "lmstudio_native_v1"
    )
}

fn invalid() -> app_core::contracts::AppError {
    failure(
        "invalid_model_list",
        "服务返回的模型列表不完整或格式不符；原模型选择保持不变，可手动填写ID",
    )
}
fn model_id(value: &Value) -> AppResult<String> {
    let id = value.as_str().ok_or_else(invalid)?;
    if id.is_empty()
        || id.trim() != id
        || id.len() > MAX_ID_BYTES
        || id.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(id.to_owned())
}
fn route(connection: &Connection) -> AppResult<reqwest::Url> {
    let (suffix, list) = match connection.protocol.as_str() {
        "openai_responses" => ("/responses", "models"),
        "gemini_generate_content" => ("", "models"),
        "ollama_native" => ("/api/chat", "api/tags"),
        "lmstudio_native_v1" => ("/api/v1/chat", "api/v1/models"),
        _ => {
            return Err(failure(
                "discovery_unsupported",
                "所选协议没有明确的原生模型发现路线，可手动填写模型ID",
            ));
        }
    };
    let mut base = endpoint(
        connection
            .endpoint
            .as_deref()
            .ok_or_else(|| failure("missing_endpoint", "请填写服务地址"))?,
        "",
    )?;
    // Only this exact dialect's complete execution route can be removed. No
    // host guessing, server-provided URL, or cross-protocol fallback is used.
    if !suffix.is_empty() && base.path().trim_end_matches('/').ends_with(suffix) {
        let path = base.path().trim_end_matches('/');
        let prefix = &path[..path.len() - suffix.len()];
        base.set_path(&format!("{prefix}/"));
    }
    endpoint(base.as_str(), list)
}

pub async fn discover(connection: &Connection, key: Option<&str>) -> AppResult<Vec<String>> {
    discover_with_headers(connection, key, &reqwest::header::HeaderMap::new()).await
}
pub(in crate::providers) async fn discover_with_headers(
    connection: &Connection,
    key: Option<&str>,
    headers: &reqwest::header::HeaderMap,
) -> AppResult<Vec<String>> {
    if !installed(&connection.protocol) {
        return Err(failure(
            "discovery_unsupported",
            "没有安装所选协议的模型发现路线",
        ));
    }
    match connection.auth_mode.as_str() {
        "none"
            if matches!(
                connection.protocol.as_str(),
                "ollama_native" | "lmstudio_native_v1"
            ) && key.is_none() => {}
        "api_key" if key.is_some_and(|key| !key.is_empty()) => {}
        "api_key" => return Err(failure("missing_credential", "模型列表需要已配置的凭据")),
        _ => {
            return Err(failure(
                "unsupported_authentication",
                "当前认证材料不适用于所选模型列表接口",
            ));
        }
    }
    tokio::time::timeout(
        Duration::from_secs(15),
        discover_all(connection, key, headers),
    )
    .await
    .map_err(|_| failure("timeout", "获取模型列表超时；原模型选择保持不变"))?
}

async fn discover_all(
    connection: &Connection,
    key: Option<&str>,
    headers: &reqwest::header::HeaderMap,
) -> AppResult<Vec<String>> {
    let base = route(connection)?;
    let http = super::super::client_with_headers(connection, 15_000, headers)?;
    let mut ids = BTreeSet::new();
    let mut seen_cursors = BTreeSet::new();
    let mut cursor: Option<String> = None;
    let mut count = 0usize;
    for _ in 0..MAX_PAGES {
        let mut url = base.clone();
        let gemini = connection.protocol == "gemini_generate_content";
        if gemini {
            url.query_pairs_mut().append_pair("pageSize", "1000");
            if let Some(cursor) = &cursor {
                url.query_pairs_mut().append_pair("pageToken", cursor);
            }
        }
        let mut request = http.get(url);
        if let Some(key) = key {
            request = if gemini {
                request.header("x-goog-api-key", key)
            } else {
                request.bearer_auth(key)
            };
        }
        let value =
            bounded_json(checked(request.send().await.map_err(network_error)?).await?).await?;
        if value.get("error").is_some_and(|error| !error.is_null()) {
            return Err(invalid());
        }
        let field = if connection.protocol == "openai_responses" {
            "data"
        } else {
            "models"
        };
        let entries = value
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        count = count.checked_add(entries.len()).ok_or_else(invalid)?;
        if count > MAX_MODELS {
            return Err(failure(
                "model_list_limit",
                "模型列表超出本次读取上限；可手动填写ID",
            ));
        }
        for entry in entries {
            let id = match connection.protocol.as_str() {
                "openai_responses" => model_id(&entry["id"])?,
                "ollama_native" => model_id(&entry["name"])?,
                "lmstudio_native_v1" => {
                    match entry.get("type").and_then(Value::as_str) {
                        Some("embedding" | "decision") => continue,
                        Some("llm") => {}
                        _ => return Err(invalid()),
                    }
                    model_id(&entry["key"])?
                }
                "gemini_generate_content" => {
                    let methods = entry
                        .get("supportedGenerationMethods")
                        .and_then(Value::as_array)
                        .ok_or_else(invalid)?;
                    if !methods.iter().all(Value::is_string) {
                        return Err(invalid());
                    }
                    if !methods
                        .iter()
                        .any(|method| method.as_str() == Some("generateContent"))
                    {
                        continue;
                    }
                    let name = model_id(&entry["name"])?;
                    let id = name.strip_prefix("models/").ok_or_else(invalid)?;
                    if id.is_empty()
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                    {
                        return Err(invalid());
                    }
                    id.to_owned()
                }
                _ => unreachable!("installed dialect checked before I/O"),
            };
            ids.insert(id);
        }
        // Non-paginated APIs must not silently discard a continuation marker.
        if !gemini {
            if value
                .get("has_more")
                .is_some_and(|v| v != &Value::Bool(false))
                || value
                    .get("nextPageToken")
                    .is_some_and(|v| v != "" && !v.is_null())
            {
                return Err(invalid());
            }
            return Ok(ids.into_iter().collect());
        }
        cursor = match value.get("nextPageToken") {
            None | Some(Value::Null) => None,
            Some(Value::String(token)) if token.is_empty() => None,
            Some(Value::String(token))
                if token.len() <= MAX_CURSOR_BYTES && !token.chars().any(char::is_control) =>
            {
                Some(token.clone())
            }
            _ => return Err(invalid()),
        };
        let Some(next) = &cursor else {
            return Ok(ids.into_iter().collect());
        };
        if !seen_cursors.insert(next.clone()) {
            return Err(invalid());
        }
    }
    Err(failure(
        "model_list_limit",
        "模型列表分页超出本次读取上限；原模型选择保持不变",
    ))
}
