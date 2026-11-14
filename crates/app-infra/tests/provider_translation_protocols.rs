//! Actual loopback HTTP translation contracts. No account, OAuth exchange or live-service acceptance is implied.
use app_core::{
    application::request_plan::RequestPlan,
    contracts::{AppError, CatalogPurpose, Connection},
    domain::model_config::credentials::{
        CredentialVersionRef, GoogleAuthorizationLeaseIdentity, PrincipalState, ProviderAuthLease,
        SecretText, SignedAppLease,
    },
};
use app_infra::providers::{
    self,
    adapters::translation::{
        self, GoogleV3Route, ReferenceKind, TokenHubTranslationOptions, TranslationReference,
        TranslationWireOptions,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

const SECRET: &str = "isolated-secret-DO-NOT-ECHO";
const PROTOCOLS: [&str; 6] = [
    "deepl_v2",
    "google_translate_v2",
    "google_translate_v3",
    "baidu_vip",
    "youdao_v3",
    "tencent_tokenhub_translation",
];

struct Materials {
    version: CredentialVersionRef,
    secret: SecretText,
    google: GoogleAuthorizationLeaseIdentity,
}
impl Materials {
    fn new() -> Self {
        let version = CredentialVersionRef {
            credential_id: "wire-credential".into(),
            credential_version_id: "wire-version".into(),
        };
        Self {
            secret: SecretText::from_bytes(SECRET.as_bytes().to_vec()).unwrap(),
            google: GoogleAuthorizationLeaseIdentity {
                grant_id: "wire-grant".into(),
                authorization_epoch: NonZeroU32::new(1).unwrap(),
                material_revision: NonZeroU32::new(1).unwrap(),
                credential_version: version.clone(),
                scopes_hash: "a".repeat(64),
                principal_state: PrincipalState::Verified {
                    subject: "wire-subject".into(),
                },
            },
            version,
        }
    }
    fn lease(&self, protocol: &str) -> ProviderAuthLease<'_> {
        match protocol {
            "baidu_vip" => ProviderAuthLease::AppSecret {
                source: &self.version,
                material: SignedAppLease::Baidu {
                    app_id: "2015063000000001",
                    secret: &self.secret,
                },
            },
            "youdao_v3" => ProviderAuthLease::AppSecret {
                source: &self.version,
                material: SignedAppLease::Youdao {
                    app_key: "wire-app-key",
                    secret: &self.secret,
                },
            },
            "google_translate_v3" => ProviderAuthLease::GoogleAccess {
                token: &self.secret,
                identity: &self.google,
            },
            _ => ProviderAuthLease::ApiKey {
                source: &self.version,
                key: &self.secret,
            },
        }
    }
}
#[derive(Debug)]
struct Request {
    head: String,
    body: Vec<u8>,
}
impl Request {
    fn header(&self, key: &str) -> Option<&str> {
        self.head
            .lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.trim())
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
    fn form(&self) -> BTreeMap<String, String> {
        reqwest::Url::parse(&format!(
            "http://fixture.invalid/?{}",
            String::from_utf8(self.body.clone()).unwrap()
        ))
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
    }
}
async fn read_request(socket: &mut TcpStream) -> Request {
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    let end = loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0, "request ended before headers");
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        assert!(bytes.len() < 65536);
    };
    let head = String::from_utf8(bytes[..end].to_vec()).unwrap();
    let length = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(length < 4 * 1024 * 1024);
    while bytes.len() < end + length {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
    }
    Request {
        head,
        body: bytes[end..end + length].to_vec(),
    }
}
async fn server(
    status: u16,
    content_type: &str,
    body: String,
    fragmented: bool,
) -> (
    String,
    oneshot::Receiver<Request>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let content_type = content_type.to_owned();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        // Error-only tests deliberately drop their request receiver. A dropped observer
        // must not prevent the listener from returning its real response.
        let _ = tx.send(request);
        let head = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        if socket.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        if fragmented {
            for chunk in body.as_bytes().chunks(2) {
                if socket.write_all(chunk).await.is_err() {
                    return;
                }
                tokio::task::yield_now().await;
            }
        } else {
            let _ = socket.write_all(body.as_bytes()).await;
        }
        let _ = socket.shutdown().await;
    });
    (format!("http://{address}"), rx, handle)
}
fn connection(base: &str, protocol: &str) -> Connection {
    Connection {
        connection_id: "translation-wire".into(),
        display_name: "Isolated listener".into(),
        protocol: protocol.into(),
        deployment: "local".into(),
        auth_mode: "typed_lease".into(),
        endpoint: Some(base.into()),
        credential_pool_id: None,
        credential_pool_revision: None,
        proxy_mode: "disabled".into(),
        network_metadata: BTreeMap::new(),
        availability_intent: true,
        adapter_binding: None,
        preset_source: None,
    }
}
fn plan(base: &str, protocol: &str) -> RequestPlan {
    RequestPlan {
        adapter_binding: None,
        purpose: None,
        protocol: protocol.into(),
        endpoint: base.into(),
        model: (protocol == "tencent_tokenhub_translation").then(|| "hunyuan-translation".into()),
        source_text: "雨がやんだ。😀 & +".into(),
        source_language: "ja-JP".into(),
        target_language: "zh-TW".into(),
        messages: vec![],
        parameters: BTreeMap::new(),
        structured: false,
        stream: false,
        timeout_ms: 3000,
    }
}
fn options(protocol: &str) -> TranslationWireOptions {
    TranslationWireOptions {
        google_v3: (protocol == "google_translate_v3").then(|| GoogleV3Route {
            project_id: "wire-project".into(),
            location: "global".into(),
        }),
        tokenhub: None,
    }
}
fn response(protocol: &str, text: &str) -> Value {
    match protocol {
        "deepl_v2" => json!({"translations":[{"text":text,"detected_source_language":"JA"}]}),
        "google_translate_v2" => json!({"data":{"translations":[{"translatedText":text}]}}),
        "google_translate_v3" => json!({"translations":[{"translatedText":text}]}),
        "baidu_vip" => {
            json!({"from":"jp","to":"cht","trans_result":[{"src":"original","dst":text}]})
        }
        "youdao_v3" => json!({"errorCode":"0","translation":[text]}),
        "tencent_tokenhub_translation" => {
            json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]})
        }
        _ => panic!("unknown fixture protocol"),
    }
}
fn safe(error: &AppError) {
    let encoded = serde_json::to_string(error).unwrap();
    assert!(!encoded.contains(SECRET));
    assert!(!encoded.contains("raw-source-sensitive"));
    assert!(!encoded.contains("untrusted-server-detail"));
}

