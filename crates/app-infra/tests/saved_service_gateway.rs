//! Real saved source + original live operation + SQLite + controlled loopback.
//! These checks do not grant external vendor/account, native or package acceptance.
use app_core::{
    application::google_test_operations::{LiveServiceTestOperations, ServiceTestOperationGuard},
    contracts::*,
    domain::model_config::{
        credentials::*, service_draft::compile_service_draft, service_graph::*,
    },
    ports::{
        IdentitySource, ProductRepository, ServiceConfigurationRepository,
        operation_budget::{OperationBudget, OperationTerminalDisposition},
        runtime::{CredentialVault, ModelGateway},
        service_test_execution::ServiceTestExecutionObservation,
        service_testing::*,
    },
};
use app_infra::{
    platform::SystemIdentity,
    providers::{builtin_registry, gateway::HttpModelGateway},
    storage::SqliteStore,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

#[derive(Default)]
struct Gate {
    entered: AtomicBool,
    released: Mutex<bool>,
    wake: Condvar,
}
impl Gate {
    fn stop(&self) {
        self.entered.store(true, Ordering::SeqCst);
        let mut done = self.released.lock().unwrap();
        while !*done {
            done = self.wake.wait(done).unwrap();
        }
    }
    async fn entered(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !self.entered.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
#[derive(Default)]
struct Vault {
    reads: AtomicUsize,
    bytes: Mutex<Vec<u8>>,
    pause: Mutex<Option<Arc<Gate>>>,
    revoke: Mutex<Option<(Arc<SqliteStore>, String)>>,
    fail: AtomicBool,
}
impl CredentialVault for Vault {
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let gate = self.pause.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.stop();
        }
        if let Some((repo, id)) = self.revoke.lock().unwrap().take() {
            repo.revoke_credential(id, SystemIdentity.now_ms())?;
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(error("controlled_vault_failure"));
        }
        Ok(self.bytes.lock().unwrap().clone())
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        panic!("saved test must not mutate Vault")
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        panic!("saved test must not delete Vault")
    }
}
fn error(code: &str) -> AppError {
    AppError::new(code, "controlled_test", "controlled failure")
}
struct Repository {
    actual: Arc<dyn ServiceTestingRepository>,
    calls: AtomicUsize,
    pause: Mutex<Option<(usize, Arc<Gate>)>>,
    pause_after: Mutex<Option<(usize, Arc<Gate>)>>,
    fail_at: AtomicUsize,
    cancel_at: Mutex<Option<(usize, OperationBudget)>>,
    expected: Mutex<Option<Arc<PreparedServiceTest>>>,
}
impl ServiceTestingRepository for Repository {
    fn prepare_service_test(
        &self,
        input: PrepareServiceTestInput,
        at: i64,
    ) -> AppResult<PreparedServiceTest> {
        self.actual.prepare_service_test(input, at)
    }
    fn validate_prepared_test(&self, p: &Arc<PreparedServiceTest>) -> AppResult<()> {
        if let Some(expected) = self.expected.lock().unwrap().as_ref() {
            assert!(
                Arc::ptr_eq(expected, p),
                "must preserve exact operation-owned Arc"
            );
        }
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let pause = self.pause.lock().unwrap().clone();
        if let Some((at, gate)) = pause {
            if n == at {
                gate.stop();
            }
        }
        if let Some((at, budget)) = self.cancel_at.lock().unwrap().as_ref() {
            if *at == n {
                budget.cancel();
            }
        }
        if self.fail_at.load(Ordering::SeqCst) == n {
            return Err(error("controlled_source_changed"));
        }
        let result = self.actual.validate_prepared_test(p);
        let pause = self.pause_after.lock().unwrap().clone();
        if let Some((at, gate)) = pause {
            if n == at {
                gate.stop();
            }
        }
        result
    }
    fn finish_service_test(
        &self,
        input: FinishServiceTestInput,
        at: i64,
    ) -> AppResult<ServiceTestResult> {
        self.actual.finish_service_test(input, at)
    }
    fn service_test_qualification(
        &self,
        q: ServiceTestQualificationQuery,
    ) -> AppResult<ServiceTestQualification> {
        self.actual.service_test_qualification(q)
    }
}
struct Fixture {
    store: Arc<SqliteStore>,
    path: PathBuf,
    live: Arc<LiveServiceTestOperations>,
    repo: Arc<Repository>,
    vault: Arc<Vault>,
    source: SavedServiceRef,
    gateway: Arc<HttpModelGateway>,
}
impl Fixture {
    fn new(endpoint: &str, auth: bool) -> Self {
        let path =
            std::env::temp_dir().join(format!("saved-gateway-{}.sqlite3", uuid::Uuid::new_v4()));
        let store = Arc::new(
            SqliteStore::open(
                &path,
                "gateway-profile",
                "gateway-server",
                SystemIdentity.now_ms(),
            )
            .unwrap(),
        );
        let directory = Arc::new(builtin_registry().unwrap());
        store
            .configure_service_directory(directory.clone())
            .unwrap();
        let vault = Arc::new(Vault::default());
        if auth {
            let material = CredentialMaterial::ApiKey {
                key: SecretText::from_bytes(b"controlled-key".to_vec()).unwrap(),
            };
            *vault.bytes.lock().unwrap() = encode_vault_credential(&material)
                .unwrap()
                .expose()
                .to_vec();
            store
                .begin_credential_material(
                    CredentialReference {
                        credential_id: "key".into(),
                        credential_version_id: "opaque-immutable-key-version".into(),
                        pool_id: "pool".into(),
                        pool_revision: 1,
                        label: "synthetic".into(),
                        masked_hint: "••".into(),
                    },
                    CredentialStorageMetadata::EnvelopeV1 {
                        kind: CredentialMaterialKind::ApiKey,
                    },
                    10,
                )
                .unwrap();
            store.activate_credential("key".into(), 11).unwrap();
        }
        let state = store.service_configuration().unwrap();
        let input = SaveServiceDraftInput {
            operation_id: "save-actual-source".into(),
            expected_profile_generation: state.profile_generation,
            expected_draft_head_revision: state.draft_head_revision,
            editing: None,
            target: ServiceFormTarget::Http {
                preset: ProviderPresetKey {
                    preset_id: if auth {
                        "builtin.openai-chat"
                    } else {
                        "builtin.lmstudio"
                    }
                    .into(),
                    preset_revision: 1,
                },
            },
            purpose: CatalogPurpose::Translation,
            values: BTreeMap::from([
                (
                    "name".into(),
                    CatalogScalar::Text("Saved unchanged source".into()),
                ),
                ("model".into(), CatalogScalar::Text("test-model".into())),
                (
                    "connection_mode".into(),
                    CatalogScalar::Text("custom".into()),
                ),
                ("endpoint".into(), CatalogScalar::Text(endpoint.into())),
                (
                    "auth_mode".into(),
                    CatalogScalar::Text(if auth { "api_key" } else { "none" }.into()),
                ),
                ("timeout".into(), CatalogScalar::Number(5.0)),
            ]),
            authentication: if auth {
                ServiceAuthenticationSelection::ExistingPool {
                    pool_id: "pool".into(),
                    pool_revision: std::num::NonZeroU32::new(1).unwrap(),
                }
            } else {
                ServiceAuthenticationSelection::None {}
            },
        };
        let compiled = compile_service_draft(
            &state.draft,
            &input,
            directory.as_ref(),
            &SystemIdentity,
            &Default::default(),
        )
        .unwrap();
        let source = store
            .save_service_configuration(&compiled, SystemIdentity.now_ms())
            .unwrap()
            .saved_resource;
        let live = Arc::new(LiveServiceTestOperations::default());
        let actual = store
            .service_testing_repository(live.clone(), live.clone())
            .unwrap();
        let repo = Arc::new(Repository {
            actual,
            calls: AtomicUsize::new(0),
            pause: Mutex::new(None),
            pause_after: Mutex::new(None),
            fail_at: AtomicUsize::new(0),
            cancel_at: Mutex::new(None),
            expected: Mutex::new(None),
        });
        let gateway = Arc::new(HttpModelGateway::with_registry(
            store.clone(),
            vault.clone(),
            directory,
        ));
        Self {
            store,
            path,
            live,
            repo,
            vault,
            source,
            gateway,
        }
    }
    fn raw_prepare(&self) -> PreparedServiceTest {
        self.repo
            .prepare_service_test(
                PrepareServiceTestInput {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    request: SavedServiceTestRequest {
                        expected_profile_generation: self
                            .store
                            .service_configuration()
                            .unwrap()
                            .profile_generation,
                        source: self.source.clone(),
                        purpose: CatalogPurpose::Translation,
                        input_scope: None,
                        output_mode: None,
                    },
                    selected_token: None,
                },
                SystemIdentity.now_ms(),
            )
            .unwrap()
    }
    fn prepare(&self, duration: Duration) -> ServiceTestOperationGuard {
        self.register(self.raw_prepare(), duration)
    }
    fn register(&self, p: PreparedServiceTest, duration: Duration) -> ServiceTestOperationGuard {
        let guard = self
            .live
            .register_service(p, OperationBudget::new(duration).unwrap())
            .unwrap();
        *self.repo.expected.lock().unwrap() = Some(guard.prepared());
        guard
    }
    fn counts(&self) -> (u32, u32, u32, u32, u32, u32, u32) {
        let c = rusqlite::Connection::open(&self.path).unwrap();
        c.query_row("SELECT (SELECT count(*) FROM reading_units),(SELECT count(*) FROM request_tasks),(SELECT count(*) FROM request_attempts),(SELECT count(*) FROM purpose_bindings),(SELECT count(*) FROM purpose_runtime_states),(SELECT count(*) FROM cache_entries),(SELECT count(*) FROM test_records)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap()
    }
    fn spawn(
        &self,
        guard: &ServiceTestOperationGuard,
        observation: Arc<ServiceTestExecutionObservation>,
    ) -> tokio::task::JoinHandle<AppResult<String>> {
        let gateway = self.gateway.clone();
        let prepared = guard.prepared();
        let repo = self.repo.clone();
        let budget = guard.budget();
        tokio::spawn(async move {
            gateway
                .execute_saved_service_test(prepared, repo, budget, observation)
                .await
        })
    }
}
struct Server {
    endpoint: String,
    seen: oneshot::Receiver<(String, Value)>,
    release: oneshot::Sender<()>,
    count: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
async fn server(status: u16) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (send, seen) = oneshot::channel();
    let (release, wait) = oneshot::channel();
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut data = vec![];
        let (head, body) = loop {
            let mut buffer = [0; 4096];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            data.extend_from_slice(&buffer[..n]);
            if let Some(i) = data.windows(4).position(|b| b == b"\r\n\r\n") {
                let head = String::from_utf8(data[..i].to_vec()).unwrap();
                let length = head
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                if data.len() >= i + 4 + length {
                    break (
                        head,
                        serde_json::from_slice::<Value>(&data[i + 4..i + 4 + length]).unwrap(),
                    );
                }
            }
        };
        counted.fetch_add(1, Ordering::SeqCst);
        let stream = body["stream"] == true;
        let _ = send.send((head, body));
        let _ = wait.await;
        let (mime, response) = if status != 200 {
            (
                "application/json",
                json!({"error":{"message":"controlled failure"}}).to_string(),
            )
        } else if stream {
            (
                "text/event-stream",
                "data: {\"choices\":[{\"delta\":{\"content\":\"受控译文\"}}]}\n\ndata: [DONE]\n\n"
                    .into(),
            )
        } else {
            (
                "application/json",
                json!({"choices":[{"message":{"content":"受控译文"},"finish_reason":"stop"}]})
                    .to_string(),
            )
        };
        let response = format!(
            "HTTP/1.1 {status} Controlled\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
    });
    Server {
        endpoint,
        seen,
        release,
        count,
        task,
    }
}
fn observation() -> Arc<ServiceTestExecutionObservation> {
    Arc::new(ServiceTestExecutionObservation::default())
}
async fn no_send(server: Server) {
    assert_eq!(server.count.load(Ordering::SeqCst), 0);
    server.task.abort();
}

#[tokio::test(flavor = "current_thread")]
async fn real_saved_unapplied_no_auth_wire_uses_original_arc_without_task_cache_or_head_changes() {
    for auth in [false, true] {
        let mut wire = server(200).await;
        let f = Fixture::new(&wire.endpoint, auth);
        let before = f.counts();
        let heads = f.store.service_configuration().unwrap();
        let guard = f.prepare(Duration::from_secs(5));
        let o = observation();
        let run = f.spawn(&guard, o.clone());
        let (head, body) = tokio::time::timeout(Duration::from_secs(5), &mut wire.seen)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(body["model"], "test-model");
        assert!(body["messages"].to_string().contains("雨がやんで"));
        assert_eq!(
            head.to_lowercase()
                .contains("authorization: bearer controlled-key"),
            auth
        );
        wire.release.send(()).unwrap();
        assert_eq!(run.await.unwrap().unwrap(), "受控译文");
        wire.task.await.unwrap();
        o.wait_for_physical_drain().await.unwrap();
        assert!(guard.budget().check().is_ok());
        assert_eq!(f.counts(), before);
        let after = f.store.service_configuration().unwrap();
        assert_eq!(after.draft_head_revision, heads.draft_head_revision);
        assert_eq!(after.applied_head_revision, heads.applied_head_revision);
        assert_eq!(f.vault.reads.load(Ordering::SeqCst), usize::from(auth));
        let facts = o.snapshot().unwrap();
        assert!(facts.request_started);
        assert_eq!(facts.credential_used, auth);
        assert!(facts.google_lease.is_none());
        assert!(f.repo.calls.load(Ordering::SeqCst) >= 3);
    }
}
#[tokio::test]
async fn provider_failure_keeps_operation_active_for_the_failed_terminal() {
    let mut wire = server(503).await;
    let f = Fixture::new(&wire.endpoint, false);
    let guard = f.prepare(Duration::from_secs(5));
    let o = observation();
    let run = f.spawn(&guard, o.clone());
    let _ = (&mut wire.seen).await.unwrap();
    wire.release.send(()).unwrap();
    assert!(run.await.unwrap().is_err());
    assert!(guard.budget().check().is_ok());
    assert!(o.snapshot().unwrap().request_started);
    wire.task.await.unwrap();
}
#[tokio::test]
async fn cancelled_before_start_has_no_sql_vault_or_transport_and_keeps_cancelled() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, true);
    let guard = f.prepare(Duration::from_secs(5));
    guard.budget().cancel();
    let o = observation();
    assert_eq!(
        f.spawn(&guard, o.clone()).await.unwrap().unwrap_err().code,
        "operation_cancelled"
    );
    assert_eq!(f.repo.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.vault.reads.load(Ordering::SeqCst), 0);
    assert!(!o.snapshot().unwrap().request_started);
    no_send(wire).await;
}
#[tokio::test(flavor = "current_thread")]
async fn blocked_sql_does_not_block_executor_and_cancel_waits_for_physical_drain() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, true);
    let guard = f.prepare(Duration::from_secs(5));
    let gate = Arc::new(Gate::default());
    *f.repo.pause.lock().unwrap() = Some((1, gate.clone()));
    let o = observation();
    let run = f.spawn(&guard, o.clone());
    gate.entered().await;
    guard.budget().cancel();
    assert_eq!(run.await.unwrap().unwrap_err().code, "operation_cancelled");
    assert!(
        tokio::time::timeout(Duration::from_millis(30), o.wait_for_physical_drain())
            .await
            .is_err()
    );
    assert_eq!(f.vault.reads.load(Ordering::SeqCst), 0);
    gate.release();
    o.wait_for_physical_drain().await.unwrap();
    assert!(!o.snapshot().unwrap().request_started);
    no_send(wire).await;
}
#[tokio::test(flavor = "current_thread")]
async fn dropped_execution_cancels_active_budget_but_retains_physical_vault_and_use_fact() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, true);
    let guard = f.prepare(Duration::from_secs(5));
    let gate = Arc::new(Gate::default());
    *f.vault.pause.lock().unwrap() = Some(gate.clone());
    let o = observation();
    let run = f.spawn(&guard, o.clone());
    gate.entered().await;
    run.abort();
    assert!(run.await.unwrap_err().is_cancelled());
    assert_eq!(
        guard.budget().check().unwrap_err().code,
        "operation_cancelled"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(30), o.wait_for_physical_drain())
            .await
            .is_err()
    );
    gate.release();
    o.wait_for_physical_drain().await.unwrap();
    let facts = o.snapshot().unwrap();
    assert!(facts.credential_used);
    assert!(!facts.request_started);
    no_send(wire).await;
}
#[tokio::test(flavor = "current_thread")]
async fn deadline_during_vault_stays_timed_out_instead_of_becoming_cancelled() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, true);
    let guard = f.prepare(Duration::from_millis(100));
    let gate = Arc::new(Gate::default());
    *f.vault.pause.lock().unwrap() = Some(gate.clone());
    let o = observation();
    let run = f.spawn(&guard, o.clone());
    gate.entered().await;
    assert_eq!(run.await.unwrap().unwrap_err().code, "operation_timeout");
    assert!(
        guard
            .budget()
            .check_terminal(OperationTerminalDisposition::TimedOut)
            .is_ok()
    );
    assert!(
        guard
            .budget()
            .check_terminal(OperationTerminalDisposition::Cancelled)
            .is_err()
    );
    gate.release();
    o.wait_for_physical_drain().await.unwrap();
    assert!(!o.snapshot().unwrap().request_started);
    assert!(o.snapshot().unwrap().credential_used);
    no_send(wire).await;
}
#[tokio::test]
async fn revocation_inside_real_vault_read_prevents_send_and_keeps_material_access_fact() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, true);
    let guard = f.prepare(Duration::from_secs(5));
    *f.vault.revoke.lock().unwrap() = Some((f.store.clone(), "key".into()));
    let o = observation();
    assert!(f.spawn(&guard, o.clone()).await.unwrap().is_err());
    assert_eq!(f.vault.reads.load(Ordering::SeqCst), 1);
    assert!(o.snapshot().unwrap().credential_used);
    assert!(!o.snapshot().unwrap().request_started);
    assert!(guard.budget().check().is_ok());
    no_send(wire).await;
}
#[tokio::test]
async fn malformed_successfully_read_material_is_observed_but_failed_os_read_is_not() {
    for failed_read in [false, true] {
        let wire = server(200).await;
        let f = Fixture::new(&wire.endpoint, true);
        let guard = f.prepare(Duration::from_secs(5));
        *f.vault.bytes.lock().unwrap() = b"malformed-envelope".to_vec();
        f.vault.fail.store(failed_read, Ordering::SeqCst);
        let o = observation();
        assert!(f.spawn(&guard, o.clone()).await.unwrap().is_err());
        assert_eq!(f.vault.reads.load(Ordering::SeqCst), 1);
        assert_eq!(o.snapshot().unwrap().credential_used, !failed_read);
        assert!(!o.snapshot().unwrap().request_started);
        assert!(guard.budget().check().is_ok());
        no_send(wire).await;
    }
}
#[tokio::test]
async fn source_stopped_before_vault_and_after_real_response_is_rejected() {
    for after_wire in [false, true] {
        let mut wire = server(200).await;
        let f = Fixture::new(&wire.endpoint, true);
        let state = f.store.service_configuration().unwrap();
        let selected = f
            .store
            .saved_services(&SavedServiceQuery {
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
            })
            .unwrap();
        f.store
            .select_saved_services(
                &SelectSavedServicesInput {
                    operation_id: "user-select-before-stop".into(),
                    expected_profile_generation: state.profile_generation,
                    scope: ServiceScope::Global {},
                    purpose: CatalogPurpose::Translation,
                    expected_selection_revision: selected.selection.selection_revision,
                    expected_applied_head_revision: state.applied_head_revision,
                    main: f.source.clone(),
                    comparisons: vec![],
                },
                SystemIdentity.now_ms(),
            )
            .unwrap();
        let guard = f.prepare(Duration::from_secs(5));
        let o = observation();
        let pause = Arc::new(Gate::default());
        if !after_wire {
            *f.repo.pause.lock().unwrap() = Some((1, pause.clone()));
        }
        let run = f.spawn(&guard, o.clone());
        if after_wire {
            let _ = (&mut wire.seen).await.unwrap();
        } else {
            pause.entered().await;
        }
        let id = guard
            .prepared()
            .source
            .canonical_control_id
            .clone()
            .unwrap();
        let revision: u32 = rusqlite::Connection::open(&f.path)
            .unwrap()
            .query_row(
                "SELECT control_revision FROM connection_runtime_states WHERE connection_id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        let stopped = f.store.dispatch(
            CommandEnvelope {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: None,
                command: Command::SetConnectionSuspended {
                    connection_id: id,
                    suspended: true,
                    expected_control_revision: revision,
                },
            },
            SystemIdentity.now_ms(),
        );
        // Release a physical test barrier before any assertion can unwind.
        if !after_wire {
            pause.release();
        }
        stopped.unwrap();
        if after_wire {
            wire.release.send(()).unwrap();
            assert!(run.await.unwrap().is_err());
            assert!(o.snapshot().unwrap().request_started);
            wire.task.await.unwrap();
        } else {
            assert!(run.await.unwrap().is_err());
            assert_eq!(f.vault.reads.load(Ordering::SeqCst), 0);
            assert!(!o.snapshot().unwrap().credential_used);
            no_send(wire).await;
        }
        assert!(guard.budget().check().is_ok());
    }
}
#[tokio::test]
async fn authority_failure_before_and_after_material_never_calls_transport() {
    for at in [1, 4] {
        let wire = server(200).await;
        let f = Fixture::new(&wire.endpoint, true);
        let guard = f.prepare(Duration::from_secs(5));
        f.repo.fail_at.store(at, Ordering::SeqCst);
        let o = observation();
        assert_eq!(
            f.spawn(&guard, o.clone()).await.unwrap().unwrap_err().code,
            "controlled_source_changed"
        );
        assert_eq!(f.vault.reads.load(Ordering::SeqCst), usize::from(at == 4));
        assert_eq!(o.snapshot().unwrap().credential_used, at == 4);
        assert!(!o.snapshot().unwrap().request_started);
        assert!(guard.budget().check().is_ok());
        no_send(wire).await;
    }
}
#[tokio::test]
async fn cancellation_after_wire_keeps_started_fact_and_rejects_late_response() {
    let mut wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, false);
    let guard = f.prepare(Duration::from_secs(5));
    let o = observation();
    let run = f.spawn(&guard, o.clone());
    let _ = (&mut wire.seen).await.unwrap();
    guard.budget().cancel();
    assert_eq!(run.await.unwrap().unwrap_err().code, "operation_cancelled");
    assert!(o.snapshot().unwrap().request_started);
    assert!(!o.snapshot().unwrap().credential_used);
    wire.release.send(()).unwrap();
    wire.task.await.unwrap();
    assert_eq!(f.counts().6, 0);
}

