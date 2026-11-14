//! One lazy App-owned producer. Views consume immutable latest projections.
use super::runtime_observation::{self, Fact, Id, WorkerStage};
mod protocol;
mod worker;
use super::{
    macos::{self, CollectorProcessIdentity},
    window_preview::WindowPreviewService,
};
use app_core::{
    contracts::{AppError, AppResult, GameWindowBindingProjection, GameWindowBindingState},
    domain::content_hash,
    ports::IdentitySource,
};
use app_infra::{
    hook_backend::{FrozenPreviewTarget, PreviewProofProvider},
    platform::SystemIdentity,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const DRAIN_SECONDS: u64 = 40;
fn now() -> u64 {
    SystemIdentity.now_ms().max(0) as u64
}
fn id(prefix: &str) -> String {
    SystemIdentity.new_id(prefix)
}
fn failure(code: &str) -> AppError {
    AppError::new(code, "preview", "独立窗口采集暂不可用，取文不受影响")
}
fn io_error(_: io::Error) -> AppError {
    failure("preview_worker_transport")
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    v: u16,
    nonce: String,
    parent: CollectorProcessIdentity,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    v: u16,
    nonce: String,
    worker: CollectorProcessIdentity,
    instance: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: String,
    epoch: String,
    frame_epoch: String,
    paused: bool,
    target: FrozenPreviewTarget,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    v: u16,
    instance: String,
    request: u64,
    op: ControlOp,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum ControlOp {
    Prepare {
        source: Box<Source>,
        select: bool,
    },
    Fence {
        source: String,
        previous_epoch: String,
        epoch: String,
        frame_epoch: String,
        paused: bool,
        retire: Vec<(String, String)>,
    },
    Release {
        source: String,
        epoch: String,
    },
    Verify {
        source: String,
        epoch: String,
        capture: String,
        frame_epoch: String,
    },
    Shutdown,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    v: u16,
    instance: String,
    source: String,
    epoch: String,
    capture_id: String,
    frame_epoch: String,
    sequence: u64,
    phase: String,
    error_code: Option<String>,
    observed_at_ms: u64,
    permission: bool,
    target_identity: String,
    window_title: Option<String>,
    request: Option<u64>,
    verified_at_ms: Option<u64>,
    authority: Option<u64>,
    #[serde(default)]
    diagnostic: Option<macos::PreviewDiagnostic>,
}
/// Optional diagnostics never grant capture authority, but an explicitly
/// supplied malformed v4 payload is rejected rather than silently normalized.
fn valid_status_metadata(status: &Status) -> bool {
    status
        .diagnostic
        .as_ref()
        .is_none_or(|value| value.clone().checked().is_some())
}
impl Status {
    fn new(
        instance: &str,
        source: &Source,
        capture: &str,
        phase: &str,
        code: Option<String>,
    ) -> Self {
        Self {
            v: protocol::VERSION,
            instance: instance.into(),
            source: source.id.clone(),
            epoch: source.epoch.clone(),
            capture_id: capture.into(),
            frame_epoch: source.frame_epoch.clone(),
            sequence: 0,
            phase: phase.into(),
            error_code: code,
            observed_at_ms: now(),
            permission: false,
            target_identity: source.target.candidate_id().into(),
            window_title: source.target.window_title(),
            request: None,
            verified_at_ms: None,
            authority: None,
            diagnostic: None,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameHeader {
    v: u16,
    instance: String,
    source: String,
    capture_id: String,
    frame_epoch: String,
    epoch: String,
    sequence: u64,
    target_identity: String,
    width: u32,
    height: u32,
    captured_at_ms: u64,
    proof_before_ms: u64,
    proof_after_ms: u64,
    host_pid: u32,
    host_start: String,
    window_id: u32,
    content_hash: String,
}
impl FrameHeader {
    fn valid(&self, instance: &str, source: &Source, previous: u64, bytes: &[u8]) -> bool {
        self.v == protocol::VERSION
            && self.instance == instance
            && self.source == source.id
            && self.epoch == source.epoch
            && self.frame_epoch == source.frame_epoch
            && self.target_identity == source.target.candidate_id()
            && !self.capture_id.is_empty()
            && self.capture_id.len() <= 112
            && self.sequence > previous
            && self.width > 0
            && self.width <= 960
            && self.height > 0
            && self.height <= 540
            && self.host_pid > 0
            && self.window_id > 0
            && !self.host_start.is_empty()
            && self.host_start.len() <= 64
            && self.proof_before_ms > 0
            && self.captured_at_ms >= self.proof_before_ms
            && self.proof_after_ms >= self.captured_at_ms
            && !bytes.is_empty()
            && bytes.len() <= protocol::MAX_FRAME
            && self.content_hash == content_hash(bytes)
    }
}
#[derive(Clone, PartialEq, Eq)]
struct NativeFrameIdentity {
    host_pid: u32,
    host_start: String,
    window_id: u32,
}
#[derive(Default)]
struct FrameStreamGuard {
    sequence: u64,
    native: Option<NativeFrameIdentity>,
    capture: Option<String>,
}
impl FrameStreamGuard {
    fn accept(
        &mut self,
        instance: &str,
        source: &Source,
        header: &FrameHeader,
        bytes: &[u8],
    ) -> bool {
        if !header.valid(instance, source, self.sequence, bytes) {
            return false;
        }
        let native = NativeFrameIdentity {
            host_pid: header.host_pid,
            host_start: header.host_start.clone(),
            window_id: header.window_id,
        };
        if self.native.as_ref().is_some_and(|n| n != &native)
            || self
                .capture
                .as_ref()
                .is_some_and(|c| c != &header.capture_id)
        {
            return false;
        }
        self.native = Some(native);
        self.capture = Some(header.capture_id.clone());
        self.sequence = header.sequence;
        true
    }
}
struct Frame {
    header: FrameHeader,
    bytes: Vec<u8>,
}
struct Latest<T> {
    value: Mutex<Option<T>>,
    changed: Condvar,
}
impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new(None),
            changed: Condvar::new(),
        }
    }
}
impl<T> Latest<T> {
    fn replace(&self, value: T) {
        if let Ok(mut slot) = self.value.lock() {
            *slot = Some(value);
            self.changed.notify_one();
        }
    }
    fn clear(&self) {
        if let Ok(mut slot) = self.value.lock() {
            *slot = None;
        }
        self.changed.notify_all();
    }
    fn take(&self, stop: &AtomicBool) -> Option<T> {
        let mut slot = self.value.lock().ok()?;
        while slot.is_none() && !stop.load(Ordering::Acquire) {
            slot = self
                .changed
                .wait_timeout(slot, Duration::from_millis(100))
                .ok()?
                .0;
        }
        slot.take()
    }
}
fn valid_ready(
    ready: &Ready,
    nonce: &str,
    parent: &CollectorProcessIdentity,
    actual: &CollectorProcessIdentity,
) -> bool {
    ready.v == protocol::VERSION
        && ready.nonce == nonce
        && ready.worker == *actual
        && actual.dev == parent.dev
        && actual.ino == parent.ino
        && !ready.instance.is_empty()
        && ready.instance.len() <= 112
}
struct Child {
    pid: u32,
    identity: Option<CollectorProcessIdentity>,
    reaped: bool,
}
struct ParentSource {
    descriptor: Source,
    guard: FrameStreamGuard,
    status: Option<Status>,
    released: bool,
    accepting: bool,
    authority: u64,
}
struct PendingProof {
    source: String,
    epoch: String,
    frame_epoch: String,
    capture: String,
    candidate: String,
    sender: mpsc::SyncSender<AppResult<Status>>,
}
impl PendingProof {
    fn matches(&self, status: &Status) -> bool {
        status.phase == "Proof"
            && status.source == self.source
            && status.epoch == self.epoch
            && status.frame_epoch == self.frame_epoch
            && status.capture_id == self.capture
            && status.target_identity == self.candidate
    }
}
struct ParentState {
    child: Option<Child>,
    ready: Option<Ready>,
    control: Option<UnixStream>,
    sources: BTreeMap<String, ParentSource>,
    pending: BTreeMap<u64, PendingProof>,
    waiting: Option<(Source, bool)>,
}
pub(crate) struct CollectorSupervisor {
    state: Mutex<ParentState>,
    changed: Condvar,
    starting: AtomicBool,
    stopped: AtomicBool,
    request: AtomicU64,
    commands: mpsc::SyncSender<Control>,
    receiver: Mutex<Option<mpsc::Receiver<Control>>>,
    service: Weak<WindowPreviewService>,
}
impl CollectorSupervisor {
    pub(crate) fn new(service: Weak<WindowPreviewService>) -> Arc<Self> {
        let (sender, receiver) = mpsc::sync_channel(8);
        Arc::new(Self {
            state: Mutex::new(ParentState {
                child: None,
                ready: None,
                control: None,
                sources: BTreeMap::new(),
                pending: BTreeMap::new(),
                waiting: None,
            }),
            changed: Condvar::new(),
            starting: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            request: AtomicU64::new(0),
            commands: sender,
            receiver: Mutex::new(Some(receiver)),
            service,
        })
    }
    fn next_request(&self) -> AppResult<u64> {
        self.request
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map(|v| v + 1)
            .map_err(|_| failure("preview_generation_exhausted"))
    }
    fn enqueue_locked(&self, state: &ParentState, op: ControlOp) -> AppResult<u64> {
        let request = self.next_request()?;
        let instance = state
            .ready
            .as_ref()
            .map(|r| r.instance.clone())
            .unwrap_or_default();
        let command = Control {
            v: protocol::VERSION,
            instance,
            request,
            op,
        };
        protocol::write_json(&mut Vec::new(), &command)
            .map_err(|_| failure("preview_budget_exceeded"))?;
        self.commands
            .try_send(command)
            .map_err(|_| failure("preview_capture_busy"))?;
        Ok(request)
    }
    fn ensure(self: &Arc<Self>) -> AppResult<()> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(failure("preview_closed"));
        }
        {
            let state = self.state.lock().map_err(|_| failure("preview_state"))?;
            if state.child.is_some() {
                return if self.starting.load(Ordering::Acquire)
                    && state.child.as_ref().is_some_and(|child| !child.reaped)
                {
                    Ok(())
                } else {
                    Err(failure("preview_worker_unavailable"))
                };
            }
        }
        if self
            .starting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let parent = self.clone();
            std::thread::spawn(move || {
                let result = parent.connect();
                parent.stopped.store(true, Ordering::Release);
                if let Ok(state) = parent.state.lock()
                    && let Some(control) = &state.control
                {
                    let _ = control.shutdown(std::net::Shutdown::Write);
                }
                if let Err(error) = result {
                    parent.fail_all(error);
                }
                // Spawn ownership is retained even when handshake/status failed.
                // Reap only this exact child; never overwrite it with a retry.
                let _ = parent.drain_owned(Instant::now() + Duration::from_secs(DRAIN_SECONDS));
                parent.starting.store(false, Ordering::Release);
                parent.changed.notify_all();
            });
        }
        Ok(())
    }
    pub(crate) fn prepare(
        self: &Arc<Self>,
        lease: &str,
        epoch: &str,
        target: FrozenPreviewTarget,
        select: bool,
    ) -> AppResult<()> {
        self.ensure()?;
        let descriptor = Source {
            id: lease.into(),
            epoch: epoch.into(),
            frame_epoch: id("CollectorFrameEpoch"),
            paused: false,
            target,
        };
        let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
        if let Some(old) = state.sources.get(lease) {
            if old.descriptor.target.candidate_id() != descriptor.target.candidate_id()
                || old.released
            {
                return Err(failure("preview_generation_conflict"));
            }
            if !select && old.descriptor.epoch == epoch {
                return Ok(());
            }
        } else if state.waiting.as_ref().is_some_and(|(source, _)| {
            source.id == lease
                && source.epoch == epoch
                && source.target.candidate_id() == descriptor.target.candidate_id()
        }) {
            state.waiting = Some((descriptor, select));
            return Ok(());
        } else if state.sources.len() >= 2 {
            if state.waiting.is_some() {
                return Err(failure("preview_lease_limit"));
            }
            state.waiting = Some((descriptor, select));
            return Ok(());
        }
        let authority = self.enqueue_locked(
            &state,
            ControlOp::Prepare {
                source: Box::new(descriptor.clone()),
                select,
            },
        )?;
        let old = state.sources.remove(lease);
        let mut guard = old.map(|s| s.guard).unwrap_or_default();
        guard.sequence = 0;
        state.sources.insert(
            lease.into(),
            ParentSource {
                descriptor,
                guard,
                status: None,
                released: false,
                accepting: true,
                authority,
            },
        );
        Ok(())
    }
    pub(crate) fn fence(
        &self,
        lease: &str,
        previous: &str,
        epoch: &str,
        paused: bool,
    ) -> AppResult<()> {
        self.transfer(lease, previous, epoch, paused, &[])
    }
    pub(crate) fn transfer(
        &self,
        lease: &str,
        previous: &str,
        epoch: &str,
        paused: bool,
        retire: &[(String, String)],
    ) -> AppResult<()> {
        let frame_epoch = id("CollectorFrameEpoch");
        let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
        if state
            .waiting
            .as_ref()
            .is_some_and(|(source, _)| source.id == lease && source.epoch == previous)
        {
            if retire.len() > 1
                || retire.iter().any(|(id, old)| {
                    state
                        .sources
                        .get(id)
                        .is_none_or(|s| s.released || s.descriptor.epoch != *old)
                })
            {
                return Err(failure("preview_generation_conflict"));
            }
            for (id, old) in retire {
                self.enqueue_locked(
                    &state,
                    ControlOp::Release {
                        source: id.clone(),
                        epoch: old.clone(),
                    },
                )?;
            }
            for (id, _) in retire {
                let source = state.sources.get_mut(id).expect("validated retired source");
                source.released = true;
                source.status = None;
            }
            let (source, _) = state.waiting.as_mut().expect("checked pending source");
            source.epoch = epoch.into();
            source.frame_epoch = frame_epoch;
            source.paused = paused;
            return Ok(());
        }
        if state
            .sources
            .get(lease)
            .is_none_or(|s| s.descriptor.epoch != previous || s.released)
            || retire.len() > 1
            || retire.iter().any(|(id, old)| {
                id == lease
                    || state
                        .sources
                        .get(id)
                        .is_none_or(|s| s.descriptor.epoch != *old || s.released)
            })
        {
            return Err(failure("preview_generation_conflict"));
        }
        self.enqueue_locked(
            &state,
            ControlOp::Fence {
                source: lease.into(),
                previous_epoch: previous.into(),
                epoch: epoch.into(),
                frame_epoch: frame_epoch.clone(),
                paused,
                retire: retire.to_vec(),
            },
        )?;
        let source = state.sources.get_mut(lease).expect("checked source");
        source.descriptor.epoch = epoch.into();
        source.descriptor.frame_epoch = frame_epoch;
        source.descriptor.paused = paused;
        source.accepting = true;
        source.guard.sequence = 0;
        source.status = None;
        for (id, _) in retire {
            let source = state.sources.get_mut(id).expect("checked retired source");
            source.released = true;
            source.status = None;
        }
        Ok(())
    }
    pub(crate) fn release(&self, lease: &str, previous: &str) -> AppResult<bool> {
        let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
        if state
            .waiting
            .as_ref()
            .is_some_and(|(s, _)| s.id == lease && s.epoch == previous)
        {
            state.waiting = None;
            return Ok(true);
        }
        let Some(source) = state.sources.get(lease) else {
            return Ok(true);
        };
        if source.descriptor.epoch != previous || source.released {
            return Err(failure("preview_generation_conflict"));
        }
        self.enqueue_locked(
            &state,
            ControlOp::Release {
                source: lease.into(),
                epoch: previous.into(),
            },
        )?;
        let source = state.sources.get_mut(lease).expect("checked source");
        source.released = true;
        source.status = None;
        Ok(false)
    }
    fn promote_waiting(&self) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
        if state.sources.len() >= 2 {
            return Ok(());
        }
        let Some((descriptor, select)) = state.waiting.clone() else {
            return Ok(());
        };
        let authority = self.enqueue_locked(
            &state,
            ControlOp::Prepare {
                source: Box::new(descriptor.clone()),
                select,
            },
        )?;
        state.waiting = None;
        state.sources.insert(
            descriptor.id.clone(),
            ParentSource {
                descriptor,
                guard: FrameStreamGuard::default(),
                status: None,
                released: false,
                accepting: true,
                authority,
            },
        );
        Ok(())
    }
    pub(crate) fn cached_binding(
        &self,
        mut binding: GameWindowBindingProjection,
    ) -> GameWindowBindingProjection {
        // M02 owns the confirmed Windows association. Native selection/TCC and
        // an absent or transient worker frame cannot turn it into unbound.
        if binding.state != GameWindowBindingState::Bound {
            return binding;
        }
        let Ok(state) = self.state.lock() else {
            return binding;
        };
        let Some(source) = state.sources.values().find(|source| {
            !source.released
                && Some(source.descriptor.target.candidate_id())
                    == binding.target_identity_sha256.as_deref()
        }) else {
            return binding;
        };
        if let Some(status) = &source.status {
            // Only the complete Windows verifier emits this code. Native CG
            // owner/filter loss is separately classified as selection_expired.
            if status.error_code.as_deref() == Some("preview_target_closed") {
                binding.state = GameWindowBindingState::Stale;
                binding.error_code = status.error_code.clone();
            }
            binding.detail = format!(
                "已确认 Windows 关联 · 独立采集进程 {} · {} · source {} · epoch {} · capture {} · frame {} · seq {}{}",
                state.child.as_ref().map(|child| child.pid).unwrap_or(0),
                status.instance,
                status.source,
                status.epoch,
                status.capture_id,
                status.frame_epoch,
                status.sequence,
                status
                    .error_code
                    .as_ref()
                    .map(|code| format!(" · {code}"))
                    .unwrap_or_default()
            );
        }
        binding
    }
    fn connect(self: &Arc<Self>) -> AppResult<()> {
        let (mut control, child_control) = UnixStream::pair().map_err(io_error)?;
        let (frames, child_frames) = UnixStream::pair().map_err(io_error)?;
        control
            .set_read_timeout(Some(Duration::from_secs(4)))
            .map_err(io_error)?;
        control
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(io_error)?;
        let executable = std::env::current_exe()
            .map_err(io_error)?
            .canonicalize()
            .map_err(io_error)?;
        let parent = macos::collector_process_identity(std::process::id())?;
        let nonce = id("CollectorHandshake");
        let pid = {
            let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
            if self.stopped.load(Ordering::Acquire) {
                return Err(failure("preview_cancelled"));
            }
            let pid = macos::collector_spawn(
                &executable,
                child_control.as_raw_fd(),
                child_frames.as_raw_fd(),
            )?;
            state.child = Some(Child {
                pid,
                identity: None,
                reaped: false,
            });
            state.control = Some(control.try_clone().map_err(io_error)?);
            pid
        };
        runtime_observation::record(Some(Fact::Worker {
            stage: WorkerStage::Spawned,
            pid,
            instance: None,
        }));
        drop(child_control);
        drop(child_frames);
        let setup = (|| {
            protocol::write_json(
                &mut control,
                &Hello {
                    v: protocol::VERSION,
                    nonce: nonce.clone(),
                    parent: parent.clone(),
                },
            )
            .map_err(io_error)?;
            let ready: Ready = protocol::read_json(&mut control).map_err(io_error)?;
            let actual = macos::collector_process_identity(pid)?;
            if !valid_ready(&ready, &nonce, &parent, &actual) {
                return Err(failure("collector_handshake"));
            }
            {
                let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
                if let Some(child) = state.child.as_mut() {
                    child.identity = Some(actual);
                }
                state.ready = Some(ready.clone());
            }
            runtime_observation::record(Id::new(&ready.instance).map(|instance| Fact::Worker {
                stage: WorkerStage::HandshakeReady,
                pid,
                instance: Some(instance),
            }));
            control.set_read_timeout(None).map_err(io_error)?;
            let (mut sender, receiver) = (
                control.try_clone().map_err(io_error)?,
                self.receiver
                    .lock()
                    .map_err(|_| failure("preview_state"))?
                    .take()
                    .ok_or_else(|| failure("preview_state"))?,
            );
            let writer = self.clone();
            let instance = ready.instance.clone();
            std::thread::spawn(move || {
                while !writer.stopped.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_millis(50)) {
                        Ok(mut command) => {
                            command.instance = instance.clone();
                            if protocol::write_json(&mut sender, &command).is_err() {
                                break;
                            }
                            let _ = writer.promote_waiting();
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(_) => break,
                    }
                }
                let _ = sender.shutdown(std::net::Shutdown::Write);
            });
            let reader = self.clone();
            let frame_instance = ready.instance.clone();
            std::thread::spawn(move || reader.receive_frames(frames, &frame_instance));
            self.receive_statuses(&mut control, &ready.instance);
            Ok(())
        })();
        if setup.is_err() {
            self.stopped.store(true, Ordering::Release);
            if let Ok(state) = self.state.lock()
                && let Some(socket) = &state.control
            {
                let _ = socket.shutdown(std::net::Shutdown::Write);
            }
        }
        setup
    }
    fn receive_frames(&self, mut socket: UnixStream, instance: &str) {
        let mut transport_failed = false;
        loop {
            let (header, bytes) = match protocol::read_frame::<FrameHeader>(&mut socket) {
                Ok(frame) => frame,
                Err(_) => {
                    transport_failed = true;
                    break;
                }
            };
            if self.stopped.load(Ordering::Acquire) {
                break;
            }
            let accepted = {
                let Ok(mut state) = self.state.lock() else {
                    break;
                };
                state
                    .sources
                    .get_mut(&header.source)
                    .filter(|s| !s.released && s.accepting)
                    .is_some_and(|source| {
                        source
                            .guard
                            .accept(instance, &source.descriptor, &header, &bytes)
                    })
            };
            runtime_observation::record((|| {
                Some(Fact::CollectorFrame {
                    accepted,
                    source_id: Id::new(&header.source)?,
                    source_epoch: Id::new(&header.epoch)?,
                    frame_epoch: Id::new(&header.frame_epoch)?,
                    capture_id: Id::new(&header.capture_id)?,
                    worker_sequence: header.sequence,
                    target_candidate_id: Id::new(&header.target_identity)?,
                    host_pid: header.host_pid,
                    host_start: Id::new(&header.host_start)?,
                    window_id: header.window_id,
                    captured_at_ms: header.captured_at_ms,
                    content_hash: Id::new(&header.content_hash)?,
                    width: header.width,
                    height: header.height,
                    encoded_len: bytes.len(),
                })
            })());
            if accepted && let Some(service) = self.service.upgrade() {
                service.collector_frame(
                    &header.source,
                    &header.epoch,
                    &header.target_identity,
                    &header.capture_id,
                    header.sequence,
                    header.host_pid,
                    &header.host_start,
                    header.window_id,
                    macos::CapturedFrame {
                        bytes,
                        width: header.width,
                        height: header.height,
                        captured_at_ms: header.captured_at_ms,
                    },
                );
            } else if !accepted {
                let current = self.state.lock().ok().and_then(|state| {
                    state
                        .sources
                        .get(&header.source)
                        .filter(|source| {
                            !source.released
                                && source.descriptor.epoch == header.epoch
                                && source.descriptor.frame_epoch == header.frame_epoch
                        })
                        .map(|source| source.authority)
                });
                if let Some(request) = current
                    && let Some(service) = self.service.upgrade()
                {
                    let mut diagnostic = macos::PreviewDiagnostic::worker("frame_rejected");
                    diagnostic.request = request;
                    diagnostic.reason = Some("frame_guard".into());
                    service.collector_diagnostic(&header.source, &header.epoch, diagnostic);
                }
            }
        }
        if transport_failed && !self.stopped.load(Ordering::Acquire) {
            let mut diagnostic = macos::PreviewDiagnostic::worker("frame_read");
            diagnostic.reason = Some("frame_transport".into());
            self.fail_all_diagnostic(failure("preview_frame_transport"), Some(diagnostic));
        }
    }
    fn receive_statuses(&self, control: &mut UnixStream, instance: &str) {
        while let Ok(mut status) = protocol::read_json::<Status>(control) {
            if status.v != protocol::VERSION
                || status.instance != instance
                || !valid_status_metadata(&status)
            {
                break;
            }
            let diagnostic = status.diagnostic.take();
            let (mut released, mut proof) = (None, None);
            let mut recovery_fact = None;
            {
                let Ok(mut state) = self.state.lock() else {
                    break;
                };
                if let Some(request) = status.request
                    && state
                        .pending
                        .get(&request)
                        .is_some_and(|pending| pending.matches(&status))
                {
                    proof = state
                        .pending
                        .remove(&request)
                        .map(|pending| (pending.sender, status.clone()));
                }
                if let Some(source) = state.sources.get_mut(&status.source)
                    && source.descriptor.epoch == status.epoch
                    && (status.phase == "Released"
                        || (source.descriptor.target.candidate_id() == status.target_identity
                            && source.descriptor.frame_epoch == status.frame_epoch))
                {
                    if status.phase == "Released" && source.released {
                        released = Some(status.source.clone());
                    } else if !source.released && status.request.is_none() {
                        status.diagnostic = diagnostic
                            .filter(|value| value.request == source.authority)
                            .and_then(macos::PreviewDiagnostic::checked);
                        if let Some(diagnostic) = status.diagnostic.as_ref() {
                            let key = runtime_observation::capture_recovery_key(diagnostic);
                            let previous = source
                                .status
                                .as_ref()
                                .and_then(|old| old.diagnostic.as_ref())
                                .and_then(runtime_observation::capture_recovery_key);
                            if key.is_some() && key != previous {
                                recovery_fact = Fact::capture_recovery(
                                    &status.source,
                                    &status.epoch,
                                    &status.frame_epoch,
                                    (!status.capture_id.is_empty())
                                        .then_some(status.capture_id.as_str()),
                                    &status.target_identity,
                                    diagnostic,
                                );
                            }
                        }
                        if status.phase == "CaptureReady"
                            && status.authority == Some(source.authority)
                            && status.verified_at_ms.is_some()
                            && !status.capture_id.is_empty()
                        {
                            if source.guard.capture.as_deref() != Some(status.capture_id.as_str()) {
                                source.guard = FrameStreamGuard {
                                    capture: Some(status.capture_id.clone()),
                                    ..Default::default()
                                };
                            }
                            source.accepting = true;
                        }
                        if status.phase == "Live" && status.verified_at_ms.is_some() {
                            source.accepting = true;
                        }
                        if matches!(
                            status.error_code.as_deref(),
                            Some(
                                "preview_target_closed"
                                    | "preview_selection_expired"
                                    | "preview_selection_mismatch"
                                    | "preview_permission_revoked"
                                    | "preview_permission_denied"
                            )
                        ) {
                            source.accepting = false;
                        }
                        source.status = Some(status.clone());
                    }
                }
                if let Some(source) = &released {
                    state.sources.remove(source);
                }
            }
            if let Some(fact) = recovery_fact {
                runtime_observation::record(Some(fact));
            }
            if let Some((sender, status)) = proof {
                let _ = sender.try_send(Ok(status));
            }
            if let Some(source) = &released {
                runtime_observation::record((|| {
                    Some(Fact::SourceReleased {
                        source_id: Id::new(source)?,
                        source_epoch: Id::new(&status.epoch)?,
                    })
                })());
            }
            if let Some(service) = self.service.upgrade() {
                if let Some(source) = released {
                    service.collector_released(&source);
                } else if status.request.is_none() {
                    service.collector_status(
                        &status.source,
                        &status.epoch,
                        super::window_preview::CollectorStatusUpdate {
                            phase: &status.phase,
                            code: status.error_code.as_deref(),
                            permission: status.permission,
                            observed_at_ms: status.observed_at_ms,
                            diagnostic: status.diagnostic.as_ref(),
                        },
                    );
                }
            }
            let _ = self.promote_waiting();
        }
        self.stopped.store(true, Ordering::Release);
        if let Ok(state) = self.state.lock()
            && let Some(control) = &state.control
        {
            let _ = control.shutdown(std::net::Shutdown::Write);
        }
        self.fail_all(failure("preview_worker_unavailable"));
    }
    fn fail_all(&self, error: AppError) {
        self.fail_all_diagnostic(error, None);
    }
    fn fail_all_diagnostic(&self, error: AppError, diagnostic: Option<macos::PreviewDiagnostic>) {
        let sources = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            for sender in std::mem::take(&mut state.pending).into_values() {
                let _ = sender.sender.try_send(Err(error.clone()));
            }
            state
                .sources
                .values()
                .map(|s| {
                    (
                        s.descriptor.id.clone(),
                        s.descriptor.epoch.clone(),
                        s.authority,
                    )
                })
                .collect::<Vec<_>>()
        };
        if let Some(service) = self.service.upgrade() {
            for (id, epoch, authority) in sources {
                let diagnostic = diagnostic.clone().map(|mut value| {
                    value.request = authority;
                    value
                });
                service.collector_status(
                    &id,
                    &epoch,
                    super::window_preview::CollectorStatusUpdate {
                        phase: "Unavailable",
                        code: Some(&error.code),
                        permission: false,
                        observed_at_ms: now(),
                        diagnostic: diagnostic.as_ref(),
                    },
                );
            }
        }
        self.changed.notify_all();
    }
    fn begin_proof(
        &self,
        source: &str,
        epoch: &str,
        frame_epoch: &str,
        capture: &str,
    ) -> AppResult<(u64, mpsc::Receiver<AppResult<Status>>)> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
        if !state.pending.is_empty() {
            return Err(failure("preview_capture_busy"));
        }
        if state.sources.get(source).is_none_or(|s| {
            s.released
                || s.descriptor.epoch != epoch
                || s.descriptor.frame_epoch != frame_epoch
                || s.guard.capture.as_deref() != Some(capture)
        }) {
            return Err(failure("preview_generation_conflict"));
        }
        let request = self.enqueue_locked(
            &state,
            ControlOp::Verify {
                source: source.into(),
                epoch: epoch.into(),
                frame_epoch: frame_epoch.into(),
                capture: capture.into(),
            },
        )?;
        let candidate = state.sources[source]
            .descriptor
            .target
            .candidate_id()
            .to_owned();
        state.pending.insert(
            request,
            PendingProof {
                source: source.into(),
                epoch: epoch.into(),
                frame_epoch: frame_epoch.into(),
                capture: capture.into(),
                candidate,
                sender,
            },
        );
        Ok((request, receiver))
    }
    fn drain_owned(&self, deadline: Instant) -> AppResult<()> {
        loop {
            let mut reaped = None;
            let done = {
                let mut state = self.state.lock().map_err(|_| failure("preview_state"))?;
                if let Some(child) = state.child.as_mut() {
                    if !child.reaped && macos::collector_reap(child.pid)?.is_some() {
                        child.reaped = true;
                        reaped = Some(child.pid);
                    }
                    child.reaped
                } else {
                    true
                }
            };
            if let Some(pid) = reaped {
                runtime_observation::record(Some(Fact::Worker {
                    stage: WorkerStage::Reaped,
                    pid,
                    instance: None,
                }));
            }
            if done {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(failure("preview_cleanup_pending"));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    pub(crate) fn shutdown_and_wait(&self) -> AppResult<()> {
        self.stopped.store(true, Ordering::Release);
        if let Ok(state) = self.state.lock()
            && let Some(socket) = &state.control
        {
            let _ = socket.shutdown(std::net::Shutdown::Write);
        }
        let deadline = Instant::now() + Duration::from_secs(DRAIN_SECONDS);
        while self.starting.load(Ordering::Acquire)
            && self
                .state
                .lock()
                .map_err(|_| failure("preview_state"))?
                .child
                .is_none()
        {
            if Instant::now() >= deadline {
                return Err(failure("preview_cleanup_pending"));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        self.drain_owned(deadline)
    }
}
impl PreviewProofProvider for CollectorSupervisor {
    fn verify_bound_window(&self, candidate_id: &str) -> AppResult<GameWindowBindingProjection> {
        let (source, epoch, frame_epoch, capture) = {
            let state = self.state.lock().map_err(|_| failure("preview_state"))?;
            let source = state
                .sources
                .values()
                .find(|s| !s.released && s.descriptor.target.candidate_id() == candidate_id)
                .ok_or_else(|| failure("preview_verification_unavailable"))?;
            let capture = source
                .guard
                .capture
                .clone()
                .ok_or_else(|| failure("preview_verification_unavailable"))?;
            (
                source.descriptor.id.clone(),
                source.descriptor.epoch.clone(),
                source.descriptor.frame_epoch.clone(),
                capture,
            )
        };
        let (request, receiver) = self.begin_proof(&source, &epoch, &frame_epoch, &capture)?;
        let result = receiver
            .recv_timeout(Duration::from_secs(150))
            .map_err(|_| failure("preview_capture_busy"));
        let status = result??;
        let state = self.state.lock().map_err(|_| failure("preview_state"))?;
        let current = state
            .sources
            .get(&source)
            .filter(|s| {
                !s.released
                    && s.descriptor.epoch == epoch
                    && s.guard.capture.as_deref() == Some(capture.as_str())
                    && s.descriptor.frame_epoch == frame_epoch
            })
            .ok_or_else(|| failure("preview_generation_conflict"))?;
        if status.source != source
            || status.epoch != epoch
            || status.capture_id != capture
            || status.frame_epoch != frame_epoch
            || status.target_identity != candidate_id
            || status.request != Some(request)
            || status.error_code.is_some()
            || status.verified_at_ms.is_none()
        {
            return Err(failure(
                status
                    .error_code
                    .as_deref()
                    .unwrap_or("preview_verification_unavailable"),
            ));
        }
        Ok(GameWindowBindingProjection {
            state: GameWindowBindingState::Bound,
            session_id: None,
            target_identity_sha256: Some(candidate_id.into()),
            window_title: current.descriptor.target.window_title(),
            verified_at_ms: status.verified_at_ms.map(|t| t.to_string()),
            error_code: None,
            detail: "独立采集进程已完成本次新鲜窗口核验".into(),
        })
    }
}
impl Drop for CollectorSupervisor {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(state) = self.state.lock()
            && let Some(socket) = &state.control
        {
            let _ = socket.shutdown(std::net::Shutdown::Write);
        }
    }
}
/// Strict early mode dispatch. A bare flag never enters AppKit or TCC.
pub(crate) fn dispatch_worker() -> Option<i32> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.iter().any(|s| s == "--capture-worker") {
        return None;
    }
    if args != ["--capture-worker", "--control-fd", "3", "--frame-fd", "4"]
        || !macos::collector_private_fd(3)
        || !macos::collector_private_fd(4)
    {
        return Some(64);
    }
    // SAFETY: Native verified two private, distinct, owned Unix sockets and set CLOEXEC.
    let control = unsafe { UnixStream::from_raw_fd(3) };
    // SAFETY: Descriptor 4 is distinct from descriptor 3 and owned by this worker mode.
    let frames = unsafe { UnixStream::from_raw_fd(4) };
    Some(if worker::run(control, frames).is_ok() {
        0
    } else {
        65
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    pub(super) fn source() -> Source {
        // Transport fixture only: never passed to the real verifier as evidence.
        let target = serde_json::from_value(serde_json::json!({
            "candidate_id":"frozen-target", "candidate":{
                "target":{"pid":12,"exe":"Fixture.exe","full_image_path":"C:\\Fixture.exe","creation_filetime":"99","architecture":"x64","errors":[]},
                "window":null,"settings":{"wine_path":"/fixture/wine","bottle":"fixture"},"environment_key":"crossover:/fixture"
            },"image_path":"/fixture/drive_c/Fixture.exe","file_stamp":[1,2,3,4,5,6,7],"resource_manifest_sha256":"fixture-resource"
        })).unwrap();
        Source {
            id: "source".into(),
            epoch: "1".into(),
            frame_epoch: "frame-1".into(),
            paused: false,
            target,
        }
    }
    pub(super) fn ready() -> Ready {
        Ready {
            v: protocol::VERSION,
            nonce: "valid-nonce-for-owned-test-session-0000".into(),
            worker: CollectorProcessIdentity {
                pid: 123,
                start: "1:2".into(),
                dev: 1,
                ino: 2,
            },
            instance: "instance".into(),
        }
    }
    pub(super) fn frame(sequence: u64, size: usize) -> Frame {
        let bytes = vec![7; size];
        let source = source();
        Frame {
            header: FrameHeader {
                v: protocol::VERSION,
                instance: ready().instance,
                source: source.id,
                capture_id: "capture".into(),
                epoch: source.epoch,
                frame_epoch: source.frame_epoch,
                target_identity: source.target.candidate_id().into(),
                sequence,
                width: 960,
                height: 540,
                captured_at_ms: 2,
                proof_before_ms: 1,
                proof_after_ms: 3,
                host_pid: 4,
                host_start: "1:2".into(),
                window_id: 5,
                content_hash: content_hash(&bytes),
            },
            bytes,
        }
    }
    fn parent() -> Arc<CollectorSupervisor> {
        let parent = CollectorSupervisor::new(Weak::new());
        let source = source();
        let mut state = parent.state.lock().unwrap();
        state.ready = Some(ready());
        state.sources.insert(
            source.id.clone(),
            ParentSource {
                descriptor: source,
                guard: FrameStreamGuard {
                    capture: Some("capture".into()),
                    ..Default::default()
                },
                status: None,
                released: false,
                accepting: true,
                authority: 1,
            },
        );
        drop(state);
        parent
    }
    pub(super) fn recovery_diagnostic_fixture() -> macos::PreviewDiagnostic {
        let mut diagnostic = macos::PreviewDiagnostic::worker("capture_complete");
        diagnostic.geometry = Some(macos::PreviewGeometrySnapshot {
            cg_width_points: Some(1462.0),
            cg_height_points: Some(850.0),
            cg_on_screen: Some(false),
            filter_width_points: Some(1462.0),
            filter_height_points: Some(850.0),
            point_pixel_scale: Some(2.0),
            request_width: Some(928),
            request_height: Some(540),
            compatibility_source_cap: Some(true),
            reason: None,
            sc_window_frame_query: "not_queried".into(),
            sc_window_width_points: None,
            sc_window_height_points: None,
        });
        diagnostic.failure_kind = Some(macos::CaptureFailureKind::CompatibilitySourceCap);
        diagnostic.post_proof = Some("post_verified_retain".into());
        diagnostic.final_stage = Some("compatibility_bounds".into());
        diagnostic.final_code = Some("preview_budget_exceeded".into());
        diagnostic
    }
    #[test]
    fn private_v4_status_metadata_refuses_typed_incomplete_or_nonfinite_fields() {
        let mut status = Status::new(
            "instance",
            &source(),
            "capture",
            "Unavailable",
            Some("preview_budget_exceeded".into()),
        );
        assert!(valid_status_metadata(&status));
        status.diagnostic = Some(recovery_diagnostic_fixture());
        assert!(valid_status_metadata(&status));
        status.diagnostic.as_mut().unwrap().failure_kind = None;
        assert!(!valid_status_metadata(&status));
        status.diagnostic = Some(recovery_diagnostic_fixture());
        status.diagnostic.as_mut().unwrap().geometry = None;
        assert!(!valid_status_metadata(&status));
        status.diagnostic = Some(recovery_diagnostic_fixture());
        status
            .diagnostic
            .as_mut()
            .unwrap()
            .geometry
            .as_mut()
            .unwrap()
            .point_pixel_scale = Some(f64::INFINITY);
        assert!(!valid_status_metadata(&status));
    }

    #[test]
    fn ready_authentication_preserves_version_nonce_pid_start_mapped_image_and_instance_checks() {
        let original = ready();
        let actual = original.worker.clone();
        assert!(valid_ready(&original, &original.nonce, &actual, &actual));
        for field in 0..7 {
            let mut wrong = original.clone();
            match field {
                0 => wrong.v += 1,
                1 => wrong.nonce.push('x'),
                2 => wrong.worker.pid += 1,
                3 => wrong.worker.start.push('x'),
                4 => wrong.worker.dev += 1,
                5 => wrong.worker.ino += 1,
                _ => wrong.instance.clear(),
            };
            assert!(!valid_ready(&wrong, &original.nonce, &actual, &actual));
        }
    }
    #[test]
    fn frame_identity_epoch_sequence_time_and_payload_are_checked_before_cache() {
        let original = frame(3, 10);
        let source = source();
        assert!(
            original
                .header
                .valid("instance", &source, 2, &original.bytes)
        );
        for field in 0..13 {
            let mut h = original.header.clone();
            match field {
                0 => h.epoch.push('x'),
                1 => h.instance.push('x'),
                2 => h.target_identity.push('x'),
                3 => h.sequence = 2,
                4 => h.width = 961,
                5 => h.captured_at_ms = 0,
                6 => h.content_hash.push('x'),
                7 => h.frame_epoch.push('x'),
                8 => h.host_pid = 0,
                9 => h.host_start.clear(),
                10 => h.window_id = 0,
                11 => h.v += 1,
                _ => h.proof_after_ms = 1,
            };
            assert!(
                !h.valid("instance", &source, 2, &original.bytes),
                "field {field}"
            );
        }
    }
    #[test]
    fn native_and_capture_pin_survive_frame_epoch_transfer_and_pause() {
        let mut source = source();
        let mut guard = FrameStreamGuard::default();
        let first = frame(1, 10);
        assert!(guard.accept("instance", &source, &first.header, &first.bytes));
        source.epoch = "2".into();
        source.frame_epoch = "frame-2".into();
        guard.sequence = 0;
        for field in 0..4 {
            let mut changed = frame(1, 10);
            changed.header.epoch = source.epoch.clone();
            changed.header.frame_epoch = source.frame_epoch.clone();
            match field {
                0 => changed.header.host_pid += 1,
                1 => changed.header.host_start.push('x'),
                2 => changed.header.window_id += 1,
                _ => changed.header.capture_id.push('x'),
            };
            assert!(!guard.accept("instance", &source, &changed.header, &changed.bytes));
        }
        let mut next = first;
        next.header.epoch = source.epoch.clone();
        next.header.frame_epoch = source.frame_epoch.clone();
        assert!(guard.accept("instance", &source, &next.header, &next.bytes));
        assert!(!guard.accept("instance", &source, &frame(3, 10).header, &[7; 10]));
    }
    #[test]
    fn closing_an_empty_latest_slot_wakes_its_real_waiter() {
        let latest = Arc::new(Latest::<u64>::default());
        let stop = Arc::new(AtomicBool::new(false));
        let slot = latest.clone();
        let stopped = stop.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let join = std::thread::spawn(move || tx.send(slot.take(&stopped)).unwrap());
        stop.store(true, Ordering::Release);
        latest.changed.notify_all();
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), None);
        join.join().unwrap();
    }
    #[test]
    fn concurrently_admitted_commands_keep_monotonic_request_order() {
        let parent = parent();
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        let threads = (0..8)
            .map(|_| {
                let parent = parent.clone();
                std::thread::spawn(move || {
                    let state = parent.state.lock().unwrap();
                    parent.enqueue_locked(&state, ControlOp::Shutdown).unwrap()
                })
            })
            .collect::<Vec<_>>();
        for join in threads {
            join.join().unwrap();
        }
        let requests = receiver.try_iter().map(|c| c.request).collect::<Vec<_>>();
        assert_eq!(requests.len(), 8);
        assert!(requests.windows(2).all(|w| w[0] < w[1]));
    }
    #[test]
    fn diagnostic_status_requires_the_current_source_request_before_publication() {
        for current in [false, true] {
            let parent = parent();
            let (descriptor, authority) = {
                let state = parent.state.lock().unwrap();
                let source = &state.sources["source"];
                (source.descriptor.clone(), source.authority)
            };
            let mut status = Status::new("instance", &descriptor, "", "Selecting", None);
            let mut diagnostic = macos::PreviewDiagnostic::worker("picker_wait");
            diagnostic.request = if current { authority } else { authority + 1 };
            status.diagnostic = Some(diagnostic);
            let (mut reading, mut writing) = UnixStream::pair().unwrap();
            protocol::write_json(&mut writing, &status).unwrap();
            writing.shutdown(std::net::Shutdown::Write).unwrap();
            parent.receive_statuses(&mut reading, "instance");
            let state = parent.state.lock().unwrap();
            let saved = state.sources["source"].status.as_ref().unwrap();
            assert_eq!(saved.diagnostic.is_some(), current);
        }
    }
    #[test]
    fn actual_frame_channel_eof_reports_only_fixed_transport_diagnostic_without_a_frame() {
        let (service, parent, _requests) = cache_parent(false);
        let authority = parent.state.lock().unwrap().sources["lease"].authority;
        let (reading, writing) = UnixStream::pair().unwrap();
        writing.shutdown(std::net::Shutdown::Write).unwrap();
        parent.receive_frames(reading, "instance");
        let update = service.latest_update("lease", "4", None).unwrap();
        assert_eq!(
            update.projection.error_code.as_deref(),
            Some("preview_frame_transport")
        );
        assert!(update.projection.detail.contains("frame_read"));
        assert!(
            update
                .projection
                .detail
                .contains(&format!("请求 {authority}"))
        );
        assert!(update.frame.is_none());
        assert!(update.projection.captured_at_ms.is_none());
        assert!(!parent.stopped.load(Ordering::Acquire));
    }
    #[test]
    fn full_control_queue_does_not_commit_fence_release_or_partial_transfer() {
        let parent = parent();
        {
            let state = parent.state.lock().unwrap();
            for _ in 0..8 {
                parent.enqueue_locked(&state, ControlOp::Shutdown).unwrap();
            }
        }
        for paused in [false, true] {
            assert_eq!(
                parent.fence("source", "1", "2", paused).unwrap_err().code,
                "preview_capture_busy"
            );
            assert_eq!(
                parent.state.lock().unwrap().sources["source"]
                    .descriptor
                    .epoch,
                "1"
            );
        }
        assert_eq!(
            parent.release("source", "1").unwrap_err().code,
            "preview_capture_busy"
        );
        assert!(!parent.state.lock().unwrap().sources["source"].released);
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        for _ in 0..8 {
            receiver.recv().unwrap();
        }
        parent.fence("source", "1", "2", false).unwrap();
        assert_eq!(
            parent.state.lock().unwrap().sources["source"]
                .descriptor
                .epoch,
            "2"
        );
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Fence { .. }
        ));
        assert_eq!(
            parent.release("source", "1").unwrap_err().code,
            "preview_generation_conflict"
        );
        parent.release("source", "2").unwrap();
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Release { .. }
        ));
    }
    #[test]
    fn fresh_proof_detached_waiter_still_occupies_one_physical_request_until_ack() {
        let parent = parent();
        let source = source();
        let (request, waiting) = parent
            .begin_proof(&source.id, &source.epoch, &source.frame_epoch, "capture")
            .unwrap();
        drop(waiting);
        for _ in 0..20 {
            assert_eq!(
                parent
                    .begin_proof(&source.id, &source.epoch, &source.frame_epoch, "capture")
                    .unwrap_err()
                    .code,
                "preview_capture_busy"
            );
        }
        assert_eq!(parent.state.lock().unwrap().pending.len(), 1);
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        let commands = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].request, request);
        parent.release("source", "1").unwrap();
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Release { .. }
        ));
        parent.fail_all(failure("preview_cancelled"));
        assert!(parent.state.lock().unwrap().pending.is_empty());
    }
    #[test]
    fn owned_child_is_reaped_after_shutdown_and_not_overwritten_after_handshake_failure() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("0.15")
            .spawn()
            .unwrap();
        let pid = child.id();
        let parent = CollectorSupervisor::new(Weak::new());
        parent.state.lock().unwrap().child = Some(Child {
            pid,
            identity: None,
            reaped: false,
        });
        assert_eq!(
            parent.ensure().unwrap_err().code,
            "preview_worker_unavailable"
        );
        assert_eq!(
            parent.state.lock().unwrap().child.as_ref().unwrap().pid,
            pid
        );
        parent.shutdown_and_wait().unwrap();
        assert!(parent.state.lock().unwrap().child.as_ref().unwrap().reaped);
        let _ = child.wait();
    }
    #[test]
    fn private_socket_descriptor_validation_rejects_files_and_prevents_following_exec_inheritance()
    {
        unsafe extern "C" {
            fn fcntl(fd: i32, command: i32, ...) -> i32;
            fn luna_collector_fd_probe_for_test(
                control: i32,
                frame: i32,
                leaked: i32,
                pid: *mut i32,
            ) -> i32;
        }
        let (control, _) = UnixStream::pair().unwrap();
        let (frames, _) = UnixStream::pair().unwrap();
        let file = OpenOptions::new().read(true).open("/dev/null").unwrap();
        assert!(!macos::collector_private_fd(file.as_raw_fd()));
        // Positive control: std::Command preserves these sockets when we
        // intentionally clear CLOEXEC. This prevents a false green result if
        // the exec implementation itself starts closing unrelated descriptors.
        for socket in [&control, &frames] {
            // SAFETY: F_SETFD changes only our live owned socket's FD flags.
            assert_eq!(unsafe { fcntl(socket.as_raw_fd(), 2, 0) }, 0);
        }
        let visible = format!(
            "test -e /dev/fd/{} && test -e /dev/fd/{}",
            control.as_raw_fd(),
            frames.as_raw_fd()
        );
        assert!(
            std::process::Command::new("/bin/sh")
                .args(["-c", &visible])
                .status()
                .unwrap()
                .success()
        );
        assert!(macos::collector_private_fd(control.as_raw_fd()));
        assert!(macos::collector_private_fd(frames.as_raw_fd()));
        // SAFETY: F_DUPFD duplicates our owned regular-file FD above 10. The
        // new descriptor is intentionally non-CLOEXEC and solely owned below.
        let leaked = unsafe { fcntl(file.as_raw_fd(), 0, 10) };
        assert!(leaked >= 10);
        // SAFETY: fcntl returned one new owned descriptor, disjoint from sockets.
        let leaked = unsafe { std::os::fd::OwnedFd::from_raw_fd(leaked) };
        let mut pid = 0;
        // SAFETY: Fixed test-only native probe uses the exact production spawn
        // actions; all descriptor arguments remain live and pid is writable.
        let result = unsafe {
            luna_collector_fd_probe_for_test(
                control.as_raw_fd(),
                frames.as_raw_fd(),
                leaked.as_raw_fd(),
                &mut pid,
            )
        };
        assert_eq!(result, 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        let code = loop {
            if let Some(code) = macos::collector_reap(pid as u32).unwrap() {
                break code;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(code, 0);
        // A following exec after the worker's private_fd call cannot inherit
        // those owned control/data FDs, independently of the parent's allowlist.
        let script = format!(
            "test ! -e /dev/fd/{} && test ! -e /dev/fd/{}",
            control.as_raw_fd(),
            frames.as_raw_fd()
        );
        assert!(
            std::process::Command::new("/bin/sh")
                .args(["-c", &script])
                .status()
                .unwrap()
                .success()
        );
    }
    fn cache_parent(
        candidate: bool,
    ) -> (
        Arc<WindowPreviewService>,
        Arc<CollectorSupervisor>,
        mpsc::Receiver<Control>,
    ) {
        use super::super::window_preview::PreviewPairing;
        let service = WindowPreviewService::coordinator_test_fixture(Arc::new(PreviewPairing {
            _token: "owned-cache-test".into(),
            _keepalive: Arc::new(|| Ok(())),
        }));
        service.coordinator_test_remote(candidate);
        let parent = CollectorSupervisor::new(Arc::downgrade(&service));
        let mut source = source();
        source.id = "lease".into();
        source.epoch = "4".into();
        let mut wire = serde_json::to_value(&source.target).unwrap();
        wire["candidate_id"] = "server-candidate".into();
        source.target = serde_json::from_value(wire).unwrap();
        {
            let mut state = parent.state.lock().unwrap();
            state.ready = Some(ready());
            state.sources.insert(
                "lease".into(),
                ParentSource {
                    descriptor: source,
                    guard: FrameStreamGuard {
                        capture: Some("capture".into()),
                        ..Default::default()
                    },
                    status: None,
                    released: false,
                    accepting: true,
                    authority: 1,
                },
            );
        }
        service.install_collector(parent.clone()).unwrap();
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        (service, parent, receiver)
    }
    #[test]
    fn full_queue_through_actual_service_pause_resume_and_stop_preserves_both_authorities_until_retry()
     {
        use app_core::contracts::window_capabilities::*;
        let (service, parent, receiver) = cache_parent(false);
        {
            let state = parent.state.lock().unwrap();
            for _ in 0..8 {
                parent.enqueue_locked(&state, ControlOp::Shutdown).unwrap();
            }
        }
        for action in [
            PreviewControl::Pause,
            PreviewControl::Resume,
            PreviewControl::Stop,
        ] {
            let request = PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action,
            };
            assert_eq!(
                service.control(request).unwrap_err().code,
                "preview_capture_busy"
            );
            let actual = service
                .consumer_update("owner", false)
                .unwrap()
                .unwrap()
                .projection;
            assert_eq!(actual.generation, "4");
            assert_eq!(
                parent.state.lock().unwrap().sources["lease"]
                    .descriptor
                    .epoch,
                "4"
            );
            assert!(!parent.state.lock().unwrap().sources["lease"].released);
        }
        for _ in 0..8 {
            receiver.recv().unwrap();
        }
        let resumed = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action: PreviewControl::Resume,
            })
            .unwrap();
        assert_ne!(resumed.generation, "4");
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Fence { .. }
        ));
    }
    #[test]
    fn actual_proxy_candidate_transfer_keeps_capture_pin_fences_old_owner_and_requires_new_frame_for_cover()
     {
        use app_core::contracts::window_capabilities::*;
        let (service, parent, receiver) = cache_parent(true);
        service.collector_frame(
            "lease",
            "4",
            "server-candidate",
            "capture",
            1,
            4,
            "start",
            5,
            macos::CapturedFrame {
                bytes: vec![1, 2],
                width: 1,
                height: 1,
                captured_at_ms: 10,
            },
        );
        let old = service
            .consumer_update("owner", true)
            .unwrap()
            .unwrap()
            .frame
            .unwrap();
        let previous_frame_epoch = parent.state.lock().unwrap().sources["lease"]
            .descriptor
            .frame_epoch
            .clone();
        let transferred = service
            .transfer_pairing_to_session("lease", "4", "server-candidate", "new-session")
            .unwrap();
        assert_ne!(transferred.generation, "4");
        assert_eq!(transferred.state, PreviewState::Starting);
        assert!(transferred.captured_at_ms.is_none());
        assert!(
            service
                .consumer_update("new-session", true)
                .unwrap()
                .unwrap()
                .frame
                .is_none()
        );
        {
            let state = parent.state.lock().unwrap();
            let source = &state.sources["lease"];
            assert_eq!(source.guard.capture.as_deref(), Some("capture"));
            assert_ne!(source.descriptor.frame_epoch, previous_frame_epoch);
            assert_eq!(source.descriptor.epoch, transferred.generation);
        }
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Fence { .. }
        ));
        assert_eq!(
            service
                .control(PreviewControlRequest {
                    lease_id: "lease".into(),
                    generation: "4".into(),
                    action: PreviewControl::Stop
                })
                .unwrap_err()
                .code,
            "preview_generation_conflict"
        );
        service.collector_frame(
            "lease",
            "4",
            "server-candidate",
            "capture",
            2,
            4,
            "start",
            5,
            macos::CapturedFrame {
                bytes: vec![2],
                width: 1,
                height: 1,
                captured_at_ms: 11,
            },
        );
        assert!(
            service
                .consumer_update("new-session", true)
                .unwrap()
                .unwrap()
                .frame
                .is_none()
        );
        assert_eq!(
            service
                .commit_current_session_frame("new-session", &old, || Ok(()))
                .unwrap_err()
                .code,
            "game_cover_preview_stale"
        );
        service.collector_frame(
            "lease",
            &transferred.generation,
            "server-candidate",
            "capture",
            1,
            4,
            "start",
            5,
            macos::CapturedFrame {
                bytes: vec![3],
                width: 1,
                height: 1,
                captured_at_ms: 12,
            },
        );
        let new = service
            .consumer_update("new-session", true)
            .unwrap()
            .unwrap()
            .frame
            .unwrap();
        assert_eq!(new.captured_at_ms, "12");
        assert_eq!(
            service
                .commit_current_session_frame("new-session", &new, || Ok(7))
                .unwrap(),
            7
        );
    }
    #[test]
    fn new_candidate_stays_filterless_pending_until_exact_old_source_released_ack() {
        let parent = parent();
        parent.starting.store(true, Ordering::Release);
        {
            let mut state = parent.state.lock().unwrap();
            state.child = Some(Child {
                pid: std::process::id(),
                identity: None,
                reaped: false,
            });
            let mut old = source();
            old.id = "old-candidate".into();
            state.sources.insert(
                old.id.clone(),
                ParentSource {
                    descriptor: old,
                    guard: FrameStreamGuard::default(),
                    status: None,
                    released: false,
                    accepting: true,
                    authority: 1,
                },
            );
        }
        parent.release("old-candidate", "1").unwrap();
        let mut next = source();
        next.id = "new-candidate".into();
        parent
            .prepare(&next.id, &next.epoch, next.target.clone(), false)
            .unwrap();
        {
            let state = parent.state.lock().unwrap();
            assert_eq!(state.sources.len(), 2);
            assert!(!state.sources.contains_key("new-candidate"));
            assert!(state.waiting.is_some());
        }
        assert_eq!(
            parent
                .prepare("fourth", "1", next.target.clone(), false)
                .unwrap_err()
                .code,
            "preview_lease_limit"
        );
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Release { .. }
        ));
        assert!(receiver.try_recv().is_err());
        let (mut reading, mut writing) = UnixStream::pair().unwrap();
        protocol::write_json(
            &mut writing,
            &worker::lifecycle_status("instance", "Released", "old-candidate", "1"),
        )
        .unwrap();
        writing.shutdown(std::net::Shutdown::Write).unwrap();
        parent.receive_statuses(&mut reading, "instance");
        let promoted = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(promoted.op,ControlOp::Prepare{source,..}if source.id=="new-candidate"));
        let state = parent.state.lock().unwrap();
        assert_eq!(state.sources.len(), 2);
        assert!(state.waiting.is_none());
        assert!(!state.sources.contains_key("old-candidate"));
        assert!(state.sources.contains_key("source"));
    }
    #[test]
    fn quit_waits_for_preparation_even_before_a_child_exists() {
        let parent = CollectorSupervisor::new(Weak::new());
        parent.starting.store(true, Ordering::Release);
        let finishing = parent.clone();
        let done = Arc::new(AtomicBool::new(false));
        let witness = done.clone();
        let join = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            witness.store(true, Ordering::Release);
            finishing.starting.store(false, Ordering::Release);
        });
        parent.shutdown_and_wait().unwrap();
        assert!(done.load(Ordering::Acquire));
        join.join().unwrap();
    }
    #[test]
    fn exact_source_release_is_pending_until_actual_released_receipt_not_just_admitted_command() {
        use app_core::contracts::window_capabilities::*;
        let (service, parent, receiver) = cache_parent(false);
        let result = service
            .control(PreviewControlRequest {
                lease_id: "lease".into(),
                generation: "4".into(),
                action: PreviewControl::Stop,
            })
            .unwrap();
        assert_eq!(
            result.error_code.as_deref(),
            Some("preview_cleanup_pending")
        );
        assert_ne!(result.generation, "4");
        assert!(parent.state.lock().unwrap().sources["lease"].released);
        assert!(matches!(
            receiver.recv().unwrap().op,
            ControlOp::Release { .. }
        ));
        assert_eq!(
            service
                .latest_update("lease", &result.generation, None)
                .unwrap()
                .projection
                .error_code
                .as_deref(),
            Some("preview_cleanup_pending")
        );
        let (mut reading, mut writing) = UnixStream::pair().unwrap();
        protocol::write_json(
            &mut writing,
            &worker::lifecycle_status("instance", "Released", "lease", "4"),
        )
        .unwrap();
        writing.shutdown(std::net::Shutdown::Write).unwrap();
        parent.receive_statuses(&mut reading, "instance");
        assert_eq!(
            service
                .latest_update("lease", &result.generation, None)
                .unwrap_err()
                .code,
            "preview_lease_expired"
        );
        assert!(!parent.state.lock().unwrap().sources.contains_key("lease"));
    }
    #[test]
    fn consecutive_prepares_during_owned_handshake_share_one_child_and_bounded_context_queue() {
        let parent = CollectorSupervisor::new(Weak::new());
        parent.starting.store(true, Ordering::Release);
        let pid = std::process::id();
        parent.state.lock().unwrap().child = Some(Child {
            pid,
            identity: None,
            reaped: false,
        });
        let target = source().target;
        parent
            .prepare("candidate-a", "1", target.clone(), false)
            .unwrap();
        parent.prepare("candidate-b", "1", target, false).unwrap();
        let state = parent.state.lock().unwrap();
        assert_eq!(state.child.as_ref().unwrap().pid, pid);
        assert!(state.ready.is_none());
        assert_eq!(state.sources.len(), 2);
        drop(state);
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        let first = receiver.recv().unwrap();
        let second = receiver.recv().unwrap();
        assert!(first.request < second.request);
        assert!(matches!(first.op, ControlOp::Prepare { .. }));
        assert!(matches!(second.op, ControlOp::Prepare { .. }));
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn filterless_pending_candidate_transfers_to_session_before_native_slot_becomes_available() {
        let parent = parent();
        parent.starting.store(true, Ordering::Release);
        {
            let mut state = parent.state.lock().unwrap();
            state.child = Some(Child {
                pid: std::process::id(),
                identity: None,
                reaped: false,
            });
            let mut candidate = source();
            candidate.id = "retiring-candidate".into();
            state.sources.insert(
                candidate.id.clone(),
                ParentSource {
                    descriptor: candidate,
                    guard: FrameStreamGuard::default(),
                    status: None,
                    released: false,
                    accepting: true,
                    authority: 1,
                },
            );
        }
        parent.release("retiring-candidate", "1").unwrap();
        parent
            .prepare("pending-candidate", "1", source().target, false)
            .unwrap();
        parent
            .transfer(
                "pending-candidate",
                "1",
                "2",
                false,
                &[("source".into(), "1".into())],
            )
            .unwrap();
        {
            let state = parent.state.lock().unwrap();
            assert_eq!(state.sources.len(), 2);
            let waiting = &state.waiting.as_ref().unwrap().0;
            assert_eq!(waiting.id, "pending-candidate");
            assert_eq!(waiting.epoch, "2");
            assert!(!waiting.paused);
            assert!(state.sources["source"].released);
        }
        let receiver = parent.receiver.lock().unwrap().take().unwrap();
        assert!(
            matches!(receiver.recv().unwrap().op,ControlOp::Release{source,..}if source=="retiring-candidate")
        );
        assert!(
            matches!(receiver.recv().unwrap().op,ControlOp::Release{source,..}if source=="source")
        );
        assert!(receiver.try_recv().is_err());
        let (mut reading, mut writing) = UnixStream::pair().unwrap();
        protocol::write_json(
            &mut writing,
            &worker::lifecycle_status("instance", "Released", "retiring-candidate", "1"),
        )
        .unwrap();
        writing.shutdown(std::net::Shutdown::Write).unwrap();
        parent.receive_statuses(&mut reading, "instance");
        assert!(
            matches!(receiver.recv().unwrap().op,ControlOp::Prepare{source,select:false}if source.id=="pending-candidate"&&source.epoch=="2")
        );
    }
    fn confirmed_binding() -> GameWindowBindingProjection {
        GameWindowBindingProjection {
            state: GameWindowBindingState::Bound,
            session_id: Some("owner".into()),
            target_identity_sha256: Some("server-candidate".into()),
            window_title: Some("M02 confirmed window".into()),
            verified_at_ms: Some("123".into()),
            error_code: None,
            detail: "Actual association receipt fixture".into(),
        }
    }
    #[test]
    fn confirmed_windows_binding_keeps_first_selection_entry_reachable_without_native_frames() {
        let (_service, parent, requests) = cache_parent(false);
        let initial = parent.cached_binding(confirmed_binding());
        assert_eq!(initial.state, GameWindowBindingState::Bound);
        assert_eq!(initial.verified_at_ms.as_deref(), Some("123"));
        for (phase, code) in [
            ("Selecting", None),
            ("Unavailable", Some("preview_window_selection_required")),
            ("Unavailable", Some("preview_permission_denied")),
            ("Unavailable", Some("preview_selection_expired")),
            ("Unavailable", Some("preview_selection_mismatch")),
            ("Unavailable", Some("preview_capture_busy")),
        ] {
            let mut state = parent.state.lock().unwrap();
            let source = state.sources.get_mut("lease").unwrap();
            let mut status = Status::new(
                "instance",
                &source.descriptor,
                "",
                phase,
                code.map(str::to_owned),
            );
            status.verified_at_ms = None;
            source.status = Some(status);
            drop(state);
            let binding = parent.cached_binding(confirmed_binding());
            assert_eq!(binding.state, GameWindowBindingState::Bound, "{code:?}");
            assert_eq!(binding.verified_at_ms.as_deref(), Some("123"));
            assert!(binding.error_code.is_none());
        }
        {
            let mut state = parent.state.lock().unwrap();
            let source = state.sources.get_mut("lease").unwrap();
            source.status = Some(Status::new(
                "instance",
                &source.descriptor,
                "",
                "Unavailable",
                Some("preview_target_closed".into()),
            ));
        }
        let closed = parent.cached_binding(confirmed_binding());
        assert_eq!(closed.state, GameWindowBindingState::Stale);
        assert_eq!(closed.error_code.as_deref(), Some("preview_target_closed"));
        let mut unbound = confirmed_binding();
        unbound.state = GameWindowBindingState::Unbound;
        unbound.target_identity_sha256 = None;
        assert_eq!(
            parent.cached_binding(unbound).state,
            GameWindowBindingState::Unbound
        );
        assert!(requests.try_recv().is_err());
    }
    #[test]
    fn repeated_metadata_reads_are_pure_and_do_not_enqueue_proof_capture_or_control() {
        let (service, parent, receiver) = cache_parent(false);
        for _ in 0..30 {
            service.consumer_update("owner", false).unwrap();
            service.latest_update("lease", "4", None).unwrap();
            let binding = parent.cached_binding(confirmed_binding());
            assert_eq!(binding.verified_at_ms.as_deref(), Some("123"));
        }
        assert!(receiver.try_recv().is_err());
        assert_eq!(service.coordinator_test_projection().generation, "4");
        assert_eq!(parent.state.lock().unwrap().sources.len(), 1);
    }
}
