//! The only native producer. Its AppKit main loop owns every original filter.
use super::*;
use app_infra::hook_backend::PreviewCollectorVerifier;
use std::collections::VecDeque;

// Ordinary per-source observations coalesce independently of resource/proof
// receipts. The one current proof, two source releases/capture authorities and
// two monotonic shutdown receipts are bounded by the existing source protocol.
#[derive(Default)]
struct StatusOutbox {
    critical: VecDeque<Status>,
    latest: BTreeMap<String, Status>,
    draining: bool,
}
#[derive(Clone)]
struct StatusSender(Arc<(Mutex<StatusOutbox>, Condvar)>);
struct StatusReceiver(Arc<(Mutex<StatusOutbox>, Condvar)>);
fn status_channel() -> (StatusSender, StatusReceiver) {
    let queue = Arc::new((Mutex::new(StatusOutbox::default()), Condvar::new()));
    (StatusSender(queue.clone()), StatusReceiver(queue))
}
impl StatusSender {
    fn try_send(&self, event: Status) -> Result<(), ()> {
        let mut queue = self.0.0.lock().map_err(|_| ())?;
        let critical = matches!(
            event.phase.as_str(),
            "Proof" | "Released" | "CaptureReady" | "StopAcceptedDraining" | "Stopped"
        );
        if !critical {
            if queue.draining {
                return Ok(());
            }
            if !queue.latest.contains_key(&event.source) && queue.latest.len() >= 2 {
                return Err(());
            }
            queue.latest.insert(event.source.clone(), event);
        } else {
            if event.phase == "StopAcceptedDraining" {
                if queue.draining {
                    return Ok(());
                }
                queue.draining = true;
                queue.latest.clear();
                // A global publication fence retires outstanding capture
                // authorities; their pixels can no longer become Live.
                queue.critical.retain(|old| old.phase != "CaptureReady");
            }
            if event.phase == "CaptureReady" && queue.draining {
                return Ok(());
            }
            if event.phase == "Released" {
                queue.latest.remove(&event.source);
                queue
                    .critical
                    .retain(|old| !(old.phase == "CaptureReady" && old.source == event.source));
            }
            let existing = queue.critical.iter().position(|old| {
                old.phase == event.phase
                    && if event.phase == "Proof" {
                        old.request == event.request
                    } else {
                        old.source == event.source
                    }
            });
            if let Some(index) = existing {
                queue.critical[index] = event;
            } else {
                if queue.critical.len() >= 8 {
                    return Err(());
                }
                queue.critical.push_back(event);
            }
        }
        self.0.1.notify_one();
        Ok(())
    }
}
impl StatusReceiver {
    fn pop(queue: &mut StatusOutbox) -> Option<Status> {
        queue.critical.pop_front().or_else(|| {
            let source = queue.latest.keys().next()?.clone();
            queue.latest.remove(&source)
        })
    }
    #[cfg(test)]
    fn try_recv(&self) -> Result<Status, mpsc::TryRecvError> {
        let mut queue = self
            .0
            .0
            .lock()
            .map_err(|_| mpsc::TryRecvError::Disconnected)?;
        Self::pop(&mut queue).ok_or(mpsc::TryRecvError::Empty)
    }
    fn recv_timeout(&self, timeout: Duration) -> Result<Status, mpsc::RecvTimeoutError> {
        let deadline = Instant::now() + timeout;
        let mut queue = self
            .0
            .0
            .lock()
            .map_err(|_| mpsc::RecvTimeoutError::Disconnected)?;
        loop {
            if let Some(event) = Self::pop(&mut queue) {
                return Ok(event);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(mpsc::RecvTimeoutError::Timeout);
            }
            queue = self
                .0
                .1
                .wait_timeout(queue, left)
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)?
                .0;
        }
    }
}
#[derive(Clone)]
struct Intent {
    epoch: String,
    frame_epoch: String,
    paused: bool,
    released: bool,
    selection_request: u64,
    target_identity: String,
}
struct State {
    stopped: AtomicBool,
    finished: AtomicBool,
    intents: Mutex<BTreeMap<String, Intent>>,
    frames: Latest<Frame>,
    critical: StatusSender,
    commands: mpsc::SyncSender<Control>,
    deferred: Mutex<Option<Control>>,
    permission_requested: AtomicBool,
    stop_accepted: AtomicBool,
    final_status_sent: AtomicBool,
}
impl State {
    fn current(&self, source: &Source) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && self
                .intents
                .lock()
                .map(|m| {
                    m.get(&source.id).is_some_and(|i| {
                        i.epoch == source.epoch
                            && i.frame_epoch == source.frame_epoch
                            && !i.released
                            && !i.paused
                    })
                })
                .unwrap_or(false)
    }
    fn selection_current(&self, source: &str, request: u64) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && self
                .intents
                .lock()
                .map(|m| {
                    m.get(source)
                        .is_some_and(|i| !i.released && !i.paused && i.selection_request == request)
                })
                .unwrap_or(false)
    }
    fn event(
        &self,
        instance: &str,
        source: &Source,
        capture: &str,
        phase: &str,
        code: Option<String>,
    ) -> Status {
        let mut event = Status::new(instance, source, capture, phase, code);
        event.permission = macos::preview_permission(false);
        event
    }
    fn emit(&self, event: Status) -> bool {
        self.critical.try_send(event).is_ok()
    }
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.frames.clear();
    }
    fn accept_stop(&self, instance: &str) {
        if !self.stop_accepted.swap(true, Ordering::AcqRel) {
            let _ =
                self.critical
                    .try_send(lifecycle_status(instance, "StopAcceptedDraining", "", ""));
        }
        self.stop();
    }
}
fn valid_hello(hello: &Hello) -> bool {
    hello.v == protocol::VERSION
        && hello.nonce.len() >= 32
        && hello.nonce.len() <= 112
        && hello
            .nonce
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && macos::collector_parent(hello.parent.pid)
        && macos::collector_process_identity(hello.parent.pid)
            .ok()
            .as_ref()
            == Some(&hello.parent)
}
pub(super) fn run(mut control: UnixStream, mut frames: UnixStream) -> AppResult<()> {
    control
        .set_read_timeout(Some(Duration::from_secs(4)))
        .map_err(io_error)?;
    control
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(io_error)?;
    let hello: Hello = protocol::read_json(&mut control).map_err(io_error)?;
    if !valid_hello(&hello) {
        return Err(failure("collector_handshake"));
    }
    let identity = macos::collector_process_identity(std::process::id())?;
    if identity.dev != hello.parent.dev || identity.ino != hello.parent.ino {
        return Err(failure("collector_handshake"));
    }
    let ready = Ready {
        v: protocol::VERSION,
        nonce: hello.nonce,
        worker: identity,
        instance: id("CollectorInstance"),
    };
    protocol::write_json(&mut control, &ready).map_err(io_error)?;
    control.set_read_timeout(None).map_err(io_error)?;
    let (sender, receiver) = mpsc::sync_channel(8);
    let (events, event_rx) = status_channel();
    let state = Arc::new(State {
        stopped: AtomicBool::new(false),
        finished: AtomicBool::new(false),
        intents: Mutex::new(BTreeMap::new()),
        frames: Latest::default(),
        critical: events,
        commands: sender,
        deferred: Mutex::new(None),
        permission_requested: AtomicBool::new(false),
        stop_accepted: AtomicBool::new(false),
        final_status_sent: AtomicBool::new(false),
    });
    let mut commands = control.try_clone().map_err(io_error)?;
    let reader = state.clone();
    let instance = ready.instance.clone();
    std::thread::spawn(move || read_commands(&mut commands, &reader, &instance));
    let status_state = state.clone();
    std::thread::spawn(move || send_statuses(&mut control, &event_rx, &status_state));
    let writer = state.clone();
    std::thread::spawn(move || {
        let _ = send_frames(&mut frames, &writer.frames, &writer.stopped);
    });
    let pipeline = state.clone();
    std::thread::spawn(move || {
        produce(receiver, &pipeline, &ready.instance);
        pipeline.accept_stop(&ready.instance);
        let _ = pipeline
            .critical
            .try_send(lifecycle_status(&ready.instance, "Stopped", "", ""));
        let deadline = Instant::now() + Duration::from_millis(300);
        while !pipeline.final_status_sent.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        pipeline.finished.store(true, Ordering::Release);
        macos::collector_stop_runloop();
    });
    // Present and observer callbacks execute only here, on this process main thread.
    let result = macos::collector_runloop();
    state.stop();
    result
}
fn read_commands(commands: &mut UnixStream, state: &State, instance: &str) {
    let mut previous = 0;
    while let Ok(command) = protocol::read_json::<Control>(commands) {
        if command.v != protocol::VERSION
            || command.instance != instance
            || command.request <= previous
        {
            break;
        }
        previous = command.request;
        if matches!(command.op, ControlOp::Shutdown) {
            break;
        }
        let accepted = {
            let Ok(mut intents) = state.intents.lock() else {
                break;
            };
            match &command.op {
                ControlOp::Prepare { source, .. } => {
                    if source.id.is_empty()
                        || source.id.len() > 128
                        || source.epoch.parse::<u64>().is_err()
                    {
                        false
                    } else if let Some(intent) = intents.get_mut(&source.id) {
                        if intent.released {
                            false
                        } else {
                            intent.epoch = source.epoch.clone();
                            intent.frame_epoch = source.frame_epoch.clone();
                            intent.paused = source.paused;
                            intent.selection_request =
                                if matches!(&command.op, ControlOp::Prepare { select: true, .. }) {
                                    command.request
                                } else {
                                    0
                                };
                            true
                        }
                    } else if intents.len() < 2 {
                        intents.insert(
                            source.id.clone(),
                            Intent {
                                epoch: source.epoch.clone(),
                                frame_epoch: source.frame_epoch.clone(),
                                paused: source.paused,
                                released: false,
                                target_identity: source.target.candidate_id().into(),
                                selection_request: if matches!(
                                    &command.op,
                                    ControlOp::Prepare { select: true, .. }
                                ) {
                                    command.request
                                } else {
                                    0
                                },
                            },
                        );
                        true
                    } else {
                        false
                    }
                }
                ControlOp::Fence {
                    source,
                    previous_epoch,
                    epoch,
                    frame_epoch,
                    paused,
                    retire,
                } => {
                    if retire.len() > 1
                        || retire.iter().any(|(id, old)| {
                            id == source
                                || intents
                                    .get(id)
                                    .is_none_or(|i| i.epoch != *old || i.released)
                        })
                    {
                        return state.accept_stop(instance);
                    }
                    for (id, _) in retire {
                        intents
                            .get_mut(id)
                            .expect("validated retired source")
                            .released = true;
                    }
                    if let Some(intent) = intents
                        .get_mut(source)
                        .filter(|i| i.epoch == *previous_epoch && !i.released)
                    {
                        intent.epoch = epoch.clone();
                        intent.frame_epoch = frame_epoch.clone();
                        intent.paused = *paused;
                        true
                    } else {
                        false
                    }
                }
                ControlOp::Release { source, epoch } => {
                    if let Some(intent) = intents.get_mut(source).filter(|i| i.epoch == *epoch) {
                        intent.released = true;
                        true
                    } else {
                        false
                    }
                }
                ControlOp::Verify { source, epoch, .. } => intents
                    .get(source)
                    .is_some_and(|i| i.epoch == *epoch && !i.released),
                ControlOp::Shutdown => false,
            }
        };
        if !accepted {
            break;
        }
        match state.commands.try_send(command) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(command)) => {
                match &command.op {
                    // Latest intent is already fenced. The serial pipeline
                    // reconciles it after its current bounded operation drains.
                    ControlOp::Fence { .. } | ControlOp::Release { .. } => {}
                    ControlOp::Prepare { source, .. } => {
                        let Ok(mut slot) = state.deferred.lock() else {
                            break;
                        };
                        if slot.as_ref().is_none_or(|old|matches!(&old.op,ControlOp::Prepare{source:old,..}if old.id==source.id)){*slot=Some(command);}else{let _=state.emit(state.event(instance,source,"","Unavailable",Some("preview_capture_busy".into())));}
                    }
                    ControlOp::Verify {
                        source,
                        epoch,
                        capture,
                        frame_epoch,
                    } => {
                        let mut status = lifecycle_status(instance, "Proof", source, epoch);
                        status.capture_id = capture.clone();
                        status.frame_epoch = frame_epoch.clone();
                        status.request = Some(command.request);
                        status.target_identity = state
                            .intents
                            .lock()
                            .ok()
                            .and_then(|m| m.get(source).map(|i| i.target_identity.clone()))
                            .unwrap_or_default();
                        status.error_code = Some("preview_capture_busy".into());
                        if !state.emit(status) {
                            break;
                        }
                    }
                    ControlOp::Shutdown => break,
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => break,
        }
        // In-flight packets retain framing. Parent rejects their old epoch; pending
        // old frames are dropped immediately rather than filling a backlog.
        state.frames.clear();
    }
    state.accept_stop(instance);
}
pub(super) fn lifecycle_status(instance: &str, phase: &str, source: &str, epoch: &str) -> Status {
    Status {
        v: protocol::VERSION,
        instance: instance.into(),
        source: source.into(),
        epoch: epoch.into(),
        capture_id: String::new(),
        frame_epoch: String::new(),
        sequence: 0,
        phase: phase.into(),
        error_code: None,
        observed_at_ms: now(),
        permission: false,
        target_identity: String::new(),
        window_title: None,
        request: None,
        verified_at_ms: None,
        authority: None,
        diagnostic: None,
    }
}
fn send_statuses(control: &mut UnixStream, event_rx: &StatusReceiver, status_state: &State) {
    while !status_state.finished.load(Ordering::Acquire) {
        match event_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => {
                let mut packet = Vec::new();
                if protocol::write_json(&mut packet, &event).is_err()
                    || send_packet(control, &packet, &status_state.finished).is_err()
                {
                    status_state.stop();
                    break;
                }
                if event.phase == "Stopped" {
                    status_state
                        .final_status_sent
                        .store(true, Ordering::Release);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
}
fn send_packet(socket: &mut UnixStream, packet: &[u8], stopped: &AtomicBool) -> io::Result<()> {
    socket.set_write_timeout(Some(Duration::from_millis(25)))?;
    let mut offset = 0;
    while offset < packet.len() {
        if stopped.load(Ordering::Acquire) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        match socket.write(&packet[offset..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => offset += count,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn send_frames(
    socket: &mut UnixStream,
    frames: &Latest<Frame>,
    stopped: &AtomicBool,
) -> io::Result<()> {
    while !stopped.load(Ordering::Acquire) {
        let Some(frame) = frames.take(stopped) else {
            break;
        };
        let mut packet = Vec::new();
        protocol::write_frame(&mut packet, &frame.header, &frame.bytes)?;
        send_packet(socket, &packet, stopped)?;
    }
    Ok(())
}
enum Choice {
    Automatic(macos::NativeWindowIdentity),
    Selected(Arc<macos::SelectedPreviewWindow>),
}
impl Choice {
    fn identity(&self) -> &macos::NativeWindowIdentity {
        match self {
            Self::Automatic(i) => i,
            Self::Selected(s) => &s.identity,
        }
    }
    fn validate(&self, verifier: &PreviewCollectorVerifier) -> AppResult<()> {
        match self {
            Self::Automatic(i) => {
                if macos::preview_identity_current(i) {
                    Ok(())
                } else {
                    Err(failure("preview_selection_expired"))
                }
            }
            Self::Selected(s) => s.validate(verifier.context()),
        }
    }
    fn snapshot(&self) -> ChoiceKey {
        let identity = self.identity();
        ChoiceKey {
            selected_token: match self {
                Self::Automatic(_) => None,
                Self::Selected(value) => Some(value.token().to_owned()),
            },
            host_pid: identity.host_pid,
            host_start: identity.host_process_start.clone(),
            window_id: identity.window_id,
        }
    }
    fn capture(&self, current: &dyn Fn() -> AppResult<()>) -> macos::PreviewCaptureAttempt {
        let mut attempt = match self {
            Self::Automatic(identity) => {
                if let Err(error) = current() {
                    return macos::PreviewCaptureAttempt {
                        result: Err(error),
                        diagnostic: None,
                        failure_kind: None,
                    };
                }
                macos::capture_selected_attempt(identity, None, Some(current))
            }
            Self::Selected(selected) => macos::capture_selected_attempt(
                &selected.identity,
                Some(selected.token()),
                Some(current),
            ),
        };
        attempt.result = attempt.result.map_err(native_capture_error);
        attempt
    }
}
fn native_capture_error(error: AppError) -> AppError {
    // These APIs only attest a Native CG window/filter. Losing that owner must
    // not invalidate the separately confirmed Windows HWND/process binding.
    if error.code == "preview_target_closed" {
        failure("preview_selection_expired")
    } else {
        error
    }
}
type PickerResult = Arc<macos::SelectedPreviewWindow>;
struct PendingSelection<T> {
    request: u64,
    transaction: String,
    receiver: mpsc::Receiver<AppResult<T>>,
    result: Option<AppResult<T>>,
    attempted: Arc<AtomicBool>,
    cancelled: bool,
}
unsafe extern "C" {
    fn luna_preview_picker_transaction_active(token: *const std::ffi::c_char) -> i32;
}
fn picker_active(token: &str) -> i32 {
    let Ok(token) = std::ffi::CString::new(token) else {
        return -1;
    };
    // SAFETY: Native borrows one immutable token and reads a locked exact registry.
    unsafe { luna_preview_picker_transaction_active(token.as_ptr()) }
}
// Capability bytes are retained only in process memory. Numeric geometry is
// deliberately absent: geometry is an observation, never capture authority.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ChoiceKey {
    selected_token: Option<String>,
    host_pid: u32,
    host_start: String,
    window_id: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct AttemptKey {
    source: String,
    epoch: String,
    frame_epoch: String,
    target: String,
    authority: u64,
    capture: String,
    choice: ChoiceKey,
}
impl AttemptKey {
    fn from_context(context: &Context) -> AppResult<Self> {
        Ok(Self {
            source: context.source.id.clone(),
            epoch: context.source.epoch.clone(),
            frame_epoch: context.source.frame_epoch.clone(),
            target: context.source.target.candidate_id().into(),
            authority: context.authority,
            capture: context.capture.clone(),
            choice: context
                .choice
                .as_ref()
                .ok_or_else(|| failure("preview_selection_required"))?
                .snapshot(),
        })
    }
    fn matches_intent(&self, intent: &Intent) -> bool {
        self.epoch == intent.epoch
            && self.frame_epoch == intent.frame_epoch
            && self.target == intent.target_identity
            && !intent.paused
            && !intent.released
    }
}
impl State {
    fn attempt_current(&self, context: &Context, key: &AttemptKey) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && AttemptKey::from_context(context).as_ref().ok() == Some(key)
            && self.intents.lock().ok().is_some_and(|intents| {
                intents
                    .get(&key.source)
                    .is_some_and(|intent| key.matches_intent(intent))
            })
    }
    fn commit_post(&self, context: &Context, key: &AttemptKey) -> AppResult<()> {
        self.commit_key(key, &AttemptKey::from_context(context)?)
    }
    fn commit_key(&self, key: &AttemptKey, actual: &AttemptKey) -> AppResult<()> {
        // This lock contains no permission API, FFI, filesystem or verifier IO.
        let intents = self
            .intents
            .lock()
            .map_err(|_| failure("preview_cancelled"))?;
        if self.stopped.load(Ordering::Acquire)
            || actual != key
            || !intents
                .get(&key.source)
                .is_some_and(|intent| key.matches_intent(intent))
        {
            return Err(failure("preview_cancelled"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostDisposition {
    Invalid,
    Unproved,
    Superseded,
}
impl PostDisposition {
    fn word(self) -> &'static str {
        match self {
            Self::Invalid => "release_invalid",
            Self::Unproved => "release_unproved",
            Self::Superseded => "superseded",
        }
    }
}
#[derive(Debug)]
struct PostFailure {
    error: AppError,
    disposition: PostDisposition,
}
impl PostFailure {
    fn new(error: AppError) -> Self {
        let disposition = if error.code == "preview_cancelled" {
            PostDisposition::Superseded
        } else if matches!(
            error.code.as_str(),
            "preview_permission_revoked"
                | "preview_permission_denied"
                | "preview_target_closed"
                | "preview_target_changed"
                | "preview_selection_expired"
                | "preview_selection_mismatch"
        ) {
            PostDisposition::Invalid
        } else {
            PostDisposition::Unproved
        };
        Self { error, disposition }
    }
}
#[derive(Debug)]
struct PostVerified(AttemptKey);
impl PostVerified {
    fn consume(self, key: &AttemptKey) -> AppResult<()> {
        if self.0 == *key {
            Ok(())
        } else {
            Err(failure("preview_cancelled"))
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetentionCause {
    CompatibilitySourceCap,
    GeometryUnavailable,
    FreshGeometryChanged,
}
#[derive(Debug)]
struct RetentionDecision {
    key: AttemptKey,
    cause: RetentionCause,
}
#[derive(Debug)]
struct RetentionPermit {
    _key: AttemptKey,
    _cause: RetentionCause,
}
#[derive(Debug)]
enum AttemptDecision {
    Retain(RetentionDecision),
    Release {
        key: AttemptKey,
        disposition: PostDisposition,
        strict: bool,
    },
}
impl PostVerified {
    fn into_retention(
        self,
        key: &AttemptKey,
        cause: RetentionCause,
    ) -> AppResult<RetentionDecision> {
        self.consume(key)?;
        Ok(RetentionDecision {
            key: key.clone(),
            cause,
        })
    }
}
// A diagnostic string cannot construct this permit. The same serial Context
// snapshot, current intent and TCC are checked again when the decision is used.
fn consume_retention(
    decision: RetentionDecision,
    actual: &AttemptKey,
    state: &State,
    permission: impl FnOnce() -> bool,
) -> AppResult<RetentionPermit> {
    state.commit_key(&decision.key, actual)?;
    let granted = permission();
    state.commit_key(&decision.key, actual)?;
    if !granted {
        return Err(failure("preview_permission_revoked"));
    }
    Ok(RetentionPermit {
        _key: decision.key,
        _cause: decision.cause,
    })
}
// The same helper serves a frame and a typed, frame-less refusal. In particular,
// an external verifier error is saved until the unconditional current/TCC check.
fn complete_post_proof(
    key: &AttemptKey,
    mut current: impl FnMut() -> AppResult<()>,
    verify: impl FnOnce() -> AppResult<()>,
    validate: impl FnOnce() -> AppResult<()>,
    commit: impl FnOnce(&AttemptKey) -> AppResult<()>,
) -> Result<PostVerified, PostFailure> {
    current().map_err(PostFailure::new)?;
    let verified = verify();
    current().map_err(PostFailure::new)?;
    verified.map_err(PostFailure::new)?;
    let validated = validate();
    current().map_err(PostFailure::new)?;
    validated.map_err(PostFailure::new)?;
    commit(key).map_err(PostFailure::new)?;
    Ok(PostVerified(key.clone()))
}
const RECOVERY_REFILL: Duration = Duration::from_secs(20);
const METADATA_INTERVAL: Duration = Duration::from_secs(5);
const GEOMETRY_DEBOUNCE: Duration = Duration::from_secs(10);
#[derive(Debug)]
struct RecoveryBucket {
    credits: u8,
    anchor: Instant,
}
impl RecoveryBucket {
    fn triggered(at: Instant) -> Self {
        Self {
            credits: 2,
            anchor: at,
        }
    }
    fn refill(&mut self, at: Instant) {
        let Some(elapsed) = at.checked_duration_since(self.anchor) else {
            return;
        };
        let ticks = elapsed.as_secs() / RECOVERY_REFILL.as_secs();
        if ticks == 0 {
            return;
        }
        let Some(seconds) = ticks.checked_mul(20) else {
            return;
        };
        let step = Duration::from_secs(seconds);
        let Some(anchor) = self.anchor.checked_add(step) else {
            return;
        };
        self.credits = (u64::from(self.credits).saturating_add(ticks)).min(3) as u8;
        self.anchor = anchor;
    }
    fn admit(&mut self, at: Instant) -> bool {
        self.refill(at);
        if self.credits == 0 {
            false
        } else {
            self.credits -= 1;
            true
        }
    }
}
#[derive(Debug)]
struct StableGeometry {
    authority: AttemptKey,
    signature: [u64; 8],
    first_seen: Instant,
    last_seen: Instant,
    samples: u32,
}
#[derive(Debug)]
struct GeometrySlot {
    signature: [u64; 8],
    stable: bool,
}
#[derive(Default, Debug)]
struct Recovery {
    bucket: Option<RecoveryBucket>,
    blocked: bool,
    next_metadata: Option<Instant>,
    candidate: Option<StableGeometry>,
    cache: VecDeque<GeometrySlot>,
    generation: u64,
    exhausted_generation: bool,
    normal_trigger_without_credit: bool,
    last_report: Option<(u8, u64, bool, Option<[u64; 8]>)>,
}
impl Recovery {
    fn trigger(&mut self, admission: Instant, returned: Instant) {
        self.normal_trigger_without_credit = match &mut self.bucket {
            None => {
                self.bucket = Some(RecoveryBucket::triggered(admission));
                false
            }
            Some(bucket) => !bucket.admit(admission),
        };
        self.blocked = true;
        self.clear_authority();
        self.after_metadata(returned);
    }
    fn clear_authority(&mut self) {
        self.candidate = None;
        for slot in &mut self.cache {
            slot.stable = false;
        }
    }
    fn after_metadata(&mut self, returned: Instant) {
        self.next_metadata = returned.checked_add(METADATA_INTERVAL);
    }
    fn due(&self, at: Instant) -> bool {
        self.blocked && self.next_metadata.is_some_and(|due| at >= due)
    }
    fn observe(&mut self, key: &AttemptKey, signature: Option<[u64; 8]>, at: Instant) -> bool {
        let Some(signature) = signature else {
            self.clear_authority();
            return false;
        };
        if self.exhausted_generation {
            return false;
        }
        let matches = self.candidate.as_ref().is_some_and(|candidate| {
            candidate.authority == *key
                && candidate.signature == signature
                && at.checked_duration_since(candidate.last_seen).is_some()
        });
        if !matches {
            let Some(next) = self.generation.checked_add(1) else {
                self.exhausted_generation = true;
                self.clear_authority();
                return false;
            };
            self.generation = next;
            self.candidate = Some(StableGeometry {
                authority: key.clone(),
                signature,
                first_seen: at,
                last_seen: at,
                samples: 1,
            });
        } else if let Some(candidate) = &mut self.candidate {
            candidate.samples = candidate.samples.saturating_add(1);
            candidate.last_seen = at;
        }
        let candidate = self.candidate.as_ref().expect("observed candidate");
        let stable = candidate.samples >= 2
            && at
                .checked_duration_since(candidate.first_seen)
                .is_some_and(|d| d >= GEOMETRY_DEBOUNCE);
        if let Some(index) = self
            .cache
            .iter()
            .position(|slot| slot.signature == signature)
        {
            self.cache.remove(index);
        }
        if self.cache.len() == 4 {
            self.cache.pop_front();
        }
        self.cache.push_back(GeometrySlot { signature, stable });
        stable
    }
    fn admit(&mut self, at: Instant) -> bool {
        self.bucket.as_mut().is_some_and(|bucket| bucket.admit(at))
    }
    fn frame_published(&mut self) {
        self.blocked = false;
        self.next_metadata = None;
        self.clear_authority();
        // A real frame changes scheduling only; the original capability bucket lives on.
    }
    fn report_changed(&mut self) -> bool {
        let key = (
            self.bucket.as_ref().map_or(3, |bucket| bucket.credits),
            self.generation,
            self.candidate.as_ref().is_some_and(|candidate| {
                candidate.samples >= 2
                    && candidate
                        .last_seen
                        .checked_duration_since(candidate.first_seen)
                        .is_some_and(|elapsed| elapsed >= GEOMETRY_DEBOUNCE)
            }),
            self.candidate.as_ref().map(|candidate| candidate.signature),
        );
        if self.last_report == Some(key) {
            return false;
        }
        self.last_report = Some(key);
        true
    }
    fn diagnostic(&self, metadata_only: bool, admission: bool) -> macos::PreviewRecoveryDiagnostic {
        macos::PreviewRecoveryDiagnostic {
            state: if self.blocked && admission {
                "recovering"
            } else if self.blocked {
                "geometry_blocked"
            } else {
                "normal"
            }
            .into(),
            credits: self.bucket.as_ref().map_or(3, |bucket| bucket.credits),
            normal_trigger_without_recovery_credit: self.normal_trigger_without_credit,
            samples: self
                .candidate
                .as_ref()
                .map_or(0, |candidate| candidate.samples),
            generation: self.generation,
            cache_entries: self.cache.len() as u8,
            metadata_only,
            admission,
        }
    }
}
struct Context {
    source: Source,
    verifier: PreviewCollectorVerifier,
    capture: String,
    choice: Option<Choice>,
    sequence: u64,
    last_attempt: Instant,
    terminal: bool,
    authority: u64,
    pending_picker: Option<PendingSelection<PickerResult>>,
    new_capture: bool,
    reselect_queued: bool,
    diagnostic: Option<macos::PreviewDiagnostic>,
    recovery: Recovery,
    post_decision: Option<AttemptDecision>,
}
fn diagnostic_stage(context: &mut Context, stage: &str) {
    context.post_decision = None;
    let mut value = macos::PreviewDiagnostic::worker(stage);
    value.inherit_picker(context.diagnostic.as_ref());
    value.request = context.authority;
    context.diagnostic = Some(value);
}
fn observe_picker(context: &mut Context, state: &State, instance: &str) {
    let Some(pending) = context.pending_picker.as_ref() else {
        return;
    };
    let Some(mut value) = macos::picker_diagnostic(&pending.transaction) else {
        return;
    };
    if context.diagnostic.as_ref().is_some_and(|old| {
        old.picker_operation == value.picker_operation && old.revision == value.revision
    }) {
        return;
    }
    value.request = pending.request;
    context.diagnostic = Some(value.clone());
    // Ordinary metadata coalesces; it never occupies the critical receipt queue.
    let mut status = state.event(
        instance,
        &context.source,
        &context.capture,
        "Selecting",
        None,
    );
    status.diagnostic = Some(value);
    let _ = state.emit(status);
}
fn resources() -> AppResult<PathBuf> {
    std::env::current_exe()
        .map_err(io_error)?
        .canonicalize()
        .map_err(io_error)?
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("Resources"))
        .ok_or_else(|| failure("collector_resources"))?
        .canonicalize()
        .map_err(io_error)
}
fn resolve_nonce(nonce: &str) -> Result<macos::NativeWindowIdentity, String> {
    let deadline = Instant::now() + Duration::from_millis(1200);
    loop {
        match macos::resolve_nonce(nonce) {
            Ok(native) => return Ok(native),
            Err(e) if e.code == "no_capture_window" && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(60))
            }
            Err(e) => return Err(e.code),
        }
    }
}
fn prepare(
    context: &mut Context,
    state: &Arc<State>,
    instance: &str,
    select: bool,
) -> AppResult<()> {
    let source = context.source.clone();
    let stopped = || !state.current(&source);
    if stopped() {
        return Err(failure("preview_cancelled"));
    }
    if macos::capture_busy() {
        return Err(failure("preview_capture_busy"));
    }
    if !macos::preview_permission(false) {
        if !state.permission_requested.swap(true, Ordering::AcqRel) {
            let _ = macos::preview_permission(true);
        }
        if !macos::preview_permission(false) {
            return Err(failure("preview_permission_denied"));
        }
    }
    diagnostic_stage(context, "win_pre");
    context.verifier.verify(&stopped)?;
    if select {
        let mut diagnostic = macos::PreviewDiagnostic::worker("picker_wait");
        diagnostic.request = context.authority;
        context.diagnostic = Some(diagnostic);
        let mut selecting = state.event(instance, &source, &context.capture, "Selecting", None);
        selecting.diagnostic = context.diagnostic.clone();
        if !state.emit(selecting) {
            return Err(failure("preview_worker_transport"));
        }
        let current_state = state.clone();
        let request = context.authority;
        let source_id = source.id.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let transaction = id("CollectorSelection");
        let attempted = Arc::new(AtomicBool::new(false));
        // Reserve the pending operation before its thread can reach Native.
        context.pending_picker = Some(PendingSelection {
            request,
            transaction: transaction.clone(),
            receiver: rx,
            result: None,
            attempted: attempted.clone(),
            cancelled: false,
        });
        std::thread::spawn(move || {
            let result = if !current_state.selection_current(&source_id, request) {
                Err(failure("preview_cancelled"))
            } else {
                attempted.store(true, Ordering::Release);
                tauri::async_runtime::block_on(macos::select_preview_window(
                    transaction,
                    Arc::new(move || current_state.selection_current(&source_id, request)),
                ))
            };
            let _ = tx.try_send(result);
        });
    } else if context.choice.is_none() && context.pending_picker.is_none() {
        let nonce = id("LunaWindowProof");
        let native = context
            .verifier
            .with_window_challenge(&nonce, || resolve_nonce(&nonce))?;
        context.verifier.verify(&stopped)?;
        if stopped() {
            return Err(failure("preview_cancelled"));
        }
        context.choice = Some(Choice::Automatic(native));
        context.capture = id("CollectorCapture");
        context.new_capture = true;
    }
    context.terminal = false;
    Ok(())
}
fn poll_selection<T>(
    pending: &mut Option<PendingSelection<T>>,
    current: impl Fn(u64) -> bool,
    mut active: impl FnMut(&str) -> i32,
) -> Option<AppResult<(u64, T)>> {
    let value = pending.as_mut()?;
    if !current(value.request) {
        value.cancelled = true;
    }
    if value.result.is_none() {
        match value.receiver.try_recv() {
            Ok(result) => value.result = Some(result),
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                value.result = Some(Err(failure("preview_selection_failed")))
            }
        }
    }
    let result = value.result.as_ref()?;
    let native_rejected = result.as_ref().err().is_some_and(|e| {
        matches!(
            e.code.as_str(),
            "preview_selection_busy"
                | "preview_selection_unsupported"
                | "preview_permission_denied"
        )
    });
    // Unknown is not completion. A local pre-call cancellation or explicit
    // begin rejection is the only case with no accepted native operation.
    if value.attempted.load(Ordering::Acquire)
        && !native_rejected
        && active(&value.transaction) != 0
    {
        return None;
    }
    let mut value = pending.take().expect("pending operation retained");
    if value.cancelled {
        drop(value.result.take());
        Some(Err(failure("preview_cancelled")))
    } else {
        Some(
            value
                .result
                .take()
                .expect("completed result")
                .map(|result| (value.request, result)),
        )
    }
}
fn transient(code: &str) -> bool {
    matches!(
        code,
        "preview_capture_busy"
            | "preview_capture_timeout"
            | "preview_verification_unavailable"
            | "preview_capture_failed"
            | "preview_encoding_failed"
            | "preview_source_changed"
            | "preview_window_not_on_screen"
            | "preview_permission_denied"
            | "preview_permission_revoked"
    )
}
fn invalidate_source_choice<T>(choice: &mut Option<T>, code: &str) -> bool {
    if matches!(
        code,
        "preview_permission_denied"
            | "preview_permission_revoked"
            | "preview_selection_expired"
            | "preview_selection_mismatch"
            | "preview_target_closed"
    ) {
        *choice = None;
        true
    } else {
        false
    }
}
// Own the retry/invalidation decision in one place, shared by the actual
// scheduler error publisher and resource-lifetime regression tests.
fn apply_capture_failure<T>(
    choice: &mut Option<T>,
    terminal: &mut bool,
    new_capture: &mut bool,
    code: &str,
) {
    if matches!(
        code,
        "preview_permission_denied" | "preview_permission_revoked"
    ) {
        invalidate_source_choice(choice, code);
        *new_capture = false;
    }
    *terminal = !transient(code);
    if matches!(
        code,
        "preview_selection_cancelled" | "preview_selection_timeout" | "preview_selection_failed"
    ) && choice.is_some()
    {
        *terminal = false;
    } else if *terminal {
        *choice = None;
    }
}
fn apply_attempt_failure<T>(
    choice: &mut Option<T>,
    terminal: &mut bool,
    new_capture: &mut bool,
    code: &str,
    retention: Option<RetentionPermit>,
    disposition: Option<PostDisposition>,
    strict: bool,
) {
    if retention.is_some() {
        *terminal = false;
        return;
    }
    apply_capture_failure(choice, terminal, new_capture, code);
    if strict
        && matches!(
            disposition,
            Some(PostDisposition::Unproved | PostDisposition::Invalid)
        )
    {
        *choice = None;
        *new_capture = false;
        *terminal = true;
    }
}

fn requires_post_proof(
    frame: bool,
    kind: Option<macos::CaptureFailureKind>,
    selected: bool,
    recovering: bool,
) -> bool {
    frame || (recoverable_kind(kind) && selected) || recovering
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerRefusal {
    FreshGeometryChanged,
}
// Only an actual fresh canonical comparison can produce this private cause.
// General Native errors and their text cannot enter the geometry-race exception.
fn capture_after_geometry(
    geometry: macos::PreviewGeometrySnapshot,
    expected: [u64; 8],
    native_capture: impl FnOnce() -> macos::PreviewCaptureAttempt,
) -> (macos::PreviewCaptureAttempt, Option<WorkerRefusal>) {
    let signature = geometry.signature();
    if signature == Some(expected) {
        return (native_capture(), None);
    }
    let cause = signature.map(|_| WorkerRefusal::FreshGeometryChanged);
    let kind = if geometry.reason.is_some() {
        macos::CaptureFailureKind::GeometryUnavailable
    } else {
        macos::CaptureFailureKind::OtherExistingFailure
    };
    let mut diagnostic = macos::PreviewDiagnostic::worker("capture");
    diagnostic.geometry = Some(geometry);
    diagnostic.failure_kind = Some(kind);
    (
        macos::PreviewCaptureAttempt {
            result: Err(failure("preview_source_changed")),
            diagnostic: Some(diagnostic),
            failure_kind: Some(kind),
        },
        cause,
    )
}
fn complete_capture_post<T>(
    captured: &AppResult<T>,
    kind: Option<macos::CaptureFailureKind>,
    selected: bool,
    recovering: bool,
    post: impl FnOnce() -> Result<PostVerified, PostFailure>,
) -> Result<Option<PostVerified>, PostFailure> {
    if requires_post_proof(captured.is_ok(), kind, selected, recovering) {
        post().map(Some)
    } else {
        Ok(None)
    }
}
fn publish_error(context: &mut Context, state: &State, instance: &str, mut error: AppError) {
    // Take the one-shot typed decision even when a fence wins this race.
    let decision = context.post_decision.take();
    if error.code == "preview_cancelled" || !state.current(&context.source) {
        return;
    }
    let (retention, disposition, strict) = match decision {
        Some(AttemptDecision::Retain(decision)) => {
            let Ok(actual) = AttemptKey::from_context(context) else {
                return;
            };
            match consume_retention(decision, &actual, state, || {
                macos::preview_permission(false)
            }) {
                Ok(permit) => (Some(permit), None, false),
                Err(failed) if failed.code == "preview_cancelled" => return,
                Err(failed) => {
                    error = failed;
                    (None, Some(PostDisposition::Invalid), true)
                }
            }
        }
        Some(AttemptDecision::Release {
            key,
            disposition,
            strict,
        }) => {
            let Ok(actual) = AttemptKey::from_context(context) else {
                return;
            };
            if state.commit_key(&key, &actual).is_err() {
                return;
            }
            (None, Some(disposition), strict)
        }
        None => (None, None, false),
    };
    if let Some(value) = &mut context.diagnostic {
        value.finish_failure(&error.code);
        if let Some(disposition) = disposition {
            value.post_proof = Some(disposition.word().into());
        }
        if error.code == "preview_selection_timeout" && value.timeout_source.is_none() {
            value.stage = "picker_rust_timeout".into();
            value.timeout_source = Some("rust".into());
        }
        if error.code == "preview_selection_mismatch" {
            value.fixed_failure_reason(&error);
        }
    }
    apply_attempt_failure(
        &mut context.choice,
        &mut context.terminal,
        &mut context.new_capture,
        &error.code,
        retention,
        disposition,
        strict,
    );
    if context.choice.is_none() {
        if let Some(value) = &mut context.diagnostic {
            if let Some(recovery) = &mut value.recovery {
                recovery.state = "released".into();
            }
        }
        context.recovery = Recovery::default();
    }
    let mut status = state.event(
        instance,
        &context.source,
        &context.capture,
        "Unavailable",
        Some(error.code),
    );
    status.sequence = context.sequence;
    status.diagnostic = context.diagnostic.clone();
    if !state.emit(status) {
        state.stop();
    }
}

fn check_attempt(state: &State, key: &AttemptKey) -> AppResult<()> {
    // Current/fence takes precedence over the capture or external verifier error.
    if state.stopped.load(Ordering::Acquire)
        || !state.intents.lock().ok().is_some_and(|intents| {
            intents
                .get(&key.source)
                .is_some_and(|intent| key.matches_intent(intent))
        })
    {
        return Err(failure("preview_cancelled"));
    }
    if !macos::preview_permission(false) {
        return Err(failure("preview_permission_revoked"));
    }
    Ok(())
}
fn recoverable_kind(kind: Option<macos::CaptureFailureKind>) -> bool {
    matches!(
        kind,
        Some(
            macos::CaptureFailureKind::CompatibilitySourceCap
                | macos::CaptureFailureKind::GeometryUnavailable
        )
    )
}
fn post_capture(
    context: &Context,
    state: &State,
    key: &AttemptKey,
) -> Result<PostVerified, PostFailure> {
    let current = || {
        if !state.attempt_current(context, key) {
            return Err(failure("preview_cancelled"));
        }
        check_attempt(state, key)
    };
    complete_post_proof(
        key,
        current,
        || {
            context
                .verifier
                .verify(&|| !state.attempt_current(context, key))
        },
        || {
            context
                .choice
                .as_ref()
                .ok_or_else(|| failure("preview_selection_required"))?
                .validate(&context.verifier)
        },
        |key| state.commit_post(context, key),
    )
}
fn capture(
    context: &mut Context,
    state: &State,
    instance: &str,
    expected_geometry: Option<[u64; 8]>,
) -> AppResult<()> {
    let recovery_admission = expected_geometry.is_some();
    let result = capture_attempt(context, state, instance, expected_geometry);
    if recovery_admission && result.is_err() {
        if context.post_decision.is_none() {
            if let Ok(key) = AttemptKey::from_context(context) {
                let disposition =
                    PostFailure::new(result.as_ref().err().expect("failed attempt").clone())
                        .disposition;
                context.post_decision = Some(AttemptDecision::Release {
                    key,
                    disposition,
                    strict: true,
                });
            }
        }
        let value = context
            .diagnostic
            .get_or_insert_with(|| macos::PreviewDiagnostic::worker("capture"));
        if value.post_proof.is_none() {
            value.post_proof = Some(
                if result
                    .as_ref()
                    .err()
                    .is_some_and(|e| e.code == "preview_cancelled")
                {
                    "superseded"
                } else {
                    "release_unproved"
                }
                .into(),
            );
        }
        value.recovery = Some(context.recovery.diagnostic(false, true));
    }
    result
}
fn capture_attempt(
    context: &mut Context,
    state: &State,
    instance: &str,
    expected_geometry: Option<[u64; 8]>,
) -> AppResult<()> {
    diagnostic_stage(context, "capture");
    let admitted_at = Instant::now();
    let key = AttemptKey::from_context(context)?;
    let source = context.source.clone();
    let current = || check_attempt(state, &key);
    current()?;
    if macos::capture_busy() {
        return Err(failure("preview_capture_busy"));
    }
    let before = now();
    diagnostic_stage(context, "win_pre");
    let pre_verified = context.verifier.verify(&|| current().is_err());
    current()?;
    pre_verified?;
    diagnostic_stage(context, "owner_pre");
    let choice = context
        .choice
        .as_ref()
        .ok_or_else(|| failure("preview_selection_required"))?;
    let pre_validated = choice.validate(&context.verifier);
    current()?;
    pre_validated?;
    if macos::capture_busy() {
        return Err(failure("preview_capture_busy"));
    }
    let (attempt, refusal) = if let Some(expected) = expected_geometry {
        let selected = match choice {
            Choice::Selected(selected) => selected,
            _ => return Err(failure("preview_selection_expired")),
        };
        let geometry = macos::selection_geometry(&selected.identity, selected.token());
        current()?;
        match geometry {
            Ok(geometry) => capture_after_geometry(geometry, expected, || choice.capture(&current)),
            Err(error) => return Err(native_capture_error(error)),
        }
    } else {
        (choice.capture(&current), None)
    };
    let macos::PreviewCaptureAttempt {
        result: captured,
        diagnostic,
        failure_kind,
    } = attempt;
    let mut value = diagnostic.unwrap_or_else(|| macos::PreviewDiagnostic::worker("capture"));
    value.inherit_picker(context.diagnostic.as_ref());
    value.request = context.authority;
    value.failure_kind = failure_kind;
    context.diagnostic = Some(value);
    // Keep the Result intact until the complete same-attempt post proof has run.
    // A Native status or AppError string never selects this retention exception.
    let typed_refusal = captured.is_err()
        && recoverable_kind(failure_kind)
        && matches!(choice, Choice::Selected(_));
    let proof = match complete_capture_post(
        &captured,
        failure_kind,
        matches!(choice, Choice::Selected(_)),
        expected_geometry.is_some(),
        || post_capture(context, state, &key),
    ) {
        Ok(proof) => proof,
        Err(failed) => {
            context.post_decision = Some(AttemptDecision::Release {
                key: key.clone(),
                disposition: failed.disposition,
                strict: typed_refusal || expected_geometry.is_some(),
            });
            if let Some(value) = &mut context.diagnostic {
                value.post_proof = Some(failed.disposition.word().into());
            }
            return Err(failed.error);
        }
    };
    if let Some(proof) = proof {
        if typed_refusal || refusal == Some(WorkerRefusal::FreshGeometryChanged) {
            let cause = if refusal.is_some() {
                RetentionCause::FreshGeometryChanged
            } else if failure_kind == Some(macos::CaptureFailureKind::CompatibilitySourceCap) {
                RetentionCause::CompatibilitySourceCap
            } else {
                RetentionCause::GeometryUnavailable
            };
            context.post_decision =
                Some(AttemptDecision::Retain(proof.into_retention(&key, cause)?));
            if expected_geometry.is_none() {
                context.recovery.trigger(admitted_at, Instant::now());
            } else {
                if refusal.is_some() {
                    context.recovery.clear_authority();
                }
                context.recovery.after_metadata(Instant::now());
            }
            if let Some(value) = &mut context.diagnostic {
                value.post_proof = Some("post_verified_retain".into());
                let mut recovery = context.recovery.diagnostic(false, true);
                recovery.state = "geometry_blocked".into();
                value.recovery = Some(recovery);
            }
            return captured.map(|_| ());
        }
        proof.consume(&key)?;
        if captured.is_err() {
            context.post_decision = Some(AttemptDecision::Release {
                key: key.clone(),
                disposition: PostDisposition::Unproved,
                strict: expected_geometry.is_some(),
            });
        }
        if let Some(value) = &mut context.diagnostic {
            value.post_proof = Some(
                if captured.is_ok() {
                    "verified_frame"
                } else {
                    "release_unproved"
                }
                .into(),
            );
        }
    }
    let frame = captured?;
    current()?;
    context.sequence = context
        .sequence
        .checked_add(1)
        .ok_or_else(|| failure("collector_sequence"))?;
    let native = context
        .choice
        .as_ref()
        .ok_or_else(|| failure("preview_selection_required"))?
        .identity();
    let _last_reported_on_screen = native.on_screen;
    let after = now();
    let header = FrameHeader {
        v: protocol::VERSION,
        instance: instance.into(),
        source: source.id.clone(),
        capture_id: context.capture.clone(),
        frame_epoch: source.frame_epoch.clone(),
        epoch: source.epoch.clone(),
        sequence: context.sequence,
        target_identity: source.target.candidate_id().into(),
        width: frame.width,
        height: frame.height,
        captured_at_ms: frame.captured_at_ms,
        proof_before_ms: before,
        proof_after_ms: after,
        host_pid: native.host_pid,
        host_start: native.host_process_start.clone(),
        window_id: native.window_id,
        content_hash: content_hash(&frame.bytes),
    };
    if context.new_capture {
        let mut ready = state.event(instance, &source, &context.capture, "CaptureReady", None);
        ready.authority = Some(context.authority);
        ready.verified_at_ms = Some(after);
        if !state.emit(ready) {
            return Err(failure("preview_worker_transport"));
        }
        context.new_capture = false;
    }
    state.frames.replace(Frame {
        header,
        bytes: frame.bytes,
    });
    context.recovery.frame_published();
    let mut status = state.event(instance, &source, &context.capture, "Live", None);
    if let Some(value) = &mut context.diagnostic {
        value.stage = "frame_published".into();
        value.recovery = Some(
            context
                .recovery
                .diagnostic(false, expected_geometry.is_some()),
        );
    }
    status.diagnostic = context.diagnostic.clone();
    status.sequence = context.sequence;
    status.verified_at_ms = Some(after);
    if !state.emit(status) {
        return Err(failure("preview_worker_transport"));
    }
    Ok(())
}
// Metadata is scheduled by this producer, not a view/timer. The Native adapter
// owns the same single-flight gate as capture, including timed-out pending work.
fn observe_recovery(context: &mut Context, state: &State) -> AppResult<Option<[u64; 8]>> {
    diagnostic_stage(context, "geometry_observation");
    let key = AttemptKey::from_context(context)?;
    check_attempt(state, &key)?;
    let geometry = match context.choice.as_ref() {
        Some(Choice::Selected(selected)) => {
            macos::selection_geometry(&selected.identity, selected.token())
        }
        _ => return Err(failure("preview_selection_expired")),
    };
    let returned = Instant::now();
    context.recovery.after_metadata(returned);
    // Native errors also take this unconditional fence/TCC cleanup path.
    check_attempt(state, &key)?;
    let signature = match geometry {
        Ok(geometry) => {
            let signature = geometry.signature();
            let mut diagnostic = macos::PreviewDiagnostic::worker("geometry_observation");
            diagnostic.inherit_picker(context.diagnostic.as_ref());
            diagnostic.request = context.authority;
            diagnostic.geometry = Some(geometry);
            context.diagnostic = Some(diagnostic);
            signature
        }
        Err(error)
            if matches!(
                error.code.as_str(),
                "preview_capture_busy"
                    | "preview_capture_timeout"
                    | "preview_geometry_unavailable"
                    | "preview_window_not_on_screen"
            ) =>
        {
            None
        }
        Err(error) => return Err(native_capture_error(error)),
    };
    let stable = context.recovery.observe(&key, signature, returned);
    // Charge before any Windows verification, SCK, compatibility screenshot or
    // fresh geometry recheck. None of their errors refunds this admission.
    let admitted = stable && context.recovery.admit(returned);
    if let Some(value) = &mut context.diagnostic {
        value.recovery = Some(context.recovery.diagnostic(!admitted, admitted));
    }
    Ok(if admitted { signature } else { None })
}

fn next_command(receiver: &mpsc::Receiver<Control>, state: &State) -> Option<Control> {
    receiver
        .try_recv()
        .ok()
        .or_else(|| state.deferred.lock().ok()?.take())
}
fn project_intent(source: &mut Source, intent: &Intent) -> bool {
    let changed = source.epoch != intent.epoch || source.frame_epoch != intent.frame_epoch;
    source.epoch = intent.epoch.clone();
    source.frame_epoch = intent.frame_epoch.clone();
    source.paused = intent.paused;
    changed
}
fn reconcile_intents(
    contexts: &mut BTreeMap<String, Context>,
    retiring: &mut BTreeMap<String, (String, Context)>,
    state: &State,
) {
    let Ok(intents) = state.intents.lock().map(|m| m.clone()) else {
        return;
    };
    let keys = contexts.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        if let Some(intent) = intents.get(&key) {
            if intent.released {
                let context = contexts.remove(&key).expect("owned context");
                retiring.insert(key, (intent.epoch.clone(), context));
            } else if let Some(context) = contexts.get_mut(&key)
                && project_intent(&mut context.source, intent)
            {
                context.sequence = 0;
                context.recovery.clear_authority();
                context.post_decision = None;
                if !intent.paused {
                    context.terminal = false;
                }
                context.last_attempt = Instant::now() - Duration::from_secs(2);
            }
        }
    }
}
fn release_empty_intents(
    contexts: &BTreeMap<String, Context>,
    retiring: &BTreeMap<String, (String, Context)>,
    state: &State,
    instance: &str,
) {
    let done = {
        let Ok(mut intents) = state.intents.lock() else {
            return;
        };
        let done = intents
            .iter()
            .filter(|(id, i)| {
                i.released && !contexts.contains_key(*id) && !retiring.contains_key(*id)
            })
            .map(|(id, i)| (id.clone(), i.epoch.clone()))
            .collect::<Vec<_>>();
        for (id, _) in &done {
            intents.remove(id);
        }
        done
    };
    for (id, epoch) in done {
        if !state.emit(lifecycle_status(instance, "Released", &id, &epoch)) {
            state.stop();
        }
    }
}
fn capture_eligible(has_choice: bool, picker_pending: bool) -> bool {
    has_choice || !picker_pending
}
fn produce(receiver: mpsc::Receiver<Control>, state: &Arc<State>, instance: &str) {
    let mut contexts = BTreeMap::<String, Context>::new();
    let mut retiring = BTreeMap::<String, (String, Context)>::new();
    let resources = match resources() {
        Ok(r) => r,
        Err(_) => return,
    };
    while !state.stopped.load(Ordering::Acquire) {
        // Controls and fresh proof requests use this same serial scheduler, before
        // starting another capture. There is no proof side-channel or second broker.
        while let Some(command) = next_command(&receiver, state) {
            match command.op {
                ControlOp::Prepare { source, select } => {
                    let source = *source;
                    let key = source.id.clone();
                    if !contexts.contains_key(&key) {
                        match PreviewCollectorVerifier::open(
                            resources.clone(),
                            PathBuf::new(),
                            source.target.clone(),
                        ) {
                            Ok(verifier) => {
                                contexts.insert(
                                    key.clone(),
                                    Context {
                                        source: source.clone(),
                                        verifier,
                                        capture: id("CollectorCapture"),
                                        choice: None,
                                        sequence: 0,
                                        last_attempt: Instant::now() - Duration::from_secs(2),
                                        terminal: false,
                                        authority: command.request,
                                        pending_picker: None,
                                        new_capture: false,
                                        reselect_queued: false,
                                        diagnostic: None,
                                        recovery: Recovery::default(),
                                        post_decision: None,
                                    },
                                );
                            }
                            Err(e) => {
                                let _ = state.emit(state.event(
                                    instance,
                                    &source,
                                    "",
                                    "Unavailable",
                                    Some(e.code),
                                ));
                                continue;
                            }
                        }
                    }
                    let context = contexts.get_mut(&key).expect("inserted context");
                    if context.source.target.candidate_id() != source.target.candidate_id() {
                        state.stop();
                        break;
                    }
                    context.source = source;
                    if context.authority != command.request {
                        context.recovery.clear_authority();
                        context.post_decision = None;
                    }
                    context.authority = command.request;
                    context.terminal = false;
                    if select && context.pending_picker.is_some() {
                        context.reselect_queued = true;
                        continue;
                    }
                    if let Err(e) = prepare(context, state, instance, select) {
                        if select && e.code == "preview_capture_busy" {
                            context.reselect_queued = true;
                        }
                        publish_error(context, state, instance, e);
                    }
                }
                ControlOp::Fence {
                    source,
                    epoch,
                    frame_epoch,
                    retire,
                    paused,
                    ..
                } => {
                    for (id, old) in retire {
                        if let Some(context) = contexts.remove(&id) {
                            retiring.insert(id, (old, context));
                        } else {
                            let _ = state.emit(lifecycle_status(instance, "Released", &id, &old));
                        }
                    }
                    if let Some(context) = contexts.get_mut(&source) {
                        context.source.epoch = epoch;
                        context.source.frame_epoch = frame_epoch;
                        context.source.paused = paused;
                        context.sequence = 0;
                        context.recovery.clear_authority();
                        context.post_decision = None;
                        if !paused {
                            context.terminal = false;
                        }
                        context.last_attempt = Instant::now() - Duration::from_secs(2);
                    }
                }
                ControlOp::Release { source, epoch } => {
                    if let Some(context) = contexts.remove(&source) {
                        retiring.insert(source, (epoch, context));
                    } else {
                        if let Ok(mut intents) = state.intents.lock() {
                            intents.remove(&source);
                        }
                        let _ = state.emit(lifecycle_status(instance, "Released", &source, &epoch));
                    }
                }
                ControlOp::Verify {
                    source,
                    epoch,
                    capture,
                    frame_epoch,
                } => {
                    if let Some(context) = contexts.get_mut(&source) {
                        let mut status =
                            state.event(instance, &context.source, &context.capture, "Proof", None);
                        status.request = Some(command.request);
                        let result = if !macos::preview_permission(false) {
                            Err(failure("preview_permission_revoked"))
                        } else if macos::capture_busy() {
                            Err(failure("preview_capture_busy"))
                        } else if context.source.epoch != epoch
                            || context.capture != capture
                            || context.source.frame_epoch != frame_epoch
                        {
                            Err(failure("preview_generation_conflict"))
                        } else {
                            let stopped = || !state.current(&context.source);
                            context.verifier.verify(&stopped).and_then(|()| {
                                context
                                    .choice
                                    .as_ref()
                                    .ok_or_else(|| failure("preview_selection_required"))?
                                    .validate(&context.verifier)?;
                                if !macos::preview_permission(false) {
                                    return Err(failure("preview_permission_revoked"));
                                }
                                Ok(())
                            })
                        };
                        match result {
                            Ok(()) => status.verified_at_ms = Some(now()),
                            Err(e) => {
                                status.error_code = Some(e.code.clone());
                                if matches!(
                                    e.code.as_str(),
                                    "preview_permission_revoked"
                                        | "preview_permission_denied"
                                        | "preview_target_closed"
                                        | "preview_selection_expired"
                                        | "preview_selection_mismatch"
                                ) {
                                    publish_error(context, state, instance, e);
                                }
                            }
                        }
                        if !state.emit(status) {
                            state.stop();
                        }
                    }
                }
                ControlOp::Shutdown => state.stop(),
            }
        }
        reconcile_intents(&mut contexts, &mut retiring, state);
        release_empty_intents(&contexts, &retiring, state, instance);
        drain_retiring(&mut retiring, state, instance);
        if state.stopped.load(Ordering::Acquire) {
            break;
        }
        for context in contexts.values_mut() {
            observe_picker(context, state, instance);
            let source_id = context.source.id.clone();
            let pending_request = context.pending_picker.as_ref().map(|value| value.request);
            let mut final_snapshot = None;
            let completed = poll_selection(
                &mut context.pending_picker,
                |request| state.selection_current(&source_id, request),
                |token| {
                    // A result can arrive after observe_picker. Read its final
                    // metadata before active()==0 consumes the exact receipt.
                    final_snapshot = macos::picker_diagnostic(token);
                    picker_active(token)
                },
            );
            if let Some(mut value) = final_snapshot
                && let Some(request) = pending_request
            {
                value.request = request;
                context.diagnostic = Some(value);
            }
            if let Some(result) = completed {
                match result {
                    Ok((request, selected)) => {
                        context.authority = request;
                        context.choice = Some(Choice::Selected(selected));
                        context.recovery = Recovery::default();
                        context.post_decision = None;
                        context.capture = id("CollectorCapture");
                        context.sequence = 0;
                        context.new_capture = true;
                        context.terminal = false;
                    }
                    Err(e) => publish_error(context, state, instance, e),
                }
            }
        }
        for context in contexts.values_mut() {
            if context.reselect_queued && context.pending_picker.is_none() {
                context.reselect_queued = false;
                if let Err(error) = prepare(context, state, instance, true) {
                    if error.code == "preview_capture_busy" {
                        context.reselect_queued = true;
                    }
                    publish_error(context, state, instance, error);
                }
            }
        }
        let key = contexts
            .iter()
            .filter(|(_, c)| {
                state.current(&c.source)
                    && !c.terminal
                    && capture_eligible(c.choice.is_some(), c.pending_picker.is_some())
                    && if c.recovery.blocked {
                        c.recovery.due(Instant::now())
                    } else {
                        c.last_attempt.elapsed() >= Duration::from_secs(1)
                    }
            })
            .min_by_key(|(_, c)| c.last_attempt)
            .map(|(key, _)| key.clone());
        if let Some(key) = key {
            let context = contexts.get_mut(&key).expect("selected context");
            context.last_attempt = Instant::now();
            if context.choice.is_none()
                && let Err(e) = prepare(context, state, instance, false)
            {
                publish_error(context, state, instance, e);
                continue;
            }
            let expected_geometry = if context.recovery.blocked {
                match observe_recovery(context, state) {
                    Ok(Some(signature)) => Some(signature),
                    Ok(None) => {
                        if context.recovery.report_changed() {
                            let mut status = state.event(
                                instance,
                                &context.source,
                                &context.capture,
                                "Unavailable",
                                Some("preview_geometry_unavailable".into()),
                            );
                            status.sequence = context.sequence;
                            status.diagnostic = context.diagnostic.clone();
                            if !state.emit(status) {
                                state.stop();
                            }
                        }
                        continue;
                    }
                    Err(error) => {
                        publish_error(context, state, instance, error);
                        continue;
                    }
                }
            } else {
                None
            };
            if let Err(e) = capture(context, state, instance, expected_geometry) {
                publish_error(context, state, instance, e);
            }
            context.last_attempt = Instant::now();
        } else {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    // Stop fences immediately, but native picker callbacks and retained filter
    // results must actually drain before Stopped/RunLoop exit.
    for (id, context) in std::mem::take(&mut contexts) {
        let epoch = context.source.epoch.clone();
        retiring.insert(id, (epoch, context));
    }
    while !retiring.is_empty() {
        reconcile_intents(&mut contexts, &mut retiring, state);
        release_empty_intents(&contexts, &retiring, state, instance);
        drain_retiring(&mut retiring, state, instance);
        if !retiring.is_empty() {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    state.frames.clear();
}
fn drain_retiring(
    retiring: &mut BTreeMap<String, (String, Context)>,
    state: &State,
    instance: &str,
) {
    let mut done = Vec::new();
    for (id, (_, context)) in retiring.iter_mut() {
        // Snapshot before active()==0 consumes the exact Native receipt.
        if let Some(pending) = context.pending_picker.as_ref()
            && let Some(mut value) = macos::picker_diagnostic(&pending.transaction)
        {
            value.request = pending.request;
            context.diagnostic = Some(value);
        }
        let _ = poll_selection(&mut context.pending_picker, |_| false, picker_active);
        if context.pending_picker.is_none() && !macos::capture_busy() {
            done.push(id.clone());
        }
    }
    for id in done {
        let (epoch, context) = retiring.remove(&id).expect("retiring context");
        drop(context);
        if let Ok(mut intents) = state.intents.lock() {
            intents.remove(&id);
        }
        if !state.emit(lifecycle_status(instance, "Released", &id, &epoch)) {
            state.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, AtomicUsize};
    fn test_state() -> (Arc<State>, StatusReceiver, mpsc::Receiver<Control>) {
        let (tx, rx) = status_channel();
        let (commands, requests) = mpsc::sync_channel(8);
        (
            Arc::new(State {
                stopped: AtomicBool::new(false),
                finished: AtomicBool::new(false),
                intents: Mutex::new(BTreeMap::new()),
                frames: Latest::default(),
                critical: tx,
                commands,
                deferred: Mutex::new(None),
                permission_requested: AtomicBool::new(false),
                stop_accepted: AtomicBool::new(false),
                final_status_sent: AtomicBool::new(false),
            }),
            rx,
            requests,
        )
    }
    #[test]
    fn wake_native_on_screen_gate_keeps_automatic_mapping_strict() {
        unsafe extern "C" {
            fn luna_preview_capture_on_screen_allowed(preserved: i32, on_screen: i32) -> i32;
        }
        for (preserved, on_screen, allowed) in [(0, 0, 0), (0, 1, 1), (1, 0, 1), (1, 1, 1)] {
            // SAFETY: This is the exact pure Native gate used by CaptureWindow;
            // existing Native single-window/owner checks still precede this gate.
            let actual = unsafe { luna_preview_capture_on_screen_allowed(preserved, on_screen) };
            assert_eq!(actual, allowed);
        }
    }
    #[test]
    fn wake_retry_does_not_preserve_invalid_identity_permission_or_budget() {
        for code in [
            "preview_budget_exceeded",
            "preview_selection_expired",
            "preview_selection_mismatch",
            "preview_target_closed",
            "preview_target_changed",
            "preview_permission_denied",
            "preview_permission_revoked",
        ] {
            let dropped = Arc::new(AtomicUsize::new(0));
            let mut choice = Some(Resource(dropped.clone()));
            let (mut terminal, mut new_capture) = (false, true);
            apply_capture_failure(&mut choice, &mut terminal, &mut new_capture, code);
            assert!(
                choice.is_none(),
                "{code} cannot retain an invalid capability"
            );
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
            if code.starts_with("preview_permission_") {
                // Preserve the existing permission recovery contract: the old
                // choice is gone and a grant cannot resurrect it automatically.
                assert!(!new_capture);
                assert!(!terminal);
            } else {
                assert!(terminal, "{code} remains terminal");
            }
        }
    }
    #[test]
    fn wake_retry_keeps_original_choice_for_encoding_and_same_window_resize() {
        for code in ["preview_encoding_failed", "preview_source_changed"] {
            let dropped = Arc::new(AtomicUsize::new(0));
            let mut choice = Some(Resource(dropped.clone()));
            let (mut terminal, mut new_capture) = (false, true);
            apply_capture_failure(&mut choice, &mut terminal, &mut new_capture, code);
            assert!(
                !terminal,
                "{code} must remain eligible for the existing scheduler"
            );
            assert!(
                choice.is_some(),
                "{code} must preserve the exact picker capability"
            );
            assert_eq!(dropped.load(Ordering::SeqCst), 0);
            assert!(capture_eligible(choice.is_some(), false));
            // A successful next attempt reuses this resource; no picker or worker
            // is recreated. Explicit cleanup still drops it exactly once.
            drop(choice);
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn external_parent_without_real_same_executable_is_rejected_before_appkit() {
        assert!(!macos::collector_parent(0));
        assert!(!macos::collector_parent(std::process::id()));
        assert!(!macos::collector_private_fd(-1));
        assert!(!macos::collector_private_fd(0));
        let hello = Hello {
            v: protocol::VERSION,
            nonce: "untrusted".into(),
            parent: macos::collector_process_identity(std::process::id()).unwrap(),
        };
        assert!(!valid_hello(&hello));
    }
    #[test]
    fn production_partial_writer_survives_full_socket_and_delivers_latest_frame_after_reader_resumes()
     {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let frames = Arc::new(Latest::default());
        frames.replace(super::super::tests::frame(1, protocol::MAX_FRAME));
        let slot = frames.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopping = stopped.clone();
        let join = std::thread::spawn(move || send_frames(&mut writer, &slot, &stopping));
        std::thread::sleep(Duration::from_millis(150));
        frames.replace(super::super::tests::frame(2, protocol::MAX_FRAME));
        frames.replace(super::super::tests::frame(3, protocol::MAX_FRAME));
        let (first, bytes): (FrameHeader, _) = protocol::read_frame(&mut reader).unwrap();
        assert_eq!(first.sequence, 1);
        assert_eq!(bytes.len(), protocol::MAX_FRAME);
        let (next, _): (FrameHeader, _) = protocol::read_frame(&mut reader).unwrap();
        assert_eq!(next.sequence, 3);
        stopped.store(true, Ordering::Release);
        frames.changed.notify_all();
        join.join().unwrap().unwrap();
    }
    #[test]
    fn production_partial_writer_can_be_stopped_without_a_reader() {
        let (mut writer, _unread) = UnixStream::pair().unwrap();
        let frames = Arc::new(Latest::default());
        frames.replace(super::super::tests::frame(1, protocol::MAX_FRAME));
        let slot = frames.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopping = stopped.clone();
        let (done, received) = mpsc::sync_channel(1);
        let join = std::thread::spawn(move || {
            done.send(send_frames(&mut writer, &slot, &stopping).is_err())
                .unwrap()
        });
        std::thread::sleep(Duration::from_millis(100));
        stopped.store(true, Ordering::Release);
        frames.changed.notify_all();
        assert!(received.recv_timeout(Duration::from_secs(1)).unwrap());
        join.join().unwrap();
    }
    #[test]
    fn production_control_eof_shutdown_replay_wrong_instance_and_unknown_field_drain_with_blocked_frames()
     {
        for ending in [
            "eof",
            "shutdown",
            "replay",
            "wrong-instance",
            "unknown-field",
            "valid-control",
        ] {
            let (state, critical, requests) = test_state();
            state
                .frames
                .replace(super::super::tests::frame(1, protocol::MAX_FRAME));
            let (mut data, _unread) = UnixStream::pair().unwrap();
            let data_state = state.clone();
            let (done, received) = mpsc::sync_channel(1);
            let sender = std::thread::spawn(move || {
                done.send(send_frames(
                    &mut data,
                    &data_state.frames,
                    &data_state.stopped,
                ))
                .unwrap()
            });
            assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
            let (mut commands, mut parent) = UnixStream::pair().unwrap();
            let reading = state.clone();
            let reader =
                std::thread::spawn(move || read_commands(&mut commands, &reading, "instance"));
            protocol::write_json(
                &mut parent,
                &Control {
                    v: protocol::VERSION,
                    instance: "instance".into(),
                    request: 1,
                    op: ControlOp::Prepare {
                        source: Box::new(super::super::tests::source()),
                        select: false,
                    },
                },
            )
            .unwrap();
            assert!(matches!(
                requests.recv_timeout(Duration::from_secs(1)).unwrap().op,
                ControlOp::Prepare { .. }
            ));
            if ending == "eof" {
                parent.shutdown(std::net::Shutdown::Write).unwrap();
            } else {
                let command = Control {
                    v: protocol::VERSION,
                    instance: if ending == "wrong-instance" {
                        "stale"
                    } else {
                        "instance"
                    }
                    .into(),
                    request: if ending == "replay" { 1 } else { 2 },
                    op: if ending == "shutdown" {
                        ControlOp::Shutdown
                    } else {
                        ControlOp::Fence {
                            source: super::super::tests::source().id,
                            previous_epoch: "1".into(),
                            epoch: "2".into(),
                            frame_epoch: "frame-2".into(),
                            paused: false,
                            retire: vec![],
                        }
                    },
                };
                if ending == "unknown-field" {
                    let mut value = serde_json::to_value(command).unwrap();
                    value["unexpected"] = true.into();
                    protocol::write_json(&mut parent, &value).unwrap();
                } else {
                    protocol::write_json(&mut parent, &command).unwrap();
                }
                if ending == "valid-control" {
                    assert!(matches!(
                        requests.recv_timeout(Duration::from_secs(1)).unwrap().op,
                        ControlOp::Fence { .. }
                    ));
                    assert!(!state.stopped.load(Ordering::Acquire));
                    assert!(critical.try_recv().is_err());
                    assert!(received.try_recv().is_err());
                    assert_eq!(
                        state.intents.lock().unwrap().get("source").unwrap().epoch,
                        "2"
                    );
                    protocol::write_json(
                        &mut parent,
                        &Control {
                            v: protocol::VERSION,
                            instance: "instance".into(),
                            request: 3,
                            op: ControlOp::Shutdown,
                        },
                    )
                    .unwrap();
                }
            }
            assert_eq!(
                critical.recv_timeout(Duration::from_secs(1)).unwrap().phase,
                "StopAcceptedDraining",
                "{ending}"
            );
            reader.join().unwrap();
            assert!(state.stopped.load(Ordering::Acquire));
            assert!(
                received
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .is_err()
            );
            sender.join().unwrap();
            assert!(critical.try_recv().is_err());
        }
    }
    #[test]
    fn critical_draining_ack_precedes_stopped_and_is_not_a_frame_or_coalesced_live_status() {
        let (state, rx, _) = test_state();
        for _ in 0..100 {
            assert!(state.emit(lifecycle_status("instance", "Live", "source", "1")));
        }
        assert!(!state.stopped.load(Ordering::Acquire));
        let mut proof = lifecycle_status("instance", "Proof", "source", "1");
        proof.request = Some(7);
        assert!(state.emit(proof));
        assert!(state.emit(lifecycle_status("instance", "Released", "other", "1")));
        state.accept_stop("instance");
        state.accept_stop("instance");
        state.emit(lifecycle_status("instance", "Stopped", "", ""));
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let sending = state.clone();
        let join = std::thread::spawn(move || send_statuses(&mut writer, &rx, &sending));
        for expected in ["Proof", "Released", "StopAcceptedDraining", "Stopped"] {
            assert_eq!(
                protocol::read_json::<Status>(&mut reader).unwrap().phase,
                expected
            );
        }
        join.join().unwrap();
        assert!(state.final_status_sent.load(Ordering::Acquire));
    }
    #[test]
    fn full_status_socket_and_one_hundred_live_updates_preserve_proof_release_and_shutdown_receipts()
     {
        let (state, rx, _) = test_state();
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        writer.set_nonblocking(true).unwrap();
        let filler = [0x5a; 4096];
        let mut filled = 0usize;
        loop {
            match writer.write(&filler) {
                Ok(count) => filled += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                other => panic!("could not fill actual status socket: {other:?}"),
            }
        }
        assert!(filled > 0);
        writer.set_nonblocking(false).unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        assert!(state.emit(lifecycle_status("instance", "Live", "source", "1")));
        let sending = state.clone();
        let (done, received) = mpsc::sync_channel(1);
        let join = std::thread::spawn(move || {
            send_statuses(&mut writer, &rx, &sending);
            done.send(()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while !state.critical.0.0.lock().unwrap().latest.is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        for _ in 0..100 {
            assert!(state.emit(lifecycle_status("instance", "Live", "source", "1")));
        }
        assert!(!state.stopped.load(Ordering::Acquire));
        assert!(received.try_recv().is_err());
        let mut proof = lifecycle_status("instance", "Proof", "source", "1");
        proof.request = Some(9);
        assert!(state.emit(proof));
        assert!(state.emit(lifecycle_status("instance", "Released", "other", "1")));
        state.accept_stop("instance");
        assert!(state.emit(lifecycle_status("instance", "Stopped", "", "")));
        // Remove only the exact test filler; all following bytes are the
        // unchanged production writer's framed protocol, including a partial
        // packet already in flight while the reader was blocked.
        let mut discard = vec![0; filled];
        std::io::Read::read_exact(&mut reader, &mut discard).unwrap();
        assert!(discard.iter().all(|byte| *byte == 0x5a));
        for expected in [
            "Live",
            "Proof",
            "Released",
            "StopAcceptedDraining",
            "Stopped",
        ] {
            assert_eq!(
                protocol::read_json::<Status>(&mut reader).unwrap().phase,
                expected
            );
        }
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        join.join().unwrap();
        assert!(state.final_status_sent.load(Ordering::Acquire));
    }
    struct Resource(Arc<AtomicUsize>);
    impl Drop for Resource {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn pending<T>(
        attempted: bool,
    ) -> (Option<PendingSelection<T>>, mpsc::SyncSender<AppResult<T>>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (
            Some(PendingSelection {
                request: 9,
                transaction: "owned-transaction".into(),
                receiver: rx,
                result: None,
                attempted: Arc::new(AtomicBool::new(attempted)),
                cancelled: false,
            }),
            tx,
        )
    }
    #[test]
    fn exact_native_begin_busy_rejection_never_waits_for_a_nonexistent_transaction() {
        let (mut rejected, tx) = pending::<u64>(true);
        tx.send(Err(failure("preview_selection_busy"))).unwrap();
        assert_eq!(
            poll_selection(
                &mut rejected,
                |_| true,
                |_| panic!("begin was explicitly rejected")
            )
            .unwrap()
            .unwrap_err()
            .code,
            "preview_selection_busy"
        );
        assert!(rejected.is_none());
        let (mut accepted, tx) = pending::<u64>(true);
        tx.send(Err(failure("preview_selection_failed"))).unwrap();
        assert!(poll_selection(&mut accepted, |_| true, |_| -1).is_none());
        assert!(accepted.is_some());
        assert_eq!(
            poll_selection(&mut accepted, |_| true, |_| 0)
                .unwrap()
                .unwrap_err()
                .code,
            "preview_selection_failed"
        );
        assert!(accepted.is_none());
    }
    #[test]
    fn native_cg_identity_loss_does_not_claim_the_windows_binding_has_closed() {
        assert_eq!(
            native_capture_error(failure("preview_target_closed")).code,
            "preview_selection_expired"
        );
        assert_eq!(
            native_capture_error(failure("preview_permission_revoked")).code,
            "preview_permission_revoked"
        );
    }
    #[test]
    fn cancellation_before_native_begin_waits_for_its_thread_and_does_not_fake_inactive_receipt() {
        let (mut pending, tx) = pending::<u64>(false);
        assert!(poll_selection(&mut pending, |_| false, |_| -1).is_none());
        assert!(pending.is_some());
        tx.send(Err(failure("preview_cancelled"))).unwrap();
        assert_eq!(
            poll_selection(&mut pending, |_| false, |_| -1)
                .unwrap()
                .unwrap_err()
                .code,
            "preview_cancelled"
        );
        assert!(pending.is_none());
    }
    #[test]
    fn accepted_but_not_main_processed_and_late_callback_remain_owned_until_native_ack_and_filter_drop()
     {
        let drops = Arc::new(AtomicUsize::new(0));
        let (mut pending, tx) = pending::<Resource>(true);
        let active = AtomicI32::new(-1);
        assert!(
            poll_selection(&mut pending, |_| false, |_| active.load(Ordering::Acquire)).is_none()
        );
        tx.send(Ok(Resource(drops.clone()))).unwrap();
        assert!(
            poll_selection(&mut pending, |_| false, |_| active.load(Ordering::Acquire)).is_none()
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        active.store(1, Ordering::Release);
        assert!(
            poll_selection(&mut pending, |_| false, |_| active.load(Ordering::Acquire)).is_none()
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        active.store(0, Ordering::Release);
        assert_eq!(
            poll_selection(&mut pending, |_| false, |_| active.load(Ordering::Acquire))
                .unwrap()
                .err()
                .unwrap()
                .code,
            "preview_cancelled"
        );
        assert!(pending.is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pending_picker_does_not_block_other_source_capture_or_release_its_resource() {
        let (mut pending, tx) = pending::<u64>(true);
        for _ in 0..100 {
            assert!(poll_selection(&mut pending, |_| true, |_| 1).is_none());
            assert!(!capture_eligible(false, true));
            assert!(capture_eligible(true, false));
            assert!(capture_eligible(true, true));
        }
        tx.send(Err(failure("preview_selection_cancelled")))
            .unwrap();
        assert_eq!(
            poll_selection(&mut pending, |_| true, |_| 0)
                .unwrap()
                .unwrap_err()
                .code,
            "preview_selection_cancelled"
        );
        assert!(capture_eligible(true, false));
    }
    #[test]
    fn actual_native_picker_policy_rejects_unknown_stream_and_late_old_operations() {
        unsafe extern "C" {
            fn luna_preview_picker_callback_policy(current: i32, finished: i32, stream: i32)
            -> i32;
        }
        // SAFETY: This private Native function is the observer's pure policy;
        // no AppKit/TCC access or real picker is involved in this boundary test.
        let accepted = unsafe { luna_preview_picker_callback_policy(1, 0, 0) };
        assert_eq!(accepted, 1);
        // SAFETY: Same pure scalar policy, current operation with unknown stream.
        let unknown = unsafe { luna_preview_picker_callback_policy(1, 0, 1) };
        assert_eq!(unknown, 2);
        for (current, finished) in [(0, 0), (0, 1), (1, 1)] {
            // SAFETY: Same bounded scalar policy; old callbacks have no authority.
            let ignored = unsafe { luna_preview_picker_callback_policy(current, finished, 1) };
            assert_eq!(ignored, 0);
        }
        assert_eq!(picker_active("never-accepted-owned-test-transaction"), -1);
    }
    #[test]
    fn permission_revocation_drops_only_its_original_choice_and_grant_does_not_resurrect_old_token()
    {
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let mut a = Some(Resource(first.clone()));
        let mut b = Some(Resource(second.clone()));
        assert!(invalidate_source_choice(
            &mut a,
            "preview_permission_revoked"
        ));
        assert!(a.is_none());
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(second.load(Ordering::SeqCst), 0);
        assert!(!invalidate_source_choice(&mut a, "preview_capture_busy"));
        assert!(a.is_none());
        assert!(b.is_some());
        assert!(invalidate_source_choice(
            &mut b,
            "preview_selection_expired"
        ));
        assert_eq!(second.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn association_frame_fence_preserves_pending_selection_authority_but_exact_release_cancels_it()
    {
        let (state, _critical, requests) = test_state();
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let reading = state.clone();
        let join = std::thread::spawn(move || read_commands(&mut reader, &reading, "instance"));
        let source = super::super::tests::source();
        protocol::write_json(
            &mut writer,
            &Control {
                v: protocol::VERSION,
                instance: "instance".into(),
                request: 1,
                op: ControlOp::Prepare {
                    source: Box::new(source.clone()),
                    select: true,
                },
            },
        )
        .unwrap();
        requests.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(state.selection_current(&source.id, 1));
        protocol::write_json(
            &mut writer,
            &Control {
                v: protocol::VERSION,
                instance: "instance".into(),
                request: 2,
                op: ControlOp::Fence {
                    source: source.id.clone(),
                    previous_epoch: "1".into(),
                    epoch: "2".into(),
                    frame_epoch: "frame-2".into(),
                    paused: false,
                    retire: vec![],
                },
            },
        )
        .unwrap();
        requests.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(state.selection_current(&source.id, 1));
        assert!(!state.current(&source));
        let mut current = source.clone();
        let intent = state.intents.lock().unwrap()[&source.id].clone();
        assert!(project_intent(&mut current, &intent));
        assert!(state.current(&current));
        protocol::write_json(
            &mut writer,
            &Control {
                v: protocol::VERSION,
                instance: "instance".into(),
                request: 3,
                op: ControlOp::Release {
                    source: source.id.clone(),
                    epoch: "2".into(),
                },
            },
        )
        .unwrap();
        requests.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!state.selection_current(&source.id, 1));
        assert!(!state.current(&current));
        writer.shutdown(std::net::Shutdown::Write).unwrap();
        join.join().unwrap();
    }
    #[test]
    fn full_pipeline_control_queue_still_fences_release_without_stopping_other_sources() {
        let (state, critical, _requests) = test_state();
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let reading = state.clone();
        let join = std::thread::spawn(move || read_commands(&mut reader, &reading, "instance"));
        let source = super::super::tests::source();
        protocol::write_json(
            &mut writer,
            &Control {
                v: protocol::VERSION,
                instance: "instance".into(),
                request: 1,
                op: ControlOp::Prepare {
                    source: Box::new(source.clone()),
                    select: false,
                },
            },
        )
        .unwrap();
        for epoch in 2..=10 {
            protocol::write_json(
                &mut writer,
                &Control {
                    v: protocol::VERSION,
                    instance: "instance".into(),
                    request: epoch,
                    op: ControlOp::Fence {
                        source: source.id.clone(),
                        previous_epoch: (epoch - 1).to_string(),
                        epoch: epoch.to_string(),
                        frame_epoch: format!("frame-{epoch}"),
                        paused: false,
                        retire: vec![],
                    },
                },
            )
            .unwrap();
        }
        protocol::write_json(
            &mut writer,
            &Control {
                v: protocol::VERSION,
                instance: "instance".into(),
                request: 11,
                op: ControlOp::Release {
                    source: source.id.clone(),
                    epoch: "10".into(),
                },
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if state
                .intents
                .lock()
                .unwrap()
                .get(&source.id)
                .is_some_and(|i| i.released)
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!state.stopped.load(Ordering::Acquire));
        assert!(critical.try_recv().is_err());
        writer.shutdown(std::net::Shutdown::Write).unwrap();
        join.join().unwrap();
        assert_eq!(
            critical.recv_timeout(Duration::from_secs(1)).unwrap().phase,
            "StopAcceptedDraining"
        );
    }
    fn attempt_key() -> AttemptKey {
        AttemptKey {
            source: "source".into(),
            epoch: "1".into(),
            frame_epoch: "frame-1".into(),
            target: "target".into(),
            authority: 7,
            capture: "capture".into(),
            choice: ChoiceKey {
                selected_token: Some("original-private-capability".into()),
                host_pid: 40,
                host_start: "native-start".into(),
                window_id: 12,
            },
        }
    }
    fn install_attempt(state: &State, key: &AttemptKey) {
        state.intents.lock().unwrap().insert(
            key.source.clone(),
            Intent {
                epoch: key.epoch.clone(),
                frame_epoch: key.frame_epoch.clone(),
                paused: false,
                released: false,
                selection_request: 7000,
                target_identity: key.target.clone(),
            },
        );
    }
    fn completed_retention(
        state: &State,
        key: &AttemptKey,
        cause: RetentionCause,
    ) -> RetentionDecision {
        complete_post_proof(
            key,
            || state.commit_key(key, key),
            || Ok(()),
            || Ok(()),
            |key| state.commit_key(key, key),
        )
        .unwrap()
        .into_retention(key, cause)
        .unwrap()
    }
    #[test]
    fn recovery_typed_refusal_and_frame_route_through_the_same_production_post_helper() {
        use macos::CaptureFailureKind::*;
        for frame in [false, true] {
            for kind in [
                None,
                Some(CompatibilitySourceCap),
                Some(GeometryUnavailable),
                Some(OutputSafetyBudget),
                Some(OtherExistingFailure),
            ] {
                let routes = requires_post_proof(frame, kind, true, false);
                assert_eq!(
                    routes,
                    frame || matches!(kind, Some(CompatibilitySourceCap | GeometryUnavailable))
                );
                let captured: AppResult<()> = if frame {
                    Ok(())
                } else {
                    Err(failure("preview_budget_exceeded"))
                };
                let calls = std::cell::RefCell::new(Vec::new());
                let key = attempt_key();
                let proof = complete_capture_post(&captured, kind, true, false, || {
                    complete_post_proof(
                        &key,
                        || {
                            calls.borrow_mut().push("current-tcc");
                            Ok(())
                        },
                        || {
                            calls.borrow_mut().push("windows-file-unique-instance");
                            Ok(())
                        },
                        || {
                            calls.borrow_mut().push("original-choice-native-witness");
                            Ok(())
                        },
                        |_| {
                            calls.borrow_mut().push("short-intent-commit");
                            Ok(())
                        },
                    )
                })
                .unwrap();
                if routes {
                    proof.unwrap().consume(&key).unwrap();
                    assert_eq!(
                        *calls.borrow(),
                        [
                            "current-tcc",
                            "windows-file-unique-instance",
                            "current-tcc",
                            "original-choice-native-witness",
                            "current-tcc",
                            "short-intent-commit"
                        ]
                    );
                } else {
                    assert!(proof.is_none());
                    assert!(calls.borrow().is_empty());
                }
            }
        }
        assert!(!requires_post_proof(
            false,
            Some(CompatibilitySourceCap),
            false,
            false
        ));
        assert!(requires_post_proof(false, None, true, true));
    }
    #[test]
    fn recovery_post_verify_error_always_rechecks_fence_and_tcc_before_returning() {
        for error in [
            "preview_capture_busy",
            "preview_verification_unavailable",
            "preview_target_changed",
        ] {
            for changed in ["fence", "tcc", "none"] {
                let checks = std::cell::Cell::new(0);
                let failure = complete_post_proof(
                    &attempt_key(),
                    || {
                        checks.set(checks.get() + 1);
                        if checks.get() == 2 {
                            match changed {
                                "fence" => return Err(failure("preview_cancelled")),
                                "tcc" => return Err(failure("preview_permission_revoked")),
                                _ => (),
                            }
                        }
                        Ok(())
                    },
                    || Err(failure(error)),
                    || panic!("failed Windows proof cannot validate choice"),
                    |_| panic!("failed Windows proof cannot commit"),
                )
                .unwrap_err();
                assert_eq!(checks.get(), 2);
                match changed {
                    "fence" => {
                        assert_eq!(failure.error.code, "preview_cancelled");
                        assert_eq!(failure.disposition, PostDisposition::Superseded);
                    }
                    "tcc" => {
                        assert_eq!(failure.error.code, "preview_permission_revoked");
                        assert_eq!(failure.disposition, PostDisposition::Invalid);
                    }
                    _ => assert_eq!(failure.error.code, error),
                }
            }
        }
    }
    #[test]
    fn recovery_original_choice_error_rechecks_and_never_reuses_an_earlier_proof() {
        for error in [
            "preview_selection_expired",
            "preview_selection_mismatch",
            "preview_verification_unavailable",
            "preview_capture_busy",
        ] {
            for revoke in [false, true] {
                let checks = std::cell::Cell::new(0);
                let failed = complete_post_proof(
                    &attempt_key(),
                    || {
                        checks.set(checks.get() + 1);
                        if revoke && checks.get() == 3 {
                            Err(failure("preview_permission_revoked"))
                        } else {
                            Ok(())
                        }
                    },
                    || Ok(()),
                    || Err(failure(error)),
                    |_| panic!("incomplete proof cannot commit"),
                )
                .unwrap_err();
                assert_eq!(checks.get(), 3);
                assert_eq!(
                    failed.error.code,
                    if revoke {
                        "preview_permission_revoked"
                    } else {
                        error
                    }
                );
                let drops = Arc::new(AtomicUsize::new(0));
                let mut choice = Some(Resource(drops.clone()));
                let (mut terminal, mut new_capture) = (false, true);
                apply_attempt_failure(
                    &mut choice,
                    &mut terminal,
                    &mut new_capture,
                    &failed.error.code,
                    None,
                    Some(failed.disposition),
                    true,
                );
                assert!(choice.is_none());
                assert_eq!(drops.load(Ordering::SeqCst), 1);
                assert!(terminal);
                assert!(!new_capture);
            }
        }
    }
    #[test]
    fn recovery_complete_identity_and_short_commit_reject_every_changed_authority_component() {
        let (state, _, _) = test_state();
        let key = attempt_key();
        install_attempt(&state, &key);
        // A control/Fence request sequence is not the serial Context authority.
        assert_eq!(
            state.intents.lock().unwrap()["source"].selection_request,
            7000
        );
        assert_eq!(key.authority, 7);
        state.commit_key(&key, &key).unwrap();
        for field in 0..11 {
            let mut changed = key.clone();
            match field {
                0 => changed.source.push('x'),
                1 => changed.epoch.push('x'),
                2 => changed.frame_epoch.push('x'),
                3 => changed.target.push('x'),
                4 => changed.authority += 1,
                5 => changed.capture.push('x'),
                6 => changed.choice.selected_token = None,
                7 => changed.choice.selected_token.as_mut().unwrap().push('x'),
                8 => changed.choice.host_pid += 1,
                9 => changed.choice.host_start.push('x'),
                _ => changed.choice.window_id += 1,
            }
            assert_eq!(
                state.commit_key(&key, &changed).unwrap_err().code,
                "preview_cancelled",
                "field={field}"
            );
        }
        let new_epoch = {
            let mut intents = state.intents.lock().unwrap();
            let intent = intents.get_mut("source").unwrap();
            intent.epoch = "2".into();
            intent.frame_epoch = "frame-2".into();
            intent.clone()
        };
        // selection_current survives a Fence; it still does not prove a capture.
        assert!(state.selection_current("source", 7000));
        assert_eq!(
            complete_post_proof(
                &key,
                || Ok(()),
                || Ok(()),
                || Ok(()),
                |key| state.commit_key(key, key)
            )
            .unwrap_err()
            .disposition,
            PostDisposition::Superseded
        );
        assert_eq!(
            state.intents.lock().unwrap()["source"].epoch,
            new_epoch.epoch
        );
        let mut winner = key.clone();
        winner.epoch = new_epoch.epoch;
        winner.frame_epoch = new_epoch.frame_epoch;
        state.commit_key(&winner, &winner).unwrap();
        // An old one-shot proof cannot be relabelled to the winner.
        assert_eq!(
            PostVerified(key).consume(&winner).unwrap_err().code,
            "preview_cancelled"
        );
    }
    #[test]
    fn recovery_fence_during_blocked_verifier_cancels_old_proof_without_mutating_the_winner() {
        let (state, _, _) = test_state();
        let key = attempt_key();
        install_attempt(&state, &key);
        let old = key.clone();
        let worker_state = state.clone();
        let (entered, seen) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        let join = std::thread::spawn(move || {
            complete_post_proof(
                &old,
                || {
                    if worker_state
                        .intents
                        .lock()
                        .unwrap()
                        .get("source")
                        .is_some_and(|i| old.matches_intent(i))
                    {
                        Ok(())
                    } else {
                        Err(failure("preview_cancelled"))
                    }
                },
                || {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                    Err(failure("preview_verification_unavailable"))
                },
                || panic!("fenced verifier cannot validate"),
                |key| worker_state.commit_key(key, key),
            )
        });
        seen.recv_timeout(Duration::from_secs(1)).unwrap();
        state
            .intents
            .lock()
            .unwrap()
            .get_mut("source")
            .unwrap()
            .epoch = "winner".into();
        release.send(()).unwrap();
        assert_eq!(
            join.join().unwrap().unwrap_err().disposition,
            PostDisposition::Superseded
        );
        assert_eq!(state.intents.lock().unwrap()["source"].epoch, "winner");
    }
    #[test]
    fn recovery_typed_retention_does_not_relax_real_output_safety_or_other_existing_terminal_rules()
    {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut choice = Some(Resource(drops.clone()));
        let (mut terminal, mut new_capture) = (false, true);
        let key = attempt_key();
        let (state, _, _) = test_state();
        install_attempt(&state, &key);
        let decision = completed_retention(&state, &key, RetentionCause::CompatibilitySourceCap);
        let permit = consume_retention(decision, &key, &state, || true).unwrap();
        apply_attempt_failure(
            &mut choice,
            &mut terminal,
            &mut new_capture,
            "preview_budget_exceeded",
            Some(permit),
            None,
            false,
        );
        assert!(choice.is_some());
        assert!(!terminal);
        assert!(new_capture);
        apply_attempt_failure(
            &mut choice,
            &mut terminal,
            &mut new_capture,
            "preview_budget_exceeded",
            None,
            None,
            false,
        );
        assert!(choice.is_none());
        assert!(terminal);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        // Existing successful-frame post failure policy remains transient.
        let mut choice = Some(1u8);
        let (mut terminal, mut capture) = (false, true);
        apply_attempt_failure(
            &mut choice,
            &mut terminal,
            &mut capture,
            "preview_verification_unavailable",
            None,
            Some(PostDisposition::Unproved),
            false,
        );
        assert_eq!(choice, Some(1));
        assert!(!terminal);
    }
    #[test]
    fn recovery_bucket_charges_trigger_and_admissions_before_work_without_refund() {
        let at = Instant::now();
        let mut recovery = Recovery::default();
        recovery.trigger(at, at);
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 2);
        assert_eq!(recovery.bucket.as_ref().unwrap().anchor, at);
        for (index, rejection) in ["preview_capture_busy", "preview_capture_timeout"]
            .into_iter()
            .enumerate()
        {
            assert!(recovery.admit(at));
            let failed = complete_post_proof(
                &attempt_key(),
                || Ok(()),
                || Err(failure(rejection)),
                || panic!("proof failed"),
                |_| panic!("proof failed"),
            )
            .unwrap_err();
            assert_eq!(failed.error.code, rejection);
            assert_eq!(failed.disposition, PostDisposition::Unproved);
            assert_eq!(recovery.bucket.as_ref().unwrap().credits, 1 - index as u8);
        }
        assert!(!recovery.admit(at));
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 0);
        assert!(!recovery.normal_trigger_without_credit);
        recovery.frame_published();
        recovery.trigger(at, at);
        assert!(recovery.blocked);
        assert!(recovery.normal_trigger_without_credit);
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 0);
        assert_eq!(recovery.bucket.as_ref().unwrap().anchor, at);
    }
    #[test]
    fn recovery_bucket_monotonic_boundary_and_large_elapsed_use_fixed_refill_grid() {
        let at = Instant::now();
        let mut bucket = RecoveryBucket {
            credits: 0,
            anchor: at,
        };
        bucket.refill(at + Duration::from_millis(19_999));
        assert_eq!(bucket.credits, 0);
        assert_eq!(bucket.anchor, at);
        bucket.refill(at + Duration::from_secs(20));
        assert_eq!(bucket.credits, 1);
        assert_eq!(bucket.anchor, at + Duration::from_secs(20));
        bucket.refill(at + Duration::from_secs(100_001));
        assert_eq!(bucket.credits, 3);
        assert_eq!(bucket.anchor, at + Duration::from_secs(100_000));
        bucket.credits = 0;
        bucket.refill(at);
        assert_eq!(bucket.credits, 0);
        assert_eq!(bucket.anchor, at + Duration::from_secs(100_000));
        bucket.refill(at + Duration::from_secs(100_019));
        assert_eq!(bucket.credits, 0);
        bucket.refill(at + Duration::from_secs(100_020));
        assert_eq!(bucket.credits, 1);
    }
    #[test]
    fn recovery_bucket_arbitrary_intervals_preserve_capacity_plus_ceil_time_bound() {
        let at = Instant::now();
        let mut bucket = RecoveryBucket {
            credits: 3,
            anchor: at,
        };
        let mut admissions = Vec::new();
        // Production bucket, fine subsecond sampling, every admitted attempt is
        // a sustained failure. Includes bursts around fixed-grid boundaries.
        for ms in (0..=240_000u64).step_by(137) {
            if bucket.admit(at + Duration::from_millis(ms)) {
                admissions.push(ms);
            }
        }
        for start in (0..240_000u64).step_by(997) {
            for length in [0u64, 1, 19_999, 20_000, 20_001, 59_999, 120_001] {
                let count = admissions
                    .iter()
                    .filter(|&&time| time >= start && time <= start + length)
                    .count() as u64;
                assert!(
                    count <= 3 + length.div_ceil(20_000),
                    "start={start} length={length} count={count}"
                );
            }
        }
    }
    #[test]
    fn recovery_metadata_stability_is_sampled_and_debounced_with_four_slot_lru_only() {
        let at = Instant::now();
        let mut recovery = Recovery::default();
        let key = attempt_key();
        recovery.trigger(at, at);
        let a = [1; 8];
        assert!(!recovery.due(at + Duration::from_millis(4_999)));
        assert!(recovery.due(at + Duration::from_secs(5)));
        assert!(!recovery.observe(&key, Some(a), at));
        assert!(!recovery.observe(&key, Some(a), at + Duration::from_secs(5)));
        assert!(recovery.observe(&key, Some(a), at + Duration::from_secs(10)));
        assert_eq!(recovery.candidate.as_ref().unwrap().samples, 3);
        assert!(recovery.cache.back().unwrap().stable);
        assert_eq!(recovery.cache.len(), 1);
        assert_eq!(recovery.cache.back().unwrap().signature, a);
        assert_eq!(recovery.candidate.as_ref().unwrap().first_seen, at);
        assert_eq!(
            recovery.candidate.as_ref().unwrap().last_seen,
            at + Duration::from_secs(10)
        );
        recovery.after_metadata(at + Duration::from_secs(71));
        assert!(!recovery.due(at + Duration::from_secs(75))); // no missed-tick replay
        assert!(recovery.due(at + Duration::from_secs(76)));
        for n in 2..=12u64 {
            assert!(!recovery.observe(&key, Some([n; 8]), at + Duration::from_secs(n + 80)));
        }
        assert_eq!(recovery.cache.len(), 4);
        assert_eq!(
            recovery
                .cache
                .iter()
                .map(|slot| slot.signature)
                .collect::<Vec<_>>(),
            vec![[9; 8], [10; 8], [11; 8], [12; 8]]
        );
        assert!(!recovery.observe(&key, Some([10; 8]), at + Duration::from_secs(95)));
        assert_eq!(
            recovery
                .cache
                .iter()
                .map(|slot| slot.signature)
                .collect::<Vec<_>>(),
            vec![[9; 8], [11; 8], [12; 8], [10; 8]]
        );
        assert!(!recovery.observe(&key, Some(a), at + Duration::from_secs(100)));
        assert_eq!(recovery.cache.len(), 4); // A->B->A is another sampled candidate
        assert_eq!(
            recovery
                .cache
                .iter()
                .map(|slot| slot.signature)
                .collect::<Vec<_>>(),
            vec![[11; 8], [12; 8], [10; 8], a]
        );
        assert!(!recovery.observe(&key, None, at + Duration::from_secs(101)));
        assert!(recovery.candidate.is_none());
        assert!(recovery.cache.iter().all(|slot| !slot.stable));
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 2);
        assert_eq!(recovery.bucket.as_ref().unwrap().anchor, at);
    }
    #[test]
    fn recovery_same_geometry_retries_after_refill_without_key_or_epoch_recharge() {
        let at = Instant::now();
        let mut recovery = Recovery::default();
        let mut key = attempt_key();
        recovery.trigger(at, at);
        assert!(recovery.admit(at));
        assert!(recovery.admit(at));
        assert!(!recovery.admit(at));
        let anchor = recovery.bucket.as_ref().unwrap().anchor;
        for (second, eligible, credit) in [
            (5, false, false),
            (10, false, false),
            (15, true, false),
            (20, true, true),
            (25, true, false),
            (40, true, true),
        ] {
            assert_eq!(
                recovery.observe(&key, Some([4; 8]), at + Duration::from_secs(second)),
                eligible
            );
            if eligible {
                assert_eq!(recovery.admit(at + Duration::from_secs(second)), credit);
            }
        }
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 0);
        assert_eq!(
            recovery.bucket.as_ref().unwrap().anchor,
            anchor + Duration::from_secs(40)
        );
        // Fence/Resume/ownership moves clear sample/proof authority only.
        key.epoch = "2".into();
        recovery.clear_authority();
        assert!(!recovery.observe(&key, Some([4; 8]), at + Duration::from_secs(41)));
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 0);
        recovery.frame_published();
        assert!(!recovery.blocked);
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 0);
        recovery.trigger(at + Duration::from_secs(42), at + Duration::from_secs(42));
        assert!(recovery.normal_trigger_without_credit);
        assert_eq!(
            recovery.bucket.as_ref().unwrap().anchor,
            anchor + Duration::from_secs(40)
        );
        assert!(recovery.blocked); // no image never manufactures Normal/Live
    }
    #[test]
    fn recovery_invalid_epoch_clock_and_exhausted_generation_never_admit_or_recharge() {
        let at = Instant::now();
        let mut recovery = Recovery::default();
        let mut key = attempt_key();
        recovery.trigger(at, at);
        assert!(!recovery.observe(&key, Some([1; 8]), at + Duration::from_secs(10)));
        assert!(!recovery.observe(&key, Some([1; 8]), at)); // backward clock clears samples
        assert_eq!(recovery.candidate.as_ref().unwrap().samples, 1);
        key.authority += 1;
        assert!(!recovery.observe(&key, Some([1; 8]), at + Duration::from_secs(11)));
        assert_eq!(recovery.candidate.as_ref().unwrap().samples, 1);
        recovery.generation = u64::MAX;
        recovery.clear_authority();
        assert!(!recovery.observe(&key, Some([1; 8]), at + Duration::from_secs(30)));
        assert!(recovery.exhausted_generation);
        assert!(recovery.candidate.is_none());
        assert_eq!(recovery.bucket.as_ref().unwrap().credits, 2);
        let (state, _, _) = test_state();
        install_attempt(&state, &key);
        for change in ["pause", "release", "target"] {
            install_attempt(&state, &key);
            {
                let mut intents = state.intents.lock().unwrap();
                let i = intents.get_mut("source").unwrap();
                match change {
                    "pause" => i.paused = true,
                    "release" => i.released = true,
                    _ => i.target_identity = "other-target".into(),
                }
            }
            assert!(state.commit_key(&key, &key).is_err());
        }
        install_attempt(&state, &key);
        state.stop();
        assert!(state.commit_key(&key, &key).is_err());
    }
    fn recovery_geometry() -> macos::PreviewGeometrySnapshot {
        macos::PreviewGeometrySnapshot {
            cg_width_points: Some(1920.0),
            cg_height_points: Some(1080.0),
            cg_on_screen: Some(true),
            filter_width_points: Some(1920.0),
            filter_height_points: Some(1080.0),
            point_pixel_scale: Some(1.0),
            request_width: Some(960),
            request_height: Some(540),
            compatibility_source_cap: Some(false),
            reason: None,
            sc_window_frame_query: "not_queried".into(),
            sc_window_width_points: None,
            sc_window_height_points: None,
        }
    }
    #[test]
    fn recovery_fresh_geometry_race_skips_native_capture_and_requires_complete_same_attempt_post() {
        for changed in ["none", "epoch", "owner", "tcc"] {
            let geometry = recovery_geometry();
            let mut expected = geometry.signature().unwrap();
            expected[0] += 1;
            let native_calls = std::cell::Cell::new(0);
            let (attempt, cause) = capture_after_geometry(geometry, expected, || {
                native_calls.set(native_calls.get() + 1);
                panic!("fresh geometry race must not request SCK or compatibility pixels")
            });
            assert_eq!(native_calls.get(), 0);
            assert_eq!(cause, Some(WorkerRefusal::FreshGeometryChanged));
            assert_eq!(
                attempt.failure_kind,
                Some(macos::CaptureFailureKind::OtherExistingFailure)
            );
            assert_eq!(
                attempt.result.as_ref().err().unwrap().code,
                "preview_source_changed"
            );
            assert!(
                attempt
                    .diagnostic
                    .as_ref()
                    .unwrap()
                    .geometry
                    .as_ref()
                    .unwrap()
                    .reason
                    .is_none()
            );
            let stages = std::cell::RefCell::new(Vec::new());
            let checks = std::cell::Cell::new(0);
            let post =
                complete_capture_post(&attempt.result, attempt.failure_kind, true, true, || {
                    complete_post_proof(
                        &attempt_key(),
                        || {
                            checks.set(checks.get() + 1);
                            stages.borrow_mut().push("current-tcc");
                            match (changed, checks.get()) {
                                ("epoch", 2) => Err(failure("preview_cancelled")),
                                ("tcc", 3) => Err(failure("preview_permission_revoked")),
                                _ => Ok(()),
                            }
                        },
                        || {
                            stages.borrow_mut().push("windows-file-unique-instance");
                            Ok(())
                        },
                        || {
                            stages.borrow_mut().push("original-choice");
                            if changed == "owner" {
                                Err(failure("preview_selection_mismatch"))
                            } else {
                                Ok(())
                            }
                        },
                        |_| {
                            stages.borrow_mut().push("commit");
                            Ok(())
                        },
                    )
                });
            let key = attempt_key();
            let (state, _, _) = test_state();
            install_attempt(&state, &key);
            let (code, retention, disposition) = match post {
                Ok(Some(proof)) => {
                    assert_eq!(cause, Some(WorkerRefusal::FreshGeometryChanged));
                    let decision = proof
                        .into_retention(&key, RetentionCause::FreshGeometryChanged)
                        .unwrap();
                    let permit = consume_retention(decision, &key, &state, || true).unwrap();
                    assert_eq!(changed, "none");
                    assert_eq!(
                        *stages.borrow(),
                        [
                            "current-tcc",
                            "windows-file-unique-instance",
                            "current-tcc",
                            "original-choice",
                            "current-tcc",
                            "commit"
                        ]
                    );
                    ("preview_source_changed".to_string(), Some(permit), None)
                }
                Err(failed) => {
                    assert_ne!(changed, "none");
                    (failed.error.code, None, Some(failed.disposition))
                }
                _ => panic!("recovery always post-proves a geometry race"),
            };
            let mut choice = Some(1u8);
            let (mut terminal, mut new_capture) = (false, true);
            if disposition != Some(PostDisposition::Superseded) {
                apply_attempt_failure(
                    &mut choice,
                    &mut terminal,
                    &mut new_capture,
                    &code,
                    retention,
                    disposition,
                    true,
                );
                assert_eq!(choice.is_some(), changed == "none");
            } else {
                assert_eq!(code, "preview_cancelled");
            }
        }
        // The same public error and generic kind cannot manufacture this cause.
        let geometry = recovery_geometry();
        let expected = geometry.signature().unwrap();
        let (attempt, cause) =
            capture_after_geometry(geometry, expected, || macos::PreviewCaptureAttempt {
                result: Err(failure("preview_source_changed")),
                diagnostic: None,
                failure_kind: Some(macos::CaptureFailureKind::OtherExistingFailure),
            });
        assert!(cause.is_none());
        assert_eq!(
            attempt.failure_kind,
            Some(macos::CaptureFailureKind::OtherExistingFailure)
        );
        let mut choice = Some(1u8);
        let (mut terminal, mut new_capture) = (false, true);
        apply_attempt_failure(
            &mut choice,
            &mut terminal,
            &mut new_capture,
            "preview_source_changed",
            None,
            Some(PostDisposition::Unproved),
            true,
        );
        assert!(choice.is_none());
        assert!(terminal);
        assert!(!new_capture);
    }
    #[test]
    fn recovery_forged_diagnostic_retain_word_cannot_replace_a_typed_consumed_proof() {
        for code in ["preview_budget_exceeded", "preview_geometry_unavailable"] {
            let mut diagnostic = macos::PreviewDiagnostic::worker("capture");
            diagnostic.post_proof = Some("post_verified_retain".into());
            diagnostic.failure_kind = Some(macos::CaptureFailureKind::CompatibilitySourceCap);
            diagnostic.final_code = Some(code.into());
            assert_eq!(
                diagnostic.post_proof.as_deref(),
                Some("post_verified_retain")
            );
            let drops = Arc::new(AtomicUsize::new(0));
            let mut choice = Some(Resource(drops.clone()));
            let (mut terminal, mut new_capture) = (false, true);
            // This is the production error action. Its authority input accepts
            // only a consumed RetentionPermit; diagnostics have no input slot.
            apply_attempt_failure(
                &mut choice,
                &mut terminal,
                &mut new_capture,
                diagnostic.final_code.as_deref().unwrap(),
                None,
                None,
                false,
            );
            assert!(choice.is_none());
            assert!(terminal);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        }
        let key = attempt_key();
        let (state, _, _) = test_state();
        install_attempt(&state, &key);
        let captured: AppResult<()> = Err(failure("preview_budget_exceeded"));
        let proof = complete_capture_post(
            &captured,
            Some(macos::CaptureFailureKind::CompatibilitySourceCap),
            true,
            false,
            || {
                complete_post_proof(
                    &key,
                    || state.commit_key(&key, &key),
                    || Ok(()),
                    || Ok(()),
                    |key| state.commit_key(key, key),
                )
            },
        )
        .unwrap()
        .unwrap();
        let typed = proof
            .into_retention(&key, RetentionCause::CompatibilitySourceCap)
            .unwrap();
        let permit = consume_retention(typed, &key, &state, || true).unwrap();
        let mut choice = Some(1u8);
        let (mut terminal, mut new_capture) = (true, true);
        apply_attempt_failure(
            &mut choice,
            &mut terminal,
            &mut new_capture,
            "preview_budget_exceeded",
            Some(permit),
            None,
            false,
        );
        assert_eq!(choice, Some(1));
        assert!(!terminal);
        assert!(new_capture);
    }
    #[test]
    fn recovery_typed_retention_rechecks_exact_epoch_choice_tcc_and_final_fence_before_use() {
        for changed in [
            "epoch",
            "authority",
            "capture",
            "token",
            "owner",
            "tcc",
            "fence-during-tcc",
        ] {
            let key = attempt_key();
            let (state, _, _) = test_state();
            install_attempt(&state, &key);
            let typed = completed_retention(&state, &key, RetentionCause::CompatibilitySourceCap);
            let mut actual = key.clone();
            match changed {
                "epoch" => {
                    actual.epoch = "2".into();
                    actual.frame_epoch = "frame-2".into();
                    let mut intents = state.intents.lock().unwrap();
                    let i = intents.get_mut("source").unwrap();
                    i.epoch = actual.epoch.clone();
                    i.frame_epoch = actual.frame_epoch.clone();
                }
                "authority" => actual.authority += 1,
                "capture" => actual.capture.push('x'),
                "token" => actual.choice.selected_token.as_mut().unwrap().push('x'),
                "owner" => actual.choice.host_start.push('x'),
                _ => (),
            }
            let permission_calls = std::cell::Cell::new(0);
            let result = consume_retention(typed, &actual, &state, || {
                permission_calls.set(permission_calls.get() + 1);
                if changed == "fence-during-tcc" {
                    state
                        .intents
                        .lock()
                        .unwrap()
                        .get_mut("source")
                        .unwrap()
                        .epoch = "winner".into();
                    false
                } else {
                    changed != "tcc"
                }
            });
            assert_eq!(
                result.unwrap_err().code,
                if changed == "tcc" {
                    "preview_permission_revoked"
                } else {
                    "preview_cancelled"
                }
            );
            if matches!(changed, "tcc" | "fence-during-tcc") {
                assert_eq!(permission_calls.get(), 1);
            } else {
                assert_eq!(permission_calls.get(), 0);
            }
            if changed == "epoch" {
                assert_eq!(state.intents.lock().unwrap()["source"].epoch, "2");
            }
            if changed == "fence-during-tcc" {
                assert_eq!(state.intents.lock().unwrap()["source"].epoch, "winner");
            }
        }
    }
}
