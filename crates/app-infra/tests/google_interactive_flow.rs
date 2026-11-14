//! Actual Store4 + Broker + memory Vault + loopback + controlled browser/token
//! transport. No Google account/client availability or Native UI acceptance.
use app_core::{
    application::google_interactive::GoogleInteractiveController,
    contracts::*,
    domain::model_config::{credentials::*, service_graph::*},
    ports::{
        IdentitySource, ProductRepository, ServiceConfigurationRepository,
        google_authorization::*,
        google_interactive::*,
        runtime::{CredentialVault, PortFuture},
    },
};
use app_infra::{
    platform::SystemIdentity,
    providers::{
        gateway::HttpModelGateway,
        google_authorization::{GoogleAuthorizationService, SystemGoogleClock},
        google_interactive::SqliteGoogleInteractiveBackend,
        google_token::*,
    },
    storage::SqliteStore,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn error(code: &str) -> AppError {
    AppError::new(code, "controlled", "controlled only")
}
struct PutBarrier {
    stage: Mutex<u8>,
    signal: std::sync::Condvar,
}
impl PutBarrier {
    fn new() -> Self {
        Self {
            stage: Mutex::new(0),
            signal: std::sync::Condvar::new(),
        }
    }
}
#[derive(Default)]
struct Vault {
    values: Mutex<BTreeMap<String, Vec<u8>>>,
    fail_delete: AtomicBool,
    put_barrier: Mutex<Option<Arc<PutBarrier>>>,
    delete_barrier: Mutex<Option<Arc<PutBarrier>>>,
}
impl CredentialVault for Vault {
    fn put(&self, id: &str, bytes: &[u8]) -> AppResult<()> {
        let barrier = self.put_barrier.lock().unwrap().clone();
        if let Some(barrier) = barrier {
            let mut stage = barrier.stage.lock().unwrap();
            *stage = 1;
            barrier.signal.notify_all();
            while *stage < 2 {
                stage = barrier.signal.wait(stage).unwrap();
            }
        }
        self.values.lock().unwrap().insert(id.into(), bytes.into());
        Ok(())
    }
    fn get(&self, id: &str) -> AppResult<Vec<u8>> {
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| error("absent"))
    }
    fn delete(&self, id: &str) -> AppResult<()> {
        let barrier = self.delete_barrier.lock().unwrap().clone();
        if let Some(barrier) = barrier {
            let mut stage = barrier.stage.lock().unwrap();
            *stage = 1;
            barrier.signal.notify_all();
            while *stage < 2 {
                stage = barrier.signal.wait(stage).unwrap();
            }
        }
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(error("controlled_vault_locked"));
        }
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}
struct NoTests;
impl GoogleTestOperationAuthority for NoTests {
    fn check(&self, _: &GoogleLeaseRequest, _: &GoogleTestOperation) -> AppResult<()> {
        Err(error("no_test"))
    }
}
#[derive(Default)]
struct Transport {
    calls: AtomicUsize,
    revoke_fails: AtomicBool,
    policies: Mutex<Vec<GoogleNetworkPolicy>>,
}
impl GoogleTokenTransport for Transport {
    fn exchange<'a>(
        &'a self,
        request: GoogleTokenRequest<'a>,
        policy: &'a GoogleNetworkPolicy,
        budget: &'a GoogleBudget,
    ) -> PortFuture<'a, GoogleTokenReply> {
        Box::pin(async move {
            budget.check()?;
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.policies.lock().unwrap().push(policy.clone());
            match request {GoogleTokenRequest::AuthorizationCode{..}=>decode_google_token_response(200,br#"{"access_token":"controlled-access","token_type":"Bearer","expires_in":3600,"refresh_token":"controlled-refresh"}"#.to_vec(),false),GoogleTokenRequest::Revoke{..}=>if self.revoke_fails.load(Ordering::SeqCst){Err(error("controlled_remote_unavailable"))}else{Ok(GoogleTokenReply::Revoked)},_=>Err(error("not_expected"))}
        })
    }
}
#[derive(Default)]
struct Browser {
    opens: AtomicUsize,
    intents: Mutex<Vec<String>>,
}
impl GoogleSystemBrowser for Browser {
    fn open<'a>(
        &'a self,
        intent: &'a GoogleSystemBrowserIntent,
        budget: &'a GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, ()> {
        Box::pin(async move {
            budget.check()?;
            self.intents
                .lock()
                .unwrap()
                .push(intent.native_url().into());
            self.opens.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}
struct Fixture {
    path: PathBuf,
    store: Arc<SqliteStore>,
    vault: Arc<Vault>,
    transport: Arc<Transport>,
    gateway: HttpModelGateway,
    backend: Arc<SqliteGoogleInteractiveBackend>,
    browser: Arc<Browser>,
    controller: Arc<GoogleInteractiveController>,
    generation: NonZeroU32,
}
fn open(path: PathBuf, profile: &str, server: &str, vault: Arc<Vault>) -> Fixture {
    let store = Arc::new(SqliteStore::open(&path, profile, server, 10).unwrap());
    store
        .configure_service_directory(Arc::new(app_infra::providers::builtin_registry().unwrap()))
        .unwrap();
    let generation = store.google_runtime_frame().unwrap().profile_generation;
    let transport = Arc::new(Transport::default());
    let tests = Arc::new(NoTests);
    let broker = Arc::new(GoogleAuthorizationService::new(
        store.google_authorization_repository(tests.clone()),
        vault.clone(),
        transport.clone(),
        Arc::new(SystemGoogleClock),
        tests,
    ));
    let gateway = HttpModelGateway::new(store.clone(), vault.clone())
        .with_google_authorization(broker.clone())
        .unwrap();
    let backend = Arc::new(SqliteGoogleInteractiveBackend::new(
        store.clone(),
        broker.clone(),
        Arc::new(SystemIdentity),
        vault.clone(),
    ));
    let browser = Arc::new(Browser::default());
    let controller = Arc::new(GoogleInteractiveController::new(
        backend.clone(),
        browser.clone(),
    ));
    Fixture {
        path,
        store,
        vault,
        transport,
        gateway,
        backend,
        browser,
        controller,
        generation,
    }
}
fn fixture() -> Fixture {
    open(
        std::env::temp_dir().join(format!(
            "google-interactive-{}.sqlite3",
            uuid::Uuid::new_v4()
        )),
        "interactive-profile",
        "first-server",
        Arc::new(Vault::default()),
    )
}
fn context(f: &Fixture) -> GoogleInteractiveContext {
    GoogleInteractiveContext {
        expected_profile_generation: f.generation,
        client_mode: GoogleInteractiveClientMode::Custom,
        public_client_id: Some("controlled.apps.googleusercontent.com".into()),
        authorization: None,
        proxy: None,
    }
}
fn query(f: &Fixture, id: &str) -> GoogleLoginOperationQuery {
    GoogleLoginOperationQuery {
        operation_id: id.into(),
        expected_profile_generation: f.generation,
    }
}
async fn wait_browser(f: &Fixture, n: usize) -> String {
    for _ in 0..500 {
        if f.browser.opens.load(Ordering::SeqCst) >= n {
            return f.browser.intents.lock().unwrap()[n - 1].clone();
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("controlled browser did not open")
}
async fn callback(url: &str, wrong_state: bool) {
    let uri = reqwest::Url::parse(url).unwrap();
    assert_eq!(uri.host_str(), Some("accounts.google.com"));
    let p: BTreeMap<_, _> = uri.query_pairs().into_owned().collect();
    assert_eq!(p["code_challenge_method"], "S256");
    assert_eq!(p["scope"], GOOGLE_TRANSLATION_SCOPE);
    assert!(!p.contains_key("client_secret"));
    let mut callback = reqwest::Url::parse(&p["redirect_uri"]).unwrap();
    callback
        .query_pairs_mut()
        .append_pair(
            "state",
            if wrong_state {
                "wrong-state"
            } else {
                &p["state"]
            },
        )
        .append_pair("code", "controlled-code");
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(callback)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        if wrong_state { 400 } else { 200 }
    );
    let body = response.text().await.unwrap();
    assert!(!body.contains("controlled-code"));
    assert!(!body.contains(&p["state"]));
}
async fn terminal(f: &Fixture, id: &str) -> GoogleLoginOperationProjection {
    for _ in 0..500 {
        let v = f.controller.status(query(f, id)).unwrap();
        if v.state != GoogleInteractiveState::Authorizing {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("login did not finish")
}
async fn login(f: &Fixture, id: &str) -> GoogleAuthorizationRef {
    let before = f.browser.opens.load(Ordering::SeqCst);
    f.controller
        .start(StartGoogleLoginInput {
            operation_id: id.into(),
            context: context(f),
        })
        .await
        .unwrap();
    callback(&wait_browser(f, before + 1).await, false).await;
    let view = terminal(f, id).await;
    assert_eq!(view.state, GoogleInteractiveState::Authorized, "{view:?}");
    view.authorization.unwrap()
}
#[tokio::test]
async fn real_login_produces_owned_reference_without_saving_or_selecting() {
    let f = fixture();
    let heads = f.store.service_configuration().unwrap();
    let projection = f.controller.inspect(context(&f)).await.unwrap();
    assert_eq!(projection.state, GoogleInteractiveState::Ready);
    assert_eq!(f.browser.opens.load(Ordering::SeqCst), 0);
    let reference = login(&f, "real-callback").await;
    let mut c = context(&f);
    c.authorization = Some(reference.clone());
    let projection = f.controller.inspect(c).await.unwrap();
    assert_eq!(projection.state, GoogleInteractiveState::Authorized);
    assert_eq!(projection.authorization, Some(reference));
    assert_eq!(projection.affected_service_count, 0);
    let without_reference = f.controller.inspect(context(&f)).await.unwrap();
    assert_eq!(without_reference.state, GoogleInteractiveState::Ready);
    assert!(
        without_reference.authorization.is_none(),
        "an arbitrary latest grant is never discovered or adopted"
    );
    assert_eq!(f.store.service_configuration().unwrap(), heads);
    assert_eq!(f.vault.values.lock().unwrap().len(), 1);
    assert!(
        !serde_json::to_string(&projection)
            .unwrap()
            .contains("controlled-refresh")
    );
    assert!(!format!("{projection:?}").contains("controlled-code"));
}
#[tokio::test]
async fn builtin_missing_client_is_blocked_without_browser_or_registration() {
    let f = fixture();
    let mut c = context(&f);
    c.client_mode = GoogleInteractiveClientMode::Application;
    c.public_client_id = None;
    let state = f.controller.inspect(c.clone()).await.unwrap();
    assert!(!state.can_login);
    assert_eq!(state.state, GoogleInteractiveState::Unconfigured);
    assert!(
        f.controller
            .start(StartGoogleLoginInput {
                operation_id: "no-builtin".into(),
                context: c
            })
            .await
            .is_err()
    );
    assert_eq!(f.browser.opens.load(Ordering::SeqCst), 0);
    assert_eq!(f.transport.calls.load(Ordering::SeqCst), 0);
    let db = rusqlite::Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM oauth_client_profiles", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn wrong_loopback_state_never_mints_grant_and_cancel_physically_closes_listener() {
    let f = fixture();
    f.controller
        .start(StartGoogleLoginInput {
            operation_id: "state-check".into(),
            context: context(&f),
        })
        .await
        .unwrap();
    let url = wait_browser(&f, 1).await;
    callback(&url, true).await;
    assert_eq!(f.transport.calls.load(Ordering::SeqCst), 0);
    f.controller.cancel(query(&f, "state-check")).unwrap();
    let end = terminal(&f, "state-check").await;
    assert_ne!(end.state, GoogleInteractiveState::Authorized);
    let parsed = reqwest::Url::parse(&url).unwrap();
    let p: BTreeMap<_, _> = parsed.query_pairs().into_owned().collect();
    let local = reqwest::Url::parse(&p["redirect_uri"]).unwrap();
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", local.port().unwrap()))
            .await
            .is_err()
    );
    let db = rusqlite::Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM oauth_authorization_snapshots WHERE state='active'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[tokio::test]
async fn actual_handler_owner_and_generation_are_required() {
    let f = fixture();
    let tests = Arc::new(NoTests);
    let other = Arc::new(GoogleAuthorizationService::new(
        f.store.google_authorization_repository(tests.clone()),
        f.vault.clone(),
        f.transport.clone(),
        Arc::new(SystemGoogleClock),
        tests,
    ));
    let unrelated = SqliteGoogleInteractiveBackend::new(
        f.store.clone(),
        other,
        Arc::new(SystemIdentity),
        f.vault.clone(),
    );
    assert_eq!(
        unrelated.inspect(context(&f)).await.unwrap_err().code,
        "google_handler_owner_mismatch"
    );
    let mut stale = context(&f);
    stale.expected_profile_generation = nz(f.generation.get() + 1);
    assert_eq!(
        f.backend.inspect(stale).await.unwrap_err().code,
        "google_profile_stale"
    );
    let current = context(&f);
    drop(f.gateway);
    assert_eq!(
        f.backend.inspect(current).await.unwrap_err().code,
        "google_handler_not_installed"
    );
}
#[tokio::test]
async fn malformed_proxy_is_rejected_before_token_and_actual_override_is_frozen() {
    let f = fixture();
    let mut c = context(&f);
    c.proxy = Some(GoogleInteractiveProxy {
        mode: "override".into(),
        proxy_url: Some("http://user:secret@localhost:9000?token=secret".into()),
    });
    assert!(f.controller.inspect(c).await.is_err());
    assert_eq!(f.transport.calls.load(Ordering::SeqCst), 0);
    let mut c = context(&f);
    c.proxy = Some(GoogleInteractiveProxy {
        mode: "override".into(),
        proxy_url: Some("http://127.0.0.1:9000".into()),
    });
    f.controller
        .start(StartGoogleLoginInput {
            operation_id: "proxy".into(),
            context: c,
        })
        .await
        .unwrap();
    callback(&wait_browser(&f, 1).await, false).await;
    assert_eq!(
        terminal(&f, "proxy").await.state,
        GoogleInteractiveState::Authorized
    );
    assert_eq!(
        f.transport.policies.lock().unwrap().as_slice(),
        &[GoogleNetworkPolicy::Override {
            proxy_url: "http://127.0.0.1:9000".into()
        }]
    );
}
#[tokio::test]
async fn local_revoke_survives_remote_failure_and_old_reference_is_not_rebound() {
    let f = fixture();
    let reference = login(&f, "revoke-source").await;
    f.transport.revoke_fails.store(true, Ordering::SeqCst);
    let result = f
        .controller
        .revoke(RevokeGoogleAuthorizationInput {
            operation_id: "real-revoke".into(),
            expected_profile_generation: f.generation,
            authorization: reference.clone(),
            proxy: None,
        })
        .await
        .unwrap();
    assert!(result.local_revoked);
    assert!(!result.remote_revocation_confirmed);
    let mut c = context(&f);
    c.authorization = Some(reference);
    let projection = f.controller.inspect(c).await.unwrap();
    assert!(!projection.can_logout);
    assert!(projection.authorization.is_none());
    let db = rusqlite::Connection::open(&f.path).unwrap();
    let state:String=db.query_row("SELECT a.state FROM oauth_grants g JOIN oauth_authorization_snapshots a USING(grant_id,authorization_epoch)",[],|r|r.get(0)).unwrap();
    assert_eq!(state, "revoked");
}
struct Gap {
    stage: Mutex<u8>,
    signal: std::sync::Condvar,
    reference: Mutex<Option<GoogleAuthorizationRef>>,
}
impl Gap {
    fn new() -> Self {
        Self {
            stage: Mutex::new(0),
            signal: std::sync::Condvar::new(),
            reference: Mutex::new(None),
        }
    }
}
struct GapBackend {
    inner: Arc<SqliteGoogleInteractiveBackend>,
    gap: Arc<Gap>,
}
struct GapLogin {
    inner: Box<dyn OwnedGoogleLogin>,
    gap: Arc<Gap>,
}
struct GapCompleted {
    inner: Box<dyn CompletedGoogleLogin>,
    gap: Arc<Gap>,
}
impl GoogleInteractiveBackend for GapBackend {
    fn inspect<'a>(
        &'a self,
        c: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        self.inner.inspect(c)
    }
    fn begin<'a>(
        &'a self,
        i: StartGoogleLoginInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move {
            Ok(Box::new(GapLogin {
                inner: self.inner.begin(i, b).await?,
                gap: self.gap.clone(),
            }) as Box<dyn OwnedGoogleLogin>)
        })
    }
    fn revoke<'a>(
        &'a self,
        i: RevokeGoogleAuthorizationInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        self.inner.revoke(i, b)
    }
}
impl OwnedGoogleLogin for GapLogin {
    fn browser_intent(&self) -> GoogleSystemBrowserIntent {
        self.inner.browser_intent()
    }
    fn finish(self: Box<Self>) -> GoogleInteractiveFuture<'static, Box<dyn CompletedGoogleLogin>> {
        Box::pin(async move {
            Ok(Box::new(GapCompleted {
                inner: self.inner.finish().await?,
                gap: self.gap,
            }) as Box<dyn CompletedGoogleLogin>)
        })
    }
}
impl CompletedGoogleLogin for GapCompleted {
    fn authorization(&self) -> GoogleAuthorizationRef {
        let reference = self.inner.authorization();
        *self.gap.reference.lock().unwrap() = Some(reference.clone());
        let mut stage = self.gap.stage.lock().unwrap();
        *stage = 1;
        self.gap.signal.notify_all();
        while *stage < 2 {
            stage = self.gap.signal.wait(stage).unwrap();
        }
        reference
    }
    fn discard(self: Box<Self>) -> GoogleInteractiveFuture<'static, ()> {
        self.inner.discard()
    }
}
#[tokio::test]
async fn cancellation_after_broker_finish_before_core_fence_cas_invalidates_only_new_grant() {
    let mut f = fixture();
    let old = login(&f, "previous-owned-grant").await;
    let gap = Arc::new(Gap::new());
    f.controller = Arc::new(GoogleInteractiveController::new(
        Arc::new(GapBackend {
            inner: f.backend.clone(),
            gap: gap.clone(),
        }),
        f.browser.clone(),
    ));
    let controller = f.controller.clone();
    let cancel_query = query(&f, "finish-gap");
    let barrier = gap.clone();
    let cancel = std::thread::spawn(move || {
        let mut stage = barrier.stage.lock().unwrap();
        while *stage < 1 {
            stage = barrier.signal.wait(stage).unwrap();
        }
        drop(stage);
        let response = controller.cancel(cancel_query).unwrap();
        assert_eq!(response.state, GoogleInteractiveState::Authorizing);
        *barrier.stage.lock().unwrap() = 2;
        barrier.signal.notify_all();
    });
    f.controller
        .start(StartGoogleLoginInput {
            operation_id: "finish-gap".into(),
            context: context(&f),
        })
        .await
        .unwrap();
    callback(&wait_browser(&f, 2).await, false).await;
    let result = terminal(&f, "finish-gap").await;
    cancel.join().unwrap();
    assert_eq!(result.state, GoogleInteractiveState::Cancelled);
    assert!(result.authorization.is_none());
    let new = gap.reference.lock().unwrap().clone().unwrap();
    assert_ne!(new.grant_id, old.grant_id);
    let fresh = f
        .store
        .google_interactive_state(f.generation, Some(new))
        .unwrap()
        .grant
        .unwrap();
    assert_eq!(fresh.state, GoogleGrantState::NeedsReauth);
    let previous = f
        .store
        .google_interactive_state(f.generation, Some(old))
        .unwrap()
        .grant
        .unwrap();
    assert_eq!(previous.state, GoogleGrantState::Active);
}
struct NoHook;
impl app_core::ports::runtime::HookBackend for NoHook {
    fn targets(&self) -> AppResult<TargetCandidates> {
        Err(error("not_started"))
    }
    fn associate(
        &self,
        _: AssociateRequest,
    ) -> AppResult<app_core::domain::capture::HookSessionInput> {
        Err(error("not_started"))
    }
    fn disconnect(&self) -> AppResult<()> {
        Ok(())
    }
    fn subscribe(
        &self,
    ) -> AppResult<std::sync::mpsc::Receiver<app_core::ports::runtime::HookUpdate>> {
        Err(error("not_started"))
    }
    fn resource_info(&self) -> ResourceInfo {
        ResourceInfo {
            kind: "hook".into(),
            version: "controlled".into(),
            manifest_sha256: "a".repeat(64),
            available: true,
            detail: "controlled".into(),
        }
    }
}
struct NoGateway;
impl app_core::ports::runtime::ModelGateway for NoGateway {
    fn execute<'a>(
        &'a self,
        _: &'a app_core::domain::TaskDispatch,
        _: &'a app_core::application::request_plan::RequestPlan,
        _: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(error("not_dispatched")) })
    }
    fn test_request<'a>(
        &'a self,
        _: &'a Connection,
        _: &'a app_core::application::request_plan::RequestPlan,
        _: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(error("not_tested")) })
    }
    fn discover_models<'a>(
        &'a self,
        _: &'a Connection,
        _: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Err(error("not_discovered")) })
    }
}
struct NoLanguage;
impl app_core::ports::runtime::LanguageServices for NoLanguage {
    fn analyze<'a>(&'a self, _: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn lookup(&self, _: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        Ok(vec![])
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![]
    }
}
fn application(f: &Fixture) -> Arc<app_core::application::runtime::ApplicationRuntime> {
    app_core::application::runtime::ApplicationRuntime::new(
        f.store.clone(),
        Arc::new(SystemIdentity),
        Arc::new(NoHook),
        Arc::new(NoGateway),
        Arc::new(NoLanguage),
        f.vault.clone(),
    )
    .unwrap()
}
fn generic_recover(f: &Fixture) {
    application(f).recover_credentials().unwrap();
}
fn reserve_unactivated(
    f: &Fixture,
    client: GoogleClientProfile,
    operation: &str,
) -> GooglePendingMaterial {
    let repo = f.store.google_authorization_repository(Arc::new(NoTests));
    let request = GoogleLoginRequest {
        operation_id: operation.into(),
        client: client.key.clone(),
        profile_generation: f.generation,
        server_instance_id: f.store.google_runtime_frame().unwrap().server_instance_id,
        network_policy: GoogleNetworkPolicy::Inherit,
    };
    f.store
        .register_google_client_profile(request.clone(), client, SystemIdentity.now_ms())
        .unwrap();
    let pending = repo
        .prepare_login(&request, SystemIdentity.now_ms())
        .unwrap();
    let reserved = repo
        .reserve_material(
            &GoogleMaterialContext::InitialLogin(pending),
            SystemIdentity.now_ms(),
        )
        .unwrap();
    let bytes = encode_vault_credential(&CredentialMaterial::GoogleOauthRefresh {
        refresh_token: SecretText::from_bytes(b"abandoned-controlled-refresh".to_vec()).unwrap(),
    })
    .unwrap();
    f.vault
        .put(&reserved.material.vault_ref, bytes.expose())
        .unwrap();
    reserved
}
#[tokio::test]
async fn generic_and_google_recovery_are_isolated_across_real_sql_reopen() {
    let f = fixture();
    let active = login(&f, "persistent-active").await;
    let active_grant = f
        .store
        .google_interactive_state(f.generation, Some(active.clone()))
        .unwrap()
        .grant
        .unwrap();
    let active_vault = active_grant.material.as_ref().unwrap().vault_ref.clone();
    let reserved = reserve_unactivated(&f, active_grant.client, "abandoned-allocation");
    let abandoned = reserved.material.vault_ref.clone();
    f.store
        .begin_credential_material(
            CredentialReference {
                credential_id: "pending-pool-credential".into(),
                credential_version_id: "pending-pool-vault".into(),
                pool_id: "pending-pool".into(),
                pool_revision: 1,
                label: "Controlled pool".into(),
                masked_hint: "controlled".into(),
            },
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::ApiKey,
            },
            SystemIdentity.now_ms(),
        )
        .unwrap();
    f.vault
        .put("pending-pool-vault", b"controlled-pool-material")
        .unwrap();
    generic_recover(&f);
    assert!(
        f.vault.get("pending-pool-vault").is_err(),
        "original pool recovery remains effective"
    );
    assert!(f.vault.get(&abandoned).is_ok());
    assert!(f.vault.get(&active_vault).is_ok());
    assert_eq!(
        f.backend
            .recover_abandoned_materials(f.vault.clone())
            .await
            .unwrap(),
        0,
        "current operation must not be cleaned"
    );
    let path = f.path.clone();
    let vault = f.vault.clone();
    drop(f);
    let reopened = open(path, "interactive-profile", "second-server", vault.clone());
    generic_recover(&reopened);
    assert!(vault.get(&abandoned).is_ok());
    assert!(vault.get(&active_vault).is_ok());
    let foreign = open(
        std::env::temp_dir().join(format!("google-foreign-{}.sqlite3", uuid::Uuid::new_v4())),
        "foreign-profile",
        "foreign-server",
        vault.clone(),
    );
    assert_eq!(
        foreign
            .backend
            .recover_abandoned_materials(vault.clone())
            .await
            .unwrap(),
        0
    );
    assert!(vault.get(&abandoned).is_ok());
    assert_eq!(
        reopened
            .backend
            .recover_abandoned_materials(vault.clone())
            .await
            .unwrap(),
        1
    );
    assert!(vault.get(&abandoned).is_err());
    assert!(vault.get(&active_vault).is_ok());
    assert_eq!(
        reopened
            .store
            .google_interactive_state(reopened.generation, Some(active))
            .unwrap()
            .grant
            .unwrap()
            .state,
        GoogleGrantState::Active
    );
    assert_eq!(
        reopened
            .backend
            .recover_abandoned_materials(vault)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn locked_vault_keeps_truthful_durable_cleanup_pending_without_disabling_reading_runtime() {
    let f = fixture();
    let active = login(&f, "cleanup-owner").await;
    let grant = f
        .store
        .google_interactive_state(f.generation, Some(active))
        .unwrap()
        .grant
        .unwrap();
    let pending = reserve_unactivated(&f, grant.client, "locked-cleanup");
    let path = f.path.clone();
    let vault = f.vault.clone();
    drop(f);
    let reopened = open(
        path,
        "interactive-profile",
        "recovery-server",
        vault.clone(),
    );
    vault.fail_delete.store(true, Ordering::SeqCst);
    assert!(
        reopened
            .backend
            .recover_abandoned_materials(vault.clone())
            .await
            .is_err()
    );
    assert!(vault.get(&pending.material.vault_ref).is_ok());
    generic_recover(&reopened);
    assert!(reopened.store.snapshot().is_ok());
    let candidates = reopened
        .store
        .google_material_cleanup_candidates(
            reopened.generation,
            "recovery-server".into(),
            SystemIdentity.now_ms(),
        )
        .unwrap();
    assert_eq!(candidates.len(), 1);
    vault.fail_delete.store(false, Ordering::SeqCst);
    assert_eq!(
        reopened
            .backend
            .recover_abandoned_materials(vault)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn cancellation_and_shutdown_wait_for_real_blocking_vault_write() {
    let f = fixture();
    let barrier = Arc::new(PutBarrier::new());
    *f.vault.put_barrier.lock().unwrap() = Some(barrier.clone());
    f.controller
        .start(StartGoogleLoginInput {
            operation_id: "physical-vault".into(),
            context: context(&f),
        })
        .await
        .unwrap();
    callback(&wait_browser(&f, 1).await, false).await;
    for _ in 0..500 {
        if *barrier.stage.lock().unwrap() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(*barrier.stage.lock().unwrap(), 1);
    let drain = {
        let controller = f.controller.clone();
        tokio::spawn(async move { controller.stop_and_drain().await })
    };
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(
        !drain.is_finished(),
        "a cancelled budget is not physical Vault completion"
    );
    assert_eq!(
        f.controller
            .status(query(&f, "physical-vault"))
            .unwrap()
            .state,
        GoogleInteractiveState::Authorizing
    );
    *barrier.stage.lock().unwrap() = 2;
    barrier.signal.notify_all();
    drain.await.unwrap().unwrap();
    assert_eq!(
        terminal(&f, "physical-vault").await.state,
        GoogleInteractiveState::Cancelled
    );
    assert!(f.vault.values.lock().unwrap().is_empty());
    let db = rusqlite::Connection::open(&f.path).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM oauth_grants g JOIN oauth_authorization_snapshots s USING(grant_id,authorization_epoch) WHERE s.state='active'",[],|r|r.get::<_,u32>(0)).unwrap(),0);
}
#[tokio::test]
async fn shared_revoke_impact_counts_only_actual_current_saved_connections() {
    use app_core::domain::model_config::service_draft::{
        PreparedGoogleClientDraft, PreparedServiceDraftResources, compile_service_draft,
    };
    let f = fixture();
    let authorization = login(&f, "shared-grant").await;
    let owned = f
        .store
        .google_interactive_state(f.generation, Some(authorization.clone()))
        .unwrap()
        .grant
        .unwrap();
    let prepared = PreparedServiceDraftResources {
        google_client: Some(PreparedGoogleClientDraft {
            mode: "custom".into(),
            client_id: Some(owned.client.public_client_id.clone()),
            authorization: authorization.clone(),
        }),
        ..Default::default()
    };
    let directory = app_infra::providers::builtin_registry().unwrap();
    for n in 0..2 {
        let state = f.store.service_configuration().unwrap();
        let input = SaveServiceDraftInput {
            operation_id: format!("save-shared-{n}"),
            expected_profile_generation: f.generation,
            expected_draft_head_revision: state.draft_head_revision,
            editing: None,
            target: ServiceFormTarget::Http {
                preset: ProviderPresetKey {
                    preset_id: "builtin.google-v3".into(),
                    preset_revision: 1,
                },
            },
            purpose: CatalogPurpose::Translation,
            values: BTreeMap::from([
                ("name".into(), CatalogScalar::Text(format!("Shared {n}"))),
                (
                    "google_auth_method".into(),
                    CatalogScalar::Text("oauth".into()),
                ),
                (
                    "oauth_client_mode".into(),
                    CatalogScalar::Text("custom".into()),
                ),
                (
                    "oauth_client_id".into(),
                    CatalogScalar::Text(owned.client.public_client_id.clone()),
                ),
                (
                    "project_id".into(),
                    CatalogScalar::Text("controlled-project".into()),
                ),
                ("location".into(), CatalogScalar::Text("global".into())),
                (
                    "model".into(),
                    CatalogScalar::Text(
                        "projects/controlled-project/locations/global/models/general/nmt".into(),
                    ),
                ),
            ]),
            authentication: ServiceAuthenticationSelection::GoogleOwnedGrant {
                authorization: authorization.clone(),
            },
        };
        let compiled =
            compile_service_draft(&state.draft, &input, &directory, &SystemIdentity, &prepared)
                .unwrap();
        f.store
            .save_service_configuration(&compiled, SystemIdentity.now_ms())
            .unwrap();
    }
    let mut c = context(&f);
    c.authorization = Some(authorization.clone());
    let projection = f.controller.inspect(c).await.unwrap();
    assert_eq!(projection.affected_service_count, 2);
    let foreign = open(
        std::env::temp_dir().join(format!(
            "google-cross-profile-{}.sqlite3",
            uuid::Uuid::new_v4()
        )),
        "other-profile",
        "other-server",
        Arc::new(Vault::default()),
    );
    let mut unrelated = context(&foreign);
    unrelated.authorization = Some(authorization.clone());
    assert_eq!(
        foreign.backend.inspect(unrelated).await.unwrap_err().code,
        "google_grant_not_found"
    );
    let revoked = f
        .controller
        .revoke(RevokeGoogleAuthorizationInput {
            operation_id: "revoke-shared".into(),
            expected_profile_generation: f.generation,
            authorization,
            proxy: None,
        })
        .await
        .unwrap();
    assert!(revoked.local_revoked);
    assert!(revoked.remote_revocation_confirmed);
}
#[tokio::test]
async fn startup_recovery_does_not_wait_for_vault_and_finished_inactive_failure_does_not_block_exit()
 {
    let f = fixture();
    let active = login(&f, "recovery-background-owner").await;
    let grant = f
        .store
        .google_interactive_state(f.generation, Some(active))
        .unwrap()
        .grant
        .unwrap();
    let pending = reserve_unactivated(&f, grant.client, "background-abandoned");
    let path = f.path.clone();
    let vault = f.vault.clone();
    drop(f);
    let f = open(
        path,
        "interactive-profile",
        "background-server",
        vault.clone(),
    );
    let barrier = Arc::new(PutBarrier::new());
    *vault.delete_barrier.lock().unwrap() = Some(barrier.clone());
    vault.fail_delete.store(true, Ordering::SeqCst);
    let runtime = application(&f);
    runtime
        .configure_google_interactive(f.controller.clone())
        .unwrap();
    runtime.start_google_material_recovery().unwrap();
    for _ in 0..500 {
        if *barrier.stage.lock().unwrap() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(*barrier.stage.lock().unwrap(), 1);
    assert!(
        runtime.snapshot().is_ok(),
        "application APIs remain usable while inactive Vault deletion is blocked"
    );
    let drain = {
        let controller = f.controller.clone();
        tokio::spawn(async move { controller.stop_and_drain().await })
    };
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(
        !drain.is_finished(),
        "in-flight physical delete is still owned"
    );
    *barrier.stage.lock().unwrap() = 2;
    barrier.signal.notify_all();
    drain.await.unwrap().unwrap();
    assert!(
        vault.get(&pending.material.vault_ref).is_ok(),
        "a failed deletion is not reported cleaned"
    );
    assert!(
        runtime
            .snapshot()
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "google_material_cleanup_pending")
    );
    assert!(
        f.controller.stop_and_drain().await.is_ok(),
        "a completed inactive cleanup failure is a durable recovery obligation, not an active-grant exit block"
    );
}
