//! Schema-two production producer, per-column boundary and relationship proofs.
//! Controlled Hook inputs test the real writer, not Wine/game compatibility.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
#[allow(dead_code)]
#[path = "storage_acceptance_support/frozen_hosts.rs"]
mod frozen_hosts;
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::{
        ProductRepository,
        runtime::{HookEventInput, HookStageInput},
    },
};
use app_infra::storage::SqliteStore;
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn output() -> PathBuf {
    let fallback = std::env::temp_dir().join(format!("luna-storage-v2-{}", uuid::Uuid::new_v4()));
    acceptance_evidence::directory("schema-two-fields", &fallback)
}
fn source(session: &str) -> HookSourceInput {
    HookSourceInput {
        source_id: format!("source-{session}"),
        host_instance_id: format!("broker-{session}"),
        hook_address_u64: u64::MAX.to_string(),
        context_u64: "0".into(),
        subcontext_u64: "1".into(),
        hook_code: "fixture".into(),
        hook_name: "controlled".into(),
        encoding: "core-decoded-unicode".into(),
        stream_identity_sha256: content_hash(session),
        identity_extensions_json: "{}".into(),
    }
}
fn session(id: &str) -> HookSessionInput {
    HookSessionInput {
        session_id: id.into(),
        game_id: None,
        environment_key: "controlled-env".into(),
        target_exe_path: "C:\\controlled.exe".into(),
        windows_pid: 100,
        process_created_u64: u64::MAX.to_string(),
        architecture: "x86".into(),
        broker_instance_id: format!("broker-{id}"),
        resource: ResourceSnapshotInput {
            snapshot_id: "controlled-core".into(),
            kind: "core".into(),
            version: "fixture".into(),
            manifest_sha256: content_hash("core"),
            contract_version: "abi1-jsonl2".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: content_hash(id),
    }
}
fn event(id: &str, attached: bool) -> HookEventInput {
    HookEventInput {
        event_id: id.into(),
        operation_id: Some("operation-a".into()),
        session_id: attached.then(|| "session-a".into()),
        source: attached.then(|| source("session-a")),
        wire_sequence: attached.then(|| u64::MAX.to_string()),
        observed_at_ms: 11,
        detail: HookStageInput {
            level: "warning".into(),
            stage: "stream".into(),
            code: "typed_loss".into(),
            message: "安全元数据".into(),
        },
        lost_count: attached.then(|| u64::MAX.to_string()),
        integrity: if attached { "lost" } else { "unknown" }.into(),
    }
}
fn fixture(dir: &Path) -> (SqliteStore, PathBuf) {
    let path = dir.join("production.sqlite3");
    let store = SqliteStore::open(&path, "schema-two-profile", "schema-two-server", 1).unwrap();
    store
        .begin_capture_operation("operation-a".into(), content_hash("request-a"), 10)
        .unwrap();
    store
        .record_hook_event(event("before-session", false))
        .unwrap();
    store.open_hook_session(session("session-a"), 12).unwrap();
    store
        .finish_capture_operation("operation-a".into(), Some("session-a".into()), 13)
        .unwrap();
    store.record_hook_event(event("attached", true)).unwrap();
    store
        .ingest_hook_text(
            HookTextInput {
                session_id: "session-a".into(),
                event_id: "raw-a".into(),
                sequence_u64: "1".into(),
                source: source("session-a"),
                text: "最初の候補。".into(),
                source_language: "ja".into(),
            },
            14,
        )
        .unwrap();
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "select-a".into(),
                expected_revision: None,
                command: Command::SelectSource {
                    source_id: "source-session-a".into(),
                },
            },
            15,
        )
        .unwrap();
    store
        .ingest_hook_text(
            HookTextInput {
                session_id: "session-a".into(),
                event_id: "raw-b".into(),
                sequence_u64: "2".into(),
                source: source("session-a"),
                text: "𠮷野の本文🦀が。".into(),
                source_language: "ja".into(),
            },
            16,
        )
        .unwrap();
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "save-preferences".into(),
                expected_revision: None,
                command: Command::SavePreferences {
                    preferences: Preferences {
                        segmentation_enabled: false,
                        reading_display_mode: ReadingDisplayMode::Translation,
                        hud_background_opacity: 0.25,
                        ..Default::default()
                    },
                },
            },
            17,
        )
        .unwrap();
    (store, path)
}
fn scalar(c: &Connection, sql: &str) -> SqlValue {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn prove(
    rows: &mut Vec<Value>,
    table: &str,
    field: &str,
    actual: SqlValue,
    expected: SqlValue,
    invariant: &str,
) {
    assert_eq!(actual, expected, "{table}.{field}: {invariant}");
    rows.push(json!({"id":format!("RUST-MAP:profile.{table}.{field}"),"status":"passed","assertions":1,"skipped":0,"source":format!("storage_session_v2::{invariant}"),"actual_sqlite_type":match actual {SqlValue::Null=>"null",SqlValue::Integer(_)=>"integer",SqlValue::Real(_)=>"real",SqlValue::Text(_)=>"text",SqlValue::Blob(_)=>"blob"}}));
}
#[test]
fn all_29_schema_two_fields_have_real_typed_producer_evidence() {
    let dir = output();
    let (store, path) = fixture(&dir);
    let c = Connection::open(&path).unwrap();
    let mut proofs = vec![];
    let mut check =
        |table: &str, field: &str, where_sql: &str, expected: SqlValue, invariant: &str| {
            prove(
                &mut proofs,
                table,
                field,
                scalar(
                    &c,
                    &format!("SELECT {field} FROM {table} WHERE {where_sql}"),
                ),
                expected,
                invariant,
            )
        };
    use SqlValue::{Integer as I, Real as R, Text as T};
    for (field, value) in [
        ("operation_id", T("operation-a".into())),
        ("request_sha256", T(content_hash("request-a"))),
        ("session_id", T("session-a".into())),
        ("state", T("active".into())),
        ("started_at_ms", I(10)),
        ("ended_at_ms", SqlValue::Null),
        ("legacy_unknown_start", I(0)),
    ] {
        check(
            "capture_operations",
            field,
            "operation_id='operation-a'",
            value,
            "begin_finish_capture_operation",
        );
    }
    for (field, value) in [
        ("event_sequence", I(2)),
        ("event_id", T("attached".into())),
        ("operation_id", T("operation-a".into())),
        ("session_id", T("session-a".into())),
        ("source_id", T("source-session-a".into())),
        ("wire_sequence_u64", T(u64::MAX.to_string())),
        ("observed_at_ms", I(11)),
        ("level", T("warning".into())),
        ("stage", T("stream".into())),
        ("code", T("typed_loss".into())),
        ("message", T("安全元数据".into())),
        ("lost_count_u64", T(u64::MAX.to_string())),
        ("integrity", T("lost".into())),
        ("metadata_json", T("{}".into())),
    ] {
        check(
            "hook_events",
            field,
            "event_id='attached'",
            value,
            "record_hook_event_full_u64_safe_metadata",
        );
    }
    let unit = store
        .snapshot()
        .unwrap()
        .history
        .units
        .into_iter()
        .find(|u| u.source_text == "𠮷野の本文🦀が。")
        .expect("actual ingest must produce a reading unit");
    for (field, value) in [
        ("record_sequence", I(1)),
        ("session_id", T("session-a".into())),
        ("unit_id", T(unit.unit_id.clone())),
        ("text_revision", I(1)),
    ] {
        check(
            "body_record_positions",
            field,
            "record_sequence=1",
            value,
            "actual_selected_ingest_insert_trigger",
        );
    }
    let pref = match scalar(
        &c,
        "SELECT preference_revision_id FROM reading_preference_settings WHERE segmentation_enabled=0",
    ) {
        T(s) => s,
        v => panic!("wrong producer type {v:?}"),
    };
    for (field, value) in [
        ("preference_revision_id", T(pref)),
        ("segmentation_enabled", I(0)),
        ("reading_display_mode", T("translation".into())),
        ("hud_background_opacity", R(0.25)),
    ] {
        check(
            "reading_preference_settings",
            field,
            "segmentation_enabled=0",
            value,
            "save_preferences_typed_revision",
        );
    }
    assert_eq!(proofs.len(), 29);
    assert_eq!(
        proofs
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        29
    );
    // Both null and nonnull producer states are observed, not inferred from DDL.
    for field in [
        "session_id",
        "source_id",
        "wire_sequence_u64",
        "lost_count_u64",
    ] {
        assert_eq!(
            scalar(
                &c,
                &format!("SELECT {field} FROM hook_events WHERE event_id='before-session'")
            ),
            SqlValue::Null
        );
    }
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "disconnect-a".into(),
                expected_revision: None,
                command: Command::Disconnect,
            },
            18,
        )
        .unwrap();
    assert_eq!(
        scalar(
            &c,
            "SELECT ended_at_ms FROM capture_operations WHERE operation_id='operation-a'"
        ),
        I(18)
    );
    assert_eq!(
        scalar(
            &c,
            "SELECT state FROM capture_operations WHERE operation_id='operation-a'"
        ),
        T("stopped".into())
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dictionary: Value = serde_json::from_slice(
        &fs::read(root.join("docs/architecture/database/v2/field-dictionary.json")).unwrap(),
    )
    .unwrap();
    let expected = dictionary["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| format!("RUST-MAP:{}", r["id"].as_str().unwrap()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        proofs
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>(),
        expected
    );
    fs::write(
        dir.join("field-proofs-v2.json"),
        serde_json::to_vec_pretty(&json!({"checks":proofs})).unwrap(),
    )
    .unwrap();
    acceptance_evidence::record(
        "schema-two-production-fields",
        &[("V2-PRODUCERS", 35)],
        json!({"receipt":dir.join("field-proofs-v2.json")}),
    );
}

