//! Synthetic wire facts exercise grammar, never actual loader/lease or product qualification.
use app_core::{
    contracts::{CatalogPurpose, ServiceTestInputScope},
    domain::{
        content_hash, model_config::service_graph::*, offline_runtime::*,
        service_execution::ServicePlanIdentity,
    },
};
use serde_json::{Value, json};
use std::num::NonZeroU32;
fn nz(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v).unwrap()
}
fn sha(s: &str) -> String {
    content_hash(s)
}
fn hello() -> OfflineWorkerHelloV2 {
    let expected = ExpectedRuntimeObservation {
        engine: OfflineEngineKey {
            engine_id: "argos_translate".into(),
            contract_revision: nz(1),
        },
        runtime_version: "fixture-version".into(),
        platform: RuntimePlatform::Macos,
        architecture: RuntimeArchitecture::Arm64,
        resource_version: None,
        physical_cancel_supported: false,
        processing_sha256: sha("processing"),
    };
    let reference = OfflineWorkerReferenceV2 {
        protocol: WorkerProtocolV2::V2,
        worker_instance_id: "child-instance".into(),
        profile_generation: nz(1),
        server_instance_id: "server".into(),
        source: OfflineConfigRef {
            revision_id: "revision".into(),
            config_id: "config".into(),
        },
        operation: OfflineOperationTarget::Attempt {
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            identity: ServicePlanIdentity {
                configuration_revision: "revision".into(),
                binding_id: "binding".into(),
                engine_id: "engine".into(),
                purpose: CatalogPurpose::Translation,
                unit_id: "unit".into(),
                text_revision: 1,
                source_hash: sha("春の空"),
                session_id: "session".into(),
                game_id: Some("game".into()),
            },
        },
        resource: OfflineResourceReference {
            snapshot_id: "snapshot".into(),
            route_id: "route".into(),
            observation: ObservationRef {
                observation_id: "observation".into(),
                epoch: nz(1),
                subject_key: "subject".into(),
                head_revision: nz(1),
            },
            profile: RuntimeProfileRef {
                profile_id: "wire-fixture".into(),
                revision: nz(1),
                engine: expected.engine.clone(),
                backend: OfflineBackend::LocalWorker,
                platform: RuntimePlatform::Macos,
                architecture: RuntimeArchitecture::Arm64,
                private_protocol_revision: nz(2),
                policy_sha256: sha("policy"),
            },
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
                runtime_content_sha256: sha("runtime"),
            },
            resource_content_sha256: sha("resource"),
            processing_sha256: expected.processing_sha256.clone(),
            expected_observation_sha256: expected.canonical_sha256().unwrap(),
            control: Some(ControlArtifactRef {
                relative_key: CONTROL_RELATIVE_KEY.into(),
                byte_length: 12,
                sha256: sha("control"),
            }),
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh-Hans".into(),
            },
        },
    };
    OfflineWorkerHelloV2 {
        route: vec![OfflineRouteHop {
            snapshot_id: reference.resource.snapshot_id.clone(),
            pair: reference.resource.pair.clone(),
            pipeline_profile_id: "pipeline".into(),
            package_relative_key: None,
        }],
        route_policy: OfflineRoutePolicy::DirectOnly {},
        reference,
        expected,
    }
}
fn parent(h: &OfflineWorkerHelloV2) -> ParentFrameV2 {
    ParentFrameV2::Hello {
        hello: Box::new(h.clone()),
    }
}
fn ready(h: &OfflineWorkerHelloV2, records: &[WorkerEvidenceRecordV2]) -> ChildFrameV2 {
    ChildFrameV2::Ready {
        reference: Box::new(h.reference.clone()),
        observed: h.expected.clone(),
        availability: ResourceAvailability::Unknown,
        evidence_count: records.len() as u32,
        evidence_sha256: evidence_records_sha256_v2(WorkerEvidencePhaseV2::Initialization, records)
            .unwrap(),
    }
}
fn record(key: &str) -> WorkerEvidenceRecordV2 {
    WorkerEvidenceRecordV2::File {
        claim: RuntimeFileClaim {
            relative_key: key.into(),
            byte_length: 1,
            sha256: sha(key),
            role: RuntimeFileRole::PackageCode,
        },
    }
}
fn evidence(
    h: &OfflineWorkerHelloV2,
    sequence: u32,
    records: Vec<WorkerEvidenceRecordV2>,
) -> ChildFrameV2 {
    ChildFrameV2::Evidence {
        reference: Box::new(h.reference.clone()),
        phase: WorkerEvidencePhaseV2::Initialization,
        sequence: nz(sequence),
        records,
    }
}
fn cancel(h: &OfflineWorkerHelloV2) -> ParentFrameV2 {
    ParentFrameV2::Cancel {
        reference: Box::new(h.reference.clone()),
    }
}
fn execute(h: &OfflineWorkerHelloV2, text: &str) -> ParentFrameV2 {
    ParentFrameV2::Execute {
        reference: Box::new(h.reference.clone()),
        source_text: text.into(),
    }
}
fn final_frame(h: &OfflineWorkerHelloV2) -> ChildFrameV2 {
    ChildFrameV2::Final {
        reference: Box::new(h.reference.clone()),
        route: h.route.clone(),
        text: "春天的天空".into(),
        evidence_count: 0,
        evidence_sha256: evidence_records_sha256_v2(WorkerEvidencePhaseV2::Execution, &[]).unwrap(),
    }
}
fn started(h: &OfflineWorkerHelloV2) -> WorkerConversationV2 {
    let mut c = WorkerConversationV2::new(h.clone()).unwrap();
    c.sent(&parent(h)).unwrap();
    c
}
fn wire(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec(v).unwrap();
    b.push(b'\n');
    b
}
#[test]
fn frames_roundtrip_without_source_in_observation_frames() {
    let h = hello();
    for p in [parent(&h), cancel(&h), execute(&h, "春の空")] {
        assert_eq!(decode_parent_v2(&encode_parent_v2(&p).unwrap()).unwrap(), p);
    }
    for c in [
        evidence(&h, 1, vec![record("pkg/one")]),
        ready(&h, &[]),
        final_frame(&h),
        ChildFrameV2::Error {
            reference: Box::new(h.reference.clone()),
            reason_code: "runtime_unavailable".into(),
        },
    ] {
        assert_eq!(decode_child_v2(&encode_child_v2(&c).unwrap()).unwrap(), c);
    }
    for b in [
        encode_parent_v2(&parent(&h)).unwrap(),
        encode_child_v2(&evidence(&h, 1, vec![record("pkg/one")])).unwrap(),
        encode_child_v2(&ready(&h, &[])).unwrap(),
    ] {
        let s = String::from_utf8(b).unwrap();
        assert!(!s.contains("sourceText"));
        assert!(!s.contains("春の空"));
        assert!(!s.contains("payloadFiles"));
    }
    assert!(!format!("{:?}", execute(&h, "春の空")).contains("春の空"));
    assert!(!format!("{:?}", final_frame(&h)).contains("春天的天空"));
}
#[test]
fn observation_state_is_not_execution_authority_and_cancel_terminates() {
    let h = hello();
    let mut c = started(&h);
    assert!(c.eof().is_err());
    c.received(&ready(&h, &[])).unwrap();
    assert_eq!(c.state(), WorkerConversationStateV2::ReadyObserved);
    assert!(c.eof().is_err());
    c.sent(&cancel(&h)).unwrap();
    c.eof().unwrap();
    assert!(c.received(&ready(&h, &[])).is_err());
    assert!(c.sent(&execute(&h, "春の空")).is_err());
}
#[test]
fn execute_final_only_exercise_the_pure_grammar_not_a_sender() {
    let h = hello();
    let mut c = started(&h);
    assert!(c.sent(&execute(&h, "春の空")).is_err());
    assert!(c.received(&final_frame(&h)).is_err());
    c.received(&ready(&h, &[])).unwrap();
    assert!(c.received(&ready(&h, &[])).is_err());
    c.sent(&execute(&h, "春の空")).unwrap();
    assert_eq!(c.state(), WorkerConversationStateV2::AwaitingFinal);
    c.received(&final_frame(&h)).unwrap();
    c.eof().unwrap();
    assert!(c.received(&final_frame(&h)).is_err());
}
#[test]
fn strict_wire_rejects_unknown_duplicate_nested_and_protocol_fields() {
    let original = serde_json::to_value(parent(&hello())).unwrap();
    for mode in 0..9 {
        let mut v = original.clone();
        match mode {
            0 => v["hello"]["reference"]["protocol"] = json!("translation-worker-v1"),
            1 => v["hello"]["sourceText"] = json!("not_allowed"),
            2 => v["hello"]["reference"]["resource"]["coverage"] = json!(true),
            3 => v["hello"]["expected"]["complete"] = json!(true),
            4 => v["hello"]["routePolicy"]["allow"] = json!(true),
            5 => v["hello"]["route"][0]["unknown"] = json!(1),
            6 => v["hello"]["reference"]["operation"]["identity"]["unknown"] = json!(1),
            7 => v["hello"]["reference"]["profileGeneration"] = json!(0),
            _ => v["kind"] = json!("ready"),
        };
        assert!(decode_parent_v2(&wire(&v)).is_err(), "mode {mode}");
    }
    let s = String::from_utf8(encode_parent_v2(&parent(&hello())).unwrap()).unwrap();
    for (needle, replacement) in [
        (
            "\"kind\":\"hello\"",
            "\"kind\":\"hello\",\"kind\":\"hello\"",
        ),
        ("\"source\":\"ja\"", "\"source\":\"ja\",\"source\":\"ja\""),
        (
            "\"taskId\":\"task\"",
            "\"taskId\":\"task\",\"taskId\":\"task\"",
        ),
    ] {
        assert!(s.contains(needle));
        assert!(decode_parent_v2(s.replacen(needle, replacement, 1).as_bytes()).is_err());
    }
}
#[test]
fn exact_frame_byte_boundary_utf8_newline_and_multiple_frames() {
    let mut large = hello();
    large.reference.worker_instance_id = "x".repeat(MAX_V2_FRAME_BYTES);
    assert_eq!(
        encode_parent_v2(&parent(&large)).unwrap_err().code,
        "offline_v2_frame_too_large"
    );
    let bytes = encode_parent_v2(&parent(&hello())).unwrap();
    let mut exact = bytes[..bytes.len() - 1].to_vec();
    exact.resize(MAX_V2_FRAME_BYTES - 1, b' ');
    exact.push(b'\n');
    decode_parent_v2(&exact).unwrap();
    exact.insert(exact.len() - 1, b' ');
    assert!(decode_parent_v2(&exact).is_err());
    assert!(decode_parent_v2(&bytes[..bytes.len() - 1]).is_err());
    let mut b = bytes.clone();
    b.extend_from_slice(&bytes);
    assert!(decode_parent_v2(&b).is_err());
    assert!(decode_parent_v2(&[]).is_err());
    assert!(decode_parent_v2(b"\xff\n").is_err());
    assert!(decode_child_v2(b"{\"kind\":1}\n").is_err());
}
#[test]
fn source_byte_limits_hash_and_nul_preserve_unicode_semantics() {
    let mut h = hello();
    let text = "あ".repeat(MAX_V2_SOURCE_BYTES / 3) + "a";
    assert_eq!(text.len(), MAX_V2_SOURCE_BYTES);
    if let OfflineOperationTarget::Attempt { identity, .. } = &mut h.reference.operation {
        identity.source_hash = sha(&text);
    }
    encode_parent_v2(&execute(&h, &text)).unwrap();
    assert!(encode_parent_v2(&execute(&h, &(text + "x"))).is_err());
    for text in ["", "春の空\0", "春の海"] {
        assert!(encode_parent_v2(&execute(&hello(), text)).is_err());
    }
}
#[test]
fn output_byte_boundary_and_error_codes_are_strict() {
    let h = hello();
    let mut f = final_frame(&h);
    if let ChildFrameV2::Final { text, .. } = &mut f {
        *text = "x".repeat(MAX_V2_OUTPUT_BYTES);
    }
    encode_child_v2(&f).unwrap();
    if let ChildFrameV2::Final { text, .. } = &mut f {
        text.push('x');
    }
    assert!(encode_child_v2(&f).is_err());
    for code in ["", "Raw native error", "secret\nline", "uppercase_A"] {
        assert!(
            encode_child_v2(&ChildFrameV2::Error {
                reference: Box::new(h.reference.clone()),
                reason_code: code.into()
            })
            .is_err()
        );
    }
}
#[test]
fn all_frozen_reference_axes_and_operation_kinds_are_checked() {
    let h = hello();
    for mode in 0..10 {
        let mut r = ready(&h, &[]);
        if let ChildFrameV2::Ready { reference, .. } = &mut r {
            match mode {
                0 => reference.worker_instance_id = "other".into(),
                1 => reference.server_instance_id = "other".into(),
                2 => reference.profile_generation = nz(2),
                3 => reference.resource.observation.epoch = nz(2),
                4 => reference.resource.observation.head_revision = nz(2),
                5 => reference.resource.route_id = "other".into(),
                6 => reference.resource.profile.policy_sha256 = sha("other"),
                7 => reference.resource.processing_sha256 = sha("other"),
                8 => reference.source.config_id = "other".into(),
                _ => {
                    reference.operation = OfflineOperationTarget::ExplicitTest {
                        operation_id: "test".into(),
                        test_id: "test-id".into(),
                        input_scope: ServiceTestInputScope::Sample {},
                        input_sha256: sha("春の空"),
                        request_semantics_sha256: sha("semantics"),
                    }
                }
            }
        }
        let mut c = started(&h);
        assert!(c.received(&r).is_err(), "mode {mode}");
        assert_eq!(c.state(), WorkerConversationStateV2::AwaitingReady);
    }
}
#[test]
fn explicit_test_does_not_synthesize_task_unit_or_installation_for_apple() {
    let mut h = hello();
    h.reference.operation = OfflineOperationTarget::ExplicitTest {
        operation_id: "test".into(),
        test_id: "receipt".into(),
        input_scope: ServiceTestInputScope::Sample {},
        input_sha256: sha("春の空"),
        request_semantics_sha256: sha("semantics"),
    };
    let b = encode_parent_v2(&parent(&h)).unwrap();
    let s = String::from_utf8(b).unwrap();
    for key in ["taskId", "attemptId", "unitId"] {
        assert!(!s.contains(key));
    }
    let mut c = started(&h);
    c.received(&ready(&h, &[])).unwrap();
    c.sent(&execute(&h, "春の空")).unwrap();
    h.reference.resource.profile.engine.engine_id = "apple_translation".into();
    h.reference.resource.profile.backend = OfflineBackend::PlatformNative;
    h.expected.engine = h.reference.resource.profile.engine.clone();
    h.reference.resource.expected_observation_sha256 = h.expected.canonical_sha256().unwrap();
    h.reference.resource.control = None;
    h.reference.resource.backend = ResourceBackendIdentity::AppleSystem {
        os_build: "os-build".into(),
        bridge_build_sha256: sha("bridge"),
        api_profile: "low-latency".into(),
        strategy: AppleStrategy::TraditionalLowLatency,
    };
    let b = encode_parent_v2(&parent(&h)).unwrap();
    assert!(!String::from_utf8(b).unwrap().contains("installationId"));
}
#[test]
fn hello_expected_and_route_binding_reject_mutations() {
    for mode in 0..7 {
        let mut h = hello();
        match mode {
            0 => h.expected.runtime_version = "other".into(),
            1 => h.expected.processing_sha256 = sha("other"),
            2 => h.expected.platform = RuntimePlatform::Windows,
            3 => h.expected.architecture = RuntimeArchitecture::X64,
            4 => h.expected.engine.contract_revision = nz(2),
            5 => h.route[0].snapshot_id = "other".into(),
            _ => h.route[0].pair.target = "en".into(),
        }
        assert!(h.validate().is_err(), "mode {mode}");
    }
}
#[test]
fn pivot_allows_three_connected_explicit_hops_not_four_or_implicit() {
    let mut h = hello();
    let langs = ["ja", "en", "de", "zh-Hans"];
    h.route = (0..3)
        .map(|i| OfflineRouteHop {
            snapshot_id: if i == 0 {
                "snapshot".into()
            } else {
                format!("snapshot-{i}")
            },
            pair: LanguagePair {
                source: langs[i].into(),
                target: langs[i + 1].into(),
            },
            pipeline_profile_id: "pipeline".into(),
            package_relative_key: Some(format!("packages/{i}")),
        })
        .collect();
    h.route_policy = OfflineRoutePolicy::ExplicitPivot {
        consent_revision: nz(1),
    };
    h.validate().unwrap();
    let mut wrong = h.clone();
    wrong.route_policy = OfflineRoutePolicy::DirectOnly {};
    assert!(wrong.validate().is_err());
    wrong = h.clone();
    wrong.route[1].pair.source = "fr".into();
    assert!(wrong.validate().is_err());
    h.route.push(h.route[2].clone());
    assert!(h.validate().is_err());
}
#[test]
fn evidence_chunk_sequence_duplicate_key_digest_and_phase_fail_closed() {
    let h = hello();
    let records = vec![record("pkg/a"), record("pkg/b")];
    let mut c = started(&h);
    assert!(c.received(&evidence(&h, 2, records.clone())).is_err());
    c.received(&evidence(&h, 1, records.clone())).unwrap();
    assert!(c.received(&evidence(&h, 2, vec![record("pkg/a")])).is_err());
    let mut wrong = ready(&h, &records);
    if let ChildFrameV2::Ready { evidence_count, .. } = &mut wrong {
        *evidence_count = 0;
    }
    assert!(c.received(&wrong).is_err());
    if let ChildFrameV2::Ready {
        evidence_count,
        evidence_sha256,
        ..
    } = &mut wrong
    {
        *evidence_count = 2;
        *evidence_sha256 = sha("fabricated");
    }
    assert!(c.received(&wrong).is_err());
    let mut bad = evidence(&h, 2, vec![record("pkg/c")]);
    if let ChildFrameV2::Evidence { phase, .. } = &mut bad {
        *phase = WorkerEvidencePhaseV2::Execution;
    }
    assert!(c.received(&bad).is_err());
    c.received(&ready(&h, &records)).unwrap();
    assert!(c.received(&evidence(&h, 2, vec![record("pkg/c")])).is_err());
}
#[test]
fn evidence_shape_rejects_oversized_empty_duplicate_and_unsafe_file_claims() {
    let h = hello();
    for records in [
        vec![],
        vec![record("a"), record("a")],
        (0..=MAX_V2_EVIDENCE_CHUNK)
            .map(|i| record(&format!("p/{i}")))
            .collect(),
    ] {
        assert!(encode_child_v2(&evidence(&h, 1, records)).is_err());
    }
    for key in ["../out", "/absolute", "a/../b", CONTROL_RELATIVE_KEY] {
        assert!(encode_child_v2(&evidence(&h, 1, vec![record(key)])).is_err());
    }
    let mut r = record("p/a");
    if let WorkerEvidenceRecordV2::File { claim } = &mut r {
        claim.byte_length = u64::MAX;
    }
    assert!(encode_child_v2(&evidence(&h, 1, vec![r])).is_err());
    let mut raw = serde_json::to_value(evidence(&h, 1, vec![record("a")])).unwrap();
    raw["records"][0]["claim"]["role"] = json!("complete");
    assert!(decode_child_v2(&wire(&raw)).is_err());
}
#[test]
fn aggregate_evidence_limit_is_independent_of_per_frame_limits() {
    let h = hello();
    let mut c = started(&h);
    let mut all = Vec::new();
    for chunk in 0..MAX_SNAPSHOT_FILES / MAX_V2_EVIDENCE_CHUNK {
        let r = (0..MAX_V2_EVIDENCE_CHUNK)
            .map(|i| record(&format!("file/{}", chunk * MAX_V2_EVIDENCE_CHUNK + i)))
            .collect::<Vec<_>>();
        c.received(&evidence(&h, chunk as u32 + 1, r.clone()))
            .unwrap();
        all.extend(r);
    }
    assert!(
        c.received(&evidence(&h, 65, vec![record("one-too-many")]))
            .is_err()
    );
    c.received(&ready(&h, &all)).unwrap();
}
#[test]
fn same_version_observed_field_changes_cannot_pass_ready_binding() {
    let h = hello();
    for mode in 0..6 {
        let mut r = ready(&h, &[]);
        if let ChildFrameV2::Ready { observed, .. } = &mut r {
            match mode {
                0 => observed.engine.contract_revision = nz(2),
                1 => observed.runtime_version = "same-label-different".into(),
                2 => observed.resource_version = Some("other".into()),
                3 => observed.processing_sha256 = sha("other"),
                4 => observed.physical_cancel_supported = true,
                _ => observed.architecture = RuntimeArchitecture::X86,
            }
        }
        assert!(started(&h).received(&r).is_err());
    }
}
#[test]
fn malformed_terminal_and_out_of_phase_records_cannot_resurrect_session() {
    let h = hello();
    let mut c = started(&h);
    let e = ChildFrameV2::Error {
        reference: Box::new(h.reference.clone()),
        reason_code: "not_available".into(),
    };
    c.received(&e).unwrap();
    c.eof().unwrap();
    assert!(c.sent(&parent(&h)).is_err());
    assert!(c.received(&e).is_err());
    let mut c = started(&h);
    c.received(&ready(&h, &[])).unwrap();
    c.sent(&execute(&h, "春の空")).unwrap();
    let mut f = final_frame(&h);
    if let ChildFrameV2::Final { route, .. } = &mut f {
        route[0].pipeline_profile_id = "other".into();
    }
    assert!(c.received(&f).is_err());
    assert_eq!(c.state(), WorkerConversationStateV2::AwaitingFinal);
    assert!(c.eof().is_err());
}

