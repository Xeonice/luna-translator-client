//! Shared genuine SQL/Vault/socket setup only. No tests or runtime bypasses.
use app_core::{
    application::service_discovery::{
        LiveServiceModelDiscoveries, ServiceModelDiscoveryController,
    },
    contracts::*,
    domain::model_config::{
        credentials::*, service_draft::compile_service_draft, service_graph::*,
    },
    ports::{
        IdentitySource, ProductRepository, ServiceConfigurationRepository,
        runtime::CredentialVault, service_discovery::ServiceModelDiscoveryRepository,
    },
};
use app_infra::{
    providers::{
        builtin_registry, gateway::HttpModelGateway,
        service_discovery::HttpSavedServiceModelDiscovery,
    },
    storage::SqliteStore,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
pub const TEST_KEY: &str = "controlled-model-list-api-key";
pub struct Ids;
impl IdentitySource for Ids {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        panic!("pure draft compiler does not read a clock")
    }
}
#[derive(Default)]
pub struct Vault {
    pub reads: AtomicUsize,
    pub writes: AtomicUsize,
    pub deletes: AtomicUsize,
    pub values: Mutex<BTreeMap<String, Vec<u8>>>,
    pub pause: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
}
impl CredentialVault for Vault {
    fn get(&self, id: &str) -> AppResult<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let pause = self.pause.lock().unwrap().take();
        if let Some((entered, release)) = pause {
            entered.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        self.values.lock().unwrap().get(id).cloned().ok_or_else(|| {
            AppError::new(
                "credential_unavailable",
                "vault",
                "controlled fixture has no material",
            )
        })
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Err(AppError::new(
            "unexpected_vault_write",
            "vault",
            "model discovery cannot write material",
        ))
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        Err(AppError::new(
            "unexpected_vault_delete",
            "vault",
            "model discovery cannot delete material",
        ))
    }
}
pub struct Fixture {
    pub store: Arc<SqliteStore>,
    pub path: PathBuf,
    pub vault: Arc<Vault>,
    pub live: Arc<LiveServiceModelDiscoveries>,
    pub repository: Arc<dyn ServiceModelDiscoveryRepository>,
    pub generation: std::num::NonZeroU32,
}
impl Default for Fixture {
    fn default() -> Self {
        let dir = std::env::temp_dir().join(format!("source-model-list-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.sqlite3");
        // All fixtures intentionally share labels: only actual writer ownership differs.
        let store = Arc::new(
            SqliteStore::open(&path, "model-list-profile", "model-list-server", 10).unwrap(),
        );
        store
            .configure_service_directory(Arc::new(builtin_registry().unwrap()))
            .unwrap();
        let generation = store.service_configuration().unwrap().profile_generation;
        let vault = Arc::new(Vault::default());
        store
            .begin_credential_material(
                CredentialReference {
                    credential_id: "key".into(),
                    credential_version_id: "opaque-model-key".into(),
                    pool_id: "model-pool".into(),
                    pool_revision: 1,
                    label: "controlled".into(),
                    masked_hint: "masked".into(),
                },
                CredentialStorageMetadata::EnvelopeV1 {
                    kind: CredentialMaterialKind::ApiKey,
                },
                11,
            )
            .unwrap();
        store.activate_credential("key".into(), 12).unwrap();
        let material = CredentialMaterial::ApiKey {
            key: SecretText::from_bytes(TEST_KEY.as_bytes().to_vec()).unwrap(),
        };
        vault.values.lock().unwrap().insert(
            "opaque-model-key".into(),
            encode_vault_credential(&material)
                .unwrap()
                .expose()
                .to_vec(),
        );
        let live = Arc::new(LiveServiceModelDiscoveries::default());
        let repository = store
            .service_model_discovery_repository(live.clone())
            .unwrap();
        Self {
            store,
            path,
            vault,
            live,
            repository,
            generation,
        }
    }
}
impl Fixture {
    pub fn save(
        &self,
        preset: &str,
        endpoint: &str,
        authenticated: bool,
        extra: Vec<(&str, CatalogScalar)>,
    ) -> SavedServiceRef {
        self.save_edit(preset, endpoint, authenticated, extra, None)
    }
    pub fn save_edit(
        &self,
        preset: &str,
        endpoint: &str,
        authenticated: bool,
        extra: Vec<(&str, CatalogScalar)>,
        editing: Option<SavedServiceRef>,
    ) -> SavedServiceRef {
        let state = self.store.service_configuration().unwrap();
        let mut values: BTreeMap<String, CatalogScalar> = [
            (
                "name".into(),
                CatalogScalar::Text("source-bound list".into()),
            ),
            (
                "connection_mode".into(),
                CatalogScalar::Text("custom".into()),
            ),
            ("endpoint".into(), CatalogScalar::Text(endpoint.into())),
            (
                "auth_mode".into(),
                CatalogScalar::Text(if authenticated { "api_key" } else { "none" }.into()),
            ),
            ("proxy_mode".into(), CatalogScalar::Text("disabled".into())),
            (
                "template".into(),
                CatalogScalar::Text("Translate {{source}}".into()),
            ),
        ]
        .into_iter()
        .collect();
        for (k, v) in extra {
            values.insert(k.into(), v);
        }
        let input = SaveServiceDraftInput {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_profile_generation: self.generation,
            expected_draft_head_revision: state.draft_head_revision,
            editing,
            target: ServiceFormTarget::Http {
                preset: ProviderPresetKey {
                    preset_id: preset.into(),
                    preset_revision: 1,
                },
            },
            purpose: CatalogPurpose::Translation,
            values,
            authentication: if authenticated {
                ServiceAuthenticationSelection::ExistingPool {
                    pool_id: "model-pool".into(),
                    pool_revision: 1.try_into().unwrap(),
                }
            } else {
                ServiceAuthenticationSelection::None {}
            },
        };
        let compiled = compile_service_draft(
            &state.draft,
            &input,
            &builtin_registry().unwrap(),
            &Ids,
            &Default::default(),
        )
        .unwrap();
        self.store
            .save_service_configuration(&compiled, 20)
            .unwrap()
            .saved_resource
    }
    pub fn controller(&self) -> Arc<ServiceModelDiscoveryController> {
        let product: Arc<dyn ProductRepository> = self.store.clone();
        self.with_gateway(Arc::new(HttpModelGateway::new(product, self.vault.clone())))
    }
    pub fn with_gateway(
        &self,
        gateway: Arc<HttpModelGateway>,
    ) -> Arc<ServiceModelDiscoveryController> {
        ServiceModelDiscoveryController::new(
            Arc::new(HttpSavedServiceModelDiscovery::new(
                gateway,
                self.repository.clone(),
            )),
            self.live.clone(),
        )
    }
    pub fn input(&self, source: SavedServiceRef, id: &str) -> ServiceModelDiscoveryInput {
        // Generation is setup metadata, never an extra preflight SQL read.
        ServiceModelDiscoveryInput {
            operation_id: id.into(),
            request: SavedServiceModelDiscoveryRequest {
                expected_profile_generation: self.generation,
                source,
                purpose: CatalogPurpose::Translation,
            },
        }
    }
    pub fn graph(&self, source: &SavedServiceRef) -> ConfigurationV3 {
        let revision = match source {
            SavedServiceRef::Http { source } => &source.revision_id,
            SavedServiceRef::Offline { source } => &source.revision_id,
        };
        let ConfigurationDocument::ServiceGraph { configuration } =
            self.store.service_configuration_revision(revision).unwrap()
        else {
            panic!("new source must be actual graph")
        };
        configuration
    }
    pub fn rows(&self) -> BTreeMap<String, String> {
        let c = rusqlite::Connection::open(&self.path).unwrap();
        let tables=c.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
        tables
            .into_iter()
            .map(|name| {
                let mut q = c
                    .prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")))
                    .unwrap();
                let count = q.column_count();
                let mut rows = q
                    .query_map([], |r| {
                        let row = (0..count)
                            .map(|i| match r.get_ref(i).unwrap() {
                                rusqlite::types::ValueRef::Null => Value::Null,
                                rusqlite::types::ValueRef::Integer(v) => json!(v),
                                rusqlite::types::ValueRef::Real(v) => json!(v),
                                rusqlite::types::ValueRef::Text(v) => {
                                    json!(String::from_utf8_lossy(v))
                                }
                                rusqlite::types::ValueRef::Blob(v) => json!(v),
                            })
                            .collect::<Vec<_>>();
                        Ok(serde_json::to_string(&row).unwrap())
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                rows.sort();
                (
                    name,
                    format!("{:x}", Sha256::digest(serde_json::to_vec(&rows).unwrap())),
                )
            })
            .collect()
    }
}
pub struct Reply {
    pub status: u16,
    pub body: String,
    pub headers: String,
}
impl Reply {
    pub fn ok(value: Value) -> Self {
        Self {
            status: 200,
            body: value.to_string(),
            headers: String::new(),
        }
    }
}
pub struct Server {
    pub base: String,
    pub requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(replies: Vec<Reply>) -> Self {
        Self::start(replies, None)
    }
    pub fn paused(replies: Vec<Reply>) -> (Self, mpsc::SyncSender<()>) {
        let (release, wait) = mpsc::sync_channel(1);
        (Self::start(replies, Some(wait)), release)
    }
    fn start(replies: Vec<Reply>, mut pause: Option<mpsc::Receiver<()>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let saved = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut replies = VecDeque::from(replies);
            while !done.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut block = [0u8; 4096];
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") && bytes.len() < 65536 {
                    match stream.read(&mut block) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bytes.extend_from_slice(&block[..n]),
                    }
                }
                saved
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&bytes).into_owned());
                if let Some(wait) = pause.take() {
                    let _ = wait.recv_timeout(Duration::from_secs(5));
                }
                let reply = replies.pop_front().unwrap_or(Reply {
                    status: 500,
                    body: "{}".into(),
                    headers: String::new(),
                });
                let response = format!(
                    "HTTP/1.1 {} Controlled\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                    reply.status,
                    reply.body.len(),
                    reply.headers,
                    reply.body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self {
            base,
            requests,
            stop,
            thread: Some(thread),
        }
    }
    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
    pub async fn wait_request(&self, n: usize) {
        let started = Instant::now();
        while self.count() < n {
            assert!(
                started.elapsed() < Duration::from_secs(4),
                "controlled listener did not receive expected request"
            );
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
pub fn models(result: ServiceModelDiscoveryResult) -> Vec<String> {
    let ServiceModelDiscoveryOutcome::Available { models } = result.outcome else {
        panic!(
            "expected genuine source-qualified list: {:?}",
            result.outcome
        )
    };
    models.into_iter().map(|m| m.id).collect()
}
