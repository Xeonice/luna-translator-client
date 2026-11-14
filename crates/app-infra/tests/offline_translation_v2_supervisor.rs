//! Real Unix Python child transport. No Argos/vendor translation or runtime grant is claimed.
#![cfg(unix)]

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

use app_core::ports::{operation_budget::OperationBudget, physical_jobs::PhysicalJobs};
use app_infra::offline_translation::{
    OfflineErrorCode, OfflineExecutor, WorkerLaunch, resources::*, v2_supervisor::*,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
struct Fixture {
    root: PathBuf,
    launch: WorkerLaunch,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("luna-v2-observation-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let script = root.join("worker.py");
        let digest =
            evidence_records_sha256_v2(WorkerEvidencePhaseV2::Initialization, &[]).unwrap();
        let code = format!(
            r#"import json,sys,os,time
root={root:?}
mode={mode:?}
def log(s):
 with open(root+'/stdin.jsonl','a') as f:f.write(s)
hline=sys.stdin.readline();log(hline);h=json.loads(hline)['hello'];ref=h['reference']
open(root+'/owned.pid','w').write(str(os.getpid()))
def emit(x):print(json.dumps(x),flush=True)
if mode=='hang':time.sleep(60)
if mode=='bad':print('broken',flush=True);time.sleep(60)
if mode=='huge':sys.stdout.write('x'*1048577);sys.stdout.flush();time.sleep(60)
if mode=='exit':sys.exit(7)
r={{'kind':'ready','reference':ref,'observed':h['expected'],'availability':'unknown','evidenceCount':0,'evidenceSha256':{digest:?}}}
if mode=='identity':r['reference']['workerInstanceId']='other'
if mode=='version':r['observed']['runtimeVersion']='other'
if mode=='digest':r['evidenceSha256']='0'*64
emit(r)
if mode=='duplicate':emit(r)
if mode=='stderr':sys.stderr.write('x'*1048577);sys.stderr.flush()
if mode=='ignore_cancel':time.sleep(60)
for line in sys.stdin:
 log(line);m=json.loads(line)
 if m['kind']=='execute':open(root+'/execute_seen','w').write('unexpected')
 if m['kind']=='cancel':
  if mode=='slow_close':open(root+'/closing','w').write('yes');time.sleep(0.15)
  if mode=='unclean_close':sys.exit(9)
  break
"#,
            root = root.to_str().unwrap(),
            mode = mode,
            digest = digest
        );
        std::fs::write(&script, code).unwrap();
        let interpreter = PathBuf::from("/usr/bin/python3").canonicalize().unwrap();
        let launch = WorkerLaunch::Argos {
            interpreter_sha256: hash_file(&interpreter, 512 * 1024 * 1024).unwrap().1,
            interpreter,
            script_sha256: hash_file(&script, 1_000_000).unwrap().1,
            script,
        };
        Self { root, launch }
    }
    fn request(&self) -> OfflineObservationRequestV2 {
        OfflineObservationRequestV2 {
            hello: hello(),
            launch: self.launch.clone(),
        }
    }
    fn pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.root.join("owned.pid"))
            .ok()
            .and_then(|x| x.parse().ok())
    }
    async fn wait_pid(&self) -> i32 {
        for _ in 0..100 {
            if let Some(pid) = self.pid() {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture child did not start")
    }
    fn no_execute(&self) {
        if let Ok(wire) = std::fs::read_to_string(self.root.join("stdin.jsonl")) {
            for line in wire.lines() {
                let v: serde_json::Value = serde_json::from_str(line).unwrap();
                assert!(v["kind"] == "hello" || v["kind"] == "cancel");
                assert!(!line.contains("sourceText"));
                assert!(!line.contains("never-sent-source"));
            }
        }
        assert!(!self.root.join("execute_seen").exists());
    }
}
fn setup() -> (OfflineExecutor, ResourceInspector, Arc<PhysicalJobs>) {
    (
        OfflineExecutor::new(),
        ResourceInspector::new(),
        Arc::new(PhysicalJobs::default()),
    )
}
fn budget() -> OperationBudget {
    OperationBudget::new(Duration::from_secs(4)).unwrap()
}
fn exited(pid: i32) -> bool {
    // SAFETY: signal 0 only queries this fixture PID; it neither signals nor dereferences memory.
    unsafe { libc::kill(pid, 0) != 0 }
}
#[tokio::test]
async fn actual_observation_returns_only_wire_facts_then_close_reaps_child() {
    let f = Fixture::new("normal");
    let (owner, inspector, physical) = setup();
    let supervisor = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
    let session = supervisor
        .observe(f.request(), budget())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let pid = f.wait_pid().await;
    assert!(matches!(
        session.facts().ready(),
        ChildFrameV2::Ready {
            availability: ResourceAvailability::Unknown,
            ..
        }
    ));
    assert!(session.facts().records().is_empty());
    assert!(
        tokio::time::timeout(
            Duration::from_millis(25),
            physical.wait_for_physical_drain()
        )
        .await
        .is_err()
    );
    session.close().await.unwrap();
    assert!(exited(pid));
    physical.wait_for_physical_drain().await.unwrap();
    f.no_execute();
}
#[tokio::test]
async fn same_executor_capacity_is_shared_while_filesystem_inspector_remains_free() {
    let f = Fixture::new("normal");
    let other = Fixture::new("normal");
    let (owner, inspector, physical) = setup();
    let a = OfflineObservationSupervisorV2::new(&owner, inspector.clone(), physical.clone());
    let b = OfflineObservationSupervisorV2::new(&owner, inspector.clone(), physical);
    let session = a
        .observe(f.request(), budget())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let error = match b.observe(other.request(), budget()).await {
        Err(e) => e,
        Ok(_) => panic!("second engine slot was allocated"),
    };
    assert_eq!(error.code, OfflineErrorCode::WorkerBusy);
    std::fs::write(f.root.join("data"), "file-job").unwrap();
    let files = inspector
        .inspect(
            InspectionRequest {
                root: f.root.clone(),
                selected: vec!["data".into()],
            },
            Duration::from_secs(1),
            tokio::sync::watch::channel(false).1,
        )
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    session.close().await.unwrap();
    b.observe(other.request(), budget())
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .close()
        .await
        .unwrap();
    f.no_execute();
    other.no_execute();
}
#[tokio::test]
async fn protocol_failures_exit_and_never_send_execute_or_release_a_live_child() {
    for mode in ["bad", "huge", "exit", "identity", "version", "digest"] {
        let f = Fixture::new(mode);
        let (owner, inspector, physical) = setup();
        let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
        assert!(s.observe(f.request(), budget()).await.is_err(), "{mode}");
        if let Some(pid) = f.pid() {
            assert!(exited(pid), "{mode}");
        }
        physical.wait_for_physical_drain().await.unwrap();
        f.no_execute();
    }
}
#[tokio::test]
async fn original_budget_cancel_and_timeout_terminate_owned_worker() {
    for timeout in [false, true] {
        let f = Fixture::new("hang");
        let (owner, inspector, physical) = setup();
        let s = Arc::new(OfflineObservationSupervisorV2::new(
            &owner,
            inspector,
            physical.clone(),
        ));
        let b = OperationBudget::new(if timeout {
            Duration::from_millis(650)
        } else {
            Duration::from_secs(4)
        })
        .unwrap();
        let original = b.clone();
        let req = f.request();
        let job = tokio::spawn(async move { s.observe(req, b).await });
        let pid = f.wait_pid().await;
        if !timeout {
            original.cancel();
        }
        let e = match job.await.unwrap() {
            Err(e) => e,
            Ok(_) => panic!("hang became ready"),
        };
        assert_eq!(
            e.code,
            if timeout {
                OfflineErrorCode::DeadlineExceeded
            } else {
                OfflineErrorCode::Cancelled
            }
        );
        assert!(exited(pid));
        physical.wait_for_physical_drain().await.unwrap();
        f.no_execute();
    }
}
#[tokio::test]
async fn dropped_pending_caller_preserves_physical_guard_until_actual_cleanup() {
    let f = Fixture::new("hang");
    let (owner, inspector, physical) = setup();
    let s = Arc::new(OfflineObservationSupervisorV2::new(
        &owner,
        inspector,
        physical.clone(),
    ));
    let req = f.request();
    let job = tokio::spawn(async move { s.observe(req, budget()).await });
    let pid = f.wait_pid().await;
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            physical.wait_for_physical_drain()
        )
        .await
        .is_err()
    );
    job.abort();
    let _ = job.await;
    tokio::time::timeout(Duration::from_secs(3), physical.wait_for_physical_drain())
        .await
        .unwrap()
        .unwrap();
    assert!(exited(pid));
    f.no_execute();
}
#[tokio::test]
async fn dropped_ready_session_and_stderr_flood_are_physically_drained() {
    for mode in ["ignore_cancel", "stderr"] {
        let f = Fixture::new(mode);
        let (owner, inspector, physical) = setup();
        let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
        let session = s
            .observe(f.request(), budget())
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let pid = f.wait_pid().await;
        drop(session);
        tokio::time::timeout(Duration::from_secs(3), physical.wait_for_physical_drain())
            .await
            .unwrap()
            .unwrap();
        assert!(exited(pid));
        f.no_execute();
    }
}
#[tokio::test]
async fn duplicate_ready_is_not_accepted_as_another_session() {
    let f = Fixture::new("duplicate");
    let (owner, inspector, physical) = setup();
    let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
    if let Ok(session) = s.observe(f.request(), budget()).await {
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(session.close().await.is_err());
    }
    physical.wait_for_physical_drain().await.unwrap();
    f.no_execute();
}
#[tokio::test]
async fn slot_and_guard_stay_bound_during_child_exit_grace() {
    let f = Fixture::new("slow_close");
    let other = Fixture::new("normal");
    let (owner, inspector, physical) = setup();
    let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
    let session = s
        .observe(f.request(), budget())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let close = tokio::spawn(session.close());
    for _ in 0..100 {
        if f.root.join("closing").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(f.root.join("closing").exists());
    assert!(s.observe(other.request(), budget()).await.is_err());
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            physical.wait_for_physical_drain()
        )
        .await
        .is_err()
    );
    close.await.unwrap().unwrap();
    s.observe(other.request(), budget())
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .close()
        .await
        .unwrap();
}
#[tokio::test]
async fn cancelled_budget_closed_admission_and_changed_helper_fail_before_spawn() {
    let f = Fixture::new("normal");
    let (owner, inspector, physical) = setup();
    let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
    let b = budget();
    b.cancel();
    assert!(s.observe(f.request(), b).await.is_err());
    assert!(f.pid().is_none());
    let mut request = f.request();
    if let WorkerLaunch::Argos { script_sha256, .. } = &mut request.launch {
        *script_sha256 = content_hash("wrong");
    }
    assert!(s.observe(request, budget()).await.is_err());
    assert!(f.pid().is_none());
    physical.close_admission().unwrap();
    assert!(s.observe(f.request(), budget()).await.is_err());
    physical.wait_for_physical_drain().await.unwrap();
    assert!(f.pid().is_none());
}

