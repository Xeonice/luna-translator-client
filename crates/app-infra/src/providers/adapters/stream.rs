use super::super::{MAX_RESPONSE_BYTES, failure, network_error, validate_output_text};
use app_core::contracts::AppResult;
use eventsource_stream::{EventStreamError, Eventsource};
use futures_util::StreamExt;
use serde_json::Value;

pub(super) enum Fragment {
    Ignore,
    Text(String),
    LastText(String),
    Final(Option<String>),
}

fn push(text: &mut String, delta: &str, on_delta: &impl Fn(&str)) -> AppResult<()> {
    validate_output_text(delta)?;
    if text.len().saturating_add(delta.len()) > MAX_RESPONSE_BYTES {
        return Err(failure("response_too_large", "模型输出超过上限"));
    }
    text.push_str(delta);
    on_delta(text);
    Ok(())
}

fn consume(fragment: Fragment, text: &mut String, on_delta: &impl Fn(&str)) -> AppResult<bool> {
    match fragment {
        Fragment::Ignore => Ok(false),
        Fragment::Text(delta) => {
            push(text, &delta, on_delta)?;
            Ok(false)
        }
        Fragment::LastText(delta) => {
            if !delta.is_empty() {
                push(text, &delta, on_delta)?;
            }
            Ok(true)
        }
        Fragment::Final(final_text) => {
            if let Some(final_text) = final_text {
                validate_output_text(&final_text)?;
                if !text.is_empty() && text != &final_text {
                    return Err(failure("stream_final_mismatch", "流式正文与最终正文不一致"));
                }
                if text.is_empty() {
                    push(text, &final_text, on_delta)?;
                }
            }
            Ok(true)
        }
    }
}

fn complete(text: String, terminal: bool) -> AppResult<String> {
    if !terminal {
        return Err(failure("stream_interrupted", "模型流未完整结束，请重试"));
    }
    if text.trim().is_empty() {
        return Err(failure("empty_response", "模型未返回正文"));
    }
    Ok(text)
}

pub(super) async fn sse(
    response: reqwest::Response,
    decode: impl Fn(&str, &Value) -> AppResult<Fragment>,
    on_delta: impl Fn(&str),
) -> AppResult<String> {
    if response
        .headers()
        .get("content-type")
        .and_then(|x| x.to_str().ok()).is_none_or(|x| x.split(';').next() != Some("text/event-stream"))
    {
        return Err(failure("stream_not_supported", "服务未返回该协议的SSE流"));
    }
    let mut received = 0usize;
    let bytes = response.bytes_stream().map(move |chunk| {
        let bytes = chunk.map_err(network_error)?;
        received = received.saturating_add(bytes.len());
        if received > MAX_RESPONSE_BYTES * 4 {
            return Err(failure("response_too_large", "模型流数据超过上限"));
        }
        Ok(bytes)
    });
    let mut events = bytes.eventsource();
    let mut text = String::new();
    let mut terminal = false;
    while let Some(event) = events.next().await {
        let event = event.map_err(|e| match e {
            EventStreamError::Transport(e) => e,
            _ => failure("malformed_stream", "模型流格式无效"),
        })?;
        let value: Value = serde_json::from_str(&event.data)
            .map_err(|_| failure("malformed_stream", "模型流包含无效JSON"))?;
        if value.get("error").is_some() || event.event == "error" {
            return Err(failure("model_error", "模型流返回错误"));
        }
        if consume(decode(&event.event, &value)?, &mut text, &on_delta)? {
            terminal = true;
            break;
        }
    }
    complete(text, terminal)
}

pub(super) async fn ndjson(
    mut response: reqwest::Response,
    decode: impl Fn(&Value) -> AppResult<Fragment>,
    on_delta: impl Fn(&str),
) -> AppResult<String> {
    let kind = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next());
    if !matches!(kind, Some("application/x-ndjson" | "application/ndjson")) {
        return Err(failure("stream_not_supported", "服务未返回NDJSON流"));
    }
    let mut pending = Vec::new();
    let mut received = 0usize;
    let mut text = String::new();
    let mut terminal = false;
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        received = received.saturating_add(chunk.len());
        if received > MAX_RESPONSE_BYTES * 4 {
            return Err(failure("response_too_large", "模型流数据超过上限"));
        }
        pending.extend_from_slice(&chunk);
        while let Some(end) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<_> = pending.drain(..=end).collect();
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let value = serde_json::from_slice(&line)
                .map_err(|_| failure("malformed_stream", "NDJSON消息无效"))?;
            if consume(decode(&value)?, &mut text, &on_delta)? {
                terminal = true;
                break;
            }
        }
        if terminal {
            break;
        }
        if pending.len() > MAX_RESPONSE_BYTES {
            return Err(failure("response_too_large", "单条模型流消息超过上限"));
        }
    }
    if !terminal && pending.iter().any(|b| !b.is_ascii_whitespace()) {
        let value = serde_json::from_slice(&pending)
            .map_err(|_| failure("malformed_stream", "NDJSON尾消息无效"))?;
        terminal = consume(decode(&value)?, &mut text, &on_delta)?;
    }
    complete(text, terminal)
}
