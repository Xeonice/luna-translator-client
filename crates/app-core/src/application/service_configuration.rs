//! Application ordering over a single shared directory and transactional store.
//! Preparation facts are supplied only by Rust resource owners, never by IPC.
use crate::{
    contracts::*,
    domain::model_config::service_draft::{
        PreparedServiceDraftResources, compile_service_draft, service_draft_request_hash,
    },
    ports::{IdentitySource, service_configuration::*},
};
use std::sync::Arc;

pub struct ServiceConfigurationController {
    repository: Arc<dyn ServiceConfigurationRepository>,
    directory: Arc<dyn ServiceDirectory>,
    clock: Arc<dyn IdentitySource>,
}
impl ServiceConfigurationController {
    pub fn new(
        repository: Arc<dyn ServiceConfigurationRepository>,
        directory: Arc<dyn ServiceDirectory>,
        clock: Arc<dyn IdentitySource>,
    ) -> Self {
        Self { repository, directory, clock }
    }
    pub fn catalog(&self) -> ServiceCatalog {
        self.directory.catalog()
    }
    pub fn form(&self, input: &ServiceFormRequest) -> AppResult<ServiceFormProjection> {
        self.directory.form(input)
    }
    pub fn configuration(&self) -> AppResult<ServiceConfigurationSnapshot> {
        self.repository.service_configuration()
    }
    fn editor_documents(&self, query: &ServiceDraftQuery) -> AppResult<(ConfigurationDocument, ServiceConfigurationSnapshot)> {
        let current = self.repository.service_configuration()?;
        if current.profile_generation != query.expected_profile_generation {
            return Err(AppError::new("profile_generation_conflict", "service_editor", "配置档已重新打开，请重新读取服务配置"));
        }
        let library = self.repository.saved_services(&SavedServiceQuery {
            expected_profile_generation: query.expected_profile_generation,
            scope: ServiceScope::Global {}, purpose: query.purpose,
        })?;
        if !library.rows.iter().any(|row| row.source == query.source && row.purpose == query.purpose) {
            return Err(AppError::new("saved_service_not_found", "service_editor", "所选来源不是当前配置档中已保存的用户服务"));
        }
        let revision = match &query.source {
            crate::domain::model_config::service_graph::SavedServiceRef::Http { source } => &source.revision_id,
            crate::domain::model_config::service_graph::SavedServiceRef::Offline { source } => &source.revision_id,
        };
        Ok((self.repository.service_configuration_revision(revision)?, current))
    }
    pub fn save(
        &self,
        input: &SaveServiceDraftInput,
        prepared: &PreparedServiceDraftResources,
    ) -> AppResult<ServiceDraftReceipt> {
        crate::domain::require_id(&input.operation_id)?;
        if let Some(receipt) = self.replay(input, prepared)? {
            return Ok(receipt);
        }
        let state = self.repository.service_configuration()?;
        if state.profile_generation != input.expected_profile_generation {
            return Err(AppError::new("profile_generation_conflict", "service_configuration", "配置档已重新打开，请重新读取服务配置"));
        }
        if state.draft_head_revision != input.expected_draft_head_revision {
            return Err(AppError::new("stale_draft_head", "service_configuration", "服务草稿已改变，当前编辑已保留，请核对后重新保存"));
        }
        let command = compile_service_draft(
            &state.draft, input, self.directory.as_ref(), self.clock.as_ref(), prepared,
        )?;
        self.repository.save_service_configuration(&command, self.clock.now_ms())
    }
    fn replay(&self, input: &SaveServiceDraftInput, prepared: &PreparedServiceDraftResources) -> AppResult<Option<ServiceDraftReceipt>> {
        let hash = service_draft_request_hash(input, prepared)?;
        self.repository.service_draft_replay(&input.operation_id, &hash, input.expected_profile_generation)
    }
    pub fn saved_services(&self, query: &SavedServiceQuery) -> AppResult<SavedServiceProjection> {
        self.repository.saved_services(query)
    }
    pub fn select(&self, input: &SelectSavedServicesInput) -> AppResult<ServiceSelectionReceipt> {
        self.repository.select_saved_services(input, self.clock.now_ms())
    }
    pub fn restore_inheritance(&self, input: &RestoreServiceInheritanceInput) -> AppResult<ServiceSelectionReceipt> {
        self.repository.restore_service_inheritance(input, self.clock.now_ms())
    }
    pub fn set_purpose_enabled(&self, input: &SetServicePurposeEnabledInput) -> AppResult<ServiceSelectionReceipt> {
        self.repository.set_service_purpose_enabled(input, self.clock.now_ms())
    }
}

