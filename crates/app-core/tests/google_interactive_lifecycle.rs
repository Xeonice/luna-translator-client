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

struct InspectionBarrier {
    state: Arc<State>,
    count: AtomicUsize,
    entered: Notify,
    release: Notify,
}
impl GoogleInteractiveBackend for InspectionBarrier {
    fn inspect<'a>(
        &'a self,
        c: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        Box::pin(async move {
            if self.count.fetch_add(1, Ordering::SeqCst) == 1 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Backend(self.state.clone()).inspect(c).await
        })
    }
    fn begin<'a>(
        &'a self,
        i: StartGoogleLoginInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move { Backend(self.state.clone()).begin(i, b).await })
    }
    fn revoke<'a>(
        &'a self,
        i: RevokeGoogleAuthorizationInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async move { Backend(self.state.clone()).revoke(i, b).await })
    }
}
#[tokio::test]
async fn independent_cleanup_pending_wins_while_new_operation_waits_for_inspect() {
    let state = Arc::new(State::default());
    state.fail_cleanup.store(true, Ordering::SeqCst);
    let backend = Arc::new(InspectionBarrier {
        state: state.clone(),
        count: AtomicUsize::new(0),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let c = Arc::new(GoogleInteractiveController::new(
        backend.clone(),
        Arc::new(Browser(state.clone())),
    ));
    c.start(input("first-mint")).await.unwrap();
    begun(&state).await;
    let b = c.clone();
    let second = tokio::spawn(async move { b.start(input("new-id-same-client")).await });
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    c.cancel(query("first-mint")).unwrap();
    state.release.notify_one();
    let first = terminal(&c, "first-mint").await;
    assert_eq!(first.state, GoogleInteractiveState::CleanupPending);
    backend.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(1), second)
        .await
        .unwrap()
        .unwrap();
    if result.is_ok() {
        let _ = c.cancel(query("new-id-same-client"));
        state.release.notify_one();
    }
    assert_eq!(
        result.unwrap_err().code,
        "google_login_cleanup_pending",
        "new ID must not bypass cleanup which won during inspect await"
    );
}

struct PerFlow {
    id: String,
    release: Notify,
    fail_discard: bool,
    discarded: AtomicBool,
}
struct PerFlowBackend {
    a: Arc<PerFlow>,
    b: Arc<PerFlow>,
}
struct PerFlowLogin(Arc<PerFlow>);
struct PerFlowCompleted(Arc<PerFlow>);
impl GoogleInteractiveBackend for PerFlowBackend {
    fn inspect<'a>(
        &'a self,
        _: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        Box::pin(async {
            Ok(GoogleAuthorizationProjection {
                state: GoogleInteractiveState::Ready,
                can_login: true,
                can_logout: false,
                message: "controlled only".into(),
                authorization: None,
                affected_service_count: 0,
            })
        })
    }
    fn begin<'a>(
        &'a self,
        i: StartGoogleLoginInput,
        _: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        let f = if i.operation_id == "a" {
            self.a.clone()
        } else {
            self.b.clone()
        };
        Box::pin(async move { Ok(Box::new(PerFlowLogin(f)) as Box<dyn OwnedGoogleLogin>) })
    }
    fn revoke<'a>(
        &'a self,
        _: RevokeGoogleAuthorizationInput,
        _: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async { Err(AppError::new("not_used", "test", "not used")) })
    }
}
impl OwnedGoogleLogin for PerFlowLogin {
    fn browser_intent(&self) -> GoogleSystemBrowserIntent {
        GoogleSystemBrowserIntent::from_broker_url(
            "https://accounts.google.com/o/oauth2/v2/auth?controlled".into(),
        )
    }
    fn finish(self: Box<Self>) -> GoogleInteractiveFuture<'static, Box<dyn CompletedGoogleLogin>> {
        Box::pin(async move {
            self.0.release.notified().await;
            Ok(Box::new(PerFlowCompleted(self.0)) as Box<dyn CompletedGoogleLogin>)
        })
    }
}
impl CompletedGoogleLogin for PerFlowCompleted {
    fn authorization(&self) -> GoogleAuthorizationRef {
        GoogleAuthorizationRef {
            grant_id: format!("new-{}", self.0.id),
            authorization_epoch: NonZeroU32::new(1).unwrap(),
        }
    }
    fn discard(self: Box<Self>) -> GoogleInteractiveFuture<'static, ()> {
        Box::pin(async move {
            if self.0.fail_discard {
                return Err(AppError::new(
                    "controlled_discard_failure",
                    "test",
                    "controlled",
                ));
            }
            self.0.discarded.store(true, Ordering::SeqCst);
            Ok(())
        })
    }
}
#[tokio::test]
async fn independent_already_admitted_flow_cannot_publish_success_after_same_client_cleanup_pending()
 {
    let a = Arc::new(PerFlow {
        id: "a".into(),
        release: Notify::new(),
        fail_discard: true,
        discarded: AtomicBool::new(false),
    });
    let b = Arc::new(PerFlow {
        id: "b".into(),
        release: Notify::new(),
        fail_discard: false,
        discarded: AtomicBool::new(false),
    });
    let state = Arc::new(State::default());
    let c = Arc::new(GoogleInteractiveController::new(
        Arc::new(PerFlowBackend {
            a: a.clone(),
            b: b.clone(),
        }),
        Arc::new(Browser(state.clone())),
    ));
    c.start(input("a")).await.unwrap();
    c.start(input("b")).await.unwrap();
    for _ in 0..100 {
        if state.opens.load(Ordering::SeqCst) == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(state.opens.load(Ordering::SeqCst), 2);
    c.cancel(query("a")).unwrap();
    a.release.notify_one();
    assert_eq!(
        terminal(&c, "a").await.state,
        GoogleInteractiveState::CleanupPending
    );
    b.release.notify_one();
    let result = terminal(&c, "b").await;
    assert_ne!(
        result.state,
        GoogleInteractiveState::Authorized,
        "known unsafe same-client cleanup must not be hidden by another operation's success"
    );
    assert!(result.authorization.is_none());
    assert!(b.discarded.load(Ordering::SeqCst));
}

struct InitialInspectBarrier {
    state: Arc<State>,
    entered: Notify,
    release: Notify,
}
impl GoogleInteractiveBackend for InitialInspectBarrier {
    fn inspect<'a>(
        &'a self,
        c: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        Box::pin(async move {
            self.entered.notify_one();
            self.release.notified().await;
            Backend(self.state.clone()).inspect(c).await
        })
    }
    fn begin<'a>(
        &'a self,
        i: StartGoogleLoginInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move { Backend(self.state.clone()).begin(i, b).await })
    }
    fn revoke<'a>(
        &'a self,
        i: RevokeGoogleAuthorizationInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async move { Backend(self.state.clone()).revoke(i, b).await })
    }
}
#[tokio::test]
async fn independent_cancel_during_registered_request_preflight_cannot_be_lost() {
    let state = Arc::new(State::default());
    let backend = Arc::new(InitialInspectBarrier {
        state: state.clone(),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let c = Arc::new(GoogleInteractiveController::new(
        backend.clone(),
        Arc::new(Browser(state.clone())),
    ));
    let start = {
        let c = c.clone();
        tokio::spawn(async move { c.start(input("preflight-cancel")).await })
    };
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    let cancelled = c.cancel(query("preflight-cancel"));
    backend.release.notify_one();
    let _ = start.await.unwrap();
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let opened = state.opens.load(Ordering::SeqCst);
    if opened > 0 {
        c.cancel(query("preflight-cancel")).unwrap();
        state.release.notify_one();
        let _ = terminal(&c, "preflight-cancel").await;
    }
    assert!(
        cancelled.is_ok(),
        "an explicit in-flight start must be cancellable during preflight, got {cancelled:?}"
    );
    assert_eq!(
        opened, 0,
        "cancel sent before preflight completion must stop browser opening"
    );
}
struct PhysicalInspect {
    state: Arc<State>,
    stage: Arc<(Mutex<u8>, std::sync::Condvar)>,
}
impl GoogleInteractiveBackend for PhysicalInspect {
    fn inspect<'a>(
        &'a self,
        c: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        let stage = self.stage.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let (lock, signal) = &*stage;
                let mut value = lock.lock().unwrap();
                *value = 1;
                signal.notify_all();
                while *value < 2 {
                    value = signal.wait(value).unwrap();
                }
            })
            .await
            .unwrap();
            Backend(self.state.clone()).inspect(c).await
        })
    }
    fn begin<'a>(
        &'a self,
        i: StartGoogleLoginInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move { Backend(self.state.clone()).begin(i, b).await })
    }
    fn revoke<'a>(
        &'a self,
        i: RevokeGoogleAuthorizationInput,
        b: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async move { Backend(self.state.clone()).revoke(i, b).await })
    }
}
#[tokio::test]
async fn independent_shutdown_waits_real_preflight_blocking_work_even_after_start_awaiter_drop() {
    let state = Arc::new(State::default());
    let stage = Arc::new((Mutex::new(0u8), std::sync::Condvar::new()));
    let c = Arc::new(GoogleInteractiveController::new(
        Arc::new(PhysicalInspect {
            state: state.clone(),
            stage: stage.clone(),
        }),
        Arc::new(Browser(state)),
    ));
    let caller = {
        let c = c.clone();
        tokio::spawn(async move { c.start(input("preflight-physical")).await })
    };
    for _ in 0..200 {
        if *stage.0.lock().unwrap() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(*stage.0.lock().unwrap(), 1);
    caller.abort();
    let _ = caller.await;
    let drain = {
        let c = c.clone();
        tokio::spawn(async move { c.stop_and_drain().await })
    };
    tokio::time::sleep(Duration::from_millis(5)).await;
    let falsely_drained = drain.is_finished();
    {
        let mut value = stage.0.lock().unwrap();
        *value = 2;
        stage.1.notify_all();
    }
    drain.await.unwrap().unwrap();
    assert!(
        !falsely_drained,
        "physical preflight still ran while stop_and_drain declared Google work drained"
    );
}

/// A real blocking preflight, before begin() receives any grant authority.
struct PreflightGate {
    state: Arc<State>,
    stage: Arc<(Mutex<u8>, std::sync::Condvar)>,
    inspections: AtomicUsize,
}
impl GoogleInteractiveBackend for PreflightGate {
    fn inspect<'a>(
        &'a self,
        context: GoogleInteractiveContext,
    ) -> GoogleInteractiveFuture<'a, GoogleAuthorizationProjection> {
        self.inspections.fetch_add(1, Ordering::SeqCst);
        let stage = self.stage.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let (lock, signal) = &*stage;
                let mut value = lock.lock().unwrap();
                if *value == 0 {
                    *value = 1;
                }
                signal.notify_all();
                while *value < 2 {
                    value = signal.wait(value).unwrap();
                }
                *value = 3;
            })
            .await
            .unwrap();
            Backend(self.state.clone()).inspect(context).await
        })
    }
    fn begin<'a>(
        &'a self,
        input: StartGoogleLoginInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, Box<dyn OwnedGoogleLogin>> {
        Box::pin(async move { Backend(self.state.clone()).begin(input, budget).await })
    }
    fn revoke<'a>(
        &'a self,
        input: RevokeGoogleAuthorizationInput,
        budget: GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, GoogleRevokeProjection> {
        Box::pin(async move { Backend(self.state.clone()).revoke(input, budget).await })
    }
}
fn preflight_setup() -> (
    Arc<State>,
    Arc<PreflightGate>,
    Arc<GoogleInteractiveController>,
) {
    let state = Arc::new(State::default());
    let gate = Arc::new(PreflightGate {
        state: state.clone(),
        stage: Arc::new((Mutex::new(0), std::sync::Condvar::new())),
        inspections: AtomicUsize::new(0),
    });
    let controller = Arc::new(GoogleInteractiveController::new(
        gate.clone(),
        Arc::new(Browser(state.clone())),
    ));
    (state, gate, controller)
}
async fn preflight_entered(gate: &PreflightGate) -> bool {
    for _ in 0..500 {
        if *gate.stage.0.lock().unwrap() == 1 {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    false
}
fn release_preflight(gate: &PreflightGate) {
    *gate.stage.0.lock().unwrap() = 2;
    gate.stage.1.notify_all();
}

#[tokio::test]
async fn identical_start_replay_while_preparing_uses_one_inspection_and_one_flow() {
    let (state, gate, controller) = preflight_setup();
    let first = {
        let controller = controller.clone();
        tokio::spawn(async move { controller.start(input("same-preflight")).await })
    };
    let entered = preflight_entered(&gate).await;
    let second = {
        let controller = controller.clone();
        tokio::spawn(async move { controller.start(input("same-preflight")).await })
    };
    tokio::time::sleep(Duration::from_millis(5)).await;
    let inspections = gate.inspections.load(Ordering::SeqCst);
    release_preflight(&gate);
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    begun(&state).await;
    controller.cancel(query("same-preflight")).unwrap();
    state.release.notify_one();
    terminal(&controller, "same-preflight").await;
    assert!(entered);
    assert_eq!(inspections, 1);
    assert_eq!(gate.inspections.load(Ordering::SeqCst), 1);
    assert_eq!(state.begins.load(Ordering::SeqCst), 1);
    assert_eq!(state.opens.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller
            .cancel(query("arbitrary-id-not-started"))
            .unwrap_err()
            .code,
        "google_login_unknown"
    );
}
