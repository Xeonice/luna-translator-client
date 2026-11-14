//! O3A conservative retention is a real zero-delete policy, not completed GC.
mod offline_resource_support;
use offline_resource_support::*;
#[tokio::test]
async fn actual_config_source_and_control_are_rebuilt_without_guessing_or_borrowing_another_config()
{
    let f = Fixture::new();
    let receipt = f.commit("resource", 1).await;
    let source = f.save_offline_config(&receipt);
    let mut lookup = receipt.lookup();
    lookup.source = Some(source.clone());
    let b = budget();
    let cp = f.repo.read_material(lookup.clone(), b.clone()).unwrap();
    f.repo.validate_material(cp, b).unwrap();
    lookup.source.as_mut().unwrap().config_id = "another-not-saved".into();
    assert_eq!(
        f.repo.read_material(lookup, budget()).unwrap_err().code,
        "offline_source_missing"
    );
    assert_eq!(
        f.store
            .load_service_graph_revision(source.revision_id)
            .unwrap()
            .offline_engine_configs[0]
            .id,
        "local-configuration"
    );
}
#[tokio::test]
async fn real_fk_and_seal_references_are_reported_and_never_deleted_under_unknown_live_protection()
{
    let f = Fixture::new();
    let receipt = f.commit("resource", 1).await;
    let _source = f.save_offline_config(&receipt);
    let before = f.counts();
    let report = f
        .repo
        .inspect_retention(vec![receipt.reference.snapshot_id.clone()])
        .unwrap();
    assert_eq!(report.inspected_snapshots, 1);
    assert_eq!(
        report.referenced_snapshots,
        vec![receipt.reference.snapshot_id]
    );
    assert_eq!(report.removed_rows, 0);
    assert_eq!(
        report.deletion_blocker,
        "offline_retention_owner_unavailable"
    );
    assert_eq!(
        f.repo.collect_unreferenced().unwrap_err().code,
        "offline_retention_owner_unavailable"
    );
    assert_eq!(before, f.counts());
    let c = f.sql();
    for sql in [
        "DELETE FROM offline_resource_seals",
        "DELETE FROM offline_route_seals",
        "DELETE FROM offline_resource_snapshot_files",
        "DELETE FROM offline_local_grants",
        "DELETE FROM offline_execution_environments",
        "DELETE FROM offline_runtime_profiles",
    ] {
        assert!(
            c.execute(sql, []).is_err(),
            "actual referenced/sealed SQL unexpectedly deleted: {sql}"
        );
    }
    assert_eq!(before, f.counts());
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
async fn preparation_guard_and_metadata_repository_do_not_claim_full_runtime_lease_gc_ownership() {
    let f = Fixture::new();
    let p = f.prepare("resource", 1).await;
    let b = budget();
    let guard = f.owner.begin(p.clone(), b.clone()).unwrap();
    f.repo.commit_unknown(p, b, 30).unwrap();
    let before = f.counts();
    assert!(f.repo.collect_unreferenced().is_err());
    drop(guard);
    assert!(f.repo.collect_unreferenced().is_err());
    assert_eq!(before, f.counts());
    let report = f
        .repo
        .inspect_retention(vec!["absent-snapshot".into()])
        .unwrap();
    assert!(report.referenced_snapshots.is_empty());
    assert_eq!(report.removed_rows, 0);
}
#[test]
fn bounded_retention_inspection_cannot_expand_an_arbitrary_input_into_an_unbounded_db_scan() {
    let f = Fixture::new();
    let before = f.counts();
    assert_eq!(
        f.repo
            .inspect_retention((0..129).map(|i| format!("candidate-{i}")).collect())
            .unwrap_err()
            .code,
        "offline_retention_budget_exceeded"
    );
    assert_eq!(before, f.counts());
}
#[tokio::test]
async fn config_control_changes_invalidate_the_original_source_checkpoint() {
    let f = Fixture::new();
    let receipt = f.commit("resource", 1).await;
    let source = f.save_offline_config(&receipt);
    let mut lookup = receipt.lookup();
    lookup.source = Some(source);
    let b = budget();
    let cp = f.repo.read_material(lookup, b.clone()).unwrap();
    f.sql().execute("UPDATE offline_config_runtime_states SET is_suspended=1,control_revision=control_revision+1 WHERE engine_config_id='local-configuration'",[]).unwrap();
    assert_eq!(
        f.repo.validate_material(cp, b).unwrap_err().code,
        "offline_material_changed"
    );
}
#[tokio::test]
async fn a_saved_unknown_resource_and_real_repository_never_make_the_service_selectable() {
    use app_core::{
        contracts::{CatalogPurpose, SavedServiceEligibility, SavedServiceQuery, ServiceScope},
        domain::model_config::service_graph::SavedServiceRef,
        ports::ServiceConfigurationRepository,
    };
    let f = Fixture::new();
    let receipt = f.commit("resource", 1).await;
    let source = f.save_offline_config(&receipt);
    let projection = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: f.repo.frame().generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    let row = projection
        .rows
        .iter()
        .find(|r| {
            r.source
                == SavedServiceRef::Offline {
                    source: source.clone(),
                }
        })
        .unwrap();
    let SavedServiceEligibility::Blocked { reasons } = &row.eligibility else {
        panic!("Unknown inventory was promoted to selectable")
    };
    assert!(
        reasons
            .iter()
            .any(|r| r.code == "offline_authority_not_ready")
    );
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT count(*) FROM purpose_runtime_states WHERE is_enabled=1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
