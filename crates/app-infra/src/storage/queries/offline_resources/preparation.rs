use super::*;
use rusqlite::types::Value as SqlValue;
fn s(v: impl Into<String>) -> SqlValue {
    SqlValue::Text(v.into())
}
fn n(v: impl Into<i64>) -> SqlValue {
    SqlValue::Integer(v.into())
}
fn opt(v: Option<String>) -> SqlValue {
    v.map(SqlValue::Text).unwrap_or(SqlValue::Null)
}
fn num(v: Option<u32>) -> SqlValue {
    v.map(|v| n(i64::from(v))).unwrap_or(SqlValue::Null)
}
pub(super) fn tag<T: serde::Serialize>(v: &T) -> AppResult<String> {
    serde_json::to_value(v)
        .map_err(err)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| fail("offline_invalid_tag"))
}
/// Only code-owned table and key expressions call this helper; never user SQL.
fn immutable(
    tx: &Transaction<'_>,
    table: &str,
    key: &str,
    keys: &[SqlValue],
    values: Vec<SqlValue>,
    retain_created: bool,
) -> AppResult<()> {
    let sql = format!("SELECT * FROM {table} WHERE {key}");
    let row = tx
        .query_row(&sql, rusqlite::params_from_iter(keys), |r| {
            (0..r.as_ref().column_count())
                .map(|i| r.get::<_, SqlValue>(i))
                .collect::<Result<Vec<_>, _>>()
        })
        .optional()
        .map_err(err)?;
    if let Some(mut actual) = row {
        if actual.len() != values.len() {
            return Err(fail("offline_storage_shape_mismatch"));
        }
        let mut expected = values;
        if retain_created {
            let last = actual.len() - 1;
            expected[last] = actual[last].clone();
        }
        if actual != expected {
            return Err(fail("offline_immutable_fact_conflict"));
        }
        actual.clear();
        return Ok(());
    }
    let placeholders = (1..=values.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    tx.execute(
        &format!("INSERT INTO {table} VALUES({placeholders})"),
        rusqlite::params_from_iter(values),
    )
    .map_err(err)?;
    Ok(())
}
pub(super) fn replay(
    conn: &Connection,
    p: &OfflineUnknownPreparation,
) -> AppResult<Option<OfflineStoredPreparationReceipt>> {
    let row: Option<(String, String, String)> = conn.query_row("SELECT request_sha256,command_kind,result_refs_json FROM operation_receipts WHERE operation_id=?1", [p.operation_id()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(err)?;
    match row {
        None => Ok(None),
        Some((hash, kind, _))
            if hash != p.request_sha256 || kind != "offline_unknown_preparation" =>
        {
            Err(fail("operation_conflict"))
        }
        Some((_, _, value)) => parse(value).map(Some),
    }
}
pub(super) fn commit(
    db: &mut Database,
    p: &OfflineUnknownPreparation,
    entry: &LiveEntry,
    budget: &OperationBudget,
    now: i64,
) -> AppResult<OfflineStoredPreparationReceipt> {
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if let Some(receipt) = replay(&tx, p)? {
        return Ok(receipt);
    }
    check_entry(entry, budget)?;
    let r = p.material.reference();
    let profile = &r.profile;
    let m = &p.metadata;
    let engine = profile.engine.engine_id.clone();
    let contract = profile.engine.contract_revision.get();
    let backend = tag(&profile.backend)?;
    immutable(
        &tx,
        "offline_runtime_profiles",
        "runtime_profile_id=?1 AND runtime_profile_revision=?2",
        &[s(&profile.profile_id), n(profile.revision.get())],
        vec![
            s(&profile.profile_id),
            n(profile.revision.get()),
            s(&engine),
            n(contract),
            s(&backend),
            s(tag(&profile.expected_worker_platform())?),
            s(tag(&profile.architecture)?),
            n(profile.private_protocol_revision.get()),
            opt(m.abi_profile_id.clone()),
            s("unknown"),
            s(&profile.policy_sha256),
            n(now),
        ],
        true,
    )?;
    if let ResourceBackendIdentity::LocalInstallation {
        installation,
        environment,
        root,
        ..
    } = &r.backend
    {
        let local = m
            .local
            .as_ref()
            .ok_or_else(|| fail("offline_local_material_missing"))?;
        let e = &local.environment;
        immutable(
            &tx,
            "offline_execution_environments",
            "environment_id=?1 AND environment_revision=?2",
            &[
                s(&environment.environment_id),
                n(environment.revision.get()),
            ],
            vec![
                s(&environment.environment_id),
                n(environment.revision.get()),
                s(tag(&e.platform)?),
                s(tag(&e.architecture)?),
                s(&environment.machine_fingerprint),
                s(&e.os_build),
                opt(e.bottle_identity.clone()),
                opt(e.local_prefix_path.clone()),
                opt(e.wine_build.clone()),
                n(now),
            ],
            true,
        )?;
        let grant: Option<(String, String, u32)> = tx.query_row("SELECT machine_fingerprint,opaque_locator,authorization_epoch FROM offline_local_grants WHERE grant_id=?1", [&root.grant_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(err)?;
        if let Some((machine, locator, epoch)) = grant {
            if machine != root.machine_fingerprint
                || locator != local.opaque_locator
                || epoch != root.authorization_epoch.get()
            {
                return Err(fail("offline_grant_changed"));
            }
        } else {
            tx.execute(
                "INSERT INTO offline_local_grants VALUES(?1,?2,?3,?4,'unknown',?5)",
                params![
                    root.grant_id,
                    root.machine_fingerprint,
                    local.opaque_locator,
                    root.authorization_epoch.get(),
                    now
                ],
            )
            .map_err(err)?;
        }
        immutable(
            &tx,
            "offline_installation_versions",
            "installation_id=?1 AND installation_revision=?2",
            &[
                s(&installation.installation_id),
                n(installation.revision.get()),
            ],
            vec![
                s(&installation.installation_id),
                n(installation.revision.get()),
                s(&engine),
                n(contract),
                s(&environment.environment_id),
                n(environment.revision.get()),
                s(local
                    .canonical_root
                    .to_str()
                    .ok_or_else(|| fail("offline_invalid_path"))?),
                s(&root.grant_id),
                s(&profile.profile_id),
                n(profile.revision.get()),
                n(now),
            ],
            true,
        )?;
        observations::installation_head(&tx, installation, m.expected_installation_head)?;
    }
    let (kind, manifest, os, bridge, api, strategy, source, target, epoch) = match &p.snapshot {
        OfflineSnapshotIdentity::FileManifest { manifest_sha256 } => (
            "file_manifest",
            Some(manifest_sha256.clone()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        OfflineSnapshotIdentity::AppleObservation {
            os_build,
            bridge_build_sha256,
            api_profile,
            strategy,
            pair,
            observation_epoch,
        } => (
            "apple_observation",
            None,
            Some(os_build.clone()),
            Some(bridge_build_sha256.clone()),
            Some(api_profile.clone()),
            Some(tag(strategy)?),
            Some(pair.source.clone()),
            Some(pair.target.clone()),
            Some(observation_epoch.get()),
        ),
    };
    immutable(
        &tx,
        "offline_resource_snapshots",
        "snapshot_id=?1",
        &[s(&r.snapshot_id)],
        vec![
            s(&r.snapshot_id),
            s(&engine),
            n(contract),
            s(&backend),
            s(&profile.profile_id),
            n(profile.revision.get()),
            s(kind),
            opt(manifest),
            opt(os),
            opt(bridge),
            opt(api),
            opt(strategy),
            opt(source),
            opt(target),
            num(epoch),
            s(&r.resource_content_sha256),
            n(now),
        ],
        true,
    )?;
    // A sealed existing snapshot is checked in full below instead of silently
    // attempting INSERT OR IGNORE into its immutable children.
    let sealed: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM offline_resource_seals WHERE snapshot_id=?1)",
            [&r.snapshot_id],
            |row| row.get(0),
        )
        .map_err(err)?;
    if !sealed {
        if let Some(inspected) = p.material.inspected_local() {
            for f in inspected.files() {
                immutable(
                    &tx,
                    "offline_resource_snapshot_files",
                    "snapshot_id=?1 AND relative_key=?2",
                    &[s(&r.snapshot_id), s(&f.relative_key)],
                    vec![
                        s(&r.snapshot_id),
                        s(&f.relative_key),
                        n(i64::try_from(f.byte_length).map_err(err)?),
                        s(&f.sha256),
                        s(tag(&f.role)?),
                    ],
                    false,
                )?;
            }
        }
        for pair in facts::sorted_pairs(&m.pairs)? {
            immutable(
                &tx,
                "offline_resource_pairs",
                "snapshot_id=?1 AND source_language=?2 AND target_language=?3 AND pipeline_profile_id=?4",
                &[
                    s(&r.snapshot_id),
                    s(&pair.pair.source),
                    s(&pair.pair.target),
                    s(&pair.pipeline_profile_id),
                ],
                vec![
                    s(&r.snapshot_id),
                    s(pair.pair.source),
                    s(pair.pair.target),
                    s(pair.pipeline_profile_id),
                ],
                false,
            )?;
        }
        immutable(
            &tx,
            "offline_resource_seals",
            "snapshot_id=?1",
            &[s(&r.snapshot_id)],
            vec![s(&r.snapshot_id), s(&r.resource_content_sha256), n(now)],
            true,
        )?;
    }
    let route_hash = offline_route_content_hash(profile, &r.pair, &m.route_policy, &m.route)?;
    let (route_kind, consent) = match &m.route_policy {
        OfflineRoutePolicy::DirectOnly {} => ("direct", None),
        OfflineRoutePolicy::ExplicitPivot { consent_revision } => {
            ("explicit_pivot", Some(consent_revision.get()))
        }
    };
    immutable(
        &tx,
        "offline_translation_routes",
        "route_id=?1",
        &[s(&r.route_id)],
        vec![
            s(&r.route_id),
            s(&engine),
            n(contract),
            s(&backend),
            s(route_kind),
            s(&r.pair.source),
            s(&r.pair.target),
            num(consent),
            s(&route_hash),
            n(now),
        ],
        true,
    )?;
    let route_sealed: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM offline_route_seals WHERE route_id=?1)",
            [&r.route_id],
            |row| row.get(0),
        )
        .map_err(err)?;
    if !route_sealed {
        for (i, hop) in m.route.iter().enumerate() {
            immutable(
                &tx,
                "offline_translation_route_hops",
                "route_id=?1 AND hop_ordinal=?2",
                &[s(&r.route_id), n(i as i64)],
                vec![
                    s(&r.route_id),
                    n(i as i64),
                    s(&hop.snapshot_id),
                    s(&engine),
                    n(contract),
                    s(&backend),
                    s(&hop.pair.source),
                    s(&hop.pair.target),
                    s(&hop.pipeline_profile_id),
                ],
                false,
            )?;
        }
        immutable(
            &tx,
            "offline_route_seals",
            "route_id=?1",
            &[s(&r.route_id)],
            vec![s(&r.route_id), s(route_hash), n(now)],
            true,
        )?;
    }
    observations::unknown_observation(&tx, p, now)?;
    // Same complete production reader validates actual children/seals, including
    // existing immutable rows, before committing anything or issuing a receipt.
    let lookup = OfflineMaterialLookup {
        snapshot_id: r.snapshot_id.clone(),
        route_id: r.route_id.clone(),
        subject_key: r.observation.subject_key.clone(),
        source: None,
    };
    let _ = authority::read_connection(&tx, &lookup)?;
    let receipt = OfflineStoredPreparationReceipt {
        operation_id: p.operation_id().into(),
        reference: r.clone(),
        state: "unknown".into(),
    };
    let revision = bump(&tx, now)?;
    tx.execute(
        "INSERT INTO operation_receipts VALUES(?1,?2,'offline_unknown_preparation',?3,?4,?5)",
        params![
            p.operation_id(),
            p.request_sha256,
            revision,
            json(&receipt)?,
            now
        ],
    )
    .map_err(err)?;
    check_entry(entry, budget)?;
    tx.commit().map_err(err)?;
    Ok(receipt)
}
