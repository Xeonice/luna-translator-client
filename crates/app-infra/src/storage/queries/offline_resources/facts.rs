//! Complete SQL material claims. None of these values grants execution authority.
use super::*;
use crate::offline_translation::{
    closure::{StoredRuntimeFileRow, StoredRuntimeMaterialSkeleton},
    resolver::ResolvedOfflineMaterial,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineResourcePair {
    pub pair: LanguagePair,
    pub pipeline_profile_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OfflineSnapshotIdentity {
    FileManifest {
        manifest_sha256: String,
    },
    AppleObservation {
        os_build: String,
        bridge_build_sha256: String,
        api_profile: String,
        strategy: AppleStrategy,
        pair: LanguagePair,
        observation_epoch: NonZeroU32,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineEnvironmentMetadata {
    pub os_build: String,
    pub platform: RuntimePlatform,
    pub architecture: RuntimeArchitecture,
    pub bottle_identity: Option<String>,
    pub local_prefix_path: Option<String>,
    pub wine_build: Option<String>,
}
/// Locator/path are private backend material, never IPC or a grant proof.
#[derive(Clone)]
pub struct OfflineLocalLocation {
    pub opaque_locator: String,
    pub canonical_root: PathBuf,
    pub environment: OfflineEnvironmentMetadata,
}
#[derive(Clone)]
pub struct OfflineUnknownMetadata {
    pub operation_id: String,
    pub frame: app_core::ports::offline_resources::OfflineProfileFrame,
    pub abi_profile_id: Option<String>,
    pub local: Option<OfflineLocalLocation>,
    pub pairs: Vec<OfflineResourcePair>,
    pub route: Vec<OfflineRouteHop>,
    pub route_policy: OfflineRoutePolicy,
    pub expected_installation_head: Option<NonZeroU32>,
    pub expected_observation_head: Option<NonZeroU32>,
}
/// Created from the real O2 private material, still only an Unknown inventory.
/// ```compile_fail
/// use app_infra::storage::OfflineUnknownPreparation;
/// fn export(v: &OfflineUnknownPreparation) { let _ = serde_json::to_string(v); }
/// ```
pub struct OfflineUnknownPreparation {
    pub(super) metadata: OfflineUnknownMetadata,
    pub(super) material: Arc<ResolvedOfflineMaterial>,
    pub(super) snapshot: OfflineSnapshotIdentity,
    pub(super) request_sha256: String,
}
impl std::fmt::Debug for OfflineUnknownPreparation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OfflineUnknownPreparation")
            .field("operation_id", &self.metadata.operation_id)
            .field("snapshot_id", &self.material.reference().snapshot_id)
            .finish_non_exhaustive()
    }
}
pub(super) fn text(v: &str, max: usize) -> AppResult<()> {
    if v.is_empty() || v.len() > max || v.chars().any(char::is_control) {
        return Err(fail("offline_invalid_text"));
    }
    Ok(())
}
pub(super) fn digest(v: &str) -> AppResult<()> {
    if v.len() != 64
        || !v
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(fail("offline_invalid_digest"));
    }
    Ok(())
}
pub(super) fn sorted_pairs(pairs: &[OfflineResourcePair]) -> AppResult<Vec<OfflineResourcePair>> {
    if pairs.is_empty() || pairs.len() > 16_384 {
        return Err(fail("offline_invalid_pairs"));
    }
    let mut pairs = pairs.to_vec();
    for p in &pairs {
        text(&p.pair.source, 32)?;
        text(&p.pair.target, 32)?;
        text(&p.pipeline_profile_id, 160)?;
        if p.pair.source == p.pair.target {
            return Err(fail("offline_invalid_pairs"));
        }
    }
    pairs.sort_by(|a, b| {
        (&a.pair.source, &a.pair.target, &a.pipeline_profile_id).cmp(&(
            &b.pair.source,
            &b.pair.target,
            &b.pipeline_profile_id,
        ))
    });
    if pairs.windows(2).any(|p| p[0] == p[1]) {
        return Err(fail("offline_duplicate_pair"));
    }
    Ok(pairs)
}
/// A content name, not an authorization proof. Existing immutable rows are not rehashed.
pub fn offline_snapshot_content_hash(
    profile: &RuntimeProfileRef,
    identity: &OfflineSnapshotIdentity,
    pairs: &[OfflineResourcePair],
) -> AppResult<String> {
    profile.validate()?;
    let pairs = sorted_pairs(pairs)?;
    match identity {
        OfflineSnapshotIdentity::FileManifest { manifest_sha256 } => {
            digest(manifest_sha256)?;
            if profile.backend != OfflineBackend::LocalWorker {
                return Err(fail("offline_backend_mismatch"));
            }
        }
        OfflineSnapshotIdentity::AppleObservation {
            os_build,
            bridge_build_sha256,
            api_profile,
            pair,
            ..
        } => {
            text(os_build, 512)?;
            digest(bridge_build_sha256)?;
            text(api_profile, 512)?;
            if profile.backend != OfflineBackend::PlatformNative
                || profile.engine.engine_id != "apple_translation"
                || profile.platform != RuntimePlatform::Macos
                || pairs.len() != 1
                || pairs[0].pair != *pair
            {
                return Err(fail("offline_backend_mismatch"));
            }
        }
    }
    canonical_sorted(&("luna-offline-snapshot-rows-v1", profile, identity, pairs))
}
pub fn offline_route_content_hash(
    profile: &RuntimeProfileRef,
    pair: &LanguagePair,
    policy: &OfflineRoutePolicy,
    route: &[OfflineRouteHop],
) -> AppResult<String> {
    profile.validate()?;
    validate_route(route, pair, policy)?;
    let mut seen = std::collections::BTreeSet::from([pair.source.as_str()]);
    for hop in route {
        if hop.package_relative_key.is_some() {
            return Err(fail("offline_route_package_locator_unstored"));
        }
        if !seen.insert(&hop.pair.target) {
            return Err(fail("offline_route_cycle"));
        }
    }
    let hops = route
        .iter()
        .map(|h| (&h.snapshot_id, &h.pair, &h.pipeline_profile_id))
        .collect::<Vec<_>>();
    canonical_sorted(&(
        "luna-offline-route-rows-v1",
        &profile.engine,
        &profile.backend,
        pair,
        policy,
        hops,
    ))
}
impl OfflineUnknownPreparation {
    pub fn from_resolved(
        metadata: OfflineUnknownMetadata,
        material: Arc<ResolvedOfflineMaterial>,
    ) -> AppResult<Arc<Self>> {
        require_id(&metadata.operation_id)?;
        metadata.frame.validate()?;
        let r = material.reference();
        r.validate()?;
        if !matches!(material.coverage(), RuntimeCoverageReport::Unknown { .. }) {
            return Err(fail("offline_unknown_only"));
        }
        if material.expected().canonical_sha256()? != r.expected_observation_sha256 {
            return Err(fail("offline_expected_mismatch"));
        }
        if let Some(abi) = &metadata.abi_profile_id {
            text(abi, 512)?;
        }
        let pairs = sorted_pairs(&metadata.pairs)?;
        offline_route_content_hash(&r.profile, &r.pair, &metadata.route_policy, &metadata.route)?;
        for hop in &metadata.route {
            if hop.snapshot_id != r.snapshot_id
                || !pairs
                    .iter()
                    .any(|p| p.pair == hop.pair && p.pipeline_profile_id == hop.pipeline_profile_id)
            {
                return Err(fail("offline_route_pair_mismatch"));
            }
        }
        let snapshot = match (&r.backend, &metadata.local, material.inspected_local()) {
            (
                ResourceBackendIdentity::LocalInstallation {
                    environment, root, ..
                },
                Some(local),
                Some(inspected),
            ) => {
                text(&local.opaque_locator, 512)?;
                let path = local
                    .canonical_root
                    .to_str()
                    .ok_or_else(|| fail("offline_invalid_path"))?;
                text(path, 8192)?;
                if !local.canonical_root.is_absolute()
                    || inspected.canonical_root_for_owner_check() != local.canonical_root
                    || environment.machine_fingerprint != root.machine_fingerprint
                    || local.environment.platform != r.profile.platform
                    || local.environment.architecture != r.profile.architecture
                {
                    return Err(fail("offline_material_location_mismatch"));
                }
                validate_environment(&local.environment)?;
                if inspected
                    .route()
                    .is_some_and(|route| route != metadata.route)
                    || inspected
                        .route_policy()
                        .is_some_and(|policy| policy != &metadata.route_policy)
                {
                    return Err(fail("offline_material_route_mismatch"));
                }
                OfflineSnapshotIdentity::FileManifest {
                    manifest_sha256: inspected.manifest_sha256().into(),
                }
            }
            (
                ResourceBackendIdentity::AppleSystem {
                    os_build,
                    bridge_build_sha256,
                    api_profile,
                    strategy,
                },
                None,
                None,
            ) => {
                if metadata.expected_installation_head.is_some() {
                    return Err(fail("offline_apple_installation_forbidden"));
                }
                OfflineSnapshotIdentity::AppleObservation {
                    os_build: os_build.clone(),
                    bridge_build_sha256: bridge_build_sha256.clone(),
                    api_profile: api_profile.clone(),
                    strategy: strategy.clone(),
                    pair: r.pair.clone(),
                    observation_epoch: r.observation.epoch,
                }
            }
            _ => return Err(fail("offline_backend_mismatch")),
        };
        if offline_snapshot_content_hash(&r.profile, &snapshot, &pairs)?
            != r.resource_content_sha256
        {
            return Err(fail("offline_snapshot_content_mismatch"));
        }
        let files = material
            .inspected_local()
            .map(|m| {
                m.files()
                    .iter()
                    .map(|f| (&f.relative_key, f.byte_length, &f.sha256, &f.role))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let local = metadata
            .local
            .as_ref()
            .map(|l| (&l.opaque_locator, l.canonical_root.to_str(), &l.environment));
        let request_sha256 = canonical_hash(&(
            "luna-offline-unknown-preparation-v1",
            &metadata.operation_id,
            &metadata.frame.profile_id,
            metadata.frame.generation,
            &metadata.frame.server_instance_id,
            &metadata.abi_profile_id,
            r,
            local,
            &pairs,
            &metadata.route,
            &metadata.route_policy,
            metadata.expected_installation_head,
            metadata.expected_observation_head,
            files,
        ))?;
        Ok(Arc::new(Self {
            metadata,
            material,
            snapshot,
            request_sha256,
        }))
    }
    pub fn operation_id(&self) -> &str {
        &self.metadata.operation_id
    }
    pub fn request_sha256(&self) -> &str {
        &self.request_sha256
    }
}
pub(super) fn validate_environment(e: &OfflineEnvironmentMetadata) -> AppResult<()> {
    text(&e.os_build, 512)?;
    match e.platform {
        RuntimePlatform::Crossover => {
            text(
                e.bottle_identity
                    .as_deref()
                    .ok_or_else(|| fail("offline_environment_incomplete"))?,
                512,
            )?;
            text(
                e.local_prefix_path
                    .as_deref()
                    .ok_or_else(|| fail("offline_environment_incomplete"))?,
                8192,
            )?;
            text(
                e.wine_build
                    .as_deref()
                    .ok_or_else(|| fail("offline_environment_incomplete"))?,
                512,
            )?;
        }
        _ if e.bottle_identity.is_some()
            || e.local_prefix_path.is_some()
            || e.wine_build.is_some() =>
        {
            return Err(fail("offline_environment_mismatch"));
        }
        _ => {}
    }
    Ok(())
}
#[derive(Debug, Clone)]
pub struct OfflineMaterialLookup {
    pub snapshot_id: String,
    pub route_id: String,
    pub subject_key: String,
    /// Optional inventory lookup has no Task/test authority. O5 must require and recheck a real source.
    pub source: Option<OfflineConfigRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfflineStoredPreparationReceipt {
    pub operation_id: String,
    pub reference: OfflineResourceReference,
    pub state: String,
}
impl OfflineStoredPreparationReceipt {
    pub fn lookup(&self) -> OfflineMaterialLookup {
        OfflineMaterialLookup {
            snapshot_id: self.reference.snapshot_id.clone(),
            route_id: self.reference.route_id.clone(),
            subject_key: self.reference.observation.subject_key.clone(),
            source: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct StoredRows {
    pub profile: RuntimeProfileRef,
    pub abi_profile_id: Option<String>,
    pub snapshot_id: String,
    pub identity: OfflineSnapshotIdentity,
    pub resource_content_sha256: String,
    pub pairs: Vec<OfflineResourcePair>,
    pub route_id: String,
    pub pair: LanguagePair,
    pub route_policy: OfflineRoutePolicy,
    pub route: Vec<OfflineRouteHop>,
    pub route_sha256: String,
    pub observation: ObservationRef,
    pub observation_state: String,
    pub installation: Option<LocalInstallationRef>,
    pub installation_head_revision: Option<NonZeroU32>,
    pub environment: Option<RuntimeEnvironmentRef>,
    pub environment_metadata: Option<OfflineEnvironmentMetadata>,
    pub root: Option<GrantedRootRef>,
    pub root_state: Option<String>,
    pub opaque_locator: Option<String>,
    pub local_native_path: Option<String>,
    pub files: Vec<(String, u64, String, StoredFileRole)>,
    pub source_hash: Option<String>,
}
impl StoredRows {
    pub(super) fn skeleton(&self) -> AppResult<StoredRuntimeMaterialSkeleton> {
        let OfflineSnapshotIdentity::FileManifest { manifest_sha256 } = &self.identity else {
            return Err(fail("offline_local_material_required"));
        };
        Ok(StoredRuntimeMaterialSkeleton {
            snapshot_id: self.snapshot_id.clone(),
            route_id: self.route_id.clone(),
            observation: self.observation.clone(),
            profile: self.profile.clone(),
            installation: self
                .installation
                .clone()
                .ok_or_else(|| fail("offline_installation_missing"))?,
            environment: self
                .environment
                .clone()
                .ok_or_else(|| fail("offline_environment_missing"))?,
            root: self
                .root
                .clone()
                .ok_or_else(|| fail("offline_grant_missing"))?,
            resource_content_sha256: self.resource_content_sha256.clone(),
            pair: self.pair.clone(),
            route: self.route.clone(),
            route_policy: self.route_policy.clone(),
            files: self
                .files
                .iter()
                .map(|(key, length, hash, role)| StoredRuntimeFileRow {
                    relative_key: key.clone(),
                    byte_length: *length,
                    sha256: hash.clone(),
                    role: role.clone(),
                })
                .collect(),
            manifest_sha256: manifest_sha256.clone(),
        })
    }
}

/// Explicit recursive key ordering; independent of serde_json preserve_order.
pub(super) fn canonical_sorted<T: Serialize>(value: &T) -> AppResult<String> {
    fn ordered(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut entries = map.into_iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                let mut result = serde_json::Map::new();
                for (key, value) in entries {
                    result.insert(key, ordered(value));
                }
                serde_json::Value::Object(result)
            }
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(ordered).collect())
            }
            value => value,
        }
    }
    canonical_hash(&ordered(serde_json::to_value(value).map_err(err)?))
}
