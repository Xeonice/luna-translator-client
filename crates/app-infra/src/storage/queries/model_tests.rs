use super::*;
use app_core::domain::testing::*;
pub(in crate::storage) fn record_model_test(
    db: &mut Database,
    i: ModelTestRecordInput,
) -> AppResult<()> {
    require_id(&i.test_id)?;
    if !i.input_summary.is_object()
        || !i.response_summary.is_object()
        || !i.output_validation.is_object()
    {
        return Err(AppError::new(
            "invalid_test_metadata",
            "test",
            "测试摘要必须是明确对象",
        ));
    }
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    tx.execute("INSERT INTO test_records(test_id,config_revision_id,connection_id,model_id,binding_id,purpose,stage,request_semantics_hash,input_scope,unit_id,text_revision,input_summary_json,response_summary_json,output_validation_json,state,started_at_ms,completed_at_ms,error_code,error_stage) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",params![i.test_id,i.config_revision_id,i.connection_id,i.model_id,i.binding_id,i.purpose,i.stage,i.request_semantics_hash,i.input_scope,i.unit_id,i.text_revision,json(&i.input_summary)?,json(&i.response_summary)?,json(&i.output_validation)?,i.state,i.started_at_ms,i.completed_at_ms,i.error_code,i.error_stage]).map_err(err)?;
    for c in &i.credentials {
        tx.execute(
            "INSERT INTO test_record_credentials VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                i.test_id,
                c.credential_id,
                c.pool_id,
                c.pool_revision,
                c.member_id,
                c.masked_identity,
                c.outcome
            ],
        )
        .map_err(err)?;
    }
    bump(&tx, i.completed_at_ms)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn credential_test_reference(
    db: &mut Database,
    pool: &str,
    rev: u32,
    vault: &str,
    outcome: &str,
) -> AppResult<TestCredentialUse> {
    db.conn.query_row("SELECT c.credential_id,m.pool_id,m.pool_revision,m.member_id,c.masked_label FROM credential_refs c JOIN credential_pool_members m USING(credential_id) WHERE m.pool_id=?1 AND m.pool_revision=?2 AND c.vault_ref=?3",params![pool,rev,vault],|r|Ok(TestCredentialUse{credential_id:r.get(0)?,pool_id:r.get(1)?,pool_revision:r.get(2)?,member_id:r.get(3)?,masked_identity:r.get(4)?,outcome:outcome.into()})).map_err(err)
}
pub(super) fn views(
    conn: &Connection,
    current: &Configuration,
) -> AppResult<Vec<ConfigurationTestView>> {
    let mut q=conn.prepare("SELECT test_id,config_revision_id,stage,connection_id,model_id,binding_id,request_semantics_hash,state,completed_at_ms,error_code,input_summary_json FROM test_records ORDER BY completed_at_ms DESC,test_id DESC LIMIT 100").map_err(err)?;
    q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,i64>(8)?,r.get::<_,Option<String>>(9)?,r.get::<_,String>(10)?))).map_err(err)?.map(|row|{let(test_id,revision_id,object_kind,con,model,binding,semantics_hash,status,completed,error_code,summary)=row.map_err(err)?;let object_id=match object_kind.as_str(){"connection"=>con.unwrap_or_default(),"model"=>model.unwrap_or_default(),"purpose"=>binding.unwrap_or_default(),_=>String::new()};let value:serde_json::Value=parse(summary)?;let now_hash=scope_semantics_hash(current,&object_kind,&object_id);let stale=value.get("scopeSemanticsHash").and_then(serde_json::Value::as_str).is_none_or(|old|match now_hash.as_ref(){Ok(now)=>old!=now,Err(_)=>true});let mut cq=conn.prepare("SELECT credential_id FROM test_record_credentials WHERE test_id=?1 ORDER BY credential_id").map_err(err)?;let credential_ids=cq.query_map([&test_id],|r|r.get(0)).map_err(err)?.collect::<Result<_,_>>().map_err(err)?;Ok(ConfigurationTestView{test_id,revision_id,object_kind,object_id,semantics_hash,status,completed_at_ms:completed as f64,error_code,credential_ids,stale,comparison_coverage:None})}).collect()
}

/// Typed semantic comparison is not yet covered. Preserve ledger facts, never turn
/// an actual success into a fabricated failure or compare it against an empty legacy graph.
pub(super) fn document_views(conn: &Connection, current: &ConfigurationDocument) -> AppResult<Vec<ConfigurationTestView>> {
    if let Some(legacy) = current.as_legacy() { return views(conn, legacy); }
    let mut q=conn.prepare("SELECT test_id,config_revision_id,stage,CASE stage WHEN 'connection' THEN coalesce(connection_id,'') WHEN 'model' THEN coalesce(model_id,'') WHEN 'purpose' THEN coalesce(binding_id,'') ELSE '' END,request_semantics_hash,state,completed_at_ms,error_code FROM test_records ORDER BY completed_at_ms DESC,test_id DESC LIMIT 100").map_err(err)?;
    q.query_map([],|r|Ok(ConfigurationTestView {test_id:r.get(0)?,revision_id:r.get(1)?,object_kind:r.get(2)?,object_id:r.get(3)?,semantics_hash:r.get(4)?,status:r.get(5)?,completed_at_ms:r.get::<_,i64>(6)? as f64,error_code:r.get(7)?,credential_ids:vec![],stale:true,comparison_coverage:Some(ConfigurationTestCoverage::NotCovered)})).map_err(err)?.map(|row| {let mut v=row.map_err(err)?;let mut q=conn.prepare("SELECT credential_id FROM test_record_credentials WHERE test_id=?1 ORDER BY credential_id").map_err(err)?;v.credential_ids=q.query_map([&v.test_id],|r|r.get(0)).map_err(err)?.collect::<Result<_,_>>().map_err(err)?;Ok(v)}).collect()
}
