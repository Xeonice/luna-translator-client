use app_core::{
    contracts::AppError,
    ports::{
        google_authorization::GoogleBudget,
        operation_budget::{OperationBudget, OperationTerminalDisposition},
    },
};
use std::{
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
fn budget() -> OperationBudget {
    OperationBudget::new(Duration::from_secs(30)).unwrap()
}
#[test]
fn cancellation_wins_before_terminal_and_never_runs_the_commit_callback() {
    let control = budget();
    let clone = control.clone();
    let ran = AtomicBool::new(false);
    clone.cancel();
    assert_eq!(
        control
            .with_terminal_fence(|| {
                ran.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap_err()
            .code,
        "operation_cancelled"
    );
    assert!(!ran.load(Ordering::SeqCst));
    assert!(!control.is_committed());
}
#[test]
fn terminal_decision_wins_and_concurrent_cancel_cannot_relabel_it() {
    let control = budget();
    let commit = control.clone();
    let cancel = control.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let committer = std::thread::spawn(move || {
        commit.with_terminal_fence(|| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            // A real repository repeats this check directly before SQL COMMIT.
            commit.check()?;
            Ok("actual-terminal-result")
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let ready = Arc::new(Barrier::new(2));
    let waiter = ready.clone();
    let (cancel_tx, cancel_rx) = mpsc::channel();
    let canceller = std::thread::spawn(move || {
        waiter.wait();
        cancel.cancel();
        cancel_tx.send(cancel.is_committed()).unwrap();
    });
    ready.wait();
    assert!(cancel_rx.recv_timeout(Duration::from_millis(30)).is_err());
    release_tx.send(()).unwrap();
    assert_eq!(committer.join().unwrap().unwrap(), "actual-terminal-result");
    assert!(cancel_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    canceller.join().unwrap();
    assert!(control.is_committed());
    assert_eq!(control.check().unwrap_err().code, "operation_completed");
    assert_eq!(
        control.with_terminal_fence(|| Ok(())).unwrap_err().code,
        "operation_completed"
    );
}
#[test]
fn rolled_back_terminal_keeps_the_operation_cancellable_and_isolated() {
    let control = budget();
    let unrelated = budget();
    let error = control
        .with_terminal_fence(|| {
            Err::<(), _>(AppError::new(
                "storage_failure",
                "storage",
                "controlled rollback",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, "storage_failure");
    control.check().unwrap();
    assert!(!control.is_committed());
    control.cancel();
    assert_eq!(control.check().unwrap_err().code, "operation_cancelled");
    unrelated.check().unwrap();
    assert!(!unrelated.is_committed());
}
#[test]
fn deadline_is_rechecked_at_the_terminal_decision_and_poison_is_failclosed() {
    let control = OperationBudget::new(Duration::from_millis(10)).unwrap();
    let result = control.with_terminal_fence(|| {
        std::thread::sleep(Duration::from_millis(20));
        control.check()?;
        Ok(())
    });
    assert_eq!(result.unwrap_err().code, "operation_timeout");
    assert!(!control.is_committed());
    let poisoned = budget();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = poisoned.with_terminal_fence::<()>(|| panic!("controlled terminal panic"));
        }))
        .is_err()
    );
    assert_eq!(poisoned.check().unwrap_err().code, "operation_state");
    poisoned.cancel();
    assert_eq!(poisoned.check().unwrap_err().code, "operation_state");
}
#[test]
fn google_wrapper_shares_the_real_fence_and_preserves_cancellation_codes() {
    let operation = budget();
    let google = GoogleBudget::from_operation_budget(operation.clone());
    let clone = google.operation_budget();
    clone.cancel();
    assert_eq!(google.check().unwrap_err().code, "authorization_cancelled");
    let operation = budget();
    let google = GoogleBudget::from_operation_budget(operation.clone());
    google
        .with_terminal_fence(|| {
            operation.check()?;
            Ok(())
        })
        .unwrap();
    google.cancel();
    assert!(operation.is_committed());
    assert_eq!(google.check().unwrap_err().code, "authorization_completed");
}
#[test]
fn stopped_operations_can_record_only_their_actual_terminal_fact() {
    let control = budget();
    assert_eq!(
        control
            .with_terminal_fence_for(OperationTerminalDisposition::Cancelled, || Ok(()))
            .unwrap_err()
            .code,
        "operation_terminal_mismatch"
    );
    control.cancel();
    assert_eq!(
        control.with_terminal_fence(|| Ok(())).unwrap_err().code,
        "operation_cancelled"
    );
    assert_eq!(
        control
            .with_terminal_fence_for(OperationTerminalDisposition::TimedOut, || Ok(()))
            .unwrap_err()
            .code,
        "operation_terminal_mismatch"
    );
    control
        .with_terminal_fence_for(OperationTerminalDisposition::Cancelled, || {
            control.check_terminal(OperationTerminalDisposition::Cancelled)?;
            Ok(())
        })
        .unwrap();
    assert!(control.is_committed());
    let expired = OperationBudget::new(Duration::ZERO).unwrap();
    assert_eq!(
        expired.with_terminal_fence(|| Ok(())).unwrap_err().code,
        "operation_timeout"
    );
    expired
        .with_terminal_fence_for(OperationTerminalDisposition::TimedOut, || {
            expired.check_terminal(OperationTerminalDisposition::TimedOut)?;
            Ok(())
        })
        .unwrap();
    assert!(expired.is_committed());
}

#[test]
fn preparation_can_only_shorten_the_original_operation_deadline() {
    let control = OperationBudget::new(Duration::from_secs(30)).unwrap();
    let original = control.deadline();
    control
        .restrict_timeout_from_start(Duration::from_secs(60))
        .unwrap();
    assert_eq!(control.deadline(), original);
    control
        .restrict_timeout_from_start(Duration::from_secs(1))
        .unwrap();
    let shorter = control.deadline();
    assert!(shorter < original);
    control
        .restrict_timeout_from_start(Duration::from_secs(20))
        .unwrap();
    assert_eq!(control.deadline(), shorter);
    control.cancel();
    assert_eq!(
        control
            .restrict_timeout_from_start(Duration::from_secs(5))
            .unwrap_err()
            .code,
        "operation_cancelled"
    );
}
#[tokio::test]
async fn dropping_a_join_handle_does_not_claim_physical_io_has_drained() {
    use app_core::ports::service_test_execution::ServiceTestExecutionObservation;
    let facts = std::sync::Arc::new(ServiceTestExecutionObservation::default());
    let job = facts.begin_physical_job().unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    let task = tokio::task::spawn_blocking(move || {
        let _physical = job;
        wait.recv().unwrap();
    });
    drop(task);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), facts.wait_for_physical_drain())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), facts.wait_for_physical_drain())
        .await
        .unwrap()
        .unwrap();
    facts.mark_credential_used().unwrap();
    let snapshot = facts.snapshot().unwrap();
    assert!(snapshot.credential_used);
    assert!(!snapshot.request_started);
}
