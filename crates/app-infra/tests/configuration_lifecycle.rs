//! Real production Repository/runtime safety controls. Gateway responses are controlled, not live-service evidence.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{TaskCompletion, TaskDispatch, capture::*, content_hash},
    ports::{ProductRepository, runtime::*},
};
use app_infra::{platform::SystemIdentity, storage::SqliteStore};
use serde_json::json;
use std::{
    sync::{Arc, Barrier, Mutex, mpsc},
    time::Duration,
};
use tokio::sync::{mpsc as async_mpsc, oneshot};

fn config() -> Configuration {
    let mut c: Configuration = serde_json::from_value(json!({
        "revisionId":"", "connections":[], "models":[], "bindings":[], "templates":[], "termSets":[]
    }))
    .unwrap();
    for suffix in ["a", "b"] {
        c.connections.push(serde_json::from_value(json!({"connectionId":format!("connection-{suffix}"),"displayName":format!("Controlled {suffix}"),"protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1/v1","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true})).unwrap());
        c.models.push(serde_json::from_value(json!({"modelId":format!("model-{suffix}"),"connectionId":format!("connection-{suffix}"),"actualModelId":format!("applied-model-{suffix}"),"displayName":format!("Controlled {suffix}"),"selectionSource":"manual","capabilities":{},"parameters":{}})).unwrap());
    }
    for purpose in ["translation", "grammar"] {
        let mut b: PurposeBinding = serde_json::from_value(json!({"bindingId":purpose,"scopeKey":"global","purpose":purpose,"bindingMode":"override","enabled":true,"outputMode":"text","sourceLanguage":"ja","targetLanguage":"zh-CN","context":{},"schedule":{"concurrency":2,"queueCapacity":8},"parameters":{},"engines":[]})).unwrap();
        for suffix in if purpose == "translation" {
            vec!["a", "b"]
        } else {
            vec!["a"]
        } {
            b.engines.push(serde_json::from_value(json!({"engineId":format!("{purpose}-{suffix}"),"modelId":format!("model-{suffix}"),"enabled":true,"parameters":{}})).unwrap());
        }
        c.bindings.push(b);
    }
    c
}
fn store() -> (Arc<SqliteStore>, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "luna-configuration-lifecycle-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    (
        Arc::new(SqliteStore::open(&path, "lifecycle-profile", "lifecycle-server", 1).unwrap()),
        path,
    )
}
fn command(s: &SqliteStore, command: Command) -> CommandReceipt {
    s.dispatch(
        CommandEnvelope {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: None,
            command,
        },
        10,
    )
    .unwrap()
}
fn save(s: &SqliteStore, configuration: Configuration) -> String {
    command(
        s,
        Command::SaveConfiguration {
            configuration,
            expected_head_revision: s.snapshot().unwrap().configuration.draft_head_revision,
        },
    )
    .result_id
    .unwrap()
}
fn apply(s: &SqliteStore, revision_id: String) {
    command(
        s,
        Command::ApplyConfiguration {
            revision_id,
            expected_head_revision: s.snapshot().unwrap().configuration.applied_head_revision,
        },
    );
}
fn configured(s: &SqliteStore) {
    apply(s, save(s, config()));
}
fn manual(s: &SqliteStore, source: &str) -> String {
    command(
        s,
        Command::ManualInput {
            text: source.into(),
            source_language: "ja".into(),
        },
    )
    .result_id
    .unwrap()
}
fn prepared(s: &SqliteStore, uid: &str, purpose: &str) -> Vec<TaskDispatch> {
    s.prepare_tasks(uid.into(), 1, purpose.into(), 20).unwrap()
}
fn completion(t: &TaskDispatch) -> TaskCompletion {
    TaskCompletion {
        task_id: t.task_id.clone(),
        attempt_id: t.attempt_id.clone(),
        text: format!("result from {}", t.engine.engine_id),
        output_mode: if t.binding.purpose == "translation" {
            "text"
        } else {
            "plain_text"
        }
        .into(),
        structured: if t.binding.purpose == "translation" {
            json!({})
        } else {
            json!({"references":[]})
        },
        target_language: "zh-CN".into(),
        provider_label: t.connection.display_name.clone(),
    }
}
fn connection_control(s: &SqliteStore, connection: &str, suspended: bool) -> Command {
    let view = s
        .snapshot()
        .unwrap()
        .configuration
        .runtime_connections
        .into_iter()
        .find(|c| c.connection_id == connection)
        .unwrap();
    Command::SetConnectionSuspended {
        connection_id: connection.into(),
        suspended,
        expected_control_revision: view.control_revision,
    }
}
fn purpose_control(s: &SqliteStore, scope: &str, purpose: &str, enabled: bool) -> Command {
    let view = s
        .snapshot()
        .unwrap()
        .configuration
        .runtime_purposes
        .into_iter()
        .find(|p| p.scope_key == scope && p.purpose == purpose)
        .unwrap();
    Command::SetPurposeRuntimeEnabled {
        scope_key: scope.into(),
        purpose: purpose.into(),
        enabled,
        expected_authorization_epoch: view.authorization_epoch,
    }
}

#[test]
fn connection_stop_cancels_shared_purposes_but_preserves_other_engine_and_history() {
    let (s, _) = store();
    configured(&s);
    let first = manual(&s, "履歴を残す。");
    let old = prepared(&s, &first, "translation");
    s.complete_task(completion(&old[0]), 21).unwrap();
    s.complete_task(completion(&old[1]), 21).unwrap();
    let uid = manual(&s, "雨がやんだ。");
    let translations = prepared(&s, &uid, "translation");
    let grammar = prepared(&s, &uid, "grammar").remove(0);
    command(&s, connection_control(&s, "connection-a", true));
    let state = s.snapshot().unwrap();
    let stopped = state
        .configuration
        .runtime_connections
        .iter()
        .find(|c| c.connection_id == "connection-a")
        .unwrap();
    assert!(stopped.suspended);
    assert_eq!(stopped.affected_purposes.len(), 2);
    for task in [&translations[0], &grammar] {
        assert!(s.validate_task_authority(task.task_id.clone()).is_err());
        assert_eq!(
            s.complete_task(completion(task), 30).unwrap_err().code,
            "task_not_authorized"
        );
        assert_eq!(
            state
                .tasks
                .iter()
                .find(|t| t.task_id == task.task_id)
                .unwrap()
                .status,
            "invalidated"
        );
    }
    s.validate_task_authority(translations[1].task_id.clone())
        .unwrap();
    s.complete_task(completion(&translations[1]), 30).unwrap();
    assert_eq!(s.unit(first, 1).unwrap().translations.len(), 2);
    let next = manual(&s, "次の台詞。");
    let remaining = prepared(&s, &next, "translation");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].connection.connection_id, "connection-b");
    assert_eq!(
        s.prepare_tasks(next, 1, "grammar".into(), 31)
            .unwrap_err()
            .code,
        "connection_suspended"
    );
}

#[test]
fn safety_commands_are_idempotent_and_reject_stale_control_versions() {
    let (s, _) = store();
    configured(&s);
    let pause = connection_control(&s, "connection-a", true);
    let request = CommandEnvelope {
        operation_id: "stable-stop".into(),
        expected_revision: None,
        command: pause.clone(),
    };
    let first = s.dispatch(request.clone(), 10).unwrap();
    let second = s.dispatch(request, 11).unwrap();
    assert_eq!(first.revision, second.revision);
    assert!(second.replayed);
    assert_eq!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "stale-stop".into(),
                expected_revision: None,
                command: pause
            },
            12
        )
        .unwrap_err()
        .code,
        "revision_conflict"
    );
    assert_eq!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "stable-stop".into(),
                expected_revision: None,
                command: connection_control(&s, "connection-a", false)
            },
            12
        )
        .unwrap_err()
        .code,
        "operation_conflict"
    );
    let pause = purpose_control(&s, "global", "grammar", false);
    command(&s, pause.clone());
    assert_eq!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "stale-purpose".into(),
                expected_revision: None,
                command: pause
            },
            13
        )
        .unwrap_err()
        .code,
        "revision_conflict"
    );
}

