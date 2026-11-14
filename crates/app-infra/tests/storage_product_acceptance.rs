//! Actual production storage acceptance. Component tests execute afresh in this
//! target; only the final test assembles their same-run assertion receipts.
//! The inventory is a completeness check, never a source of PASS assertions.
// Each included module remains an independently runnable Cargo integration
// target. Its stateless evidence helper is intentionally loaded in each local
// namespace here as well; it contains no tests or shared mutable state. This
// test-crate-only exception avoids exposing private Writer test machinery from
// the production library. All other Clippy checks remain denied by -D warnings.
#![allow(clippy::duplicate_mod)]
#[path = "application_runtime.rs"]
mod application_runtime;
#[path = "configuration_lifecycle.rs"]
mod configuration_lifecycle;
#[path = "history_queries.rs"]
mod history_queries;
#[path = "model_default_templates.rs"]
mod model_default_templates;
#[path = "session_export_delivery.rs"]
mod session_export_delivery;
#[path = "session_records_batch.rs"]
mod session_records_batch;
#[path = "storage_acceptance_routes.rs"]
mod storage_acceptance_routes;
#[path = "storage_cache_semantics.rs"]
mod storage_cache_semantics;
#[path = "storage_credential_crash.rs"]
mod storage_credential_crash;
#[path = "storage_disk_full.rs"]
mod storage_disk_full;
#[path = "storage_field_boundaries.rs"]
mod storage_field_boundaries;
#[path = "storage_field_mapping.rs"]
mod storage_field_mapping;
#[path = "storage_product.rs"]
mod storage_product;
#[path = "storage_schema_contract.rs"]
mod storage_schema_contract;
#[path = "storage_session_v2.rs"]
mod storage_session_v2;

#[path = "storage_acceptance_support/dictionary.rs"]
mod dictionary;
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod evidence;
#[path = "storage_acceptance_support/fields.rs"]
mod fields;
#[path = "storage_acceptance_support/receipt.rs"]
mod receipt;
#[path = "storage_acceptance_support/resource_rejections.rs"]
mod resource_rejections;

#[test]
fn y_profile_fields_have_producer_specific_typed_evidence() {
    let checks = fields::verify_profile_fields();
    let mapping_count = checks
        .iter()
        .filter(|v| v["id"].as_str().unwrap().starts_with("RUST-MAP:profile."))
        .count();
    assert_eq!(
        mapping_count, 399,
        "all profile fields require concrete production evidence"
    );
    let output = receipt::output();
    receipt::write_new(
        &output.join("profile-field-proofs.json"),
        &serde_json::json!({"checks":checks}),
    );
    evidence::record(
        "production-profile-field-proofs",
        &[("STORAGE-01", mapping_count as u64)],
        serde_json::json!({"receipt":output.join("profile-field-proofs.json")}),
    );
}

#[test]
fn y_real_nonempty_dictionary_has_asset_and_readonly_consumer_proofs() {
    let output = receipt::output();
    let directory = output.join("dictionary-fresh");
    std::fs::create_dir(&directory).unwrap();
    let (database, checks) = dictionary::build_and_verify_dictionary(&directory);
    assert_eq!(
        checks
            .iter()
            .filter(|v| v["id"]
                .as_str()
                .unwrap()
                .starts_with("RUST-MAP:dictionary."))
            .count(),
        32
    );
    let connection = receipt::readonly(&database);
    let entries: i64 = connection
        .query_row("SELECT count(*) FROM dictionary_entries", [], |r| r.get(0))
        .unwrap();
    assert!(
        entries > 0,
        "fresh_dictionary is an actual offline resource build, never the empty schema constructor"
    );
    let violations: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
    drop(connection);
    receipt::write_new(
        &output.join("dictionary-field-proofs.json"),
        &serde_json::json!({"checks":checks,"database":receipt::snapshot(&output,&database),"entries":entries}),
    );
    evidence::record(
        "real-nonempty-dictionary",
        &[
            ("STORAGE-02", 1),
            ("STORAGE-16", 1),
            ("MIGRATION-ROUTE:fresh_dictionary", 1),
        ],
        serde_json::json!({"receipt":output.join("dictionary-field-proofs.json"),"database":receipt::snapshot(&output,&database),"entries":entries}),
    );
}

#[test]
fn y_resource_mismatch_rejects_and_preserves_historical_references() {
    let output = receipt::output().join("resource-rejections");
    resource_rejections::verify(&output);
}

#[test]
fn zz_complete_storage_runtime_receipt_requires_every_actual_assertion() {
    if std::env::var_os("FP_STORAGE_OUTPUT").is_none() {
        // The normal workspace test run still executed the production scenarios;
        // it cannot sign a runtime receipt without the executor's run identity.
        return;
    }
    receipt::assemble();
}
