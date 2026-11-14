use super::{
    callbacks,
    identity::{TargetError, TargetIdentity},
};
use crate::{abi::*, queue::EventSender, runtime::VerifiedRuntime};
use hook_contracts::{Architecture, AttachParams, ErrorCode, Event};
use libloading::os::windows::{
    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library,
};
use std::{
    fs::File,
    os::windows::{
        io::{FromRawHandle, RawHandle},
        process::CommandExt,
    },
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, INVALID_HANDLE_VALUE},
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle},
        Threading::{CREATE_NO_WINDOW, GetCurrentProcess},
    },
};

pub struct NativeHost {
    pub runtime: &'static VerifiedRuntime,
    connect: ProcessFn,
    check_inject: CheckInjectFn,
    detach: ProcessFn,
}

impl NativeHost {
    pub fn start(
        runtime: VerifiedRuntime,
        events: EventSender<Event>,
        states: Arc<callbacks::ProcessStates>,
    ) -> Result<Self, String> {
        callbacks::install(events, states)?;
        // SAFETY: Every runtime file is hash-verified and held with write/delete
        // sharing disabled. Dependencies are searched in the pinned DLL's
        // directory and standard safe locations, never the working directory.
        let library = unsafe {
            Library::load_with_flags(
                runtime.host_path(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
        .map_err(|e| format!("load pinned Core Host: {e}"))?;
        // There is no Luna_Stop/Join export. Do not FreeLibrary even if a later
        // symbol or startup check fails: native background threads may exist.
        let library = Box::leak(Box::new(library));
        let runtime = Box::leak(Box::new(runtime));
        redirect_crt_stdout()?;
        // SAFETY: The commit/hash allowlist binds these exact cdecl signatures.
        // Library and runtime locks live until process exit; function pointers
        // never outlive the module, including failed initialization paths.
        // ABI check is the first export invocation. A legacy ten-callback DLL
        // cannot be initialized through this seven-callback boundary.
        let abi_version = *unsafe { library.get::<AbiVersionFn>(b"Luna_GetAbiVersion\0") }
            .map_err(|e| format!("missing Core ABI export: {e}"))?;
        if unsafe { abi_version() } != 1 {
            return Err("Core ABI mismatch; expected version 1".into());
        }
        let (start, settings, connect, check_inject, detach) = unsafe {
            (
                *library
                    .get::<StartFn>(b"Luna_Start\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<SettingsFn>(b"Luna_Settings\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<ProcessFn>(b"Luna_ConnectProcess\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<CheckInjectFn>(b"Luna_CheckIfNeedInject\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<ProcessFn>(b"Luna_DetachProcess\0")
                    .map_err(|e| e.to_string())?,
            )
        };
        // SAFETY: All seven callbacks are non-null cdecl functions. They copy
        // payloads into a bounded queue and do not re-enter Host APIs.
        unsafe {
            start(
                callbacks::connected,
                callbacks::disconnected,
                callbacks::created,
                callbacks::destroyed,
                callbacks::output,
                callbacks::info,
                callbacks::inserted,
            );
            // Exact upstream Host/TextThread defaults, including auto-codepage 0.
            settings(100, 0, 3000, 10_000_000, true);
        }
        Ok(Self {
            runtime,
            connect,
            check_inject,
            detach,
        })
    }

    pub fn attach(
        &self,
        request: &AttachParams,
        cancelled: &AtomicBool,
        connection_deadline: Instant,
    ) -> Result<Option<(Architecture, Option<i32>)>, TargetError> {
        check_attach_deadline(cancelled, connection_deadline)?;
        let identity = TargetIdentity::validate(request)?;
        identity.ensure_alive()?;
        if identity.architecture != crate::runtime::host_architecture() {
            return Err((
                ErrorCode::UnsupportedArchitecture,
                "broker and target bitness differ; start the matching broker".into(),
            ));
        }
        check_attach_deadline(cancelled, connection_deadline)?;
        // SAFETY: Identity guard pins the validated process object. This call
        // creates native pipe listeners before loading Hook. It does not wait
        // for the process-connected callback.
        unsafe { (self.connect)(request.pid) };
        check_attach_deadline(cancelled, connection_deadline)?;
        // SAFETY: Read-only host bookkeeping for the same validated target PID.
        let needs_injection = unsafe { (self.check_inject)(request.pid) };
        if !needs_injection {
            return Ok(None);
        }
        identity.ensure_alive()?;
        check_attach_deadline(cancelled, connection_deadline)?;
        let (helper, dll) = match identity.architecture {
            Architecture::X86 => ("LunaInjectCore32.exe", "LunaHookCore32.dll"),
            Architecture::X64 => ("LunaInjectCore64.exe", "LunaHookCore64.dll"),
        };
        let mut child = Command::new(self.runtime.path(helper))
            .arg(request.pid.to_string())
            .arg(self.runtime.path(dll))
            .current_dir(&self.runtime.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| (ErrorCode::InjectionFailed, format!("start {helper}: {e}")))?;
        let deadline = (Instant::now() + Duration::from_secs(12)).min(connection_deadline);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(Some((identity.architecture, status.code()))),
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err((
                        ErrorCode::InjectionFailed,
                        format!("wait for owned injector: {e}"),
                    ));
                }
            }
            if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
                // This is our own injector, never the user's game or Luna GUI.
                let _ = child.kill();
                let _ = child.wait();
                return Err((ErrorCode::InjectionFailed, "owned injector cancelled or exceeded 12-second deadline; no connection success inferred".into()));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn detach(&self, pid: u32) {
        // SAFETY: Only the native worker invokes Host APIs. Disconnect is
        // asynchronous; the supervisor relies on its callback, not this return.
        unsafe { (self.detach)(pid) };
    }
}

fn cancelled_error() -> TargetError {
    (
        ErrorCode::ShuttingDown,
        "attach cancelled by shutdown".into(),
    )
}

fn check_attach_deadline(cancelled: &AtomicBool, deadline: Instant) -> Result<(), TargetError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(cancelled_error());
    }
    if Instant::now() >= deadline {
        return Err((
            ErrorCode::ConnectionTimeout,
            "attach expired before native operation; injector was not started".into(),
        ));
    }
    Ok(())
}

/// Preserve the protocol handle before redirecting Win32 and CRT stdout.
pub fn capture_protocol_stdout() -> Result<File, String> {
    // SAFETY: Queries return handles owned by the current process; DuplicateHandle
    // creates a separate, uniquely owned handle for the JSONL writer.
    let (original, stderr, process) = unsafe {
        (
            GetStdHandle(STD_OUTPUT_HANDLE),
            GetStdHandle(STD_ERROR_HANDLE),
            GetCurrentProcess(),
        )
    };
    if original.is_null()
        || original == INVALID_HANDLE_VALUE
        || stderr.is_null()
        || stderr == INVALID_HANDLE_VALUE
    {
        return Err("stdout and stderr must be redirected to valid streams".into());
    }
    let mut duplicate = std::ptr::null_mut();
    // SAFETY: Valid current-process handles and writable output pointer.
    if unsafe {
        DuplicateHandle(
            process,
            original,
            process,
            &mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(format!(
            "duplicate protocol stdout: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: The duplicate was newly created and transfers sole ownership to File.
    let protocol = unsafe { File::from_raw_handle(duplicate as RawHandle) };
    // SAFETY: Native diagnostic writes should use the already-valid stderr handle.
    if unsafe { SetStdHandle(STD_OUTPUT_HANDLE, stderr) } == 0 {
        return Err(format!(
            "redirect Win32 stdout: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(protocol)
}

fn redirect_crt_stdout() -> Result<(), String> {
    let mut redirected = false;
    for name in ["ucrtbase.dll", "msvcrt.dll"] {
        let Ok(library) = Library::open_already_loaded(name) else {
            continue;
        };
        type Dup2 = unsafe extern "C" fn(i32, i32) -> i32;
        type Flush = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
        // SAFETY: These are documented C runtime functions in known CRT modules.
        let duplicate =
            unsafe { library.get::<Dup2>(b"_dup2\0") }.map_err(|e| format!("{name} _dup2: {e}"))?;
        // SAFETY: NULL flushes all C output streams before descriptor replacement.
        if let Ok(flush) = unsafe { library.get::<Flush>(b"fflush\0") } {
            // SAFETY: NULL requests flushing all valid C output streams.
            unsafe { flush(std::ptr::null_mut()) };
        }
        // SAFETY: C runtimes reserve fd 2 for stderr and fd 1 for stdout.
        if unsafe { duplicate(2, 1) } != 0 {
            return Err(format!("cannot redirect {name} stdout to stderr"));
        }
        redirected = true;
    }
    if !redirected {
        return Err("no loaded CRT found; cannot guarantee JSONL stdout isolation".into());
    }
    Ok(())
}

pub type CancelFlag = Arc<AtomicBool>;
