//! Protocol integration against an isolated loopback TCP server. These fixtures are not live model evidence.
use app_core::{application::request_plan::RequestPlan, contracts::*};
use app_infra::providers;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

#[derive(Debug)]
struct Request {
    head: String,
    body: Value,
}
async fn request(socket: &mut TcpStream) -> Request {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end = loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0, "request ended before headers");
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        assert!(bytes.len() < 65536);
    };
    let head = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let length = head
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while bytes.len() < header_end + length {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
    }
    Request {
        head,
        body: serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap(),
    }
}
async fn fixture(
    status: u16,
    content_type: &str,
    body: &str,
    fragmented: bool,
) -> (
    String,
    oneshot::Receiver<Request>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let body = body.as_bytes().to_vec();
    let content_type = content_type.to_owned();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let captured = request(&mut socket).await;
        tx.send(captured).unwrap();
        let head = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        if fragmented {
            for chunk in body.chunks(2) {
                socket.write_all(chunk).await.unwrap();
                tokio::task::yield_now().await;
            }
        } else {
            socket.write_all(&body).await.unwrap();
        }
        socket.shutdown().await.unwrap();
    });
    (format!("http://{address}/v1"), rx, handle)
}
fn connection(endpoint: &str, protocol: &str) -> Connection {
    Connection {
        connection_id: "protocol-fixture".into(),
        display_name: "Isolated fixture".into(),
        protocol: protocol.into(),
        deployment: "local".into(),
        auth_mode: "none".into(),
        endpoint: Some(endpoint.into()),
        credential_pool_id: None,
        credential_pool_revision: None,
        proxy_mode: "disabled".into(),
        network_metadata: BTreeMap::new(),
        availability_intent: true,
        adapter_binding: None,
        preset_source: None,
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
            json!({"role":"system","content":"Translate into Chinese."}),
            json!({"role":"user","content":"雨がやんだ。"}),
        ],
        parameters: BTreeMap::from([("temperature".into(), json!(0))]),
        structured: false,
        stream,
        timeout_ms: 2000,
    }
}

