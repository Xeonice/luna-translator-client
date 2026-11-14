//! Validate saved paths against actual Bottle drive mappings, then launch without a shell.
use app_core::{contracts::*, domain::library::normalize_windows_path};
use hook_client::{LaunchSettings, Target};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "launch", message)
}
pub(crate) struct LaunchPlan {
    pub settings: LaunchSettings,
    executable: String,
    workdir: String,
    arguments: Vec<String>,
    pub target_keys: Vec<String>,
}
impl LaunchPlan {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.settings.wine_path);
        command.args([
            "--bottle",
            &self.settings.bottle,
            "--no-update",
            "--no-gui",
            "--no-wait",
            "--no-convert",
            "--workdir",
            &self.workdir,
            "--cx-app",
            &self.executable,
            "--",
        ]);
        command.args(&self.arguments);
        command
    }
    pub fn already_running(&self, targets: &[Target]) -> AppResult<bool> {
        let mut found = false;
        for target in targets {
            let key = target
                .full_image_path
                .as_deref()
                .and_then(|path| normalize_windows_path(path).ok().map(|(_, key)| key));
            let exact_path = key
                .as_ref()
                .is_some_and(|key| self.target_keys.contains(key));
            let possible_name = key.is_none()
                && self.target_keys.iter().any(|key| {
                    key.rsplit('\\')
                        .next()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&target.exe))
                });
            if !exact_path && !possible_name {
                continue;
            }
            if !exact_path
                || target.pid == 0
                || !target.errors.is_empty()
                || !target
                    .creation_filetime
                    .as_deref()
                    .is_some_and(|value| value.parse::<u64>().is_ok_and(|value| value != 0))
                || !matches!(target.architecture.as_deref(), Some("x86" | "x64"))
            {
                return Err(error(
                    "launch_identity_incomplete",
                    "发现可能已运行的同一程序，但身份尚无法核对；未启动新实例，请刷新运行目标",
                ));
            }
            found = true;
        }
        Ok(found)
    }
}

pub(crate) fn prepare(
    target: &GameTarget,
    environments: Vec<(LaunchSettings, String)>,
) -> AppResult<LaunchPlan> {
    if target.environment_kind != "crossover"
        || !matches!(
            target.verification_state.as_str(),
            "unresolved" | "verified"
        )
    {
        return Err(error(
            "unverified_launch_target",
            "请先核对游戏的有效运行环境与启动路径",
        ));
    }
    let (settings, key) = environments
        .into_iter()
        .find(|(_, key)| key == &target.environment_key)
        .ok_or_else(|| {
            error(
                "launch_environment_missing",
                "已保存的CrossOver容器不可用，请重新核对",
            )
        })?;
    let locator = target
        .bottle_locator
        .as_deref()
        .ok_or_else(|| error("launch_environment_missing", "游戏资料缺少真实Bottle定位"))?;
    let bottle = Path::new(locator)
        .canonicalize()
        .map_err(|_| error("launch_environment_missing", "已保存的Bottle路径失效"))?;
    if key != format!("crossover:{}", bottle.display())
        || !bottle.join("cxbottle.conf").is_file()
        || !Path::new(&settings.wine_path).is_file()
    {
        return Err(error(
            "launch_environment_mismatch",
            "Bottle或CrossOver路径与已核验环境不一致",
        ));
    }
    if settings.bottle.is_empty()
        || settings.bottle.contains('\0')
        || target.arguments.len() > 128
        || target.arguments.iter().map(String::len).sum::<usize>() > 65_536
        || target.arguments.iter().any(|value| value.contains('\0'))
    {
        return Err(error(
            "invalid_launch_arguments",
            "启动参数无效或超过长度限制",
        ));
    }
    let drives = drive_mappings(&bottle)?;
    let launch = target
        .launch_path
        .as_deref()
        .ok_or_else(|| error("launch_path_missing", "请先选择启动程序"))?;
    let executable = resolve(launch, &drives)?;
    super::executables::pe_architecture(&executable)?;
    let (executable, launch_key) = windows_path(&executable, &drives)?;
    let mut target_keys = vec![launch_key];
    if let Some(path) = &target.canonical_exe_path {
        let (_, key) = normalize_windows_path(path)?;
        if target.target_path_key.as_deref() != Some(&key) {
            return Err(error(
                "launch_target_mismatch",
                "已保存的取文目标身份不一致，请重新核对",
            ));
        }
        let native = resolve(path, &drives)?;
        let architecture = super::executables::pe_architecture(&native)?;
        if target
            .architecture
            .as_deref()
            .is_some_and(|value| value != architecture)
        {
            return Err(error(
                "launch_target_changed",
                "取文程序位数已变化，请重新核对",
            ));
        }
        target_keys.push(key);
    }
    let workdir = match target.working_directory.as_deref() {
        Some(path) => resolve_workdir(path, &drives)?,
        None => resolve(launch, &drives)?
            .parent()
            .ok_or_else(|| error("launch_workdir_invalid", "无法确定启动程序目录"))?
            .to_owned(),
    };
    if !workdir.is_dir() {
        return Err(error("launch_workdir_invalid", "工作目录不存在或不是目录"));
    }
    Ok(LaunchPlan {
        settings,
        executable,
        workdir: windows_path(&workdir, &drives)?.0,
        arguments: target.arguments.clone(),
        target_keys,
    })
}

