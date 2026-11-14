//! Explicit acceptance storage isolation. This selects real runtime paths;
//! it does not substitute a repository, vault, Hook backend or Native API.
use app_core::contracts::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const ACCEPTANCE_FLAG: &str = "LUNA_ACCEPTANCE_PROFILE";
const NONCE_ENV: &str = "LUNA_PRODUCT_EXECUTION_NONCE";
const PARENT: &str = "luna-reader-acceptance";
const MARKER: &str = "acceptance-root.json";
const DIAGNOSTIC: &str = "runtime-paths.json";
const MARKER_LIMIT: u64 = 4096;
const INDEX_LIMIT: u64 = 1_048_576;
static PENDING_ID: AtomicU64 = AtomicU64::new(0);

fn error(code: &str, detail: &str) -> AppError {
    AppError::new(code, "acceptance_profile", detail)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RootMarker {
    schema_version: u32,
    kind: String,
    execution_nonce: String,
    canonical_data_root: PathBuf,
    bound_profile_id: Option<String>,
}

#[derive(Clone)]
struct Acceptance {
    nonce: String,
    canonical_data_root: PathBuf,
}

pub(super) struct AcceptanceLease {
    file: File,
    owner_pid: u32,
}

impl Drop for AcceptanceLease {
    fn drop(&mut self) {
        // Forked children must not unlock the parent's live acceptance root.
        if self.owner_pid == std::process::id() {
            let _ = self.file.unlock();
        }
    }
}

pub(super) struct RuntimePaths {
    pub(super) data_root: PathBuf,
    pub(super) session_logs: PathBuf,
    pub(super) target_locks: PathBuf,
    acceptance: Option<Acceptance>,
    lease: Option<AcceptanceLease>,
}

impl RuntimePaths {
    pub(super) fn from_environment(
        canonical_data_root: PathBuf,
        canonical_log_root: PathBuf,
    ) -> AppResult<Self> {
        Self::select(
            canonical_data_root,
            canonical_log_root,
            &std::env::temp_dir(),
            std::env::var_os(ACCEPTANCE_FLAG).as_deref(),
            std::env::var_os(NONCE_ENV).as_deref(),
        )
    }

    fn select(
        canonical_data_root: PathBuf,
        canonical_log_root: PathBuf,
        temporary_root: &Path,
        flag: Option<&OsStr>,
        nonce: Option<&OsStr>,
    ) -> AppResult<Self> {
        // A tracing nonce by itself has always been valid for normal launches.
        // Preserve their exact directories and avoid acceptance I/O entirely.
        if flag.is_none() {
            return Ok(Self {
                session_logs: canonical_log_root.join("sessions"),
                target_locks: canonical_data_root.join("target-locks"),
                data_root: canonical_data_root,
                acceptance: None,
                lease: None,
            });
        }
        if flag != Some(OsStr::new("1")) {
            return Err(error(
                "acceptance_flag",
                "验收配置档开关无效，未使用个人数据目录",
            ));
        }
        let nonce = nonce
            .and_then(OsStr::to_str)
            .filter(|s| {
                s.len() == 32
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or_else(|| error("acceptance_nonce", "验收配置档需要32位小写十六进制执行身份"))?;
        let shared_data = resolve_missing_path(&canonical_data_root)?;
        let shared_logs = resolve_missing_path(&canonical_log_root)?;
        if !temporary_root.is_absolute() {
            return Err(error("acceptance_temp", "系统临时目录必须为绝对路径"));
        }
        // macOS's standard /var temporary path may itself be an OS alias.
        // Resolve that trusted anchor, then reject links in every owned child.
        let temporary_root = fs::canonicalize(temporary_root)
            .map_err(|_| error("acceptance_temp", "系统临时目录无法核对"))?;
        if !temporary_root.is_dir() {
            return Err(error("acceptance_temp", "系统临时目录不可用"));
        }
        let parent = temporary_root.join(PARENT);
        let root = parent.join(nonce);
        if overlaps(&root, &shared_data) || overlaps(&root, &shared_logs) {
            return Err(error(
                "acceptance_overlap",
                "验收目录与个人数据或日志目录重叠",
            ));
        }
        private_directory(&parent)?;
        let created = private_directory(&root)?;
        let lease = acceptance_lease(&root)?;
        let acceptance = Acceptance {
            nonce: nonce.to_owned(),
            canonical_data_root: shared_data.clone(),
        };
        let paths = Self {
            session_logs: root.join("logs").join("sessions"),
            target_locks: shared_data.join("target-locks"),
            data_root: root,
            acceptance: Some(acceptance),
            lease: Some(lease),
        };
        if created {
            let marker = paths.expected_marker(None);
            write_new_json(&paths.data_root.join(MARKER), &marker)?;
        } else {
            let marker = paths.marker()?;
            let index_exists = fs::symlink_metadata(paths.data_root.join("profiles.json")).is_ok();
            if marker.bound_profile_id.is_none() && index_exists {
                return Err(error(
                    "acceptance_incomplete",
                    "验收配置档初始化未完成，请保留现场并使用新的执行身份",
                ));
            }
            if marker.bound_profile_id.is_some() && !index_exists {
                return Err(error(
                    "acceptance_index",
                    "验收配置档索引缺失，原目录已保留",
                ));
            }
        }
        optional_regular(&paths.data_root.join("profiles.json"), INDEX_LIMIT)?;
        validate_owned_tree(&paths.data_root)?;
        Ok(paths)
    }

    pub(super) fn session_logs_for(&self, instance: &str) -> AppResult<PathBuf> {
        if self.acceptance.is_none() {
            return Ok(self.session_logs.clone());
        }
        if !valid_profile_id(instance) {
            return Err(error("acceptance_instance", "验收运行实例身份无效"));
        }
        Ok(self.data_root.join("logs").join(instance).join("sessions"))
    }

    pub(super) fn take_acceptance_lease(&mut self) -> Option<AcceptanceLease> {
        self.lease.take()
    }

    fn expected_marker(&self, profile: Option<String>) -> RootMarker {
        let acceptance = self.acceptance.as_ref().expect("acceptance marker only");
        RootMarker {
            schema_version: 1,
            kind: "luna-acceptance-profile-root/v1".into(),
            execution_nonce: acceptance.nonce.clone(),
            canonical_data_root: acceptance.canonical_data_root.clone(),
            bound_profile_id: profile,
        }
    }

    fn marker(&self) -> AppResult<RootMarker> {
        let bytes = regular_bytes(&self.data_root.join(MARKER), MARKER_LIMIT)?;
        let marker: RootMarker = serde_json::from_slice(&bytes)
            .map_err(|_| error("acceptance_marker", "验收目录标记无效，未回退个人配置"))?;
        let expected = self.expected_marker(None);
        if marker.schema_version != expected.schema_version
            || marker.kind != expected.kind
            || marker.execution_nonce != expected.execution_nonce
            || marker.canonical_data_root != expected.canonical_data_root
            || marker
                .bound_profile_id
                .as_deref()
                .is_some_and(|id| !valid_profile_id(id))
        {
            return Err(error(
                "acceptance_marker",
                "验收目录身份不匹配，原目录已保留",
            ));
        }
        Ok(marker)
    }

    pub(super) fn bind_profile(&self, profile: &str) -> AppResult<()> {
        let Some(acceptance) = &self.acceptance else {
            return Ok(());
        };
        if !valid_profile_id(profile) {
            return Err(error("acceptance_profile_id", "验收配置档身份无效"));
        }
        let marker = self.marker()?;
        if marker
            .bound_profile_id
            .as_deref()
            .is_some_and(|bound| bound != profile)
        {
            return Err(error(
                "acceptance_profile_changed",
                "验收目录不能换用另一个配置档身份",
            ));
        }
        let canonical_index = acceptance.canonical_data_root.join("profiles.json");
        if let Some(bytes) = optional_regular(&canonical_index, INDEX_LIMIT)? {
            let index: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
                error(
                    "acceptance_canonical_index",
                    "无法核对个人配置档身份，验收启动已停止",
                )
            })?;
            let active = index["activeProfileId"]
                .as_str()
                .filter(|id| valid_profile_id(id))
                .ok_or_else(|| {
                    error(
                        "acceptance_canonical_index",
                        "无法核对个人配置档身份，验收启动已停止",
                    )
                })?;
            if index["schemaVersion"] != 1 {
                return Err(error(
                    "acceptance_canonical_index",
                    "个人配置档索引版本无法核对",
                ));
            }
            let mut ids = vec![active];
            if let Some(profiles) = index.get("profiles") {
                for row in profiles
                    .as_array()
                    .ok_or_else(|| error("acceptance_canonical_index", "个人配置档列表无效"))?
                {
                    ids.push(
                        row["profileId"]
                            .as_str()
                            .filter(|id| valid_profile_id(id))
                            .ok_or_else(|| {
                                error("acceptance_canonical_index", "个人配置档身份列表无法核对")
                            })?,
                    );
                }
            }
            if ids.contains(&profile) {
                return Err(error(
                    "acceptance_profile_collision",
                    "验收配置档不得使用个人配置档或凭据命名空间",
                ));
            }
        }
        if marker.bound_profile_id.is_none() {
            replace_json(
                &self.data_root,
                MARKER,
                &self.expected_marker(Some(profile.to_owned())),
            )?;
        }
        Ok(())
    }

    pub(super) fn write_diagnostic(
        &self,
        profile: &str,
        resources: &Path,
        instance: &str,
    ) -> AppResult<()> {
        let Some(acceptance) = &self.acceptance else {
            return Ok(());
        };
        if self.marker()?.bound_profile_id.as_deref() != Some(profile) {
            return Err(error(
                "acceptance_profile_changed",
                "验收配置档尚未绑定，未写入运行回执",
            ));
        }
        let logs = self.session_logs_for(instance)?;
        replace_json(
            &self.data_root,
            DIAGNOSTIC,
            &serde_json::json!({
                "schema_version": 1,
                "kind": "luna-acceptance-runtime-paths/v1",
                "execution_nonce": acceptance.nonce,
                "process_id": std::process::id(),
                "profile_id": profile,
                "server_instance_id": instance,
                "data_root": self.data_root,
                "session_log_root": logs,
                "observation_log": logs.with_file_name("runtime-observation.jsonl"),
                "target_lock_root": self.target_locks,
                "canonical_data_root": acceptance.canonical_data_root,
                "resource_root": resources,
                "scope": "actual bootstrap path selection only",
                "native_gui_verified": false,
                "product_acceptance_approved": false
            }),
        )
    }
}

fn valid_profile_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

fn resolve_missing_path(path: &Path) -> AppResult<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(error(
            "acceptance_path",
            "验收路径必须为无相对跳转的绝对路径",
        ));
    }
    let mut existing = path;
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    existing
                        .file_name()
                        .ok_or_else(|| error("acceptance_path", "无法核对路径父目录"))?
                        .to_owned(),
                );
                existing = existing
                    .parent()
                    .ok_or_else(|| error("acceptance_path", "无法核对路径父目录"))?;
            }
            Err(_) => return Err(error("acceptance_path", "共享数据路径不可读")),
        }
    }
    let mut resolved =
        fs::canonicalize(existing).map_err(|_| error("acceptance_path", "无法核对共享数据路径"))?;
    if !resolved.is_dir() {
        return Err(error("acceptance_path", "共享数据路径不是目录"));
    }
    for part in suffix.iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

