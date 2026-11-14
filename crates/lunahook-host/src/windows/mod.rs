//! One Windows same-bitness host per process. Native lifetimes end only at process exit.

mod callbacks;
mod identity;
mod native;

use crate::{
    framing::{self, Frame},
    queue::{self, EventSender},
    runtime::VerifiedRuntime,
};
use hook_contracts::{
    Architecture, Command, DecimalU64, Envelope, ErrorCode, Event, Request, RequestId,
    ResponseResult, TargetStage, TargetStatus,
};
use native::NativeHost;
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::{self, BufReader, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

const CALLBACK_QUEUE_CAPACITY: usize = 256;
const OUTPUT_QUEUE_CAPACITY: usize = 256;
const CONNECTION_TIMEOUT_MS: u32 = 15_000;
const SHUTDOWN_DEADLINE_MS: u32 = 2_000;
const MAX_TRACKED_TARGETS: usize = callbacks::MAX_PROCESS_SLOTS;
const MAX_SESSION_REQUESTS: usize = 65_536;

enum Input {
    Request(Result<Request, String>),
    End,
}

enum NativeCommand {
    Attach(RequestId, hook_contracts::AttachParams, Instant),
    Detach(u32),
}

enum WorkerFinished {
    Attach(AttachFinished),
    Detach(u32),
}

struct AttachFinished {
    id: RequestId,
    pid: u32,
    outcome: Result<Option<(Architecture, Option<i32>)>, identity::TargetError>,
}

struct Target {
    stage: TargetStage,
    deadline: Instant,
    request_id: RequestId,
}

pub fn run(directory: &Path) -> u8 {
    let protocol = match native::capture_protocol_stdout() {
        Ok(file) => file,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    let (output, output_rx, output_dropped) = queue::bounded(OUTPUT_QUEUE_CAPACITY);
    let (callbacks, callback_rx, callback_dropped) = queue::bounded(CALLBACK_QUEUE_CAPACITY);
    let writer_done = Arc::new(AtomicBool::new(false));
    let writer_failed = Arc::new(AtomicBool::new(false));
    start_writer(
        protocol,
        output_rx,
        Arc::clone(&callback_dropped),
        Arc::clone(&output_dropped),
        Arc::clone(&writer_done),
        Arc::clone(&writer_failed),
    );
    let runtime = match VerifiedRuntime::verify(directory) {
        Ok(runtime) => runtime,
        Err(message) => {
            error(&output, None, ErrorCode::RuntimeInvalid, message);
            finish_output(output, &writer_done);
            return 2;
        }
    };
    let process_states = Arc::new(callbacks::ProcessStates::new());
    let host = match NativeHost::start(runtime, callbacks, Arc::clone(&process_states)) {
        Ok(host) => host,
        Err(message) => {
            error(&output, None, ErrorCode::HostInitialization, message);
            finish_output(output, &writer_done);
            return 2;
        }
    };
    output.try_send(Event::Ready {
        host_arch: crate::runtime::host_architecture(),
        abi_version: hook_contracts::CORE_ABI_VERSION,
        upstream_commit: host.runtime.manifest.commit.clone(),
        abi_profile: host.runtime.manifest.abi_profile.clone(),
    });
    let (input_tx, input_rx) = mpsc::sync_channel(32);
    start_input(input_tx);
    let (native_tx, native_rx) = mpsc::sync_channel(32);
    let (finished_tx, finished_rx) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    start_native_worker(host, native_rx, finished_tx, Arc::clone(&cancelled));
    let mut supervisor = Supervisor {
        output: &output,
        native_tx,
        cancelled,
        targets: BTreeMap::new(),
        attaching: HashSet::new(),
        request_ids: HashSet::new(),
        shutdown: None,
        callback_dropped: &callback_dropped,
        output_dropped: &output_dropped,
        process_states,
        shutdown_detaches: HashSet::new(),
    };
    let mut code = 0;
    loop {
        if writer_failed.load(Ordering::Acquire) {
            supervisor.begin_shutdown(None);
            code = 2;
        }
        // Bound each drain so callback floods cannot starve ping or shutdown.
        for _ in 0..128 {
            match callback_rx.try_recv() {
                Ok(event) => supervisor.callback(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        for _ in 0..32 {
            match finished_rx.try_recv() {
                Ok(WorkerFinished::Attach(finished)) => supervisor.finished(finished),
                Ok(WorkerFinished::Detach(pid)) => {
                    supervisor.shutdown_detaches.remove(&pid);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        supervisor.reconcile_process_states();
        for _ in 0..32 {
            match input_rx.try_recv() {
                Ok(Input::Request(Ok(request))) => supervisor.command(request),
                Ok(Input::Request(Err(message))) => {
                    error(&output, None, ErrorCode::InvalidRequest, message)
                }
                Ok(Input::End) => supervisor.begin_shutdown(None),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    supervisor.begin_shutdown(None);
                    break;
                }
            }
        }
        supervisor.timeouts();
        if let Some(deadline) = supervisor.shutdown {
            let connected = supervisor
                .targets
                .values()
                .any(|target| target.stage == TargetStage::Connected);
            if !connected
                && supervisor.attaching.is_empty()
                && supervisor.shutdown_detaches.is_empty()
            {
                break;
            }
            if Instant::now() >= deadline {
                error(&output, None, ErrorCode::ShutdownTimeout, "shutdown deadline reached; helper process will exit without unloading Host DLL; pending native pipe listeners are not joined".into());
                code = 2;
                break;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    // Native workers/callbacks are deliberately never joined or unloaded. The
    // process boundary is the cancellation boundary for upstream unmanaged work.
    drop(supervisor);
    finish_output(output, &writer_done);
    code
}

struct Supervisor<'a> {
    output: &'a EventSender<Event>,
    native_tx: SyncSender<NativeCommand>,
    cancelled: native::CancelFlag,
    targets: BTreeMap<u32, Target>,
    attaching: HashSet<u32>,
    request_ids: HashSet<String>,
    shutdown: Option<Instant>,
    callback_dropped: &'a AtomicU64,
    output_dropped: &'a AtomicU64,
    process_states: Arc<callbacks::ProcessStates>,
    shutdown_detaches: HashSet<u32>,
}

impl Supervisor<'_> {
    fn command(&mut self, request: Request) {
        if self.request_ids.contains(request.id.as_str()) {
            error(
                self.output,
                Some(request.id),
                ErrorCode::DuplicateRequest,
                "request id was already used by this host session".into(),
            );
            return;
        }
        if self.request_ids.len() >= MAX_SESSION_REQUESTS
            && !matches!(&request.command, Command::Shutdown(_))
        {
            error(
                self.output,
                Some(request.id),
                ErrorCode::Busy,
                "session request-id budget exhausted; issue shutdown with a fresh id, then start a new host process".into(),
            );
            return;
        }
        self.request_ids.insert(request.id.as_str().to_owned());
        if self.shutdown.is_some() {
            error(
                self.output,
                Some(request.id),
                ErrorCode::ShuttingDown,
                "host is shutting down".into(),
            );
            return;
        }
        match request.command {
            Command::Ping(_) => self.respond(request.id, ResponseResult::Pong),
            Command::Status(_) => self.respond(
                request.id,
                ResponseResult::Snapshot {
                    targets: self
                        .targets
                        .iter()
                        .map(|(&pid, target)| TargetStatus {
                            pid,
                            stage: target.stage.clone(),
                        })
                        .collect(),
                    integrity_lost: self.callback_dropped.load(Ordering::Relaxed) != 0
                        || self.output_dropped.load(Ordering::Relaxed) != 0,
                },
            ),
            Command::Attach(params) => {
                if !self.attaching.is_empty() {
                    error(self.output, Some(request.id), ErrorCode::Busy, "one attach may be in flight; wait for its injector completion before starting another".into());
                    return;
                }
                if self.targets.contains_key(&params.pid) {
                    error(self.output, Some(request.id), ErrorCode::AlreadyAttached, "PID was already attempted by this host; start a fresh host process for every reconnect or PID reuse".into());
                    return;
                }
                if self.targets.len() >= MAX_TRACKED_TARGETS
                    && !self.targets.contains_key(&params.pid)
                {
                    error(
                        self.output,
                        Some(request.id),
                        ErrorCode::Busy,
                        "host target limit reached; use a fresh host process".into(),
                    );
                    return;
                }
                let pid = params.pid;
                if !self.process_states.register(pid) {
                    error(
                        self.output,
                        Some(request.id),
                        ErrorCode::Busy,
                        "native process-state slots exhausted".into(),
                    );
                    return;
                }
                let deadline =
                    Instant::now() + Duration::from_millis(u64::from(CONNECTION_TIMEOUT_MS));
                if self
                    .native_tx
                    .try_send(NativeCommand::Attach(request.id.clone(), params, deadline))
                    .is_err()
                {
                    self.process_states.forget(pid);
                    error(
                        self.output,
                        Some(request.id),
                        ErrorCode::Busy,
                        "native command queue full or unavailable".into(),
                    );
                    return;
                }
                self.targets.insert(
                    pid,
                    Target {
                        stage: TargetStage::Pending,
                        deadline,
                        request_id: request.id.clone(),
                    },
                );
                self.attaching.insert(pid);
                self.respond(
                    request.id,
                    ResponseResult::Pending {
                        pid,
                        connection_timeout_ms: CONNECTION_TIMEOUT_MS,
                    },
                );
            }
            Command::Detach(params) => {
                if !self
                    .targets
                    .get(&params.pid)
                    .is_some_and(|target| target.stage == TargetStage::Connected)
                {
                    error(self.output, Some(request.id), ErrorCode::NotAttached, "detach requires a callback-confirmed connection; pending pipe listeners are cancelled by exiting the host".into());
                } else if self
                    .native_tx
                    .try_send(NativeCommand::Detach(params.pid))
                    .is_err()
                {
                    error(
                        self.output,
                        Some(request.id),
                        ErrorCode::Busy,
                        "native command queue full or unavailable".into(),
                    );
                } else {
                    self.respond(
                        request.id,
                        ResponseResult::DetachRequested { pid: params.pid },
                    );
                }
            }
            Command::Shutdown(_) => self.begin_shutdown(Some(request.id)),
        }
    }

    fn callback(&mut self, event: Event) {
        self.output.try_send(event);
    }

    fn reconcile_process_states(&mut self) {
        for (&pid, target) in &mut self.targets {
            match self.process_states.get(pid) {
                1 => {
                    let just_connected = target.stage != TargetStage::Connected;
                    target.stage = TargetStage::Connected;
                    if just_connected
                        && self.shutdown.is_some()
                        && self.native_tx.try_send(NativeCommand::Detach(pid)).is_ok()
                    {
                        self.shutdown_detaches.insert(pid);
                    }
                }
                2 => target.stage = TargetStage::Disconnected,
                _ => {}
            }
        }
    }

    fn finished(&mut self, finished: AttachFinished) {
        self.attaching.remove(&finished.pid);
        match finished.outcome {
            Ok(Some((architecture, exit_code))) => {
                let accepted_by_helper = crate::runtime::injector_load_confirmed(exit_code);
                self.output.try_send(Event::InjectionFinished {
                    pid: finished.pid,
                    architecture,
                    exit_code,
                    accepted_by_helper,
                });
                if !accepted_by_helper {
                    self.fail_pending(finished.pid);
                    error(
                        self.output,
                        Some(finished.id),
                        ErrorCode::InjectionFailed,
                        format!(
                            "Core injector returned {exit_code:?}; expected 0; only process_connected/text callbacks establish native success"
                        ),
                    );
                }
            }
            Ok(None) => {
                self.output.try_send(Event::Log { kind: 0, message: format!("pid {} already has a Hook mutex; waiting for connection callback without running injector", finished.pid) });
            }
            Err((code, message)) => {
                self.fail_pending(finished.pid);
                error(self.output, Some(finished.id), code, message);
            }
        }
    }

    fn fail_pending(&mut self, pid: u32) {
        if let Some(target) = self.targets.get_mut(&pid) {
            // A callback-confirmed connection remains authoritative even if the
            // helper reports an error after the connection has already occurred.
            if target.stage == TargetStage::Pending {
                target.stage = TargetStage::Failed;
            }
        }
    }

    fn begin_shutdown(&mut self, id: Option<RequestId>) {
        if self.shutdown.is_some() {
            return;
        }
        self.cancelled.store(true, Ordering::Release);
        self.shutdown =
            Some(Instant::now() + Duration::from_millis(u64::from(SHUTDOWN_DEADLINE_MS)));
        for &pid in self.targets.keys() {
            // Also cover a connected callback whose public event was dropped.
            // For unconnected targets native DetachProcess returns immediately;
            // this does not cancel or join the upstream pending pipe listener.
            if self.native_tx.try_send(NativeCommand::Detach(pid)).is_ok() {
                self.shutdown_detaches.insert(pid);
            }
        }
        if let Some(id) = id {
            self.respond(
                id,
                ResponseResult::ShuttingDown {
                    deadline_ms: SHUTDOWN_DEADLINE_MS,
                },
            );
        }
    }

    fn timeouts(&mut self) {
        for (&pid, target) in &mut self.targets {
            if target.stage == TargetStage::Pending && Instant::now() >= target.deadline {
                target.stage = TargetStage::TimedOut;
                error(
                    self.output,
                    Some(target.request_id.clone()),
                    ErrorCode::ConnectionTimeout,
                    format!(
                        "pid {pid}: no process_connected callback within {CONNECTION_TIMEOUT_MS}ms; native listener remains pending until host exit"
                    ),
                );
            }
        }
    }

    fn respond(&self, id: RequestId, result: ResponseResult) {
        self.output.try_send(Event::Response { id, result });
    }
}

fn error(output: &EventSender<Event>, id: Option<RequestId>, code: ErrorCode, message: String) {
    output.try_send(Event::Error { id, code, message });
}

fn start_native_worker(
    host: NativeHost,
    commands: Receiver<NativeCommand>,
    finished: mpsc::Sender<WorkerFinished>,
    cancelled: native::CancelFlag,
) {
    thread::spawn(move || {
        while let Ok(command) = commands.recv() {
            match command {
                NativeCommand::Attach(id, params, deadline) => {
                    let pid = params.pid;
                    let outcome = host.attach(&params, &cancelled, deadline);
                    let _ =
                        finished.send(WorkerFinished::Attach(AttachFinished { id, pid, outcome }));
                }
                NativeCommand::Detach(pid) => {
                    host.detach(pid);
                    let _ = finished.send(WorkerFinished::Detach(pid));
                }
            }
        }
    });
}

fn start_input(input: SyncSender<Input>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(io::stdin().lock());
        loop {
            let frame = match framing::read_frame(&mut reader) {
                Ok(Some(Frame::Json(bytes))) => {
                    serde_json::from_slice::<Request>(&bytes).map_err(|e| e.to_string())
                }
                Ok(Some(Frame::TooLong)) => Err("JSONL request exceeds 65536-byte limit".into()),
                Ok(None) => {
                    let _ = input.send(Input::End);
                    break;
                }
                Err(e) => {
                    let _ = input.send(Input::Request(Err(format!("stdin: {e}"))));
                    let _ = input.send(Input::End);
                    break;
                }
            };
            if input.send(Input::Request(frame)).is_err() {
                break;
            }
        }
    });
}

fn start_writer(
    mut protocol: File,
    events: Receiver<Event>,
    callback_dropped: Arc<AtomicU64>,
    output_dropped: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut last_callback = 0;
        let mut last_output = 0;
        let result = (|| -> io::Result<()> {
            loop {
                let item = events.recv_timeout(Duration::from_millis(100));
                for (source, counter, last) in [
                    ("callback_ingress", &callback_dropped, &mut last_callback),
                    ("output_queue", &output_dropped, &mut last_output),
                ] {
                    let total = counter.load(Ordering::Relaxed);
                    if total > *last {
                        write_event(
                            &mut protocol,
                            Event::Overflow {
                                source: source.into(),
                                dropped_since_last: DecimalU64(total - *last),
                                dropped_total: DecimalU64(total),
                                integrity_lost: true,
                            },
                        )?;
                        *last = total;
                    }
                }
                match item {
                    Ok(event) => write_event(&mut protocol, event)?,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            protocol.flush()
        })();
        if let Err(error) = result {
            eprintln!("protocol stdout: {error}");
            failed.store(true, Ordering::Release);
        }
        done.store(true, Ordering::Release);
    });
}

fn write_event(writer: &mut impl Write, event: Event) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &Envelope::new(event))?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn finish_output(output: EventSender<Event>, done: &AtomicBool) {
    drop(output);
    let deadline = Instant::now() + Duration::from_millis(300);
    while !done.load(Ordering::Acquire) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    if !done.load(Ordering::Acquire) {
        eprintln!("protocol drain deadline reached; stdout consumer may be blocked");
    }
}