#[test]
fn save_test_apply_restart_cannot_restore_gates_and_resume_uses_applied_not_draft() {
    let (s, path) = store();
    configured(&s);
    command(&s, connection_control(&s, "connection-a", true));
    command(&s, purpose_control(&s, "global", "translation", false));
    let mut edit = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone();
    edit.connections[0].display_name = "renamed applied connection".into();
    let renamed = save(&s, edit);
    apply(&s, renamed);
    let applied = s.snapshot().unwrap().configuration.applied.unwrap().as_legacy().expect("legacy fixture").clone();
    let mut draft = applied.clone();
    draft.models[0].actual_model_id = Some("UNAPPLIED-MODEL".into());
    draft.connections[0].endpoint = Some("http://127.0.0.1:9/v1".into());
    let draft_id = save(&s, draft);
    s.record_model_test(app_core::domain::testing::ModelTestRecordInput {
        test_id: "explicit-test-while-stopped".into(),
        config_revision_id: draft_id.clone(),
        connection_id: Some("connection-a".into()),
        model_id: None,
        binding_id: None,
        purpose: None,
        stage: "connection".into(),
        request_semantics_hash: content_hash("controlled test"),
        input_scope: "sample".into(),
        unit_id: None,
        text_revision: None,
        input_summary: json!({}),
        response_summary: json!({}),
        output_validation: json!({}),
        state: "passed".into(),
        started_at_ms: 12,
        completed_at_ms: 13,
        error_code: None,
        error_stage: None,
        credentials: vec![],
    })
    .unwrap();
    assert!(s.snapshot().unwrap().configuration.runtime_connections[0].suspended);
    drop(s);
    let s = SqliteStore::open(path, "lifecycle-profile", "restart-server", 30).unwrap();
    let state = s.snapshot().unwrap().configuration;
    assert_eq!(state.draft.as_legacy().expect("legacy fixture").clone().revision_id, draft_id);
    assert_eq!(
        state.applied.as_ref().unwrap().as_legacy().expect("legacy fixture").revision_id,
        applied.revision_id
    );
    assert!(state.runtime_connections[0].suspended);
    assert!(
        !state
            .runtime_purposes
            .iter()
            .find(|p| p.purpose == "translation")
            .unwrap()
            .runtime_enabled
    );
    command(&s, connection_control(&s, "connection-a", false));
    command(&s, purpose_control(&s, "global", "translation", true));
    let next = manual(&s, "再開した。");
    let tasks = prepared(&s, &next, "translation");
    assert_eq!(tasks.len(), 2);
    assert_eq!(
        tasks[0].model.actual_model_id.as_deref(),
        Some("applied-model-a")
    );
    assert_eq!(
        tasks[0].connection.endpoint.as_deref(),
        Some("http://127.0.0.1:1/v1")
    );
    assert_eq!(
        s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone().revision_id,
        draft_id
    );
}

#[test]
fn purpose_stop_is_independent_of_intent_and_other_purposes() {
    let (s, _) = store();
    configured(&s);
    let uid = manual(&s, "用途を分ける。");
    let translations = prepared(&s, &uid, "translation");
    let grammar = prepared(&s, &uid, "grammar").remove(0);
    command(&s, purpose_control(&s, "global", "translation", false));
    for task in translations {
        assert!(s.validate_task_authority(task.task_id).is_err());
    }
    s.complete_task(completion(&grammar), 25).unwrap();
    let view = s.snapshot().unwrap().configuration;
    assert!(
        view.applied
            .unwrap().as_legacy().expect("legacy fixture").clone()
            .bindings
            .iter()
            .find(|b| b.purpose == "translation")
            .unwrap()
            .enabled
    );
    assert!(
        !view
            .runtime_purposes
            .iter()
            .find(|p| p.purpose == "translation")
            .unwrap()
            .runtime_enabled
    );
    let next = manual(&s, "次へ進む。");
    assert_eq!(
        s.prepare_tasks(next.clone(), 1, "translation".into(), 30)
            .unwrap_err()
            .code,
        "purpose_suspended"
    );
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone();
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .enabled = false;
    let revision = save(&s, c);
    apply(&s, revision);
    assert_eq!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "disabled-intent-restore".into(),
                expected_revision: None,
                command: purpose_control(&s, "global", "translation", true)
            },
            31
        )
        .unwrap_err()
        .code,
        "purpose_restore_blocked"
    );
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone();
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .enabled = true;
    let revision = save(&s, c);
    apply(&s, revision);
    assert_eq!(
        s.prepare_tasks(next.clone(), 1, "translation".into(), 32)
            .unwrap_err()
            .code,
        "purpose_suspended"
    );
    command(&s, purpose_control(&s, "global", "translation", true));
    assert_eq!(prepared(&s, &next, "translation").len(), 2);
}