#[test]
fn persistent_sequences_above_javascript_integer_precision_remain_decimal_strings() {
    let dir = output();
    let (store, path) = fixture(&dir);
    let c = Connection::open(&path).unwrap();
    c.execute(
        "UPDATE sqlite_sequence SET seq=9007199254740993 WHERE name='hook_events'",
        [],
    )
    .unwrap();
    let mut next = event("wide-integer-event", true);
    next.wire_sequence = Some("18446744073709551614".into());
    store.record_hook_event(next).unwrap();
    let page = store
        .session_records(
            SessionRecordQuery {
                session_id: "session-a".into(),
                dataset: SessionDataset::HookEvents,
                order: SessionRecordOrder::OldestFirst,
                filter: SessionRecordFilter::default(),
                cursor: None,
                page_size: 200,
            },
            30,
        )
        .unwrap();
    assert_eq!(page.snapshot.high_watermark, "9007199254740994");
    let record = page.records.iter().find(|row| matches!(row, SessionRecord::HookEvent {event_id, ..} if event_id == "wide-integer-event")).unwrap();
    let encoded = serde_json::to_value(record).unwrap();
    assert_eq!(encoded["sequence"], "9007199254740994");
    assert_eq!(encoded["wireSequence"], "18446744073709551614");
    acceptance_evidence::record(
        "schema-two-string-sequences",
        &[("V2-PRODUCERS", 3)],
        json!({"local_sequence":encoded["sequence"],"wire_sequence":encoded["wireSequence"]}),
    );
}
fn insert(
    c: &Connection,
    table: &str,
    row: &BTreeMap<String, SqlValue>,
) -> rusqlite::Result<usize> {
    let fields = row.keys().cloned().collect::<Vec<_>>();
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        fields.join(","),
        vec!["?"; fields.len()].join(",")
    );
    c.execute(&sql, params_from_iter(fields.iter().map(|key| &row[key])))
}
fn attempt(c: &Connection, table: &str, row: &BTreeMap<String, SqlValue>, expected: Option<&str>) {
    c.execute_batch("SAVEPOINT field_probe;").unwrap();
    if table == "body_record_positions" {
        c.execute("DELETE FROM body_record_positions", []).unwrap();
    }
    if table == "reading_preference_settings" {
        c.execute("DELETE FROM reading_preference_settings", [])
            .unwrap();
    }
    let result = insert(c, table, row);
    match expected {
        None => assert!(result.is_ok(), "{table} valid row failed: {result:?}"),
        Some(text) => {
            let failure = result.expect_err("invalid field must be rejected by real SQLite");
            if text == "TYPE" {
                assert!(
                    matches!(&failure,rusqlite::Error::SqliteFailure(code,_) if code.extended_code==20 || code.extended_code==3091),
                    "{table}: expected SQLite type rejection, got {failure}"
                );
            } else {
                let error = failure.to_string();
                assert!(
                    error.contains(text),
                    "{table}: expected {text}, got {error}"
                );
            }
        }
    }
    c.execute_batch("ROLLBACK TO field_probe; RELEASE field_probe;")
        .unwrap();
}
#[test]
fn every_new_column_has_actual_type_null_and_constraint_probes() {
    let dir = output();
    let (_store, path) = fixture(&dir);
    let c = Connection::open(&path).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    use SqlValue::{Integer as I, Real as R, Text as T};
    let unit = match scalar(&c, "SELECT unit_id FROM body_record_positions LIMIT 1") {
        T(v) => v,
        _ => panic!(),
    };
    let pref = match scalar(
        &c,
        "SELECT preference_revision_id FROM reading_preference_settings LIMIT 1",
    ) {
        T(v) => v,
        _ => panic!(),
    };
    let tables = BTreeMap::from([
        (
            "capture_operations",
            vec![
                ("operation_id", T("probe-op".into())),
                ("request_sha256", T(content_hash("probe"))),
                ("session_id", SqlValue::Null),
                ("state", T("opening".into())),
                ("started_at_ms", I(0)),
                ("ended_at_ms", SqlValue::Null),
                ("legacy_unknown_start", I(0)),
            ],
        ),
        (
            "hook_events",
            vec![
                ("event_sequence", I(999000)),
                ("event_id", T("probe-event".into())),
                ("operation_id", T("operation-a".into())),
                ("session_id", SqlValue::Null),
                ("source_id", SqlValue::Null),
                ("wire_sequence_u64", SqlValue::Null),
                ("observed_at_ms", I(0)),
                ("level", T("info".into())),
                ("stage", T("capture".into())),
                ("code", T("probe_code".into())),
                ("message", T("".into())),
                ("lost_count_u64", SqlValue::Null),
                ("integrity", T("intact".into())),
                ("metadata_json", T("{}".into())),
            ],
        ),
        (
            "body_record_positions",
            vec![
                ("record_sequence", I(999000)),
                ("session_id", T("session-a".into())),
                ("unit_id", T(unit)),
                ("text_revision", I(1)),
            ],
        ),
        (
            "reading_preference_settings",
            vec![
                ("preference_revision_id", T(pref)),
                ("segmentation_enabled", I(1)),
                ("reading_display_mode", T("both".into())),
                ("hud_background_opacity", R(0.7)),
            ],
        ),
    ]);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dictionary: Value = serde_json::from_slice(
        &fs::read(root.join("docs/architecture/database/v2/field-dictionary.json")).unwrap(),
    )
    .unwrap();
    let mut checks = vec![];
    for field in dictionary["fields"].as_array().unwrap() {
        let table = field["table"].as_str().unwrap();
        let column = field["field"].as_str().unwrap();
        let row = tables[table]
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>();
        attempt(&c, table, &row, None);
        let mut wrong = row.clone();
        wrong.insert(column.into(), SqlValue::Blob(vec![0, 255]));
        attempt(&c, table, &wrong, Some("TYPE"));
        let mut null = row.clone();
        null.insert(column.into(), SqlValue::Null);
        let allocator = column == "event_sequence" || column == "record_sequence";
        attempt(
            &c,
            table,
            &null,
            if field["nullable"] == true || allocator {
                None
            } else {
                Some("NOT NULL constraint failed")
            },
        );
        checks.push(json!({"id":format!("V2-FIELD:{}",field["id"].as_str().unwrap()),"status":"passed","assertions":3,"skipped":0}));
    }
    let mut constraint_assertions = 0_u64;
    for (table, column, value) in [
        ("capture_operations", "ended_at_ms", I(0)),
        ("capture_operations", "legacy_unknown_start", I(1)),
        ("hook_events", "wire_sequence_u64", T("0".into())),
        ("hook_events", "lost_count_u64", T(u64::MAX.to_string())),
        ("hook_events", "message", T("文".repeat(4096))),
        (
            "reading_preference_settings",
            "hud_background_opacity",
            R(0.0),
        ),
        (
            "reading_preference_settings",
            "hud_background_opacity",
            R(1.0),
        ),
    ] {
        let mut row = tables[table]
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        row.insert(column.into(), value);
        attempt(&c, table, &row, None);
        constraint_assertions += 1;
    }
    for (table, column, bad) in [
        ("capture_operations", "operation_id", T("".into())),
        ("capture_operations", "request_sha256", T("a".repeat(63))),
        ("capture_operations", "request_sha256", T("a".repeat(65))),
        ("capture_operations", "request_sha256", T("g".repeat(64))),
        ("capture_operations", "state", T("active".into())),
        ("hook_events", "event_id", T("event\0id".into())),
        ("hook_events", "source_id", T("source-session-a".into())),
        ("hook_events", "code", T("".into())),
        ("hook_events", "message", T("m\0n".into())),
        ("hook_events", "wire_sequence_u64", T("".into())),
        ("hook_events", "wire_sequence_u64", T("1".repeat(21))),
        ("hook_events", "wire_sequence_u64", T("1a".into())),
        ("hook_events", "wire_sequence_u64", T("1\0".into())),
        ("hook_events", "lost_count_u64", T("01".into())),
        (
            "hook_events",
            "lost_count_u64",
            T("18446744073709551616".into()),
        ),
        ("hook_events", "metadata_json", T("not json".into())),
        (
            "reading_preference_settings",
            "reading_display_mode",
            T("invented".into()),
        ),
        ("capture_operations", "operation_id", T("a\0b".into())),
        (
            "capture_operations",
            "request_sha256",
            T(format!("{}\0", "a".repeat(64))),
        ),
        ("capture_operations", "request_sha256", T("A".repeat(64))),
        ("capture_operations", "state", T("invented".into())),
        ("capture_operations", "started_at_ms", I(-1)),
        ("capture_operations", "ended_at_ms", I(-1)),
        ("capture_operations", "legacy_unknown_start", I(2)),
        ("hook_events", "event_sequence", I(0)),
        ("hook_events", "event_id", T("".into())),
        ("hook_events", "observed_at_ms", I(-1)),
        ("hook_events", "level", T("debug".into())),
        ("hook_events", "stage", T("invented".into())),
        ("hook_events", "code", T("a\0b".into())),
        ("hook_events", "message", T("a".repeat(4097))),
        (
            "hook_events",
            "wire_sequence_u64",
            T("18446744073709551616".into()),
        ),
        ("hook_events", "wire_sequence_u64", T("01".into())),
        ("hook_events", "lost_count_u64", T("1\0".into())),
        ("hook_events", "integrity", T("assumed".into())),
        ("hook_events", "metadata_json", T("[]".into())),
        ("hook_events", "metadata_json", T("{}\0".into())),
        ("body_record_positions", "record_sequence", I(-1)),
        ("body_record_positions", "text_revision", I(0)),
        ("reading_preference_settings", "segmentation_enabled", I(-1)),
        (
            "reading_preference_settings",
            "reading_display_mode",
            T("both\0".into()),
        ),
        (
            "reading_preference_settings",
            "hud_background_opacity",
            R(-0.01),
        ),
        (
            "reading_preference_settings",
            "hud_background_opacity",
            R(1.01),
        ),
        (
            "reading_preference_settings",
            "hud_background_opacity",
            R(f64::INFINITY),
        ),
    ] {
        let mut row = tables[table]
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>();
        row.insert(column.into(), bad);
        attempt(&c, table, &row, Some("CHECK constraint failed"));
        constraint_assertions += 1;
    }
    for (table, column, bad) in [
        (
            "capture_operations",
            "session_id",
            T("missing-session".into()),
        ),
        ("hook_events", "operation_id", T("missing-operation".into())),
        ("hook_events", "session_id", T("missing-session".into())),
        ("body_record_positions", "unit_id", T("missing-unit".into())),
        (
            "reading_preference_settings",
            "preference_revision_id",
            T("missing-pref".into()),
        ),
    ] {
        let mut row = tables[table]
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>();
        row.insert(column.into(), bad);
        let error = if table == "capture_operations" {
            "capture_operation_requires_hook"
        } else if table == "hook_events" && column == "session_id" {
            "hook_event_operation_session"
        } else {
            "FOREIGN KEY constraint failed"
        };
        attempt(&c, table, &row, Some(error));
        constraint_assertions += 1;
    }
    assert_eq!(checks.len(), 29);
    fs::write(
        dir.join("boundaries-v2.json"),
        serde_json::to_vec_pretty(&json!({"checks":checks})).unwrap(),
    )
    .unwrap();
    acceptance_evidence::record(
        "schema-two-field-boundaries",
        &[("V2-RELATIONSHIPS", constraint_assertions)],
        json!({"receipt":dir.join("boundaries-v2.json")}),
    );
}

