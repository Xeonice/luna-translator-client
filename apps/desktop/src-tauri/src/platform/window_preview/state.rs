//! Pure preview decisions. The service owns resources, locks and effects.
use app_core::contracts::window_capabilities::{PreviewProjection, PreviewState};

/// All service entry points use the same physical ownership budget. A source
/// awaiting actual release still consumes its one draining slot.
pub(super) fn lease_budget_admits(active: usize, draining: usize, additional: usize) -> bool {
    draining <= 1
        && active.saturating_add(additional) <= 2
        && active.saturating_add(draining).saturating_add(additional) <= 3
}

/// A checked diagnostic summary is data; platform validation stays with the service.
pub(super) fn projection_snapshot(
    projection: &PreviewProjection,
    diagnostic_summary: Option<&str>,
) -> PreviewProjection {
    let mut projection = projection.clone();
    if let Some(detail) = diagnostic_summary {
        projection.detail.push_str("\n诊断：");
        projection.detail.push_str(detail);
    }
    projection
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeIdentity<'a> {
    pub candidate: &'a str,
    pub pid: u32,
    pub process_start: &'a str,
    pub window: u32,
}

#[cfg(target_os = "macos")]
pub(super) struct RemoteFrameLease<'a> {
    pub remote: bool,
    pub generation: &'a str,
    pub stop_requested: bool,
    pub state: &'a PreviewState,
    pub candidate: &'a str,
    pub capture_id: Option<&'a str>,
    pub target: Option<NativeIdentity<'a>>,
}

#[cfg(target_os = "macos")]
pub(super) struct RemoteFrameObservation<'a> {
    pub generation: &'a str,
    pub capture_id: &'a str,
    pub native: NativeIdentity<'a>,
}

/// This service fence supplements the worker's full frame/permission validation.
/// A new capture is still subject to the worker's authorization, not trusted here.
#[cfg(target_os = "macos")]
pub(super) fn remote_frame_admitted(
    lease: RemoteFrameLease<'_>,
    observation: RemoteFrameObservation<'_>,
) -> bool {
    lease.remote
        && lease.generation == observation.generation
        && !lease.stop_requested
        && *lease.state != PreviewState::Paused
        && lease.candidate == observation.native.candidate
        && (lease.capture_id != Some(observation.capture_id)
            || lease
                .target
                .is_none_or(|target| target == observation.native))
}

/// Evaluated under the second service lock after the collector resolved identity.
/// The service handles stop and actual cleanup before this decision.
pub(super) fn capture_completion_admitted(
    current_generation: &str,
    expected_generation: &str,
    current_state: &PreviewState,
    current_target_hash: Option<&str>,
    capture_target_hash: &str,
) -> bool {
    current_generation == expected_generation
        && *current_state != PreviewState::Paused
        && current_target_hash == Some(capture_target_hash)
}

pub(super) struct FrameCommitDecision {
    pub sequence: u64,
    pub content_changed: bool,
}

/// Service delivery sequence is independent of the worker's frame sequence.
pub(super) fn next_frame_commit(
    previous_sequence: u64,
    previous_content_hash: Option<&str>,
    content_hash: &str,
) -> Option<FrameCommitDecision> {
    previous_sequence
        .checked_add(1)
        .map(|sequence| FrameCommitDecision {
            sequence,
            content_changed: previous_content_hash != Some(content_hash),
        })
}

#[cfg(target_os = "macos")]
pub(super) struct CollectorStatusDecision {
    pub state: PreviewState,
    pub detail: &'static str,
    pub clear_image: bool,
}

