//! Bounded Apple CLI compatibility capture for an already verified picker window.
//! The caller owns TCC, retained-filter identity, scale and fallback eligibility.
use app_core::contracts::{AppError, AppResult};
use std::ffi::{CString, c_int};
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROGRAM: &str = "/usr/sbin/screencapture";
const ROOT_NAME: &str = "luna-window-preview-v1";
const MARKER: &[u8] = b"luna-window-preview-v1\n";
const FRAME: &str = "frame.jpg";
const OWNER: &str = "owner";
const MAX_SOURCE_PIXELS: u64 = 4_194_304;
const MAX_INPUT: u64 = 2 * 1024 * 1024;
const MAX_OUTPUT: usize = 524_288;
const TIMEOUT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(25);
static IN_FLIGHT: Mutex<()> = Mutex::new(());
// Darwin SDK sys/fcntl.h, sys/resource.h and sys/errno.h constants. This module
// is compiled only for macOS; no dependency or shared platform policy is added.
const O_CLOEXEC: c_int = 0x0100_0000;
const O_DIRECTORY: c_int = 0x0010_0000;
const O_RDWR: c_int = 2;
const O_CREAT: c_int = 0x200;
const O_EXCL: c_int = 0x800;
const O_NOFOLLOW: c_int = 0x100;
const O_NONBLOCK: c_int = 4;
const AT_REMOVEDIR: c_int = 0x80;
#[repr(C)]
struct RLimit {
    current: u64,
    maximum: u64,
}
unsafe extern "C" {
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    fn openat(directory: c_int, name: *const std::ffi::c_char, flags: c_int, ...) -> c_int;
    fn mkdirat(directory: c_int, name: *const std::ffi::c_char, mode: u16) -> c_int;
    fn setrlimit(resource: c_int, limit: *const RLimit) -> c_int;
    fn geteuid() -> u32;
    fn umask(mask: u16) -> u16;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn unlinkat(directory: c_int, name: *const std::ffi::c_char, flags: c_int) -> c_int;
}
fn error(code: &str) -> AppError {
    AppError::new(
        code,
        "preview",
        "这个窗口暂时无法安全预览；取文不受影响，可重试",
    )
}
fn failed() -> AppError {
    error("preview_capture_failed")
}
fn budget() -> AppError {
    error("preview_budget_exceeded")
}
fn encoding() -> AppError {
    error("preview_encoding_failed")
}
fn uid() -> u32 {
    // SAFETY: geteuid has no pointer arguments or ownership side effects.
    unsafe { geteuid() }
}
fn private(metadata: &Metadata, directory: bool) -> bool {
    metadata.uid() == uid()
        && metadata.mode() & 0o7777 == if directory { 0o700 } else { 0o600 }
        && if directory {
            metadata.is_dir()
        } else {
            metadata.is_file() && metadata.nlink() == 1
        }
}
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
fn file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(O_NOFOLLOW | O_NONBLOCK);
    options
}
fn open_relative(directory: &File, name: &str, flags: c_int) -> AppResult<File> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(failed());
    }
    let name = CString::new(name).map_err(|_| failed())?;
    // SAFETY: The held directory and NUL-terminated basename are valid. Flags
    // always reject a symlink at this relative entry, and no parent path is read.
    let fd = unsafe {
        openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK,
            0o600 as c_int,
        )
    };
    if fd < 0 {
        return Err(failed());
    }
    // SAFETY: Successful openat returned a unique owned file descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn create_relative(directory: &File, name: &str) -> AppResult<File> {
    let file = open_relative(directory, name, O_RDWR | O_CREAT | O_EXCL)?;
    if !private(&file.metadata().map_err(|_| failed())?, false) {
        return Err(failed());
    }
    Ok(file)
}
fn open_directory(path: &Path) -> AppResult<File> {
    let file = file_options().open(path).map_err(|_| failed())?;
    if !private(&file.metadata().map_err(|_| failed())?, true) {
        return Err(failed());
    }
    Ok(file)
}
fn random_name() -> AppResult<String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| failed())?;
    let suffix: String = bytes.iter().map(|value| format!("{value:02x}")).collect();
    Ok(format!("{}-{suffix}", std::process::id()))
}
#[cfg(test)]
fn create_file(path: &Path) -> AppResult<File> {
    let file = file_options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| failed())?;
    if !private(&file.metadata().map_err(|_| failed())?, false) {
        return Err(failed());
    }
    Ok(file)
}
fn unlink(directory: &File, name: &str, is_directory: bool) {
    if let Ok(name) = CString::new(name) {
        // SAFETY: The live owned directory descriptor and NUL-terminated single
        // basename name are valid. unlinkat never follows the final symlink.
        let _ = unsafe {
            unlinkat(
                directory.as_raw_fd(),
                name.as_ptr(),
                if is_directory { AT_REMOVEDIR } else { 0 },
            )
        };
    }
}
struct TemporaryCapture {
    root: File,
    directory: File,
    name: String,
    path: PathBuf,
    root_path: PathBuf,
}
fn clean_private_files(path: &Path, directory: &File) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    let Ok(opened) = directory.metadata() else {
        return false;
    };
    if !fs::symlink_metadata(path).is_ok_and(|m| private(&m, true) && same_file(&m, &opened)) {
        return false;
    }
    let entries: Vec<_> = entries.take(17).collect();
    if entries.len() > 16 {
        return false;
    }
    let mut names = Vec::new();
    // Only this unpredictable transaction directory is eligible. Apple atomic
    // write siblings must be same-owner regular single-link private files.
    // The fixed output basename may be unlinked even if a hostile child made
    // it a symlink/hardlink: unlinkat removes that entry, never its target.
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            return false;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return false;
        };
        let known_output = name == FRAME && metadata.uid() == uid() && !metadata.is_dir();
        if !known_output && (!private(&metadata, false) || metadata.dev() != opened.dev()) {
            return false;
        }
        names.push(name);
    }
    // Preserve the ownership marker until all data-file removals succeed.
    for name in names.iter().filter(|name| name.as_str() != OWNER) {
        unlink(directory, name, false);
    }
    if fs::read_dir(path).map_or(true, |mut entries| {
        entries.any(|entry| entry.map_or(true, |entry| entry.file_name() != OWNER))
    }) {
        return false;
    }
    unlink(directory, OWNER, false);
    fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}
