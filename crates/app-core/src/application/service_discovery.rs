//! Explicit saved-source model lists. No reading queue, test qualification or service selection.
use crate::{
    contracts::*,
    domain::model_config::service_graph::SavedServiceRef,
    ports::{
        operation_budget::{OperationBudget, OperationTerminalDisposition},
        physical_jobs::PhysicalJobs,
        service_discovery::*,
    },
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    mem::size_of,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;

const ACTIVE_MAX: usize = 8;
const PAYLOAD_MAX: usize = 32;
const PAYLOAD_BUDGET: usize = 8 * 1024 * 1024;
const IDENTITY_MAX: usize = 4096;
const IDENTITY_BUDGET: usize = 2 * 1024 * 1024;
const FIXED_BYTES: usize = 64 * 1024;
const IDENTITY_BYTES: usize = 384;
type Key = [u8; 32];
type Reply = AppResult<Arc<ServiceModelDiscoveryResult>>;
struct Identity {
    request: Key,
    generation: u32,
    purpose: CatalogPurpose,
    terminal: bool,
}
struct Running {
    operation: Arc<ServiceModelDiscoveryOperation>,
    jobs: Arc<PhysicalJobs>,
    prepared: Mutex<Option<Arc<PreparedServiceModelDiscovery>>>,
    candidate: Mutex<Option<Arc<ServiceModelDiscoveryResult>>>,
    reply: watch::Sender<Option<Reply>>,
    drained: watch::Sender<bool>,
}
struct Registry {
    closing: bool,
    active: BTreeMap<Key, Arc<Running>>,
    identities: BTreeMap<Key, Identity>,
    payloads: BTreeMap<Key, (Arc<ServiceModelDiscoveryResult>, usize)>,
    recent: VecDeque<Key>,
    payload_bytes: usize,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            closing: false,
            active: BTreeMap::new(),
            identities: BTreeMap::new(),
            payloads: BTreeMap::new(),
            recent: VecDeque::new(),
            payload_bytes: FIXED_BYTES,
        }
    }
}
#[derive(Default)]
pub struct LiveServiceModelDiscoveries {
    registry: Mutex<Registry>,
}
enum Admission {
    Start(Arc<Running>),
    Wait(Arc<Running>),
    Complete(Arc<ServiceModelDiscoveryResult>),
}
fn failure(code: &str) -> AppError {
    AppError::new(
        code,
        "service_model_discovery",
        "获取模型列表的状态或来源已变化；原输入已保留",
    )
}
fn key(id: &str) -> Key {
    let mut h = Sha256::new();
    h.update(b"luna/model-list/operation/v1\0");
    h.update(id.as_bytes());
    h.finalize().into()
}
fn request_key(input: &ServiceModelDiscoveryInput) -> AppResult<Key> {
    let mut h = Sha256::new();
    h.update(b"luna/model-list/request/v1\0");
    h.update(serde_json::to_vec(&input.request).map_err(|_| failure("discovery_request_invalid"))?);
    Ok(h.finalize().into())
}