#[tokio::test]
async fn openai_sse_fragmentation_preserves_utf8_and_cumulative_deltas() {
    let body = ": heartbeat\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"雨\"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"停了。\"},\"finish_reason\":\"stop\"}]}\r\n\r\ndata: [DONE]\r\n\r\n";
    let (endpoint, rx, server) = fixture(200, "text/event-stream; charset=utf-8", body, true).await;
    let partials = Mutex::new(Vec::new());
    let output = providers::execute(
        &plan(&endpoint, "openai_compatible", true),
        &connection(&endpoint, "openai_compatible"),
        Some("fixture-secret"),
        |text| partials.lock().unwrap().push(text.to_owned()),
    )
    .await
    .unwrap();
    assert_eq!(output.text, "雨停了。");
    assert_eq!(*partials.lock().unwrap(), ["雨", "雨停了。"]);
    let sent = rx.await.unwrap();
    assert!(sent.head.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(
        sent.head
            .to_lowercase()
            .contains("authorization: bearer fixture-secret")
    );
    assert_eq!(sent.body["temperature"], 0);
    assert_eq!(sent.body["stream"], true);
    assert_eq!(sent.body["model"], "fixture-model");
    assert_eq!(sent.body["messages"][1]["content"], "雨がやんだ。");
    server.await.unwrap();
}
#[tokio::test]
async fn nonstream_json_is_supported_when_streaming_is_disabled() {
    let (endpoint, rx, server) = fixture(
        200,
        "application/json",
        r#"{"choices":[{"message":{"content":"雨停了。"},"finish_reason":"stop"}]}"#,
        true,
    )
    .await;
    let output = providers::execute(
        &plan(&endpoint, "openai_compatible", false),
        &connection(&endpoint, "openai_compatible"),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(output.text, "雨停了。");
    assert_eq!(rx.await.unwrap().body["stream"], false);
    server.await.unwrap();
}
#[tokio::test]
async fn incomplete_or_rejected_model_output_never_becomes_success() {
    for (body, expected) in [
        (
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
            "stream_incomplete",
        ),
        (
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n\n",
            "output_incomplete",
        ),
        ("data: {broken}\n\ndata: [DONE]\n\n", "malformed_stream"),
    ] {
        let (endpoint, rx, server) = fixture(200, "text/event-stream", body, false).await;
        let result = providers::execute(
            &plan(&endpoint, "openai_compatible", true),
            &connection(&endpoint, "openai_compatible"),
            None,
            |_| {},
        )
        .await;
        assert_eq!(result.unwrap_err().code, expected);
        rx.await.unwrap();
        server.await.unwrap();
    }
}
#[tokio::test]
async fn service_errors_are_classified_without_echoing_sensitive_body() {
    for (status, expected) in [
        (401, "authentication_failed"),
        (429, "rate_limited"),
        (503, "service_unavailable"),
    ] {
        let (endpoint, rx, server) = fixture(
            status,
            "application/json",
            r#"{"error":"secret-key-EXPOSE source-private-EXPOSE"}"#,
            false,
        )
        .await;
        let result = providers::execute(
            &plan(&endpoint, "openai_compatible", true),
            &connection(&endpoint, "openai_compatible"),
            None,
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(result.code, expected);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("EXPOSE"));
        assert!(!serialized.contains(&endpoint));
        rx.await.unwrap();
        server.await.unwrap();
    }
}
#[tokio::test]
async fn actual_http_timeout_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap()
    });
    let mut p = plan(&endpoint, "openai_compatible", true);
    p.timeout_ms = 40;
    let error = providers::execute(
        &p,
        &connection(&endpoint, "openai_compatible"),
        None,
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "timeout");
    assert_eq!(
        server.await.unwrap(),
        0,
        "timeout must close the in-flight socket"
    );
}
#[tokio::test]
async fn actual_sse_body_timeout_preserves_timeout_classification_and_closes_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n").await.unwrap();
        // Keep the actual HTTP body open after a valid SSE event. This must
        // exercise a read timeout, rather than a timeout waiting for headers.
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(3), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap()
    });
    let partials = Mutex::new(Vec::new());
    let mut p = plan(&endpoint, "openai_compatible", true);
    p.timeout_ms = 250;
    let error = providers::execute(
        &p,
        &connection(&endpoint, "openai_compatible"),
        None,
        |text| partials.lock().unwrap().push(text.to_owned()),
    )
    .await
    .unwrap_err();
    assert_eq!(*partials.lock().unwrap(), ["partial"]);
    assert_eq!(error.code, "timeout");
    assert_eq!(error.stage, "provider");
    assert!(!serde_json::to_string(&error).unwrap().contains(&endpoint));
    assert_eq!(
        server.await.unwrap(),
        0,
        "body timeout must close the in-flight socket"
    );
}

