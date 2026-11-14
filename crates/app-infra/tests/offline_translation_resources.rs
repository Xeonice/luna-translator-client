use app_infra::offline_translation::{resources::*, *};
use std::path::PathBuf;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "luna-offline-resource-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p.canonicalize().unwrap())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn manifest(root: PathBuf) -> ResourceManifest {
    let f = root.join("model.bin");
    std::fs::write(&f, b"resource-only fixture").unwrap();
    let (length, sha256) = hash_file(&f, 1000).unwrap();
    let files = vec![ResourceFile {
        relative_path: "model.bin".into(),
        sha256,
        length,
    }];
    let route = vec![RouteHop {
        pair: app_core::domain::model_config::service_graph::LanguagePair {
            source: "ja".into(),
            target: "zh".into(),
        },
        package_relative_path: None,
    }];
    let engine_version = "fixture-v1".to_string();
    ResourceManifest {
        root,
        identity: ResourceIdentity {
            snapshot_id: "s".into(),
            route_id: "r".into(),
            observation_id: "o".into(),
            runtime_profile_id: "p".into(),
            runtime_profile_revision: 1,
            installation_revision: 1,
            installation_id: "i".into(),
        },
        digest: manifest_digest(
            &files,
            &route,
            &engine_version,
            &ResourceIdentity {
                snapshot_id: "s".into(),
                route_id: "r".into(),
                observation_id: "o".into(),
                runtime_profile_id: "p".into(),
                runtime_profile_revision: 1,
                installation_revision: 1,
                installation_id: "i".into(),
            },
        )
        .unwrap(),
        files,
        route,
        engine_version,
    }
}
#[test]
fn actual_files_are_hashed_and_replacement_is_rejected() {
    let tmp = Temp::new();
    let m = manifest(tmp.0.clone());
    verify_manifest(&m, OfflineEngine::ArgosTranslate).unwrap();
    std::fs::write(tmp.0.join("model.bin"), b"changed").unwrap();
    assert_eq!(
        verify_manifest(&m, OfflineEngine::ArgosTranslate)
            .unwrap_err()
            .code,
        OfflineErrorCode::ResourceChanged
    );
}
#[test]
fn traversal_and_duplicate_resource_facts_are_rejected() {
    let tmp = Temp::new();
    for path in ["../x", "/tmp/x", "a/../b", "a\\b"] {
        assert!(relative_path(&tmp.0, path).is_err());
    }
    let mut m = manifest(tmp.0.clone());
    m.files.push(m.files[0].clone());
    assert!(verify_manifest(&m, OfflineEngine::ArgosTranslate).is_err());
}
#[cfg(unix)]
#[test]
fn symlink_escape_is_not_a_selected_resource() {
    let tmp = Temp::new();
    std::os::unix::fs::symlink("/etc/hosts", tmp.0.join("escape")).unwrap();
    assert_eq!(
        relative_path(&tmp.0, "escape").unwrap_err().code,
        OfflineErrorCode::InvalidResource
    );
}
#[test]
fn manifest_digest_is_location_independent_and_route_sensitive() {
    let a = Temp::new();
    let b = Temp::new();
    let m = manifest(a.0.clone());
    let n = manifest(b.0.clone());
    assert_eq!(m.digest, n.digest);
    let mut route = m.route.clone();
    route[0].pair.target = "en".into();
    assert_ne!(
        m.digest,
        manifest_digest(&m.files, &route, &m.engine_version, &m.identity).unwrap()
    );
}
#[test]
fn malformed_and_unsupported_pe_do_not_claim_architecture() {
    let tmp = Temp::new();
    let path = tmp.0.join("worker.exe");
    std::fs::write(&path, b"MZ").unwrap();
    assert!(pe_architecture(&path).is_err());
    let mut data = vec![0u8; 70];
    data[..2].copy_from_slice(b"MZ");
    data[60..64].copy_from_slice(&64u32.to_le_bytes());
    data[64..68].copy_from_slice(b"PE\0\0");
    data[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
    std::fs::write(&path, &data).unwrap();
    assert_eq!(pe_architecture(&path).unwrap(), PeArchitecture::X86);
    data[68..70].copy_from_slice(&0x9999u16.to_le_bytes());
    std::fs::write(&path, data).unwrap();
    assert_eq!(
        pe_architecture(&path).unwrap_err().code,
        OfflineErrorCode::ArchitectureMismatch
    );
}
#[test]
fn maximum_resource_lengths_do_not_overflow() {
    let tmp = Temp::new();
    let mut m = manifest(tmp.0.clone());
    m.files[0].length = u64::MAX;
    assert_eq!(
        verify_manifest(&m, OfflineEngine::ArgosTranslate)
            .unwrap_err()
            .code,
        OfflineErrorCode::ResourceBudgetExceeded
    );
}
#[test]
fn selected_tree_inspection_produces_real_sorted_content_facts_only() {
    let tmp = Temp::new();
    std::fs::create_dir(tmp.0.join("selected")).unwrap();
    std::fs::write(tmp.0.join("selected/b"), b"b").unwrap();
    std::fs::write(tmp.0.join("selected/a"), b"aa").unwrap();
    std::fs::write(tmp.0.join("unselected-secret"), b"must not scan").unwrap();
    let files = inspect_selected_trees(&tmp.0, &["selected".into()]).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["selected/a", "selected/b"]
    );
    assert_eq!(files[0].length, 2);
    assert!(inspect_selected_trees(&tmp.0, &["../".into()]).is_err());
}

#[test]
fn empty_route_and_malformed_relative_components_are_rejected_before_engine_access() {
    let tmp = Temp::new();
    let mut m = manifest(tmp.0.clone());
    m.route.clear();
    assert_eq!(
        verify_manifest(&m, OfflineEngine::KingsoftFastait)
            .unwrap_err()
            .stage,
        "resource_route"
    );
    for relative in ["a/./b", "a//b", "a/."] {
        assert!(relative_path(&tmp.0, relative).is_err());
    }
}

#[cfg(unix)]
#[test]
fn special_files_are_rejected_before_opening_a_blocking_pipe() {
    let tmp = Temp::new();
    let pipe = tmp.0.join("pipe");
    let status = std::process::Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        hash_file(&pipe, 1000).unwrap_err().stage,
        "resource_file_type"
    );
}

#[cfg(unix)]
#[test]
fn pe_special_file_is_rejected_without_waiting_for_a_writer() {
    const CHILD: &str = "LUNA_PE_SPECIAL_FILE_TEST_CHILD";
    if let Some(path) = std::env::var_os(CHILD) {
        assert_eq!(
            pe_architecture(std::path::Path::new(&path))
                .unwrap_err()
                .stage,
            "resource_file_type"
        );
        return;
    }
    let tmp = Temp::new();
    let pipe = tmp.0.join("pe-pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap()
            .success()
    );
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "pe_special_file_is_rejected_without_waiting_for_a_writer",
            "--nocapture",
        ])
        .env(CHILD, &pipe)
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("PE inspection waited on a FIFO writer instead of rejecting a non-regular file");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