pub fn model_discovery_result(
    input: &ServiceModelDiscoveryInput,
    outcome: ServiceModelDiscoveryOutcome,
) -> ServiceModelDiscoveryResult {
    let message = match &outcome {
        ServiceModelDiscoveryOutcome::Available { models } if models.is_empty() => {
            "服务没有返回可用模型；可继续手动填写模型ID。"
        }
        ServiceModelDiscoveryOutcome::Available { .. } => {
            "已获取模型列表；选择后仅更新当前编辑，仍需明确保存。"
        }
        ServiceModelDiscoveryOutcome::Unsupported { reason }
            if reason.code == "discovery_typed_source_required" =>
        {
            "请先保存当前服务，再获取可用模型。"
        }
        ServiceModelDiscoveryOutcome::Unsupported { .. } => {
            "此服务不提供当前模型列表接口；原输入已保留。"
        }
        ServiceModelDiscoveryOutcome::Failed { .. } => "获取模型列表失败；原输入已保留。",
        ServiceModelDiscoveryOutcome::Cancelled => "本次获取已取消；原输入已保留。",
        ServiceModelDiscoveryOutcome::TimedOut => "获取模型列表超时；原输入已保留。",
        ServiceModelDiscoveryOutcome::ResultExpired { .. } => {
            "本次结果已过期；原输入已保留，可再次点击获取可用模型。"
        }
    };
    ServiceModelDiscoveryResult {
        operation_id: input.operation_id.clone(),
        request: input.request.clone(),
        outcome,
        message: message.into(),
    }
}
fn safe_failure(error: &AppError) -> ServiceModelDiscoveryFailure {
    let safe = |s: &str, max: usize| {
        !s.is_empty()
            && s.len() <= max
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    ServiceModelDiscoveryFailure {
        code: if safe(&error.code, 128) {
            error.code.clone()
        } else {
            "discovery_failed".into()
        },
        stage: if safe(&error.stage, 64) {
            error.stage.clone()
        } else {
            "service_model_discovery".into()
        },
        recovery: match error.code.as_str() {
            "discovery_typed_source_required" => {
                "打开服务配置，确认接口并保存当前服务后，再明确获取模型列表。"
            }
            "authentication_failed" | "missing_credential" | "credential_authority_changed" => {
                "检查此服务保存的账号与凭据，保存后再获取；也可继续手动填写模型ID。"
            }
            _ => "检查当前服务地址、网络及保存的配置；原输入可继续手动编辑。",
        }
        .into(),
    }
}
fn error_outcome(error: &AppError) -> ServiceModelDiscoveryOutcome {
    match error.code.as_str() {
        "operation_cancelled" => ServiceModelDiscoveryOutcome::Cancelled,
        "operation_timeout" => ServiceModelDiscoveryOutcome::TimedOut,
        "discovery_unsupported" | "discovery_typed_source_required" => {
            ServiceModelDiscoveryOutcome::Unsupported {
                reason: safe_failure(error),
            }
        }
        _ => ServiceModelDiscoveryOutcome::Failed {
            reason: safe_failure(error),
        },
    }
}
fn expired(input: &ServiceModelDiscoveryInput) -> Arc<ServiceModelDiscoveryResult> {
    Arc::new(model_discovery_result(
        input,
        ServiceModelDiscoveryOutcome::ResultExpired {
            reason: ServiceModelDiscoveryFailure {
                code: "discovery_result_expired".into(),
                stage: "service_model_discovery".into(),
                recovery: "如需重新获取，请再次明确点击获取可用模型；不会自动重新发送请求。".into(),
            },
        },
    ))
}
pub fn validate_discovered_models(models: &[DiscoveredServiceModel]) -> AppResult<()> {
    if models.len() > 4096
        || models.iter().any(|m| {
            m.id.is_empty()
                || m.id.len() > 512
                || m.id.trim() != m.id
                || m.id.chars().any(char::is_control)
        })
    {
        return Err(failure("invalid_model_list"));
    }
    Ok(())
}
fn heap_bytes(capacity: usize) -> Option<usize> {
    if capacity == 0 {
        return Some(0);
    }
    capacity
        .checked_add(15)?
        .checked_div(16)?
        .checked_mul(16)?
        .checked_add(32)
}
fn payload_cost(result: &ServiceModelDiscoveryResult) -> Option<usize> {
    let mut cost = 4096usize;
    let mut string = |s: &String| {
        cost = cost.checked_add(heap_bytes(s.capacity())?)?;
        Some(())
    };
    string(&result.operation_id)?;
    string(&result.message)?;
    match &result.request.source {
        SavedServiceRef::Http { source } => {
            string(&source.revision_id)?;
            string(&source.model_id)?;
        }
        SavedServiceRef::Offline { source } => {
            string(&source.revision_id)?;
            string(&source.config_id)?;
        }
    }
    match &result.outcome {
        ServiceModelDiscoveryOutcome::Available { models } => {
            cost = cost.checked_add(heap_bytes(
                models
                    .capacity()
                    .checked_mul(size_of::<DiscoveredServiceModel>())?,
            )?)?;
            for model in models {
                cost = cost.checked_add(heap_bytes(model.id.capacity())?)?;
            }
        }
        ServiceModelDiscoveryOutcome::Failed { reason }
        | ServiceModelDiscoveryOutcome::Unsupported { reason }
        | ServiceModelDiscoveryOutcome::ResultExpired { reason } => {
            for text in [&reason.code, &reason.stage, &reason.recovery] {
                cost = cost.checked_add(heap_bytes(text.capacity())?)?;
            }
        }
        _ => {}
    }
    Some(cost)
}
impl LiveServiceModelDiscoveries {
    fn admit(&self, input: ServiceModelDiscoveryInput) -> AppResult<Admission> {
        input.validate()?;
        let operation_key = key(&input.operation_id);
        let request = request_key(&input)?;
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        if let Some(identity) = registry.identities.get(&operation_key) {
            if identity.request != request
                || identity.generation != input.request.expected_profile_generation.get()
                || identity.purpose != input.request.purpose
            {
                return Err(failure("operation_conflict"));
            }
            if let Some(active) = registry.active.get(&operation_key) {
                return Ok(Admission::Wait(active.clone()));
            }
            if !identity.terminal {
                return Err(failure("discovery_state"));
            }
            if let Some((result, _)) = registry.payloads.get(&operation_key) {
                let result = result.clone();
                registry.recent.retain(|v| *v != operation_key);
                registry.recent.push_back(operation_key);
                return Ok(Admission::Complete(result));
            }
            return Ok(Admission::Complete(expired(&input)));
        }
        if registry.closing {
            return Err(failure("application_closing"));
        }
        if registry.active.len() >= ACTIVE_MAX {
            return Err(failure("discovery_busy"));
        }
        let identity_cost = registry
            .identities
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(IDENTITY_BYTES))
            .and_then(|n| n.checked_add(FIXED_BYTES))
            .ok_or_else(|| failure("discovery_identity_capacity"))?;
        if registry.identities.len() >= IDENTITY_MAX || identity_cost > IDENTITY_BUDGET {
            return Err(failure("discovery_identity_capacity"));
        }
        let budget = OperationBudget::new(Duration::from_secs(15))?;
        let (reply, _) = watch::channel(None);
        let (drained, _) = watch::channel(false);
        let generation = input.request.expected_profile_generation.get();
        let purpose = input.request.purpose;
        let active = Arc::new(Running {
            operation: Arc::new(ServiceModelDiscoveryOperation { input, budget }),
            jobs: Arc::new(PhysicalJobs::default()),
            prepared: Mutex::new(None),
            candidate: Mutex::new(None),
            reply,
            drained,
        });
        registry.identities.insert(
            operation_key,
            Identity {
                request,
                generation,
                purpose,
                terminal: false,
            },
        );
        registry.active.insert(operation_key, active.clone());
        Ok(Admission::Start(active))
    }
    fn running(
        registry: &Registry,
        op: &Arc<ServiceModelDiscoveryOperation>,
    ) -> AppResult<Arc<Running>> {
        registry
            .active
            .get(&key(&op.input.operation_id))
            .filter(|r| Arc::ptr_eq(&r.operation, op))
            .cloned()
            .ok_or_else(|| failure("discovery_operation_unowned"))
    }
    fn publish_candidate(
        &self,
        running: &Arc<Running>,
        result: Arc<ServiceModelDiscoveryResult>,
    ) -> AppResult<()> {
        if result.operation_id != running.operation.input.operation_id
            || result.request != running.operation.input.request
        {
            return Err(failure("discovery_result_identity_mismatch"));
        }
        let mut candidate = running
            .candidate
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        if candidate.as_ref().is_some_and(|old| **old != *result) {
            return Err(failure("discovery_terminal_conflict"));
        }
        *candidate = Some(result);
        Ok(())
    }
    fn finish(&self, running: &Arc<Running>) -> AppResult<()> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        Self::running(&registry, &running.operation)?;
        let result = running
            .candidate
            .lock()
            .map_err(|_| failure("discovery_state"))?
            .clone()
            .ok_or_else(|| failure("discovery_terminal_missing"))?;
        // No live preparation is retained by either the payload cache or identity.
        running
            .prepared
            .lock()
            .map_err(|_| failure("discovery_state"))?
            .take();
        let operation_key = key(&running.operation.input.operation_id);
        registry
            .identities
            .get_mut(&operation_key)
            .ok_or_else(|| failure("discovery_state"))?
            .terminal = true;
        registry.active.remove(&operation_key);
        if let Some(cost) = payload_cost(&result).filter(|n| {
            n.checked_add(FIXED_BYTES)
                .is_some_and(|v| v <= PAYLOAD_BUDGET)
        }) {
            while registry.payloads.len() >= PAYLOAD_MAX
                || registry
                    .payload_bytes
                    .checked_add(cost)
                    .is_none_or(|v| v > PAYLOAD_BUDGET)
            {
                let oldest = registry
                    .recent
                    .pop_front()
                    .ok_or_else(|| failure("discovery_cache_state"))?;
                let (_, old_cost) = registry
                    .payloads
                    .remove(&oldest)
                    .ok_or_else(|| failure("discovery_cache_state"))?;
                registry.payload_bytes = registry
                    .payload_bytes
                    .checked_sub(old_cost)
                    .ok_or_else(|| failure("discovery_cache_state"))?;
            }
            registry.payload_bytes += cost;
            registry.recent.push_back(operation_key);
            registry
                .payloads
                .insert(operation_key, (result.clone(), cost));
        }
        running.reply.send_replace(Some(Ok(result)));
        running.drained.send_replace(true);
        Ok(())
    }
    pub fn cancel(
        &self,
        input: CancelServiceModelDiscoveryInput,
    ) -> AppResult<ServiceModelDiscoveryCancelReceipt> {
        crate::contracts::service_discovery::bounded_id(&input.operation_id, 128)?;
        let operation_key = key(&input.operation_id);
        let running = {
            let registry = self
                .registry
                .lock()
                .map_err(|_| failure("discovery_state"))?;
            let identity = registry
                .identities
                .get(&operation_key)
                .ok_or_else(|| failure("discovery_operation_unknown"))?;
            if identity.generation != input.expected_profile_generation.get() {
                return Err(failure("profile_generation_conflict"));
            }
            registry.active.get(&operation_key).cloned()
        };
        let disposition = if let Some(running) = running {
            if running.operation.budget.is_committed() {
                ServiceModelDiscoveryCancelDisposition::AlreadyTerminal
            } else {
                running.operation.budget.cancel();
                ServiceModelDiscoveryCancelDisposition::Requested
            }
        } else {
            ServiceModelDiscoveryCancelDisposition::AlreadyTerminal
        };
        Ok(ServiceModelDiscoveryCancelReceipt {
            operation_id: input.operation_id,
            disposition,
        })
    }
}
impl ServiceModelDiscoveryAuthority for LiveServiceModelDiscoveries {
    fn operation_budget(
        &self,
        op: &Arc<ServiceModelDiscoveryOperation>,
    ) -> AppResult<OperationBudget> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        Self::running(&registry, op)?;
        op.budget.check()?;
        Ok(op.budget.clone())
    }
    fn prepared_budget(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
    ) -> AppResult<OperationBudget> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        let running = Self::running(&registry, &p.operation)?;
        if !running
            .prepared
            .lock()
            .map_err(|_| failure("discovery_state"))?
            .as_ref()
            .is_some_and(|original| Arc::ptr_eq(original, p))
        {
            return Err(failure("discovery_prepared_unowned"));
        }
        p.operation.budget.check()?;
        Ok(p.operation.budget.clone())
    }
    fn attach_prepared(
        &self,
        op: &Arc<ServiceModelDiscoveryOperation>,
        p: Arc<PreparedServiceModelDiscovery>,
    ) -> AppResult<()> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        let running = Self::running(&registry, op)?;
        op.budget.check()?;
        if !Arc::ptr_eq(&p.operation, op)
            || p.source.source != op.input.request.source
            || p.profile_generation != op.input.request.expected_profile_generation
        {
            return Err(failure("discovery_prepared_identity_mismatch"));
        }
        let mut original = running
            .prepared
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        if original.as_ref().is_some_and(|old| !Arc::ptr_eq(old, &p)) {
            return Err(failure("discovery_prepared_conflict"));
        }
        *original = Some(p);
        Ok(())
    }
    fn active_pins(&self) -> AppResult<Vec<Arc<PreparedServiceModelDiscovery>>> {
        self.registry
            .lock()
            .map_err(|_| failure("discovery_state"))?
            .active
            .values()
            .map(|r| {
                r.prepared
                    .lock()
                    .map(|p| p.clone())
                    .map_err(|_| failure("discovery_state"))
            })
            .collect::<AppResult<Vec<_>>>()
            .map(|v| v.into_iter().flatten().collect())
    }
    fn accept_validated_terminal(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        result: ServiceModelDiscoveryResult,
    ) -> AppResult<()> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        let running = Self::running(&registry, &p.operation)?;
        if !running
            .prepared
            .lock()
            .map_err(|_| failure("discovery_state"))?
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, p))
        {
            return Err(failure("discovery_prepared_unowned"));
        }
        p.operation.budget.check()?;
        let ServiceModelDiscoveryOutcome::Available { models } = &result.outcome else {
            return Err(failure("discovery_terminal_invalid"));
        };
        validate_discovered_models(models)?;
        p.operation.budget.check()?;
        self.publish_candidate(&running, Arc::new(result))
    }
}

