//! Immutable provider source snapshots. Presentation is resolved by the registry, not SQL JSON.
use super::*;
use app_core::domain::model_config::catalog::*;
fn validate(p: &StoredProviderPreset) -> AppResult<()> {
    validate_preset_key(&p.key, p.namespace)?;
    validate_catalog_id(&p.provider_id)?;
    validate_catalog_label(&p.provider_label)?;
    validate_catalog_label(&p.display_name)?;
    validate_binding(&p.binding)?;
    // OAuth is an owned authorization identity, not a pool material kind.
    // Preset validation must not ask a pool-only resolver to fabricate one.
    if p.auth_kind == "google_oauth" {
        if p.binding.protocol.protocol_id != "google_translate_v3" {
            return Err(AppError::new("preset_auth_protocol_mismatch", "provider_preset", "Google授权只适用于明确的Advanced v3接口"));
        }
    } else {
        credential_kind_for_auth_mode(&p.auth_kind)?;
    }
    if p.endpoint.as_ref().is_some_and(|e| {
        e.contains(['\0', '?', '#'])
            || e.split("://")
                .nth(1)
                .unwrap_or("")
                .split('/')
                .next()
                .unwrap_or("")
                .contains('@')
    }) {
        return Err(AppError::new(
            "secret_in_endpoint",
            "provider_preset",
            "预设地址不能携带秘密",
        ));
    }
    for (key, value) in &p.options {
        validate_catalog_id(key)?;
        if forbidden_import_field(key)
            || matches!(
                key.as_str(),
                "model"
                    | "endpoint"
                    | "auth_mode"
                    | "app_secret"
                    | "service_account_json"
                    | "credential_ref"
                    | "vault_ref"
                    | "credential_pool_id"
            )
        {
            return Err(AppError::new(
                "protected_provider_preset_option",
                "provider_preset",
                "选项不能覆盖身份或含秘密",
            ));
        }
        match value {
            CatalogScalar::Text(s) if s.len() > 16384 || s.contains('\0') => {
                return Err(AppError::new(
                    "invalid_preset_option",
                    "provider_preset",
                    "选项文字无效",
                ));
            }
            CatalogScalar::Number(n) if !n.is_finite() => {
                return Err(AppError::new(
                    "invalid_preset_option",
                    "provider_preset",
                    "选项数字无效",
                ));
            }
            _ => {}
        }
    }
    if p.source_refs.len() > 128
        || p.source_refs
            .iter()
            .any(|s| s.is_empty() || s.len() > 4096 || s.chars().any(char::is_control))
    {
        return Err(AppError::new(
            "invalid_preset_source",
            "provider_preset",
            "来源记录无效",
        ));
    }
    Ok(())
}
pub(in crate::storage) fn store_provider_preset(
    db: &mut Database,
    p: StoredProviderPreset,
    now: i64,
) -> AppResult<()> {
    validate(&p)?;
    let hash = canonical_hash(&p)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let existing:Option<String>=tx.query_row("SELECT content_hash FROM provider_preset_versions WHERE preset_id=?1 AND preset_revision=?2",params![p.key.preset_id,p.key.preset_revision],|r|r.get(0)).optional().map_err(err)?;
    if let Some(old) = existing {
        if old != hash {
            return Err(AppError::new(
                "provider_preset_conflict",
                "provider_preset",
                "同一预设版本已保存不同内容",
            ));
        }
        return Ok(());
    }
    tx.execute("INSERT INTO provider_preset_versions(preset_id,preset_revision,namespace,provider_id,provider_label,display_name,protocol_id,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,endpoint,auth_kind,options_json,content_hash,source_json,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",params![p.key.preset_id,p.key.preset_revision,match p.namespace {CatalogNamespace::Builtin=>"builtin",CatalogNamespace::User=>"user"},p.provider_id,p.provider_label,p.display_name,p.binding.protocol.protocol_id,p.binding.protocol.contract_revision,p.binding.profile.profile_id,p.binding.profile.profile_revision,p.endpoint,p.auth_kind,json(&p.options)?,hash,json(&p.source_refs)?,now]).map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn provider_preset(
    conn: &Connection,
    key: &ProviderPresetKey,
) -> AppResult<StoredProviderPreset> {
    let row=conn.query_row("SELECT namespace,provider_id,provider_label,display_name,protocol_id,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,endpoint,auth_kind,options_json,source_json,content_hash FROM provider_preset_versions WHERE preset_id=?1 AND preset_revision=?2",params![key.preset_id,key.preset_revision],|r|Ok((StoredProviderPreset{key:key.clone(),namespace:if r.get::<_,String>(0)?=="builtin"{CatalogNamespace::Builtin}else{CatalogNamespace::User},provider_id:r.get(1)?,provider_label:r.get(2)?,display_name:r.get(3)?,binding:AdapterBinding{protocol:ProtocolKey{protocol_id:r.get(4)?,contract_revision:r.get(5)?},profile:CompatibilityProfileKey{profile_id:r.get(6)?,profile_revision:r.get(7)?}},endpoint:r.get(8)?,auth_kind:r.get(9)?,options:BTreeMap::new(),source_refs:vec![]},r.get::<_,String>(10)?,r.get::<_,String>(11)?,r.get::<_,String>(12)?))).optional().map_err(err)?.ok_or_else(||AppError::new("provider_preset_missing","provider_preset","预设来源版本不存在"))?;
    let (mut p, options, sources, hash) = row;
    p.options = parse(options)?;
    p.source_refs = parse(sources)?;
    validate(&p)?;
    if canonical_hash(&p)? != hash {
        return Err(AppError::new(
            "provider_preset_corrupt",
            "provider_preset",
            "预设来源内容与身份不符",
        ));
    }
    Ok(p)
}