#[tokio::test]
async fn six_distinct_json_or_form_wires_use_official_routes_and_lease_auth() {
    let materials = Materials::new();
    for protocol in PROTOCOLS {
        let (base, rx, task) = server(
            200,
            "application/json",
            response(protocol, "雨停了。😀").to_string(),
            false,
        )
        .await;
        let p = plan(&base, protocol);
        let updates = Mutex::new(Vec::new());
        let actual = translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |t| updates.lock().unwrap().push(t.to_owned()),
        )
        .await
        .unwrap();
        assert_eq!(actual.text, "雨停了。😀");
        assert_eq!(*updates.lock().unwrap(), vec![actual.text]);
        let request = rx.await.unwrap();
        task.await.unwrap();
        let route = match protocol {
            "deepl_v2" => "/v2/translate",
            "google_translate_v2" => "/language/translate/v2",
            "google_translate_v3" => "/v3/projects/wire-project/locations/global:translateText",
            "baidu_vip" => "/api/trans/vip/translate",
            "youdao_v3" => "/api",
            _ => "/api/translations",
        };
        assert!(
            request.head.starts_with(&format!("POST {route} HTTP/1.1")),
            "{protocol}: {}",
            request.head
        );
        assert!(!request.head.lines().next().unwrap().contains('?'));
        assert!(!String::from_utf8_lossy(&request.body).contains(SECRET));
        if matches!(protocol, "baidu_vip" | "youdao_v3") {
            assert_eq!(
                request.header("content-type"),
                Some("application/x-www-form-urlencoded")
            );
            assert!(request.header("authorization").is_none());
            let form = request.form();
            assert_eq!(form["q"], p.source_text);
            assert_eq!(
                form["from"],
                if protocol == "baidu_vip" { "jp" } else { "ja" }
            );
            assert_eq!(
                form["to"],
                if protocol == "baidu_vip" {
                    "cht"
                } else {
                    "zh-CHT"
                }
            );
            assert!(!form.contains_key("secret"));
            assert!(uuid::Uuid::parse_str(&form["salt"]).is_ok());
        } else {
            assert_eq!(request.header("content-type"), Some("application/json"));
            let body = request.json();
            assert!(body.get("messages").is_none());
            assert!(body.get("temperature").is_none());
            if protocol == "deepl_v2" {
                assert_eq!(
                    request.header("authorization"),
                    Some(format!("DeepL-Auth-Key {SECRET}").as_str())
                );
                assert_eq!(
                    body,
                    json!({"text":[p.source_text],"source_lang":"JA","target_lang":"ZH-HANT"})
                );
            }
            if protocol == "google_translate_v2" {
                assert_eq!(request.header("x-goog-api-key"), Some(SECRET));
                assert!(request.header("authorization").is_none());
                assert_eq!(
                    body,
                    json!({"q":[p.source_text],"source":"ja","target":"zh-TW","format":"text"})
                );
            }
            if protocol == "google_translate_v3" {
                assert_eq!(
                    request.header("authorization"),
                    Some(format!("Bearer {SECRET}").as_str())
                );
                assert!(request.header("x-goog-api-key").is_none());
                assert_eq!(
                    body,
                    json!({"contents":[p.source_text],"sourceLanguageCode":"ja","targetLanguageCode":"zh-TW","mimeType":"text/plain"})
                );
            }
            if protocol == "tencent_tokenhub_translation" {
                assert_eq!(
                    request.header("authorization"),
                    Some(format!("Bearer {SECRET}").as_str())
                );
                assert_eq!(
                    body,
                    json!({"model":"hunyuan-translation","text":p.source_text,"source":"ja","target":"zh-TR","stream":false})
                );
            }
        }
    }
}

