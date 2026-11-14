//! Filesystem preparation only. No engine worker, business queue or authority grant.
use super::resources::{self, MAX_RESOURCE_BYTES};
use super::{
    MAX_RESOURCE_FILES, MAX_ROUTE_HOPS, OfflineErrorCode, OfflineFailure, OfflineResult,
    ResourceFile,
};
use app_core::ports::operation_budget::OperationBudget;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

/// Callers may tighten the production limits, never expand them.
#[derive(Debug, Clone, Copy)]
pub struct InspectionLimits {
    pub max_selected_trees: usize,
    pub max_entries: usize,
    pub max_files: usize,
    pub max_depth: usize,
    pub max_total_bytes: u64,
}
impl Default for InspectionLimits {
    fn default() -> Self {
        Self {
            max_selected_trees: MAX_ROUTE_HOPS,
            max_entries: MAX_RESOURCE_FILES * 2,
            max_files: MAX_RESOURCE_FILES,
            max_depth: 32,
            max_total_bytes: MAX_RESOURCE_BYTES,
        }
    }
}
impl InspectionLimits {
    pub(super) fn validate(&self) -> OfflineResult<()> {
        let cap = Self::default();
        if self.max_selected_trees == 0
            || self.max_selected_trees > cap.max_selected_trees
            || self.max_entries == 0
            || self.max_entries > cap.max_entries
            || self.max_files == 0
            || self.max_files > cap.max_files
            || self.max_depth > cap.max_depth
            || self.max_total_bytes > cap.max_total_bytes
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidRequest,
                "inspection_limits",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct InspectionRequest {
    pub root: PathBuf,
    pub selected: Vec<String>,
}
/// Share this instance (or its clones) at the resource-owner composition root.
/// It has one physical inspection permit and no internal waiting/business queue.
#[derive(Clone)]
pub struct ResourceInspector {
    slot: Arc<Semaphore>,
    limits: InspectionLimits,
}
impl Default for ResourceInspector {
    fn default() -> Self {
        Self {
            slot: Arc::new(Semaphore::new(1)),
            limits: InspectionLimits::default(),
        }
    }
}
impl ResourceInspector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_limits(limits: InspectionLimits) -> OfflineResult<Self> {
        limits.validate()?;
        Ok(Self {
            slot: Arc::new(Semaphore::new(1)),
            limits,
        })
    }
    pub async fn inspect(
        &self,
        request: InspectionRequest,
        budget: Duration,
        cancellation: watch::Receiver<bool>,
    ) -> OfflineResult<Vec<ResourceFile>> {
        let limits = self.limits;
        self.run_blocking(budget, cancellation, move |check| {
            resources::inspect_selected_trees_checked(
                &request.root,
                &request.selected,
                limits,
                check,
            )
        })
        .await
    }
    pub(super) fn limits(&self) -> InspectionLimits {
        self.limits
    }
    /// The exact Core lifetime is shared, not translated to a fresh duration or
    /// cancellation channel. The same physical permit is retained by the job.
    pub(super) async fn run_with_budget<T: Send + 'static, F>(
        &self,
        budget: OperationBudget,
        operation: F,
    ) -> OfflineResult<T>
    where
        F: FnOnce(&dyn Fn() -> OfflineResult<()>) -> OfflineResult<T> + Send + 'static,
    {
        budget.check().map_err(operation_failure)?;
        if budget.deadline().saturating_duration_since(Instant::now()) > Duration::from_secs(600) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidRequest,
                "inspection_deadline",
            ));
        }
        let permit = self.slot.clone().try_acquire_owned().map_err(|_| {
            OfflineFailure::new(OfflineErrorCode::WorkerBusy, "resource_inspection_busy")
        })?;
        let control = Arc::new(SharedControl {
            stopped: Arc::new(AtomicBool::new(false)),
            budget: budget.clone(),
            _permit: permit,
        });
        let _guard = StopOnDrop(control.stopped.clone());
        let worker_control = control.clone();
        let work = tokio::task::spawn_blocking(move || {
            worker_control.check()?;
            let result = operation(&|| worker_control.check());
            worker_control.check()?;
            result
        });
        tokio::select! {
            biased;
            _ = budget.cancelled() => {
                Err(budget.check().err().map(operation_failure).unwrap_or_else(|| OfflineFailure::new(OfflineErrorCode::Cancelled, "resource_inspection")))
            }
            result = tokio::time::timeout_at(tokio::time::Instant::from_std(budget.deadline()), work) => {
                let result = result.map_err(|_| OfflineFailure::new(OfflineErrorCode::DeadlineExceeded, "resource_inspection"))?
                    .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_inspection_join"))??;
                control.check()?;
                Ok(result)
            }
        }
    }
    async fn run_blocking<T: Send + 'static, F>(
        &self,
        budget: Duration,
        mut cancellation: watch::Receiver<bool>,
        operation: F,
    ) -> OfflineResult<T>
    where
        F: FnOnce(&dyn Fn() -> OfflineResult<()>) -> OfflineResult<T> + Send + 'static,
    {
        if *cancellation.borrow() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::Cancelled,
                "resource_inspection",
            ));
        }
        if budget.is_zero() || budget > Duration::from_secs(600) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidRequest,
                "inspection_deadline",
            ));
        }
        let permit = self.slot.clone().try_acquire_owned().map_err(|_| {
            OfflineFailure::new(OfflineErrorCode::WorkerBusy, "resource_inspection_busy")
        })?;
        let control = Arc::new(Control {
            stopped: Arc::new(AtomicBool::new(false)),
            cancellation: cancellation.clone(),
            deadline: Instant::now() + budget,
            _permit: permit,
        });
        let _guard = StopOnDrop(control.stopped.clone());
        let worker_control = control.clone();
        let work = tokio::task::spawn_blocking(move || {
            worker_control.check()?;
            let result = operation(&|| worker_control.check());
            worker_control.check()?;
            result
        });
        tokio::select! {
            biased;
            _ = cancelled(&mut cancellation) => Err(OfflineFailure::new(OfflineErrorCode::Cancelled, "resource_inspection")),
            result = tokio::time::timeout_at(tokio::time::Instant::from_std(control.deadline), work) => {
                let result = result.map_err(|_| OfflineFailure::new(OfflineErrorCode::DeadlineExceeded, "resource_inspection"))?
                    .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_inspection_join"))??;
                control.check()?;
                Ok(result)
            }
        }
    }
}
struct Control {
    stopped: Arc<AtomicBool>,
    cancellation: watch::Receiver<bool>,
    deadline: Instant,
    _permit: OwnedSemaphorePermit,
}
impl Control {
    fn check(&self) -> OfflineResult<()> {
        if self.stopped.load(Ordering::Acquire) || *self.cancellation.borrow() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::Cancelled,
                "resource_inspection",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(OfflineFailure::new(
                OfflineErrorCode::DeadlineExceeded,
                "resource_inspection",
            ));
        }
        Ok(())
    }
}
struct SharedControl {
    stopped: Arc<AtomicBool>,
    budget: OperationBudget,
    _permit: OwnedSemaphorePermit,
}
impl SharedControl {
    fn check(&self) -> OfflineResult<()> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::Cancelled,
                "resource_inspection",
            ));
        }
        self.budget.check().map_err(operation_failure)
    }
}
pub(super) fn operation_failure(error: app_core::contracts::AppError) -> OfflineFailure {
    let code = match error.code.as_str() {
        "operation_cancelled" | "operation_completed" => OfflineErrorCode::Cancelled,
        "operation_timeout" => OfflineErrorCode::DeadlineExceeded,
        _ => OfflineErrorCode::InvalidRequest,
    };
    OfflineFailure::new(code, "resource_inspection")
}
struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
async fn cancelled(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow() {
            return;
        }
        if receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // A blocked channel is a controlled surrogate for an uninterruptible syscall,
    // not a claim that the OS can cancel actual filesystem I/O.
    #[tokio::test]
    async fn timeout_does_not_release_the_physical_slot_before_blocking_work_returns() {
        let inspector = ResourceInspector::new();
        let (_tx, rx) = watch::channel(false);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let running = {
            let inspector = inspector.clone();
            tokio::spawn(async move {
                inspector
                    .run_blocking(Duration::from_millis(100), rx, move |check| {
                        check()?;
                        let _ = entered_tx.send(());
                        release_rx.recv().unwrap();
                        check()?;
                        Ok(())
                    })
                    .await
            })
        };
        entered_rx.await.unwrap();
        assert_eq!(
            running.await.unwrap().unwrap_err().code,
            OfflineErrorCode::DeadlineExceeded
        );
        assert_eq!(inspector.slot.available_permits(), 0);
        let (_tx, rx) = watch::channel(false);
        assert_eq!(
            inspector
                .run_blocking(Duration::from_secs(1), rx, |_| Ok(()))
                .await
                .unwrap_err()
                .code,
            OfflineErrorCode::WorkerBusy
        );
        release_tx.send(()).unwrap();
        for _ in 0..100 {
            if inspector.slot.available_permits() == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("physical slot not reclaimed");
    }
    #[tokio::test]
    async fn cancelled_and_dropped_futures_keep_the_slot_until_cooperative_work_exits() {
        for abort in [false, true] {
            let inspector = ResourceInspector::new();
            let (tx, rx) = watch::channel(false);
            let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let running = {
                let inspector = inspector.clone();
                tokio::spawn(async move {
                    inspector
                        .run_blocking(Duration::from_secs(5), rx, move |check| {
                            check()?;
                            let _ = entered_tx.send(());
                            release_rx.recv().unwrap();
                            check()?;
                            Ok(())
                        })
                        .await
                })
            };
            entered_rx.await.unwrap();
            if abort {
                running.abort();
                assert!(running.await.unwrap_err().is_cancelled());
            } else {
                tx.send(true).unwrap();
                assert_eq!(
                    running.await.unwrap().unwrap_err().code,
                    OfflineErrorCode::Cancelled
                );
            }
            assert_eq!(inspector.slot.available_permits(), 0);
            let (_tx, rx) = watch::channel(false);
            assert_eq!(
                inspector
                    .run_blocking(Duration::from_secs(1), rx, |_| Ok(()))
                    .await
                    .unwrap_err()
                    .code,
                OfflineErrorCode::WorkerBusy
            );
            release_tx.send(()).unwrap();
            for _ in 0..100 {
                if inspector.slot.available_permits() == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            assert_eq!(inspector.slot.available_permits(), 1);
        }
    }
}

#[cfg(test)]
mod shared_budget_tests {
    use super::*;
    // A channel models a physically blocked syscall. These test permit lifetime
    // and the real Core budget, not the OS's ability to interrupt filesystem I/O.
    #[tokio::test]
    async fn shared_core_budget_cancel_timeout_and_drop_retain_physical_permit_until_drain() {
        for stop in ["cancel", "timeout", "drop"] {
            let inspector = ResourceInspector::new();
            let budget = OperationBudget::new(if stop == "timeout" {
                Duration::from_millis(100)
            } else {
                Duration::from_secs(5)
            })
            .unwrap();
            let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let running = {
                let inspector = inspector.clone();
                let budget = budget.clone();
                tokio::spawn(async move {
                    inspector
                        .run_with_budget(budget, move |check| {
                            check()?;
                            let _ = entered_tx.send(());
                            release_rx.recv().unwrap();
                            check()?;
                            Ok(())
                        })
                        .await
                })
            };
            entered_rx.await.unwrap();
            match stop {
                "cancel" => {
                    budget.cancel();
                    assert_eq!(
                        running.await.unwrap().unwrap_err().code,
                        OfflineErrorCode::Cancelled
                    );
                }
                "timeout" => assert_eq!(
                    running.await.unwrap().unwrap_err().code,
                    OfflineErrorCode::DeadlineExceeded
                ),
                _ => {
                    running.abort();
                    assert!(running.await.unwrap_err().is_cancelled());
                }
            }
            assert_eq!(inspector.slot.available_permits(), 0);
            let (_tx, rx) = watch::channel(false);
            assert_eq!(
                inspector
                    .run_blocking(Duration::from_secs(1), rx, |_| Ok(()))
                    .await
                    .unwrap_err()
                    .code,
                OfflineErrorCode::WorkerBusy
            );
            release_tx.send(()).unwrap();
            for _ in 0..100 {
                if inspector.slot.available_permits() == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            assert_eq!(inspector.slot.available_permits(), 1);
        }
    }
    #[tokio::test]
    async fn terminal_operation_cannot_return_new_material_after_its_shared_budget_commits() {
        let inspector = ResourceInspector::new();
        let budget = OperationBudget::new(Duration::from_secs(5)).unwrap();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let running = {
            let inspector = inspector.clone();
            let budget = budget.clone();
            tokio::spawn(async move {
                inspector
                    .run_with_budget(budget, move |check| {
                        check()?;
                        let _ = entered_tx.send(());
                        release_rx.recv().unwrap();
                        check()?;
                        Ok(())
                    })
                    .await
            })
        };
        entered_rx.await.unwrap();
        budget.with_terminal_fence(|| Ok(())).unwrap();
        assert_eq!(
            running.await.unwrap().unwrap_err().code,
            OfflineErrorCode::Cancelled
        );
        assert_eq!(inspector.slot.available_permits(), 0);
        release_tx.send(()).unwrap();
        for _ in 0..100 {
            if inspector.slot.available_permits() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(inspector.slot.available_permits(), 1);
    }
}
