//! One process coordinator: prepare system effects, commit durable intent, then retire old registrations.
use app_core::contracts::window_capabilities::{
    ShortcutBindingProjection, ShortcutIntent, ShortcutProjection, ShortcutState,
};
use app_core::contracts::{AppError, AppResult};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
const INTENTS: [ShortcutIntent; 3] = [
    ShortcutIntent::ToggleHud,
    ShortcutIntent::ToggleCapturePause,
    ShortcutIntent::CycleReadingDisplayMode,
];
fn key(intent: ShortcutIntent) -> &'static str {
    match intent {
        ShortcutIntent::ToggleHud => "toggleHud",
        ShortcutIntent::ToggleCapturePause => "togglePause",
        ShortcutIntent::CycleReadingDisplayMode => "cycleDisplayMode",
    }
}
fn error(code: &str, message: impl Into<String>) -> AppError {
    AppError::new(code, "shortcut", message)
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Accelerator {
    canonical: String,
    key: u32,
    modifiers: u32,
}
fn parse(text: &str) -> Result<Option<Accelerator>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<_> = text.split('+').map(str::trim).collect();
    if parts.len() < 2 || parts.len() > 5 {
        return Err("请使用修饰键与一个字母、数字或功能键".into());
    }
    let mut modifiers = 0;
    for part in &parts[..parts.len() - 1] {
        let flag = match part.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "cmd" | "command" | "super" | "meta" => 1,
            "alt" | "option" => 2,
            "ctrl" | "control" => 4,
            "shift" => 8,
            _ => return Err("无法识别快捷键修饰键".into()),
        };
        if modifiers & flag != 0 {
            return Err("快捷键不能重复修饰键".into());
        }
        modifiers |= flag;
    }
    if modifiers & 7 == 0 {
        return Err("全局快捷键至少需要 Command、Option 或 Control".into());
    }
    let label = parts.last().unwrap().to_ascii_uppercase();
    let keycode = match label.as_str() {
        "A" => 0,
        "S" => 1,
        "D" => 2,
        "F" => 3,
        "H" => 4,
        "G" => 5,
        "Z" => 6,
        "X" => 7,
        "C" => 8,
        "V" => 9,
        "B" => 11,
        "Q" => 12,
        "W" => 13,
        "E" => 14,
        "R" => 15,
        "Y" => 16,
        "T" => 17,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "6" => 22,
        "5" => 23,
        "9" => 25,
        "7" => 26,
        "8" => 28,
        "0" => 29,
        "O" => 31,
        "U" => 32,
        "I" => 34,
        "P" => 35,
        "L" => 37,
        "J" => 38,
        "K" => 40,
        "N" => 45,
        "M" => 46,
        "F1" => 122,
        "F2" => 120,
        "F3" => 99,
        "F4" => 118,
        "F5" => 96,
        "F6" => 97,
        "F7" => 98,
        "F8" => 100,
        "F9" => 101,
        "F10" => 109,
        "F11" => 103,
        "F12" => 111,
        _ => return Err("本机快捷键仅支持字母、数字与 F1–F12".into()),
    };
    let mut names = Vec::new();
    for (flag, name) in [(1, "Super"), (2, "Alt"), (4, "Control"), (8, "Shift")] {
        if modifiers & flag != 0 {
            names.push(name.to_owned());
        }
    }
    names.push(label);
    Ok(Some(Accelerator {
        canonical: names.join("+"),
        key: keycode,
        modifiers,
    }))
}
trait Driver: Send + Sync {
    fn register(&self, id: u32, value: &Accelerator) -> Result<(), String>;
    fn unregister(&self, id: u32) -> Result<(), String>;
}
struct NativeDriver;
impl Driver for NativeDriver {
    fn register(&self, id: u32, value: &Accelerator) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            super::macos::register_hotkey(id, value.key, value.modifiers)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (id, value);
            Err("shortcut_platform_unsupported".into())
        }
    }
    fn unregister(&self, id: u32) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            super::macos::unregister_hotkey(id)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = id;
            Ok(())
        }
    }
}
#[derive(Clone)]
struct Binding {
    intent: ShortcutIntent,
    accelerator: Option<Accelerator>,
    id: Option<u32>,
}
struct Pending {
    token: String,
    bindings: Vec<Binding>,
    added: Vec<u32>,
    scope: String,
}
struct Inner {
    generation: u64,
    next_id: u32,
    next_transaction: u64,
    pending: Option<Pending>,
    active: Vec<Binding>,
    desired: Vec<Binding>,
    scope: String,
    owned: BTreeSet<u32>,
    held: BTreeSet<u32>,
    suppressed: bool,
    state: ShortcutState,
    errors: BTreeMap<String, String>,
    initialization_error: Option<String>,
}
pub struct ShortcutCoordinator {
    operation: Mutex<()>,
    inner: Mutex<Inner>,
    driver: Arc<dyn Driver>,
    on_action: Arc<dyn Fn(ShortcutIntent) + Send + Sync>,
}
enum InputEvent {
    KeyEdge(u32, bool),
    ExplicitAction(ShortcutIntent),
}
static EDGES: OnceLock<mpsc::SyncSender<InputEvent>> = OnceLock::new();
static EDGE_OVERFLOW: AtomicBool = AtomicBool::new(false);
extern "C" fn native_edge(id: u32, pressed: i32) {
    if let Some(sender) = EDGES.get()
        && sender
            .try_send(InputEvent::KeyEdge(id, pressed != 0))
            .is_err()
    {
        EDGE_OVERFLOW.store(true, Ordering::Release);
    }
}
impl ShortcutCoordinator {
    pub fn new(on_action: Arc<dyn Fn(ShortcutIntent) + Send + Sync>) -> Arc<Self> {
        let this = Self::with_driver(Arc::new(NativeDriver), on_action);
        let (tx, rx) = mpsc::sync_channel(64);
        if EDGES.set(tx).is_err() {
            if let Ok(mut state) = this.inner.lock() {
                state.state = ShortcutState::Failed;
                state
                    .errors
                    .insert("all".into(), "shortcut_coordinator_exists".into());
                state.initialization_error = Some("shortcut_coordinator_exists".into());
            }
            return this;
        }
        #[cfg(target_os = "macos")]
        if let Err(code) = super::macos::initialize_hotkeys(native_edge)
            && let Ok(mut state) = this.inner.lock()
        {
            state.state = ShortcutState::Failed;
            state.initialization_error = Some(code.clone());
            state.errors.insert("all".into(), code);
        }
        let weak = Arc::downgrade(&this);
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let Some(coordinator) = weak.upgrade() else {
                    break;
                };
                if EDGE_OVERFLOW.swap(false, Ordering::AcqRel)
                    && let Ok(mut state) = coordinator.inner.lock()
                {
                    state.suppressed = true;
                    state.state = ShortcutState::Failed;
                    state.errors.insert(
                        "all".into(),
                        "快捷键事件队列繁忙，请重新应用快捷键恢复".into(),
                    );
                }
                if let Some(intent) = coordinator.resolve_event(event) {
                    (coordinator.on_action)(intent);
                }
            }
        });
        this
    }
    fn with_driver(
        driver: Arc<dyn Driver>,
        on_action: Arc<dyn Fn(ShortcutIntent) + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            driver,
            on_action,
            operation: Mutex::new(()),
            inner: Mutex::new(Inner {
                generation: 0,
                next_id: 1,
                next_transaction: 1,
                pending: None,
                active: vec![],
                desired: vec![],
                scope: "global".into(),
                owned: BTreeSet::new(),
                held: BTreeSet::new(),
                suppressed: false,
                state: ShortcutState::Draft,
                errors: BTreeMap::new(),
                initialization_error: None,
            }),
        })
    }
    pub fn prepare(
        &self,
        shortcuts: &BTreeMap<String, String>,
        scope: &str,
        expected_generation: Option<&str>,
    ) -> AppResult<String> {
        let _serial = self
            .operation
            .try_lock()
            .map_err(|_| error("shortcut_apply_in_progress", "另一项快捷键更改正在应用"))?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
        if let Some(code) = state.initialization_error.clone() {
            state.state = ShortcutState::Failed;
            return Err(error("shortcut_initialize", code));
        }
        if state.pending.is_some() {
            return Err(error(
                "shortcut_apply_in_progress",
                "另一项快捷键更改正在应用",
            ));
        }
        if expected_generation.is_some_and(|v| v != state.generation.to_string()) {
            return Err(error(
                "shortcut_generation_conflict",
                "快捷键有效版本已变化，请重新检查",
            ));
        }
        if state.generation == u64::MAX || state.next_transaction == u64::MAX {
            return Err(error(
                "shortcut_generation_exhausted",
                "请重启客户端恢复快捷键",
            ));
        }
        state.errors.clear();
        state.state = ShortcutState::Validating;
        let mut desired = Vec::new();
        let mut duplicates = BTreeSet::new();
        for intent in INTENTS {
            let default = match intent {
                ShortcutIntent::ToggleHud => "Alt+Super+H",
                ShortcutIntent::ToggleCapturePause => "Alt+Super+P",
                ShortcutIntent::CycleReadingDisplayMode => "",
            };
            let accelerator = match parse(
                shortcuts
                    .get(key(intent))
                    .map(String::as_str)
                    .unwrap_or(default),
            ) {
                Ok(value) => value,
                Err(message) => {
                    state.state = ShortcutState::Failed;
                    state.errors.insert(key(intent).into(), message.clone());
                    return Err(error("shortcut_invalid", message));
                }
            };
            if let Some(value) = &accelerator
                && !duplicates.insert(value.canonical.clone())
            {
                state.state = ShortcutState::Failed;
                state
                    .errors
                    .insert(key(intent).into(), "应用内快捷键重复".into());
                return Err(error("shortcut_duplicate", "多个动作不能使用相同快捷键"));
            }
            desired.push(Binding {
                intent,
                accelerator,
                id: None,
            });
        }
        let mut candidates = Vec::new();
        for binding in &mut desired {
            let Some(value) = &binding.accelerator else {
                continue;
            };
            if let Some(existing) = state
                .active
                .iter()
                .find(|b| b.accelerator.as_ref().map(|a| &a.canonical) == Some(&value.canonical))
            {
                binding.id = existing.id;
                continue;
            }
            let id = state.next_id;
            let Some(next) = id.checked_add(1) else {
                state.state = ShortcutState::Failed;
                return Err(error(
                    "shortcut_generation_exhausted",
                    "请重启客户端恢复快捷键",
                ));
            };
            state.next_id = next;
            binding.id = Some(id);
            candidates.push((id, binding.intent, value.clone()));
        }
        let token = format!("shortcuts-{}-{}", state.generation, state.next_transaction);
        state.next_transaction += 1;
        state.desired = desired.clone();
        state.state = ShortcutState::Registering;
        drop(state);
        // The operation guard serializes mutations, while the projection mutex
        // is never held across AppKit main-thread work. Main-thread close/menu
        // handlers can query/suppress dispatch without lock inversion.
        let mut registered = Vec::new();
        for (id, intent, value) in candidates {
            if let Err(message) = self.driver.register(id, &value) {
                {
                    let mut state = self
                        .inner
                        .lock()
                        .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
                    state.state = ShortcutState::Failed;
                    state.errors.insert(key(intent).into(), message.clone());
                }
                self.cleanup_ids(registered)?;
                return Err(error(
                    "shortcut_conflict",
                    format!("快捷键未应用，已保留原有效绑定：{message}"),
                ));
            }
            self.inner
                .lock()
                .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?
                .owned
                .insert(id);
            registered.push(id);
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
        state.pending = Some(Pending {
            token: token.clone(),
            bindings: desired,
            added: registered,
            scope: scope.into(),
        });
        Ok(token)
    }
    fn cleanup_ids(&self, ids: Vec<u32>) -> AppResult<()> {
        for id in ids {
            let result = self.driver.unregister(id);
            let mut state = self
                .inner
                .lock()
                .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
            if result.is_ok() {
                state.owned.remove(&id);
                state.held.remove(&id);
            } else {
                state.state = ShortcutState::CleanupPending;
                state
                    .errors
                    .insert("all".into(), "旧注册清理失败；无效动作不会派发".into());
            }
        }
        Ok(())
    }
    pub fn commit(&self, token: &str) -> AppResult<ShortcutProjection> {
        let _serial = self
            .operation
            .try_lock()
            .map_err(|_| error("shortcut_apply_in_progress", "另一项快捷键更改正在应用"))?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
        if state.pending.as_ref().map(|p| p.token.as_str()) != Some(token) {
            return Err(error("shortcut_transaction", "快捷键事务已失效"));
        }
        let generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| error("shortcut_generation_exhausted", "请重启客户端"))?;
        let pending = state.pending.take().unwrap();
        state.generation = generation;
        state.active = pending.bindings.clone();
        state.desired = pending.bindings;
        state.scope = pending.scope;
        state.state = ShortcutState::Active;
        let active: BTreeSet<_> = state.active.iter().filter_map(|b| b.id).collect();
        let obsolete = state.owned.difference(&active).copied().collect();
        drop(state);
        self.cleanup_ids(obsolete)?;
        self.projection()
    }
    pub fn rollback(&self, token: &str) -> AppResult<ShortcutProjection> {
        let _serial = self
            .operation
            .try_lock()
            .map_err(|_| error("shortcut_apply_in_progress", "另一项快捷键更改正在应用"))?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
        if state.pending.as_ref().map(|p| p.token.as_str()) != Some(token) {
            return Err(error("shortcut_transaction", "快捷键事务已失效"));
        }
        let pending = state.pending.take().unwrap();
        state.state = ShortcutState::Failed;
        state
            .errors
            .insert("all".into(), "偏好未提交，原有效快捷键已保留".into());
        drop(state);
        self.cleanup_ids(pending.added)?;
        self.projection()
    }
    pub fn projection(&self) -> AppResult<ShortcutProjection> {
        self.inner
            .lock()
            .map(|s| project(&s))
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))
    }
    /// Explicit menu clicks use the same ordered consumer as native hotkeys,
    /// including when a shortcut is unset or its registration has failed.
    pub fn trigger_action(&self, intent: ShortcutIntent) -> AppResult<()> {
        EDGES
            .get()
            .ok_or_else(|| error("shortcut_action_unavailable", "阅读操作暂不可用"))?
            .try_send(InputEvent::ExplicitAction(intent))
            .map_err(|_| error("shortcut_action_busy", "阅读操作正在处理，请稍后重试"))
    }
    fn resolve_event(&self, event: InputEvent) -> Option<ShortcutIntent> {
        match event {
            InputEvent::KeyEdge(id, pressed) => self.edge(id, pressed),
            InputEvent::ExplicitAction(intent) => Some(intent),
        }
    }
    pub fn suppress_dispatch(&self, suppressed: bool) -> AppResult<ShortcutProjection> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
        state.suppressed = suppressed;
        Ok(project(&state))
    }
    fn edge(&self, id: u32, pressed: bool) -> Option<ShortcutIntent> {
        let mut state = self.inner.lock().ok()?;
        if !pressed {
            state.held.remove(&id);
            return None;
        }
        if !state.held.insert(id) || state.suppressed {
            return None;
        }
        state
            .active
            .iter()
            .find(|b| b.id == Some(id))
            .map(|b| b.intent)
    }
    pub fn shutdown(&self) -> AppResult<ShortcutProjection> {
        let _serial = self
            .operation
            .try_lock()
            .map_err(|_| error("shortcut_apply_in_progress", "另一项快捷键更改正在应用"))?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| error("shortcut_state", "快捷键状态不可用"))?;
        state.active.clear();
        state.pending = None;
        state.state = ShortcutState::Draft;
        let ids = state.owned.iter().copied().collect();
        drop(state);
        self.cleanup_ids(ids)?;
        self.projection()
    }
}
fn project(state: &Inner) -> ShortcutProjection {
    ShortcutProjection {
        state: state.state,
        registration_generation: state.generation.to_string(),
        dispatch_suppressed: state.suppressed,
        bindings: INTENTS
            .into_iter()
            .map(|intent| ShortcutBindingProjection {
                intent,
                desired: state
                    .desired
                    .iter()
                    .find(|b| b.intent == intent)
                    .and_then(|b| b.accelerator.as_ref().map(|a| a.canonical.clone())),
                effective: state
                    .active
                    .iter()
                    .find(|b| b.intent == intent)
                    .and_then(|b| b.accelerator.as_ref().map(|a| a.canonical.clone())),
                scope_key: state.scope.clone(),
                error_code: state
                    .errors
                    .get(key(intent))
                    .or_else(|| state.errors.get("all"))
                    .map(|_| "shortcut_registration".into()),
                error_message: state
                    .errors
                    .get(key(intent))
                    .or_else(|| state.errors.get("all"))
                    .cloned(),
            })
            .collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Mock {
        registered: Mutex<BTreeMap<u32, String>>,
        fail_register: Mutex<bool>,
        fail_cleanup: Mutex<bool>,
        on_native_call: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    }
    #[test]
    fn explicit_menu_recovery_remains_available_after_registration_failure() {
        let driver = Arc::new(Mock::default());
        *driver.fail_register.lock().unwrap() = true;
        let coordinator = ShortcutCoordinator::with_driver(driver, Arc::new(|_| {}));
        assert!(
            coordinator
                .prepare(&BTreeMap::new(), "global", None)
                .is_err()
        );
        coordinator.suppress_dispatch(true).unwrap();
        assert_eq!(
            coordinator.resolve_event(InputEvent::KeyEdge(1, true)),
            None
        );
        assert_eq!(
            coordinator.resolve_event(InputEvent::ExplicitAction(ShortcutIntent::ToggleHud)),
            Some(ShortcutIntent::ToggleHud)
        );
        let after = coordinator.projection().unwrap();
        assert_eq!(after.state, ShortcutState::Failed);
        assert!(after.dispatch_suppressed);
        assert!(
            after
                .bindings
                .iter()
                .all(|binding| binding.effective.is_none())
        );
    }
    impl Driver for Mock {
        fn register(&self, id: u32, value: &Accelerator) -> Result<(), String> {
            if let Some(call) = self.on_native_call.lock().unwrap().as_ref() {
                call();
            }
            if *self.fail_register.lock().unwrap() {
                return Err("occupied".into());
            }
            self.registered
                .lock()
                .unwrap()
                .insert(id, value.canonical.clone());
            Ok(())
        }
        fn unregister(&self, id: u32) -> Result<(), String> {
            if let Some(call) = self.on_native_call.lock().unwrap().as_ref() {
                call();
            }
            if *self.fail_cleanup.lock().unwrap() {
                return Err("cleanup".into());
            }
            self.registered.lock().unwrap().remove(&id);
            Ok(())
        }
    }
    fn setup() -> (Arc<ShortcutCoordinator>, Arc<Mock>) {
        let driver = Arc::new(Mock::default());
        (
            ShortcutCoordinator::with_driver(driver.clone(), Arc::new(|_| {})),
            driver,
        )
    }
    #[test]
    fn native_main_thread_calls_do_not_hold_projection_mutex() {
        let (c, d) = setup();
        let weak = Arc::downgrade(&c);
        *d.on_native_call.lock().unwrap() = Some(Box::new(move || {
            let c = weak.upgrade().unwrap();
            assert!(
                c.inner.try_lock().is_ok(),
                "AppKit callback must be able to read native projection"
            );
            c.suppress_dispatch(false).unwrap();
        }));
        let p = c.prepare(&BTreeMap::new(), "global", None).unwrap();
        c.commit(&p).unwrap();
        let changed = BTreeMap::from([("toggleHud".into(), "Alt+Super+J".into())]);
        let p = c.prepare(&changed, "global", None).unwrap();
        c.commit(&p).unwrap();
        c.shutdown().unwrap();
    }
    #[test]
    fn stale_transaction_token_cannot_commit_a_new_prepare() {
        let (c, _) = setup();
        let p = c.prepare(&BTreeMap::new(), "global", None).unwrap();
        c.commit(&p).unwrap();
        let old = c.prepare(&BTreeMap::new(), "global", None).unwrap();
        c.rollback(&old).unwrap();
        let new = c.prepare(&BTreeMap::new(), "game:next", None).unwrap();
        assert_ne!(old, new);
        assert!(c.commit(&old).is_err());
        c.commit(&new).unwrap();
    }
    #[test]
    fn exhausted_identifiers_are_rejected_before_any_native_registration() {
        let (c, d) = setup();
        c.inner.lock().unwrap().next_id = u32::MAX - 1;
        assert!(c.prepare(&BTreeMap::new(), "global", None).is_err());
        assert!(d.registered.lock().unwrap().is_empty());
        assert!(c.inner.lock().unwrap().owned.is_empty());
        c.inner.lock().unwrap().generation = u64::MAX;
        assert!(c.prepare(&BTreeMap::new(), "global", None).is_err());
        assert!(d.registered.lock().unwrap().is_empty());
    }
    #[test]
    fn duplicates_and_invalid_combinations_rejected() {
        assert_eq!(
            parse("Cmd+Alt+H").unwrap(),
            parse("Option+Super+H").unwrap()
        );
        assert!(parse("Shift+H").is_err());
        let (c, _) = setup();
        let m = BTreeMap::from([("togglePause".into(), "Alt+Super+H".into())]);
        assert!(c.prepare(&m, "global", None).is_err());
    }
    #[test]
    fn prepared_events_do_not_dispatch_and_durable_rollback_keeps_old() {
        let (c, d) = setup();
        let p = c.prepare(&BTreeMap::new(), "global", None).unwrap();
        assert_eq!(c.edge(1, true), None);
        c.edge(1, false);
        c.commit(&p).unwrap();
        assert_eq!(c.edge(1, true), Some(ShortcutIntent::ToggleHud));
        c.edge(1, false);
        let m = BTreeMap::from([("toggleHud".into(), "Alt+Super+J".into())]);
        let p = c.prepare(&m, "game:g", Some("1")).unwrap();
        assert_eq!(c.edge(3, true), None);
        c.rollback(&p).unwrap();
        assert_eq!(d.registered.lock().unwrap().len(), 2);
        assert_eq!(c.edge(1, true), Some(ShortcutIntent::ToggleHud));
    }
    #[test]
    fn edges_repeat_release_suppression_and_generation_conflict() {
        let (c, _) = setup();
        let p = c.prepare(&BTreeMap::new(), "global", None).unwrap();
        assert!(c.prepare(&BTreeMap::new(), "global", None).is_err());
        c.commit(&p).unwrap();
        assert!(c.prepare(&BTreeMap::new(), "global", Some("0")).is_err());
        assert!(c.edge(1, true).is_some());
        assert_eq!(c.edge(1, true), None);
        assert_eq!(c.edge(1, false), None);
        c.suppress_dispatch(true).unwrap();
        assert_eq!(c.edge(1, true), None);
        c.suppress_dispatch(false).unwrap();
        assert_eq!(c.edge(1, true), None);
        c.edge(1, false);
        assert!(c.edge(1, true).is_some());
    }
    #[test]
    fn conflict_preserves_effective_and_cleanup_failure_never_dispatches_retired() {
        let (c, d) = setup();
        let p = c.prepare(&BTreeMap::new(), "global", None).unwrap();
        c.commit(&p).unwrap();
        *d.fail_register.lock().unwrap() = true;
        let m = BTreeMap::from([("toggleHud".into(), "Alt+Super+J".into())]);
        assert!(c.prepare(&m, "global", None).is_err());
        assert!(
            c.projection().unwrap().bindings[0]
                .effective
                .as_ref()
                .unwrap()
                .ends_with("H")
        );
        *d.fail_register.lock().unwrap() = false;
        let p = c.prepare(&m, "global", None).unwrap();
        *d.fail_cleanup.lock().unwrap() = true;
        let receipt = c.commit(&p).unwrap();
        assert_eq!(receipt.state, ShortcutState::CleanupPending);
        assert_eq!(c.edge(1, true), None);
        assert_eq!(c.edge(4, true), Some(ShortcutIntent::ToggleHud));
    }
}
