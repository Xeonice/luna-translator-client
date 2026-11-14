use app_core::contracts::*;
use app_core::{
    application::service_discovery::{
        LiveServiceModelDiscoveries, ServiceModelDiscoveryController, model_discovery_result,
    },
    ports::{
        physical_jobs::PhysicalJobs, runtime::PortFuture,
        service_configuration::SavedServiceSourceProof, service_discovery::*,
    },
};
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::Semaphore;

fn input() -> ServiceModelDiscoveryInput {
    serde_json::from_value(json!({"operationId":"model-list", "request":{"expectedProfileGeneration":1,"source":{"kind":"http","source":{"revisionId":"saved","modelId":"model"}},"purpose":"translation"}})).unwrap()
}
#[test]
fn discovery_accepts_only_a_saved_source_not_a_connection_or_credential() {
    let original = input();
    original.validate().unwrap();
    for key in ["connection", "credential", "url", "budget", "writerProof"] {
        let mut value = serde_json::to_value(&original).unwrap();
        value["request"][key] = json!("untrusted");
        assert!(serde_json::from_value::<ServiceModelDiscoveryInput>(value).is_err());
    }
    let mut value = serde_json::to_value(original).unwrap();
    value["request"]["expectedProfileGeneration"] = json!(0);
    assert!(serde_json::from_value::<ServiceModelDiscoveryInput>(value).is_err());
}
#[test]
fn new_endpoint_identity_bounds_use_utf8_bytes_and_do_not_change_old_id_policy() {
    let mut request = input();
    request.operation_id = "界".repeat(42) + "ab";
    assert_eq!(request.operation_id.len(), 128);
    request.validate().unwrap();
    request.operation_id.push('c');
    assert_eq!(
        request.validate().unwrap_err().code,
        "discovery_identity_too_large"
    );
    // Existing APIs keep their unchanged nonempty/no-control contract.
    app_core::domain::require_id(&request.operation_id).unwrap();
    request.operation_id = "valid".into();
    let app_core::domain::model_config::service_graph::SavedServiceRef::Http { source } =
        &mut request.request.source
    else {
        unreachable!()
    };
    source.revision_id = "x".repeat(512);
    source.model_id = "界".repeat(170) + "ab";
    request.validate().unwrap();
    let app_core::domain::model_config::service_graph::SavedServiceRef::Http { source } =
        &mut request.request.source
    else {
        unreachable!()
    };
    source.model_id.push('c');
    assert!(request.validate().is_err());
    for identity in ["", "bad\nidentity"] {
        request.operation_id = identity.into();
        assert!(request.validate().is_err());
    }
}

