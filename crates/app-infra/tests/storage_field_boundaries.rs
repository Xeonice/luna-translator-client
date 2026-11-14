//! Rust/production-SQL execution of the frozen field probes. Dynamic SQL probe
//! arguments are not production DTO codecs and never produce RUST-MAP markers.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
#[allow(dead_code)]
#[path = "storage_acceptance_support/frozen_hosts.rs"]
mod frozen_hosts;
use app_core::domain::content_hash;
use app_infra::storage::{SqliteStore, create_dictionary, profile_migration_sha256};
use rusqlite::{
    Connection, ToSql, params_from_iter,
    types::{ToSqlOutput, Value as SqlValue},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
struct Registry {
    fields: Vec<Field>,
    probes: Vec<Probe>,
}
#[derive(Deserialize)]
struct Field {
    id: String,
    database: String,
    table: String,
    field: String,
    #[serde(rename = "type")]
    kind: String,
    nullable: bool,
}
#[derive(Deserialize)]
struct Probe {
    id: String,
    database: String,
    table: String,
    kind: String,
    context: i64,
    mutations: Map<String, Value>,
    expected: Value,
    field: Option<String>,
}
#[derive(Deserialize)]
struct Fixtures {
    tables: BTreeMap<String, Fixture>,
}
#[derive(Deserialize)]
struct Fixture {
    valid_row: Map<String, Value>,
    valid_contexts: Vec<Map<String, Value>>,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn ensure(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
enum Argument {
    Sql(SqlValue),
    Unsigned(u64),
    CheckedSigned(i128),
}
impl ToSql for Argument {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            Self::Sql(value) => value.to_sql(),
            Self::Unsigned(value) => i64::try_from(*value)
                .map(|value| ToSqlOutput::Owned(SqlValue::Integer(value)))
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error))),
            Self::CheckedSigned(value) => i64::try_from(*value)
                .map(|value| ToSqlOutput::Owned(SqlValue::Integer(value)))
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error))),
        }
    }
}
fn argument(value: &Value) -> Result<Argument, String> {
    Ok(match value {
        Value::Null => Argument::Sql(SqlValue::Null),
        Value::String(value) => Argument::Sql(SqlValue::Text(value.clone())),
        Value::Bool(value) => Argument::Sql(SqlValue::Integer(i64::from(*value))),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Argument::Sql(SqlValue::Integer(value))
            } else if let Some(value) = value.as_u64() {
                Argument::Unsigned(value)
            } else {
                Argument::Sql(SqlValue::Real(
                    value.as_f64().ok_or("invalid JSON numeric argument")?,
                ))
            }
        }
        Value::Object(map) if map.len() == 1 && map.contains_key("$integer") => {
            let decimal = map["$integer"]
                .as_str()
                .ok_or("integer boundary must be a decimal string")?;
            if let Ok(value) = decimal.parse::<u64>() {
                Argument::Unsigned(value)
            } else {
                Argument::CheckedSigned(
                    decimal
                        .parse()
                        .map_err(|e| format!("invalid i128 boundary: {e}"))?,
                )
            }
        }
        Value::Object(map) if map.len() == 1 && map.contains_key("$float") => {
            let value = match map["$float"].as_str() {
                Some("nan") => f64::NAN,
                Some("positive_infinity") => f64::INFINITY,
                Some("negative_infinity") => f64::NEG_INFINITY,
                _ => return Err("unrecognized float boundary".into()),
            };
            Argument::Sql(SqlValue::Real(value))
        }
        Value::Object(map) if map.len() == 1 && map.contains_key("$bytes_hex") => {
            let hex = map["$bytes_hex"]
                .as_str()
                .ok_or("blob boundary must be a hex string")?;
            ensure(hex.len() % 2 == 0, "blob hex has an odd length")?;
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|start| u8::from_str_radix(&hex[start..start + 2], 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            Argument::Sql(SqlValue::Blob(bytes))
        }
        _ => return Err("probe arguments must be scalar or explicitly tagged boundaries".into()),
    })
}
fn float_value(value: f64) -> Value {
    if value.is_nan() {
        json!({"$float":"nan"})
    } else if value == f64::INFINITY {
        json!({"$float":"positive_infinity"})
    } else if value == f64::NEG_INFINITY {
        json!({"$float":"negative_infinity"})
    } else {
        json!(value)
    }
}
fn primitive(connection: &Connection, sql: &str) -> Result<(String, Value), String> {
    connection.query_row(sql,[],|row|{
        let kind:String=row.get(1)?;
        let value=match kind.as_str(){
            "null"=>Value::Null,
            "text"=>json!(row.get::<_,String>(0)?),
            "integer"=>json!(row.get::<_,i64>(0)?),
            "real"=>float_value(row.get::<_,f64>(0)?),
            "blob"=>json!({"$bytes_hex":row.get::<_,Vec<u8>>(0)?.iter().map(|v|format!("{v:02x}")).collect::<String>()}),
            _=>return Err(rusqlite::Error::InvalidQuery),
        };
        Ok((kind,value))
    }).map_err(|e|e.to_string())
}
struct Projection {
    connection: Connection,
    table: String,
}
impl Projection {
    fn from_production(
        source: &Connection,
        table: &str,
        fields: &[&Field],
    ) -> Result<Self, String> {
        let sql: String = source
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let connection = Connection::open_in_memory().map_err(|e| e.to_string())?;
        connection
            .execute_batch("PRAGMA foreign_keys=OFF;")
            .map_err(|e| e.to_string())?;
        connection.execute_batch(&sql).map_err(|e| e.to_string())?;
        let mut query=source.prepare("SELECT sql FROM sqlite_schema WHERE type='index' AND tbl_name=?1 AND sql IS NOT NULL ORDER BY name").map_err(|e|e.to_string())?;
        let indexes = query
            .query_map([table], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for index in indexes {
            connection
                .execute_batch(&index)
                .map_err(|e| e.to_string())?;
        }
        let projected: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        ensure(
            projected == sql,
            "field projection changed production table SQL",
        )?;
        let triggers: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='trigger'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        ensure(
            triggers == 0,
            "field projection unexpectedly includes business triggers",
        )?;
        let fk: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        ensure(fk == 0, "field probes must isolate foreign key failures")?;
        let mut query = connection
            .prepare(&format!("PRAGMA table_info({})", quote(table)))
            .map_err(|e| e.to_string())?;
        let columns = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(|e| e.to_string())?;
        let expected = fields
            .iter()
            .map(|field| (field.field.clone(), field.kind.clone(), !field.nullable))
            .collect::<BTreeSet<_>>();
        ensure(
            columns == expected,
            "production column type/nullability set differs from frozen field registry",
        )?;
        drop(query);
        Ok(Self {
            connection,
            table: table.into(),
        })
    }
    fn execute(&self, probe: &Probe, fixture: &Fixture) -> Result<Value, String> {
        let mut row = fixture.valid_row.clone();
        if probe.context >= 0 {
            row.extend(
                fixture
                    .valid_contexts
                    .get(probe.context as usize)
                    .ok_or("probe context missing")?
                    .clone(),
            );
        } else {
            ensure(probe.context == -1, "unknown negative probe context")?;
        }
        row.extend(probe.mutations.clone());
        if probe.kind == "declared_default" {
            row.remove(
                probe
                    .field
                    .as_ref()
                    .ok_or("default probe requires a field")?,
            );
        }
        self.connection
            .execute(&format!("DELETE FROM {}", quote(&self.table)), [])
            .map_err(|e| e.to_string())?;
        let arguments = row.values().map(argument).collect::<Result<Vec<_>, _>>()?;
        let names = row
            .keys()
            .map(|name| quote(name))
            .collect::<Vec<_>>()
            .join(",");
        let placeholders = vec!["?"; arguments.len()].join(",");
        let sql = format!(
            "INSERT INTO {} ({names}) VALUES ({placeholders})",
            quote(&self.table)
        );
        let result = self
            .connection
            .execute(&sql, params_from_iter(arguments.iter()));
        let mut assertions = 1;
        let native = match (probe.expected["outcome"].as_str(), result) {
            (Some("binding_reject"), Err(rusqlite::Error::ToSqlConversionFailure(error))) => {
                assertions += 1;
                json!({"outcome":"binding_reject","phase":"rust_checked_u64_i128_to_sqlite_i64","native_error":error.to_string(),"oracle_phase":probe.expected["phase"],"oracle_error_class":probe.expected["error_class"]})
            }
            (Some("reject"), Err(rusqlite::Error::SqliteFailure(error, message))) => {
                ensure(
                    Some(i64::from(error.extended_code))
                        == probe.expected["sqlite_errorcode"].as_i64(),
                    format!(
                        "actual SQLite code {} differs from {}",
                        error.extended_code, probe.expected["sqlite_errorcode"]
                    ),
                )?;
                let message = message.unwrap_or_default();
                let contains = probe.expected["error_contains"]
                    .as_str()
                    .ok_or("rejection lacks its error attribution")?;
                ensure(
                    message.contains(contains),
                    format!("actual SQLite attribution '{message}' does not contain '{contains}'"),
                )?;
                assertions += 2;
                json!({"outcome":"reject","sqlite_errorcode":error.extended_code,"error":message})
            }
            (Some("accept"), Ok(written)) => {
                ensure(
                    written == 1,
                    "positive probe did not insert exactly one row",
                )?;
                let count: i64 = self
                    .connection
                    .query_row(
                        &format!("SELECT count(*) FROM {}", quote(&self.table)),
                        [],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                ensure(count == 1, "positive probe left an unexpected row count")?;
                assertions += 2;
                let mut native = json!({"outcome":"accept"});
                if let Some(field) = &probe.field {
                    let (kind, value) = primitive(
                        &self.connection,
                        &format!(
                            "SELECT {},typeof({}) FROM {}",
                            quote(field),
                            quote(field),
                            quote(&self.table)
                        ),
                    )?;
                    if let Some(expected) = probe.expected.get("stored_type") {
                        ensure(
                            expected == &kind,
                            format!("actual stored type {kind} differs from {expected}"),
                        )?;
                        assertions += 1;
                    }
                    if let Some(expected) = probe.expected.get("stored_value") {
                        ensure(
                            expected == &value,
                            format!("actual stored primitive {value} differs from {expected}"),
                        )?;
                        assertions += 1;
                    }
                    if let Some(default) = probe.expected["default_sql"].as_str() {
                        let actual_default = primitive(
                            &self.connection,
                            &format!("SELECT ({default}),typeof(({default}))"),
                        )?;
                        ensure(
                            actual_default == (kind.clone(), value.clone()),
                            "omitted field did not store its declared default",
                        )?;
                        assertions += 1;
                    }
                    native["stored_type"] = json!(kind);
                    native["stored_value"] = value;
                }
                native
            }
            (expected, result) => {
                return Err(format!("expected {expected:?}; actual insert {result:?}"));
            }
        };
        Ok(
            json!({"id":probe.id,"status":"passed","assertions":assertions,"skipped":0,"native":native}),
        )
    }
}

#[test]
fn every_frozen_field_probe_runs_against_real_production_sqlite_tables() {
    let root = root();
    let fallback = root.join(".build/storage-rust-boundaries").join(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let out = acceptance_evidence::directory("field-boundaries", &fallback);
    let registry_bytes =
        fs::read(root.join("docs/architecture/database/field-boundaries.json")).unwrap();
    let fixtures_bytes =
        fs::read(root.join("docs/architecture/database/boundary-fixtures.json")).unwrap();
    let registry: Registry = serde_json::from_slice(&registry_bytes).unwrap();
    let fixtures: Fixtures = serde_json::from_slice(&fixtures_bytes).unwrap();
    let profile = out.join("production-profile.sqlite3");
    drop(SqliteStore::open(&profile, "boundary-profile", "boundary-server", 100).unwrap());
    let dictionary = out.join("production-dictionary-schema.sqlite3");
    create_dictionary(&dictionary).unwrap();
    let profiles =
        Connection::open_with_flags(&profile, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    // Keep every original field assertion on the real old writer for the
    // exact eight tables physically extended by reviewed schema3/schema4.
    // Current-schema fields and boundary obligations are a separate matrix;
    // projecting an old registry cannot qualify those new fields.
    let legacy_path = out.join("actual-frozen-store2.sqlite3");
    let legacy_host = frozen_hosts::FrozenHost::two();
    legacy_host.open(&legacy_path, "original-boundary-profile");
    let legacy =
        Connection::open_with_flags(&legacy_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert_eq!(
        legacy
            .query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        2
    );
    let dictionaries =
        Connection::open_with_flags(&dictionary, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let sqlite: (String, String) = profiles
        .query_row("SELECT sqlite_version(),sqlite_source_id()", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    let mut grouped: BTreeMap<String, Vec<&Field>> = BTreeMap::new();
    for field in &registry.fields {
        grouped
            .entry(format!("{}.{}", field.database, field.table))
            .or_default()
            .push(field);
    }
    assert_eq!(
        registry
            .fields
            .iter()
            .map(|f| &f.id)
            .collect::<BTreeSet<_>>()
            .len(),
        registry.fields.len(),
        "frozen field IDs must be unique"
    );
    let mut projections = BTreeMap::new();
    for (key, fields) in &grouped {
        let first = fields[0];
        let source = match first.database.as_str() {
            "profile"
                if matches!(
                    first.table.as_str(),
                    "binding_engines"
                        | "cache_entries"
                        | "config_revisions"
                        | "connections"
                        | "credential_refs"
                        | "models"
                        | "purpose_bindings"
                        | "test_records"
                ) =>
            {
                &legacy
            }
            "profile" => &profiles,
            "dictionary" => &dictionaries,
            other => panic!("unknown production database {other}"),
        };
        projections.insert(
            key.clone(),
            Projection::from_production(source, &first.table, fields)
                .unwrap_or_else(|e| panic!("{key}: {e}")),
        );
    }
    let mut checks = Vec::new();
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();
    for probe in &registry.probes {
        assert!(
            seen.insert(probe.id.clone()),
            "probe IDs must be unique before actual execution"
        );
        let key = format!("{}.{}", probe.database, probe.table);
        let projection = projections
            .get(&key)
            .expect("probe references a real production table");
        let fixture = fixtures
            .tables
            .get(&key)
            .expect("probe references a frozen legal fixture");
        match projection.execute(probe, fixture) {
            Ok(check) => {
                checks.push(check);
            }
            Err(error) => {
                failures.push(json!({"id":probe.id,"error":error}));
                checks.push(json!({"id":probe.id,"status":"failed","assertions":0,"skipped":0,"error":error}));
            }
        }
    }
    let report = json!({"schema_version":1,"kind":"production_rust_sql_field_probe_execution","status":if failures.is_empty(){"passed"}else{"failed"},"not_production_dto_mapping":true,"full_storage_acceptance":"not_issued","projection":"current_production_unchanged_tables_and_eight_expanded_tables_from_real_frozen_store2_without_triggers_foreign_keys_off","frozen_host_manifest":{"path":legacy_host.manifest_path,"sha256":legacy_host.manifest_sha256},"current_schema4_additional_field_boundaries":"not_executed_by_this_original_matrix","registry_sha256":content_hash(registry_bytes),"fixtures_sha256":content_hash(fixtures_bytes),"production_migration_sha256":profile_migration_sha256(),"sqlite_version":sqlite.0,"sqlite_source_id":sqlite.1,"field_count":registry.fields.len(),"checks":checks,"failures":failures});
    fs::write(
        out.join("receipt.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!(
        "STORAGE_RUST_BOUNDARY_RECEIPT {}",
        out.join("receipt.json").display()
    );
    assert!(
        failures.is_empty(),
        "real Rust/SQLite probes failed: {}",
        serde_json::to_string(&failures.iter().take(5).collect::<Vec<_>>()).unwrap()
    );
    assert_eq!(checks.len(), registry.probes.len());
    acceptance_evidence::record(
        "all-sql-field-probes",
        &[("STORAGE-17", checks.len() as u64)],
        json!({"receipt":out.join("receipt.json"),"fields":registry.fields.len(),"probes":checks.len()}),
    );
}
