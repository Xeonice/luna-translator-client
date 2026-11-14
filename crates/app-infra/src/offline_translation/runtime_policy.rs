//! Compiled material contracts, not loader admission. Even a matching policy
//! and reconstructed Expected value leave unimplemented loader routes Unknown.
use super::{OfflineEngine, OfflineErrorCode, OfflineFailure, OfflineResult, argos, windows};
use app_core::domain::{
    canonical_hash,
    model_config::service_graph::{OfflineBackend, OfflineEngineKey},
    offline_runtime::*,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

pub const APPLE_API_PROFILE: &str = "installed-source-low-latency-26.4-v1";
const ARGOS_PROCESSING: &str = "argos-1.11.0-stanza-local-v1";
const VENDOR_PROCESSING: &str = "vendor-unicode-abi-v1";
const APPLE_PROCESSING: &str = "apple-traditional-low-latency-v1";
fn one() -> NonZeroU32 {
    NonZeroU32::new(1).expect("constant nonzero")
}
fn failure(stage: &str) -> OfflineFailure {
    OfflineFailure::new(OfflineErrorCode::RuntimeIncompatible, stage)
}
fn canonical<T: Serialize>(value: &T) -> OfflineResult<String> {
    canonical_hash(value).map_err(|_| failure("runtime_policy_canonical"))
}
#[derive(Debug, Serialize)]
struct PolicyDefinition {
    contract: &'static str,
    engine: &'static str,
    platform: RuntimePlatform,
    architecture: RuntimeArchitecture,
    worker_platform: RuntimePlatform,
    private_protocol_revision: u32,
    processing_rule: &'static str,
    engine_runtime_constraint: &'static str,
    loader_admission: &'static str,
}
/// Private fields prevent caller metadata registering a new compiled policy.
/// This descriptor explicitly has no qualified loader implementation.
#[derive(Debug)]
pub struct CompiledRuntimePolicy {
    profile: RuntimeProfileRef,
    engine: OfflineEngine,
}
impl CompiledRuntimePolicy {
    pub fn profile(&self) -> &RuntimeProfileRef {
        &self.profile
    }
    pub fn engine(&self) -> OfflineEngine {
        self.engine
    }
    pub fn coverage(&self) -> RuntimeCoverageReport {
        RuntimeCoverageReport::Unknown {
            reason_code: match self.engine {
                OfflineEngine::AppleTranslation => "system_api_owner_not_bound",
                OfflineEngine::ArgosTranslate => "argos_loader_proof_not_implemented",
                OfflineEngine::Jbeijing | OfflineEngine::KingsoftFastait => {
                    "vendor_loader_proof_not_implemented"
                }
            }
            .into(),
        }
    }
    /// Typed declaration processing, not an observation of installed software.
    pub fn processing_for_declared_runtime(
        &self,
        runtime_version: &str,
    ) -> OfflineResult<RuntimeProcessingContract> {
        let (contract_id, settings) = match self.engine {
            OfflineEngine::ArgosTranslate => {
                let declared = argos_declaration(runtime_version)?;
                (
                    ARGOS_PROCESSING,
                    vec![
                        text_setting("pipeline", &declared.processing.pipeline),
                        text_setting("chunk_type", &declared.processing.chunk_type),
                        text_setting("device", &declared.processing.device),
                        text_setting("compute_type", &declared.processing.compute_type),
                        ProcessingSetting {
                            key: "inter_threads".into(),
                            value: ProcessingValue::Integer {
                                value: i64::from(declared.processing.inter_threads),
                            },
                        },
                        ProcessingSetting {
                            key: "intra_threads".into(),
                            value: ProcessingValue::Integer {
                                value: i64::from(declared.processing.intra_threads),
                            },
                        },
                        ProcessingSetting {
                            key: "effective_compute_types".into(),
                            value: ProcessingValue::TextList {
                                value: declared.processing.effective_compute_types,
                            },
                        },
                    ],
                )
            }
            OfflineEngine::Jbeijing | OfflineEngine::KingsoftFastait => {
                if runtime_version != vendor_abi(self.engine) {
                    return Err(failure("vendor_expected_abi"));
                }
                (
                    VENDOR_PROCESSING,
                    vec![
                        text_setting("abi", runtime_version),
                        text_setting("encoding", "utf16"),
                        text_setting("upstream_abi_commit", windows::UPSTREAM_ABI_COMMIT),
                    ],
                )
            }
            OfflineEngine::AppleTranslation => {
                if !runtime_version.starts_with("macOS-")
                    || !runtime_version.ends_with("/Translation/lowLatency/26.4")
                {
                    return Err(failure("apple_expected_runtime"));
                }
                (
                    APPLE_PROCESSING,
                    vec![
                        text_setting("api_profile", APPLE_API_PROFILE),
                        text_setting("strategy", "traditional_low_latency"),
                    ],
                )
            }
        };
        let mut contract = RuntimeProcessingContract {
            contract_id: contract_id.into(),
            revision: one(),
            settings,
        };
        contract
            .settings
            .sort_by(|a, b| a.key.as_bytes().cmp(b.key.as_bytes()));
        contract
            .canonical_sha256()
            .map_err(|_| failure("runtime_processing"))?;
        Ok(contract)
    }
    pub fn reconstruct_expected(
        &self,
        bundle: &RuntimeBundleManifest,
    ) -> OfflineResult<ExpectedRuntimeObservation> {
        bundle
            .validate()
            .map_err(|_| failure("runtime_bundle_contract"))?;
        if bundle.profile != self.profile || self.engine == OfflineEngine::AppleTranslation {
            return Err(failure("runtime_policy_unavailable"));
        }
        let processing = self.processing_for_declared_runtime(&bundle.expected.runtime_version)?;
        let processing_sha256 = processing
            .canonical_sha256()
            .map_err(|_| failure("runtime_processing"))?;
        if processing_sha256
            != bundle
                .processing
                .canonical_sha256()
                .map_err(|_| failure("runtime_processing"))?
        {
            return Err(failure("runtime_processing_mismatch"));
        }
        let expected = ExpectedRuntimeObservation {
            engine: self.profile.engine.clone(),
            runtime_version: bundle.expected.runtime_version.clone(),
            platform: self.profile.expected_worker_platform(),
            architecture: self.profile.architecture.clone(),
            resource_version: (self.engine == OfflineEngine::ArgosTranslate)
                .then(|| argos::ARGOS_VERSION.into()),
            physical_cancel_supported: false,
            processing_sha256,
        };
        if expected != bundle.expected {
            return Err(failure("runtime_expected_mismatch"));
        }
        let required = match self.engine {
            OfflineEngine::ArgosTranslate => &[
                RuntimeFileRole::Interpreter,
                RuntimeFileRole::StandardLibrary,
                RuntimeFileRole::PackageCode,
                RuntimeFileRole::NativeLibrary,
                RuntimeFileRole::Worker,
                RuntimeFileRole::LanguagePackage,
                RuntimeFileRole::Tokenizer,
                RuntimeFileRole::SentenceBoundary,
            ][..],
            _ => &[
                RuntimeFileRole::Worker,
                RuntimeFileRole::Engine,
                RuntimeFileRole::Dictionary,
            ][..],
        };
        if required
            .iter()
            .any(|role| !bundle.payload_files.iter().any(|f| &f.role == role))
        {
            return Err(failure("runtime_declared_roles_missing"));
        }
        // Required declarations reject omissions; they are never a proof of
        // actual loader semantics or of complete/native/delayed-load coverage.
        Ok(expected)
    }
    /// Rebuilds declarations from the actual stored Apple snapshot columns.
    /// It neither observes OS availability nor creates a SystemApiBound proof.
    pub fn expected_apple_system_declaration(
        &self,
        os_build: &str,
        api_profile: &str,
        strategy: &AppleStrategy,
    ) -> OfflineResult<ExpectedRuntimeObservation> {
        if self.engine != OfflineEngine::AppleTranslation {
            return Err(failure("runtime_policy_unavailable"));
        }
        if api_profile != APPLE_API_PROFILE
            || strategy != &AppleStrategy::TraditionalLowLatency
            || os_build.is_empty()
            || os_build == "unknown"
            || os_build.len() > 512
            || os_build.chars().any(char::is_control)
        {
            return Err(failure("apple_api_profile"));
        }
        let runtime_version = format!("macOS-{os_build}/Translation/lowLatency/26.4");
        let processing_sha256 = self
            .processing_for_declared_runtime(&runtime_version)?
            .canonical_sha256()
            .map_err(|_| failure("runtime_processing"))?;
        Ok(ExpectedRuntimeObservation {
            engine: self.profile.engine.clone(),
            runtime_version,
            platform: RuntimePlatform::Macos,
            architecture: self.profile.architecture.clone(),
            resource_version: None,
            physical_cancel_supported: false,
            processing_sha256,
        })
    }
    pub fn reconstruct_apple_expected(
        &self,
        reference: &OfflineResourceReference,
    ) -> OfflineResult<ExpectedRuntimeObservation> {
        reference
            .validate()
            .map_err(|_| failure("apple_resource_reference"))?;
        if self.engine != OfflineEngine::AppleTranslation || reference.profile != self.profile {
            return Err(failure("runtime_policy_unavailable"));
        }
        let ResourceBackendIdentity::AppleSystem {
            os_build,
            api_profile,
            strategy,
            ..
        } = &reference.backend
        else {
            return Err(failure("apple_resource_reference"));
        };
        let expected = self.expected_apple_system_declaration(os_build, api_profile, strategy)?;
        if expected.processing_sha256 != reference.processing_sha256
            || expected
                .canonical_sha256()
                .map_err(|_| failure("apple_expected"))?
                != reference.expected_observation_sha256
        {
            return Err(failure("apple_expected_mismatch"));
        }
        Ok(expected)
    }
}
fn text_setting(key: &str, value: &str) -> ProcessingSetting {
    ProcessingSetting {
        key: key.into(),
        value: ProcessingValue::Text {
            value: value.into(),
        },
    }
}
fn vendor_abi(engine: OfflineEngine) -> &'static str {
    match engine {
        OfflineEngine::Jbeijing => windows::JBEIJING_ABI,
        OfflineEngine::KingsoftFastait => windows::KINGSOFT_ABI,
        _ => "",
    }
}
pub fn registered_policy(
    engine: OfflineEngine,
    platform: RuntimePlatform,
    architecture: RuntimeArchitecture,
) -> OfflineResult<CompiledRuntimePolicy> {
    let supported = match engine {
        OfflineEngine::AppleTranslation => {
            platform == RuntimePlatform::Macos && architecture != RuntimeArchitecture::X86
        }
        OfflineEngine::ArgosTranslate => matches!(
            (&platform, &architecture),
            (
                RuntimePlatform::Macos,
                RuntimeArchitecture::Arm64 | RuntimeArchitecture::X64
            ) | (RuntimePlatform::Windows, RuntimeArchitecture::X64)
        ),
        OfflineEngine::Jbeijing | OfflineEngine::KingsoftFastait => {
            architecture == RuntimeArchitecture::X86
                && matches!(
                    platform,
                    RuntimePlatform::Windows | RuntimePlatform::Crossover
                )
        }
    };
    if !supported {
        return Err(failure("runtime_policy_unavailable"));
    }
    let rule = match engine {
        OfflineEngine::AppleTranslation => APPLE_PROCESSING,
        OfflineEngine::ArgosTranslate => ARGOS_PROCESSING,
        _ => VENDOR_PROCESSING,
    };
    let definition = PolicyDefinition {
        contract: "offline-material-policy-v1",
        engine: engine.id(),
        platform: platform.clone(),
        architecture: architecture.clone(),
        worker_platform: if platform == RuntimePlatform::Crossover {
            RuntimePlatform::Windows
        } else {
            platform.clone()
        },
        private_protocol_revision: PRIVATE_PROTOCOL_REVISION,
        processing_rule: rule,
        engine_runtime_constraint: match engine {
            OfflineEngine::AppleTranslation => APPLE_API_PROFILE,
            OfflineEngine::ArgosTranslate => argos::ARGOS_VERSION,
            _ => vendor_abi(engine),
        },
        loader_admission: "unknown-no-actual-loader-proof",
    };
    let id = format!(
        "luna.{}.material-v2.{:?}.{:?}",
        engine.id(),
        platform,
        architecture
    )
    .to_ascii_lowercase();
    let profile = RuntimeProfileRef {
        profile_id: id,
        revision: one(),
        engine: OfflineEngineKey {
            engine_id: engine.id().into(),
            contract_revision: one(),
        },
        backend: if engine == OfflineEngine::AppleTranslation {
            OfflineBackend::PlatformNative
        } else {
            OfflineBackend::LocalWorker
        },
        platform,
        architecture,
        private_protocol_revision: NonZeroU32::new(PRIVATE_PROTOCOL_REVISION)
            .expect("constant nonzero"),
        policy_sha256: canonical(&("offline-compiled-material-policy-v1", definition))?,
    };
    Ok(CompiledRuntimePolicy { profile, engine })
}
pub fn policy_for_profile(profile: &RuntimeProfileRef) -> OfflineResult<CompiledRuntimePolicy> {
    profile
        .validate()
        .map_err(|_| failure("runtime_policy_unavailable"))?;
    let engine = OfflineEngine::parse(&profile.engine.engine_id)
        .map_err(|_| failure("runtime_policy_unavailable"))?;
    let registered = registered_policy(
        engine,
        profile.platform.clone(),
        profile.architecture.clone(),
    )?;
    if profile != registered.profile() {
        return Err(failure("runtime_policy_unavailable"));
    }
    Ok(registered)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgosVersionDeclaration {
    argostranslate: String,
    ctranslate2: String,
    sentencepiece: String,
    stanza: String,
    minisbd: String,
    packaging: String,
    python: String,
    processing: ArgosProcessing,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArgosProcessing {
    pipeline: String,
    chunk_type: String,
    device: String,
    compute_type: String,
    inter_threads: u32,
    intra_threads: u32,
    effective_compute_types: Vec<String>,
}
fn argos_declaration(raw: &str) -> OfflineResult<ArgosVersionDeclaration> {
    if raw.len() > 4096 {
        return Err(failure("argos_expected_declaration"));
    }
    let d: ArgosVersionDeclaration =
        serde_json::from_str(raw).map_err(|_| failure("argos_expected_declaration"))?;
    let scalar = |s: &str| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control);
    if d.argostranslate != argos::ARGOS_VERSION
        || [
            &d.ctranslate2,
            &d.sentencepiece,
            &d.stanza,
            &d.minisbd,
            &d.packaging,
            &d.python,
            &d.processing.compute_type,
        ]
        .iter()
        .any(|v| !scalar(v))
        || d.processing.pipeline != ARGOS_PROCESSING
        || d.processing.chunk_type != "STANZA"
        || d.processing.device != "cpu"
        || d.processing.effective_compute_types.is_empty()
        || d.processing.effective_compute_types.len() > MAX_STORED_ROUTE_HOPS
        || d.processing
            .effective_compute_types
            .iter()
            .any(|v| !scalar(v))
    {
        return Err(failure("argos_expected_declaration"));
    }
    let canonical_json = serde_json::to_string(
        &serde_json::to_value(&d).map_err(|_| failure("argos_expected_declaration"))?,
    )
    .map_err(|_| failure("argos_expected_declaration"))?;
    if raw != canonical_json {
        return Err(failure("argos_expected_noncanonical"));
    }
    Ok(d)
}
