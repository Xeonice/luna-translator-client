use hook_contracts::{Architecture, AttachParams, ErrorCode};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, STILL_ACTIVE},
    System::{
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        SystemInformation::{GetNativeSystemInfo, PROCESSOR_ARCHITECTURE_AMD64, SYSTEM_INFO},
        Threading::{
            GetCurrentProcessId, GetExitCodeProcess, GetProcessTimes, IsWow64Process, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    },
};

pub type TargetError = (ErrorCode, String);

pub struct TargetIdentity {
    handle: HANDLE,
    pub architecture: Architecture,
}

impl Drop for TargetIdentity {
    fn drop(&mut self) {
        // SAFETY: This struct uniquely owns a successfully opened process handle.
        unsafe { CloseHandle(self.handle) };
    }
}

impl TargetIdentity {
    pub fn validate(request: &AttachParams) -> Result<Self, TargetError> {
        // SAFETY: This Win32 query takes no pointers and cannot invalidate state.
        if request.pid == unsafe { GetCurrentProcessId() } {
            return Err((
                ErrorCode::TargetIdentity,
                "cannot attach the host to itself".into(),
            ));
        }
        // SAFETY: Access is limited to read-only identity queries; no elevation.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, request.pid) };
        if handle.is_null() {
            return Err(last_error("OpenProcess"));
        }
        let result = (|| {
            let mut path = vec![0_u16; 32_768];
            let mut length = path.len() as u32;
            // SAFETY: The mutable buffer is allocated for length UTF-16 units.
            if unsafe { QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut length) } == 0
            {
                return Err(last_error("QueryFullProcessImageNameW"));
            }
            let actual_path = String::from_utf16(&path[..length as usize])
                .map_err(|e| (ErrorCode::TargetIdentity, e.to_string()))?;
            let expected_path =
                std::fs::canonicalize(&request.expected_image_path).map_err(|e| {
                    (
                        ErrorCode::TargetIdentity,
                        format!("expected image path cannot be resolved: {e}"),
                    )
                })?;
            if normalize(&actual_path) != normalize(&expected_path.to_string_lossy()) {
                return Err((
                    ErrorCode::TargetIdentity,
                    format!("image path mismatch: actual {actual_path}"),
                ));
            }
            let mut creation = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit = creation;
            let mut kernel = creation;
            let mut user = creation;
            // SAFETY: All out-pointers refer to initialized writable FILETIMEs.
            if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) }
                == 0
            {
                return Err(last_error("GetProcessTimes"));
            }
            let actual_time =
                (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
            if actual_time != request.expected_creation_time.0 {
                return Err((
                    ErrorCode::TargetIdentity,
                    "creation FILETIME mismatch; PID may have been reused".into(),
                ));
            }
            let architecture = architecture(handle)?;
            Ok(Self {
                handle,
                architecture,
            })
        })();
        if result.is_err() {
            // SAFETY: Failure means ownership was not transferred to the result.
            unsafe { CloseHandle(handle) };
        }
        result
    }

    pub fn ensure_alive(&self) -> Result<(), TargetError> {
        let mut code = 0;
        // SAFETY: The process handle stays owned by this identity guard.
        if unsafe { GetExitCodeProcess(self.handle, &mut code) } == 0 || code != STILL_ACTIVE as u32
        {
            return Err((
                ErrorCode::TargetIdentity,
                "target exited before injection".into(),
            ));
        }
        Ok(())
    }
}

fn architecture(handle: HANDLE) -> Result<Architecture, TargetError> {
    let kernel: Vec<u16> = OsStr::new("kernel32.dll")
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: NUL-terminated library name; this query does not change refcounts.
    let module = unsafe { GetModuleHandleW(kernel.as_ptr()) };
    // SAFETY: kernel32 is a process-lifetime module; the symbol name is terminated.
    let function = unsafe { GetProcAddress(module, c"IsWow64Process2".as_ptr().cast()) };
    if let Some(function) = function {
        type IsWow64Process2Fn = unsafe extern "system" fn(HANDLE, *mut u16, *mut u16) -> i32;
        // SAFETY: This documented kernel32 symbol has precisely this Win32 ABI.
        let function: IsWow64Process2Fn = unsafe { std::mem::transmute(function) };
        let mut process_machine = 0_u16;
        let mut native_machine = 0_u16;
        // SAFETY: Valid process handle and writable machine-code out-pointers.
        if unsafe { function(handle, &mut process_machine, &mut native_machine) } != 0 {
            return match (process_machine, native_machine) {
                (0x014c, _) => Ok(Architecture::X86),
                (0x8664, _) | (0, 0x8664) => Ok(Architecture::X64),
                _ => Err((
                    ErrorCode::UnsupportedArchitecture,
                    format!("unsupported machine {process_machine:#x}, native {native_machine:#x}"),
                )),
            };
        }
    }
    // Older Wine may expose an absent/unimplemented IsWow64Process2. This
    // fallback is sound only when the reported native CPU is AMD64.
    // SAFETY: SYSTEM_INFO is a C POD initialized before the API fills it.
    let mut system: SYSTEM_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: Writable SYSTEM_INFO out-pointer.
    unsafe { GetNativeSystemInfo(&mut system) };
    // SAFETY: The API initializes the architecture variant of this union.
    if unsafe { system.Anonymous.Anonymous.wProcessorArchitecture } != PROCESSOR_ARCHITECTURE_AMD64
    {
        return Err((
            ErrorCode::UnsupportedArchitecture,
            "cannot establish x86/x64 target architecture on this native CPU".into(),
        ));
    }
    let mut wow64 = 0;
    // SAFETY: Valid process handle and writable BOOL out-pointer.
    if unsafe { IsWow64Process(handle, &mut wow64) } == 0 {
        return Err(last_error("IsWow64Process"));
    }
    Ok(if wow64 != 0 {
        Architecture::X86
    } else {
        Architecture::X64
    })
}

fn normalize(path: &str) -> String {
    let path = path.replace('/', "\\");
    let path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned()
    };
    path.to_lowercase()
}

fn last_error(operation: &str) -> TargetError {
    (
        ErrorCode::TargetIdentity,
        format!("{operation}: {}", std::io::Error::last_os_error()),
    )
}
