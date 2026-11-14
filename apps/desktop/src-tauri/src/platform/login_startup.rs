//! App-level login registration. No profile/SQLite boolean shadows OS authority.
use app_core::contracts::{AppError, AppResult, LoginStartupState, LoginStartupStatus};

pub const IPC_ROLES: &[&str] = &["settings"];

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "login_startup", message)
}

fn unsupported(detail: &str) -> LoginStartupState {
    LoginStartupState {
        status: LoginStartupStatus::Unsupported,
        can_change: false,
        error_code: None,
        detail: detail.into(),
    }
}

/// Small native seam: tests never invoke ServiceManagement or mutate login items.
trait Backend {
    fn query(&mut self) -> AppResult<LoginStartupState>;
    fn change(&mut self, enabled: bool) -> AppResult<()>;
}

fn change_and_read(backend: &mut impl Backend, enabled: bool) -> AppResult<LoginStartupState> {
    let before = backend.query()?;
    if !before.can_change {
        return Err(error("login_startup_unavailable", &before.detail));
    }
    // An idempotent click must not provoke AlreadyRegistered errors.
    if enabled
        && matches!(
            before.status,
            LoginStartupStatus::Enabled | LoginStartupStatus::RequiresApproval
        )
        || !enabled && before.status == LoginStartupStatus::NotRegistered
    {
        return backend.query();
    }
    let changed = backend.change(enabled);
    // Also re-read after a failed native write: approval/status may have changed.
    let mut current = backend.query()?;
    if let Err(cause) = changed {
        current.error_code = Some(cause.code);
        current.detail = cause.message;
    }
    Ok(current)
}

pub fn query(app: &tauri::AppHandle) -> AppResult<LoginStartupState> {
    #[cfg(target_os = "macos")]
    {
        on_main_thread(app, |native| native.query())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(unsupported("当前系统未提供登录后打开客户端的能力。"))
    }
}

pub fn set(app: &tauri::AppHandle, enabled: bool) -> AppResult<LoginStartupState> {
    #[cfg(target_os = "macos")]
    {
        on_main_thread(app, move |native| change_and_read(native, enabled))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, enabled);
        Err(error(
            "login_startup_unsupported",
            "当前系统不支持修改登录启动。",
        ))
    }
}

#[cfg(target_os = "macos")]
fn on_main_thread(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&mut native::Native) -> AppResult<LoginStartupState> + Send + 'static,
) -> AppResult<LoginStartupState> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = native::Native::with_pool(operation);
        let _ = sender.send(result);
    })
    .map_err(|_| error("login_startup_executor", "无法在主线程核对登录启动状态。"))?;
    // Called only by IPC's blocking executor. No timeout that could report a
    // rejected mutation while leaving a queued system write behind.
    receiver
        .recv()
        .map_err(|_| error("login_startup_executor", "登录启动状态执行器已停止。"))?
}

#[cfg(any(target_os = "macos", test))]
fn status(raw: isize) -> AppResult<LoginStartupStatus> {
    Ok(match raw {
        0 => LoginStartupStatus::NotRegistered,
        1 => LoginStartupStatus::Enabled,
        2 => LoginStartupStatus::RequiresApproval,
        3 => LoginStartupStatus::NotFound,
        _ => {
            return Err(error(
                "login_startup_unknown_status",
                "系统返回了无法识别的登录启动状态。",
            ));
        }
    })
}

