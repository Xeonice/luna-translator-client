//! Real runtime/SQLite current-status regression. Controlled ports are not live-model evidence.
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{TaskCompletion, TaskDispatch, capture::*, content_hash},
    ports::{ProductRepository, runtime::*},
};
use app_infra::{platform::SystemIdentity, storage::SqliteStore};
use rusqlite::{Connection as SqliteConnection, OpenFlags, params};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::{mpsc as async_mpsc, oneshot};

fn configuration(two_engines: bool) -> Configuration {
    let mut value = json!({
        "revisionId": "",
        "connections": [{"connectionId":"connection", "displayName":"同名服务", "protocol":"openai_compatible", "deployment":"local", "authMode":"none", "endpoint":"http://127.0.0.1:1234/v1", "proxyMode":"disabled", "networkMetadata":{}, "availabilityIntent":true}],
        "models":[{"modelId":"model-a", "connectionId":"connection", "actualModelId":"controlled-a", "displayName":"同名模型", "selectionSource":"manual", "capabilities":{}, "parameters":{}}],
        "bindings":[{"bindingId":"binding", "scopeKey":"global", "purpose":"translation", "bindingMode":"override", "enabled":true, "outputMode":"text", "sourceLanguage":"ja", "targetLanguage":"zh-CN", "context":{}, "schedule":{"concurrency":2}, "parameters":{}, "engines":[{"engineId":"engine-a", "modelId":"model-a", "enabled":true, "parameters":{}}]}],
        "templates":[], "termSets":[]
    });
    if two_engines {
        value["models"].as_array_mut().unwrap().push(json!({"modelId":"model-b", "connectionId":"connection", "actualModelId":"controlled-b", "displayName":"同名模型", "selectionSource":"manual", "capabilities":{}, "parameters":{}}));
        value["bindings"][0]["engines"].as_array_mut().unwrap().push(json!({"engineId":"engine-b", "modelId":"model-b", "enabled":true, "parameters":{}}));
    }
    serde_json::from_value(value).unwrap()
}

struct StoreHarness {
    store: Arc<SqliteStore>,
    path: PathBuf,
    operation: AtomicUsize,
}
impl StoreHarness {
    fn new(two_engines: bool) -> Self {
        let path = std::env::temp_dir().join(format!(
            "luna-translation-current-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let h = Self {
            store: Arc::new(
                SqliteStore::open(&path, "current-profile", "current-server", 100).unwrap(),
            ),
            path,
            operation: AtomicUsize::new(0),
        };
        h.apply(configuration(two_engines), 101);
        h
    }
    fn command(&self, command: Command, now: i64) -> CommandReceipt {
        self.store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!(
                        "current-op-{}",
                        self.operation.fetch_add(1, Ordering::SeqCst)
                    ),
                    expected_revision: None,
                    command,
                },
                now,
            )
            .unwrap()
    }
    fn apply(&self, config: Configuration, now: i64) {
        let state = self.store.snapshot().unwrap().configuration;
        let saved = self.command(
            Command::SaveConfiguration {
                configuration: config,
                expected_head_revision: state.draft_head_revision,
            },
            now,
        );
        self.command(
            Command::ApplyConfiguration {
                revision_id: saved.result_id.unwrap(),
                expected_head_revision: state.applied_head_revision,
            },
            now + 1,
        );
    }
    fn unit(&self, text: &str, now: i64) -> String {
        self.command(
            Command::ManualInput {
                text: text.into(),
                source_language: "ja".into(),
            },
            now,
        )
        .result_id
        .unwrap()
    }
    fn prepare(&self, unit: &str, operation: &str, now: i64) -> Vec<TaskDispatch> {
        self.store
            .prepare_uncached_translation_for_operation(unit.into(), 1, operation.into(), now)
            .unwrap()
            .dispatches
    }
    fn view(&self, task: &str) -> TaskView {
        self.store
            .snapshot()
            .unwrap()
            .tasks
            .into_iter()
            .find(|v| v.task_id == task)
            .unwrap()
    }
    fn read(&self) -> SqliteConnection {
        SqliteConnection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
    }
    fn fail(&self, task: &TaskDispatch, code: &str, now: i64) {
        self.store
            .fail_task(task.task_id.clone(), code.into(), now)
            .unwrap();
    }
    fn complete(&self, task: &TaskDispatch, text: &str, now: i64) {
        self.store
            .complete_task(completion(task, text), now)
            .unwrap();
    }
}

