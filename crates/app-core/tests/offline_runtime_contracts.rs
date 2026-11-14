//! O1 pure contracts only. Synthetic facts never qualify an installed runtime,
//! authorize a real grant, load an engine, or claim a product test passed.
use app_core::{
    contracts::{CatalogPurpose, ServiceTestInputScope},
    domain::{
        content_hash, model_config::service_graph::*, offline_runtime::*,
        service_execution::ServicePlanIdentity,
    },
    ports::offline_resources::*,
};
use serde_json::{Value, json};
use std::num::NonZeroU32;
fn nz(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v).unwrap()
}
fn sha(s: &str) -> String {
    content_hash(s)
}
fn profile() -> RuntimeProfileRef {
    RuntimeProfileRef {
        profile_id: "controlled-profile".into(),
        revision: nz(1),
        engine: OfflineEngineKey {
            engine_id: "argos_translate".into(),
            contract_revision: nz(1),
        },
        backend: OfflineBackend::LocalWorker,
        platform: RuntimePlatform::Macos,
        architecture: RuntimeArchitecture::Arm64,
        private_protocol_revision: nz(2),
        policy_sha256: sha("reviewed-policy-reference-only"),
    }
}
fn processing() -> RuntimeProcessingContract {
    RuntimeProcessingContract {
        contract_id: "cpu-contract".into(),
        revision: nz(1),
        settings: vec![
            ProcessingSetting {
                key: "threads".into(),
                value: ProcessingValue::Integer { value: 2 },
            },
            ProcessingSetting {
                key: "compute".into(),
                value: ProcessingValue::Text {
                    value: "int8".into(),
                },
            },
        ],
    }
}
fn file(key: &str, role: RuntimeFileRole) -> RuntimeFileClaim {
    RuntimeFileClaim {
        relative_key: key.into(),
        byte_length: 7,
        sha256: sha(key),
        role,
    }
}
fn bundle() -> RuntimeBundleManifest {
    let profile = profile();
    let mut processing = processing();
    processing
        .settings
        .sort_by(|a, b| a.key.as_bytes().cmp(b.key.as_bytes()));
    let expected = ExpectedRuntimeObservation {
        engine: profile.engine.clone(),
        runtime_version: "controlled-expected-version".into(),
        platform: RuntimePlatform::Macos,
        architecture: RuntimeArchitecture::Arm64,
        resource_version: Some("1.11.0".into()),
        physical_cancel_supported: false,
        processing_sha256: processing.canonical_sha256().unwrap(),
    };
    let mut payload_files = vec![
        file("bin/python", RuntimeFileRole::Interpreter),
        file("stdlib/site.py", RuntimeFileRole::StandardLibrary),
        file("packages/translate.py", RuntimeFileRole::PackageCode),
    ];
    payload_files.sort_by(|a, b| a.relative_key.as_bytes().cmp(b.relative_key.as_bytes()));
    let content_sha256 =
        runtime_content_sha256(&profile, &processing, &expected, &payload_files).unwrap();
    RuntimeBundleManifest {
        schema: BundleSchema::V1,
        profile,
        processing,
        expected,
        payload_files,
        content_sha256,
    }
}
fn control(b: &RuntimeBundleManifest) -> ControlArtifactRef {
    let bytes = encode_bundle_metadata(b).unwrap();
    ControlArtifactRef {
        relative_key: CONTROL_RELATIVE_KEY.into(),
        byte_length: bytes.len() as u64,
        sha256: content_hash(bytes),
    }
}
fn resource() -> OfflineResourceReference {
    let b = bundle();
    OfflineResourceReference {
        snapshot_id: "snapshot".into(),
        route_id: "direct-route".into(),
        observation: ObservationRef {
            observation_id: "observation-1".into(),
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
                machine_fingerprint: sha("machine"),
            },
            root: GrantedRootRef {
                grant_id: "grant".into(),
                authorization_epoch: nz(1),
                machine_fingerprint: sha("machine"),
            },
            runtime_content_sha256: b.content_sha256.clone(),
        },
        resource_content_sha256: sha("snapshot-files"),
        processing_sha256: b.expected.processing_sha256.clone(),
        expected_observation_sha256: b.expected.canonical_sha256().unwrap(),
        control: Some(control(&b)),
        pair: LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        },
    }
}
fn task_identity() -> ServicePlanIdentity {
    ServicePlanIdentity {
        configuration_revision: "saved-source".into(),
        binding_id: "binding".into(),
        engine_id: "engine".into(),
        purpose: CatalogPurpose::Translation,
        unit_id: "real-unit-reference".into(),
        text_revision: 2,
        source_hash: sha("日本語"),
        session_id: "session".into(),
        game_id: Some("game".into()),
    }
}
fn frame() -> OfflineWorkerReferenceV2 {
    OfflineWorkerReferenceV2 {
        protocol: WorkerProtocolV2::V2,
        worker_instance_id: "worker".into(),
        profile_generation: nz(1),
        server_instance_id: "server".into(),
        source: OfflineConfigRef {
            revision_id: "saved-source".into(),
            config_id: "offline-config".into(),
        },
        operation: OfflineOperationTarget::Attempt {
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            identity: task_identity(),
        },
        resource: resource(),
    }
}
fn apple() -> OfflineResourceReference {
    let mut r = resource();
    r.profile.engine.engine_id = "apple_translation".into();
    r.profile.backend = OfflineBackend::PlatformNative;
    r.backend = ResourceBackendIdentity::AppleSystem {
        os_build: "controlled-os-build".into(),
        bridge_build_sha256: sha("app-bridge"),
        api_profile: "traditional-installed-source-v1".into(),
        strategy: AppleStrategy::TraditionalLowLatency,
    };
    r.resource_content_sha256 = sha("system-metadata-not-package-bytes");
    r.control = None;
    r
}
fn recalculate(b: &mut RuntimeBundleManifest) {
    b.expected.engine = b.profile.engine.clone();
    b.expected.processing_sha256 = b.processing.canonical_sha256().unwrap();
    b.content_sha256 =
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &b.payload_files).unwrap();
}
fn hash_changed(change: impl FnOnce(&mut RuntimeBundleManifest)) {
    let before = bundle();
    let mut after = before.clone();
    change(&mut after);
    recalculate(&mut after);
    assert_ne!(before.content_sha256, after.content_sha256);
    after.validate().unwrap();
}
fn wire(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn canonical_content_and_encoded_metadata_ignore_only_order_not_semantic_values() {
    let a = bundle();
    let mut b = a.clone();
    b.payload_files.reverse();
    b.processing.settings.reverse();
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &b.payload_files).unwrap(),
        a.content_sha256
    );
    assert_eq!(
        encode_bundle_metadata(&a).unwrap(),
        encode_bundle_metadata(&b).unwrap()
    );
    assert_eq!(
        decode_bundle_metadata(&encode_bundle_metadata(&a).unwrap()).unwrap(),
        a
    );
    hash_changed(|b| b.profile.policy_sha256 = sha("different-policy"));
    hash_changed(|b| b.profile.revision = nz(2));
    hash_changed(|b| b.profile.engine.contract_revision = nz(2));
    hash_changed(|b| b.processing.revision = nz(2));
    hash_changed(|b| {
        b.processing
            .settings
            .iter_mut()
            .find(|s| s.key == "threads")
            .unwrap()
            .value = ProcessingValue::Integer { value: 3 }
    });
    hash_changed(|b| b.expected.runtime_version = "same-name-changed-build".into());
    hash_changed(|b| b.expected.resource_version = None);
    hash_changed(|b| b.expected.physical_cancel_supported = true);
    hash_changed(|b| b.payload_files[0].sha256 = sha("same-version-replaced-file"));
    hash_changed(|b| b.payload_files[0].byte_length += 1);
    hash_changed(|b| b.payload_files[1].relative_key = "stdlib/other.py".into());
    hash_changed(|b| {
        b.payload_files
            .iter_mut()
            .find(|file| file.role == RuntimeFileRole::StandardLibrary)
            .unwrap()
            .role = RuntimeFileRole::PackageCode
    });
}

