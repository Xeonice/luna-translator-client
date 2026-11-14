//! Actual filesystem inspection; no engine worker or translator is launched.
use app_infra::offline_translation::{
    OfflineErrorCode,
    resources::{InspectionLimits, InspectionRequest, ResourceInspector, inspect_selected_trees},
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::watch;
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("luna-resource-inspection-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn request(&self, selected: &[&str]) -> InspectionRequest {
        InspectionRequest {
            root: self.0.clone(),
            selected: selected.iter().map(|s| (*s).into()).collect(),
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn limits(entries: usize, files: usize, depth: usize, bytes: u64) -> InspectionLimits {
    InspectionLimits {
        max_entries: entries,
        max_files: files,
        max_depth: depth,
        max_total_bytes: bytes,
        ..InspectionLimits::default()
    }
}
async fn inspect(
    tree: &Tree,
    selected: &[&str],
    bounds: InspectionLimits,
) -> app_infra::offline_translation::OfflineResult<Vec<app_infra::offline_translation::ResourceFile>>
{
    let (_tx, rx) = watch::channel(false);
    ResourceInspector::with_limits(bounds)
        .unwrap()
        .inspect(tree.request(selected), Duration::from_secs(5), rx)
        .await
}
#[tokio::test]
async fn selected_root_and_each_real_child_share_one_entry_budget() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("wide")).unwrap();
    for n in 0..4 {
        std::fs::write(tree.0.join("wide").join(n.to_string()), b"x").unwrap();
    }
    let error = inspect(&tree, &["wide"], limits(4, 10, 4, 100))
        .await
        .unwrap_err();
    assert_eq!(error.stage, "resource_tree_budget");
    let files = inspect(&tree, &["wide"], limits(5, 10, 4, 100))
        .await
        .unwrap();
    assert_eq!(files.len(), 4);
}
#[tokio::test]
async fn multiple_selected_files_also_consume_entry_budget() {
    let tree = Tree::new();
    for n in ["a", "b", "c"] {
        std::fs::write(tree.0.join(n), b"x").unwrap();
    }
    assert_eq!(
        inspect(&tree, &["a", "b", "c"], limits(2, 10, 4, 100))
            .await
            .unwrap_err()
            .stage,
        "resource_tree_budget"
    );
    assert_eq!(
        inspect(&tree, &["a", "b", "c"], limits(3, 10, 4, 100))
            .await
            .unwrap()
            .len(),
        3
    );
}
#[tokio::test]
async fn deep_real_tree_is_rejected_before_visiting_beyond_depth() {
    let tree = Tree::new();
    std::fs::create_dir_all(tree.0.join("a/b/c")).unwrap();
    std::fs::write(tree.0.join("a/b/c/data"), b"x").unwrap();
    assert_eq!(
        inspect(&tree, &["a"], limits(10, 10, 2, 100))
            .await
            .unwrap_err()
            .stage,
        "resource_tree_depth"
    );
    assert_eq!(
        inspect(&tree, &["a"], limits(10, 10, 3, 100))
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn file_count_and_aggregate_bytes_use_actual_files() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("pkg")).unwrap();
    for name in ["a", "b"] {
        std::fs::write(tree.0.join("pkg").join(name), b"1234").unwrap();
    }
    assert_eq!(
        inspect(&tree, &["pkg"], limits(10, 1, 2, 100))
            .await
            .unwrap_err()
            .stage,
        "resource_file_count"
    );
    assert_eq!(
        inspect(&tree, &["pkg"], limits(10, 10, 2, 7))
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::ResourceBudgetExceeded
    );
    assert_eq!(
        inspect(&tree, &["pkg"], limits(10, 10, 2, 8))
            .await
            .unwrap()
            .iter()
            .map(|f| f.length)
            .sum::<u64>(),
        8
    );
}
#[tokio::test]
async fn duplicate_and_overlapping_selected_roots_are_explicit_errors() {
    let tree = Tree::new();
    std::fs::create_dir_all(tree.0.join("a/b")).unwrap();
    for selected in [&["a", "a"][..], &["a", "a/b"][..], &["a/b", "a"][..]] {
        assert_eq!(
            inspect(&tree, selected, InspectionLimits::default())
                .await
                .unwrap_err()
                .stage,
            "duplicate_or_overlapping_selection"
        );
    }
}
#[cfg(unix)]
#[tokio::test]
async fn literal_backslash_child_name_cannot_be_projected_as_a_different_file() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("pkg")).unwrap();
    std::fs::write(tree.0.join("pkg/bad\\name"), b"x").unwrap();
    assert_eq!(
        inspect(&tree, &["pkg"], InspectionLimits::default())
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::InvalidResource
    );
}
#[cfg(all(unix, not(target_os = "macos")))]
#[tokio::test]
async fn actual_non_utf8_child_names_are_rejected() {
    use std::os::unix::ffi::OsStringExt;
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("pkg")).unwrap();
    std::fs::write(
        tree.0
            .join("pkg")
            .join(std::ffi::OsString::from_vec(vec![b'x', 0xff])),
        b"x",
    )
    .unwrap();
    assert_eq!(
        inspect(&tree, &["pkg"], InspectionLimits::default())
            .await
            .unwrap_err()
            .stage,
        "resource_path_encoding"
    );
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_rejects_illegal_filename_and_inspector_rejects_non_utf8_root_without_io() {
    use std::os::unix::ffi::OsStringExt;
    let tree = Tree::new();
    let invalid = tree.0.join(std::ffi::OsString::from_vec(vec![b'x', 0xff]));
    assert_eq!(
        std::fs::write(&invalid, b"x").unwrap_err().raw_os_error(),
        Some(libc::EILSEQ)
    );
    let (_tx, rx) = watch::channel(false);
    assert_eq!(
        ResourceInspector::new()
            .inspect(
                InspectionRequest {
                    root: invalid,
                    selected: vec!["pkg".into()]
                },
                Duration::from_secs(1),
                rx
            )
            .await
            .unwrap_err()
            .stage,
        "resource_path_encoding"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn fifo_and_symlink_are_rejected_without_blocking_or_following() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("pkg")).unwrap();
    let pipe = tree.0.join("pkg/pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        inspect(&tree, &["pkg"], InspectionLimits::default())
            .await
            .unwrap_err()
            .stage,
        "resource_file_type"
    );
    std::fs::remove_file(pipe).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", tree.0.join("pkg/link")).unwrap();
    assert_eq!(
        inspect(&tree, &["pkg"], InspectionLimits::default())
            .await
            .unwrap_err()
            .stage,
        "resource_symlink"
    );
}
#[tokio::test]
async fn tightening_is_supported_but_expanding_production_budgets_is_rejected() {
    let mut bounds = InspectionLimits::default();
    bounds.max_entries += 1;
    assert!(ResourceInspector::with_limits(bounds).is_err());
    bounds = InspectionLimits::default();
    bounds.max_files = 0;
    assert!(ResourceInspector::with_limits(bounds).is_err());
}
#[tokio::test]
async fn pre_cancel_and_invalid_deadline_do_not_touch_a_missing_root() {
    let tree = Tree::new();
    let mut request = tree.request(&["absent"]);
    request.root = tree.0.join("missing-root");
    let (_tx, rx) = watch::channel(true);
    assert_eq!(
        ResourceInspector::new()
            .inspect(request, Duration::from_secs(1), rx)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::Cancelled
    );
    let (_tx, rx) = watch::channel(false);
    assert_eq!(
        ResourceInspector::new()
            .inspect(tree.request(&["absent"]), Duration::ZERO, rx)
            .await
            .unwrap_err()
            .stage,
        "inspection_deadline"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn scanning_an_executable_worker_only_hashes_it_and_never_launches_it() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("pkg")).unwrap();
    let worker = tree.0.join("pkg/worker.sh");
    let marker = tree.0.join("launched");
    std::fs::write(
        &worker,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700)).unwrap();
    let files = inspect(&tree, &["pkg"], InspectionLimits::default())
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(!marker.exists());
}
#[test]
fn synchronous_selected_tree_entry_preserves_sorted_content_facts() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("pkg")).unwrap();
    for name in ["z", "a"] {
        std::fs::write(tree.0.join("pkg").join(name), b"x").unwrap();
    }
    let files = inspect_selected_trees(&tree.0, &["pkg".into()]).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["pkg/a", "pkg/z"]
    );
}

