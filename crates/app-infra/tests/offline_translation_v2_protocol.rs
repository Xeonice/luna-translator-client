//! Real bounded-pipe checks, not actual translation.

use app_core::{
    contracts::ServiceTestInputScope,
    domain::{content_hash, offline_runtime::*},
};
use serde_json::json;
fn hello() -> OfflineWorkerHelloV2 {
    let sha = |s: &str| content_hash(s);
    let expected = json!({"engine":{"engineId":"argos_translate","contractRevision":1},"runtimeVersion":"transport-fixture-only","platform":"macos","architecture":"arm64","resourceVersion":null,"physicalCancelSupported":false,"processingSha256":sha("processing")});
    let observed: ExpectedRuntimeObservation = serde_json::from_value(expected.clone()).unwrap();
    serde_json::from_value(json!({"reference":{"protocol":"translation-worker-v2","workerInstanceId":"worker","profileGeneration":1,"serverInstanceId":"server","source":{"revisionId":"revision","configId":"config"},"operation":{"kind":"explicit_test","operationId":"test-operation","testId":"test-receipt","inputScope":ServiceTestInputScope::Sample{},"inputSha256":sha("never-sent-source"),"requestSemanticsSha256":sha("semantics")},"resource":{"snapshotId":"snapshot","routeId":"route","observation":{"observationId":"observation","epoch":1,"subjectKey":"subject","headRevision":1},"profile":{"profileId":"transport-fixture","revision":1,"engine":observed.engine,"backend":"local_worker","platform":"macos","architecture":"arm64","privateProtocolRevision":2,"policySha256":sha("policy")},"backend":{"kind":"local_installation","installation":{"installationId":"installation","revision":1},"environment":{"environmentId":"environment","revision":1,"machineFingerprint":sha("machine")},"root":{"grantId":"grant","authorizationEpoch":1,"machineFingerprint":sha("machine")},"runtimeContentSha256":sha("runtime")},"resourceContentSha256":sha("resource"),"processingSha256":sha("processing"),"expectedObservationSha256":observed.canonical_sha256().unwrap(),"control":{"relativeKey":CONTROL_RELATIVE_KEY,"byteLength":12,"sha256":sha("control")},"pair":{"source":"ja","target":"zh-Hans"}}},"expected":expected,"route":[{"snapshotId":"snapshot","pair":{"source":"ja","target":"zh-Hans"},"pipelineProfileId":"pipeline","packageRelativeKey":null}],"routePolicy":{"kind":"direct_only"}})).unwrap()
}
fn ready(h: &OfflineWorkerHelloV2) -> ChildFrameV2 {
    ChildFrameV2::Ready {
        reference: Box::new(h.reference.clone()),
        observed: h.expected.clone(),
        availability: ResourceAvailability::Unknown,
        evidence_count: 0,
        evidence_sha256: evidence_records_sha256_v2(WorkerEvidencePhaseV2::Initialization, &[])
            .unwrap(),
    }
}

use app_infra::offline_translation::v2_protocol::*;
use tokio::io::{AsyncWriteExt, BufReader};
#[tokio::test]
async fn fragmented_real_pipe_preserves_frame_boundaries_and_eof() {
    let a = encode_child_v2(&ready(&hello())).unwrap();
    let mut two = a.clone();
    two.extend_from_slice(&a);
    let (mut w, r) = tokio::io::duplex(32);
    let job = tokio::spawn(async move {
        for chunk in two.chunks(3) {
            w.write_all(chunk).await.unwrap();
        }
    });
    let mut r = BufReader::with_capacity(7, r);
    assert_eq!(read_child_v2(&mut r).await.unwrap(), Some(ready(&hello())));
    assert!(read_child_v2(&mut r).await.unwrap().is_some());
    assert!(read_child_v2(&mut r).await.unwrap().is_none());
    job.await.unwrap();
}
#[tokio::test]
async fn pipe_rejects_limit_during_read_without_needing_a_newline() {
    let (mut w, r) = tokio::io::duplex(4096);
    let job = tokio::spawn(async move {
        let block = [b'x'; 4096];
        loop {
            if w.write_all(&block).await.is_err() {
                break;
            }
        }
    });
    let mut r = BufReader::new(r);
    let e = read_child_v2(&mut r).await.unwrap_err();
    assert_eq!(e.stage, "v2_frame_limit");
    drop(r);
    job.await.unwrap();
}
#[tokio::test]
async fn incomplete_invalid_and_duplicate_json_are_rejected() {
    for bytes in [
        b"{\"kind\":\"ready\"}".to_vec(),
        b"broken\n".to_vec(),
        b"\xff\n".to_vec(),
        b"{\"kind\":\"ready\",\"kind\":\"ready\"}\n".to_vec(),
    ] {
        let mut r = BufReader::new(bytes.as_slice());
        assert!(read_child_v2(&mut r).await.is_err());
    }
}
#[tokio::test]
async fn writer_has_only_hello_cancel_and_never_a_source_field() {
    let h = hello();
    let mut bytes = Vec::new();
    write_hello_v2(&mut bytes, &h).await.unwrap();
    write_cancel_v2(&mut bytes, &h.reference).await.unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let frames = text
        .lines()
        .map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(frames[0]["kind"], "hello");
    assert_eq!(frames[1]["kind"], "cancel");
    assert!(!text.contains("sourceText"));
    assert!(!text.contains("never-sent-source"));
}
#[tokio::test]
async fn exact_one_mebibyte_frame_passes_and_next_byte_fails() {
    let mut bytes = encode_child_v2(&ready(&hello())).unwrap();
    bytes.pop();
    bytes.resize(MAX_V2_FRAME_BYTES - 1, b' ');
    bytes.push(b'\n');
    let mut r = BufReader::new(bytes.as_slice());
    assert!(read_child_v2(&mut r).await.unwrap().is_some());
    bytes.insert(bytes.len() - 1, b' ');
    let mut r = BufReader::new(bytes.as_slice());
    assert!(read_child_v2(&mut r).await.is_err());
}
