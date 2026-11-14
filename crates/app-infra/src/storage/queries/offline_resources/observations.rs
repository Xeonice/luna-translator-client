use super::*;
pub(super) fn installation_head(
    tx: &Transaction<'_>,
    installation: &LocalInstallationRef,
    expected: Option<NonZeroU32>,
) -> AppResult<()> {
    let current: Option<(u32,u32)> = tx.query_row("SELECT installation_revision,row_revision FROM offline_installation_heads WHERE installation_id=?1", [&installation.installation_id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
    match current {
        None if expected.is_none() => {
            tx.execute(
                "INSERT INTO offline_installation_heads VALUES(?1,?2,1,0)",
                params![installation.installation_id, installation.revision.get()],
            )
            .map_err(err)?;
        }
        Some((version, row))
            if expected.is_some_and(|e| e.get() == row)
                && installation.revision.get() == version => {}
        Some((version, row))
            if expected.is_some_and(|e| e.get() == row)
                && installation.revision.get() > version =>
        {
            if row == u32::MAX {
                return Err(fail("offline_head_overflow"));
            }
            let changed = tx.execute("UPDATE offline_installation_heads SET installation_revision=?1,row_revision=row_revision+1,enabled=0 WHERE installation_id=?2 AND row_revision=?3", params![installation.revision.get(),installation.installation_id,row]).map_err(err)?;
            if changed != 1 {
                return Err(fail("offline_installation_head_conflict"));
            }
        }
        _ => return Err(fail("offline_installation_head_conflict")),
    }
    Ok(())
}
pub(super) fn unknown_observation(
    tx: &Transaction<'_>,
    p: &OfflineUnknownPreparation,
    now: i64,
) -> AppResult<()> {
    let r = p.material.reference();
    let o = &r.observation;
    let current:Option<(u32,u32)>=tx.query_row("SELECT observation_epoch,row_revision FROM offline_observation_heads WHERE subject_key=?1",[&o.subject_key],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(err)?;
    let next = match current {
        None if p.metadata.expected_observation_head.is_none() => 1,
        Some((epoch, row))
            if p.metadata
                .expected_observation_head
                .is_some_and(|v| v.get() == row)
                && o.epoch.get() > epoch =>
        {
            row.checked_add(1)
                .ok_or_else(|| fail("offline_head_overflow"))?
        }
        _ => return Err(fail("offline_observation_head_conflict")),
    };
    if o.head_revision.get() != next {
        return Err(fail("offline_observation_head_conflict"));
    }
    let (installation, revision, grant, grant_epoch) = match &r.backend {
        ResourceBackendIdentity::LocalInstallation {
            installation, root, ..
        } => (
            Some(installation.installation_id.clone()),
            Some(installation.revision.get()),
            Some(root.grant_id.clone()),
            Some(root.authorization_epoch.get()),
        ),
        ResourceBackendIdentity::AppleSystem { .. } => (None, None, None, None),
    };
    tx.execute("INSERT INTO offline_resource_observations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'unknown',NULL,?9)",params![o.observation_id,o.subject_key,o.epoch.get(),installation,revision,r.snapshot_id,grant,grant_epoch,now]).map_err(err)?;
    if current.is_none() {
        tx.execute(
            "INSERT INTO offline_observation_heads VALUES(?1,?2,?3,1)",
            params![o.subject_key, o.epoch.get(), o.observation_id],
        )
        .map_err(err)?;
    } else {
        let n=tx.execute("UPDATE offline_observation_heads SET observation_epoch=?1,observation_id=?2,row_revision=?3 WHERE subject_key=?4 AND row_revision=?5",params![o.epoch.get(),o.observation_id,next,o.subject_key,next-1]).map_err(err)?;
        if n != 1 {
            return Err(fail("offline_observation_head_conflict"));
        }
    }
    Ok(())
}
pub(super) fn revoke(
    db: &mut Database,
    operation: &str,
    root: &GrantedRootRef,
    entry: &LiveEntry,
    budget: &OperationBudget,
    now: i64,
) -> AppResult<()> {
    let hash = canonical_hash(&("luna-offline-root-revoke-v1", root))?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if replay_revoke(&tx, operation, root)? {
        return Ok(());
    }
    let next = root
        .authorization_epoch
        .get()
        .checked_add(1)
        .ok_or_else(|| fail("offline_grant_epoch_overflow"))?;
    let changed=tx.execute("UPDATE offline_local_grants SET authorization_epoch=?1,state='revoked' WHERE grant_id=?2 AND machine_fingerprint=?3 AND authorization_epoch=?4",params![next,root.grant_id,root.machine_fingerprint,root.authorization_epoch.get()]).map_err(err)?;
    if changed != 1 {
        return Err(fail("offline_grant_changed"));
    }
    let revision = bump(&tx, now)?;
    tx.execute(
        "INSERT INTO operation_receipts VALUES(?1,?2,'offline_root_revoke',?3,'{}',?4)",
        params![operation, hash, revision, now],
    )
    .map_err(err)?;
    check_entry(entry, budget)?;
    tx.commit().map_err(err)?;
    Ok(())
}

pub(super) fn replay_revoke(
    conn: &Connection,
    operation: &str,
    root: &GrantedRootRef,
) -> AppResult<bool> {
    let hash = canonical_hash(&("luna-offline-root-revoke-v1", root))?;
    let old: Option<(String, String)> = conn
        .query_row(
            "SELECT request_sha256,command_kind FROM operation_receipts WHERE operation_id=?1",
            [operation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    match old {
        None => Ok(false),
        Some((old, kind)) if old == hash && kind == "offline_root_revoke" => Ok(true),
        _ => Err(fail("operation_conflict")),
    }
}
