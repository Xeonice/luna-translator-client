use app_core::contracts::{AppError, AppResult, Preferences, WindowRole};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
pub fn show(app: &AppHandle, role: WindowRole) -> AppResult<()> {
    let (label, title, width, height) = match role {
        WindowRole::Main => ("main", "Luna", 1040., 720.),
        WindowRole::Settings => ("settings", "设置", 1000., 720.),
        WindowRole::Hud => ("hud", "Luna · 字幕", 660., 280.),
        WindowRole::Learning => ("learning", "查词与学习", 600., 670.),
    };
    if let Some(window) = app.get_webview_window(label) {
        window
            .unminimize()
            .and_then(|_| window.show())
            .and_then(|_| {
                if label == "hud" {
                    Ok(())
                } else {
                    window.set_focus()
                }
            })
            .map_err(|_| error())?;
        if label == "hud" {
            let state = app.state::<crate::bootstrap::DesktopState>();
            sync_hud_preferences(app, &state.runtime.snapshot()?.preferences)?;
            if let Some(coordinator) = app.try_state::<Arc<WindowCoordinator>>() {
                coordinator.restore_hud_position()?;
            }
        }
        return Ok(());
    }
    let mut builder = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html?role={label}").into()),
    )
    .title(title)
    .inner_size(width, height)
    .min_inner_size(
        match label {
            "hud" => 360.,
            "learning" => 390.,
            _ => 760.,
        },
        match label {
            "hud" => 140.,
            "learning" => 480.,
            _ => 560.,
        },
    )
    .transparent(true)
    .theme(Some(tauri::Theme::Dark));
    if label == "hud" {
        builder = builder
            .always_on_top(true)
            .decorations(false)
            .focused(false);
    }
    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .effects(
                tauri::window::EffectsBuilder::new()
                    .effect(tauri::window::Effect::HudWindow)
                    .state(tauri::window::EffectState::Active)
                    .radius(16.)
                    .build(),
            );
    }
    builder.build().map_err(|_| error())?;
    if label == "hud"
        && let Some(coordinator) = app.try_state::<Arc<WindowCoordinator>>()
    {
        coordinator.restore_hud_position()?;
    }
    if label == "hud" {
        let state = app.state::<crate::bootstrap::DesktopState>();
        sync_hud_preferences(app, &state.runtime.snapshot()?.preferences)?;
    }
    Ok(())
}
pub fn sync_hud_preferences(app: &AppHandle, preferences: &Preferences) -> AppResult<()> {
    if let Some(window) = app.get_webview_window("hud") {
        let compact = preferences
            .overrides
            .get("hud.compact")
            .copied()
            .unwrap_or(false);
        let pinned = preferences
            .overrides
            .get("hud.alwaysOnTop")
            .copied()
            .unwrap_or(true);
        let width = (preferences.hud_width_pt * if compact { 500. / 560. } else { 1. }).max(360.);
        let height = window
            .inner_size()
            .and_then(|size| {
                window
                    .scale_factor()
                    .map(|scale| size.height as f64 / scale)
            })
            .map_err(|_| error())?;
        window.set_always_on_top(pinned).map_err(|_| error())?;
        let scale = window.scale_factor().map_err(|_| error())?;
        let current_width = window.inner_size().map_err(|_| error())?.width;
        if current_width != (width * scale).round() as u32 {
            if let Some(coordinator) = app.try_state::<Arc<WindowCoordinator>>() {
                coordinator.invalidate_owner("hud", "width_or_compact_changed");
            }
            window
                .set_size(tauri::LogicalSize::new(width, height))
                .map_err(|_| error())?;
        }
    }
    Ok(())
}
fn error() -> AppError {
    AppError::new("window_unavailable", "desktop", "无法打开或恢复窗口")
}
/// Always leaves native notification / Tao callback stacks on macOS. Other
/// platforms keep conservative geometry invalidation, without a fit allowance.
pub fn defer_owner_effect(effect: impl FnOnce() + Send + 'static) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        super::macos::defer_geometry(effect)
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::thread::spawn(effect);
        Ok(())
    }
}

use super::window_geometry::{self, DisplayArea, GeometryFile, StoredGeometry};
use app_core::contracts::window_capabilities::{
    GeometryProjection, GeometryState, LogicalBounds, MaterialEffect, MaterialProjection,
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::Duration,
};