#[cfg(any(target_os = "macos", test))]
fn mutation_block(acceptance: bool, bundled: bool, signed: bool) -> Option<&'static str> {
    if acceptance {
        Some("验收运行仅允许读取登录启动状态。")
    } else if !bundled {
        Some("请从已签名的应用包打开客户端后设置登录启动。")
    } else if !signed {
        Some("当前应用签名未通过核对，无法修改登录启动。")
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::ffi::{CStr, c_char, c_void};
    type Object = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Object;
        fn objc_msgSend();
    }
    // ServiceManagement is linked here, never in app-core or the webview.
    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecStaticCodeCreateWithPath(url: Object, flags: u32, code: *mut Object) -> i32;
        fn SecStaticCodeCheckValidity(code: Object, flags: u32, requirement: Object) -> i32;
        fn SecCodeCopySigningInformation(code: Object, flags: u32, info: *mut Object) -> i32;
        static kSecCodeInfoFlags: Object;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(object: Object);
        fn CFDictionaryGetValue(dictionary: Object, key: Object) -> Object;
        fn CFNumberGetValue(number: Object, number_type: isize, value: *mut i32) -> bool;
    }

    /// Owned +1 Cocoa/CF objects are released on their creation thread.
    struct Owned(Object);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: Owned is constructed only for non-null +1 CF objects.
                unsafe { CFRelease(self.0) };
            }
        }
    }
    unsafe fn class(name: &CStr) -> Object {
        // SAFETY: The static class name is NUL-terminated.
        unsafe { objc_getClass(name.as_ptr()) }
    }
    unsafe fn object(receiver: Object, selector: &CStr) -> Object {
        // SAFETY: All callers use documented zero-argument ObjC object methods.
        unsafe {
            let send: unsafe extern "C" fn(Object, Object) -> Object =
                std::mem::transmute(objc_msgSend as *const ());
            send(receiver, sel_registerName(selector.as_ptr()))
        }
    }
    unsafe fn integer(receiver: Object, selector: &CStr) -> isize {
        // SAFETY: Callers use NSInteger-returning status/code selectors.
        unsafe {
            let send: unsafe extern "C" fn(Object, Object) -> isize =
                std::mem::transmute(objc_msgSend as *const ());
            send(receiver, sel_registerName(selector.as_ptr()))
        }
    }
    unsafe fn boolean(receiver: Object, selector: &CStr) -> bool {
        // SAFETY: NSThread.isMainThread returns Objective-C BOOL on macOS.
        unsafe {
            let send: unsafe extern "C" fn(Object, Object) -> i8 =
                std::mem::transmute(objc_msgSend as *const ());
            send(receiver, sel_registerName(selector.as_ptr())) != 0
        }
    }
    unsafe fn drain(pool: Object) {
        // SAFETY: NSAutoreleasePool.drain takes no arguments and returns void.
        unsafe {
            let send: unsafe extern "C" fn(Object, Object) =
                std::mem::transmute(objc_msgSend as *const ());
            send(pool, sel_registerName(c"drain".as_ptr()));
        }
    }
    unsafe fn text(receiver: Object) -> Option<String> {
        if receiver.is_null() {
            return None;
        }
        // SAFETY: Receiver is an NSString. UTF8String remains alive in the pool;
        // it is copied before the pool is drained.
        unsafe {
            let pointer = object(receiver, c"UTF8String") as *const c_char;
            (!pointer.is_null()).then(|| CStr::from_ptr(pointer).to_string_lossy().into_owned())
        }
    }
    fn signed_bundle(url: Object) -> bool {
        let mut raw_code = std::ptr::null_mut();
        // SAFETY: URL is NSBundle's retained-by-pool NSURL, toll-free bridged to
        // CFURL. Both copy/create outputs are released through Owned.
        unsafe {
            if url.is_null()
                || SecStaticCodeCreateWithPath(url, 0, &mut raw_code) != 0
                || raw_code.is_null()
            {
                return false;
            }
            let code = Owned(raw_code);
            if SecStaticCodeCheckValidity(code.0, 0, std::ptr::null_mut()) != 0 {
                return false;
            }
            let mut raw_info = std::ptr::null_mut();
            if SecCodeCopySigningInformation(code.0, 1 << 1, &mut raw_info) != 0
                || raw_info.is_null()
            {
                return false;
            }
            let info = Owned(raw_info);
            let flags = CFDictionaryGetValue(info.0, kSecCodeInfoFlags);
            let mut value = 0i32;
            // kCFNumberSInt32Type=3, kSecCodeSignatureAdhoc=0x0002.
            !flags.is_null() && CFNumberGetValue(flags, 3, &mut value) && value & 2 == 0
        }
    }
    pub(super) struct Native;
    impl Native {
        pub(super) fn with_pool(
            operation: impl FnOnce(&mut Self) -> AppResult<LoginStartupState>,
        ) -> AppResult<LoginStartupState> {
            // SAFETY: NSThread and NSAutoreleasePool are Foundation classes;
            // no Cocoa/ServiceManagement work is allowed off the main thread.
            unsafe {
                if !boolean(class(c"NSThread"), c"isMainThread") {
                    return Err(error(
                        "login_startup_thread",
                        "登录启动只能在应用主线程核对。",
                    ));
                }
                let pool = object(object(class(c"NSAutoreleasePool"), c"alloc"), c"init");
                if pool.is_null() {
                    return Err(error("login_startup_pool", "无法准备系统状态读取。"));
                }
                let result = operation(&mut Self);
                drain(pool);
                result
            }
        }
        fn service(&self) -> AppResult<Option<Object>> {
            // SAFETY: SMAppService exists starting macOS 13. Lookup is dynamic,
            // so older systems never send an unavailable selector.
            unsafe {
                let class = class(c"SMAppService");
                if class.is_null() {
                    return Ok(None);
                }
                let service = object(class, c"mainAppService");
                if service.is_null() {
                    return Err(error("login_startup_service", "无法读取系统登录启动服务。"));
                }
                Ok(Some(service))
            }
        }
    }
    impl Backend for Native {
        fn query(&mut self) -> AppResult<LoginStartupState> {
            let Some(service) = self.service()? else {
                return Ok(unsupported("登录启动需要 macOS 13 或更高版本。"));
            };
            // SAFETY: All referenced objects live in the current main-thread
            // autorelease pool; selectors are their documented signatures.
            unsafe {
                let bundle = object(class(c"NSBundle"), c"mainBundle");
                let path = text(object(bundle, c"bundlePath"));
                let bundled = path.as_deref().is_some_and(|path| path.ends_with(".app"));
                if !bundled {
                    return Ok(unsupported("请从已签名的应用包打开客户端后设置登录启动。"));
                }
                let signed = signed_bundle(object(bundle, c"bundleURL"));
                let block = mutation_block(
                    std::env::var_os("LUNA_ACCEPTANCE_PROFILE").is_some(),
                    bundled,
                    signed,
                );
                let state = status(integer(service, c"status"))?;
                let detail = match state {
                    LoginStartupStatus::NotRegistered => "登录后不会自动打开客户端。",
                    LoginStartupStatus::Enabled => "系统已允许登录后打开客户端。",
                    LoginStartupStatus::RequiresApproval => {
                        "已登记，尚未生效，等待系统批准；请前往系统设置 → 通用 → 登录项，允许 Luna 在登录后打开。"
                    }
                    LoginStartupStatus::NotFound => {
                        "系统找不到此应用服务，请重新安装签名应用包后重试。"
                    }
                    LoginStartupStatus::Unsupported => unreachable!(),
                };
                Ok(LoginStartupState {
                    can_change: block.is_none() && state != LoginStartupStatus::NotFound,
                    status: state,
                    error_code: None,
                    detail: block
                        .map_or_else(|| detail.into(), |reason| format!("{detail} {reason}")),
                })
            }
        }
        fn change(&mut self, enabled: bool) -> AppResult<()> {
            // Defense in depth: Native mutations independently re-check safety.
            let current = self.query()?;
            if !current.can_change {
                return Err(error("login_startup_unavailable", &current.detail));
            }
            let service = self
                .service()?
                .ok_or_else(|| error("login_startup_unsupported", "当前系统不支持登录启动。"))?;
            let selector = if enabled {
                c"registerAndReturnError:"
            } else {
                c"unregisterAndReturnError:"
            };
            let mut native_error = std::ptr::null_mut();
            // SAFETY: Exact SMAppService BOOL (NSError **) ABI from the SDK.
            // NSError is autoreleased and only its numeric code is copied.
            unsafe {
                let send: unsafe extern "C" fn(Object, Object, *mut Object) -> i8 =
                    std::mem::transmute(objc_msgSend as *const ());
                if send(
                    service,
                    sel_registerName(selector.as_ptr()),
                    &mut native_error,
                ) == 0
                {
                    let code = if native_error.is_null() {
                        0
                    } else {
                        integer(native_error, c"code")
                    };
                    return Err(error(
                        if enabled {
                            "login_startup_register_failed"
                        } else {
                            "login_startup_unregister_failed"
                        },
                        &format!("系统未完成登录启动修改（错误 {code}），请核对系统登录项后重试。"),
                    ));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        states: std::collections::VecDeque<AppResult<LoginStartupState>>,
        queries: usize,
        writes: Vec<bool>,
        failure: bool,
    }
    impl Backend for Fake {
        fn query(&mut self) -> AppResult<LoginStartupState> {
            self.queries += 1;
            self.states.pop_front().expect("prepared read")
        }
        fn change(&mut self, enabled: bool) -> AppResult<()> {
            self.writes.push(enabled);
            if self.failure {
                Err(error("controlled_write_error", "Controlled failure"))
            } else {
                Ok(())
            }
        }
    }
    fn state(status: LoginStartupStatus) -> AppResult<LoginStartupState> {
        Ok(LoginStartupState {
            status,
            can_change: true,
            error_code: None,
            detail: "controlled native seam; no OS call".into(),
        })
    }
    #[test]
    fn login_startup_status_maps_all_sdk_values_and_rejects_unknown() {
        for (raw, expected) in [
            (0, LoginStartupStatus::NotRegistered),
            (1, LoginStartupStatus::Enabled),
            (2, LoginStartupStatus::RequiresApproval),
            (3, LoginStartupStatus::NotFound),
        ] {
            assert_eq!(status(raw).unwrap(), expected);
        }
        assert!(status(4).is_err());
        assert!(status(-1).is_err());
    }
    #[test]
    fn login_startup_mutation_requires_signed_bundle_and_excludes_acceptance() {
        assert!(mutation_block(false, true, true).is_none());
        for (acceptance, bundled, signed) in [
            (true, true, true),
            (false, false, true),
            (false, true, false),
            (true, false, false),
        ] {
            assert!(mutation_block(acceptance, bundled, signed).is_some());
        }
        assert_eq!(IPC_ROLES, ["settings"]);
    }
    #[test]
    fn login_startup_writes_read_back_authority_and_preserve_approval() {
        let mut fake = Fake {
            states: [
                state(LoginStartupStatus::NotRegistered),
                state(LoginStartupStatus::RequiresApproval),
            ]
            .into(),
            writes: vec![],
            queries: 0,
            failure: false,
        };
        let result = change_and_read(&mut fake, true).unwrap();
        assert_eq!(result.status, LoginStartupStatus::RequiresApproval);
        assert_eq!(fake.writes, [true]);
        assert_eq!(fake.queries, 2);
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "requires_approval");
        assert_eq!(json["canChange"], true);
        assert!(json["errorCode"].is_null());
    }
    #[test]
    fn login_startup_failed_write_still_reads_actual_state() {
        let mut fake = Fake {
            states: [
                state(LoginStartupStatus::Enabled),
                state(LoginStartupStatus::Enabled),
            ]
            .into(),
            writes: vec![],
            queries: 0,
            failure: true,
        };
        let result = change_and_read(&mut fake, false).unwrap();
        assert_eq!(result.status, LoginStartupStatus::Enabled);
        assert_eq!(result.error_code.as_deref(), Some("controlled_write_error"));
        assert_eq!(fake.writes, [false]);
    }
    #[test]
    fn login_startup_read_failures_and_unavailable_states_never_write() {
        for initial in [
            Err(error("controlled_read_failure", "Controlled failure")),
            Ok(unsupported("unsupported controlled seam")),
        ] {
            let mut fake = Fake {
                states: [initial].into(),
                writes: vec![],
                queries: 0,
                failure: false,
            };
            assert!(change_and_read(&mut fake, true).is_err());
            assert!(fake.writes.is_empty());
        }
    }
    #[test]
    fn login_startup_does_not_register_again_when_already_enabled() {
        let mut fake = Fake {
            states: [
                state(LoginStartupStatus::Enabled),
                state(LoginStartupStatus::Enabled),
            ]
            .into(),
            writes: vec![],
            queries: 0,
            failure: false,
        };
        assert_eq!(
            change_and_read(&mut fake, true).unwrap().status,
            LoginStartupStatus::Enabled
        );
        assert!(fake.writes.is_empty());
        assert_eq!(fake.queries, 2);
    }
    #[test]
    fn login_startup_approval_is_registered_without_reregistering_and_can_be_cancelled() {
        let mut fake = Fake {
            states: [
                state(LoginStartupStatus::RequiresApproval),
                state(LoginStartupStatus::RequiresApproval),
                state(LoginStartupStatus::RequiresApproval),
                state(LoginStartupStatus::NotRegistered),
            ]
            .into(),
            queries: 0,
            writes: vec![],
            failure: false,
        };
        assert_eq!(
            change_and_read(&mut fake, true).unwrap().status,
            LoginStartupStatus::RequiresApproval
        );
        assert_eq!(fake.queries, 2);
        assert!(fake.writes.is_empty());
        assert_eq!(
            change_and_read(&mut fake, false).unwrap().status,
            LoginStartupStatus::NotRegistered
        );
        assert_eq!(fake.queries, 4);
        assert_eq!(fake.writes, [false]);
    }
}
