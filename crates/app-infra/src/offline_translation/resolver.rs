//! Material resolver used outside the SQLite actor. It reconstructs declarations
//! after physical reads; it owns no grant/handler/Task/test execution authority.
use super::{
    OfflineResult,
    closure::{
        InspectedRuntimeMaterial, SealedRuntimeMaterialInput, StoredRuntimeMaterialSkeleton,
        inspect_material_checked, inspect_stored_material_checked,
    },
    inspection::operation_failure,
    resources::ResourceInspector,
    runtime_policy::policy_for_profile,
};
use app_core::{
    domain::offline_runtime::{
        ExpectedRuntimeObservation, OfflineResourceReference, RuntimeCoverageReport,
    },
    ports::operation_budget::OperationBudget,
};
use std::path::PathBuf;

#[derive(Debug)]
enum Material {
    Local(Box<InspectedRuntimeMaterial>),
    Apple(Box<OfflineResourceReference>),
}
/// No public constructor, Serde implementation or conversion from metadata.
/// Inspection success remains independent of actual grant and loader admission.
/// ```compile_fail
/// use app_infra::offline_translation::resolver::ResolvedOfflineMaterial;
/// fn export(material: &ResolvedOfflineMaterial) { let _ = serde_json::to_string(material); }
/// ```
#[derive(Debug)]
pub struct ResolvedOfflineMaterial {
    material: Material,
    expected: ExpectedRuntimeObservation,
    coverage: RuntimeCoverageReport,
}
impl ResolvedOfflineMaterial {
    pub fn inspected_local(&self) -> Option<&InspectedRuntimeMaterial> {
        match &self.material {
            Material::Local(material) => Some(material),
            Material::Apple(_) => None,
        }
    }
    pub fn reference(&self) -> &OfflineResourceReference {
        match &self.material {
            Material::Local(material) => material.reference(),
            Material::Apple(reference) => reference,
        }
    }
    pub fn expected(&self) -> &ExpectedRuntimeObservation {
        &self.expected
    }
    pub fn coverage(&self) -> &RuntimeCoverageReport {
        &self.coverage
    }
}
/// Share clones of the same ResourceInspector at composition; constructing this
/// resolver does not register a handler or manufacture a local grant Owner.
#[derive(Clone)]
pub struct OfflineRuntimeMaterialResolver {
    inspector: ResourceInspector,
}
impl OfflineRuntimeMaterialResolver {
    pub fn new(inspector: ResourceInspector) -> Self {
        Self { inspector }
    }
    pub async fn inspect_stored(
        &self,
        stored: StoredRuntimeMaterialSkeleton,
        canonical_root: PathBuf,
        budget: OperationBudget,
    ) -> OfflineResult<ResolvedOfflineMaterial> {
        budget.check().map_err(operation_failure)?;
        let policy = policy_for_profile(&stored.profile)?;
        let limits = self.inspector.limits();
        self.inspector
            .run_with_budget(budget, move |check| {
                let material = inspect_stored_material_checked(
                    stored,
                    &canonical_root,
                    limits,
                    check,
                    &policy,
                )?;
                let expected = policy.reconstruct_expected(material.bundle())?;
                check()?;
                Ok(ResolvedOfflineMaterial {
                    material: Material::Local(Box::new(material)),
                    expected,
                    coverage: policy.coverage(),
                })
            })
            .await
    }
    pub async fn inspect_local(
        &self,
        input: SealedRuntimeMaterialInput,
        canonical_root: PathBuf,
        budget: OperationBudget,
    ) -> OfflineResult<ResolvedOfflineMaterial> {
        budget.check().map_err(operation_failure)?;
        let policy = policy_for_profile(&input.reference.profile)?;
        let limits = self.inspector.limits();
        self.inspector
            .run_with_budget(budget, move |check| {
                let material = inspect_material_checked(input, &canonical_root, limits, check)?;
                check()?;
                let expected = policy.reconstruct_expected(material.bundle())?;
                check()?;
                Ok(ResolvedOfflineMaterial {
                    material: Material::Local(Box::new(material)),
                    expected,
                    coverage: policy.coverage(),
                })
            })
            .await
    }
    /// Repeats actual control/root reads. An earlier successful result cannot
    /// replace the new physical inspection or the Repository's own validation.
    pub async fn revalidate_local(
        &self,
        input: SealedRuntimeMaterialInput,
        canonical_root: PathBuf,
        budget: OperationBudget,
    ) -> OfflineResult<ResolvedOfflineMaterial> {
        self.inspect_local(input, canonical_root, budget).await
    }
    /// Pure expected reconstruction only: no OS availability probe or language
    /// package installation is performed, and no SystemApiBound proof is minted.
    pub fn reconstruct_apple(
        &self,
        reference: OfflineResourceReference,
        budget: &OperationBudget,
    ) -> OfflineResult<ResolvedOfflineMaterial> {
        budget.check().map_err(operation_failure)?;
        let policy = policy_for_profile(&reference.profile)?;
        let expected = policy.reconstruct_apple_expected(&reference)?;
        budget.check().map_err(operation_failure)?;
        Ok(ResolvedOfflineMaterial {
            material: Material::Apple(Box::new(reference)),
            expected,
            coverage: policy.coverage(),
        })
    }
}
