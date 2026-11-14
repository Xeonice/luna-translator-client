//! Narrow shared Native-test fixture: real Runtime and SQLite; controlled I/O only.
#![allow(dead_code)]
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{self, TaskDispatch, capture::*},
    ports::{IdentitySource, runtime::*},
};
use app_infra::storage::SqliteStore;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
fn bj_error(code: &str, message: &str) -> AppError {
    AppError::new(code, "controlled_native_fixture", message)
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
struct BjAux {
    ids: AtomicU64,
    now: AtomicI64,
}
impl BjAux {
    fn new() -> Self {
        Self {
            ids: AtomicU64::new(1),
            now: AtomicI64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            ),
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

struct NativeHook {
    candidates: Vec<TargetCandidate>,
    active: Mutex<Option<HookSessionInput>>,
    lease: Mutex<()>,
    serial: AtomicU64,
    stops: AtomicU64,
    resource_path: Mutex<Option<PathBuf>>,
    target_lease_path: Mutex<Option<PathBuf>>,
    sender: mpsc::Sender<HookUpdate>,
    receiver: Mutex<Option<mpsc::Receiver<HookUpdate>>>,
}

impl NativeHook {
    fn active(&self) -> AppResult<HookSessionInput> {
        self.active
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| bj_error("no_active_session", "没有活动宿主"))
    }
}
impl HookBackend for NativeHook {
    fn targets(&self) -> AppResult<TargetCandidates> {
        Ok(TargetCandidates {
            candidates: self.candidates.clone(),
            diagnostics: vec![],
        })
    }
    fn association_candidate(&self, id: &str) -> AppResult<TargetCandidate> {
        self.candidates
            .iter()
            .find(|c| c.candidate_id == id)
            .cloned()
            .ok_or_else(|| bj_error("stale_target", "目标不存在"))
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
        if !same_hook_session_facts(&self.active()?, expected) {
            return Err(bj_error("reading_effect_stale", "完整宿主事实已变化"));
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
            .ok_or_else(|| bj_error("stale_session", "宿主不存在"))?;
        if current.session_id != input.session_id
            || current.target_identity_sha256 != input.target_identity_sha256
        {
            return Err(bj_error("stale_session", "宿主已变化"));
        }
        commit()?;
        if current.game_id.is_none() {
            current.game_id = input.game_id.clone()
        }
        Ok(())
    }
    fn associate(&self, _r: AssociateRequest) -> AppResult<HookSessionInput> {
        Err(bj_error("reviewed_association_required", "必须核对目标"))
    }
    fn associate_reviewed_traced(
        &self,
        r: AssociateRequest,
        g: &AssociationExecutionGuard,
        record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        let _l = self.lease.lock().unwrap();
        let candidate = self.association_candidate(&r.candidate_id)?;
        let mut active = self.active.lock().unwrap();
        let full_matches = match (g.expected_active.as_ref(), active.as_ref()) {
            (Some(expected), Some(current)) => same_hook_session_facts(expected, current),
            (None, None) => true,
            _ => false,
        };
        if !full_matches
            || g.expected_active_session_id.as_ref() != active.as_ref().map(|s| &s.session_id)
            || g.expected_active_target_identity_sha256.as_ref()
                != active.as_ref().map(|s| &s.target_identity_sha256)
            || g.expected_broker_instance_id.as_ref()
                != active.as_ref().map(|s| &s.broker_instance_id)
        {
            return Err(bj_error("association_review_stale", "原活动上下文已变化"));
        }
        let proposed = bj_session(&candidate, self.serial.load(Ordering::SeqCst), r.game_id);
        if proposed.target_identity_sha256 != g.candidate_identity_sha256 {
            return Err(bj_error("association_review_stale", "完整目标已变化"));
        }
        if let Some(current) = active
            .as_ref()
            .filter(|s| s.target_identity_sha256 == proposed.target_identity_sha256)
        {
            return Ok(current.clone());
        }
        record(HookStageInput {
            level: "info".into(),
            stage: "resource".into(),
            code: "resource_check_started".into(),
            message: "开始实际受控资源读取".into(),
        })?;
        if let Some(path) = self.resource_path.lock().unwrap().as_ref() {
            std::fs::read(path)
                .map_err(|_| bj_error("resource_unavailable", "实际资源文件不可读"))?;
        }
        let lease_path = self.target_lease_path.lock().unwrap().clone();
        let _reservation = if let Some(path) = lease_path {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .map_err(|_| bj_error("target_lease_unavailable", "实际租约文件不可用"))?;
            file.try_lock()
                .map_err(|_| bj_error("target_owned_elsewhere", "实际租约被另一持有者占用"))?;
            Some(file)
        } else {
            None
        };
        record(HookStageInput {
            level: "info".into(),
            stage: "injection".into(),
            code: "injection_requested".into(),
            message: "受控宿主端口已调用".into(),
        })?;
        self.serial.fetch_add(1, Ordering::SeqCst);
        if active.is_some() {
            self.stops.fetch_add(1, Ordering::SeqCst);
        }
        *active = Some(proposed.clone());
        Ok(proposed)
    }
    fn disconnect(&self) -> AppResult<()> {
        if self.active.lock().unwrap().take().is_some() {
            self.stops.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
    fn stop_and_drain(&self, a: mpsc::SyncSender<AppResult<()>>) -> AppResult<()> {
        self.disconnect()?;
        self.sender
            .send(HookUpdate::DrainBarrier { acknowledged: a })
            .map_err(|_| bj_error("fixture_channel_closed", "Core通道已关闭"))
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        self.receiver
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| bj_error("fixture_receiver_taken", "订阅已获取"))
    }
    fn resource_info(&self) -> ResourceInfo {
        ResourceInfo {
            kind: "core".into(),
            version: "controlled-native-port-v1".into(),
            manifest_sha256: domain::content_hash("controlled-native-port-v1"),
            available: true,
            detail: "受控I/O".into(),
        }
    }
}
static FIXTURE_IDS: AtomicU64 = AtomicU64::new(1);
pub struct NativePreviewFixture {
    app: Arc<ApplicationRuntime>,
    store: Arc<SqliteStore>,
    hook: Arc<NativeHook>,
    clock: Arc<BjAux>,
    path: PathBuf,
}
impl NativePreviewFixture {
    /// Construct inside the caller's real async-runtime context; no executor is manufactured here.
    pub fn new(base: &Path) -> AppResult<Self> {
        let directory = base.join(format!(
            "native-preview-fixture-{}-{}",
            std::process::id(),
            FIXTURE_IDS.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&directory)
            .map_err(|_| bj_error("fixture_directory", "无法建立独立目录"))?;
        let path = directory.join("profile.sqlite3");
        let store = Arc::new(SqliteStore::open(
            &path,
            "native-preview-profile",
            "native-preview-server",
            1,
        )?);
        let clock = Arc::new(BjAux::new());
        let (tx, rx) = mpsc::channel();
        let hook = Arc::new(NativeHook {
            candidates: vec![
                bj_candidate("main", 100, true),
                bj_candidate("other", 200, true),
            ],
            active: Mutex::new(None),
            lease: Mutex::new(()),
            serial: AtomicU64::new(1),
            stops: AtomicU64::new(0),
            resource_path: Mutex::new(None),
            target_lease_path: Mutex::new(None),
            sender: tx,
            receiver: Mutex::new(Some(rx)),
        });
        let app = ApplicationRuntime::new(
            store.clone(),
            clock.clone(),
            hook.clone(),
            clock.clone(),
            clock.clone(),
            clock.clone(),
        )?;
        app.start()?;
        Ok(Self {
            app,
            store,
            hook,
            clock,
            path,
        })
    }
    pub fn runtime(&self) -> Arc<ApplicationRuntime> {
        self.app.clone()
    }
    pub fn store(&self) -> Arc<SqliteStore> {
        self.store.clone()
    }
    pub fn database_path(&self) -> &Path {
        &self.path
    }
    pub fn connect(&self, id: &str) -> AppResult<AssociationOperationReceipt> {
        let snapshot = self.app.snapshot()?;
        let review = self.app.review_association(AssociationReviewQuery {
            candidate_id: id.into(),
            game_id: None,
            expected_profile_generation: snapshot.profile_generation,
            expected_active_session_id: snapshot
                .session
                .as_ref()
                .filter(|s| matches!(s.state.as_str(), "opening" | "active"))
                .map(|s| s.session_id.clone()),
            prior_review_id: None,
            attribution_intent: Some(AssociationAttributionIntent::SessionOnly),
        })?;
        self.app.associate_reviewed(ReviewedAssociateRequest {
            operation_id: self.clock.new_id("native-associate"),
            review_id: review.review_id,
        })
    }
    pub fn current_hook(&self) -> AppResult<HookSessionInput> {
        self.hook.active()
    }
    pub fn stop_count(&self) -> u64 {
        self.hook.stops.load(Ordering::SeqCst)
    }
    pub fn resource_path(&self, p: Option<PathBuf>) {
        *self.hook.resource_path.lock().unwrap() = p
    }
    pub fn target_lease_path(&self, p: Option<PathBuf>) {
        *self.hook.target_lease_path.lock().unwrap() = p
    }
    pub fn mutate_hook_facts(&self, m: impl FnOnce(&mut HookSessionInput)) -> AppResult<()> {
        let _l = self.hook.lease.lock().unwrap();
        let mut a = self.hook.active.lock().unwrap();
        m(a.as_mut()
            .ok_or_else(|| bj_error("no_active_session", "没有活动宿主"))?);
        Ok(())
    }
    pub fn drain_and_publish(&self) -> AppResult<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.hook
            .sender
            .send(HookUpdate::DrainBarrier { acknowledged: tx })
            .map_err(|_| bj_error("fixture_channel_closed", "Core通道已关闭"))?;
        rx.recv_timeout(Duration::from_secs(3))
            .map_err(|_| bj_error("fixture_drain_timeout", "实际消费未完成"))??;
        self.app.publish()?;
        Ok(())
    }
    pub fn snapshot(&self) -> AppResult<AppSnapshot> {
        self.app.snapshot()
    }
}
