//! Explicit, supervised saved-service tests. These never enter the reading queue.
use crate::{
    application::google_test_operations::{LiveServiceTestOperations, ServiceTestPreparationGuard},
    contracts::*,
    ports::{
        IdentitySource, ProductRepository, operation_budget::OperationBudget,
        runtime::ModelGateway, service_test_execution::ServiceTestExecutionObservation,
        service_testing::*,
    },
};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{oneshot, watch};
#[derive(Default)]
struct Supervision {
    closing: bool,
    next_run: u64,
    drains: BTreeMap<u64, watch::Receiver<bool>>,
}
pub struct ServiceTestingController {
    repository: Arc<dyn ServiceTestingRepository>,
    reading: Arc<dyn ProductRepository>,
    gateway: Arc<dyn ModelGateway>,
    clock: Arc<dyn IdentitySource>,
    live: Arc<LiveServiceTestOperations>,
    supervision: Mutex<Supervision>,
    on_terminal: Arc<dyn Fn() + Send + Sync>,
}
fn error(code: &str) -> AppError {
    AppError::new(
        code,
        "service_test",
        "测试尚未完成，操作状态或执行条件已改变",
    )
}
impl ServiceTestingController {
    pub fn new(
        repository: Arc<dyn ServiceTestingRepository>,
        reading: Arc<dyn ProductRepository>,
        gateway: Arc<dyn ModelGateway>,
        clock: Arc<dyn IdentitySource>,
        live: Arc<LiveServiceTestOperations>,
        on_terminal: Arc<dyn Fn() + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            repository,
            reading,
            gateway,
            clock,
            live,
            supervision: Mutex::new(Supervision::default()),
            on_terminal,
        })
    }
    pub async fn run(
        self: &Arc<Self>,
        operation_id: String,
        request: SavedServiceTestRequest,
        selected_token_index: Option<u32>,
    ) -> AppResult<ServiceTestResult> {
        crate::domain::require_id(&operation_id)?;
        let (reply, receive) = oneshot::channel();
        let (finished, drain) = watch::channel(false);

        // This is the existing execution-policy maximum, not a fresh timeout
        // after preparation. The actual saved policy may only shorten it.
        let budget = OperationBudget::new(Duration::from_millis(300_000))?;
        let (reservation, run_id) = {
            let mut supervision = self
                .supervision
                .lock()
                .map_err(|_| error("service_test_state"))?;
            if supervision.closing {
                return Err(error("application_closing"));
            }
            let reserved = self.live.begin_service(
                operation_id.clone(),
                request.expected_profile_generation,
                budget.clone(),
            )?;
            let run_id = supervision
                .next_run
                .checked_add(1)
                .ok_or_else(|| error("service_test_capacity"))?;
            supervision.next_run = run_id;
            supervision.drains.insert(run_id, drain);
            (reserved, run_id)
        };
        let owner = self.clone();
        tokio::spawn(async move {
            let result = owner
                .execute(
                    reservation,
                    operation_id.clone(),
                    request,
                    selected_token_index,
                    budget,
                )
                .await;
            let callback = owner.on_terminal.clone();
            let _ = tokio::task::spawn_blocking(move || callback()).await;
            let _ = reply.send(result);
            finished.send_replace(true);
            if let Ok(mut state) = owner.supervision.lock() {
                state.drains.remove(&run_id);
            }
        });
        // Closing a caller window drops only its receiver. The actual operation
        // remains supervised until the terminal transaction or explicit stop.
        receive
            .await
            .map_err(|_| error("service_test_supervision_lost"))?
    }
    pub fn cancel(&self, operation_id: &str, generation: NonZeroU32) -> AppResult<()> {
        self.live.cancel_service(operation_id, generation)
    }
    pub async fn stop_and_drain(&self) -> AppResult<()> {
        let drains = {
            let mut state = self
                .supervision
                .lock()
                .map_err(|_| error("service_test_state"))?;
            state.closing = true;
            state.drains.values().cloned().collect::<Vec<_>>()
        };
        self.live.cancel_services()?;
        let wait = async {
            for mut drain in drains {
                while !*drain.borrow_and_update() {
                    drain
                        .changed()
                        .await
                        .map_err(|_| error("service_test_supervision_lost"))?;
                }
            }
            Ok(())
        };
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .map_err(|_| error("service_test_drain_timeout"))?
    }
    pub fn reopen_after_failed_shutdown(&self) {
        if let Ok(mut state) = self.supervision.lock() {
            state.closing = false;
        }
    }
    async fn execute(
        self: &Arc<Self>,
        reservation: ServiceTestPreparationGuard,
        operation_id: String,
        request: SavedServiceTestRequest,
        selected_token_index: Option<u32>,
        budget: OperationBudget,
    ) -> AppResult<ServiceTestResult> {
        let owner = self.clone();
        let started = self.clock.now_ms();
        let prepared = tokio::task::spawn_blocking(move || {
            let selected_token = match (&request.input_scope, selected_token_index) {
                (
                    Some(ServiceTestInputScope::SelectedUnit {
                        unit_id,
                        text_revision,
                        source_hash,
                    }),
                    Some(index),
                ) => {
                    let unit = owner.reading.unit(unit_id.clone(), text_revision.get())?;
                    if &unit.source_hash != source_hash {
                        return Err(error("test_reading_identity_mismatch"));
                    }
                    Some(
                        unit.tokens
                            .get(index as usize)
                            .cloned()
                            .ok_or_else(|| error("selected_token_mismatch"))?,
                    )
                }
                (_, None) => None,
                _ => return Err(error("test_input_scope_mismatch")),
            };
            let input = PrepareServiceTestInput {
                operation_id,
                request,
                selected_token,
            };
            if let Some(result) = owner.repository.replay_service_test(&input)? {
                return Ok(Err(result));
            }
            owner
                .repository
                .prepare_service_test(input, started)
                .map(Ok)
        })
        .await
        .map_err(|_| error("test_preparation_failed"))??;
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(result) => return Ok(result),
        };
        let guard = reservation.bind(prepared)?;
        let prepared = guard.prepared();
        // If preparation already timed out/cancelled, preserve that terminal
        // fact. It must never start Vault, token or translation work.
        let _ = budget.restrict_timeout_from_start(Duration::from_millis(u64::from(
            prepared.plan.policy.execution.timeout_ms,
        )));
        let observation = Arc::new(ServiceTestExecutionObservation::default());
        let execution = async {
            budget.check()?;
            let repo = self.repository.clone();
            let plan = prepared.clone();
            tokio::task::spawn_blocking(move || repo.validate_prepared_test(&plan))
                .await
                .map_err(|_| error("test_validation_failed"))??;
            budget.check()?;
            self.gateway
                .execute_saved_service_test(
                    prepared.clone(),
                    self.repository.clone(),
                    budget.clone(),
                    observation.clone(),
                )
                .await
        };
        let result = tokio::select! {
            biased;
            _=budget.cancelled()=>Err(error("operation_cancelled")),
            _=tokio::time::sleep_until(budget.deadline().into())=>Err(error("operation_timeout")),
            result=execution=>result,
        };
        observation.wait_for_physical_drain().await?;
        let facts = observation.snapshot()?;
        let repo = self.repository.clone();
        let clock = self.clock.clone();
        let completion = tokio::task::spawn_blocking(move || {
            let finish = |outcome: ServiceTestTerminalOutcome| {
                repo.finish_service_test(
                    FinishServiceTestInput {
                        prepared: prepared.clone(),
                        outcome,
                        budget: budget.clone(),
                        request_sent: facts.request_started,
                        credential_used: facts.credential_used,
                        google_lease: facts.google_lease.clone(),
                    },
                    clock.now_ms(),
                )
            };
            let terminal = match budget.check() {
                Err(e) if e.code == "operation_cancelled" => ServiceTestTerminalOutcome::Cancelled,
                Err(e) if e.code == "operation_timeout" => ServiceTestTerminalOutcome::TimedOut,
                Err(e) => return Err(e),
                Ok(()) => match result {
                    Ok(response) => ServiceTestTerminalOutcome::Succeeded { response },
                    Err(e)
                        if e.code.ends_with("_unavailable")
                            || e.code.starts_with("unsupported_")
                            || e.code == "adapter_not_implemented" =>
                    {
                        ServiceTestTerminalOutcome::Unsupported { code: e.code }
                    }
                    Err(error) => ServiceTestTerminalOutcome::Failed { error },
                },
            };
            match finish(terminal) {
                // Cancellation can win after output validation but before the
                // repository's terminal fence. Retry only the actual stop fact.
                Err(e) if e.code == "operation_cancelled" => {
                    finish(ServiceTestTerminalOutcome::Cancelled)
                }
                Err(e) if e.code == "operation_timeout" => {
                    finish(ServiceTestTerminalOutcome::TimedOut)
                }
                result => result,
            }
        })
        .await
        .map_err(|_| error("test_terminal_failed"))?;
        drop(guard);
        completion
    }
}

