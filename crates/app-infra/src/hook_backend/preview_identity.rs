//! Frozen preview identity and proof checks. External verification does not own the Hook lease.
use super::window_association::{Candidate, crossover_environments};
use super::{CoreHookBackend, error};
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
};
use hook_client::{Client, LaunchSettings, Target};
use std::{fs::File, path::PathBuf, sync::Arc};

/// Opaque run-scoped context for an explicit user-selected native preview.
/// File mappings and launcher metadata only filter mistaken choices; they do
/// not prove that a macOS owner executes the selected Windows process.
#[derive(Clone)]
pub struct PreviewPairingContext {
    candidate_id: String,
    candidate: Candidate,
    bottle: PathBuf,
    image_path: PathBuf,
    image: Arc<File>,
    file_identity: (u64, u64),
    file_stamp: PreviewFileStamp,
}

#[cfg(all(test, unix))]
mod requested_drive_tests {
    use super::*;
    use hook_client::{WindowRect, WindowTarget};
    use std::path::Path;

    struct Fixture {
        root: PathBuf,
        bottle: PathBuf,
        image: PathBuf,
        resources: PathBuf,
        candidate: Candidate,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("preview-requested-drive-{}", uuid::Uuid::new_v4()));
            let bottle = root.join("Bottle");
            std::fs::create_dir_all(bottle.join("dosdevices")).unwrap();
            std::fs::create_dir_all(bottle.join("drive_c/Games")).unwrap();
            std::os::unix::fs::symlink("../drive_c", bottle.join("dosdevices/c:")).unwrap();
            let image = bottle.join("drive_c/Games/Game.exe");
            std::fs::write(&image, b"immutable test image").unwrap();
            let resources = root.join("resources");
            std::fs::create_dir(&resources).unwrap();
            std::fs::write(
                resources.join("core-runtime.lock.json"),
                serde_json::to_vec(&serde_json::json!({
                    "abi_version": 1, "protocol_version": 2,
                    "commit": "e9242ebac32a09e911f306b7c90714891c217302"
                }))
                .unwrap(),
            )
            .unwrap();
            let bottle = bottle.canonicalize().unwrap();
            let target = Target {
                pid: 41,
                exe: "Game.exe".into(),
                full_image_path: Some("C:\\Games\\Game.exe".into()),
                creation_filetime: Some("1234".into()),
                architecture: Some("x64".into()),
                errors: vec![],
            };
            let candidate = Candidate {
                target: target.clone(),
                window: Some(WindowTarget {
                    hwnd: "125".into(),
                    title: "Controlled real-file target".into(),
                    class_name: "Game".into(),
                    rect: WindowRect {
                        left: 0,
                        top: 0,
                        right: 640,
                        bottom: 480,
                    },
                    target,
                }),
                settings: LaunchSettings {
                    wine_path: "/not-executed".into(),
                    bottle: "Bottle".into(),
                },
                environment_key: format!("crossover:{}", bottle.display()),
            };
            Self {
                root,
                bottle,
                image,
                resources,
                candidate,
            }
        }
        fn context(&self) -> PreviewPairingContext {
            preview_context(
                &self.candidate.selection_identity().unwrap(),
                self.candidate.clone(),
            )
            .unwrap()
        }
        fn freeze(&self) -> AppResult<FrozenPreviewTarget> {
            let backend = CoreHookBackend::new(
                self.resources.clone(),
                self.root.join("logs"),
                self.root.join("locks"),
            );
            let id = self.candidate.selection_identity().unwrap();
            backend.remember(vec![(self.candidate.clone(), "controlled".into())])?;
            // The production trusted-candidate -> preview_context -> resource
            // freeze entry runs with real files, without spawning a Wine probe.
            backend.freeze_preview_target(&id)
        }
        fn remap_c(&self, destination: &Path) {
            let link = self.bottle.join("dosdevices/c:");
            std::fs::remove_file(&link).unwrap();
            std::os::unix::fs::symlink(destination, link).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn production_freeze_keeps_real_target_with_an_unrelated_dangling_drive() {
        let fixture = Fixture::new();
        std::os::unix::fs::symlink(
            fixture.root.join("unmounted"),
            fixture.bottle.join("dosdevices/d:"),
        )
        .unwrap();
        let frozen = fixture.freeze().unwrap();
        assert_eq!(
            frozen.candidate_id(),
            fixture.candidate.selection_identity().unwrap()
        );
        assert_eq!(
            frozen.file_stamp,
            preview_file_stamp(&File::open(&fixture.image).unwrap()).unwrap()
        );
        verify_preview_image(&fixture.context()).unwrap();
    }
    #[test]
    fn production_freeze_rejects_the_requested_broken_mapping_and_missing_image() {
        let fixture = Fixture::new();
        fixture.remap_c(&fixture.root.join("unmounted"));
        assert_eq!(fixture.freeze().err().unwrap().code, "launch_drive_missing");
        fixture.remap_c(&fixture.bottle.join("drive_c"));
        std::fs::remove_file(&fixture.image).unwrap();
        assert_eq!(fixture.freeze().err().unwrap().code, "launch_path_missing");
    }
    #[test]
    fn production_freeze_rejects_non_absolute_or_device_stream_windows_targets() {
        let mut fixture = Fixture::new();
        for path in [
            "Game.exe",
            "C:Games\\Game.exe",
            "C:\\..\\Game.exe",
            "C:\\Games\\Game.exe:stream",
            "/tmp/Game.exe",
        ] {
            fixture.candidate.target.full_image_path = Some(path.into());
            fixture.candidate.window.as_mut().unwrap().target = fixture.candidate.target.clone();
            assert!(fixture.freeze().is_err(), "{path}");
        }
    }
    #[test]
    fn production_image_revalidation_rejects_same_drive_remap_even_for_a_hardlink_alias() {
        let fixture = Fixture::new();
        let other = fixture.root.join("other-drive");
        std::fs::create_dir_all(other.join("Games")).unwrap();
        std::fs::hard_link(&fixture.image, other.join("Games/Game.exe")).unwrap();
        let context = fixture.context();
        let stamp = preview_file_stamp(&context.image).unwrap();
        fixture.remap_c(&other);
        assert_eq!(preview_file_stamp(&context.image).unwrap(), stamp);
        let rejection = verify_preview_image(&context).unwrap_err();
        assert_eq!(rejection.code, "preview_selection_mismatch");
        assert_eq!(rejection.message, "目标程序驱动器映射已变化");
    }
    #[test]
    fn production_frozen_reopen_preserves_path_and_rejects_same_inode_drive_remap() {
        let fixture = Fixture::new();
        let other = fixture.root.join("other-drive");
        std::fs::create_dir_all(other.join("Games")).unwrap();
        std::fs::hard_link(&fixture.image, other.join("Games/Game.exe")).unwrap();
        let frozen = fixture.freeze().unwrap();
        assert_eq!(frozen.image_path, fixture.image.canonicalize().unwrap());
        reopen_frozen_preview_context(&frozen).unwrap();
        fixture.remap_c(&other);
        let reopened = fixture.context();
        assert_eq!(reopened.file_stamp, frozen.file_stamp);
        assert_ne!(reopened.image_path, frozen.image_path);
        assert_eq!(
            reopen_frozen_preview_context(&frozen).err().unwrap().code,
            "preview_selection_mismatch"
        );
    }
    #[test]
    fn production_inventory_rechecks_original_mapping_after_same_inode_remap() {
        let fixture = Fixture::new();
        let other = fixture.root.join("other-drive");
        std::fs::create_dir_all(other.join("Games")).unwrap();
        std::fs::hard_link(&fixture.image, other.join("Games/Game.exe")).unwrap();
        let context = fixture.context();
        let target = fixture.candidate.target.clone();
        verify_preview_image(&context).unwrap();
        verify_preview_instances(&context, vec![target.clone()], &|| false).unwrap();
        // Model a mapping change while discovery runs; the returned inventory
        // still refers to the same file inode and exact process identity.
        fixture.remap_c(&other);
        assert_eq!(fixture.context().file_stamp, context.file_stamp);
        assert_eq!(
            verify_preview_instances(&context, vec![target], &|| false)
                .unwrap_err()
                .code,
            "preview_selection_mismatch"
        );
    }
    #[test]
    fn production_image_revalidation_still_rejects_mapping_loss_and_image_replacement() {
        let fixture = Fixture::new();
        let context = fixture.context();
        fixture.remap_c(&fixture.root.join("unmounted"));
        assert_eq!(
            verify_preview_image(&context).unwrap_err().code,
            "launch_drive_missing"
        );
        fixture.remap_c(&fixture.bottle.join("drive_c"));
        std::fs::remove_file(&fixture.image).unwrap();
        std::fs::write(&fixture.image, b"immutable test image").unwrap();
        assert_eq!(
            verify_preview_image(&context).unwrap_err().code,
            "preview_selection_mismatch"
        );
    }
}
impl PreviewPairingContext {
    pub fn candidate_id(&self) -> &str {
        &self.candidate_id
    }
    pub fn bottle(&self) -> &std::path::Path {
        &self.bottle
    }
    pub fn image_path(&self) -> &std::path::Path {
        &self.image_path
    }
    pub fn image(&self) -> &File {
        &self.image
    }
}
/// Private Rust/worker wire value, never a public application IPC contract.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenPreviewTarget {
    candidate_id: String,
    candidate: Candidate,
    image_path: PathBuf,
    file_stamp: PreviewFileStamp,
    resource_manifest_sha256: String,
}
/// Internal application adapter: never reachable through a UI-provided native ID.
pub trait PreviewProofProvider: Send + Sync {
    fn verify_bound_window(&self, candidate_id: &str) -> AppResult<GameWindowBindingProjection>;
}
impl FrozenPreviewTarget {
    pub fn candidate_id(&self) -> &str {
        &self.candidate_id
    }
    pub fn window_title(&self) -> Option<String> {
        self.candidate
            .window
            .as_ref()
            .map(|window| window.title.clone())
    }
}
pub struct PreviewCollectorVerifier {
    client: Client,
    context: PreviewPairingContext,
}
impl PreviewPairingContext {
    fn freeze(&self, resource_manifest_sha256: String) -> FrozenPreviewTarget {
        FrozenPreviewTarget {
            candidate_id: self.candidate_id.clone(),
            candidate: self.candidate.clone(),
            image_path: self.image_path.clone(),
            file_stamp: self.file_stamp,
            resource_manifest_sha256,
        }
    }
}
impl PreviewCollectorVerifier {
    pub fn open(resources: PathBuf, logs: PathBuf, target: FrozenPreviewTarget) -> AppResult<Self> {
        if target.candidate_id.len() != 64
            || target.candidate.selection_identity().as_deref() != Some(&target.candidate_id)
            || target.candidate.window.as_ref().is_none_or(|window| {
                let mut owner = target.candidate.clone();
                owner.target = window.target.clone();
                owner.process_identity() != target.candidate.process_identity()
            })
        {
            return Err(error("preview_selection_mismatch", "冻结目标身份不一致"));
        }
        if !collector_environment_matches(&target.candidate, &crossover_environments()?)
            || preview_resource_snapshot(&resources)?.manifest_sha256
                != target.resource_manifest_sha256
        {
            return Err(error(
                "preview_selection_mismatch",
                "冻结环境或资源身份不一致",
            ));
        }
        let context = reopen_frozen_preview_context(&target)?;
        Ok(Self {
            client: Client::new(resources, logs),
            context,
        })
    }
    pub fn with_window_challenge<T>(
        &self,
        nonce: &str,
        observe: impl FnOnce() -> Result<T, String>,
    ) -> AppResult<T> {
        let candidate = &self.context.candidate;
        let window = candidate
            .window
            .as_ref()
            .ok_or_else(|| error("preview_target_missing", "目标没有窗口"))?;
        self.client
            .with_window_title_challenge(&candidate.settings, window, nonce, observe)
            .map_err(|message| window_title_proof_error(&message))
    }
    pub fn context(&self) -> &PreviewPairingContext {
        &self.context
    }
    pub fn verify(&self, stopped: &dyn Fn() -> bool) -> AppResult<()> {
        verify_preview_context(&self.client, &self.context, stopped)
    }
}
pub(super) fn preview_resource_snapshot(
    resources: &std::path::Path,
) -> AppResult<ResourceSnapshotInput> {
    let bytes = std::fs::read(resources.join("core-runtime.lock.json")).map_err(|_| {
        error(
            "core_resources_missing",
            "新Core资源清单缺失，请重新安装完整客户端",
        )
    })?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| error("core_resources_invalid", "Core资源清单无效"))?;
    if value["abi_version"] != 1
        || value["protocol_version"] != 2
        || value["commit"] != "e9242ebac32a09e911f306b7c90714891c217302"
    {
        return Err(error("core_version_mismatch", "Core ABI、协议或版本不匹配"));
    }
    let hash = content_hash(&bytes);
    Ok(ResourceSnapshotInput {
        snapshot_id: format!("core-{hash}"),
        kind: "core".into(),
        version: value["commit"].as_str().unwrap_or("").into(),
        manifest_sha256: hash,
        contract_version: "abi1-jsonl2".into(),
        metadata_json: serde_json::to_string(&value)
            .map_err(|_| error("core_resources_invalid", "无法读取Core身份"))?,
    })
}
pub(super) fn collector_environment_matches(
    candidate: &Candidate,
    environments: &[(LaunchSettings, String)],
) -> bool {
    environments
        .iter()
        .any(|(settings, key)| settings == &candidate.settings && key == &candidate.environment_key)
}
fn preview_context(candidate_id: &str, candidate: Candidate) -> AppResult<PreviewPairingContext> {
    let bottle = candidate
        .environment_key
        .strip_prefix("crossover:")
        .ok_or_else(|| {
            error(
                "preview_selection_unsupported",
                "当前环境不支持系统窗口配对",
            )
        })?;
    let bottle = PathBuf::from(bottle)
        .canonicalize()
        .map_err(|_| error("preview_selection_mismatch", "无法核对目标容器"))?;
    let image_path = crate::platform::game_launch::resolve_windows_target(
        &bottle,
        candidate
            .target
            .full_image_path
            .as_deref()
            .ok_or_else(|| error("preview_selection_mismatch", "缺少目标程序身份"))?,
    )?;
    let image = Arc::new(
        File::open(&image_path)
            .map_err(|_| error("preview_selection_mismatch", "无法读取目标程序身份"))?,
    );
    Ok(PreviewPairingContext {
        candidate_id: candidate_id.into(),
        candidate,
        bottle,
        image_path,
        file_identity: preview_file_identity(&image)?,
        file_stamp: preview_file_stamp(&image)?,
        image,
    })
}
fn reopen_frozen_preview_context(target: &FrozenPreviewTarget) -> AppResult<PreviewPairingContext> {
    let context = preview_context(&target.candidate_id, target.candidate.clone())?;
    if context.image_path != target.image_path || context.file_stamp != target.file_stamp {
        return Err(error(
            "preview_selection_mismatch",
            "冻结目标路径或文件已变化",
        ));
    }
    Ok(context)
}
fn collector_stopped(stopped: &dyn Fn() -> bool) -> AppResult<()> {
    if stopped() {
        Err(error("preview_cancelled", "独立采集正在停止"))
    } else {
        Ok(())
    }
}
fn verify_preview_image(context: &PreviewPairingContext) -> AppResult<()> {
    let current_path = crate::platform::game_launch::resolve_windows_target(
        &context.bottle,
        context
            .candidate
            .target
            .full_image_path
            .as_deref()
            .ok_or_else(|| error("preview_selection_mismatch", "缺少目标程序身份"))?,
    )?;
    if current_path != context.image_path {
        return Err(error(
            "preview_selection_mismatch",
            "目标程序驱动器映射已变化",
        ));
    }
    let image = File::open(&current_path)
        .map_err(|_| error("preview_selection_mismatch", "目标程序路径已变化"))?;
    if preview_file_stamp(&image)? != context.file_stamp
        || preview_file_stamp(&context.image)? != context.file_stamp
    {
        return Err(error("preview_selection_mismatch", "目标程序文件已变化"));
    }
    Ok(())
}
fn verify_preview_context(
    client: &Client,
    context: &PreviewPairingContext,
    stopped: &dyn Fn() -> bool,
) -> AppResult<()> {
    collector_stopped(stopped)?;
    let candidate = &context.candidate;
    let window = candidate
        .window
        .as_ref()
        .ok_or_else(|| error("stale_target", "游戏窗口已解除关联"))?;
    let verified = client
        .verify_bound_window(&candidate.settings, window)
        .map_err(|e| preview_verification_error(&e))?;
    collector_stopped(stopped)?;
    preview_verification_status(verified.status)?;
    verify_preview_image(context)?;
    collector_stopped(stopped)?;
    let targets = client
        .discover_launch_targets(&candidate.settings)
        .map_err(|_| {
            error(
                "preview_verification_unavailable",
                "本次运行实例核验未完成，稍后重试",
            )
        })?;
    collector_stopped(stopped)?;
    verify_preview_instances(context, targets, stopped)
}
fn verify_preview_instances(
    context: &PreviewPairingContext,
    targets: Vec<Target>,
    stopped: &dyn Fn() -> bool,
) -> AppResult<()> {
    if targets.len() > 4096 {
        return Err(error(
            "preview_selection_mismatch",
            "运行实例清单超过核验限制",
        ));
    }
    let mut matching = Vec::new();
    for target in targets {
        collector_stopped(stopped)?;
        let path = target
            .full_image_path
            .as_deref()
            .ok_or_else(|| error("preview_selection_mismatch", "运行实例身份不完整"))?;
        let resolved = crate::platform::game_launch::resolve_windows_target(&context.bottle, path)
            .map_err(|_| error("preview_selection_mismatch", "无法解析运行实例身份"))?;
        let file = File::open(resolved)
            .map_err(|_| error("preview_selection_mismatch", "无法核对运行实例文件"))?;
        if preview_file_identity(&file)? == context.file_identity {
            matching.push(target);
        }
    }
    if !single_preview_instance(&matching, &context.candidate.target) {
        return Err(error(
            "preview_selection_mismatch",
            "同一程序有多个实例或目标身份已变化，请仅保留所需实例后重试",
        ));
    }
    collector_stopped(stopped)?;
    // Discovery and path checks may span an external probe. Recheck the
    // original canonical target after that work, including same-inode aliases.
    verify_preview_image(context)
}
type PreviewFileStamp = (u64, u64, u64, i64, i64, i64, i64);
pub(super) fn preview_file_stamp(file: &File) -> AppResult<PreviewFileStamp> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file
            .metadata()
            .map_err(|_| error("preview_selection_mismatch", "无法核对目标文件版本"))?;
        Ok((
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(error(
            "preview_selection_unsupported",
            "当前平台不支持窗口配对",
        ))
    }
}
pub(super) fn single_preview_instance(matching: &[Target], expected: &Target) -> bool {
    matching.len() == 1
        && matching[0].pid == expected.pid
        && matching[0].creation_filetime == expected.creation_filetime
}
pub(super) fn preview_verification_error(message: &str) -> AppError {
    if message.starts_with("window_proof_busy:") {
        error("preview_capture_busy", "游戏窗口正在核验或激活，请稍后重试")
    } else {
        error(
            "preview_verification_unavailable",
            "本次窗口核验未完成，稍后重试",
        )
    }
}
pub(super) fn preview_verification_status(status: hook_client::BoundWindowStatus) -> AppResult<()> {
    match status {
        hook_client::BoundWindowStatus::Verified => Ok(()),
        hook_client::BoundWindowStatus::Stale => {
            Err(error("preview_target_closed", "游戏窗口已退出或身份已变化"))
        }
        hook_client::BoundWindowStatus::Unavailable => Err(error(
            "preview_verification_unavailable",
            "本次窗口核验未完成，稍后重试",
        )),
    }
}
pub(super) fn preview_file_identity(file: &File) -> AppResult<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file
            .metadata()
            .map_err(|_| error("preview_selection_mismatch", "无法核对目标文件身份"))?;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(error(
            "preview_selection_unsupported",
            "当前平台不支持系统窗口配对",
        ))
    }
}
pub(super) fn window_title_proof_error(message: &str) -> AppError {
    if message
        .split_once(':')
        .is_some_and(|(code, _)| code == "window_title_proof_requires_unicode")
        || message == "window_title_proof_requires_unicode"
    {
        error(
            "preview_window_selection_required",
            "请通过系统选择对应的游戏窗口以预览；原标题未更改，游戏取文不受影响。",
        )
    } else {
        error(
            "no_capture_window",
            "无法完成窗口归属核验或恢复窗口标题，请重新核对",
        )
    }
}

