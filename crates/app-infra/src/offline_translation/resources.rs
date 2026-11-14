//! Bounded inspection of explicitly selected local resources, without discovery/install/download.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};
// Conservative observation budget, not an assertion of any vendor package size.
pub const MAX_RESOURCE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub fn is_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn relative_path(root: &Path, relative: &str) -> OfflineResult<PathBuf> {
    let p = Path::new(relative);
    if relative.is_empty()
        || relative.len() > 4096
        || relative.contains(['\\', '\0'])
        || relative
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_path",
        ));
    }
    let canonical = root
        .canonicalize()
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::NotInstalled, "resource_root"))?;
    if !canonical.is_dir() || canonical != root {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "canonical_root",
        ));
    }
    let mut candidate = root.to_owned();
    for component in p.components() {
        candidate.push(component.as_os_str());
        let meta = std::fs::symlink_metadata(&candidate)
            .map_err(|_| OfflineFailure::new(OfflineErrorCode::MissingResource, "resource_file"))?;
        if meta.file_type().is_symlink() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "resource_symlink",
            ));
        }
    }
    let actual = candidate
        .canonicalize()
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::MissingResource, "resource_file"))?;
    if !actual.starts_with(root) {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_escape",
        ));
    }
    Ok(actual)
}
pub fn hash_file(path: &Path, budget: u64) -> OfflineResult<(u64, String)> {
    hash_file_checked(path, budget, &|| Ok(()))
}
/// Final component is opened without following a symlink/reparse point and
/// without waiting for a FIFO writer. This is not a dirfd capability walk:
/// ancestor-directory replacement remains outside this leaf assurance.
fn open_regular(path: &Path) -> OfflineResult<File> {
    let selected = std::fs::symlink_metadata(path)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::MissingResource, "resource_metadata"))?;
    if !selected.is_file() || selected.file_type().is_symlink() {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_file_type",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use ::windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0);
    }
    let file = options
        .open(path)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::MissingResource, "resource_open"))?;
    let meta = file
        .metadata()
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_metadata"))?;
    if !meta.is_file() {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_file_type",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (selected.dev(), selected.ino()) != (meta.dev(), meta.ino()) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceChanged,
                "resource_open_identity",
            ));
        }
    }
    #[cfg(windows)]
    {
        use ::windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "resource_reparse_point",
            ));
        }
    }
    Ok(file)
}
pub(super) fn hash_file_checked(
    path: &Path,
    budget: u64,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<(u64, String)> {
    let (length, digest, _) = hash_regular_checked(path, budget, check, false)?;
    Ok((length, digest))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PhysicalFileIdentity(u64, u64);
fn physical_file_identity(file: &File) -> OfflineResult<PhysicalFileIdentity> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata().map_err(|_| {
            OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_file_identity")
        })?;
        Ok(PhysicalFileIdentity(metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use ::windows::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };
        use std::os::windows::io::AsRawHandle;
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // The borrowed OS handle remains owned and open for this entire call.
        unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) }
            .map_err(|_| {
                OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_file_identity")
            })?;
        Ok(PhysicalFileIdentity(
            u64::from(information.dwVolumeSerialNumber),
            (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedPlatform,
            "resource_file_identity",
        ))
    }
}
fn hash_regular_checked(
    path: &Path,
    budget: u64,
    check: &dyn Fn() -> OfflineResult<()>,
    physical_identity: bool,
) -> OfflineResult<(u64, String, Option<PhysicalFileIdentity>)> {
    check()?;
    let mut file = open_regular(path)?;
    let meta = file
        .metadata()
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_metadata"))?;
    if !meta.is_file() || meta.len() > budget {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceBudgetExceeded,
            "resource_budget",
        ));
    }
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    let mut total = 0u64;
    loop {
        check()?;
        let count = file
            .read(&mut bytes)
            .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_read"))?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).ok_or_else(|| {
            OfflineFailure::new(OfflineErrorCode::ResourceBudgetExceeded, "resource_budget")
        })?;
        if total > budget {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceBudgetExceeded,
                "resource_budget",
            ));
        }
        hash.update(&bytes[..count]);
    }
    if total != meta.len() {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceChanged,
            "resource_length",
        ));
    }
    let identity = physical_identity
        .then(|| physical_file_identity(&file))
        .transpose()?;
    check()?;
    Ok((total, format!("{:x}", hash.finalize()), identity))
}
/// Bounded control-material read. No preallocation follows an untrusted file
/// length; regular/no-follow checks and the cooperative budget apply throughout.
pub(super) fn read_bounded_checked(
    path: &Path,
    maximum: usize,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<Vec<u8>> {
    check()?;
    let mut file = open_regular(path)?;
    let length = file
        .metadata()
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "control_metadata"))?
        .len();
    if length > maximum as u64 {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceBudgetExceeded,
            "control_budget",
        ));
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        check()?;
        let read = file
            .read(&mut chunk)
            .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "control_read"))?;
        if read == 0 {
            break;
        }
        if bytes.len().checked_add(read).is_none_or(|n| n > maximum) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceBudgetExceeded,
                "control_budget",
            ));
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    check()?;
    if bytes.len() as u64 != length {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceChanged,
            "control_length",
        ));
    }
    Ok(bytes)
}
/// Content identity excludes absolute location; moving identical resources requires a new grant,
/// but does not invent a new package version. Caller owns repository snapshot identity.
pub fn manifest_digest(
    files: &[ResourceFile],
    route: &[RouteHop],
    engine_version: &str,
    identity: &ResourceIdentity,
) -> OfflineResult<String> {
    let value = serde_json::to_value((files, route, engine_version, identity))
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "manifest"))?;
    let data = serde_json::to_vec(&value)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "manifest"))?;
    Ok(format!("{:x}", Sha256::digest(&data)))
}
pub fn verify_manifest(manifest: &ResourceManifest, engine: OfflineEngine) -> OfflineResult<()> {
    verify_manifest_checked(manifest, engine, &|| Ok(()))
}
pub(super) fn verify_manifest_checked(
    manifest: &ResourceManifest,
    engine: OfflineEngine,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<()> {
    check()?;
    if manifest.files.len() > MAX_RESOURCE_FILES
        || manifest.engine_version.is_empty()
        || manifest.engine_version.len() > 256
        || !is_digest(&manifest.digest)
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "manifest",
        ));
    }
    if manifest.route.is_empty()
        || manifest.route.len() > MAX_ROUTE_HOPS
        || manifest
            .route
            .windows(2)
            .any(|h| h[0].pair.target != h[1].pair.source)
        || manifest.route.iter().any(|h| {
            [&h.pair.source, &h.pair.target].iter().any(|l| {
                l.is_empty()
                    || l.len() > 64
                    || !l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
        })
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_route",
        ));
    }
    let id = &manifest.identity;
    if [
        &id.snapshot_id,
        &id.route_id,
        &id.observation_id,
        &id.runtime_profile_id,
        &id.installation_id,
    ]
    .iter()
    .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        || id.runtime_profile_revision == 0
        || id.installation_revision == 0
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_identity",
        ));
    }
    if manifest.files.is_empty() && engine != OfflineEngine::AppleTranslation {
        return Err(OfflineFailure::new(
            OfflineErrorCode::MissingResource,
            "resource_files",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for file in &manifest.files {
        check()?;
        if !seen.insert(&file.relative_path) || !is_digest(&file.sha256) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "resource_manifest",
            ));
        }
        total = total.checked_add(file.length).ok_or_else(|| {
            OfflineFailure::new(OfflineErrorCode::ResourceBudgetExceeded, "resource_budget")
        })?;
        if total > MAX_RESOURCE_BYTES {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceBudgetExceeded,
                "resource_budget",
            ));
        }
        let path = relative_path(&manifest.root, &file.relative_path)?;
        let (length, hash) = hash_file_checked(&path, file.length, check)?;
        if length != file.length || hash != file.sha256 {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceChanged,
                "resource_digest",
            ));
        }
    }
    if manifest_digest(
        &manifest.files,
        &manifest.route,
        &manifest.engine_version,
        &manifest.identity,
    )? != manifest.digest
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceChanged,
            "manifest_digest",
        ));
    }
    Ok(())
}
pub fn verify_executable(path: &Path, expected: &str) -> OfflineResult<()> {
    verify_executable_checked(path, expected, &|| Ok(()))
}
pub(super) fn verify_executable_checked(
    path: &Path,
    expected: &str,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<()> {
    check()?;
    if !path.is_absolute() || !is_digest(expected) {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "worker_path",
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::NotInstalled, "worker_path"))?;
    if canonical != path {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "worker_canonical_path",
        ));
    }
    if hash_file_checked(path, 512 * 1024 * 1024, check)?.1 != expected {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceChanged,
            "worker_digest",
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeArchitecture {
    X86,
    X64,
    Arm64,
}
pub fn pe_architecture(path: &Path) -> OfflineResult<PeArchitecture> {
    let mut file = open_regular(path)?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "pe_header"))?;
    if &dos[..2] != b"MZ" {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "pe_header",
        ));
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap());
    if offset > 4 * 1024 * 1024 {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "pe_offset",
        ));
    }
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "pe_offset"))?;
    let mut signature = [0u8; 6];
    file.read_exact(&mut signature)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::InvalidResource, "pe_signature"))?;
    if &signature[..4] != b"PE\0\0" {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "pe_signature",
        ));
    }
    match u16::from_le_bytes(signature[4..6].try_into().unwrap()) {
        0x14c => Ok(PeArchitecture::X86),
        0x8664 => Ok(PeArchitecture::X64),
        0xaa64 => Ok(PeArchitecture::Arm64),
        _ => Err(OfflineFailure::new(
            OfflineErrorCode::ArchitectureMismatch,
            "pe_machine",
        )),
    }
}

