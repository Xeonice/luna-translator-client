//! Real loopback discovery only; these tests do not qualify a translation model.
use app_core::contracts::Connection;
use app_infra::providers::adapters::model_discovery;
use serde_json::json;
use std::collections::BTreeMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

async fn server(responses: Vec<(u16, String, String)>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let job = tokio::spawn(async move {
        let mut requests = vec![];
        for (status, body, extra) in responses {
            let (mut socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut input = vec![];
            loop {
                let mut buf = [0; 1024];
                let count = socket.read(&mut buf).await.unwrap();
                assert!(count > 0);
                input.extend_from_slice(&buf[..count]);
                assert!(input.len() <= 32768);
                if input.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(input).unwrap());
            socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    (format!("http://{address}"), job)
}
fn ok(body: serde_json::Value) -> (u16, String, String) {
    (200, body.to_string(), String::new())
}
fn connection(protocol: &str, base: String, authenticated: bool) -> Connection {
    Connection {
        connection_id: "discovery-only".into(),
        display_name: "Controlled native list".into(),
        protocol: protocol.into(),
        adapter_binding: None,
        preset_source: None,
        deployment: "local".into(),
        auth_mode: if authenticated { "api_key" } else { "none" }.into(),
        endpoint: Some(base),
        credential_pool_id: None,
        credential_pool_revision: None,
        proxy_mode: "disabled".into(),
        network_metadata: BTreeMap::new(),
        availability_intent: true,
    }
}

#[tokio::test]
async fn native_dialects_use_their_exact_get_route_auth_and_model_identity() {
    for (protocol, suffix, route, body, expected) in [
        (
            "openai_responses",
            "/tenant/v1/responses",
            "/tenant/v1/models",
            json!({"data":[{"id":"model-z"},{"id":"model-a"},{"id":"model-z"}]}),
            vec!["model-a", "model-z"],
        ),
        (
            "ollama_native",
            "/tenant/api/chat",
            "/tenant/api/tags",
            json!({"models":[{"name":"translator:latest"}]}),
            vec!["translator:latest"],
        ),
        (
            "lmstudio_native_v1",
            "/tenant/api/v1/chat",
            "/tenant/api/v1/models",
            json!({"models":[{"type":"embedding","key":"not-chat"},{"type":"decision","key":"not-llm"},{"type":"llm","key":"publisher/actual-model"}]}),
            vec!["publisher/actual-model"],
        ),
    ] {
        let (base, job) = server(vec![ok(body)]).await;
        let con = connection(protocol, format!("{base}{suffix}"), true);
        assert_eq!(
            model_discovery::discover(&con, Some("wire-key"))
                .await
                .unwrap(),
            expected
        );
        let requests = job.await.unwrap();
        assert!(requests[0].starts_with(&format!("GET {route} HTTP/1.1\r\n")));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer wire-key\r\n")
        );
        assert!(!requests[0].contains("source_text"));
        assert!(!requests[0].contains("load"));
    }
}

#[tokio::test]
async fn gemini_follows_bounded_tokens_at_the_same_origin_and_filters_non_generating_models() {
    let (base, job) = server(vec![
        ok(json!({"models":[{"name":"models/model-b","supportedGenerationMethods":["generateContent"]},{"name":"models/embed-only","supportedGenerationMethods":["embedContent"]}],"nextPageToken":"opaque+cursor/=x"})),
        ok(json!({"models":[{"name":"models/model-a","supportedGenerationMethods":["generateContent"]}]})),
    ]).await;
    let con = connection("gemini_generate_content", format!("{base}/v1beta"), true);
    assert_eq!(
        model_discovery::discover(&con, Some("gemini-key"))
            .await
            .unwrap(),
        vec!["model-a", "model-b"]
    );
    let requests = job.await.unwrap();
    for request in &requests {
        assert!(request.starts_with("GET /v1beta/models?pageSize=1000"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("x-goog-api-key: gemini-key\r\n")
        );
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        assert!(!request.lines().next().unwrap().contains("gemini-key"));
    }
    assert!(
        requests[1]
            .lines()
            .next()
            .unwrap()
            .contains("pageToken=opaque%2Bcursor%2F%3Dx")
    );
}

#[tokio::test]
async fn local_native_lists_work_without_auth_and_never_download_or_load_a_model() {
    for (protocol, body) in [
        ("ollama_native", json!({"models":[]})),
        ("lmstudio_native_v1", json!({"models":[]})),
    ] {
        let (base, job) = server(vec![ok(body)]).await;
        assert!(
            model_discovery::discover(&connection(protocol, base, false), None)
                .await
                .unwrap()
                .is_empty()
        );
        let requests = job.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET "));
        assert!(!requests[0].to_ascii_lowercase().contains("authorization:"));
    }
}

#[tokio::test]
async fn missing_or_wrong_authentication_is_rejected_before_transport() {
    for (protocol, mode, key) in [
        ("openai_responses", "api_key", None),
        ("gemini_generate_content", "api_key", None),
        ("openai_responses", "none", None),
        ("ollama_native", "google_oauth", Some("do-not-send")),
        ("lmstudio_native_v1", "none", Some("unexpected-key")),
    ] {
        let mut con = connection(protocol, "invalid URL must not be parsed".into(), true);
        con.auth_mode = mode.into();
        let err = model_discovery::discover(&con, key).await.unwrap_err();
        assert!(matches!(
            err.code.as_str(),
            "missing_credential" | "unsupported_authentication"
        ));
    }
}

#[tokio::test]
async fn incomplete_malformed_and_unbounded_lists_fail_without_returning_partial_ids() {
    for (protocol, body, code) in [
        (
            "openai_responses",
            json!({"data":[{"id":"ok"}],"has_more":true}),
            "invalid_model_list",
        ),
        (
            "openai_responses",
            json!({"data":[{"id":"ok"},{"id":"bad\u{0000}id"}]}),
            "invalid_model_list",
        ),
        (
            "ollama_native",
            json!({"models":[{"name":"x".repeat(513)}]}),
            "invalid_model_list",
        ),
        (
            "lmstudio_native_v1",
            json!({"models":[{"type":"new-type","key":"unknown"}]}),
            "invalid_model_list",
        ),
        (
            "gemini_generate_content",
            json!({"models":[{"name":"models/model"}]}),
            "invalid_model_list",
        ),
        (
            "ollama_native",
            json!({"models":vec![json!({"name":"duplicate"});4097]}),
            "model_list_limit",
        ),
    ] {
        let (base, job) = server(vec![ok(body)]).await;
        let err = model_discovery::discover(&connection(protocol, base, true), Some("key"))
            .await
            .unwrap_err();
        assert_eq!(err.code, code);
        job.await.unwrap();
    }
}

#[tokio::test]
async fn repeated_or_invalid_gemini_cursor_is_not_followed_indefinitely() {
    let body = json!({"models":[],"nextPageToken":"repeated"});
    let (base, job) = server(vec![ok(body.clone()), ok(body)]).await;
    let err = model_discovery::discover(
        &connection("gemini_generate_content", base, true),
        Some("key"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, "invalid_model_list");
    assert_eq!(job.await.unwrap().len(), 2);
}

#[tokio::test]
async fn provider_error_bodies_and_redirects_are_not_exposed_or_followed() {
    for status in [401, 302] {
        let (base, job) = server(vec![(
            status,
            "raw-provider-secret-marker".into(),
            "Location: http://127.0.0.1:1/do-not-follow\r\n".into(),
        )])
        .await;
        let err =
            model_discovery::discover(&connection("openai_responses", base, true), Some("key"))
                .await
                .unwrap_err();
        assert_eq!(
            err.code,
            if status == 401 {
                "authentication_failed"
            } else {
                "http_error"
            }
        );
        assert!(!format!("{err:?}").contains("raw-provider-secret-marker"));
        assert_eq!(job.await.unwrap().len(), 1);
    }
}
