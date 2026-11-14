//! Relational codec3 persistence. Saves do not activate purposes or move the editor head implicitly.
use super::*;
mod provenance;
use app_core::domain::model_config::{service_codec, service_graph::*};
use serde_json::{Map, Value, json as value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RevisionRole {
    User,
    Activation,
}
impl RevisionRole {
    fn sql(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Activation => "activation",
        }
    }
}
fn failure(code: &str) -> AppError {
    AppError::new(
        code,
        "service_graph_storage",
        "配置版本、来源、关系或封存身份不符",
    )
}
fn purpose(p: CatalogPurpose) -> &'static str {
    match p {
        CatalogPurpose::Translation => "translation",
        CatalogPurpose::Grammar => "grammar",
        CatalogPurpose::ContextWord => "context_word",
    }
}
fn tag<T: serde::Serialize>(v: &T) -> AppResult<String> {
    serde_json::to_value(v)
        .map_err(err)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| failure("invalid_scalar_tag"))
}
fn parameter_rows(
    tx: &Transaction,
    table: &str,
    revision: &str,
    owner: &str,
    engine: Option<&str>,
    parameters: &BTreeMap<String, GenerationParameter>,
) -> AppResult<()> {
    for (key, p) in parameters {
        let (mode, v) = match p {
            GenerationParameter::Inherit {} => ("inherit", None),
            GenerationParameter::Omit {} => ("omit", None),
            GenerationParameter::Set { value } => ("set", Some(json(value)?)),
        };
        let sql = match table {
            "model_parameters" => "INSERT INTO model_parameters VALUES(?1,?2,?3,?4,?5)",
            "binding_parameters" => "INSERT INTO binding_parameters VALUES(?1,?2,?3,?4,?5)",
            "engine_parameters" => "INSERT INTO engine_parameters VALUES(?1,?2,?6,?3,?4,?5)",
            _ => return Err(failure("invalid_parameter_owner")),
        };
        if engine.is_some() {
            tx.execute(sql, params![revision, owner, key, mode, v, engine])
                .map_err(err)?;
        } else {
            tx.execute(sql, params![revision, owner, key, mode, v])
                .map_err(err)?;
        }
    }
    Ok(())
}
fn write_defaults(
    tx: &Transaction,
    revision: &str,
    model: Option<&str>,
    offline: Option<&str>,
    defaults: &ServiceRequestDefaults,
) -> AppResult<()> {
    let defaults_id = id("defaults", &json(&(model, offline, defaults.purpose))?);
    let (sm, sl, tm, tl) = match &defaults.languages {
        ServiceLanguageDefaults::OfflineUseConfigPair {} => {
            ("config_pair", None, "config_pair", None)
        }
        ServiceLanguageDefaults::Http { source, target } => {
            let (sm, sl) = match source {
                SourceLanguageChoice::InheritSource {} => ("inherit_source", None),
                SourceLanguageChoice::Explicit { language } => {
                    ("explicit", Some(language.as_str()))
                }
            };
            let (tm, tl) = match target {
                TargetLanguageChoice::InheritPreference {} => ("inherit_preference", None),
                TargetLanguageChoice::Explicit { language } => {
                    ("explicit", Some(language.as_str()))
                }
            };
            (sm, sl, tm, tl)
        }
    };
    tx.execute("INSERT INTO service_request_defaults VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",params![revision,defaults_id,if model.is_some(){"http"}else{"offline"},model,offline,purpose(defaults.purpose),sm,sl,tm,tl,defaults.context.enabled,defaults.context.max_sentences,tag(&defaults.context.mode)?,defaults.execution.timeout_ms,defaults.execution.concurrency,defaults.execution.retries,defaults.cache.enabled]).map_err(err)?;
    for (position, target) in defaults.fallback.iter().enumerate() {
        let (kind, source, model, offline) = match target {
            SavedServiceRef::Http { source } => (
                "http",
                &source.revision_id,
                Some(source.model_id.as_str()),
                None,
            ),
            SavedServiceRef::Offline { source } => (
                "offline",
                &source.revision_id,
                None,
                Some(source.config_id.as_str()),
            ),
        };
        let saved: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id))",[source],|r|r.get(0)).map_err(err)?;
        if !saved {
            return Err(failure("fallback_source_not_saved"));
        }
        tx.execute(
            "INSERT INTO service_default_fallbacks VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                revision,
                defaults_id,
                position as i64,
                kind,
                source,
                model,
                offline
            ],
        )
        .map_err(err)?;
    }
    Ok(())
}
/// Internal writer seam; callers must already own the transaction and its controlled source authority.
/// Untrusted public save does not accept role=activation or self-asserted legacy materialization.
pub(in crate::storage) fn persist(
    tx: &Transaction,
    graph: &ConfigurationV3,
    parent: Option<&str>,
    role: RevisionRole,
    now: i64,
) -> AppResult<String> {
    persist_impl(tx, graph, PersistOptions {
        parent, role, now, aliases: &[], selections: &BTreeMap::new(), trusted: false,
    })
}
#[derive(Clone, Debug)]
pub(super) struct Materialization {
    pub effective_model: Option<String>,
    pub effective_offline: Option<String>,
    pub source: SavedServiceRef,
    pub reason: String,
}
/// The caller cannot supply SQL provenance. Every row below is rebuilt and proved
/// against a sealed real source, including the original control identity.
pub(super) fn persist_verified(
    tx: &Transaction,
    graph: &ConfigurationV3,
    parent: Option<&str>,
    role: RevisionRole,
    now: i64,
    aliases: &[Materialization],
    selections: &BTreeMap<String, u32>,
) -> AppResult<String> {
    persist_impl(tx, graph, PersistOptions {
        parent, role, now, aliases, selections, trusted: true,
    })
}
struct PersistOptions<'a> {
    parent: Option<&'a str>,
    role: RevisionRole,
    now: i64,
    aliases: &'a [Materialization],
    selections: &'a BTreeMap<String, u32>,
    trusted: bool,
}
fn persist_impl(
    tx: &Transaction,
    graph: &ConfigurationV3,
    options: PersistOptions<'_>,
) -> AppResult<String> {
    let PersistOptions { parent, role, now, aliases, selections, trusted } = options;
    if now < 0 {
        return Err(failure("invalid_time"));
    }
    let bytes = service_codec::encode_service_graph(graph)?;
    if role == RevisionRole::User && !trusted {
        service_codec::decode_service_graph_import(&bytes)?;
    }
    if !trusted
        && (graph
            .connections
            .iter()
            .any(|c| c.materialization_source.is_some())
            || graph
                .models
                .iter()
                .any(|m| m.legacy_eligibility_origin.is_some()))
    {
        return Err(failure("untrusted_materialization_claim"));
    }
    if let Some(parent) = parent {
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id))",[parent],|r|r.get(0)).map_err(err)?;
        if !valid {
            return Err(failure("invalid_parent_revision"));
        }
    }
    let hash = content_hash(&bytes);
    let existing:Option<(String,u32,String,Option<String>)>=tx.query_row("SELECT content_hash,codec_version,revision_role,parent_revision_id FROM config_revisions WHERE revision_id=?1",[&graph.revision_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
    if let Some((prior, codec, prior_role, prior_parent)) = existing {
        if codec != 3
            || prior != hash
            || prior_role != role.sql()
            || prior_parent.as_deref() != parent
        {
            return Err(failure("configuration_revision_conflict"));
        }
        if service_codec::encode_service_graph(&load(tx, &graph.revision_id)?)? != bytes {
            return Err(failure("configuration_revision_conflict"));
        }
        return Ok(graph.revision_id.clone());
    }
    tx.execute("INSERT INTO config_revisions(revision_id,parent_revision_id,content_hash,created_at_ms,save_reason,local_validation_json,codec_version,revision_role) VALUES(?1,?2,?3,?4,'edit','{}',3,?5)",params![graph.revision_id,parent,hash,now,role.sql()]).map_err(err)?;
    for c in &graph.connections {
        tx.execute("INSERT INTO connections(revision_id,connection_id,display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,adapter_binding_schema,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,provider_preset_id,provider_preset_revision,http_options_schema,http_options_json,origin_schema,origin_json,oauth_grant_id,oauth_authorization_epoch) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)",params![graph.revision_id,c.connection_id,c.display_name,c.protocol,c.deployment,c.auth_mode,c.endpoint,c.credential_pool_id,c.credential_pool_revision.map(|v|v.get()),c.proxy_mode,json(&c.network_metadata)?,c.availability_intent,c.adapter_binding.as_ref().map(|_|1),c.adapter_binding.as_ref().map(|b|b.protocol.contract_revision),c.adapter_binding.as_ref().map(|b|&b.profile.profile_id),c.adapter_binding.as_ref().map(|b|b.profile.profile_revision),c.preset_source.as_ref().map(|p|&p.preset_id),c.preset_source.as_ref().map(|p|p.preset_revision),c.http_options.as_ref().map(|_|1),c.http_options.as_ref().map(json).transpose()?,c.origin.as_ref().map(|_|1),c.origin.as_ref().map(json).transpose()?,c.google_authorization.as_ref().map(|g|&g.grant_id),c.google_authorization.as_ref().map(|g|g.authorization_epoch.get())]).map_err(err)?;
    }
    for m in &graph.models {
        tx.execute("INSERT INTO models(revision_id,model_id,connection_id,actual_model_id,display_name,selection_source,capability_json,template_purpose) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![graph.revision_id,m.model_id,m.connection_id,m.actual_model_id,m.display_name,m.selection_source,json(&m.capabilities)?,m.template_purpose.map(purpose)]).map_err(err)?;
        parameter_rows(
            tx,
            "model_parameters",
            &graph.revision_id,
            &m.model_id,
            None,
            &m.parameters,
        )?;
    }
    for t in &graph.templates {
        tx.execute(
            "INSERT INTO templates VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                graph.revision_id,
                t.template_id,
                purpose(t.purpose),
                t.display_name,
                t.source_kind,
                t.source_version,
                json(&t.body)?,
                json(&t.variables)?,
                json(&t.compatibility)?,
                canonical_hash(&(purpose(t.purpose), &t.body, &t.variables, &t.compatibility))?
            ],
        )
        .map_err(err)?;
    }
    for m in &graph.models {
        for (p, t) in &m.default_templates {
            tx.execute(
                "INSERT INTO model_default_templates VALUES(?1,?2,?3,?4)",
                params![graph.revision_id, m.model_id, purpose(*p), t],
            )
            .map_err(err)?;
        }
    }
    for o in &graph.offline_engine_configs {
        let (route, consent) = match o.route_policy {
            OfflineRoutePolicy::DirectOnly {} => ("direct_only", None),
            OfflineRoutePolicy::ExplicitPivot { consent_revision } => {
                ("explicit_pivot", Some(consent_revision.get()))
            }
        };
        let (resolution, snapshot, route_id) = match &o.resolution {
            OfflineConfigResolution::Draft {
                observed_resource,
                proposed_route,
            } => (
                "draft",
                observed_resource.as_deref(),
                proposed_route.as_deref(),
            ),
            OfflineConfigResolution::Resolved {
                resource_snapshot_id,
                route_id,
            } => (
                "resolved",
                Some(resource_snapshot_id.as_str()),
                Some(route_id.as_str()),
            ),
        };
        tx.execute("INSERT INTO offline_engine_configs VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![graph.revision_id,o.id,o.engine.engine_id,o.engine.contract_revision.get(),tag(&o.backend)?,o.display_name,o.pair.source,o.pair.target,route,consent,o.installation.as_ref().map(|i|&i.installation_id),o.installation.as_ref().map(|i|i.revision.get()),resolution,snapshot,route_id]).map_err(err)?;
    }
    for m in &graph.models {
        for d in m.request_defaults.values() {
            write_defaults(tx, &graph.revision_id, Some(&m.model_id), None, d)?;
        }
    }
    for o in &graph.offline_engine_configs {
        write_defaults(
            tx,
            &graph.revision_id,
            None,
            Some(&o.id),
            &o.request_defaults,
        )?;
    }
    for b in &graph.bindings {
        tx.execute("INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,game_id,purpose,binding_mode,enabled,output_mode,source_language,target_language,template_id,context_json,schedule_json,semantics_version,selection_revision,policy_overrides_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?16,?15)",params![graph.revision_id,b.binding_id,b.scope_key,b.game_id,purpose(b.purpose),b.binding_mode,b.enabled,b.output_mode,b.source_language,b.target_language,b.template_id,json(&b.context)?,json(&b.schedule)?,if b.semantics_version==PurposeSemanticsVersion::TypedV3{3}else{2},json(&b.scope_overrides)?,selections.get(&b.binding_id).copied().unwrap_or(0)]).map_err(err)?;
        parameter_rows(
            tx,
            "binding_parameters",
            &graph.revision_id,
            &b.binding_id,
            None,
            &b.parameters,
        )?;
        for (position, e) in b.engines.iter().enumerate() {
            let (kind, model, offline) = match &e.target {
                EngineTarget::Http { model_id } => ("http", Some(model_id.as_str()), None),
                EngineTarget::Offline { offline_config_id } => {
                    ("offline", None, Some(offline_config_id.as_str()))
                }
            };
            tx.execute("INSERT INTO binding_engines(revision_id,binding_id,engine_id,purpose,model_id,position,enabled,template_id,target_kind,offline_engine_config_id,policy_overrides_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![graph.revision_id,b.binding_id,e.engine_id,purpose(b.purpose),model,position as i64,e.enabled,e.http_template_override,kind,offline,json(&e.policy_overrides)?]).map_err(err)?;
            parameter_rows(
                tx,
                "engine_parameters",
                &graph.revision_id,
                &b.binding_id,
                Some(&e.engine_id),
                &e.http_parameters,
            )?;
        }
    }
    for set in &graph.term_sets {
        tx.execute(
            "INSERT INTO term_sets VALUES(?1,?2,?3,?4,?5,?6,'{}',?7)",
            params![
                graph.revision_id,
                set.term_set_id,
                set.scope_key,
                set.game_id,
                set.display_name,
                set.enabled,
                canonical_hash(&set.terms)?
            ],
        )
        .map_err(err)?;
        for (position, t) in set.terms.iter().enumerate() {
            tx.execute(
                "INSERT INTO terms VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'{}')",
                params![
                    graph.revision_id,
                    set.term_set_id,
                    t.term_id,
                    t.source_text,
                    t.target_text,
                    t.source_language,
                    t.target_language,
                    position as i64,
                    t.enabled
                ],
            )
            .map_err(err)?;
        }
    }
    if trusted {
        let mut all = aliases.to_vec();
        for model in &graph.models {
            if let Some(source) = &model.legacy_eligibility_origin
                && !all
                    .iter()
                    .any(|m| m.effective_model.as_deref() == Some(&model.model_id))
                {
                    all.push(Materialization {
                        effective_model: Some(model.model_id.clone()),
                        effective_offline: None,
                        source: SavedServiceRef::Http {
                            source: source.clone(),
                        },
                        reason: "legacy_materialization".into(),
                    });
                }
        }
        for materialization in &all {
            write_proved_materialization(tx, graph, materialization)?;
        }
        for connection in graph
            .connections
            .iter()
            .filter(|c| c.materialization_source.is_some())
        {
            if !graph
                .models
                .iter()
                .any(|model| model.connection_id == connection.connection_id)
            {
                write_verified_legacy_connection_proof(tx, graph, connection)?;
            }
        }
    }
    tx.execute(
        "INSERT INTO config_revision_seals VALUES(?1,?2,?3)",
        params![graph.revision_id, hash, now],
    )
    .map_err(err)?;
    if trusted {
        super::service_selection::ensure_controls(tx, graph, now)?;
    }
    let reloaded = load(tx, &graph.revision_id)?;
    if service_codec::encode_service_graph(&reloaded)? != bytes {
        return Err(failure("configuration_roundtrip_mismatch"));
    }
    Ok(graph.revision_id.clone())
}

fn objects(
    conn: &Connection,
    sql: &str,
    arguments: &[&dyn rusqlite::ToSql],
) -> AppResult<Vec<Map<String, Value>>> {
    let mut q = conn.prepare(sql).map_err(err)?;
    let names = q
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut rows = q.query(arguments).map_err(err)?;
    while let Some(row) = rows.next().map_err(err)? {
        let mut object = Map::new();
        for (i, name) in names.iter().enumerate() {
            let value = match row.get_ref(i).map_err(err)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(n) => {
                    if matches!(
                        name.as_str(),
                        "enabled" | "availabilityIntent" | "contextEnabled" | "cacheEnabled"
                    ) {
                        if !matches!(n, 0 | 1) {
                            return Err(failure("invalid_boolean_storage"));
                        }
                        value!(n == 1)
                    } else {
                        value!(n)
                    }
                }
                rusqlite::types::ValueRef::Real(n) => value!(n),
                rusqlite::types::ValueRef::Text(t) => value!(std::str::from_utf8(t).map_err(err)?),
                _ => return Err(failure("invalid_graph_storage_type")),
            };
            object.insert(name.clone(), value);
        }
        result.push(object);
    }
    Ok(result)
}
fn parse_field(row: &mut Map<String, Value>, key: &str) -> AppResult<()> {
    if let Some(Value::String(text)) = row.get(key) {
        service_codec::validate_unique_service_json(text.as_bytes())?;
        let value: Value = serde_json::from_str(text).map_err(err)?;
        row.insert(key.into(), value);
    }
    Ok(())
}
fn parameters(
    conn: &Connection,
    revision: &str,
    table: &str,
    owner: &str,
    engine: Option<&str>,
) -> AppResult<Value> {
    let sql = match table {
        "model_parameters" => {
            "SELECT parameter_key AS key,mode,value_json AS value FROM model_parameters WHERE revision_id=?1 AND model_id=?2 AND ?3 IS NULL ORDER BY parameter_key"
        }
        "binding_parameters" => {
            "SELECT parameter_key AS key,mode,value_json AS value FROM binding_parameters WHERE revision_id=?1 AND binding_id=?2 AND ?3 IS NULL ORDER BY parameter_key"
        }
        "engine_parameters" => {
            "SELECT parameter_key AS key,mode,value_json AS value FROM engine_parameters WHERE revision_id=?1 AND binding_id=?2 AND engine_id=?3 ORDER BY parameter_key"
        }
        _ => return Err(failure("invalid_parameter_owner")),
    };
    let rows = objects(conn, sql, &[&revision, &owner, &engine])?;
    let mut result = Map::new();
    for mut row in rows {
        let key = row
            .remove("key")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or_else(|| failure("invalid_parameter_key"))?;
        if row["mode"] == "set" {
            parse_field(&mut row, "value")?;
        } else {
            row.remove("value");
        }
        result.insert(key, Value::Object(row));
    }
    Ok(Value::Object(result))
}
fn defaults(
    conn: &Connection,
    revision: &str,
    model: Option<&str>,
    offline: Option<&str>,
) -> AppResult<Map<String, Value>> {
    let rows = objects(
        conn,
        "SELECT defaults_id AS id,purpose,source_mode AS sourceMode,source_language AS sourceLanguage,target_mode AS targetMode,target_language AS targetLanguage,context_enabled AS contextEnabled,context_sentences AS contextSentences,context_mode AS contextMode,timeout_ms AS timeoutMs,concurrency,retries,cache_enabled AS cacheEnabled FROM service_request_defaults WHERE revision_id=?1 AND (model_id=?2 OR offline_engine_config_id=?3) ORDER BY purpose",
        &[&revision, &model, &offline],
    )?;
    let mut result = Map::new();
    for row in rows {
        let fallback=objects(conn,"SELECT target_kind AS kind,source_revision_id AS revisionId,source_model_id AS modelId,source_offline_config_id AS configId FROM service_default_fallbacks WHERE revision_id=?1 AND defaults_id=?2 ORDER BY position",&[&revision,&row["id"].as_str().unwrap_or_default()])?.into_iter().map(|r|if r["kind"]=="http"{value!({"kind":"http","source":{"revisionId":r["revisionId"],"modelId":r["modelId"]}})}else{value!({"kind":"offline","source":{"revisionId":r["revisionId"],"configId":r["configId"]}})}).collect::<Vec<_>>();
        let languages = if row["sourceMode"] == "config_pair" {
            value!({"kind":"offline_use_config_pair"})
        } else {
            let source = if row["sourceMode"] == "explicit" {
                value!({"kind":"explicit","language":row["sourceLanguage"]})
            } else {
                value!({"kind":"inherit_source"})
            };
            let target = if row["targetMode"] == "explicit" {
                value!({"kind":"explicit","language":row["targetLanguage"]})
            } else {
                value!({"kind":"inherit_preference"})
            };
            value!({"kind":"http","source":source,"target":target})
        };
        result.insert(row["purpose"].as_str().ok_or_else(||failure("invalid_default_purpose"))?.into(),value!({"purpose":row["purpose"],"languages":languages,"context":{"enabled":row["contextEnabled"],"maxSentences":row["contextSentences"],"mode":row["contextMode"]},"execution":{"timeoutMs":row["timeoutMs"],"concurrency":row["concurrency"],"retries":row["retries"]},"cache":{"enabled":row["cacheEnabled"]},"fallback":fallback}));
    }
    Ok(result)
}
/// Load only the trusted SQL codec3 branch; old Configuration decoder is never reinterpreted.
pub(in crate::storage) fn load(conn: &Connection, revision: &str) -> AppResult<ConfigurationV3> {
    provenance::load_verified(conn, revision)
}
fn load_raw(conn: &Connection, revision: &str) -> AppResult<ConfigurationV3> {
    let identity: Option<(u32, String, String)> = conn
        .query_row(
            "SELECT c.codec_version,c.content_hash,s.content_hash FROM config_revisions c JOIN config_revision_seals s ON s.revision_id=c.revision_id WHERE c.revision_id=?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=c.revision_id)",
            [revision],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((codec, metadata_hash, seal)) = identity else {
        return Err(failure("configuration_not_found"));
    };
    if codec != 3 {
        return Err(failure("unsupported_configuration_codec"));
    }
    if metadata_hash != seal {
        return Err(failure("configuration_seal_mismatch"));
    }
    let mut connections = objects(
        conn,
        "SELECT connection_id AS connectionId,display_name AS displayName,protocol,deployment,auth_mode AS authMode,endpoint,credential_pool_id AS credentialPoolId,credential_pool_revision AS credentialPoolRevision,proxy_mode AS proxyMode,network_metadata_json AS networkMetadata,availability_intent AS availabilityIntent,adapter_binding_schema AS adapterSchema,adapter_contract_revision AS adapterRevision,compatibility_profile_id AS profileId,compatibility_profile_revision AS profileRevision,provider_preset_id AS presetId,provider_preset_revision AS presetRevision,http_options_schema AS optionsSchema,http_options_json AS httpOptions,origin_schema AS originSchema,origin_json AS origin,oauth_grant_id AS grantId,oauth_authorization_epoch AS grantEpoch FROM connections WHERE revision_id=?1 ORDER BY connection_id",
        &[&revision],
    )?;
    for row in &mut connections {
        parse_field(row, "networkMetadata")?;
        parse_field(row, "httpOptions")?;
        parse_field(row, "origin")?;
        for (schema, payload) in [("optionsSchema", "httpOptions"), ("originSchema", "origin")] {
            if !(row[schema].is_null() && row[payload].is_null()
                || row[schema] == 1 && !row[payload].is_null())
            {
                return Err(failure("unsupported_graph_field_codec"));
            }
        }
        row.insert("adapterBinding".into(),if row["adapterSchema"].is_null(){Value::Null}else{if row["adapterSchema"]!=1{return Err(failure("unsupported_adapter_schema"));}value!({"protocol":{"protocolId":row["protocol"],"contractRevision":row["adapterRevision"]},"profile":{"profileId":row["profileId"],"profileRevision":row["profileRevision"]}})});
        row.insert(
            "presetSource".into(),
            if row["presetId"].is_null() {
                Value::Null
            } else {
                value!({"presetId":row["presetId"],"presetRevision":row["presetRevision"]})
            },
        );
        row.insert(
            "googleAuthorization".into(),
            if row["grantId"].is_null() {
                Value::Null
            } else {
                value!({"grantId":row["grantId"],"authorizationEpoch":row["grantEpoch"]})
            },
        );
        if let Some(source) = connection_materialization(
            conn,
            revision,
            row["connectionId"].as_str().unwrap_or_default(),
        )?
            && source.legacy {
                row.insert("materializationSource".into(),value!({"revisionId":source.source_revision_id,"connectionId":source.source_connection_id}));
            }
        for key in [
            "adapterSchema",
            "adapterRevision",
            "profileId",
            "profileRevision",
            "presetId",
            "presetRevision",
            "optionsSchema",
            "originSchema",
            "grantId",
            "grantEpoch",
        ] {
            row.remove(key);
        }
    }
    let mut models = objects(
        conn,
        "SELECT model_id AS modelId,connection_id AS connectionId,actual_model_id AS actualModelId,display_name AS displayName,selection_source AS selectionSource,capability_json AS capabilities,template_purpose AS templatePurpose FROM models WHERE revision_id=?1 ORDER BY model_id",
        &[&revision],
    )?;
    for row in &mut models {
        let model = row["modelId"]
            .as_str()
            .ok_or_else(|| failure("invalid_model_storage"))?
            .to_owned();
        parse_field(row, "capabilities")?;
        row.insert(
            "parameters".into(),
            parameters(conn, revision, "model_parameters", &model, None)?,
        );
        let templates = objects(
            conn,
            "SELECT purpose,template_id AS templateId FROM model_default_templates WHERE revision_id=?1 AND model_id=?2 ORDER BY purpose",
            &[&revision, &model],
        )?;
        row.insert(
            "defaultTemplates".into(),
            Value::Object(
                templates
                    .into_iter()
                    .map(|r| {
                        (
                            r["purpose"].as_str().unwrap_or_default().into(),
                            r["templateId"].clone(),
                        )
                    })
                    .collect(),
            ),
        );
        row.insert(
            "requestDefaults".into(),
            Value::Object(defaults(conn, revision, Some(&model), None)?),
        );
        let source:Option<(String,String)>=conn.query_row("SELECT source_revision_id,source_model_id FROM activation_resource_materializations WHERE effective_revision_id=?1 AND effective_model_id=?2 AND reason='legacy_materialization'",params![revision,model],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
        if let Some((rev, id)) = source {
            row.insert(
                "legacyEligibilityOrigin".into(),
                value!({"revisionId":rev,"modelId":id}),
            );
        }
    }
    let mut offline = objects(
        conn,
        "SELECT engine_config_id AS id,engine_id AS engineId,engine_contract_revision AS engineRevision,backend,display_name AS displayName,source_language AS source,target_language AS target,route_policy AS routePolicyKind,consent_revision AS consentRevision,installation_id AS installationId,installation_revision AS installationRevision,resolution,snapshot_id AS snapshotId,route_id AS routeId FROM offline_engine_configs WHERE revision_id=?1 ORDER BY engine_config_id",
        &[&revision],
    )?;
    for row in &mut offline {
        let id = row["id"]
            .as_str()
            .ok_or_else(|| failure("invalid_offline_storage"))?
            .to_owned();
        row.insert(
            "engine".into(),
            value!({"engineId":row["engineId"],"contractRevision":row["engineRevision"]}),
        );
        row.insert(
            "pair".into(),
            value!({"source":row["source"],"target":row["target"]}),
        );
        row.insert(
            "routePolicy".into(),
            if row["routePolicyKind"] == "direct_only" {
                value!({"kind":"direct_only"})
            } else {
                value!({"kind":"explicit_pivot","consentRevision":row["consentRevision"]})
            },
        );
        row.insert("installation".into(),if row["installationId"].is_null(){Value::Null}else{value!({"installationId":row["installationId"],"revision":row["installationRevision"]})});
        row.insert("resolution".into(),if row["resolution"]=="draft"{value!({"kind":"draft","observedResource":row["snapshotId"],"proposedRoute":row["routeId"]})}else{value!({"kind":"resolved","resourceSnapshotId":row["snapshotId"],"routeId":row["routeId"]})});
        let mut d = defaults(conn, revision, None, Some(&id))?;
        let d = d
            .remove("translation")
            .ok_or_else(|| failure("offline_defaults_missing"))?;
        row.insert("requestDefaults".into(), d);
        for key in [
            "engineId",
            "engineRevision",
            "source",
            "target",
            "routePolicyKind",
            "consentRevision",
            "installationId",
            "installationRevision",
            "snapshotId",
            "routeId",
        ] {
            row.remove(key);
        }
    }
    let mut bindings = objects(
        conn,
        "SELECT binding_id AS bindingId,scope_key AS scopeKey,game_id AS gameId,purpose,binding_mode AS bindingMode,enabled,output_mode AS outputMode,source_language AS sourceLanguage,target_language AS targetLanguage,template_id AS templateId,context_json AS context,schedule_json AS schedule,semantics_version AS semanticsVersion,policy_overrides_json AS scopeOverrides FROM purpose_bindings WHERE revision_id=?1 ORDER BY binding_id",
        &[&revision],
    )?;
    for row in &mut bindings {
        let binding = row["bindingId"]
            .as_str()
            .ok_or_else(|| failure("invalid_binding_storage"))?
            .to_owned();
        for key in ["context", "schedule", "scopeOverrides"] {
            parse_field(row, key)?;
        }
        let semantics = match row["semanticsVersion"].as_u64() {
            Some(2) => "legacy_v2",
            Some(3) => "typed_v3",
            _ => return Err(failure("unsupported_purpose_semantics")),
        };
        row.insert("semanticsVersion".into(), value!(semantics));
        row.insert(
            "parameters".into(),
            parameters(conn, revision, "binding_parameters", &binding, None)?,
        );
        let mut engines = objects(
            conn,
            "SELECT engine_id AS engineId,target_kind AS kind,model_id AS modelId,offline_engine_config_id AS offlineConfigId,enabled,template_id AS httpTemplateOverride,policy_overrides_json AS policyOverrides FROM binding_engines WHERE revision_id=?1 AND binding_id=?2 ORDER BY position",
            &[&revision, &binding],
        )?;
        for engine in &mut engines {
            let id = engine["engineId"]
                .as_str()
                .ok_or_else(|| failure("invalid_engine_storage"))?
                .to_owned();
            parse_field(engine, "policyOverrides")?;
            engine.insert(
                "target".into(),
                if engine["kind"] == "http" {
                    value!({"kind":"http","modelId":engine["modelId"]})
                } else {
                    value!({"kind":"offline","offlineConfigId":engine["offlineConfigId"]})
                },
            );
            engine.insert(
                "httpParameters".into(),
                parameters(conn, revision, "engine_parameters", &binding, Some(&id))?,
            );
            for k in ["kind", "modelId", "offlineConfigId"] {
                engine.remove(k);
            }
        }
        row.insert("engines".into(), value!(engines));
    }
    let mut templates = objects(
        conn,
        "SELECT template_id AS templateId,purpose,display_name AS displayName,source_kind AS sourceKind,source_version AS sourceVersion,template_body_json AS body,template_variables_json AS variables,compatibility_json AS compatibility FROM templates WHERE revision_id=?1 ORDER BY template_id",
        &[&revision],
    )?;
    for row in &mut templates {
        for k in ["body", "variables", "compatibility"] {
            parse_field(row, k)?;
        }
    }
    let mut terms = objects(
        conn,
        "SELECT term_set_id AS termSetId,scope_key AS scopeKey,game_id AS gameId,display_name AS displayName,enabled FROM term_sets WHERE revision_id=?1 ORDER BY term_set_id",
        &[&revision],
    )?;
    for row in &mut terms {
        let id = row["termSetId"]
            .as_str()
            .ok_or_else(|| failure("invalid_term_storage"))?;
        let rows = objects(
            conn,
            "SELECT term_id AS termId,source_text AS sourceText,target_text AS targetText,source_language AS sourceLanguage,target_language AS targetLanguage,enabled FROM terms WHERE revision_id=?1 AND term_set_id=?2 ORDER BY position",
            &[&revision, &id],
        )?;
        row.insert("terms".into(), value!(rows));
    }
    let document = value!({"revisionId":revision,"connections":connections,"models":models,"bindings":bindings,"templates":templates,"termSets":terms,"offlineEngineConfigs":offline,"codecVersion":3});
    let graph =
        service_codec::decode_service_graph_internal(&serde_json::to_vec(&document).map_err(err)?)?;
    if content_hash(service_codec::encode_service_graph(&graph)?) != seal {
        return Err(failure("configuration_seal_mismatch"));
    }
    Ok(graph)
}

impl crate::storage::SqliteStore {
    /// Save a user revision without moving draft/applied heads. Activation is not a public input.
    pub fn persist_service_graph_revision(
        &self,
        graph: ConfigurationV3,
        parent: Option<String>,
        now: i64,
    ) -> AppResult<String> {
        self.call(move |db| {
            let tx = db
                .conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            let id = persist(&tx, &graph, parent.as_deref(), RevisionRole::User, now)?;
            tx.commit().map_err(err)?;
            Ok(id)
        })
    }
    /// Only the editor draft head changes here; purpose enable/selection/runtime state is untouched.
    pub fn save_service_graph_draft(
        &self,
        graph: ConfigurationV3,
        expected_head_revision: u32,
        now: i64,
    ) -> AppResult<String> {
        self.call(move |db| {
            let tx=db.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            let current:Option<(String,u32)>=tx.query_row("SELECT revision_id,head_revision FROM config_heads WHERE head_kind='draft'",[],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
            if current.as_ref().map_or(0,|v|v.1)!=expected_head_revision {return Err(failure("stale_draft_head"));}
            let id=persist(&tx,&graph,current.as_ref().map(|v|v.0.as_str()),RevisionRole::User,now)?;
            if current.is_some(){let changed=tx.execute("UPDATE config_heads SET revision_id=?1,head_revision=head_revision+1,updated_at_ms=?2 WHERE head_kind='draft' AND head_revision=?3",params![id,now,expected_head_revision]).map_err(err)?;if changed!=1{return Err(failure("stale_draft_head"));}}
            else{tx.execute("INSERT INTO config_heads VALUES('draft',?1,1,?2)",params![id,now]).map_err(err)?;}
            tx.commit().map_err(err)?;Ok(id)
        })
    }
    pub fn load_service_graph_revision(&self, revision: String) -> AppResult<ConfigurationV3> {
        self.call(move |db| load(&db.conn, &revision))
    }
}

fn write_proved_materialization(
    tx: &Transaction,
    graph: &ConfigurationV3,
    m: &Materialization,
) -> AppResult<()> {
    let (source_revision, source_model, source_offline) = match &m.source {
        SavedServiceRef::Http { source } => {
            (&source.revision_id, Some(source.model_id.as_str()), None)
        }
        SavedServiceRef::Offline { source } => {
            (&source.revision_id, None, Some(source.config_id.as_str()))
        }
    };
    let document = super::configuration::load_document(tx, source_revision)?;
    let source = app_core::domain::model_config::service_draft::materialize_base(&document)?;
    super::service_selection::ensure_controls(tx, &source, 0)?;
    let mut effective_connection = None;
    let mut source_connection = None;
    let mut control_connection = None;
    let mut control_config = None;
    if let Some(source_id) = source_model {
        let copied = provenance::verify_http_copy(
            &document,
            &source,
            graph,
            provenance::HttpCopyProof {
                source_revision,
                source_id,
                effective_id: m.effective_model
                    .as_deref()
                    .ok_or_else(|| failure("invalid_materialization_target"))?,
                reason: &m.reason,
                scope: provenance::CopyScope::ExactGraph,
            },
        )?;
        effective_connection = Some(copied.effective_connection.clone());
        source_connection = Some(copied.source_connection.clone());
        control_connection = Some(super::service_selection::connection_control(
            tx,
            source_revision,
            &copied.source_connection,
        )?);
    } else {
        let original_id =
            source_offline.ok_or_else(|| failure("invalid_materialization_source"))?;
        provenance::verify_offline_copy(
            &source,
            graph,
            original_id,
            m.effective_offline
                .as_deref()
                .ok_or_else(|| failure("invalid_materialization_target"))?,
        )?;
        control_config=Some(tx.query_row("SELECT coalesce((SELECT control_config_id FROM activation_resource_materializations WHERE effective_revision_id=?1 AND effective_offline_config_id=?2),?2)",params![source_revision,original_id],|r|r.get::<_,String>(0)).map_err(err)?);
    }
    tx.execute("INSERT INTO activation_resource_materializations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![id("materialization",&json(&(&graph.revision_id,&m.effective_model,&m.effective_offline))?),graph.revision_id,if source_model.is_some(){"http"}else{"offline"},m.effective_model,m.effective_offline,source_revision,source_model,source_offline,m.reason,effective_connection,source_connection,control_connection,control_config]).map_err(err)?;
    Ok(())
}

/// All proofs for an effective connection must converge. Multiple HTTP models may
/// share that same proven source; a connection-only proof cannot be model authority.
pub(super) struct ConnectionMaterialization {
    pub source_revision_id: String,
    pub source_connection_id: String,
    pub control_connection_id: String,
    pub legacy: bool,
}
pub(super) fn connection_materialization(
    conn: &Connection,
    revision: &str,
    connection: &str,
) -> AppResult<Option<ConnectionMaterialization>> {
    let initial = read_connection_materialization(conn, revision, connection)?;
    let Some(first) = initial.as_ref() else {
        return Ok(None);
    };
    let mut declared = vec![first.control_connection_id.clone()];
    let mut cursor = (
        first.source_revision_id.clone(),
        first.source_connection_id.clone(),
    );
    let mut visited =
        std::collections::BTreeSet::from([(revision.to_owned(), connection.to_owned())]);
    loop {
        if !visited.insert(cursor.clone()) {
            return Err(failure("connection_materialization_cycle"));
        }
        match read_connection_materialization(conn, &cursor.0, &cursor.1)? {
            Some(proof) => {
                declared.push(proof.control_connection_id);
                cursor = (proof.source_revision_id, proof.source_connection_id);
            }
            None => {
                if declared.iter().any(|control| control != &cursor.1) {
                    return Err(failure("invalid_connection_control_source"));
                }
                return Ok(initial);
            }
        }
    }
}
fn read_connection_materialization(
    conn: &Connection,
    revision: &str,
    connection: &str,
) -> AppResult<Option<ConnectionMaterialization>> {
    let mut q=conn.prepare("SELECT target_kind,source_revision_id,source_connection_id,control_connection_id,reason,effective_model_id,source_model_id,effective_offline_config_id,source_offline_config_id FROM activation_resource_materializations WHERE effective_revision_id=?1 AND effective_connection_id=?2 ORDER BY materialization_id").map_err(err)?;
    let rows = q
        .query_map(params![revision, connection], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut result: Option<ConnectionMaterialization> = None;
    let mut connection_only = false;
    for (
        kind,
        source_revision_id,
        source_connection_id,
        control_connection_id,
        reason,
        model,
        source_model,
        offline,
        source_offline,
    ) in rows
    {
        let valid = match kind.as_str() {
            "http" => {
                model.is_some()
                    && source_model.is_some()
                    && offline.is_none()
                    && source_offline.is_none()
            }
            "connection" => {
                model.is_none()
                    && source_model.is_none()
                    && offline.is_none()
                    && source_offline.is_none()
                    && reason == "legacy_materialization"
            }
            _ => false,
        };
        if !valid {
            return Err(failure("invalid_connection_materialization"));
        }
        if let Some(prior) = &result
            && (kind == "connection"
                || connection_only
                || prior.source_revision_id != source_revision_id
                || prior.source_connection_id != source_connection_id
                || prior.control_connection_id != control_connection_id)
            {
                return Err(failure("conflicting_connection_materialization"));
            }
        let source_codec: Option<u32> = conn.query_row(
            "SELECT r.codec_version FROM config_revisions r JOIN config_revision_seals s USING(revision_id) JOIN connections c ON c.revision_id=r.revision_id WHERE r.revision_id=?1 AND c.connection_id=?2 AND r.content_hash=s.content_hash AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=r.revision_id)",
            params![source_revision_id, source_connection_id], |row| row.get(0),
        ).optional().map_err(err)?;
        if !matches!(source_codec, Some(1..=3)) {
            return Err(failure("invalid_connection_control_source"));
        }
        if kind == "connection" {
            let genuinely_orphan: bool = conn.query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM models WHERE revision_id=?1 AND connection_id=?2) AND NOT EXISTS(SELECT 1 FROM models WHERE revision_id=?3 AND connection_id=?4)",
                params![source_revision_id, source_connection_id, revision, connection], |row| row.get(0),
            ).map_err(err)?;
            if source_codec == Some(3) || !genuinely_orphan {
                return Err(failure("invalid_connection_materialization"));
            }
        } else {
            let exact_models: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM models WHERE revision_id=?1 AND model_id=?2 AND connection_id=?3) AND EXISTS(SELECT 1 FROM models WHERE revision_id=?4 AND model_id=?5 AND connection_id=?6)",
                params![revision, model, connection, source_revision_id, source_model, source_connection_id], |row| row.get(0),
            ).map_err(err)?;
            if !exact_models {
                return Err(failure("invalid_connection_materialization"));
            }
        }
        connection_only = kind == "connection";
        let legacy =
            reason == "legacy_materialization" || result.as_ref().is_some_and(|p| p.legacy);
        result = Some(ConnectionMaterialization {
            source_revision_id,
            source_connection_id,
            control_connection_id,
            legacy,
        });
    }
    Ok(result)
}
fn write_verified_legacy_connection_proof(
    tx: &Transaction,
    graph: &ConfigurationV3,
    effective: &ConnectionV3,
) -> AppResult<()> {
    let source_ref = effective
        .materialization_source
        .as_ref()
        .ok_or_else(|| failure("invalid_connection_materialization"))?;
    if source_ref.revision_id == graph.revision_id
        || graph
            .models
            .iter()
            .any(|m| m.connection_id == effective.connection_id)
    {
        return Err(failure("connection_source_is_not_orphan"));
    }
    let document = super::configuration::load_document(tx, &source_ref.revision_id)?;
    let source = app_core::domain::model_config::service_draft::materialize_base(&document)?;
    provenance::verify_connection_copy(
        &document,
        &source,
        graph,
        &source_ref.connection_id,
        &effective.connection_id,
    )?;
    super::service_selection::ensure_controls(tx, &source, 0)?;
    let control = super::service_selection::connection_control(
        tx,
        &source_ref.revision_id,
        &source_ref.connection_id,
    )?;
    if control != source_ref.connection_id {
        return Err(failure("invalid_connection_control_source"));
    }
    tx.execute("INSERT INTO activation_resource_materializations(materialization_id,effective_revision_id,target_kind,effective_model_id,effective_offline_config_id,source_revision_id,source_model_id,source_offline_config_id,reason,effective_connection_id,source_connection_id,control_connection_id,control_config_id) VALUES(?1,?2,'connection',NULL,NULL,?3,NULL,NULL,'legacy_materialization',?4,?5,?5,NULL)",params![id("materialization",&json(&("connection",&graph.revision_id,&effective.connection_id))?),graph.revision_id,source_ref.revision_id,effective.connection_id,source_ref.connection_id]).map_err(err)?;
    Ok(())
}
