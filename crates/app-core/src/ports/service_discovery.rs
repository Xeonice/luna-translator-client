//! Non-Serde saved-source discovery seams; Core never inspects repository actor proofs.
use crate::{
    contracts::*,
    domain::{credentials::CredentialLeaseMetadata, model_config::service_graph::*},
    ports::{
        operation_budget::OperationBudget, physical_jobs::PhysicalJobs,
        service_configuration::SavedServiceSourceProof,
    },
};
use std::{any::Any, sync::Arc};

pub struct ServiceModelDiscoveryOperation {
    pub(crate) input: ServiceModelDiscoveryInput,
    pub(crate) budget: OperationBudget,
}
impl ServiceModelDiscoveryOperation {
    pub fn input(&self) -> &ServiceModelDiscoveryInput {
        &self.input
    }
    /// Lifetime only; the installed authority must still prove this original Arc.
    pub fn budget(&self) -> OperationBudget {
        self.budget.clone()
    }
}
#[derive(Clone)]
pub enum ServiceDiscoveryAuthentication {
    None,
    PoolMember {
        metadata: Box<CredentialLeaseMetadata>,
        pool_policy_sha256: String,
    },
}
pub struct PreparedServiceModelDiscovery {
    pub operation: Arc<ServiceModelDiscoveryOperation>,
    pub profile_id: String,
    pub profile_generation: std::num::NonZeroU32,
    pub server_instance_id: String,
    pub source: SavedServiceSourceProof,
    pub connection: ConnectionV3,
    pub http_options: HttpServiceOptionsV1,
    pub origin: ConnectionOrigin,
    pub authentication: ServiceDiscoveryAuthentication,
    pub repository_witness: Arc<dyn Any + Send + Sync>,
}
#[derive(Clone, Copy)]
pub enum ServiceDiscoveryValidationPhase {
    BeforeMaterial,
    AfterMaterial,
    BeforeTransport,
}
pub trait ServiceModelDiscoveryAuthority: Send + Sync {
    fn operation_budget(
        &self,
        op: &Arc<ServiceModelDiscoveryOperation>,
    ) -> AppResult<OperationBudget>;
    fn prepared_budget(&self, p: &Arc<PreparedServiceModelDiscovery>)
    -> AppResult<OperationBudget>;
    fn attach_prepared(
        &self,
        op: &Arc<ServiceModelDiscoveryOperation>,
        p: Arc<PreparedServiceModelDiscovery>,
    ) -> AppResult<()>;
    fn active_pins(&self) -> AppResult<Vec<Arc<PreparedServiceModelDiscovery>>>;
    /// Called only after the original repository's terminal-fenced actor check.
    fn accept_validated_terminal(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        result: ServiceModelDiscoveryResult,
    ) -> AppResult<()>;
}
pub trait ServiceModelDiscoveryRepository: Send + Sync {
    fn prepare(
        &self,
        op: &Arc<ServiceModelDiscoveryOperation>,
    ) -> AppResult<Arc<PreparedServiceModelDiscovery>>;
    fn validate(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        phase: ServiceDiscoveryValidationPhase,
    ) -> AppResult<()>;
    /// The transport supplies its actual repository, not a caller label or UI frame.
    fn validate_gateway_owner(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        gateway_repository: &Arc<dyn super::ProductRepository>,
    ) -> AppResult<()>;
    fn complete(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        outcome: ServiceModelDiscoveryOutcome,
    ) -> AppResult<ServiceModelDiscoveryResult>;
}
pub trait ServiceModelDiscoveryBackend: Send + Sync {
    fn prepare(
        &self,
        op: Arc<ServiceModelDiscoveryOperation>,
        jobs: Arc<PhysicalJobs>,
    ) -> super::runtime::PortFuture<'_, Arc<PreparedServiceModelDiscovery>>;
    fn discover(
        &self,
        p: Arc<PreparedServiceModelDiscovery>,
        jobs: Arc<PhysicalJobs>,
    ) -> super::runtime::PortFuture<'_, Vec<DiscoveredServiceModel>>;
    fn complete(
        &self,
        p: Arc<PreparedServiceModelDiscovery>,
        outcome: ServiceModelDiscoveryOutcome,
        jobs: Arc<PhysicalJobs>,
    ) -> super::runtime::PortFuture<'_, ServiceModelDiscoveryResult>;
}
