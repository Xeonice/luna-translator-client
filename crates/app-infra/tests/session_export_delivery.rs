//! Actual filesystem delivery over real runtime/SQLite snapshot paging.
//! Controlled unused service ports do not claim model or live game validation.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
    contracts::*,
    domain::{TaskDispatch, capture::*, content_hash},
    ports::{ProductRepository, runtime::*},
};
use app_infra::{
    platform::{
        SystemIdentity,
        session_export::{ExportControl, deliver},
    },
    storage::SqliteStore,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
struct UnusedPorts;
fn unused<T>() -> AppResult<T> {
    Err(AppError::new(
        "unused_controlled_service",
        "test",
        "filesystem test must not invoke external services",
    ))
}
impl HookBackend for UnusedPorts {
    fn targets(&self) -> AppResult<TargetCandidates> {
        unused()
    }
    fn associate(&self, _: AssociateRequest) -> AppResult<HookSessionInput> {
        unused()
    }
    fn disconnect(&self) -> AppResult<()> {
        unused()
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        unused()
    }
    fn resource_info(&self) -> ResourceInfo {
        resource()
    }
}
fn resource() -> ResourceInfo {
    ResourceInfo {
        kind: "controlled".into(),
        version: "unused".into(),
        manifest_sha256: content_hash("unused"),
        available: false,
        detail: "no external request in filesystem test".into(),
    }
}
impl ModelGateway for UnusedPorts {
    fn execute<'a>(
        &'a self,
        _: &'a TaskDispatch,
        _: &'a RequestPlan,
        _: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { unused() })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a Connection,
        _: &'a RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { unused() })
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { unused() })
    }
}
impl LanguageServices for UnusedPorts {
    fn analyze<'a>(&'a self, _: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        Box::pin(async { unused() })
    }
    fn lookup(&self, _: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        unused()
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![resource()]
    }
}
impl CredentialVault for UnusedPorts {
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        unused()
    }
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        unused()
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        unused()
    }
}
fn directory() -> PathBuf {
    let fallback = std::env::temp_dir().join(format!("luna-jsonl-export-{}", uuid::Uuid::new_v4()));
    acceptance_evidence::directory("jsonl-export", &fallback)
        .canonicalize()
        .unwrap()
}
fn export_source() -> HookSourceInput {
    HookSourceInput {
        source_id: "export-source".into(),
        host_instance_id: "controlled-export-broker".into(),
        hook_address_u64: "1".into(),
        context_u64: "0".into(),
        subcontext_u64: "0".into(),
        hook_code: "fixture".into(),
        hook_name: "controlled".into(),
        encoding: "unicode".into(),
        stream_identity_sha256: content_hash("export-stream"),
        identity_extensions_json: "{}".into(),
    }
}
fn add(store: &SqliteStore, n: i64) {
    store
        .ingest_hook_text(
            HookTextInput {
                session_id: "export-session".into(),
                event_id: format!("export-raw-{n}"),
                sequence_u64: (n + 2).to_string(),
                source: export_source(),
                text: format!("原文第{n}句𠮷🦀が。\n改行。"),
                source_language: "ja".into(),
            },
            n + 10,
        )
        .unwrap();
}
fn setup(dir: &Path, n: i64) -> (Arc<ApplicationRuntime>, Arc<SqliteStore>, String) {
    let store = Arc::new(
        SqliteStore::open(
            dir.join("profile.sqlite3"),
            "export-profile",
            "export-server",
            1,
        )
        .unwrap(),
    );
    let session = "export-session".to_string();
    store
        .open_hook_session(
            HookSessionInput {
                session_id: session.clone(),
                game_id: None,
                environment_key: "controlled-export-env".into(),
                target_exe_path: "C:\\controlled.exe".into(),
                windows_pid: 1,
                process_created_u64: "1".into(),
                architecture: "x86".into(),
                broker_instance_id: "controlled-export-broker".into(),
                resource: ResourceSnapshotInput {
                    snapshot_id: "export-core".into(),
                    kind: "core".into(),
                    version: "controlled".into(),
                    manifest_sha256: content_hash("core"),
                    contract_version: "1".into(),
                    metadata_json: "{}".into(),
                },
                target_identity_sha256: content_hash("export-target"),
            },
            2,
        )
        .unwrap();
    store
        .ingest_hook_text_with_policy(
            HookTextInput {
                session_id: session.clone(),
                event_id: "export-candidate".into(),
                sequence_u64: "1".into(),
                source: export_source(),
                text: "候補".into(),
                source_language: "ja".into(),
            },
            false,
            3,
        )
        .unwrap();
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "export-select".into(),
                expected_revision: None,
                command: Command::SelectSource {
                    source_id: "export-source".into(),
                },
            },
            4,
        )
        .unwrap();
    for i in 0..n {
        add(&store, i);
    }
    let ports = Arc::new(UnusedPorts);
    let runtime = ApplicationRuntime::new(
        store.clone(),
        Arc::new(SystemIdentity),
        ports.clone(),
        ports.clone(),
        ports.clone(),
        ports,
    )
    .unwrap();
    (runtime, store, session)
}
fn plan(runtime: &Arc<ApplicationRuntime>, session: &str) -> SessionExportPlan {
    runtime
        .prepare_session_export(SessionExportRequest {
            query: SessionRecordQuery {
                session_id: session.into(),
                dataset: SessionDataset::Body,
                order: SessionRecordOrder::NewestFirst,
                filter: SessionRecordFilter::default(),
                cursor: None,
                page_size: 7,
            },
            privacy_confirmed: true,
        })
        .unwrap()
}
fn pending(dir: &Path) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(".luna-export-"))
        .count()
}
#[test]
fn fixed_range_multi_page_jsonl_is_atomic_unicode_and_hash_correct() {
    let dir = directory();
    let (runtime, store, session) = setup(&dir, 205);
    let plan = plan(&runtime, &session);
    assert_eq!(plan.snapshot.matching_count, "205");
    assert_eq!(plan.order, SessionRecordOrder::OldestFirst);
    for i in 205..209 {
        add(&store, i);
    }
    let destination = dir.join("complete.jsonl");
    let original = b"original user file\n";
    fs::write(&destination, original).unwrap();
    let finished = Arc::new(AtomicBool::new(false));
    let flag = finished.clone();
    let path = destination.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    let observer = std::thread::spawn(move || {
        let mut hashes = std::collections::BTreeSet::from([content_hash(fs::read(&path).unwrap())]);
        tx.send(()).unwrap();
        while !flag.load(Ordering::Acquire) {
            hashes.insert(content_hash(fs::read(&path).unwrap()));
        }
        hashes
    });
    rx.recv().unwrap();
    let result = deliver(&runtime, &plan, &destination, &ExportControl::default());
    finished.store(true, Ordering::Release);
    let observed = observer.join().unwrap();
    let receipt = result.unwrap();
    assert!(
        observed.contains(&content_hash(original)),
        "the independent reader must actually observe the original"
    );
    let bytes = fs::read(&destination).unwrap();
    let hash = content_hash(&bytes);
    assert_eq!(receipt.status, SessionExportStatus::Completed);
    assert_eq!(receipt.record_count, "205");
    assert_eq!(receipt.content_sha256.as_deref(), Some(hash.as_str()));
    assert!(bytes.ends_with(b"\n"));
    let lines = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 206);
    assert_eq!(lines[0]["format"], "luna-session-jsonl");
    assert_eq!(
        lines[0]["plan"]["snapshot"]["highWatermark"],
        plan.snapshot.high_watermark
    );
    for (n, row) in lines[1..].iter().enumerate() {
        assert_eq!(row["kind"], "body");
        assert_eq!(
            row["unit"]["sourceText"],
            format!("原文第{n}句𠮷🦀が。\n改行。")
        );
    }
    let permitted = std::collections::BTreeSet::from([content_hash(original), hash]);
    assert!(
        observed.is_subset(&permitted),
        "reader may observe only complete old or complete new files"
    );
    assert_eq!(pending(&dir), 0);
    runtime
        .finish_session_export(plan.export_id.clone())
        .unwrap();
    assert_eq!(
        runtime
            .session_export_page(plan.export_id, None, 100)
            .unwrap_err()
            .code,
        "export_missing"
    );
    acceptance_evidence::record(
        "schema-two-jsonl-atomic-delivery",
        &[("V2-EXPORT", 213)],
        json!({"record_count":205,"destination":destination,"content_sha256":receipt.content_sha256,"concurrent_reader_hashes":observed}),
    );
}
#[test]
fn cancellation_and_invalid_destination_preserve_original_and_remove_pending() {
    let dir = directory();
    let (runtime, _store, session) = setup(&dir, 1);
    let p = plan(&runtime, &session);
    let destination = dir.join("preserve.jsonl");
    fs::write(&destination, b"preserve exact original").unwrap();
    let original = fs::read(&destination).unwrap();
    let control = ExportControl::default();
    assert!(control.request_cancel());
    let error = deliver(&runtime, &p, &destination, &control).unwrap_err();
    assert_eq!(error.code, "export_cancelled");
    assert_eq!(fs::read(&destination).unwrap(), original);
    assert_eq!(pending(&dir), 0);
    runtime.cancel_session_export(p.export_id.clone()).unwrap();
    assert_eq!(
        runtime
            .session_export_page(p.export_id, None, 100)
            .unwrap_err()
            .code,
        "export_missing"
    );
    let p = plan(&runtime, &session);
    for invalid in [
        dir.join("missing").join("result.jsonl"),
        dir.clone(),
        PathBuf::from("relative.jsonl"),
    ] {
        assert_eq!(
            deliver(&runtime, &p, &invalid, &ExportControl::default())
                .unwrap_err()
                .code,
            "export_destination"
        );
    }
    #[cfg(unix)]
    {
        let link = dir.join("link.jsonl");
        std::os::unix::fs::symlink(&destination, &link).unwrap();
        assert_eq!(
            deliver(&runtime, &p, &link, &ExportControl::default())
                .unwrap_err()
                .code,
            "export_destination"
        );
        assert_eq!(fs::read(&destination).unwrap(), original);
        // This negative fixture is owned by this test. Preserve the real
        // rejection assertions, then remove only the link, so the immutable
        // same-run artifact walk never has to trust a symbolic link.
        fs::remove_file(&link).unwrap();
        assert!(!link.try_exists().unwrap());
    }
    assert_eq!(pending(&dir), 0);
    runtime.cancel_session_export(p.export_id).unwrap();
    acceptance_evidence::record(
        "schema-two-jsonl-cancel-reject",
        &[("V2-EXPORT", 10)],
        json!({"original_content_sha256":content_hash(original),"pending_files":0}),
    );
}

