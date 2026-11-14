use crate::{
    application::request_plan::RequestPlan,
    contracts::*,
    domain::{TaskDispatch, capture::*},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, mpsc::Receiver},
};
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = AppResult<T>> + Send + 'a>>;

pub enum HookUpdate {
    /// FIFO acknowledgement is emitted only after the adapter stopped all
    /// native producers and forwarded every accepted event.
    DrainBarrier {
        acknowledged: std::sync::mpsc::SyncSender<AppResult<()>>,
    },
    /// Natural exit drains only this immutable session, never a later target.
    SessionDrainBarrier {
        session_id: String,
        acknowledged: std::sync::mpsc::SyncSender<AppResult<()>>,
    },
    Text(Box<HookTextInput>),
    Control {
        event: Box<HookEventInput>,
        diagnostic: Option<Diagnostic>,
    },
    Health {
        session_id: Option<String>,
        diagnostic: Diagnostic,
    },
}
/// Process/control facts and bounded native diagnostics. Adapters must remove
/// control characters and redact credentials/local paths before persistence;
/// an unchecked native payload must never be forwarded verbatim.
#[derive(Debug, Clone)]
pub struct HookStageInput {
    pub level: String,
    pub stage: String,
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone)]
pub struct HookEventInput {
    pub event_id: String,
    pub operation_id: Option<String>,
    pub session_id: Option<String>,
    pub source: Option<HookSourceInput>,
    pub wire_sequence: Option<String>,
    pub observed_at_ms: i64,
    pub detail: HookStageInput,
    pub lost_count: Option<String>,
    pub integrity: String,
}
/// Rust-only read witness for the independent current-session preview owner.
/// It grants no operation effects and is never deserialized from IPC.
#[derive(Debug, Clone)]
pub struct CurrentSessionPreviewAuthority {
    pub profile: BindingProfileContext,
    pub hook_session: HookSessionInput,
    pub origin: Option<CurrentSessionPreviewOrigin>,
}

/// Rust-only witness for exact obsolete-session cleanup while no Hook session
/// is active. It is rechecked under both application and native Hook ordering.
#[derive(Debug, Clone)]
pub struct InactiveSessionPreviewAuthority {
    pub profile: BindingProfileContext,
}

/// Origin of a successful reviewed association for the exact active witness.
/// Unlike an attempted operation head, this fact survives a later failed switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSessionPreviewOrigin {
    pub operation_id: String,
    pub review_id: String,
    pub candidate: AssociationTargetIdentity,
}

/// Compare the complete Rust-side host witness; no IPC projection can replace these facts.
pub fn same_hook_session_facts(
    left: &crate::domain::capture::HookSessionInput,
    right: &crate::domain::capture::HookSessionInput,
) -> bool {
    left.session_id == right.session_id
        && left.game_id == right.game_id
        && left.environment_key == right.environment_key
        && left.target_exe_path == right.target_exe_path
        && left.windows_pid == right.windows_pid
        && left.process_created_u64 == right.process_created_u64
        && left.architecture == right.architecture
        && left.broker_instance_id == right.broker_instance_id
        && left.target_identity_sha256 == right.target_identity_sha256
        && left.resource.snapshot_id == right.resource.snapshot_id
        && left.resource.kind == right.resource.kind
        && left.resource.version == right.resource.version
        && left.resource.manifest_sha256 == right.resource.manifest_sha256
        && left.resource.contract_version == right.resource.contract_version
        && left.resource.metadata_json == right.resource.metadata_json
}

