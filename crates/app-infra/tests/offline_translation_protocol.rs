use app_core::domain::{model_config::service_graph::*, service_execution::OfflineRequestPayload};
use app_infra::offline_translation::{protocol::*, resources::manifest_digest, *};
use sha2::{Digest, Sha256};
use std::{num::NonZeroU32, path::PathBuf, time::Duration};
fn request() -> OfflineExecution {
    let pair = LanguagePair {
        source: "ja".into(),
        target: "zh".into(),
    };
    let route = vec![RouteHop {
        pair: pair.clone(),
        package_relative_path: None,
    }];
    OfflineExecution {
        identity: ExecutionIdentity {
            request_id: "q".into(),
            task_id: "t".into(),
            attempt_id: "a".into(),
            configuration_revision: "c".into(),
            profile_generation: 1,
            authorization_epoch: 1,
            lease_id: "l".into(),
            unit_id: "u".into(),
            text_revision: 1,
            source_hash: format!("{:x}", Sha256::digest("原文".as_bytes())),
        },
        config: OfflineEngineConfigV1 {
            id: "offline".into(),
            engine: OfflineEngineKey {
                engine_id: "apple_translation".into(),
                contract_revision: NonZeroU32::new(1).unwrap(),
            },
            backend: OfflineBackend::PlatformNative,
            display_name: "test".into(),
            pair: pair.clone(),
            route_policy: OfflineRoutePolicy::DirectOnly {},
            installation: None,
            resolution: OfflineConfigResolution::Resolved {
                resource_snapshot_id: "s".into(),
                route_id: "r".into(),
            },
            request_defaults: ServiceRequestDefaults {
                purpose: app_core::contracts::CatalogPurpose::Translation,
                languages: ServiceLanguageDefaults::OfflineUseConfigPair {},
                context: ServiceContextConsent {
                    enabled: false,
                    max_sentences: 0,
                    mode: HistoryInputMode::Original,
                },
                execution: ServiceExecutionDefaults {
                    timeout_ms: 5000,
                    concurrency: 1,
                    retries: 0,
                },
                cache: ServiceCachePolicy { enabled: false },
                fallback: vec![],
            },
        },
        payload: OfflineRequestPayload {
            source_text: "原文".into(),
            source_language: "ja".into(),
            target_language: "zh".into(),
            context: vec![],
            glossary: vec![],
            selected_token: None,
        },
        resource: ResourceManifest {
            root: PathBuf::from("/tmp"),
            identity: ResourceIdentity {
                snapshot_id: "s".into(),
                route_id: "r".into(),
                observation_id: "o".into(),
                runtime_profile_id: "p".into(),
                runtime_profile_revision: 1,
                installation_revision: 1,
                installation_id: "i".into(),
            },
            digest: manifest_digest(
                &[],
                &route,
                "test",
                &ResourceIdentity {
                    snapshot_id: "s".into(),
                    route_id: "r".into(),
                    observation_id: "o".into(),
                    runtime_profile_id: "p".into(),
                    runtime_profile_revision: 1,
                    installation_revision: 1,
                    installation_id: "i".into(),
                },
            )
            .unwrap(),
            files: vec![],
            route,
            engine_version: "test".into(),
        },
        launch: WorkerLaunch::Native {
            executable: PathBuf::from("/helper"),
            executable_sha256: "0".repeat(64),
        },
        timeout: Duration::from_secs(2),
        expected_observation: RuntimeObservation {
            engine_id: "apple_translation".into(),
            engine_contract_revision: 1,
            runtime_version: "protocol-fixture".into(),
            platform: "fixture".into(),
            architecture: "fixture".into(),
            availability: ResourceAvailability::Installed,
            physical_cancel_supported: false,
            resource_version: None,
            reason: None,
        },
    }
}
#[test]
fn validates_frozen_identity_and_source_hash() {
    let mut r = request();
    assert!(validate_request(&r).is_ok());
    r.payload.source_text.push('新');
    assert_eq!(validate_request(&r).unwrap_err().stage, "source_hash");
}
#[test]
fn rejects_context_or_glossary_instead_of_silently_ignoring() {
    let mut r = request();
    r.payload.context.push(serde_json::json!({"text":"secret"}));
    assert_eq!(
        validate_request(&r).unwrap_err().code,
        OfflineErrorCode::UnsupportedInput
    );
}
#[test]
fn default_direct_does_not_silently_pivot() {
    let mut r = request();
    r.resource.route = vec![
        RouteHop {
            pair: LanguagePair {
                source: "ja".into(),
                target: "en".into(),
            },
            package_relative_path: None,
        },
        RouteHop {
            pair: LanguagePair {
                source: "en".into(),
                target: "zh".into(),
            },
            package_relative_path: None,
        },
    ];
    assert_eq!(validate_request(&r).unwrap_err().stage, "direct_route");
}
#[test]
fn rejects_draft_and_resource_identity_mismatch() {
    let mut r = request();
    r.resource.identity.snapshot_id = "other".into();
    assert_eq!(
        validate_request(&r).unwrap_err().code,
        OfflineErrorCode::ResourceChanged
    );
    r.config.resolution = OfflineConfigResolution::Draft {
        observed_resource: None,
        proposed_route: None,
    };
    assert_eq!(
        validate_request(&r).unwrap_err().code,
        OfflineErrorCode::NotReady
    );
}
#[test]
fn protocol_is_bounded_strict_and_separate_from_hook() {
    for frame in [
        b"{}\n".as_slice(),
        b"{\"kind\":\"ready\",\"protocol\":\"hook-v2\"}\n",
        b"garbage\n",
        b"{}",
    ] {
        assert!(decode(frame).is_err());
    }
    assert!(decode(&vec![b'x'; MAX_FRAME_BYTES + 1]).is_err());
    let mut r = request();
    r.config.engine.contract_revision = NonZeroU32::new(2).unwrap();
    assert_eq!(
        validate_request(&r).unwrap_err().code,
        OfflineErrorCode::UnsupportedContract
    );
}
#[test]
fn identity_cannot_be_empty_or_nonpositive() {
    let mut r = request();
    r.identity.lease_id.clear();
    assert_eq!(validate_request(&r).unwrap_err().stage, "identity");
    r.identity.lease_id = "lease".into();
    r.identity.authorization_epoch = 0;
    assert!(validate_request(&r).is_err());
}

#[test]
fn local_worker_requires_exact_installation_and_translation_purpose() {
    let mut r = request();
    r.config.request_defaults.purpose = app_core::contracts::CatalogPurpose::Grammar;
    assert_eq!(validate_request(&r).unwrap_err().stage, "purpose");
    r.config.request_defaults.purpose = app_core::contracts::CatalogPurpose::Translation;
    r.config.engine.engine_id = "argos_translate".into();
    r.config.backend = OfflineBackend::LocalWorker;
    assert_eq!(
        validate_request(&r).unwrap_err().stage,
        "installation_identity"
    );
    r.config.installation = Some(LocalInstallationRef {
        installation_id: "different-installation".into(),
        revision: NonZeroU32::new(1).unwrap(),
    });
    assert_eq!(
        validate_request(&r).unwrap_err().code,
        OfflineErrorCode::ResourceChanged
    );
}
