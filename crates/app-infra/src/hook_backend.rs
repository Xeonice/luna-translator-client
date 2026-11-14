//! CrossOver and broker I/O adapter. User confirmations and reading rules remain in app-core.
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::runtime::*,
};
use hook_client::{Client, SubscriptionError};
#[cfg(test)]
use hook_client::{LaunchSettings, Target, WindowTarget};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

mod journal;
mod preview_identity;
mod window_association;

#[cfg(test)]
use journal::diagnostic_text;
use journal::{project_event, queue_loss, stage};
use preview_identity::preview_resource_snapshot;
pub use preview_identity::{
    FrozenPreviewTarget, PreviewCollectorVerifier, PreviewPairingContext, PreviewProofProvider,
};
#[cfg(test)]
use preview_identity::{
    collector_environment_matches, preview_file_identity, preview_file_stamp,
    preview_verification_error, preview_verification_status, single_preview_instance,
    window_title_proof_error,
};
use window_association::{Candidate, window_observed_ms};

pub struct CoreHookBackend {
    client: Arc<Client>,
    resources: PathBuf,
    target_locks: PathBuf,
    candidates: Mutex<BTreeMap<String, Candidate>>,
    active: Arc<Mutex<Option<HookSessionInput>>>,
    preview_binding: Mutex<Option<(String, Candidate)>>,
    preview_bound_at: Mutex<Option<(String, String, u64)>>,
    association_has_window: Mutex<bool>,
    failed: Arc<Mutex<BTreeSet<String>>>,
    lease: Mutex<Option<(String, File)>>,
    launches: Mutex<BTreeMap<String, std::time::Instant>>,
    relay_control: Mutex<Option<mpsc::SyncSender<RelayDrain>>>,
    preview_proof: Mutex<Option<std::sync::Weak<dyn PreviewProofProvider>>>,
}
struct RelayDrain {
    session_id: Option<String>,
    application_ack: Option<mpsc::SyncSender<AppResult<()>>>,
    relay_ack: mpsc::SyncSender<AppResult<()>>,
}
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "capture", message)
}
fn check_target_switch(
    held: Option<&str>,
    target: &str,
    host_status: &str,
    confirmed: bool,
) -> AppResult<()> {
    if held.is_some_and(|held| held != target) {
        if matches!(
            host_status,
            "disconnected" | "disconnecting" | "exited" | "stopped" | "error"
        ) {
            return Err(error(
                "capture_cleanup_pending",
                "上一会话尚未完成退出收尾，请先重试断开",
            ));
        }
        if !confirmed {
            return Err(error("replace_required", "另一个游戏正在取文，请确认切换"));
        }
    }
    Ok(())
}

impl CoreHookBackend {
    pub fn new(resources: PathBuf, logs: PathBuf, target_locks: PathBuf) -> Self {
        Self {
            client: Arc::new(Client::new(resources.clone(), logs)),
            resources,
            target_locks,
            candidates: Mutex::new(BTreeMap::new()),
            active: Arc::new(Mutex::new(None)),
            preview_binding: Mutex::new(None),
            preview_bound_at: Mutex::new(None),
            preview_proof: Mutex::new(None),
            association_has_window: Mutex::new(false),
            failed: Arc::new(Mutex::new(BTreeSet::new())),
            lease: Mutex::new(None),
            launches: Mutex::new(BTreeMap::new()),
            relay_control: Mutex::new(None),
        }
    }
    fn stop_owned_session(&self) -> AppResult<()> {
        self.stop_owned_producers()?;
        self.drain_relay(None)
    }
    fn stop_owned_producers(&self) -> AppResult<()> {
        *self
            .preview_binding
            .lock()
            .map_err(|_| error("capture_state", "窗口关联状态不可用"))? = None;
        self.client
            .disconnect()
            .map_err(|_| error("disconnect_failed", "宿主管理线程未能接受退出请求"))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(13);
        while self.client.has_active_session() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        if self.client.has_active_session() {
            return Err(error(
                "disconnect_pending",
                "宿主尚未完全退出，已保留目标租约，请稍后重试",
            ));
        }
        self.client
            .wait_event_producers_stopped(Duration::from_secs(3))
            .map_err(|_| {
                error(
                    "capture_producers_pending",
                    "宿主发布线程尚未全部结束，不能确认完整接收范围",
                )
            })?;
        Ok(())
    }
    fn drain_relay(
        &self,
        application_ack: Option<mpsc::SyncSender<AppResult<()>>>,
    ) -> AppResult<()> {
        self.drain_session_relay(None, application_ack)
    }
    fn drain_session_relay(
        &self,
        session_id: Option<String>,
        application_ack: Option<mpsc::SyncSender<AppResult<()>>>,
    ) -> AppResult<()> {
        let sender = self
            .relay_control
            .lock()
            .map_err(|_| error("capture_state", "事件适配器控制状态不可用"))?
            .clone();
        if let Some(sender) = sender {
            let (tx, rx) = mpsc::sync_channel(1);
            sender
                .try_send(RelayDrain {
                    session_id,
                    application_ack,
                    relay_ack: tx,
                })
                .map_err(|_| error("capture_drain_busy", "已有接收收尾正在执行"))?;
            rx.recv_timeout(Duration::from_secs(15))
                .map_err(|_| error("capture_drain_pending", "事件适配器未完成排空确认"))??;
        } else if application_ack.is_some() {
            return Err(error("capture_drain_unavailable", "事件适配器尚未启动"));
        }
        Ok(())
    }
    fn native_resource(&self) -> AppResult<ResourceSnapshotInput> {
        preview_resource_snapshot(&self.resources)
    }
    fn selected_core_resource_fact(
        &self,
        resource: &ResourceSnapshotInput,
        architecture: &str,
    ) -> AppResult<String> {
        if !matches!(architecture, "x86" | "x64") {
            return Err(error(
                "core_architecture_unknown",
                "目标位数未知，未选择Core资源",
            ));
        }
        let manifest: serde_json::Value = serde_json::from_str(&resource.metadata_json)
            .map_err(|_| error("core_resources_invalid", "Core资源清单无效"))?;
        let files = manifest["files"]
            .as_array()
            .ok_or_else(|| error("core_resources_invalid", "Core文件清单缺失"))?;
        let mut facts = Vec::new();
        for entry in files
            .iter()
            .filter(|entry| entry["architecture"] == architecture)
        {
            let name = entry["runtime_path"]
                .as_str()
                .ok_or_else(|| error("core_resources_invalid", "Core资源路径缺失"))?;
            if name.is_empty()
                || name.contains(['/', '\\', ':', '\0'])
                || name == "."
                || name == ".."
            {
                return Err(error("core_resources_invalid", "Core资源路径无效"));
            }
            let expected_size = entry["size"]
                .as_u64()
                .filter(|size| *size > 0 && *size <= 16 * 1024 * 1024)
                .ok_or_else(|| error("core_resources_invalid", "Core资源大小无效"))?;
            let expected_hash = entry["sha256"]
                .as_str()
                .filter(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                })
                .ok_or_else(|| error("core_resources_invalid", "Core资源指纹无效"))?;
            let path = self
                .resources
                .join("core")
                .join(architecture)
                .join("bin")
                .join(name);
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|_| error("core_resources_missing", "所选架构Core文件缺失"))?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() != expected_size
            {
                return Err(error(
                    "core_resource_mismatch",
                    "所选架构Core文件大小或类型不匹配",
                ));
            }
            use std::io::Read;
            let file = File::open(path)
                .map_err(|_| error("core_resources_missing", "所选架构Core文件不可读"))?;
            let mut bytes = Vec::new();
            file.take(expected_size + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| error("core_resources_missing", "所选架构Core文件读取失败"))?;
            if bytes.len() as u64 != expected_size || content_hash(&bytes) != expected_hash {
                return Err(error(
                    "core_resource_mismatch",
                    "所选架构Core文件SHA不匹配，未请求注入",
                ));
            }
            facts.push(format!(
                "core/{architecture}/bin/{name} sha256={expected_hash}"
            ));
        }
        if facts.len() != 3 {
            return Err(error("core_resources_invalid", "所选架构Core文件集不完整"));
        }
        Ok(format!(
            "architecture={architecture} · {} · commit={} · manifest_sha256={} · {}",
            resource.contract_version,
            resource.version,
            resource.manifest_sha256,
            facts.join(" · ")
        ))
    }
}
impl HookBackend for CoreHookBackend {
    fn cover_session_guard(&self) -> AppResult<CoverSessionGuard> {
        let _guard = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        let (active, candidate) = self.trusted_bound_candidate()?;
        Ok(CoverSessionGuard {
            session_id: active.session_id,
            process_identity_sha256: active.target_identity_sha256,
            binding_identity_sha256: candidate
                .selection_identity()
                .ok_or_else(|| error("game_cover_stale", "窗口身份不完整"))?,
        })
    }
    fn with_verified_cover_binding_commit(
        &self,
        request: GameWindowActivationRequest,
        commit: &mut dyn FnMut() -> AppResult<CoverInstallOutcome>,
    ) -> AppResult<CoverInstallOutcome> {
        let _guard = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        let (active, candidate) = self.trusted_bound_candidate()?;
        if active.session_id != request.expected_session_id
            || candidate.selection_identity().as_deref()
                != Some(request.expected_target_identity_sha256.as_str())
        {
            return Err(error("game_cover_stale", "封面所属会话或窗口已变化"));
        }
        // Fresh external verification already ran outside the preview lock. This
        // final lease recheck prevents a switch/unbind between that proof and SQL.
        // Never spawn a potentially slow native probe while the preview is locked.
        commit()
    }
    fn game_window_binding(&self) -> AppResult<GameWindowBindingProjection> {
        self.read_game_window_binding()
    }
    fn game_window_activation_request(&self) -> AppResult<GameWindowActivationRequest> {
        let _guard = self.lease.try_lock().map_err(|_| {
            error(
                "game_window_activation_busy",
                "关联或窗口核验正在进行，请稍后重试",
            )
        })?;
        let (active, candidate) = self.trusted_bound_candidate()?;
        Ok(GameWindowActivationRequest {
            expected_session_id: active.session_id,
            expected_target_identity_sha256: candidate
                .selection_identity()
                .ok_or_else(|| error("game_window_stale", "窗口关联身份不完整"))?,
        })
    }
    fn show_associated_game(
        &self,
        request: GameWindowActivationRequest,
    ) -> AppResult<GameWindowActivationReceipt> {
        // This is the same guard used by association, disconnect and unlink.
        // A stale request can never raise a newly unrelated target by HWND alone.
        let _guard = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        let actual = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .clone()
            .ok_or_else(|| error("game_window_unbound", "当前没有已关联的游戏窗口"))?;
        if actual.session_id != request.expected_session_id {
            return Err(error(
                "game_window_activation_stale",
                "显示请求所属的会话或窗口目标已变化，请重新操作",
            ));
        }
        let (active, candidate) = self.trusted_bound_candidate()?;
        let window_identity = candidate
            .selection_identity()
            .ok_or_else(|| error("game_window_stale", "窗口关联身份不完整"))?;
        if window_identity != request.expected_target_identity_sha256 {
            return Err(error(
                "game_window_activation_stale",
                "显示请求所属的窗口关联已变化，请重新操作",
            ));
        }
        let window = candidate
            .window
            .as_ref()
            .ok_or_else(|| error("game_window_no_window", "尚未选择窗口"))?;
        let receipt = self
            .client
            .activate_bound_window(&candidate.settings, window)
            .map_err(|e| {
                error(
                    if e.starts_with("window_proof_busy:") {
                        "window_proof_busy"
                    } else {
                        "game_window_activation_unavailable"
                    },
                    if e.starts_with("window_proof_busy:") {
                        "窗口身份核验正在进行，请稍后重试"
                    } else {
                        "显示游戏请求未完成，请重新核对窗口"
                    },
                )
            })?;
        let state = match receipt.status.as_str() {
            "activated" if receipt.visible_after && receipt.foreground_after => {
                GameWindowActivationState::Activated
            }
            "visible_not_foreground" if receipt.visible_after && !receipt.foreground_after => {
                GameWindowActivationState::VisibleNotForeground
            }
            _ => {
                return Err(error(
                    match receipt.error_code.as_deref() {
                        Some("target_closed") => "game_window_closed",
                        Some("window_identity_changed") => "game_window_stale",
                        _ => "game_window_activation_failed",
                    },
                    "窗口已关闭、身份变化或未能显示，请重新核对",
                ));
            }
        };
        Ok(GameWindowActivationReceipt {
            session_id: active.session_id,
            target_identity_sha256: window_identity,
            state,
            was_minimized: receipt.was_minimized,
            show_was_visible: receipt.show_was_visible,
            set_foreground_accepted: receipt.set_foreground_accepted,
            visible_after: receipt.visible_after,
            foreground_after: receipt.foreground_after,
            observed_at_ms: window_observed_ms(),
        })
    }

