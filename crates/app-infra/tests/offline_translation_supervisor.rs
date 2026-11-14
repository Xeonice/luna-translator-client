//! Real child-process fault injection. Fixtures prove transport, never translation quality.
#![cfg(unix)]
use app_core::domain::{model_config::service_graph::*, service_execution::OfflineRequestPayload};
use app_infra::offline_translation::{resources::*, *};
use sha2::{Digest, Sha256};
use std::{num::NonZeroU32, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};
use tokio::sync::watch;
struct Fixture {
    root: PathBuf,
    request: OfflineExecution,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "luna-offline-transport-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("pkg/model")).unwrap();
        std::fs::create_dir_all(root.join("pkg/stanza")).unwrap();
        let root = root.canonicalize().unwrap();
        let mut files = vec![];
        for relative in [
            "pkg/metadata.json",
            "pkg/model/model.bin",
            "pkg/stanza/resources.json",
            "pkg/sentencepiece.model",
        ] {
            let path = root.join(relative);
            std::fs::write(&path, b"transport fixture - not a model").unwrap();
            let (length, sha256) = hash_file(&path, 1000).unwrap();
            files.push(ResourceFile {
                relative_path: relative.into(),
                sha256,
                length,
            });
        }
        let route = vec![RouteHop {
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh".into(),
            },
            package_relative_path: Some("pkg".into()),
        }];
        let identity = ResourceIdentity {
            snapshot_id: "s".into(),
            route_id: "r".into(),
            observation_id: "o".into(),
            runtime_profile_id: "p".into(),
            runtime_profile_revision: 1,
            installation_revision: 1,
            installation_id: "i".into(),
        };
        let pid_path = root.join("owned.pid");
        let script = root.join("worker.py");
        let source = format!(
            r#"import json,sys,time,os
h=json.loads(sys.stdin.readline())
open({pid:?},'w').write(str(os.getpid()))
mode={mode:?}
def emit(x): print(json.dumps(x),flush=True)
if mode=='bad_json': print('broken',flush=True);sys.exit(0)
if mode=='huge': print('x'*1048578,flush=True);sys.exit(0)
if mode=='hang': time.sleep(60);sys.exit(0)
if mode=='exit': sys.exit(7)
o={{'engineId':'argos_translate','engineContractRevision':1,'runtimeVersion':'transport-fixture','platform':'fixture','architecture':'fixture','availability':'installed','physicalCancelSupported':False,'resourceVersion':None,'reason':None}}
if mode=='version_changed': o['runtimeVersion']='transport-v2'
if mode=='dependency_changed': o['runtimeVersion']='dependency-version-changed'
if mode=='contract_changed': o['engineContractRevision']=2
emit({{'kind':'ready','protocol':h['protocol'],'workerInstanceId':h['workerInstanceId'],'requestId':h['requestId'],'resourceDigest':h['resource']['digest'],'observation':o}})
if h['operation']=='observe': sys.exit(0)
e=json.loads(sys.stdin.readline())
open(os.path.join(h['resource']['root'],'body_sent'),'w').write('sent')
f={{'kind':'final','protocol':h['protocol'],'workerInstanceId':h['workerInstanceId'],'identity':e['identity'],'resourceIdentity':h['resource']['identity'],'resourceDigest':h['resource']['digest'],'route':h['resource']['route'],'text':'transport-only-output'}}
if mode=='wrong_identity': f['workerInstanceId']='old-instance'
if mode=='stderr': sys.stderr.write('x'*4096);sys.stderr.flush()
if mode=='mutate': open(os.path.join(h['resource']['root'],'pkg/model/model.bin'),'w').write('new')
emit(f)
if mode=='duplicate': emit(f)
if mode=='unclean': sys.exit(9)
"#,
            pid = pid_path.to_str().unwrap(),
            mode = mode
        );
        std::fs::write(&script, source).unwrap();
        let mut perm = std::fs::metadata(&script).unwrap().permissions();
        perm.set_mode(0o700);
        std::fs::set_permissions(&script, perm).unwrap();
        let interpreter = PathBuf::from("/usr/bin/python3").canonicalize().unwrap();
        let launch = WorkerLaunch::Argos {
            interpreter_sha256: hash_file(&interpreter, 512 * 1024 * 1024).unwrap().1,
            interpreter,
            script_sha256: hash_file(&script, 100000).unwrap().1,
            script,
        };
        let source_text = "原文".to_string();
        let request = OfflineExecution {
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
                source_hash: format!("{:x}", Sha256::digest(source_text.as_bytes())),
            },
            config: OfflineEngineConfigV1 {
                id: "offline".into(),
                engine: OfflineEngineKey {
                    engine_id: "argos_translate".into(),
                    contract_revision: NonZeroU32::new(1).unwrap(),
                },
                backend: OfflineBackend::LocalWorker,
                display_name: "transport fixture".into(),
                pair: LanguagePair {
                    source: "ja".into(),
                    target: "zh".into(),
                },
                route_policy: OfflineRoutePolicy::DirectOnly {},
                installation: Some(LocalInstallationRef {
                    installation_id: "i".into(),
                    revision: NonZeroU32::new(1).unwrap(),
                }),
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
                source_text,
                source_language: "ja".into(),
                target_language: "zh".into(),
                context: vec![],
                glossary: vec![],
                selected_token: None,
            },
            resource: ResourceManifest {
                root: root.clone(),
                identity: identity.clone(),
                digest: manifest_digest(&files, &route, "1.11.0", &identity).unwrap(),
                files,
                route,
                engine_version: "1.11.0".into(),
            },
            launch,
            timeout: Duration::from_secs(5),
            expected_observation: RuntimeObservation {
                engine_id: "argos_translate".into(),
                engine_contract_revision: 1,
                runtime_version: "transport-fixture".into(),
                platform: "fixture".into(),
                architecture: "fixture".into(),
                availability: ResourceAvailability::Installed,
                physical_cancel_supported: false,
                resource_version: None,
                reason: None,
            },
        };
        Self { root, request }
    }
    async fn wait_started(&self) {
        for _ in 0..100 {
            if self.root.join("owned.pid").exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture child did not start");
    }
    fn assert_reaped(&self) {
        let pid = std::fs::read_to_string(self.root.join("owned.pid")).unwrap();
        let status = std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "owned child must be reaped");
    }
}
#[tokio::test]
async fn real_child_transport_roundtrip_preserves_all_identities() {
    let f = Fixture::new("ok");
    let (_tx, rx) = watch::channel(false);
    let output = OfflineExecutor::new()
        .execute(f.request.clone(), rx)
        .await
        .unwrap();
    assert_eq!(output.identity, f.request.identity);
    assert_eq!(output.resource, f.request.resource.identity);
    assert_eq!(output.observation.platform, "fixture");
    assert_eq!(output.text, "transport-only-output");
    f.assert_reaped();
}
#[tokio::test]
async fn observe_is_not_a_translation_success() {
    let f = Fixture::new("ok");
    let (_tx, rx) = watch::channel(false);
    let observation = OfflineExecutor::new()
        .observe(
            OfflineProbe {
                request_id: "probe".into(),
                engine: OfflineEngine::ArgosTranslate,
                contract_revision: 1,
                resource: f.request.resource.clone(),
                launch: f.request.launch.clone(),
                timeout: Duration::from_secs(5),
            },
            rx,
        )
        .await
        .unwrap();
    assert_eq!(observation.runtime_version, "transport-fixture");
    f.assert_reaped();
}
#[tokio::test]
async fn malformed_large_eof_old_instance_and_duplicate_are_rejected() {
    for mode in [
        "bad_json",
        "huge",
        "exit",
        "wrong_identity",
        "duplicate",
        "unclean",
    ] {
        let f = Fixture::new(mode);
        let (_tx, rx) = watch::channel(false);
        assert!(
            OfflineExecutor::new()
                .execute(f.request.clone(), rx)
                .await
                .is_err(),
            "{mode}"
        );
        f.assert_reaped();
    }
}
#[tokio::test]
async fn timeout_reaps_only_owned_child() {
    let mut f = Fixture::new("hang");
    f.request.timeout = Duration::from_millis(500);
    let (_tx, rx) = watch::channel(false);
    assert_eq!(
        OfflineExecutor::new()
            .execute(f.request.clone(), rx)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::DeadlineExceeded
    );
    f.assert_reaped();
}
#[tokio::test]
async fn explicit_cancel_blocks_result_and_reaps_child() {
    let f = Fixture::new("hang");
    let (tx, rx) = watch::channel(false);
    let executor = std::sync::Arc::new(OfflineExecutor::new());
    let pending = {
        let request = f.request.clone();
        let executor = executor.clone();
        tokio::spawn(async move { executor.execute(request, rx).await })
    };
    f.wait_started().await;
    tx.send(true).unwrap();
    assert_eq!(
        pending.await.unwrap().unwrap_err().code,
        OfflineErrorCode::Cancelled
    );
    f.assert_reaped();
}
#[tokio::test]
async fn resource_mutation_during_operation_cannot_return_success() {
    let f = Fixture::new("mutate");
    let (_tx, rx) = watch::channel(false);
    assert_eq!(
        OfflineExecutor::new()
            .execute(f.request.clone(), rx)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::ResourceChanged
    );
    f.assert_reaped();
}
#[tokio::test]
async fn same_engine_is_bounded_without_an_extra_business_queue() {
    let f = Fixture::new("hang");
    let (tx, rx) = watch::channel(false);
    let executor = std::sync::Arc::new(OfflineExecutor::new());
    let pending = {
        let executor = executor.clone();
        let request = f.request.clone();
        tokio::spawn(async move { executor.execute(request, rx).await })
    };
    f.wait_started().await;
    let (_other, rx) = watch::channel(false);
    assert_eq!(
        executor
            .execute(f.request.clone(), rx)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::WorkerBusy
    );
    tx.send(true).unwrap();
    let _ = pending.await;
    f.assert_reaped();
}
#[tokio::test]
async fn pre_cancel_does_not_start_a_process() {
    let f = Fixture::new("ok");
    let (_tx, rx) = watch::channel(true);
    assert_eq!(
        OfflineExecutor::new()
            .execute(f.request.clone(), rx)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::Cancelled
    );
    assert!(!f.root.join("owned.pid").exists());
}

