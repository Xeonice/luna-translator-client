use super::*;
use adapters::{dispatch, translation::TranslationWireOptions};
use app_core::{
    contracts::*,
    domain::{
        TaskDispatch,
        credentials::CredentialLeaseMetadata,
        model_config::{
            catalog,
            credentials::{
                CredentialMaterial, CredentialMaterialKind, ProviderAuthLease, SignedAppLease,
                decode_vault_credential,
            },
        },
    },
    ports::{ProductRepository, runtime::*},
};
use std::sync::Arc;

/// Installed credential handlers, separate from wire support and local SQL
/// metadata. Google source support requires an actual per-gateway owner too;
/// this registry fact never proves a configured grant or usable credential.
pub fn authentication_implemented(auth_mode: &str) -> bool {
    matches!(
        auth_mode,
        "none" | "api_key" | "baidu_app_secret" | "youdao_app_secret"
            | "google_service_account" | "google_oauth"
    )
}

#[derive(Clone)]
pub struct HttpModelGateway {
    repository: Arc<dyn ProductRepository>,
    vault: Arc<dyn CredentialVault>,
    registry: Arc<registry::ProviderRegistry>,
    google: Option<Arc<super::google_authorization::GoogleAuthorizationService>>,
    google_registration: Option<app_core::ports::google_authorization::GoogleHandlerHandle>,
}
pub(super) struct Prepared {
    pub(super) kind: Option<CredentialMaterialKind>,
    pub(super) options: TranslationWireOptions,
}
pub(super) struct LeasedMaterial {
    pub(super) metadata: CredentialLeaseMetadata,
    material: CredentialMaterial,
}
impl LeasedMaterial {
    pub(super) fn auth(&self) -> AppResult<ProviderAuthLease<'_>> {
        let source = &self.metadata.credential;
        Ok(match &self.material {
            CredentialMaterial::ApiKey { key } => ProviderAuthLease::ApiKey { source, key },
            CredentialMaterial::BaiduAppSecret { app_id, secret } => ProviderAuthLease::AppSecret {
                source,
                material: SignedAppLease::Baidu { app_id, secret },
            },
            CredentialMaterial::YoudaoAppSecret { app_key, secret } => {
                ProviderAuthLease::AppSecret {
                    source,
                    material: SignedAppLease::Youdao { app_key, secret },
                }
            }
            _ => {
                return Err(failure(
                    "authorization_provider_unavailable",
                    "当前授权提供器尚未就绪",
                ));
            }
        })
    }
}
impl HttpModelGateway {
    pub(super) fn product_repository(&self) -> Arc<dyn ProductRepository> {
        self.repository.clone()
    }
    /// Compatibility composition uses the exact built-in registry, never network discovery.
    pub fn new(repository: Arc<dyn ProductRepository>, vault: Arc<dyn CredentialVault>) -> Self {
        Self::with_registry(
            repository,
            vault,
            Arc::new(builtin_registry().expect("reviewed built-in registry must be valid")),
        )
    }
    pub fn with_registry(
        repository: Arc<dyn ProductRepository>,
        vault: Arc<dyn CredentialVault>,
        registry: Arc<registry::ProviderRegistry>,
    ) -> Self {
        Self {
            repository,
            vault,
            registry,
            google: None,
            google_registration: None,
        }
    }
    /// Install the same profile-bound authorization owner used by login and
    /// revoke operations. Presence is separate from mere protocol wire support.
    pub fn with_google_authorization(
        mut self,
        service: Arc<super::google_authorization::GoogleAuthorizationService>,
    ) -> AppResult<Self> {
        let frame = self.repository.google_runtime_frame()?;
        let registration = service.register_handler(frame)?;
        self.google_registration = Some(registration);
        self.google = Some(service);
        Ok(self)
    }
    fn auth_kind(connection: &Connection) -> AppResult<CatalogAuthKind> {
        Ok(match connection.auth_mode.as_str() {
            "none" => CatalogAuthKind::None,
            "api_key" => match connection.protocol.as_str() {
                "anthropic" => CatalogAuthKind::AnthropicApiKey,
                "gemini_generate_content" | "google_translate_v2" => CatalogAuthKind::GoogleApiKey,
                "azure_translator" => CatalogAuthKind::AzureSubscriptionKey,
                "deepl_v2" => CatalogAuthKind::DeeplAuthKey,
                _ => CatalogAuthKind::BearerApiKey,
            },
            "baidu_app_secret" => CatalogAuthKind::BaiduAppSecret,
            "youdao_app_secret" => CatalogAuthKind::YoudaoAppSecret,
            "google_service_account" => CatalogAuthKind::GoogleServiceAccount,
            "google_oauth" | "google_oauth_refresh" => CatalogAuthKind::GoogleOauth,
            _ => {
                return Err(failure(
                    "unsupported_authentication",
                    "认证类型与当前配置合同不兼容",
                ));
            }
        })
    }
    fn preflight(
        &self,
        connection: &Connection,
        plan: Option<&RequestPlan>,
        purpose: Option<CatalogPurpose>,
    ) -> AppResult<Prepared> {
        self.preflight_with_options(connection, plan, purpose, TranslationWireOptions::default())
    }
    pub(super) fn preflight_with_options(
        &self,
        connection: &Connection,
        plan: Option<&RequestPlan>,
        purpose: Option<CatalogPurpose>,
        options: TranslationWireOptions,
    ) -> AppResult<Prepared> {
        let legacy = connection.adapter_binding.is_none();
        if legacy {
            match connection.protocol.as_str() {
                "openai_compatible" | "anthropic" | "azure_translator" => {}
                id if dispatch::installed(id) => {
                    return Err(failure(
                        "adapter_binding_required",
                        "请先保存该协议的明确版本绑定",
                    ));
                }
                _ => return Err(failure("unsupported_protocol", "未安装此配置协议")),
            }
            if !matches!(connection.auth_mode.as_str(), "none" | "api_key") {
                return Err(failure(
                    "unsupported_authentication",
                    "旧连接不能借用新类型的认证材料",
                ));
            }
            if connection.preset_source.is_some()
                || plan.is_some_and(|p| p.adapter_binding.is_some() || p.purpose.is_some())
            {
                return Err(failure(
                    "protocol_mismatch",
                    "旧配置不能借用新版本的协议身份",
                ));
            }
        } else {
            let binding = connection
                .adapter_binding
                .as_ref()
                .expect("checked binding");
            let resolved = self.registry.resolve(binding)?;
            if binding.protocol.protocol_id != connection.protocol
                || !dispatch::installed(&connection.protocol)
            {
                return Err(failure(
                    "protocol_mismatch",
                    "精确协议身份与已安装适配器不一致",
                ));
            }
            if !resolved
                .profile
                .auth_kinds
                .contains(&Self::auth_kind(connection)?)
            {
                return Err(failure(
                    "unsupported_authentication",
                    "认证材料不属于所选兼容规则",
                ));
            }
            if purpose.is_some_and(|p| !resolved.profile.allowed_purposes.contains(&p)) {
                return Err(failure("unsupported_purpose", "所选服务不支持此用途"));
            }
            if let Some(preset) = &connection.preset_source {
                let provenance = self.repository.provider_preset(preset.clone())?;
                if provenance.binding != *binding {
                    return Err(failure(
                        "preset_binding_mismatch",
                        "来源预设与连接精确协议身份不一致",
                    ));
                }
            }
            if let Some(plan) = plan
                && (plan.adapter_binding.as_ref() != Some(binding)
                    || plan.purpose != purpose
                    || purpose.is_none())
                {
                    return Err(failure(
                        "protocol_mismatch",
                        "请求未绑定明确的协议版本及用途",
                    ));
                }
        }
        Self::auth_kind(connection)?;
        // User OAuth owns a grant, never a fabricated credential-pool member.
        let kind = if connection.protocol == "google_translate_v3"
            && connection.auth_mode == "google_oauth"
        {
            None
        } else {
            catalog::credential_kind_for_auth_mode(&connection.auth_mode)?
        };
        let base = connection
            .endpoint
            .as_deref()
            .ok_or_else(|| failure("missing_endpoint", "请填写服务地址"))?;
        endpoint(base, "")?;
        // OAuth/resource routes have separate contracts. Unsupported authorization
        // must fail before reading a service-account key or treating it as a bearer token.
        let google = connection.protocol == "google_translate_v3"
            && matches!(connection.auth_mode.as_str(), "google_service_account" | "google_oauth");
        if google {
            super::google_execution::validate_destination(base)?;
        }
        if connection.protocol == "google_translate_v3" && (!google || self.google.is_none() || self.google_registration.as_ref().is_none_or(|r| !r.is_live()))
            || !google && !authentication_implemented(&connection.auth_mode)
        {
            return Err(failure(
                "authorization_provider_unavailable",
                "Google授权提供器尚未就绪",
            ));
        }
        if google {
            let current = self.repository.google_runtime_frame()?;
            if self.google_registration.as_ref().is_none_or(|registration| registration.frame() != &current) {
                return Err(failure("google_handler_frame_mismatch", "Google处理器与当前配置档或实际存储身份不一致"));
            }
        }
        for key in connection.network_metadata.keys() {
            if !(legacy
                || matches!(key.as_str(), "proxyUrl" | "timeoutMs")
                || connection.protocol == "azure_translator" && key == "region")
            {
                return Err(failure(
                    "unsupported_service_option",
                    "服务选项尚无明确的执行合同，不能静默忽略",
                ));
            }
        }
        if let Some(plan) = plan {
            dispatch::preflight(
                plan,
                connection,
                purpose.unwrap_or(CatalogPurpose::Translation),
                &options,
            )?;
        }
        Ok(Prepared { kind, options })
    }
    pub(super) fn lease(
        &self,
        connection: &Connection,
        reference: Option<&str>,
        kind: Option<CredentialMaterialKind>,
    ) -> AppResult<Option<LeasedMaterial>> {
        self.lease_with_observation(connection, reference, kind, || Ok(()), || Ok(()))
    }
    pub(super) fn lease_with_observation(
        &self,
        connection: &Connection,
        reference: Option<&str>,
        kind: Option<CredentialMaterialKind>,
        before_material: impl FnOnce() -> AppResult<()>,
        material_read: impl FnOnce() -> AppResult<()>,
    ) -> AppResult<Option<LeasedMaterial>> {
        let Some(kind) = kind else {
            if reference.is_some() {
                return Err(failure(
                    "credential_kind_mismatch",
                    "无认证连接不能使用凭据租约",
                ));
            }
            return Ok(None);
        };
        let id =
            reference.ok_or_else(|| failure("missing_credential", "请求未绑定明确的凭据版本"))?;
        let pool = connection
            .credential_pool_id
            .as_ref()
            .ok_or_else(|| failure("missing_credential_pool", "请明确选择已保存的凭据池"))?;
        let revision = connection
            .credential_pool_revision
            .ok_or_else(|| failure("missing_credential_pool", "凭据池未绑定明确版本"))?;
        let metadata =
            self.repository
                .credential_lease_metadata(pool.clone(), revision, id.into(), kind)?;
        before_material()?;
        let bytes = self.vault.get(&metadata.vault_ref)?;
        material_read()?;
        let material = decode_vault_credential(metadata.storage, bytes)?;
        // A revoke, GC or pool change during Vault I/O cannot authorize a send.
        let current =
            self.repository
                .credential_lease_metadata(pool.clone(), revision, id.into(), kind)?;
        if metadata != current || material.kind() != kind {
            return Err(failure(
                "credential_authority_changed",
                "读取凭据期间授权已变化，请重新核对",
            ));
        }
        Ok(Some(LeasedMaterial { metadata, material }))
    }
}
impl ModelGateway for HttpModelGateway {
    fn execute_saved_service_test<'a>(
        &'a self,
        prepared: Arc<app_core::ports::service_testing::PreparedServiceTest>,
        repository: Arc<dyn app_core::ports::service_testing::ServiceTestingRepository>,
        budget: app_core::ports::operation_budget::OperationBudget,
        observation: Arc<app_core::ports::service_test_execution::ServiceTestExecutionObservation>,
    ) -> PortFuture<'a, String> {
        let gateway = self.clone();
        let product_repository = self.repository.clone();
        let google = self.google.clone();
        Box::pin(super::service_testing::execute(
            gateway,
            product_repository,
            google,
            prepared,
            repository,
            budget,
            observation,
        ))
    }
    fn validate_cached_service<'a>(
        &'a self,
        task: &'a app_core::domain::service_dispatch::ServiceTaskDispatch,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let app_core::domain::service_execution::ServiceExecutionTarget::Http { plan } = &task.plan.target else {
                return Err(failure("offline_runtime_required", "离线缓存需要当前本机资源权威核验"));
            };
            let transport = super::typed_http::project(plan)?;
            self.preflight_with_options(&transport.connection, Some(&plan.request), Some(task.plan.identity.purpose), transport.options)?;
            self.repository.validate_service_dispatch(task)?;
            if transport.connection.protocol == "google_translate_v3" {
                let service = self.google.as_deref().ok_or_else(|| failure("authorization_provider_unavailable", "Google授权提供器尚未装配"))?;
                let authorized = super::google_execution::authorize(service, self.repository.as_ref(), task).await?;
                authorized.lease.check_current_provenance()?;
            }
            self.repository.validate_service_dispatch(task)
        })
    }
    fn execute_service<'a>(
        &'a self,
        task: &'a app_core::domain::service_dispatch::ServiceTaskDispatch,
        on_delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            use app_core::domain::service_execution::ServiceExecutionTarget;
            let ServiceExecutionTarget::Http { plan } = &task.plan.target else {
                return Err(failure(
                    "offline_runtime_required",
                    "离线翻译需要独立本机运行器",
                ));
            };
            let identity = &task.plan.identity;
            if identity.unit_id != task.unit.unit_id
                || identity.text_revision != task.unit.text_revision
                || identity.source_hash != task.unit.source_hash
                || identity.session_id != task.unit.session_id
                || identity.game_id != task.unit.game_id
                || plan.request.source_text != task.unit.source_text
                || task.unit.source_hash != app_core::domain::content_hash(&task.unit.source_text)
                || plan.request.purpose != Some(identity.purpose)
            {
                return Err(failure(
                    "service_plan_identity_mismatch",
                    "服务请求与已固定原文身份不一致",
                ));
            }
            let transport = super::typed_http::project(plan)?;
            let prepared = self.preflight_with_options(
                &transport.connection,
                Some(&plan.request),
                Some(identity.purpose),
                transport.options.clone(),
            )?;
            self.repository.validate_service_dispatch(task)?;
            if transport.connection.protocol == "google_translate_v3" {
                let service = self.google.as_deref().ok_or_else(|| failure(
                    "authorization_provider_unavailable", "Google授权提供器尚未装配",
                ))?;
                return super::google_execution::execute(
                    service, self.repository.as_ref(), task, &transport, on_delta,
                ).await;
            }
            let material = self.lease(
                &transport.connection,
                task.credential_vault_ref.as_deref(),
                prepared.kind,
            )?;
            self.repository.validate_service_dispatch(task)?;
            let auth = match material.as_ref() {
                Some(material) => material.auth()?,
                None => ProviderAuthLease::None,
            };
            let output = dispatch::execute_with_headers(
                &plan.request,
                &transport.connection,
                identity.purpose,
                &auth,
                &prepared.options,
                &transport.headers,
                move |text| on_delta(text.to_owned()),
            )
            .await?;
            self.repository.validate_service_dispatch(task)?;
            Ok(output.text)
        })
    }
    fn execute<'a>(
        &'a self,
        task: &'a TaskDispatch,
        plan: &'a RequestPlan,
        on_delta: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            let purpose = catalog::parse_purpose(&task.binding.purpose)?;
            let prepared = self.preflight(&task.connection, Some(plan), Some(purpose))?;
            self.repository
                .validate_task_authority(task.task_id.clone())?;
            let material = self.lease(
                &task.connection,
                task.credential_vault_ref.as_deref(),
                prepared.kind,
            )?;
            self.repository
                .validate_task_authority(task.task_id.clone())?;
            let auth = match material.as_ref() {
                Some(material) => material.auth()?,
                None => ProviderAuthLease::None,
            };
            dispatch::execute(
                plan,
                &task.connection,
                purpose,
                &auth,
                &prepared.options,
                move |text| on_delta(text.to_owned()),
            )
            .await
            .map(|output| output.text)
        })
    }
    fn test_request<'a>(
        &'a self,
        connection: &'a Connection,
        plan: &'a RequestPlan,
        credential_ref: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async move {
            let purpose = plan.purpose.unwrap_or(CatalogPurpose::Translation);
            let prepared = self.preflight(connection, Some(plan), Some(purpose))?;
            if connection.protocol == "google_translate_v3" {
                return Err(failure("google_owned_request_required", "Google测试需要明确保存的来源与本次测试身份"));
            }
            let material = self.lease(connection, credential_ref, prepared.kind)?;
            let auth = match material.as_ref() {
                Some(material) => material.auth()?,
                None => ProviderAuthLease::None,
            };
            dispatch::execute(plan, connection, purpose, &auth, &prepared.options, |_| {})
                .await
                .map(|output| output.text)
        })
    }
    fn discover_models<'a>(
        &'a self,
        connection: &'a Connection,
        credential_ref: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async move {
            let prepared = self.preflight(connection, None, None)?;
            if matches!(
                connection.protocol.as_str(),
                "azure_translator" | "deepl_v2" | "google_translate_v2" | "google_translate_v3" | "baidu_vip" | "youdao_v3"
            ) {
                return Ok(Vec::new());
            }
            if !matches!(
                connection.protocol.as_str(),
                "openai_compatible" | "anthropic" | "openai_responses"
                    | "gemini_generate_content" | "ollama_native" | "lmstudio_native_v1"
            ) {
                return Err(failure(
                    "discovery_unsupported",
                    "所选接口需要手动填写模型ID，不使用其他协议的发现路由",
                ));
            }
            let material = self.lease(connection, credential_ref, prepared.kind)?;
            let key = match material.as_ref() {
                Some(LeasedMaterial {
                    material: CredentialMaterial::ApiKey { key },
                    ..
                }) => Some(key.expose()),
                None => None,
                _ => {
                    return Err(failure(
                        "credential_kind_mismatch",
                        "模型列表需要明确的API凭据材料",
                    ));
                }
            };
            if adapters::model_discovery::installed(&connection.protocol) {
                // The native route is separate from the legacy generic list;
                // registry and actual credential checks above remain mandatory.
                let models = adapters::model_discovery::discover(connection, key).await?;
                // A revoked/changed member during the real GET cannot publish
                // results under the stale material that authorized the request.
                if let Some(leased) = material.as_ref() {
                    let metadata = &leased.metadata;
                    let current = self.repository.credential_lease_metadata(
                        metadata.pool_id.clone(), metadata.pool_revision,
                        metadata.vault_ref.clone(), leased.material.kind(),
                    )?;
                    if &current != metadata {
                        return Err(failure("credential_authority_changed", "获取模型列表期间授权已变化，原选择保持不变"));
                    }
                }
                Ok(models)
            } else {
                super::discover_models(connection, key).await
            }
        })
    }
}
