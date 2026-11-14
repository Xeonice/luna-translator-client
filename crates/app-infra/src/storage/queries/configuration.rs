use super::*;
pub(super) fn save(tx: &Transaction, c: &Configuration, now: i64) -> AppResult<()> {
    let parent: Option<String> = tx
        .query_row(
            "SELECT revision_id FROM config_heads WHERE head_kind='draft'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    let normalized = if c.codec_version == Some(2) {
        Some(app_core::domain::model_config::codec::normalize_service_graph_configuration(c)?)
    } else {
        None
    };
    let c = normalized.as_ref().unwrap_or(c);
    let hash = super::configuration_codec::content_hash(c)?;
    tx.execute(
        "INSERT INTO config_revisions(revision_id,parent_revision_id,content_hash,created_at_ms,save_reason,local_validation_json,codec_version) VALUES(?1,?2,?3,?4,'edit','{}',?5)",
        params![c.revision_id, parent, hash, now,c.codec_version.unwrap_or(1)],
    )
    .map_err(err)?;
    for con in &c.connections {
        tx.execute(
            "INSERT INTO connections(revision_id,connection_id,display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,adapter_binding_schema,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,provider_preset_id,provider_preset_revision) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                c.revision_id,
                con.connection_id,
                con.display_name,
                con.protocol,
                con.deployment,
                con.auth_mode,
                con.endpoint,
                con.credential_pool_id,
                con.credential_pool_revision,
                con.proxy_mode,
                json(&con.network_metadata)?,
                con.availability_intent,
                con.adapter_binding.as_ref().map(|_|1u32),
                con.adapter_binding.as_ref().map(|b|b.protocol.contract_revision),
                con.adapter_binding.as_ref().map(|b|b.profile.profile_id.as_str()),
                con.adapter_binding.as_ref().map(|b|b.profile.profile_revision),
                con.preset_source.as_ref().map(|p|p.preset_id.as_str()),
                con.preset_source.as_ref().map(|p|p.preset_revision)
            ],
        )
        .map_err(err)?;
    }
    for m in &c.models {
        tx.execute(
            "INSERT INTO models(revision_id,model_id,connection_id,actual_model_id,display_name,selection_source,capability_json) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                c.revision_id,
                m.model_id,
                m.connection_id,
                m.actual_model_id,
                m.display_name,
                m.selection_source,
                json(&m.capabilities)?
            ],
        )
        .map_err(err)?;
        for (key, p) in &m.parameters {
            tx.execute(
                "INSERT INTO model_parameters VALUES(?1,?2,?3,?4,?5)",
                params![
                    c.revision_id,
                    m.model_id,
                    key,
                    mode(p),
                    p.value.as_ref().map(json).transpose()?
                ],
            )
            .map_err(err)?;
        }
    }
    for t in &c.templates {
        tx.execute(
            "INSERT INTO templates VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                c.revision_id,
                t.template_id,
                t.purpose,
                t.display_name,
                t.source_kind,
                t.source_version,
                json(&t.body)?,
                json(&t.variables)?,
                json(&t.compatibility)?,
                canonical_hash(&(&t.purpose, &t.body, &t.variables, &t.compatibility))?
            ],
        )
        .map_err(err)?;
    }
    for model in &c.models {
        for (purpose, template) in &model.default_templates {
            tx.execute(
                "INSERT INTO model_default_templates VALUES(?1,?2,?3,?4)",
                params![c.revision_id, model.model_id, purpose, template],
            )
            .map_err(err)?;
        }
    }
    for b in &c.bindings {
        tx.execute(
            "INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,game_id,purpose,binding_mode,enabled,output_mode,source_language,target_language,template_id,context_json,schedule_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                c.revision_id,
                b.binding_id,
                b.scope_key,
                b.game_id,
                b.purpose,
                b.binding_mode,
                b.enabled,
                b.output_mode,
                b.source_language,
                b.target_language,
                b.template_id,
                json(&b.context)?,
                json(&b.schedule)?
            ],
        )
        .map_err(err)?;
        for (key, p) in &b.parameters {
            tx.execute(
                "INSERT INTO binding_parameters VALUES(?1,?2,?3,?4,?5)",
                params![
                    c.revision_id,
                    b.binding_id,
                    key,
                    mode(p),
                    p.value.as_ref().map(json).transpose()?
                ],
            )
            .map_err(err)?;
        }
        for (position, e) in b.engines.iter().enumerate() {
            tx.execute(
                "INSERT INTO binding_engines(revision_id,binding_id,engine_id,purpose,model_id,position,enabled,template_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    c.revision_id,
                    b.binding_id,
                    e.engine_id,
                    b.purpose,
                    e.model_id,
                    position as i64,
                    e.enabled,
                    e.template_id
                ],
            )
            .map_err(err)?;
            for (key, p) in &e.parameters {
                tx.execute(
                    "INSERT INTO engine_parameters VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        c.revision_id,
                        b.binding_id,
                        e.engine_id,
                        key,
                        mode(p),
                        p.value.as_ref().map(json).transpose()?
                    ],
                )
                .map_err(err)?;
            }
        }
    }
    for ts in &c.term_sets {
        tx.execute(
            "INSERT INTO term_sets VALUES(?1,?2,?3,?4,?5,?6,'{}',?7)",
            params![
                c.revision_id,
                ts.term_set_id,
                ts.scope_key,
                ts.game_id,
                ts.display_name,
                ts.enabled,
                canonical_hash(&ts.terms)?
            ],
        )
        .map_err(err)?;
        for (i, t) in ts.terms.iter().enumerate() {
            tx.execute(
                "INSERT INTO terms VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'{}')",
                params![
                    c.revision_id,
                    ts.term_set_id,
                    t.term_id,
                    t.source_text,
                    t.target_text,
                    t.source_language,
                    t.target_language,
                    i as i64,
                    t.enabled
                ],
            )
            .map_err(err)?;
        }
    }
    tx.execute(
        "INSERT INTO config_revision_seals VALUES(?1,?2,?3)",
        params![c.revision_id, hash, now],
    )
    .map_err(err)?;
    tx.execute("INSERT INTO config_heads VALUES('draft',?1,1,?2) ON CONFLICT(head_kind) DO UPDATE SET revision_id=excluded.revision_id,head_revision=config_heads.head_revision+1,updated_at_ms=excluded.updated_at_ms",params![c.revision_id,now]).map_err(err)?;
    Ok(())
}
fn mode(p: &Parameter) -> &'static str {
    match p.mode {
        ParameterMode::Set => "set",
        ParameterMode::Inherit => "inherit",
        ParameterMode::Omit => "omit",
    }
}
fn parameters(
    conn: &Connection,
    table: &str,
    revision: &str,
    owner: &str,
    engine: Option<&str>,
) -> AppResult<BTreeMap<String, Parameter>> {
    let ownercol = if table == "model_parameters" {
        "model_id"
    } else {
        "binding_id"
    };
    let sql = if engine.is_some() {
        format!(
            "SELECT parameter_key,mode,value_json FROM {table} WHERE revision_id=?1 AND {ownercol}=?2 AND engine_id=?3"
        )
    } else {
        format!(
            "SELECT parameter_key,mode,value_json FROM {table} WHERE revision_id=?1 AND {ownercol}=?2 AND ?3 IS NULL"
        )
    };
    let mut q = conn.prepare(&sql).map_err(err)?;
    let rows = q
        .query_map(params![revision, owner, engine], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(err)?;
    rows.map(|r| {
        let (k, m, v) = r.map_err(err)?;
        Ok((
            k,
            Parameter {
                mode: match m.as_str() {
                    "set" => ParameterMode::Set,
                    "omit" => ParameterMode::Omit,
                    _ => ParameterMode::Inherit,
                },
                value: v.map(parse).transpose()?,
            },
        ))
    })
    .collect()
}
pub(super) fn load(conn: &Connection, revision: &str) -> AppResult<Configuration> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=?1)",
            [revision],
            |r| r.get(0),
        )
        .map_err(err)?;
    if !exists {
        return Err(AppError::new(
            "configuration_not_found",
            "configuration",
            "配置版本不存在或尚未封存",
        ));
    }
    let mut c = Configuration {
        revision_id: revision.into(),
        ..Configuration::default()
    };
    let codec: u32 = conn
        .query_row(
            "SELECT codec_version FROM config_revisions WHERE revision_id=?1",
            [revision],
            |r| r.get(0),
        )
        .map_err(err)?;
    c.codec_version = match codec {
        1 => None,
        2 => Some(2),
        _ => {
            return Err(AppError::new(
                "unsupported_configuration_codec",
                "storage",
                "存储配置编码未知",
            ));
        }
    };
    let mut q=conn.prepare("SELECT connection_id,display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,adapter_binding_schema,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,provider_preset_id,provider_preset_revision FROM connections WHERE revision_id=?1 ORDER BY connection_id").map_err(err)?;
    let rows = q
        .query_map([revision], |r| {
            Ok((
                app_core::contracts::Connection {
                    connection_id: r.get(0)?,
                    display_name: r.get(1)?,
                    protocol: r.get(2)?,
                    deployment: r.get(3)?,
                    auth_mode: r.get(4)?,
                    endpoint: r.get(5)?,
                    credential_pool_id: r.get(6)?,
                    credential_pool_revision: r.get(7)?,
                    proxy_mode: r.get(8)?,
                    network_metadata: BTreeMap::new(),
                    availability_intent: r.get(10)?,
                    adapter_binding: if r.get::<_, Option<u32>>(11)?.is_some() {
                        Some(AdapterBinding {
                            protocol: ProtocolKey {
                                protocol_id: r.get(2)?,
                                contract_revision: r.get(12)?,
                            },
                            profile: CompatibilityProfileKey {
                                profile_id: r.get(13)?,
                                profile_revision: r.get(14)?,
                            },
                        })
                    } else {
                        None
                    },
                    preset_source: r
                        .get::<_, Option<String>>(15)?
                        .map(|preset_id| {
                            r.get(16).map(|preset_revision| ProviderPresetKey {
                                preset_id,
                                preset_revision,
                            })
                        })
                        .transpose()?,
                },
                r.get::<_, String>(9)?,
            ))
        })
        .map_err(err)?;
    c.connections = rows
        .map(|r| {
            let (mut v, j) = r.map_err(err)?;
            v.network_metadata = parse(j)?;
            Ok(v)
        })
        .collect::<AppResult<_>>()?;
    let mut q=conn.prepare("SELECT model_id,connection_id,actual_model_id,display_name,selection_source,capability_json FROM models WHERE revision_id=?1 ORDER BY model_id").map_err(err)?;
    c.models = q
        .query_map([revision], |r| {
            Ok((
                Model {
                    default_templates: BTreeMap::new(),
                    model_id: r.get(0)?,
                    connection_id: r.get(1)?,
                    actual_model_id: r.get(2)?,
                    display_name: r.get(3)?,
                    selection_source: r.get(4)?,
                    capabilities: BTreeMap::new(),
                    parameters: BTreeMap::new(),
                },
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(err)?
        .map(|r| {
            let (mut m, j) = r.map_err(err)?;
            m.capabilities = parse(j)?;
            m.parameters = parameters(conn, "model_parameters", revision, &m.model_id, None)?;
            let mut defaults=conn.prepare("SELECT purpose,template_id FROM model_default_templates WHERE revision_id=?1 AND model_id=?2 ORDER BY purpose").map_err(err)?;
            m.default_templates=defaults.query_map(params![revision,m.model_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?.collect::<Result<_,_>>().map_err(err)?;
            Ok(m)
        })
        .collect::<AppResult<_>>()?;
    let mut q=conn.prepare("SELECT template_id,purpose,display_name,source_kind,source_version,template_body_json,template_variables_json,compatibility_json FROM templates WHERE revision_id=?1 ORDER BY template_id").map_err(err)?;
    c.templates = q
        .query_map([revision], |r| {
            Ok((
                Template {
                    template_id: r.get(0)?,
                    purpose: r.get(1)?,
                    display_name: r.get(2)?,
                    source_kind: r.get(3)?,
                    source_version: r.get(4)?,
                    body: BTreeMap::new(),
                    variables: vec![],
                    compatibility: BTreeMap::new(),
                },
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ))
        })
        .map_err(err)?
        .map(|r| {
            let (mut t, b, v, c) = r.map_err(err)?;
            t.body = parse(b)?;
            t.variables = parse(v)?;
            t.compatibility = parse(c)?;
            Ok(t)
        })
        .collect::<AppResult<_>>()?;
    let mut q=conn.prepare("SELECT binding_id,scope_key,game_id,purpose,binding_mode,enabled,output_mode,source_language,target_language,template_id,context_json,schedule_json FROM purpose_bindings WHERE revision_id=?1 ORDER BY binding_id").map_err(err)?;
    c.bindings=q.query_map([revision],|r|Ok((PurposeBinding{binding_id:r.get(0)?,scope_key:r.get(1)?,game_id:r.get(2)?,purpose:r.get(3)?,binding_mode:r.get(4)?,enabled:r.get(5)?,output_mode:r.get(6)?,source_language:r.get(7)?,target_language:r.get(8)?,template_id:r.get(9)?,context:BTreeMap::new(),schedule:BTreeMap::new(),parameters:BTreeMap::new(),engines:vec![]},r.get::<_,String>(10)?,r.get::<_,String>(11)?))).map_err(err)?.map(|r|{let(mut b,c,s)=r.map_err(err)?;b.context=parse(c)?;b.schedule=parse(s)?;b.parameters=parameters(conn,"binding_parameters",revision,&b.binding_id,None)?;let mut q=conn.prepare("SELECT engine_id,model_id,enabled,template_id FROM binding_engines WHERE revision_id=?1 AND binding_id=?2 ORDER BY position").map_err(err)?;b.engines=q.query_map(params![revision,b.binding_id],|r|Ok(Engine{engine_id:r.get(0)?,model_id:r.get(1)?,enabled:r.get(2)?,template_id:r.get(3)?,parameters:BTreeMap::new()})).map_err(err)?.map(|r|{let mut e=r.map_err(err)?;e.parameters=parameters(conn,"engine_parameters",revision,&b.binding_id,Some(&e.engine_id))?;Ok(e)}).collect::<AppResult<_>>()?;Ok(b)}).collect::<AppResult<_>>()?;
    let mut q=conn.prepare("SELECT term_set_id,scope_key,game_id,display_name,enabled FROM term_sets WHERE revision_id=?1 ORDER BY term_set_id").map_err(err)?;
    c.term_sets=q.query_map([revision],|r|Ok(TermSet{term_set_id:r.get(0)?,scope_key:r.get(1)?,game_id:r.get(2)?,display_name:r.get(3)?,enabled:r.get(4)?,terms:vec![]})).map_err(err)?.map(|r|{let mut ts=r.map_err(err)?;let mut q=conn.prepare("SELECT term_id,source_text,target_text,source_language,target_language,enabled FROM terms WHERE revision_id=?1 AND term_set_id=?2 ORDER BY position").map_err(err)?;ts.terms=q.query_map(params![revision,ts.term_set_id],|r|Ok(Term{term_id:r.get(0)?,source_text:r.get(1)?,target_text:r.get(2)?,source_language:r.get(3)?,target_language:r.get(4)?,enabled:r.get(5)?})).map_err(err)?.collect::<Result<_,_>>().map_err(err)?;Ok(ts)}).collect::<AppResult<_>>()?;
    super::configuration_codec::verify_loaded(conn, &c)?;
    Ok(c)
}
/// SQL metadata chooses the decoder. JSON shape never decides its codec.
pub(super) fn load_document(conn: &Connection, revision: &str) -> AppResult<ConfigurationDocument> {
    let saved: Option<(u32, String, String)> = conn.query_row(
        "SELECT c.codec_version,c.content_hash,s.content_hash FROM config_revisions c JOIN config_revision_seals s ON s.revision_id=c.revision_id WHERE c.revision_id=?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=c.revision_id)",
        [revision], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).optional().map_err(err)?;
    let (codec, metadata_hash, seal_hash) = saved.ok_or_else(|| AppError::new(
        "configuration_revision_unavailable", "configuration", "配置版本未封存、已清理或不存在",
    ))?;
    // Legacy-one vectors lost storage order: compare its two original identities,
    // never regenerate a seal from the loaded/sorted DTO.
    if metadata_hash != seal_hash {
        return Err(AppError::new("configuration_seal_mismatch", "configuration", "配置元数据与原封存身份不一致"));
    }
    match codec {
        1 | 2 => Ok(ConfigurationDocument::Legacy { configuration: load(conn, revision)? }),
        3 => Ok(ConfigurationDocument::ServiceGraph { configuration: super::service_graph::load(conn, revision)? }),
        _ => Err(AppError::new("unsupported_configuration_codec", "configuration", "配置编码版本不受支持")),
    }
}
pub(super) fn state(conn: &Connection) -> AppResult<ConfigurationState> {
    let head = |kind: &str| -> AppResult<Option<(String, u32)>> {
        conn.query_row(
            "SELECT revision_id,head_revision FROM config_heads WHERE head_kind=?1",
            [kind],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)
    };
    let d = head("draft")?;
    let a = head("applied")?;
    let applied = a.as_ref().map(|(id, _)| load_document(conn, id)).transpose()?;
    let (runtime_connections, runtime_purposes) =
        super::configuration_lifecycle::document_views(conn, applied.as_ref())?;
    Ok(ConfigurationState {
        draft: d
            .as_ref()
            .map(|(id, _)| load_document(conn, id))
            .transpose()?
            .unwrap_or_else(|| ConfigurationDocument::Legacy { configuration: Configuration::default() }),
        applied,
        draft_head_revision: d.map(|(_, r)| r).unwrap_or(0),
        applied_head_revision: a.map(|(_, r)| r).unwrap_or(0),
        runtime_connections,
        runtime_purposes,
    })
}
pub(super) fn semantic_hash(c: &Configuration, b: &PurposeBinding) -> AppResult<String> {
    semantic_hash_for_scope(c, b, &b.scope_key)
}
fn semantic_hash_for_scope(
    c: &Configuration,
    b: &PurposeBinding,
    scope: &str,
) -> AppResult<String> {
    let inherited_parameters = if b.scope_key != "global" && b.binding_mode == "override" {
        c.bindings
            .iter()
            .find(|global| global.scope_key == "global" && global.purpose == b.purpose)
            .map(|global| {
                let engines = b
                    .engines
                    .iter()
                    .map(|engine| {
                        let exact = global
                            .engines
                            .iter()
                            .find(|candidate| candidate.engine_id == engine.engine_id);
                        let matching = global
                            .engines
                            .iter()
                            .filter(|candidate| candidate.model_id == engine.model_id)
                            .collect::<Vec<_>>();
                        let inherited =
                            exact.or_else(|| (matching.len() == 1).then(|| matching[0]));
                        if exact.is_none() && matching.len() > 1 {
                            return Err(AppError::new(
                                "ambiguous_global_engine",
                                "configuration",
                                "全局引擎参数继承存在歧义，请明确引擎身份",
                            ));
                        }
                        Ok((
                            &engine.engine_id,
                            inherited.map(|parent| &parent.parameters),
                        ))
                    })
                    .collect::<AppResult<Vec<_>>>()?;
                Ok((&global.parameters, engines))
            })
            .transpose()?
    } else {
        None
    };
    let models: Vec<_> = b
        .engines
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| c.models.iter().find(|m| m.model_id == e.model_id))
        .map(|m| {
            (
                &m.model_id,
                &m.connection_id,
                &m.actual_model_id,
                &m.parameters,
            )
        })
        .collect();
    let cons: Vec<_> = models
        .iter()
        .filter_map(|m| c.connections.iter().find(|con| con.connection_id == *m.1))
        .map(|con| {
            (
                &con.protocol,
                &con.endpoint,
                &con.auth_mode,
                &con.credential_pool_id,
                &con.credential_pool_revision,
                &con.proxy_mode,
                &con.network_metadata,
                &con.availability_intent,
            )
        })
        .collect();
    let mut selected_templates = std::collections::BTreeSet::new();
    for engine in b.engines.iter().filter(|e| e.enabled) {
        if let Some(model) = c.models.iter().find(|m| m.model_id == engine.model_id)
            && let Some(template) =
                app_core::domain::model_config::templates::resolve_template(c, model, b, engine)?
        {
            selected_templates.insert(template.template_id.clone());
        }
    }
    let templates: Vec<_> = c
        .templates
        .iter()
        .filter(|t| selected_templates.contains(&t.template_id))
        .map(|t| {
            (
                &t.template_id,
                &t.purpose,
                &t.body,
                &t.variables,
                &t.compatibility,
            )
        })
        .collect();
    let terms: Vec<_> = c
        .term_sets
        .iter()
        .filter(|t| t.enabled && (t.scope_key == "global" || t.scope_key == scope))
        .map(|t| (&t.scope_key, &t.terms))
        .collect();
    if c.codec_version == Some(2) {
        let bindings: Vec<_> = c
            .connections
            .iter()
            .filter(|con| models.iter().any(|m| m.1 == &con.connection_id))
            .map(|con| (&con.connection_id, &con.adapter_binding))
            .collect();
        canonical_hash(&(
            b,
            inherited_parameters,
            models,
            cons,
            templates,
            terms,
            bindings,
        ))
    } else {
        canonical_hash(&(b, inherited_parameters, models, cons, templates, terms))
    }
}
pub(super) fn apply(tx: &Transaction, revision: &str, now: i64) -> AppResult<()> {
    let c = load(tx, revision)?;
    validate_configuration(&c, true)?;
    let previous_configuration = tx
        .query_row(
            "SELECT revision_id FROM config_heads WHERE head_kind='applied'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(err)?
        .map(|id| load(tx, &id))
        .transpose()?;
    for con in &c.connections {
        let blocked: Option<bool> = tx
            .query_row(
                "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                [&con.connection_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if blocked.is_none() {
            tx.execute(
                "INSERT INTO connection_runtime_states VALUES(?1,?3,0,1,NULL,?2)",
                params![con.connection_id, now, c.revision_id],
            )
            .map_err(err)?;
        } else {
            tx.execute("UPDATE connection_runtime_states SET config_revision_id=?2,control_revision=control_revision+1,updated_at_ms=?3 WHERE connection_id=?1",params![con.connection_id,c.revision_id,now]).map_err(err)?;
        }
    }
    // Resolve every global/game scope before dispatch. Override-off deliberately blocks global inheritance.
    let mut scopes = vec!["global".to_owned()];
    let mut q = tx
        .prepare("SELECT game_id FROM games WHERE archived_at_ms IS NULL")
        .map_err(err)?;
    scopes.extend(
        q.query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| r.map(|g| format!("game:{g}")))
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?,
    );
    drop(q);
    for scope in scopes {
        for purpose in ["translation", "grammar", "context_word"] {
            let binding = c.bindings.iter().find(|b| {
                b.scope_key == scope && b.purpose == purpose && b.binding_mode == "override"
            });
            let previous:Option<(String,u32,bool)>=tx.query_row("SELECT config_semantics_sha256,authorization_epoch,is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2",params![scope,purpose],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
            if let Some(b) = binding {
                let hash = semantic_hash(&c, b)?;
                let changed = previous.as_ref().is_none_or(|(old, _, _)| old != &hash);
                let epoch = previous
                    .as_ref()
                    .map(|(_, e, _)| {
                        e.checked_add(u32::from(changed)).ok_or_else(|| {
                            AppError::new(
                                "revision_exhausted",
                                "configuration",
                                "用途授权版本已耗尽",
                            )
                        })
                    })
                    .transpose()?
                    .unwrap_or(1);
                tx.execute("INSERT INTO purpose_runtime_states VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(scope_key,purpose) DO UPDATE SET config_revision_id=excluded.config_revision_id,binding_id=excluded.binding_id,is_enabled=excluded.is_enabled,authorization_epoch=excluded.authorization_epoch,config_semantics_sha256=excluded.config_semantics_sha256,updated_at_ms=excluded.updated_at_ms",params![scope,purpose,b.binding_id,c.revision_id,previous.as_ref().map(|(_,_,enabled)| *enabled).unwrap_or(true),epoch,hash,now]).map_err(err)?;
                if changed {
                    invalidate_scope(tx, &scope, purpose, now)?;
                }
            } else {
                if previous.is_some() {
                    // Retain the independent safety gate across temporary removal or inheritance.
                    tx.execute("UPDATE purpose_runtime_states SET authorization_epoch=authorization_epoch+1,updated_at_ms=?3 WHERE scope_key=?1 AND purpose=?2",params![scope,purpose,now]).map_err(err)?;
                    invalidate_scope(tx, &scope, purpose, now)?;
                }
                if let Some(game_id) = scope.strip_prefix("game:") {
                    // Inheriting a global binding does not create a second runtime authority row.
                    // Its request still includes the game's glossary, so compare that effective
                    // request scope when applying a new snapshot and cancel only that game's work.
                    let old_hash = previous_configuration
                        .as_ref()
                        .and_then(|old| {
                            effective_binding(old, Some(game_id), purpose).map(|b| (old, b))
                        })
                        .map(|(old, b)| semantic_hash_for_scope(old, b, &scope))
                        .transpose()?;
                    let new_hash = effective_binding(&c, Some(game_id), purpose)
                        .map(|b| semantic_hash_for_scope(&c, b, &scope))
                        .transpose()?;
                    if old_hash != new_hash {
                        invalidate_scope(tx, &scope, purpose, now)?;
                    }
                }
            }
        }
    }
    tx.execute("INSERT INTO config_heads VALUES('applied',?1,1,?2) ON CONFLICT(head_kind) DO UPDATE SET revision_id=excluded.revision_id,head_revision=config_heads.head_revision+1,updated_at_ms=excluded.updated_at_ms",params![revision,now]).map_err(err)?;
    Ok(())
}
fn invalidate_scope(tx: &Transaction, scope: &str, purpose: &str, now: i64) -> AppResult<()> {
    let game = scope.strip_prefix("game:");
    tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='configuration_changed',finished_at_ms=?3 WHERE purpose=?2 AND state IN ('queued','running') AND ((?1='global' AND scope_key='global') OR session_id IN (SELECT session_id FROM sessions WHERE game_id=?4))",params![scope,purpose,now,game]).map_err(err)?;
    tx.execute("UPDATE request_attempts SET state='cancelled',error_code='configuration_changed',finished_at_ms=?1 WHERE state='started' AND task_id IN (SELECT task_id FROM request_tasks WHERE state='invalidated')",[now]).map_err(err)?;
    Ok(())
}

pub(super) fn check_head(conn: &Connection, kind: &str, expected: u32) -> AppResult<()> {
    let current: u32 = conn
        .query_row(
            "SELECT head_revision FROM config_heads WHERE head_kind=?1",
            [kind],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or(0);
    if current != expected {
        return Err(AppError::new(
            "revision_conflict",
            "configuration",
            "配置已被另一窗口修改，请保留本地编辑并重新核对",
        ));
    }
    Ok(())
}