#[test]
fn schema_two_composite_relationships_and_immutable_identity_cannot_be_bypassed() {
    let dir = output();
    let (store, path) = fixture(&dir);
    store.open_hook_session(session("session-b"), 20).unwrap();
    store
        .ingest_hook_text(
            HookTextInput {
                session_id: "session-b".into(),
                event_id: "raw-other".into(),
                sequence_u64: "1".into(),
                source: source("session-b"),
                text: "別会話。".into(),
                source_language: "ja".into(),
            },
            21,
        )
        .unwrap();
    let c = Connection::open(&path).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let fails = |sql: &str, expected: &str| {
        let e = c
            .execute_batch(sql)
            .expect_err("reviewed relationship/immutability must reject the write")
            .to_string();
        assert!(e.contains(expected), "{sql}: expected {expected}, got {e}");
    };
    fails(
        "INSERT INTO hook_events(event_id,operation_id,session_id,source_id,observed_at_ms,level,stage,code,message,integrity) VALUES('cross-source','operation-a','session-a','source-session-b',1,'info','capture','fact','','intact')",
        "FOREIGN KEY constraint failed",
    );
    fails(
        "INSERT INTO hook_events(event_id,operation_id,session_id,source_id,observed_at_ms,level,stage,code,message,integrity) VALUES('cross-op','operation-a','session-b','source-session-b',1,'info','capture','fact','','intact')",
        "hook_event_operation_session",
    );
    fails(
        "UPDATE capture_operations SET session_id='session-b' WHERE operation_id='operation-a'",
        "immutable_capture_operation_identity",
    );
    fails(
        "UPDATE capture_operations SET request_sha256=printf('%064d',0) WHERE operation_id='operation-a'",
        "immutable_capture_operation_identity",
    );
    fails(
        "UPDATE capture_operations SET state='opening' WHERE operation_id='operation-a'",
        "capture_operation_transition",
    );
    fails(
        "UPDATE hook_events SET message='rewritten' WHERE event_id='attached'",
        "immutable_hook_events",
    );
    fails(
        "UPDATE body_record_positions SET record_sequence=99",
        "immutable_body_record_positions",
    );
    fails(
        "UPDATE reading_preference_settings SET segmentation_enabled=1",
        "immutable_reading_preference_settings",
    );
    let e = store
        .record_hook_event(HookEventInput {
            event_id: "attached".into(),
            detail: HookStageInput {
                message: "rewritten".into(),
                ..event("attached", true).detail
            },
            ..event("attached", true)
        })
        .unwrap_err();
    assert_eq!(e.code, "event_identity_conflict");
    let e = store
        .begin_capture_operation("operation-a".into(), content_hash("different"), 22)
        .unwrap_err();
    assert_eq!(e.code, "operation_conflict");
    acceptance_evidence::record(
        "schema-two-immutable-relations",
        &[("V2-RELATIONSHIPS", 10)],
        json!({"database":path}),
    );
}

