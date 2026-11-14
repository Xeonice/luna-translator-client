//! Native desktop client for the existing headless LunaHook host.
//!
//! This module requests injection through the existing native host API; it never
//! scans game memory, translates, or terminates a game. A session owns only its
//! host launcher and exchanges the versioned JSONL API.

pub use hook_contracts::{Event as NativeEvent, StreamId as NativeStreamId};

mod window_binding;
pub use window_binding::{
    BoundWindowActivation, BoundWindowStatus, BoundWindowVerification, WindowBinding,
    WindowBindingSnapshot, WindowConnectionResult, WindowPickResult, WindowRect, WindowTarget,
};

use hook_contracts::{
    AttachParams, Command as HostCommand, EmptyParams, Envelope, Event, PROTOCOL_VERSION, Request,
    ResponseResult, StreamId,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, TrySendError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const HISTORY_LIMIT: usize = 100;
const LOG_LIMIT: usize = 100;
const MAX_EVENT_BYTES: usize = 1_048_576;
const START_TIMEOUT: Duration = Duration::from_secs(25);
const ATTACH_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(7);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LaunchSettings {
    pub wine_path: String,
    pub bottle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    pub pid: u32,
    pub exe: String,
    pub full_image_path: Option<String>,
    pub creation_filetime: Option<String>,
    pub architecture: Option<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLine {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSnapshot {
    /// Full stream identity. Hook names alone are never used to merge streams.
    pub key: String,
    pub id: StreamId,
    pub hook_name: String,
    pub hook_code: String,
    pub active: bool,
    pub latest_text: Option<String>,
    pub history: Vec<TextLine>,
    pub text_events: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSnapshot {
    pub sequence: u64,
    pub session_id: Option<String>,
    pub status: String,
    /// Includes the reserved startup phase. Cleared only after the owned
    /// launcher was observed exited, or startup failed before spawning one.
    pub host_running: bool,
    pub target: Option<Target>,
    pub settings: Option<LaunchSettings>,
    pub streams: Vec<StreamSnapshot>,
    pub session_log_dir: Option<String>,
    pub errors: Vec<String>,
    pub logs: Vec<String>,
    pub integrity_lost: bool,
    pub text_events: u64,
    pub last_text_at_ms: Option<u64>,
    pub shutdown_acknowledged: bool,
    pub exit_code: Option<i32>,
}

impl Default for ClientSnapshot {
    fn default() -> Self {
        Self {
            sequence: 0,
            session_id: None,
            status: "idle".into(),
            host_running: false,
            target: None,
            settings: None,
            streams: Vec::new(),
            session_log_dir: None,
            errors: Vec::new(),
            logs: Vec::new(),
            integrity_lost: false,
            text_events: 0,
            last_text_at_ms: None,
            shutdown_acknowledged: false,
            exit_code: None,
        }
    }
}

/// A revisioned native event projection; full history is obtained once through
/// `snapshot`, rather than copied on a periodic whole-application poll.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEvent {
    pub session_id: Option<String>,
    pub sequence: u64,
    /// Time received by this client, not a fabricated timestamp from the game.
    pub received_at_ms: u64,
    pub status: String,
    pub target: Option<Target>,
    pub integrity_lost: bool,
    pub payload: Option<Event>,
}

struct Subscriber {
    sender: SyncSender<ClientEvent>,
    dropped: Arc<AtomicU64>,
    lost_session: Arc<Mutex<Option<String>>>,
}

pub struct ClientSubscription {
    receiver: Receiver<ClientEvent>,
    dropped: Arc<AtomicU64>,
    lost_session: Arc<Mutex<Option<String>>>,
    pending_received: Mutex<Option<ClientEvent>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    Timeout,
    Closed,
    /// Sticky: a consumer must resynchronize and must not treat later text as
    /// a complete source sequence. Loss never depends on an available queue slot.
    IntegrityLost {
        dropped_total: u64,
    },
}

impl ClientSubscription {
    pub fn lost_session_id(&self) -> Option<String> {
        self.lost_session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Result<ClientEvent, SubscriptionError> {
        let check_loss = || {
            let dropped_total = self.dropped.load(Ordering::Acquire);
            if dropped_total == 0 {
                Ok(())
            } else {
                Err(SubscriptionError::IntegrityLost { dropped_total })
            }
        };
        check_loss()?;
        let event = self
            .receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => SubscriptionError::Timeout,
                RecvTimeoutError::Disconnected => SubscriptionError::Closed,
            });
        if let Err(loss) = check_loss() {
            if let Ok(event) = event {
                *self
                    .pending_received
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(event);
            }
            return Err(loss);
        }
        event
    }
    /// Drain facts already accepted by this bounded subscription, without
    /// clearing its sticky loss. Caller first stops producers or replaces the
    /// poisoned subscription atomically before treating this as a final drain.
    pub fn drain_received(&self) -> Vec<ClientEvent> {
        let mut accepted = Vec::new();
        if let Some(event) = self
            .pending_received
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            accepted.push(event);
        }
        while let Ok(event) = self.receiver.try_recv() {
            accepted.push(event);
        }
        accepted
    }
    pub fn dropped_total(&self) -> u64 {
        self.dropped.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub struct Client(Arc<Inner>);

struct Inner {
    resource_dir: PathBuf,
    log_root: PathBuf,
    state: Mutex<ClientSnapshot>,
    session: Mutex<Option<SessionHandle>>,
    counter: AtomicU64,
    disconnect_epoch: AtomicU64,
    connection_operation: AtomicBool,
    window_binding: Mutex<window_binding::BindingState>,
    picking_window: AtomicBool,
    subscribers: Mutex<Vec<Subscriber>>,
    event_producers: Arc<AtomicUsize>,
}
struct EventProducer(Arc<AtomicUsize>);
impl Drop for EventProducer {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct SessionHandle {
    id: String,
    control: Sender<Control>,
    shutdown_requested: Arc<AtomicBool>,
    settings: LaunchSettings,
    target: Target,
}

#[derive(Debug, PartialEq, Eq)]
enum SessionRelation {
    None,
    Reusable,
    Different,
}

#[derive(Debug, PartialEq, Eq)]
enum ConnectOutcome {
    Started,
    Reused,
}

/// A reservation serializes user-initiated connection preparations without
/// holding any mutex while probing or waiting for the owned host to exit.
struct ConnectionOperation<'a> {
    client: &'a Client,
    epoch: u64,
}

impl ConnectionOperation<'_> {
    fn check(&self) -> Result<(), String> {
        if self.client.0.disconnect_epoch.load(Ordering::Acquire) == self.epoch {
            Ok(())
        } else {
            Err("连接准备期间收到断开或解除请求，本次操作已取消".into())
        }
    }
}

impl Drop for ConnectionOperation<'_> {
    fn drop(&mut self) {
        self.client
            .0
            .connection_operation
            .store(false, Ordering::Release);
    }
}

enum Control {
    Ready,
    Connected,
    Shutdown,
    Fault(String),
    OutputEnded,
}

/// Rust Child intentionally does not terminate a process on drop. This guard
/// keeps unexpected I/O errors from abandoning the owned launcher.
struct OwnedLauncher {
    child: Child,
    running: Arc<AtomicBool>,
}

impl std::ops::Deref for OwnedLauncher {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}

impl std::ops::DerefMut for OwnedLauncher {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

impl Drop for OwnedLauncher {
    fn drop(&mut self) {
        let stopped =
            matches!(self.child.try_wait(), Ok(Some(_))) || terminate_owned(&mut self.child);
        self.running.store(!stopped, Ordering::Release);
    }
}

impl Client {
    pub fn new(resource_dir: PathBuf, log_root: PathBuf) -> Self {
        Self(Arc::new(Inner {
            resource_dir,
            log_root,
            state: Mutex::new(ClientSnapshot::default()),
            session: Mutex::new(None),
            counter: AtomicU64::new(0),
            disconnect_epoch: AtomicU64::new(0),
            connection_operation: AtomicBool::new(false),
            window_binding: Mutex::new(window_binding::BindingState::default()),
            picking_window: AtomicBool::new(false),
            subscribers: Mutex::new(Vec::new()),
            event_producers: Arc::new(AtomicUsize::new(0)),
        }))
    }

    pub fn snapshot(&self) -> ClientSnapshot {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Subscribe to actual state changes and native events. This is an adapter
    /// boundary for the Rust runtime; UI code should consume its typed projection.
    pub fn subscribe(&self, capacity: usize) -> Result<ClientSubscription, String> {
        if !(1..=4096).contains(&capacity) {
            return Err("event subscription capacity must be 1..=4096".into());
        }
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let dropped = Arc::new(AtomicU64::new(0));
        let lost_session = Arc::new(Mutex::new(None));
        let mut subscribers = self.0.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subscribers.retain(|subscriber| subscriber.dropped.load(Ordering::Acquire) == 0);
        if subscribers.len() >= 16 {
            return Err("too many native event subscribers".into());
        }
        subscribers.push(Subscriber {
            sender,
            dropped: Arc::clone(&dropped),
            lost_session: Arc::clone(&lost_session),
        });
        Ok(ClientSubscription {
            receiver,
            dropped,
            lost_session,
            pending_received: Mutex::new(None),
        })
    }
    fn event_producer(&self) -> EventProducer {
        self.0.event_producers.fetch_add(1, Ordering::AcqRel);
        EventProducer(self.0.event_producers.clone())
    }
    /// A reservation ending is insufficient: timed-out output readers can
    /// still publish. Require every registered publishing thread to finish.
    pub fn wait_event_producers_stopped(&self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        while self.has_active_session() || self.0.event_producers.load(Ordering::Acquire) > 0 {
            if Instant::now() >= deadline {
                return Err("native event producers have not all stopped".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    fn publish(&self, state: &ClientSnapshot, payload: Option<Event>) {
        let event = ClientEvent {
            session_id: state.session_id.clone(),
            sequence: state.sequence,
            received_at_ms: timestamp_ms(),
            status: state.status.clone(),
            target: state.target.clone(),
            integrity_lost: state.integrity_lost,
            payload,
        };
        self.0
            .subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(
                |subscriber| match subscriber.sender.try_send(event.clone()) {
                    Ok(()) => true,
                    Err(TrySendError::Full(_)) => {
                        if subscriber.dropped.load(Ordering::Acquire) == 0 {
                            *subscriber
                                .lost_session
                                .lock()
                                .unwrap_or_else(|e| e.into_inner()) = event.session_id.clone();
                        }
                        subscriber.dropped.fetch_add(1, Ordering::AcqRel);
                        true
                    }
                    Err(TrySendError::Disconnected(_)) => false,
                },
            );
    }

    /// Includes startup, owned-process shutdown, output draining, and the final
    /// snapshot write. Desktop quit should wait for this, not callback status.
    pub fn has_active_session(&self) -> bool {
        self.0
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Read-only discovery, inside the selected Wine bottle when configured.
    /// Unknown games remain selectable when the probe can verify their identity.
    pub fn discover_targets(&self, settings: &LaunchSettings) -> Result<Vec<Target>, String> {
        let (targets, _) = self.probe(settings, &["--all"])?;
        Ok(targets.into_iter().filter(is_discoverable_target).collect())
    }

    /// Raw read-only process inventory for launch duplicate prevention. Keep
    /// incomplete identities and diagnostics: non-injectable does not mean absent.
    pub fn discover_launch_targets(
        &self,
        settings: &LaunchSettings,
    ) -> Result<Vec<Target>, String> {
        self.probe(settings, &["--all"]).map(|(targets, _)| targets)
    }

    /// Re-query identity before starting a new host. The host independently
    /// verifies the same identity before injecting its original native runtime.
    pub fn connect(&self, settings: LaunchSettings, target: Target) -> Result<(), String> {
        let operation = self.begin_connection_operation()?;
        self.connect_reserved(settings, target, &operation, None)
            .map(|_| ())
    }

    fn begin_connection_operation(&self) -> Result<ConnectionOperation<'_>, String> {
        // Capture before reserving: a cancellation between these operations
        // must invalidate this preparation, not become its new baseline.
        let epoch = self.0.disconnect_epoch.load(Ordering::Acquire);
        if self.0.connection_operation.swap(true, Ordering::AcqRel) {
            return Err("已有窗口关联或连接操作正在进行，请等待完成后重试".into());
        }
        Ok(ConnectionOperation {
            client: self,
            epoch,
        })
    }

    fn session_relation(&self, settings: &LaunchSettings, target: &Target) -> SessionRelation {
        let session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
        let Some(session) = session.as_ref() else {
            return SessionRelation::None;
        };
        if !session.shutdown_requested.load(Ordering::Acquire)
            && session.settings == *settings
            && match_identity(target, vec![session.target.clone()]).is_ok()
            && matches!(
                self.snapshot().status.as_str(),
                "starting" | "ready" | "attaching" | "connected"
            )
        {
            SessionRelation::Reusable
        } else {
            SessionRelation::Different
        }
    }

    fn connect_reserved(
        &self,
        settings: LaunchSettings,
        target: Target,
        operation: &ConnectionOperation<'_>,
        required_binding_revision: Option<u64>,
    ) -> Result<ConnectOutcome, String> {
        operation.check()?;
        if let Some(revision) = required_binding_revision {
            self.check_binding_revision(revision)?;
        }
        let attach = attach_params(&target)?;
        match self.session_relation(&settings, &target) {
            SessionRelation::Reusable => {
                operation.check()?;
                return Ok(ConnectOutcome::Reused);
            }
            SessionRelation::Different => {
                return Err("已有不同目标或正在退出的宿主会话，请先确认切换并等待退出".into());
            }
            SessionRelation::None => {}
        }
        let (fresh, raw_probe) = self.probe(&settings, &[&target.exe])?;
        let target = match_identity(&target, fresh)?;
        let resource_dir = self
            .0
            .resource_dir
            .canonicalize()
            .map_err(|e| format!("资源目录无效: {e}"))?;
        for path in [
            broker_path(&resource_dir, &target)?,
            core_path(&resource_dir, &target)?,
        ] {
            if !path.exists() {
                return Err(format!("缺少打包资源: {}", path.display()));
            }
        }
        fs::create_dir_all(&self.0.log_root).map_err(|e| format!("创建日志目录失败: {e}"))?;
        let id = format!(
            "{}-{}-{}",
            timestamp_ms(),
            std::process::id(),
            self.0.counter.fetch_add(1, Ordering::Relaxed)
        );
        let log_dir = self.0.log_root.join(&id);
        fs::create_dir(&log_dir).map_err(|e| format!("创建会话目录失败: {e}"))?;
        fs::write(log_dir.join("target-probe.json"), raw_probe)
            .map_err(|e| format!("保存目标身份失败: {e}"))?;
        let stdout_file = File::create(log_dir.join("stdout.jsonl")).map_err(|e| e.to_string())?;
        let stderr_file = File::create(log_dir.join("stderr.txt")).map_err(|e| e.to_string())?;
        let requests_file =
            File::create(log_dir.join("requests.jsonl")).map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        {
            let mut session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
            if session.is_some() {
                return Err("另一个连接已启动，请等待该会话退出".into());
            }
            operation.check()?;
            if let Some(revision) = required_binding_revision {
                self.check_binding_revision(revision)?;
            }
            *session = Some(SessionHandle {
                id: id.clone(),
                control: tx.clone(),
                shutdown_requested: Arc::clone(&shutdown_requested),
                settings: settings.clone(),
                target: target.clone(),
            });
            let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            let sequence = state.sequence + 1;
            *state = ClientSnapshot {
                sequence,
                session_id: Some(id.clone()),
                status: "starting".into(),
                host_running: true,
                target: Some(target),
                settings: Some(settings.clone()),
                session_log_dir: Some(log_dir.to_string_lossy().into_owned()),
                ..ClientSnapshot::default()
            };
            self.publish(&state, None);
        }
        let client = self.clone();
        let thread_id = id.clone();
        let producer = self.event_producer();
        let result = thread::Builder::new()
            .name(format!("hook-session-{id}"))
            .spawn(move || {
                let _producer = producer;
                client.run_session(
                    thread_id,
                    settings,
                    attach,
                    resource_dir,
                    log_dir,
                    rx,
                    tx,
                    shutdown_requested,
                    stdout_file,
                    stderr_file,
                    requests_file,
                );
            });
        if let Err(error) = result {
            self.fail(&id, format!("无法启动宿主管理线程: {error}"));
            self.update(&id, |s| s.host_running = false);
            self.finish_session(&id);
            return Err(error.to_string());
        }
        Ok(ConnectOutcome::Started)
    }

    /// Nonblocking. The supervisor requests normal host shutdown first, waits a
    /// bounded interval, and may terminate only its own launcher as a last resort.
    pub fn disconnect(&self) -> Result<(), String> {
        // Cancellation and the final session reservation share this short
        // critical section. Once cancellation returns, an older preparation
        // cannot reserve a new host using a previously checked epoch.
        let session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
        self.0.disconnect_epoch.fetch_add(1, Ordering::AcqRel);
        self.request_session_disconnect(session.as_ref())
    }

    fn request_disconnect(&self) -> Result<(), String> {
        let session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
        self.request_session_disconnect(session.as_ref())
    }

    fn request_session_disconnect(&self, session: Option<&SessionHandle>) -> Result<(), String> {
        if let Some(session) = session
            && !session.shutdown_requested.swap(true, Ordering::AcqRel)
        {
            session
                .control
                .send(Control::Shutdown)
                .map_err(|_| "宿主管理线程已退出".to_string())?;
            self.update(&session.id, |s| s.status = "disconnecting".into());
        }
        Ok(())
    }

    fn probe(
        &self,
        settings: &LaunchSettings,
        names: &[&str],
    ) -> Result<(Vec<Target>, Vec<u8>), String> {
        let exe = self
            .0
            .resource_dir
            .join("brokers/x64/target_probe.exe")
            .canonicalize()
            .map_err(|e| format!("缺少目标查询程序 target_probe.exe: {e}"))?;
        let mut command = launcher(settings, &exe)?;
        command.args(names);
        let (stdout, stderr, success) = capture_command(command, Duration::from_secs(30))?;
        if !success {
            return Err(format!(
                "目标查询失败: {}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        #[derive(Deserialize)]
        struct Report {
            processes: Vec<Target>,
        }
        let report: Report = serde_json::from_slice(&stdout).map_err(|e| {
            format!(
                "目标查询输出无效: {e}; stderr={}",
                String::from_utf8_lossy(&stderr)
            )
        })?;
        Ok((report.processes, stdout))
    }

    #[allow(clippy::too_many_arguments)]
    fn run_session(
        &self,
        id: String,
        settings: LaunchSettings,
        attach: AttachParams,
        resource_dir: PathBuf,
        log_dir: PathBuf,
        rx: Receiver<Control>,
        tx: Sender<Control>,
        shutdown_requested: Arc<AtomicBool>,
        stdout_file: File,
        stderr_file: File,
        requests_file: File,
    ) {
        let launcher_running = Arc::new(AtomicBool::new(false));
        let outcome = (|| -> Result<(), String> {
            let target = self.snapshot().target.ok_or("目标身份丢失")?;
            let mut command = launcher(&settings, &broker_path(&resource_dir, &target)?)?;
            command
                .arg("--runtime-dir")
                .arg(core_path(&resource_dir, &target)?);
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let child = command.spawn().map_err(|e| format!("启动宿主失败: {e}"))?;
            launcher_running.store(true, Ordering::Release);
            let mut child = OwnedLauncher {
                child,
                running: Arc::clone(&launcher_running),
            };
            self.update(&id, |s| {
                s.host_running = true;
                push_bounded(
                    &mut s.logs,
                    format!("Owned host launcher PID {}", child.id()),
                    LOG_LIMIT,
                )
            });
            let stdin = child.stdin.take().ok_or("宿主 stdin 不可用")?;
            let stdout = child.stdout.take().ok_or("宿主 stdout 不可用")?;
            let stderr = child.stderr.take().ok_or("宿主 stderr 不可用")?;
            let (write_tx, write_rx) = mpsc::channel::<Request>();
            let writer_tx = tx.clone();
            let writer_done = Arc::new(AtomicBool::new(false));
            let writer_flag = Arc::clone(&writer_done);
            thread::spawn(move || {
                let result = write_requests(stdin, requests_file, write_rx);
                if let Err(error) = result {
                    let _ = writer_tx.send(Control::Fault(error));
                }
                writer_flag.store(true, Ordering::Release);
            });
            let stdout_done = Arc::new(AtomicBool::new(false));
            let reader_client = self.clone();
            let reader_id = id.clone();
            let reader_tx = tx.clone();
            let reader_done = Arc::clone(&stdout_done);
            let reader_target = target.clone();
            let producer = self.event_producer();
            thread::spawn(move || {
                let _producer = producer;
                reader_client.read_stdout(
                    &reader_id,
                    reader_target,
                    stdout,
                    stdout_file,
                    reader_tx.clone(),
                );
                let _ = reader_tx.send(Control::OutputEnded);
                reader_done.store(true, Ordering::Release);
            });
            let stderr_done = Arc::new(AtomicBool::new(false));
            let reader_client = self.clone();
            let reader_id = id.clone();
            let reader_done = Arc::clone(&stderr_done);
            let producer = self.event_producer();
            thread::spawn(move || {
                let _producer = producer;
                reader_client.read_stderr(&reader_id, stderr, stderr_file);
                reader_done.store(true, Ordering::Release);
            });
            let started = Instant::now();
            let mut attached_at = None;
            let mut connected = false;
            let mut ready = false;
            let mut shutting_down = None;
            let mut forced = false;
            loop {
                for _ in 0..16 {
                    match rx.try_recv() {
                        Ok(Control::Ready) if !ready => {
                            ready = true;
                            if shutting_down.is_none()
                                && !shutdown_requested.load(Ordering::Acquire)
                            {
                                let request =
                                    request(&id, "attach", HostCommand::Attach(attach.clone()))?;
                                write_tx.send(request).map_err(|_| "宿主输入线程已退出")?;
                                self.update(&id, |s| s.status = "attaching".into());
                                attached_at = Some(Instant::now());
                            }
                        }
                        Ok(Control::Ready) => self.fail(&id, "宿主重复发送 ready".into()),
                        Ok(Control::Connected) => connected = true,
                        Ok(Control::Shutdown) => shutdown_requested.store(true, Ordering::Release),
                        Ok(Control::Fault(message)) => {
                            self.fail(&id, message);
                            shutdown_requested.store(true, Ordering::Release);
                        }
                        Ok(Control::OutputEnded) => {
                            if shutting_down.is_none()
                                && !shutdown_requested.load(Ordering::Acquire)
                            {
                                self.fail(&id, "宿主 stdout 在请求关闭之前结束".into());
                                shutdown_requested.store(true, Ordering::Release);
                            }
                        }
                        Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                    }
                }
                if shutting_down.is_none() {
                    let timeout = if !ready && started.elapsed() > START_TIMEOUT {
                        Some("等待宿主 ready 超时")
                    } else if !connected
                        && attached_at.is_some_and(|time| time.elapsed() > ATTACH_TIMEOUT)
                    {
                        Some("等待游戏连接超时")
                    } else {
                        None
                    };
                    if let Some(message) = timeout {
                        self.fail(&id, message.into());
                        shutdown_requested.store(true, Ordering::Release);
                    }
                    if shutdown_requested.load(Ordering::Acquire) {
                        let request =
                            request(&id, "shutdown", HostCommand::Shutdown(EmptyParams {}))?;
                        if write_tx.send(request).is_err() {
                            self.fail(&id, "宿主输入线程已退出，无法发送 shutdown".into());
                        }
                        self.update(&id, |s| {
                            if s.errors.is_empty() {
                                s.status = "disconnecting".into();
                            }
                        });
                        shutting_down = Some(Instant::now());
                    }
                }
                if let Some(status) = child
                    .try_wait()
                    .map_err(|e| format!("查询宿主退出失败: {e}"))?
                {
                    launcher_running.store(false, Ordering::Release);
                    self.update(&id, |s| {
                        s.host_running = false;
                        s.exit_code = status.code();
                        s.status = if status.success() && s.errors.is_empty() {
                            "exited"
                        } else {
                            "error"
                        }
                        .into();
                        for stream in &mut s.streams {
                            stream.active = false;
                        }
                    });
                    if !status.success() {
                        self.fail(&id, format!("宿主异常退出: {status}"));
                    }
                    if !shutdown_requested.load(Ordering::Acquire) {
                        self.fail(&id, "宿主在未请求关闭时退出".into());
                    }
                    break;
                }
                if !forced && shutting_down.is_some_and(|time| time.elapsed() > SHUTDOWN_TIMEOUT) {
                    self.fail(
                        &id,
                        "宿主关闭超时；仅终止本客户端持有的 launcher，未终止游戏或 bottle".into(),
                    );
                    child
                        .kill()
                        .map_err(|e| format!("终止 owned launcher 失败: {e}"))?;
                    forced = true;
                }
                if forced
                    && shutting_down.is_some_and(|time| {
                        time.elapsed() > SHUTDOWN_TIMEOUT + Duration::from_secs(2)
                    })
                {
                    return Err("owned launcher 未在期限内退出；未对游戏或其他进程执行终止".into());
                }
                thread::sleep(Duration::from_millis(15));
            }
            drop(write_tx);
            let drain_started = Instant::now();
            while !(stdout_done.load(Ordering::Acquire)
                && stderr_done.load(Ordering::Acquire)
                && writer_done.load(Ordering::Acquire))
                && drain_started.elapsed() < Duration::from_secs(2)
            {
                thread::sleep(Duration::from_millis(10));
            }
            if !stdout_done.load(Ordering::Acquire) || !stderr_done.load(Ordering::Acquire) {
                self.fail(
                    &id,
                    "宿主退出后输出管道未完成 EOF，会话记录可能不完整".into(),
                );
            }
            // A reader can discover a truncated frame at the same instant the
            // child exits. Preserve those faults even after the wait loop ends.
            while let Ok(control) = rx.try_recv() {
                if let Control::Fault(message) = control {
                    self.fail(&id, message);
                }
            }
            self.update(&id, |s| {
                s.status = if s.errors.is_empty() && s.exit_code == Some(0) {
                    "exited"
                } else {
                    "error"
                }
                .into();
                for stream in &mut s.streams {
                    stream.active = false;
                }
            });
            Ok(())
        })();
        if let Err(error) = outcome {
            self.fail(&id, error);
        }
        self.update(&id, |s| {
            s.host_running = launcher_running.load(Ordering::Acquire)
        });
        let snapshot = self.snapshot();
        if let Err(error) = serde_json::to_vec_pretty(&snapshot)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                fs::write(log_dir.join("final-snapshot.json"), bytes).map_err(|e| e.to_string())
            })
        {
            self.fail(&id, format!("保存最终会话记录失败: {error}"));
        }
        if !launcher_running.load(Ordering::Acquire) {
            self.finish_session(&id);
        }
    }

    fn read_stdout(
        &self,
        id: &str,
        target: Target,
        stdout: impl Read,
        mut file: File,
        control: Sender<Control>,
    ) {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut bytes = Vec::new();
            let count = (&mut reader)
                .take((MAX_EVENT_BYTES + 1) as u64)
                .read_until(b'\n', &mut bytes);
            match count {
                Ok(0) => break,
                Ok(_) => {
                    if let Err(error) = file.write_all(&bytes).and_then(|_| file.flush()) {
                        let _ = control.send(Control::Fault(format!("保存 stdout 失败: {error}")));
                        break;
                    }
                    if bytes.len() > MAX_EVENT_BYTES || bytes.last() != Some(&b'\n') {
                        let _ = control.send(Control::Fault(
                            "宿主 JSONL 行过长或缺少换行，协议记录不完整".into(),
                        ));
                        self.update(id, |s| s.integrity_lost = true);
                        break;
                    }
                    match parse_envelope(&bytes) {
                        Ok(envelope) => {
                            let is_current = {
                                let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
                                state.session_id.as_deref() == Some(id) && state.target.as_ref() == Some(&target)
                            };
                            if !is_current { break; }
                            if let Event::ProcessDisconnected { pid } = &envelope.payload
                                && *pid != target.pid
                            {
                                self.update(id, |state| push_bounded(&mut state.logs,
                                    format!("Ignored process_disconnected for non-target PID {pid}"), LOG_LIMIT));
                                continue;
                            }
                            if event_pid(&envelope.payload).is_some_and(|pid| pid != target.pid) {
                                self.update(id, |s| s.integrity_lost = true);
                                let _ = control
                                    .send(Control::Fault("宿主事件 PID 与本次目标不符".into()));
                                break;
                            }
                            if let Event::Ready { host_arch, .. } = &envelope.payload {
                                let expected = self
                                    .snapshot()
                                    .target
                                    .and_then(|target| target.architecture);
                                let actual = match host_arch {
                                    hook_contracts::Architecture::X86 => "x86",
                                    hook_contracts::Architecture::X64 => "x64",
                                };
                                if expected.as_deref() != Some(actual) {
                                    self.update(id, |s| s.integrity_lost = true);
                                    let _ = control.send(Control::Fault(
                                        "broker bitness differs from selected target".into(),
                                    ));
                                    break;
                                }
                            }
                            let ready = matches!(envelope.payload, Event::Ready { .. });
                            let connected =
                                matches!(envelope.payload, Event::ProcessConnected { .. });
                            let disconnected =
                                matches!(envelope.payload, Event::ProcessDisconnected { .. });
                            let is_error = matches!(envelope.payload, Event::Error { .. });
                            self.update_native(id, envelope.payload);
                            if ready {
                                let _ = control.send(Control::Ready);
                            }
                            if connected {
                                let _ = control.send(Control::Connected);
                            }
                            if disconnected {
                                // A target exiting does not stop the native host's
                                // command loop. Route it through the same bounded
                                // owned-process shutdown as an explicit disconnect.
                                let _ = control.send(Control::Shutdown);
                            }
                            if is_error {
                                let _ = control
                                    .send(Control::Fault("宿主报告错误，正在关闭该会话".into()));
                            }
                        }
                        Err(message) => {
                            self.update(id, |s| s.integrity_lost = true);
                            let _ = control.send(Control::Fault(message));
                            break;
                        }
                    }
                }
                Err(error) => {
                    let _ = control.send(Control::Fault(format!("读取宿主 stdout 失败: {error}")));
                    break;
                }
            }
        }
    }

    fn read_stderr(&self, id: &str, mut stderr: impl Read, mut file: File) {
        let mut bytes = [0_u8; 4096];
        loop {
            match stderr.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    if let Err(error) = file.write_all(&bytes[..count]).and_then(|_| file.flush()) {
                        self.fail(id, format!("保存 stderr 失败: {error}"));
                        break;
                    }
                    let message = String::from_utf8_lossy(&bytes[..count]).trim().to_owned();
                    if !message.is_empty() {
                        self.update(id, |s| {
                            push_bounded(&mut s.logs, format!("stderr: {message}"), LOG_LIMIT)
                        });
                    }
                }
                Err(error) => {
                    self.fail(id, format!("读取宿主 stderr 失败: {error}"));
                    break;
                }
            }
        }
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut ClientSnapshot)) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.session_id.as_deref() == Some(id) {
            state.sequence += 1;
            change(&mut state);
            self.publish(&state, None);
        }
    }

    fn update_native(&self, id: &str, event: Event) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.session_id.as_deref() == Some(id) {
            state.sequence += 1;
            apply_event(&mut state, event.clone());
            self.publish(&state, Some(event));
        }
    }

    fn fail(&self, id: &str, message: String) {
        self.update(id, |s| {
            s.status = "error".into();
            push_bounded(&mut s.errors, message, LOG_LIMIT);
        });
    }

    fn finish_session(&self, id: &str) {
        let mut session = self.0.session.lock().unwrap_or_else(|e| e.into_inner());
        if session.as_ref().is_some_and(|session| session.id == id) {
            *session = None;
        }
    }
}

fn apply_event(state: &mut ClientSnapshot, event: Event) {
    match event {
        Event::Ready {
            host_arch,
            abi_version,
            upstream_commit,
            abi_profile,
        } => {
            state.status = "ready".into();
            push_bounded(
                &mut state.logs,
                format!("Ready {host_arch:?}; ABI {abi_version}; {abi_profile}; {upstream_commit}"),
                LOG_LIMIT,
            );
        }
        Event::ProcessConnected { pid } => {
            if state
                .target
                .as_ref()
                .is_some_and(|target| target.pid == pid)
            {
                state.status = "connected".into();
            } else {
                state.integrity_lost = true;
                push_bounded(
                    &mut state.errors,
                    format!("收到非当前目标的 process_connected: {pid}"),
                    LOG_LIMIT,
                );
            }
        }
        Event::ProcessDisconnected { pid } => {
            if state
                .target
                .as_ref()
                .is_some_and(|target| target.pid == pid)
            {
                state.status = "disconnected".into();
                for stream in &mut state.streams {
                    stream.active = false;
                }
            }
        }
        Event::StreamCreated {
            stream,
            hook_code,
            hook_name,
            ..
        } => {
            let index = stream_index(state, stream, hook_name, hook_code);
            state.streams[index].active = true;
        }
        Event::StreamDestroyed { stream, .. } => {
            if let Some(value) = state.streams.iter_mut().find(|value| value.id == stream) {
                value.active = false;
            }
        }
        Event::Text {
            stream,
            hook_code,
            hook_name,
            text,
        } => {
            let timestamp_ms = timestamp_ms();
            let index = stream_index(state, stream, hook_name, hook_code);
            let value = &mut state.streams[index];
            value.active = true;
            value.latest_text = Some(text.clone());
            value.text_events += 1;
            push_bounded(
                &mut value.history,
                TextLine {
                    sequence: state.sequence,
                    timestamp_ms,
                    text,
                },
                HISTORY_LIMIT,
            );
            state.text_events += 1;
            state.last_text_at_ms = Some(timestamp_ms);
        }
        Event::Error { code, message, .. } => {
            state.status = "error".into();
            push_bounded(&mut state.errors, format!("{code:?}: {message}"), LOG_LIMIT);
        }
        Event::Overflow {
            source,
            dropped_since_last,
            dropped_total,
            integrity_lost,
        } => {
            state.integrity_lost |= integrity_lost || dropped_since_last.0 > 0;
            push_bounded(
                &mut state.errors,
                format!(
                    "Overflow {source}: +{} / total {}; integrity_lost={integrity_lost}",
                    dropped_since_last.0, dropped_total.0
                ),
                LOG_LIMIT,
            );
        }
        Event::Response { id, result } => {
            if matches!(result, ResponseResult::ShuttingDown { .. }) {
                state.shutdown_acknowledged = true;
            }
            if let ResponseResult::Snapshot { integrity_lost, .. } = &result {
                state.integrity_lost |= integrity_lost;
            }
            push_bounded(
                &mut state.logs,
                format!("{}: {result:?}", id.as_str()),
                LOG_LIMIT,
            );
        }
        Event::Log { kind, message } => {
            push_bounded(&mut state.logs, format!("[{kind}] {message}"), LOG_LIMIT)
        }
        Event::HookInserted {
            pid,
            address,
            hook_code,
        } => push_bounded(
            &mut state.logs,
            format!("Hook inserted {pid}:{} {hook_code}", address.0),
            LOG_LIMIT,
        ),
        Event::InjectionFinished {
            pid,
            architecture,
            exit_code,
            accepted_by_helper,
        } => push_bounded(
            &mut state.logs,
            format!(
                "Injection {pid} {architecture:?}: exit={exit_code:?}, accepted={accepted_by_helper}"
            ),
            LOG_LIMIT,
        ),
    }
}

fn stream_index(
    state: &mut ClientSnapshot,
    id: StreamId,
    hook_name: String,
    hook_code: String,
) -> usize {
    if let Some(index) = state.streams.iter().position(|stream| stream.id == id) {
        state.streams[index].hook_name = hook_name;
        state.streams[index].hook_code = hook_code;
        return index;
    }
    let key = format!(
        "{}:{}:{}:{}",
        id.pid, id.address.0, id.context.0, id.context2.0
    );
    state.streams.push(StreamSnapshot {
        key,
        id,
        hook_name,
        hook_code,
        active: true,
        latest_text: None,
        history: Vec::new(),
        text_events: 0,
    });
    state.streams.len() - 1
}

fn parse_envelope(bytes: &[u8]) -> Result<Envelope, String> {
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|e| format!("宿主 JSONL 解析失败: {e}"))?;
    if envelope.v != PROTOCOL_VERSION {
        return Err(format!("不支持的宿主协议版本 {}", envelope.v));
    }
    if let Event::Ready {
        abi_version,
        upstream_commit,
        abi_profile,
        ..
    } = &envelope.payload
        && (*abi_version != hook_contracts::CORE_ABI_VERSION
            || upstream_commit != hook_contracts::CORE_COMMIT
            || abi_profile != hook_contracts::CORE_ABI_PROFILE)
    {
        return Err("unapproved Core commit or ABI profile; legacy runtime is not accepted".into());
    }
    Ok(envelope)
}

fn event_pid(event: &Event) -> Option<u32> {
    match event {
        Event::ProcessConnected { pid }
        | Event::ProcessDisconnected { pid }
        | Event::HookInserted { pid, .. }
        | Event::InjectionFinished { pid, .. } => Some(*pid),
        Event::StreamCreated { stream, .. }
        | Event::StreamDestroyed { stream, .. }
        | Event::Text { stream, .. } => Some(stream.pid),
        _ => None,
    }
}

fn attach_params(target: &Target) -> Result<AttachParams, String> {
    // Request's custom deserializer enforces canonical integers and absolute
    // Windows image paths, sharing precisely the host's validation rules.
    let request: Request = serde_json::from_value(serde_json::json!({
        "v": PROTOCOL_VERSION, "id": "identity-validation", "method": "attach", "params": {
            "pid": target.pid,
            "expected_image_path": target.full_image_path,
            "expected_creation_time": target.creation_filetime,
        }
    }))
    .map_err(|e| format!("目标身份不完整，无法安全连接: {e}"))?;
    if !matches!(target.architecture.as_deref(), Some("x86" | "x64")) {
        return Err("目标进程架构无法确定".into());
    }
    let HostCommand::Attach(params) = request.command else {
        unreachable!()
    };
    Ok(params)
}

fn is_discoverable_target(target: &Target) -> bool {
    let exe = target.exe.to_ascii_lowercase();
    if matches!(
        exe.as_str(),
        "target_probe.exe"
            | "lunahook-host.exe"
            | "system"
            | "system idle process"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "winlogon.exe"
            | "services.exe"
            | "lsass.exe"
            | "svchost.exe"
            | "rpcss.exe"
            | "explorer.exe"
            | "conhost.exe"
            | "winedevice.exe"
            | "wineboot.exe"
            | "wineserver.exe"
    ) {
        return false;
    }
    // Wine exposes its Windows services and helpers under the Windows directory.
    // Use the image path as well as known service names, rather than requiring a
    // game-name allowlist that would hide every new title.
    let system_image = target.full_image_path.as_ref().is_some_and(|path| {
        let path = path.replace('/', "\\").to_ascii_lowercase();
        path.as_bytes().get(1) == Some(&b':')
            && path
                .get(2..)
                .is_some_and(|tail| tail.starts_with("\\windows\\"))
    });
    !target.exe.trim().is_empty() && !system_image && attach_params(target).is_ok()
}

fn match_identity(selected: &Target, candidates: Vec<Target>) -> Result<Target, String> {
    candidates
        .into_iter()
        .find(|candidate| {
            candidate.pid == selected.pid
                && candidate.exe.eq_ignore_ascii_case(&selected.exe)
                && candidate
                    .full_image_path
                    .as_ref()
                    .zip(selected.full_image_path.as_ref())
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
                && candidate.creation_filetime == selected.creation_filetime
                && candidate.architecture == selected.architecture
        })
        .ok_or_else(|| "游戏进程已退出或身份发生变化，请刷新进程列表后重试".into())
}

fn request(session: &str, suffix: &str, command: HostCommand) -> Result<Request, String> {
    let id = serde_json::from_value(serde_json::json!(format!("{session}:{suffix}")))
        .map_err(|e| format!("请求 ID 无效: {e}"))?;
    Ok(Request {
        v: PROTOCOL_VERSION,
        id,
        command,
    })
}

fn target_architecture(target: &Target) -> Result<&str, String> {
    match target.architecture.as_deref() {
        Some(architecture @ ("x86" | "x64")) => Ok(architecture),
        _ => Err("cannot select broker for unknown target architecture".into()),
    }
}

fn broker_path(resources: &Path, target: &Target) -> Result<PathBuf, String> {
    Ok(resources
        .join("brokers")
        .join(target_architecture(target)?)
        .join("lunahook-host.exe"))
}

fn core_path(resources: &Path, target: &Target) -> Result<PathBuf, String> {
    Ok(resources
        .join("core")
        .join(target_architecture(target)?)
        .join("bin"))
}

fn launcher(settings: &LaunchSettings, exe: &Path) -> Result<Command, String> {
    if settings.wine_path.contains('\0') || settings.bottle.contains('\0') {
        return Err("启动设置包含 NUL".into());
    }
    if settings.wine_path.trim().is_empty() {
        if !cfg!(windows) {
            return Err("macOS 需要指定 CrossOver wine 路径".into());
        }
        Ok(Command::new(exe))
    } else {
        if settings.bottle.trim().is_empty() {
            return Err("请选择 CrossOver bottle".into());
        }
        if !Path::new(&settings.wine_path).is_file() {
            return Err("CrossOver wine 路径不存在".into());
        }
        let mut command = Command::new(&settings.wine_path);
        command.args(["--bottle", &settings.bottle, "--no-update", "--no-gui"]);
        command.arg(exe);
        Ok(command)
    }
}

fn write_requests(
    mut stdin: impl Write,
    mut file: File,
    rx: Receiver<Request>,
) -> Result<(), String> {
    for request in rx {
        let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        file.write_all(&bytes)
            .and_then(|_| file.flush())
            .map_err(|e| format!("保存请求失败: {e}"))?;
        stdin
            .write_all(&bytes)
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("写入宿主请求失败: {e}"))?;
    }
    Ok(())
}

