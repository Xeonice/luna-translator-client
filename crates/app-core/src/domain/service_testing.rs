//! A saved-service test is an explicit operation, never a reading task or an
//! activation. These internal values intentionally have no Serde/IPC contract.
use crate::{
    application::request_plan::RequestPlan,
    contracts::{CatalogPurpose, ServiceTestOutputMode},
    domain::{
        model_config::service_graph::{EffectiveServicePolicy, SavedServiceRef},
        service_execution::{PlanFieldSource, ServiceExecutionTarget},
    },
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedServiceTestInputIdentity {
    Sample {
        sample_id: String,
        source_hash: String,
    },
    SelectedUnit {
        unit_id: String,
        text_revision: u32,
        source_hash: String,
        session_id: String,
        game_id: Option<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedServiceTestIdentity {
    pub source: SavedServiceRef,
    pub purpose: CatalogPurpose,
    pub output_mode: ServiceTestOutputMode,
    pub input: SavedServiceTestInputIdentity,
}
/// This internal authorization-free plan cannot be exported as an IPC object.
/// ```compile_fail
/// use app_core::domain::service_testing::SavedServiceTestPlan;
/// fn export(plan: &SavedServiceTestPlan) {
///     let _ = serde_json::to_string(plan);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SavedServiceTestPlan {
    pub identity: SavedServiceTestIdentity,
    /// The actual fixed selection, not a live global UI token.
    pub selected_token: Option<crate::contracts::LexicalToken>,
    pub policy: EffectiveServicePolicy,
    pub provenance: BTreeMap<String, PlanFieldSource>,
    pub context_units: Vec<(String, u32)>,
    pub term_sources: Vec<(String, String)>,
    pub target: ServiceExecutionTarget<RequestPlan>,
}

/// Non-secret evidence scope. A selected game narrows terms; it never changes
/// the request to whichever game happens to be active later.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServiceTestEvidenceDomain {
    SampleJapaneseV1,
    Selected {
        game_id: Option<String>,
        source_language: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTestSemanticFingerprint {
    pub version: &'static str,
    pub sha256: String,
    pub(crate) purpose: CatalogPurpose,
    pub(crate) output_mode: ServiceTestOutputMode,
    pub(crate) evidence_domain: ServiceTestEvidenceDomain,
}
/// Projection is a description of supplied authority facts, never permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceTestAuthorityCoverage {
    Projected,
    Unresolved { code: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTestRequestSemantics {
    pub version: &'static str,
    pub sha256: String,
    pub input_sha256: String,
    pub authority_sha256: String,
    pub authority_coverage: ServiceTestAuthorityCoverage,
}

/// Return only application-owned diagnostic vocabulary. Raw provider messages,
/// bodies, URLs and arbitrary strings are never reflected into a public result.
pub fn safe_test_failure(
    code: &str,
    stage: Option<&str>,
) -> crate::contracts::ServiceTestFailureDetail {
    let recovery = match code {
        "missing_credential"
        | "credential_unavailable"
        | "credential_kind_mismatch"
        | "invalid_credential"
        | "missing_pool" => Some("核对并保存此服务需要的凭据，再重新测试。"),
        "authentication_failed" | "google_authorization_rejected" => {
            Some("服务拒绝授权，请核对账号权限及所选凭据。")
        }
        "rate_limited" | "quota_exceeded" => Some("检查服务配额或限流设置，稍后再试。"),
        "missing_model" | "model_not_found" | "missing_endpoint" | "invalid_endpoint"
        | "protocol_mismatch" | "http_error" => Some("核对服务地址、接口类型和实际模型ID。"),
        "operation_timeout" | "timeout" | "authorization_timeout" => {
            Some("检查服务是否正在运行；必要时调整超时后重新测试。")
        }
        "network_error"
        | "connection_failed"
        | "google_authorization_network"
        | "service_unavailable" => Some("检查服务是否可用，以及网络和代理设置，再重新测试。"),
        "google_client_setup_unverified"
        | "google_client_preparation_required"
        | "invalid_google_client" => Some("先配置Google桌面登录客户端，再重新登录。"),
        "google_grant_inactive"
        | "google_grant_missing"
        | "google_needs_reauth"
        | "google_invalid_grant"
        | "google_refresh_material_invalid"
        | "google_scope_mismatch"
        | "oauth_grant_required" => Some("Google授权尚未完成、范围不符或已经失效，请重新登录。"),
        "invalid_structured_output"
        | "invalid_test_output"
        | "invalid_output"
        | "empty_output"
        | "empty_response"
        | "malformed_response"
        | "malformed_stream"
        | "model_error"
        | "output_incomplete"
        | "stream_incomplete"
        | "stream_interrupted"
        | "response_too_large" => Some("返回内容不符合本用途要求；检查模板、输出模式与模型能力。"),
        "offline_resource_authority_unavailable"
        | "offline_resource_authority_unresolved"
        | "offline_test_authority_unresolved"
        | "offline_runtime_not_implemented" => {
            Some("先检查本机引擎、语言资源及运行条件，再测试翻译。")
        }
        "service_test_execution_unavailable"
        | "adapter_not_implemented"
        | "unsupported_authentication"
        | "unsupported_protocol"
        | "unsupported_purpose"
        | "discovery_unsupported" => Some("当前接口或所需能力尚不可用，请核对所选服务及协议。"),
        "profile_generation_conflict"
        | "service_source_changed"
        | "test_reading_identity_mismatch"
        | "prepared_test_mismatch"
        | "connection_suspended"
        | "google_handler_unavailable"
        | "google_handler_frame_mismatch" => {
            Some("配置、输入或运行状态已变化；重新读取并核对后再测试。")
        }
        _ => None,
    };
    let code = if recovery.is_some() {
        code
    } else {
        "service_test_failed"
    };
    let stage = stage
        .filter(|s| {
            matches!(
                *s,
                "provider"
                    | "service_test"
                    | "configuration"
                    | "credentials"
                    | "google_authorization"
                    | "storage"
                    | "models"
                    | "test"
                    | "operation_budget"
                    | "output_validation"
            )
        })
        .map(str::to_owned);
    crate::contracts::ServiceTestFailureDetail {
        code: code.into(),
        stage,
        recovery: recovery
            .unwrap_or("测试未完成，请核对此项服务配置；原编辑和当前选用保持不变。")
            .into(),
    }
}
