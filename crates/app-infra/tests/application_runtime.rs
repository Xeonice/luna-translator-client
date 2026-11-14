//! Runtime + actual SQLite writer; controlled ports exercise ordering, not live Hook/model compatibility.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{TaskCompletion, TaskDispatch, capture::*, content_hash},
    ports::{IdentitySource, ProductRepository, runtime::*},
};
use app_infra::storage::SqliteStore;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::{mpsc as async_mpsc, oneshot};

struct Clock(AtomicI64, AtomicBool, AtomicUsize, AtomicUsize);
impl IdentitySource for Clock {
    fn new_id(&self, prefix: &str) -> String {
        if self.1.load(Ordering::SeqCst) {
            format!("{prefix}-frozen-{}", self.2.fetch_add(1, Ordering::SeqCst))
        } else {
            format!("{prefix}-{}", self.0.fetch_add(1, Ordering::SeqCst))
        }
    }
    fn now_ms(&self) -> i64 {
        if self.1.load(Ordering::SeqCst) {
            if std::thread::current().name() == Some("capture-application") {
                self.3.fetch_add(1, Ordering::SeqCst);
            }
            self.0.load(Ordering::SeqCst)
        } else {
            self.0.fetch_add(1, Ordering::SeqCst)
        }
    }
}
struct FakeHook {
    sender: mpsc::Sender<HookUpdate>,
    late_on_stop: Mutex<Vec<HookUpdate>>,
    receiver: Mutex<Option<mpsc::Receiver<HookUpdate>>>,
    launches: Mutex<Vec<GameTarget>>,
    launch_failure: AtomicBool,
    associations: AtomicUsize,
    disconnects: AtomicUsize,
    unlinks: AtomicUsize,
    binding: Mutex<Option<GameWindowActivationRequest>>,
    unbind_after_preflight: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
    disconnect_failure: AtomicBool,
    natural_cleanup_gate: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
    natural_cleanup_committed: AtomicBool,
    attribution: Mutex<Option<String>>,
}
impl HookBackend for FakeHook {
    fn game_window_binding(&self) -> AppResult<GameWindowBindingProjection> {
        let binding = self.binding.lock().unwrap().clone();
        Ok(GameWindowBindingProjection {
            state: if binding.is_some() {
                GameWindowBindingState::Bound
            } else {
                GameWindowBindingState::Unbound
            },
            session_id: binding.as_ref().map(|b| b.expected_session_id.clone()),
            target_identity_sha256: binding.map(|b| b.expected_target_identity_sha256),
            window_title: Some("controlled window verification".into()),
            verified_at_ms: Some("1".into()),
            error_code: None,
            detail: "controlled verification, not a live native window".into(),
        })
    }
    fn launch_game(&self, target: &GameTarget) -> AppResult<()> {
        self.launches.lock().unwrap().push(target.clone());
        if self.launch_failure.swap(false, Ordering::SeqCst) {
            return Err(AppError::new(
                "fixture_launch_failed",
                "launch",
                "controlled launch failure",
            ));
        }
        Ok(())
    }
    fn targets(&self) -> AppResult<TargetCandidates> {
        Ok(TargetCandidates {
            candidates: vec![],
            diagnostics: vec![],
        })
    }
    fn association_candidate(&self, candidate_id: &str) -> AppResult<TargetCandidate> {
        if candidate_id != "controlled-candidate" {
            return Err(AppError::new(
                "stale_target",
                "test",
                "controlled candidate expired",
            ));
        }
        let session = hook_session();
        Ok(TargetCandidate {
            candidate_id: candidate_id.into(),
            window_title: "controlled game".into(),
            executable_path: session.target_exe_path,
            windows_pid: session.windows_pid,
            process_created_u64: session.process_created_u64,
            architecture: session.architecture,
            environment_key: session.environment_key,
            bottle_label: "runtime".into(),
            has_window: true,
            window_handle: Some("100".into()),
            macos_host_pid: None,
            macos_window_id: None,
        })
    }
    fn with_associated_session_commit(
        &self,
        input: &HookSessionInput,
        commit: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let mut current = self.attribution.lock().unwrap();
        if current.is_some() && input.game_id.is_some() && *current != input.game_id {
            return Err(AppError::new(
                "session_game_conflict",
                "test",
                "controlled immutable attribution",
            ));
        }
        commit()?;
        if current.is_none() {
            *current = input.game_id.clone();
        }
        Ok(())
    }
    fn associate(&self, request: AssociateRequest) -> AppResult<HookSessionInput> {
        let current = self.attribution.lock().unwrap().clone();
        if current.is_some() && request.game_id.is_some() && current != request.game_id {
            return Err(AppError::new(
                "session_game_conflict",
                "test",
                "controlled immutable attribution",
            ));
        }
        self.associations.fetch_add(1, Ordering::SeqCst);
        let mut session = hook_session();
        session.game_id = current.or(request.game_id);
        *self.binding.lock().unwrap() = Some(GameWindowActivationRequest {
            expected_session_id: session.session_id.clone(),
            expected_target_identity_sha256: content_hash(request.candidate_id),
        });
        Ok(session)
    }
    fn game_window_activation_request(&self) -> AppResult<GameWindowActivationRequest> {
        self.binding.lock().unwrap().clone().ok_or_else(|| {
            AppError::new("game_window_unbound", "test", "no bound controlled window")
        })
    }
    fn disconnect(&self) -> AppResult<()> {
        if self.disconnect_failure.swap(false, Ordering::SeqCst) {
            return Err(AppError::new(
                "fixture_disconnect_failed",
                "capture",
                "controlled disconnect failure",
            ));
        }
        self.disconnects.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn stop_and_drain(&self, acknowledged: mpsc::SyncSender<AppResult<()>>) -> AppResult<()> {
        self.disconnect()?;
        for update in self.late_on_stop.lock().unwrap().drain(..) {
            self.sender.send(update).map_err(|_| {
                AppError::new("fixture_consumer_closed", "test", "FIFO consumer closed")
            })?;
        }
        self.sender
            .send(HookUpdate::DrainBarrier { acknowledged })
            .map_err(|_| AppError::new("fixture_consumer_closed", "test", "FIFO consumer closed"))
    }
    fn finish_disconnected_session(&self, session: &str) -> AppResult<bool> {
        if self
            .binding
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|binding| binding.expected_session_id != session)
        {
            return Ok(false);
        }
        self.disconnect()?;
        if let Some((entered, resume)) = self.natural_cleanup_gate.lock().unwrap().take() {
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(5)).map_err(|_| {
                AppError::new(
                    "fixture_cleanup_gate_timeout",
                    "test",
                    "controlled cleanup gate was not released",
                )
            })?;
        }
        for update in self.late_on_stop.lock().unwrap().drain(..) {
            self.sender.send(update).map_err(|_| {
                AppError::new("fixture_consumer_closed", "test", "FIFO consumer closed")
            })?;
        }
        let (acknowledged, consumed) = mpsc::sync_channel(1);
        self.sender
            .send(HookUpdate::SessionDrainBarrier {
                session_id: session.into(),
                acknowledged,
            })
            .map_err(|_| {
                AppError::new("fixture_consumer_closed", "test", "FIFO consumer closed")
            })?;
        consumed
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| {
                AppError::new(
                    "fixture_barrier_deadlock",
                    "test",
                    "consumer waited for itself",
                )
            })??;
        self.natural_cleanup_committed.store(true, Ordering::SeqCst);
        Ok(true)
    }

    fn unlink_window(
        &self,
        request: GameWindowActivationRequest,
        preflight: &mut dyn FnMut() -> AppResult<Option<CommandReceipt>>,
        commit: &mut dyn FnMut() -> AppResult<CommandReceipt>,
    ) -> AppResult<CommandReceipt> {
        let mut binding = self.binding.lock().unwrap();
        if let Some(receipt) = preflight()? {
            return Ok(receipt);
        }
        if let Some((entered, release)) = self.unbind_after_preflight.lock().unwrap().take() {
            entered.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        if binding.as_ref() != Some(&request) {
            return Err(AppError::new(
                "game_window_unbind_stale",
                "test",
                "controlled window changed",
            ));
        }
        let previous = binding.take();
        match commit() {
            Ok(receipt) => {
                self.unlinks.fetch_add(1, Ordering::SeqCst);
                Ok(receipt)
            }
            Err(error) => {
                *binding = previous;
                Err(error)
            }
        }
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        self.receiver
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| AppError::new("fixture_receiver", "test", "receiver already taken"))
    }
    fn resource_info(&self) -> ResourceInfo {
        info("core", true)
    }
}
struct Call {
    stream: bool,
    purpose: String,
    source: String,
    model: Option<String>,
    reply: oneshot::Sender<String>,
    // Optional explicit fault input. Dropping it preserves the original reply path.
    failure: oneshot::Sender<AppError>,
    task: TaskDispatch,
}
struct ControlledGateway {
    calls: async_mpsc::UnboundedSender<Call>,
    count: AtomicUsize,
}
impl ModelGateway for ControlledGateway {
    fn execute<'a>(
        &'a self,
        task: &'a TaskDispatch,
        plan: &'a RequestPlan,
        on_delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            let (tx, mut rx) = oneshot::channel();
            let (failure, failed) = oneshot::channel();
            self.count.fetch_add(1, Ordering::SeqCst);
            self.calls
                .send(Call {
                    stream: plan.stream,
                    purpose: task.binding.purpose.clone(),
                    source: task.unit.source_text.clone(),
                    model: plan.model.clone(),
                    reply: tx,
                    failure,
                    task: task.clone(),
                })
                .map_err(|_| AppError::new("fixture_closed", "test", "call receiver dropped"))?;
            let text = tokio::select! {
                biased;
                result = &mut rx => result,
                error = failed => match error {
                    Ok(error) => return Err(error),
                    Err(_) => rx.await,
                },
            }
            .map_err(|_| AppError::new("fixture_cancelled", "test", "reply dropped"))?;
            on_delta(text.clone());
            Ok(text)
        })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a Connection,
        _: &'a RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async {
            Err(AppError::new(
                "not_a_live_model",
                "test",
                "not used by runtime tests",
            ))
        })
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Ok(vec![]) })
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
        vec![info("language", false)]
    }
}
struct NoVault;
struct ControlledCoverAssets {
    imports: AtomicUsize,
    reads: AtomicUsize,
}
#[tokio::test]
async fn session_cover_external_verification_rejects_changed_full_binding_before_short_commit() {
    let h = Harness::new(4);
    let before = h.store.snapshot().unwrap().commit_revision;
    let expected = GameWindowActivationRequest {
        expected_session_id: "cover-session".into(),
        expected_target_identity_sha256: content_hash("bound-window"),
    };
    *h.hook.binding.lock().unwrap() = Some(expected.clone());
    let hash = content_hash("controlled-image");
    let prepared = PreparedSessionCover {
        guard: CoverCommitGuard {
            profile_id: "runtime-test".into(),
            profile_generation: 1,
            server_instance_id: "runtime-server".into(),
            game_id: "cover-game".into(),
            row_revision: 1,
            environment_key: "controlled".into(),
            target_path_key: None,
            session: Some(CoverSessionGuard {
                session_id: expected.expected_session_id.clone(),
                process_identity_sha256: content_hash("different-process-identity"),
                binding_identity_sha256: expected.expected_target_identity_sha256.clone(),
            }),
        },
        cover: PreparedGameCover {
            asset_ref: format!("cover:v1:session_frame:{hash}"),
            origin: GameCoverOrigin::SessionFrame,
            content_hash: hash,
            width: 600,
            height: 400,
        },
    };
    h.app.verify_session_cover(&prepared).unwrap();
    h.hook
        .binding
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .expected_target_identity_sha256 = content_hash("replacement-window");
    assert_eq!(
        h.app.verify_session_cover(&prepared).unwrap_err().code,
        "game_cover_stale"
    );
    *h.hook.binding.lock().unwrap() = None;
    assert_eq!(
        h.app.verify_session_cover(&prepared).unwrap_err().code,
        "game_cover_stale"
    );
    assert_eq!(h.store.snapshot().unwrap().commit_revision, before);
}
impl app_core::ports::GameCoverAssets for ControlledCoverAssets {
    fn import_local_cover(&self, _: &Game) -> AppResult<Option<PreparedGameCover>> {
        self.imports.fetch_add(1, Ordering::SeqCst);
        let hash = content_hash("controlled-cover-bytes");
        Ok(Some(PreparedGameCover {
            asset_ref: format!("cover:v1:steam_library:{hash}"),
            origin: GameCoverOrigin::SteamLibrary,
            content_hash: hash,
            width: 600,
            height: 900,
        }))
    }
    fn import_session_frame(&self, _: &[u8]) -> AppResult<PreparedGameCover> {
        panic!("library query must not manufacture a session frame")
    }
    fn read_cover(&self, reference: &str) -> AppResult<GameCoverBytes> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(GameCoverBytes {
            prepared: PreparedGameCover {
                asset_ref: reference.into(),
                origin: GameCoverOrigin::SteamLibrary,
                content_hash: content_hash("controlled-cover-bytes"),
                width: 600,
                height: 900,
            },
            bytes: b"controlled-cover-bytes".to_vec(),
        })
    }
}
#[tokio::test]
async fn game_cover_hydrates_once_with_actual_writer_and_keeps_image_bytes_out_of_snapshots() {
    let h = Harness::new(4);
    let id = h
        .command(Command::SaveGame(Box::new(GameInput {
            game_id: None,
            row_revision: None,
            display_name: "Cover test".into(),
            original_title: None,
            source_language: Some("ja".into()),
            cover_asset_ref: None,
            target: GameTarget {
                environment_kind: "unresolved".into(),
                environment_key: "".into(),
                bottle_locator: None,
                launch_path: Some("/test/Game.exe".into()),
                canonical_exe_path: None,
                target_path_key: None,
                working_directory: None,
                arguments: vec![],
                architecture: Some("x86".into()),
                verification_state: "unresolved".into(),
            },
        })))
        .await
        .result_id
        .unwrap();
    let assets = Arc::new(ControlledCoverAssets {
        imports: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
    });
    h.app.configure_game_assets(assets.clone()).unwrap();
    let before = h.store.snapshot().unwrap();
    let first = h
        .app
        .game_cover(GameCoverRequest {
            game_id: id.clone(),
            expected_asset_ref: None,
        })
        .unwrap();
    assert_eq!(first.status, GameCoverStatus::Ready);
    assert_eq!(
        first.payload.as_deref(),
        Some(b"controlled-cover-bytes".as_slice())
    );
    let snapshot = h.app.snapshot().unwrap();
    assert_eq!(snapshot.games[0].cover_asset_ref, first.asset_ref);
    assert_eq!(snapshot.commit_revision, before.commit_revision + 1);
    let serialized = serde_json::to_string(&snapshot).unwrap();
    assert!(!serialized.contains("controlled-cover-bytes"));
    assert!(!serialized.contains("\"payload\""));
    let second = h
        .app
        .game_cover(GameCoverRequest {
            game_id: id.clone(),
            expected_asset_ref: first.asset_ref,
        })
        .unwrap();
    assert_eq!(second.status, GameCoverStatus::Ready);
    assert_eq!(assets.imports.load(Ordering::SeqCst), 1);
    assert_eq!(assets.reads.load(Ordering::SeqCst), 2);
    let changed = h
        .app
        .game_cover(GameCoverRequest {
            game_id: id,
            expected_asset_ref: Some("stale-reference".into()),
        })
        .unwrap();
    assert_eq!(changed.status, GameCoverStatus::ReferenceChanged);
    assert!(changed.payload.is_none());
    assert_eq!(assets.reads.load(Ordering::SeqCst), 2);
    assert_eq!(
        h.store.snapshot().unwrap().commit_revision,
        snapshot.commit_revision
    );
}
impl CredentialVault for NoVault {
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        Ok(())
    }
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        Err(AppError::new(
            "no_fixture_secret",
            "test",
            "fixture has no secret",
        ))
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        Ok(())
    }
}
fn info(kind: &str, available: bool) -> ResourceInfo {
    ResourceInfo {
        kind: kind.into(),
        version: "controlled-test-v1".into(),
        manifest_sha256: content_hash(kind),
        available,
        detail: "controlled test port, not production evidence".into(),
    }
}
#[tokio::test]
async fn launch_uses_current_saved_target_revision_without_association_or_storage_writes() {
    let mut h = Harness::new(4);
    let saved = h
        .command(Command::SaveGame(Box::new(GameInput {
            game_id: None,
            row_revision: None,
            display_name: "Controlled launch target".into(),
            original_title: None,
            source_language: Some("ja".into()),
            cover_asset_ref: None,
            target: GameTarget {
                environment_kind: "crossover".into(),
                environment_key: "crossover:/controlled/bottle".into(),
                bottle_locator: Some("/controlled/bottle".into()),
                launch_path: Some("C:\\Games\\Launcher.exe".into()),
                canonical_exe_path: Some("C:\\Games\\Game.exe".into()),
                target_path_key: None,
                working_directory: Some("C:\\Games".into()),
                arguments: vec!["--bottle".into(), "space argument".into(), "".into()],
                architecture: Some("x64".into()),
                verification_state: "verified".into(),
            },
        })))
        .await;
    let id = saved.result_id.unwrap();
    let before = h.store.snapshot().unwrap();
    let game = before
        .games
        .iter()
        .find(|game| game.game_id == id)
        .unwrap()
        .clone();
    assert_eq!(
        h.app.launch_game("missing-game", 1).unwrap_err().code,
        "game_missing"
    );
    assert_eq!(
        h.app
            .launch_game(&id, game.row_revision + 1)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    assert!(h.hook.launches.lock().unwrap().is_empty());
    h.app.launch_game(&id, game.row_revision).unwrap();
    assert_eq!(
        h.hook.launches.lock().unwrap().as_slice(),
        std::slice::from_ref(&game.target)
    );
    assert_eq!(h.store.snapshot().unwrap(), before);
    assert_eq!(h.hook.associations.load(Ordering::SeqCst), 0);
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 0);
    assert_eq!(h.hook.unlinks.load(Ordering::SeqCst), 0);
    assert!(h.calls.try_recv().is_err());
    h.hook.launch_failure.store(true, Ordering::SeqCst);
    assert_eq!(
        h.app.launch_game(&id, game.row_revision).unwrap_err().code,
        "fixture_launch_failed"
    );
    assert_eq!(h.store.snapshot().unwrap(), before);
    h.command(Command::ArchiveGame {
        game_id: id.clone(),
        row_revision: game.row_revision,
    })
    .await;
    assert_eq!(
        h.app.launch_game(&id, game.row_revision).unwrap_err().code,
        "game_missing"
    );
    assert_eq!(h.hook.launches.lock().unwrap().len(), 2);
}
fn hook_session() -> HookSessionInput {
    HookSessionInput {
        session_id: "controlled-hook-session".into(),
        game_id: None,
        environment_key: "crossover:/bottles/runtime".into(),
        target_exe_path: "C:\\game.exe".into(),
        windows_pid: 100,
        process_created_u64: "100".into(),
        architecture: "x86".into(),
        broker_instance_id: "controlled-broker".into(),
        resource: ResourceSnapshotInput {
            snapshot_id: "controlled-core-resource".into(),
            kind: "core".into(),
            version: "controlled-test-v1".into(),
            manifest_sha256: content_hash("core"),
            contract_version: "1".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: content_hash("controlled-target"),
    }
}
fn hook_text(sequence: u32, text: &str) -> HookUpdate {
    HookUpdate::Text(Box::new(HookTextInput {
        session_id: "controlled-hook-session".into(),
        event_id: format!("event-{sequence}"),
        sequence_u64: sequence.to_string(),
        source: HookSourceInput {
            source_id: "controlled-source".into(),
            host_instance_id: "controlled-host".into(),
            hook_address_u64: "123".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "HW".into(),
            hook_name: "TextOutW".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash("controlled-source"),
            identity_extensions_json: "{}".into(),
        },
        text: text.into(),
        source_language: "ja".into(),
    }))
}
fn config(queue: u64) -> Configuration {
    let binding = |purpose: &str| PurposeBinding {
        binding_id: format!("binding-{purpose}"),
        scope_key: "global".into(),
        game_id: None,
        purpose: purpose.into(),
        binding_mode: "override".into(),
        enabled: true,
        output_mode: Some(
            if purpose == "translation" {
                "text"
            } else {
                "plain_text"
            }
            .into(),
        ),
        source_language: Some("ja".into()),
        target_language: Some("zh-Hans".into()),
        template_id: None,
        context: BTreeMap::new(),
        schedule: BTreeMap::from([
            ("concurrency".into(), serde_json::json!(1)),
            ("queueCapacity".into(), serde_json::json!(queue)),
        ]),
        parameters: BTreeMap::new(),
        engines: vec![Engine {
            engine_id: format!("engine-{purpose}"),
            model_id: "controlled-model".into(),
            enabled: true,
            template_id: None,
            parameters: BTreeMap::new(),
        }],
    };
    Configuration {
        revision_id: String::new(),
        connections: vec![Connection {
            connection_id: "controlled-connection".into(),
            display_name: "Controlled model port".into(),
            protocol: "openai_compatible".into(),
            deployment: "local".into(),
            auth_mode: "none".into(),
            endpoint: Some("http://127.0.0.1:1/v1".into()),
            credential_pool_id: None,
            credential_pool_revision: None,
            proxy_mode: "disabled".into(),
            network_metadata: BTreeMap::new(),
            availability_intent: true,
            adapter_binding: None,
            preset_source: None,
        }],
        models: vec![Model {
            default_templates: Default::default(),
            model_id: "controlled-model".into(),
            connection_id: "controlled-connection".into(),
            actual_model_id: Some("controlled-only".into()),
            display_name: "Controlled".into(),
            selection_source: "manual".into(),
            capabilities: BTreeMap::new(),
            parameters: BTreeMap::new(),
        }],
        bindings: vec![binding("translation"), binding("grammar")],
        templates: vec![],
        term_sets: vec![],
        codec_version: None,
    }
}
struct Harness {
    path: std::path::PathBuf,
    app: Arc<ApplicationRuntime>,
    store: Arc<SqliteStore>,
    hook: Arc<FakeHook>,
    gateway: Arc<ControlledGateway>,
    events: mpsc::Sender<HookUpdate>,
    calls: async_mpsc::UnboundedReceiver<Call>,
    clock: Arc<Clock>,
}
impl Harness {
    fn new(queue: u64) -> Self {
        let path = acceptance_evidence::directory("application-runtime", &std::env::temp_dir())
            .join(format!("luna-runtime-{}.sqlite3", uuid::Uuid::new_v4()));
        let store =
            Arc::new(SqliteStore::open(&path, "runtime-test", "runtime-server", 1).unwrap());
        let saved = store
            .dispatch(
                CommandEnvelope {
                    operation_id: "fixture-save".into(),
                    expected_revision: None,
                    command: Command::SaveConfiguration {
                        configuration: config(queue),
                        expected_head_revision: 0,
                    },
                },
                2,
            )
            .unwrap();
        store
            .dispatch(
                CommandEnvelope {
                    operation_id: "fixture-apply".into(),
                    expected_revision: None,
                    command: Command::ApplyConfiguration {
                        revision_id: saved.result_id.unwrap(),
                        expected_head_revision: 0,
                    },
                },
                3,
            )
            .unwrap();
        let (events, receiver) = mpsc::channel();
        let hook = Arc::new(FakeHook {
            sender: events.clone(),
            late_on_stop: Mutex::new(Vec::new()),
            receiver: Mutex::new(Some(receiver)),
            launches: Mutex::new(vec![]),
            launch_failure: AtomicBool::new(false),
            associations: AtomicUsize::new(0),
            disconnects: AtomicUsize::new(0),
            unlinks: AtomicUsize::new(0),
            binding: Mutex::new(None),
            unbind_after_preflight: Mutex::new(None),
            disconnect_failure: AtomicBool::new(false),
            natural_cleanup_gate: Mutex::new(None),
            natural_cleanup_committed: AtomicBool::new(false),
            attribution: Mutex::new(None),
        });
        let (tx, calls) = async_mpsc::unbounded_channel();
        let gateway = Arc::new(ControlledGateway {
            calls: tx,
            count: AtomicUsize::new(0),
        });
        let clock = Arc::new(Clock(
            AtomicI64::new(100),
            AtomicBool::new(false),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
        ));
        let app = ApplicationRuntime::new(
            store.clone(),
            clock.clone(),
            hook.clone(),
            gateway.clone(),
            Arc::new(NoLanguage),
            Arc::new(NoVault),
        )
        .unwrap();
        Self {
            path,
            app,
            store,
            hook,
            gateway,
            events,
            calls,
            clock,
        }
    }
    async fn command(&self, command: Command) -> CommandReceipt {
        self.app
            .dispatch(CommandEnvelope {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: None,
                command,
            })
            .await
            .unwrap()
    }
    async fn manual(&self, text: &str) -> String {
        self.command(Command::ManualInput {
            text: text.into(),
            source_language: "ja".into(),
        })
        .await
        .result_id
        .unwrap()
    }
    async fn call(&mut self) -> Call {
        // Let logical time progress while awaiting stability; only the runtime
        // timer finalizes Hook text. No test calls a direct flush to hide bugs.
        tokio::time::timeout(Duration::from_secs(5),async {
            loop {
                tokio::select! {
                    call=self.calls.recv()=>return call.unwrap(),
                    _=tokio::time::sleep(Duration::from_millis(100))=>{ self.clock.0.fetch_add(CAPTURE_IDLE_MS+1,Ordering::SeqCst); }
                }
            }
        }).await.expect("model gateway never received the committed source")
    }

    fn associate(&self) {
        self.app
            .associate(AssociateRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                game_id: None,
                candidate_id: "controlled-candidate".into(),
                confirm_switch: false,
            })
            .unwrap();
    }
    async fn select_hook(&self) {
        self.app.start().unwrap();
        self.associate();
        self.events.send(hook_text(1, "候補")).unwrap();
        wait(&self.app, |s| {
            s.session.as_ref().is_some_and(|s| !s.sources.is_empty())
        })
        .await;
        self.command(Command::SelectSource {
            source_id: "controlled-source".into(),
        })
        .await;
    }
}
async fn assert_cancelled(mut call: Call) {
    tokio::time::timeout(Duration::from_secs(5), call.reply.closed())
        .await
        .expect("cancelled gateway future remained alive");
    assert!(
        call.reply
            .send("late result must be rejected".into())
            .is_err()
    );
}
async fn wait(app: &Arc<ApplicationRuntime>, predicate: impl Fn(&AppSnapshot) -> bool) {
    let mut changes = app.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snap = app.snapshot().unwrap();
            if predicate(&snap) {
                break;
            }
            changes.changed().await.unwrap();
        }
    })
    .await
    .expect("application projection did not reach expected observable state");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exports_current_session_component_snapshots() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.events.send(hook_text(2, "以前のゲームの台詞。")).unwrap();
    h.call()
        .await
        .reply
        .send("以前游戏的对白。".into())
        .unwrap();
    wait(&h.app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| !u.translations.is_empty())
    })
    .await;
    let previous = h.app.snapshot().unwrap().current.unwrap();

    // Open a different session through the real repository producer. The
    // runtime's unchanged publisher must pass its authoritative projection to
    // both windows without deriving current from the global history page.
    let mut next_session = hook_session();
    next_session.session_id = "controlled-next-hook-session".into();
    next_session.broker_instance_id = "controlled-next-broker".into();
    next_session.target_identity_sha256 = content_hash("controlled-next-target");
    next_session.target_exe_path = "C:\\next-game.exe".into();
    let next_session_id = next_session.session_id.clone();
    h.store
        .open_hook_session(next_session, h.clock.now_ms())
        .unwrap();
    h.app.publish().unwrap();
    let waiting = h.app.snapshot().unwrap();
    assert_eq!(
        waiting.session.as_ref().unwrap().session_id,
        next_session_id
    );
    assert_eq!(waiting.session.as_ref().unwrap().state, "opening");
    assert!(waiting.current.is_none());
    assert_eq!(waiting.history.units[0].unit_id, previous.unit_id);

    // A queued callback from the stopped producer is still retained as a raw
    // fact. It must not create a new body or become the next session's current.
    h.events
        .send(hook_text(3, "遅れて届いた以前の台詞。"))
        .unwrap();
    h.events
        .send(HookUpdate::Health {
            session_id: None,
            diagnostic: Diagnostic {
                code: "current_projection_late_callback_barrier".into(),
                stage: "test".into(),
                message: "ordered consumer marker".into(),
                recoverable: true,
            },
        })
        .unwrap();
    wait(&h.app, |s| {
        s.diagnostics
            .iter()
            .any(|d| d.code == "current_projection_late_callback_barrier")
    })
    .await;
    let late = h.app.snapshot().unwrap();
    assert!(late.current.is_none());
    assert_eq!(late.history.units.len(), waiting.history.units.len());
    assert_eq!(late.history.units[0].source_text, previous.source_text);
    let connection =
        rusqlite::Connection::open_with_flags(&h.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let retained: (String, String) = connection.query_row(
        "SELECT r.session_id,CAST(p.payload_bytes AS TEXT) FROM raw_events r JOIN raw_event_payloads p USING(event_id) WHERE r.event_id='event-3'", [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(retained.0, previous.session_id);
    assert_eq!(retained.1, "遅れて届いた以前の台詞。");
    drop(connection);

    let next_text = |sequence, text| {
        let HookUpdate::Text(mut input) = hook_text(sequence, text) else {
            unreachable!()
        };
        input.session_id = next_session_id.clone();
        input.event_id = format!("next-event-{sequence}");
        input.source.source_id = "controlled-next-source".into();
        input.source.host_instance_id = "controlled-next-host".into();
        input.source.stream_identity_sha256 = content_hash("controlled-next-source");
        HookUpdate::Text(input)
    };
    h.events.send(next_text(1, "新しい候補。")).unwrap();
    wait(&h.app, |s| {
        s.session.as_ref().is_some_and(|s| !s.sources.is_empty())
    })
    .await;
    assert!(h.app.snapshot().unwrap().current.is_none());
    h.command(Command::SelectSource {
        source_id: "controlled-next-source".into(),
    })
    .await;
    assert!(h.app.snapshot().unwrap().current.is_none());
    h.events.send(next_text(2, "今のゲームの台詞。")).unwrap();
    h.call()
        .await
        .reply
        .send("当前游戏的对白。".into())
        .unwrap();
    wait(&h.app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| u.session_id == next_session_id && !u.translations.is_empty())
    })
    .await;
    let reading = h.app.snapshot().unwrap();
    let current = reading.current.as_ref().unwrap();
    assert_ne!(current.unit_id, previous.unit_id);
    assert_eq!(current.source_text, "今のゲームの台詞。");
    assert_eq!(current.translations[0].text, "当前游戏的对白。");
    h.command(Command::Disconnect).await;
    let disconnected = h.app.snapshot().unwrap();
    assert_eq!(disconnected.session.as_ref().unwrap().state, "stopped");
    let last = disconnected.current.as_ref().unwrap();
    assert_eq!(
        (&last.unit_id, last.text_revision, &last.session_id),
        (&current.unit_id, current.text_revision, &current.session_id)
    );
    assert_eq!(last.translations, current.translations);

    // Independent object-status evidence retains the four original scenarios.
    // It always executes its real SQLite/runtime assertions, even without export.
    let mut reading_status_history = reading_status_history_fixture().await;
    // Optional evidence export contains controlled test data only. React tests
    // consume these actual Rust DTOs directly instead of copying the selector.
    if let Some(output) = std::env::var_os("LUNA_CURRENT_SESSION_FIXTURE_DIR") {
        let nonce = std::env::var("LUNA_CURRENT_SESSION_FIXTURE_NONCE")
            .expect("fixture export requires this invocation's nonce");
        assert!(!nonce.is_empty() && nonce.len() <= 128);
        assert!(
            nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        );
        let output = std::path::PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        let mut entries = vec![];
        for (scenario, snapshot) in [
            ("01-new-session-waiting", &waiting),
            ("02-old-session-late", &late),
            ("03-new-session-body", &reading),
            ("04-disconnected-last-sentence", &disconnected),
        ] {
            assert!(snapshot.credentials.is_empty());
            assert!(snapshot.credential_pools.is_empty());
            let bytes = serde_json::to_vec_pretty(snapshot).unwrap();
            let name = format!("{scenario}.json");
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join(&name))
                .unwrap();
            std::io::Write::write_all(&mut file, &bytes).unwrap();
            entries.push(serde_json::json!({"scenario":scenario,"path":name,"sha256":content_hash(&bytes),"sessionId":snapshot.session.as_ref().map(|s|&s.session_id),"current":snapshot.current.as_ref().map(|u|serde_json::json!({"unitId":u.unit_id,"textRevision":u.text_revision,"sessionId":u.session_id,"sourceText":u.source_text}))}));
        }
        reading_status_history["nonce"] = serde_json::json!(nonce);
        let history_bytes = serde_json::to_vec_pretty(&reading_status_history).unwrap();
        let history_path = "reading-status-history.json";
        let mut history_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(history_path))
            .unwrap();
        std::io::Write::write_all(&mut history_file, &history_bytes).unwrap();
        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "kind": "actual_sqlite_runtime_projection_fixtures",
            "producerTarget": "application_runtime",
            "producer": "exports_current_session_component_snapshots",
            "nonce": nonce,
            "notLiveGameEvidence": true,
            "snapshotQuerySourceSha256": content_hash(include_str!("../src/storage/queries/mod.rs")),
            "producerSourceSha256": content_hash(include_str!("application_runtime.rs")),
            "readingStatusHistory": { "path": history_path, "sha256": content_hash(&history_bytes) },
            "entries": entries,
        });
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join("manifest.json"))
            .unwrap();
        serde_json::to_writer_pretty(&mut file, &manifest).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn committed_source_is_visible_before_model_completion_and_result_stays_with_old_sentence() {
    let mut h = Harness::new(4);
    let first = h.manual("雨がやんだ。").await;
    let call = h.call().await;
    assert_eq!(call.source, "雨がやんだ。");
    assert_eq!(call.purpose, "translation");
    let snapshot = h.app.snapshot().unwrap();
    let unit = snapshot
        .history
        .units
        .iter()
        .find(|u| u.unit_id == first)
        .unwrap();
    assert_eq!(unit.source_text, "雨がやんだ。");
    assert!(unit.translations.is_empty());
    let second = h.manual("駅まで歩こう。").await;
    call.reply.send("雨停了。".into()).unwrap();
    let next = h.call().await;
    assert_eq!(next.source, "駅まで歩こう。");
    wait(&h.app, |s| {
        s.history
            .units
            .iter()
            .any(|u| u.unit_id == first && !u.translations.is_empty())
    })
    .await;
    let snap = h.app.snapshot().unwrap();
    assert_eq!(
        snap.history
            .units
            .iter()
            .find(|u| u.unit_id == first)
            .unwrap()
            .translations[0]
            .text,
        "雨停了。"
    );
    assert!(
        snap.history
            .units
            .iter()
            .find(|u| u.unit_id == second)
            .unwrap()
            .translations
            .is_empty()
    );
    next.reply.send("走到车站吧。".into()).unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_pause_does_not_pause_manual_input_or_lose_selected_hook() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.command(Command::SetCapturePaused { paused: true }).await;
    h.events.send(hook_text(2, "この行は停止中")).unwrap();
    h.events
        .send(HookUpdate::Health {
            session_id: Some("controlled-hook-session".into()),
            diagnostic: Diagnostic {
                code: "fixture_barrier".into(),
                stage: "test".into(),
                message: "ordered consumer barrier".into(),
                recoverable: true,
            },
        })
        .unwrap();
    wait(&h.app, |s| {
        s.diagnostics.iter().any(|d| d.code == "fixture_barrier")
    })
    .await;
    let uid = h.manual("手動入力は続く。").await;
    let call = h.call().await;
    assert_eq!(call.source, "手動入力は続く。");
    let snap = h.app.snapshot().unwrap();
    let session = snap.session.as_ref().unwrap();
    assert_eq!(session.session_id, "controlled-hook-session");
    assert!(session.paused);
    assert!(session.sources[0].selected);
    assert!(
        !snap
            .history
            .units
            .iter()
            .any(|u| u.source_text == "この行は停止中")
    );
    assert!(snap.history.units.iter().any(|u| u.unit_id == uid));
    call.reply.send("手动输入继续。".into()).unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn translation_off_cancels_waiting_and_running_hook_tasks_without_new_gateway_calls() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.events.send(hook_text(2, "最初の台詞")).unwrap();
    let first = h.call().await;
    h.events.send(hook_text(3, "待機する台詞")).unwrap();
    wait(&h.app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| u.source_text == "待機する台詞")
    })
    .await;
    // V2 requires the creation hard-age boundary for this incomplete Hook line
    // before asserting that its automatic request waits behind the first one.
    h.clock.0.fetch_add(CAPTURE_MAX_AGE_MS, Ordering::SeqCst);
    wait(&h.app, |s| s.tasks.len() == 2).await;
    h.command(Command::SetTranslationEnabled { enabled: false })
        .await;
    wait(&h.app, |s| s.tasks.iter().all(|t| t.status == "cancelled")).await;
    assert_cancelled(first).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    assert!(h.calls.try_recv().is_err());
    assert!(
        h.app
            .snapshot()
            .unwrap()
            .history
            .units
            .iter()
            .all(|u| u.translations.is_empty())
    );
    // Turning off this Hook session must leave manual translation usable.
    h.manual("手動は独立する。").await;
    let manual = h.call().await;
    assert_eq!(manual.source, "手動は独立する。");
    manual.reply.send("手动输入独立。".into()).unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_a_running_task_drops_the_gateway_and_refuses_late_history() {
    let mut h = Harness::new(4);
    let uid = h.manual("取り消す台詞").await;
    let call = h.call().await;
    let id = h
        .app
        .snapshot()
        .unwrap()
        .tasks
        .iter()
        .find(|t| t.unit_id == uid)
        .unwrap()
        .task_id
        .clone();
    h.command(Command::CancelTask {
        task_id: id.clone(),
    })
    .await;
    wait(&h.app, |s| {
        s.tasks
            .iter()
            .any(|t| t.task_id == id && t.status == "cancelled")
    })
    .await;
    assert_cancelled(call).await;
    assert!(h.store.unit(uid, 1).unwrap().translations.is_empty());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_translation_queue_does_not_starve_grammar_and_overflow_is_visible() {
    let mut h = Harness::new(2);
    let first = h.manual("第一の文").await;
    let blocked = h.call().await;
    h.manual("第二の文").await;
    wait(&h.app, |s| s.tasks.len() == 2).await;
    h.manual("第三の文").await;
    wait(&h.app, |s| {
        s.diagnostics.iter().any(|d| d.code == "purpose_queue_full")
    })
    .await;
    let grammar = h.app.study(first, 1, "grammar".into(), None).unwrap();
    let call = h.call().await;
    assert_eq!(call.purpose, "grammar");
    call.reply.send("这是一句测试文法解释。".into()).unwrap();
    wait(&h.app, |s| {
        s.tasks
            .iter()
            .any(|t| t.task_id == grammar.task_id && t.status == "completed")
    })
    .await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 2);
    blocked.reply.send("第一句。".into()).unwrap();
    let queued = h.call().await;
    assert_eq!(queued.source, "第二の文");
    queued.reply.send("第二句。".into()).unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_same_target_association_reuses_session_without_disconnect() {
    let h = Harness::new(4);
    h.associate();
    let before = h.app.snapshot().unwrap();
    h.associate();
    let after = h.app.snapshot().unwrap();
    assert_eq!(
        before.session.as_ref().unwrap().session_id,
        after.session.as_ref().unwrap().session_id
    );
    // A new explicit association is a durable operation/journal fact, while
    // the native session and its selected stream must still be reused.
    assert!(after.commit_revision > before.commit_revision);
    let journal = h
        .app
        .session_records(SessionRecordQuery {
            session_id: after.session.as_ref().unwrap().session_id.clone(),
            dataset: SessionDataset::HookEvents,
            order: SessionRecordOrder::OldestFirst,
            filter: SessionRecordFilter::default(),
            cursor: None,
            page_size: 200,
        })
        .unwrap();
    assert_eq!(
        journal
            .records
            .iter()
            .filter(|r| matches!(r,SessionRecord::HookEvent{code,..}if code=="association_started"))
            .count(),
        2
    );
    assert_eq!(
        journal
            .records
            .iter()
            .filter(|r| matches!(r,SessionRecord::HookEvent{code,..}if code=="association_ready"))
            .count(),
        2
    );
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 0);
    let request = h.hook.game_window_activation_request().unwrap();
    h.command(Command::UnbindWindow(request)).await;
    assert_eq!(h.hook.unlinks.load(Ordering::SeqCst), 1);
    assert!(h.app.snapshot().unwrap().session.is_some());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frozen_learning_object_survives_new_dialogue_and_preferences_changes() {
    let mut h = Harness::new(4);
    let first = h.manual("覚えておきたい文。").await;
    let pending = h.call().await;
    let fixed = h.app.freeze_learning(first.clone(), 1, None).unwrap();
    h.manual("新しい台詞。").await;
    let mut preferences = h.app.snapshot().unwrap().preferences;
    preferences.theme = "dark".into();
    h.command(Command::SavePreferences { preferences }).await;
    let snap = h.app.snapshot().unwrap();
    assert_eq!(snap.learning_object, Some(fixed));
    assert_eq!(snap.learning_object.unwrap().unit.unit_id, first);
    pending.reply.send("想记住的句子。".into()).unwrap();
    h.call().await.reply.send("新的台词。".into()).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_without_streaming_capability_produces_nonstream_request_plan() {
    let mut h = Harness::new(4);
    let snapshot = h.app.snapshot().unwrap();
    let mut configuration = snapshot
        .configuration
        .draft
        .as_legacy()
        .expect("actual legacy fixture")
        .clone();
    configuration.models[0]
        .capabilities
        .insert("streaming".into(), serde_json::json!(false));
    let saved = h
        .command(Command::SaveConfiguration {
            configuration,
            expected_head_revision: snapshot.configuration.draft_head_revision,
        })
        .await;
    h.command(Command::ApplyConfiguration {
        revision_id: saved.result_id.unwrap(),
        expected_head_revision: snapshot.configuration.applied_head_revision,
    })
    .await;
    h.manual("非流式の検証文。 ").await;
    let call = h.call().await;
    assert!(
        !call.stream,
        "model capability must reach the actual application request plan"
    );
    call.reply.send("非流式验证句。".into()).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_retranslation_bypasses_real_cache_replays_once_and_preserves_correction() {
    let mut h = Harness::new(4);
    let initial = h.app.snapshot().unwrap().configuration;
    let mut configuration = initial
        .draft
        .as_legacy()
        .expect("actual legacy fixture")
        .clone();
    configuration
        .bindings
        .iter_mut()
        .find(|b| b.purpose == "translation")
        .unwrap()
        .schedule
        .insert("cacheTtlMs".into(), serde_json::json!(100000));
    let saved = h
        .command(Command::SaveConfiguration {
            configuration,
            expected_head_revision: initial.draft_head_revision,
        })
        .await;
    h.command(Command::ApplyConfiguration {
        revision_id: saved.result_id.unwrap(),
        expected_head_revision: initial.applied_head_revision,
    })
    .await;

    let first = h.manual("同じ文を読む。").await;
    h.call()
        .await
        .reply
        .send("缓存中的原始译文".into())
        .unwrap();
    wait(&h.app, |s| {
        s.history
            .units
            .iter()
            .any(|u| u.unit_id == first && u.translations.len() == 1)
    })
    .await;
    let second = h.manual("同じ文を読む。").await;
    wait(&h.app, |s| {
        s.history
            .units
            .iter()
            .any(|u| u.unit_id == second && u.translations.len() == 1)
    })
    .await;
    // A separate history record really used the old cache without a second
    // provider invocation. Explicit retranslating below must not disable this.
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    let cached = h.store.unit(second.clone(), 1).unwrap();
    assert_ne!(cached.unit_id, first);
    let original = cached.translations[0].clone();
    assert_eq!(original.text, "缓存中的原始译文");
    h.command(Command::SaveCorrection {
        translation_id: original.translation_id.clone(),
        base_revision: 0,
        text: "保留的人工修正".into(),
    })
    .await;
    let corrected = h.store.unit(second.clone(), 1).unwrap();
    let corrected_result = corrected.translations[0].clone();
    let frozen_hash = |unit: &ReadingUnit, result: &Translation| {
        content_hash(
            serde_json::to_vec(&(
                &unit.source_text,
                &unit.source_hash,
                unit.text_revision,
                result,
            ))
            .unwrap(),
        )
    };
    let before_hash = frozen_hash(&corrected, &corrected_result);
    let before = h.app.snapshot().unwrap().configuration;
    let applied_id = before.applied.as_ref().unwrap().revision_id().to_owned();
    let mut draft = before
        .draft
        .as_legacy()
        .expect("actual legacy fixture")
        .clone();
    draft.models[0].actual_model_id = Some("unapplied-draft-only-model".into());
    let draft_id = h
        .command(Command::SaveConfiguration {
            configuration: draft,
            expected_head_revision: before.draft_head_revision,
        })
        .await
        .result_id
        .unwrap();
    assert_ne!(draft_id, applied_id);

    let operation = "explicit-cache-bypass-operation".to_owned();
    let prepared = h
        .app
        .retranslate(second.clone(), 1, operation.clone())
        .unwrap();
    assert_eq!(prepared.len(), 1);
    let retranslating = h.call().await;
    assert_eq!(retranslating.source, "同じ文を読む。");
    assert_eq!(retranslating.model.as_deref(), Some("controlled-only"));
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 2);
    let replayed = h
        .app
        .retranslate(second.clone(), 1, operation.clone())
        .unwrap();
    assert_eq!(
        replayed.iter().map(|t| &t.task_id).collect::<Vec<_>>(),
        prepared.iter().map(|t| &t.task_id).collect::<Vec<_>>()
    );
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 2);
    let connection =
        rusqlite::Connection::open_with_flags(&h.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let (cache_kind,): (String,) = connection.query_row(
        "SELECT a.dispatch_kind FROM request_attempts a JOIN request_tasks t USING(task_id) WHERE t.task_id=?1",
        [&original.task_id], |r| Ok((r.get(0)?,)),
    ).unwrap();
    assert_eq!(cache_kind, "cache");
    let (dispatch, policy, frozen_configuration, count): (String, String, String, i64) = connection.query_row(
        "SELECT a.dispatch_kind,json_extract(t.input_snapshot_json,'$.cacheReadPolicy'),t.config_revision_id,(SELECT count(*) FROM request_tasks WHERE json_extract(input_snapshot_json,'$.operationId')=?2) FROM request_tasks t JOIN request_attempts a USING(task_id) WHERE t.task_id=?1",
        rusqlite::params![prepared[0].task_id,operation], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).unwrap();
    assert_eq!((dispatch.as_str(), policy.as_str()), ("initial", "bypass"));
    assert_eq!(frozen_configuration, applied_id);
    assert_ne!(frozen_configuration, draft_id);
    assert_eq!(count, 1);
    drop(connection);

    // The retranslating result arrives after both a correction and a newer
    // active sentence. It is an additional immutable result of the old unit.
    let newer = h.manual("新しい文へ進む。").await;
    retranslating.reply.send("迟到的后台重译".into()).unwrap();
    h.call().await.reply.send("新的句子".into()).unwrap();
    wait(&h.app, |s| {
        s.history
            .units
            .iter()
            .any(|u| u.unit_id == second && u.translations.len() == 2)
            && s.history
                .units
                .iter()
                .any(|u| u.unit_id == newer && u.translations.len() == 1)
    })
    .await;
    let after = h.store.unit(second.clone(), 1).unwrap();
    let old_result = after
        .translations
        .iter()
        .find(|t| t.translation_id == original.translation_id)
        .unwrap();
    assert_eq!(old_result.text, "缓存中的原始译文");
    assert_eq!(
        old_result.correction.as_ref().unwrap().text,
        "保留的人工修正"
    );
    assert_eq!(before_hash, frozen_hash(&after, old_result));
    let new_result = after
        .translations
        .iter()
        .find(|t| t.task_id == prepared[0].task_id)
        .unwrap();
    assert_eq!(new_result.text, "迟到的后台重译");
    assert!(new_result.correction.is_none());
    assert_eq!(h.app.snapshot().unwrap().current.unwrap().unit_id, newer);
    assert_eq!(
        h.store.unit(newer, 1).unwrap().translations[0].text,
        "新的句子"
    );
    let replayed_after_completion = h.app.retranslate(second, 1, operation).unwrap();
    assert_eq!(replayed_after_completion[0].task_id, prepared[0].task_id);
    assert_eq!(replayed_after_completion[0].status, "completed");
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 3);
    assert!(h.calls.try_recv().is_err());
    acceptance_evidence::record(
        "explicit-retranslation-bypasses-cache-idempotently",
        &[("STORAGE-13", 1)],
        serde_json::json!({"actual_test":"explicit_retranslation_bypasses_real_cache_replays_once_and_preserves_correction",
            "normal_dispatch":"cache","explicit_dispatch":dispatch,"cache_read_policy":policy,
            "applied_configuration":applied_id,"unapplied_draft":draft_id,
            "source_original_and_correction_hash_before":before_hash,"source_original_and_correction_hash_after":frozen_hash(&after,old_result),
            "provider_calls":h.gateway.count.load(Ordering::SeqCst),"same_operation_tasks":count}),
    );
}

fn model_test_request(snapshot: &AppSnapshot) -> ConfigurationTestRequest {
    ConfigurationTestRequest {
        revision_id: snapshot.configuration.draft.revision_id().to_owned(),
        object_kind: "model".into(),
        object_id: "controlled-model".into(),
    }
}

async fn next_projection(
    changes: &mut tokio::sync::watch::Receiver<ProjectionUpdate>,
) -> AppSnapshot {
    // Do not call runtime.snapshot(): it publishes and would hide a missing
    // notification from the operation under test.
    tokio::time::timeout(Duration::from_secs(5), changes.changed())
        .await
        .expect("operation did not publish its committed projection")
        .unwrap();
    changes.borrow_and_update().snapshot.clone()
}

async fn projected_text(
    changes: &mut tokio::sync::watch::Receiver<ProjectionUpdate>,
    text: &str,
) -> AppSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = changes.borrow_and_update().snapshot.clone();
            if snapshot
                .current
                .as_ref()
                .is_some_and(|u| u.source_text == text)
            {
                return snapshot;
            }
            changes.changed().await.unwrap();
        }
    })
    .await
    .expect("source did not publish directly to watch")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_publishes_open_source_before_stable_exact_revision_translation() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    let mut changes = h.app.subscribe();
    h.clock.0.store(1000, Ordering::SeqCst);
    h.events.send(hook_text(2, "静かな朝の")).unwrap();
    let first = projected_text(&mut changes, "静かな朝の")
        .await
        .current
        .unwrap();
    assert_eq!(first.text_revision, 1);
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    assert!(h.calls.try_recv().is_err());
    h.clock.0.store(2307, Ordering::SeqCst);
    h.events.send(hook_text(3, "物語だった。")).unwrap();
    let final_unit = projected_text(&mut changes, "静かな朝の物語だった。")
        .await
        .current
        .unwrap();
    assert_eq!(first.unit_id, final_unit.unit_id);
    assert_eq!(final_unit.text_revision, 2);
    let call = h.call().await;
    assert_eq!(call.source, final_unit.source_text);
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    assert_eq!(
        h.store.unit(first.unit_id, 1).unwrap().source_text,
        "静かな朝の"
    );
    call.reply.send("这是一个宁静早晨的故事。".into()).unwrap();
}

