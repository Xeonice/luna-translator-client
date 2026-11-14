//! Windows target identity probe; discovery modes are read-only.
//! The explicit bounded window-title challenge temporarily changes one verified
//! HWND title and restores it before acknowledging successful attestation.
//!
//! Usage: `target_probe.exe [GameN7_Steam.exe SR.exe ...]` or `target_probe.exe --all`.
//! The all-process discovery mode queries identity only, without module snapshots.

#[cfg(any(windows, test))]
mod window_query {
    use std::{fmt, marker::PhantomData, rc::Rc};

    // Win32 DPI_AWARENESS_CONTEXT values are opaque, pointer-sized handles. -4
    // is the documented PER_MONITOR_AWARE_V2 pseudo-handle, not a game setting.
    pub(super) type DpiContext = isize;
    const PER_MONITOR_AWARE_V2: DpiContext = -4;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum DpiApiFailure {
        ApiUnavailable,
        ChangeRejected(u32),
    }

    impl fmt::Display for DpiApiFailure {
        fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::ApiUnavailable => output.write_str("api_unavailable"),
                Self::ChangeRejected(code) => write!(output, "change_rejected: win32={code}"),
            }
        }
    }

    pub(super) trait ThreadDpiApi {
        fn replace_context(&self, context: DpiContext) -> Result<DpiContext, DpiApiFailure>;
    }

    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum WindowQueryFailure<E> {
        Enter(DpiApiFailure),
        Operation(E),
        Restore(DpiApiFailure),
        OperationAndRestore {
            operation: E,
            restore: DpiApiFailure,
        },
    }

    impl<E: fmt::Display> fmt::Display for WindowQueryFailure<E> {
        fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Enter(error) => write!(output, "window_query_dpi_enter_failed: {error}"),
                Self::Operation(error) => write!(output, "{error}"),
                Self::Restore(error) => write!(output, "window_query_dpi_restore_failed: {error}"),
                Self::OperationAndRestore { operation, restore } => write!(
                    output,
                    "window_query_dpi_restore_failed: {restore}; operation_failed: {operation}"
                ),
            }
        }
    }

    struct ThreadDpiScope<'api, A: ThreadDpiApi> {
        api: &'api A,
        previous: Option<DpiContext>,
        // A context must be restored on the thread that changed it. This
        // marker makes the guard !Send and !Sync even if its API is Send/Sync.
        _thread_bound: PhantomData<Rc<()>>,
    }

    impl<'api, A: ThreadDpiApi> ThreadDpiScope<'api, A> {
        fn enter(api: &'api A) -> Result<Self, DpiApiFailure> {
            let previous = api.replace_context(PER_MONITOR_AWARE_V2)?;
            Ok(Self {
                api,
                previous: Some(previous),
                _thread_bound: PhantomData,
            })
        }

        fn restore(&mut self) -> Result<(), DpiApiFailure> {
            if let Some(previous) = self.previous {
                self.api.replace_context(previous)?;
                self.previous = None;
            }
            Ok(())
        }
    }

    impl<A: ThreadDpiApi> Drop for ThreadDpiScope<'_, A> {
        fn drop(&mut self) {
            // Also restore during unwinding, or retry a rejected explicit
            // restoration. A retry never turns the explicit failure into Ok.
            let _ = self.restore();
        }
    }

    pub(super) fn with_physical_pixels<A: ThreadDpiApi, T, E>(
        api: &A,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, WindowQueryFailure<E>> {
        let mut scope = ThreadDpiScope::enter(api).map_err(WindowQueryFailure::Enter)?;
        let result = operation();
        match (result, scope.restore()) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(WindowQueryFailure::Operation(error)),
            (Ok(_), Err(error)) => Err(WindowQueryFailure::Restore(error)),
            (Err(operation), Err(restore)) => {
                Err(WindowQueryFailure::OperationAndRestore { operation, restore })
            }
        }
    }

    pub(super) fn valid_window_rect(left: i32, top: i32, right: i32, bottom: i32) -> bool {
        right > left && bottom > top
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{cell::Cell, cell::RefCell, panic::AssertUnwindSafe};

        struct FakeApi {
            current: Cell<DpiContext>,
            calls: RefCell<Vec<DpiContext>>,
            failures: Vec<(usize, DpiApiFailure)>,
        }

        impl FakeApi {
            fn new(current: DpiContext, failures: Vec<(usize, DpiApiFailure)>) -> Self {
                Self {
                    current: Cell::new(current),
                    calls: RefCell::new(Vec::new()),
                    failures,
                }
            }
        }

        impl ThreadDpiApi for FakeApi {
            fn replace_context(&self, context: DpiContext) -> Result<DpiContext, DpiApiFailure> {
                let mut calls = self.calls.borrow_mut();
                calls.push(context);
                if let Some((_, error)) = self
                    .failures
                    .iter()
                    .find(|(number, _)| *number == calls.len())
                {
                    return Err(*error);
                }
                Ok(self.current.replace(context))
            }
        }

        #[test]
        fn successful_query_uses_physical_pixels_and_restores_exact_context() {
            let original = 73;
            let api = FakeApi::new(original, vec![]);
            let result = with_physical_pixels(&api, || {
                assert_eq!(api.current.get(), PER_MONITOR_AWARE_V2);
                Ok::<_, &str>("query result")
            });
            assert_eq!(result, Ok("query result"));
            assert_eq!(api.current.get(), original);
            assert_eq!(*api.calls.borrow(), [PER_MONITOR_AWARE_V2, original]);
        }

        #[test]
        fn operation_failure_is_preserved_after_restoring_context() {
            let original = 91;
            let api = FakeApi::new(original, vec![]);
            let result = with_physical_pixels(&api, || Err::<(), _>("EnumWindows failed"));
            assert_eq!(
                result,
                Err(WindowQueryFailure::Operation("EnumWindows failed"))
            );
            assert_eq!(api.current.get(), original);
            assert_eq!(*api.calls.borrow(), [PER_MONITOR_AWARE_V2, original]);
        }

        #[test]
        fn absent_api_or_rejected_enter_never_runs_query() {
            for failure in [
                DpiApiFailure::ApiUnavailable,
                DpiApiFailure::ChangeRejected(5),
            ] {
                let original = 113;
                let api = FakeApi::new(original, vec![(1, failure)]);
                let queried = Cell::new(false);
                let result = with_physical_pixels(&api, || {
                    queried.set(true);
                    Ok::<_, &str>(())
                });
                assert_eq!(result, Err(WindowQueryFailure::Enter(failure)));
                assert!(!queried.get());
                assert_eq!(api.current.get(), original);
                assert_eq!(*api.calls.borrow(), [PER_MONITOR_AWARE_V2]);
            }
        }

        #[test]
        fn rejected_restore_fails_query_even_when_drop_retry_restores_context() {
            let original = 127;
            let failure = DpiApiFailure::ChangeRejected(87);
            let api = FakeApi::new(original, vec![(2, failure)]);
            let result = with_physical_pixels(&api, || Ok::<_, &str>("not acknowledged"));
            assert_eq!(result, Err(WindowQueryFailure::Restore(failure)));
            assert_eq!(api.current.get(), original);
            assert_eq!(
                *api.calls.borrow(),
                [PER_MONITOR_AWARE_V2, original, original]
            );
        }

        #[test]
        fn operation_and_restore_failures_are_both_preserved() {
            let original = 149;
            let failure = DpiApiFailure::ChangeRejected(5);
            let api = FakeApi::new(original, vec![(2, failure), (3, failure)]);
            let result = with_physical_pixels(&api, || Err::<(), _>("cursor query failed"));
            assert_eq!(
                result,
                Err(WindowQueryFailure::OperationAndRestore {
                    operation: "cursor query failed",
                    restore: failure,
                })
            );
            assert_eq!(api.current.get(), PER_MONITOR_AWARE_V2);
            assert_eq!(
                *api.calls.borrow(),
                [PER_MONITOR_AWARE_V2, original, original]
            );
            let message = result.unwrap_err().to_string();
            assert!(message.contains("window_query_dpi_restore_failed"));
            assert!(message.contains("cursor query failed"));
        }

        #[test]
        fn nested_queries_restore_outer_then_original_context() {
            let original = 163;
            let api = FakeApi::new(original, vec![]);
            let result = with_physical_pixels(&api, || {
                let inner = with_physical_pixels(&api, || {
                    assert_eq!(api.current.get(), PER_MONITOR_AWARE_V2);
                    Ok::<_, &str>(())
                });
                assert_eq!(inner, Ok(()));
                assert_eq!(api.current.get(), PER_MONITOR_AWARE_V2);
                Ok::<_, &str>(())
            });
            assert_eq!(result, Ok(()));
            assert_eq!(api.current.get(), original);
            assert_eq!(
                *api.calls.borrow(),
                [
                    PER_MONITOR_AWARE_V2,
                    PER_MONITOR_AWARE_V2,
                    PER_MONITOR_AWARE_V2,
                    original
                ]
            );
        }

        #[test]
        fn panic_unwinding_restores_original_context() {
            let original = 181;
            let api = FakeApi::new(original, vec![]);
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                with_physical_pixels(&api, || -> Result<(), &str> {
                    assert_eq!(api.current.get(), PER_MONITOR_AWARE_V2);
                    panic!("owned test operation panicked");
                })
            }));
            assert!(result.is_err());
            assert_eq!(api.current.get(), original);
            assert_eq!(*api.calls.borrow(), [PER_MONITOR_AWARE_V2, original]);
        }

        #[test]
        fn geometry_guard_accepts_negative_coordinates_but_not_empty_or_inverted_rects() {
            assert!(valid_window_rect(-1200, -900, -1, -1));
            assert!(valid_window_rect(-20, 30, 500, 800));
            assert!(valid_window_rect(i32::MIN, i32::MIN, i32::MAX, i32::MAX));
            for rect in [
                (0, 0, 0, 100),
                (10, 0, -10, 100),
                (0, 0, 100, 0),
                (0, 10, 100, -10),
            ] {
                assert!(!valid_window_rect(rect.0, rect.1, rect.2, rect.3));
            }
        }
    }
}

