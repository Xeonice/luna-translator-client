//! Actor-local full row reads. No Owner callback, file I/O or Expected fabrication.
use super::source::{field, objects, one};
use super::*;
use facts::{OfflineEnvironmentMetadata, OfflineSnapshotIdentity, StoredRows};
use serde_json::Value;
fn nz(value: u32) -> AppResult<NonZeroU32> {
    NonZeroU32::new(value).ok_or_else(|| fail("offline_stored_fact_invalid"))
}
pub(super) fn read(
    db: &Database,
    lookup: &OfflineMaterialLookup,
) -> AppResult<(StoredRows, String)> {
    read_connection(&db.conn, lookup)
}
pub(super) fn read_connection(
    conn: &Connection,
    lookup: &OfflineMaterialLookup,
) -> AppResult<(StoredRows, String)> {
    require_id(&lookup.snapshot_id)?;
    require_id(&lookup.route_id)?;
    facts::text(&lookup.subject_key, 512)?;
    let snap = one(
        conn,
        "SELECT * FROM offline_resource_snapshots WHERE snapshot_id=?1",
        &[&lookup.snapshot_id],
    )?;
    let profile_id: String = field(&snap, "runtime_profile_id")?;
    let profile_rev: u32 = field(&snap, "runtime_profile_revision")?;
    let profile_row = one(
        conn,
        "SELECT * FROM offline_runtime_profiles WHERE runtime_profile_id=?1 AND runtime_profile_revision=?2",
        &[&profile_id, &profile_rev],
    )?;
    let head = one(
        conn,
        "SELECT * FROM offline_observation_heads WHERE subject_key=?1",
        &[&lookup.subject_key],
    )?;
    let observation_id: String = field(&head, "observation_id")?;
    let obs = one(
        conn,
        "SELECT * FROM offline_resource_observations WHERE observation_id=?1",
        &[&observation_id],
    )?;
    let observation = ObservationRef {
        observation_id,
        epoch: nz(field(&head, "observation_epoch")?)?,
        subject_key: lookup.subject_key.clone(),
        head_revision: nz(field(&head, "row_revision")?)?,
    };
    if field::<String>(&obs, "subject_key")? != lookup.subject_key
        || field::<u32>(&obs, "observation_epoch")? != observation.epoch.get()
        || field::<Option<String>>(&obs, "snapshot_id")?.as_deref() != Some(&lookup.snapshot_id)
    {
        return Err(fail("offline_observation_mismatch"));
    }
    let profile = RuntimeProfileRef {
        profile_id,
        revision: nz(profile_rev)?,
        engine: OfflineEngineKey {
            engine_id: field(&profile_row, "engine_id")?,
            contract_revision: nz(field(&profile_row, "engine_contract_revision")?)?,
        },
        backend: field(&profile_row, "backend")?,
        platform: super::source::parent_platform(conn, &profile_row, &obs)?,
        architecture: field(&profile_row, "architecture")?,
        private_protocol_revision: nz(field(&profile_row, "private_protocol_revision")?)?,
        policy_sha256: field(&profile_row, "policy_sha256")?,
    };
    profile.validate()?;
    crate::offline_translation::runtime_policy::policy_for_profile(&profile)
        .map_err(|_| fail("offline_runtime_policy_unavailable"))?;
    let verification: String = field(&profile_row, "verification_state")?;
    if !matches!(
        verification.as_str(),
        "unknown" | "verified" | "unsupported"
    ) {
        return Err(fail("offline_stored_fact_invalid"));
    }
    let abi_profile_id: Option<String> = field(&profile_row, "abi_profile_id")?;
    if let Some(v) = &abi_profile_id {
        facts::text(v, 512)?;
    }
    let engine: String = field(&snap, "engine_id")?;
    let contract: u32 = field(&snap, "engine_contract_revision")?;
    let backend: OfflineBackend = field(&snap, "backend")?;
    if engine != profile.engine.engine_id
        || contract != profile.engine.contract_revision.get()
        || backend != profile.backend
    {
        return Err(fail("offline_snapshot_profile_mismatch"));
    }
    let files_raw = objects(
        conn,
        "SELECT * FROM offline_resource_snapshot_files WHERE snapshot_id=?1 ORDER BY relative_key",
        &[&lookup.snapshot_id],
    )?;
    let pairs_raw = objects(
        conn,
        "SELECT * FROM offline_resource_pairs WHERE snapshot_id=?1 ORDER BY source_language,target_language,pipeline_profile_id",
        &[&lookup.snapshot_id],
    )?;
    let mut pairs = Vec::new();
    for row in &pairs_raw {
        pairs.push(OfflineResourcePair {
            pair: LanguagePair {
                source: field(row, "source_language")?,
                target: field(row, "target_language")?,
            },
            pipeline_profile_id: field(row, "pipeline_profile_id")?,
        });
    }
    let pairs = facts::sorted_pairs(&pairs)?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for row in &files_raw {
        let key: String = field(row, "relative_key")?;
        validate_relative_key(&key)?;
        let length: u64 = field(row, "byte_length")?;
        let hash: String = field(row, "sha256")?;
        facts::digest(&hash)?;
        let role: StoredFileRole = field(row, "role")?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= crate::offline_translation::resources::MAX_RESOURCE_BYTES)
            .ok_or_else(|| fail("offline_resource_budget_exceeded"))?;
        files.push((key, length, hash, role));
    }
    if files.len() > MAX_SNAPSHOT_FILES {
        return Err(fail("offline_resource_budget_exceeded"));
    }
    let identity_kind: String = field(&snap, "identity_kind")?;
    let identity = match identity_kind.as_str() {
        "file_manifest" => {
            if profile.backend != OfflineBackend::LocalWorker || files.is_empty() {
                return Err(fail("offline_snapshot_profile_mismatch"));
            }
            let control = files
                .iter()
                .find(|f| f.0 == CONTROL_RELATIVE_KEY)
                .ok_or_else(|| fail("offline_control_row_missing"))?;
            if control.1 == 0
                || control.1 > MAX_CONTROL_BYTES as u64
                || control.3 != StoredFileRole::Runtime
            {
                return Err(fail("offline_control_row_invalid"));
            }
            let tuples = files
                .iter()
                .map(|f| (&f.0, f.1, &f.2, &f.3))
                .collect::<Vec<_>>();
            let actual = canonical_hash(&("offline-snapshot-files-v1", tuples))?;
            let manifest: String = field(&snap, "manifest_sha256")?;
            if actual != manifest {
                return Err(fail("offline_manifest_mismatch"));
            }
            OfflineSnapshotIdentity::FileManifest {
                manifest_sha256: manifest,
            }
        }
        "apple_observation" => {
            if !files.is_empty() {
                return Err(fail("offline_apple_files_forbidden"));
            }
            OfflineSnapshotIdentity::AppleObservation {
                os_build: field(&snap, "os_build")?,
                bridge_build_sha256: field(&snap, "bridge_build_sha256")?,
                api_profile: field(&snap, "api_profile")?,
                strategy: field(&snap, "strategy")?,
                pair: LanguagePair {
                    source: field(&snap, "source_language")?,
                    target: field(&snap, "target_language")?,
                },
                observation_epoch: nz(field(&snap, "observation_epoch")?)?,
            }
        }
        _ => return Err(fail("offline_stored_fact_invalid")),
    };
    let resource_hash: String = field(&snap, "content_hash")?;
    let seal = one(
        conn,
        "SELECT * FROM offline_resource_seals WHERE snapshot_id=?1",
        &[&lookup.snapshot_id],
    )?;
    if offline_snapshot_content_hash(&profile, &identity, &pairs)? != resource_hash
        || field::<String>(&seal, "content_hash")? != resource_hash
    {
        return Err(fail("offline_snapshot_seal_mismatch"));
    }
    let route_row = one(
        conn,
        "SELECT * FROM offline_translation_routes WHERE route_id=?1",
        &[&lookup.route_id],
    )?;
    let hops_raw = objects(
        conn,
        "SELECT * FROM offline_translation_route_hops WHERE route_id=?1 ORDER BY hop_ordinal",
        &[&lookup.route_id],
    )?;
    let pair = LanguagePair {
        source: field(&route_row, "source_language")?,
        target: field(&route_row, "target_language")?,
    };
    let route_kind: String = field(&route_row, "route_kind")?;
    let consent: Option<u32> = field(&route_row, "consent_revision")?;
    let route_policy = match (route_kind.as_str(), consent) {
        ("direct", None) => OfflineRoutePolicy::DirectOnly {},
        ("explicit_pivot", Some(n)) => OfflineRoutePolicy::ExplicitPivot {
            consent_revision: nz(n)?,
        },
        _ => return Err(fail("offline_stored_fact_invalid")),
    };
    if field::<String>(&route_row, "engine_id")? != engine
        || field::<u32>(&route_row, "engine_contract_revision")? != contract
        || field::<OfflineBackend>(&route_row, "backend")? != backend
    {
        return Err(fail("offline_route_profile_mismatch"));
    }
    let mut route = Vec::new();
    for (i, h) in hops_raw.iter().enumerate() {
        if field::<usize>(h, "hop_ordinal")? != i
            || field::<String>(h, "snapshot_id")? != lookup.snapshot_id
            || field::<String>(h, "engine_id")? != engine
            || field::<u32>(h, "engine_contract_revision")? != contract
            || field::<OfflineBackend>(h, "backend")? != backend
        {
            return Err(fail("offline_route_snapshot_unavailable"));
        }
        let hop = OfflineRouteHop {
            snapshot_id: lookup.snapshot_id.clone(),
            pair: LanguagePair {
                source: field(h, "source_language")?,
                target: field(h, "target_language")?,
            },
            pipeline_profile_id: field(h, "pipeline_profile_id")?,
            package_relative_key: None,
        };
        if !pairs
            .iter()
            .any(|p| p.pair == hop.pair && p.pipeline_profile_id == hop.pipeline_profile_id)
        {
            return Err(fail("offline_route_pair_mismatch"));
        }
        route.push(hop);
    }
    let route_hash: String = field(&route_row, "content_hash")?;
    let route_seal = one(
        conn,
        "SELECT * FROM offline_route_seals WHERE route_id=?1",
        &[&lookup.route_id],
    )?;
    if offline_route_content_hash(&profile, &pair, &route_policy, &route)? != route_hash
        || field::<String>(&route_seal, "content_hash")? != route_hash
    {
        return Err(fail("offline_route_seal_mismatch"));
    }
    let observation_state: String = field(&obs, "state")?;
    if !matches!(
        observation_state.as_str(),
        "unknown"
            | "installed"
            | "missing"
            | "changed"
            | "permission_denied"
            | "unsupported"
            | "error"
    ) {
        return Err(fail("offline_stored_fact_invalid"));
    }
    let mut raw = vec![
        ("profile", profile_row),
        ("snapshot", snap),
        ("files", Value::Array(files_raw)),
        ("pairs", Value::Array(pairs_raw)),
        ("seal", seal),
        ("route", route_row),
        ("hops", Value::Array(hops_raw)),
        ("route_seal", route_seal),
        ("observation", obs.clone()),
        ("observation_head", head),
    ];
    let (
        mut installation,
        mut installation_head_revision,
        mut environment,
        mut environment_metadata,
        mut root,
        mut root_state,
        mut opaque_locator,
        mut local_native_path,
    ) = (None, None, None, None, None, None, None, None);
    if profile.backend == OfflineBackend::LocalWorker {
        let id: String = field(&obs, "installation_id")?;
        let revision: u32 = field(&obs, "installation_revision")?;
        let inst = one(
            conn,
            "SELECT * FROM offline_installation_versions WHERE installation_id=?1 AND installation_revision=?2",
            &[&id, &revision],
        )?;
        let ih = one(
            conn,
            "SELECT * FROM offline_installation_heads WHERE installation_id=?1",
            &[&id],
        )?;
        if field::<u32>(&ih, "installation_revision")? != revision
            || field::<String>(&inst, "engine_id")? != engine
            || field::<u32>(&inst, "engine_contract_revision")? != contract
            || field::<String>(&inst, "runtime_profile_id")? != profile.profile_id
            || field::<u32>(&inst, "runtime_profile_revision")? != profile.revision.get()
        {
            return Err(fail("offline_installation_changed"));
        }
        installation = Some(LocalInstallationRef {
            installation_id: id,
            revision: nz(revision)?,
        });
        installation_head_revision = Some(nz(field(&ih, "row_revision")?)?);
        let env_id: String = field(&inst, "environment_id")?;
        let env_rev: u32 = field(&inst, "environment_revision")?;
        let env = one(
            conn,
            "SELECT * FROM offline_execution_environments WHERE environment_id=?1 AND environment_revision=?2",
            &[&env_id, &env_rev],
        )?;
        let metadata = OfflineEnvironmentMetadata {
            os_build: field(&env, "os_build")?,
            platform: field(&env, "platform")?,
            architecture: field(&env, "architecture")?,
            bottle_identity: field(&env, "bottle_identity")?,
            local_prefix_path: field(&env, "local_prefix_path")?,
            wine_build: field(&env, "wine_build")?,
        };
        facts::validate_environment(&metadata)?;
        if metadata.platform != profile.platform {
            return Err(fail("offline_environment_mismatch"));
        }
        let machine: String = field(&env, "machine_fingerprint")?;
        facts::digest(&machine)?;
        environment = Some(RuntimeEnvironmentRef {
            environment_id: env_id,
            revision: nz(env_rev)?,
            machine_fingerprint: machine.clone(),
        });
        environment_metadata = Some(metadata);
        let grant_id: String = field(&inst, "local_grant_id")?;
        let grant = one(
            conn,
            "SELECT * FROM offline_local_grants WHERE grant_id=?1",
            &[&grant_id],
        )?;
        let epoch: u32 = field(&grant, "authorization_epoch")?;
        if field::<String>(&grant, "machine_fingerprint")? != machine
            || field::<Option<String>>(&obs, "local_grant_id")?.as_ref() != Some(&grant_id)
            || field::<Option<u32>>(&obs, "local_grant_epoch")? != Some(epoch)
        {
            return Err(fail("offline_grant_changed"));
        }
        root = Some(GrantedRootRef {
            grant_id,
            machine_fingerprint: machine,
            authorization_epoch: nz(epoch)?,
        });
        let state: String = field(&grant, "state")?;
        if !matches!(state.as_str(), "unknown" | "active" | "revoked") {
            return Err(fail("offline_stored_fact_invalid"));
        }
        root_state = Some(state);
        let locator: String = field(&grant, "opaque_locator")?;
        facts::text(&locator, 512)?;
        opaque_locator = Some(locator);
        let path: String = field(&inst, "local_native_path")?;
        facts::text(&path, 8192)?;
        if !std::path::Path::new(&path).is_absolute() {
            return Err(fail("offline_invalid_path"));
        }
        local_native_path = Some(path);
        raw.extend([
            ("installation", inst),
            ("installation_head", ih),
            ("environment", env),
            ("grant", grant),
        ]);
    } else if field::<Option<String>>(&obs, "installation_id")?.is_some()
        || field::<Option<u32>>(&obs, "installation_revision")?.is_some()
        || field::<Option<String>>(&obs, "local_grant_id")?.is_some()
        || field::<Option<u32>>(&obs, "local_grant_epoch")?.is_some()
    {
        return Err(fail("offline_apple_installation_forbidden"));
    }
    let source_hash = super::source::read_source(
        conn,
        lookup,
        profile.clone(),
        pair.clone(),
        route_policy.clone(),
        installation.clone(),
        &mut raw,
    )?;
    let fingerprint = facts::canonical_sorted(&("luna-offline-row-checkpoint-v1", raw))?;
    Ok((
        StoredRows {
            profile,
            abi_profile_id,
            snapshot_id: lookup.snapshot_id.clone(),
            identity,
            resource_content_sha256: resource_hash,
            pairs,
            route_id: lookup.route_id.clone(),
            pair,
            route_policy,
            route,
            route_sha256: route_hash,
            observation,
            observation_state,
            installation,
            installation_head_revision,
            environment,
            environment_metadata,
            root,
            root_state,
            opaque_locator,
            local_native_path,
            files,
            source_hash,
        },
        fingerprint,
    ))
}