#[test]
fn snapshot_hash_includes_the_control_file_while_runtime_content_excludes_its_self_hash() {
    let b = bundle();
    let c = control(&b);
    let h = snapshot_manifest_sha256(&b, &c).unwrap();
    let mut changed = c.clone();
    changed.sha256 = sha("different-real-control-bytes");
    assert_ne!(snapshot_manifest_sha256(&b, &changed).unwrap(), h);
    changed = c.clone();
    changed.byte_length += 1;
    assert_ne!(snapshot_manifest_sha256(&b, &changed).unwrap(), h);
    assert_eq!(bundle().content_sha256, b.content_sha256);
    assert_ne!(h, b.content_sha256);
    let bytes = encode_bundle_metadata(&b).unwrap();
    assert_eq!(decode_control_artifact(&bytes, &c).unwrap(), b);
    assert_eq!(
        decode_control_artifact(&bytes, &changed).unwrap_err().code,
        "offline_control_bytes_mismatch"
    );
    let mut corrupted = bytes.clone();
    corrupted.push(b' ');
    assert!(decode_control_artifact(&corrupted, &c).is_err());
    let mut self_referencing = b.clone();
    self_referencing
        .payload_files
        .push(file(CONTROL_RELATIVE_KEY, RuntimeFileRole::RuntimeMetadata));
    assert_eq!(
        runtime_content_sha256(
            &self_referencing.profile,
            &self_referencing.processing,
            &self_referencing.expected,
            &self_referencing.payload_files
        )
        .unwrap_err()
        .code,
        "offline_control_self_reference"
    );
}