struct CountBackend {
    calls: AtomicUsize,
    gate: Option<Arc<Semaphore>>,
    physical: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
    restrict: bool,
}
impl CountBackend {
    fn new(gate: Option<Arc<Semaphore>>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            gate,
            physical: Mutex::new(None),
            restrict: false,
        })
    }
}
impl ServiceModelDiscoveryBackend for CountBackend {
    fn prepare(
        &self,
        op: Arc<ServiceModelDiscoveryOperation>,
        jobs: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, Arc<PreparedServiceModelDiscovery>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let physical = self.physical.lock().unwrap().take();
            if let Some((entered, release)) = physical {
                let guard = jobs.try_begin()?;
                tokio::task::spawn_blocking(move || {
                    let _guard = guard;
                    entered.send(()).unwrap();
                    release.recv_timeout(Duration::from_secs(4)).unwrap();
                })
                .await
                .unwrap();
            }
            if self.restrict {
                let budget = op.budget();
                let initial = budget.deadline();
                tokio::time::sleep(Duration::from_millis(5)).await;
                budget.restrict_timeout_from_start(Duration::from_millis(15))?;
                assert_eq!(
                    budget.deadline(),
                    initial - Duration::from_secs(15) + Duration::from_millis(15)
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            if let Some(gate) = &self.gate {
                gate.acquire().await.unwrap().forget();
            }
            Err(AppError::new(
                "discovery_unsupported",
                "service_model_discovery",
                "controlled backend without list",
            ))
        })
    }
    fn discover(
        &self,
        _: Arc<PreparedServiceModelDiscovery>,
        _: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, Vec<DiscoveredServiceModel>> {
        Box::pin(async { panic!("unsupported source cannot discover") })
    }
    fn complete(
        &self,
        _: Arc<PreparedServiceModelDiscovery>,
        _: ServiceModelDiscoveryOutcome,
        _: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, ServiceModelDiscoveryResult> {
        Box::pin(async { panic!("unsupported source cannot complete") })
    }
}
fn controller(
    backend: Arc<dyn ServiceModelDiscoveryBackend>,
) -> Arc<ServiceModelDiscoveryController> {
    ServiceModelDiscoveryController::new(backend, Arc::new(LiveServiceModelDiscoveries::default()))
}
async fn calls(backend: &CountBackend, n: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while backend.calls.load(Ordering::SeqCst) < n {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
fn cancel(input: &ServiceModelDiscoveryInput) -> CancelServiceModelDiscoveryInput {
    CancelServiceModelDiscoveryInput {
        operation_id: input.operation_id.clone(),
        expected_profile_generation: input.request.expected_profile_generation,
    }
}

#[tokio::test]
async fn preparing_operation_is_cancellable_but_unknown_ids_never_become_tombstones() {
    let backend = CountBackend::new(Some(Arc::new(Semaphore::new(0))));
    let app = controller(backend.clone());
    assert_eq!(
        app.cancel(cancel(&input())).unwrap_err().code,
        "discovery_operation_unknown"
    );
    let owner = app.clone();
    let run = tokio::spawn(async move { owner.run(input()).await });
    calls(&backend, 1).await;
    assert_eq!(
        app.cancel(cancel(&input())).unwrap().disposition,
        ServiceModelDiscoveryCancelDisposition::Requested
    );
    assert!(matches!(
        run.await.unwrap().unwrap().outcome,
        ServiceModelDiscoveryOutcome::Cancelled
    ));
    assert!(matches!(
        app.run(input()).await.unwrap().outcome,
        ServiceModelDiscoveryOutcome::Cancelled
    ));
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn dropping_the_caller_cannot_hide_a_real_blocking_prepare_from_shutdown_drain() {
    let backend = CountBackend::new(None);
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    *backend.physical.lock().unwrap() = Some((entered, wait));
    let app = controller(backend);
    let owner = app.clone();
    let caller = tokio::spawn(async move { owner.run(input()).await });
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(3)))
        .await
        .unwrap()
        .unwrap();
    caller.abort();
    let owner = app.clone();
    let shutdown = tokio::spawn(async move { owner.stop_and_drain().await });
    tokio::time::sleep(Duration::from_millis(15)).await;
    assert!(!shutdown.is_finished());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), shutdown)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(
        app.run(input()).await.unwrap().outcome,
        ServiceModelDiscoveryOutcome::Cancelled
    ));
}
#[tokio::test]
async fn original_request_replay_has_one_preflight_and_conflicting_input_is_rejected() {
    let gate = Arc::new(Semaphore::new(0));
    let backend = CountBackend::new(Some(gate.clone()));
    let app = controller(backend.clone());
    let owner = app.clone();
    let first = tokio::spawn(async move { owner.run(input()).await });
    calls(&backend, 1).await;
    let owner = app.clone();
    let repeated = tokio::spawn(async move { owner.run(input()).await });
    let mut different = input();
    different.request.purpose = CatalogPurpose::Grammar;
    assert_eq!(
        app.run(different).await.unwrap_err().code,
        "operation_conflict"
    );
    let mut wrong = cancel(&input());
    wrong.expected_profile_generation = 2.try_into().unwrap();
    assert_eq!(
        app.cancel(wrong).unwrap_err().code,
        "profile_generation_conflict"
    );
    gate.add_permits(1);
    assert_eq!(
        first.await.unwrap().unwrap(),
        repeated.await.unwrap().unwrap()
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn saved_timeout_shortens_the_original_registration_budget_instead_of_resetting_it() {
    let backend = Arc::new(CountBackend {
        calls: AtomicUsize::new(0),
        gate: None,
        physical: Mutex::new(None),
        restrict: true,
    });
    let app = controller(backend.clone());
    assert!(matches!(
        app.run(input()).await.unwrap().outcome,
        ServiceModelDiscoveryOutcome::TimedOut
    ));
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn eight_active_operations_do_not_admit_a_ninth_or_charge_replays_another_slot() {
    let gate = Arc::new(Semaphore::new(0));
    let backend = CountBackend::new(Some(gate));
    let app = controller(backend.clone());
    let mut runs = Vec::new();
    for i in 0..8 {
        let mut request = input();
        request.operation_id = format!("active-{i}");
        let owner = app.clone();
        runs.push(tokio::spawn(async move { owner.run(request).await }));
    }
    calls(&backend, 8).await;
    let mut ninth = input();
    ninth.operation_id = "active-9".into();
    assert_eq!(app.run(ninth).await.unwrap_err().code, "discovery_busy");
    let mut repeat = input();
    repeat.operation_id = "active-0".into();
    let owner = app.clone();
    let replay = tokio::spawn(async move { owner.run(repeat).await });
    for i in 0..8 {
        let mut request = input();
        request.operation_id = format!("active-{i}");
        app.cancel(cancel(&request)).unwrap();
    }
    for run in runs {
        assert!(matches!(
            run.await.unwrap().unwrap().outcome,
            ServiceModelDiscoveryOutcome::Cancelled
        ));
    }
    assert!(matches!(
        replay.await.unwrap().unwrap().outcome,
        ServiceModelDiscoveryOutcome::Cancelled
    ));
    assert_eq!(backend.calls.load(Ordering::SeqCst), 8);
}
#[tokio::test]
async fn thirty_third_refresh_executes_while_expired_replay_never_prepares_again() {
    let backend = CountBackend::new(None);
    let app = controller(backend.clone());
    for i in 0..33 {
        let mut request = input();
        request.operation_id = format!("recent-{i}");
        assert!(matches!(
            app.run(request).await.unwrap().outcome,
            ServiceModelDiscoveryOutcome::Unsupported { .. }
        ));
    }
    let mut old = input();
    old.operation_id = "recent-0".into();
    let result = app.run(old.clone()).await.unwrap();
    let ServiceModelDiscoveryOutcome::ResultExpired { reason } = result.outcome else {
        panic!("evicted payload must expire")
    };
    assert_eq!(reason.code, "discovery_result_expired");
    old.request.purpose = CatalogPurpose::Grammar;
    assert_eq!(app.run(old).await.unwrap_err().code, "operation_conflict");
    let mut cached = input();
    cached.operation_id = "recent-32".into();
    assert!(matches!(
        app.run(cached).await.unwrap().outcome,
        ServiceModelDiscoveryOutcome::Unsupported { .. }
    ));
    assert_eq!(backend.calls.load(Ordering::SeqCst), 33);
}
#[tokio::test]
async fn identity_capacity_only_rejects_new_ids_and_keeps_old_conflicts_and_expiry() {
    let backend = CountBackend::new(None);
    let app = controller(backend.clone());
    for i in 0..4096 {
        let mut request = input();
        request.operation_id = format!("identity-{i}");
        app.run(request).await.unwrap();
    }
    let mut fresh = input();
    fresh.operation_id = "identity-4096".into();
    assert_eq!(
        app.run(fresh).await.unwrap_err().code,
        "discovery_identity_capacity"
    );
    let mut old = input();
    old.operation_id = "identity-0".into();
    assert!(matches!(
        app.run(old.clone()).await.unwrap().outcome,
        ServiceModelDiscoveryOutcome::ResultExpired { .. }
    ));
    assert_eq!(
        app.cancel(cancel(&old)).unwrap().disposition,
        ServiceModelDiscoveryCancelDisposition::AlreadyTerminal
    );
    old.request.purpose = CatalogPurpose::ContextWord;
    assert_eq!(app.run(old).await.unwrap_err().code, "operation_conflict");
    assert_eq!(backend.calls.load(Ordering::SeqCst), 4096);
}

struct ReadyBackend {
    live: Arc<LiveServiceModelDiscoveries>,
    after_commit: Option<Arc<Semaphore>>,
    committed: AtomicUsize,
    prepared_weak: Mutex<Option<Weak<PreparedServiceModelDiscovery>>>,
    rollback: bool,
}
impl ServiceModelDiscoveryBackend for ReadyBackend {
    fn prepare(
        &self,
        op: Arc<ServiceModelDiscoveryOperation>,
        _: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, Arc<PreparedServiceModelDiscovery>> {
        Box::pin(async move {
            let connection=serde_json::from_value(json!({"connectionId":"controlled","displayName":"controlled Core backend, no SQL","protocol":"ollama_native","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:11434","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true})).unwrap();
            let p=Arc::new(PreparedServiceModelDiscovery {
                source:SavedServiceSourceProof {source:op.input().request.source.clone(),logical_resource_id:"http:model".into(),codec_version:3,content_sha256:"controlled".into(),seal_sha256:"controlled".into(),canonical_control_id:Some("controlled".into())},profile_id:"core-test".into(),profile_generation:op.input().request.expected_profile_generation,server_instance_id:"controlled-no-sql".into(),connection,http_options:app_core::domain::model_config::service_graph::HttpServiceOptionsV1::OllamaNative {},origin:app_core::domain::model_config::service_graph::ConnectionOrigin::Custom {copied_from:None,site:None},authentication:ServiceDiscoveryAuthentication::None,repository_witness:Arc::new(()),operation:op.clone(),
            });
            *self.prepared_weak.lock().unwrap() = Some(Arc::downgrade(&p));
            self.live.attach_prepared(&op, p.clone())?;
            Ok(p)
        })
    }
    fn discover(
        &self,
        _: Arc<PreparedServiceModelDiscovery>,
        _: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, Vec<DiscoveredServiceModel>> {
        Box::pin(async {
            Ok(vec![DiscoveredServiceModel {
                id: "manual-model".into(),
            }])
        })
    }
    fn complete(
        &self,
        p: Arc<PreparedServiceModelDiscovery>,
        outcome: ServiceModelDiscoveryOutcome,
        jobs: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, ServiceModelDiscoveryResult> {
        Box::pin(async move {
            // Controlled Core terminal adapter only; real SQL/writer is tested in Infra.
            let physical = jobs.try_begin()?;
            let budget = self.live.prepared_budget(&p)?;
            let result = model_discovery_result(p.operation.input(), outcome);
            budget.with_terminal_fence(|| {
                self.live.accept_validated_terminal(&p, result.clone())?;
                if self.rollback {
                    return Err(AppError::new(
                        "controlled_terminal_rollback",
                        "service_model_discovery",
                        "controlled rollback after provisional decision",
                    ));
                }
                budget.check()?;
                Ok(())
            })?;
            self.committed.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = &self.after_commit {
                gate.acquire().await.unwrap().forget();
            }
            drop(physical);
            Ok(result)
        })
    }
}
#[tokio::test]
async fn committed_result_waits_for_final_physical_job_and_releases_prepared_pins() {
    let live = Arc::new(LiveServiceModelDiscoveries::default());
    let gate = Arc::new(Semaphore::new(0));
    let backend = Arc::new(ReadyBackend {
        live: live.clone(),
        after_commit: Some(gate.clone()),
        committed: AtomicUsize::new(0),
        prepared_weak: Mutex::new(None),
        rollback: false,
    });
    let app = ServiceModelDiscoveryController::new(backend.clone(), live.clone());
    let owner = app.clone();
    let run = tokio::spawn(async move { owner.run(input()).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while backend.committed.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!run.is_finished());
    assert_eq!(live.active_pins().unwrap().len(), 1);
    assert_eq!(
        app.cancel(cancel(&input())).unwrap().disposition,
        ServiceModelDiscoveryCancelDisposition::AlreadyTerminal
    );
    let owner = app.clone();
    let duplicate = tokio::spawn(async move { owner.run(input()).await });
    gate.add_permits(1);
    let result = run.await.unwrap().unwrap();
    assert!(matches!(
        result.outcome,
        ServiceModelDiscoveryOutcome::Available { .. }
    ));
    assert_eq!(result, duplicate.await.unwrap().unwrap());
    assert!(live.active_pins().unwrap().is_empty());
    assert!(
        backend
            .prepared_weak
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_none()
    );
    assert_eq!(backend.committed.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rejected_terminal_fence_cannot_leave_a_provisional_success_or_stuck_operation() {
    let live = Arc::new(LiveServiceModelDiscoveries::default());
    let backend = Arc::new(ReadyBackend {
        live: live.clone(),
        after_commit: None,
        committed: AtomicUsize::new(0),
        prepared_weak: Mutex::new(None),
        rollback: true,
    });
    let app = ServiceModelDiscoveryController::new(backend, live.clone());
    let result = app.run(input()).await.unwrap();
    let ServiceModelDiscoveryOutcome::Failed { reason } = result.outcome else {
        panic!("provisional models are not success")
    };
    assert_eq!(reason.code, "controlled_terminal_rollback");
    assert!(live.active_pins().unwrap().is_empty());
    app.stop_and_drain().await.unwrap();
}