#[cfg(target_os = "macos")]
#[test]
fn actual_limited_volume_write_failure_keeps_original_and_same_plan_retries() {
    let dir = directory();
    let (runtime, _store, session) = setup(&dir, 205);
    let p = plan(&runtime, &session);
    let script = r#"
import sys,pathlib,subprocess,plistlib,os,json
work=pathlib.Path(sys.argv[1]); image=work/'export-failure.dmg';mount=work/'owned-volume';mount.mkdir()
def run(label,args):
 p=subprocess.run(args,capture_output=True);(work/(label+'.stdout')).write_bytes(p.stdout);(work/(label+'.stderr')).write_bytes(p.stderr)
 if p.returncode:raise RuntimeError(label+' failed: '+p.stderr.decode(errors='replace'))
 return p.stdout
run('create',['hdiutil','create','-size','64m','-fs','HFS+','-volname','LunaExportTest','-type','UDIF','-nospotlight',str(image)])
attached=plistlib.loads(run('attach',['hdiutil','attach','-plist','-nobrowse','-owners','off','-mountpoint',str(mount),str(image)]))
try:
 entities=attached['system-entities'];device=next(e['dev-entry'] for e in entities if e.get('dev-entry','').startswith('/dev/disk') and e['dev-entry'][9:].isdigit())
 assert any(e.get('mount-point')==str(mount) for e in entities)
 v=os.statvfs(mount);capacity=v.f_frsize*v.f_blocks;dev=os.stat(mount).st_dev
 assert os.path.ismount(mount) and dev!=os.stat('/').st_dev and dev!=os.stat(work).st_dev and 16*1024**2<capacity<=64*1024**2
 original=mount/'original.jsonl';original.write_bytes(b'original user file\n')
 filler=mount/'owned-filler.bin';total=0
 with filler.open('wb',buffering=0) as f:
  while total<=64*1024**2:
   try:total+=f.write(b'x'*32768)
   except OSError as e:
    assert e.errno==28,e;break
  assert total<=64*1024**2
  os.fsync(f.fileno());f.truncate(max(0,total-65536));os.fsync(f.fileno())
 print(json.dumps(dict(device=device,mount=str(mount),capacity_bytes=capacity,device_identity=dev,filler=str(filler),destination=str(original))))
except BaseException:
 subprocess.run(['diskutil','eject',str(mount)],capture_output=True);raise
"#;
    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .arg(&dir)
        .output()
        .unwrap();
    fs::write(dir.join("limited-volume-setup.stderr"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "isolated volume setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_slice(&output.stdout).unwrap();
    struct Volume {
        device: String,
    }
    impl Drop for Volume {
        fn drop(&mut self) {
            let output = std::process::Command::new("hdiutil")
                .args(["detach", &self.device])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "test-owned volume normal detach failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    let _volume = Volume {
        device: state["device"].as_str().unwrap().into(),
    };
    let mount = PathBuf::from(state["mount"].as_str().unwrap());
    let destination = PathBuf::from(state["destination"].as_str().unwrap());
    let original = fs::read(&destination).unwrap();
    let error = deliver(&runtime, &p, &destination, &ExportControl::default()).unwrap_err();
    assert_eq!(
        error.code, "export_write",
        "actual IO failure must occur during streaming and preserve the original"
    );
    assert_eq!(fs::read(&destination).unwrap(), original);
    assert_eq!(pending(&mount), 0);
    fs::remove_file(state["filler"].as_str().unwrap()).unwrap();
    let receipt = deliver(&runtime, &p, &destination, &ExportControl::default()).unwrap();
    assert_eq!(receipt.record_count, "205");
    assert_eq!(
        receipt.content_sha256,
        Some(content_hash(fs::read(&destination).unwrap()))
    );
    assert_eq!(pending(&mount), 0);
    runtime.finish_session_export(p.export_id).unwrap();
    acceptance_evidence::record(
        "schema-two-jsonl-enospc-retry",
        &[("V2-EXPORT", 8)],
        json!({"owned_volume":state,"actual_error":error.code,"retry_record_count":receipt.record_count,"original_preserved_after_failure":true}),
    );
}
