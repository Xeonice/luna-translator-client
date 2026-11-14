//! Production schema-one compatibility routes. This target intentionally does
//! not emit a complete product-storage receipt: the full 22 checks and every
//! production Rust field mapping are independent prerequisites, still pending.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    contracts::{Command as AppCommand, CommandEnvelope, HistorySourceKind},
    domain::content_hash,
    ports::ProductRepository,
};
use app_infra::storage::{SqliteStore, create_dictionary, profile_migration_sha256};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn output(label: &str) -> PathBuf {
    let fallback = root()
        .join(".build/storage-runtime-components")
        .join(format!(
            "{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    acceptance_evidence::directory(label, &fallback)
}
fn ro(path: &Path) -> Connection {
    // These are closed, checkpointed test snapshots. immutable=1 prevents a
    // read-only WAL-mode inspection from creating new -shm/-wal sidecars.
    let uri = format!("file:{}?immutable=1", path.to_str().unwrap());
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    conn.execute_batch("PRAGMA query_only=ON;").unwrap();
    conn
}
fn sha(path: &Path) -> String {
    content_hash(fs::read(path).unwrap())
}
fn archive_backup(path: &Path) {
    // The production backup verifies the WAL-mode result with a read-only
    // connection. SQLite may retain that connection's empty WAL/shared-memory
    // files after close. Turn this test-owned offline backup into a standalone
    // SQLite artifact before hashing or copying it; never delete live sidecars.
    let conn = Connection::open(path).unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
        .unwrap();
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    drop(conn);
}
fn snapshot(dir: &Path, path: &Path) -> Value {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
        assert!(
            !sidecar.exists() || fs::metadata(&sidecar).unwrap().len() == 0,
            "acceptance snapshots must be closed and checkpointed"
        );
    }
    json!({"path": path.strip_prefix(dir).unwrap().to_str().unwrap(), "sha256": sha(path)})
}
fn manual(store: &SqliteStore, operation: &str, text: &str, now: i64) -> String {
    store
        .dispatch(
            CommandEnvelope {
                operation_id: operation.into(),
                expected_revision: None,
                command: AppCommand::ManualInput {
                    text: text.into(),
                    source_language: "ja".into(),
                },
            },
            now,
        )
        .unwrap()
        .result_id
        .unwrap()
}
fn inspect(path: &Path, database: &str) {
    let before = sha(path);
    let inspection = path.with_extension("inspection.sqlite3");
    fs::copy(path, &inspection).unwrap();
    assert_eq!(sha(&inspection), before);
    let process = Command::new("python3")
        .args(["-c", "import sys,json;from pathlib import Path;r=Path(sys.argv[1]);sys.path.insert(0,str(r/'scripts/quality'));from storage_schema_v5 import compare_current_schema;e=compare_current_schema(r,Path(sys.argv[2]),sys.argv[3]);print(json.dumps(e));sys.exit(bool(e))"])
        .arg(root()).arg(&inspection).arg(database).output().unwrap();
    assert!(
        process.status.success(),
        "{} {}",
        String::from_utf8_lossy(&process.stdout),
        String::from_utf8_lossy(&process.stderr)
    );
    assert_eq!(
        sha(path),
        before,
        "independent inspection must be read-only"
    );
}
fn fresh_profile(path: &Path) {
    let store = SqliteStore::open(path, "acceptance-profile", "first-instance", 100).unwrap();
    assert_eq!(store.snapshot().unwrap().commit_revision, 0);
    drop(store);
}
fn fresh_copy(dir: &Path, label: &str) -> PathBuf {
    let path = dir.join(format!("{label}.sqlite3"));
    fresh_profile(&path);
    path
}
fn route(
    dir: &Path,
    id: &str,
    before: Option<&Path>,
    after: &Path,
    backup: Option<&Path>,
    error: Option<&str>,
) -> Value {
    let row = json!({"id":id, "status":"passed", "before":before.map(|p|snapshot(dir,p)),
        "after":snapshot(dir,after), "backup":backup.map(|p|snapshot(dir,p)), "error_code":error});
    // Emit only after all route assertions have completed, never from an input
    // inventory or an expected-status list.
    if id != "dictionary_constructor_schema_only" {
        acceptance_evidence::record(
            &format!("route-{id}"),
            &[(&format!("MIGRATION-ROUTE:{id}"), 1)],
            row.clone(),
        );
    }
    row
}
fn rejected(dir: &Path, id: &str, original: &Path, error: &str) -> Value {
    let before = dir.join(format!("{id}-before.sqlite3"));
    let after = dir.join(format!("{id}-after.sqlite3"));
    fs::copy(original, &before).unwrap();
    let original_bytes = fs::read(original).unwrap();
    let result = SqliteStore::open(original, "acceptance-profile", "rejected-instance", 200);
    let actual = match result {
        Ok(_) => panic!("{id}: production unexpectedly accepted input"),
        Err(e) => e,
    };
    assert_eq!(
        actual.code, error,
        "{id}: rejection must come from the actual production preflight"
    );
    assert_eq!(
        fs::read(original).unwrap(),
        original_bytes,
        "rejection must preserve every byte of the user's original"
    );
    fs::copy(original, &after).unwrap();
    route(dir, id, Some(&before), &after, None, Some(error))
}

#[test]
fn production_schema_one_routes_preserve_unknown_inputs_and_restore_real_backup() {
    let dir = output("schema-one-routes");
    let mut routes = Vec::new();
    let profile = fresh_copy(&dir, "fresh-profile");
    inspect(&profile, "profile");
    let conn = ro(&profile);
    let ledger: (u32, u32, String) = conn
        .query_row(
            "SELECT version,previous_version,migration_sha256 FROM schema_migrations WHERE version=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(ledger, (1, 0, profile_migration_sha256()));
    let schema_two: (u32, u32, String) = conn.query_row("SELECT version,previous_version,migration_sha256 FROM schema_migrations WHERE version=2", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
    assert_eq!(
        schema_two,
        (
            2,
            1,
            content_hash(include_bytes!(
                "../src/storage/migrations/002_session_records.sql"
            ))
        )
    );
    let schema_three:(u32,u32,String)=conn.query_row("SELECT version,previous_version,migration_sha256 FROM schema_migrations WHERE version=3",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        schema_three,
        (
            3,
            2,
            app_infra::storage::http_service_identity_migration_sha256()
        )
    );
    let schema_four: (u32, u32, String) = conn.query_row("SELECT version,previous_version,migration_sha256 FROM schema_migrations WHERE version=4", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        schema_four,
        (4, 3, app_infra::storage::service_graph_migration_sha256())
    );
    let schema_five: (u32,u32,String) = conn.query_row("SELECT version,previous_version,migration_sha256 FROM schema_migrations WHERE version=5", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert_eq!(schema_five, (5,4,content_hash(include_bytes!("../src/storage/migrations/005_source_selection_sets.sql"))));
    let (sqlite_version, sqlite_source): (String, String) = conn
        .query_row("SELECT sqlite_version(),sqlite_source_id()", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    drop(conn);
    routes.push(route(&dir, "fresh_profile", None, &profile, None, None));

    let dictionary = dir.join("fresh-dictionary.sqlite3");
    create_dictionary(&dictionary).unwrap();
    inspect(&dictionary, "dictionary");
    let conn = ro(&dictionary);
    let entries: u32 = conn
        .query_row("SELECT count(*) FROM dictionary_entries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        entries, 0,
        "schema-only constructor cannot seed fake product definitions"
    );
    drop(conn);
    routes.push(route(
        &dir,
        "dictionary_constructor_schema_only",
        None,
        &dictionary,
        None,
        None,
    ));

    let reopen = fresh_copy(&dir, "reopen-live");
    let store = SqliteStore::open(&reopen, "acceptance-profile", "reopen-a", 201).unwrap();
    let original = "𠮷野の駅🦀が";
    let id = manual(&store, "saved-before-reopen", original, 202);
    let old_unit = store.unit(id.clone(), 1).unwrap();
    assert_eq!(old_unit.source_text, original);
    let summary = old_unit.source_summary.as_ref().unwrap();
    assert_eq!(summary.session_id, old_unit.session_id);
    assert_eq!(summary.game_id, old_unit.game_id);
    assert_eq!(summary.source_kind, HistorySourceKind::Manual);
    assert_eq!(summary.state, "active");
    assert_eq!(summary.started_at_ms, 202.0);
    assert_eq!(summary.ended_at_ms, None);
    let before_invalid = store.snapshot().unwrap().commit_revision;
    let invalid = store
        .dispatch(
            CommandEnvelope {
                operation_id: "reject-nul-original".into(),
                expected_revision: None,
                command: AppCommand::ManualInput {
                    text: "本文\0後半".into(),
                    source_language: "ja".into(),
                },
            },
            202,
        )
        .unwrap_err();
    assert_eq!(
        invalid.code, "invalid_text",
        "manual-input domain explicitly rejects NUL; SQL alone accepting it is not product acceptance"
    );
    assert_eq!(store.snapshot().unwrap().commit_revision, before_invalid);
    let old_generation = store.snapshot().unwrap().profile_generation;
    drop(store);
    let before = dir.join("reopen-before.sqlite3");
    fs::copy(&reopen, &before).unwrap();
    let store = SqliteStore::open(&reopen, "acceptance-profile", "reopen-b", 203).unwrap();
    // Recovery changes the live session summary, while every reading field,
    // revision, translation and token must remain byte-for-byte equivalent.
    let mut expected = old_unit.clone();
    let summary = expected.source_summary.as_mut().unwrap();
    summary.state = "crashed".into();
    summary.ended_at_ms = Some(203.0);
    assert_eq!(store.unit(id, 1).unwrap(), expected);
    assert_eq!(
        store.snapshot().unwrap().profile_generation,
        old_generation + 1
    );
    drop(store);
    let after = dir.join("reopen-after.sqlite3");
    fs::copy(&reopen, &after).unwrap();
    inspect(&after, "profile");
    routes.push(route(
        &dir,
        "reopen_profile_v1",
        Some(&before),
        &after,
        None,
        None,
    ));

    let backup_live = fresh_copy(&dir, "backup-live");
    let store =
        SqliteStore::open(&backup_live, "acceptance-profile", "backup-instance", 300).unwrap();
    let unit_id = manual(&store, "before-backup", "バックアップ前の本文", 301);
    let stored = store.unit(unit_id.clone(), 1).unwrap();
    let summary = stored.source_summary.as_ref().unwrap();
    assert_eq!(summary.session_id, stored.session_id);
    assert_eq!(summary.game_id, stored.game_id);
    assert_eq!(summary.source_kind, HistorySourceKind::Manual);
    assert_eq!(summary.state, "active");
    assert_eq!(summary.started_at_ms, 301.0);
    assert_eq!(summary.ended_at_ms, None);
    let backup = dir.join("recovery-verified-backup.sqlite3");
    store.backup(&backup).unwrap();
    archive_backup(&backup);
    manual(&store, "after-backup", "バックアップ後の本文", 302);
    drop(store);
    let before = dir.join("recovery-before.sqlite3");
    fs::copy(&backup_live, &before).unwrap();
    let after = dir.join("recovery-after.sqlite3");
    fs::copy(&backup, &after).unwrap();
    assert_eq!(sha(&backup), sha(&after));
    inspect(&after, "profile");
    // Opening the restored snapshot changes only runtime lifecycle rows. Probe
    // a separate copy so the immutable restoration artifact still proves the
    // exact backup bytes, rather than replacing it with a post-open database.
    let probe = dir.join("recovery-open-probe.sqlite3");
    fs::copy(&after, &probe).unwrap();
    let restored =
        SqliteStore::open(&probe, "acceptance-profile", "restored-instance", 400).unwrap();
    let mut expected = stored.clone();
    let summary = expected.source_summary.as_mut().unwrap();
    summary.state = "crashed".into();
    summary.ended_at_ms = Some(400.0);
    assert_eq!(restored.unit(unit_id, 1).unwrap(), expected);
    assert_eq!(restored.history(None, None, 100).unwrap().units.len(), 1);
    drop(restored);
    routes.push(route(
        &dir,
        "same_version_backup_recovery",
        Some(&before),
        &after,
        Some(&backup),
        None,
    ));

    let unknown = fresh_copy(&dir, "unknown-version");
    let conn = Connection::open(&unknown).unwrap();
    conn.execute_batch("UPDATE schema_migrations SET version=99 WHERE version=2;")
        .unwrap();
    drop(conn);
    routes.push(rejected(
        &dir,
        "reject_unknown_version",
        &unknown,
        "unsupported_schema",
    ));

    let unversioned = dir.join("unversioned.sqlite3");
    let conn = Connection::open(&unversioned).unwrap();
    conn.execute_batch("CREATE TABLE preserved_original(original TEXT); INSERT INTO preserved_original VALUES('旧形式の本文');").unwrap();
    drop(conn);
    routes.push(rejected(
        &dir,
        "reject_unversioned",
        &unversioned,
        "unsupported_schema",
    ));

    let corrupt = dir.join("corrupt.sqlite3");
    fs::write(
        &corrupt,
        b"not a SQLite database: isolated acceptance input",
    )
    .unwrap();
    routes.push(rejected(
        &dir,
        "reject_corrupt",
        &corrupt,
        "unsupported_schema",
    ));

    let mismatch = fresh_copy(&dir, "migration-hash-mismatch");
    let conn = Connection::open(&mismatch).unwrap();
    conn.execute(
        "UPDATE schema_migrations SET migration_sha256=?1",
        ["0".repeat(64)],
    )
    .unwrap();
    drop(conn);
    routes.push(rejected(
        &dir,
        "reject_migration_hash",
        &mismatch,
        "migration_mismatch",
    ));

    let drift = fresh_copy(&dir, "schema-drift");
    let conn = Connection::open(&drift).unwrap();
    conn.execute_batch("ALTER TABLE games ADD COLUMN unexpected TEXT;")
        .unwrap();
    drop(conn);
    routes.push(rejected(
        &dir,
        "reject_schema_drift",
        &drift,
        "schema_contract_mismatch",
    ));

    let relationship = fresh_copy(&dir, "relationship-corrupt");
    let conn = Connection::open(&relationship).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF; INSERT INTO sessions(session_id,game_id,source_kind,state,started_at_ms) VALUES('orphan','missing-game','manual','active',100);").unwrap();
    drop(conn);
    routes.push(rejected(
        &dir,
        "reject_relationship_corrupt",
        &relationship,
        "database_relationship_corrupt",
    ));

    let result = json!({"schema_version":1,"kind":"production_storage_route_components","status":"passed",
        "full_storage_acceptance":"blocked_incomplete_22_checks_and_typed_mapping",
        "sqlite_version":sqlite_version,"sqlite_source_id":sqlite_source,
        "migration_content_sha256":profile_migration_sha256(),"routes":routes});
    fs::write(
        dir.join("routes.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!(
        "PRODUCTION_STORAGE_COMPONENT_REPORT {}",
        dir.join("routes.json").display()
    );
}

#[test]
fn production_online_wal_backup_preserves_a_real_committed_repository_prefix() {
    let dir = output("online-wal");
    let path = dir.join("live.sqlite3");
    let store = SqliteStore::open(&path, "acceptance-profile", "live-instance", 100).unwrap();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let writer = store.clone();
    let child = thread::spawn(move || {
        for n in 0..100 {
            manual(
                &writer,
                &format!("wal-{n}"),
                &format!("実際の本文 {n} 🦀"),
                101 + n,
            );
            if n == 9 {
                ready_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
            }
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        fs::metadata(PathBuf::from(format!("{}-wal", path.display())))
            .unwrap()
            .len()
            > 0
    );
    resume_tx.send(()).unwrap();
    let backup = dir.join("online-backup.sqlite3");
    store.backup(&backup).unwrap();
    archive_backup(&backup);
    child.join().unwrap();
    let conn = ro(&backup);
    let mut statement = conn
        .prepare("SELECT source_text FROM reading_unit_revisions ORDER BY created_at_ms,unit_id")
        .unwrap();
    let texts = statement
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!((10..=100).contains(&texts.len()));
    for (n, text) in texts.iter().enumerate() {
        assert_eq!(text, &format!("実際の本文 {n} 🦀"));
    }
    let receipts: u32 = conn
        .query_row("SELECT count(*) FROM operation_receipts", [], |r| r.get(0))
        .unwrap();
    let revision: u32 = conn
        .query_row("SELECT commit_revision FROM profile_meta", [], |r| r.get(0))
        .unwrap();
    assert_eq!(receipts as usize, texts.len());
    assert_eq!(revision, receipts);
    drop(statement);
    drop(conn);
    inspect(&backup, "profile");
    let restored =
        SqliteStore::open(&backup, "acceptance-profile", "restore-instance", 500).unwrap();
    assert_eq!(
        restored.history(None, None, 200).unwrap().units.len(),
        texts.len()
    );
    assert_eq!(store.snapshot().unwrap().commit_revision, 100);
    drop(restored);
    drop(store);
    acceptance_evidence::record(
        "online-wal-prefix",
        &[("STORAGE-05", 1)],
        json!({"rows":texts.len(),"revision":revision}),
    );
    fs::write(dir.join("wal-backup.json"),serde_json::to_vec_pretty(&json!({"status":"passed","wal_observed":true,"restored_rows":texts.len(),"restored_revision":revision,"live_final_revision":100,"database":snapshot(&dir,&backup)})).unwrap()).unwrap();
    println!(
        "PRODUCTION_STORAGE_COMPONENT_REPORT {}",
        dir.join("wal-backup.json").display()
    );
}