#[tokio::test]
async fn catalog_default_full_paths_custom_prefixes_and_completed_endpoints_are_idempotent() {
    let catalog = providers::builtin_registry().unwrap().catalog();
    let materials = Materials::new();
    for (preset_id, protocol, route) in [
        (
            "builtin.google-v2",
            "google_translate_v2",
            "language/translate/v2",
        ),
        (
            "builtin.google-v3",
            "google_translate_v3",
            "v3/projects/wire-project/locations/global:translateText",
        ),
        ("builtin.baidu", "baidu_vip", "api/trans/vip/translate"),
        ("builtin.youdao", "youdao_v3", "api"),
        (
            "builtin.tencent-tokenhub-translation",
            "tencent_tokenhub_translation",
            "api/translations",
        ),
        ("builtin.deepl", "deepl_v2", "v2/translate"),
    ] {
        let preset = catalog
            .presets
            .iter()
            .find(|p| p.key.preset_id == preset_id)
            .unwrap();
        // DeepL chooses Free/Pro in the service form, so its catalogue default is absent.
        let origin_path = preset
            .default_endpoint
            .as_deref()
            .map(|u| reqwest::Url::parse(u).unwrap().path().to_owned())
            .unwrap_or_else(|| "/v2/translate".into());
        for prefix in [
            origin_path.clone(),
            format!("/proxy{origin_path}"),
            format!("/{route}"),
        ] {
            let (base, rx, task) = server(
                200,
                "application/json",
                response(protocol, "正文").to_string(),
                false,
            )
            .await;
            let base = format!("{base}{prefix}");
            let p = plan(&base, protocol);
            translation::execute(
                &p,
                &connection(&base, protocol),
                CatalogPurpose::Translation,
                &materials.lease(protocol),
                &options(protocol),
                |_| {},
            )
            .await
            .unwrap();
            let final_route =
                if protocol == "tencent_tokenhub_translation" && prefix != format!("/{route}") {
                    "v1/api/translations"
                } else {
                    route
                };
            let expected = if prefix.starts_with("/proxy") {
                format!("/proxy/{final_route}")
            } else {
                format!("/{final_route}")
            };
            assert!(
                rx.await
                    .unwrap()
                    .head
                    .starts_with(&format!("POST {expected} HTTP/1.1")),
                "{protocol}/{prefix}"
            );
            task.await.unwrap();
        }
    }
}

#[tokio::test]
async fn app_signatures_use_raw_utf8_generated_nonce_and_unicode_scalar_truncation() {
    let materials = Materials::new();
    for protocol in ["baidu_vip", "youdao_v3"] {
        let (base, rx, task) = server(
            200,
            "application/json",
            response(protocol, "译文").to_string(),
            false,
        )
        .await;
        let mut p = plan(&base, protocol);
        p.source_text = "😀😀😀😀😀😀😀😀😀😀日😀😀😀😀😀😀😀😀😀😀".into();
        p.source_language = "auto".into();
        translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap();
        let request = rx.await.unwrap();
        task.await.unwrap();
        let form = request.form();
        assert_eq!(form["q"], p.source_text);
        assert_eq!(form["from"], "auto");
        if protocol == "baidu_vip" {
            assert_eq!(form["appid"], "2015063000000001");
            let signed = format!("2015063000000001{}{}{SECRET}", p.source_text, form["salt"]);
            assert_eq!(
                form["sign"],
                format!("{:x}", md5::compute(signed.as_bytes()))
            );
            assert_eq!(form.len(), 6);
        } else {
            let input = "😀😀😀😀😀😀😀😀😀😀21😀😀😀😀😀😀😀😀😀😀";
            let signed = format!(
                "wire-app-key{input}{}{curtime}{SECRET}",
                form["salt"],
                curtime = form["curtime"]
            );
            assert_eq!(
                form["sign"],
                format!("{:x}", Sha256::digest(signed.as_bytes()))
            );
            assert_eq!(form["signType"], "v3");
            assert_eq!(form["strict"], "true");
            let seconds = form["curtime"].parse::<u64>().unwrap();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            assert!(now.abs_diff(seconds) < 10);
            assert_eq!(form.len(), 9);
        }
    }
}

#[tokio::test]
async fn automatic_source_and_supported_language_aliases_are_service_specific() {
    let materials = Materials::new();
    for (protocol, target, expected) in [
        ("deepl_v2", "zh-CN", "ZH-HANS"),
        ("google_translate_v2", "zh-Hans", "zh-CN"),
        ("google_translate_v3", "zh-Hant", "zh-TW"),
        ("baidu_vip", "ko-KR", "kor"),
        ("youdao_v3", "zh-CN", "zh-CHS"),
        ("tencent_tokenhub_translation", "zh-CN", "zh"),
    ] {
        let (base, rx, task) = server(
            200,
            "application/json",
            response(protocol, "译文").to_string(),
            false,
        )
        .await;
        let mut p = plan(&base, protocol);
        p.source_language = "auto".into();
        p.target_language = target.into();
        translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap();
        let request = rx.await.unwrap();
        task.await.unwrap();
        match protocol {
            "baidu_vip" | "youdao_v3" => {
                let f = request.form();
                assert_eq!(f["from"], "auto");
                assert_eq!(f["to"], expected);
            }
            "deepl_v2" => {
                let b = request.json();
                assert!(b.get("source_lang").is_none());
                assert_eq!(b["target_lang"], expected);
            }
            "google_translate_v3" => {
                let b = request.json();
                assert!(b.get("sourceLanguageCode").is_none());
                assert_eq!(b["targetLanguageCode"], expected);
            }
            _ => {
                let b = request.json();
                assert!(b.get("source").is_none());
                assert_eq!(b["target"], expected);
            }
        }
    }
}

