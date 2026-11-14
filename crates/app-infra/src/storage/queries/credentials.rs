use super::*;
pub(in crate::storage) fn begin_credential(
    db: &mut Database,
    r: CredentialReference,
    now: i64,
) -> AppResult<()> {
    begin_credential_material(
        db,
        r,
        app_core::domain::model_config::credentials::CredentialStorageMetadata::LegacyRawApiKey,
        now,
    )
}
pub(in crate::storage) fn begin_credential_material(
    db: &mut Database,
    r: CredentialReference,
    storage: app_core::domain::model_config::credentials::CredentialStorageMetadata,
    now: i64,
) -> AppResult<()> {
    use app_core::domain::model_config::credentials::CredentialStorageMetadata;
    let (kind, schema) = match storage {
        CredentialStorageMetadata::LegacyRawApiKey => ("api_key", None),
        CredentialStorageMetadata::EnvelopeV1 { kind } => (kind.key(), Some(1u32)),
    };
    require_id(&r.credential_id)?;
    require_id(&r.credential_version_id)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    tx.execute(
        "INSERT INTO credential_refs(credential_id,vault_ref,masked_label,auth_kind,state,created_at_ms,activated_at_ms,revoked_at_ms,cleanup_requested_at_ms,cleaned_at_ms,lifecycle_revision,envelope_schema) VALUES(?1,?2,?3,?5,'pending',?4,NULL,NULL,NULL,NULL,0,?6)",
        params![
            r.credential_id,
            r.credential_version_id,
            format!("{} {}", r.label, r.masked_hint),
            now, kind, schema
        ],
    )
    .map_err(err)?;
    let hash = canonical_hash(&(&r.pool_id, r.pool_revision, &r.credential_id))?;
    tx.execute(
        "INSERT INTO credential_pool_revisions VALUES(?1,?2,'selected','{}',?3,?4)",
        params![r.pool_id, r.pool_revision, now, hash],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT INTO credential_pool_members VALUES(?1,?2,?3,?4,0,1,1)",
        params![
            r.pool_id,
            r.pool_revision,
            id("member", &r.credential_id),
            r.credential_id
        ],
    )
    .map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn activate_credential(
    db: &mut Database,
    credential: &str,
    now: i64,
) -> AppResult<()> {
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if tx.execute("UPDATE credential_refs SET state='active',activated_at_ms=?2,lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND state='pending'",params![credential,now]).map_err(err)?!=1{return Err(AppError::new("credential_state_conflict","credentials","凭据已撤销、已激活或不存在"));}
    tx.execute("INSERT INTO credential_pool_seals SELECT p.pool_id,p.pool_revision,p.content_hash,?2 FROM credential_pool_revisions p JOIN credential_pool_members m USING(pool_id,pool_revision) WHERE m.credential_id=?1 AND NOT EXISTS(SELECT 1 FROM credential_pool_seals s WHERE s.pool_id=p.pool_id AND s.pool_revision=p.pool_revision)",params![credential,now]).map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn revoke_credential(
    db: &mut Database,
    credential: &str,
    now: i64,
) -> AppResult<()> {
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if revoke_authority(&tx, credential, now)? {
        bump(&tx, now)?;
    }
    tx.commit().map_err(err)
}
pub(super) fn revoke_authority(tx: &Transaction, credential: &str, now: i64) -> AppResult<bool> {
    let state: Option<String> = tx
        .query_row(
            "SELECT state FROM credential_refs WHERE credential_id=?1",
            [credential],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    if state.is_none() {
        return Err(AppError::new(
            "credential_not_found",
            "credentials",
            "凭据不存在",
        ));
    }
    let mut changed = tx.execute("UPDATE credential_refs SET state='revoked',revoked_at_ms=coalesce(revoked_at_ms,?2),lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND state IN ('pending','active')",params![credential,now]).map_err(err)?;
    // A shared pool revocation closes the whole affected connection. Cancel every
    // frozen dependent task, including attempts which selected a different key.
    changed += tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='credential_revoked',finished_at_ms=?2 WHERE state IN ('queued','running') AND (EXISTS(SELECT 1 FROM request_attempts a WHERE a.task_id=request_tasks.task_id AND a.credential_id=?1) OR EXISTS(SELECT 1 FROM binding_engines e JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN credential_pool_members pm ON pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision WHERE e.revision_id=request_tasks.config_revision_id AND e.binding_id=request_tasks.binding_id AND e.engine_id=request_tasks.engine_id AND pm.credential_id=?1))",params![credential,now]).map_err(err)?;
    changed += tx.execute("UPDATE request_attempts SET state='cancelled',error_code='credential_revoked',finished_at_ms=?2 WHERE state='started' AND task_id IN (SELECT task_id FROM request_tasks WHERE state='invalidated' AND invalidation_code='credential_revoked')",params![credential,now]).map_err(err)?;
    // Old immutable revisions do not suspend a current connection after its
    // applied pool has explicitly changed to an unrelated credential set.
    changed += tx.execute("UPDATE connection_runtime_states SET is_suspended=1,control_revision=control_revision+1,reason_code='credential_revoked',updated_at_ms=?2 WHERE (is_suspended=0 OR reason_code IS NOT 'credential_revoked') AND connection_id IN (SELECT DISTINCT c.connection_id FROM connections c JOIN config_heads h ON h.revision_id=c.revision_id AND h.head_kind='applied' JOIN credential_pool_members m ON m.pool_id=c.credential_pool_id AND m.pool_revision=c.credential_pool_revision WHERE m.credential_id=?1)",params![credential,now]).map_err(err)?;
    Ok(changed > 0)
}
pub(in crate::storage) fn mark_credential_cleaned(
    db: &mut Database,
    credential: &str,
    now: i64,
) -> AppResult<()> {
    revoke_credential(db, credential, now)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    tx.execute("UPDATE credential_refs SET state='cleanup_pending',cleanup_requested_at_ms=coalesce(cleanup_requested_at_ms,?2),cleaned_at_ms=coalesce(cleaned_at_ms,?2),lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND state='revoked'",params![credential,now]).map_err(err)?;
    bump(&tx, now)?;
    tx.commit().map_err(err)
}
pub(in crate::storage) fn pending_credentials(
    db: &mut Database,
) -> AppResult<Vec<CredentialReference>> {
    let mut q=db.conn.prepare("SELECT credential_id,vault_ref,pool_id,pool_revision,masked_label FROM (SELECT c.credential_id,c.vault_ref,m.pool_id,m.pool_revision,c.masked_label,c.created_at_ms,row_number() OVER (PARTITION BY c.credential_id ORDER BY m.pool_id,m.pool_revision,m.member_id) AS reference_rank FROM credential_refs c JOIN credential_pool_members m USING(credential_id) WHERE c.state='pending' OR (c.state IN ('revoked','cleanup_pending') AND c.cleaned_at_ms IS NULL)) WHERE reference_rank=1 ORDER BY created_at_ms,credential_id").map_err(err)?;
    q.query_map([], |r| {
        Ok(CredentialReference {
            credential_id: r.get(0)?,
            credential_version_id: r.get(1)?,
            pool_id: r.get(2)?,
            pool_revision: r.get(3)?,
            label: r.get(4)?,
            masked_hint: String::new(),
        })
    })
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)
}
pub(in crate::storage) fn register_credential(
    db: &mut Database,
    r: CredentialReference,
    now: i64,
) -> AppResult<()> {
    let id = r.credential_id.clone();
    begin_credential(db, r, now)?;
    activate_credential(db, &id, now)
}

pub(super) fn metadata(conn: &Connection) -> AppResult<Vec<CredentialMetadata>> {
    let mut q=conn.prepare("SELECT credential_id,masked_label,state,lifecycle_revision FROM credential_refs ORDER BY created_at_ms,credential_id").map_err(err)?;
    q.query_map([], |r| {
        Ok(CredentialMetadata {
            credential_id: r.get(0)?,
            masked_label: r.get(1)?,
            state: r.get(2)?,
            lifecycle_revision: r.get(3)?,
        })
    })
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)
}
pub(super) fn pool(conn: &Connection, id: &str, revision: u32) -> AppResult<CredentialPool> {
    let mut result=conn.query_row("SELECT p.policy,p.policy_metadata_json,p.content_hash FROM credential_pool_revisions p JOIN credential_pool_seals s USING(pool_id,pool_revision) WHERE p.pool_id=?1 AND p.pool_revision=?2 AND NOT EXISTS(SELECT 1 FROM credential_pool_gc_marks g WHERE g.pool_id=p.pool_id AND g.pool_revision=p.pool_revision)",params![id,revision],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(err)?;
    let mut q=conn.prepare("SELECT member_id,credential_id,position,enabled,weight FROM credential_pool_members WHERE pool_id=?1 AND pool_revision=?2 ORDER BY position").map_err(err)?;
    let members = q
        .query_map(params![id, revision], |r| {
            Ok(CredentialPoolMember {
                member_id: r.get(0)?,
                credential_id: r.get(1)?,
                position: r.get(2)?,
                enabled: r.get(3)?,
                weight: r.get(4)?,
            })
        })
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    Ok(CredentialPool {
        pool_id: id.into(),
        revision,
        policy: std::mem::take(&mut result.0),
        metadata: parse(result.1)?,
        members,
        content_hash: result.2,
    })
}
pub(super) fn pools(conn: &Connection) -> AppResult<Vec<CredentialPool>> {
    let mut q=conn.prepare("SELECT p.pool_id,p.pool_revision FROM credential_pool_revisions p JOIN credential_pool_seals s USING(pool_id,pool_revision) WHERE NOT EXISTS(SELECT 1 FROM credential_pool_gc_marks g WHERE g.pool_id=p.pool_id AND g.pool_revision=p.pool_revision) ORDER BY p.pool_id,p.pool_revision DESC").map_err(err)?;
    q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))
        .map_err(err)?
        .map(|row| {
            let (id, revision) = row.map_err(err)?;
            pool(conn, &id, revision)
        })
        .collect()
}
pub(super) fn save_pool(
    tx: &Transaction,
    input: &CredentialPoolInput,
    operation: &str,
    now: i64,
) -> AppResult<(String, u32)> {
    app_core::domain::credentials::validate_pool(input)?;
    let pool_id = input
        .pool_id
        .clone()
        .unwrap_or_else(|| id("pool", operation));
    let current: u32 = tx
        .query_row(
            "SELECT coalesce(max(pool_revision),0) FROM credential_pool_revisions WHERE pool_id=?1",
            [&pool_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if current != input.expected_revision {
        return Err(AppError::new(
            "revision_conflict",
            "credentials",
            "凭据池已被其他窗口修改",
        ));
    }
    let revision = current
        .checked_add(1)
        .ok_or_else(|| AppError::new("revision_exhausted", "credentials", "凭据池版本已耗尽"))?;
    let hash = canonical_hash(&(
        "credential-pool-v1",
        &pool_id,
        revision,
        &input.policy,
        &input.metadata,
        &input.members,
    ))?;
    tx.execute(
        "INSERT INTO credential_pool_revisions VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            pool_id,
            revision,
            input.policy,
            json(&input.metadata)?,
            now,
            hash
        ],
    )
    .map_err(err)?;
    for (position, member) in input.members.iter().enumerate() {
        tx.execute(
            "INSERT INTO credential_pool_members VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                pool_id,
                revision,
                member.member_id.clone().unwrap_or_else(|| id(
                    "member",
                    &format!("{pool_id}:{}", member.credential_id)
                )),
                member.credential_id,
                position as i64,
                member.enabled,
                member.weight
            ],
        )
        .map_err(err)?;
    }
    tx.execute(
        "INSERT INTO credential_pool_seals VALUES(?1,?2,?3,?4)",
        params![pool_id, revision, hash, now],
    )
    .map_err(err)?;
    Ok((pool_id, revision))
}
pub(in crate::storage) fn select_credential(
    db: &mut Database,
    pool_id: &str,
    revision: u32,
) -> AppResult<app_core::domain::credentials::CredentialSelection> {
    let cursor = *db
        .pool_cursors
        .entry((pool_id.into(), revision))
        .or_default();
    let (selection, next) = select_at(&db.conn, pool_id, revision, cursor)?;
    db.pool_cursors.insert((pool_id.into(), revision), next);
    Ok(selection)
}
pub(super) fn select_at(
    conn: &Connection,
    pool_id: &str,
    revision: u32,
    cursor: u64,
) -> AppResult<(app_core::domain::credentials::CredentialSelection, u64)> {
    select_at_for_kind(
        conn,
        pool_id,
        revision,
        cursor,
        app_core::domain::model_config::credentials::CredentialMaterialKind::ApiKey,
    )
}
pub(super) fn select_at_for_kind(
    conn: &Connection,
    pool_id: &str,
    revision: u32,
    cursor: u64,
    kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
) -> AppResult<(app_core::domain::credentials::CredentialSelection, u64)> {
    let mut p = pool(conn, pool_id, revision)?;
    validate_pool_kind(conn, pool_id, revision, kind)?;
    let mut q = conn
        .prepare(
            "SELECT credential_id,vault_ref,masked_label FROM credential_refs WHERE state='active' AND auth_kind=?1",
        )
        .map_err(err)?;
    let refs = q
        .query_map([kind.key()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, String>(2)?),
            ))
        })
        .map_err(err)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(err)?;
    for m in &mut p.members {
        m.enabled = m.enabled && refs.contains_key(&m.credential_id);
    }
    let (member, next) =
        app_core::domain::credentials::choose_member(&p.policy, &p.members, cursor)?;
    let (vault, label) = refs
        .get(&member.credential_id)
        .ok_or_else(|| AppError::new("credential_unavailable", "credentials", "所选凭据已失效"))?;
    Ok((
        app_core::domain::credentials::CredentialSelection {
            credential_id: member.credential_id.clone(),
            vault_ref: vault.clone(),
            pool_id: pool_id.into(),
            pool_revision: revision,
            member_id: member.member_id.clone(),
            masked_identity: label.clone(),
        },
        next,
    ))
}