    fn launch_game(&self, target: &GameTarget) -> AppResult<()> {
        let mut pending = self.launches.try_lock().map_err(|_| {
            AppError::new(
                "game_launch_pending",
                "launch",
                "已有游戏启动核对正在进行，请稍后重试",
            )
        })?;
        let plan = crate::platform::game_launch::prepare(target, self.environments()?)?;
        let identity = content_hash(format!(
            "{}\0{}",
            target.environment_key,
            plan.target_keys.join("\0")
        ));
        std::fs::create_dir_all(&self.target_locks)
            .map_err(|_| error("target_lock", "无法创建启动目标锁目录"))?;
        let lease = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.target_locks.join(format!("launch-{identity}.lock")))
            .map_err(|_| error("target_lock", "无法打开启动目标锁"))?;
        let mut lease =
            crate::platform::game_launch::PendingLaunchLease::try_lock(lease).map_err(|_| {
                AppError::new(
                    "game_launch_pending",
                    "launch",
                    "另一个客户端正在启动此目标，请先刷新运行目标",
                )
            })?;
        // Read-only probe is already bounded by the existing 30-second port.
        // An unavailable probe is not permission to risk a duplicate game launch.
        let targets = self
            .client
            .discover_launch_targets(&plan.settings)
            .map_err(|_| {
                AppError::new(
                    "launch_discovery_failed",
                    "launch",
                    "无法核对已有运行目标，未启动新实例，请刷新后重试",
                )
            })?;
        if plan.already_running(&targets)? {
            return Ok(());
        }
        pending.retain(|_, started| started.elapsed() < Duration::from_secs(10));
        if pending.contains_key(&identity) {
            return Err(AppError::new(
                "game_launch_pending",
                "launch",
                "刚发送过启动请求，请等待并刷新运行目标",
            ));
        }
        // The lock alone ends with this call. Persist the bounded pending period
        // before spawning so a second client cannot race a slow-starting process.
        crate::platform::game_launch::reserve_launch(&mut lease, std::time::SystemTime::now())?;
        // Keep the debounce even on timeout: a request may have reached Wine.
        let result = crate::platform::game_launch::run(plan.command(), Duration::from_secs(15));
        pending.insert(identity, std::time::Instant::now());
        result
    }
    fn targets(&self) -> AppResult<TargetCandidates> {
        self.discover_candidates()
    }
    fn association_candidate(&self, candidate_id: &str) -> AppResult<TargetCandidate> {
        self.association_candidate_projection(candidate_id)
    }
    fn with_associated_session_commit(
        &self,
        input: &HookSessionInput,
        commit: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        let mut active = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "关联状态不可用"))?;
        let current = active
            .as_mut()
            .ok_or_else(|| error("session_identity_conflict", "关联会话已退出"))?;
        if lease.as_ref().map(|entry| entry.0.as_str())
            != Some(input.target_identity_sha256.as_str())
            || current.session_id != input.session_id
            || current.target_identity_sha256 != input.target_identity_sha256
            || current.broker_instance_id != input.broker_instance_id
            || current.environment_key != input.environment_key
            || current.target_exe_path != input.target_exe_path
            || current.windows_pid != input.windows_pid
            || current.process_created_u64 != input.process_created_u64
            || current.architecture != input.architecture
        {
            return Err(error(
                "session_identity_conflict",
                "关联提交前目标身份已变化",
            ));
        }
        if current.game_id.is_some() && input.game_id.is_some() && current.game_id != input.game_id
        {
            return Err(error(
                "session_game_conflict",
                "当前会话已属于另一条游戏资料，请保留原归属",
            ));
        }
        // Neither producer nor another association can observe a provisional attribution.
        commit()?;
        if current.game_id.is_none() {
            current.game_id = input.game_id.clone();
        }
        Ok(())
    }
    fn associate(&self, request: AssociateRequest) -> AppResult<HookSessionInput> {
        self.associate_traced(request, &mut |_| Ok(()))
    }
    fn associate_traced(
        &self,
        request: AssociateRequest,
        record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        // Legacy callers receive the same final lease-bound identity check.
        let context = self.association_context(&request.candidate_id)?;
        let guard = AssociationExecutionGuard {
            candidate_identity_sha256: content_hash(format!(
                "{}\0{}\0{}\0{}\0{}",
                context.candidate.environment_key,
                context.candidate.executable_path,
                context.candidate.windows_pid,
                context.candidate.process_created_u64,
                context.candidate.architecture
            )),
            expected_active_session_id: context.active.as_ref().map(|s| s.session_id.clone()),
            expected_active_target_identity_sha256: context
                .active
                .as_ref()
                .map(|s| s.target_identity_sha256.clone()),
            expected_broker_instance_id: context
                .active
                .as_ref()
                .map(|s| s.broker_instance_id.clone()),
            expected_active: context.active,
        };
        self.associate_with_guard(request, Some(&guard), record)
    }
    fn associate_reviewed_traced(
        &self,
        request: AssociateRequest,
        guard: &AssociationExecutionGuard,
        record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        self.associate_with_guard(request, Some(guard), record)
    }
    fn association_context(&self, candidate_id: &str) -> AppResult<AssociationBackendContext> {
        let _lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾"))?;
        let candidate = self.association_candidate_projection(candidate_id)?;
        let active = self.read_active_context()?;
        Ok(AssociationBackendContext { candidate, active })
    }
    fn active_session_context(&self) -> AppResult<Option<HookSessionInput>> {
        let _lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾"))?;
        self.read_active_context()
    }
    fn with_active_session_guard(
        &self,
        expected: &HookSessionInput,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾"))?;
        let active = self
            .read_active_context()?
            .ok_or_else(|| error("reading_effect_stale", "游戏会话已经变化"))?;
        if !same_hook_session_facts(&active, expected) {
            return Err(error("reading_effect_stale", "窗口操作的活动会话已变化"));
        }
        effect()
    }
    fn with_no_active_session_guard(
        &self,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾"))?;
        if self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .is_some()
            || self.client.has_active_session()
            || self.client.snapshot().host_running
        {
            return Err(error(
                "inactive_session_preview_stale",
                "已有实际宿主，旧清理不能继续",
            ));
        }
        effect()
    }
    fn disconnect(&self) -> AppResult<()> {
        let mut lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        self.stop_owned_session()?;
        *self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态无效"))? = None;
        *lease = None;
        Ok(())
    }
    fn finish_disconnected_session(&self, session_id: &str) -> AppResult<bool> {
        // Serialize with association/unlink/disconnect, while leaving the sole
        // capture consumer free to persist the FIFO barrier below.
        let mut lease = self
            .lease
            .lock()
            .map_err(|_| error("target_lock", "目标租约不可用"))?;
        let expected = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .clone();
        let Some(expected) = expected.filter(|s| s.session_id == session_id) else {
            return Ok(false);
        };
        let snapshot = self.client.snapshot();
        let full_identity = content_hash(format!(
            "{}\0{}\0{}\0{}\0{}",
            expected.environment_key,
            expected.target_exe_path,
            expected.windows_pid,
            expected.process_created_u64,
            expected.architecture
        ));
        let native_matches = expected.target_identity_sha256 == full_identity
            && snapshot.session_id.as_deref() == Some(session_id)
            && snapshot.target.as_ref().is_some_and(|target| {
                target.pid == expected.windows_pid
                    && target.full_image_path.as_deref() == Some(expected.target_exe_path.as_str())
                    && target.creation_filetime.as_deref()
                        == Some(expected.process_created_u64.as_str())
                    && target.architecture.as_deref() == Some(expected.architecture.as_str())
            });
        if !native_matches
            || lease.as_ref().map(|(identity, _)| identity.as_str())
                != Some(expected.target_identity_sha256.as_str())
        {
            return Err(error(
                "capture_cleanup_stale",
                "退出收尾的宿主或目标身份已变化，已保留租约",
            ));
        }
        self.stop_owned_producers()?;
        let stopped = self.client.snapshot();
        if stopped.exit_code != Some(0)
            || !stopped.shutdown_acknowledged
            || stopped.integrity_lost
            || !stopped.errors.is_empty()
        {
            return Err(error(
                "capture_cleanup_unconfirmed",
                "宿主退出或输出完整性未确认，已保留目标租约，请查看诊断后重试",
            ));
        }
        let (acknowledged, consumed) = mpsc::sync_channel(1);
        self.drain_session_relay(Some(session_id.into()), Some(acknowledged))?;
        consumed
            .recv_timeout(Duration::from_secs(20))
            .map_err(|_| {
                error(
                    "capture_drain_pending",
                    "退出事实尚未持久确认，已保留目标租约",
                )
            })??;
        let mut active = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?;
        if active.as_ref().is_none_or(|current| {
            current.session_id != expected.session_id
                || current.target_identity_sha256 != expected.target_identity_sha256
                || current.windows_pid != expected.windows_pid
                || current.target_exe_path != expected.target_exe_path
                || current.process_created_u64 != expected.process_created_u64
                || current.architecture != expected.architecture
                || current.environment_key != expected.environment_key
        }) {
            return Ok(false);
        }
        let mut association_has_window = self
            .association_has_window
            .lock()
            .map_err(|_| error("capture_state", "关联状态不可用"))?;
        // No fallible operation may remain after ownership is released.
        *active = None;
        *lease = None;
        *association_has_window = false;
        Ok(true)
    }

    fn stop_and_drain(&self, acknowledged: mpsc::SyncSender<AppResult<()>>) -> AppResult<()> {
        self.disconnect()?;
        self.drain_relay(Some(acknowledged))
    }
    fn unlink_window(
        &self,
        request: GameWindowActivationRequest,
        preflight: &mut dyn FnMut() -> AppResult<Option<CommandReceipt>>,
        commit: &mut dyn FnMut() -> AppResult<CommandReceipt>,
    ) -> AppResult<CommandReceipt> {
        let _guard = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        // Replay comes first: A's old successful operation must not touch B.
        if let Some(receipt) = preflight()? {
            return Ok(receipt);
        }
        if self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .as_ref()
            .map(|s| s.session_id.as_str())
            != Some(request.expected_session_id.as_str())
        {
            return Err(error(
                "game_window_unbind_stale",
                "解绑请求所属的会话已变化，请重新操作",
            ));
        }
        let (active, candidate) = self.trusted_bound_candidate()?;
        if active.session_id != request.expected_session_id
            || candidate.selection_identity().as_deref()
                != Some(request.expected_target_identity_sha256.as_str())
        {
            return Err(error(
                "game_window_unbind_stale",
                "解绑请求所属的会话或窗口关联已变化，请重新操作",
            ));
        }
        // capture_lock -> lease -> preview binding -> Client session/binding
        // -> SQL actor. The actor executes only SQL; no callbacks into Client
        // or AppKit. Keep previews from seeing an uncommitted local clear.
        let mut preview = self
            .preview_binding
            .lock()
            .map_err(|_| error("capture_state", "窗口关联状态不可用"))?;
        let previous = preview.take();
        self.client.with_window_unbound(|| match commit() {
            Ok(receipt) => Ok(receipt),
            Err(error) => {
                *preview = previous;
                Err(error)
            }
        })
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        let mut subscription = self
            .client
            .subscribe(1024)
            .map_err(|_| error("capture_subscription", "无法订阅宿主事件"))?;
        let (tx, rx) = mpsc::sync_channel(512);
        let (control_tx, control_rx) = mpsc::sync_channel::<RelayDrain>(1);
        *self
            .relay_control
            .lock()
            .map_err(|_| error("capture_state", "事件适配器控制不可用"))? = Some(control_tx);
        let active = self.active.clone();
        let failed = self.failed.clone();
        let client = self.client.clone();
        std::thread::Builder::new()
            .name("core-event-adapter".into())
            .spawn(move || {
                loop {
                    match control_rx.try_recv() {
                        Ok(request) => {
                            let result = (|| {
                                if subscription.dropped_total() > 0 {
                                    tx.send(queue_loss(
                                        subscription.lost_session_id(),
                                        subscription.dropped_total(),
                                        "subscription_overflow",
                                    ))
                                    .map_err(|_| {
                                        error("capture_consumer_closed", "事实消费者已退出")
                                    })?;
                                }
                                let current = active
                                    .lock()
                                    .map_err(|_| error("capture_state", "会话状态不可用"))?
                                    .clone()
                                    .filter(|current| {
                                        request
                                            .session_id
                                            .as_ref()
                                            .is_none_or(|id| id == &current.session_id)
                                    });
                                if let Some(current) = current {
                                    for event in subscription.drain_received() {
                                        if event.session_id.as_deref()
                                            == Some(current.session_id.as_str())
                                        {
                                            tx.send(project_event(&current, event)).map_err(
                                                |_| {
                                                    error(
                                                        "capture_consumer_closed",
                                                        "事实消费者已退出",
                                                    )
                                                },
                                            )?;
                                        }
                                    }
                                }
                                if let Some(acknowledged) = request.application_ack {
                                    let barrier = match request.session_id {
                                        Some(session_id) => HookUpdate::SessionDrainBarrier {
                                            session_id,
                                            acknowledged,
                                        },
                                        None => HookUpdate::DrainBarrier { acknowledged },
                                    };
                                    tx.send(barrier).map_err(|_| {
                                        error("capture_consumer_closed", "消费确认无法交付")
                                    })?;
                                }
                                Ok(())
                            })();
                            let _ = request.relay_ack.send(result);
                            continue;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    let event = match subscription.recv_timeout(Duration::from_millis(20)) {
                        Ok(event) => event,
                        Err(SubscriptionError::Timeout) => continue,
                        Err(SubscriptionError::Closed) => break,
                        Err(SubscriptionError::IntegrityLost { dropped_total }) => {
                            let session_id = subscription.lost_session_id();
                            if let Some(id) = &session_id
                                && let Ok(mut set) = failed.lock()
                            {
                                set.insert(id.clone());
                            }
                            if tx
                                .send(queue_loss(
                                    session_id,
                                    dropped_total,
                                    "subscription_overflow",
                                ))
                                .is_err()
                            {
                                break;
                            }
                            match client.subscribe(1024) {
                                Ok(next) => {
                                    let current = active.lock().ok().and_then(|s| s.clone());
                                    if let Some(current) = current {
                                        for accepted in subscription.drain_received() {
                                            if accepted.session_id.as_deref()
                                                == Some(current.session_id.as_str())
                                                && tx
                                                    .send(project_event(&current, accepted))
                                                    .is_err()
                                            {
                                                return;
                                            }
                                        }
                                    }
                                    subscription = next;
                                    continue;
                                }
                                Err(_) => break,
                            }
                        }
                    };
                    let event_session = event.session_id.clone();
                    // Association holds this mutex until the complete identity is
                    // installed. Runtime separately orders persistence before updates.
                    let Ok(current) = active.lock().map(|v| v.clone()) else {
                        break;
                    };
                    let Some(current) = current else {
                        continue;
                    };
                    if event_session.as_deref() != Some(current.session_id.as_str()) {
                        continue;
                    }
                    let update = project_event(&current, event);
                    // Outside the game callback. A full consumer poisons only this session; a new explicit association can recover.
                    match tx.try_send(update) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Disconnected(_)) => break,
                        Err(mpsc::TrySendError::Full(_)) => {
                            if let Some(id) = &event_session
                                && let Ok(mut set) = failed.lock()
                            {
                                set.insert(id.clone());
                            }
                            if tx
                                .send(queue_loss(event_session, 1, "application_queue_overflow"))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|_| error("capture_subscription", "无法启动宿主事件适配器"))?;
        Ok(rx)
    }
    fn resource_info(&self) -> ResourceInfo {
        match self.native_resource() {
            Ok(r) => ResourceInfo {
                kind: r.kind,
                version: r.version,
                manifest_sha256: r.manifest_sha256,
                available: true,
                detail: "独立Core ABI1 / JSONL v2；两位数资源由broker逐文件校验".into(),
            },
            Err(e) => ResourceInfo {
                kind: "core".into(),
                version: String::new(),
                manifest_sha256: String::new(),
                available: false,
                detail: e.message,
            },
        }
    }
}

#[cfg(test)]
mod candidate_projection_tests {
    use super::*;
    #[test]
    fn title_proof_maps_only_the_exact_typed_unicode_failure() {
        for message in [
            "window_title_proof_requires_unicode",
            "window_title_proof_requires_unicode: untrusted stderr secret",
        ] {
            let error = window_title_proof_error(message);
            assert_eq!(error.code, "preview_window_selection_required");
            assert_eq!(
                error.message,
                "请通过系统选择对应的游戏窗口以预览；原标题未更改，游戏取文不受影响。"
            );
            assert!(!error.message.contains("secret"));
        }
        for message in [
            "window_title_proof_requires_unicode_extra: secret",
            "prefix window_title_proof_requires_unicode: secret",
            "window_title_proof_requires_unicode secret",
            "arbitrary secret",
        ] {
            let error = window_title_proof_error(message);
            assert_eq!(error.code, "no_capture_window");
            assert!(!error.message.contains("secret"));
        }
    }
    #[test]
    fn terminal_lease_requires_cleanup_while_live_target_switch_requires_confirmation() {
        for status in ["starting", "ready", "attaching", "connected"] {
            assert_eq!(
                check_target_switch(Some("old-target"), "new-target", status, false)
                    .unwrap_err()
                    .code,
                "replace_required"
            );
            assert!(check_target_switch(Some("old-target"), "new-target", status, true).is_ok());
        }
        for status in [
            "disconnected",
            "disconnecting",
            "exited",
            "stopped",
            "error",
        ] {
            for confirmed in [false, true] {
                assert_eq!(
                    check_target_switch(Some("old-target"), "new-target", status, confirmed)
                        .unwrap_err()
                        .code,
                    "capture_cleanup_pending"
                );
                assert!(check_target_switch(None, "new-target", status, confirmed).is_ok());
                assert!(
                    check_target_switch(Some("new-target"), "new-target", status, confirmed)
                        .is_ok()
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn natural_exit_shuts_down_owned_child_and_keeps_lease_until_fifo_ack() {
        use std::os::unix::fs::PermissionsExt;
        for failed_shutdown in [false, true] {
            let root =
                std::env::temp_dir().join(format!("luna-natural-exit-{}", uuid::Uuid::new_v4()));
            let resources = root.join("resources");
            for directory in ["brokers/x64", "brokers/x86", "core/x86/bin"] {
                std::fs::create_dir_all(resources.join(directory)).unwrap();
            }
            for file in [
                "brokers/x64/target_probe.exe",
                "brokers/x86/lunahook-host.exe",
            ] {
                std::fs::write(resources.join(file), "controlled resource").unwrap();
            }
            let launcher = root.join("owned-host.py");
            std::fs::write(&launcher, r#"#!/usr/bin/env python3
import json,sys,pathlib
if any(x.endswith('target_probe.exe') for x in sys.argv):
    print(json.dumps(dict(processes=[dict(pid=100,exe='controlled.exe',full_image_path='C:\\controlled.exe',creation_filetime='1',architecture='x86',errors=[])])),flush=True)
    raise SystemExit(0)
def event(name,**fields):
    print(json.dumps(dict(v=2,event=name,**fields),ensure_ascii=False),flush=True)
event('ready',host_arch='x86',abi_version=1,upstream_commit='e9242ebac32a09e911f306b7c90714891c217302',abi_profile='lunahooks-core-abi1-cdecl-seven')
for line in sys.stdin:
    request=json.loads(line)
    if request['method']=='attach':
        event('process_connected',pid=100)
        event('process_disconnected',pid=100)
        event('process_disconnected',pid=100)
    elif request['method']=='shutdown':
        pathlib.Path(__file__).with_suffix('.shutdown').write_text(request['id'])
        if pathlib.Path(__file__).with_suffix('.failure').exists():
            raise SystemExit(17)
        event('text',stream=dict(pid=100,address='1',context='0',context2='0'),hook_code='fixture',hook_name='fixture',text='終了時の原文')
        event('log',kind=1,message='native final diagnostic')
        event('response',id=request['id'],result=dict(status='shutting_down',deadline_ms=1000))
        break
"#).unwrap();
            std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
            if failed_shutdown {
                std::fs::write(
                    launcher.with_extension("failure"),
                    "controlled host shutdown error",
                )
                .unwrap();
            }
            let backend = Arc::new(CoreHookBackend::new(
                resources,
                root.join("logs"),
                root.join("locks"),
            ));
            let updates = backend.subscribe().unwrap();
            let mut active = backend.active.lock().unwrap();
            let target = Target {
                pid: 100,
                exe: "controlled.exe".into(),
                full_image_path: Some("C:\\controlled.exe".into()),
                creation_filetime: Some("1".into()),
                architecture: Some("x86".into()),
                errors: vec![],
            };
            let settings = LaunchSettings {
                wine_path: launcher.to_string_lossy().into_owned(),
                bottle: "controlled".into(),
            };
            backend
                .client
                .connect(settings.clone(), target.clone())
                .unwrap();
            let candidate = Candidate {
                target,
                window: None,
                settings,
                environment_key: "controlled".into(),
            };
            let mut input = HookSessionInput {
                session_id: String::new(),
                game_id: None,
                environment_key: String::new(),
                target_exe_path: String::new(),
                windows_pid: 100,
                process_created_u64: String::new(),
                architecture: "x86".into(),
                broker_instance_id: "controlled-owned-host".into(),
                resource: ResourceSnapshotInput {
                    snapshot_id: "controlled-resource".into(),
                    kind: "core".into(),
                    version: "controlled".into(),
                    manifest_sha256: content_hash("controlled"),
                    contract_version: "1".into(),
                    metadata_json: "{}".into(),
                },
                target_identity_sha256: String::new(),
            };
            input.session_id = backend.client.snapshot().session_id.unwrap();
            input.target_exe_path = "C:\\controlled.exe".into();
            input.process_created_u64 = "1".into();
            input.environment_key = candidate.environment_key.clone();
            input.target_identity_sha256 = candidate.process_identity().unwrap();
            let lease = std::fs::File::create(root.join("target.lock")).unwrap();
            lease.try_lock().unwrap();
            *backend.lease.lock().unwrap() = Some((input.target_identity_sha256.clone(), lease));
            *active = Some(input.clone());
            drop(active);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while backend.client.has_active_session() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "target exit never shut down its host: {:?}",
                    backend.client.snapshot()
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(launcher.with_extension("shutdown").is_file());
            let requests = std::fs::read_to_string(
                root.join("logs")
                    .join(&input.session_id)
                    .join("requests.jsonl"),
            )
            .unwrap();
            assert_eq!(
                requests
                    .lines()
                    .filter(|line| line.contains("\"method\":\"shutdown\""))
                    .count(),
                1
            );
            if failed_shutdown {
                assert_eq!(backend.client.snapshot().exit_code, Some(17));
                assert_eq!(
                    backend
                        .finish_disconnected_session(&input.session_id)
                        .unwrap_err()
                        .code,
                    "capture_cleanup_unconfirmed"
                );
                assert!(backend.active.lock().unwrap().is_some());
                assert!(backend.lease.lock().unwrap().is_some());
                backend.disconnect().unwrap();
                drop(updates);
                drop(backend);
                std::fs::remove_dir_all(root).unwrap();
                continue;
            }
            assert_eq!(backend.client.snapshot().exit_code, Some(0));
            assert!(backend.client.snapshot().errors.is_empty());
            assert!(backend.client.snapshot().shutdown_acknowledged);
            // A stale A request cannot stop/release the current target.
            assert!(
                !backend
                    .finish_disconnected_session("older-session")
                    .unwrap()
            );
            assert!(backend.active.lock().unwrap().is_some());
            let finishing = backend.clone();
            let session_id = input.session_id.clone();
            let finished =
                std::thread::spawn(move || finishing.finish_disconnected_session(&session_id));
            let mut saw_tail = false;
            let mut saw_log = false;
            loop {
                match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
                    HookUpdate::Text(text) => {
                        saw_tail |= text.text == "終了時の原文";
                    }
                    HookUpdate::Control { event, .. } => {
                        saw_log |= event.detail.message.contains("native final diagnostic");
                    }
                    HookUpdate::SessionDrainBarrier {
                        session_id,
                        acknowledged,
                    } => {
                        assert_eq!(session_id, input.session_id);
                        assert!(saw_tail && saw_log);
                        assert!(
                            backend.lease.try_lock().is_err(),
                            "lease released before final consumer ACK"
                        );
                        assert!(backend.active.lock().unwrap().is_some());
                        // These are called by application operations while holding
                        // capture_lock. None may wait for the lease while the FIFO
                        // consumer needs that same capture_lock to acknowledge us.
                        let started = std::time::Instant::now();
                        let activation = GameWindowActivationRequest {
                            expected_session_id: input.session_id.clone(),
                            expected_target_identity_sha256: input.target_identity_sha256.clone(),
                        };
                        assert_eq!(
                            backend
                                .unlink_window(
                                    activation.clone(),
                                    &mut || panic!("busy unlink reached SQL preflight"),
                                    &mut || panic!("busy unlink committed")
                                )
                                .unwrap_err()
                                .code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend
                                .associate(AssociateRequest {
                                    operation_id: "concurrent-association".into(),
                                    game_id: None,
                                    candidate_id: "not-used-while-cleaning".into(),
                                    confirm_switch: false
                                })
                                .unwrap_err()
                                .code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend
                                .with_associated_session_commit(&input, &mut || panic!(
                                    "busy association committed"
                                ))
                                .unwrap_err()
                                .code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend
                                .with_verified_cover_binding_commit(
                                    activation.clone(),
                                    &mut || panic!("busy cover committed")
                                )
                                .unwrap_err()
                                .code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend.show_associated_game(activation).unwrap_err().code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend.cover_session_guard().unwrap_err().code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend.game_window_binding().unwrap_err().code,
                            "capture_cleanup_pending"
                        );
                        assert_eq!(
                            backend.disconnect().unwrap_err().code,
                            "capture_cleanup_pending"
                        );
                        assert!(
                            started.elapsed() < Duration::from_secs(2),
                            "capture-lock operations waited for the native cleanup lease"
                        );
                        acknowledged
                            .send(Err(error(
                                "controlled_persistence_failed",
                                "controlled rejected ACK",
                            )))
                            .unwrap();
                        break;
                    }
                    _ => {}
                }
            }
            assert_eq!(
                finished.join().unwrap().unwrap_err().code,
                "controlled_persistence_failed"
            );
            assert!(backend.lease.lock().unwrap().is_some());
            assert!(backend.active.lock().unwrap().is_some());
            backend
                .active
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .process_created_u64 = "2".into();
            assert_eq!(
                backend
                    .finish_disconnected_session(&input.session_id)
                    .unwrap_err()
                    .code,
                "capture_cleanup_stale"
            );
            assert!(
                backend.lease.lock().unwrap().is_some(),
                "a full-target mismatch must retain ownership"
            );
            backend
                .active
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .process_created_u64 = input.process_created_u64.clone();
            let finishing = backend.clone();
            let session_id = input.session_id.clone();
            let finished =
                std::thread::spawn(move || finishing.finish_disconnected_session(&session_id));
            loop {
                if let HookUpdate::SessionDrainBarrier { acknowledged, .. } =
                    updates.recv_timeout(Duration::from_secs(5)).unwrap()
                {
                    acknowledged.send(Ok(())).unwrap();
                    break;
                }
            }
            assert!(finished.join().unwrap().unwrap());
            assert!(backend.active.lock().unwrap().is_none());
            assert!(backend.preview_binding.lock().unwrap().is_none());
            assert!(backend.lease.lock().unwrap().is_none());
            assert!(!backend.client.has_active_session());
            assert!(
                !backend
                    .finish_disconnected_session(&input.session_id)
                    .unwrap()
            );
            let mut later = input.clone();
            later.session_id = "later-session".into();
            later.process_created_u64 = "2".into();
            *backend.active.lock().unwrap() = Some(later.clone());
            let later_lease = std::fs::File::create(root.join("later-target.lock")).unwrap();
            later_lease.try_lock().unwrap();
            *backend.lease.lock().unwrap() = Some(("later-target".into(), later_lease));
            assert!(
                !backend
                    .finish_disconnected_session(&input.session_id)
                    .unwrap()
            );
            assert_eq!(
                backend.active.lock().unwrap().as_ref().unwrap().session_id,
                later.session_id
            );
            assert_eq!(
                backend.lease.lock().unwrap().as_ref().unwrap().0,
                "later-target"
            );
            drop(updates);
            drop(backend);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    fn selected_resource_fixture() -> (CoreHookBackend, PathBuf, ResourceSnapshotInput) {
        let root =
            std::env::temp_dir().join(format!("luna-selected-resource-{}", uuid::Uuid::new_v4()));
        let mut files = Vec::new();
        for architecture in ["x86", "x64"] {
            let bin = root.join("core").join(architecture).join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            for name in ["Hook.dll", "Host.dll", "Inject.exe"] {
                let bytes = format!("controlled-{architecture}-{name}").into_bytes();
                std::fs::write(bin.join(name), &bytes).unwrap();
                files.push(serde_json::json!({"architecture": architecture,"runtime_path":name,"size":bytes.len(),"sha256":content_hash(&bytes)}));
            }
        }
        let metadata_json = serde_json::json!({"files":files}).to_string();
        let resource = ResourceSnapshotInput {
            snapshot_id: "selected-resource-fixture".into(),
            kind: "core".into(),
            version: "controlled-commit".into(),
            manifest_sha256: content_hash(&metadata_json),
            contract_version: "abi1-jsonl2".into(),
            metadata_json,
        };
        (
            CoreHookBackend::new(root.clone(), root.join("logs"), root.join("locks")),
            root,
            resource,
        )
    }
    #[cfg(unix)]
    #[test]
    fn stale_switch_target_is_verified_before_stopping_the_real_owned_host_and_after_drain() {
        use std::os::unix::fs::PermissionsExt;
        for (has_window, changes_after_drain) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let (backend, root, resource) = selected_resource_fixture();
            let mut manifest: serde_json::Value =
                serde_json::from_str(&resource.metadata_json).unwrap();
            manifest["abi_version"] = 1.into();
            manifest["protocol_version"] = 2.into();
            manifest["commit"] = "e9242ebac32a09e911f306b7c90714891c217302".into();
            std::fs::write(root.join("core-runtime.lock.json"), manifest.to_string()).unwrap();
            for architecture in ["x86", "x64"] {
                std::fs::create_dir_all(root.join("brokers").join(architecture)).unwrap();
                std::fs::write(
                    root.join("brokers")
                        .join(architecture)
                        .join("lunahook-host.exe"),
                    "controlled host resource",
                )
                .unwrap();
            }
            std::fs::write(
                root.join("brokers/x64/target_probe.exe"),
                "controlled probe resource",
            )
            .unwrap();
            let launcher = root.join("preflight.py");
            std::fs::write(&launcher,r#"#!/usr/bin/env python3
import json,sys,pathlib
base=pathlib.Path(__file__)
a=dict(pid=100,exe='A.exe',full_image_path='C:\\Games\\A.exe',creation_filetime='1',architecture='x86',errors=[])
b=dict(pid=200,exe='B.exe',full_image_path='C:\\Games\\B.exe',creation_filetime='2',architecture='x86',errors=[])
if any(value.endswith('target_probe.exe') for value in sys.argv):
    is_b='--all' in sys.argv or '--verify-bound-window' in sys.argv
    available=base.with_suffix('.valid-before-drain').exists() and not base.with_suffix('.shutdown').exists()
    if is_b:
        with base.with_suffix('.checks').open('a') as stream:stream.write(('valid' if available else 'stale')+'\n')
    if '--verify-bound-window' in sys.argv:
        print(json.dumps(dict(status='verified' if available else 'stale',windowTitle='B' if available else None,errorCode=None if available else 'target_changed')),flush=True)
    else:print(json.dumps(dict(processes=[a]+([b] if is_b and available else []))),flush=True)
    raise SystemExit(0)
def event(name,**fields):print(json.dumps(dict(v=2,event=name,**fields)),flush=True)
event('ready',host_arch='x86',abi_version=1,upstream_commit='e9242ebac32a09e911f306b7c90714891c217302',abi_profile='lunahooks-core-abi1-cdecl-seven')
for line in sys.stdin:
    request=json.loads(line)
    if request['method']=='attach':event('process_connected',pid=100)
    if request['method']=='shutdown':
        base.with_suffix('.shutdown').write_text(request['id']);event('response',id=request['id'],result=dict(status='shutting_down',deadline_ms=1000));break
"#).unwrap();
            std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
            if changes_after_drain {
                std::fs::write(
                    launcher.with_extension("valid-before-drain"),
                    "valid then disappears",
                )
                .unwrap();
            }
            let settings = LaunchSettings {
                wine_path: launcher.to_string_lossy().into_owned(),
                bottle: "controlled".into(),
            };
            let target_a = Target {
                pid: 100,
                exe: "A.exe".into(),
                full_image_path: Some("C:\\Games\\A.exe".into()),
                creation_filetime: Some("1".into()),
                architecture: Some("x86".into()),
                errors: vec![],
            };
            backend
                .client
                .connect(settings.clone(), target_a.clone())
                .unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while backend.client.snapshot().status != "connected" {
                assert!(
                    std::time::Instant::now() < deadline,
                    "owned A did not connect"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            let native = backend.client.snapshot();
            let mut inactive_callbacks = 0;
            assert!(
                backend
                    .with_no_active_session_guard(&mut || {
                        inactive_callbacks += 1;
                        Ok(())
                    })
                    .is_err(),
                "a running actual Client cannot grant inactive authority before cached attribution"
            );
            assert_eq!(inactive_callbacks, 0);
            let candidate_a = Candidate {
                target: target_a,
                window: None,
                settings: settings.clone(),
                environment_key: "crossover:/controlled".into(),
            };
            let active = HookSessionInput {
                session_id: native.session_id.unwrap(),
                game_id: None,
                environment_key: candidate_a.environment_key.clone(),
                target_exe_path: "C:\\Games\\A.exe".into(),
                windows_pid: 100,
                process_created_u64: "1".into(),
                architecture: "x86".into(),
                broker_instance_id: "old-broker".into(),
                resource: backend.native_resource().unwrap(),
                target_identity_sha256: candidate_a.process_identity().unwrap(),
            };
            let lease = std::fs::File::create(root.join("held-a.lock")).unwrap();
            lease.try_lock().unwrap();
            *backend.lease.lock().unwrap() = Some((active.target_identity_sha256.clone(), lease));
            *backend.active.lock().unwrap() = Some(active.clone());
            let target_b = Target {
                pid: 200,
                exe: "B.exe".into(),
                full_image_path: Some("C:\\Games\\B.exe".into()),
                creation_filetime: Some("2".into()),
                architecture: Some("x86".into()),
                errors: vec![],
            };
            let candidate_b = Candidate {
                target: target_b.clone(),
                window: has_window.then(|| WindowTarget {
                    hwnd: "200".into(),
                    title: "B".into(),
                    class_name: "GameClass".into(),
                    rect: hook_client::WindowRect {
                        left: 0,
                        top: 0,
                        right: 800,
                        bottom: 600,
                    },
                    target: target_b,
                }),
                settings,
                environment_key: active.environment_key.clone(),
            };
            let identity_b = candidate_b.process_identity().unwrap();
            let projected = backend.remember(vec![(candidate_b, "B".into())]).unwrap();
            let request = |operation: &str| AssociateRequest {
                operation_id: operation.into(),
                game_id: None,
                candidate_id: projected.candidates[0].candidate_id.clone(),
                confirm_switch: true,
            };
            let guard = AssociationExecutionGuard {
                candidate_identity_sha256: identity_b.clone(),
                expected_active_session_id: Some(active.session_id.clone()),
                expected_active_target_identity_sha256: Some(active.target_identity_sha256.clone()),
                expected_broker_instance_id: Some(active.broker_instance_id.clone()),
                expected_active: Some(active.clone()),
            };
            let mutations: [fn(&mut HookSessionInput); 7] = [
                |s| s.game_id = Some("changed-game-owner".into()),
                |s| s.resource.snapshot_id.push_str("-changed"),
                |s| s.resource.kind.push_str("-changed"),
                |s| s.resource.version.push_str("-changed"),
                |s| s.resource.manifest_sha256 = "f".repeat(64),
                |s| s.resource.contract_version.push_str("-changed"),
                |s| s.resource.metadata_json = "{\"changed\":true}".into(),
            ];
            for mutate in mutations {
                let mut changed = active.clone();
                mutate(&mut changed);
                *backend.active.lock().unwrap() = Some(changed);
                assert_eq!(
                    backend
                        .associate_reviewed_traced(
                            request("changed-full-owner"),
                            &guard,
                            &mut |_| Ok(())
                        )
                        .unwrap_err()
                        .code,
                    "association_review_stale"
                );
                assert!(!launcher.with_extension("shutdown").exists());
                assert_eq!(
                    backend.client.snapshot().session_id.as_deref(),
                    Some(active.session_id.as_str())
                );
                *backend.active.lock().unwrap() = Some(active.clone());
            }
            // Actual corrupt resources and a competing OS file lock must fail
            // before issuing a shutdown request to the owned A process.
            let resource_path = root.join("core/x86/bin/Hook.dll");
            let original_resource = std::fs::read(&resource_path).unwrap();
            std::fs::write(&resource_path, b"tampered resource bytes").unwrap();
            assert_eq!(
                backend
                    .associate(request("resource-preflight"))
                    .unwrap_err()
                    .code,
                "core_resource_mismatch"
            );
            assert!(!launcher.with_extension("shutdown").exists());
            assert_eq!(
                backend.client.snapshot().session_id.as_deref(),
                Some(active.session_id.as_str())
            );
            std::fs::write(&resource_path, original_resource).unwrap();
            std::fs::create_dir_all(&backend.target_locks).unwrap();
            let competing_lease =
                std::fs::File::create(backend.target_locks.join(format!("{identity_b}.lock")))
                    .unwrap();
            competing_lease.try_lock().unwrap();
            assert_eq!(
                backend
                    .associate(request("target-lease-preflight"))
                    .unwrap_err()
                    .code,
                "target_in_use"
            );
            assert!(!launcher.with_extension("shutdown").exists());
            assert_eq!(
                backend.client.snapshot().session_id.as_deref(),
                Some(active.session_id.as_str())
            );
            drop(competing_lease);
            let result = backend.associate(request("actual-preflight"));
            assert_eq!(result.unwrap_err().code, "stale_target");
            let checks = std::fs::read_to_string(launcher.with_extension("checks")).unwrap();
            if changes_after_drain {
                assert_eq!(checks.lines().collect::<Vec<_>>(), ["valid", "stale"]);
                assert!(launcher.with_extension("shutdown").exists());
            } else {
                assert_eq!(checks.lines().collect::<Vec<_>>(), ["stale"]);
                assert!(!launcher.with_extension("shutdown").exists());
                assert_eq!(
                    backend.client.snapshot().session_id.as_deref(),
                    Some(active.session_id.as_str())
                );
                assert!(backend.client.has_active_session());
                assert_eq!(
                    backend
                        .active
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .target_identity_sha256,
                    active.target_identity_sha256
                );
            }
            backend.disconnect().unwrap();
            backend
                .with_no_active_session_guard(&mut || {
                    inactive_callbacks += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(inactive_callbacks, 1);
            drop(backend);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn selected_core_resource_facts_bind_actual_files_and_preserve_architecture() {
        let (backend, root, resource) = selected_resource_fixture();
        let facts = backend
            .selected_core_resource_fact(&resource, "x86")
            .unwrap();
        assert!(
            facts.contains("architecture=x86")
                && facts.contains("abi1-jsonl2")
                && facts.contains(&resource.manifest_sha256)
        );
        assert!(
            facts.contains("core/x86/bin/Hook.dll")
                && facts.contains("core/x86/bin/Host.dll")
                && facts.contains("core/x86/bin/Inject.exe")
        );
        assert!(!facts.contains("core/x64/") && !facts.contains(root.to_str().unwrap()));
        assert!(facts.contains(&content_hash(b"controlled-x86-Hook.dll")));
        assert!(
            backend
                .selected_core_resource_fact(&resource, "x64")
                .unwrap()
                .contains("architecture=x64")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn selected_core_resource_facts_reject_tampering_unknown_arch_and_unsafe_manifest() {
        let (backend, root, resource) = selected_resource_fixture();
        assert_eq!(
            backend
                .selected_core_resource_fact(&resource, "arm64")
                .unwrap_err()
                .code,
            "core_architecture_unknown"
        );
        let hook = root.join("core/x86/bin/Hook.dll");
        let size = std::fs::metadata(&hook).unwrap().len() as usize;
        std::fs::write(&hook, vec![b'X'; size]).unwrap();
        assert_eq!(
            backend
                .selected_core_resource_fact(&resource, "x86")
                .unwrap_err()
                .code,
            "core_resource_mismatch"
        );
        std::fs::write(&hook, b"changed").unwrap();
        assert_eq!(
            backend
                .selected_core_resource_fact(&resource, "x86")
                .unwrap_err()
                .code,
            "core_resource_mismatch"
        );
        let mut bad = resource.clone();
        let mut manifest: serde_json::Value = serde_json::from_str(&bad.metadata_json).unwrap();
        manifest["files"][3]["runtime_path"] = "../Hook.dll".into();
        bad.metadata_json = manifest.to_string();
        assert_eq!(
            backend
                .selected_core_resource_fact(&bad, "x64")
                .unwrap_err()
                .code,
            "core_resources_invalid"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn concurrent_launch_is_rejected_without_mutating_a_session_or_candidate() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let _in_progress = backend.launches.lock().unwrap();
        let target = GameTarget {
            environment_kind: "unresolved".into(),
            environment_key: String::new(),
            bottle_locator: None,
            launch_path: None,
            canonical_exe_path: None,
            target_path_key: None,
            working_directory: None,
            arguments: vec![],
            architecture: None,
            verification_state: "unresolved".into(),
        };
        assert_eq!(
            backend.launch_game(&target).unwrap_err().code,
            "game_launch_pending"
        );
        assert!(backend.active.lock().unwrap().is_none());
        assert!(backend.candidates.lock().unwrap().is_empty());
        assert!(backend.lease.lock().unwrap().is_none());
        assert!(!backend.client.has_active_session());
    }

    fn candidate(window: bool) -> Candidate {
        let target = Target {
            pid: 812,
            exe: "Example.exe".into(),
            full_image_path: Some("C:\\Games\\Example.exe".into()),
            creation_filetime: Some("18446744073709551615".into()),
            architecture: Some("x64".into()),
            errors: vec![],
        };
        Candidate {
            window: window.then(|| WindowTarget {
                hwnd: "18446744073709551614".into(),
                title: "Window title from probe".into(),
                class_name: "ExampleClass".into(),
                rect: hook_client::WindowRect {
                    left: 10,
                    top: 20,
                    right: 1010,
                    bottom: 720,
                },
                target: target.clone(),
            }),
            target,
            settings: LaunchSettings {
                wine_path: "/test/wine".into(),
                bottle: "Probe bottle".into(),
            },
            environment_key: "crossover:/test/bottle".into(),
        }
    }
    #[test]
    fn collector_environment_binds_launcher_and_bottle_even_when_candidate_id_is_unchanged() {
        let original = candidate(true);
        let environments = vec![(original.settings.clone(), original.environment_key.clone())];
        assert!(collector_environment_matches(&original, &environments));
        for settings in [
            LaunchSettings {
                wine_path: "/other/wine".into(),
                ..original.settings.clone()
            },
            LaunchSettings {
                bottle: "other".into(),
                ..original.settings.clone()
            },
        ] {
            let mut changed = original.clone();
            changed.settings = settings;
            assert_eq!(changed.selection_identity(), original.selection_identity());
            assert!(!collector_environment_matches(&changed, &environments));
        }
        let mut changed = original;
        changed.environment_key.push_str("-other");
        assert!(!collector_environment_matches(&changed, &environments));
    }
    #[test]
    fn verified_windows_identity_is_projected_without_inventing_macos_mapping() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let projected = backend
            .remember(vec![(candidate(true), "Window title from probe".into())])
            .unwrap();
        let result = &projected.candidates[0];
        assert!(result.has_window);
        assert_eq!(result.windows_pid, 812);
        assert_eq!(result.process_created_u64, "18446744073709551615");
        assert_eq!(
            result.window_handle.as_deref(),
            Some("18446744073709551614")
        );
        assert_eq!(result.macos_host_pid, None);
        assert_eq!(result.macos_window_id, None);
        let json = serde_json::to_value(result).unwrap();
        assert!(json["windowHandle"].is_string());
        assert!(json["macosHostPid"].is_null());
        assert!(json["macosWindowId"].is_null());
    }
    #[test]
    fn process_only_candidate_does_not_claim_window_identity_and_refresh_discards_old_candidates() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let previous = backend
            .remember(vec![(candidate(true), "title".into())])
            .unwrap();
        let refreshed = backend
            .remember(vec![(candidate(false), "Example.exe".into())])
            .unwrap();
        assert!(!refreshed.candidates[0].has_window);
        assert_eq!(refreshed.candidates[0].window_handle, None);
        assert_eq!(refreshed.candidates[0].macos_host_pid, None);
        assert_eq!(refreshed.candidates[0].macos_window_id, None);
        assert_ne!(
            previous.candidates[0].candidate_id,
            refreshed.candidates[0].candidate_id
        );
        assert!(
            backend
                .candidates
                .lock()
                .unwrap()
                .values()
                .all(|c| c.window.is_none())
        );
        let mut incomplete = candidate(true);
        incomplete.target.creation_filetime = None;
        assert!(
            backend
                .remember(vec![(incomplete, "title".into())])
                .unwrap()
                .candidates
                .is_empty()
        );
        assert!(backend.candidates.lock().unwrap().is_empty());
    }
    #[test]
    fn distinct_windows_in_one_process_keep_the_selected_hwnd_in_the_candidate_map() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let first = candidate(true);
        let mut second = first.clone();
        second.window.as_mut().unwrap().hwnd = "18446744073709551613".into();
        // Equal titles must not collapse different windows or select the last one.
        let projected = backend
            .remember(vec![
                (first.clone(), "Same game title".into()),
                (second.clone(), "Same game title".into()),
            ])
            .unwrap();
        assert_eq!(projected.candidates.len(), 2);
        assert_ne!(
            projected.candidates[0].candidate_id,
            projected.candidates[1].candidate_id
        );
        let map = backend.candidates.lock().unwrap();
        assert_eq!(map.len(), 2);
        for (projection, expected) in projected.candidates.iter().zip([&first, &second]) {
            let resolved = map.get(&projection.candidate_id).unwrap();
            assert_eq!(
                resolved.window.as_ref().unwrap().hwnd,
                expected.window.as_ref().unwrap().hwnd
            );
            assert_eq!(
                projection.window_handle,
                expected.window.as_ref().map(|window| window.hwnd.clone())
            );
        }
        // Both selections still reserve the same injected process, so a second
        // window cannot bypass the existing cross-client target lease.
        assert_eq!(first.process_identity(), second.process_identity());
    }
    #[test]
    fn window_refresh_keeps_identity_when_only_title_or_geometry_changes() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let original = candidate(true);
        let before = backend
            .remember(vec![(original.clone(), "Old title".into())])
            .unwrap();
        let mut refreshed = original.clone();
        let window = refreshed.window.as_mut().unwrap();
        window.title = "New dialogue in title".into();
        window.rect.left = 500;
        window.rect.right = 1500;
        let after = backend
            .remember(vec![(refreshed.clone(), "New dialogue in title".into())])
            .unwrap();
        assert_eq!(
            before.candidates[0].candidate_id,
            after.candidates[0].candidate_id
        );
        let map = backend.candidates.lock().unwrap();
        let resolved = map.get(&before.candidates[0].candidate_id).unwrap();
        assert_eq!(resolved.window, refreshed.window);
        assert_eq!(resolved.process_identity(), original.process_identity());
    }
    #[test]
    fn complete_process_lifecycle_and_candidate_kind_cannot_reuse_a_previous_selection() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let original = candidate(true);
        let original_id = original.selection_identity().unwrap();
        let mut changed = Vec::new();
        let mut environment = original.clone();
        environment.environment_key.push_str("-another-bottle");
        changed.push(environment);
        let mut path = original.clone();
        path.target.full_image_path = Some("C:\\Other\\Example.exe".into());
        changed.push(path);
        let mut pid = original.clone();
        pid.target.pid += 1;
        changed.push(pid);
        let mut creation = original.clone();
        creation.target.creation_filetime = Some("18446744073709551614".into());
        changed.push(creation);
        let mut architecture = original.clone();
        architecture.target.architecture = Some("x86".into());
        changed.push(architecture);
        for mut different in changed {
            different.window.as_mut().unwrap().target = different.target.clone();
            assert_ne!(original.process_identity(), different.process_identity());
            let refreshed = backend
                .remember(vec![(different.clone(), "Same title".into())])
                .unwrap();
            assert_ne!(refreshed.candidates[0].candidate_id, original_id);
            let map = backend.candidates.lock().unwrap();
            assert!(!map.contains_key(&original_id));
            assert_eq!(
                map.get(&refreshed.candidates[0].candidate_id)
                    .unwrap()
                    .target,
                different.target
            );
        }
        let process_only = candidate(false);
        assert_eq!(original.process_identity(), process_only.process_identity());
        assert_ne!(
            original.selection_identity(),
            process_only.selection_identity()
        );
        let mut changed_class = original.clone();
        changed_class.window.as_mut().unwrap().class_name = "DifferentWindowClass".into();
        assert_eq!(
            original.process_identity(),
            changed_class.process_identity()
        );
        assert_ne!(
            original.selection_identity(),
            changed_class.selection_identity()
        );
        // Keep the pre-existing process-lock identity compatible across this fix.
        assert_eq!(
            original.process_identity(),
            Some(content_hash(concat!(
                "crossover:/test/bottle",
                "\0",
                "C:\\Games\\Example.exe",
                "\0",
                "812",
                "\0",
                "18446744073709551615",
                "\0",
                "x64"
            )))
        );
    }
}

#[cfg(test)]
mod session_event_projection_tests {
    use super::*;
    fn session() -> HookSessionInput {
        HookSessionInput {
            session_id: "projection-session".into(),
            game_id: None,
            environment_key: "controlled".into(),
            target_exe_path: "C:\\controlled.exe".into(),
            windows_pid: 100,
            process_created_u64: "1".into(),
            architecture: "x86".into(),
            broker_instance_id: "projection-broker".into(),
            resource: ResourceSnapshotInput {
                snapshot_id: "resource".into(),
                kind: "core".into(),
                version: "fixture".into(),
                manifest_sha256: content_hash("fixture"),
                contract_version: "1".into(),
                metadata_json: "{}".into(),
            },
            target_identity_sha256: content_hash("target"),
        }
    }
    #[test]
    fn association_attribution_is_published_only_after_lease_guarded_commit() {
        let root = std::env::temp_dir().join(format!("luna-attribution-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), root.clone());
        let initial = session();
        let file = File::create(root.join("owned.lock")).unwrap();
        file.try_lock().unwrap();
        *backend.lease.lock().unwrap() = Some((initial.target_identity_sha256.clone(), file));
        *backend.active.lock().unwrap() = Some(initial.clone());
        let mut reviewed = initial.clone();
        reviewed.game_id = Some("reviewed-game".into());
        let mut commits = 0;
        assert_eq!(
            backend
                .with_associated_session_commit(&reviewed, &mut || {
                    commits += 1;
                    Err(error("controlled_sql_failure", "controlled failed commit"))
                })
                .unwrap_err()
                .code,
            "controlled_sql_failure"
        );
        assert_eq!(commits, 1);
        assert!(
            backend
                .active
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .game_id
                .is_none()
        );
        backend
            .with_associated_session_commit(&reviewed, &mut || {
                commits += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            backend.active.lock().unwrap().as_ref().unwrap().game_id,
            reviewed.game_id
        );
        let mut conflicting = reviewed.clone();
        conflicting.game_id = Some("different-game".into());
        assert_eq!(
            backend
                .with_associated_session_commit(&conflicting, &mut || {
                    commits += 1;
                    Ok(())
                })
                .unwrap_err()
                .code,
            "session_game_conflict"
        );
        let mut stale = reviewed.clone();
        stale.process_created_u64 = "2".into();
        assert_eq!(
            backend
                .with_associated_session_commit(&stale, &mut || {
                    commits += 1;
                    Ok(())
                })
                .unwrap_err()
                .code,
            "session_identity_conflict"
        );
        assert_eq!(
            commits, 2,
            "neither conflicting nor stale ownership reaches SQL"
        );
        assert_eq!(
            backend.active.lock().unwrap().as_ref().unwrap().session_id,
            initial.session_id
        );
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn owned_process_eof_and_native_relay_deliver_shutdown_text_and_control_before_barrier() {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("luna-core-relay-drain-{}", uuid::Uuid::new_v4()));
        let resources = dir.join("resources");
        for relative in ["brokers/x64", "brokers/x86", "core/x86/bin"] {
            std::fs::create_dir_all(resources.join(relative)).unwrap();
        }
        for relative in [
            "brokers/x64/target_probe.exe",
            "brokers/x86/lunahook-host.exe",
        ] {
            std::fs::write(resources.join(relative), b"controlled resource").unwrap();
        }
        let launcher = dir.join("controlled-launcher.py");
        std::fs::write(&launcher,r#"#!/usr/bin/env python3
import json,sys,pathlib
if any(x.endswith('target_probe.exe') for x in sys.argv):
    with (pathlib.Path(__file__).parent/'probe-calls.jsonl').open('a') as f:
        f.write(json.dumps(sys.argv[1:]))
    if '--window-title-challenge' in sys.argv:
        print(json.dumps(dict(status='ready')),flush=True)
        requested=sys.stdin.readline().strip()=='restore'
        print(json.dumps(dict(status='restored',titleRestored=True,identityVerified=True,requested=requested)),flush=True)
        raise SystemExit(0)
    target=dict(pid=100,exe='controlled.exe',full_image_path='C:\\controlled.exe',creation_filetime='1',architecture='x86',errors=[])
    if '--verify-bound-window' in sys.argv:
        saved=pathlib.Path(__file__).parent/'controlled-windows.json'
        windows=json.loads(saved.read_text()) if saved.exists() else [dict(hwnd='100',title='controlled game',class_name='GameClass',target=target)]
        expected=sys.argv[sys.argv.index('--verify-bound-window')+1:]
        matched=next((w for w in windows if expected==[w['hwnd'],str(w['target']['pid']),w['target']['full_image_path'],w['target']['creation_filetime'],w['target']['architecture'],w['class_name']]),None)
        print(json.dumps(dict(status='verified' if matched else 'stale',windowTitle=matched['title'] if matched else None,errorCode=None if matched else 'window_identity_changed')),flush=True)
        raise SystemExit(0)
    if '--windows' in sys.argv:
        saved=pathlib.Path(__file__).parent/'controlled-windows.json'
        windows=json.loads(saved.read_text()) if saved.exists() else [dict(hwnd='100',title='controlled game',class_name='GameClass',rect=dict(left=0,top=0,right=100,bottom=100),target=target)]
        print(json.dumps(dict(windows=windows)),flush=True)
        raise SystemExit(0)
    print(json.dumps(dict(processes=[target])),flush=True)
    raise SystemExit(0)
def event(event_kind,**fields):
    print(json.dumps(dict(v=2,event=event_kind,**fields),ensure_ascii=False),flush=True)
event('ready',host_arch='x86',abi_version=1,upstream_commit='e9242ebac32a09e911f306b7c90714891c217302',abi_profile='lunahooks-core-abi1-cdecl-seven')
stream=dict(pid=100,address='1',context='0',context2='0')
for line in sys.stdin:
    request=json.loads(line)
    if request['method']=='attach':
        event('process_connected',pid=100)
    elif request['method']=='shutdown':
        for i in range(100):
            event('text',stream=stream,hook_code='fixture',hook_name='fixture',text='終了時の原文'+str(i))
            event('log',kind=1,message='controlled engine diagnosis Bearer private native log '+str(i))
        event('process_disconnected',pid=100)
        event('response',id=request['id'],result=dict(status='shutting_down',deadline_ms=1000))
        break
"#).unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
        let backend = Arc::new(CoreHookBackend::new(
            resources,
            dir.join("logs"),
            dir.join("locks"),
        ));
        let updates = backend.subscribe().unwrap();
        let target = Target {
            pid: 100,
            exe: "controlled.exe".into(),
            full_image_path: Some("C:\\controlled.exe".into()),
            creation_filetime: Some("1".into()),
            architecture: Some("x86".into()),
            errors: vec![],
        };
        let mut active = backend.active.lock().unwrap();
        backend
            .client
            .connect(
                LaunchSettings {
                    wine_path: launcher.to_string_lossy().into_owned(),
                    bottle: "controlled".into(),
                },
                target,
            )
            .unwrap();
        let mut input = session();
        input.session_id = backend.client.snapshot().session_id.unwrap();
        *active = Some(input);
        drop(active);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while backend.client.snapshot().status != "connected" {
            assert!(
                std::time::Instant::now() < deadline,
                "controlled host did not connect: {:?}",
                backend.client.snapshot()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let candidate = Candidate {
            target: backend.client.snapshot().target.unwrap(),
            window: Some(WindowTarget {
                hwnd: "100".into(),
                title: "controlled game".into(),
                class_name: "GameClass".into(),
                rect: hook_client::WindowRect {
                    left: 0,
                    top: 0,
                    right: 100,
                    bottom: 100,
                },
                target: backend.client.snapshot().target.unwrap(),
            }),
            settings: backend.client.snapshot().settings.unwrap(),
            environment_key: "controlled".into(),
        };
        let binding_receipt = backend
            .client
            .bind_window(
                candidate.settings.clone(),
                candidate.window.clone().unwrap(),
            )
            .unwrap();
        let bound_hash = candidate.process_identity().unwrap();
        let bound_window_hash = candidate.selection_identity().unwrap();
        backend
            .active
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .target_identity_sha256 = bound_hash.clone();
        *backend.preview_binding.lock().unwrap() =
            Some((candidate.selection_identity().unwrap(), candidate));
        *backend.association_has_window.lock().unwrap() = true;
        let actual_bound_at = binding_receipt.binding.unwrap().bound_at_ms;
        *backend.preview_bound_at.lock().unwrap() = Some((
            backend
                .active
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .session_id
                .clone(),
            bound_window_hash.clone(),
            actual_bound_at,
        ));
        {
            let _held_hook_lease = backend.lease.lock().unwrap();
            let cached = backend.cached_preview_binding().unwrap();
            assert_eq!(cached.state, GameWindowBindingState::Bound);
            assert_eq!(cached.verified_at_ms, Some(actual_bound_at.to_string()));
            assert_eq!(
                cached.target_identity_sha256.as_deref(),
                Some(bound_window_hash.as_str())
            );
        }
        // This synchronous capture must not invoke the identity probe. The
        // controlled probe only implements process discovery, not bound actions.
        let frozen = backend.game_window_activation_request().unwrap();
        assert_eq!(frozen.expected_target_identity_sha256, bound_window_hash);
        assert_ne!(frozen.expected_target_identity_sha256, bound_hash);
        let cover_guard = backend.cover_session_guard().unwrap();
        assert_eq!(cover_guard.process_identity_sha256, bound_hash);
        assert_eq!(cover_guard.binding_identity_sha256, bound_window_hash);
        assert_ne!(
            cover_guard.binding_identity_sha256,
            cover_guard.process_identity_sha256
        );
        let mut wrong_cover = frozen.clone();
        wrong_cover.expected_target_identity_sha256 = content_hash("wrong-cover-window");
        assert_eq!(
            backend
                .with_verified_cover_binding_commit(wrong_cover, &mut || panic!(
                    "wrong bound window saved a cover"
                ))
                .unwrap_err()
                .code,
            "game_cover_stale"
        );
        let original_candidate = backend
            .preview_binding
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .1
            .clone();
        let (proof_entered_tx, proof_entered_rx) = mpsc::sync_channel(1);
        let (proof_release_tx, proof_release_rx) = mpsc::sync_channel(1);
        let proof_backend = backend.clone();
        let proof = std::thread::spawn(move || {
            proof_backend.client.with_window_title_challenge(
                &original_candidate.settings,
                original_candidate.window.as_ref().unwrap(),
                "LunaWindowProof-Backend-Busy-123456789",
                || {
                    proof_entered_tx.send(()).unwrap();
                    proof_release_rx.recv().unwrap();
                    Ok(())
                },
            )
        });
        proof_entered_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        let busy = backend.game_window_binding().unwrap();
        assert_eq!(busy.state, GameWindowBindingState::Unavailable);
        assert_eq!(busy.error_code.as_deref(), Some("window_proof_busy"));
        assert_eq!(
            busy.target_identity_sha256.as_deref(),
            Some(frozen.expected_target_identity_sha256.as_str())
        );
        assert_eq!(
            busy.session_id.as_deref(),
            Some(frozen.expected_session_id.as_str())
        );
        assert!(busy.verified_at_ms.is_none());
        assert!(busy.window_title.is_none());
        assert_eq!(
            backend
                .show_associated_game(frozen.clone())
                .unwrap_err()
                .code,
            "window_proof_busy"
        );
        let calls_before = std::fs::read_to_string(dir.join("probe-calls.jsonl")).unwrap();
        assert!(!calls_before.contains("--verify-bound-window"));
        assert!(!calls_before.contains("--activate-window"));
        proof_release_tx.send(()).unwrap();
        proof.join().unwrap().unwrap();
        assert_eq!(
            backend
                .with_verified_cover_binding_commit(frozen.clone(), &mut || Ok(
                    CoverInstallOutcome::Installed
                ))
                .unwrap(),
            CoverInstallOutcome::Installed
        );
        let old = backend.active.lock().unwrap().clone().unwrap();
        backend.active.lock().unwrap().as_mut().unwrap().session_id = "new-binding-session".into();
        assert_eq!(
            backend
                .show_associated_game(frozen.clone())
                .unwrap_err()
                .code,
            "game_window_activation_stale"
        );
        *backend.active.lock().unwrap() = Some(old.clone());
        let original_binding = backend.preview_binding.lock().unwrap().clone().unwrap();
        for class_only in [false, true] {
            let mut changed = original_binding.clone();
            let window = changed.1.window.as_mut().unwrap();
            if class_only {
                window.class_name = "NewGameClass".into();
            } else {
                window.hwnd = "101".into();
            }
            changed.0 = changed.1.selection_identity().unwrap();
            *backend.preview_binding.lock().unwrap() = Some(changed);
            // Same process and same Client session: a different HWND/class is
            // still a different target. The controlled probe cannot handle
            // activation; this exact error proves rejection before any probe.
            assert_eq!(
                backend
                    .show_associated_game(frozen.clone())
                    .unwrap_err()
                    .code,
                "game_window_activation_stale"
            );
            assert_eq!(
                backend
                    .unlink_window(frozen.clone(), &mut || Ok(None), &mut || panic!(
                        "same-session stale HWND/class cannot commit unbind"
                    ))
                    .unwrap_err()
                    .code,
                "game_window_unbind_stale"
            );
        }
        let original_candidate_for_unbind = original_binding.1.clone();
        *backend.preview_binding.lock().unwrap() = Some(original_binding);
        *backend.active.lock().unwrap() = Some(old);
        let calls_after = std::fs::read_to_string(dir.join("probe-calls.jsonl")).unwrap();
        assert_eq!(
            calls_after, calls_before,
            "stale requests must not execute any new target probe"
        );
        // Real SQLite commit failure must compensate both the actual Client
        // association and Core preview reference. The owned host keeps running.
        use app_core::ports::ProductRepository;
        let database_path = dir.join("unbind-profile.sqlite3");
        let store =
            crate::storage::SqliteStore::open(&database_path, "unbind-profile", "unbind-server", 1)
                .unwrap();
        let input = backend.active.lock().unwrap().clone().unwrap();
        store
            .begin_capture_operation("core-operation".into(), content_hash("core-association"), 2)
            .unwrap();
        store.open_hook_session(input.clone(), 3).unwrap();
        store
            .finish_capture_operation("core-operation".into(), Some(input.session_id.clone()), 4)
            .unwrap();
        let source = HookSourceInput {
            source_id: "core-unbind-source".into(),
            host_instance_id: input.session_id.clone(),
            hook_address_u64: "1".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "fixture".into(),
            hook_name: "fixture".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash("core-unbind-source"),
            identity_extensions_json: "{}".into(),
        };
        let raw = |seq: u64, text: &str| HookTextInput {
            session_id: input.session_id.clone(),
            event_id: format!("core-before-after-{seq}"),
            sequence_u64: seq.to_string(),
            source: source.clone(),
            text: text.into(),
            source_language: "ja".into(),
        };
        store.ingest_hook_text(raw(1, "候補"), 5).unwrap();
        store
            .dispatch(
                CommandEnvelope {
                    operation_id: "core-select".into(),
                    expected_revision: None,
                    command: Command::SelectSource {
                        source_id: source.source_id.clone(),
                    },
                },
                6,
            )
            .unwrap();
        store.ingest_hook_text(raw(2, "解除前の原文。"), 7).unwrap();
        let envelope = CommandEnvelope {
            operation_id: "core-compensated-unbind".into(),
            expected_revision: Some(0),
            command: Command::UnbindWindow(frozen.clone()),
        };
        assert_eq!(
            store.dispatch(envelope.clone(), 8).unwrap_err().code,
            "native_effect_required"
        );
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        connection.execute_batch("CREATE TRIGGER controlled_native_unbind_failure BEFORE INSERT ON operation_receipts WHEN NEW.operation_id='core-compensated-unbind' BEGIN SELECT RAISE(ABORT,'actual controlled SQL failure'); END;").unwrap();
        let settings = backend.client.snapshot().settings.unwrap();
        let client_binding_before =
            serde_json::to_value(backend.client.window_binding_status(&settings)).unwrap();
        assert!(client_binding_before["binding"].is_object());
        let hook_before = serde_json::to_value(backend.client.snapshot()).unwrap();
        let failure = backend
            .unlink_window(
                frozen.clone(),
                &mut || store.preflight_window_unbind(envelope.clone()),
                &mut || store.commit_window_unbind(envelope.clone(), 9),
            )
            .unwrap_err();
        assert_eq!(failure.code, "storage_failure");
        assert_eq!(
            serde_json::to_value(backend.client.window_binding_status(&settings)).unwrap(),
            client_binding_before
        );
        assert_eq!(backend.game_window_activation_request().unwrap(), frozen);
        assert_eq!(
            serde_json::to_value(backend.client.snapshot()).unwrap(),
            hook_before
        );
        let successes = || -> (i64, i64) {
            (connection.query_row("SELECT count(*) FROM operation_receipts WHERE operation_id='core-compensated-unbind'", [], |r| r.get(0)).unwrap(),
             connection.query_row("SELECT count(*) FROM hook_events WHERE event_id='command-hook:core-compensated-unbind'", [], |r| r.get(0)).unwrap())
        };
        assert_eq!(successes(), (0, 0));
        connection
            .execute_batch("DROP TRIGGER controlled_native_unbind_failure")
            .unwrap();
        let history_before = store.snapshot().unwrap().history;
        let receipt = backend
            .unlink_window(
                frozen.clone(),
                &mut || store.preflight_window_unbind(envelope.clone()),
                &mut || store.commit_window_unbind(envelope.clone(), 10),
            )
            .unwrap();
        assert!(!receipt.replayed);
        assert_eq!(
            receipt.result_id.as_deref(),
            Some(frozen.expected_target_identity_sha256.as_str())
        );
        assert_eq!(successes(), (1, 1));
        assert!(
            backend
                .client
                .window_binding_status(&settings)
                .binding
                .is_none()
        );
        assert_eq!(
            backend.game_window_binding().unwrap().state,
            GameWindowBindingState::Unbound
        );
        assert!(backend.client.has_active_session());
        assert_eq!(
            serde_json::to_value(backend.client.snapshot()).unwrap(),
            hook_before
        );
        assert_eq!(store.snapshot().unwrap().history, history_before);
        store
            .ingest_hook_text(raw(3, "解除後の原文。"), 11)
            .unwrap();
        let records = store
            .session_records(
                SessionRecordQuery {
                    session_id: input.session_id.clone(),
                    dataset: SessionDataset::Extraction,
                    order: SessionRecordOrder::OldestFirst,
                    filter: SessionRecordFilter::default(),
                    cursor: None,
                    page_size: 200,
                },
                12,
            )
            .unwrap();
        assert_eq!(records.records.len(), 3);
        assert!(
            store
                .snapshot()
                .unwrap()
                .session
                .unwrap()
                .sources
                .iter()
                .any(|s| s.selected && s.source_id == source.source_id)
        );
        // Reassociate B in the same process/session with a distinct real Client
        // window binding. A replay must not clear B or rerun the commit callback.
        assert!(backend.preview_binding.lock().unwrap().is_none());
        let mut next = original_candidate_for_unbind;
        next.window.as_mut().unwrap().hwnd = "101".into();
        next.window.as_mut().unwrap().class_name = "NextGameClass".into();
        std::fs::write(
            dir.join("controlled-windows.json"),
            serde_json::to_vec(&vec![next.window.clone().unwrap()]).unwrap(),
        )
        .unwrap();
        backend
            .client
            .bind_window(settings.clone(), next.window.clone().unwrap())
            .unwrap();
        *backend.preview_binding.lock().unwrap() = Some((next.selection_identity().unwrap(), next));
        let new_request = backend.game_window_activation_request().unwrap();
        assert_eq!(
            backend
                .with_verified_cover_binding_commit(frozen.clone(), &mut || panic!(
                    "A frame committed after binding changed to B"
                ))
                .unwrap_err()
                .code,
            "game_cover_stale"
        );
        assert_eq!(new_request.expected_session_id, frozen.expected_session_id);
        assert_ne!(
            new_request.expected_target_identity_sha256,
            frozen.expected_target_identity_sha256
        );
        let replay = backend
            .unlink_window(
                frozen.clone(),
                &mut || store.preflight_window_unbind(envelope.clone()),
                &mut || panic!("replay cannot execute another unlink commit"),
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(
            backend.game_window_activation_request().unwrap(),
            new_request
        );
        assert_eq!(
            backend
                .client
                .window_binding_status(&settings)
                .binding
                .unwrap()
                .window
                .hwnd,
            "101"
        );
        let stale = CommandEnvelope {
            operation_id: "core-stale-unbind".into(),
            ..envelope.clone()
        };
        assert_eq!(
            backend
                .unlink_window(
                    frozen.clone(),
                    &mut || store.preflight_window_unbind(stale.clone()),
                    &mut || panic!("stale old A must not commit against B")
                )
                .unwrap_err()
                .code,
            "game_window_unbind_stale"
        );
        assert_eq!(
            backend.game_window_activation_request().unwrap(),
            new_request
        );
        assert_eq!(successes(), (1, 1));
        // Unknown and non-Hook sessions cannot borrow the current window's
        // authority. Even an ended Hook session retains its independent window
        // association and can be unlinked without changing capture end state.
        let manual = store
            .dispatch(
                CommandEnvelope {
                    operation_id: "core-manual-session".into(),
                    expected_revision: None,
                    command: Command::ManualInput {
                        text: "手動の原文".into(),
                        source_language: "ja".into(),
                    },
                },
                13,
            )
            .unwrap();
        let manual_session = store.unit(manual.result_id.unwrap(), 1).unwrap().session_id;
        for (index, sid) in ["unknown-hook-session".to_string(), manual_session]
            .into_iter()
            .enumerate()
        {
            let invalid = CommandEnvelope {
                operation_id: format!("invalid-unbind-session-{index}"),
                expected_revision: None,
                command: Command::UnbindWindow(GameWindowActivationRequest {
                    expected_session_id: sid,
                    ..new_request.clone()
                }),
            };
            let Command::UnbindWindow(invalid_request) = invalid.command.clone() else {
                unreachable!()
            };
            assert_eq!(
                backend
                    .unlink_window(
                        invalid_request,
                        &mut || store.preflight_window_unbind(invalid.clone()),
                        &mut || panic!("unknown/non-Hook session cannot commit unbind")
                    )
                    .unwrap_err()
                    .code,
                "game_window_unbind_stale"
            );
            assert_eq!(
                backend.game_window_activation_request().unwrap(),
                new_request
            );
            assert_eq!(
                backend
                    .client
                    .window_binding_status(&settings)
                    .binding
                    .unwrap()
                    .window
                    .hwnd,
                "101"
            );
        }
        store
            .dispatch(
                CommandEnvelope {
                    operation_id: "core-durable-hook-stop".into(),
                    expected_revision: None,
                    command: Command::Disconnect,
                },
                14,
            )
            .unwrap();
        for (index, state) in ["stopped", "failed"].into_iter().enumerate() {
            if index > 0 {
                // Reapply this controlled verified Client binding only; no
                // second host/session is started by a local window association.
                let window: WindowTarget = serde_json::from_slice::<Vec<WindowTarget>>(
                    &std::fs::read(dir.join("controlled-windows.json")).unwrap(),
                )
                .unwrap()
                .remove(0);
                backend
                    .client
                    .bind_window(settings.clone(), window.clone())
                    .unwrap();
                let candidate = Candidate {
                    target: window.target.clone(),
                    window: Some(window),
                    settings: settings.clone(),
                    environment_key: "controlled".into(),
                };
                *backend.preview_binding.lock().unwrap() =
                    Some((candidate.selection_identity().unwrap(), candidate));
                connection
                    .execute(
                        "UPDATE sessions SET state='failed' WHERE session_id=?1",
                        [&input.session_id],
                    )
                    .unwrap();
            }
            let expected = backend.game_window_activation_request().unwrap();
            let ended_before: (String, i64) = connection
                .query_row(
                    "SELECT state,ended_at_ms FROM sessions WHERE session_id=?1",
                    [&input.session_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(ended_before.0, state);
            let request = CommandEnvelope {
                operation_id: format!("ended-hook-unbind-{state}"),
                expected_revision: Some(0),
                command: Command::UnbindWindow(expected.clone()),
            };
            let receipt = backend
                .unlink_window(
                    expected.clone(),
                    &mut || store.preflight_window_unbind(request.clone()),
                    &mut || store.commit_window_unbind(request.clone(), 15 + index as i64),
                )
                .unwrap();
            assert!(!receipt.replayed);
            assert_eq!(
                receipt.result_id.as_deref(),
                Some(expected.expected_target_identity_sha256.as_str())
            );
            assert_eq!(
                backend.game_window_binding().unwrap().state,
                GameWindowBindingState::Unbound
            );
            assert!(backend.client.has_active_session());
            let ended_after: (String, i64) = connection
                .query_row(
                    "SELECT state,ended_at_ms FROM sessions WHERE session_id=?1",
                    [&input.session_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(ended_after, ended_before);
            let journal_session: String = connection
                .query_row(
                    "SELECT session_id FROM hook_events WHERE event_id=?1",
                    [format!("command-hook:{}", request.operation_id)],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(journal_session, expected.expected_session_id);
        }
        drop(store);
        drop(connection);
        let (ack, acknowledged) = mpsc::sync_channel(1);
        let stopping = backend.clone();
        let worker = std::thread::spawn(move || stopping.stop_and_drain(ack));
        let mut texts = Vec::new();
        let mut logs = 0;
        loop {
            match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
                HookUpdate::Text(text) => texts.push(text.text),
                HookUpdate::Control { event, .. } => {
                    if event.detail.code == "core_log" {
                        logs += 1;
                        assert!(event.detail.message.contains("controlled engine diagnosis"));
                        assert!(!event.detail.message.contains("private native log"));
                    }
                    assert_ne!(event.integrity, "lost");
                }
                HookUpdate::Health { .. } => {
                    panic!("controlled stream unexpectedly lost integrity")
                }
                HookUpdate::SessionDrainBarrier { .. } => {
                    panic!("explicit application stop unexpectedly used a natural-exit barrier")
                }
                HookUpdate::DrainBarrier {
                    acknowledged: reply,
                } => {
                    reply.send(Ok(())).unwrap();
                    break;
                }
            }
        }
        worker.join().unwrap().unwrap();
        acknowledged
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        assert_eq!(
            texts,
            (0..100)
                .map(|i| format!("終了時の原文{i}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(logs, 100);
        assert!(backend.active.lock().unwrap().is_none());
        backend
            .client
            .wait_event_producers_stopped(Duration::from_millis(50))
            .unwrap();
        drop(updates);
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn binding_projection_does_not_infer_a_window_from_hook_or_game_presence_and_stale_unlink_is_rejected()
     {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        {
            let _busy = backend.lease.lock().unwrap();
            assert_eq!(
                backend.game_window_activation_request().unwrap_err().code,
                "game_window_activation_busy"
            );
        }
        let request = || {
            let current = backend.active.lock().unwrap().clone().unwrap();
            GameWindowActivationRequest {
                expected_session_id: current.session_id,
                expected_target_identity_sha256: current.target_identity_sha256,
            }
        };
        assert_eq!(
            backend.game_window_binding().unwrap().state,
            GameWindowBindingState::Unbound
        );
        *backend.active.lock().unwrap() = Some(session());
        assert_eq!(
            backend.game_window_binding().unwrap().state,
            GameWindowBindingState::NoWindow
        );
        assert_eq!(
            backend.show_associated_game(request()).unwrap_err().code,
            "game_window_no_window"
        );
        *backend.association_has_window.lock().unwrap() = true;
        assert_eq!(
            backend.game_window_binding().unwrap().state,
            GameWindowBindingState::Unbound
        );
        let target = Target {
            pid: 100,
            exe: "controlled.exe".into(),
            full_image_path: Some("C:\\controlled.exe".into()),
            creation_filetime: Some("1".into()),
            architecture: Some("x86".into()),
            errors: vec![],
        };
        let candidate = Candidate {
            target: target.clone(),
            window: Some(WindowTarget {
                hwnd: "1".into(),
                title: "同じ題名".into(),
                class_name: "GameClass".into(),
                rect: hook_client::WindowRect {
                    left: 0,
                    top: 0,
                    right: 100,
                    bottom: 100,
                },
                target,
            }),
            settings: LaunchSettings {
                wine_path: "/not-used".into(),
                bottle: "controlled".into(),
            },
            environment_key: "controlled".into(),
        };
        backend
            .active
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .target_identity_sha256 = candidate.process_identity().unwrap();
        *backend.preview_binding.lock().unwrap() =
            Some((candidate.selection_identity().unwrap(), candidate));
        // The adapter has no matching live Client session. A remembered window
        // alone must not turn this stale association into a bound projection.
        assert_eq!(
            backend.game_window_binding().unwrap().state,
            GameWindowBindingState::Stale
        );
        assert_eq!(
            backend.show_associated_game(request()).unwrap_err().code,
            "game_window_stale"
        );
        let mut commit_called = false;
        assert_eq!(
            backend
                .unlink_window(request(), &mut || Ok(None), &mut || {
                    commit_called = true;
                    unreachable!("stale local client cannot commit an unlink")
                })
                .unwrap_err()
                .code,
            "game_window_stale"
        );
        assert!(!commit_called);
        assert!(backend.active.lock().unwrap().is_some());
        assert!(backend.preview_binding.lock().unwrap().is_some());
        assert_eq!(
            backend.game_window_binding().unwrap().state,
            GameWindowBindingState::Stale
        );
        assert_eq!(
            backend.show_associated_game(request()).unwrap_err().code,
            "game_window_stale"
        );
    }
    fn project(value: serde_json::Value, n: u64) -> HookUpdate {
        project_event(
            &session(),
            hook_client::ClientEvent {
                session_id: Some("projection-session".into()),
                sequence: n,
                received_at_ms: 100,
                status: "running".into(),
                target: None,
                integrity_lost: false,
                payload: Some(serde_json::from_value(value).unwrap()),
            },
        )
    }
    #[test]
    fn every_native_control_variant_has_a_typed_safe_journal_projection() {
        // The 2026-10-21 diagnostic requirement replaces blanket message discard.
        // Keep the original credential/path fixture: its sensitive suffix must
        // remain absent while useful, sanitised native diagnostics now survive.
        let stream = serde_json::json!({"pid":100,"address":"18446744073709551615","context":"0","context2":"1"});
        let cases = vec![
            (
                serde_json::json!({"event":"ready","host_arch":"x86","abi_version":1,"upstream_commit":"fixture","abi_profile":"fixture"}),
                "host_ready",
            ),
            (
                serde_json::json!({"event":"response","id":"request","result":{"status":"pong"}}),
                "host_response",
            ),
            (
                serde_json::json!({"event":"error","id":null,"code":"injection_failed","message":"Bearer TOP_SECRET 台词全文 C:\\private"}),
                "core_error",
            ),
            (
                serde_json::json!({"event":"process_connected","pid":100}),
                "process_connected",
            ),
            (
                serde_json::json!({"event":"process_disconnected","pid":100}),
                "process_disconnected",
            ),
            (
                serde_json::json!({"event":"stream_created","stream":stream,"hook_code":"fixture","hook_name":"fixture"}),
                "stream_created",
            ),
            (
                serde_json::json!({"event":"stream_destroyed","stream":stream,"hook_code":"fixture","hook_name":"fixture"}),
                "stream_destroyed",
            ),
            (
                serde_json::json!({"event":"log","kind":1,"message":"Bearer TOP_SECRET 台词全文 C:\\private"}),
                "core_log",
            ),
            (
                serde_json::json!({"event":"hook_inserted","pid":100,"address":"18446744073709551615","hook_code":"fixture"}),
                "hook_inserted",
            ),
            (
                serde_json::json!({"event":"injection_finished","pid":100,"architecture":"x86","exit_code":0,"accepted_by_helper":true}),
                "helper_accepted",
            ),
            (
                serde_json::json!({"event":"overflow","source":"callback","dropped_since_last":"18446744073709551615","dropped_total":"18446744073709551615","integrity_lost":true}),
                "queue_overflow",
            ),
        ];
        for (index, (value, code)) in cases.into_iter().enumerate() {
            match project(value, index as u64 + 1) {
                HookUpdate::Control { event, .. } => {
                    assert_eq!(event.detail.code, code);
                    assert!(!event.detail.message.contains("TOP_SECRET"));
                    assert!(!event.detail.message.contains("台词全文"));
                    if code == "queue_overflow" {
                        assert_eq!(event.lost_count.as_deref(), Some("18446744073709551615"));
                        assert_eq!(event.integrity, "lost");
                    }
                    if code.starts_with("stream_") {
                        assert_eq!(
                            event.source.as_ref().unwrap().hook_address_u64,
                            "18446744073709551615"
                        );
                    }
                }
                _ => panic!("control callback disappeared"),
            }
        }
    }
    fn projected_detail(value: serde_json::Value) -> HookStageInput {
        match project(value, u64::MAX) {
            HookUpdate::Control { event, .. } => {
                assert_eq!(event.wire_sequence.as_deref(), Some("18446744073709551615"));
                event.detail
            }
            _ => panic!("expected a native diagnostic control event"),
        }
    }
    #[test]
    fn native_diagnostic_facts_survive_projection_without_integer_or_unicode_loss() {
        let ready = projected_detail(
            serde_json::json!({"event":"ready","host_arch":"x64","abi_version":1,"upstream_commit":"e9242ebac32a09e911f306b7c90714891c217302","abi_profile":"lunahooks-core-abi1-cdecl-seven"}),
        );
        for fact in [
            "architecture=x64",
            "ABI=1",
            "e9242ebac32a09e911f306b7c90714891c217302",
            "lunahooks-core-abi1-cdecl-seven",
        ] {
            assert!(
                ready.message.contains(fact),
                "missing {fact}: {}",
                ready.message
            );
        }
        let log = projected_detail(
            serde_json::json!({"event":"log","kind":0,"message":"匹配到Leaf引擎 · 注入钩子 TextOutA 7B680BB0 · 𠮷"}),
        );
        assert_eq!(
            log.message,
            "Core日志 · kind=0 · 匹配到Leaf引擎 · 注入钩子 TextOutA 7B680BB0 · 𠮷"
        );
        for (kind, level) in [
            (0, "info"),
            (1, "warning"),
            (2, "warning"),
            (-1, "info"),
            (i32::MAX, "info"),
        ] {
            let detail = projected_detail(
                serde_json::json!({"event":"log","kind":kind,"message":"エンジン診断"}),
            );
            assert_eq!(detail.level, level);
            assert!(detail.message.contains(&format!("kind={kind}")));
        }
        let installed = projected_detail(
            serde_json::json!({"event":"hook_inserted","pid":u32::MAX,"address":"18446744073709551615","hook_code":"HSN-4@TextOutW"}),
        );
        for fact in [
            "Windows PID=4294967295",
            "address_u64=18446744073709551615",
            "address_hex=0xFFFFFFFFFFFFFFFF",
            "hook_code=HSN-4@TextOutW",
        ] {
            assert!(installed.message.contains(fact), "{}", installed.message);
        }
        let error = projected_detail(
            serde_json::json!({"event":"error","id":"attach-42","code":"injection_failed","message":"目标拒绝注入，Win32 error 5"}),
        );
        assert_eq!(error.level, "error");
        for fact in [
            "code=injection_failed",
            "request=attach-42",
            "Win32 error 5",
        ] {
            assert!(error.message.contains(fact));
        }
        for (accepted, exit) in [(true, Some(0)), (false, Some(-2147483648)), (false, None)] {
            let detail = projected_detail(
                serde_json::json!({"event":"injection_finished","pid":100,"architecture":"x86","exit_code":exit,"accepted_by_helper":accepted}),
            );
            assert!(detail.message.contains("Windows PID=100"));
            assert!(detail.message.contains("architecture=x86"));
            assert!(
                detail
                    .message
                    .contains(&format!("accepted_by_helper={accepted}"))
            );
            assert!(detail.message.contains(&format!(
                "exit_code={}",
                exit.map_or_else(|| "未提供".into(), |n| n.to_string())
            )));
            assert_eq!(detail.level, if accepted { "info" } else { "error" });
        }
    }
    #[test]
    fn native_diagnostics_are_bounded_control_free_and_redact_sensitive_suffixes() {
        for payload in [
            "Bearer TOP_SECRET 后续私密原文",
            "bEaReR TOP_SECRET",
            "Bea\0rer TOP_SECRET",
            "Bearer\u{2028}TOP_SECRET",
            "Bea\u{061c}rer TOP_SECRET",
            "Bearer\u{3000}TOP_SECRET",
            "Authorization: Basic TOP_SECRET",
            "api_key = \"TOP_SECRET contains spaces\"",
            "{\"APIKey\":\"TOP_SECRET\"}",
            "?access_token=TOP_SECRET",
            "TOKEN: TOP_SECRET",
            "password=TOP_SECRET",
            "x-api-key: TOP_SECRET",
            "refresh_token = TOP_SECRET",
            "secret: TOP_SECRET",
            "C:\\private\\TOP_SECRET.exe",
            "/Users/private/TOP_SECRET",
            "\\\\host\\TOP_SECRET",
            "https://name:TOP_SECRET@example.org/v1",
        ] {
            let detail = projected_detail(
                serde_json::json!({"event":"log","kind":1,"message":format!("匹配到引擎 · {payload}")}),
            );
            assert!(detail.message.contains("匹配到引擎"));
            assert!(
                detail.message.contains("敏感信息已隐藏"),
                "{}",
                detail.message
            );
            assert!(!detail.message.contains("TOP_SECRET"), "{}", detail.message);
            assert!(!detail.message.contains("后续私密原文"));
            assert!(!detail.message.chars().any(char::is_control));
        }
        let clean = diagnostic_text("開始\0\u{1b}\u{202e}𠮷\n続行\t完了", 256);
        assert_eq!(clean, "開始𠮷 続行 完了");
        for message in [
            "𠮷".repeat(5000),
            format!("{}BearER TOP_SECRET", "\0".repeat(9000)),
        ] {
            let detail =
                projected_detail(serde_json::json!({"event":"log","kind":0,"message":message}));
            assert!(detail.message.len() <= 4096);
            assert!(detail.message.contains("内容已截断"));
            assert!(!detail.message.contains("TOP_SECRET"));
        }
        let installed = projected_detail(
            serde_json::json!({"event":"hook_inserted","pid":100,"address":"9007199254740993","hook_code":format!("{} token=TOP_SECRET", "𠮷".repeat(2000))}),
        );
        assert!(installed.message.len() <= 4096);
        assert!(installed.message.contains("9007199254740993"));
        assert!(!installed.message.contains("TOP_SECRET"));
    }
    #[test]
    fn application_and_subscription_losses_preserve_known_full_width_counts() {
        for (count, code) in [
            (1, "application_queue_overflow"),
            (u64::MAX, "subscription_overflow"),
        ] {
            match queue_loss(Some("projection-session".into()), count, code) {
                HookUpdate::Control { event, diagnostic } => {
                    assert_eq!(event.lost_count, Some(count.to_string()));
                    assert_eq!(event.detail.code, code);
                    assert_eq!(event.integrity, "lost");
                    assert_eq!(diagnostic.unwrap().code, "integrity_lost");
                }
                _ => panic!("known loss count was discarded"),
            }
        }
    }
    #[test]
    fn text_projection_preserves_exact_japanese_and_full_width_wire_identity() {
        let value = serde_json::json!({"event":"text","stream":{"pid":100,"address":"18446744073709551615","context":"18446744073709551614","context2":"0"},"hook_code":"fixture","hook_name":"fixture","text":"桜が舞う。😀\n次の行。"});
        match project(value, u64::MAX) {
            HookUpdate::Text(text) => {
                assert_eq!(text.text, "桜が舞う。😀\n次の行。");
                assert_eq!(text.sequence_u64, u64::MAX.to_string());
                assert_eq!(text.source.context_u64, "18446744073709551614");
            }
            _ => panic!("text callback changed datasets"),
        }
    }
    #[test]
    fn forged_preview_reference_never_runs_the_capture_observer() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let called = std::sync::atomic::AtomicBool::new(false);
        assert_eq!(
            backend
                .with_preview_window(
                    "forged",
                    "LunaWindowProof-controlled000000000000000",
                    || {
                        called.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    }
                )
                .unwrap_err()
                .code,
            "stale_target"
        );
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(
            backend
                .preview_candidate_id_for_active_binding()
                .unwrap_err()
                .code,
            "preview_target_missing"
        );
    }
}

#[cfg(all(test, unix))]
mod preview_pairing_file_tests {
    use super::*;
    #[test]
    fn real_hardlink_aliases_share_vnode_but_replaced_or_modified_files_do_not_keep_version() {
        let root = std::env::temp_dir().join(format!(
            "luna-preview-vnode-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("game.exe");
        let alias = root.join("alias.exe");
        std::fs::write(&path, b"first executable content").unwrap();
        std::fs::hard_link(&path, &alias).unwrap();
        let held = File::open(&path).unwrap();
        let alias_file = File::open(&alias).unwrap();
        assert_eq!(
            preview_file_identity(&held).unwrap(),
            preview_file_identity(&alias_file).unwrap()
        );
        let original = preview_file_stamp(&held).unwrap();
        std::fs::write(&path, b"changed executable content and length").unwrap();
        assert_ne!(preview_file_stamp(&held).unwrap(), original);
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"replacement inode").unwrap();
        assert_ne!(
            preview_file_identity(&held).unwrap(),
            preview_file_identity(&File::open(&path).unwrap()).unwrap()
        );
        drop(held);
        drop(alias_file);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn process_alias_set_requires_exact_single_pid_and_creation_identity() {
        let expected = Target {
            pid: 45,
            exe: "game.exe".into(),
            full_image_path: Some("C:\\game.exe".into()),
            creation_filetime: Some("100".into()),
            architecture: Some("x64".into()),
            errors: vec![],
        };
        assert!(!single_preview_instance(&[], &expected));
        assert!(single_preview_instance(
            std::slice::from_ref(&expected),
            &expected
        ));
        assert!(!single_preview_instance(
            &[expected.clone(), expected.clone()],
            &expected
        ));
        let mut reused = expected.clone();
        reused.creation_filetime = Some("101".into());
        assert!(!single_preview_instance(&[reused], &expected));
        let mut other = expected.clone();
        other.pid += 1;
        assert!(!single_preview_instance(&[other], &expected));
    }
    #[test]
    fn temporary_probe_failures_do_not_claim_closed_or_mismatched_identity() {
        assert_eq!(
            preview_verification_error("window_proof_busy: fixture").code,
            "preview_capture_busy"
        );
        assert_eq!(
            preview_verification_error("probe_timeout private fixture").code,
            "preview_verification_unavailable"
        );
        assert!(
            !preview_verification_error("probe_timeout private fixture")
                .message
                .contains("private")
        );
        assert!(preview_verification_status(hook_client::BoundWindowStatus::Verified).is_ok());
        assert_eq!(
            preview_verification_status(hook_client::BoundWindowStatus::Stale)
                .unwrap_err()
                .code,
            "preview_target_closed"
        );
        assert_eq!(
            preview_verification_status(hook_client::BoundWindowStatus::Unavailable)
                .unwrap_err()
                .code,
            "preview_verification_unavailable"
        );
    }
}

#[cfg(test)]
mod collector_weak_provider_tests {
    use super::*;
    struct Provider;
    impl PreviewProofProvider for Provider {
        fn verify_bound_window(&self, _: &str) -> AppResult<GameWindowBindingProjection> {
            panic!("unbound fixture must not dispatch a real proof")
        }
    }
    #[test]
    fn desktop_provider_is_weak_and_dead_provider_never_falls_back_to_a_held_hook_lease() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let provider: Arc<dyn PreviewProofProvider> = Arc::new(Provider);
        backend
            .configure_preview_proof(Arc::downgrade(&provider))
            .unwrap();
        assert_eq!(Arc::strong_count(&provider), 1);
        let _held = backend.lease.lock().unwrap();
        assert_eq!(
            backend.game_window_binding().unwrap_err().code,
            "game_window_unbound"
        );
        drop(provider);
        assert_eq!(
            backend.game_window_binding().unwrap_err().code,
            "preview_worker_unavailable"
        );
    }
}

#[cfg(test)]
mod frozen_preview_wire_tests {
    use super::*;

    #[test]
    fn frozen_worker_wire_preserves_names_full_width_identity_and_strict_envelopes() {
        let target = serde_json::json!({
            "pid": 812,
            "exe": "Example.exe",
            "full_image_path": "C:\\Games\\Example.exe",
            "creation_filetime": "18446744073709551615",
            "architecture": "x64",
            "errors": []
        });
        let candidate_wire = serde_json::json!({
            "target": target,
            "window": {
                "hwnd": "18446744073709551614",
                "title": "春の空に染める。",
                "class_name": "ExampleClass",
                "rect": {"left": -10, "top": 20, "right": 1010, "bottom": 720},
                "target": target
            },
            "settings": {"wine_path": "/test/wine", "bottle": "Probe bottle"},
            "environment_key": "crossover:/test/bottle"
        });
        let candidate: Candidate = serde_json::from_value(candidate_wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&candidate).unwrap(), candidate_wire);
        let wire = serde_json::json!({
            "candidate_id": "a".repeat(64),
            "candidate": candidate_wire,
            "image_path": "/test/bottle/drive_c/Games/Example.exe",
            "file_stamp": [1, 2, u64::MAX, -7, 8, -9, 10],
            "resource_manifest_sha256": "b".repeat(64)
        });
        let frozen: FrozenPreviewTarget = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(frozen.candidate_id(), "a".repeat(64));
        assert_eq!(frozen.window_title().as_deref(), Some("春の空に染める。"));
        assert_eq!(serde_json::to_value(&frozen).unwrap(), wire);
        let mut extra_envelope = wire.clone();
        extra_envelope["unrecognized"] = true.into();
        assert!(serde_json::from_value::<FrozenPreviewTarget>(extra_envelope).is_err());
        let mut extra_candidate = wire.clone();
        extra_candidate["candidate"]["unrecognized"] = true.into();
        assert!(serde_json::from_value::<FrozenPreviewTarget>(extra_candidate).is_err());
        let mut missing_image_path = wire.clone();
        missing_image_path
            .as_object_mut()
            .unwrap()
            .remove("image_path");
        assert!(serde_json::from_value::<FrozenPreviewTarget>(missing_image_path).is_err());
        let mut incomplete_stamp = wire;
        incomplete_stamp["file_stamp"] = serde_json::json!([1, 2, 3]);
        assert!(serde_json::from_value::<FrozenPreviewTarget>(incomplete_stamp).is_err());
    }
}

impl CoreHookBackend {
    fn associate_with_guard(
        &self,
        request: AssociateRequest,
        guard: Option<&AssociationExecutionGuard>,
        record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        // Fail promptly before resource/probe I/O when natural cleanup owns the
        // lease. Application association may already hold capture_lock.
        drop(
            self.lease
                .try_lock()
                .map_err(|_| error("capture_cleanup_pending", "上一会话正在收尾，请稍后重试"))?,
        );
        record(stage(
            "resource",
            "resource_check_started",
            "正在核对独立Core资源",
        ))?;
        let resource = self.native_resource()?;
        record(stage(
            "resource",
            "resource_verified",
            &format!(
                "独立Core资源清单核对完成 · {} · commit={} · manifest_sha256={}",
                resource.contract_version, resource.version, resource.manifest_sha256
            ),
        ))?;
        let candidate = self
            .candidates
            .lock()
            .map_err(|_| error("capture_state", "候选状态无效"))?
            .get(&request.candidate_id)
            .cloned()
            .ok_or_else(|| error("stale_target", "候选已过期，请刷新后重新核对"))?;
        let selected_core = self.selected_core_resource_fact(
            &resource,
            candidate
                .target
                .architecture
                .as_deref()
                .unwrap_or("unknown"),
        )?;
        record(stage(
            "resource",
            "selected_core_resources_verified",
            &selected_core,
        ))?;
        let identity = candidate
            .process_identity()
            .ok_or_else(|| error("incomplete_identity", "候选缺少完整进程身份，请重新扫描"))?;
        record(stage(
            "identity",
            "target_identity_verified",
            "候选环境、进程创建身份与位数已核对",
        ))?;
        let has_window = candidate.window.is_some();
        let preview_candidate = candidate
            .window
            .as_ref()
            .map(|_| (request.candidate_id.clone(), candidate.clone()));
        let mut lease = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "上一会话正在收尾，请稍后重试"))?;
        if let Some(guard) = guard {
            let active = self
                .active
                .lock()
                .map_err(|_| error("capture_state", "会话状态无效"))?;
            let current = active.as_ref();
            let full_active_matches = match (guard.expected_active.as_ref(), current) {
                (Some(expected), Some(current)) => same_hook_session_facts(expected, current),
                (None, None) => true,
                _ => false,
            };
            if !full_active_matches
                || guard.candidate_identity_sha256 != identity
                || guard.expected_active_session_id.as_deref()
                    != current.map(|s| s.session_id.as_str())
                || guard.expected_active_target_identity_sha256.as_deref()
                    != current.map(|s| s.target_identity_sha256.as_str())
                || guard.expected_broker_instance_id.as_deref()
                    != current.map(|s| s.broker_instance_id.as_str())
                || current.is_some_and(|s| {
                    self.client.snapshot().session_id.as_deref() != Some(s.session_id.as_str())
                })
            {
                return Err(error(
                    "association_review_stale",
                    "确认后的目标或活动会话已变化，请重新核对",
                ));
            }
        }
        check_target_switch(
            lease.as_ref().map(|(key, _)| key.as_str()),
            &identity,
            &self.client.snapshot().status,
            request.confirm_switch,
        )?;
        let new_lease = if lease.as_ref().is_some_and(|(key, _)| key == &identity) {
            None
        } else {
            std::fs::create_dir_all(&self.target_locks)
                .map_err(|_| error("target_lock", "无法创建目标租约目录"))?;
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(self.target_locks.join(format!("{identity}.lock")))
                .map_err(|_| error("target_lock", "无法打开目标租约"))?;
            file.try_lock()
                .map_err(|_| error("target_in_use", "另一个客户端配置档正在接入此游戏"))?;
            Some(file)
        };
        record(stage(
            "identity",
            "target_preflight_started",
            "正在重新核实新目标，尚未停止原会话",
        ))?;
        self.preflight_association_target(&candidate)?;
        record(stage(
            "identity",
            "target_preflight_verified",
            "新目标的完整运行身份已重新核实，原会话仍保留",
        ))?;
        // Publish the new native identity before the event adapter can consume
        // its first Ready/connected/text event. The callback queues stay bounded.
        let mut stopped_previous = false;
        let mut before = self.client.snapshot();
        let failed = before.session_id.as_ref().is_some_and(|id| {
            self.failed
                .lock()
                .map(|set| set.contains(id))
                .unwrap_or(true)
        });
        if failed {
            self.stop_owned_session()?;
            stopped_previous = true;
            before = self.client.snapshot();
        }

        let same = before.target.as_ref().is_some_and(|t| {
            t.pid == candidate.target.pid
                && t.full_image_path == candidate.target.full_image_path
                && t.creation_filetime == candidate.target.creation_filetime
                && t.architecture == candidate.target.architecture
        }) && before.settings.as_ref() == Some(&candidate.settings);
        if before.host_running && !same {
            if !request.confirm_switch {
                return Err(error("replace_required", "请确认切换正在取文的游戏"));
            }
            self.stop_owned_session()?;
            stopped_previous = true;
        }
        if stopped_previous {
            self.preflight_association_target(&candidate)?;
            record(stage(
                "identity",
                "target_revalidated_after_drain",
                "原会话收尾后再次核实了新目标完整身份",
            ))?;
        }
        let mut active = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态无效"))?;
        if same
            && let Some(current) = active.as_ref()
            && current.game_id.is_some()
            && request.game_id.is_some()
            && current.game_id != request.game_id
        {
            return Err(error(
                "session_game_conflict",
                "当前会话已有不同的游戏库归属，不能改写",
            ));
        }
        if same && let Some(current) = active.as_ref() {
            if current.session_id == before.session_id.as_deref().unwrap_or("") {
                // Reuse never requests another injection or replaces its lease.
                *self
                    .association_has_window
                    .lock()
                    .map_err(|_| error("capture_state", "关联状态不可用"))? = has_window;
                *self
                    .preview_binding
                    .lock()
                    .map_err(|_| error("capture_state", "窗口关联状态不可用"))? = preview_candidate;
                let mut retained = current.clone();
                if retained.game_id.is_none() {
                    retained.game_id = request.game_id.clone();
                }
                record(stage(
                    "association",
                    "association_reused",
                    "已复用当前会话，正文选择和阅读记录保留",
                ))?;
                return Ok(retained);
            }
        }
        record(stage(
            "injection",
            "injection_requested",
            &format!(
                "已请求匹配位数Core宿主接入 · {selected_core}；等待辅助程序结果与Core握手，尚不证明DLL已加载"
            ),
        ))?;
        let mut bound_at = None;
        let client = if let Some(window) = candidate.window.clone() {
            let outcome = self.client.bind_and_connect_window(
                candidate.settings.clone(),
                window,
                request.confirm_switch,
            );
            if matches!(
                outcome.status.as_str(),
                "replace_required" | "failed" | "cancelled" | "hook_failed"
            ) {
                return Err(error(
                    &outcome.status,
                    outcome
                        .error
                        .as_deref()
                        .unwrap_or("关联接入失败，请刷新目标后重试"),
                ));
            }
            bound_at = outcome
                .binding
                .binding
                .as_ref()
                .filter(|binding| {
                    binding.settings == candidate.settings
                        && candidate.window.as_ref().is_some_and(|window| {
                            binding.window.hwnd == window.hwnd
                                && binding.window.class_name == window.class_name
                                && binding.window.target == window.target
                        })
                })
                .map(|binding| binding.bound_at_ms);
            outcome.client
        } else {
            self.client
                .connect(candidate.settings.clone(), candidate.target.clone())
                .map_err(|_| error("connect_failed", "无法启动新Core宿主，请查看诊断阶段"))?;
            self.client.snapshot()
        };
        if let Some(file) = new_lease {
            *lease = Some((identity.clone(), file));
        }
        let session_id = client
            .session_id
            .ok_or_else(|| error("session_missing", "宿主未建立会话身份"))?;
        *self
            .preview_bound_at
            .lock()
            .map_err(|_| error("capture_state", "窗口关联回执不可用"))? = bound_at
            .zip(candidate.selection_identity())
            .map(|(at, identity)| (session_id.clone(), identity, at));
        if let Some(current) = active.as_ref()
            && current.session_id == session_id
        {
            *self
                .association_has_window
                .lock()
                .map_err(|_| error("capture_state", "关联状态不可用"))? = has_window;
            *self
                .preview_binding
                .lock()
                .map_err(|_| error("capture_state", "窗口关联状态不可用"))? = preview_candidate;
            let mut reviewed = current.clone();
            if reviewed.game_id.is_none() {
                reviewed.game_id = request.game_id.clone();
            }
            return Ok(reviewed);
        }
        let target = client.target.unwrap_or(candidate.target);
        let input = HookSessionInput {
            session_id: session_id.clone(),
            game_id: request.game_id,
            environment_key: candidate.environment_key,
            target_exe_path: target
                .full_image_path
                .ok_or_else(|| error("incomplete_identity", "缺少目标完整路径"))?,
            windows_pid: target.pid,
            process_created_u64: target
                .creation_filetime
                .ok_or_else(|| error("incomplete_identity", "缺少进程创建时间"))?,
            architecture: target
                .architecture
                .ok_or_else(|| error("incomplete_identity", "缺少目标位数"))?,
            broker_instance_id: session_id,
            resource,
            target_identity_sha256: identity,
        };
        *self
            .association_has_window
            .lock()
            .map_err(|_| error("capture_state", "关联状态不可用"))? = has_window;
        let mut provisional = input.clone();
        provisional.game_id = None;
        *active = Some(provisional);
        *self
            .preview_binding
            .lock()
            .map_err(|_| error("capture_state", "窗口关联状态不可用"))? = preview_candidate;
        Ok(input)
    }
}

#[cfg(test)]
mod binding_journey_context_tests {
    use super::*;

    #[test]
    fn review_reads_cached_identity_without_starting_host_and_rejects_missing_effect_owner() {
        let backend = CoreHookBackend::new(PathBuf::new(), PathBuf::new(), PathBuf::new());
        let target = Target {
            pid: 72,
            exe: "game.exe".into(),
            full_image_path: Some("C:\\Games\\game.exe".into()),
            creation_filetime: Some("18446744073709551615".into()),
            architecture: Some("x64".into()),
            errors: vec![],
        };
        let candidate = Candidate {
            target,
            window: None,
            settings: LaunchSettings {
                wine_path: "/not-invoked/wine".into(),
                bottle: "bottle".into(),
            },
            environment_key: "crossover:/bottle".into(),
        };
        let projected = backend
            .remember(vec![(candidate.clone(), "game".into())])
            .unwrap();
        let context = backend
            .association_context(&projected.candidates[0].candidate_id)
            .unwrap();
        assert_eq!(
            context.candidate.process_created_u64,
            "18446744073709551615"
        );
        assert!(context.active.is_none());
        assert!(!backend.client.snapshot().host_running);
        assert!(backend.lease.lock().unwrap().is_none());
        let expected = HookSessionInput {
            session_id: "never-created".into(),
            game_id: None,
            environment_key: candidate.environment_key,
            target_exe_path: "C:\\Games\\game.exe".into(),
            windows_pid: 72,
            process_created_u64: "18446744073709551615".into(),
            architecture: "x64".into(),
            broker_instance_id: "never-created".into(),
            resource: ResourceSnapshotInput {
                snapshot_id: "unused".into(),
                kind: "core".into(),
                version: "unused".into(),
                manifest_sha256: "a".repeat(64),
                contract_version: "1".into(),
                metadata_json: "{}".into(),
            },
            target_identity_sha256: "b".repeat(64),
        };
        let mut effects = 0;
        assert!(
            backend
                .with_active_session_guard(&expected, &mut || {
                    effects += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(effects, 0);
        let _held = backend.lease.lock().unwrap();
        assert_eq!(
            backend
                .association_context(&context.candidate.candidate_id)
                .unwrap_err()
                .code,
            "capture_cleanup_pending"
        );
    }
}
