//! Actual loopback HTTP wire tests. These do not claim live-provider acceptance.
use app_core::{application::request_plan::RequestPlan, contracts::Connection};
use app_infra::providers;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

async fn server(
    body: String,
    content_type: &str,
    status: u16,
) -> (String, oneshot::Receiver<(String, Value)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (send, receive) = oneshot::channel();
    let content_type = content_type.to_owned();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0; 1024];
        let end = loop {
            let n = socket.read(&mut buf).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                break p + 4;
            }
            assert!(bytes.len() < 65536);
        };
        let head = String::from_utf8(bytes[..end].to_vec()).unwrap();
        let length = head
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .map(|(_, v)| v.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        while bytes.len() < end + length {
            let n = socket.read(&mut buf).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
        }
        let request: Value = serde_json::from_slice(&bytes[end..end + length]).unwrap();
        // Body-decoding tests keep the receiver. Error/stream tests deliberately
        // drop it and still require the real listener to return its response.
        let _ = send.send((head, request));
        socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
        // Split in the middle of UTF-8, JSON tokens and SSE delimiters.
        for chunk in body.as_bytes().chunks(2) {
            if socket.write_all(chunk).await.is_err() {
                return;
            }
            tokio::task::yield_now().await;
        }
        let _ = socket.shutdown().await;
    });
    (format!("http://{address}"), receive)
}
fn connection(endpoint: &str, protocol: &str) -> Connection {
    Connection {
        adapter_binding: None,
        preset_source: None,
        connection_id: "wire".into(),
        display_name: "Isolated test".into(),
        protocol: protocol.into(),
        deployment: "local".into(),
        auth_mode: "api_key".into(),
        endpoint: Some(endpoint.into()),
        credential_pool_id: None,
        credential_pool_revision: None,
        proxy_mode: "disabled".into(),
        network_metadata: BTreeMap::new(),
        availability_intent: true,
    }
}
fn plan(endpoint: &str, protocol: &str, stream: bool) -> RequestPlan {
    RequestPlan {
        adapter_binding: None,
        purpose: None,
        protocol: protocol.into(),
        endpoint: endpoint.into(),
        model: Some("fixture-model".into()),
        source_text: "雨がやんだ。".into(),
        source_language: "ja".into(),
        target_language: "zh-CN".into(),
        messages: vec![
            json!({"role":"system","content":"Translate."}),
            json!({"role":"user","content":"雨がやんだ。"}),
        ],
        parameters: BTreeMap::from([("temperature".into(), json!(0))]),
        structured: false,
        stream,
        timeout_ms: 3000,
    }
}
fn response(text: &str) -> Value {
    json!({"status":"completed","error":null,"incomplete_details":null,"output":[{"type":"message","content":[{"type":"output_text","text":text}]}]})
}
fn sse(kind: &str, v: Value) -> String {
    format!("event: {kind}\ndata: {v}\n\n")
}