/// Hash only explicitly selected trees beneath the user's granted root. This
/// synchronous entry is for blocking callers. Async owners must share ResourceInspector.
/// Facts returned here never grant runtime readiness or a Repository lease.
pub fn inspect_selected_trees(
    root: &Path,
    selected: &[String],
) -> OfflineResult<Vec<ResourceFile>> {
    inspect_selected_trees_checked(root, selected, InspectionLimits::default(), &|| Ok(()))
}

pub use super::inspection::{InspectionLimits, InspectionRequest, ResourceInspector};

pub(super) fn inspect_selected_trees_checked(
    root: &Path,
    selected: &[String],
    limits: InspectionLimits,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<Vec<ResourceFile>> {
    limits.validate()?;
    check()?;
    if selected.is_empty() || selected.len() > limits.max_selected_trees {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "selected_resource_trees",
        ));
    }
    if root.to_str().is_none() {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "resource_path_encoding",
        ));
    }
    // This list is at most the selected-root limit. There is no fan-out queue:
    // one ReadDir cursor per bounded recursion depth is kept at any time.
    let mut roots = Vec::<PathBuf>::with_capacity(selected.len());
    let mut entry_count = 0usize;
    for relative in selected {
        charge_entry(&mut entry_count, limits, 0, check)?;
        let path = relative_path(root, relative)?;
        if roots
            .iter()
            .any(|other| path.starts_with(other) || other.starts_with(&path))
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "duplicate_or_overlapping_selection",
            ));
        }
        roots.push(path);
    }
    let mut scan = TreeScan {
        root,
        limits,
        entries: entry_count,
        files: std::collections::BTreeMap::new(),
        total_bytes: 0,
        check,
        storage_keys: false,
        physical_files: BTreeSet::new(),
    };
    for path in roots {
        scan.visit(path, 0)?;
    }
    check()?;
    Ok(scan.files.into_values().collect())
}
/// A complete explicitly selected root, including the control file. This is
/// material inspection only; it does not know which files a loader can access.
pub(super) fn inspect_runtime_root_checked(
    root: &Path,
    limits: InspectionLimits,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<Vec<ResourceFile>> {
    limits.validate()?;
    check()?;
    if root.to_str().is_none() || !root.is_absolute() {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "canonical_root",
        ));
    }
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::NotInstalled, "resource_root"))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || root.canonicalize().ok().as_deref() != Some(root)
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "canonical_root",
        ));
    }
    let mut scan = TreeScan {
        root,
        limits,
        entries: 0,
        files: std::collections::BTreeMap::new(),
        total_bytes: 0,
        check,
        storage_keys: true,
        physical_files: BTreeSet::new(),
    };
    let mut children = std::fs::read_dir(root).map_err(|_| {
        OfflineFailure::new(OfflineErrorCode::PermissionDenied, "resource_directory")
    })?;
    loop {
        check()?;
        let Some(entry) = children.next() else {
            break;
        };
        charge_entry(&mut scan.entries, limits, 0, check)?;
        let entry = entry.map_err(|_| {
            OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_entry")
        })?;
        scan.visit(entry.path(), 0)?;
    }
    check()?;
    Ok(scan.files.into_values().collect())
}
fn charge_entry(
    entries: &mut usize,
    limits: InspectionLimits,
    depth: usize,
    check: &dyn Fn() -> OfflineResult<()>,
) -> OfflineResult<()> {
    check()?;
    if depth > limits.max_depth {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceBudgetExceeded,
            "resource_tree_depth",
        ));
    }
    *entries = entries.checked_add(1).ok_or_else(|| {
        OfflineFailure::new(
            OfflineErrorCode::ResourceBudgetExceeded,
            "resource_tree_budget",
        )
    })?;
    if *entries > limits.max_entries {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceBudgetExceeded,
            "resource_tree_budget",
        ));
    }
    Ok(())
}
struct TreeScan<'a> {
    root: &'a Path,
    limits: InspectionLimits,
    entries: usize,
    files: std::collections::BTreeMap<String, ResourceFile>,
    total_bytes: u64,
    check: &'a dyn Fn() -> OfflineResult<()>,
    storage_keys: bool,
    physical_files: BTreeSet<PhysicalFileIdentity>,
}
impl TreeScan<'_> {
    fn visit(&mut self, path: PathBuf, depth: usize) -> OfflineResult<()> {
        (self.check)()?;
        let relative = self.relative_key(&path)?;
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| OfflineFailure::new(OfflineErrorCode::MissingResource, "resource_tree"))?;
        (self.check)()?;
        if metadata.file_type().is_symlink() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "resource_symlink",
            ));
        }
        #[cfg(windows)]
        {
            use ::windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidResource,
                    "resource_reparse_point",
                ));
            }
        }
        if metadata.is_dir() {
            let mut directory = std::fs::read_dir(&path).map_err(|_| {
                OfflineFailure::new(OfflineErrorCode::PermissionDenied, "resource_directory")
            })?;
            loop {
                (self.check)()?;
                let Some(entry) = directory.next() else {
                    break;
                };
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    OfflineFailure::new(
                        OfflineErrorCode::ResourceBudgetExceeded,
                        "resource_tree_depth",
                    )
                })?;
                // Charge immediately on enumeration, before retaining or visiting a child.
                charge_entry(&mut self.entries, self.limits, child_depth, self.check)?;
                let entry = entry.map_err(|_| {
                    OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_entry")
                })?;
                self.visit(entry.path(), child_depth)?;
            }
        } else if metadata.is_file() {
            if self.files.len() >= self.limits.max_files {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::ResourceBudgetExceeded,
                    "resource_file_count",
                ));
            }
            let remaining = self
                .limits
                .max_total_bytes
                .checked_sub(self.total_bytes)
                .ok_or_else(|| {
                    OfflineFailure::new(OfflineErrorCode::ResourceBudgetExceeded, "resource_budget")
                })?;
            let (length, sha256, identity) =
                hash_regular_checked(&path, remaining, self.check, self.storage_keys)?;
            if identity.is_some_and(|id| !self.physical_files.insert(id)) {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidResource,
                    "resource_file_alias",
                ));
            }
            self.total_bytes = self.total_bytes.checked_add(length).ok_or_else(|| {
                OfflineFailure::new(OfflineErrorCode::ResourceBudgetExceeded, "resource_budget")
            })?;
            if self
                .files
                .insert(
                    relative.clone(),
                    ResourceFile {
                        relative_path: relative,
                        sha256,
                        length,
                    },
                )
                .is_some()
            {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidResource,
                    "duplicate_resource_entry",
                ));
            }
        } else {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "resource_file_type",
            ));
        }
        (self.check)()
    }
    fn relative_key(&self, path: &Path) -> OfflineResult<String> {
        let relative = path.strip_prefix(self.root).map_err(|_| {
            OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_escape")
        })?;
        let mut parts = Vec::new();
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidResource,
                    "resource_path",
                ));
            };
            let part = component.to_str().ok_or_else(|| {
                OfflineFailure::new(OfflineErrorCode::InvalidResource, "resource_path_encoding")
            })?;
            if part.is_empty() || part.contains(['\\', '\0']) {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::InvalidResource,
                    "resource_path",
                ));
            }
            parts.push(part);
        }
        let key = parts.join("/");
        if key.is_empty()
            || key.len() > 4096
            || (self.storage_keys
                && app_core::domain::offline_runtime::validate_relative_key(&key).is_err())
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "resource_path",
            ));
        }
        Ok(key)
    }
}