#[tokio::test]
async fn dropping_unpolled_gateway_future_cancels_only_its_actual_active_budget() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, false);
    let guard = f.prepare(Duration::from_secs(5));
    let o = observation();
    let future = f.gateway.execute_saved_service_test(
        guard.prepared(),
        f.repo.clone(),
        guard.budget(),
        o.clone(),
    );
    drop(future);
    assert_eq!(
        guard.budget().check().unwrap_err().code,
        "operation_cancelled"
    );
    assert_eq!(f.repo.calls.load(Ordering::SeqCst), 0);
    assert!(!o.snapshot().unwrap().request_started);
    no_send(wire).await;
}
#[tokio::test]
async fn tampered_frozen_endpoint_model_material_or_identity_is_rejected_before_vault_or_wire() {
    use app_core::domain::service_execution::ServiceExecutionTarget;
    for variant in 0..4 {
        let wire = server(200).await;
        let f = Fixture::new(&wire.endpoint, true);
        let mut p = f.raw_prepare();
        match variant {
            0 => {
                let ServiceExecutionTarget::Http { plan } = &mut p.plan.target else {
                    panic!("HTTP")
                };
                plan.request.endpoint = "http://127.0.0.1:1/v1".into();
            }
            1 => {
                let ServiceExecutionTarget::Http { plan } = &mut p.plan.target else {
                    panic!("HTTP")
                };
                plan.request.model = Some("forged-model".into());
            }
            2 => {
                let PreparedTestAuthentication::PoolMember(key) = &mut p.authentication else {
                    panic!("pool")
                };
                key.lifecycle_revision += 1;
            }
            3 => p.input.request.purpose = CatalogPurpose::Grammar,
            _ => unreachable!(),
        }
        let guard = f.register(p, Duration::from_secs(5));
        let o = observation();
        assert!(
            f.spawn(&guard, o.clone()).await.unwrap().is_err(),
            "variant {variant}"
        );
        assert_eq!(f.vault.reads.load(Ordering::SeqCst), 0);
        assert!(!o.snapshot().unwrap().request_started);
        assert!(guard.budget().check().is_ok());
        no_send(wire).await;
    }
}

