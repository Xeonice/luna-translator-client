//! Actual compiled dictionary reader failures and immutable historical links.
use app_core::{
    contracts::LexicalToken,
    domain::{capture::*, content_hash},
    ports::ProductRepository,
};
use app_infra::{dictionary::Dictionary, storage::SqliteStore};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
pub fn verify(output: &Path) {
    fs::create_dir_all(output).unwrap();
    let resources = root().join("resources");
    let frozen = Dictionary::open(&resources).unwrap();
    let info = frozen.resource_info();
    drop(frozen);
    super::dictionary::verify_profile_link_with_record(output, &info, None);
    let link: Value = serde_json::from_slice(
        &fs::read(output.join("cross-database-resource-link.json")).unwrap(),
    )
    .unwrap();
    let profile_path = output.join("dictionary-consumer-profile.sqlite3");
    let unit = link["reading_unit_id"].as_str().unwrap().to_owned();
    let old = fingerprint(&profile_path, &unit);
    let isolated = output.join("isolated-resources");
    let language = isolated.join("language");
    fs::create_dir_all(&language).unwrap();
    let manifest = fs::read(resources.join("language/manifest.json")).unwrap();
    let provenance = fs::read(resources.join("language/dictionary-provenance.json")).unwrap();
    fs::write(language.join("manifest.json"), &manifest).unwrap();
    fs::write(language.join("dictionary-provenance.json"), &provenance).unwrap();
    let original_db = resources.join("language/dictionary.sqlite3");
    let copied_db = language.join("dictionary.sqlite3");
    clone_file(&original_db, &copied_db);
    let mut rejections = vec![];
    for kind in [
        "manifest-version",
        "provenance-identity",
        "index-schema-version",
    ] {
        let expected = match kind {
            "manifest-version" => {
                let mut v: Value = serde_json::from_slice(&manifest).unwrap();
                v["dictionary_version"] = json!("unsupported-test-version");
                fs::write(
                    language.join("manifest.json"),
                    serde_json::to_vec(&v).unwrap(),
                )
                .unwrap();
                "dictionary manifest differs from compiled resource lock"
            }
            "provenance-identity" => {
                let mut v: Value = serde_json::from_slice(&provenance).unwrap();
                v["resource_version"] = json!("wrong-test-version");
                fs::write(
                    language.join("dictionary-provenance.json"),
                    serde_json::to_vec(&v).unwrap(),
                )
                .unwrap();
                "dictionary provenance hash mismatch"
            }
            _ => {
                let c = Connection::open(&copied_db).unwrap();
                assert_eq!(
                    c.execute(
                        "UPDATE dictionary_meta SET index_schema_version=99 WHERE singleton=1",
                        []
                    )
                    .unwrap(),
                    1
                );
                assert_eq!(
                    c.query_row(
                        "SELECT index_schema_version FROM dictionary_meta",
                        [],
                        |r| r.get::<_, u32>(0)
                    )
                    .unwrap(),
                    99
                );
                drop(c);
                "dictionary file hash mismatch"
            }
        };
        let error = match Dictionary::open(&isolated) {
            Ok(_) => panic!("unreviewed resource {kind} was accepted"),
            Err(e) => e,
        };
        assert_eq!(error, expected);
        assert_eq!(
            fingerprint(&profile_path, &unit),
            old,
            "failure must not relabel old references or source"
        );
        fs::write(language.join("manifest.json"), &manifest).unwrap();
        fs::write(language.join("dictionary-provenance.json"), &provenance).unwrap();
        if kind == "index-schema-version" {
            fs::remove_file(&copied_db).unwrap();
            clone_file(&original_db, &copied_db);
        }
        let restored = Dictionary::open(&isolated).unwrap();
        assert_eq!(restored.resource_info().version, info.version);
        assert!(
            !restored
                .lookup("図書館", "図書館", "としょかん")
                .unwrap()
                .is_empty()
        );
        drop(restored);
        assert_eq!(fingerprint(&profile_path, &unit), old);
        rejections.push(json!({"case":kind,"actual_error":error,"old_reference_sha256":old,"restored_available":true,"rejection_layer":if kind=="index-schema-version"{"integrity rejects unsupported index bytes before interpretation"}else{"compiled manifest/provenance identity"}}));
    }
    let store = SqliteStore::open(
        &profile_path,
        "dictionary-link-profile",
        "resource-rejection-server",
        40,
    )
    .unwrap();
    let read =
        Connection::open_with_flags(&profile_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let (id,kind,version,hash,contract,metadata):(String,String,String,String,String,String)=read.query_row("SELECT resource_snapshot_id,kind,version_label,manifest_sha256,contract_version,metadata_json FROM resource_snapshots WHERE resource_snapshot_id=?1",[link["language_snapshot_id"].as_str().unwrap()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
    drop(read);
    let base = ResourceSnapshotInput {
        snapshot_id: id,
        kind,
        version,
        manifest_sha256: hash,
        contract_version: contract,
        metadata_json: metadata,
    };
    let hook = HookSessionInput {
        session_id: "resource-identity-hook".into(),
        game_id: None,
        environment_key: "crossover:/controlled/resource-identity".into(),
        target_exe_path: "C:\\resource-identity.exe".into(),
        windows_pid: 101,
        process_created_u64: "101".into(),
        architecture: "x86".into(),
        broker_instance_id: "resource-identity-broker".into(),
        target_identity_sha256: content_hash("resource-identity-target"),
        resource: ResourceSnapshotInput {
            snapshot_id: "resource-identity-core".into(),
            kind: "core".into(),
            version: "controlled-core-v1".into(),
            manifest_sha256: content_hash("controlled-core-v1"),
            contract_version: "core-v1".into(),
            metadata_json: "{\"a\":1,\"b\":2}".into(),
        },
    };
    store.open_hook_session(hook.clone(), 40).unwrap();
    let committed = store.snapshot().unwrap().commit_revision;
    store.open_hook_session(hook.clone(), 41).unwrap();
    assert_eq!(store.snapshot().unwrap().commit_revision, committed);
    for changed in ["version", "contract", "metadata", "manifest", "kind"] {
        let mut conflicting = hook.clone();
        match changed {
            "version" => conflicting.resource.version = "conflicting-core-v2".into(),
            "contract" => {
                conflicting.resource.contract_version = "conflicting-core-contract".into()
            }
            "metadata" => conflicting.resource.metadata_json = "{\"changed\":true}".into(),
            "manifest" => {
                conflicting.resource.manifest_sha256 = content_hash("conflicting-core-manifest")
            }
            _ => conflicting.resource.kind = "language".into(),
        }
        let error = store.open_hook_session(conflicting, 42).unwrap_err();
        assert_eq!(error.code, "resource_identity_conflict");
        assert_eq!(store.snapshot().unwrap().commit_revision, committed);
        assert_eq!(fingerprint(&profile_path, &unit), old);
        let read =
            Connection::open_with_flags(&profile_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let persisted: (String, String, String, String, String) = read.query_row(
            "SELECT kind,version_label,manifest_sha256,contract_version,metadata_json FROM resource_snapshots WHERE resource_snapshot_id=?1",
            [&hook.resource.snapshot_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
        ).unwrap();
        assert_eq!(
            persisted,
            (
                hook.resource.kind.clone(),
                hook.resource.version.clone(),
                hook.resource.manifest_sha256.clone(),
                hook.resource.contract_version.clone(),
                hook.resource.metadata_json.clone()
            )
        );
        rejections.push(json!({"case":format!("same-session-core-{changed}-conflict"),"actual_error":error.code,"old_reference_sha256":old,"commit_revision_unchanged":committed}));
    }
    let token = LexicalToken {
        surface: "図書館".into(),
        lemma: "図書館".into(),
        reading: "としょかん".into(),
        part_of_speech: "名詞".into(),
        byte_start: 0,
        byte_end: 9,
        utf16_start: 0,
        utf16_end: 3,
        known: true,
    };
    for changed in ["version", "contract", "metadata", "manifest", "kind"] {
        let mut resource = base.clone();
        match changed {
            "version" => resource.version = "conflicting-v2".into(),
            "contract" => resource.contract_version = "conflicting-contract".into(),
            "metadata" => resource.metadata_json = "{\"different\":true}".into(),
            "manifest" => resource.manifest_sha256 = content_hash("conflicting-manifest"),
            _ => resource.kind = "core".into(),
        }
        let error = store
            .set_tokens(
                TokenSnapshotInput {
                    unit_id: unit.clone(),
                    text_revision: 1,
                    analysis_revision: format!("must-reject-{changed}"),
                    tokens: vec![token.clone()],
                    resource,
                },
                41,
            )
            .unwrap_err();
        assert_eq!(
            error.code, "resource_identity_conflict",
            "reused identity must reject {changed}"
        );
        assert_eq!(fingerprint(&profile_path, &unit), old);
        rejections.push(json!({"case":format!("snapshot-{changed}-conflict"),"actual_error":error.code,"old_reference_sha256":old}));
    }
    let value: Value = serde_json::from_str(&base.metadata_json).unwrap();
    let reversed = value
        .as_object()
        .unwrap()
        .iter()
        .rev()
        .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v))
        .collect::<Vec<_>>()
        .join(",");
    let mut equivalent = base;
    equivalent.metadata_json = format!("{{{reversed}}}");
    store
        .set_tokens(
            TokenSnapshotInput {
                unit_id: unit.clone(),
                text_revision: 1,
                analysis_revision: "equivalent-metadata-order".into(),
                tokens: vec![token],
                resource: equivalent,
            },
            42,
        )
        .unwrap();
    assert_eq!(fingerprint(&profile_path, &unit), old);
    drop(store);
    let reopened = SqliteStore::open(
        &profile_path,
        "dictionary-link-profile",
        "resource-restored-reopen",
        50,
    )
    .unwrap();
    assert_eq!(
        reopened.unit(unit.clone(), 1).unwrap().source_text,
        "図書館"
    );
    assert_eq!(fingerprint(&profile_path, &unit), old);
    drop(reopened);
    assert_eq!(
        content_hash(fs::read(&original_db).unwrap()),
        link["dictionary_file_sha256"].as_str().unwrap()
    );
    let report = json!({"status":"passed","rejections":rejections,"old_original_and_reference_sha256":old,"equivalent_json_metadata_order_accepted":true,"profile_reopened":true,"frozen_resource_file_unchanged":true});
    fs::write(
        output.join("resource-rejections.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    super::evidence::record(
        "resource-mismatch-rejection-and-old-references",
        &[("STORAGE-16", rejections.len() as u64 + 4)],
        report,
    );
}
fn clone_file(source: &Path, destination: &Path) {
    assert!(!destination.exists());
    #[cfg(target_os = "macos")]
    {
        let result = std::process::Command::new("/bin/cp")
            .arg("-c")
            .arg(source)
            .arg(destination)
            .output()
            .unwrap();
        if result.status.success() {
            return;
        }
        if destination.exists() {
            fs::remove_file(destination).unwrap();
        }
    }
    fs::copy(source, destination).unwrap();
}
fn fingerprint(path: &Path, unit: &str) -> String {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    c.execute_batch("PRAGMA query_only=ON").unwrap();
    let row:(String,String,String,String,String,String,String,String,String,String)=c.query_row("SELECT u.source_text,u.source_sha256,l.language_resource_snapshot_id,l.analysis_revision,l.token_schema_version,r.kind,r.version_label,r.manifest_sha256,r.contract_version,r.metadata_json FROM reading_unit_revisions u JOIN lexical_snapshots l USING(unit_id,text_revision) JOIN resource_snapshots r ON r.resource_snapshot_id=l.language_resource_snapshot_id WHERE u.unit_id=?1 AND u.text_revision=1 AND l.analysis_revision='controlled-resource-link-v1'",[unit],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).unwrap();
    content_hash(serde_json::to_vec(&row).unwrap())
}
