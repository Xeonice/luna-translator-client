//! Strict same-execution aggregation. This code never declares an unobserved
//! check passed and never reads a component artifact from a historical run.
use app_core::domain::content_hash;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn output() -> PathBuf {
    if let Some(value) = std::env::var_os("FP_STORAGE_OUTPUT") {
        return PathBuf::from(value);
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.build/storage-complete-components")
        .join(format!(
            "{}-{}",
            std::process::id(),
            super::evidence::now_ms()
        ));
    fs::create_dir_all(&path).unwrap();
    path
}
pub fn write_new(path: &Path, value: &Value) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(value).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
pub fn readonly(path: &Path) -> Connection {
    let uri = format!("file:{}?immutable=1", path.to_str().unwrap());
    let connection = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    connection.execute_batch("PRAGMA query_only=ON;").unwrap();
    connection
}
pub fn snapshot(output: &Path, path: &Path) -> Value {
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", path.display()));
        assert!(
            !side.exists(),
            "acceptance artifact must have no active or stale sidecar: {}",
            side.display()
        );
    }
    assert!(!path.symlink_metadata().unwrap().file_type().is_symlink());
    json!({"path":path.strip_prefix(output).unwrap().to_str().unwrap(),"sha256":content_hash(fs::read(path).unwrap())})
}
fn find_files(directory: &Path, name: &str, result: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        assert!(
            !kind.is_symlink(),
            "same-run evidence cannot be a symbolic link"
        );
        if kind.is_dir() {
            find_files(&entry.path(), name, result)
        } else if entry.file_name() == name {
            result.push(entry.path());
        }
    }
}
fn add(checks: &mut BTreeMap<String, Value>, value: &Value) {
    let id = value["id"].as_str().unwrap();
    assert_eq!(value["status"], "passed", "{id}");
    assert_eq!(value["skipped"], 0, "{id}");
    let count = value["assertions"].as_u64().filter(|v| *v > 0).unwrap();
    if id.starts_with("RUST-MAP:") {
        assert!(
            !checks.contains_key(id),
            "each field has one auditable combined producer proof: {id}"
        );
        assert!(
            value["source"]
                .as_str()
                .is_some_and(|source| !source.trim().is_empty()),
            "field proof must identify its concrete producer invariant: {id}"
        );
    }
    let row = checks
        .entry(id.into())
        .or_insert_with(|| json!({"id":id,"status":"passed","assertions":0,"skipped":0}));
    row["assertions"] = json!(
        row["assertions"]
            .as_u64()
            .unwrap()
            .checked_add(count)
            .unwrap()
    );
    for key in [
        "source",
        "cases",
        "model",
        "codec",
        "null_seen",
        "nonnull_seen",
        "actual_sqlite_type",
        "field_assertions",
    ] {
        if let Some(detail) = value.get(key) {
            row[key] = detail.clone();
        }
    }
}