#[tokio::test]
async fn native_json_requests_use_distinct_paths_bodies_and_authentication() {
    let cases = [
        ("openai_responses", "/v1/responses", response("雨停了。")),
        (
            "gemini_generate_content",
            "/v1beta/models/fixture-model:generateContent",
            json!({"candidates":[{"content":{"parts":[{"text":"雨停了。"}]},"finishReason":"STOP"}]}),
        ),
        (
            "ollama_native",
            "/api/chat",
            json!({"message":{"content":"雨停了。"},"done":true,"done_reason":"stop"}),
        ),
        (
            "lmstudio_native_v1",
            "/api/v1/chat",
            json!({"output":[{"type":"reasoning","content":"hidden"},{"type":"message","content":"雨停了。"}]}),
        ),
    ];
    for (protocol, route, body) in cases {
        let (base, rx) = server(body.to_string(), "application/json", 200).await;
        let base = match protocol {
            "openai_responses" => format!("{base}/v1"),
            "gemini_generate_content" => format!("{base}/v1beta"),
            _ => base,
        };
        let p = plan(&base, protocol, false);
        let c = connection(&base, protocol);
        assert_eq!(
            providers::execute(&p, &c, Some("wire-test-key"), |_| {})
                .await
                .unwrap()
                .text,
            "雨停了。"
        );
        let (head, body) = rx.await.unwrap();
        assert!(
            head.starts_with(&format!("POST {route} HTTP/1.1")),
            "{head}"
        );
        assert!(!head.contains("?key="));
        assert!(!body.to_string().contains("wire-test-key"));
        if protocol == "gemini_generate_content" {
            assert!(
                head.to_lowercase()
                    .contains("x-goog-api-key: wire-test-key")
            );
            assert_eq!(body["generationConfig"]["temperature"], 0);
            assert!(body.get("messages").is_none());
        } else {
            assert!(
                head.to_lowercase()
                    .contains("authorization: bearer wire-test-key")
            );
        }
        if protocol == "openai_responses" {
            assert_eq!(body["store"], false);
            assert!(body.get("input").is_some());
            assert!(body.get("messages").is_none());
        }
        if protocol == "lmstudio_native_v1" {
            assert_eq!(body["store"], false);
            assert_eq!(body["integrations"], json!([]));
            assert_eq!(body["input"], "雨がやんだ。");
        }
        if protocol == "ollama_native" {
            assert_eq!(body["options"]["temperature"], 0);
        }
    }
}

#[tokio::test]
async fn approved_native_defaults_and_custom_prefixes_never_duplicate_api_paths() {
    let catalog = providers::builtin_registry().unwrap().catalog();
    for (preset_id, protocol, route, body) in [
        (
            "builtin.ollama-native",
            "ollama_native",
            "api/chat",
            json!({"message":{"content":"正文"},"done":true,"done_reason":"stop"}),
        ),
        (
            "builtin.lmstudio-native",
            "lmstudio_native_v1",
            "api/v1/chat",
            json!({"output":[{"type":"message","content":"正文"}]}),
        ),
    ] {
        let preset = catalog
            .presets
            .iter()
            .find(|p| p.key.preset_id == preset_id)
            .unwrap();
        let original = reqwest::Url::parse(preset.default_endpoint.as_deref().unwrap()).unwrap();
        for suffix in [
            original.path().to_owned(),
            format!("/proxy{}", original.path()),
            format!("/{route}"),
        ] {
            let (base, rx) = server(body.to_string(), "application/json", 200).await;
            let base = format!("{base}{suffix}");
            assert_eq!(
                providers::execute(
                    &plan(&base, protocol, false),
                    &connection(&base, protocol),
                    Some("key"),
                    |_| {}
                )
                .await
                .unwrap()
                .text,
                "正文"
            );
            let expected = if suffix.starts_with("/proxy") {
                format!("/proxy/{route}")
            } else {
                format!("/{route}")
            };
            assert!(
                rx.await
                    .unwrap()
                    .0
                    .starts_with(&format!("POST {expected} HTTP/1.1"))
            );
        }
    }
    assert_eq!(
        providers::endpoint("http://localhost/notapi", "api/chat")
            .unwrap()
            .path(),
        "/notapi/api/chat"
    );
}

#[tokio::test]
async fn responses_sse_requires_completed_and_excludes_reasoning() {
    let body = sse("response.created", json!({"type":"response.created"}))
        + &sse(
            "response.reasoning_text.delta",
            json!({"type":"response.reasoning_text.delta","delta":"not正文"}),
        )
        + &sse(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","delta":"雨"}),
        )
        + &sse(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","delta":"停了。"}),
        )
        + &sse(
            "response.completed",
            json!({"type":"response.completed","response":response("雨停了。")}),
        );
    let (base, _) = server(body, "text/event-stream; charset=utf-8", 200).await;
    let updates = Arc::new(Mutex::new(Vec::new()));
    let captured = updates.clone();
    let value = providers::execute(
        &plan(&base, "openai_responses", true),
        &connection(&base, "openai_responses"),
        Some("key"),
        move |t| captured.lock().unwrap().push(t.to_owned()),
    )
    .await
    .unwrap();
    assert_eq!(value.text, "雨停了。");
    assert_eq!(*updates.lock().unwrap(), vec!["雨", "雨停了。"]);
}