#[tokio::test]
async fn tokenhub_uses_dedicated_translation_options_and_fragmented_terminal_delta() {
    let body = format!(
        "data: {}\r\n\r\ndata: {}\r\n\r\ndata: {}\r\n\r\ndata: [DONE]\r\n\r\n",
        json!({"usage":{"prompt_tokens":2}}),
        json!({"choices":[{"delta":{"content":"雨😀"},"finish_reason":null}]}),
        json!({"choices":[{"delta":{"content":"停了。"},"finish_reason":"stop"}]})
    );
    let (base, rx, task) = server(200, "text/event-stream; charset=utf-8", body, true).await;
    let materials = Materials::new();
    let mut p = plan(&format!("{base}/v1"), "tencent_tokenhub_translation");
    p.stream = true;
    let options = TranslationWireOptions {
        google_v3: None,
        tokenhub: Some(TokenHubTranslationOptions {
            context: Some("公园的雨景".into()),
            references: vec![
                TranslationReference {
                    kind: ReferenceKind::Sentence,
                    text: "雨".into(),
                    translation: "下雨".into(),
                },
                TranslationReference {
                    kind: ReferenceKind::Term,
                    text: "桜".into(),
                    translation: "樱花".into(),
                },
            ],
            glossary_ids: vec!["wire-glossary".into()],
        }),
    };
    let updates = Mutex::new(Vec::new());
    let result = translation::execute(
        &p,
        &connection(&p.endpoint, &p.protocol),
        CatalogPurpose::Translation,
        &materials.lease(&p.protocol),
        &options,
        |s| updates.lock().unwrap().push(s.to_owned()),
    )
    .await
    .unwrap();
    assert_eq!(result.text, "雨😀停了。");
    assert_eq!(*updates.lock().unwrap(), vec!["雨😀", "雨😀停了。"]);
    let r = rx.await.unwrap();
    task.await.unwrap();
    assert!(r.head.starts_with("POST /v1/api/translations HTTP/1.1"));
    assert_eq!(r.header("accept"), Some("text/event-stream"));
    let b = r.json();
    assert_eq!(b["text"], p.source_text);
    assert_eq!(b["context"], "公园的雨景");
    assert_eq!(
        b["references"],
        json!([{"type":"sentence","text":"雨","translation":"下雨"},{"type":"term","text":"桜","translation":"樱花"}])
    );
    assert_eq!(b["glossary_ids"], json!(["wire-glossary"]));
    assert!(b.get("messages").is_none());
}