impl super::runtime::ApplicationRuntime {
    pub fn configure_service_testing(
        self: &Arc<Self>,
        repository: Arc<dyn ServiceTestingRepository>,
        live: Arc<LiveServiceTestOperations>,
    ) -> AppResult<()> {
        let mut slot = self
            .service_testing
            .lock()
            .map_err(|_| error("service_test_state"))?;
        if slot.is_some() {
            return Err(error("service_testing_already_configured"));
        }
        let runtime = Arc::downgrade(self);
        let callback = Arc::new(move || {
            if let Some(app) = runtime.upgrade()
                && let Err(error) = app.publish() {
                    app.report_native_effect_failure(error);
                }
        });
        *slot = Some(ServiceTestingController::new(
            repository,
            self.repository.clone(),
            self.gateway.clone(),
            self.clock.clone(),
            live,
            callback,
        ));
        Ok(())
    }
    fn test_controller(&self) -> AppResult<Arc<ServiceTestingController>> {
        self.service_testing
            .lock()
            .map_err(|_| error("service_test_state"))?
            .clone()
            .ok_or_else(|| error("service_testing_unavailable"))
    }
    pub async fn test_saved_service(
        self: &Arc<Self>,
        operation_id: Option<String>,
        request: SavedServiceTestRequest,
        selected_token_index: Option<u32>,
    ) -> AppResult<ServiceTestResult> {
        if self.closing.load(std::sync::atomic::Ordering::Acquire) {
            return Err(error("application_closing"));
        }
        self.test_controller()?
            .run(
                operation_id.unwrap_or_else(|| self.clock.new_id("saved-service-test")),
                request,
                selected_token_index,
            )
            .await
    }
    pub fn cancel_saved_service_test(
        &self,
        operation_id: &str,
        generation: NonZeroU32,
    ) -> AppResult<()> {
        self.ensure_service_profile_generation(generation)?;
        self.test_controller()?.cancel(operation_id, generation)
    }
}
