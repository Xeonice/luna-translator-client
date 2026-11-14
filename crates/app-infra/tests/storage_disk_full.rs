//! STORAGE-08: actual macOS capacity-limited disk image, real Repository writes.
//! A successful run is one storage subcheck, never complete release acceptance.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
#[cfg(target_os = "macos")]
mod macos {
    use super::acceptance_evidence;
    use app_core::{
        contracts::{
            Command as AppCommand, CommandEnvelope, HistorySessionSummary, HistorySourceKind,
        },
        domain::content_hash,
        ports::ProductRepository,
    };
    use app_infra::storage::SqliteStore;
    use rusqlite::{Connection, OpenFlags};
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        fs::{self, File},
        io::Write,
        os::unix::fs::MetadataExt,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };

    const IMAGE_BYTES: u64 = 64 * 1024 * 1024;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }
    fn ensure(condition: bool, message: &str) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.into())
        }
    }
    fn write_report(dir: &Path, value: &Value) {
        fs::write(
            dir.join("receipt.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
    fn execute(dir: &Path, label: &str, program: &str, args: &[&str]) -> Result<Output, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        fs::write(dir.join(format!("{label}.stdout")), &output.stdout)
            .map_err(|e| e.to_string())?;
        fs::write(dir.join(format!("{label}.stderr")), &output.stderr)
            .map_err(|e| e.to_string())?;
        Ok(output)
    }
    fn plist(data: &[u8]) -> Result<Value, String> {
        let mut child = Command::new("python3")
            .args([
                "-c",
                "import json,plistlib,sys;print(json.dumps(plistlib.loads(sys.stdin.buffer.read())))",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("missing plist parser stdin")?
            .write_all(data)
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        ensure(
            output.status.success(),
            &format!(
                "cannot parse attach plist: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )?;
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
    }
    fn capacity(mount: &Path, work: &Path) -> Result<Value, String> {
        let output=Command::new("python3").args(["-c","import json,os,sys;p=sys.argv[1];v=os.statvfs(p);print(json.dumps(dict(device=os.stat(p).st_dev,root_device=os.stat('/').st_dev,work_device=os.stat(sys.argv[2]).st_dev,is_mount=os.path.ismount(p),capacity_bytes=v.f_frsize*v.f_blocks,available_bytes=v.f_frsize*v.f_bavail,block_size=v.f_frsize)))"])
            .arg(mount).arg(work).output().map_err(|e|e.to_string())?;
        ensure(
            output.status.success(),
            "cannot inspect real mounted capacity",
        )?;
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
    }
    struct MountedImage {
        device: String,
        directory: PathBuf,
        attached: bool,
    }
    impl MountedImage {
        fn detach(&mut self) -> Result<(), String> {
            if !self.attached {
                return Ok(());
            }
            let output = if self.device.starts_with("/dev/disk") {
                execute(
                    &self.directory,
                    "detach",
                    "hdiutil",
                    &["detach", &self.device],
                )?
            } else {
                // Installed before parsing attach output: a parser failure can
                // eject only this run's exact mount, without guessing a device.
                execute(
                    &self.directory,
                    "detach",
                    "diskutil",
                    &["eject", &self.device],
                )?
            };
            let mut report: Value = serde_json::from_slice(
                &fs::read(self.directory.join("receipt.json")).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            report["detach"] = json!({"device":self.device,"exit_code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)});
            write_report(&self.directory, &report);
            ensure(
                output.status.success(),
                "normal detach failed; no force detach or user-process changes were attempted",
            )?;
            self.attached = false;
            Ok(())
        }
    }
    impl Drop for MountedImage {
        fn drop(&mut self) {
            if self.attached {
                let _ = self.detach();
            }
        }
    }
    fn manual(operation: &str, text: &str) -> CommandEnvelope {
        CommandEnvelope {
            operation_id: operation.into(),
            expected_revision: None,
            command: AppCommand::ManualInput {
                text: text.into(),
                source_language: "ja".into(),
            },
        }
    }
    fn state(connection: &Connection) -> Result<Value, String> {
        let mut counts = BTreeMap::new();
        for table in [
            "sessions",
            "reading_units",
            "reading_unit_revisions",
            "source_refs",
            "raw_events",
            "raw_event_payloads",
            "capture_sources",
            "operation_receipts",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            counts.insert(table, count);
        }
        let revision: i64 = connection
            .query_row(
                "SELECT commit_revision FROM profile_meta WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut query=connection.prepare("SELECT r.unit_id,r.session_id,r.source_text,r.source_sha256,r.source_byte_length,s.source_kind FROM reading_unit_revisions r JOIN sessions s USING(session_id) ORDER BY r.unit_id,r.text_revision").map_err(|e|e.to_string())?;
        let units = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut query=connection.prepare("SELECT operation_id,request_sha256,commit_revision,result_refs_json FROM operation_receipts ORDER BY operation_id").map_err(|e|e.to_string())?;
        let receipts = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(
            json!({"counts":counts,"commit_revision":revision,"source_units":units,"operation_receipts":receipts}),
        )
    }
    fn run(dir: &Path, report: &mut Value) -> Result<(), String> {
        let mount = dir.join("mount");
        fs::create_dir(&mount).map_err(|e| e.to_string())?;
        let image = dir.join("limited-64m.dmg");
        let volume_name = format!(
            "LunaStorageFull-{}",
            dir.file_name()
                .ok_or("missing isolated run identity")?
                .to_str()
                .ok_or("non-UTF8 run identity")?
        );
        report["stage"] = json!("create_isolated_image");
        write_report(dir, report);
        let created = execute(
            dir,
            "create",
            "hdiutil",
            &[
                "create",
                "-size",
                "64m",
                "-fs",
                "HFS+",
                "-volname",
                &volume_name,
                "-type",
                "UDIF",
                "-nospotlight",
                image.to_str().ok_or("non-UTF8 image path")?,
            ],
        )?;
        if !created.status.success() {
            let message = String::from_utf8_lossy(&created.stderr);
            if message.contains("not permitted") || message.contains("not allowed") {
                report["status"] = json!("blocked");
            }
            return Err(format!(
                "hdiutil could not create isolated image: {}",
                message
            ));
        }
        report["stage"] = json!("attach_isolated_image");
        write_report(dir, report);
        let attached = execute(
            dir,
            "attach",
            "hdiutil",
            &[
                "attach",
                "-plist",
                "-nobrowse",
                "-owners",
                "off",
                "-mountpoint",
                mount.to_str().ok_or("non-UTF8 mount path")?,
                image.to_str().ok_or("non-UTF8 image path")?,
            ],
        )?;
        if !attached.status.success() {
            report["status"] = json!("blocked");
            return Err(format!(
                "hdiutil mounting not allowed or unavailable: {}",
                String::from_utf8_lossy(&attached.stderr)
            ));
        }
        let mut mounted = MountedImage {
            device: mount
                .to_str()
                .ok_or("non-UTF8 owned mount path")?
                .to_owned(),
            directory: dir.to_path_buf(),
            attached: true,
        };
        let attached = plist(&attached.stdout)?;
        let entities = attached["system-entities"]
            .as_array()
            .ok_or("attach returned no device entities")?;
        let device = entities
            .iter()
            .filter_map(|e| e["dev-entry"].as_str())
            .find(|d| {
                d.strip_prefix("/dev/disk")
                    .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            })
            .ok_or("attach did not identify its own disk device")?
            .to_owned();
        // From this point, all returns and panics attempt normal cleanup of only
        // the exact device returned by attaching this run's newly-created image.
        mounted.device = device.clone();
        report["attach"] = attached.clone();
        write_report(dir, report);
        let mounted_entity = entities
            .iter()
            .find(|e| e["mount-point"].as_str() == mount.to_str())
            .ok_or("attach did not mount at the dedicated test path")?;
        let mounted_device = mounted_entity["dev-entry"]
            .as_str()
            .ok_or("mounted volume has no actual device")?;
        ensure(
            mounted_device.starts_with(&device),
            "mounted partition is not part of the attached image device",
        )?;
        let initial_capacity = capacity(&mount, dir)?;
        let volume_device = initial_capacity["device"]
            .as_u64()
            .ok_or("missing stat device identity")?;
        let root_device = initial_capacity["root_device"]
            .as_u64()
            .ok_or("missing root filesystem identity")?;
        let work_device = initial_capacity["work_device"]
            .as_u64()
            .ok_or("missing worktree filesystem identity")?;
        let total = initial_capacity["capacity_bytes"]
            .as_u64()
            .ok_or("missing actual filesystem capacity")?;
        ensure(
            initial_capacity["is_mount"] == true,
            "test mount is not a mount point",
        )?;
        ensure(
            volume_device != root_device && volume_device != work_device,
            "refusing to fill a filesystem shared with root or the worktree",
        )?;
        ensure(
            total > 16 * 1024 * 1024 && total <= IMAGE_BYTES,
            "actual mounted capacity does not match a genuinely limited 64 MiB image",
        )?;
        let df = execute(
            dir,
            "mounted-capacity",
            "df",
            &["-kP", mount.to_str().ok_or("invalid mount path")?],
        )?;
        ensure(
            df.status.success(),
            "df did not confirm the independent mount",
        )?;
        report["stage"] = json!("validated_independent_volume");
        report["initial_capacity"] = initial_capacity;
        report["image"] = json!(image);
        report["image_device"] = json!(device);
        report["volume_device"] = json!(mounted_device);
        write_report(dir, report);

        let profile = mount.join("isolated-profile.sqlite3");
        let store = SqliteStore::open(&profile, "disk-full-profile", "before-full", 100)
            .map_err(|e| format!("{}: {}", e.code, e.message))?;
        let baseline = store
            .dispatch(
                manual("committed-before-full", "満杯になる前の本文𠮷🦀"),
                101,
            )
            .map_err(|e| e.message)?;
        let baseline_unit = store
            .unit(
                baseline.result_id.clone().ok_or("missing committed unit")?,
                1,
            )
            .map_err(|e| e.message)?;
        ensure(
            baseline_unit.source_summary.as_ref()
                == Some(&HistorySessionSummary {
                    session_id: baseline_unit.session_id.clone(),
                    game_id: baseline_unit.game_id.clone(),
                    source_kind: HistorySourceKind::Manual,
                    state: "active".into(),
                    started_at_ms: 101.0,
                    ended_at_ms: None,
                }),
            "pre-full reading object did not expose its authoritative live session",
        )?;
        store.checkpoint().map_err(|e| e.message)?;
        let observer = Connection::open_with_flags(&profile, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
        observer
            .execute_batch("PRAGMA query_only=ON;")
            .map_err(|e| e.to_string())?;
        let before = state(&observer)?;
        let sqlite_identity: (String, String) = observer
            .query_row("SELECT sqlite_version(),sqlite_source_id()", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|e| e.to_string())?;
        let native_probe = mount.join("native-sqlite-full-probe.sqlite3");
        let probe = Connection::open(&native_probe).map_err(|e| e.to_string())?;
        probe
            .execute_batch("CREATE TABLE io_probe(payload BLOB);")
            .map_err(|e| e.to_string())?;
        report["before"] = before.clone();
        report["sqlite_identity"] = json!(sqlite_identity);
        write_report(dir, report);

        // Check both directory and newly-opened file device IDs before every
        // write. The hard byte bound is independent of df and cannot fill root.
        ensure(
            fs::metadata(&mount).map_err(|e| e.to_string())?.dev() == volume_device,
            "test mount changed before filling",
        )?;
        let filler = mount.join("owned-filler.bin");
        let mut file = File::create(&filler).map_err(|e| e.to_string())?;
        ensure(
            file.metadata().map_err(|e| e.to_string())?.dev() == volume_device,
            "filler did not open on the validated image filesystem",
        )?;
        let buffer = vec![0x73_u8; 1024 * 1024];
        let mut attempted = 0_u64;
        let fill_error = loop {
            ensure(
                fs::metadata(&mount).map_err(|e| e.to_string())?.dev() == volume_device,
                "mount identity changed; refusing further filler writes",
            )?;
            ensure(
                attempted <= IMAGE_BYTES + buffer.len() as u64,
                "filler exceeded the independent 64 MiB safety bound",
            )?;
            attempted += buffer.len() as u64;
            if let Err(error) = file.write_all(&buffer) {
                break error;
            }
        };
        let _ = file.sync_all();
        drop(file);
        ensure(
            fill_error.raw_os_error() == Some(28),
            &format!("real filesystem did not return ENOSPC: {fill_error}"),
        )?;
        let full_capacity = capacity(&mount, dir)?;
        report["stage"] = json!("actual_enospc");
        report["full_capacity"] = full_capacity;
        report["filler_bytes"] = json!(fs::metadata(&filler).map_err(|e| e.to_string())?.len());
        report["filesystem_error"] =
            json!({"raw_os_error":fill_error.raw_os_error(),"message":fill_error.to_string()});
        write_report(dir, report);

        let native_error = match probe.execute(
            "INSERT INTO io_probe VALUES(zeroblob(?1))",
            [2 * 1024 * 1024],
        ) {
            Ok(_) => {
                return Err(
                    "actual SQLite unexpectedly wrote successfully on the purported full image"
                        .into(),
                );
            }
            Err(error) => error,
        };
        match native_error {
            rusqlite::Error::SqliteFailure(error, message) => {
                report["native_sqlite_probe_error"] = json!({"code":format!("{:?}",error.code),"extended_code":error.extended_code,"message":message});
                write_report(dir, report);
                ensure(
                    error.code == rusqlite::ErrorCode::DiskFull,
                    "actual SQLite did not report SQLITE_FULL on the real full filesystem",
                )?;
            }
            other => {
                return Err(format!(
                    "native SQLite full probe failed for an unrelated reason: {other}"
                ));
            }
        }
        let payload = "限容の原文🦀".repeat(65536);
        let operation = "retry-same-operation-after-full";
        let error = match store.dispatch(manual(operation, &payload), 102) {
            Ok(_) => {
                return Err(
                    "real Repository unexpectedly committed on the purported full image".into(),
                );
            }
            Err(error) => error,
        };
        report["repository_error"] =
            json!({"code":error.code,"stage":error.stage,"message":error.message});
        write_report(dir, report);
        ensure(
            error.code == "storage_failure" && error.message.contains("database or disk is full"),
            "production Repository did not expose the actual SQLite full failure",
        )?;
        let after_failure = state(&observer)?;
        report["after_failure"] = after_failure.clone();
        write_report(dir, report);
        ensure(
            before == after_failure,
            "disk-full transaction changed source rows, operation receipts, or business revision",
        )?;
        ensure(
            store
                .unit(baseline_unit.unit_id.clone(), 1)
                .map_err(|e| e.message)?
                == baseline_unit,
            "disk-full failure changed the committed reading object",
        )?;
        ensure(
            store.snapshot().map_err(|e| e.message)?.commit_revision == baseline.revision,
            "disk-full failure advanced Repository revision",
        )?;
        ensure(
            probe
                .query_row::<i64, _, _>("SELECT count(*) FROM io_probe", [], |r| r.get(0))
                .map_err(|e| e.to_string())?
                == 0,
            "failed native probe left a partial row",
        )?;

        ensure(
            fs::metadata(&mount).map_err(|e| e.to_string())?.dev() == volume_device,
            "mount identity changed; refusing filler removal",
        )?;
        fs::remove_file(&filler).map_err(|e| e.to_string())?;
        report["capacity_after_release"] = capacity(&mount, dir)?;
        write_report(dir, report);
        let recovered = store
            .dispatch(manual(operation, &payload), 102)
            .map_err(|e| format!("recovery failed {}: {}", e.code, e.message))?;
        ensure(
            !recovered.replayed && recovered.revision == baseline.revision + 1,
            "failed operation must commit once, rather than replay a nonexistent success",
        )?;
        let replay = store
            .dispatch(manual(operation, &payload), 103)
            .map_err(|e| e.message)?;
        ensure(
            replay.replayed
                && replay.revision == recovered.revision
                && replay.result_id == recovered.result_id,
            "same operation retry created a second unit or receipt",
        )?;
        let result_id = recovered
            .result_id
            .clone()
            .ok_or("retry has no actual reading result")?;
        let unit = store.unit(result_id.clone(), 1).map_err(|e| e.message)?;
        ensure(
            unit.source_summary.as_ref()
                == Some(&HistorySessionSummary {
                    session_id: unit.session_id.clone(),
                    game_id: unit.game_id.clone(),
                    source_kind: HistorySourceKind::Manual,
                    state: "active".into(),
                    started_at_ms: 102.0,
                    ended_at_ms: None,
                }),
            "recovered reading object did not expose its authoritative live session",
        )?;
        ensure(
            unit.source_text == payload,
            "recovered Repository input was truncated or replaced",
        )?;
        let after_recovery = state(&observer)?;
        ensure(
            after_recovery["counts"]["reading_units"] == 2
                && after_recovery["counts"]["sessions"] == 2
                && after_recovery["counts"]["operation_receipts"] == 2,
            "recovered multi-table unit/receipt cardinality is not exactly two",
        )?;
        let integrity: String = observer
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        ensure(
            integrity == "ok",
            "database did not remain intact after full/recovery",
        )?;
        let violations: i64 = observer
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        ensure(
            violations == 0,
            "foreign-key integrity broke after full/recovery",
        )?;
        report["after_recovery"] = after_recovery;
        report["recovered_receipt"] = json!(recovered);
        report["replay_receipt"] = json!(replay);
        report["recovered_source_sha256"] = json!(content_hash(&payload));
        write_report(dir, report);
        drop(observer);
        drop(probe);
        drop(store);
        let reopened = SqliteStore::open(&profile, "disk-full-profile", "after-restart", 200)
            .map_err(|e| e.message)?;
        let mut expected_recovered = unit.clone();
        let recovered_summary = expected_recovered
            .source_summary
            .as_mut()
            .ok_or("missing recovered session summary")?;
        recovered_summary.state = "crashed".into();
        recovered_summary.ended_at_ms = Some(200.0);
        ensure(
            reopened.unit(result_id, 1).map_err(|e| e.message)? == expected_recovered,
            "reopen changed the recovered reading object beyond session recovery",
        )?;
        let mut expected_baseline = baseline_unit.clone();
        let baseline_summary = expected_baseline
            .source_summary
            .as_mut()
            .ok_or("missing pre-full session summary")?;
        baseline_summary.state = "crashed".into();
        baseline_summary.ended_at_ms = Some(200.0);
        ensure(
            reopened
                .unit(baseline_unit.unit_id.clone(), 1)
                .map_err(|e| e.message)?
                == expected_baseline,
            "reopen changed the pre-full reading object beyond session recovery",
        )?;
        ensure(
            reopened.snapshot().map_err(|e| e.message)?.commit_revision == 2,
            "reopen changed business revision or lost receipts",
        )?;
        drop(reopened);
        report["stage"] = json!("detaching_test_image");
        write_report(dir, report);
        mounted.detach()?;
        ensure(
            fs::metadata(&mount).map_err(|e| e.to_string())?.dev() == work_device,
            "normal detach returned success but the test image is still mounted",
        )?;
        // detach appends evidence; read it back before declaring the subcheck.
        *report =
            serde_json::from_slice(&fs::read(dir.join("receipt.json")).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        report["status"] = json!("passed");
        report["stage"] = json!("complete");
        report["check"] = json!("STORAGE-08");
        report["full_storage_acceptance"] = json!("not_issued");
        write_report(dir, report);
        acceptance_evidence::record(
            "actual-limited-volume-enospc",
            &[("STORAGE-08", 1)],
            json!({"receipt":dir.join("receipt.json"),"device":device,"observed_enospc":true}),
        );
        Ok(())
    }

    #[test]
    fn real_isolated_disk_full_rolls_back_and_retries_the_same_repository_operation() {
        let run_id = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let fallback = root().join(".build/storage-disk-full").join(run_id);
        let dir = acceptance_evidence::directory("disk-full", &fallback);
        let mut report = json!({"schema_version":1,"kind":"real_capacity_limited_storage_subcheck","status":"running","stage":"created_test_directory","image_limit_bytes":IMAGE_BYTES,"product_release_claim":"not_issued"});
        write_report(&dir, &report);
        if let Err(error) = run(&dir, &mut report) {
            let persisted: Value =
                serde_json::from_slice(&fs::read(dir.join("receipt.json")).unwrap()).unwrap();
            if let Some(detach) = persisted.get("detach") {
                report["detach"] = detach.clone();
            }
            if report["status"] != "blocked" {
                report["status"] = json!("failed");
            }
            report["error"] = json!(error);
            write_report(&dir, &report);
            panic!(
                "STORAGE-08 {}: {}; evidence={}",
                report["status"],
                error,
                dir.join("receipt.json").display()
            );
        }
        println!(
            "STORAGE_DISK_FULL_RECEIPT {}",
            dir.join("receipt.json").display()
        );
    }
}
