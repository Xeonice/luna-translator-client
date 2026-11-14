//! Actual bounded-writer saturation and runtime recovery, not mocked QueueFull.
use super::*;
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    domain::capture::*,
    ports::{IdentitySource, runtime::*},
};
use std::sync::{
    Mutex,
    atomic::{AtomicI64, Ordering},
};
struct Clock(AtomicI64);
impl IdentitySource for Clock {
    fn now_ms(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", self.now_ms())
    }
}
struct Hook {
    sender: mpsc::Sender<HookUpdate>,
    receiver: Mutex<Option<mpsc::Receiver<HookUpdate>>>,
}
impl HookBackend for Hook {
    fn targets(&self) -> AppResult<TargetCandidates> {
        Ok(TargetCandidates {
            candidates: vec![],
            diagnostics: vec![],
        })
    }
    fn associate(&self, _: AssociateRequest) -> AppResult<HookSessionInput> {
        Err(AppError::new("not_used", "test", "not used"))
    }
    fn disconnect(&self) -> AppResult<()> {
        Ok(())
    }
    fn stop_and_drain(&self, acknowledged: mpsc::SyncSender<AppResult<()>>) -> AppResult<()> {
        self.disconnect()?;
        self.sender
            .send(HookUpdate::DrainBarrier { acknowledged })
            .map_err(err)
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        self.receiver
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| AppError::new("used", "test", "used"))
    }
    fn resource_info(&self) -> ResourceInfo {
        info()
    }
}
struct Services;
impl ModelGateway for Services {
    fn execute<'a>(
        &'a self,
        _: &'a TaskDispatch,
        _: &'a RequestPlan,
        _: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(AppError::new("unexpected_model", "test", "unexpected")) })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a app_core::contracts::Connection,
        _: &'a RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(AppError::new("not_used", "test", "not used")) })
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a app_core::contracts::Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Ok(vec![]) })
    }
}
impl LanguageServices for Services {
    fn analyze<'a>(&'a self, _: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn lookup(&self, _: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        Ok(vec![])
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![]
    }
}
impl CredentialVault for Services {
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        Ok(())
    }
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        Err(AppError::new("not_used", "test", "not used"))
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        Ok(())
    }
}
fn info() -> ResourceInfo {
    ResourceInfo {
        kind: "core".into(),
        version: "controlled".into(),
        manifest_sha256: content_hash("controlled"),
        available: true,
        detail: "controlled".into(),
    }
}
fn session() -> HookSessionInput {
    HookSessionInput {
        session_id: "saturated-session".into(),
        game_id: None,
        environment_key: "controlled".into(),
        target_exe_path: "C:\\controlled.exe".into(),
        windows_pid: 100,
        process_created_u64: "1".into(),
        architecture: "x86".into(),
        broker_instance_id: "controlled-broker".into(),
        resource: ResourceSnapshotInput {
            snapshot_id: "controlled-core".into(),
            kind: "core".into(),
            version: "fixture".into(),
            manifest_sha256: content_hash("core"),
            contract_version: "1".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: content_hash("controlled-target"),
    }
}
fn text(n: u32) -> HookTextInput {
    HookTextInput {
        session_id: "saturated-session".into(),
        event_id: format!("saturated-{n}"),
        sequence_u64: n.to_string(),
        source: HookSourceInput {
            source_id: "saturated-source".into(),
            host_instance_id: "controlled-broker".into(),
            hook_address_u64: "1".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "fixture".into(),
            hook_name: "fixture".into(),
            encoding: "utf8".into(),
            stream_identity_sha256: content_hash("source"),
            identity_extensions_json: "{}".into(),
        },
        text: "実際に受信した原文。".into(),
        source_language: "ja".into(),
    }
}
fn query() -> SessionRecordQuery {
    SessionRecordQuery {
        session_id: "saturated-session".into(),
        dataset: SessionDataset::Extraction,
        order: SessionRecordOrder::OldestFirst,
        filter: Default::default(),
        cursor: None,
        page_size: 50,
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_writer_saturation_blocks_clean_export_and_quit_then_persists_unknown_across_reopen() {
    let path = std::env::temp_dir().join(format!(
        "luna-runtime-saturation-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let store =
        Arc::new(SqliteStore::open(&path, "saturation-profile", "saturation-server", 1).unwrap());
    store
        .begin_capture_operation("operation".into(), content_hash("request"), 1)
        .unwrap();
    store.open_hook_session(session(), 2).unwrap();
    store
        .finish_capture_operation("operation".into(), Some("saturated-session".into()), 3)
        .unwrap();
    store.ingest_hook_text(text(1), 4).unwrap();
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "select-source".into(),
                expected_revision: None,
                command: Command::SelectSource {
                    source_id: "saturated-source".into(),
                },
            },
            5,
        )
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let hook = Arc::new(Hook {
        sender: sender.clone(),
        receiver: Mutex::new(Some(receiver)),
    });
    let app = ApplicationRuntime::new(
        store.clone(),
        Arc::new(Clock(AtomicI64::new(10))),
        hook,
        Arc::new(Services),
        Arc::new(Services),
        Arc::new(Services),
    )
    .unwrap();
    app.start().unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let busy = store.clone();
    let blocker = thread::spawn(move || {
        busy.call(move |_db| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..128 {
        assert!(
            store
                .inner
                .sender
                .as_ref()
                .unwrap()
                .try_send(Box::new(|_| {}))
                .is_ok()
        );
    }
    sender.send(HookUpdate::Text(Box::new(text(2)))).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !store
            .inner
            .capture_pending
            .lock()
            .unwrap()
            .contains_key("saturated-session")
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        app.session_records(query()).unwrap_err().code,
        "storage_queue_full"
    );
    assert_eq!(
        app.prepare_session_export(SessionExportRequest {
            query: query(),
            privacy_confirmed: true
        })
        .unwrap_err()
        .code,
        "storage_queue_full"
    );
    assert_eq!(app.shutdown().await.unwrap_err().code, "storage_queue_full");
    release_tx.send(()).unwrap();
    blocker.join().unwrap().unwrap();
    let p = app.session_records(query()).unwrap();
    assert_eq!(p.completeness.status, "unknown");
    assert!(p.completeness.unknown_range);
    assert_eq!(p.records.len(), 1);
    assert_eq!(store.history(None, None, 100).unwrap().units.len(), 0);
    app.shutdown().await.unwrap();
    drop(app);
    drop(store);
    let reopened =
        SqliteStore::open(&path, "saturation-profile", "after-clean-stop", 1000).unwrap();
    let after = reopened.session_records(query(), 1001).unwrap();
    assert_eq!(after.completeness.status, "unknown");
    let c = Connection::open(&path).unwrap();
    let count:i64=c.query_row("SELECT count(*) FROM hook_events WHERE code='storage_unconfirmed' AND integrity='unknown'",[],|r|r.get(0)).unwrap();
    assert!(count >= 1);
}

#[test]
fn real_sqlite_write_failure_keeps_old_frozen_scope_unknown_after_clean_reopen() {
    let path = std::env::temp_dir().join(format!(
        "luna-readonly-capture-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let store = SqliteStore::open(&path, "readonly-profile", "readonly-server", 1).unwrap();
    store
        .begin_capture_operation("operation".into(), content_hash("request"), 1)
        .unwrap();
    store.open_hook_session(session(), 2).unwrap();
    store
        .finish_capture_operation("operation".into(), Some("saturated-session".into()), 3)
        .unwrap();
    store.ingest_hook_text(text(1), 4).unwrap();
    let before = store.session_records(query(), 5).unwrap();
    assert_eq!(before.completeness.status, "complete_received_range");
    let mut frozen = query();
    frozen.cursor = Some(before.snapshot.refresh_cursor.clone());
    store
        .call(|db| db.conn.execute_batch("PRAGMA query_only=ON").map_err(err))
        .unwrap();
    assert_eq!(
        store
            .ingest_hook_text_with_policy(text(2), true, 6)
            .unwrap_err()
            .code,
        "storage_failure"
    );
    assert_eq!(
        store.session_records(frozen.clone(), 7).unwrap_err().code,
        "storage_failure"
    );
    assert_eq!(
        store
            .prepare_session_export(
                SessionExportRequest {
                    query: frozen.clone(),
                    privacy_confirmed: true
                },
                8
            )
            .unwrap_err()
            .code,
        "storage_failure"
    );
    assert_eq!(
        store
            .dispatch(
                CommandEnvelope {
                    operation_id: "failed-disconnect".into(),
                    expected_revision: None,
                    command: Command::Disconnect
                },
                9
            )
            .unwrap_err()
            .code,
        "storage_failure"
    );
    assert!(matches!(
        store.snapshot().unwrap().session.unwrap().state.as_str(),
        "active" | "opening"
    ));
    store
        .call(|db| db.conn.execute_batch("PRAGMA query_only=OFF").map_err(err))
        .unwrap();
    let after = store.session_records(frozen, 10).unwrap();
    assert_eq!(after.snapshot.snapshot_id, before.snapshot.snapshot_id);
    assert_eq!(
        after.snapshot.high_watermark,
        before.snapshot.high_watermark
    );
    assert_eq!(
        after.snapshot.refresh_cursor,
        before.snapshot.refresh_cursor
    );
    assert_eq!(after.completeness.status, "unknown");
    assert!(after.completeness.unknown_range);
    assert_eq!(after.records.len(), 1);
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "clean-disconnect".into(),
                expected_revision: None,
                command: Command::Disconnect,
            },
            11,
        )
        .unwrap();
    drop(store);
    let reopened = SqliteStore::open(&path, "readonly-profile", "reopened-server", 100).unwrap();
    assert_eq!(
        reopened.snapshot().unwrap().session.unwrap().state,
        "stopped"
    );
    let persisted = reopened.session_records(query(), 101).unwrap();
    assert_eq!(persisted.completeness.status, "unknown");
    assert_eq!(persisted.records.len(), 1);
    let c = Connection::open(&path).unwrap();
    let count:i64=c.query_row("SELECT count(*) FROM hook_events WHERE code='storage_unconfirmed' AND integrity='unknown'",[],|r|r.get(0)).unwrap();
    assert_eq!(count, 1);
}
