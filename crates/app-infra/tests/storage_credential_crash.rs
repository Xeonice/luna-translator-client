//! Actual macOS Keychain + production SQLite/runtime recovery across process death.
//! All Keychain items belong to fresh, isolated test-only profile namespaces.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
#[cfg(target_os = "macos")]
mod macos {
    use super::acceptance_evidence;
    use app_core::{
        application::{request_plan::RequestPlan, runtime::ApplicationRuntime},
        contracts::*,
        domain::{TaskDispatch, capture::*},
        ports::{IdentitySource, ProductRepository, runtime::*},
    };
    use app_infra::{credentials::SystemVault, storage::SqliteStore};
    use serde_json::{Value, json};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    struct Clock {
        profile: String,
        time: AtomicI64,
    }
    impl IdentitySource for Clock {
        fn new_id(&self, prefix: &str) -> String {
            format!("{prefix}-{}", self.profile)
        }
        fn now_ms(&self) -> i64 {
            self.time.fetch_add(1, Ordering::SeqCst)
        }
    }
    struct UnusedPorts;
    fn unused() -> AppError {
        AppError::new(
            "unexpected_test_port",
            "storage_credential_test",
            "credential recovery must not invoke Hook, model, or language services",
        )
    }
    impl HookBackend for UnusedPorts {
        fn targets(&self) -> AppResult<TargetCandidates> {
            Err(unused())
        }
        fn associate(&self, _: AssociateRequest) -> AppResult<HookSessionInput> {
            Err(unused())
        }
        fn disconnect(&self) -> AppResult<()> {
            Err(unused())
        }
        fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
            Err(unused())
        }
        fn resource_info(&self) -> ResourceInfo {
            panic!("unused Hook port was called during Keychain test")
        }
    }
    impl ModelGateway for UnusedPorts {
        fn execute<'a>(
            &'a self,
            _: &'a TaskDispatch,
            _: &'a RequestPlan,
            _: Arc<dyn Fn(String) + Send + Sync>,
        ) -> PortFuture<'a, String> {
            Box::pin(async { Err(unused()) })
        }
        fn test_request<'a>(
            &'a self,
            _: &'a Connection,
            _: &'a RequestPlan,
            _: Option<&'a str>,
        ) -> PortFuture<'a, String> {
            Box::pin(async { Err(unused()) })
        }
        fn discover_models<'a>(
            &'a self,
            _: &'a Connection,
            _: Option<&'a str>,
        ) -> PortFuture<'a, Vec<String>> {
            Box::pin(async { Err(unused()) })
        }
    }
    impl LanguageServices for UnusedPorts {
        fn analyze<'a>(&'a self, _: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
            Box::pin(async { Err(unused()) })
        }
        fn lookup(&self, _: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
            Err(unused())
        }
        fn resources(&self) -> Vec<ResourceInfo> {
            panic!("unused language port was called during Keychain test")
        }
    }
    fn ready_and_wait(directory: &Path, stage: &str) -> ! {
        fs::write(
            directory.join("barrier.json"),
            serde_json::to_vec(&json!({"stage":stage,"pid":std::process::id()})).unwrap(),
        )
        .unwrap();
        loop {
            thread::sleep(Duration::from_millis(50));
        }
    }
    struct BarrierVault {
        actual: SystemVault,
        stage: String,
        directory: PathBuf,
    }
    impl CredentialVault for BarrierVault {
        fn put(&self, id: &str, secret: &[u8]) -> AppResult<()> {
            if self.stage == "after_pending" {
                ready_and_wait(&self.directory, &self.stage);
            }
            self.actual.put(id, secret)?;
            if self.stage == "after_vault" {
                ready_and_wait(&self.directory, &self.stage);
            }
            Ok(())
        }
        fn get(&self, id: &str) -> AppResult<Vec<u8>> {
            self.actual.get(id)
        }
        fn delete(&self, id: &str) -> AppResult<()> {
            self.actual.delete(id)
        }
    }
    fn runtime(
        store: Arc<SqliteStore>,
        profile: &str,
        vault: Arc<dyn CredentialVault>,
    ) -> Arc<ApplicationRuntime> {
        ApplicationRuntime::new(
            store,
            Arc::new(Clock {
                profile: profile.into(),
                time: AtomicI64::new(200),
            }),
            Arc::new(UnusedPorts),
            Arc::new(UnusedPorts),
            Arc::new(UnusedPorts),
            vault,
        )
        .unwrap()
    }
    struct OwnedItem {
        vault: SystemVault,
        id: String,
    }
    impl Drop for OwnedItem {
        fn drop(&mut self) {
            let _ = self.vault.delete(&self.id);
        }
    }
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn credential_crash_child_entry() {
        let Ok(stage) = std::env::var("FP_STORAGE_CREDENTIAL_CRASH_STAGE") else {
            return;
        };
        let directory =
            PathBuf::from(std::env::var_os("FP_STORAGE_CREDENTIAL_CRASH_DIRECTORY").unwrap());
        let profile = std::env::var("FP_STORAGE_CREDENTIAL_CRASH_PROFILE").unwrap();
        let secret = std::env::var("FP_STORAGE_CREDENTIAL_CRASH_DUMMY_SECRET").unwrap();
        let store = Arc::new(
            SqliteStore::open(directory.join("profile.sqlite3"), &profile, "child", 100).unwrap(),
        );
        let vault = Arc::new(BarrierVault {
            actual: SystemVault::new(&profile),
            stage: stage.clone(),
            directory: directory.clone(),
        });
        let app = runtime(store, &profile, vault);
        if stage == "before_pending" {
            ready_and_wait(&directory, &stage);
        }
        let reference=app.store_credential(CredentialInput{label:"Isolated recovery test only".into(),secret}).expect("real isolated Keychain item must be writable; unavailable service blocks this actual case");
        assert_eq!(reference.credential_id, format!("credential-{profile}"));
        assert_eq!(stage, "after_active");
        ready_and_wait(&directory, &stage);
    }

    #[test]
    fn real_keychain_and_sqlite_crash_boundaries_recover_without_plaintext_or_active_secret_loss() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let run = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let fallback = root.join(".build/storage-keychain-crashes").join(&run);
        let out = acceptance_evidence::directory("keychain-crashes", &fallback);
        let mut results = Vec::new();
        for stage in [
            "before_pending",
            "after_pending",
            "after_vault",
            "after_active",
        ] {
            let directory = out.join(stage);
            fs::create_dir(&directory).unwrap();
            let profile = format!("isolated-storage-test-{run}-{stage}");
            let id = format!("credential-{profile}");
            let dummy = format!("not-a-real-service-key-{run}-{stage}");
            let cleanup = OwnedItem {
                vault: SystemVault::new(&profile),
                id: id.clone(),
            };
            assert!(
                cleanup.vault.get(&id).is_err(),
                "random test namespace must begin without an existing secret"
            );
            let log = fs::File::create(directory.join("child.log")).unwrap();
            let mut child = OwnedChild(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        &format!(
                            "{}::credential_crash_child_entry",
                            module_path!()
                                .split_once("::")
                                .map(|(_, suffix)| suffix)
                                .unwrap()
                        ),
                        "--nocapture",
                    ])
                    .env("FP_STORAGE_CREDENTIAL_CRASH_STAGE", stage)
                    .env("FP_STORAGE_CREDENTIAL_CRASH_DIRECTORY", &directory)
                    .env("FP_STORAGE_CREDENTIAL_CRASH_PROFILE", &profile)
                    .env("FP_STORAGE_CREDENTIAL_CRASH_DUMMY_SECRET", &dummy)
                    .stdout(Stdio::from(log.try_clone().unwrap()))
                    .stderr(Stdio::from(log))
                    .spawn()
                    .unwrap(),
            );
            let started = Instant::now();
            while !directory.join("barrier.json").exists() {
                if let Some(status) = child.0.try_wait().unwrap() {
                    panic!(
                        "real Keychain subprocess stopped before its actual barrier ({status}); see {}",
                        directory.join("child.log").display()
                    );
                }
                assert!(
                    started.elapsed() < Duration::from_secs(15),
                    "isolated Keychain subprocess could not reach {stage}; native access unavailable or blocked"
                );
                thread::sleep(Duration::from_millis(10));
            }
            let barrier: Value =
                serde_json::from_slice(&fs::read(directory.join("barrier.json")).unwrap()).unwrap();
            assert_eq!(barrier["stage"], stage);
            let expected_secret_before = matches!(stage, "after_vault" | "after_active");
            let actual_before = cleanup.vault.get(&id);
            if expected_secret_before {
                assert_eq!(actual_before.unwrap(), dummy.as_bytes());
            } else {
                assert!(actual_before.is_err());
            }
            child.0.kill().unwrap();
            let exit = child.0.wait().unwrap();
            assert!(
                !exit.success(),
                "fault child must actually terminate abnormally"
            );
            let path = directory.join("profile.sqlite3");
            let store = Arc::new(SqliteStore::open(&path, &profile, "recovery", 300).unwrap());
            let metadata_before = store.snapshot().unwrap().credentials;
            if stage == "before_pending" {
                assert!(metadata_before.is_empty());
            } else {
                assert_eq!(metadata_before.len(), 1);
                assert_eq!(
                    metadata_before[0].state,
                    if stage == "after_active" {
                        "active"
                    } else {
                        "pending"
                    }
                );
            }
            let app = runtime(
                store.clone(),
                &profile,
                Arc::new(SystemVault::new(&profile)),
            );
            app.recover_credentials().unwrap();
            let metadata_after = store.snapshot().unwrap().credentials;
            let secret_after = cleanup.vault.get(&id);
            if stage == "after_active" {
                assert_eq!(secret_after.unwrap(), dummy.as_bytes());
                assert_eq!(metadata_after[0].state, "active");
            } else {
                assert!(
                    secret_after.is_err(),
                    "pending-orphan secret must be removed from the actual Keychain"
                );
                if stage != "before_pending" {
                    assert_eq!(metadata_after[0].state, "cleanup_pending");
                    assert!(metadata_after[0].lifecycle_revision >= 2);
                }
            }
            assert!(
                store.pending_credentials().unwrap().is_empty(),
                "recovery must not leave dangling cleanup work"
            );
            drop(app);
            drop(store);
            for suffix in ["", "-wal", "-shm"] {
                let file = PathBuf::from(format!("{}{suffix}", path.display()));
                if file.is_file() {
                    let bytes = fs::read(file).unwrap();
                    assert!(
                        !bytes
                            .windows(dummy.len())
                            .any(|part| part == dummy.as_bytes()),
                        "database artifacts must never contain a plaintext secret"
                    );
                }
            }
            let child_log = fs::read(directory.join("child.log")).unwrap();
            assert!(
                !child_log
                    .windows(dummy.len())
                    .any(|part| part == dummy.as_bytes()),
                "child output must never expose the secret"
            );
            cleanup.vault.delete(&id).unwrap();
            assert!(
                cleanup.vault.get(&id).is_err(),
                "test cleanup must remove only its own Keychain item"
            );
            results.push(json!({"stage":stage,"status":"passed","actual_process_killed":true,"exit":exit.to_string(),"secret_present_before_kill":expected_secret_before,"secret_present_after_recovery":stage=="after_active","metadata_before":metadata_before,"metadata_after":metadata_after,"plaintext_absent_in_sqlite_and_logs":true,"owned_keychain_item_cleaned":true}));
        }
        let report = json!({"schema_version":1,"kind":"real_keychain_sqlite_process_crash_recovery","status":"passed","full_storage_acceptance":"not_issued","cases":results,"secret_bytes_in_receipt":false,"namespace_prefix":"com.lunatranslator.reader.credentials.isolated-storage-test-","source_scope":"actual ApplicationRuntime store_credential/recover_credentials and SystemVault; unused unrelated ports never supply success"});
        fs::write(
            out.join("receipt.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!(
            "STORAGE_KEYCHAIN_CRASH_RECEIPT {}",
            out.join("receipt.json").display()
        );
        acceptance_evidence::record(
            "actual-keychain-crash-boundaries",
            &[("STORAGE-10", 4)],
            serde_json::json!({"receipt":out.join("receipt.json"),"stages":4}),
        );
    }
}