/// Immutable native context, read without probing, injection or lease replacement.
#[derive(Debug, Clone)]
pub struct AssociationBackendContext {
    pub candidate: TargetCandidate,
    pub active: Option<HookSessionInput>,
}
/// Compared again under the native association lease before any old target is stopped.
#[derive(Debug, Clone)]
pub struct AssociationExecutionGuard {
    pub expected_active: Option<HookSessionInput>,
    pub candidate_identity_sha256: String,
    pub expected_active_session_id: Option<String>,
    pub expected_active_target_identity_sha256: Option<String>,
    pub expected_broker_instance_id: Option<String>,
}
/// Application-owned context; never deserialized from UI or used to skip writer validation.
#[derive(Debug, Clone)]
pub struct SourceSelectionCommit {
    pub request: BeginReadingRequest,
    pub profile: BindingProfileContext,
    pub active_session: HookSessionInput,
    pub request_sha256: String,
    pub match_snapshot_json: String,
}
#[derive(Debug, Clone)]
pub struct AssociationJournalFact {
    pub event_id: String,
    pub ingest_sequence: String,
    pub session_id: Option<String>,
    pub observed_at_ms: i64,
    pub level: String,
    pub stage: String,
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone)]
pub struct AssociationOperationFacts {
    pub profile: BindingProfileContext,
    pub operation_id: String,
    pub request_sha256: String,
    pub session_id: Option<String>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub events: Vec<AssociationJournalFact>,
}
pub trait HookBackend: Send + Sync {
    fn active_session_context(&self) -> AppResult<Option<HookSessionInput>> {
        Err(binding_journey_unavailable())
    }
    /// Bounded synchronous callback under the native association lease. It must
    /// not call association, disconnect or another lease-owning Hook method.
    fn with_active_session_guard(
        &self,
        _expected: &HookSessionInput,
        _effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        Err(binding_journey_unavailable())
    }
    /// The callback must be short and may not re-enter any lease-owning Hook
    /// operation. A bare check followed by an unlocked callback is insufficient.
    fn with_no_active_session_guard(
        &self,
        _effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        Err(binding_journey_unavailable())
    }
    fn association_context(&self, _candidate_id: &str) -> AppResult<AssociationBackendContext> {
        Err(binding_journey_unavailable())
    }
    fn associate_reviewed_traced(
        &self,
        _request: AssociateRequest,
        _guard: &AssociationExecutionGuard,
        _record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        Err(binding_journey_unavailable())
    }
    fn cover_session_guard(&self) -> AppResult<CoverSessionGuard> {
        Err(AppError::new(
            "game_cover_binding_unavailable",
            "library",
            "当前没有可保存封面的已核验窗口",
        ))
    }
    /// Final short SQL-only cover commit under the current association lease.
    fn with_verified_cover_binding_commit(
        &self,
        _request: GameWindowActivationRequest,
        _commit: &mut dyn FnMut() -> AppResult<CoverInstallOutcome>,
    ) -> AppResult<CoverInstallOutcome> {
        Err(AppError::new(
            "game_cover_binding_unavailable",
            "library",
            "当前接入不支持核验封面归属",
        ))
    }
    fn game_window_binding(&self) -> AppResult<GameWindowBindingProjection> {
        Err(AppError::new(
            "game_window_unavailable",
            "window",
            "当前接入适配器未提供窗口核验",
        ))
    }
    fn game_window_activation_request(&self) -> AppResult<GameWindowActivationRequest> {
        Err(AppError::new(
            "game_window_unavailable",
            "window",
            "当前接入适配器未提供可信窗口身份",
        ))
    }
    fn show_associated_game(
        &self,
        _request: GameWindowActivationRequest,
    ) -> AppResult<GameWindowActivationReceipt> {
        Err(AppError::new(
            "game_window_unavailable",
            "window",
            "当前接入适配器未提供显示游戏",
        ))
    }
    fn targets(&self) -> AppResult<TargetCandidates>;
    /// Resolve a previously issued opaque candidate without discovery or native side effects.
    fn association_candidate(&self, _candidate_id: &str) -> AppResult<TargetCandidate> {
        Err(AppError::new(
            "association_target_unavailable",
            "capture",
            "当前接入适配器不能核对游戏资料与运行目标",
        ))
    }
    /// Persist under the verified native ownership lease; publish attribution only after commit.
    fn with_associated_session_commit(
        &self,
        _input: &HookSessionInput,
        commit: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        commit()
    }
    /// Request a launch only. Target review and Hook association remain explicit.
    fn launch_game(&self, _target: &GameTarget) -> AppResult<()> {
        Err(AppError::new(
            "game_launch_unavailable",
            "launch",
            "当前执行环境未提供游戏启动能力",
        ))
    }
    fn associate(&self, request: AssociateRequest) -> AppResult<HookSessionInput>;
    fn associate_traced(
        &self,
        request: AssociateRequest,
        _record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        self.associate(request)
    }
    fn disconnect(&self) -> AppResult<()>;
    /// Called off the sole capture consumer. The adapter compares its current
    /// SID and full native target, drains producers and the FIFO consumer, then
    /// releases only that session's lease. False means the request became stale.
    fn finish_disconnected_session(&self, _session_id: &str) -> AppResult<bool> {
        Err(AppError::new(
            "capture_cleanup_unavailable",
            "capture",
            "此接入适配器尚未提供自然退出收尾",
        ))
    }
    fn stop_and_drain(
        &self,
        _acknowledged: std::sync::mpsc::SyncSender<AppResult<()>>,
    ) -> AppResult<()> {
        Err(AppError::new(
            "capture_drain_unavailable",
            "capture",
            "此接入适配器尚未提供生产停止与消费确认协议",
        ))
    }
    /// The adapter serializes preflight, identity check, reversible local
    /// association removal and the durable commit under its binding lease.
    /// Replay is checked before current identity and must have no native effect.
    fn unlink_window(
        &self,
        _request: GameWindowActivationRequest,
        _preflight: &mut dyn FnMut() -> AppResult<Option<CommandReceipt>>,
        _commit: &mut dyn FnMut() -> AppResult<CommandReceipt>,
    ) -> AppResult<CommandReceipt> {
        Err(AppError::new(
            "window_unbind_unavailable",
            "window",
            "当前接入适配器未提供可补偿的窗口解绑",
        ))
    }
    fn subscribe(&self) -> AppResult<Receiver<HookUpdate>>;
    fn resource_info(&self) -> ResourceInfo;
}
pub trait ModelGateway: Send + Sync {
    /// Explicit saved-source test. The original prepared Arc, live budget and
    /// repository must be revalidated around each material or transport boundary.
    fn execute_saved_service_test<'a>(
        &'a self,
        _prepared: Arc<crate::ports::service_testing::PreparedServiceTest>,
        _repository: Arc<dyn crate::ports::service_testing::ServiceTestingRepository>,
        _budget: crate::ports::operation_budget::OperationBudget,
        _observation: Arc<crate::ports::service_test_execution::ServiceTestExecutionObservation>,
    ) -> PortFuture<'a, String> {
        Box::pin(async {
            Err(AppError::new(
                "service_test_execution_unavailable",
                "service_test",
                "当前适配器尚未提供保存服务测试",
            ))
        })
    }

    /// Cache reuse still belongs to an actual current attempt. A provider which
    /// requires a real short-lived authorization records it without retranslating
    /// or sending the cached source. Old legacy cache behavior is unchanged.
    fn validate_cached_service<'a>(
        &'a self,
        _task: &'a crate::domain::service_dispatch::ServiceTaskDispatch,
    ) -> PortFuture<'a, ()> {
        Box::pin(async {
            Err(AppError::new(
                "service_cache_authority_unavailable",
                "provider",
                "当前适配器未提供缓存复用授权核验",
            ))
        })
    }
    /// Genuine codec3 target, already admitted by the repository. Adapters must
    /// still acquire and recheck the actual attempt's authority before I/O.
    fn execute_service<'a>(
        &'a self,
        _task: &'a crate::domain::service_dispatch::ServiceTaskDispatch,
        _on_delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async {
            Err(AppError::new(
                "service_execution_unavailable",
                "provider",
                "当前执行适配器未提供版本化服务执行",
            ))
        })
    }
    fn execute<'a>(
        &'a self,
        task: &'a TaskDispatch,
        plan: &'a RequestPlan,
        on_delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String>;
    fn test_request<'a>(
        &'a self,
        connection: &'a Connection,
        plan: &'a RequestPlan,
        credential_ref: Option<&'a str>,
    ) -> PortFuture<'a, String>;
    fn discover_models<'a>(
        &'a self,
        connection: &'a Connection,
        credential_ref: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>>;
}
pub trait LanguageServices: Send + Sync {
    fn analysis_revision(&self) -> &'static str {
        "external-language-v1"
    }
    fn analyze<'a>(&'a self, text: &'a str) -> PortFuture<'a, Vec<LexicalToken>>;
    fn lookup(&self, token: &LexicalToken) -> AppResult<Vec<DictionaryEntry>>;
    fn resources(&self) -> Vec<ResourceInfo>;
}
pub trait CredentialVault: Send + Sync {
    fn put(&self, immutable_version_id: &str, secret: &[u8]) -> AppResult<()>;
    fn get(&self, immutable_version_id: &str) -> AppResult<Vec<u8>>;
    fn delete(&self, immutable_version_id: &str) -> AppResult<()>;
}