pub(crate) fn drive_mappings(bottle: &Path) -> AppResult<Vec<(char, PathBuf)>> {
    collect_drive_mappings(bottle, None)
}

/// Resolve one Windows target using only the drive that its normalized absolute
/// path requests. An unrelated unavailable volume grants no authority and does
/// not prevent verifying this target; the requested mapping still must resolve.
pub(crate) fn resolve_windows_target(bottle: &Path, value: &str) -> AppResult<PathBuf> {
    if value.len() > 32_768 || value.chars().any(char::is_control) {
        return Err(error("invalid_launch_path", "启动路径包含无效字符或过长"));
    }
    let (windows, _) = normalize_windows_path(value)?;
    if windows.starts_with("\\\\") {
        return Err(error(
            "launch_drive_missing",
            "请将网络程序映射到Bottle中的驱动器后再选择",
        ));
    }
    let letter = windows.as_bytes()[0] as char;
    let drives = collect_drive_mappings(bottle, Some(letter))?;
    resolve(&windows, &drives)
}

fn collect_drive_mappings(
    bottle: &Path,
    requested: Option<char>,
) -> AppResult<Vec<(char, PathBuf)>> {
    let entries = std::fs::read_dir(bottle.join("dosdevices"))
        .map_err(|_| error("launch_drive_missing", "无法核验Bottle的驱动器映射"))?;
    let mut drives = Vec::new();
    for entry in entries.take(256) {
        let entry = entry.map_err(|_| error("launch_drive_missing", "无法读取驱动器映射"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let bytes = name.as_bytes();
        if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            let letter = (bytes[0] as char).to_ascii_uppercase();
            if requested.is_some_and(|requested| requested != letter) {
                continue;
            }
            if requested.is_some() && drives.iter().any(|(existing, _)| *existing == letter) {
                return Err(error("launch_drive_missing", "所选驱动器存在重复映射"));
            }
            let path = entry
                .path()
                .canonicalize()
                .map_err(|_| error("launch_drive_missing", "Bottle含有失效的驱动器映射"))?;
            if requested.is_some() && !path.is_dir() {
                return Err(error("launch_drive_missing", "所选驱动器不是有效目录映射"));
            }
            if path.is_dir() {
                drives.push((letter, path));
            }
        }
    }
    // Prefer a real, more specific drive mapping over an existing root mapping.
    drives.sort_by(|a, b| {
        b.1.components()
            .count()
            .cmp(&a.1.components().count())
            .then(a.0.cmp(&b.0))
    });
    Ok(drives)
}
fn resolve_workdir(value: &str, drives: &[(char, PathBuf)]) -> AppResult<PathBuf> {
    let bytes = value.as_bytes();
    if bytes.len() == 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        return drives
            .iter()
            .find(|(drive, _)| *drive == (bytes[0] as char).to_ascii_uppercase())
            .map(|(_, path)| path.clone())
            .ok_or_else(|| error("launch_drive_missing", "工作目录的驱动器没有有效映射"));
    }
    resolve(value, drives)
}