// Two capture-thread clock reads with no queued input guarantee that at least
// one actual recv_timeout iteration has finished its repository Due flush.
// Logical time stays fixed; neither this barrier nor awaiting the gateway uses
// Harness::call(), which deliberately advances time for unrelated old fixtures.
async fn frozen_capture_timer_barrier(clock: &Clock) {
    assert!(clock.1.load(Ordering::SeqCst));
    let before = clock.3.load(Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), async {
        while clock.3.load(Ordering::SeqCst) < before + 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("real capture timer did not progress at the frozen time");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_v2_frozen_idle_then_slow_append_or_growth_translates_exact_revision_once() {
    for (head, incoming, complete) in [
        ("静かな😀朝の", "物語だった。", "静かな😀朝の物語だった。"),
        ("雨", "雨がやんだ。", "雨がやんだ。"),
    ] {
        let mut h = Harness::new(4);
        h.select_hook().await;
        h.clock.1.store(true, Ordering::SeqCst);
        let mut changes = h.app.subscribe();
        h.clock.0.store(1000, Ordering::SeqCst);
        h.events.send(hook_text(2, head)).unwrap();
        let first = projected_text(&mut changes, head).await.current.unwrap();
        assert_eq!(first.text_revision, 1);
        for now in [2499, 2500, 2501, 3000] {
            h.clock.0.store(now, Ordering::SeqCst);
            frozen_capture_timer_barrier(&h.clock).await;
            assert_eq!(h.clock.0.load(Ordering::SeqCst), now);
            assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
            assert!(h.calls.try_recv().is_err());
            assert!(h.store.snapshot().unwrap().tasks.is_empty());
            let c = rusqlite::Connection::open_with_flags(
                &h.path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM reading_units WHERE state='open'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                1
            );
        }
        h.clock.0.store(3700, Ordering::SeqCst);
        h.events.send(hook_text(3, incoming)).unwrap();
        let latest = projected_text(&mut changes, complete)
            .await
            .current
            .unwrap();
        assert_eq!(latest.unit_id, first.unit_id);
        assert_eq!(latest.text_revision, 2);
        assert_eq!(
            h.store.unit(first.unit_id.clone(), 1).unwrap().source_text,
            head
        );
        let call = tokio::time::timeout(Duration::from_secs(5), h.calls.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(call.source, complete);
        assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
        frozen_capture_timer_barrier(&h.clock).await;
        assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
        assert!(h.calls.try_recv().is_err());
        // A completed old version arriving after a newer open unit may enrich
        // its own history only; it must not become the newer unit's translation.
        h.clock.0.store(4000, Ordering::SeqCst);
        h.events.send(hook_text(4, "新しい続き")).unwrap();
        let newer = projected_text(&mut changes, "新しい続き")
            .await
            .current
            .unwrap();
        assert_ne!(newer.unit_id, first.unit_id);
        call.reply.send("完整版本的译文".into()).unwrap();
        wait(&h.app, |snapshot| {
            snapshot
                .history
                .units
                .iter()
                .any(|u| u.unit_id == first.unit_id && !u.translations.is_empty())
        })
        .await;
        let current = h.app.snapshot().unwrap().current.unwrap();
        assert_eq!(current.unit_id, newer.unit_id);
        assert!(current.translations.is_empty());
        assert!(
            h.store
                .unit(first.unit_id, 1)
                .unwrap()
                .translations
                .is_empty()
        );
        assert_eq!(h.clock.0.load(Ordering::SeqCst), 4000);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_v3_quoted_prefix_growth_uses_exact_final_revision_once_without_open_network() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.clock.1.store(true, Ordering::SeqCst);
    let mut changes = h.app.subscribe();
    h.clock.0.store(1000, Ordering::SeqCst);
    h.events.send(hook_text(2, "【葵😀】「明日は")).unwrap();
    let first = projected_text(&mut changes, "【葵😀】「明日は")
        .await
        .current
        .unwrap();
    h.clock.0.store(2501, Ordering::SeqCst);
    frozen_capture_timer_barrier(&h.clock).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    assert!(h.store.snapshot().unwrap().tasks.is_empty());
    h.clock.0.store(3200, Ordering::SeqCst);
    h.events.send(hook_text(3, "「明日は図書館で")).unwrap();
    let second = projected_text(&mut changes, "【葵😀】「明日は図書館で")
        .await
        .current
        .unwrap();
    assert_eq!(second.unit_id, first.unit_id);
    assert_eq!(second.text_revision, 2);
    frozen_capture_timer_barrier(&h.clock).await;
    assert_eq!(h.clock.0.load(Ordering::SeqCst), 3200);
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    assert!(h.calls.try_recv().is_err());
    assert!(h.store.snapshot().unwrap().tasks.is_empty());
    h.clock.0.store(3600, Ordering::SeqCst);
    h.events.send(hook_text(4, "待とう。」")).unwrap();
    let complete = "【葵😀】「明日は図書館で待とう。」";
    let latest = projected_text(&mut changes, complete)
        .await
        .current
        .unwrap();
    assert_eq!(latest.unit_id, first.unit_id);
    assert_eq!(latest.text_revision, 3);
    let call = tokio::time::timeout(Duration::from_secs(5), h.calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        call.source, complete,
        "model must receive the source; its output is not a cleanup oracle"
    );
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    assert_eq!(
        h.store.unit(first.unit_id.clone(), 1).unwrap().source_text,
        "【葵😀】「明日は"
    );
    assert_eq!(
        h.store.unit(first.unit_id.clone(), 2).unwrap().source_text,
        "【葵😀】「明日は図書館で"
    );
    frozen_capture_timer_barrier(&h.clock).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    assert!(h.calls.try_recv().is_err());
    h.clock.0.store(4000, Ordering::SeqCst);
    h.events.send(hook_text(5, "次の原文")).unwrap();
    let newer = projected_text(&mut changes, "次の原文")
        .await
        .current
        .unwrap();
    call.reply.send("完整正文的译文".into()).unwrap();
    wait(&h.app, |snap| {
        snap.history
            .units
            .iter()
            .any(|u| u.unit_id == first.unit_id && !u.translations.is_empty())
    })
    .await;
    let current = h.app.snapshot().unwrap().current.unwrap();
    assert_eq!(current.unit_id, newer.unit_id);
    assert!(current.translations.is_empty());
    assert!(
        h.store
            .unit(first.unit_id.clone(), 1)
            .unwrap()
            .translations
            .is_empty()
    );
    assert!(
        h.store
            .unit(first.unit_id, 2)
            .unwrap()
            .translations
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_v3_quoted_growth_does_not_extend_the_creation_hard_age() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.clock.1.store(true, Ordering::SeqCst);
    let mut changes = h.app.subscribe();
    h.clock.0.store(1000, Ordering::SeqCst);
    h.events.send(hook_text(2, "語り手「雨は")).unwrap();
    let first = projected_text(&mut changes, "語り手「雨は")
        .await
        .current
        .unwrap();
    h.clock.0.store(6500, Ordering::SeqCst);
    h.events.send(hook_text(3, "「雨はまだ続く")).unwrap();
    let latest = projected_text(&mut changes, "語り手「雨はまだ続く")
        .await
        .current
        .unwrap();
    assert_eq!(latest.unit_id, first.unit_id);
    assert_eq!(latest.text_revision, 2);
    h.clock.0.store(6999, Ordering::SeqCst);
    frozen_capture_timer_barrier(&h.clock).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    assert!(h.calls.try_recv().is_err());
    h.clock.0.store(7000, Ordering::SeqCst);
    let call = tokio::time::timeout(Duration::from_secs(5), h.calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(call.source, "語り手「雨はまだ続く");
    h.clock.0.store(7001, Ordering::SeqCst);
    frozen_capture_timer_barrier(&h.clock).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    assert!(h.calls.try_recv().is_err());
    call.reply.send("未完成但已到硬上限".into()).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_hard_age_timer_finalizes_growth_without_waiting_for_another_callback() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.clock.1.store(true, Ordering::SeqCst);
    let mut changes = h.app.subscribe();
    h.clock.0.store(1000, Ordering::SeqCst);
    h.events.send(hook_text(2, "雨")).unwrap();
    let first = projected_text(&mut changes, "雨").await.current.unwrap();
    h.clock.0.store(1100, Ordering::SeqCst);
    h.events.send(hook_text(3, "雨がやんだ")).unwrap();
    let latest = projected_text(&mut changes, "雨がやんだ")
        .await
        .current
        .unwrap();
    assert_eq!(latest.unit_id, first.unit_id);
    assert_eq!(latest.text_revision, 2);
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    for now in [1100 + CAPTURE_IDLE_MS, 1000 + CAPTURE_MAX_AGE_MS - 1] {
        h.clock.0.store(now, Ordering::SeqCst);
        frozen_capture_timer_barrier(&h.clock).await;
        assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
        assert!(h.calls.try_recv().is_err());
    }
    h.clock.0.store(1000 + CAPTURE_MAX_AGE_MS, Ordering::SeqCst);
    let call = tokio::time::timeout(Duration::from_secs(5), h.calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(call.source, "雨がやんだ");
    h.clock
        .0
        .store(1000 + CAPTURE_MAX_AGE_MS + 1, Ordering::SeqCst);
    frozen_capture_timer_barrier(&h.clock).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 1);
    assert!(h.calls.try_recv().is_err());
    call.reply.send("雨停了".into()).unwrap();
    assert_eq!(h.store.snapshot().unwrap().history.units.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_seals_open_capture_without_sending_a_new_model_request() {
    let h = Harness::new(4);
    h.select_hook().await;
    let mut changes = h.app.subscribe();
    h.events.send(hook_text(2, "まだ続く原文")).unwrap();
    projected_text(&mut changes, "まだ続く原文").await;
    h.app.shutdown().await.unwrap();
    h.clock.0.fetch_add(CAPTURE_IDLE_MS * 2, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    assert_eq!(
        h.store.snapshot().unwrap().session.unwrap().state,
        "stopped"
    );
    assert!(
        h.store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Shutdown
                },
                9000
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        h.store.snapshot().unwrap().history.units[0].source_text,
        "まだ続く原文"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_shutdown_can_resume_capture_and_retry_a_clean_stopped_exit() {
    let mut h = Harness::new(4);
    h.select_hook().await;
    h.events.send(hook_text(2, "終了前の原文")).unwrap();
    wait(&h.app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| u.source_text == "終了前の原文")
    })
    .await;
    h.hook.disconnect_failure.store(true, Ordering::SeqCst);
    assert_eq!(
        h.app.shutdown().await.unwrap_err().code,
        "fixture_disconnect_failed"
    );
    h.events.send(hook_text(3, "終了失敗後に読む文。")).unwrap();
    let call = h.call().await;
    assert_eq!(call.source, "終了失敗後に読む文。");
    call.reply
        .send("退出失败后继续阅读的句子。".into())
        .unwrap();
    h.app.shutdown().await.unwrap();
    assert_eq!(
        h.store.snapshot().unwrap().session.unwrap().state,
        "stopped"
    );
    assert_eq!(h.store.snapshot().unwrap().history.units.len(), 2);
}

#[tokio::test]
async fn finalized_callback_scheduled_after_disconnect_or_reselection_cannot_prepare_automatic_tasks()
 {
    for boundary in [
        Command::Disconnect,
        Command::SelectSource {
            source_id: "controlled-source".into(),
        },
    ] {
        let mut h = Harness::new(4);
        let count = Arc::new(AtomicUsize::new(0));
        h.app = ApplicationRuntime::new(
            Arc::new(PublicationFaultRepository {
                inner: h.store.clone(),
                fail_next_snapshot: AtomicBool::new(false),
                capture_preparations: count.clone(),
            }),
            h.clock.clone(),
            h.hook.clone(),
            h.gateway.clone(),
            Arc::new(NoLanguage),
            Arc::new(NoVault),
        )
        .unwrap();
        h.select_hook().await;
        h.events.send(hook_text(2, "遅れて実行される文。")).unwrap();
        // Deliberately do not yield this single-thread Tokio executor. The
        // actual capture thread/writer commits and spawns a future, but that
        // future cannot execute until the real boundary command has committed.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while h
            .store
            .snapshot()
            .unwrap()
            .current
            .is_none_or(|u| u.source_text != "遅れて実行される文。")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "capture never committed"
            );
            std::thread::yield_now();
        }
        h.store
            .dispatch(
                CommandEnvelope {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    expected_revision: None,
                    command: boundary,
                },
                h.clock.now_ms(),
            )
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while count.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("late automatic preparation never reached real writer guard");
        assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
        assert!(h.store.snapshot().unwrap().tasks.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paused_capture_ignores_later_callbacks_before_processing_the_next_health_marker() {
    let h = Harness::new(4);
    h.select_hook().await;
    h.events.send(hook_text(2, "保存済みの途中")).unwrap();
    wait(&h.app, |s| {
        s.current
            .as_ref()
            .is_some_and(|u| u.source_text == "保存済みの途中")
    })
    .await;
    h.command(Command::SetCapturePaused { paused: true }).await;
    h.events.send(hook_text(3, "取り込まない文。")).unwrap();
    h.events
        .send(HookUpdate::Health {
            session_id: Some("controlled-hook-session".into()),
            diagnostic: Diagnostic {
                code: "fixture_after_paused_callback".into(),
                stage: "test".into(),
                message: "ordered consumer marker".into(),
                recoverable: true,
            },
        })
        .unwrap();
    wait(&h.app, |s| {
        s.diagnostics
            .iter()
            .any(|d| d.code == "fixture_after_paused_callback")
    })
    .await;
    assert_eq!(
        h.store.snapshot().unwrap().current.unwrap().source_text,
        "保存済みの途中"
    );
    assert_eq!(h.store.snapshot().unwrap().history.units.len(), 1);
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_configuration_test_publishes_committed_failure_without_snapshot_refresh() {
    let h = Harness::new(4);
    let mut changes = h.app.subscribe();
    let before = changes.borrow_and_update().snapshot.clone();
    let error = h
        .app
        .test_configuration(model_test_request(&before))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        AppError::new("not_a_live_model", "test", "not used by runtime tests")
    );
    let published = next_projection(&mut changes).await;
    assert_eq!(published.commit_revision, before.commit_revision + 1);
    assert_eq!(published.configuration, before.configuration);
    assert_eq!(published.configuration_tests.len(), 1);
    let record = &published.configuration_tests[0];
    assert_eq!(record.status, "failed");
    assert_eq!(record.error_code.as_deref(), Some(error.code.as_str()));
    assert_eq!(record.object_kind, "model");
    assert_eq!(record.object_id, "controlled-model");
    assert!(!record.stale);
    // This independent Repository read verifies persistence only after watch
    // already proved the runtime published without an explicit refresh.
    let stored = h.store.snapshot().unwrap();
    assert_eq!(stored.commit_revision, published.commit_revision);
    assert_eq!(stored.configuration_tests, published.configuration_tests);
}

#[tokio::test]
async fn failed_configuration_test_refresh_preserves_commit_and_draft_head_cas() {
    let h = Harness::new(4);
    let mut changes = h.app.subscribe();
    let before = changes.borrow_and_update().snapshot.clone();
    h.app
        .test_configuration(model_test_request(&before))
        .await
        .unwrap_err();
    let tested = next_projection(&mut changes).await;
    let mut edited = tested
        .configuration
        .draft
        .as_legacy()
        .expect("actual legacy fixture")
        .clone();
    edited.models[0].display_name = "Explicit local edit".into();

    let stale_commit = h
        .app
        .dispatch(CommandEnvelope {
            operation_id: "save-before-failed-test-refresh".into(),
            expected_revision: Some(before.commit_revision),
            command: Command::SaveConfiguration {
                configuration: edited.clone(),
                expected_head_revision: before.configuration.draft_head_revision,
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        (stale_commit.code.as_str(), stale_commit.stage.as_str()),
        ("revision_conflict", "command")
    );
    assert_eq!(
        h.store.snapshot().unwrap().configuration,
        before.configuration
    );

    h.app
        .dispatch(CommandEnvelope {
            operation_id: "explicit-save-after-failed-test-refresh".into(),
            expected_revision: Some(tested.commit_revision),
            command: Command::SaveConfiguration {
                configuration: edited,
                expected_head_revision: before.configuration.draft_head_revision,
            },
        })
        .await
        .unwrap();
    let saved = next_projection(&mut changes).await;
    assert_eq!(
        saved
            .configuration
            .draft
            .as_legacy()
            .expect("actual legacy fixture")
            .models[0]
            .display_name,
        "Explicit local edit"
    );
    assert_eq!(
        saved.configuration.draft_head_revision,
        before.configuration.draft_head_revision + 1
    );
    assert_eq!(saved.configuration.applied, before.configuration.applied);
    assert_eq!(
        saved.configuration.applied_head_revision,
        before.configuration.applied_head_revision
    );

    // A different window saves a real new head. Refreshing the global commit
    // must not grant this older editor permission to overwrite that head.
    let mut other_edit = saved
        .configuration
        .draft
        .as_legacy()
        .expect("actual legacy fixture")
        .clone();
    other_edit.models[0].display_name = "Other window edit".into();
    h.app
        .dispatch(CommandEnvelope {
            operation_id: "other-window-save".into(),
            expected_revision: Some(saved.commit_revision),
            command: Command::SaveConfiguration {
                configuration: other_edit,
                expected_head_revision: saved.configuration.draft_head_revision,
            },
        })
        .await
        .unwrap();
    let concurrent = next_projection(&mut changes).await;
    let stale_head = h
        .app
        .dispatch(CommandEnvelope {
            operation_id: "old-editor-keeps-old-head".into(),
            expected_revision: Some(concurrent.commit_revision),
            command: Command::SaveConfiguration {
                configuration: saved
                    .configuration
                    .draft
                    .as_legacy()
                    .expect("actual legacy fixture")
                    .clone(),
                expected_head_revision: saved.configuration.draft_head_revision,
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        (stale_head.code.as_str(), stale_head.stage.as_str()),
        ("revision_conflict", "configuration")
    );
    let unchanged = h.store.snapshot().unwrap();
    assert_eq!(unchanged.configuration, concurrent.configuration);
    assert_eq!(unchanged.commit_revision, concurrent.commit_revision);
    assert_eq!(
        unchanged
            .configuration
            .draft
            .as_legacy()
            .expect("actual legacy fixture")
            .models[0]
            .display_name,
        "Other window edit"
    );
}

#[tokio::test]
async fn successful_local_configuration_test_publishes_committed_record() {
    let h = Harness::new(4);
    let mut changes = h.app.subscribe();
    let before = changes.borrow_and_update().snapshot.clone();
    let result = h
        .app
        .test_configuration(ConfigurationTestRequest {
            revision_id: before.configuration.draft.revision_id().to_owned(),
            object_kind: "local".into(),
            object_id: String::new(),
        })
        .await
        .unwrap();
    assert_eq!(result.status, "passed");
    let published = next_projection(&mut changes).await;
    assert_eq!(published.commit_revision, before.commit_revision + 1);
    assert_eq!(published.configuration, before.configuration);
    assert_eq!(published.configuration_tests[0].status, "passed");
    assert_eq!(published.configuration_tests[0].error_code, None);
}

/// Delegate all business and CAS semantics to real SQLite. Only the first
/// projection read after a test-record commit fails, then diagnostics can be
/// persisted and published through the runtime's normal recovery path.
struct PublicationFaultRepository {
    inner: Arc<SqliteStore>,
    fail_next_snapshot: AtomicBool,
    capture_preparations: Arc<AtomicUsize>,
}
macro_rules! delegate_publication_repository {
    ($(fn $method:ident(&self $(,$arg:ident:$ty:ty)*) -> $result:ty;)+) => {
        $(fn $method(&self,$($arg:$ty),*) -> $result { self.inner.$method($($arg),*) })+
    };
}
impl ProductRepository for PublicationFaultRepository {
    delegate_publication_repository! {
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
        fn prepare_study_tasks(&self,id:String,revision:u32,purpose:String,token:Option<u32>,operation:String,now:i64)->AppResult<Vec<TaskDispatch>>;
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
        fn set_tokens(&self,input:TokenSnapshotInput,now:i64)->AppResult<()>;
        fn record_diagnostic(&self,diagnostic:Diagnostic,now:i64)->AppResult<()>;
        fn record_hook_diagnostic(&self,session:Option<String>,diagnostic:Diagnostic,now:i64)->AppResult<()>;
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
    fn prepare_capture_tasks(
        &self,
        version: CaptureVersion,
        operation: String,
        now: i64,
    ) -> AppResult<Vec<TaskDispatch>> {
        let result = self.inner.prepare_capture_tasks(version, operation, now);
        self.capture_preparations.fetch_add(1, Ordering::SeqCst);
        result
    }
    fn snapshot(&self) -> AppResult<AppSnapshot> {
        if self.fail_next_snapshot.swap(false, Ordering::SeqCst) {
            Err(AppError::new(
                "fixture_publication_unavailable",
                "storage",
                "controlled projection read failure",
            ))
        } else {
            self.inner.snapshot()
        }
    }
    fn record_model_test(
        &self,
        input: app_core::domain::testing::ModelTestRecordInput,
    ) -> AppResult<()> {
        self.inner.record_model_test(input)?;
        self.fail_next_snapshot.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn configuration_test_publish_failure_keeps_original_error_and_records_diagnostic() {
    let h = Harness::new(4);
    let app = ApplicationRuntime::new(
        Arc::new(PublicationFaultRepository {
            inner: h.store.clone(),
            fail_next_snapshot: AtomicBool::new(false),
            capture_preparations: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(Clock(
            AtomicI64::new(100),
            AtomicBool::new(false),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
        )),
        h.hook,
        h.gateway,
        Arc::new(NoLanguage),
        Arc::new(NoVault),
    )
    .unwrap();
    let mut changes = app.subscribe();
    let before = changes.borrow_and_update().snapshot.clone();
    let error = app
        .test_configuration(model_test_request(&before))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        AppError::new("not_a_live_model", "test", "not used by runtime tests")
    );
    let published = next_projection(&mut changes).await;
    assert_eq!(published.configuration_tests[0].status, "failed");
    assert_eq!(
        published.configuration_tests[0].error_code.as_deref(),
        Some("not_a_live_model")
    );
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.code == "fixture_publication_unavailable" && d.stage == "storage")
    );
    let stored = h.store.snapshot().unwrap();
    assert_eq!(stored.configuration_tests, published.configuration_tests);
    assert_eq!(stored.diagnostics, published.diagnostics);
    assert_eq!(stored.configuration, before.configuration);
}

fn lifecycle_control(session: &str, event: &str, diagnostic: &str) -> HookUpdate {
    HookUpdate::Control {
        event: Box::new(HookEventInput {
            event_id: format!("{session}:{event}"),
            operation_id: None,
            session_id: Some(session.into()),
            source: None,
            wire_sequence: None,
            observed_at_ms: 500,
            detail: HookStageInput {
                level: "info".into(),
                stage: "disconnect".into(),
                code: event.into(),
                message: "controlled native fact".into(),
            },
            lost_count: None,
            integrity: "intact".into(),
        }),
        diagnostic: Some(Diagnostic {
            code: diagnostic.into(),
            stage: "capture".into(),
            message: "controlled lifecycle".into(),
            recoverable: true,
        }),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn natural_exit_consumer_keeps_tail_facts_and_acks_without_waiting_on_itself() {
    let h = Harness::new(4);
    h.select_hook().await;
    h.events.send(hook_text(2, "終了前の途中")).unwrap();
    wait(&h.app, |snapshot| {
        snapshot
            .current
            .as_ref()
            .is_some_and(|u| u.source_text == "終了前の途中")
    })
    .await;
    let before = h.store.history(None, None, 100).unwrap().units.len();
    let (entered, entering) = mpsc::sync_channel(1);
    let (resume, resumed) = mpsc::sync_channel(1);
    *h.hook.natural_cleanup_gate.lock().unwrap() = Some((entered, resumed));
    h.hook
        .late_on_stop
        .lock()
        .unwrap()
        .push(hook_text(3, "終了直前の原文。"));
    h.events
        .send(lifecycle_control(
            "controlled-hook-session",
            "process_disconnected",
            "game_disconnected",
        ))
        .unwrap();
    h.events
        .send(lifecycle_control(
            "controlled-hook-session",
            "repeated_process_disconnected",
            "game_disconnected",
        ))
        .unwrap();
    tokio::task::spawn_blocking(move || entering.recv_timeout(Duration::from_secs(3)).unwrap())
        .await
        .unwrap();
    h.clock.0.fetch_add(CAPTURE_IDLE_MS + 1, Ordering::SeqCst);
    for marker in ["during-natural-stop-first", "during-natural-stop-second"] {
        h.events
            .send(HookUpdate::Health {
                session_id: Some(hook_session().session_id),
                diagnostic: Diagnostic {
                    code: marker.into(),
                    stage: "test".into(),
                    message: "ordered consumer marker while native cleanup waits".into(),
                    recoverable: true,
                },
            })
            .unwrap();
        wait(&h.app, |snapshot| {
            snapshot.diagnostics.iter().any(|d| d.code == marker)
        })
        .await;
    }
    let database =
        rusqlite::Connection::open_with_flags(&h.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let state: String = database.query_row("SELECT state FROM reading_units WHERE session_id=?1 ORDER BY unit_ordinal DESC LIMIT 1", [hook_session().session_id], |row| row.get(0)).unwrap();
    assert_eq!(
        state, "open",
        "Due must not finalize a known disconnected SID before its drain barrier"
    );
    assert_eq!(
        h.gateway.count.load(Ordering::SeqCst),
        0,
        "natural stop must revoke automatic model eligibility immediately"
    );
    resume.send(()).unwrap();
    wait(&h.app, |s| {
        s.session.as_ref().is_some_and(|s| s.state == "stopped")
            && h.hook.natural_cleanup_committed.load(Ordering::SeqCst)
    })
    .await;
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 1);
    let raw = h
        .store
        .session_records(
            SessionRecordQuery {
                session_id: "controlled-hook-session".into(),
                dataset: SessionDataset::Extraction,
                order: SessionRecordOrder::OldestFirst,
                filter: SessionRecordFilter::default(),
                cursor: None,
                page_size: 100,
            },
            1000,
        )
        .unwrap();
    assert!(raw.records.iter().any(|r| matches!(r, SessionRecord::Extraction { text, .. } if text.as_deref() == Some("終了直前の原文。"))));
    assert_eq!(
        h.store.history(None, None, 100).unwrap().units.len(),
        before,
        "tail facts remain raw without creating new body units"
    );
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    assert!(
        !h.app
            .snapshot()
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "fixture_barrier_deadlock")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn natural_exit_terminal_write_failure_is_not_acknowledged_or_reauthorized() {
    let h = Harness::new(4);
    h.select_hook().await;
    let database = rusqlite::Connection::open(&h.path).unwrap();
    database.execute_batch("CREATE TRIGGER controlled_terminal_failure BEFORE UPDATE OF state ON sessions WHEN NEW.state='stopped' BEGIN SELECT RAISE(ABORT,'controlled terminal unavailable'); END;").unwrap();
    h.events
        .send(lifecycle_control(
            "controlled-hook-session",
            "process_disconnected",
            "game_disconnected",
        ))
        .unwrap();
    wait(&h.app, |snapshot| {
        snapshot
            .diagnostics
            .iter()
            .any(|d| d.code == "storage_failure")
    })
    .await;
    assert!(
        !h.hook.natural_cleanup_committed.load(Ordering::SeqCst),
        "terminal state persistence is required before native cleanup can accept its ACK"
    );
    assert_eq!(h.app.snapshot().unwrap().session.unwrap().state, "active");
    let before = h.store.history(None, None, 100).unwrap().units.len();
    h.events.send(hook_text(2, "退出後の遅い原文。")).unwrap();
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::DrainBarrier { acknowledged })
        .unwrap();
    tokio::task::spawn_blocking(move || {
        consumed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        h.store.history(None, None, 100).unwrap().units.len(),
        before
    );
    assert_eq!(
        h.gateway.count.load(Ordering::SeqCst),
        0,
        "failed cleanup cannot restore a disconnected SID's model eligibility"
    );
    database
        .execute_batch("DROP TRIGGER controlled_terminal_failure;")
        .unwrap();
    h.command(Command::Disconnect).await;
    assert_eq!(h.app.snapshot().unwrap().session.unwrap().state, "stopped");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_old_lifecycle_and_session_barrier_never_stop_or_seal_the_new_session() {
    let h = Harness::new(4);
    h.select_hook().await;
    let old = hook_session().session_id;
    let mut next = hook_session();
    next.session_id = "new-hook-session".into();
    next.broker_instance_id = "new-hook-broker".into();
    next.windows_pid = 101;
    next.process_created_u64 = "101".into();
    next.target_identity_sha256 = content_hash("new-hook-target");
    h.store.open_hook_session(next.clone(), 600).unwrap();
    *h.hook.binding.lock().unwrap() = Some(GameWindowActivationRequest {
        expected_session_id: next.session_id.clone(),
        expected_target_identity_sha256: next.target_identity_sha256.clone(),
    });
    h.events
        .send(lifecycle_control(
            &old,
            "late-connected",
            "capture_connected",
        ))
        .unwrap();
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::SessionDrainBarrier {
            session_id: old.clone(),
            acknowledged,
        })
        .unwrap();
    tokio::task::spawn_blocking(move || {
        consumed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        h.app.snapshot().unwrap().session.unwrap().state,
        "opening",
        "A's late connected event must not activate B"
    );
    let HookUpdate::Text(mut text) = hook_text(10, "新しい候補") else {
        panic!("text")
    };
    text.session_id = next.session_id.clone();
    text.source.source_id = "new-hook-source".into();
    text.source.host_instance_id = next.broker_instance_id.clone();
    h.store.ingest_hook_text(*text.clone(), 601).unwrap();
    h.store
        .dispatch(
            CommandEnvelope {
                operation_id: "new-select".into(),
                expected_revision: None,
                command: Command::SelectSource {
                    source_id: text.source.source_id.clone(),
                },
            },
            602,
        )
        .unwrap();
    text.event_id = "new-growing".into();
    text.sequence_u64 = "11".into();
    text.text = "まだ続く".into();
    h.store.ingest_hook_text(*text.clone(), 603).unwrap();
    h.events
        .send(lifecycle_control(&old, "late-error", "capture_failed"))
        .unwrap();
    h.events
        .send(lifecycle_control(
            &old,
            "late-disconnected",
            "game_disconnected",
        ))
        .unwrap();
    h.store
        .record_hook_diagnostic(
            None,
            Diagnostic {
                code: "game_disconnected".into(),
                stage: "capture".into(),
                message: "unscoped native status".into(),
                recoverable: true,
            },
            605,
        )
        .unwrap();
    h.events
        .send(HookUpdate::Health {
            session_id: Some(old.clone()),
            diagnostic: Diagnostic {
                code: "integrity_lost".into(),
                stage: "capture".into(),
                message: "old health".into(),
                recoverable: true,
            },
        })
        .unwrap();
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::SessionDrainBarrier {
            session_id: old,
            acknowledged,
        })
        .unwrap();
    tokio::task::spawn_blocking(move || {
        consumed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    let snapshot = h.app.snapshot().unwrap();
    assert_eq!(
        snapshot.session.as_ref().unwrap().session_id,
        next.session_id
    );
    assert_eq!(snapshot.session.as_ref().unwrap().state, "active");
    text.event_id = "new-growing-continued".into();
    text.sequence_u64 = "12".into();
    text.text = "まだ続く原文".into();
    h.store.ingest_hook_text(*text, 604).unwrap();
    let history = h.store.history(None, None, 100).unwrap();
    assert_eq!(
        history
            .units
            .iter()
            .filter(|u| u.session_id == next.session_id)
            .count(),
        1,
        "old SID barrier must not seal B's growing sentence into a new unit"
    );
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_still_persists_late_text_and_control_before_fifo_ack_without_new_body() {
    let h = Harness::new(4);
    h.select_hook().await;
    let before = h.store.history(None, None, 100).unwrap().units.len();
    h.hook.late_on_stop.lock().unwrap().extend([
        hook_text(2, "退出期间仍收到的原文。"),
        HookUpdate::Control {
            event: Box::new(HookEventInput {
                event_id: "closing-control".into(),
                operation_id: None,
                session_id: Some("controlled-hook-session".into()),
                source: None,
                wire_sequence: None,
                observed_at_ms: 200,
                detail: HookStageInput {
                    level: "warning".into(),
                    stage: "capture".into(),
                    code: "closing_received_control".into(),
                    message: "退出期间仍收到控制事实".into(),
                },
                lost_count: Some("1".into()),
                integrity: "lost".into(),
            }),
            diagnostic: None,
        },
    ]);
    h.app.shutdown().await.unwrap();
    let raw = h
        .store
        .session_records(
            SessionRecordQuery {
                session_id: "controlled-hook-session".into(),
                dataset: SessionDataset::Extraction,
                order: SessionRecordOrder::OldestFirst,
                filter: SessionRecordFilter::default(),
                cursor: None,
                page_size: 200,
            },
            300,
        )
        .unwrap();
    assert!(raw.records.iter().any(|r|matches!(r,SessionRecord::Extraction{text,..}if text.as_deref()==Some("退出期间仍收到的原文。"))));
    let controls = h
        .store
        .session_records(
            SessionRecordQuery {
                session_id: "controlled-hook-session".into(),
                dataset: SessionDataset::HookEvents,
                order: SessionRecordOrder::OldestFirst,
                filter: SessionRecordFilter::default(),
                cursor: None,
                page_size: 200,
            },
            301,
        )
        .unwrap();
    assert!(
        controls.records.iter().any(
            |r| matches!(r,SessionRecord::HookEvent{event_id,..}if event_id=="closing-control")
        )
    );
    assert!(raw.completeness.known_loss);
    assert_eq!(
        h.store.history(None, None, 100).unwrap().units.len(),
        before
    );
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_replay_and_conflict_cannot_stop_a_later_native_connection() {
    let h = Harness::new(4);
    h.select_hook().await;
    let disconnect = CommandEnvelope {
        operation_id: "durable-native-disconnect".into(),
        expected_revision: None,
        command: Command::Disconnect,
    };
    let first = h.app.dispatch(disconnect.clone()).await.unwrap();
    assert!(!first.replayed);
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 1);
    let replay = h.app.dispatch(disconnect.clone()).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.revision, first.revision);
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 1);
    let conflicting = CommandEnvelope {
        operation_id: "conflicting-disconnect".into(),
        expected_revision: None,
        command: Command::ManualInput {
            text: "既存の操作。".into(),
            source_language: "ja".into(),
        },
    };
    h.store.dispatch(conflicting, 1000).unwrap();
    let conflict = CommandEnvelope {
        operation_id: "conflicting-disconnect".into(),
        expected_revision: None,
        command: Command::Disconnect,
    };
    assert_eq!(
        h.app.dispatch(conflict).await.unwrap_err().code,
        "operation_conflict"
    );
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 1);
    let stale = CommandEnvelope {
        operation_id: "stale-disconnect".into(),
        expected_revision: Some(0),
        command: Command::Disconnect,
    };
    assert_eq!(
        h.app.dispatch(stale).await.unwrap_err().code,
        "revision_conflict"
    );
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 1);
}

fn unbind_envelope(operation: &str, request: GameWindowActivationRequest) -> CommandEnvelope {
    CommandEnvelope {
        operation_id: operation.into(),
        // Window identity, rather than unrelated text commits, is authority.
        expected_revision: Some(0),
        command: Command::UnbindWindow(request),
    }
}
fn unbind_success_counts(path: &std::path::Path, operation: &str) -> (i64, i64) {
    let conn = rusqlite::Connection::open(path).unwrap();
    (
        conn.query_row(
            "SELECT count(*) FROM operation_receipts WHERE operation_id=?1",
            [operation],
            |r| r.get(0),
        )
        .unwrap(),
        conn.query_row(
            "SELECT count(*) FROM hook_events WHERE event_id=?1 AND code='window_unlinked'",
            [format!("command-hook:{operation}")],
            |r| r.get(0),
        )
        .unwrap(),
    )
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unbind_preflight_effect_and_commit_are_serialized_against_association_and_replay_is_inert()
{
    let h = Harness::new(4);
    h.associate();
    let old = h.hook.game_window_activation_request().unwrap();
    let envelope = unbind_envelope("controlled-unbind-atomic", old.clone());
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    *h.hook.unbind_after_preflight.lock().unwrap() = Some((entered_tx, release_rx));
    let app = h.app.clone();
    let request = envelope.clone();
    let unbind = tokio::spawn(async move { app.dispatch(request).await });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        unbind_success_counts(&h.path, &envelope.operation_id),
        (0, 0)
    );
    assert_eq!(h.hook.unlinks.load(Ordering::SeqCst), 0);
    let before = h.hook.associations.load(Ordering::SeqCst);
    let app = h.app.clone();
    let (attempted_tx, attempted_rx) = mpsc::sync_channel(1);
    let (finished_tx, finished_rx) = mpsc::sync_channel(1);
    let next = std::thread::spawn(move || {
        attempted_tx.send(()).unwrap();
        let outcome = app.associate(AssociateRequest {
            operation_id: "later-B-association".into(),
            game_id: None,
            candidate_id: "same-process-new-HWND".into(),
            confirm_switch: false,
        });
        finished_tx.send(()).unwrap();
        outcome
    });
    attempted_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        finished_rx.recv_timeout(Duration::from_millis(80)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    // associate() increments this before the fixture binding mutex. Thus this
    // assertion observes Runtime's capture_lock, not the fixture's own lease.
    assert_eq!(h.hook.associations.load(Ordering::SeqCst), before);
    release_tx.send(()).unwrap();
    let receipt = unbind.await.unwrap().unwrap();
    next.join().unwrap().unwrap();
    assert!(!receipt.replayed);
    assert_eq!(
        receipt.result_id.as_deref(),
        Some(old.expected_target_identity_sha256.as_str())
    );
    assert_eq!(
        unbind_success_counts(&h.path, &envelope.operation_id),
        (1, 1)
    );
    let new = h.hook.game_window_activation_request().unwrap();
    assert_ne!(
        new.expected_target_identity_sha256,
        old.expected_target_identity_sha256
    );
    assert_eq!(new.expected_session_id, old.expected_session_id);
    let replay = h.app.dispatch(envelope.clone()).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.result_id, receipt.result_id);
    assert_eq!(h.hook.game_window_activation_request().unwrap(), new);
    assert_eq!(h.hook.unlinks.load(Ordering::SeqCst), 1);
    let conflict = h
        .app
        .dispatch(unbind_envelope(&envelope.operation_id, new.clone()))
        .await
        .unwrap_err();
    assert_eq!(conflict.code, "operation_conflict");
    assert_eq!(h.hook.game_window_activation_request().unwrap(), new);
    assert_eq!(
        unbind_success_counts(&h.path, &envelope.operation_id),
        (1, 1)
    );
    let conn = rusqlite::Connection::open(&h.path).unwrap();
    let owner: String = conn
        .query_row(
            "SELECT session_id FROM hook_events WHERE event_id=?1",
            [format!("command-hook:{}", envelope.operation_id)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(owner, old.expected_session_id);
    acceptance_evidence::record(
        "schema-two-unbind-serialization-replay",
        &[("V2-UNBIND-SERIALIZATION", 1), ("STORAGE-20", 1)],
        serde_json::json!({"producer":"ApplicationRuntime + SqliteStore","hook":"controlled binding port; not a live game"}),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unbind_stale_session_or_window_is_rejected_before_success_fact_or_effect() {
    let h = Harness::new(4);
    h.associate();
    let old = h.hook.game_window_activation_request().unwrap();
    let initial = h.app.snapshot().unwrap().commit_revision;
    for (index, next) in [
        GameWindowActivationRequest {
            expected_session_id: "other-session".into(),
            ..old.clone()
        },
        GameWindowActivationRequest {
            expected_target_identity_sha256: content_hash("new-HWND-same-process"),
            ..old.clone()
        },
        GameWindowActivationRequest {
            expected_target_identity_sha256: content_hash("new-window-class-same-HWND"),
            ..old.clone()
        },
    ]
    .into_iter()
    .enumerate()
    {
        *h.hook.binding.lock().unwrap() = Some(next.clone());
        let operation = format!("stale-window-unbind-{index}");
        let error = h
            .app
            .dispatch(unbind_envelope(&operation, old.clone()))
            .await
            .unwrap_err();
        assert_eq!(error.code, "game_window_unbind_stale");
        assert_eq!(h.hook.game_window_activation_request().unwrap(), next);
        assert_eq!(unbind_success_counts(&h.path, &operation), (0, 0));
        assert_eq!(h.app.snapshot().unwrap().commit_revision, initial);
    }
    assert_eq!(h.hook.unlinks.load(Ordering::SeqCst), 0);
    acceptance_evidence::record(
        "schema-two-unbind-stale-identity",
        &[("V2-UNBIND-IDENTITY", 1), ("STORAGE-21", 1)],
        serde_json::json!({"producer":"ApplicationRuntime + SqliteStore","hook":"controlled binding port; not a live game"}),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_unbind_commit_failure_restores_association_then_retry_preserves_capture_and_history()
{
    let h = Harness::new(4);
    h.select_hook().await;
    h.command(Command::SetTranslationEnabled { enabled: false })
        .await;
    let request = h.hook.game_window_activation_request().unwrap();
    let before = h.app.snapshot().unwrap().session.unwrap();
    let envelope = unbind_envelope("unbind-real-sql-failure", request.clone());
    let conn = rusqlite::Connection::open(&h.path).unwrap();
    conn.execute_batch("CREATE TRIGGER controlled_unbind_commit_failure BEFORE INSERT ON operation_receipts WHEN NEW.operation_id='unbind-real-sql-failure' BEGIN SELECT RAISE(ABORT,'controlled durable commit failure'); END;").unwrap();
    let error = h.app.dispatch(envelope.clone()).await.unwrap_err();
    assert_eq!(error.code, "storage_failure");
    assert_eq!(h.hook.game_window_activation_request().unwrap(), request);
    assert_eq!(h.hook.unlinks.load(Ordering::SeqCst), 0);
    assert_eq!(
        unbind_success_counts(&h.path, &envelope.operation_id),
        (0, 0)
    );
    assert_eq!(h.app.snapshot().unwrap().session.unwrap(), before);
    conn.execute_batch("DROP TRIGGER controlled_unbind_commit_failure")
        .unwrap();
    h.events.send(hook_text(2, "解除前の原文。")).unwrap();
    wait(&h.app, |s| {
        s.history
            .units
            .iter()
            .any(|u| u.source_text == "解除前の原文。")
    })
    .await;
    let history_before = h.app.snapshot().unwrap().history.units;
    let receipt = h.app.dispatch(envelope.clone()).await.unwrap();
    assert!(!receipt.replayed);
    assert_eq!(
        h.hook.game_window_activation_request().unwrap_err().code,
        "game_window_unbound"
    );
    let after = h.app.snapshot().unwrap().session.unwrap();
    assert_eq!(after.session_id, before.session_id);
    assert_eq!(
        after
            .sources
            .iter()
            .filter(|s| s.selected)
            .map(|s| &s.source_id)
            .collect::<Vec<_>>(),
        before
            .sources
            .iter()
            .filter(|s| s.selected)
            .map(|s| &s.source_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(h.app.snapshot().unwrap().history.units, history_before);
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 0);
    h.events
        .send(hook_text(3, "解除後も日本語を受信する。"))
        .unwrap();
    wait(&h.app, |s| {
        s.history
            .units
            .iter()
            .any(|u| u.source_text.contains("解除後も日本語を受信する。"))
    })
    .await;
    let raw = h
        .app
        .session_records(SessionRecordQuery {
            session_id: request.expected_session_id.clone(),
            dataset: SessionDataset::Extraction,
            order: SessionRecordOrder::OldestFirst,
            filter: SessionRecordFilter::default(),
            cursor: None,
            page_size: 200,
        })
        .unwrap();
    assert_eq!(raw.records.len(), 3);
    assert_eq!(
        unbind_success_counts(&h.path, &envelope.operation_id),
        (1, 1)
    );
    assert_eq!(h.gateway.count.load(Ordering::SeqCst), 0);
    acceptance_evidence::record(
        "schema-two-unbind-commit-compensation",
        &[
            ("V2-UNBIND-COMPENSATION", 1),
            ("STORAGE-20", 1),
            ("STORAGE-21", 1),
        ],
        serde_json::json!({"producer":"ApplicationRuntime + SqliteStore","hook":"controlled binding port; not a live game"}),
    );
}

fn mapping_game(path: &str, environment: &str) -> GameInput {
    GameInput {
        game_id: None,
        row_revision: None,
        display_name: "Reviewed mapping".into(),
        original_title: None,
        source_language: Some("ja".into()),
        cover_asset_ref: None,
        target: GameTarget {
            environment_kind: "crossover".into(),
            environment_key: environment.into(),
            bottle_locator: environment.strip_prefix("crossover:").map(str::to_owned),
            launch_path: Some(path.into()),
            canonical_exe_path: Some(path.into()),
            target_path_key: None,
            working_directory: None,
            arguments: vec![],
            architecture: Some("x86".into()),
            verification_state: "verified".into(),
        },
    }
}
#[tokio::test]
async fn association_mapping_preflight_rejects_wrong_library_identity_before_native_side_effects() {
    let h = Harness::new(4);
    for (path, env) in [
        ("C:\\Other.exe", "crossover:/bottles/runtime"),
        ("C:\\game.exe", "crossover:/bottles/other"),
    ] {
        let id = h
            .command(Command::SaveGame(Box::new(mapping_game(path, env))))
            .await
            .result_id
            .unwrap();
        let count = h.hook.associations.load(Ordering::SeqCst);
        let result = h.app.associate(AssociateRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            game_id: Some(id),
            candidate_id: "controlled-candidate".into(),
            confirm_switch: false,
        });
        assert_eq!(result.unwrap_err().code, "game_target_conflict");
        assert_eq!(h.hook.associations.load(Ordering::SeqCst), count);
        assert!(h.store.snapshot().unwrap().session.is_none());
    }
    let count = h.hook.associations.load(Ordering::SeqCst);
    let mut unresolved = mapping_game("C:\\UnreviewedOther.exe", "crossover:/bottles/runtime");
    unresolved.target.environment_kind = "unresolved".into();
    unresolved.target.environment_key.clear();
    unresolved.target.bottle_locator = None;
    unresolved.target.verification_state = "unresolved".into();
    let unresolved_id = h
        .command(Command::SaveGame(Box::new(unresolved)))
        .await
        .result_id
        .unwrap();
    assert_eq!(
        h.app
            .associate(AssociateRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                game_id: Some(unresolved_id),
                candidate_id: "controlled-candidate".into(),
                confirm_switch: false
            })
            .unwrap_err()
            .code,
        "game_target_conflict"
    );
    assert_eq!(h.hook.associations.load(Ordering::SeqCst), count);
    let mut wrong_arch = mapping_game("C:\\game.exe", "crossover:/bottles/runtime");
    wrong_arch.target.architecture = Some("x64".into());
    let wrong_arch_id = h
        .command(Command::SaveGame(Box::new(wrong_arch)))
        .await
        .result_id
        .unwrap();
    assert_eq!(
        h.app
            .associate(AssociateRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                game_id: Some(wrong_arch_id),
                candidate_id: "controlled-candidate".into(),
                confirm_switch: false
            })
            .unwrap_err()
            .code,
        "game_target_conflict"
    );
    assert_eq!(h.hook.associations.load(Ordering::SeqCst), count);
    assert_eq!(
        h.app
            .associate(AssociateRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                game_id: Some("missing-game".into()),
                candidate_id: "controlled-candidate".into(),
                confirm_switch: false
            })
            .unwrap_err()
            .code,
        "game_not_active"
    );
    assert_eq!(h.hook.associations.load(Ordering::SeqCst), count);
}

#[tokio::test]
async fn association_unknown_actual_uses_candidate_architecture_and_preserves_launcher() {
    let h = Harness::new(4);
    let mut input = mapping_game("C:\\game.exe", "crossover:/bottles/runtime");
    input.target.launch_path = Some("C:\\Launcher.exe".into());
    input.target.arguments = vec!["--safe-option".into()];
    input.target.canonical_exe_path = None;
    input.target.target_path_key = None;
    input.target.architecture = Some("x64".into());
    input.target.verification_state = "unresolved".into();
    let id = h
        .command(Command::SaveGame(Box::new(input)))
        .await
        .result_id
        .unwrap();
    let before = h.store.snapshot().unwrap();
    let original = before.games.iter().find(|game| game.game_id == id).unwrap();
    assert!(original.target.canonical_exe_path.is_none());
    assert_eq!(original.target.architecture.as_deref(), Some("x64"));
    h.app
        .associate(AssociateRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            game_id: Some(id.clone()),
            candidate_id: "controlled-candidate".into(),
            confirm_switch: false,
        })
        .unwrap();
    let after = h.store.snapshot().unwrap();
    let game = after.games.iter().find(|game| game.game_id == id).unwrap();
    assert_eq!(game.target.architecture.as_deref(), Some("x86"));
    assert_eq!(
        game.target.canonical_exe_path.as_deref(),
        Some("C:\\game.exe")
    );
    assert_eq!(game.target.target_path_key.as_deref(), Some("c:\\game.exe"));
    assert_eq!(game.target.launch_path.as_deref(), Some("C:\\Launcher.exe"));
    assert_eq!(game.target.arguments, ["--safe-option"]);
    assert_eq!(game.target.verification_state, "verified");
    assert_eq!(game.row_revision, original.row_revision + 1);
    assert_eq!(after.session.as_ref().unwrap().game_id, Some(id.clone()));
    assert_eq!(*h.hook.attribution.lock().unwrap(), Some(id));
    assert_eq!(h.hook.associations.load(Ordering::SeqCst), 1);
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn association_mapping_null_supplement_commits_atomically_and_keeps_old_language_and_sources()
{
    let h = Harness::new(4);
    let id = h
        .command(Command::SaveGame(Box::new(mapping_game(
            "C:\\game.exe",
            "crossover:/bottles/runtime",
        ))))
        .await
        .result_id
        .unwrap();
    h.associate();
    for sequence in 1..=2 {
        let HookUpdate::Text(mut input) = hook_text(sequence, "保留原句。") else {
            panic!("expected text")
        };
        input.source_language = "zh-CN".into();
        h.store
            .receive_hook_text(*input, 100 + i64::from(sequence))
            .unwrap();
        if sequence == 1 {
            h.command(Command::SelectSource {
                source_id: "controlled-source".into(),
            })
            .await;
        }
    }
    let before = h.store.snapshot().unwrap();
    let original = before
        .current
        .clone()
        .expect("actual selected source produced a unit");
    // The existing game-less capture deliberately has an unknown language;
    // supplementing a Japanese library record must not rewrite that snapshot.
    assert_eq!(original.source_language, "und");
    assert!(original.game_id.is_none());
    let conn = rusqlite::Connection::open(&h.path).unwrap();
    let revisions = || {
        let mut query = conn.prepare("SELECT unit_id,text_revision,source_text,source_sha256,source_language,processing_snapshot_json FROM reading_unit_revisions ORDER BY unit_id,text_revision").unwrap();
        let rows = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .unwrap();
        rows.collect::<Result<Vec<_>, _>>().unwrap()
    };
    let immutable_before = revisions();
    conn.execute_batch("CREATE TRIGGER mapping_commit_fault BEFORE UPDATE OF game_id ON sessions BEGIN SELECT RAISE(ABORT,'controlled mapping commit failure'); END;").unwrap();
    let request = || AssociateRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        game_id: Some(id.clone()),
        candidate_id: "controlled-candidate".into(),
        confirm_switch: false,
    };
    let disconnects = h.hook.disconnects.load(Ordering::SeqCst);
    assert!(h.app.associate(request()).is_err());
    assert_eq!(
        h.hook.disconnects.load(Ordering::SeqCst),
        disconnects,
        "SQL failure must retain the prior connection"
    );
    assert!(h.hook.attribution.lock().unwrap().is_none());
    assert!(
        h.store
            .snapshot()
            .unwrap()
            .session
            .unwrap()
            .game_id
            .is_none()
    );
    assert_eq!(h.store.snapshot().unwrap().current.unwrap(), original);
    conn.execute_batch("DROP TRIGGER mapping_commit_fault")
        .unwrap();
    h.app.associate(request()).unwrap();
    let after = h.store.snapshot().unwrap();
    assert_eq!(
        after.session.as_ref().unwrap().session_id,
        before.session.as_ref().unwrap().session_id
    );
    assert_eq!(
        after.session.as_ref().unwrap().game_id.as_deref(),
        Some(id.as_str())
    );
    assert_eq!(
        after.session.as_ref().unwrap().sources,
        before.session.as_ref().unwrap().sources
    );
    assert_eq!(
        revisions(),
        immutable_before,
        "no existing immutable reading revision is rewritten"
    );
    // Game ownership is a session JOIN in the approved schema, while the
    // revision's text/language/identity and all attached results remain fixed.
    let mut expected_projection = original;
    expected_projection.game_id = Some(id.clone());
    expected_projection.source_summary.as_mut().unwrap().game_id = Some(id.clone());
    assert_eq!(after.current.unwrap(), expected_projection);
    assert_eq!(*h.hook.attribution.lock().unwrap(), Some(id.clone()));
    let mut other = mapping_game("C:\\game.exe", "crossover:/bottles/runtime");
    other.target.environment_kind = "unresolved".into();
    other.target.environment_key.clear();
    other.target.bottle_locator = None;
    other.target.canonical_exe_path = None;
    other.target.verification_state = "unresolved".into();
    let other_id = h
        .command(Command::SaveGame(Box::new(other)))
        .await
        .result_id
        .unwrap();
    assert_ne!(other_id, id);
    assert_eq!(
        h.app
            .associate(AssociateRequest {
                game_id: Some(other_id),
                ..request()
            })
            .unwrap_err()
            .code,
        "session_game_conflict"
    );
    assert_eq!(
        h.store.snapshot().unwrap().session.unwrap().game_id,
        Some(id)
    );
    assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), disconnects);
}

// Candidate layered regression coverage: ApplicationRuntime FIFO and Repository/SQLite.
// The third case calls the production Repository directly; the other cases use Runtime FIFO.
// These controlled-port tests do not establish packaged game acceptance.
fn durable_operation_stop_rows(
    database: &rusqlite::Connection,
    session: &str,
) -> Vec<(String, String, Option<i64>)> {
    let mut query = database
        .prepare("SELECT operation_id,state,ended_at_ms FROM capture_operations WHERE session_id=?1 ORDER BY operation_id")
        .unwrap();
    query
        .query_map([session], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableOperationStopRow {
    operation_id: String,
    request_sha256: String,
    session_id: Option<String>,
    state: String,
    started_at_ms: i64,
    ended_at_ms: Option<i64>,
    legacy_unknown_start: i64,
}

fn durable_operation_stop_all_rows(
    database: &rusqlite::Connection,
) -> Vec<DurableOperationStopRow> {
    let mut query = database.prepare("SELECT operation_id,request_sha256,session_id,state,started_at_ms,ended_at_ms,legacy_unknown_start FROM capture_operations ORDER BY operation_id").unwrap();
    query
        .query_map([], |row| {
            Ok(DurableOperationStopRow {
                operation_id: row.get(0)?,
                request_sha256: row.get(1)?,
                session_id: row.get(2)?,
                state: row.get(3)?,
                started_at_ms: row.get(4)?,
                ended_at_ms: row.get(5)?,
                legacy_unknown_start: row.get(6)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn durable_operation_stop_revision(database: &rusqlite::Connection) -> i64 {
    database
        .query_row(
            "SELECT commit_revision FROM profile_meta WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn natural_exit_ack_requires_all_current_session_operations_stopped() {
    // Both production routes must close the same active-only operation set.
    for natural_exit in [true, false] {
        let h = Harness::new(4);
        h.select_hook().await;
        // Same-target reuse creates a distinct durable operation for this SID.
        h.associate();
        let database = rusqlite::Connection::open(&h.path).unwrap();
        let session = hook_session().session_id;
        let before = durable_operation_stop_rows(&database, &session);
        assert_eq!(before.len(), 2);
        assert!(
            before
                .iter()
                .all(|(_, state, ended)| state == "active" && ended.is_none())
        );
        // Historical marker and immutable identities are fixtures, not changed by the route under test.
        for (operation, state, ended, legacy) in [
            ("legacy-active", "active", None, 1),
            ("legacy-stopped", "stopped", Some(20_i64), 1),
            ("existing-failed", "failed", Some(21_i64), 0),
            ("legacy-recovered", "recovered", Some(22_i64), 1),
        ] {
            database
                .execute(
                    "INSERT INTO capture_operations VALUES(?1,?2,?3,?4,10,?5,?6)",
                    rusqlite::params![
                        operation,
                        content_hash(operation),
                        session,
                        state,
                        ended,
                        legacy
                    ],
                )
                .unwrap();
        }
        h.store
            .begin_capture_operation(
                "unbound-opening".into(),
                content_hash("unbound-opening-request"),
                30,
            )
            .unwrap();
        let all_before = durable_operation_stop_all_rows(&database);
        assert_eq!(
            all_before
                .iter()
                .filter(|row| row.session_id.as_deref() == Some(session.as_str())
                    && row.state == "active")
                .count(),
            3
        );
        assert_eq!(
            all_before
                .iter()
                .find(|row| row.operation_id == "legacy-active")
                .unwrap()
                .legacy_unknown_start,
            1
        );
        if natural_exit {
            h.events
                .send(lifecycle_control(
                    &session,
                    "process_disconnected",
                    "game_disconnected",
                ))
                .unwrap();
            wait(&h.app, |snapshot| {
                snapshot
                    .session
                    .as_ref()
                    .is_some_and(|s| s.state == "stopped")
                    && h.hook.natural_cleanup_committed.load(Ordering::SeqCst)
            })
            .await;
        } else {
            h.command(Command::Disconnect).await;
            assert!(!h.hook.natural_cleanup_committed.load(Ordering::SeqCst));
        }
        let ended: i64 = database
            .query_row(
                "SELECT ended_at_ms FROM sessions WHERE session_id=?1 AND state='stopped'",
                [&session],
                |row| row.get(0),
            )
            .unwrap();
        let all_after = durable_operation_stop_all_rows(&database);
        assert_eq!(all_after.len(), all_before.len());
        for before in &all_before {
            let after = all_after
                .iter()
                .find(|row| row.operation_id == before.operation_id)
                .unwrap();
            if before.session_id.as_deref() == Some(session.as_str()) && before.state == "active" {
                assert_eq!(
                    after.state, "stopped",
                    "natural={natural_exit}: current active operation must stop"
                );
                assert_eq!(
                    after.ended_at_ms,
                    Some(ended),
                    "natural={natural_exit}: operation and session must share terminal time"
                );
                let mut expected = before.clone();
                expected.state = "stopped".into();
                expected.ended_at_ms = Some(ended);
                assert_eq!(
                    after, &expected,
                    "operation identity and legacy_unknown_start must be preserved"
                );
            } else {
                assert_eq!(
                    after, before,
                    "natural={natural_exit}: terminal and unbound opening operations must remain byte-equivalent"
                );
            }
        }
        assert_eq!(h.hook.disconnects.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_drain_operation_write_failure_rolls_back_and_rejects_ack() {
    let h = Harness::new(4);
    h.select_hook().await;
    h.events.send(hook_text(2, "終了前の途中")).unwrap();
    wait(&h.app, |snapshot| {
        snapshot
            .current
            .as_ref()
            .is_some_and(|u| u.source_text == "終了前の途中")
    })
    .await;
    let session = hook_session().session_id;
    let database = rusqlite::Connection::open(&h.path).unwrap();
    let before = durable_operation_stop_rows(&database, &session);
    assert!(!before.is_empty());
    let revision_before = durable_operation_stop_revision(&database);
    database.execute_batch("CREATE TRIGGER controlled_operation_stop_failure BEFORE UPDATE OF state ON capture_operations WHEN NEW.state='stopped' BEGIN SELECT RAISE(ABORT,'controlled operation terminal unavailable'); END;").unwrap();
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::SessionDrainBarrier {
            session_id: session.clone(),
            acknowledged,
        })
        .unwrap();
    let result =
        tokio::task::spawn_blocking(move || consumed.recv_timeout(Duration::from_secs(3)).unwrap())
            .await
            .unwrap();
    assert_eq!(
        result.unwrap_err().code,
        "storage_failure",
        "failed operation persistence cannot receive a successful drain ACK"
    );
    let state: (String, Option<i64>) = database
        .query_row(
            "SELECT state,ended_at_ms FROM sessions WHERE session_id=?1",
            [&session],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        state,
        ("active".into(), None),
        "operation failure must roll back the session transition"
    );
    assert_eq!(durable_operation_stop_rows(&database, &session), before);
    assert_eq!(
        durable_operation_stop_revision(&database),
        revision_before,
        "failed terminal transaction must not commit a revision"
    );
    let unit_state: String = database.query_row("SELECT state FROM reading_units WHERE session_id=?1 ORDER BY unit_ordinal DESC LIMIT 1", [&session], |row| row.get(0)).unwrap();
    assert_eq!(
        unit_state, "open",
        "operation failure must roll back sealing from the same transaction"
    );
    database
        .execute_batch("DROP TRIGGER controlled_operation_stop_failure;")
        .unwrap();
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::SessionDrainBarrier {
            session_id: session.clone(),
            acknowledged,
        })
        .unwrap();
    tokio::task::spawn_blocking(move || {
        consumed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    let ended: i64 = database
        .query_row(
            "SELECT ended_at_ms FROM sessions WHERE session_id=?1 AND state='stopped'",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    let finished = durable_operation_stop_rows(&database, &session);
    assert_eq!(
        durable_operation_stop_revision(&database),
        revision_before + 1,
        "successful natural session barrier commits its session/operations/seal exactly once"
    );
    assert!(
        finished.iter().all(
            |(_, state, operation_ended)| state == "stopped" && *operation_ended == Some(ended)
        )
    );
    // Replay cannot change an already acknowledged terminal operation or its time.
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::SessionDrainBarrier {
            session_id: session.clone(),
            acknowledged,
        })
        .unwrap();
    tokio::task::spawn_blocking(move || {
        consumed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(durable_operation_stop_rows(&database, &session), finished);
    assert_eq!(
        durable_operation_stop_revision(&database),
        revision_before + 1,
        "duplicate natural session barrier must not commit a new revision"
    );
    let repeated_ended: i64 = database
        .query_row(
            "SELECT ended_at_ms FROM sessions WHERE session_id=?1 AND state='stopped'",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        repeated_ended, ended,
        "duplicate barrier must preserve session terminal time"
    );
}

#[tokio::test]
async fn scoped_terminal_diagnostic_finishes_operations_and_preserves_failed_terminal_state() {
    for code in ["capture_failed", "integrity_lost"] {
        let h = Harness::new(4);
        h.associate();
        let session = hook_session().session_id;
        let database = rusqlite::Connection::open_with_flags(
            &h.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let before = durable_operation_stop_rows(&database, &session);
        assert!(!before.is_empty());
        let diagnostic = |code: &str| Diagnostic {
            code: code.into(),
            stage: "capture".into(),
            message: "controlled lifecycle".into(),
            recoverable: true,
        };
        h.store
            .record_hook_diagnostic(None, diagnostic(code), 500)
            .unwrap();
        assert_eq!(
            durable_operation_stop_rows(&database, &session),
            before,
            "unscoped native lifecycle must never select the latest session"
        );
        h.store
            .record_hook_diagnostic(Some(session.clone()), diagnostic(code), 501)
            .unwrap();
        let failed = durable_operation_stop_rows(&database, &session);
        assert!(
            failed
                .iter()
                .all(|(_, state, ended)| state == "failed" && *ended == Some(501))
        );
        h.store
            .record_hook_diagnostic(Some(session.clone()), diagnostic("game_disconnected"), 502)
            .unwrap();
        h.store
            .record_hook_diagnostic(Some(session.clone()), diagnostic("capture_connected"), 503)
            .unwrap();
        assert_eq!(
            durable_operation_stop_rows(&database, &session),
            failed,
            "late disconnect/connected facts cannot rewrite failed terminal operations"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_session_barrier_never_finishes_a_new_sessions_operation() {
    let h = Harness::new(4);
    h.select_hook().await;
    let old = hook_session().session_id;
    let mut next = hook_session();
    next.session_id = "durable-operation-next-session".into();
    next.broker_instance_id = "durable-operation-next-broker".into();
    next.windows_pid = 101;
    next.process_created_u64 = "101".into();
    next.target_identity_sha256 = content_hash("durable-operation-next-target");
    h.store
        .begin_capture_operation(
            "durable-operation-next".into(),
            content_hash("durable-operation-next-request"),
            599,
        )
        .unwrap();
    h.store.open_hook_session(next.clone(), 600).unwrap();
    h.store
        .finish_capture_operation(
            "durable-operation-next".into(),
            Some(next.session_id.clone()),
            601,
        )
        .unwrap();
    let database =
        rusqlite::Connection::open_with_flags(&h.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let before = durable_operation_stop_rows(&database, &next.session_id);
    assert_eq!(
        before,
        vec![("durable-operation-next".into(), "active".into(), None)]
    );
    let (acknowledged, consumed) = mpsc::sync_channel(1);
    h.events
        .send(HookUpdate::SessionDrainBarrier {
            session_id: old,
            acknowledged,
        })
        .unwrap();
    tokio::task::spawn_blocking(move || {
        consumed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        durable_operation_stop_rows(&database, &next.session_id),
        before
    );
    let state: (String, Option<i64>) = database
        .query_row(
            "SELECT state,ended_at_ms FROM sessions WHERE session_id=?1",
            [&next.session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, ("opening".into(), None));
}

// Real producer for the old-history C17 path. No terminal status or display
// timestamp is fabricated with SQL: admissions and endings use production APIs.
fn reading_fixture_manual(h: &Harness, name: &str, text: &str, now: i64) -> String {
    h.store
        .dispatch(
            CommandEnvelope {
                operation_id: name.into(),
                expected_revision: None,
                command: Command::ManualInput {
                    text: text.into(),
                    source_language: "ja".into(),
                },
            },
            now,
        )
        .unwrap()
        .result_id
        .unwrap()
}
fn reading_fixture_counts(h: &Harness) -> (i64, i64) {
    let connection =
        rusqlite::Connection::open_with_flags(&h.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    connection
        .query_row(
            "SELECT (SELECT count(*) FROM request_tasks),(SELECT count(*) FROM request_attempts)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}
fn reading_fixture_current(projection: &ReadingUnitProjection) -> &TaskView {
    assert_eq!(projection.translation_state.engines.len(), 1);
    match &projection.translation_state.engines[0].status {
        TranslationEngineStatus::Current { task } => task,
        other => panic!("expected one true current task: {other:?}"),
    }
}
async fn reading_fixture_wait(
    app: &Arc<ApplicationRuntime>,
    unit: &str,
    task: &str,
    status: &str,
) -> ReadingUnitProjection {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let projection = app.reading_unit_projection(unit.into(), 1).unwrap();
            if let TranslationEngineStatus::Current { task: actual } =
                &projection.translation_state.engines[0].status
                && actual.task_id == task
                && actual.status == status
            {
                return projection;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("real fixed-object status did not settle")
}
fn reading_fixture_no_live_state(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                assert!(
                    ![
                        "translationState",
                        "isCurrentForReading",
                        "attemptId",
                        "partialText"
                    ]
                    .contains(&key.as_str()),
                    "live status must not be persisted into a fixed unit: {key}"
                );
                reading_fixture_no_live_state(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                reading_fixture_no_live_state(value);
            }
        }
        _ => {}
    }
}
// Compatibility/fault admission models an old binary in this isolated fixture
// DB only. It inserts legal queued/started facts, advances to running through
// the real trigger, and leaves all terminal transitions to fail_task.
fn reading_fixture_old_writer(
    h: &Harness,
    template: &TaskDispatch,
    name: &str,
    metadata: Option<serde_json::Value>,
) -> TaskDispatch {
    let mut task = template.clone();
    task.task_id = format!("reading-compat-task-{name}");
    task.attempt_id = format!("reading-compat-attempt-{name}");
    let mut connection = rusqlite::Connection::open(&h.path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    let raw: String = connection
        .query_row(
            "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
            [&template.task_id],
            |row| row.get(0),
        )
        .unwrap();
    let mut input: serde_json::Value = serde_json::from_str(&raw).unwrap();
    input
        .as_object_mut()
        .unwrap()
        .remove("translationStatusLineage");
    input["operationId"] = serde_json::json!(format!("reading-compat-operation-{name}"));
    if let Some(metadata) = metadata {
        input["translationStatusLineage"] = metadata;
    }
    let transaction = connection.transaction().unwrap();
    transaction.execute("INSERT INTO request_tasks(task_id,unit_id,text_revision,session_id,scope_key,purpose,config_revision_id,binding_id,engine_id,profile_generation,server_instance_id,authorization_epoch,request_semantics_sha256,input_snapshot_json,state,invalidation_code,created_at_ms,finished_at_ms) SELECT ?1,unit_id,text_revision,session_id,scope_key,purpose,config_revision_id,binding_id,engine_id,profile_generation,server_instance_id,authorization_epoch,request_semantics_sha256,?2,'queued',NULL,4000,NULL FROM request_tasks WHERE task_id=?3", rusqlite::params![task.task_id,input.to_string(),template.task_id]).unwrap();
    transaction.execute("INSERT INTO request_attempts(attempt_id,task_id,attempt_number,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,dispatch_kind,state,error_code,finish_reason,usage_json,started_at_ms,finished_at_ms) SELECT ?1,?2,1,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,'initial','started',NULL,NULL,'{}',4000,NULL FROM request_attempts WHERE attempt_id=?3", rusqlite::params![task.attempt_id,task.task_id,template.attempt_id]).unwrap();
    transaction
        .execute(
            "UPDATE request_tasks SET state='running' WHERE task_id=?1",
            [&task.task_id],
        )
        .unwrap();
    transaction.commit().unwrap();
    task
}
fn reading_fixture_stage(
    name: &str,
    h: &Harness,
    projection: ReadingUnitProjection,
    old_success: &str,
    must_be_hidden: bool,
) -> serde_json::Value {
    let snapshot = h.app.snapshot().unwrap();
    let task = match &projection.translation_state.engines[0].status {
        TranslationEngineStatus::Current { task } => Some(task),
        TranslationEngineStatus::Unavailable { .. } => None,
        other => panic!("unexpected fixture stage: {other:?}"),
    };
    assert!(
        snapshot
            .tasks
            .iter()
            .any(|task| task.task_id == old_success),
        "original success must remain in the bounded task history"
    );
    if let Some(task) = task {
        assert_eq!(
            snapshot
                .tasks
                .iter()
                .any(|view| view.task_id == task.task_id),
            !must_be_hidden,
            "stage {name} must actually exercise the task window"
        );
    }
    assert_ne!(
        snapshot.current.as_ref().unwrap().unit_id,
        projection.unit.unit_id
    );
    assert!(
        !snapshot
            .history
            .units
            .iter()
            .any(|unit| unit.unit_id == projection.unit.unit_id)
    );
    assert!(snapshot.commit_revision >= projection.commit_revision);
    serde_json::json!({"name":name,"snapshot":snapshot,"unit":projection.unit,"projection":projection})
}
async fn reading_status_history_fixture() -> serde_json::Value {
    let mut h = Harness::new(4);
    h.clock.1.store(true, Ordering::SeqCst);
    let target = reading_fixture_manual(
        &h,
        "reading-history-target",
        "古い履歴の翻訳状態を読む。",
        200,
    );
    // The successful historical request has a high wall clock. Explicit later
    // requests run after a clock rollback, proving timestamps are not causality.
    h.clock.0.store(1_000_000, Ordering::SeqCst);
    let old_receipts = h
        .app
        .retranslate(target.clone(), 1, "reading-history-old-success".into())
        .unwrap();
    let old_call = h.call().await;
    assert_eq!(old_receipts[0].task_id, old_call.task.task_id);
    old_call.reply.send("保留的旧成功译文。".into()).unwrap();
    reading_fixture_wait(&h.app, &target, &old_receipts[0].task_id, "completed").await;
    let old_success = old_receipts[0].task_id.clone();
    let mut units = vec![];
    for n in 0..61 {
        units.push(reading_fixture_manual(
            &h,
            &format!("reading-history-newer-unit-{n}"),
            &format!("新しい履歴の句 {n}。"),
            10_000 + n,
        ));
    }
    let mut latest_dispatches = BTreeMap::new();
    for n in 0..101 {
        let unit = &units[n % units.len()];
        let prepared = h
            .store
            .prepare_uncached_translation_for_operation(
                unit.clone(),
                1,
                format!("reading-history-terminal-{n}"),
                800_000 + n as i64,
            )
            .unwrap();
        let task = prepared.dispatches.into_iter().next().unwrap();
        h.store
            .fail_task(task.task_id.clone(), "timeout".into(), 801_000 + n as i64)
            .unwrap();
        latest_dispatches.insert(unit.clone(), task);
    }
    let initial_snapshot = h.app.snapshot().unwrap();
    assert_eq!(initial_snapshot.tasks.len(), 100);
    assert!(
        initial_snapshot
            .tasks
            .iter()
            .any(|task| task.task_id == old_success)
    );
    assert_ne!(initial_snapshot.current.as_ref().unwrap().unit_id, target);
    assert!(
        !initial_snapshot
            .history
            .units
            .iter()
            .any(|unit| unit.unit_id == target)
    );
    let first_query = HistoryQuery {
        filter: HistoryFilter::default(),
        cursor: None,
        limit: 50,
    };
    let first_page = h.app.history_page(first_query.clone()).unwrap();
    assert_eq!(first_page.units.len(), 50);
    assert!(!first_page.units.iter().any(|unit| unit.unit_id == target));
    let next_query = HistoryQuery {
        filter: HistoryFilter::default(),
        cursor: Some(
            first_page
                .next_cursor
                .clone()
                .expect("actual opaque next cursor"),
        ),
        limit: 50,
    };
    let next_page = h.app.history_page(next_query.clone()).unwrap();
    assert_eq!(next_page.units.len(), 12);
    let page_target = next_page
        .units
        .iter()
        .find(|unit| unit.unit_id == target)
        .expect("target must be on the actual second history page");
    let selected_unit = h.app.unit(target.clone(), 1).unwrap();
    assert_eq!(&selected_unit, page_target);
    assert_eq!(selected_unit.translations.len(), 1);
    assert_eq!(selected_unit.translations[0].task_id, old_success);
    let serialized_unit = serde_json::to_string(&selected_unit).unwrap();
    reading_fixture_no_live_state(&serde_json::to_value(&selected_unit).unwrap());
    let frozen = h.app.freeze_learning(target.clone(), 1, None).unwrap();
    let serialized_frozen = serde_json::to_string(&frozen).unwrap();
    reading_fixture_no_live_state(&serde_json::to_value(&frozen).unwrap());
    let first_projection = h.app.reading_unit_projection(target.clone(), 1).unwrap();
    assert_eq!(first_projection.unit, selected_unit);
    assert_eq!(
        reading_fixture_current(&first_projection).status,
        "completed"
    );
    let healthy_before = h.app.reading_unit_projection(units[0].clone(), 1).unwrap();

    h.clock.0.store(1000, Ordering::SeqCst);
    let failed_receipts = h
        .app
        .retranslate(target.clone(), 1, "reading-history-explicit-failed".into())
        .unwrap();
    let failed_call = h.call().await;
    assert_eq!(failed_call.task.task_id, failed_receipts[0].task_id);
    failed_call
        .failure
        .send(AppError::new(
            "connection_failed",
            "controlled",
            "受控连接失败：这是后续页旧句的真实失败请求",
        ))
        .unwrap();
    let failed = reading_fixture_wait(&h.app, &target, &failed_receipts[0].task_id, "failed").await;
    assert_eq!(
        reading_fixture_current(&failed).error_code.as_deref(),
        Some("connection_failed")
    );
    let mut hidden_failed = reading_fixture_stage("hidden_failed", &h, failed, &old_success, true);
    hidden_failed["receipts"] = serde_json::json!(failed_receipts);

    h.clock.0.store(2000, Ordering::SeqCst);
    let running_receipts = h
        .app
        .retranslate(
            target.clone(),
            1,
            "reading-history-explicit-cancelled".into(),
        )
        .unwrap();
    let running_call = h.call().await;
    let running =
        reading_fixture_wait(&h.app, &target, &running_receipts[0].task_id, "running").await;
    let mut new_running = reading_fixture_stage("new_running", &h, running, &old_success, false);
    new_running["receipts"] = serde_json::json!(running_receipts);
    let cancel_id = running_receipts[0].task_id.clone();
    let cancel_receipt = h
        .command(Command::CancelTask {
            task_id: cancel_id.clone(),
        })
        .await;
    assert_cancelled(running_call).await;
    let cancelled = reading_fixture_wait(&h.app, &target, &cancel_id, "cancelled").await;
    assert_eq!(
        reading_fixture_current(&cancelled).error_code.as_deref(),
        Some("user_cancelled")
    );
    let mut hidden_cancelled =
        reading_fixture_stage("hidden_cancelled", &h, cancelled, &old_success, true);
    hidden_cancelled["cancelTaskId"] = serde_json::json!(cancel_id);
    hidden_cancelled["cancelReceipt"] = serde_json::json!(cancel_receipt);

    h.clock.0.store(3000, Ordering::SeqCst);
    let recovered_receipts = h
        .app
        .retranslate(
            target.clone(),
            1,
            "reading-history-explicit-recovered".into(),
        )
        .unwrap();
    let recovered_call = h.call().await;
    let recovered_dispatch = recovered_call.task.clone();
    recovered_call
        .reply
        .send("明确重译后的新成功译文。".into())
        .unwrap();
    let recovered =
        reading_fixture_wait(&h.app, &target, &recovered_receipts[0].task_id, "completed").await;
    assert!(reading_fixture_current(&recovered).error_code.is_none());
    assert_eq!(recovered.unit.translations.len(), 2);
    assert_eq!(
        recovered
            .unit
            .translations
            .iter()
            .find(|translation| translation.task_id == old_success)
            .unwrap(),
        &selected_unit.translations[0]
    );
    let mut recovered_stage = reading_fixture_stage("recovered", &h, recovered, &old_success, true);
    recovered_stage["receipts"] = serde_json::json!(recovered_receipts);

    let bad = reading_fixture_old_writer(
        &h,
        &recovered_dispatch,
        "self-reference",
        Some(
            serde_json::json!({"schemaVersion":1,"supersedesTaskIds":["reading-compat-task-self-reference"]}),
        ),
    );
    h.store
        .fail_task(bad.task_id.clone(), "timeout".into(), 4001)
        .unwrap();
    let corrupt = h.app.reading_unit_projection(target.clone(), 1).unwrap();
    match &corrupt.translation_state.engines[0].status {
        TranslationEngineStatus::Unavailable { error } => {
            assert_eq!(error.code, "invalid_translation_status_lineage")
        }
        other => panic!("only the corrupt lane must be unavailable: {other:?}"),
    }
    let corrupt_stage = reading_fixture_stage("corrupt_lane", &h, corrupt, &old_success, true);
    assert!(
        !h.app
            .snapshot()
            .unwrap()
            .tasks
            .iter()
            .any(|task| task.task_id == bad.task_id)
    );
    let before_rejection = reading_fixture_counts(&h);
    let rejection = h
        .app
        .retranslate(
            target.clone(),
            1,
            "reading-history-rejected-admission".into(),
        )
        .unwrap_err();
    assert_eq!(rejection.code, "invalid_translation_status_lineage");
    let after_rejection = reading_fixture_counts(&h);
    assert_eq!(before_rejection, after_rejection);
    let healthy_after = h.app.reading_unit_projection(units[0].clone(), 1).unwrap();
    assert_eq!(healthy_after.unit, healthy_before.unit);
    assert_eq!(
        healthy_after.translation_state,
        healthy_before.translation_state
    );

    let unordered_unit = units[1].clone();
    let template = &latest_dispatches[&unordered_unit];
    for name in ["legacy-one", "legacy-two"] {
        let legacy = reading_fixture_old_writer(&h, template, name, None);
        h.store
            .fail_task(legacy.task_id, "connection_failed".into(), 4002)
            .unwrap();
    }
    let unordered = h
        .app
        .reading_unit_projection(unordered_unit.clone(), 1)
        .unwrap();
    match &unordered.translation_state.engines[0].status {
        TranslationEngineStatus::Unordered {
            summary,
            active_task,
        } => {
            assert_eq!(summary.head_count, "3");
            assert_eq!(summary.failed_count, "3");
            assert_eq!(summary.running_count, "0");
            assert!(active_task.is_none());
        }
        other => panic!("legacy unordered heads must stay explicit: {other:?}"),
    }
    let final_unit = h.app.unit(target.clone(), 1).unwrap();
    assert_eq!(selected_unit.source_hash, final_unit.source_hash);
    assert_eq!(selected_unit.source_text, final_unit.source_text);
    assert_eq!(
        serde_json::to_string(&selected_unit).unwrap(),
        serialized_unit
    );
    assert_eq!(
        serde_json::to_string(&h.app.snapshot().unwrap().learning_object.unwrap()).unwrap(),
        serialized_frozen
    );
    reading_fixture_no_live_state(&serde_json::to_value(&final_unit).unwrap());
    assert!(
        h.calls.try_recv().is_err(),
        "corrupt admission cannot schedule a wire request"
    );
    serde_json::json!({
        "schemaVersion":1,"kind":"actual_sqlite_reading_unit_projection_history",
        "sourceSha256": {
            "crates/app-infra/tests/application_runtime.rs":content_hash(include_str!("application_runtime.rs")),
            "crates/app-core/src/application/runtime.rs":content_hash(include_str!("../../app-core/src/application/runtime.rs")),
            "crates/app-core/src/contracts/desktop.rs":content_hash(include_str!("../../app-core/src/contracts/desktop.rs")),
            "crates/app-core/src/domain/translation_status.rs":content_hash(include_str!("../../app-core/src/domain/translation_status.rs")),
            "crates/app-infra/src/storage/database.rs":content_hash(include_str!("../src/storage/database.rs")),
            "crates/app-infra/src/storage/queries/mod.rs":content_hash(include_str!("../src/storage/queries/mod.rs")),
            "crates/app-infra/src/storage/queries/tasks.rs":content_hash(include_str!("../src/storage/queries/tasks.rs")),
            "crates/app-infra/src/storage/queries/translation_status.rs":content_hash(include_str!("../src/storage/queries/translation_status.rs")),
        },
        "initialSnapshot":initial_snapshot,"firstQuery":first_query,"firstPage":first_page,
        "nextQuery":next_query,"nextPage":next_page,"selectedUnit":selected_unit,"firstProjection":first_projection,
        "stages":[hidden_failed,new_running,hidden_cancelled,recovered_stage,corrupt_stage],
        "unordered":{"unit":unordered.unit,"snapshot":h.app.snapshot().unwrap(),"projection":unordered},
        "healthyUnit":{"unit":healthy_before.unit,"firstProjection":healthy_before,"afterProjection":healthy_after},
        "rejectedAdmission":Diagnostic::from(rejection),
        "rejectedAdmissionEvidence":{"operationId":"reading-history-rejected-admission","taskAttemptCountsBefore":before_rejection,"taskAttemptCountsAfter":after_rejection},
        "fixedObjectEvidence":{"serializedUnit":serialized_unit,"frozenLearningObject":frozen,"serializedFrozen":serialized_frozen,"finalUnit":final_unit},
    })
}
