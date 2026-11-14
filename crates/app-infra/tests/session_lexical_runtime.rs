//! Fixed-identity offline progress using the real runtime and SQLite writer.
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{TaskDispatch, capture::*, content_hash},
    ports::{IdentitySource, ProductRepository, runtime::*},
};
use app_infra::storage::SqliteStore;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI64, AtomicU8, AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;
struct Clock(AtomicI64);
impl IdentitySource for Clock {
    fn now_ms(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", self.now_ms())
    }
}
struct Hook;
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
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        Ok(mpsc::channel().1)
    }
    fn resource_info(&self) -> ResourceInfo {
        resource(false)
    }
}
struct Gateway(AtomicUsize);
impl ModelGateway for Gateway {
    fn execute<'a>(
        &'a self,
        _: &'a TaskDispatch,
        _: &'a RequestPlan,
        _: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(AppError::new(
                "unexpected_model",
                "test",
                "offline retry must not call a model",
            ))
        })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a Connection,
        _: &'a RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(AppError::new("not_used", "test", "not used")) })
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Ok(vec![]) })
    }
}
struct Vault;
impl CredentialVault for Vault {
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
fn resource(available: bool) -> ResourceInfo {
    ResourceInfo {
        kind: "language".into(),
        version: "controlled-v1".into(),
        manifest_sha256: content_hash("controlled-language"),
        available,
        detail: "controlled port, not native resource acceptance".into(),
    }
}
struct Language {
    available: AtomicBool,
    mode: AtomicU8,
    calls: AtomicUsize,
    blocked: AtomicBool,
    release: tokio::sync::Notify,
}
impl LanguageServices for Language {
    fn analyze<'a>(&'a self, text: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if self.blocked.load(Ordering::SeqCst) {
                self.release.notified().await;
            }
            let mode = self.mode.load(Ordering::SeqCst);
            if mode == 1 {
                return Err(AppError::new(
                    "controlled_worker_failure",
                    "language",
                    "controlled error",
                ));
            }
            Ok(vec![LexicalToken {
                surface: text.into(),
                lemma: text.into(),
                reading: if mode == 3 {
                    String::new()
                } else {
                    "さくら".into()
                },
                part_of_speech: "名詞".into(),
                byte_start: if mode == 2 { 1 } else { 0 },
                byte_end: text.len() as u32,
                utf16_start: 0,
                utf16_end: text.encode_utf16().count() as u32,
                known: true,
            }])
        })
    }
    fn lookup(&self, _: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        Ok(vec![])
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![resource(self.available.load(Ordering::SeqCst))]
    }
}
fn setup(
    mode: u8,
    available: bool,
) -> (
    Arc<ApplicationRuntime>,
    Arc<SqliteStore>,
    Arc<Language>,
    Arc<Gateway>,
) {
    let store = Arc::new(
        SqliteStore::open(
            std::env::temp_dir().join(format!(
                "luna-lexical-runtime-{}.sqlite3",
                uuid::Uuid::new_v4()
            )),
            "lexical-profile",
            "lexical-server",
            1,
        )
        .unwrap(),
    );
    let language = Arc::new(Language {
        available: AtomicBool::new(available),
        mode: AtomicU8::new(mode),
        calls: AtomicUsize::new(0),
        blocked: AtomicBool::new(false),
        release: tokio::sync::Notify::new(),
    });
    let gateway = Arc::new(Gateway(AtomicUsize::new(0)));
    let app = ApplicationRuntime::new(
        store.clone(),
        Arc::new(Clock(AtomicI64::new(10))),
        Arc::new(Hook),
        gateway.clone(),
        language.clone(),
        Arc::new(Vault),
    )
    .unwrap();
    (app, store, language, gateway)
}
async fn manual(app: &Arc<ApplicationRuntime>, text: &str) -> String {
    app.dispatch(CommandEnvelope {
        operation_id: uuid::Uuid::new_v4().to_string(),
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
async fn wait(app: &ApplicationRuntime, id: &str, state: LexicalPresentationState) -> LexicalState {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(s) = app
                .snapshot()
                .unwrap()
                .lexical_states
                .into_iter()
                .find(|s| s.unit_id == id && s.state == state)
            {
                break s;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_visible_while_pending_and_success_keeps_exact_identity() {
    let (app, store, language, gateway) = setup(0, true);
    language.blocked.store(true, Ordering::SeqCst);
    let id = manual(&app, "桜").await;
    let pending = wait(&app, &id, LexicalPresentationState::Pending).await;
    assert_eq!(store.unit(id.clone(), 1).unwrap().source_text, "桜");
    assert_eq!(pending.source_hash, content_hash("桜"));
    language.blocked.store(false, Ordering::SeqCst);
    language.release.notify_waiters();
    let ready = wait(&app, &id, LexicalPresentationState::Ready).await;
    assert_eq!(
        (ready.unit_id, ready.text_revision, ready.source_hash),
        (id.clone(), 1, content_hash("桜"))
    );
    assert_eq!(store.unit(id, 1).unwrap().tokens[0].reading, "さくら");
    assert_eq!(gateway.0.load(Ordering::SeqCst), 0);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_failure_retry_is_offline_and_identical_retry_is_idempotent() {
    let (app, store, language, gateway) = setup(1, true);
    let id = manual(&app, "桜").await;
    wait(&app, &id, LexicalPresentationState::WorkerFailed).await;
    assert_eq!(store.unit(id.clone(), 1).unwrap().source_text, "桜");
    language.mode.store(0, Ordering::SeqCst);
    app.retry_lexical(id.clone(), 1).await.unwrap();
    wait(&app, &id, LexicalPresentationState::Ready).await;
    let first = store.unit(id.clone(), 1).unwrap().tokens;
    app.retry_lexical(id.clone(), 1).await.unwrap();
    assert_eq!(store.unit(id, 1).unwrap().tokens, first);
    assert_eq!(gateway.0.load(Ordering::SeqCst), 0);
    assert_eq!(language.calls.load(Ordering::SeqCst), 3);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unavailable_resources_and_alignment_failures_are_distinct_and_preserve_source() {
    let (app, store, language, _) = setup(0, false);
    let id = manual(&app, "桜").await;
    wait(&app, &id, LexicalPresentationState::ResourceUnavailable).await;
    assert_eq!(language.calls.load(Ordering::SeqCst), 0);
    language.available.store(true, Ordering::SeqCst);
    language.mode.store(2, Ordering::SeqCst);
    app.retry_lexical(id.clone(), 1).await.unwrap();
    let failed = wait(&app, &id, LexicalPresentationState::AlignmentInvalid).await;
    assert_eq!(failed.error_code.as_deref(), Some("invalid_span"));
    assert!(store.unit(id.clone(), 1).unwrap().tokens.is_empty());
    assert_eq!(store.unit(id, 1).unwrap().source_text, "桜");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_reading_is_explicit_while_tokens_and_source_remain_readable() {
    let (app, store, _, gateway) = setup(3, true);
    let id = manual(&app, "桜").await;
    let status = wait(&app, &id, LexicalPresentationState::ReadingUnavailable).await;
    assert_eq!(status.error_code.as_deref(), Some("reading_unavailable"));
    let u = store.unit(id, 1).unwrap();
    assert_eq!(u.source_text, "桜");
    assert_eq!(u.tokens.len(), 1);
    assert!(u.tokens[0].reading.is_empty());
    assert_eq!(gateway.0.load(Ordering::SeqCst), 0);
}
