//! Internal admission output. Legacy grammar stays intact; codec3 owns a real
//! HTTP or offline plan. Both use the application's single task lifecycle.
use crate::{
    application::service_request_plan::ComposedServicePlan,
    contracts::{CatalogPurpose, ReadingUnit},
    domain::{CachedTaskResult, TaskDispatch},
};

#[derive(Debug, Clone)]
pub struct ServiceTaskDispatch {
    pub task_id: String,
    pub attempt_id: String,
    pub unit: ReadingUnit,
    pub plan: ComposedServicePlan,
    pub request_semantics_hash: String,
    pub authorization_epoch: u32,
    pub profile_generation: u32,
    pub server_instance_id: String,
    pub credential_vault_ref: Option<String>,
    pub cached_result: Option<CachedTaskResult>,
}

#[derive(Debug, Clone)]
pub enum WorkDispatch {
    Legacy(Box<TaskDispatch>),
    Service(Box<ServiceTaskDispatch>),
}
impl From<TaskDispatch> for WorkDispatch {
    fn from(value: TaskDispatch) -> Self {
        Self::Legacy(Box::new(value))
    }
}
impl From<ServiceTaskDispatch> for WorkDispatch {
    fn from(value: ServiceTaskDispatch) -> Self {
        Self::Service(Box::new(value))
    }
}
impl WorkDispatch {
    pub fn task_id(&self) -> &str {
        match self {
            Self::Legacy(t) => &t.task_id,
            Self::Service(t) => &t.task_id,
        }
    }
    pub fn attempt_id(&self) -> &str {
        match self {
            Self::Legacy(t) => &t.attempt_id,
            Self::Service(t) => &t.attempt_id,
        }
    }
    pub fn set_attempt_id(&mut self, value: String) {
        match self {
            Self::Legacy(t) => t.attempt_id = value,
            Self::Service(t) => t.attempt_id = value,
        }
    }
    pub fn unit(&self) -> &ReadingUnit {
        match self {
            Self::Legacy(t) => &t.unit,
            Self::Service(t) => &t.unit,
        }
    }
    pub fn purpose(&self) -> &str {
        match self {
            Self::Legacy(t) => &t.binding.purpose,
            Self::Service(t) => match t.plan.identity.purpose {
                CatalogPurpose::Translation => "translation",
                CatalogPurpose::Grammar => "grammar",
                CatalogPurpose::ContextWord => "context_word",
            },
        }
    }
    pub fn concurrency(&self) -> usize {
        match self {
            Self::Legacy(t) => t
                .binding
                .schedule
                .get("concurrency")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 8) as usize,
            Self::Service(t) => usize::from(t.plan.policy.execution.concurrency),
        }
    }
    pub fn queue_capacity(&self) -> usize {
        match self {
            Self::Legacy(t) => t
                .binding
                .schedule
                .get("queueCapacity")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(32)
                .clamp(1, 128) as usize,
            Self::Service(_) => 32,
        }
    }
    pub fn retries(&self) -> u64 {
        match self {
            Self::Legacy(t) => t
                .binding
                .schedule
                .get("retries")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            Self::Service(t) => u64::from(t.plan.policy.execution.retries),
        }
    }
    pub fn cached_result(&self) -> Option<&CachedTaskResult> {
        match self {
            Self::Legacy(t) => t.cached_result.as_ref(),
            Self::Service(t) => t.cached_result.as_ref(),
        }
    }
}
pub struct PreparedWorkTranslation {
    pub dispatches: Vec<WorkDispatch>,
    pub task_ids: Vec<String>,
    pub task_views: Vec<crate::contracts::TaskView>,
}
