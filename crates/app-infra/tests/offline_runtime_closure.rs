//! Real bounded temporary files; matching material remains Unknown, never a
//! real vendor/runtime/loader qualification or an execution grant.
use app_core::{
    domain::{
        content_hash,
        model_config::service_graph::{LanguagePair, LocalInstallationRef},
        offline_runtime::*,
    },
    ports::operation_budget::OperationBudget,
};
use app_infra::offline_translation::{
    OfflineEngine, OfflineErrorCode,
    closure::*,
    resolver::OfflineRuntimeMaterialResolver,
    resources::{InspectionLimits, ResourceInspector},
    runtime_policy::registered_policy,
    windows,
};
use std::{num::NonZeroU32, path::PathBuf, time::Duration};
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
struct Fixture {
    root: PathBuf,
    input: SealedRuntimeMaterialInput,
    bundle: RuntimeBundleManifest,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("luna-o2-material-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".luna")).unwrap();
        let root = root.canonicalize().unwrap();
        let policy = registered_policy(
            OfflineEngine::Jbeijing,
            RuntimePlatform::Crossover,
            RuntimeArchitecture::X86,
        )
        .unwrap();
        let processing = policy
            .processing_for_declared_runtime(windows::JBEIJING_ABI)
            .unwrap();
        let expected = ExpectedRuntimeObservation {
            engine: policy.profile().engine.clone(),
            runtime_version: windows::JBEIJING_ABI.into(),
            platform: RuntimePlatform::Windows,
            architecture: RuntimeArchitecture::X86,
            resource_version: None,
            physical_cancel_supported: false,
            processing_sha256: processing.canonical_sha256().unwrap(),
        };
        let payload_files = [
            ("worker.exe", RuntimeFileRole::Worker),
            ("engine.dll", RuntimeFileRole::Engine),
            ("dictionary.bin", RuntimeFileRole::Dictionary),
        ]
        .into_iter()
        .map(|(name, role)| {
            std::fs::write(root.join(name), name.as_bytes()).unwrap();
            RuntimeFileClaim {
                relative_key: name.into(),
                byte_length: name.len() as u64,
                sha256: content_hash(name),
                role,
            }
        })
        .collect::<Vec<_>>();
        let content_sha256 =
            runtime_content_sha256(policy.profile(), &processing, &expected, &payload_files)
                .unwrap();
        let bundle = RuntimeBundleManifest {
            schema: BundleSchema::V1,
            profile: policy.profile().clone(),
            processing,
            expected,
            content_sha256,
            payload_files,
        };
        let mut fixture = Self {
            root,
            input: SealedRuntimeMaterialInput {
                reference: reference(&bundle),
                files: vec![],
                manifest_sha256: String::new(),
            },
            bundle,
        };
        fixture.seal();
        fixture
    }
    fn seal(&mut self) {
        self.bundle.expected.processing_sha256 = self.bundle.processing.canonical_sha256().unwrap();
        self.bundle.content_sha256 = runtime_content_sha256(
            &self.bundle.profile,
            &self.bundle.processing,
            &self.bundle.expected,
            &self.bundle.payload_files,
        )
        .unwrap();
        let bytes = encode_bundle_metadata(&self.bundle).unwrap();
        std::fs::write(self.root.join(CONTROL_RELATIVE_KEY), &bytes).unwrap();
        let control = ControlArtifactRef {
            relative_key: CONTROL_RELATIVE_KEY.into(),
            byte_length: bytes.len() as u64,
            sha256: content_hash(&bytes),
        };
        self.input.reference = reference(&self.bundle);
        self.input.reference.control = Some(control.clone());
        self.input.files = self
            .bundle
            .payload_files
            .iter()
            .map(|f| StoredRuntimeFileRow {
                relative_key: f.relative_key.clone(),
                byte_length: f.byte_length,
                sha256: f.sha256.clone(),
                role: f.role.stored_role(),
            })
            .collect();
        self.input.files.push(StoredRuntimeFileRow {
            relative_key: CONTROL_RELATIVE_KEY.into(),
            byte_length: control.byte_length,
            sha256: control.sha256.clone(),
            role: StoredFileRole::Runtime,
        });
        self.input.manifest_sha256 = snapshot_manifest_sha256(&self.bundle, &control).unwrap();
    }
    fn skeleton(&self) -> StoredRuntimeMaterialSkeleton {
        let reference = &self.input.reference;
        let ResourceBackendIdentity::LocalInstallation {
            installation,
            environment,
            root,
            ..
        } = &reference.backend
        else {
            panic!("fixture local branch");
        };
        StoredRuntimeMaterialSkeleton {
            snapshot_id: reference.snapshot_id.clone(),
            route_id: reference.route_id.clone(),
            observation: reference.observation.clone(),
            profile: reference.profile.clone(),
            installation: installation.clone(),
            environment: environment.clone(),
            root: root.clone(),
            resource_content_sha256: reference.resource_content_sha256.clone(),
            pair: reference.pair.clone(),
            route: vec![OfflineRouteHop {
                snapshot_id: reference.snapshot_id.clone(),
                pair: reference.pair.clone(),
                pipeline_profile_id: "fixture-vendor-ABI-pair".into(),
                package_relative_key: None,
            }],
            route_policy:
                app_core::domain::model_config::service_graph::OfflineRoutePolicy::DirectOnly {},
            files: self.input.files.clone(),
            manifest_sha256: self.input.manifest_sha256.clone(),
        }
    }
    async fn run(
        &self,
    ) -> app_infra::offline_translation::OfflineResult<
        app_infra::offline_translation::resolver::ResolvedOfflineMaterial,
    > {
        self.with_limits(InspectionLimits::default()).await
    }
    async fn with_limits(
        &self,
        limits: InspectionLimits,
    ) -> app_infra::offline_translation::OfflineResult<
        app_infra::offline_translation::resolver::ResolvedOfflineMaterial,
    > {
        OfflineRuntimeMaterialResolver::new(ResourceInspector::with_limits(limits).unwrap())
            .inspect_local(
                self.input.clone(),
                self.root.clone(),
                OperationBudget::new(Duration::from_secs(10)).unwrap(),
            )
            .await
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn reference(b: &RuntimeBundleManifest) -> OfflineResourceReference {
    OfflineResourceReference {
        snapshot_id: "snapshot".into(),
        route_id: "route".into(),
        observation: ObservationRef {
            observation_id: "observation".into(),
            epoch: nz(1),
            subject_key: "subject".into(),
            head_revision: nz(1),
        },
        profile: b.profile.clone(),
        backend: ResourceBackendIdentity::LocalInstallation {
            installation: LocalInstallationRef {
                installation_id: "installation".into(),
                revision: nz(1),
            },
            environment: RuntimeEnvironmentRef {
                environment_id: "environment".into(),
                revision: nz(1),
                machine_fingerprint: content_hash("machine"),
            },
            root: GrantedRootRef {
                grant_id: "grant".into(),
                authorization_epoch: nz(1),
                machine_fingerprint: content_hash("machine"),
            },
            runtime_content_sha256: b.content_sha256.clone(),
        },
        resource_content_sha256: content_hash("repository-overall-seal-not-O2-authority"),
        processing_sha256: b.expected.processing_sha256.clone(),
        expected_observation_sha256: b.expected.canonical_sha256().unwrap(),
        control: None,
        pair: LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        },
    }
}

#[tokio::test]
async fn reconstructs_real_control_files_and_complete_stored_set_but_never_loader_authority() {
    let f = Fixture::new();
    let result = f.run().await.unwrap();
    assert_eq!(result.expected(), &f.bundle.expected);
    assert_eq!(result.inspected_local().unwrap().files(), f.input.files);
    assert_eq!(
        result.inspected_local().unwrap().actual_total_bytes(),
        f.input.files.iter().map(|f| f.byte_length).sum::<u64>()
    );
    assert!(matches!(
        result.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
    assert!(validate_required_coverage_binding(result.coverage(), result.reference()).is_err());
}
#[tokio::test]
async fn rejects_same_version_changed_real_bytes_and_missing_or_undeclared_files() {
    for mutation in 0..3 {
        let f = Fixture::new();
        match mutation {
            0 => std::fs::write(f.root.join("engine.dll"), b"engine.zzz").unwrap(),
            1 => std::fs::remove_file(f.root.join("engine.dll")).unwrap(),
            _ => std::fs::write(f.root.join("hidden-native.dll"), b"unsealed").unwrap(),
        };
        assert_eq!(f.run().await.unwrap_err().stage, "actual_file_set_mismatch");
    }
}
#[tokio::test]
async fn rejects_changed_control_bytes_and_missing_control() {
    let f = Fixture::new();
    let mut bytes = std::fs::read(f.root.join(CONTROL_RELATIVE_KEY)).unwrap();
    bytes.push(b' ');
    std::fs::write(f.root.join(CONTROL_RELATIVE_KEY), &bytes).unwrap();
    assert_eq!(f.run().await.unwrap_err().stage, "sealed_control_bytes");
    std::fs::remove_file(f.root.join(CONTROL_RELATIVE_KEY)).unwrap();
    assert!(f.run().await.is_err());
}
#[tokio::test]
async fn rejects_duplicate_row_role_hash_length_and_set_mismatches() {
    for mutation in 0..6 {
        let mut f = Fixture::new();
        match mutation {
            0 => f.input.files.push(f.input.files[0].clone()),
            1 => f.input.files[0].role = StoredFileRole::Runtime,
            2 => f.input.files[0].sha256 = content_hash("wrong"),
            3 => f.input.files[0].byte_length += 1,
            4 => {
                f.input.files.pop();
            }
            _ => f.input.manifest_sha256 = content_hash("wrong-manifest"),
        };
        assert!(f.run().await.is_err(), "mutation {mutation}");
    }
}
#[tokio::test]
async fn control_row_is_required_and_must_have_runtime_role() {
    let mut f = Fixture::new();
    f.input.files.last_mut().unwrap().role = StoredFileRole::Engine;
    assert_eq!(
        f.run().await.unwrap_err().stage,
        "sealed_control_row_mismatch"
    );
}
#[tokio::test]
async fn giant_sparse_control_is_rejected_before_allocating_its_declared_length() {
    let f = Fixture::new();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(f.root.join(CONTROL_RELATIVE_KEY))
        .unwrap();
    file.set_len(MAX_CONTROL_BYTES as u64 + 1).unwrap();
    assert_eq!(f.run().await.unwrap_err().stage, "control_budget");
}
#[tokio::test]
async fn aggregate_byte_and_file_budgets_include_the_control_artifact() {
    let f = Fixture::new();
    let total = f.input.files.iter().map(|f| f.byte_length).sum::<u64>();
    let mut limits = InspectionLimits::default();
    limits.max_total_bytes = total - 1;
    assert_eq!(
        f.with_limits(limits).await.unwrap_err().code,
        OfflineErrorCode::ResourceBudgetExceeded
    );
    limits.max_total_bytes = total;
    assert!(f.with_limits(limits).await.is_ok());
    limits.max_files = f.input.files.len() - 1;
    assert_eq!(
        f.with_limits(limits).await.unwrap_err().stage,
        "resource_file_count"
    );
}
#[test]
fn sql_input_paths_use_exact_utf8_bytes_without_truncation() {
    let mut f = Fixture::new();
    let last = f.input.files[0].clone();
    for key in ["a".repeat(1024), "あ".repeat(341) + "a"] {
        let mut row = last.clone();
        row.relative_key = key;
        f.input.files[0] = row;
        f.input.validate_shape().unwrap();
    }
    for key in [
        "a".repeat(1025),
        "あ".repeat(342),
        "a/../b".into(),
        "a:b".into(),
    ] {
        f.input.files[0].relative_key = key;
        assert!(f.input.validate_shape().is_err());
    }
}
#[cfg(unix)]
#[tokio::test]
async fn nofollow_rejects_symlink_and_fifo_without_waiting_for_a_writer() {
    use std::os::unix::fs::symlink;
    for fifo in [false, true] {
        let f = Fixture::new();
        std::fs::remove_file(f.root.join("engine.dll")).unwrap();
        if fifo {
            let path = std::ffi::CString::new(f.root.join("engine.dll").to_str().unwrap()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        } else {
            symlink(f.root.join("worker.exe"), f.root.join("engine.dll")).unwrap();
        }
        let result = tokio::time::timeout(Duration::from_secs(2), f.run())
            .await
            .unwrap();
        assert!(result.is_err());
    }
}
#[tokio::test]
async fn actual_hardlink_alias_is_rejected_even_when_metadata_uses_distinct_keys() {
    let f = Fixture::new();
    std::fs::hard_link(f.root.join("engine.dll"), f.root.join("other-name.dll")).unwrap();
    assert_eq!(f.run().await.unwrap_err().stage, "resource_file_alias");
}
#[tokio::test]
async fn expected_reference_and_runtime_content_are_independently_bound() {
    for expected in [true, false] {
        let mut f = Fixture::new();
        if expected {
            f.input.reference.expected_observation_sha256 = content_hash("other expected");
        } else if let ResourceBackendIdentity::LocalInstallation {
            runtime_content_sha256,
            ..
        } = &mut f.input.reference.backend
        {
            *runtime_content_sha256 = content_hash("other runtime");
        }
        assert_eq!(
            f.run().await.unwrap_err().stage,
            "sealed_bundle_reference_mismatch"
        );
    }
}
#[tokio::test]
async fn revalidation_repeats_physical_reads_instead_of_reusing_a_success_flag() {
    let f = Fixture::new();
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    resolver
        .inspect_local(
            f.input.clone(),
            f.root.clone(),
            OperationBudget::new(Duration::from_secs(5)).unwrap(),
        )
        .await
        .unwrap();
    std::fs::write(f.root.join("dictionary.bin"), b"replacement").unwrap();
    assert!(
        resolver
            .revalidate_local(
                f.input.clone(),
                f.root.clone(),
                OperationBudget::new(Duration::from_secs(5)).unwrap()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn first_sql_skeleton_rebuilds_absent_hashes_only_from_real_control_and_roundtrips_through_revalidation()
 {
    let mut f = Fixture::new();
    let expected_reference = f.input.reference.clone();
    // These fields are absent in actual SQL. Poisoning the old full-reference
    // fixture cannot affect the skeleton or bootstrap's derived values.
    f.input.reference.processing_sha256 = content_hash("not a SQL column");
    f.input.reference.expected_observation_sha256 = content_hash("not a SQL column either");
    f.input.reference.control = None;
    if let ResourceBackendIdentity::LocalInstallation {
        runtime_content_sha256,
        ..
    } = &mut f.input.reference.backend
    {
        *runtime_content_sha256 = content_hash("also not a SQL column");
    }
    let stored = f.skeleton();
    let route = stored.route.clone();
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let resolved = resolver
        .inspect_stored(
            stored,
            f.root.clone(),
            OperationBudget::new(Duration::from_secs(5)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resolved.reference(), &expected_reference);
    assert_eq!(resolved.expected(), &f.bundle.expected);
    let material = resolved.inspected_local().unwrap();
    assert_eq!(material.route().unwrap(), route);
    let roundtrip = SealedRuntimeMaterialInput {
        reference: resolved.reference().clone(),
        files: material.files().to_vec(),
        manifest_sha256: material.manifest_sha256().into(),
    };
    assert!(
        resolver
            .revalidate_local(
                roundtrip.clone(),
                f.root.clone(),
                OperationBudget::new(Duration::from_secs(5)).unwrap()
            )
            .await
            .is_ok()
    );
    std::fs::write(f.root.join("engine.dll"), b"after-bootstrap mutation").unwrap();
    assert!(
        resolver
            .revalidate_local(
                roundtrip,
                f.root.clone(),
                OperationBudget::new(Duration::from_secs(5)).unwrap()
            )
            .await
            .is_err()
    );
}
#[tokio::test]
async fn first_read_rejects_mixed_control_root_and_uninspected_route_snapshots() {
    let f = Fixture::new();
    let mut other = Fixture::new();
    std::fs::write(other.root.join("engine.dll"), b"other runtime same ABI").unwrap();
    let changed = other
        .bundle
        .payload_files
        .iter_mut()
        .find(|file| file.relative_key == "engine.dll")
        .unwrap();
    changed.byte_length = 22;
    changed.sha256 = content_hash("other runtime same ABI");
    other.seal();
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    assert_eq!(
        resolver
            .inspect_stored(
                f.skeleton(),
                other.root.clone(),
                OperationBudget::new(Duration::from_secs(5)).unwrap()
            )
            .await
            .unwrap_err()
            .stage,
        "sealed_control_bytes"
    );
    let mut mixed = f.skeleton();
    mixed.route[0].snapshot_id = "another-unread-snapshot".into();
    assert_eq!(
        resolver
            .inspect_stored(
                mixed,
                f.root.clone(),
                OperationBudget::new(Duration::from_secs(5)).unwrap()
            )
            .await
            .unwrap_err()
            .stage,
        "route_snapshot_material_unavailable"
    );
}
#[test]
fn stored_file_count_limit_includes_control_without_serializing_a_giant_manifest_frame() {
    let mut f = Fixture::new();
    while f.input.files.len() < MAX_SNAPSHOT_FILES {
        let n = f.input.files.len();
        f.input.files.push(StoredRuntimeFileRow {
            relative_key: format!("extra/{n}"),
            byte_length: 0,
            sha256: content_hash(n.to_string()),
            role: StoredFileRole::Runtime,
        });
    }
    f.input.validate_shape().unwrap();
    f.input.files.push(StoredRuntimeFileRow {
        relative_key: "overflow".into(),
        byte_length: 0,
        sha256: content_hash("overflow"),
        role: StoredFileRole::Runtime,
    });
    assert_eq!(
        f.input.validate_shape().unwrap_err().stage,
        "sealed_material_shape"
    );
}

#[tokio::test]
async fn identical_content_read_from_a_different_root_retains_actual_location_for_the_original_owner()
 {
    let f = Fixture::new();
    let other = Fixture::new();
    let resolver = OfflineRuntimeMaterialResolver::new(ResourceInspector::new());
    let result = resolver
        .inspect_stored(
            f.skeleton(),
            other.root.clone(),
            OperationBudget::new(Duration::from_secs(5)).unwrap(),
        )
        .await
        .unwrap();
    let material = result.inspected_local().unwrap();
    assert_eq!(material.canonical_root_for_owner_check(), other.root);
    assert_ne!(material.canonical_root_for_owner_check(), f.root);
    assert_eq!(result.reference(), &f.input.reference);
    assert!(matches!(
        result.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
    assert!(!format!("{material:?}").contains(other.root.to_str().unwrap()));
    // The concrete O3/Owner must reject this for checkpoint f; the resolver's
    // material facts and matching digests cannot perform that authority check.
}
