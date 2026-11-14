//! Real frozen Store3 -> current Registry/compiler -> proved connection-only preservation.
//! The writer artifact is an explicit test prerequisite; absence is blocked, never skipped.
use app_core::domain::model_config::service_graph::{HttpConfigRef, SavedServiceRef};
use app_core::{
    contracts::*,
    domain::{content_hash, model_config::service_draft::compile_service_draft},
    ports::{
        IdentitySource, ServiceConfigurationRepository, service_configuration::CompiledServiceDraft,
    },
};
use app_infra::{providers::builtin_registry, storage::SqliteStore};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
struct Ids;
impl IdentitySource for Ids {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        panic!("pure compilation may not read time")
    }
}
fn old_writer() -> PathBuf {
    let manifest = PathBuf::from(
        std::env::var_os("FP_STORAGE_OLD_STORE3_CONNECTION_WRITER_MANIFEST").expect(
            "real frozen Store3 connection writer prerequisite missing; blocked, never skip",
        ),
    );
    let bytes = std::fs::read(&manifest).unwrap();
    let expected = std::env::var("FP_STORAGE_OLD_STORE3_CONNECTION_WRITER_SHA256")
        .expect("locked old writer manifest SHA prerequisite missing; blocked");
    assert_eq!(content_hash(&bytes), expected);
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["kind"], "real_frozen_store3_connection_writer");
    assert_eq!(value["registry_dependencies_match_original_lock"], true);
    let binary = PathBuf::from(value["binary_path"].as_str().unwrap());
    assert_eq!(
        content_hash(std::fs::read(&binary).unwrap()),
        value["binary_sha256"]
    );
    let source = PathBuf::from(value["old_source_root"].as_str().unwrap());
    for (path, sha) in value["old_source_files"].as_object().unwrap() {
        assert_eq!(
            content_hash(std::fs::read(source.join(path)).unwrap()),
            sha.as_str().unwrap(),
            "old source {path}"
        )
    }
    assert_eq!(
        content_hash(std::fs::read(source.join("Cargo.lock")).unwrap()),
        value["original_lock_sha256"]
    );
    binary
}
struct Fixture {
    path: PathBuf,
    store: SqliteStore,
    source: String,
    source_seal: String,
    old_columns: Vec<String>,
    old_rows: Vec<Vec<String>>,
}
fn rows(conn: &Connection, revision: &str, columns: &[String]) -> Vec<Vec<String>> {
    let columns = columns
        .iter()
        .map(|c| format!("quote(\"{c}\")"))
        .collect::<Vec<_>>()
        .join(",");
    let mut q = conn
        .prepare(&format!(
            "SELECT {columns} FROM connections WHERE revision_id=?1 ORDER BY connection_id"
        ))
        .unwrap();
    q.query_map([revision], |r| {
        (0..columns_count(&columns))
            .map(|i| r.get(i))
            .collect::<rusqlite::Result<Vec<String>>>()
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}
fn columns_count(columns: &str) -> usize {
    columns.split(',').count()
}
fn fixture(count: usize) -> Fixture {
    let dir = std::env::temp_dir().join(format!(
        "r4-real-connection-source-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.sqlite3");
    let output = Command::new(old_writer())
        .args([path.to_str().unwrap(), &count.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run: Value = serde_json::from_slice(&output.stdout).unwrap();
    let source = run["source_revision"].as_str().unwrap().to_owned();
    let db = Connection::open(&path).unwrap();
    // Inspect the actual closed old-writer database. Its diagnostic JSON version
    // label is not used as evidence of the on-disk schema.
    let old_schema: u32 = db
        .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(old_schema, 3);
    let source_seal = db
        .query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&source],
            |r| r.get(0),
        )
        .unwrap();
    let mut q = db.prepare("PRAGMA table_info(connections)").unwrap();
    let old_columns = q
        .query_map([], |r| r.get(1))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap();
    drop(q);
    let old_rows = rows(&db, &source, &old_columns);
    drop(db);
    let store = SqliteStore::open(&path, "profile", "current-store", 100).unwrap();
    store
        .configure_service_directory(Arc::new(builtin_registry().unwrap()))
        .unwrap();
    Fixture {
        path,
        store,
        source,
        source_seal,
        old_columns,
        old_rows,
    }
}
fn compile(f: &Fixture, op: &str) -> CompiledServiceDraft {
    let s = f.store.service_configuration().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: op.into(),
        expected_profile_generation: s.profile_generation,
        expected_draft_head_revision: s.draft_head_revision,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: "builtin.lmstudio".into(),
                preset_revision: 1,
            },
        },
        purpose: CatalogPurpose::Translation,
        values: [
            (
                "name".into(),
                CatalogScalar::Text("New guided service".into()),
            ),
            (
                "model".into(),
                CatalogScalar::Text("synthetic-model".into()),
            ),
            (
                "template".into(),
                CatalogScalar::Text("Translate {{source}}".into()),
            ),
        ]
        .into_iter()
        .collect(),
        authentication: ServiceAuthenticationSelection::None {},
    };
    compile_service_draft(
        &s.draft,
        &input,
        &builtin_registry().unwrap(),
        &Ids,
        &Default::default(),
    )
    .unwrap()
}
fn preserved(f: &Fixture) {
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&f.source],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        f.source_seal
    );
    assert_eq!(rows(&db, &f.source, &f.old_columns), f.old_rows);
    assert_eq!(db.query_row("SELECT is_suspended,control_revision,config_revision_id FROM connection_runtime_states WHERE connection_id='orphan-0'",[],|r|Ok((r.get::<_,bool>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?))).unwrap(),(true,2,f.source.clone()));
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    )
}
fn inventory(path: &Path) -> Value {
    let db = Connection::open(path).unwrap();
    let count = |table: &str| {
        db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
    };
    let head: Vec<(String, String, u32)> = {
        let mut q = db
            .prepare(
                "SELECT head_kind,revision_id,head_revision FROM config_heads ORDER BY head_kind",
            )
            .unwrap();
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    json!({"head":head,"config":count("config_revisions"),"seals":count("config_revision_seals"),"proofs":count("activation_resource_materializations"),"models":count("models"),"receipts":count("operation_receipts"),"tasks":count("request_tasks")})
}
#[test]
fn real_old_writer_two_orphans_compile_save_reopen_and_stopped_control_remains_stopped() {
    let f = fixture(2);
    let compiled = compile(&f, "first");
    assert_eq!(compiled.graph.models.len(), 1);
    assert_eq!(
        compiled
            .graph
            .connections
            .iter()
            .filter(|c| c.materialization_source.is_some())
            .count(),
        2
    );
    let saved = f.store.save_service_configuration(&compiled, 110).unwrap();
    preserved(&f);
    let db = Connection::open(&f.path).unwrap();
    let(count,ids):(u32,u32)=db.query_row("SELECT count(*),count(DISTINCT materialization_id) FROM activation_resource_materializations WHERE effective_revision_id=?1 AND target_kind='connection'",[&saved.revision_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((count, ids), (2, 2));
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM models WHERE revision_id=?1 AND connection_id LIKE 'orphan-%'",
            [&saved.revision_id],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM request_tasks", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(db);
    let library = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: f
                .store
                .service_configuration()
                .unwrap()
                .profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    assert_eq!(library.rows.len(), 1);
    let second = compile(&f, "second");
    let second = f.store.save_service_configuration(&second, 120).unwrap();
    assert_eq!(
        f.store
            .service_configuration_revision(&second.revision_id)
            .unwrap()
            .as_service_graph()
            .unwrap()
            .connections
            .iter()
            .filter(|c| c.materialization_source.is_some())
            .count(),
        2
    );
    preserved(&f);
    let path = f.path.clone();
    let revision = second.revision_id;
    drop(f.store);
    let reopened = SqliteStore::open(path, "profile", "reopen", 130).unwrap();
    assert_eq!(
        reopened
            .service_configuration_revision(&revision)
            .unwrap()
            .as_service_graph()
            .unwrap()
            .models
            .len(),
        2
    );
}
#[test]
fn public_source_claim_bad_fields_bad_source_and_late_model_all_rollback() {
    let f = fixture(1);
    let mut c = compile(&f, "base");
    let before = inventory(&f.path);
    assert!(
        f.store
            .persist_service_graph_revision(c.graph.clone(), Some(f.source.clone()), 110)
            .is_err()
    );
    assert_eq!(inventory(&f.path), before);
    let connection = c
        .graph
        .connections
        .iter_mut()
        .find(|c| c.connection_id == "orphan-0")
        .unwrap();
    connection.endpoint = Some("http://127.0.0.1:5432/v1".into());
    assert!(f.store.save_service_configuration(&c, 111).is_err());
    assert_eq!(inventory(&f.path), before);
    let mut c = compile(&f, "bad-source");
    c.graph
        .connections
        .iter_mut()
        .find(|c| c.connection_id == "orphan-0")
        .unwrap()
        .materialization_source
        .as_mut()
        .unwrap()
        .revision_id = "not-a-real-source".into();
    assert!(f.store.save_service_configuration(&c, 112).is_err());
    assert_eq!(inventory(&f.path), before);
    let mut late = compile(&f, "late-model");
    late.graph.models[0].connection_id = "orphan-0".into();
    let error = f.store.save_service_configuration(&late, 113).unwrap_err();
    assert!(error.message.contains("invalid_codec3_graph"), "{error:?}");
    assert_eq!(inventory(&f.path), before);
    preserved(&f);
}
#[test]
fn live_connection_source_prevents_gc_then_child_cleanup_precedes_source_gc() {
    let f = fixture(1);
    let saved = f
        .store
        .save_service_configuration(&compile(&f, "saved"), 110)
        .unwrap();
    let before = inventory(&f.path);
    let mut db = Connection::open(&f.path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    {
        let tx = db.transaction().unwrap();
        tx.execute("DELETE FROM config_heads", []).unwrap();
        tx.execute("UPDATE connection_runtime_states SET config_revision_id=?1,control_revision=control_revision+1 WHERE config_revision_id=?2",params![saved.revision_id,f.source]).unwrap();
        let error = tx
            .execute(
                "INSERT INTO config_gc_marks VALUES(?1,'old-save',120)",
                [&f.source],
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("config_gc_still_referenced"),
            "{error}"
        );
    }
    assert_eq!(inventory(&f.path), before);
    preserved(&f);
    {
        let tx = db.transaction().unwrap();
        tx.execute("DELETE FROM config_heads", []).unwrap();
        tx.execute(
            "DELETE FROM connection_runtime_states WHERE config_revision_id=?1",
            [&saved.revision_id],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO config_gc_marks VALUES(?1,'old-save',121)",
            [&saved.revision_id],
        )
        .unwrap();
        tx.execute(
            "DELETE FROM activation_resource_materializations WHERE effective_revision_id=?1",
            [&saved.revision_id],
        )
        .unwrap();
        tx.execute(
            "DELETE FROM connection_runtime_states WHERE config_revision_id=?1",
            [&f.source],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO config_gc_marks VALUES(?1,'old-save',122)",
            [&f.source],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    assert!(f.store.service_configuration_revision(&f.source).is_err());
    assert!(
        f.store
            .service_configuration_revision(&saved.revision_id)
            .is_err()
    );
    assert_eq!(
        db.query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&f.source],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        f.source_seal
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
}
#[test]
fn shared_http_proofs_converge_and_distinct_saved_versions_clone_connection_provenance() {
    let f = fixture(1);
    let mut first = compile(&f, "two-models");
    let primary = first.graph.models[0].model_id.clone();
    let mut second_model = first.graph.models[0].clone();
    second_model.model_id = "secondary-model".into();
    second_model.actual_model_id = Some("secondary-runtime-model".into());
    second_model.display_name = "Secondary".into();
    first.graph.models.push(second_model);
    let first_saved = f.store.save_service_configuration(&first, 110).unwrap();
    let generation = f.store.service_configuration().unwrap().profile_generation;
    let reference = |revision: &str, model: &str| SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: revision.into(),
            model_id: model.into(),
        },
    };
    let first_active = f
        .store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: "same-version-models".into(),
                expected_profile_generation: generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: 0,
                expected_applied_head_revision: 1,
                main: reference(&first_saved.revision_id, &primary),
                comparisons: vec![reference(&first_saved.revision_id, "secondary-model")],
            },
            120,
        )
        .unwrap();
    let mut later = compile(&f, "later-version");
    let last = later.graph.models.last_mut().unwrap();
    let third = last.model_id.clone();
    last.connection_id = first.graph.models[0].connection_id.clone();
    let later_saved = f.store.save_service_configuration(&later, 130).unwrap();
    let selected = f
        .store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: "different-version-models".into(),
                expected_profile_generation: generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: 1,
                expected_applied_head_revision: first_active.applied_head_revision,
                main: reference(&first_saved.revision_id, &primary),
                comparisons: vec![reference(&later_saved.revision_id, &third)],
            },
            140,
        )
        .unwrap();
    let db = Connection::open(&f.path).unwrap();
    let (effective,controls):(u32,u32)=db.query_row("SELECT count(DISTINCT effective_connection_id),count(DISTINCT control_connection_id) FROM activation_resource_materializations WHERE effective_revision_id=?1 AND target_kind='http'",[&selected.revision_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((effective, controls), (2, 1));
    // Controlled corruption fixture: restore the exact original immutable trigger SQL.
    // Every FK remains valid; only convergence of two HTTP proofs is broken.
    let guard:String=db.query_row("SELECT sql FROM sqlite_schema WHERE type='trigger' AND name='v4_activation_resource_materializations_no_update'",[],|r|r.get(0)).unwrap();
    db.execute_batch("DROP TRIGGER v4_activation_resource_materializations_no_update")
        .unwrap();
    let changed=db.execute("UPDATE activation_resource_materializations SET source_revision_id=?1 WHERE effective_revision_id=?2 AND source_model_id='secondary-model' AND target_kind='http'",params![later_saved.revision_id,first_active.revision_id]).unwrap();
    assert_eq!(changed, 1);
    db.execute_batch(&guard).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(db.query_row("SELECT sql FROM sqlite_schema WHERE name='v4_activation_resource_materializations_no_update'",[],|r|r.get::<_,String>(0)).unwrap(),guard);
    let error = f
        .store
        .service_configuration_revision(&first_active.revision_id)
        .unwrap_err();
    assert_eq!(error.code, "conflicting_connection_materialization");
    assert!(
        f.store
            .service_configuration_revision(&selected.revision_id)
            .is_ok()
    );
}
#[test]
fn codec_two_legacy_orphan_source_retains_its_original_normalized_seal() {
    use app_core::ports::ProductRepository;
    let f = fixture(2);
    let state = f.store.service_configuration().unwrap();
    let mut legacy = state.draft.as_legacy().unwrap().clone();
    legacy.codec_version = Some(2);
    // Create a separately edited, valid codec2 source. The original codec1
    // inert legacyPosition metadata is preserved in its original sealed revision.
    for connection in &mut legacy.connections {
        connection.network_metadata.clear();
    }
    let old = f
        .store
        .dispatch(
            CommandEnvelope {
                operation_id: "legacy-codec-two-save".into(),
                expected_revision: None,
                command: app_core::contracts::Command::SaveConfiguration {
                    configuration: legacy,
                    expected_head_revision: state.draft_head_revision,
                },
            },
            105,
        )
        .unwrap();
    let old_revision = old.result_id.unwrap();
    let db = Connection::open(&f.path).unwrap();
    let old_seal: String = db
        .query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&old_revision],
            |r| r.get(0),
        )
        .unwrap();
    let codec: u32 = db
        .query_row(
            "SELECT codec_version FROM config_revisions WHERE revision_id=?1",
            [&old_revision],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(codec, 2);
    let old_document = f
        .store
        .service_configuration_revision(&old_revision)
        .unwrap();
    let expected = app_core::domain::model_config::codec::encode_configuration(
        old_document.as_legacy().unwrap(),
    )
    .unwrap();
    assert_eq!(content_hash(&expected), old_seal);
    let compiled = compile(&f, "after-codec-two");
    assert!(
        compiled
            .graph
            .connections
            .iter()
            .filter_map(|c| c.materialization_source.as_ref())
            .all(|s| s.revision_id == old_revision)
    );
    let saved = f.store.save_service_configuration(&compiled, 110).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&old_revision],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        old_seal
    );
    assert_eq!(db.query_row("SELECT count(*) FROM activation_resource_materializations WHERE effective_revision_id=?1 AND source_revision_id=?2 AND target_kind='connection'",params![saved.revision_id,old_revision],|r|r.get::<_,u32>(0)).unwrap(),2);
    preserved(&f);
}

