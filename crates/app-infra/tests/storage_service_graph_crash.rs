//! Actual process/backup boundaries. Mid-DDL SQL rollback is the production-host cfg(test) unit.
use app_core::{domain::content_hash, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use rusqlite::Connection;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("graph-crash-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn original_three(path: &Path) {
    let manifest = PathBuf::from(
        std::env::var_os("FP_STORAGE_OLD_STORE3_READER_MANIFEST")
            .expect("frozen actual Store3 reader prerequisite missing; blocked"),
    );
    let m: Value = serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
    let binary = PathBuf::from(m["binary_path"].as_str().unwrap());
    assert_eq!(
        content_hash(std::fs::read(&binary).unwrap()),
        m["binary_sha256"]
    );
    let output = Command::new(binary)
        .arg(path)
        .arg("profile")
        .arg("accept")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
}
#[test]
fn controlled_child() {
    let Ok(path) = std::env::var("LUNA_GRAPH_CRASH_PATH") else {
        return;
    };
    let ready = std::env::var("LUNA_GRAPH_CRASH_READY").unwrap();
    let store = SqliteStore::open(path, "profile", "graph-child", 100).unwrap();
    store.snapshot().unwrap();
    std::fs::write(ready, b"actual host4 committed and snapshot read").unwrap();
    loop {
        std::thread::park_timeout(Duration::from_secs(1));
    }
}
#[test]
fn real_backup_collision_preserves_original_three_bytes_and_existing_backup() {
    let d = dir();
    let p = d.join("three.sqlite3");
    original_three(&p);
    let before = std::fs::read(&p).unwrap();
    let backup = p.with_file_name("three.sqlite3.pre-v5-100.sqlite3");
    std::fs::write(&backup, b"preserve-existing-evidence").unwrap();
    let error = match SqliteStore::open(&p, "profile", "four", 100) {
        Ok(_) => panic!("backup collision must refuse"),
        Err(e) => e,
    };
    assert_eq!(error.code, "migration_backup_failed");
    assert_eq!(std::fs::read(&p).unwrap(), before);
    assert_eq!(
        std::fs::read(backup).unwrap(),
        b"preserve-existing-evidence"
    );
}
#[test]
fn killed_process_after_real_host_commit_reopens_four_and_retains_three_backup() {
    let d = dir();
    let p = d.join("three.sqlite3");
    original_three(&p);
    let ready = d.join("ready");
    let module = module_path!();
    let suffix = module
        .split_once("::")
        .map(|(_, s)| format!("{s}::"))
        .unwrap_or_default();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(format!("{suffix}controlled_child"))
        .arg("--nocapture")
        .env("LUNA_GRAPH_CRASH_PATH", &p)
        .env("LUNA_GRAPH_CRASH_READY", &ready)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let observed = ready.exists();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(observed, "actual child never reached committed snapshot");
    let store = SqliteStore::open(&p, "profile", "recovered", 101).unwrap();
    store.snapshot().unwrap();
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        5
    );
    let b = Connection::open(p.with_file_name("three.sqlite3.pre-v5-100.sqlite3")).unwrap();
    assert_eq!(
        b.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
}
