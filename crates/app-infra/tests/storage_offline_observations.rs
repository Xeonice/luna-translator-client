mod offline_resource_support;
use app_core::{domain::offline_runtime::*, ports::operation_budget::OperationBudget};
use app_infra::{
    offline_translation::{resolver::OfflineRuntimeMaterialResolver, resources::ResourceInspector},
    storage::*,
};
use offline_resource_support::*;
use std::{sync::Arc, time::Duration};
#[tokio::test]
async fn a_new_observation_uses_exact_cas_and_keeps_the_same_immutable_snapshot() {
    let f = Fixture::new();
    f.commit("first", 1).await;
    let second = f.commit("second", 2).await;
    let c = f.sql();
    assert_eq!(
        c.query_row("SELECT count(*) FROM offline_resource_snapshots", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM offline_resource_observations",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        c.query_row(
            "SELECT row_revision FROM offline_observation_heads",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        c.query_row(
            "SELECT row_revision FROM offline_installation_heads",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        f.repo
            .read_material(second.lookup(), budget())
            .unwrap()
            .observation()
            .epoch
            .get(),
        2
    );
}
#[tokio::test]
async fn a_late_observation_constraint_rolls_back_new_snapshot_route_and_receipt() {
    let f = Fixture::new();
    f.commit("base", 1).await;
    let before = f.counts();
    let mut input = f.skeleton.clone();
    input.snapshot_id = "new-snapshot".into();
    input.route_id = "new-route".into();
    input.route[0].snapshot_id = input.snapshot_id.clone();
    input.observation.epoch = nz(2);
    input.observation.head_revision = nz(2);
    // Duplicate actual observation ID fails after staged snapshot/route writes.
    let material = Arc::new(
        OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
            .inspect_stored(input.clone(), f.root.clone(), budget())
            .await
            .unwrap(),
    );
    let mut meta = f.metadata("late-failure", 2);
    meta.route = input.route;
    let p = OfflineUnknownPreparation::from_resolved(meta, material).unwrap();
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    assert!(f.repo.commit_unknown(p, b.clone(), 50).is_err());
    assert!(!b.is_committed());
    assert_eq!(before, f.counts());
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT observation_epoch FROM offline_observation_heads",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn stale_head_proposals_do_not_write_even_an_operation_receipt() {
    let f = Fixture::new();
    f.commit("base", 1).await;
    let before = f.counts();
    let material = f.resolved(2).await;
    let mut meta = f.metadata("stale", 2);
    meta.expected_observation_head = None;
    let p = OfflineUnknownPreparation::from_resolved(meta, material).unwrap();
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    assert_eq!(
        f.repo.commit_unknown(p, b, 50).unwrap_err().code,
        "offline_observation_head_conflict"
    );
    assert_eq!(before, f.counts());
}
#[tokio::test]
async fn installation_revision_head_advance_is_monotonic_and_cas_checked() {
    let f = Fixture::new();
    f.commit("base", 1).await;
    let mut input = f.skeleton.clone();
    input.installation.revision = nz(2);
    input.observation.observation_id = "observation-2".into();
    input.observation.epoch = nz(2);
    input.observation.head_revision = nz(2);
    let material = Arc::new(
        OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
            .inspect_stored(input, f.root.clone(), budget())
            .await
            .unwrap(),
    );
    let p = OfflineUnknownPreparation::from_resolved(f.metadata("installation-v2", 2), material)
        .unwrap();
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    f.repo.commit_unknown(p, b, 50).unwrap();
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT installation_revision,row_revision,enabled FROM offline_installation_heads",
                [],
                |r| Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, i64>(2)?
                ))
            )
            .unwrap(),
        (2, 2, 0)
    );
    let before = f.counts();
    let p = f.prepare("regression", 3).await;
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    assert_eq!(
        f.repo.commit_unknown(p, b, 60).unwrap_err().code,
        "offline_installation_head_conflict"
    );
    assert_eq!(before, f.counts());
}
#[tokio::test]
async fn explicit_root_revocation_changes_only_its_epoch_and_invalidates_old_checkpoint() {
    let f = Fixture::new();
    let receipt = f.commit("base", 1).await;
    let b = budget();
    let cp = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    let root = f.skeleton.root.clone();
    let revoke = budget();
    let _guard = f
        .owner
        .begin_revocation(
            "revoke".into(),
            f.repo.frame().clone(),
            root.clone(),
            revoke.clone(),
        )
        .unwrap();
    f.repo
        .revoke_root("revoke".into(), root, revoke, 60)
        .unwrap();
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT authorization_epoch,state FROM offline_local_grants",
                [],
                |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?))
            )
            .unwrap(),
        (2, "revoked".into())
    );
    assert_eq!(
        f.repo.validate_material(cp, b).unwrap_err().code,
        "offline_grant_changed"
    );
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM purpose_runtime_states", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn mismatched_root_revocation_and_new_budget_never_change_the_real_grant() {
    let f = Fixture::new();
    f.commit("base", 1).await;
    let original = f.skeleton.root.clone();
    let b = budget();
    let _guard = f
        .owner
        .begin_revocation(
            "revoke".into(),
            f.repo.frame().clone(),
            original.clone(),
            b.clone(),
        )
        .unwrap();
    let mut wrong = original.clone();
    wrong.authorization_epoch = nz(2);
    assert_eq!(
        f.repo
            .revoke_root("revoke".into(), wrong, b.clone(), 60)
            .unwrap_err()
            .code,
        "offline_operation_identity_mismatch"
    );
    assert_eq!(
        f.repo
            .revoke_root("revoke".into(), original, budget(), 60)
            .unwrap_err()
            .code,
        "offline_operation_identity_mismatch"
    );
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT authorization_epoch FROM offline_local_grants",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn stopped_or_dropped_original_operation_cannot_commit_unknown_inventory() {
    let f = Fixture::new();
    let before = f.counts();
    let p = f.prepare("cancel-first", 1).await;
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    f.owner.cancel("cancel-first").unwrap();
    assert!(f.repo.commit_unknown(p, b, 20).is_err());
    assert_eq!(before, f.counts());
    let p = f.prepare("drop-first", 1).await;
    let b = budget();
    let guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    drop(guard);
    assert_eq!(
        f.repo.commit_unknown(p, b, 20).unwrap_err().code,
        "offline_operation_not_live"
    );
    assert_eq!(before, f.counts());
}
#[tokio::test]
async fn expiry_while_waiting_for_the_real_sql_write_lock_rolls_back_every_row() {
    let f = Fixture::new();
    let p = f.prepare("sql-deadline", 1).await;
    let before = f.counts();
    let b = OperationBudget::new(Duration::from_millis(80)).unwrap();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    let lock = f.sql();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let repo = f.repo.clone();
    let work = std::thread::spawn(move || repo.commit_unknown(p, b, 20));
    std::thread::sleep(Duration::from_millis(160));
    lock.execute_batch("ROLLBACK").unwrap();
    assert_eq!(work.join().unwrap().unwrap_err().code, "operation_timeout");
    assert_eq!(before, f.counts());
}
#[test]
fn apple_unknown_inventory_never_creates_a_fake_local_grant_installation_or_file() {
    let f = Fixture::new();
    let p = f.apple_preparation("apple", 1);
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    let receipt = f.repo.commit_unknown(p, b, 30).unwrap();
    assert!(matches!(
        receipt.reference.backend,
        ResourceBackendIdentity::AppleSystem { .. }
    ));
    let cp = f.repo.read_material(receipt.lookup(), budget()).unwrap();
    assert!(cp.local_material().is_err());
    assert!(cp.local_native_path_for_owner_check().is_none());
    assert_eq!(cp.state(), "unknown");
    let c = f.sql();
    for table in [
        "offline_installation_versions",
        "offline_local_grants",
        "offline_resource_snapshot_files",
        "offline_execution_environments",
    ] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        c.query_row("SELECT state FROM offline_resource_observations", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "unknown"
    );
}
#[tokio::test]
async fn wrong_guest_platform_or_architecture_is_not_reconstructed_as_crossover() {
    let f = Fixture::new();
    let receipt = f.commit("base", 1).await;
    let b = budget();
    let cp = f.repo.read_material(receipt.lookup(), b.clone()).unwrap();
    f.sql().execute_batch("DROP TRIGGER v4_offline_runtime_profiles_no_update; UPDATE offline_runtime_profiles SET platform='macos';").unwrap();
    assert_eq!(
        f.repo.validate_material(cp, b).unwrap_err().code,
        "offline_environment_runtime_mismatch"
    );
}
#[tokio::test]
async fn locator_utf8_boundaries_are_checked_before_sql_and_cannot_be_relabelled_as_opaque_permission()
 {
    let f = Fixture::new();
    let material = f.resolved(1).await;
    let mut meta = f.metadata("limit", 1);
    meta.local.as_mut().unwrap().opaque_locator = "語".repeat(171);
    assert_eq!(
        OfflineUnknownPreparation::from_resolved(meta, material.clone())
            .unwrap_err()
            .code,
        "offline_invalid_text"
    );
    let mut meta = f.metadata("limit", 1);
    meta.local.as_mut().unwrap().opaque_locator = format!("{}ab", "語".repeat(170));
    let p = OfflineUnknownPreparation::from_resolved(meta, material).unwrap();
    let b = budget();
    let _guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    f.repo.commit_unknown(p, b, 30).unwrap();
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT length(CAST(opaque_locator AS BLOB)) FROM offline_local_grants",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        512
    );
}
#[tokio::test]
async fn revocation_replay_returns_the_durable_result_without_a_second_epoch_change() {
    let f = Fixture::new();
    f.commit("base", 1).await;
    let root = f.skeleton.root.clone();
    let b = budget();
    let _guard = f
        .owner
        .begin_revocation(
            "revoke-replay".into(),
            f.repo.frame().clone(),
            root.clone(),
            b.clone(),
        )
        .unwrap();
    f.repo
        .revoke_root("revoke-replay".into(), root.clone(), b.clone(), 50)
        .unwrap();
    let before = f.counts();
    f.repo
        .revoke_root("revoke-replay".into(), root, b, 999)
        .unwrap();
    assert_eq!(before, f.counts());
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT authorization_epoch FROM offline_local_grants",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        2
    );
}
#[tokio::test]
async fn first_read_rejects_a_different_compiled_policy_even_when_profile_labels_match() {
    let f = Fixture::new();
    let receipt = f.commit("base", 1).await;
    f.sql()
        .execute_batch("DROP TRIGGER v4_offline_runtime_profiles_no_update;")
        .unwrap();
    f.sql()
        .execute(
            "UPDATE offline_runtime_profiles SET policy_sha256=?1",
            ["f".repeat(64)],
        )
        .unwrap();
    assert_eq!(
        f.repo
            .read_material(receipt.lookup(), budget())
            .unwrap_err()
            .code,
        "offline_runtime_policy_unavailable"
    );
}