pub struct ServiceModelDiscoveryController {
    backend: Arc<dyn ServiceModelDiscoveryBackend>,
    live: Arc<LiveServiceModelDiscoveries>,
}
impl ServiceModelDiscoveryController {
    pub fn new(
        backend: Arc<dyn ServiceModelDiscoveryBackend>,
        live: Arc<LiveServiceModelDiscoveries>,
    ) -> Arc<Self> {
        Arc::new(Self { backend, live })
    }
    pub async fn run(
        self: &Arc<Self>,
        input: ServiceModelDiscoveryInput,
    ) -> AppResult<ServiceModelDiscoveryResult> {
        let running = match self.live.admit(input)? {
            Admission::Complete(result) => return Ok((*result).clone()),
            Admission::Wait(running) => running,
            Admission::Start(running) => {
                let owner = self.clone();
                let work = running.clone();
                tokio::spawn(async move {
                    owner.supervise(work).await;
                });
                running
            }
        };
        let mut reply = running.reply.subscribe();
        loop {
            if let Some(result) = reply.borrow_and_update().clone() {
                return result.map(|r| (*r).clone());
            }
            reply
                .changed()
                .await
                .map_err(|_| failure("discovery_supervision_lost"))?;
        }
    }
    async fn supervise(self: Arc<Self>, running: Arc<Running>) {
        let worker = self.clone();
        let work = running.clone();
        let result = tokio::spawn(async move { worker.execute(&work).await })
            .await
            .map_err(|_| failure("discovery_supervision_failure"))
            .and_then(|v| v);
        let error = result
            .err()
            .unwrap_or_else(|| failure("discovery_completion_unverified"));
        if let Err(error) = self.settle_failure(&running, &error) {
            running.reply.send_replace(Some(Err(error)));
            return;
        }
        // execute has stopped producing I/O; even a discarded blocking awaiter
        // still owns its guard. All last terminal work was registered already.
        if let Err(error) = running.jobs.close_admission() {
            running.reply.send_replace(Some(Err(error)));
            return;
        }
        if let Err(error) = running.jobs.wait_for_physical_drain().await {
            running.reply.send_replace(Some(Err(error)));
            return;
        }
        if let Err(error) = self.live.finish(&running) {
            running.reply.send_replace(Some(Err(error)));
        }
    }
    async fn execute(&self, running: &Arc<Running>) -> AppResult<()> {
        let op = &running.operation;
        let prepared = within(
            &op.budget,
            self.backend.prepare(op.clone(), running.jobs.clone()),
        )
        .await?;
        self.live.prepared_budget(&prepared)?;
        let models = within(
            &op.budget,
            self.backend
                .discover(prepared.clone(), running.jobs.clone()),
        )
        .await?;
        validate_discovered_models(&models)?;
        let mut seen = BTreeSet::new();
        let models = models
            .into_iter()
            .filter(|m| seen.insert(m.id.clone()))
            .collect();
        let result = self
            .backend
            .complete(
                prepared,
                ServiceModelDiscoveryOutcome::Available { models },
                running.jobs.clone(),
            )
            .await?;
        if !op.budget.is_committed()
            || !running
                .candidate
                .lock()
                .map_err(|_| failure("discovery_state"))?
                .as_ref()
                .is_some_and(|c| **c == result)
        {
            return Err(failure("discovery_completion_unverified"));
        }
        Ok(())
    }
    fn settle_failure(&self, running: &Arc<Running>, original: &AppError) -> AppResult<()> {
        // Active -> expired -> cancelled are the only possible stop changes.
        // The same terminal fence may be won by actual in-flight SQL work.
        for _ in 0..3 {
            if running.operation.budget.is_committed() {
                if running
                    .candidate
                    .lock()
                    .map_err(|_| failure("discovery_state"))?
                    .is_none()
                {
                    // A defective adapter committed no verified result: no models.
                    self.live.publish_candidate(
                        running,
                        Arc::new(model_discovery_result(
                            &running.operation.input,
                            error_outcome(&failure("discovery_completion_unverified")),
                        )),
                    )?;
                }
                return Ok(());
            }
            let error = running
                .operation
                .budget
                .check()
                .err()
                .unwrap_or_else(|| original.clone());
            let disposition = match error.code.as_str() {
                "operation_cancelled" => OperationTerminalDisposition::Cancelled,
                "operation_timeout" => OperationTerminalDisposition::TimedOut,
                _ => OperationTerminalDisposition::Active,
            };
            let result = running
                .operation
                .budget
                .with_terminal_fence_for(disposition, || {
                    // Replace a rolled-back provisional candidate under the same fence.
                    running
                        .candidate
                        .lock()
                        .map_err(|_| failure("discovery_state"))?
                        .take();
                    self.live.publish_candidate(
                        running,
                        Arc::new(model_discovery_result(
                            &running.operation.input,
                            error_outcome(&error),
                        )),
                    )
                });
            match result {
                Ok(()) => return Ok(()),
                Err(error)
                    if running.operation.budget.is_committed()
                        || matches!(
                            error.code.as_str(),
                            "operation_cancelled"
                                | "operation_timeout"
                                | "operation_terminal_mismatch"
                        ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        Err(failure("discovery_terminal_unsettled"))
    }
    pub fn cancel(
        &self,
        input: CancelServiceModelDiscoveryInput,
    ) -> AppResult<ServiceModelDiscoveryCancelReceipt> {
        self.live.cancel(input)
    }
    pub async fn stop_and_drain(&self) -> AppResult<()> {
        let active = {
            let mut r = self
                .live
                .registry
                .lock()
                .map_err(|_| failure("discovery_state"))?;
            r.closing = true;
            r.active.values().cloned().collect::<Vec<_>>()
        };
        for r in &active {
            r.operation.budget.cancel();
        }
        let drain = async {
            for r in active {
                let mut receiver = r.drained.subscribe();
                while !*receiver.borrow_and_update() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| failure("discovery_supervision_lost"))?;
                }
            }
            Ok(())
        };
        tokio::time::timeout(Duration::from_secs(5), drain)
            .await
            .map_err(|_| failure("discovery_drain_timeout"))?
    }
    pub fn reopen_after_failed_shutdown(&self) {
        if let Ok(mut r) = self.live.registry.lock() {
            r.closing = false;
        }
    }
}
async fn within<T>(
    budget: &OperationBudget,
    future: impl std::future::Future<Output = AppResult<T>>,
) -> AppResult<T> {
    budget.check()?;
    tokio::select! { biased;
        _=budget.cancelled()=>Err(budget.check().err().unwrap_or_else(||failure("discovery_state"))),
        _=tokio::time::sleep_until(budget.deadline().into())=>Err(budget.check().err().unwrap_or_else(||failure("operation_timeout"))),
        result=future=>{budget.check()?;result}
    }
}

impl super::runtime::ApplicationRuntime {
    pub fn configure_service_model_discovery(
        &self,
        backend: Arc<dyn ServiceModelDiscoveryBackend>,
        live: Arc<LiveServiceModelDiscoveries>,
    ) -> AppResult<()> {
        let mut slot = self
            .service_model_discovery
            .lock()
            .map_err(|_| failure("discovery_state"))?;
        if slot.is_some() {
            return Err(failure("discovery_already_configured"));
        }
        *slot = Some(ServiceModelDiscoveryController::new(backend, live));
        Ok(())
    }
    fn discovery_controller(&self) -> AppResult<Arc<ServiceModelDiscoveryController>> {
        self.service_model_discovery
            .lock()
            .map_err(|_| failure("discovery_state"))?
            .clone()
            .ok_or_else(|| failure("discovery_unavailable"))
    }
    pub async fn discover_saved_service_models(
        &self,
        input: ServiceModelDiscoveryInput,
    ) -> AppResult<ServiceModelDiscoveryResult> {
        if self.closing.load(std::sync::atomic::Ordering::Acquire) {
            return Err(failure("application_closing"));
        }
        // The Controller reserves before the first SQL/Vault await.
        self.discovery_controller()?.run(input).await
    }
    pub fn cancel_service_model_discovery(
        &self,
        input: CancelServiceModelDiscoveryInput,
    ) -> AppResult<ServiceModelDiscoveryCancelReceipt> {
        self.discovery_controller()?.cancel(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(id: &str) -> ServiceModelDiscoveryInput {
        serde_json::from_value(serde_json::json!({"operationId":id,"request":{"expectedProfileGeneration":1,"source":{"kind":"http","source":{"revisionId":"saved","modelId":"model"}},"purpose":"translation"}})).unwrap()
    }
    // Cache accounting only. This deliberately does not claim source/SQL/wire qualification.
    fn cache_only(
        live: &LiveServiceModelDiscoveries,
        input: ServiceModelDiscoveryInput,
        outcome: ServiceModelDiscoveryOutcome,
    ) {
        let Admission::Start(running) = live.admit(input).unwrap() else {
            panic!("new cache fixture")
        };
        let result = Arc::new(model_discovery_result(&running.operation.input, outcome));
        running
            .operation
            .budget
            .with_terminal_fence(|| live.publish_candidate(&running, result))
            .unwrap();
        running.jobs.close_admission().unwrap();
        live.finish(&running).unwrap();
    }
    fn large() -> ServiceModelDiscoveryOutcome {
        ServiceModelDiscoveryOutcome::Available {
            models: (0..4096)
                .map(|_| DiscoveredServiceModel {
                    id: {
                        let mut id = String::with_capacity(512);
                        id.push_str(&"界".repeat(170));
                        id.push_str("ab");
                        assert_eq!(id.capacity(), 512);
                        id
                    },
                })
                .collect(),
        }
    }
    #[test]
    fn actual_fixed_identity_layout_fits_the_conservative_identity_budget() {
        assert_eq!(size_of::<Key>(), 32);
        assert_eq!(size_of::<Identity>(), 40);
        assert!(FIXED_BYTES + IDENTITY_MAX * IDENTITY_BYTES <= IDENTITY_BUDGET);
        assert!(IDENTITY_BYTES >= 4 * (size_of::<Identity>() + size_of::<Key>()));
        assert_eq!(size_of::<DiscoveredServiceModel>(), size_of::<String>());
    }
    #[test]
    fn actual_utf8_capacity_and_allocations_drive_the_byte_limit_and_lru_eviction() {
        let live = LiveServiceModelDiscoveries::default();
        for i in 0..3 {
            cache_only(&live, input(&format!("large-{i}")), large());
        }
        {
            let registry = live.registry.lock().unwrap();
            assert_eq!(registry.payloads.len(), 3);
            assert!(registry.payload_bytes <= PAYLOAD_BUDGET);
            let (result, charged) = registry.payloads.get(&key("large-0")).unwrap();
            let ServiceModelDiscoveryOutcome::Available { models } = &result.outcome else {
                unreachable!()
            };
            let raw = models.iter().map(|m| m.id.len()).sum::<usize>();
            assert_eq!(raw, 4096 * 512);
            assert!(*charged > raw + models.capacity() * size_of::<DiscoveredServiceModel>());
        }
        assert!(matches!(
            live.admit(input("large-0")).unwrap(),
            Admission::Complete(_)
        ));
        cache_only(&live, input("large-3"), large());
        let registry = live.registry.lock().unwrap();
        assert_eq!(registry.payloads.len(), 3);
        assert!(registry.payload_bytes <= PAYLOAD_BUDGET);
        assert!(registry.payloads.contains_key(&key("large-0")));
        assert!(!registry.payloads.contains_key(&key("large-1")));
        assert!(registry.identities.contains_key(&key("large-1")));
        assert!(registry.active.is_empty());
    }
    #[test]
    fn spare_capacity_or_arithmetic_overflow_cannot_sneak_past_the_byte_budget() {
        assert!(heap_bytes(usize::MAX).is_none());
        let mut id = String::with_capacity(PAYLOAD_BUDGET);
        id.push('猫');
        let outcome = ServiceModelDiscoveryOutcome::Available {
            models: vec![DiscoveredServiceModel { id }],
        };
        let result = model_discovery_result(&input("large-capacity"), outcome);
        assert!(payload_cost(&result).unwrap() > PAYLOAD_BUDGET);
        let live = LiveServiceModelDiscoveries::default();
        cache_only(&live, input("large-capacity"), result.outcome);
        assert!(live.registry.lock().unwrap().payloads.is_empty());
        let Admission::Complete(result) = live.admit(input("large-capacity")).unwrap() else {
            panic!("expired original ID, never new budget")
        };
        assert!(matches!(
            result.outcome,
            ServiceModelDiscoveryOutcome::ResultExpired { .. }
        ));
        assert!(live.registry.lock().unwrap().active.is_empty());
    }
}
