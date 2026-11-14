//! Application lifetime is independent of every webview. No platform, network or SQL implementation here.
use crate::{
    application::request_plan,
    contracts::*,
    domain::{
        TaskCompletion,
        capture::{CaptureFlush, CaptureFlushReason, CaptureOutcome, TokenSnapshotInput},
        service_dispatch::WorkDispatch,
    },
    ports::{IdentitySource, ProductRepository, runtime::*},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, watch};

pub struct ApplicationRuntime {
    pub(super) binding_journey: Mutex<super::binding_journey::BindingJourneyState>,
    pub(super) service_model_discovery: Mutex<Option<Arc<super::service_discovery::ServiceModelDiscoveryController>>>,
    pub(super) google_interactive: Mutex<Option<Arc<super::google_interactive::GoogleInteractiveController>>>,
    pub(super) service_testing: Mutex<Option<Arc<super::service_testing::ServiceTestingController>>>,
    pub(super) service_configuration:
        Mutex<Option<Arc<super::service_configuration::ServiceConfigurationController>>>,
    pub(super) game_assets: Mutex<Option<Arc<dyn crate::ports::GameCoverAssets>>>,
    pub(super) repository: Arc<dyn ProductRepository>,
    pub(super) clock: Arc<dyn IdentitySource>,
    pub(super) hook: Arc<dyn HookBackend>,
    pub(super) gateway: Arc<dyn ModelGateway>,
    language: Arc<dyn LanguageServices>,
    pub(super) vault: Arc<dyn CredentialVault>,
    projection: watch::Sender<ProjectionUpdate>,
    projection_lock: Mutex<()>,
    pub(super) capture_lock: Mutex<()>,
    learning: Mutex<Option<LearningObject>>,
    partials: Mutex<BTreeMap<String, AttemptPartial>>,
    active_attempts: Mutex<BTreeMap<String, String>>,
    tasks: Mutex<BTreeMap<String, tokio::task::AbortHandle>>,
    schedulers: Mutex<BTreeMap<String, Arc<PurposeScheduler>>>,
    pub(super) translation_gate: Mutex<()>,
    controls: Mutex<BTreeMap<String, (bool, bool)>>,
    lexical_states: Mutex<BTreeMap<(String, u32, String), LexicalState>>,
    pub(super) closing: AtomicBool,
    capture_started: AtomicBool,
    capture_stopping: AtomicBool,
    /// Terminal SIDs remain ineligible even if asynchronous cleanup fails.
    /// A later association always has a new immutable session identity.
    natural_disconnects: Mutex<BTreeSet<String>>,
    pending_integrity: Mutex<BTreeMap<String, HookEventInput>>,
    export_sessions: Mutex<BTreeMap<String, String>>,
}
struct AttemptPartial {
    attempt_id: String,
    text: String,
}
struct PurposeScheduler {
    running: Arc<Semaphore>,
    pending: Arc<Semaphore>,
}
fn poison() -> AppError {
    AppError::new("application_state", "application", "应用状态锁不可用")
}
impl ApplicationRuntime {
    pub fn new(
        repository: Arc<dyn ProductRepository>,
        clock: Arc<dyn IdentitySource>,
        hook: Arc<dyn HookBackend>,
        gateway: Arc<dyn ModelGateway>,
        language: Arc<dyn LanguageServices>,
        vault: Arc<dyn CredentialVault>,
    ) -> AppResult<Arc<Self>> {
        let mut snapshot = repository.snapshot()?;
        snapshot.revision = 1;
        let (projection, _) = watch::channel(ProjectionUpdate {
            projection_id: "application".into(),
            server_instance_id: snapshot.server_instance_id.clone(),
            profile_generation: snapshot.profile_generation,
            base_revision: 0,
            revision: 1,
            snapshot,
        });
        Ok(Arc::new(Self {
            binding_journey: Mutex::new(Default::default()),
            service_model_discovery: Mutex::new(None),
            google_interactive: Mutex::new(None),
            service_configuration: Mutex::new(None),
            service_testing: Mutex::new(None),
            game_assets: Mutex::new(None),
            repository,
            clock,
            hook,
            gateway,
            language,
            vault,
            projection,
            projection_lock: Mutex::new(()),
            capture_lock: Mutex::new(()),
            learning: Mutex::new(None),
            partials: Mutex::new(BTreeMap::new()),
            active_attempts: Mutex::new(BTreeMap::new()),
            tasks: Mutex::new(BTreeMap::new()),
            schedulers: Mutex::new(BTreeMap::new()),
            translation_gate: Mutex::new(()),
            controls: Mutex::new(BTreeMap::new()),
            lexical_states: Mutex::new(BTreeMap::new()),
            closing: AtomicBool::new(false),
            capture_started: AtomicBool::new(false),
            capture_stopping: AtomicBool::new(false),
            natural_disconnects: Mutex::new(BTreeSet::new()),
            pending_integrity: Mutex::new(BTreeMap::new()),
            export_sessions: Mutex::new(BTreeMap::new()),
        }))
    }
    pub fn snapshot(&self) -> AppResult<AppSnapshot> {
        self.publish()?;
        Ok(self.projection.borrow().snapshot.clone())
    }
    pub fn subscribe(&self) -> watch::Receiver<ProjectionUpdate> {
        self.projection.subscribe()
    }
    pub async fn shutdown(self: &Arc<Self>) -> AppResult<()> {
        let result = self.shutdown_inner().await;
        if let Err(error) = &result {
            // The desktop may restore its main window after an exit failure.
            // Keep the consumer alive so explicit retry/association can work.
            self.closing.store(false, Ordering::Release);
            if let Ok(Some(controller)) = self.service_model_discovery.lock().map(|v|v.clone()) { controller.reopen_after_failed_shutdown(); }
            if let Ok(Some(controller)) = self.google_interactive.lock().map(|v|v.clone()) { controller.reopen_after_failed_shutdown(); }
            if let Ok(Some(controller)) = self.service_testing.lock().map(|v|v.clone()) { controller.reopen_after_failed_shutdown(); }
            self.diagnostic(error.clone().into());
        }
        result
    }
    async fn shutdown_inner(self: &Arc<Self>) -> AppResult<()> {
        {
            let _gate = self.translation_gate.lock().map_err(|_| poison())?;
            self.closing.store(true, Ordering::Release);
        }
        let discovery = self.service_model_discovery.lock().map_err(|_|poison())?.clone();
        if let Some(controller) = discovery { controller.stop_and_drain().await?; }
        let google_interactive = self.google_interactive.lock().map_err(|_|poison())?.clone();
        if let Some(controller) = google_interactive { controller.stop_and_drain().await?; }
        let service_tests = self.service_testing.lock().map_err(|_|poison())?.clone();
        if let Some(controller) = service_tests { controller.stop_and_drain().await?; }
        if let Err(error) = self.stop_capture_pipeline().await {
            // Even an unsuccessful stop is a reading boundary. Preserve the
            // accepted partial sentence without sending it to a model, so a
            // restored desktop session cannot append its next line to it.
            let app = self.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _capture = app.capture_lock.lock().map_err(|_| poison())?;
                app.repository.flush_hook_capture(
                    CaptureFlush {
                        session_id: None,
                        reason: CaptureFlushReason::Shutdown,
                    },
                    app.clock.now_ms(),
                )?;
                app.publish()
            })
            .await;
            return Err(error);
        }
        let app = self.clone();
        tokio::task::spawn_blocking(move || {
            let _capture = app.capture_lock.lock().map_err(|_| poison())?;
            app.repository.flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Shutdown,
                },
                app.clock.now_ms(),
            )?;
            app.publish()
        })
        .await
        .map_err(|_| poison())??;
        // Persist cancellation before dropping network futures or exiting. A
        // clean shutdown cannot leave a paid request authorized in the profile.
        let repo = self.repository.clone();
        let tasks = tokio::task::spawn_blocking(move || repo.snapshot())
            .await
            .map_err(|_| poison())??
            .tasks;
        for task in tasks
            .into_iter()
            .filter(|t| matches!(t.status.as_str(), "queued" | "running"))
        {
            self.dispatch(CommandEnvelope {
                operation_id: self.clock.new_id("shutdown-cancel"),
                expected_revision: None,
                command: Command::CancelTask {
                    task_id: task.task_id,
                },
            })
            .await?;
        }
        let app = self.clone();
        tokio::task::spawn_blocking(move || {
            let _capture = app.capture_lock.lock().map_err(|_| poison())?;
            app.flush_pending_integrity()?;
            app.repository.dispatch(
                CommandEnvelope {
                    operation_id: app.clock.new_id("shutdown-disconnect"),
                    expected_revision: None,
                    command: Command::Disconnect,
                },
                app.clock.now_ms(),
            )?;
            app.publish()
        })
        .await
        .map_err(|_| poison())?
    }
    fn note_unconfirmed(&self, session: &str) {
        if let Ok(mut pending) = self.pending_integrity.lock() {
            pending
                .entry(session.into())
                .or_insert_with(|| HookEventInput {
                    event_id: self.clock.new_id("storage-unconfirmed"),
                    operation_id: None,
                    session_id: Some(session.into()),
                    source: None,
                    wire_sequence: None,
                    observed_at_ms: self.clock.now_ms(),
                    detail: HookStageInput {
                        level: "error".into(),
                        stage: "capture".into(),
                        code: "storage_unconfirmed".into(),
                        message: "接收事实未能确认持久，未知范围保留；请检查存储".into(),
                    },
                    lost_count: None,
                    integrity: "unknown".into(),
                });
        }
    }
    fn flush_pending_integrity_for(&self, session: Option<&str>) -> AppResult<()> {
        self.repository
            .flush_capture_integrity(session.map(str::to_owned))?;
        let facts = self
            .pending_integrity
            .lock()
            .map_err(|_| poison())?
            .values()
            .filter(|p| session.is_none() || p.session_id.as_deref() == session)
            .cloned()
            .collect::<Vec<_>>();
        for fact in facts {
            self.repository.record_hook_event(fact.clone())?;
            if let Some(session) = fact.session_id {
                let mut pending = self.pending_integrity.lock().map_err(|_| poison())?;
                if pending
                    .get(&session)
                    .is_some_and(|p| p.event_id == fact.event_id)
                {
                    pending.remove(&session);
                }
            }
        }
        Ok(())
    }
    fn flush_pending_integrity(&self) -> AppResult<()> {
        self.flush_pending_integrity_for(None)
    }
    async fn stop_capture_pipeline(self: &Arc<Self>) -> AppResult<()> {
        if self.capture_stopping.swap(true, Ordering::AcqRel) {
            return Err(AppError::new(
                "capture_drain_busy",
                "capture",
                "接收收尾已在执行",
            ));
        }
        let app = self.clone();
        let result = tokio::task::spawn_blocking(move || {
            if app.capture_started.load(Ordering::Acquire) {
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                app.hook.stop_and_drain(tx)?;
                rx.recv_timeout(Duration::from_secs(20)).map_err(|_| {
                    AppError::new(
                        "capture_drain_pending",
                        "capture",
                        "接收事实尚未完成持久确认，拒绝退出",
                    )
                })??;
            } else {
                app.hook.disconnect()?;
                let _guard = app.capture_lock.lock().map_err(|_| poison())?;
                app.flush_pending_integrity()?;
            }
            Ok::<_, AppError>(())
        })
        .await
        .map_err(|_| poison())
        .and_then(|r| r);
        if result.is_err() {
            // No complete producer/relay/consumer acknowledgement was obtained.
            // Retain this uncertainty before a later stopped/failed transition.
            let session = self
                .projection
                .borrow()
                .snapshot
                .session
                .as_ref()
                .map(|s| s.session_id.clone());
            if let Some(session) = session {
                self.note_unconfirmed(&session);
                let app = self.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    app.flush_pending_integrity_for(Some(&session))
                })
                .await;
            }
        }
        self.capture_stopping.store(false, Ordering::Release);
        result
    }
    pub fn publish(&self) -> AppResult<()> {
        let _lock = self.projection_lock.lock().map_err(|_| poison())?;
        let old = self.projection.borrow().clone();
        let mut snapshot = self.repository.snapshot()?;
        snapshot.learning_object = self.learning.lock().map_err(|_| poison())?.clone();
        snapshot.lexical_states = self
            .lexical_states
            .lock()
            .map_err(|_| poison())?
            .values()
            .cloned()
            .collect();
        if let Some(session) = &mut snapshot.session {
            let controls = self.controls.lock().map_err(|_| poison())?;
            if let Some((paused, translation)) = controls.get(&session.session_id) {
                session.paused = *paused;
                session.translation_enabled = *translation;
            }
        }
        let partials = self.partials.lock().map_err(|_| poison())?;
        for task in &mut snapshot.tasks {
            task.partial_text = partials.get(&task.task_id).and_then(|partial| {
                (task.attempt_id.as_deref() == Some(partial.attempt_id.as_str())
                    && matches!(task.status.as_str(), "queued" | "running"))
                .then(|| partial.text.clone())
            });
        }
        // A repository sample may predate a newly registered retry buffer.
        // Only task lifecycle cleanup owns deletion; an old sample cannot prune it.
        drop(partials);
        snapshot.revision = old.revision;
        if snapshot == old.snapshot {
            return Ok(());
        }
        let revision = old.revision.checked_add(1).ok_or_else(|| {
            AppError::new(
                "revision_exhausted",
                "application",
                "投影版本已耗尽，请重启应用",
            )
        })?;
        snapshot.revision = revision;
        self.projection.send_replace(ProjectionUpdate {
            projection_id: "application".into(),
            server_instance_id: snapshot.server_instance_id.clone(),
            profile_generation: snapshot.profile_generation,
            base_revision: old.revision,
            revision,
            snapshot,
        });
        Ok(())
    }
    pub fn start(self: &Arc<Self>) -> AppResult<()> {
        if self.capture_started.swap(true, Ordering::AcqRel) {
            return Err(AppError::new(
                "capture_already_started",
                "capture",
                "接收协调器已启动",
            ));
        }
        let receiver = match self.hook.subscribe() {
            Ok(receiver) => receiver,
            Err(error) => {
                self.capture_started.store(false, Ordering::Release);
                return Err(error);
            }
        };
        let weak = Arc::downgrade(self);
        let handle = tokio::runtime::Handle::current();
        std::thread::Builder::new()
            .name("capture-application".into())
            .spawn(move || {
                loop {
                    let update = match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(u) => Some(u),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if weak.strong_count() == 0 {
                                break;
                            }
                            None
                        }
                        Err(_) => break,
                    };
                    let Some(runtime) = weak.upgrade() else {
                        break;
                    };
                    if let Some(update) = update {
                        match update {
                            HookUpdate::DrainBarrier { acknowledged } => {
                                let result =
                                    runtime.capture_lock.lock().map_err(|_| poison()).and_then(
                                        |_guard| {
                                            runtime.flush_pending_integrity()?;
                                            runtime.repository.flush_hook_capture(
                                                CaptureFlush {
                                                    session_id: None,
                                                    reason: CaptureFlushReason::Shutdown,
                                                },
                                                runtime.clock.now_ms(),
                                            )?;
                                            runtime.publish()
                                        },
                                    );
                                let _ = acknowledged.send(result);
                            }
                            HookUpdate::SessionDrainBarrier {
                                session_id,
                                acknowledged,
                            } => {
                                let result = runtime
                                    .capture_lock
                                    .lock()
                                    .map_err(|_| poison())
                                    .and_then(|_guard| {
                                        runtime.flush_pending_integrity_for(Some(&session_id))?;
                                        // The ACK covers both the last accepted facts
                                        // and the durable terminal lifecycle. The
                                        // native lease cannot be released before this.
                                        runtime.repository.record_hook_diagnostic(
                                            Some(session_id),
                                            Diagnostic {
                                                code: "game_disconnected".into(),
                                                stage: "capture".into(),
                                                message: "游戏已退出或断开，历史保留".into(),
                                                recoverable: true,
                                            },
                                            runtime.clock.now_ms(),
                                        )?;
                                        runtime.publish()
                                    });
                                let _ = acknowledged.send(result);
                            }
                            HookUpdate::Control { event, diagnostic } => {
                                let session = event.session_id.clone();
                                let result =
                                    runtime.capture_lock.lock().map_err(|_| poison()).and_then(
                                        |_guard| runtime.repository.record_hook_event(*event),
                                    );
                                if let Err(error) = result {
                                    if let Some(session) = session.as_deref() {
                                        runtime.note_unconfirmed(session);
                                    }
                                    runtime.diagnostic(error.into());
                                }
                                if let Some(diagnostic) = diagnostic {
                                    if diagnostic.code == "game_disconnected"
                                        && let Some(session) = session
                                    {
                                        // Never wait for the FIFO's own barrier from
                                        // its only consumer. Keep accepting tail facts.
                                        runtime.schedule_natural_disconnect(session, &handle);
                                    } else {
                                        runtime.hook_diagnostic(session, diagnostic);
                                    }
                                }
                                let _ = runtime.publish();
                            }
                            HookUpdate::Health {
                                session_id,
                                diagnostic,
                            } => {
                                let Ok(_capture) = runtime.capture_lock.lock() else {
                                    break;
                                };
                                if let Some(session_id) = session_id.as_ref() {
                                    let event = HookEventInput {
                                        event_id: runtime.clock.new_id("hook-health"),
                                        operation_id: None,
                                        session_id: Some(session_id.clone()),
                                        source: None,
                                        wire_sequence: None,
                                        observed_at_ms: runtime.clock.now_ms(),
                                        detail: HookStageInput {
                                            level: if diagnostic.code == "integrity_lost" {
                                                "error"
                                            } else {
                                                "warning"
                                            }
                                            .into(),
                                            stage: "capture".into(),
                                            code: "capture_health".into(),
                                            message: "接收协调器报告运行状态变化；详情见当前诊断"
                                                .into(),
                                        },
                                        lost_count: None,
                                        integrity: if diagnostic.code == "integrity_lost" {
                                            "lost"
                                        } else {
                                            "unknown"
                                        }
                                        .into(),
                                    };
                                    if let Err(error) = runtime.repository.record_hook_event(event)
                                    {
                                        runtime.note_unconfirmed(session_id);
                                        runtime.diagnostic(error.into());
                                    }
                                }
                                runtime.hook_diagnostic(session_id, diagnostic);
                            }
                            HookUpdate::Text(input) => {
                                let session_id = input.session_id.clone();
                                let result = runtime
                                    .capture_lock
                                    .lock()
                                    .map_err(|_| poison())
                                    .and_then(|_guard| {
                                        let integrity_ready = runtime
                                            .flush_pending_integrity_for(Some(&input.session_id))
                                            .is_ok();
                                        let paused = runtime
                                            .controls
                                            .lock()
                                            .map_err(|_| poison())?
                                            .get(&input.session_id)
                                            .copied()
                                            .is_some_and(|(paused, _)| paused);
                                        let terminal = runtime
                                            .natural_disconnects
                                            .lock()
                                            .map_err(|_| poison())?
                                            .contains(&input.session_id);
                                        runtime.repository.ingest_hook_text_with_policy(
                                            *input,
                                            !paused
                                                && !terminal
                                                && integrity_ready
                                                && !runtime.closing.load(Ordering::Acquire)
                                                && !runtime
                                                    .capture_stopping
                                                    .load(Ordering::Acquire),
                                            runtime.clock.now_ms(),
                                        )
                                    });
                                match result {
                                    Ok(outcome) => runtime.schedule_capture(outcome, &handle),
                                    Err(error) => {
                                        runtime.note_unconfirmed(&session_id);
                                        runtime.diagnostic(error.into());
                                    }
                                }
                                let _ = runtime.publish();
                            }
                        }
                    }
                    // Facts above are always handled. During stop, only the
                    // explicit producer/relay barrier seals capture; no model work.
                    if runtime.closing.load(Ordering::Acquire)
                        || runtime.capture_stopping.load(Ordering::Acquire)
                    {
                        continue;
                    }
                    let due =
                        runtime
                            .capture_lock
                            .lock()
                            .map_err(|_| poison())
                            .and_then(|_guard| {
                                let current = runtime.repository.snapshot()?.session;
                                if current.is_some_and(|session| {
                                    runtime
                                        .natural_disconnects
                                        .lock()
                                        .map(|closed| closed.contains(&session.session_id))
                                        .unwrap_or(true)
                                }) {
                                    return Ok(vec![]);
                                }
                                runtime.repository.flush_hook_capture(
                                    CaptureFlush {
                                        session_id: None,
                                        reason: CaptureFlushReason::Due,
                                    },
                                    runtime.clock.now_ms(),
                                )
                            });
                    match due {
                        Ok(versions) => {
                            if !versions.is_empty() {
                                runtime.schedule_capture(
                                    CaptureOutcome {
                                        visible_version: None,
                                        finalized_versions: versions,
                                    },
                                    &handle,
                                );
                                let _ = runtime.publish();
                            }
                        }
                        Err(error) => runtime.diagnostic(error.into()),
                    }
                }
            })
            .map_err(|_| {
                self.capture_started.store(false, Ordering::Release);
                AppError::new("capture_consumer", "capture", "无法启动接收协调器")
            })?;
        Ok(())
    }
    pub fn targets(&self) -> AppResult<TargetCandidates> {
        self.hook.targets()
    }
    pub fn launch_game(&self, game_id: &str, row_revision: u32) -> AppResult<()> {
        crate::domain::require_id(game_id)?;
        if self.closing.load(Ordering::Acquire) {
            return Err(AppError::new(
                "application_closing",
                "launch",
                "应用正在退出",
            ));
        }
        let game = self
            .repository
            .snapshot()?
            .games
            .into_iter()
            .find(|game| game.game_id == game_id)
            .ok_or_else(|| AppError::new("game_missing", "library", "游戏记录不存在"))?;
        if game.row_revision != row_revision {
            return Err(AppError::new(
                "revision_conflict",
                "library",
                "游戏资料已更新，请刷新后重新核对启动目标",
            ));
        }
        // Do not dispatch, publish a session, or inject as a side effect of launch.
        self.hook.launch_game(&game.target)
    }
    fn schedule_capture(
        self: &Arc<Self>,
        outcome: CaptureOutcome,
        handle: &tokio::runtime::Handle,
    ) {
        if let Some(visible) = outcome.visible_version
            && !outcome.finalized_versions.contains(&visible)
        {
            let app = self.clone();
            let operation = self.clock.new_id("capture-visible");
            handle.spawn(async move {
                app.process_unit(visible.unit_id, visible.text_revision, operation, false)
                    .await;
            });
        }
        for version in outcome.finalized_versions {
            let app = self.clone();
            let operation = format!(
                "capture-final:{}:{}",
                version.unit_id, version.text_revision
            );
            handle.spawn(async move {
                app.process_unit(version.unit_id, version.text_revision, operation, true)
                    .await;
            });
        }
    }
    pub fn associate(&self, mut request: AssociateRequest) -> AppResult<CommandReceipt> {
        let _lock = self.capture_lock.lock().map_err(|_| poison())?;
        match self.hook.association_context(&request.candidate_id) {
            Ok(context) => {
                let target = crate::domain::content_hash(format!("{}\0{}\0{}\0{}\0{}",
                    context.candidate.environment_key, context.candidate.executable_path,
                    context.candidate.windows_pid, context.candidate.process_created_u64,
                    context.candidate.architecture));
                if context.active.as_ref().is_some_and(|active| active.target_identity_sha256 != target) {
                    return Err(AppError::new("reviewed_association_required", "capture", "切换游戏需要重新核对当前会话，请使用确认后的连接入口"));
                }
                // The legacy Boolean is never a cross-target stop authority.
                request.confirm_switch = false;
            }
            Err(error) if error.code == "binding_journey_unavailable" && !request.confirm_switch => {
                // Old adapters remain usable only without replacement authority;
                // their pre-existing false-switch guard must still reject a switch.
            }
            Err(error) if error.code == "binding_journey_unavailable" => {
                return Err(AppError::new("reviewed_association_required", "capture", "旧确认参数不能授权切换，请重新核对运行目标"));
            }
            Err(error) => return Err(error),
        }
        self.associate_guarded_locked(request, None)
    }
    pub(super) fn associate_guarded_locked(
        &self, request: AssociateRequest, guard: Option<&AssociationExecutionGuard>,
    ) -> AppResult<CommandReceipt> {
        if self.closing.load(Ordering::Acquire) || self.capture_stopping.load(Ordering::Acquire) {
            return Err(AppError::new(
                "application_closing",
                "capture",
                "应用或接收正在收尾",
            ));
        }
        self.flush_pending_integrity()?;
        let before = self.repository.snapshot()?;
        if let Some(game_id) = &request.game_id {
            let game = before
                .games
                .iter()
                .find(|game| &game.game_id == game_id)
                .ok_or_else(|| {
                    AppError::new("game_not_active", "capture", "游戏资料已不存在，请重新选择")
                })?;
            let candidate = self.hook.association_candidate(&request.candidate_id)?;
            let mut saved = GameInput {
                game_id: Some(game.game_id.clone()),
                row_revision: Some(game.row_revision),
                display_name: game.display_name.clone(),
                original_title: game.original_title.clone(),
                source_language: game.source_language.clone(),
                cover_asset_ref: game.cover_asset_ref.clone(),
                target: game.target.clone(),
            };
            let declared_path = saved
                .target
                .canonical_exe_path
                .as_deref()
                .map(crate::domain::normalize_windows_path)
                .transpose()?
                .map(|(_, key)| key);
            crate::domain::normalize_game_input(&mut saved)?;
            let mut actual = saved.clone();
            actual.target.environment_kind = "crossover".into();
            actual.target.environment_key = candidate.environment_key.clone();
            actual.target.bottle_locator = candidate
                .environment_key
                .strip_prefix("crossover:")
                .map(str::to_owned);
            actual.target.canonical_exe_path = Some(candidate.executable_path);
            actual.target.architecture = Some(candidate.architecture);
            actual.target.verification_state = "verified".into();
            crate::domain::normalize_game_input(&mut actual)?;
            crate::domain::validate_game(&actual)?;
            if saved.target.environment_kind == "crossover"
                && saved.target.environment_key != actual.target.environment_key
                || declared_path.is_some() && declared_path != actual.target.target_path_key
                || declared_path.is_some()
                    && saved.target.architecture.is_some()
                    && saved.target.architecture != actual.target.architecture
            {
                return Err(AppError::new(
                    "game_target_conflict",
                    "capture",
                    "游戏资料的环境、完整路径或架构与运行目标不一致，请重新选择归属",
                ));
            }
        }
        let operation_id = request.operation_id.clone();
        self.repository.begin_capture_operation(
            operation_id.clone(),
            crate::domain::canonical_hash(&request)?,
            self.clock.now_ms(),
        )?;
        let mut trace = |detail: HookStageInput| {
            self.repository.record_hook_event(HookEventInput {
                event_id: self.clock.new_id("hook-stage"),
                operation_id: Some(operation_id.clone()),
                session_id: None,
                source: None,
                wire_sequence: None,
                observed_at_ms: self.clock.now_ms(),
                detail,
                lost_count: None,
                integrity: "intact".into(),
            })
        };
        trace(HookStageInput {
            level: "info".into(),
            stage: "association".into(),
            code: "association_started".into(),
            message: "已开始核对目标并接入".into(),
        })?;
        let input_result = match guard {
            Some(guard) => self.hook.associate_reviewed_traced(request, guard, &mut trace),
            None => self.hook.associate_traced(request, &mut trace),
        };
        let input = match input_result {
            Ok(input) => input,
            Err(error) => {
                let _ = trace(HookStageInput {
                    level: "error".into(),
                    stage: "association".into(),
                    code: "association_failed".into(),
                    message: "目标核对、资源或接入阶段失败，请重新核对".into(),
                });
                let _ = self.repository.finish_capture_operation(
                    operation_id,
                    None,
                    self.clock.now_ms(),
                );
                return Err(error);
            }
        };
        let session_id = input.session_id.clone();
        let mut commit = || {
            self.repository
                .open_hook_session(input.clone(), self.clock.now_ms())
        };
        if let Err(error) = self
            .hook
            .with_associated_session_commit(&input, &mut commit)
        {
            // A failed attribution retry must not tear down an existing valid capture.
            if before
                .session
                .as_ref()
                .is_none_or(|session| session.session_id != session_id)
            {
                let _ = self.hook.disconnect();
            }
            let _ =
                self.repository
                    .finish_capture_operation(operation_id, None, self.clock.now_ms());
            return Err(error);
        }
        let persisted = self
            .repository
            .finish_capture_operation(
                operation_id.clone(),
                Some(session_id.clone()),
                self.clock.now_ms(),
            )
            .and_then(|()| {
                self.repository.record_hook_event(HookEventInput {
                    event_id: self.clock.new_id("hook-associated"),
                    operation_id: Some(operation_id.clone()),
                    session_id: Some(session_id.clone()),
                    source: None,
                    wire_sequence: None,
                    observed_at_ms: self.clock.now_ms(),
                    detail: HookStageInput {
                        level: "info".into(),
                        stage: "association".into(),
                        code: "association_ready".into(),
                        message: "已关联核验目标，等待Core回调与正文候选".into(),
                    },
                    lost_count: None,
                    integrity: "intact".into(),
                })
            });
        if let Err(error) = persisted {
            let retained = before
                .session
                .as_ref()
                .is_some_and(|session| session.session_id == session_id);
            if !retained {
                let _ = self.hook.disconnect();
            }
            self.diagnostic(Diagnostic {
                code: "capture_failed".into(),
                stage: "storage".into(),
                message: if retained {
                    "接入附加事实保存失败，已保留原会话；请检查存储后重试"
                } else {
                    "接入事实保存失败，已请求断开；请检查存储后重试"
                }
                .into(),
                recoverable: true,
            });
            return Err(error);
        }
        self.publish()?;
        Ok(CommandReceipt {
            request_id: operation_id,
            replayed: false,
            revision: self.repository.snapshot()?.commit_revision,
            result_id: Some(session_id),
            game_results: None,
        })
    }
    pub fn session_records(&self, query: SessionRecordQuery) -> AppResult<SessionRecordPage> {
        super::session_records::validate_query(&query)?;
        let _guard = self.capture_lock.lock().map_err(|_| poison())?;
        self.flush_pending_integrity_for(Some(&query.session_id))?;
        self.repository.session_records(query, self.clock.now_ms())
    }
    pub fn preview_preferences(&self, preferences: Preferences) -> AppResult<Preferences> {
        crate::domain::validate_preferences(&preferences)?;
        self.repository.preview_preferences(preferences)
    }
    pub fn preferences_scope(&self, scope_key: String) -> AppResult<ScopedPreferences> {
        self.repository.preferences_scope(scope_key)
    }
    pub fn game_window_binding(&self) -> AppResult<GameWindowBindingProjection> {
        self.hook.game_window_binding()
    }
    pub fn game_window_activation_request(&self) -> AppResult<GameWindowActivationRequest> {
        self.hook.game_window_activation_request()
    }
    pub fn show_associated_game(
        &self,
        request: GameWindowActivationRequest,
    ) -> AppResult<GameWindowActivationReceipt> {
        if self.closing.load(Ordering::Acquire) || self.capture_stopping.load(Ordering::Acquire) {
            return Err(AppError::new(
                "application_closing",
                "window",
                "接收正在收尾，不能切换游戏窗口",
            ));
        }
        self.hook.show_associated_game(request)
    }
    pub fn prepare_session_export(
        &self,
        request: SessionExportRequest,
    ) -> AppResult<SessionExportPlan> {
        super::session_records::validate_query(&request.query)?;
        let _guard = self.capture_lock.lock().map_err(|_| poison())?;
        self.flush_pending_integrity_for(Some(&request.query.session_id))?;
        let plan = self
            .repository
            .prepare_session_export(request, self.clock.now_ms())?;
        self.export_sessions
            .lock()
            .map_err(|_| poison())?
            .insert(plan.export_id.clone(), plan.session_id.clone());
        Ok(plan)
    }
    pub fn session_export_page(
        &self,
        id: String,
        cursor: Option<String>,
        size: u32,
    ) -> AppResult<SessionRecordPage> {
        let _guard = self.capture_lock.lock().map_err(|_| poison())?;
        let session = self
            .export_sessions
            .lock()
            .map_err(|_| poison())?
            .get(&id)
            .cloned();
        if let Some(session) = session {
            self.flush_pending_integrity_for(Some(&session))?;
        }
        self.repository
            .session_export_page(id, cursor, size, self.clock.now_ms())
    }
    pub fn cancel_session_export(&self, id: String) -> AppResult<()> {
        self.repository.close_session_export(id.clone())?;
        self.export_sessions
            .lock()
            .map_err(|_| poison())?
            .remove(&id);
        Ok(())
    }
    pub fn finish_session_export(&self, id: String) -> AppResult<()> {
        self.repository.close_session_export(id.clone())?;
        self.export_sessions
            .lock()
            .map_err(|_| poison())?
            .remove(&id);
        Ok(())
    }
    pub async fn dispatch(
        self: &Arc<Self>,
        envelope: CommandEnvelope,
    ) -> AppResult<CommandReceipt> {
        let operation_id = envelope.operation_id.clone();
        let command = envelope.command.clone();
        if let Command::UnbindWindow(request) = &command {
            let app = self.clone();
            let request = request.clone();
            let receipt = tokio::task::spawn_blocking(move || {
                // associate() takes this same guard. Keep the readonly
                // preflight, binding effect and durable outcome indivisible
                // with respect to a later A -> B association.
                let _capture = app.capture_lock.lock().map_err(|_| poison())?;
                let mut preflight = || app.repository.preflight_window_unbind(envelope.clone());
                let mut commit = || {
                    app.repository
                        .commit_window_unbind(envelope.clone(), app.clock.now_ms())
                };
                app.hook.unlink_window(request, &mut preflight, &mut commit)
            })
            .await
            .map_err(|_| poison())??;
            self.publish()?;
            return Ok(receipt);
        }
        if matches!(command, Command::Disconnect) {
            let app = self.clone();
            let preflight = envelope.clone();
            let replay =
                tokio::task::spawn_blocking(move || app.repository.preflight_disconnect(preflight))
                    .await
                    .map_err(|_| poison())??;
            if let Some(receipt) = replay {
                self.publish()?;
                return Ok(receipt);
            }
            self.stop_capture_pipeline().await?;
        }
        let repo = self.repository.clone();
        let clock = self.clock.clone();
        let capture_boundary = matches!(
            command,
            Command::SelectSource { .. } | Command::Disconnect | Command::SetCapturePaused { .. }
        );
        let paused = if let Command::SetCapturePaused { paused } = &command {
            Some(*paused)
        } else {
            None
        };
        let app = self.clone();
        let receipt = tokio::task::spawn_blocking(move || {
            let _capture = if capture_boundary {
                Some(app.capture_lock.lock().map_err(|_| poison())?)
            } else {
                None
            };
            let receipt = super::AppService::new(repo, clock).dispatch(envelope)?;
            if let Some(paused) = paused.filter(|_| !receipt.replayed)
                && let Some(session) = app.repository.snapshot()?.session
            {
                app.controls
                    .lock()
                    .map_err(|_| poison())?
                    .entry(session.session_id)
                    .or_insert((false, true))
                    .0 = paused;
            }
            Ok::<_, AppError>(receipt)
        })
        .await
        .map_err(|_| poison())??;
        if receipt.replayed {
            if let Command::RevokeCredential { credential_id, .. } = &command {
                self.clean_revoked_credential(credential_id.clone())?;
            }
            if matches!(
                command,
                Command::SetConnectionSuspended { .. } | Command::SetPurposeRuntimeEnabled { .. }
            ) {
                self.abort_invalidated()?;
            }
            self.publish()?;
            return Ok(receipt);
        }
        match command {
            Command::RevokeCredential { credential_id, .. } => {
                self.clean_revoked_credential(credential_id)?
            }
            Command::ManualInput { .. } => {
                if let Some(id) = receipt.result_id.clone() {
                    let app = self.clone();
                    tokio::spawn(async move {
                        app.process_unit(id, 0, operation_id, true).await;
                    });
                }
            }
            Command::CancelTask { task_id } => {
                let mut tasks = self.tasks.lock().map_err(|_| poison())?;
                let mut active_attempts = self.active_attempts.lock().map_err(|_| poison())?;
                let mut partials = self.partials.lock().map_err(|_| poison())?;
                if let Some(handle) = tasks.remove(&task_id) {
                    handle.abort();
                }
                active_attempts.remove(&task_id);
                partials.remove(&task_id);
            }
            Command::Disconnect => {}
            Command::ApplyConfiguration { .. }
            | Command::SetConnectionSuspended { .. }
            | Command::SetPurposeRuntimeEnabled { .. } => self.abort_invalidated()?,
            // Applied under capture_lock in the writer command boundary above.
            Command::SetCapturePaused { .. } => {}
            Command::SetTranslationEnabled { enabled } => {
                let _gate = self.translation_gate.lock().map_err(|_| poison())?;
                if let Some(session) = self.repository.snapshot()?.session {
                    self.controls
                        .lock()
                        .map_err(|_| poison())?
                        .entry(session.session_id)
                        .or_insert((false, true))
                        .1 = enabled;
                }
                if !enabled {
                    for task in self.repository.snapshot()?.tasks.into_iter().filter(|t| {
                        t.purpose == "translation"
                            && matches!(t.status.as_str(), "queued" | "running")
                    }) {
                        let _ = self.repository.dispatch(
                            CommandEnvelope {
                                operation_id: self.clock.new_id("translation-off"),
                                expected_revision: None,
                                command: Command::CancelTask {
                                    task_id: task.task_id,
                                },
                            },
                            self.clock.now_ms(),
                        );
                    }
                    self.abort_invalidated()?;
                }
            }
            Command::UnbindWindow(_) => unreachable!("handled inside the binding transaction"),
            _ => {}
        }
        self.publish()?;
        Ok(receipt)
    }
    pub(super) fn abort_invalidated(&self) -> AppResult<()> {
        // Freeze the cleanup candidates before reading authority. A task may be
        // prepared and registered while this synchronous repository read is in
        // progress; absence from that older snapshot cannot invalidate it.
        let candidates: Vec<_> = self
            .tasks
            .lock()
            .map_err(|_| poison())?
            .keys()
            .cloned()
            .collect();
        let snapshot = self.repository.snapshot()?;
        let active: Vec<_> = snapshot
            .tasks
            .iter()
            .filter(|t| matches!(t.status.as_str(), "queued" | "running" | "started"))
            .map(|t| t.task_id.clone())
            .collect();
        // Registration, removal and partial cleanup share this registry boundary.
        // Never hold a registry lock across a repository call or an await.
        let mut tasks = self.tasks.lock().map_err(|_| poison())?;
        tasks.retain(|id, handle| {
            if !candidates.contains(id) || active.contains(id) {
                true
            } else {
                handle.abort();
                false
            }
        });
        self.active_attempts
            .lock()
            .map_err(|_| poison())?
            .retain(|id, _| tasks.contains_key(id));
        self.partials
            .lock()
            .map_err(|_| poison())?
            .retain(|id, _| tasks.contains_key(id));
        Ok(())
    }
    fn lexical_state(
        &self,
        unit: &ReadingUnit,
        state: LexicalPresentationState,
        error_code: Option<String>,
    ) -> AppResult<()> {
        let mut states = self.lexical_states.lock().map_err(|_| poison())?;
        let key = (
            unit.unit_id.clone(),
            unit.text_revision,
            unit.source_hash.clone(),
        );
        if !states.contains_key(&key) && states.len() >= 256 {
            states.pop_first();
        }
        states.insert(
            key,
            LexicalState {
                unit_id: unit.unit_id.clone(),
                text_revision: unit.text_revision,
                source_hash: unit.source_hash.clone(),
                state,
                error_code,
            },
        );
        drop(states);
        self.publish()
    }
    pub async fn retry_lexical(
        self: &Arc<Self>,
        unit_id: String,
        text_revision: u32,
    ) -> AppResult<()> {
        crate::domain::require_id(&unit_id)?;
        let unit = self.repository.unit(unit_id, text_revision)?;
        if unit.source_language != "ja" {
            return Err(AppError::new(
                "language_not_supported",
                "language",
                "此原文无需日语分词",
            ));
        }
        self.clone().analyze_lexical(unit).await
    }
    async fn analyze_lexical(self: Arc<Self>, unit: ReadingUnit) -> AppResult<()> {
        let key = (
            unit.unit_id.clone(),
            unit.text_revision,
            unit.source_hash.clone(),
        );
        {
            let mut states = self.lexical_states.lock().map_err(|_| poison())?;
            if states
                .get(&key)
                .is_some_and(|s| s.state == LexicalPresentationState::Pending)
            {
                return Ok(());
            }
            if !states.contains_key(&key) && states.len() >= 256 {
                states.pop_first();
            }
            states.insert(
                key,
                LexicalState {
                    unit_id: unit.unit_id.clone(),
                    text_revision: unit.text_revision,
                    source_hash: unit.source_hash.clone(),
                    state: LexicalPresentationState::Pending,
                    error_code: None,
                },
            );
        }
        self.publish()?;
        let Some(info) = self
            .language
            .resources()
            .into_iter()
            .find(|i| i.kind == "language" && i.available)
        else {
            self.lexical_state(
                &unit,
                LexicalPresentationState::ResourceUnavailable,
                Some("language_resource_unavailable".into()),
            )?;
            return Ok(());
        };
        let tokens = match self.language.analyze(&unit.source_text).await {
            Ok(tokens) => tokens,
            Err(error) => {
                self.lexical_state(
                    &unit,
                    LexicalPresentationState::WorkerFailed,
                    Some(error.code.clone()),
                )?;
                self.diagnostic(error.into());
                return Ok(());
            }
        };
        let reading_missing = tokens.iter().any(|t| {
            t.reading.is_empty()
                && t.surface
                    .chars()
                    .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        });
        let input = TokenSnapshotInput {
            analysis_revision: self.language.analysis_revision().into(),
            unit_id: unit.unit_id.clone(),
            text_revision: unit.text_revision,
            tokens,
            resource: crate::domain::capture::ResourceSnapshotInput {
                snapshot_id: format!("language-{}", info.manifest_sha256),
                kind: "language".into(),
                version: info.version,
                manifest_sha256: info.manifest_sha256,
                contract_version: "1".into(),
                metadata_json: "{}".into(),
            },
        };
        match self
            .repository
            .set_tokens_for_retry(input, self.clock.now_ms())
        {
            Ok(()) => self.lexical_state(
                &unit,
                if reading_missing {
                    LexicalPresentationState::ReadingUnavailable
                } else {
                    LexicalPresentationState::Ready
                },
                if reading_missing {
                    Some("reading_unavailable".into())
                } else {
                    None
                },
            )?,
            Err(error) => {
                let state = if [
                    "invalid_token_alignment",
                    "invalid_utf8_span",
                    "invalid_span",
                ]
                .contains(&error.code.as_str())
                {
                    LexicalPresentationState::AlignmentInvalid
                } else {
                    LexicalPresentationState::WorkerFailed
                };
                self.lexical_state(&unit, state, Some(error.code.clone()))?;
                self.diagnostic(error.into());
            }
        }
        Ok(())
    }
    async fn process_unit(
        self: Arc<Self>,
        unit_id: String,
        text_revision: u32,
        operation_id: String,
        schedule_translation: bool,
    ) {
        let unit = match self.repository.unit(unit_id.clone(), text_revision) {
            Ok(u) => u,
            Err(e) => {
                self.diagnostic(e.into());
                return;
            }
        };
        // Source is already committed and published; offline lexical work cannot hold back source display.
        let _ = self.publish();
        if unit.source_language == "ja" && unit.tokens.is_empty() {
            let app = self.clone();
            let lexical_unit = unit.clone();
            tokio::spawn(async move {
                let _ = app.analyze_lexical(lexical_unit).await;
            });
        }
        let Ok(_gate) = self.translation_gate.lock() else {
            return;
        };
        if !schedule_translation || self.closing.load(Ordering::Acquire) {
            return;
        }
        let automatic = unit
            .source_summary
            .as_ref()
            .is_some_and(|s| s.source_kind == HistorySourceKind::Hook);
        let _capture = if automatic {
            match self.capture_lock.lock() {
                Ok(guard) => Some(guard),
                Err(_) => return,
            }
        } else {
            None
        };
        if automatic
            && self
                .natural_disconnects
                .lock()
                .map(|closed| closed.contains(&unit.session_id))
                .unwrap_or(true)
        {
            return;
        }
        if self
            .controls
            .lock()
            .ok()
            .and_then(|controls| controls.get(&unit.session_id).copied())
            .is_some_and(|(paused, enabled)| !enabled || (automatic && paused))
        {
            return;
        }
        let prepared = if automatic {
            self.repository.prepare_work_capture_tasks(
                crate::domain::capture::CaptureVersion {
                    unit_id: unit.unit_id,
                    text_revision: unit.text_revision,
                },
                operation_id,
                self.clock.now_ms(),
            )
        } else {
            self.repository.prepare_work_tasks_for_operation(
                unit.unit_id,
                unit.text_revision,
                "translation".into(),
                operation_id,
                self.clock.now_ms(),
            )
        };
        match prepared {
            Ok(tasks) => {
                for task in tasks {
                    self.spawn_task(task);
                }
            }
            Err(error) => {
                if ![
                    "purpose_disabled",
                    "purpose_suspended",
                    "connection_suspended",
                    "not_configured",
                ]
                .contains(&error.code.as_str())
                {
                    self.diagnostic(error.into());
                }
            }
        }
        let _ = self.publish();
    }
    fn spawn_task(self: &Arc<Self>, task: WorkDispatch) {
        let id = task.task_id().to_owned();
        let attempt_id = task.attempt_id().to_owned();
        let limit = task.concurrency();
        let queue = task.queue_capacity();
        if !(1..=8).contains(&limit) || !(1..=128).contains(&queue) {
            let _ = self
                .repository
                .fail_task(id, "invalid_schedule".into(), self.clock.now_ms());
            self.diagnostic(
                AppError::new("invalid_schedule", "scheduling", "并发或待执行队列上限无效").into(),
            );
            let _ = self.publish();
            return;
        }
        // Queue policy belongs to the purpose, independently of HTTP/offline.
        let key = format!("{}:{limit}:{queue}", task.purpose());
        let scheduler = match self.schedulers.lock() {
            Ok(mut map) => map
                .entry(key)
                .or_insert_with(|| {
                    Arc::new(PurposeScheduler {
                        running: Arc::new(Semaphore::new(limit)),
                        pending: Arc::new(Semaphore::new(queue)),
                    })
                })
                .clone(),
            Err(_) => return,
        };
        let queued = match scheduler.pending.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                let _ =
                    self.repository
                        .fail_task(id, "purpose_queue_full".into(), self.clock.now_ms());
                self.diagnostic(
                    AppError::new(
                        "purpose_queue_full",
                        "scheduling",
                        "此用途的待执行队列已满，可稍后重试",
                    )
                    .into(),
                );
                return;
            }
        };
        let (start_tx, start_rx) = tokio::sync::oneshot::channel::<()>();
        let app = self.clone();
        let handle = tokio::spawn(async move {
            if start_rx.await.is_err() {
                return;
            }
            let _queued = queued;
            let permit = scheduler.running.clone().acquire_owned().await;
            if permit.is_err() {
                return;
            }
            let result = app.run_task(task.clone()).await;
            if let Err(error) = result {
                let _ = app.repository.fail_task(
                    task.task_id().to_owned(),
                    error.code.clone(),
                    app.clock.now_ms(),
                );
                app.diagnostic(error.into());
            }
            // All callback/lifecycle nesting follows tasks -> attempts -> partials.
            if let Ok(mut tasks) = app.tasks.lock()
                && let Ok(mut active_attempts) = app.active_attempts.lock()
                    && let Ok(mut partials) = app.partials.lock() {
                        tasks.remove(task.task_id());
                        active_attempts.remove(task.task_id());
                        partials.remove(task.task_id());
                    }
            let _ = app.publish();
        });
        if let Ok(mut tasks) = self.tasks.lock()
            && let Ok(mut active_attempts) = self.active_attempts.lock() {
                active_attempts.insert(id.clone(), attempt_id);
                tasks.insert(id, handle.abort_handle());
            }
        let _ = start_tx.send(());
    }
    async fn run_task(self: &Arc<Self>, mut task: WorkDispatch) -> AppResult<()> {
        if task.purpose() == "translation"
            && self
                .controls
                .lock()
                .map_err(|_| poison())?
                .get(&task.unit().session_id)
                .is_some_and(|(_, enabled)| !*enabled)
        {
            return Err(AppError::new(
                "translation_disabled",
                "scheduling",
                "本会话自动翻译已关闭",
            ));
        }
        self.repository
            .validate_task_authority(task.task_id().to_owned())?;
        if let Some(cached) = task.cached_result().cloned() {
            if let WorkDispatch::Service(service) = &task {
                tokio::time::timeout(
                    Duration::from_millis(u64::from(service.plan.policy.execution.timeout_ms)),
                    self.gateway.validate_cached_service(service),
                ).await.map_err(|_| AppError::new("timeout", "scheduling", "缓存复用授权核验已超时"))??;
            }
            self.repository.complete_task(
                TaskCompletion {
                    task_id: task.task_id().to_owned(),
                    attempt_id: task.attempt_id().to_owned(),
                    text: cached.text,
                    output_mode: cached.output_mode,
                    structured: cached.structured,
                    target_language: cached.target_language,
                    provider_label: cached.provider_label,
                },
                self.clock.now_ms(),
            )?;
            return Ok(());
        }
        let legacy_plan = match &task {
            WorkDispatch::Legacy(legacy) => {
                Some(request_plan::compose(legacy, &legacy.frozen_history)?)
            }
            WorkDispatch::Service(_) => None,
        };
        self.repository
            .validate_task_authority(task.task_id().to_owned())?;
        let max_retries = task.retries();
        if max_retries > 3 {
            return Err(AppError::new(
                "invalid_schedule",
                "scheduling",
                "自动重试次数必须为0至3，默认关闭",
            ));
        }
        let mut retried = 0u64;
        let raw = loop {
            // Every wire attempt gets a distinct callback identity. Keeping the
            // old callback alive cannot authorize it for a later retry.
            let weak: Weak<Self> = Arc::downgrade(self);
            let task_id = task.task_id().to_owned();
            let attempt_id = task.attempt_id().to_owned();
            let delta: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |text| {
                if let Some(app) = weak.upgrade() {
                    {
                        let Ok(tasks) = app.tasks.lock() else {
                            return;
                        };
                        let Ok(active_attempts) = app.active_attempts.lock() else {
                            return;
                        };
                        if !tasks.contains_key(&task_id)
                            || active_attempts.get(&task_id) != Some(&attempt_id)
                        {
                            return;
                        }
                        let Ok(mut partials) = app.partials.lock() else {
                            return;
                        };
                        partials.insert(
                            task_id.clone(),
                            AttemptPartial {
                                attempt_id: attempt_id.clone(),
                                text,
                            },
                        );
                    }
                    let _ = app.publish();
                }
            });
            let response = match &task {
                WorkDispatch::Legacy(legacy) => {
                    self.gateway
                        .execute(
                            legacy,
                            legacy_plan.as_ref().expect("legacy plan composed"),
                            delta,
                        )
                        .await
                }
                WorkDispatch::Service(service) => {
                    self.gateway.execute_service(service, delta).await
                }
            };
            match response {
                Ok(text) => break text,
                Err(error)
                    if retried < max_retries
                        && [
                            "timeout",
                            "connection_failed",
                            "network_error",
                            "service_unavailable",
                        ]
                        .contains(&error.code.as_str()) =>
                {
                    let next_attempt = self.repository.retry_attempt(
                        task.task_id().to_owned(),
                        task.attempt_id().to_owned(),
                        error.code,
                        self.clock.now_ms(),
                    )?;
                    task.set_attempt_id(next_attempt);
                    {
                        let tasks = self.tasks.lock().map_err(|_| poison())?;
                        let mut active_attempts =
                            self.active_attempts.lock().map_err(|_| poison())?;
                        let mut partials = self.partials.lock().map_err(|_| poison())?;
                        if tasks.contains_key(task.task_id()) {
                            active_attempts
                                .insert(task.task_id().to_owned(), task.attempt_id().to_owned());
                            partials.remove(task.task_id());
                        }
                    }
                    let _ = self.publish();
                    tokio::time::sleep(Duration::from_millis(500u64 << retried)).await;
                    self.repository
                        .validate_task_authority(task.task_id().to_owned())?;
                    retried += 1;
                }
                Err(error) => return Err(error),
            }
        };
        let (text, structured, mode) = if task.purpose() == "translation" {
            (raw, serde_json::json!({}), "text".into())
        } else {
            let (text, refs, mode) =
                request_plan::validate_study_text(&raw, &task.unit().source_text);
            (text, serde_json::json!({"references":refs}), mode)
        };
        let (target_language, provider_label) = match (&task, legacy_plan) {
            (WorkDispatch::Legacy(legacy), Some(plan)) => {
                (plan.target_language, legacy.connection.display_name.clone())
            }
            (WorkDispatch::Service(service), _) => match &service.plan.target {
                crate::domain::service_execution::ServiceExecutionTarget::Http { plan } => (
                    plan.request.target_language.clone(),
                    plan.connection.display_name.clone(),
                ),
                crate::domain::service_execution::ServiceExecutionTarget::Offline {
                    config,
                    payload,
                } => (payload.target_language.clone(), config.display_name.clone()),
            },
            _ => unreachable!("legacy plan composed before execution"),
        };
        self.repository.complete_task(
            TaskCompletion {
                task_id: task.task_id().to_owned(),
                attempt_id: task.attempt_id().to_owned(),
                text,
                output_mode: mode,
                structured,
                target_language,
                provider_label,
            },
            self.clock.now_ms(),
        )?;
        Ok(())
    }
    pub fn freeze_learning(
        &self,
        unit_id: String,
        text_revision: u32,
        token_index: Option<u32>,
    ) -> AppResult<LearningObject> {
        let unit = self.repository.unit(unit_id, text_revision)?;
        if token_index.is_some_and(|i| i as usize >= unit.tokens.len()) {
            return Err(AppError::new(
                "stale_token",
                "study",
                "词语位置已失效，请重新选择",
            ));
        }
        let object = LearningObject { unit, token_index };
        *self.learning.lock().map_err(|_| poison())? = Some(object.clone());
        self.publish()?;
        Ok(object)
    }
    pub fn study(
        self: &Arc<Self>,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        token_index: Option<u32>,
    ) -> AppResult<TaskView> {
        if !matches!(purpose.as_str(), "grammar" | "context_word") {
            return Err(AppError::new(
                "invalid_purpose",
                "study",
                "请选择文法或语境查词用途",
            ));
        }
        let tasks = self.repository.prepare_work_study_tasks(
            unit_id,
            text_revision,
            purpose,
            token_index,
            self.clock.new_id("study-operation"),
            self.clock.now_ms(),
        )?;
        let first = tasks
            .first()
            .ok_or_else(|| AppError::new("not_configured", "study", "学习用途尚未配置或未启用"))?
            .task_id()
            .to_owned();
        for task in tasks {
            self.spawn_task(task);
        }
        self.publish()?;
        self.repository
            .snapshot()?
            .tasks
            .into_iter()
            .find(|t| t.task_id == first)
            .ok_or_else(|| AppError::new("task_missing", "study", "任务状态不存在"))
    }
    pub fn lookup(
        &self,
        unit_id: String,
        text_revision: u32,
        token_index: u32,
    ) -> AppResult<WordLookup> {
        let unit = self.repository.unit(unit_id.clone(), text_revision)?;
        let token = unit
            .tokens
            .get(token_index as usize)
            .cloned()
            .ok_or_else(|| AppError::new("stale_token", "dictionary", "词语位置已失效"))?;
        let entries = self.language.lookup(&token)?;
        let version = self
            .language
            .resources()
            .into_iter()
            .find(|r| r.kind == "dictionary")
            .map(|r| r.version)
            .unwrap_or_default();
        Ok(WordLookup {
            unit_id,
            text_revision,
            token_index,
            token,
            entries,
            resource_version: version,
        })
    }
    pub fn resources(&self) -> Vec<ResourceInfo> {
        let mut resources = self.language.resources();
        resources.push(self.hook.resource_info());
        resources
    }
    pub fn unit(&self, id: String, revision: u32) -> AppResult<ReadingUnit> {
        self.repository.unit(id, revision)
    }
    pub fn reading_unit_projection(
        &self,
        id: String,
        revision: u32,
    ) -> AppResult<ReadingUnitProjection> {
        self.repository.reading_unit_projection(id, revision)
    }
    pub fn history(
        &self,
        game_id: Option<String>,
        cursor: Option<String>,
        limit: u32,
    ) -> AppResult<HistoryPage> {
        self.repository
            .history(game_id, cursor, limit.clamp(1, 100))
    }
    pub fn history_page(&self, query: HistoryQuery) -> AppResult<HistoryPage> {
        super::AppService::new(self.repository.clone(), self.clock.clone()).history_page(query)
    }
    pub fn history_sessions(&self, query: HistorySessionsQuery) -> AppResult<HistorySessionsPage> {
        super::AppService::new(self.repository.clone(), self.clock.clone()).history_sessions(query)
    }
    pub fn game_environment(&self, game_id: &str) -> AppResult<String> {
        self.repository
            .snapshot()?
            .games
            .into_iter()
            .find(|g| g.game_id == game_id)
            .map(|g| g.target.environment_key)
            .ok_or_else(|| AppError::new("game_missing", "library", "游戏记录不存在"))
    }
    pub fn retranslate(
        self: &Arc<Self>,
        unit_id: String,
        text_revision: u32,
        operation_id: String,
    ) -> AppResult<Vec<TaskView>> {
        let prepared = self
            .repository
            .prepare_work_uncached_translation_for_operation(
                unit_id,
                text_revision,
                operation_id,
                self.clock.now_ms(),
            )?;
        let task_views = prepared.task_views;
        for task in prepared.dispatches {
            self.spawn_task(task);
        }
        self.publish()?;
        Ok(task_views)
    }
    pub async fn test_configuration(
        &self,
        request: ConfigurationTestRequest,
    ) -> AppResult<ConfigurationTestResult> {
        let config = self.repository.snapshot()?.configuration;
        let configuration = if request.revision_id == config.draft.revision_id() {
            config.draft
        } else {
            config
                .applied
                .filter(|c| c.revision_id() == request.revision_id)
                .ok_or_else(|| {
                    AppError::new(
                        "stale_configuration",
                        "configuration",
                        "测试配置版本已改变，请重新保存草稿",
                    )
                })?
        };
        let configuration = configuration.as_legacy().cloned().ok_or_else(|| {
            AppError::new(
                "versioned_service_test_required",
                "models",
                "此服务需使用对应版本的服务测试入口",
            )
        })?;
        let result = super::model_tests::run(
            self.repository.as_ref(),
            self.clock.as_ref(),
            self.gateway.as_ref(),
            configuration,
            request,
        )
        .await;
        // Failed tests also commit a record and advance the repository revision.
        // Publish that authority to every window without replacing the original
        // test error if publication itself fails.
        let publication = self.publish();
        if let Err(error) = &publication {
            self.diagnostic(error.clone().into());
        }
        match result {
            Ok(result) => publication.map(|()| result),
            Err(error) => Err(error),
        }
    }
    pub fn store_credential(&self, input: CredentialInput) -> AppResult<CredentialReference> {
        use crate::domain::model_config::credentials::*;
        let secret = SecretText::from_bytes(input.secret.into_bytes())?;
        let result = super::credential_storage::store(
            self.repository.as_ref(),
            self.vault.as_ref(),
            self.clock.as_ref(),
            input.label,
            CredentialStorageMetadata::LegacyRawApiKey,
            secret.expose().as_bytes(),
        )?;
        self.publish()?;
        Ok(result)
    }
    pub fn store_credential_material(
        &self,
        label: String,
        material: crate::domain::model_config::credentials::CredentialMaterial,
    ) -> AppResult<CredentialReference> {
        use crate::domain::model_config::credentials::*;
        if material.kind() == CredentialMaterialKind::GoogleOauthRefresh {
            return Err(AppError::new(
                "oauth_grant_required",
                "credentials",
                "OAuth凭据必须通过正式授权生命周期建立",
            ));
        }
        let bytes = encode_vault_credential(&material)?;
        let result = super::credential_storage::store(
            self.repository.as_ref(),
            self.vault.as_ref(),
            self.clock.as_ref(),
            label,
            CredentialStorageMetadata::EnvelopeV1 {
                kind: material.kind(),
            },
            bytes.expose(),
        )?;
        self.publish()?;
        Ok(result)
    }
    pub fn credential_revocation_impact(
        &self,
        credential_id: String,
    ) -> AppResult<CredentialRevocationImpact> {
        let snapshot = self.repository.snapshot()?;
        super::configuration_controls::credential_impact(
            &snapshot.configuration,
            &snapshot.credentials,
            &snapshot.credential_pools,
            &credential_id,
        )
    }
    pub async fn revoke_credential(
        self: &Arc<Self>,
        request: CredentialRevocationRequest,
    ) -> AppResult<CommandReceipt> {
        self.dispatch(CommandEnvelope {
            operation_id: request.operation_id,
            expected_revision: None,
            command: Command::RevokeCredential {
                credential_id: request.credential_id,
                expected_lifecycle_revision: request.expected_lifecycle_revision,
                expected_impact_hash: request.expected_impact_hash,
            },
        })
        .await
    }
    /// Authority is already revoked before any secret deletion, including cleanup failures.
    fn clean_revoked_credential(&self, credential_id: String) -> AppResult<()> {
        self.abort_invalidated()?;
        self.publish()?;
        let pending = self
            .repository
            .pending_credentials()?
            .into_iter()
            .find(|r| r.credential_id == credential_id);
        let Some(reference) = pending else {
            return Ok(());
        };
        if self.vault.delete(&reference.credential_version_id).is_err() {
            return Err(AppError::new(
                "credential_cleanup_pending",
                "credentials",
                "凭据已撤销并停止依赖请求；系统秘密清理未完成，将在下次启动时重试",
            ));
        }
        self.repository
            .mark_credential_cleaned(credential_id, self.clock.now_ms())?;
        self.publish()
    }
    pub fn recover_credentials(&self) -> AppResult<()> {
        for reference in self.repository.pending_credentials()? {
            self.repository
                .revoke_credential(reference.credential_id.clone(), self.clock.now_ms())?;
            if self.vault.delete(&reference.credential_version_id).is_ok() {
                self.repository
                    .mark_credential_cleaned(reference.credential_id, self.clock.now_ms())?;
            }
        }
        Ok(())
    }
    pub fn report_native_effect_failure(&self, error: AppError) {
        self.diagnostic(error.into());
    }
    fn schedule_natural_disconnect(
        self: &Arc<Self>,
        session: String,
        handle: &tokio::runtime::Handle,
    ) {
        // Serialize eligibility revocation with automatic task preparation.
        // Never hold capture_lock while acquiring translation_gate.
        let Ok(gate) = self.translation_gate.lock() else {
            return;
        };
        let Ok(mut pending) = self.natural_disconnects.lock() else {
            return;
        };
        if !pending.insert(session.clone()) {
            return;
        }
        drop(pending);
        drop(gate);
        let app = self.clone();
        handle.spawn(async move {
            let stopping = app.clone();
            let expected = session.clone();
            let result = tokio::task::spawn_blocking(move || {
                stopping.hook.finish_disconnected_session(&expected)
            })
            .await
            .map_err(|_| poison())
            .and_then(|result| result);
            match result {
                Ok(true) => {}  // Terminal persistence was part of the consumer ACK.
                Ok(false) => {} // An older target may never stop or release a new one.
                Err(error) => {
                    app.hook_diagnostic(
                        Some(session.clone()),
                        Diagnostic {
                            code: error.code,
                            stage: error.stage,
                            message: error.message,
                            recoverable: true,
                        },
                    );
                }
            }
            let _ = app.publish();
        });
    }
    fn hook_diagnostic(&self, session: Option<String>, diagnostic: Diagnostic) {
        if matches!(
            diagnostic.code.as_str(),
            "capture_connected" | "game_disconnected" | "capture_failed" | "integrity_lost"
        ) && self
            .flush_pending_integrity_for(session.as_deref())
            .is_err()
        {
            let _ = self.repository.record_hook_diagnostic(
                session,
                Diagnostic {
                    code: "capture_integrity_pending".into(),
                    stage: "storage".into(),
                    message: "接收事实尚未确认持久，保留原会话恢复标记".into(),
                    recoverable: true,
                },
                self.clock.now_ms(),
            );
        } else if let Err(error) =
            self.repository
                .record_hook_diagnostic(session, diagnostic, self.clock.now_ms())
        {
            self.diagnostic(error.into());
        }
        let _ = self.publish();
    }

    fn diagnostic(&self, diagnostic: Diagnostic) {
        if matches!(
            diagnostic.code.as_str(),
            "capture_connected" | "game_disconnected" | "capture_failed" | "integrity_lost"
        ) && self.flush_pending_integrity().is_err()
        {
            let _ = self.repository.record_diagnostic(
                Diagnostic {
                    code: "capture_integrity_pending".into(),
                    stage: "storage".into(),
                    message: "接收事实尚未确认持久，保留原会话恢复标记".into(),
                    recoverable: true,
                },
                self.clock.now_ms(),
            );
            let _ = self.publish();
            return;
        }
        let _ = self
            .repository
            .record_diagnostic(diagnostic, self.clock.now_ms());
        let _ = self.publish();
    }
}
impl From<AppError> for Diagnostic {
    fn from(value: AppError) -> Self {
        Self {
            code: value.code,
            stage: value.stage,
            message: value.message,
            recoverable: value.recoverable,
        }
    }
}
