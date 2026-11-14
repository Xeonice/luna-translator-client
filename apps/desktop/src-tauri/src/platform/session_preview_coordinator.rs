//! Session preview ownership and lifecycle effects; no AppHandle is retained.
use super::window_preview::WindowPreviewService;
use app_core::{
    application::runtime::ApplicationRuntime,
    contracts::*,
    ports::runtime::{
        CurrentSessionPreviewAuthority, HookBackend, InactiveSessionPreviewAuthority,
    },
};
use app_infra::hook_backend::CoreHookBackend;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
#[cfg(test)]
#[path = "../../../../../crates/app-infra/tests/reading_component_fixtures.rs"]
pub(crate) mod reading_component_fixtures;

/// Passive, bounded binding metadata. It never owns a Runtime, Hook lease, game
/// text, credential or frame payload and cannot grant an operation authority.
pub(crate) mod binding_observation {
    use serde::Serialize;
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        fs::{File, OpenOptions},
        io::{self, Seek, SeekFrom, Write},
        path::Path,
        sync::{
            Arc, OnceLock,
            atomic::{AtomicU64, Ordering},
            mpsc::{self, Receiver, SyncSender, TrySendError},
        },
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    const MAX_RECORD: usize = 16 * 1024;
    const QUEUE_LIMIT: usize = 128;
    const HEALTH_INTERVAL: Duration = Duration::from_millis(100);
    #[derive(Clone, Default, Serialize)]
    pub(crate) struct Context {
        pub review_id: Option<String>,
        pub session_id: Option<String>,
        pub operation_id: Option<String>,
        pub selection_set_revision: Option<String>,
        pub active_target_identity_sha256: Option<String>,
        pub candidate_target_identity_sha256: Option<String>,
    }
    struct Event {
        observed_at_ms: u64,
        action: String,
        correlation: String,
        extra: Value,
        context: Option<Context>,
    }
    #[derive(Default)]
    struct Counters {
        accepted: AtomicU64,
        processed: AtomicU64,
        written: AtomicU64,
        flushed: AtomicU64,
        dropped: AtomicU64,
        invalid: AtomicU64,
        write_failures: AtomicU64,
    }
    /// The caller only uses bounded try_send and atomics: no diagnostic lock,
    /// queue space or filesystem operation can delay a Core authority guard.
    struct Observer {
        sender: SyncSender<Event>,
        counters: Arc<Counters>,
    }
    trait JournalSink: Write + Seek + Send + 'static {
        fn truncate(&mut self, length: u64) -> io::Result<()>;
    }
    impl JournalSink for File {
        fn truncate(&mut self, length: u64) -> io::Result<()> {
            self.set_len(length)
        }
    }
    struct Journal<S: JournalSink> {
        file: S,
        cohort: Value,
        contexts: BTreeMap<String, Context>,
        health_offset: u64,
        sequence: u64,
    }
    static JOURNAL: OnceLock<Observer> = OnceLock::new();
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    fn hex(value: &str, len: usize) -> bool {
        value.len() == len
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn parse_boot(value: &str) -> Option<(u64, u64)> {
        let values = value
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .take(2)
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        (values.len() == 2 && values[0] > 0 && values[1] < 1_000_000)
            .then(|| (values[0], values[1]))
    }
    fn write_line(file: &mut impl Write, value: &Value) -> io::Result<()> {
        serde_json::to_writer(&mut *file, value)?;
        file.write_all(b"\n")
    }
    struct SizeLimit(usize);
    impl Write for SizeLimit {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| io::Error::other("binding observation record limit"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn within_size(value: &impl Serialize, limit: usize) -> bool {
        serde_json::to_writer(SizeLimit(limit), value).is_ok()
    }
    fn valid(event: &Event) -> bool {
        matches!(
            event.action.as_str(),
            "context_transition"
                | "intent"
                | "port_call"
                | "port_return"
                | "persist_commit"
                | "show_hud_completed"
                | "hide_main_completed"
                | "window_effect_failed"
                | "operation_cancelled"
        ) && !event.correlation.is_empty()
            && event.correlation.len() <= 128
            && event.extra.is_object()
            && within_size(&event.extra, MAX_RECORD / 2)
            && event
                .context
                .as_ref()
                .is_none_or(|context| within_size(context, 4096))
    }
    impl Observer {
        fn enqueue(&self, event: Event) {
            if !valid(&event) {
                self.counters.invalid.fetch_add(1, Ordering::Relaxed);
                return;
            }
            // Count before publication so the receiver cannot finish an event
            // before its acceptance is visible. A rejected try_send rolls back
            // only this tentative count; it can never have been processed.
            self.counters.accepted.fetch_add(1, Ordering::AcqRel);
            match self.sender.try_send(event) {
                Ok(()) => {}
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                    self.counters.accepted.fetch_sub(1, Ordering::AcqRel);
                    self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
    impl<S: JournalSink> Journal<S> {
        fn row(&self, kind: &str) -> Value {
            let mut value = self.cohort.clone();
            value["kind"] = json!(kind);
            value
        }
        fn health(&mut self, counters: &Counters, accepted: u64) -> io::Result<()> {
            self.file.seek(SeekFrom::Start(self.health_offset))?;
            let mut value = self.row("binding_journal_health");
            value["accepted"] = json!(accepted);
            value["written"] = json!(self.sequence);
            value["last_sequence"] = json!(self.sequence);
            value["dropped"] = json!(counters.dropped.load(Ordering::Acquire));
            value["invalid"] = json!(counters.invalid.load(Ordering::Acquire));
            value["write_failures"] = json!(counters.write_failures.load(Ordering::Acquire));
            write_line(&mut self.file, &value)?;
            let end = self.file.stream_position()?;
            self.file.truncate(end)?;
            self.file.flush()?;
            counters.flushed.store(self.sequence, Ordering::Release);
            Ok(())
        }
        fn record(&mut self, event: Event, counters: &Counters) -> io::Result<()> {
            if let Some(context) = event.context {
                if !self.contexts.contains_key(&event.correlation) && self.contexts.len() >= 1024 {
                    counters.dropped.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
                self.contexts.insert(event.correlation.clone(), context);
            }
            let mut value = self.row("binding_journal_event");
            value["sequence"] = json!(self.sequence + 1);
            value["observed_at_ms"] = json!(event.observed_at_ms);
            value["action"] = json!(event.action);
            value["correlation_id"] = json!(event.correlation);
            let context = serde_json::to_value(
                self.contexts
                    .get(&event.correlation)
                    .cloned()
                    .unwrap_or_default(),
            )?;
            for (key, val) in context.as_object().into_iter().flatten() {
                value[key] = val.clone();
            }
            for (key, val) in event.extra.as_object().into_iter().flatten() {
                if matches!(
                    key.as_str(),
                    "method" | "args" | "response" | "effect" | "error_code"
                ) {
                    value[key] = val.clone();
                }
            }
            let bytes = serde_json::to_vec(&value)?;
            if bytes.len() > MAX_RECORD {
                counters.invalid.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            if self.health_offset + bytes.len() as u64 + 4096 > MAX_BYTES {
                counters.dropped.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            self.file.seek(SeekFrom::Start(self.health_offset))?;
            self.file.write_all(&bytes)?;
            self.file.write_all(b"\n")?;
            self.health_offset = self.file.stream_position()?;
            self.sequence += 1;
            counters.written.store(self.sequence, Ordering::Release);
            Ok(())
        }
    }
    fn writer<S: JournalSink>(
        mut journal: Journal<S>,
        receiver: Receiver<Event>,
        counters: Arc<Counters>,
    ) {
        let mut last_health = None;
        loop {
            let mut disconnected = false;
            match receiver.recv_timeout(HEALTH_INTERVAL) {
                Ok(event) => {
                    if journal.record(event, &counters).is_err() {
                        counters.write_failures.fetch_add(1, Ordering::Relaxed);
                    }
                    counters.processed.fetch_add(1, Ordering::Release);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    disconnected = true;
                }
            }
            // A health record represents an actual complete drain, not the
            // number merely queued. A later enqueue belongs to the next drain.
            let accepted = counters.accepted.load(Ordering::Acquire);
            let accepted =
                (accepted == counters.processed.load(Ordering::Acquire)).then_some(accepted);
            if let Some(accepted) = accepted {
                let fingerprint = (
                    accepted,
                    journal.sequence,
                    counters.dropped.load(Ordering::Acquire),
                    counters.invalid.load(Ordering::Acquire),
                    counters.write_failures.load(Ordering::Acquire),
                );
                if last_health != Some(fingerprint) {
                    match journal.health(&counters, accepted) {
                        Ok(()) => last_health = Some(fingerprint),
                        Err(_) => {
                            counters.write_failures.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            if disconnected {
                break;
            }
        }
    }
    fn start_writer<S: JournalSink>(
        journal: Journal<S>,
        capacity: usize,
    ) -> (Observer, std::thread::JoinHandle<()>) {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let counters = Arc::new(Counters::default());
        let observer = Observer {
            sender,
            counters: counters.clone(),
        };
        let handle = std::thread::spawn(move || writer(journal, receiver, counters));
        (observer, handle)
    }
    pub(crate) fn initialize(
        path: &Path,
        resources: &Path,
        profile: &str,
        instance: &str,
    ) -> io::Result<()> {
        let sealed: Value =
            serde_json::from_slice(&std::fs::read(resources.join("luna-source-identity.json"))?)?;
        let source = sealed["source_tree_sha256"]
            .as_str()
            .filter(|s| hex(s, 64))
            .ok_or_else(|| io::Error::other("sealed source identity missing"))?;
        if sealed["schema_version"] != 1 || sealed["kind"] != "luna_sealed_source_identity" {
            return Err(io::Error::other("sealed source identity invalid"));
        }
        let nonce = std::env::var("LUNA_BINDING_NONCE")
            .unwrap_or_else(|_| app_core::domain::content_hash(instance.as_bytes())[..32].into());
        if !hex(&nonce, 32) {
            return Err(io::Error::other("binding observation nonce invalid"));
        }
        let boot = std::process::Command::new("/usr/sbin/sysctl")
            .args(["-n", "kern.boottime"])
            .output()?;
        let (sec, usec) = boot
            .status
            .success()
            .then(|| parse_boot(&String::from_utf8_lossy(&boot.stdout)))
            .flatten()
            .ok_or_else(|| io::Error::other("binding observation boot identity missing"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        let cohort = json!({"nonce":nonce,"pid":std::process::id(),"profile_id":profile,"boot_sec":sec,"boot_usec":usec});
        let mut first = cohort.clone();
        first["kind"] = json!("binding_journal_begin");
        first["schema_version"] = json!(1);
        first["source_tree_sha256"] = json!(source);
        write_line(&mut file, &first)?;
        let health_offset = file.stream_position()?;
        let mut journal = Journal {
            file,
            cohort,
            contexts: BTreeMap::new(),
            health_offset,
            sequence: 0,
        };
        journal.health(&Counters::default(), 0)?;
        let (observer, _writer) = start_writer(journal, QUEUE_LIMIT);
        JOURNAL
            .set(observer)
            .map_err(|_| io::Error::other("binding observation already initialized"))
    }
    pub(crate) fn event(action: &str, correlation: &str, extra: Value) {
        if let Some(journal) = JOURNAL.get() {
            journal.enqueue(Event {
                observed_at_ms: now_ms(),
                action: action.into(),
                correlation: correlation.into(),
                extra,
                context: None,
            });
        }
    }
    pub(crate) fn correlation() -> String {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        format!(
            "native-binding-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }
    pub(crate) fn transition(correlation: &str, context: Context) {
        if let Some(journal) = JOURNAL.get() {
            journal.enqueue(Event {
                observed_at_ms: now_ms(),
                action: "context_transition".into(),
                correlation: correlation.into(),
                extra: json!({}),
                context: Some(context),
            });
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::{
            Condvar, Mutex,
            atomic::{AtomicBool, Ordering},
        };
        fn path(label: &str) -> std::path::PathBuf {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            std::env::temp_dir().join(format!(
                "luna-binding-{label}-{}-{}.jsonl",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
        }
        fn journal(path: &Path) -> Journal<File> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            Journal {
                file,
                cohort: json!({"nonce":"a".repeat(32),"pid":1}),
                contexts: BTreeMap::new(),
                health_offset: 0,
                sequence: 0,
            }
        }
        fn event(action: &str, correlation: &str, context: Option<Context>) -> Event {
            Event {
                observed_at_ms: 123,
                action: action.into(),
                correlation: correlation.into(),
                extra: json!({}),
                context,
            }
        }
        fn rows(path: &Path) -> Vec<Value> {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        }
        #[test]
        fn boot_identity_requires_seconds_and_valid_microseconds() {
            assert_eq!(
                parse_boot("{ sec = 1794000000, usec = 12345 } Wed"),
                Some((1794000000, 12345))
            );
            assert_eq!(parse_boot("{ sec = 0, usec = 1 }"), None);
            assert_eq!(parse_boot("{ sec = 1, usec = 1000000 }"), None);
            assert_eq!(parse_boot("1"), None);
        }
        #[test]
        fn bounded_writer_has_exact_sequence_and_visible_loss_health() {
            let path = path("writer");
            let (observer, handle) = start_writer(journal(&path), 4);
            observer.enqueue(event("port_call", "operation", None));
            observer.enqueue(event("port_return", "operation", None));
            observer.enqueue(event("invented_action", "operation", None));
            drop(observer);
            handle.join().unwrap();
            let rows = rows(&path);
            assert_eq!(rows.len(), 3);
            assert_eq!(rows[0]["sequence"], 1);
            assert_eq!(rows[1]["sequence"], 2);
            assert_eq!(rows[2]["kind"], "binding_journal_health");
            assert_eq!(rows[2]["accepted"], 2);
            assert_eq!(rows[2]["written"], 2);
            assert_eq!(rows[2]["invalid"], 1);
            assert_eq!(rows[2]["dropped"], 0);
            assert_eq!(
                rows[0]["observed_at_ms"], 123,
                "writer does not replace API action time"
            );
            std::fs::remove_file(path).unwrap();
        }
        #[test]
        fn concurrent_review_does_not_relabel_prior_operation_effects() {
            let path = path("context");
            let (observer, handle) = start_writer(journal(&path), 8);
            observer.enqueue(event(
                "context_transition",
                "operation-a",
                Some(Context {
                    session_id: Some("session-a".into()),
                    operation_id: Some("operation-a".into()),
                    selection_set_revision: Some("7".into()),
                    ..Default::default()
                }),
            ));
            observer.enqueue(event("persist_commit", "operation-a", None));
            observer.enqueue(event(
                "context_transition",
                "review-b",
                Some(Context {
                    session_id: Some("session-b".into()),
                    review_id: Some("review-b".into()),
                    ..Default::default()
                }),
            ));
            observer.enqueue(event("show_hud_completed", "operation-a", None));
            drop(observer);
            handle.join().unwrap();
            let rows = rows(&path);
            assert_eq!(rows[2]["session_id"], "session-b");
            assert_eq!(rows[3]["session_id"], "session-a");
            assert_eq!(rows[3]["operation_id"], "operation-a");
            assert_eq!(rows[3]["selection_set_revision"], "7");
            std::fs::remove_file(path).unwrap();
        }
        struct SlowSink {
            file: File,
            entered: Arc<(Mutex<bool>, Condvar)>,
            release: Arc<(Mutex<bool>, Condvar)>,
        }
        impl Write for SlowSink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let (lock, cv) = &*self.entered;
                *lock.lock().unwrap() = true;
                cv.notify_all();
                let (lock, cv) = &*self.release;
                let mut ready = lock.lock().unwrap();
                while !*ready {
                    ready = cv.wait(ready).unwrap();
                }
                self.file.write(bytes)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.file.flush()
            }
        }
        impl Seek for SlowSink {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                self.file.seek(position)
            }
        }
        impl JournalSink for SlowSink {
            fn truncate(&mut self, length: u64) -> io::Result<()> {
                self.file.set_len(length)
            }
        }
        #[test]
        fn blocked_disk_and_full_queue_never_block_producer_and_loss_is_visible() {
            let path = path("slow");
            let base = journal(&path);
            let entered = Arc::new((Mutex::new(false), Condvar::new()));
            let release = Arc::new((Mutex::new(false), Condvar::new()));
            let sink = SlowSink {
                file: base.file,
                entered: entered.clone(),
                release: release.clone(),
            };
            let (observer, handle) = start_writer(
                Journal {
                    file: sink,
                    cohort: base.cohort,
                    contexts: base.contexts,
                    health_offset: 0,
                    sequence: 0,
                },
                1,
            );
            observer.enqueue(event("port_call", "first", None));
            let (lock, cv) = &*entered;
            let ready = lock.lock().unwrap();
            let (ready, timed) = cv
                .wait_timeout_while(ready, Duration::from_secs(3), |v| !*v)
                .unwrap();
            assert!(!timed.timed_out());
            drop(ready);
            let before = std::time::Instant::now();
            observer.enqueue(event("port_return", "queued", None));
            observer.enqueue(event("intent", "full", None));
            assert!(
                before.elapsed() < Duration::from_millis(100),
                "enqueue cannot wait for a blocked file writer"
            );
            assert_eq!(observer.counters.accepted.load(Ordering::Acquire), 2);
            assert_eq!(
                observer.counters.written.load(Ordering::Acquire),
                0,
                "queued is not written"
            );
            assert_eq!(observer.counters.dropped.load(Ordering::Acquire), 1);
            let (lock, cv) = &*release;
            *lock.lock().unwrap() = true;
            cv.notify_all();
            drop(observer);
            handle.join().unwrap();
            let rows = rows(&path);
            let health = rows.last().unwrap();
            assert_eq!(health["accepted"], 2);
            assert_eq!(health["written"], 2);
            assert_eq!(health["dropped"], 1);
            std::fs::remove_file(path).unwrap();
        }
        struct FailureSink {
            file: File,
            fail: Arc<AtomicBool>,
        }
        impl Write for FailureSink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.fail.load(Ordering::Acquire) {
                    Err(io::Error::other("controlled disk write failure"))
                } else {
                    self.file.write(bytes)
                }
            }
            fn flush(&mut self) -> io::Result<()> {
                self.file.flush()
            }
        }
        impl Seek for FailureSink {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                self.file.seek(position)
            }
        }
        impl JournalSink for FailureSink {
            fn truncate(&mut self, length: u64) -> io::Result<()> {
                self.file.set_len(length)
            }
        }
        #[test]
        fn recovered_write_failure_is_never_reported_as_loss_free() {
            let path = path("failure");
            let base = journal(&path);
            let fail = Arc::new(AtomicBool::new(true));
            let sink = FailureSink {
                file: base.file,
                fail: fail.clone(),
            };
            let (observer, handle) = start_writer(
                Journal {
                    file: sink,
                    cohort: base.cohort,
                    contexts: base.contexts,
                    health_offset: 0,
                    sequence: 0,
                },
                4,
            );
            observer.enqueue(event("port_call", "failed", None));
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while observer.counters.processed.load(Ordering::Acquire) < 1
                && std::time::Instant::now() < deadline
            {
                std::thread::yield_now();
            }
            assert_eq!(observer.counters.processed.load(Ordering::Acquire), 1);
            fail.store(false, Ordering::Release);
            observer.enqueue(event("port_return", "recovered", None));
            drop(observer);
            handle.join().unwrap();
            let rows = rows(&path);
            let health = rows.last().unwrap();
            assert_eq!(health["accepted"], 2);
            assert_eq!(health["written"], 1);
            assert_eq!(health["last_sequence"], 1);
            assert!(health["write_failures"].as_u64().unwrap() > 0);
            assert_eq!(rows[0]["correlation_id"], "recovered");
            std::fs::remove_file(path).unwrap();
        }
    }
}

#[derive(Clone)]
struct PreviewLease {
    generation: String,
    consumer_id: String,
    consumer: PreviewConsumer,
}
#[derive(Default)]
struct PreviewOwnership {
    epoch: u64,
    leases: BTreeMap<String, PreviewLease>,
    handoffs: BTreeMap<String, ReviewedPreviewHandoff>,
}
#[derive(Clone)]
struct ReviewedPreviewHandoff {
    review_id: String,
    profile: BindingProfileContext,
    candidate: AssociationTargetIdentity,
    epoch: u64,
    ticket: super::window_preview::PreviewLeaseTicket,
}

struct PreparedCurrentPreview {
    candidate_id: String,
    start: Box<dyn FnOnce(&WindowPreviewService, &PreviewProjection) -> AppResult<()> + Send>,
}
trait CurrentPreviewNative: Send + Sync {
    fn current_candidate_id(&self) -> AppResult<String>;
    fn freeze(&self, stopped: &AtomicBool) -> AppResult<PreparedCurrentPreview>;
}
struct CoreCurrentPreviewNative(Arc<CoreHookBackend>);
impl CurrentPreviewNative for CoreCurrentPreviewNative {
    fn current_candidate_id(&self) -> AppResult<String> {
        self.0.preview_candidate_id_for_active_binding()
    }
    fn freeze(&self, stopped: &AtomicBool) -> AppResult<PreparedCurrentPreview> {
        let target = self
            .0
            .freeze_active_preview_target(&|| stopped.load(Ordering::Acquire))?;
        let candidate_id = target.candidate_id().to_owned();
        Ok(PreparedCurrentPreview {
            candidate_id,
            start: Box::new(move |service, projection| {
                #[cfg(target_os = "macos")]
                {
                    service.start_remote(projection, target, false)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (service, projection, target);
                    Err(AppError::new(
                        "preview_unsupported",
                        "preview",
                        "当前平台不支持窗口预览",
                    ))
                }
            }),
        })
    }
}
#[cfg(test)]
type CurrentOwnerObserver = dyn Fn(&str, serde_json::Value) + Send + Sync;
struct CurrentPreviewWorker {
    runtime: Arc<ApplicationRuntime>,
    native: Arc<dyn CurrentPreviewNative>,
    service: Arc<WindowPreviewService>,
    ownership: Arc<Mutex<PreviewOwnership>>,
    closing: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    #[cfg(test)]
    observer: Option<Arc<CurrentOwnerObserver>>,
}
struct CurrentPreviewOwnerHandle {
    stopped: Arc<AtomicBool>,
    wake: mpsc::SyncSender<()>,
    subscription: tauri::async_runtime::JoinHandle<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Drop for CurrentPreviewOwnerHandle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        self.subscription.abort();
        // Joining is explicit and off the native main loop. A drop never
        // waits for an external verifier or a collector acknowledgement.
        if self
            .worker
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}
impl CurrentPreviewWorker {
    fn observe(&self, kind: &str, data: serde_json::Value) {
        #[cfg(test)]
        if let Some(observer) = &self.observer {
            observer(kind, data);
        }
        #[cfg(not(test))]
        let _ = (kind, data);
    }
    fn spawn(self: Arc<Self>) -> AppResult<CurrentPreviewOwnerHandle> {
        let (wake, receiver) = mpsc::sync_channel(1);
        self.service.subscribe_lifecycle(wake.clone())?;
        let mut updates = self.runtime.subscribe();
        let subscribed = self.clone();
        let core_wake = wake.clone();
        let subscription = tauri::async_runtime::spawn(async move {
            subscribed.observe("subscription_started", serde_json::json!({}));
            loop {
                if subscribed.stopped.load(Ordering::Acquire) {
                    break;
                }
                let (identity, revision) = {
                    let update = updates.borrow_and_update();
                    let snapshot = &update.snapshot;
                    (
                        serde_json::json!({"profile":snapshot.profile_id,"server":snapshot.server_instance_id,"generation":snapshot.profile_generation,
                        "session":snapshot.session.as_ref().map(|s|(&s.session_id,&s.game_id,&s.source_kind,&s.state))}),
                        snapshot.commit_revision,
                    )
                };
                // Same-session reuse can change the committed origin without
                // changing the public SID. Every genuine Core update wakes the
                // bounded singleflight owner; its read-only query never publishes.
                subscribed.observe(
                    "active_subscription_event",
                    serde_json::json!({"identity":identity,"commit_revision":revision}),
                );
                if matches!(
                    core_wake.try_send(()),
                    Err(mpsc::TrySendError::Disconnected(_))
                ) {
                    break;
                }
                if updates.changed().await.is_err() {
                    break;
                }
            }
        });
        let stopped = self.stopped.clone();
        let spawned = self.clone();
        let worker = std::thread::spawn(move || {
            let mut retry = None;
            while !spawned.stopped.load(Ordering::Acquire) {
                match retry {
                    Some(delay) => {
                        let _ = receiver.recv_timeout(delay);
                    }
                    None => {
                        if receiver.recv().is_err() {
                            break;
                        }
                    }
                }
                if spawned.stopped.load(Ordering::Acquire) {
                    break;
                }
                // Cleanup ownership survives a closed Candidate panel and does
                // not require a Core active session or a live operation Promise.
                let cleanup_pending = spawned.service.drive_deferred_retirements();
                if matches!(cleanup_pending, Ok(true) | Err(_)) {
                    retry = Some(Duration::from_millis(100));
                }
                if spawned.closing.load(Ordering::Acquire) {
                    retry = Some(Duration::from_millis(250));
                    continue;
                }
                if spawned.service.is_closed() {
                    break;
                }
                match spawned.reconcile() {
                    Ok(false) => {
                        retry = if matches!(cleanup_pending, Ok(true) | Err(_)) {
                            Some(Duration::from_millis(100))
                        } else {
                            None
                        }
                    }
                    Ok(true) | Err(_) => {
                        retry = Some(retry.map_or(Duration::from_millis(50), |delay: Duration| {
                            (delay * 2).min(Duration::from_secs(1))
                        }))
                    }
                }
            }
            spawned.stopped.store(true, Ordering::Release);
        });
        Ok(CurrentPreviewOwnerHandle {
            stopped,
            wake,
            subscription,
            worker: Some(worker),
        })
    }
    fn reconcile(&self) -> AppResult<bool> {
        // Capture the exact old slice before reading Core. A later active
        // session must never be enumerated by an earlier inactive observation.
        let original = self.service.session_tickets()?;
        let Some(authority) = self.runtime.current_session_preview_authority()? else {
            let Some(inactive) = self.runtime.inactive_session_preview_authority()? else {
                return Ok(true);
            };
            return self.reconcile_inactive(inactive, original);
        };
        self.reconcile_authority(authority)
    }
    fn reconcile_inactive(
        &self,
        inactive: InactiveSessionPreviewAuthority,
        original: Vec<super::window_preview::PreviewLeaseTicket>,
    ) -> AppResult<bool> {
        let mut busy = false;
        self.runtime
            .with_inactive_session_preview_authority(&inactive, &mut || {
                if self.closing.load(Ordering::Acquire) || self.stopped.load(Ordering::Acquire) {
                    return Err(AppError::new(
                        "application_closing",
                        "preview",
                        "预览正在收尾",
                    ));
                }
                for ticket in &original {
                    if self.service.retire_exact(ticket)?
                        == super::window_preview::RetirementAdmission::Draining
                    {
                        busy = true;
                    }
                }
                Ok(())
            })?;
        Ok(busy)
    }
    fn reconcile_authority(&self, authority: CurrentSessionPreviewAuthority) -> AppResult<bool> {
        self.observe("active_authority", authority_observation(&authority));
        let sid = authority.hook_session.session_id.clone();
        let candidate = self.native.current_candidate_id()?;
        if let Some(projection) = self.service.current_session_projection(&sid, &candidate)? {
            self.runtime
                .with_current_session_preview_authority(&authority, &mut || {
                    if self.native.current_candidate_id()? != candidate {
                        return Err(AppError::new("stale_target", "preview", "已关联窗口已变化"));
                    }
                    let mut owner = self.ownership.lock().map_err(|_| state_error())?;
                    synchronize_session_ownership(&mut owner, &self.service.session_tickets()?);
                    publish_preview(&mut owner, &projection, PreviewConsumer::Session);
                    Ok(())
                })?;
            self.observe("session_retained",serde_json::json!({"session_id":sid,"lease_id":projection.lease_id,"generation":projection.generation}));
            return Ok(false);
        }
        // File preparation and external verification are outside Core/Hook locks.
        let prepared = self.native.freeze(&self.stopped)?;
        let mut prepared = Some(prepared);
        let mut busy = false;
        self.runtime.with_current_session_preview_authority(&authority,&mut || {
            if self.closing.load(Ordering::Acquire) || self.stopped.load(Ordering::Acquire) || self.service.is_closed() {
                return Err(AppError::new("application_closing","preview","预览正在收尾"));
            }
            let candidate=prepared.as_ref().ok_or_else(state_error)?.candidate_id.clone();
            if self.native.current_candidate_id()?!=candidate {return Err(AppError::new("stale_target","preview","已关联窗口已变化"));}
            self.observe("guard_accepted",authority_observation(&authority));
            let mut owner=self.ownership.lock().map_err(|_|state_error())?;
            synchronize_session_ownership(&mut owner,&self.service.session_tickets()?);
            for ticket in self.service.session_tickets()? {
                if ticket.consumer_id!=sid || ticket.candidate_id!=candidate {
                    let admission=self.service.retire_exact(&ticket)?;
                    self.observe("retirement_mark",serde_json::to_value(&ticket).unwrap_or_default());
                    if admission==super::window_preview::RetirementAdmission::Draining {busy=true;}
                    else if owner.leases.get(&ticket.lease_id).is_some_and(|lease|lease.generation==ticket.generation) {owner.leases.remove(&ticket.lease_id);}
                }
            }
            if busy {self.observe("typed_busy",serde_json::json!({"code":"preview_cleanup_pending"}));return Ok(());}
            let transfer=authority.origin.as_ref().and_then(|origin|owner.handoffs.get(&origin.operation_id)
                .filter(|handoff|handoff.review_id==origin.review_id && handoff.profile==authority.profile
                    && handoff.candidate==origin.candidate && handoff.epoch==owner.epoch)
                .filter(|handoff|self.service.exact_candidate(&handoff.ticket)
                    && self.service.transferable(&handoff.ticket.lease_id,&handoff.ticket.generation,&candidate))
                .cloned());
            if let Some(handoff)=transfer {
                let projection=self.service.transfer_pairing_to_session(&handoff.ticket.lease_id,&handoff.ticket.generation,&candidate,&sid)?;
                owner.leases.remove(&handoff.ticket.lease_id);
                publish_preview(&mut owner,&projection,PreviewConsumer::Session);
                self.observe("session_handoff",serde_json::json!({"session_id":sid,"lease_id":projection.lease_id,"generation":projection.generation}));
                return Ok(());
            }
            let projection=self.service.begin_current_session_resolution(PreviewStartRequest {
                candidate_id:candidate,consumer:PreviewConsumer::Session,consumer_id:sid.clone(),expected_generation:None,
            })?;
            let prepared=prepared.take().ok_or_else(state_error)?;
            match (prepared.start)(&self.service,&projection) {
                Ok(())=>{
                    publish_preview(&mut owner,&projection,PreviewConsumer::Session);
                    self.observe("session_collector_started",authority_observation(&authority));
                }
                Err(error)=>{
                    if matches!(error.code.as_str(),"preview_capture_busy"|"preview_lease_limit"|"preview_cleanup_pending"|"capture_cleanup_pending") {
                        for ticket in self.service.session_tickets()?.into_iter().filter(|ticket|Some(ticket.lease_id.as_str())==projection.lease_id.as_deref() && ticket.generation==projection.generation) {
                            let _=self.service.retire_exact(&ticket)?;
                        }
                        busy=true;self.observe("typed_busy",serde_json::json!({"code":error.code}));return Ok(());
                    }
                    let failed=self.service.fail_resolution(projection.lease_id.as_deref().ok_or_else(state_error)?,&projection.generation,error)?;
                    publish_preview(&mut owner,&failed,PreviewConsumer::Session);
                    self.observe("session_start_failed",serde_json::json!({"session_id":sid,"error_code":failed.error_code}));
                }
            }
            Ok(())
        })?;
        self.observe("guard_exit", serde_json::json!({"session_id":sid}));
        Ok(busy)
    }
}
#[cfg(test)]
impl CurrentPreviewOwnerHandle {
    fn stop_and_join(mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        self.subscription.abort();
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
async fn prepare_reviewed_handoff(
    runtime: Arc<ApplicationRuntime>,
    service: Arc<WindowPreviewService>,
    ownership: Arc<Mutex<PreviewOwnership>>,
    request: ReviewedAssociateRequest,
) -> AppResult<u64> {
    let (epoch, tickets) = {
        let owner = ownership.lock().map_err(|_| state_error())?;
        (owner.epoch, service.candidate_tickets()?)
    };
    let query = request.clone();
    let review = run_preview_effect(move || runtime.association_preview_intent(&query)).await;
    if let Ok(review) = review {
        let mut owner = ownership.lock().map_err(|_| state_error())?;
        let current_epoch = owner.epoch;
        owner.handoffs.retain(|_, handoff| {
            handoff.epoch == current_epoch && service.exact_candidate(&handoff.ticket)
        });
        if owner.epoch == epoch && !owner.handoffs.contains_key(&request.operation_id) {
            let mut matching = tickets
                .into_iter()
                .filter(|ticket| {
                    ticket.candidate_id == review.candidate.candidate_id
                        && service.exact_candidate(ticket)
                })
                .collect::<Vec<_>>();
            if matching.len() == 1 && owner.handoffs.len() < 64 {
                owner.handoffs.insert(
                    request.operation_id,
                    ReviewedPreviewHandoff {
                        review_id: review.review_id,
                        profile: review.profile,
                        candidate: review.candidate,
                        epoch,
                        ticket: matching.pop().expect("one exact ticket"),
                    },
                );
            }
        }
    }
    Ok(epoch)
}

fn admit_prepared_preview(
    runtime: &ApplicationRuntime,
    native: &dyn CurrentPreviewNative,
    service: &WindowPreviewService,
    ownership: &Mutex<PreviewOwnership>,
    request: &PreviewStartRequest,
    preserve: bool,
    epoch: u64,
    authority: Option<&CurrentSessionPreviewAuthority>,
) -> AppResult<PreviewProjection> {
    let mut result = None;
    let mut admit = || {
        let mut owner = ownership.lock().map_err(|_| state_error())?;
        if owner.epoch != epoch {
            return Err(AppError::new(
                "preview_cancelled",
                "preview",
                "预览目标已变化",
            ));
        }
        let begin = || {
            if request.consumer == PreviewConsumer::Session {
                service.begin_current_session_resolution(request.clone())
            } else {
                service.begin_resolution(request.clone())
            }
        };
        let projection = if preserve {
            if let Some(update) = service.consumer_update(&request.consumer_id, false)? {
                if request
                    .expected_generation
                    .as_deref()
                    .is_some_and(|g| g != update.projection.generation)
                    || !service.request_matches(
                        update.projection.lease_id.as_deref().unwrap_or_default(),
                        &update.projection.generation,
                        request,
                    )
                {
                    return Err(AppError::new(
                        "preview_generation_conflict",
                        "preview",
                        "预览目标已变化",
                    ));
                }
                update.projection
            } else {
                begin()?
            }
        } else {
            begin()?
        };
        publish_preview(&mut owner, &projection, request.consumer);
        result = Some(projection);
        Ok(())
    };
    if request.consumer == PreviewConsumer::Session {
        let authority = authority
            .ok_or_else(|| AppError::new("preview_session", "preview", "缺少当前会话授权"))?;
        runtime.with_current_session_preview_authority(authority, &mut || {
            if authority.hook_session.session_id != request.consumer_id
                || native.current_candidate_id()? != request.candidate_id
            {
                return Err(AppError::new("stale_target", "preview", "已关联窗口已变化"));
            }
            admit()
        })?;
    } else {
        admit()?;
    }
    result.ok_or_else(state_error)
}

fn authority_observation(authority: &CurrentSessionPreviewAuthority) -> serde_json::Value {
    let active = &authority.hook_session;
    let resource = &active.resource;
    serde_json::json!({"profile":authority.profile,"active_witness":{
        "session_id":active.session_id,"game_id":active.game_id,"environment_key":active.environment_key,
        "target_exe_path":active.target_exe_path,"windows_pid":active.windows_pid,"process_created_u64":active.process_created_u64,
        "architecture":active.architecture,"broker_instance_id":active.broker_instance_id,"target_identity_sha256":active.target_identity_sha256,
        "resource":{"snapshot_id":resource.snapshot_id,"kind":resource.kind,"version":resource.version,
            "manifest_sha256":resource.manifest_sha256,"contract_version":resource.contract_version,"metadata_json":resource.metadata_json}}})
}

pub(crate) struct SessionPreviewCoordinator {
    runtime: Arc<ApplicationRuntime>,
    hook: Arc<CoreHookBackend>,
    service: Arc<WindowPreviewService>,
    ownership: Arc<Mutex<PreviewOwnership>>,
    current_owner: Mutex<Option<CurrentPreviewOwnerHandle>>,
}

/// Cover consumers can read and guarded-commit an observed frame, never control capture.
#[derive(Clone)]
pub(crate) struct SessionCoverConsumer {
    service: Arc<WindowPreviewService>,
}
impl SessionCoverConsumer {
    pub(super) fn consumer_update(
        &self,
        consumer_id: &str,
        include_frame: bool,
    ) -> AppResult<Option<PreviewUpdate>> {
        self.service.consumer_update(consumer_id, include_frame)
    }
    fn commit_current_session_frame<T>(
        &self,
        session_id: &str,
        frame: &PreviewFrame,
        commit: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        self.service
            .commit_current_session_frame(session_id, frame, commit)
    }
    #[cfg(test)]
    pub(super) fn for_test(service: Arc<WindowPreviewService>) -> Self {
        Self { service }
    }
}

/// Effect dependencies stay private here; cover receives only its required operations.
pub(crate) struct SessionCoverDependencies {
    runtime: Arc<ApplicationRuntime>,
    hook: Arc<CoreHookBackend>,
    closing: Arc<AtomicBool>,
}
impl SessionCoverDependencies {
    pub(super) fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }
    pub(super) fn report_failure(&self, error: AppError) {
        self.runtime.report_native_effect_failure(error);
    }
    pub(super) async fn identity(&self) -> AppResult<Option<(CoverSessionGuard, Option<String>)>> {
        let runtime = self.runtime.clone();
        let hook = self.hook.clone();
        run_preview_effect(move || {
            let snapshot = runtime.snapshot()?;
            let Some(session) = snapshot.session.filter(|s| {
                s.source_kind == "hook"
                    && matches!(s.state.as_str(), "opening" | "connected" | "active")
            }) else {
                return Ok(None);
            };
            let expected = hook.cover_session_guard()?;
            if expected.session_id != session.session_id {
                return Ok(None);
            }
            Ok(Some((expected, session.game_id)))
        })
        .await
    }
    pub(super) async fn prepare(
        &self,
        guard: CoverSessionGuard,
        frame: PreviewFrame,
    ) -> AppResult<Option<(PreviewFrame, PreparedSessionCover)>> {
        let runtime = self.runtime.clone();
        run_preview_effect(move || {
            runtime
                .prepare_session_cover(guard, &frame.payload)
                .map(|prepared| prepared.map(|p| (frame, p)))
        })
        .await
    }
    pub(super) async fn verify_and_commit(
        &self,
        consumer: SessionCoverConsumer,
        session_id: String,
        frame: PreviewFrame,
        prepared: PreparedSessionCover,
    ) -> AppResult<CoverInstallOutcome> {
        let runtime = self.runtime.clone();
        run_preview_effect(move || {
            runtime.verify_session_cover(&prepared)?;
            consumer.commit_current_session_frame(&session_id, &frame, || {
                runtime.commit_session_cover(prepared)
            })
        })
        .await
    }
}

fn state_error() -> AppError {
    AppError::new("desktop_state", "desktop", "窗口状态暂不可用，请重试")
}

/// Platform effects reuse the existing executor/error contract without importing IPC.
async fn run_preview_effect<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppError::new("application_executor", "ipc", "应用执行器意外停止"))?
}

impl SessionPreviewCoordinator {
    pub(crate) fn new(
        runtime: Arc<ApplicationRuntime>,
        hook: Arc<CoreHookBackend>,
        service: Arc<WindowPreviewService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            hook,
            service,
            ownership: Arc::new(Mutex::new(PreviewOwnership::default())),
            current_owner: Mutex::new(None),
        })
    }
    pub(crate) fn epoch(&self) -> AppResult<u64> {
        Ok(self.ownership.lock().map_err(|_| state_error())?.epoch)
    }
    pub(crate) async fn prepare_reviewed_association(
        &self,
        request: ReviewedAssociateRequest,
    ) -> AppResult<u64> {
        prepare_reviewed_handoff(
            self.runtime.clone(),
            self.service.clone(),
            self.ownership.clone(),
            request,
        )
        .await
    }
    pub(crate) fn start_current_session_owner(&self, closing: Arc<AtomicBool>) -> AppResult<()> {
        let mut owner = self.current_owner.lock().map_err(|_| state_error())?;
        if owner.is_some() {
            return Ok(());
        }
        let worker = Arc::new(CurrentPreviewWorker {
            runtime: self.runtime.clone(),
            native: Arc::new(CoreCurrentPreviewNative(self.hook.clone())),
            service: self.service.clone(),
            ownership: self.ownership.clone(),
            closing,
            stopped: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            observer: None,
        });
        *owner = Some(worker.spawn()?);
        Ok(())
    }
    pub(crate) fn begin_association(&self) -> AppResult<u64> {
        let mut owner = self.ownership.lock().map_err(|_| state_error())?;
        owner.epoch = owner.epoch.checked_add(1).ok_or_else(|| {
            AppError::new("preview_generation_exhausted", "preview", "请重启客户端")
        })?;
        Ok(owner.epoch)
    }
    pub(crate) fn cover_dependencies(&self, closing: Arc<AtomicBool>) -> SessionCoverDependencies {
        SessionCoverDependencies {
            runtime: self.runtime.clone(),
            hook: self.hook.clone(),
            closing,
        }
    }
    pub(crate) fn cover_consumer(&self) -> SessionCoverConsumer {
        SessionCoverConsumer {
            service: self.service.clone(),
        }
    }
    pub(crate) fn stop_all(&self, expected_epoch: Option<u64>) {
        self.stop_previews_at_epoch(expected_epoch);
    }
    pub(crate) fn stop_candidates(
        &self,
        expected_epoch: Option<u64>,
    ) -> tauri::async_runtime::JoinHandle<()> {
        self.stop_preview_consumers_if_epoch(PreviewConsumer::Candidate, expected_epoch)
    }
    pub(crate) async fn transfer_candidate(
        &self,
        candidate: String,
        expected_epoch: u64,
    ) -> AppResult<()> {
        let runtime = self.runtime.clone();
        let hook = self.hook.clone();
        let service = self.service.clone();
        let ownership = self.ownership.clone();
        run_preview_effect(move || {
            let session = runtime
                .snapshot()?
                .session
                .ok_or_else(|| AppError::new("preview_session", "preview", "当前没有游戏会话"))?;
            if hook.preview_candidate_id_for_active_binding()? != candidate {
                return Err(AppError::new("stale_target", "preview", "已关联窗口已变化"));
            }
            let proposals = {
                let owner = ownership.lock().map_err(|_| state_error())?;
                if owner.epoch != expected_epoch {
                    return Err(AppError::new(
                        "preview_cancelled",
                        "preview",
                        "关联预览事务已变化",
                    ));
                }
                owner
                    .leases
                    .iter()
                    .filter(|(_, l)| l.consumer == PreviewConsumer::Candidate)
                    .map(|(id, l)| (id.clone(), l.clone()))
                    .collect::<Vec<_>>()
            };
            let mut matches = proposals
                .into_iter()
                .filter_map(|(id, lease)| {
                    service
                        .transferable(&id, &lease.generation, &candidate)
                        .then_some((id, lease))
                })
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Ok(());
            }
            if matches.len() != 1 {
                return Err(AppError::new(
                    "preview_generation_conflict",
                    "preview",
                    "存在多个候选预览",
                ));
            }
            let (id, lease) = matches.pop().expect("one matching pair");
            // Transfer authority only. No candidate pixels/timestamps survive, and
            // the first session frame still requires full pre/post validation. A
            // transient probe lock here must not discard an explicit selection.
            if runtime
                .snapshot()?
                .session
                .as_ref()
                .map(|s| s.session_id.as_str())
                != Some(session.session_id.as_str())
                || hook.preview_candidate_id_for_active_binding()? != candidate
            {
                return Err(AppError::new("stale_target", "preview", "已关联会话已变化"));
            }
            let mut owner = ownership.lock().map_err(|_| state_error())?;
            if owner.epoch != expected_epoch
                || owner.leases.get(&id).is_none_or(|l| {
                    l.consumer != PreviewConsumer::Candidate
                        || l.generation != lease.generation
                        || l.consumer_id != lease.consumer_id
                })
            {
                return Err(AppError::new(
                    "preview_generation_conflict",
                    "preview",
                    "候选预览已变化",
                ));
            }
            let projection = service.transfer_pairing_to_session(
                &id,
                &lease.generation,
                &candidate,
                &session.session_id,
            )?;
            owner
                .leases
                .retain(|_, l| l.consumer != PreviewConsumer::Session);
            publish_preview(&mut owner, &projection, PreviewConsumer::Session);
            Ok(())
        })
        .await
    }
    /// Only the original Core receipt can authorize preview handoff. Filesystem
    /// freezing happens before the lease; the guarded callback only changes
    /// metadata and enqueues bounded collector work for that same candidate.
    pub(crate) async fn finish_reviewed_association(
        &self,
        receipt: AssociationOperationReceipt,
        _expected_epoch: u64,
    ) -> AppResult<()> {
        if receipt.replayed {
            return Ok(());
        }
        let runtime = self.runtime.clone();
        let wake = self
            .current_owner
            .lock()
            .map_err(|_| state_error())?
            .as_ref()
            .map(|owner| owner.wake.clone());
        run_preview_effect(move || {
            // One-shot operation validation remains separate. Capture ownership
            // is driven independently by the current-session subscription and
            // fresh Core authority, never by this Promise continuation.
            runtime.with_association_preview_authority(&receipt, &mut |candidate, session| {
                binding_observation::transition(
                    &receipt.operation_id,
                    binding_observation::Context {
                        review_id: Some(receipt.review_id.clone()),
                        session_id: Some(session.into()),
                        operation_id: Some(receipt.operation_id.clone()),
                        active_target_identity_sha256: Some(
                            candidate.target_identity_sha256.clone(),
                        ),
                        candidate_target_identity_sha256: Some(
                            candidate.target_identity_sha256.clone(),
                        ),
                        ..Default::default()
                    },
                );
                if let Some(wake) = &wake {
                    let _ = wake.try_send(());
                }
                Ok(())
            })?;
            Ok(())
        })
        .await
    }
    fn stop_preview_consumers_if_epoch(
        &self,
        consumer: PreviewConsumer,
        expected_epoch: Option<u64>,
    ) -> tauri::async_runtime::JoinHandle<()> {
        let leases = self
            .ownership
            .lock()
            .map(|mut owner| take_consumer_previews(&mut owner, consumer, expected_epoch))
            .unwrap_or_default();
        let service = self.service.clone();
        tauri::async_runtime::spawn_blocking(move || {
            for (lease_id, lease) in leases {
                let _ = service.control(PreviewControlRequest {
                    lease_id,
                    generation: lease.generation,
                    action: PreviewControl::Stop,
                });
            }
        })
    }
    fn stop_previews_at_epoch(&self, expected_epoch: Option<u64>) {
        let leases = self
            .ownership
            .lock()
            .map(|mut owner| take_cancelled_previews(&mut owner, expected_epoch))
            .unwrap_or_default();
        let service = self.service.clone();
        tauri::async_runtime::spawn_blocking(move || {
            for (lease_id, lease) in leases {
                let _ = service.control(PreviewControlRequest {
                    lease_id,
                    generation: lease.generation,
                    action: PreviewControl::Stop,
                });
            }
        });
    }
    async fn prepare_preview_attempt(
        &self,
        mut request: PreviewStartRequest,
        preserve: bool,
    ) -> AppResult<(
        PreviewStartRequest,
        PreviewProjection,
        u64,
        Option<CurrentSessionPreviewAuthority>,
    )> {
        let hook = self.hook.clone();
        let runtime = self.runtime.clone();
        let ownership = self.ownership.clone();
        let epoch = ownership.lock().map_err(|_| state_error())?.epoch;
        let (request, authority) = run_preview_effect(move || {
            if request.consumer_id.is_empty() || request.consumer_id.len() > 128 {
                return Err(AppError::new(
                    "preview_consumer",
                    "preview",
                    "预览使用方无效",
                ));
            }
            let authority = if request.consumer == PreviewConsumer::Session {
                let current = runtime
                    .current_session_preview_authority()?
                    .ok_or_else(|| {
                        AppError::new("preview_session", "preview", "当前没有游戏会话")
                    })?;
                if current.hook_session.session_id != request.consumer_id {
                    return Err(AppError::new("preview_session", "preview", "会话已变化"));
                }
                request.candidate_id = hook.preview_candidate_id_for_active_binding()?;
                Some(current)
            } else {
                if runtime
                    .snapshot()?
                    .session
                    .is_some_and(|s| s.session_id == request.consumer_id)
                {
                    return Err(AppError::new(
                        "preview_consumer",
                        "preview",
                        "候选不能借用会话身份",
                    ));
                }
                None
            };
            Ok((request, authority))
        })
        .await?;
        let service = self.service.clone();
        let runtime = self.runtime.clone();
        let native = CoreCurrentPreviewNative(self.hook.clone());
        run_preview_effect(move || {
            let projection = admit_prepared_preview(
                &runtime,
                &native,
                &service,
                &ownership,
                &request,
                preserve,
                epoch,
                authority.as_ref(),
            )?;
            Ok((request, projection, epoch, authority))
        })
        .await
    }
    async fn enqueue_remote_preview(
        &self,
        request: PreviewStartRequest,
        select: bool,
    ) -> AppResult<PreviewProjection> {
        let (request, before, epoch, authority) =
            self.prepare_preview_attempt(request, true).await?;
        let service = self.service.clone();
        let hook = self.hook.clone();
        let ownership = self.ownership.clone();
        let runtime = self.runtime.clone();
        #[cfg(target_os = "macos")]
        let result = run_preview_effect(move || {
            let attempt: AppResult<PreviewProjection> = (|| {
                let mut target = Some(hook.freeze_preview_target(&request.candidate_id)?);
                let mut enqueue = || {
                    let owner = ownership.lock().map_err(|_| state_error())?;
                    let id = before.lease_id.as_deref().ok_or_else(state_error)?;
                    if owner.epoch != epoch
                        || owner
                            .leases
                            .get(id)
                            .is_none_or(|l| l.generation != before.generation)
                    {
                        return Err(AppError::new(
                            "preview_cancelled",
                            "preview",
                            "预览事务已变化",
                        ));
                    }
                    service.start_remote(
                        &before,
                        target.take().ok_or_else(state_error)?,
                        select,
                    )?;
                    Ok(())
                };
                if let Some(authority) = &authority {
                    runtime.with_current_session_preview_authority(authority, &mut || {
                        if hook.preview_candidate_id_for_active_binding()? != request.candidate_id {
                            return Err(AppError::new(
                                "stale_target",
                                "preview",
                                "已关联窗口已变化",
                            ));
                        }
                        enqueue()
                    })?;
                } else {
                    enqueue()?;
                }
                Ok(before.clone())
            })();
            match attempt {
                Ok(value) => Ok(value),
                Err(failure) => {
                    if matches!(
                        failure.code.as_str(),
                        "preview_cancelled"
                            | "preview_generation_conflict"
                            | "current_session_preview_stale"
                            | "reading_effect_stale"
                            | "application_closing"
                            | "stale_target"
                    ) {
                        return Err(failure);
                    }
                    let id = before.lease_id.as_deref().ok_or_else(state_error)?;
                    if service.has_resolved(id, &before.generation) {
                        return Err(failure);
                    }
                    service.fail_resolution(
                        id,
                        &before.generation,
                        AppError::new(
                            &failure.code,
                            "preview",
                            "当前窗口预览尚未建立，可核对目标后重试；取文不受影响",
                        ),
                    )
                }
            }
        })
        .await;
        #[cfg(not(target_os = "macos"))]
        let result = {
            let _ = (
                request, before, epoch, service, hook, ownership, runtime, authority, select,
            );
            Err(AppError::new(
                "preview_unsupported",
                "preview",
                "当前平台不支持窗口预览",
            ))
        };
        result
    }
    pub(crate) async fn start(&self, request: PreviewStartRequest) -> AppResult<PreviewProjection> {
        self.enqueue_remote_preview(request, false).await
    }
    pub(crate) async fn select(
        &self,
        request: PreviewStartRequest,
    ) -> AppResult<PreviewProjection> {
        self.enqueue_remote_preview(request, true).await
    }
    pub(crate) async fn ensure_associated(&self) -> AppResult<()> {
        let Some(session) = self.runtime.snapshot()?.session else {
            return Ok(());
        };
        if self
            .service
            .consumer_update(&session.session_id, false)?
            .is_none()
        {
            let _ = self
                .start(PreviewStartRequest {
                    candidate_id: String::new(),
                    consumer: PreviewConsumer::Session,
                    consumer_id: session.session_id,
                    expected_generation: None,
                })
                .await?;
        }
        Ok(())
    }
    pub(crate) fn session_projection(&self) -> AppResult<PreviewProjection> {
        let session_id = self.runtime.snapshot()?.session.map(|s| s.session_id);
        let service = &self.service;
        if let Some(id) = &session_id
            && let Some(update) = service.consumer_update(id, false)?
        {
            self.owned_lease(
                update.projection.lease_id.as_deref().unwrap_or_default(),
                &update.projection.generation,
            )?;
            return Ok(update.projection);
        }
        let capability = service.capability();
        Ok(PreviewProjection {
            lease_id: None,
            generation: "0".into(),
            consumer_id: session_id.unwrap_or_default(),
            state: if capability.permission == PreviewPermission::Denied {
                PreviewState::PermissionDenied
            } else {
                PreviewState::Unavailable
            },
            permission: capability.permission,
            target_identity_hash: None,
            attempted_at_ms: None,
            captured_at_ms: None,
            content_changed_at_ms: None,
            permission_checked_at_ms: None,
            error_code: Some("preview_session_pending".into()),
            detail: "已关联窗口将自动预览；若权限未授予，游戏取文仍保持独立".into(),
        })
    }
    pub(crate) fn binding_projection(&self) -> AppResult<GameWindowBindingProjection> {
        let session = self.runtime.snapshot()?.session.map(|s| s.session_id);
        let binding = match self.hook.cached_preview_binding() {
            Ok(binding) if binding.session_id == session => binding,
            Ok(_) => {
                return Err(AppError::new(
                    "game_window_stale",
                    "capture",
                    "窗口关联已属于其他会话",
                ));
            }
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "game_window_unbound" | "game_window_no_window" | "game_window_stale"
                ) =>
            {
                GameWindowBindingProjection {
                    state: match error.code.as_str() {
                        "game_window_no_window" => GameWindowBindingState::NoWindow,
                        "game_window_unbound" => GameWindowBindingState::Unbound,
                        _ => GameWindowBindingState::Stale,
                    },
                    session_id: session,
                    target_identity_sha256: None,
                    window_title: None,
                    verified_at_ms: None,
                    error_code: Some(error.code),
                    detail: "当前没有已确认的 Windows 窗口关联".into(),
                }
            }
            Err(error) => return Err(error),
        };
        #[cfg(target_os = "macos")]
        {
            Ok(self.service.collector()?.cached_binding(binding))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = binding;
            Err(state_error())
        }
    }
    fn owned_lease(&self, id: &str, generation: &str) -> AppResult<()> {
        if self
            .ownership
            .lock()
            .map_err(|_| state_error())?
            .leases
            .get(id)
            .map(|entry| entry.generation.as_str())
            != Some(generation)
        {
            return Err(AppError::new(
                "preview_generation_conflict",
                "preview",
                "预览已结束或目标已变化",
            ));
        }
        Ok(())
    }
    pub(crate) async fn control(
        &self,
        request: PreviewControlRequest,
    ) -> AppResult<PreviewProjection> {
        self.owned_lease(&request.lease_id, &request.generation)?;
        let service = self.service.clone();
        let id = request.lease_id.clone();
        let old_generation = request.generation.clone();
        let stopped = request.action == PreviewControl::Stop;
        let result = run_preview_effect(move || service.control(request)).await?;
        let mut owner = self.ownership.lock().map_err(|_| state_error())?;
        if owner.leases.get(&id).map(|entry| entry.generation.as_str())
            == Some(old_generation.as_str())
        {
            if stopped && result.state == PreviewState::Stopped {
                owner.leases.remove(&id);
            } else if let Some(entry) = owner.leases.get_mut(&id) {
                entry.generation = result.generation.clone();
            }
        } else if !stopped {
            let service = self.service.clone();
            let generation = result.generation.clone();
            let lease_id = id.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = service.control(PreviewControlRequest {
                    lease_id,
                    generation,
                    action: PreviewControl::Stop,
                });
            });
            return Err(AppError::new(
                "preview_cancelled",
                "preview",
                "预览使用方已关闭",
            ));
        }
        Ok(result)
    }
    pub(crate) fn latest(
        &self,
        lease_id: &str,
        generation: &str,
        after_sequence: Option<&str>,
    ) -> AppResult<PreviewUpdate> {
        self.owned_lease(lease_id, generation)?;
        self.service
            .latest_update(lease_id, generation, after_sequence)
    }
}

/// Core has already validated the exact session authority. The independent
/// candidate epoch controls only transfer/cleanup, never the session preview.
fn publish_reviewed_session_preview(
    owner: &mut PreviewOwnership,
    projection: &PreviewProjection,
    candidate_epoch: u64,
) -> Vec<(String, PreviewLease)> {
    let mut releases = owner
        .leases
        .iter()
        .filter(|(id, lease)| {
            lease.consumer == PreviewConsumer::Session
                && (Some(id.as_str()) != projection.lease_id.as_deref()
                    || lease.generation != projection.generation)
        })
        .map(|(id, lease)| (id.clone(), lease.clone()))
        .collect::<Vec<_>>();
    owner
        .leases
        .retain(|_, lease| lease.consumer != PreviewConsumer::Session);
    publish_preview(owner, projection, PreviewConsumer::Session);
    // Removed leases are exact generation tickets; later candidates are kept.
    // Actual stop/cleanup runs after releasing the Core authority guard.
    releases.extend(take_consumer_previews(
        owner,
        PreviewConsumer::Candidate,
        Some(candidate_epoch),
    ));
    releases
}
fn synchronize_session_ownership(
    owner: &mut PreviewOwnership,
    tickets: &[super::window_preview::PreviewLeaseTicket],
) {
    owner.leases.retain(|id, lease| {
        lease.consumer != PreviewConsumer::Session
            || tickets.iter().any(|ticket| {
                ticket.lease_id == *id
                    && ticket.generation == lease.generation
                    && ticket.consumer_id == lease.consumer_id
            })
    });
}

/// A full collector queue must not orphan a still-current service lease.
/// Never replace ownership published by a later operation for the same ID.
fn retain_cleanup_ticket(owner: &mut PreviewOwnership, id: String, lease: PreviewLease) -> bool {
    if let std::collections::btree_map::Entry::Vacant(entry) = owner.leases.entry(id) {
        entry.insert(lease);
        true
    } else {
        false
    }
}

fn take_consumer_previews(
    owner: &mut PreviewOwnership,
    consumer: PreviewConsumer,
    expected_epoch: Option<u64>,
) -> Vec<(String, PreviewLease)> {
    if expected_epoch.is_some_and(|epoch| owner.epoch != epoch) {
        return Vec::new();
    }
    if consumer == PreviewConsumer::Candidate {
        owner.epoch = owner.epoch.saturating_add(1);
    }
    let ids: Vec<_> = owner
        .leases
        .iter()
        .filter(|(_, l)| l.consumer == consumer)
        .map(|(id, _)| id.clone())
        .collect();
    ids.into_iter()
        .filter_map(|id| owner.leases.remove(&id).map(|lease| (id, lease)))
        .collect()
}

fn take_cancelled_previews(
    owner: &mut PreviewOwnership,
    expected_epoch: Option<u64>,
) -> std::collections::BTreeMap<String, PreviewLease> {
    if expected_epoch.is_some_and(|epoch| owner.epoch != epoch) {
        return std::collections::BTreeMap::new();
    }
    owner.epoch = owner.epoch.saturating_add(1);
    std::mem::take(&mut owner.leases)
}

fn publish_preview(
    owner: &mut PreviewOwnership,
    projection: &PreviewProjection,
    consumer: PreviewConsumer,
) {
    owner
        .leases
        .retain(|_, l| l.consumer_id != projection.consumer_id);
    if let Some(id) = &projection.lease_id {
        owner.leases.insert(
            id.clone(),
            PreviewLease {
                generation: projection.generation.clone(),
                consumer_id: projection.consumer_id.clone(),
                consumer,
            },
        );
    }
}

#[cfg(test)]
pub(crate) fn automatic_permission_resume_request(
    projection: &PreviewProjection,
    permission: PreviewPermission,
) -> Option<PreviewControlRequest> {
    (matches!(
        projection.state,
        PreviewState::PermissionRevoked | PreviewState::PermissionDenied
    ) && permission == PreviewPermission::Granted)
        .then(|| PreviewControlRequest {
            lease_id: projection.lease_id.clone().unwrap_or_default(),
            generation: projection.generation.clone(),
            action: PreviewControl::Resume,
        })
        .filter(|request| !request.lease_id.is_empty())
}

#[cfg(test)]
mod preview_cancellation_tests {
    use super::*;

    use super::reading_component_fixtures::NativePreviewFixture;
    use crate::platform::window_preview::preview_unit_observation::Recorder;
    use std::sync::{
        Condvar,
        atomic::{AtomicU8, AtomicU64},
    };
    struct FreezeBarrier {
        entered: Mutex<bool>,
        changed: Condvar,
        released: Mutex<bool>,
        resume: Condvar,
    }
    impl FreezeBarrier {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                entered: Mutex::new(false),
                changed: Condvar::new(),
                released: Mutex::new(false),
                resume: Condvar::new(),
            })
        }
        fn wait_entered(&self) {
            let entered = self.entered.lock().unwrap();
            let (entered, timeout) = self
                .changed
                .wait_timeout_while(entered, Duration::from_secs(5), |ready| !*ready)
                .unwrap();
            assert!(*entered && !timeout.timed_out());
        }
        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.resume.notify_all();
        }
        fn block(&self) {
            *self.entered.lock().unwrap() = true;
            self.changed.notify_all();
            let mut released = self.released.lock().unwrap();
            while !*released {
                released = self.resume.wait(released).unwrap();
            }
        }
    }
    struct NativePortState {
        candidate: Mutex<Option<String>>,
        mode: AtomicU8,
        starts: AtomicU64,
        filters: AtomicU64,
        barrier: Mutex<Option<Arc<FreezeBarrier>>>,
        guard_exits: AtomicU64,
        candidate_queries: AtomicU64,
        retained: AtomicU64,
        handoffs: AtomicU64,
    }
    #[derive(Clone)]
    struct NativePort {
        state: Arc<NativePortState>,
    }
    impl NativePort {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                state: Arc::new(NativePortState {
                    candidate: Mutex::new(Some("candidate-main".into())),
                    mode: AtomicU8::new(0),
                    starts: AtomicU64::new(0),
                    filters: AtomicU64::new(0),
                    barrier: Mutex::new(None),
                    guard_exits: AtomicU64::new(0),
                    candidate_queries: AtomicU64::new(0),
                    retained: AtomicU64::new(0),
                    handoffs: AtomicU64::new(0),
                }),
            })
        }
        fn counts(&self, service: &WindowPreviewService) -> serde_json::Value {
            serde_json::json!({"collector_start":self.state.starts.load(Ordering::Acquire),"filter_start":self.state.filters.load(Ordering::Acquire),
                "picker_calls":0,"synthetic_frames":service.observed_frame_count(),"direct_ensure_calls":0,"preview_session_polls":0,"mounted_ui":0})
        }
    }
    impl CurrentPreviewNative for NativePort {
        fn current_candidate_id(&self) -> AppResult<String> {
            self.state.candidate_queries.fetch_add(1, Ordering::Release);
            self.state
                .candidate
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| AppError::new("preview_target_missing", "preview", "没有已核验窗口"))
        }
        fn freeze(&self, _stopped: &AtomicBool) -> AppResult<PreparedCurrentPreview> {
            let candidate = self.current_candidate_id()?;
            let barrier = self.state.barrier.lock().unwrap().take();
            if let Some(barrier) = barrier {
                barrier.block();
            }
            Ok(PreparedCurrentPreview {
                candidate_id: candidate.clone(),
                start: Box::new(move |service, projection| {
                    service.start_remote(
                        projection,
                        WindowPreviewService::controlled_frozen_target(&candidate),
                        false,
                    )
                }),
            })
        }
    }
    fn fixture() -> Arc<NativePreviewFixture> {
        let root = std::env::var_os("LUNA_BINDING_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Arc::new(NativePreviewFixture::new(&root).unwrap())
    }
    fn wait(mut predicate: impl FnMut() -> bool) {
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !predicate() {
            assert!(
                std::time::Instant::now() < until,
                "actual asynchronous owner did not settle"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn make_worker(
        fixture: &NativePreviewFixture,
        service: Arc<WindowPreviewService>,
        native: Arc<NativePort>,
        ownership: Arc<Mutex<PreviewOwnership>>,
        record: Arc<Recorder>,
        branch: &str,
    ) -> Arc<CurrentPreviewWorker> {
        let state = native.state.clone();
        let prepare_state = state.clone();
        service.controlled_prepare_port(Arc::new(move |_, _, target, select| {
            assert!(!select, "current owner must never open the system picker");
            match prepare_state.mode.load(Ordering::Acquire) {
                1 => {
                    return Err(AppError::new(
                        "preview_window_selection_required",
                        "preview",
                        "需要通过系统选择对应的游戏窗口",
                    ));
                }
                2 => {
                    return Err(AppError::new(
                        "preview_capture_busy",
                        "preview",
                        "受控低层队列暂满",
                    ));
                }
                _ => {}
            }
            let source = prepare_state.starts.fetch_add(1, Ordering::AcqRel) + 1;
            let filter = prepare_state.filters.fetch_add(1, Ordering::AcqRel) + 1;
            Ok(crate::platform::window_preview::ControlledNativePrepared {
                target_identity_hash: app_core::domain::content_hash(format!(
                    "{}-{source}",
                    target.candidate_id()
                )),
                capture_id: format!("controlled-filter-{filter}"),
            })
        }));
        let branch = branch.to_owned();
        Arc::new(CurrentPreviewWorker {
            runtime: fixture.runtime(),
            native,
            service,
            ownership,
            closing: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            observer: Some(Arc::new(move |kind, data| {
                let kind = if kind == "active_subscription_event"
                    && data["identity"]["session"].is_null()
                {
                    "subscription_idle"
                } else {
                    kind
                };
                record.event(&branch, kind, data);
                if kind == "guard_exit" {
                    state.guard_exits.fetch_add(1, Ordering::Release);
                }
                if kind == "session_retained" {
                    state.retained.fetch_add(1, Ordering::Release);
                }
                if kind == "session_handoff" {
                    state.handoffs.fetch_add(1, Ordering::Release);
                }
            })),
        })
    }
    fn candidate(
        service: &WindowPreviewService,
        ownership: &Arc<Mutex<PreviewOwnership>>,
        name: &str,
        candidate: &str,
    ) -> PreviewProjection {
        let p = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: candidate.into(),
                consumer: PreviewConsumer::Candidate,
                consumer_id: name.into(),
                expected_generation: None,
            })
            .unwrap();
        service
            .controlled_native_started(
                &p,
                &app_core::domain::content_hash(name),
                &format!("controlled-{name}-filter"),
                false,
            )
            .unwrap();
        publish_preview(
            &mut ownership.lock().unwrap(),
            &p,
            PreviewConsumer::Candidate,
        );
        p
    }
    fn lease_snapshot(
        service: &WindowPreviewService,
        p: &PreviewProjection,
        ownership: &Arc<Mutex<PreviewOwnership>>,
    ) -> serde_json::Value {
        service
            .observed_lease(
                p.lease_id.as_deref().unwrap(),
                ownership.lock().unwrap().epoch,
            )
            .unwrap()
    }
    fn request(fixture: &NativePreviewFixture, operation: &str) -> ReviewedAssociateRequest {
        let snap = fixture.snapshot().unwrap();
        let review = fixture
            .runtime()
            .review_association(AssociationReviewQuery {
                candidate_id: "candidate-main".into(),
                game_id: None,
                expected_profile_generation: snap.profile_generation,
                expected_active_session_id: snap
                    .session
                    .as_ref()
                    .filter(|s| matches!(s.state.as_str(), "opening" | "active" | "connected"))
                    .map(|s| s.session_id.clone()),
                prior_review_id: None,
                attribution_intent: Some(AssociationAttributionIntent::SessionOnly),
            })
            .unwrap();
        ReviewedAssociateRequest {
            operation_id: operation.into(),
            review_id: review.review_id,
        }
    }
    #[test]
    fn bj_07_active_session_preview_reconciliation() {
        tauri::async_runtime::block_on(async {
            let record = Arc::new(Recorder::new(
                "platform::session_preview_coordinator::preview_cancellation_tests::bj_07_active_session_preview_reconciliation",
            ));
            for (branch, same_target) in [
                ("resource_failure", false),
                ("resource_failure", true),
                ("target_lease_failure", false),
                ("target_lease_failure", true),
            ] {
                let fixture = fixture();
                let service = WindowPreviewService::new();
                let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
                let native = NativePort::new();
                service.controlled_retirement_port(Arc::new(|_| Ok(true)));
                let original =
                    candidate(&service, &ownership, "original-candidate", "candidate-main");
                let req = request(&fixture, "native-original-association");
                prepare_reviewed_handoff(
                    fixture.runtime(),
                    service.clone(),
                    ownership.clone(),
                    req.clone(),
                )
                .await
                .unwrap();
                let barrier = FreezeBarrier::new();
                *native.state.barrier.lock().unwrap() = Some(barrier.clone());
                let worker = make_worker(
                    &fixture,
                    service.clone(),
                    native.clone(),
                    ownership.clone(),
                    record.clone(),
                    branch,
                );
                let handle = worker.spawn().unwrap();
                let receipt = fixture.runtime().associate_reviewed(req).unwrap();
                fixture.drain_and_publish().unwrap();
                barrier.wait_entered();
                let before = fixture
                    .runtime()
                    .current_session_preview_authority()
                    .unwrap()
                    .unwrap();
                record.event(branch, "s1_active_before", authority_observation(&before));
                // The real Native observer detach invalidates only candidate ownership.
                let removed = take_consumer_previews(
                    &mut ownership.lock().unwrap(),
                    PreviewConsumer::Candidate,
                    None,
                );
                for (id, lease) in removed {
                    service
                        .control(PreviewControlRequest {
                            lease_id: id,
                            generation: lease.generation,
                            action: PreviewControl::Stop,
                        })
                        .unwrap();
                }
                assert!(
                    service
                        .consumer_update(&original.consumer_id, false)
                        .unwrap()
                        .is_none()
                );
                let next = candidate(
                    &service,
                    &ownership,
                    "new-candidate",
                    if same_target {
                        "candidate-main"
                    } else {
                        "candidate-other"
                    },
                );
                let next_before = lease_snapshot(&service, &next, &ownership);
                record.event(branch, "candidate_before", next_before.clone());
                let held = if branch == "resource_failure" {
                    fixture.resource_path(Some(
                        fixture
                            .database_path()
                            .with_file_name("missing-real-resource.bin"),
                    ));
                    None
                } else {
                    let path = fixture
                        .database_path()
                        .with_file_name("actual-held-target.lock");
                    let file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(&path)
                        .unwrap();
                    file.try_lock().unwrap();
                    fixture.target_lease_path(Some(path));
                    Some(file)
                };
                let error = fixture.connect("candidate-other").unwrap_err();
                record.event(
                    branch,
                    "s2_failed_before_stop",
                    serde_json::json!({"code":error.code}),
                );
                let after = fixture
                    .runtime()
                    .current_session_preview_authority()
                    .unwrap()
                    .unwrap();
                record.event(branch, "s1_active_after", authority_observation(&after));
                assert_eq!(
                    authority_observation(&before),
                    authority_observation(&after)
                );
                record.event(
                    branch,
                    "fresh_authority_acquired",
                    authority_observation(&after),
                );
                let callbacks = AtomicU64::new(0);
                assert!(
                    fixture
                        .runtime()
                        .with_association_preview_authority(&receipt, &mut |_, _| {
                            callbacks.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        })
                        .is_err()
                );
                barrier.release();
                let sid = before.hook_session.session_id;
                wait(|| {
                    service
                        .observed_capture(&sid, ownership.lock().unwrap().epoch)
                        .is_some()
                });
                let next_after = lease_snapshot(&service, &next, &ownership);
                assert_eq!(next_before, next_after);
                record.event(branch, "candidate_after", next_after);
                let mut counts = native.counts(&service);
                counts["stop_s1"] = serde_json::json!(fixture.stop_count());
                counts["old_handoff_callbacks"] =
                    serde_json::json!(callbacks.load(Ordering::Acquire));
                assert_eq!(fixture.stop_count(), 0);
                assert_eq!(callbacks.load(Ordering::Acquire), 0);
                record.event(branch, "call_counters", counts);
                handle.stop_and_join();
                drop(held);
                fixture.runtime().shutdown().await.unwrap();
            }
            record.output();
        });
    }
    fn mutate_authority(authority: &mut CurrentSessionPreviewAuthority, field: &str) {
        let h = &mut authority.hook_session;
        match field {
            "profileId" => authority.profile.profile_id.push_str("-stale"),
            "serverInstanceId" => authority.profile.server_instance_id.push_str("-stale"),
            "profileGeneration" => authority.profile.profile_generation += 1,
            "session_id" => h.session_id.push_str("-stale"),
            "game_id" => h.game_id = Some("stale-game".into()),
            "environment_key" => h.environment_key.push_str("-stale"),
            "target_exe_path" => h.target_exe_path.push_str(".stale"),
            "windows_pid" => h.windows_pid += 1,
            "process_created_u64" => h.process_created_u64 = "1".into(),
            "architecture" => h.architecture = "x86".into(),
            "broker_instance_id" => h.broker_instance_id.push_str("-stale"),
            "target_identity_sha256" => {
                h.target_identity_sha256 = app_core::domain::content_hash("stale-target")
            }
            "resource.snapshot_id" => h.resource.snapshot_id.push_str("-stale"),
            "resource.kind" => h.resource.kind.push_str("-stale"),
            "resource.version" => h.resource.version.push_str("-stale"),
            "resource.manifest_sha256" => {
                h.resource.manifest_sha256 = app_core::domain::content_hash("stale-resource")
            }
            "resource.contract_version" => h.resource.contract_version.push_str("-stale"),
            "resource.metadata_json" => h.resource.metadata_json = "{\"stale\":true}".into(),
            _ => panic!("unknown mutation"),
        }
    }
    #[test]
    fn bj_07_preview_authority_fence_races() {
        tauri::async_runtime::block_on(async {
            let name = "platform::session_preview_coordinator::preview_cancellation_tests::bj_07_preview_authority_fence_races";
            let record = Arc::new(Recorder::new(name));
            let fixture = fixture();
            fixture.connect("candidate-main").unwrap();
            let base = fixture
                .runtime()
                .current_session_preview_authority()
                .unwrap()
                .unwrap();
            let service = WindowPreviewService::new();
            let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
            let native = NativePort::new();
            let p = candidate(&service, &ownership, "new-candidate", "candidate-main");
            let worker = make_worker(
                &fixture,
                service.clone(),
                native.clone(),
                ownership.clone(),
                record.clone(),
                "authority_races",
            );
            for field in [
                "profileId",
                "serverInstanceId",
                "profileGeneration",
                "session_id",
                "game_id",
                "environment_key",
                "target_exe_path",
                "windows_pid",
                "process_created_u64",
                "architecture",
                "broker_instance_id",
                "target_identity_sha256",
                "resource.snapshot_id",
                "resource.kind",
                "resource.version",
                "resource.manifest_sha256",
                "resource.contract_version",
                "resource.metadata_json",
            ] {
                let mut attempted = base.clone();
                mutate_authority(&mut attempted, field);
                let before = lease_snapshot(&service, &p, &ownership);
                let callbacks = native.state.starts.load(Ordering::Acquire);
                assert!(worker.reconcile_authority(attempted.clone()).is_err());
                let after = lease_snapshot(&service, &p, &ownership);
                assert_eq!(before, after);
                let callbacks = native.state.starts.load(Ordering::Acquire) - callbacks;
                assert_eq!(callbacks, 0);
                record.event("authority_races","mutated_authority_attempt",serde_json::json!({"mutated_field":field,"current_authority":authority_observation(&base),"attempted_authority":authority_observation(&attempted),"effect_callbacks":callbacks,"before":before,"after":after}));
            }
            assert!(!worker.reconcile_authority(base).unwrap());
            assert_eq!(native.state.starts.load(Ordering::Acquire), 1);
            record.event(
                "authority_races",
                "fresh_authority_attempt",
                serde_json::json!({"effect_callbacks":native.state.starts.load(Ordering::Acquire)}),
            );
            fixture.runtime().shutdown().await.unwrap();
            record.output();
        });
    }
    async fn busy_branch(record: Arc<Recorder>, branch: &str) {
        let fixture = fixture();
        fixture.connect("candidate-main").unwrap();
        let authority = fixture
            .runtime()
            .current_session_preview_authority()
            .unwrap()
            .unwrap();
        let service = WindowPreviewService::new();
        let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
        let native = NativePort::new();
        let old = service
            .begin_resolution(PreviewStartRequest {
                candidate_id: "obsolete-target".into(),
                consumer: PreviewConsumer::Session,
                consumer_id: "obsolete-session".into(),
                expected_generation: None,
            })
            .unwrap();
        service
            .controlled_native_started(
                &old,
                &app_core::domain::content_hash("old-target"),
                "old-filter",
                false,
            )
            .unwrap();
        publish_preview(
            &mut ownership.lock().unwrap(),
            &old,
            PreviewConsumer::Session,
        );
        let next = candidate(&service, &ownership, "new-candidate", "candidate-other");
        let next_before = lease_snapshot(&service, &next, &ownership);
        let (tx, rx) = mpsc::sync_channel(1);
        let old_id = old.lease_id.clone().unwrap();
        service.controlled_retirement_port(Arc::new(move |ticket| {
            if ticket.lease_id == old_id {
                tx.try_send(ticket.clone()).map_err(|_| {
                    AppError::new(
                        "preview_capture_busy",
                        "preview",
                        "low-level release queue full",
                    )
                })?;
                Ok(false)
            } else {
                Ok(true)
            }
        }));
        let worker = make_worker(
            &fixture,
            service.clone(),
            native.clone(),
            ownership.clone(),
            record.clone(),
            branch,
        );
        let handle = worker.spawn().unwrap();
        let ticket = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        wait(|| native.state.guard_exits.load(Ordering::Acquire) > 0);
        assert!(
            service
                .observed_capture(&authority.hook_session.session_id, 0)
                .is_none()
        );
        record.event(
            branch,
            "active_authority",
            authority_observation(&authority),
        );
        assert!(service.controlled_release_ack(&ticket));
        record.event(
            branch,
            "release_ack",
            serde_json::to_value(&ticket).unwrap(),
        );
        wait(|| {
            service
                .observed_capture(
                    &authority.hook_session.session_id,
                    ownership.lock().unwrap().epoch,
                )
                .is_some()
        });
        let admitted = service
            .observed_capture(
                &authority.hook_session.session_id,
                ownership.lock().unwrap().epoch,
            )
            .unwrap();
        record.event(branch, "session_admitted", admitted);
        assert!(!service.controlled_release_ack(&ticket));
        fixture.drain_and_publish().unwrap();
        assert_eq!(next_before, lease_snapshot(&service, &next, &ownership));
        assert_eq!(native.state.starts.load(Ordering::Acquire), 1);
        record.event(branch, "call_counters", native.counts(&service));
        handle.stop_and_join();
        fixture.runtime().shutdown().await.unwrap();
    }
    #[test]
    fn bj_07_preview_reconciliation_singleflight() {
        tauri::async_runtime::block_on(async {
            let record = Arc::new(Recorder::new(
                "platform::session_preview_coordinator::preview_cancellation_tests::bj_07_preview_reconciliation_singleflight",
            ));
            busy_branch(record.clone(), "singleflight").await;
            record.output();
        });
    }
    #[test]
    fn bj_07_preview_no_implicit_picker() {
        tauri::async_runtime::block_on(async {
            let record = Arc::new(Recorder::new(
                "platform::session_preview_coordinator::preview_cancellation_tests::bj_07_preview_no_implicit_picker",
            ));
            for branch in ["no_verified_window", "first_confirmation"] {
                let fixture = fixture();
                fixture.connect("candidate-main").unwrap();
                let authority = fixture
                    .runtime()
                    .current_session_preview_authority()
                    .unwrap()
                    .unwrap();
                let service = WindowPreviewService::new();
                let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
                let native = NativePort::new();
                if branch == "no_verified_window" {
                    *native.state.candidate.lock().unwrap() = None;
                } else {
                    native.state.mode.store(1, Ordering::Release);
                }
                let worker = make_worker(
                    &fixture,
                    service.clone(),
                    native.clone(),
                    ownership.clone(),
                    record.clone(),
                    branch,
                );
                let handle = worker.spawn().unwrap();
                wait(|| {
                    if branch == "no_verified_window" {
                        native.state.candidate_queries.load(Ordering::Acquire) > 0
                    } else {
                        service
                            .consumer_update(&authority.hook_session.session_id, false)
                            .unwrap()
                            .is_some_and(|u| {
                                u.projection.error_code.as_deref()
                                    == Some("preview_window_selection_required")
                            })
                    }
                });
                record.event(
                    branch,
                    "active_authority",
                    authority_observation(&authority),
                );
                record.event(branch,"lease_after",serde_json::json!({"session_lease":service.observed_capture(&authority.hook_session.session_id,0)}));
                record.event(branch, "call_counters", native.counts(&service));
                assert_eq!(native.state.starts.load(Ordering::Acquire), 0);
                handle.stop_and_join();
                fixture.runtime().shutdown().await.unwrap();
            }
            busy_branch(record.clone(), "typed_busy").await;
            record.output();
        });
    }
    #[test]
    fn bj_07_preview_existing_healthy_and_paused() {
        tauri::async_runtime::block_on(async {
            let record = Arc::new(Recorder::new(
                "platform::session_preview_coordinator::preview_cancellation_tests::bj_07_preview_existing_healthy_and_paused",
            ));
            for (branch, paused) in [("healthy", false), ("paused", true)] {
                let fixture = fixture();
                fixture.connect("candidate-main").unwrap();
                let authority = fixture
                    .runtime()
                    .current_session_preview_authority()
                    .unwrap()
                    .unwrap();
                let service = WindowPreviewService::new();
                let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
                let native = NativePort::new();
                let projection = service
                    .begin_resolution(PreviewStartRequest {
                        candidate_id: "candidate-main".into(),
                        consumer: PreviewConsumer::Session,
                        consumer_id: authority.hook_session.session_id.clone(),
                        expected_generation: None,
                    })
                    .unwrap();
                service
                    .controlled_native_started(
                        &projection,
                        &authority.hook_session.target_identity_sha256,
                        "existing-controlled-filter",
                        paused,
                    )
                    .unwrap();
                publish_preview(
                    &mut ownership.lock().unwrap(),
                    &projection,
                    PreviewConsumer::Session,
                );
                let before = lease_snapshot(&service, &projection, &ownership);
                record.event(branch, "lease_before", before.clone());
                let worker = make_worker(
                    &fixture,
                    service.clone(),
                    native.clone(),
                    ownership.clone(),
                    record.clone(),
                    branch,
                );
                let handle = worker.spawn().unwrap();
                wait(|| native.state.retained.load(Ordering::Acquire) > 0);
                fixture.drain_and_publish().unwrap();
                let after = lease_snapshot(&service, &projection, &ownership);
                assert_eq!(before, after);
                record.event(branch, "lease_after", after);
                record.event(
                    branch,
                    "active_authority",
                    authority_observation(&authority),
                );
                record.event(branch, "call_counters", native.counts(&service));
                assert_eq!(native.state.starts.load(Ordering::Acquire), 0);
                handle.stop_and_join();
                fixture.runtime().shutdown().await.unwrap();
            }
            record.output();
        });
    }

    #[test]
    fn originating_prepared_candidate_transfers_once_without_operation_continuation() {
        tauri::async_runtime::block_on(async {
            for paused in [false, true] {
                let fixture = fixture();
                let service = WindowPreviewService::new();
                let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
                let native = NativePort::new();
                let original =
                    candidate(&service, &ownership, "original-candidate", "candidate-main");
                if paused {
                    service
                        .controlled_native_started(
                            &original,
                            &app_core::domain::content_hash("original-candidate"),
                            "controlled-original-candidate-filter",
                            true,
                        )
                        .unwrap();
                }
                let before = lease_snapshot(&service, &original, &ownership);
                let transfer_count = Arc::new(AtomicU64::new(0));
                let count = transfer_count.clone();
                service.controlled_transfer_port(Arc::new(move |_, _, _, displaced| {
                    assert!(displaced.is_empty());
                    count.fetch_add(1, Ordering::AcqRel);
                    Ok(())
                }));
                let req = request(&fixture, "native-original-handoff");
                let epoch = prepare_reviewed_handoff(
                    fixture.runtime(),
                    service.clone(),
                    ownership.clone(),
                    req.clone(),
                )
                .await
                .unwrap();
                assert_eq!(epoch, 0);
                // Native metadata is prepared, but no UI promise continuation or
                // one-shot association effect callback is executed after commit.
                fixture.runtime().associate_reviewed(req.clone()).unwrap();
                fixture.drain_and_publish().unwrap();
                let authority = fixture
                    .runtime()
                    .current_session_preview_authority()
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    authority.origin.as_ref().unwrap().operation_id,
                    req.operation_id
                );
                let record = Arc::new(Recorder::new("original-filter-handoff"));
                let worker = make_worker(
                    &fixture,
                    service.clone(),
                    native.clone(),
                    ownership.clone(),
                    record,
                    "original_handoff",
                );
                let handle = worker.clone().spawn().unwrap();
                wait(|| native.state.handoffs.load(Ordering::Acquire) == 1);
                let after = service
                    .observed_capture(
                        &authority.hook_session.session_id,
                        ownership.lock().unwrap().epoch,
                    )
                    .unwrap();
                assert_eq!(before["lease_id"], after["lease_id"]);
                assert_eq!(before["filter"], after["filter"]);
                assert_eq!(before["paused"], after["paused"]);
                assert_ne!(before["generation"], after["generation"]);
                assert_eq!(after["owner_kind"], "session");
                assert_eq!(transfer_count.load(Ordering::Acquire), 1);
                assert_eq!(native.state.starts.load(Ordering::Acquire), 0);
                // A replay cannot replace the immutable ticket with a later same
                // target candidate, and repeated reconciliation cannot retransfer.
                let later = candidate(&service, &ownership, "later-candidate", "candidate-main");
                let later_before = lease_snapshot(&service, &later, &ownership);
                prepare_reviewed_handoff(
                    fixture.runtime(),
                    service.clone(),
                    ownership.clone(),
                    req.clone(),
                )
                .await
                .unwrap();
                let replay = fixture.runtime().associate_reviewed(req).unwrap();
                assert!(replay.replayed);
                fixture.drain_and_publish().unwrap();
                assert!(!worker.reconcile().unwrap());
                assert!(native.state.retained.load(Ordering::Acquire) > 0);
                assert_eq!(later_before, lease_snapshot(&service, &later, &ownership));
                assert_eq!(transfer_count.load(Ordering::Acquire), 1);
                handle.stop_and_join();
                fixture.runtime().shutdown().await.unwrap();
            }
        });
    }
    #[test]
    fn native_unbind_or_application_close_during_offlock_freeze_blocks_start() {
        tauri::async_runtime::block_on(async {
            for closing in [false, true] {
                let fixture = fixture();
                fixture.connect("candidate-main").unwrap();
                let service = WindowPreviewService::new();
                let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
                let native = NativePort::new();
                let next = candidate(&service, &ownership, "new-candidate", "candidate-other");
                let before = lease_snapshot(&service, &next, &ownership);
                let record = Arc::new(Recorder::new("offlock-native-fence"));
                let worker = make_worker(
                    &fixture,
                    service.clone(),
                    native.clone(),
                    ownership.clone(),
                    record,
                    "freeze_race",
                );
                let barrier = FreezeBarrier::new();
                *native.state.barrier.lock().unwrap() = Some(barrier.clone());
                let attempted = worker.clone();
                let action = std::thread::spawn(move || attempted.reconcile());
                barrier.wait_entered();
                // A fresh Core read succeeds while Native verification is held,
                // proving external preparation does not retain the Core guard.
                assert!(
                    fixture
                        .runtime()
                        .current_session_preview_authority()
                        .unwrap()
                        .is_some()
                );
                if closing {
                    worker.closing.store(true, Ordering::Release);
                } else {
                    *native.state.candidate.lock().unwrap() = None;
                }
                barrier.release();
                assert!(action.join().unwrap().is_err());
                assert_eq!(native.state.starts.load(Ordering::Acquire), 0);
                assert_eq!(before, lease_snapshot(&service, &next, &ownership));
                assert!(service.session_tickets().unwrap().is_empty());
                fixture.runtime().shutdown().await.unwrap();
            }
        });
    }
    #[test]
    fn inactive_observation_cannot_retire_new_session_started_through_guarded_manual_admission() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let service = WindowPreviewService::new();
            let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
            let native = NativePort::new();
            let record = Arc::new(Recorder::new("inactive-manual-race"));
            let worker = make_worker(
                &fixture,
                service.clone(),
                native.clone(),
                ownership.clone(),
                record,
                "inactive_race",
            );
            let old = service
                .begin_resolution(PreviewStartRequest {
                    candidate_id: "old-target".into(),
                    consumer: PreviewConsumer::Session,
                    consumer_id: "old-session".into(),
                    expected_generation: None,
                })
                .unwrap();
            service
                .start_remote(
                    &old,
                    WindowPreviewService::controlled_frozen_target("old-target"),
                    false,
                )
                .unwrap();
            let original = service.session_tickets().unwrap();
            let inactive = fixture
                .runtime()
                .inactive_session_preview_authority()
                .unwrap()
                .unwrap();
            let (go_tx, go_rx) = mpsc::sync_channel(1);
            let old_worker = worker.clone();
            let observer = std::thread::spawn(move || {
                go_rx.recv().unwrap();
                old_worker.reconcile_inactive(inactive, original)
            });
            fixture.connect("candidate-main").unwrap();
            let current = fixture
                .runtime()
                .current_session_preview_authority()
                .unwrap()
                .unwrap();
            service.controlled_retirement_port(Arc::new(|_| Ok(false)));
            let old_ticket = service.session_tickets().unwrap().pop().unwrap();
            service.retire_exact(&old_ticket).unwrap();
            let request = PreviewStartRequest {
                candidate_id: "candidate-main".into(),
                consumer: PreviewConsumer::Session,
                consumer_id: current.hook_session.session_id.clone(),
                expected_generation: None,
            };
            // Both manual start and select share this production admission. A
            // fresh S2 grant cannot bypass an old Session awaiting actual ack.
            for preserve in [false, true] {
                assert_eq!(
                    admit_prepared_preview(
                        &fixture.runtime(),
                        native.as_ref(),
                        &service,
                        &ownership,
                        &request,
                        preserve,
                        0,
                        Some(&current)
                    )
                    .unwrap_err()
                    .code,
                    "preview_cleanup_pending"
                );
                assert!(
                    service
                        .current_session_projection(&request.consumer_id, &request.candidate_id)
                        .unwrap()
                        .is_none()
                );
            }
            assert!(service.controlled_release_ack(&old_ticket));
            let projection = admit_prepared_preview(
                &fixture.runtime(),
                native.as_ref(),
                &service,
                &ownership,
                &request,
                true,
                0,
                Some(&current),
            )
            .unwrap();
            fixture
                .runtime()
                .with_current_session_preview_authority(&current, &mut || {
                    service.start_remote(
                        &projection,
                        WindowPreviewService::controlled_frozen_target(&request.candidate_id),
                        false,
                    )
                })
                .unwrap();
            let before = service.observed_capture(&request.consumer_id, 0).unwrap();
            go_tx.send(()).unwrap();
            assert!(observer.join().unwrap().is_err());
            assert_eq!(
                before,
                service.observed_capture(&request.consumer_id, 0).unwrap()
            );
            assert_eq!(service.session_tickets().unwrap().len(), 1);
            fixture.runtime().shutdown().await.unwrap();
        });
    }
    #[test]
    fn queued_candidate_cleanup_retries_without_ui_even_while_application_is_closing() {
        tauri::async_runtime::block_on(async {
            for closing in [false, true] {
                let fixture = fixture();
                let service = WindowPreviewService::new();
                let ownership = Arc::new(Mutex::new(PreviewOwnership::default()));
                let native = NativePort::new();
                let record = Arc::new(Recorder::new("headless-cleanup-queue"));
                let worker = make_worker(
                    &fixture,
                    service.clone(),
                    native.clone(),
                    ownership.clone(),
                    record,
                    "headless_cleanup",
                );
                worker.closing.store(closing, Ordering::Release);
                let candidate = service
                    .begin_resolution(PreviewStartRequest {
                        candidate_id: "candidate-main".into(),
                        consumer: PreviewConsumer::Candidate,
                        consumer_id: "closed-panel".into(),
                        expected_generation: None,
                    })
                    .unwrap();
                service
                    .start_remote(
                        &candidate,
                        WindowPreviewService::controlled_frozen_target("candidate-main"),
                        false,
                    )
                    .unwrap();
                let before = service
                    .observed_lease(candidate.lease_id.as_deref().unwrap(), 0)
                    .unwrap();
                let attempts = Arc::new(AtomicU64::new(0));
                let called = attempts.clone();
                let (tx, rx) = mpsc::sync_channel(1);
                service.controlled_retirement_port(Arc::new(move |ticket| {
                    if called.fetch_add(1, Ordering::AcqRel) == 0 {
                        return Err(AppError::new(
                            "preview_capture_busy",
                            "preview",
                            "实际低层队列受控繁忙",
                        ));
                    }
                    tx.try_send(ticket.clone()).unwrap();
                    Ok(false)
                }));
                assert_eq!(
                    service
                        .control(PreviewControlRequest {
                            lease_id: candidate.lease_id.clone().unwrap(),
                            generation: candidate.generation,
                            action: PreviewControl::Stop
                        })
                        .unwrap_err()
                        .code,
                    "preview_capture_busy"
                );
                assert_eq!(
                    before,
                    service
                        .observed_lease(candidate.lease_id.as_deref().unwrap(), 0)
                        .unwrap()
                );
                // No UI timer or retry is involved: runtime.subscribe wakes the
                // same Native owner, whose cleanup step precedes closing checks.
                let handle = worker.spawn().unwrap();
                let queued = rx.recv_timeout(Duration::from_secs(3)).unwrap();
                assert_eq!(attempts.load(Ordering::Acquire), 2);
                assert!(service.controlled_release_ack(&queued));
                wait(|| {
                    service
                        .consumer_update("closed-panel", true)
                        .unwrap()
                        .is_none()
                });
                handle.stop_and_join();
                service.stop_all().unwrap();
                fixture.runtime().shutdown().await.unwrap();
            }
        });
    }

    fn owner(epoch: u64, generation: &str) -> PreviewOwnership {
        let mut owner = PreviewOwnership {
            epoch,
            ..Default::default()
        };
        owner.leases.insert(
            "preview".into(),
            PreviewLease {
                generation: generation.into(),
                consumer_id: "session".into(),
                consumer: PreviewConsumer::Session,
            },
        );
        owner
    }

    fn reviewed_projection() -> PreviewProjection {
        PreviewProjection {
            lease_id: Some("automatic-session".into()),
            generation: "session-generation".into(),
            consumer_id: "committed-session".into(),
            state: PreviewState::Starting,
            permission: PreviewPermission::NotRequested,
            target_identity_hash: None,
            attempted_at_ms: None,
            captured_at_ms: None,
            content_changed_at_ms: None,
            permission_checked_at_ms: None,
            error_code: None,
            detail: String::new(),
        }
    }
    #[test]
    fn cancelled_candidate_epoch_still_publishes_current_session_without_touching_new_candidate() {
        let mut current = owner(22, "prior-session");
        current.leases.insert(
            "candidate-b".into(),
            PreviewLease {
                generation: "candidate-b-generation".into(),
                consumer_id: "candidate-b".into(),
                consumer: PreviewConsumer::Candidate,
            },
        );
        let projection = reviewed_projection();
        let released = publish_reviewed_session_preview(&mut current, &projection, 21);
        assert_eq!(released.len(), 1);
        assert_eq!(released[0].0, "preview");
        assert_eq!(released[0].1.generation, "prior-session");
        assert_eq!(current.epoch, 22);
        assert_eq!(
            current.leases["candidate-b"].generation,
            "candidate-b-generation"
        );
        assert_eq!(
            current.leases["automatic-session"].consumer_id,
            "committed-session"
        );
        assert_eq!(
            current.leases["automatic-session"].consumer,
            PreviewConsumer::Session
        );
        assert!(
            publish_reviewed_session_preview(&mut current, &projection, 21).is_empty(),
            "reattach is idempotent"
        );
        assert_eq!(current.leases.len(), 2);
        assert_eq!(current.epoch, 22);
    }
    #[test]
    fn current_candidate_epoch_releases_only_exact_old_candidates_once() {
        let mut current = owner(21, "prior-session");
        current.leases.insert(
            "candidate-a".into(),
            PreviewLease {
                generation: "candidate-a-generation".into(),
                consumer_id: "candidate-a".into(),
                consumer: PreviewConsumer::Candidate,
            },
        );
        let removed = publish_reviewed_session_preview(&mut current, &reviewed_projection(), 21);
        assert_eq!(removed.len(), 2);
        assert!(
            removed
                .iter()
                .any(|(id, lease)| id == "candidate-a"
                    && lease.generation == "candidate-a-generation")
        );
        assert!(
            removed
                .iter()
                .any(|(id, lease)| id == "preview" && lease.generation == "prior-session")
        );
        assert_eq!(current.epoch, 22);
        assert_eq!(current.leases.len(), 1);
    }

    #[test]
    fn failed_cleanup_retains_old_ticket_without_replacing_later_owner() {
        let mut current = owner(22, "new-generation");
        let old = PreviewLease {
            generation: "old-generation".into(),
            consumer_id: "old-session".into(),
            consumer: PreviewConsumer::Session,
        };
        assert!(retain_cleanup_ticket(
            &mut current,
            "old-preview".into(),
            old.clone()
        ));
        assert_eq!(current.leases["old-preview"].generation, "old-generation");
        assert!(!retain_cleanup_ticket(&mut current, "preview".into(), old));
        assert_eq!(current.leases["preview"].generation, "new-generation");
        assert_eq!(current.epoch, 22);
    }

    #[test]
    fn preview_effect_executor_keeps_runtime_context_and_typed_join_errors() {
        tauri::async_runtime::block_on(async {
            let result = run_preview_effect(|| {
                let nested = tauri::async_runtime::spawn(async { 7 });
                Ok(tauri::async_runtime::block_on(nested).unwrap())
            })
            .await
            .unwrap();
            assert_eq!(result, 7);
            let failure =
                run_preview_effect::<()>(|| panic!("controlled preview effect join failure"))
                    .await
                    .unwrap_err();
            assert_eq!(failure.code, "application_executor");
            assert_eq!(failure.stage, "ipc");
        });
    }

    #[test]
    fn completed_unbind_cannot_cancel_a_new_association_epoch() {
        let mut current = owner(12, "new-window");
        assert!(take_cancelled_previews(&mut current, Some(11)).is_empty());
        assert_eq!(current.epoch, 12);
        assert_eq!(current.leases["preview"].generation, "new-window");
    }

    #[test]
    fn matching_unbind_invalidates_pending_starts_but_not_the_next_lease() {
        let mut current = owner(11, "old-window");
        let stopped = take_cancelled_previews(&mut current, Some(11));
        assert_eq!(stopped["preview"].generation, "old-window");
        assert!(current.leases.is_empty());
        assert_ne!(current.epoch, 11); // pending start publication must reject this epoch
        current.leases = owner(current.epoch, "later-window").leases;
        assert!(take_cancelled_previews(&mut current, Some(11)).is_empty());
        assert_eq!(current.leases["preview"].generation, "later-window");
    }
    #[test]
    fn late_association_cleanup_cannot_stop_a_new_candidate_or_session() {
        let mut current = owner(22, "session-b-generation");
        current.leases.insert(
            "candidate-b".into(),
            PreviewLease {
                generation: "candidate-b-generation".into(),
                consumer_id: "candidate-b".into(),
                consumer: PreviewConsumer::Candidate,
            },
        );
        let original = current.leases.clone();
        assert!(
            take_consumer_previews(&mut current, PreviewConsumer::Candidate, Some(21)).is_empty()
        );
        assert_eq!(current.epoch, 22);
        assert_eq!(current.leases.len(), original.len());
        assert_eq!(
            current.leases["candidate-b"].generation,
            "candidate-b-generation"
        );
        assert_eq!(current.leases["preview"].generation, "session-b-generation");
        let removed = take_consumer_previews(&mut current, PreviewConsumer::Candidate, Some(22));
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].0, "candidate-b");
        assert_eq!(current.leases.len(), 1);
        assert_eq!(current.leases["preview"].generation, "session-b-generation");
        assert_eq!(current.epoch, 23);
    }
}