pub(crate) fn resolve(value: &str, drives: &[(char, PathBuf)]) -> AppResult<PathBuf> {
    if value.len() > 32_768 || value.chars().any(char::is_control) {
        return Err(error("invalid_launch_path", "启动路径包含无效字符或过长"));
    }
    let native = if value.starts_with('/') {
        PathBuf::from(value)
    } else {
        let (windows, _) = normalize_windows_path(value)?;
        if windows.starts_with("\\\\") {
            return Err(error(
                "launch_drive_missing",
                "请将网络程序映射到Bottle中的驱动器后再选择",
            ));
        }
        let letter = windows.as_bytes()[0] as char;
        let root = drives
            .iter()
            .find(|(drive, _)| *drive == letter)
            .map(|(_, path)| path)
            .ok_or_else(|| error("launch_drive_missing", "启动路径的驱动器没有有效映射"))?;
        root.join(windows[3..].replace('\\', "/"))
    };
    native
        .canonicalize()
        .map_err(|_| error("launch_path_missing", "已保存的启动路径或工作目录已失效"))
}
fn windows_path(native: &Path, drives: &[(char, PathBuf)]) -> AppResult<(String, String)> {
    for (letter, root) in drives {
        if let Ok(relative) = native.strip_prefix(root) {
            let path = format!(
                "{letter}:\\{}",
                relative.to_string_lossy().replace('/', "\\")
            );
            // A drive root is a valid working directory, though not a valid EXE.
            return if relative.as_os_str().is_empty() {
                Ok((path.clone(), path.to_ascii_lowercase()))
            } else {
                normalize_windows_path(&path)
            };
        }
    }
    Err(error(
        "launch_drive_missing",
        "文件不在Bottle的任何已核验驱动器映射中",
    ))
}

/// Own the pending-launch flock across discovery, reservation and launcher exit.
/// Closing alone is insufficient while a concurrent fork retains the descriptor.
pub(crate) struct PendingLaunchLease {
    file: std::fs::File,
    owner_pid: u32,
}
impl PendingLaunchLease {
    pub(crate) fn try_lock(file: std::fs::File) -> Result<Self, std::fs::TryLockError> {
        file.try_lock()?;
        Ok(Self {
            file,
            owner_pid: std::process::id(),
        })
    }
}
impl Drop for PendingLaunchLease {
    fn drop(&mut self) {
        // A forked child shares the flock, but cannot release its parent's lease.
        if self.owner_pid == std::process::id() {
            let _ = self.file.unlock();
        }
    }
}

pub(crate) fn reserve_launch(
    lease: &mut PendingLaunchLease,
    now: std::time::SystemTime,
) -> AppResult<()> {
    use std::io::{Read, Seek, Write};
    fn save(file: &mut std::fs::File, now: u128) -> AppResult<()> {
        file.rewind()
            .and_then(|_| file.set_len(0))
            .and_then(|_| file.write_all(now.to_string().as_bytes()))
            .and_then(|_| file.sync_data())
            .map_err(|_| error("target_lock", "无法保存启动请求记录"))
    }
    let file = &mut lease.file;
    let now = now
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| error("launch_clock", "无法核对启动请求时间"))?
        .as_millis();
    file.rewind()
        .map_err(|_| error("target_lock", "无法读取启动请求记录"))?;
    let mut value = String::new();
    file.take(40)
        .read_to_string(&mut value)
        .map_err(|_| error("target_lock", "无法读取启动请求记录"))?;
    if !value.is_empty() {
        let previous = value
            .parse::<u128>()
            .map_err(|_| error("target_lock", "启动请求记录无效，请核对后重试"))?;
        if previous > now {
            save(file, now)?;
            return Err(error(
                "game_launch_pending",
                "系统时间已变化，请等待30秒并刷新运行目标",
            ));
        }
        if now - previous < 30_000 {
            return Err(error(
                "game_launch_pending",
                "刚发送过启动请求，请等待并刷新运行目标",
            ));
        }
    }
    save(file, now)
}

