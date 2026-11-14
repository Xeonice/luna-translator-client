//! Verify pinned release bytes before any DLL is loaded or helper is executed.

use hook_contracts::Architecture;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

pub const ABI_PROFILE: &str = "lunahooks-core-abi1-cdecl-seven";
pub const PINNED_MANIFEST: &str = include_str!("../../../native/core-runtime.lock.json");

#[derive(Debug, Deserialize)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub abi_profile: String,
    pub commit: String,
    pub abi_version: u32,
    pub protocol_version: u16,
    pub files: Vec<RuntimeFile>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeFile {
    pub runtime_path: String,
    pub architecture: Architecture,
    pub sha256: String,
    pub size: u64,
}

pub struct VerifiedRuntime {
    pub directory: PathBuf,
    pub manifest: RuntimeManifest,
    // On Windows these handles deny write/delete sharing until the host exits.
    _locked_files: Vec<File>,
}

impl VerifiedRuntime {
    pub fn verify(directory: &Path) -> Result<Self, String> {
        Self::verify_manifest(directory, PINNED_MANIFEST)
    }

    fn verify_manifest(directory: &Path, manifest_json: &str) -> Result<Self, String> {
        let manifest: RuntimeManifest = serde_json::from_str(manifest_json)
            .map_err(|e| format!("invalid runtime lock: {e}"))?;
        if manifest.schema_version != 1
            || manifest.abi_profile != ABI_PROFILE
            || manifest.abi_version != 1
            || manifest.protocol_version != 2
            || manifest.commit != "e9242ebac32a09e911f306b7c90714891c217302"
        {
            return Err("unsupported runtime lock schema or ABI profile".into());
        }
        let directory = directory
            .canonicalize()
            .map_err(|e| format!("runtime directory: {e}"))?;
        let mut names = HashSet::new();
        let mut locked_files = Vec::new();
        for entry in manifest
            .files
            .iter()
            .filter(|entry| entry.architecture == host_architecture())
        {
            if entry.runtime_path.contains(['/', '\\'])
                || entry.runtime_path == "."
                || entry.runtime_path == ".."
                || !names.insert(entry.runtime_path.as_str())
            {
                return Err("invalid or duplicate runtime file name in lock".into());
            }
            let path = directory.join(&entry.runtime_path);
            let link_metadata = path
                .symlink_metadata()
                .map_err(|e| format!("{}: {e}", entry.runtime_path))?;
            if link_metadata.file_type().is_symlink() {
                return Err(format!("{} is a symbolic link", entry.runtime_path));
            }
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // FILE_SHARE_READ: prevent replacement between hash verification and loading.
                // FILE_FLAG_OPEN_REPARSE_POINT: inspect the opened object,
                // rather than following a mutable link after verification.
                options.share_mode(1).custom_flags(0x0020_0000);
            }
            let mut file = options
                .open(&path)
                .map_err(|e| format!("{}: {e}", entry.runtime_path))?;
            let metadata = file.metadata().map_err(|e| e.to_string())?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(format!("{} is a reparse point", entry.runtime_path));
                }
            }
            if !metadata.is_file() || metadata.len() != entry.size {
                return Err(format!("{} size/type mismatch", entry.runtime_path));
            }
            let mut hasher = Sha256::new();
            io::copy(&mut file, &mut HashWriter(&mut hasher)).map_err(|e| e.to_string())?;
            let hash = format!("{:x}", hasher.finalize());
            if hash != entry.sha256 {
                return Err(format!("{} SHA-256 mismatch", entry.runtime_path));
            }
            locked_files.push(file);
        }
        let required = match host_architecture() {
            Architecture::X86 => [
                "LunaHostCore32.dll",
                "LunaHookCore32.dll",
                "LunaInjectCore32.exe",
            ],
            Architecture::X64 => [
                "LunaHostCore64.dll",
                "LunaHookCore64.dll",
                "LunaInjectCore64.exe",
            ],
        };
        if names.len() != required.len() {
            return Err("Core runtime must contain exactly the pinned three-file set".into());
        }
        for required in required {
            if !names.contains(required) {
                return Err(format!("runtime lock missing {required}"));
            }
        }
        for entry in std::fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name();
            if !names.contains(name.to_string_lossy().as_ref()) {
                return Err(format!(
                    "unexpected file in pinned Core runtime: {}",
                    name.to_string_lossy()
                ));
            }
        }
        Ok(Self {
            directory,
            manifest,
            _locked_files: locked_files,
        })
    }

    pub fn host_path(&self) -> PathBuf {
        self.path(match host_architecture() {
            Architecture::X86 => "LunaHostCore32.dll",
            Architecture::X64 => "LunaHostCore64.dll",
        })
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }
}

