use app_core::ports::{
    physical_jobs::PhysicalJobs, service_test_execution::ServiceTestExecutionObservation,
};
use std::{
    sync::{Arc, Barrier, mpsc},
    time::Duration,
};

#[tokio::test]
async fn final_drain_requires_every_real_guard_and_admission_stays_closed() {
    let jobs = Arc::new(PhysicalJobs::default());
    let first = jobs.try_begin().unwrap();
    let second = jobs.try_begin().unwrap();
    jobs.close_admission().unwrap();
    jobs.close_admission().unwrap();
    assert_eq!(jobs.try_begin().unwrap_err().code, "physical_jobs_closed");
    drop(first);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), jobs.wait_for_physical_drain())
            .await
            .is_err()
    );
    drop(second);
    tokio::time::timeout(Duration::from_secs(2), jobs.wait_for_physical_drain())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(jobs.try_begin().unwrap_err().code, "physical_jobs_closed");
}

#[tokio::test]
async fn discarded_blocking_join_handle_does_not_claim_physical_drain() {
    let jobs = Arc::new(PhysicalJobs::default());
    let guard = jobs.try_begin().unwrap();
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let job = tokio::task::spawn_blocking(move || {
        let _physical = guard;
        entered.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(job);
    jobs.close_admission().unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), jobs.wait_for_physical_drain())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), jobs.wait_for_physical_drain())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn closing_and_registration_have_one_linearization_lock() {
    for _ in 0..64 {
        let jobs = Arc::new(PhysicalJobs::default());
        let barrier = Arc::new(Barrier::new(3));
        let registrant = jobs.clone();
        let ready = barrier.clone();
        let begin = std::thread::spawn(move || {
            ready.wait();
            registrant.try_begin()
        });
        let closer = jobs.clone();
        let ready = barrier.clone();
        let close = std::thread::spawn(move || {
            ready.wait();
            closer.close_admission()
        });
        barrier.wait();
        close.join().unwrap().unwrap();
        let guard = begin.join().unwrap();
        assert_eq!(jobs.try_begin().unwrap_err().code, "physical_jobs_closed");
        match guard {
            Ok(guard) => {
                assert!(
                    tokio::time::timeout(Duration::from_millis(1), jobs.wait_for_physical_drain())
                        .await
                        .is_err()
                );
                drop(guard);
            }
            Err(error) => assert_eq!(error.code, "physical_jobs_closed"),
        }
        tokio::time::timeout(Duration::from_secs(2), jobs.wait_for_physical_drain())
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn last_drop_before_or_after_subscription_cannot_lose_the_wakeup() {
    for poll_before_drop in [true, false].into_iter().cycle().take(128) {
        let jobs = Arc::new(PhysicalJobs::default());
        let guard = jobs.try_begin().unwrap();
        jobs.close_admission().unwrap();
        let waiter = jobs.clone();
        let wait = tokio::spawn(async move { waiter.wait_for_physical_drain().await });
        if poll_before_drop {
            tokio::task::yield_now().await;
        }
        drop(guard);
        tokio::time::timeout(Duration::from_secs(2), wait)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn existing_open_observation_wrapper_keeps_its_api_facts_and_guard_owner() {
    let observation = Arc::new(ServiceTestExecutionObservation::default());
    observation.wait_for_physical_drain().await.unwrap();
    observation.mark_credential_used().unwrap();
    observation.mark_transport_started(false).unwrap();
    let facts = observation.snapshot().unwrap();
    assert!(facts.credential_used && facts.request_started);
    assert!(facts.google_lease.is_none());
    let guard = observation.begin_physical_job().unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(10),
            observation.wait_for_physical_drain()
        )
        .await
        .is_err()
    );
    drop(guard);
    observation.wait_for_physical_drain().await.unwrap();
    // Waiting never implicitly closes the original wrapper's admission.
    let guard = observation.begin_physical_job().unwrap();
    let weak = Arc::downgrade(&observation);
    drop(observation);
    assert!(weak.upgrade().is_some());
    drop(guard);
    assert!(weak.upgrade().is_none());
}