struct ControlledGoogleToken {
    calls: AtomicUsize,
}
impl app_infra::providers::google_token::GoogleTokenTransport for ControlledGoogleToken {
    fn exchange<'a>(
        &'a self,
        request: app_infra::providers::google_token::GoogleTokenRequest<'a>,
        _: &'a app_core::ports::google_authorization::GoogleNetworkPolicy,
        budget: &'a app_core::ports::google_authorization::GoogleBudget,
    ) -> app_core::ports::runtime::PortFuture<
        'a,
        app_infra::providers::google_token::GoogleTokenReply,
    > {
        Box::pin(async move {
            use app_infra::providers::google_token::*;
            budget.check()?;
            let GoogleTokenRequest::ServiceAccount { assertion } = request else {
                return Err(error("expected_real_sa_assertion"));
            };
            assert_eq!(assertion.expose().split('.').count(), 3);
            self.calls.fetch_add(1, Ordering::SeqCst);
            decode_google_token_response(200,serde_json::to_vec(&json!({"access_token":"controlled-short-token","token_type":"Bearer","expires_in":3600,"scope":app_infra::providers::google_auth::GOOGLE_TRANSLATION_SCOPE})).unwrap(),false)
        })
    }
}
impl Fixture {
    fn save_google_service_account(&mut self) {
        use app_infra::providers::google_auth::import_service_account;
        let material=import_service_account(serde_json::to_vec(&json!({"type":"service_account","client_email":"controlled@example.invalid","private_key":include_str!("fixtures/google-synthetic-only-private.pem"),"private_key_id":"controlled-key-id","project_id":"controlled-project","token_uri":"https://oauth2.googleapis.com/token"})).unwrap().as_slice()).unwrap();
        *self.vault.bytes.lock().unwrap() = encode_vault_credential(&material)
            .unwrap()
            .expose()
            .to_vec();
        self.store
            .begin_credential_material(
                CredentialReference {
                    credential_id: "sa-key".into(),
                    credential_version_id: "opaque-sa-key-version".into(),
                    pool_id: "sa-pool".into(),
                    pool_revision: 1,
                    label: "synthetic SA".into(),
                    masked_hint: "••".into(),
                },
                CredentialStorageMetadata::EnvelopeV1 {
                    kind: CredentialMaterialKind::GoogleServiceAccount,
                },
                SystemIdentity.now_ms(),
            )
            .unwrap();
        self.store
            .activate_credential("sa-key".into(), SystemIdentity.now_ms())
            .unwrap();
        let state = self.store.service_configuration().unwrap();
        let input = SaveServiceDraftInput {
            operation_id: "save-google-source".into(),
            expected_profile_generation: state.profile_generation,
            expected_draft_head_revision: state.draft_head_revision,
            editing: None,
            target: ServiceFormTarget::Http {
                preset: ProviderPresetKey {
                    preset_id: "builtin.google-v3".into(),
                    preset_revision: 1,
                },
            },
            purpose: CatalogPurpose::Translation,
            values: BTreeMap::from([
                (
                    "name".into(),
                    CatalogScalar::Text("Controlled Google saved source".into()),
                ),
                (
                    "google_auth_method".into(),
                    CatalogScalar::Text("service_account".into()),
                ),
                (
                    "project_id".into(),
                    CatalogScalar::Text("controlled-project".into()),
                ),
                ("location".into(), CatalogScalar::Text("global".into())),
                (
                    "model".into(),
                    CatalogScalar::Text(
                        "projects/controlled-project/locations/global/models/general/nmt".into(),
                    ),
                ),
            ]),
            authentication: ServiceAuthenticationSelection::ExistingPool {
                pool_id: "sa-pool".into(),
                pool_revision: std::num::NonZeroU32::new(1).unwrap(),
            },
        };
        let compiled = compile_service_draft(
            &state.draft,
            &input,
            &builtin_registry().unwrap(),
            &SystemIdentity,
            &Default::default(),
        )
        .unwrap();
        self.source = self
            .store
            .save_service_configuration(&compiled, SystemIdentity.now_ms())
            .unwrap()
            .saved_resource;
    }
}
#[tokio::test(flavor = "current_thread")]
async fn real_google_owner_acquires_deferred_test_lease_then_cancel_before_wire_retains_actual_facts()
 {
    use app_core::ports::google_authorization::*;
    use app_infra::providers::google_authorization::{
        GoogleAuthorizationService, SystemGoogleClock,
    };
    let wire = server(200).await;
    let mut f = Fixture::new(&wire.endpoint, false);
    f.save_google_service_account();
    let tokens = Arc::new(ControlledGoogleToken {
        calls: AtomicUsize::new(0),
    });
    let owner = Arc::new(GoogleAuthorizationService::new(
        f.store.google_authorization_repository(f.live.clone()),
        f.vault.clone(),
        tokens.clone(),
        Arc::new(SystemGoogleClock),
        f.live.clone(),
    ));
    f.gateway = Arc::new(
        HttpModelGateway::new(f.store.clone(), f.vault.clone()).with_google_authorization(owner).unwrap(),
    );
    let before = f.counts();
    let guard = f.prepare(Duration::from_secs(5));
    *f.repo.cancel_at.lock().unwrap() = Some((3, guard.budget()));
    let o = observation();
    let result = f.spawn(&guard, o.clone()).await.unwrap();
    assert!(result.is_err());
    o.wait_for_physical_drain().await.unwrap();
    assert_eq!(
        guard.budget().check().unwrap_err().code,
        "operation_cancelled"
    );
    assert_eq!(tokens.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.vault.reads.load(Ordering::SeqCst), 1);
    let facts = o.snapshot().unwrap();
    assert!(!facts.request_started);
    assert!(facts.credential_used);
    let lease = facts
        .google_lease
        .as_ref()
        .expect("real acquired owner lease");
    assert_eq!(lease.request, guard.google_request().unwrap());
    assert!(matches!(lease.origin, GoogleLeaseOrigin::ServiceAccount(_)));
    assert_eq!(f.counts(), before);
    let record = f
        .repo
        .finish_service_test(
            FinishServiceTestInput {
                prepared: guard.prepared(),
                outcome: ServiceTestTerminalOutcome::Cancelled,
                budget: guard.budget(),
                request_sent: facts.request_started,
                credential_used: facts.credential_used,
                google_lease: facts.google_lease,
            },
            SystemIdentity.now_ms(),
        )
        .unwrap();
    assert_eq!(record.status, ServiceTestStatus::Cancelled);
    assert_eq!(f.counts().6, before.6 + 1);
    let db = rusqlite::Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM execution_authorization_leases",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    no_send(wire).await;
}
#[tokio::test]
async fn saved_google_without_installed_owner_rejects_before_material_or_token() {
    let wire = server(200).await;
    let mut f = Fixture::new(&wire.endpoint, false);
    f.save_google_service_account();
    let guard = f.prepare(Duration::from_secs(5));
    let o = observation();
    assert_eq!(
        f.spawn(&guard, o.clone()).await.unwrap().unwrap_err().code,
        "authorization_provider_unavailable"
    );
    assert_eq!(f.vault.reads.load(Ordering::SeqCst), 0);
    assert!(!o.snapshot().unwrap().credential_used);
    assert!(!o.snapshot().unwrap().request_started);
    assert!(guard.budget().check().is_ok());
    no_send(wire).await;
}
#[tokio::test(flavor = "current_thread")]
async fn cancel_after_final_pre_vault_sql_return_cannot_read_material_later() {
    let wire = server(200).await;
    let f = Fixture::new(&wire.endpoint, true);
    let guard = f.prepare(Duration::from_secs(5));
    let gate = Arc::new(Gate::default());
    *f.repo.pause_after.lock().unwrap() = Some((3, gate.clone()));
    let o = observation();
    let run = f.spawn(&guard, o.clone());
    gate.entered().await;
    guard.budget().cancel();
    assert_eq!(run.await.unwrap().unwrap_err().code, "operation_cancelled");
    gate.release();
    o.wait_for_physical_drain().await.unwrap();
    assert_eq!(
        f.vault.reads.load(Ordering::SeqCst),
        0,
        "cancel won after SQL checked but before Vault began"
    );
    let facts = o.snapshot().unwrap();
    assert!(!facts.credential_used);
    assert!(!facts.request_started);
    no_send(wire).await;
}