pub(crate) fn run(mut command: Command, timeout: Duration) -> AppResult<()> {
    // No pipe readers or inherited terminal; game descendants cannot retain an IPC pipe.
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| error("game_launch_failed", "CrossOver未能接受启动请求"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                return Err(error(
                    "game_launch_failed",
                    "CrossOver启动请求失败，请核对路径和容器",
                ));
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(15))
            }
            Ok(None) => {
                // This is the owned --no-wait launcher only. Never kill a process group,
                // wineserver, Bottle, Steam, or a discovered Windows game process.
                let _ = child.kill();
                let _ = child.wait();
                return Err(error(
                    "game_launch_timeout",
                    "启动请求超时；游戏可能仍已启动，请先刷新运行目标",
                ));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error(
                    "game_launch_failed",
                    "无法核对CrossOver启动请求的退出状态",
                ));
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        target: GameTarget,
        environments: Vec<(LaunchSettings, String)>,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("luna-launch-{}", uuid::Uuid::new_v4()));
            let bottle = root.join("Real Bottle");
            std::fs::create_dir_all(bottle.join("dosdevices")).unwrap();
            std::fs::create_dir_all(bottle.join("drive_c/Games/日本語 Name")).unwrap();
            std::fs::write(bottle.join("cxbottle.conf"), "[Bottle]\n").unwrap();
            std::os::unix::fs::symlink("../drive_c", bottle.join("dosdevices/c:")).unwrap();
            let executable = bottle.join("drive_c/Games/日本語 Name/Game.exe");
            let mut pe = vec![0u8; 88];
            pe[..2].copy_from_slice(b"MZ");
            pe[60..64].copy_from_slice(&64u32.to_le_bytes());
            pe[64..68].copy_from_slice(b"PE\0\0");
            pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
            std::fs::write(&executable, pe).unwrap();
            let bottle = bottle.canonicalize().unwrap();
            let key = format!("crossover:{}", bottle.display());
            let target = GameTarget {
                environment_kind: "crossover".into(),
                environment_key: key.clone(),
                bottle_locator: Some(bottle.to_string_lossy().into()),
                launch_path: Some(executable.to_string_lossy().into()),
                canonical_exe_path: Some("C:\\Games\\日本語 Name\\Game.exe".into()),
                target_path_key: Some("c:\\games\\日本語 name\\game.exe".into()),
                working_directory: None,
                arguments: vec![
                    "--bottle".into(),
                    "$(touch /never-execute)".into(),
                    "a b".into(),
                    "\"quoted\"".into(),
                    "".into(),
                    "日本語".into(),
                ],
                architecture: Some("x64".into()),
                verification_state: "verified".into(),
            };
            Self {
                root,
                target,
                environments: vec![(
                    LaunchSettings {
                        wine_path: std::env::current_exe().unwrap().to_string_lossy().into(),
                        bottle: "Real Bottle".into(),
                    },
                    key,
                )],
            }
        }
        fn prepare(&self) -> AppResult<LaunchPlan> {
            prepare(&self.target, self.environments.clone())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn launch_argv_preserves_bottle_cwd_argument_order_and_shell_characters() {
        let fixture = Fixture::new();
        let plan = fixture.prepare().unwrap();
        let command = plan.command();
        let argv: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        let mut expected = [
            "--bottle",
            "Real Bottle",
            "--no-update",
            "--no-gui",
            "--no-wait",
            "--no-convert",
            "--workdir",
            "C:\\Games\\日本語 Name",
            "--cx-app",
            "C:\\Games\\日本語 Name\\Game.exe",
            "--",
        ]
        .map(String::from)
        .to_vec();
        expected.extend(fixture.target.arguments.clone());
        assert_eq!(argv, expected);
        assert_eq!(command.get_program(), &*fixture.environments[0].0.wine_path);
        assert!(command.get_current_dir().is_none());
    }
    #[test]
    fn actual_drive_mappings_resolve_windows_paths_and_do_not_guess_an_external_drive() {
        let mut fixture = Fixture::new();
        fixture.target.launch_path = fixture.target.canonical_exe_path.clone();
        fixture.target.working_directory = Some("C:\\Games".into());
        let plan = fixture.prepare().unwrap();
        assert_eq!(plan.workdir, "C:\\Games");
        fixture.target.working_directory = Some(fixture.root.to_string_lossy().into());
        assert_eq!(
            fixture.prepare().err().unwrap().code,
            "launch_drive_missing"
        );
        for root in ["C:\\", "c:/"] {
            fixture.target.working_directory = Some(root.into());
            assert_eq!(fixture.prepare().unwrap().workdir, "C:\\");
        }
        let bottle = PathBuf::from(fixture.target.bottle_locator.as_ref().unwrap());
        std::os::unix::fs::symlink(&fixture.root, bottle.join("dosdevices/y:")).unwrap();
        fixture.target.working_directory = Some(fixture.root.to_string_lossy().into());
        assert_eq!(fixture.prepare().unwrap().workdir, "Y:\\");
        fixture.target.working_directory = Some("Q:\\Missing".into());
        assert_eq!(
            fixture.prepare().err().unwrap().code,
            "launch_drive_missing"
        );
        fixture.target.working_directory = Some("\\\\server\\share".into());
        assert_eq!(
            fixture.prepare().err().unwrap().code,
            "launch_drive_missing"
        );
    }
    #[test]
    fn requested_windows_target_ignores_only_an_unrequested_unavailable_drive() {
        let fixture = Fixture::new();
        let bottle = Path::new(fixture.target.bottle_locator.as_deref().unwrap());
        std::os::unix::fs::symlink(fixture.root.join("unmounted"), bottle.join("dosdevices/d:"))
            .unwrap();
        let target = fixture.target.canonical_exe_path.as_deref().unwrap();
        assert_eq!(
            resolve_windows_target(bottle, target).unwrap(),
            bottle
                .join("drive_c/Games/日本語 Name/Game.exe")
                .canonicalize()
                .unwrap()
        );
        // Callers that require the complete inventory retain their strict policy.
        assert_eq!(
            drive_mappings(bottle).unwrap_err().code,
            "launch_drive_missing"
        );
        assert_eq!(
            resolve_windows_target(bottle, "D:\\Game.exe")
                .unwrap_err()
                .code,
            "launch_drive_missing"
        );
    }
    #[test]
    fn requested_windows_target_rejects_missing_unusable_and_absent_target_paths() {
        let fixture = Fixture::new();
        let bottle = Path::new(fixture.target.bottle_locator.as_deref().unwrap());
        assert_eq!(
            resolve_windows_target(bottle, "Q:\\Game.exe")
                .unwrap_err()
                .code,
            "launch_drive_missing"
        );
        assert_eq!(
            resolve_windows_target(bottle, "C:\\Missing.exe")
                .unwrap_err()
                .code,
            "launch_path_missing"
        );
        let mapping = bottle.join("dosdevices/c:");
        std::fs::remove_file(&mapping).unwrap();
        std::os::unix::fs::symlink(fixture.root.join("unmounted"), &mapping).unwrap();
        assert_eq!(
            resolve_windows_target(bottle, "C:\\Game.exe")
                .unwrap_err()
                .code,
            "launch_drive_missing"
        );
        std::fs::remove_file(&mapping).unwrap();
        let regular = fixture.root.join("not-a-drive");
        std::fs::write(&regular, b"not a directory").unwrap();
        std::os::unix::fs::symlink(regular, &mapping).unwrap();
        assert_eq!(
            resolve_windows_target(bottle, "C:\\Game.exe")
                .unwrap_err()
                .code,
            "launch_drive_missing"
        );
    }
    #[test]
    fn requested_windows_target_uses_existing_absolute_path_normalization() {
        let fixture = Fixture::new();
        let bottle = Path::new(fixture.target.bottle_locator.as_deref().unwrap());
        for invalid in [
            "Game.exe",
            "C:Game.exe",
            "/tmp/Game.exe",
            "C:\\..\\Game.exe",
            "C:\\Game.exe:stream",
            "C:\\Game.exe\0",
        ] {
            assert!(
                resolve_windows_target(bottle, invalid).is_err(),
                "{invalid:?}"
            );
        }
        assert_eq!(
            resolve_windows_target(bottle, "\\\\server\\share\\Game.exe")
                .unwrap_err()
                .code,
            "launch_drive_missing"
        );
        assert_eq!(
            resolve_windows_target(bottle, "c:/Games/日本語 Name/./Game.exe").unwrap(),
            resolve_windows_target(
                bottle,
                fixture.target.canonical_exe_path.as_deref().unwrap()
            )
            .unwrap()
        );
    }
    #[test]
    fn stale_missing_or_inconsistent_saved_records_are_rejected_before_spawn() {
        let fixture = Fixture::new();
        let mut target = fixture.target.clone();
        for state in ["stale", "invalid", "unexpected"] {
            target.verification_state = state.into();
            assert_eq!(
                prepare(&target, fixture.environments.clone())
                    .err()
                    .unwrap()
                    .code,
                "unverified_launch_target"
            );
        }
        target = fixture.target.clone();
        target.environment_key.push_str("-other");
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "launch_environment_missing"
        );
        target = fixture.target.clone();
        target.bottle_locator = Some(fixture.root.to_string_lossy().into());
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "launch_environment_mismatch"
        );
        target = fixture.target.clone();
        target.architecture = Some("x86".into());
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "launch_target_changed"
        );
        target = fixture.target.clone();
        target.target_path_key = Some("c:\\other.exe".into());
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "launch_target_mismatch"
        );
        target = fixture.target.clone();
        target.launch_path.as_mut().unwrap().push_str("missing");
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "launch_path_missing"
        );
        target = fixture.target.clone();
        target.arguments.push("nul\0argument".into());
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "invalid_launch_arguments"
        );
        target = fixture.target.clone();
        target.working_directory = target.launch_path.clone();
        assert_eq!(
            prepare(&target, fixture.environments.clone())
                .err()
                .unwrap()
                .code,
            "launch_workdir_invalid"
        );
    }
    #[test]
    fn an_existing_complete_process_matches_full_path_not_name_or_bare_pid() {
        let fixture = Fixture::new();
        let plan = fixture.prepare().unwrap();
        let actual = Target {
            pid: 42,
            exe: "Game.exe".into(),
            full_image_path: fixture.target.canonical_exe_path.clone(),
            creation_filetime: Some("12345".into()),
            architecture: Some("x64".into()),
            errors: vec![],
        };
        assert!(plan.already_running(std::slice::from_ref(&actual)).unwrap());
        let mut incomplete = actual.clone();
        incomplete.creation_filetime = None;
        assert_eq!(
            plan.already_running(&[incomplete]).unwrap_err().code,
            "launch_identity_incomplete"
        );
        let mut other = actual.clone();
        other.full_image_path = Some("C:\\Other\\Game.exe".into());
        assert!(!plan.already_running(&[other]).unwrap());
        let mut failed = actual;
        failed.errors.push("identity incomplete".into());
        assert_eq!(
            plan.already_running(&[failed]).unwrap_err().code,
            "launch_identity_incomplete"
        );
    }
    #[test]
    fn shared_pending_survives_reopen_and_expires_without_spawning() {
        let fixture = Fixture::new();
        let path = fixture.root.join("pending.lock");
        let open = || {
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap()
        };
        let start = std::time::UNIX_EPOCH + Duration::from_secs(1_000);
        {
            let mut first = PendingLaunchLease::try_lock(open()).unwrap();
            reserve_launch(&mut first, start).unwrap();
        }
        let mut second = PendingLaunchLease::try_lock(open()).unwrap();
        for time in [
            start + Duration::from_secs(1),
            start - Duration::from_secs(1),
        ] {
            assert_eq!(
                reserve_launch(&mut second, time).unwrap_err().code,
                "game_launch_pending"
            );
        }
        reserve_launch(&mut second, start + Duration::from_secs(30)).unwrap();
        let reset = start - Duration::from_secs(200);
        assert_eq!(
            reserve_launch(&mut second, reset).unwrap_err().code,
            "game_launch_pending"
        );
        reserve_launch(&mut second, reset + Duration::from_secs(30)).unwrap();
    }
    fn assert_inherited_pending_launch_lease(child_drops_guard: bool) {
        use std::{
            io::{Read, Write},
            os::{fd::AsRawFd, raw::c_int, unix::net::UnixStream},
        };

        unsafe extern "C" {
            fn fork() -> c_int;
            fn read(fd: c_int, buffer: *mut u8, count: usize) -> isize;
            fn write(fd: c_int, buffer: *const u8, count: usize) -> isize;
            fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
            fn _exit(status: c_int) -> !;
        }

        let fixture = Fixture::new();
        let path = fixture.root.join("inherited-pending.lock");
        let open = || {
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap()
        };
        let lease = PendingLaunchLease::try_lock(open()).unwrap();
        // Independent opens, never try_clone: finish fallible setup before fork
        // so a setup error cannot strand a child waiting for the release byte.
        let while_owner_alive_file = open();
        let after_owner_drop_file = open();
        let after_child_exit_file = open();
        let (mut parent, child) = UnixStream::pair().unwrap();
        parent
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let child_fd = child.as_raw_fd();
        // SAFETY: the child only closes descriptors/drops this narrow guard,
        // uses one-byte read/write, then _exits without copied Rust cleanup.
        let pid = unsafe { fork() };
        assert!(pid >= 0, "{}", std::io::Error::last_os_error());
        if pid == 0 {
            drop(parent);
            if child_drops_guard {
                drop(lease);
            }
            let mut byte = 1_u8;
            // SAFETY: child_fd and the one-byte buffer remain valid until _exit.
            unsafe {
                if write(child_fd, &byte, 1) != 1 {
                    _exit(11);
                }
                if read(child_fd, &mut byte, 1) != 1 {
                    _exit(12);
                }
                _exit(0);
            }
        }
        drop(child);
        let mut ready = [0];
        let ready_result = parent.read_exact(&mut ready);
        let while_owner_alive = PendingLaunchLease::try_lock(while_owner_alive_file);
        let blocked_while_owner_alive =
            matches!(while_owner_alive, Err(std::fs::TryLockError::WouldBlock));
        drop(while_owner_alive);
        drop(lease);
        // The child is still waiting on the release byte, so its inherited
        // descriptor is alive here unless it explicitly dropped the guard.
        let after_owner_drop = PendingLaunchLease::try_lock(after_owner_drop_file);
        let acquired_after_owner_drop = after_owner_drop.is_ok();
        drop(after_owner_drop);
        // Reap before asserting lock results, so a deliberately broken guard
        // cannot strand a child blocked on this handshake.
        let release_result = parent.write_all(&[1]);
        let mut status = 0;
        let waited = loop {
            // SAFETY: pid is our unreaped child and status is a valid pointer.
            let waited = unsafe { waitpid(pid, &mut status, 0) };
            if waited == pid
                || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                break waited;
            }
        };
        ready_result.unwrap();
        assert_eq!(ready, [1]);
        release_result.unwrap();
        assert_eq!(waited, pid);
        assert_eq!(status, 0);
        assert!(blocked_while_owner_alive);
        assert!(acquired_after_owner_drop);
        drop(PendingLaunchLease::try_lock(after_child_exit_file).unwrap());
    }
    #[test]
    fn owner_drop_releases_pending_launch_with_a_live_inherited_descriptor() {
        assert_inherited_pending_launch_lease(false);
    }
    #[test]
    fn dropping_an_inherited_pending_launch_guard_cannot_unlock_its_owner() {
        assert_inherited_pending_launch_lease(true);
    }
    #[test]
    fn owned_launcher_result_is_checked_and_timeout_is_bounded_without_a_shell() {
        run(Command::new("/usr/bin/true"), Duration::from_secs(1)).unwrap();
        assert_eq!(
            run(Command::new("/usr/bin/false"), Duration::from_secs(1))
                .unwrap_err()
                .code,
            "game_launch_failed"
        );
        let mut sleep = Command::new("/bin/sleep");
        sleep.arg("60");
        let started = Instant::now();
        assert_eq!(
            run(sleep, Duration::from_millis(30)).unwrap_err().code,
            "game_launch_timeout"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
