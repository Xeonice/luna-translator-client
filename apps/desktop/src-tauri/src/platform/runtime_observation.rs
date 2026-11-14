//! Passive bounded metadata, with no business handles or control authority.
//! Guard acceptance, service commit and server delivery never claim UI render.
use app_core::contracts::window_capabilities::{
    PreviewControl, PreviewFrame, PreviewPermission, PreviewProjection, PreviewState, PreviewUpdate,
};
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const QUEUE_LIMIT: usize = 128;
const RECORD_LIMIT: usize = 4096;
const SEGMENT_LIMIT: u64 = 524_288;
const HEALTH_INTERVAL: Duration = Duration::from_secs(1);

/// Reject arbitrary text and oversize input before copying it, even for
/// untrusted collector headers rejected by the guard.
#[derive(Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct Id(String);
impl Id {
    pub(crate) fn new(value: &str) -> Option<Self> {
        (!value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b)))
        .then(|| Self(value.to_owned()))
    }
}
fn optional_id(value: Option<&str>) -> Option<Option<Id>> {
    value
        .map(Id::new)
        .map_or(Some(None), |value| value.map(Some))
}
#[derive(Serialize)]
pub(crate) struct ProjectionMeta {
    lease_id: Option<Id>,
    generation: Id,
    consumer_id: Option<Id>,
    state: PreviewState,
    permission: PreviewPermission,
    target_identity_hash: Option<Id>,
    captured_at_ms: Option<Id>,
    content_changed_at_ms: Option<Id>,
}
impl ProjectionMeta {
    fn new(p: &PreviewProjection) -> Option<Self> {
        Some(Self {
            lease_id: optional_id(p.lease_id.as_deref())?,
            generation: Id::new(&p.generation)?,
            consumer_id: optional_id(
                (!p.consumer_id.is_empty()).then_some(p.consumer_id.as_str()),
            )?,
            state: p.state.clone(),
            permission: p.permission.clone(),
            target_identity_hash: optional_id(p.target_identity_hash.as_deref())?,
            captured_at_ms: optional_id(p.captured_at_ms.as_deref())?,
            content_changed_at_ms: optional_id(p.content_changed_at_ms.as_deref())?,
        })
    }
}
#[derive(Serialize)]
pub(crate) struct FrameMeta {
    lease_id: Id,
    generation: Id,
    service_sequence: Id,
    target_identity_hash: Id,
    captured_at_ms: Id,
    content_hash: Id,
    width: u32,
    height: u32,
    encoded_len: usize,
}
impl FrameMeta {
    fn new(f: &PreviewFrame) -> Option<Self> {
        Some(Self {
            lease_id: Id::new(&f.lease_id)?,
            generation: Id::new(&f.generation)?,
            service_sequence: Id::new(&f.sequence)?,
            target_identity_hash: Id::new(&f.target_identity_hash)?,
            captured_at_ms: Id::new(&f.captured_at_ms)?,
            content_hash: Id::new(&f.content_hash)?,
            width: f.width,
            height: f.height,
            encoded_len: f.payload.len(),
        })
    }
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionStage {
    ServiceCommitted,
    ServiceStatus,
    ControlResult,
    ServerDelivered,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerStage {
    Spawned,
    HandshakeReady,
    Reaped,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Rejection {
    MissingLease,
    StopRequested,
    GenerationOrIdentity,
    SequenceExhausted,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Fact {
    CollectorFrame {
        accepted: bool,
        source_id: Id,
        source_epoch: Id,
        frame_epoch: Id,
        capture_id: Id,
        worker_sequence: u64,
        target_candidate_id: Id,
        host_pid: u32,
        host_start: Id,
        window_id: u32,
        captured_at_ms: u64,
        content_hash: Id,
        width: u32,
        height: u32,
        encoded_len: usize,
    },
    RemoteFrameAdmission {
        accepted: bool,
        lease_id: Id,
        generation: Id,
        capture_id: Id,
        worker_sequence: u64,
        target_candidate_id: Id,
        host_pid: u32,
        host_start: Id,
        window_id: u32,
        captured_at_ms: u64,
    },
    Worker {
        stage: WorkerStage,
        pid: u32,
        instance: Option<Id>,
    },
    #[cfg(target_os = "macos")]
    CaptureRecovery {
        source_id: Id,
        source_epoch: Id,
        frame_epoch: Id,
        capture_id: Option<Id>,
        target_candidate_id: Id,
        diagnostic: Box<super::macos::PreviewDiagnostic>,
    },
    SourceReleased {
        source_id: Id,
        source_epoch: Id,
    },
    Projection {
        stage: ProjectionStage,
        projection: ProjectionMeta,
        frame: Option<FrameMeta>,
        action: Option<PreviewControl>,
        caller_role: Option<Id>,
        requested_lease: Option<Id>,
        requested_generation: Option<Id>,
        requested_after_sequence: Option<Id>,
    },
    ServiceReleased {
        lease_id: Id,
        removed: bool,
    },
    ServiceFrameRejected {
        lease_id: Id,
        generation: Id,
        target_identity_hash: Id,
        reason: Rejection,
    },
    WordCardSnapshot {
        card_id: Id,
        unit_id: Id,
        text_revision: u32,
        token_index: u32,
        source_hash: Id,
    },
}
/// A bounded observation signature, never an authority or retry input. Raw
/// Native operation/sample counters do not make each polling tick a new fact.
#[cfg(target_os = "macos")]
pub(crate) fn capture_recovery_key(value: &super::macos::PreviewDiagnostic) -> Option<String> {
    let value = value.clone().checked()?;
    if value.geometry.is_none()
        && value.native_geometry.is_none()
        && value.failure_kind.is_none()
        && value.post_proof.is_none()
        && value.recovery.is_none()
    {
        return None;
    }
    let geometry_key = |geometry: &super::macos::PreviewGeometrySnapshot| {
        let sc_pixel = |point: Option<f64>| {
            let value = (point? * geometry.point_pixel_scale?).ceil();
            (value.is_finite() && value >= 1.0 && value < 18_446_744_073_709_551_616.0)
                .then_some(value as u64)
        };
        serde_json::json!({
            "signature": geometry.signature(),
            "reason": geometry.reason,
            "sc_query": geometry.sc_window_frame_query,
            "sc_width_pixels": sc_pixel(geometry.sc_window_width_points),
            "sc_height_pixels": sc_pixel(geometry.sc_window_height_points),
        })
    };
    let recovery = value.recovery.as_ref().map(|recovery| {
        let mut fields = serde_json::to_value(recovery).ok()?;
        let object = fields.as_object_mut()?;
        // Sample ticks and a diagnostic generation do not by themselves cause
        // logging; canonical dimensions/state/quota changes already do.
        object.remove("samples");
        object.remove("generation");
        Some(fields)
    });
    serde_json::to_string(&serde_json::json!({
        "stage": value.stage,
        "geometry": value.geometry.as_ref().map(geometry_key),
        "native_geometry": value.native_geometry.as_ref().map(geometry_key),
        "failure_kind": value.failure_kind,
        "post_proof": value.post_proof,
        "recovery": recovery,
        "native_status": value.native_status,
        "sc_domain": value.sc_domain,
        "sc_code": value.sc_code,
        "api": value.api,
        "final_stage": value.final_stage,
        "final_code": value.final_code,
    }))
    .ok()
}
impl Fact {
    #[cfg(target_os = "macos")]
    pub(crate) fn capture_recovery(
        source: &str,
        epoch: &str,
        frame_epoch: &str,
        capture: Option<&str>,
        candidate: &str,
        diagnostic: &super::macos::PreviewDiagnostic,
    ) -> Option<Self> {
        let diagnostic = diagnostic.clone().checked()?;
        capture_recovery_key(&diagnostic)?;
        Some(Self::CaptureRecovery {
            source_id: Id::new(source)?,
            source_epoch: Id::new(epoch)?,
            frame_epoch: Id::new(frame_epoch)?,
            capture_id: optional_id(capture)?,
            target_candidate_id: Id::new(candidate)?,
            diagnostic: Box::new(diagnostic),
        })
    }
    pub(crate) fn rejected(
        lease: &str,
        generation: &str,
        target_hash: &str,
        reason: Rejection,
    ) -> Option<Self> {
        Some(Self::ServiceFrameRejected {
            lease_id: Id::new(lease)?,
            generation: Id::new(generation)?,
            target_identity_hash: Id::new(target_hash)?,
            reason,
        })
    }
    pub(crate) fn projection(
        stage: ProjectionStage,
        p: &PreviewProjection,
        frame: Option<&PreviewFrame>,
        action: Option<PreviewControl>,
    ) -> Option<Self> {
        Some(Self::Projection {
            stage,
            projection: ProjectionMeta::new(p)?,
            frame: frame
                .map(FrameMeta::new)
                .map_or(Some(None), |value| value.map(Some))?,
            action,
            caller_role: None,
            requested_lease: None,
            requested_generation: None,
            requested_after_sequence: None,
        })
    }
    pub(crate) fn delivered(
        caller: &str,
        lease: &str,
        generation: &str,
        after: Option<&str>,
        update: &PreviewUpdate,
    ) -> Option<Self> {
        let mut fact = Self::projection(
            ProjectionStage::ServerDelivered,
            &update.projection,
            update.frame.as_ref(),
            None,
        )?;
        if let Self::Projection {
            caller_role,
            requested_lease,
            requested_generation,
            requested_after_sequence,
            ..
        } = &mut fact
        {
            *caller_role = Some(Id::new(caller)?);
            *requested_lease = Some(Id::new(lease)?);
            *requested_generation = Some(Id::new(generation)?);
            *requested_after_sequence = optional_id(after)?;
        }
        Some(fact)
    }
}
#[derive(Default)]
struct Counters {
    attempted: AtomicU64,
    enqueued: AtomicU64,
    dropped: AtomicU64,
    invalid: AtomicU64,
    processed: AtomicU64,
    written: AtomicU64,
    write_failures: AtomicU64,
}
struct Queued {
    attempt: u64,
    observed_at_ms: u128,
    observed_after_ns: u128,
    fact: Fact,
}
struct Recorder {
    sender: SyncSender<Queued>,
    counters: Arc<Counters>,
    started: Instant,
}
impl Recorder {
    fn record(&self, fact: Option<Fact>) {
        let attempt = self.counters.attempted.fetch_add(1, Ordering::Relaxed) + 1;
        let Some(fact) = fact else {
            self.counters.invalid.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let queued = Queued {
            attempt,
            observed_at_ms: now_ms(),
            observed_after_ns: self.started.elapsed().as_nanos(),
            fact,
        };
        if self.sender.try_send(queued).is_ok() {
            self.counters.enqueued.fetch_add(1, Ordering::Relaxed);
        } else {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
static RECORDER: OnceLock<Recorder> = OnceLock::new();
/// One detached writer. No AppHandle, repository, producer, exit hook, join,
/// fsync, business callback or path operation on the caller's recording path.
pub(crate) fn initialize(path: PathBuf, instance: &str, profile_hash: &str) {
    let Some((instance, profile_hash)) = Id::new(instance).zip(Id::new(profile_hash)) else {
        return;
    };
    RECORDER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_LIMIT);
        let counters = Arc::new(Counters::default());
        let health = counters.clone();
        let run_id = format!("{}-{}", std::process::id(), now_ms());
        let execution_nonce = std::env::var("LUNA_PRODUCT_EXECUTION_NONCE")
            .ok()
            .and_then(|value| parse_nonce(&value));
        let _ = std::thread::Builder::new()
            .name("runtime-observation".into())
            .spawn(move || {
                writer(
                    path,
                    receiver,
                    health,
                    run_id,
                    instance,
                    profile_hash,
                    execution_nonce,
                )
            });
        Recorder {
            sender,
            counters,
            started: Instant::now(),
        }
    });
}
pub(crate) fn record(fact: Option<Fact>) {
    if let Some(recorder) = RECORDER.get() {
        recorder.record(fact);
    }
}
fn parse_nonce(value: &str) -> Option<Id> {
    (value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| Id(value.to_owned()))
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis())
        .unwrap_or_default()
}
fn private_open(path: &Path) -> io::Result<File> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err(io::Error::other("observation file type"));
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true).read(true);
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
struct Segments {
    path: PathBuf,
    file: File,
    bytes: u64,
    rotation: u64,
}
impl Segments {
    fn new(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = private_open(&path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            path,
            file,
            bytes,
            rotation: 0,
        })
    }
    fn append(&mut self, record: &serde_json::Value) -> io::Result<()> {
        // Segment identity is assigned by the actual writer after rotation,
        // never guessed from a producer-side status field.
        let mut record = record.clone();
        record["segment_index"] = self.rotation.into();
        let mut line = serde_json::to_vec(&record)?;
        if line.len() >= RECORD_LIMIT {
            return Err(io::Error::other("observation record limit"));
        }
        line.push(b'\n');
        if self.bytes.saturating_add(line.len() as u64) > SEGMENT_LIMIT {
            let previous = self.path.with_extension("previous.jsonl");
            match std::fs::remove_file(&previous) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            std::fs::rename(&self.path, previous)?;
            self.file = private_open(&self.path)?;
            self.bytes = 0;
            self.rotation = self.rotation.saturating_add(1);
            record["segment_index"] = self.rotation.into();
            if record["kind"] == "observation_health" {
                record["rotation"] = self.rotation.into();
            }
            line = serde_json::to_vec(&record)?;
            if line.len() >= RECORD_LIMIT {
                return Err(io::Error::other("observation record limit"));
            }
            line.push(b'\n');
        }
        self.file.write_all(&line)?;
        self.bytes = self.bytes.saturating_add(line.len() as u64);
        Ok(())
    }
}
fn writer(
    path: PathBuf,
    receiver: Receiver<Queued>,
    counters: Arc<Counters>,
    run_id: String,
    instance: Id,
    profile_hash: Id,
    execution_nonce: Option<Id>,
) {
    let start = || -> io::Result<(File, Segments)> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lock = private_open(&path.with_extension("lock"))?;
        lock.try_lock().map_err(io::Error::other)?;
        Ok((lock, Segments::new(path.clone())?))
    };
    let Ok((_lock, mut output)) = start() else {
        counters.write_failures.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let begin = serde_json::json!({"kind":"observation_begin","schema_version":1,"run_id":run_id,"execution_nonce":execution_nonce,"pid":std::process::id(),"instance_id":instance,"profile_hash":profile_hash,"initialized_at_ms":now_ms(),"queue_limit":QUEUE_LIMIT,"record_limit":RECORD_LIMIT,"segment_limit":SEGMENT_LIMIT,"retained_segments":2,"exit_drain":"not_an_exit_gate","attempt_order":"unique_not_queue_order"});
    if output.append(&begin).is_err() {
        counters.write_failures.fetch_add(1, Ordering::Relaxed);
    }
    let mut max_observed_attempt = 0;
    let mut last_written_attempt = 0;
    let mut last_health = Instant::now();
    loop {
        let message = receiver.recv_timeout(HEALTH_INTERVAL);
        let terminal = matches!(message, Err(mpsc::RecvTimeoutError::Disconnected));
        if let Ok(queued) = message {
            max_observed_attempt = max_observed_attempt.max(queued.attempt);
            let record = serde_json::json!({"kind":"runtime_fact","run_id":run_id,"execution_nonce":execution_nonce,"attempt_sequence":queued.attempt,"observed_at_ms":queued.observed_at_ms,"observed_after_ns":queued.observed_after_ns,"fact":queued.fact});
            if output.append(&record).is_ok() {
                counters.written.fetch_add(1, Ordering::Relaxed);
                last_written_attempt = queued.attempt;
            } else {
                counters.write_failures.fetch_add(1, Ordering::Relaxed);
            }
            counters.processed.fetch_add(1, Ordering::Relaxed);
        }
        // Health is periodic even under continuous frame traffic; the last
        // dropped attempt does not depend on a later successful business event.
        if terminal || last_health.elapsed() >= HEALTH_INTERVAL {
            let record = serde_json::json!({"kind":"observation_health","run_id":run_id,"execution_nonce":execution_nonce,"at_ms":now_ms(),"attempted":counters.attempted.load(Ordering::Relaxed),"enqueued":counters.enqueued.load(Ordering::Relaxed),"dropped":counters.dropped.load(Ordering::Relaxed),"invalid":counters.invalid.load(Ordering::Relaxed),"processed":counters.processed.load(Ordering::Relaxed),"written_facts":counters.written.load(Ordering::Relaxed),"write_failures":counters.write_failures.load(Ordering::Relaxed),"max_observed_attempt":max_observed_attempt,"last_written_attempt":last_written_attempt,"rotation":output.rotation,"terminal":terminal,"evidence_only":true});
            if output.append(&record).is_err() {
                counters.write_failures.fetch_add(1, Ordering::Relaxed);
            }
            last_health = Instant::now();
        }
        if terminal {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fact() -> Option<Fact> {
        Some(Fact::Worker {
            stage: WorkerStage::Spawned,
            pid: 1,
            instance: None,
        })
    }
    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "luna-observation-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ))
    }
    #[cfg(target_os = "macos")]
    fn recovery_diagnostic() -> super::super::macos::PreviewDiagnostic {
        use super::super::macos::{
            CaptureFailureKind, PreviewDiagnostic, PreviewGeometrySnapshot,
            PreviewRecoveryDiagnostic,
        };
        let mut diagnostic = PreviewDiagnostic::worker("capture_complete");
        diagnostic.geometry = Some(PreviewGeometrySnapshot {
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
        diagnostic.failure_kind = Some(CaptureFailureKind::CompatibilitySourceCap);
        diagnostic.post_proof = Some("post_verified_retain".into());
        diagnostic.final_stage = Some("compatibility_bounds".into());
        diagnostic.final_code = Some("preview_budget_exceeded".into());
        diagnostic.recovery = Some(PreviewRecoveryDiagnostic {
            state: "geometry_blocked".into(),
            credits: 2,
            normal_trigger_without_recovery_credit: false,
            samples: 1,
            generation: 0,
            cache_entries: 1,
            metadata_only: true,
            admission: false,
        });
        diagnostic
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn recovery_fact_records_checked_same_attempt_scalars_without_image_or_private_text() {
        let diagnostic = recovery_diagnostic();
        let fact = Fact::capture_recovery(
            "source",
            "1",
            "frame-1",
            Some("capture"),
            &"a".repeat(64),
            &diagnostic,
        )
        .unwrap();
        let value = serde_json::to_value(fact).unwrap();
        assert_eq!(value["kind"], "capture_recovery");
        assert_eq!(value["diagnostic"]["geometry"]["point_pixel_scale"], 2.0);
        assert_eq!(value["diagnostic"]["post_proof"], "post_verified_retain");
        let encoded = value.to_string();
        assert!(encoded.len() + 512 < RECORD_LIMIT);
        for private in [
            "payload",
            "window_title",
            "full_image_path",
            "selection_token",
            "PRIVATE_FRAME",
            "台詞",
        ] {
            assert!(!encoded.contains(private), "{private}");
        }
        let mut invalid = diagnostic.clone();
        invalid.geometry.as_mut().unwrap().point_pixel_scale = Some(f64::NAN);
        assert!(
            Fact::capture_recovery("source", "1", "frame-1", None, "candidate", &invalid).is_none()
        );
        invalid = diagnostic.clone();
        invalid.post_proof = Some("PRIVATE_PATH_OR_TEXT".into());
        assert!(
            Fact::capture_recovery("source", "1", "frame-1", None, "candidate", &invalid).is_none()
        );
        assert!(
            Fact::capture_recovery(
                "/Users/private",
                "1",
                "frame-1",
                None,
                "candidate",
                &diagnostic
            )
            .is_none()
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn recovery_fact_key_coalesces_ticks_raw_noise_and_changes_only_structural_facts() {
        let original = recovery_diagnostic();
        let key = capture_recovery_key(&original).unwrap();
        let mut tick = original.clone();
        tick.operation += 100;
        tick.revision += 10;
        let recovery = tick.recovery.as_mut().unwrap();
        recovery.samples += 10;
        recovery.generation += 1;
        tick.geometry.as_mut().unwrap().cg_width_points = Some(1461.9999);
        assert_eq!(capture_recovery_key(&tick).unwrap(), key);
        tick.recovery.as_mut().unwrap().credits = 1;
        assert_ne!(capture_recovery_key(&tick).unwrap(), key);
        tick = original.clone();
        tick.geometry.as_mut().unwrap().cg_width_points = Some(157.0);
        assert_ne!(capture_recovery_key(&tick).unwrap(), key);
        tick = original.clone();
        tick.post_proof = Some("release_unproved".into());
        assert_ne!(capture_recovery_key(&tick).unwrap(), key);
        tick = original.clone();
        tick.native_geometry = tick.geometry.clone();
        let native = tick.native_geometry.as_mut().unwrap();
        native.sc_window_frame_query = "queried".into();
        native.sc_window_width_points = Some(158.0);
        native.sc_window_height_points = Some(153.0);
        assert_ne!(capture_recovery_key(&tick).unwrap(), key);
    }
    #[test]
    fn full_and_disconnected_queue_drop_evidence_without_waiting_or_business_callback() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let recorder = Recorder {
            sender,
            counters: Arc::new(Counters::default()),
            started: Instant::now(),
        };
        recorder.record(fact());
        recorder.record(fact());
        assert_eq!(recorder.counters.enqueued.load(Ordering::Relaxed), 1);
        assert_eq!(recorder.counters.dropped.load(Ordering::Relaxed), 1);
        assert!(receiver.try_recv().is_ok());
        drop(receiver);
        recorder.record(fact());
        recorder.record(None);
        assert_eq!(recorder.counters.attempted.load(Ordering::Relaxed), 4);
        assert_eq!(recorder.counters.dropped.load(Ordering::Relaxed), 2);
        assert_eq!(recorder.counters.invalid.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn nonce_is_only_a_bounded_hex_observation_label() {
        assert!(parse_nonce("0123456789abcdef0123456789abcdef").is_some());
        for value in [
            "",
            "not-a-nonce",
            "0123456789abcdef0123456789abcdeg",
            "0123456789abcdef0123456789abcdef00",
        ] {
            assert!(parse_nonce(value).is_none());
        }
    }
    #[test]
    fn arbitrary_text_and_oversized_ids_cannot_enter_typed_metadata() {
        for value in [
            "path/to/file",
            "C:\\secret",
            "a secret",
            "台詞",
            "bad\0value",
        ] {
            assert!(Id::new(value).is_none());
        }
        assert!(Id::new(&"a".repeat(129)).is_none());
        assert!(Id::new(&"a".repeat(128)).is_some());
        let frame = PreviewFrame {
            lease_id: "lease".into(),
            generation: "2".into(),
            sequence: "4".into(),
            target_identity_hash: "a".repeat(64),
            captured_at_ms: "42".into(),
            width: 2,
            height: 3,
            content_hash: "b".repeat(64),
            encoding: "jpeg".into(),
            payload: b"PRIVATE_FRAME_PAYLOAD".to_vec(),
        };
        let encoded = serde_json::to_string(&FrameMeta::new(&frame).unwrap()).unwrap();
        assert!(!encoded.contains("PRIVATE_FRAME_PAYLOAD"));
        assert!(!encoded.contains("payload"));
        assert!(!encoded.contains("encoding"));
        assert!(encoded.contains("service_sequence"));
        assert!(!encoded.contains("worker_sequence"));
    }
    #[test]
    fn writer_failure_is_evidence_failure_and_does_not_require_business_drain() {
        let parent = temporary("file-parent");
        std::fs::write(&parent, b"regular").unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        let counters = Arc::new(Counters::default());
        writer(
            parent.join("facts.jsonl"),
            receiver,
            counters.clone(),
            "run".into(),
            Id::new("instance").unwrap(),
            Id::new("hash").unwrap(),
            None,
        );
        let recorder = Recorder {
            sender,
            counters: counters.clone(),
            started: Instant::now(),
        };
        recorder.record(fact());
        assert_eq!(counters.write_failures.load(Ordering::Relaxed), 1);
        assert_eq!(counters.dropped.load(Ordering::Relaxed), 1);
        std::fs::remove_file(parent).unwrap();
    }
    #[test]
    fn segments_are_bounded_private_and_reject_oversized_records() {
        let directory = temporary("bounded");
        let path = directory.join("facts.jsonl");
        let mut segments = Segments::new(path.clone()).unwrap();
        let record = serde_json::json!({"kind":"test","fixed":"x".repeat(2000)});
        for _ in 0..600 {
            segments.append(&record).unwrap();
        }
        assert!(segments.rotation >= 2);
        let last: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .last()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(last["segment_index"], segments.rotation);
        assert!(std::fs::metadata(&path).unwrap().len() <= SEGMENT_LIMIT);
        assert!(
            std::fs::metadata(path.with_extension("previous.jsonl"))
                .unwrap()
                .len()
                <= SEGMENT_LIMIT
        );
        assert!(
            segments
                .append(&serde_json::json!({"fixed":"x".repeat(RECORD_LIMIT)}))
                .is_err()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn health_accounts_final_drop_and_invalid_without_a_followup_fact() {
        let directory = temporary("health");
        let path = directory.join("facts.jsonl");
        let (sender, receiver) = mpsc::sync_channel(1);
        let counters = Arc::new(Counters::default());
        let recorder = Recorder {
            sender,
            counters: counters.clone(),
            started: Instant::now(),
        };
        recorder.record(fact());
        recorder.record(fact());
        recorder.record(None);
        drop(recorder);
        writer(
            path.clone(),
            receiver,
            counters,
            "run".into(),
            Id::new("instance").unwrap(),
            Id::new("profile").unwrap(),
            Some(parse_nonce("0123456789abcdef0123456789abcdef").unwrap()),
        );
        let rows: Vec<serde_json::Value> = std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let health = rows.last().unwrap();
        assert_eq!(
            health["execution_nonce"],
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(health["kind"], "observation_health");
        assert_eq!(health["attempted"], 3);
        assert_eq!(health["enqueued"], 1);
        assert_eq!(health["dropped"], 1);
        assert_eq!(health["invalid"], 1);
        assert_eq!(health["processed"], 1);
        assert_eq!(health["written_facts"], 1);
        assert_eq!(health["write_failures"], 0);
        assert_eq!(health["terminal"], true);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn delivered_metadata_cannot_expose_payload_detail_or_claim_service_commit() {
        let update = PreviewUpdate {
            projection: PreviewProjection {
                lease_id: Some("lease".into()),
                generation: "3".into(),
                consumer_id: "consumer".into(),
                state: PreviewState::Live,
                permission: PreviewPermission::Granted,
                target_identity_hash: Some("a".repeat(64)),
                attempted_at_ms: Some("10".into()),
                captured_at_ms: Some("20".into()),
                content_changed_at_ms: Some("20".into()),
                permission_checked_at_ms: Some("9".into()),
                error_code: Some("PRIVATE_ERROR_WITH_TEXT".into()),
                detail: "PRIVATE_DETAIL".into(),
            },
            frame: None,
        };
        let fact = Fact::delivered("main", "lease", "3", Some("5"), &update).unwrap();
        let row = serde_json::to_value(fact).unwrap();
        assert_eq!(row["stage"], "server_delivered");
        assert_eq!(row["requested_after_sequence"], "5");
        assert!(row["frame"].is_null());
        let encoded = row.to_string();
        assert!(!encoded.contains("PRIVATE_DETAIL"));
        assert!(!encoded.contains("PRIVATE_ERROR"));
        assert!(!encoded.contains("service_committed"));
    }
    #[test]
    fn continuous_traffic_reports_health_before_the_stream_or_process_stops() {
        let directory = temporary("busy-health");
        let path = directory.join("facts.jsonl");
        let (sender, receiver) = mpsc::sync_channel(QUEUE_LIMIT);
        let counters = Arc::new(Counters::default());
        let health = counters.clone();
        let output = path.clone();
        let worker = std::thread::spawn(move || {
            writer(
                output,
                receiver,
                health,
                "busy-run".into(),
                Id::new("instance").unwrap(),
                Id::new("profile").unwrap(),
                None,
            )
        });
        let recorder = Recorder {
            sender,
            counters,
            started: Instant::now(),
        };
        let until = Instant::now() + Duration::from_secs(5);
        let mut observed_running_health = false;
        while Instant::now() < until {
            recorder.record(fact());
            std::thread::sleep(Duration::from_millis(10));
            if std::fs::read_to_string(&path).is_ok_and(|text| {
                text.lines().any(|line| {
                    serde_json::from_str::<serde_json::Value>(line).is_ok_and(|row| {
                        row["kind"] == "observation_health" && row["terminal"] == false
                    })
                })
            }) {
                observed_running_health = true;
                break;
            }
        }
        drop(recorder);
        worker.join().unwrap();
        assert!(observed_running_health);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn exclusive_writer_failure_and_symlink_are_rejected_without_following() {
        let directory = temporary("exclusive");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("facts.jsonl");
        let lock = private_open(&path.with_extension("lock")).unwrap();
        lock.try_lock().unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        let counters = Arc::new(Counters::default());
        writer(
            path.clone(),
            receiver,
            counters.clone(),
            "other".into(),
            Id::new("instance").unwrap(),
            Id::new("profile").unwrap(),
            None,
        );
        assert_eq!(counters.write_failures.load(Ordering::Relaxed), 1);
        assert!(!path.exists());
        drop(sender);
        #[cfg(unix)]
        {
            let original = directory.join("original");
            std::fs::write(&original, b"original bytes").unwrap();
            std::os::unix::fs::symlink(&original, &path).unwrap();
            assert!(private_open(&path).is_err());
            assert_eq!(std::fs::read(&original).unwrap(), b"original bytes");
        }
        drop(lock);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
