//! Real runtime and SQLite attempts, controlled network errors (not a live-service claim).
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{TaskCompletion, TaskDispatch, capture::HookSessionInput},
    ports::{ProductRepository, runtime::*},
};
use app_infra::{platform::SystemIdentity, storage::SqliteStore};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
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
struct Gateway {
    calls: AtomicUsize,
    failures: usize,
    code: &'static str,
    attempts: Mutex<Vec<String>>,
    deltas: Mutex<Vec<Delta>>,
}
impl ModelGateway for Gateway {
    fn execute<'a>(
        &'a self,
        t: &'a TaskDispatch,
        _: &'a RequestPlan,
        delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            self.attempts.lock().unwrap().push(t.attempt_id.clone());
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            self.deltas.lock().unwrap().push(delta.clone());
            if self.code == "hold" {
                delta("尚未完成的半句".into());
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            if n < self.failures {
                Err(AppError::new(self.code, "provider", "controlled failure"))
            } else {
                Ok("雨停了。".into())
            }
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
fn setup(
    retries: u32,
    failures: usize,
    code: &'static str,
) -> (
    Arc<ApplicationRuntime>,
    Arc<SqliteStore>,
    Arc<Gateway>,
    std::path::PathBuf,
) {
    let path = std::env::temp_dir().join(format!(
        "luna-runtime-retry-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let repo =
        Arc::new(SqliteStore::open(path.clone(), "retry-profile", "retry-server", 100).unwrap());
    let configuration:Configuration=serde_json::from_value(serde_json::json!({"revisionId":"","connections":[{"connectionId":"c","displayName":"controlled","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1234/v1","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true}],"models":[{"modelId":"m","connectionId":"c","actualModelId":"test","displayName":"controlled","selectionSource":"manual","capabilities":{},"parameters":{}}],"bindings":[{"bindingId":"b","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"outputMode":"text","sourceLanguage":"ja","targetLanguage":"zh-CN","context":{},"schedule":{"retries":retries},"parameters":{},"engines":[{"engineId":"e","modelId":"m","enabled":true,"parameters":{}}]}],"templates":[],"termSets":[]})).unwrap();
    let dispatch = |id: &str, command| {
        repo.dispatch(
            CommandEnvelope {
                operation_id: id.into(),
                expected_revision: None,
                command,
            },
            101,
        )
        .unwrap()
    };
    let saved = dispatch(
        "save",
        Command::SaveConfiguration {
            configuration,
            expected_head_revision: 0,
        },
    );
    dispatch(
        "apply",
        Command::ApplyConfiguration {
            revision_id: saved.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    let gateway = Arc::new(Gateway {
        calls: AtomicUsize::new(0),
        failures,
        code,
        attempts: Mutex::new(vec![]),
        deltas: Mutex::new(vec![]),
    });
    let runtime = ApplicationRuntime::new(
        repo.clone(),
        Arc::new(SystemIdentity),
        Arc::new(Hook),
        gateway.clone(),
        Arc::new(Language),
        Arc::new(Vault),
    )
    .unwrap();
    (runtime, repo, gateway, path)
}
async fn submit(app: &Arc<ApplicationRuntime>) {
    app.dispatch(CommandEnvelope {
        operation_id: "manual".into(),
        expected_revision: None,
        command: Command::ManualInput {
            text: "雨がやんだ。".into(),
            source_language: "ja".into(),
        },
    })
    .await
    .unwrap();
}
async fn wait_terminal(repo: &SqliteStore) -> TaskView {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Some(task) = repo
                .snapshot()
                .unwrap()
                .tasks
                .into_iter()
                .find(|t| ["completed", "failed", "cancelled"].contains(&t.status.as_str()))
            {
                return task;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_retry_has_distinct_attempts_and_rejects_old_attempt_completion() {
    let (app, repo, gateway, path) = setup(1, 1, "service_unavailable");
    submit(&app).await;
    let task = wait_terminal(&repo).await;
    assert_eq!(task.status, "completed");
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 2);
    let attempts = gateway.attempts.lock().unwrap();
    assert_ne!(attempts[0], attempts[1]);
    let conn = rusqlite::Connection::open(path).unwrap();
    let rows:Vec<(String,String,String)>=conn.prepare("SELECT dispatch_kind,state,actual_engine_id FROM request_attempts ORDER BY attempt_number").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(
        rows,
        vec![
            ("initial".into(), "failed".into(), "e".into()),
            ("retry".into(), "completed".into(), "e".into())
        ]
    );
    assert!(
        repo.complete_task(
            TaskCompletion {
                task_id: task.task_id,
                attempt_id: attempts[0].clone(),
                text: "stale".into(),
                output_mode: "text".into(),
                structured: serde_json::json!({}),
                target_language: "zh-CN".into(),
                provider_label: "controlled".into()
            },
            300
        )
        .is_err()
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_or_authentication_retry_never_resends() {
    for (retries, code) in [
        (0, "service_unavailable"),
        (3, "authentication_failed"),
        (3, "rate_limited"),
    ] {
        let (app, repo, gateway, _) = setup(retries, 10, code);
        submit(&app).await;
        assert_eq!(wait_terminal(&repo).await.status, "failed");
        assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_retry_limit_is_exact_and_cancel_during_backoff_stops_wire() {
    let (app, repo, gateway, _) = setup(2, 10, "timeout");
    submit(&app).await;
    assert_eq!(wait_terminal(&repo).await.status, "failed");
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 3);
    let (app, repo, gateway, _) = setup(3, 10, "timeout");
    submit(&app).await;
    let id = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if gateway.calls.load(Ordering::SeqCst) == 1
                && !repo.snapshot().unwrap().tasks.is_empty()
            {
                break repo.snapshot().unwrap().tasks[0].task_id.clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    app.dispatch(CommandEnvelope {
        operation_id: "cancel".into(),
        expected_revision: None,
        command: Command::CancelTask { task_id: id },
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    assert_eq!(wait_terminal(&repo).await.status, "cancelled");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_and_invalidated_tasks_cannot_republish_late_partial_text() {
    for invalidate in [false, true] {
        let (app, repo, gateway, _) = setup(0, 0, "hold");
        submit(&app).await;
        let task = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(t) = app
                    .snapshot()
                    .unwrap()
                    .tasks
                    .into_iter()
                    .find(|t| t.partial_text.is_some())
                {
                    break t;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        if invalidate {
            let state = repo.snapshot().unwrap().configuration;
            let mut configuration = state
                .draft
                .as_legacy()
                .expect("actual legacy fixture")
                .clone();
            configuration.bindings[0].enabled = false;
            let saved = app
                .dispatch(CommandEnvelope {
                    operation_id: "disable-save".into(),
                    expected_revision: None,
                    command: Command::SaveConfiguration {
                        configuration,
                        expected_head_revision: state.draft_head_revision,
                    },
                })
                .await
                .unwrap();
            app.dispatch(CommandEnvelope {
                operation_id: "disable-apply".into(),
                expected_revision: None,
                command: Command::ApplyConfiguration {
                    revision_id: saved.result_id.unwrap(),
                    expected_head_revision: state.applied_head_revision,
                },
            })
            .await
            .unwrap();
        } else {
            app.dispatch(CommandEnvelope {
                operation_id: "cancel-partial".into(),
                expected_revision: None,
                command: Command::CancelTask {
                    task_id: task.task_id.clone(),
                },
            })
            .await
            .unwrap();
        }
        gateway.deltas.lock().unwrap()[0]("迟到的旧半句".into());
        let final_task = app
            .snapshot()
            .unwrap()
            .tasks
            .into_iter()
            .find(|t| t.task_id == task.task_id)
            .unwrap();
        assert_eq!(
            final_task.status,
            if invalidate {
                "invalidated"
            } else {
                "cancelled"
            }
        );
        assert_eq!(final_task.partial_text, None);
    }
}
