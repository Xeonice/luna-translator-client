//! Private saved-service test port. Plans and material locators never become IPC snapshots.
use crate::{
    contracts::*,
    domain::{
        credentials::CredentialSelection,
        model_config::{credentials::CredentialStorageMetadata, service_graph::*},
        service_testing::SavedServiceTestPlan,
    },
    ports::{
        google_authorization::{GoogleExecutionLeaseRecord, GoogleOwnedGrant},
        operation_budget::OperationBudget,
    },
};
use std::{num::NonZeroU32, sync::Arc};

#[derive(Clone)]
pub struct PrepareServiceTestInput {
    pub operation_id: String,
    pub request: SavedServiceTestRequest,
    pub selected_token: Option<LexicalToken>,
}
pub type PreparedTestSourceProof = super::service_configuration::SavedServiceSourceProof;
#[derive(Clone)]
pub enum PreparedTestReading {
    Sample,
    Selected {
        unit: Box<ReadingUnit>,
        selected_token: Option<LexicalToken>,
        lexical_snapshot_id: Option<String>,
        history: Vec<ReadingUnit>,
    },
}
#[derive(Clone)]
pub struct PreparedTestCredential {
    pub selection: CredentialSelection,
    pub storage: CredentialStorageMetadata,
    pub lifecycle_revision: u32,
    pub pool_policy_sha256: String,
}
impl std::fmt::Debug for PreparedTestCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedTestCredential")
            .field("credential_id", &self.selection.credential_id)
            .field("pool_id", &self.selection.pool_id)
            .field("pool_revision", &self.selection.pool_revision)
            .field("member_id", &self.selection.member_id)
            .field("lifecycle_revision", &self.lifecycle_revision)
            .finish_non_exhaustive()
    }
}
#[derive(Clone)]
pub enum PreparedTestAuthentication {
    None,
    PoolMember(PreparedTestCredential),
    GoogleOwnedGrant(GoogleOwnedGrant),
}
#[derive(Clone)]
pub struct PreparedServiceTest {
    pub test_id: String,
    pub input: PrepareServiceTestInput,
    pub profile_id: String,
    pub profile_generation: NonZeroU32,
    pub server_instance_id: String,
    pub started_at_ms: i64,
    pub source: PreparedTestSourceProof,
    pub reading: PreparedTestReading,
    pub authentication: PreparedTestAuthentication,
    pub plan: SavedServiceTestPlan,
    pub source_semantics_version: String,
    pub source_semantics_sha256: String,
    pub request_semantics_version: String,
    pub request_semantics_sha256: String,
    pub input_sha256: String,
    pub authority_sha256: String,
    pub authority_blocker: Option<String>,
    pub execution_blocker: Option<AppError>,
}
#[derive(Clone)]
pub enum ServiceTestTerminalOutcome {
    Succeeded { response: String },
    Failed { error: AppError },
    Cancelled,
    TimedOut,
    Unsupported { code: String },
}
#[derive(Clone)]
pub struct FinishServiceTestInput {
    pub prepared: Arc<PreparedServiceTest>,
    pub outcome: ServiceTestTerminalOutcome,
    /// The same operation-owned fence used by the Controller and all cancellation handles.
    pub budget: OperationBudget,
    pub request_sent: bool,
    pub credential_used: bool,
    pub google_lease: Option<GoogleExecutionLeaseRecord>,
}
#[derive(Debug, Clone)]
pub struct ServiceTestQualificationQuery {
    pub expected_profile_generation: NonZeroU32,
    pub source: SavedServiceRef,
    pub purpose: CatalogPurpose,
    pub output_mode: ServiceTestOutputMode,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceTestComparison {
    Compared,
    Stale,
    NotCovered,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTestCredentialCoverage {
    pub credential_id: String,
    pub pool_id: Option<String>,
    pub pool_revision: Option<u32>,
    pub member_id: Option<String>,
    pub status: ServiceTestStatus,
}
#[derive(Debug, Clone)]
pub struct ServiceTestQualification {
    pub current_source: SavedServiceRef,
    pub tested_source: Option<SavedServiceRef>,
    pub test_id: Option<String>,
    pub purpose: CatalogPurpose,
    pub output_mode: ServiceTestOutputMode,
    pub status: Option<ServiceTestStatus>,
    pub comparison: ServiceTestComparison,
    pub credentials: Vec<ServiceTestCredentialCoverage>,
    pub required_credentials: usize,
    pub covered_credentials: usize,
    pub complete_credential_coverage: bool,
    pub input_scope: Option<ServiceTestInputScope>,
    pub tested_at_ms: Option<i64>,
    pub reasons: Vec<String>,
}

pub trait ServiceTestingRepository: Send + Sync {
    fn replay_service_test(
        &self,
        _input: &PrepareServiceTestInput,
    ) -> AppResult<Option<ServiceTestResult>> {
        Err(AppError::new(
            "service_test_replay_unavailable",
            "service_test",
            "当前存储尚未提供真实测试操作重放",
        ))
    }
    fn prepare_service_test(
        &self,
        input: PrepareServiceTestInput,
        started_at_ms: i64,
    ) -> AppResult<PreparedServiceTest>;
    fn validate_prepared_test(&self, prepared: &Arc<PreparedServiceTest>) -> AppResult<()>;
    fn finish_service_test(
        &self,
        input: FinishServiceTestInput,
        completed_at_ms: i64,
    ) -> AppResult<ServiceTestResult>;
    fn service_test_qualification(
        &self,
        query: ServiceTestQualificationQuery,
    ) -> AppResult<ServiceTestQualification>;
}

impl std::fmt::Debug for PreparedTestReading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sample => f.write_str("PreparedTestReading::Sample"),
            Self::Selected { unit, .. } => f
                .debug_struct("PreparedTestReading::Selected")
                .field("unit_id", &unit.unit_id)
                .field("text_revision", &unit.text_revision)
                .field("source_hash", &unit.source_hash)
                .finish_non_exhaustive(),
        }
    }
}
impl std::fmt::Debug for PreparedServiceTest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedServiceTest")
            .field("test_id", &self.test_id)
            .field("source", &self.source)
            .field("purpose", &self.plan.identity.purpose)
            .field("output_mode", &self.plan.identity.output_mode)
            .field("request_semantics_sha256", &self.request_semantics_sha256)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for ServiceTestTerminalOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Succeeded { .. } => {
                f.write_str("ServiceTestTerminalOutcome::Succeeded([response omitted])")
            }
            Self::Failed { error } => f
                .debug_struct("ServiceTestTerminalOutcome::Failed")
                .field("code", &error.code)
                .field("stage", &error.stage)
                .finish(),
            Self::Cancelled => f.write_str("ServiceTestTerminalOutcome::Cancelled"),
            Self::TimedOut => f.write_str("ServiceTestTerminalOutcome::TimedOut"),
            Self::Unsupported { code } => f
                .debug_tuple("ServiceTestTerminalOutcome::Unsupported")
                .field(code)
                .finish(),
        }
    }
}

