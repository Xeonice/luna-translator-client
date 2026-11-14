//! Synthetic material declarations only: fixture dependency labels below do
//! not claim that any external build exists, is reviewed, or can be executed.
use app_core::domain::{
    content_hash,
    model_config::service_graph::{LanguagePair, OfflineBackend},
    offline_runtime::*,
};
use app_infra::offline_translation::{OfflineEngine, runtime_policy::*, windows};
use serde_json::json;
use std::num::NonZeroU32;
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn argos_version() -> String {
    serde_json::to_string(&json!({"argostranslate":"1.11.0","ctranslate2":"fixture-ct2","sentencepiece":"fixture-sp","stanza":"fixture-stanza","minisbd":"fixture-minisbd","packaging":"fixture-packaging","python":"fixture-python","processing":{"pipeline":"argos-1.11.0-stanza-local-v1","chunkType":"STANZA","device":"cpu","computeType":"auto","interThreads":1,"intraThreads":0,"effectiveComputeTypes":["fixture-compute"]}})).unwrap()
}
fn bundle(policy: &CompiledRuntimePolicy, version: &str) -> RuntimeBundleManifest {
    let processing = policy.processing_for_declared_runtime(version).unwrap();
    let expected = ExpectedRuntimeObservation {
        engine: policy.profile().engine.clone(),
        runtime_version: version.into(),
        platform: policy.profile().expected_worker_platform(),
        architecture: policy.profile().architecture.clone(),
        resource_version: (policy.engine() == OfflineEngine::ArgosTranslate)
            .then(|| "1.11.0".into()),
        physical_cancel_supported: false,
        processing_sha256: processing.canonical_sha256().unwrap(),
    };
    let roles = if policy.engine() == OfflineEngine::ArgosTranslate {
        vec![
            RuntimeFileRole::Worker,
            RuntimeFileRole::Interpreter,
            RuntimeFileRole::StandardLibrary,
            RuntimeFileRole::PackageCode,
            RuntimeFileRole::NativeLibrary,
            RuntimeFileRole::LanguagePackage,
            RuntimeFileRole::Tokenizer,
            RuntimeFileRole::SentenceBoundary,
        ]
    } else {
        vec![
            RuntimeFileRole::Worker,
            RuntimeFileRole::Engine,
            RuntimeFileRole::Dictionary,
        ]
    };
    let payload_files = roles
        .into_iter()
        .enumerate()
        .map(|(n, role)| RuntimeFileClaim {
            relative_key: format!("file{n}"),
            byte_length: 1,
            sha256: content_hash(format!("file{n}")),
            role,
        })
        .collect::<Vec<_>>();
    let content_sha256 =
        runtime_content_sha256(policy.profile(), &processing, &expected, &payload_files).unwrap();
    RuntimeBundleManifest {
        schema: BundleSchema::V1,
        profile: policy.profile().clone(),
        processing,
        expected,
        payload_files,
        content_sha256,
    }
}
fn recalc(b: &mut RuntimeBundleManifest) {
    b.expected.processing_sha256 = b.processing.canonical_sha256().unwrap();
    b.content_sha256 =
        runtime_content_sha256(&b.profile, &b.processing, &b.expected, &b.payload_files).unwrap();
}
fn apple_reference() -> OfflineResourceReference {
    let p = registered_policy(
        OfflineEngine::AppleTranslation,
        RuntimePlatform::Macos,
        RuntimeArchitecture::Arm64,
    )
    .unwrap();
    let runtime = "macOS-fixture-os-build/Translation/lowLatency/26.4";
    let processing_sha256 = p
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
        processing_sha256: processing_sha256.clone(),
    };
    OfflineResourceReference {
        snapshot_id: "system-snapshot".into(),
        route_id: "system-route".into(),
        observation: ObservationRef {
            observation_id: "system-observation".into(),
            epoch: nz(1),
            subject_key: "system-subject".into(),
            head_revision: nz(1),
        },
        profile: p.profile().clone(),
        backend: ResourceBackendIdentity::AppleSystem {
            os_build: "fixture-os-build".into(),
            bridge_build_sha256: content_hash("fixture-bridge"),
            api_profile: APPLE_API_PROFILE.into(),
            strategy: AppleStrategy::TraditionalLowLatency,
        },
        resource_content_sha256: content_hash("system-metadata-not-package"),
        processing_sha256,
        expected_observation_sha256: expected.canonical_sha256().unwrap(),
        control: None,
        pair: LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        },
    }
}
#[test]
fn compiled_material_profiles_are_deterministic_and_keep_unknown_loader_admission() {
    for engine in [OfflineEngine::Jbeijing, OfflineEngine::KingsoftFastait] {
        let a =
            registered_policy(engine, RuntimePlatform::Windows, RuntimeArchitecture::X86).unwrap();
        let b =
            registered_policy(engine, RuntimePlatform::Windows, RuntimeArchitecture::X86).unwrap();
        let c = registered_policy(engine, RuntimePlatform::Crossover, RuntimeArchitecture::X86)
            .unwrap();
        assert_eq!(a.profile(), b.profile());
        assert_ne!(a.profile().policy_sha256, c.profile().policy_sha256);
        assert!(matches!(
            a.coverage(),
            RuntimeCoverageReport::Unknown { .. }
        ));
    }
    let p = registered_policy(
        OfflineEngine::ArgosTranslate,
        RuntimePlatform::Macos,
        RuntimeArchitecture::Arm64,
    )
    .unwrap();
    let b = bundle(&p, &argos_version());
    assert_eq!(p.reconstruct_expected(&b).unwrap(), b.expected);
    assert!(matches!(
        p.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
}
#[test]
fn caller_made_policy_id_hash_revision_contract_and_backend_never_register_themselves() {
    let original = registered_policy(
        OfflineEngine::Jbeijing,
        RuntimePlatform::Windows,
        RuntimeArchitecture::X86,
    )
    .unwrap();
    for mutation in 0..6 {
        let mut profile = original.profile().clone();
        match mutation {
            0 => profile.profile_id = "caller-profile".into(),
            1 => profile.policy_sha256 = content_hash("same-shape-unknown-policy"),
            2 => profile.revision = nz(2),
            3 => profile.engine.contract_revision = nz(2),
            4 => profile.private_protocol_revision = nz(1),
            _ => profile.backend = OfflineBackend::PlatformNative,
        };
        assert_eq!(
            policy_for_profile(&profile).unwrap_err().stage,
            "runtime_policy_unavailable"
        );
    }
}
#[test]
fn unsupported_platform_architecture_combinations_fail_closed() {
    for (e, p, a) in [
        (
            OfflineEngine::AppleTranslation,
            RuntimePlatform::Windows,
            RuntimeArchitecture::X64,
        ),
        (
            OfflineEngine::ArgosTranslate,
            RuntimePlatform::Crossover,
            RuntimeArchitecture::X86,
        ),
        (
            OfflineEngine::Jbeijing,
            RuntimePlatform::Macos,
            RuntimeArchitecture::Arm64,
        ),
        (
            OfflineEngine::KingsoftFastait,
            RuntimePlatform::Windows,
            RuntimeArchitecture::X64,
        ),
    ] {
        assert!(registered_policy(e, p, a).is_err());
    }
}
#[test]
fn argos_declaration_is_strict_complete_typed_and_canonical_not_ready_metadata() {
    let p = registered_policy(
        OfflineEngine::ArgosTranslate,
        RuntimePlatform::Macos,
        RuntimeArchitecture::Arm64,
    )
    .unwrap();
    let base = serde_json::from_str::<serde_json::Value>(&argos_version()).unwrap();
    for mutation in 0..10 {
        let mut v = base.clone();
        match mutation {
            0 => v["argostranslate"] = json!("other"),
            1 => v["processing"]["device"] = json!("cuda"),
            2 => v["processing"]["chunkType"] = json!("MINISBD"),
            3 => v["processing"]["effectiveComputeTypes"] = json!([]),
            4 => v["processing"]["intraThreads"] = json!(-1),
            5 => v["processing"]["interThreads"] = json!(true),
            6 => v["processing"]["extra"] = json!(true),
            7 => {
                v.as_object_mut().unwrap().remove("packaging");
            }
            8 => v["python"] = json!("bad\u{0}version"),
            _ => v["Complete"] = json!(true),
        };
        assert!(
            p.processing_for_declared_runtime(&serde_json::to_string(&v).unwrap())
                .is_err(),
            "mutation {mutation}"
        );
    }
    assert!(
        p.processing_for_declared_runtime(&(argos_version() + " "))
            .is_err()
    );
    let duplicated = argos_version().replacen("\"python\":", "\"python\":\"other\",\"python\":", 1);
    assert!(p.processing_for_declared_runtime(&duplicated).is_err());
}
#[test]
fn changed_processing_contract_even_with_recomputed_hash_does_not_match_compiled_rule() {
    let p = registered_policy(
        OfflineEngine::ArgosTranslate,
        RuntimePlatform::Windows,
        RuntimeArchitecture::X64,
    )
    .unwrap();
    for mutation in 0..4 {
        let mut b = bundle(&p, &argos_version());
        match mutation {
            0 => b.processing.contract_id = "caller-contract".into(),
            1 => b.processing.revision = nz(2),
            2 => b.processing.settings[0].value = ProcessingValue::Boolean { value: true },
            _ => b.processing.settings.push(ProcessingSetting {
                key: "caller-option".into(),
                value: ProcessingValue::Integer { value: 3 },
            }),
        };
        recalc(&mut b);
        assert_eq!(
            p.reconstruct_expected(&b).unwrap_err().stage,
            "runtime_processing_mismatch"
        );
    }
}
#[test]
fn vendor_expected_fields_and_declared_roles_are_checked_without_qualifying_the_dll() {
    for (engine, abi) in [
        (OfflineEngine::Jbeijing, windows::JBEIJING_ABI),
        (OfflineEngine::KingsoftFastait, windows::KINGSOFT_ABI),
    ] {
        let p = registered_policy(engine, RuntimePlatform::Crossover, RuntimeArchitecture::X86)
            .unwrap();
        let b = bundle(&p, abi);
        assert_eq!(
            p.reconstruct_expected(&b).unwrap().platform,
            RuntimePlatform::Windows
        );
        for mutation in 0..3 {
            let mut changed = b.clone();
            match mutation {
                0 => changed.expected.physical_cancel_supported = true,
                1 => changed.expected.resource_version = Some("fake-version".into()),
                _ => changed
                    .payload_files
                    .retain(|f| f.role != RuntimeFileRole::Dictionary),
            };
            recalc(&mut changed);
            assert!(p.reconstruct_expected(&changed).is_err());
        }
        assert!(
            p.processing_for_declared_runtime("same-name-other-ABI")
                .is_err()
        );
        assert!(matches!(
            p.coverage(),
            RuntimeCoverageReport::Unknown { .. }
        ));
    }
}
#[test]
fn apple_expected_is_reconstructed_only_from_system_metadata_and_does_not_mint_system_api_proof() {
    let reference = apple_reference();
    let policy = policy_for_profile(&reference.profile).unwrap();
    let expected = policy.reconstruct_apple_expected(&reference).unwrap();
    assert_eq!(
        expected.runtime_version,
        "macOS-fixture-os-build/Translation/lowLatency/26.4"
    );
    assert_eq!(expected.resource_version, None);
    assert!(reference.control.is_none());
    assert!(matches!(
        policy.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
    assert!(validate_required_coverage_binding(&policy.coverage(), &reference).is_err());
    let json = serde_json::to_string(&reference).unwrap();
    assert!(!json.contains("installationId"));
    assert!(!json.contains("runtimeContentSha256"));
}
#[test]
fn changed_apple_os_api_processing_and_expected_refs_fail_closed() {
    for mutation in 0..5 {
        let mut r = apple_reference();
        match mutation {
            0 => {
                if let ResourceBackendIdentity::AppleSystem { os_build, .. } = &mut r.backend {
                    *os_build = "changed-build".into();
                }
            }
            1 => {
                if let ResourceBackendIdentity::AppleSystem { api_profile, .. } = &mut r.backend {
                    *api_profile = "unknown-api".into();
                }
            }
            2 => r.processing_sha256 = content_hash("wrong-processing"),
            3 => r.expected_observation_sha256 = content_hash("wrong-expected"),
            _ => {
                if let ResourceBackendIdentity::AppleSystem { os_build, .. } = &mut r.backend {
                    *os_build = "unknown".into();
                }
            }
        };
        let policy = policy_for_profile(&r.profile).unwrap();
        assert!(policy.reconstruct_apple_expected(&r).is_err());
    }
}

#[test]
fn first_apple_expected_can_be_rebuilt_from_stored_columns_without_a_full_reference_or_fake_hashes()
{
    let policy = registered_policy(
        OfflineEngine::AppleTranslation,
        RuntimePlatform::Macos,
        RuntimeArchitecture::Arm64,
    )
    .unwrap();
    let expected = policy
        .expected_apple_system_declaration(
            "fixture-os-build",
            APPLE_API_PROFILE,
            &AppleStrategy::TraditionalLowLatency,
        )
        .unwrap();
    let mut reference = apple_reference();
    reference.processing_sha256 = expected.processing_sha256.clone();
    reference.expected_observation_sha256 = expected.canonical_sha256().unwrap();
    assert_eq!(
        policy.reconstruct_apple_expected(&reference).unwrap(),
        expected
    );
    assert!(matches!(
        policy.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
    for build in ["", "unknown", "bad\0build"] {
        assert!(
            policy
                .expected_apple_system_declaration(
                    build,
                    APPLE_API_PROFILE,
                    &AppleStrategy::TraditionalLowLatency
                )
                .is_err()
        );
    }
    let vendor = registered_policy(
        OfflineEngine::Jbeijing,
        RuntimePlatform::Windows,
        RuntimeArchitecture::X86,
    )
    .unwrap();
    assert!(
        vendor
            .expected_apple_system_declaration(
                "fixture",
                APPLE_API_PROFILE,
                &AppleStrategy::TraditionalLowLatency
            )
            .is_err()
    );
}
