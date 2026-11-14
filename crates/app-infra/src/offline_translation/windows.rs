use super::*;
use resources::PeArchitecture;
pub const UPSTREAM_ABI_COMMIT: &str = "31b502ebf34c35d27276a3d5aaf58ca7b45f0f3b";
pub const JBEIJING_ABI: &str = "lunatranslator-jbj7-unicode-v1";
pub const KINGSOFT_ABI: &str = "lunatranslator-fastait-unicode-v1";
pub fn dll_relative(engine: OfflineEngine, pair: &LanguagePair) -> OfflineResult<String> {
    match engine {
        OfflineEngine::Jbeijing
            if pair.source == "ja"
                && matches!(pair.target.as_str(), "zh" | "zh-Hans" | "zh-Hant") =>
        {
            Ok("JBJCT.dll".into())
        }
        OfflineEngine::KingsoftFastait => {
            fn lang(s: &str) -> Option<&'static str> {
                match s {
                    "ja" => Some("Japanese"),
                    "zh" | "zh-Hans" => Some("SChinese"),
                    "zh-Hant" => Some("TChinese"),
                    "en" => Some("English"),
                    _ => None,
                }
            }
            let (a, b) = (lang(&pair.source), lang(&pair.target));
            match (a, b) {
                (Some(a), Some(b)) if a != b => Ok(format!("GTS/{a}{b}")),
                _ => Err(OfflineFailure::new(
                    OfflineErrorCode::UnsupportedPair,
                    "dll_pair",
                )),
            }
        }
        _ => Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedPair,
            "dll_pair",
        )),
    }
}
pub fn validate(
    engine: OfflineEngine,
    manifest: &ResourceManifest,
    launch: &WorkerLaunch,
) -> OfflineResult<()> {
    let worker = match launch {
        WorkerLaunch::Native { executable, .. } if cfg!(windows) => executable,
        WorkerLaunch::CrossOver { worker, .. } if cfg!(target_os = "macos") => worker,
        _ => {
            return Err(OfflineFailure::new(
                OfflineErrorCode::UnsupportedPlatform,
                "windows_runtime",
            ));
        }
    };
    let pair = &manifest.route[0].pair;
    let relative = dll_relative(engine, pair)?;
    let dll = if engine == OfflineEngine::Jbeijing {
        relative
    } else {
        let candidates = manifest
            .files
            .iter()
            .filter(|f| {
                f.relative_path.starts_with(&(relative.clone() + "/"))
                    && f.relative_path.to_ascii_lowercase().ends_with(".dll")
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidResource,
                "ambiguous_engine_dll",
            ));
        }
        candidates[0].relative_path.clone()
    };
    if !manifest.files.iter().any(|f| f.relative_path == dll) {
        return Err(OfflineFailure::new(
            OfflineErrorCode::MissingResource,
            "engine_dll",
        ));
    }
    let dll_path = resources::relative_path(&manifest.root, &dll)?;
    let arch = resources::pe_architecture(&dll_path)?;
    if arch != resources::pe_architecture(worker)? || arch != PeArchitecture::X86 {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ArchitectureMismatch,
            "worker_dll_architecture",
        ));
    }
    let expected = if engine == OfflineEngine::Jbeijing {
        JBEIJING_ABI
    } else {
        KINGSOFT_ABI
    };
    if manifest.engine_version != expected {
        return Err(OfflineFailure::new(
            OfflineErrorCode::RuntimeIncompatible,
            "dll_abi_contract",
        ));
    }
    Ok(())
}
