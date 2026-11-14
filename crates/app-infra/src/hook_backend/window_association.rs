//! Windows candidate discovery and association reads. No lease acquisition or commit ownership.
use super::{CoreHookBackend, error};
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
};
use hook_client::{LaunchSettings, Target, WindowTarget};
use std::path::PathBuf;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Candidate {
    pub(super) target: Target,
    pub(super) window: Option<WindowTarget>,
    pub(super) settings: LaunchSettings,
    pub(super) environment_key: String,
}
impl Candidate {
    /// The injected process owns the lease independently of its visible windows.
    pub(super) fn process_identity(&self) -> Option<String> {
        Some(content_hash(format!(
            "{}\0{}\0{}\0{}\0{}",
            self.environment_key,
            self.target.full_image_path.as_deref()?,
            self.target.pid,
            self.target.creation_filetime.as_deref()?,
            self.target.architecture.as_deref()?,
        )))
    }
    /// Opaque selection identity. A Win32 HWND never stands in for a macOS ID.
    pub(super) fn selection_identity(&self) -> Option<String> {
        let process = self.process_identity()?;
        Some(content_hash(match &self.window {
            Some(window) => format!(
                "window-candidate-v1\0{process}\0{}\0{}",
                window.hwnd, window.class_name,
            ),
            None => format!("process-candidate-v1\0{process}"),
        }))
    }
}
pub(super) fn crossover_environments() -> AppResult<Vec<(LaunchSettings, String)>> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| error("home_missing", "无法定位当前用户的CrossOver环境"))?;
    let wine = [
        home.join("Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"),
        PathBuf::from("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .ok_or_else(|| error("crossover_missing", "未找到CrossOver，请安装后重试"))?;
    let bottles = home.join("Library/Application Support/CrossOver/Bottles");
    let mut result = Vec::new();
    for entry in std::fs::read_dir(bottles)
        .map_err(|_| error("bottles_missing", "未找到CrossOver容器"))?
        .flatten()
    {
        let p = entry.path();
        if !p.join("cxbottle.conf").is_file() {
            continue;
        }
        let canonical = p
            .canonicalize()
            .map_err(|_| error("bottle_unavailable", "无法核验容器路径"))?;
        let bottle = entry.file_name().to_string_lossy().into_owned();
        let key = format!("crossover:{}", canonical.display());
        result.push((
            LaunchSettings {
                wine_path: wine.to_string_lossy().into_owned(),
                bottle,
            },
            key,
        ));
    }
    result.sort_by(|a, b| a.0.bottle.cmp(&b.0.bottle));
    Ok(result)
}

impl CoreHookBackend {
    pub(super) fn read_active_context(&self) -> AppResult<Option<HookSessionInput>> {
        let active = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .clone();
        if let Some(active) = &active {
            let snapshot = self.client.snapshot();
            if !snapshot.host_running
                || !matches!(
                    snapshot.status.as_str(),
                    "starting" | "ready" | "attaching" | "connected"
                )
                || snapshot.session_id.as_deref() != Some(active.session_id.as_str())
                || snapshot.target.as_ref().is_none_or(|target| {
                    target.pid != active.windows_pid
                        || target.full_image_path.as_deref()
                            != Some(active.target_exe_path.as_str())
                        || target.creation_filetime.as_deref()
                            != Some(active.process_created_u64.as_str())
                        || target.architecture.as_deref() != Some(active.architecture.as_str())
                })
            {
                return Err(error(
                    "association_context_stale",
                    "活动宿主的完整进程身份已经变化",
                ));
            }
        }
        Ok(active)
    }
    /// The M02 association receipt is a cached Windows fact, independent of
    /// Native preview permission, selection and frame availability. This read
    /// does not acquire the Hook lease or invoke any identity probe.
    pub fn cached_preview_binding(&self) -> AppResult<GameWindowBindingProjection> {
        let (active, candidate) = self.trusted_bound_candidate()?;
        let identity = candidate
            .selection_identity()
            .ok_or_else(|| error("game_window_stale", "窗口关联身份不完整"))?;
        let observed = self
            .preview_bound_at
            .lock()
            .map_err(|_| error("capture_state", "窗口关联回执不可用"))?
            .as_ref()
            .filter(|(session, target, _)| session == &active.session_id && target == &identity)
            .map(|(_, _, at)| at.to_string());
        let (after, current) = self.trusted_bound_candidate()?;
        if after.session_id != active.session_id
            || current.selection_identity().as_deref() != Some(identity.as_str())
        {
            return Err(error("game_window_stale", "读取期间窗口关联已变化"));
        }
        Ok(GameWindowBindingProjection {
            state: GameWindowBindingState::Bound,
            session_id: Some(active.session_id),
            target_identity_sha256: Some(identity),
            window_title: candidate.window.as_ref().map(|window| window.title.clone()),
            verified_at_ms: observed,
            error_code: None,
            detail: "保留已确认的 Windows 窗口关联；预览选择与权限独立".into(),
        })
    }
    pub(super) fn trusted_bound_candidate(&self) -> AppResult<(HookSessionInput, Candidate)> {
        let active = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .clone()
            .ok_or_else(|| error("game_window_unbound", "当前没有已关联的游戏窗口"))?;
        let (_, candidate) = self
            .preview_binding
            .lock()
            .map_err(|_| error("capture_state", "关联状态不可用"))?
            .clone()
            .ok_or_else(|| {
                if *self
                    .association_has_window
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                {
                    error("game_window_unbound", "窗口关联已解除，取文仍可独立保留")
                } else {
                    error(
                        "game_window_no_window",
                        "当前通过进程接入，尚未选择游戏窗口",
                    )
                }
            })?;
        if candidate.process_identity().as_deref() != Some(active.target_identity_sha256.as_str())
            || self.client.snapshot().session_id.as_deref() != Some(active.session_id.as_str())
        {
            return Err(error(
                "game_window_stale",
                "当前关联身份已变化，请重新核对窗口",
            ));
        }
        if candidate.window.is_none() {
            return Err(error("game_window_no_window", "当前接入没有已核验窗口"));
        }
        Ok((active, candidate))
    }
    pub fn preview_candidate_id_for_active_binding(&self) -> AppResult<String> {
        let active = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .clone()
            .ok_or_else(|| error("preview_target_missing", "当前未关联游戏窗口"))?;
        let binding = self
            .preview_binding
            .lock()
            .map_err(|_| error("capture_state", "窗口关联状态不可用"))?
            .clone()
            .ok_or_else(|| error("preview_target_missing", "当前会话没有已核验的窗口关联"))?;
        if binding.1.process_identity().as_deref() != Some(active.target_identity_sha256.as_str())
            || self.client.snapshot().session_id.as_deref() != Some(active.session_id.as_str())
        {
            return Err(error("stale_target", "已关联窗口身份已变化"));
        }
        Ok(binding.0)
    }
    pub(super) fn preview_candidate(&self, candidate_id: &str) -> AppResult<Candidate> {
        let candidate = self
            .candidates
            .lock()
            .map_err(|_| error("capture_state", "候选状态不可用"))?
            .get(candidate_id)
            .cloned();
        let candidate = candidate
            .or_else(|| {
                self.preview_binding
                    .lock()
                    .ok()?
                    .as_ref()
                    .filter(|(id, _)| id == candidate_id)
                    .map(|(_, c)| c.clone())
            })
            .ok_or_else(|| error("stale_target", "候选窗口已过期，请重新选择"))?;
        if candidate.selection_identity().as_deref() != Some(candidate_id)
            || candidate.window.is_none()
        {
            return Err(error("stale_target", "候选窗口身份不一致"));
        }
        Ok(candidate)
    }
    pub(super) fn environments(&self) -> AppResult<Vec<(LaunchSettings, String)>> {
        crossover_environments()
    }
    pub(super) fn remember(
        &self,
        candidates: Vec<(Candidate, String)>,
    ) -> AppResult<TargetCandidates> {
        let mut map = self
            .candidates
            .lock()
            .map_err(|_| error("capture_state", "无法读取候选状态"))?;
        map.clear();
        let mut output = Vec::new();
        for (candidate, title) in candidates {
            let Some(path) = candidate.target.full_image_path.clone() else {
                continue;
            };
            let Some(created) = candidate.target.creation_filetime.clone() else {
                continue;
            };
            let Some(architecture) = candidate.target.architecture.clone() else {
                continue;
            };
            let Some(id) = candidate.selection_identity() else {
                continue;
            };
            output.push(TargetCandidate {
                candidate_id: id.clone(),
                window_title: title,
                executable_path: path,
                windows_pid: candidate.target.pid,
                process_created_u64: created,
                architecture,
                environment_key: candidate.environment_key.clone(),
                bottle_label: candidate.settings.bottle.clone(),
                has_window: candidate.window.is_some(),
                window_handle: candidate.window.as_ref().map(|window| window.hwnd.clone()),
                // The Win32 probe does not attest macOS host or CoreGraphics identity.
                macos_host_pid: None,
                macos_window_id: None,
            });
            map.insert(id, candidate);
        }
        Ok(TargetCandidates {
            candidates: output,
            diagnostics: vec![],
        })
    }
    pub fn pick_window(&self, environment_key: Option<String>) -> AppResult<TargetCandidates> {
        let settings = self.client.snapshot().settings;
        let environments = self.environments()?;
        let selected = if let Some(key) = environment_key {
            environments.into_iter().find(|(_, k)| k == &key)
        } else if let Some(settings) = settings {
            environments.into_iter().find(|(s, _)| s == &settings)
        } else if environments.len() == 1 {
            environments.into_iter().next()
        } else {
            None
        };
        let (settings, key) =
            selected.ok_or_else(|| error("choose_environment", "请先在列表选择游戏所在的容器"))?;
        let result = self.client.pick_window(&settings).map_err(|_| {
            error(
                "window_picker_failed",
                "窗口点选失败或已取消，可用窗口列表重试",
            )
        })?;
        if let Some(window) = result.window {
            self.remember(vec![(
                Candidate {
                    target: window.target.clone(),
                    window: Some(window.clone()),
                    settings,
                    environment_key: key,
                },
                window.title,
            )])
        } else {
            Ok(TargetCandidates {
                candidates: vec![],
                diagnostics: vec![],
            })
        }
    }
    pub(super) fn read_game_window_binding(&self) -> AppResult<GameWindowBindingProjection> {
        let provider = self
            .preview_proof
            .lock()
            .map_err(|_| error("capture_state", "窗口核验状态不可用"))?
            .clone();
        if let Some(provider) = provider {
            let provider = provider
                .upgrade()
                .ok_or_else(|| error("preview_worker_unavailable", "窗口采集进程不可用"))?;
            let (active, candidate) = self.trusted_bound_candidate()?;
            let identity = candidate
                .selection_identity()
                .ok_or_else(|| error("game_window_stale", "窗口身份不完整"))?;
            // No M02 operation/lease lock is held while the one worker verifies.
            let verified = provider.verify_bound_window(&identity);
            let mut projection = match verified {
                Ok(projection) => projection,
                Err(failure)
                    if matches!(
                        failure.code.as_str(),
                        "preview_capture_busy"
                            | "preview_verification_unavailable"
                            | "preview_worker_transport"
                            | "preview_worker_unavailable"
                            | "preview_generation_conflict"
                            | "preview_cancelled"
                            | "preview_permission_denied"
                            | "preview_permission_revoked"
                            | "preview_selection_required"
                    ) =>
                {
                    GameWindowBindingProjection {
                        state: GameWindowBindingState::Unavailable,
                        session_id: Some(active.session_id.clone()),
                        target_identity_sha256: Some(identity.clone()),
                        window_title: candidate.window.as_ref().map(|w| w.title.clone()),
                        verified_at_ms: None,
                        error_code: Some(failure.code),
                        detail: "独立采集核验尚未完成，旧缓存不作为新鲜核验".into(),
                    }
                }
                Err(failure) => return Err(failure),
            };
            let (after, current) = self.trusted_bound_candidate()?;
            if after.session_id != active.session_id
                || current.selection_identity().as_deref() != Some(identity.as_str())
                || projection.target_identity_sha256.as_deref() != Some(identity.as_str())
            {
                return Err(error("stale_target", "窗口核验期间关联已变化"));
            }
            projection.session_id = Some(active.session_id);
            return Ok(projection);
        }
        // Standalone/non-desktop backend compatibility. The desktop always
        // installs its worker provider; a dead installed provider never falls back.

        let _guard = self
            .lease
            .try_lock()
            .map_err(|_| error("capture_cleanup_pending", "会话正在关联或收尾，请稍后重试"))?;
        let current = self
            .active
            .lock()
            .map_err(|_| error("capture_state", "会话状态不可用"))?
            .clone();
        let mut projection = GameWindowBindingProjection {
            state: GameWindowBindingState::Unbound,
            session_id: current.as_ref().map(|s| s.session_id.clone()),
            target_identity_sha256: None,
            window_title: None,
            verified_at_ms: None,
            error_code: None,
            detail: "当前没有已关联的游戏窗口".into(),
        };
        let (_, candidate) = match self.trusted_bound_candidate() {
            Ok(value) => value,
            Err(e) => {
                projection.state = match e.code.as_str() {
                    "game_window_no_window" => GameWindowBindingState::NoWindow,
                    "game_window_unbound" => GameWindowBindingState::Unbound,
                    _ => GameWindowBindingState::Stale,
                };
                projection.error_code = Some(e.code);
                projection.detail = e.message;
                return Ok(projection);
            }
        };
        let window = candidate
            .window
            .as_ref()
            .ok_or_else(|| error("game_window_no_window", "尚未选择窗口"))?;
        match self.client.verify_bound_window(&candidate.settings, window) {
            Ok(verification) => {
                if matches!(
                    verification.status,
                    hook_client::BoundWindowStatus::Verified
                ) {
                    projection.verified_at_ms = Some(window_observed_ms());
                    projection.target_identity_sha256 = candidate.selection_identity();
                }
                projection.window_title = verification.window_title;
                projection.error_code = verification.error_code;
                projection.state = match verification.status {
                    hook_client::BoundWindowStatus::Verified => GameWindowBindingState::Bound,
                    hook_client::BoundWindowStatus::Stale => GameWindowBindingState::Stale,
                    hook_client::BoundWindowStatus::Unavailable => {
                        GameWindowBindingState::Unavailable
                    }
                };
                projection.detail = match projection.state {
                    GameWindowBindingState::Bound => "已重新核验当前关联窗口",
                    GameWindowBindingState::Stale => "窗口已关闭或身份发生变化，请重新关联",
                    _ => "当前无法核验关联窗口，请稍后重试",
                }
                .into();
            }
            Err(e) => {
                projection.state = GameWindowBindingState::Unavailable;
                let busy = e.starts_with("window_proof_busy:");
                if busy {
                    // This is the trusted cached binding, not a new native
                    // verification. Preserve its identity without publishing a
                    // temporary title or a successful verification timestamp.
                    projection.target_identity_sha256 = candidate.selection_identity();
                }
                projection.error_code = Some(
                    if busy {
                        "window_proof_busy"
                    } else {
                        "game_window_verification_unavailable"
                    }
                    .into(),
                );
                projection.detail = if busy {
                    "窗口身份核验正在进行，请稍后重试"
                } else {
                    "窗口身份查询暂不可用，请稍后重试"
                }
                .into();
            }
        }
        Ok(projection)
    }
    pub(super) fn discover_candidates(&self) -> AppResult<TargetCandidates> {
        let mut found = Vec::new();
        let mut diagnostics = Vec::new();
        for (settings, key) in self.environments()? {
            match self.client.discover_windows(&settings) {
                Ok(windows) => {
                    for window in windows {
                        found.push((
                            Candidate {
                                target: window.target.clone(),
                                window: Some(window.clone()),
                                settings: settings.clone(),
                                environment_key: key.clone(),
                            },
                            window.title,
                        ));
                    }
                }
                Err(_) => diagnostics.push(Diagnostic {
                    code: "window_discovery_failed".into(),
                    stage: "target".into(),
                    message: format!("容器“{}”暂时无法枚举窗口", settings.bottle),
                    recoverable: true,
                }),
            }
            // Explicit process candidates allow games with an unavailable/unmapped visible window.
            if let Ok(targets) = self.client.discover_targets(&settings) {
                for target in targets {
                    if found
                        .iter()
                        .any(|(c, _)| c.environment_key == key && c.target.pid == target.pid)
                    {
                        continue;
                    }
                    let label = target.exe.clone();
                    found.push((
                        Candidate {
                            target,
                            window: None,
                            settings: settings.clone(),
                            environment_key: key.clone(),
                        },
                        label,
                    ));
                }
            }
        }
        let mut result = self.remember(found)?;
        result.diagnostics = diagnostics;
        Ok(result)
    }
    /// Fresh read-only target proof before any old host is stopped. The client
    /// still performs its own post-drain verification before binding/injection.
    pub(super) fn preflight_association_target(&self, candidate: &Candidate) -> AppResult<()> {
        if let Some(window) = &candidate.window {
            let proof = self
                .client
                .verify_bound_window(&candidate.settings, window)
                .map_err(|_| {
                    error(
                        "target_preflight_unavailable",
                        "尚未核实新游戏窗口，已保留原会话，请重新扫描",
                    )
                })?;
            if !matches!(proof.status, hook_client::BoundWindowStatus::Verified) {
                return Err(error(
                    "stale_target",
                    "新游戏窗口的完整身份已失效，原会话未停止",
                ));
            }
        } else {
            let targets = self
                .client
                .discover_targets(&candidate.settings)
                .map_err(|_| {
                    error(
                        "target_preflight_unavailable",
                        "尚未核实新游戏进程，已保留原会话，请重新扫描",
                    )
                })?;
            if !targets.iter().any(|fresh| {
                fresh.pid == candidate.target.pid
                    && fresh.full_image_path == candidate.target.full_image_path
                    && fresh.creation_filetime == candidate.target.creation_filetime
                    && fresh.architecture == candidate.target.architecture
            }) {
                return Err(error(
                    "stale_target",
                    "新游戏进程的路径、位数或创建身份已变化，原会话未停止",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn association_candidate_projection(
        &self,
        candidate_id: &str,
    ) -> AppResult<TargetCandidate> {
        let candidates = self
            .candidates
            .lock()
            .map_err(|_| error("capture_state", "候选状态无效"))?;
        let candidate = candidates
            .get(candidate_id)
            .ok_or_else(|| error("stale_target", "候选已过期，请重新扫描"))?;
        Ok(TargetCandidate {
            candidate_id: candidate_id.into(),
            window_title: candidate
                .window
                .as_ref()
                .map(|window| window.title.clone())
                .unwrap_or_default(),
            executable_path: candidate
                .target
                .full_image_path
                .clone()
                .ok_or_else(|| error("incomplete_identity", "候选缺少完整路径"))?,
            windows_pid: candidate.target.pid,
            process_created_u64: candidate
                .target
                .creation_filetime
                .clone()
                .ok_or_else(|| error("incomplete_identity", "候选缺少进程创建身份"))?,
            architecture: candidate
                .target
                .architecture
                .clone()
                .ok_or_else(|| error("incomplete_identity", "候选缺少架构"))?,
            environment_key: candidate.environment_key.clone(),
            bottle_label: candidate.settings.bottle.clone(),
            has_window: candidate.window.is_some(),
            window_handle: candidate.window.as_ref().map(|window| window.hwnd.clone()),
            macos_host_pid: None,
            macos_window_id: None,
        })
    }
}

pub(super) fn window_observed_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
