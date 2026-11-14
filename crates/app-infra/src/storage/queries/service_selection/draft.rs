//! Compiled draft save and independently proved immutable sources.
use super::*;
pub(super) fn stored_preset(
    tx: &Transaction,
    catalog: &ServiceCatalog,
    key: &ProviderPresetKey,
    now: i64,
) -> AppResult<()> {
    let p = catalog
        .presets
        .iter()
        .find(|p| &p.key == key)
        .ok_or_else(|| failure("provider_preset_missing"))?;
    let auth = match p.default_auth_kind {
        CatalogAuthKind::None => "none",
        CatalogAuthKind::BearerApiKey
        | CatalogAuthKind::AnthropicApiKey
        | CatalogAuthKind::GoogleApiKey
        | CatalogAuthKind::AzureSubscriptionKey
        | CatalogAuthKind::DeeplAuthKey => "api_key",
        CatalogAuthKind::BaiduAppSecret => "baidu_app_secret",
        CatalogAuthKind::YoudaoAppSecret => "youdao_app_secret",
        CatalogAuthKind::GoogleServiceAccount => "google_service_account",
        CatalogAuthKind::GoogleOauth => "google_oauth",
    };
    let record = StoredProviderPreset {
        key: p.key.clone(),
        namespace: p.namespace,
        provider_id: p.provider_id.clone(),
        provider_label: p.brand_label.clone(),
        display_name: p.label.clone(),
        binding: p.binding.clone(),
        endpoint: p.default_endpoint.clone(),
        auth_kind: auth.into(),
        options: BTreeMap::new(),
        source_refs: p.source_refs.clone(),
    };
    let old:Option<String>=tx.query_row("SELECT content_hash FROM provider_preset_versions WHERE preset_id=?1 AND preset_revision=?2",params![key.preset_id,key.preset_revision],|r|r.get(0)).optional().map_err(err)?;
    if old.is_some() {
        let existing = provider_presets::provider_preset(tx, key)?;
        if existing.binding != record.binding
            || existing.namespace != record.namespace
            || existing.provider_id != record.provider_id
            || existing.display_name != record.display_name
            || existing.endpoint != record.endpoint
            || existing.auth_kind != record.auth_kind
        {
            return Err(failure("provider_preset_conflict"));
        }
        return Ok(());
    }
    if p.namespace == CatalogNamespace::User {
        return Err(failure("user_preset_not_persisted"));
    }
    tx.execute("INSERT INTO provider_preset_versions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",params![key.preset_id,key.preset_revision,if p.namespace==CatalogNamespace::Builtin{"builtin"}else{"user"},record.provider_id,record.provider_label,record.display_name,record.binding.protocol.protocol_id,record.binding.protocol.contract_revision,record.binding.profile.profile_id,record.binding.profile.profile_revision,record.endpoint,record.auth_kind,json(&record.options)?,canonical_hash(&record)?,json(&record.source_refs)?,now]).map_err(err)?;
    Ok(())
}
pub(super) fn ensure_presets(
    tx: &Transaction,
    graph: &ConfigurationV3,
    directory: &dyn ServiceDirectory,
    now: i64,
) -> AppResult<()> {
    let catalog = directory.catalog();
    for c in &graph.connections {
        if c.materialization_source.is_some() {
            continue;
        }
        if let Some(key) = &c.preset_source {
            let descriptor = catalog
                .presets
                .iter()
                .find(|p| &p.key == key)
                .ok_or_else(|| failure("provider_preset_missing"))?;
            if c.adapter_binding.as_ref() != Some(&descriptor.binding) {
                return Err(failure("adapter_binding_mismatch"));
            }
            stored_preset(tx, &catalog, key, now)?
        }
    }
    Ok(())
}
pub(in crate::storage::queries) fn ensure_controls(
    tx: &Transaction,
    graph: &ConfigurationV3,
    now: i64,
) -> AppResult<()> {
    for c in &graph.connections {
        if connection_control(tx, &graph.revision_id, &c.connection_id)? != c.connection_id {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO connection_runtime_states VALUES(?1,?2,0,1,NULL,?3)",
            params![c.connection_id, graph.revision_id, now],
        )
        .map_err(err)?;
    }
    for o in &graph.offline_engine_configs {
        let alias:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM activation_resource_materializations WHERE effective_revision_id=?1 AND effective_offline_config_id=?2)",params![graph.revision_id,o.id],|r|r.get(0)).map_err(err)?;
        if alias {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO offline_config_runtime_states VALUES(?1,1,0,NULL,?2)",
            params![o.id, now],
        )
        .map_err(err)?;
    }
    Ok(())
}
pub(super) fn save_compiled(
    db: &mut Database,
    command: &CompiledServiceDraft,
    now: i64,
) -> AppResult<ServiceDraftReceipt> {
    if now < 0 {
        return Err(failure("invalid_time"));
    }
    let directory = directory(db)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    check_generation(&tx, command.expected_profile_generation)?;
    if let Some(r) = replay(
        &tx,
        &command.operation_id,
        &command.request_hash,
        "service_draft",
    )? {
        return Ok(r);
    }
    let current = head(&tx, "draft")?;
    if current.as_ref().map(|v| v.1).unwrap_or(0) != command.expected_draft_head_revision {
        return Err(failure("revision_conflict"));
    }
    let base = if let Some((id, _)) = &current {
        configuration::load_document(&tx, id)?
    } else {
        ConfigurationDocument::Legacy {
            configuration: Configuration::default(),
        }
    };
    let baseline = materialize_base(&base)?;
    if baseline.bindings != command.graph.bindings || baseline.term_sets != command.graph.term_sets
    {
        return Err(failure("service_save_changes_runtime_intent"));
    }
    // Existing legacy objects cannot be edited under a borrowed source claim.
    for original in baseline
        .models
        .iter()
        .filter(|m| m.legacy_eligibility_origin.is_some())
    {
        if !command.graph.models.contains(original) {
            return Err(failure("materialization_field_mismatch"));
        }
    }
    for original in baseline
        .connections
        .iter()
        .filter(|m| m.materialization_source.is_some())
    {
        if !command.graph.connections.contains(original) {
            return Err(failure("materialization_field_mismatch"));
        }
    }
    for model in command
        .graph
        .models
        .iter()
        .filter(|m| m.legacy_eligibility_origin.is_some())
    {
        if !baseline.models.contains(model) {
            return Err(failure("untrusted_materialization_claim"));
        }
    }
    for connection in command
        .graph
        .connections
        .iter()
        .filter(|m| m.materialization_source.is_some())
    {
        if !baseline.connections.contains(connection) {
            return Err(failure("untrusted_materialization_claim"));
        }
    }
    let target_model = match &command.resource {
        EngineTarget::Http { model_id } => Some(model_id.as_str()),
        _ => None,
    };
    let target_connection = target_model
        .and_then(|id| command.graph.models.iter().find(|m| m.model_id == id))
        .map(|m| m.connection_id.as_str());
    for original in &baseline.models {
        if Some(original.model_id.as_str()) != target_model
            && !command.graph.models.contains(original)
        {
            return Err(failure("service_save_changes_unselected_resource"));
        }
    }
    for original in &baseline.connections {
        if Some(original.connection_id.as_str()) != target_connection
            && !command.graph.connections.contains(original)
        {
            return Err(failure("service_save_changes_unselected_resource"));
        }
    }
    for original in &baseline.templates {
        if !command.graph.templates.contains(original) {
            return Err(failure("service_save_changes_unselected_resource"));
        }
    }
    for original in &baseline.offline_engine_configs {
        if !matches!(&command.resource,EngineTarget::Offline{offline_config_id}if offline_config_id==&original.id)
            && !command.graph.offline_engine_configs.contains(original)
        {
            return Err(failure("service_save_changes_unselected_resource"));
        }
    }
    match &command.resource {
        EngineTarget::Http { model_id }
            if !command.graph.models.iter().any(|m| &m.model_id == model_id) =>
        {
            return Err(failure("missing_saved_resource"));
        }
        EngineTarget::Offline { offline_config_id }
            if !command
                .graph
                .offline_engine_configs
                .iter()
                .any(|m| &m.id == offline_config_id) =>
        {
            return Err(failure("missing_saved_resource"));
        }
        _ => {}
    }
    ensure_presets(&tx, &command.graph, directory.as_ref(), now)?;
    let prior_aliases = current
        .as_ref()
        .map(|v| aliases(&tx, &v.0))
        .transpose()?
        .unwrap_or_default();
    let selection = current
        .as_ref()
        .map(|v| selections(&tx, &v.0))
        .transpose()?
        .unwrap_or_default();
    service_graph::persist_verified(
        &tx,
        &command.graph,
        current.as_ref().map(|v| v.0.as_str()),
        service_graph::RevisionRole::User,
        now,
        &prior_aliases,
        &selection,
    )?;
    let next = command
        .expected_draft_head_revision
        .checked_add(1)
        .ok_or_else(|| failure("revision_exhausted"))?;
    tx.execute("INSERT INTO config_heads VALUES('draft',?1,?2,?3) ON CONFLICT(head_kind) DO UPDATE SET revision_id=excluded.revision_id,head_revision=excluded.head_revision,updated_at_ms=excluded.updated_at_ms",params![command.graph.revision_id,next,now]).map_err(err)?;
    let result = ServiceDraftReceipt {
        operation_id: command.operation_id.clone(),
        revision_id: command.graph.revision_id.clone(),
        draft_head_revision: next,
        saved_resource: resource_ref(&command.graph.revision_id, &command.resource),
        affected_shared_model_ids: vec![],
    };
    record(
        &tx,
        &command.operation_id,
        &command.request_hash,
        "service_draft",
        &result,
        now,
    )?;
    tx.commit().map_err(err)?;
    Ok(result)
}