/// A narrow read-only lens over the application-owned live operation, not a second scheduler.
pub trait ServiceTestOperationAuthority: Send + Sync {
    /// Match the original prepared identity. Terminal cancellation remains a fact,
    /// while budget disposition is independently checked by the repository.
    fn operation_budget(&self, prepared: &Arc<PreparedServiceTest>) -> AppResult<OperationBudget>;
    fn active_prepared(&self) -> AppResult<Vec<Arc<PreparedServiceTest>>> {
        Err(AppError::new(
            "service_test_protection_unavailable",
            "service_test",
            "无法核验当前测试的真实保护对象",
        ))
    }
}

impl std::fmt::Debug for PrepareServiceTestInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrepareServiceTestInput")
            .field("operation_id", &self.operation_id)
            .field("source", &self.request.source)
            .field("purpose", &self.request.purpose)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for PreparedTestAuthentication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("PreparedTestAuthentication::None"),
            Self::PoolMember(p) => p.fmt(f),
            Self::GoogleOwnedGrant(g) => f
                .debug_struct("PreparedTestAuthentication::GoogleOwnedGrant")
                .field("grant_id", &g.cursor.grant_id)
                .field("authorization_epoch", &g.cursor.authorization_epoch)
                .finish_non_exhaustive(),
        }
    }
}

impl std::fmt::Debug for FinishServiceTestInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinishServiceTestInput")
            .field("test_id", &self.prepared.test_id)
            .field("outcome", &self.outcome)
            .field("request_sent", &self.request_sent)
            .field("credential_used", &self.credential_used)
            .field("has_google_lease", &self.google_lease.is_some())
            .finish_non_exhaustive()
    }
}
