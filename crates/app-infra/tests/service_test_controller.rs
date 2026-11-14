//! Real Controller + SQLite + HTTP; these controlled endpoints are not provider-account acceptance.
use app_core::{
    application::{
        google_test_operations::LiveServiceTestOperations,
        service_testing::ServiceTestingController,
    },
    contracts::*,
    domain::model_config::{service_draft::compile_service_draft, service_graph::*},
    ports::{
        IdentitySource, ServiceConfigurationRepository, runtime::CredentialVault,
        service_testing::*,
    },
};
use app_infra::{
    platform::SystemIdentity,
    providers::{builtin_registry, gateway::HttpModelGateway},
    storage::SqliteStore,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
struct NoVault;
impl CredentialVault for NoVault {
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        panic!("no-auth test must not read Vault")
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        panic!("read-only test")
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        panic!("read-only test")
    }
}
struct Fixture {
    store: Arc<SqliteStore>,
    repo: Arc<dyn ServiceTestingRepository>,
    live: Arc<LiveServiceTestOperations>,
    source: SavedServiceRef,
    path: PathBuf,
}
fn fixture(endpoint: &str, timeout: f64) -> Fixture {
    let setup_started = std::time::Instant::now();
    let path = std::env::temp_dir().join(format!(
        "service-controller-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let store = Arc::new(
        SqliteStore::open(
            &path,
            "controller-profile",
            "controller-server",
            SystemIdentity.now_ms(),
        )
        .unwrap(),
    );
    let directory = Arc::new(builtin_registry().unwrap());
    store
        .configure_service_directory(directory.clone())
        .unwrap();
    let state = store.service_configuration().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: "save-controller-source".into(),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: "builtin.lmstudio".into(),
                preset_revision: 1,
            },
        },
        purpose: CatalogPurpose::Translation,
        values: BTreeMap::from([
            (
                "name".into(),
                CatalogScalar::Text("Controlled saved source".into()),
            ),
            ("model".into(), CatalogScalar::Text("test-model".into())),
            (
                "connection_mode".into(),
                CatalogScalar::Text("custom".into()),
            ),
            ("endpoint".into(), CatalogScalar::Text(endpoint.into())),
            ("auth_mode".into(), CatalogScalar::Text("none".into())),
            ("timeout".into(), CatalogScalar::Number(timeout)),
        ]),
        authentication: ServiceAuthenticationSelection::None {},
    };
    let command = compile_service_draft(
        &state.draft,
        &input,
        directory.as_ref(),
        &SystemIdentity,
        &Default::default(),
    )
    .unwrap();
    let source = store
        .save_service_configuration(&command, SystemIdentity.now_ms())
        .unwrap()
        .saved_resource;
    let live = Arc::new(LiveServiceTestOperations::default());
    let repo = store
        .service_testing_repository(live.clone(), live.clone())
        .unwrap();
    eprintln!(
        "controller fixture setup elapsed {:?}",
        setup_started.elapsed()
    );
    Fixture {
        store,
        repo,
        live,
        source,
        path,
    }
}
fn request(f: &Fixture) -> SavedServiceTestRequest {
    SavedServiceTestRequest {
        expected_profile_generation: f.store.service_configuration().unwrap().profile_generation,
        source: f.source.clone(),
        purpose: CatalogPurpose::Translation,
        input_scope: None,
        output_mode: None,
    }
}
fn controller(
    f: &Fixture,
    repo: Arc<dyn ServiceTestingRepository>,
    callback: Arc<dyn Fn() + Send + Sync>,
) -> Arc<ServiceTestingController> {
    ServiceTestingController::new(
        repo,
        f.store.clone(),
        Arc::new(HttpModelGateway::new(f.store.clone(), Arc::new(NoVault))),
        Arc::new(SystemIdentity),
        f.live.clone(),
        callback,
    )
}
fn counts(f: &Fixture) -> (i64, i64, i64) {
    let c = rusqlite::Connection::open(&f.path).unwrap();
    c.query_row("SELECT (SELECT count(*) FROM reading_units),(SELECT count(*) FROM request_tasks),(SELECT count(*) FROM request_attempts)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap()
}
fn status(f: &Fixture) -> Option<String> {
    rusqlite::Connection::open(&f.path)
        .unwrap()
        .query_row(
            "SELECT state FROM test_records ORDER BY completed_at_ms DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()
}
struct Server {
    endpoint: String,
    armed: mpsc::SyncSender<()>,
    seen: mpsc::Receiver<()>,
    release: mpsc::SyncSender<()>,
    count: Arc<AtomicUsize>,
    thread: std::thread::JoinHandle<()>,
}
fn server() -> Server {
    server_with_error(None)
}
fn server_with_error(error: Option<(u16, String)>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (armed, start) = mpsc::sync_channel(1);
    let (seen, seen_rx) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let thread = std::thread::spawn(move || {
        // Fixture schema migration is setup, not part of the HTTP-start SLO.
        start
            .recv_timeout(Duration::from_secs(30))
            .expect("controller setup did not arm its listener");
        let started = std::time::Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        started.elapsed() < Duration::from_secs(5),
                        "real wire never arrived"
                    );
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => panic!("{e}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = vec![];
        let body = loop {
            let mut buf = [0; 4096];
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                let length = String::from_utf8_lossy(&bytes[..i])
                    .lines()
                    .find_map(|s| {
                        s.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                if bytes.len() >= i + 4 + length {
                    break serde_json::from_slice::<serde_json::Value>(
                        &bytes[i + 4..i + 4 + length],
                    )
                    .unwrap();
                }
            }
        };
        calls.fetch_add(1, Ordering::SeqCst);
        seen.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(5)).unwrap();
        if let Some((status, body)) = error {
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            return;
        }
        let (kind, body) = if body["stream"] == true {
            (
                "text/event-stream",
                "data: {\"choices\":[{\"delta\":{\"content\":\"受控译文\"}}]}\n\ndata: [DONE]\n\n"
                    .to_owned(),
            )
        } else {
            (
                "application/json",
                serde_json::json!({"choices":[{"message":{"content":"受控译文"}}]}).to_string(),
            )
        };
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    });
    Server {
        endpoint,
        armed,
        seen: seen_rx,
        release,
        count,
        thread,
    }
}
async fn seen(server: &mut Server) {
    let (_, placeholder) = mpsc::channel();
    let rx = std::mem::replace(&mut server.seen, placeholder);
    tokio::task::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(5)))
        .await
        .unwrap()
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_wire_terminal_is_replayed_before_another_request_and_no_reading_task_is_created() {
    let mut server = server();
    let f = fixture(&server.endpoint, 5.0);
    server.armed.send(()).unwrap();
    let before = counts(&f);
    let controller = controller(&f, f.repo.clone(), Arc::new(|| {}));
    let request = request(&f);
    let copy = controller.clone();
    let input = request.clone();
    let run = tokio::spawn(async move {
        copy.run("wire-once".into(), input, None)
            .await
            .map(|r| {
                eprintln!(
                    "controlled terminal before join: {:?} / {}",
                    r.status, r.message
                );
                r
            })
            .inspect_err(|e| {
                eprintln!(
                    "controlled test failed before wire: {} / {}",
                    e.code, e.stage
                )
            })
    });
    seen(&mut server).await;
    server.release.send(()).unwrap();
    let result = run.await.unwrap().unwrap();
    assert_eq!(result.status, ServiceTestStatus::Passed);
    let replay = controller
        .run("wire-once".into(), request.clone(), None)
        .await
        .unwrap();
    assert_eq!(replay.test_id, result.test_id);
    assert_eq!(server.count.load(Ordering::SeqCst), 1);
    assert_eq!(counts(&f), before);
    let mut conflict = request;
    conflict.output_mode = Some(ServiceTestOutputMode::Structured);
    assert_eq!(
        controller
            .run("wire-once".into(), conflict, None)
            .await
            .unwrap_err()
            .code,
        "operation_conflict"
    );
    server.thread.join().unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_receiver_drop_does_not_abandon_the_real_terminal() {
    let mut server = server();
    let f = fixture(&server.endpoint, 5.0);
    server.armed.send(()).unwrap();
    let (done, mut completed) = tokio::sync::mpsc::unbounded_channel();
    let controller = controller(
        &f,
        f.repo.clone(),
        Arc::new(move || {
            let _ = done.send(());
        }),
    );
    let copy = controller.clone();
    let input = request(&f);
    let run = tokio::spawn(async move { copy.run("dropped-window".into(), input, None).await });
    seen(&mut server).await;
    run.abort();
    server.release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), completed.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status(&f).as_deref(), Some("passed"));
    controller.stop_and_drain().await.unwrap();
    server.thread.join().unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_cancel_and_shutdown_persist_real_stops_without_making_future_tests_eligible() {
    for closing in [false, true] {
        let mut server = server();
        let f = fixture(&server.endpoint, 5.0);
        server.armed.send(()).unwrap();
        let controller = controller(&f, f.repo.clone(), Arc::new(|| {}));
        let input = request(&f);
        let generation = input.expected_profile_generation;
        let copy = controller.clone();
        let run = tokio::spawn(async move { copy.run("stop-test".into(), input, None).await });
        seen(&mut server).await;
        if closing {
            controller.stop_and_drain().await.unwrap();
        } else {
            controller.cancel("stop-test", generation).unwrap();
        }
        assert_eq!(
            run.await.unwrap().unwrap().status,
            ServiceTestStatus::Cancelled
        );
        assert_eq!(status(&f).as_deref(), Some("cancelled"));
        server.release.send(()).unwrap();
        server.thread.join().unwrap();
        if closing {
            assert_eq!(
                controller
                    .run("after-close".into(), request(&f), None)
                    .await
                    .unwrap_err()
                    .code,
                "application_closing"
            );
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_saved_timeout_is_not_relabelled_as_an_explicit_cancel() {
    let mut server = server();
    let f = fixture(&server.endpoint, 1.0);
    server.armed.send(()).unwrap();
    let controller = controller(&f, f.repo.clone(), Arc::new(|| {}));
    let input = request(&f);
    let copy = controller.clone();
    let run = tokio::spawn(async move { copy.run("deadline-test".into(), input, None).await });
    seen(&mut server).await;
    assert_eq!(
        run.await.unwrap().unwrap().status,
        ServiceTestStatus::TimedOut
    );
    assert_eq!(status(&f).as_deref(), Some("timed_out"));
    server.release.send(()).unwrap();
    server.thread.join().unwrap();
}
struct PausedPrepare {
    inner: Arc<dyn ServiceTestingRepository>,
    entered: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}
impl ServiceTestingRepository for PausedPrepare {
    fn replay_service_test(
        &self,
        i: &PrepareServiceTestInput,
    ) -> AppResult<Option<ServiceTestResult>> {
        self.inner.replay_service_test(i)
    }
    fn prepare_service_test(
        &self,
        i: PrepareServiceTestInput,
        t: i64,
    ) -> AppResult<PreparedServiceTest> {
        self.entered.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        self.inner.prepare_service_test(i, t)
    }
    fn validate_prepared_test(&self, p: &Arc<PreparedServiceTest>) -> AppResult<()> {
        self.inner.validate_prepared_test(p)
    }
    fn finish_service_test(
        &self,
        i: FinishServiceTestInput,
        t: i64,
    ) -> AppResult<ServiceTestResult> {
        self.inner.finish_service_test(i, t)
    }
    fn service_test_qualification(
        &self,
        q: ServiceTestQualificationQuery,
    ) -> AppResult<ServiceTestQualification> {
        self.inner.service_test_qualification(q)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_during_blocking_prepare_exists_before_material_or_transport() {
    let f = fixture("http://127.0.0.1:1/v1", 5.0);
    let (entered, rx) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let repo = Arc::new(PausedPrepare {
        inner: f.repo.clone(),
        entered,
        release: Mutex::new(gate),
    });
    let controller = controller(&f, repo, Arc::new(|| {}));
    let input = request(&f);
    let generation = input.expected_profile_generation;
    let copy = controller.clone();
    let run = tokio::spawn(async move { copy.run("preparation-stop".into(), input, None).await });
    tokio::task::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(3)))
        .await
        .unwrap()
        .unwrap();
    controller.cancel("preparation-stop", generation).unwrap();
    release.send(()).unwrap();
    assert_eq!(
        run.await.unwrap().unwrap().status,
        ServiceTestStatus::Cancelled
    );
    assert_eq!(status(&f).as_deref(), Some("cancelled"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_late_callback_cannot_erase_another_run_or_hide_an_unfinished_drain() {
    let mut server = server();
    let f = fixture(&server.endpoint, 5.0);
    server.armed.send(()).unwrap();
    let (entered, wait) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let gate = Mutex::new(gate);
    let callbacks = AtomicUsize::new(0);
    let controller = controller(
        &f,
        f.repo.clone(),
        Arc::new(move || {
            if callbacks.fetch_add(1, Ordering::SeqCst) == 0 {
                entered.send(()).unwrap();
                gate.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
        }),
    );
    let copy = controller.clone();
    let input = request(&f);
    let original =
        tokio::spawn(async move { copy.run("same-op-callback".into(), input, None).await });
    seen(&mut server).await;
    server.release.send(()).unwrap();
    tokio::task::spawn_blocking(move || wait.recv_timeout(Duration::from_secs(3)))
        .await
        .unwrap()
        .unwrap();
    let replay = controller
        .run("same-op-callback".into(), request(&f), None)
        .await
        .unwrap();
    assert_eq!(replay.status, ServiceTestStatus::Passed);
    assert_eq!(server.count.load(Ordering::SeqCst), 1);
    let copy = controller.clone();
    let stopped = tokio::spawn(async move { copy.stop_and_drain().await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        !stopped.is_finished(),
        "the first callback still owns a real unfinished run"
    );
    release.send(()).unwrap();
    assert_eq!(original.await.unwrap().unwrap().test_id, replay.test_id);
    stopped.await.unwrap().unwrap();
    server.thread.join().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_http_failure_keeps_a_safe_actionable_diagnosis_and_replays_it() {
    let secret_marker = "not-for-public-status-raw-server-marker";
    let mut server = server_with_error(Some((401, format!("{{\"error\":\"{secret_marker}\"}}"))));
    let f = fixture(&server.endpoint, 5.0);
    server.armed.send(()).unwrap();
    let controller = controller(&f, f.repo.clone(), Arc::new(|| {}));
    let input = request(&f);
    let repeat = input.clone();
    let copy = controller.clone();
    let run = tokio::spawn(async move { copy.run("auth-failed".into(), input, None).await });
    seen(&mut server).await;
    server.release.send(()).unwrap();
    let result = run.await.unwrap().unwrap();
    assert_eq!(result.status, ServiceTestStatus::Failed);
    let detail = result.failure.as_ref().unwrap();
    assert_eq!(detail.code, "authentication_failed");
    assert!(detail.recovery.contains("授权"));
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(secret_marker)
    );
    let replay = controller
        .run("auth-failed".into(), repeat, None)
        .await
        .unwrap();
    assert_eq!(result, replay);
    assert_eq!(server.count.load(Ordering::SeqCst), 1);
    server.thread.join().unwrap();
}
