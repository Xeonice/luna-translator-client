//! Actual Runtime + SQLite RPC fixture for browser component tests.
//! Hook callbacks, preview pixels and window effects are controlled low-level
//! ports. This target does not claim live Hook, ScreenCaptureKit or OS effects.
use app_core::contracts::window_capabilities::*;
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{self, TaskDispatch, capture::*},
    ports::{IdentitySource, ProductRepository, runtime::*},
};
use app_infra::storage::SqliteStore;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
#[allow(dead_code)]
#[path = "../../../apps/desktop/src-tauri/src/platform/window_preview/state.rs"]
mod production_preview_decisions;

const RPC_PREFIX: &str = "LUNA_BINDING_RPC_V1 ";

const SCENES: &[&str] = &[
    "BJS-01-library-window",
    "BJS-01-library-process",
    "BJS-01-library-global-owner",
    "BJS-01-scoped-reconnect-owner",
    "BJS-02-first-confirmation",
    "BJS-02-tcc",
    "BJS-03-pairing-transfer",
    "BJS-04-idle-refresh-details",
    "BJS-04-picker-cancel",
    "BJS-04-picker-timeout",
    "BJS-05-reuse",
    "BJS-05-switch-cancel",
    "BJS-05-stale-review",
    "BJS-06-real-stage-order",
    "BJS-06-stale-events",
    "BJS-07-cancel-candidate",
    "BJS-07-close-operation",
    "BJS-08-empty",
    "BJS-08-draft-later",
    "BJS-08-commit-set",
    "BJS-08-commit-failure",
    "BJS-09-owned-remember",
    "BJS-09-unowned-remember",
    "BJS-09-temporary-saved",
    "BJS-09-requires-review",
    "BJS-10-details",
    "BJS-13-effects-order",
    "BJS-13-effects-retry",
    "BJS-13-stale-effects",
    "BJS-14-layout-normal",
    "BJS-14-layout-narrow",
    "BJS-14-layout-200",
    "BJS-14-other-bottle-warning",
    "BJS-15-frame-state",
    "BJS-15-revoke",
    "BJS-15-late-metadata",
    "BJS-17-association-retry",
    "BJS-17-source-query-retry",
    "BJS-17-source-expired",
    "BJS-17-submit-retry",
];
const METHODS: &[&str] = &[
    "snapshot",
    "targets",
    "pickWindow",
    "reviewAssociation",
    "associate",
    "associationProgress",
    "sourceSelection",
    "beginReading",
    "retryReadingWindowEffects",
    "previewCapability",
    "startWindowPreview",
    "selectWindowPreview",
    "controlWindowPreview",
    "previewSession",
    "dispatch",
    "openWindow",
    "closeWindow",
    "readGameCover",
    "gameWindowBinding",
    "windowCapabilities",
    "resources",
];
fn bj_error(code: &str, message: &str) -> AppError {
    AppError::new(code, "controlled_fixture", message)
}
fn bj_value<T: Serialize>(value: T) -> AppResult<Value> {
    serde_json::to_value(value).map_err(|_| bj_error("fixture_serialization", "实际响应序列化失败"))
}
fn bj_arg<T: serde::de::DeserializeOwned>(args: &[Value], index: usize) -> AppResult<T> {
    serde_json::from_value(
        args.get(index)
            .cloned()
            .ok_or_else(|| bj_error("invalid_fixture_argument", "缺少实际请求参数"))?,
    )
    .map_err(|error| bj_error("invalid_fixture_argument", &error.to_string()))
}
fn bj_candidate(name: &str, pid: u32, window: bool) -> TargetCandidate {
    TargetCandidate {
        candidate_id: format!("candidate-{name}"),
        window_title: format!("実際の制御されたゲーム {name}"),
        executable_path: format!("C:\\Games\\{name}.exe"),
        windows_pid: pid,
        process_created_u64: "18446744073709551615".into(),
        architecture: "x64".into(),
        environment_key: "crossover:/controlled/bottle".into(),
        bottle_label: "controlled-bottle".into(),
        has_window: window,
        window_handle: window.then(|| pid.to_string()),
        macos_host_pid: None,
        macos_window_id: None,
    }
}
fn bj_session(
    candidate: &TargetCandidate,
    serial: u64,
    game_id: Option<String>,
) -> HookSessionInput {
    let identity = domain::content_hash(format!(
        "{}\0{}\0{}\0{}\0{}",
        candidate.environment_key,
        candidate.executable_path,
        candidate.windows_pid,
        candidate.process_created_u64,
        candidate.architecture
    ));
    HookSessionInput {
        session_id: format!("session-{}-{serial}", candidate.candidate_id),
        game_id,
        environment_key: candidate.environment_key.clone(),
        target_exe_path: candidate.executable_path.clone(),
        windows_pid: candidate.windows_pid,
        process_created_u64: candidate.process_created_u64.clone(),
        architecture: candidate.architecture.clone(),
        broker_instance_id: format!("broker-{}-{serial}", candidate.candidate_id),
        resource: ResourceSnapshotInput {
            snapshot_id: "fixture-core-resource".into(),
            kind: "core".into(),
            version: "controlled-native-port-v1".into(),
            manifest_sha256: domain::content_hash("controlled-native-port-v1"),
            contract_version: "abi1-jsonl2".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: identity,
    }
}
fn bj_game(candidate: &TargetCandidate) -> GameInput {
    GameInput {
        game_id: None,
        row_revision: None,
        display_name: format!("制御ゲーム {}", candidate.windows_pid),
        original_title: Some(candidate.window_title.clone()),
        source_language: Some("ja".into()),
        cover_asset_ref: None,
        target: GameTarget {
            environment_kind: "crossover".into(),
            environment_key: candidate.environment_key.clone(),
            bottle_locator: Some("/controlled/bottle".into()),
            launch_path: Some(candidate.executable_path.clone()),
            canonical_exe_path: Some(candidate.executable_path.clone()),
            target_path_key: None,
            working_directory: None,
            arguments: vec![],
            architecture: Some(candidate.architecture.clone()),
            verification_state: "verified".into(),
        },
    }
}
struct BjAux {
    ids: AtomicU64,
    now: AtomicI64,
}
impl BjAux {
    fn new() -> Self {
        Self {
            ids: AtomicU64::new(1),
            now: AtomicI64::new(1794234000000),
        }
    }
}
impl IdentitySource for BjAux {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", self.ids.fetch_add(1, Ordering::SeqCst))
    }
    fn now_ms(&self) -> i64 {
        self.now.fetch_add(1, Ordering::SeqCst)
    }
}
impl ModelGateway for BjAux {
    fn execute<'a>(
        &'a self,
        _t: &'a TaskDispatch,
        _p: &'a RequestPlan,
        _delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async {
            Err(bj_error(
                "fixture_models_disabled",
                "组件测试没有启用翻译服务",
            ))
        })
    }
    fn test_request<'a>(
        &'a self,
        _c: &'a Connection,
        _p: &'a RequestPlan,
        _r: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(bj_error("fixture_models_disabled", "未配置模型测试")) })
    }
    fn discover_models<'a>(
        &'a self,
        _c: &'a Connection,
        _r: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Ok(vec![]) })
    }
}
impl LanguageServices for BjAux {
    fn analyze<'a>(&'a self, _text: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn lookup(&self, _token: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        Ok(vec![])
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![]
    }
}
impl CredentialVault for BjAux {
    fn put(&self, _id: &str, _secret: &[u8]) -> AppResult<()> {
        Err(bj_error("fixture_vault_disabled", "未启用凭据"))
    }
    fn get(&self, _id: &str) -> AppResult<Vec<u8>> {
        Err(bj_error("fixture_vault_disabled", "未启用凭据"))
    }
    fn delete(&self, _id: &str) -> AppResult<()> {
        Err(bj_error("fixture_vault_disabled", "未启用凭据"))
    }
}
struct BjHook {
    candidates: Vec<TargetCandidate>,
    active: Mutex<Option<HookSessionInput>>,
    lease: Mutex<()>,
    serial: AtomicU64,
    fail_associations: AtomicU64,
    resource_path: Mutex<Option<PathBuf>>,
    target_lease_path: Mutex<Option<PathBuf>>,
    sender: mpsc::Sender<HookUpdate>,
    receiver: Mutex<Option<mpsc::Receiver<HookUpdate>>>,
}
impl BjHook {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            candidates: vec![
                bj_candidate("main", 100, true),
                bj_candidate("other", 200, true),
                bj_candidate("process", 300, false),
            ],
            active: Mutex::new(None),
            lease: Mutex::new(()),
            serial: AtomicU64::new(1),
            fail_associations: AtomicU64::new(0),
            resource_path: Mutex::new(None),
            target_lease_path: Mutex::new(None),
            sender,
            receiver: Mutex::new(Some(receiver)),
        }
    }
    fn active(&self) -> AppResult<HookSessionInput> {
        self.active
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| bj_error("fixture_no_active", "没有活动宿主"))
    }
    fn event(&self, clock: &BjAux, code: &str, stage: &str) -> AppResult<()> {
        let current = self.active()?;
        self.sender
            .send(HookUpdate::Control {
                event: Box::new(HookEventInput {
                    event_id: clock.new_id("native-event"),
                    operation_id: None,
                    session_id: Some(current.session_id),
                    source: None,
                    wire_sequence: None,
                    observed_at_ms: clock.now_ms(),
                    detail: HookStageInput {
                        level: "info".into(),
                        stage: stage.into(),
                        code: code.into(),
                        message: format!("受控 Core 端口事实：{code}"),
                    },
                    lost_count: None,
                    integrity: "intact".into(),
                }),
                diagnostic: None,
            })
            .map_err(|_| bj_error("fixture_channel_closed", "Core通道已关闭"))
    }
    fn text(&self, clock: &BjAux, label: &str) -> AppResult<()> {
        let current = self.active()?;
        let seq = clock.ids.fetch_add(1, Ordering::SeqCst);
        let source_id = domain::content_hash(format!("{}:{label}", current.broker_instance_id));
        let input = HookTextInput {
            session_id: current.session_id,
            event_id: clock.new_id("native-text"),
            sequence_u64: seq.to_string(),
            source: HookSourceInput {
                source_id,
                host_instance_id: current.broker_instance_id,
                hook_address_u64: if label == "A" {
                    "18446744073709551615"
                } else {
                    "9007199254740993"
                }
                .into(),
                context_u64: "9007199254740993".into(),
                subcontext_u64: "1".into(),
                hook_code: format!("HWN-{label}@game"),
                hook_name: format!("TextOutW {label}"),
                encoding: "utf-16".into(),
                stream_identity_sha256: domain::content_hash(format!("{label}-stream")),
                identity_extensions_json: "{}".into(),
            },
            text: format!(
                "{label}：春の空を薄紅に染める。{}",
                "完全な原文のサンプル。".repeat(24)
            ),
            source_language: "ja".into(),
        };
        self.sender
            .send(HookUpdate::Text(Box::new(input)))
            .map_err(|_| bj_error("fixture_channel_closed", "Core通道已关闭"))
    }
    fn drain(&self) -> AppResult<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .send(HookUpdate::DrainBarrier { acknowledged: tx })
            .map_err(|_| bj_error("fixture_channel_closed", "Core通道已关闭"))?;
        rx.recv_timeout(Duration::from_secs(3))
            .map_err(|_| bj_error("fixture_drain_timeout", "实际消费未完成"))?
    }
}
impl HookBackend for BjHook {
    fn targets(&self) -> AppResult<TargetCandidates> {
        Ok(TargetCandidates {
            candidates: self.candidates.clone(),
            diagnostics: vec![Diagnostic {
                code: "unrelated_bottle_unavailable".into(),
                stage: "discovery".into(),
                message: "其他 Bottle 容器暂时无法枚举窗口；当前目标未受影响".into(),
                recoverable: true,
            }],
        })
    }
    fn association_candidate(&self, id: &str) -> AppResult<TargetCandidate> {
        self.candidates
            .iter()
            .find(|c| c.candidate_id == id)
            .cloned()
            .ok_or_else(|| bj_error("stale_target", "候选已经失效"))
    }
    fn association_context(&self, id: &str) -> AppResult<AssociationBackendContext> {
        let _l = self.lease.lock().unwrap();
        Ok(AssociationBackendContext {
            candidate: self.association_candidate(id)?,
            active: self.active.lock().unwrap().clone(),
        })
    }
    fn active_session_context(&self) -> AppResult<Option<HookSessionInput>> {
        Ok(self.active.lock().unwrap().clone())
    }
    fn with_active_session_guard(
        &self,
        expected: &HookSessionInput,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _l = self.lease.lock().unwrap();
        let current = self.active()?;
        if !same_hook_session_facts(&current, expected) {
            return Err(bj_error("reading_effect_stale", "宿主身份已经变化"));
        }
        effect()
    }
    fn with_no_active_session_guard(
        &self,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _lease = self.lease.lock().unwrap();
        if self.active.lock().unwrap().is_some() {
            return Err(bj_error("inactive_session_preview_stale", "实际宿主已建立"));
        }
        effect()
    }
    fn with_associated_session_commit(
        &self,
        input: &HookSessionInput,
        commit: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _l = self.lease.lock().unwrap();
        let mut active = self.active.lock().unwrap();
        let current = active
            .as_mut()
            .ok_or_else(|| bj_error("stale_session", "没有实际会话"))?;
        if current.session_id != input.session_id
            || current.target_identity_sha256 != input.target_identity_sha256
        {
            return Err(bj_error("stale_session", "会话身份已经变化"));
        }
        commit()?;
        if current.game_id.is_none() {
            current.game_id = input.game_id.clone();
        }
        Ok(())
    }
    fn associate(&self, _request: AssociateRequest) -> AppResult<HookSessionInput> {
        Err(bj_error("reviewed_association_required", "使用已核对目标"))
    }
    fn associate_reviewed_traced(
        &self,
        request: AssociateRequest,
        guard: &AssociationExecutionGuard,
        record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        let _l = self.lease.lock().unwrap();
        let candidate = self.association_candidate(&request.candidate_id)?;
        let mut active = self.active.lock().unwrap();
        let full_matches = match (guard.expected_active.as_ref(), active.as_ref()) {
            (Some(expected), Some(current)) => same_hook_session_facts(expected, current),
            (None, None) => true,
            _ => false,
        };
        if !full_matches
            || guard.expected_active_session_id.as_ref() != active.as_ref().map(|s| &s.session_id)
            || guard.expected_active_target_identity_sha256.as_ref()
                != active.as_ref().map(|s| &s.target_identity_sha256)
            || guard.expected_broker_instance_id.as_ref()
                != active.as_ref().map(|s| &s.broker_instance_id)
        {
            return Err(bj_error(
                "association_review_stale",
                "最终租约中的活动身份已变化",
            ));
        }
        let proposed = bj_session(
            &candidate,
            self.serial.load(Ordering::SeqCst),
            request.game_id.clone(),
        );
        if proposed.target_identity_sha256 != guard.candidate_identity_sha256 {
            return Err(bj_error("association_review_stale", "最终目标已经变化"));
        }
        if let Some(current) = active
            .as_ref()
            .filter(|s| s.target_identity_sha256 == proposed.target_identity_sha256)
        {
            let mut retained = current.clone();
            if retained.game_id.is_none() {
                retained.game_id = request.game_id;
            }
            record(HookStageInput {
                level: "info".into(),
                stage: "association".into(),
                code: "association_reused".into(),
                message: "受控宿主保留原会话，没有再次注入".into(),
            })?;
            return Ok(retained);
        }
        record(HookStageInput {
            level: "info".into(),
            stage: "resource".into(),
            code: "resource_check_started".into(),
            message: "受控资源端口开始核验".into(),
        })?;
        if let Some(path) = self.resource_path.lock().unwrap().as_ref() {
            std::fs::read(path)
                .map_err(|_| bj_error("resource_unavailable", "实际受控资源文件不可读"))?;
        }
        let target_lease = self.target_lease_path.lock().unwrap().clone();
        let _target_reservation = if let Some(path) = target_lease {
            let reservation = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .map_err(|_| bj_error("target_lease_unavailable", "实际目标租约文件不可用"))?;
            reservation
                .try_lock()
                .map_err(|_| bj_error("target_owned_elsewhere", "实际目标租约已被占用"))?;
            Some(reservation)
        } else {
            None
        };
        if self
            .fail_associations
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n > 0).then(|| n - 1)
            })
            .is_ok()
        {
            return Err(bj_error(
                "controlled_hook_start_failed",
                "受控 Hook 启动失败，可重试",
            ));
        }
        record(HookStageInput {
            level: "info".into(),
            stage: "injection".into(),
            code: "injection_requested".into(),
            message: "实际调用受控宿主启动端口，尚无握手".into(),
        })?;
        self.serial.fetch_add(1, Ordering::SeqCst);
        *active = Some(proposed.clone());
        Ok(proposed)
    }
    fn disconnect(&self) -> AppResult<()> {
        *self.active.lock().unwrap() = None;
        Ok(())
    }
    fn stop_and_drain(&self, acknowledged: mpsc::SyncSender<AppResult<()>>) -> AppResult<()> {
        self.disconnect()?;
        self.sender
            .send(HookUpdate::DrainBarrier { acknowledged })
            .map_err(|_| bj_error("fixture_channel_closed", "实际Core消费通道已关闭"))
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        self.receiver
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| bj_error("fixture_receiver_taken", "接收者已被获取"))
    }
    fn resource_info(&self) -> ResourceInfo {
        ResourceInfo {
            kind: "core".into(),
            version: "controlled-native-port-v1".into(),
            manifest_sha256: domain::content_hash("controlled-native-port-v1"),
            available: true,
            detail: "受控 Core 端口，不是真实游戏Hook验收".into(),
        }
    }
}

