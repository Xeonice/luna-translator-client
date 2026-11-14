//! Google authorization orchestration. SQL is behind the Core repository port;
//! access tokens stay in memory and actual mint provenance survives refresh.
use super::{
    google_auth::service_account_assertion,
    google_oauth::{GoogleBrowserIntent, GoogleOAuthCallback, GoogleOAuthFlow},
    google_token::{
        GoogleTokenReply, GoogleTokenRequest, GoogleTokenResponse, GoogleTokenTransport,
        MAX_ACCESS_LIFETIME_SECONDS, validate_google_network_policy, within_budget,
    },
};
use app_core::{
    contracts::{AppError, AppResult},
    domain::model_config::credentials::{
        CredentialMaterial, ProviderAuthLease, SecretText, decode_vault_credential,
        encode_vault_credential,
    },
    ports::{google_authorization::*, runtime::CredentialVault},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, MutexGuard, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const MAX_CACHE_ENTRIES: usize = 64;
const MAX_ACTIVE_KEYS: usize = 256;
const CACHE_SKEW_MS: i64 = 30_000;
const MAX_CLOCK_DRIFT_MS: i64 = 5_000;
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_authorization", message)
}
fn locked<T>(mutex: &Mutex<T>) -> AppResult<MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_| failure("google_authorization_lock", "授权状态暂不可用"))
}
#[derive(Debug, Clone, Copy)]
pub struct GoogleNow {
    pub unix_ms: i64,
    pub monotonic: Instant,
}
pub trait GoogleClock: Send + Sync {
    fn now(&self) -> AppResult<GoogleNow>;
}
pub struct SystemGoogleClock;
impl GoogleClock for SystemGoogleClock {
    fn now(&self) -> AppResult<GoogleNow> {
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|n| i64::try_from(n.as_millis()).ok())
            .filter(|n| *n > 0 && *n <= 253_402_300_799_000)
            .ok_or_else(|| failure("authorization_clock_invalid", "系统时间不能用于授权"))?;
        Ok(GoogleNow {
            unix_ms,
            monotonic: Instant::now(),
        })
    }
}
fn current_now(clock: &dyn GoogleClock) -> AppResult<GoogleNow> {
    let now = clock.now()?;
    if now.unix_ms <= 0 || now.unix_ms > 253_402_300_799_000 {
        return Err(failure(
            "authorization_clock_invalid",
            "系统时间不能用于授权",
        ));
    }
    Ok(now)
}
#[derive(Clone)]
struct Fence {
    source: Arc<SourceFence>,
    revision: u64,
    wake: u64,
}
struct SourceFence {
    revision: AtomicU64,
    changed: tokio::sync::Notify,
}
impl Fence {
    fn check(&self, wake: &AtomicU64) -> AppResult<()> {
        if self.source.revision.load(Ordering::SeqCst) != self.revision
            || wake.load(Ordering::SeqCst) != self.wake
        {
            return Err(failure(
                "google_authority_changed",
                "授权或休眠状态已变化，请重新核验",
            ));
        }
        Ok(())
    }
    async fn changed(&self, inner: &Inner) {
        loop {
            let source = self.source.changed.notified();
            let wake = inner.wake_changed.notified();
            tokio::pin!(source, wake);
            // Register before reading the revisions to avoid a lost wake when
            // invalidation races with the first poll of this future.
            source.as_mut().enable();
            wake.as_mut().enable();
            if self.check(&inner.wake).is_err() {
                return;
            }
            tokio::select! { _ = source => {}, _ = wake => {} }
        }
    }
}
#[derive(Clone)]
struct AccessEntry {
    token: Arc<SecretText>,
    origin: GoogleLeaseOrigin,
    issued: GoogleNow,
    expires_at_ms: i64,
    expires_at_monotonic: Instant,
    fence: Fence,
    network_policy: GoogleNetworkPolicy,
}
fn clock_elapsed_consistent(start: GoogleNow, now: GoogleNow) -> bool {
    let Some(elapsed) = now
        .monotonic
        .checked_duration_since(start.monotonic)
        .and_then(|value| i64::try_from(value.as_millis()).ok())
    else {
        return false;
    };
    let Some(wall) = now.unix_ms.checked_sub(start.unix_ms) else {
        return false;
    };
    wall >= 0 && wall.abs_diff(elapsed) <= MAX_CLOCK_DRIFT_MS as u64
}
impl AccessEntry {
    fn current(&self, now: GoogleNow, wake: &AtomicU64, min_remaining: i64) -> bool {
        clock_elapsed_consistent(self.issued, now)
            && self.expires_at_ms.saturating_sub(now.unix_ms) > min_remaining
            && self
                .expires_at_monotonic
                .checked_duration_since(now.monotonic)
                .is_some_and(|remaining| remaining.as_millis() > min_remaining as u128)
            && self.fence.check(wake).is_ok()
    }
}
struct Inner {
    handler_owner: Arc<()>,
    repository: Arc<dyn GoogleAuthorizationRepository>,
    vault: Arc<dyn CredentialVault>,
    transport: Arc<dyn GoogleTokenTransport>,
    clock: Arc<dyn GoogleClock>,
    test_operations: Arc<dyn GoogleTestOperationAuthority>,
    cache: Mutex<BTreeMap<String, AccessEntry>>,
    flights: Mutex<BTreeMap<String, Weak<tokio::sync::Mutex<()>>>>,
    fences: Mutex<BTreeMap<String, Weak<SourceFence>>>,
    wake: AtomicU64,
    wake_changed: tokio::sync::Notify,
    logins: Mutex<BTreeSet<String>>,
}
#[derive(Clone)]
pub struct GoogleAuthorizationService {
    inner: Arc<Inner>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleLeasePersistence {
    RecordedAttempt,
    DeferredTerminalTest,
}
/// No Serialize/TS or plaintext Debug. A wire borrow rechecks actual issued
/// authority, cancellation, expiry, source fence and current in-memory test.
pub struct GoogleAccessLease {
    inner: Arc<Inner>,
    record: GoogleExecutionLeaseRecord,
    entry: AccessEntry,
    budget: GoogleBudget,
    persistence: GoogleLeasePersistence,
}
impl std::fmt::Debug for GoogleAccessLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleAccessLease([REDACTED])")
    }
}
impl GoogleAccessLease {
    /// The wire owner selects this against its HTTP future. This listens to
    /// provider revoke/wake and the calling budget; repository lifecycle/control
    /// changes must also cancel the caller's shared runtime budget. It does not
    /// replace the mandatory provenance checks before and after wire execution.
    pub async fn invalidated(&self) -> AppError {
        let expiry = self.entry.expires_at_monotonic;
        tokio::select! {
            biased;
            _ = self.entry.fence.changed(&self.inner) => failure("google_authority_changed", "授权或休眠状态已变化，请重新核验"),
            _ = self.budget.cancelled() => failure("authorization_cancelled", "授权操作已取消"),
            _ = tokio::time::sleep_until(self.budget.deadline().into()) => failure("authorization_timeout", "授权操作超过时间预算"),
            _ = tokio::time::sleep_until(expiry.into()) => failure("google_access_expired", "短期授权已到期，请重新获取"),
        }
    }
    pub fn record(&self) -> &GoogleExecutionLeaseRecord {
        &self.record
    }
    pub fn persistence(&self) -> GoogleLeasePersistence {
        self.persistence
    }
    pub fn check_current_provenance(&self) -> AppResult<()> {
        self.budget.check()?;
        self.entry.fence.check(&self.inner.wake)?;
        check_test(&self.inner, &self.record.request)?;
        self.inner.repository.check_issued_lease(&self.record)?;
        self.entry.fence.check(&self.inner.wake)?;
        self.budget.check()
    }
    pub fn authorization(&self) -> AppResult<ProviderAuthLease<'_>> {
        self.check_current_provenance()?;
        if !self
            .entry
            .current(current_now(self.inner.clock.as_ref())?, &self.inner.wake, 0)
        {
            return Err(failure(
                "google_access_expired",
                "短期授权已到期或时钟变化，请重新获取",
            ));
        }
        Ok(match &self.record.origin {
            GoogleLeaseOrigin::ServiceAccount(identity) => {
                ProviderAuthLease::GoogleServiceAccountAccess {
                    token: &self.entry.token,
                    identity,
                }
            }
            GoogleLeaseOrigin::UserOAuth { identity, .. } => ProviderAuthLease::GoogleAccess {
                token: &self.entry.token,
                identity,
            },
        })
    }
}
fn check_test(inner: &Inner, request: &GoogleLeaseRequest) -> AppResult<()> {
    if let GoogleLeaseTarget::TestOperation(operation) = &request.target {
        inner.test_operations.check(request, operation)?;
    }
    Ok(())
}
fn source_key(origin: &GoogleLeaseOrigin) -> String {
    match origin {
        GoogleLeaseOrigin::ServiceAccount(s) => format!(
            "sa\0{}\0{}",
            s.credential_version.credential_id, s.credential_version.credential_version_id
        ),
        GoogleLeaseOrigin::UserOAuth { identity, .. } => format!("oauth\0{}", identity.grant_id),
    }
}
fn cache_key(authority: &GoogleAuthoritySnapshot) -> String {
    let r = &authority.request;
    let mut parts = vec![
        r.profile_generation.to_string(),
        r.server_instance_id.clone(),
        r.config_revision_id.clone(),
        r.connection_id.clone(),
        r.target_policy_sha256.clone(),
        authority.network_policy.cache_identity(),
    ];
    match &authority.origin {
        GoogleLeaseOrigin::ServiceAccount(s) => parts.extend([
            "sa".into(),
            s.pool_id.clone(),
            s.pool_revision.to_string(),
            s.member_id.clone(),
            s.credential_version.credential_id.clone(),
            s.credential_version.credential_version_id.clone(),
            s.lifecycle_revision.to_string(),
            s.scopes_hash.clone(),
        ]),
        // A normal refresh is not a new authorization. Actual old mint material
        // is still revalidated on every hit; no token is relabelled as new.
        GoogleLeaseOrigin::UserOAuth {
            identity, client, ..
        } => parts.extend([
            "oauth".into(),
            identity.grant_id.clone(),
            identity.authorization_epoch.to_string(),
            identity.scopes_hash.clone(),
            client.key.client_profile_id.clone(),
            client.key.client_profile_revision.to_string(),
            client.policy_sha256.clone(),
        ]),
    }
    parts.join("\0")
}
/// Refresh rotation belongs to an authorization source, not one request target.
/// Distinct frozen targets keep separate caches but must not refresh the same
/// owned grant concurrently and race its material CAS.
fn refresh_key(authority: &GoogleAuthoritySnapshot) -> String {
    let mut parts = vec![
        authority.request.profile_generation.to_string(),
        authority.request.server_instance_id.clone(),
        source_key(&authority.origin),
    ];
    match &authority.origin {
        GoogleLeaseOrigin::ServiceAccount(s) => {
            parts.extend([s.lifecycle_revision.to_string(), s.scopes_hash.clone()])
        }
        GoogleLeaseOrigin::UserOAuth {
            identity, client, ..
        } => parts.extend([
            identity.authorization_epoch.to_string(),
            identity.scopes_hash.clone(),
            client.key.client_profile_id.clone(),
            client.key.client_profile_revision.to_string(),
            client.policy_sha256.clone(),
        ]),
    }
    parts.join("\0")
}
fn control_request(
    cursor: &GoogleGrantCursor,
    generation: std::num::NonZeroU32,
    server: &str,
    network_policy: &GoogleNetworkPolicy,
) -> GoogleGrantControlRequest {
    GoogleGrantControlRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected: cursor.clone(),
        profile_generation: generation,
        server_instance_id: server.into(),
        network_policy: network_policy.clone(),
    }
}
impl GoogleAuthorizationService {
    pub fn new(
        repository: Arc<dyn GoogleAuthorizationRepository>,
        vault: Arc<dyn CredentialVault>,
        transport: Arc<dyn GoogleTokenTransport>,
        clock: Arc<dyn GoogleClock>,
        test_operations: Arc<dyn GoogleTestOperationAuthority>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                handler_owner: Arc::new(()),
                repository,
                vault,
                transport,
                clock,
                test_operations,
                cache: Mutex::new(BTreeMap::new()),
                flights: Mutex::new(BTreeMap::new()),
                fences: Mutex::new(BTreeMap::new()),
                wake: AtomicU64::new(0),
                wake_changed: tokio::sync::Notify::new(),
                logins: Mutex::new(BTreeSet::new()),
            }),
        }
    }
    /// Composition-only registration through this Broker's actual repository.
    /// A constructed Broker alone never grants daily selection eligibility.
    pub(super) fn register_handler(&self, destination: GoogleRuntimeFrame) -> AppResult<GoogleHandlerHandle> {
        self.inner.repository.register_google_handler(destination, Arc::downgrade(&self.inner.handler_owner))
    }
    /// Settings uses the very registration held by the installed Gateway,
    /// including this Broker owner, rather than mere compiled availability.
    pub fn check_interactive_handler(&self, registration: &GoogleHandlerRegistration) -> AppResult<()> {
        if !registration.is_live() || !registration.same_owner(&Arc::downgrade(&self.inner.handler_owner)) {
            return Err(failure("google_handler_owner_mismatch", "Google授权处理器不属于当前后端"));
        }
        Ok(())
    }
    /// Only the provisional, exact newly committed cursor is invalidated. Used
    /// when cancellation wins between finish() and the Core publication fence.
    /// No network/Vault work and no mutation of a previous/shared grant.
    pub fn invalidate_unpublished_login(&self, request: GoogleGrantControlRequest) -> AppResult<()> {
        request.validate()?;
        self.inner.repository.invalidate_grant(&request,GoogleGrantInvalidation::NeedsReauth,current_now(self.inner.clock.as_ref())?.unix_ms)?;
        self.invalidate_origin(&format!("oauth\0{}",request.expected.grant_id))
    }
    /// Native wake events invalidate all cached/in-flight time assumptions.
    pub fn on_wake(&self) -> AppResult<()> {
        self.inner.wake.fetch_add(1, Ordering::SeqCst);
        self.inner.wake_changed.notify_waiters();
        locked(&self.inner.cache)?.clear();
        Ok(())
    }
    fn fence(&self, origin: &GoogleLeaseOrigin) -> AppResult<Fence> {
        let key = source_key(origin);
        let mut fences = locked(&self.inner.fences)?;
        fences.retain(|_, v| v.strong_count() > 0);
        let source = if let Some(source) = fences.get(&key).and_then(Weak::upgrade) {
            source
        } else {
            if fences.len() >= MAX_ACTIVE_KEYS {
                return Err(failure(
                    "google_authorization_busy",
                    "并发授权过多，请稍后重试",
                ));
            }
            let value = Arc::new(SourceFence {
                revision: AtomicU64::new(0),
                changed: tokio::sync::Notify::new(),
            });
            fences.insert(key, Arc::downgrade(&value));
            value
        };
        Ok(Fence {
            revision: source.revision.load(Ordering::SeqCst),
            source,
            wake: self.inner.wake.load(Ordering::SeqCst),
        })
    }
    fn invalidate_origin(&self, key: &str) -> AppResult<()> {
        if let Some(value) = locked(&self.inner.fences)?.get(key).and_then(Weak::upgrade) {
            value.revision.fetch_add(1, Ordering::SeqCst);
            value.changed.notify_waiters();
        }
        locked(&self.inner.cache)?.retain(|_, entry| source_key(&entry.origin) != key);
        Ok(())
    }
    fn flight(&self, key: &str) -> AppResult<Arc<tokio::sync::Mutex<()>>> {
        let mut flights = locked(&self.inner.flights)?;
        flights.retain(|_, v| v.strong_count() > 0);
        if let Some(flight) = flights.get(key).and_then(Weak::upgrade) {
            return Ok(flight);
        }
        if flights.len() >= MAX_ACTIVE_KEYS {
            return Err(failure(
                "google_authorization_busy",
                "并发授权过多，请稍后重试",
            ));
        }
        let flight = Arc::new(tokio::sync::Mutex::new(()));
        flights.insert(key.into(), Arc::downgrade(&flight));
        Ok(flight)
    }
    fn current(
        &self,
        request: &GoogleLeaseRequest,
        budget: &GoogleBudget,
    ) -> AppResult<GoogleAuthoritySnapshot> {
        budget.check()?;
        request.validate()?;
        check_test(&self.inner, request)?;
        let authority = self.inner.repository.lease_authority(request)?;
        authority.validate()?;
        validate_google_network_policy(&authority.network_policy)?;
        if &authority.request != request {
            return Err(failure(
                "google_target_mismatch",
                "授权来源不属于本次实际目标",
            ));
        }
        self.inner.repository.check_lease_authority(&authority)?;
        budget.check()?;
        Ok(authority)
    }
    fn strict(&self, authority: &GoogleAuthoritySnapshot, budget: &GoogleBudget) -> AppResult<()> {
        budget.check()?;
        check_test(&self.inner, &authority.request)?;
        self.inner.repository.check_lease_authority(authority)?;
        budget.check()
    }
    async fn exchange_authority(
        &self,
        request: GoogleTokenRequest<'_>,
        authority: &GoogleAuthoritySnapshot,
        budget: &GoogleBudget,
        fence: &Fence,
    ) -> AppResult<GoogleTokenReply> {
        tokio::select! {
            biased;
            _ = fence.changed(&self.inner) => Err(failure("google_authority_changed", "授权或休眠状态已变化，请重新核验")),
            result = within_budget(budget, self.inner.transport.exchange(request, &authority.network_policy, budget)) => result,
        }
    }
    pub async fn lease(
        &self,
        request: GoogleLeaseRequest,
        budget: GoogleBudget,
    ) -> AppResult<GoogleAccessLease> {
        within_budget(&budget, self.lease_inner(request, &budget)).await
    }
    async fn lease_inner(
        &self,
        request: GoogleLeaseRequest,
        budget: &GoogleBudget,
    ) -> AppResult<GoogleAccessLease> {
        let first = self.current(&request, budget)?;
        let key = cache_key(&first);
        let flight = self.flight(&refresh_key(&first))?;
        let _flight = within_budget(budget, async { Ok(flight.lock().await) }).await?;
        let authority = self.current(&request, budget)?;
        if cache_key(&authority) != key {
            return Err(failure(
                "google_authority_changed",
                "等待期间授权来源已变化，请重试",
            ));
        }
        let cached = { locked(&self.inner.cache)?.get(&key).cloned() };
        if let Some(entry) = cached {
            if entry.current(
                current_now(self.inner.clock.as_ref())?,
                &self.inner.wake,
                CACHE_SKEW_MS,
            ) {
                let outcome = self.issue(&request, entry, budget);
                if outcome.is_err() {
                    locked(&self.inner.cache)?.remove(&key);
                }
                return outcome;
            }
            locked(&self.inner.cache)?.remove(&key);
        }
        let fence = self.fence(&authority.origin)?;
        self.strict(&authority, budget)?;
        let material = decode_vault_credential(
            authority.material.storage,
            self.inner.vault.get(&authority.material.vault_ref)?,
        )?;
        self.strict(&authority, budget)?;
        fence.check(&self.inner.wake)?;
        let started = current_now(self.inner.clock.as_ref())?;
        let response = match (&authority.origin, &material) {
            (
                GoogleLeaseOrigin::ServiceAccount(_),
                CredentialMaterial::GoogleServiceAccount { .. },
            ) => {
                let when = UNIX_EPOCH
                    .checked_add(Duration::from_millis(
                        u64::try_from(started.unix_ms)
                            .map_err(|_| failure("authorization_clock_invalid", "授权时间无效"))?,
                    ))
                    .ok_or_else(|| failure("authorization_clock_invalid", "授权时间无效"))?;
                let assertion = service_account_assertion(&material, when)?;
                self.exchange_authority(
                    GoogleTokenRequest::ServiceAccount {
                        assertion: &assertion,
                    },
                    &authority,
                    budget,
                    &fence,
                )
                .await
            }
            (
                GoogleLeaseOrigin::UserOAuth { client, .. },
                CredentialMaterial::GoogleOauthRefresh { refresh_token },
            ) => {
                self.exchange_authority(
                    GoogleTokenRequest::Refresh {
                        client_id: &client.public_client_id,
                        refresh_token,
                    },
                    &authority,
                    budget,
                    &fence,
                )
                .await
            }
            _ => {
                return Err(failure(
                    "google_material_kind_mismatch",
                    "授权材料种类与当前来源不符",
                ));
            }
        };
        let response = match response {
            Ok(GoogleTokenReply::Tokens(value)) => value,
            Ok(GoogleTokenReply::Revoked) => {
                return Err(failure("google_token_malformed", "授权服务未返回短期材料"));
            }
            Err(error) => {
                if matches!(
                    error.code.as_str(),
                    "google_invalid_grant" | "google_scope_mismatch"
                )
                    && let Some(cursor) = authority.origin.cursor() {
                        let request = control_request(
                            &cursor,
                            authority.request.profile_generation,
                            &authority.request.server_instance_id,
                            &authority.network_policy,
                        );
                        self.inner.repository.invalidate_grant(
                            &request,
                            GoogleGrantInvalidation::NeedsReauth,
                            current_now(self.inner.clock.as_ref())?.unix_ms,
                        )?;
                        self.invalidate_origin(&source_key(&authority.origin))?;
                    }
                return Err(error);
            }
        };
        budget.check()?;
        fence.check(&self.inner.wake)?;
        let GoogleTokenResponse {
            access_token,
            expires_in_seconds,
            refresh_token,
        } = response;
        if expires_in_seconds == 0
            || u64::from(expires_in_seconds) > MAX_ACCESS_LIFETIME_SECONDS
            || access_token
                .expose()
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
        {
            return Err(failure("google_token_malformed", "短期授权数据无效"));
        }
        // Audit the first locally observed successful token response. This is
        // not an unreported server mint timestamp, nor a later cache borrow.
        let observed = current_now(self.inner.clock.as_ref())?;
        if !clock_elapsed_consistent(started, observed) {
            return Err(failure(
                "authorization_clock_changed",
                "换取授权期间系统时钟发生变化",
            ));
        }
        // Keep both deadlines anchored to the original request start. Moving
        // the audit observation later must never extend token availability.
        let expires_at_ms = started
            .unix_ms
            .checked_add(i64::from(expires_in_seconds) * 1000)
            .ok_or_else(|| failure("google_token_expiry_invalid", "授权有效时间溢出"))?;
        let expires_at_monotonic = started
            .monotonic
            .checked_add(Duration::from_secs(u64::from(expires_in_seconds)))
            .ok_or_else(|| failure("google_token_expiry_invalid", "授权有效时间溢出"))?;
        if expires_at_ms <= observed.unix_ms || expires_at_monotonic <= observed.monotonic {
            return Err(failure("google_access_expired", "响应到达时短期授权已到期"));
        }
        if let Some(refresh) = refresh_token {
            match &authority.origin {
                GoogleLeaseOrigin::UserOAuth { .. } => {
                    self.strict(&authority, budget)?;
                    let context = GoogleMaterialContext::Refresh {
                        operation_id: uuid::Uuid::new_v4().to_string(),
                        request: request.clone(),
                        expected: authority
                            .origin
                            .cursor()
                            .ok_or_else(|| failure("google_grant_missing", "刷新授权来源缺失"))?,
                        mint_material: authority.material.clone(),
                        network_policy: authority.network_policy.clone(),
                    };
                    self.persist_material(context, refresh, budget)?;
                }
                GoogleLeaseOrigin::ServiceAccount(_) => {
                    return Err(failure(
                        "google_token_malformed",
                        "服务账号不能返回用户刷新材料",
                    ));
                }
            }
        }
        let entry = AccessEntry {
            token: Arc::new(access_token),
            origin: authority.origin.clone(),
            issued: observed,
            expires_at_ms,
            expires_at_monotonic,
            fence,
            network_policy: authority.network_policy.clone(),
        };
        let lease = self.issue(&request, entry.clone(), budget)?;
        let mut cache = locked(&self.inner.cache)?;
        if cache.len() >= MAX_CACHE_ENTRIES
            && let Some(old) = cache.keys().next().cloned() {
                cache.remove(&old);
            }
        cache.insert(key, entry);
        Ok(lease)
    }
    fn issue(
        &self,
        request: &GoogleLeaseRequest,
        entry: AccessEntry,
        budget: &GoogleBudget,
    ) -> AppResult<GoogleAccessLease> {
        let now = current_now(self.inner.clock.as_ref())?;
        if !entry.current(now, &self.inner.wake, 0) {
            return Err(failure(
                "google_access_expired",
                "短期授权到期或时钟发生变化",
            ));
        }
        let record = GoogleExecutionLeaseRecord {
            lease_id: uuid::Uuid::new_v4().to_string(),
            request: request.clone(),
            origin: entry.origin.clone(),
            minted_at_ms: entry.issued.unix_ms,
            expires_at_ms: entry.expires_at_ms,
            network_policy: entry.network_policy.clone(),
        };
        record.validate()?;
        let persistence = match request.target {
            GoogleLeaseTarget::Attempt { .. } => GoogleLeasePersistence::RecordedAttempt,
            GoogleLeaseTarget::TestOperation(_) => GoogleLeasePersistence::DeferredTerminalTest,
        };
        let lease = GoogleAccessLease {
            inner: self.inner.clone(),
            record,
            entry,
            budget: budget.clone(),
            persistence,
        };
        lease.check_current_provenance()?;
        if persistence == GoogleLeasePersistence::RecordedAttempt {
            self.inner.repository.record_attempt_lease(&lease.record)?;
        }
        lease.check_current_provenance()?;
        Ok(lease)
    }
    fn check_material_context(
        &self,
        context: &GoogleMaterialContext,
        budget: &GoogleBudget,
    ) -> AppResult<()> {
        context.validate()?;
        budget.check()?;
        match context {
            GoogleMaterialContext::InitialLogin(pending) => {
                self.inner.repository.check_login(pending)?
            }
            GoogleMaterialContext::Refresh {
                request,
                expected,
                mint_material,
                network_policy,
                ..
            } => {
                let current = self.current(request, budget)?;
                if current.origin.cursor().as_ref() != Some(expected)
                    || &current.material != mint_material
                    || &current.network_policy != network_policy
                {
                    return Err(failure(
                        "google_material_cursor_mismatch",
                        "刷新期间授权材料或生命周期已变化",
                    ));
                }
            }
        }
        budget.check()
    }
    fn persist_material(
        &self,
        context: GoogleMaterialContext,
        refresh_token: SecretText,
        budget: &GoogleBudget,
    ) -> AppResult<GoogleOwnedGrant> {
        self.check_material_context(&context, budget)?;
        let previous_grant = if let GoogleMaterialContext::Refresh {
            operation_id,
            request,
            expected,
            mint_material,
            network_policy,
        } = &context
        {
            expected
                .control_revision
                .checked_add(1)
                .ok_or_else(|| failure("google_grant_revision_exhausted", "授权版本已达上限"))?;
            let control = GoogleGrantControlRequest {
                operation_id: operation_id.clone(),
                expected: expected.clone(),
                profile_generation: request.profile_generation,
                server_instance_id: request.server_instance_id.clone(),
                network_policy: network_policy.clone(),
            };
            let grant = self.inner.repository.owned_grant(&control)?;
            grant.validate()?;
            if grant.cursor != *expected
                || grant.state != GoogleGrantState::Active
                || grant.material.as_ref() != Some(mint_material)
            {
                return Err(failure(
                    "google_material_cursor_mismatch",
                    "刷新来源与当前授权材料不符",
                ));
            }
            Some(grant)
        } else {
            None
        };
        let pending = self
            .inner
            .repository
            .reserve_material(&context, current_now(self.inner.clock.as_ref())?.unix_ms)?;
        if pending.context != context || pending.allocation_id.is_empty() {
            return Err(failure(
                "google_pending_material_mismatch",
                "秘密材料预留不属于本次操作",
            ));
        }
        pending.material.validate_kind(
            app_core::domain::model_config::credentials::CredentialMaterialKind::GoogleOauthRefresh,
        )?;
        if let GoogleMaterialContext::Refresh { mint_material, .. } = &context
            && (pending.material.credential.credential_id == mint_material.credential.credential_id
                || pending.material.credential.credential_version_id
                    == mint_material.credential.credential_version_id
                || pending.material.vault_ref == mint_material.vault_ref)
            {
                return Err(failure(
                    "google_material_not_immutable",
                    "刷新不能覆盖原秘密材料版本",
                ));
            }
        let transition = match &context {
            GoogleMaterialContext::InitialLogin(_) => GoogleMaterialTransition::InitialLogin,
            GoogleMaterialContext::Refresh { .. } => GoogleMaterialTransition::Refresh,
        };
        let material = CredentialMaterial::GoogleOauthRefresh { refresh_token };
        let bytes = encode_vault_credential(&material)?;
        let committed = (|| {
            self.inner
                .vault
                .put(&pending.material.vault_ref, bytes.expose())?;
            self.check_material_context(&context, budget)?;
            self.inner
                .repository
                .commit_material(&GoogleMaterialCommit {
                    pending: pending.clone(),
                    transition,
                    scopes_sha256: google_scope_hash(),
                    committed_at_ms: current_now(self.inner.clock.as_ref())?.unix_ms,
                })
        })();
        let grant = match committed {
            Ok(value) => value,
            Err(error) => {
                if let Ok(now) = current_now(self.inner.clock.as_ref())
                    && self
                        .inner
                        .repository
                        .release_failed_material(&pending, now.unix_ms)
                        .unwrap_or(false)
                    {
                        let _ = self.inner.vault.delete(&pending.material.vault_ref);
                    }
                return Err(error);
            }
        };
        grant.validate()?;
        if grant.cursor.grant_id != context.expected().grant_id
            || grant.state != GoogleGrantState::Active
            || grant
                .material
                .as_ref()
                .map(|m| (&m.credential, &m.vault_ref, m.storage))
                != Some((
                    &pending.material.credential,
                    &pending.material.vault_ref,
                    pending.material.storage,
                ))
        {
            return Err(failure(
                "google_material_commit_mismatch",
                "已保存的授权材料与本次预留不符",
            ));
        }
        match &context {
            GoogleMaterialContext::InitialLogin(p) => {
                if grant.client != p.grant.client
                    || grant.principal_state != p.grant.principal_state
                    || grant.cursor.authorization_epoch <= p.grant.cursor.authorization_epoch
                    || grant.cursor.control_revision
                        != p.grant
                            .cursor
                            .control_revision
                            .checked_add(1)
                            .ok_or_else(|| {
                                failure("google_grant_revision_exhausted", "授权版本已达上限")
                            })?
                {
                    return Err(failure(
                        "google_grant_identity_changed",
                        "登录返回的授权身份不符",
                    ));
                }
            }
            GoogleMaterialContext::Refresh { expected, .. } => {
                if grant.cursor.authorization_epoch != expected.authorization_epoch
                    || grant.cursor.material_revision <= expected.material_revision
                    || grant.cursor.control_revision
                        != expected.control_revision.checked_add(1).ok_or_else(|| {
                            failure("google_grant_revision_exhausted", "授权版本已达上限")
                        })?
                {
                    return Err(failure(
                        "google_grant_identity_changed",
                        "刷新不能改变授权范围或主体",
                    ));
                }
                if previous_grant.as_ref().is_none_or(|old| {
                    old.client != grant.client || old.principal_state != grant.principal_state
                }) {
                    return Err(failure(
                        "google_grant_identity_changed",
                        "刷新不能更换授权主体或客户端",
                    ));
                }
            }
        }
        if let Err(error) = budget.check() {
            if let GoogleMaterialContext::InitialLogin(p) = &context {
                let control = control_request(
                    &grant.cursor,
                    p.request.profile_generation,
                    &p.request.server_instance_id,
                    &p.request.network_policy,
                );
                let invalidated = self.inner.repository.invalidate_grant(
                    &control,
                    GoogleGrantInvalidation::NeedsReauth,
                    current_now(self.inner.clock.as_ref())?.unix_ms,
                );
                self.invalidate_origin(&format!("oauth\0{}", grant.cursor.grant_id))?;
                if invalidated.is_err() {
                    return Err(failure(
                        "google_cleanup_pending",
                        "本次登录已取消，授权清理仍待恢复",
                    ));
                }
            }
            return Err(error);
        }
        Ok(grant)
    }
    /// Local epoch invalidation occurs before remote revoke. Network failure
    /// never re-enables local authority or claims remote revocation succeeded.
    pub async fn revoke(
        &self,
        request: GoogleGrantControlRequest,
        budget: GoogleBudget,
    ) -> AppResult<()> {
        request.validate()?;
        budget.check()?;
        let grant = self.inner.repository.owned_grant(&request)?;
        grant.validate()?;
        if grant.cursor != request.expected {
            return Err(failure("google_grant_cursor_stale", "授权状态已变化"));
        }
        self.inner.repository.invalidate_grant(
            &request,
            GoogleGrantInvalidation::Revoked,
            current_now(self.inner.clock.as_ref())?.unix_ms,
        )?;
        self.invalidate_origin(&format!("oauth\0{}", request.expected.grant_id))?;
        validate_google_network_policy(&request.network_policy)?;
        let Some(material) = grant.material else {
            return Ok(());
        };
        let material =
            decode_vault_credential(material.storage, self.inner.vault.get(&material.vault_ref)?)?;
        let CredentialMaterial::GoogleOauthRefresh { refresh_token } = material else {
            return Err(failure(
                "google_material_kind_mismatch",
                "撤权材料不属于用户授权",
            ));
        };
        match within_budget(
            &budget,
            self.inner.transport.exchange(
                GoogleTokenRequest::Revoke {
                    token: &refresh_token,
                },
                &request.network_policy,
                &budget,
            ),
        )
        .await?
        {
            GoogleTokenReply::Revoked => Ok(()),
            _ => Err(failure(
                "google_revoke_unconfirmed",
                "远端撤权尚未确认，本机保持停用",
            )),
        }
    }
    pub async fn begin_login(
        &self,
        request: GoogleLoginRequest,
        budget: GoogleBudget,
    ) -> AppResult<GoogleLoginSession> {
        request.validate()?;
        budget.check()?;
        let now = current_now(self.inner.clock.as_ref())?.unix_ms;
        let key = format!(
            "{}\0{}\0{}",
            request.profile_generation, request.server_instance_id, request.operation_id
        );
        {
            let mut logins = locked(&self.inner.logins)?;
            if logins.len() >= 32 || !logins.insert(key.clone()) {
                return Err(failure(
                    "google_login_in_progress",
                    "本次登录已开始或登录会话过多",
                ));
            }
        }
        let pending = match self.inner.repository.prepare_login(&request, now) {
            Ok(value) => value,
            Err(error) => {
                locked(&self.inner.logins)?.remove(&key);
                return Err(error);
            }
        };
        if pending.request != request {
            locked(&self.inner.logins)?.remove(&key);
            return Err(failure(
                "google_login_identity_mismatch",
                "登录来源不属于本次profile操作",
            ));
        }
        if let Err(error) = pending.validate() {
            locked(&self.inner.logins)?.remove(&key);
            return Err(error);
        }
        let mut session = GoogleLoginSession {
            service: self.clone(),
            flow: None,
            pending: pending.clone(),
            budget: budget.clone(),
            key,
            completed: false,
        };
        self.inner.repository.check_login(&pending)?;
        validate_google_network_policy(&pending.request.network_policy)?;
        session.flow = Some(GoogleOAuthFlow::bind(pending, budget).await?);
        Ok(session)
    }
    async fn complete_login(&self, callback: GoogleOAuthCallback) -> AppResult<GoogleOwnedGrant> {
        callback.pending.validate()?;
        callback.budget.check()?;
        self.inner.repository.check_login(&callback.pending)?;
        let reply = within_budget(
            &callback.budget,
            self.inner.transport.exchange(
                GoogleTokenRequest::AuthorizationCode {
                    client_id: &callback.pending.grant.client.public_client_id,
                    code: &callback.code,
                    verifier: &callback.verifier,
                    redirect_uri: &callback.redirect_uri,
                },
                &callback.pending.request.network_policy,
                &callback.budget,
            ),
        )
        .await?;
        callback.budget.check()?;
        self.inner.repository.check_login(&callback.pending)?;
        let GoogleTokenReply::Tokens(response) = reply else {
            return Err(failure("google_token_malformed", "登录未返回授权材料"));
        };
        let refresh = response.refresh_token.ok_or_else(|| {
            failure(
                "google_refresh_token_missing",
                "本次登录未返回可保存的刷新授权，请重新同意授权",
            )
        })?;
        self.persist_material(
            GoogleMaterialContext::InitialLogin(callback.pending),
            refresh,
            &callback.budget,
        )
    }
}
pub struct GoogleLoginSession {
    service: GoogleAuthorizationService,
    flow: Option<GoogleOAuthFlow>,
    pending: GooglePendingLogin,
    budget: GoogleBudget,
    key: String,
    completed: bool,
}
impl std::fmt::Debug for GoogleLoginSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleLoginSession([operation-owned])")
    }
}
impl GoogleLoginSession {
    pub fn intent(&self) -> &GoogleBrowserIntent {
        self.flow
            .as_ref()
            .expect("owned unfinished login flow")
            .intent()
    }
    pub fn cancel(self) {
        self.budget.cancel();
    }
    pub async fn finish(mut self) -> AppResult<GoogleOwnedGrant> {
        let flow = self
            .flow
            .take()
            .ok_or_else(|| failure("google_login_finished", "登录会话已结束"))?;
        let callback = flow.wait_for_callback().await?;
        let grant = self.service.complete_login(callback).await?;
        self.completed = true;
        Ok(grant)
    }
}
impl Drop for GoogleLoginSession {
    fn drop(&mut self) {
        if !self.completed
            && let Ok(now) = current_now(self.service.inner.clock.as_ref()) {
                let request = control_request(
                    &self.pending.grant.cursor,
                    self.pending.request.profile_generation,
                    &self.pending.request.server_instance_id,
                    &self.pending.request.network_policy,
                );
                if self
                    .service
                    .inner
                    .repository
                    .invalidate_grant(&request, GoogleGrantInvalidation::NeedsReauth, now.unix_ms)
                    .is_ok()
                {
                    let _ = self
                        .service
                        .invalidate_origin(&format!("oauth\0{}", request.expected.grant_id));
                }
            }
        if let Ok(mut logins) = self.service.inner.logins.lock() {
            logins.remove(&self.key);
        }
    }
}