#[test]
fn system_api_wire_records_bind_os_bridge_strategy_and_epoch_without_creating_a_proof() {
    let mut h = hello();
    h.reference.resource.profile.engine.engine_id = "apple_translation".into();
    h.reference.resource.profile.backend = OfflineBackend::PlatformNative;
    h.expected.engine = h.reference.resource.profile.engine.clone();
    h.reference.resource.expected_observation_sha256 = h.expected.canonical_sha256().unwrap();
    h.reference.resource.control = None;
    h.reference.resource.backend = ResourceBackendIdentity::AppleSystem {
        os_build: "os-build".into(),
        bridge_build_sha256: sha("bridge"),
        api_profile: "low-latency".into(),
        strategy: AppleStrategy::TraditionalLowLatency,
    };
    let fact = WorkerEvidenceRecordV2::SystemApi {
        os_build: "os-build".into(),
        bridge_build_sha256: sha("bridge"),
        api_profile: "low-latency".into(),
        strategy: AppleStrategy::TraditionalLowLatency,
        observation_epoch: nz(1),
    };
    let mut c = started(&h);
    c.received(&evidence(&h, 1, vec![fact.clone()])).unwrap();
    c.received(&ready(&h, std::slice::from_ref(&fact))).unwrap();
    assert_eq!(c.state(), WorkerConversationStateV2::ReadyObserved);
    for mode in 0..4 {
        let mut changed = fact.clone();
        if let WorkerEvidenceRecordV2::SystemApi {
            os_build,
            bridge_build_sha256,
            api_profile,
            observation_epoch,
            ..
        } = &mut changed
        {
            match mode {
                0 => *os_build = "changed".into(),
                1 => *bridge_build_sha256 = sha("changed"),
                2 => *api_profile = "changed".into(),
                _ => *observation_epoch = nz(2),
            }
        }
        assert!(encode_child_v2(&evidence(&h, 1, vec![changed])).is_err());
    }
    assert!(encode_child_v2(&evidence(&h, 1, vec![record("dictionary/file")])).is_err());
    assert!(encode_child_v2(&evidence(&hello(), 1, vec![fact])).is_err());
}
#[test]
fn execution_phase_has_its_own_sequence_records_and_digest_domain() {
    let h = hello();
    let r = vec![record("pkg/one")];
    let mut c = started(&h);
    c.received(&evidence(&h, 1, r.clone())).unwrap();
    c.received(&ready(&h, &r)).unwrap();
    c.sent(&execute(&h, "春の空")).unwrap();
    let mut e = evidence(&h, 1, r.clone());
    if let ChildFrameV2::Evidence { phase, .. } = &mut e {
        *phase = WorkerEvidencePhaseV2::Execution;
    }
    c.received(&e).unwrap();
    let mut final_value = final_frame(&h);
    if let ChildFrameV2::Final {
        evidence_count,
        evidence_sha256,
        ..
    } = &mut final_value
    {
        *evidence_count = 1;
        *evidence_sha256 =
            evidence_records_sha256_v2(WorkerEvidencePhaseV2::Initialization, &r).unwrap();
    }
    assert!(c.received(&final_value).is_err());
    if let ChildFrameV2::Final {
        evidence_sha256, ..
    } = &mut final_value
    {
        *evidence_sha256 =
            evidence_records_sha256_v2(WorkerEvidencePhaseV2::Execution, &r).unwrap();
    }
    c.received(&final_value).unwrap();
    c.eof().unwrap();
}