fn validate_pool_kind(
    conn: &Connection,
    pool_id: &str,
    revision: u32,
    kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
) -> AppResult<()> {
    use app_core::domain::model_config::credentials::CredentialStorageMetadata;
    pool(conn, pool_id, revision)?; // Exact sealed revision, never a latest or GC-marked pool.
    let mut q=conn.prepare("SELECT c.auth_kind,c.envelope_schema FROM credential_pool_members m JOIN credential_refs c USING(credential_id) WHERE m.pool_id=?1 AND m.pool_revision=?2 AND m.enabled=1 AND c.state='active'").map_err(err)?;
    for row in q
        .query_map(params![pool_id, revision], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<u32>>(1)?))
        })
        .map_err(err)?
    {
        let (actual, schema) = row.map_err(err)?;
        if actual != kind.key() {
            return Err(AppError::new(
                "credential_kind_mismatch",
                "credentials",
                "启用成员的认证材料与连接不一致",
            ));
        }
        CredentialStorageMetadata::from_stored(&actual, schema)?;
    }
    Ok(())
}
pub(in crate::storage) fn select_credential_for_kind(
    db: &mut Database,
    pool_id: &str,
    revision: u32,
    kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
) -> AppResult<app_core::domain::credentials::CredentialSelection> {
    let cursor = *db
        .pool_cursors
        .entry((pool_id.into(), revision))
        .or_default();
    let (selection, next) = select_at_for_kind(&db.conn, pool_id, revision, cursor, kind)?;
    db.pool_cursors.insert((pool_id.into(), revision), next);
    Ok(selection)
}
pub(in crate::storage) fn credential_lease_metadata(
    db: &mut Database,
    pool_id: &str,
    revision: u32,
    vault: &str,
    kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
) -> AppResult<app_core::domain::credentials::CredentialLeaseMetadata> {
    use app_core::domain::model_config::credentials::*;
    validate_pool_kind(&db.conn, pool_id, revision, kind)?;
    let row=db.conn.query_row("SELECT c.credential_id,c.vault_ref,c.auth_kind,c.envelope_schema,c.lifecycle_revision,m.member_id FROM credential_refs c JOIN credential_pool_members m USING(credential_id) WHERE m.pool_id=?1 AND m.pool_revision=?2 AND m.enabled=1 AND c.state='active' AND c.vault_ref=?3 AND c.auth_kind=?4",params![pool_id,revision,vault,kind.key()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<u32>>(3)?,r.get::<_,u32>(4)?,r.get::<_,String>(5)?))).optional().map_err(err)?.ok_or_else(||AppError::new("credential_unavailable","credentials","凭据授权已改变或不属于指定池"))?;
    Ok(app_core::domain::credentials::CredentialLeaseMetadata {
        credential: CredentialVersionRef {
            credential_id: row.0,
            credential_version_id: row.1.clone(),
        },
        vault_ref: row.1,
        storage: CredentialStorageMetadata::from_stored(&row.2, row.3)?,
        lifecycle_revision: row.4,
        pool_id: pool_id.into(),
        pool_revision: revision,
        member_id: row.5,
    })
}
pub(in crate::storage) fn credential_pool_materials(
    db: &mut Database,
    pool_id: &str,
    revision: u32,
    kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
) -> AppResult<Vec<app_core::domain::credentials::CredentialLeaseMetadata>> {
    validate_pool_kind(&db.conn, pool_id, revision, kind)?;
    let vault_refs: Vec<String> = {
        let mut statement = db.conn.prepare("SELECT c.vault_ref FROM credential_pool_members m JOIN credential_refs c USING(credential_id) WHERE m.pool_id=?1 AND m.pool_revision=?2 AND m.enabled=1 AND c.state='active' ORDER BY m.position,m.member_id").map_err(err)?;
        statement.query_map(params![pool_id, revision], |row| row.get(0)).map_err(err)?
            .collect::<Result<_, _>>().map_err(err)?
    };
    if vault_refs.is_empty() {
        return Err(AppError::new("credential_unavailable", "credentials", "凭据池没有可准备的有效材料"));
    }
    vault_refs.into_iter().map(|vault| credential_lease_metadata(db, pool_id, revision, &vault, kind)).collect()
}
