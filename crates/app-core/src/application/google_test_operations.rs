//! One application-owned live test registry. Google is a narrow projection of
//! the same immutable operation, not a second queue or persisted pending test.
use crate::{
    contracts::{AppError, AppResult},
    domain::{model_config::credentials::*, service_execution::ServiceExecutionTarget},
    ports::{google_authorization::*, operation_budget::OperationBudget, service_testing::*},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const MAX_ACTIVE_TESTS: usize = 16;
fn invalid(code: &str) -> AppError {
    AppError::new(
        code,
        "service_test",
        "测试操作已取消、到期或与固定来源不一致",
    )
}
enum Registration {
    /// Backend-only authorization seam retained for callers which explicitly
    /// test Google authentication. It cannot stand in for a prepared service.
    Google {
        request: Box<GoogleLeaseRequest>,
        source: Option<GoogleServiceAccountLeaseIdentity>,
    },
    Preparing {
        generation: std::num::NonZeroU32,
    },
    Service(Arc<PreparedServiceTest>),
}
struct Entry {
    registration: Registration,
    budget: OperationBudget,
    owner: Arc<()>,
}
#[derive(Default)]
struct Inner {
    entries: Mutex<BTreeMap<String, Entry>>,
}
#[derive(Clone, Default)]
pub struct LiveServiceTestOperations {
    inner: Arc<Inner>,
}
pub type LiveGoogleTestOperations = LiveServiceTestOperations;

pub struct GoogleTestOperationGuard {
    registry: LiveServiceTestOperations,
    request: GoogleLeaseRequest,
    budget: GoogleBudget,
    owner: Arc<()>,
}
pub struct ServiceTestOperationGuard {
    registry: LiveServiceTestOperations,
    prepared: Arc<PreparedServiceTest>,
    budget: OperationBudget,
    owner: Arc<()>,
}
/// A reservation exists before blocking SQL preparation, so cancellation cannot
/// race an unregistered start into later material I/O.
pub struct ServiceTestPreparationGuard {
    registry: LiveServiceTestOperations,
    operation_id: String,
    budget: OperationBudget,
    owner: Arc<()>,
    armed: bool,
}
impl ServiceTestPreparationGuard {
    pub fn bind(mut self, prepared: PreparedServiceTest) -> AppResult<ServiceTestOperationGuard> {
        let prepared = Arc::new(prepared);
        {
            let mut entries = self
                .registry
                .inner
                .entries
                .lock()
                .map_err(|_| invalid("service_test_state"))?;
            let entry = entries
                .get_mut(&self.operation_id)
                .ok_or_else(|| invalid("service_test_operation_inactive"))?;
            if !Arc::ptr_eq(&entry.owner, &self.owner)
                || prepared.input.operation_id != self.operation_id
            {
                return Err(invalid("service_test_operation_mismatch"));
            }
            if !matches!(entry.registration, Registration::Preparing{generation} if generation == prepared.profile_generation)
            {
                return Err(invalid("profile_generation_conflict"));
            }
            entry.registration = Registration::Service(prepared.clone());
        }
        self.armed = false;
        Ok(ServiceTestOperationGuard {
            registry: self.registry.clone(),
            prepared,
            budget: self.budget.clone(),
            owner: self.owner.clone(),
        })
    }
}
impl Drop for ServiceTestPreparationGuard {
    fn drop(&mut self) {
        if self.armed {
            self.budget.cancel();
            self.registry.remove(&self.operation_id, &self.owner);
        }
    }
}
impl std::fmt::Debug for GoogleTestOperationGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleTestOperationGuard([application-owned])")
    }
}
impl std::fmt::Debug for ServiceTestOperationGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServiceTestOperationGuard([application-owned])")
    }
}
impl GoogleTestOperationGuard {
    pub fn request(&self) -> &GoogleLeaseRequest {
        &self.request
    }
    pub fn budget(&self) -> GoogleBudget {
        self.budget.clone()
    }
}
impl ServiceTestOperationGuard {
    pub fn prepared(&self) -> Arc<PreparedServiceTest> {
        self.prepared.clone()
    }
    pub fn budget(&self) -> OperationBudget {
        self.budget.clone()
    }
    pub fn google_request(&self) -> AppResult<GoogleLeaseRequest> {
        google_projection(&self.prepared).map(|p| p.0)
    }
}
impl Drop for GoogleTestOperationGuard {
    fn drop(&mut self) {
        self.budget.cancel();
        if let GoogleLeaseTarget::TestOperation(op) = &self.request.target {
            self.registry.remove(&op.operation_id, &self.owner);
        }
    }
}
impl Drop for ServiceTestOperationGuard {
    fn drop(&mut self) {
        self.budget.cancel();
        self.registry
            .remove(&self.prepared.input.operation_id, &self.owner);
    }
}
fn google_projection(
    prepared: &PreparedServiceTest,
) -> AppResult<(
    GoogleLeaseRequest,
    Option<GoogleServiceAccountLeaseIdentity>,
)> {
    let ServiceExecutionTarget::Http { plan } = &prepared.plan.target else {
        return Err(invalid("google_target_mismatch"));
    };
    let c = &plan.connection;
    if c.protocol != "google_translate_v3" {
        return Err(invalid("google_target_mismatch"));
    }
    let source = match (&prepared.authentication, c.auth_mode.as_str()) {
        (PreparedTestAuthentication::PoolMember(material), "google_service_account") => {
            if material.storage
                != (CredentialStorageMetadata::EnvelopeV1 {
                    kind: CredentialMaterialKind::GoogleServiceAccount,
                })
                || c.credential_pool_id.as_deref() != Some(material.selection.pool_id.as_str())
                || c.credential_pool_revision.map(|v| v.get())
                    != Some(material.selection.pool_revision)
            {
                return Err(invalid("google_test_source_mismatch"));
            }
            Some(GoogleServiceAccountLeaseIdentity {
                pool_id: material.selection.pool_id.clone(),
                pool_revision: std::num::NonZeroU32::new(material.selection.pool_revision)
                    .ok_or_else(|| invalid("google_test_source_mismatch"))?,
                member_id: material.selection.member_id.clone(),
                credential_version: CredentialVersionRef {
                    credential_id: material.selection.credential_id.clone(),
                    credential_version_id: material.selection.vault_ref.clone(),
                },
                lifecycle_revision: material.lifecycle_revision,
                scopes_hash: google_scope_hash(),
            })
        }
        (PreparedTestAuthentication::GoogleOwnedGrant(grant), "google_oauth") => {
            if c.google_authorization.as_ref().is_none_or(|reference| {
                reference.grant_id != grant.cursor.grant_id
                    || Some(reference.authorization_epoch) != grant.cursor.authorization_epoch
            }) {
                return Err(invalid("google_test_source_mismatch"));
            }
            None
        }
        _ => return Err(invalid("google_test_source_mismatch")),
    };
    let input_scope = match prepared.plan.identity.input {
        crate::domain::service_testing::SavedServiceTestInputIdentity::Sample { .. } => {
            GoogleTestInputScope::Sample
        }
        crate::domain::service_testing::SavedServiceTestInputIdentity::SelectedUnit { .. } => {
            GoogleTestInputScope::SelectedCurrent
        }
    };
    let revision = match &prepared.plan.identity.source {
        crate::domain::model_config::service_graph::SavedServiceRef::Http { source } => {
            source.revision_id.clone()
        }
        _ => return Err(invalid("google_target_mismatch")),
    };
    let request = GoogleLeaseRequest {
        target: GoogleLeaseTarget::TestOperation(GoogleTestOperation {
            operation_id: prepared.input.operation_id.clone(),
            input_scope,
            input_sha256: prepared.input_sha256.clone(),
            request_semantics_sha256: prepared.request_semantics_sha256.clone(),
        }),
        config_revision_id: revision,
        connection_id: c.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(c)?,
        profile_generation: prepared.profile_generation,
        server_instance_id: prepared.server_instance_id.clone(),
    };
    request.validate()?;
    Ok((request, source))
}
impl LiveServiceTestOperations {
    pub fn begin_service(
        &self,
        operation_id: String,
        generation: std::num::NonZeroU32,
        budget: OperationBudget,
    ) -> AppResult<ServiceTestPreparationGuard> {
        crate::domain::require_id(&operation_id)?;
        let owner = self.insert(
            operation_id.clone(),
            Registration::Preparing { generation },
            budget.clone(),
        )?;
        Ok(ServiceTestPreparationGuard {
            registry: self.clone(),
            operation_id,
            budget,
            owner,
            armed: true,
        })
    }
    pub fn cancel_service(
        &self,
        operation_id: &str,
        generation: std::num::NonZeroU32,
    ) -> AppResult<()> {
        let budget = {
            let entries = self
                .inner
                .entries
                .lock()
                .map_err(|_| invalid("service_test_state"))?;
            let entry = entries
                .get(operation_id)
                .ok_or_else(|| invalid("service_test_operation_inactive"))?;
            let actual = match &entry.registration {
                Registration::Preparing { generation } => *generation,
                Registration::Service(p) => p.profile_generation,
                _ => return Err(invalid("service_test_operation_mismatch")),
            };
            if actual != generation {
                return Err(invalid("profile_generation_conflict"));
            }
            entry.budget.clone()
        };
        budget.cancel();
        Ok(())
    }
    pub fn cancel_services(&self) -> AppResult<()> {
        let budgets: Vec<_> = self
            .inner
            .entries
            .lock()
            .map_err(|_| invalid("service_test_state"))?
            .values()
            .filter(|entry| !matches!(entry.registration, Registration::Google { .. }))
            .map(|entry| entry.budget.clone())
            .collect();
        for budget in budgets {
            budget.cancel()
        }
        Ok(())
    }