struct BjFixture {
    scene: String,
    path: PathBuf,
    store: Arc<SqliteStore>,
    app: Arc<ApplicationRuntime>,
    hook: Arc<BjHook>,
    clock: Arc<BjAux>,
    handle: tokio::runtime::Handle,
    preview: Mutex<Option<PreviewProjection>>,
    previous_candidate: Mutex<Option<PreviewProjection>>,
    frame: Mutex<Option<PreviewFrame>>,
    frame_sequence: AtomicU64,
    operations: Mutex<Vec<String>>,
    events: Mutex<Vec<Value>>,
    fail_effects: AtomicU64,
    source_query_failures: AtomicU64,
    last_effect: Mutex<Option<ReadingWindowEffectAuthority>>,
    saved_before: Mutex<Option<String>>,
    published: AtomicU64,
    seed_commands: Vec<Value>,
}
impl BjFixture {
    fn new(scene: &str, handle: tokio::runtime::Handle) -> AppResult<Self> {
        let base = std::env::var_os("LUNA_BINDING_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Self::new_in(scene, handle, &base)
    }
    fn new_in(
        scene: &str,
        handle: tokio::runtime::Handle,
        base: &std::path::Path,
    ) -> AppResult<Self> {
        let directory = base.join(format!("bj-rpc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory)
            .map_err(|_| bj_error("fixture_directory", "无法创建独立测试目录"))?;
        let path = directory.join("profile.sqlite3");
        let store = Arc::new(SqliteStore::open(
            &path,
            "binding-rpc-profile",
            "binding-rpc-server",
            1,
        )?);
        let clock = Arc::new(BjAux::new());
        let hook = Arc::new(BjHook::new());
        let unowned = scene == "BJS-09-unowned-remember";
        let mut seed_commands = vec![];
        if !unowned {
            let request = CommandEnvelope {
                operation_id: "seed-library-games".into(),
                expected_revision: Some(store.snapshot()?.commit_revision),
                command: Command::SaveGames {
                    items: hook
                        .candidates
                        .iter()
                        .take(2)
                        .enumerate()
                        .map(|(index, candidate)| GameBatchItem {
                            item_id: format!("seed-item-{index}"),
                            game: bj_game(candidate),
                        })
                        .collect(),
                },
            };
            let response = store.dispatch(request.clone(), clock.now_ms())?;
            seed_commands.push(json!({"request":request,"response":response}));
        }
        let app = ApplicationRuntime::new(
            store.clone(),
            clock.clone(),
            hook.clone(),
            clock.clone(),
            clock.clone(),
            clock.clone(),
        )?;
        {
            let _enter = handle.enter();
            app.start()?;
        }
        let fixture = Self {
            scene: scene.into(),
            path,
            store,
            app,
            hook,
            clock,
            handle,
            preview: Mutex::new(None),
            previous_candidate: Mutex::new(None),
            frame: Mutex::new(None),
            frame_sequence: AtomicU64::new(0),
            operations: Mutex::new(vec![]),
            events: Mutex::new(vec![]),
            fail_effects: AtomicU64::new(u64::from(scene == "BJS-13-effects-retry")),
            source_query_failures: AtomicU64::new(u64::from(scene == "BJS-17-source-query-retry")),
            last_effect: Mutex::new(None),
            saved_before: Mutex::new(None),
            published: AtomicU64::new(0),
            seed_commands,
        };
        if scene == "BJS-01-scoped-reconnect-owner" {
            let snapshot = fixture.app.snapshot()?;
            let target = &snapshot
                .games
                .first()
                .ok_or_else(|| bj_error("fixture_seed", "缺少实际游戏A"))?
                .target;
            let candidate = fixture
                .hook
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.environment_key == target.environment_key
                        && Some(candidate.executable_path.as_str())
                            == target.canonical_exe_path.as_deref()
                        && Some(candidate.architecture.as_str()) == target.architecture.as_deref()
                })
                .ok_or_else(|| bj_error("fixture_seed", "无法对应实际游戏A的目标"))?;
            fixture.connect(&candidate.candidate_id, true)?;
            fixture.candidates_ready()?;
            fixture.events.lock().unwrap().clear();
            fixture.operations.lock().unwrap().clear();
        }
        if ["BJS-05-reuse", "BJS-09-unowned-remember", "BJS-15-revoke"].contains(&scene) {
            fixture.connect("candidate-main", !unowned)?;
            fixture.candidates_ready()?;
            fixture.events.lock().unwrap().clear();
            fixture.operations.lock().unwrap().clear();
        }
        if ["BJS-05-switch-cancel", "BJS-07-cancel-candidate"].contains(&scene) {
            fixture.connect("candidate-other", true)?;
            fixture.candidates_ready()?;
            fixture.events.lock().unwrap().clear();
            fixture.operations.lock().unwrap().clear();
        }
        if ["BJS-09-requires-review", "BJS-09-temporary-saved"].contains(&scene) {
            fixture.connect("candidate-main", true)?;
            fixture.candidates_ready()?;
            let selection = fixture.app.source_selection(SourceSelectionQuery {
                session_id: fixture.hook.active()?.session_id,
                expected_profile_generation: 1,
            })?;
            let receipt = fixture.app.begin_reading(BeginReadingRequest {
                operation_id: fixture.clock.new_id("seed-selection"),
                session_id: selection.session_id,
                expected_profile_generation: 1,
                expected_set_revision: None,
                source_ids: selection
                    .sources
                    .iter()
                    .map(|s| s.source_id.clone())
                    .collect(),
                remember: true,
            })?;
            let _ = receipt;
            fixture.connect("candidate-other", true)?;
            fixture.connect("candidate-main", true)?;
            fixture.candidates_ready()?;
            fixture.events.lock().unwrap().clear();
            fixture.operations.lock().unwrap().clear();
        }
        if scene == "BJS-09-temporary-saved" {
            let projection = fixture.app.source_selection(SourceSelectionQuery {
                session_id: fixture.hook.active()?.session_id,
                expected_profile_generation: 1,
            })?;
            *fixture.saved_before.lock().unwrap() =
                Some(domain::canonical_hash(&projection.saved)?);
        }
        if scene == "BJS-17-association-retry" {
            fixture.hook.fail_associations.store(1, Ordering::SeqCst);
        }
        if ["BJS-08-commit-failure", "BJS-17-submit-retry"].contains(&scene) {
            rusqlite::Connection::open(&fixture.path).map_err(|_|bj_error("fixture_sqlite","无法打开故障注入连接"))?.execute_batch("CREATE TRIGGER controlled_selection_failure BEFORE INSERT ON source_selection_sets BEGIN SELECT RAISE(ABORT,'controlled selection write failure'); END;").map_err(|_|bj_error("fixture_sqlite","无法安装真实写入故障"))?;
        }
        Ok(fixture)
    }
    fn connect(&self, candidate_id: &str, owned: bool) -> AppResult<AssociationOperationReceipt> {
        let snapshot = self.app.snapshot()?;
        let candidate = self.hook.association_candidate(candidate_id)?;
        let game_id = owned
            .then(|| {
                snapshot
                    .games
                    .iter()
                    .find(|g| {
                        g.target.canonical_exe_path.as_deref()
                            == Some(candidate.executable_path.as_str())
                    })
                    .map(|g| g.game_id.clone())
            })
            .flatten();
        let review = self.app.review_association(AssociationReviewQuery {
            candidate_id: candidate_id.into(),
            game_id,
            expected_profile_generation: snapshot.profile_generation,
            expected_active_session_id: snapshot
                .session
                .as_ref()
                .filter(|s| matches!(s.state.as_str(), "opening" | "active"))
                .map(|s| s.session_id.clone()),
            prior_review_id: None,
            attribution_intent: None,
        })?;
        let request = ReviewedAssociateRequest {
            operation_id: self.clock.new_id("fixture-operation"),
            review_id: review.review_id,
        };
        let receipt = self.app.associate_reviewed(request)?;
        self.operations
            .lock()
            .unwrap()
            .push(receipt.operation_id.clone());
        Ok(receipt)
    }
    fn stage_event(&self, operation: &str) -> AppResult<()> {
        let progress = self.app.association_progress(AssociationProgressQuery {
            operation_id: operation.into(),
            expected_profile_generation: 1,
        })?;
        self.events.lock().unwrap().push(json!({"kind":"association_stage","operation_id":progress.operation_id,"session_id":progress.session_id,"stage":progress.stage,"generation":progress.generation}));
        Ok(())
    }
    fn candidates_ready(&self) -> AppResult<()> {
        self.hook
            .event(&self.clock, "process_connected", "handshake")?;
        self.hook.text(&self.clock, "A")?;
        self.hook.text(&self.clock, "B")?;
        self.hook.drain()?;
        self.app.publish()?;
        Ok(())
    }
    fn publication(&self) -> AppResult<Vec<ProjectionUpdate>> {
        self.app.publish()?;
        let update = self.app.subscribe().borrow().clone();
        let previous = self
            .published
            .fetch_max(u64::from(update.revision), Ordering::SeqCst);
        Ok(if u64::from(update.revision) > previous {
            vec![update]
        } else {
            vec![]
        })
    }
    fn source_metadata(
        &self,
        projection: &SourceSelectionProjection,
        response_id: &str,
        root: &str,
    ) -> Value {
        Value::Array(projection.sources.iter().enumerate().map(|(i,source)|{let entity=format!("{root}/sources/{i}");json!({"source_id":source.source_id,"source_pointer":entity,"source_id_pointer":"/sourceId","response_id":response_id,"fields":[{"label":"Hook","value":source.hook_name,"json_pointer":format!("{entity}/hookName")},{"label":"Address","value":source.address,"json_pointer":format!("{entity}/address")},{"label":"Context","value":source.context,"json_pointer":format!("{entity}/context")},{"label":"Context 2","value":source.context2,"json_pointer":format!("{entity}/context2")}]})}).collect())
    }
    fn reset_value(&self, id: &str) -> AppResult<Value> {
        let snapshot = self.app.snapshot()?;
        let details = if let Some(session) = snapshot.session.as_ref() {
            Some(self.app.source_selection(SourceSelectionQuery {
                session_id: session.session_id.clone(),
                expected_profile_generation: 1,
            })?)
        } else if self.scene == "BJS-10-details" {
            // The browser needs its immutable evidence envelope before the UI
            // connects. Obtain that envelope from a second real Runtime/SQLite
            // consuming the identical controlled native session/stream inputs.
            // No source DTO fields or opaque IDs are patched by Node. The main
            // fixture still executes its own review/associate/callback/query.
            let reference = Self::new("reference-source-projection", self.handle.clone())?;
            reference.connect("candidate-main", true)?;
            reference.candidates_ready()?;
            Some(reference.app.source_selection(SourceSelectionQuery {
                session_id: reference.hook.active()?.session_id,
                expected_profile_generation: 1,
            })?)
        } else {
            None
        };
        let metadata = details
            .as_ref()
            .map(|projection| self.source_metadata(projection, id, "/source_selection"))
            .unwrap_or_else(|| json!([]));
        let sealed = if [
            "BJS-01-library-global-owner",
            "BJS-01-scoped-reconnect-owner",
        ]
        .contains(&self.scene.as_str())
        {
            Some(self.seal_library_database("before")?)
        } else {
            None
        };
        Ok(
            json!({"initial_snapshot":snapshot,"source_selection":details,"source_detail_obligations":metadata,
            "seed_commands":self.seed_commands,"sealed_database_before":sealed,
            "fixture_scope":"Actual ApplicationRuntime/SQLite; controlled Core/preview/window ports. No native acceptance."}),
        )
    }
    fn effects(&self, authority: ReadingWindowEffectAuthority) -> ReadingWindowEffectsResult {
        let mut shown = NativeWindowStepResult {
            state: NativeWindowStepState::NotAttempted,
            error: None,
        };
        let mut hidden = shown.clone();
        let result=self.app.with_reading_window_effect_authority(authority.clone(),&mut ||{if self.fail_effects.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|(n>0).then(||n-1)).is_ok(){let error=bj_error("controlled_hud_show_failed","受控窗口端口显示浮窗失败");shown=NativeWindowStepResult{state:NativeWindowStepState::Failed,error:Some(error.clone())};return Err(error);}shown.state=NativeWindowStepState::Succeeded;self.events.lock().unwrap().push(json!({"kind":"hud_shown","authority":authority,"scope":"controlled-window-port"}));hidden.state=NativeWindowStepState::Succeeded;self.events.lock().unwrap().push(json!({"kind":"main_hidden","authority":authority,"scope":"controlled-window-port"}));Ok(())});
        let success = result.is_ok();
        if let Err(error) = result {
            if shown.state == NativeWindowStepState::NotAttempted {
                shown.error = Some(error);
            }
        }
        ReadingWindowEffectsResult {
            authority,
            show_hud: shown,
            hide_main: hidden,
            retryable: !success,
            detail: if success {
                "正文选择已保存，已完成受控窗口效果"
            } else {
                "选择已保存，窗口操作未完成，请重试窗口操作"
            }
            .into(),
        }
    }
    fn begin(&self, request: BeginReadingRequest) -> AppResult<BeginReadingResult> {
        let receipt = self.app.begin_reading(request.clone());
        if self.scene == "BJS-17-submit-retry" && receipt.is_err() {
            rusqlite::Connection::open(&self.path)
                .map_err(|_| bj_error("fixture_sqlite", "无法打开故障恢复连接"))?
                .execute_batch("DROP TRIGGER IF EXISTS controlled_selection_failure;")
                .map_err(|_| bj_error("fixture_sqlite", "无法移除写入故障"))?;
        }
        let receipt = receipt?;
        if !receipt.replayed {
            self.events.lock().unwrap().push(json!({"kind":"selection_commit","operation_id":receipt.operation_id,"session_id":receipt.selection.session_id,"set_revision":receipt.selection.set_revision,"source_ids":receipt.selection.members.iter().map(|m|&m.source_id).collect::<Vec<_>>(),"remember":request.remember}));
        }
        *self.last_effect.lock().unwrap() = Some(receipt.effect_authority.clone());
        let effects = self.effects(receipt.effect_authority.clone());
        Ok(BeginReadingResult {
            selection: receipt,
            effects,
        })
    }
    fn preview_start(&self, request: PreviewStartRequest) -> AppResult<PreviewProjection> {
        let mut state = self.preview.lock().unwrap();
        if let Some(current) = state
            .as_ref()
            .filter(|p| p.consumer_id == request.consumer_id)
        {
            return Ok(current.clone());
        }
        let generation = state
            .as_ref()
            .map(|p| p.generation.parse::<u64>().unwrap() + 1)
            .unwrap_or(1)
            .to_string();
        let code = if self.scene == "BJS-02-tcc" {
            "preview_permission_denied"
        } else {
            "preview_window_selection_required"
        };
        let policy = production_preview_decisions::collector_status_policy("Failure", Some(code))
            .ok_or_else(|| bj_error("fixture_preview_policy", "生产预览状态策略缺失"))?;
        let projection = PreviewProjection {
            lease_id: Some(self.clock.new_id("preview-lease")),
            generation,
            consumer_id: request.consumer_id,
            state: policy.state,
            permission: if code == "preview_permission_denied" {
                PreviewPermission::Denied
            } else {
                PreviewPermission::Granted
            },
            target_identity_hash: Some(domain::content_hash(request.candidate_id)),
            attempted_at_ms: Some(self.clock.now_ms().to_string()),
            captured_at_ms: None,
            content_changed_at_ms: None,
            permission_checked_at_ms: Some(self.clock.now_ms().to_string()),
            error_code: Some(code.into()),
            detail: policy.detail.into(),
        };
        *state = Some(projection.clone());
        Ok(production_preview_decisions::projection_snapshot(
            &projection,
            None,
        ))
    }
    fn preview_update(
        &self,
        projection: PreviewProjection,
        frame: Option<PreviewFrame>,
    ) -> PreviewUpdate {
        PreviewUpdate { projection, frame }
    }
    fn frame_update(&self, newer: bool) -> AppResult<PreviewUpdate> {
        let mut guard = self.preview.lock().unwrap();
        let projection = guard
            .as_mut()
            .ok_or_else(|| bj_error("fixture_preview_missing", "未建立实际预览请求"))?;
        let sequence = self.frame_sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let now = if newer {
            self.clock.now_ms() + 1000
        } else {
            self.clock.now_ms()
        };
        let payload = base64::engine::general_purpose::STANDARD
            .decode(BJ_JPEG)
            .map_err(|_| bj_error("fixture_jpeg", "受控像素无效"))?;
        let frame = PreviewFrame {
            lease_id: projection.lease_id.clone().unwrap(),
            generation: projection.generation.clone(),
            sequence: sequence.to_string(),
            target_identity_hash: projection.target_identity_hash.clone().unwrap(),
            captured_at_ms: now.to_string(),
            width: 32,
            height: 18,
            content_hash: domain::content_hash(&payload),
            encoding: "jpeg".into(),
            payload,
        };
        projection.state = PreviewState::Live;
        projection.permission = PreviewPermission::Granted;
        projection.captured_at_ms = Some(frame.captured_at_ms.clone());
        projection.content_changed_at_ms = Some(frame.captured_at_ms.clone());
        projection.error_code = None;
        projection.detail = "受控窗口像素已经交付；Native 层另验".into();
        *self.frame.lock().unwrap() = Some(frame.clone());
        Ok(self.preview_update(projection.clone(), Some(frame)))
    }
    fn transfer_preview(&self) -> AppResult<()> {
        let active = self.hook.active()?;
        let mut guard = self.preview.lock().unwrap();
        if let Some(projection) = guard.as_mut() {
            *self.previous_candidate.lock().unwrap() = Some(projection.clone());
            projection.consumer_id = active.session_id;
            projection.generation = (projection.generation.parse::<u64>().unwrap() + 1).to_string();
            if let Some(frame) = self.frame.lock().unwrap().as_mut() {
                frame.consumer_identity_transfer(projection);
            }
        }
        Ok(())
    }
}
trait BjTransferredFrame {
    fn consumer_identity_transfer(&mut self, p: &PreviewProjection);
}
impl BjTransferredFrame for PreviewFrame {
    fn consumer_identity_transfer(&mut self, p: &PreviewProjection) {
        self.generation = p.generation.clone();
    }
}