/// Physical backing pixels; x/y is AppKit's global top edge on macOS. Origins
/// are only compared within the same native instance and backing scale.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct PhysicalGeometry {
    pub epoch: u64,
    pub x: f64,
    pub y: f64,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub live_resize: u32,
}
impl PhysicalGeometry {
    fn same_bounds(self, other: Self) -> bool {
        self.epoch == other.epoch
            && self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
            && self.scale == other.scale
    }
    fn valid(self) -> bool {
        self.epoch > 0
            && self.x.is_finite()
            && self.y.is_finite()
            && self.scale.is_finite()
            && self.scale > 0.
            && self.width > 0
            && self.height > 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct OwnerGeometryStamp {
    pub epoch: u64,
    pub user_generation: u64,
    pub revision: u64,
    pub actual: PhysicalGeometry,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct OwnerInvalidation {
    pub owner: String,
    pub epoch: u64,
    pub through_generation: u64,
    pub reason: &'static str,
}
#[derive(Clone, Debug)]
pub struct OwnerGeometryDecision {
    pub invalidation: Option<OwnerInvalidation>,
    pub classification: &'static str,
}
#[path = "owner_geometry.rs"]
mod content_fit_policy;
use content_fit_policy::GeometryLifecycle;

mod geometry_trace {
    use super::*;
    #[derive(serde::Serialize)]
    pub(super) struct GeometryFact {
        pub(super) phase: &'static str,
        pub(super) reason: &'static str,
        pub(super) owner: String,
        pub(super) epoch: u64,
        pub(super) user_generation: u64,
        pub(super) operation: Option<u64>,
        pub(super) actual: Option<PhysicalGeometry>,
        pub(super) event_size: Option<[u32; 2]>,
        pub(super) card_id: Option<String>,
        pub(super) card_generation: Option<u64>,
    }
    struct QueuedFact {
        attempt: u64,
        observed_at_ms: u128,
        fact: GeometryFact,
    }
    #[derive(Default)]
    struct Counters {
        attempts: AtomicU64,
        dropped: AtomicU64,
        write_failed: AtomicU64,
        written: AtomicU64,
    }
    fn now_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default()
    }
    fn append(path: &std::path::Path, record: &serde_json::Value) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(record)?;
        if line.len() > 4096 {
            return Err(std::io::Error::other("geometry fact limit"));
        }
        line.push(b'\n');
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::metadata(path).is_ok_and(|meta| meta.len() + line.len() as u64 > 524_288) {
            let backup = path.with_extension("previous.jsonl");
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            std::fs::rename(path, backup)?;
        }
        private_open(path, true)?.write_all(&line)
    }
    fn private_open(path: &std::path::Path, append: bool) -> std::io::Result<std::fs::File> {
        let mut options = std::fs::OpenOptions::new();
        options
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .append(append);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(file)
    }
    fn exclusive(path: &std::path::Path) -> std::io::Result<std::fs::File> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lock = private_open(&path.with_extension("lock"), false)?;
        lock.try_lock().map_err(std::io::Error::other)?;
        Ok(lock)
    }
    /// All counters describe observed trace delivery, never native success.
    /// Idle health flush makes a final full-queue drop visible without waiting
    /// for a later business event. Its timer has no geometry authority.
    pub(super) struct GeometryTrace {
        sender: Mutex<Option<SyncSender<QueuedFact>>>,
        counters: Arc<Counters>,
        draining: std::sync::atomic::AtomicBool,
        unavailable: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    }
    impl GeometryTrace {
        pub(super) fn new(path: Option<PathBuf>, failure: impl Fn() + Send + 'static) -> Self {
            let counters = Arc::new(Counters::default());
            let Some(path) = path else {
                return Self {
                    sender: Mutex::new(None),
                    counters,
                    draining: std::sync::atomic::AtomicBool::new(false),
                    unavailable: Mutex::new(Some(Box::new(failure))),
                };
            };
            let (sender, receiver) = sync_channel::<QueuedFact>(128);
            let health = counters.clone();
            let run_id = format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_nanos())
                    .unwrap_or_default()
            );
            std::thread::spawn(move || {
                let _exclusive = match exclusive(&path) {
                    Ok(lock) => lock,
                    Err(_) => {
                        failure();
                        return;
                    }
                };
                let mut failure_reported = false;
                let mut observed_through = 0;
                let mut last_written_attempt = 0;
                let mut last_health = None;
                if append(&path,&serde_json::json!({"kind":"trace_begin","run_id":run_id,"pid":std::process::id(),"at_ms":now_ms(),"attempt_start":1,"exit_drain":"best_effort_not_an_exit_gate"})).is_err() {
                    health.write_failed.fetch_add(1,Ordering::Relaxed);failure();failure_reported=true;
                }
                loop {
                    let message = receiver.recv_timeout(Duration::from_secs(1));
                    let done = matches!(
                        message,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
                    );
                    if let Ok(queued) = message {
                        observed_through = queued.attempt;
                        let record = serde_json::json!({"kind":"geometry_fact","run_id":run_id,"attempt_sequence":queued.attempt,"observed_at_ms":queued.observed_at_ms,"fact":queued.fact});
                        match append(&path, &record) {
                            Ok(()) => {
                                health.written.fetch_add(1, Ordering::Relaxed);
                                last_written_attempt = queued.attempt;
                            }
                            Err(_) => {
                                health.write_failed.fetch_add(1, Ordering::Relaxed);
                                if !failure_reported {
                                    failure();
                                    failure_reported = true;
                                }
                            }
                        }
                    } else {
                        let status = (
                            health.attempts.load(Ordering::Relaxed),
                            health.dropped.load(Ordering::Relaxed),
                            health.write_failed.load(Ordering::Relaxed),
                            health.written.load(Ordering::Relaxed),
                        );
                        if last_health != Some(status) || done {
                            let record = serde_json::json!({"kind":"trace_health","run_id":run_id,"at_ms":now_ms(),"attempted":status.0,"dropped":status.1,"write_failures":status.2,"written_facts":status.3,"observed_through_attempt":observed_through,"last_written_attempt":last_written_attempt,"terminal":done,"complete":done&&status.1==0&&status.2==0&&status.0==status.3});
                            match append(&path, &record) {
                                Ok(()) => {
                                    last_health = Some(status);
                                    failure_reported = false;
                                }
                                Err(_) => {
                                    health.write_failed.fetch_add(1, Ordering::Relaxed);
                                    if !failure_reported {
                                        failure();
                                        failure_reported = true;
                                    }
                                }
                            }
                        }
                    }
                    if done {
                        break;
                    }
                }
            });
            Self {
                sender: Mutex::new(Some(sender)),
                counters,
                draining: std::sync::atomic::AtomicBool::new(false),
                unavailable: Mutex::new(None),
            }
        }
        pub(super) fn request_drain(&self) {
            // Called only after the application's real Exit event. No join,
            // disk flush or diagnostics acknowledgement can gate business quit.
            if let Ok(mut sender) = self.sender.lock() {
                self.draining.store(true, Ordering::Release);
                sender.take();
            }
        }
        pub(super) fn record(&self, fact: GeometryFact) {
            let Ok(sender) = self.sender.lock() else {
                return;
            };
            if self.draining.load(Ordering::Acquire) {
                return;
            }
            let attempt = self.counters.attempts.fetch_add(1, Ordering::Relaxed) + 1;
            let queued = QueuedFact {
                attempt,
                observed_at_ms: now_ms(),
                fact,
            };
            if sender
                .as_ref()
                .is_none_or(|sender| sender.try_send(queued).is_err())
            {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            }
            if sender.is_none()
                && let Ok(mut failure) = self.unavailable.lock()
                && let Some(failure) = failure.take()
            {
                std::thread::spawn(failure);
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn fact() -> GeometryFact {
            GeometryFact {
                phase: "native_event",
                reason: "known_no_effect",
                owner: "hud".into(),
                epoch: 1,
                user_generation: 0,
                operation: None,
                actual: None,
                event_size: None,
                card_id: None,
                card_generation: None,
            }
        }
        fn wait_until(mut condition: impl FnMut() -> bool) {
            let deadline = std::time::Instant::now() + Duration::from_secs(6);
            while !condition() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "diagnostic writer did not reach the observed condition"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        fn records(path: &std::path::Path) -> Vec<serde_json::Value> {
            std::fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        }
        #[test]
        fn a_full_or_disconnected_trace_queue_is_nonblocking_and_counts_loss() {
            let (sender, receiver) = sync_channel(1);
            let trace = GeometryTrace {
                sender: Mutex::new(Some(sender)),
                counters: Arc::new(Counters::default()),
                draining: std::sync::atomic::AtomicBool::new(false),
                unavailable: Mutex::new(None),
            };
            trace.record(fact());
            trace.record(fact());
            assert_eq!(trace.counters.attempts.load(Ordering::Relaxed), 2);
            assert_eq!(trace.counters.dropped.load(Ordering::Relaxed), 1);
            let queued = receiver.try_recv().unwrap();
            assert_eq!(queued.attempt, 1);
            assert!(queued.observed_at_ms > 0);
            drop(receiver);
            trace.record(fact());
            assert_eq!(trace.counters.dropped.load(Ordering::Relaxed), 2);
        }
        #[test]
        fn writer_enforces_one_backup_line_limit_and_reports_real_io_failure() {
            let directory = std::env::temp_dir().join(format!(
                "luna-geometry-trace-{}-{}",
                std::process::id(),
                now_ms()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("trace.jsonl");
            std::fs::write(&path, vec![b' '; 524_287]).unwrap();
            append(&path, &serde_json::json!({"phase":"native_event"})).unwrap();
            assert!(path.with_extension("previous.jsonl").is_file());
            assert!(std::fs::metadata(&path).unwrap().len() < 4096);
            assert!(append(&path, &serde_json::json!({"invalid": "x".repeat(4097)})).is_err());
            assert!(append(&directory, &serde_json::json!({"phase":"health"})).is_err());
            std::fs::remove_dir_all(directory).unwrap();
        }
        #[test]
        fn trace_has_exclusive_private_files_and_drain_never_waits_for_its_writer() {
            let directory = std::env::temp_dir().join(format!(
                "luna-geometry-exclusive-{}-{}",
                std::process::id(),
                now_ms()
            ));
            let path = directory.join("trace.jsonl");
            let lock = exclusive(&path).unwrap();
            assert!(exclusive(&path).is_err());
            drop(lock);
            let trace = GeometryTrace::new(Some(path.clone()), || {
                panic!("owned trace should be available")
            });
            trace.record(fact());
            wait_until(|| {
                records(&path)
                    .iter()
                    .any(|record| record["kind"] == "geometry_fact")
            });
            assert!(
                records(&path)
                    .iter()
                    .any(|record| record["kind"] == "trace_begin")
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            trace.request_drain();
            trace.record(fact());
            assert_eq!(trace.counters.attempts.load(Ordering::Relaxed), 1);
            wait_until(|| {
                records(&path)
                    .iter()
                    .any(|record| record["kind"] == "trace_health" && record["terminal"] == true)
            });
            drop(trace);
            std::fs::remove_dir_all(directory).unwrap();
        }
        #[test]
        fn idle_writer_reports_final_queue_loss_without_another_business_fact() {
            let directory = std::env::temp_dir().join(format!(
                "luna-geometry-tail-{}-{}",
                std::process::id(),
                now_ms()
            ));
            let path = directory.join("trace.jsonl");
            let trace = GeometryTrace::new(Some(path.clone()), || {
                panic!("owned trace path should be writable")
            });
            for _ in 0..10_000 {
                trace.record(fact());
            }
            let attempts = trace.counters.attempts.load(Ordering::Relaxed);
            let dropped = trace.counters.dropped.load(Ordering::Relaxed);
            assert_eq!(attempts, 10_000);
            assert!(dropped > 0, "bounded queue must report actual saturation");
            // No more record() calls. Only the writer's idle health transition
            // can expose the last loss and observed interval in the file.
            wait_until(|| {
                records(&path).iter().any(|record| {
                    record["kind"] == "trace_health"
                        && record["attempted"].as_u64() == Some(attempts)
                        && record["dropped"].as_u64() == Some(dropped)
                })
            });
            let health = records(&path)
                .into_iter()
                .find(|record| {
                    record["kind"] == "trace_health"
                        && record["attempted"].as_u64() == Some(attempts)
                })
                .unwrap();
            assert!(!health["run_id"].as_str().unwrap().is_empty());
            assert_eq!(health["complete"], false);
            drop(trace);
            wait_until(|| {
                records(&path)
                    .iter()
                    .any(|record| record["kind"] == "trace_health" && record["terminal"] == true)
            });
            std::fs::remove_dir_all(directory).unwrap();
        }
        #[test]
        fn persistent_io_failure_is_visible_once_per_episode_off_main_and_recovers_without_business()
         {
            let directory = std::env::temp_dir().join(format!(
                "luna-geometry-failure-{}-{}",
                std::process::id(),
                now_ms()
            ));
            let path = directory.join("trace.jsonl");
            std::fs::create_dir_all(&path).unwrap();
            let caller = std::thread::current().id();
            let (failure, receive) = std::sync::mpsc::channel();
            let trace = GeometryTrace::new(Some(path.clone()), move || {
                failure.send(std::thread::current().id()).unwrap();
            });
            trace.record(fact());
            assert_ne!(
                receive.recv_timeout(Duration::from_secs(3)).unwrap(),
                caller
            );
            wait_until(|| trace.counters.write_failed.load(Ordering::Relaxed) >= 2);
            assert!(
                receive.try_recv().is_err(),
                "persistent I/O failure should not flood diagnostics"
            );
            std::fs::remove_dir(&path).unwrap();
            wait_until(|| {
                records(&path).iter().any(|record| {
                    record["kind"] == "trace_health"
                        && record["write_failures"]
                            .as_u64()
                            .is_some_and(|count| count >= 2)
                })
            });
            // A new failure after the successful idle health write is a new
            // observable episode, and is reported through the same port.
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            trace.record(fact());
            assert_ne!(
                receive.recv_timeout(Duration::from_secs(3)).unwrap(),
                caller
            );
            std::fs::remove_dir(&path).unwrap();
            drop(trace);
            wait_until(|| {
                records(&path)
                    .iter()
                    .any(|record| record["kind"] == "trace_health" && record["terminal"] == true)
            });
            std::fs::remove_dir_all(directory).unwrap();
        }
    }
}
use geometry_trace::{GeometryFact, GeometryTrace};

type FitReply = tauri::async_runtime::Sender<AppResult<GeometryProjection>>;
struct PendingFit {
    height: f64,
    reply: FitReply,
    owner_epoch: u64,
    user_generation: u64,
}
struct GeometryStateInner {
    stored: Option<StoredGeometry>,
    projection: GeometryProjection,
    revision: u64,
    saved_revision: u64,
    invalid_stored: bool,
    worker_running: bool,
}
/// Owns only native window effects and platform geometry, never product/session state.
pub struct WindowCoordinator {
    app: AppHandle,
    file: GeometryFile,
    geometry: Mutex<GeometryStateInner>,
    lifecycle: Mutex<GeometryLifecycle>,
    pending_fit: Mutex<Option<PendingFit>>,
    trace: GeometryTrace,
}
impl WindowCoordinator {
    pub fn request_trace_drain(&self) {
        self.trace.request_drain();
    }
    pub fn new(app: AppHandle, data_dir: PathBuf) -> Arc<Self> {
        #[cfg(target_os = "macos")]
        super::macos::initialize_geometry(app.clone());
        let trace_app = app.clone();
        let trace = GeometryTrace::new(
            app.path()
                .app_log_dir()
                .ok()
                .map(|path| path.join("geometry-lifecycle.jsonl")),
            move || {
                // Only a genuine diagnostics-delivery failure enters the existing
                // failure port, on the writer thread, once per failure episode.
                // Setup may not have managed DesktopState yet, and a full disk
                // can reject both JSONL and repository diagnostics. Keep one
                // fixed, content-free stderr fact as the independent fallback.
                eprintln!(
                    "luna_geometry_trace_write_failed: native geometry evidence may be incomplete"
                );
                if let Some(state) = trace_app.try_state::<crate::bootstrap::DesktopState>() {
                    state.runtime.report_native_effect_failure(AppError::new(
                        "geometry_trace_write_failed",
                        "desktop",
                        "原生窗口诊断记录暂不可写；几何证据可能不完整",
                    ));
                }
            },
        );
        let file = GeometryFile::new(&data_dir);
        let loaded = file.load();
        let invalid_stored = loaded.is_err();
        let error_code = loaded.as_ref().err().cloned();
        Arc::new(Self {
            app,
            file,
            lifecycle: Mutex::new(GeometryLifecycle::default()),
            pending_fit: Mutex::new(None),
            trace,
            geometry: Mutex::new(GeometryStateInner {
                stored: loaded.ok().flatten(),
                revision: 0,
                saved_revision: 0,
                invalid_stored,
                worker_running: false,
                projection: GeometryProjection {
                    state: GeometryState::Unavailable,
                    logical_bounds: None,
                    monitor_id: None,
                    scale_factor: None,
                    error_code,
                },
            }),
        })
    }
    fn read_owner(&self, window: &tauri::WebviewWindow) -> AppResult<PhysicalGeometry> {
        #[cfg(target_os = "macos")]
        {
            super::macos::owner_geometry(window)
        }
        #[cfg(not(target_os = "macos"))]
        {
            static EPOCH: AtomicU64 = AtomicU64::new(1);
            let epoch = self
                .lifecycle
                .lock()
                .map_err(|_| error())?
                .owners
                .get(window.label())
                .filter(|owner| !owner.closed)
                .map(|owner| owner.stamp.epoch)
                .unwrap_or_else(|| EPOCH.fetch_add(1, Ordering::Relaxed));
            let position = window.outer_position().map_err(|_| error())?;
            let size = window.inner_size().map_err(|_| error())?;
            Ok(PhysicalGeometry {
                epoch,
                x: position.x as f64,
                y: position.y as f64,
                width: size.width,
                height: size.height,
                scale: window.scale_factor().map_err(|_| error())?,
                live_resize: 0,
            })
        }
    }
    pub fn capture_owner(&self, window: &tauri::WebviewWindow) -> AppResult<OwnerGeometryStamp> {
        self.capture_owner_snapshot(window, false)
    }
    fn capture_owner_snapshot(
        &self,
        window: &tauri::WebviewWindow,
        allow_live_resize: bool,
    ) -> AppResult<OwnerGeometryStamp> {
        let version = self
            .lifecycle
            .lock()
            .map_err(|_| error())?
            .read_version(window.label());
        let actual = self.read_owner(window)?;
        let (stamp, decision, resizing) = {
            let mut lifecycle = self.lifecycle.lock().map_err(|_| error())?;
            let decision = lifecycle.accept_read(window.label(), version, actual)?;
            let resizing = actual.live_resize != 0
                || lifecycle
                    .owners
                    .get(window.label())
                    .is_some_and(|state| state.live_resize);
            let mut stamp = lifecycle
                .owners
                .get(window.label())
                .ok_or_else(error)?
                .stamp;
            stamp.actual = actual;
            (stamp, decision, resizing)
        };
        self.dispatch_invalidation(decision.invalidation);
        if !allow_live_resize && resizing {
            return Err(AppError::new(
                "word_card_owner_resizing",
                "dictionary",
                "原文窗口正在调整，请松开鼠标后重新选词",
            ));
        }
        Ok(stamp)
    }
    pub fn validate_owner(
        &self,
        window: &tauri::WebviewWindow,
        opening: OwnerGeometryStamp,
    ) -> AppResult<()> {
        let current = self.capture_owner(window)?;
        if current.epoch != opening.epoch || current.user_generation != opening.user_generation {
            return Err(AppError::new(
                "word_card_owner_changed",
                "dictionary",
                "原文窗口已调整，请重新选择词语",
            ));
        }
        Ok(())
    }
    fn dispatch_invalidation(&self, invalidation: Option<OwnerInvalidation>) {
        if let Some(invalidation) = invalidation
            && let Some(state) = self.app.try_state::<crate::bootstrap::DesktopState>()
        {
            state.word_cards.owner_invalidated(&self.app, &invalidation);
        }
    }
    fn invalidate_owner(&self, owner: &str, reason: &'static str) {
        let invalidation = self
            .lifecycle
            .lock()
            .ok()
            .and_then(|mut life| life.invalidate(owner, reason));
        self.dispatch_invalidation(invalidation);
    }
    fn fact(
        &self,
        owner: &str,
        phase: &'static str,
        reason: &'static str,
        operation: Option<u64>,
        actual: Option<PhysicalGeometry>,
        event_size: Option<[u32; 2]>,
    ) {
        let stamp = self
            .lifecycle
            .lock()
            .ok()
            .and_then(|life| life.owners.get(owner).map(|state| state.stamp));
        self.trace.record(GeometryFact {
            phase,
            reason,
            owner: owner.into(),
            epoch: actual
                .map(|value| value.epoch)
                .or(stamp.map(|value| value.epoch))
                .unwrap_or_default(),
            user_generation: stamp.map(|value| value.user_generation).unwrap_or_default(),
            operation,
            actual,
            event_size,
            card_id: None,
            card_generation: None,
        });
    }
    pub fn card_fact(
        &self,
        phase: &'static str,
        reason: &'static str,
        owner: &str,
        stamp: OwnerGeometryStamp,
        card_id: &str,
        generation: u64,
    ) {
        self.trace.record(GeometryFact {
            phase,
            reason,
            owner: owner.into(),
            epoch: stamp.epoch,
            user_generation: stamp.user_generation,
            operation: None,
            actual: Some(stamp.actual),
            event_size: None,
            card_id: Some(card_id.into()),
            card_generation: Some(generation),
        });
    }
    /// Called on an AppKit notification stack: only bounded state changes and
    /// deferred effects. No Tauri/AppKit mutation or disk write under a lock.
    #[cfg(target_os = "macos")]
    pub fn native_geometry_edge(self: &Arc<Self>, epoch: u64, edge: i32) {
        let (owner, invalidation) = {
            let Ok(mut lifecycle) = self.lifecycle.lock() else {
                return;
            };
            let Some(edge) = lifecycle.native_edge(epoch, edge) else {
                return;
            };
            edge
        };
        self.fact(
            &owner,
            "native_edge",
            match edge {
                1 => "live_resize_begin",
                2 => "live_resize_end",
                _ => "owner_closed",
            },
            None,
            None,
            None,
        );
        self.dispatch_invalidation(invalidation);
        if owner == "hud" && edge != 1 {
            let pending = self
                .pending_fit
                .lock()
                .ok()
                .and_then(|mut pending| pending.take());
            if let Some(pending) = pending {
                if edge == 3 {
                    let _ = pending.reply.try_send(Err(error()));
                } else {
                    let coordinator = self.clone();
                    let _ = defer_owner_effect(move || coordinator.perform_fit(pending));
                }
            }
        }
    }
    pub fn geometry_projection(&self) -> GeometryProjection {
        self.geometry
            .lock()
            .map(|g| g.projection.clone())
            .unwrap_or(GeometryProjection {
                state: GeometryState::NotSaved,
                logical_bounds: None,
                monitor_id: None,
                scale_factor: None,
                error_code: Some("geometry_state".into()),
            })
    }
    fn displays(&self) -> AppResult<Vec<DisplayArea>> {
        #[cfg(target_os = "macos")]
        {
            super::macos::screens()
        }
        #[cfg(not(target_os = "macos"))]
        {
            let monitors = self.app.available_monitors().map_err(|_| error())?;
            Ok(monitors
                .into_iter()
                .map(|m| {
                    let scale = m.scale_factor();
                    let a = m.work_area();
                    DisplayArea {
                        id: format!(
                            "{}:{:?}",
                            m.name().map(String::as_str).unwrap_or("monitor"),
                            m.position()
                        ),
                        scale_factor: scale,
                        work_area: LogicalBounds {
                            x: a.position.x as f64 / scale,
                            y: a.position.y as f64 / scale,
                            width: a.size.width as f64 / scale,
                            height: a.size.height as f64 / scale,
                        },
                    }
                })
                .collect())
        }
    }
    fn actual(&self) -> AppResult<LogicalBounds> {
        let window = self.app.get_webview_window("hud").ok_or_else(error)?;
        let scale = window.scale_factor().map_err(|_| error())?;
        let p = window.outer_position().map_err(|_| error())?;
        let s = window.inner_size().map_err(|_| error())?;
        Ok(LogicalBounds {
            x: p.x as f64 / scale,
            y: p.y as f64 / scale,
            width: s.width as f64 / scale,
            height: s.height as f64 / scale,
        })
    }
    fn apply(&self, geometry: &StoredGeometry) -> AppResult<()> {
        let window = self.app.get_webview_window("hud").ok_or_else(error)?;
        window
            .set_size(tauri::LogicalSize::new(
                geometry.bounds.width,
                geometry.bounds.height,
            ))
            .and_then(|_| {
                window.set_position(tauri::LogicalPosition::new(
                    geometry.bounds.x,
                    geometry.bounds.y,
                ))
            })
            .map_err(|_| error())
    }
    pub fn restore_hud_position(&self) -> AppResult<GeometryProjection> {
        self.invalidate_owner("hud", "restore_position");
        let current = self.actual()?;
        let saved = self.geometry.lock().map_err(|_| error())?.stored.clone();
        let desired = saved
            .as_ref()
            .map(|g| LogicalBounds {
                width: current.width,
                ..g.bounds.clone()
            })
            .unwrap_or(current);
        let geometry = window_geometry::constrain(
            &desired,
            saved.as_ref().map(|g| g.monitor_id.as_str()),
            &self.displays()?,
        )
        .ok_or_else(error)?;
        self.apply(&geometry)?;
        let mut state = self.geometry.lock().map_err(|_| error())?;
        state.projection =
            geometry.projection(GeometryState::Restored, state.projection.error_code.clone());
        state.stored = Some(geometry);
        Ok(state.projection.clone())
    }
    pub fn begin_hud_drag(&self, caller_label: &str) -> AppResult<()> {
        if caller_label != "hud" {
            return Err(AppError::new(
                "window_role_denied",
                "desktop",
                "仅字幕窗口可开始拖动",
            ));
        }
        self.invalidate_owner("hud", "begin_user_drag");
        self.app
            .get_webview_window("hud")
            .ok_or_else(error)?
            .start_dragging()
            .map_err(|_| error())
    }
    pub async fn fit_hud_content(
        self: &Arc<Self>,
        caller_label: &str,
        height: f64,
    ) -> AppResult<GeometryProjection> {
        if caller_label != "hud" {
            return Err(AppError::new(
                "window_role_denied",
                "desktop",
                "仅字幕窗口可调整正文高度",
            ));
        }
        if !height.is_finite() || !(0. ..=1_000_000.).contains(&height) {
            return Err(AppError::new(
                "hud_extent_invalid",
                "desktop",
                "字幕正文高度无效",
            ));
        }
        let (reply, mut completion) = tauri::async_runtime::channel(1);
        let window = self.app.get_webview_window("hud").ok_or_else(error)?;
        let opening = self.capture_owner_snapshot(&window, true)?;
        let coordinator = self.clone();
        defer_owner_effect(move || {
            coordinator.perform_fit(PendingFit {
                height,
                reply,
                owner_epoch: opening.epoch,
                user_generation: opening.user_generation,
            })
        })?;
        completion.recv().await.unwrap_or_else(|| Err(error()))
    }
    fn perform_fit(self: &Arc<Self>, request: PendingFit) {
        let result = (|| -> AppResult<Option<GeometryProjection>> {
            let window = self.app.get_webview_window("hud").ok_or_else(error)?;
            let native = self.read_owner(&window)?;
            if native.epoch != request.owner_epoch {
                return Err(error());
            }
            {
                let mut lifecycle = self.lifecycle.lock().map_err(|_| error())?;
                lifecycle.observe("hud", native)?;
                if lifecycle
                    .owners
                    .get("hud")
                    .is_none_or(|state| state.stamp.user_generation != request.user_generation)
                {
                    return Err(error());
                }
                if native.live_resize != 0
                    || lifecycle
                        .owners
                        .get("hud")
                        .is_some_and(|state| state.live_resize)
                {
                    drop(lifecycle);
                    let mut pending = self.pending_fit.lock().map_err(|_| error())?;
                    let previous = pending.replace(PendingFit {
                        height: request.height,
                        reply: request.reply.clone(),
                        owner_epoch: request.owner_epoch,
                        user_generation: request.user_generation,
                    });
                    if let Some(previous) = previous {
                        let _ = previous.reply.try_send(Err(AppError::new(
                            "hud_fit_superseded",
                            "desktop",
                            "较新的字幕高度已替代本次调整",
                        )));
                    }
                    return Ok(None);
                }
            }
            self.fit_now(&window, native, request.height).map(Some)
        })();
        match result {
            Ok(None) => {}
            Ok(Some(projection)) => {
                let _ = request.reply.try_send(Ok(projection));
            }
            Err(error) => {
                self.fact("hud", "fit_failed", "native_fit_failed", None, None, None);
                let _ = request.reply.try_send(Err(error));
            }
        }
    }
    fn fit_now(
        self: &Arc<Self>,
        window: &tauri::WebviewWindow,
        native: PhysicalGeometry,
        height: f64,
    ) -> AppResult<GeometryProjection> {
        let decision =
            self.lifecycle
                .lock()
                .map_err(|_| error())?
                .classify("hud", Some(native), None);
        self.dispatch_invalidation(decision.invalidation);
        let actual = self.actual()?;
        let displays = self.displays()?;
        let current = window_geometry::constrain(&actual, None, &displays).ok_or_else(error)?;
        let available = (current.work_area.y + current.work_area.height - actual.y)
            .min(current.work_area.height)
            .min(366.);
        if available < 140. {
            return Err(AppError::new(
                "hud_work_area_unavailable",
                "desktop",
                "当前位置没有足够的可见空间，请移动或恢复字幕位置",
            ));
        }
        let height = height.clamp(140., available);
        let physical_height = (height * native.scale).round() as u32;
        let reservation =
            self.lifecycle
                .lock()
                .map_err(|_| error())?
                .reserve("hud", native, physical_height);
        let operation = match reservation {
            Ok(operation) => operation,
            Err(error) => {
                self.invalidate_owner("hud", "fit_reservation_failed");
                return Err(error);
            }
        };
        if let Some(operation) = operation {
            self.fact(
                "hud",
                "fit_reserved",
                "content_fit",
                Some(operation.sequence),
                Some(native),
                None,
            );
            #[cfg(target_os = "macos")]
            let applied = super::macos::apply_hud_content_fit(window, &native, physical_height);
            #[cfg(not(target_os = "macos"))]
            let applied: AppResult<PhysicalGeometry> = {
                // No native intent/completion bridge on this platform: do not
                // grant a preservation allowance based on a queued receipt.
                let _ = window.set_size(tauri::PhysicalSize::new(native.width, physical_height));
                Err(error())
            };
            let after = applied
                .as_ref()
                .ok()
                .copied()
                .or_else(|| self.read_owner(window).ok());
            let (settled, invalidation) = {
                let mut lifecycle = self.lifecycle.lock().map_err(|_| error())?;
                let settled = applied.is_ok() && lifecycle.settle("hud", operation, after);
                let still_owned = lifecycle.owners.get("hud").is_some_and(|state| {
                    state.stamp.epoch == operation.before.epoch
                        && state.stamp.user_generation == operation.before.user_generation
                });
                let invalidation = if !settled && still_owned {
                    lifecycle.invalidate("hud", "fit_apply_or_readback_failed")
                } else {
                    None
                };
                (settled, invalidation)
            };
            self.dispatch_invalidation(invalidation);
            self.fact(
                "hud",
                "fit_completed",
                if settled {
                    "confirmed_content_fit"
                } else {
                    "fit_rejected"
                },
                Some(operation.sequence),
                after,
                None,
            );
            if !settled {
                return Err(applied.err().unwrap_or_else(error));
            }
        } else {
            self.fact(
                "hud",
                "fit_noop",
                "physical_size_unchanged",
                None,
                Some(native),
                None,
            );
        }
        // Only read native bounds after resize; no frontend x/y or arbitrary width is accepted.
        let actual = self.actual()?;
        let saved = StoredGeometry {
            schema_version: 1,
            bounds: actual,
            monitor_id: current.monitor_id,
            work_area: current.work_area,
            scale_factor: current.scale_factor,
        };
        self.schedule_geometry_save(saved);
        Ok(self.geometry_projection())
    }
    pub fn reset_hud_position(&self) -> AppResult<GeometryProjection> {
        self.invalidate_owner("hud", "reset_position");
        let displays = self.displays()?;
        let display = displays.first().ok_or_else(error)?;
        let actual = self.actual()?;
        let area = &display.work_area;
        let desired = LogicalBounds {
            x: area.x + (area.width - actual.width) / 2.,
            y: area.y + area.height - actual.height - 64.,
            ..actual
        };
        let geometry =
            window_geometry::constrain(&desired, Some(&display.id), &displays).ok_or_else(error)?;
        if self.geometry.lock().map_err(|_| error())?.invalid_stored {
            self.file.preserve_invalid().map_err(|_| {
                AppError::new(
                    "geometry_backup_failed",
                    "desktop",
                    "旧位置配置已保留，但无法保存恢复位置",
                )
            })?;
        }
        self.apply(&geometry)?;
        {
            let mut state = self.geometry.lock().map_err(|_| error())?;
            state.invalid_stored = false;
            state.revision = state.revision.saturating_add(1);
            state.stored = Some(geometry.clone());
            state.projection = geometry.projection(GeometryState::Moved, None);
        }
        self.flush()?;
        Ok(self.geometry_projection())
    }
    pub fn on_window_event(
        self: &Arc<Self>,
        window: &tauri::Window,
        event: &tauri::WindowEvent,
    ) -> OwnerGeometryDecision {
        let relevant = matches!(
            event,
            tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. }
                | tauri::WindowEvent::CloseRequested { .. }
                | tauri::WindowEvent::Destroyed
        );
        if !relevant || window.label().starts_with("word-card-") {
            return OwnerGeometryDecision {
                invalidation: None,
                classification: "irrelevant",
            };
        }
        #[cfg(target_os = "macos")]
        let native = super::macos::event_owner_geometry(window).ok();
        #[cfg(not(target_os = "macos"))]
        let native = self
            .app
            .get_webview_window(window.label())
            .and_then(|window| self.read_owner(&window).ok());
        let forced = match event {
            tauri::WindowEvent::ScaleFactorChanged { .. } => Some("owner_scale_changed"),
            tauri::WindowEvent::CloseRequested { .. } => Some("owner_close_requested"),
            tauri::WindowEvent::Destroyed => Some("owner_destroyed"),
            _ if window.label() != "hud" => Some("owner_geometry_changed"),
            _ => None,
        };
        let unresolved_terminal = cfg!(target_os = "macos")
            && native.is_none()
            && matches!(
                event,
                tauri::WindowEvent::Destroyed | tauri::WindowEvent::CloseRequested { .. }
            );
        let decision = if unresolved_terminal {
            OwnerGeometryDecision {
                invalidation: None,
                classification: "terminal_event_without_instance",
            }
        } else if let Ok(mut lifecycle) = self.lifecycle.lock() {
            let decision = lifecycle.classify(window.label(), native, forced);
            if matches!(event, tauri::WindowEvent::Destroyed)
                && native.is_some_and(|actual| {
                    lifecycle
                        .owners
                        .get(window.label())
                        .is_some_and(|owner| owner.stamp.epoch == actual.epoch)
                })
                && let Some(state) = lifecycle.owners.get_mut(window.label())
            {
                state.closed = true;
            }
            decision
        } else {
            OwnerGeometryDecision {
                invalidation: None,
                classification: "geometry_state_failed",
            }
        };
        self.fact(
            window.label(),
            "native_event",
            decision.classification,
            None,
            native,
            match event {
                tauri::WindowEvent::Resized(size) => Some([size.width, size.height]),
                _ => None,
            },
        );
        if decision.classification != "known_stale_window"
            && !unresolved_terminal
            && matches!(
                event,
                tauri::WindowEvent::Destroyed | tauri::WindowEvent::CloseRequested { .. }
            )
            && window.label() == "hud"
            && let Some(pending) = self
                .pending_fit
                .lock()
                .ok()
                .and_then(|mut pending| pending.take())
        {
            let _ = pending.reply.try_send(Err(error()));
        }
        if window.label() != "hud" {
            return decision;
        }
        let Ok(actual) = self.actual() else {
            return decision;
        };
        let Ok(displays) = self.displays() else {
            return decision;
        };
        let Some(geometry) = window_geometry::constrain(&actual, None, &displays) else {
            return decision;
        };
        self.schedule_geometry_save(geometry);
        decision
    }
    fn schedule_geometry_save(self: &Arc<Self>, geometry: StoredGeometry) {
        let spawn_worker = {
            let Ok(mut state) = self.geometry.lock() else {
                return;
            };
            state.revision = state.revision.saturating_add(1);
            state.projection = geometry.projection(
                GeometryState::Moved,
                if state.invalid_stored {
                    Some("geometry_invalid".into())
                } else {
                    None
                },
            );
            state.stored = Some(geometry);
            let start = !state.worker_running;
            state.worker_running = true;
            start
        };
        if !spawn_worker {
            return;
        }
        let this = self.clone();
        std::thread::spawn(move || {
            loop {
                let revision = match this.geometry.lock() {
                    Ok(state) => state.revision,
                    Err(_) => return,
                };
                std::thread::sleep(Duration::from_millis(350));
                if this
                    .geometry
                    .lock()
                    .map(|s| s.revision != revision)
                    .unwrap_or(true)
                {
                    continue;
                }
                // Re-enumerating after a native move also handles removed/changed displays.
                if let (Ok(current), Ok(displays)) = (this.actual(), this.displays())
                    && let Some(clamped) = window_geometry::constrain(&current, None, &displays)
                    && clamped.bounds != current
                {
                    let _ = this.apply(&clamped);
                }
                let _ = this.flush();
                let Ok(mut state) = this.geometry.lock() else {
                    return;
                };
                if state.revision == revision {
                    state.worker_running = false;
                    break;
                }
            }
        });
    }
    pub fn flush(&self) -> AppResult<()> {
        let mut state = self.geometry.lock().map_err(|_| error())?;
        if state.invalid_stored {
            state.projection.state = GeometryState::NotSaved;
            state.projection.error_code = Some("geometry_invalid".into());
            return Err(AppError::new(
                "geometry_invalid",
                "desktop",
                "原位置配置无效，已保留；请用恢复位置重建",
            ));
        }
        if state.revision == state.saved_revision {
            return Ok(());
        }
        let Some(value) = state.stored.clone() else {
            return Ok(());
        };
        match self.file.save(&value) {
            Ok(()) => {
                state.saved_revision = state.revision;
                state.projection.error_code = None;
                Ok(())
            }
            Err(code) => {
                state.projection.state = GeometryState::NotSaved;
                state.projection.error_code = Some(code.clone());
                Err(AppError::new(
                    &code,
                    "desktop",
                    "本次位置未保存；窗口与游戏取文仍可继续",
                ))
            }
        }
    }
}
pub fn material_projection(app: &AppHandle, preferences: &Preferences) -> MaterialProjection {
    #[cfg(target_os = "macos")]
    let system_reduced = super::macos::reduce_transparency();
    #[cfg(not(target_os = "macos"))]
    let system_reduced = false;
    let reduced = system_reduced || preferences.transparency_mode == "reduced";
    let mut native = false;
    let mut applied = false;
    if let Some(window) = app.get_webview_window("hud") {
        #[cfg(target_os = "macos")]
        {
            if let Ok(active) = super::macos::set_hud_material(&window, reduced) {
                applied = true;
                native = active;
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
        }
    }
    MaterialProjection {
        requested: preferences.transparency_mode.clone(),
        effective: if native {
            MaterialEffect::NativeMaterial
        } else {
            MaterialEffect::OpaqueFallback
        },
        system_reduce_transparency: system_reduced,
        effective_reduce_transparency: !native,
        error_code: if !reduced && !applied {
            Some("native_material_unavailable".into())
        } else {
            None
        },
    }
}
