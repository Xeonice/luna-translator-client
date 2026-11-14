//! Real SQL/Vault/Broker settings adapter. Does not implement a second OAuth flow.
use super::{
    google_authorization::{GoogleAuthorizationService, GoogleLoginSession},
    google_token::validate_google_network_policy,
};
use crate::storage::SqliteStore;
use app_core::{
    contracts::*,
    domain::{canonical_hash, model_config::service_graph::GoogleAuthorizationRef},
    ports::{IdentitySource, google_authorization::*, google_interactive::*},
};
use std::{num::NonZeroU32, sync::Arc};
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_interactive", message)
}
pub struct SqliteGoogleInteractiveBackend {
    store: Arc<SqliteStore>,
    broker: Arc<GoogleAuthorizationService>,
    clock: Arc<dyn IdentitySource>,
    vault: Arc<dyn app_core::ports::runtime::CredentialVault>,
}
impl SqliteGoogleInteractiveBackend {
    pub fn new(
        store: Arc<SqliteStore>,
        broker: Arc<GoogleAuthorizationService>,
        clock: Arc<dyn IdentitySource>,
        vault: Arc<dyn app_core::ports::runtime::CredentialVault>,
    ) -> Self {
        Self {
            store,
            broker,
            clock,
            vault,
        }
    }
    /// Startup-only dedicated recovery. Generic pool recovery must not infer
    /// ownership of unpooled OAuth material. Candidates and CAS remain SQL-owned.
    pub async fn recover_abandoned_materials(
        &self,
        vault: Arc<dyn app_core::ports::runtime::CredentialVault>,
    ) -> AppResult<u32> {
        use app_core::ports::ProductRepository;
        let store = self.store.clone();
        let frame = blocking(move || store.google_runtime_frame()).await?;
        let generation = frame.profile_generation;
        let server = frame.server_instance_id;
        let store = self.store.clone();
        let check = blocking(move || store.google_interactive_state(generation, None)).await?;
        self.broker.check_interactive_handler(&check.handler)?;
        let store = self.store.clone();
        let origin = server.clone();
        let time = self.clock.now_ms();
        let candidates =
            blocking(move || store.google_material_cleanup_candidates(generation, origin, time))
                .await?;
        let mut cleaned = 0;
        for material in candidates {
            let vault = vault.clone();
            let reference = material.vault_ref.clone();
            blocking(move || vault.delete(&reference)).await?;
            let store = self.store.clone();
            let origin = server.clone();
            let time = self.clock.now_ms();
            blocking(move || {
                store.confirm_google_material_cleaned(material, generation, origin, time)
            })
            .await?;
            cleaned += 1;
        }
        Ok(cleaned)
    }
}
fn policy(proxy: Option<&GoogleInteractiveProxy>) -> AppResult<GoogleNetworkPolicy> {
    let mut metadata = std::collections::BTreeMap::new();
    let mode = if let Some(proxy) = proxy {
        if let Some(url) = &proxy.proxy_url {
            metadata.insert("proxyUrl".into(), serde_json::Value::String(url.clone()));
        }
        proxy.mode.as_str()
    } else {
        "inherit"
    };
    let policy = GoogleNetworkPolicy::from_connection(mode, &metadata)?;
    validate_google_network_policy(&policy)?;
    Ok(policy)
}
fn client(context: &GoogleInteractiveContext) -> AppResult<GoogleClientProfile> {
    context.validate()?;
    if context.client_mode == GoogleInteractiveClientMode::Application {
        return Err(failure(
            "application_client_unavailable",
            "内置Google应用客户端资料尚未配置；可在高级设置使用自己的公开桌面客户端ID",
        ));
    }
    let id = context.public_client_id.as_deref().unwrap_or("").trim();
    if id.is_empty()
        || id.len() > 512
        || id
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(failure(
            "google_client_invalid",
            "请填写公开桌面客户端ID；本地格式检查不代表Google端已验证",
        ));
    }
    let hash = canonical_hash(&(
        "google-desktop-pkce-client-v1",
        id,
        "google_accounts",
        "loopback_pkce",
        GOOGLE_TRANSLATION_SCOPE,
    ))?;
    let profile = GoogleClientProfile {
        key: GoogleClientKey {
            client_profile_id: format!("google-desktop-{hash}"),
            client_profile_revision: NonZeroU32::new(1).expect("nonzero"),
        },
        public_client_id: id.into(),
        client_mode: GoogleClientMode::Custom,
        policy_sha256: hash,
        setup_state: GoogleClientSetup::Verified,
    };
    profile.validate_verified()?;
    Ok(profile)
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    // Await the actual blocking handle. Cancellation never drops physical SQL,
    // Vault or broker I/O and never claims that an outstanding thread drained.
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| failure("google_worker_failed", "授权工作未能正常收尾"))?
}
impl GoogleInteractiveBackend for SqliteGoogleInteractiveBackend {
    fn recover_abandoned<'a>(&'a self) -> GoogleInteractiveFuture<'a, u32> {
        Box::pin(async move { self.recover_abandoned_materials(self.vault.clone()).await })
    }

    fn inspect<'a>(
        &'a self,
        context: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        Box::pin(async move {
            context.validate()?;
            policy(context.proxy.as_ref())?;
            let store = self.store.clone();
            let generation = context.expected_profile_generation;
            let reference = context.authorization.clone();
            let stored =
                blocking(move || store.google_interactive_state(generation, reference)).await?;
            self.broker.check_interactive_handler(&stored.handler)?;
            if let Some(grant) = stored.grant {
                // SQL resolved this explicitly owned grant and its original
                // issuer, redirect, scopes, client revision and material. A new
                // UI adapter's derived key must not replace that saved identity.
                let matches_client = match context.client_mode {
                    GoogleInteractiveClientMode::Custom => grant.client.client_mode == GoogleClientMode::Custom
                        && context.public_client_id.as_deref().map(str::trim) == Some(grant.client.public_client_id.as_str()),
                    GoogleInteractiveClientMode::Application => grant.client.client_mode == GoogleClientMode::Application
                        && context.public_client_id.as_deref().map(str::trim).is_none_or(|id| id == grant.client.public_client_id),
                };
                if !matches_client {
                    return Err(failure("google_client_identity_mismatch", "此授权与当前编辑的客户端不符，请重新核对"));
                }
                // Reading/revoking an owned old grant is independent of whether
                // this build has material for starting a new application login.
                let can_login = client(&context).is_ok();
                let exact = grant.cursor.authorization_epoch
                    == context
                        .authorization
                        .as_ref()
                        .map(|r| r.authorization_epoch);
                let (state, active) = if !exact {
                    (GoogleInteractiveState::NeedsReauth, false)
                } else {
                    match grant.state {
                        GoogleGrantState::Active => (GoogleInteractiveState::Authorized, true),
                        GoogleGrantState::Revoked => (GoogleInteractiveState::Revoked, false),
                        _ => (GoogleInteractiveState::NeedsReauth, false),
                    }
                };
                return Ok(GoogleAuthorizationProjection {
                    state,
                    can_login,
                    can_logout: active,
                    message: if active {
                        "已有本机授权；保存与选用是独立操作"
                    } else {
                        "原授权已停用或过期，请重新授权"
                    }
                    .into(),
                    authorization: if active { context.authorization } else { None },
                    affected_service_count: stored.affected_service_count,
                });
            }
            let _profile = match client(&context) {
                Ok(profile) => profile,
                Err(error)
                    if matches!(
                        error.code.as_str(),
                        "application_client_unavailable" | "google_client_invalid"
                    ) =>
                {
                    return Ok(GoogleAuthorizationProjection {
                        state: GoogleInteractiveState::Unconfigured,
                        can_login: false,
                        can_logout: false,
                        message: error.message,
                        authorization: None,
                        affected_service_count: 0,
                    });
                }
                Err(error) => return Err(error),
            };
            Ok(GoogleAuthorizationProjection {
                state: GoogleInteractiveState::Ready,
                can_login: true,
                can_logout: false,
                message: "公开客户端资料已在本机准备；Google端状态需显式登录验证".into(),
                authorization: None,
                affected_service_count: 0,
            })
        })
    }
    fn begin<'a>(
        &'a self,
        input: StartGoogleLoginInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move {
            let profile = client(&input.context)?;
            let network_policy = policy(input.context.proxy.as_ref())?;
            budget.check()?;
            let store = self.store.clone();
            let generation = input.context.expected_profile_generation;
            let stored = blocking(move || store.google_interactive_state(generation, None)).await?;
            self.broker.check_interactive_handler(&stored.handler)?;
            let request = GoogleLoginRequest {
                operation_id: input.operation_id,
                client: profile.key.clone(),
                profile_generation: generation,
                server_instance_id: stored.frame.server_instance_id,
                network_policy,
            };
            let store = self.store.clone();
            let registration = request.clone();
            let time = self.clock.now_ms();
            let check = budget.clone();
            blocking(move || {
                check.check()?;
                store.register_google_client_profile(registration, profile, time)
            })
            .await?;
            let broker = self.broker.clone();
            let runtime = tokio::runtime::Handle::current();
            let login_request = request.clone();
            let control = budget.clone();
            let session =
                blocking(move || runtime.block_on(broker.begin_login(login_request, control)))
                    .await?;
            Ok(Box::new(Login {
                session,
                broker: self.broker.clone(),
                request,
            }) as Box<dyn OwnedGoogleLogin>)
        })
    }
    fn revoke<'a>(
        &'a self,
        input: RevokeGoogleAuthorizationInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async move {
            let store = self.store.clone();
            let generation = input.expected_profile_generation;
            let reference = input.authorization.clone();
            let stored =
                blocking(move || store.google_interactive_state(generation, Some(reference)))
                    .await?;
            self.broker.check_interactive_handler(&stored.handler)?;
            let grant = stored
                .grant
                .ok_or_else(|| failure("google_grant_not_found", "授权不存在"))?;
            if grant.state != GoogleGrantState::Active
                || grant.cursor.authorization_epoch != Some(input.authorization.authorization_epoch)
            {
                return Err(failure(
                    "google_grant_cursor_stale",
                    "授权状态已变化，请重新核对",
                ));
            }
            let request = GoogleGrantControlRequest {
                operation_id: input.operation_id,
                expected: grant.cursor,
                profile_generation: generation,
                server_instance_id: stored.frame.server_instance_id,
                network_policy: policy(input.proxy.as_ref())?,
            };
            let broker = self.broker.clone();
            let runtime = tokio::runtime::Handle::current();
            let result = blocking(move || runtime.block_on(broker.revoke(request, budget))).await;
            let store = self.store.clone();
            let reference = input.authorization;
            let after =
                blocking(move || store.google_interactive_state(generation, Some(reference)))
                    .await?;
            if !after
                .grant
                .is_some_and(|g| g.state == GoogleGrantState::Revoked)
            {
                return Err(result.err().unwrap_or_else(|| {
                    failure("google_revoke_not_confirmed", "本机撤权尚未确认")
                }));
            }
            Ok(GoogleRevokeProjection {
                local_revoked: true,
                remote_revocation_confirmed: result.is_ok(),
                message: if result.is_ok() {
                    "本机授权已解除，Google远端撤权已确认"
                } else {
                    "本机授权已解除；Google远端撤权未获确认，共用此授权的服务已停用"
                }
                .into(),
            })
        })
    }
}
struct Login {
    session: GoogleLoginSession,
    broker: Arc<GoogleAuthorizationService>,
    request: GoogleLoginRequest,
}
impl OwnedGoogleLogin for Login {
    fn browser_intent(&self) -> GoogleSystemBrowserIntent {
        GoogleSystemBrowserIntent::from_broker_url(self.session.intent().authorization_url().into())
    }
    fn finish(self: Box<Self>) -> GoogleInteractiveFuture<'static, Box<dyn CompletedGoogleLogin>> {
        Box::pin(async move {
            let Login {
                session,
                broker,
                request,
            } = *self;
            let runtime = tokio::runtime::Handle::current();
            let grant = blocking(move || runtime.block_on(session.finish())).await?;
            let reference = GoogleAuthorizationRef {
                grant_id: grant.cursor.grant_id.clone(),
                authorization_epoch: grant
                    .cursor
                    .authorization_epoch
                    .ok_or_else(|| failure("google_grant_not_ready", "授权未完成"))?,
            };
            let control = GoogleGrantControlRequest {
                operation_id: format!("{}-unpublished", request.operation_id),
                expected: grant.cursor,
                profile_generation: request.profile_generation,
                server_instance_id: request.server_instance_id,
                network_policy: request.network_policy,
            };
            Ok(Box::new(Completed {
                broker,
                control,
                reference,
            }) as Box<dyn CompletedGoogleLogin>)
        })
    }
}
struct Completed {
    broker: Arc<GoogleAuthorizationService>,
    control: GoogleGrantControlRequest,
    reference: GoogleAuthorizationRef,
}
impl CompletedGoogleLogin for Completed {
    fn authorization(&self) -> GoogleAuthorizationRef {
        self.reference.clone()
    }
    fn discard(self: Box<Self>) -> GoogleInteractiveFuture<'static, ()> {
        Box::pin(async move {
            blocking(move || self.broker.invalidate_unpublished_login(self.control)).await
        })
    }
}
