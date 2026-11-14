//! The application gateway rejects invalid identities before touching the Vault.
use app_core::{
    application::request_plan::RequestPlan,
    contracts::*,
    ports::runtime::{CredentialVault, ModelGateway},
};
use app_infra::{
    providers::{self, gateway::HttpModelGateway},
    storage::SqliteStore,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct CountingVault(AtomicUsize);
impl CredentialVault for CountingVault {
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(b"DO-NOT-READ".to_vec())
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        panic!("read-only admission test must not write secrets")
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        panic!("read-only admission test must not delete secrets")
    }
}
fn connection() -> Connection {
    Connection {
        connection_id: "admission".into(),
        display_name: "Test".into(),
        protocol: "openai_compatible".into(),
        deployment: "local".into(),
        auth_mode: "api_key".into(),
        endpoint: Some("http://127.0.0.1:1/v1".into()),
        credential_pool_id: None,
        credential_pool_revision: None,
        proxy_mode: "disabled".into(),
        network_metadata: BTreeMap::new(),
        availability_intent: true,
        adapter_binding: None,
        preset_source: None,
    }
}
fn plan(c: &Connection) -> RequestPlan {
    RequestPlan {
        adapter_binding: None,
        purpose: None,
        protocol: c.protocol.clone(),
        endpoint: c.endpoint.clone().unwrap(),
        model: Some("test".into()),
        source_text: "source-not-to-send".into(),
        source_language: "ja".into(),
        target_language: "zh-CN".into(),
        messages: vec![],
        parameters: BTreeMap::new(),
        structured: false,
        stream: false,
        timeout_ms: 1000,
    }
}
#[tokio::test]
async fn invalid_protocol_endpoint_auth_and_unbound_native_never_read_vault() {
    let directory =
        std::env::temp_dir().join(format!("luna-gateway-admission-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let repository = Arc::new(
        SqliteStore::open(
            directory.join("reader.sqlite3"),
            "admission-profile",
            "admission-server",
            1,
        )
        .unwrap(),
    );
    let vault = Arc::new(CountingVault(AtomicUsize::new(0)));
    let gateway = HttpModelGateway::new(repository.clone(), vault.clone());
    let mut cases = Vec::new();
    let mut c = connection();
    c.protocol = "unknown".into();
    cases.push((c, "unsupported_protocol"));
    let mut c = connection();
    c.auth_mode = "arbitrary_header".into();
    cases.push((c, "unsupported_authentication"));
    let mut c = connection();
    c.endpoint = Some("https://user:password@example.invalid".into());
    cases.push((c, "invalid_endpoint"));
    for protocol in [
        "openai_responses",
        "gemini_generate_content",
        "ollama_native",
        "lmstudio_native_v1",
    ] {
        let mut c = connection();
        c.protocol = protocol.into();
        cases.push((c, "adapter_binding_required"));
    }
    for (c, code) in cases {
        assert_eq!(
            gateway
                .test_request(&c, &plan(&c), Some("vault-ref"))
                .await
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(vault.0.load(Ordering::SeqCst), 0);
        assert_eq!(
            gateway
                .discover_models(&c, Some("vault-ref"))
                .await
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(vault.0.load(Ordering::SeqCst), 0);
    }
    let c = connection();
    let mut p = plan(&c);
    p.endpoint = "http://127.0.0.1:2/v1".into();
    assert_eq!(
        gateway
            .test_request(&c, &p, Some("vault-ref"))
            .await
            .unwrap_err()
            .code,
        "protocol_mismatch"
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 0);
    let mut c = connection();
    c.protocol = "azure_translator".into();
    assert!(
        gateway
            .discover_models(&c, Some("vault-ref"))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 0);
    drop(gateway);
    drop(repository);
    std::fs::remove_dir_all(directory).unwrap();
}
#[tokio::test]
async fn no_unknown_or_native_protocol_can_fall_back_to_generic_models_route() {
    for protocol in [
        "unknown",
        "openai_responses",
        "gemini_generate_content",
        "ollama_native",
        "lmstudio_native_v1",
        "deepl_v2",
    ] {
        let mut c = connection();
        c.protocol = protocol.into();
        assert_eq!(
            providers::discover_models(&c, Some("key"))
                .await
                .unwrap_err()
                .code,
            "discovery_unsupported"
        );
    }
}