fn schema_three_components(
    output: &Path,
    run: &str,
    source: &str,
    checks: &mut BTreeMap<String, Value>,
) -> Value {
    let nonce = std::env::var("FP_STORAGE_EXECUTION_NONCE").unwrap();
    assert!(
        !nonce.is_empty(),
        "schema3 requires a fresh execution nonce"
    );
    let started = std::env::var("FP_STORAGE_STARTED_AT_MS")
        .unwrap()
        .parse::<u128>()
        .unwrap();
    let mut components = Vec::new();
    for (case, kind) in [
        ("migration", "http_storage_v3_delta_receipt"),
        ("codec", "http_storage_v3_delta_receipt"),
        ("credentials", "http_storage_v3_delta_receipt"),
        ("fields", "http_storage_v3_delta_receipt"),
        ("gateway-authority", "http_storage_v3_delta_receipt"),
        ("crash-mid-sql", "http_storage_v3_supporting_case"),
        ("crash-after-host", "http_storage_v3_supporting_case"),
        ("backup-collision", "http_storage_v3_supporting_case"),
        ("typed-envelope-sqlite", "http_storage_v3_supporting_case"),
        (
            "typed-activation-compensation",
            "http_storage_v3_supporting_case",
        ),
        ("typed-vault-failure", "http_storage_v3_supporting_case"),
        ("legacy-raw-vault", "http_storage_v3_supporting_case"),
        (
            "typed-real-system-keychain",
            "http_storage_v3_supporting_case",
        ),
        (
            "old-client-rejects-three",
            "http_storage_v3_supporting_case",
        ),
        ("profile-mismatch-reject", "http_storage_v3_supporting_case"),
        ("host-rollback-failure", "http_storage_v3_supporting_case"),
        ("single-file-backup", "http_storage_v3_supporting_case"),
    ] {
        let path = output
            .join("components/v3-delta")
            .join(format!("{case}.json"));
        assert!(!path.symlink_metadata().unwrap().file_type().is_symlink());
        let proof = read(&path);
        assert_eq!(proof["kind"], kind, "{case}");
        assert_eq!(proof["run_id"], run, "{case}");
        assert_eq!(proof["source_tree_sha256"], source, "{case}");
        assert_eq!(proof["execution_nonce"], nonce, "{case}");
        let completed = proof["completed_at_ms"].as_u64().unwrap() as u128;
        assert!(
            completed >= started && completed <= super::evidence::now_ms(),
            "{case}"
        );
        if kind == "http_storage_v3_delta_receipt" {
            let rows = proof["checks"].as_array().unwrap();
            assert!(!rows.is_empty(), "{case}");
            for row in rows {
                let id = row["id"].as_str().unwrap();
                assert!(!checks.contains_key(id), "duplicate schema3 proof: {id}");
                add(checks, row);
            }
        } else {
            assert_eq!(proof["case"], case);
            assert!(
                proof["assertions"].as_u64().is_some_and(|count| count > 0),
                "{case}"
            );
        }
        assert!(proof["detail"].is_object(), "{case}");
        components.push(
            json!({"case":case,"path":path.strip_prefix(output).unwrap(),
            "sha256":content_hash(fs::read(&path).unwrap())}),
        );
    }
    json!(components)
}

