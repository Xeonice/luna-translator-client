//! GPL-3.0-or-later. Isolated single-operation Windows translation worker.
//! ABI source: LunaTranslator commit 31b502ebf34c35d27276a3d5aaf58ca7b45f0f3b,
//! src/NativeImpl/LunaSubprocess/{jbj7,kingsoft}.cpp. No vendor DLL/data bundled.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
};
const PROTOCOL: &str = "translation-worker-v1";
const MAX_FRAME: usize = 1_048_576;
type Result<T> = std::result::Result<T, (&'static str, &'static str)>;
fn fail(code: &'static str, stage: &'static str) -> (&'static str, &'static str) {
    (code, stage)
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or(fail("protocol_violation", "field"))
}
fn read_frame() -> Result<Value> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take((MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|_| fail("protocol_violation", "read"))?;
    if bytes.len() > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err(fail("protocol_violation", "frame_budget"));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| fail("protocol_violation", "decode"))?;
    if field(&value, "protocol")? != PROTOCOL {
        return Err(fail("protocol_violation", "protocol"));
    }
    Ok(value)
}
fn emit(value: Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(&value).map_err(|_| fail("protocol_violation", "encode"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_FRAME {
        return Err(fail("output_too_large", "frame_budget"));
    }
    io::stdout()
        .lock()
        .write_all(&bytes)
        .and_then(|_| io::stdout().flush())
        .map_err(|_| fail("protocol_violation", "write"))
}
fn file_digest(path: &Path) -> Result<(u64, String)> {
    let mut f = std::fs::File::open(path).map_err(|_| fail("missing_resource", "resource_open"))?;
    let len = f
        .metadata()
        .map_err(|_| fail("invalid_resource", "resource_metadata"))?
        .len();
    if len > 34_359_738_368 {
        return Err(fail("resource_budget_exceeded", "file_budget"));
    }
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0; 65536];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|_| fail("invalid_resource", "resource_read"))?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > len {
            return Err(fail("resource_changed", "resource_length"));
        }
        digest.update(&buf[..n]);
    }
    Ok((count, format!("{:x}", digest.finalize())))
}
fn resource_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty()
        || relative.contains(['\\', '\0'])
        || relative
            .split('/')
            .any(|s| s.is_empty() || matches!(s, "." | ".."))
        || Path::new(relative).is_absolute()
    {
        return Err(fail("invalid_resource", "resource_path"));
    }
    let mut path = root.to_owned();
    for part in relative.split('/') {
        path.push(part);
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|_| fail("missing_resource", "resource_path"))?;
        if meta.file_type().is_symlink() {
            return Err(fail("invalid_resource", "resource_symlink"));
        }
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| fail("missing_resource", "resource_path"))?;
    if !canonical.starts_with(
        root.canonicalize()
            .map_err(|_| fail("missing_resource", "resource_root"))?,
    ) {
        return Err(fail("invalid_resource", "resource_escape"));
    }
    Ok(canonical)
}
fn verify(resource: &Value) -> Result<PathBuf> {
    let root = PathBuf::from(field(resource, "root")?);
    if !root.is_absolute() || !root.is_dir() {
        return Err(fail("not_installed", "resource_root"));
    }
    let files = resource["files"]
        .as_array()
        .ok_or(fail("invalid_resource", "resource_manifest"))?;
    if files.is_empty() || files.len() > 16384 {
        return Err(fail("invalid_resource", "resource_budget"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for file in files {
        let relative = field(file, "relativePath")?;
        if !seen.insert(relative) {
            return Err(fail("invalid_resource", "duplicate_resource"));
        }
        let expected = file["length"]
            .as_u64()
            .ok_or(fail("invalid_resource", "resource_length"))?;
        total = total
            .checked_add(expected)
            .ok_or(fail("resource_budget_exceeded", "resource_budget"))?;
        if total > 34_359_738_368 {
            return Err(fail("resource_budget_exceeded", "resource_budget"));
        }
        let (len, hash) = file_digest(&resource_path(&root, relative)?)?;
        if len != expected || hash != field(file, "sha256")? {
            return Err(fail("resource_changed", "resource_digest"));
        }
    }
    let data = serde_json::to_vec(&json!([
        files,
        resource["route"],
        resource["engineVersion"],
        resource["identity"]
    ]))
    .map_err(|_| fail("invalid_resource", "manifest"))?;
    if format!("{:x}", Sha256::digest(data)) != field(resource, "digest")? {
        return Err(fail("resource_changed", "manifest_digest"));
    }
    Ok(root)
}
#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        ffi::{c_char, c_void},
        os::windows::ffi::OsStrExt,
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    pub(super) struct Library(*mut c_void);
    impl Drop for Library {
        fn drop(&mut self) {
            // SAFETY: handle came from successful LoadLibraryExW, uniquely owned by this process object.
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }
    impl Library {
        fn open(path: &Path) -> Result<Self> {
            let path = wide(path.as_os_str());
            // SAFETY: NUL-terminated absolute path lives through call; no handle supplied. Search
            // scope is only DLL's own directory and system/default safe directories.
            let handle =
                unsafe { LoadLibraryExW(path.as_ptr(), std::ptr::null_mut(), 0x100 | 0x1000) };
            if handle.is_null() {
                return Err(fail("engine_failed", "dll_load"));
            }
            Ok(Self(handle))
        }
        fn symbol(&self, name: &'static [u8]) -> Result<*mut c_void> {
            // SAFETY: all call sites pass static NUL-terminated ASCII export names and this live handle.
            let ptr = unsafe { GetProcAddress(self.0, name.as_ptr().cast()) };
            if ptr.is_null() {
                return Err(fail("runtime_incompatible", "dll_export"));
            }
            Ok(ptr)
        }
    }
    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }
    type JcTransfer = unsafe extern "C" fn(
        i32,
        u32,
        u32,
        i32,
        i32,
        *const u16,
        *mut u16,
        *mut i32,
        *mut u16,
        *mut i32,
    ) -> i32;
    type JcOpen = unsafe extern "C" fn(*mut u16, i32) -> i32;
    type StartSession =
        unsafe extern "system" fn(*mut u16, *mut c_void, *mut c_void, *const u16) -> u32;
    type OpenEngine = unsafe extern "system" fn(i32) -> u32;
    type SetBasicDictPath = unsafe extern "system" fn(i32, *const u16) -> u32;
    type SimpleTrans = unsafe extern "system" fn(i32, *const u16, *mut u16, i32, i32) -> u32;
    pub enum Engine {
        Jb {
            _lib: Library,
            transfer: JcTransfer,
            target: u32,
            _user_dictionaries: Box<[u16; 1548]>,
        },
        Kingsoft {
            _lib: Library,
            translate: SimpleTrans,
            _arena: Box<[u8]>,
            _dictionary: Vec<u16>,
        },
    }
    impl Engine {
        pub fn open(engine: &str, resource: &Value, root: &Path) -> Result<Self> {
            if !cfg!(target_pointer_width = "32") {
                return Err(fail("architecture_mismatch", "worker_32bit_required"));
            }
            let route = resource["route"]
                .as_array()
                .ok_or(fail("invalid_resource", "route"))?;
            if route.len() != 1 {
                return Err(fail("unsupported_pair", "direct_route"));
            }
            let pair = &route[0]["pair"];
            if engine == "jbeijing" {
                if field(resource, "engineVersion")? != "lunatranslator-jbj7-unicode-v1"
                    || field(pair, "source")? != "ja"
                {
                    return Err(fail("unsupported_pair", "jbeijing_pair"));
                }
                let target = match field(pair, "target")? {
                    "zh" | "zh-Hans" => 936,
                    "zh-Hant" => 950,
                    _ => return Err(fail("unsupported_pair", "jbeijing_pair")),
                };
                let dll = resource_path(root, "JBJCT.dll")?;
                let lib = Library::open(&dll)?;
                // SAFETY: exact cdecl signatures are frozen from the reviewed upstream jbj7.cpp ABI.
                let transfer = unsafe {
                    std::mem::transmute::<*mut c_void, JcTransfer>(
                        lib.symbol(b"JC_Transfer_Unicode\0")?,
                    )
                };
                // SAFETY: same upstream declares this exact cdecl dictionary initializer.
                let open = unsafe {
                    std::mem::transmute::<*mut c_void, JcOpen>(
                        lib.symbol(b"DJC_OpenAllUserDic_Unicode\0")?,
                    )
                };
                let mut user_dictionaries = Box::new([0u16; 1548]);
                // SAFETY: three fixed 0x204 UTF16 dictionary slots are present; no user dictionaries
                // are silently discovered. Vendor owns no caller buffer after this single call.
                unsafe {
                    open(user_dictionaries.as_mut_ptr(), 0);
                }
                Ok(Self::Jb {
                    _lib: lib,
                    transfer,
                    target,
                    _user_dictionaries: user_dictionaries,
                })
            } else if engine == "kingsoft_fastait" {
                if field(resource, "engineVersion")? != "lunatranslator-fastait-unicode-v1" {
                    return Err(fail("runtime_incompatible", "kingsoft_abi"));
                }
                fn lang(s: &str) -> Result<&'static str> {
                    match s {
                        "ja" => Ok("Japanese"),
                        "zh" | "zh-Hans" => Ok("SChinese"),
                        "zh-Hant" => Ok("TChinese"),
                        "en" => Ok("English"),
                        _ => Err(fail("unsupported_pair", "kingsoft_pair")),
                    }
                }
                let prefix = format!(
                    "GTS/{}{}/",
                    lang(field(pair, "source")?)?,
                    lang(field(pair, "target")?)?
                );
                let files = resource["files"]
                    .as_array()
                    .ok_or(fail("invalid_resource", "resource_files"))?;
                let dlls = files
                    .iter()
                    .filter_map(|f| f["relativePath"].as_str())
                    .filter(|p| p.starts_with(&prefix) && p.to_ascii_lowercase().ends_with(".dll"))
                    .collect::<Vec<_>>();
                if dlls.len() != 1 {
                    return Err(fail("invalid_resource", "ambiguous_engine_dll"));
                }
                let lib = Library::open(&resource_path(root, dlls[0])?)?;
                let dictionary = resource_path(root, &format!("{prefix}DCT"))?;
                // SAFETY: all four exact stdcall signatures are frozen from upstream kingsoft.cpp.
                let (start, open, set, translate) = unsafe {
                    (
                        std::mem::transmute::<*mut c_void, StartSession>(
                            lib.symbol(b"StartSession\0")?,
                        ),
                        std::mem::transmute::<*mut c_void, OpenEngine>(
                            lib.symbol(b"OpenEngine\0")?,
                        ),
                        std::mem::transmute::<*mut c_void, SetBasicDictPath>(
                            lib.symbol(b"SetBasicDictPathW\0")?,
                        ),
                        std::mem::transmute::<*mut c_void, SimpleTrans>(
                            lib.symbol(b"SimpleTransSentM\0")?,
                        ),
                    )
                };
                let mut dictionary = wide(dictionary.as_os_str());
                let mut arena = vec![0u8; 0x4f4].into_boxed_slice();
                static APP: [u16; 4] = [b'D' as u16, b'C' as u16, b'T' as u16, 0];
                // SAFETY: arena spans the exact source ABI's range and remains alive for engine's
                // lifetime; absolute UTF16 dictionary path is also retained. No guessed close ABI.
                unsafe {
                    start(
                        dictionary.as_mut_ptr(),
                        arena.as_mut_ptr().cast(),
                        arena.as_mut_ptr().add(arena.len()).cast(),
                        APP.as_ptr(),
                    );
                    open(0x4f4);
                    set(0x4f4, dictionary.as_ptr());
                }
                Ok(Self::Kingsoft {
                    _lib: lib,
                    translate,
                    _arena: arena,
                    _dictionary: dictionary,
                })
            } else {
                Err(fail("unsupported_engine", "engine"))
            }
        }
        pub fn translate(&mut self, text: &str) -> Result<String> {
            let mut input = text.encode_utf16().collect::<Vec<_>>();
            input.push(0);
            let (capacity, limit) = match self {
                Self::Jb { .. } => (3000usize, 2999usize),
                Self::Kingsoft { .. } => (1024, 511),
            };
            // Vendor ABI has a fixed destination and no independent size argument. Refuse oversized
            // input; process isolation contains vendor faults. Real output-memory safety needs actual
            // vendor binaries and remains a separate supported-version qualification.
            if input.len() > limit + 1 {
                return Err(fail("unsupported_input", "vendor_utf16_budget"));
            }
            let mut output = vec![0u16; capacity];
            match self {
                Self::Jb {
                    transfer, target, ..
                } => {
                    let mut scratch = vec![0u16; 3000];
                    let (mut a, mut b) = (3000i32, 3000i32);
                    // SAFETY: exact reviewed ABI, terminated bounded input, output/scratch and inout
                    // capacities remain live. Native DLL is loaded only inside this disposable worker.
                    unsafe {
                        transfer(
                            0,
                            932,
                            *target,
                            1,
                            1,
                            input.as_ptr(),
                            output.as_mut_ptr(),
                            &mut a,
                            scratch.as_mut_ptr(),
                            &mut b,
                        );
                    }
                    if !(0..=3000).contains(&a) || !(0..=3000).contains(&b) {
                        return Err(fail("engine_failed", "vendor_capacity"));
                    }
                }
                Self::Kingsoft { translate, .. } => {
                    // SAFETY: exact reviewed upstream ABI and fixed buffer. 0x28/0x4 are the upstream
                    // operation arguments, not guessed size parameters. Worker is never a game process.
                    unsafe {
                        translate(0x4f4, input.as_ptr(), output.as_mut_ptr(), 0x28, 0x4);
                    }
                }
            }
            let count = output
                .iter()
                .position(|&u| u == 0)
                .ok_or(fail("engine_failed", "unterminated_output"))?;
            String::from_utf16(&output[..count]).map_err(|_| fail("engine_failed", "invalid_utf16"))
        }
    }
}
fn run(hello: &Value) -> Result<()> {
    let instance = field(hello, "workerInstanceId")?;
    let request_id = field(hello, "requestId")?;
    if field(hello, "kind")? != "hello" || hello["contractRevision"] != 1 {
        return Err(fail("protocol_violation", "hello"));
    }
    let engine = field(hello, "engineId")?;
    if !matches!(engine, "jbeijing" | "kingsoft_fastait") {
        return Err(fail("unsupported_engine", "engine"));
    }
    let resource = &hello["resource"];
    let root = verify(resource)?;
    #[cfg(not(windows))]
    {
        let _ = (instance, request_id, root);
        return Err(fail("unsupported_platform", "windows_required"));
    }
    #[cfg(windows)]
    {
        let mut native = native::Engine::open(engine, resource, &root)?;
        let observation = json!({"engineId":engine,"engineContractRevision":1,"runtimeVersion":field(resource,"engineVersion")?,"platform":"windows","architecture":if cfg!(target_pointer_width="32"){"x86"}else{"x64"},"availability":"installed","physicalCancelSupported":false,"resourceVersion":null,"reason":null});
        emit(
            json!({"kind":"ready","protocol":PROTOCOL,"workerInstanceId":instance,"requestId":request_id,"resourceDigest":resource["digest"],"observation":observation}),
        )?;
        if field(hello, "operation")? == "observe" {
            return Ok(());
        }
        if field(hello, "operation")? != "translate" {
            return Err(fail("unsupported_input", "resource_prepare_external"));
        }
        let execute = read_frame()?;
        if field(&execute, "kind")? == "cancel" {
            return Err(fail("cancelled", "before_execute"));
        }
        if field(&execute, "kind")? != "execute"
            || field(&execute, "workerInstanceId")? != instance
            || execute["identity"] != hello["identity"]
            || hello["identity"].is_null()
        {
            return Err(fail("protocol_violation", "execute_identity"));
        }
        let text = field(&execute, "sourceText")?;
        if text.is_empty()
            || text.len() > 65536
            || text.contains('\0')
            || format!("{:x}", Sha256::digest(text.as_bytes()))
                != field(&hello["identity"], "sourceHash")?
        {
            return Err(fail("invalid_request", "input"));
        }
        let output = native.translate(text)?;
        if output.is_empty() || output.len() > 262144 {
            return Err(fail("engine_failed", "empty_or_large_output"));
        }
        verify(resource)?;
        emit(
            json!({"kind":"final","protocol":PROTOCOL,"workerInstanceId":instance,"identity":hello["identity"],"resourceIdentity":resource["identity"],"resourceDigest":resource["digest"],"route":resource["route"],"text":output}),
        )
    }
}
fn main() {
    let hello = match read_frame() {
        Ok(h) => h,
        Err(_) => std::process::exit(2),
    };
    if let Err((code, stage)) = run(&hello) {
        let _ = emit(
            json!({"kind":"error","protocol":PROTOCOL,"workerInstanceId":hello["workerInstanceId"],"requestId":hello["requestId"],"error":{"code":code,"stage":stage}}),
        );
    }
}