#[test]
fn resource_epochs_and_locators_cannot_enter_the_stable_metadata_domain() {
    let b = bundle();
    let a = resource();
    let mut moved = a.clone();
    moved.snapshot_id = "other-snapshot".into();
    moved.observation.observation_id = "later-observation".into();
    moved.observation.epoch = nz(2);
    moved.observation.head_revision = nz(2);
    if let ResourceBackendIdentity::LocalInstallation {
        installation, root, ..
    } = &mut moved.backend
    {
        installation.revision = nz(2);
        root.grant_id = "new-explicit-root".into();
        root.authorization_epoch = nz(7)
    }
    moved.validate().unwrap();
    assert_ne!(a, moved);
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &b.payload_files).unwrap(),
        b.content_sha256
    );
    for key in [
        "root",
        "grantId",
        "authorizationEpoch",
        "installationId",
        "observationId",
        "taskId",
        "attemptId",
        "complete",
    ] {
        let mut raw = serde_json::to_value(&b).unwrap();
        raw[key] = json!("caller-claim");
        assert!(
            decode_bundle_metadata(&serde_json::to_vec(&raw).unwrap()).is_err(),
            "unexpected metadata field {key}"
        );
    }
}

#[test]
fn metadata_has_strict_nested_types_unknown_fields_and_duplicate_fields() {
    let b = bundle();
    let original = serde_json::to_value(&b).unwrap();
    for (path, value) in [
        ("schema", json!("unknown-schema")),
        ("contentSha256", json!("A".repeat(64))),
    ] {
        let mut data = original.clone();
        data[path] = value;
        assert!(decode_bundle_metadata(&serde_json::to_vec(&data).unwrap()).is_err())
    }
    for mutate in [0, 1, 2, 3, 4, 5, 6, 7] {
        let mut data = original.clone();
        match mutate {
            0 => data["profile"]["revision"] = json!(0),
            1 => data["profile"]["privateProtocolRevision"] = json!(1),
            2 => data["payloadFiles"][0]["byteLength"] = json!(-1),
            3 => data["payloadFiles"][0]["byteLength"] = json!(1.5),
            4 => data["payloadFiles"][0]["role"] = json!("runtime"),
            5 => data["processing"]["settings"][0]["value"] = json!({"kind":"integer","value":2.1}),
            6 => data["expected"]["Complete"] = json!(true),
            _ => data["processing"]["settings"][0]["extra"] = json!(true),
        }
        assert!(
            decode_bundle_metadata(&serde_json::to_vec(&data).unwrap()).is_err(),
            "mutation {mutate}"
        )
    }
    let json = String::from_utf8(encode_bundle_metadata(&b).unwrap()).unwrap();
    for duplicated in [
        json.replacen(
            "\"schema\":",
            "\"schema\":\"offline-runtime-bundle-v1\",\"schema\":",
            1,
        ),
        json.replacen(
            "\"profileId\":",
            "\"profileId\":\"duplicate\",\"profileId\":",
            1,
        ),
        json.replacen("\"byteLength\":", "\"byteLength\":0,\"byteLength\":", 1),
    ] {
        assert!(decode_bundle_metadata(duplicated.as_bytes()).is_err())
    }
}