impl super::runtime::ApplicationRuntime {
    pub fn configure_service_configuration(
        &self,
        controller: Arc<ServiceConfigurationController>,
    ) -> AppResult<()> {
        let mut slot = self.service_configuration.lock().map_err(|_| service_state_error())?;
        if slot.is_some() {
            return Err(AppError::new("service_directory_already_configured", "application", "服务目录已装配，不能在现有配置档上隐式替换"));
        }
        *slot = Some(controller);
        Ok(())
    }
    fn service_controller(&self) -> AppResult<Arc<ServiceConfigurationController>> {
        self.service_configuration.lock().map_err(|_| service_state_error())?
            .clone().ok_or_else(|| AppError::new("service_configuration_unavailable", "application", "服务配置尚未装配"))
    }
    pub fn service_configuration(&self) -> AppResult<ServiceConfigurationSnapshot> {
        self.service_controller()?.configuration()
    }
    pub fn ensure_service_profile_generation(&self, expected: std::num::NonZeroU32) -> AppResult<()> {
        if self.repository.snapshot()?.profile_generation != expected.get() {
            return Err(AppError::new("profile_generation_conflict", "service_configuration", "配置档已重新打开，请重新读取服务配置"));
        }
        Ok(())
    }
    pub fn saved_services(&self, query: SavedServiceQuery) -> AppResult<SavedServiceProjection> {
        self.service_controller()?.saved_services(&query)
    }
    pub fn service_draft(&self, query: ServiceDraftQuery) -> AppResult<ServiceDraftEditorProjection> {
        use crate::domain::model_config::{
            credentials::*, service_draft::{materialize_base, PreparedCredentialDraft},
            service_editor::project_service_editor, service_graph::SavedServiceRef,
        };
        let controller = self.service_controller()?;
        let (source, current) = controller.editor_documents(&query)?;
        let graph = materialize_base(&source)?;
        let mut facts = PreparedServiceDraftResources::default();
        if let SavedServiceRef::Http { source: reference } = &query.source
            && let Some(model) = graph.models.iter().find(|model| model.model_id == reference.model_id)
            && let Some(connection) = graph.connections.iter().find(|connection| connection.connection_id == model.connection_id)
            && let Some(authorization) = &connection.google_authorization
        {
            facts.google_client = Some(controller.repository.google_client_draft(authorization, query.expected_profile_generation)?);
        }
        if let SavedServiceRef::Http { source: reference } = &query.source
            && let Some(model) = graph.models.iter().find(|model| model.model_id == reference.model_id)
            && let Some(connection) = graph.connections.iter().find(|connection| connection.connection_id == model.connection_id)
            && let (Some(pool_id), Some(pool_revision)) = (&connection.credential_pool_id, connection.credential_pool_revision)
        {
            let snapshot = self.repository.snapshot()?;
            if let Some(pool) = snapshot.credential_pools.iter().find(|pool| &pool.pool_id == pool_id && pool.revision == pool_revision.get()) {
                let mut identifiers = std::collections::BTreeMap::new();
                let mut identifiers_known = true;
                if let Some(kind @ (CredentialMaterialKind::BaiduAppSecret | CredentialMaterialKind::YoudaoAppSecret)) = crate::domain::model_config::catalog::credential_kind_for_auth_mode(&connection.auth_mode)? {
                    // Only the explicitly opened settings editor requests public
                    // account identifiers. Keys never enter this projection.
                    match self.repository.credential_pool_materials(pool_id.clone(), pool_revision.get(), kind) {
                        Ok(leases) => {
                            let mut observed = None;
                            for lease in leases {
                                let bytes = self.vault.get(&lease.vault_ref)?;
                                let actual = match decode_vault_credential(lease.storage, bytes)? {
                                    CredentialMaterial::BaiduAppSecret { app_id, .. } => std::collections::BTreeMap::from([("app_id".into(), app_id)]),
                                    CredentialMaterial::YoudaoAppSecret { app_key, .. } => std::collections::BTreeMap::from([("app_key".into(), app_key)]),
                                    _ => return Err(AppError::new("credential_kind_mismatch", "credentials", "账号材料类型不匹配")),
                                };
                                if observed.as_ref().is_some_and(|old| old != &actual) { identifiers_known = false; break; }
                                let fresh = self.repository.credential_lease_metadata(pool_id.clone(), pool_revision.get(), lease.vault_ref.clone(), kind)?;
                                if fresh != lease { return Err(AppError::new("credential_authorization_changed", "credentials", "凭据授权已改变，请重新核对")); }
                                observed = Some(actual);
                            }
                            if let Some(observed) = observed { identifiers = observed; } else { identifiers_known = false; }
                        }
                        Err(_) => identifiers_known = false,
                    }
                }
                if identifiers_known {
                    facts.credential = Some(PreparedCredentialDraft { pool_id: pool_id.clone(), pool_revision, multiple_keys: pool.members.len() > 1, public_identifiers: identifiers });
                }
            }
        }
        project_service_editor(&query, &source, &current, controller.directory.as_ref(), &facts)
    }
    /// Only trusted Rust preparation owners can call this seam; IPC receives the
    /// ordinary SaveServiceDraftInput and must not deserialize preparation facts.
    pub fn save_prepared_service_draft(
        &self,
        input: SaveServiceDraftInput,
        prepared: PreparedServiceDraftResources,
    ) -> AppResult<ServiceDraftReceipt> {
        let receipt = self.service_controller()?.save(&input, &prepared)?;
        self.publish()?;
        Ok(receipt)
    }
    pub fn save_service_draft(&self, input: SaveServiceDraftInput) -> AppResult<ServiceDraftReceipt> {
        use crate::domain::model_config::{credentials::*, service_draft::PreparedCredentialDraft};
        self.ensure_service_profile_generation(input.expected_profile_generation)?;
        let controller = self.service_controller()?;
        // Projection validates protected/foreign input before any possible Vault read.
        let projection = controller.form(&ServiceFormRequest { target: input.target.clone(), purpose: input.purpose, values: input.values.clone() })?;
        let mut prepared = PreparedServiceDraftResources::default();
        if let ServiceAuthenticationSelection::GoogleOwnedGrant { authorization } = &input.authentication {
            prepared.google_client = Some(controller.repository.google_client_draft(authorization, input.expected_profile_generation)?);
        }
        if let ServiceAuthenticationSelection::ExistingPool { pool_id, pool_revision } = &input.authentication
            && ["key_pool", "app_id", "app_key"].iter().any(|id| input.values.contains_key(*id))
        {
            let snapshot = self.repository.snapshot()?;
            let pool = snapshot.credential_pools.iter().find(|pool| &pool.pool_id == pool_id && pool.revision == pool_revision.get())
                .ok_or_else(|| AppError::new("credential_pool_unavailable", "credentials", "所选凭据池版本不存在"))?;
            let identifiers = ["app_id", "app_key"].into_iter().filter_map(|id| match input.values.get(id) {
                Some(CatalogScalar::Text(value)) => Some((id.to_owned(), value.clone())), _ => None,
            }).collect();
            prepared.credential = Some(PreparedCredentialDraft {
                pool_id: pool_id.clone(), pool_revision: *pool_revision,
                multiple_keys: pool.members.len() > 1, public_identifiers: identifiers,
            });
        }
        // Candidate values here serve only as a stable replay key. They cannot
        // reach save until the actual secret owner below independently proves them.
        if let Some(receipt) = controller.replay(&input, &prepared)? { return Ok(receipt); }
        if let Some(credential) = prepared.credential.as_ref() && !credential.public_identifiers.is_empty() {
            let kind = match projection.credential_capture {
                ServiceCredentialCapture::BaiduAppSecret => CredentialMaterialKind::BaiduAppSecret,
                ServiceCredentialCapture::YoudaoAppSecret => CredentialMaterialKind::YoudaoAppSecret,
                _ => return Err(AppError::new("credential_preparation_mismatch", "credentials", "公开账号标识与当前认证类型不一致")),
            };
            let leases = self.repository.credential_pool_materials(credential.pool_id.clone(), credential.pool_revision.get(), kind)?;
            for lease in leases {
                let bytes = self.vault.get(&lease.vault_ref)?;
                let actual = match decode_vault_credential(lease.storage, bytes)? {
                    CredentialMaterial::BaiduAppSecret { app_id, .. } => std::collections::BTreeMap::from([("app_id".to_owned(), app_id)]),
                    CredentialMaterial::YoudaoAppSecret { app_key, .. } => std::collections::BTreeMap::from([("app_key".to_owned(), app_key)]),
                    _ => return Err(AppError::new("credential_kind_mismatch", "credentials", "凭据材料类型不匹配")),
                };
                if actual != credential.public_identifiers {
                    return Err(AppError::new("credential_preparation_mismatch", "credentials", "账号标识与所选凭据池中的材料不一致"));
                }
                let fresh = self.repository.credential_lease_metadata(credential.pool_id.clone(), credential.pool_revision.get(), lease.vault_ref.clone(), kind)?;
                if fresh != lease { return Err(AppError::new("credential_authorization_changed", "credentials", "凭据授权已改变，请重新核对")); }
            }
        }
        self.save_prepared_service_draft(input, prepared)
    }
    pub fn select_saved_services(&self, input: SelectSavedServicesInput) -> AppResult<ServiceSelectionReceipt> {
        let _gate = self.translation_gate.lock().map_err(|_| service_state_error())?;
        let receipt = self.service_controller()?.select(&input)?;
        self.abort_invalidated()?;
        self.publish()?;
        Ok(receipt)
    }
    pub fn restore_service_inheritance(&self, input: RestoreServiceInheritanceInput) -> AppResult<ServiceSelectionReceipt> {
        let _gate = self.translation_gate.lock().map_err(|_| service_state_error())?;
        let receipt = self.service_controller()?.restore_inheritance(&input)?;
        self.abort_invalidated()?;
        self.publish()?;
        Ok(receipt)
    }
    pub fn set_service_purpose_enabled(&self, input: SetServicePurposeEnabledInput) -> AppResult<ServiceSelectionReceipt> {
        let _gate = self.translation_gate.lock().map_err(|_| service_state_error())?;
        let receipt = self.service_controller()?.set_purpose_enabled(&input)?;
        self.abort_invalidated()?;
        self.publish()?;
        Ok(receipt)
    }
}
fn service_state_error() -> AppError {
    AppError::new("application_state", "application", "服务配置状态锁不可用")
}