const BJ_JPEG: &str = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAUDBAQEAwUEBAQFBQUGBwwIBwcHBw8LCwkMEQ8SEhEPERETFhwXExQaFRERGCEYGh0dHx8fExciJCIeJBweHx7/2wBDAQUFBQcGBw4ICA4eFBEUHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh7/wAARCAASACADASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDCooor6o8UKKKKACiiigAooooA/9k=";

impl BjFixture {
    fn invoke(&self, method: &str, args: &[Value]) -> AppResult<(Value, Vec<PreviewUpdate>)> {
        let mut updates = vec![];
        let value = match method {
            "snapshot" => bj_value(self.app.snapshot()?)?,
            "targets" | "pickWindow" => bj_value(self.hook.targets()?)?,
            "reviewAssociation" => bj_value(self.app.review_association(bj_arg(args, 0)?)?)?,
            "associate" => {
                let request: ReviewedAssociateRequest = bj_arg(args, 0)?;
                let receipt = self.app.associate_reviewed(request)?;
                self.operations
                    .lock()
                    .unwrap()
                    .push(receipt.operation_id.clone());
                self.app.with_association_preview_authority(
                    &receipt,
                    &mut |candidate, session_id| {
                        if self.hook.active()?.session_id != session_id {
                            return Err(bj_error("fixture_transfer_stale", "接入会话已经变化"));
                        }
                        let _ = candidate;
                        self.transfer_preview()
                    },
                )?;
                if ![
                    "BJS-06-real-stage-order",
                    "BJS-06-stale-events",
                    "BJS-07-close-operation",
                ]
                .contains(&self.scene.as_str())
                {
                    self.candidates_ready()?;
                }
                self.stage_event(&receipt.operation_id)?;
                bj_value(receipt)?
            }
            "associationProgress" => {
                let query: AssociationProgressQuery = bj_arg(args, 0)?;
                let progress = self.app.association_progress(query)?;
                bj_value(progress)?
            }
            "sourceSelection" => {
                let query: SourceSelectionQuery = bj_arg(args, 0)?;
                if self
                    .source_query_failures
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                        (n > 0).then(|| n - 1)
                    })
                    .is_ok()
                {
                    return Err(bj_error(
                        "controlled_source_query_failed",
                        "受控来源读取失败，可重试",
                    ));
                }
                bj_value(self.app.source_selection(query)?)?
            }
            "beginReading" => bj_value(self.begin(bj_arg(args, 0)?)?)?,
            "retryReadingWindowEffects" => {
                let authority: ReadingWindowEffectAuthority = bj_arg(args, 0)?;
                bj_value(self.effects(authority))?
            }
            "dispatch" => {
                let envelope: CommandEnvelope = bj_arg(args, 0)?;
                bj_value(self.handle.block_on(self.app.dispatch(envelope))?)?
            }
            "previewCapability" => bj_value(PreviewCapability {
                supported: true,
                permission: if self.scene == "BJS-02-tcc" {
                    PreviewPermission::Denied
                } else if self
                    .preview
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|p| p.state == PreviewState::PermissionRevoked)
                {
                    PreviewPermission::Denied
                } else {
                    PreviewPermission::Granted
                },
                error_code: None,
                detail: "受控窗口采集端口；Native 权限另验".into(),
            })?,
            "startWindowPreview" => bj_value(self.preview_start(bj_arg(args, 0)?)?)?,
            "selectWindowPreview" => {
                let request: PreviewStartRequest = bj_arg(args, 0)?;
                let mut projection = self.preview_start(request)?;
                if ["BJS-04-picker-cancel", "BJS-04-picker-timeout"].contains(&self.scene.as_str())
                {
                    let code = if self.scene.ends_with("timeout") {
                        "preview_selection_timeout"
                    } else {
                        "preview_selection_cancelled"
                    };
                    let policy = production_preview_decisions::capture_failure_policy(code, false);
                    projection.state = policy.state;
                    projection.error_code = Some(code.into());
                    projection.detail = if code == "preview_selection_cancelled" {
                        "已取消系统选窗，可以重新选择"
                    } else {
                        policy.detail
                    }
                    .into();
                    *self.preview.lock().unwrap() = Some(projection.clone());
                    bj_value(projection)?
                } else {
                    let update = self.frame_update(false)?;
                    let projection = update.projection.clone();
                    updates.push(update);
                    bj_value(projection)?
                }
            }
            "previewSession" => {
                let projection = self
                    .preview
                    .lock()
                    .unwrap()
                    .clone()
                    .ok_or_else(|| bj_error("fixture_preview_missing", "尚无预览租约"))?;
                if let Some(frame) = self.frame.lock().unwrap().clone() {
                    updates.push(self.preview_update(projection.clone(), Some(frame)));
                }
                bj_value(projection)?
            }
            "controlWindowPreview" => {
                let request: PreviewControlRequest = bj_arg(args, 0)?;
                let mut guard = self.preview.lock().unwrap();
                let projection = guard
                    .as_mut()
                    .ok_or_else(|| bj_error("fixture_preview_missing", "尚无预览租约"))?;
                if projection.lease_id.as_ref() != Some(&request.lease_id)
                    || projection.generation != request.generation
                {
                    return Err(bj_error(
                        "stale_preview_generation",
                        "旧预览操作不会更改当前租约",
                    ));
                }
                projection.state = match request.action {
                    PreviewControl::Pause => PreviewState::Paused,
                    PreviewControl::Resume => {
                        if self.frame.lock().unwrap().is_none() {
                            let policy = production_preview_decisions::collector_status_policy(
                                "Failure",
                                Some("preview_window_selection_required"),
                            )
                            .ok_or_else(|| {
                                bj_error("fixture_preview_policy", "缺少生产缺配对策略")
                            })?;
                            projection.error_code =
                                Some("preview_window_selection_required".into());
                            projection.detail = policy.detail.into();
                            policy.state
                        } else {
                            PreviewState::Starting
                        }
                    }
                    PreviewControl::Stop => PreviewState::Stopped,
                };
                if projection.state != PreviewState::Unavailable {
                    projection.error_code = None;
                    projection.detail = "受控采集器已经处理显式控制；等待新有效帧".into();
                }
                let result = projection.clone();
                updates.push(self.preview_update(result.clone(), None));
                bj_value(result)?
            }
            "openWindow" => {
                self.events.lock().unwrap().push(json!({"kind":"explicit_window_open","args":args,"scope":"controlled-window-port"}));
                Value::Null
            }
            "closeWindow" => {
                self.events
                    .lock()
                    .unwrap()
                    .push(json!({"kind":"explicit_window_close","scope":"controlled-window-port"}));
                Value::Null
            }
            "readGameCover" => bj_value(self.app.game_cover(bj_arg(args, 0)?)?)?,
            "resources" => bj_value(self.app.resources())?,
            "gameWindowBinding" => {
                let active = self.hook.active_session_context()?;
                bj_value(GameWindowBindingProjection {
                    state: if active.is_some() {
                        GameWindowBindingState::Bound
                    } else {
                        GameWindowBindingState::Unbound
                    },
                    session_id: active.as_ref().map(|s| s.session_id.clone()),
                    target_identity_sha256: active.map(|s| s.target_identity_sha256),
                    window_title: None,
                    verified_at_ms: None,
                    error_code: None,
                    detail: "受控窗口端口；真实OS归属另验".into(),
                })?
            }
            "windowCapabilities" => {
                return Err(bj_error(
                    "fixture_native_unavailable",
                    "Native 能力由独立层回归",
                ));
            }
            _ => {
                return Err(bj_error(
                    "fixture_method_unavailable",
                    "实际组件fixture没有注册该端口",
                ));
            }
        };
        Ok((value, updates))
    }
    fn deliver(&self, stimulus: &str) -> AppResult<(Value, Vec<PreviewUpdate>)> {
        let mut updates = vec![];
        match stimulus {
            "scoped_session_disconnected" => {
                self.handle.block_on(self.app.dispatch(CommandEnvelope {
                    operation_id: self.clock.new_id("scoped-disconnect"),
                    expected_revision: None,
                    command: Command::Disconnect,
                }))?;
            }
            "seal_library_database" => {
                let sealed = self.seal_library_database("after")?;
                return Ok((json!({"sealed_database_after":sealed}), updates));
            }
            "verified"
            | "resources_ready"
            | "start_requested"
            | "handshake"
            | "waiting_text"
            | "operation_continues" => {
                let (code, stage) = match stimulus {
                    "verified" => ("target_identity_verified", "identity"),
                    "resources_ready" => ("resource_verified", "resource"),
                    "start_requested" => ("injection_requested", "injection"),
                    _ => ("process_connected", "handshake"),
                };
                self.hook.event(&self.clock, code, stage)?;
                self.hook.drain()?;
                if let Some(operation) = self.operations.lock().unwrap().last().cloned() {
                    self.stage_event(&operation)?;
                }
            }
            "candidates_ready" => {
                self.candidates_ready()?;
                if let Some(operation) = self.operations.lock().unwrap().last().cloned() {
                    self.stage_event(&operation)?;
                }
            }
            "session_replaced" | "current_operation" => {
                let receipt = self.connect("candidate-other", true)?;
                self.candidates_ready()?;
                self.stage_event(&receipt.operation_id)?;
            }
            "old_operation_event" => {
                let operations = self.operations.lock().unwrap().clone();
                if let Some(old) = operations.first() {
                    let progress = self.app.association_progress(AssociationProgressQuery {
                        operation_id: old.clone(),
                        expected_profile_generation: 1,
                    })?;
                    if progress.superseded {
                        self.events.lock().unwrap().push(json!({"kind":"association_event_rejected","operation_id":old,"stage":progress.stage,"reason":"actual-operation-superseded"}));
                    }
                }
            }
            "valid_frame" | "new_frame" | "session_frame" => {
                updates.push(self.frame_update(stimulus == "new_frame")?);
            }
            "late_candidate_frame" => {
                if let Some(projection) = self.previous_candidate.lock().unwrap().clone() {
                    let mut frame = self.frame.lock().unwrap().clone();
                    if let Some(frame) = frame.as_mut() {
                        frame.generation = projection.generation.clone();
                        frame.sequence = "999".into();
                        frame.captured_at_ms = (self.clock.now_ms() + 9000).to_string();
                    }
                    updates.push(self.preview_update(projection, frame));
                }
            }
            "permission_revoked" | "capture_failed" => {
                let code = if stimulus == "permission_revoked" {
                    "preview_permission_revoked"
                } else {
                    "preview_capture_timeout"
                };
                let policy = production_preview_decisions::capture_failure_policy(code, true);
                let mut guard = self.preview.lock().unwrap();
                if let Some(projection) = guard.as_mut() {
                    projection.state = policy.state;
                    projection.error_code = Some(code.into());
                    projection.detail = policy.detail.into();
                    if policy.permission_denied {
                        projection.permission = PreviewPermission::Denied;
                    }
                    updates.push(self.preview_update(projection.clone(), None));
                }
            }
            "metadata_only" => {
                if let Some(projection) = self.preview.lock().unwrap().clone() {
                    if production_preview_decisions::collector_status_policy("Live", None).is_none()
                    {
                        updates.push(self.preview_update(projection, None));
                    }
                }
            }
            "old_frame" => {
                if let (Some(projection), Some(mut frame)) = (
                    self.preview.lock().unwrap().clone(),
                    self.frame.lock().unwrap().clone(),
                ) {
                    frame.sequence = "0".into();
                    updates.push(self.preview_update(projection, Some(frame)));
                }
            }
            "source_expired" => {
                let active = self.hook.active()?;
                self.store.record_hook_diagnostic(
                    Some(active.session_id),
                    Diagnostic {
                        code: "game_disconnected".into(),
                        stage: "capture".into(),
                        message: "受控游戏已经退出，来源失效".into(),
                        recoverable: true,
                    },
                    self.clock.now_ms(),
                )?;
            }
            "old_window_effect" => {
                if let Some(authority) = self.last_effect.lock().unwrap().clone() {
                    let mut called = false;
                    let result = self.app.with_reading_window_effect_authority(
                        authority.clone(),
                        &mut || {
                            called = true;
                            Ok(())
                        },
                    );
                    if let Err(error) = result {
                        self.events.lock().unwrap().push(json!({"kind":"stale_window_effect_rejected","authority":authority,"code":error.code,"callback_invoked":called}));
                    } else {
                        return Err(bj_error(
                            "fixture_stale_effect_accepted",
                            "旧窗口操作竟然获得授权",
                        ));
                    }
                }
            }
            "reopen_saved_selection" => {
                let session = self.hook.active()?.session_id;
                let projection = self.app.source_selection(SourceSelectionQuery {
                    session_id: session,
                    expected_profile_generation: 1,
                })?;
                let after = domain::canonical_hash(&projection.saved)?;
                if let Some(before) = self.saved_before.lock().unwrap().clone() {
                    self.events.lock().unwrap().push(json!({"kind":"saved_selection_comparison","before_hash":before,"after_hash":after}));
                }
            }
            _ => return Err(bj_error("fixture_stimulus_unavailable", stimulus)),
        }
        self.app.publish()?;
        Ok((json!({"delivered":stimulus}), updates))
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BjRpcRequest {
    #[serde(rename = "type")]
    kind: String,
    schema_version: u32,
    nonce: String,
    id: String,
    scenario: String,
    action: String,
    method: Option<String>,
    args: Vec<Value>,
}
fn bj_emit(output: &Mutex<std::io::Stdout>, value: &Value) {
    let mut stdout = output.lock().unwrap();
    writeln!(
        stdout,
        "{RPC_PREFIX}{}",
        serde_json::to_string(value).unwrap()
    )
    .unwrap();
    stdout.flush().unwrap();
}
#[test]
#[ignore = "registered actual runtime transport; driven by versioned NDJSON"]
fn bj_component_transport() {
    if std::env::var("LUNA_BINDING_MODE").ok().as_deref() != Some("stdio") {
        return;
    }
    let nonce = std::env::var("LUNA_BINDING_NONCE").unwrap();
    let source_tree = std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap();
    let output = Arc::new(Mutex::new(std::io::stdout()));
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let handle = executor.handle().clone();
    let current: Arc<Mutex<Option<Arc<BjFixture>>>> = Arc::new(Mutex::new(None));
    let mut workers = vec![];
    bj_emit(
        &output,
        &json!({"type":"ready","schema_version":1,"nonce":nonce,"source_tree_sha256":source_tree,"capabilities":{"scene_ids":SCENES,"methods":METHODS,"controls":["reset","deliver"]}}),
    );
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        assert!(line.len() <= 1024 * 1024, "bounded fixture request");
        let request: BjRpcRequest =
            serde_json::from_str(line.strip_prefix(RPC_PREFIX).expect("exact RPC framing"))
                .unwrap();
        assert_eq!(request.kind, "request");
        assert_eq!(request.schema_version, 1);
        assert_eq!(request.nonce, nonce);
        canonical_binding_decimal(&request.id).unwrap();
        assert!(SCENES.contains(&request.scenario.as_str()));
        if request.action == "reset" {
            for worker in workers.drain(..) {
                std::thread::JoinHandle::join(worker).unwrap();
            }
            let fixture = Arc::new(BjFixture::new(&request.scenario, handle.clone()).unwrap());
            let value = fixture.reset_value(&request.id);
            let publications = fixture.publication().unwrap();
            *current.lock().unwrap() = Some(fixture);
            let (response, error, outcome) = match value {
                Ok(v) => (v, Value::Null, "ok"),
                Err(e) => (Value::Null, bj_value(e).unwrap(), "error"),
            };
            bj_emit(
                &output,
                &json!({"type":"response","schema_version":1,"nonce":nonce,"id":request.id,"scenario":request.scenario,"action":request.action,"method":request.method,"outcome":outcome,"response":response,"error":error,"publications":publications,"previewUpdates":[],"events":[]}),
            );
            continue;
        }
        let fixture = current.lock().unwrap().as_ref().unwrap().clone();
        assert_eq!(fixture.scene, request.scenario);
        let output = output.clone();
        let nonce = nonce.clone();
        workers.push(std::thread::spawn(move||{let result=match request.action.as_str(){"invoke"=>fixture.invoke(request.method.as_deref().unwrap(),&request.args),"deliver"=>fixture.deliver(request.args.first().and_then(Value::as_str).unwrap()),_=>Err(bj_error("fixture_action","未知控制"))};let publications=fixture.publication().unwrap_or_default();let events=std::mem::take(&mut *fixture.events.lock().unwrap());let(response,error,outcome,updates)=match result{Ok((v,u))=>(v,Value::Null,"ok",u),Err(e)=>(Value::Null,bj_value(e).unwrap(),"error",vec![])};bj_emit(&output,&json!({"type":"response","schema_version":1,"nonce":nonce,"id":request.id,"scenario":request.scenario,"action":request.action,"method":request.method,"outcome":outcome,"response":response,"error":error,"publications":publications,"previewUpdates":updates,"events":events}));}));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    drop(current);
    executor.shutdown_timeout(Duration::from_secs(2));
}

impl BjFixture {
    fn seal_library_database(&self, label: &str) -> AppResult<Value> {
        let directory = self
            .path
            .parent()
            .ok_or_else(|| bj_error("fixture_directory", "缺少独立配置档目录"))?;
        let database = directory.join(format!("library-{label}.sqlite3"));
        self.store.backup(&database)?;
        let root = std::env::current_dir()
            .map_err(|_| bj_error("fixture_directory", "无法取得项目目录"))?;
        let reference = |file: &std::path::Path| -> AppResult<Value> {
            let bytes =
                std::fs::read(file).map_err(|_| bj_error("fixture_reference", "封存文件不可读"))?;
            let path = file.strip_prefix(&root).map_err(|_| {
                bj_error("fixture_reference", "封存文件必须在当前项目ignored输出中")
            })?;
            Ok(json!({"path":path.to_string_lossy(),"sha256":domain::content_hash(bytes)}))
        };
        let manifest = directory.join(format!("library-{label}.manifest.json"));
        let document = json!({"kind":"binding-library-database-snapshot/v1","schema_version":1,
            "nonce":std::env::var("LUNA_BINDING_NONCE").map_err(|_|bj_error("fixture_nonce","缺少本次执行身份"))?,
            "source_tree_sha256":std::env::var("LUNA_BINDING_SOURCE_TREE").map_err(|_|bj_error("fixture_source","缺少本次源码身份"))?,
            "scene_id":self.scene,"pid":std::process::id(),"captured_at_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|bj_error("fixture_clock","系统时间无效"))?.as_millis(),
            "database":reference(&database)?});
        std::fs::write(
            &manifest,
            serde_json::to_vec_pretty(&document)
                .map_err(|_| bj_error("fixture_serialization", "封存说明序列化失败"))?,
        )
        .map_err(|_| bj_error("fixture_reference", "封存说明写入失败"))?;
        reference(&manifest)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_sqlite_route_preserves_hook_only_preview_session_projection() {
    async fn manual(fixture: &BjFixture) {
        fixture
            .app
            .dispatch(CommandEnvelope {
                operation_id: fixture.clock.new_id("actual-manual-route"),
                expected_revision: None,
                command: Command::ManualInput {
                    text: "手動入力の実際の経路。".into(),
                    source_language: "ja".into(),
                },
            })
            .await
            .unwrap();
    }
    let fixture = BjFixture::new(
        "actual-manual-preview-route",
        tokio::runtime::Handle::current(),
    )
    .unwrap();
    manual(&fixture).await;
    assert!(fixture.app.snapshot().unwrap().session.is_none());
    assert!(
        fixture
            .app
            .current_session_preview_authority()
            .unwrap()
            .is_none()
    );
    let initial = fixture
        .app
        .inactive_session_preview_authority()
        .unwrap()
        .unwrap();
    let mut callbacks = 0;
    fixture
        .app
        .with_inactive_session_preview_authority(&initial, &mut || {
            callbacks += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(callbacks, 1);

    fixture.connect("candidate-main", true).unwrap();
    let hook_before = fixture
        .app
        .current_session_preview_authority()
        .unwrap()
        .unwrap();
    manual(&fixture).await;
    let active = fixture.app.snapshot().unwrap().session.unwrap();
    assert_eq!(active.source_kind, "hook");
    assert_eq!(active.session_id, hook_before.hook_session.session_id);
    assert!(
        fixture
            .app
            .inactive_session_preview_authority()
            .unwrap()
            .is_none()
    );

    fixture
        .app
        .dispatch(CommandEnvelope {
            operation_id: fixture.clock.new_id("actual-disconnect-before-manual"),
            expected_revision: None,
            command: Command::Disconnect,
        })
        .await
        .unwrap();
    manual(&fixture).await;
    let stopped = fixture.app.snapshot().unwrap().session.unwrap();
    assert_eq!(stopped.source_kind, "hook");
    assert!(!matches!(stopped.state.as_str(), "opening" | "active"));
    assert!(fixture.hook.active_session_context().unwrap().is_none());
    assert!(
        fixture
            .app
            .current_session_preview_authority()
            .unwrap()
            .is_none()
    );
    let manual_rows: i64 = rusqlite::Connection::open(&fixture.path)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sessions WHERE source_kind='manual' AND state='active'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        manual_rows, 3,
        "all three actual ManualInput transactions created active manual rows"
    );
    let inactive = fixture
        .app
        .inactive_session_preview_authority()
        .unwrap()
        .unwrap();
    fixture
        .app
        .with_inactive_session_preview_authority(&inactive, &mut || {
            callbacks += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        callbacks, 2,
        "manual rows do not prevent exact obsolete Hook cleanup"
    );

    fixture.connect("candidate-other", true).unwrap();
    assert!(
        fixture
            .app
            .with_inactive_session_preview_authority(&inactive, &mut || {
                callbacks += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(
        callbacks, 2,
        "the actual manual-to-Hook transition invalidates old cleanup"
    );
}
