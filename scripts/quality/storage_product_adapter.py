#!/usr/bin/env python3
"""Read-only audit of the real Rust storage adapter's source and run evidence.

This is not an alternative producer. Only locked Cargo tests may issue runtime
assertions; this utility makes their case-to-source mapping reviewable.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TESTS = "crates/app-infra/tests/"
FAULTS = "crates/app-infra/src/storage/database_fault_tests.rs"
# Stable, explicit obligations: not inferred from expected check inventory.
CASE_SOURCES = {
    "bounded-writer-queue": (FAULTS, "writer_queue_rejects_the_129th_pending_job_and_recovers_without_losing_accepted_jobs", "STORAGE-06"),
    "cross-process-busy": (FAULTS, "another_process_reserved_lock_times_out_then_the_real_writer_recovers", "STORAGE-06"),
    "readonly-retry": (FAULTS, "actual_readonly_connection_failure_keeps_committed_state_and_allows_same_operation_retry", "STORAGE-07"),
    "kill-before-commit": (FAULTS, "process_kill_before_sqlite_actor_commit_rolls_back_all_related_rows_and_revision", "STORAGE-09"),
    "kill-after-commit": (FAULTS, "process_kill_after_actual_business_commit_keeps_unit_receipt_and_revision_together", "STORAGE-09"),
    "os-lease-profile-isolation": (FAULTS, "killed_process_releases_actual_os_lease_and_separate_profiles_remain_isolated", "STORAGE-15"),
    "online-wal-prefix": (TESTS + "storage_acceptance_routes.rs", "production_online_wal_backup_preserves_a_real_committed_repository_prefix", "STORAGE-05"),
    "actual-keychain-crash-boundaries": (TESTS + "storage_credential_crash.rs", "real_keychain_and_sqlite_crash_boundaries_recover_without_plaintext_or_active_secret_loss", "STORAGE-10"),
    "actual-limited-volume-enospc": (TESTS + "storage_disk_full.rs", "real_isolated_disk_full_rolls_back_and_retries_the_same_repository_operation", "STORAGE-08"),
    "revoked-cleanup-restart": (TESTS + "configuration_lifecycle.rs", "failed_vault_cleanup_survives_restart_and_recovery_never_reenables_connection", "STORAGE-11"),
    "credential-revoke-cas-cleanup": (TESTS + "configuration_lifecycle.rs", "credential_revocation_cas_shared_pool_abort_and_cleanup_retry_are_authoritative", "STORAGE-11"),
    "completion-suspend-transaction-race": (TESTS + "configuration_lifecycle.rs", "completion_and_suspend_share_writer_order_and_late_result_never_reappears", "STORAGE-12"),
    "runtime-stop-late-callback": (TESTS + "configuration_lifecycle.rs", "runtime_immediate_stop_aborts_shared_calls_erases_partials_and_ignores_late_callbacks", "STORAGE-12"),
    "correction-immutable-results": (TESTS + "storage_product.rs", "completed_old_sentence_stays_old_and_corrections_are_independent", "STORAGE-13"),
    "cache-semantic-authority": (TESTS + "storage_product.rs", "cache_hit_preserves_independent_history_and_current_authorization", "STORAGE-13"),
    "cache-semantic-metamorphic-matrix": (TESTS + "storage_cache_semantics.rs", "real_cache_lookup_metamorphic_matrix_preserves_semantics_and_corrections", "STORAGE-13"),
    "explicit-retranslation-bypasses-cache-idempotently": (TESTS + "application_runtime.rs", "explicit_retranslation_bypasses_real_cache_replays_once_and_preserves_correction", "STORAGE-13"),
    "resource-mismatch-rejection-and-old-references": (TESTS + "storage_acceptance_support/resource_rejections.rs", "verify", "STORAGE-16"),
    "retention-protected-provenance": (TESTS + "storage_product.rs", "raw_retention_drops_only_payload_and_history_clear_respects_protected_objects", "STORAGE-14"),
    "keyset-paging-concurrent-revisions": (TESTS + "history_queries.rs", "keyset_handles_equal_timestamps_multiple_revisions_and_concurrent_newer_inserts", "STORAGE-14"),
    "sealed-config-gc": (TESTS + "storage_product.rs", "config_gc_retains_header_seal_tombstone_and_refuses_referenced_snapshots", "STORAGE-22"),
    "sealed-pool-gc": (TESTS + "storage_acceptance_support/fields.rs", "verify_pool_gc", "STORAGE-22"),
    "typed-production-relationships": (TESTS + "storage_acceptance_support/fields.rs", "verify_relationships", "STORAGE-18"),
    "draft-tristate-parameters": (TESTS + "storage_product.rs", "draft_is_not_applied_and_zero_parameter_survives", "STORAGE-19"),
    "configuration-head-cas": (TESTS + "storage_product.rs", "configuration_head_cas_rejects_stale_form_even_with_fresh_commit_revision", "STORAGE-19"),
    "operation-cas-idempotency": (TESTS + "storage_product.rs", "game_cas_idempotency_archive_and_history_preservation", "STORAGE-20"),
    "task-operation-idempotency": (TESTS + "storage_product.rs", "active_and_duplicate_operation_never_prepare_a_second_request", "STORAGE-20"),
    "hook-identity-provenance": (TESTS + "storage_product.rs", "hook_requires_explicit_selection_and_preserves_repeat_provenance", "STORAGE-21"),
    "unicode-multiline-refs": (TESTS + "storage_product.rs", "capture_fragments_have_immutable_revisions_and_ordered_raw_provenance", "STORAGE-21"),
    "model-default-templates-reopen": (TESTS + "model_default_templates.rs", "model_defaults_round_trip_real_four_column_relation_and_drive_actual_requests_after_restart", "STORAGE-19"),
    "all-sql-field-probes": (TESTS + "storage_field_boundaries.rs", "every_frozen_field_probe_runs_against_real_production_sqlite_tables", "STORAGE-17"),
    "production-profile-field-proofs": (TESTS + "storage_product_acceptance.rs", "y_profile_fields_have_producer_specific_typed_evidence", "STORAGE-01"),
    "real-nonempty-dictionary": (TESTS + "storage_product_acceptance.rs", "y_real_nonempty_dictionary_has_asset_and_readonly_consumer_proofs", "STORAGE-16"),
}

V2_CASE_SOURCES = {
    "schema-two-unbind-serialization-replay": (TESTS + "application_runtime.rs", "unbind_preflight_effect_and_commit_are_serialized_against_association_and_replay_is_inert", "V2-UNBIND-SERIALIZATION"),
    "schema-two-unbind-stale-identity": (TESTS + "application_runtime.rs", "unbind_stale_session_or_window_is_rejected_before_success_fact_or_effect", "V2-UNBIND-IDENTITY"),
    "schema-two-unbind-commit-compensation": (TESTS + "application_runtime.rs", "real_unbind_commit_failure_restores_association_then_retry_preserves_capture_and_history", "V2-UNBIND-COMPENSATION"),
    "schema-two-scoped-query-saved-head": (TESTS + "scoped_preferences.rs", "scope_query_distinguishes_missing_saved_head_and_is_read_only", "STORAGE-19"),
    "schema-two-scoped-head-cas-idempotency": (TESTS + "scoped_preferences.rs", "scoped_head_cas_ignores_unrelated_reading_commits_but_preserves_idempotency_and_conflicts", "STORAGE-20"),
    "schema-two-scoped-isolation-reopen": (TESTS + "scoped_preferences.rs", "explicit_scope_does_not_return_another_active_game_and_roundtrips_saved_values", "STORAGE-19"),
    "schema-two-scoped-inherit-false-zero-empty": (TESTS + "scoped_preferences.rs", "false_zero_empty_and_legacy_segmentation_survive_until_explicit_inherited_draft_save", "STORAGE-19"),
    "schema-two-scoped-writer-race": (TESTS + "scoped_preferences.rs", "competing_scope_writers_have_one_winner_and_one_explicit_head_conflict", "STORAGE-20"),
    "schema-two-string-sequences": (TESTS + "storage_session_v2.rs", "persistent_sequences_above_javascript_integer_precision_remain_decimal_strings", "V2-PRODUCERS"),
    "schema-two-jsonl-atomic-delivery": (TESTS + "session_export_delivery.rs", "fixed_range_multi_page_jsonl_is_atomic_unicode_and_hash_correct", "V2-EXPORT"),
    "schema-two-jsonl-cancel-reject": (TESTS + "session_export_delivery.rs", "cancellation_and_invalid_destination_preserve_original_and_remove_pending", "V2-EXPORT"),
    "schema-two-jsonl-enospc-retry": (TESTS + "session_export_delivery.rs", "actual_limited_volume_write_failure_keeps_original_and_same_plan_retries", "V2-EXPORT"),
    "schema-two-production-fields": (TESTS + "storage_session_v2.rs", "all_29_schema_two_fields_have_real_typed_producer_evidence", "V2-PRODUCERS"),
    "schema-two-field-boundaries": (TESTS + "storage_session_v2.rs", "every_new_column_has_actual_type_null_and_constraint_probes", "V2-RELATIONSHIPS"),
    "schema-two-immutable-relations": (TESTS + "storage_session_v2.rs", "schema_two_composite_relationships_and_immutable_identity_cannot_be_bypassed", "V2-RELATIONSHIPS"),
    "schema-two-migration-backup": (TESTS + "storage_session_v2.rs", "migration_two_preserves_legacy_data_and_validated_standalone_v1_backup", "V2-MIGRATION"),
}

V3_CASE_SOURCES = {
    "migration": (TESTS + "storage_http_identity_v3.rs", "original_writer_schema_two_migrates_without_rewriting_old_seal_order_zero_or_null", "V3-MIGRATION"),
    "codec": (TESTS + "storage_http_identity_v3.rs", "actual_codec_two_save_apply_reopen_rehash_matches_seal_and_immutable_preset", "V3-CODEC"),
    "credentials": (TESTS + "storage_http_identity_v3.rs", "trusted_typed_pool_metadata_cannot_select_other_kind_inactive_or_wrong_revision", "V3-CREDENTIALS"),
    "fields": (TESTS + "storage_http_identity_v3.rs", "all_24_delta_fields_have_real_producers_and_frozen_field_boundaries_execute_on_production_tables", "V3-PRODUCERS"),
    "gateway-authority": (TESTS + "provider_gateway_identity.rs", "exact_binding_typed_authority_and_real_task_gateway_paths", "V3-AUTHORITY"),
    "crash-mid-sql": (TESTS + "storage_http_identity_crash.rs", "kill_inside_actual_sql_rebuild_rolls_back_dropped_fk_target_and_original_seal", None),
    "crash-after-host": (TESTS + "storage_http_identity_crash.rs", "kill_after_real_store_migration_commit_recovers_schema_three_and_old_immutable_data", None),
    "backup-collision": (TESTS + "storage_http_identity_crash.rs", "preupgrade_backup_collision_blocks_migration_without_changing_original_bytes", None),
    "old-client-rejects-three": (TESTS + "storage_http_identity_crash.rs", "old_v2_reader_actually_rejects_current_three_without_mutating_database", None),
    "profile-mismatch-reject": (TESTS + "storage_http_identity_crash.rs", "current_three_profile_hash_and_structure_errors_reject_readonly_without_writing", None),
    "host-rollback-failure": ("crates/app-infra/src/storage/database.rs", "actual_migration_host_sql_failure_after_ddl_rolls_back_and_then_recovers", None),
    "single-file-backup": (TESTS + "storage_http_identity_v3.rs", "online_backup_delivers_a_closed_single_file_without_changing_live_wal_or_losing_its_prefix", None),
    "typed-envelope-sqlite": (TESTS + "provider_credential_envelopes.rs", "each_http_material_uses_trusted_metadata_and_never_persists_plaintext_in_sqlite", None),
    "typed-activation-compensation": (TESTS + "provider_credential_envelopes.rs", "vault_failure_and_activation_revocation_race_compensate_without_active_material", None),
    "typed-vault-failure": (TESTS + "provider_credential_envelopes.rs", "vault_failure_and_activation_revocation_race_compensate_without_active_material", None),
    "legacy-raw-vault": (TESTS + "provider_credential_envelopes.rs", "legacy_json_like_key_remains_raw_through_same_shared_storage_lifecycle", None),
    "typed-real-system-keychain": (TESTS + "provider_credential_envelopes.rs", "actual_system_keychain_roundtrips_all_http_envelopes_with_real_sql_metadata", None),
}


def v3_source_map(root: Path = ROOT) -> dict:
    rows, errors = [], []
    for case, (source, function, check) in V3_CASE_SOURCES.items():
        try:
            path = root / source
            if path.is_symlink():
                raise ValueError("source cannot be a symbolic link")
            content = path.read_text()
            function_token = "fn " + function + "("
            # Ordinary components have a literal emit case; the gateway has a
            # single fixed output filename instead of a generic emit helper.
            token = '"' + case + '"'
            if case == "gateway-authority":
                token = "components/v3-delta/gateway-authority.json"
            if function_token not in content or token not in content:
                raise ValueError("missing actual function/evidence emission")
            rows.append(dict(case=case, check=check, source=source, function=function,
                             function_line=content[:content.index(function_token)].count("\n") + 1,
                             evidence_line=content[:content.index(token)].count("\n") + 1,
                             source_sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
        except (OSError, ValueError) as exc:
            errors.append("schema3 assertion source " + case + ": " + str(exc))
    return dict(kind="schema_three_actual_assertion_source_map", status="failed" if errors else "passed",
                scope="source review aid only; not runtime acceptance", cases=rows, errors=errors)


def source_map(root: Path = ROOT) -> dict:
    rows, errors = [], []
    for case, (source, function, check) in (CASE_SOURCES | V2_CASE_SOURCES).items():
        path = root / source
        content = path.read_text()
        function_token = "fn " + function + "("
        record_token = '"' + case + '"'
        if function_token not in content or record_token not in content:
            errors.append("missing actual source function or assertion evidence call: " + case)
            continue
        rows.append(dict(case=case, check=check, source=source, function=function,
                         function_line=content[:content.index(function_token)].count("\n") + 1,
                         evidence_line=content[:content.index(record_token)].count("\n") + 1,
                         source_sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    return dict(kind="storage_actual_assertion_source_map", status="failed" if errors else "passed",
                scope="source review aid only; not runtime storage acceptance", cases=rows, errors=errors)


def audit_execution(directory: Path) -> dict:
    root = directory.resolve()
    receipt = json.loads((root / "receipt.json").read_text())
    errors = []
    if receipt.get("kind") != "product_storage_runtime_receipt" or receipt.get("status") != "passed":
        errors.append("complete real runtime receipt is required")
    cases = {}
    for path in (root / "asserted-cases").glob("*.json"):
        if path.is_symlink():
            errors.append("case evidence cannot be a symbolic link")
            continue
        data = json.loads(path.read_text())
        if data.get("run_id") != receipt.get("run_id") or data.get("source_tree_sha256") != receipt.get("source_tree_sha256"):
            errors.append("case is not from this runtime/source: " + str(path.name))
        if data.get("case") in cases:
            errors.append("duplicate asserted case")
        cases[data.get("case")] = data
    is_v3 = receipt.get("adapter_version") == "rust-storage-complete-v3"
    is_v2 = is_v3 or receipt.get("adapter_version") == "rust-storage-complete-v2"
    required_cases = CASE_SOURCES | (V2_CASE_SOURCES if is_v2 else {})
    for case in required_cases:
        if case not in cases:
            errors.append("required real scenario missing: " + case)
        elif is_v2 and case.startswith("schema-two-unbind-"):
            required_check = V2_CASE_SOURCES[case][2]
            proofs = cases[case].get("checks")
            if not isinstance(proofs, list) or not any(isinstance(row, dict) and row.get("id") == required_check and row.get("status") == "passed"
                       and type(row.get("assertions")) is int and row["assertions"] > 0
                       and type(row.get("skipped")) is int and row["skipped"] == 0 for row in proofs):
                errors.append("v2 unbind case lacks its own actual semantic assertion: " + case)
    checks = receipt.get("checks", [])
    ids = [row.get("id") for row in checks]
    if len(ids) != len(set(ids)):
        errors.append("duplicate runtime assertion identity")
    fields = [row for row in checks if str(row.get("id", "")).startswith("RUST-MAP:")]
    boundary = json.loads((ROOT / "docs/architecture/database/field-boundaries.json").read_text())
    expected_fields = {"RUST-MAP:" + row["id"] for row in boundary["fields"]}
    expected_count = 431
    if is_v2:
        delta = json.loads((ROOT / "docs/architecture/database/v2/field-dictionary.json").read_text())
        expected_fields |= {"RUST-MAP:" + row["id"] for row in delta["fields"]}
        expected_count += 29
    if is_v3:
        delta = json.loads((ROOT / "docs/architecture/database/v3/field-dictionary.json").read_text())
        expected_fields |= {"RUST-MAP:" + row["id"] for row in delta["fields"]}
        expected_count += len(delta["fields"])
        from storage_acceptance import (_validate_v3_delta_receipt, _read, _path,
                                        _source_hash, STORAGE_ADAPTERS)
        registry = json.loads((ROOT / "docs/architecture/database/v3/storage-acceptance.json").read_text())
        errors.extend(v3_source_map(ROOT)["errors"])
        # The final test may assemble its proved rows before libtest reports a
        # failure elsewhere in the same unfiltered target. A JSON receipt alone
        # must never turn a failed real execution into a green supplemental audit.
        if _source_hash(ROOT) != receipt.get("source_tree_sha256"):
            errors.append("schema3 audit requires the exact current complete source tree")
        stages = []
        for stage in ("compile", "run"):
            try:
                process = _read(_path(root, stage + ".process.json"))
                expected_command = STORAGE_ADAPTERS["rust-storage-complete-v3"][stage + "_command"]
                if (process.get("stage") != stage or process.get("command") != expected_command
                        or process.get("status") != "passed" or type(process.get("exit_code")) is not int
                        or process["exit_code"] != 0 or process.get("timed_out") is not False
                        or type(process.get("pid")) is not int or process["pid"] <= 0):
                    errors.append("schema3 actual " + stage + " process did not complete successfully")
                for key in ("run_id", "execution_nonce", "source_tree_sha256"):
                    if not isinstance(receipt.get(key), str) or not receipt[key] or process.get(key) != receipt[key]:
                        errors.append("schema3 actual process belongs to another execution: " + key)
                started, finished = process.get("started_at_ms"), process.get("finished_at_ms")
                if type(started) is not int or type(finished) is not int or finished < started:
                    errors.append("schema3 actual process interval is missing or invalid")
                stages.append(process)
            except (OSError, ValueError, TypeError, KeyError) as exc:
                errors.append("schema3 actual process proof: " + str(exc))
        if len(stages) == 2 and all(type(s.get(k)) is int for s in stages for k in ("started_at_ms", "finished_at_ms")):
            if stages[0]["finished_at_ms"] > stages[1]["started_at_ms"]:
                errors.append("schema3 actual compile/run phases overlap or are reversed")
            if not (receipt.get("started_at_ms", -1) <= stages[0]["started_at_ms"]
                    and receipt.get("finished_at_ms", -1) <= stages[1]["finished_at_ms"]):
                errors.append("schema3 receipt is outside the actual command interval")
        errors.extend(_validate_v3_delta_receipt(ROOT, root, receipt, registry["supported_upgrade_matrix"]))
    if len(fields) != expected_count or {row.get("id") for row in fields} != expected_fields:
        errors.append(f"complete {expected_count}-field producer proof required")
    for row in fields:
        if (row.get("status") != "passed" or type(row.get("assertions")) is not int
                or row["assertions"] <= 0 or row.get("skipped") != 0):
            errors.append("field has no actual non-skipped assertion: " + str(row.get("id")))
        if not isinstance(row.get("source"), str) or not row["source"].strip():
            errors.append("field lacks its concrete producer invariant: " + str(row.get("id")))
    return dict(kind="storage_runtime_proof_audit", status="failed" if errors else "passed",
                scope="supplemental proof audit; independent schema/ledger executor remains mandatory",
                run_id=receipt.get("run_id"), source_tree_sha256=receipt.get("source_tree_sha256"),
                asserted_cases=len(cases), typed_fields=len(fields), errors=errors)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execution", type=Path)
    parser.add_argument("--schema3", action="store_true", help="Inspect the separate schema3 assertion-source mapping")
    args = parser.parse_args()
    try:
        result = audit_execution(args.execution) if args.execution else v3_source_map() if args.schema3 else source_map()
    except (OSError, ValueError, TypeError, KeyError) as exc:
        result = dict(kind="storage_runtime_proof_audit", status="failed", errors=[str(exc)])
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return int(result["status"] != "passed")


if __name__ == "__main__":
    raise SystemExit(main())
