//! One explicitly prepared saved-service request. Configuration/terminal
//! authority belongs to the original Repository and operation, never this wire.
use super::{
    adapters::dispatch,
    failure,
    gateway::{HttpModelGateway, Prepared},
    google_authorization::{GoogleAuthorizationService, GoogleLeasePersistence},
    typed_http::{self, HttpTransport},
};
use app_core::{
    contracts::AppResult,
    domain::{
        credentials::CredentialLeaseMetadata, model_config::credentials::ProviderAuthLease,
        service_execution::ServiceExecutionTarget,
    },
    ports::{
        ProductRepository,
        google_authorization::*,
        operation_budget::OperationBudget,
        service_test_execution::ServiceTestExecutionObservation,
        service_testing::{
            PreparedServiceTest, PreparedTestAuthentication, PreparedTestCredential,
            PreparedTestReading, ServiceTestingRepository,
        },
    },
};
use std::{future::Future, sync::Arc, time::Duration};

struct CancelOnDrop(Option<OperationBudget>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(budget) = &self.0
            && budget.check().is_ok()
        {
            // An expired operation must remain TimedOut, not become Cancelled.
            budget.cancel();
        }
    }
}
async fn within_budget<T>(budget: &OperationBudget, future:impl Future<Output=AppResult<T>>) -> AppResult<T> {
    super::operation_io::within_budget(budget,future).await
}
async fn blocking<T:Send+'static>(budget:&OperationBudget,observation:&Arc<ServiceTestExecutionObservation>,action:impl FnOnce()->AppResult<T>+Send+'static) -> AppResult<T> {
    budget.check()?;let physical=observation.begin_physical_job()?;
    super::operation_io::blocking(budget,physical,action,||failure("service_test_blocking_failure","测试后台操作未正常完成")).await
}
async fn validate(
    repository: Arc<dyn ServiceTestingRepository>,
    prepared: Arc<PreparedServiceTest>,
    budget: &OperationBudget,
    observation: &Arc<ServiceTestExecutionObservation>,
) -> AppResult<()> {
    blocking(budget, observation, move || {
        repository.validate_prepared_test(&prepared)
    })
    .await
}
fn metadata_matches(metadata: &CredentialLeaseMetadata, frozen: &PreparedTestCredential) -> bool {
    metadata.credential.credential_id == frozen.selection.credential_id
        && metadata.credential.credential_version_id == frozen.selection.vault_ref
        && metadata.vault_ref == frozen.selection.vault_ref
        && metadata.storage == frozen.storage
        && metadata.lifecycle_revision == frozen.lifecycle_revision
        && metadata.pool_id == frozen.selection.pool_id
        && metadata.pool_revision == frozen.selection.pool_revision
        && metadata.member_id == frozen.selection.member_id
}
fn check_identity(prepared: &PreparedServiceTest) -> AppResult<()> {
    let request = &prepared.input.request;
    if prepared.plan.identity.source != prepared.source.source
        || prepared.plan.identity.source != request.source
        || prepared.plan.identity.purpose != request.purpose
        || prepared.plan.identity.output_mode != request.output_mode.unwrap_or_default()
        || prepared.profile_generation != request.expected_profile_generation
    {
        return Err(failure(
            "service_test_identity_mismatch",
            "固定测试来源、用途或配置档不一致",
        ));
    }
    Ok(())
}
pub(super) fn execute(
    gateway: HttpModelGateway,
    product_repository: Arc<dyn ProductRepository>,
    google: Option<Arc<GoogleAuthorizationService>>,
    prepared: Arc<PreparedServiceTest>,
    repository: Arc<dyn ServiceTestingRepository>,
    budget: OperationBudget,
    observation: Arc<ServiceTestExecutionObservation>,
) -> impl Future<Output = AppResult<String>> + Send {
    let mut owner = CancelOnDrop(Some(budget.clone()));
    async move {
        let result = execute_inner(
            gateway,
            product_repository,
            google,
            prepared,
            repository,
            budget,
            observation,
        )
        .await;
        // Any completed outcome, including a provider failure, still needs its
        // original active budget for the Controller's real terminal transaction.
        owner.0 = None;
        result
    }
}
async fn execute_inner(
    gateway: HttpModelGateway,
    product_repository: Arc<dyn ProductRepository>,
    google: Option<Arc<GoogleAuthorizationService>>,
    prepared: Arc<PreparedServiceTest>,
    repository: Arc<dyn ServiceTestingRepository>,
    budget: OperationBudget,
    observation: Arc<ServiceTestExecutionObservation>,
) -> AppResult<String> {
    budget.check()?;
    check_identity(&prepared)?;
    budget.restrict_timeout_from_start(Duration::from_millis(u64::from(
        prepared.plan.policy.execution.timeout_ms,
    )))?;
    if let Some(error) = &prepared.execution_blocker {
        return Err(error.clone());
    }
    let ServiceExecutionTarget::Http { .. } = &prepared.plan.target else {
        return Err(failure(
            "offline_test_authority_unresolved",
            "离线测试需要真实本机资源观察与独立运行器",
        ));
    };
    let checked = prepared.clone();
    let repo = repository.clone();
    let preflight_gateway = gateway.clone();
    let (transport, preflight) = blocking(&budget, &observation, move || {
        repo.validate_prepared_test(&checked)?;
        let ServiceExecutionTarget::Http { plan } = &checked.plan.target else {
            unreachable!("checked HTTP target")
        };
        let transport = typed_http::project(plan)?;
        let preflight = preflight_gateway.preflight_with_options(
            &transport.connection,
            Some(&plan.request),
            Some(checked.plan.identity.purpose),
            transport.options.clone(),
        )?;
        Ok((transport, preflight))
    })
    .await?;
    if transport.connection.protocol == "google_translate_v3" {
        let service = google.ok_or_else(|| {
            failure(
                "authorization_provider_unavailable",
                "Google授权提供器尚未装配",
            )
        })?;
        let runtime = tokio::runtime::Handle::current();
        let worker_budget = budget.clone();
        let worker_observation = observation.clone();
        return blocking(&budget, &observation, move || {
            // Google owner has synchronous Repository/Vault checks. Drive its
            // bounded async future on this physical blocking job, using the
            // existing runtime's I/O driver, not a second runtime or queue.
            runtime.block_on(execute_google(
                service,
                prepared,
                repository,
                worker_budget,
                worker_observation,
                transport,
                preflight,
            ))
        })
        .await;
    }
    let material = match &prepared.authentication {
        PreparedTestAuthentication::None if preflight.kind.is_none() => None,
        PreparedTestAuthentication::PoolMember(frozen) => {
            let frozen = frozen.clone();
            let lease_gateway = gateway.clone();
            let checked = prepared.clone();
            let repo = repository.clone();
            let connection = transport.connection.clone();
            let kind = preflight.kind;
            let material_observation = observation.clone();
            let material_budget = budget.clone();
            Some(
                blocking(&budget, &observation, move || {
                    repo.validate_prepared_test(&checked)?;
                    material_budget.check()?;
                    let kind = kind.ok_or_else(|| {
                        failure("credential_kind_mismatch", "无认证目标不能借用凭据")
                    })?;
                    // Read-only lookup in the exact selected pool/version. It does
                    // not choose a member or reverse-search a locator alone.
                    let metadata = product_repository.credential_lease_metadata(
                        frozen.selection.pool_id.clone(),
                        frozen.selection.pool_revision,
                        frozen.selection.vault_ref.clone(),
                        kind,
                    )?;
                    material_budget.check()?;
                    if !metadata_matches(&metadata, &frozen) {
                        return Err(failure(
                            "credential_authority_changed",
                            "已选材料与当前实际来源不一致",
                        ));
                    }
                    let material = lease_gateway
                        .lease_with_observation(
                            &connection,
                            Some(&frozen.selection.vault_ref),
                            Some(kind),
                            || {
                                material_budget.check()?;
                                repo.validate_prepared_test(&checked)?;
                                material_budget.check()
                            },
                            || {
                                material_observation.mark_credential_used()?;
                                material_budget.check()?;
                                repo.validate_prepared_test(&checked)?;
                                material_budget.check()
                            },
                        )?
                        .ok_or_else(|| failure("missing_credential", "测试材料不可用"))?;
                    if !metadata_matches(&material.metadata, &frozen) {
                        return Err(failure(
                            "credential_authority_changed",
                            "读取材料期间授权已变化",
                        ));
                    }
                    repo.validate_prepared_test(&checked)?;
                    Ok(material)
                })
                .await?,
            )
        }
        _ => {
            return Err(failure(
                "credential_kind_mismatch",
                "已固定认证来源与实际协议不匹配",
            ));
        }
    };
    validate(repository.clone(), prepared.clone(), &budget, &observation).await?;
    let ServiceExecutionTarget::Http { plan } = &prepared.plan.target else {
        unreachable!("checked HTTP target")
    };
    let auth = match material.as_ref() {
        Some(material) => material.auth()?,
        None => ProviderAuthLease::None,
    };
    budget.check()?;
    let output = within_budget(&budget, async {
        budget.check()?;
        observation.mark_transport_started(material.is_some())?;
        dispatch::execute_with_headers(
            &plan.request,
            &transport.connection,
            prepared.plan.identity.purpose,
            &auth,
            &preflight.options,
            &transport.headers,
            |_| {},
        )
        .await
    })
    .await?;
    validate(repository, prepared, &budget, &observation).await?;
    Ok(output.text)
}
async fn execute_google(
    service: Arc<GoogleAuthorizationService>,
    prepared: Arc<PreparedServiceTest>,
    repository: Arc<dyn ServiceTestingRepository>,
    budget: OperationBudget,
    observation: Arc<ServiceTestExecutionObservation>,
    transport: HttpTransport,
    preflight: Prepared,
) -> AppResult<String> {
    budget.check()?;
    repository.validate_prepared_test(&prepared)?;
    budget.check()?;
    let ServiceExecutionTarget::Http { plan } = &prepared.plan.target else {
        unreachable!("checked HTTP target")
    };
    let request = GoogleLeaseRequest {
        target: GoogleLeaseTarget::TestOperation(GoogleTestOperation {
            operation_id: prepared.input.operation_id.clone(),
            input_scope: match prepared.reading {
                PreparedTestReading::Sample => GoogleTestInputScope::Sample,
                PreparedTestReading::Selected { .. } => GoogleTestInputScope::SelectedCurrent,
            },
            input_sha256: prepared.input_sha256.clone(),
            request_semantics_sha256: prepared.request_semantics_sha256.clone(),
        }),
        config_revision_id: match &prepared.source.source {
            app_core::domain::model_config::service_graph::SavedServiceRef::Http { source } => {
                source.revision_id.clone()
            }
            _ => {
                return Err(failure(
                    "google_target_mismatch",
                    "Google测试必须固定HTTP来源",
                ));
            }
        },
        connection_id: plan.connection.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(&plan.connection)?,
        profile_generation: prepared.profile_generation,
        server_instance_id: prepared.server_instance_id.clone(),
    };
    request.validate()?;
    let lease = service
        .lease(
            request.clone(),
            GoogleBudget::from_operation_budget(budget.clone()),
        )
        .await?;
    // Retain the real acquired fact even when cancellation or a source change
    // makes the following check fail. It is not a prewritten active SQL lease.
    observation.record_google_lease(lease.record().clone())?;
    observation.mark_credential_used()?;
    if lease.record().request != request
        || !matches!(
            lease.persistence(),
            GoogleLeasePersistence::DeferredTerminalTest
        )
    {
        return Err(failure(
            "google_test_lease_mismatch",
            "授权租约不属于原测试操作",
        ));
    }
    budget.check()?;
    repository.validate_prepared_test(&prepared)?;
    lease.check_current_provenance()?;
    let auth = lease.authorization()?;
    let output = tokio::select! {
        biased;
        error=lease.invalidated()=>return Err(error),
        result=within_budget(&budget,async {
            budget.check()?;
            observation.mark_transport_started(true)?;
            dispatch::execute_with_headers(&plan.request,&transport.connection,prepared.plan.identity.purpose,&auth,&preflight.options,&transport.headers,|_|{}).await
        })=>result?,
    };
    budget.check()?;
    lease.check_current_provenance()?;
    repository.validate_prepared_test(&prepared)?;
    budget.check()?;
    Ok(output.text)
}
