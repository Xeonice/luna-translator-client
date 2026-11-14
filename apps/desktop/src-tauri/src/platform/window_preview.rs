//! Window-isolated preview. One capture in flight and one latest frame per lease.
mod state;
use super::runtime_observation::{self, Fact, Id, ProjectionStage, Rejection};
use app_core::{
    contracts::window_capabilities::*,
    contracts::{AppError, AppResult},
    domain::content_hash,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "preview", message)
}
#[derive(Debug, Clone)]
pub struct VerifiedNativeWindow {
    pub host_pid: u32,
    pub host_process_start: String,
    pub window_id: u32,
}
#[derive(Clone)]
pub struct VerifiedPreviewTarget {
    pub candidate_id: String,
    pub target_identity_hash: String,
    pub native: VerifiedNativeWindow,
    pub pairing: Option<Arc<PreviewPairing>>,
}
pub struct PreviewPairing {
    #[cfg(test)]
    pub _token: String,
    #[cfg(test)]
    pub _keepalive: Arc<dyn Fn() -> AppResult<()> + Send + Sync>,
}
enum PreviewBacking {
    Pending,
    Failed,
    Resolved(VerifiedPreviewTarget),
}
#[cfg(target_os = "macos")]
pub(super) struct CollectorStatusUpdate<'a> {
    pub phase: &'a str,
    pub code: Option<&'a str>,
    pub permission: bool,
    pub observed_at_ms: u64,
    pub diagnostic: Option<&'a super::macos::PreviewDiagnostic>,
}
struct Lease {
    consumer: PreviewConsumer,
    request: PreviewStartRequest,
    backing: PreviewBacking,
    projection: PreviewProjection,
    frame: Option<PreviewFrame>,
    sequence: u64,
    #[cfg(test)]
    last_attempt: u64,
    stop_requested: bool,
    retirement_requested: bool,
    retiring_ticket: Option<PreviewLeaseTicket>,
    remote: bool,
    capture_id: Option<String>,
    #[cfg(target_os = "macos")]
    diagnostic: Option<super::macos::PreviewDiagnostic>,
}
impl Lease {
    fn public_projection(&self) -> PreviewProjection {
        #[cfg(target_os = "macos")]
        let diagnostic_summary = self
            .diagnostic
            .as_ref()
            .and_then(super::macos::PreviewDiagnostic::summary);
        #[cfg(not(target_os = "macos"))]
        let diagnostic_summary: Option<String> = None;
        state::projection_snapshot(&self.projection, diagnostic_summary.as_deref())
    }
    fn target(&self) -> Option<&VerifiedPreviewTarget> {
        if let PreviewBacking::Resolved(target) = &self.backing {
            Some(target)
        } else {
            None
        }
    }
}
struct Inner {
    generation: u64,
    permission_requested: bool,
    leases: BTreeMap<String, Lease>,
    deferred_retirements: Vec<PreviewLeaseTicket>,
    inflight: Option<String>,
    closed: bool,
}
/// An exact Native ownership ticket. It is never accepted from IPC or UI data.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct PreviewLeaseTicket {
    pub lease_id: String,
    pub generation: String,
    pub consumer: PreviewConsumer,
    pub consumer_id: String,
    pub candidate_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetirementAdmission {
    Released,
    Draining,
}
#[cfg(test)]
type ControlledRetirement = dyn Fn(&PreviewLeaseTicket) -> AppResult<bool> + Send + Sync;
#[cfg(test)]
type ControlledTransfer =
    dyn Fn(&str, &str, &str, &[(String, String)]) -> AppResult<()> + Send + Sync;
#[cfg(test)]
pub(crate) struct ControlledNativePrepared {
    pub target_identity_hash: String,
    pub capture_id: String,
}
#[cfg(test)]
type ControlledPrepare = dyn Fn(
        &str,
        &str,
        &app_infra::hook_backend::FrozenPreviewTarget,
        bool,
    ) -> AppResult<ControlledNativePrepared>
    + Send
    + Sync;