#[test]
fn duplicate_file_and_singleton_roles_are_rejected_but_repeated_package_roles_are_valid() {
    let b = bundle();
    let mut files = b.payload_files.clone();
    files.push(files[0].clone());
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files)
            .unwrap_err()
            .code,
        "offline_duplicate_file_key"
    );
    files = b.payload_files.clone();
    files.push(file("bin/python-other", RuntimeFileRole::Interpreter));
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files)
            .unwrap_err()
            .code,
        "offline_duplicate_singleton_role"
    );
    files = b.payload_files.clone();
    files.push(file("packages/second.py", RuntimeFileRole::PackageCode));
    assert!(runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files).is_ok());
    let mut p = b.processing.clone();
    p.settings.push(p.settings[0].clone());
    assert_eq!(
        p.canonical_sha256().unwrap_err().code,
        "offline_duplicate_processing_key"
    );
    for role in [
        RuntimeFileRole::Interpreter,
        RuntimeFileRole::StandardLibrary,
        RuntimeFileRole::PackageCode,
        RuntimeFileRole::NativeLibrary,
        RuntimeFileRole::RuntimeMetadata,
    ] {
        assert_eq!(role.stored_role(), StoredFileRole::Runtime)
    }
    assert_eq!(
        RuntimeFileRole::Worker.stored_role(),
        StoredFileRole::Worker
    );
}

#[test]
fn relative_key_uses_exact_utf8_storage_bytes_and_component_rules() {
    for valid in ["a..b", "a/.../b", "日本語/辞書.bin", ".hidden/file"] {
        validate_relative_key(valid).unwrap()
    }
    validate_relative_key(&"a".repeat(1024)).unwrap();
    validate_relative_key(&"あ".repeat(341)).unwrap();
    for bad in [
        "",
        "/absolute",
        "a//b",
        "a/./b",
        "a/../b",
        ".",
        "..",
        "a/",
        "C:relative",
        "a\\b",
        "a\0b",
    ] {
        assert!(validate_relative_key(bad).is_err(), "bad path {bad:?}")
    }
    assert!(validate_relative_key(&"a".repeat(1025)).is_err());
    assert!(validate_relative_key(&"あ".repeat(342)).is_err());
}

#[test]
fn snapshot_file_count_includes_control_and_sqlite_lengths_never_overflow() {
    let b = bundle();
    let files = (0..MAX_SNAPSHOT_FILES - 1)
        .map(|n| file(&format!("packages/{n}.py"), RuntimeFileRole::PackageCode))
        .collect::<Vec<_>>();
    assert!(runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files).is_ok());
    let mut excessive = files;
    excessive.push(file("packages/extra.py", RuntimeFileRole::PackageCode));
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &excessive)
            .unwrap_err()
            .code,
        "offline_file_count_exceeded"
    );
    let mut files = vec![file("one", RuntimeFileRole::PackageCode)];
    files[0].byte_length = i64::MAX as u64;
    assert!(runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files).is_ok());
    files.push(file("two", RuntimeFileRole::PackageCode));
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files)
            .unwrap_err()
            .code,
        "offline_file_length_overflow"
    );
    files.truncate(1);
    files[0].byte_length = u64::MAX;
    assert!(runtime_content_sha256(&b.profile, &b.processing, &b.expected, &files).is_err());
}

#[test]
fn control_artifact_budget_is_separate_from_wire_and_rejects_before_json_parsing() {
    let mut c = control(&bundle());
    c.byte_length = MAX_CONTROL_BYTES as u64;
    c.validate().unwrap();
    c.byte_length += 1;
    assert!(c.validate().is_err());
    c.byte_length = 0;
    assert!(c.validate().is_err());
    c.byte_length = 1;
    c.relative_key = "other.json".into();
    assert!(c.validate().is_err());
    let oversized = vec![b' '; MAX_CONTROL_BYTES + 1];
    assert_eq!(
        decode_bundle_metadata(&oversized).unwrap_err().code,
        "offline_control_too_large"
    );
}

