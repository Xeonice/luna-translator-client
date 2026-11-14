//! One owned child per operation; common Runtime owns business scheduling/retries/commit.
use super::{
    protocol::{self, ChildFrame, ParentFrame, WorkerOperation},
    *,
};
use std::{
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Semaphore, watch},
    time::{Instant, timeout},
};
const STARTUP_BUDGET: Duration = Duration::from_secs(20);
const CANCEL_GRACE: Duration = Duration::from_millis(250);
pub struct OfflineExecutor {
    slots: [Arc<Semaphore>; 4],
}
impl Default for OfflineExecutor {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| Arc::new(Semaphore::new(1))),
        }
    }
}
impl OfflineExecutor {
    pub fn new() -> Self {
        Self::default()
    }
    pub(super) fn shared_engine_slots(&self) -> [Arc<Semaphore>; 4] {
        self.slots.clone()
    }
    pub async fn execute(
        &self,
        request: OfflineExecution,
        cancel: watch::Receiver<bool>,
    ) -> OfflineResult<OfflineOutput> {
        let engine = protocol::validate_request(&request)?;
        let result = self
            .run(
                engine,
                request.identity.request_id.clone(),
                Some(request.identity.clone()),
                &request.resource,
                &request.launch,
                request.timeout,
                WorkerOperation::Translate,
                Some(&request.payload.source_text),
                Some(&request.expected_observation),
                cancel,
            )
            .await?;
        let Some(output) = result.output else {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ProtocolViolation,
                "missing_final",
            ));
        };
        Ok(output)
    }
    pub async fn observe(
        &self,
        probe: OfflineProbe,
        cancel: watch::Receiver<bool>,
    ) -> OfflineResult<RuntimeObservation> {
        if probe.contract_revision != 1
            || probe.request_id.is_empty()
            || probe.request_id.len() > 256
            || probe.request_id.chars().any(char::is_control)
            || probe.timeout.is_zero()
            || probe.timeout > Duration::from_secs(600)
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidRequest,
                "probe",
            ));
        }
        Ok(self
            .run(
                probe.engine,
                probe.request_id,
                None,
                &probe.resource,
                &probe.launch,
                probe.timeout,
                WorkerOperation::Observe,
                None,
                None,
                cancel,
            )
            .await?
            .observation)
    }
    #[allow(clippy::too_many_arguments)]
    async fn run(
        &self,
        engine: OfflineEngine,
        request_id: String,
        identity: Option<ExecutionIdentity>,
        resource: &ResourceManifest,
        launch: &WorkerLaunch,
        budget: Duration,
        operation: WorkerOperation,
        text: Option<&str>,
        expected_observation: Option<&RuntimeObservation>,
        mut cancel: watch::Receiver<bool>,
    ) -> OfflineResult<RunOutput> {
        let index = match engine {
            OfflineEngine::AppleTranslation => 0,
            OfflineEngine::ArgosTranslate => 1,
            OfflineEngine::Jbeijing => 2,
            OfflineEngine::KingsoftFastait => 3,
        };
        let slot = self.slots[index].clone().try_acquire_owned().map_err(|_| {
            OfflineFailure::new(OfflineErrorCode::WorkerBusy, "engine_serialization")
        })?;
        if *cancel.borrow() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::Cancelled,
                "before_start",
            ));
        }
        let start = Instant::now();
        let inspection = Arc::new(InspectionControl {
            stopped: AtomicBool::new(false),
            deadline: std::time::Instant::now() + budget,
            cancellation: cancel.clone(),
            _slot: slot,
        });
        // Drop/abort also stops cooperative inspection. The closure retains
        // the permit until its physical I/O ends, even after a caller timeout.
        let _preparation = PreparationGuard(inspection.clone());
        let manifest = resource.clone();
        let launch_for_preparation = launch.clone();
        let prepare_control = inspection.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let check = || prepare_control.check();
            resources::verify_manifest_checked(&manifest, engine, &check)?;
            check()?;
            match engine {
                OfflineEngine::AppleTranslation => {
                    apple::validate(&manifest, &launch_for_preparation)
                }
                OfflineEngine::ArgosTranslate => {
                    argos::validate(&manifest, &launch_for_preparation)
                }
                _ => windows::validate(engine, &manifest, &launch_for_preparation),
            }?;
            check()?;
            let command = prepare_command(&launch_for_preparation, &check)?;
            check()?;
            Ok::<_, OfflineFailure>(command)
        });
        let mut command = tokio::select! {
            biased;
            _ = cancelled(&mut cancel) => return Err(OfflineFailure::new(OfflineErrorCode::Cancelled, "inspection")),
            result = timeout(budget, prepared) => result
                .map_err(|_| OfflineFailure::new(OfflineErrorCode::DeadlineExceeded, "inspection"))?
                .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "inspection_join"))??,
        };
        let mut wire_resource = resource.clone();
        if let WorkerLaunch::CrossOver {
            worker_resource_root,
            ..
        } = launch
        {
            let bytes = worker_resource_root.as_bytes();
            if bytes.len() < 3
                || !bytes[0].is_ascii_alphabetic()
                || bytes[1] != b':'
                || bytes[2] != b'\\'
                || worker_resource_root.contains('\0')
                || worker_resource_root.split(['\\', '/']).any(|s| s == "..")
            {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidResource,
                    "worker_resource_mapping",
                ));
            }
            wire_resource.root = PathBuf::from(worker_resource_root);
        }
        let instance = uuid::Uuid::new_v4().to_string();
        if *cancel.borrow() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::Cancelled,
                "before_spawn",
            ));
        }
        if start.elapsed() >= budget {
            return Err(OfflineFailure::new(
                OfflineErrorCode::DeadlineExceeded,
                "before_spawn",
            ));
        }
        // Only this owned future starts a child, never the blocking inspector.
        let mut child = command
            .spawn()
            .map_err(|_| OfflineFailure::new(OfflineErrorCode::WorkerStartFailed, "spawn"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| OfflineFailure::new(OfflineErrorCode::WorkerStartFailed, "stderr"))?;
        // Drain without retaining vendor messages (which may include source text or local paths).
        let stderr_task = tokio::spawn(async move {
            let mut reader = stderr;
            let mut buf = [0u8; 4096];
            let mut count = 0usize;
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        count = count.saturating_add(n);
                        if count > MAX_FRAME_BYTES {
                            break;
                        }
                    }
                }
            }
        });
        let result = async {
            let mut input = child.stdin.take().ok_or_else(|| OfflineFailure::new(OfflineErrorCode::WorkerStartFailed, "stdin"))?;
            let stdout = child.stdout.take().ok_or_else(|| OfflineFailure::new(OfflineErrorCode::WorkerStartFailed, "stdout"))?;
            let mut output = BufReader::new(stdout);
            let exchange = Exchange { engine, request_id: &request_id, instance: &instance, identity: identity.as_ref(), resource, wire_resource: &wire_resource, operation, text, expected_observation, inspection: &inspection };
            let remaining = budget.saturating_sub(start.elapsed());
            tokio::select! {
                biased;
                _ = cancelled(&mut cancel) => {
                    let frame = ParentFrame::Cancel { protocol: PROTOCOL.into(), worker_instance_id: instance.clone(), request_id: request_id.clone() };
                    let _ = timeout(CANCEL_GRACE, send(&mut input, &frame)).await;
                    Err(OfflineFailure::new(OfflineErrorCode::Cancelled, "execution"))
                }
                result = timeout(remaining, exchange.run(&mut input, &mut output)) => {
                    result.map_err(|_| OfflineFailure::new(OfflineErrorCode::DeadlineExceeded, "execution"))?
                }
            }
        }.await;
        // No pid/name kill: Child refers only to the process this call owns.
        let exited = timeout(CANCEL_GRACE, child.wait()).await;
        let clean_exit = match exited {
            Ok(Ok(status)) => status.success(),
            _ => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                false
            }
        };
        stderr_task.abort();
        let _ = stderr_task.await;
        if result.is_ok() && !clean_exit {
            return Err(OfflineFailure::new(
                OfflineErrorCode::WorkerExited,
                "unclean_exit",
            ));
        }
        result
    }
}
struct Exchange<'a> {
    engine: OfflineEngine,
    request_id: &'a str,
    instance: &'a str,
    identity: Option<&'a ExecutionIdentity>,
    resource: &'a ResourceManifest,
    wire_resource: &'a ResourceManifest,
    operation: WorkerOperation,
    text: Option<&'a str>,
    expected_observation: Option<&'a RuntimeObservation>,
    inspection: &'a Arc<InspectionControl>,
}
impl Exchange<'_> {
    async fn run(
        &self,
        input: &mut tokio::process::ChildStdin,
        output: &mut BufReader<tokio::process::ChildStdout>,
    ) -> OfflineResult<RunOutput> {
        send(
            input,
            &ParentFrame::Hello {
                protocol: PROTOCOL.into(),
                worker_instance_id: self.instance.into(),
                engine_id: self.engine.id().into(),
                contract_revision: 1,
                request_id: self.request_id.into(),
                identity: self.identity.cloned(),
                resource: Box::new(self.wire_resource.clone()),
                operation: self.operation,
            },
        )
        .await?;
        let frame = timeout(STARTUP_BUDGET, read(output))
            .await
            .map_err(|_| OfflineFailure::new(OfflineErrorCode::DeadlineExceeded, "handshake"))??;
        let observation = match frame {
            ChildFrame::Ready {
                protocol,
                worker_instance_id,
                request_id,
                resource_digest,
                observation,
            } if protocol == PROTOCOL
                && worker_instance_id == self.instance
                && request_id == self.request_id
                && resource_digest == self.resource.digest
                && observation.engine_id == self.engine.id() =>
            {
                observation
            }
            ChildFrame::Error {
                protocol,
                worker_instance_id,
                request_id,
                error,
            } if protocol == PROTOCOL
                && worker_instance_id == self.instance
                && request_id == self.request_id =>
            {
                return Err(safe_worker_error(error));
            }
            _ => {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::ProtocolViolation,
                    "ready_identity",
                ));
            }
        };
        if observation.engine_contract_revision != 1 {
            return Err(OfflineFailure::new(
                OfflineErrorCode::UnsupportedContract,
                "runtime_contract",
            ));
        }
        if self
            .expected_observation
            .is_some_and(|expected| expected != &observation)
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceChanged,
                "runtime_attestation",
            ));
        }
        if observation.runtime_version.is_empty()
            || observation.runtime_version.len() > 4096
            || observation.platform.is_empty()
            || observation.platform.len() > 64
            || observation.architecture.is_empty()
            || observation.architecture.len() > 64
            || (observation.availability == ResourceAvailability::Installed
                && observation.reason.is_some())
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ProtocolViolation,
                "runtime_observation",
            ));
        }
        if self.operation == WorkerOperation::Observe {
            expect_eof(output).await?;
            return Ok(RunOutput {
                observation,
                output: None,
            });
        }
        if observation.availability != ResourceAvailability::Installed {
            return Err(OfflineFailure::new(
                observation.reason.unwrap_or(OfflineErrorCode::NotReady),
                "worker_readiness",
            ));
        }
        let identity = self.identity.ok_or_else(|| {
            OfflineFailure::new(OfflineErrorCode::InvalidRequest, "execution_identity")
        })?;
        send(
            input,
            &ParentFrame::Execute {
                protocol: PROTOCOL.into(),
                worker_instance_id: self.instance.into(),
                identity: identity.clone(),
                source_text: self.text.unwrap_or_default().into(),
            },
        )
        .await?;
        match read(output).await? {
            ChildFrame::Final {
                protocol,
                worker_instance_id,
                identity: reply,
                resource_identity,
                resource_digest,
                route,
                text,
            } if protocol == PROTOCOL
                && worker_instance_id == self.instance
                && *reply == *identity
                && resource_identity == self.resource.identity
                && resource_digest == self.resource.digest
                && route == self.resource.route =>
            {
                if text.len() > MAX_OUTPUT_BYTES {
                    return Err(OfflineFailure::new(
                        OfflineErrorCode::OutputTooLarge,
                        "output",
                    ));
                }
                if text.is_empty() || text.contains('\0') {
                    return Err(OfflineFailure::new(
                        OfflineErrorCode::EngineFailed,
                        "empty_or_invalid_output",
                    ));
                }
                let manifest = self.resource.clone();
                let engine = self.engine;
                let inspection = self.inspection.clone();
                tokio::task::spawn_blocking(move || {
                    resources::verify_manifest_checked(&manifest, engine, &|| inspection.check())
                })
                .await
                .map_err(|_| {
                    OfflineFailure::new(OfflineErrorCode::InvalidResource, "post_inspection")
                })??;
                expect_eof(output).await?;
                Ok(RunOutput {
                    observation: observation.clone(),
                    output: Some(OfflineOutput {
                        text,
                        identity: *reply,
                        resource: resource_identity,
                        resource_digest,
                        route,
                        worker_instance_id,
                        observation,
                    }),
                })
            }
            ChildFrame::Error {
                protocol,
                worker_instance_id,
                request_id,
                error,
            } if protocol == PROTOCOL
                && worker_instance_id == self.instance
                && request_id == self.request_id =>
            {
                Err(safe_worker_error(error))
            }
            _ => Err(OfflineFailure::new(
                OfflineErrorCode::ProtocolViolation,
                "final_identity",
            )),
        }
    }
}
async fn expect_eof(output: &mut BufReader<tokio::process::ChildStdout>) -> OfflineResult<()> {
    let mut trailing = [0u8; 1];
    let count = output
        .read(&mut trailing)
        .await
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::ProtocolViolation, "trailing_read"))?;
    if count != 0 {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ProtocolViolation,
            "duplicate_or_trailing_frame",
        ));
    }
    Ok(())
}
struct RunOutput {
    observation: RuntimeObservation,
    output: Option<OfflineOutput>,
}
fn safe_worker_error(mut error: OfflineFailure) -> OfflineFailure {
    if error.stage.len() > 64
        || !error
            .stage
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        error.stage = "worker".into();
    }
    error
}
async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    loop {
        if *cancel.borrow() {
            return;
        }
        if cancel.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
async fn send(writer: &mut tokio::process::ChildStdin, frame: &ParentFrame) -> OfflineResult<()> {
    writer
        .write_all(&protocol::encode(frame)?)
        .await
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::WorkerExited, "write"))?;
    writer
        .flush()
        .await
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::WorkerExited, "flush"))
}
async fn read(reader: &mut BufReader<tokio::process::ChildStdout>) -> OfflineResult<ChildFrame> {
    let mut bytes = Vec::new();
    let count = reader
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::ProtocolViolation, "read"))?;
    if count == 0 {
        return Err(OfflineFailure::new(OfflineErrorCode::WorkerExited, "eof"));
    }
    protocol::decode(&bytes)
}
struct InspectionControl {
    stopped: AtomicBool,
    deadline: std::time::Instant,
    cancellation: watch::Receiver<bool>,
    _slot: tokio::sync::OwnedSemaphorePermit,
}
impl InspectionControl {
    fn check(&self) -> OfflineResult<()> {
        if self.stopped.load(Ordering::Acquire) || *self.cancellation.borrow() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::Cancelled,
                "inspection",
            ));
        }
        if std::time::Instant::now() >= self.deadline {
            return Err(OfflineFailure::new(
                OfflineErrorCode::DeadlineExceeded,
                "inspection",
            ));
        }
        Ok(())
    }
}
struct PreparationGuard(Arc<InspectionControl>);
impl Drop for PreparationGuard {
    fn drop(&mut self) {
        self.0.stopped.store(true, Ordering::Release);
    }
}
pub(super) fn prepare_command(
    launch: &WorkerLaunch,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<Command> {
    let mut command = match launch {
        WorkerLaunch::Native {
            executable,
            executable_sha256,
        } => {
            resources::verify_executable_checked(executable, executable_sha256, check)?;
            Command::new(executable)
        }
        WorkerLaunch::Argos {
            interpreter,
            interpreter_sha256,
            script,
            script_sha256,
        } => {
            resources::verify_executable_checked(interpreter, interpreter_sha256, check)?;
            resources::verify_executable_checked(script, script_sha256, check)?;
            let mut command = Command::new(interpreter);
            command.args(["-I", "-B"]).arg(script);
            command
        }
        WorkerLaunch::CrossOver {
            launcher,
            launcher_sha256,
            bottle,
            worker,
            worker_sha256,
            ..
        } => {
            resources::verify_executable_checked(launcher, launcher_sha256, check)?;
            resources::verify_executable_checked(worker, worker_sha256, check)?;
            if bottle.is_empty()
                || bottle.len() > 128
                || bottle.starts_with('-')
                || bottle.chars().any(char::is_control)
            {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidRequest,
                    "bottle",
                ));
            }
            let mut command = Command::new(launcher);
            command.arg("--bottle").arg(bottle).arg(worker);
            command
        }
    };
    // Deliberately do not inherit proxy, PYTHONPATH, injected libraries, custom Argos configs.
    command.env_clear();
    for name in [
        "HOME",
        "TMPDIR",
        "SYSTEMROOT",
        "WINDIR",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command.env(
        "PATH",
        if cfg!(windows) {
            "C:\\Windows\\System32"
        } else {
            "/usr/bin:/bin"
        },
    );
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    check()?;
    Ok(command)
}