pub struct WindowPreviewService {
    inner: Mutex<Inner>,
    finished: Condvar,
    lifecycle: Mutex<Vec<mpsc::SyncSender<()>>>,
    #[cfg(test)]
    controlled_retirement: Mutex<Option<Arc<ControlledRetirement>>>,
    #[cfg(test)]
    controlled_transfer: Mutex<Option<Arc<ControlledTransfer>>>,
    #[cfg(test)]
    controlled_prepare: Mutex<Option<Arc<ControlledPrepare>>>,
    #[cfg(target_os = "macos")]
    collector: Mutex<Option<Arc<super::window_collector::CollectorSupervisor>>>,
}
impl WindowPreviewService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                generation: 0,
                permission_requested: false,
                leases: BTreeMap::new(),
                deferred_retirements: Vec::new(),
                inflight: None,
                closed: false,
            }),
            finished: Condvar::new(),
            lifecycle: Mutex::new(Vec::new()),
            #[cfg(test)]
            controlled_retirement: Mutex::new(None),
            #[cfg(test)]
            controlled_transfer: Mutex::new(None),
            #[cfg(test)]
            controlled_prepare: Mutex::new(None),
            #[cfg(target_os = "macos")]
            collector: Mutex::new(None),
        })
    }
    pub(crate) fn subscribe_lifecycle(&self, wake: mpsc::SyncSender<()>) -> AppResult<()> {
        let mut subscribers = self
            .lifecycle
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if subscribers.len() >= 16 {
            return Err(error("preview_subscriber_limit", "预览订阅已达到上限"));
        }
        subscribers.push(wake);
        Ok(())
    }
    fn notify_lifecycle(&self) {
        if let Ok(mut subscribers) = self.lifecycle.lock() {
            subscribers.retain(|wake| {
                !matches!(wake.try_send(()), Err(mpsc::TrySendError::Disconnected(_)))
            });
        }
    }
    fn ticket(id: &str, lease: &Lease) -> PreviewLeaseTicket {
        PreviewLeaseTicket {
            lease_id: id.into(),
            generation: lease.projection.generation.clone(),
            consumer: lease.consumer,
            consumer_id: lease.projection.consumer_id.clone(),
            candidate_id: lease.request.candidate_id.clone(),
        }
    }
    pub(crate) fn session_tickets(&self) -> AppResult<Vec<PreviewLeaseTicket>> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        Ok(state
            .leases
            .iter()
            .filter(|(_, lease)| lease.consumer == PreviewConsumer::Session)
            .map(|(id, lease)| Self::ticket(id, lease))
            .collect())
    }
    pub(crate) fn candidate_tickets(&self) -> AppResult<Vec<PreviewLeaseTicket>> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        Ok(state
            .leases
            .iter()
            .filter(|(_, lease)| {
                lease.consumer == PreviewConsumer::Candidate
                    && !lease.stop_requested
                    && !lease.retirement_requested
            })
            .map(|(id, lease)| Self::ticket(id, lease))
            .collect())
    }
    pub(crate) fn current_session_projection(
        &self,
        session: &str,
        candidate: &str,
    ) -> AppResult<Option<PreviewProjection>> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        Ok(state
            .leases
            .values()
            .find(|lease| {
                !lease.stop_requested
                    && !lease.retirement_requested
                    && lease.consumer == PreviewConsumer::Session
                    && lease.projection.consumer_id == session
                    && lease.request.candidate_id == candidate
            })
            .map(Lease::public_projection))
    }
    pub(crate) fn exact_candidate(&self, ticket: &PreviewLeaseTicket) -> bool {
        self.inner.lock().is_ok_and(|state| {
            state.leases.get(&ticket.lease_id).is_some_and(|lease| {
                !lease.stop_requested
                    && !lease.retirement_requested
                    && lease.consumer == PreviewConsumer::Candidate
                    && Self::ticket(&ticket.lease_id, lease) == *ticket
            })
        })
    }
    /// Unlike a UI pending slot, automatic current-session capture waits for
    /// actual obsolete-session release. A queued Stop is not a release grant.
    pub(crate) fn begin_current_session_resolution(
        &self,
        request: PreviewStartRequest,
    ) -> AppResult<PreviewProjection> {
        if request.consumer != PreviewConsumer::Session {
            return Err(error("preview_consumer", "当前会话预览身份无效"));
        }
        self.begin_resolution(request)
    }
    /// Mark and enqueue an exact old lease only. No acknowledgement or Stop
    /// wait is performed here, so a Core/Hook authority guard stays short.
    pub(crate) fn retire_exact(
        &self,
        ticket: &PreviewLeaseTicket,
    ) -> AppResult<RetirementAdmission> {
        self.retire_exact_with_public_fence(ticket, false)
            .map(|(admission, _)| admission)
    }
    /// A public Stop fence and its resource admission commit under this same
    /// lock. Counter exhaustion is rejected before any low-level Release.
    fn retire_exact_with_public_fence(
        &self,
        ticket: &PreviewLeaseTicket,
        public_fence: bool,
    ) -> AppResult<(RetirementAdmission, Option<String>)> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let Some(lease) = state.leases.get(&ticket.lease_id) else {
            return if public_fence {
                Err(error("preview_lease_expired", "预览已结束"))
            } else {
                Ok((RetirementAdmission::Released, None))
            };
        };
        if Self::ticket(&ticket.lease_id, lease) != *ticket {
            return Err(error(
                "preview_generation_conflict",
                "待释放的预览身份已变化",
            ));
        }
        if lease.stop_requested {
            // Private cleanup may already have admitted the source Release.
            // Its first public Stop still fences the UI once; repeated public
            // Stop observers neither redispatch Release nor advance it again.
            let needs_fence = public_fence
                && lease
                    .retiring_ticket
                    .as_ref()
                    .is_some_and(|original| original.generation == lease.projection.generation);
            let next = if needs_fence {
                Some(
                    state
                        .generation
                        .checked_add(1)
                        .ok_or_else(|| error("preview_generation_exhausted", "请重启客户端"))?,
                )
            } else {
                None
            };
            if let Some(next) = next {
                state.generation = next;
                state
                    .leases
                    .get_mut(&ticket.lease_id)
                    .expect("checked retiring lease")
                    .projection
                    .generation = next.to_string();
            }
            return Ok((
                RetirementAdmission::Draining,
                next.map(|next| next.to_string()),
            ));
        }
        // One source may drain at a time. Reject before touching either the
        // parent collector or the service generation of this second owner.
        if state
            .leases
            .iter()
            .any(|(id, lease)| id != &ticket.lease_id && lease.stop_requested)
        {
            let changed = Self::defer_retirement(&mut state, ticket, true);
            drop(state);
            if changed {
                self.notify_lifecycle();
            }
            return Err(error(
                "preview_cleanup_pending",
                "另一采集仍在释放，已排队释放当前采集",
            ));
        }
        let next = if public_fence {
            Some(
                state
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| error("preview_generation_exhausted", "请重启客户端"))?,
            )
        } else {
            None
        };
        let remote = lease.remote;
        let attempted = (|| -> AppResult<bool> {
            #[cfg(test)]
            if let Some(retire) = self
                .controlled_retirement
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?
                .clone()
            {
                retire(ticket)
            } else {
                self.enqueue_retirement(
                    ticket,
                    remote,
                    state.inflight.as_deref() == Some(ticket.lease_id.as_str()),
                )
            }
            #[cfg(not(test))]
            self.enqueue_retirement(
                ticket,
                remote,
                state.inflight.as_deref() == Some(ticket.lease_id.as_str()),
            )
        })();
        let immediate = match attempted {
            Ok(immediate) => immediate,
            Err(error) => {
                let changed = Self::defer_retirement(&mut state, ticket, false);
                drop(state);
                if changed {
                    self.notify_lifecycle();
                }
                return Err(error);
            }
        };
        state
            .deferred_retirements
            .retain(|deferred| deferred != ticket);
        if let Some(next) = next {
            state.generation = next;
        }
        if immediate {
            state.leases.remove(&ticket.lease_id);
        } else {
            let lease = state
                .leases
                .get_mut(&ticket.lease_id)
                .expect("exact lease retained");
            lease.stop_requested = true;
            lease.retirement_requested = true;
            lease.retiring_ticket = Some(ticket.clone());
            if let Some(next) = next {
                lease.projection.generation = next.to_string();
            }
            lease.frame = None;
            lease.projection.state = PreviewState::Unavailable;
            lease.projection.error_code = Some("preview_cleanup_pending".into());
            lease.projection.detail = "正在释放原游戏采集，等待确认后接续当前会话".into();
        }
        drop(state);
        self.notify_lifecycle();
        Ok((
            if immediate {
                RetirementAdmission::Released
            } else {
                RetirementAdmission::Draining
            },
            next.map(|next| next.to_string()),
        ))
    }
    fn defer_retirement(state: &mut Inner, ticket: &PreviewLeaseTicket, freeze: bool) -> bool {
        state.deferred_retirements.retain(|old| {
            state.leases.get(&old.lease_id).is_some_and(|lease| {
                Self::ticket(&old.lease_id, lease) == *old
                    && lease.retirement_requested
                    && !lease.stop_requested
            })
        });
        let changed = !state.deferred_retirements.contains(ticket);
        if changed && state.deferred_retirements.len() < 3 {
            state.deferred_retirements.push(ticket.clone());
        }
        if let Some(lease) = state
            .leases
            .get_mut(&ticket.lease_id)
            .filter(|lease| Self::ticket(&ticket.lease_id, lease) == *ticket)
        {
            lease.retirement_requested = true;
            if freeze {
                lease.frame = None;
                lease.projection.state = PreviewState::Unavailable;
                lease.projection.error_code = Some("preview_cleanup_pending".into());
                lease.projection.detail = "已排队释放窗口采集".into();
            }
        }
        changed
    }
    /// Runs only bounded metadata/queue work. Actual acknowledgement wakes the
    /// same driver, and the independent Native owner retries queue pressure even
    /// after the Candidate UI has gone away. Exact tickets cannot retire a new generation.
    pub(crate) fn drive_deferred_retirements(&self) -> AppResult<bool> {
        let tickets = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .deferred_retirements
            .clone();
        for ticket in tickets {
            let current = {
                let mut state = self
                    .inner
                    .lock()
                    .map_err(|_| error("preview_state", "预览状态不可用"))?;
                let current = state.leases.get(&ticket.lease_id).is_some_and(|lease| {
                    Self::ticket(&ticket.lease_id, lease) == ticket && lease.retirement_requested
                });
                if !current {
                    state.deferred_retirements.retain(|old| old != &ticket);
                }
                current
            };
            if current {
                match self.retire_exact(&ticket) {
                    Ok(RetirementAdmission::Released) => {}
                    Ok(RetirementAdmission::Draining) => return Ok(true),
                    Err(error)
                        if matches!(
                            error.code.as_str(),
                            "preview_cleanup_pending"
                                | "preview_capture_busy"
                                | "capture_cleanup_pending"
                        ) =>
                    {
                        return Ok(true);
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(!self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .deferred_retirements
            .is_empty())
    }
    fn enqueue_retirement(
        &self,
        ticket: &PreviewLeaseTicket,
        remote: bool,
        inflight: bool,
    ) -> AppResult<bool> {
        #[cfg(target_os = "macos")]
        if remote {
            return self
                .collector()?
                .release(&ticket.lease_id, &ticket.generation);
        }
        let _ = (ticket, remote);
        Ok(!inflight)
    }
    /// A retry cannot manufacture release: only an exact acknowledged ticket
    /// removes its stopped lease. Duplicate/late acknowledgements are no-ops.
    fn retirement_acknowledged(&self, ticket: &PreviewLeaseTicket) -> bool {
        let removed = if let Ok(mut state) = self.inner.lock() {
            if state.leases.get(&ticket.lease_id).is_some_and(|lease| {
                lease.stop_requested && lease.retiring_ticket.as_ref() == Some(ticket)
            }) {
                state.leases.remove(&ticket.lease_id);
                true
            } else {
                false
            }
        } else {
            false
        };
        if removed {
            self.finished.notify_all();
            self.notify_lifecycle();
            let _ = self.drive_deferred_retirements();
        }
        removed
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.inner.lock().map(|state| state.closed).unwrap_or(true)
    }
    pub fn capability(&self) -> PreviewCapability {
        #[cfg(target_os = "macos")]
        {
            let supported = super::macos::preview_supported();
            let permission = if self
                .collector_permission()
                .unwrap_or_else(|| super::macos::preview_permission(false))
            {
                PreviewPermission::Granted
            } else if self
                .inner
                .lock()
                .map(|s| s.permission_requested)
                .unwrap_or(false)
            {
                PreviewPermission::Denied
            } else {
                PreviewPermission::NotRequested
            };
            PreviewCapability {
                supported,
                permission,
                error_code: if supported {
                    None
                } else {
                    Some("preview_unsupported".into())
                },
                detail: if supported {
                    "自动预览已核验游戏窗口；不含音频，不上传。无本机封面时仅保存正式关联后的首张有效画面作为封面".into()
                } else {
                    "当前系统不支持窗口预览，需要 macOS 14 或更新版本；取文不受影响".into()
                },
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            PreviewCapability {
                supported: false,
                permission: PreviewPermission::Unavailable,
                error_code: Some("preview_unsupported".into()),
                detail: "当前平台尚不支持窗口预览，取文不受影响".into(),
            }
        }
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn install_collector(
        &self,
        collector: Arc<super::window_collector::CollectorSupervisor>,
    ) -> AppResult<()> {
        *self
            .collector
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))? = Some(collector);
        Ok(())
    }
    #[cfg(target_os = "macos")]
    fn collector_permission(&self) -> Option<bool> {
        self.inner.lock().ok()?.leases.values().find_map(|l| {
            l.remote
                .then_some(l.projection.permission == PreviewPermission::Granted)
        })
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn collector(&self) -> AppResult<Arc<super::window_collector::CollectorSupervisor>> {
        self.collector
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .clone()
            .ok_or_else(|| error("preview_worker_unavailable", "独立采集进程尚未就绪"))
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn start_remote(
        &self,
        projection: &PreviewProjection,
        target: app_infra::hook_backend::FrozenPreviewTarget,
        select: bool,
    ) -> AppResult<()> {
        let id = projection
            .lease_id
            .as_deref()
            .ok_or_else(|| error("preview_state", "缺少预览句柄"))?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if state.closed {
            return Err(error("preview_closed", "预览服务已停止"));
        }
        let lease = state
            .leases
            .get_mut(id)
            .filter(|l| {
                l.projection.generation == projection.generation
                    && !l.stop_requested
                    && !l.retirement_requested
            })
            .ok_or_else(|| error("preview_generation_conflict", "预览目标已变化"))?;
        if target.candidate_id() != lease.request.candidate_id {
            return Err(error("preview_target_identity", "冻结窗口与当前目标不一致"));
        }
        #[cfg(test)]
        let prepared = if let Some(prepare) = self
            .controlled_prepare
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .clone()
        {
            Some(prepare(id, &projection.generation, &target, select)?)
        } else {
            self.collector()?
                .prepare(id, &projection.generation, target, select)?;
            None
        };
        #[cfg(not(test))]
        self.collector()?
            .prepare(id, &projection.generation, target, select)?;
        lease.remote = true;
        #[cfg(test)]
        if let Some(prepared) = prepared {
            lease.capture_id = Some(prepared.capture_id);
            lease.backing = PreviewBacking::Resolved(VerifiedPreviewTarget {
                candidate_id: lease.request.candidate_id.clone(),
                target_identity_hash: prepared.target_identity_hash.clone(),
                native: VerifiedNativeWindow {
                    host_pid: std::process::id(),
                    host_process_start: "controlled-collector-prepare-port".into(),
                    window_id: 1,
                },
                pairing: None,
            });
            lease.projection.target_identity_hash = Some(prepared.target_identity_hash);
        }
        lease.projection.state = PreviewState::Starting;
        lease.projection.error_code = None;
        lease.projection.detail = "独立采集进程正在核对游戏窗口".into();
        state.permission_requested = true;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    pub(super) fn collector_status(
        &self,
        id: &str,
        generation: &str,
        update: CollectorStatusUpdate<'_>,
    ) {
        let CollectorStatusUpdate {
            phase,
            code,
            permission,
            observed_at_ms: at,
            diagnostic,
        } = update;
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        let Some(lease) = state.leases.get_mut(id).filter(|l| {
            l.remote
                && l.projection.generation == generation
                && !l.stop_requested
                && !l.retirement_requested
        }) else {
            return;
        };
        if let Some(value) = diagnostic
            .cloned()
            .and_then(super::macos::PreviewDiagnostic::checked)
        {
            lease.diagnostic = Some(value);
        }
        lease.projection.permission = if permission {
            PreviewPermission::Granted
        } else {
            PreviewPermission::Denied
        };
        lease.projection.permission_checked_at_ms = Some(at.to_string());
        let Some(decision) = state::collector_status_policy(phase, code) else {
            return;
        };
        lease.projection.state = decision.state;
        lease.projection.error_code = code.map(str::to_owned);
        lease.projection.detail = decision.detail.into();
        if decision.clear_image {
            lease.frame = None;
            lease.projection.captured_at_ms = None;
            lease.projection.content_changed_at_ms = None;
        }
        let fact = Fact::projection(
            ProjectionStage::ServiceStatus,
            &lease.projection,
            lease.frame.as_ref(),
            None,
        );
        drop(state);
        runtime_observation::record(fact);
        self.notify_lifecycle();
    }
    #[cfg(target_os = "macos")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn collector_frame(
        &self,
        id: &str,
        generation: &str,
        candidate: &str,
        capture: &str,
        worker_sequence: u64,
        pid: u32,
        start: &str,
        window: u32,
        frame: super::macos::CapturedFrame,
    ) {
        let observe_admission = |accepted| {
            (|| {
                Some(Fact::RemoteFrameAdmission {
                    accepted,
                    lease_id: Id::new(id)?,
                    generation: Id::new(generation)?,
                    capture_id: Id::new(capture)?,
                    worker_sequence,
                    target_candidate_id: Id::new(candidate)?,
                    host_pid: pid,
                    host_start: Id::new(start)?,
                    window_id: window,
                    captured_at_ms: frame.captured_at_ms,
                })
            })()
        };
        let target = {
            let Ok(mut state) = self.inner.lock() else {
                return;
            };
            let Some(lease) = state.leases.get_mut(id).filter(|lease| {
                state::remote_frame_admitted(
                    state::RemoteFrameLease {
                        remote: lease.remote,
                        generation: &lease.projection.generation,
                        stop_requested: lease.stop_requested || lease.retirement_requested,
                        state: &lease.projection.state,
                        candidate: &lease.request.candidate_id,
                        capture_id: lease.capture_id.as_deref(),
                        target: lease.target().map(|target| state::NativeIdentity {
                            candidate: &target.candidate_id,
                            pid: target.native.host_pid,
                            process_start: &target.native.host_process_start,
                            window: target.native.window_id,
                        }),
                    },
                    state::RemoteFrameObservation {
                        generation,
                        capture_id: capture,
                        native: state::NativeIdentity {
                            candidate,
                            pid,
                            process_start: start,
                            window,
                        },
                    },
                )
            }) else {
                drop(state);
                runtime_observation::record(observe_admission(false));
                return;
            };
            let target = VerifiedPreviewTarget {
                candidate_id: candidate.into(),
                target_identity_hash: content_hash(format!(
                    "{candidate}\0{pid}\0{start}\0{window}"
                )),
                native: VerifiedNativeWindow {
                    host_pid: pid,
                    host_process_start: start.into(),
                    window_id: window,
                },
                pairing: None,
            };
            lease.projection.target_identity_hash = Some(target.target_identity_hash.clone());
            lease.capture_id = Some(capture.into());
            lease.backing = PreviewBacking::Resolved(target.clone());
            target
        };
        runtime_observation::record(observe_admission(true));
        self.finish_capture(id.into(), generation.into(), target, Ok(frame));
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn collector_diagnostic(
        &self,
        id: &str,
        generation: &str,
        diagnostic: super::macos::PreviewDiagnostic,
    ) {
        let Some(diagnostic) = diagnostic.checked() else {
            return;
        };
        if let Ok(mut state) = self.inner.lock()
            && let Some(lease) = state.leases.get_mut(id).filter(|lease| {
                lease.remote
                    && lease.projection.generation == generation
                    && !lease.stop_requested
                    && !lease.retirement_requested
            })
        {
            lease.diagnostic = Some(diagnostic);
        }
    }
    /// Only user target selection/association may initiate the first OS prompt.
    #[cfg(target_os = "macos")]
    pub(crate) fn collector_released(&self, id: &str) {
        // CollectorSupervisor has already matched the actual Released source
        // epoch before this private callback. Lease IDs are never recycled.
        let ticket = self.inner.lock().ok().and_then(|state| {
            state
                .leases
                .get(id)
                .filter(|lease| lease.remote && lease.stop_requested)
                .and_then(|lease| lease.retiring_ticket.clone())
        });
        let removed = ticket
            .as_ref()
            .is_some_and(|ticket| self.retirement_acknowledged(ticket));
        self.finished.notify_all();
        runtime_observation::record(
            Id::new(id).map(|lease_id| Fact::ServiceReleased { lease_id, removed }),
        );
    }
    pub fn consumer_update(
        &self,
        consumer_id: &str,
        include_frame: bool,
    ) -> AppResult<Option<PreviewUpdate>> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        Ok(state
            .leases
            .values()
            .find(|lease| lease.projection.consumer_id == consumer_id && !lease.stop_requested)
            .map(|lease| PreviewUpdate {
                projection: lease.public_projection(),
                frame: if include_frame {
                    lease.frame.clone()
                } else {
                    None
                },
            }))
    }
    /// A logical control handle is published even when no native capture could
    /// be created. Pending/Failed slots can never yield a frame or a cover.
    pub fn begin_resolution(&self, request: PreviewStartRequest) -> AppResult<PreviewProjection> {
        if request.consumer_id.is_empty() || request.consumer_id.len() > 128 {
            return Err(error("preview_consumer", "预览使用方无效"));
        }
        let capability = self.capability();
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if state.closed {
            return Err(error("preview_closed", "预览服务已停止"));
        }
        let previous = state
            .leases
            .iter()
            .find(|(_, l)| l.projection.consumer_id == request.consumer_id)
            .map(|(id, l)| (id.clone(), l.projection.generation.clone()));
        if let Some((id, _)) = &previous {
            let lease = &state.leases[id];
            if lease.stop_requested || lease.retirement_requested {
                return Err(error("preview_cleanup_pending", "原采集进程仍在收尾"));
            }
            if lease.request.candidate_id != request.candidate_id
                && (lease.remote || state.inflight.as_deref() == Some(id.as_str()))
            {
                return Err(error(
                    "preview_cleanup_pending",
                    "原目标采集尚未释放，请先结束原预览",
                ));
            }
            if request.expected_generation.is_none()
                && lease.request.candidate_id == request.candidate_id
                && lease.request.consumer == request.consumer
            {
                return Ok(lease.projection.clone());
            }
        }
        if request
            .expected_generation
            .as_deref()
            .is_some_and(|g| previous.as_ref().map(|(_, v)| v.as_str()) != Some(g))
        {
            return Err(error("preview_generation_conflict", "预览目标已变化"));
        }
        if request.consumer == PreviewConsumer::Session
            && state.leases.iter().any(|(id, lease)| {
                lease.consumer == PreviewConsumer::Session
                    && previous.as_ref().is_none_or(|(previous, _)| previous != id)
            })
        {
            return Err(error(
                "preview_cleanup_pending",
                "原会话采集尚未释放，稍后自动接续",
            ));
        }
        let active = state
            .leases
            .values()
            .filter(|lease| !lease.stop_requested)
            .count();
        let draining = state
            .leases
            .values()
            .filter(|lease| lease.stop_requested)
            .count();
        if !state::lease_budget_admits(active, draining, usize::from(previous.is_none())) {
            return Err(error(
                "preview_lease_limit",
                "已有两个预览使用方及一个待释放采集",
            ));
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| error("preview_generation_exhausted", "请重启客户端"))?;
        let generation = state.generation.to_string();
        let id = previous
            .map(|(id, _)| id)
            .unwrap_or_else(|| format!("preview-{}-{generation}", std::process::id()));
        let projection = PreviewProjection {
            lease_id: Some(id.clone()),
            generation,
            consumer_id: request.consumer_id.clone(),
            state: PreviewState::Starting,
            permission: capability.permission,
            target_identity_hash: None,
            attempted_at_ms: Some(now().to_string()),
            captured_at_ms: None,
            content_changed_at_ms: None,
            permission_checked_at_ms: Some(now().to_string()),
            error_code: None,
            detail: "正在核对预览窗口".into(),
        };
        state.leases.insert(
            id,
            Lease {
                consumer: request.consumer,
                request,
                backing: PreviewBacking::Pending,
                projection: projection.clone(),
                frame: None,
                sequence: 0,
                #[cfg(test)]
                last_attempt: 0,
                stop_requested: false,
                retirement_requested: false,
                retiring_ticket: None,
                remote: false,
                capture_id: None,
                #[cfg(target_os = "macos")]
                diagnostic: None,
            },
        );
        Ok(projection)
    }
    #[cfg(test)]
    pub fn is_current(&self, id: &str, generation: &str) -> bool {
        self.inner
            .lock()
            .map(|s| {
                !s.closed
                    && s.leases.get(id).is_some_and(|l| {
                        !l.stop_requested
                            && !l.retirement_requested
                            && l.projection.generation == generation
                    })
            })
            .unwrap_or(false)
    }
    pub fn has_resolved(&self, id: &str, generation: &str) -> bool {
        self.inner
            .lock()
            .map(|s| {
                s.leases
                    .get(id)
                    .is_some_and(|l| l.projection.generation == generation && l.target().is_some())
            })
            .unwrap_or(false)
    }
    pub fn request_matches(
        &self,
        id: &str,
        generation: &str,
        request: &PreviewStartRequest,
    ) -> bool {
        self.inner
            .lock()
            .map(|s| {
                s.leases.get(id).is_some_and(|l| {
                    !l.stop_requested
                        && !l.retirement_requested
                        && l.projection.generation == generation
                        && l.request.candidate_id == request.candidate_id
                        && l.request.consumer == request.consumer
                        && l.request.consumer_id == request.consumer_id
                })
            })
            .unwrap_or(false)
    }
    pub fn transferable(&self, id: &str, generation: &str, candidate: &str) -> bool {
        self.inner
            .lock()
            .map(|state| {
                state.leases.get(id).is_some_and(|l| {
                    !l.stop_requested
                        && !l.retirement_requested
                        && l.consumer == PreviewConsumer::Candidate
                        && l.projection.generation == generation
                        && l.request.candidate_id == candidate
                        && (l.remote || l.target().is_some_and(|t| t.pairing.is_some()))
                })
            })
            .unwrap_or(false)
    }
    #[cfg(test)]
    pub fn resolved_pairing(
        &self,
        id: &str,
        generation: &str,
        candidate: &str,
    ) -> Option<Arc<PreviewPairing>> {
        let state = self.inner.lock().ok()?;
        let lease = state.leases.get(id)?;
        if lease.stop_requested
            || lease.consumer != PreviewConsumer::Candidate
            || lease.projection.generation != generation
        {
            return None;
        }
        let target = lease.target()?;
        (target.candidate_id == candidate)
            .then(|| target.pairing.clone())
            .flatten()
    }
    #[cfg(test)]
    pub fn session_matches_candidate(&self, session: &str, candidate: &str) -> bool {
        self.inner
            .lock()
            .map(|s| {
                s.leases.values().any(|l| {
                    !l.stop_requested
                        && !l.retirement_requested
                        && l.consumer == PreviewConsumer::Session
                        && l.projection.consumer_id == session
                        && l.target().is_some_and(|t| t.candidate_id == candidate)
                })
            })
            .unwrap_or(false)
    }
    /// Move a reviewed in-memory pairing, not a saved native ID. Both the owner
    /// and this exact generation are checked by the application coordinator.
    pub fn transfer_pairing_to_session(
        &self,
        id: &str,
        generation: &str,
        candidate: &str,
        session: &str,
    ) -> AppResult<PreviewProjection> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if state.closed {
            return Err(error("preview_closed", "预览服务已停止"));
        }
        let lease = state
            .leases
            .get(id)
            .filter(|l| {
                !l.stop_requested
                    && !l.retirement_requested
                    && l.consumer == PreviewConsumer::Candidate
                    && l.projection.generation == generation
                    && l.request.candidate_id == candidate
                    && (l.remote || l.target().is_some_and(|t| t.pairing.is_some()))
            })
            .ok_or_else(|| error("preview_generation_conflict", "候选预览已变化"))?;
        let paused = lease.projection.state == PreviewState::Paused;
        let next = state
            .generation
            .checked_add(1)
            .ok_or_else(|| error("preview_generation_exhausted", "请重启客户端"))?;
        // Queued retirement is not released capacity. The current-session
        // owner retires exact old leases first and retries only after their
        // acknowledgement. Transferring a candidate must never silently drop
        // another Session owner while its collector still owns a source.
        if state
            .leases
            .iter()
            .any(|(key, lease)| key != id && lease.consumer == PreviewConsumer::Session)
        {
            return Err(error(
                "preview_cleanup_pending",
                "原会话采集尚未释放，稍后自动接续",
            ));
        }
        #[cfg(target_os = "macos")]
        let displaced: Vec<(String, String)> = Vec::new();
        #[cfg(target_os = "macos")]
        if state.leases.get(id).is_some_and(|l| l.remote) {
            #[cfg(test)]
            if let Some(transfer) = self
                .controlled_transfer
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?
                .clone()
            {
                transfer(id, generation, &next.to_string(), &displaced)?;
            } else {
                self.collector()?.transfer(
                    id,
                    generation,
                    &next.to_string(),
                    paused,
                    &displaced,
                )?;
            }
            #[cfg(not(test))]
            self.collector()?
                .transfer(id, generation, &next.to_string(), paused, &displaced)?;
        }
        state.generation = next;
        let lease = state.leases.get_mut(id).expect("verified lease retained");
        lease.consumer = PreviewConsumer::Session;
        lease.request.consumer = PreviewConsumer::Session;
        lease.request.consumer_id = session.into();
        lease.request.expected_generation = Some(next.to_string());
        lease.projection.consumer_id = session.into();
        lease.projection.generation = next.to_string();
        #[cfg(target_os = "macos")]
        {
            lease.diagnostic = None;
        }
        lease.projection.state = if paused {
            PreviewState::Paused
        } else {
            PreviewState::Starting
        };
        lease.projection.error_code = None;
        lease.projection.detail = "已接续核对过的游戏窗口，正在采集新画面".into();
        lease.projection.captured_at_ms = None;
        lease.projection.content_changed_at_ms = None;
        lease.frame = None;
        lease.sequence = 0;
        #[cfg(test)]
        {
            lease.last_attempt = 0;
        }
        let result = lease.projection.clone();
        drop(state);
        Ok(result)
    }
    #[cfg(test)]
    pub fn retry_request(
        &self,
        id: &str,
        generation: &str,
    ) -> AppResult<Option<PreviewStartRequest>> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let lease = state
            .leases
            .get(id)
            .filter(|l| {
                l.projection.generation == generation
                    && !l.stop_requested
                    && !l.retirement_requested
            })
            .ok_or_else(|| error("preview_generation_conflict", "预览目标已变化"))?;
        Ok(matches!(lease.backing, PreviewBacking::Failed).then(|| {
            let mut request = lease.request.clone();
            request.expected_generation = Some(generation.into());
            request
        }))
    }
    pub fn fail_resolution(
        &self,
        id: &str,
        generation: &str,
        failure: AppError,
    ) -> AppResult<PreviewProjection> {
        let capability = self.capability();
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let lease = state
            .leases
            .get_mut(id)
            .filter(|l| {
                l.projection.generation == generation
                    && !l.stop_requested
                    && !l.retirement_requested
            })
            .ok_or_else(|| error("preview_cancelled", "预览目标已变化"))?;
        lease.projection.permission = capability.permission;
        lease.backing = PreviewBacking::Failed;
        lease.frame = None;
        lease.projection.state = if failure.code == "preview_permission_denied" {
            PreviewState::PermissionDenied
        } else if failure.code == "preview_target_closed" {
            PreviewState::TargetClosed
        } else {
            PreviewState::Unavailable
        };
        lease.projection.error_code = Some(failure.code);
        lease.projection.detail = failure.message;
        lease.projection.captured_at_ms = None;
        lease.projection.content_changed_at_ms = None;
        lease.projection.target_identity_hash = None;
        Ok(lease.projection.clone())
    }
    #[cfg(test)]
    pub fn start(
        &self,
        target: VerifiedPreviewTarget,
        request: PreviewStartRequest,
    ) -> AppResult<PreviewProjection> {
        let capability = self.capability();
        if !capability.supported {
            return Err(error("preview_unsupported", &capability.detail));
        }
        if capability.permission != PreviewPermission::Granted {
            return Err(error(
                "preview_permission_denied",
                "窗口预览需要录屏权限，游戏取文不受影响",
            ));
        }
        if request.candidate_id != target.candidate_id
            || target.native.window_id == 0
            || target.native.host_pid == 0
            || target.native.host_process_start.is_empty()
            || target.target_identity_hash.is_empty()
        {
            return Err(error("preview_target_identity", "窗口身份无法核验"));
        }
        let pending = {
            let state = self
                .inner
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?;
            state
                .leases
                .values()
                .find(|l| {
                    l.projection.consumer_id == request.consumer_id
                        && matches!(l.backing, PreviewBacking::Pending)
                        && request.expected_generation.as_deref()
                            == Some(l.projection.generation.as_str())
                })
                .map(|l| l.projection.clone())
        };
        let projection = match pending {
            Some(p) => p,
            None => self.begin_resolution(request)?,
        };
        let id = projection
            .lease_id
            .as_deref()
            .ok_or_else(|| error("preview_state", "缺少控制句柄"))?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let lease = state
            .leases
            .get_mut(id)
            .filter(|l| {
                l.projection.generation == projection.generation
                    && !l.stop_requested
                    && !l.retirement_requested
            })
            .ok_or_else(|| error("preview_cancelled", "预览已取消"))?;
        lease.projection.target_identity_hash = Some(target.target_identity_hash.clone());
        lease.projection.permission = capability.permission;
        lease.projection.detail = if target.pairing.is_some() {
            "正在采集你选择并核对的游戏窗口"
        } else {
            "正在采集已核验的游戏窗口"
        }
        .into();
        lease.backing = PreviewBacking::Resolved(target);
        Ok(lease.projection.clone())
    }
    /// A window-scoped pull consumes no unbounded transport queue. The bridge can
    /// expose this as one-outstanding-request subscription and call onUpdate.
    pub fn latest_update(
        &self,
        lease_id: &str,
        generation: &str,
        after_sequence: Option<&str>,
    ) -> AppResult<PreviewUpdate> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let lease = state
            .leases
            .get(lease_id)
            .ok_or_else(|| error("preview_lease_expired", "窗口预览已结束"))?;
        if lease.projection.generation != generation {
            return Err(error("preview_generation_conflict", "预览目标已变化"));
        }
        Ok(PreviewUpdate {
            projection: lease.public_projection(),
            frame: lease
                .frame
                .as_ref()
                .filter(|f| Some(f.sequence.as_str()) != after_sequence)
                .cloned(),
        })
    }
    /// Only the final short commit runs under this lease; decoding and file preparation run before it.
    pub fn commit_current_session_frame<T>(
        &self,
        session_id: &str,
        frame: &PreviewFrame,
        commit: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        let remote = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .leases
            .get(&frame.lease_id)
            .is_some_and(|l| l.remote);
        if remote {
            return self.commit_current_session_frame_checked(session_id, frame, |_| true, commit);
        }
        self.commit_current_session_frame_checked(session_id, frame, |_| false, commit)
    }
    fn commit_current_session_frame_checked<T>(
        &self,
        session_id: &str,
        frame: &PreviewFrame,
        native_current: impl FnOnce(&VerifiedPreviewTarget) -> bool,
        commit: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let lease = state
            .leases
            .get(&frame.lease_id)
            .ok_or_else(|| error("game_cover_preview_stale", "采集租约已结束"))?;
        if lease.consumer != PreviewConsumer::Session
            || lease.stop_requested
            || lease.projection.consumer_id != session_id
            || lease.projection.generation != frame.generation
            || lease.projection.state != PreviewState::Live
            || lease
                .target()
                .is_none_or(|t| t.target_identity_hash != frame.target_identity_hash)
            || lease.projection.target_identity_hash.as_deref()
                != Some(frame.target_identity_hash.as_str())
        {
            return Err(error(
                "game_cover_preview_stale",
                "采集目标或代次已变化，旧画面未设为封面",
            ));
        }
        if !lease.target().is_some_and(native_current) {
            return Err(error("game_cover_preview_stale", "原生窗口身份已变化"));
        }
        commit()
    }
    pub fn control(&self, request: PreviewControlRequest) -> AppResult<PreviewProjection> {
        let action = request.action.clone();
        let result = self.control_inner(request);
        if let Ok(projection) = &result {
            runtime_observation::record(Fact::projection(
                ProjectionStage::ControlResult,
                projection,
                None,
                Some(action),
            ));
            self.notify_lifecycle();
        }
        result
    }
    fn control_inner(&self, request: PreviewControlRequest) -> AppResult<PreviewProjection> {
        if request.action == PreviewControl::Stop {
            return self.stop_exact_request(request);
        }
        #[cfg(target_os = "macos")]
        if self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .leases
            .get(&request.lease_id)
            .is_some_and(|l| l.remote)
        {
            return self.control_remote(request);
        }

        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if state.closed {
            return Err(error("preview_closed", "预览服务已停止"));
        }
        let lease = state
            .leases
            .get(&request.lease_id)
            .ok_or_else(|| error("preview_lease_expired", "预览已结束"))?;
        if lease.projection.generation != request.generation {
            return Err(error("preview_generation_conflict", "预览目标已变化"));
        }
        if request.action == PreviewControl::Resume
            && !matches!(lease.backing, PreviewBacking::Resolved(_))
        {
            return Err(error("preview_resolution_required", "请重新核对预览窗口"));
        }
        state.deferred_retirements.retain(|ticket| {
            ticket.lease_id != request.lease_id || ticket.generation != request.generation
        });
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| error("preview_generation_exhausted", "请重启客户端"))?;
        let generation = state.generation.to_string();
        let lease = state.leases.get_mut(&request.lease_id).unwrap();
        lease.retirement_requested = false;
        lease.projection.generation = generation;
        #[cfg(target_os = "macos")]
        {
            lease.diagnostic = None;
        }
        lease.frame = if request.action == PreviewControl::Pause {
            // The frozen image remains readable by a new UI subscriber. Moving
            // its transport generation does not create a capture or change any
            // source time/sequence/content; old in-flight callbacks still fail
            // the generation check in finish_capture.
            lease
                .frame
                .take()
                .filter(|frame| {
                    frame.lease_id == request.lease_id
                        && frame.generation == request.generation
                        && lease
                            .target()
                            .is_some_and(|t| frame.target_identity_hash == t.target_identity_hash)
                        && Some(frame.target_identity_hash.as_str())
                            == lease.projection.target_identity_hash.as_deref()
                })
                .map(|mut frame| {
                    frame.generation = lease.projection.generation.clone();
                    frame
                })
        } else {
            None
        };
        match request.action {
            PreviewControl::Pause => {
                lease.projection.state = PreviewState::Paused;
                lease.projection.detail = "预览已暂停，最后帧保留原采集时间".into();
            }
            PreviewControl::Resume => {
                lease.projection.state = PreviewState::Starting;
                lease.projection.error_code = None;
                lease.projection.detail = "正在恢复窗口预览".into();
            }
            PreviewControl::Stop => unreachable!("Stop uses exact retirement admission"),
        }
        Ok(lease.projection.clone())
    }
    #[cfg(target_os = "macos")]
    fn control_remote(&self, request: PreviewControlRequest) -> AppResult<PreviewProjection> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if state.closed {
            return Err(error("preview_closed", "预览服务已停止"));
        }
        let lease = state
            .leases
            .get(&request.lease_id)
            .filter(|l| l.projection.generation == request.generation && !l.stop_requested)
            .ok_or_else(|| error("preview_generation_conflict", "预览目标已变化"))?;
        let _ = lease;
        let next = state
            .generation
            .checked_add(1)
            .ok_or_else(|| error("preview_generation_exhausted", "请重启客户端"))?;
        // Stop is routed through the same exact retirement admission as every
        // other caller before this method. Pause/resume only fence this source.
        #[cfg(test)]
        if let Some(fence) = self
            .controlled_transfer
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .clone()
        {
            fence(
                &request.lease_id,
                &request.generation,
                &next.to_string(),
                &[],
            )?;
        } else {
            self.collector()?.fence(
                &request.lease_id,
                &request.generation,
                &next.to_string(),
                request.action == PreviewControl::Pause,
            )?;
        }
        #[cfg(not(test))]
        self.collector()?.fence(
            &request.lease_id,
            &request.generation,
            &next.to_string(),
            request.action == PreviewControl::Pause,
        )?;
        state.deferred_retirements.retain(|ticket| {
            ticket.lease_id != request.lease_id || ticket.generation != request.generation
        });
        state.generation = next;
        let lease = state
            .leases
            .get_mut(&request.lease_id)
            .expect("checked lease");
        lease.retirement_requested = false;
        lease.projection.generation = next.to_string();
        #[cfg(target_os = "macos")]
        {
            lease.diagnostic = None;
        }
        lease.projection.error_code = None;
        if request.action == PreviewControl::Pause {
            if let Some(frame) = lease.frame.as_mut() {
                frame.generation = next.to_string();
            }
            lease.projection.state = PreviewState::Paused;
        } else {
            lease.frame = None;
            lease.projection.captured_at_ms = None;
            lease.projection.content_changed_at_ms = None;
            lease.projection.state = PreviewState::Starting;
        }
        Ok(lease.projection.clone())
    }
    fn stop_exact_request(&self, request: PreviewControlRequest) -> AppResult<PreviewProjection> {
        let (mut ticket, mut projection, remote) = {
            let state = self
                .inner
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?;
            let lease = state
                .leases
                .get(&request.lease_id)
                .ok_or_else(|| error("preview_lease_expired", "预览已结束"))?;
            if lease.projection.generation != request.generation {
                return Err(error("preview_generation_conflict", "预览目标已变化"));
            }
            (
                Self::ticket(&request.lease_id, lease),
                lease.public_projection(),
                lease.remote,
            )
        };
        let (admission, generation) = self.retire_exact_with_public_fence(&ticket, true)?;
        if let Some(generation) = generation {
            projection.generation = generation.clone();
            ticket.generation = generation;
        }
        if admission == RetirementAdmission::Released {
            projection.state = PreviewState::Stopped;
            return Ok(projection);
        }
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let (mut state, timeout) = self
            .finished
            .wait_timeout_while(state, Duration::from_secs(3), |state| {
                state.leases.get(&ticket.lease_id).is_some_and(|lease| {
                    Self::ticket(&ticket.lease_id, lease) == ticket && lease.stop_requested
                })
            })
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if timeout.timed_out() {
            projection.state = PreviewState::Unavailable;
            projection.error_code = Some(
                if remote {
                    "preview_cleanup_pending"
                } else {
                    "preview_stop_timeout"
                }
                .into(),
            );
            projection.detail = "已停止发布画面，采集资源仍在完成释放".into();
            if let Some(lease) = state
                .leases
                .get_mut(&ticket.lease_id)
                .filter(|lease| Self::ticket(&ticket.lease_id, lease) == ticket)
            {
                lease.projection = projection.clone();
            }
        } else {
            projection.state = PreviewState::Stopped;
            projection.error_code = None;
            projection.detail = "采集资源已释放".into();
        }
        Ok(projection)
    }
    pub fn stop_consumer(&self, consumer_id: &str) -> AppResult<()> {
        let requests: Vec<_> = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?
            .leases
            .iter()
            .filter(|(_, l)| l.projection.consumer_id == consumer_id)
            .map(|(id, l)| PreviewControlRequest {
                lease_id: id.clone(),
                generation: l.projection.generation.clone(),
                action: PreviewControl::Stop,
            })
            .collect();
        for request in requests {
            let _ = self.control(request)?;
        }
        Ok(())
    }
    pub fn stop_all(&self) -> AppResult<()> {
        // Closing is an admission fence, not a declaration that cleanup is done.
        let tickets = {
            let mut state = self
                .inner
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?;
            state.closed = true;
            state
                .leases
                .iter()
                .map(|(id, lease)| Self::ticket(id, lease))
                .collect::<Vec<_>>()
        };
        let mut pending = false;
        for ticket in tickets {
            let result = self.stop_exact_request(PreviewControlRequest {
                lease_id: ticket.lease_id,
                generation: ticket.generation,
                action: PreviewControl::Stop,
            });
            if !matches!(result, Ok(ref projection) if projection.state == PreviewState::Stopped) {
                pending = true;
                break;
            }
        }
        // A slow native proof can outlive the per-source timeout. Shutdown owns
        // the existing collector's final process drain; it may wait off Core/Hook
        // guards. Only actual reap permits removing all remaining remote owners.
        #[cfg(target_os = "macos")]
        if pending
            && self
                .inner
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?
                .leases
                .values()
                .any(|lease| lease.remote)
        {
            self.collector()?.shutdown_and_wait()?;
            let mut state = self
                .inner
                .lock()
                .map_err(|_| error("preview_state", "预览状态不可用"))?;
            state.leases.retain(|_, lease| !lease.remote);
            state.deferred_retirements.clear();
            drop(state);
            self.finished.notify_all();
            self.notify_lifecycle();
        }
        let state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        if state.inflight.is_some() || !state.leases.is_empty() {
            return Err(error("preview_cleanup_pending", "原生预览采集仍在清理"));
        }
        let _ = pending;
        Ok(())
    }
    fn finish_capture(
        &self,
        id: String,
        generation: String,
        target: VerifiedPreviewTarget,
        captured: AppResult<NativeCaptured>,
    ) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        let native_pending = false;
        if !native_pending {
            Self::release_finished_capture(&mut state);
            self.finished.notify_all();
            self.notify_lifecycle();
        }
        let Some(lease) = state.leases.get_mut(&id) else {
            let fact = Fact::rejected(
                &id,
                &generation,
                &target.target_identity_hash,
                Rejection::MissingLease,
            );
            drop(state);
            runtime_observation::record(fact);
            return;
        };
        if lease.stop_requested || lease.retirement_requested {
            lease.projection.state = PreviewState::Stopped;
            lease.frame = None;
            let fact = Fact::rejected(
                &id,
                &generation,
                &target.target_identity_hash,
                Rejection::StopRequested,
            );
            drop(state);
            runtime_observation::record(fact);
            return;
        }
        if !state::capture_completion_admitted(
            &lease.projection.generation,
            &generation,
            &lease.projection.state,
            lease
                .target()
                .map(|current| current.target_identity_hash.as_str()),
            &target.target_identity_hash,
        ) {
            let fact = Fact::rejected(
                &id,
                &generation,
                &target.target_identity_hash,
                Rejection::GenerationOrIdentity,
            );
            drop(state);
            runtime_observation::record(fact);
            return;
        }
        let committed = captured.is_ok();
        match captured {
            Ok(frame) => {
                let hash = content_hash(&frame.bytes);
                let Some(state::FrameCommitDecision {
                    sequence,
                    content_changed: changed,
                }) = state::next_frame_commit(
                    lease.sequence,
                    lease.frame.as_ref().map(|f| f.content_hash.as_str()),
                    &hash,
                )
                else {
                    lease.projection.state = PreviewState::Unavailable;
                    lease.projection.error_code = Some("preview_sequence_exhausted".into());
                    let fact = Fact::rejected(
                        &id,
                        &generation,
                        &target.target_identity_hash,
                        Rejection::SequenceExhausted,
                    );
                    drop(state);
                    runtime_observation::record(fact);
                    return;
                };
                lease.sequence = sequence;
                lease.projection.state = PreviewState::Live;
                lease.projection.permission = PreviewPermission::Granted;
                lease.projection.captured_at_ms = Some(frame.captured_at_ms.to_string());
                if changed {
                    lease.projection.content_changed_at_ms =
                        lease.projection.captured_at_ms.clone();
                }
                lease.projection.error_code = None;
                lease.projection.detail = if target.pairing.is_some() {
                    "你选择的游戏窗口实时画面"
                } else {
                    "已核验游戏窗口的实时画面"
                }
                .into();
                lease.frame = Some(PreviewFrame {
                    lease_id: id,
                    generation,
                    sequence: sequence.to_string(),
                    target_identity_hash: target.target_identity_hash,
                    captured_at_ms: frame.captured_at_ms.to_string(),
                    width: frame.width,
                    height: frame.height,
                    content_hash: hash,
                    encoding: "jpeg".into(),
                    payload: frame.bytes,
                });
            }
            Err(e) => {
                let decision = state::capture_failure_policy(&e.code, target.pairing.is_some());
                lease.projection.state = decision.state;
                if decision.permission_denied {
                    lease.projection.permission = PreviewPermission::Denied;
                }
                if decision.invalidate_pairing {
                    lease.backing = PreviewBacking::Failed;
                    lease.frame = None;
                    lease.projection.captured_at_ms = None;
                    lease.projection.content_changed_at_ms = None;
                }
                #[cfg(target_os = "macos")]
                let safe_detail = super::macos::safe_preview_failure_detail(&e);
                #[cfg(not(target_os = "macos"))]
                let safe_detail: Option<String> = None;
                lease.projection.error_code = Some(e.code);
                lease.projection.detail = decision.detail.into();
                if let Some(detail) = safe_detail {
                    lease.projection.detail = detail;
                }
            }
        }
        let stage = if committed {
            ProjectionStage::ServiceCommitted
        } else {
            ProjectionStage::ServiceStatus
        };
        let fact = Fact::projection(stage, &lease.projection, lease.frame.as_ref(), None);
        drop(state);
        runtime_observation::record(fact);
    }
    /// Stop may return before a bounded native callback has drained. The worker
    /// owns that final cleanup even after IPC ownership has been removed. A
    /// replaced lease has stop_requested=false and must survive an old result.
    fn release_finished_capture(state: &mut Inner) {
        if let Some(id) = state.inflight.take()
            && state.leases.get(&id).is_some_and(|l| l.stop_requested)
        {
            state.leases.remove(&id);
        }
    }
}
#[cfg(target_os = "macos")]
type NativeCaptured = super::macos::CapturedFrame;
#[cfg(not(target_os = "macos"))]
struct NativeCaptured {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    captured_at_ms: u64,
}
#[cfg(test)]
pub(crate) mod preview_unit_observation {
    use serde_json::{Value, json};
    use std::{
        collections::BTreeSet,
        sync::Mutex,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    pub(crate) struct Recorder {
        name: String,
        start: Instant,
        events: Mutex<(Vec<Value>, BTreeSet<(String, String)>)>,
    }
    impl Recorder {
        pub(crate) fn new(name: &str) -> Self {
            Self {
                name: name.into(),
                start: Instant::now(),
                events: Mutex::new((Vec::new(), BTreeSet::new())),
            }
        }
        pub(crate) fn event(&self, branch: &str, kind: &str, data: Value) {
            let mut events = self.events.lock().unwrap();
            let identity = (branch.to_owned(), kind.to_owned());
            let kind = if matches!(kind, "mutated_authority_attempt" | "lease_table_mutation")
                || events.1.insert(identity)
            {
                kind.to_owned()
            } else {
                format!("{kind}_repeat")
            };
            let sequence = events.0.len() + 1;
            let wall = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let mono = self.start.elapsed().as_nanos() as u64;
            events.0.push(json!({"sequence":sequence,"monotonic_ns":mono,"wall_ms":wall,"thread_id":format!("{:?}",std::thread::current().id()),"branch":branch,"kind":kind,"data":data}));
        }
        pub(crate) fn output(&self) {
            let (Ok(nonce), Ok(source)) = (
                std::env::var("LUNA_BINDING_NONCE"),
                std::env::var("LUNA_BINDING_SOURCE_TREE"),
            ) else {
                return;
            };
            println!(
                "LUNA_BINDING_PREVIEW_OBSERVATIONS_V1 {}",
                json!({"kind":"binding-preview-observations/v1","schema_version":1,"nonce":nonce,
                "source_tree_sha256":source,"test_name":self.name,"pid":std::process::id(),"events":self.events.lock().unwrap().0})
            );
        }
    }
}

#[cfg(test)]
impl WindowPreviewService {
    pub(crate) fn controlled_prepare_port(&self, port: Arc<ControlledPrepare>) {
        *self.controlled_prepare.lock().unwrap() = Some(port);
    }
    pub(crate) fn controlled_frozen_target(
        candidate: &str,
    ) -> app_infra::hook_backend::FrozenPreviewTarget {
        // Low transport fixture, never supplied to the real verifier as proof.
        serde_json::from_value(serde_json::json!({"candidate_id":candidate,"candidate":{
            "target":{"pid":12,"exe":"Fixture.exe","full_image_path":"C:\\Fixture.exe","creation_filetime":"99","architecture":"x64","errors":[]},
            "window":null,"settings":{"wine_path":"/fixture/wine","bottle":"fixture"},"environment_key":"crossover:/fixture"},
            "image_path":"/fixture/drive_c/Fixture.exe","file_stamp":[1,2,3,4,5,6,7],"resource_manifest_sha256":"controlled-resource"})).unwrap()
    }
    pub(crate) fn controlled_retirement_port(&self, port: Arc<ControlledRetirement>) {
        *self.controlled_retirement.lock().unwrap() = Some(port);
    }
    pub(crate) fn controlled_transfer_port(&self, port: Arc<ControlledTransfer>) {
        *self.controlled_transfer.lock().unwrap() = Some(port);
    }
    pub(crate) fn controlled_native_started(
        &self,
        projection: &PreviewProjection,
        target_hash: &str,
        capture: &str,
        paused: bool,
    ) -> AppResult<()> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("preview_state", "预览状态不可用"))?;
        let lease = state
            .leases
            .get_mut(
                projection
                    .lease_id
                    .as_deref()
                    .ok_or_else(|| error("preview_state", "missing lease"))?,
            )
            .filter(|lease| {
                lease.projection.generation == projection.generation
                    && !lease.stop_requested
                    && !lease.retirement_requested
            })
            .ok_or_else(|| error("preview_generation_conflict", "预览已变化"))?;
        lease.remote = true;
        lease.capture_id = Some(capture.into());
        lease.backing = PreviewBacking::Resolved(VerifiedPreviewTarget {
            candidate_id: lease.request.candidate_id.clone(),
            target_identity_hash: target_hash.into(),
            native: VerifiedNativeWindow {
                host_pid: std::process::id(),
                host_process_start: "controlled-low-level-port".into(),
                window_id: 1,
            },
            pairing: None,
        });
        lease.projection.state = if paused {
            PreviewState::Paused
        } else {
            PreviewState::Starting
        };
        lease.projection.target_identity_hash = Some(target_hash.into());
        Ok(())
    }
    pub(crate) fn controlled_release_ack(&self, ticket: &PreviewLeaseTicket) -> bool {
        self.retirement_acknowledged(ticket)
    }
    pub(crate) fn observed_lease(&self, id: &str, epoch: u64) -> Option<serde_json::Value> {
        let state = self.inner.lock().unwrap();
        let lease = state.leases.get(id)?;
        Some(
            serde_json::json!({"lease_id":id,"owner_kind":lease.consumer,"owner_id":lease.projection.consumer_id,
            "generation":lease.projection.generation,"fence":lease.projection.generation,"epoch":epoch,
            "filter":lease.capture_id.as_deref().unwrap_or("no-native-filter"),"paused":lease.projection.state==PreviewState::Paused}),
        )
    }
    pub(crate) fn observed_capture(&self, session: &str, epoch: u64) -> Option<serde_json::Value> {
        let id = self
            .inner
            .lock()
            .unwrap()
            .leases
            .iter()
            .find(|(_, lease)| {
                lease.consumer == PreviewConsumer::Session
                    && lease.projection.consumer_id == session
                    && !lease.stop_requested
                    && !lease.retirement_requested
                    && lease.capture_id.is_some()
            })
            .map(|(id, _)| id.clone())?;
        self.observed_lease(&id, epoch)
    }
    pub(crate) fn observed_budget(&self) -> serde_json::Value {
        let state = self.inner.lock().unwrap();
        serde_json::json!({"active":state.leases.values().filter(|lease|!lease.stop_requested).count(),
            "draining_pending":state.leases.values().filter(|lease|lease.stop_requested).count(),"physical_slots":state.leases.len()})
    }
    pub(crate) fn observed_frame_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .leases
            .values()
            .filter(|lease| lease.frame.is_some())
            .count()
    }
    pub(crate) fn coordinator_test_fixture(pairing: Arc<PreviewPairing>) -> Arc<Self> {
        let (service, _) = tests::fixture();
        if let Some(lease) = service.inner.lock().unwrap().leases.get_mut("lease") {
            let PreviewBacking::Resolved(target) = &mut lease.backing else {
                panic!("resolved fixture");
            };
            target.pairing = Some(pairing);
        }
        service
    }
    pub(crate) fn coordinator_test_remote(&self, candidate: bool) {
        let mut state = self.inner.lock().unwrap();
        let lease = state.leases.get_mut("lease").unwrap();
        lease.remote = true;
        if candidate {
            lease.consumer = PreviewConsumer::Candidate;
            lease.request.consumer = PreviewConsumer::Candidate;
        }
    }
    pub(crate) fn coordinator_test_poison(self: &Arc<Self>) {
        let service = self.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = service.inner.lock().unwrap();
                panic!("owned fixture read-lock failure");
            })
            .join()
            .is_err()
        );
    }
    pub(crate) fn coordinator_test_projection(&self) -> PreviewProjection {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .leases["lease"]
            .projection
            .clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::session_preview_coordinator::{
        SessionPreviewCoordinator, reading_component_fixtures::NativePreviewFixture,
    };
    use app_core::contracts::{AssociationAttributionIntent, AssociationReviewQuery};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct PrepareDispatch {
        lease: String,
        generation: String,
        candidate: String,
        select: bool,
        filter: String,
    }
    fn observe_prepare_dispatch(
        service: &WindowPreviewService,
    ) -> Arc<Mutex<Vec<PrepareDispatch>>> {
        let calls = Arc::new(Mutex::new(Vec::<PrepareDispatch>::new()));
        let observed = calls.clone();
        service.controlled_prepare_port(Arc::new(move |lease, generation, target, select| {
            let mut calls = observed.lock().unwrap();
            let filter = format!("native-prepare-filter-{}", calls.len() + 1);
            calls.push(PrepareDispatch {
                lease: lease.into(),
                generation: generation.into(),
                candidate: target.candidate_id().into(),
                select,
                filter: filter.clone(),
            });
            Ok(ControlledNativePrepared {
                target_identity_hash: app_core::domain::content_hash(target.candidate_id()),
                capture_id: filter,
            })
        }));
        calls
    }
    fn start_through_production_dispatch(
        service: &WindowPreviewService,
        projection: &PreviewProjection,
        candidate: &str,
    ) {
        service
            .start_remote(
                projection,
                WindowPreviewService::controlled_frozen_target(candidate),
                false,
            )
            .unwrap();
    }
    fn observed_start_counts(
        calls: &Mutex<Vec<PrepareDispatch>>,
        projection: &PreviewProjection,
    ) -> (usize, usize) {
        let calls = calls.lock().unwrap();
        let admitted = calls
            .iter()
            .filter(|call| {
                Some(call.lease.as_str()) == projection.lease_id.as_deref()
                    && call.generation == projection.generation
            })
            .collect::<Vec<_>>();
        assert!(
            admitted
                .iter()
                .all(|call| !call.select && !call.candidate.is_empty())
        );
        let filters = admitted
            .iter()
            .map(|call| call.filter.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        (admitted.len(), filters)
    }

    fn actual_preview_core_fixture(name: &str) -> NativePreviewFixture {
        let base = std::env::var_os("LUNA_BINDING_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("luna-preview-lease-tests"))
            .join(name);
        tauri::async_runtime::block_on(async { NativePreviewFixture::new(&base).unwrap() })
    }

    fn observed_budget_mutation(
        service: &WindowPreviewService,
        recorder: &preview_unit_observation::Recorder,
        branch: &str,
    ) {
        let budget = service.observed_budget();
        assert!(budget["active"].as_u64().unwrap() <= 2);
        assert!(budget["draining_pending"].as_u64().unwrap() <= 1);
        recorder.event(branch, "lease_table_mutation", budget);
    }

    fn actual_observation_epoch(
        runtime: Arc<app_core::application::runtime::ApplicationRuntime>,
        service: Arc<WindowPreviewService>,
    ) -> u64 {
        // This coordinator supplies the real Native ownership epoch only. The
        // controlled Hook used for Core authority remains the SQLite fixture's
        // actual port, and no OS collector is started by this coordinator.
        let unused_native_hook = app_infra::hook_backend::CoreHookBackend::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
        );
        SessionPreviewCoordinator::new(runtime, Arc::new(unused_native_hook), service)
            .epoch()
            .unwrap()
    }

    #[test]
    fn bj_07_nonblocking_retirement_admission() {
        const NAME: &str =
            "platform::window_preview::tests::bj_07_nonblocking_retirement_admission";
        const BRANCH: &str = "retirement_admission";
        let fixture = actual_preview_core_fixture("nonblocking-retirement");
        let runtime = fixture.runtime();
        let first = fixture.connect("candidate-main").unwrap();
        let old_authority = runtime
            .current_session_preview_authority()
            .unwrap()
            .unwrap();
        let service = WindowPreviewService::new();
        let prepare_calls = observe_prepare_dispatch(&service);
        let epoch = actual_observation_epoch(runtime.clone(), service.clone());
        let recorder = Arc::new(preview_unit_observation::Recorder::new(NAME));
        let old = service
            .begin_current_session_resolution(PreviewStartRequest {
                candidate_id: old_authority
                    .origin
                    .as_ref()
                    .unwrap()
                    .candidate
                    .candidate_id
                    .clone(),
                consumer: PreviewConsumer::Session,
                consumer_id: first.session_id.unwrap(),
                expected_generation: None,
            })
            .unwrap();
        observed_budget_mutation(&service, &recorder, BRANCH);
        start_through_production_dispatch(
            &service,
            &old,
            old_authority
                .origin
                .as_ref()
                .unwrap()
                .candidate
                .candidate_id
                .as_str(),
        );
        observed_budget_mutation(&service, &recorder, BRANCH);
        let old_ticket = service.session_tickets().unwrap().pop().unwrap();
        let old_lease = service.observed_lease(&old_ticket.lease_id, epoch).unwrap();

        fixture.connect("candidate-other").unwrap();
        let current = runtime
            .current_session_preview_authority()
            .unwrap()
            .unwrap();
        let request = PreviewStartRequest {
            candidate_id: current
                .origin
                .as_ref()
                .unwrap()
                .candidate
                .candidate_id
                .clone(),
            consumer: PreviewConsumer::Session,
            consumer_id: current.hook_session.session_id.clone(),
            expected_generation: None,
        };
        let (queued_tx, queued_rx) = mpsc::sync_channel(1);
        service.controlled_retirement_port(Arc::new(move |ticket| {
            queued_tx
                .try_send(ticket.clone())
                .map_err(|_| error("controlled_retirement_queue", "受控退出队列不可用"))?;
            Ok(false)
        }));
        let (dispatch_tx, dispatch_rx) = mpsc::sync_channel(1);
        let emergency_dispatch = dispatch_tx.clone();
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let (stop_entered_tx, stop_entered_rx) = mpsc::sync_channel(1);
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let stop_service = service.clone();
        let stop_recorder = recorder.clone();
        let stop_lease = old_lease.clone();
        let worker = std::thread::spawn(move || {
            let ticket = queued_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            // A real collector worker receives queued retirement independently
            // of the Core guard. Stop/ack is deliberately held by a channel.
            dispatch_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            stop_recorder.event(BRANCH, "collector_stop_dispatch", stop_lease.clone());
            stop_entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(stop_service.controlled_release_ack(&ticket));
            stop_recorder.event(BRANCH, "release_ack", stop_lease);
            observed_budget_mutation(&stop_service, &stop_recorder, BRANCH);
            ack_tx.send(()).unwrap();
        });
        let guarded_service = service.clone();
        let guarded_runtime = runtime.clone();
        let guarded_recorder = recorder.clone();
        let mark = old_lease.clone();
        let guarded_ticket = old_ticket.clone();
        let guarded_authority = current.clone();
        let (returned_tx, returned_rx) = mpsc::sync_channel(1);
        let guard = std::thread::spawn(move || {
            let result = guarded_runtime.with_current_session_preview_authority(
                &guarded_authority,
                &mut || {
                    assert_eq!(
                        guarded_service.retire_exact(&guarded_ticket)?,
                        RetirementAdmission::Draining
                    );
                    guarded_recorder.event(BRANCH, "retirement_mark", mark.clone());
                    observed_budget_mutation(&guarded_service, &guarded_recorder, BRANCH);
                    Ok(())
                },
            );
            guarded_recorder.event(BRANCH, "guard_exit", serde_json::json!({"result":result}));
            let _ = dispatch_tx.send(());
            returned_tx.send(result).unwrap();
        });
        let returned = returned_rx.recv_timeout(Duration::from_secs(3));
        if returned.is_err() {
            // Safety cleanup permits a regressed blocking implementation to
            // exit; this branch remains a test failure, never a timing pass.
            let _ = emergency_dispatch.try_send(());
            let _ = release_tx.try_send(());
            panic!("Core guard waited for retirement acknowledgement");
        }
        assert!(returned.unwrap().unwrap());
        guard.join().unwrap();
        stop_entered_rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        let fresh = runtime
            .current_session_preview_authority()
            .unwrap()
            .unwrap();
        let mut reentered = 0;
        runtime
            .with_current_session_preview_authority(&fresh, &mut || {
                reentered += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            reentered, 1,
            "Core and Hook guards remain available while Stop is held"
        );
        let busy = service
            .begin_current_session_resolution(request.clone())
            .unwrap_err();
        assert_eq!(busy.code, "preview_cleanup_pending");
        recorder.event(BRANCH, "typed_busy", serde_json::json!({"error":busy}));
        observed_budget_mutation(&service, &recorder, BRANCH);
        release_tx.send(()).unwrap();
        ack_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        worker.join().unwrap();
        assert!(
            service
                .observed_lease(&old_ticket.lease_id, epoch)
                .is_none()
        );
        let mut admitted = None;
        runtime
            .with_current_session_preview_authority(&fresh, &mut || {
                admitted = Some(service.begin_current_session_resolution(request.clone())?);
                Ok(())
            })
            .unwrap();
        observed_budget_mutation(&service, &recorder, BRANCH);
        let admitted = admitted.unwrap();
        start_through_production_dispatch(&service, &admitted, &request.candidate_id);
        let (collector_starts, filter_starts) = observed_start_counts(&prepare_calls, &admitted);
        assert_eq!((collector_starts, filter_starts), (1, 1));
        observed_budget_mutation(&service, &recorder, BRANCH);
        recorder.event(
            BRANCH,
            "session_admitted",
            service
                .observed_capture(&fresh.hook_session.session_id, epoch)
                .unwrap(),
        );
        assert_eq!(service.observed_frame_count(), 0);
        recorder.event(BRANCH, "call_counters", serde_json::json!({
            "collector_start":collector_starts,
            "filter_start":filter_starts,
            "core_reentries_while_stop_waited":reentered,"synthetic_frames":service.observed_frame_count()
        }));
        recorder.output();
    }

    #[test]
    fn bj_07_old_retirement_cannot_release_new_candidate() {
        const NAME: &str =
            "platform::window_preview::tests::bj_07_old_retirement_cannot_release_new_candidate";
        const BRANCH: &str = "old_ack";
        let fixture = actual_preview_core_fixture("old-retirement-new-candidate");
        let runtime = fixture.runtime();
        fixture.connect("candidate-main").unwrap();
        let old = runtime
            .current_session_preview_authority()
            .unwrap()
            .unwrap();
        let service = WindowPreviewService::new();
        let prepare_calls = observe_prepare_dispatch(&service);
        let epoch = actual_observation_epoch(runtime.clone(), service.clone());
        let recorder = preview_unit_observation::Recorder::new(NAME);
        let original = service
            .begin_current_session_resolution(PreviewStartRequest {
                candidate_id: old.origin.as_ref().unwrap().candidate.candidate_id.clone(),
                consumer: PreviewConsumer::Session,
                consumer_id: old.hook_session.session_id,
                expected_generation: None,
            })
            .unwrap();
        observed_budget_mutation(&service, &recorder, BRANCH);
        start_through_production_dispatch(
            &service,
            &original,
            old.origin.as_ref().unwrap().candidate.candidate_id.as_str(),
        );
        observed_budget_mutation(&service, &recorder, BRANCH);
        let ticket = service.session_tickets().unwrap().pop().unwrap();
        let candidate_stops = Arc::new(AtomicUsize::new(0));
        let stops = candidate_stops.clone();
        let (queue_tx, queue_rx) = mpsc::sync_channel(1);
        service.controlled_retirement_port(Arc::new(move |ticket| {
            if ticket.consumer == PreviewConsumer::Candidate {
                stops.fetch_add(1, Ordering::SeqCst);
            }
            queue_tx
                .try_send(ticket.clone())
                .map_err(|_| error("controlled_retirement_queue", "受控退出队列不可用"))?;
            Ok(false)
        }));
        fixture.connect("candidate-other").unwrap();
        let current = runtime
            .current_session_preview_authority()
            .unwrap()
            .unwrap();
        runtime
            .with_current_session_preview_authority(&current, &mut || {
                assert_eq!(
                    service.retire_exact(&ticket)?,
                    RetirementAdmission::Draining
                );
                Ok(())
            })
            .unwrap();
        observed_budget_mutation(&service, &recorder, BRANCH);
        let queued = queue_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let review = runtime
            .review_association(AssociationReviewQuery {
                candidate_id: "candidate-main".into(),
                game_id: None,
                expected_profile_generation: current.profile.profile_generation,
                expected_active_session_id: Some(current.hook_session.session_id),
                prior_review_id: None,
                attribution_intent: Some(AssociationAttributionIntent::SessionOnly),
            })
            .unwrap();
        let candidate = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: review.candidate.candidate_id.clone(),
                consumer: PreviewConsumer::Candidate,
                consumer_id: review.review_id,
                expected_generation: None,
            })
            .unwrap();
        observed_budget_mutation(&service, &recorder, BRANCH);
        start_through_production_dispatch(&service, &candidate, &review.candidate.candidate_id);
        observed_budget_mutation(&service, &recorder, BRANCH);
        let id = candidate.lease_id.as_deref().unwrap();
        let before = service.observed_lease(id, epoch).unwrap();
        recorder.event(BRANCH, "lease_before", before.clone());
        assert!(service.controlled_release_ack(&queued));
        observed_budget_mutation(&service, &recorder, BRANCH);
        assert!(
            !service.controlled_release_ack(&queued),
            "late duplicate ack is inert"
        );
        let new_ticket = service.candidate_tickets().unwrap().pop().unwrap();
        assert!(
            !service.controlled_release_ack(&new_ticket),
            "an active candidate has no retirement to acknowledge"
        );
        let after = service.observed_lease(id, epoch).unwrap();
        assert_eq!(before, after);
        recorder.event(BRANCH, "lease_after", after);
        assert_eq!(candidate_stops.load(Ordering::SeqCst), 0);
        assert_eq!(observed_start_counts(&prepare_calls, &candidate), (1, 1));
        assert_eq!(service.observed_frame_count(), 0);
        recorder.event(
            BRANCH,
            "call_counters",
            serde_json::json!({
                "new_candidate_stops":candidate_stops.load(Ordering::SeqCst),
                "synthetic_frames":service.observed_frame_count()
            }),
        );
        recorder.output();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn diagnostic_cache_reads_share_the_same_detail_without_capturing_or_changing_frame_time() {
        let (service, target) = fixture();
        service.coordinator_test_remote(false);
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target,
            Ok(frame(b"original", 123)),
        );
        let mut diagnostic = super::super::macos::PreviewDiagnostic::worker("frame_published");
        diagnostic.request = 9;
        service.collector_diagnostic("lease", "4", diagnostic);
        let latest = service.latest_update("lease", "4", None).unwrap();
        assert!(latest.projection.detail.contains("frame_published"));
        assert!(latest.frame.is_some());
        for include_frame in [false, true] {
            let attached = service
                .consumer_update("owner", include_frame)
                .unwrap()
                .unwrap();
            assert_eq!(attached.projection.detail, latest.projection.detail);
            assert_eq!(attached.projection.captured_at_ms.as_deref(), Some("123"));
            assert_eq!(attached.frame.is_some(), include_frame);
            if let Some(frame) = attached.frame {
                assert_eq!(frame.sequence, latest.frame.as_ref().unwrap().sequence);
            }
        }
        let after = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(after.projection.detail, latest.projection.detail);
        assert_eq!(
            after.frame.unwrap().sequence,
            latest.frame.unwrap().sequence
        );
        assert!(service.collector.lock().unwrap().is_none());
        assert!(service.inner.lock().unwrap().inflight.is_none());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn diagnostic_cache_reads_reject_unchecked_secrets_in_both_read_paths() {
        let (service, _) = fixture();
        service.coordinator_test_remote(false);
        let mut diagnostic = super::super::macos::PreviewDiagnostic::worker("capture_complete");
        diagnostic.reason = Some("/Users/private/sk-secret-value".into());
        // Exercise the shared formatter's defense even if an invalid internal
        // value bypassed the normal checked collector publication path.
        service
            .inner
            .lock()
            .unwrap()
            .leases
            .get_mut("lease")
            .unwrap()
            .diagnostic = Some(diagnostic);
        let latest = service.latest_update("lease", "4", None).unwrap();
        assert!(!latest.projection.detail.contains("sk-secret"));
        assert!(!latest.projection.detail.contains("诊断："));
        for include_frame in [false, true] {
            let attached = service
                .consumer_update("owner", include_frame)
                .unwrap()
                .unwrap();
            assert_eq!(attached.projection.detail, latest.projection.detail);
            assert!(attached.frame.is_none());
        }
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn remote_diagnostics_are_visible_without_promoting_metadata_into_a_first_frame() {
        let (service, _) = fixture();
        service.coordinator_test_remote(false);
        let known_before_frame = service.consumer_update("owner", true).unwrap().unwrap();
        assert!(known_before_frame.projection.target_identity_hash.is_some());
        assert_eq!(known_before_frame.projection.state, PreviewState::Starting);
        assert_eq!(known_before_frame.projection.captured_at_ms, None);
        assert!(known_before_frame.frame.is_none());
        let mut diagnostic = super::super::macos::PreviewDiagnostic::worker("capture_complete");
        diagnostic.native_status = 5;
        diagnostic.reason = Some("image_dimensions".into());
        diagnostic.configured_width = 960;
        diagnostic.configured_height = 540;
        diagnostic.actual_width = 1920;
        diagnostic.actual_height = 1080;
        service.collector_status(
            "lease",
            "4",
            CollectorStatusUpdate {
                phase: "Unavailable",
                code: Some("preview_budget_exceeded"),
                permission: true,
                observed_at_ms: 10,
                diagnostic: Some(&diagnostic),
            },
        );
        let first = service.latest_update("lease", "4", None).unwrap();
        assert!(first.frame.is_none());
        assert_eq!(first.projection.captured_at_ms, None);
        assert!(first.projection.detail.contains("1920×1080"));
        service.collector_status(
            "lease",
            "4",
            CollectorStatusUpdate {
                phase: "Live",
                code: None,
                permission: true,
                observed_at_ms: 20,
                diagnostic: Some(&diagnostic),
            },
        );
        let next = service.latest_update("lease", "4", None).unwrap();
        assert_ne!(next.projection.state, PreviewState::Live);
        assert!(next.frame.is_none());
        assert_eq!(next.projection.captured_at_ms, None);
        let old = next.projection.detail;
        diagnostic.actual_width = 123;
        service.collector_diagnostic("lease", "stale-generation", diagnostic);
        assert_eq!(
            service
                .latest_update("lease", "4", None)
                .unwrap()
                .projection
                .detail,
            old
        );
    }
    fn dispatched_source(
        service: &WindowPreviewService,
        consumer: PreviewConsumer,
        owner: &str,
        target: &str,
    ) -> PreviewProjection {
        let projection = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: target.into(),
                consumer,
                consumer_id: owner.into(),
                expected_generation: None,
            })
            .unwrap();
        start_through_production_dispatch(service, &projection, target);
        projection
    }
    fn assert_physical_budget(service: &WindowPreviewService) {
        let budget = service.observed_budget();
        assert!(budget["active"].as_u64().unwrap() <= 2);
        assert!(budget["draining_pending"].as_u64().unwrap() <= 1);
        assert!(budget["physical_slots"].as_u64().unwrap() <= 3);
    }
    #[test]
    fn remote_stop_timeout_defers_closed_candidate_until_ack_and_protects_two_new_generations() {
        let service = WindowPreviewService::new();
        let calls = observe_prepare_dispatch(&service);
        let session =
            dispatched_source(&service, PreviewConsumer::Session, "session-a", "target-a");
        let candidate = dispatched_source(
            &service,
            PreviewConsumer::Candidate,
            "candidate-b",
            "target-b",
        );
        let (tx, rx) = mpsc::sync_channel(4);
        service.controlled_retirement_port(Arc::new(move |ticket| {
            tx.try_send(ticket.clone()).unwrap();
            Ok(false)
        }));
        let stopped = service
            .control(PreviewControlRequest {
                lease_id: session.lease_id.clone().unwrap(),
                generation: session.generation,
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(
            stopped.error_code.as_deref(),
            Some("preview_cleanup_pending")
        );
        let a = rx.recv().unwrap();
        assert_physical_budget(&service);
        let before_busy = service
            .observed_lease(candidate.lease_id.as_deref().unwrap(), 0)
            .unwrap();
        let before_counter = service.inner.lock().unwrap().generation;
        let rejected = service
            .control(PreviewControlRequest {
                lease_id: candidate.lease_id.clone().unwrap(),
                generation: candidate.generation,
                action: PreviewControl::Stop,
            })
            .unwrap_err();
        assert_eq!(rejected.code, "preview_cleanup_pending");
        assert_eq!(before_counter, service.inner.lock().unwrap().generation);
        assert_eq!(
            before_busy,
            service
                .observed_lease(candidate.lease_id.as_deref().unwrap(), 0)
                .unwrap()
        );
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert_physical_budget(&service);
        // The Candidate UI is gone: no caller retries its rejected Stop.
        let next = dispatched_source(
            &service,
            PreviewConsumer::Candidate,
            "candidate-c",
            "target-c",
        );
        assert_physical_budget(&service);
        let newer = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: "target-c".into(),
                consumer: PreviewConsumer::Candidate,
                consumer_id: "candidate-c".into(),
                expected_generation: Some(next.generation.clone()),
            })
            .unwrap();
        start_through_production_dispatch(&service, &newer, "target-c");
        assert_ne!(next.generation, newer.generation);
        let id = newer.lease_id.as_deref().unwrap();
        let before = service.observed_lease(id, 0).unwrap();
        assert!(service.controlled_release_ack(&a));
        let b = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(b.consumer_id, "candidate-b");
        assert_physical_budget(&service);
        assert!(!service.controlled_release_ack(&a));
        assert!(service.controlled_release_ack(&b));
        assert_physical_budget(&service);
        assert_eq!(before, service.observed_lease(id, 0).unwrap());
        assert!(
            service
                .consumer_update("candidate-b", true)
                .unwrap()
                .is_none()
        );
        assert_eq!(observed_start_counts(&calls, &newer), (1, 1));
        let ticket = service.candidate_tickets().unwrap().pop().unwrap();
        assert_eq!(
            service.retire_exact(&ticket).unwrap(),
            RetirementAdmission::Draining
        );
        assert_eq!(rx.recv().unwrap(), ticket);
        assert!(service.controlled_release_ack(&ticket));
        service.stop_all().unwrap();
    }
    #[test]
    fn successful_new_generation_control_supersedes_only_the_unadmitted_old_cleanup_ticket() {
        let service = WindowPreviewService::new();
        observe_prepare_dispatch(&service);
        service.controlled_transfer_port(Arc::new(|_, _, _, _| Ok(())));
        let session =
            dispatched_source(&service, PreviewConsumer::Session, "session-a", "target-a");
        let candidate = dispatched_source(
            &service,
            PreviewConsumer::Candidate,
            "candidate-b",
            "target-b",
        );
        let a = service.session_tickets().unwrap().pop().unwrap();
        let old_b = service.candidate_tickets().unwrap().pop().unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        service.controlled_retirement_port(Arc::new(move |ticket| {
            tx.try_send(ticket.clone()).unwrap();
            Ok(false)
        }));
        service.retire_exact(&a).unwrap();
        assert_eq!(rx.recv().unwrap(), a);
        assert_eq!(
            service.retire_exact(&old_b).unwrap_err().code,
            "preview_cleanup_pending"
        );
        let resumed = service
            .control(PreviewControlRequest {
                lease_id: candidate.lease_id.unwrap(),
                generation: candidate.generation,
                action: PreviewControl::Resume,
            })
            .unwrap();
        assert_ne!(old_b.generation, resumed.generation);
        let before = service
            .observed_lease(resumed.lease_id.as_deref().unwrap(), 0)
            .unwrap();
        assert!(service.controlled_release_ack(&a));
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert_eq!(
            service.retire_exact(&old_b).unwrap_err().code,
            "preview_generation_conflict"
        );
        assert!(!service.controlled_release_ack(&old_b));
        assert_eq!(
            before,
            service
                .observed_lease(resumed.lease_id.as_deref().unwrap(), 0)
                .unwrap()
        );
        let current = service.candidate_tickets().unwrap().pop().unwrap();
        service.retire_exact(&current).unwrap();
        assert_eq!(rx.recv().unwrap(), current);
        assert!(service.controlled_release_ack(&current));
        service.stop_all().unwrap();
        let _ = session;
    }
    #[test]
    fn shutdown_keeps_serial_release_driver_until_both_real_acknowledgements() {
        let service = WindowPreviewService::new();
        observe_prepare_dispatch(&service);
        dispatched_source(&service, PreviewConsumer::Session, "session-a", "target-a");
        let candidate = dispatched_source(
            &service,
            PreviewConsumer::Candidate,
            "candidate-b",
            "target-b",
        );
        let (tx, rx) = mpsc::sync_channel(4);
        service.controlled_retirement_port(Arc::new(move |ticket| {
            tx.try_send(ticket.clone()).unwrap();
            Ok(false)
        }));
        let closing = service.clone();
        let worker = std::thread::spawn(move || closing.stop_all());
        let first = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_physical_budget(&service);
        assert!(service.is_closed());
        assert_eq!(
            service
                .start_remote(
                    &candidate,
                    WindowPreviewService::controlled_frozen_target("target-b"),
                    false
                )
                .unwrap_err()
                .code,
            "preview_closed"
        );
        assert_eq!(
            service
                .transfer_pairing_to_session(
                    candidate.lease_id.as_deref().unwrap(),
                    &candidate.generation,
                    "target-b",
                    "late-session"
                )
                .unwrap_err()
                .code,
            "preview_closed"
        );
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(service.controlled_release_ack(&first));
        let second = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_ne!(first.lease_id, second.lease_id);
        assert_physical_budget(&service);
        assert!(service.controlled_release_ack(&second));
        worker.join().unwrap().unwrap();
        assert_eq!(service.observed_budget()["physical_slots"], 0);
        assert_eq!(
            service
                .begin_resolution(PreviewStartRequest {
                    candidate_id: "late".into(),
                    consumer: PreviewConsumer::Candidate,
                    consumer_id: "late".into(),
                    expected_generation: None
                })
                .unwrap_err()
                .code,
            "preview_closed"
        );
    }

    pub(super) fn fixture() -> (Arc<WindowPreviewService>, VerifiedPreviewTarget) {
        let service = Arc::new(WindowPreviewService {
            inner: Mutex::new(Inner {
                generation: 4,
                permission_requested: false,
                leases: BTreeMap::new(),
                deferred_retirements: Vec::new(),
                inflight: None,
                closed: false,
            }),
            finished: Condvar::new(),
            lifecycle: Mutex::new(Vec::new()),
            controlled_retirement: Mutex::new(None),
            controlled_transfer: Mutex::new(None),
            #[cfg(test)]
            controlled_prepare: Mutex::new(None),
            #[cfg(target_os = "macos")]
            collector: Mutex::new(None),
        });
        let target = VerifiedPreviewTarget {
            candidate_id: "server-candidate".into(),
            target_identity_hash: "verified-target".into(),
            pairing: None,
            native: VerifiedNativeWindow {
                host_pid: 1,
                host_process_start: "start".into(),
                window_id: 2,
            },
        };
        let projection = PreviewProjection {
            lease_id: Some("lease".into()),
            generation: "4".into(),
            consumer_id: "owner".into(),
            state: PreviewState::Starting,
            permission: PreviewPermission::Granted,
            target_identity_hash: Some(target.target_identity_hash.clone()),
            attempted_at_ms: None,
            captured_at_ms: None,
            content_changed_at_ms: None,
            permission_checked_at_ms: None,
            error_code: None,
            detail: String::new(),
        };
        service.inner.lock().unwrap().leases.insert(
            "lease".into(),
            Lease {
                consumer: PreviewConsumer::Session,
                request: PreviewStartRequest {
                    candidate_id: target.candidate_id.clone(),
                    consumer: PreviewConsumer::Session,
                    consumer_id: "owner".into(),
                    expected_generation: None,
                },
                backing: PreviewBacking::Resolved(target.clone()),
                projection,
                frame: None,
                sequence: 0,
                #[cfg(test)]
                last_attempt: 0,
                stop_requested: false,
                retirement_requested: false,
                retiring_ticket: None,
                remote: false,
                capture_id: None,
                #[cfg(target_os = "macos")]
                diagnostic: None,
            },
        );
        (service, target)
    }
    fn failure_request() -> PreviewStartRequest {
        PreviewStartRequest {
            candidate_id: "server-candidate".into(),
            consumer: PreviewConsumer::Session,
            consumer_id: "owner".into(),
            expected_generation: Some("4".into()),
        }
    }
    #[test]
    fn failed_logical_slot_is_typed_controllable_and_cannot_capture_or_commit_cover() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"old", 10)),
        );
        let pending = service.begin_resolution(failure_request()).unwrap();
        let id = pending.lease_id.clone().unwrap();
        let failure = service
            .fail_resolution(
                &id,
                &pending.generation,
                error("preview_window_selection_required", "明确选择窗口"),
            )
            .unwrap();
        assert_eq!(failure.state, PreviewState::Unavailable);
        assert_eq!(
            failure.error_code.as_deref(),
            Some("preview_window_selection_required")
        );
        assert!(failure.captured_at_ms.is_none());
        assert!(failure.target_identity_hash.is_none());
        assert!(
            service
                .consumer_update("owner", true)
                .unwrap()
                .unwrap()
                .frame
                .is_none()
        );
        assert!(service.inner.lock().unwrap().inflight.is_none());
        service.finish_capture(
            id.clone(),
            failure.generation.clone(),
            target,
            Ok(frame(b"late", 20)),
        );
        let latest = service
            .latest_update(&id, &failure.generation, None)
            .unwrap();
        assert!(latest.frame.is_none());
        assert_eq!(latest.projection.state, PreviewState::Unavailable);
        let retry = service
            .retry_request(&id, &failure.generation)
            .unwrap()
            .unwrap();
        assert_eq!(
            retry.expected_generation.as_deref(),
            Some(failure.generation.as_str())
        );
        assert!(service.retry_request(&id, "4").is_err());
        let stopped = service
            .control(PreviewControlRequest {
                lease_id: id.clone(),
                generation: failure.generation,
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(stopped.state, PreviewState::Stopped);
        assert!(service.consumer_update("owner", true).unwrap().is_none());
        assert!(service.retry_request(&id, &stopped.generation).is_err());
    }
    #[test]
    fn late_failure_and_result_cannot_replace_new_generation_pending_slot() {
        let (service, target) = fixture();
        let first = service.begin_resolution(failure_request()).unwrap();
        let mut next = failure_request();
        next.expected_generation = Some(first.generation.clone());
        let second = service.begin_resolution(next).unwrap();
        let id = first.lease_id.unwrap();
        assert!(!service.is_current(&id, &first.generation));
        assert!(service.is_current(&id, &second.generation));
        assert!(
            service
                .fail_resolution(
                    &id,
                    &first.generation,
                    error("preview_selection_cancelled", "old cancel")
                )
                .is_err()
        );
        let mut old = failure_request();
        old.expected_generation = Some(first.generation);
        assert!(service.start(target, old).is_err());
        let latest = service
            .latest_update(&id, &second.generation, None)
            .unwrap();
        assert_eq!(latest.projection.state, PreviewState::Starting);
        assert!(latest.frame.is_none());
    }
    #[test]
    fn logical_pending_slot_also_stops_without_a_native_target() {
        let (service, _) = fixture();
        let pending = service.begin_resolution(failure_request()).unwrap();
        let id = pending.lease_id.unwrap();
        assert!(!service.has_resolved(&id, &pending.generation));
        assert!(
            service
                .retry_request(&id, &pending.generation)
                .unwrap()
                .is_none()
        );
        let stopped = service
            .control(PreviewControlRequest {
                lease_id: id.clone(),
                generation: pending.generation.clone(),
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(stopped.state, PreviewState::Stopped);
        assert_ne!(stopped.generation, pending.generation);
        assert!(!service.is_current(&id, &pending.generation));
        assert!(service.consumer_update("owner", false).unwrap().is_none());
    }
    #[test]
    fn immediate_stop_advances_public_fence_and_old_repeat_or_ack_cannot_touch_the_next_lease() {
        let service = WindowPreviewService::new();
        let first = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: "pending-target".into(),
                consumer: PreviewConsumer::Candidate,
                consumer_id: "panel".into(),
                expected_generation: None,
            })
            .unwrap();
        let old = service.candidate_tickets().unwrap().pop().unwrap();
        let previous = service.inner.lock().unwrap().generation;
        let stopped = service
            .control(PreviewControlRequest {
                lease_id: first.lease_id.clone().unwrap(),
                generation: first.generation.clone(),
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(stopped.state, PreviewState::Stopped);
        assert_eq!(stopped.generation, (previous + 1).to_string());
        assert!(service.candidate_tickets().unwrap().is_empty());
        let next = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: "next-target".into(),
                consumer: PreviewConsumer::Candidate,
                consumer_id: "panel".into(),
                expected_generation: None,
            })
            .unwrap();
        let before = service
            .observed_lease(next.lease_id.as_deref().unwrap(), 0)
            .unwrap();
        let counter = service.inner.lock().unwrap().generation;
        assert_eq!(
            service
                .control(PreviewControlRequest {
                    lease_id: first.lease_id.unwrap(),
                    generation: first.generation,
                    action: PreviewControl::Stop
                })
                .unwrap_err()
                .code,
            "preview_lease_expired"
        );
        assert!(!service.controlled_release_ack(&old));
        assert_eq!(counter, service.inner.lock().unwrap().generation);
        assert_eq!(
            before,
            service
                .observed_lease(next.lease_id.as_deref().unwrap(), 0)
                .unwrap()
        );
        service.stop_all().unwrap();
    }
    #[test]
    fn public_stop_generation_exhaustion_has_no_release_effect_for_pending_or_remote_sources() {
        for remote in [false, true] {
            let service = WindowPreviewService::new();
            observe_prepare_dispatch(&service);
            let pending = service
                .begin_resolution(PreviewStartRequest {
                    candidate_id: "target".into(),
                    consumer: PreviewConsumer::Candidate,
                    consumer_id: "panel".into(),
                    expected_generation: None,
                })
                .unwrap();
            if remote {
                start_through_production_dispatch(&service, &pending, "target");
            }
            let release_calls = Arc::new(AtomicUsize::new(0));
            let observed = release_calls.clone();
            service.controlled_retirement_port(Arc::new(move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(!remote)
            }));
            let ticket = service.candidate_tickets().unwrap().pop().unwrap();
            let before = service
                .observed_lease(pending.lease_id.as_deref().unwrap(), 0)
                .unwrap();
            service.inner.lock().unwrap().generation = u64::MAX;
            assert_eq!(
                service
                    .control(PreviewControlRequest {
                        lease_id: pending.lease_id.clone().unwrap(),
                        generation: pending.generation.clone(),
                        action: PreviewControl::Stop
                    })
                    .unwrap_err()
                    .code,
                "preview_generation_exhausted"
            );
            assert_eq!(release_calls.load(Ordering::SeqCst), 0);
            assert_eq!(
                before,
                service
                    .observed_lease(pending.lease_id.as_deref().unwrap(), 0)
                    .unwrap()
            );
            assert!(service.is_current(pending.lease_id.as_deref().unwrap(), &pending.generation));
            assert!(
                service
                    .inner
                    .lock()
                    .unwrap()
                    .deferred_retirements
                    .is_empty()
            );
            // Final exact Native cleanup remains possible without minting a UI
            // fence; it is not reported as a successful public Stop.
            let result = service.retire_exact(&ticket).unwrap();
            assert_eq!(release_calls.load(Ordering::SeqCst), 1);
            if remote {
                assert_eq!(result, RetirementAdmission::Draining);
                assert!(service.controlled_release_ack(&ticket));
            } else {
                assert_eq!(result, RetirementAdmission::Released);
            }
            service.stop_all().unwrap();
        }
    }
    #[test]
    fn privately_draining_source_gets_one_public_fence_and_duplicate_stop_keeps_original_release_ticket()
     {
        let service = WindowPreviewService::new();
        observe_prepare_dispatch(&service);
        let pending = dispatched_source(&service, PreviewConsumer::Candidate, "panel", "target");
        let original = service.candidate_tickets().unwrap().pop().unwrap();
        let releases = Arc::new(AtomicUsize::new(0));
        let count = releases.clone();
        service.controlled_retirement_port(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(false)
        }));
        assert_eq!(
            service.retire_exact(&original).unwrap(),
            RetirementAdmission::Draining
        );
        let first_service = service.clone();
        let id = pending.lease_id.clone().unwrap();
        let first_id = id.clone();
        let old = pending.generation.clone();
        let first = std::thread::spawn(move || {
            first_service.control(PreviewControlRequest {
                lease_id: first_id,
                generation: old,
                action: PreviewControl::Stop,
            })
        });
        let until = std::time::Instant::now() + Duration::from_secs(2);
        let public = loop {
            let current = service.observed_lease(&id, 0).unwrap();
            if current["generation"] != pending.generation {
                break current["generation"].as_str().unwrap().to_owned();
            }
            assert!(
                std::time::Instant::now() < until,
                "public Stop did not fence privately admitted release"
            );
            std::thread::yield_now();
        };
        let mut repeated_ticket = original.clone();
        repeated_ticket.generation = public.clone();
        let counter = service.inner.lock().unwrap().generation;
        assert_eq!(
            service
                .retire_exact_with_public_fence(&repeated_ticket, true)
                .unwrap(),
            (RetirementAdmission::Draining, None)
        );
        assert_eq!(counter, service.inner.lock().unwrap().generation);
        // The original exact transport receipt remains authoritative even though
        // public UI generation moved. Late duplicate receipt is then inert.
        assert!(service.controlled_release_ack(&original));
        let first = first.join().unwrap().unwrap();
        assert_eq!(first.state, PreviewState::Stopped);
        assert_eq!(first.generation, public);
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        assert!(!service.controlled_release_ack(&original));
        service.stop_all().unwrap();
    }

    fn frame(bytes: &[u8], time: u64) -> NativeCaptured {
        NativeCaptured {
            bytes: bytes.to_vec(),
            width: 4,
            height: 3,
            captured_at_ms: time,
        }
    }
    #[test]
    fn exhausted_service_sequence_preserves_the_last_image_and_observation_time() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"retained", 101)),
        );
        let before = service.latest_update("lease", "4", None).unwrap();
        service
            .inner
            .lock()
            .unwrap()
            .leases
            .get_mut("lease")
            .unwrap()
            .sequence = u64::MAX;

        service.finish_capture("lease".into(), "4".into(), target, Ok(frame(b"new", 202)));

        let after = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(after.frame, before.frame);
        assert_eq!(
            after.projection.captured_at_ms,
            before.projection.captured_at_ms
        );
        assert_eq!(
            after.projection.content_changed_at_ms,
            before.projection.content_changed_at_ms
        );
        assert_eq!(after.projection.state, PreviewState::Unavailable);
        assert_eq!(
            after.projection.error_code.as_deref(),
            Some("preview_sequence_exhausted")
        );
        assert_eq!(
            service.inner.lock().unwrap().leases["lease"].sequence,
            u64::MAX
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn same_capture_cannot_replace_native_identity_but_new_capture_keeps_existing_resolution_path()
    {
        let (service, _) = fixture();
        service.coordinator_test_remote(false);
        service.collector_frame(
            "lease",
            "4",
            "server-candidate",
            "capture-a",
            98,
            1,
            "start",
            2,
            frame(b"original", 101),
        );
        let before = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(before.frame.as_ref().unwrap().sequence, "1");
        for (pid, start, window) in [(9, "start", 2), (1, "reused-pid", 2), (1, "start", 3)] {
            service.collector_frame(
                "lease",
                "4",
                "server-candidate",
                "capture-a",
                99,
                pid,
                start,
                window,
                frame(b"foreign", 202),
            );
            assert_eq!(service.latest_update("lease", "4", None).unwrap(), before);
        }
        service.collector_frame(
            "lease",
            "4",
            "server-candidate",
            "capture-b",
            1,
            1,
            "start",
            3,
            frame(b"new capture", 303),
        );
        let next = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(next.frame.as_ref().unwrap().sequence, "2");
        assert_ne!(
            next.projection.target_identity_hash,
            before.projection.target_identity_hash
        );
        assert_eq!(next.projection.captured_at_ms.as_deref(), Some("303"));
    }
    #[test]
    fn native_capture_failures_preserve_paired_and_unpaired_invalidation_distinctions() {
        for paired in [false, true] {
            for (code, expected_state, invalidates_pair) in [
                (
                    "preview_permission_revoked",
                    PreviewState::PermissionRevoked,
                    false,
                ),
                ("preview_target_closed", PreviewState::TargetClosed, true),
                (
                    "preview_selection_mismatch",
                    PreviewState::Unavailable,
                    true,
                ),
                ("preview_selection_expired", PreviewState::Unavailable, true),
                ("stale_target", PreviewState::Unavailable, true),
                ("preview_capture_timeout", PreviewState::Stale, false),
                ("preview_capture_busy", PreviewState::Stale, false),
                (
                    "preview_verification_unavailable",
                    PreviewState::Stale,
                    false,
                ),
                (
                    "preview_window_not_on_screen",
                    PreviewState::Unavailable,
                    false,
                ),
                ("preview_unknown", PreviewState::Unavailable, false),
            ] {
                let (service, mut target) = fixture();
                if paired {
                    target.pairing = Some(Arc::new(PreviewPairing {
                        _token: "test-pair".into(),
                        _keepalive: Arc::new(|| Ok(())),
                    }));
                    service
                        .inner
                        .lock()
                        .unwrap()
                        .leases
                        .get_mut("lease")
                        .unwrap()
                        .backing = PreviewBacking::Resolved(target.clone());
                }
                service.finish_capture(
                    "lease".into(),
                    "4".into(),
                    target.clone(),
                    Ok(frame(b"retained", 101)),
                );
                let before = service.latest_update("lease", "4", None).unwrap();
                service.finish_capture(
                    "lease".into(),
                    "4".into(),
                    target,
                    Err(error(code, "failure")),
                );
                let after = service.latest_update("lease", "4", None).unwrap();
                assert_eq!(
                    after.projection.state, expected_state,
                    "{code} paired={paired}"
                );
                assert_eq!(after.projection.error_code.as_deref(), Some(code));
                assert_eq!(
                    after.projection.permission,
                    if code == "preview_permission_revoked" {
                        PreviewPermission::Denied
                    } else {
                        PreviewPermission::Granted
                    }
                );
                if paired && invalidates_pair {
                    assert!(after.frame.is_none(), "{code}");
                    assert_eq!(after.projection.captured_at_ms, None);
                    assert_eq!(after.projection.content_changed_at_ms, None);
                    assert!(!service.has_resolved("lease", "4"));
                } else {
                    assert_eq!(after.frame, before.frame, "{code} paired={paired}");
                    assert_eq!(after.projection.captured_at_ms.as_deref(), Some("101"));
                    assert_eq!(
                        after.projection.content_changed_at_ms.as_deref(),
                        Some("101")
                    );
                    assert!(service.has_resolved("lease", "4"));
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn collector_metadata_keeps_its_own_failure_classification_and_clear_image_rules() {
        for (code, expected_state, clears_image) in [
            (
                "preview_permission_revoked",
                PreviewState::PermissionDenied,
                false,
            ),
            (
                "preview_permission_denied",
                PreviewState::PermissionDenied,
                false,
            ),
            ("preview_target_closed", PreviewState::TargetClosed, true),
            ("preview_selection_expired", PreviewState::Stale, true),
            ("preview_selection_mismatch", PreviewState::Stale, true),
            ("stale_target", PreviewState::Stale, false),
        ] {
            let (service, target) = fixture();
            service.coordinator_test_remote(false);
            service.finish_capture(
                "lease".into(),
                "4".into(),
                target,
                Ok(frame(b"retained", 101)),
            );
            let before = service.latest_update("lease", "4", None).unwrap();
            service.collector_status(
                "lease",
                "4",
                CollectorStatusUpdate {
                    phase: "Unavailable",
                    code: Some(code),
                    permission: false,
                    observed_at_ms: 202,
                    diagnostic: None,
                },
            );
            let after = service.latest_update("lease", "4", None).unwrap();
            assert_eq!(after.projection.state, expected_state, "{code}");
            assert_eq!(after.projection.permission, PreviewPermission::Denied);
            assert_eq!(
                after.projection.permission_checked_at_ms.as_deref(),
                Some("202")
            );
            assert!(
                service.has_resolved("lease", "4"),
                "metadata does not clear backing: {code}"
            );
            if clears_image {
                assert!(after.frame.is_none());
                assert_eq!(after.projection.captured_at_ms, None);
                assert_eq!(after.projection.content_changed_at_ms, None);
            } else {
                assert_eq!(after.frame, before.frame);
                assert_eq!(after.projection.captured_at_ms.as_deref(), Some("101"));
            }
        }
    }
    #[test]
    fn first_frame_commit_rechecks_native_lease_generation_consumer_and_identity_after_preparation()
    {
        let (service, target) = fixture();
        service.finish_capture("lease".into(), "4".into(), target, Ok(frame(b"current", 7)));
        let original = service
            .consumer_update("owner", true)
            .unwrap()
            .unwrap()
            .frame
            .unwrap();
        let mut count = 0;
        assert_eq!(
            service
                .commit_current_session_frame_checked(
                    "owner",
                    &original,
                    |_| true,
                    || {
                        count += 1;
                        Ok(9)
                    }
                )
                .unwrap(),
            9
        );
        assert_eq!(count, 1);
        assert!(
            service
                .commit_current_session_frame_checked(
                    "other-session",
                    &original,
                    |_| true,
                    || -> AppResult<()> { panic!("wrong session committed") }
                )
                .is_err()
        );
        assert!(
            service
                .commit_current_session_frame_checked(
                    "owner",
                    &original,
                    |_| false,
                    || -> AppResult<()> { panic!("dead native identity committed") }
                )
                .is_err()
        );
        let mut wrong = original.clone();
        wrong.target_identity_hash = "other-native-window".into();
        assert!(
            service
                .commit_current_session_frame_checked(
                    "owner",
                    &wrong,
                    |_| true,
                    || -> AppResult<()> { panic!("wrong native frame committed") }
                )
                .is_err()
        );
        service
            .inner
            .lock()
            .unwrap()
            .leases
            .get_mut("lease")
            .unwrap()
            .consumer = PreviewConsumer::Candidate;
        assert!(
            service
                .commit_current_session_frame_checked(
                    "owner",
                    &original,
                    |_| true,
                    || -> AppResult<()> { panic!("candidate frame saved") }
                )
                .is_err()
        );
        service
            .inner
            .lock()
            .unwrap()
            .leases
            .get_mut("lease")
            .unwrap()
            .consumer = PreviewConsumer::Session;
        service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action: PreviewControl::Pause,
            })
            .unwrap();
        assert!(
            service
                .commit_current_session_frame_checked(
                    "owner",
                    &original,
                    |_| true,
                    || -> AppResult<()> { panic!("late paused frame committed") }
                )
                .is_err()
        );
        let current_generation = service
            .consumer_update("owner", false)
            .unwrap()
            .unwrap()
            .projection
            .generation;
        service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: current_generation,
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert!(
            service
                .commit_current_session_frame_checked(
                    "owner",
                    &original,
                    |_| true,
                    || -> AppResult<()> { panic!("stopped frame committed") }
                )
                .is_err()
        );
    }
    #[test]
    fn session_permission_recovery_resumes_the_existing_lease_but_preserves_user_pause() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target,
            Err(error(
                "preview_permission_revoked",
                "controlled OS revocation",
            )),
        );
        let revoked = service
            .consumer_update("owner", false)
            .unwrap()
            .unwrap()
            .projection;
        assert_eq!(revoked.state, PreviewState::PermissionRevoked);
        assert!(
            crate::ipc::reading::automatic_permission_resume_request(
                &revoked,
                PreviewPermission::Denied
            )
            .is_none()
        );
        let request = crate::ipc::reading::automatic_permission_resume_request(
            &revoked,
            PreviewPermission::Granted,
        )
        .unwrap();
        let recovered = service.control(request).unwrap();
        assert_eq!(recovered.lease_id.as_deref(), Some("lease"));
        assert_eq!(recovered.state, PreviewState::Starting);
        assert_ne!(recovered.generation, revoked.generation);
        let paused = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: recovered.generation,
                action: PreviewControl::Pause,
            })
            .unwrap();
        assert!(
            crate::ipc::reading::automatic_permission_resume_request(
                &paused,
                PreviewPermission::Granted
            )
            .is_none()
        );
        assert_eq!(
            service
                .consumer_update("owner", false)
                .unwrap()
                .unwrap()
                .projection
                .state,
            PreviewState::Paused
        );
    }
    #[test]
    fn old_generation_and_paused_capture_cannot_replace_current_frame_or_time() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "3".into(),
            target.clone(),
            Ok(frame(b"old", 1)),
        );
        assert!(
            service
                .latest_update("lease", "4", None)
                .unwrap()
                .frame
                .is_none()
        );
        let paused = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action: PreviewControl::Pause,
            })
            .unwrap();
        service.finish_capture("lease".into(), "4".into(), target, Ok(frame(b"late", 2)));
        let latest = service
            .latest_update("lease", &paused.generation, None)
            .unwrap();
        assert!(latest.frame.is_none());
        assert_eq!(latest.projection.captured_at_ms, None);
        assert_eq!(latest.projection.state, PreviewState::Paused);
    }
    #[test]
    fn latest_slot_replaces_old_frame_without_fabricating_content_change_time() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"same pixels", 100)),
        );
        let first = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(
            first.projection.content_changed_at_ms.as_deref(),
            Some("100")
        );
        assert!(
            service
                .latest_update("lease", "4", Some("1"))
                .unwrap()
                .frame
                .is_none()
        );
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target,
            Ok(frame(b"same pixels", 200)),
        );
        let next = service.latest_update("lease", "4", Some("1")).unwrap();
        assert_eq!(next.projection.captured_at_ms.as_deref(), Some("200"));
        assert_eq!(
            next.projection.content_changed_at_ms.as_deref(),
            Some("100")
        );
        assert_eq!(next.frame.unwrap().sequence, "2");
        assert_eq!(service.inner.lock().unwrap().leases.len(), 1);
    }
    #[test]
    fn paused_last_successful_frame_survives_reattach_without_accepting_stale_captures() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"last successful pixels", 321)),
        );
        let original = service
            .latest_update("lease", "4", None)
            .unwrap()
            .frame
            .unwrap();
        let paused = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action: PreviewControl::Pause,
            })
            .unwrap();
        assert_ne!(paused.generation, "4");
        assert_eq!(paused.state, PreviewState::Paused);
        assert_eq!(paused.captured_at_ms.as_deref(), Some("321"));
        assert_eq!(paused.content_changed_at_ms.as_deref(), Some("321"));
        let mut expected = original.clone();
        expected.generation = paused.generation.clone();
        let first = service
            .latest_update("lease", &paused.generation, None)
            .unwrap();
        assert_eq!(first.frame.as_ref(), Some(&expected));
        assert!(
            service
                .latest_update("lease", &paused.generation, Some(&original.sequence))
                .unwrap()
                .frame
                .is_none()
        );
        // A remounted component first reads the existing projection, then pulls
        // without a previous sequence. It must receive the same frozen image.
        let attached = service.consumer_update("owner", false).unwrap().unwrap();
        assert_eq!(attached.projection, paused);
        assert!(attached.frame.is_none());
        assert_eq!(
            service
                .latest_update("lease", &attached.projection.generation, None)
                .unwrap()
                .frame
                .as_ref(),
            Some(&expected)
        );
        assert_eq!(
            service.latest_update("lease", "4", None).unwrap_err().code,
            "preview_generation_conflict"
        );
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"late old capture", 999)),
        );
        let unchanged = service
            .latest_update("lease", &paused.generation, None)
            .unwrap();
        assert_eq!(unchanged.frame.as_ref(), Some(&expected));
        assert_eq!(unchanged.projection.captured_at_ms.as_deref(), Some("321"));
        assert_eq!(
            service
                .control(PreviewControlRequest {
                    lease_id: "lease".into(),
                    generation: "4".into(),
                    action: PreviewControl::Resume
                })
                .unwrap_err()
                .code,
            "preview_generation_conflict"
        );
        let resumed = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: paused.generation.clone(),
                action: PreviewControl::Resume,
            })
            .unwrap();
        assert_eq!(resumed.state, PreviewState::Starting);
        assert!(
            service
                .latest_update("lease", &resumed.generation, None)
                .unwrap()
                .frame
                .is_none()
        );
        assert_eq!(
            service
                .latest_update("lease", &paused.generation, None)
                .unwrap_err()
                .code,
            "preview_generation_conflict"
        );
        service.finish_capture(
            "lease".into(),
            paused.generation,
            target.clone(),
            Ok(frame(b"late paused generation", 1000)),
        );
        assert!(
            service
                .latest_update("lease", &resumed.generation, None)
                .unwrap()
                .frame
                .is_none()
        );
        service.finish_capture(
            "lease".into(),
            resumed.generation.clone(),
            target,
            Ok(frame(b"fresh resumed pixels", 777)),
        );
        assert_eq!(
            service
                .latest_update("lease", &resumed.generation, None)
                .unwrap()
                .frame
                .unwrap()
                .captured_at_ms,
            "777"
        );
        let stopped = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: resumed.generation,
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(stopped.state, PreviewState::Stopped);
        assert!(service.consumer_update("owner", true).unwrap().is_none());
        assert_eq!(
            service
                .latest_update("lease", &stopped.generation, None)
                .unwrap_err()
                .code,
            "preview_lease_expired"
        );
    }
    #[test]
    fn revoked_permission_preserves_last_frame_actual_time_and_is_not_live() {
        let (service, target) = fixture();
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"last", 100)),
        );
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target,
            Err(error("preview_permission_revoked", "denied")),
        );
        let latest = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(latest.projection.state, PreviewState::PermissionRevoked);
        assert_eq!(latest.projection.captured_at_ms.as_deref(), Some("100"));
        assert_eq!(latest.frame.unwrap().captured_at_ms, "100");
    }
    #[test]
    fn permission_query_does_not_mark_user_request() {
        let service = WindowPreviewService::new();
        assert!(!service.inner.lock().unwrap().permission_requested);
        let _ = service.capability();
        assert!(!service.inner.lock().unwrap().permission_requested);
        service.stop_all().unwrap();
    }
    #[test]
    fn expired_and_foreign_generations_cannot_read_frames() {
        let service = WindowPreviewService::new();
        assert!(service.latest_update("arbitrary", "1", None).is_err());
        assert!(
            service
                .control(PreviewControlRequest {
                    lease_id: "arbitrary".into(),
                    generation: "1".into(),
                    action: PreviewControl::Resume
                })
                .is_err()
        );
        service.stop_all().unwrap();
    }
    #[test]
    fn transient_pair_verification_retains_filter_and_last_frame_until_explicit_failure() {
        let (service, mut target) = fixture();
        let resource = Arc::new(());
        let retained = resource.clone();
        target.pairing = Some(Arc::new(PreviewPairing {
            _token: "owned-resource".into(),
            _keepalive: Arc::new(move || {
                let _ = &retained;
                Ok(())
            }),
        }));
        service
            .inner
            .lock()
            .unwrap()
            .leases
            .get_mut("lease")
            .unwrap()
            .backing = PreviewBacking::Resolved(target.clone());
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"valid", 12)),
        );
        for code in ["preview_capture_busy", "preview_verification_unavailable"] {
            service.finish_capture(
                "lease".into(),
                "4".into(),
                target.clone(),
                Err(error(code, "temporary")),
            );
            let update = service.latest_update("lease", "4", None).unwrap();
            assert_eq!(update.projection.state, PreviewState::Stale);
            assert_eq!(update.frame.unwrap().captured_at_ms, "12");
            assert!(service.has_resolved("lease", "4"));
        }
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Err(error("preview_selection_mismatch", "identity changed")),
        );
        assert!(!service.has_resolved("lease", "4"));
        assert!(
            service
                .latest_update("lease", "4", None)
                .unwrap()
                .frame
                .is_none()
        );
        drop(target);
        assert_eq!(Arc::strong_count(&resource), 1);
    }
    #[test]
    fn timed_out_stop_is_finally_released_when_the_owned_capture_drains() {
        let (service, mut target) = fixture();
        let resource = Arc::new(());
        let retained = resource.clone();
        target.pairing = Some(Arc::new(PreviewPairing {
            _token: "owned".into(),
            _keepalive: Arc::new(move || {
                let _ = &retained;
                Ok(())
            }),
        }));
        {
            let mut state = service.inner.lock().unwrap();
            state.leases.get_mut("lease").unwrap().backing =
                PreviewBacking::Resolved(target.clone());
            state.inflight = Some("lease".into());
        }
        let stopped = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(stopped.error_code.as_deref(), Some("preview_stop_timeout"));
        assert!(service.inner.lock().unwrap().leases.contains_key("lease"));
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target,
            Ok(frame(b"late pixels", 100)),
        );
        assert!(service.inner.lock().unwrap().leases.is_empty());
        assert_eq!(Arc::strong_count(&resource), 1);
        let mut next = failure_request();
        next.expected_generation = None;
        assert!(service.begin_resolution(next).is_ok());
    }
    #[test]
    fn late_native_cleanup_never_removes_a_replaced_generation() {
        let (service, target) = fixture();
        service.inner.lock().unwrap().inflight = Some("lease".into());
        let next = service.begin_resolution(failure_request()).unwrap();
        service.finish_capture("lease".into(), "4".into(), target, Ok(frame(b"old", 100)));
        assert!(service.is_current("lease", &next.generation));
        assert!(
            service
                .latest_update("lease", &next.generation, None)
                .unwrap()
                .frame
                .is_none()
        );
    }
    #[test]
    fn candidate_pair_transfer_retains_resource_but_rejects_old_candidate_stop_and_frames() {
        let (service, mut target) = fixture();
        target.pairing = Some(Arc::new(PreviewPairing {
            _token: "owned".into(),
            _keepalive: Arc::new(|| Ok(())),
        }));
        let candidate = target.candidate_id.clone();
        {
            let mut state = service.inner.lock().unwrap();
            let lease = state.leases.get_mut("lease").unwrap();
            lease.consumer = PreviewConsumer::Candidate;
            lease.request.consumer = PreviewConsumer::Candidate;
            lease.backing = PreviewBacking::Resolved(target.clone());
        }
        assert!(service.resolved_pairing("lease", "4", &candidate).is_some());
        assert!(
            service
                .transfer_pairing_to_session("lease", "3", &candidate, "new-session")
                .is_err()
        );
        assert!(
            service
                .transfer_pairing_to_session("lease", "4", "different-candidate", "new-session")
                .is_err()
        );
        let projection = service
            .transfer_pairing_to_session("lease", "4", &candidate, "new-session")
            .unwrap();
        assert!(service.session_matches_candidate("new-session", &candidate));
        assert!(
            service
                .resolved_pairing("lease", &projection.generation, &candidate)
                .is_none()
        );
        assert!(
            service
                .control(PreviewControlRequest {
                    lease_id: "lease".into(),
                    generation: "4".into(),
                    action: PreviewControl::Stop
                })
                .is_err()
        );
        service.finish_capture(
            "lease".into(),
            "4".into(),
            target.clone(),
            Ok(frame(b"candidate frame", 1)),
        );
        assert!(
            service
                .latest_update("lease", &projection.generation, None)
                .unwrap()
                .frame
                .is_none()
        );
        service.finish_capture(
            "lease".into(),
            projection.generation.clone(),
            target,
            Ok(frame(b"session frame", 2)),
        );
        let update = service
            .latest_update("lease", &projection.generation, None)
            .unwrap();
        assert_eq!(update.projection.consumer_id, "new-session");
        assert_eq!(update.frame.unwrap().captured_at_ms, "2");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod safe_capture_diagnostic_tests {
    use super::*;
    #[test]
    fn capture_projection_uses_only_regenerated_fixed_native_reason_details() {
        let fixed = "所选窗口身份或启动器归属无法核对，请重新选择对应的游戏窗口（核验阶段：owner_evidence_changed）";
        for (message, expected_detail) in [
            (fixed.to_string(), fixed),
            (
                format!("{fixed} /private/secret api_key=secret"),
                "当前窗口画面暂不可用，最后帧保留原采集时间",
            ),
        ] {
            let (service, target) = tests::fixture();
            service.finish_capture(
                "lease".into(),
                "4".into(),
                target,
                Err(error("preview_selection_mismatch", &message)),
            );
            let projection = service
                .consumer_update("owner", false)
                .unwrap()
                .unwrap()
                .projection;
            assert_eq!(projection.detail, expected_detail);
            assert_eq!(
                projection.error_code.as_deref(),
                Some("preview_selection_mismatch")
            );
            assert_eq!(projection.state, PreviewState::Unavailable);
        }
    }
}
