//! Internal Google authorization contracts. None of these are IPC/Serde DTOs.
//! Repositories prove current stored ownership; a separate operation authority
//! proves in-memory tests. Token exchange and Vault I/O do not hold SQL locks.
use crate::{
    contracts::{AppError, AppResult},
    domain::{
        model_config::credentials::{
            CredentialMaterialKind, CredentialStorageMetadata, CredentialVersionRef,
            GoogleAuthorizationLeaseIdentity, GoogleServiceAccountLeaseIdentity, PrincipalState,
        },
        require_id,
    },
};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    num::NonZeroU32,
    time::{Duration, Instant},
};

pub const GOOGLE_TRANSLATION_SCOPE: &str = "https://www.googleapis.com/auth/cloud-translation";
pub fn google_scope_hash() -> String {
    format!("{:x}", Sha256::digest(GOOGLE_TRANSLATION_SCOPE.as_bytes()))
}
/// One identity for the complete frozen connection, shared by the trusted
/// repository and dispatch adapter. A caller-provided digest is never proof of
/// stored ownership: the repository must recompute it from admitted data.
pub fn google_target_policy_sha256(
    connection: &crate::domain::model_config::service_graph::ConnectionV3,
) -> AppResult<String> {
    crate::domain::canonical_hash(&("luna-google-target-policy-v1", connection))
}
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_authorization", message)
}
fn hash(value: &str) -> AppResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(failure("invalid_google_authority", "授权指纹无效"));
    }
    Ok(())
}
/// Typed projection of the existing Connection HTTP proxy policy. It grants no
/// endpoint/header extension. Full URI parsing belongs to the HTTP adapter.
#[derive(Clone, PartialEq, Eq)]
pub enum GoogleNetworkPolicy {
    Inherit,
    Disabled,
    Override { proxy_url: String },
}
impl fmt::Debug for GoogleNetworkPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inherit => "GoogleNetworkPolicy::Inherit",
            Self::Disabled => "GoogleNetworkPolicy::Disabled",
            Self::Override { .. } => "GoogleNetworkPolicy::Override([REDACTED])",
        })
    }
}
impl GoogleNetworkPolicy {
    pub fn from_connection(
        mode: &str,
        metadata: &std::collections::BTreeMap<String, serde_json::Value>,
    ) -> AppResult<Self> {
        let value = match mode {
            "inherit" => Self::Inherit,
            "disabled" => Self::Disabled,
            "override" => Self::Override {
                proxy_url: metadata
                    .get("proxyUrl")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| failure("invalid_google_proxy", "独立代理地址尚未填写"))?
                    .into(),
            },
            _ => return Err(failure("invalid_google_proxy", "代理策略不受支持")),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> AppResult<()> {
        if let Self::Override { proxy_url } = self {
            let Some((scheme, rest)) = proxy_url.split_once("://") else {
                return Err(failure("invalid_google_proxy", "代理地址无效"));
            };
            if proxy_url.len() > 2048
                || !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
                || proxy_url
                    .bytes()
                    .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
                || proxy_url.contains(['@', '?', '#', '\\'])
                || rest.split('/').next().is_none_or(str::is_empty)
                || rest
                    .split_once('/')
                    .is_some_and(|(_, path)| !path.is_empty())
            {
                return Err(failure(
                    "invalid_google_proxy",
                    "仅支持无凭据和参数的HTTP(S)代理地址",
                ));
            }
        }
        Ok(())
    }
    pub fn cache_identity(&self) -> String {
        match self {
            Self::Inherit => "inherit".into(),
            Self::Disabled => "disabled".into(),
            Self::Override { proxy_url } => format!("override\0{proxy_url}"),
        }
    }
}

/// Shared cancellation belongs to the calling task/login operation. No detached
/// refresh work is authorized by merely cloning this budget.
#[derive(Clone)]
pub struct GoogleBudget {
    operation: super::operation_budget::OperationBudget,
}
fn budget_error(error: AppError) -> AppError {
    if error.stage != "operation_budget" {
        return error;
    }
    match error.code.as_str() {
        "operation_cancelled" => failure("authorization_cancelled", "授权操作已取消"),
        "operation_timeout" => failure("authorization_timeout", "授权操作已超时"),
        "operation_completed" => failure("authorization_completed", "授权操作已完成"),
        _ => failure("authorization_state", "授权操作状态不可核验"),
    }
}
impl GoogleBudget {
    pub fn new(duration: Duration) -> AppResult<Self> {
        super::operation_budget::OperationBudget::new(duration)
            .map(|operation| Self { operation })
            .map_err(budget_error)
    }
    /// Reuse the same explicit test's lifetime; this grants no Google authority.
    pub fn from_operation_budget(operation: super::operation_budget::OperationBudget) -> Self {
        Self { operation }
    }
    pub fn operation_budget(&self) -> super::operation_budget::OperationBudget {
        self.operation.clone()
    }
    pub fn deadline(&self) -> Instant {
        self.operation.deadline()
    }
    pub fn cancel(&self) {
        self.operation.cancel();
    }
    pub fn check(&self) -> AppResult<()> {
        self.operation.check().map_err(budget_error)
    }
    pub async fn cancelled(&self) {
        self.operation.cancelled().await;
    }
    pub fn with_terminal_fence<T>(&self, commit: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
        self.operation
            .with_terminal_fence(commit)
            .map_err(budget_error)
    }
}
impl fmt::Debug for GoogleBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GoogleBudget([operation-owned])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleTestInputScope {
    Sample,
    SelectedCurrent,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleTestOperation {
    pub operation_id: String,
    pub input_scope: GoogleTestInputScope,
    pub input_sha256: String,
    pub request_semantics_sha256: String,
}
/// A test is not a prewritten test_records row. Root must check this operation
/// against its live, profile-bound in-memory registry, including frozen sample.
pub trait GoogleTestOperationAuthority: Send + Sync {
    fn check(&self, request: &GoogleLeaseRequest, operation: &GoogleTestOperation)
    -> AppResult<()>;
    /// A live test operation owns the source selected once by the trusted
    /// backend. SQL independently verifies it; even a sole pool member is not
    /// proof that this test selected that identity.
    fn service_account_source(
        &self,
        _request: &GoogleLeaseRequest,
        _operation: &GoogleTestOperation,
    ) -> AppResult<GoogleServiceAccountLeaseIdentity> {
        Err(failure(
            "google_test_source_not_admitted",
            "本次测试未固定服务账号来源",
        ))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleLeaseTarget {
    Attempt { attempt_id: String, task_id: String },
    TestOperation(GoogleTestOperation),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleLeaseRequest {
    pub target: GoogleLeaseTarget,
    pub config_revision_id: String,
    pub connection_id: String,
    pub target_policy_sha256: String,
    pub profile_generation: NonZeroU32,
    pub server_instance_id: String,
}
impl GoogleLeaseRequest {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.config_revision_id)?;
        require_id(&self.connection_id)?;
        require_id(&self.server_instance_id)?;
        hash(&self.target_policy_sha256)?;
        match &self.target {
            GoogleLeaseTarget::Attempt {
                attempt_id,
                task_id,
            } => {
                require_id(attempt_id)?;
                require_id(task_id)?;
            }
            GoogleLeaseTarget::TestOperation(t) => {
                require_id(&t.operation_id)?;
                hash(&t.input_sha256)?;
                hash(&t.request_semantics_sha256)?;
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleClientMode {
    Application,
    Custom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleClientSetup {
    Unknown,
    Verified,
    Unsupported,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleClientKey {
    pub client_profile_id: String,
    pub client_profile_revision: NonZeroU32,
}
/// Maps oauth_client_profiles: issuer is fixed google_accounts and redirect
/// policy is fixed loopback_pkce, never a caller-controlled URL or secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleClientProfile {
    pub key: GoogleClientKey,
    pub public_client_id: String,
    pub client_mode: GoogleClientMode,
    pub policy_sha256: String,
    pub setup_state: GoogleClientSetup,
}
impl GoogleClientProfile {
    pub fn validate_verified(&self) -> AppResult<()> {
        require_id(&self.key.client_profile_id)?;
        hash(&self.policy_sha256)?;
        if self.public_client_id.is_empty()
            || self.public_client_id.len() > 512
            || self
                .public_client_id
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
        {
            return Err(failure("invalid_google_client", "Google客户端公开身份无效"));
        }
        if self.setup_state != GoogleClientSetup::Verified {
            return Err(failure(
                "google_client_setup_unverified",
                "Google登录客户端尚未核验",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleGrantState {
    Pending,
    Active,
    Revoked,
    NeedsReauth,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleGrantCursor {
    pub grant_id: String,
    pub authorization_epoch: Option<NonZeroU32>,
    pub material_revision: Option<NonZeroU32>,
    pub control_revision: u32,
}
impl GoogleGrantCursor {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.grant_id)
    }
}
/// Vault locators are private backend metadata. Never log or serialize them.
#[derive(Clone, PartialEq, Eq)]
pub struct GoogleVaultMaterial {
    pub credential: CredentialVersionRef,
    pub vault_ref: String,
    pub storage: CredentialStorageMetadata,
    pub lifecycle_revision: u32,
}
impl fmt::Debug for GoogleVaultMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GoogleVaultMaterial([REDACTED])")
    }
}
impl GoogleVaultMaterial {
    pub fn validate_kind(&self, kind: CredentialMaterialKind) -> AppResult<()> {
        self.credential.validate()?;
        require_id(&self.vault_ref)?;
        if self.storage != (CredentialStorageMetadata::EnvelopeV1 { kind }) {
            return Err(failure(
                "google_material_kind_mismatch",
                "Google秘密材料的可信类型不符",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq)]
pub enum GoogleLeaseOrigin {
    ServiceAccount(GoogleServiceAccountLeaseIdentity),
    UserOAuth {
        identity: GoogleAuthorizationLeaseIdentity,
        client: GoogleClientProfile,
        control_revision: u32,
        lifecycle_revision: u32,
    },
}
impl fmt::Debug for GoogleLeaseOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Existing credential-version IDs are the private Vault locator. Keep
        // old grant identity/wire semantics intact while redacting new wrappers.
        f.write_str(match self {
            Self::ServiceAccount(_) => "GoogleLeaseOrigin::ServiceAccount([REDACTED])",
            Self::UserOAuth { .. } => "GoogleLeaseOrigin::UserOAuth([REDACTED])",
        })
    }
}
impl GoogleLeaseOrigin {
    pub fn cursor(&self) -> Option<GoogleGrantCursor> {
        match self {
            Self::ServiceAccount(_) => None,
            Self::UserOAuth {
                identity,
                control_revision,
                ..
            } => Some(GoogleGrantCursor {
                grant_id: identity.grant_id.clone(),
                authorization_epoch: Some(identity.authorization_epoch),
                material_revision: Some(identity.material_revision),
                control_revision: *control_revision,
            }),
        }
    }
    pub fn scopes_hash(&self) -> &str {
        match self {
            Self::ServiceAccount(s) => &s.scopes_hash,
            Self::UserOAuth { identity, .. } => &identity.scopes_hash,
        }
    }
    pub fn validate(&self) -> AppResult<()> {
        match self {
            Self::ServiceAccount(s) => s.validate()?,
            Self::UserOAuth {
                identity, client, ..
            } => {
                identity.validate()?;
                client.validate_verified()?;
            }
        }
        if self.scopes_hash() != google_scope_hash() {
            return Err(failure("google_scope_mismatch", "Google翻译授权范围不符"));
        }
        Ok(())
    }
}
/// Repository must load this from the exact admitted target, owned grant/pool,
/// current epoch/cursor/lifecycle, immutable client policy and stopped controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleAuthoritySnapshot {
    pub request: GoogleLeaseRequest,
    pub origin: GoogleLeaseOrigin,
    pub material: GoogleVaultMaterial,
    pub network_policy: GoogleNetworkPolicy,
}
impl GoogleAuthoritySnapshot {
    pub fn validate(&self) -> AppResult<()> {
        self.request.validate()?;
        self.origin.validate()?;
        self.network_policy.validate()?;
        let (kind, credential, lifecycle) = match &self.origin {
            GoogleLeaseOrigin::ServiceAccount(s) => (
                CredentialMaterialKind::GoogleServiceAccount,
                &s.credential_version,
                s.lifecycle_revision,
            ),
            GoogleLeaseOrigin::UserOAuth {
                identity,
                lifecycle_revision,
                ..
            } => (
                CredentialMaterialKind::GoogleOauthRefresh,
                &identity.credential_version,
                *lifecycle_revision,
            ),
        };
        self.material.validate_kind(kind)?;
        if &self.material.credential != credential || self.material.lifecycle_revision != lifecycle
        {
            return Err(failure(
                "google_authority_material_mismatch",
                "授权秘密材料版本不属于当前来源",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleOwnedGrant {
    pub cursor: GoogleGrantCursor,
    pub client: GoogleClientProfile,
    pub state: GoogleGrantState,
    pub scopes_sha256: String,
    pub principal_state: PrincipalState,
    pub material: Option<GoogleVaultMaterial>,
}
impl GoogleOwnedGrant {
    pub fn validate(&self) -> AppResult<()> {
        self.cursor.validate()?;
        self.client.validate_verified()?;
        if let PrincipalState::Verified { subject } = &self.principal_state
            && (subject.is_empty() || subject.len() > 512 || subject.contains('\0')) {
                return Err(failure("invalid_google_principal", "授权主体标识无效"));
            }
        if self.scopes_sha256 != google_scope_hash() {
            return Err(failure("google_scope_mismatch", "授权范围不符"));
        }
        if let Some(material) = &self.material {
            material.validate_kind(CredentialMaterialKind::GoogleOauthRefresh)?;
        }
        if self.cursor.material_revision.is_some() != self.material.is_some() {
            return Err(failure(
                "google_material_cursor_mismatch",
                "授权材料游标不完整",
            ));
        }
        if self.state == GoogleGrantState::Active
            && (self.cursor.authorization_epoch.is_none() || self.material.is_none())
        {
            return Err(failure("google_grant_not_active", "授权尚未完成"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleLoginRequest {
    pub operation_id: String,
    pub client: GoogleClientKey,
    pub profile_generation: NonZeroU32,
    pub server_instance_id: String,
    pub network_policy: GoogleNetworkPolicy,
}
impl GoogleLoginRequest {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.operation_id)?;
        require_id(&self.client.client_profile_id)?;
        require_id(&self.server_instance_id)?;
        self.network_policy.validate()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GooglePendingLogin {
    pub request: GoogleLoginRequest,
    pub grant: GoogleOwnedGrant,
}
impl GooglePendingLogin {
    pub fn validate(&self) -> AppResult<()> {
        self.request.validate()?;
        self.grant.validate()?;
        if self.grant.client.key != self.request.client
            || self.grant.state != GoogleGrantState::Pending
            || self.grant.material.is_some()
        {
            return Err(failure(
                "google_login_identity_mismatch",
                "本次登录身份或状态不符",
            ));
        }
        Ok(())
    }
}
/// The same operation/profile fence is retained through reserve, Vault write,
/// CAS commit and cleanup. A Repository instance is not proof of current profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleMaterialContext {
    InitialLogin(GooglePendingLogin),
    Refresh {
        operation_id: String,
        request: GoogleLeaseRequest,
        expected: GoogleGrantCursor,
        /// Exact material/lifecycle read BEFORE the refresh I/O. Reserve and
        /// commit must reject a revoke/re-enable cycle, not just stale cursor.
        mint_material: GoogleVaultMaterial,
        network_policy: GoogleNetworkPolicy,
    },
}
impl GoogleMaterialContext {
    pub fn expected(&self) -> &GoogleGrantCursor {
        match self {
            Self::InitialLogin(p) => &p.grant.cursor,
            Self::Refresh { expected, .. } => expected,
        }
    }
    pub fn validate(&self) -> AppResult<()> {
        match self {
            Self::InitialLogin(p) => p.validate(),
            Self::Refresh {
                operation_id,
                request,
                expected,
                mint_material,
                network_policy,
            } => {
                require_id(operation_id)?;
                request.validate()?;
                expected.validate()?;
                mint_material.validate_kind(CredentialMaterialKind::GoogleOauthRefresh)?;
                network_policy.validate()?;
                if expected.authorization_epoch.is_none() || expected.material_revision.is_none() {
                    return Err(failure(
                        "google_material_cursor_mismatch",
                        "刷新必须绑定当前完整材料游标",
                    ));
                }
                Ok(())
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleGrantControlRequest {
    pub operation_id: String,
    pub expected: GoogleGrantCursor,
    pub profile_generation: NonZeroU32,
    pub server_instance_id: String,
    pub network_policy: GoogleNetworkPolicy,
}
impl GoogleGrantControlRequest {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.operation_id)?;
        self.expected.validate()?;
        require_id(&self.server_instance_id)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GooglePendingMaterial {
    pub allocation_id: String,
    pub context: GoogleMaterialContext,
    pub material: GoogleVaultMaterial,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleMaterialTransition {
    InitialLogin,
    Refresh,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleMaterialCommit {
    pub pending: GooglePendingMaterial,
    pub transition: GoogleMaterialTransition,
    pub scopes_sha256: String,
    pub committed_at_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleExecutionLeaseRecord {
    pub lease_id: String,
    pub request: GoogleLeaseRequest,
    pub origin: GoogleLeaseOrigin,
    pub minted_at_ms: i64,
    pub expires_at_ms: i64,
    pub network_policy: GoogleNetworkPolicy,
}
impl GoogleExecutionLeaseRecord {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.lease_id)?;
        self.request.validate()?;
        self.origin.validate()?;
        self.network_policy.validate()?;
        if self.minted_at_ms < 0 || self.expires_at_ms <= self.minted_at_ms {
            return Err(failure("google_lease_time_invalid", "短期授权时间无效"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleGrantInvalidation {
    Revoked,
    NeedsReauth,
}

/// Actual repository identity used only during trusted handler composition.
/// This is neither an IPC field nor a caller-supplied authorization proof.
#[derive(Clone)]
pub struct GoogleRuntimeFrame {
    pub profile_id: String,
    pub profile_generation: NonZeroU32,
    pub server_instance_id: String,
    writer_identity: std::sync::Weak<()>,
}
impl PartialEq for GoogleRuntimeFrame {
    fn eq(&self, other: &Self) -> bool {
        self.profile_id == other.profile_id
            && self.profile_generation == other.profile_generation
            && self.server_instance_id == other.server_instance_id
            && std::sync::Weak::ptr_eq(&self.writer_identity, &other.writer_identity)
    }
}
impl Eq for GoogleRuntimeFrame {}
impl fmt::Debug for GoogleRuntimeFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GoogleRuntimeFrame([repository-owned])")
    }
}
impl GoogleRuntimeFrame {
    pub fn new(
        profile_id: String,
        profile_generation: NonZeroU32,
        server_instance_id: String,
        writer_identity: std::sync::Weak<()>,
    ) -> AppResult<Self> {
        let frame = Self {
            profile_id,
            profile_generation,
            server_instance_id,
            writer_identity,
        };
        frame.validate()?;
        Ok(frame)
    }
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.profile_id)?;
        require_id(&self.server_instance_id)?;
        if self.writer_identity.upgrade().is_none() {
            return Err(failure(
                "google_handler_writer_closed",
                "实际存储writer已释放",
            ));
        }
        Ok(())
    }
}
/// A Gateway owns this handle after its actual Google handler is installed.
/// SQLite retains only a Weak handle; no SQL/Broker callback can run in an actor.
/// ```compile_fail
/// use app_core::ports::google_authorization::GoogleHandlerRegistration;
/// fn export(value: &GoogleHandlerRegistration) { let _ = serde_json::to_string(value); }
/// ```
pub struct GoogleHandlerRegistration {
    frame: GoogleRuntimeFrame,
    owner: std::sync::Weak<()>,
    handler_lifetime: std::sync::Weak<()>,
}
/// Only an actual Gateway owns this lifetime. Read-only actor observations may
/// borrow the registration but cannot prolong an installed handler's life.
#[derive(Clone)]
pub struct GoogleHandlerHandle {
    registration: std::sync::Arc<GoogleHandlerRegistration>,
    _handler_lifetime: std::sync::Arc<()>,
}
impl GoogleHandlerHandle {
    pub fn new(frame: GoogleRuntimeFrame, owner: std::sync::Weak<()>) -> AppResult<Self> {
        frame.validate()?;
        if owner.upgrade().is_none() {
            return Err(failure(
                "google_handler_owner_closed",
                "Google处理器拥有者已释放",
            ));
        }
        let lifetime = std::sync::Arc::new(());
        let registration = std::sync::Arc::new(GoogleHandlerRegistration {
            frame,
            owner,
            handler_lifetime: std::sync::Arc::downgrade(&lifetime),
        });
        Ok(Self {
            registration,
            _handler_lifetime: lifetime,
        })
    }
    pub fn registration(&self) -> &std::sync::Arc<GoogleHandlerRegistration> {
        &self.registration
    }
    pub fn is_live(&self) -> bool {
        self.registration.is_live()
    }
    pub fn frame(&self) -> &GoogleRuntimeFrame {
        self.registration.frame()
    }
}
impl fmt::Debug for GoogleHandlerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GoogleHandlerHandle([handler-owned])")
    }
}
impl GoogleHandlerRegistration {
    pub fn acquire(self: &std::sync::Arc<Self>) -> AppResult<GoogleHandlerHandle> {
        if self.owner.upgrade().is_none() {
            return Err(failure(
                "google_handler_owner_closed",
                "Google处理器拥有者已释放",
            ));
        }
        let lifetime = self
            .handler_lifetime
            .upgrade()
            .ok_or_else(|| failure("google_handler_closed", "Google处理器已释放"))?;
        Ok(GoogleHandlerHandle {
            registration: self.clone(),
            _handler_lifetime: lifetime,
        })
    }
    pub fn frame(&self) -> &GoogleRuntimeFrame {
        &self.frame
    }
    pub fn is_live(&self) -> bool {
        self.owner.upgrade().is_some() && self.handler_lifetime.upgrade().is_some()
    }
    pub fn same_owner(&self, owner: &std::sync::Weak<()>) -> bool {
        std::sync::Weak::ptr_eq(&self.owner, owner)
    }
}
impl fmt::Debug for GoogleHandlerRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GoogleHandlerRegistration([composition-owned])")
    }
}

/// Short synchronous transactions only. No method receives a token/private key.
/// SQL/Vault ownership cannot be replaced by copying these types in a fixture.
pub trait GoogleAuthorizationRepository: Send + Sync {
    /// Called only by actual Gateway composition, through its Broker-bound repo.
    /// No default installed state may be inferred from compiled protocol support.
    fn register_google_handler(
        &self, _destination: GoogleRuntimeFrame, _owner: std::sync::Weak<()>,
    ) -> AppResult<GoogleHandlerHandle> {
        Err(failure("google_handler_registration_unavailable", "授权存储未提供真实处理器安装登记"))
    }

    fn lease_authority(&self, request: &GoogleLeaseRequest) -> AppResult<GoogleAuthoritySnapshot>;
    /// Compare complete frozen snapshot + real current attempt/config controls.
    fn check_lease_authority(&self, authority: &GoogleAuthoritySnapshot) -> AppResult<()>;
    /// The access token records the material ACTUALLY used to mint it. A normal
    /// same-grant refresh may advance current material/control without revoking
    /// an already issued token. Verify owned historical material + its lifecycle,
    /// current active epoch/client/scope/target/profile and stopped controls.
    /// Cross-grant/client/scope/source changes never inherit this allowance.
    fn check_issued_lease(&self, record: &GoogleExecutionLeaseRecord) -> AppResult<()>;
    /// Actual started attempt only. Reject TestOperation: terminal test_records
    /// cannot be forged as a pending/passed placeholder to obtain a token.
    /// Re-run check_issued_lease in the actual INSERT transaction. Do not replace
    /// the record's old mint material with the new refresh cursor.
    fn record_attempt_lease(&self, record: &GoogleExecutionLeaseRecord) -> AppResult<()>;
    /// Allocate owned pending grant using a verified immutable client profile.
    fn prepare_login(
        &self,
        request: &GoogleLoginRequest,
        now_ms: i64,
    ) -> AppResult<GooglePendingLogin>;
    fn check_login(&self, pending: &GooglePendingLogin) -> AppResult<()>;
    fn reserve_material(
        &self,
        context: &GoogleMaterialContext,
        now_ms: i64,
    ) -> AppResult<GooglePendingMaterial>;
    /// Initial login creates a new active snapshot; refresh retains auth epoch
    /// and principal/client/scope, appends material and CAS-advances cursor only.
    fn commit_material(&self, command: &GoogleMaterialCommit) -> AppResult<GoogleOwnedGrant>;
    /// True authorizes deleting ONLY this pending Vault allocation after an
    /// atomic stored check that it was never activated/referenced. False means
    /// do not delete (e.g. a commit actually succeeded despite a lost response).
    fn release_failed_material(
        &self,
        pending: &GooglePendingMaterial,
        now_ms: i64,
    ) -> AppResult<bool>;
    fn owned_grant(&self, request: &GoogleGrantControlRequest) -> AppResult<GoogleOwnedGrant>;
    /// CAS immutable authorization snapshot + grant cursor before network revoke;
    /// cancel affected tasks/invalidate caches via the shared runtime fence.
    fn invalidate_grant(
        &self,
        request: &GoogleGrantControlRequest,
        state: GoogleGrantInvalidation,
        now_ms: i64,
    ) -> AppResult<GoogleOwnedGrant>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model_config::service_graph::{
        ConnectionV3, GoogleAuthorizationRef, HttpServiceOptionsV1,
    };
    #[test]
    fn target_policy_binds_complete_connection_and_versioned_hash_domain() {
        let base:ConnectionV3=serde_json::from_value(serde_json::json!({"connectionId":"connection","displayName":"Google","protocol":"google_translate_v3","deployment":"cloud","authMode":"google_oauth","endpoint":"https://translation.googleapis.com","proxyMode":"inherit","networkMetadata":{},"availabilityIntent":true,"httpOptions":{"protocol":"google_translate_v3","projectId":"project-a","location":"global"},"googleAuthorization":{"grantId":"grant-a","authorizationEpoch":1}})).unwrap();
        let original = google_target_policy_sha256(&base).unwrap();
        assert_eq!(original.len(), 64);
        assert_ne!(original, crate::domain::canonical_hash(&base).unwrap());
        let mut variants = vec![];
        let mut c = base.clone();
        c.endpoint = Some("https://custom.example.invalid".into());
        variants.push(c);
        let mut c = base.clone();
        c.auth_mode = "google_service_account".into();
        variants.push(c);
        let mut c = base.clone();
        c.credential_pool_id = Some("pool-a".into());
        c.credential_pool_revision = NonZeroU32::new(1);
        variants.push(c);
        let mut c = base.clone();
        c.google_authorization = Some(GoogleAuthorizationRef {
            grant_id: "grant-b".into(),
            authorization_epoch: NonZeroU32::new(2).unwrap(),
        });
        variants.push(c);
        let mut c = base.clone();
        c.proxy_mode = "disabled".into();
        variants.push(c);
        let mut c = base.clone();
        c.proxy_mode = "override".into();
        c.network_metadata
            .insert("proxyUrl".into(), "http://127.0.0.1:8080".into());
        variants.push(c);
        let mut c = base.clone();
        c.http_options = Some(HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id: "project-b".into(),
            location: "global".into(),
        });
        variants.push(c);
        let mut c = base.clone();
        c.availability_intent = false;
        variants.push(c);
        for changed in variants {
            assert_ne!(original, google_target_policy_sha256(&changed).unwrap());
        }
        // The fixed serialized literal is independent of the helper encoder.
        let bytes=br#"["luna-google-target-policy-v1",{"connectionId":"connection","displayName":"Google","protocol":"google_translate_v3","deployment":"cloud","authMode":"google_oauth","endpoint":"https://translation.googleapis.com","proxyMode":"inherit","networkMetadata":{},"availabilityIntent":true,"httpOptions":{"protocol":"google_translate_v3","projectId":"project-a","location":"global"},"googleAuthorization":{"grantId":"grant-a","authorizationEpoch":1}}]"#;
        assert_eq!(original, format!("{:x}", Sha256::digest(bytes)));
    }
}
