//! Local translation execution boundary. Repository/lease authority stays in app-core/runtime.
//! No business queue, cache, fallback, HTTP client or implicit resource installation lives here.
pub mod apple;
pub mod argos;
pub mod closure;
mod inspection;
pub mod protocol;
pub mod resolver;
pub mod resources;
pub mod runtime_policy;
pub mod supervisor;
pub mod v2_protocol;
pub mod v2_supervisor;
pub mod windows;

use app_core::domain::{
    model_config::service_graph::{LanguagePair, OfflineEngineConfigV1},
    service_execution::OfflineRequestPayload,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

pub const PROTOCOL: &str = "translation-worker-v1";
pub const MAX_TEXT_BYTES: usize = 65_536;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const MAX_OUTPUT_BYTES: usize = 262_144;
pub const MAX_RESOURCE_FILES: usize = 16_384;
pub const MAX_ROUTE_HOPS: usize = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfflineEngine {
    AppleTranslation,
    ArgosTranslate,
    Jbeijing,
    KingsoftFastait,
}
impl OfflineEngine {
    pub fn id(self) -> &'static str {
        match self {
            Self::AppleTranslation => "apple_translation",
            Self::ArgosTranslate => "argos_translate",
            Self::Jbeijing => "jbeijing",
            Self::KingsoftFastait => "kingsoft_fastait",
        }
    }
    pub fn parse(id: &str) -> Result<Self, OfflineFailure> {
        match id {
            "apple_translation" => Ok(Self::AppleTranslation),
            "argos_translate" => Ok(Self::ArgosTranslate),
            "jbeijing" => Ok(Self::Jbeijing),
            "kingsoft_fastait" => Ok(Self::KingsoftFastait),
            _ => Err(OfflineFailure::new(
                OfflineErrorCode::UnsupportedEngine,
                "configuration",
            )),
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfflineErrorCode {
    UnsupportedEngine,
    UnsupportedContract,
    UnsupportedPlatform,
    UnsupportedPair,
    UnsupportedInput,
    NotInstalled,
    MissingResource,
    ResourceChanged,
    InvalidResource,
    ResourceBudgetExceeded,
    RuntimeIncompatible,
    ArchitectureMismatch,
    RequiresInteractiveSystemHost,
    PermissionDenied,
    NotReady,
    InvalidRequest,
    ProtocolViolation,
    WorkerExited,
    WorkerStartFailed,
    WorkerBusy,
    DeadlineExceeded,
    Cancelled,
    EngineFailed,
    OutputTooLarge,
}
/// Deliberately excludes external stderr/exception/path/text details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineFailure {
    pub code: OfflineErrorCode,
    pub stage: String,
}
impl OfflineFailure {
    pub fn new(code: OfflineErrorCode, stage: &str) -> Self {
        Self {
            code,
            stage: stage.into(),
        }
    }
}
impl std::fmt::Display for OfflineFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} ({})", self.code, self.stage)
    }
}
impl std::error::Error for OfflineFailure {}
pub type OfflineResult<T> = Result<T, OfflineFailure>;

/// Supplied only by the Rust composition root after resolving a repository-owned lease.
/// These strings are correlation evidence, never an independent authorization grant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionIdentity {
    pub request_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub configuration_revision: String,
    pub profile_generation: u64,
    pub authorization_epoch: u64,
    pub lease_id: String,
    pub unit_id: String,
    pub text_revision: u32,
    pub source_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceIdentity {
    pub snapshot_id: String,
    pub route_id: String,
    pub observation_id: String,
    pub runtime_profile_id: String,
    pub runtime_profile_revision: u32,
    pub installation_revision: u32,
    pub installation_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceFile {
    pub relative_path: String,
    pub sha256: String,
    pub length: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteHop {
    pub pair: LanguagePair,
    pub package_relative_path: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceManifest {
    /// Only a user-selected, canonical, locally authorized directory. Never exported.
    pub root: PathBuf,
    pub identity: ResourceIdentity,
    pub digest: String,
    pub files: Vec<ResourceFile>,
    pub route: Vec<RouteHop>,
    pub engine_version: String,
}
/// Typed launch recipe: no shell, arbitrary argv, inherited env, or caller-supplied script.
#[derive(Debug, Clone)]
pub enum WorkerLaunch {
    Native {
        executable: PathBuf,
        executable_sha256: String,
    },
    Argos {
        interpreter: PathBuf,
        interpreter_sha256: String,
        script: PathBuf,
        script_sha256: String,
    },
    CrossOver {
        launcher: PathBuf,
        launcher_sha256: String,
        bottle: String,
        worker: PathBuf,
        worker_sha256: String,
        worker_resource_root: String,
    },
}
#[derive(Debug, Clone)]
pub struct OfflineExecution {
    pub identity: ExecutionIdentity,
    pub config: OfflineEngineConfigV1,
    pub payload: OfflineRequestPayload,
    pub resource: ResourceManifest,
    pub launch: WorkerLaunch,
    pub timeout: Duration,
    pub expected_observation: RuntimeObservation,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum ResourceAvailability {
    Installed,
    SupportedNotInstalled,
    Unsupported,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeObservation {
    pub engine_id: String,
    pub engine_contract_revision: u32,
    pub runtime_version: String,
    pub platform: String,
    pub architecture: String,
    pub availability: ResourceAvailability,
    pub physical_cancel_supported: bool,
    pub resource_version: Option<String>,
    pub reason: Option<OfflineErrorCode>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineOutput {
    pub text: String,
    pub identity: ExecutionIdentity,
    pub resource: ResourceIdentity,
    pub resource_digest: String,
    pub route: Vec<RouteHop>,
    pub worker_instance_id: String,
    pub observation: RuntimeObservation,
}

#[derive(Debug, Clone)]
pub struct OfflineProbe {
    pub request_id: String,
    pub engine: OfflineEngine,
    pub contract_revision: u32,
    pub resource: ResourceManifest,
    pub launch: WorkerLaunch,
    pub timeout: Duration,
}
pub use supervisor::OfflineExecutor;
