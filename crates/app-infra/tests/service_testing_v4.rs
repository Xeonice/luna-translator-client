//! Genuine saved-source preparation, loopback wire and atomic terminal SQL facts.
//! These tests do not qualify external accounts, native/worker resources or a package.
use app_core::{
    contracts::*,
    domain::{
        content_hash,
        model_config::{
            credentials::ProviderAuthLease, service_draft::compile_service_draft, service_graph::*,
        },
    },
    ports::{
        IdentitySource, ProductRepository, ServiceConfigurationRepository, google_authorization::*,
        operation_budget::OperationBudget, service_testing::*,
    },
};
use app_infra::{
    providers::{
        adapters::{dispatch, translation::TranslationWireOptions},
        builtin_registry,
    },
    storage::SqliteStore,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Ids;
impl IdentitySource for Ids {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        panic!("pure compiler cannot read time")
    }
}
#[derive(Default)]
struct Live {
    entries: Mutex<BTreeMap<String, (Arc<PreparedServiceTest>, OperationBudget)>>,
    calls: AtomicUsize,
    pause: Mutex<Option<(usize, Arc<Barrier>, Arc<Barrier>)>>,
}
impl Live {
    fn register(
        &self,
        p: PreparedServiceTest,
        duration: Duration,
    ) -> (Arc<PreparedServiceTest>, OperationBudget) {
        let p = Arc::new(p);
        let b = OperationBudget::new(duration).unwrap();
        self.entries
            .lock()
            .unwrap()
            .insert(p.test_id.clone(), (p.clone(), b.clone()));
        (p, b)
    }
}
impl ServiceTestOperationAuthority for Live {
    fn active_prepared(&self) -> AppResult<Vec<Arc<PreparedServiceTest>>> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .values()
            .map(|(p, _)| p.clone())
            .collect())
    }

    fn operation_budget(&self, p: &Arc<PreparedServiceTest>) -> AppResult<OperationBudget> {
        let budget = {
            let e = self.entries.lock().unwrap();
            let (a, b) = e
                .get(&p.test_id)
                .ok_or_else(|| AppError::new("test_inactive", "test", "inactive"))?;
            if !Arc::ptr_eq(a, p) {
                return Err(AppError::new("test_identity", "test", "wrong object"));
            }
            b.clone()
        };
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let pause = self.pause.lock().unwrap().clone();
        if let Some((at, entered, release)) = pause {
            if n == at {
                entered.wait();
                release.wait();
            }
        }
        Ok(budget)
    }
}
struct NoGoogle;
impl GoogleTestOperationAuthority for NoGoogle {
    fn check(&self, _: &GoogleLeaseRequest, _: &GoogleTestOperation) -> AppResult<()> {
        Err(AppError::new(
            "not_google_test",
            "test",
            "no Google operation",
        ))
    }
}
struct Fixture {
    store: SqliteStore,
    path: PathBuf,
    live: Arc<Live>,
    repo: Arc<dyn ServiceTestingRepository>,
}
fn fixture() -> Fixture {
    let dir = std::env::temp_dir().join(format!("saved-service-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.sqlite3");
    let store = SqliteStore::open(&path, "test-profile", "test-server", 10).unwrap();
    store
        .configure_service_directory(Arc::new(builtin_registry().unwrap()))
        .unwrap();
    let live = Arc::new(Live::default());
    let repo = store
        .service_testing_repository(live.clone(), Arc::new(NoGoogle))
        .unwrap();
    Fixture {
        store,
        path,
        live,
        repo,
    }
}
fn save(
    f: &Fixture,
    op: &str,
    purpose: CatalogPurpose,
    endpoint: Option<&str>,
    editing: Option<SavedServiceRef>,
    extra: Vec<(&str, CatalogScalar)>,
) -> SavedServiceRef {
    save_auth(
        f,
        op,
        purpose,
        endpoint,
        editing,
        extra,
        "builtin.lmstudio",
        ServiceAuthenticationSelection::None {},
    )
}
fn save_auth(
    f: &Fixture,
    op: &str,
    purpose: CatalogPurpose,
    endpoint: Option<&str>,
    editing: Option<SavedServiceRef>,
    extra: Vec<(&str, CatalogScalar)>,
    preset: &str,
    authentication: ServiceAuthenticationSelection,
) -> SavedServiceRef {
    let state = f.store.service_configuration().unwrap();
    let mut values: BTreeMap<String, CatalogScalar> = [
        ("name".into(), CatalogScalar::Text(format!("{op} name"))),
        (
            "model".into(),
            CatalogScalar::Text("controlled-model".into()),
        ),
        (
            "template".into(),
            CatalogScalar::Text("Translate {{source}}".into()),
        ),
        ("cache".into(), CatalogScalar::Boolean(true)),
    ]
    .into_iter()
    .collect();
    if let Some(endpoint) = endpoint {
        values.insert(
            "connection_mode".into(),
            CatalogScalar::Text("custom".into()),
        );
        values.insert("endpoint".into(), CatalogScalar::Text(endpoint.into()));
        values.insert(
            "auth_mode".into(),
            CatalogScalar::Text(
                if matches!(authentication, ServiceAuthenticationSelection::None {}) {
                    "none"
                } else {
                    "api_key"
                }
                .into(),
            ),
        );
    }
    for (k, v) in extra {
        values.insert(k.into(), v);
    }
    let input = SaveServiceDraftInput {
        operation_id: op.into(),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision,
        editing,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: preset.into(),
                preset_revision: 1,
            },
        },
        purpose,
        values,
        authentication,
    };
    let compiled = compile_service_draft(
        &state.draft,
        &input,
        &builtin_registry().unwrap(),
        &Ids,
        &Default::default(),
    )
    .unwrap();
    f.store
        .save_service_configuration(&compiled, 20)
        .unwrap()
        .saved_resource
}
fn prepare(
    f: &Fixture,
    source: SavedServiceRef,
    purpose: CatalogPurpose,
    op: &str,
) -> (Arc<PreparedServiceTest>, OperationBudget) {
    let p = f
        .repo
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: op.into(),
                request: SavedServiceTestRequest {
                    expected_profile_generation: f
                        .store
                        .service_configuration()
                        .unwrap()
                        .profile_generation,
                    source,
                    purpose,
                    input_scope: None,
                    output_mode: None,
                },
                selected_token: None,
            },
            30,
        )
        .unwrap();
    f.live.register(p, Duration::from_secs(5))
}
fn finish(
    p: Arc<PreparedServiceTest>,
    budget: OperationBudget,
    outcome: ServiceTestTerminalOutcome,
    sent: bool,
) -> FinishServiceTestInput {
    FinishServiceTestInput {
        prepared: p,
        outcome,
        budget,
        request_sent: sent,
        credential_used: false,
        google_lease: None,
    }
}
fn counts(path: &PathBuf) -> (u32, u32, u32, u32, u32, u32) {
    let c = Connection::open(path).unwrap();
    c.query_row("SELECT (SELECT count(*) FROM reading_units),(SELECT count(*) FROM request_tasks),(SELECT count(*) FROM request_attempts),(SELECT count(*) FROM purpose_bindings),(SELECT count(*) FROM purpose_runtime_states),(SELECT count(*) FROM cache_entries)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap()
}
fn wire_server(
    calls: usize,
) -> (
    String,
    Arc<AtomicUsize>,
    std::thread::JoinHandle<Vec<Value>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let h = std::thread::spawn(move || {
        let mut bodies = vec![];
        for _ in 0..calls {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut data = vec![];
            let mut buf = [0u8; 4096];
            let mut end = None;
            let mut length = 0;
            loop {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                data.extend_from_slice(&buf[..n]);
                if end.is_none() {
                    if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&data[..i]);
                        length = head
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        end = Some(i + 4);
                    }
                }
                if end.is_some_and(|e| data.len() >= e + length) {
                    break;
                }
            }
            let body: Value =
                serde_json::from_slice(&data[end.unwrap()..end.unwrap() + length]).unwrap();
            bodies.push(body.clone());
            c.fetch_add(1, Ordering::SeqCst);
            let (content_type, response) = if body["stream"] == true {
                (
                    "text/event-stream",
                    format!(
                        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                        json!({"choices":[{"delta":{"content":"雨停了。"}}]}),
                        json!({"choices":[{"delta":{},"finish_reason":"stop"}]})
                    ),
                )
            } else {
                (
                    "application/json",
                    json!({"choices":[{"message":{"content":"雨停了。"},"finish_reason":"stop"}]})
                        .to_string(),
                )
            };
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
        }
        bodies
    });
    (endpoint, count, h)
}
fn wire_connection(p: &PreparedServiceTest) -> ConnectionConfig {
    let app_core::domain::service_execution::ServiceExecutionTarget::Http { plan } = &p.plan.target
    else {
        panic!("HTTP")
    };
    let c = &plan.connection;
    ConnectionConfig {
        adapter_binding: c.adapter_binding.clone(),
        preset_source: c.preset_source.clone(),
        connection_id: c.connection_id.clone(),
        display_name: c.display_name.clone(),
        protocol: c.protocol.clone(),
        deployment: c.deployment.clone(),
        auth_mode: c.auth_mode.clone(),
        endpoint: c.endpoint.clone(),
        credential_pool_id: c.credential_pool_id.clone(),
        credential_pool_revision: c.credential_pool_revision.map(|v| v.get()),
        proxy_mode: c.proxy_mode.clone(),
        network_metadata: c.network_metadata.clone(),
        availability_intent: c.availability_intent,
    }
}
type ConnectionConfig = app_core::contracts::Connection;
#[tokio::test]
async fn saved_unapplied_sample_uses_actual_adapter_each_time_and_writes_only_real_terminal() {
    let f = fixture();
    let (endpoint, calls, server) = wire_server(2);
    let source = save(
        &f,
        "service",
        CatalogPurpose::Translation,
        Some(&endpoint),
        None,
        vec![],
    );
    let before = f.store.service_configuration().unwrap();
    let original = counts(&f.path);
    for op in ["first", "second"] {
        let (p, b) = prepare(&f, source.clone(), CatalogPurpose::Translation, op);
        assert!(p.plan.policy.cache.enabled);
        f.repo.validate_prepared_test(&p).unwrap();
        let app_core::domain::service_execution::ServiceExecutionTarget::Http { plan } =
            &p.plan.target
        else {
            panic!()
        };
        let output = dispatch::execute(
            &plan.request,
            &wire_connection(&p),
            CatalogPurpose::Translation,
            &ProviderAuthLease::None,
            &TranslationWireOptions::default(),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(output.text, "雨停了。");
        f.repo.validate_prepared_test(&p).unwrap();
        let invocation = p.input.clone();
        let input = finish(
            p.clone(),
            b.clone(),
            ServiceTestTerminalOutcome::Succeeded {
                response: output.text,
            },
            true,
        );
        let result = f.repo.finish_service_test(input.clone(), 50).unwrap();
        assert_eq!(result.status, ServiceTestStatus::Passed);
        assert!(b.is_committed());
        assert_eq!(
            f.repo.finish_service_test(input, 51).unwrap().test_id,
            result.test_id
        );
        assert_eq!(
            f.repo
                .replay_service_test(&invocation)
                .unwrap()
                .unwrap()
                .test_id,
            result.test_id
        );
        assert_eq!(
            f.repo
                .prepare_service_test(invocation.clone(), 52)
                .err()
                .unwrap()
                .code,
            "service_test_already_completed"
        );
        let mut changed = invocation;
        changed.request.output_mode = Some(ServiceTestOutputMode::Structured);
        assert_eq!(
            f.repo.replay_service_test(&changed).unwrap_err().code,
            "operation_conflict"
        );
    }
    let bodies = server.join().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(bodies.len(), 2);
    assert_eq!(original, counts(&f.path));
    let after = f.store.service_configuration().unwrap();
    assert_eq!(before.draft_head_revision, after.draft_head_revision);
    assert_eq!(before.applied_head_revision, after.applied_head_revision);
    assert!(after.applied.is_none());
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM test_records WHERE stage='model' AND target_kind='http' AND binding_id IS NULL AND purpose IS NULL",[],|r|r.get::<_,u32>(0)).unwrap(),2);
    let q = f
        .repo
        .service_test_qualification(ServiceTestQualificationQuery {
            expected_profile_generation: after.profile_generation,
            source,
            purpose: CatalogPurpose::Translation,
            output_mode: ServiceTestOutputMode::Text,
        })
        .unwrap();
    assert_eq!(q.comparison, ServiceTestComparison::Compared);
    assert_eq!(q.status, Some(ServiceTestStatus::Passed));
    assert!(q.complete_credential_coverage);
}
#[test]
fn original_arc_and_original_budget_are_required_and_debug_is_redacted() {
    let f = fixture();
    let source = save(
        &f,
        "service",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let (p, b) = prepare(&f, source, CatalogPurpose::Translation, "same-object");
    f.repo.validate_prepared_test(&p).unwrap();
    assert!(
        f.repo
            .validate_prepared_test(&Arc::new((*p).clone()))
            .is_err()
    );
    let wrong = OperationBudget::new(Duration::from_secs(5)).unwrap();
    assert!(
        f.repo
            .finish_service_test(
                finish(
                    p.clone(),
                    wrong,
                    ServiceTestTerminalOutcome::Succeeded {
                        response: "sensitive response".into()
                    },
                    true
                ),
                40
            )
            .is_err()
    );
    let debug = format!(
        "{:?} {:?}",
        p,
        ServiceTestTerminalOutcome::Succeeded {
            response: "sensitive response".into()
        }
    );
    assert!(!debug.contains("sensitive response"));
    assert!(!debug.contains("雨がやんで"));
    assert!(!b.is_committed());
    assert!(
        f.store
            .service_testing_repository(Arc::new(Live::default()), Arc::new(NoGoogle))
            .is_err()
    );
    assert_eq!(
        Connection::open(&f.path)
            .unwrap()
            .query_row("SELECT count(*) FROM test_records", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn cancellation_and_expiry_have_real_terminal_facts_without_reauthorizing_success() {
    let f = fixture();
    let source = save(
        &f,
        "service",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let (p, b) = prepare(&f, source.clone(), CatalogPurpose::Translation, "cancelled");
    b.cancel();
    assert!(f.repo.validate_prepared_test(&p).is_err());
    assert!(
        f.repo
            .finish_service_test(
                finish(
                    p.clone(),
                    b.clone(),
                    ServiceTestTerminalOutcome::Succeeded {
                        response: "late".into()
                    },
                    true
                ),
                40
            )
            .is_err()
    );
    assert_eq!(
        f.repo
            .finish_service_test(
                finish(p, b, ServiceTestTerminalOutcome::Cancelled, false),
                41
            )
            .unwrap()
            .status,
        ServiceTestStatus::Cancelled
    );
    let p = f
        .repo
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: "expired".into(),
                request: SavedServiceTestRequest {
                    expected_profile_generation: f
                        .store
                        .service_configuration()
                        .unwrap()
                        .profile_generation,
                    source,
                    purpose: CatalogPurpose::Translation,
                    input_scope: None,
                    output_mode: None,
                },
                selected_token: None,
            },
            45,
        )
        .unwrap();
    let (p, b) = f.live.register(p, Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(
        f.repo
            .finish_service_test(
                finish(p, b, ServiceTestTerminalOutcome::TimedOut, false),
                50
            )
            .unwrap()
            .status,
        ServiceTestStatus::TimedOut
    );
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM test_records WHERE state IN('cancelled','timed_out')",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
}
#[test]
fn commit_fence_wins_cancel_and_sql_failure_rolls_back_before_marking_committed() {
    let f = fixture();
    let source = save(
        &f,
        "service",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let (p, b) = prepare(
        &f,
        source.clone(),
        CatalogPurpose::Translation,
        "commit-first",
    );
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    *f.live.pause.lock().unwrap() = Some((3, entered.clone(), release.clone()));
    let repo = f.repo.clone();
    let input = finish(
        p,
        b.clone(),
        ServiceTestTerminalOutcome::Succeeded {
            response: "actual terminal fixture response".into(),
        },
        true,
    );
    let writer = std::thread::spawn(move || repo.finish_service_test(input, 40));
    entered.wait();
    let b2 = b.clone();
    let cancel = std::thread::spawn(move || b2.cancel());
    release.wait();
    assert_eq!(
        writer.join().unwrap().unwrap().status,
        ServiceTestStatus::Passed
    );
    cancel.join().unwrap();
    assert!(b.is_committed());
    *f.live.pause.lock().unwrap() = None;
    let (p, b) = prepare(&f, source, CatalogPurpose::Translation, "rollback");
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("CREATE TRIGGER reject_test_receipt BEFORE INSERT ON operation_receipts WHEN NEW.command_kind='saved_service_test' BEGIN SELECT RAISE(ABORT,'controlled_terminal_receipt_failure'); END;").unwrap();
    let input = finish(
        p.clone(),
        b.clone(),
        ServiceTestTerminalOutcome::Succeeded {
            response: "rollback response".into(),
        },
        true,
    );
    assert!(f.repo.finish_service_test(input.clone(), 50).is_err());
    assert!(!b.is_committed());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM test_records WHERE test_id=?1",
            [&p.test_id],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM operation_receipts WHERE operation_id='rollback'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    db.execute_batch("DROP TRIGGER reject_test_receipt")
        .unwrap();
    assert_eq!(
        f.repo.finish_service_test(input, 51).unwrap().status,
        ServiceTestStatus::Passed
    );
}

fn command(f: &Fixture, op: &str, command: Command, now: i64) -> CommandReceipt {
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: op.into(),
                expected_revision: None,
                command,
            },
            now,
        )
        .unwrap()
}
fn verification(f: &Fixture, source: &SavedServiceRef) -> ServiceVerification {
    let q = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: f
                .store
                .service_configuration()
                .unwrap()
                .profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    q.rows
        .into_iter()
        .find(|row| &row.source == source)
        .unwrap()
        .verification
}
fn qualification(f: &Fixture, source: SavedServiceRef) -> ServiceTestQualification {
    f.repo
        .service_test_qualification(ServiceTestQualificationQuery {
            expected_profile_generation: f
                .store
                .service_configuration()
                .unwrap()
                .profile_generation,
            source,
            purpose: CatalogPurpose::Translation,
            output_mode: ServiceTestOutputMode::Text,
        })
        .unwrap()
}
#[test]
fn qualification_compares_only_used_semantics_does_not_copy_success_or_erase_old_unknown() {
    let f = fixture();
    let source = save(
        &f,
        "service",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let (p, b) = prepare(
        &f,
        source.clone(),
        CatalogPurpose::Translation,
        "original-test",
    );
    f.repo
        .finish_service_test(
            finish(
                p,
                b,
                ServiceTestTerminalOutcome::Succeeded {
                    response: "controlled response".into(),
                },
                true,
            ),
            40,
        )
        .unwrap();
    let renamed = save(
        &f,
        "renamed",
        CatalogPurpose::Translation,
        None,
        Some(source.clone()),
        vec![],
    );
    assert_eq!(
        qualification(&f, renamed.clone()).comparison,
        ServiceTestComparison::Compared
    );
    assert!(matches!(
        verification(&f, &renamed),
        ServiceVerification::Passed { .. }
    ));

    let changed = save(
        &f,
        "changed",
        CatalogPurpose::Translation,
        None,
        Some(renamed),
        vec![(
            "template",
            CatalogScalar::Text("Different {{source}}".into()),
        )],
    );
    assert!(matches!(
        verification(&f, &changed),
        ServiceVerification::Stale { .. }
    ));
    assert_eq!(
        qualification(&f, changed).comparison,
        ServiceTestComparison::Stale
    );
    let copied = save(&f, "copy", CatalogPurpose::Translation, None, None, vec![]);
    assert_eq!(qualification(&f, copied).status, None);
    let (p, b) = prepare(
        &f,
        source.clone(),
        CatalogPurpose::Translation,
        "cancel-after-success",
    );
    b.cancel();
    f.repo
        .finish_service_test(
            finish(p, b, ServiceTestTerminalOutcome::Cancelled, false),
            50,
        )
        .unwrap();
    assert_eq!(
        qualification(&f, source).status,
        Some(ServiceTestStatus::Passed)
    );
}
#[test]
fn saved_selected_input_and_explicit_word_are_verified_and_new_manual_text_is_not_substituted() {
    use app_core::domain::capture::ResourceSnapshotInput;
    let f = fixture();
    let source = save(&f, "grammar", CatalogPurpose::Grammar, None, None, vec![]);
    let uid = command(
        &f,
        "input",
        Command::ManualInput {
            text: "空は明るい。".into(),
            source_language: "ja".into(),
        },
        22,
    )
    .result_id
    .unwrap();
    let unit = f
        .store
        .snapshot()
        .unwrap()
        .history
        .units
        .into_iter()
        .find(|u| u.unit_id == uid)
        .unwrap();
    let request = SavedServiceTestRequest {
        expected_profile_generation: f.store.service_configuration().unwrap().profile_generation,
        source: source.clone(),
        purpose: CatalogPurpose::Grammar,
        input_scope: Some(ServiceTestInputScope::SelectedUnit {
            unit_id: uid.clone(),
            text_revision: std::num::NonZeroU32::new(1).unwrap(),
            source_hash: unit.source_hash.clone(),
        }),
        output_mode: None,
    };
    let p = f
        .repo
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: "selected".into(),
                request: request.clone(),
                selected_token: None,
            },
            30,
        )
        .unwrap();
    let (p, b) = f.live.register(p, Duration::from_secs(5));
    assert_eq!(
        f.store
            .maintain_storage(
                app_core::domain::retention::RetentionPolicy {
                    max_rows_per_batch: 100,
                    history_unit_ids: vec![uid.clone()],
                    ..Default::default()
                },
                Default::default(),
                "protect-live-selected-unit".into(),
                30
            )
            .unwrap_err()
            .code,
        "reading_object_in_use"
    );
    command(
        &f,
        "later",
        Command::ManualInput {
            text: "別の新しい文章。".into(),
            source_language: "ja".into(),
        },
        31,
    );
    f.repo.validate_prepared_test(&p).unwrap();
    let app_core::domain::service_execution::ServiceExecutionTarget::Http { plan } = &p.plan.target
    else {
        panic!()
    };
    assert_eq!(plan.request.source_text, "空は明るい。");
    f.repo
        .finish_service_test(
            finish(
                p,
                b,
                ServiceTestTerminalOutcome::Succeeded {
                    response: "controlled grammar explanation".into(),
                },
                true,
            ),
            40,
        )
        .unwrap();
    let mut wrong = request;
    wrong.input_scope = Some(ServiceTestInputScope::SelectedUnit {
        unit_id: uid,
        text_revision: std::num::NonZeroU32::new(1).unwrap(),
        source_hash: content_hash("different"),
    });
    assert!(
        f.repo
            .prepare_service_test(
                PrepareServiceTestInput {
                    operation_id: "wrong-source".into(),
                    request: wrong,
                    selected_token: None
                },
                41
            )
            .is_err()
    );
    let _ = std::mem::size_of::<ResourceSnapshotInput>();
}
#[tokio::test]
async fn one_actual_key_does_not_qualify_the_pool_and_prepare_does_not_advance_rotation() {
    use app_core::domain::model_config::credentials::{
        CredentialMaterialKind, CredentialStorageMetadata, CredentialVersionRef, SecretText,
    };
    let f = fixture();
    for id in ["a", "b"] {
        f.store
            .begin_credential_material(
                CredentialReference {
                    credential_id: id.into(),
                    credential_version_id: format!("opaque-version-{id}"),
                    pool_id: format!("seed-{id}"),
                    pool_revision: 1,
                    label: "synthetic".into(),
                    masked_hint: "••".into(),
                },
                CredentialStorageMetadata::EnvelopeV1 {
                    kind: CredentialMaterialKind::ApiKey,
                },
                12,
            )
            .unwrap();
        f.store.activate_credential(id.into(), 13).unwrap();
    }
    let pool = command(
        &f,
        "shared-pool",
        Command::SaveCredentialPool {
            pool: CredentialPoolInput {
                pool_id: Some("shared-pool".into()),
                expected_revision: 0,
                policy: "round_robin".into(),
                metadata: BTreeMap::new(),
                members: ["a", "b"]
                    .into_iter()
                    .map(|id| CredentialPoolMemberInput {
                        member_id: None,
                        credential_id: id.into(),
                        enabled: true,
                        weight: 1,
                    })
                    .collect(),
            },
        },
        15,
    );
    assert!(pool.result_id.is_some());
    let (endpoint, calls, server) = wire_server(2);
    let source = save_auth(
        &f,
        "pooled",
        CatalogPurpose::Translation,
        Some(&endpoint),
        None,
        vec![],
        "builtin.openai-chat",
        ServiceAuthenticationSelection::ExistingPool {
            pool_id: "shared-pool".into(),
            pool_revision: std::num::NonZeroU32::new(1).unwrap(),
        },
    );
    for (op, expected) in [("key-a", "a"), ("key-b", "b")] {
        let (p, b) = prepare(&f, source.clone(), CatalogPurpose::Translation, op);
        let PreparedTestAuthentication::PoolMember(c) = &p.authentication else {
            panic!("actual key")
        };
        assert_eq!(c.selection.credential_id, expected);
        assert_eq!(
            f.store
                .maintain_storage(
                    app_core::domain::retention::RetentionPolicy {
                        max_rows_per_batch: 100,
                        credential_pool_versions: vec![("shared-pool".into(), 1)],
                        ..Default::default()
                    },
                    Default::default(),
                    format!("protect-pool-{expected}"),
                    35
                )
                .unwrap_err()
                .code,
            "credential_pool_in_use"
        );
        let check = f
            .store
            .select_credential_for_kind("shared-pool".into(), 1, CredentialMaterialKind::ApiKey)
            .unwrap();
        assert_eq!(
            check.credential_id, expected,
            "prepare must not advance production cursor"
        );
        f.repo.validate_prepared_test(&p).unwrap();
        let app_core::domain::service_execution::ServiceExecutionTarget::Http { plan } =
            &p.plan.target
        else {
            panic!()
        };
        let version = CredentialVersionRef {
            credential_id: c.selection.credential_id.clone(),
            credential_version_id: c.selection.vault_ref.clone(),
        };
        let key = SecretText::from_bytes(format!("test-key-{expected}").into_bytes()).unwrap();
        let output = dispatch::execute(
            &plan.request,
            &wire_connection(&p),
            CatalogPurpose::Translation,
            &ProviderAuthLease::ApiKey {
                source: &version,
                key: &key,
            },
            &TranslationWireOptions::default(),
            |_| {},
        )
        .await
        .unwrap();
        let mut terminal = finish(
            p,
            b,
            ServiceTestTerminalOutcome::Succeeded {
                response: output.text,
            },
            true,
        );
        terminal.credential_used = true;
        f.repo.finish_service_test(terminal, 40).unwrap();
        let q = qualification(&f, source.clone());
        assert_eq!(q.required_credentials, 2);
        assert_eq!(q.covered_credentials, if expected == "a" { 1 } else { 2 });
        assert_eq!(q.complete_credential_coverage, expected == "b");
        if expected == "a" {
            assert!(matches!(
                verification(&f, &source),
                ServiceVerification::PartiallyTested {
                    passed_credential_count: 1,
                    total_credential_count: 2,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                verification(&f, &source),
                ServiceVerification::Passed { .. }
            ));
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.join().unwrap();
    f.store.revoke_credential("b".into(), 50).unwrap();
    let q = qualification(&f, source);
    assert_eq!(q.required_credentials, 1);
    assert_eq!(q.covered_credentials, 1);
}
#[test]
fn selected_word_requires_the_actual_explicit_snapshot_item_and_structured_validation_is_real() {
    use app_core::domain::capture::{ResourceSnapshotInput, TokenSnapshotInput};
    let f = fixture();
    let source = save(
        &f,
        "word-service",
        CatalogPurpose::ContextWord,
        None,
        None,
        vec![],
    );
    let uid = command(
        &f,
        "word-input",
        Command::ManualInput {
            text: "空は明るい。".into(),
            source_language: "ja".into(),
        },
        22,
    )
    .result_id
    .unwrap();
    let tokens = vec![
        LexicalToken {
            surface: "空".into(),
            lemma: "空".into(),
            reading: "そら".into(),
            part_of_speech: "名詞".into(),
            byte_start: 0,
            byte_end: 3,
            utf16_start: 0,
            utf16_end: 1,
            known: true,
        },
        LexicalToken {
            surface: "明るい".into(),
            lemma: "明るい".into(),
            reading: "あかるい".into(),
            part_of_speech: "形容詞".into(),
            byte_start: 6,
            byte_end: 15,
            utf16_start: 2,
            utf16_end: 5,
            known: true,
        },
    ];
    f.store
        .set_tokens(
            TokenSnapshotInput {
                unit_id: uid.clone(),
                text_revision: 1,
                analysis_revision: "controlled-lexical-v1".into(),
                tokens: tokens.clone(),
                resource: ResourceSnapshotInput {
                    snapshot_id: "controlled-language".into(),
                    kind: "language".into(),
                    version: "test-v1".into(),
                    manifest_sha256: content_hash("controlled-language"),
                    contract_version: "test-v1".into(),
                    metadata_json: "{}".into(),
                },
            },
            25,
        )
        .unwrap();
    let scope = ServiceTestInputScope::SelectedUnit {
        unit_id: uid,
        text_revision: std::num::NonZeroU32::new(1).unwrap(),
        source_hash: content_hash("空は明るい。"),
    };
    let req = SavedServiceTestRequest {
        expected_profile_generation: f.store.service_configuration().unwrap().profile_generation,
        source: source.clone(),
        purpose: CatalogPurpose::ContextWord,
        input_scope: Some(scope),
        output_mode: None,
    };
    assert!(
        f.repo
            .prepare_service_test(
                PrepareServiceTestInput {
                    operation_id: "no-selection".into(),
                    request: req.clone(),
                    selected_token: None
                },
                30
            )
            .is_err()
    );
    let mut wrong = tokens[1].clone();
    wrong.lemma = "not-the-stored-item".into();
    assert!(
        f.repo
            .prepare_service_test(
                PrepareServiceTestInput {
                    operation_id: "wrong-selection".into(),
                    request: req.clone(),
                    selected_token: Some(wrong)
                },
                30
            )
            .is_err()
    );
    let p = f
        .repo
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: "word-selection".into(),
                request: req,
                selected_token: Some(tokens[1].clone()),
            },
            30,
        )
        .unwrap();
    let PreparedTestReading::Selected {
        selected_token,
        lexical_snapshot_id,
        ..
    } = &p.reading
    else {
        panic!()
    };
    assert_eq!(selected_token.as_ref(), Some(&tokens[1]));
    assert!(lexical_snapshot_id.is_some());
    let (p, b) = f.live.register(p, Duration::from_secs(5));
    f.repo.validate_prepared_test(&p).unwrap();
    f.repo
        .finish_service_test(
            finish(
                p,
                b,
                ServiceTestTerminalOutcome::Succeeded {
                    response: "controlled word explanation".into(),
                },
                true,
            ),
            40,
        )
        .unwrap();
    let grammar = save(
        &f,
        "structured",
        CatalogPurpose::Grammar,
        None,
        None,
        vec![],
    );
    let p = f
        .repo
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: "structured-output".into(),
                request: SavedServiceTestRequest {
                    expected_profile_generation: f
                        .store
                        .service_configuration()
                        .unwrap()
                        .profile_generation,
                    source: grammar,
                    purpose: CatalogPurpose::Grammar,
                    input_scope: None,
                    output_mode: Some(ServiceTestOutputMode::Structured),
                },
                selected_token: None,
            },
            45,
        )
        .unwrap();
    let (p, b) = f.live.register(p, Duration::from_secs(5));
    let result = f
        .repo
        .finish_service_test(
            finish(
                p,
                b,
                ServiceTestTerminalOutcome::Succeeded {
                    response: "{\"notTheStructuredSchema\":true}".into(),
                },
                true,
            ),
            50,
        )
        .unwrap();
    assert_eq!(result.status, ServiceTestStatus::Failed);
    assert_eq!(result.stage, "output_validation");
}
#[test]
fn real_source_gc_and_explicit_suspension_do_not_become_test_authority() {
    use app_core::domain::retention::RetentionPolicy;
    let f = fixture();
    let initial = save(
        &f,
        "source",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let state = f
        .store
        .service_configuration_revision(source_parts_test(&initial).0)
        .unwrap();
    let mut graph = state.as_service_graph().unwrap().clone();
    graph.revision_id = "unheaded-test-source".into();
    f.store
        .persist_service_graph_revision(graph, None, 23)
        .unwrap();
    let source = SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "unheaded-test-source".into(),
            model_id: source_parts_test(&initial).1.into(),
        },
    };
    let (p, b) = prepare(
        &f,
        source.clone(),
        CatalogPurpose::Translation,
        "removed-source",
    );
    let protected_policy = RetentionPolicy {
        max_rows_per_batch: 100,
        config_revision_ids: vec!["unheaded-test-source".into()],
        ..Default::default()
    };
    assert!(
        f.store
            .maintain_storage(
                protected_policy,
                Default::default(),
                "protect-live-test-source".into(),
                34
            )
            .is_err()
    );
    f.live.entries.lock().unwrap().remove(&p.test_id);
    f.store
        .maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                config_revision_ids: vec!["unheaded-test-source".into()],
                ..Default::default()
            },
            Default::default(),
            "controlled-source-gc".into(),
            35,
        )
        .unwrap();
    assert!(f.repo.validate_prepared_test(&p).is_err());
    assert!(
        f.repo
            .finish_service_test(
                finish(
                    p,
                    b,
                    ServiceTestTerminalOutcome::Succeeded {
                        response: "late after actual source gc".into()
                    },
                    true
                ),
                40
            )
            .is_err()
    );
    // The existing STOP command controls an applied connection; explicitly
    // establish that real precondition, without having the test apply anything.
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
                main: initial.clone(),
                comparisons: vec![],
            },
            42,
        )
        .unwrap();
    let (p, b) = prepare(&f, initial, CatalogPurpose::Translation, "explicit-stop");
    let control = p.source.canonical_control_id.clone().unwrap();
    let db = Connection::open(&f.path).unwrap();
    let rev: u32 = db
        .query_row(
            "SELECT control_revision FROM connection_runtime_states WHERE connection_id=?1",
            [&control],
            |r| r.get(0),
        )
        .unwrap();
    command(
        &f,
        "stop",
        Command::SetConnectionSuspended {
            connection_id: control,
            suspended: true,
            expected_control_revision: rev,
        },
        45,
    );
    assert!(f.repo.validate_prepared_test(&p).is_err());
    assert!(
        f.repo
            .finish_service_test(
                finish(
                    p,
                    b,
                    ServiceTestTerminalOutcome::Succeeded {
                        response: "late after explicit stop".into()
                    },
                    true
                ),
                50
            )
            .is_err()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM test_records", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
fn source_parts_test(s: &SavedServiceRef) -> (&str, &str) {
    match s {
        SavedServiceRef::Http { source } => (&source.revision_id, &source.model_id),
        SavedServiceRef::Offline { source } => (&source.revision_id, &source.config_id),
    }
}
#[test]
fn deadline_crossed_inside_the_real_transaction_rolls_back_all_terminal_rows() {
    let f = fixture();
    let source = save(
        &f,
        "source",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let p = f
        .repo
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: "expire-before-commit".into(),
                request: SavedServiceTestRequest {
                    expected_profile_generation: f
                        .store
                        .service_configuration()
                        .unwrap()
                        .profile_generation,
                    source,
                    purpose: CatalogPurpose::Translation,
                    input_scope: None,
                    output_mode: None,
                },
                selected_token: None,
            },
            30,
        )
        .unwrap();
    let (p, b) = f.live.register(p, Duration::from_millis(200));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    *f.live.pause.lock().unwrap() = Some((3, entered.clone(), release.clone()));
    let repo = f.repo.clone();
    let id = p.test_id.clone();
    let input = finish(
        p,
        b.clone(),
        ServiceTestTerminalOutcome::Succeeded {
            response: "response before deadline boundary".into(),
        },
        true,
    );
    let writer = std::thread::spawn(move || repo.finish_service_test(input, 40));
    entered.wait();
    std::thread::sleep(Duration::from_millis(220));
    release.wait();
    assert!(writer.join().unwrap().is_err());
    assert!(!b.is_committed());
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM test_records WHERE test_id=?1",
            [id],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM operation_receipts WHERE operation_id='expire-before-commit'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn unknown_legacy_ledger_is_preserved_without_claiming_typed_qualification() {
    use app_core::domain::model_config::testing::ModelTestRecordInput;
    let f = fixture();
    let source = save(
        &f,
        "old-record-source",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let (revision, model) = source_parts_test(&source);
    let d = f.store.service_configuration_revision(revision).unwrap();
    let g = d.as_service_graph().unwrap();
    let c = g
        .models
        .iter()
        .find(|m| m.model_id == model)
        .unwrap()
        .connection_id
        .clone();
    f.store
        .record_model_test(ModelTestRecordInput {
            test_id: "old-unknown-record".into(),
            config_revision_id: revision.into(),
            connection_id: Some(c),
            model_id: Some(model.into()),
            binding_id: None,
            purpose: None,
            stage: "model".into(),
            request_semantics_hash: content_hash("frozen-old-unknown-contract"),
            input_scope: "sample".into(),
            unit_id: None,
            text_revision: None,
            input_summary: json!({"sampleId":"old-controlled-fixture"}),
            response_summary: json!({}),
            output_validation: json!({}),
            state: "failed".into(),
            started_at_ms: 30,
            completed_at_ms: 40,
            error_code: Some("old_observation_failed".into()),
            error_stage: Some("provider".into()),
            credentials: vec![],
        })
        .unwrap();
    let q = qualification(&f, source);
    assert_eq!(q.status, Some(ServiceTestStatus::Failed));
    assert_eq!(q.comparison, ServiceTestComparison::NotCovered);
    assert!(!q.complete_credential_coverage);
}
#[test]
fn a_saved_fallback_keeps_its_actual_source_alive_during_gc() {
    use app_core::domain::retention::RetentionPolicy;
    let f = fixture();
    let initial = save(
        &f,
        "initial",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let d = f
        .store
        .service_configuration_revision(source_parts_test(&initial).0)
        .unwrap();
    let mut g = d.as_service_graph().unwrap().clone();
    g.revision_id = "fallback-owned-source".into();
    f.store.persist_service_graph_revision(g, None, 23).unwrap();
    let target = SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "fallback-owned-source".into(),
            model_id: source_parts_test(&initial).1.into(),
        },
    };
    save(
        &f,
        "consumer",
        CatalogPurpose::Translation,
        None,
        None,
        vec![(
            "fallback",
            CatalogScalar::Text(serde_json::to_string(&vec![target]).unwrap()),
        )],
    );
    let db = Connection::open(&f.path).unwrap();
    let before: u32 = db
        .query_row(
            "SELECT count(*) FROM models WHERE revision_id='fallback-owned-source'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        f.store
            .maintain_storage(
                RetentionPolicy {
                    max_rows_per_batch: 100,
                    config_revision_ids: vec!["fallback-owned-source".into()],
                    ..Default::default()
                },
                Default::default(),
                "reject-external-source-gc".into(),
                35
            )
            .unwrap_err()
            .code,
        "config_in_use"
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM config_gc_marks WHERE revision_id='fallback-owned-source'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM models WHERE revision_id='fallback-owned-source'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        before
    );
}
#[test]
fn over_one_hundred_real_cancellations_cannot_wash_away_the_same_scope_failure() {
    let f = fixture();
    let source = save(
        &f,
        "scope-failure",
        CatalogPurpose::Translation,
        None,
        None,
        vec![],
    );
    let (p, b) = prepare(
        &f,
        source.clone(),
        CatalogPurpose::Translation,
        "known-failure",
    );
    f.repo
        .finish_service_test(
            finish(
                p,
                b,
                ServiceTestTerminalOutcome::Failed {
                    error: AppError::new(
                        "controlled_service_failure",
                        "provider",
                        "controlled failure, no vendor claim",
                    ),
                },
                true,
            ),
            40,
        )
        .unwrap();
    assert_eq!(
        qualification(&f, source.clone()).status,
        Some(ServiceTestStatus::Failed)
    );
    for i in 0..101 {
        let (p, b) = prepare(
            &f,
            source.clone(),
            CatalogPurpose::Translation,
            &format!("cancelled-observation-{i}"),
        );
        let id = p.test_id.clone();
        b.cancel();
        f.repo
            .finish_service_test(
                finish(p, b, ServiceTestTerminalOutcome::Cancelled, false),
                50 + i,
            )
            .unwrap();
        f.live.entries.lock().unwrap().remove(&id);
    }
    let q = qualification(&f, source.clone());
    assert_eq!(q.status, Some(ServiceTestStatus::Failed));
    assert!(matches!(
        verification(&f, &source),
        ServiceVerification::Failed { .. }
    ));
}