#[tokio::test]
async fn changed_runtime_dependency_or_engine_contract_never_receives_original_text() {
    for mode in ["version_changed", "dependency_changed", "contract_changed"] {
        let f = Fixture::new(mode);
        let (_tx, rx) = watch::channel(false);
        let error = OfflineExecutor::new()
            .execute(f.request.clone(), rx)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error.code,
                OfflineErrorCode::ResourceChanged | OfflineErrorCode::UnsupportedContract
            ),
            "{mode}: {error:?}"
        );
        assert!(
            !f.root.join("body_sent").exists(),
            "{mode} must not receive source text"
        );
        f.assert_reaped();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn launcher_hash_deadline_does_not_start_a_late_child() {
    use std::io::Write;
    let mut f = Fixture::new("normal");
    let executable = f.root.join("large-launcher.sh");
    let marker = f.root.join("late-child-started");
    let mut file = std::fs::File::create(&executable).unwrap();
    writeln!(
        file,
        "#!/bin/sh\nprintf started > '{}'\nexit 0\n#",
        marker.display()
    )
    .unwrap();
    let padding = vec![b'#'; 65536];
    for _ in 0..4096 {
        file.write_all(&padding).unwrap();
    }
    file.sync_all().unwrap();
    drop(file);
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let measured = std::time::Instant::now();
    let digest = hash_file(&executable, 512 * 1024 * 1024).unwrap().1;
    let hash_duration = measured.elapsed();
    // Scale to the observed local hashing rate. The launcher is genuinely
    // executable, so an overdue spawn would leave the independent marker.
    let budget = (hash_duration / 4).clamp(Duration::from_millis(5), Duration::from_millis(20));
    f.request.timeout = budget;
    f.request.launch = WorkerLaunch::Argos {
        interpreter: executable,
        interpreter_sha256: digest,
        script: f.root.join("worker.py"),
        script_sha256: hash_file(&f.root.join("worker.py"), 100000).unwrap().1,
    };
    let (_cancel, receiver) = watch::channel(false);
    let started = std::time::Instant::now();
    let result = OfflineExecutor::new()
        .execute(f.request.clone(), receiver)
        .await
        .unwrap_err();
    let elapsed = started.elapsed();
    assert_eq!(result.code, OfflineErrorCode::DeadlineExceeded);
    assert!(elapsed <= budget + Duration::from_millis(300),
        "launcher hashing blocked the single-thread runtime past its deadline: elapsed={elapsed:?}, budget={budget:?}, measured_hash={hash_duration:?}");
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        !marker.exists(),
        "a child started after launcher preparation exceeded its deadline"
    );
}