fn completion(task: &TaskDispatch, text: &str) -> TaskCompletion {
    TaskCompletion {
        task_id: task.task_id.clone(),
        attempt_id: task.attempt_id.clone(),
        text: text.into(),
        output_mode: "text".into(),
        structured: json!({}),
        target_language: "zh-CN".into(),
        provider_label: "同名服务".into(),
    }
}
fn assert_current(h: &StoreHarness, view: &TaskView, current: bool) {
    assert_current_in(h.store.as_ref(), view, current);
}
fn assert_current_in(store: &SqliteStore, view: &TaskView, current: bool) {
    let projection = store
        .reading_unit_projection(view.unit_id.clone(), view.text_revision)
        .unwrap();
    let represented = projection.translation_state.engines.iter().any(|engine| {
        matches!(&engine.status, TranslationEngineStatus::Current { task } if task.task_id == view.task_id)
    });
    assert_eq!(
        represented, current,
        "current must come from the fixed-object projection: {projection:?}"
    );
    let value = serde_json::to_value(view).unwrap();
    assert!(value.get("isCurrentForReading").is_none());
}
fn status(h: &StoreHarness, unit: &str, engine: &str) -> TranslationEngineStatus {
    h.store
        .reading_unit_projection(unit.into(), 1)
        .unwrap()
        .translation_state
        .engines
        .into_iter()
        .find(|e| e.engine_id == engine)
        .unwrap()
        .status
}
fn current_task(h: &StoreHarness, unit: &str, engine: &str) -> TaskView {
    match status(h, unit, engine) {
        TranslationEngineStatus::Current { task } => task,
        other => panic!("expected current {engine}, got {other:?}"),
    }
}
fn assert_unordered(h: &StoreHarness, unit: &str, heads: u32, failed: u32, completed: u32) {
    match status(h, unit, "engine-a") {
        TranslationEngineStatus::Unordered {
            summary,
            active_task,
        } => {
            assert_eq!(summary.head_count, heads.to_string());
            assert_eq!(summary.failed_count, failed.to_string());
            assert_eq!(summary.completed_count, completed.to_string());
            assert_eq!(summary.queued_count, "0");
            assert_eq!(summary.running_count, "0");
            assert_eq!(summary.cancelled_count, "0");
            assert_eq!(summary.invalidated_count, "0");
            assert!(active_task.is_none());
        }
        other => panic!("legacy heads must remain unordered: {other:?}"),
    }
}
fn lineage(h: &StoreHarness, task: &TaskDispatch) -> Value {
    let snapshot: String = h
        .read()
        .query_row(
            "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
            [&task.task_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str::<Value>(&snapshot).unwrap()["translationStatusLineage"].clone()
}
fn assert_lineage(h: &StoreHarness, task: &TaskDispatch, predecessors: &[&str]) {
    let value = lineage(h, task);
    assert_eq!(value["schemaVersion"], json!(1));
    let mut actual = value["supersedesTaskIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let mut expected = predecessors
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
fn assert_failed_facts(h: &StoreHarness, task: &TaskDispatch, code: &str) {
    let actual: (String, String, String, String) = h.read().query_row(
        "SELECT t.state,t.invalidation_code,a.state,a.error_code FROM request_tasks t JOIN request_attempts a USING(task_id) WHERE t.task_id=?1 AND a.attempt_id=?2",
        params![task.task_id,task.attempt_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).unwrap();
    assert_eq!(
        actual,
        ("failed".into(), code.into(), "failed".into(), code.into())
    );
}

// Compatibility/fault fixture for a prior binary's admission format. This never
// edits an admitted task or fabricates a terminal status. The genuine schema
// enforces queued -> running and the production repository ends the attempt.
// Only this test's isolated database is writable here.
fn admit_old_writer_task(
    h: &StoreHarness,
    template: &TaskDispatch,
    name: &str,
    now: i64,
    lineage_value: Option<Value>,
) -> TaskDispatch {
    let raw = lineage_value.as_ref().map(Value::to_string);
    admit_old_writer_task_raw(h, template, name, now, raw.as_deref())
}
fn admit_old_writer_task_raw(
    h: &StoreHarness,
    template: &TaskDispatch,
    name: &str,
    now: i64,
    lineage_raw: Option<&str>,
) -> TaskDispatch {
    let mut task = template.clone();
    task.task_id = format!("compat-task-{name}");
    task.attempt_id = format!("compat-attempt-{name}");
    let mut snapshot: Value = serde_json::from_str(
        &h.read()
            .query_row::<String, _, _>(
                "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
                [&template.task_id],
                |r| r.get(0),
            )
            .unwrap(),
    )
    .unwrap();
    snapshot
        .as_object_mut()
        .unwrap()
        .remove("translationStatusLineage");
    snapshot["operationId"] = json!(format!("compat-operation-{name}"));
    let mut snapshot_json = snapshot.to_string();
    if let Some(raw) = lineage_raw {
        snapshot_json.pop();
        snapshot_json.push_str(",\"translationStatusLineage\":");
        snapshot_json.push_str(raw);
        snapshot_json.push('}');
    }
    let mut connection = SqliteConnection::open(&h.path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    let transaction = connection.transaction().unwrap();
    transaction.execute(
        "INSERT INTO request_tasks(task_id,unit_id,text_revision,session_id,scope_key,purpose,config_revision_id,binding_id,engine_id,profile_generation,server_instance_id,authorization_epoch,request_semantics_sha256,input_snapshot_json,state,invalidation_code,created_at_ms,finished_at_ms) SELECT ?1,unit_id,text_revision,session_id,scope_key,purpose,config_revision_id,binding_id,engine_id,profile_generation,server_instance_id,authorization_epoch,request_semantics_sha256,?2,'queued',NULL,?3,NULL FROM request_tasks WHERE task_id=?4",
        params![task.task_id, snapshot_json, now, template.task_id],
    ).unwrap();
    transaction.execute(
        "INSERT INTO request_attempts(attempt_id,task_id,attempt_number,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,dispatch_kind,state,error_code,finish_reason,usage_json,started_at_ms,finished_at_ms) SELECT ?1,?2,1,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,'initial','started',NULL,NULL,'{}',?3,NULL FROM request_attempts WHERE attempt_id=?4",
        params![task.attempt_id, task.task_id, now, template.attempt_id],
    ).unwrap();
    transaction
        .execute(
            "UPDATE request_tasks SET state='running' WHERE task_id=?1",
            [&task.task_id],
        )
        .unwrap();
    transaction.commit().unwrap();
    task
}

#[test]
fn failed_then_explicit_success_projects_recovery_and_preserves_attempts_and_operation_receipt() {
    let h = StoreHarness::new(false);
    let unit = h.unit("風が吹く。", 200);
    let old = h.prepare(&unit, "failed-first", 300).remove(0);
    h.fail(&old, "connection_failed", 301);
    assert_current(&h, &h.view(&old.task_id), true);
    let new = h.prepare(&unit, "successful-retranslation", 400).remove(0);
    assert_ne!(old.task_id, new.task_id);
    assert_ne!(old.attempt_id, new.attempt_id);
    assert_lineage(&h, &old, &[]);
    assert_lineage(&h, &new, &[&old.task_id]);
    assert_current(&h, &h.view(&old.task_id), false);
    assert_current(&h, &h.view(&new.task_id), true);
    assert_eq!(h.view(&new.task_id).status, "running");
    h.complete(&new, "风吹过。", 401);
    assert_current(&h, &h.view(&new.task_id), true);
    assert_eq!(h.view(&new.task_id).status, "completed");
    assert_eq!(h.view(&new.task_id).error_code, None);
    assert_failed_facts(&h, &old, "connection_failed");
    let stored = h.store.unit(unit.clone(), 1).unwrap();
    assert_eq!(stored.translations.len(), 1);
    assert_eq!(stored.translations[0].task_id, new.task_id);
    assert_eq!(stored.translations[0].text, "风吹过。");
    let replay = h
        .store
        .prepare_uncached_translation_for_operation(unit, 1, "failed-first".into(), 500)
        .unwrap();
    assert!(replay.dispatches.is_empty());
    assert_eq!(replay.task_ids, vec![old.task_id.clone()]);
    assert_eq!(h.view(&replay.task_ids[0]).status, "failed");
    assert_failed_facts(&h, &old, "connection_failed");
    let counts: (i64, i64) = h
        .read()
        .query_row(
            "SELECT (SELECT count(*) FROM request_tasks),(SELECT count(*) FROM request_attempts)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        counts,
        (2, 2),
        "Replaying an operation must not mint a task or attempt"
    );
}

#[test]
fn same_provider_label_does_not_clear_the_other_engine_failure() {
    let h = StoreHarness::new(true);
    let unit = h.unit("雨が降る。", 200);
    let first = h.prepare(&unit, "first-two-engines", 300);
    assert_eq!(first.len(), 2);
    for t in &first {
        h.fail(t, "connection_failed", 301);
    }
    let second = h.prepare(&unit, "second-two-engines", 400);
    let a = second
        .iter()
        .find(|t| t.engine.engine_id == "engine-a")
        .unwrap();
    let b = second
        .iter()
        .find(|t| t.engine.engine_id == "engine-b")
        .unwrap();
    h.complete(a, "雨落下。", 401);
    h.fail(b, "service_unavailable", 402);
    assert_eq!(
        h.view(&a.task_id).provider_label,
        h.view(&b.task_id).provider_label
    );
    assert_current(&h, &h.view(&a.task_id), true);
    assert_current(&h, &h.view(&b.task_id), true);
    assert_eq!(h.view(&b.task_id).status, "failed");
    assert_eq!(
        h.view(&b.task_id).error_code.as_deref(),
        Some("service_unavailable")
    );
    for old in &first {
        let replacement = second
            .iter()
            .find(|t| t.engine.engine_id == old.engine.engine_id)
            .unwrap();
        assert_lineage(&h, replacement, &[&old.task_id]);
        assert_current(&h, &h.view(&old.task_id), false);
        assert_failed_facts(&h, old, "connection_failed");
    }
}

#[test]
fn new_failed_or_cancelled_request_replaces_old_failure_without_claiming_success() {
    let h = StoreHarness::new(false);
    let unit = h.unit("まだ終わらない。", 200);
    let old = h.prepare(&unit, "first-failure", 300).remove(0);
    h.fail(&old, "connection_failed", 301);
    let failed = h.prepare(&unit, "next-failure", 400).remove(0);
    h.fail(&failed, "timeout", 401);
    assert_current(&h, &h.view(&old.task_id), false);
    assert_current(&h, &h.view(&failed.task_id), true);
    assert_eq!(h.view(&failed.task_id).status, "failed");
    assert_eq!(
        h.view(&failed.task_id).error_code.as_deref(),
        Some("timeout")
    );
    let cancelled = h.prepare(&unit, "next-cancelled", 500).remove(0);
    h.command(
        Command::CancelTask {
            task_id: cancelled.task_id.clone(),
        },
        501,
    );
    assert_current(&h, &h.view(&failed.task_id), false);
    assert_current(&h, &h.view(&cancelled.task_id), true);
    assert_eq!(h.view(&cancelled.task_id).status, "cancelled");
    assert!(h.store.unit(unit, 1).unwrap().translations.is_empty());
}

#[test]
fn stable_engine_survives_configuration_revision_but_removed_member_becomes_historical() {
    let h = StoreHarness::new(true);
    let unit = h.unit("別の道を選ぶ。", 200);
    let first = h.prepare(&unit, "before-config", 300);
    for t in &first {
        h.fail(t, "connection_failed", 301);
    }
    let old_a = first
        .iter()
        .find(|t| t.engine.engine_id == "engine-a")
        .unwrap();
    let old_b = first
        .iter()
        .find(|t| t.engine.engine_id == "engine-b")
        .unwrap();
    let mut config = h
        .store
        .snapshot()
        .unwrap()
        .configuration
        .applied
        .unwrap()
        .as_legacy()
        .expect("legacy status fixture remains codec1/2")
        .clone();
    config.models[0].actual_model_id = Some("changed-route-a".into());
    config.bindings[0]
        .engines
        .retain(|e| e.engine_id == "engine-a");
    h.apply(config, 400);
    assert_current(&h, &h.view(&old_b.task_id), false);
    let replacement = h.prepare(&unit, "after-config", 500).remove(0);
    assert_eq!(replacement.engine.engine_id, old_a.engine.engine_id);
    assert_ne!(
        replacement.configuration.revision_id,
        old_a.configuration.revision_id
    );
    assert_ne!(
        replacement.request_semantics_hash,
        old_a.request_semantics_hash
    );
    h.complete(&replacement, "另一条路。", 501);
    assert_current(&h, &h.view(&old_a.task_id), false);
    assert_current(&h, &h.view(&replacement.task_id), true);
    assert_current(&h, &h.view(&old_b.task_id), false);
    assert_failed_facts(&h, old_a, "connection_failed");
    assert_failed_facts(&h, old_b, "connection_failed");
}

#[test]
fn current_membership_uses_the_fixed_units_game_scope_and_rejects_a_replaced_binding() {
    let h = StoreHarness::new(false);
    let game: GameInput = serde_json::from_value(json!({
        "gameId":null, "rowRevision":null, "displayName":"作用域测试", "originalTitle":null, "sourceLanguage":"ja", "coverAssetRef":null,
        "target":{"environmentKind":"unresolved", "environmentKey":"", "bottleLocator":null, "launchPath":null, "canonicalExePath":null, "targetPathKey":null, "workingDirectory":null, "arguments":[], "architecture":null, "verificationState":"unresolved"}
    })).unwrap();
    let game_id = h
        .command(Command::SaveGame(Box::new(game)), 150)
        .result_id
        .unwrap();
    let mut config = h
        .store
        .snapshot()
        .unwrap()
        .configuration
        .applied
        .unwrap()
        .as_legacy()
        .expect("legacy status fixture remains codec1/2")
        .clone();
    let mut scoped = config.bindings[0].clone();
    scoped.scope_key = format!("game:{game_id}");
    scoped.game_id = Some(game_id.clone());
    scoped.binding_id = "game-binding".into();
    scoped.engines[0].engine_id = "game-engine".into();
    config.bindings.push(scoped);
    h.apply(config, 160);
    let session = |id: &str, game: Option<String>| HookSessionInput {
        session_id: id.into(),
        game_id: game,
        environment_key: "crossover:/bottles/current".into(),
        target_exe_path: "C:\\game.exe".into(),
        windows_pid: 100,
        process_created_u64: "100".into(),
        architecture: "x86".into(),
        broker_instance_id: format!("broker-{id}"),
        resource: ResourceSnapshotInput {
            snapshot_id: "current-core-resource".into(),
            kind: "core".into(),
            version: "controlled-v1".into(),
            manifest_sha256: content_hash("current-core"),
            contract_version: "1".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: content_hash(id),
    };
    h.store
        .open_hook_session(session("fixed-game-session", Some(game_id.clone())), 200)
        .unwrap();
    let input = |n: u32, text: &str| HookTextInput {
        session_id: "fixed-game-session".into(),
        event_id: format!("fixed-game-event-{n}"),
        sequence_u64: n.to_string(),
        source: HookSourceInput {
            source_id: "fixed-game-source".into(),
            host_instance_id: "fixed-game-host".into(),
            hook_address_u64: "123".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "HW".into(),
            hook_name: "TextOutW".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash("fixed-game-source"),
            identity_extensions_json: "{}".into(),
        },
        text: text.into(),
        source_language: "ja".into(),
    };
    h.store
        .ingest_hook_text(input(1, "候補を選ぶ。"), 210)
        .unwrap();
    h.command(
        Command::SelectSource {
            source_id: "fixed-game-source".into(),
        },
        220,
    );
    let unit = h
        .store
        .ingest_hook_text(input(2, "固定したゲームの句。"), 230)
        .unwrap()
        .visible_version
        .unwrap()
        .unit_id;
    let old = h.prepare(&unit, "game-scoped-failure", 300).remove(0);
    assert_eq!(old.binding.scope_key, format!("game:{game_id}"));
    assert_eq!(old.binding.binding_id, "game-binding");
    assert_eq!(old.engine.engine_id, "game-engine");
    h.fail(&old, "connection_failed", 301);
    h.store
        .open_hook_session(session("different-active-session", None), 400)
        .unwrap();
    assert_eq!(
        h.store.snapshot().unwrap().session.unwrap().session_id,
        "different-active-session"
    );
    assert_current(&h, &h.view(&old.task_id), true);
    let mut config = h
        .store
        .snapshot()
        .unwrap()
        .configuration
        .applied
        .unwrap()
        .as_legacy()
        .expect("legacy status fixture remains codec1/2")
        .clone();
    config
        .bindings
        .iter_mut()
        .find(|b| b.scope_key == format!("game:{game_id}"))
        .unwrap()
        .binding_id = "replacement-game-binding".into();
    h.apply(config, 500);
    assert_current(&h, &h.view(&old.task_id), false);
    assert_failed_facts(&h, &old, "connection_failed");
    let new = h.prepare(&unit, "new-game-binding-request", 600).remove(0);
    assert_eq!(new.binding.binding_id, "replacement-game-binding");
    assert_lineage(&h, &new, &[]);
    assert_current(&h, &h.view(&new.task_id), true);
    assert_current(&h, &h.view(&old.task_id), false);
    h.complete(&new, "当前游戏作用域结果。", 601);
}

#[test]
fn full_database_successor_is_found_when_the_newer_task_is_outside_the_terminal_window() {
    for terminal in ["failed", "cancelled", "completed"] {
        let h = StoreHarness::new(false);
        let unit = h.unit("古い句の状態を保つ。", 200);
        let old = h.prepare(&unit, "visible-old-failure", 10000).remove(0);
        h.fail(&old, "connection_failed", 10001);
        // Reversed clock: the causal successor really falls outside tasks100.
        let new = h.prepare(&unit, "hidden-new-request", 1000).remove(0);
        match terminal {
            "failed" => h.fail(&new, "timeout", 1001),
            "cancelled" => {
                h.command(
                    Command::CancelTask {
                        task_id: new.task_id.clone(),
                    },
                    1001,
                );
            }
            "completed" => h.complete(&new, "新的结果。", 1001),
            _ => unreachable!(),
        }
        assert_lineage(&h, &new, &[&old.task_id]);
        for n in 0..101 {
            let filler_unit = h.unit(&format!("独立した句{n}。"), 8000 + n);
            let filler = h
                .prepare(&filler_unit, &format!("unrelated-filler-{n}"), 9000 + n)
                .remove(0);
            h.fail(&filler, "timeout", 9200 + n);
        }
        let tasks = h.store.snapshot().unwrap().tasks;
        assert_eq!(tasks.len(), 100);
        let visible_old = tasks
            .iter()
            .find(|t| t.task_id == old.task_id)
            .expect("stale failure must remain in bounded view");
        assert!(
            !tasks.iter().any(|t| t.task_id == new.task_id),
            "real successor must be outside bounded view"
        );
        assert_current(&h, visible_old, false);
        let head = current_task(&h, &unit, "engine-a");
        assert_eq!(head.task_id, new.task_id);
        assert_eq!(head.attempt_id.as_deref(), Some(new.attempt_id.as_str()));
        assert_eq!(head.status, terminal);
        assert_eq!(
            head.error_code.as_deref(),
            match terminal {
                "failed" => Some("timeout"),
                "cancelled" => Some("user_cancelled"),
                _ => None,
            }
        );
        assert_failed_facts(&h, &old, "connection_failed");
        let stored = h.store.unit(unit, 1).unwrap();
        if terminal == "completed" {
            assert_eq!(stored.translations[0].task_id, new.task_id);
        } else {
            assert!(stored.translations.is_empty());
        }
    }
}

#[test]
fn lineage_is_independent_of_equal_or_reversed_clocks_after_backup_and_reopen() {
    let h = StoreHarness::new(false);
    let unit = h.unit("同時刻の依頼。", 200);
    let first = h.prepare(&unit, "same-clock-first", 300).remove(0);
    h.fail(&first, "connection_failed", 300);
    let second = h.prepare(&unit, "same-clock-second", 300).remove(0);
    h.fail(&second, "timeout", 300);
    let third = h.prepare(&unit, "clock-went-backwards", 250).remove(0);
    h.complete(&third, "顺序明确。", 250);
    assert_lineage(&h, &first, &[]);
    assert_lineage(&h, &second, &[&first.task_id]);
    assert_lineage(&h, &third, &[&second.task_id]);
    assert_current(&h, &h.view(&first.task_id), false);
    assert_current(&h, &h.view(&second.task_id), false);
    assert_current(&h, &h.view(&third.task_id), true);
    let backup = h.path.with_extension("backup.sqlite3");
    h.store.backup(&backup).unwrap();
    {
        // A backup's physical ROWIDs are not a causal contract. Deliberately
        // reverse them without changing any task/attempt/metadata fact, then
        // exercise SQLite's actual repack path before reopening the store.
        let connection = SqliteConnection::open(&backup).unwrap();
        let facts = |connection: &SqliteConnection| {
            connection
                .prepare("SELECT task_id,input_snapshot_json,state,invalidation_code FROM request_tasks ORDER BY task_id")
                .unwrap()
                .query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?)))
                .unwrap()
                .collect::<Result<Vec<_>,_>>()
                .unwrap()
        };
        let before = facts(&connection);
        let rowids: Vec<i64> = connection
            .prepare("SELECT rowid FROM request_tasks ORDER BY task_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        connection
            .execute("UPDATE request_tasks SET rowid=-rowid", [])
            .unwrap();
        let reversed: Vec<i64> = connection
            .prepare("SELECT rowid FROM request_tasks ORDER BY task_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(reversed, rowids.iter().map(|n| -n).collect::<Vec<_>>());
        assert_eq!(before, facts(&connection));
        connection.execute_batch("VACUUM").unwrap();
        assert_eq!(before, facts(&connection));
    }
    let copied = SqliteStore::open(&backup, "current-profile", "restored-server", 600).unwrap();
    let state = copied.snapshot().unwrap();
    for old in [&first, &second] {
        let v = state
            .tasks
            .iter()
            .find(|t| t.task_id == old.task_id)
            .unwrap();
        assert_current_in(&copied, v, false);
        assert_eq!(v.status, "failed");
    }
    assert_current_in(
        &copied,
        state
            .tasks
            .iter()
            .find(|t| t.task_id == third.task_id)
            .unwrap(),
        true,
    );
    assert_eq!(
        copied.unit(unit, 1).unwrap().translations[0].text,
        "顺序明确。"
    );
}

#[test]
fn legacy_and_later_old_writer_heads_are_conservatively_merged_by_the_next_real_request() {
    let h = StoreHarness::new(false);
    let unit = h.unit("互換の依頼を残す。", 200);
    let initial = h.prepare(&unit, "lineage-root", 300).remove(0);
    h.fail(&initial, "connection_failed", 301);
    let legacy_one = admit_old_writer_task(&h, &initial, "legacy-one", 5000, None);
    h.fail(&legacy_one, "timeout", 5001);
    let legacy_two = admit_old_writer_task(&h, &initial, "legacy-two", 250, None);
    h.fail(&legacy_two, "service_unavailable", 251);
    assert_unordered(&h, &unit, 3, 3, 0);
    assert_eq!(lineage(&h, &legacy_one), Value::Null);
    assert_eq!(lineage(&h, &legacy_two), Value::Null);
    let joined = h.prepare(&unit, "join-all-existing-heads", 400).remove(0);
    assert_lineage(
        &h,
        &joined,
        &[&initial.task_id, &legacy_one.task_id, &legacy_two.task_id],
    );
    h.complete(&joined, "明确重译后的结果。", 401);
    for task in [&initial, &legacy_one, &legacy_two] {
        assert_current(&h, &h.view(&task.task_id), false);
    }
    assert_current(&h, &h.view(&joined.task_id), true);
    // An old-format writer added afterwards has no causal edge. Its request
    // must remain an independent head regardless of its wall-clock timestamp.
    let later_legacy = admit_old_writer_task(&h, &joined, "later-old-binary", 225, None);
    h.fail(&later_legacy, "connection_failed", 226);
    assert_unordered(&h, &unit, 2, 1, 1);
    let rejoined = h.prepare(&unit, "join-after-old-binary", 220).remove(0);
    assert_lineage(&h, &rejoined, &[&joined.task_id, &later_legacy.task_id]);
    h.complete(&rejoined, "再次明确恢复。", 221);
    assert_current(&h, &h.view(&joined.task_id), false);
    assert_current(&h, &h.view(&later_legacy.task_id), false);
    assert_current(&h, &h.view(&rejoined.task_id), true);
    assert_failed_facts(&h, &initial, "connection_failed");
    assert_failed_facts(&h, &legacy_one, "timeout");
    assert_failed_facts(&h, &legacy_two, "service_unavailable");
    assert_failed_facts(&h, &later_legacy, "connection_failed");
}

fn task_attempt_counts(h: &StoreHarness) -> (i64, i64) {
    h.read()
        .query_row(
            "SELECT (SELECT count(*) FROM request_tasks),(SELECT count(*) FROM request_attempts)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}
fn assert_isolated_fault(h: &StoreHarness, unit: &str, bad: &TaskDispatch, operation: &str) {
    // A bad lineage is local: ordinary task history remains fully readable.
    let snapshot = h.store.snapshot().unwrap();
    assert!(
        snapshot
            .tasks
            .iter()
            .any(|t| t.task_id == bad.task_id && t.status == "failed")
    );
    match status(h, unit, "engine-a") {
        TranslationEngineStatus::Unavailable { error } => {
            assert_eq!(error.code, "invalid_translation_status_lineage")
        }
        other => panic!("malformed lineage must be visible as unavailable: {other:?}"),
    }
    assert_eq!(current_task(h, unit, "engine-b").status, "failed");
    // Admission keeps its full-group transaction even though reads isolate lanes.
    let before = task_attempt_counts(h);
    let error = h
        .store
        .prepare_uncached_translation_for_operation(unit.into(), 1, operation.into(), 500)
        .err()
        .expect("invalid lineage admission must fail");
    assert_eq!(error.code, "invalid_translation_status_lineage");
    assert_eq!(
        before,
        task_attempt_counts(h),
        "bad multi-engine admission cannot partially enqueue the healthy engine"
    );
    let other_unit = h.unit(&format!("健康な別の原文 {operation}"), 600);
    let healthy = h.prepare(&other_unit, &format!("healthy-{operation}"), 601);
    assert_eq!(healthy.len(), 2);
    for task in healthy {
        assert_eq!(
            current_task(h, &other_unit, &task.engine.engine_id).task_id,
            task.task_id
        );
        h.complete(&task, "健康な結果", 602);
    }
}

#[test]
fn invalid_lineage_versions_duplicate_or_missing_references_and_cycles_fail_closed() {
    for (name, raw) in [
        ("null-metadata", "null"),
        (
            "unknown-version",
            r#"{"schemaVersion":2,"supersedesTaskIds":[]}"#,
        ),
        (
            "self-reference",
            r#"{"schemaVersion":1,"supersedesTaskIds":["compat-task-self-reference"]}"#,
        ),
        (
            "duplicate-reference",
            r#"{"schemaVersion":1,"supersedesTaskIds":["ROOT","ROOT"]}"#,
        ),
        (
            "missing-reference",
            r#"{"schemaVersion":1,"supersedesTaskIds":["nonexistent-task"]}"#,
        ),
        (
            "wrong-type",
            r#"{"schemaVersion":1,"supersedesTaskIds":"ROOT"}"#,
        ),
        (
            "duplicate-version-key",
            r#"{"schemaVersion":1,"schemaVersion":1,"supersedesTaskIds":[]}"#,
        ),
        (
            "duplicate-list-key",
            r#"{"schemaVersion":1,"supersedesTaskIds":[],"supersedesTaskIds":[]}"#,
        ),
    ] {
        let h = StoreHarness::new(true);
        let unit = h.unit("壊れた系譜を拒む。", 200);
        let roots = h.prepare(&unit, "valid-root", 300);
        for root in &roots {
            h.fail(root, "connection_failed", 301);
        }
        let root = roots
            .iter()
            .find(|t| t.engine.engine_id == "engine-a")
            .unwrap();
        let malformed = admit_old_writer_task_raw(
            &h,
            root,
            name,
            400,
            Some(&raw.replace("ROOT", &root.task_id)),
        );
        h.fail(&malformed, "timeout", 401);
        assert_isolated_fault(&h, &unit, &malformed, &format!("request-after-{name}"));
        assert_failed_facts(&h, root, "connection_failed");
        assert_failed_facts(&h, &malformed, "timeout");
    }
    let h = StoreHarness::new(true);
    let unit = h.unit("循環を拒む。", 200);
    let roots = h.prepare(&unit, "cycle-root", 300);
    for root in &roots {
        h.fail(root, "connection_failed", 301);
    }
    let root = roots
        .iter()
        .find(|t| t.engine.engine_id == "engine-a")
        .unwrap();
    let first = admit_old_writer_task(
        &h,
        root,
        "cycle-one",
        400,
        Some(json!({"schemaVersion":1,"supersedesTaskIds":["compat-task-cycle-two"]})),
    );
    let second = admit_old_writer_task(
        &h,
        root,
        "cycle-two",
        400,
        Some(json!({"schemaVersion":1,"supersedesTaskIds":[first.task_id]})),
    );
    h.fail(&first, "timeout", 401);
    h.fail(&second, "timeout", 401);
    assert_isolated_fault(&h, &unit, &first, "request-after-cycle");
    assert_failed_facts(&h, &second, "timeout");
}

#[test]
fn lineage_cannot_reference_another_engine_or_reading_object() {
    for different_object in [false, true] {
        let h = StoreHarness::new(true);
        let unit = h.unit("参照元を固定する。", 200);
        let tasks = h.prepare(&unit, "two-source-roots", 300);
        for task in &tasks {
            h.fail(task, "connection_failed", 301);
        }
        let root_a = tasks
            .iter()
            .find(|t| t.engine.engine_id == "engine-a")
            .unwrap();
        let foreign = if different_object {
            let other_unit = h.unit("別の原文。", 350);
            let other = h.prepare(&other_unit, "other-reading-object", 360);
            for t in &other {
                h.fail(t, "timeout", 361);
            }
            let foreign = other
                .into_iter()
                .find(|t| t.engine.engine_id == "engine-a")
                .unwrap();
            assert_eq!(
                current_task(&h, &other_unit, "engine-a").task_id,
                foreign.task_id
            );
            foreign
        } else {
            tasks
                .iter()
                .find(|t| t.engine.engine_id == "engine-b")
                .unwrap()
                .clone()
        };
        let malformed = admit_old_writer_task(
            &h,
            root_a,
            "foreign-lane",
            400,
            Some(json!({"schemaVersion":1,"supersedesTaskIds":[foreign.task_id]})),
        );
        h.fail(&malformed, "timeout", 401);
        assert_isolated_fault(&h, &unit, &malformed, "request-after-foreign-lane");
        assert_failed_facts(&h, root_a, "connection_failed");
    }
}

struct NoHook;
impl HookBackend for NoHook {
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
struct NoLanguage;
impl LanguageServices for NoLanguage {
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
struct NoVault;
impl CredentialVault for NoVault {
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
    task: TaskDispatch,
    delta: Delta,
    reply: oneshot::Sender<AppResult<String>>,
}
struct ControlledGateway {
    calls: async_mpsc::UnboundedSender<Call>,
}
impl ModelGateway for ControlledGateway {
    fn execute<'a>(
        &'a self,
        task: &'a TaskDispatch,
        _: &'a RequestPlan,
        delta: Delta,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            self.calls
                .send(Call {
                    task: task.clone(),
                    delta,
                    reply,
                })
                .unwrap_or_else(|_| panic!("controlled call receiver closed"));
            result.await.map_err(|_| {
                AppError::new(
                    "controlled_reply_dropped",
                    "test",
                    "controlled reply dropped",
                )
            })?
        })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a Connection,
        _: &'a RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        unreachable!()
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        unreachable!()
    }
}
async fn wait_for(app: &Arc<ApplicationRuntime>, predicate: impl Fn(&AppSnapshot) -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if predicate(&app.snapshot().unwrap()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("controlled runtime state did not settle");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalidated_old_provider_callback_cannot_recreate_current_partial_or_translation() {
    let h = StoreHarness::new(false);
    let (sent, mut received) = async_mpsc::unbounded_channel();
    let app = ApplicationRuntime::new(
        h.store.clone(),
        Arc::new(SystemIdentity),
        Arc::new(NoHook),
        Arc::new(ControlledGateway { calls: sent }),
        Arc::new(NoLanguage),
        Arc::new(NoVault),
    )
    .unwrap();
    let manual = app
        .dispatch(CommandEnvelope {
            operation_id: "runtime-manual".into(),
            expected_revision: None,
            command: Command::ManualInput {
                text: "古い応答を待つ。".into(),
                source_language: "ja".into(),
            },
        })
        .await
        .unwrap();
    let unit = manual.result_id.unwrap();
    let old = tokio::time::timeout(Duration::from_secs(5), received.recv())
        .await
        .unwrap()
        .unwrap();
    (old.delta)("取消前的片段".into());
    wait_for(&app, |s| {
        s.tasks
            .iter()
            .any(|t| t.task_id == old.task.task_id && t.partial_text.is_some())
    })
    .await;
    let before = app.snapshot().unwrap().configuration;
    let mut changed = before
        .draft
        .as_legacy()
        .expect("legacy status fixture remains codec1/2")
        .clone();
    changed.models[0].actual_model_id = Some("new-authorized-route".into());
    let saved = app
        .dispatch(CommandEnvelope {
            operation_id: "runtime-save-new-route".into(),
            expected_revision: None,
            command: Command::SaveConfiguration {
                configuration: changed,
                expected_head_revision: before.draft_head_revision,
            },
        })
        .await
        .unwrap();
    app.dispatch(CommandEnvelope {
        operation_id: "runtime-apply-new-route".into(),
        expected_revision: None,
        command: Command::ApplyConfiguration {
            revision_id: saved.result_id.unwrap(),
            expected_head_revision: before.applied_head_revision,
        },
    })
    .await
    .unwrap();
    wait_for(&app, |s| {
        s.tasks.iter().any(|t| {
            t.task_id == old.task.task_id && t.status == "invalidated" && t.partial_text.is_none()
        })
    })
    .await;
    let prepared = app
        .retranslate(unit.clone(), 1, "runtime-new-request".into())
        .unwrap();
    assert_eq!(prepared.len(), 1);
    let new = tokio::time::timeout(Duration::from_secs(5), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        new.task.configuration.revision_id,
        old.task.configuration.revision_id
    );
    assert_ne!(
        new.task.request_semantics_hash,
        old.task.request_semantics_hash
    );
    wait_for(&app, |_| old.reply.is_closed()).await;
    (old.delta)("绝不能复活的旧片段".into());
    assert!(
        old.reply.send(Ok("绝不能入历史的旧结果".into())).is_err(),
        "Actual runtime cancellation must have dropped the old provider future"
    );
    assert!(
        h.store
            .complete_task(completion(&old.task, "未经授权的迟到完成"), 900)
            .is_err()
    );
    new.reply.send(Ok("新授权结果。".into())).unwrap();
    wait_for(&app, |s| {
        s.tasks
            .iter()
            .any(|t| t.task_id == new.task.task_id && t.status == "completed")
    })
    .await;
    let after = app.snapshot().unwrap();
    let old_view = after
        .tasks
        .iter()
        .find(|t| t.task_id == old.task.task_id)
        .unwrap();
    assert_current(&h, old_view, false);
    assert_eq!(old_view.status, "invalidated");
    assert_eq!(old_view.partial_text, None);
    assert_current(
        &h,
        after
            .tasks
            .iter()
            .find(|t| t.task_id == new.task.task_id)
            .unwrap(),
        true,
    );
    let stored = h.store.unit(unit, 1).unwrap();
    assert_eq!(stored.translations.len(), 1);
    assert_eq!(stored.translations[0].text, "新授权结果。");
    let old_attempt: (String, i64) = h.read().query_row("SELECT state,(SELECT count(*) FROM translations WHERE task_id=?1) FROM request_attempts WHERE attempt_id=?2", params![old.task.task_id,old.task.attempt_id], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(old_attempt, ("cancelled".into(), 0));
}

#[test]
fn persisted_status_writers_signal_real_commits_and_failure_noops_do_not_bump() {
    let h = StoreHarness::new(false);
    let mut config = configuration(false);
    config.bindings[0]
        .schedule
        .insert("retries".into(), json!(1));
    h.apply(config, 150);
    let unit = h.unit("永続状態の更新信号。", 200);
    let task = h.prepare(&unit, "failure-commit", 300).remove(0);
    let before = h.store.reading_unit_projection(unit.clone(), 1).unwrap();
    h.fail(&task, "connection_failed", 301);
    let failed = h.store.reading_unit_projection(unit.clone(), 1).unwrap();
    assert!(failed.commit_revision > before.commit_revision);
    assert_eq!(current_task(&h, &unit, "engine-a").status, "failed");
    h.fail(&task, "timeout", 302);
    assert_eq!(
        h.store
            .reading_unit_projection(unit.clone(), 1)
            .unwrap()
            .commit_revision,
        failed.commit_revision
    );
    assert_failed_facts(&h, &task, "connection_failed");

    let retried = h.prepare(&unit, "retry-commit", 400).remove(0);
    let before = h.store.snapshot().unwrap().commit_revision;
    let attempt = h
        .store
        .retry_attempt(
            retried.task_id.clone(),
            retried.attempt_id.clone(),
            "timeout".into(),
            401,
        )
        .unwrap();
    assert_ne!(attempt, retried.attempt_id);
    let retry_projection = h.store.reading_unit_projection(unit.clone(), 1).unwrap();
    assert!(retry_projection.commit_revision > before);
    assert_eq!(
        current_task(&h, &unit, "engine-a").attempt_id.as_deref(),
        Some(attempt.as_str())
    );
    let before = retry_projection.commit_revision;
    h.command(
        Command::CancelTask {
            task_id: retried.task_id.clone(),
        },
        402,
    );
    assert!(
        h.store
            .reading_unit_projection(unit.clone(), 1)
            .unwrap()
            .commit_revision
            > before
    );
    assert_eq!(current_task(&h, &unit, "engine-a").status, "cancelled");

    let completed = h.prepare(&unit, "completion-commit", 500).remove(0);
    let before = h.store.snapshot().unwrap().commit_revision;
    h.complete(&completed, "完成", 501);
    assert!(
        h.store
            .reading_unit_projection(unit.clone(), 1)
            .unwrap()
            .commit_revision
            > before
    );
    let invalidated = h.prepare(&unit, "invalidation-commit", 600).remove(0);
    let before = h.store.snapshot().unwrap().commit_revision;
    let mut config = h
        .store
        .snapshot()
        .unwrap()
        .configuration
        .applied
        .unwrap()
        .as_legacy()
        .expect("legacy status fixture remains codec1/2")
        .clone();
    config.models[0].actual_model_id = Some("changed-authority".into());
    h.apply(config, 601);
    let changed = h.store.reading_unit_projection(unit.clone(), 1).unwrap();
    assert!(changed.commit_revision > before);
    assert_eq!(
        current_task(&h, &unit, "engine-a").task_id,
        invalidated.task_id
    );
    assert_eq!(current_task(&h, &unit, "engine-a").status, "invalidated");
}

fn controlled_app(
    h: &StoreHarness,
) -> (Arc<ApplicationRuntime>, async_mpsc::UnboundedReceiver<Call>) {
    let (sent, received) = async_mpsc::unbounded_channel();
    let app = ApplicationRuntime::new(
        h.store.clone(),
        Arc::new(SystemIdentity),
        Arc::new(NoHook),
        Arc::new(ControlledGateway { calls: sent }),
        Arc::new(NoLanguage),
        Arc::new(NoVault),
    )
    .unwrap();
    (app, received)
}
async fn next_call(calls: &mut async_mpsc::UnboundedReceiver<Call>) -> Call {
    tokio::time::timeout(Duration::from_secs(5), calls.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn runtime_manual(app: &Arc<ApplicationRuntime>, operation: &str, text: &str) -> String {
    app.dispatch(CommandEnvelope {
        operation_id: operation.into(),
        expected_revision: None,
        command: Command::ManualInput {
            text: text.into(),
            source_language: "ja".into(),
        },
    })
    .await
    .unwrap()
    .result_id
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_retry_rejects_old_attempt_deltas_without_erasing_the_new_buffer() {
    let h = StoreHarness::new(false);
    let mut config = configuration(false);
    config.bindings[0]
        .schedule
        .insert("retries".into(), json!(1));
    h.apply(config, 150);
    let (app, mut calls) = controlled_app(&h);
    let unit = runtime_manual(&app, "retry-manual", "同じ依頼の再試行。 ").await;
    let old = next_call(&mut calls).await;
    (old.delta)("old attempt partial".into());
    wait_for(&app, |s| {
        s.tasks.iter().any(|t| {
            t.task_id == old.task.task_id
                && t.partial_text.as_deref() == Some("old attempt partial")
        })
    })
    .await;
    old.reply
        .send(Err(AppError::new(
            "timeout",
            "controlled",
            "first attempt timeout",
        )))
        .unwrap();
    let new = next_call(&mut calls).await;
    assert_eq!(new.task.task_id, old.task.task_id);
    assert_ne!(new.task.attempt_id, old.task.attempt_id);
    let head = app.reading_unit_projection(unit.clone(), 1).unwrap();
    let TranslationEngineStatus::Current { task: current } =
        &head.translation_state.engines[0].status
    else {
        panic!("retry must retain one current task");
    };
    assert_eq!(
        current.attempt_id.as_deref(),
        Some(new.task.attempt_id.as_str())
    );
    assert!(
        current.partial_text.is_none(),
        "SQLite RPC never merges memory partials"
    );
    assert!(
        app.snapshot()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task_id == new.task.task_id)
            .unwrap()
            .partial_text
            .is_none()
    );
    (new.delta)("new attempt partial".into());
    wait_for(&app, |s| {
        s.tasks.iter().any(|t| {
            t.task_id == new.task.task_id
                && t.partial_text.as_deref() == Some("new attempt partial")
        })
    })
    .await;
    for _ in 0..4 {
        (old.delta)("obsolete attempt must not replace new buffer".into());
        let snapshot = app.snapshot().unwrap();
        let task = snapshot
            .tasks
            .iter()
            .find(|t| t.task_id == new.task.task_id)
            .unwrap();
        assert_eq!(
            task.attempt_id.as_deref(),
            Some(new.task.attempt_id.as_str())
        );
        assert_eq!(task.partial_text.as_deref(), Some("new attempt partial"));
    }
    new.reply.send(Ok("新しい試行の完成。".into())).unwrap();
    wait_for(&app, |s| {
        s.tasks.iter().any(|t| {
            t.task_id == new.task.task_id && t.status == "completed" && t.partial_text.is_none()
        })
    })
    .await;
    (old.delta)("late terminal callback".into());
    (new.delta)("late terminal callback".into());
    let snapshot = app.snapshot().unwrap();
    assert!(
        snapshot
            .tasks
            .iter()
            .find(|t| t.task_id == new.task.task_id)
            .unwrap()
            .partial_text
            .is_none()
    );
    let attempts: Vec<(String, String, Option<String>)> = h.read().prepare("SELECT attempt_id,state,error_code FROM request_attempts WHERE task_id=?1 ORDER BY attempt_number").unwrap().query_map([&new.task.task_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(
        attempts,
        vec![
            (old.task.attempt_id, "failed".into(), Some("timeout".into())),
            (new.task.attempt_id, "completed".into(), None)
        ]
    );
    assert_eq!(
        h.store.unit(unit, 1).unwrap().translations[0].text,
        "新しい試行の完成。"
    );
}

fn assert_no_live_state(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(
                    ![
                        "translationState",
                        "isCurrentForReading",
                        "attemptId",
                        "partialText"
                    ]
                    .contains(&key.as_str()),
                    "live state leaked into fixed ReadingUnit: {key}"
                );
                assert_no_live_state(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_no_live_state(value);
            }
        }
        _ => {}
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_history_cursor_detail_projection_and_retranslation_keep_fixed_objects_and_hidden_receipts()
 {
    let h = StoreHarness::new(false);
    let (app, mut calls) = controlled_app(&h);
    for n in 0..56 {
        runtime_manual(
            &app,
            &format!("history-manual-{n}"),
            &format!("実際の履歴句 {n}。"),
        )
        .await;
        let call = next_call(&mut calls).await;
        let id = call.task.task_id.clone();
        call.reply
            .send(Err(AppError::new(
                "connection_failed",
                "controlled",
                "history initial failure",
            )))
            .unwrap();
        wait_for(&app, |s| {
            s.tasks
                .iter()
                .any(|t| t.task_id == id && t.status == "failed")
        })
        .await;
    }
    let first = app
        .history_page(HistoryQuery {
            filter: HistoryFilter::default(),
            cursor: None,
            limit: 50,
        })
        .unwrap();
    assert_eq!(first.units.len(), 50);
    let cursor = first
        .next_cursor
        .clone()
        .expect("real page must supply an opaque next cursor");
    let later = app
        .history_page(HistoryQuery {
            filter: HistoryFilter::default(),
            cursor: Some(cursor),
            limit: 50,
        })
        .unwrap();
    assert_eq!(later.units.len(), 6);
    let selected = &later.units[0];
    assert!(!first.units.iter().any(|u| u.unit_id == selected.unit_id));
    assert!(
        !app.snapshot()
            .unwrap()
            .history
            .units
            .iter()
            .any(|u| u.unit_id == selected.unit_id)
    );
    let raw = app
        .unit(selected.unit_id.clone(), selected.text_revision)
        .unwrap();
    assert_eq!(&raw, selected);
    let serialized_unit = serde_json::to_string(&raw).unwrap();
    assert_no_live_state(&serde_json::from_str(&serialized_unit).unwrap());
    let frozen = app
        .freeze_learning(raw.unit_id.clone(), raw.text_revision, None)
        .unwrap();
    let frozen_json = serde_json::to_string(&frozen).unwrap();
    assert_no_live_state(&serde_json::to_value(&frozen).unwrap());
    let before = app
        .reading_unit_projection(raw.unit_id.clone(), raw.text_revision)
        .unwrap();
    assert_eq!(before.unit, raw);
    let TranslationEngineStatus::Current { task: failed } =
        &before.translation_state.engines[0].status
    else {
        panic!("selected old unit must have its true failed head");
    };
    assert_eq!(failed.status, "failed");
    let receipt = app
        .retranslate(
            raw.unit_id.clone(),
            raw.text_revision,
            "selected-history-retranslation".into(),
        )
        .unwrap();
    assert_eq!(receipt.len(), 1);
    let call = next_call(&mut calls).await;
    assert_eq!(receipt[0].task_id, call.task.task_id);
    assert_eq!(
        receipt[0].attempt_id.as_deref(),
        Some(call.task.attempt_id.as_str())
    );
    assert_ne!(receipt[0].task_id, failed.task_id);
    call.reply
        .send(Err(AppError::new(
            "service_unavailable",
            "controlled",
            "selected retry fails honestly",
        )))
        .unwrap();
    wait_for(&app, |s| {
        s.tasks
            .iter()
            .any(|t| t.task_id == receipt[0].task_id && t.status == "failed")
    })
    .await;
    assert_eq!(
        current_task(&h, &raw.unit_id, "engine-a").task_id,
        receipt[0].task_id
    );
    for n in 0..101 {
        let filler = h.unit(&format!("履歴窓外の句 {n}。"), 10_000_000_000_000 + n);
        let task = h
            .prepare(
                &filler,
                &format!("history-filler-{n}"),
                10_000_000_001_000 + n,
            )
            .remove(0);
        h.fail(&task, "timeout", 10_000_000_002_000 + n);
    }
    assert_eq!(app.snapshot().unwrap().tasks.len(), 100);
    assert!(
        !app.snapshot()
            .unwrap()
            .tasks
            .iter()
            .any(|t| t.task_id == receipt[0].task_id)
    );
    let after = app
        .reading_unit_projection(raw.unit_id.clone(), raw.text_revision)
        .unwrap();
    assert!(after.commit_revision > before.commit_revision);
    let TranslationEngineStatus::Current { task: final_task } =
        &after.translation_state.engines[0].status
    else {
        panic!("hidden head still belongs to selected unit");
    };
    assert_eq!(final_task.task_id, receipt[0].task_id);
    assert_eq!(final_task.status, "failed");
    assert_eq!(
        final_task.error_code.as_deref(),
        Some("service_unavailable")
    );
    let counts = task_attempt_counts(&h);
    let replay = app
        .retranslate(
            raw.unit_id.clone(),
            raw.text_revision,
            "selected-history-retranslation".into(),
        )
        .unwrap();
    assert_eq!(replay, vec![final_task.clone()]);
    assert_eq!(counts, task_attempt_counts(&h));
    assert!(
        calls.try_recv().is_err(),
        "replay cannot dispatch another wire request"
    );
    assert_eq!(
        receipt[0].status, "running",
        "original receipt value is unchanged"
    );
    assert_eq!(
        app.unit(raw.unit_id.clone(), raw.text_revision).unwrap(),
        raw
    );
    assert_eq!(serde_json::to_string(&after.unit).unwrap(), serialized_unit);
    assert_eq!(
        serde_json::to_string(&app.snapshot().unwrap().learning_object.unwrap()).unwrap(),
        frozen_json
    );
    assert_no_live_state(&serde_json::to_value(&after.unit).unwrap());
}

#[test]
fn second_invalid_engine_rolls_back_an_already_inserted_healthy_engine() {
    let h = StoreHarness::new(true);
    let unit = h.unit("後続エンジンの拒否は全体を戻す。", 200);
    let roots = h.prepare(&unit, "ordered-two-engine-roots", 300);
    assert_eq!(
        roots
            .iter()
            .map(|task| task.engine.engine_id.as_str())
            .collect::<Vec<_>>(),
        vec!["engine-a", "engine-b"],
        "the healthy engine is admitted before the invalid engine"
    );
    for root in &roots {
        h.fail(root, "connection_failed", 301);
    }
    let bad = admit_old_writer_task_raw(&h, &roots[1], "bad-second-engine", 400, Some("null"));
    h.fail(&bad, "timeout", 401);
    let before_snapshot = serde_json::to_value(h.store.snapshot().unwrap()).unwrap();
    let before_counts = task_attempt_counts(&h);
    let before_healthy = current_task(&h, &unit, "engine-a");
    let before_lineage = lineage(&h, &roots[0]);
    assert!(matches!(
        status(&h, &unit, "engine-b"),
        TranslationEngineStatus::Unavailable { .. }
    ));

    let error = h
        .store
        .prepare_uncached_translation_for_operation(
            unit.clone(),
            1,
            "second-engine-must-rollback".into(),
            500,
        )
        .err()
        .expect("the second lane must reject the whole admission");
    assert_eq!(error.code, "invalid_translation_status_lineage");
    assert_eq!(task_attempt_counts(&h), before_counts);
    assert_eq!(
        serde_json::to_value(h.store.snapshot().unwrap()).unwrap(),
        before_snapshot,
        "the first engine task/attempt and commit revision must all roll back"
    );
    assert_eq!(
        serde_json::to_value(current_task(&h, &unit, "engine-a")).unwrap(),
        serde_json::to_value(before_healthy).unwrap()
    );
    assert_eq!(lineage(&h, &roots[0]), before_lineage);
    let replay_count: i64 = h.read().query_row(
        "SELECT count(*) FROM request_tasks WHERE json_extract(input_snapshot_json,'$.operationId')=?1",
        ["second-engine-must-rollback"], |row| row.get(0),
    ).unwrap();
    assert_eq!(
        replay_count, 0,
        "a rejected group must not create a partial operation receipt"
    );
    assert_failed_facts(&h, &roots[0], "connection_failed");
    assert_failed_facts(&h, &roots[1], "connection_failed");
    assert_failed_facts(&h, &bad, "timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn projection_rejects_zero_revision_and_preserves_raw_latest_facts_and_frozen_object() {
    let h = StoreHarness::new(false);
    let unit = h.unit("明示した原文版の翻訳失敗を保つ。", 200);
    let tasks = h.prepare(&unit, "positive-revision-root", 300);
    h.fail(&tasks[0], "connection_failed", 301);
    let (app, _calls) = controlled_app(&h);
    let frozen = app.freeze_learning(unit.clone(), 1, None).unwrap();
    let before_snapshot = serde_json::to_value(h.store.snapshot().unwrap()).unwrap();
    let before_counts = task_attempt_counts(&h);
    let raw_latest = h.store.unit(unit.clone(), 0).unwrap();
    assert_eq!(raw_latest.text_revision, 1);
    assert_eq!(raw_latest.unit_id, unit);
    let positive = h.store.reading_unit_projection(unit.clone(), 1).unwrap();
    assert_eq!(positive.unit.text_revision, 1);
    assert!(matches!(&positive.translation_state.engines[0].status,
        TranslationEngineStatus::Current { task }
            if task.task_id == tasks[0].task_id
                && task.attempt_id.as_deref() == Some(tasks[0].attempt_id.as_str())
                && task.status == "failed"
                && task.error_code.as_deref() == Some("connection_failed")));

    for result in [
        h.store.reading_unit_projection(unit.clone(), 0),
        app.reading_unit_projection(unit.clone(), 0),
    ] {
        let error = result.expect_err("zero must not return a resolved unit with an empty lane");
        assert_eq!(error.code, "invalid_reading_revision");
        assert_eq!(error.stage, "reading");
    }

    assert_eq!(task_attempt_counts(&h), before_counts);
    assert_eq!(
        serde_json::to_value(h.store.snapshot().unwrap()).unwrap(),
        before_snapshot
    );
    assert_eq!(
        serde_json::to_value(h.store.unit(unit.clone(), 0).unwrap()).unwrap(),
        serde_json::to_value(raw_latest).unwrap(),
        "the legacy latest alias must remain unchanged"
    );
    assert_eq!(
        serde_json::to_value(app.reading_unit_projection(unit, 1).unwrap()).unwrap(),
        serde_json::to_value(positive).unwrap()
    );
    assert_eq!(
        serde_json::to_value(app.snapshot().unwrap().learning_object).unwrap(),
        serde_json::to_value(Some(frozen)).unwrap(),
        "a rejected live-status read cannot mutate a frozen learning object"
    );
    assert_failed_facts(&h, &tasks[0], "connection_failed");
}