fn hop(a: &str, b: &str) -> OfflineRouteHop {
    OfflineRouteHop {
        snapshot_id: format!("snapshot-{a}-{b}"),
        pair: LanguagePair {
            source: a.into(),
            target: b.into(),
        },
        pipeline_profile_id: "pipeline".into(),
        package_relative_key: Some(format!("packages/{a}-{b}")),
    }
}
#[test]
fn stored_routes_allow_one_direct_or_explicit_two_to_three_hops_without_truncation() {
    let direct = vec![hop("ja", "zh-Hans")];
    let pair = LanguagePair {
        source: "ja".into(),
        target: "zh-Hans".into(),
    };
    validate_route(&direct, &pair, &OfflineRoutePolicy::DirectOnly {}).unwrap();
    assert!(
        validate_route(
            &direct,
            &pair,
            &OfflineRoutePolicy::ExplicitPivot {
                consent_revision: nz(1)
            }
        )
        .is_err()
    );
    let triple = vec![hop("ja", "en"), hop("en", "de"), hop("de", "zh-Hans")];
    validate_route(
        &triple,
        &pair,
        &OfflineRoutePolicy::ExplicitPivot {
            consent_revision: nz(1),
        },
    )
    .unwrap();
    assert!(validate_route(&triple, &pair, &OfflineRoutePolicy::DirectOnly {}).is_err());
    let four = vec![
        hop("ja", "en"),
        hop("en", "de"),
        hop("de", "fr"),
        hop("fr", "zh-Hans"),
    ];
    assert_eq!(
        validate_route(
            &four,
            &pair,
            &OfflineRoutePolicy::ExplicitPivot {
                consent_revision: nz(1)
            }
        )
        .unwrap_err()
        .code,
        "offline_invalid_route_length"
    );
    let wrong = vec![hop("ja", "en"), hop("de", "zh-Hans")];
    assert!(
        validate_route(
            &wrong,
            &pair,
            &OfflineRoutePolicy::ExplicitPivot {
                consent_revision: nz(1)
            }
        )
        .is_err()
    );
    let mut too_long = direct;
    too_long[0].pipeline_profile_id = "x".repeat(161);
    assert!(validate_route(&too_long, &pair, &OfflineRoutePolicy::DirectOnly {}).is_err());
}

#[test]
fn local_backend_requires_one_original_root_machine_and_control_reference() {
    let good = resource();
    good.validate().unwrap();
    let mut bad = good.clone();
    bad.control = None;
    assert_eq!(
        bad.validate().unwrap_err().code,
        "offline_control_ref_missing"
    );
    let mut bad = good.clone();
    if let ResourceBackendIdentity::LocalInstallation { root, .. } = &mut bad.backend {
        root.machine_fingerprint = sha("other-machine")
    }
    assert_eq!(
        bad.validate().unwrap_err().code,
        "offline_backend_identity_mismatch"
    );
    let mut bad = good.clone();
    bad.profile.backend = OfflineBackend::PlatformNative;
    assert!(bad.validate().is_err());
    for field in ["rootPath", "secondRoot", "legacyReady"] {
        let mut raw = serde_json::to_value(&good).unwrap();
        raw["backend"][field] = json!("/untrusted");
        assert!(serde_json::from_value::<OfflineResourceReference>(raw).is_err())
    }
}

#[test]
fn crossover_environment_and_windows_worker_are_distinct_facts() {
    let mut b = bundle();
    b.profile.platform = RuntimePlatform::Crossover;
    b.profile.architecture = RuntimeArchitecture::X86;
    b.expected.platform = RuntimePlatform::Windows;
    b.expected.architecture = RuntimeArchitecture::X86;
    recalculate(&mut b);
    b.validate().unwrap();
    assert_eq!(
        b.profile.expected_worker_platform(),
        RuntimePlatform::Windows
    );
    b.expected.platform = RuntimePlatform::Crossover;
    assert_eq!(
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &b.payload_files)
            .unwrap_err()
            .code,
        "offline_runtime_metadata_mismatch"
    );
}

#[test]
fn apple_system_reference_has_no_fake_installation_dll_or_package_content_hash() {
    let a = apple();
    a.validate().unwrap();
    let raw = serde_json::to_value(&a).unwrap();
    let fields = raw["backend"].as_object().unwrap();
    for absent in [
        "installation",
        "root",
        "runtimeContentSha256",
        "dll",
        "modelId",
    ] {
        assert!(!fields.contains_key(absent))
    }
    assert!(a.control.is_none());
    let mut bad = a.clone();
    bad.control = Some(control(&bundle()));
    assert!(bad.validate().is_err());
    let mut bad = a.clone();
    bad.profile.platform = RuntimePlatform::Windows;
    assert!(bad.validate().is_err());
    let mut bad = raw;
    bad["backend"]["strategy"] = json!("apple_intelligence");
    assert!(serde_json::from_value::<OfflineResourceReference>(bad).is_err());
}

