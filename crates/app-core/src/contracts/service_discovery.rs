//! Safe model-list request/results. No connection, route, credential or plan input.
use super::{AppError, AppResult, CatalogPurpose};
use crate::domain::model_config::service_graph::SavedServiceRef;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedServiceModelDiscoveryRequest {
    pub expected_profile_generation: NonZeroU32,
    pub source: SavedServiceRef,
    pub purpose: CatalogPurpose,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceModelDiscoveryInput {
    pub operation_id: String,
    pub request: SavedServiceModelDiscoveryRequest,
}
impl ServiceModelDiscoveryInput {
    pub fn validate(&self) -> AppResult<()> {
        bounded_id(&self.operation_id, 128)?;
        match &self.request.source {
            SavedServiceRef::Http { source } => {
                bounded_id(&source.revision_id, 512)?;
                bounded_id(&source.model_id, 512)
            }
            SavedServiceRef::Offline { source } => {
                bounded_id(&source.revision_id, 512)?;
                bounded_id(&source.config_id, 512)
            }
        }
    }
}
pub(crate) fn bounded_id(value: &str, limit: usize) -> AppResult<()> {
    crate::domain::require_id(value)?;
    if value.len() > limit {
        return Err(AppError::new(
            "discovery_identity_too_large",
            "service_model_discovery",
            "本次获取请求的对象身份超过上限",
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelServiceModelDiscoveryInput {
    pub operation_id: String,
    pub expected_profile_generation: NonZeroU32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveredServiceModel {
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServiceModelDiscoveryOutcome {
    Available {
        models: Vec<DiscoveredServiceModel>,
    },
    Unsupported {
        reason: ServiceModelDiscoveryFailure,
    },
    Failed {
        reason: ServiceModelDiscoveryFailure,
    },
    Cancelled,
    TimedOut,
    ResultExpired {
        reason: ServiceModelDiscoveryFailure,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceModelDiscoveryFailure {
    pub code: String,
    pub stage: String,
    pub recovery: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceModelDiscoveryResult {
    pub operation_id: String,
    pub request: SavedServiceModelDiscoveryRequest,
    pub outcome: ServiceModelDiscoveryOutcome,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceModelDiscoveryCancelDisposition {
    Requested,
    AlreadyTerminal,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceModelDiscoveryCancelReceipt {
    pub operation_id: String,
    pub disposition: ServiceModelDiscoveryCancelDisposition,
}
pub(super) fn typescript_declarations() -> String {
    [
        SavedServiceModelDiscoveryRequest::decl(&ts_rs::Config::default()),
        ServiceModelDiscoveryInput::decl(&ts_rs::Config::default()),
        CancelServiceModelDiscoveryInput::decl(&ts_rs::Config::default()),
        DiscoveredServiceModel::decl(&ts_rs::Config::default()),
        ServiceModelDiscoveryOutcome::decl(&ts_rs::Config::default()),
        ServiceModelDiscoveryFailure::decl(&ts_rs::Config::default()),
        ServiceModelDiscoveryResult::decl(&ts_rs::Config::default()),
        ServiceModelDiscoveryCancelDisposition::decl(&ts_rs::Config::default()),
        ServiceModelDiscoveryCancelReceipt::decl(&ts_rs::Config::default()),
    ]
    .map(|v| format!("export {v}\n"))
    .join("\n")
}
