//! Real production migration artifacts compared by the independent frozen-contract inspector.
//! This target is schema evidence, not STORAGE-01..22 completion or a full Rust mapping claim.
use app_core::{contracts::*, ports::ProductRepository};
use app_infra::storage::{SqliteStore, create_dictionary};
#[allow(dead_code)]
#[path = "storage_acceptance_support/frozen_hosts.rs"]
mod frozen_hosts;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn artifact_dir(label: &str) -> PathBuf {
    let path = root().join(".build/storage-schema-contract").join(format!(
        "{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn compare_with_contract(path: &Path, database: &str, module: &str) -> Vec<String> {
    // The production backup's completed read-only integrity probe may leave a
    // stale shared-memory file. Archive only the completed, checkpointed main
    // file into this test's own directory; never remove/checkpoint live files.
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    assert!(
        !wal.exists() || std::fs::metadata(&wal).unwrap().len() == 0,
        "a pending WAL cannot be omitted from a standalone schema candidate"
    );
    let original = std::fs::read(path).unwrap();
    let candidate = path.with_extension("inspection.sqlite3");
    std::fs::write(&candidate, &original).unwrap();
    assert_eq!(std::fs::read(&candidate).unwrap(), original);
    let script = "import sys,json,importlib;from pathlib import Path;root=Path(sys.argv[1]);sys.path.insert(0,str(root/'scripts/quality'));m=importlib.import_module(sys.argv[4]);print(json.dumps(m.compare_current_schema(root,Path(sys.argv[2]),sys.argv[3])))";
    let result = Command::new("python3")
        .args(["-c", script])
        .arg(root())
        .arg(&candidate)
        .arg(database)
        .arg(module)
        .output()
        .expect("schema precondition: Python3 and independent inspector must exist");
    assert!(
        result.status.success(),
        "independent schema inspector failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let differences = serde_json::from_slice(&result.stdout)
        .expect("schema inspector must return its actual differences");
    assert_eq!(std::fs::read(path).unwrap(), original);
    assert_eq!(std::fs::read(candidate).unwrap(), original);
    differences
}
fn compare(path: &Path, database: &str) -> Vec<String> {
    compare_with_contract(path, database, "storage_schema_v5")
}
fn profile(dir: &Path) -> PathBuf {
    let live = dir.join("writer.sqlite3");
    let candidate = dir.join("profile.sqlite3");
    let store = SqliteStore::open(live, "schema-profile", "schema-server", 100).unwrap();
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "manual-schema-proof".into(),
                expected_revision: None,
                command: app_core::contracts::Command::ManualInput {
                    text: "実際の生成経路".into(),
                    source_language: "ja".into(),
                },
            },
            101,
        )
        .unwrap();
    let (version, source) = store.database_versions().unwrap();
    store.backup(&candidate).unwrap();
    println!("ACTUAL_SQLITE_VERSION {version} {source}");
    println!("PRODUCTION_SCHEMA_ARTIFACT {}", candidate.display());
    candidate
}
#[test]
fn production_profile_migration_matches_every_frozen_schema_object() {
    let dir = artifact_dir("profile");
    let candidate = profile(&dir);
    assert_eq!(compare(&candidate, "profile"), Vec::<String>::new());
    let conn = rusqlite::Connection::open_with_flags(
        &candidate,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let version: u32 = conn
        .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .unwrap();
    let reviewed: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root().join("docs/architecture/database/v5/schema-contract.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        version,
        reviewed["migration_version"].as_u64().unwrap() as u32
    );
    let count: u32 = conn
        .query_row("SELECT count(*) FROM reading_unit_revisions", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}
#[test]
fn dictionary_constructor_matches_frozen_schema_without_claiming_resource_content() {
    let dir = artifact_dir("dictionary");
    let candidate = dir.join("dictionary.sqlite3");
    create_dictionary(&candidate).unwrap();
    assert_eq!(compare(&candidate, "dictionary"), Vec::<String>::new());
    let conn = rusqlite::Connection::open_with_flags(
        &candidate,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        conn.query_row::<u32, _, _>("SELECT count(*) FROM dictionary_entries", [], |r| r.get(0))
            .unwrap(),
        0,
        "schema-only constructor must not seed fake dictionary entries"
    );
}
#[test]
fn real_field_type_or_trigger_changes_are_rejected_by_independent_inspector() {
    let dir = artifact_dir("mutant");
    let candidate = profile(&dir);
    let conn = rusqlite::Connection::open(&candidate).unwrap();
    conn.execute_batch(
        "ALTER TABLE games ADD COLUMN unauthorized_field TEXT; DROP TRIGGER immutable_corrections;",
    )
    .unwrap();
    drop(conn);
    let differences = compare(&candidate, "profile");
    assert!(!differences.is_empty());
    assert!(differences.iter().any(|e| e.contains("games")));
    assert!(
        differences
            .iter()
            .any(|e| e.contains("immutable_corrections"))
    );
}
#[test]
fn actual_foreign_key_data_violation_is_rejected_not_only_schema_shape() {
    let dir = artifact_dir("invalid-relationship");
    let candidate = profile(&dir);
    let conn = rusqlite::Connection::open(&candidate).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF; INSERT INTO sessions(session_id,game_id,source_kind,state,started_at_ms) VALUES('orphan-session','missing-game','manual','active',100);").unwrap();
    drop(conn);
    let differences = compare(&candidate, "profile");
    assert!(
        differences
            .iter()
            .any(|e| e.contains("foreign key violation")),
        "{differences:?}"
    );
}

#[test]
fn actual_original_schema_two_writer_keeps_its_own_frozen_contract_and_version() {
    let dir = artifact_dir("original-schema-two");
    let candidate = dir.join("original.sqlite3");
    let c = rusqlite::Connection::open(&candidate).unwrap();
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    c.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
        .unwrap();
    drop(c);
    assert_eq!(
        compare_with_contract(&candidate, "profile", "storage_schema_v2"),
        Vec::<String>::new()
    );
    let c = rusqlite::Connection::open(&candidate).unwrap();
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        2
    );
}

#[test]
fn frozen_schema_three_migration_retains_every_original_schema_object() {
    let dir = artifact_dir("original-schema-three");
    let candidate = dir.join("original-three.sqlite3");
    let conn = rusqlite::Connection::open(&candidate).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    conn.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
        .unwrap();
    // The preserved production migration is byte-bound to its independent
    // review; do not learn an expected schema from the current writer.
    let migration = include_str!("../src/storage/migrations/003_http_service_identity.sql");
    let reviewed: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root().join("docs/architecture/database/v3/storage-review.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        app_core::domain::content_hash(migration.as_bytes()),
        reviewed["reviewed_design_sha256"]
    );
    conn.execute_batch(migration).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    drop(conn);
    assert_eq!(
        compare_with_contract(&candidate, "profile", "storage_schema_v3"),
        Vec::<String>::new()
    );
}

#[test]
fn actual_frozen_store_four_keeps_its_exact_whole_contract_and_rejects_five() {
    let dir = artifact_dir("actual-frozen-four");
    let old_path = dir.join("actual-four.sqlite3");
    let old = frozen_hosts::FrozenHost::four();
    old.open(&old_path, "frozen-four-profile");
    assert_eq!(compare_with_contract(&old_path, "profile", "storage_schema_v4"), Vec::<String>::new());
    let current = profile(&dir);
    let before = std::fs::read(&current).unwrap();
    let result = Command::new(old.binary).arg(&current).arg("schema-profile").arg("reject").output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let rejected: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(rejected["error_code"], "unsupported_schema");
    assert_eq!(std::fs::read(current).unwrap(), before);
}