#[test]
fn closed_store_single_proof_cannot_redirect_an_orphan_control_to_another_valid_connection() {
    let f = fixture(2);
    let compiled = compile(&f, "single-control-corruption");
    let saved = f.store.save_service_configuration(&compiled, 110).unwrap();
    f.store.checkpoint().unwrap();
    drop(f.store);
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    let guard: String = db.query_row("SELECT sql FROM sqlite_schema WHERE name='v4_activation_resource_materializations_no_update'", [], |row| row.get(0)).unwrap();
    let seal: String = db
        .query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&saved.revision_id],
            |row| row.get(0),
        )
        .unwrap();
    db.execute_batch("DROP TRIGGER v4_activation_resource_materializations_no_update")
        .unwrap();
    assert_eq!(db.execute("UPDATE activation_resource_materializations SET control_connection_id='orphan-1' WHERE effective_revision_id=?1 AND effective_connection_id='orphan-0'", [&saved.revision_id]).unwrap(), 1);
    db.execute_batch(&guard).unwrap();
    assert_eq!(db.query_row("SELECT sql FROM sqlite_schema WHERE name='v4_activation_resource_materializations_no_update'", [], |row| row.get::<_,String>(0)).unwrap(), guard);
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&saved.revision_id],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        seal
    );
    drop(db);
    let reopened =
        SqliteStore::open(&f.path, "profile", "reopened-control-corruption", 120).unwrap();
    assert_eq!(
        reopened
            .service_configuration_revision(&saved.revision_id)
            .unwrap_err()
            .code,
        "invalid_connection_control_source"
    );
    assert!(
        reopened
            .service_configuration_revision(&f.source)
            .unwrap()
            .as_legacy()
            .is_some()
    );
}

