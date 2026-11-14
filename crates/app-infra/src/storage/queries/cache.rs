use super::*;
/// Versioned qualification of the fixed input, independent of history/unit IDs.
/// The latest lexical selection uses the same ordering as the ReadingUnit DTO.
pub(super) fn input_identity(
    conn: &Connection,
    unit: &ReadingUnit,
) -> AppResult<serde_json::Value> {
    let profile: String = conn
        .query_row(
            "SELECT profile_id FROM profile_meta WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    let (processing, snapshot):(String,String)=conn.query_row("SELECT processing_revision,processing_snapshot_json FROM reading_unit_revisions WHERE unit_id=?1 AND text_revision=?2",params![unit.unit_id,unit.text_revision],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    let lexical:Option<(String,String,String,String,String,String,String)>=conn.query_row("SELECT l.analysis_revision,l.token_schema_version,r.kind,r.version_label,r.manifest_sha256,r.contract_version,r.metadata_json FROM lexical_snapshots l JOIN resource_snapshots r ON r.resource_snapshot_id=l.language_resource_snapshot_id WHERE l.unit_id=?1 AND l.text_revision=?2 ORDER BY l.created_at_ms DESC,l.lexical_id DESC LIMIT 1",params![unit.unit_id,unit.text_revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(err)?;
    let lexical=lexical.map(|(analysis,schema,kind,version,manifest,contract,metadata)|->AppResult<_>{Ok(serde_json::json!({"analysisRevision":analysis,"tokenSchema":schema,"resource":{"kind":kind,"version":version,"manifestSha256":manifest,"contractVersion":contract,"metadata":parse::<serde_json::Value>(metadata)?}}))}).transpose()?;
    Ok(
        serde_json::json!({"schema":"fixed-cache-input-v2","profileId":profile,
        "sourceHash":unit.source_hash,"textRevision":unit.text_revision,"sourceLanguage":unit.source_language,
        "processingRevision":processing,"processingSnapshot":parse::<serde_json::Value>(snapshot)?,"lexical":lexical}),
    )
}
fn key(purpose: &str, request: &str) -> AppResult<String> {
    canonical_hash(&("request-cache-v1", purpose, request))
}
pub(super) fn lookup(
    conn: &Connection,
    c: &Configuration,
    b: &PurposeBinding,
    request: &str,
    now: i64,
) -> AppResult<(Option<CachedTaskResult>, bool)> {
    if b.schedule
        .get("cacheTtlMs")
        .and_then(serde_json::Value::as_i64)
        .is_none_or(|v| v <= 0)
    {
        return Ok((None, false));
    }
    let resource = configuration::semantic_hash(c, b)?;
    let cache_key = key(&b.purpose, request)?;
    let row:Option<(String,String,i64)>=conn.query_row("SELECT result_json,result_sha256,byte_size FROM cache_entries WHERE cache_key=?1 AND purpose=?2 AND request_semantics_sha256=?3 AND resource_semantics_sha256=?4 AND expires_at_ms>?5",params![cache_key,b.purpose,request,resource,now],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
    if let Some((value, hash, size)) = row {
        let result = if value.len() as i64 == size && content_hash(&value) == hash {
            parse::<CachedTaskResult>(value)
                .ok()
                .filter(|r| !r.text.trim().is_empty() && r.structured.is_object())
        } else {
            None
        };
        if let Some(result) = result {
            conn.execute(
                "UPDATE cache_entries SET last_accessed_at_ms=?2 WHERE cache_key=?1",
                params![cache_key, now],
            )
            .map_err(err)?;
            Ok((Some(result), false))
        } else {
            conn.execute("DELETE FROM cache_entries WHERE cache_key=?1", [&cache_key])
                .map_err(err)?;
            Ok((None, true))
        }
    } else {
        Ok((None, false))
    }
}

pub(super) fn store(tx: &Transaction, result: &TaskCompletion, now: i64) -> AppResult<()> {
    let codec:u32=tx.query_row("SELECT c.codec_version FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=?1",[&result.task_id],|r|r.get(0)).map_err(err)?;
    if codec == 3 {
        return store_service(tx, result, now);
    }
    let (revision,binding,request,purpose,dispatch):(String,String,String,String,String)=tx.query_row("SELECT t.config_revision_id,t.binding_id,t.request_semantics_sha256,t.purpose,a.dispatch_kind FROM request_tasks t JOIN request_attempts a USING(task_id) WHERE t.task_id=?1 AND a.attempt_id=?2",params![result.task_id,result.attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(err)?;
    if dispatch == "cache" {
        return Ok(());
    }
    let c = configuration::load(tx, &revision)?;
    let b = c
        .bindings
        .iter()
        .find(|b| b.binding_id == binding)
        .ok_or_else(|| AppError::new("binding_missing", "cache", "任务用途版本不存在"))?;
    let Some(ttl) = b
        .schedule
        .get("cacheTtlMs")
        .and_then(serde_json::Value::as_i64)
        .filter(|v| *v > 0)
    else {
        return Ok(());
    };
    let expires = now
        .checked_add(ttl)
        .ok_or_else(|| AppError::new("invalid_cache_lifetime", "cache", "缓存期限溢出"))?;
    let value = json(&CachedTaskResult {
        text: result.text.clone(),
        output_mode: result.output_mode.clone(),
        structured: result.structured.clone(),
        target_language: result.target_language.clone(),
        provider_label: result.provider_label.clone(),
    })?;
    tx.execute("INSERT INTO cache_entries(cache_key,purpose,request_semantics_sha256,resource_semantics_sha256,result_json,result_sha256,byte_size,created_at_ms,expires_at_ms,last_accessed_at_ms,origin_attempt_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?8,?10) ON CONFLICT(cache_key) DO UPDATE SET result_json=excluded.result_json,result_sha256=excluded.result_sha256,byte_size=excluded.byte_size,created_at_ms=excluded.created_at_ms,expires_at_ms=excluded.expires_at_ms,last_accessed_at_ms=excluded.last_accessed_at_ms,origin_attempt_id=excluded.origin_attempt_id",params![key(&purpose,&request)?,purpose,request,configuration::semantic_hash(&c,b)?,value,content_hash(&value),value.len() as i64,now,expires,result.attempt_id]).map_err(err)?;
    Ok(())
}

/// Typed cache has an explicit internal policy and a real producer attempt.
pub(super) fn lookup_service(
    conn: &Connection,
    purpose: &str,
    request: &str,
    resource: &str,
    target: &str,
    now: i64,
) -> AppResult<(Option<CachedTaskResult>, bool)> {
    let cache_key = canonical_hash(&(super::service_tasks::CACHE_POLICY, purpose, request))?;
    let row:Option<(String,String,i64)>=conn.query_row("SELECT c.result_json,c.result_sha256,c.byte_size FROM cache_entries c JOIN request_attempts a ON a.attempt_id=c.origin_attempt_id AND a.state='completed' JOIN request_tasks task ON task.task_id=a.task_id AND task.state='completed' JOIN task_execution_targets t ON t.task_id=task.task_id WHERE c.cache_key=?1 AND c.purpose=?2 AND c.request_semantics_sha256=?3 AND c.resource_semantics_sha256=?4 AND t.target_sha256=?5 AND c.expires_at_ms>?6 AND task.request_semantics_sha256=?3",params![cache_key,purpose,request,resource,target,now],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
    if let Some((value, hash, size)) = row {
        let result = if value.len() as i64 == size && content_hash(&value) == hash {
            parse::<CachedTaskResult>(value)
                .ok()
                .filter(|v| !v.text.trim().is_empty() && v.structured.is_object())
        } else {
            None
        };
        if let Some(value) = result {
            conn.execute(
                "UPDATE cache_entries SET last_accessed_at_ms=?2 WHERE cache_key=?1",
                params![cache_key, now],
            )
            .map_err(err)?;
            Ok((Some(value), false))
        } else {
            conn.execute("DELETE FROM cache_entries WHERE cache_key=?1", [cache_key])
                .map_err(err)?;
            Ok((None, true))
        }
    } else {
        Ok((None, false))
    }
}
fn store_service(tx: &Transaction, result: &TaskCompletion, now: i64) -> AppResult<()> {
    let snapshot = super::service_tasks::snapshot(tx, &result.task_id)?;
    if !snapshot.service_plan.policy.cache.enabled {
        return Ok(());
    }
    let (purpose,request,dispatch):(String,String,String)=tx.query_row("SELECT t.purpose,t.request_semantics_sha256,a.dispatch_kind FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id WHERE t.task_id=?1 AND a.attempt_id=?2",params![result.task_id,result.attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
    if dispatch == "cache" {
        return Ok(());
    }
    let expires = now
        .checked_add(super::service_tasks::CACHE_TTL_MS)
        .ok_or_else(|| AppError::new("invalid_cache_lifetime", "cache", "缓存期限溢出"))?;
    let value = json(&CachedTaskResult {
        text: result.text.clone(),
        output_mode: result.output_mode.clone(),
        structured: result.structured.clone(),
        target_language: result.target_language.clone(),
        provider_label: result.provider_label.clone(),
    })?;
    let key = canonical_hash(&(super::service_tasks::CACHE_POLICY, &purpose, &request))?;
    tx.execute("INSERT INTO cache_entries(cache_key,purpose,request_semantics_sha256,resource_semantics_sha256,result_json,result_sha256,byte_size,created_at_ms,expires_at_ms,last_accessed_at_ms,origin_attempt_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?8,?10) ON CONFLICT(cache_key) DO UPDATE SET result_json=excluded.result_json,result_sha256=excluded.result_sha256,byte_size=excluded.byte_size,created_at_ms=excluded.created_at_ms,expires_at_ms=excluded.expires_at_ms,last_accessed_at_ms=excluded.last_accessed_at_ms,origin_attempt_id=excluded.origin_attempt_id",params![key,purpose,request,snapshot.resource_semantics_sha256,value,content_hash(&value),value.len()as i64,now,expires,result.attempt_id]).map_err(err)?;
    Ok(())
}
