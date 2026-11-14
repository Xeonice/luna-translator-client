//! Test-only access to byte-verified, actually compiled earlier production hosts.
//! Missing prerequisites fail the test; no current writer or SQL emulates an old host.
use app_core::domain::content_hash;
use serde_json::Value;
use std::{
    path::{Component, Path, PathBuf},
    process::Command,
};

pub struct FrozenHost {
    pub manifest_path: PathBuf,
    pub manifest_sha256: String,
    pub binary: PathBuf,
    pub schema_version: u32,
}

fn verify_source(root: &Path, relative: &str, expected: &str) {
    let path = Path::new(relative);
    assert!(
        !path.is_absolute() && path.components().all(|c| matches!(c, Component::Normal(_))),
        "frozen source must be a repository-relative file"
    );
    assert_eq!(
        content_hash(std::fs::read(root.join(path)).unwrap()),
        expected,
        "frozen source changed: {relative}"
    );
}

impl FrozenHost {
    pub fn two() -> Self {
        Self::load(
            "LUNA_HTTP_OLD_READER_MANIFEST",
            "actual_old_sqlite_store_reader_v2",
            2,
        )
    }

    pub fn three() -> Self {
        Self::load(
            "FP_STORAGE_OLD_STORE3_READER_MANIFEST",
            "actual_old_sqlite_store_reader_v3",
            3,
        )
    }

    pub fn four() -> Self {
        Self::load(
            "FP_STORAGE_OLD_STORE4_READER_MANIFEST",
            "actual_old_sqlite_store_reader_v4",
            4,
        )
    }

    fn load(variable: &str, kind: &str, schema_version: u32) -> Self {
        let manifest_path = PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| panic!("required real frozen host prerequisite {variable}; missing is blocked, never skipped")));
        let bytes = std::fs::read(&manifest_path).unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["kind"], kind);
        assert_eq!(value["schema_version"], schema_version);
        let binary = PathBuf::from(value["binary_path"].as_str().unwrap());
        assert_eq!(
            content_hash(std::fs::read(&binary).unwrap()),
            value["binary_sha256"]
        );
        let root = Path::new(value["source_root"].as_str().unwrap());
        match &value["source_files"] {
            Value::Array(rows) => {
                assert!(!rows.is_empty());
                for row in rows {
                    verify_source(
                        root,
                        row["path"].as_str().unwrap(),
                        row["sha256"].as_str().unwrap(),
                    );
                }
            }
            Value::Object(rows) => {
                assert!(!rows.is_empty());
                for (path, sha) in rows {
                    verify_source(root, path, sha.as_str().unwrap());
                }
            }
            _ => panic!("frozen host requires its complete source file map"),
        }
        if let Some(path) = value["source_manifest_path"].as_str() {
            assert_eq!(
                content_hash(std::fs::read(path).unwrap()),
                value["source_manifest_sha256"]
            );
        }
        if let Some(hash) = value["original_cargo_lock_sha256"].as_str() {
            verify_source(root, "Cargo.lock", hash);
        }
        Self {
            manifest_path,
            manifest_sha256: content_hash(bytes),
            binary,
            schema_version,
        }
    }

    pub fn open(&self, path: &Path, profile: &str) -> Value {
        let result = Command::new(&self.binary)
            .arg(path)
            .arg(profile)
            .arg("accept")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "actual frozen host{} failed: {} {}",
            self.schema_version,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let opened: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(opened["opened"], true);
        let conn =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        assert_eq!(
            conn.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            self.schema_version,
            "the actual old writer's database, not its diagnostic label, establishes the schema"
        );
        opened
    }
}