impl CoreHookBackend {
    pub fn configure_preview_proof(
        &self,
        provider: std::sync::Weak<dyn PreviewProofProvider>,
    ) -> AppResult<()> {
        *self
            .preview_proof
            .lock()
            .map_err(|_| error("capture_state", "窗口核验状态不可用"))? = Some(provider);
        Ok(())
    }
    /// Freeze trusted local intent only. All external Win proof runs in worker.
    pub fn freeze_preview_target(&self, candidate_id: &str) -> AppResult<FrozenPreviewTarget> {
        let context = preview_context(candidate_id, self.preview_candidate(candidate_id)?)?;
        Ok(context.freeze(self.native_resource()?.manifest_sha256))
    }
    pub fn freeze_active_preview_target(
        &self,
        stopped: &dyn Fn() -> bool,
    ) -> AppResult<FrozenPreviewTarget> {
        collector_stopped(stopped)?;
        let id = self.preview_candidate_id_for_active_binding()?;
        let context = preview_context(&id, self.preview_candidate(&id)?)?;
        verify_preview_context(&self.client, &context, stopped)?;
        collector_stopped(stopped)?;
        if self.preview_candidate_id_for_active_binding()? != id {
            return Err(error("stale_target", "冻结期间关联已变化"));
        }
        Ok(context.freeze(self.native_resource()?.manifest_sha256))
    }
    pub fn prepare_preview_pairing(&self, candidate_id: &str) -> AppResult<PreviewPairingContext> {
        let context = preview_context(candidate_id, self.preview_candidate(candidate_id)?)?;
        self.verify_preview_pairing(&context)?;
        Ok(context)
    }
    /// Read-only checks run outside desktop/native locks, both before and after
    /// selection/capture. An incomplete inventory is never evidence of absence.
    pub fn verify_preview_pairing(&self, context: &PreviewPairingContext) -> AppResult<()> {
        let candidate = self.preview_candidate(&context.candidate_id)?;
        if candidate.selection_identity() != context.candidate.selection_identity() {
            return Err(error("stale_target", "游戏窗口身份已变化"));
        }
        verify_preview_context(&self.client, context, &|| false)
    }
    pub fn with_preview_window<T>(
        &self,
        candidate_id: &str,
        nonce: &str,
        observe: impl FnOnce() -> Result<T, String>,
    ) -> AppResult<T> {
        let candidate = self
            .candidates
            .lock()
            .map_err(|_| error("capture_state", "候选状态不可用"))?
            .get(candidate_id)
            .cloned();
        let candidate = match candidate {
            Some(c) => c,
            None => self
                .preview_binding
                .lock()
                .map_err(|_| error("capture_state", "关联状态不可用"))?
                .as_ref()
                .filter(|(id, _)| id == candidate_id)
                .map(|(_, c)| c.clone())
                .ok_or_else(|| error("stale_target", "候选窗口已过期，请重新选择"))?,
        };
        if candidate.selection_identity().as_deref() != Some(candidate_id) {
            return Err(error("stale_target", "候选窗口身份不一致"));
        }
        let window = candidate
            .window
            .ok_or_else(|| error("no_capture_window", "进程候选没有可核验的运行窗口"))?;
        self.client
            .with_window_title_challenge(&candidate.settings, &window, nonce, observe)
            .map_err(|message| window_title_proof_error(&message))
    }
}