#[tokio::test]
async fn actual_large_file_cancel_and_future_drop_release_capacity_before_retry() {
    for abort in [false, true] {
        let tree = Tree::new();
        std::fs::create_dir(tree.0.join("large")).unwrap();
        let file = std::fs::File::create(tree.0.join("large/model.bin")).unwrap();
        file.set_len(256 * 1024 * 1024).unwrap();
        std::fs::create_dir(tree.0.join("small")).unwrap();
        std::fs::write(tree.0.join("small/data"), b"ok").unwrap();
        let inspector = ResourceInspector::new();
        let (tx, rx) = watch::channel(false);
        let running = {
            let inspector = inspector.clone();
            let request = tree.request(&["large"]);
            tokio::spawn(async move {
                inspector
                    .inspect(request, Duration::from_secs(10), rx)
                    .await
            })
        };
        // A real second request observes that the first owns physical capacity.
        tokio::task::yield_now().await;
        let (_tx2, rx2) = watch::channel(false);
        assert_eq!(
            inspector
                .inspect(tree.request(&["small"]), Duration::from_secs(1), rx2)
                .await
                .unwrap_err()
                .code,
            OfflineErrorCode::WorkerBusy
        );
        if abort {
            running.abort();
            assert!(running.await.unwrap_err().is_cancelled());
        } else {
            tx.send(true).unwrap();
            assert_eq!(
                running.await.unwrap().unwrap_err().code,
                OfflineErrorCode::Cancelled
            );
        }
        let mut completed = false;
        for _ in 0..200 {
            let (_retry_tx, retry_rx) = watch::channel(false);
            match inspector
                .inspect(tree.request(&["small"]), Duration::from_secs(1), retry_rx)
                .await
            {
                Ok(files) => {
                    assert_eq!(files.len(), 1);
                    completed = true;
                    break;
                }
                Err(error) => assert_eq!(error.code, OfflineErrorCode::WorkerBusy),
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            completed,
            "cancelled/dropped physical inspection must cooperatively exit and release capacity"
        );
    }
}
#[tokio::test]
async fn actual_large_file_deadline_returns_without_waiting_for_entire_hash() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("large")).unwrap();
    std::fs::File::create(tree.0.join("large/model.bin"))
        .unwrap()
        .set_len(256 * 1024 * 1024)
        .unwrap();
    let inspector = ResourceInspector::new();
    let (_tx, rx) = watch::channel(false);
    let start = std::time::Instant::now();
    assert_eq!(
        inspector
            .inspect(tree.request(&["large"]), Duration::from_millis(1), rx)
            .await
            .unwrap_err()
            .code,
        OfflineErrorCode::DeadlineExceeded
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    // Wait for cooperative disposal so temporary files outlive the owned job.
    std::fs::create_dir(tree.0.join("empty")).unwrap();
    for _ in 0..200 {
        let (_tx, rx) = watch::channel(false);
        match inspector
            .inspect(tree.request(&["empty"]), Duration::from_secs(1), rx)
            .await
        {
            Ok(files) => {
                assert!(files.is_empty());
                return;
            }
            Err(error) => assert_eq!(error.code, OfflineErrorCode::WorkerBusy),
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("physical inspection did not finish");
}