// Keep the exact reversible caption separate from discovery's lossy display text.
// Discovery is allowed to show unusual text; a mutating proof is not allowed to
// back up a truncated or invalid UTF-16 caption and later write that back.
#[cfg(any(windows, test))]
fn proof_caption(units: &[u16], truncated: bool) -> Result<String, String> {
    if truncated {
        return Err("window_title_proof_caption_truncated".into());
    }
    String::from_utf16(units).map_err(|_| "window_title_proof_caption_invalid_utf16".into())
}

#[cfg(test)]
mod caption_tests {
    #[test]
    fn preserves_exact_unicode_including_surrogate_pairs() {
        let caption = "メモリーズオフ 🌸 e\u{301}";
        assert_eq!(
            super::proof_caption(&caption.encode_utf16().collect::<Vec<_>>(), false).unwrap(),
            caption
        );
    }

    #[test]
    fn rejects_invalid_or_truncated_backup_instead_of_lossy_restore() {
        assert!(super::proof_caption(&[0xd800], false).is_err());
        assert!(super::proof_caption(&[0xdc00], false).is_err());
        assert!(super::proof_caption(&[0x65e5], true).is_err());
        assert_eq!(super::proof_caption(&[], false).unwrap(), "");
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        r#"{{"error":"unsupported_platform","message":"target_probe requires Windows or a Windows process in the target Wine bottle"}}"#
    );
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("{}", serde_json::json!({ "error": error }));
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod windows {
    use super::window_query::{self, DpiApiFailure, DpiContext, ThreadDpiApi};
    use serde::Serialize;
    use std::{
        io,
        mem::size_of,
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{
            BOOL, CloseHandle, FILETIME, GetLastError, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM,
            POINT, RECT, SetLastError,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW,
                PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPMODULE,
                TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
            },
            LibraryLoader::{GetModuleHandleW, GetProcAddress},
            SystemInformation::{GetNativeSystemInfo, PROCESSOR_ARCHITECTURE_AMD64, SYSTEM_INFO},
            Threading::{
                GetCurrentProcessId, GetProcessTimes, IsWow64Process, OpenProcess,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON},
            WindowsAndMessaging::{
                EnumWindows, GA_ROOT, GetAncestor, GetClassNameW, GetCursorPos,
                GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
                IsIconic, IsWindow, IsWindowUnicode, IsWindowVisible, SMTO_ABORTIFHUNG, SW_RESTORE,
                SW_SHOW, SendMessageTimeoutW, SetForegroundWindow, ShowWindow, WM_SETTEXT,
                WindowFromPoint,
            },
        },
    };

