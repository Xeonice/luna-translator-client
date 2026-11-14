//! Native Responses, Gemini, Ollama and LM Studio dialects.
//! Official contracts are recorded in the implementation change; no tools,
//! provider-side history, integrations or undisclosed protocol fallbacks.
use super::{
    super::*,
    stream::{self, Fragment},
};
use serde_json::{Map, Value, json};

pub fn installed(protocol: &str) -> bool {
    matches!(
        protocol,
        "openai_responses" | "gemini_generate_content" | "ollama_native" | "lmstudio_native_v1"
    )
}

fn model(plan: &RequestPlan) -> AppResult<&str> {
    plan.model
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .ok_or_else(|| failure("missing_model", "请填写实际模型ID"))
}

fn parameters(plan: &RequestPlan) -> AppResult<Map<String, Value>> {
    Ok(plan_generation_parameters(plan)?.into_iter().collect())
}

fn system(plan: &RequestPlan) -> String {
    plan.messages
        .iter()
        .filter(|m| m["role"] == "system")
        .filter_map(|m| m["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn input(plan: &RequestPlan) -> AppResult<String> {
    let mut parts = Vec::new();
    for m in plan.messages.iter().filter(|m| m["role"] != "system") {
        if m["role"] != "user" {
            return Err(failure(
                "unsupported_message_role",
                "该原生请求不支持隐式对话历史",
            ));
        }
        parts.push(
            m["content"]
                .as_str()
                .ok_or_else(|| failure("invalid_message", "消息正文必须为文本"))?,
        );
    }
    Ok(parts.join("\n\n"))
}

fn gemini_text(value: &Value) -> AppResult<(String, bool)> {
    if value.get("error").is_some() || value.pointer("/promptFeedback/blockReason").is_some() {
        return Err(failure("model_error", "Gemini拒绝或无法处理该请求"));
    }
    let c = value
        .pointer("/candidates/0")
        .ok_or_else(|| failure("malformed_response", "Gemini未返回候选正文"))?;
    let finish = c.get("finishReason").and_then(Value::as_str);
    if finish.is_some_and(|f| !matches!(f, "STOP" | "FINISH_REASON_UNSPECIFIED")) {
        return Err(failure("output_incomplete", "模型正文被截断或拒绝"));
    }
    let mut text = String::new();
    if let Some(parts) = c.pointer("/content/parts").and_then(Value::as_array) {
        for part in parts {
            if part.get("functionCall").is_some() {
                return Err(failure("output_incomplete", "不执行模型工具调用"));
            }
            if part.get("thought").and_then(Value::as_bool) != Some(true)
                && let Some(t) = part.get("text").and_then(Value::as_str) {
                    text.push_str(t);
                }
        }
    }
    Ok((text, finish == Some("STOP")))
}

fn response_text(value: &Value) -> AppResult<String> {
    if value["status"] != "completed"
        || value.get("error").is_some_and(|v| !v.is_null())
        || value
            .get("incomplete_details")
            .is_some_and(|v| !v.is_null())
    {
        return Err(failure("output_incomplete", "Responses未完整结束"));
    }
    let items = value["output"]
        .as_array()
        .ok_or_else(|| failure("malformed_response", "Responses缺少输出"))?;
    let mut text = String::new();
    for item in items {
        match item["type"].as_str() {
            Some("reasoning") => continue,
            Some("message") => {
                let parts = item["content"]
                    .as_array()
                    .ok_or_else(|| failure("malformed_response", "Responses消息无效"))?;
                for part in parts {
                    if part["type"] == "refusal" {
                        return Err(failure("output_incomplete", "模型拒绝生成正文"));
                    }
                    if part["type"] != "output_text" {
                        return Err(failure("unsupported_output", "模型返回未支持的内容"));
                    }
                    text.push_str(
                        part["text"]
                            .as_str()
                            .ok_or_else(|| failure("malformed_response", "Responses正文无效"))?,
                    );
                }
            }
            _ => return Err(failure("output_incomplete", "不执行模型工具或外部操作")),
        }
    }
    Ok(text)
}

fn lm_text(value: &Value) -> AppResult<String> {
    if value.get("error").is_some() {
        return Err(failure("model_error", "LM Studio返回错误"));
    }
    let items = value["output"]
        .as_array()
        .ok_or_else(|| failure("malformed_response", "LM Studio缺少输出"))?;
    let mut text = String::new();
    for item in items {
        match item["type"].as_str() {
            Some("reasoning") => {}
            Some("message") => text.push_str(
                item["content"]
                    .as_str()
                    .ok_or_else(|| failure("malformed_response", "LM Studio消息无效"))?,
            ),
            _ => return Err(failure("output_incomplete", "不执行模型工具或外部操作")),
        }
    }
    Ok(text)
}

fn ollama_fragment(value: &Value) -> AppResult<Fragment> {
    if value.get("error").is_some() {
        return Err(failure("model_error", "Ollama返回错误"));
    }
    if value
        .pointer("/message/tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        return Err(failure("output_incomplete", "不执行模型工具调用"));
    }
    let text = value
        .pointer("/message/content")
        .and_then(Value::as_str)
        .unwrap_or("");
    if value["done"] == true {
        if value
            .get("done_reason")
            .and_then(Value::as_str)
            .is_some_and(|r| r != "stop")
        {
            return Err(failure("output_incomplete", "Ollama正文未完整结束"));
        }
        // Terminal content is a delta too. The caller handles it before the terminal.
        Ok(Fragment::LastText(text.into()))
    } else {
        Ok(Fragment::Text(text.into()))
    }
}

fn prepare(plan: &RequestPlan, connection: &Connection) -> AppResult<(reqwest::Url, Value)> {
    if !installed(&plan.protocol) || plan.protocol != connection.protocol {
        return Err(failure("protocol_mismatch", "请求与连接协议不一致"));
    }
    if !matches!(connection.auth_mode.as_str(), "none" | "api_key") {
        return Err(failure(
            "unsupported_authentication",
            "当前原生模型协议认证类型不受支持",
        ));
    }
    if connection.endpoint.as_deref() != Some(plan.endpoint.as_str()) {
        return Err(failure("protocol_mismatch", "请求地址与已选连接不一致"));
    }
    let opts = parameters(plan)?;
    let mut body = Map::new();
    body.insert("model".into(), json!(model(plan)?));
    body.insert("stream".into(), json!(plan.stream));
    let route = match plan.protocol.as_str() {
        "openai_responses" => {
            body.extend(opts);
            body.insert("input".into(), json!(plan.messages));
            body.insert("store".into(), json!(false));
            if plan.structured {
                body.insert("text".into(), json!({"format":{"type":"json_object"}}));
            }
            "responses".to_owned()
        }
        "ollama_native" => {
            body.insert("messages".into(), json!(plan.messages));
            body.insert("options".into(), Value::Object(opts));
            if plan.structured {
                body.insert("format".into(), json!("json"));
            }
            "api/chat".to_owned()
        }
        "lmstudio_native_v1" => {
            if plan.structured {
                return Err(failure(
                    "unsupported_structured_output",
                    "该原生接口尚不支持此结构化输出合同",
                ));
            }
            body.extend(opts);
            body.insert("input".into(), json!(input(plan)?));
            body.insert("system_prompt".into(), json!(system(plan)));
            body.insert("store".into(), json!(false));
            body.insert("integrations".into(), json!([]));
            "api/v1/chat".to_owned()
        }
        "gemini_generate_content" => {
            body.remove("model");
            body.remove("stream");
            let mut contents = Vec::new();
            for m in plan.messages.iter().filter(|m| m["role"] != "system") {
                let role = match m["role"].as_str() {
                    Some("user") => "user",
                    Some("assistant") => "model",
                    _ => return Err(failure("unsupported_message_role", "Gemini消息角色无效")),
                };
                let text = m["content"]
                    .as_str()
                    .ok_or_else(|| failure("invalid_message", "消息正文必须为文本"))?;
                contents.push(json!({"role":role,"parts":[{"text":text}]}));
            }
            body.insert("contents".into(), json!(contents));
            if !system(plan).is_empty() {
                body.insert(
                    "systemInstruction".into(),
                    json!({"parts":[{"text":system(plan)}]}),
                );
            }
            let mut config = opts;
            if plan.structured {
                config.insert("responseMimeType".into(), json!("application/json"));
            }
            body.insert("generationConfig".into(), Value::Object(config));
            let id = model(plan)?.strip_prefix("models/").unwrap_or(model(plan)?);
            if id.is_empty()
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            {
                return Err(failure("invalid_model", "Gemini模型ID无效"));
            }
            format!(
                "models/{id}:{}",
                if plan.stream {
                    "streamGenerateContent"
                } else {
                    "generateContent"
                }
            )
        }
        _ => return Err(failure("unsupported_protocol", "未安装原生协议")),
    };
    let mut url = endpoint(&plan.endpoint, &route)?;
    if plan.protocol == "gemini_generate_content" && plan.stream {
        url.query_pairs_mut().append_pair("alt", "sse");
    }
    Ok((url, Value::Object(body)))
}

/// Pure request validation for the application gateway before reading Vault.
pub fn preflight(plan: &RequestPlan, connection: &Connection) -> AppResult<()> {
    prepare(plan, connection).map(|_| ())
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
pub(crate) async fn execute_with_headers(
    plan: &RequestPlan,
    connection: &Connection,
    key: Option<&str>,
    headers: &reqwest::header::HeaderMap,
    on_delta: impl Fn(&str),
) -> AppResult<ModelOutput> {
    let (url, body) = prepare(plan, connection)?;
    if connection.auth_mode == "api_key" && key.is_none_or(str::is_empty) {
        return Err(failure("missing_credential", "连接凭据不可用"));
    }
    let client = crate::providers::client_with_headers(connection, plan.timeout_ms, headers)?;
    let mut request = client.post(url).json(&body);
    if let Some(key) = key {
        request = if plan.protocol == "gemini_generate_content" {
            request.header("x-goog-api-key", key)
        } else {
            request.bearer_auth(key)
        };
    }
    let response = checked(request.send().await.map_err(network_error)?).await?;
    let text = if !plan.stream {
        let value = bounded_json(response).await?;
        let text = match plan.protocol.as_str() {
            "openai_responses" => response_text(&value)?,
            "gemini_generate_content" => {
                let (text, done) = gemini_text(&value)?;
                if !done {
                    return Err(failure("output_incomplete", "Gemini未完整结束"));
                }
                text
            }
            "lmstudio_native_v1" => lm_text(&value)?,
            "ollama_native" => match ollama_fragment(&value)? {
                Fragment::LastText(t) => t,
                _ => return Err(failure("output_incomplete", "Ollama未完整结束")),
            },
            _ => unreachable!(),
        };
        validate_output_text(&text)?;
        if text.trim().is_empty() {
            return Err(failure("empty_response", "模型未返回正文"));
        }
        if text.len() > MAX_RESPONSE_BYTES {
            return Err(failure("response_too_large", "模型输出超过上限"));
        }
        on_delta(&text);
        text
    } else if plan.protocol == "ollama_native" {
        stream::ndjson(response, ollama_fragment, &on_delta).await?
    } else {
        stream::sse(
            response,
            |event, v| {
                let kind = v["type"].as_str().unwrap_or(event);
                match plan.protocol.as_str() {
                    "openai_responses" => match kind {
                        "response.output_text.delta" => Ok(Fragment::Text(
                            v["delta"]
                                .as_str()
                                .ok_or_else(|| failure("malformed_stream", "Responses分片无效"))?
                                .into(),
                        )),
                        "response.completed" => {
                            Ok(Fragment::Final(Some(response_text(&v["response"])?)))
                        }
                        "error"
                        | "response.failed"
                        | "response.incomplete"
                        | "response.refusal.delta"
                        | "response.function_call_arguments.delta" => Err(failure(
                            "output_incomplete",
                            "Responses正文不完整或请求外部操作",
                        )),
                        _ => Ok(Fragment::Ignore),
                    },
                    "lmstudio_native_v1" => match kind {
                        "message.delta" => Ok(Fragment::Text(
                            v["content"]
                                .as_str()
                                .ok_or_else(|| failure("malformed_stream", "LM Studio分片无效"))?
                                .into(),
                        )),
                        "chat.end" => Ok(Fragment::Final(Some(lm_text(&v["result"])?))),
                        "error"
                        | "tool_call.start"
                        | "tool_call.arguments"
                        | "tool_call.success"
                        | "tool_call.failure" => Err(failure(
                            "output_incomplete",
                            "LM Studio正文不完整或请求工具",
                        )),
                        _ => Ok(Fragment::Ignore),
                    },
                    "gemini_generate_content" => {
                        let (t, done) = gemini_text(v)?;
                        if done {
                            Ok(Fragment::LastText(t))
                        } else {
                            Ok(Fragment::Text(t))
                        }
                    }
                    _ => Err(failure("unsupported_protocol", "未安装SSE协议")),
                }
            },
            &on_delta,
        )
        .await?
    };
    Ok(ModelOutput { text })
}