#[test]
fn first_disabled_intent_can_later_be_explicitly_applied_enabled_without_false_suspension() {
    let (s, _) = store();
    let mut c = config();
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .enabled = false;
    let revision = save(&s, c);
    apply(&s, revision);
    let view = s
        .snapshot()
        .unwrap()
        .configuration
        .runtime_purposes
        .into_iter()
        .find(|p| p.purpose == "translation")
        .unwrap();
    assert!(!view.enabled_intent);
    assert!(view.runtime_enabled);
    assert!(view.authorized_engine_ids.is_empty());
    let uid = manual(&s, "明示的に有効化する。");
    assert_eq!(
        s.prepare_tasks(uid.clone(), 1, "translation".into(), 20)
            .unwrap_err()
            .code,
        "purpose_disabled"
    );
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone();
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .enabled = true;
    let revision = save(&s, c);
    apply(&s, revision);
    assert_eq!(prepared(&s, &uid, "translation").len(), 2);
}

fn add_game(s: &SqliteStore, name: &str) -> String {
    let input: GameInput=serde_json::from_value(json!({"displayName":name,"sourceLanguage":"ja","target":{"environmentKind":"crossover","environmentKey":format!("crossover:/private/{name}"),"bottleLocator":format!("/private/{name}"),"launchPath":format!("C:\\{name}.exe"),"canonicalExePath":format!("C:\\{name}.exe"),"targetPathKey":format!("c:/{name}.exe"),"arguments":[],"architecture":"x86","verificationState":"verified"}})).unwrap();
    command(s, Command::SaveGame(Box::new(input)))
        .result_id
        .unwrap()
}
fn game_unit(s: &SqliteStore, game: &str, suffix: &str) -> String {
    let session = format!("session-{suffix}");
    s.open_hook_session(
        HookSessionInput {
            session_id: session.clone(),
            game_id: Some(game.into()),
            environment_key: format!("crossover:/private/{suffix}"),
            target_exe_path: format!("C:\\{suffix}.exe"),
            windows_pid: 100,
            process_created_u64: "100".into(),
            architecture: "x86".into(),
            broker_instance_id: format!("broker-{suffix}"),
            resource: ResourceSnapshotInput {
                snapshot_id: "core-controlled".into(),
                kind: "core".into(),
                version: "controlled".into(),
                manifest_sha256: content_hash("controlled"),
                contract_version: "1".into(),
                metadata_json: "{}".into(),
            },
            target_identity_sha256: content_hash(suffix),
        },
        30,
    )
    .unwrap();
    let event = |sequence: &str| HookTextInput {
        session_id: session.clone(),
        event_id: format!("event-{suffix}-{sequence}"),
        sequence_u64: sequence.into(),
        source: HookSourceInput {
            source_id: format!("source-{suffix}"),
            host_instance_id: format!("host-{suffix}"),
            hook_address_u64: "10".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "HW".into(),
            hook_name: "TextOutW".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash(format!("stream-{suffix}")),
            identity_extensions_json: "{}".into(),
        },
        text: format!("ゲーム{suffix}の台詞{sequence}"),
        source_language: "ja".into(),
    };
    s.receive_hook_text(event("1"), 31).unwrap();
    command(
        s,
        Command::SelectSource {
            source_id: format!("source-{suffix}"),
        },
    );
    s.receive_hook_text(event("2"), 32).unwrap().unwrap()
}

#[test]
fn global_stop_covers_inheriting_games_but_not_explicit_game_override() {
    let (s, _) = store();
    let a = add_game(&s, "a");
    let b = add_game(&s, "b");
    let mut c = config();
    let mut custom = c.bindings[0].clone();
    custom.binding_id = "game-b-translation".into();
    custom.scope_key = format!("game:{b}");
    custom.game_id = Some(b.clone());
    c.bindings.push(custom);
    let revision = save(&s, c);
    apply(&s, revision);
    let ua = game_unit(&s, &a, "a");
    let ub = game_unit(&s, &b, "b");
    let ta = prepared(&s, &ua, "translation");
    let tb = prepared(&s, &ub, "translation");
    command(&s, purpose_control(&s, "global", "translation", false));
    for task in &ta {
        assert!(s.validate_task_authority(task.task_id.clone()).is_err());
    }
    for task in &tb {
        s.complete_task(completion(task), 40).unwrap();
    }
    let views = s.snapshot().unwrap().configuration.runtime_purposes;
    let global = views
        .iter()
        .find(|p| p.scope_key == "global" && p.purpose == "translation")
        .unwrap();
    assert_eq!(global.affected_game_ids, vec![a]);
    let custom = views
        .iter()
        .find(|p| p.scope_key == format!("game:{b}"))
        .unwrap();
    assert_eq!(custom.affected_game_ids, vec![b]);
    assert_eq!(custom.authorized_engine_ids.len(), 2);
}

#[test]
fn completion_and_suspend_share_writer_order_and_late_result_never_reappears() {
    for _ in 0..6 {
        let (s, _) = store();
        configured(&s);
        let uid = manual(&s, "完了と停止が競合する。");
        let task = prepared(&s, &uid, "grammar").remove(0);
        let barrier = Arc::new(Barrier::new(3));
        let finish = {
            let s = s.clone();
            let t = task.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                s.complete_task(completion(&t), 50)
            })
        };
        let pause = {
            let s = s.clone();
            let b = barrier.clone();
            let c = connection_control(&s, "connection-a", true);
            std::thread::spawn(move || {
                b.wait();
                command(&s, c)
            })
        };
        barrier.wait();
        let completed = finish.join().unwrap().is_ok();
        pause.join().unwrap();
        let state = s.snapshot().unwrap();
        assert_eq!(
            state
                .study_results
                .iter()
                .filter(|r| r.task_id == task.task_id)
                .count(),
            usize::from(completed)
        );
        assert!(s.complete_task(completion(&task), 60).is_err());
        assert!(state.configuration.runtime_connections[0].suspended);
    }
    acceptance_evidence::record(
        "completion-suspend-transaction-race",
        &[("STORAGE-12", 1)],
        serde_json::json!({"actual_test":"completion_and_suspend_share_writer_order_and_late_result_never_reappears"}),
    );
}

