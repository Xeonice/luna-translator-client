use super::*;

/// Observations come only through the trusted resource port, not metadata/IPC.
/// Even a matching positive report is not a private authorization proof or lease.
/// ```compile_fail
/// use app_core::domain::offline_runtime::RuntimeCoverageReport;
/// fn export(report:&RuntimeCoverageReport){let _=serde_json::to_string(report);}
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCoverageReport {
    ActualReviewedFileClosure {
        runtime_content_sha256: String,
        loader_policy_sha256: String,
        processing_sha256: String,
        evidence_sha256: String,
    },
    SystemApiBound {
        os_build: String,
        bridge_build_sha256: String,
        api_profile: String,
        strategy: AppleStrategy,
        observation_epoch: NonZeroU32,
    },
    Unknown {
        reason_code: String,
    },
    Unsupported {
        reason_code: String,
    },
}
/// Shape/binding check for an owner-reported fact. O5 still has to verify actual
/// stored/physical authority; this intentionally returns no proof/lease object.
pub fn validate_required_coverage_binding(
    report: &RuntimeCoverageReport,
    resource: &OfflineResourceReference,
) -> AppResult<()> {
    resource.validate()?;
    match (report, &resource.backend) {
        (RuntimeCoverageReport::Unknown { reason_code }, _) => {
            text(reason_code, 160)?;
            Err(invalid("offline_coverage_unknown"))
        }
        (RuntimeCoverageReport::Unsupported { reason_code }, _) => {
            text(reason_code, 160)?;
            Err(invalid("offline_coverage_unsupported"))
        }
        (
            RuntimeCoverageReport::ActualReviewedFileClosure {
                runtime_content_sha256,
                loader_policy_sha256,
                processing_sha256,
                evidence_sha256,
            },
            ResourceBackendIdentity::LocalInstallation {
                runtime_content_sha256: expected_content,
                ..
            },
        ) => {
            digest(evidence_sha256)?;
            if runtime_content_sha256 != expected_content
                || loader_policy_sha256 != &resource.profile.policy_sha256
                || processing_sha256 != &resource.processing_sha256
            {
                return Err(invalid("offline_coverage_binding_mismatch"));
            }
            Ok(())
        }
        (
            RuntimeCoverageReport::SystemApiBound {
                os_build,
                bridge_build_sha256,
                api_profile,
                strategy,
                observation_epoch,
            },
            ResourceBackendIdentity::AppleSystem {
                os_build: expected_build,
                bridge_build_sha256: expected_bridge,
                api_profile: expected_api,
                strategy: expected_strategy,
            },
        ) => {
            if os_build != expected_build
                || bridge_build_sha256 != expected_bridge
                || api_profile != expected_api
                || strategy != expected_strategy
                || observation_epoch != &resource.observation.epoch
            {
                return Err(invalid("offline_coverage_binding_mismatch"));
            }
            Ok(())
        }
        _ => Err(invalid("offline_coverage_backend_mismatch")),
    }
}
