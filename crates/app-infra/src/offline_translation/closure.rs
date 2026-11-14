//! Read-only sealed material reconstruction. SQL rows are claims until the
//! concrete Repository and Owner revalidate their own actor/grant authority.
use super::{OfflineErrorCode, OfflineFailure, OfflineResult, resources};
use app_core::domain::offline_runtime::{
    CONTROL_RELATIVE_KEY, ControlArtifactRef, MAX_CONTROL_BYTES, MAX_SNAPSHOT_FILES,
    OfflineResourceReference, ResourceBackendIdentity, RuntimeBundleManifest, StoredFileRole,
    decode_control_artifact, snapshot_manifest_sha256, validate_relative_key,
};
use app_core::domain::{
    model_config::service_graph::{
        LanguagePair, LocalInstallationRef, OfflineBackend, OfflineRoutePolicy,
    },
    offline_runtime::{
        GrantedRootRef, ObservationRef, OfflineRouteHop, RuntimeEnvironmentRef, RuntimeProfileRef,
        validate_route,
    },
    require_id,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Exact SQL file-row facts. This type is not a registration or a root capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRuntimeFileRow {
    pub relative_key: String,
    pub byte_length: u64,
    pub sha256: String,
    pub role: StoredFileRole,
}
/// Non-authoritative input from the concrete Repository. The Repository must
/// independently validate the entire snapshot/pairs/seals/heads/grants chain.
#[derive(Debug, Clone)]
pub struct SealedRuntimeMaterialInput {
    pub reference: OfflineResourceReference,
    pub files: Vec<StoredRuntimeFileRow>,
    pub manifest_sha256: String,
}
fn fail(stage: &str) -> OfflineFailure {
    OfflineFailure::new(OfflineErrorCode::InvalidResource, stage)
}
/// First-read SQL facts have no runtime/processing/Expected hashes: those do not
/// exist as columns in 004 and must be derived from actual sealed control bytes.
#[derive(Debug, Clone)]
pub struct StoredRuntimeMaterialSkeleton {
    pub snapshot_id: String,
    pub route_id: String,
    pub observation: ObservationRef,
    pub profile: RuntimeProfileRef,
    pub installation: LocalInstallationRef,
    pub environment: RuntimeEnvironmentRef,
    pub root: GrantedRootRef,
    pub resource_content_sha256: String,
    pub pair: LanguagePair,
    pub route: Vec<OfflineRouteHop>,
    pub route_policy: OfflineRoutePolicy,
    pub files: Vec<StoredRuntimeFileRow>,
    pub manifest_sha256: String,
}
impl SealedRuntimeMaterialInput {
    pub fn validate_shape(&self) -> OfflineResult<()> {
        self.reference
            .validate()
            .map_err(|_| fail("sealed_reference"))?;
        if !matches!(
            self.reference.backend,
            ResourceBackendIdentity::LocalInstallation { .. }
        ) {
            return Err(fail("sealed_material_shape"));
        }
        let control = validate_stored_rows(&self.files, &self.manifest_sha256)?;
        if self.reference.control.as_ref() != Some(&control) {
            return Err(fail("sealed_control_row_mismatch"));
        }
        Ok(())
    }
}
impl StoredRuntimeMaterialSkeleton {
    pub fn validate_shape(&self) -> OfflineResult<()> {
        self.profile
            .validate()
            .map_err(|_| fail("stored_profile"))?;
        if self.profile.backend != OfflineBackend::LocalWorker {
            return Err(fail("stored_backend"));
        }
        for id in [
            &self.snapshot_id,
            &self.route_id,
            &self.observation.observation_id,
            &self.observation.subject_key,
            &self.installation.installation_id,
            &self.environment.environment_id,
            &self.root.grant_id,
        ] {
            require_id(id).map_err(|_| fail("stored_identity"))?;
        }
        if self.observation.subject_key.len() > 512
            || !resources::is_digest(&self.resource_content_sha256)
            || !resources::is_digest(&self.root.machine_fingerprint)
            || self.root.machine_fingerprint != self.environment.machine_fingerprint
        {
            return Err(fail("stored_identity"));
        }
        validate_route(&self.route, &self.pair, &self.route_policy)
            .map_err(|_| fail("stored_route"))?;
        if self
            .route
            .iter()
            .any(|hop| hop.snapshot_id != self.snapshot_id)
        {
            return Err(fail("route_snapshot_material_unavailable"));
        }
        validate_stored_rows(&self.files, &self.manifest_sha256)?;
        Ok(())
    }
}
fn validate_stored_rows(
    files: &[StoredRuntimeFileRow],
    manifest_sha256: &str,
) -> OfflineResult<ControlArtifactRef> {
    if files.is_empty()
        || files.len() > MAX_SNAPSHOT_FILES
        || !resources::is_digest(manifest_sha256)
    {
        return Err(fail("sealed_material_shape"));
    }
    let mut keys = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for file in files {
        validate_relative_key(&file.relative_key).map_err(|_| fail("sealed_file_key"))?;
        if !keys.insert(&file.relative_key)
            || !resources::is_digest(&file.sha256)
            || file.byte_length > i64::MAX as u64
        {
            return Err(fail("sealed_file_row"));
        }
        total = total
            .checked_add(file.byte_length)
            .filter(|n| *n <= resources::MAX_RESOURCE_BYTES)
            .ok_or_else(|| {
                OfflineFailure::new(
                    OfflineErrorCode::ResourceBudgetExceeded,
                    "sealed_total_bytes",
                )
            })?;
    }
    let row = files
        .iter()
        .find(|row| row.relative_key == CONTROL_RELATIVE_KEY)
        .ok_or_else(|| fail("sealed_control_missing"))?;
    if row.role != StoredFileRole::Runtime {
        return Err(fail("sealed_control_row_mismatch"));
    }
    let control = ControlArtifactRef {
        relative_key: row.relative_key.clone(),
        byte_length: row.byte_length,
        sha256: row.sha256.clone(),
    };
    control
        .validate()
        .map_err(|_| fail("sealed_control_row_mismatch"))?;
    Ok(control)
}
/// Privately constructed only after actual bounded I/O. It is still an
/// observation at inspection time, never a proof that a loader is confined.
/// ```compile_fail
/// use app_infra::offline_translation::closure::InspectedRuntimeMaterial;
/// fn export(material: &InspectedRuntimeMaterial) { let _ = serde_json::to_string(material); }
/// ```
pub struct InspectedRuntimeMaterial {
    bundle: RuntimeBundleManifest,
    input: SealedRuntimeMaterialInput,
    actual_total_bytes: u64,
    route: Option<(Vec<OfflineRouteHop>, OfflineRoutePolicy)>,
    canonical_root: PathBuf,
}
impl std::fmt::Debug for InspectedRuntimeMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InspectedRuntimeMaterial")
            .field("file_count", &self.input.files.len())
            .field("runtime_content_sha256", &self.bundle.content_sha256)
            .field("actual_total_bytes", &self.actual_total_bytes)
            .finish_non_exhaustive()
    }
}
impl InspectedRuntimeMaterial {
    /// Actual read location, retained only for the original Owner/checkpoint to
    /// compare. Path equality alone never proves a grant or actor identity.
    pub fn canonical_root_for_owner_check(&self) -> &Path {
        &self.canonical_root
    }
    pub fn bundle(&self) -> &RuntimeBundleManifest {
        &self.bundle
    }
    pub fn reference(&self) -> &OfflineResourceReference {
        &self.input.reference
    }
    pub fn files(&self) -> &[StoredRuntimeFileRow] {
        &self.input.files
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.input.manifest_sha256
    }
    pub fn control(&self) -> &ControlArtifactRef {
        self.input
            .reference
            .control
            .as_ref()
            .expect("checked local control")
    }
    pub fn route(&self) -> Option<&[OfflineRouteHop]> {
        self.route.as_ref().map(|route| route.0.as_slice())
    }
    pub fn route_policy(&self) -> Option<&OfflineRoutePolicy> {
        self.route.as_ref().map(|route| &route.1)
    }
    pub fn actual_total_bytes(&self) -> u64 {
        self.actual_total_bytes
    }
}

