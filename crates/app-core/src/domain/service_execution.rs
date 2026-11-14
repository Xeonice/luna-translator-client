//! Frozen, non-secret planning data. A plan never grants repository, Vault or runtime authority.
use crate::{
    contracts::{CatalogPurpose, LexicalToken},
    domain::model_config::service_graph::{
        ConfigurationV3, ConnectionOrigin, ConnectionV3, EffectiveServicePolicy,
        HttpServiceOptionsV1, ModelV3, OfflineEngineConfigV1, SavedServiceRef, TemplateV3,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Caller freezes this against the actual repository version before obtaining any secret lease.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServicePlanIdentity {
    pub configuration_revision: String,
    pub binding_id: String,
    pub engine_id: String,
    pub purpose: CatalogPurpose,
    pub unit_id: String,
    pub text_revision: u32,
    pub source_hash: String,
    pub session_id: String,
    pub game_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanSourceKind {
    Service,
    Scope,
    Engine,
    Template,
    Term,
    ReadingUnit,
    ReadingPreference,
    CatalogContract,
    Builtin,
    /// An explicit saved-service test requirement, not an active purpose scope.
    TestRequest,
}
/// Describes where a field came from, not proof that its resource is authorized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanFieldSource {
    pub kind: PlanSourceKind,
    pub revision_id: Option<String>,
    pub entity_id: Option<String>,
    pub field: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenHttpServicePlan<R> {
    pub connection: ConnectionV3,
    pub model: ModelV3,
    pub http_options: HttpServiceOptionsV1,
    pub origin: ConnectionOrigin,
    pub request: R,
    pub template: Option<TemplateV3>,
    #[serde(default)]
    pub reference_material: HttpReferenceMaterial,
}
/// Explicitly consented data. Dedicated translation adapters must use these
/// fields rather than parsing model prompts or silently dropping references.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpReferenceMaterial {
    pub context: Vec<serde_json::Value>,
    pub glossary: Vec<serde_json::Value>,
}
/// Real offline input. No HTTP endpoint, model ID, generation parameter, message or secret field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineRequestPayload {
    pub source_text: String,
    pub source_language: String,
    pub target_language: String,
    pub context: Vec<serde_json::Value>,
    pub glossary: Vec<serde_json::Value>,
    pub selected_token: Option<LexicalToken>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ServiceExecutionTarget<R> {
    Http {
        plan: Box<FrozenHttpServicePlan<R>>,
    },
    Offline {
        config: Box<OfflineEngineConfigV1>,
        payload: Box<OfflineRequestPayload>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceExecutionPlan<R> {
    pub identity: ServicePlanIdentity,
    pub policy: EffectiveServicePolicy,
    pub fallback: Vec<SavedServiceRef>,
    pub provenance: BTreeMap<String, PlanFieldSource>,
    pub context_units: Vec<(String, u32)>,
    pub term_sources: Vec<(String, String)>,
    pub target: ServiceExecutionTarget<R>,
}

/// The codec3 planner does not reinterpret materialized LegacyV2 bindings.
pub fn has_typed_semantics(graph: &ConfigurationV3, binding_id: &str) -> Option<bool> {
    graph
        .bindings
        .iter()
        .find(|b| b.binding_id == binding_id)
        .map(|b| {
            b.semantics_version
                == crate::domain::model_config::service_graph::PurposeSemanticsVersion::TypedV3
        })
}

/// Static configuration validation never replaces these authoritative runtime checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ServicePreflightRequirement {
    InstalledAdapter,
    CredentialAuthority,
    GoogleAuthorizationAuthority,
    OfflineResourceAuthority,
    EffectiveRequestLanguages,
    ModelCapabilities,
    NetworkAvailability,
    MaterialCondition {
        reason: crate::contracts::CatalogReason,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfigPreflight {
    pub target: crate::domain::model_config::service_graph::EngineTarget,
    pub purpose: CatalogPurpose,
    pub runtime_requirements: Vec<ServicePreflightRequirement>,
}
