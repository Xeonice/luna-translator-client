//! Resolver lifetime and API checks. No fake repository, loader, grant or
//! installed-system observation is used to turn these facts into authority.
use app_core::{
    domain::{content_hash, model_config::service_graph::LanguagePair, offline_runtime::*},
    ports::operation_budget::OperationBudget,
};
use app_infra::offline_translation::{
    OfflineEngine, OfflineErrorCode,
    closure::SealedRuntimeMaterialInput,
    resolver::OfflineRuntimeMaterialResolver,
    resources::ResourceInspector,
    runtime_policy::{APPLE_API_PROFILE, registered_policy},
};
use std::{num::NonZeroU32, path::PathBuf, time::Duration};
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn apple() -> OfflineResourceReference {
    let p = registered_policy(
        OfflineEngine::AppleTranslation,
        RuntimePlatform::Macos,
        RuntimeArchitecture::Arm64,
    )
    .unwrap();
    let runtime = "macOS-fixture/Translation/lowLatency/26.4";
    let processing = p
        .processing_for_declared_runtime(runtime)
        .unwrap()
        .canonical_sha256()
        .unwrap();
    let expected = ExpectedRuntimeObservation {
        engine: p.profile().engine.clone(),
        runtime_version: runtime.into(),
        platform: RuntimePlatform::Macos,
        architecture: RuntimeArchitecture::Arm64,
        resource_version: None,
        physical_cancel_supported: false,
        processing_sha256: processing.clone(),
    };
    OfflineResourceReference {
        snapshot_id: "snapshot".into(),
        route_id: "route".into(),
        observation: ObservationRef {
            observation_id: "observation".into(),
            epoch: nz(1),
            subject_key: "subject".into(),
            head_revision: nz(1),
        },
        profile: p.profile().clone(),
        backend: ResourceBackendIdentity::AppleSystem {
            os_build: "fixture".into(),
            bridge_build_sha256: content_hash("fixture bridge"),
            api_profile: APPLE_API_PROFILE.into(),
            strategy: AppleStrategy::TraditionalLowLatency,
        },
        resource_content_sha256: content_hash("metadata not language package"),
        processing_sha256: processing,
        expected_observation_sha256: expected.canonical_sha256().unwrap(),
        control: None,
        pair: LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        },
    }
}
#[tokio::test]
async fn original_cancelled_budget_wins_before_policy_or_material_io() {
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let budget = OperationBudget::new(Duration::from_secs(5)).unwrap();
    budget.cancel();
    let mut reference = apple();
    reference.profile.policy_sha256 = content_hash("unknown policy");
    let input = SealedRuntimeMaterialInput {
        reference,
        files: vec![],
        manifest_sha256: content_hash("none"),
    };
    assert_eq!(
        resolver
            .inspect_local(input, PathBuf::from("/path-that-must-never-open"), budget)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::Cancelled
    );
}
#[tokio::test]
async fn original_deadline_is_not_restarted_for_material_reconstruction() {
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let budget = OperationBudget::new(Duration::ZERO).unwrap();
    let input = SealedRuntimeMaterialInput {
        reference: apple(),
        files: vec![],
        manifest_sha256: content_hash("none"),
    };
    assert_eq!(
        resolver
            .inspect_local(input, PathBuf::from("/never-open"), budget)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::DeadlineExceeded
    );
}
#[test]
fn committed_budget_does_not_gain_a_new_resource_preparation_lifetime() {
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let budget = OperationBudget::new(Duration::from_secs(5)).unwrap();
    budget.with_terminal_fence(|| Ok(())).unwrap();
    assert_eq!(
        resolver
            .reconstruct_apple(apple(), &budget)
            .unwrap_err()
            .code,
        OfflineErrorCode::Cancelled
    );
}
#[test]
fn apple_material_reconstruction_stays_unknown_and_has_no_local_installation() {
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let reference = apple();
    let result = resolver
        .reconstruct_apple(
            reference.clone(),
            &OperationBudget::new(Duration::from_secs(5)).unwrap(),
        )
        .unwrap();
    assert_eq!(result.reference(), &reference);
    assert!(result.inspected_local().is_none());
    assert!(matches!(
        result.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
    assert!(validate_required_coverage_binding(result.coverage(), result.reference()).is_err());
}
#[test]
fn changing_apple_bridge_is_a_new_reference_not_a_live_api_proof() {
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let mut r = apple();
    if let ResourceBackendIdentity::AppleSystem {
        bridge_build_sha256,
        ..
    } = &mut r.backend
    {
        *bridge_build_sha256 = content_hash("other bridge declaration");
    }
    let result = resolver
        .reconstruct_apple(r, &OperationBudget::new(Duration::from_secs(5)).unwrap())
        .unwrap();
    assert!(matches!(
        result.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
}