#[tokio::test]
async fn nonzero_exit_after_cancel_is_reported_after_physical_cleanup() {
    let f = Fixture::new("unclean_close");
    let (owner, inspector, physical) = setup();
    let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
    let session = s
        .observe(f.request(), budget())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let pid = f.wait_pid().await;
    assert!(session.close().await.is_err());
    physical.wait_for_physical_drain().await.unwrap();
    assert!(exited(pid));
    f.no_execute();
}
#[tokio::test]
async fn crossover_launcher_is_not_misrepresented_as_an_owned_guest_worker() {
    let f = Fixture::new("normal");
    let (owner, inspector, physical) = setup();
    let s = OfflineObservationSupervisorV2::new(&owner, inspector, physical.clone());
    let request = OfflineObservationRequestV2 {
        hello: hello(),
        launch: WorkerLaunch::CrossOver {
            launcher: f.root.join("not_launched"),
            launcher_sha256: content_hash("launcher"),
            bottle: "fixture".into(),
            worker: f.root.join("not_launched.exe"),
            worker_sha256: content_hash("worker"),
            worker_resource_root: "C:\\runtime".into(),
        },
    };
    let e = match s.observe(request, budget()).await {
        Err(e) => e,
        Ok(_) => panic!("unverified guest ownership accepted"),
    };
    assert_eq!(e.stage, "v2_guest_owner_pending");
    physical.wait_for_physical_drain().await.unwrap();
    assert!(f.pid().is_none());
}