fn inspect_contract(path: &Path, version: u32) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let module = if version == 1 {
        "storage_schema"
    } else if version == 2 {
        "storage_schema_v2"
    } else if version == 3 {
        "storage_schema_v3"
    } else if version == 4 {
        "storage_schema_v4"
    } else if version == 5 {
        "storage_schema_v5"
    } else {
        panic!("unregistered schema contract version {version}")
    };
    let call = if version == 1 {
        "compare_schema"
    } else {
        "compare_current_schema"
    };
    let script = format!(
        "import sys,json;from pathlib import Path;sys.path.insert(0,sys.argv[1]+'/scripts/quality');from {module} import {call};e={call}(Path(sys.argv[1]),Path(sys.argv[2]),'profile');print(json.dumps(e));sys.exit(bool(e))"
    );
    let output = std::process::Command::new("python3")
        .args(["-c", &script])
        .arg(root)
        .arg(path.canonicalize().unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn logical_sha(path: &Path) -> String {
    let output=std::process::Command::new("python3").args(["-c","import sqlite3,sys,hashlib; c=sqlite3.connect(sys.argv[1]); s='\\n'.join(c.iterdump());c.close();print(hashlib.sha256(s.encode()).hexdigest())"]).arg(path).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn seal(path: &Path) {
    let c = Connection::open(path).unwrap();
    c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);PRAGMA journal_mode=DELETE;")
        .unwrap();
    assert_eq!(
        c.query_row::<String, _, _>("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap(),
        "ok"
    );
}
fn snap(dir: &Path, path: &Path) -> Value {
    json!({"path":path.strip_prefix(dir).unwrap(),"sha256":content_hash(fs::read(path).unwrap())})
}
#[test]
fn migration_two_preserves_legacy_data_and_validated_standalone_v1_backup() {
    let dir = output();
    let path = dir.join("upgrade.sqlite3");
    let c = Connection::open(&path).unwrap();
    c.execute_batch(include_str!("../src/storage/migrations/001_profile.sql"))
        .unwrap();
    c.execute_batch(include_str!("../src/storage/migrations/001_config.sql"))
        .unwrap();
    c.execute(
        "INSERT INTO schema_migrations VALUES(1,?1,1,'old',0)",
        [app_infra::storage::profile_migration_sha256()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO profile_meta VALUES(1,'upgrade-profile',0,1,'old','active',1,1)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO resource_snapshots VALUES('r','core','v1',?1,'1','{}',1)",
        [content_hash("legacy-resource")],
    )
    .unwrap();
    c.execute("INSERT INTO sessions VALUES('legacy',NULL,'hook','stopped','env','path',1,'1','x86','broker','r',?1,100,99)",[content_hash("legacy-identity")]).unwrap();
    c.execute("INSERT INTO sessions(session_id,source_kind,state,started_at_ms) VALUES('legacy-manual','manual','stopped',1)",[]).unwrap();
    for (id, n, rev, text) in [
        ("unit-b", 2, 1, "二番目。"),
        ("unit-a", 1, 1, "最初。"),
        ("unit-a", 1, 2, "最初の修正🦀。"),
    ] {
        c.execute(
            "INSERT OR IGNORE INTO reading_units VALUES(?1,'legacy-manual',?2,'finalized',1)",
            rusqlite::params![id, n],
        )
        .unwrap();
        c.execute("INSERT INTO reading_unit_revisions VALUES(?1,?2,'legacy-manual',?3,?4,?5,'ja','v1','{}',NULL,1)",rusqlite::params![id,rev,text,i64::try_from(text.len()).unwrap(),content_hash(text)]).unwrap();
    }
    c.execute("INSERT INTO preference_revisions VALUES('legacy-pref','global',NULL,NULL,'system','material',1,0,18,24,560,'{}','{\"reading.segmentationEnabled\":false}',1)",[]).unwrap();
    drop(c);
    seal(&path);
    let before = dir.join("before-v1.sqlite3");
    fs::copy(&path, &before).unwrap();
    inspect_contract(&before, 1);
    // Keep the original v1→v2 migration/shape assertions as an explicit frozen route.
    let intermediate = dir.join("intermediate-v2.sqlite3");
    fs::copy(&before, &intermediate).unwrap();
    let old_host = frozen_hosts::FrozenHost::two();
    old_host.open(&intermediate, "upgrade-profile");
    let original_v2_backup = dir.join("intermediate-v2.sqlite3.pre-v2-300.sqlite3");
    assert!(original_v2_backup.is_file());
    seal(&original_v2_backup);
    inspect_contract(&original_v2_backup, 1);
    assert_eq!(logical_sha(&before), logical_sha(&original_v2_backup));
    seal(&intermediate);
    inspect_contract(&intermediate, 2);
    // Preserve the exact former v1→v3 route using the actually compiled old
    // production host, alongside the original v2 and current v4 routes.
    let three_path = dir.join("intermediate-v3.sqlite3");
    fs::copy(&before, &three_path).unwrap();
    let three_host = frozen_hosts::FrozenHost::three();
    three_host.open(&three_path, "upgrade-profile");
    let three_backup = dir.join("intermediate-v3.sqlite3.pre-v3-300.sqlite3");
    assert!(three_backup.is_file());
    seal(&three_backup);
    inspect_contract(&three_backup, 1);
    assert_eq!(logical_sha(&before), logical_sha(&three_backup));
    seal(&three_path);
    inspect_contract(&three_path, 3);
    let four_path = dir.join("intermediate-v4.sqlite3");
    fs::copy(&before, &four_path).unwrap();
    frozen_hosts::FrozenHost::four().open(&four_path, "upgrade-profile");
    let four_backup = dir.join("intermediate-v4.sqlite3.pre-v4-100.sqlite3");
    assert!(four_backup.is_file());
    seal(&four_backup);
    inspect_contract(&four_backup, 1);
    assert_eq!(logical_sha(&before), logical_sha(&four_backup));
    seal(&four_path);
    inspect_contract(&four_path, 4);
    let store = SqliteStore::open(&path, "upgrade-profile", "new", 200).unwrap();
    drop(store);
    seal(&path);
    let backup = dir.join("upgrade.sqlite3.pre-v5-200.sqlite3");
    assert!(backup.is_file());
    seal(&backup);
    inspect_contract(&backup, 1);
    inspect_contract(&path, 5);
    assert_eq!(
        logical_sha(&before),
        logical_sha(&backup),
        "online backup must preserve every original schema object and row; SQLite page headers may change"
    );
    for migrated in [&intermediate, &three_path, &four_path, &path] {
        let c = Connection::open(migrated).unwrap();
        let positions = c
            .prepare(
                "SELECT unit_id,text_revision FROM body_record_positions ORDER BY record_sequence",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            positions,
            vec![
                ("unit-a".into(), 1),
                ("unit-a".into(), 2),
                ("unit-b".into(), 1)
            ]
        );
        assert_eq!(
            scalar(
                &c,
                "SELECT legacy_unknown_start FROM capture_operations WHERE session_id='legacy'"
            ),
            SqlValue::Integer(1)
        );
        assert_eq!(
            scalar(
                &c,
                "SELECT ended_at_ms FROM capture_operations WHERE session_id='legacy'"
            ),
            SqlValue::Integer(99)
        );
        assert_eq!(
            scalar(
                &c,
                "SELECT segmentation_enabled FROM reading_preference_settings WHERE preference_revision_id='legacy-pref'"
            ),
            SqlValue::Integer(0)
        );
        assert_eq!(
            scalar(&c, "SELECT count(*) FROM hook_events"),
            SqlValue::Integer(0),
            "migration must never invent old Hook facts"
        );
        drop(c);
    }
    let restored = dir.join("restore-v1.sqlite3");
    fs::copy(&backup, &restored).unwrap();
    assert_eq!(fs::read(&restored).unwrap(), fs::read(&backup).unwrap());
    inspect_contract(&restored, 1);
    let corrupt = dir.join("bad-v2.sqlite3");
    fs::copy(&path, &corrupt).unwrap();
    let c = Connection::open(&corrupt).unwrap();
    c.execute(
        "UPDATE schema_migrations SET migration_sha256=?1 WHERE version=2",
        ["0".repeat(64)],
    )
    .unwrap();
    drop(c);
    seal(&corrupt);
    let bytes = fs::read(&corrupt).unwrap();
    let error = match SqliteStore::open(&corrupt, "upgrade-profile", "rejected", 201) {
        Ok(_) => panic!("changed schema2 migration identity must be rejected"),
        Err(e) => e,
    };
    assert_eq!(error.code, "migration_mismatch");
    assert_eq!(fs::read(&corrupt).unwrap(), bytes);
    let detail = json!({"before":snap(&dir,&before),"after":snap(&dir,&path),"backup":snap(&dir,&backup),"restored":snap(&dir,&restored),"producer":"actual frozen old SqliteStore v1 to v2 and v1 to v3 plus current SqliteStore v1 to v4, each with independently verified backup","schema_two_intermediate":snap(&dir,&intermediate),"schema_two_original_backup":snap(&dir,&original_v2_backup),"schema_three_intermediate":snap(&dir,&three_path),"schema_three_original_backup":snap(&dir,&three_backup),"old_reader_binary_sha256":content_hash(fs::read(&old_host.binary).unwrap()),"old_reader_manifest_sha256":old_host.manifest_sha256,"old_three_reader_manifest_sha256":three_host.manifest_sha256,"legacy_body_revisions":3,"legacy_unknown_start":true,"no_invented_hook_events":true});
    fs::write(
        dir.join("migration-v2.json"),
        serde_json::to_vec_pretty(&detail).unwrap(),
    )
    .unwrap();
    acceptance_evidence::record(
        "schema-two-migration-backup",
        &[("V2-MIGRATION", 13)],
        json!({"receipt":dir.join("migration-v2.json")}),
    );
}