pub fn assemble() {
    let output = output();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let run = std::env::var("FP_STORAGE_RUN_ID").unwrap();
    let source = std::env::var("FP_STORAGE_SOURCE_SHA256").unwrap();
    let inventory_path = PathBuf::from(std::env::var_os("FP_STORAGE_INVENTORY").unwrap());
    assert_eq!(
        content_hash(fs::read(&inventory_path).unwrap()),
        std::env::var("FP_STORAGE_INVENTORY_SHA256").unwrap()
    );
    let inventory = read(&inventory_path);
    let mut checks = BTreeMap::new();
    let mut cases = BTreeMap::new();
    for entry in fs::read_dir(output.join("asserted-cases")).unwrap() {
        let path = entry.unwrap().path();
        let case = read(&path);
        assert_eq!(case["kind"], "storage_asserted_case");
        assert_eq!(case["run_id"], run);
        assert_eq!(case["source_tree_sha256"], source);
        let start = std::env::var("FP_STORAGE_STARTED_AT_MS")
            .unwrap()
            .parse::<u128>()
            .unwrap();
        assert!(case["completed_at_ms"].as_u64().unwrap() as u128 >= start);
        assert!(case["completed_at_ms"].as_u64().unwrap() as u128 <= super::evidence::now_ms());
        let delta_case = case["case"].as_str().unwrap().starts_with("schema-two-");
        let is_v2 = matches!(
            std::env::var("FP_STORAGE_ADAPTER_VERSION").as_deref(),
            Ok("rust-storage-complete-v2" | "rust-storage-complete-v3")
        );
        if !delta_case || is_v2 {
            for row in case["checks"].as_array().unwrap() {
                add(&mut checks, row)
            }
        }
        assert!(
            cases
                .insert(case["case"].as_str().unwrap().to_owned(), case)
                .is_none()
        );
    }
    // These are explicit real scenario obligations, independent of field/DDL
    // enumeration. Multiple scenarios sharing a module ID must all execute.
    for required in [
        "bounded-writer-queue",
        "cross-process-busy",
        "readonly-retry",
        "kill-before-commit",
        "kill-after-commit",
        "os-lease-profile-isolation",
        "online-wal-prefix",
        "actual-keychain-crash-boundaries",
        "actual-limited-volume-enospc",
        "revoked-cleanup-restart",
        "credential-revoke-cas-cleanup",
        "completion-suspend-transaction-race",
        "runtime-stop-late-callback",
        "correction-immutable-results",
        "cache-semantic-authority",
        "cache-semantic-metamorphic-matrix",
        "explicit-retranslation-bypasses-cache-idempotently",
        "resource-mismatch-rejection-and-old-references",
        "retention-protected-provenance",
        "keyset-paging-concurrent-revisions",
        "sealed-config-gc",
        "sealed-pool-gc",
        "typed-production-relationships",
        "draft-tristate-parameters",
        "configuration-head-cas",
        "operation-cas-idempotency",
        "task-operation-idempotency",
        "hook-identity-provenance",
        "unicode-multiline-refs",
        "model-default-templates-reopen",
        "all-sql-field-probes",
        "production-profile-field-proofs",
        "real-nonempty-dictionary",
    ] {
        assert!(
            cases.contains_key(required),
            "actual semantic case did not execute: {required}"
        );
    }
    for file in ["profile-field-proofs.json", "dictionary-field-proofs.json"] {
        let proof = read(&output.join(file));
        let mut seen = BTreeSet::new();
        for row in proof["checks"].as_array().unwrap() {
            assert!(
                seen.insert(row["id"].as_str().unwrap().to_owned()),
                "duplicate field proof"
            );
            add(&mut checks, row);
        }
    }
    let is_v3 =
        std::env::var("FP_STORAGE_ADAPTER_VERSION").as_deref() == Ok("rust-storage-complete-v3");
    let is_v2 = is_v3
        || std::env::var("FP_STORAGE_ADAPTER_VERSION").as_deref() == Ok("rust-storage-complete-v2");
    let mut schema_two_migration = Value::Null;
    if is_v2 {
        for (case_name, expected_count) in [
            ("schema-two-production-fields", 29),
            ("schema-two-field-boundaries", 29),
        ] {
            let case = cases
                .get(case_name)
                .expect("schema-two actual producer case must execute");
            let path = PathBuf::from(case["detail"]["receipt"].as_str().unwrap());
            assert!(path.starts_with(&output));
            let rows = read(&path);
            let rows = rows["checks"].as_array().unwrap();
            assert_eq!(rows.len(), expected_count);
            for row in rows {
                add(&mut checks, row);
            }
        }
        assert!(cases.contains_key("schema-two-immutable-relations"));
        assert!(cases.contains_key("schema-two-string-sequences"));
        for (case, semantic_id) in [
            (
                "schema-two-unbind-serialization-replay",
                "V2-UNBIND-SERIALIZATION",
            ),
            ("schema-two-unbind-stale-identity", "V2-UNBIND-IDENTITY"),
            (
                "schema-two-unbind-commit-compensation",
                "V2-UNBIND-COMPENSATION",
            ),
        ] {
            let proof = cases
                .get(case)
                .expect("actual typed unbind scenario must execute");
            assert!(
                proof["checks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == semantic_id
                        && row["status"] == "passed"
                        && row["assertions"].as_u64().is_some_and(|n| n > 0)
                        && row["skipped"] == 0),
                "actual unbind scenario lacks its own semantic assertion: {case}"
            );
        }
        for case in [
            "schema-two-scoped-query-saved-head",
            "schema-two-scoped-head-cas-idempotency",
            "schema-two-scoped-isolation-reopen",
            "schema-two-scoped-inherit-false-zero-empty",
            "schema-two-scoped-writer-race",
            "schema-two-jsonl-atomic-delivery",
            "schema-two-jsonl-cancel-reject",
            "schema-two-jsonl-enospc-retry",
        ] {
            assert!(
                cases.contains_key(case),
                "schema-two actual semantic case did not execute: {case}"
            );
        }
        let case = cases
            .get("schema-two-migration-backup")
            .expect("real migration backup proof must execute");
        let path = PathBuf::from(case["detail"]["receipt"].as_str().unwrap());
        assert!(path.starts_with(&output));
        schema_two_migration = read(&path);
        let migration_snapshots = if is_v3 {
            vec![
                "before",
                "after",
                "backup",
                "restored",
                "schema_two_intermediate",
                "schema_two_original_backup",
            ]
        } else {
            vec!["before", "after", "backup", "restored"]
        };
        for key in migration_snapshots {
            let relative = schema_two_migration[key]["path"].as_str().unwrap();
            let file = path.parent().unwrap().join(relative);
            let sealed = snapshot(&output, &file);
            assert_eq!(sealed["sha256"], schema_two_migration[key]["sha256"]);
            schema_two_migration[key] = sealed;
        }
    }
    let probe_path = PathBuf::from(
        cases["all-sql-field-probes"]["detail"]["receipt"]
            .as_str()
            .unwrap(),
    );
    assert!(probe_path.starts_with(&output));
    let probes = read(&probe_path);
    assert_eq!(probes["status"], "passed");
    assert_eq!(probes["failures"], json!([]));
    for row in probes["checks"].as_array().unwrap() {
        add(&mut checks, row)
    }

    let mut route_reports = Vec::new();
    find_files(
        &output.join("components"),
        "routes.json",
        &mut route_reports,
    );
    assert_eq!(
        route_reports.len(),
        1,
        "routes must be from this one execution"
    );
    let route_report = read(&route_reports[0]);
    assert_eq!(route_report["status"], "passed");
    let base = route_reports[0].parent().unwrap();
    let dictionary = read(&output.join("dictionary-field-proofs.json"));
    let mut routes = Vec::new();
    for actual in route_report["routes"].as_array().unwrap() {
        if actual["id"] == "dictionary_constructor_schema_only" {
            continue;
        }
        let mut actual = actual.clone();
        for key in ["before", "after", "backup"] {
            if !actual[key].is_null() {
                let path = base.join(actual[key]["path"].as_str().unwrap());
                let sealed = snapshot(&output, &path);
                assert_eq!(sealed["sha256"], actual[key]["sha256"]);
                actual[key] = sealed;
            }
        }
        routes.push(actual);
    }
    routes.push(json!({"id":"fresh_dictionary","status":"passed","before":null,"after":dictionary["database"],"backup":null,"error_code":null}));
    let route_ids = routes
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(route_ids.len(), routes.len());
    let required_route_ids = inventory["migration_routes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(route_ids, required_route_ids);
    let specifications = inventory["migration_routes"].as_array().unwrap();
    for route in &mut routes {
        let spec = specifications
            .iter()
            .find(|v| v["id"] == route["id"])
            .unwrap();
        for key in ["database", "source_schema", "target_schema"] {
            route[key] = spec[key].clone()
        }
        for key in ["source_schema", "target_schema"] {
            route[format!("{key}_sha256")] = if spec[key].is_null() {
                Value::Null
            } else {
                json!(content_hash(
                    fs::read(root.join(spec[key].as_str().unwrap())).unwrap()
                ))
            };
        }
        // Outcome is established by actual rejection/acceptance assertions,
        // then independently compared to the frozen policy by the executor.
        route["outcome"] = json!(if route["error_code"].is_null() {
            "accepted"
        } else {
            "rejected"
        });
    }
    // Fresh creation and route semantics were asserted in the production
    // component target; these checks point to their actual asserted cases.
    for (id, case_names) in [
        ("STORAGE-02", vec!["route-fresh_profile"]),
        (
            "STORAGE-03",
            vec![
                "route-reopen_profile_v1",
                "route-same_version_backup_recovery",
            ],
        ),
        (
            "STORAGE-04",
            vec![
                "route-reject_unknown_version",
                "route-reject_unversioned",
                "route-reject_corrupt",
                "route-reject_migration_hash",
                "route-reject_schema_drift",
                "route-reject_relationship_corrupt",
            ],
        ),
    ] {
        for case in &case_names {
            assert!(
                cases.contains_key(*case),
                "missing actual route assertion {case}"
            )
        }
        add(
            &mut checks,
            &json!({"id":id,"status":"passed","assertions":case_names.len(),"skipped":0}),
        );
    }
    let schema_three_components = if is_v3 {
        schema_three_components(&output, &run, &source, &mut checks)
    } else {
        Value::Null
    };
    let expected = inventory["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    let actual = checks.keys().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "a receipt cannot be issued with unproved/unknown fields, boundaries, semantic checks or routes"
    );
    let mut databases = Vec::new();
    for name in ["profile", "dictionary"] {
        let route = routes
            .iter()
            .find(|r| r["id"] == format!("fresh_{name}"))
            .unwrap();
        databases.push(json!({"database":name,"path":route["after"]["path"],"sha256":route["after"]["sha256"],"open_mode":"read_only_snapshot","origin":"registered_production_migrations"}));
    }
    let artifact = std::env::var("FP_STORAGE_ARTIFACT_SHA256").unwrap();
    let receipt = json!({"schema_version":1,"kind":"product_storage_runtime_receipt","run_id":run,"status":"passed",
        "source_tree_sha256":source,"artifact_sha256":if artifact.is_empty(){Value::Null}else{json!(artifact)},
        "adapter_version":if is_v3 {"rust-storage-complete-v3"}else if is_v2 {"rust-storage-complete-v2"}else{"rust-storage-complete-v1"},"schema_two_migration":schema_two_migration,"sqlite_version":route_report["sqlite_version"],"sqlite_source_id":route_report["sqlite_source_id"],
        "started_at_ms":std::env::var("FP_STORAGE_STARTED_AT_MS").unwrap().parse::<u64>().unwrap(),
        "finished_at_ms":super::evidence::now_ms(),"checks":checks.values().collect::<Vec<_>>(),"migration_routes":routes,"databases":databases,
        "asserted_case_files":cases.keys().collect::<Vec<_>>()});
    let mut receipt = receipt;
    if is_v3 {
        receipt["execution_nonce"] = json!(std::env::var("FP_STORAGE_EXECUTION_NONCE").unwrap());
        receipt["schema_three_components"] = schema_three_components;
    }
    write_new(
        &PathBuf::from(std::env::var_os("FP_STORAGE_RECEIPT").unwrap()),
        &receipt,
    );
    let stdout = std::io::stdout();
    write_assertion_markers(stdout.lock(), &checks).unwrap();
}

fn write_assertion_markers(
    mut writer: impl Write,
    checks: &BTreeMap<String, Value>,
) -> std::io::Result<()> {
    // With --nocapture, libtest can leave its test-name prefix without a newline.
    // Keep every actual assertion marker on its own complete, parseable line.
    writeln!(writer)?;
    for row in checks.values() {
        writeln!(
            writer,
            "STORAGE_ASSERTION {} {} PASS",
            row["id"].as_str().unwrap(),
            row["assertions"].as_u64().unwrap()
        )?;
    }
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertion_markers_start_after_libtest_prefix_and_preserve_all_counts() {
        // Formatter inputs are isolated fixtures, never product acceptance evidence.
        let checks = BTreeMap::from([
            (
                "MIGRATION-ROUTE:fresh_dictionary".into(),
                json!({"id":"MIGRATION-ROUTE:fresh_dictionary","assertions":1}),
            ),
            (
                "RUST-MAP:dictionary.dictionary_entries.lemma".into(),
                json!({"id":"RUST-MAP:dictionary.dictionary_entries.lemma","assertions":339651}),
            ),
            (
                "STORAGE-02".into(),
                json!({"id":"STORAGE-02","assertions":37}),
            ),
        ]);
        let prefix = "test receipt_formatter_contract ... ";
        let mut output = prefix.as_bytes().to_vec();

        write_assertion_markers(&mut output, &checks).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.starts_with(&format!("{prefix}\n")));
        assert!(output.ends_with('\n'));
        let mut lines = output.lines();
        assert_eq!(lines.next(), Some(prefix));
        let markers = lines.collect::<Vec<_>>();
        assert_eq!(markers.len(), checks.len());
        for (line, (id, row)) in markers.into_iter().zip(&checks) {
            let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
            assert_eq!(fields.len(), 4);
            assert_eq!(fields[0], "STORAGE_ASSERTION");
            assert_eq!(fields[1], id);
            assert_eq!(
                fields[2].parse::<u64>().unwrap(),
                row["assertions"].as_u64().unwrap()
            );
            assert_eq!(fields[3], "PASS");
        }
    }
}