#[test]
fn closed_store_rejects_modified_copied_legacy_connection_fields_with_original_seals() {
    for field in ["display_name", "endpoint"] {
        let f = fixture(2);
        let compiled = compile(&f, &format!("source-field-{field}"));
        let saved = f.store.save_service_configuration(&compiled, 110).unwrap();
        let before = f
            .store
            .load_service_graph_revision(saved.revision_id.clone())
            .unwrap();
        assert!(
            before
                .connections
                .iter()
                .any(|c| c.connection_id == "orphan-0")
        );
        f.store.checkpoint().unwrap();
        drop(f.store);
        let db = Connection::open(&f.path).unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        let guards: Vec<(String, String)> = {
            let mut query = db.prepare("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name='connections' ORDER BY name").unwrap();
            query
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let sealed: Vec<(String, String)> = {
            let mut query=db.prepare("SELECT revision_id,content_hash FROM config_revision_seals ORDER BY revision_id").unwrap();
            query
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert!(!guards.is_empty());
        for (name, _) in &guards {
            db.execute_batch(&format!("DROP TRIGGER \"{}\"", name.replace('"', "\"\"")))
                .unwrap();
        }
        let replacement = if field == "endpoint" {
            "http://127.0.0.1:54321/changed"
        } else {
            "altered copied name"
        };
        assert_eq!(db.execute(&format!("UPDATE connections SET {field}=?1 WHERE revision_id=?2 AND connection_id='orphan-0'"),params![replacement,f.source]).unwrap(),1);
        for (_, sql) in &guards {
            db.execute_batch(sql).unwrap();
        }
        for (name, sql) in &guards {
            assert_eq!(
                db.query_row(
                    "SELECT sql FROM sqlite_schema WHERE name=?1",
                    [name],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                *sql
            );
        }
        for (revision, seal) in &sealed {
            assert_eq!(
                db.query_row(
                    "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
                    [revision],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                *seal
            );
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, u32>(0)
            })
            .unwrap(),
            0
        );
        drop(db);
        let reopened = SqliteStore::open(&f.path, "profile", "source-field-reopen", 120).unwrap();
        assert_eq!(
            reopened
                .load_service_graph_revision(saved.revision_id)
                .unwrap_err()
                .code,
            "materialization_field_mismatch",
            "field {field}"
        );
    }
}

#[test]
fn iterative_reader_verifies_long_finite_source_dag_and_rejects_a_closed_store_cycle() {
    // Every graph below is sealed by the real public Store writer. Only the
    // relational edges are assembled as a closed-store reader fixture; this
    // does not assert that the public editor normally emits a 132-edge chain.
    let f = fixture(1);
    let compiled = compile(&f, "long-source-dag");
    // Install the actual registered adapter/preset rows and stable control via
    // the normal compiler save, before persisting the independent sealed graphs.
    f.store.save_service_configuration(&compiled, 110).unwrap();
    let mut graph = compiled.graph;
    graph
        .connections
        .retain(|connection| connection.materialization_source.is_none());
    let model = graph.models[0].model_id.clone();
    let connection = graph.models[0].connection_id.clone();
    let revisions: Vec<String> = (0..=132)
        .map(|index| format!("finite-source-{index:03}"))
        .collect();
    for revision in &revisions {
        graph.revision_id = revision.clone();
        assert_eq!(
            f.store
                .persist_service_graph_revision(graph.clone(), None, 110)
                .unwrap(),
            *revision
        );
    }
    f.store.checkpoint().unwrap();
    drop(f.store);
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    let guard:String=db.query_row("SELECT sql FROM sqlite_schema WHERE name='v4_activation_resource_materializations_sealed_insert'",[],|row|row.get(0)).unwrap();
    db.execute_batch("DROP TRIGGER v4_activation_resource_materializations_sealed_insert")
        .unwrap();
    for (index, pair) in revisions.windows(2).enumerate() {
        db.execute("INSERT INTO activation_resource_materializations VALUES(?1,?2,'http',?3,NULL,?4,?3,NULL,'selected_closure',?5,?5,?5,NULL)",params![format!("finite-proof-{index}"),pair[1],model,pair[0],connection]).unwrap();
    }
    db.execute_batch(&guard).unwrap();
    assert_eq!(db.query_row("SELECT sql FROM sqlite_schema WHERE name='v4_activation_resource_materializations_sealed_insert'",[],|row|row.get::<_,String>(0)).unwrap(),guard);
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap(),
        0
    );
    drop(db);
    let last = revisions.last().unwrap().clone();
    let reopened = SqliteStore::open(&f.path, "profile", "long-source-dag", 120).unwrap();
    let loaded = reopened.load_service_graph_revision(last.clone()).unwrap();
    assert_eq!(loaded.revision_id, last);
    assert_eq!(loaded.models[0].model_id, model);
    reopened.checkpoint().unwrap();
    drop(reopened);
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    db.execute_batch("DROP TRIGGER v4_activation_resource_materializations_sealed_insert")
        .unwrap();
    db.execute("INSERT INTO activation_resource_materializations VALUES('closed-source-cycle',?1,'http',?2,NULL,?3,?2,NULL,'selected_closure',?4,?4,?4,NULL)",params![revisions[0],model,last,connection]).unwrap();
    db.execute_batch(&guard).unwrap();
    assert_eq!(db.query_row("SELECT sql FROM sqlite_schema WHERE name='v4_activation_resource_materializations_sealed_insert'",[],|row|row.get::<_,String>(0)).unwrap(),guard);
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap(),
        0
    );
    drop(db);
    let reopened = SqliteStore::open(&f.path, "profile", "source-cycle", 130).unwrap();
    assert_eq!(
        reopened.load_service_graph_revision(last).unwrap_err().code,
        "materialization_source_cycle"
    );
}