fn capture_command(
    mut command: Command,
    timeout: Duration,
) -> Result<(Vec<u8>, Vec<u8>, bool), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|e| format!("启动目标查询失败: {e}"))?;
    let mut child = OwnedLauncher {
        child,
        running: Arc::new(AtomicBool::new(true)),
    };
    let stdout = child.stdout.take().ok_or("目标查询 stdout 不可用")?;
    let stderr = child.stderr.take().ok_or("目标查询 stderr 不可用")?;
    let (tx, rx) = mpsc::channel();
    let stderr_tx = tx.clone();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(2_097_153).read_to_end(&mut bytes);
        let _ = tx.send((true, bytes, result));
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.take(65_537).read_to_end(&mut bytes);
        let _ = stderr_tx.send((false, bytes, result));
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > timeout {
            terminate_owned(&mut child);
            return Err("目标查询超时；仅停止本次查询 launcher".into());
        }
        thread::sleep(Duration::from_millis(15));
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for _ in 0..2 {
        let (is_stdout, bytes, result) = rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "目标查询输出未完成 EOF")?;
        result.map_err(|e| e.to_string())?;
        if (is_stdout && bytes.len() > 2_097_152) || (!is_stdout && bytes.len() > 65_536) {
            return Err("目标查询输出超过限制".into());
        }
        if is_stdout {
            stdout = bytes;
        } else {
            stderr = bytes;
        }
    }
    Ok((stdout, stderr, status.success()))
}

