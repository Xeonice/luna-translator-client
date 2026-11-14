//! One source-qualified model-list path, using the actual Gateway and original operation.
use super::{
    adapters::model_discovery,
    failure,
    gateway::{HttpModelGateway, LeasedMaterial},
    operation_io, typed_http,
};
use app_core::{
    contracts::*,
    domain::model_config::credentials::ProviderAuthLease,
    ports::{
        operation_budget::OperationBudget, physical_jobs::PhysicalJobs, runtime::PortFuture,
        service_discovery::*,
    },
};
use std::{sync::Arc, time::Duration};

pub struct HttpSavedServiceModelDiscovery {
    gateway: Arc<HttpModelGateway>,
    repository: Arc<dyn ServiceModelDiscoveryRepository>,
}
impl HttpSavedServiceModelDiscovery {
    pub fn new(
        gateway: Arc<HttpModelGateway>,
        repository: Arc<dyn ServiceModelDiscoveryRepository>,
    ) -> Self {
        Self {
            gateway,
            repository,
        }
    }
    async fn validate(
        &self,
        p: &Arc<PreparedServiceModelDiscovery>,
        jobs: &Arc<PhysicalJobs>,
        phase: ServiceDiscoveryValidationPhase,
    ) -> AppResult<()> {
        let repo = self.repository.clone();
        let gateway = self.gateway.product_repository();
        let p = p.clone();
        let budget = p.operation.budget();
        blocking(&budget, jobs, move || {
            repo.validate_gateway_owner(&p, &gateway)?;
            repo.validate(&p, phase)
        })
        .await
    }
}
async fn blocking<T: Send + 'static>(
    budget: &OperationBudget,
    jobs: &Arc<PhysicalJobs>,
    action: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    budget.check()?;
    let physical = jobs.try_begin()?;
    operation_io::blocking(budget, physical, action, || {
        failure(
            "discovery_blocking_failure",
            "获取模型列表的后台操作未正常结束",
        )
    })
    .await
}
fn network_preflight(
    c: &Connection,
    headers: &reqwest::header::HeaderMap,
    budget: &OperationBudget,
) -> AppResult<()> {
    if let Some(value) = c.network_metadata.get("timeoutMs") {
        let timeout = value
            .as_u64()
            .filter(|v| (1..=300_000).contains(v))
            .ok_or_else(|| failure("invalid_timeout", "保存的连接超时设置无效"))?;
        budget.restrict_timeout_from_start(Duration::from_millis(timeout))?;
    }
    if c.proxy_mode == "override" {
        let raw = c
            .network_metadata
            .get("proxyUrl")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| failure("invalid_proxy", "保存的代理地址无效"))?;
        let proxy =
            reqwest::Url::parse(raw).map_err(|_| failure("invalid_proxy", "保存的代理地址无效"))?;
        if !proxy.username().is_empty()
            || proxy.password().is_some()
            || proxy.query().is_some()
            || proxy.fragment().is_some()
        {
            return Err(failure(
                "secret_in_proxy",
                "代理地址不能包含凭据、查询参数或片段",
            ));
        }
    }
    // Build only: no DNS/HTTP request or credential access. Preserve the existing
    // client policy and fail malformed transport options before opening Vault.
    let _ = super::client_with_headers(c, 15_000, headers)?;
    Ok(())
}
fn same_material(
    p: &PreparedServiceModelDiscovery,
    material: &Option<LeasedMaterial>,
) -> AppResult<()> {
    match (&p.authentication, material) {
        (ServiceDiscoveryAuthentication::None, None) => Ok(()),
        (ServiceDiscoveryAuthentication::PoolMember { metadata, .. }, Some(leased))
            if **metadata == leased.metadata =>
        {
            Ok(())
        }
        _ => Err(failure(
            "discovery_credential_changed",
            "固定材料与实际租用材料不一致",
        )),
    }
}
impl ServiceModelDiscoveryBackend for HttpSavedServiceModelDiscovery {
    fn prepare(
        &self,
        op: Arc<ServiceModelDiscoveryOperation>,
        jobs: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, Arc<PreparedServiceModelDiscovery>> {
        Box::pin(async move {
            let budget = op.budget();
            let repo = self.repository.clone();
            let prepared = blocking(&budget, &jobs, move || repo.prepare(&op)).await?;
            self.validate(
                &prepared,
                &jobs,
                ServiceDiscoveryValidationPhase::BeforeMaterial,
            )
            .await?;
            Ok(prepared)
        })
    }
    fn discover(
        &self,
        p: Arc<PreparedServiceModelDiscovery>,
        jobs: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, Vec<DiscoveredServiceModel>> {
        Box::pin(async move {
            let budget = p.operation.budget();
            let gateway = self.gateway.clone();
            let repo = self.repository.clone();
            let prepared = p.clone();
            let controlled = budget.clone();
            let (transport, material) = blocking(&budget, &jobs, move || {
                repo.validate_gateway_owner(&prepared, &gateway.product_repository())?;
                repo.validate(&prepared, ServiceDiscoveryValidationPhase::BeforeMaterial)?;
                let transport = typed_http::project_connection(
                    &prepared.connection,
                    &prepared.http_options,
                    &prepared.origin,
                )?;
                let dispatch = gateway.preflight_with_options(
                    &transport.connection,
                    None,
                    Some(prepared.operation.input().request.purpose),
                    transport.options.clone(),
                )?;
                network_preflight(&transport.connection, &transport.headers, &controlled)?;
                let reference = match &prepared.authentication {
                    ServiceDiscoveryAuthentication::None => None,
                    ServiceDiscoveryAuthentication::PoolMember { metadata, .. } => {
                        Some(metadata.vault_ref.as_str())
                    }
                };
                let material = gateway.lease_with_observation(
                    &transport.connection,
                    reference,
                    dispatch.kind,
                    || {
                        controlled.check()?;
                        repo.validate(&prepared, ServiceDiscoveryValidationPhase::BeforeMaterial)
                    },
                    || {
                        controlled.check()?;
                        repo.validate(&prepared, ServiceDiscoveryValidationPhase::AfterMaterial)
                    },
                )?;
                same_material(&prepared, &material)?;
                Ok((transport, material))
            })
            .await?;
            self.validate(&p, &jobs, ServiceDiscoveryValidationPhase::AfterMaterial)
                .await?;
            self.validate(&p, &jobs, ServiceDiscoveryValidationPhase::BeforeTransport)
                .await?;
            let auth = match &material {
                Some(leased) => leased.auth()?,
                None => ProviderAuthLease::None,
            };
            let key = match auth {
                ProviderAuthLease::None => None,
                ProviderAuthLease::ApiKey { key, .. } => Some(key.expose()),
                _ => {
                    return Err(failure(
                        "credential_kind_mismatch",
                        "模型列表只使用已核验的API凭据",
                    ));
                }
            };
            let ids = operation_io::within_budget(&budget, async {
                if model_discovery::installed(&transport.connection.protocol) {
                    model_discovery::discover_with_headers(
                        &transport.connection,
                        key,
                        &transport.headers,
                    )
                    .await
                } else {
                    super::discover_models_with_options(
                        &transport.connection,
                        key,
                        &transport.headers,
                        true,
                    )
                    .await
                }
            })
            .await?;
            self.validate(&p, &jobs, ServiceDiscoveryValidationPhase::BeforeTransport)
                .await?;
            Ok(ids
                .into_iter()
                .map(|id| DiscoveredServiceModel { id })
                .collect())
        })
    }
    fn complete(
        &self,
        p: Arc<PreparedServiceModelDiscovery>,
        outcome: ServiceModelDiscoveryOutcome,
        jobs: Arc<PhysicalJobs>,
    ) -> PortFuture<'_, ServiceModelDiscoveryResult> {
        Box::pin(async move {
            self.validate(&p, &jobs, ServiceDiscoveryValidationPhase::BeforeTransport)
                .await?;
            let physical = jobs.try_begin()?;
            let repo = self.repository.clone();
            operation_io::terminal_blocking(
                physical,
                move || repo.complete(&p, outcome),
                || {
                    failure(
                        "discovery_terminal_failure",
                        "获取模型列表的终态操作未正常结束",
                    )
                },
            )
            .await
        })
    }
}
