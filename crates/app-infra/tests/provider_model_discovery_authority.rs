//! Maintained independent regressions for bounded model-list wire and live credential authority.
//! Origin: /root/offline_o2_review; listing never grants model or purpose qualification.
use app_core::contracts::Connection;
use app_infra::providers;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
struct Reply {
    status: u16,
    body: String,
    headers: String,
    delay: Duration,
}
fn ok(value: Value) -> Reply {
    Reply {
        status: 200,
        body: value.to_string(),
        headers: String::new(),
        delay: Duration::ZERO,
    }
}
async fn serve(replies: Vec<Reply>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let job = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in replies {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(19), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut b = [0u8; 1024];
                let n = socket.read(&mut b).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&b[..n]);
                assert!(bytes.len() < 32768);
                if bytes.windows(4).any(|x| x == b"\r\n\r\n") {
                    break;
                }
            }
            seen.push(String::from_utf8(bytes).unwrap());
            tokio::time::sleep(reply.delay).await;
            let response = format!(
                "HTTP/1.1 {} Probe\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                reply.status,
                reply.body.len(),
                reply.headers,
                reply.body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
        seen
    });
    (base, job)
}
fn con(protocol: &str, endpoint: String, auth: &str) -> Connection {
    Connection {
        connection_id: "review-wire".into(),
        display_name: "Review only".into(),
        protocol: protocol.into(),
        adapter_binding: None,
        preset_source: None,
        deployment: "local".into(),
        auth_mode: auth.into(),
        endpoint: Some(endpoint),
        credential_pool_id: None,
        credential_pool_revision: None,
        proxy_mode: "disabled".into(),
        network_metadata: BTreeMap::new(),
        availability_intent: true,
    }
}
async fn result(protocol: &str, body: Value) -> Result<Vec<String>, app_core::contracts::AppError> {
    let (base, job) = serve(vec![ok(body)]).await;
    let result = providers::adapters::model_discovery::discover(
        &con(protocol, base, "api_key"),
        Some("review-secret"),
    )
    .await;
    assert_eq!(job.await.unwrap().len(), 1);
    result
}
#[tokio::test]
async fn root_prefix_and_complete_routes_keep_one_exact_dialect_path() {
    for (protocol, paths, route, body) in [
        (
            "openai_responses",
            vec!["/tenant/v1/", "/tenant/v1/responses/"],
            "/tenant/v1/models",
            json!({"data":[]}),
        ),
        (
            "ollama_native",
            vec!["/tenant/", "/tenant/api", "/tenant/api/chat/"],
            "/tenant/api/tags",
            json!({"models":[]}),
        ),
        (
            "lmstudio_native_v1",
            vec!["/tenant/", "/tenant/api/v1/", "/tenant/api/v1/chat/"],
            "/tenant/api/v1/models",
            json!({"models":[]}),
        ),
        (
            "gemini_generate_content",
            vec!["/tenant/v1beta/", "/tenant/v1beta/models/"],
            "/tenant/v1beta/models?pageSize=1000",
            json!({"models":[]}),
        ),
    ] {
        for path in paths {
            let (base, job) = serve(vec![ok(body.clone())]).await;
            assert!(
                providers::adapters::model_discovery::discover(
                    &con(protocol, format!("{base}{path}"), "api_key"),
                    Some("secret")
                )
                .await
                .unwrap()
                .is_empty()
            );
            let seen = job.await.unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(
                seen[0].lines().next().unwrap(),
                format!("GET {route} HTTP/1.1")
            );
        }
    }
}
#[tokio::test]
async fn auth_rejection_and_unsupported_protocols_do_not_open_a_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    for (protocol, auth, key, expected) in [
        (
            "openai_responses",
            "none",
            None,
            "unsupported_authentication",
        ),
        (
            "gemini_generate_content",
            "api_key",
            None,
            "missing_credential",
        ),
        ("ollama_native", "api_key", Some(""), "missing_credential"),
        (
            "lmstudio_native_v1",
            "none",
            Some("not-permitted"),
            "unsupported_authentication",
        ),
        (
            "not_installed",
            "api_key",
            Some("secret"),
            "discovery_unsupported",
        ),
    ] {
        let err =
            providers::adapters::model_discovery::discover(&con(protocol, base.clone(), auth), key)
                .await
                .unwrap_err();
        assert_eq!(err.code, expected);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn id_bounds_measure_utf8_bytes_and_preserve_exact_accepted_identity() {
    let max = format!("{}ab", "あ".repeat(170));
    assert_eq!(max.len(), 512);
    assert_eq!(
        result("ollama_native", json!({"models":[{"name":max.clone()}]}))
            .await
            .unwrap(),
        vec![max]
    );
    for invalid in [
        "あ".repeat(171),
        "x".repeat(513),
        " bad".into(),
        "bad ".into(),
        "bad\nline".into(),
        "".into(),
    ] {
        assert_eq!(
            result(
                "ollama_native",
                json!({"models":[{"name":"valid"},{"name":invalid}]})
            )
            .await
            .unwrap_err()
            .code,
            "invalid_model_list"
        );
    }
}
#[tokio::test]
async fn global_row_cap_counts_duplicates_and_filtered_rows_before_dedup() {
    for (second, expected) in [
        (2048, Ok(vec!["same".to_owned()])),
        (2049, Err("model_list_limit")),
    ] {
        let entry = json!({"name":"models/same","supportedGenerationMethods":["generateContent"]});
        let (base, job) = serve(vec![
            ok(json!({"models":vec![entry.clone();2048],"nextPageToken":"second"})),
            ok(json!({"models":vec![entry;second]})),
        ])
        .await;
        let actual = providers::adapters::model_discovery::discover(
            &con("gemini_generate_content", base, "api_key"),
            Some("secret"),
        )
        .await
        .map_err(|e| e.code);
        assert_eq!(actual, expected.map_err(str::to_owned));
        assert_eq!(job.await.unwrap().len(), 2);
    }
    let filtered = json!({"type":"embedding","key":"not-selected"});
    assert_eq!(
        result("lmstudio_native_v1", json!({"models":vec![filtered;4097]}))
            .await
            .unwrap_err()
            .code,
        "model_list_limit"
    );
}
#[tokio::test]
async fn thirty_two_pages_are_a_total_ceiling_without_partial_success() {
    for ends in [true, false] {
        let replies=(0..32).map(|n| {if ends&&n==31 {ok(json!({"models":[{"name":"models/end","supportedGenerationMethods":["generateContent"]}]}))}else{ok(json!({"models":[],"nextPageToken":format!("page-{n}")}))}}).collect();
        let (base, job) = serve(replies).await;
        let output = providers::adapters::model_discovery::discover(
            &con("gemini_generate_content", base, "api_key"),
            Some("secret"),
        )
        .await;
        if ends {
            assert_eq!(output.unwrap(), vec!["end"])
        } else {
            assert_eq!(output.unwrap_err().code, "model_list_limit")
        };
        assert_eq!(job.await.unwrap().len(), 32);
    }
}
#[tokio::test]
async fn malformed_tokens_error_envelopes_and_nonpagination_markers_fail_closed() {
    for token in [json!(7), json!("x".repeat(4097)), json!("line\nbreak")] {
        assert_eq!(
            result(
                "gemini_generate_content",
                json!({"models":[],"nextPageToken":token})
            )
            .await
            .unwrap_err()
            .code,
            "invalid_model_list"
        );
    }
    for marker in [Value::Null, json!(true), json!("false"), json!(0)] {
        assert_eq!(
            result(
                "openai_responses",
                json!({"data":[{"id":"should-not-return"}],"has_more":marker})
            )
            .await
            .unwrap_err()
            .code,
            "invalid_model_list"
        );
    }
    assert!(
        result(
            "openai_responses",
            json!({"data":[],"has_more":false,"nextPageToken":null})
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        result(
            "ollama_native",
            json!({"models":[{"name":"ok"}],"error":{"message":"provider-secret"}})
        )
        .await
        .unwrap_err()
        .code,
        "invalid_model_list"
    );
    assert_eq!(
        result(
            "ollama_native",
            json!({"models":[{"name":"ok"}],"nextPageToken":"unread"})
        )
        .await
        .unwrap_err()
        .code,
        "invalid_model_list"
    );
}
#[tokio::test]
async fn redirects_never_contact_the_destination_or_forward_the_bearer() {
    let sink = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = format!("http://{}/secrets", sink.local_addr().unwrap());
    let (base, job) = serve(vec![Reply {
        status: 307,
        body: "secret-body".into(),
        headers: format!("Location: {destination}\r\n"),
        delay: Duration::ZERO,
    }])
    .await;
    let err = providers::adapters::model_discovery::discover(
        &con("openai_responses", base, "api_key"),
        Some("original-secret"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, "http_error");
    assert!(!format!("{err:?}").contains("secret"));
    assert_eq!(job.await.unwrap().len(), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), sink.accept())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn response_bytes_are_bounded_even_with_valid_unused_padding() {
    let limit = 2 * 1024 * 1024usize;
    let overhead = json!({"data":[],"padding":""}).to_string().len();
    let (base, job) = serve(vec![ok(
        json!({"data":[],"padding":"x".repeat(limit-overhead)}),
    )])
    .await;
    assert!(
        providers::adapters::model_discovery::discover(
            &con("openai_responses", base, "api_key"),
            Some("secret")
        )
        .await
        .unwrap()
        .is_empty()
    );
    job.await.unwrap();
    let (base, job) = serve(vec![ok(
        json!({"data":[],"padding":"x".repeat(limit-overhead+1)}),
    )])
    .await;
    assert_eq!(
        providers::adapters::model_discovery::discover(
            &con("openai_responses", base, "api_key"),
            Some("secret")
        )
        .await
        .unwrap_err()
        .code,
        "response_too_large"
    );
    job.await.unwrap();
}
#[tokio::test]
async fn whole_pagination_has_one_fifteen_second_deadline_not_fifteen_per_page() {
    let mut first = ok(json!({"models":[],"nextPageToken":"second"}));
    first.delay = Duration::from_secs(8);
    let mut second = ok(
        json!({"models":[{"name":"models/too-late","supportedGenerationMethods":["generateContent"]}]}),
    );
    second.delay = Duration::from_secs(8);
    let (base, job) = serve(vec![first, second]).await;
    let begin = Instant::now();
    let err = tokio::time::timeout(
        Duration::from_secs(18),
        providers::adapters::model_discovery::discover(
            &con("gemini_generate_content", base, "api_key"),
            Some("secret"),
        ),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(err.code, "timeout");
    assert!(begin.elapsed() >= Duration::from_millis(14800));
    assert!(begin.elapsed() < Duration::from_millis(17000));
    job.abort();
}

#[tokio::test]
async fn actual_gateway_preserves_exact_registry_live_typed_lease_and_read_only_discovery() {
    use app_core::{
        contracts::*,
        domain::model_config::credentials::*,
        ports::{
            ProductRepository,
            runtime::{CredentialVault, ModelGateway},
        },
    };
    use app_infra::{providers::gateway::HttpModelGateway, storage::SqliteStore};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Vault {
        reads: AtomicUsize,
        bytes: Vec<u8>,
    }
    impl CredentialVault for Vault {
        fn get(&self, _: &str) -> AppResult<Vec<u8>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.bytes.clone())
        }
        fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
            panic!("discovery must not write Vault")
        }
        fn delete(&self, _: &str) -> AppResult<()> {
            panic!("discovery must not delete Vault")
        }
    }
    for (protocol, body, expected) in [
        (
            "openai_responses",
            json!({"data":[{"id":"ready-list-only"}]}),
            "ready-list-only",
        ),
        (
            "gemini_generate_content",
            json!({"models":[{"name":"models/ready-list-only","supportedGenerationMethods":["generateContent"]}]}),
            "ready-list-only",
        ),
        (
            "ollama_native",
            json!({"models":[{"name":"ready-list-only"}]}),
            "ready-list-only",
        ),
        (
            "lmstudio_native_v1",
            json!({"models":[{"type":"llm","key":"ready-list-only"}]}),
            "ready-list-only",
        ),
    ] {
        let dir =
            std::env::temp_dir().join(format!("review-discovery-gateway-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let repo = Arc::new(
            SqliteStore::open(dir.join("profile.sqlite3"), "review", "server", 1).unwrap(),
        );
        let secret = CredentialMaterial::ApiKey {
            key: SecretText::from_bytes(b"review-api-key".to_vec()).unwrap(),
        };
        repo.begin_credential_material(
            CredentialReference {
                credential_id: "credential".into(),
                credential_version_id: "opaque-vault-version".into(),
                pool_id: "pool".into(),
                pool_revision: 1,
                label: "review".into(),
                masked_hint: "masked".into(),
            },
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::ApiKey,
            },
            2,
        )
        .unwrap();
        repo.activate_credential("credential".into(), 3).unwrap();
        let vault = Arc::new(Vault {
            reads: AtomicUsize::new(0),
            bytes: encode_vault_credential(&secret).unwrap().expose().to_vec(),
        });
        let gateway = HttpModelGateway::new(repo.clone(), vault.clone());
        let (base, job) = serve(vec![ok(body)]).await;
        let mut connection = con(protocol, base, "api_key");
        connection.adapter_binding = Some(
            providers::builtin_registry()
                .unwrap()
                .catalog()
                .presets
                .iter()
                .find(|p| p.binding.protocol.protocol_id == protocol)
                .unwrap()
                .binding
                .clone(),
        );
        connection.credential_pool_id = Some("pool".into());
        connection.credential_pool_revision = Some(1);
        assert_eq!(
            gateway
                .discover_models(&connection, None)
                .await
                .unwrap_err()
                .code,
            "missing_credential"
        );
        assert_eq!(vault.reads.load(Ordering::SeqCst), 0);
        let mut invalid = connection.clone();
        invalid
            .adapter_binding
            .as_mut()
            .unwrap()
            .profile
            .profile_revision += 1;
        assert!(
            gateway
                .discover_models(&invalid, Some("opaque-vault-version"))
                .await
                .is_err()
        );
        assert_eq!(vault.reads.load(Ordering::SeqCst), 0);
        let before = serde_json::to_value(repo.snapshot().unwrap()).unwrap();
        assert_eq!(
            gateway
                .discover_models(&connection, Some("opaque-vault-version"))
                .await
                .unwrap(),
            vec![expected]
        );
        assert_eq!(vault.reads.load(Ordering::SeqCst), 1);
        assert_eq!(
            serde_json::to_value(repo.snapshot().unwrap()).unwrap(),
            before,
            "list success cannot qualify purposes or rewrite configuration"
        );
        let requests = job.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].to_ascii_lowercase().contains(
            if protocol == "gemini_generate_content" {
                "x-goog-api-key: review-api-key\r\n"
            } else {
                "authorization: bearer review-api-key\r\n"
            }
        ));
        repo.revoke_credential("credential".into(), 4).unwrap();
        assert!(
            gateway
                .discover_models(&connection, Some("opaque-vault-version"))
                .await
                .is_err()
        );
        assert_eq!(vault.reads.load(Ordering::SeqCst), 1);
        drop(gateway);
        drop(repo);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn native_list_revoked_during_http_wait_must_not_return_old_authorized_results() {
    use app_core::{
        contracts::*,
        domain::model_config::credentials::*,
        ports::{
            ProductRepository,
            runtime::{CredentialVault, ModelGateway},
        },
    };
    use app_infra::{providers::gateway::HttpModelGateway, storage::SqliteStore};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Vault {
        reads: AtomicUsize,
        bytes: Vec<u8>,
    }
    impl CredentialVault for Vault {
        fn get(&self, _: &str) -> AppResult<Vec<u8>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.bytes.clone())
        }
        fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
            panic!("read-only discovery")
        }
        fn delete(&self, _: &str) -> AppResult<()> {
            panic!("read-only discovery")
        }
    }
    let dir =
        std::env::temp_dir().join(format!("review-discovery-revoke-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let repo =
        Arc::new(SqliteStore::open(dir.join("profile.sqlite3"), "review", "server", 1).unwrap());
    let secret = CredentialMaterial::ApiKey {
        key: SecretText::from_bytes(b"review-api-key".to_vec()).unwrap(),
    };
    repo.begin_credential_material(
        CredentialReference {
            credential_id: "credential".into(),
            credential_version_id: "opaque-vault-version".into(),
            pool_id: "pool".into(),
            pool_revision: 1,
            label: "review".into(),
            masked_hint: "masked".into(),
        },
        CredentialStorageMetadata::EnvelopeV1 {
            kind: CredentialMaterialKind::ApiKey,
        },
        2,
    )
    .unwrap();
    repo.activate_credential("credential".into(), 3).unwrap();
    let vault = Arc::new(Vault {
        reads: AtomicUsize::new(0),
        bytes: encode_vault_credential(&secret).unwrap().expose().to_vec(),
    });
    let gateway = HttpModelGateway::new(repo.clone(), vault.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut b = [0u8; 1024];
            let n = socket.read(&mut b).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&b[..n]);
            if bytes.windows(4).any(|x| x == b"\r\n\r\n") {
                break;
            }
        }
        entered_tx.send(()).unwrap();
        release_rx.await.unwrap();
        let body = json!({"models":[{"name":"must-be-discarded"}]}).to_string();
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    });
    let mut connection = con("ollama_native", endpoint, "api_key");
    connection.adapter_binding = Some(
        providers::builtin_registry()
            .unwrap()
            .catalog()
            .presets
            .iter()
            .find(|p| p.binding.protocol.protocol_id == "ollama_native")
            .unwrap()
            .binding
            .clone(),
    );
    connection.credential_pool_id = Some("pool".into());
    connection.credential_pool_revision = Some(1);
    let request = tokio::spawn(async move {
        gateway
            .discover_models(&connection, Some("opaque-vault-version"))
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(vault.reads.load(Ordering::SeqCst), 1);
    repo.revoke_credential("credential".into(), 4).unwrap();
    release_tx.send(()).unwrap();
    server.await.unwrap();
    let output = request.await.unwrap();
    assert!(
        output.is_err(),
        "revocation during real network wait must discard list; actual={output:?}"
    );
    assert_eq!(vault.reads.load(Ordering::SeqCst), 1);
    drop(repo);
    std::fs::remove_dir_all(dir).unwrap();
}