#[test]
fn unknown_and_unsupported_coverage_never_pass_a_required_evidence_check() {
    let r = resource();
    for (coverage, code) in [
        (
            RuntimeCoverageReport::Unknown {
                reason_code: "loader_not_proven".into(),
            },
            "offline_coverage_unknown",
        ),
        (
            RuntimeCoverageReport::Unsupported {
                reason_code: "unavailable_loader_policy".into(),
            },
            "offline_coverage_unsupported",
        ),
    ] {
        assert_eq!(
            validate_required_coverage_binding(&coverage, &r)
                .unwrap_err()
                .code,
            code
        )
    }
    let b = bundle();
    let positive = RuntimeCoverageReport::ActualReviewedFileClosure {
        runtime_content_sha256: b.content_sha256,
        loader_policy_sha256: r.profile.policy_sha256.clone(),
        processing_sha256: r.processing_sha256.clone(),
        evidence_sha256: sha("synthetic-binding-test-not-real-loader"),
    };
    validate_required_coverage_binding(&positive, &r).unwrap();
    let mut changed = r.clone();
    changed.profile.policy_sha256 = sha("other-policy");
    assert!(validate_required_coverage_binding(&positive, &changed).is_err());
    assert!(validate_required_coverage_binding(&positive, &apple()).is_err());
    let mut raw = serde_json::to_value(bundle()).unwrap();
    raw["coverage"] = json!({"kind":"actual_reviewed_file_closure"});
    assert!(decode_bundle_metadata(&serde_json::to_vec(&raw).unwrap()).is_err());
}

#[test]
fn system_api_evidence_requires_its_original_observation_epoch_and_bridge() {
    let a = apple();
    let ResourceBackendIdentity::AppleSystem {
        os_build,
        bridge_build_sha256,
        api_profile,
        strategy,
    } = &a.backend
    else {
        panic!()
    };
    let proof = RuntimeCoverageReport::SystemApiBound {
        os_build: os_build.clone(),
        bridge_build_sha256: bridge_build_sha256.clone(),
        api_profile: api_profile.clone(),
        strategy: strategy.clone(),
        observation_epoch: a.observation.epoch,
    };
    validate_required_coverage_binding(&proof, &a).unwrap();
    let mut changed = a.clone();
    changed.observation.epoch = nz(2);
    assert_eq!(
        validate_required_coverage_binding(&proof, &changed)
            .unwrap_err()
            .code,
        "offline_coverage_binding_mismatch"
    );
    let mut changed = a.clone();
    if let ResourceBackendIdentity::AppleSystem {
        bridge_build_sha256,
        ..
    } = &mut changed.backend
    {
        *bridge_build_sha256 = sha("other-bridge")
    }
    assert!(validate_required_coverage_binding(&proof, &changed).is_err());
    assert!(validate_required_coverage_binding(&proof, &resource()).is_err());
}

#[test]
fn v2_keeps_true_attempt_and_explicit_test_identity_separate() {
    let f = frame();
    let bytes = encode_v2_reference(&f).unwrap();
    assert_eq!(decode_v2_reference(&bytes).unwrap(), f);
    let mut test = f;
    test.operation = OfflineOperationTarget::ExplicitTest {
        operation_id: "test-operation".into(),
        test_id: "test-receipt".into(),
        input_scope: ServiceTestInputScope::Sample {},
        input_sha256: sha("fixed-ja-sample"),
        request_semantics_sha256: sha("explicit-test-request"),
    };
    let bytes = encode_v2_reference(&test).unwrap();
    assert_eq!(decode_v2_reference(&bytes).unwrap(), test);
    let text = String::from_utf8(bytes).unwrap();
    for absent in [
        "taskId",
        "attemptId",
        "unitId",
        "sourceText",
        "payloadFiles",
    ] {
        assert!(
            !text.contains(absent),
            "explicit sample unexpectedly contained {absent}"
        )
    }
    test.operation = OfflineOperationTarget::ExplicitTest {
        operation_id: "test-operation".into(),
        test_id: "test-receipt".into(),
        input_scope: ServiceTestInputScope::SelectedUnit {
            unit_id: "selected-real-unit".into(),
            text_revision: nz(3),
            source_hash: sha("selected-source"),
        },
        input_sha256: sha("selected-input"),
        request_semantics_sha256: sha("selected-request"),
    };
    decode_v2_reference(&encode_v2_reference(&test).unwrap()).unwrap();
}