/// Metadata updates permission separately; only a validated frame establishes Live.
#[cfg(target_os = "macos")]
pub(super) fn collector_status_policy(
    phase: &str,
    code: Option<&str>,
) -> Option<CollectorStatusDecision> {
    if matches!(phase, "Live" | "CaptureReady") {
        return None;
    }
    Some(CollectorStatusDecision {
        state: if phase == "Paused" {
            PreviewState::Paused
        } else if code == Some("preview_target_closed") {
            PreviewState::TargetClosed
        } else if matches!(
            code,
            Some("preview_permission_denied" | "preview_permission_revoked")
        ) {
            PreviewState::PermissionDenied
        } else if matches!(
            code,
            Some("preview_window_selection_required" | "preview_selection_required")
        ) {
            PreviewState::Unavailable
        } else if code.is_some() {
            PreviewState::Stale
        } else {
            PreviewState::Starting
        },
        detail: if phase == "Selecting" {
            "请在系统浮层中选择并确认游戏窗口"
        } else if matches!(
            code,
            Some("preview_window_selection_required" | "preview_selection_required")
        ) {
            "此窗口需要你通过系统选择对应的游戏窗口"
        } else if code == Some("preview_selection_timeout") {
            "系统选窗未在期限内完成，可在当前客户端重新选择"
        } else if code.is_some() {
            "当前游戏画面暂不可用，最后帧保留原采集时间"
        } else {
            "独立采集进程正在核对窗口"
        },
        clear_image: matches!(
            code,
            Some(
                "preview_target_closed"
                    | "preview_window_selection_required"
                    | "preview_selection_required"
                    | "preview_selection_expired"
                    | "preview_selection_mismatch"
            )
        ),
    })
}

pub(super) struct CaptureFailureDecision {
    pub state: PreviewState,
    pub permission_denied: bool,
    pub invalidate_pairing: bool,
    pub detail: &'static str,
}

/// Capture completion retains its existing semantics, distinct from metadata.
pub(super) fn capture_failure_policy(code: &str, paired: bool) -> CaptureFailureDecision {
    let state = match code {
        "preview_permission_revoked" => PreviewState::PermissionRevoked,
        "preview_target_closed" => PreviewState::TargetClosed,
        "preview_window_not_on_screen" => PreviewState::Unavailable,
        "preview_capture_timeout" | "preview_capture_busy" | "preview_verification_unavailable" => {
            PreviewState::Stale
        }
        _ => PreviewState::Unavailable,
    };
    let detail = if state == PreviewState::PermissionRevoked {
        "录屏权限已撤销，取文不受影响"
    } else if state == PreviewState::TargetClosed {
        "目标窗口已退出，请重新选择"
    } else if matches!(
        code,
        "preview_window_selection_required" | "preview_selection_required"
    ) {
        "此窗口需要你通过系统选择对应的游戏窗口"
    } else {
        "当前窗口画面暂不可用，最后帧保留原采集时间"
    };
    CaptureFailureDecision {
        state,
        permission_denied: code == "preview_permission_revoked",
        invalidate_pairing: paired
            && matches!(
                code,
                "preview_selection_mismatch"
                    | "preview_selection_expired"
                    | "stale_target"
                    | "preview_target_closed"
            ),
        detail,
    }
}

#[cfg(all(test, target_os = "macos"))]
mod selection_confirmation_tests {
    use super::*;

    #[test]
    fn first_window_confirmation_is_not_an_expired_frame() {
        for code in [
            "preview_window_selection_required",
            "preview_selection_required",
        ] {
            let status = collector_status_policy("Failure", Some(code)).unwrap();
            assert!(status.state == PreviewState::Unavailable);
            assert_eq!(status.detail, "此窗口需要你通过系统选择对应的游戏窗口");
            assert!(status.clear_image);
            let capture = capture_failure_policy(code, false);
            assert!(capture.state == PreviewState::Unavailable);
            assert_eq!(capture.detail, status.detail);
            assert!(!capture.permission_denied);
            assert!(!capture.invalidate_pairing);
        }
    }

    #[test]
    fn recording_permission_and_valid_frames_keep_their_own_meaning() {
        let denied = collector_status_policy("Failure", Some("preview_permission_denied")).unwrap();
        assert!(denied.state == PreviewState::PermissionDenied);
        assert!(collector_status_policy("Live", None).is_none());
        assert!(collector_status_policy("CaptureReady", None).is_none());
        assert!(
            capture_failure_policy("preview_capture_timeout", true).state == PreviewState::Stale
        );
    }
}
