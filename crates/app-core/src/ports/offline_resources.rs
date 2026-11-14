//! Trusted offline preparation/authority seams. These are Rust-only facts, not
//! IPC input, a second scheduler, or a default-success installation adapter.
use crate::{
    contracts::{AppError, AppResult},
    domain::{
        model_config::service_graph::{
            LanguagePair, OfflineConfigRef, OfflineEngineKey, OfflineRoutePolicy,
        },
        offline_runtime::{
            ExpectedRuntimeObservation, GrantedRootRef, OfflineOperationTarget,
            OfflineResourceReference, OfflineRouteHop, ResourceAvailability, RuntimeCoverageReport,
            RuntimeProfileRef, validate_required_coverage_binding, validate_route,
        },
    },
    ports::{operation_budget::OperationBudget, runtime::PortFuture},
};
use std::num::NonZeroU32;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Query scope only. Equal strings do not prove a concrete writer/actor identity.
/// O3/O5 composition must bind the actual writer token and original owner facts.
pub struct OfflineProfileFrame {
    pub profile_id: String,
    pub generation: NonZeroU32,
    pub server_instance_id: String,
}
impl OfflineProfileFrame {
    pub fn validate(&self) -> AppResult<()> {
        crate::domain::require_id(&self.profile_id)?;
        crate::domain::require_id(&self.server_instance_id)
    }
}
/// The directory locator/OS capability remains private to the concrete owner.
/// Core receives the original grant reference, never an arbitrary filesystem path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflinePreparationSource {
    LocalGrantedRoot { root: GrantedRootRef },
    AppleSystem,
}
#[derive(Debug, Clone)]
pub struct OfflinePreparationContext {
    pub operation_id: String,
    pub frame: OfflineProfileFrame,
    pub engine: OfflineEngineKey,
    pub profile: RuntimeProfileRef,
    pub pair: LanguagePair,
    pub source: OfflinePreparationSource,
    pub expected_installation_head: Option<NonZeroU32>,
    pub expected_observation_head: Option<NonZeroU32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflineObservationState {
    Unknown,
    Installed,
    Missing,
    Changed,
    PermissionDenied,
    Unsupported,
    Error,
}
/// Shape-valid preparation facts do not qualify a runtime or authorize source
/// text. Only real Repository/owner implementations can supply/revalidate them.
/// ```compile_fail
/// use app_core::ports::offline_resources::PreparedOfflineResourceFacts;
/// fn export(facts:&PreparedOfflineResourceFacts){let _=serde_json::to_string(facts);}
/// ```
#[derive(Debug, Clone)]
pub struct PreparedOfflineResourceFacts {
    pub frame: OfflineProfileFrame,
    pub reference: OfflineResourceReference,
    pub expected_observation: ExpectedRuntimeObservation,
    pub availability: ResourceAvailability,
    pub observed_state: OfflineObservationState,
    pub coverage: RuntimeCoverageReport,
    pub route: Vec<OfflineRouteHop>,
    pub route_policy: OfflineRoutePolicy,
    pub installation_head_revision: Option<NonZeroU32>,
}
impl PreparedOfflineResourceFacts {
    pub fn validate_shape(&self) -> AppResult<()> {
        self.frame.validate()?;
        self.reference.validate()?;
        self.expected_observation.validate()?;
        validate_route(&self.route, &self.reference.pair, &self.route_policy)?;
        if self.expected_observation.canonical_sha256()?
            != self.reference.expected_observation_sha256
            || self.expected_observation.engine != self.reference.profile.engine
            || self.expected_observation.platform
                != self.reference.profile.expected_worker_platform()
            || self.expected_observation.architecture != self.reference.profile.architecture
            || self.expected_observation.processing_sha256 != self.reference.processing_sha256
        {
            return Err(AppError::new(
                "offline_observation_binding_mismatch",
                "offline_resources",
                "离线观察与固定资源身份不一致",
            ));
        }
        Ok(())
    }
    /// Still only validates facts. It returns no lease/proof and performs no
    /// live operation, handler, SQL or physical-runtime authorization check.
    pub fn validate_required_evidence_shape(&self) -> AppResult<()> {
        self.validate_shape()?;
        if self.availability != ResourceAvailability::Installed
            || self.observed_state != OfflineObservationState::Installed
        {
            return Err(AppError::new(
                "offline_resources_not_installed",
                "offline_resources",
                "当前离线资源尚未核实为已安装",
            ));
        }
        validate_required_coverage_binding(&self.coverage, &self.reference)
    }
}
#[derive(Debug, Clone)]
pub struct OfflineResourceQuery {
    pub frame: OfflineProfileFrame,
    pub source: OfflineConfigRef,
    pub operation: OfflineOperationTarget,
}
#[derive(Debug, Clone)]
pub struct OfflinePreparationReceipt {
    pub operation_id: String,
    pub frame: OfflineProfileFrame,
    pub reference: OfflineResourceReference,
    pub state: OfflineObservationState,
}

/// The implementation must resolve original rows/seals/heads/grants and its
/// actual handler registration and concrete writer identity. Neither matching
/// frame strings, a digest, non-Serde data nor caller-made facts suffice. These
/// methods recheck facts; they do not manufacture an execution lease.
pub trait OfflineResourcesRepository: Send + Sync {
    fn read_offline_resource_facts(
        &self,
        query: &OfflineResourceQuery,
    ) -> AppResult<PreparedOfflineResourceFacts>;
    fn validate_offline_resource_facts(
        &self,
        query: &OfflineResourceQuery,
        facts: &PreparedOfflineResourceFacts,
    ) -> AppResult<()>;
    fn commit_offline_preparation(
        &self,
        context: &OfflinePreparationContext,
        facts: &PreparedOfflineResourceFacts,
        now_ms: i64,
    ) -> AppResult<OfflinePreparationReceipt>;
    fn revoke_offline_root(
        &self,
        frame: &OfflineProfileFrame,
        root: &GrantedRootRef,
        operation_id: &str,
        now_ms: i64,
    ) -> AppResult<()>;
}
/// Only the concrete resource owner can inspect the granted root, compiled
/// policy and actual loader. Observation is not a grant or an execution lease.
pub trait OfflineRuntimeResourceOwner: Send + Sync {
    fn observe_offline_resources<'a>(
        &'a self,
        context: &'a OfflinePreparationContext,
        budget: OperationBudget,
    ) -> PortFuture<'a, PreparedOfflineResourceFacts>;
    fn revalidate_offline_runtime<'a>(
        &'a self,
        query: &'a OfflineResourceQuery,
        facts: &'a PreparedOfflineResourceFacts,
        budget: OperationBudget,
    ) -> PortFuture<'a, RuntimeCoverageReport>;
}
