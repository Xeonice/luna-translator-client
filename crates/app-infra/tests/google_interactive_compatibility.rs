//! Historical owned-client regressions from the independent Google review.
//! Real Store/Broker/memory Vault/loopback with controlled token replies, no live account.
use app_core::{
    application::google_interactive::GoogleInteractiveController,
    contracts::*,
    domain::model_config::service_graph::GoogleAuthorizationRef,
    ports::{
        IdentitySource, ProductRepository,
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
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn error(code: &str) -> AppError {
    AppError::new(code, "controlled_historical_client", "controlled only")
}
#[derive(Default)]
struct Vault {
    values: Mutex<BTreeMap<String, Vec<u8>>>,
}
impl CredentialVault for Vault {
    fn put(&self, id: &str, bytes: &[u8]) -> AppResult<()> {
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
}
impl GoogleTokenTransport for Transport {
    fn exchange<'a>(
        &'a self,
        request: GoogleTokenRequest<'a>,
        _: &'a GoogleNetworkPolicy,
        budget: &'a GoogleBudget,
    ) -> PortFuture<'a, GoogleTokenReply> {
        Box::pin(async move {
            budget.check()?;
            self.calls.fetch_add(1, Ordering::SeqCst);
            match request {
            GoogleTokenRequest::AuthorizationCode{..}=>decode_google_token_response(200,br#"{"access_token":"controlled-access","token_type":"Bearer","expires_in":3600,"refresh_token":"controlled-refresh"}"#.to_vec(),false),
            _=>Err(error("not_expected")),
        }
        })
    }
}
struct NoBrowser;
impl GoogleSystemBrowser for NoBrowser {
    fn open<'a>(
        &'a self,
        _: &'a GoogleSystemBrowserIntent,
        _: &'a GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, ()> {
        Box::pin(async { Err(error("browser_not_expected_in_readonly_probe")) })
    }
}
struct Fixture {
    path: PathBuf,
    store: Arc<SqliteStore>,
    transport: Arc<Transport>,
    broker: Arc<GoogleAuthorizationService>,
    controller: Arc<GoogleInteractiveController>,
    generation: NonZeroU32,
    _gateway: HttpModelGateway,
}
fn fixture() -> Fixture {
    let path = std::env::temp_dir().join(format!(
        "google-interactive-history-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let store =
        Arc::new(SqliteStore::open(&path, "interactive-profile", "first-server", 10).unwrap());
    store
        .configure_service_directory(Arc::new(app_infra::providers::builtin_registry().unwrap()))
        .unwrap();
    let generation = store.google_runtime_frame().unwrap().profile_generation;
    let vault = Arc::new(Vault::default());
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
        vault,
    ));
    let controller = Arc::new(GoogleInteractiveController::new(
        backend,
        Arc::new(NoBrowser),
    ));
    Fixture {
        path,
        store,
        transport,
        broker,
        controller,
        generation,
        _gateway: gateway,
    }
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

#[tokio::test]
async fn independent_preexisting_owned_client_profile_is_inspected_without_reinventing_its_identity()
 {
    let f = fixture();
    let original = context(&f);
    let legacy = GoogleClientProfile {
        key: GoogleClientKey {
            client_profile_id: "prior-client-profile-explicitly-registered".into(),
            client_profile_revision: nz(3),
        },
        public_client_id: original.public_client_id.clone().unwrap(),
        client_mode: GoogleClientMode::Custom,
        policy_sha256: app_core::domain::content_hash("original-reviewed-client-policy"),
        setup_state: GoogleClientSetup::Verified,
    };
    let actual = f.store.google_runtime_frame().unwrap();
    let request = GoogleLoginRequest {
        operation_id: "prior-owned-flow".into(),
        client: legacy.key.clone(),
        profile_generation: f.generation,
        server_instance_id: actual.server_instance_id,
        network_policy: GoogleNetworkPolicy::Inherit,
    };
    f.store
        .register_google_client_profile(request.clone(), legacy.clone(), SystemIdentity.now_ms())
        .unwrap();
    let session = f
        .broker
        .begin_login(request, GoogleBudget::new(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    let url = session.intent().authorization_url().to_owned();
    let finishing = tokio::spawn(async move { session.finish().await });
    callback(&url, false).await;
    let minted = finishing.await.unwrap().unwrap();
    assert_eq!(minted.client, legacy);
    let owned = GoogleAuthorizationRef {
        grant_id: minted.cursor.grant_id.clone(),
        authorization_epoch: minted.cursor.authorization_epoch.unwrap(),
    };
    let mut edited = original;
    edited.authorization = Some(owned.clone());
    let result = f.controller.inspect(edited).await.unwrap();
    assert_eq!(result.state, GoogleInteractiveState::Authorized);
    assert_eq!(result.authorization, Some(owned));
    assert!(result.can_logout);
    assert_eq!(
        f.store
            .google_interactive_state(f.generation, result.authorization)
            .unwrap()
            .grant
            .unwrap()
            .client,
        legacy,
        "inspect must retain exact historical owned client record, not synthesize a replacement"
    );
}

async fn independent_mint_historical(
    f: &Fixture,
    mode: GoogleClientMode,
) -> (GoogleAuthorizationRef, GoogleClientProfile) {
    let legacy = GoogleClientProfile {
        key: GoogleClientKey {
            client_profile_id: "prior-trusted-desktop-client".into(),
            client_profile_revision: nz(4),
        },
        public_client_id: context(f).public_client_id.unwrap(),
        client_mode: mode,
        policy_sha256: app_core::domain::content_hash(
            "retained-original-policy-not-the-new-derived-policy",
        ),
        setup_state: GoogleClientSetup::Verified,
    };
    let actual = f.store.google_runtime_frame().unwrap();
    let request = GoogleLoginRequest {
        operation_id: "prior-trusted-flow".into(),
        client: legacy.key.clone(),
        profile_generation: f.generation,
        server_instance_id: actual.server_instance_id,
        network_policy: GoogleNetworkPolicy::Disabled,
    };
    f.store
        .register_google_client_profile(request.clone(), legacy.clone(), SystemIdentity.now_ms())
        .unwrap();
    let session = f
        .broker
        .begin_login(request, GoogleBudget::new(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    let url = session.intent().authorization_url().to_owned();
    let finish = tokio::spawn(async move { session.finish().await });
    callback(&url, false).await;
    let grant = finish.await.unwrap().unwrap();
    assert_eq!(grant.client, legacy);
    (
        GoogleAuthorizationRef {
            grant_id: grant.cursor.grant_id,
            authorization_epoch: grant.cursor.authorization_epoch.unwrap(),
        },
        legacy,
    )
}
#[tokio::test]
async fn independent_historical_application_grant_remains_readable_but_missing_new_application_login_stays_blocked()
 {
    let f = fixture();
    let (owned, legacy) = independent_mint_historical(&f, GoogleClientMode::Application).await;
    let mut c = context(&f);
    c.client_mode = GoogleInteractiveClientMode::Application;
    c.public_client_id = None;
    c.authorization = Some(owned.clone());
    let before = rusqlite::Connection::open(&f.path)
        .unwrap()
        .query_row("SELECT count(*) FROM oauth_client_profiles", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap();
    let view = f.controller.inspect(c.clone()).await.unwrap();
    assert_eq!(view.state, GoogleInteractiveState::Authorized);
    assert_eq!(view.authorization, Some(owned.clone()));
    assert!(view.can_logout);
    assert!(
        !view.can_login,
        "retained grant is not a newly installed application client ID"
    );
    assert_eq!(
        f.store
            .google_interactive_state(f.generation, Some(owned))
            .unwrap()
            .grant
            .unwrap()
            .client,
        legacy
    );
    let after = rusqlite::Connection::open(&f.path)
        .unwrap()
        .query_row("SELECT count(*) FROM oauth_client_profiles", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap();
    assert_eq!(
        before, after,
        "inspect cannot synthesize a new client registration"
    );
    c.authorization = None;
    let missing = f.controller.inspect(c).await.unwrap();
    assert_eq!(missing.state, GoogleInteractiveState::Unconfigured);
    assert!(!missing.can_login);
    assert!(!missing.can_logout);
}
#[tokio::test]
async fn independent_historical_custom_reference_still_rejects_wrong_public_client_and_mode() {
    let f = fixture();
    let (owned, _) = independent_mint_historical(&f, GoogleClientMode::Custom).await;
    let mut c = context(&f);
    c.authorization = Some(owned);
    c.public_client_id = Some("unrelated.apps.googleusercontent.com".into());
    let before = f.transport.calls.load(Ordering::SeqCst);
    assert!(f.controller.inspect(c.clone()).await.is_err());
    c.public_client_id = context(&f).public_client_id;
    c.client_mode = GoogleInteractiveClientMode::Application;
    assert!(f.controller.inspect(c).await.is_err());
    assert_eq!(f.transport.calls.load(Ordering::SeqCst), before);
}
#[tokio::test]
async fn independent_historical_owned_reference_stale_epoch_never_becomes_authorized() {
    let f = fixture();
    let (mut owned, _) = independent_mint_historical(&f, GoogleClientMode::Custom).await;
    owned.authorization_epoch = nz(owned.authorization_epoch.get() + 1);
    let mut c = context(&f);
    c.authorization = Some(owned);
    let view = f.controller.inspect(c).await.unwrap();
    assert_eq!(view.state, GoogleInteractiveState::NeedsReauth);
    assert!(!view.can_logout);
    assert!(view.authorization.is_none());
}