#[tokio::test]
async fn truncated_http_sse_transport_is_not_reported_as_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n";
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        // A clean HTTP EOF without [DONE] is already tested as
        // stream_incomplete. Here Content-Length proves the transport itself
        // ended early even though a valid partial event reached the client.
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len() + 100
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        socket.write_all(body).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    let partials = Mutex::new(Vec::new());
    let error = providers::execute(
        &plan(&endpoint, "openai_compatible", true),
        &connection(&endpoint, "openai_compatible"),
        None,
        |text| partials.lock().unwrap().push(text.to_owned()),
    )
    .await
    .unwrap_err();
    assert_eq!(*partials.lock().unwrap(), ["partial"]);
    assert_eq!(error.code, "network_error");
    assert_eq!(error.stage, "provider");
    assert!(!serde_json::to_string(&error).unwrap().contains(&endpoint));
    server.await.unwrap();
}
#[tokio::test]
async fn cancelling_provider_future_closes_inflight_stream_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n").await.unwrap();
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(3), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap()
    });
    let (tx, rx) = oneshot::channel();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let client = tokio::spawn(async move {
        providers::execute(
            &plan(&endpoint, "openai_compatible", true),
            &connection(&endpoint, "openai_compatible"),
            None,
            move |_| {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            },
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), rx)
        .await
        .unwrap()
        .unwrap();
    client.abort();
    assert!(client.await.unwrap_err().is_cancelled());
    assert_eq!(
        server.await.unwrap(),
        0,
        "cancelled future must not retain live model stream"
    );
}
#[tokio::test]
async fn anthropic_request_and_event_protocol_remain_distinct_from_openai() {
    let body = "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"雨停了。\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let (endpoint, rx, server) = fixture(200, "text/event-stream", body, true).await;
    let output = providers::execute(
        &plan(&endpoint, "anthropic", true),
        &connection(&endpoint, "anthropic"),
        Some("anthropic-fixture-secret"),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(output.text, "雨停了。");
    let sent = rx.await.unwrap();
    let head = sent.head.to_lowercase();
    assert!(sent.head.starts_with("POST /v1/messages "));
    assert!(head.contains("x-api-key: anthropic-fixture-secret"));
    assert!(head.contains("anthropic-version: 2023-06-01"));
    assert!(!head.contains("authorization:"));
    assert_eq!(sent.body["system"], "Translate into Chinese.");
    assert_eq!(sent.body["messages"].as_array().unwrap().len(), 1);
    assert_eq!(sent.body["max_tokens"], 2048);
    server.await.unwrap();
}
#[tokio::test]
async fn azure_translator_uses_rest_parameters_and_subscription_headers() {
    let (endpoint, rx, server) = fixture(
        200,
        "application/json",
        r#"[{"translations":[{"text":"雨停了。","to":"zh-Hans"}]}]"#,
        true,
    )
    .await;
    let mut c = connection(&endpoint, "azure_translator");
    c.network_metadata
        .insert("region".into(), json!("eastasia"));
    let output = providers::execute(
        &plan(&endpoint, "azure_translator", false),
        &c,
        Some("azure-fixture-secret"),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(output.text, "雨停了。");
    let sent = rx.await.unwrap();
    let head = sent.head.to_lowercase();
    assert!(
        sent.head
            .starts_with("POST /v1/translate?api-version=3.0&to=zh-Hans&from=ja ")
    );
    assert!(head.contains("ocp-apim-subscription-key: azure-fixture-secret"));
    assert!(head.contains("ocp-apim-subscription-region: eastasia"));
    assert_eq!(sent.body, json!([{"Text":"雨がやんだ。"}]));
    server.await.unwrap();
}

#[tokio::test]
async fn legacy_protocols_reject_nul_before_emitting_any_observer_text() {
    let cases = [
        (
            "openai_compatible",
            false,
            "application/json",
            json!({"choices":[{"message":{"content":"正文\0尾部"},"finish_reason":"stop"}]})
                .to_string(),
        ),
        (
            "anthropic",
            false,
            "application/json",
            json!({"content":[{"type":"text","text":"正文\0尾部"}],"stop_reason":"end_turn"})
                .to_string(),
        ),
        (
            "azure_translator",
            false,
            "application/json",
            json!([{"translations":[{"text":"正文\0尾部"}]}]).to_string(),
        ),
        (
            "openai_compatible",
            true,
            "text/event-stream",
            format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{"content":"正文\0尾部"}}]})
            ),
        ),
        (
            "anthropic",
            true,
            "text/event-stream",
            format!(
                "event: content_block_delta\ndata: {}\n\n",
                json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"正文\0尾部"}})
            ),
        ),
    ];
    for (protocol, streaming, content_type, body) in cases {
        let (base, request, worker) = fixture(200, content_type, &body, false).await;
        let updates = Arc::new(Mutex::new(Vec::new()));
        let captured = updates.clone();
        let error = providers::execute(
            &plan(&base, protocol, streaming),
            &connection(&base, protocol),
            None,
            move |text| captured.lock().unwrap().push(text.to_owned()),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.code, "invalid_output",
            "{protocol} streaming={streaming}"
        );
        assert!(
            updates.lock().unwrap().is_empty(),
            "invalid output reached observer"
        );
        request.await.unwrap();
        worker.await.unwrap();
    }
}

#[tokio::test]
async fn azure_empty_output_is_not_a_successful_translation() {
    for text in ["", " \n\t"] {
        let body = json!([{"translations":[{"text":text}]}]).to_string();
        let (base, request, worker) = fixture(200, "application/json", &body, false).await;
        let updates = Arc::new(Mutex::new(Vec::new()));
        let captured = updates.clone();
        let error = providers::execute(
            &plan(&base, "azure_translator", false),
            &connection(&base, "azure_translator"),
            None,
            move |text| captured.lock().unwrap().push(text.to_owned()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "empty_response");
        assert!(updates.lock().unwrap().is_empty());
        request.await.unwrap();
        worker.await.unwrap();
    }
}
