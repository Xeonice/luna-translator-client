use super::*;
use app_core::domain::retention::*;
pub(in crate::storage) fn maintain_storage(
    db: &mut Database,
    p: RetentionPolicy,
    mut protected: ProtectedObjects,
    operation: &str,
    now: i64,
) -> AppResult<MaintenanceReceipt> {
    require_id(operation)?;
    if p.max_rows_per_batch == 0 || now < 0 || p.raw_payload_before_ms.is_some_and(|v| v < 0) {
        return Err(AppError::new(
            "invalid_retention_policy",
            "storage",
            "保留时间不能为负",
        ));
    }
    // Same actor, before BEGIN: snapshot the genuine installed live owner. No
    // live-map lock is retained while SQLite performs maintenance.
    if let Some(authority) = &db.service_test_authority {
        for prepared in authority.active_prepared()? {
            if prepared.profile_id != db.profile_id { return Err(AppError::new("service_test_profile_mismatch", "storage", "测试保护对象不属于当前配置档")); }
            let source_revision = match &prepared.source.source {
                app_core::domain::model_config::service_graph::SavedServiceRef::Http { source } => &source.revision_id,
                app_core::domain::model_config::service_graph::SavedServiceRef::Offline { source } => &source.revision_id,
            };
            protected.config_revision_ids.push(source_revision.clone());
            for (unit_id, _) in &prepared.plan.context_units { protected.unit_ids.push(unit_id.clone()); }
            if let app_core::ports::service_testing::PreparedTestReading::Selected { unit, .. } = &prepared.reading {
                protected.unit_ids.push(unit.unit_id.clone());
            }
            if let app_core::ports::service_testing::PreparedTestAuthentication::PoolMember(material) = &prepared.authentication {
                protected.credential_pool_versions.push((material.selection.pool_id.clone(), material.selection.pool_revision));
            }
        }
    }
    if let Some(authority)=&db.service_model_discovery_authority {
        for prepared in authority.active_pins()? {
            if prepared.profile_id!=db.profile_id {return Err(AppError::new("discovery_source_profile_mismatch","storage","模型列表保护对象不属于当前配置档"));}
            let revision=match &prepared.source.source {
                app_core::domain::model_config::service_graph::SavedServiceRef::Http {source}=>&source.revision_id,
                app_core::domain::model_config::service_graph::SavedServiceRef::Offline {source}=>&source.revision_id,
            };
            protected.config_revision_ids.push(revision.clone());
            if let app_core::ports::service_discovery::ServiceDiscoveryAuthentication::PoolMember {metadata,..}=&prepared.authentication {
                protected.credential_pool_versions.push((metadata.pool_id.clone(),metadata.pool_revision));
            }
        }
    }
    let pinned = {
        let mut q = db
            .conn
            .prepare("SELECT session_id FROM sessions WHERE source_kind='hook'")
            .map_err(err)?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        ids.into_iter()
            .filter(|id| super::session_records::session_pinned(db, id))
            .collect::<Vec<_>>()
    };
    let pinned_json = json(&pinned)?;
    let hash = canonical_hash(&p)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let previous: Option<(String, String)> = tx
        .query_row(
            "SELECT request_sha256,result_refs_json FROM operation_receipts WHERE operation_id=?1",
            [operation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    if let Some((old, r)) = previous {
        if old != hash {
            return Err(AppError::new(
                "operation_conflict",
                "storage",
                "相同保留操作用于不同政策",
            ));
        }
        let mut r: MaintenanceReceipt = parse(r)?;
        r.replayed = true;
        return Ok(r);
    }
    let mut remaining = p.max_rows_per_batch as usize;
    let mut receipt = MaintenanceReceipt {
        operation_id: operation.into(),
        ..Default::default()
    };
    if let Some(before) = p.raw_payload_before_ms {
        receipt.removed_raw_payloads += tx
            .execute(
                "DELETE FROM raw_event_payloads WHERE event_id IN (SELECT event_id FROM raw_event_payloads WHERE retained_at_ms<?1 AND NOT EXISTS(SELECT 1 FROM source_refs f JOIN reading_units u USING(unit_id) WHERE f.event_id=raw_event_payloads.event_id AND u.state='open') AND NOT EXISTS(SELECT 1 FROM raw_events e JOIN sessions s USING(session_id) WHERE e.event_id=raw_event_payloads.event_id AND (s.state IN ('opening','active','crashed') OR s.session_id IN (SELECT value FROM json_each(?3)))) ORDER BY retained_at_ms,event_id LIMIT ?2)",
                params![before,remaining as i64,pinned_json],
            )
            .map_err(err)? as u32;
    }
    remaining = remaining.saturating_sub(receipt.removed_raw_payloads as usize);
    if let Some(limit) = p.raw_payload_byte_limit {
        let mut size: i64 = tx
            .query_row(
                "SELECT coalesce(sum(length(payload_bytes)),0) FROM raw_event_payloads",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let mut q=tx.prepare("SELECT event_id,length(payload_bytes) FROM raw_event_payloads WHERE NOT EXISTS(SELECT 1 FROM source_refs f JOIN reading_units u USING(unit_id) WHERE f.event_id=raw_event_payloads.event_id AND u.state='open') AND NOT EXISTS(SELECT 1 FROM raw_events e JOIN sessions s USING(session_id) WHERE e.event_id=raw_event_payloads.event_id AND (s.state IN ('opening','active','crashed') OR s.session_id IN (SELECT value FROM json_each(?2)))) ORDER BY retained_at_ms,event_id LIMIT ?1").map_err(err)?;
        let rows = q
            .query_map(params![remaining as i64, pinned_json], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        drop(q);
        for (id, n) in rows {
            if size as u64 <= limit {
                break;
            }
            tx.execute("DELETE FROM raw_event_payloads WHERE event_id=?1", [id])
                .map_err(err)?;
            size -= n;
            receipt.removed_raw_payloads += 1;
            remaining = remaining.saturating_sub(1);
        }
    }
    if p.remove_expired_cache {
        receipt.removed_cache_entries += tx
            .execute("DELETE FROM cache_entries WHERE cache_key IN (SELECT cache_key FROM cache_entries WHERE expires_at_ms<=?1 ORDER BY expires_at_ms,cache_key LIMIT ?2)", params![now,remaining as i64])
            .map_err(err)? as u32;
    }
    remaining = remaining.saturating_sub(receipt.removed_cache_entries as usize);
    if let Some(limit) = p.cache_byte_limit {
        let mut size: i64 = tx
            .query_row(
                "SELECT coalesce(sum(byte_size),0) FROM cache_entries",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let mut q=tx.prepare("SELECT cache_key,byte_size FROM cache_entries ORDER BY last_accessed_at_ms,cache_key LIMIT ?1").map_err(err)?;
        let rows = q
            .query_map([remaining as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        drop(q);
        for (id, n) in rows {
            if size as u64 <= limit {
                break;
            }
            tx.execute("DELETE FROM cache_entries WHERE cache_key=?1", [id])
                .map_err(err)?;
            size -= n;
            receipt.removed_cache_entries += 1;
            remaining = remaining.saturating_sub(1);
        }
    }
    if p.history_unit_ids.len() + p.config_revision_ids.len() + p.credential_pool_versions.len()
        > remaining
    {
        return Err(AppError::new(
            "retention_batch_too_large",
            "storage",
            "显式清理对象超过本批授权数量，请分批执行",
        ));
    }
    for uid in &p.history_unit_ids {
        let session: Option<String> = tx
            .query_row(
                "SELECT session_id FROM reading_units WHERE unit_id=?1",
                [uid],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if session.as_ref().is_some_and(|id| pinned.contains(id)) {
            return Err(AppError::new(
                "reading_object_in_use",
                "storage",
                "导出仍持有该会话的固定正文范围",
            ));
        }
        let open: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM reading_units WHERE unit_id=?1 AND state='open')",
                [uid],
                |r| r.get(0),
            )
            .map_err(err)?;
        if open {
            return Err(AppError::new(
                "reading_object_in_use",
                "storage",
                "不能清理仍在聚合的阅读对象",
            ));
        }
        if protected.unit_ids.contains(uid) {
            return Err(AppError::new(
                "reading_object_in_use",
                "storage",
                "不能清理当前或已固定的阅读对象",
            ));
        }
        let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM request_tasks WHERE unit_id=?1 AND state IN ('queued','running'))",[uid],|r|r.get(0)).map_err(err)?;
        if active {
            return Err(AppError::new(
                "reading_object_in_use",
                "storage",
                "阅读对象还有未完成请求，请先取消",
            ));
        }
        // A derived cache row records its real producer attempt. Collect only
        // caches of this explicitly authorized history object before its task
        // cascade, preserving unrelated caches and the source FK constraint.
        receipt.removed_cache_entries += tx.execute(
            "DELETE FROM cache_entries WHERE origin_attempt_id IN (SELECT a.attempt_id FROM request_attempts a JOIN request_tasks t ON t.task_id=a.task_id WHERE t.unit_id=?1)",
            [uid],
        ).map_err(err)? as u32;
        tx.execute("DELETE FROM translations WHERE task_id IN (SELECT task_id FROM request_tasks WHERE unit_id=?1)",[uid]).map_err(err)?;
        tx.execute("DELETE FROM study_results WHERE task_id IN (SELECT task_id FROM request_tasks WHERE unit_id=?1)",[uid]).map_err(err)?;
        // Attempt targets restrict deletion of their task target. Delete these
        // exact children first instead of relying on sibling cascade ordering.
        tx.execute(
            "DELETE FROM attempt_execution_targets WHERE task_id IN (SELECT task_id FROM request_tasks WHERE unit_id=?1)",
            [uid],
        ).map_err(err)?;
        receipt.removed_history_units += tx
            .execute("DELETE FROM reading_units WHERE unit_id=?1", [uid])
            .map_err(err)? as u32;
    }
    for revision in &p.config_revision_ids {
        if protected.config_revision_ids.contains(revision) {
            return Err(AppError::new(
                "config_in_use",
                "storage",
                "配置仍由运行对象持有",
            ));
        }
        // A saved fallback or materialized closure outside this revision owns
        // its real immutable source. Never erase that source to satisfy GC.
        let external: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM activation_resource_materializations a WHERE a.source_revision_id=?1 AND a.effective_revision_id<>?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=a.effective_revision_id)) OR EXISTS(SELECT 1 FROM service_default_fallbacks f WHERE f.source_revision_id=?1 AND f.revision_id<>?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=f.revision_id))",
            [revision], |r| r.get(0),
        ).map_err(err)?;
        if external { return Err(AppError::new("config_in_use", "storage", "配置仍是已保存服务或物化闭包的真实来源")); }
        tx.execute(
            "INSERT INTO config_gc_marks VALUES(?1,?2,?3)",
            params![revision, operation, now],
        )
        .map_err(err)?;
        tx.execute("DELETE FROM activation_resource_materializations WHERE effective_revision_id=?1", [revision]).map_err(err)?;
        for table in [
            "service_default_fallbacks",
            "service_request_defaults",
            "engine_parameters",
            "binding_parameters",
            "model_parameters",
            "model_default_templates",
            "binding_engines",
            "purpose_bindings",
            "terms",
            "term_sets",
            "offline_engine_configs",
            "models",
            "templates",
            "connections",
        ] {
            let sql = format!("DELETE FROM {table} WHERE revision_id=?1");
            tx.execute(&sql, [revision]).map_err(err)?;
        }
        receipt.collected_config_snapshots += 1;
    }
    for (pool, revision) in &p.credential_pool_versions {
        if protected
            .credential_pool_versions
            .contains(&(pool.clone(), *revision))
        {
            return Err(AppError::new(
                "credential_pool_in_use",
                "storage",
                "凭据池仍由运行对象持有",
            ));
        }
        tx.execute(
            "INSERT INTO credential_pool_gc_marks VALUES(?1,?2,?3,?4)",
            params![pool, revision, operation, now],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM credential_pool_members WHERE pool_id=?1 AND pool_revision=?2",
            params![pool, revision],
        )
        .map_err(err)?;
        receipt.collected_pool_snapshots += 1;
    }
    if let Some(limit) = p.raw_payload_byte_limit {
        let size: i64 = tx
            .query_row(
                "SELECT coalesce(sum(length(payload_bytes)),0) FROM raw_event_payloads",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        receipt.more_work |= size as u64 > limit;
    }
    if let Some(before) = p.raw_payload_before_ms {
        let more: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM raw_event_payloads WHERE retained_at_ms<?1)",
                [before],
                |r| r.get(0),
            )
            .map_err(err)?;
        receipt.more_work |= more;
    }
    if let Some(limit) = p.cache_byte_limit {
        let size: i64 = tx
            .query_row(
                "SELECT coalesce(sum(byte_size),0) FROM cache_entries",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        receipt.more_work |= size as u64 > limit;
    }
    if p.remove_expired_cache {
        let more: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM cache_entries WHERE expires_at_ms<=?1)",
                [now],
                |r| r.get(0),
            )
            .map_err(err)?;
        receipt.more_work |= more;
    }
    receipt.commit_revision = bump(&tx, now)?;
    tx.execute(
        "INSERT INTO operation_receipts VALUES(?1,?2,'storage_maintenance',?3,?4,?5)",
        params![
            operation,
            hash,
            receipt.commit_revision,
            json(&receipt)?,
            now
        ],
    )
    .map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(receipt)
}
