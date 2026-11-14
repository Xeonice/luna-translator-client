//! Controller ownership tests, not live Google account acceptance.
use app_core::{
    application::google_interactive::GoogleInteractiveController,
    contracts::*,
    domain::model_config::service_graph::GoogleAuthorizationRef,
    ports::{google_authorization::GoogleBudget, google_interactive::*},
};
use std::{
    num::NonZeroU32,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
fn context() -> GoogleInteractiveContext {
    GoogleInteractiveContext {
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
        client_mode: GoogleInteractiveClientMode::Custom,
        public_client_id: Some("controlled.apps.googleusercontent.com".into()),
        authorization: None,
        proxy: None,
    }
}
fn input(id: &str) -> StartGoogleLoginInput {
    StartGoogleLoginInput {
        operation_id: id.into(),
        context: context(),
    }
}
fn query(id: &str) -> GoogleLoginOperationQuery {
    GoogleLoginOperationQuery {
        operation_id: id.into(),
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
    }
}
fn reference() -> GoogleAuthorizationRef {
    GoogleAuthorizationRef {
        grant_id: "only-new-grant".into(),
        authorization_epoch: NonZeroU32::new(1).unwrap(),
    }
}
struct State {
    opens: AtomicUsize,
    begins: AtomicUsize,
    finished: AtomicBool,
    discarded: AtomicBool,
    fail_cleanup: AtomicBool,
    release: Notify,
    revokes: AtomicUsize,
    revoke_release: Notify,
    budget: Mutex<Option<GoogleBudget>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            opens: AtomicUsize::new(0),
            begins: AtomicUsize::new(0),
            finished: AtomicBool::new(false),
            discarded: AtomicBool::new(false),
            fail_cleanup: AtomicBool::new(false),
            release: Notify::new(),
            revokes: AtomicUsize::new(0),
            revoke_release: Notify::new(),
            budget: Mutex::new(None),
        }
    }
}
struct Backend(Arc<State>);
struct Browser(Arc<State>);
struct Login(Arc<State>);
struct Completed(Arc<State>);
impl GoogleInteractiveBackend for Backend {
    fn inspect<'a>(
        &'a self,
        context: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        Box::pin(async move {
            context.validate()?;
            Ok(GoogleAuthorizationProjection {
                state: GoogleInteractiveState::Ready,
                can_login: true,
                can_logout: false,
                message: "Controlled only".into(),
                authorization: None,
                affected_service_count: 0,
            })
        })
    }
    fn begin<'a>(
        &'a self,
        _: StartGoogleLoginInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move {
            self.0.begins.fetch_add(1, Ordering::SeqCst);
            *self.0.budget.lock().unwrap() = Some(budget);
            Ok(Box::new(Login(self.0.clone())) as Box<dyn OwnedGoogleLogin>)
        })
    }
    fn revoke<'a>(
        &'a self,
        _: RevokeGoogleAuthorizationInput,
        _: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async move {
            self.0.revokes.fetch_add(1, Ordering::SeqCst);
            self.0.revoke_release.notified().await;
            Ok(GoogleRevokeProjection {
                local_revoked: true,
                remote_revocation_confirmed: false,
                message: "controlled local only".into(),
            })
        })
    }
}
impl GoogleSystemBrowser for Browser {
    fn open<'a>(
        &'a self,
        _: &'a GoogleSystemBrowserIntent,
        _: &'a GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, ()> {
        Box::pin(async move {
            self.0.opens.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}
impl OwnedGoogleLogin for Login {
    fn browser_intent(&self) -> GoogleSystemBrowserIntent {
        GoogleSystemBrowserIntent::from_broker_url(
            "https://accounts.google.com/o/oauth2/v2/auth?code=never-in-ipc".into(),
        )
    }
    fn finish(self: Box<Self>) -> GoogleInteractiveFuture<'static, Box<dyn CompletedGoogleLogin>> {
        Box::pin(async move {
            self.0.release.notified().await;
            self.0.finished.store(true, Ordering::SeqCst);
            Ok(Box::new(Completed(self.0.clone())) as Box<dyn CompletedGoogleLogin>)
        })
    }
}
impl CompletedGoogleLogin for Completed {
    fn authorization(&self) -> GoogleAuthorizationRef {
        reference()
    }
    fn discard(self: Box<Self>) -> GoogleInteractiveFuture<'static, ()> {
        Box::pin(async move {
            if self.0.fail_cleanup.load(Ordering::SeqCst) {
                return Err(AppError::new(
                    "controlled_cleanup_failure",
                    "test",
                    "controlled",
                ));
            }
            self.0.discarded.store(true, Ordering::SeqCst);
            Ok(())
        })
    }
}
fn setup() -> (Arc<State>, Arc<GoogleInteractiveController>) {
    let state = Arc::new(State::default());
    let controller = Arc::new(GoogleInteractiveController::new(
        Arc::new(Backend(state.clone())),
        Arc::new(Browser(state.clone())),
    ));
    (state, controller)
}
async fn begun(state: &State) {
    for _ in 0..100 {
        if state.opens.load(Ordering::SeqCst) > 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("operation never began")
}
async fn terminal(c: &GoogleInteractiveController, id: &str) -> GoogleLoginOperationProjection {
    for _ in 0..100 {
        let v = c.status(query(id)).unwrap();
        if v.state != GoogleInteractiveState::Authorizing {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("operation never drained")
}
#[tokio::test]
async fn read_does_not_launch_and_duplicate_operation_opens_once() {
    let (s, c) = setup();
    c.inspect(context()).await.unwrap();
    assert_eq!(s.opens.load(Ordering::SeqCst), 0);
    c.start(input("one")).await.unwrap();
    c.start(input("one")).await.unwrap();
    begun(&s).await;
    assert_eq!(s.opens.load(Ordering::SeqCst), 1);
    let mut other = input("one");
    other.context.public_client_id = Some("other".into());
    assert_eq!(c.start(other).await.unwrap_err().code, "operation_conflict");
    s.release.notify_one();
    let v = terminal(&c, "one").await;
    assert_eq!(v.state, GoogleInteractiveState::Authorized);
    assert_eq!(v.authorization, Some(reference()));
    assert!(!serde_json::to_string(&v).unwrap().contains("never-in-ipc"));
    assert!(!s.discarded.load(Ordering::SeqCst));
    assert_eq!(
        c.cancel(query("one")).unwrap().state,
        GoogleInteractiveState::Authorized
    );
}
#[tokio::test]
async fn cancellation_does_not_claim_drained_before_exact_new_grant_cleanup() {
    let (s, c) = setup();
    c.start(input("cancel")).await.unwrap();
    begun(&s).await;
    assert_eq!(
        c.cancel(query("cancel")).unwrap().state,
        GoogleInteractiveState::Authorizing
    );
    assert!(!s.discarded.load(Ordering::SeqCst));
    s.release.notify_one();
    let v = terminal(&c, "cancel").await;
    assert_eq!(v.state, GoogleInteractiveState::Cancelled);
    assert!(s.discarded.load(Ordering::SeqCst));
    assert!(v.authorization.is_none());
}
#[tokio::test]
async fn cleanup_failure_remains_explicit_and_prevents_normal_exit() {
    let (s, c) = setup();
    s.fail_cleanup.store(true, Ordering::SeqCst);
    c.start(input("cleanup")).await.unwrap();
    begun(&s).await;
    c.cancel(query("cleanup")).unwrap();
    s.release.notify_one();
    assert_eq!(
        terminal(&c, "cleanup").await.state,
        GoogleInteractiveState::CleanupPending
    );
    assert_eq!(
        c.inspect(context()).await.unwrap().state,
        GoogleInteractiveState::CleanupPending
    );
    assert_eq!(
        c.start(input("cannot-bypass-cleanup"))
            .await
            .unwrap_err()
            .code,
        "google_login_cleanup_pending"
    );
    assert_eq!(
        c.stop_and_drain().await.unwrap_err().code,
        "google_login_cleanup_pending"
    );
}
#[tokio::test]
async fn real_shared_deadline_cannot_publish_late_grant() {
    let (s, c) = setup();
    c.start(input("timeout")).await.unwrap();
    begun(&s).await;
    s.budget
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .operation_budget()
        .restrict_timeout_from_start(Duration::from_millis(1))
        .ok();
    tokio::time::sleep(Duration::from_millis(3)).await;
    s.release.notify_one();
    assert_eq!(
        terminal(&c, "timeout").await.state,
        GoogleInteractiveState::TimedOut
    );
    assert!(s.discarded.load(Ordering::SeqCst));
}
#[tokio::test]
async fn foreign_generation_and_unknown_operation_never_obtain_result() {
    let (_, c) = setup();
    assert_eq!(
        c.status(query("unknown")).unwrap_err().code,
        "google_login_unknown"
    );
    c.start(input("profile")).await.unwrap();
    let mut other = query("profile");
    other.expected_profile_generation = NonZeroU32::new(2).unwrap();
    assert_eq!(
        c.cancel(other).unwrap_err().code,
        "profile_generation_conflict"
    );
}
#[tokio::test]
async fn dropped_revoke_awaiter_does_not_drop_physical_work() {
    let (s, c) = setup();
    let input = RevokeGoogleAuthorizationInput {
        operation_id: "revoke".into(),
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
        authorization: reference(),
        proxy: None,
    };
    let caller = {
        let c = c.clone();
        tokio::spawn(async move { c.revoke(input).await })
    };
    for _ in 0..100 {
        if s.revokes.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    caller.abort();
    let draining = {
        let c = c.clone();
        tokio::spawn(async move { c.stop_and_drain().await })
    };
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(!draining.is_finished());
    s.revoke_release.notify_one();
    draining.await.unwrap().unwrap();
}
#[test]
fn ui_inputs_reject_authority_or_secret_fields() {
    let mut v = serde_json::json!({"expectedProfileGeneration":1,"clientMode":"custom","publicClientId":"public"});
    for key in [
        "verified",
        "token",
        "authorizationUrl",
        "scope",
        "clientSecret",
    ] {
        v[key] = serde_json::Value::String("untrusted".into());
        assert!(serde_json::from_value::<GoogleInteractiveContext>(v.clone()).is_err());
        v.as_object_mut().unwrap().remove(key);
    }
}
#[test]
fn even_rejected_proxy_input_debug_does_not_expose_supplied_credentials() {
    let proxy = GoogleInteractiveProxy {
        mode: "override".into(),
        proxy_url: Some("http://user:secret@127.0.0.1:9000?token=private".into()),
    };
    let debug = format!("{proxy:?}");
    assert!(!debug.contains("secret"));
    assert!(!debug.contains("private"));
    assert!(!debug.contains("user:"));
}
struct BrokerCleanupFailure(Arc<State>);
impl GoogleInteractiveBackend for BrokerCleanupFailure {
    fn inspect<'a>(
        &'a self,
        context: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        Box::pin(async move { Backend(self.0.clone()).inspect(context).await })
    }
    fn begin<'a>(
        &'a self,
        _: StartGoogleLoginInput,
        _: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async {
            Err(AppError::new(
                "google_cleanup_pending",
                "google_authorization",
                "provider raw body must not be projected",
            ))
        })
    }
    fn revoke<'a>(
        &'a self,
        _: RevokeGoogleAuthorizationInput,
        _: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async { Err(AppError::new("not_used", "test", "not used")) })
    }
}
#[tokio::test]
async fn broker_postcommit_cleanup_failure_keeps_authority_blocked() {
    let state = Arc::new(State::default());
    let controller = Arc::new(GoogleInteractiveController::new(
        Arc::new(BrokerCleanupFailure(state.clone())),
        Arc::new(Browser(state)),
    ));
    controller.start(input("broker-cleanup")).await.unwrap();
    let result = terminal(&controller, "broker-cleanup").await;
    assert_eq!(result.state, GoogleInteractiveState::CleanupPending);
    assert!(!result.message.contains("raw body"));
    assert!(result.authorization.is_none());
    assert!(!controller.inspect(context()).await.unwrap().can_login);
    assert_eq!(
        controller.stop_and_drain().await.unwrap_err().code,
        "google_login_cleanup_pending"
    );
}
