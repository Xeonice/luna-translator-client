//! Real schema3 repository -> task plan -> typed Vault -> loopback gateway proof.
//! Loopback contracts do not grant real vendor, UI or packaged acceptance.
use app_core::{
    application::request_plan::{self, RequestPlan},
    contracts::*,
    domain::{TaskDispatch, model_config::credentials::*},
    ports::{
        ProductRepository,
        runtime::{CredentialVault, ModelGateway},
    },
};
use app_infra::{
    providers::{self, gateway::HttpModelGateway},
    storage::SqliteStore,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

struct Vault {
    bytes: Vec<u8>,
    reads: AtomicUsize,
    revoke: Mutex<Option<(Arc<SqliteStore>, String)>>,
}
impl CredentialVault for Vault {
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some((repository, id)) = self.revoke.lock().unwrap().take() {
            repository.revoke_credential(id, 100)?;
        }
        Ok(self.bytes.clone())
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        panic!("gateway must not store secrets")
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        panic!("gateway must not delete secrets")
    }
}
struct Fixture {
    _directory: PathBuf,
    repository: Arc<SqliteStore>,
    vault: Arc<Vault>,
    connection: Connection,
    gateway: HttpModelGateway,
}
fn command(store: &SqliteStore, command: Command) -> CommandReceipt {
    store
        .dispatch(
            CommandEnvelope {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: None,
                command,
            },
            20,
        )
        .unwrap()
}
fn binding(protocol: &str) -> AdapterBinding {
    providers::builtin_registry()
        .unwrap()
        .catalog()
        .presets
        .iter()
        .find(|p| p.binding.protocol.protocol_id == protocol)
        .unwrap()
        .binding
        .clone()
}
impl Fixture {
    fn new(protocol: &str, endpoint: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("gateway-identity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let repository = Arc::new(
            SqliteStore::open(
                directory.join("profile.sqlite3"),
                "gateway-test",
                "gateway-server",
                1,
            )
            .unwrap(),
        );
        let (auth_mode, material) = match protocol {
            "baidu_vip" => (
                "baidu_app_secret",
                CredentialMaterial::BaiduAppSecret {
                    app_id: "test-app".into(),
                    secret: SecretText::from_bytes(b"test-secret".to_vec()).unwrap(),
                },
            ),
            "youdao_v3" => (
                "youdao_app_secret",
                CredentialMaterial::YoudaoAppSecret {
                    app_key: "test-app".into(),
                    secret: SecretText::from_bytes(b"test-secret".to_vec()).unwrap(),
                },
            ),
            _ => (
                "api_key",
                CredentialMaterial::ApiKey {
                    key: SecretText::from_bytes(b"test-key".to_vec()).unwrap(),
                },
            ),
        };
        let reference = CredentialReference {
            credential_id: "credential".into(),
            credential_version_id: "opaque-vault-version".into(),
            pool_id: "pool".into(),
            pool_revision: 1,
            label: "test".into(),
            masked_hint: "••".into(),
        };
        repository
            .begin_credential_material(
                reference,
                CredentialStorageMetadata::EnvelopeV1 {
                    kind: material.kind(),
                },
                2,
            )
            .unwrap();
        repository
            .activate_credential("credential".into(), 3)
            .unwrap();
        let vault = Arc::new(Vault {
            bytes: encode_vault_credential(&material)
                .unwrap()
                .expose()
                .to_vec(),
            reads: AtomicUsize::new(0),
            revoke: Mutex::new(None),
        });
        let connection = Connection {
            adapter_binding: Some(binding(protocol)),
            preset_source: None,
            connection_id: "connection".into(),
            display_name: "test".into(),
            protocol: protocol.into(),
            deployment: "local".into(),
            auth_mode: auth_mode.into(),
            endpoint: Some(endpoint.into()),
            credential_pool_id: Some("pool".into()),
            credential_pool_revision: Some(1),
            proxy_mode: "disabled".into(),
            network_metadata: BTreeMap::new(),
            availability_intent: true,
        };
        let gateway = HttpModelGateway::with_registry(
            repository.clone(),
            vault.clone(),
            Arc::new(providers::builtin_registry().unwrap()),
        );
        Self {
            _directory: directory,
            repository,
            vault,
            connection,
            gateway,
        }
    }
    fn plan(&self) -> RequestPlan {
        RequestPlan {
            adapter_binding: self.connection.adapter_binding.clone(),
            purpose: Some(CatalogPurpose::Translation),
            protocol: self.connection.protocol.clone(),
            endpoint: self.connection.endpoint.clone().unwrap(),
            model: Some("test-model".into()),
            source_text: "雨がやんだ。".into(),
            source_language: "ja".into(),
            target_language: "zh-CN".into(),
            messages: vec![json!({"role":"user","content":"雨がやんだ。"})],
            parameters: BTreeMap::new(),
            structured: false,
            stream: false,
            timeout_ms: 2000,
        }
    }
    fn task(&self) -> TaskDispatch {
        let configuration = Configuration {
            codec_version: Some(2),
            connections: vec![self.connection.clone()],
            models: vec![Model {
                default_templates: BTreeMap::new(),
                model_id: "model".into(),
                connection_id: self.connection.connection_id.clone(),
                actual_model_id: Some("test-model".into()),
                display_name: "test".into(),
                selection_source: "manual".into(),
                capabilities: BTreeMap::from([("streaming".into(), json!(false))]),
                parameters: BTreeMap::new(),
            }],
            bindings: vec![PurposeBinding {
                binding_id: "translation".into(),
                scope_key: "global".into(),
                game_id: None,
                purpose: "translation".into(),
                binding_mode: "override".into(),
                enabled: true,
                output_mode: Some("text".into()),
                source_language: Some("ja".into()),
                target_language: Some("zh-CN".into()),
                template_id: None,
                context: BTreeMap::new(),
                schedule: BTreeMap::new(),
                parameters: BTreeMap::new(),
                engines: vec![Engine {
                    engine_id: "engine".into(),
                    model_id: "model".into(),
                    enabled: true,
                    template_id: None,
                    parameters: BTreeMap::new(),
                }],
            }],
            ..Configuration::default()
        };
        let saved = command(
            &self.repository,
            Command::SaveConfiguration {
                configuration,
                expected_head_revision: 0,
            },
        );
        command(
            &self.repository,
            Command::ApplyConfiguration {
                revision_id: saved.result_id.unwrap(),
                expected_head_revision: 0,
            },
        );
        let unit = command(
            &self.repository,
            Command::ManualInput {
                text: "雨がやんだ。".into(),
                source_language: "ja".into(),
            },
        )
        .result_id
        .unwrap();
        self.repository
            .prepare_tasks(unit, 1, "translation".into(), 30)
            .unwrap()
            .into_iter()
            .next()
            .expect("real enabled configuration must prepare a task")
    }
}
async fn listener(body: Value) -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (send, receive) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = vec![];
        let mut buffer = [0; 4096];
        let end = loop {
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                break index + 4;
            }
            assert!(bytes.len() < 65536);
        };
        let head = String::from_utf8(bytes[..end].to_vec()).unwrap();
        let length = head
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        while bytes.len() < end + length {
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
        }
        let _ = send.send(head);
        let body = body.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
    });
    (format!("http://{address}"), receive)
}
fn response(protocol: &str) -> Value {
    match protocol {
        "openai_compatible" => {
            json!({"choices":[{"message":{"content":"雨停了。"},"finish_reason":"stop"}]})
        }
        "anthropic" => {
            json!({"content":[{"type":"text","text":"雨停了。"}],"stop_reason":"end_turn"})
        }
        "openai_responses" => {
            json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"雨停了。"}]}]})
        }
        "gemini_generate_content" => {
            json!({"candidates":[{"content":{"parts":[{"text":"雨停了。"}]},"finishReason":"STOP"}]})
        }
        "ollama_native" => {
            json!({"message":{"content":"雨停了。"},"done":true,"done_reason":"stop"})
        }
        "lmstudio_native_v1" => json!({"output":[{"type":"message","content":"雨停了。"}]}),
        "azure_translator" => json!([{"translations":[{"text":"雨停了。"}]}]),
        "deepl_v2" => json!({"translations":[{"text":"雨停了。"}]}),
        "google_translate_v2" => json!({"data":{"translations":[{"translatedText":"雨停了。"}]}}),
        "baidu_vip" => json!({"trans_result":[{"dst":"雨停了。"}]}),
        "youdao_v3" => json!({"errorCode":"0","translation":["雨停了。"]}),
        "tencent_tokenhub_translation" => {
            json!({"choices":[{"message":{"content":"雨停了。"},"finish_reason":"stop"}]})
        }
        _ => unreachable!(),
    }
}
#[tokio::test]
async fn exact_binding_typed_authority_and_real_task_gateway_paths() {
    let mut assertions = 0usize;
    macro_rules! check {
        ($condition:expr) => {{
            assert!($condition);
            assertions += 1;
        }};
    }
    let protocols = [
        "openai_compatible",
        "anthropic",
        "openai_responses",
        "gemini_generate_content",
        "ollama_native",
        "lmstudio_native_v1",
        "azure_translator",
        "deepl_v2",
        "google_translate_v2",
        "baidu_vip",
        "youdao_v3",
        "tencent_tokenhub_translation",
    ];
    for protocol in protocols {
        for formal in [false, true] {
            let (endpoint, request) = listener(response(protocol)).await;
            let fixture = Fixture::new(protocol, &endpoint);
            let text = if formal {
                let task = fixture.task();
                let plan = request_plan::compose(&task, &[]).unwrap();
                fixture
                    .gateway
                    .execute(&task, &plan, Arc::new(|_| {}))
                    .await
                    .unwrap()
            } else {
                fixture
                    .gateway
                    .test_request(
                        &fixture.connection,
                        &fixture.plan(),
                        Some("opaque-vault-version"),
                    )
                    .await
                    .unwrap()
            };
            check!(text == "雨停了。");
            check!(fixture.vault.reads.load(Ordering::SeqCst) == 1);
            let head = request.await.unwrap();
            check!(head.starts_with("POST "));
            check!(!head.contains("opaque-vault-version"));
        }
    }
    let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", socket.local_addr().unwrap());
    let fixture = Fixture::new("openai_responses", &endpoint);
    let task = fixture.task();
    let plan = request_plan::compose(&task, &[]).unwrap();
    let mut bad = fixture.connection.clone();
    bad.adapter_binding
        .as_mut()
        .unwrap()
        .profile
        .profile_revision = 999;
    check!(
        fixture
            .gateway
            .test_request(&bad, &plan, Some("opaque-vault-version"))
            .await
            .unwrap_err()
            .code
            == "unknown_profile_version"
    );
    check!(
        fixture
            .gateway
            .discover_models(&bad, Some("opaque-vault-version"))
            .await
            .unwrap_err()
            .code
            == "unknown_profile_version"
    );
    let mut bad_task = task.clone();
    bad_task.connection = bad;
    check!(
        fixture
            .gateway
            .execute(&bad_task, &plan, Arc::new(|_| {}))
            .await
            .unwrap_err()
            .code
            == "unknown_profile_version"
    );
    let mut invalid = plan.clone();
    invalid.parameters.insert("temperature".into(), json!(-1));
    check!(
        fixture
            .gateway
            .test_request(&fixture.connection, &invalid, Some("opaque-vault-version"))
            .await
            .unwrap_err()
            .code
            == "invalid_parameter"
    );
    let mut missing = plan.clone();
    missing.purpose = None;
    check!(
        fixture
            .gateway
            .test_request(&fixture.connection, &missing, Some("opaque-vault-version"))
            .await
            .unwrap_err()
            .code
            == "protocol_mismatch"
    );
    let mut google = fixture.connection.clone();
    google.protocol = "google_translate_v3".into();
    google.adapter_binding = Some(binding("google_translate_v3"));
    google.auth_mode = "google_service_account".into();
    google.endpoint = Some("https://translate.googleapis.com/v3".into());
    check!(
        fixture
            .gateway
            .discover_models(&google, Some("opaque-vault-version"))
            .await
            .unwrap_err()
            .code
            == "authorization_provider_unavailable"
    );
    check!(fixture.vault.reads.load(Ordering::SeqCst) == 0);
    fixture
        .repository
        .fail_task(task.task_id.clone(), "cancelled".into(), 40)
        .unwrap();
    check!(
        fixture
            .gateway
            .execute(&task, &plan, Arc::new(|_| {}))
            .await
            .is_err()
    );
    check!(fixture.vault.reads.load(Ordering::SeqCst) == 0);
    let revoked = Fixture::new("openai_responses", &endpoint);
    *revoked.vault.revoke.lock().unwrap() = Some((revoked.repository.clone(), "credential".into()));
    check!(
        revoked
            .gateway
            .test_request(
                &revoked.connection,
                &revoked.plan(),
                Some("opaque-vault-version")
            )
            .await
            .is_err()
    );
    check!(revoked.vault.reads.load(Ordering::SeqCst) == 1);
    check!(
        tokio::time::timeout(std::time::Duration::from_millis(100), socket.accept())
            .await
            .is_err()
    );
    if let Some(output) = std::env::var_os("FP_STORAGE_OUTPUT") {
        let run_id = std::env::var("FP_STORAGE_RUN_ID").expect("same-run storage identity");
        let execution_nonce =
            std::env::var("FP_STORAGE_EXECUTION_NONCE").expect("same-run execution nonce");
        let source_tree_sha256 =
            std::env::var("FP_STORAGE_SOURCE_SHA256").expect("same-run source identity");
        let path = PathBuf::from(output).join("components/v3-delta/gateway-authority.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let receipt = json!({"kind":"http_storage_v3_delta_receipt","run_id":run_id,"execution_nonce":execution_nonce,"source_tree_sha256":source_tree_sha256,"completed_at_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),"checks":[{"id":"V3-AUTHORITY","status":"passed","assertions":assertions,"skipped":0,"source":"provider_gateway_identity::exact_binding_typed_authority_and_real_task_gateway_paths"}],"detail":{"loopback_protocols":protocols,"executions":24,"real_repository":true,"google_oauth":"blocked_before_vault","negative_cases":["wrong_profile_all_three_entrypoints","bad_parameter_before_vault","missing_purpose","terminal_task_before_vault","revoked_during_vault_before_network"]}});
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(serde_json::to_string_pretty(&receipt).unwrap().as_bytes())
            .unwrap();
    }
}