    fn remove(&self, id: &str, owner: &Arc<()>) {
        if let Ok(mut entries) = self.inner.entries.lock()
            && entries
                .get(id)
                .is_some_and(|entry| Arc::ptr_eq(&entry.owner, owner))
        {
            entries.remove(id);
        }
    }
    fn insert(
        &self,
        id: String,
        registration: Registration,
        budget: OperationBudget,
    ) -> AppResult<Arc<()>> {
        budget.check()?;
        let mut entries = self
            .inner
            .entries
            .lock()
            .map_err(|_| invalid("service_test_state"))?;
        if entries.contains_key(&id) {
            return Err(invalid("service_test_operation_conflict"));
        }
        if entries.len() >= MAX_ACTIVE_TESTS {
            return Err(invalid("service_test_capacity"));
        }
        let owner = Arc::new(());
        entries.insert(
            id,
            Entry {
                registration,
                budget,
                owner: owner.clone(),
            },
        );
        Ok(owner)
    }
    pub fn register_service(
        &self,
        prepared: PreparedServiceTest,
        budget: OperationBudget,
    ) -> AppResult<ServiceTestOperationGuard> {
        crate::domain::require_id(&prepared.input.operation_id)?;
        crate::domain::require_id(&prepared.test_id)?;
        let prepared = Arc::new(prepared);
        let owner = self.insert(
            prepared.input.operation_id.clone(),
            Registration::Service(prepared.clone()),
            budget.clone(),
        )?;
        Ok(ServiceTestOperationGuard {
            registry: self.clone(),
            prepared,
            budget,
            owner,
        })
    }
    pub fn register(
        &self,
        request: GoogleLeaseRequest,
        budget: GoogleBudget,
        source: Option<GoogleServiceAccountLeaseIdentity>,
    ) -> AppResult<GoogleTestOperationGuard> {
        request.validate()?;
        budget.check()?;
        let GoogleLeaseTarget::TestOperation(op) = &request.target else {
            return Err(invalid("google_test_operation_required"));
        };
        if let Some(source) = &source {
            source.validate()?;
            if source.scopes_hash != google_scope_hash() {
                return Err(invalid("google_scope_mismatch"));
            }
        }
        let owner = self.insert(
            op.operation_id.clone(),
            Registration::Google {
                request: Box::new(request.clone()),
                source,
            },
            budget.operation_budget(),
        )?;
        Ok(GoogleTestOperationGuard {
            registry: self.clone(),
            request,
            budget,
            owner,
        })
    }
    fn with_google<T>(
        &self,
        request: &GoogleLeaseRequest,
        operation: &GoogleTestOperation,
        read: impl FnOnce(Option<GoogleServiceAccountLeaseIdentity>) -> AppResult<T>,
    ) -> AppResult<T> {
        request.validate()?;
        if request.target != GoogleLeaseTarget::TestOperation(operation.clone()) {
            return Err(invalid("google_test_operation_mismatch"));
        }
        let entries = self
            .inner
            .entries
            .lock()
            .map_err(|_| invalid("service_test_state"))?;
        let entry = entries
            .get(&operation.operation_id)
            .ok_or_else(|| invalid("google_test_operation_inactive"))?;
        let (expected, source) = match &entry.registration {
            Registration::Google { request, source } => (request.as_ref().clone(), source.clone()),
            Registration::Service(prepared) => google_projection(prepared)?,
            Registration::Preparing { .. } => return Err(invalid("service_test_preparing")),
        };
        if expected != *request {
            return Err(invalid("google_test_operation_mismatch"));
        }
        // Preserve the Google port's existing operation-specific error codes.
        GoogleBudget::from_operation_budget(entry.budget.clone()).check()?;
        read(source)
    }
    /// Used only by the application retention/GC protection projection.
    pub fn active_prepared(&self) -> AppResult<Vec<Arc<PreparedServiceTest>>> {
        Ok(self
            .inner
            .entries
            .lock()
            .map_err(|_| invalid("service_test_state"))?
            .values()
            .filter_map(|entry| {
                if let Registration::Service(prepared) = &entry.registration {
                    Some(prepared.clone())
                } else {
                    None
                }
            })
            .collect())
    }
}
impl ServiceTestOperationAuthority for LiveServiceTestOperations {
    fn active_prepared(&self) -> AppResult<Vec<Arc<PreparedServiceTest>>> { LiveServiceTestOperations::active_prepared(self) }
    fn operation_budget(&self, prepared: &Arc<PreparedServiceTest>) -> AppResult<OperationBudget> {
        let entries = self
            .inner
            .entries
            .lock()
            .map_err(|_| invalid("service_test_state"))?;
        let entry = entries
            .get(&prepared.input.operation_id)
            .ok_or_else(|| invalid("service_test_operation_inactive"))?;
        if let Registration::Service(original) = &entry.registration
            && Arc::ptr_eq(original, prepared)
        {
            return Ok(entry.budget.clone());
        }
        Err(invalid("service_test_operation_mismatch"))
    }
}
impl GoogleTestOperationAuthority for LiveServiceTestOperations {
    fn check(
        &self,
        request: &GoogleLeaseRequest,
        operation: &GoogleTestOperation,
    ) -> AppResult<()> {
        self.with_google(request, operation, |_| Ok(()))
    }
    fn service_account_source(
        &self,
        request: &GoogleLeaseRequest,
        operation: &GoogleTestOperation,
    ) -> AppResult<GoogleServiceAccountLeaseIdentity> {
        self.with_google(request, operation, |source| {
            source.ok_or_else(|| invalid("google_test_source_unavailable"))
        })
    }
}
