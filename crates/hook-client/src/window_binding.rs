//! Verified window association and the explicit bind-and-connect user action.
use super::{
    Client, ClientSnapshot, ConnectOutcome, ConnectionOperation, LaunchSettings, SessionRelation,
    Target, attach_params, capture_command, is_discoverable_target, launcher, match_identity,
    timestamp_ms,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

// Prevent overlapping challenges from backing up each other's temporary title.
static WINDOW_PROOF_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(test)]
static WINDOW_PROOF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn window_proof_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    WINDOW_PROOF_LOCK
        .try_lock()
        .map_err(|_| "window_proof_busy: 已有窗口身份核验或激活正在进行，请稍后重试".into())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowTarget {
    /// Decimal string: HWND must not pass through a JavaScript Number.
    pub hwnd: String,
    pub title: String,
    pub class_name: String,
    pub rect: WindowRect,
    pub target: Target,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPickResult {
    pub status: String,
    pub window: Option<WindowTarget>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoundWindowStatus {
    Verified,
    Stale,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundWindowVerification {
    pub status: BoundWindowStatus,
    pub window_title: Option<String>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundWindowActivation {
    pub status: String,
    pub was_minimized: bool,
    pub show_was_visible: bool,
    pub set_foreground_accepted: bool,
    pub visible_after: bool,
    pub foreground_after: bool,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowBinding {
    pub window: WindowTarget,
    pub settings: LaunchSettings,
    pub bound_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowBindingSnapshot {
    pub binding: Option<WindowBinding>,
    pub status: String,
    pub last_error: Option<String>,
}

/// The command only starts the asynchronous host supervisor. A `connecting` or
/// `reused` flow is not proof of injection, ProcessConnected, or received text;
/// callers must render those independent facts from `client` and the event subscription.
#[derive(Debug, Clone, Serialize)]
pub struct WindowConnectionResult {
    pub status: String,
    pub binding: WindowBindingSnapshot,
    pub client: ClientSnapshot,
    pub error: Option<String>,
}

pub(crate) struct BindingState {
    snapshot: WindowBindingSnapshot,
    revision: u64,
    checked_at: Option<Instant>,
}

impl Default for BindingState {
    fn default() -> Self {
        Self {
            snapshot: WindowBindingSnapshot {
                binding: None,
                status: "unbound".into(),
                last_error: None,
            },
            revision: 0,
            checked_at: None,
        }
    }
}

struct Picking<'a>(&'a AtomicBool);
impl Drop for Picking<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Client {
    fn bound_window_probe<T: for<'de> Deserialize<'de>>(
        &self,
        settings: &LaunchSettings,
        window: &WindowTarget,
        action: &str,
    ) -> Result<T, String> {
        valid_window(window)?;
        let pid = window.target.pid.to_string();
        self.window_probe(
            settings,
            &[
                action,
                &window.hwnd,
                &pid,
                window
                    .target
                    .full_image_path
                    .as_deref()
                    .ok_or("缺少完整路径")?,
                window
                    .target
                    .creation_filetime
                    .as_deref()
                    .ok_or("缺少进程创建身份")?,
                window
                    .target
                    .architecture
                    .as_deref()
                    .ok_or("缺少进程位数")?,
                &window.class_name,
            ],
            Duration::from_secs(12),
        )
    }
    /// Query exact bound identity even while hidden/minimized. Discovery's
    /// visible-window filtering is intentionally independent.
    pub fn verify_bound_window(
        &self,
        settings: &LaunchSettings,
        window: &WindowTarget,
    ) -> Result<BoundWindowVerification, String> {
        let _proof = window_proof_guard()?;
        let value: BoundWindowVerification =
            self.bound_window_probe(settings, window, "--verify-bound-window")?;
        if matches!(value.status, BoundWindowStatus::Verified)
            && (value.error_code.is_some() || value.window_title.is_none())
        {
            return Err("窗口身份核验回执不一致".into());
        }
        Ok(value)
    }
    pub fn activate_bound_window(
        &self,
        settings: &LaunchSettings,
        window: &WindowTarget,
    ) -> Result<BoundWindowActivation, String> {
        let _proof = window_proof_guard()?;
        let value: BoundWindowActivation =
            self.bound_window_probe(settings, window, "--activate-window")?;
        match value.status.as_str() {
            "activated"
                if value.error_code.is_none() && value.visible_after && value.foreground_after => {}
            "visible_not_foreground"
                if value.error_code.is_none() && value.visible_after && !value.foreground_after => {
            }
            "failed" if value.error_code.is_some() => {}
            _ => return Err("窗口激活结果与实际观察不一致".into()),
        }
        Ok(value)
    }
    fn window_probe<T: for<'de> Deserialize<'de>>(
        &self,
        settings: &LaunchSettings,
        args: &[&str],
        timeout: Duration,
    ) -> Result<T, String> {
        let executable = self
            .0
            .resource_dir
            .join("brokers/x64/target_probe.exe")
            .canonicalize()
            .map_err(|e| format!("缺少窗口查询程序 target_probe.exe: {e}"))?;
        let mut command = launcher(settings, &executable)?;
        command.args(args);
        let (stdout, stderr, success) = capture_command(command, timeout)?;
        if !success {
            return Err(format!(
                "窗口查询失败: {}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        serde_json::from_slice(&stdout).map_err(|e| {
            format!(
                "窗口查询输出无效: {e}; stderr={}",
                String::from_utf8_lossy(&stderr)
            )
        })
    }

    pub fn discover_windows(&self, settings: &LaunchSettings) -> Result<Vec<WindowTarget>, String> {
        #[derive(Deserialize)]
        struct Report {
            windows: Vec<WindowTarget>,
        }
        let report: Report =
            self.window_probe(settings, &["--windows"], Duration::from_secs(12))?;
        Ok(report
            .windows
            .into_iter()
            .filter(|window| valid_window(window).is_ok())
            .collect())
    }

    pub fn pick_window(&self, settings: &LaunchSettings) -> Result<WindowPickResult, String> {
        if self.0.picking_window.swap(true, Ordering::AcqRel) {
            return Err("已有窗口点选正在进行，请等待本次点选结束".into());
        }
        let _reservation = Picking(&self.0.picking_window);
        let result: WindowPickResult =
            self.window_probe(settings, &["--pick-window", "15"], Duration::from_secs(22))?;
        match (result.status.as_str(), result.window.as_ref()) {
            ("picked", Some(window)) => {
                valid_window(window).map_err(|e| format!("所选窗口不适合绑定: {e}"))?
            }
            ("cancelled" | "timeout" | "not_found", None) => {}
            _ => return Err("窗口点选返回了不一致的状态".into()),
        }
        Ok(result)
    }

    /// Confirm the selected window and immediately request the existing native
    /// Hook session. A different active target is never stopped without the
    /// caller's explicit `replace_existing` confirmation.
    pub fn bind_and_connect_window(
        &self,
        settings: LaunchSettings,
        window: WindowTarget,
        replace_existing: bool,
    ) -> WindowConnectionResult {
        let operation = match self.begin_connection_operation() {
            Ok(operation) => operation,
            Err(error) => return self.window_connection_result("failed", Some(error)),
        };
        let revision = self.binding_revision();
        let outcome = (|| -> Result<(&str, Option<String>), String> {
            valid_window(&window)?;
            operation.check()?;
            // Verify before asking to replace, and once again after any shutdown
            // wait. A PID/HWND may be recycled while the old host is draining.
            let fresh = self.verified_window(&settings, &window)?;
            operation.check()?;
            if self.session_relation(&settings, &fresh.target) == SessionRelation::Different {
                if !replace_existing {
                    return Ok((
                        "replace_required",
                        Some(
                            "已有另一个取文目标；确认切换后才会关闭旧宿主并关联、接入新窗口".into(),
                        ),
                    ));
                }
                self.wait_for_replaced_session(&operation, Duration::from_secs(15))?;
            }
            let fresh = if self.session_relation(&settings, &fresh.target) == SessionRelation::None
                && replace_existing
            {
                self.verified_window(&settings, &window)?
            } else {
                fresh
            };
            self.commit_window_binding(&settings, fresh.clone(), revision, &operation)?;
            Ok(self.connect_bound_target(
                settings,
                fresh.target,
                &operation,
                revision.wrapping_add(1),
            ))
        })();
        match outcome {
            Ok((status, error)) => self.window_connection_result(status, error),
            Err(error) => self.window_connection_result(
                if operation.check().is_err() {
                    "cancelled"
                } else {
                    "failed"
                },
                Some(error),
            ),
        }
    }

    fn connect_bound_target(
        &self,
        settings: LaunchSettings,
        target: Target,
        operation: &ConnectionOperation<'_>,
        binding_revision: u64,
    ) -> (&'static str, Option<String>) {
        match self.connect_reserved(settings, target, operation, Some(binding_revision)) {
            Ok(ConnectOutcome::Started) => ("connecting", None),
            Ok(ConnectOutcome::Reused) => ("reused", None),
            Err(error) => {
                let status = if operation.check().is_err() {
                    "cancelled"
                } else {
                    "hook_failed"
                };
                self.record_binding_error(error.clone());
                (status, Some(error))
            }
        }
    }

    fn window_connection_result(
        &self,
        status: &str,
        error: Option<String>,
    ) -> WindowConnectionResult {
        WindowConnectionResult {
            status: status.into(),
            binding: self.binding_snapshot(),
            client: self.snapshot(),
            error,
        }
    }

    fn record_binding_error(&self, error: String) {
        let mut state = self
            .0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Keep a successfully confirmed association available for retry. The
        // regular Hook snapshot remains authoritative for the host lifecycle.
        if state.snapshot.binding.is_some() {
            state.snapshot.last_error = Some(error);
        }
    }

    pub(crate) fn check_binding_revision(&self, revision: u64) -> Result<(), String> {
        let state = self
            .0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.revision == revision && state.snapshot.binding.is_some() {
            Ok(())
        } else {
            Err("确认后的窗口关联已改变，未启动新的 Hook 会话，请重新选择窗口".into())
        }
    }

    fn binding_snapshot(&self) -> WindowBindingSnapshot {
        self.0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }

    fn binding_revision(&self) -> u64 {
        self.0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revision
    }

    fn verified_window(
        &self,
        settings: &LaunchSettings,
        window: &WindowTarget,
    ) -> Result<WindowTarget, String> {
        let windows = self.discover_windows(settings)?;
        match_window(window, windows)
            .ok_or_else(|| "窗口已关闭或身份发生变化，请重新选择窗口".into())
    }

    fn wait_for_replaced_session(
        &self,
        operation: &ConnectionOperation<'_>,
        timeout: Duration,
    ) -> Result<(), String> {
        operation.check()?;
        self.request_disconnect()?;
        let deadline = Instant::now() + timeout;
        while self.has_active_session() {
            operation.check()?;
            if Instant::now() >= deadline {
                return Err("旧宿主尚未完成退出，未关联或注入新目标；请稍后重试".into());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        operation.check()
    }

    /// Association-only API retained for non-UI callers. The desktop confirm
    /// action uses bind_and_connect_window instead.
    pub fn bind_window(
        &self,
        settings: LaunchSettings,
        window: WindowTarget,
    ) -> Result<WindowBindingSnapshot, String> {
        let operation = self.begin_connection_operation()?;
        valid_window(&window)?;
        let revision = self.binding_revision();
        let fresh = self.verified_window(&settings, &window)?;
        self.commit_window_binding(&settings, fresh, revision, &operation)
    }

    fn commit_window_binding(
        &self,
        settings: &LaunchSettings,
        fresh: WindowTarget,
        revision: u64,
        operation: &ConnectionOperation<'_>,
    ) -> Result<WindowBindingSnapshot, String> {
        let mut state = self
            .0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        operation.check()?;
        if state.revision != revision {
            return Err("窗口关联已被其他操作更新，请重新确认".into());
        }
        state.revision = state.revision.wrapping_add(1);
        state.checked_at = Some(Instant::now());
        state.snapshot = WindowBindingSnapshot {
            binding: Some(WindowBinding {
                window: fresh,
                settings: settings.clone(),
                bound_at_ms: timestamp_ms(),
            }),
            status: "bound".into(),
            last_error: None,
        };
        Ok(state.snapshot.clone())
    }

    pub fn unbind_window(&self) -> WindowBindingSnapshot {
        // Cancel any not-yet-reserved connection preparation, while preserving
        // an already owned Hook session and its text/stream history. Share the
        // final reservation lock, always session -> binding, so cancellation
        // cannot finish between the last epoch check and host reservation.
        let _session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
        self.0.disconnect_epoch.fetch_add(1, Ordering::AcqRel);
        let mut state = self
            .0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        state.revision = state.revision.wrapping_add(1);
        state.checked_at = None;
        state.snapshot = BindingState::default().snapshot;
        state.snapshot.clone()
    }

    /// Remove only the local window association while preserving the owned
    /// Hook session, stream selection and received text. The callback is a
    /// controlled SQL-only durable commit: it must not reenter Client/native
    /// APIs or dispatch work onto AppKit. Lock order is session -> binding;
    /// the caller already owns its application capture/binding lease.
    /// Failed commits restore the exact cached association, but not old
    /// cancellation epochs/revisions (in-flight preparations stay cancelled).
    pub fn with_window_unbound<T, E>(&self, commit: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        let _session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
        self.0.disconnect_epoch.fetch_add(1, Ordering::AcqRel);
        let mut state = self
            .0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let before = state.snapshot.clone();
        let checked_at = state.checked_at;
        state.revision = state.revision.wrapping_add(1);
        state.checked_at = None;
        state.snapshot = BindingState::default().snapshot;
        match commit() {
            Ok(receipt) => Ok(receipt),
            Err(error) => {
                state.snapshot = before;
                state.checked_at = checked_at;
                Err(error)
            }
        }
    }

    /// Cached at most three seconds, independent of the normal Hook snapshot poll.
    pub fn window_binding_status(&self, settings: &LaunchSettings) -> WindowBindingSnapshot {
        let (binding, revision) = {
            let mut state = self
                .0
                .window_binding
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let Some(binding) = state.snapshot.binding.clone() else {
                return state.snapshot.clone();
            };
            if &binding.settings != settings {
                state.revision = state.revision.wrapping_add(1);
                state.snapshot = WindowBindingSnapshot {
                    binding: None,
                    status: "settings_changed".into(),
                    last_error: Some("运行环境或 Bottle 已改变，请重新选择窗口".into()),
                };
                return state.snapshot.clone();
            }
            if state
                .checked_at
                .is_some_and(|time| time.elapsed() < Duration::from_secs(3))
            {
                return state.snapshot.clone();
            }
            state.checked_at = Some(Instant::now());
            (binding, state.revision)
        };
        let result = self.discover_windows(settings);
        let mut state = self
            .0
            .window_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.revision != revision {
            return state.snapshot.clone();
        }
        match result {
            Ok(windows) => {
                if let Some(fresh) = match_window(&binding.window, windows) {
                    if let Some(binding) = state.snapshot.binding.as_mut() {
                        binding.window = fresh;
                    }
                } else {
                    state.revision = state.revision.wrapping_add(1);
                    state.snapshot = WindowBindingSnapshot {
                        binding: None,
                        status: "window_closed".into(),
                        last_error: Some("窗口已关闭或进程身份发生变化，已解除窗口关联".into()),
                    };
                }
            }
            Err(error) => {
                state.revision = state.revision.wrapping_add(1);
                state.snapshot = WindowBindingSnapshot {
                    binding: None,
                    status: "verification_failed".into(),
                    last_error: Some(format!("无法继续确认窗口身份，已解除关联: {error}")),
                };
            }
        }
        state.snapshot.clone()
    }
}

fn valid_window(window: &WindowTarget) -> Result<(), String> {
    let handle = window.hwnd.parse::<u64>().map_err(|_| "窗口句柄无效")?;
    if handle == 0 || handle.to_string() != window.hwnd {
        return Err("窗口句柄必须是非零规范十进制字符串".into());
    }
    if window.title.trim().is_empty() || window.class_name.is_empty() {
        return Err("窗口标题或类名不可用".into());
    }
    if window.rect.right <= window.rect.left || window.rect.bottom <= window.rect.top {
        return Err("窗口不可见或尺寸无效".into());
    }
    attach_params(&window.target)?;
    if !is_discoverable_target(&window.target) {
        return Err("系统或查询工具窗口不作为游戏窗口".into());
    }
    Ok(())
}

fn match_window(selected: &WindowTarget, windows: Vec<WindowTarget>) -> Option<WindowTarget> {
    windows.into_iter().find(|candidate| {
        candidate.hwnd == selected.hwnd
            && candidate.class_name == selected.class_name
            && match_identity(&selected.target, vec![candidate.target.clone()]).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window() -> WindowTarget {
        WindowTarget {
            hwnd: "18446744073709551615".into(),
            title: "ゲーム".into(),
            class_name: "GameWindow".into(),
            rect: WindowRect {
                left: 1,
                top: 1,
                right: 100,
                bottom: 100,
            },
            target: Target {
                pid: 42,
                exe: "game.exe".into(),
                full_image_path: Some("C:\\Games\\game.exe".into()),
                creation_filetime: Some("133000000000000000".into()),
                architecture: Some("x86".into()),
                errors: vec![],
            },
        }
    }
    #[test]
    fn title_challenge_rejects_untrusted_nonce_before_observing_or_launching() {
        let _test = WINDOW_PROOF_TEST_LOCK.lock().unwrap();
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let settings = LaunchSettings {
            wine_path: "/not-used".into(),
            bottle: "test".into(),
        };
        let called = AtomicBool::new(false);
        for nonce in [
            "Game title",
            "LunaWindowProof-short",
            "LunaWindowProof-1234567890123456/unsafe",
        ] {
            let result = client.with_window_title_challenge(&settings, &window(), nonce, || {
                called.store(true, Ordering::Release);
                Ok(())
            });
            assert!(result.is_err());
        }
        assert!(!called.load(Ordering::Acquire));
    }
    #[test]
    fn decimal_handles_preserve_precision_and_reject_ambiguous_input() {
        assert!(valid_window(&window()).is_ok());
        for hwnd in ["0", "0002", "-1", "0x123", "18446744073709551616"] {
            let mut value = window();
            value.hwnd = hwnd.into();
            assert!(valid_window(&value).is_err());
        }
        let json = serde_json::to_string(&window()).unwrap();
        assert!(json.contains("\"18446744073709551615\""));
    }
    #[test]
    fn recycled_pid_or_hwnd_is_not_accepted_but_titles_and_positions_can_change() {
        let selected = window();
        let mut fresh = selected.clone();
        fresh.title = "next scene".into();
        fresh.rect.left = 2;
        assert!(match_window(&selected, vec![fresh.clone()]).is_some());
        fresh.target.creation_filetime = Some("133000000000000001".into());
        assert!(match_window(&selected, vec![fresh.clone()]).is_none());
        fresh = selected.clone();
        fresh.hwnd = "43".into();
        assert!(match_window(&selected, vec![fresh]).is_none());
    }
    #[test]
    fn changing_bottle_clears_binding_without_touching_hook_session() {
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let settings = LaunchSettings {
            wine_path: "/wine".into(),
            bottle: "Bottle A".into(),
        };
        {
            let mut state = client.0.window_binding.lock().unwrap();
            state.snapshot = WindowBindingSnapshot {
                binding: Some(WindowBinding {
                    window: window(),
                    settings: settings.clone(),
                    bound_at_ms: 1,
                }),
                status: "bound".into(),
                last_error: None,
            };
        }
        let before = serde_json::to_value(client.snapshot()).unwrap();
        let changed = LaunchSettings {
            bottle: "Bottle B".into(),
            ..settings
        };
        let status = client.window_binding_status(&changed);
        assert!(status.binding.is_none());
        assert_eq!(status.status, "settings_changed");
        assert_eq!(serde_json::to_value(client.snapshot()).unwrap(), before);
        assert!(!client.has_active_session());
    }

    #[test]
    fn unbinding_never_changes_hook_snapshot_or_session() {
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let before = serde_json::to_value(client.snapshot()).unwrap();
        let result = client.unbind_window();
        assert!(result.binding.is_none());
        assert_eq!(serde_json::to_value(client.snapshot()).unwrap(), before);
        assert!(!client.has_active_session());
    }

    #[test]
    fn compensated_unbind_restores_cache_on_commit_failure_and_never_resumes_cancelled_preparations()
     {
        let (client, controls) = reserved_client("connected");
        {
            let mut state = client.0.window_binding.lock().unwrap();
            state.snapshot = WindowBindingSnapshot {
                binding: Some(WindowBinding {
                    window: window(),
                    settings: settings(),
                    bound_at_ms: 7,
                }),
                status: "bound".into(),
                last_error: Some("retained cache".into()),
            };
            state.revision = 9;
            state.checked_at = Some(Instant::now());
        }
        let before = serde_json::to_value(client.binding_snapshot()).unwrap();
        let hook_before = serde_json::to_value(client.snapshot()).unwrap();
        let epoch = client.0.disconnect_epoch.load(Ordering::Acquire);
        let error = client
            .with_window_unbound(|| {
                assert!(client.0.session.try_lock().is_err());
                assert!(client.0.window_binding.try_lock().is_err());
                Err::<(), _>("original SQL commit error")
            })
            .unwrap_err();
        assert_eq!(error, "original SQL commit error");
        assert_eq!(
            serde_json::to_value(client.binding_snapshot()).unwrap(),
            before
        );
        assert_eq!(
            serde_json::to_value(client.snapshot()).unwrap(),
            hook_before
        );
        assert_eq!(client.0.window_binding.lock().unwrap().revision, 10);
        assert!(client.0.window_binding.lock().unwrap().checked_at.is_some());
        assert_eq!(client.0.disconnect_epoch.load(Ordering::Acquire), epoch + 1);
        assert!(client.has_active_session());
        assert!(matches!(
            controls.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        assert_eq!(
            client
                .with_window_unbound(|| Ok::<_, &str>("durable receipt"))
                .unwrap(),
            "durable receipt"
        );
        assert!(client.binding_snapshot().binding.is_none());
        assert_eq!(client.binding_snapshot().status, "unbound");
        assert_eq!(
            serde_json::to_value(client.snapshot()).unwrap(),
            hook_before
        );
        assert!(client.has_active_session());
        assert!(matches!(
            controls.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    fn settings() -> LaunchSettings {
        LaunchSettings {
            wine_path: "/wine".into(),
            bottle: "Bottle A".into(),
        }
    }

    fn reserved_client(status: &str) -> (Client, std::sync::mpsc::Receiver<super::super::Control>) {
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let (control, rx) = std::sync::mpsc::channel();
        *client.0.session.lock().unwrap() = Some(super::super::SessionHandle {
            id: "existing".into(),
            control,
            shutdown_requested: std::sync::Arc::new(AtomicBool::new(false)),
            settings: settings(),
            target: window().target,
        });
        *client.0.state.lock().unwrap() = ClientSnapshot {
            session_id: Some("existing".into()),
            status: status.into(),
            host_running: true,
            target: Some(window().target),
            settings: Some(settings()),
            ..ClientSnapshot::default()
        };
        (client, rx)
    }

    #[test]
    fn same_complete_target_reuses_startup_and_connected_session_without_new_host() {
        for status in ["starting", "ready", "attaching", "connected"] {
            let (client, rx) = reserved_client(status);
            let before = serde_json::to_value(client.snapshot()).unwrap();
            // No packaged resources exist in this test. Reuse succeeds without
            // spawning a probe/host or writing a new session/log directory.
            for _ in 0..2 {
                client.connect(settings(), window().target).unwrap();
            }
            assert_eq!(serde_json::to_value(client.snapshot()).unwrap(), before);
            assert!(matches!(
                rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));
            assert_eq!(client.0.counter.load(Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn active_identity_includes_environment_bottle_path_creation_exe_and_architecture() {
        let (client, _rx) = reserved_client("connected");
        let mut other_settings = settings();
        other_settings.bottle = "Bottle B".into();
        assert_eq!(
            client.session_relation(&other_settings, &window().target),
            SessionRelation::Different
        );
        other_settings = settings();
        other_settings.wine_path = "/another/wine".into();
        assert_eq!(
            client.session_relation(&other_settings, &window().target),
            SessionRelation::Different
        );
        for changed in 0..5 {
            let mut target = window().target;
            match changed {
                0 => target.pid += 1,
                1 => target.exe = "other.exe".into(),
                2 => target.full_image_path = Some(r"C:\Other\game.exe".into()),
                3 => target.creation_filetime = Some("133000000000000001".into()),
                4 => target.architecture = Some("x64".into()),
                _ => unreachable!(),
            }
            assert_eq!(
                client.session_relation(&settings(), &target),
                SessionRelation::Different
            );
            assert!(client.connect(settings(), target).is_err());
        }
        assert!(client.connect(other_settings, window().target).is_err());
        assert_eq!(client.snapshot().session_id.as_deref(), Some("existing"));
    }

    #[test]
    fn cancelled_or_failed_session_cannot_be_reused_as_connected() {
        let (client, _rx) = reserved_client("error");
        assert_eq!(
            client.session_relation(&settings(), &window().target),
            SessionRelation::Different
        );
        client.0.state.lock().unwrap().status = "connected".into();
        client.disconnect().unwrap();
        assert_eq!(
            client.session_relation(&settings(), &window().target),
            SessionRelation::Different
        );
        assert!(client.connect(settings(), window().target).is_err());
    }

    #[test]
    fn pending_bind_and_manual_connect_are_serialized_and_disconnect_cancels_preparation() {
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let operation = client.begin_connection_operation().unwrap();
        assert!(client.begin_connection_operation().is_err());
        assert!(client.connect(settings(), window().target).is_err());
        client.disconnect().unwrap();
        assert!(operation.check().is_err());
        assert!(
            client
                .commit_window_binding(&settings(), window(), 0, &operation)
                .is_err()
        );
        assert!(!client.has_active_session());
        drop(operation);
        assert!(client.begin_connection_operation().is_ok());
    }

    #[test]
    fn unbind_cancels_pending_bind_but_preserves_existing_hook_session() {
        let (client, rx) = reserved_client("connected");
        let operation = client.begin_connection_operation().unwrap();
        client
            .commit_window_binding(&settings(), window(), 0, &operation)
            .unwrap();
        let before = serde_json::to_value(client.snapshot()).unwrap();
        assert!(client.unbind_window().binding.is_none());
        assert!(operation.check().is_err());
        assert_eq!(serde_json::to_value(client.snapshot()).unwrap(), before);
        assert!(client.has_active_session());
        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn hook_start_failure_keeps_confirmed_window_and_returns_failure_not_connection() {
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let operation = client.begin_connection_operation().unwrap();
        client
            .commit_window_binding(&settings(), window(), 0, &operation)
            .unwrap();
        let (status, error) =
            client.connect_bound_target(settings(), window().target, &operation, 1);
        let result = client.window_connection_result(status, error);
        assert_eq!(result.status, "hook_failed");
        assert!(
            result
                .error
                .as_ref()
                .is_some_and(|error| error.contains("target_probe.exe"))
        );
        assert_eq!(result.binding.status, "bound");
        assert_eq!(result.binding.binding.unwrap().window, window());
        assert!(result.binding.last_error.is_some());
        assert_eq!(result.client.status, "idle");
        assert!(!result.client.host_running);
        assert!(result.client.session_id.is_none());
        assert!(!client.has_active_session());
    }

    #[test]
    fn stale_binding_revision_prevents_host_reservation_and_invalid_window_does_not_bind() {
        let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let operation = client.begin_connection_operation().unwrap();
        client
            .commit_window_binding(&settings(), window(), 0, &operation)
            .unwrap();
        assert!(client.check_binding_revision(0).is_err());
        assert!(client.check_binding_revision(1).is_ok());
        assert!(
            client
                .connect_reserved(settings(), window().target, &operation, Some(0))
                .is_err()
        );
        drop(operation);
        client.unbind_window();
        let mut invalid = window();
        invalid.hwnd = "0".into();
        let result = client.bind_and_connect_window(settings(), invalid, false);
        assert_eq!(result.status, "failed");
        assert!(result.error.is_some());
        assert!(result.binding.binding.is_none());
        assert!(!client.has_active_session());
    }

    #[test]
    fn replacement_wait_is_bounded_and_does_not_commit_new_binding_early() {
        let (client, rx) = reserved_client("connected");
        let operation = client.begin_connection_operation().unwrap();
        let started = Instant::now();
        let result = client.wait_for_replaced_session(&operation, Duration::from_millis(5));
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(matches!(rx.try_recv(), Ok(super::super::Control::Shutdown)));
        assert!(client.has_active_session());
        assert!(client.binding_snapshot().binding.is_none());
        assert_eq!(client.snapshot().target.unwrap(), window().target);
    }

    #[test]
    fn replacement_wait_allows_supervisor_to_finish_and_cancellation_to_interrupt() {
        let (client, rx) = reserved_client("connected");
        let operation = client.begin_connection_operation().unwrap();
        let finisher = client.clone();
        let thread = std::thread::spawn(move || {
            assert!(matches!(
                rx.recv_timeout(Duration::from_secs(1)),
                Ok(super::super::Control::Shutdown)
            ));
            finisher.finish_session("existing");
        });
        client
            .wait_for_replaced_session(&operation, Duration::from_secs(1))
            .unwrap();
        thread.join().unwrap();
        assert!(!client.has_active_session());
        drop(operation);

        let (client, rx) = reserved_client("connected");
        let operation = client.begin_connection_operation().unwrap();
        let canceller = client.clone();
        let thread = std::thread::spawn(move || {
            assert!(matches!(
                rx.recv_timeout(Duration::from_secs(1)),
                Ok(super::super::Control::Shutdown)
            ));
            canceller.unbind_window();
        });
        assert!(
            client
                .wait_for_replaced_session(&operation, Duration::from_secs(1))
                .is_err()
        );
        thread.join().unwrap();
        assert!(client.has_active_session());
        assert!(client.binding_snapshot().binding.is_none());
    }

    #[test]
    fn cancellation_and_final_reservation_use_the_same_short_session_lock() {
        for unbind in [false, true] {
            let client = Client::new(std::path::PathBuf::new(), std::path::PathBuf::new());
            let operation = client.begin_connection_operation().unwrap();
            let reservation_lock = client.0.session.lock().unwrap();
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (done_tx, done_rx) = std::sync::mpsc::channel();
            let canceller = client.clone();
            let thread = std::thread::spawn(move || {
                started_tx.send(()).unwrap();
                if unbind {
                    canceller.unbind_window();
                } else {
                    canceller.disconnect().unwrap();
                }
                done_tx.send(()).unwrap();
            });
            started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            // The cancellation cannot complete while a final reservation owns
            // this lock, and its epoch cannot change outside that boundary.
            assert!(done_rx.recv_timeout(Duration::from_millis(10)).is_err());
            assert!(operation.check().is_ok());
            drop(reservation_lock);
            done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(operation.check().is_err());
            thread.join().unwrap();
            assert!(!client.has_active_session());
        }
    }
}

impl Client {
    /// Attest a verified Win32 HWND to a native host window with a short-lived
    /// nonce. Observer failure still restores the title. No game termination or
    /// Core ABI/protocol modification is involved.
    pub fn with_window_title_challenge<T>(
        &self,
        settings: &LaunchSettings,
        window: &WindowTarget,
        nonce: &str,
        observe: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        use std::{
            io::{BufRead, BufReader, Read, Write},
            process::Stdio,
            sync::mpsc,
        };
        let _proof = window_proof_guard()?;
        valid_window(window)?;
        if !nonce.starts_with("LunaWindowProof-")
            || !(32..=112).contains(&nonce.len())
            || !nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("窗口核验挑战格式无效".into());
        }
        let executable = self
            .0
            .resource_dir
            .join("brokers/x64/target_probe.exe")
            .canonicalize()
            .map_err(|_| "缺少窗口身份查询程序")?;
        let mut command = launcher(settings, &executable)?;
        command.args([
            "--window-title-challenge",
            &window.hwnd,
            &window.target.pid.to_string(),
            window
                .target
                .full_image_path
                .as_deref()
                .ok_or("缺少目标路径")?,
            window
                .target
                .creation_filetime
                .as_deref()
                .ok_or("缺少目标创建身份")?,
            nonce,
            window
                .target
                .architecture
                .as_deref()
                .ok_or("缺少目标位数")?,
            &window.class_name,
        ]);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("窗口身份查询无法启动: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("窗口核验输入不可用")?;
        let stdout = child.stdout.take().ok_or("窗口核验输出不可用")?;
        let stderr = child.stderr.take().ok_or("窗口核验错误输出不可用")?;
        let (tx, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            for _ in 0..2 {
                let mut bytes = Vec::new();
                let result = reader.by_ref().take(4097).read_until(b'\n', &mut bytes);
                let parsed = result
                    .map_err(|_| "proof read failed".to_owned())
                    .and_then(|_| {
                        if bytes.len() > 4096 {
                            return Err("proof output limit".into());
                        }
                        serde_json::from_slice::<serde_json::Value>(&bytes)
                            .map_err(|_| "invalid proof receipt".into())
                    });
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        let errors = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.take(4096).read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        });
        let exchange = (|| {
            let ready = rx
                .recv_timeout(Duration::from_secs(10))
                .map_err(|_| "窗口身份核验启动超时")??;
            if ready["status"] == "unavailable"
                && ready["errorCode"] == "window_title_proof_requires_unicode"
            {
                return Err("window_title_proof_requires_unicode: 此窗口不能安全完成预览归属核验，已保留原标题；可通过进程列表接入游戏取文".into());
            }
            if ready["status"] != "ready" {
                return Err("窗口身份核验未就绪".into());
            }
            let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(observe))
                .unwrap_or_else(|_| Err("窗口核验观察中断，原标题恢复后可重试".into()));
            let send = stdin.write_all(b"restore\n").and_then(|_| stdin.flush());
            // Even an observer error must wait for and validate restoration.
            let restored = rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| "窗口原标题恢复未确认，请检查游戏窗口")??;
            if restored["status"] != "restored"
                || restored["titleRestored"] != true
                || restored["identityVerified"] != true
                || restored["requested"] != true
                || send.is_err()
            {
                return Err("窗口原标题恢复或身份二次核验失败".into());
            }
            observed
        })();
        // No early protocol/read/observer error may leave a child holding the
        // temporary caption. EOF tells the probe to restore without a request;
        // then wait for this owned child only, within the probe's restore budget.
        drop(stdin);
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => {
                    // Never signal a game, a bottle or a process family.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("窗口核验子进程未正常结束，原标题恢复未确认".into());
                }
            }
        };
        // A descendant must not keep the caller blocked by retaining stderr.
        let detail = if errors.is_finished() {
            errors.join().unwrap_or_default()
        } else {
            String::new()
        };
        if !status.success() {
            return Err(format!(
                "窗口核验失败，原标题恢复未确认: {}; {detail}",
                exchange.err().unwrap_or_else(|| "查询程序异常退出".into())
            ));
        }
        exchange
    }
}

#[cfg(all(test, unix))]
mod bound_action_client_tests {
    use super::*;
    // These real child-process tests intentionally serialize their own users of
    // the production global native-action guard. The concurrent negative test
    // below exercises overlapping operations explicitly.

    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    fn fixture() -> (Client, LaunchSettings, WindowTarget, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "luna-bound-action-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("brokers/x64")).unwrap();
        std::fs::write(
            dir.join("brokers/x64/target_probe.exe"),
            b"controlled probe",
        )
        .unwrap();
        let launcher = dir.join("launcher.py");
        std::fs::write(
            &launcher,
            r#"#!/usr/bin/env python3
import json,sys,pathlib
root=pathlib.Path(__file__).parent
(root/'actual-args.json').write_text(json.dumps(sys.argv[1:]))
print((root/'receipt.json').read_text(),flush=True)
"#,
        )
        .unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
        let window = WindowTarget {
            hwnd: "18446744073709551615".into(),
            title: "登録時の題名".into(),
            class_name: "窓クラス".into(),
            rect: WindowRect {
                left: 0,
                top: 0,
                right: 100,
                bottom: 100,
            },
            target: Target {
                pid: 42,
                exe: "game.exe".into(),
                full_image_path: Some("C:\\Games\\game.exe".into()),
                creation_filetime: Some("18446744073709551614".into()),
                architecture: Some("x64".into()),
                errors: vec![],
            },
        };
        let settings = LaunchSettings {
            wine_path: launcher.to_string_lossy().into_owned(),
            bottle: "controlled bottle".into(),
        };
        (
            Client::new(dir.clone(), dir.join("logs")),
            settings,
            window,
            dir,
        )
    }
    #[test]
    fn bound_probe_preserves_full_identity_and_reports_actual_foreground_denial() {
        let _test = WINDOW_PROOF_TEST_LOCK.lock().unwrap();
        let (client, settings, window, dir) = fixture();
        std::fs::write(
            dir.join("receipt.json"),
            r#"{"status":"verified","windowTitle":"新しい題名","errorCode":null}"#,
        )
        .unwrap();
        let verified = client.verify_bound_window(&settings, &window).unwrap();
        assert_eq!(verified.status, BoundWindowStatus::Verified);
        assert_eq!(verified.window_title.as_deref(), Some("新しい題名"));
        let args: Vec<String> =
            serde_json::from_slice(&std::fs::read(dir.join("actual-args.json")).unwrap()).unwrap();
        let i = args
            .iter()
            .position(|a| a == "--verify-bound-window")
            .unwrap();
        assert_eq!(
            &args[i + 1..],
            &[
                window.hwnd.clone(),
                "42".into(),
                "C:\\Games\\game.exe".into(),
                "18446744073709551614".into(),
                "x64".into(),
                "窓クラス".into()
            ]
        );
        assert!(!client.has_active_session());
        std::fs::write(dir.join("receipt.json"),r#"{"status":"visible_not_foreground","wasMinimized":true,"showWasVisible":false,"setForegroundAccepted":false,"visibleAfter":true,"foregroundAfter":false,"errorCode":null}"#).unwrap();
        let result = client.activate_bound_window(&settings, &window).unwrap();
        assert_eq!(result.status, "visible_not_foreground");
        assert!(!result.set_foreground_accepted);
        assert!(!result.foreground_after);
        assert!(result.visible_after);
        assert!(result.was_minimized);
        assert!(!result.show_was_visible);
        assert!(!client.has_active_session());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn real_title_challenge_rejects_concurrent_query_and_activation_before_any_probe() {
        let _test = WINDOW_PROOF_TEST_LOCK.lock().unwrap();
        let (client, settings, window, dir) = fixture();
        std::fs::write(&settings.wine_path,r#"#!/usr/bin/env python3
import json,sys,pathlib
root=pathlib.Path(__file__).parent
with (root/'calls.jsonl').open('a') as f:
    f.write(json.dumps(sys.argv[1:])+'\n')
print(json.dumps(dict(status='ready')),flush=True)
requested=sys.stdin.readline().strip()=='restore'
print(json.dumps(dict(status='restored',titleRestored=True,identityVerified=True,requested=requested)),flush=True)
"#).unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let worker_client = client.clone();
        let worker_settings = settings.clone();
        let worker_window = window.clone();
        let worker = std::thread::spawn(move || {
            worker_client.with_window_title_challenge(
                &worker_settings,
                &worker_window,
                "LunaWindowProof-Concurrency-123456789",
                || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                },
            )
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            client
                .verify_bound_window(&settings, &window)
                .unwrap_err()
                .starts_with("window_proof_busy:")
        );
        assert!(
            client
                .activate_bound_window(&settings, &window)
                .unwrap_err()
                .starts_with("window_proof_busy:")
        );
        let calls = std::fs::read_to_string(dir.join("calls.jsonl")).unwrap();
        assert_eq!(calls.lines().count(), 1);
        assert!(calls.contains("--window-title-challenge"));
        assert!(!calls.contains("--verify-bound-window"));
        assert!(!calls.contains("--activate-window"));
        release_tx.send(()).unwrap();
        worker.join().unwrap().unwrap();
        assert!(WINDOW_PROOF_LOCK.try_lock().is_ok());
        assert!(!client.has_active_session());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn title_proof_protocol_errors_reap_owned_child_and_observer_failures_restore() {
        let _test = WINDOW_PROOF_TEST_LOCK.lock().unwrap();
        for mode in [
            "ansi",
            "invalid_ready",
            "observer_error",
            "observer_panic",
            "bad_restore",
        ] {
            let (client, settings, window, dir) = fixture();
            std::fs::write(dir.join("mode"), mode).unwrap();
            std::fs::write(&settings.wine_path, r#"#!/usr/bin/env python3
import json,sys,pathlib
root=pathlib.Path(__file__).parent
mode=(root/'mode').read_text()
if mode=='ansi':
    print(json.dumps(dict(status='unavailable',errorCode='window_title_proof_requires_unicode')),flush=True)
elif mode=='invalid_ready':
    print('invalid-json',flush=True)
else:
    print(json.dumps(dict(status='ready')),flush=True)
request=sys.stdin.readline()
(root/'received-input').write_text(request)
if mode not in ['ansi','invalid_ready']:
    print(json.dumps(dict(status='restored',titleRestored=mode!='bad_restore',identityVerified=True,requested=request.strip()=='restore')),flush=True)
(root/'normal-exit').write_text('owned child exited')
"#).unwrap();
            let called = AtomicBool::new(false);
            let result = client.with_window_title_challenge(
                &settings,
                &window,
                "LunaWindowProof-Protocol-1234567890",
                || {
                    called.store(true, Ordering::Release);
                    if mode == "observer_panic" {
                        panic!("controlled observer panic");
                    }
                    if mode == "observer_error" {
                        return Err("controlled observer error".into());
                    }
                    Ok(())
                },
            );
            let error = result.unwrap_err();
            assert!(
                dir.join("normal-exit").is_file(),
                "{mode}: child was not reaped normally"
            );
            let request = std::fs::read_to_string(dir.join("received-input")).unwrap();
            if matches!(mode, "ansi" | "invalid_ready") {
                assert!(!called.load(Ordering::Acquire));
                assert_eq!(request, "");
            } else {
                assert!(called.load(Ordering::Acquire));
                assert_eq!(request, "restore\n");
            }
            if mode == "ansi" {
                assert!(error.starts_with("window_title_proof_requires_unicode:"));
            }
            if mode == "observer_error" {
                assert_eq!(error, "controlled observer error");
            }
            assert!(WINDOW_PROOF_LOCK.try_lock().is_ok());
            assert!(!client.has_active_session());
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
    #[test]
    fn malformed_success_and_invalid_bound_target_never_become_activation_success() {
        let _test = WINDOW_PROOF_TEST_LOCK.lock().unwrap();
        let (client, settings, mut window, dir) = fixture();
        std::fs::write(dir.join("receipt.json"),r#"{"status":"activated","wasMinimized":false,"showWasVisible":true,"setForegroundAccepted":true,"visibleAfter":true,"foregroundAfter":false,"errorCode":null}"#).unwrap();
        assert!(client.activate_bound_window(&settings, &window).is_err());
        std::fs::write(
            dir.join("receipt.json"),
            r#"{"status":"verified","windowTitle":null,"errorCode":"window_identity_changed"}"#,
        )
        .unwrap();
        assert!(client.verify_bound_window(&settings, &window).is_err());
        std::fs::write(
            dir.join("receipt.json"),
            r#"{"status":"stale","windowTitle":null,"errorCode":"window_identity_changed"}"#,
        )
        .unwrap();
        assert_eq!(
            client
                .verify_bound_window(&settings, &window)
                .unwrap()
                .status,
            BoundWindowStatus::Stale
        );
        std::fs::remove_file(dir.join("actual-args.json")).unwrap();
        window.hwnd = "0".into();
        assert!(client.activate_bound_window(&settings, &window).is_err());
        assert!(!dir.join("actual-args.json").exists());
        assert!(!client.has_active_session());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