fn terminate_owned(child: &mut Child) -> bool {
    let _ = child.kill();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn push_bounded<T>(items: &mut Vec<T>, item: T, maximum: usize) {
    if items.len() >= maximum {
        items.remove(0);
    }
    items.push(item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hook_contracts::DecimalU64;

    fn target() -> Target {
        Target {
            pid: 7,
            exe: "SR.exe".into(),
            full_image_path: Some(r"C:\games\SR.exe".into()),
            creation_filetime: Some("134367348829636590".into()),
            architecture: Some("x86".into()),
            errors: Vec::new(),
        }
    }

    #[test]
    fn reused_pid_is_not_a_matching_identity() {
        let selected = target();
        let mut recycled = selected.clone();
        recycled.creation_filetime = Some("134367348829636591".into());
        assert!(match_identity(&selected, vec![recycled]).is_err());
        assert!(match_identity(&selected, vec![selected.clone()]).is_ok());
        let mut invalid = selected;
        invalid.creation_filetime = Some("013".into());
        assert!(attach_params(&invalid).is_err());
    }

    #[test]
    fn natural_disconnect_only_requests_shutdown_for_the_current_full_target() {
        let root = std::env::temp_dir().join(format!(
            "luna-disconnect-reader-{}-{}",
            std::process::id(),
            timestamp_ms()
        ));
        fs::create_dir_all(&root).unwrap();
        let client = Client::new(root.clone(), root.clone());
        let target = target();
        {
            let mut state = client.0.state.lock().unwrap();
            state.session_id = Some("owned-session".into());
            state.target = Some(target.clone());
        }
        let (tx, rx) = mpsc::channel();
        let foreign = b"{\"v\":2,\"event\":\"process_disconnected\",\"pid\":8}\n";
        client.read_stdout(
            "owned-session",
            target.clone(),
            foreign.as_slice(),
            File::create(root.join("foreign.jsonl")).unwrap(),
            tx.clone(),
        );
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
        assert!(!client.snapshot().integrity_lost);
        let bytes = b"{\"v\":2,\"event\":\"process_disconnected\",\"pid\":7}\n";
        let mut recycled = target.clone();
        recycled.creation_filetime = Some("134367348829636591".into());
        client.read_stdout(
            "owned-session",
            recycled,
            bytes.as_slice(),
            File::create(root.join("recycled.jsonl")).unwrap(),
            tx.clone(),
        );
        client.read_stdout(
            "old-session",
            target.clone(),
            bytes.as_slice(),
            File::create(root.join("old.jsonl")).unwrap(),
            tx.clone(),
        );
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
        client.read_stdout(
            "owned-session",
            target,
            bytes.as_slice(),
            File::create(root.join("current.jsonl")).unwrap(),
            tx,
        );
        assert!(matches!(rx.try_recv(), Ok(Control::Shutdown)));
        assert_eq!(client.snapshot().status, "disconnected");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovery_keeps_previously_unknown_games() {
        let mut unknown = target();
        unknown.exe = "未知のゲーム.exe".into();
        unknown.full_image_path = Some(r"C:\games\未知のゲーム.exe".into());
        unknown.architecture = Some("x64".into());
        assert!(is_discoverable_target(&unknown));
    }

    #[test]
    fn discovery_excludes_its_helpers_and_windows_processes() {
        for exe in ["TARGET_PROBE.EXE", "lunahook-host.exe", "services.exe"] {
            let mut helper = target();
            helper.exe = exe.into();
            helper.full_image_path = Some(format!(r"C:\tools\{exe}"));
            assert!(!is_discoverable_target(&helper), "{exe}");
        }
        let mut system_image = target();
        system_image.exe = "other-helper.exe".into();
        system_image.full_image_path = Some(r"C:\WINDOWS\system32\other-helper.exe".into());
        assert!(!is_discoverable_target(&system_image));
    }

    #[test]
    fn discovery_requires_a_complete_connectable_identity() {
        let mut missing_path = target();
        missing_path.full_image_path = None;
        missing_path
            .errors
            .push("QueryFullProcessImageNameW failed".into());
        let mut missing_time = target();
        missing_time.creation_filetime = None;
        let mut invalid_time = target();
        invalid_time.creation_filetime = Some("013".into());
        let mut unknown_architecture = target();
        unknown_architecture.architecture = None;
        for invalid in [
            missing_path,
            missing_time,
            invalid_time,
            unknown_architecture,
        ] {
            assert!(!is_discoverable_target(&invalid));
        }
    }

    #[test]
    fn identically_named_hooks_keep_independent_full_width_streams() {
        let mut state = ClientSnapshot::default();
        for context in [u64::MAX, u64::MAX - 1] {
            for number in 0..105 {
                state.sequence += 1;
                apply_event(
                    &mut state,
                    Event::Text {
                        stream: StreamId {
                            pid: 7,
                            address: DecimalU64(u64::MAX),
                            context: DecimalU64(context),
                            context2: DecimalU64(0),
                        },
                        hook_code: "HW0@0".into(),
                        hook_name: "TextOutW".into(),
                        text: format!("台词 {number}"),
                    },
                );
            }
        }
        assert_eq!(state.streams.len(), 2);
        assert_ne!(state.streams[0].key, state.streams[1].key);
        assert_eq!(state.streams[0].history.len(), 100);
        assert_eq!(state.streams[0].history[0].text, "台词 5");
        assert_eq!(state.text_events, 210);
        let json = serde_json::to_value(state).unwrap();
        assert_eq!(json["streams"][0]["id"]["context"], u64::MAX.to_string());
    }

    #[test]
    fn invalid_protocol_and_overflow_cannot_look_like_clean_text() {
        assert!(parse_envelope(br#"{"v":1,"event":"ready","host_arch":"x64","upstream_commit":"x","abi_profile":"x"}"#).is_err());
        assert!(parse_envelope(br#"{"v":2,"event":"text","stream":{}}"#).is_err());
        let mut state = ClientSnapshot::default();
        apply_event(
            &mut state,
            Event::Overflow {
                source: "callback".into(),
                dropped_since_last: DecimalU64(1),
                dropped_total: DecimalU64(1),
                integrity_lost: true,
            },
        );
        assert!(state.integrity_lost);
        assert_eq!(state.errors.len(), 1);
        assert_eq!(state.text_events, 0);
    }

    #[test]
    fn retired_session_reader_cannot_write_into_reconnected_state() {
        let client = Client::new(PathBuf::new(), PathBuf::new());
        {
            let mut state = client.0.state.lock().unwrap();
            state.session_id = Some("new-session".into());
            state.status = "connected".into();
        }
        client.fail("retired-session", "late EOF error".into());
        let state = client.snapshot();
        assert_eq!(state.status, "connected");
        assert!(state.errors.is_empty());
        assert_eq!(state.sequence, 0);
    }
    #[test]
    fn subscriptions_deliver_actual_native_text_without_polling() {
        let client = Client::new(PathBuf::new(), PathBuf::new());
        let subscription = client.subscribe(4).expect("subscription");
        client.0.state.lock().unwrap().session_id = Some("native-session".into());
        let stream = StreamId {
            pid: 7,
            address: DecimalU64(u64::MAX),
            context: DecimalU64(1 << 60),
            context2: DecimalU64(9),
        };
        client.update_native(
            "native-session",
            Event::Text {
                stream: stream.clone(),
                hook_name: "arbitrary engine".into(),
                hook_code: "HW@0".into(),
                text: "実際のイベント".into(),
            },
        );
        let event = subscription
            .recv_timeout(Duration::from_millis(20))
            .expect("push event");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.session_id.as_deref(), Some("native-session"));
        assert!(event.received_at_ms > 0);
        let Some(Event::Text {
            stream: actual,
            text,
            ..
        }) = event.payload
        else {
            panic!("text event")
        };
        assert_eq!(actual, stream);
        assert_eq!(text, "実際のイベント");
        assert_eq!(client.snapshot().streams.len(), 1);
    }

    #[test]
    fn full_subscription_reports_loss_even_without_a_future_event() {
        let client = Client::new(PathBuf::new(), PathBuf::new());
        let subscription = client.subscribe(1).unwrap();
        client.0.state.lock().unwrap().session_id = Some("session".into());
        client.update("session", |_| {});
        client.update("session", |_| {});
        assert!(matches!(
            subscription.recv_timeout(Duration::from_millis(1)),
            Err(SubscriptionError::IntegrityLost { dropped_total: 1 })
        ));
    }

    #[test]
    fn legacy_and_wrong_core_profiles_are_rejected() {
        for (version, abi, commit, profile) in [
            (
                1,
                1,
                hook_contracts::CORE_COMMIT,
                hook_contracts::CORE_ABI_PROFILE,
            ),
            (
                2,
                2,
                hook_contracts::CORE_COMMIT,
                hook_contracts::CORE_ABI_PROFILE,
            ),
            (2, 1, "old-commit", hook_contracts::CORE_ABI_PROFILE),
            (
                2,
                1,
                hook_contracts::CORE_COMMIT,
                "old-ten-callback-profile",
            ),
        ] {
            let bytes = serde_json::to_vec(&serde_json::json!({"v":version,"event":"ready",
                "host_arch":"x64","abi_version":abi,"upstream_commit":commit,"abi_profile":profile})).unwrap();
            assert!(parse_envelope(&bytes).is_err());
        }
        let bytes = serde_json::to_vec(&Envelope::new(Event::Ready {
            host_arch: hook_contracts::Architecture::X86,
            abi_version: hook_contracts::CORE_ABI_VERSION,
            upstream_commit: hook_contracts::CORE_COMMIT.into(),
            abi_profile: hook_contracts::CORE_ABI_PROFILE.into(),
        }))
        .unwrap();
        assert!(parse_envelope(&bytes).is_ok());
    }

    #[test]
    fn broker_is_selected_by_verified_target_bitness() {
        let resources = Path::new("/resources");
        let mut selected = target();
        assert_eq!(
            broker_path(resources, &selected).unwrap(),
            resources.join("brokers/x86/lunahook-host.exe")
        );
        selected.architecture = Some("x64".into());
        assert_eq!(
            broker_path(resources, &selected).unwrap(),
            resources.join("brokers/x64/lunahook-host.exe")
        );
        assert_eq!(
            core_path(resources, &selected).unwrap(),
            resources.join("core/x64/bin")
        );
        selected.architecture = None;
        assert!(broker_path(resources, &selected).is_err());
    }
}

#[cfg(test)]
mod capture_drain_tests {
    use super::*;
    fn client() -> Client {
        Client::new(PathBuf::new(), PathBuf::new())
    }
    #[test]
    fn publisher_guard_prevents_false_stopped_proof_until_real_thread_finishes() {
        let client = client();
        let producer = client.event_producer();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let _producer = producer;
            rx.recv().unwrap();
        });
        assert!(
            client
                .wait_event_producers_stopped(Duration::from_millis(10))
                .is_err()
        );
        tx.send(()).unwrap();
        worker.join().unwrap();
        client
            .wait_event_producers_stopped(Duration::from_millis(50))
            .unwrap();
    }
    #[test]
    fn sticky_subscription_keeps_all_accepted_facts_for_explicit_recovery_drain() {
        let client = client();
        let poisoned = client.subscribe(1).unwrap();
        {
            let mut state = client.0.state.lock().unwrap();
            state.session_id = Some("drain-session".into());
            state.sequence = 1;
            client.publish(&state, None);
            state.sequence = 2;
            client.publish(&state, None);
        }
        assert!(matches!(
            poisoned.recv_timeout(Duration::from_millis(1)),
            Err(SubscriptionError::IntegrityLost { dropped_total: 1 })
        ));
        let next = client.subscribe(1).unwrap();
        let accepted = poisoned.drain_received();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].sequence, 1);
        assert_eq!(poisoned.dropped_total(), 1);
        assert!(matches!(
            poisoned.recv_timeout(Duration::from_millis(1)),
            Err(SubscriptionError::IntegrityLost { dropped_total: 1 })
        ));
        {
            let mut state = client.0.state.lock().unwrap();
            state.sequence = 3;
            client.publish(&state, None);
        }
        assert_eq!(
            next.recv_timeout(Duration::from_millis(10))
                .unwrap()
                .sequence,
            3
        );
    }
}
