use super::*;
pub const ARGOS_VERSION: &str = "1.11.0";
pub const PIPELINE: &str = "argos-1.11.0-stanza-local-v1";
pub fn validate(manifest: &ResourceManifest, launch: &WorkerLaunch) -> OfflineResult<()> {
    if !matches!(launch, WorkerLaunch::Argos { .. }) || manifest.engine_version != ARGOS_VERSION {
        return Err(OfflineFailure::new(
            OfflineErrorCode::RuntimeIncompatible,
            "argos_runtime",
        ));
    }
    for hop in &manifest.route {
        let package = hop.package_relative_path.as_deref().ok_or_else(|| {
            OfflineFailure::new(OfflineErrorCode::MissingResource, "argos_package")
        })?;
        let dir = resources::relative_path(&manifest.root, package)?;
        if !dir.is_dir() {
            return Err(OfflineFailure::new(
                OfflineErrorCode::MissingResource,
                "argos_package",
            ));
        }
        for required in ["metadata.json", "model/model.bin", "stanza/resources.json"] {
            let relative = format!("{package}/{required}");
            if !manifest.files.iter().any(|f| f.relative_path == relative) {
                return Err(OfflineFailure::new(
                    OfflineErrorCode::MissingResource,
                    "argos_pipeline_resource",
                ));
            }
        }
        if !manifest.files.iter().any(|f| {
            f.relative_path == format!("{package}/sentencepiece.model")
                || f.relative_path == format!("{package}/bpe.model")
        }) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::MissingResource,
                "argos_tokenizer",
            ));
        }
    }
    Ok(())
}