impl Drop for TemporaryCapture {
    fn drop(&mut self) {
        // Normal returns explicitly check cleanup. Drop is the panic/unwind
        // safety net; it never recursively touches an unknown directory.
        if clean_private_files(&self.path, &self.directory) {
            unlink(&self.root, &self.name, true);
        }
    }
}
fn dead_owner(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    // SAFETY: Signal zero checks existence only; it neither signals nor stops
    // another process. ESRCH is the only result accepted as a dead owner.
    let result = unsafe { kill(pid as c_int, 0) };
    result == -1 && io::Error::last_os_error().raw_os_error() == Some(3)
}
fn clean_abandoned(root: &Path, root_fd: &File) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    // Bounded best effort; malformed/foreign/active/unknown entries are retained.
    for entry in entries.take(64).flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some((pid, nonce)) = name.split_once('-') else {
            continue;
        };
        if nonce.len() != 32
            || !nonce
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            continue;
        }
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        if !dead_owner(pid) {
            continue;
        }
        let Ok(directory) = open_relative(root_fd, &name, O_DIRECTORY) else {
            continue;
        };
        if !directory.metadata().is_ok_and(|m| private(&m, true)) {
            continue;
        }
        let Ok(mut owner) = open_relative(&directory, OWNER, 0) else {
            continue;
        };
        if !owner
            .metadata()
            .is_ok_and(|m| private(&m, false) && m.len() <= 128)
        {
            continue;
        }
        let mut contents = String::new();
        if Read::by_ref(&mut owner)
            .take(129)
            .read_to_string(&mut contents)
            .is_err()
            || contents != format!("{}{}\n{}\n", String::from_utf8_lossy(MARKER), pid, nonce)
        {
            continue;
        }
        let Ok(opened) = directory.metadata() else {
            continue;
        };
        if !fs::symlink_metadata(entry.path()).is_ok_and(|m| same_file(&m, &opened)) {
            continue;
        }
        if clean_private_files(&entry.path(), &directory) {
            unlink(root_fd, &name, true);
        }
    }
}
impl TemporaryCapture {
    fn validate_directory(&self, final_output: bool) -> AppResult<()> {
        let root = self.root.metadata().map_err(|_| failed())?;
        let directory = self.directory.metadata().map_err(|_| failed())?;
        if !private(&root, true)
            || !private(&directory, true)
            || !fs::symlink_metadata(&self.root_path)
                .is_ok_and(|m| private(&m, true) && same_file(&m, &root))
            || !fs::symlink_metadata(&self.path)
                .is_ok_and(|m| private(&m, true) && same_file(&m, &directory))
        {
            return Err(failed());
        }
        if final_output {
            let entries: Vec<_> = fs::read_dir(&self.path)
                .map_err(|_| failed())?
                .take(3)
                .collect();
            if entries.len() != 2
                || entries.iter().any(|e| {
                    e.as_ref()
                        .map_or(true, |e| e.file_name() != FRAME && e.file_name() != OWNER)
                })
            {
                return Err(failed());
            }
        }
        Ok(())
    }
    fn cleanup(&self) -> AppResult<()> {
        self.validate_directory(false)?;
        if !clean_private_files(&self.path, &self.directory) {
            return Err(failed());
        }
        unlink(&self.root, &self.name, true);
        if self.path.try_exists().map_err(|_| failed())? {
            return Err(failed());
        }
        Ok(())
    }
    fn create(base: &Path) -> AppResult<Self> {
        let root_path = base.canonicalize().map_err(|_| failed())?.join(ROOT_NAME);
        match DirBuilder::new().mode(0o700).create(&root_path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(failed()),
        }
        let root = open_directory(&root_path)?;
        clean_abandoned(&root_path, &root);
        let name = random_name()?;
        let path = root_path.join(&name);
        let c_name = CString::new(name.as_str()).map_err(|_| failed())?;
        // SAFETY: mkdirat uses a held private root and a random single basename.
        if unsafe { mkdirat(root.as_raw_fd(), c_name.as_ptr(), 0o700) } != 0 {
            return Err(failed());
        }
        let directory = match open_relative(&root, &name, O_DIRECTORY) {
            Ok(file) => file,
            Err(e) => {
                unlink(&root, &name, true);
                return Err(e);
            }
        };
        if !directory.metadata().is_ok_and(|m| private(&m, true)) {
            unlink(&root, &name, true);
            return Err(failed());
        }
        let capture = Self {
            root,
            directory,
            name,
            path,
            root_path,
        };
        let nonce = capture.name.split_once('-').ok_or_else(failed)?.1;
        let mut owner = create_relative(&capture.directory, OWNER)?;
        write!(
            owner,
            "{}{}\n{}\n",
            String::from_utf8_lossy(MARKER),
            std::process::id(),
            nonce
        )
        .map_err(|_| failed())?;
        Ok(capture)
    }
}
struct Input(Vec<u8>);
impl Drop for Input {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl OwnedChild {
    fn stop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.reaped = true;
        }
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.stop();
    }
}
fn nonblocking(stderr: &std::process::ChildStderr) -> AppResult<()> {
    // SAFETY: The descriptor is live and owned by stderr; F_GETFL has no third argument.
    let flags = unsafe { fcntl(stderr.as_raw_fd(), 3) };
    if flags < 0 {
        return Err(failed());
    }
    // SAFETY: F_SETFL receives the retrieved flags with only O_NONBLOCK added.
    if unsafe { fcntl(stderr.as_raw_fd(), 4, flags | O_NONBLOCK) } < 0 {
        return Err(failed());
    }
    Ok(())
}
fn stderr_empty(stderr: &mut std::process::ChildStderr, path: &Path) -> AppResult<()> {
    let mut byte = [0_u8; 1];
    for _ in 0..8 {
        match stderr.read(&mut byte) {
            Ok(0) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            _ => {
                return Err(
                    if fs::symlink_metadata(path).is_ok_and(|m| m.len() >= MAX_INPUT) {
                        budget()
                    } else {
                        failed()
                    },
                );
            }
        }
    }
    Err(failed()) // An interrupted read is never evidence of empty stderr.
}
fn source_bounds(width: u32, height: u32) -> AppResult<()> {
    if width == 0
        || height == 0
        || u64::from(width)
            .checked_mul(u64::from(height))
            .is_none_or(|area| area > MAX_SOURCE_PIXELS)
    {
        return Err(budget());
    }
    Ok(())
}
fn jpeg_dimensions(bytes: &[u8]) -> AppResult<(u32, u32)> {
    if !bytes.starts_with(&[0xff, 0xd8]) || !bytes.ends_with(&[0xff, 0xd9]) {
        return Err(encoding());
    }
    let mut cursor = 2;
    while cursor + 4 <= bytes.len() {
        if bytes[cursor] != 0xff {
            return Err(encoding());
        }
        while cursor < bytes.len() && bytes[cursor] == 0xff {
            cursor += 1;
        }
        let marker = *bytes.get(cursor).ok_or_else(encoding)?;
        cursor += 1;
        if matches!(marker, 0x00 | 0xd8 | 0xd9 | 0xda) {
            return Err(encoding());
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(cursor).ok_or_else(encoding)?,
            *bytes.get(cursor + 1).ok_or_else(encoding)?,
        ]));
        if length < 2
            || cursor
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
        {
            return Err(encoding());
        }
        if matches!(marker,0xc0..=0xc3|0xc5..=0xc7|0xc9..=0xcb|0xcd..=0xcf) {
            if length < 8 {
                return Err(encoding());
            }
            let height = u32::from(u16::from_be_bytes([bytes[cursor + 3], bytes[cursor + 4]]));
            let width = u32::from(u16::from_be_bytes([bytes[cursor + 5], bytes[cursor + 6]]));
            source_bounds(width, height)?;
            return Ok((width, height));
        }
        cursor += length;
    }
    Err(encoding())
}
fn stable_file(file: &File, path: &Path) -> AppResult<Metadata> {
    let descriptor = file.metadata().map_err(|_| failed())?;
    let named = fs::symlink_metadata(path).map_err(|_| failed())?;
    if !private(&descriptor, false) || !private(&named, false) || !same_file(&descriptor, &named) {
        return Err(failed());
    }
    if descriptor.len() > MAX_INPUT {
        return Err(budget());
    }
    Ok(descriptor)
}
fn read_frame(file: &mut File, path: &Path, width: u32, height: u32) -> AppResult<Input> {
    let before = stable_file(file, path)?;
    if before.len() == 0 {
        return Err(encoding());
    }
    let mut input = Input(Vec::with_capacity(before.len() as usize));
    Read::by_ref(file)
        .take(MAX_INPUT + 1)
        .read_to_end(&mut input.0)
        .map_err(|_| failed())?;
    let after = stable_file(file, path)?;
    if input.0.len() as u64 > MAX_INPUT {
        return Err(budget());
    }
    if input.0.len() as u64 != before.len()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(failed());
    }
    let (actual_width, actual_height) = jpeg_dimensions(&input.0)?;
    if actual_width > width || actual_height > height {
        return Err(budget());
    }
    Ok(input)
}
fn capture_with_program(
    program: &Path,
    base: &Path,
    window: u32,
    width: u32,
    height: u32,
    validate: &dyn Fn() -> AppResult<()>,
    normalize: &dyn Fn(&[u8]) -> AppResult<super::CapturedFrame>,
) -> AppResult<super::CapturedFrame> {
    let _single = IN_FLIGHT.try_lock().map_err(|_| failed())?;
    source_bounds(width, height)?;
    if window == 0 || !program.is_absolute() {
        return Err(failed());
    }
    validate()?;
    let temporary = TemporaryCapture::create(base)?;
    let result = capture_transaction(
        program, &temporary, window, width, height, validate, normalize,
    );
    // Cleanup failure never produces a live frame. An earlier capture/cancel
    // error remains its exact typed identity; unknown residue is not erased.
    let cleaned = temporary.cleanup();
    match (result, cleaned) {
        (Ok(frame), Ok(())) => Ok(frame),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(mut error), Err(_)) => {
            error.message.push_str("；临时预览缓存清理未完成");
            Err(error)
        }
    }
}
fn pending_file(path: &Path, directory: &File) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !private(&metadata, false)
                || metadata.dev() != directory.metadata().map_err(|_| failed())?.dev()
            {
                return Err(failed());
            }
            if metadata.len() > MAX_INPUT {
                return Err(budget());
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(failed()),
    }
}
fn capture_transaction(
    program: &Path,
    temporary: &TemporaryCapture,
    window: u32,
    width: u32,
    height: u32,
    validate: &dyn Fn() -> AppResult<()>,
    normalize: &dyn Fn(&[u8]) -> AppResult<super::CapturedFrame>,
) -> AppResult<super::CapturedFrame> {
    let frame_path = temporary.path.join(FRAME);
    // The Apple tool atomically replaces this file. Never read the old FD as
    // though it were the completed output; bind the replacement after exit.
    drop(create_relative(&temporary.directory, FRAME)?);
    let mut command = Command::new(program);
    command
        .args(["-x", "-o", "-l", &window.to_string(), "-t", "jpg"])
        .arg(&frame_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // SAFETY: The post-fork closure calls only async-signal-safe umask and the
    // Darwin setrlimit syscall with stack data. No allocation/locks/logging.
    unsafe {
        command.pre_exec(|| {
            umask(0o077);
            let limit = RLimit {
                current: MAX_INPUT,
                maximum: MAX_INPUT,
            };
            if setrlimit(1, &limit) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let deadline = Instant::now() + TIMEOUT;
    let mut child = OwnedChild {
        child: command.spawn().map_err(|_| failed())?,
        reaped: false,
    };
    let mut stderr = child.child.stderr.take().ok_or_else(failed)?;
    nonblocking(&stderr)?;
    let status: ExitStatus = loop {
        validate()?;
        temporary.validate_directory(false)?;
        pending_file(&frame_path, &temporary.directory)?;
        stderr_empty(&mut stderr, &frame_path)?;
        if let Some(status) = child.child.try_wait().map_err(|_| failed())? {
            child.reaped = true;
            break status;
        }
        if Instant::now() >= deadline {
            return Err(error("preview_capture_timeout"));
        }
        std::thread::sleep(POLL);
    };
    stderr_empty(&mut stderr, &frame_path)?;
    validate()?;
    temporary.validate_directory(true)?;
    if !status.success() {
        return Err(
            if fs::symlink_metadata(&frame_path).is_ok_and(|m| m.len() >= MAX_INPUT) {
                budget()
            } else {
                failed()
            },
        );
    }
    let mut file = open_relative(&temporary.directory, FRAME, 0)?;
    let metadata = file.metadata().map_err(|_| failed())?;
    if metadata.dev() != temporary.directory.metadata().map_err(|_| failed())?.dev() {
        return Err(failed());
    }
    let input = read_frame(&mut file, &frame_path, width, height)?;
    temporary.validate_directory(true)?;
    validate()?;
    let mut output = normalize(&input.0)?;
    if output.bytes.is_empty()
        || output.bytes.len() > MAX_OUTPUT
        || output.width == 0
        || output.width > 960
        || output.height == 0
        || output.height > 540
        || jpeg_dimensions(&output.bytes)? != (output.width, output.height)
    {
        return Err(encoding());
    }
    validate()?;
    output.captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| failed())?
        .as_millis()
        .try_into()
        .map_err(|_| failed())?;
    Ok(output)
}
pub(super) fn capture_window(
    window_id: u32,
    source_width: u32,
    source_height: u32,
    validate: &dyn Fn() -> AppResult<()>,
) -> AppResult<super::CapturedFrame> {
    capture_with_program(
        Path::new(PROGRAM),
        &std::env::temp_dir(),
        window_id,
        source_width,
        source_height,
        validate,
        &super::normalize_preview_image,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::MutexGuard;
    static SERIAL: Mutex<()> = Mutex::new(());
    fn serial() -> MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }
    struct Environment {
        base: PathBuf,
        program: PathBuf,
    }
    impl Environment {
        fn new() -> Self {
            let base = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../.build/window-preview-sakura-20261023/cli-fallback/tests")
                .join(random_name().unwrap());
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&base)
                .unwrap();
            let base = base.canonicalize().unwrap();
            Self {
                program: base.join("fake-screencapture"),
                base,
            }
        }
        fn script(&self, body: &str) {
            fs::write(
                &self.program,
                format!("#!/bin/sh\nfor output; do :; done\n{body}\n"),
            )
            .unwrap();
            fs::set_permissions(&self.program, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fn capture(&self) -> AppResult<super::super::CapturedFrame> {
            self.with(&|| Ok(()), &normalize)
        }
        fn with(
            &self,
            validate: &dyn Fn() -> AppResult<()>,
            normalize: &dyn Fn(&[u8]) -> AppResult<super::super::CapturedFrame>,
        ) -> AppResult<super::super::CapturedFrame> {
            capture_with_program(&self.program, &self.base, 42, 32, 24, validate, normalize)
        }
        fn empty(&self) {
            let root = self.base.join(ROOT_NAME);
            assert!(
                !root.exists() || fs::read_dir(root).unwrap().next().is_none(),
                "private frame or sibling survived cleanup"
            );
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    // Deliberate metadata-only JPEG test doubles. Production normalization uses
    // the real ImageIO codec; these fixtures prove CLI/resource guards only.
    fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut value = vec![0xff, 0xd8, 0xff, 0xc0, 0, 11, 8];
        value.extend(height.to_be_bytes());
        value.extend(width.to_be_bytes());
        value.extend([1, 1, 0x11, 0, 0xff, 0xd9]);
        value
    }
    fn write_jpeg(width: u16, height: u16, target: &str) -> String {
        let octal: String = jpeg(width, height)
            .iter()
            .map(|v| format!("\\{v:03o}"))
            .collect();
        format!("printf '{octal}' > {target}")
    }
    fn normalize(bytes: &[u8]) -> AppResult<super::super::CapturedFrame> {
        let (width, height) = jpeg_dimensions(bytes)?;
        Ok(super::super::CapturedFrame {
            bytes: bytes.to_vec(),
            width,
            height,
            captured_at_ms: 0,
        })
    }
    fn code(result: AppResult<super::super::CapturedFrame>) -> String {
        result.err().expect("capture unexpectedly succeeded").code
    }
    fn quote(path: &Path) -> String {
        format!("'{}'", path.to_str().unwrap().replace('\'', "'\"'\"'"))
    }
    fn assert_reaped(path: &Path) {
        let pid = fs::read_to_string(path).unwrap().trim().parse().unwrap();
        assert!(dead_owner(pid), "owned child was not reaped");
    }

    #[test]
    fn exact_cli_arguments_and_private_success() {
        let _guard = serial();
        let env = Environment::new();
        env.script(&format!("[ \"$#\" = 7 ] && [ \"$1\" = -x ] && [ \"$2\" = -o ] && [ \"$3\" = -l ] && [ \"$4\" = 42 ] && [ \"$5\" = -t ] && [ \"$6\" = jpg ] || exit 9\n{}",write_jpeg(16,8,"\"$output\"")));
        let calls = Cell::new(0);
        let result = env
            .with(
                &|| {
                    calls.set(calls.get() + 1);
                    Ok(())
                },
                &normalize,
            )
            .unwrap();
        assert_eq!((result.width, result.height), (16, 8));
        assert!(result.captured_at_ms > 0);
        assert!(calls.get() >= 5);
        env.empty();
    }
    #[test]
    fn apple_style_atomic_replace_binds_new_private_inode() {
        let _guard = serial();
        let env = Environment::new();
        env.script(&format!(
            "{}\n/bin/mv -f \"$output.tmp\" \"$output\"",
            write_jpeg(16, 8, "\"$output.tmp\"")
        ));
        assert_eq!(env.capture().unwrap().bytes, jpeg(16, 8));
        env.empty();
    }
    #[test]
    fn exit_zero_empty_and_nonzero_valid_image_never_succeed() {
        let _guard = serial();
        let env = Environment::new();
        env.script("exit 0");
        assert_eq!(code(env.capture()), "preview_encoding_failed");
        env.empty();
        env.script(&format!("{}\nexit 7", write_jpeg(16, 8, "\"$output\"")));
        assert_eq!(code(env.capture()), "preview_capture_failed");
        env.empty();
    }
    #[test]
    fn stderr_byte_rejects_even_exit_zero_with_valid_image() {
        let _guard = serial();
        let env = Environment::new();
        env.script(&format!(
            "{}\nprintf 'private test error' >&2\nexit 0",
            write_jpeg(16, 8, "\"$output\"")
        ));
        let failure = env.capture().err().unwrap();
        assert_eq!(failure.code, "preview_capture_failed");
        assert!(!failure.message.contains("private test error"));
        env.empty();
    }
    #[test]
    fn child_kernel_file_limit_stops_fast_oversized_writer() {
        let _guard = serial();
        let env = Environment::new();
        env.script("exec /bin/dd if=/dev/zero of=\"$output\" bs=1048576 count=4 2>/dev/null");
        let maximum = Cell::new(0);
        let result = env.with(
            &|| {
                if let Ok(entries) = fs::read_dir(env.base.join(ROOT_NAME)) {
                    for entry in entries.flatten() {
                        if let Ok(m) = fs::metadata(entry.path().join(FRAME)) {
                            maximum.set(maximum.get().max(m.len()));
                        }
                    }
                }
                Ok(())
            },
            &normalize,
        );
        assert_eq!(code(result), "preview_budget_exceeded");
        assert!(maximum.get() <= MAX_INPUT);
        env.empty();
    }
    #[test]
    fn timeout_kills_only_owned_child_reaps_and_cleans() {
        let _guard = serial();
        let env = Environment::new();
        let pid = env.base.join("child-pid");
        env.script(&format!("echo $$ > {}\nexec /bin/sleep 8", quote(&pid)));
        let started = Instant::now();
        assert_eq!(code(env.capture()), "preview_capture_timeout");
        assert!(started.elapsed() >= TIMEOUT);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_reaped(&pid);
        env.empty();
    }
    #[test]
    fn cancellation_preserves_typed_error_and_reaps_owned_child() {
        let _guard = serial();
        let env = Environment::new();
        let pid = env.base.join("child-pid");
        env.script(&format!("echo $$ > {}\nexec /bin/sleep 8", quote(&pid)));
        let calls = Cell::new(0);
        let result = env.with(
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() > 4 && pid.exists() {
                    Err(error("preview_permission_revoked"))
                } else {
                    Ok(())
                }
            },
            &normalize,
        );
        assert_eq!(code(result), "preview_permission_revoked");
        assert_reaped(&pid);
        env.empty();
    }
    #[test]
    fn permission_or_identity_rejection_before_spawn_does_not_launch() {
        let _guard = serial();
        let env = Environment::new();
        let marker = env.base.join("must-not-launch");
        env.script(&format!("echo bad > {}", quote(&marker)));
        for reason in ["preview_permission_denied", "preview_target_changed"] {
            assert_eq!(code(env.with(&|| Err(error(reason)), &normalize)), reason);
            assert!(!marker.exists());
            env.empty();
        }
    }
    #[test]
    fn post_codec_identity_change_rejects_frame() {
        let _guard = serial();
        let env = Environment::new();
        env.script(&write_jpeg(16, 8, "\"$output\""));
        let normalized = Cell::new(false);
        let result = env.with(
            &|| {
                if normalized.get() {
                    Err(error("preview_target_changed"))
                } else {
                    Ok(())
                }
            },
            &|bytes| {
                normalized.set(true);
                normalize(bytes)
            },
        );
        assert_eq!(code(result), "preview_target_changed");
        env.empty();
    }
    #[test]
    fn input_jpeg_type_truncation_dimensions_and_pixel_budget_are_checked() {
        let _guard = serial();
        let env = Environment::new();
        for bytes in [
            b"not a jpeg".to_vec(),
            vec![0xff, 0xd8, 0xff, 0xe0, 0xff, 0xff, 0xff, 0xd9],
            jpeg(0, 8),
            jpeg(4096, 4096),
            jpeg(33, 8),
        ] {
            let text: String = bytes.iter().map(|v| format!("\\{v:03o}")).collect();
            env.script(&format!("printf '{text}' > \"$output\""));
            assert!(env.capture().is_err());
            env.empty();
        }
    }
    #[test]
    fn source_budget_and_relative_program_reject_before_child() {
        let _guard = serial();
        let env = Environment::new();
        env.script("exit 0");
        for (width, height) in [(0, 1), (1, 0), (4096, 4096), (u32::MAX, u32::MAX)] {
            assert_eq!(
                code(capture_with_program(
                    &env.program,
                    &env.base,
                    42,
                    width,
                    height,
                    &|| Ok(()),
                    &normalize
                )),
                "preview_budget_exceeded"
            );
            env.empty();
        }
        assert_eq!(
            code(capture_with_program(
                Path::new("relative-tool"),
                &env.base,
                42,
                32,
                24,
                &|| Ok(()),
                &normalize
            )),
            "preview_capture_failed"
        );
        env.empty();
    }
    #[test]
    fn normalization_rechecks_output_contract_and_codec_failure() {
        let _guard = serial();
        let env = Environment::new();
        env.script(&write_jpeg(16, 8, "\"$output\""));
        assert_eq!(
            code(env.with(&|| Ok(()), &|_| Err(encoding()))),
            "preview_encoding_failed"
        );
        env.empty();
        for (width, height, bytes) in [
            (961, 8, jpeg(961, 8)),
            (16, 541, jpeg(16, 541)),
            (16, 8, vec![0; MAX_OUTPUT + 1]),
            (15, 8, jpeg(16, 8)),
        ] {
            assert_eq!(
                code(env.with(&|| Ok(()), &|_| Ok(super::super::CapturedFrame {
                    bytes: bytes.clone(),
                    width,
                    height,
                    captured_at_ms: 0
                }))),
                "preview_encoding_failed"
            );
            env.empty();
        }
    }
    #[test]
    fn symlink_hardlink_and_public_mode_output_are_rejected_without_following() {
        let _guard = serial();
        let env = Environment::new();
        let victim = env.base.join("outside-payload");
        fs::write(&victim, b"untouched").unwrap();
        for command in [
            format!(
                "/bin/rm -f \"$output\"\n/bin/ln -s {} \"$output\"",
                quote(&victim)
            ),
            format!(
                "/bin/rm -f \"$output\"\n/bin/ln {} \"$output\"",
                quote(&victim)
            ),
            format!(
                "{}\n/bin/chmod 644 \"$output\"",
                write_jpeg(16, 8, "\"$output\"")
            ),
        ] {
            env.script(&command);
            assert_eq!(code(env.capture()), "preview_capture_failed");
            assert_eq!(fs::read(&victim).unwrap(), b"untouched");
            env.empty();
        }
    }
    #[test]
    fn extra_private_atomic_sibling_rejects_output_but_is_cleaned() {
        let _guard = serial();
        let env = Environment::new();
        env.script(&format!(
            "{}\n{}",
            write_jpeg(16, 8, "\"$output\""),
            write_jpeg(16, 8, "\"$output.atomic-pending\"")
        ));
        assert_eq!(code(env.capture()), "preview_capture_failed");
        env.empty();
    }
    #[test]
    fn cancellation_cleans_sibling_left_before_atomic_rename() {
        let _guard = serial();
        let env = Environment::new();
        let pid = env.base.join("child-pid");
        env.script(&format!(
            "echo $$ > {}\n{}\nexec /bin/sleep 8",
            quote(&pid),
            write_jpeg(16, 8, "\"$output.atomic-pending\"")
        ));
        let calls = Cell::new(0);
        let result = env.with(
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() > 4
                    && pid.exists()
                    && fs::read_dir(env.base.join(ROOT_NAME)).is_ok_and(|entries| {
                        entries
                            .flatten()
                            .any(|entry| entry.path().join("frame.jpg.atomic-pending").exists())
                    })
                {
                    Err(error("preview_target_changed"))
                } else {
                    Ok(())
                }
            },
            &normalize,
        );
        assert_eq!(code(result), "preview_target_changed");
        assert_reaped(&pid);
        env.empty();
    }
    #[test]
    fn exclusive_inflight_and_spawn_failure_are_bounded_and_clean() {
        let _guard = serial();
        let env = Environment::new();
        env.script("exit 0");
        {
            let _busy = IN_FLIGHT.lock().unwrap();
            assert_eq!(code(env.capture()), "preview_capture_failed");
            env.empty();
        }
        assert_eq!(
            code(capture_with_program(
                &env.base.join("missing-tool"),
                &env.base,
                42,
                32,
                24,
                &|| Ok(()),
                &normalize
            )),
            "preview_capture_failed"
        );
        env.empty();
    }
    #[test]
    fn private_root_symlink_never_traverses_foreign_scope() {
        let _guard = serial();
        let env = Environment::new();
        let foreign = env.base.join("foreign");
        fs::create_dir(&foreign).unwrap();
        fs::write(foreign.join("keep"), b"keep").unwrap();
        std::os::unix::fs::symlink(&foreign, env.base.join(ROOT_NAME)).unwrap();
        env.script("exit 0");
        assert_eq!(code(env.capture()), "preview_capture_failed");
        assert_eq!(fs::read(foreign.join("keep")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&foreign).unwrap().count(), 1);
    }
    #[test]
    fn fifo_output_and_abandoned_owner_never_block_open() {
        let _guard = serial();
        let env = Environment::new();
        env.script("/bin/rm -f \"$output\"\n/usr/bin/mkfifo \"$output\"");
        let started = Instant::now();
        assert_eq!(code(env.capture()), "preview_capture_failed");
        assert!(started.elapsed() < Duration::from_secs(2));
        env.empty();
        let root = env.base.join(ROOT_NAME);
        let root_fd = open_directory(&root).unwrap();
        let mut process = Command::new("/usr/bin/true").spawn().unwrap();
        let pid = process.id();
        process.wait().unwrap();
        assert!(dead_owner(pid));
        let stale = root.join(format!("{pid}-{}", "d".repeat(32)));
        DirBuilder::new().mode(0o700).create(&stale).unwrap();
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(stale.join(OWNER))
                .status()
                .unwrap()
                .success()
        );
        let started = Instant::now();
        clean_abandoned(&root, &root_fd);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(stale.join(OWNER).exists());
    }
    #[test]
    fn abandoned_cleanup_requires_owned_marker_and_dead_process() {
        let _guard = serial();
        let env = Environment::new();
        let root = env.base.join(ROOT_NAME);
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        let root_fd = open_directory(&root).unwrap();
        let mut process = Command::new("/usr/bin/true").spawn().unwrap();
        let dead = process.id();
        process.wait().unwrap();
        assert!(dead_owner(dead));
        let entries = [
            (dead, "a".repeat(32), true),
            (std::process::id(), "b".repeat(32), true),
            (dead, "c".repeat(32), false),
        ];
        for (pid, nonce, owned) in &entries {
            let path = root.join(format!("{pid}-{nonce}"));
            DirBuilder::new().mode(0o700).create(&path).unwrap();
            let mut owner = create_file(&path.join(OWNER)).unwrap();
            if *owned {
                write!(
                    owner,
                    "{}{}\n{}\n",
                    String::from_utf8_lossy(MARKER),
                    pid,
                    nonce
                )
                .unwrap();
            } else {
                owner.write_all(b"foreign").unwrap();
            }
            create_file(&path.join("frame.jpg.atomic-pending"))
                .unwrap()
                .write_all(b"temporary")
                .unwrap();
        }
        clean_abandoned(&root, &root_fd);
        assert!(
            !root
                .join(format!("{}-{}", entries[0].0, entries[0].1))
                .exists()
        );
        assert!(
            root.join(format!("{}-{}", entries[1].0, entries[1].1))
                .exists()
        );
        assert!(
            root.join(format!("{}-{}", entries[2].0, entries[2].1))
                .exists()
        );
    }
}