#[tokio::test]
async fn ollama_ndjson_preserves_last_delta_with_or_without_final_newline() {
    for newline in ["", "\n"] {
        let body = format!(
            "{}\n{}{}",
            json!({"message":{"content":"雨"},"done":false}),
            json!({"message":{"content":"停了。"},"done":true,"done_reason":"stop"}),
            newline
        );
        let (base, _) = server(body, "application/x-ndjson", 200).await;
        let v = providers::execute(
            &plan(&base, "ollama_native", true),
            &connection(&base, "ollama_native"),
            Some("key"),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(v.text, "雨停了。");
    }
}

#[tokio::test]
async fn gemini_sse_preserves_final_chunk_and_explicit_stop() {
    let body = sse(
        "message",
        json!({"candidates":[{"content":{"parts":[{"text":"雨"},{"text":"thinking","thought":true}]}}]}),
    ) + &sse(
        "message",
        json!({"candidates":[{"content":{"parts":[{"text":"停了。"}]},"finishReason":"STOP"}]}),
    );
    let (base, rx) = server(body, "text/event-stream", 200).await;
    assert_eq!(
        providers::execute(
            &plan(&base, "gemini_generate_content", true),
            &connection(&base, "gemini_generate_content"),
            Some("key"),
            |_| {}
        )
        .await
        .unwrap()
        .text,
        "雨停了。"
    );
    assert!(
        rx.await
            .unwrap()
            .0
            .starts_with("POST /models/fixture-model:streamGenerateContent?alt=sse ")
    );
}

#[tokio::test]
async fn lmstudio_stream_checks_aggregate_and_never_executes_tools() {
    let body = sse(
        "message.delta",
        json!({"type":"message.delta","content":"雨停了。"}),
    ) + &sse(
        "chat.end",
        json!({"type":"chat.end","result":{"output":[{"type":"message","content":"雨停了。"}]}}),
    );
    let (base, _) = server(body, "text/event-stream", 200).await;
    assert_eq!(
        providers::execute(
            &plan(&base, "lmstudio_native_v1", true),
            &connection(&base, "lmstudio_native_v1"),
            Some("key"),
            |_| {}
        )
        .await
        .unwrap()
        .text,
        "雨停了。"
    );
    let body = sse(
        "tool_call.start",
        json!({"type":"tool_call.start","tool":"untrusted"}),
    );
    let (base, _) = server(body, "text/event-stream", 200).await;
    assert_eq!(
        providers::execute(
            &plan(&base, "lmstudio_native_v1", true),
            &connection(&base, "lmstudio_native_v1"),
            Some("key"),
            |_| {}
        )
        .await
        .unwrap_err()
        .code,
        "output_incomplete"
    );
}

#[tokio::test]
async fn missing_terminal_truncation_final_mismatch_and_refusal_fail() {
    let cases = [
        (
            "openai_responses",
            sse(
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","delta":"partial"}),
            ),
            "stream_interrupted",
        ),
        (
            "openai_responses",
            sse("response.incomplete", json!({"type":"response.incomplete"})),
            "output_incomplete",
        ),
        (
            "openai_responses",
            sse(
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","delta":"wrong"}),
            ) + &sse(
                "response.completed",
                json!({"type":"response.completed","response":response("different")}),
            ),
            "stream_final_mismatch",
        ),
        (
            "gemini_generate_content",
            sse(
                "message",
                json!({"candidates":[{"content":{"parts":[{"text":"partial"}]},"finishReason":"MAX_TOKENS"}]}),
            ),
            "output_incomplete",
        ),
        (
            "lmstudio_native_v1",
            sse(
                "error",
                json!({"type":"error","error":{"message":"secret body"}}),
            ),
            "model_error",
        ),
    ];
    for (protocol, body, code) in cases {
        let (base, _) = server(body, "text/event-stream", 200).await;
        let err = providers::execute(
            &plan(&base, protocol, true),
            &connection(&base, protocol),
            Some("key"),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, code, "{protocol}");
        assert!(!err.message.contains("secret body"));
    }
}

#[tokio::test]
async fn unknown_parameters_and_protocol_mismatch_fail_before_network() {
    let base = "http://127.0.0.1:1";
    for protocol in [
        "openai_responses",
        "gemini_generate_content",
        "ollama_native",
        "lmstudio_native_v1",
    ] {
        let mut p = plan(base, protocol, false);
        p.parameters.insert("tools".into(), json!([]));
        assert_eq!(
            providers::execute(&p, &connection(base, protocol), Some("key"), |_| {})
                .await
                .unwrap_err()
                .code,
            "unsupported_parameter"
        );
        p.parameters.clear();
        let mut c = connection(base, protocol);
        c.protocol = "anthropic".into();
        assert_eq!(
            providers::execute(&p, &c, Some("key"), |_| {})
                .await
                .unwrap_err()
                .code,
            "protocol_mismatch"
        );
    }
}

#[tokio::test]
async fn native_numeric_domains_reject_invalid_values_without_clamping_or_network() {
    let base = "http://127.0.0.1:1";
    for protocol in [
        "openai_responses",
        "gemini_generate_content",
        "ollama_native",
        "lmstudio_native_v1",
    ] {
        for (field, bad) in [
            ("max_tokens", json!(-1)),
            ("max_tokens", json!(0.5)),
            ("max_tokens", json!(0)),
            ("max_tokens", json!(u64::MAX)),
            ("temperature", json!(-0.01)),
            ("top_p", json!(1.01)),
            ("top_p", json!(-1)),
            ("temperature", json!(1e100)),
        ] {
            let mut p = plan(base, protocol, false);
            p.parameters = BTreeMap::from([(field.into(), bad)]);
            assert_eq!(
                providers::execute(&p, &connection(base, protocol), Some("key"), |_| {})
                    .await
                    .unwrap_err()
                    .code,
                "invalid_parameter",
                "{protocol}/{field}"
            );
        }
    }
    for (protocol, field, value) in [
        ("lmstudio_native_v1", "temperature", json!(1.5)),
        ("gemini_generate_content", "temperature", json!(2.1)),
        ("openai_responses", "temperature", json!(2.1)),
        ("ollama_native", "top_k", json!(-1)),
        ("lmstudio_native_v1", "top_k", json!(1.5)),
        ("lmstudio_native_v1", "context_length", json!(0)),
    ] {
        let mut p = plan(base, protocol, false);
        p.parameters = BTreeMap::from([(field.into(), value)]);
        assert_eq!(
            providers::execute(&p, &connection(base, protocol), Some("key"), |_| {})
                .await
                .unwrap_err()
                .code,
            "invalid_parameter"
        );
    }
    let mut p = plan(base, "lmstudio_native_v1", false);
    p.parameters.insert("max_tokens".into(), json!(5));
    p.parameters.insert("max_output_tokens".into(), json!(10));
    assert_eq!(
        providers::execute(
            &p,
            &connection(base, "lmstudio_native_v1"),
            Some("key"),
            |_| {}
        )
        .await
        .unwrap_err()
        .code,
        "conflicting_parameter"
    );
}

#[tokio::test]
async fn authentication_errors_are_safe_and_raw_payload_is_not_echoed() {
    let (base, _) = server(
        json!({"error":{"message":"key-DO-NOT-LEAK 雨がやんだ。"}}).to_string(),
        "application/json",
        401,
    )
    .await;
    let e = providers::execute(
        &plan(&base, "openai_responses", false),
        &connection(&base, "openai_responses"),
        Some("key-DO-NOT-LEAK"),
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "authentication_failed");
    assert!(!e.message.contains("DO-NOT-LEAK"));
    assert!(!e.message.contains("雨が"));
}

#[tokio::test]
async fn nul_in_native_json_is_rejected_before_any_observer() {
    for (protocol, value) in [
        ("openai_responses", response("正文\0尾部")),
        (
            "gemini_generate_content",
            json!({"candidates":[{"content":{"parts":[{"text":"正文\0尾部"}]},"finishReason":"STOP"}]}),
        ),
        (
            "ollama_native",
            json!({"message":{"content":"正文\0尾部"},"done":true,"done_reason":"stop"}),
        ),
        (
            "lmstudio_native_v1",
            json!({"output":[{"type":"message","content":"正文\0尾部"}]}),
        ),
    ] {
        let (base, _) = server(value.to_string(), "application/json", 200).await;
        let updates = Arc::new(Mutex::new(Vec::new()));
        let captured = updates.clone();
        let error = providers::execute(
            &plan(&base, protocol, false),
            &connection(&base, protocol),
            Some("key"),
            move |text| captured.lock().unwrap().push(text.to_owned()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "invalid_output", "{protocol}");
        assert!(
            updates.lock().unwrap().is_empty(),
            "invalid JSON text reached observer for {protocol}"
        );
    }
}

#[tokio::test]
async fn nul_deltas_last_deltas_and_final_aggregates_never_reach_observers() {
    let gemini = |text: &str, done: bool| json!({"candidates":[{"content":{"parts":[{"text":text}]},"finishReason": if done {"STOP"} else {"FINISH_REASON_UNSPECIFIED"}}]});
    let cases = [
        (
            "openai_responses",
            "text/event-stream",
            sse(
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","delta":"正文\0尾部"}),
            ),
        ),
        (
            "openai_responses",
            "text/event-stream",
            sse(
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","delta":"合法前文\n"}),
            ) + &sse(
                "response.completed",
                json!({"type":"response.completed","response":response("合法前文\n\0尾部")}),
            ),
        ),
        (
            "gemini_generate_content",
            "text/event-stream",
            format!("data: {}\n\n", gemini("正文\0尾部", false)),
        ),
        (
            "gemini_generate_content",
            "text/event-stream",
            format!("data: {}\n\n", gemini("正文\0尾部", true)),
        ),
        (
            "ollama_native",
            "application/x-ndjson",
            json!({"message":{"content":"正文\0尾部"},"done":false}).to_string() + "\n",
        ),
        (
            "ollama_native",
            "application/x-ndjson",
            json!({"message":{"content":"正文\0尾部"},"done":true,"done_reason":"stop"})
                .to_string()
                + "\n",
        ),
        (
            "lmstudio_native_v1",
            "text/event-stream",
            sse(
                "message.delta",
                json!({"type":"message.delta","content":"正文\0尾部"}),
            ),
        ),
        (
            "lmstudio_native_v1",
            "text/event-stream",
            sse(
                "chat.end",
                json!({"type":"chat.end","result":{"output":[{"type":"message","content":"正文\0尾部"}]}}),
            ),
        ),
    ];
    for (protocol, content_type, body) in cases {
        let (base, _) = server(body, content_type, 200).await;
        let updates = Arc::new(Mutex::new(Vec::new()));
        let captured = updates.clone();
        let error = providers::execute(
            &plan(&base, protocol, true),
            &connection(&base, protocol),
            Some("key"),
            move |text| captured.lock().unwrap().push(text.to_owned()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "invalid_output", "{protocol}");
        assert!(
            updates
                .lock()
                .unwrap()
                .iter()
                .all(|text| !text.contains('\0')),
            "NUL fragment reached observer for {protocol}"
        );
    }
}