#[tokio::test]
async fn tokenhub_never_accepts_truncation_sensitive_tools_or_unterminated_streams() {
    let materials = Materials::new();
    let cases=[
        (false,json!({"choices":[{"message":{"content":"partial"},"finish_reason":"length"}]}).to_string(),"output_incomplete"),
        (false,json!({"choices":[{"message":{"content":"partial"},"finish_reason":"sensitive"}]}).to_string(),"output_incomplete"),
        (false,json!({"choices":[{"message":{"content":"partial"}}]}).to_string(),"output_incomplete"),
        (false,json!({"choices":[{"message":{"content":"partial","tool_calls":[]},"finish_reason":"stop"}]}).to_string(),"output_incomplete"),
        (true,format!("data: {}\n\n",json!({"choices":[{"delta":{"content":"partial"}}]})),"stream_interrupted"),
        (true,format!("data: {}\n\n",json!({"choices":[{"delta":{"content":"partial"},"finish_reason":"sensitive"}]})),"output_incomplete"),
        (true,"data: [DONE]\n\n".into(),"malformed_stream"),
        (true,format!("data: {}\n\n",json!({"choices":[{"delta":{},"finish_reason":"stop"}]})),"empty_response"),
    ];
    for (stream, body, code) in cases {
        let (base, _, task) = server(
            200,
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
            body,
            true,
        )
        .await;
        let mut p = plan(&base, "tencent_tokenhub_translation");
        p.stream = stream;
        let e = translation::execute(
            &p,
            &connection(&base, &p.protocol),
            CatalogPurpose::Translation,
            &materials.lease(&p.protocol),
            &options(&p.protocol),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, code);
        safe(&e);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn material_and_purpose_mismatches_are_rejected_before_any_socket_accepts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let materials = Materials::new();
    for protocol in PROTOCOLS {
        let p = plan(&base, protocol);
        let c = connection(&base, protocol);
        for purpose in [CatalogPurpose::Grammar, CatalogPurpose::ContextWord] {
            assert_eq!(
                translation::execute(
                    &p,
                    &c,
                    purpose,
                    &materials.lease(protocol),
                    &options(protocol),
                    |_| {}
                )
                .await
                .unwrap_err()
                .code,
                "purpose_not_supported"
            );
        }
        assert_eq!(
            translation::execute(
                &p,
                &c,
                CatalogPurpose::Translation,
                &ProviderAuthLease::None,
                &options(protocol),
                |_| {}
            )
            .await
            .unwrap_err()
            .code,
            "credential_kind_mismatch"
        );
        for other in PROTOCOLS {
            let other_kind = if matches!(
                other,
                "deepl_v2" | "google_translate_v2" | "tencent_tokenhub_translation"
            ) {
                "api"
            } else {
                other
            };
            let kind = if matches!(
                protocol,
                "deepl_v2" | "google_translate_v2" | "tencent_tokenhub_translation"
            ) {
                "api"
            } else {
                protocol
            };
            if kind != other_kind {
                assert_eq!(
                    translation::execute(
                        &p,
                        &c,
                        CatalogPurpose::Translation,
                        &materials.lease(other),
                        &options(protocol),
                        |_| {}
                    )
                    .await
                    .unwrap_err()
                    .code,
                    "credential_kind_mismatch",
                    "{protocol}/{other}"
                );
            }
        }
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unsupported_parameters_bad_routes_and_identity_fail_before_network() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let materials = Materials::new();
    for protocol in PROTOCOLS {
        let mut p = plan(&base, protocol);
        let c = connection(&base, protocol);
        p.parameters.insert("temperature".into(), json!(0));
        assert_eq!(
            translation::execute(
                &p,
                &c,
                CatalogPurpose::Translation,
                &materials.lease(protocol),
                &options(protocol),
                |_| {}
            )
            .await
            .unwrap_err()
            .code,
            "unsupported_parameter"
        );
        p.parameters.clear();
        p.structured = true;
        assert_eq!(
            translation::preflight(&p, &c, CatalogPurpose::Translation, &options(protocol))
                .unwrap_err()
                .code,
            "unsupported_structured_output"
        );
        p.structured = false;
        if protocol != "tencent_tokenhub_translation" {
            p.stream = true;
            assert_eq!(
                translation::preflight(&p, &c, CatalogPurpose::Translation, &options(protocol))
                    .unwrap_err()
                    .code,
                "stream_not_supported"
            );
            p.stream = false;
        }
        p.target_language = "auto".into();
        assert_eq!(
            translation::preflight(&p, &c, CatalogPurpose::Translation, &options(protocol))
                .unwrap_err()
                .code,
            "invalid_language"
        );
        p.target_language = "zh-CN".into();
        let mut wrong = c.clone();
        wrong.endpoint = Some(format!("{base}/other"));
        assert_eq!(
            translation::preflight(&p, &wrong, CatalogPurpose::Translation, &options(protocol))
                .unwrap_err()
                .code,
            "protocol_mismatch"
        );
        wrong = c.clone();
        wrong.protocol = "openai_compatible".into();
        assert_eq!(
            translation::preflight(&p, &wrong, CatalogPurpose::Translation, &options(protocol))
                .unwrap_err()
                .code,
            "protocol_mismatch"
        );
    }
    let p = plan(&base, "google_translate_v3");
    let c = connection(&base, &p.protocol);
    for route in [
        None,
        Some(GoogleV3Route {
            project_id: "../escape".into(),
            location: "global".into(),
        }),
        Some(GoogleV3Route {
            project_id: "project".into(),
            location: "global?key=bad".into(),
        }),
        Some(GoogleV3Route {
            project_id: "..".into(),
            location: "global".into(),
        }),
        Some(GoogleV3Route {
            project_id: "project".into(),
            location: ".".into(),
        }),
    ] {
        let o = TranslationWireOptions {
            google_v3: route,
            tokenhub: None,
        };
        assert!(translation::preflight(&p, &c, CatalogPurpose::Translation, &o).is_err());
    }
    let mut bad = Materials::new();
    bad.google.scopes_hash = "not-a-scope-fingerprint".into();
    assert_eq!(
        translation::execute(
            &p,
            &c,
            CatalogPurpose::Translation,
            &bad.lease(&p.protocol),
            &options(&p.protocol),
            |_| {}
        )
        .await
        .unwrap_err()
        .code,
        "invalid_authorization_identity"
    );
    let bad_key = SecretText::from_bytes(b"not-a-valid\r\nheader".to_vec()).unwrap();
    let api = ProviderAuthLease::ApiKey {
        source: &materials.version,
        key: &bad_key,
    };
    let p = plan(&base, "deepl_v2");
    assert_eq!(
        translation::execute(
            &p,
            &connection(&base, &p.protocol),
            CatalogPurpose::Translation,
            &api,
            &options(&p.protocol),
            |_| {}
        )
        .await
        .unwrap_err()
        .code,
        "invalid_credential"
    );
    let p = plan(&format!("{base}/?key=not-allowed"), "deepl_v2");
    assert_eq!(
        translation::preflight(
            &p,
            &connection(&p.endpoint, &p.protocol),
            CatalogPurpose::Translation,
            &options(&p.protocol)
        )
        .unwrap_err()
        .code,
        "invalid_endpoint"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn tokenhub_option_boundaries_and_invalid_deltas_are_rejected_before_observers() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let materials = Materials::new();
    let p = plan(&base, "tencent_tokenhub_translation");
    let c = connection(&base, &p.protocol);
    let reference = TranslationReference {
        kind: ReferenceKind::Term,
        text: "桜".into(),
        translation: "樱花".into(),
    };
    for invalid in [
        TokenHubTranslationOptions {
            references: vec![reference.clone(); 11],
            ..Default::default()
        },
        TokenHubTranslationOptions {
            glossary_ids: vec!["id".into(); 11],
            ..Default::default()
        },
        TokenHubTranslationOptions {
            glossary_ids: vec!["query?secret".into()],
            ..Default::default()
        },
        TokenHubTranslationOptions {
            context: Some("context\0invalid".into()),
            ..Default::default()
        },
        TokenHubTranslationOptions {
            references: vec![TranslationReference {
                text: "".into(),
                ..reference
            }],
            ..Default::default()
        },
    ] {
        let o = TranslationWireOptions {
            google_v3: None,
            tokenhub: Some(invalid),
        };
        assert_eq!(
            translation::execute(
                &p,
                &c,
                CatalogPurpose::Translation,
                &materials.lease(&p.protocol),
                &o,
                |_| {}
            )
            .await
            .unwrap_err()
            .code,
            "invalid_reference"
        );
    }
    let o = TranslationWireOptions {
        google_v3: Some(GoogleV3Route {
            project_id: "project".into(),
            location: "global".into(),
        }),
        tokenhub: None,
    };
    assert_eq!(
        translation::preflight(&p, &c, CatalogPurpose::Translation, &o)
            .unwrap_err()
            .code,
        "unsupported_parameter"
    );
    let mut p = p;
    p.model = None;
    assert_eq!(
        translation::preflight(&p, &c, CatalogPurpose::Translation, &options(&p.protocol))
            .unwrap_err()
            .code,
        "missing_model"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
    let body = format!(
        "data: {}\n\n",
        json!({"choices":[{"delta":{"content":"bad\u{0}text"},"finish_reason":"stop"}]})
    );
    let (base, _, task) = server(200, "text/event-stream", body, true).await;
    let mut p = plan(&base, "tencent_tokenhub_translation");
    p.stream = true;
    let updates = Mutex::new(Vec::new());
    let error = translation::execute(
        &p,
        &connection(&base, &p.protocol),
        CatalogPurpose::Translation,
        &materials.lease(&p.protocol),
        &options(&p.protocol),
        |s| updates.lock().unwrap().push(s.to_owned()),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "malformed_response");
    assert!(updates.lock().unwrap().is_empty());
    task.await.unwrap();
}

#[tokio::test]
async fn http_vendor_and_payload_errors_are_classified_without_echoing_secrets_or_source() {
    let materials = Materials::new();
    let cases = [
        (
            "deepl_v2",
            401,
            json!({"message":"untrusted-server-detail"}),
            "authentication_failed",
        ),
        (
            "google_translate_v2",
            429,
            json!({"error":{"message":"untrusted-server-detail"}}),
            "rate_limited",
        ),
        (
            "deepl_v2",
            456,
            json!({"message":"untrusted-server-detail"}),
            "quota_exceeded",
        ),
        (
            "google_translate_v3",
            500,
            json!({"error":{"message":"untrusted-server-detail"}}),
            "service_unavailable",
        ),
        (
            "baidu_vip",
            200,
            json!({"error_code":"54001","error_msg":"untrusted-server-detail"}),
            "authentication_failed",
        ),
        (
            "baidu_vip",
            200,
            json!({"error_code":"54003","error_msg":"untrusted-server-detail"}),
            "rate_limited",
        ),
        (
            "baidu_vip",
            200,
            json!({"error_code":"54004"}),
            "quota_exceeded",
        ),
        (
            "baidu_vip",
            200,
            json!({"error_code":"58001"}),
            "unsupported_language",
        ),
        (
            "youdao_v3",
            200,
            json!({"errorCode":"202","msg":"untrusted-server-detail"}),
            "authentication_failed",
        ),
        ("youdao_v3", 200, json!({"errorCode":"411"}), "rate_limited"),
        (
            "youdao_v3",
            200,
            json!({"errorCode":"401"}),
            "quota_exceeded",
        ),
        (
            "youdao_v3",
            200,
            json!({"errorCode":"102"}),
            "unsupported_language",
        ),
        (
            "youdao_v3",
            200,
            json!({"errorCode":"103"}),
            "request_too_large",
        ),
        (
            "tencent_tokenhub_translation",
            200,
            json!({"error":{"message":"untrusted-server-detail"}}),
            "translation_error",
        ),
    ];
    for (protocol, status, mut body, code) in cases {
        body["sensitive_detail"] = json!(format!("{SECRET} raw-source-sensitive"));
        let (base, _, task) = server(status, "application/json", body.to_string(), false).await;
        let mut p = plan(&base, protocol);
        p.source_text = "raw-source-sensitive".into();
        let error = translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, code, "{protocol}");
        safe(&error);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn malformed_empty_and_oversized_responses_cannot_be_successful_translations() {
    let materials = Materials::new();
    for protocol in PROTOCOLS {
        for (body, code) in [
            ("not json".into(), "malformed_response"),
            (response(protocol, "").to_string(), "empty_response"),
            ("{}".into(), "malformed_response"),
        ] {
            let (base, _, task) = server(200, "application/json", body, false).await;
            let p = plan(&base, protocol);
            let error = translation::execute(
                &p,
                &connection(&base, protocol),
                CatalogPurpose::Translation,
                &materials.lease(protocol),
                &options(protocol),
                |_| {},
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, code, "{protocol}");
            safe(&error);
            task.await.unwrap();
        }
    }
    let (base, _, task) = server(
        200,
        "application/json",
        response("deepl_v2", &"大".repeat(700_000)).to_string(),
        false,
    )
    .await;
    let p = plan(&base, "deepl_v2");
    assert_eq!(
        translation::execute(
            &p,
            &connection(&base, &p.protocol),
            CatalogPurpose::Translation,
            &materials.lease(&p.protocol),
            &options(&p.protocol),
            |_| {}
        )
        .await
        .unwrap_err()
        .code,
        "response_too_large"
    );
    task.await.unwrap();
    let (base, _, task) = server(
        200,
        "application/json",
        json!({"translations":[{"text":"a"},{"text":"b"}]}).to_string(),
        false,
    )
    .await;
    let p = plan(&base, "deepl_v2");
    assert_eq!(
        translation::execute(
            &p,
            &connection(&base, &p.protocol),
            CatalogPurpose::Translation,
            &materials.lease(&p.protocol),
            &options(&p.protocol),
            |_| {}
        )
        .await
        .unwrap_err()
        .code,
        "malformed_response"
    );
    task.await.unwrap();
}

#[tokio::test]
async fn request_boundaries_count_utf8_bytes_or_unicode_characters_as_required() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let materials = Materials::new();
    for (protocol, text) in [
        ("baidu_vip", "日".repeat(2001)),
        ("youdao_v3", "😀".repeat(5001)),
        ("deepl_v2", "a".repeat(128 * 1024)),
        ("google_translate_v2", "a".repeat(2 * 1024 * 1024 + 1)),
        (
            "tencent_tokenhub_translation",
            "a".repeat(2 * 1024 * 1024 + 1),
        ),
    ] {
        let mut p = plan(&base, protocol);
        p.source_text = text;
        assert_eq!(
            translation::execute(
                &p,
                &connection(&base, protocol),
                CatalogPurpose::Translation,
                &materials.lease(protocol),
                &options(protocol),
                |_| {}
            )
            .await
            .unwrap_err()
            .code,
            "request_too_large",
            "{protocol}"
        );
    }
    for text in ["  ", "hello\0world"] {
        let mut p = plan(&base, "deepl_v2");
        p.source_text = text.into();
        assert_eq!(
            translation::preflight(
                &p,
                &connection(&base, &p.protocol),
                CatalogPurpose::Translation,
                &options(&p.protocol)
            )
            .unwrap_err()
            .code,
            "invalid_input"
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
    // The exact limits are accepted on the actual wire; these tests cannot be satisfied by
    // simply moving the rejection threshold below the documented service boundary.
    for (protocol, text) in [
        ("baidu_vip", "日".repeat(2000)),
        ("youdao_v3", "😀".repeat(5000)),
    ] {
        let (base, rx, task) = server(
            200,
            "application/json",
            response(protocol, "译文").to_string(),
            false,
        )
        .await;
        let mut p = plan(&base, protocol);
        p.source_text = text.clone();
        translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(rx.await.unwrap().form()["q"], text);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn body_timeout_and_redirect_do_not_leak_or_follow_credentials() {
    let materials = Materials::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_request(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{").await.unwrap();
        tokio::time::sleep(Duration::from_millis(180)).await;
    });
    let mut p = plan(&base, "deepl_v2");
    p.timeout_ms = 60;
    let e = translation::execute(
        &p,
        &connection(&base, &p.protocol),
        CatalogPurpose::Translation,
        &materials.lease(&p.protocol),
        &options(&p.protocol),
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "timeout");
    safe(&e);
    task.await.unwrap();
    let redirect_target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let sink = redirect_target.local_addr().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_request(&mut socket).await;
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{sink}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let p = plan(&base, "deepl_v2");
    let e = translation::execute(
        &p,
        &connection(&base, &p.protocol),
        CatalogPurpose::Translation,
        &materials.lease(&p.protocol),
        &options(&p.protocol),
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "http_error");
    safe(&e);
    task.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), redirect_target.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn google_models_are_explicit_resources_bound_to_the_current_project_and_location() {
    let materials = Materials::new();
    for suffix in [
        None,
        Some("model-123"),
        Some("general/nmt"),
        Some("general/translation-llm"),
        Some("translation-llm-custom/endpoint-123"),
    ] {
        let (base, rx, task) = server(
            200,
            "application/json",
            response("google_translate_v3", "译文").to_string(),
            false,
        )
        .await;
        let mut p = plan(&format!("{base}/v3"), "google_translate_v3");
        p.model = suffix.map(|s| format!("projects/wire-project/locations/global/models/{s}"));
        translation::execute(
            &p,
            &connection(&p.endpoint, &p.protocol),
            CatalogPurpose::Translation,
            &materials.lease(&p.protocol),
            &options(&p.protocol),
            |_| {},
        )
        .await
        .unwrap();
        let request = rx.await.unwrap();
        task.await.unwrap();
        let body = request.json();
        match &p.model {
            Some(model) => assert_eq!(body["model"], *model),
            None => assert!(body.get("model").is_none()),
        }
        assert!(
            request.head.starts_with(
                "POST /v3/projects/wire-project/locations/global:translateText HTTP/1.1"
            )
        );
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut p = plan(&base, "google_translate_v3");
    for invalid in [
        "",
        " ",
        "general/nmt",
        "https://translate.googleapis.com/v3/projects/wire-project/locations/global/models/general/nmt",
        "projects/other-project/locations/global/models/general/nmt",
        "projects/wire-project/locations/us-central1/models/general/nmt",
        "projects/wire-project/locations/global/models/..",
        "projects/wire-project/locations/global/models/model%2Fescape",
        "projects/wire-project/locations/global/models/model?secret=value",
        "projects/wire-project/locations/global/models/model#fragment",
        "projects/wire-project/locations/global/models/general/unknown",
        "projects/wire-project/locations/global/models/general/nmt/extra",
        "projects/wire-project/locations/global/models/translation-llm-custom/.",
        "projects/wire-project/locations/global/models/translation-llm-custom/id/extra",
    ] {
        p.model = Some(invalid.into());
        let error = translation::preflight(
            &p,
            &connection(&base, &p.protocol),
            CatalogPurpose::Translation,
            &options(&p.protocol),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_model", "{invalid}");
        safe(&error);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn google_basic_json_byte_limit_and_advanced_codepoint_limit_are_public_admission_checks() {
    let materials = Materials::new();
    let overhead =
        serde_json::to_vec(&json!({"q":[""],"source":"ja","target":"zh-TW","format":"text"}))
            .unwrap()
            .len();
    let basic_payload = 100_000 - overhead;
    let emoji_basic = format!(
        "{}{}",
        "😀".repeat(basic_payload / 4),
        "a".repeat(basic_payload % 4)
    );
    for (protocol, source, expected_wire_bytes) in [
        (
            "google_translate_v2",
            "a".repeat(basic_payload),
            Some(100_000),
        ),
        ("google_translate_v2", emoji_basic.clone(), Some(100_000)),
        ("google_translate_v3", "a".repeat(30_000), None),
        ("google_translate_v3", "😀".repeat(30_000), None),
    ] {
        let (base, rx, task) = server(
            200,
            "application/json",
            response(protocol, "译文").to_string(),
            false,
        )
        .await;
        let mut p = plan(&base, protocol);
        p.source_text = source.clone();
        translation::preflight(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &options(protocol),
        )
        .unwrap();
        translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap();
        let request = rx.await.unwrap();
        task.await.unwrap();
        if let Some(expected) = expected_wire_bytes {
            assert_eq!(request.body.len(), expected);
            assert_eq!(request.json()["q"][0], source);
        } else {
            assert_eq!(request.json()["contents"][0], source);
            assert_eq!(source.chars().count(), 30_000);
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    for (protocol, source) in [
        ("google_translate_v2", "a".repeat(basic_payload + 1)),
        ("google_translate_v2", format!("{emoji_basic}a")),
        ("google_translate_v2", format!("x{}", "\n".repeat(60_000))),
        ("google_translate_v2", "a".repeat(100_000)),
        ("google_translate_v3", "a".repeat(30_001)),
        ("google_translate_v3", "😀".repeat(30_001)),
    ] {
        let mut p = plan(&base, protocol);
        p.source_text = source;
        assert_eq!(
            translation::preflight(
                &p,
                &connection(&base, protocol),
                CatalogPurpose::Translation,
                &options(protocol)
            )
            .unwrap_err()
            .code,
            "request_too_large",
            "{protocol}"
        );
        // No material is supplied: public admission must win before lease access or HTTP.
        assert_eq!(
            translation::execute(
                &p,
                &connection(&base, protocol),
                CatalogPurpose::Translation,
                &ProviderAuthLease::None,
                &options(protocol),
                |_| {}
            )
            .await
            .unwrap_err()
            .code,
            "request_too_large"
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn google_403_quota_rate_and_permission_errors_use_bounded_structured_facts_only() {
    let materials = Materials::new();
    for protocol in ["google_translate_v2", "google_translate_v3"] {
        for (mut body, expected) in [
            (
                json!({"error":{"errors":[{"reason":"dailyLimitExceeded"}],"code":403}}),
                "quota_exceeded",
            ),
            (
                json!({"error":{"errors":[{"reason":"quotaExceeded"}],"code":403}}),
                "quota_exceeded",
            ),
            (
                json!({"error":{"errors":[{"reason":"userRateLimitExceeded"}],"code":403}}),
                "rate_limited",
            ),
            (
                json!({"error":{"errors":[{"reason":"rateLimitExceeded"}],"code":403}}),
                "rate_limited",
            ),
            (
                json!({"error":{"status":"RESOURCE_EXHAUSTED","code":403}}),
                "quota_exceeded",
            ),
            (
                json!({"error":{"status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"RATE_LIMIT_EXCEEDED"}]}}),
                "rate_limited",
            ),
            (
                json!({"error":{"status":"PERMISSION_DENIED","errors":[{"reason":"insufficientPermissions"}]}}),
                "authentication_failed",
            ),
            (
                json!({"error":{"status":"PERMISSION_DENIED","message":"dailyLimitExceeded quotaExceeded RESOURCE_EXHAUSTED"}}),
                "authentication_failed",
            ),
            (
                json!({"error":{"details":[{"@type":"arbitrary","reason":"quotaExceeded"}]}}),
                "authentication_failed",
            ),
        ] {
            body["sensitive_detail"] = json!(format!(
                "{SECRET} raw-source-sensitive untrusted-server-detail"
            ));
            let (base, _, task) = server(403, "application/json", body.to_string(), false).await;
            let p = plan(&base, protocol);
            let error = translation::execute(
                &p,
                &connection(&base, protocol),
                CatalogPurpose::Translation,
                &materials.lease(protocol),
                &options(protocol),
                |_| {},
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, expected, "{protocol}");
            safe(&error);
            task.await.unwrap();
        }
        let (base, _, task) = server(
            403,
            "application/json",
            "not-json untrusted-server-detail".into(),
            false,
        )
        .await;
        let p = plan(&base, protocol);
        let error = translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "authentication_failed");
        safe(&error);
        task.await.unwrap();
        let (base, _, task) = server(
            403,
            "application/json",
            json!({"error":{"message":"x".repeat(2*1024*1024+1)}}).to_string(),
            false,
        )
        .await;
        let p = plan(&base, protocol);
        let error = translation::execute(
            &p,
            &connection(&base, protocol),
            CatalogPurpose::Translation,
            &materials.lease(protocol),
            &options(protocol),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "response_too_large");
        safe(&error);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn service_account_access_wire_has_its_own_pool_identity_without_a_fabricated_user_grant() {
    use app_core::domain::model_config::credentials::GoogleServiceAccountLeaseIdentity;
    let materials = Materials::new();
    let (base, rx, server) = server(
        200,
        "application/json",
        response("google_translate_v3", "雨停了。").to_string(),
        false,
    )
    .await;
    let mut selected_connection = connection(&base, "google_translate_v3");
    selected_connection.auth_mode = "google_service_account".into();
    let identity = GoogleServiceAccountLeaseIdentity {
        pool_id: "actual-synthetic-pool".into(),
        pool_revision: NonZeroU32::new(2).unwrap(),
        member_id: "synthetic-member".into(),
        credential_version: materials.version.clone(),
        lifecycle_revision: 3,
        scopes_hash: "a".repeat(64),
    };
    let auth = ProviderAuthLease::GoogleServiceAccountAccess {
        token: &materials.secret,
        identity: &identity,
    };
    let result = translation::execute(
        &plan(&base, "google_translate_v3"),
        &selected_connection,
        CatalogPurpose::Translation,
        &auth,
        &options("google_translate_v3"),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(result.text, "雨停了。");
    let request = rx.await.unwrap();
    server.await.unwrap();
    assert!(
        request
            .head
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {SECRET}").to_ascii_lowercase())
    );
    assert!(!String::from_utf8(request.body).unwrap().contains("grant"));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut invalid = identity;
    invalid.pool_id.clear();
    let invalid_auth = ProviderAuthLease::GoogleServiceAccountAccess {
        token: &materials.secret,
        identity: &invalid,
    };
    let result = translation::execute(
        &plan(&base, "google_translate_v3"),
        &connection(&base, "google_translate_v3"),
        CatalogPurpose::Translation,
        &invalid_auth,
        &options("google_translate_v3"),
        |_| {},
    )
    .await;
    assert!(result.is_err());
    assert!(
        tokio::time::timeout(Duration::from_millis(25), listener.accept())
            .await
            .is_err(),
        "invalid source identity must fail before network"
    );
}