#[test]
fn v2_is_strict_even_for_the_existing_legacy_task_identity_value() {
    let original = serde_json::to_value(frame()).unwrap();
    for mutate in 0..7 {
        let mut raw = original.clone();
        match mutate {
            0 => raw["protocol"] = json!("translation-worker-v1"),
            1 => raw["operation"]["identity"]["endpoint"] = json!("https://example.invalid"),
            2 => raw["resource"]["control"] = Value::Null,
            3 => raw["operation"]["identity"]["purpose"] = json!("grammar"),
            4 => raw["operation"]["identity"]["textRevision"] = json!(0),
            5 => raw["profileGeneration"] = json!(0),
            _ => raw["payloadFiles"] = json!([]),
        }
        assert!(
            decode_v2_reference(&wire(&raw)).is_err(),
            "mutation {mutate}"
        );
        assert!(
            serde_json::from_value::<OfflineWorkerReferenceV2>(raw).is_err() || mutate >= 2,
            "nested unknown must also fail direct typed decode"
        )
    }
    let mut wrong = frame();
    wrong.source.revision_id = "different-source".into();
    assert_eq!(
        encode_v2_reference(&wrong).unwrap_err().code,
        "offline_operation_source_mismatch"
    );
    let good = encode_v2_reference(&frame()).unwrap();
    assert!(decode_v2_reference(&good[..good.len() - 1]).is_err());
    let mut two = good.clone();
    two.push(b'\n');
    assert!(decode_v2_reference(&two).is_err());
    let text = String::from_utf8(good).unwrap();
    let duplicate = text.replacen("\"taskId\":", "\"taskId\":\"duplicate\",\"taskId\":", 1);
    assert!(decode_v2_reference(duplicate.as_bytes()).is_err());
}

#[test]
fn v2_frame_budget_does_not_follow_the_large_control_artifact_budget() {
    let mut f = frame();
    f.worker_instance_id = "x".repeat(MAX_V2_FRAME_BYTES);
    assert_eq!(
        encode_v2_reference(&f).unwrap_err().code,
        "offline_v2_frame_too_large"
    );
    let mut exact = encode_v2_reference(&frame()).unwrap();
    exact.pop();
    exact.resize(MAX_V2_FRAME_BYTES - 1, b' ');
    exact.push(b'\n');
    decode_v2_reference(&exact).unwrap();
    exact.insert(exact.len() - 1, b' ');
    assert_eq!(
        decode_v2_reference(&exact).unwrap_err().code,
        "offline_invalid_v2_frame"
    );
}

#[test]
fn prepared_shape_validation_does_not_turn_nonserde_facts_or_matching_frames_into_authority() {
    let r = resource();
    let facts = PreparedOfflineResourceFacts {
        frame: OfflineProfileFrame {
            profile_id: "profile".into(),
            generation: nz(1),
            server_instance_id: "server".into(),
        },
        reference: r.clone(),
        expected_observation: bundle().expected,
        availability: ResourceAvailability::Installed,
        observed_state: OfflineObservationState::Installed,
        coverage: RuntimeCoverageReport::Unknown {
            reason_code: "loader_proof_missing".into(),
        },
        route: vec![OfflineRouteHop {
            snapshot_id: r.snapshot_id,
            pair: r.pair,
            pipeline_profile_id: "pipeline".into(),
            package_relative_key: Some("packages/direct".into()),
        }],
        route_policy: OfflineRoutePolicy::DirectOnly {},
        installation_head_revision: Some(nz(1)),
    };
    facts.validate_shape().unwrap();
    assert_eq!(
        facts.validate_required_evidence_shape().unwrap_err().code,
        "offline_coverage_unknown"
    );
    let same_scope = facts.frame.clone();
    assert_eq!(same_scope, facts.frame);
    let mut missing = facts.clone();
    missing.availability = ResourceAvailability::SupportedNotInstalled;
    assert_eq!(
        missing.validate_required_evidence_shape().unwrap_err().code,
        "offline_resources_not_installed"
    );
    let mut changed = facts;
    changed.expected_observation.runtime_version = "different-runtime-version".into();
    assert_eq!(
        changed.validate_shape().unwrap_err().code,
        "offline_observation_binding_mismatch"
    );
    assert_eq!(
        changed.validate_required_evidence_shape().unwrap_err().code,
        "offline_observation_binding_mismatch"
    );
}