fn private_directory(path: &Path) -> AppResult<bool> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    let created = match builder.create(path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(_) => return Err(error("acceptance_directory", "无法创建私有验收目录")),
    };
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| error("acceptance_directory", "无法核对私有验收目录"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(error("acceptance_directory", "验收路径不是实际私有目录"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(error(
                "acceptance_permissions",
                "验收目录不得对其他用户开放",
            ));
        }
    }
    Ok(created)
}

fn optional_regular(path: &Path, limit: u64) -> AppResult<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(error("acceptance_file", "无法核对验收文件")),
        Ok(_) => regular_bytes(path, limit).map(Some),
    }
}

fn validate_owned_tree(root: &Path) -> AppResult<()> {
    // Marked roots can be reopened, but old child links must never redirect
    // an existing initializer outside the private root. Do not read user data.
    let mut pending = vec![(root.to_owned(), 0usize)];
    let mut entries = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 32 {
            return Err(error(
                "acceptance_tree",
                "验收目录层级过深，未初始化任何运行组件",
            ));
        }
        let children =
            fs::read_dir(&directory).map_err(|_| error("acceptance_tree", "验收子目录无法核对"))?;
        for child in children {
            entries += 1;
            if entries > 50_000 {
                return Err(error(
                    "acceptance_tree",
                    "验收目录项目过多，请使用新的执行身份",
                ));
            }
            let path = child
                .map_err(|_| error("acceptance_tree", "验收子路径无法读取"))?
                .path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| error("acceptance_tree", "验收子路径身份无法核对"))?;
            if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
                return Err(error("acceptance_tree", "验收子路径不能包含链接或特殊文件"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.is_file() && metadata.nlink() != 1 {
                    return Err(error("acceptance_tree", "验收子文件不能复用硬链接"));
                }
            }
            if metadata.is_dir() {
                pending.push((path, depth + 1));
            }
        }
    }
    Ok(())
}