struct Hook;
impl HookBackend for Hook {
    fn targets(&self) -> AppResult<TargetCandidates> {
        unreachable!()
    }
    fn associate(&self, _: AssociateRequest) -> AppResult<HookSessionInput> {
        unreachable!()
    }
    fn disconnect(&self) -> AppResult<()> {
        Ok(())
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        let (_, r) = mpsc::channel();
        Ok(r)
    }
    fn resource_info(&self) -> ResourceInfo {
        unreachable!()
    }
}
struct Language;
impl LanguageServices for Language {
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
struct Vault;
impl CredentialVault for Vault {
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        unreachable!()
    }
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        unreachable!()
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        unreachable!()
    }
}
type Delta = Arc<dyn Fn(String) + Send + Sync>;
struct Call {
    task_id: String,
    connection: String,
    purpose: String,
    delta: Delta,
    reply: oneshot::Sender<String>,
}
struct Gateway {
    calls: async_mpsc::UnboundedSender<Call>,
    deltas: Mutex<Vec<Delta>>,
}
impl ModelGateway for Gateway {
    fn execute<'a>(
        &'a self,
        t: &'a TaskDispatch,
        _: &'a RequestPlan,
        delta: Delta,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            let (reply, receive) = oneshot::channel();
            self.deltas.lock().unwrap().push(delta.clone());
            self.calls
                .send(Call {
                    task_id: t.task_id.clone(),
                    connection: t.connection.connection_id.clone(),
                    purpose: t.binding.purpose.clone(),
                    delta,
                    reply,
                })
                .unwrap();
            receive
                .await
                .map_err(|_| AppError::new("controlled_cancelled", "test", "receiver cancelled"))
        })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a Connection,
        _: &'a RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Ok("explicit test".into()) })
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Ok(vec!["controlled-model".into()]) })
    }
}
async fn runtime_command(app: &Arc<ApplicationRuntime>, command: Command) -> CommandReceipt {
    app.dispatch(CommandEnvelope {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: None,
        command,
    })
    .await
    .unwrap()
}
async fn call(calls: &mut async_mpsc::UnboundedReceiver<Call>) -> Call {
    tokio::time::timeout(Duration::from_secs(5), calls.recv())
        .await
        .expect("no controlled gateway request")
        .unwrap()
}
async fn wait(app: &Arc<ApplicationRuntime>, predicate: impl Fn(&AppSnapshot) -> bool) {
    let mut changes = app.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if predicate(&app.snapshot().unwrap()) {
                break;
            }
            changes.changed().await.unwrap();
        }
    })
    .await
    .expect("runtime did not reach expected state");
}
fn runtime(
    s: Arc<SqliteStore>,
) -> (
    Arc<ApplicationRuntime>,
    Arc<Gateway>,
    async_mpsc::UnboundedReceiver<Call>,
) {
    let (tx, rx) = async_mpsc::unbounded_channel();
    let gateway = Arc::new(Gateway {
        calls: tx,
        deltas: Mutex::new(vec![]),
    });
    let app = ApplicationRuntime::new(
        s,
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        gateway.clone(),
        Arc::new(Language),
        Arc::new(Vault),
    )
    .unwrap();
    (app, gateway, rx)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_immediate_stop_aborts_shared_calls_erases_partials_and_ignores_late_callbacks() {
    let (s, _) = store();
    configured(&s);
    let (app, _gateway, mut calls) = runtime(s.clone());
    let uid = runtime_command(
        &app,
        Command::ManualInput {
            text: "実際のランタイム制御。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let one = call(&mut calls).await;
    let two = call(&mut calls).await;
    app.study(uid.clone(), 1, "grammar".into(), None).unwrap();
    let three = call(&mut calls).await;
    let mut active = vec![one, two, three];
    for call in &active {
        (call.delta)(format!("partial {}", call.task_id));
    }
    wait(&app, |s| {
        s.tasks.iter().filter(|t| t.partial_text.is_some()).count() == 3
    })
    .await;
    runtime_command(&app, connection_control(&s, "connection-a", true)).await;
    for stopped in active.iter_mut().filter(|c| c.connection == "connection-a") {
        tokio::time::timeout(Duration::from_secs(5), stopped.reply.closed())
            .await
            .unwrap();
        (stopped.delta)("late invalidated partial must not reappear".into());
    }
    let snapshot = app.snapshot().unwrap();
    assert_eq!(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.partial_text.is_some())
            .count(),
        1
    );
    assert!(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.status == "invalidated")
            .all(|t| t.partial_text.is_none())
    );
    let unaffected = active
        .into_iter()
        .find(|c| c.connection == "connection-b")
        .unwrap();
    assert_eq!(unaffected.purpose, "translation");
    unaffected.reply.send("独立引擎译文".into()).unwrap();
    wait(&app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| u.translations.iter().any(|t| t.text == "独立引擎译文"))
    })
    .await;
    let second = runtime_command(
        &app,
        Command::ManualInput {
            text: "停止後も原文は続く。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let remaining = call(&mut calls).await;
    assert_eq!(remaining.connection, "connection-b");
    assert_eq!(
        app.study(second, 1, "grammar".into(), None)
            .unwrap_err()
            .code,
        "connection_suspended"
    );
    assert!(calls.try_recv().is_err());
    runtime_command(
        &app,
        Command::CancelTask {
            task_id: remaining.task_id,
        },
    )
    .await;
    app.shutdown().await.unwrap();
    acceptance_evidence::record(
        "runtime-stop-late-callback",
        &[("STORAGE-12", 1)],
        serde_json::json!({"actual_test":"runtime_immediate_stop_aborts_shared_calls_erases_partials_and_ignores_late_callbacks"}),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn purpose_stop_cancels_queued_and_running_network_work_without_touching_grammar() {
    let (s, _) = store();
    let mut c = config();
    c.bindings[0]
        .schedule
        .insert("concurrency".into(), json!(1));
    let revision = save(&s, c);
    apply(&s, revision);
    let (app, _gateway, mut calls) = runtime(s.clone());
    let first = runtime_command(
        &app,
        Command::ManualInput {
            text: "最初の文。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let mut held = call(&mut calls).await;
    let second = runtime_command(
        &app,
        Command::ManualInput {
            text: "待機する文。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    wait(&app, |s| {
        s.tasks
            .iter()
            .filter(|t| t.purpose == "translation" && t.status == "running")
            .count()
            == 4
    })
    .await;
    app.study(first, 1, "grammar".into(), None).unwrap();
    let grammar = call(&mut calls).await;
    assert_eq!(grammar.purpose, "grammar");
    runtime_command(&app, purpose_control(&s, "global", "translation", false)).await;
    tokio::time::timeout(Duration::from_secs(5), held.reply.closed())
        .await
        .unwrap();
    (held.delta)("late stopped translation".into());
    assert!(calls.try_recv().is_err());
    grammar.reply.send("文法の説明。".into()).unwrap();
    wait(&app, |s| !s.study_results.is_empty()).await;
    assert!(s.unit(second, 1).unwrap().translations.is_empty());
    let snapshot = app.snapshot().unwrap();
    assert_eq!(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.purpose == "translation" && t.status == "invalidated")
            .count(),
        4
    );
    assert!(snapshot.tasks.iter().all(|t| t.partial_text.is_none()));
    app.shutdown().await.unwrap();
}

fn with_shared_credentials(s: &SqliteStore) -> Configuration {
    for id in ["first-key", "second-key"] {
        s.register_credential(
            CredentialReference {
                credential_id: id.into(),
                credential_version_id: format!("immutable-vault-{id}"),
                pool_id: format!("initial-{id}"),
                pool_revision: 1,
                label: "controlled credential".into(),
                masked_hint: "••••".into(),
            },
            2,
        )
        .unwrap();
    }
    command(
        s,
        Command::SaveCredentialPool {
            pool: CredentialPoolInput {
                pool_id: Some("shared-pool".into()),
                expected_revision: 0,
                policy: "round_robin".into(),
                metadata: Default::default(),
                members: ["first-key", "second-key"]
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
    );
    let mut c = config();
    c.connections[0].auth_mode = "api_key".into();
    c.connections[0].credential_pool_id = Some("shared-pool".into());
    c.connections[0].credential_pool_revision = Some(1);
    c
}
struct CleanupVault {
    fail: std::sync::atomic::AtomicBool,
    deleted: Mutex<Vec<String>>,
}
impl CredentialVault for CleanupVault {
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        Ok(())
    }
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        Ok(b"controlled-only".to_vec())
    }
    fn delete(&self, key: &str) -> AppResult<()> {
        self.deleted.lock().unwrap().push(key.into());
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err(AppError::new(
                "controlled_vault_failure",
                "test",
                "delete unavailable",
            ))
        } else {
            Ok(())
        }
    }
}
fn revoke_request(
    impact: &CredentialRevocationImpact,
    operation: &str,
) -> CredentialRevocationRequest {
    CredentialRevocationRequest {
        operation_id: operation.into(),
        credential_id: impact.credential_id.clone(),
        expected_lifecycle_revision: impact.lifecycle_revision,
        expected_impact_hash: impact.impact_hash.clone(),
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credential_revocation_cas_shared_pool_abort_and_cleanup_retry_are_authoritative() {
    let (s, path) = store();
    let c = with_shared_credentials(&s);
    let revision = save(&s, c);
    apply(&s, revision);
    let (tx, mut calls) = async_mpsc::unbounded_channel();
    let gateway = Arc::new(Gateway {
        calls: tx,
        deltas: Mutex::new(vec![]),
    });
    let vault = Arc::new(CleanupVault {
        fail: std::sync::atomic::AtomicBool::new(true),
        deleted: Mutex::new(vec![]),
    });
    let app = ApplicationRuntime::new(
        s.clone(),
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        gateway,
        Arc::new(Language),
        vault.clone(),
    )
    .unwrap();
    let impact = app
        .credential_revocation_impact("first-key".into())
        .unwrap();
    assert_eq!(impact.affected_connection_ids, vec!["connection-a"]);
    assert_eq!(impact.affected_purposes.len(), 2);
    let request = revoke_request(&impact, "confirmed-revocation");
    let mut changed = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone();
    changed.connections[0].display_name = "new applied identity".into();
    let revision = save(&s, changed.clone());
    assert_eq!(
        app.credential_revocation_impact("first-key".into())
            .unwrap()
            .impact_hash,
        impact.impact_hash,
        "draft saves do not change applied revocation authority"
    );
    apply(&s, revision);
    assert_eq!(
        app.revoke_credential(request).await.unwrap_err().code,
        "revision_conflict"
    );
    assert!(vault.deleted.lock().unwrap().is_empty());
    let uid = runtime_command(
        &app,
        Command::ManualInput {
            text: "共有キーを使う。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let one = call(&mut calls).await;
    let two = call(&mut calls).await;
    app.study(uid, 1, "grammar".into(), None).unwrap();
    let three = call(&mut calls).await;
    let credential_ids: Vec<String> = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
    .prepare(
        "SELECT credential_id FROM request_attempts WHERE credential_id IS NOT NULL ORDER BY rowid",
    )
    .unwrap()
    .query_map([], |r| r.get(0))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap();
    assert!(credential_ids.contains(&"first-key".into()));
    assert!(credential_ids.contains(&"second-key".into()));
    let impact = app
        .credential_revocation_impact("first-key".into())
        .unwrap();
    let request = revoke_request(&impact, "actual-revocation");
    assert_eq!(
        app.revoke_credential(request.clone())
            .await
            .unwrap_err()
            .code,
        "credential_cleanup_pending"
    );
    let mut active = vec![one, two, three];
    for stopped in active.iter_mut().filter(|c| c.connection == "connection-a") {
        tokio::time::timeout(Duration::from_secs(5), stopped.reply.closed())
            .await
            .unwrap();
        (stopped.delta)("revoked late callback".into());
    }
    let state = app.snapshot().unwrap();
    assert_eq!(
        state
            .credentials
            .iter()
            .find(|c| c.credential_id == "first-key")
            .unwrap()
            .state,
        "revoked"
    );
    assert!(
        state
            .configuration
            .runtime_connections
            .iter()
            .find(|c| c.connection_id == "connection-a")
            .unwrap()
            .suspended
    );
    assert!(
        !state
            .configuration
            .runtime_connections
            .iter()
            .find(|c| c.connection_id == "connection-b")
            .unwrap()
            .suspended
    );
    assert!(
        state
            .tasks
            .iter()
            .filter(|t| t.error_code.as_deref() == Some("credential_revoked"))
            .all(|t| t.status == "invalidated" && t.partial_text.is_none())
    );
    assert_eq!(
        vault.deleted.lock().unwrap().as_slice(),
        ["immutable-vault-first-key"]
    );
    vault.fail.store(false, std::sync::atomic::Ordering::SeqCst);
    let receipt = app.revoke_credential(request.clone()).await.unwrap();
    assert!(receipt.replayed);
    assert!(
        s.pending_credentials()
            .unwrap()
            .iter()
            .all(|c| c.credential_id != "first-key")
    );
    let deleted = vault.deleted.lock().unwrap().len();
    assert!(app.revoke_credential(request).await.unwrap().replayed);
    assert_eq!(
        vault.deleted.lock().unwrap().len(),
        deleted,
        "successful retry is idempotent and does not delete another secret"
    );
    let unaffected = active
        .into_iter()
        .find(|c| c.connection == "connection-b")
        .unwrap();
    runtime_command(
        &app,
        Command::CancelTask {
            task_id: unaffected.task_id,
        },
    )
    .await;
    app.shutdown().await.unwrap();
    acceptance_evidence::record(
        "credential-revoke-cas-cleanup",
        &[("STORAGE-11", 1)],
        serde_json::json!({"actual_test":"credential_revocation_cas_shared_pool_abort_and_cleanup_retry_are_authoritative"}),
    );
}

#[test]
fn revoking_a_historical_pool_does_not_suspend_the_current_unrelated_pool() {
    let (s, _) = store();
    let c = with_shared_credentials(&s);
    let revision = save(&s, c);
    apply(&s, revision);
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy fixture").clone();
    let con = c
        .connections
        .iter_mut()
        .find(|c| c.connection_id == "connection-a")
        .unwrap();
    con.credential_pool_id = Some("initial-second-key".into());
    con.credential_pool_revision = Some(1);
    let revision = save(&s, c);
    apply(&s, revision);
    s.revoke_credential("first-key".into(), 40).unwrap();
    assert!(
        !s.snapshot()
            .unwrap()
            .configuration
            .runtime_connections
            .iter()
            .find(|c| c.connection_id == "connection-a")
            .unwrap()
            .suspended
    );
    let uid = manual(&s, "現在の独立したキー。");
    let tasks = prepared(&s, &uid, "translation");
    assert_eq!(tasks.len(), 2);
    assert_eq!(
        tasks[0].credential_vault_ref.as_deref(),
        Some("immutable-vault-second-key")
    );
}

#[tokio::test]
async fn failed_vault_cleanup_survives_restart_and_recovery_never_reenables_connection() {
    let (s, path) = store();
    let c = with_shared_credentials(&s);
    let revision = save(&s, c);
    apply(&s, revision);
    let (tx, _rx) = async_mpsc::unbounded_channel();
    let first_vault = Arc::new(CleanupVault {
        fail: std::sync::atomic::AtomicBool::new(true),
        deleted: Mutex::new(vec![]),
    });
    let app = ApplicationRuntime::new(
        s.clone(),
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        Arc::new(Gateway {
            calls: tx,
            deltas: Mutex::new(vec![]),
        }),
        Arc::new(Language),
        first_vault,
    )
    .unwrap();
    let impact = app
        .credential_revocation_impact("first-key".into())
        .unwrap();
    assert_eq!(
        app.revoke_credential(revoke_request(&impact, "before-restart"))
            .await
            .unwrap_err()
            .code,
        "credential_cleanup_pending"
    );
    drop(app);
    drop(s);
    let s = Arc::new(SqliteStore::open(path, "lifecycle-profile", "new-instance", 100).unwrap());
    let state = s.snapshot().unwrap();
    assert_eq!(
        state
            .credentials
            .iter()
            .find(|c| c.credential_id == "first-key")
            .unwrap()
            .state,
        "revoked"
    );
    assert!(
        state
            .configuration
            .runtime_connections
            .iter()
            .find(|c| c.connection_id == "connection-a")
            .unwrap()
            .suspended
    );
    let (tx, _rx) = async_mpsc::unbounded_channel();
    let vault = Arc::new(CleanupVault {
        fail: std::sync::atomic::AtomicBool::new(false),
        deleted: Mutex::new(vec![]),
    });
    let app = ApplicationRuntime::new(
        s.clone(),
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        Arc::new(Gateway {
            calls: tx,
            deltas: Mutex::new(vec![]),
        }),
        Arc::new(Language),
        vault.clone(),
    )
    .unwrap();
    app.recover_credentials().unwrap();
    assert_eq!(
        vault.deleted.lock().unwrap().as_slice(),
        ["immutable-vault-first-key"]
    );
    assert!(s.pending_credentials().unwrap().is_empty());
    assert!(
        s.snapshot()
            .unwrap()
            .configuration
            .runtime_connections
            .iter()
            .find(|c| c.connection_id == "connection-a")
            .unwrap()
            .suspended
    );
    acceptance_evidence::record(
        "revoked-cleanup-restart",
        &[("STORAGE-11", 1)],
        serde_json::json!({"actual_test":"failed_vault_cleanup_survives_restart_and_recovery_never_reenables_connection"}),
    );
}

/// Wrap the production Repository without replacing any storage/auth semantics.
/// Only the cleanup controller's captured snapshot is held, forcing the precise
/// old-snapshot/new-registration interleaving without relying on scheduler sleeps.
struct SnapshotBarrierRepository {
    inner: Arc<SqliteStore>,
    barrier: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
    prepared_barrier: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
}
macro_rules! delegate_repository {
    ($(fn $method:ident(&self $(,$arg:ident:$ty:ty)*) -> $result:ty;)+) => {
        $(fn $method(&self,$($arg:$ty),*) -> $result { self.inner.$method($($arg),*) })+
    };
}
impl ProductRepository for SnapshotBarrierRepository {
    delegate_repository! {
        fn store_provider_preset(&self,preset:StoredProviderPreset,now:i64)->AppResult<()>;
        fn provider_preset(&self,key:ProviderPresetKey)->AppResult<StoredProviderPreset>;
        fn select_credential_for_kind(&self,pool:String,rev:u32,kind:app_core::domain::model_config::credentials::CredentialMaterialKind)->AppResult<app_core::domain::credentials::CredentialSelection>;
        fn credential_lease_metadata(&self,pool:String,rev:u32,vault:String,kind:app_core::domain::model_config::credentials::CredentialMaterialKind)->AppResult<app_core::domain::credentials::CredentialLeaseMetadata>;
        fn begin_credential_material(&self,reference:CredentialReference,storage:app_core::domain::model_config::credentials::CredentialStorageMetadata,now:i64)->AppResult<()>;

        fn preflight_window_unbind(&self,envelope:CommandEnvelope)->AppResult<Option<CommandReceipt>>;
        fn commit_window_unbind(&self,envelope:CommandEnvelope,now:i64)->AppResult<CommandReceipt>;
        fn preflight_disconnect(&self,envelope:CommandEnvelope)->AppResult<Option<CommandReceipt>>;
        fn flush_capture_integrity(&self,session:Option<String>)->AppResult<()>;
        fn preview_preferences(&self,preferences:Preferences)->AppResult<Preferences>;
        fn preferences_scope(&self,scope:String)->AppResult<ScopedPreferences>;
        fn begin_capture_operation(&self,operation:String,hash:String,now:i64)->AppResult<()>;
        fn finish_capture_operation(&self,operation:String,session:Option<String>,now:i64)->AppResult<()>;
        fn record_hook_event(&self,input:app_core::ports::runtime::HookEventInput)->AppResult<()>;
        fn ingest_hook_text_with_policy(&self,input:HookTextInput,consume:bool,now:i64)->AppResult<app_core::domain::capture::CaptureOutcome>;
        fn session_records(&self,query:SessionRecordQuery,now:i64)->AppResult<SessionRecordPage>;
        fn prepare_session_export(&self,request:SessionExportRequest,now:i64)->AppResult<SessionExportPlan>;
        fn session_export_page(&self,id:String,cursor:Option<String>,size:u32,now:i64)->AppResult<SessionRecordPage>;
        fn close_session_export(&self,id:String)->AppResult<()>;

        fn unit(&self, id:String, revision:u32)->AppResult<ReadingUnit>;
        fn reading_unit_projection(&self,id:String,revision:u32)->AppResult<ReadingUnitProjection>;
        fn prepare_tasks_for_operation(&self,id:String,revision:u32,purpose:String,operation:String,now:i64)->AppResult<Vec<TaskDispatch>>;
        fn prepare_uncached_translation_for_operation(&self,id:String,revision:u32,operation:String,now:i64)->AppResult<app_core::domain::PreparedTranslation>;
        fn record_model_test(&self,input:app_core::domain::testing::ModelTestRecordInput)->AppResult<()>;
        fn credential_test_reference(&self,pool:String,revision:u32,vault:String,outcome:String)->AppResult<app_core::domain::testing::TestCredentialUse>;
        fn maintain_storage(&self,policy:app_core::domain::retention::RetentionPolicy,protected:app_core::domain::retention::ProtectedObjects,operation:String,now:i64)->AppResult<app_core::domain::retention::MaintenanceReceipt>;
        fn select_credential(&self,pool:String,revision:u32)->AppResult<app_core::domain::credentials::CredentialSelection>;
        fn dispatch(&self,envelope:CommandEnvelope,now:i64)->AppResult<CommandReceipt>;
        fn prepare_tasks(&self,id:String,revision:u32,purpose:String,now:i64)->AppResult<Vec<TaskDispatch>>;
        fn complete_task(&self,result:TaskCompletion,now:i64)->AppResult<CommandReceipt>;
        fn validate_task_authority(&self,id:String)->AppResult<()>;
        fn retry_attempt(&self,id:String,attempt:String,code:String,now:i64)->AppResult<String>;
        fn fail_task(&self,id:String,code:String,now:i64)->AppResult<()>;
        fn open_hook_session(&self,input:HookSessionInput,now:i64)->AppResult<()>;
        fn receive_hook_text(&self,input:HookTextInput,now:i64)->AppResult<Option<String>>;
        fn ingest_hook_text(&self,input:HookTextInput,now:i64)->AppResult<app_core::domain::capture::CaptureOutcome>;
        fn flush_hook_capture(&self,request:app_core::domain::capture::CaptureFlush,now:i64)->AppResult<Vec<app_core::domain::capture::CaptureVersion>>;
        fn prepare_capture_tasks(&self,version:app_core::domain::capture::CaptureVersion,operation:String,now:i64)->AppResult<Vec<TaskDispatch>>;
        fn set_tokens(&self,input:TokenSnapshotInput,now:i64)->AppResult<()>;
        fn record_diagnostic(&self,diagnostic:Diagnostic,now:i64)->AppResult<()>;
        fn begin_credential(&self,reference:CredentialReference,now:i64)->AppResult<()>;
        fn activate_credential(&self,id:String,now:i64)->AppResult<()>;
        fn revoke_credential(&self,id:String,now:i64)->AppResult<()>;
        fn mark_credential_cleaned(&self,id:String,now:i64)->AppResult<()>;
        fn pending_credentials(&self)->AppResult<Vec<CredentialReference>>;
        fn register_credential(&self,reference:CredentialReference,now:i64)->AppResult<()>;
        fn credential_for_pool(&self,pool:String,revision:u32)->AppResult<String>;
        fn history(&self,game:Option<String>,cursor:Option<String>,limit:u32)->AppResult<HistoryPage>;
        fn history_page(&self,query:HistoryQuery)->AppResult<HistoryPage>;
        fn history_sessions(&self,query:HistorySessionsQuery)->AppResult<HistorySessionsPage>;
    }
    fn prepare_study_tasks(
        &self,
        id: String,
        revision: u32,
        purpose: String,
        token: Option<u32>,
        operation: String,
        now: i64,
    ) -> AppResult<Vec<TaskDispatch>> {
        let tasks = self
            .inner
            .prepare_study_tasks(id, revision, purpose, token, operation, now)?;
        let barrier = self.prepared_barrier.lock().unwrap().take();
        if let Some((prepared, release)) = barrier {
            prepared.send(()).unwrap();
            release
                .recv_timeout(Duration::from_secs(10))
                .expect("test did not release prepared request");
        }
        Ok(tasks)
    }
    fn snapshot(&self) -> AppResult<AppSnapshot> {
        let snapshot = self.inner.snapshot()?;
        if std::thread::current().name() == Some("configuration-cleanup-race") {
            let barrier = self.barrier.lock().unwrap().take();
            if let Some((captured, release)) = barrier {
                captured.send(()).unwrap();
                release
                    .recv_timeout(Duration::from_secs(10))
                    .expect("test did not release captured authority snapshot");
            }
        }
        Ok(snapshot)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn old_cleanup_snapshot_cannot_abort_independent_task_registered_after_capture() {
    let (s, _) = store();
    let mut c = config();
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .engines
        .retain(|e| e.model_id == "model-a");
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "grammar")
        .unwrap()
        .engines[0]
        .model_id = "model-b".into();
    let revision = save(&s, c);
    apply(&s, revision);
    let (captured_tx, captured_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let repository = Arc::new(SnapshotBarrierRepository {
        inner: s.clone(),
        barrier: Mutex::new(Some((captured_tx, release_rx))),
        prepared_barrier: Mutex::new(None),
    });
    let (tx, mut calls) = async_mpsc::unbounded_channel();
    let app = ApplicationRuntime::new(
        repository,
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        Arc::new(Gateway {
            calls: tx,
            deltas: Mutex::new(vec![]),
        }),
        Arc::new(Language),
        Arc::new(Vault),
    )
    .unwrap();
    let uid = runtime_command(
        &app,
        Command::ManualInput {
            text: "独立タスクの登録競合。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let mut a = call(&mut calls).await;
    assert_eq!(a.connection, "connection-a");
    let stop = connection_control(&s, "connection-a", true);
    let pause = {
        let app = app.clone();
        let handle = tokio::runtime::Handle::current();
        std::thread::Builder::new()
            .name("configuration-cleanup-race".into())
            .spawn(move || handle.block_on(runtime_command(&app, stop)))
            .unwrap()
    };
    tokio::task::spawn_blocking(move || captured_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    // This task did not exist in the captured authority snapshot. Force it all
    // the way through actual registration and into its independent gateway.
    let b_task = app.study(uid.clone(), 1, "grammar".into(), None).unwrap();
    let b = call(&mut calls).await;
    assert_eq!(b.connection, "connection-b");
    assert_eq!(b.task_id, b_task.task_id);
    (b.delta)("independent B partial must survive cleanup".into());
    assert!(
        app.snapshot()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task_id == b.task_id)
            .unwrap()
            .partial_text
            .as_deref()
            == Some("independent B partial must survive cleanup")
    );
    assert_eq!(
        s.snapshot()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task_id == b.task_id)
            .unwrap()
            .status,
        "running"
    );
    release_tx.send(()).unwrap();
    tokio::task::spawn_blocking(move || pause.join().unwrap())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), a.reply.closed())
        .await
        .unwrap();
    assert!(a.reply.send("invalidated A result".into()).is_err());
    assert_eq!(
        app.snapshot()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task_id == b.task_id)
            .unwrap()
            .partial_text
            .as_deref(),
        Some("independent B partial must survive cleanup")
    );
    // The old implementation aborts B here while leaving its DB task running.
    b.reply
        .send("独立した文法説明。".into())
        .expect("unrelated newly registered B task was incorrectly aborted");
    wait(&app, |s| {
        s.study_results.iter().any(|r| r.task_id == b_task.task_id)
    })
    .await;
    let state = s.snapshot().unwrap();
    assert_eq!(
        state
            .tasks
            .iter()
            .find(|t| t.task_id == b_task.task_id)
            .unwrap()
            .status,
        "completed"
    );
    assert_eq!(
        state
            .tasks
            .iter()
            .find(|t| t.task_id == a.task_id)
            .unwrap()
            .status,
        "invalidated"
    );
    assert!(
        state
            .tasks
            .iter()
            .all(|t| !matches!(t.status.as_str(), "queued" | "running"))
    );
    assert!(s.unit(uid, 1).unwrap().translations.is_empty());
    app.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_prepared_before_stop_but_registered_after_cleanup_never_reaches_gateway() {
    let (s, _) = store();
    let mut c = config();
    c.bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .engines
        .retain(|e| e.model_id == "model-b");
    let revision = save(&s, c);
    apply(&s, revision);
    let (prepared_tx, prepared_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let repository = Arc::new(SnapshotBarrierRepository {
        inner: s.clone(),
        barrier: Mutex::new(None),
        prepared_barrier: Mutex::new(Some((prepared_tx, release_rx))),
    });
    let (tx, mut calls) = async_mpsc::unbounded_channel();
    let app = ApplicationRuntime::new(
        repository,
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        Arc::new(Gateway {
            calls: tx,
            deltas: Mutex::new(vec![]),
        }),
        Arc::new(Language),
        Arc::new(Vault),
    )
    .unwrap();
    let uid = runtime_command(
        &app,
        Command::ManualInput {
            text: "停止より先に準備したリクエスト。".into(),
            source_language: "ja".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let b = call(&mut calls).await;
    assert_eq!(b.connection, "connection-b");
    let study = {
        let app = app.clone();
        let handle = tokio::runtime::Handle::current();
        let uid = uid.clone();
        std::thread::spawn(move || {
            handle.block_on(async move { app.study(uid, 1, "grammar".into(), None) })
        })
    };
    tokio::task::spawn_blocking(move || prepared_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    runtime_command(&app, connection_control(&s, "connection-a", true)).await;
    let invalidated = s
        .snapshot()
        .unwrap()
        .tasks
        .into_iter()
        .find(|t| t.purpose == "grammar")
        .unwrap();
    assert_eq!(invalidated.status, "invalidated");
    release_tx.send(()).unwrap();
    let returned = tokio::task::spawn_blocking(move || study.join().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(returned.task_id, invalidated.task_id);
    wait(&app, |s| {
        s.diagnostics
            .iter()
            .any(|d| d.code == "task_not_authorized")
    })
    .await;
    assert!(
        calls.try_recv().is_err(),
        "post-stop registration must fail authority before any network request"
    );
    assert_eq!(
        s.snapshot()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task_id == invalidated.task_id)
            .unwrap()
            .status,
        "invalidated"
    );
    b.reply.send("独立エンジンの結果。".into()).unwrap();
    wait(&app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| !u.translations.is_empty())
    })
    .await;
    app.shutdown().await.unwrap();
}
