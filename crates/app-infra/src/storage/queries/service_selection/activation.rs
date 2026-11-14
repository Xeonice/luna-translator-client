//! Atomic scoped selection, purpose control and inheritance.
use super::*;
pub(super) fn copy_closure(
    conn: &Connection,
    g: &mut ConfigurationV3,
    selected: &SavedServiceRef,
    op: &str,
    lineage: &mut Vec<service_graph::Materialization>,
) -> AppResult<EngineTarget> {
    let source_doc = configuration::load_document(conn, source_revision(selected))?;
    let source = materialize_base(&source_doc)?;
    match selected {
        SavedServiceRef::Http { source: reference } => {
            let original = source
                .models
                .iter()
                .find(|m| m.model_id == reference.model_id)
                .ok_or_else(|| failure("missing_model"))?;
            let con = source
                .connections
                .iter()
                .find(|c| c.connection_id == original.connection_id)
                .ok_or_else(|| failure("missing_connection"))?;
            let mut model = original.clone();
            let mut connection = con.clone();
            let origin = source_for(
                conn,
                &source,
                &EngineTarget::Http {
                    model_id: original.model_id.clone(),
                },
            )?;
            let origin_connection = match &origin {
                SavedServiceRef::Http { source: reference } => {
                    let document = configuration::load_document(conn, &reference.revision_id)?;
                    let source = materialize_base(&document)?;
                    let model = source
                        .models
                        .iter()
                        .find(|m| m.model_id == reference.model_id)
                        .ok_or_else(|| failure("missing_model"))?;
                    (reference.revision_id.clone(), model.connection_id.clone())
                }
                _ => return Err(failure("invalid_materialization_source")),
            };
            let mut provenance_conflict = false;
            for existing in lineage.iter().filter(|m| {
                m.effective_model.as_ref().is_some_and(|id| {
                    g.models.iter().any(|model| {
                        &model.model_id == id && model.connection_id == connection.connection_id
                    })
                })
            }) {
                let SavedServiceRef::Http { source: reference } = &existing.source else {
                    return Err(failure("invalid_materialization_source"));
                };
                let source =
                    materialize_base(&configuration::load_document(conn, &reference.revision_id)?)?;
                let model = source
                    .models
                    .iter()
                    .find(|m| m.model_id == reference.model_id)
                    .ok_or_else(|| failure("missing_model"))?;
                if (reference.revision_id.as_str(), model.connection_id.as_str())
                    != (origin_connection.0.as_str(), origin_connection.1.as_str())
                {
                    provenance_conflict = true;
                }
            }
            if provenance_conflict
                || g.connections
                    .iter()
                    .any(|c| c.connection_id == connection.connection_id && c != &connection)
            {
                connection.connection_id = id(
                    "activation-connection",
                    &format!("{op}:{}:{}", reference.revision_id, con.connection_id),
                );
            }
            model.connection_id = connection.connection_id.clone();
            for template_id in model.default_templates.values_mut() {
                let original = source
                    .templates
                    .iter()
                    .find(|t| &t.template_id == template_id)
                    .ok_or_else(|| failure("missing_template"))?;
                let mut template = original.clone();
                if g.templates
                    .iter()
                    .any(|t| t.template_id == template.template_id && t != &template)
                {
                    template.template_id = id(
                        "activation-template",
                        &format!("{op}:{}:{}", reference.revision_id, template.template_id),
                    );
                }
                *template_id = template.template_id.clone();
                if !g
                    .templates
                    .iter()
                    .any(|t| t.template_id == template.template_id)
                {
                    g.templates.push(template)
                }
            }
            if g.models
                .iter()
                .any(|m| m.model_id == model.model_id && m != &model)
            {
                model.model_id = id(
                    "activation-model",
                    &format!("{op}:{}:{}", reference.revision_id, model.model_id),
                );
            }
            if !g
                .connections
                .iter()
                .any(|c| c.connection_id == connection.connection_id)
            {
                g.connections.push(connection)
            }
            if !g.models.iter().any(|m| m.model_id == model.model_id) {
                g.models.push(model.clone())
            }

            let effective_model = model.model_id.clone();
            lineage.retain(|l| l.effective_model.as_ref() != Some(&effective_model));
            let legacy = matches!(&origin,SavedServiceRef::Http{source}if configuration::load_document(conn,&source.revision_id).is_ok_and(|d|d.as_legacy().is_some()));
            lineage.push(service_graph::Materialization {
                effective_model: Some(effective_model.clone()),
                effective_offline: None,
                source: origin,
                reason: if legacy {
                    "legacy_materialization"
                } else if effective_model != original.model_id {
                    "activation_collision"
                } else {
                    "selected_closure"
                }
                .into(),
            });
            Ok(EngineTarget::Http {
                model_id: effective_model,
            })
        }
        SavedServiceRef::Offline { source: reference } => {
            let original = source
                .offline_engine_configs
                .iter()
                .find(|o| o.id == reference.config_id)
                .ok_or_else(|| failure("missing_offline_config"))?;
            let mut config = original.clone();
            if g.offline_engine_configs
                .iter()
                .any(|o| o.id == config.id && o != &config)
            {
                config.id = id(
                    "activation-offline",
                    &format!("{op}:{}:{}", reference.revision_id, config.id),
                );
            }
            if !g.offline_engine_configs.iter().any(|o| o.id == config.id) {
                g.offline_engine_configs.push(config.clone())
            }
            lineage.retain(|l| l.effective_offline.as_ref() != Some(&config.id));
            lineage.push(service_graph::Materialization {
                effective_model: None,
                effective_offline: Some(config.id.clone()),
                source: selected.clone(),
                reason: if config.id == original.id {
                    "selected_closure"
                } else {
                    "activation_collision"
                }
                .into(),
            });
            Ok(EngineTarget::Offline {
                offline_config_id: config.id,
            })
        }
    }
}
pub(super) fn invalidate(
    tx: &Transaction,
    scope: &str,
    p: CatalogPurpose,
    now: i64,
) -> AppResult<Vec<String>> {
    let game = scope.strip_prefix("game:");
    let mut q=tx.prepare("SELECT t.task_id FROM request_tasks t JOIN sessions s ON s.session_id=t.session_id WHERE t.purpose=?2 AND t.state IN('queued','running') AND (t.scope_key=?1 OR (?3 IS NOT NULL AND t.scope_key='global' AND s.game_id=?3)) ORDER BY t.task_id").map_err(err)?;
    let tasks = q
        .query_map(params![scope, purpose(p), game], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    drop(q);
    for task in &tasks {
        tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='service_selection_changed',finished_at_ms=?2 WHERE task_id=?1 AND state IN('queued','running')",params![task,now]).map_err(err)?;
        tx.execute("UPDATE request_attempts SET state='cancelled',error_code='service_selection_changed',finished_at_ms=?2 WHERE task_id=?1 AND state='started'",params![task,now]).map_err(err)?;
    }
    Ok(tasks)
}
pub(super) fn select(
    db: &mut Database,
    input: &SelectSavedServicesInput,
    now: i64,
) -> AppResult<ServiceSelectionReceipt> {
    if now < 0 {
        return Err(failure("invalid_time"));
    }
    let directory = directory(db)?;
    let google_handler = super::super::google_authorization::handler_context(db);
    let hash = canonical_hash(input)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    check_generation(&tx, input.expected_profile_generation)?;
    if let Some(r) = replay(&tx, &input.operation_id, &hash, "service_selection")? {
        return Ok(r);
    }
    let current = head(&tx, "applied")?;
    if current.as_ref().map(|v| v.1).unwrap_or(0) != input.expected_applied_head_revision {
        return Err(failure("revision_conflict"));
    }
    let scope = input.scope.scope_key()?;
    if let ServiceScope::Game { game_id } = &input.scope {
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM games WHERE game_id=?1 AND archived_at_ms IS NULL)",
                [game_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !exists {
            return Err(failure("unknown_game"));
        }
    }
    let prior = projection(&tx, &input.scope, input.purpose)?;
    if prior.selection_revision != input.expected_selection_revision {
        return Err(failure("selection_revision_conflict"));
    }
    if prior.main.as_ref() == Some(&input.main)
        && prior.comparisons == input.comparisons
        && prior.effective_scope == input.scope
    {
        let result = ServiceSelectionReceipt {
            operation_id: input.operation_id.clone(),
            revision_id: current
                .as_ref()
                .ok_or_else(|| failure("configuration_not_applied"))?
                .0
                .clone(),
            applied_head_revision: input.expected_applied_head_revision,
            selection_revision: prior.selection_revision,
            affected_task_ids: vec![],
            semantic_change: false,
        };
        record(
            &tx,
            &input.operation_id,
            &hash,
            "service_selection",
            &result,
            now,
        )?;
        tx.commit().map_err(err)?;
        return Ok(result);
    }
    let sources = std::iter::once(&input.main)
        .chain(input.comparisons.iter())
        .collect::<Vec<_>>();
    let mut logical_ids = BTreeSet::new();
    for source in &sources {
        let role: String = tx
            .query_row(
                "SELECT revision_role FROM config_revisions WHERE revision_id=?1",
                [source_revision(source)],
                |r| r.get(0),
            )
            .map_err(err)?;
        if role != "user" {
            return Err(failure("activation_source_not_user"));
        }
        if !logical_ids.insert(logical(&tx, source)?) {
            return Err(failure("duplicate_logical_service"));
        }
        let g = materialize_base(&configuration::load_document(&tx, source_revision(source))?)?;
        if !matches!(
            eligibility(&tx, directory.as_ref(), &google_handler, &g, source, input.purpose)?,
            SavedServiceEligibility::Selectable {}
        ) {
            return Err(failure("service_not_selectable"));
        }
    }
    let document = current
        .as_ref()
        .map(|v| configuration::load_document(&tx, &v.0))
        .transpose()?
        .unwrap_or_else(|| ConfigurationDocument::Legacy {
            configuration: Configuration::default(),
        });
    let mut graph = materialize_base(&document)?;
    let mut lineage = current
        .as_ref()
        .map(|v| aliases(&tx, &v.0))
        .transpose()?
        .unwrap_or_default();
    let mut selection = current
        .as_ref()
        .map(|v| selections(&tx, &v.0))
        .transpose()?
        .unwrap_or_default();
    let old = binding(&graph, &scope, input.purpose).cloned();
    let enabled = effective(&graph, &scope, input.purpose)
        .map(|b| b.enabled)
        .unwrap_or(false);
    let mut engines = Vec::new();
    for (position, source) in sources.iter().enumerate() {
        engines.push(EngineV3 {
            engine_id: id(
                "selected-engine",
                &format!("{}:{position}", input.operation_id),
            ),
            target: copy_closure(&tx, &mut graph, source, &input.operation_id, &mut lineage)?,
            enabled: true,
            http_template_override: None,
            http_parameters: BTreeMap::new(),
            policy_overrides: Default::default(),
        })
    }
    let binding_id = old
        .as_ref()
        .map(|b| b.binding_id.clone())
        .unwrap_or_else(|| {
            id(
                "service-binding",
                &format!("{scope}:{}", purpose(input.purpose)),
            )
        });
    let selected = PurposeBindingV3 {
        binding_id: binding_id.clone(),
        scope_key: scope.clone(),
        game_id: if let ServiceScope::Game { game_id } = &input.scope {
            Some(game_id.clone())
        } else {
            None
        },
        purpose: input.purpose,
        binding_mode: "override".into(),
        enabled,
        output_mode: old
            .as_ref()
            .and_then(|b| b.output_mode.clone())
            .or_else(|| Some("text".into())),
        source_language: old.as_ref().and_then(|b| b.source_language.clone()),
        target_language: old.as_ref().and_then(|b| b.target_language.clone()),
        template_id: None,
        context: BTreeMap::new(),
        schedule: BTreeMap::new(),
        parameters: BTreeMap::new(),
        engines,
        semantics_version: PurposeSemanticsVersion::TypedV3,
        scope_overrides: old
            .as_ref()
            .map(|b| b.scope_overrides.clone())
            .unwrap_or_default(),
    };
    graph
        .bindings
        .retain(|b| !(b.scope_key == scope && b.purpose == input.purpose));
    graph.bindings.push(selected);
    let next = input
        .expected_selection_revision
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    selection.insert(binding_id.clone(), next);
    graph.revision_id = id("activation", &input.operation_id);
    ensure_presets(&tx, &graph, directory.as_ref(), now)?;
    service_graph::persist_verified(
        &tx,
        &graph,
        current.as_ref().map(|v| v.0.as_str()),
        service_graph::RevisionRole::Activation,
        now,
        &lineage,
        &selection,
    )?;
    let result = commit_selection(&tx, SelectionCommit {
        operation_id: input.operation_id.as_str(), request_hash: &hash, scope: &scope,
        purpose: input.purpose, graph: &graph, binding_id: &binding_id,
        selection_revision: next, head_revision: input.expected_applied_head_revision,
        enabled, force_runtime: Some(prior.purpose_enabled), now, kind: "service_selection",
    })?;
    tx.commit().map_err(err)?;
    Ok(result)
}
struct SelectionCommit<'a> {
    operation_id: &'a str,
    request_hash: &'a str,
    scope: &'a str,
    purpose: CatalogPurpose,
    graph: &'a ConfigurationV3,
    binding_id: &'a str,
    selection_revision: u32,
    head_revision: u32,
    enabled: bool,
    force_runtime: Option<bool>,
    now: i64,
    kind: &'a str,
}
fn commit_selection(
    tx: &Transaction,
    input: SelectionCommit<'_>,
) -> AppResult<ServiceSelectionReceipt> {
    let SelectionCommit {
        operation_id: op, request_hash: hash, scope, purpose: p, graph,
        binding_id, selection_revision, head_revision, enabled, force_runtime, now, kind,
    } = input;
    let mut affected = invalidate(tx, scope, p, now)?;
    if scope == "global" {
        let mut q=tx.prepare("SELECT DISTINCT scope_key FROM (SELECT scope_key,purpose FROM purpose_runtime_states UNION ALL SELECT scope_key,purpose FROM request_tasks WHERE state IN('queued','running')) WHERE purpose=?1 AND scope_key<>'global'").map_err(err)?;
        let scopes = q
            .query_map([purpose(p)], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        for scope in scopes {
            if !graph
                .bindings
                .iter()
                .any(|b| b.scope_key == scope && b.purpose == p && b.binding_mode == "override")
            {
                affected.extend(invalidate(tx, &scope, p, now)?);
            }
        }
    }
    let previous:Option<(u32,bool)>=tx.query_row("SELECT authorization_epoch,is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2",params![scope,purpose(p)],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
    let epoch = previous
        .as_ref()
        .map(|v| v.0)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    let inherited: Option<bool> = if scope != "global" {
        tx.query_row(
            "SELECT is_enabled FROM purpose_runtime_states WHERE scope_key='global' AND purpose=?1",
            [purpose(p)],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
    } else {
        None
    };
    let runtime_enabled = force_runtime
        .unwrap_or_else(|| previous.map(|v| v.1).or(inherited).unwrap_or(false))
        && enabled;
    let semantic = app_core::application::service_request_plan::purpose_service_semantics_hash(
        graph, binding_id,
    )?;
    tx.execute("INSERT INTO purpose_runtime_states VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(scope_key,purpose) DO UPDATE SET binding_id=excluded.binding_id,config_revision_id=excluded.config_revision_id,is_enabled=excluded.is_enabled,authorization_epoch=excluded.authorization_epoch,config_semantics_sha256=excluded.config_semantics_sha256,updated_at_ms=excluded.updated_at_ms",params![scope,purpose(p),binding_id,graph.revision_id,runtime_enabled,epoch,semantic,now]).map_err(err)?;
    let applied_head_revision = head_revision
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    tx.execute("INSERT INTO config_heads VALUES('applied',?1,?2,?3) ON CONFLICT(head_kind) DO UPDATE SET revision_id=excluded.revision_id,head_revision=excluded.head_revision,updated_at_ms=excluded.updated_at_ms",params![graph.revision_id,applied_head_revision,now]).map_err(err)?;
    let r = ServiceSelectionReceipt {
        operation_id: op.into(),
        revision_id: graph.revision_id.clone(),
        applied_head_revision,
        selection_revision,
        affected_task_ids: affected,
        semantic_change: true,
    };
    record(tx, op, hash, kind, &r, now)?;
    Ok(r)
}
pub(super) fn inherit(
    db: &mut Database,
    input: &RestoreServiceInheritanceInput,
    now: i64,
) -> AppResult<ServiceSelectionReceipt> {
    if now < 0 {
        return Err(failure("invalid_time"));
    }
    let directory = directory(db)?;
    let hash = canonical_hash(input)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    check_generation(&tx, input.expected_profile_generation)?;
    if let Some(r) = replay(&tx, &input.operation_id, &hash, "service_inheritance")? {
        return Ok(r);
    }
    let current = head(&tx, "applied")?.ok_or_else(|| failure("configuration_not_applied"))?;
    if current.1 != input.expected_applied_head_revision {
        return Err(failure("revision_conflict"));
    }
    let scope = format!("game:{}", input.game_id);
    require_id(&input.game_id)?;
    let prior = projection(
        &tx,
        &ServiceScope::Game {
            game_id: input.game_id.clone(),
        },
        input.purpose,
    )?;
    if prior.selection_revision != input.expected_selection_revision {
        return Err(failure("selection_revision_conflict"));
    }
    let mut graph = materialize_base(&configuration::load_document(&tx, &current.0)?)?;
    let global = binding(&graph, "global", input.purpose)
        .filter(|b| b.binding_mode == "override")
        .cloned()
        .ok_or_else(|| failure("global_service_not_selected"))?;
    let local = binding(&graph, &scope, input.purpose)
        .cloned()
        .ok_or_else(|| failure("game_selection_missing"))?;
    if local.binding_mode == "inherit" {
        let result = ServiceSelectionReceipt {
            operation_id: input.operation_id.clone(),
            revision_id: current.0,
            applied_head_revision: current.1,
            selection_revision: prior.selection_revision,
            affected_task_ids: vec![],
            semantic_change: false,
        };
        record(
            &tx,
            &input.operation_id,
            &hash,
            "service_inheritance",
            &result,
            now,
        )?;
        tx.commit().map_err(err)?;
        return Ok(result);
    }
    let mut inherited = local.clone();
    inherited.binding_mode = "inherit".into();
    inherited.enabled = false;
    inherited.engines.clear();
    inherited.template_id = None;
    inherited.context.clear();
    inherited.schedule.clear();
    inherited.parameters.clear();
    inherited.scope_overrides = Default::default();
    inherited.semantics_version = PurposeSemanticsVersion::TypedV3;
    graph.bindings.retain(|b| b.binding_id != local.binding_id);
    graph.bindings.push(inherited);
    let mut selection = selections(&tx, &current.0)?;
    let next = input
        .expected_selection_revision
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    selection.insert(local.binding_id, next);
    graph.revision_id = id("activation", &input.operation_id);
    let lineage = aliases(&tx, &current.0)?;
    ensure_presets(&tx, &graph, directory.as_ref(), now)?;
    service_graph::persist_verified(
        &tx,
        &graph,
        Some(&current.0),
        service_graph::RevisionRole::Activation,
        now,
        &lineage,
        &selection,
    )?;
    let affected = invalidate(&tx, &scope, input.purpose, now)?;
    // Historical tasks retain a foreign key to this control row. Revoke its
    // permission and advance the epoch; current selection resolves inheritance
    // from the graph instead of treating this historical row as an override.
    let epoch: Option<u32> = tx.query_row(
        "SELECT authorization_epoch FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2",
        params![scope, purpose(input.purpose)], |r| r.get(0),
    ).optional().map_err(err)?;
    if let Some(epoch) = epoch {
        let next_epoch = epoch
            .checked_add(1)
            .ok_or_else(|| failure("revision_exhausted"))?;
        tx.execute(
            "UPDATE purpose_runtime_states SET is_enabled=0,authorization_epoch=?3,updated_at_ms=?4 WHERE scope_key=?1 AND purpose=?2",
            params![scope, purpose(input.purpose), next_epoch, now],
        ).map_err(err)?;
    }
    let next_head = current
        .1
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    tx.execute("UPDATE config_heads SET revision_id=?1,head_revision=?2,updated_at_ms=?3 WHERE head_kind='applied'",params![graph.revision_id,next_head,now]).map_err(err)?;
    let r = ServiceSelectionReceipt {
        operation_id: input.operation_id.clone(),
        revision_id: graph.revision_id,
        applied_head_revision: next_head,
        selection_revision: next,
        affected_task_ids: affected,
        semantic_change: local.binding_mode != "inherit",
    };
    let _ = global;
    record(
        &tx,
        &input.operation_id,
        &hash,
        "service_inheritance",
        &r,
        now,
    )?;
    tx.commit().map_err(err)?;
    Ok(r)
}
pub(super) fn set_enabled(
    db: &mut Database,
    input: &SetServicePurposeEnabledInput,
    now: i64,
) -> AppResult<ServiceSelectionReceipt> {
    if now < 0 {
        return Err(failure("invalid_time"));
    }
    let directory = directory(db)?;
    let google_handler = super::super::google_authorization::handler_context(db);
    let hash = canonical_hash(input)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    check_generation(&tx, input.expected_profile_generation)?;
    if let Some(result) = replay(&tx, &input.operation_id, &hash, "service_purpose_enabled")? {
        return Ok(result);
    }
    let current = head(&tx, "applied")?.ok_or_else(|| failure("configuration_not_applied"))?;
    if current.1 != input.expected_applied_head_revision {
        return Err(failure("revision_conflict"));
    }
    let scope = input.scope.scope_key()?;
    let prior = projection(&tx, &input.scope, input.purpose)?;
    if prior.selection_revision != input.expected_selection_revision {
        return Err(failure("selection_revision_conflict"));
    }
    let mut graph = materialize_base(&configuration::load_document(&tx, &current.0)?)?;
    let previous = binding(&graph, &scope, input.purpose).cloned();
    let mut selected = effective(&graph, &scope, input.purpose)
        .cloned()
        .ok_or_else(|| failure("service_not_selected"))?;
    if selected.semantics_version != PurposeSemanticsVersion::TypedV3 {
        return Err(failure("legacy_requires_guided_configuration"));
    }
    if selected.engines.is_empty() {
        return Err(failure("service_not_selected"));
    }
    if input.enabled {
        for engine in selected.engines.iter().filter(|e| e.enabled) {
            let source = source_for(&tx, &graph, &engine.target)?;
            let source_graph = materialize_base(&configuration::load_document(
                &tx,
                source_revision(&source),
            )?)?;
            if !matches!(
                eligibility(
                    &tx,
                    directory.as_ref(),
                    &google_handler,
                    &source_graph,
                    &source,
                    input.purpose
                )?,
                SavedServiceEligibility::Selectable {}
            ) {
                return Err(failure("service_not_selectable"));
            }
        }
    }
    if previous
        .as_ref()
        .is_some_and(|b| b.binding_mode == "override" && b.enabled == input.enabled)
        && prior.purpose_enabled == input.enabled
    {
        let result = ServiceSelectionReceipt {
            operation_id: input.operation_id.clone(),
            revision_id: current.0,
            applied_head_revision: current.1,
            selection_revision: prior.selection_revision,
            affected_task_ids: vec![],
            semantic_change: false,
        };
        record(
            &tx,
            &input.operation_id,
            &hash,
            "service_purpose_enabled",
            &result,
            now,
        )?;
        tx.commit().map_err(err)?;
        return Ok(result);
    }
    selected.binding_id = previous
        .as_ref()
        .map(|b| b.binding_id.clone())
        .unwrap_or_else(|| {
            id(
                "service-binding",
                &format!("{scope}:{}", purpose(input.purpose)),
            )
        });
    selected.scope_key = scope.clone();
    selected.game_id = match &input.scope {
        ServiceScope::Global {} => None,
        ServiceScope::Game { game_id } => Some(game_id.clone()),
    };
    selected.binding_mode = "override".into();
    selected.enabled = input.enabled;
    let binding_id = selected.binding_id.clone();
    graph
        .bindings
        .retain(|b| !(b.scope_key == scope && b.purpose == input.purpose));
    graph.bindings.push(selected);
    let next = input
        .expected_selection_revision
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    let mut revisions = selections(&tx, &current.0)?;
    revisions.insert(binding_id.clone(), next);
    let lineage = aliases(&tx, &current.0)?;
    graph.revision_id = id("activation", &input.operation_id);
    ensure_presets(&tx, &graph, directory.as_ref(), now)?;
    service_graph::persist_verified(
        &tx,
        &graph,
        Some(&current.0),
        service_graph::RevisionRole::Activation,
        now,
        &lineage,
        &revisions,
    )?;
    let result = commit_selection(&tx, SelectionCommit {
        operation_id: &input.operation_id, request_hash: &hash, scope: &scope,
        purpose: input.purpose, graph: &graph, binding_id: &binding_id,
        selection_revision: next, head_revision: input.expected_applied_head_revision,
        enabled: input.enabled, force_runtime: Some(input.enabled), now, kind: "service_purpose_enabled",
    })?;
    tx.commit().map_err(err)?;
    Ok(result)
}
