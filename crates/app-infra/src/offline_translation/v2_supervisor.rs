//! Observe-only owned child. Wire facts never install an engine or authorize text.
use super::{inspection::ResourceInspector, supervisor::OfflineExecutor, v2_protocol, *};
use app_core::{
    domain::offline_runtime::*,
    ports::{operation_budget::OperationBudget, physical_jobs::PhysicalJobs},
};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::{Child, Command},
    sync::{Semaphore, oneshot},
    task::JoinHandle,
};
const GRACE: Duration = Duration::from_millis(250);

pub struct OfflineObservationRequestV2 {
    pub hello: OfflineWorkerHelloV2,
    pub launch: WorkerLaunch,
}
/// Child assertions, not trusted material facts/coverage. Constructed by the owned transport only.
pub struct ObservedWorkerFactsV2 {
    ready: ChildFrameV2,
    records: Vec<WorkerEvidenceRecordV2>,
}
impl ObservedWorkerFactsV2 {
    pub fn ready(&self) -> &ChildFrameV2 {
        &self.ready
    }
    pub fn records(&self) -> &[WorkerEvidenceRecordV2] {
        &self.records
    }
}
/// Non-Clone handle. Dropping it signals cleanup; the runner retains actual I/O guards.
pub struct OfflineObservedSessionV2 {
    facts: ObservedWorkerFactsV2,
    stop: Option<oneshot::Sender<()>>,
    runner: Option<JoinHandle<OfflineResult<()>>>,
}
impl OfflineObservedSessionV2 {
    pub fn facts(&self) -> &ObservedWorkerFactsV2 {
        &self.facts
    }
    pub async fn close(mut self) -> OfflineResult<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.runner
            .take()
            .ok_or_else(|| failure(OfflineErrorCode::WorkerExited, "v2_owner_missing"))?
            .await
            .map_err(|_| failure(OfflineErrorCode::WorkerExited, "v2_owner_join"))?
    }
}
pub struct OfflineObservationSupervisorV2 {
    slots: [Arc<Semaphore>; 4],
    inspector: ResourceInspector,
    physical: Arc<PhysicalJobs>,
}
impl OfflineObservationSupervisorV2 {
    pub fn new(
        owner: &OfflineExecutor,
        inspector: ResourceInspector,
        physical: Arc<PhysicalJobs>,
    ) -> Self {
        Self {
            slots: owner.shared_engine_slots(),
            inspector,
            physical,
        }
    }
    pub async fn observe(
        &self,
        request: OfflineObservationRequestV2,
        budget: OperationBudget,
    ) -> OfflineResult<OfflineObservedSessionV2> {
        request
            .hello
            .validate()
            .map_err(|_| failure(OfflineErrorCode::InvalidRequest, "v2_hello_shape"))?;
        budget.check().map_err(budget_failure)?;
        let engine =
            OfflineEngine::parse(&request.hello.reference.resource.profile.engine.engine_id)?;
        // Guest ownership needs the later actual CrossOver worker/Native owner chain.
        if matches!(request.launch, WorkerLaunch::CrossOver { .. }) {
            return Err(failure(
                OfflineErrorCode::UnsupportedPlatform,
                "v2_guest_owner_pending",
            ));
        }
        let index = match engine {
            OfflineEngine::AppleTranslation => 0,
            OfflineEngine::ArgosTranslate => 1,
            OfflineEngine::Jbeijing => 2,
            OfflineEngine::KingsoftFastait => 3,
        };
        let slot = Arc::new(
            self.slots[index]
                .clone()
                .try_acquire_owned()
                .map_err(|_| failure(OfflineErrorCode::WorkerBusy, "v2_engine_serialization"))?,
        );
        let physical = Arc::new(
            self.physical
                .try_begin()
                .map_err(|_| failure(OfflineErrorCode::NotReady, "v2_physical_admission"))?,
        );
        let work_slot = slot.clone();
        let work_physical = physical.clone();
        let (prepared, _preparation_slot, _preparation_physical) = self
            .inspector
            .run_with_budget(budget.clone(), move |check| {
                // Carry ownership through the Inspector's final check/result,
                // including a detached blocking result after caller cancellation.
                Ok((
                    super::supervisor::prepare_command(&request.launch, check),
                    work_slot,
                    work_physical,
                ))
            })
            .await?;
        let command = prepared?;
        budget.check().map_err(budget_failure)?;
        let (stop, receiver) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        // This owned runner, not the caller future, retains capacity and physical ownership.
        let runner = tokio::spawn(async move {
            let _slot = slot;
            let _physical = physical;
            run_owned(command, request.hello, budget, receiver, ready_tx).await
        });
        match ready_rx.await {
            Ok(Ok(facts)) => Ok(OfflineObservedSessionV2 {
                facts,
                stop: Some(stop),
                runner: Some(runner),
            }),
            result => {
                drop(stop);
                let _ = runner.await;
                match result {
                    Ok(Err(e)) => Err(e),
                    _ => Err(failure(OfflineErrorCode::WorkerExited, "v2_ready_channel")),
                }
            }
        }
    }
}
fn failure(code: OfflineErrorCode, stage: &str) -> OfflineFailure {
    OfflineFailure::new(code, stage)
}
fn budget_failure(error: app_core::contracts::AppError) -> OfflineFailure {
    failure(
        if error.code == "operation_timeout" {
            OfflineErrorCode::DeadlineExceeded
        } else {
            OfflineErrorCode::Cancelled
        },
        "v2_original_budget",
    )
}
async fn stop_reason(
    budget: &OperationBudget,
    stop: &mut oneshot::Receiver<()>,
) -> OfflineResult<()> {
    loop {
        budget.check().map_err(budget_failure)?;
        tokio::select! { biased;
            _ = &mut *stop => return Ok(()),
            _ = budget.cancelled() => return Err(budget.check().err().map(budget_failure).unwrap_or_else(|| failure(OfflineErrorCode::Cancelled,"v2_original_budget"))),
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }
}
async fn run_owned(
    mut command: Command,
    hello: OfflineWorkerHelloV2,
    budget: OperationBudget,
    mut stop: oneshot::Receiver<()>,
    ready_tx: oneshot::Sender<OfflineResult<ObservedWorkerFactsV2>>,
) -> OfflineResult<()> {
    let mut ready_tx = Some(ready_tx);
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => {
            let e = failure(OfflineErrorCode::WorkerStartFailed, "v2_spawn");
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(Err(e.clone()));
            }
            return Err(e);
        }
    };
    let mut stdin = child.stdin.take().expect("fixed launch always pipes stdin");
    let mut stdout = BufReader::new(
        child
            .stdout
            .take()
            .expect("fixed launch always pipes stdout"),
    );
    let mut stderr = child
        .stderr
        .take()
        .expect("fixed launch always pipes stderr");
    let stderr_job = tokio::spawn(async move {
        let mut chunk = [0u8; 4096];
        loop {
            match stderr.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
    let mut conversation = WorkerConversationV2::new(hello.clone())
        .map_err(|_| failure(OfflineErrorCode::ProtocolViolation, "v2_hello_shape"))?;
    let mut records = Vec::new();
    let result = async {
        budget.check().map_err(budget_failure)?;
        conversation.sent(&ParentFrameV2::Hello { hello:Box::new(hello.clone()) }).map_err(|_| failure(OfflineErrorCode::ProtocolViolation,"v2_hello_state"))?;
        tokio::select! { biased;
            result = stop_reason(&budget,&mut stop) => return result,
            result = v2_protocol::write_hello_v2(&mut stdin,&hello) => result?,
        }
        loop {
            let frame=tokio::select! { biased;
                result=stop_reason(&budget,&mut stop) => return result,
                result=v2_protocol::read_child_v2(&mut stdout) => result?.ok_or_else(|| failure(OfflineErrorCode::WorkerExited,"v2_stdout_eof"))?,
            };
            budget.check().map_err(budget_failure)?;
            conversation.received(&frame).map_err(|_| failure(OfflineErrorCode::ProtocolViolation,"v2_child_state"))?;
            match frame {
                ChildFrameV2::Evidence{records:more,..} => records.extend(more),
                frame @ ChildFrameV2::Ready{..} => {
                    if let Some(tx)=ready_tx.take() {
                        tx.send(Ok(ObservedWorkerFactsV2{ready:frame,records:std::mem::take(&mut records)}))
                            .map_err(|_| failure(OfflineErrorCode::Cancelled,"v2_observer_dropped"))?;
                    }
                }
                ChildFrameV2::Error{..} => return Err(failure(OfflineErrorCode::EngineFailed,"v2_child_error")),
                ChildFrameV2::Final{..} => return Err(failure(OfflineErrorCode::ProtocolViolation,"v2_unexpected_final")),
            }
        }
    }.await;
    // Any logical completion enters cleanup while slot + PhysicalJob remain in the runner.
    if matches!(
        conversation.state(),
        WorkerConversationStateV2::AwaitingReady | WorkerConversationStateV2::ReadyObserved
    ) {
        let _ = tokio::time::timeout(
            GRACE,
            v2_protocol::write_cancel_v2(&mut stdin, &hello.reference),
        )
        .await;
    }
    drop(stdin);
    drop(stdout);
    let cleanup = cleanup_child(&mut child, stderr_job).await;
    let result = result.and(cleanup);
    if let Some(tx) = ready_tx.take() {
        let e = result
            .clone()
            .err()
            .unwrap_or_else(|| failure(OfflineErrorCode::Cancelled, "v2_observation_closed"));
        let _ = tx.send(Err(e));
    }
    result
}
async fn cleanup_child(child: &mut Child, mut stderr_job: JoinHandle<()>) -> OfflineResult<()> {
    let waited = match tokio::time::timeout(GRACE, child.wait()).await {
        Ok(result) => result
            .map_err(|_| failure(OfflineErrorCode::WorkerExited, "v2_wait"))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(failure(OfflineErrorCode::WorkerExited, "v2_exit_status"))
                }
            }),
        Err(_) => {
            let killed = child.start_kill();
            let waited = child.wait().await;
            killed
                .and(waited.map(|_| ()))
                .map_err(|_| failure(OfflineErrorCode::WorkerExited, "v2_kill_wait"))
        }
    };
    if tokio::time::timeout(GRACE, &mut stderr_job).await.is_err() {
        stderr_job.abort();
        let _ = stderr_job.await;
    }
    waited
}
