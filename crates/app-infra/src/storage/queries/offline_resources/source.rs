//! Source/config/control closure, mechanically separated from resource row reads.
use super::*;
use rusqlite::types::ValueRef;
use serde::de::DeserializeOwned;
use serde_json::Value;
pub(super) fn read_source(
    conn: &Connection,
    lookup: &OfflineMaterialLookup,
    profile: RuntimeProfileRef,
    pair: LanguagePair,
    route_policy: OfflineRoutePolicy,
    installation: Option<LocalInstallationRef>,
    raw: &mut Vec<(&'static str, Value)>,
) -> AppResult<Option<String>> {
    let source_hash = if let Some(source) = &lookup.source {
        let document = configuration::load_document(conn, &source.revision_id)?;
        let graph = app_core::domain::model_config::service_draft::materialize_base(&document)?;
        let config = graph
            .offline_engine_configs
            .iter()
            .find(|c| c.id == source.config_id)
            .ok_or_else(|| fail("offline_source_missing"))?;
        if config.engine != profile.engine
            || config.backend != profile.backend
            || config.pair != pair
            || config.installation != installation
            || config.route_policy != route_policy
            || !matches!(&config.resolution,OfflineConfigResolution::Resolved{resource_snapshot_id,route_id} if resource_snapshot_id==&lookup.snapshot_id&&route_id==&lookup.route_id)
        {
            return Err(fail("offline_source_changed"));
        }
        let header = one(
            conn,
            "SELECT * FROM config_revisions WHERE revision_id=?1",
            &[&source.revision_id],
        )?;
        let config_seal = one(
            conn,
            "SELECT * FROM config_revision_seals WHERE revision_id=?1",
            &[&source.revision_id],
        )?;
        let config_row = one(
            conn,
            "SELECT * FROM offline_engine_configs WHERE revision_id=?1 AND engine_config_id=?2",
            &[&source.revision_id, &source.config_id],
        )?;
        let alias = objects(
            conn,
            "SELECT * FROM activation_resource_materializations WHERE effective_revision_id=?1 AND effective_offline_config_id=?2 ORDER BY materialization_id",
            &[&source.revision_id, &source.config_id],
        )?;
        let control = alias
            .first()
            .and_then(|r| r.get("control_config_id"))
            .and_then(Value::as_str)
            .unwrap_or(&source.config_id);
        let control_row = one(
            conn,
            "SELECT * FROM offline_config_runtime_states WHERE engine_config_id=?1",
            &[&control],
        )?;
        let hash = canonical_hash(&document)?;
        raw.extend([
            ("source_header", header),
            ("source_seal", config_seal),
            ("source_config", config_row),
            ("source_alias", Value::Array(alias)),
            ("source_control", control_row),
        ]);
        Some(hash)
    } else {
        None
    };
    Ok(source_hash)
}

pub(super) fn objects(
    conn: &Connection,
    sql: &str,
    args: &[&dyn rusqlite::ToSql],
) -> AppResult<Vec<Value>> {
    let mut q = conn.prepare(sql).map_err(err)?;
    let names = q
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    q.query_map(args, |r| {
        let mut value = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            let field = match r.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => Value::from(v),
                ValueRef::Text(v) => Value::String(
                    std::str::from_utf8(v)
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                i,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?
                        .into(),
                ),
                _ => {
                    return Err(rusqlite::Error::InvalidColumnType(
                        i,
                        name.clone(),
                        r.get_ref(i)?.data_type(),
                    ));
                }
            };
            value.insert(name.clone(), field);
        }
        Ok(Value::Object(value))
    })
    .map_err(err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(err)
}
pub(super) fn one(conn: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> AppResult<Value> {
    let mut rows = objects(conn, sql, args)?;
    if rows.len() != 1 {
        return Err(fail("offline_stored_fact_missing"));
    }
    Ok(rows.remove(0))
}
fn decode<T: DeserializeOwned>(value: Value) -> AppResult<T> {
    serde_json::from_value(value).map_err(|_| fail("offline_stored_fact_invalid"))
}
pub(super) fn field<T: DeserializeOwned>(value: &Value, key: &str) -> AppResult<T> {
    decode(
        value
            .get(key)
            .cloned()
            .ok_or_else(|| fail("offline_stored_fact_invalid"))?,
    )
}

/// Schema4 stores the guest worker platform. Core's Crossover profile names its
/// parent execution environment; reconstruct it only from the real FK chain.
pub(super) fn parent_platform(
    conn: &Connection,
    profile: &Value,
    observation: &Value,
) -> AppResult<RuntimePlatform> {
    let stored: RuntimePlatform = field(profile, "platform")?;
    let backend: OfflineBackend = field(profile, "backend")?;
    if backend == OfflineBackend::PlatformNative {
        return Ok(stored);
    }
    let installation: String = field(observation, "installation_id")?;
    let revision: u32 = field(observation, "installation_revision")?;
    let inst = one(
        conn,
        "SELECT * FROM offline_installation_versions WHERE installation_id=?1 AND installation_revision=?2",
        &[&installation, &revision],
    )?;
    let environment: String = field(&inst, "environment_id")?;
    let revision: u32 = field(&inst, "environment_revision")?;
    let env = one(
        conn,
        "SELECT * FROM offline_execution_environments WHERE environment_id=?1 AND environment_revision=?2",
        &[&environment, &revision],
    )?;
    let parent: RuntimePlatform = field(&env, "platform")?;
    let guest = match parent {
        RuntimePlatform::Crossover => RuntimePlatform::Windows,
        _ => parent.clone(),
    };
    if guest != stored
        || field::<RuntimeArchitecture>(profile, "architecture")?
            != field::<RuntimeArchitecture>(&env, "architecture")?
    {
        return Err(fail("offline_environment_runtime_mismatch"));
    }
    Ok(parent)
}
