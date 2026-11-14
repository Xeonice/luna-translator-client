//! Actual process-kill boundaries. The middle-rebuild case is explicitly a SQL transaction probe;
//! the committed-open case exercises the real SqliteStore migration host.
use app_core::{domain::content_hash, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use rusqlite::Connection;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const OLD: &str = include_str!("fixtures/http-schema2-original.sql");
fn dir() -> PathBuf {
    let p = std::env::var_os("FP_STORAGE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("http-crash-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn old(path: &Path) {
    let c = Connection::open(path).unwrap();
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    c.execute_batch(OLD).unwrap();
}
fn state(path: &Path) -> (u32, String, u32) {
    let c = Connection::open(path).unwrap();
    (
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .unwrap(),
        c.query_row("SELECT content_hash FROM config_revision_seals", [], |r| {
            r.get(0)
        })
        .unwrap(),
        c.query_row("SELECT count(*) FROM connections", [], |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn controlled_child() {
    let Ok(mode) = std::env::var("LUNA_HTTP_CRASH_MODE") else {
        return;
    };
    let path = PathBuf::from(std::env::var_os("LUNA_HTTP_CRASH_PATH").unwrap());
    let ready = PathBuf::from(std::env::var_os("LUNA_HTTP_CRASH_READY").unwrap());
    if mode == "mid-sql" {
        let c = Connection::open(&path).unwrap();
        c.execute_batch("PRAGMA foreign_keys=OFF;BEGIN IMMEDIATE;")
            .unwrap();
        let ddl = include_str!("../src/storage/migrations/003_http_service_identity.sql");
        let prefix = ddl
            .split_once("ALTER TABLE connections_new RENAME TO connections;")
            .unwrap()
            .0;
        c.execute_batch(prefix).unwrap();
        std::fs::write(
            &ready,
            b"actual dropped FK target; transaction not committed",
        )
        .unwrap();
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
    let store = SqliteStore::open(&path, "legacy-http-profile", "committed-child", 100).unwrap();
    assert!(store.snapshot().unwrap().configuration.applied.is_some());
    std::fs::write(&ready, b"real SqliteStore::open migration committed").unwrap();
    loop {
        std::thread::park_timeout(Duration::from_secs(1));
    }
}
fn child(mode: &str, path: &Path, ready: &Path) -> std::process::Child {
    let module = module_path!();
    let suffix = module
        .split_once("::")
        .map(|(_, s)| format!("{s}::"))
        .unwrap_or_default();
    let child_name = format!("{suffix}controlled_child");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &child_name, "--nocapture"])
        .env("LUNA_HTTP_CRASH_MODE", mode)
        .env("LUNA_HTTP_CRASH_PATH", path)
        .env("LUNA_HTTP_CRASH_READY", ready)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.is_file() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "child did not reach actual boundary"
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "child failed before boundary"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    child
}
fn emit(case: &str, detail: serde_json::Value) {
    if let Some(output) = std::env::var_os("FP_STORAGE_OUTPUT") {
        use std::io::Write;
        let dir = PathBuf::from(output).join("components/v3-delta");
        std::fs::create_dir_all(&dir).unwrap();
        let value = serde_json::json!({"kind":"http_storage_v3_supporting_case","case":case,"run_id":std::env::var("FP_STORAGE_RUN_ID").unwrap(),"source_tree_sha256":std::env::var("FP_STORAGE_SOURCE_SHA256").unwrap(),"execution_nonce":std::env::var("FP_STORAGE_EXECUTION_NONCE").unwrap(),"completed_at_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),"assertions":4,"detail":detail});
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(dir.join(format!("{case}.json")))
            .unwrap();
        f.write_all(&serde_json::to_vec_pretty(&value).unwrap())
            .unwrap();
        f.sync_all().unwrap();
    }
}
fn snapshot(path: &Path) -> serde_json::Value {
    serde_json::json!({"path":path.canonicalize().unwrap(),"sha256":content_hash(std::fs::read(path).unwrap())})
}
#[test]
fn kill_inside_actual_sql_rebuild_rolls_back_dropped_fk_target_and_original_seal() {
    let d = dir();
    let path = d.join("mid.sqlite3");
    old(&path);
    let before = state(&path);
    let mut p = child("mid-sql", &path, &d.join("ready"));
    p.kill().unwrap();
    assert!(!p.wait().unwrap().success());
    assert_eq!(state(&path), before);
    let c = Connection::open(&path).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='connections_new'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    drop(c);
    emit(
        "crash-mid-sql",
        serde_json::json!({"database":snapshot(&path),"scope":"real SQLite migration body midpoint, not a production host failpoint"}),
    );
}
#[test]
fn kill_after_real_store_migration_commit_recovers_schema_three_and_old_immutable_data() {
    let d = dir();
    let path = d.join("committed.sqlite3");
    old(&path);
    let original = state(&path);
    let mut p = child("post-host", &path, &d.join("ready"));
    p.kill().unwrap();
    assert!(!p.wait().unwrap().success());
    let store = SqliteStore::open(&path, "legacy-http-profile", "recovered", 200).unwrap();
    let after = state(&path);
    // The current host now commits schema4. Its schema3 migration stamp must
    // still survive; original host3 crash assertions run on the frozen host.
    assert_eq!(after.0, 5);
    let ledger = Connection::open(&path).unwrap();
    assert_eq!(
        ledger
            .query_row(
                "SELECT migration_sha256 FROM schema_migrations WHERE version=3",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        app_infra::storage::http_service_identity_migration_sha256()
    );
    assert_eq!(
        ledger
            .query_row(
                "SELECT migration_sha256 FROM schema_migrations WHERE version=4",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        app_infra::storage::service_graph_migration_sha256()
    );
    drop(ledger);
    assert_eq!((after.1, after.2), (original.1, original.2));
    let snapshot_path = d.join("recovered.sqlite3");
    store.backup(&snapshot_path).unwrap();
    emit(
        "crash-after-host",
        serde_json::json!({"database":snapshot(&snapshot_path),"scope":"actual current Store4 migration commit and OS process kill; original schema3 ledger and immutable data retained"}),
    );
}
#[test]
fn preupgrade_backup_collision_blocks_migration_without_changing_original_bytes() {
    let d = dir();
    let path = d.join("blocked.sqlite3");
    old(&path);
    let before = std::fs::read(&path).unwrap();
    let backup = d.join("blocked.sqlite3.pre-v5-100.sqlite3");
    std::fs::write(&backup, b"preserved older artifact").unwrap();
    let error = match SqliteStore::open(&path, "legacy-http-profile", "blocked", 100) {
        Ok(_) => panic!("must reject overwrite"),
        Err(e) => e,
    };
    assert_eq!(error.code, "migration_backup_failed");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read(&backup).unwrap(), b"preserved older artifact");
    assert_eq!(state(&path).0, 2);
    emit(
        "backup-collision",
        serde_json::json!({"database":snapshot(&path),"backup":snapshot(&backup)}),
    );
}

#[test]
fn old_v2_reader_actually_rejects_current_three_without_mutating_database() {
    let manifest_path=PathBuf::from(std::env::var_os("LUNA_HTTP_OLD_READER_MANIFEST").expect("required precondition: frozen original v2 reader manifest; missing artifact is blocked, not skip"));
    let manifest_bytes = std::fs::read(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["kind"], "actual_old_sqlite_store_reader_v2");
    assert_eq!(manifest["schema_version"], 2);
    let binary = PathBuf::from(manifest["binary_path"].as_str().unwrap());
    assert_eq!(
        content_hash(std::fs::read(&binary).unwrap()),
        manifest["binary_sha256"]
    );
    let source = PathBuf::from(manifest["source_root"].as_str().unwrap());
    for row in manifest["source_files"].as_array().unwrap() {
        assert_eq!(
            content_hash(std::fs::read(source.join(row["path"].as_str().unwrap())).unwrap()),
            row["sha256"]
        );
    }
    let d = dir();
    let path = d.join("new-three.sqlite3");
    let store = SqliteStore::open(d.join("writer.sqlite3"), "current-three", "server", 1).unwrap();
    store.backup(&path).unwrap();
    drop(store);
    let before = std::fs::read(&path).unwrap();
    let result = Command::new(&binary)
        .arg(&path)
        .arg("current-three")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let actual: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(actual["error_code"], "unsupported_schema");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    emit(
        "old-client-rejects-three",
        serde_json::json!({"database":snapshot(&path),"old_reader_manifest":{"path":manifest_path,"sha256":content_hash(manifest_bytes)},"actual_error":actual,"binary_sha256":manifest["binary_sha256"]}),
    );
}
#[test]
fn current_three_profile_hash_and_structure_errors_reject_readonly_without_writing() {
    for kind in ["profile", "hash", "structure", "future"] {
        let d = dir();
        let path = d.join("rejected.sqlite3");
        let store =
            SqliteStore::open(d.join("writer.sqlite3"), "actual-profile", "server", 1).unwrap();
        store.backup(&path).unwrap();
        drop(store);
        let c = Connection::open(&path).unwrap();
        match kind {
            "hash" => {
                c.execute(
                    "UPDATE schema_migrations SET migration_sha256=?1 WHERE version=3",
                    ["0".repeat(64)],
                )
                .unwrap();
            }
            "structure" => {
                c.execute(
                    "ALTER TABLE provider_preset_versions ADD COLUMN unknown_field TEXT",
                    [],
                )
                .unwrap();
            }
            "future" => {
                c.execute(
                    "UPDATE schema_migrations SET version=99 WHERE version=3",
                    [],
                )
                .unwrap();
            }
            _ => {}
        }
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);PRAGMA journal_mode=DELETE")
            .unwrap();
        drop(c);
        let before = std::fs::read(&path).unwrap();
        let error = match SqliteStore::open(
            &path,
            if kind == "profile" {
                "wrong-profile"
            } else {
                "actual-profile"
            },
            "rejected",
            100,
        ) {
            Ok(_) => panic!("invalid startup must fail"),
            Err(e) => e,
        };
        assert_eq!(
            error.code,
            match kind {
                "profile" => "profile_mismatch",
                "hash" => "migration_mismatch",
                "structure" => "schema_contract_mismatch",
                _ => "unsupported_schema",
            }
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        if kind == "profile" {
            emit(
                "profile-mismatch-reject",
                serde_json::json!({"database":snapshot(&path),"actual_error":error.code}),
            );
        }
    }
}