fn acceptance_lease(root: &Path) -> AppResult<AcceptanceLease> {
    let path = root.join("acceptance-root.lock");
    optional_regular(&path, 0)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&path)
        .map_err(|_| error("acceptance_lock", "无法打开私有验收租约"))?;
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| error("acceptance_lock", "验收租约身份不可读"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != 0 {
        return Err(error("acceptance_lock", "验收租约必须是实际空文件"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = file
            .metadata()
            .map_err(|_| error("acceptance_lock", "验收租约身份不可读"))?;
        if metadata.nlink() != 1 || actual.dev() != metadata.dev() || actual.ino() != metadata.ino()
        {
            return Err(error("acceptance_lock", "验收租约身份发生变化"));
        }
    }
    file.try_lock()
        .map_err(|_| error("acceptance_in_use", "此验收身份已被另一个启动实例占用"))?;
    Ok(AcceptanceLease {
        file,
        owner_pid: std::process::id(),
    })
}

fn regular_bytes(path: &Path, limit: u64) -> AppResult<Vec<u8>> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| error("acceptance_file", "验收文件缺失或不可读"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(error("acceptance_file", "验收文件必须为有界的实际文件"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(error("acceptance_file", "验收文件不能复用硬链接"));
        }
    }
    let file = File::open(path).map_err(|_| error("acceptance_file", "验收文件无法打开"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = file
            .metadata()
            .map_err(|_| error("acceptance_file", "验收文件身份不可读"))?;
        if actual.dev() != metadata.dev() || actual.ino() != metadata.ino() {
            return Err(error("acceptance_file", "验收文件在核对过程中发生变化"));
        }
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("acceptance_file", "验收文件无法读取"))?;
    let after =
        fs::symlink_metadata(path).map_err(|_| error("acceptance_file", "验收文件在读取时失效"))?;
    if bytes.len() as u64 > limit || after.file_type().is_symlink() || !after.is_file() {
        return Err(error("acceptance_file", "验收文件在读取时失效"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if after.dev() != metadata.dev() || after.ino() != metadata.ino() || after.nlink() != 1 {
            return Err(error("acceptance_file", "验收文件在读取时发生身份变化"));
        }
    }
    if after.len() != metadata.len() || after.modified().ok() != metadata.modified().ok() {
        return Err(error("acceptance_file", "验收文件在读取时发生内容变化"));
    }
    Ok(bytes)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| error("acceptance_write", "无法编码验收目录身份"))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| error("acceptance_write", "无法独占创建验收文件"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| error("acceptance_write", "验收文件未能完整保存"))
}

fn replace_json(root: &Path, name: &str, value: &impl Serialize) -> AppResult<()> {
    optional_regular(&root.join(name), INDEX_LIMIT)?;
    let pending = root.join(format!(
        ".{name}.{}.{}.pending",
        std::process::id(),
        PENDING_ID.fetch_add(1, Ordering::Relaxed)
    ));
    write_new_json(&pending, value)?;
    fs::rename(&pending, root.join(name))
        .map_err(|_| error("acceptance_write", "验收文件未能原子替换"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::ports::IdentitySource;
    use app_infra::platform::SystemIdentity;

    const NONCE: &str = "0123456789abcdef0123456789abcdef";

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(SystemIdentity.new_id("runtime-path-tests"));
            private_directory(&root).unwrap();
            Self(fs::canonicalize(root).unwrap())
        }
        fn data(&self) -> PathBuf {
            self.0.join("personal-data")
        }
        fn logs(&self) -> PathBuf {
            self.0.join("personal-logs")
        }
        fn select(&self, flag: Option<&str>, nonce: Option<&str>) -> AppResult<RuntimePaths> {
            RuntimePaths::select(
                self.data(),
                self.logs(),
                &self.0,
                flag.map(OsStr::new),
                nonce.map(OsStr::new),
            )
        }
        fn index(&self, active: &str, another: &str) {
            fs::create_dir_all(self.data()).unwrap();
            fs::write(self.data().join("profiles.json"), serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"activeProfileId":active,"profiles":[{"profileId":another}]})).unwrap()).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn normal_paths_and_tracing_nonce_do_not_create_acceptance_files() {
        let f = Fixture::new();
        let p = f.select(None, Some("not-an-acceptance-nonce")).unwrap();
        assert_eq!(p.data_root, f.data());
        assert_eq!(p.session_logs, f.logs().join("sessions"));
        assert_eq!(p.target_locks, f.data().join("target-locks"));
        p.bind_profile("ordinary-profile").unwrap();
        p.write_diagnostic(
            "ordinary-profile",
            Path::new("/actual/resources"),
            "server-normal",
        )
        .unwrap();
        assert!(!f.data().exists());
        assert!(!f.logs().exists());
        assert!(!f.0.join(PARENT).exists());
    }

    #[test]
    fn explicit_acceptance_isolates_data_logs_and_geometry_but_shares_target_locks() {
        let f = Fixture::new();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        assert_eq!(p.data_root, f.0.join(PARENT).join(NONCE));
        assert_eq!(p.session_logs, p.data_root.join("logs/sessions"));
        assert_eq!(p.target_locks, f.data().join("target-locks"));
        assert_eq!(
            p.session_logs_for("server-actual").unwrap(),
            p.data_root.join("logs/server-actual/sessions")
        );
        assert!(p.session_logs_for("../escape").is_err());
        assert!(!f.data().exists());
        assert!(!f.logs().exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&p.data_root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(p.data_root.join(MARKER))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn invalid_flags_and_nonces_never_fall_back_or_create_directories() {
        let f = Fixture::new();
        for flag in ["0", "", "true", " 1"] {
            assert!(
                matches!(f.select(Some(flag), Some(NONCE)), Err(e) if e.code == "acceptance_flag")
            );
        }
        for nonce in [
            None,
            Some(""),
            Some("../escape"),
            Some("0123456789ABCDEF0123456789ABCDEF"),
            Some("0123456789abcdef0123456789abcdef0"),
        ] {
            assert!(matches!(f.select(Some("1"), nonce), Err(e) if e.code == "acceptance_nonce"));
        }
        assert!(!f.0.join(PARENT).exists());
        assert!(!f.data().exists());
    }

    #[test]
    fn profile_uuid_uses_existing_real_generator_and_supports_marked_reopen() {
        let f = Fixture::new();
        let active = SystemIdentity.new_id("profile");
        let secondary = SystemIdentity.new_id("profile");
        let instance = SystemIdentity.new_id("server");
        f.index(&active, &secondary);
        let original = fs::read(f.data().join("profiles.json")).unwrap();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        let id = super::super::profile_id(&p.data_root).unwrap();
        assert!(id.starts_with("profile-"));
        p.bind_profile(&id).unwrap();
        p.write_diagnostic(&id, Path::new("/actual/signed-bundle/resources"), &instance)
            .unwrap();
        let private = p.data_root.clone();
        drop(p);
        let reopened = f.select(Some("1"), Some(NONCE)).unwrap();
        assert_eq!(super::super::profile_id(&reopened.data_root).unwrap(), id);
        reopened.bind_profile(&id).unwrap();
        assert_eq!(fs::read(f.data().join("profiles.json")).unwrap(), original);
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(private.join(DIAGNOSTIC)).unwrap()).unwrap();
        assert_eq!(diagnostic["execution_nonce"], NONCE);
        assert_eq!(diagnostic["profile_id"], id);
        assert_eq!(diagnostic["process_id"], std::process::id());
        assert_eq!(diagnostic["native_gui_verified"], false);
        assert_eq!(
            diagnostic["target_lock_root"],
            f.data().join("target-locks").to_string_lossy().as_ref()
        );
        assert_eq!(diagnostic["server_instance_id"], instance);
        assert_eq!(
            diagnostic["observation_log"],
            private
                .join("logs")
                .join(instance)
                .join("runtime-observation.jsonl")
                .to_string_lossy()
                .as_ref()
        );
    }

    #[test]
    fn active_or_inactive_personal_profile_id_cannot_reuse_vault_namespace() {
        let f = Fixture::new();
        let active = SystemIdentity.new_id("profile");
        let secondary = SystemIdentity.new_id("profile");
        f.index(&active, &secondary);
        let original = fs::read(f.data().join("profiles.json")).unwrap();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        for id in [&active, &secondary] {
            assert!(
                matches!(p.bind_profile(id), Err(e) if e.code == "acceptance_profile_collision")
            );
        }
        assert_eq!(fs::read(f.data().join("profiles.json")).unwrap(), original);
        assert!(p.marker().unwrap().bound_profile_id.is_none());
    }

    #[test]
    fn bound_identity_cannot_be_swapped_or_disappear() {
        let f = Fixture::new();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        let id = super::super::profile_id(&p.data_root).unwrap();
        p.bind_profile(&id).unwrap();
        assert!(
            matches!(p.bind_profile("different-profile"), Err(e) if e.code == "acceptance_profile_changed")
        );
        fs::remove_file(p.data_root.join("profiles.json")).unwrap();
        drop(p);
        assert!(matches!(f.select(Some("1"), Some(NONCE)), Err(e) if e.code == "acceptance_index"));
    }

    #[test]
    fn unmarked_existing_root_is_not_adopted() {
        let f = Fixture::new();
        private_directory(&f.0.join(PARENT)).unwrap();
        private_directory(&f.0.join(PARENT).join(NONCE)).unwrap();
        assert!(f.select(Some("1"), Some(NONCE)).is_err());
    }

    #[test]
    fn marker_mismatch_unknown_fields_and_interrupted_initialization_are_rejected() {
        let f = Fixture::new();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        let private = p.data_root.clone();
        let original = fs::read(private.join(MARKER)).unwrap();
        drop(p);
        for bad in [
            serde_json::json!({"schema_version":1}),
            serde_json::json!({"claimed_pass":true}),
        ] {
            fs::write(private.join(MARKER), serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(f.select(Some("1"), Some(NONCE)).is_err());
        }
        fs::write(private.join(MARKER), original).unwrap();
        fs::write(private.join("profiles.json"), b"{}").unwrap();
        assert!(
            matches!(f.select(Some("1"), Some(NONCE)), Err(e) if e.code == "acceptance_incomplete")
        );
    }

    #[test]
    fn malformed_personal_index_blocks_acceptance_without_changing_it() {
        let f = Fixture::new();
        fs::create_dir_all(f.data()).unwrap();
        fs::write(f.data().join("profiles.json"), b"{bad").unwrap();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        assert!(
            matches!(p.bind_profile("private-profile"), Err(e) if e.code == "acceptance_canonical_index")
        );
        assert_eq!(fs::read(f.data().join("profiles.json")).unwrap(), b"{bad");
    }

    #[test]
    fn temporary_root_may_not_overlap_personal_data_or_logs() {
        let f = Fixture::new();
        let private = f.0.join(PARENT).join(NONCE);
        assert!(
            matches!(RuntimePaths::select(private.clone(), f.logs(), &f.0, Some(OsStr::new("1")), Some(OsStr::new(NONCE))), Err(e) if e.code == "acceptance_overlap")
        );
        assert!(
            matches!(RuntimePaths::select(f.data(), private, &f.0, Some(OsStr::new("1")), Some(OsStr::new(NONCE))), Err(e) if e.code == "acceptance_overlap")
        );
        assert!(!f.0.join(PARENT).exists());
    }

    #[cfg(unix)]
    #[test]
    fn owned_parent_root_marker_and_profile_symlinks_or_hardlinks_are_rejected() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        let outside = f.0.join("outside");
        private_directory(&outside).unwrap();
        symlink(&outside, f.0.join(PARENT)).unwrap();
        assert!(f.select(Some("1"), Some(NONCE)).is_err());
        fs::remove_file(f.0.join(PARENT)).unwrap();
        private_directory(&f.0.join(PARENT)).unwrap();
        symlink(&outside, f.0.join(PARENT).join(NONCE)).unwrap();
        assert!(
            matches!(f.select(Some("1"), Some(NONCE)), Err(e) if e.code == "acceptance_directory")
        );
        fs::remove_file(f.0.join(PARENT).join(NONCE)).unwrap();
        let p = f.select(Some("1"), Some(NONCE)).unwrap();
        let private = p.data_root.clone();
        let bytes = fs::read(private.join(MARKER)).unwrap();
        drop(p);
        let external_marker = outside.join("marker.json");
        fs::write(&external_marker, bytes).unwrap();
        fs::remove_file(private.join(MARKER)).unwrap();
        symlink(&external_marker, private.join(MARKER)).unwrap();
        assert!(f.select(Some("1"), Some(NONCE)).is_err());
        fs::remove_file(private.join(MARKER)).unwrap();
        fs::hard_link(&external_marker, private.join(MARKER)).unwrap();
        assert!(f.select(Some("1"), Some(NONCE)).is_err());
        fs::remove_file(private.join(MARKER)).unwrap();
        fs::copy(&external_marker, private.join(MARKER)).unwrap();
        symlink(&external_marker, private.join("profiles.json")).unwrap();
        assert!(f.select(Some("1"), Some(NONCE)).is_err());
    }

    #[test]
    fn concurrent_nonce_is_rejected_before_profile_or_log_initialization() {
        let f = Fixture::new();
        let mut first = f.select(Some("1"), Some(NONCE)).unwrap();
        let private = first.data_root.clone();
        let profile = super::super::profile_id(&private).unwrap();
        let instance = SystemIdentity.new_id("server");
        first.bind_profile(&profile).unwrap();
        first
            .write_diagnostic(&profile, Path::new("/actual/bundle/resources"), &instance)
            .unwrap();
        let diagnostic = fs::read(private.join(DIAGNOSTIC)).unwrap();
        let log = first
            .session_logs_for(&instance)
            .unwrap()
            .with_file_name("runtime-observation.jsonl");
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        fs::write(&log, b"first-owned-observation").unwrap();
        let profile_index = fs::read(private.join("profiles.json")).unwrap();
        let guard = first.take_acceptance_lease().unwrap();
        #[cfg(unix)]
        let lease_inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(private.join("acceptance-root.lock"))
                .unwrap()
                .ino()
        };
        drop(first);
        assert!(
            matches!(f.select(Some("1"), Some(NONCE)), Err(e) if e.code == "acceptance_in_use")
        );
        assert_eq!(fs::read(private.join(DIAGNOSTIC)).unwrap(), diagnostic);
        assert_eq!(fs::read(&log).unwrap(), b"first-owned-observation");
        assert_eq!(
            fs::read(private.join("profiles.json")).unwrap(),
            profile_index
        );
        drop(guard);
        let reopened = f.select(Some("1"), Some(NONCE)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(private.join("acceptance-root.lock"))
                    .unwrap()
                    .ino(),
                lease_inode
            );
        }
        assert_eq!(
            super::super::profile_id(&reopened.data_root).unwrap(),
            profile
        );
    }

    #[cfg(unix)]
    #[test]
    fn reopened_private_children_and_lease_must_not_redirect_to_external_files() {
        use std::os::unix::fs::symlink;
        for name in [
            "logs",
            "profiles",
            "window-state.json",
            "acceptance-root.lock",
        ] {
            let f = Fixture::new();
            let first = f.select(Some("1"), Some(NONCE)).unwrap();
            let private = first.data_root.clone();
            drop(first);
            let outside = f.0.join("external-data");
            fs::write(&outside, b"external-sentinel").unwrap();
            let child = private.join(name);
            if child.exists() {
                fs::remove_file(&child).unwrap();
            }
            symlink(&outside, &child).unwrap();
            assert!(f.select(Some("1"), Some(NONCE)).is_err());
            assert_eq!(fs::read(&outside).unwrap(), b"external-sentinel");
        }
        let f = Fixture::new();
        let first = f.select(Some("1"), Some(NONCE)).unwrap();
        let private = first.data_root.clone();
        drop(first);
        let outside = f.0.join("external-empty-lock");
        fs::write(&outside, b"").unwrap();
        fs::remove_file(private.join("acceptance-root.lock")).unwrap();
        fs::hard_link(&outside, private.join("acceptance-root.lock")).unwrap();
        assert!(f.select(Some("1"), Some(NONCE)).is_err());
    }

    #[test]
    fn distinct_nonces_and_instances_do_not_share_private_profiles_or_logs() {
        let f = Fixture::new();
        let first = f.select(Some("1"), Some(NONCE)).unwrap();
        let second = f
            .select(Some("1"), Some("fedcba9876543210fedcba9876543210"))
            .unwrap();
        assert_ne!(first.data_root, second.data_root);
        assert_eq!(first.target_locks, second.target_locks);
        assert_ne!(
            super::super::profile_id(&first.data_root).unwrap(),
            super::super::profile_id(&second.data_root).unwrap()
        );
        assert_ne!(
            first
                .session_logs_for(&SystemIdentity.new_id("server"))
                .unwrap(),
            first
                .session_logs_for(&SystemIdentity.new_id("server"))
                .unwrap()
        );
    }

    #[test]
    fn fully_formed_marker_cannot_change_nonce_canonical_root_or_version() {
        let f = Fixture::new();
        let paths = f.select(Some("1"), Some(NONCE)).unwrap();
        let private = paths.data_root.clone();
        let marker = serde_json::to_value(paths.marker().unwrap()).unwrap();
        drop(paths);
        for (key, value) in [
            (
                "execution_nonce",
                serde_json::json!("fedcba9876543210fedcba9876543210"),
            ),
            ("canonical_data_root", serde_json::json!(f.logs())),
            ("schema_version", serde_json::json!(2)),
            ("unknown_approval", serde_json::json!(true)),
        ] {
            let mut changed = marker.clone();
            changed[key] = value;
            fs::write(private.join(MARKER), serde_json::to_vec(&changed).unwrap()).unwrap();
            assert!(
                matches!(f.select(Some("1"), Some(NONCE)), Err(e) if e.code == "acceptance_marker")
            );
        }
    }
}