/// Core's pure LoadLibrary injector uses conventional zero-on-success, unlike
/// the removed upstream multipurpose helper. This proves file loading only.
pub fn injector_load_confirmed(exit_code: Option<i32>) -> bool {
    exit_code == Some(0)
}

pub fn host_architecture() -> Architecture {
    if cfg!(target_arch = "x86") {
        Architecture::X86
    } else {
        Architecture::X64
    }
}

struct HashWriter<'a>(&'a mut Sha256);

impl io::Write for HashWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn core_injector_exit_codes_do_not_reuse_legacy_success_conventions() {
        assert!(injector_load_confirmed(Some(0)));
        for failure in [None, Some(1), Some(2), Some(3), Some(-1)] {
            assert!(!injector_load_confirmed(failure));
        }
    }

    #[test]
    fn pinned_manifest_is_recognized() {
        let manifest: RuntimeManifest = serde_json::from_str(PINNED_MANIFEST).expect("pinned lock");
        assert_eq!(manifest.abi_profile, ABI_PROFILE);
        assert_eq!(manifest.commit, "e9242ebac32a09e911f306b7c90714891c217302");
        assert!(manifest.files.len() == 6);
    }

    #[test]
    fn legacy_profile_cannot_initialize_the_new_core_boundary() {
        let path = std::env::temp_dir();
        let legacy = include_str!("../../../native/upstream.lock.json");
        assert!(
            VerifiedRuntime::verify_manifest(&path, legacy)
                .err()
                .unwrap()
                .contains("runtime lock")
        );
    }

    #[test]
    fn untrusted_bytes_are_rejected_before_loading() {
        let path = std::env::temp_dir().join(format!(
            "lunahook-hash-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("test directory");
        fs::write(path.join("LunaHostCore64.dll"), b"wrong").expect("fixture bytes");
        let manifest = serde_json::json!({
            "schema_version": 1, "abi_profile": ABI_PROFILE, "commit": "e9242ebac32a09e911f306b7c90714891c217302", "abi_version":1,"protocol_version":2,
            "files": [{"runtime_path":"LunaHostCore64.dll","architecture":"x64","size":5,"sha256":"0".repeat(64)}]
        });
        let result = VerifiedRuntime::verify_manifest(&path, &manifest.to_string());
        assert!(
            result
                .err()
                .expect("must reject")
                .contains("SHA-256 mismatch")
        );
        fs::remove_dir_all(path).expect("clean fixture");
    }

    #[cfg(unix)]
    #[test]
    fn runtime_dll_cannot_be_a_mutable_symbolic_link() {
        use std::os::unix::fs::symlink;
        let path = std::env::temp_dir().join(format!(
            "lunahook-link-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("test directory");
        fs::write(path.join("actual.dll"), b"verified bytes").expect("target bytes");
        symlink(path.join("actual.dll"), path.join("LunaHostCore64.dll")).expect("fixture symlink");
        let manifest = serde_json::json!({
            "schema_version": 1, "abi_profile": ABI_PROFILE, "commit": "e9242ebac32a09e911f306b7c90714891c217302", "abi_version":1,"protocol_version":2,
            "files": [{"runtime_path":"LunaHostCore64.dll","architecture":"x64","size":14,"sha256":format!("{:x}", Sha256::digest(b"verified bytes"))}]
        });
        let error = VerifiedRuntime::verify_manifest(&path, &manifest.to_string())
            .err()
            .expect("must reject symlink");
        assert!(error.contains("symbolic link"), "{error}");
        fs::remove_dir_all(path).expect("clean fixture");
    }
}
