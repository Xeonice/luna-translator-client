//! Explicit, application-owned Google desktop login. No webview owns a flow.
use crate::{
    contracts::*,
    ports::{google_authorization::GoogleBudget, google_interactive::*},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;
const CAPACITY: usize = 32;
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_interactive", message)
}
struct Operation {
    input: StartGoogleLoginInput,
    budget: GoogleBudget,
    view: Mutex<GoogleLoginOperationProjection>,
    drained: watch::Sender<bool>,
    prepared: watch::Sender<Option<AppResult<()>>>,
}
struct RevokeOperation {
    input: RevokeGoogleAuthorizationInput,
    budget: GoogleBudget,
    result: Mutex<Option<AppResult<GoogleRevokeProjection>>>,
    drained: watch::Sender<bool>,
}
struct MaterialRecovery {
    drained: watch::Sender<bool>,
}
pub struct GoogleInteractiveController {
    backend: Arc<dyn GoogleInteractiveBackend>,
    browser: Arc<dyn GoogleSystemBrowser>,
    operations: Mutex<BTreeMap<String, Arc<Operation>>>,
    revocations: Mutex<BTreeMap<String, Arc<RevokeOperation>>>,
    recovery: Mutex<Option<Arc<MaterialRecovery>>>,
    closing: std::sync::atomic::AtomicBool,
    timeout: Duration,
}
impl GoogleInteractiveController {
    pub fn new(
        backend: Arc<dyn GoogleInteractiveBackend>,
        browser: Arc<dyn GoogleSystemBrowser>,
    ) -> Self {
        Self {
            backend,
            browser,
            operations: Mutex::new(BTreeMap::new()),
            revocations: Mutex::new(BTreeMap::new()),
            recovery: Mutex::new(None),
            closing: std::sync::atomic::AtomicBool::new(false),
            timeout: Duration::from_secs(300),
        }
    }
    pub async fn inspect(
        &self,
        context: GoogleInteractiveContext,
    ) -> AppResult<GoogleAuthorizationProjection> {
        context.validate()?;
        if self.cleanup_pending(&context)? {
            return Ok(GoogleAuthorizationProjection {
                state: GoogleInteractiveState::CleanupPending,
                can_login: false,
                can_logout: false,
                message: "本次客户端的新授权仍待安全清理，暂不能重新登录".into(),
                authorization: None,
                affected_service_count: 0,
            });
        }
        self.backend.inspect(context).await
    }
    fn cleanup_pending(&self, context: &GoogleInteractiveContext) -> AppResult<bool> {
        let operations = self
            .operations
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
        Self::cleanup_pending_in(&operations, context)
    }
    // Admission and publication share this map lock. An awaited backend check
    // is not authority to ignore a cleanup failure published in the meantime.
    fn cleanup_pending_in(
        operations: &BTreeMap<String, Arc<Operation>>,
        context: &GoogleInteractiveContext,
    ) -> AppResult<bool> {
        for operation in operations.values() {
            let previous = &operation.input.context;
            if previous.expected_profile_generation == context.expected_profile_generation
                && previous.client_mode == context.client_mode
                && previous.public_client_id.as_deref().map(str::trim)
                    == context.public_client_id.as_deref().map(str::trim)
                && Self::view(operation)?.state == GoogleInteractiveState::CleanupPending
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub async fn start(
        self: &Arc<Self>,
        input: StartGoogleLoginInput,
    ) -> AppResult<GoogleLoginOperationProjection> {
        crate::domain::require_id(&input.operation_id)?;
        input.context.validate()?;
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| failure("google_executor_unavailable", "授权执行器尚未就绪"))?;
        // Register the original budget and physical drain BEFORE the first
        // awaited repository check. The IPC awaiter owns only a receiver.
        let (operation, fresh) = {
            let mut operations = self
                .operations
                .lock()
                .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
            if self.closing.load(std::sync::atomic::Ordering::Acquire) {
                return Err(failure("application_closing", "应用正在退出"));
            }
            if let Some(previous) = operations.get(&input.operation_id) {
                if previous.input != input {
                    return Err(failure("operation_conflict", "操作标识已用于其他授权内容"));
                }
                (previous.clone(), false)
            } else {
                if Self::cleanup_pending_in(&operations, &input.context)? {
                    return Err(failure(
                        "google_login_cleanup_pending",
                        "本次客户端的新授权仍待安全清理",
                    ));
                }
                if operations.len() >= CAPACITY {
                    let terminal = operations
                        .iter()
                        .find(|(_, v)| {
                            *v.drained.borrow()
                                && v.view.lock().is_ok_and(|v| {
                                    v.state != GoogleInteractiveState::CleanupPending
                                })
                        })
                        .map(|(k, _)| k.clone());
                    if let Some(key) = terminal {
                        operations.remove(&key);
                    } else {
                        return Err(failure(
                            "google_login_capacity",
                            "授权操作过多，请等待当前操作结束",
                        ));
                    }
                }
                let (drained, _) = watch::channel(false);
                let (prepared, _) = watch::channel(None);
                let operation = Arc::new(Operation {
                    input: input.clone(),
                    budget: GoogleBudget::new(self.timeout)?,
                    view: Mutex::new(GoogleLoginOperationProjection {
                        operation_id: input.operation_id.clone(),
                        state: GoogleInteractiveState::Authorizing,
                        message: "正在核对本次授权配置；尚未打开系统浏览器".into(),
                        authorization: None,
                        can_cancel: true,
                    }),
                    drained,
                    prepared,
                });
                operations.insert(input.operation_id.clone(), operation.clone());
                (operation, true)
            }
        };
        if fresh {
            let controller = self.clone();
            let running = operation.clone();
            runtime.spawn(async move {
                controller.prepare_and_run(running).await;
            });
        }
        let mut prepared = operation.prepared.subscribe();
        loop {
            let result = prepared.borrow_and_update().clone();
            if let Some(result) = result {
                result?;
                return Self::view(&operation);
            }
            prepared
                .changed()
                .await
                .map_err(|_| failure("google_preflight_state", "授权准备状态不可核验"))?;
        }
    }
    async fn prepare_and_run(self: Arc<Self>, operation: Arc<Operation>) {
        let preparation = async {
            operation.budget.check()?;
            // Await the actual backend future, including its blocking SQL join.
            // Cancelling the budget or dropping start() must not detach this I/O.
            let projection = self.backend.inspect(operation.input.context.clone()).await;
            operation.budget.check()?;
            let projection = projection?;
            if !projection.can_login {
                return Err(failure("google_login_not_ready", &projection.message));
            }
            let operations = self
                .operations
                .lock()
                .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
            operation.budget.check()?;
            if self.closing.load(std::sync::atomic::Ordering::Acquire) {
                return Err(failure("application_closing", "应用正在退出"));
            }
            // Preserve the same lock used by cleanup-pending publication. A
            // result read before an await cannot admit a new unsafe flow.
            if Self::cleanup_pending_in(&operations, &operation.input.context)? {
                return Err(failure(
                    "google_login_cleanup_pending",
                    "本次客户端的新授权仍待安全清理",
                ));
            }
            let mut view = operation
                .view
                .lock()
                .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
            view.message = "正在通过系统浏览器授权；完成后仍需保存配置".into();
            Ok(())
        }
        .await;
        match preparation {
            Ok(()) => {
                operation.prepared.send_replace(Some(Ok(())));
                self.run(operation).await;
            }
            Err(error) => {
                self.publish_failure(&operation, &error);
                // The preflight I/O has physically returned and the safe view
                // exists before either the start reply or drain is announced.
                operation.drained.send_replace(true);
                operation.prepared.send_replace(Some(Err(error)));
            }
        }
    }
    async fn run(self: Arc<Self>, operation: Arc<Operation>) {
        let outcome = async {
            operation.budget.check()?;
            let login = self
                .backend
                .begin(operation.input.clone(), operation.budget.clone())
                .await?;
            operation.budget.check()?;
            self.browser
                .open(&login.browser_intent(), &operation.budget)
                .await?;
            let completed = login.finish().await?;
            let reference = completed.authorization();
            // The broker already committed the exact new grant. Cancellation can
            // win in this gap: cleanup must finish BEFORE a stopped UI terminal.
            let publication = operation.budget.with_terminal_fence(|| {
                let operations = self
                    .operations
                    .lock()
                    .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
                if Self::cleanup_pending_in(&operations, &operation.input.context)? {
                    return Err(failure(
                        "google_login_cleanup_pending",
                        "本次客户端的新授权仍待安全清理",
                    ));
                }
                let mut view = operation
                    .view
                    .lock()
                    .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
                view.state = GoogleInteractiveState::Authorized;
                view.message = "Google授权完成；尚未保存或选用此服务".into();
                view.authorization = Some(reference);
                view.can_cancel = false;
                Ok(())
            });
            if let Err(error) = publication {
                if completed.discard().await.is_err() {
                    let _operations = self.operations.lock().unwrap_or_else(|e| e.into_inner());
                    let mut view = operation.view.lock().unwrap_or_else(|e| e.into_inner());
                    view.state = GoogleInteractiveState::CleanupPending;
                    view.message = "本次新授权尚待安全清理，请保留应用并重新核对".into();
                    view.authorization = None;
                    view.can_cancel = false;
                    return Ok(());
                }
                return Err(error);
            }
            Ok(())
        }
        .await;
        if let Err(error) = outcome {
            self.publish_failure(&operation, &error);
        }
        // This signal follows all owned SQL/Vault/blocking worker completion.
        operation.drained.send_replace(true);
    }
    fn publish_failure(&self, operation: &Operation, error: &AppError) {
        // Only fixed application messages reach IPC, never provider bodies.
        let (state, message) = match error.code.as_str() {
            "authorization_cancelled" | "operation_cancelled" => (
                GoogleInteractiveState::Cancelled,
                "授权已取消，本次本机新授权已停用",
            ),
            "authorization_timeout" | "operation_timeout" => (
                GoogleInteractiveState::TimedOut,
                "授权已超时，本次本机新授权已停用",
            ),
            "google_cleanup_pending" => (
                GoogleInteractiveState::CleanupPending,
                "本次新授权尚待安全清理，请保留应用并重新核对",
            ),
            "google_oauth_denied" => (GoogleInteractiveState::Failed, "Google授权未获同意"),
            _ => (
                GoogleInteractiveState::Failed,
                "Google授权未完成，请检查公开客户端配置和网络后重试",
            ),
        };
        let _publication = if state == GoogleInteractiveState::CleanupPending {
            Some(self.operations.lock().unwrap_or_else(|e| e.into_inner()))
        } else {
            None
        };
        let mut view = operation.view.lock().unwrap_or_else(|e| e.into_inner());
        view.state = state;
        view.message = message.into();
        view.authorization = None;
        view.can_cancel = false;
    }
    fn operation(&self, query: &GoogleLoginOperationQuery) -> AppResult<Arc<Operation>> {
        crate::domain::require_id(&query.operation_id)?;
        let operation = self
            .operations
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?
            .get(&query.operation_id)
            .cloned()
            .ok_or_else(|| failure("google_login_unknown", "此授权操作不存在或已随应用重启结束"))?;
        if operation.input.context.expected_profile_generation != query.expected_profile_generation
        {
            return Err(failure(
                "profile_generation_conflict",
                "授权不属于当前配置档",
            ));
        }
        Ok(operation)
    }
    fn view(operation: &Operation) -> AppResult<GoogleLoginOperationProjection> {
        operation
            .view
            .lock()
            .map(|v| v.clone())
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))
    }
    pub fn status(
        &self,
        query: GoogleLoginOperationQuery,
    ) -> AppResult<GoogleLoginOperationProjection> {
        Self::view(self.operation(&query)?.as_ref())
    }
    pub fn cancel(
        &self,
        query: GoogleLoginOperationQuery,
    ) -> AppResult<GoogleLoginOperationProjection> {
        let operation = self.operation(&query)?;
        operation.budget.cancel();
        let mut view = operation
            .view
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
        if !*operation.drained.borrow() && view.state == GoogleInteractiveState::Authorizing {
            view.can_cancel = false;
            view.message = "正在取消并等待本次授权安全收尾".into();
        }
        Ok(view.clone())
    }
    pub async fn revoke(
        self: &Arc<Self>,
        input: RevokeGoogleAuthorizationInput,
    ) -> AppResult<GoogleRevokeProjection> {
        crate::domain::require_id(&input.operation_id)?;
        crate::domain::require_id(&input.authorization.grant_id)?;
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| failure("google_executor_unavailable", "授权执行器尚未就绪"))?;
        let (operation, fresh) = {
            let mut revocations = self
                .revocations
                .lock()
                .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
            if self.closing.load(std::sync::atomic::Ordering::Acquire) {
                return Err(failure("application_closing", "应用正在退出"));
            }
            if let Some(previous) = revocations.get(&input.operation_id) {
                if previous.input != input {
                    return Err(failure("operation_conflict", "操作标识已用于其他撤权内容"));
                }
                (previous.clone(), false)
            } else {
                if revocations.len() >= CAPACITY {
                    let terminal = revocations
                        .iter()
                        .find(|(_, v)| *v.drained.borrow())
                        .map(|(k, _)| k.clone());
                    if let Some(key) = terminal {
                        revocations.remove(&key);
                    } else {
                        return Err(failure("google_login_capacity", "授权操作过多，请稍后重试"));
                    }
                }
                let (drained, _) = watch::channel(false);
                let operation = Arc::new(RevokeOperation {
                    input: input.clone(),
                    budget: GoogleBudget::new(Duration::from_secs(30))?,
                    result: Mutex::new(None),
                    drained,
                });
                revocations.insert(input.operation_id.clone(), operation.clone());
                (operation, true)
            }
        };
        if fresh {
            let controller = self.clone();
            let running = operation.clone();
            // Dropping the IPC awaiter cannot drop the local-first revoke or its
            // actual blocking join. Shutdown observes this app-owned completion.
            handle.spawn(async move {
                let result = controller
                    .backend
                    .revoke(running.input.clone(), running.budget.clone())
                    .await;
                *running.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                running.drained.send_replace(true);
            });
        }
        let mut done = operation.drained.subscribe();
        while !*done.borrow_and_update() {
            done.changed()
                .await
                .map_err(|_| failure("google_revoke_drain", "撤权收尾状态不可核验"))?;
        }
        operation
            .result
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?
            .clone()
            .ok_or_else(|| failure("google_revoke_drain", "撤权结果尚未确认"))?
    }
    pub fn start_material_recovery(
        self: &Arc<Self>,
        application: std::sync::Weak<super::runtime::ApplicationRuntime>,
    ) -> AppResult<()> {
        let executor = tokio::runtime::Handle::try_current()
            .map_err(|_| failure("google_executor_unavailable", "授权执行器尚未就绪"))?;
        let recovery = {
            let mut slot = self
                .recovery
                .lock()
                .map_err(|_| failure("google_operation_state", "授权恢复状态不可核验"))?;
            if self.closing.load(std::sync::atomic::Ordering::Acquire) {
                return Err(failure("application_closing", "应用正在退出"));
            }
            if slot.is_some() {
                return Ok(());
            }
            let (drained, _) = watch::channel(false);
            let recovery = Arc::new(MaterialRecovery { drained });
            *slot = Some(recovery.clone());
            recovery
        };
        let controller = self.clone();
        executor.spawn(async move {
            if controller.backend.recover_abandoned().await.is_err()
                && let Some(application) = application.upgrade() {
                    // Failure is a durable inactive allocation, not a reason to
                    // stop Hook or restart the app. Await the diagnostic writer
                    // too before announcing physical recovery drain.
                    let _ = tokio::task::spawn_blocking(move || {
                        application.report_native_effect_failure(failure(
                            "google_material_cleanup_pending",
                            "历史未激活Google材料尚待安全清理；取文继续运行",
                        ))
                    })
                    .await;
                }
            recovery.drained.send_replace(true);
        });
        Ok(())
    }
    pub fn reopen_after_failed_shutdown(&self) {
        self.closing
            .store(false, std::sync::atomic::Ordering::Release);
    }
    pub async fn stop_and_drain(&self) -> AppResult<()> {
        let operations = {
            let values = self
                .operations
                .lock()
                .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
            self.closing
                .store(true, std::sync::atomic::Ordering::Release);
            values.values().cloned().collect::<Vec<_>>()
        };
        let revocations = self
            .revocations
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let recovery = self
            .recovery
            .lock()
            .map_err(|_| failure("google_operation_state", "授权恢复状态不可核验"))?
            .clone();
        for operation in &operations {
            operation.budget.cancel();
        }
        for operation in &revocations {
            operation.budget.cancel();
        }
        let drain = async {
            for operation in &operations {
                let mut done = operation.drained.subscribe();
                while !*done.borrow_and_update() {
                    done.changed()
                        .await
                        .map_err(|_| failure("google_login_drain", "授权收尾状态不可核验"))?;
                }
                if Self::view(operation)?.state == GoogleInteractiveState::CleanupPending {
                    return Err(failure(
                        "google_login_cleanup_pending",
                        "授权清理尚未完成，暂不退出",
                    ));
                }
            }
            for operation in &revocations {
                let mut done = operation.drained.subscribe();
                while !*done.borrow_and_update() {
                    done.changed()
                        .await
                        .map_err(|_| failure("google_revoke_drain", "撤权收尾状态不可核验"))?;
                }
            }
            if let Some(recovery) = recovery {
                let mut done = recovery.drained.subscribe();
                while !*done.borrow_and_update() {
                    done.changed().await.map_err(|_| {
                        failure("google_material_recovery_drain", "授权材料恢复仍在收尾")
                    })?;
                }
            }
            Ok(())
        };
        tokio::time::timeout(Duration::from_secs(5), drain)
            .await
            .map_err(|_| failure("google_login_drain_timeout", "授权仍在安全收尾，暂不退出"))?
    }
}
impl super::runtime::ApplicationRuntime {
    pub fn start_google_material_recovery(self: &Arc<Self>) -> AppResult<()> {
        self.google_controller()?
            .start_material_recovery(Arc::downgrade(self))
    }
    pub fn configure_google_interactive(
        &self,
        controller: Arc<GoogleInteractiveController>,
    ) -> AppResult<()> {
        let mut slot = self
            .google_interactive
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?;
        if slot.is_some() {
            return Err(failure("google_interactive_configured", "授权交互已配置"));
        }
        *slot = Some(controller);
        Ok(())
    }
    fn google_controller(&self) -> AppResult<Arc<GoogleInteractiveController>> {
        self.google_interactive
            .lock()
            .map_err(|_| failure("google_operation_state", "授权操作状态不可核验"))?
            .clone()
            .ok_or_else(|| {
                failure(
                    "google_interactive_unavailable",
                    "此版本未安装Google授权交互",
                )
            })
    }
    pub async fn inspect_google_authorization(
        &self,
        context: GoogleInteractiveContext,
    ) -> AppResult<GoogleAuthorizationProjection> {
        self.ensure_service_profile_generation(context.expected_profile_generation)?;
        self.google_controller()?.inspect(context).await
    }
    pub async fn start_google_login(
        &self,
        input: StartGoogleLoginInput,
    ) -> AppResult<GoogleLoginOperationProjection> {
        // The registered preflight validates the actual profile/frame/handler.
        // A synchronous snapshot here would precede operation ownership again.
        self.google_controller()?.start(input).await
    }
    pub fn google_login_status(
        &self,
        query: GoogleLoginOperationQuery,
    ) -> AppResult<GoogleLoginOperationProjection> {
        self.ensure_service_profile_generation(query.expected_profile_generation)?;
        self.google_controller()?.status(query)
    }
    pub fn cancel_google_login(
        &self,
        query: GoogleLoginOperationQuery,
    ) -> AppResult<GoogleLoginOperationProjection> {
        self.ensure_service_profile_generation(query.expected_profile_generation)?;
        self.google_controller()?.cancel(query)
    }
    pub async fn revoke_google_authorization(
        &self,
        input: RevokeGoogleAuthorizationInput,
    ) -> AppResult<GoogleRevokeProjection> {
        self.ensure_service_profile_generation(input.expected_profile_generation)?;
        self.google_controller()?.revoke(input).await
    }
}
