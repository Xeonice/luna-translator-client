//! Controlled-Repo/Vault orchestration checks. These are NOT real SQLite,
//! Google authorization, 740 storage markers, Native or product acceptance.
use app_core::{
    contracts::{AppError, AppResult},
    domain::model_config::credentials::*,
    ports::{
        google_authorization::*,
        runtime::{CredentialVault, PortFuture},
    },
};
use app_infra::providers::{
    google_auth::import_service_account, google_authorization::*, google_token::*,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroU32,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn err(code: &str) -> AppError {
    AppError::new(
        code,
        "controlled_google_fixture",
        "synthetic controlled failure",
    )
}
fn client() -> GoogleClientProfile {
    GoogleClientProfile {
        key: GoogleClientKey {
            client_profile_id: "synthetic-client-profile".into(),
            client_profile_revision: nz(1),
        },
        public_client_id: "synthetic-desktop-client".into(),
        client_mode: GoogleClientMode::Custom,
        policy_sha256: "a".repeat(64),
        setup_state: GoogleClientSetup::Verified,
    }
}
fn material(n: u32, kind: CredentialMaterialKind) -> GoogleVaultMaterial {
    GoogleVaultMaterial {
        credential: CredentialVersionRef {
            credential_id: format!("credential-{n}"),
            credential_version_id: format!("version-{n}"),
        },
        vault_ref: format!("vault-{n}"),
        storage: CredentialStorageMetadata::EnvelopeV1 { kind },
        lifecycle_revision: 1,
    }
}
fn request(n: u32) -> GoogleLeaseRequest {
    GoogleLeaseRequest {
        target: GoogleLeaseTarget::Attempt {
            attempt_id: format!("synthetic-attempt-{n}"),
            task_id: format!("synthetic-task-{n}"),
        },
        config_revision_id: "synthetic-config".into(),
        connection_id: "synthetic-connection".into(),
        target_policy_sha256: "b".repeat(64),
        profile_generation: nz(1),
        server_instance_id: "synthetic-server".into(),
    }
}
fn login_request() -> GoogleLoginRequest {
    GoogleLoginRequest {
        operation_id: "synthetic-login-operation".into(),
        client: client().key,
        profile_generation: nz(1),
        server_instance_id: "synthetic-server".into(),
        network_policy: GoogleNetworkPolicy::Inherit,
    }
}
fn control(cursor: GoogleGrantCursor) -> GoogleGrantControlRequest {
    GoogleGrantControlRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected: cursor,
        profile_generation: nz(1),
        server_instance_id: "synthetic-server".into(),
        network_policy: GoogleNetworkPolicy::Inherit,
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    OAuth,
    ServiceAccount,
    Pending,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    CommitFail,
    LostCommitReply,
    OldVault,
    WrongContext,
}
struct State {
    mode: Mode,
    grant: GoogleOwnedGrant,
    history: BTreeMap<u32, GoogleVaultMaterial>,
    inactive: BTreeSet<String>,
    admitted: BTreeMap<String, GoogleLeaseRequest>,
    pending: BTreeMap<String, GooglePendingMaterial>,
    fault: Fault,
    next: u32,
    profile_alive: bool,
    login: Option<GooglePendingLogin>,
    records: Vec<GoogleExecutionLeaseRecord>,
    network_policy: GoogleNetworkPolicy,
}
fn target_key(r: &GoogleLeaseRequest) -> String {
    format!("{:?}", r.target)
}
struct ControlledRepo {
    state: Mutex<State>,
    checks: AtomicUsize,
    events: Arc<Mutex<Vec<String>>>,
    cancel_after_commit: Mutex<Option<GoogleBudget>>,
}
impl ControlledRepo {
    fn new(mode: Mode, events: Arc<Mutex<Vec<String>>>) -> Self {
        let m = material(
            1,
            if mode == Mode::ServiceAccount {
                CredentialMaterialKind::GoogleServiceAccount
            } else {
                CredentialMaterialKind::GoogleOauthRefresh
            },
        );
        let pending = mode == Mode::Pending;
        let grant = GoogleOwnedGrant {
            cursor: GoogleGrantCursor {
                grant_id: "synthetic-owned-grant".into(),
                authorization_epoch: if pending { None } else { Some(nz(1)) },
                material_revision: if pending { None } else { Some(nz(1)) },
                control_revision: if pending { 0 } else { 1 },
            },
            client: client(),
            state: if pending {
                GoogleGrantState::Pending
            } else {
                GoogleGrantState::Active
            },
            scopes_sha256: google_scope_hash(),
            principal_state: PrincipalState::Unknown,
            material: if pending { None } else { Some(m.clone()) },
        };
        Self {
            state: Mutex::new(State {
                mode,
                grant,
                history: if pending {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(1, m)])
                },
                inactive: BTreeSet::new(),
                admitted: BTreeMap::new(),
                pending: BTreeMap::new(),
                fault: Fault::None,
                next: if pending { 0 } else { 1 },
                profile_alive: true,
                login: None,
                records: vec![],
                network_policy: GoogleNetworkPolicy::Inherit,
            }),
            checks: AtomicUsize::new(0),
            events,
            cancel_after_commit: Mutex::new(None),
        }
    }
    fn admit(&self, request: GoogleLeaseRequest) {
        self.state
            .lock()
            .unwrap()
            .admitted
            .insert(target_key(&request), request);
    }
    fn frame(s: &State, generation: NonZeroU32, server: &str) -> AppResult<()> {
        if !s.profile_alive || generation != nz(1) || server != "synthetic-server" {
            return Err(err("controlled_profile_stale"));
        }
        Ok(())
    }
    fn authority(s: &State, r: &GoogleLeaseRequest) -> AppResult<GoogleAuthoritySnapshot> {
        Self::frame(s, r.profile_generation, &r.server_instance_id)?;
        if s.admitted.get(&target_key(r)) != Some(r) {
            return Err(err("controlled_target_not_admitted"));
        }
        if s.grant.state != GoogleGrantState::Active {
            return Err(err("controlled_grant_inactive"));
        }
        let material = s
            .grant
            .material
            .clone()
            .ok_or_else(|| err("controlled_no_material"))?;
        if s.inactive.contains(&material.credential.credential_id) {
            return Err(err("controlled_material_revoked"));
        }
        let origin = if s.mode == Mode::ServiceAccount {
            GoogleLeaseOrigin::ServiceAccount(GoogleServiceAccountLeaseIdentity {
                pool_id: "synthetic-pool".into(),
                pool_revision: nz(1),
                member_id: "synthetic-member".into(),
                credential_version: material.credential.clone(),
                lifecycle_revision: material.lifecycle_revision,
                scopes_hash: google_scope_hash(),
            })
        } else {
            GoogleLeaseOrigin::UserOAuth {
                identity: GoogleAuthorizationLeaseIdentity {
                    grant_id: s.grant.cursor.grant_id.clone(),
                    authorization_epoch: s.grant.cursor.authorization_epoch.unwrap(),
                    material_revision: s.grant.cursor.material_revision.unwrap(),
                    credential_version: material.credential.clone(),
                    scopes_hash: google_scope_hash(),
                    principal_state: s.grant.principal_state.clone(),
                },
                client: s.grant.client.clone(),
                control_revision: s.grant.cursor.control_revision,
                lifecycle_revision: material.lifecycle_revision,
            }
        };
        Ok(GoogleAuthoritySnapshot {
            request: r.clone(),
            origin,
            material,
            network_policy: s.network_policy.clone(),
        })
    }
    fn check_context(s: &State, c: &GoogleMaterialContext) -> AppResult<()> {
        match c {
            GoogleMaterialContext::InitialLogin(p) => {
                Self::frame(
                    s,
                    p.request.profile_generation,
                    &p.request.server_instance_id,
                )?;
                if s.login.as_ref() != Some(p) || s.grant.cursor != p.grant.cursor {
                    return Err(err("controlled_login_stale"));
                }
            }
            GoogleMaterialContext::Refresh {
                request,
                expected,
                mint_material,
                network_policy,
                ..
            } => {
                let a = Self::authority(s, request)?;
                if a.origin.cursor().as_ref() != Some(expected)
                    || &a.material != mint_material
                    || &a.network_policy != network_policy
                {
                    return Err(err("controlled_CAS_stale"));
                }
            }
        }
        Ok(())
    }
}
impl GoogleAuthorizationRepository for ControlledRepo {
    fn lease_authority(&self, r: &GoogleLeaseRequest) -> AppResult<GoogleAuthoritySnapshot> {
        Self::authority(&self.state.lock().unwrap(), r)
    }
    fn check_lease_authority(&self, a: &GoogleAuthoritySnapshot) -> AppResult<()> {
        if Self::authority(&self.state.lock().unwrap(), &a.request)? != *a {
            return Err(err("controlled_CAS_stale"));
        }
        Ok(())
    }
    fn check_issued_lease(&self, r: &GoogleExecutionLeaseRecord) -> AppResult<()> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        r.validate()?;
        let s = self.state.lock().unwrap();
        Self::frame(
            &s,
            r.request.profile_generation,
            &r.request.server_instance_id,
        )?;
        if s.admitted.get(&target_key(&r.request)) != Some(&r.request)
            || s.grant.state != GoogleGrantState::Active
            || r.network_policy != s.network_policy
        {
            return Err(err("controlled_issued_authority_stale"));
        }
        let (credential, lifecycle) = match &r.origin {
            GoogleLeaseOrigin::ServiceAccount(i) => {
                if s.mode != Mode::ServiceAccount
                    || i.pool_id != "synthetic-pool"
                    || i.pool_revision != nz(1)
                    || i.member_id != "synthetic-member"
                {
                    return Err(err("controlled_wrong_source"));
                }
                (&i.credential_version, i.lifecycle_revision)
            }
            GoogleLeaseOrigin::UserOAuth {
                identity,
                client,
                lifecycle_revision,
                ..
            } => {
                if s.mode == Mode::ServiceAccount
                    || identity.grant_id != s.grant.cursor.grant_id
                    || Some(identity.authorization_epoch) != s.grant.cursor.authorization_epoch
                    || client != &s.grant.client
                    || identity.principal_state != s.grant.principal_state
                {
                    return Err(err("controlled_wrong_source"));
                }
                let m = s
                    .history
                    .get(&identity.material_revision.get())
                    .ok_or_else(|| err("controlled_foreign_material"))?;
                if m.credential != identity.credential_version {
                    return Err(err("controlled_foreign_material"));
                }
                (&identity.credential_version, *lifecycle_revision)
            }
        };
        let m = s
            .history
            .values()
            .find(|m| &m.credential == credential)
            .ok_or_else(|| err("controlled_foreign_material"))?;
        if m.lifecycle_revision != lifecycle || s.inactive.contains(&credential.credential_id) {
            return Err(err("controlled_material_revoked"));
        }
        Ok(())
    }
    fn record_attempt_lease(&self, r: &GoogleExecutionLeaseRecord) -> AppResult<()> {
        if !matches!(r.request.target, GoogleLeaseTarget::Attempt { .. }) {
            return Err(err("controlled_test_placeholder_forbidden"));
        }
        self.check_issued_lease(r)?;
        self.state.lock().unwrap().records.push(r.clone());
        Ok(())
    }
    fn prepare_login(&self, r: &GoogleLoginRequest, _: i64) -> AppResult<GooglePendingLogin> {
        let mut s = self.state.lock().unwrap();
        Self::frame(&s, r.profile_generation, &r.server_instance_id)?;
        if s.mode != Mode::Pending || r.client != s.grant.client.key {
            return Err(err("controlled_login_not_pending"));
        }
        let p = GooglePendingLogin {
            request: r.clone(),
            grant: s.grant.clone(),
        };
        s.login = Some(p.clone());
        Ok(p)
    }
    fn check_login(&self, p: &GooglePendingLogin) -> AppResult<()> {
        let s = self.state.lock().unwrap();
        Self::frame(
            &s,
            p.request.profile_generation,
            &p.request.server_instance_id,
        )?;
        if s.login.as_ref() != Some(p)
            || s.grant.cursor != p.grant.cursor
            || s.grant.state != GoogleGrantState::Pending
        {
            return Err(err("controlled_login_stale"));
        }
        Ok(())
    }
    fn reserve_material(
        &self,
        c: &GoogleMaterialContext,
        _: i64,
    ) -> AppResult<GooglePendingMaterial> {
        let mut s = self.state.lock().unwrap();
        Self::check_context(&s, c)?;
        s.next += 1;
        let n = s.next;
        let mut m = material(n, CredentialMaterialKind::GoogleOauthRefresh);
        m.lifecycle_revision = 0;
        let mut p = GooglePendingMaterial {
            allocation_id: format!("allocation-{n}"),
            context: c.clone(),
            material: m,
        };
        if s.fault == Fault::OldVault {
            p.material.vault_ref = s.grant.material.as_ref().unwrap().vault_ref.clone();
        }
        if s.fault == Fault::WrongContext {
            if let GoogleMaterialContext::Refresh { request, .. } = &mut p.context {
                request.profile_generation = nz(2);
            }
        }
        s.pending.insert(p.allocation_id.clone(), p.clone());
        self.events.lock().unwrap().push("repo:reserve".into());
        Ok(p)
    }
    fn commit_material(&self, c: &GoogleMaterialCommit) -> AppResult<GoogleOwnedGrant> {
        let mut s = self.state.lock().unwrap();
        Self::check_context(&s, &c.pending.context)?;
        if s.pending.get(&c.pending.allocation_id) != Some(&c.pending)
            || c.scopes_sha256 != google_scope_hash()
        {
            return Err(err("controlled_bad_commit"));
        }
        self.events.lock().unwrap().push("repo:commit".into());
        if s.fault == Fault::CommitFail {
            return Err(err("controlled_commit_failed"));
        }
        let old = s.grant.cursor.clone();
        let revision = old.material_revision.map(|n| n.get() + 1).unwrap_or(1);
        let epoch = match c.transition {
            GoogleMaterialTransition::InitialLogin => {
                old.authorization_epoch.map(|n| n.get() + 1).unwrap_or(1)
            }
            GoogleMaterialTransition::Refresh => old.authorization_epoch.unwrap().get(),
        };
        let mut m = c.pending.material.clone();
        m.lifecycle_revision = 1;
        s.history.insert(revision, m.clone());
        s.grant.material = Some(m);
        s.grant.cursor = GoogleGrantCursor {
            grant_id: old.grant_id,
            authorization_epoch: Some(nz(epoch)),
            material_revision: Some(nz(revision)),
            control_revision: old.control_revision + 1,
        };
        s.grant.state = GoogleGrantState::Active;
        let result = s.grant.clone();
        if let Some(b) = self.cancel_after_commit.lock().unwrap().take() {
            b.cancel();
        }
        if s.fault == Fault::LostCommitReply {
            return Err(err("controlled_lost_commit_reply"));
        }
        Ok(result)
    }
    fn release_failed_material(&self, p: &GooglePendingMaterial, _: i64) -> AppResult<bool> {
        let mut s = self.state.lock().unwrap();
        Self::check_context(&s, &p.context).or_else(|_| match &p.context {
            GoogleMaterialContext::Refresh { request, .. } => {
                Self::frame(&s, request.profile_generation, &request.server_instance_id)
            }
            GoogleMaterialContext::InitialLogin(x) => Self::frame(
                &s,
                x.request.profile_generation,
                &x.request.server_instance_id,
            ),
        })?;
        self.events.lock().unwrap().push("repo:release".into());
        if s.history
            .values()
            .any(|m| m.credential == p.material.credential)
        {
            return Ok(false);
        }
        Ok(s.pending.remove(&p.allocation_id).as_ref() == Some(p))
    }
    fn owned_grant(&self, r: &GoogleGrantControlRequest) -> AppResult<GoogleOwnedGrant> {
        let s = self.state.lock().unwrap();
        Self::frame(&s, r.profile_generation, &r.server_instance_id)?;
        if s.grant.cursor != r.expected {
            return Err(err("controlled_CAS_stale"));
        }
        Ok(s.grant.clone())
    }
    fn invalidate_grant(
        &self,
        r: &GoogleGrantControlRequest,
        state: GoogleGrantInvalidation,
        _: i64,
    ) -> AppResult<GoogleOwnedGrant> {
        let mut s = self.state.lock().unwrap();
        Self::frame(&s, r.profile_generation, &r.server_instance_id)?;
        if s.grant.cursor != r.expected {
            return Err(err("controlled_CAS_stale"));
        }
        s.grant.cursor.authorization_epoch = Some(nz(s
            .grant
            .cursor
            .authorization_epoch
            .map(|n| n.get() + 1)
            .unwrap_or(1)));
        s.grant.cursor.control_revision += 1;
        s.grant.state = match state {
            GoogleGrantInvalidation::Revoked => GoogleGrantState::Revoked,
            GoogleGrantInvalidation::NeedsReauth => GoogleGrantState::NeedsReauth,
        };
        self.events.lock().unwrap().push("repo:invalidate".into());
        Ok(s.grant.clone())
    }
}
struct TestOperations {
    allowed: Mutex<BTreeMap<String, GoogleLeaseRequest>>,
    alive: AtomicBool,
}
impl GoogleTestOperationAuthority for TestOperations {
    fn check(&self, r: &GoogleLeaseRequest, o: &GoogleTestOperation) -> AppResult<()> {
        if !self.alive.load(Ordering::SeqCst)
            || self.allowed.lock().unwrap().get(&o.operation_id) != Some(r)
        {
            return Err(err("controlled_test_operation_expired"));
        }
        Ok(())
    }
}
struct Vault {
    values: Mutex<BTreeMap<String, Vec<u8>>>,
    events: Arc<Mutex<Vec<String>>>,
    get_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    put_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    put_fails: AtomicBool,
}
impl CredentialVault for Vault {
    fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()> {
        self.events.lock().unwrap().push(format!("vault:put:{key}"));
        if self.put_fails.load(Ordering::SeqCst) {
            return Err(err("controlled_vault_unavailable"));
        }
        let mut v = self.values.lock().unwrap();
        if v.contains_key(key) {
            return Err(err("controlled_overwrite_forbidden"));
        }
        v.insert(key.into(), bytes.to_vec());
        drop(v);
        if let Some(h) = self.put_hook.lock().unwrap().take() {
            h();
        }
        Ok(())
    }
    fn get(&self, key: &str) -> AppResult<Vec<u8>> {
        let value = self
            .values
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| err("controlled_vault_missing"))?;
        if let Some(h) = self.get_hook.lock().unwrap().take() {
            h();
        }
        Ok(value)
    }
    fn delete(&self, key: &str) -> AppResult<()> {
        self.events
            .lock()
            .unwrap()
            .push(format!("vault:delete:{key}"));
        self.values.lock().unwrap().remove(key);
        Ok(())
    }
}
struct Clock {
    base: Instant,
    wall: AtomicI64,
    mono: AtomicI64,
}
impl Clock {
    fn advance(&self, wall: i64, mono: i64) {
        self.wall.fetch_add(wall, Ordering::SeqCst);
        self.mono.fetch_add(mono, Ordering::SeqCst);
    }
}
impl GoogleClock for Clock {
    fn now(&self) -> AppResult<GoogleNow> {
        let delta = self.mono.load(Ordering::SeqCst);
        let monotonic = if delta >= 0 {
            self.base + Duration::from_millis(delta as u64)
        } else {
            self.base - Duration::from_millis(delta.unsigned_abs())
        };
        Ok(GoogleNow {
            unix_ms: 1_700_000_000_000 + self.wall.load(Ordering::SeqCst),
            monotonic,
        })
    }
}
#[derive(Clone)]
enum Reply {
    Token(&'static str, Option<&'static str>),
    InvalidGrant,
    WrongScope,
    RevokeOk,
    NetworkFail,
}
#[derive(Debug, Clone)]
enum Call {
    ServiceAccount(String),
    Refresh(String),
    Code {
        code: String,
        verifier: String,
        redirect: String,
    },
    Revoke,
}
struct Transport {
    queue: Mutex<VecDeque<Reply>>,
    calls: Mutex<Vec<Call>>,
    policies: Mutex<Vec<GoogleNetworkPolicy>>,
    delay: AtomicU64,
    active: Arc<AtomicUsize>,
    max_active: AtomicUsize,
    hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl GoogleTokenTransport for Transport {
    fn exchange<'a>(
        &'a self,
        r: GoogleTokenRequest<'a>,
        policy: &'a GoogleNetworkPolicy,
        _: &'a GoogleBudget,
    ) -> PortFuture<'a, GoogleTokenReply> {
        Box::pin(async move {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            let _active = Active(self.active.clone());
            let call = match r {
                GoogleTokenRequest::ServiceAccount { assertion } => {
                    Call::ServiceAccount(assertion.expose().into())
                }
                GoogleTokenRequest::Refresh { refresh_token, .. } => {
                    Call::Refresh(refresh_token.expose().into())
                }
                GoogleTokenRequest::AuthorizationCode {
                    code,
                    verifier,
                    redirect_uri,
                    ..
                } => Call::Code {
                    code: code.expose().into(),
                    verifier: verifier.expose().into(),
                    redirect: redirect_uri.into(),
                },
                GoogleTokenRequest::Revoke { .. } => Call::Revoke,
            };
            self.calls.lock().unwrap().push(call);
            self.policies.lock().unwrap().push(policy.clone());
            if let Some(h) = self.hook.lock().unwrap().take() {
                h();
            }
            tokio::time::sleep(Duration::from_millis(self.delay.load(Ordering::SeqCst))).await;
            match self.queue.lock().unwrap().pop_front().unwrap_or(Reply::Token("synthetic-access",None)) {Reply::Token(access,refresh)=>{let mut value=json!({"access_token":access,"token_type":"Bearer","expires_in":3600,"scope":GOOGLE_TRANSLATION_SCOPE});if let Some(refresh)=refresh{value["refresh_token"]=json!(refresh);}decode_google_token_response(200,serde_json::to_vec(&value).unwrap(),false)},Reply::WrongScope=>decode_google_token_response(200,serde_json::to_vec(&json!({"access_token":"synthetic-access","token_type":"Bearer","expires_in":3600,"scope":"openid"})).unwrap(),false),Reply::InvalidGrant=>Err(err("google_invalid_grant")),Reply::NetworkFail=>Err(err("google_authorization_network")),Reply::RevokeOk=>Ok(GoogleTokenReply::Revoked)}
        })
    }
}
struct Fixture {
    service: GoogleAuthorizationService,
    repo: Arc<ControlledRepo>,
    vault: Arc<Vault>,
    transport: Arc<Transport>,
    clock: Arc<Clock>,
    ops: Arc<TestOperations>,
}
impl Fixture {
    fn new(mode: Mode, replies: Vec<Reply>) -> Self {
        let events = Arc::new(Mutex::new(vec![]));
        let repo = Arc::new(ControlledRepo::new(mode, events.clone()));
        let mut values = BTreeMap::new();
        if mode != Mode::Pending {
            let credential = if mode == Mode::ServiceAccount {
                import_service_account(serde_json::to_vec(&json!({"type":"service_account","client_email":"synthetic@example.iam.gserviceaccount.com","private_key":include_str!("fixtures/google-synthetic-only-private.pem"),"token_uri":"https://oauth2.googleapis.com/token"})).unwrap().as_slice()).unwrap()
            } else {
                CredentialMaterial::GoogleOauthRefresh {
                    refresh_token: SecretText::from_bytes(b"synthetic-refresh-old".to_vec())
                        .unwrap(),
                }
            };
            values.insert(
                "vault-1".into(),
                encode_vault_credential(&credential)
                    .unwrap()
                    .expose()
                    .to_vec(),
            );
        }
        let vault = Arc::new(Vault {
            values: Mutex::new(values),
            events,
            get_hook: Mutex::new(None),
            put_hook: Mutex::new(None),
            put_fails: AtomicBool::new(false),
        });
        let transport = Arc::new(Transport {
            queue: Mutex::new(replies.into()),
            calls: Mutex::new(vec![]),
            policies: Mutex::new(vec![]),
            delay: AtomicU64::new(0),
            active: Arc::new(AtomicUsize::new(0)),
            max_active: AtomicUsize::new(0),
            hook: Mutex::new(None),
        });
        let clock = Arc::new(Clock {
            base: Instant::now(),
            wall: AtomicI64::new(0),
            mono: AtomicI64::new(0),
        });
        let ops = Arc::new(TestOperations {
            allowed: Mutex::new(BTreeMap::new()),
            alive: AtomicBool::new(true),
        });
        let service = GoogleAuthorizationService::new(
            repo.clone(),
            vault.clone(),
            transport.clone(),
            clock.clone(),
            ops.clone(),
        );
        Self {
            service,
            repo,
            vault,
            transport,
            clock,
            ops,
        }
    }
    fn admit(&self, r: GoogleLeaseRequest) {
        self.repo.admit(r.clone());
        if let GoogleLeaseTarget::TestOperation(o) = &r.target {
            self.ops
                .allowed
                .lock()
                .unwrap()
                .insert(o.operation_id.clone(), r);
        }
    }
    async fn lease(&self, n: u32) -> GoogleAccessLease {
        let r = request(n);
        self.admit(r.clone());
        self.service
            .lease(r, GoogleBudget::new(Duration::from_secs(3)).unwrap())
            .await
            .unwrap()
    }
}
fn used_material(lease: &GoogleAccessLease) -> u32 {
    match &lease.record().origin {
        GoogleLeaseOrigin::UserOAuth { identity, .. } => identity.material_revision.get(),
        _ => panic!("not user"),
    }
}
#[tokio::test]
async fn service_account_exchange_uses_actual_pool_source_and_no_fake_user_grant() {
    let f = Fixture::new(
        Mode::ServiceAccount,
        vec![Reply::Token("synthetic-sa-access", None)],
    );
    let lease = f.lease(1).await;
    match lease.authorization().unwrap() {
        ProviderAuthLease::GoogleServiceAccountAccess { identity, token } => {
            assert_eq!(identity.pool_id, "synthetic-pool");
            assert_eq!(identity.credential_version.credential_id, "credential-1");
            assert_eq!(token.expose(), "synthetic-sa-access");
        }
        _ => panic!("SA must not fabricate user grant"),
    }
    assert_eq!(f.repo.state.lock().unwrap().records.len(), 1);
    assert!(!format!("{lease:?}").contains("synthetic-sa-access"));
    let calls = f.transport.calls.lock().unwrap();
    let Call::ServiceAccount(assertion) = &calls[0] else {
        panic!("wrong auth")
    };
    assert_eq!(assertion.split('.').count(), 3);
}
#[tokio::test]
async fn same_authority_singleflight_mints_once_for_ten_distinct_admitted_attempts() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.transport.delay.store(30, Ordering::SeqCst);
    let mut tasks = vec![];
    for n in 0..10 {
        let r = request(n);
        f.admit(r.clone());
        let service = f.service.clone();
        tasks.push(tokio::spawn(async move {
            service
                .lease(r, GoogleBudget::new(Duration::from_secs(3)).unwrap())
                .await
                .unwrap()
        }));
    }
    for task in tasks {
        task.await.unwrap().authorization().unwrap();
    }
    assert_eq!(f.transport.calls.lock().unwrap().len(), 1);
    assert_eq!(f.repo.state.lock().unwrap().records.len(), 10);
}
#[tokio::test]
async fn refresh_rotation_preserves_actual_old_mint_material_and_cache_survives_benign_cursor_advance()
 {
    let f = Fixture::new(
        Mode::OAuth,
        vec![Reply::Token(
            "synthetic-from-old",
            Some("synthetic-refresh-new"),
        )],
    );
    let first = f.lease(1).await;
    assert_eq!(
        f.repo.state.lock().unwrap().grant.cursor.material_revision,
        Some(nz(2))
    );
    assert_eq!(used_material(&first), 1);
    first.authorization().unwrap();
    let checks = f.repo.checks.load(Ordering::SeqCst);
    let second = f.lease(2).await;
    assert_eq!(used_material(&second), 1);
    assert_eq!(f.transport.calls.lock().unwrap().len(), 1);
    assert!(f.repo.checks.load(Ordering::SeqCst) > checks);
    let state = f.repo.state.lock().unwrap();
    assert!(state.history.contains_key(&1) && state.history.contains_key(&2));
}
#[tokio::test]
async fn revoking_actual_old_material_kills_cached_lease_without_relabelling_it_as_new() {
    let f = Fixture::new(
        Mode::OAuth,
        vec![
            Reply::Token("synthetic-old-access", Some("synthetic-refresh-new")),
            Reply::Token("synthetic-new-access", None),
        ],
    );
    let first = f.lease(1).await;
    f.repo
        .state
        .lock()
        .unwrap()
        .inactive
        .insert("credential-1".into());
    assert!(first.authorization().is_err());
    let r = request(2);
    f.admit(r.clone());
    assert!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .is_err()
    );
    let third = f.lease(3).await;
    assert_eq!(used_material(&third), 2);
    let calls = f.transport.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(matches!(&calls[1],Call::Refresh(v)if v=="synthetic-refresh-new"));
}
#[tokio::test]
async fn wake_expiry_and_wall_clock_discontinuity_invalidate_time_assumptions() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    let a = f.lease(1).await;
    f.service.on_wake().unwrap();
    assert!(a.authorization().is_err());
    f.lease(2).await;
    f.clock.advance(3_700_000, 3_700_000);
    f.lease(3).await;
    f.clock.advance(-20_000, 0);
    f.lease(4).await;
    assert_eq!(f.transport.calls.lock().unwrap().len(), 4);
}
#[tokio::test]
async fn local_revoke_fences_authority_before_remote_failure_and_never_reenables_cache() {
    let f = Fixture::new(
        Mode::OAuth,
        vec![Reply::Token("synthetic-access", None), Reply::NetworkFail],
    );
    let lease = f.lease(1).await;
    let cursor = f.repo.state.lock().unwrap().grant.cursor.clone();
    let repo = f.repo.clone();
    *f.transport.hook.lock().unwrap() = Some(Box::new(move || {
        assert_eq!(
            repo.state.lock().unwrap().grant.state,
            GoogleGrantState::Revoked
        )
    }));
    assert_eq!(
        f.service
            .revoke(
                control(cursor),
                GoogleBudget::new(Duration::from_secs(2)).unwrap()
            )
            .await
            .unwrap_err()
            .code,
        "google_authorization_network"
    );
    assert!(lease.authorization().is_err());
    let r = request(2);
    f.admit(r.clone());
    assert!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .is_err()
    );
    assert_eq!(f.transport.calls.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn invalid_grant_or_expanded_scope_requires_reauth_without_any_execution_lease() {
    for reply in [Reply::InvalidGrant, Reply::WrongScope] {
        let f = Fixture::new(Mode::OAuth, vec![reply]);
        let r = request(1);
        f.admit(r.clone());
        assert!(
            f.service
                .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
                .await
                .is_err()
        );
        let s = f.repo.state.lock().unwrap();
        assert_eq!(s.grant.state, GoogleGrantState::NeedsReauth);
        assert!(s.records.is_empty());
    }
}
#[tokio::test]
async fn revoke_during_vault_read_or_operation_cancel_prevents_token_exchange() {
    for cancel in [false, true] {
        let f = Fixture::new(Mode::OAuth, vec![]);
        let r = request(1);
        f.admit(r.clone());
        let budget = GoogleBudget::new(Duration::from_secs(2)).unwrap();
        let b = budget.clone();
        let repo = f.repo.clone();
        *f.vault.get_hook.lock().unwrap() = Some(Box::new(move || {
            if cancel {
                b.cancel();
            } else {
                repo.state.lock().unwrap().grant.state = GoogleGrantState::Revoked;
            }
        }));
        assert!(f.service.lease(r, budget).await.is_err());
        assert!(f.transport.calls.lock().unwrap().is_empty());
        assert!(f.repo.state.lock().unwrap().records.is_empty());
    }
}
#[tokio::test]
async fn cas_or_vault_failure_only_cleans_uncommitted_new_material() {
    for vault_failure in [false, true] {
        let f = Fixture::new(
            Mode::OAuth,
            vec![Reply::Token(
                "synthetic-access",
                Some("synthetic-refresh-new"),
            )],
        );
        let original = f.vault.values.lock().unwrap()["vault-1"].clone();
        if vault_failure {
            f.vault.put_fails.store(true, Ordering::SeqCst);
        } else {
            f.repo.state.lock().unwrap().fault = Fault::CommitFail;
        }
        let r = request(1);
        f.admit(r.clone());
        assert!(
            f.service
                .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
                .await
                .is_err()
        );
        assert_eq!(f.vault.values.lock().unwrap()["vault-1"], original);
        assert!(!f.vault.values.lock().unwrap().contains_key("vault-2"));
        let state = f.repo.state.lock().unwrap();
        assert_eq!(state.grant.cursor.material_revision, Some(nz(1)));
        assert!(state.records.is_empty());
    }
}
#[tokio::test]
async fn ambiguous_success_is_not_cleaned_as_an_uncommitted_secret() {
    let f = Fixture::new(
        Mode::OAuth,
        vec![Reply::Token(
            "synthetic-access",
            Some("synthetic-refresh-new"),
        )],
    );
    f.repo.state.lock().unwrap().fault = Fault::LostCommitReply;
    let r = request(1);
    f.admit(r.clone());
    assert_eq!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .unwrap_err()
            .code,
        "controlled_lost_commit_reply"
    );
    assert!(f.vault.values.lock().unwrap().contains_key("vault-2"));
    assert_eq!(
        f.repo.state.lock().unwrap().grant.cursor.material_revision,
        Some(nz(2))
    );
    assert!(
        !f.repo
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e == "vault:delete:vault-2")
    );
}
#[tokio::test]
async fn forged_pending_context_or_old_vault_locator_never_overwrites_existing_material() {
    for fault in [Fault::OldVault, Fault::WrongContext] {
        let f = Fixture::new(
            Mode::OAuth,
            vec![Reply::Token(
                "synthetic-access",
                Some("synthetic-refresh-new"),
            )],
        );
        f.repo.state.lock().unwrap().fault = fault;
        let original = f.vault.values.lock().unwrap()["vault-1"].clone();
        let r = request(1);
        f.admit(r.clone());
        assert!(
            f.service
                .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
                .await
                .is_err()
        );
        assert_eq!(f.vault.values.lock().unwrap()["vault-1"], original);
        assert!(
            !f.repo
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|e| e.starts_with("vault:put:"))
        );
    }
}
#[tokio::test]
async fn in_memory_test_operation_is_deferred_and_never_forges_terminal_test_rows() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    let mut r = request(1);
    r.target = GoogleLeaseTarget::TestOperation(GoogleTestOperation {
        operation_id: "synthetic-test-op".into(),
        input_scope: GoogleTestInputScope::SelectedCurrent,
        input_sha256: "c".repeat(64),
        request_semantics_sha256: "d".repeat(64),
    });
    f.admit(r.clone());
    let lease = f
        .service
        .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    assert_eq!(
        lease.persistence(),
        GoogleLeasePersistence::DeferredTerminalTest
    );
    assert!(f.repo.state.lock().unwrap().records.is_empty());
    f.ops.alive.store(false, Ordering::SeqCst);
    assert_eq!(
        lease.authorization().unwrap_err().code,
        "controlled_test_operation_expired"
    );
}
#[tokio::test]
async fn expired_budget_drops_active_network_future_and_cannot_record_success() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.transport.delay.store(1_000, Ordering::SeqCst);
    let r = request(1);
    f.admit(r.clone());
    assert_eq!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_millis(30)).unwrap())
            .await
            .unwrap_err()
            .code,
        "authorization_timeout"
    );
    assert_eq!(f.transport.active.load(Ordering::SeqCst), 0);
    assert!(f.repo.state.lock().unwrap().records.is_empty());
}
async fn complete_browser_session(session: GoogleLoginSession) -> AppResult<GoogleOwnedGrant> {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    let auth = reqwest::Url::parse(session.intent().authorization_url()).unwrap();
    let q: BTreeMap<_, _> = auth.query_pairs().into_owned().collect();
    let uri = reqwest::Url::parse(&q["redirect_uri"]).unwrap();
    let state = q["state"].clone();
    let task = tokio::spawn(session.finish());
    let mut stream = TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
        .await
        .unwrap();
    stream
        .write_all(
            format!(
                "GET {}?state={state}&code=synthetic-code HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                uri.path(),
                uri.port().unwrap()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut data = vec![];
    stream.read_to_end(&mut data).await.unwrap();
    assert!(data.starts_with(b"HTTP/1.1 200"));
    task.await.unwrap()
}
#[tokio::test]
async fn actual_pkce_callback_persists_owned_refresh_and_does_not_publish_execution_access() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::digest;
    let f = Fixture::new(
        Mode::Pending,
        vec![Reply::Token(
            "synthetic-login-access",
            Some("synthetic-login-refresh"),
        )],
    );
    let session = f
        .service
        .begin_login(
            login_request(),
            GoogleBudget::new(Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    let query: BTreeMap<_, _> = reqwest::Url::parse(session.intent().authorization_url())
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    let grant = complete_browser_session(session).await.unwrap();
    assert_eq!(grant.state, GoogleGrantState::Active);
    assert!(f.repo.state.lock().unwrap().records.is_empty());
    let calls = f.transport.calls.lock().unwrap();
    let Call::Code {
        code,
        verifier,
        redirect,
    } = &calls[0]
    else {
        panic!("not code exchange")
    };
    assert_eq!(code, "synthetic-code");
    assert_eq!(
        URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()).as_ref()),
        query["code_challenge"]
    );
    assert_eq!(redirect, &query["redirect_uri"]);
    assert!(f.vault.values.lock().unwrap().contains_key("vault-1"));
}
#[tokio::test]
async fn missing_refresh_or_cancel_after_initial_commit_cannot_return_active_login_success() {
    let f = Fixture::new(
        Mode::Pending,
        vec![Reply::Token("synthetic-only-access", None)],
    );
    let session = f
        .service
        .begin_login(
            login_request(),
            GoogleBudget::new(Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        complete_browser_session(session).await.unwrap_err().code,
        "google_refresh_token_missing"
    );
    assert_eq!(
        f.repo.state.lock().unwrap().grant.state,
        GoogleGrantState::NeedsReauth
    );
    assert!(f.vault.values.lock().unwrap().is_empty());
    let f = Fixture::new(
        Mode::Pending,
        vec![Reply::Token("synthetic-access", Some("synthetic-refresh"))],
    );
    let budget = GoogleBudget::new(Duration::from_secs(3)).unwrap();
    *f.repo.cancel_after_commit.lock().unwrap() = Some(budget.clone());
    let session = f
        .service
        .begin_login(login_request(), budget)
        .await
        .unwrap();
    assert_eq!(
        complete_browser_session(session).await.unwrap_err().code,
        "authorization_cancelled"
    );
    assert_eq!(
        f.repo.state.lock().unwrap().grant.state,
        GoogleGrantState::NeedsReauth
    );
}
#[tokio::test]
async fn cancellation_or_profile_change_during_login_never_activates_wrong_profile() {
    let f = Fixture::new(Mode::Pending, vec![]);
    let session = f
        .service
        .begin_login(
            login_request(),
            GoogleBudget::new(Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    session.cancel();
    assert_eq!(
        f.repo.state.lock().unwrap().grant.state,
        GoogleGrantState::NeedsReauth
    );
    let f = Fixture::new(
        Mode::Pending,
        vec![Reply::Token("synthetic-access", Some("synthetic-refresh"))],
    );
    let repo = f.repo.clone();
    *f.transport.hook.lock().unwrap() = Some(Box::new(move || {
        repo.state.lock().unwrap().profile_alive = false
    }));
    let session = f
        .service
        .begin_login(
            login_request(),
            GoogleBudget::new(Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        complete_browser_session(session).await.unwrap_err().code,
        "controlled_profile_stale"
    );
    assert_eq!(
        f.repo.state.lock().unwrap().grant.state,
        GoogleGrantState::Pending
    );
    assert!(f.vault.values.lock().unwrap().is_empty());
}
#[tokio::test]
async fn invalid_context_or_unverified_client_is_refused_without_secret_or_network_use() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    let mut r = request(1);
    r.profile_generation = nz(2);
    f.admit(r.clone());
    assert!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .is_err()
    );
    assert!(f.transport.calls.lock().unwrap().is_empty());
    let f = Fixture::new(Mode::Pending, vec![]);
    f.repo.state.lock().unwrap().grant.client.setup_state = GoogleClientSetup::Unknown;
    assert_eq!(
        f.service
            .begin_login(
                login_request(),
                GoogleBudget::new(Duration::from_secs(2)).unwrap()
            )
            .await
            .unwrap_err()
            .code,
        "google_client_setup_unverified"
    );
    assert!(f.transport.calls.lock().unwrap().is_empty());
    assert!(f.vault.values.lock().unwrap().is_empty());
}
#[tokio::test]
async fn revoke_then_reenable_lifecycle_during_vault_put_cannot_commit_a_stale_refresh() {
    let f = Fixture::new(
        Mode::OAuth,
        vec![Reply::Token(
            "synthetic-access",
            Some("synthetic-refresh-new"),
        )],
    );
    let repo = f.repo.clone();
    *f.vault.put_hook.lock().unwrap() = Some(Box::new(move || {
        let mut s = repo.state.lock().unwrap();
        s.history.get_mut(&1).unwrap().lifecycle_revision += 1;
        s.grant.material.as_mut().unwrap().lifecycle_revision += 1;
    }));
    let r = request(1);
    f.admit(r.clone());
    assert_eq!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .unwrap_err()
            .code,
        "google_material_cursor_mismatch"
    );
    let s = f.repo.state.lock().unwrap();
    assert_eq!(s.grant.cursor.material_revision, Some(nz(1)));
    assert!(s.records.is_empty());
    assert!(!f.vault.values.lock().unwrap().contains_key("vault-2"));
}
#[tokio::test]
async fn cancelling_one_singleflight_owner_does_not_cancel_another_operation_or_leave_a_refresh_task()
 {
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.transport.delay.store(60, Ordering::SeqCst);
    let a = request(1);
    let b = request(2);
    f.admit(a.clone());
    f.admit(b.clone());
    let service = f.service.clone();
    let first = tokio::spawn(async move {
        service
            .lease(a, GoogleBudget::new(Duration::from_millis(25)).unwrap())
            .await
    });
    for _ in 0..30 {
        if f.transport.active.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let service = f.service.clone();
    let second = tokio::spawn(async move {
        service
            .lease(b, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
    });
    assert_eq!(
        first.await.unwrap().unwrap_err().code,
        "authorization_timeout"
    );
    second.await.unwrap().unwrap().authorization().unwrap();
    assert_eq!(f.transport.active.load(Ordering::SeqCst), 0);
    assert_eq!(f.repo.state.lock().unwrap().records.len(), 1);
    assert_eq!(f.transport.calls.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn revoke_during_exchange_and_invalid_clock_cannot_issue_or_cache_success() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    let repo = f.repo.clone();
    *f.transport.hook.lock().unwrap() = Some(Box::new(move || {
        let mut s = repo.state.lock().unwrap();
        s.grant.cursor.authorization_epoch = Some(nz(2));
        s.grant.cursor.control_revision += 1;
        s.grant.state = GoogleGrantState::Revoked;
    }));
    let r = request(1);
    f.admit(r.clone());
    assert!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .is_err()
    );
    assert!(f.repo.state.lock().unwrap().records.is_empty());
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.clock.advance(-1_700_000_000_000, 0);
    let r = request(1);
    f.admit(r.clone());
    assert_eq!(
        f.service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .unwrap_err()
            .code,
        "authorization_clock_invalid"
    );
    assert!(f.transport.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn successful_remote_revoke_keeps_immutable_mint_record_but_blocks_wire_use() {
    let f = Fixture::new(
        Mode::OAuth,
        vec![Reply::Token("synthetic-access", None), Reply::RevokeOk],
    );
    let lease = f.lease(1).await;
    let cursor = f.repo.state.lock().unwrap().grant.cursor.clone();
    f.service
        .revoke(
            control(cursor),
            GoogleBudget::new(Duration::from_secs(2)).unwrap(),
        )
        .await
        .unwrap();
    assert!(lease.authorization().is_err());
    assert_eq!(f.repo.state.lock().unwrap().records.len(), 1);
    assert_eq!(used_material(&lease), 1);
}
#[tokio::test]
async fn one_owned_grant_serializes_refresh_across_distinct_frozen_targets() {
    let f = Fixture::new(
        Mode::OAuth,
        vec![
            Reply::Token("synthetic-one", Some("synthetic-rotate-one")),
            Reply::Token("synthetic-two", Some("synthetic-rotate-two")),
        ],
    );
    f.transport.delay.store(30, Ordering::SeqCst);
    let a = request(1);
    let mut b = request(2);
    b.config_revision_id = "synthetic-other-config".into();
    b.target_policy_sha256 = "e".repeat(64);
    f.admit(a.clone());
    f.admit(b.clone());
    let service = f.service.clone();
    let left = tokio::spawn(async move {
        service
            .lease(a, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
    });
    let service = f.service.clone();
    let right = tokio::spawn(async move {
        service
            .lease(b, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
    });
    let left = left.await.unwrap();
    let right = right.await.unwrap();
    assert!(left.is_ok(), "left {left:?}");
    assert!(right.is_ok(), "right {right:?}");
    assert_eq!(f.transport.max_active.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.repo.state.lock().unwrap().grant.cursor.material_revision,
        Some(nz(3))
    );
}

#[tokio::test]
async fn frozen_network_policy_is_forwarded_and_cache_cannot_cross_policy_change() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.repo.state.lock().unwrap().network_policy = GoogleNetworkPolicy::Disabled;
    let old = f.lease(1).await;
    assert_eq!(
        *f.transport.policies.lock().unwrap(),
        vec![GoogleNetworkPolicy::Disabled]
    );
    let proxy = GoogleNetworkPolicy::Override {
        proxy_url: "http://127.0.0.1:8765".into(),
    };
    f.repo.state.lock().unwrap().network_policy = proxy.clone();
    assert!(old.authorization().is_err());
    let mut next = request(2);
    next.target_policy_sha256 = "c".repeat(64);
    f.admit(next.clone());
    let fresh = f
        .service
        .lease(next, GoogleBudget::new(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    assert_eq!(fresh.record().network_policy, proxy);
    assert_eq!(
        *f.transport.policies.lock().unwrap(),
        vec![GoogleNetworkPolicy::Disabled, proxy.clone()]
    );
    let mut command = control(f.repo.state.lock().unwrap().grant.cursor.clone());
    command.network_policy = proxy.clone();
    f.transport.queue.lock().unwrap().push_back(Reply::RevokeOk);
    f.service
        .revoke(command, GoogleBudget::new(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    assert_eq!(f.transport.policies.lock().unwrap().last(), Some(&proxy));
    assert!(fresh.authorization().is_err());
}

#[tokio::test]
async fn login_uses_its_frozen_network_policy_and_bad_proxy_cannot_prevent_local_revoke() {
    let f = Fixture::new(
        Mode::Pending,
        vec![Reply::Token("synthetic-access", Some("synthetic-refresh"))],
    );
    let mut login = login_request();
    login.network_policy = GoogleNetworkPolicy::Disabled;
    let session = f
        .service
        .begin_login(login, GoogleBudget::new(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    complete_browser_session(session).await.unwrap();
    assert_eq!(
        *f.transport.policies.lock().unwrap(),
        vec![GoogleNetworkPolicy::Disabled]
    );
    let mut command = control(f.repo.state.lock().unwrap().grant.cursor.clone());
    command.network_policy = GoogleNetworkPolicy::Override {
        proxy_url: "socks5://127.0.0.1:1080".into(),
    };
    let error = f
        .service
        .revoke(command, GoogleBudget::new(Duration::from_secs(2)).unwrap())
        .await
        .unwrap_err();
    assert_eq!(error.code, "invalid_google_proxy");
    assert_eq!(
        f.repo.state.lock().unwrap().grant.state,
        GoogleGrantState::Revoked
    );
    assert_eq!(f.transport.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_source_proxy_fails_before_any_vault_read_or_token_exchange() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.repo.state.lock().unwrap().network_policy = GoogleNetworkPolicy::Override {
        proxy_url: "http://private:synthetic-secret@127.0.0.1:8080".into(),
    };
    let r = request(1);
    f.admit(r.clone());
    let error = f
        .service
        .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
        .await
        .unwrap_err();
    assert_eq!(error.code, "invalid_google_proxy");
    assert!(!format!("{error:?}").contains("synthetic-secret"));
    assert!(f.transport.calls.lock().unwrap().is_empty());
    assert!(
        f.vault
            .events
            .lock()
            .unwrap()
            .iter()
            .all(|e| !e.starts_with("vault:"))
    );
}

#[tokio::test]
async fn lease_invalidation_signal_handles_wake_revoke_and_already_cancelled_budget() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    let lease = f.lease(1).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(10), lease.invalidated())
            .await
            .is_err()
    );
    f.service.on_wake().unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(100), lease.invalidated())
            .await
            .unwrap()
            .code,
        "google_authority_changed"
    );
    let lease = f.lease(2).await;
    let command = control(f.repo.state.lock().unwrap().grant.cursor.clone());
    f.transport.queue.lock().unwrap().push_back(Reply::RevokeOk);
    let (signal, result) = tokio::join!(
        lease.invalidated(),
        f.service
            .revoke(command, GoogleBudget::new(Duration::from_secs(1)).unwrap())
    );
    result.unwrap();
    assert_eq!(signal.code, "google_authority_changed");
    let f = Fixture::new(Mode::OAuth, vec![]);
    let r = request(1);
    f.admit(r.clone());
    let budget = GoogleBudget::new(Duration::from_secs(1)).unwrap();
    let lease = f.service.lease(r, budget.clone()).await.unwrap();
    budget.cancel();
    assert_eq!(lease.invalidated().await.code, "authorization_cancelled");
}

#[tokio::test]
async fn wake_drops_active_token_exchange_instead_of_waiting_for_provider_timeout() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    f.transport.delay.store(10_000, Ordering::SeqCst);
    let r = request(1);
    f.admit(r.clone());
    let mint = f
        .service
        .lease(r, GoogleBudget::new(Duration::from_secs(15)).unwrap());
    let wake = async {
        while f.transport.active.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        f.service.on_wake().unwrap();
    };
    let (outcome, ()) = tokio::time::timeout(Duration::from_millis(250), async {
        tokio::join!(mint, wake)
    })
    .await
    .unwrap();
    assert_eq!(outcome.unwrap_err().code, "google_authority_changed");
    assert_eq!(f.transport.active.load(Ordering::SeqCst), 0);
    assert!(f.repo.state.lock().unwrap().records.is_empty());
}
#[tokio::test]
async fn cached_lease_retains_first_successful_response_observation_without_extending_expiry() {
    let f = Fixture::new(Mode::OAuth, vec![]);
    let started = f.clock.now().unwrap();
    let clock = f.clock.clone();
    *f.transport.hook.lock().unwrap() = Some(Box::new(move || clock.advance(200, 200)));
    let first = f.lease(1).await;
    assert_eq!(first.record().minted_at_ms, started.unix_ms + 200);
    assert_eq!(first.record().expires_at_ms, started.unix_ms + 3_600_000);
    f.clock.advance(1000, 1000);
    let second = f.lease(2).await;
    assert!(f.clock.now().unwrap().unix_ms > second.record().minted_at_ms);
    assert_ne!(first.record().lease_id, second.record().lease_id);
    assert_eq!(second.record().minted_at_ms, first.record().minted_at_ms);
    assert_eq!(second.record().expires_at_ms, first.record().expires_at_ms);
    assert_eq!(second.record().origin, first.record().origin);
    assert_eq!(f.transport.calls.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn response_observation_rejects_consumed_lifetime_and_clock_discontinuity_before_rotation() {
    for (wall, mono, expected) in [
        (3_600_001, 3_600_001, "google_access_expired"),
        (20_000, 0, "authorization_clock_changed"),
    ] {
        let f = Fixture::new(
            Mode::OAuth,
            vec![Reply::Token(
                "synthetic-access",
                Some("synthetic-new-refresh"),
            )],
        );
        let clock = f.clock.clone();
        *f.transport.hook.lock().unwrap() = Some(Box::new(move || clock.advance(wall, mono)));
        let r = request(1);
        f.admit(r.clone());
        let error = f
            .service
            .lease(r, GoogleBudget::new(Duration::from_secs(2)).unwrap())
            .await
            .unwrap_err();
        assert_eq!(error.code, expected);
        assert!(f.repo.state.lock().unwrap().records.is_empty());
        assert_eq!(
            f.repo.state.lock().unwrap().grant.cursor.material_revision,
            Some(nz(1))
        );
        assert_eq!(f.vault.values.lock().unwrap().len(), 1);
    }
}
