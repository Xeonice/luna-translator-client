//! A retry cannot select a different provider, engine, credential, or input scope.
use super::*;

pub(in crate::storage) fn retry_attempt(
    db: &mut Database,
    task_id: String,
    attempt_id: String,
    error_code: String,
    now: i64,
) -> AppResult<String> {
    if ![
        "timeout",
        "connection_failed",
        "network_error",
        "service_unavailable",
    ]
    .contains(&error_code.as_str())
    {
        return Err(AppError::new(
            "retry_not_allowed",
            "scheduling",
            "此错误不允许自动重试",
        ));
    }
    let codec:Option<u32>=db.conn.query_row("SELECT c.codec_version FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=?1",[&task_id],|r|r.get(0)).optional().map_err(err)?;
    if codec == Some(3) {
        return service_tasks::retry(db, &task_id, &attempt_id, &error_code, now);
    }
    let handler = super::google_authorization::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    validate_task_authority(&tx, &handler, &task_id)?;
    let (number, retries): (u32, u32) = tx.query_row(
        "SELECT a.attempt_number,coalesce(json_extract(b.schedule_json,'$.retries'),0) FROM request_attempts a JOIN request_tasks t ON t.task_id=a.task_id JOIN purpose_bindings b ON b.revision_id=t.config_revision_id AND b.binding_id=t.binding_id WHERE a.attempt_id=?1 AND a.task_id=?2 AND a.state='started' AND a.dispatch_kind IN ('initial','retry') AND a.actual_engine_id=t.engine_id AND NOT EXISTS(SELECT 1 FROM request_attempts newer WHERE newer.task_id=a.task_id AND newer.attempt_number>a.attempt_number)",
        params![attempt_id,task_id], |r|Ok((r.get(0)?,r.get(1)?)),
    ).map_err(|_|AppError::new("retry_not_authorized","scheduling","此尝试已结束或不允许重试"))?;
    if retries > 3 || number > retries {
        return Err(AppError::new(
            "retry_limit",
            "scheduling",
            "已达到用户配置的重试次数上限",
        ));
    }
    let next = number
        .checked_add(1)
        .ok_or_else(|| AppError::new("retry_limit", "scheduling", "尝试编号已耗尽"))?;
    let new_id = id("attempt", &format!("{task_id}:{next}"));
    tx.execute("UPDATE request_attempts SET state='failed',error_code=?2,finished_at_ms=?3 WHERE attempt_id=?1 AND state='started'",params![attempt_id,error_code,now]).map_err(err)?;
    tx.execute("INSERT INTO request_attempts(attempt_id,task_id,attempt_number,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,dispatch_kind,state,error_code,finish_reason,usage_json,started_at_ms,finished_at_ms) SELECT ?1,task_id,?2,config_revision_id,binding_id,purpose,actual_engine_id,credential_id,'retry','started',NULL,NULL,'{}',?3,NULL FROM request_attempts WHERE attempt_id=?4",params![new_id,next,now,attempt_id]).map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)?;
    Ok(new_id)
}
