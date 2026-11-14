//! Real production Store4 and O2 temporary-file integration, Unknown-only.
mod offline_resource_support;
use app_core::{
    domain::{content_hash, model_config::service_graph::*, offline_runtime::*},
    ports::operation_budget::OperationBudget,
};
use app_infra::{
    offline_translation::{resolver::OfflineRuntimeMaterialResolver, resources::ResourceInspector},
    storage::*,
};
use offline_resource_support::*;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn real_file_inventory_writes_all_rows_unknown_and_reconstructs_after_actual_sql_read() {
    let f = Fixture::new();
    let receipt = f.commit("inventory", 1).await;
    assert_eq!(receipt.state, "unknown");
    let b = budget();
    let checkpoint = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    assert_eq!(checkpoint.state(), "unknown");
    assert_eq!(
        checkpoint.local_native_path_for_owner_check(),
        f.root.to_str()
    );
    let actual = OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
        .inspect_stored(
            checkpoint.local_material().unwrap(),
            f.root.clone(),
            b.clone(),
        )
        .await
        .unwrap();
    assert_eq!(actual.reference(), &receipt.reference);
    assert!(matches!(
        actual.coverage(),
        RuntimeCoverageReport::Unknown { .. }
    ));
    f.repo.validate_material(checkpoint, b).unwrap();
    let c = f.sql();
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM offline_resource_snapshot_files",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        4
    );
    assert_eq!(
        c.query_row("SELECT state FROM offline_local_grants", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "unknown"
    );
    assert_eq!(
        c.query_row(
            "SELECT verification_state FROM offline_runtime_profiles",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "unknown"
    );
    assert_eq!(
        c.query_row("SELECT enabled FROM offline_installation_heads", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM request_tasks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM request_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
}
#[tokio::test]
async fn original_operation_replays_its_real_receipt_without_more_rows_or_revision_changes() {
    let f = Fixture::new();
    let p = f.prepare("replay", 1).await;
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    let first = f.repo.commit_unknown(p.clone(), b.clone(), 20).unwrap();
    let before = f.counts();
    let revision = f
        .sql()
        .query_row("SELECT commit_revision FROM profile_meta", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();
    let second = f.repo.commit_unknown(p, b, 999).unwrap();
    assert_eq!(first, second);
    assert_eq!(before, f.counts());
    assert_eq!(
        f.sql()
            .query_row("SELECT commit_revision FROM profile_meta", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        revision
    );
}
#[tokio::test]
async fn equal_facts_in_a_new_arc_are_not_the_original_owned_preparation() {
    let f = Fixture::new();
    let original = f.prepare("original", 1).await;
    let copy = f.prepare("original", 1).await;
    assert_eq!(original.request_sha256(), copy.request_sha256());
    assert!(!Arc::ptr_eq(&original, &copy));
    let b = budget();
    let _guard = f.owner.begin(original, b.clone()).unwrap();
    let before = f.counts();
    assert_eq!(
        f.repo.commit_unknown(copy, b, 20).unwrap_err().code,
        "offline_preparation_not_original"
    );
    assert_eq!(before, f.counts());
}
#[tokio::test]
async fn a_new_equal_deadline_budget_cannot_replace_the_original_control() {
    let f = Fixture::new();
    let p = f.prepare("budget", 1).await;
    let original = budget();
    let _guard = f.owner.begin(p.clone(), original).unwrap();
    let before = f.counts();
    assert_eq!(
        f.repo.commit_unknown(p, budget(), 20).unwrap_err().code,
        "offline_operation_identity_mismatch"
    );
    assert_eq!(before, f.counts());
}
#[tokio::test]
async fn identical_frame_strings_and_even_the_same_owner_cannot_cross_actual_writers() {
    let f = Fixture::new();
    let receipt = f.commit("one", 1).await;
    let b = budget();
    let checkpoint = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    let second_db = f.directory.join("same-labels-different-actor.sqlite3");
    let other = SqliteStore::open(second_db, "profile", "server", 10).unwrap();
    let other = other.offline_resources_repository(f.owner.clone()).unwrap();
    assert_eq!(other.frame(), f.repo.frame());
    assert_eq!(
        other
            .validate_material(checkpoint.clone(), b.clone())
            .unwrap_err()
            .code,
        "offline_checkpoint_identity_mismatch"
    );
    let same = f
        .store
        .clone()
        .offline_resources_repository(f.owner.clone())
        .unwrap();
    same.validate_material(checkpoint, b).unwrap();
}
#[tokio::test]
async fn dropping_the_real_store_releases_profile_and_old_checkpoint_never_reopens() {
    let f = Fixture::new();
    let receipt = f.commit("first-server", 1).await;
    let b = budget();
    let cp = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    let Fixture {
        store,
        repo,
        database,
        owner,
        ..
    } = f;
    drop(repo);
    drop(store);
    let reopened = SqliteStore::open(database, "profile", "server", 100).unwrap();
    let repo = reopened.offline_resources_repository(owner).unwrap();
    assert_eq!(
        repo.validate_material(cp, b).unwrap_err().code,
        "offline_checkpoint_identity_mismatch"
    );
    let fresh = repo.read_material(receipt.lookup(), budget()).unwrap();
    assert_eq!(fresh.state(), "unknown");
}
#[test]
fn a_different_live_owner_cannot_replace_the_actual_registered_one() {
    let f = Fixture::new();
    assert_eq!(
        f.store
            .offline_resources_repository(LiveOfflinePreparations::new())
            .err()
            .unwrap()
            .code,
        "offline_owner_already_configured"
    );
}
#[tokio::test]
async fn content_identical_other_root_does_not_become_the_declared_owner_location() {
    let f = Fixture::new();
    let other = f.directory.join("other-identical-root");
    std::fs::create_dir_all(other.join(".luna")).unwrap();
    for name in [
        "worker.exe",
        "engine.dll",
        "dictionary.bin",
        CONTROL_RELATIVE_KEY,
    ] {
        std::fs::copy(f.root.join(name), other.join(name)).unwrap();
    }
    let actual = Arc::new(
        OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
            .inspect_stored(f.skeleton.clone(), other.canonicalize().unwrap(), budget())
            .await
            .unwrap(),
    );
    assert_eq!(
        OfflineUnknownPreparation::from_resolved(f.metadata("wrong-root", 1), actual)
            .unwrap_err()
            .code,
        "offline_material_location_mismatch"
    );
    assert!(f.counts().iter().all(|(_, n)| *n == 0));
}
#[tokio::test]
async fn checkpoint_freezes_all_actual_rows_even_nonsemantic_timestamp_changes() {
    let f = Fixture::new();
    let receipt = f.commit("row-bytes", 1).await;
    let b = budget();
    let cp = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    // Deliberate damaged-database injection, not a production mutation path.
    f.sql().execute_batch("DROP TRIGGER v4_offline_resource_snapshots_no_update; UPDATE offline_resource_snapshots SET created_at_ms=999 WHERE snapshot_id='snapshot';").unwrap();
    assert_eq!(
        f.repo.validate_material(cp, b).unwrap_err().code,
        "offline_material_changed"
    );
}
#[tokio::test]
async fn second_phase_actual_io_detects_file_changes_without_treating_sql_as_physical_proof() {
    let f = Fixture::new();
    let receipt = f.commit("physical-change", 1).await;
    let b = budget();
    let cp = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    std::fs::write(f.root.join("engine.dll"), "different untrusted bytes").unwrap();
    assert!(
        OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
            .inspect_stored(cp.local_material().unwrap(), f.root.clone(), b.clone())
            .await
            .is_err()
    );
    // SQL itself remains equal; only the explicit physical phase proves bytes.
    f.repo.validate_material(cp, b).unwrap();
}
#[test]
fn new_snapshot_and_route_domains_have_independently_built_canonical_bytes() {
    use serde_json::json;
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let p = &f.skeleton.profile;
    let manifest = &f.skeleton.manifest_sha256;
    let profile = json!({"profileId":p.profile_id,"revision":p.revision.get(),"engine":{"engineId":"jbeijing","contractRevision":1},"backend":"local_worker","platform":"crossover","architecture":"x86","privateProtocolRevision":2,"policySha256":p.policy_sha256});
    let snapshot_bytes=serde_json::to_vec(&json!(["luna-offline-snapshot-rows-v1",profile,{"kind":"file_manifest","manifest_sha256":manifest},[{"pair":{"source":"ja","target":"zh-Hans"},"pipeline_profile_id":"controlled-vendor-pipeline"}]])).unwrap();
    let expected = format!("{:x}", Sha256::digest(snapshot_bytes));
    assert_eq!(
        offline_snapshot_content_hash(
            p,
            &OfflineSnapshotIdentity::FileManifest {
                manifest_sha256: manifest.clone()
            },
            &f.metadata("not-used", 1).pairs
        )
        .unwrap(),
        expected
    );
    let route_bytes=serde_json::to_vec(&json!(["luna-offline-route-rows-v1",{"engineId":"jbeijing","contractRevision":1},"local_worker",{"source":"ja","target":"zh-Hans"},{"kind":"direct_only"},[["snapshot",{"source":"ja","target":"zh-Hans"},"controlled-vendor-pipeline"]]])).unwrap();
    assert_eq!(
        offline_route_content_hash(
            p,
            &f.skeleton.pair,
            &OfflineRoutePolicy::DirectOnly {},
            &f.skeleton.route
        )
        .unwrap(),
        format!("{:x}", Sha256::digest(route_bytes))
    );
}
#[test]
fn route_cycles_unstored_locators_four_hops_and_duplicate_pairs_are_rejected() {
    let f = Fixture::new();
    let p = &f.skeleton.profile;
    let mut hops = f.skeleton.route.clone();
    hops[0].package_relative_key = Some("package.argosmodel".into());
    assert_eq!(
        offline_route_content_hash(
            p,
            &f.skeleton.pair,
            &OfflineRoutePolicy::DirectOnly {},
            &hops
        )
        .unwrap_err()
        .code,
        "offline_route_package_locator_unstored"
    );
    let route = (0..4)
        .map(|i| OfflineRouteHop {
            snapshot_id: "snapshot".into(),
            pair: LanguagePair {
                source: format!("l{i}"),
                target: format!("l{}", i + 1),
            },
            pipeline_profile_id: "p".into(),
            package_relative_key: None,
        })
        .collect::<Vec<_>>();
    assert!(
        offline_route_content_hash(
            p,
            &LanguagePair {
                source: "l0".into(),
                target: "l4".into()
            },
            &OfflineRoutePolicy::ExplicitPivot {
                consent_revision: nz(1)
            },
            &route
        )
        .is_err()
    );
    let cycle = vec![
        OfflineRouteHop {
            snapshot_id: "snapshot".into(),
            pair: LanguagePair {
                source: "ja".into(),
                target: "en".into(),
            },
            pipeline_profile_id: "p".into(),
            package_relative_key: None,
        },
        OfflineRouteHop {
            snapshot_id: "snapshot".into(),
            pair: LanguagePair {
                source: "en".into(),
                target: "ja".into(),
            },
            pipeline_profile_id: "p".into(),
            package_relative_key: None,
        },
        OfflineRouteHop {
            snapshot_id: "snapshot".into(),
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh-Hans".into(),
            },
            pipeline_profile_id: "p".into(),
            package_relative_key: None,
        },
    ];
    assert_eq!(
        offline_route_content_hash(
            p,
            &f.skeleton.pair,
            &OfflineRoutePolicy::ExplicitPivot {
                consent_revision: nz(1)
            },
            &cycle
        )
        .unwrap_err()
        .code,
        "offline_route_cycle"
    );
    let pair = f.metadata("unused", 1).pairs[0].clone();
    assert_eq!(
        offline_snapshot_content_hash(
            p,
            &OfflineSnapshotIdentity::FileManifest {
                manifest_sha256: content_hash("manifest")
            },
            &[pair.clone(), pair]
        )
        .unwrap_err()
        .code,
        "offline_duplicate_pair"
    );
}
#[test]
fn the_inventory_frame_is_not_a_new_operation_lifetime() {
    let b = OperationBudget::new(Duration::ZERO).unwrap();
    assert_eq!(b.check().unwrap_err().code, "operation_timeout");
}
#[tokio::test]
async fn simultaneous_original_replays_have_one_commit_and_return_the_same_real_receipt() {
    let f = Fixture::new();
    let p = f.prepare("concurrent", 1).await;
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(9));
    let workers = (0..8)
        .map(|_| {
            let repo = f.repo.clone();
            let p = p.clone();
            let b = b.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                repo.commit_unknown(p, b, 30)
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let receipts = workers
        .into_iter()
        .map(|w| w.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert!(receipts.iter().all(|r| r == &receipts[0]));
    assert_eq!(f.sql().query_row("SELECT count(*) FROM operation_receipts WHERE command_kind='offline_unknown_preparation'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT count(*) FROM offline_resource_observations",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}
