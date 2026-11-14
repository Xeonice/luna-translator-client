//! Synchronous storage and immutable-directory seams. Application executor owns ordering;
//! UI input cannot select activation roles or fabricate repository provenance.
use crate::{
    contracts::*,
    domain::model_config::service_graph::{ConfigurationV3, EngineTarget},
};

/// Metadata from a real sealed user source. This alone grants no I/O authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedServiceSourceProof {
    pub source: crate::domain::model_config::service_graph::SavedServiceRef,
    pub logical_resource_id: String,
    pub codec_version: u32,
    pub content_sha256: String,
    pub seal_sha256: String,
    pub canonical_control_id: Option<String>,
}

/// Internal compiled command. This has no serde/TS implementation and is never accepted by IPC.
pub struct CompiledServiceDraft {
    pub operation_id: String,
    pub request_hash: String,
    pub expected_profile_generation: std::num::NonZeroU32,
    pub expected_draft_head_revision: u32,
    pub graph: ConfigurationV3,
    pub resource: EngineTarget,
}

pub trait ServiceDirectory: Send + Sync {
    fn catalog(&self) -> ServiceCatalog;
    fn form(&self, request: &ServiceFormRequest) -> AppResult<ServiceFormProjection>;
}

pub trait ServiceConfigurationRepository: Send + Sync {
    /// Public client metadata from an actual owned current grant. This is not
    /// caller-supplied setup evidence and never returns token/private material.
    fn google_client_draft(
        &self,
        _authorization: &crate::domain::model_config::service_graph::GoogleAuthorizationRef,
        _expected_profile_generation: std::num::NonZeroU32,
    ) -> AppResult<crate::domain::model_config::service_draft::PreparedGoogleClientDraft> {
        Err(AppError::new("google_client_preparation_required", "service_configuration", "尚未装配已保存授权的客户端核验"))
    }
    fn service_draft_replay(
        &self,
        operation_id: &str,
        request_hash: &str,
        expected_profile_generation: std::num::NonZeroU32,
    ) -> AppResult<Option<ServiceDraftReceipt>>;
    fn service_configuration(&self) -> AppResult<ServiceConfigurationSnapshot>;
    fn service_configuration_revision(&self, revision: &str) -> AppResult<ConfigurationDocument>;
    /// Caller supplies a compiled graph, not raw form values. Repository independently
    /// verifies legacy materialization origins, parent/seal/pool authority and CAS.
    fn save_service_configuration(
        &self,
        command: &CompiledServiceDraft,
        now_ms: i64,
    ) -> AppResult<ServiceDraftReceipt>;
    fn saved_services(&self, query: &SavedServiceQuery) -> AppResult<SavedServiceProjection>;
    fn select_saved_services(
        &self,
        input: &SelectSavedServicesInput,
        now_ms: i64,
    ) -> AppResult<ServiceSelectionReceipt>;
    fn restore_service_inheritance(
        &self,
        input: &RestoreServiceInheritanceInput,
        now_ms: i64,
    ) -> AppResult<ServiceSelectionReceipt>;
    fn set_service_purpose_enabled(
        &self,
        input: &SetServicePurposeEnabledInput,
        now_ms: i64,
    ) -> AppResult<ServiceSelectionReceipt>;
}
