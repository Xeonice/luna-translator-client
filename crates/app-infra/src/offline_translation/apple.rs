use super::*;
/// installedSource + explicit traditional-model strategy are only jointly available on 26.4+.
pub const MIN_MACOS: (u32, u32) = (26, 4);
pub const STRATEGY: &str = "low_latency";
pub fn validate(manifest: &ResourceManifest, launch: &WorkerLaunch) -> OfflineResult<()> {
    if !cfg!(target_os = "macos") {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedPlatform,
            "apple_platform",
        ));
    }
    if !matches!(launch, WorkerLaunch::Native { .. })
        || manifest.route.len() != 1
        || !manifest.files.is_empty()
        || manifest.route[0].package_relative_path.is_some()
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "apple_system_resource",
        ));
    }
    // System language package hashes are not exposed: engine_version is the observed OS build.
    // Helper independently checks that exact build and same-strategy availability at execution.
    Ok(())
}