pub(super) fn inspect_material_checked(
    input: SealedRuntimeMaterialInput,
    root: &Path,
    limits: resources::InspectionLimits,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<InspectedRuntimeMaterial> {
    check()?;
    input.validate_shape()?;
    let control = input
        .reference
        .control
        .as_ref()
        .ok_or_else(|| fail("sealed_control_missing"))?;
    let bundle = read_bundle_checked(root, control, check)?;
    validate_material_bundle(input, bundle, root, limits, check)
}
fn read_bundle_checked(
    root: &Path,
    control: &ControlArtifactRef,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<RuntimeBundleManifest> {
    let path = resources::relative_path(root, CONTROL_RELATIVE_KEY)?;
    let bytes = resources::read_bounded_checked(&path, MAX_CONTROL_BYTES, check)?;
    let bundle =
        decode_control_artifact(&bytes, control).map_err(|_| fail("sealed_control_bytes"))?;
    check()?;
    Ok(bundle)
}
pub(super) fn inspect_stored_material_checked(
    stored: StoredRuntimeMaterialSkeleton,
    root: &Path,
    limits: resources::InspectionLimits,
    check: &dyn Fn() -> OfflineResult<()>,
    policy: &super::runtime_policy::CompiledRuntimePolicy,
) -> OfflineResult<InspectedRuntimeMaterial> {
    check()?;
    stored.validate_shape()?;
    let control = validate_stored_rows(&stored.files, &stored.manifest_sha256)?;
    let bundle = read_bundle_checked(root, &control, check)?;
    if bundle.profile != stored.profile {
        return Err(fail("stored_bundle_profile_mismatch"));
    }
    let expected = policy.reconstruct_expected(&bundle)?;
    // Hashes are derived here only after actual byte/hash verification and the
    // compiled contract. No SQL placeholder, Ready or caller Expected is used.
    let reference = OfflineResourceReference {
        snapshot_id: stored.snapshot_id,
        route_id: stored.route_id,
        observation: stored.observation,
        profile: stored.profile,
        backend: ResourceBackendIdentity::LocalInstallation {
            installation: stored.installation,
            environment: stored.environment,
            root: stored.root,
            runtime_content_sha256: bundle.content_sha256.clone(),
        },
        resource_content_sha256: stored.resource_content_sha256,
        processing_sha256: expected.processing_sha256.clone(),
        expected_observation_sha256: expected
            .canonical_sha256()
            .map_err(|_| fail("sealed_expected"))?,
        control: Some(control),
        pair: stored.pair,
    };
    let input = SealedRuntimeMaterialInput {
        reference,
        files: stored.files,
        manifest_sha256: stored.manifest_sha256,
    };
    input.validate_shape()?;
    let mut material = validate_material_bundle(input, bundle, root, limits, check)?;
    material.route = Some((stored.route, stored.route_policy));
    Ok(material)
}
fn validate_material_bundle(
    input: SealedRuntimeMaterialInput,
    bundle: RuntimeBundleManifest,
    root: &Path,
    limits: resources::InspectionLimits,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<InspectedRuntimeMaterial> {
    let control = input
        .reference
        .control
        .as_ref()
        .ok_or_else(|| fail("sealed_control_missing"))?;
    let ResourceBackendIdentity::LocalInstallation {
        runtime_content_sha256,
        ..
    } = &input.reference.backend
    else {
        return Err(fail("sealed_backend"));
    };
    if bundle.profile != input.reference.profile
        || &bundle.content_sha256 != runtime_content_sha256
        || bundle.expected.processing_sha256 != input.reference.processing_sha256
        || bundle
            .expected
            .canonical_sha256()
            .map_err(|_| fail("sealed_expected"))?
            != input.reference.expected_observation_sha256
    {
        return Err(fail("sealed_bundle_reference_mismatch"));
    }
    let mut declared = BTreeMap::new();
    for file in &bundle.payload_files {
        declared.insert(
            file.relative_key.as_str(),
            (
                file.byte_length,
                file.sha256.as_str(),
                file.role.stored_role(),
            ),
        );
    }
    declared.insert(
        CONTROL_RELATIVE_KEY,
        (
            control.byte_length,
            control.sha256.as_str(),
            StoredFileRole::Runtime,
        ),
    );
    let stored = input
        .files
        .iter()
        .map(|row| {
            (
                row.relative_key.as_str(),
                (row.byte_length, row.sha256.as_str(), row.role.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if declared != stored {
        return Err(fail("sealed_payload_set_mismatch"));
    }
    if snapshot_manifest_sha256(&bundle, control).map_err(|_| fail("sealed_manifest"))?
        != input.manifest_sha256
    {
        return Err(fail("sealed_manifest_mismatch"));
    }
    // All actual files are charged, including the control artifact. Metadata is
    // not used to skip an undeclared code/model/native file beneath this root.
    let actual = resources::inspect_runtime_root_checked(root, limits, check)?;
    let actual_rows = actual
        .iter()
        .map(|row| {
            (
                row.relative_path.as_str(),
                (row.length, row.sha256.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected_rows = input
        .files
        .iter()
        .map(|row| {
            (
                row.relative_key.as_str(),
                (row.byte_length, row.sha256.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if actual_rows != expected_rows {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceChanged,
            "actual_file_set_mismatch",
        ));
    }
    let actual_total_bytes = actual
        .iter()
        .try_fold(0u64, |sum, row| sum.checked_add(row.length))
        .ok_or_else(|| fail("actual_total_bytes"))?;
    check()?;
    Ok(InspectedRuntimeMaterial {
        bundle,
        input,
        actual_total_bytes,
        route: None,
        canonical_root: root.to_path_buf(),
    })
}