    type SetThreadDpiAwarenessContextFn = unsafe extern "system" fn(HANDLE) -> HANDLE;

    struct WindowsThreadDpiApi {
        set_context: Option<SetThreadDpiAwarenessContextFn>,
    }

    impl WindowsThreadDpiApi {
        fn load() -> Self {
            let module_name: Vec<u16> = "user32.dll".encode_utf16().chain(Some(0)).collect();
            // SAFETY: The name is NUL-terminated. user32 is already imported
            // by this executable; this query does not change module refcounts.
            let module = unsafe { GetModuleHandleW(module_name.as_ptr()) };
            let function = if module.is_null() {
                None
            } else {
                // SAFETY: The loaded user32 module stays live and the symbol
                // name is NUL-terminated. Missing exports are handled below.
                unsafe { GetProcAddress(module, c"SetThreadDpiAwarenessContext".as_ptr().cast()) }
            };
            Self {
                set_context: function.map(|function| {
                    // SAFETY: The documented user32 symbol has exactly this
                    // Win32 ABI and opaque pointer-sized context signature.
                    unsafe { std::mem::transmute(function) }
                }),
            }
        }
    }

    impl ThreadDpiApi for WindowsThreadDpiApi {
        fn replace_context(&self, context: DpiContext) -> Result<DpiContext, DpiApiFailure> {
            let function = self.set_context.ok_or(DpiApiFailure::ApiUnavailable)?;
            // SAFETY: The function pointer was resolved with its Win32 ABI.
            // Only this helper's current thread context changes; the return
            // value is the opaque context required to restore that same thread.
            unsafe { SetLastError(0) };
            let previous = unsafe { function(context as HANDLE) };
            if previous.is_null() {
                // SAFETY: Reads the error of the failed thread-local call.
                Err(DpiApiFailure::ChangeRejected(unsafe { GetLastError() }))
            } else {
                Ok(previous as DpiContext)
            }
        }
    }

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: This guard exclusively owns a non-null, non-invalid Win32 handle.
            unsafe { CloseHandle(self.0) };
        }
    }

    #[derive(Serialize)]
    struct Process {
        pid: u32,
        exe: String,
        full_image_path: Option<String>,
        creation_filetime: Option<String>,
        architecture: Option<&'static str>,
        modules: Vec<String>,
        errors: Vec<String>,
    }

    #[derive(Serialize)]
    struct Report {
        processes: Vec<Process>,
    }

    #[derive(Serialize)]
    struct WindowRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[derive(Serialize)]
    struct WindowTarget {
        hwnd: String,
        title: String,
        class_name: String,
        rect: WindowRect,
        target: Process,
    }

    pub fn run() -> Result<(), String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args
            .first()
            .is_some_and(|a| a == "--verify-bound-window" || a == "--activate-window")
        {
            return bound_window_action(&args);
        }
        if args
            .first()
            .is_some_and(|a| a == "--window-title-challenge")
        {
            return window_title_challenge(&args);
        }
        if args == ["--windows"] {
            return write_json(&serde_json::json!({"windows": enumerate_windows()?}));
        }
        if args.first().is_some_and(|a| a == "--pick-window") {
            if args.len() > 2 {
                return Err("usage: --pick-window [timeout-seconds]".into());
            }
            let seconds = args
                .get(1)
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|_| "invalid picker timeout")?
                .unwrap_or(15);
            if !(1..=30).contains(&seconds) {
                return Err("picker timeout must be 1..30 seconds".into());
            }
            return pick_window(Duration::from_secs(seconds));
        }
        let mut names: Vec<String> = std::env::args().skip(1).collect();
        let all = names.iter().any(|name| name == "--all");
        if all && names.len() != 1 {
            return Err("--all cannot be combined with process names".into());
        }
        if names
            .iter()
            .any(|name| name.starts_with('-') && name != "--all")
        {
            return Err("usage: target_probe.exe [process.exe ...] | --all | --windows | --pick-window [timeout-seconds]".into());
        }
        if !all && names.is_empty() {
            names = [
                "GameN7_Steam.exe",
                "SR.exe",
                "LunaTranslator.exe",
                "lunahook-host.exe",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
        }
        // SAFETY: A process snapshot is read-only and takes no borrowed pointers.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(last_error("CreateToolhelp32Snapshot(processes)"));
        }
        let snapshot = OwnedHandle(raw);
        // SAFETY: PROCESSENTRY32W is a plain Win32 output record, sized below.
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut processes = Vec::new();
        // SAFETY: The snapshot is valid and the output record has the required size.
        if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
            if io::Error::last_os_error().raw_os_error() != Some(18) {
                return Err(last_error("Process32FirstW"));
            }
        } else {
            loop {
                let exe = wide(&entry.szExeFile);
                if all || names.iter().any(|name| name.eq_ignore_ascii_case(&exe)) {
                    processes.push(inspect(entry.th32ProcessID, exe, !all));
                }
                // SAFETY: Snapshot and correctly sized output record remain alive.
                if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() == Some(18) {
                        break;
                    }
                    return Err(format!("Process32NextW: {error}"));
                }
            }
        }
        serde_json::to_writer(io::stdout().lock(), &Report { processes })
            .map_err(|error| format!("write report: {error}"))
    }

    fn write_json(value: &impl Serialize) -> Result<(), String> {
        serde_json::to_writer(io::stdout().lock(), value).map_err(|e| format!("write report: {e}"))
    }

    fn proof_line(value: &impl Serialize) -> Result<(), String> {
        use std::io::Write;
        let mut output = io::stdout().lock();
        serde_json::to_writer(&mut output, value).map_err(|_| "proof output failed")?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|_| "proof output failed".into())
    }
    fn normalized(path: &str) -> String {
        path.trim_start_matches(r"\\?\")
            .replace('/', "\\")
            .to_lowercase()
    }
    // Exact identity checks are independent of visible-window discovery. Hidden
    // windows must be verifiable before the explicit user restore action.
    fn bound_identity(
        hwnd: HWND,
        pid: u32,
        path: &str,
        creation: &str,
        arch: &str,
        class: &str,
    ) -> Result<String, &'static str> {
        // SAFETY: Win32 validates expired HWND values; output pointers are live.
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            return Err("target_closed");
        }
        if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
            return Err("window_identity_changed");
        }
        let mut actual_pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut actual_pid) };
        if actual_pid != pid {
            return Err("window_identity_changed");
        }
        let actual = inspect(pid, String::new(), false);
        if actual.full_image_path.is_none()
            || actual.creation_filetime.is_none()
            || actual.architecture.is_none()
        {
            return Err("window_identity_unavailable");
        }
        if actual.full_image_path.as_deref().map(normalized) != Some(normalized(path))
            || actual.creation_filetime.as_deref() != Some(creation)
            || actual.architecture != Some(arch)
        {
            return Err("window_identity_changed");
        }
        let mut current_class = [0u16; 256];
        let class_len =
            unsafe { GetClassNameW(hwnd, current_class.as_mut_ptr(), current_class.len() as i32) };
        if class_len <= 0 {
            return Err("window_identity_unavailable");
        }
        if String::from_utf16(&current_class[..class_len as usize])
            .map_err(|_| "window_identity_unavailable")?
            != class
        {
            return Err("window_identity_changed");
        }
        let mut title = [0u16; 2048];
        let title_len = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) };
        Ok(String::from_utf16_lossy(
            &title[..title_len.max(0) as usize],
        ))
    }
    fn bound_window_action(args: &[String]) -> Result<(), String> {
        if args.len() != 7 {
            return Err(
                "usage: --verify-bound-window|--activate-window HWND PID IMAGE CREATION ARCH CLASS"
                    .into(),
            );
        }
        let handle = args[1].parse::<usize>().map_err(|_| "invalid HWND")?;
        let pid = args[2].parse::<u32>().map_err(|_| "invalid PID")?;
        let creation = args[4]
            .parse::<u64>()
            .map_err(|_| "invalid creation identity")?;
        if handle == 0
            || pid == 0
            || handle.to_string() != args[1]
            || pid.to_string() != args[2]
            || creation.to_string() != args[4]
            || !(args[3].starts_with(r"\\") || args[3].as_bytes().get(1) == Some(&b':'))
            || !matches!(args[5].as_str(), "x86" | "x64")
            || args[6].is_empty()
        {
            return Err("invalid bound window identity".into());
        }
        let hwnd = handle as HWND;
        let verify = || bound_identity(hwnd, pid, &args[3], &args[4], &args[5], &args[6]);
        let selected = verify();
        if args[0] == "--verify-bound-window" {
            return match selected {
                Ok(title) => write_json(
                    &serde_json::json!({"status":"verified","windowTitle":title,"errorCode":null}),
                ),
                Err(code) => write_json(
                    &serde_json::json!({"status":if code=="window_identity_unavailable"{"unavailable"}else{"stale"},"windowTitle":null,"errorCode":code}),
                ),
            };
        }
        if let Err(code) = selected {
            return write_json(
                &serde_json::json!({"status":"failed","wasMinimized":false,"showWasVisible":false,"setForegroundAccepted":false,"visibleAfter":false,"foregroundAfter":false,"errorCode":code}),
            );
        }
        // SAFETY: Exact HWND/process/class identity was checked immediately
        // above; no synthetic input or foreground-policy bypass is used.
        let minimized = unsafe { IsIconic(hwnd) } != 0;
        let was_visible =
            unsafe { ShowWindow(hwnd, if minimized { SW_RESTORE } else { SW_SHOW }) } != 0;
        let accepted = unsafe { SetForegroundWindow(hwnd) } != 0;
        let deadline = Instant::now() + Duration::from_millis(400);
        while unsafe { GetForegroundWindow() } != hwnd && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let identity_after = verify();
        let visible = unsafe { IsWindowVisible(hwnd) } != 0;
        let foreground = unsafe { GetForegroundWindow() } == hwnd;
        let code = identity_after.err().or(if !visible {
            Some("window_not_visible")
        } else {
            None
        });
        write_json(
            &serde_json::json!({"status":if code.is_some(){"failed"}else if foreground{"activated"}else{"visible_not_foreground"},"wasMinimized":minimized,"showWasVisible":was_visible,"setForegroundAccepted":accepted,"visibleAfter":visible,"foregroundAfter":foreground,"errorCode":code}),
        )
    }
    fn bounded_title(hwnd: HWND, title: &str) -> Result<(), String> {
        let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let mut result = 0;
        // SAFETY: A validated HWND and live UTF-16 buffer are passed to WM_SETTEXT.
        // SMTO_ABORTIFHUNG bounds the synchronous target call to 400ms.
        unsafe { SetLastError(0) };
        if unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_SETTEXT,
                0,
                wide.as_ptr() as LPARAM,
                SMTO_ABORTIFHUNG,
                400,
                &mut result,
            )
        } == 0
        {
            return Err(format!("window_title_update_failed: win32={}", unsafe {
                GetLastError()
            }));
        }
        if result == 0 {
            return Err("window_title_update_rejected".into());
        }
        Ok(())
    }

    fn exact_proof_caption(hwnd: HWND) -> Result<String, String> {
        // SAFETY: These queries do not dereference HWND or mutate its target.
        if unsafe { IsWindowUnicode(hwnd) } == 0 {
            return Err("window_title_proof_requires_unicode".into());
        }
        let mut original = [0u16; 2048];
        // SAFETY: GetWindowTextW writes at most the advertised buffer capacity;
        // resetting thread-local error distinguishes failure from an empty title.
        unsafe { SetLastError(0) };
        let length = unsafe { GetWindowTextW(hwnd, original.as_mut_ptr(), original.len() as i32) };
        let error = unsafe { GetLastError() };
        if length == 0 && error != 0 {
            return Err(format!(
                "window_title_proof_caption_unavailable: win32={error}"
            ));
        }
        super::proof_caption(
            &original[..length as usize],
            length as usize >= original.len() - 1,
        )
    }

    fn verified_proof_caption(
        hwnd: HWND,
        pid: u32,
        path: &str,
        creation: &str,
        arch: &str,
        class: &str,
    ) -> Result<String, String> {
        // Recovery must still work if the game becomes hidden or clears its
        // caption after the challenge. Discovery's visible/title filters are
        // not identity guards. Freeze the expected class and architecture too.
        bound_identity(hwnd, pid, path, creation, arch, class).map_err(str::to_owned)?;
        exact_proof_caption(hwnd)
    }
    struct TitleProof {
        hwnd: HWND,
        pid: u32,
        path: String,
        creation: String,
        arch: String,
        class: String,
        original: String,
        nonce: String,
        restored: bool,
        restore_attempted: bool,
        write_may_be_pending: bool,
    }
    impl TitleProof {
        fn restore(&mut self) -> Result<(), String> {
            if self.restored {
                return Ok(());
            }
            if self.restore_attempted {
                return Err("original title restoration already failed".into());
            }
            self.restore_attempted = true;
            let actual = verified_proof_caption(
                self.hwnd,
                self.pid,
                &self.path,
                &self.creation,
                &self.arch,
                &self.class,
            )?;
            if actual == self.original {
                // A timed-out WM_SETTEXT may still be executing in the target.
                // A single unchanged read is not restoration proof. Require an
                // acknowledged write of the backed-up value through that same
                // window procedure, then recheck identity and exact caption.
                if self.write_may_be_pending {
                    bounded_title(self.hwnd, &self.original)?;
                    let actual = verified_proof_caption(
                        self.hwnd,
                        self.pid,
                        &self.path,
                        &self.creation,
                        &self.arch,
                        &self.class,
                    )?;
                    if actual != self.original {
                        return Err("original title restoration was not confirmed".into());
                    }
                }
                self.restored = true;
                return Ok(());
            }
            if actual != self.nonce {
                return Err(
                    "window title changed during proof; unrelated title was preserved".into(),
                );
            }
            bounded_title(self.hwnd, &self.original)?;
            let actual = verified_proof_caption(
                self.hwnd,
                self.pid,
                &self.path,
                &self.creation,
                &self.arch,
                &self.class,
            )?;
            if actual != self.original {
                return Err("original title restoration was not confirmed".into());
            }
            self.restored = true;
            Ok(())
        }
    }
    impl Drop for TitleProof {
        fn drop(&mut self) {
            if !self.restore_attempted {
                let _ = self.restore();
            }
        }
    }
    fn window_title_challenge(args: &[String]) -> Result<(), String> {
        use std::io::{BufRead, Read};
        if args.len() != 8 {
            return Err(
                "usage: --window-title-challenge HWND PID IMAGE CREATION NONCE ARCH CLASS".into(),
            );
        }
        let handle = args[1].parse::<usize>().map_err(|_| "invalid HWND")?;
        let pid = args[2].parse::<u32>().map_err(|_| "invalid PID")?;
        let nonce = &args[5];
        if handle == 0
            || pid == 0
            || !matches!(args[6].as_str(), "x86" | "x64")
            || args[7].is_empty()
            || !nonce.starts_with("LunaWindowProof-")
            || !(32..=112).contains(&nonce.len())
            || !nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("invalid proof identity or nonce".into());
        }
        let hwnd = handle as HWND;
        bound_identity(hwnd, pid, &args[3], &args[4], &args[6], &args[7]).map_err(str::to_owned)?;
        // Windows normally converts W messages for ANSI windows. A custom game
        // procedure can convert them incorrectly, and an ANSI code page may not
        // represent the original Japanese caption at all. No external writer
        // tested can guarantee restoration in that case. Refuse before mutation.
        // SAFETY: bound_identity has checked this HWND and complete process identity.
        if unsafe { IsWindowUnicode(hwnd) } == 0 {
            return proof_line(&serde_json::json!({
                "status":"unavailable", "errorCode":"window_title_proof_requires_unicode"
            }));
        }
        let original = verified_proof_caption(hwnd, pid, &args[3], &args[4], &args[6], &args[7])?;
        let mut guard = TitleProof {
            hwnd,
            pid,
            path: args[3].clone(),
            creation: args[4].clone(),
            arch: args[6].clone(),
            class: args[7].clone(),
            original,
            nonce: nonce.clone(),
            restored: false,
            restore_attempted: false,
            write_may_be_pending: false,
        };
        // Handle every ordinary failure explicitly, including the initial write,
        // readback, stdout failure and observer EOF. Drop is only a final unwind
        // fallback; its ignored result must never hide the normal restore result.
        let attempt: Result<bool, String> = (|| {
            guard.write_may_be_pending = true;
            bounded_title(hwnd, nonce)?;
            guard.write_may_be_pending = false;
            if verified_proof_caption(
                hwnd,
                pid,
                &guard.path,
                &guard.creation,
                &guard.arch,
                &guard.class,
            )? != *nonce
            {
                return Err("challenge title not confirmed".into());
            }
            proof_line(&serde_json::json!({"status":"ready"}))?;
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            thread::spawn(move || {
                let mut line = String::new();
                let result = io::BufReader::new(io::stdin().take(65)).read_line(&mut line);
                let _ = tx.send(result.ok().filter(|_| line.trim() == "restore").is_some());
            });
            Ok(rx
                .recv_timeout(Duration::from_millis(3000))
                .unwrap_or(false))
        })();
        let restored = guard.restore();
        let requested = match (attempt, restored) {
            (_, Err(error)) => return Err(format!("title_restore_failed: {error}")),
            (Err(error), Ok(())) => return Err(format!("{error}; original_title_restored")),
            (Ok(requested), Ok(())) => requested,
        };
        proof_line(
            &serde_json::json!({"status":"restored","titleRestored":true,"identityVerified":true,"requested":requested}),
        )
    }

    fn enumerate_windows() -> Result<Vec<WindowTarget>, String> {
        window_query::with_physical_pixels(&WindowsThreadDpiApi::load(), enumerate_windows_query)
            .map_err(|error| error.to_string())
    }

    fn enumerate_windows_query() -> Result<Vec<WindowTarget>, String> {
        let mut windows = Vec::<WindowTarget>::new();
        // SAFETY: The callback runs synchronously; lparam points to this live Vec.
        if unsafe {
            EnumWindows(
                Some(enum_window),
                (&mut windows as *mut Vec<WindowTarget>) as LPARAM,
            )
        } == 0
        {
            return Err(last_error("EnumWindows"));
        }
        Ok(windows)
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> BOOL {
        if let Some(window) = inspect_window(hwnd) {
            // SAFETY: enumerate_windows supplies this exclusive live Vec for the synchronous callback.
            unsafe { (&mut *(data as *mut Vec<WindowTarget>)).push(window) };
        }
        1
    }

    fn inspect_window(hwnd: HWND) -> Option<WindowTarget> {
        // SAFETY: The Win32 queries accept nullable/expired HWNDs and report failure without dereferencing them.
        if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 {
            return None;
        }
        let mut pid = 0;
        // SAFETY: pid is a writable process-id output.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        // SAFETY: GetCurrentProcessId has no preconditions.
        if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
            return None;
        }
        let mut title = [0u16; 2048];
        let mut class_name = [0u16; 256];
        // SAFETY: Both fixed buffers have their advertised writable capacity.
        let title_len = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) };
        // SAFETY: The class-name buffer has its advertised capacity.
        let class_len =
            unsafe { GetClassNameW(hwnd, class_name.as_mut_ptr(), class_name.len() as i32) };
        if title_len <= 0 {
            return None;
        }
        let class_name = String::from_utf16_lossy(&class_name[..class_len.max(0) as usize]);
        if matches!(
            class_name.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "WineDesktop"
        ) {
            return None;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: rect is a writable RECT output.
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0
            || !window_query::valid_window_rect(rect.left, rect.top, rect.right, rect.bottom)
        {
            return None;
        }
        let target = inspect(pid, String::new(), false);
        let exe = target
            .full_image_path
            .as_deref()
            .and_then(|path| path.rsplit(['\\', '/']).next())
            .unwrap_or("")
            .to_owned();
        Some(WindowTarget {
            hwnd: (hwnd as usize).to_string(),
            title: String::from_utf16_lossy(&title[..title_len as usize]),
            class_name,
            rect: WindowRect {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            },
            target: Process { exe, ..target },
        })
    }

    fn pick_window(timeout: Duration) -> Result<(), String> {
        // A picked/cancelled result is published only after the original
        // thread context was restored. Keep cursor hit-testing and inspection
        // in the same physical-pixel scope as window enumeration.
        let result = window_query::with_physical_pixels(&WindowsThreadDpiApi::load(), || {
            pick_window_query(timeout)
        })
        .map_err(|error| error.to_string())?;
        write_json(&result)
    }

    fn pick_window_query(timeout: Duration) -> Result<serde_json::Value, String> {
        let start = Instant::now();
        // Wait for the initiating click to be released. Only a later press selects.
        let mut released = false;
        loop {
            // SAFETY: These queries only read global key state and synthesize no input.
            if unsafe { GetAsyncKeyState(VK_ESCAPE as i32) } < 0 {
                return Ok(serde_json::json!({"status":"cancelled", "window":null}));
            }
            if start.elapsed() >= timeout {
                return Ok(serde_json::json!({"status":"timeout", "window":null}));
            }
            // SAFETY: Reads the left-button state without changing it.
            let pressed = unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } < 0;
            if !pressed {
                released = true;
            }
            if pressed && released {
                let mut point = POINT { x: 0, y: 0 };
                // SAFETY: point is writable; WindowFromPoint/GetAncestor accept their returned handles.
                let selected = unsafe {
                    if GetCursorPos(&mut point) == 0 {
                        None
                    } else {
                        inspect_window(GetAncestor(WindowFromPoint(point), GA_ROOT))
                    }
                };
                return Ok(
                    serde_json::json!({"status":if selected.is_some() {"picked"} else {"not_found"}, "window":selected}),
                );
            }
            thread::sleep(Duration::from_millis(8));
        }
    }

    fn inspect(pid: u32, exe: String, inspect_modules: bool) -> Process {
        let mut process = Process {
            pid,
            exe,
            full_image_path: None,
            creation_filetime: None,
            architecture: None,
            modules: Vec::new(),
            errors: Vec::new(),
        };
        // SAFETY: The requested access allows read-only process identity queries.
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if raw.is_null() {
            process.errors.push(last_error("OpenProcess"));
        } else {
            let handle = OwnedHandle(raw);
            match image_path(handle.0) {
                Ok(path) => process.full_image_path = Some(path),
                Err(error) => process.errors.push(error),
            }
            match creation_time(handle.0) {
                Ok(time) => process.creation_filetime = Some(time.to_string()),
                Err(error) => process.errors.push(error),
            }
            match architecture(handle.0) {
                Ok(architecture) => process.architecture = Some(architecture),
                Err(error) => process.errors.push(error),
            }
        }
        if inspect_modules {
            match modules(pid) {
                Ok(modules) => process.modules = modules,
                Err(error) => process.errors.push(error),
            }
        }
        process
    }

    fn image_path(handle: HANDLE) -> Result<String, String> {
        let mut buffer = vec![0_u16; 32_768];
        let mut length = buffer.len() as u32;
        // SAFETY: Handle stays open and the buffer has length writable UTF-16 units.
        if unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) } == 0 {
            return Err(last_error("QueryFullProcessImageNameW"));
        }
        String::from_utf16(&buffer[..length as usize])
            .map_err(|error| format!("image path UTF-16: {error}"))
    }

    fn creation_time(handle: HANDLE) -> Result<u64, String> {
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        // SAFETY: Handle is open and all out-pointers refer to writable FILETIMEs.
        if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } == 0
        {
            return Err(last_error("GetProcessTimes"));
        }
        Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    fn architecture(handle: HANDLE) -> Result<&'static str, String> {
        // SAFETY: SYSTEM_INFO is a plain output record filled by the following query.
        let mut system: SYSTEM_INFO = unsafe { std::mem::zeroed() };
        // SAFETY: The output pointer refers to a writable SYSTEM_INFO.
        unsafe { GetNativeSystemInfo(&mut system) };
        // SAFETY: GetNativeSystemInfo initializes this architecture union variant.
        if unsafe { system.Anonymous.Anonymous.wProcessorArchitecture }
            != PROCESSOR_ARCHITECTURE_AMD64
        {
            return Err("cannot establish x86/x64 architecture on reported native CPU".into());
        }
        let mut wow64 = 0;
        // SAFETY: Handle is open and the BOOL out-pointer is writable.
        if unsafe { IsWow64Process(handle, &mut wow64) } == 0 {
            return Err(last_error("IsWow64Process"));
        }
        Ok(if wow64 == 0 { "x64" } else { "x86" })
    }

    fn modules(pid: u32) -> Result<Vec<String>, String> {
        // SAFETY: This only takes a read-only module snapshot for the target process.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(last_error("CreateToolhelp32Snapshot(modules)"));
        }
        let snapshot = OwnedHandle(raw);
        // SAFETY: MODULEENTRY32W is a plain Win32 output record, sized below.
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
        // SAFETY: Snapshot is valid and the output record has the required size.
        if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
            return Err(last_error("Module32FirstW"));
        }
        let mut modules = Vec::new();
        loop {
            modules.push(wide(&entry.szModule));
            // SAFETY: Snapshot and correctly sized output record remain alive.
            if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(18) {
                    break;
                }
                return Err(format!("Module32NextW: {error}"));
            }
        }
        Ok(modules)
    }

    fn wide(buffer: &[u16]) -> String {
        let length = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    }

    fn last_error(operation: &str) -> String {
        format!("{operation}: {}", io::Error::last_os_error())
    }
}