#[test]
fn reader_rechecks_copied_legacy_http_model_and_template_fields() {
    use app_core::ports::ProductRepository;
    for changed in ["model", "template"] {
        let f = fixture(1);
        let snapshot = f.store.service_configuration().unwrap();
        let mut legacy = snapshot.draft.as_legacy().unwrap().clone();
        legacy.models.push(Model {
            default_templates: [("translation".into(), "legacy-template".into())].into(),
            model_id: "legacy-model".into(),
            connection_id: "orphan-0".into(),
            actual_model_id: Some("legacy-http-model".into()),
            display_name: "Legacy HTTP model".into(),
            selection_source: "manual".into(),
            capabilities: Default::default(),
            parameters: Default::default(),
        });
        legacy.templates.push(Template {
            template_id: "legacy-template".into(),
            purpose: "translation".into(),
            display_name: "Legacy template".into(),
            source_kind: "custom".into(),
            source_version: None,
            body: [("user".into(), json!("Translate {{source}}"))].into(),
            variables: vec!["source".into()],
            compatibility: Default::default(),
        });
        let legacy_revision = f
            .store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!("copied-http-source-{changed}"),
                    expected_revision: None,
                    command: app_core::contracts::Command::SaveConfiguration {
                        configuration: legacy,
                        expected_head_revision: snapshot.draft_head_revision,
                    },
                },
                105,
            )
            .unwrap()
            .result_id
            .unwrap();
        let compiled = compile(&f, &format!("copied-http-target-{changed}"));
        let saved = f.store.save_service_configuration(&compiled, 110).unwrap();
        let graph = f
            .store
            .load_service_graph_revision(saved.revision_id.clone())
            .unwrap();
        assert!(
            graph
                .models
                .iter()
                .any(|model| model.model_id == "legacy-model")
        );
        assert!(
            graph
                .templates
                .iter()
                .any(|template| template.template_id == "legacy-template")
        );
        f.store.checkpoint().unwrap();
        drop(f.store);
        let db = Connection::open(&f.path).unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        let table = if changed == "model" {
            "models"
        } else {
            "templates"
        };
        let guards: Vec<(String, String)> = {
            let mut query = db.prepare("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=?1 ORDER BY name").unwrap();
            query
                .query_map([table], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert!(!guards.is_empty());
        let seal: String = db
            .query_row(
                "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
                [&legacy_revision],
                |row| row.get(0),
            )
            .unwrap();
        for (name, _) in &guards {
            db.execute_batch(&format!("DROP TRIGGER \"{}\"", name.replace('"', "\"\"")))
                .unwrap();
        }
        let count = if changed == "model" {
            db.execute("UPDATE models SET actual_model_id='changed-model' WHERE revision_id=?1 AND model_id='legacy-model'",[&legacy_revision]).unwrap()
        } else {
            db.execute("UPDATE templates SET template_body_json=?1 WHERE revision_id=?2 AND template_id='legacy-template'",params![json!({"user":"changed {{source}}"}).to_string(),legacy_revision]).unwrap()
        };
        assert_eq!(count, 1);
        for (_, sql) in &guards {
            db.execute_batch(sql).unwrap();
        }
        for (name, sql) in &guards {
            assert_eq!(
                db.query_row(
                    "SELECT sql FROM sqlite_schema WHERE name=?1",
                    [name],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                *sql
            );
        }
        assert_eq!(
            db.query_row(
                "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
                [&legacy_revision],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            seal
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, u32>(0)
            })
            .unwrap(),
            0
        );
        drop(db);
        let reopened = SqliteStore::open(&f.path, "profile", "http-source-reopen", 120).unwrap();
        assert_eq!(
            reopened
                .load_service_graph_revision(saved.revision_id)
                .unwrap_err()
                .code,
            "materialization_field_mismatch",
            "changed {changed}"
        );
    }
}
