#!/usr/bin/env python3
"""Fail-closed storage acceptance registry and real Rust adapter executor.

Design checks are not production migrations. Code registers the actual locked
Rust producers; neither verification metadata nor receipts may add commands.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sqlite3
import subprocess
import time
from typing import Any
import uuid

from gate_core import source_files
from package_identity import inspect_package
from storage_product_process import execute as execute_storage_stage

ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = "docs/development/storage-acceptance.json"
BOUNDARY_PATH = "docs/architecture/database/field-boundaries.json"
SCHEMA_PATH = "docs/architecture/database/schema-contract.json"
V2_REGISTRY_PATH = "docs/architecture/database/v2/storage-acceptance.json"
V2_SCHEMA_PATH = "docs/architecture/database/v2/schema-contract.json"
V3_REGISTRY_PATH = "docs/architecture/database/v3/storage-acceptance.json"
V3_SCHEMA_PATH = "docs/architecture/database/v3/schema-contract.json"
# Explicit original-host prerequisite for this reviewed macOS acceptance run.
# A manifest cannot enroll another executable by declaring its own hash. Other
# machine artifacts require preparation and a separate reviewed registration.
ORIGINAL_V2_READER_SHA256 = "3c5723b2987cffa3ce99ba2915b685cf4a563046decdf8224eed9a1db288cea6"
CHECK_IDS = {f"STORAGE-{index:02}" for index in range(1, 23)}
OWNERS = {"AP", "PL", "AI", "JA"}
# Only code registers commands. Neither verification.json nor a receipt can add
# a command. Guard fixtures are never registered here.
STORAGE_ADAPTERS: dict[str, dict[str, Any]] = {
    "rust-storage-complete-v1": {
        "adapter_version": "rust-storage-complete-v1",
        "compile_timeout_seconds": 600,
        "run_timeout_seconds": 1800,
        "rust_test_target": "storage_product_acceptance",
        "sqlite_version": "3.53.2",
        "sqlite_source_id": "2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24",
        "migration_sources": ["crates/app-infra/src/storage/database.rs"],
        "test_sources": [
            "crates/app-infra/tests/storage_product_acceptance.rs",
            "crates/app-infra/tests/application_runtime.rs",
            "crates/app-infra/tests/storage_acceptance_support/evidence.rs",
            "crates/app-infra/tests/storage_acceptance_support/receipt.rs",
            "crates/app-infra/tests/storage_acceptance_support/fields.rs",
            "crates/app-infra/tests/storage_acceptance_support/dictionary.rs",
            "crates/app-infra/tests/storage_acceptance_support/resource_rejections.rs",
            "crates/app-infra/tests/storage_acceptance_support/frozen_hosts.rs",
            "crates/app-infra/src/storage/database_fault_tests.rs",
            "crates/app-infra/tests/storage_product.rs",
            "crates/app-infra/tests/storage_acceptance_routes.rs",
            "crates/app-infra/tests/storage_field_boundaries.rs",
            "crates/app-infra/tests/storage_field_mapping.rs",
            "crates/app-infra/tests/storage_credential_crash.rs",
            "crates/app-infra/tests/storage_disk_full.rs",
            "crates/app-infra/tests/configuration_lifecycle.rs",
            "crates/app-infra/tests/history_queries.rs",
            "crates/app-infra/tests/model_default_templates.rs",
            "crates/app-infra/tests/storage_schema_contract.rs",
            "crates/app-infra/tests/storage_cache_semantics.rs",
            "crates/app-infra/src/storage/database.rs",
            "crates/app-infra/src/storage/queries/google_authorization.rs",
            "crates/app-infra/src/dictionary/mod.rs",
            "crates/app-infra/src/providers/mod.rs",
            "crates/app-infra/src/providers/google_execution.rs",
            "crates/app-infra/src/providers/typed_http.rs",
            "crates/app-infra/src/providers/typed_http_tests.rs",
            "crates/app-infra/src/hook_backend.rs",
            "crates/app-infra/src/hook_backend/preview_identity.rs",
            "crates/app-infra/src/platform/game_launch.rs",
            "crates/app-infra/src/language/mod.rs",
            "crates/app-infra/src/language/tests.rs",
            "crates/app-infra/src/offline_translation/inspection.rs",
        ],
        "compile_command": ["cargo", "test", "--locked", "--package", "app-infra", "--lib", "--test", "storage_product_acceptance", "--no-run"],
        "run_command": ["cargo", "test", "--locked", "--package", "app-infra", "--lib", "--test", "storage_product_acceptance", "--", "--nocapture", "--test-threads=1"],
    }
}
# Both target closures compile the current Rust tests. This does not let the
# historical v1 adapter accept a schema2 database or claim the extra fields.
STORAGE_ADAPTERS["rust-storage-complete-v1"]["test_sources"].extend([
    "crates/app-infra/tests/storage_session_v2.rs",
    "crates/app-infra/tests/session_export_delivery.rs",
    "crates/app-infra/tests/session_records_batch.rs",
    "crates/app-infra/src/platform/session_export.rs",
    "crates/app-infra/src/storage/closure_integrity_tests.rs",
    "crates/app-infra/src/providers/adapters/translation.rs",
])
# Schema two retains all original runtime obligations and adds independent delta proofs.
STORAGE_ADAPTERS["rust-storage-complete-v2"] = {
    **STORAGE_ADAPTERS["rust-storage-complete-v1"],
    "adapter_version": "rust-storage-complete-v2",
    "registry_path": V2_REGISTRY_PATH,
    "schema_contract": V2_SCHEMA_PATH,
    "rust_test_target": "storage_product_acceptance_v2",
    "test_sources": list(STORAGE_ADAPTERS["rust-storage-complete-v1"]["test_sources"]) + [
        "crates/app-infra/tests/storage_product_acceptance_v2.rs",
        "crates/app-infra/tests/scoped_preferences.rs",
    ],
    "compile_command": ["cargo", "test", "--locked", "--package", "app-infra", "--lib", "--test", "storage_product_acceptance_v2", "--no-run"],
    "run_command": ["cargo", "test", "--locked", "--package", "app-infra", "--lib", "--test", "storage_product_acceptance_v2", "--", "--nocapture", "--test-threads=1"],
}

# The current HTTP identity delta runs the complete existing target and its
# independent schema-three producers in one unfiltered serial execution.
STORAGE_ADAPTERS["rust-storage-complete-v3"] = {
    **STORAGE_ADAPTERS["rust-storage-complete-v2"],
    "adapter_version": "rust-storage-complete-v3",
    "registry_path": V3_REGISTRY_PATH,
    "schema_contract": V3_SCHEMA_PATH,
    "rust_test_target": "storage_product_acceptance_v3",
    "test_sources": list(STORAGE_ADAPTERS["rust-storage-complete-v2"]["test_sources"]) + [
        "crates/app-infra/tests/storage_product_acceptance_v3.rs",
        "crates/app-infra/tests/storage_http_identity_v3.rs",
        "crates/app-infra/tests/storage_http_identity_crash.rs",
        "crates/app-infra/tests/provider_credential_envelopes.rs",
        "crates/app-infra/tests/provider_gateway_identity.rs",
    ],
    "compile_command": ["cargo", "test", "--locked", "--package", "app-infra", "--lib", "--test", "storage_product_acceptance_v3", "--no-run"],
    "run_command": ["cargo", "test", "--locked", "--package", "app-infra", "--lib", "--test", "storage_product_acceptance_v3", "--", "--nocapture", "--test-threads=1"],
}

def _registry_path(root: Path) -> str:
    if (root / V3_REGISTRY_PATH).is_file():
        return V3_REGISTRY_PATH
    return V2_REGISTRY_PATH if (root / V2_REGISTRY_PATH).is_file() else REGISTRY_PATH

def v2_check_ids(root: Path) -> set[str]:
    from storage_schema_v2 import validate_schema_contract, DICTIONARY
    errors = validate_schema_contract(root)
    if errors:
        raise ValueError("; ".join(errors))
    fields = _read(_path(root, DICTIONARY))["fields"]
    return ({"V2-PRODUCERS", "V2-RELATIONSHIPS", "V2-MIGRATION", "V2-EXPORT",
             "V2-UNBIND-SERIALIZATION", "V2-UNBIND-IDENTITY", "V2-UNBIND-COMPENSATION"}
            | {"RUST-MAP:" + row["id"] for row in fields}
            | {"V2-FIELD:" + row["id"] for row in fields})

STORAGE_MODULES = {"M01", "M03", "M04", "M08", "M10", "M11", "M16"}
STORAGE_PATHS = (
    "crates/app-infra/src/storage/", "crates/app-infra/src/dictionary/",
    "crates/app-infra/src/credentials/", "crates/app-infra/tests/",
    "crates/app-core/src/domain/library/", "crates/app-core/src/domain/reading/",
    "crates/app-core/src/domain/model_config/", "crates/app-core/src/domain/preferences/",
    "crates/app-core/src/application/history/", "resources/language/",
    "docs/architecture/database/",
)
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*\Z")
REJECTION_ERRORS = {
    "supported": {"unsupported_schema"},
    "unversioned": {"unsupported_schema"},
    "unknown_version": {"unsupported_schema"},
    "corrupt": {"unsupported_schema", "database_corrupt", "storage_failure"},
    "migration_mismatch": {"migration_mismatch"},
    "schema_mismatch": {"schema_contract_mismatch"},
    "relationship_corrupt": {"database_relationship_corrupt"},
}


def _hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _source_hash(root: Path) -> str:
    return hashlib.sha256(json.dumps(source_files(root), sort_keys=True).encode()).hexdigest()


def _read(path: Path) -> dict[str, Any]:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    value = json.loads(path.read_text(), object_pairs_hook=pairs,
                       parse_constant=lambda value: (_ for _ in ()).throw(ValueError("non-JSON number")))
    if not isinstance(value, dict):
        raise ValueError("expected JSON object: " + str(path))
    return value


def _path(root: Path, value: Any, *, exists=True) -> Path:
    root = root.resolve()
    if not isinstance(value, str) or not value or "\\" in value:
        raise ValueError("expected normalized relative evidence/source path")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or path.as_posix() != value or "." in path.parts:
        raise ValueError("unsafe path: " + value)
    target = root / path
    for parent in (target, *target.parents):
        if parent == root:
            break
        if parent.is_symlink():
            raise ValueError("symbolic link is forbidden: " + value)
    if not target.resolve().is_relative_to(root):
        raise ValueError("path escapes root: " + value)
    if exists and not target.is_file():
        raise ValueError("missing regular file: " + value)
    return target


def _nonempty_strings(value: Any) -> bool:
    return (isinstance(value, list) and bool(value)
            and all(isinstance(item, str) and item.strip() == item and item for item in value)
            and len(value) == len(set(value)))


def boundary_check_ids(root: Path) -> set[str]:
    """Derive every probe and Rust field mapping from the one boundary registry."""
    boundary = _read(_path(root, BOUNDARY_PATH))
    if boundary.get("kind") != "storage_field_boundary_registry":
        raise ValueError("invalid field boundary registry kind")
    fields, probes = boundary.get("fields"), boundary.get("probes")
    if not isinstance(fields, list) or not isinstance(probes, list) or not fields or not probes:
        raise ValueError("field boundary registry requires fields and probes")
    field_ids = [row.get("id") for row in fields if isinstance(row, dict)]
    probe_ids = [row.get("id") for row in probes if isinstance(row, dict)]
    if not _nonempty_strings(field_ids) or len(field_ids) != len(fields):
        raise ValueError("field identities must be nonempty and unique")
    if not _nonempty_strings(probe_ids) or len(probe_ids) != len(probes):
        raise ValueError("boundary probe identities must be nonempty and unique")
    from database_design import build_metadata
    metadata, errors = build_metadata(root)
    expected = {f"{db['name']}.{table['name']}.{field['name']}"
                for db in metadata.get("databases", []) for table in db["tables"] for field in table["fields"]}
    if errors or set(field_ids) != expected:
        raise ValueError("field boundary identities differ from the executable design DDL")
    known_probes = set(probe_ids)
    if any(not _nonempty_strings(row.get("probes")) or not set(row["probes"]) <= known_probes for row in fields):
        raise ValueError("every field must reference its actual boundary probes")
    result = known_probes | {"RUST-MAP:" + identifier for identifier in field_ids}
    if len(result) != len(probe_ids) + len(field_ids) or result & CHECK_IDS:
        raise ValueError("storage subcheck identities collide")
    return result


def _design_checks(root: Path) -> list[str]:
    """Standalone runtime must not depend on having called the parent gate first."""
    from storage_schema import validate_schema_contract
    from storage_boundaries import validate
    errors = list(validate_schema_contract(root))
    if _registry_path(root) in {V2_REGISTRY_PATH, V3_REGISTRY_PATH}:
        from storage_schema_v2 import validate_schema_contract as validate_v2
        errors.extend(validate_v2(root))
    if _registry_path(root) == V3_REGISTRY_PATH:
        from storage_schema_v3 import validate_schema_contract as validate_v3
        from storage_boundaries_v3 import assess as assess_v3_boundaries
        errors.extend(validate_v3(root))
        delta = assess_v3_boundaries(root)
        if delta.get("status") != "passed":
            errors.extend("schema3 boundary contract: " + str(error) for error in delta.get("errors", []))
            if not delta.get("errors"):
                errors.append("schema3 field boundary contract did not pass")
    receipt = validate(root)
    if not isinstance(receipt, dict) or receipt.get("status") != "passed":
        details = receipt.get("errors", []) if isinstance(receipt, dict) else []
        errors.extend("field boundary contract: " + str(error) for error in details)
        if not details:
            errors.append("field boundary contract did not actually pass")
    return errors


def _v2_registry_errors(root: Path, data: dict[str, Any]) -> list[str]:
    """A delta may add guarantees, never rewrite the original 22/431 obligations."""
    errors = []
    original = _read(_path(root, REGISTRY_PATH))
    for key in ("checks", "boundary_contract", "plan", "prerequisite_ids", "maintenance_ids", "enablement_ids", "runtime_policy", "required_fresh_routes"):
        if data.get(key) != original.get(key):
            errors.append("schema2 must retain every original storage obligation: " + key)
    matrix = data.get("supported_upgrade_matrix", {})
    old_matrix = original["supported_upgrade_matrix"]
    review = _read(_path(root, "docs/architecture/database/v2/storage-review.json"))
    from storage_schema_v2 import DESIGN_SHA256
    expected_review = {
        "kind": "independent_schema_two_storage_implementation_review",
        "reviewed_design_sha256": DESIGN_SHA256,
        "prior_registry_sha256": _hash(_path(root, REGISTRY_PATH)),
        "schema_contract": V2_SCHEMA_PATH,
        "schema_contract_sha256": _hash(_path(root, V2_SCHEMA_PATH)),
        "production_migration_source": "crates/app-infra/src/storage/database.rs",
        "production_migration_source_sha256": _hash(_path(root, "crates/app-infra/src/storage/database.rs")),
        "old_field_count": 431, "new_field_count": 29, "status": "independent_qa_reviewed",
    }
    for key, value in expected_review.items():
        if review.get(key) != value:
            errors.append("schema2 independent implementation review differs: " + key)
    chain = matrix.get("migration_chain", [])
    if not isinstance(chain,list) or len(chain) != 2:
        return errors + ["schema2 must retain migration one and add exactly reviewed migration two"]
    old_entry = old_matrix["migration_chain"][0]
    for key in ("version", "previous_version", "source_path", "sql_sources", "migration_sha256"):
        if chain[0].get(key) != old_entry[key]:
            errors.append("schema2 changed immutable migration-one identity: " + key)
    if (chain[1].get("version") != 2 or chain[1].get("previous_version") != 1
            or chain[1].get("migration_sha256") != DESIGN_SHA256
            or chain[1].get("sql_sources") != [{"path":"crates/app-infra/src/storage/migrations/002_session_records.sql", "sha256":DESIGN_SHA256}]):
        errors.append("schema2 migration must bind the independently reviewed exact SQL delta")
    if matrix.get("fresh_versions") != {"profile":2,"dictionary":1} or matrix.get("dictionary_versions") != old_matrix["dictionary_versions"]:
        errors.append("schema2 must preserve the dictionary resource/version identity")
    import copy
    expected_routes=copy.deepcopy(old_matrix["routes"])
    for row in expected_routes:
        if row["from_version"]==1:
            row["from_version"]=2
        row["to_version"]=2
        for key in ("source_schema","target_schema"):
            if row[key]:row[key]=V2_SCHEMA_PATH
    if matrix.get("routes") != expected_routes:
        errors.append("schema2 must preserve every real route with its exact current version and schema")
    return errors


def validate_registry(root: Path, data: dict[str, Any] | None = None) -> list[str]:
    root = Path(root).resolve()
    errors: list[str] = []
    try:
        data = _read(_path(root, _registry_path(root))) if data is None else data
        if data.get("id") == "FP1-STORAGE-20261105-SCHEMA3-HTTP":
            from storage_schema_v3 import validate_acceptance_registry, v3_check_ids
            errors.extend(validate_acceptance_registry(root, data))
            errors.extend(_design_checks(root))
            # These inventories remain mandatory and are executed afresh. The
            # new exact independent registry retains their original definitions.
            boundary_check_ids(root)
            v2_check_ids(root)
            v3_check_ids(root)
            return errors
        is_v2 = data.get("id") == "FP1-STORAGE-20261020-SCHEMA2-R1"
        schema_path = V2_SCHEMA_PATH if is_v2 else SCHEMA_PATH
        registry_id = "FP1-STORAGE-20261020-SCHEMA2-R1" if is_v2 else "FP1-STORAGE-20261019-R1"
        plan = _read(_path(root, "docs/development/first-product-test-plan.json"))
        if not isinstance(data, dict):
            return ["storage registry must be an object"]
        if type(data.get("schema_version")) is not int or data["schema_version"] != 1:
            errors.append("storage schema_version must be integer 1")
        for key, value in (("id", registry_id), ("kind", "storage_acceptance_registry"),
                           ("phase", "planning"), ("product_tests_executed", False),
                           ("boundary_contract", BOUNDARY_PATH), ("schema_contract", schema_path),
                           ("plan", "docs/development/first-product-test-plan.json")):
            if data.get(key) != value or key == "product_tests_executed" and data.get(key) is not False:
                errors.append(f"storage {key}: unreviewed identity or execution claim")
        expected_refs = {"prerequisite_ids": {"PRE-13", "PRE-14"},
                         "maintenance_ids": {"MA-02", "MA-05", "MA-10", "MA-16"},
                         "enablement_ids": {"TE-04", "TE-06", "TE-08", "TE-10", "TE-12"},
                         "required_fresh_routes": {"fresh_profile", "fresh_dictionary"}}
        for key, values in expected_refs.items():
            if not _nonempty_strings(data.get(key)) or set(data[key]) != values:
                errors.append(f"storage {key}: missing or unknown ownership/prerequisite/route")
        from test_plan_contract import required_executions
        executions = required_executions(plan)
        checks = data.get("checks")
        if not isinstance(checks, list):
            return errors + ["storage checks must be a nonempty list"]
        identifiers = [row.get("id") for row in checks if isinstance(row, dict)]
        if not _nonempty_strings(identifiers) or len(identifiers) != len(checks) or set(identifiers) != CHECK_IDS:
            errors.append("storage checks: missing/duplicate/unknown required storage acceptance subcheck")
        covered = set()
        for row in checks:
            if not isinstance(row, dict):
                errors.append("storage check must be object")
                continue
            if set(row) != {"id", "title", "owner", "execution_ids", "expected_evidence"}:
                errors.append("storage check has missing or unknown fields")
            if row.get("owner") not in OWNERS or not isinstance(row.get("title"), str) or not row["title"].strip():
                errors.append("storage check requires a known owner and title")
            refs = row.get("execution_ids")
            if not _nonempty_strings(refs) or not set(refs) <= executions:
                errors.append(f"storage {row.get('id')}: unknown or missing case@variant")
            else:
                covered.update(refs)
            if not _nonempty_strings(row.get("expected_evidence")):
                errors.append(f"storage {row.get('id')}: expected evidence is mandatory")
        required_coverage = {f"TP-{index:02}@INFRA" for index in (1, 2, 3, 4, 5, 6, 7, 8, 9, 14, 25)} | {
            "TP-10@UNIT", "TP-11@UNIT", "TP-12@PROTOCOL", "TP-16@UNIT", "TP-17@PROTOCOL",
            "TP-19@NATIVE", "TP-24@DISTRIBUTION", "TP-27@UNIT", "TP-30@DISTRIBUTION",
            "MC-16@NATIVE", "MC-21@NATIVE", "CR-08-CONFIG@NATIVE", "CR-08-HISTORY@NATIVE",
            "CR-09-UPGRADE@DISTRIBUTION", "CR-12-RESOURCE@NATIVE", "CR-12-PERSIST@NATIVE"}
        if not required_coverage <= covered:
            errors.append("storage required execution links are missing: " + ",".join(sorted(required_coverage - covered)))
        matrix = data.get("supported_upgrade_matrix")
        if not isinstance(matrix, dict) or set(matrix) != {"status", "approval_ref", "fresh_versions", "migration_chain", "dictionary_versions", "routes"}:
            errors.append("supported_upgrade_matrix: explicit pending/frozen matrix is required")
        elif matrix.get("status") == "pending_review":
            if (matrix.get("routes") != [] or matrix.get("approval_ref") is not None or matrix.get("fresh_versions") is not None
                    or matrix.get("migration_chain") != [] or matrix.get("dictionary_versions") != {}):
                errors.append("pending migration matrix cannot invent supported versions or approval")
        elif matrix.get("status") == "frozen":
            if not isinstance(matrix.get("approval_ref"), str):
                errors.append("frozen migration matrix requires an approved PRE-14 artifact")
            else:
                _path(root, matrix["approval_ref"])
            versions = matrix.get("fresh_versions")
            if (not isinstance(versions, dict) or set(versions) != {"profile", "dictionary"}
                    or any(type(value) is not int or value <= 0 for value in versions.values())):
                errors.append("frozen fresh database versions must be explicit positive integers")
            chain = matrix.get("migration_chain")
            previous = 0
            if not isinstance(chain, list) or not chain:
                errors.append("frozen profile migration source/hash/previous chain is required")
            else:
                for entry in chain:
                    if (not isinstance(entry, dict) or set(entry) != {"version", "previous_version", "source_path", "source_sha256", "sql_sources", "migration_sha256"}
                            or type(entry.get("version")) is not int or entry["version"] <= previous
                            or entry.get("previous_version") != previous):
                        errors.append("profile migration chain must be positive, increasing and linked")
                        continue
                    source = _path(root, entry["source_path"])
                    if (not entry["source_path"].startswith("crates/app-infra/src/storage/") or source.suffix != ".rs"
                            or entry["source_sha256"] != _hash(source)):
                        errors.append("frozen migration source must bind actual production Rust and its hash")
                    sql_sources = entry.get("sql_sources")
                    if (not isinstance(sql_sources, list) or not sql_sources
                            or len({row.get("path") for row in sql_sources if isinstance(row, dict)}) != len(sql_sources)):
                        errors.append("frozen migration must bind its ordered SQL content sources separately")
                    else:
                        sql_bytes = []
                        for sql in sql_sources:
                            if not isinstance(sql, dict) or set(sql) != {"path", "sha256"}:
                                errors.append("frozen migration SQL source requires path and content hash")
                                continue
                            sql_path = _path(root, sql["path"])
                            if (not sql["path"].startswith("crates/app-infra/src/storage/migrations/")
                                    or sql_path.suffix != ".sql" or sql["sha256"] != _hash(sql_path)):
                                errors.append("frozen migration SQL source/hash differs from production content")
                            sql_bytes.append(sql_path.read_bytes())
                        if entry.get("migration_sha256") != hashlib.sha256(b"\n".join(sql_bytes)).hexdigest():
                            errors.append("migration ledger hash must bind ordered SQL content, not Rust source bytes")
                    previous = entry["version"]
                if isinstance(versions, dict) and previous != versions.get("profile"):
                    errors.append("fresh profile version must equal the frozen migration chain head")
            dictionaries = matrix.get("dictionary_versions")
            if not isinstance(dictionaries, dict) or not dictionaries:
                errors.append("frozen dictionary index versions and actual resource manifests are required")
            else:
                for version, resource in dictionaries.items():
                    if (not isinstance(version, str) or not re.fullmatch(r"[1-9][0-9]*", version)
                            or not isinstance(resource, dict) or set(resource) != {"resource_version", "manifest_path", "manifest_sha256"}
                            or not isinstance(resource.get("resource_version"), str) or not resource["resource_version"].strip()
                            or not isinstance(resource.get("manifest_sha256"), str) or not SHA256.fullmatch(resource["manifest_sha256"])):
                        errors.append("dictionary index/manifest version mapping is invalid")
                        continue
                    manifest_path = _path(root, resource["manifest_path"])
                    if (not resource["manifest_path"].startswith("resources/language/")
                            or manifest_path.suffix != ".json" or resource["manifest_sha256"] != _hash(manifest_path)):
                        errors.append("dictionary provenance manifest path/hash differs from actual resource")
                    else:
                        manifest = _read(manifest_path)
                        if (manifest.get("index_schema_version") != int(version)
                                or manifest.get("resource_version") != resource["resource_version"]):
                            errors.append("dictionary provenance version differs from frozen index mapping")
                if isinstance(versions, dict) and str(versions.get("dictionary")) not in dictionaries:
                    errors.append("fresh dictionary version has no pinned resource manifest")
            routes = matrix.get("routes")
            if not isinstance(routes, list) or not routes:
                errors.append("frozen matrix requires supported upgrade/rejection/recovery routes")
            else:
                ids = [route.get("id") for route in routes if isinstance(route, dict)]
                if not _nonempty_strings(ids) or len(ids) != len(routes):
                    errors.append("migration route identities must be unique")
                for route in routes:
                    if not isinstance(route, dict) or set(route) != {"id", "kind", "database", "from_version", "to_version", "source_state", "source_schema", "target_schema", "expected", "expected_error"}:
                        errors.append("migration route requires exact explicit version span and expected outcome")
                    elif (route["kind"] not in {"upgrade", "reopen", "compatibility_rejection", "downgrade_rejection", "paired_recovery"}
                          or route["database"] not in {"profile", "dictionary"}
                          or route["expected"] not in {"accepted", "rejected"}
                          or not isinstance(route["id"], str) or not route["id"].strip()
                          or type(route["to_version"]) is not int or route["to_version"] <= 0):
                        errors.append("invalid supported migration route")
                    else:
                        state = route["source_state"]
                        if state not in {"supported", "unversioned", "unknown_version", "corrupt", "migration_mismatch", "schema_mismatch", "relationship_corrupt"}:
                            errors.append("migration route requires an explicit recognized source state")
                        unversioned = state in {"unversioned", "corrupt"}
                        if (unversioned and route["from_version"] is not None
                                or not unversioned and (type(route["from_version"]) is not int or route["from_version"] <= 0)):
                            errors.append("source version must be null for unversioned/corrupt data and positive for versioned data")
                        unknown_shape = state in {"unversioned", "unknown_version", "corrupt", "schema_mismatch"}
                        if unknown_shape and route["source_schema"] is not None:
                            errors.append("unknown or corrupt source may not claim an approved schema contract")
                        elif not unknown_shape:
                            _path(root, route["source_schema"])
                        _path(root, route["target_schema"])
                        if route["expected"] == "rejected":
                            if (state == "supported" and route["kind"] != "downgrade_rejection"
                                    or route["kind"] not in {"compatibility_rejection", "downgrade_rejection"}
                                    or not isinstance(route["expected_error"], str) or not NAME.fullmatch(route["expected_error"])):
                                errors.append("rejection route requires a rejected source state and exact production error code")
                            if route["expected_error"] not in REJECTION_ERRORS.get(state, set()):
                                errors.append("rejection error code is incompatible with its declared source state")
                        elif (state != "supported" or route["expected_error"] is not None
                                or route["kind"] not in {"upgrade", "reopen", "paired_recovery"}):
                            errors.append("accepted routes require supported data and no error code")
                        if route["kind"] in {"reopen", "paired_recovery"} and route["from_version"] != route["to_version"]:
                            errors.append("same-version reopen/recovery cannot imply a nonexistent migration")
                        if route["id"] in {"fresh_profile", "fresh_dictionary"}:
                            errors.append("supported migration routes cannot replace the required fresh routes")
        else:
            errors.append("migration matrix status must be pending_review or frozen")
        policy = data.get("runtime_policy")
        if not isinstance(policy, dict) or policy != {
            "adapter_status": "not_implemented", "field_boundary_expansion": "all_probe_ids",
            "rust_mapping_expansion": "every_field_id_with_RUST-MAP-prefix",
            "prohibited_substitutes": ["design_ddl_rebuild", "guard_fixture", "plan_lint", "existing_poc", "hash_only_receipt"],
            "product_status_source": "separate_run_receipts_only"}:
            errors.append("storage runtime policy cannot promote adapter/guard fixtures to product acceptance")
        boundary_check_ids(root)
        if is_v2:
            errors.extend(_v2_registry_errors(root,data))
            v2_check_ids(root)
        errors.extend(_design_checks(root))
    except (OSError, ValueError, KeyError, TypeError) as exc:
        errors.append("storage registry input: " + str(exc))
    return errors


def _version_errors(path: Path, database: str, expected_version: int, matrix: dict[str, Any]) -> list[str]:
    """Inspect data versions independently of schema shape and receipt metadata."""
    errors = []
    connection = None
    try:
        from storage_schema import sealed_snapshot_errors
        errors.extend(sealed_snapshot_errors(path))
        if errors:
            return errors
        connection = sqlite3.connect(path.resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
        connection.execute("PRAGMA query_only=ON")
        if database == "profile":
            rows = connection.execute("SELECT version,previous_version,migration_sha256 FROM schema_migrations ORDER BY version").fetchall()
            expected_chain = [(entry["version"], entry["previous_version"], entry["migration_sha256"])
                              for entry in matrix["migration_chain"] if entry["version"] <= expected_version]
            if not rows or rows[-1][0] != expected_version or rows != expected_chain:
                errors.append("actual profile migration version/previous/source hash chain differs from frozen matrix")
        else:
            rows = connection.execute("SELECT index_schema_version,resource_version,manifest_sha256 FROM dictionary_meta WHERE singleton=1").fetchall()
            expected_resource = matrix["dictionary_versions"].get(str(expected_version))
            expected_row = (expected_version, expected_resource["resource_version"], expected_resource["manifest_sha256"]) if expected_resource else None
            if rows != [expected_row]:
                errors.append("actual dictionary index/resource/manifest version differs from frozen matrix")
    except (sqlite3.Error, OSError, ValueError, KeyError, TypeError) as exc:
        errors.append("actual database version inspection failed: " + str(exc))
    finally:
        if connection is not None:
            connection.close()
    return errors


def _rejected_source_errors(root: Path, path: Path, spec: dict[str, Any], matrix: dict[str, Any]) -> list[str]:
    """Inspect the rejected input without treating unknown bytes as a valid old schema.

    The production Rust test must still assert its actual error code. This
    independent reader verifies that a route really supplied the declared bad
    input, so a valid v1 database cannot stand in for an unversioned/corrupt one.
    """
    state = spec["source_state"]
    if state == "supported":
        return _version_errors(path, spec["database"], spec["from_version"], matrix)
    if spec["database"] != "profile":
        return ["unsupported rejected dictionary source inspector"]
    connection = None
    try:
        from storage_schema import sealed_snapshot_errors
        sealed_errors = sealed_snapshot_errors(path)
        if sealed_errors:
            return sealed_errors
        connection = sqlite3.connect(path.resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
        connection.execute("PRAGMA query_only=ON")
        if state == "corrupt":
            try:
                integrity = connection.execute("PRAGMA integrity_check").fetchall()
                if integrity != [("ok",)]:
                    return []
            except sqlite3.DatabaseError:
                return []
            return ["corrupt rejection route supplied an intact SQLite database"]
        if connection.execute("PRAGMA integrity_check").fetchall() != [("ok",)]:
            return ["non-corrupt rejection source failed SQLite integrity inspection"]
        try:
            version = connection.execute("SELECT max(version) FROM schema_migrations").fetchone()[0]
        except sqlite3.DatabaseError:
            if state == "unversioned":
                return []
            raise
        if state == "unversioned":
            return [] if version is None else ["unversioned route actually has a migration version"]
        if version != spec["from_version"]:
            return ["rejected source actual version differs from declared source version"]
        if state == "unknown_version":
            known = {entry["version"] for entry in matrix["migration_chain"]}
            return [] if version not in known else ["unknown-version route supplied a supported version"]
        if state == "migration_mismatch":
            return [] if _version_errors(path, "profile", version, matrix) else ["migration-mismatch route supplied the valid frozen SQL ledger"]
        if state == "schema_mismatch":
            from storage_schema import compare_schema_file
            return [] if compare_schema_file(root, path, "profile", spec["target_schema"]) else ["schema-mismatch route supplied the approved schema"]
        if state == "relationship_corrupt":
            ledger_errors = _version_errors(path, spec["database"], version, matrix)
            if ledger_errors:
                return ["relationship-corrupt source has invalid migration ledger: " + error for error in ledger_errors]
            return [] if connection.execute("PRAGMA foreign_key_check").fetchone() is not None else ["relationship-corrupt route has no actual foreign key violation"]
        return ["unknown rejected source state"]
    except (sqlite3.Error, OSError, ValueError, KeyError, TypeError) as exc:
        return ["rejected source inspection failed: " + str(exc)]
    finally:
        if connection is not None:
            connection.close()


def _test_source_closure(root: Path, target: str, include_library: bool) -> set[str]:
    """Enumerate review sources, never assertion outcomes or mapping proofs."""
    pending = [f"crates/app-infra/tests/{target}.rs"]
    if include_library:
        for path in (root / "crates/app-infra/src").rglob("*.rs"):
            if re.search(r"#\[\s*(?:tokio::)?test\b|#\[\s*cfg\(test\)\s*\]", path.read_text()):
                pending.append(path.relative_to(root).as_posix())
    seen = set()
    while pending:
        ref = pending.pop()
        if ref in seen:
            continue
        source = _path(root, ref)
        seen.add(ref)
        for relative in re.findall(r'#\[\s*path\s*=\s*"([^"]+)"\s*\]', source.read_text()):
            unresolved = source.parent / relative
            if any(path.is_symlink() for path in (unresolved, *unresolved.parents)):
                raise ValueError("symbolic link in reviewed Rust test import")
            imported = unresolved.resolve()
            if not imported.is_relative_to(root.resolve()):
                raise ValueError("reviewed Rust test import escapes repository")
            pending.append(imported.relative_to(root.resolve()).as_posix())
    return seen


def _adapter_errors(root: Path, name: str, adapter: Any) -> list[str]:
    errors = []
    if not isinstance(adapter, dict):
        return [f"{name}: registered adapter must be an object"]
    if not NAME.fullmatch(name):
        errors.append("adapter name must be a safe identifier")
    target = adapter.get("rust_test_target")
    if not isinstance(target, str) or not NAME.fullmatch(target):
        errors.append(f"{name}: explicit Rust test target is required")
    if name in {"rust-storage-complete-v1", "rust-storage-complete-v2", "rust-storage-complete-v3"}:
        # Independent of the mutable registry object: a malformed registration
        # cannot validate itself by supplying the same filtered argv twice.
        required_target = {"rust-storage-complete-v1": "storage_product_acceptance",
                           "rust-storage-complete-v2": "storage_product_acceptance_v2",
                           "rust-storage-complete-v3": "storage_product_acceptance_v3"}[name]
        required = ["cargo", "test", "--locked", "--package", "app-infra", "--lib",
                    "--test", required_target]
        expected_commands = {"compile_command": required + ["--no-run"],
                             "run_command": required + ["--", "--nocapture", "--test-threads=1"]}
        if target != required_target:
            errors.append(f"{name}: closed production target must remain {required_target}")
        for key, expected in expected_commands.items():
            if adapter.get(key) != expected:
                errors.append(f"{name}: closed production argv differs for {key}; filters, omitted targets and alternate flags are forbidden")
    for key in ("compile_command", "run_command"):
        command = adapter.get(key)
        if not isinstance(command, list) or not command or any(not isinstance(part, str) or not part for part in command):
            errors.append(f"{name}: {key} must be code-registered argv")
            continue
        if Path(command[0]).name != "cargo" or "test" not in command or "--locked" not in command:
            errors.append(f"{name}: {key} must run actual locked Cargo test targets")
        for flag, value in (("--package", "app-infra"), ("--test", target)):
            if flag not in command or command.index(flag) + 1 >= len(command) or command[command.index(flag) + 1] != value:
                errors.append(f"{name}: {key} must select app-infra and the registered target")
        if "docs/architecture/database/" in " ".join(command) or any(part in {"--list", "--ignored", "--include-ignored", "--skip", "--exact"} for part in command):
            errors.append(f"{name}: design SQL/list/ignored mode cannot substitute runtime migration tests")
        if (key == "compile_command") != ("--no-run" in command):
            errors.append(f"{name}: compilation and actual execution must be distinct commands")
        if key == "run_command" and "--nocapture" not in command:
            errors.append(f"{name}: actual assertion markers require --nocapture")
    for stage in ("compile", "run"):
        budget = adapter.get(stage + "_timeout_seconds", 600)
        if type(budget) is not int or not 1 <= budget <= 1800:
            errors.append(f"{name}: {stage} execution requires an integer bounded timeout from 1 to 1800 seconds")
    for key in ("adapter_version", "sqlite_version", "sqlite_source_id"):
        if not isinstance(adapter.get(key), str) or not adapter[key].strip():
            errors.append(f"{name}: pinned {key} is required")
    for key in ("migration_sources", "test_sources"):
        refs = adapter.get(key)
        if not _nonempty_strings(refs):
            errors.append(f"{name}: {key} must identify actual reviewed implementation sources")
            continue
        for ref in refs:
            try:
                source = _path(root, ref)
                if not ref.startswith("crates/app-infra/") or source.suffix != ".rs":
                    errors.append(f"{name}: {key} must reference reviewed app-infra Rust source")
                if key == "migration_sources" and (not ref.startswith("crates/app-infra/src/") or "storage" not in ref and "dictionary" not in ref):
                    errors.append(f"{name}: migration_sources must reference production storage/dictionary entry points")
            except ValueError as exc:
                errors.append(str(exc))
    if isinstance(target, str) and NAME.fullmatch(target) and _nonempty_strings(adapter.get("test_sources")):
        try:
            closure = _test_source_closure(Path(root).resolve(), target, any("--lib" in adapter.get(key, []) for key in ("compile_command", "run_command")))
            missing = closure - set(adapter["test_sources"])
            if missing:
                errors.append(f"{name}: reviewed test_sources omits compiled test import closure: " + ",".join(sorted(missing)))
        except (OSError, ValueError) as exc:
            errors.append(f"{name}: test source closure: {exc}")
    return errors


def storage_requirement_gaps(root: Path, metadata: dict[str, Any], changed_paths: list[str], profile: str) -> list[str]:
    """Planning/tooling can pass; a storage implementation or release cannot fake readiness."""
    modules = {value for row in metadata.get("requirements", []) if isinstance(row, dict) for value in row.get("module_ids", [])}
    required = profile == "release" or metadata.get("kind") == "product" and (
        bool(modules & STORAGE_MODULES) or any(path.startswith(STORAGE_PATHS) for path in changed_paths))
    if not required:
        return []
    errors = validate_registry(root)
    ids = metadata.get("product_storage_checks")
    if not _nonempty_strings(ids):
        errors.append("Actual product storage adapter is required for storage implementation/release; design guards cannot substitute")
        ids = []
    for name in ids:
        if name not in STORAGE_ADAPTERS:
            errors.append("Product storage adapter is not implemented/registered: " + name)
        else:
            errors.extend(_adapter_errors(Path(root), name, STORAGE_ADAPTERS[name]))
    try:
        registry = _read(_path(Path(root), _registry_path(Path(root))))
        expected_status = "design_frozen_runtime_not_run" if registry.get("id") == "FP1-STORAGE-20261105-SCHEMA3-HTTP" else "frozen"
        if registry["supported_upgrade_matrix"]["status"] != expected_status:
            errors.append("PRE-14 supported schema/migration/recovery matrix is pending; actual storage acceptance is blocked")
    except (OSError, ValueError, KeyError, TypeError) as exc:
        errors.append("storage acceptance prerequisites: " + str(exc))
    return errors


def _validate_receipt(root: Path, output: Path, receipt: dict[str, Any], adapter: dict[str, Any],
                      run_id: str, started: int, finished: int, source_hash: str, artifact_hash: str | None,
                      stdout: str, expected_checks: set[str], expected_routes: dict[str, dict[str, Any]],
                      matrix: dict[str, Any], execution_nonce: str | None = None) -> list[str]:
    errors = []
    expected = {"schema_version": 1, "kind": "product_storage_runtime_receipt", "run_id": run_id,
                "status": "passed", "source_tree_sha256": source_hash, "artifact_sha256": artifact_hash,
                "adapter_version": adapter["adapter_version"], "sqlite_version": adapter["sqlite_version"],
                "sqlite_source_id": adapter["sqlite_source_id"]}
    for key, value in expected.items():
        if receipt.get(key) != value or key == "schema_version" and type(receipt.get(key)) is not int:
            errors.append("runtime receipt identity/status mismatch: " + key)
    if adapter["adapter_version"] == "rust-storage-complete-v3":
        if (not isinstance(execution_nonce, str) or re.fullmatch(r"[0-9a-f]{32}", execution_nonce) is None
                or receipt.get("execution_nonce") != execution_nonce):
            errors.append("schema3 runtime receipt must bind this fresh execution nonce")
    start, end = receipt.get("started_at_ms"), receipt.get("finished_at_ms")
    if type(start) is not int or type(end) is not int or not started <= start <= end <= finished:
        errors.append("runtime receipt must describe this fresh command execution interval")
    checks = receipt.get("checks")
    if not isinstance(checks, list):
        checks = []
        errors.append("runtime checks must be an explicit complete list")
    ids = [row.get("id") for row in checks if isinstance(row, dict)]
    if len(ids) != len(checks) or len(ids) != len(set(str(value) for value in ids)) or set(ids) != expected_checks:
        errors.append("runtime subchecks: missing/duplicate/unknown storage, field boundary or Rust mapping check")
    markers = re.findall(r"^STORAGE_ASSERTION (\S+) ([1-9][0-9]*) PASS$", stdout, re.M)
    marker_map: dict[str, list[int]] = {}
    for identifier, count in markers:
        marker_map.setdefault(identifier, []).append(int(count))
    summaries = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", stdout)
    if not summaries or any(int(failed) or int(ignored) for _, failed, ignored in summaries) or not sum(int(passed) for passed, _, _ in summaries):
        errors.append("actual Rust runner must execute nonzero passing tests with zero ignored/failed tests")
    if re.search(r"\bSTORAGE_SKIP\b", stdout):
        errors.append("storage assertions may not be skipped")
    for row in checks:
        if not isinstance(row, dict):
            continue
        count, skip = row.get("assertions"), row.get("skipped")
        if row.get("status") != "passed" or type(count) is not int or count <= 0 or type(skip) is not int or skip != 0:
            errors.append("storage subcheck did not actually pass without skip: " + str(row.get("id")))
        if marker_map.get(row.get("id")) != [count]:
            errors.append("storage subcheck lacks its unique actual runner assertion marker: " + str(row.get("id")))
    if set(marker_map) != expected_checks:
        errors.append("actual assertion log identity differs from required subcheck inventory")
    routes = receipt.get("migration_routes")
    if not isinstance(routes, list):
        routes = []
    route_ids = [row.get("id") for row in routes if isinstance(row, dict)]
    if not _nonempty_strings(route_ids) or len(route_ids) != len(routes) or set(route_ids) != set(expected_routes):
        errors.append("migration route coverage differs from fresh and PRE-14 frozen matrix")
    from storage_schema import compare_schema_file, compare_rejected_relationship_snapshot, sealed_snapshot_errors
    route_paths = set()
    route_inodes = set()
    for route in routes:
        try:
            if not isinstance(route, dict) or route.get("id") not in expected_routes:
                continue
            spec = expected_routes[route["id"]]
            for key in ("database", "source_schema", "target_schema"):
                if route.get(key) != spec[key]:
                    raise ValueError("route database/schema identity differs from frozen matrix")
            for key in ("source_schema", "target_schema"):
                declared = route.get(key + "_sha256")
                expected_hash = _hash(_path(root, spec[key])) if spec[key] else None
                if declared != expected_hash:
                    raise ValueError("route frozen source/target schema hash differs")
            if route.get("outcome") != spec["expected"]:
                raise ValueError("route outcome differs from frozen migration policy")
            if route.get("error_code") != spec.get("expected_error"):
                raise ValueError("route production rejection code differs from frozen policy")
            files = {}
            for position in ("before", "after", "backup"):
                value = route.get(position)
                if position == "before" and spec["kind"] == "fresh":
                    if value is not None:
                        raise ValueError("fresh route cannot reuse a before database")
                    continue
                if position == "backup" and spec["kind"] != "paired_recovery":
                    if value is not None:
                        raise ValueError("backup belongs only to a paired recovery route")
                    continue
                if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
                    raise ValueError("route requires actual unique before/after/backup database snapshots")
                path = _path(output, value["path"])
                if path in route_paths:
                    raise ValueError("different route positions must not reuse a fresh database snapshot")
                route_paths.add(path)
                inode = (path.stat().st_dev, path.stat().st_ino)
                if inode in route_inodes:
                    raise ValueError("different routes must not hard-link the same database snapshot")
                route_inodes.add(inode)
                before_hash = _hash(path)
                if not path.stat().st_size or before_hash != value["sha256"]:
                    raise ValueError("route database hash mismatch or empty snapshot")
                sealed_errors = sealed_snapshot_errors(path)
                if sealed_errors:
                    raise ValueError("route database must be a closed consistent snapshot: " + "; ".join(sealed_errors))
                source_position = position in {"before", "backup"} or spec["expected"] == "rejected"
                contract = spec["source_schema"] if source_position else spec["target_schema"]
                expected_contract_hash = _hash(_path(root, contract)) if contract else None
                if route.get(("source_schema_sha256" if source_position else "target_schema_sha256")) != expected_contract_hash:
                    raise ValueError("route frozen schema contract hash differs")
                if contract:
                    if (source_position and spec["expected"] == "rejected"
                            and spec.get("source_state") == "relationship_corrupt"
                            and spec.get("expected_error") == "database_relationship_corrupt"):
                        errors.extend(compare_rejected_relationship_snapshot(root, path, spec["database"], contract))
                    else:
                        errors.extend(compare_schema_file(root, path, spec["database"], contract))
                if source_position and spec["expected"] == "rejected":
                    errors.extend(_rejected_source_errors(root, path, spec, matrix))
                else:
                    version = spec["from_version"] if source_position else spec["to_version"]
                    errors.extend(_version_errors(path, spec["database"], version, matrix))
                if _hash(path) != before_hash:
                    raise ValueError("read-only comparison changed a route database")
                files[position] = before_hash
            if spec["expected"] == "rejected" and files.get("before") != files.get("after"):
                raise ValueError("rejected migration changed its original database")
            if spec["kind"] == "paired_recovery" and files.get("backup") != files.get("after"):
                raise ValueError("paired recovery does not match its verified backup")
        except (OSError, ValueError, TypeError, KeyError) as exc:
            errors.append("actual migration route: " + str(exc))
    dbs = receipt.get("databases")
    if not isinstance(dbs, list) or len(dbs) != 2 or {row.get("database") for row in dbs if isinstance(row, dict)} != {"profile", "dictionary"}:
        return errors + ["runtime must supply both actual migrated profile and built dictionary databases"]
    from storage_schema import compare_schema
    seen = set()
    for db in dbs:
        try:
            if not isinstance(db, dict) or db.get("open_mode") != "read_only_snapshot" or db.get("origin") != "registered_production_migrations":
                raise ValueError("actual closed migrated database snapshot is required")
            fresh = next((route for route in routes if isinstance(route, dict) and route.get("id") == "fresh_" + str(db.get("database"))), {})
            if fresh.get("after") != {"path": db.get("path"), "sha256": db.get("sha256")}:
                raise ValueError("candidate database must be bound to its actual fresh migration route")
            path = _path(output, db.get("path"))
            if path in seen:
                raise ValueError("profile and dictionary must be distinct actual database files")
            seen.add(path)
            before = _hash(path)
            if not path.stat().st_size or before != db.get("sha256"):
                raise ValueError("actual migrated database is empty or hash differs from receipt")
            sealed_errors = sealed_snapshot_errors(path)
            if sealed_errors:
                raise ValueError("candidate must be checkpointed/closed; live WAL snapshot is not accepted: " + "; ".join(sealed_errors))
            errors.extend(compare_schema_file(root, path, db["database"], adapter.get("schema_contract", SCHEMA_PATH)))
            if _hash(path) != before:
                errors.append("read-only schema comparison changed candidate database")
        except (OSError, ValueError, TypeError, KeyError) as exc:
            errors.append("actual migrated database: " + str(exc))
    if adapter["adapter_version"] in {"rust-storage-complete-v2", "rust-storage-complete-v3"}:
        errors.extend(_validate_v2_delta_receipt(root,output,receipt,matrix,
            schema_three=adapter["adapter_version"] == "rust-storage-complete-v3"))
    if adapter["adapter_version"] == "rust-storage-complete-v3":
        errors.extend(_validate_v3_delta_receipt(root, output, receipt, matrix))
    return errors


def _validate_v2_delta_receipt(root: Path, output: Path, receipt: dict, matrix: dict, *, schema_three=False) -> list[str]:
    """Independent reader verifies every new codec and real pre-upgrade backup."""
    errors = []
    from storage_schema_v2 import DICTIONARY
    from storage_schema import compare_schema_file, sealed_snapshot_errors
    wanted = {"RUST-MAP:" + row["id"]: row for row in _read(_path(root, DICTIONARY))["fields"]}
    actual = {row.get("id"): row for row in receipt.get("checks",[]) if isinstance(row,dict)}
    for identifier, field in wanted.items():
        row = actual.get(identifier,{})
        expected_type = {"INTEGER":"integer","TEXT":"text","REAL":"real"}[field["type"]]
        # Opening operations have no end time; the producer test also observes terminal INTEGER.
        if identifier == "RUST-MAP:profile.capture_operations.ended_at_ms":
            expected_type = "null"
        if not str(row.get("source","")).startswith("storage_session_v2::") or row.get("actual_sqlite_type") != expected_type:
            errors.append("schema2 field lacks exact real producer codec: " + identifier)
        boundary = actual.get(identifier.replace("RUST-MAP:","V2-FIELD:"),{})
        if type(boundary.get("assertions")) is not int or boundary["assertions"] < 3:
            errors.append("schema2 field lacks valid/type/null probes: " + identifier)
    detail = receipt.get("schema_two_migration")
    if not isinstance(detail,dict):
        return errors + ["schema2 actual migration and backup evidence is missing"]
    paths = {}
    target_version, target_contract = (3, V3_SCHEMA_PATH) if schema_three else (2, V2_SCHEMA_PATH)
    for key, version, contract in [("before",1,SCHEMA_PATH),("backup",1,SCHEMA_PATH),("restored",1,SCHEMA_PATH),("after",target_version,target_contract)]:
        try:
            row=detail[key]
            if not isinstance(row,dict) or set(row)!={"path","sha256"}:
                raise ValueError("migration snapshot identity is incomplete")
            path=_path(output,row["path"])
            if _hash(path)!=row["sha256"]:
                raise ValueError("migration snapshot content hash differs")
            paths[key]=path
            errors.extend(sealed_snapshot_errors(path))
            errors.extend(compare_schema_file(root,path,"profile",contract))
            errors.extend(_version_errors(path,"profile",version,matrix))
        except (OSError,ValueError,KeyError,TypeError) as exc:
            errors.append("schema2 migration " + key + ": " + str(exc))
    if len(paths)==4:
        if len({(p.stat().st_dev,p.stat().st_ino) for p in paths.values()}) != 4:
            errors.append("schema2 migration snapshots must be distinct physical files")
        if _hash(paths["backup"]) != _hash(paths["restored"]):
            errors.append("schema2 recovery must preserve the actual standalone v1 backup bytes")
        def logical_snapshot(path):
            connection=sqlite3.connect(path.resolve().as_uri()+"?mode=ro&immutable=1",uri=True)
            try:
                return list(connection.iterdump())
            finally:
                connection.close()
        try:
            if logical_snapshot(paths["before"]) != logical_snapshot(paths["backup"]):
                errors.append("schema2 backup changed an original schema object or data row")
            before = sqlite3.connect(paths["before"].resolve().as_uri()+"?mode=ro&immutable=1",uri=True)
            after = sqlite3.connect(paths["after"].resolve().as_uri()+"?mode=ro&immutable=1",uri=True)
            try:
                tables=[row[0] for row in before.execute("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
                for table in tables:
                    if table in {"schema_migrations","profile_meta","sessions"}:
                        continue # Installed ledger and ordinary runtime lifecycle legitimately advance.
                    quoted='"'+table.replace('"','""')+'"'
                    old=sorted(before.execute("SELECT * FROM "+quoted).fetchall(),key=repr)
                    if schema_three:
                        # The independently reviewed delta adds columns, but
                        # every original column still has the same obligation.
                        columns = [row[1] for row in before.execute("PRAGMA table_info(" + quoted + ")")]
                        projection = ",".join('"' + column.replace('"', '""') + '"' for column in columns)
                        new = sorted(after.execute("SELECT " + projection + " FROM " + quoted).fetchall(), key=repr)
                    else:
                        new=sorted(after.execute("SELECT * FROM "+quoted).fetchall(),key=repr)
                    if old!=new:
                        errors.append("schema2 migration changed an original immutable table row: "+table)
                expected=before.execute("SELECT r.session_id,r.unit_id,r.text_revision FROM reading_unit_revisions r JOIN reading_units u USING(unit_id) ORDER BY r.session_id,u.unit_ordinal,r.text_revision").fetchall()
                actual=after.execute("SELECT session_id,unit_id,text_revision FROM body_record_positions ORDER BY record_sequence").fetchall()
                if actual!=expected:
                    errors.append("schema2 migration backfill order or complete original revision set differs")
                legacy=before.execute("SELECT session_id,started_at_ms,ended_at_ms FROM sessions WHERE source_kind='hook' ORDER BY session_id").fetchall()
                actual=after.execute("SELECT session_id,started_at_ms,ended_at_ms FROM capture_operations WHERE legacy_unknown_start=1 ORDER BY session_id").fetchall()
                if actual!=legacy or after.execute("SELECT count(*) FROM hook_events").fetchone()!=(0,):
                    errors.append("schema2 migration lost legacy identities/times or invented historical events")
                preferences=before.execute("SELECT preference_revision_id,CASE WHEN json_type(overrides_json,'$.\"reading.segmentationEnabled\"') IN ('true','false') THEN json_extract(overrides_json,'$.\"reading.segmentationEnabled\"') ELSE 1 END FROM preference_revisions ORDER BY preference_revision_id").fetchall()
                actual=after.execute("SELECT preference_revision_id,segmentation_enabled FROM reading_preference_settings ORDER BY preference_revision_id").fetchall()
                if actual!=preferences:
                    errors.append("schema2 migration changed legacy explicit segmentation values")
            finally:
                before.close();after.close()
        except sqlite3.Error as exc:
            errors.append("schema2 independent data preservation inspection failed: "+str(exc))
    if schema_three:
        # The current host upgrades 1->2->3. In addition, retain an actual run
        # of the frozen original host's 1->2 transition and standalone backup;
        # observing only the final schema3 cannot replace that old obligation.
        try:
            intermediate = _v3_snapshot(root, output, detail["schema_two_intermediate"], 2, matrix)
            original_backup = _v3_snapshot(root, output, detail["schema_two_original_backup"], 1, matrix)
            old_case = _read(_path(output, "components/v3-delta/old-client-rejects-three.json"))
            if detail.get("old_reader_binary_sha256") != old_case["detail"].get("binary_sha256"):
                errors.append("schema2 migration did not use the same frozen original reader prerequisite")
            if original_backup in paths.values() or intermediate in paths.values() or original_backup == intermediate:
                errors.append("original schema2 host evidence must use separate fresh snapshots")
            before = sqlite3.connect(paths["before"].resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
            backup = sqlite3.connect(original_backup.resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
            legacy_two = sqlite3.connect(intermediate.resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
            try:
                if list(before.iterdump()) != list(backup.iterdump()):
                    errors.append("original schema2 host backup changed source-v1 rows or schema")
                expected = before.execute("SELECT r.session_id,r.unit_id,r.text_revision FROM reading_unit_revisions r JOIN reading_units u USING(unit_id) ORDER BY r.session_id,u.unit_ordinal,r.text_revision").fetchall()
                actual = legacy_two.execute("SELECT session_id,unit_id,text_revision FROM body_record_positions ORDER BY record_sequence").fetchall()
                if expected != actual:
                    errors.append("original schema2 host backfill lost/reordered legacy revisions")
            finally:
                before.close(); backup.close(); legacy_two.close()
        except (OSError, ValueError, KeyError, TypeError, sqlite3.Error) as exc:
            errors.append("schema2 original-host transition proof: " + str(exc))
    return errors


V3_COMPONENT_KINDS = {
    **{name: "http_storage_v3_delta_receipt" for name in
       ("migration", "codec", "credentials", "fields", "gateway-authority")},
    **{name: "http_storage_v3_supporting_case" for name in
       ("crash-mid-sql", "crash-after-host", "backup-collision", "typed-envelope-sqlite",
        "typed-activation-compensation", "typed-vault-failure", "legacy-raw-vault",
        "typed-real-system-keychain", "old-client-rejects-three", "profile-mismatch-reject",
        "host-rollback-failure", "single-file-backup")},
}


def _v3_snapshot(root: Path, output: Path, value: Any, version: int, matrix: dict) -> Path:
    from storage_schema import compare_schema_file, sealed_snapshot_errors
    if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
        raise ValueError("schema3 snapshot requires an exact path/hash object")
    relative = value["path"]
    if isinstance(relative, str) and Path(relative).is_absolute():
        relative = Path(relative).relative_to(output.resolve()).as_posix()
    path = _path(output, relative)
    if not path.stat().st_size or _hash(path) != value["sha256"]:
        raise ValueError("schema3 snapshot is empty or differs from its actual producer hash")
    contract = {1: SCHEMA_PATH, 2: V2_SCHEMA_PATH, 3: V3_SCHEMA_PATH}[version]
    errors = sealed_snapshot_errors(path) + compare_schema_file(root, path, "profile", contract)
    errors += _version_errors(path, "profile", version, matrix)
    if errors:
        raise ValueError("; ".join(errors))
    if _hash(path) != value["sha256"]:
        raise ValueError("read-only schema3 inspection changed a snapshot")
    return path


def _validate_v3_delta_receipt(root: Path, output: Path, receipt: dict, matrix: dict) -> list[str]:
    """Inspect fresh real component evidence, never turn the design into PASS."""
    from storage_schema_v3 import validate_delta_receipt, read_json, BASE, v3_check_ids
    errors = validate_delta_receipt(root, receipt)
    from storage_product_adapter import v3_source_map
    errors.extend(v3_source_map(root)["errors"])
    listed = receipt.get("schema_three_components")
    if not isinstance(listed, list):
        return errors + ["schema3 same-run component inventory is missing"]
    components, raw_checks = {}, {}
    for row in listed:
        try:
            if not isinstance(row, dict) or set(row) != {"case", "path", "sha256"}:
                raise ValueError("schema3 component requires exact case/path/hash")
            case = row["case"]
            if case not in V3_COMPONENT_KINDS or case in components:
                raise ValueError("schema3 component case is duplicate or unregistered")
            if row["path"] != f"components/v3-delta/{case}.json":
                raise ValueError("schema3 component does not use its registered output path")
            path = _path(output, row["path"])
            if _hash(path) != row["sha256"]:
                raise ValueError("schema3 component content differs from the assembled identity")
            proof = _read(path)
            for key in ("run_id", "execution_nonce", "source_tree_sha256"):
                if not isinstance(receipt.get(key), str) or not receipt[key] or proof.get(key) != receipt[key]:
                    raise ValueError("schema3 component is not from this exact execution: " + key)
            if proof.get("kind") != V3_COMPONENT_KINDS[case]:
                raise ValueError("schema3 component has the wrong producer kind")
            completed = proof.get("completed_at_ms")
            if (type(completed) is not int or not receipt["started_at_ms"] <= completed <= receipt["finished_at_ms"]
                    or not isinstance(proof.get("detail"), dict)):
                raise ValueError("schema3 component lacks this execution's interval or typed detail")
            if proof["kind"] == "http_storage_v3_delta_receipt":
                checks = proof.get("checks")
                if not isinstance(checks, list) or not checks:
                    raise ValueError("schema3 component has no actual assertions")
                for check in checks:
                    if not isinstance(check, dict) or check.get("id") in raw_checks:
                        raise ValueError("schema3 producer check is malformed or duplicated")
                    if (check.get("status") != "passed" or type(check.get("assertions")) is not int
                            or check["assertions"] <= 0 or type(check.get("skipped")) is not int or check["skipped"] != 0):
                        raise ValueError("schema3 producer assertion did not actually pass")
                    raw_checks[check["id"]] = check
            elif (proof.get("case") != case or type(proof.get("assertions")) is not int or proof["assertions"] <= 0):
                raise ValueError("schema3 supporting case has no actual positive assertions")
            components[case] = proof
        except (OSError, ValueError, KeyError, TypeError) as exc:
            errors.append("schema3 component: " + str(exc))
    missing = set(V3_COMPONENT_KINDS) - set(components)
    if missing:
        errors.append("schema3 actual component scenarios missing: " + ",".join(sorted(missing)))
    directory = output / "components/v3-delta"
    actual_files = {path.name for path in directory.iterdir()} if directory.is_dir() else set()
    if actual_files != {case + ".json" for case in V3_COMPONENT_KINDS}:
        errors.append("schema3 component directory contains missing or unregistered evidence files")
    expected = v3_check_ids(root)
    if set(raw_checks) != expected:
        errors.append("schema3 real component assertion set differs from the independent delta contract")
    assembled = {row.get("id"): row for row in receipt.get("checks", []) if isinstance(row, dict)}
    for identifier, row in raw_checks.items():
        for key in ("id", "status", "assertions", "skipped", "source", "field_assertions"):
            if key in row and assembled.get(identifier, {}).get(key) != row[key]:
                errors.append("schema3 assembled row differs from its actual component: " + identifier)
    # These routes are not inferred from a green V3-MIGRATION label. Each has
    # its own mandatory real scenario and immutable snapshot inspection below;
    # the retained v1/v2 cases also execute in this same closed Cargo target.
    covered_routes = {
        "fresh_0_to_3", "upgrade_1_to_3_with_backup", "upgrade_2_to_3_with_backup",
        "reopen_3", "old_client_rejects_3", "rollback_failed_3", "crash_atomic_2_or_3",
        "backup_restore", "reject_unknown_version", "reject_corrupt_hash_schema_fk_profile",
        "readonly_busy_disk_full",
    }
    registry = read_json(root, BASE + "storage-acceptance.json")
    if set(registry["required_routes"]) != covered_routes:
        errors.append("schema3 required route set has no exact registered runtime inspection")
    ordinary_cases = set(receipt.get("asserted_case_files", []))
    for case in ("readonly-retry", "cross-process-busy", "actual-limited-volume-enospc"):
        if case not in ordinary_cases:
            errors.append("schema3 actual fault route did not execute: " + case)
    try:
        fields = components["fields"]["detail"]
        boundary = read_json(root, BASE + "field-boundaries.json")
        expected_probes = {row["id"] for row in boundary["probes"]}
        actual_probes = fields.get("probe_ids")
        if not isinstance(actual_probes, list) or len(actual_probes) != len(set(actual_probes)) or set(actual_probes) != expected_probes:
            errors.append("schema3 production boundary probes differ from the full independent inventory")
        wanted_relations = set(boundary["relationship_ids"])
        actual_relations = fields.get("relationship_ids")
        if not isinstance(actual_relations, list) or len(actual_relations) != len(set(actual_relations)) or set(actual_relations) != wanted_relations:
            errors.append("schema3 actual FK/trigger relationships differ from the independent inventory")
        _v3_snapshot(root, output, fields["producer_database"], 3, matrix)
        for case in ("codec", "credentials", "crash-after-host", "typed-envelope-sqlite",
                     "typed-activation-compensation", "typed-vault-failure", "legacy-raw-vault", "typed-real-system-keychain",
                     "old-client-rejects-three", "profile-mismatch-reject", "host-rollback-failure", "single-file-backup"):
            _v3_snapshot(root, output, components[case]["detail"]["database"], 3, matrix)
        for case in ("crash-mid-sql", "backup-collision"):
            _v3_snapshot(root, output, components[case]["detail"]["database"], 2, matrix)
        _v3_snapshot(root, output, components["host-rollback-failure"]["detail"]["backup"], 2, matrix)
        if components["profile-mismatch-reject"]["detail"].get("actual_error") != "profile_mismatch":
            errors.append("schema3 wrong-profile startup was not actually rejected")
        if components["host-rollback-failure"]["detail"].get("actual_error") != "storage_failure":
            errors.append("schema3 real migration host rollback did not report the expected SQLite failure")
        old_reader = components["old-client-rejects-three"]["detail"]
        if old_reader.get("actual_error", {}).get("error_code") != "unsupported_schema":
            errors.append("actual old schema2 reader did not reject schema3")
        old_snapshot = _v3_snapshot(root, output, old_reader["database"], 3, matrix)
        reader_path = old_reader["old_reader_manifest"]["path"]
        if Path(reader_path).is_absolute():
            reader_path = Path(reader_path).relative_to(root.resolve()).as_posix()
        manifest_path = _path(root, reader_path)
        if _hash(manifest_path) != old_reader["old_reader_manifest"]["sha256"]:
            errors.append("old-reader prerequisite manifest changed after real execution")
        manifest = _validate_old_reader_manifest(root, manifest_path)
        replay = subprocess.run([manifest["binary_path"], str(old_snapshot), "current-three"],
                                capture_output=True, text=True, timeout=10)
        if replay.returncode != 0 or json.loads(replay.stdout).get("error_code") != "unsupported_schema":
            errors.append("independent actual old-reader replay did not reject schema3")
        if _hash(old_snapshot) != old_reader["database"]["sha256"]:
            errors.append("old-reader replay modified the rejected schema3 snapshot")
        migration = components["migration"]["detail"]
        paths = {key: _v3_snapshot(root, output, migration[key], version, matrix)
                 for key, version in (("before", 2), ("backup", 2), ("restored", 2), ("after", 3))}
        if len({(path.stat().st_dev, path.stat().st_ino) for path in paths.values()}) != 4:
            errors.append("schema3 original/backup/restored/after snapshots must be separate physical files")
        if _hash(paths["backup"]) != _hash(paths["restored"]):
            errors.append("schema3 actual backup restoration changed bytes")
        before = sqlite3.connect(paths["before"].resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
        backup = sqlite3.connect(paths["backup"].resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
        after = sqlite3.connect(paths["after"].resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
        try:
            if list(before.iterdump()) != list(backup.iterdump()):
                errors.append("schema3 pre-upgrade backup differs from the original logical database")
            for (table,) in before.execute("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"):
                if table in {"schema_migrations", "profile_meta"}:
                    continue
                quoted = '"' + table.replace('"', '""') + '"'
                columns = [row[1] for row in before.execute("PRAGMA table_info(" + quoted + ")")]
                projection = ",".join('"' + column.replace('"', '""') + '"' for column in columns)
                old = sorted(before.execute("SELECT " + projection + " FROM " + quoted).fetchall(), key=repr)
                new = sorted(after.execute("SELECT " + projection + " FROM " + quoted).fetchall(), key=repr)
                if old != new:
                    errors.append("schema3 migration changed original columns/rows: " + table)
        finally:
            before.close(); backup.close(); after.close()
    except (OSError, ValueError, KeyError, TypeError, sqlite3.Error, subprocess.SubprocessError) as exc:
        errors.append("schema3 actual database evidence: " + str(exc))
    return errors


def _validate_old_reader_manifest(root: Path, path: Path) -> dict:
    """Explicit old-binary prerequisite; no substitute built from current Store."""
    manifest = _read(path)
    if manifest.get("kind") != "actual_old_sqlite_store_reader_v2" or type(manifest.get("schema_version")) is not int or manifest["schema_version"] != 2:
        raise ValueError("schema3 requires an actual frozen original schema2 reader")
    old_matrix = _read(_path(root, V2_REGISTRY_PATH))["supported_upgrade_matrix"]
    old_source = old_matrix["migration_chain"][0]["source_sha256"]
    if manifest.get("original_database_source_sha256") != old_source:
        raise ValueError("old reader does not bind the original reviewed schema2 migration host")
    def owned_absolute(value):
        relative = Path(value).relative_to(root.resolve()).as_posix() if Path(value).is_absolute() else value
        return _path(root, relative)
    binary = owned_absolute(manifest["binary_path"])
    if (manifest.get("binary_sha256") != ORIGINAL_V2_READER_SHA256
            or _hash(binary) != ORIGINAL_V2_READER_SHA256 or not binary.stat().st_size):
        raise ValueError("actual frozen old reader binary is empty or changed")
    source = Path(manifest["source_root"])
    if not source.is_absolute() or not source.resolve().is_relative_to(root.resolve()) or source.is_symlink():
        raise ValueError("old reader source must be an isolated owned prerequisite")
    for candidate in (source, *source.parents):
        if candidate == root.resolve():
            break
        if candidate.is_symlink():
            raise ValueError("old reader source cannot contain symbolic-link ancestors")
    rows = manifest.get("source_files")
    if not isinstance(rows, list) or not rows:
        raise ValueError("old reader source closure is missing")
    identities = {}
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256"} or row["path"] in identities:
            raise ValueError("old reader source closure is malformed or duplicated")
        candidate = _path(source.resolve(), row["path"])
        if _hash(candidate) != row["sha256"]:
            raise ValueError("actual old reader source changed: " + row["path"])
        identities[row["path"]] = row["sha256"]
    if identities.get("crates/app-infra/src/storage/database.rs") != old_source:
        raise ValueError("old reader uses the wrong SqliteStore implementation")
    for entry in old_matrix["migration_chain"]:
        for sql in entry["sql_sources"]:
            if identities.get(sql["path"]) != sql["sha256"]:
                raise ValueError("old reader changed the immutable schema1/2 migrations")
    return {**manifest, "binary_path": str(binary)}


def run_product_storage(root: Path, adapter_ids: list[str], output_dir: Path | str,
                        artifact: Path | str | None = None,
                        old_reader_manifest: Path | str | None = None) -> dict[str, Any]:
    """Compile and execute registered Rust producers, then independently inspect DBs."""
    input_root = Path(root).absolute()
    root = input_root.resolve()
    result: dict[str, Any] = {"schema_version": 1, "kind": "storage_acceptance_execution", "status": "blocked",
                              "product_storage_tests_executed": False, "product_release_claim": "not_issued", "errors": [], "adapters": []}
    metadata = {"kind": "product", "product_storage_checks": adapter_ids}
    errors = _design_checks(root)
    errors.extend(storage_requirement_gaps(root, metadata, [], "release"))
    if errors:
        result["errors"] = errors
        return result
    reader_path = None
    if "rust-storage-complete-v3" in adapter_ids:
        try:
            supplied = old_reader_manifest or __import__("os").environ.get("LUNA_HTTP_OLD_READER_MANIFEST")
            if not supplied:
                raise ValueError("schema3 runtime requires --old-reader-manifest or LUNA_HTTP_OLD_READER_MANIFEST for the frozen original schema2 binary; missing prerequisite is blocked")
            relative = Path(supplied)
            if relative.is_absolute():
                relative = relative.relative_to(input_root) if relative.is_relative_to(input_root) else relative.relative_to(root)
            reader_path = _path(root, relative.as_posix())
            _validate_old_reader_manifest(root, reader_path)
            result["old_reader_prerequisite"] = {"path": str(reader_path.relative_to(root)), "sha256": _hash(reader_path)}
        except (OSError, ValueError, KeyError, TypeError) as exc:
            result["errors"].append("schema3 runtime prerequisite: " + str(exc))
            return result
    run_root = None
    try:
        directory = Path(output_dir)
        if directory.is_absolute():
            directory = directory.relative_to(input_root) if directory.is_relative_to(input_root) else directory.relative_to(root)
        if not directory.parts or directory.parts[0] != ".build":
            raise ValueError("storage output must be in an isolated repository .build directory")
        output_base = _path(root, directory.as_posix(), exists=False)
        output_base.mkdir(parents=True, exist_ok=True)
        run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ") + "-" + uuid.uuid4().hex[:8]
        run_root = _path(root, (directory / run_id).as_posix(), exists=False)
        run_root.mkdir()
        result.update(run_id=run_id, output=str(run_root.relative_to(root)))
        registry_path = _registry_path(root)
        schema_path = {V3_REGISTRY_PATH: V3_SCHEMA_PATH, V2_REGISTRY_PATH: V2_SCHEMA_PATH}.get(registry_path, SCHEMA_PATH)
        registry = _read(_path(root, registry_path))
        for identifier in adapter_ids:
            adapter_schema = STORAGE_ADAPTERS[identifier].get("schema_contract", SCHEMA_PATH)
            if adapter_schema != schema_path:
                raise ValueError("adapter schema differs from current reviewed profile; historical v1 cannot validate schema2")
        expected_routes = {row["id"]: row for row in registry["supported_upgrade_matrix"]["routes"]}
        for database in ("profile", "dictionary"):
            expected_routes["fresh_" + database] = {"id": "fresh_" + database, "kind": "fresh", "database": database,
                                                    "source_schema": None, "target_schema": schema_path, "expected": "accepted",
                                                    "source_state": None, "expected_error": None,
                                                    "from_version": None, "to_version": registry["supported_upgrade_matrix"]["fresh_versions"][database]}
        expected_checks = CHECK_IDS | boundary_check_ids(root) | {"MIGRATION-ROUTE:" + identifier for identifier in expected_routes}
        if schema_path in {V2_SCHEMA_PATH, V3_SCHEMA_PATH}:
            expected_checks |= v2_check_ids(root)
        if schema_path == V3_SCHEMA_PATH:
            from storage_schema_v3 import v3_check_ids
            expected_checks |= v3_check_ids(root)
        source_hash = _source_hash(root)
        artifact_path = None
        if artifact is not None:
            relative = Path(artifact)
            if relative.is_absolute():
                relative = relative.relative_to(input_root) if relative.is_relative_to(input_root) else relative.relative_to(root)
            artifact_path = _path(root, relative.as_posix())
            if not artifact_path.stat().st_size:
                raise ValueError("final package artifact is empty")
            result["package_identity"] = inspect_package(root, artifact_path, source_hash, run_root)
        artifact_hash = _hash(artifact_path) if artifact_path else None
        for name in adapter_ids:
            adapter = STORAGE_ADAPTERS[name]
            frozen_sources = {entry["source_path"] for entry in registry["supported_upgrade_matrix"]["migration_chain"]}
            if not frozen_sources <= set(adapter["migration_sources"]):
                raise ValueError("registered adapter omits frozen production migration sources")
            adapter_output = run_root / name
            adapter_output.mkdir()
            receipt_path = adapter_output / "receipt.json"
            inventory_path = adapter_output / "required-checks.json"
            expected_inventory = {"checks": sorted(expected_checks), "migration_routes": [expected_routes[key] for key in sorted(expected_routes)]}
            inventory_path.write_text(json.dumps(expected_inventory, ensure_ascii=False) + "\n")
            started = time.time_ns() // 1_000_000
            execution_nonce = uuid.uuid4().hex
            env = dict(__import__("os").environ,
                       FP_STORAGE_RUN_ID=run_id, FP_STORAGE_OUTPUT=str(adapter_output),
                       FP_STORAGE_RECEIPT=str(receipt_path), FP_STORAGE_SOURCE_SHA256=source_hash,
                       FP_STORAGE_ARTIFACT_SHA256=artifact_hash or "", FP_STORAGE_STARTED_AT_MS=str(started),
                       FP_STORAGE_ADAPTER_VERSION=adapter["adapter_version"],
                       FP_STORAGE_EXECUTION_NONCE=execution_nonce,
                       FP_STORAGE_INVENTORY=str(inventory_path), FP_STORAGE_INVENTORY_SHA256=_hash(inventory_path))
            if reader_path is not None:
                env["LUNA_HTTP_OLD_READER_MANIFEST"] = str(reader_path)
            record: dict[str, Any] = {"id": name, "status": "failed", "commands": [], "errors": []}
            if adapter["adapter_version"] == "rust-storage-complete-v3":
                record["execution_nonce"] = execution_nonce
            result["adapters"].append(record)
            stdout = ""
            for stage in ("compile", "run"):
                command = adapter[stage + "_command"]
                before_receipt = receipt_path.exists()
                if stage == "run" and before_receipt:
                    record["errors"].append("compile stage must not fabricate a runtime receipt")
                    break
                stage_record, captured = execute_storage_stage(command, root=root, output=adapter_output,
                                                               stage=stage, env=env,
                                                               timeout=adapter.get(stage + "_timeout_seconds", 600))
                if stage == "run" and stage_record.get("pid") is not None:
                    result["product_storage_tests_executed"] = True
                log = adapter_output / (stage + ".log")
                log.write_text(captured)
                stage_record.update(log=str(log.relative_to(root)), sha256=_hash(log))
                record["commands"].append(stage_record)
                if stage_record["status"] != "passed":
                    record["errors"].append(stage + (" command timed out; partial output and owned-tree cleanup recorded" if stage_record["timed_out"] else " command failed"))
                    break
                if stage == "run":
                    stdout = captured
            finished = time.time_ns() // 1_000_000
            if not record["errors"]:
                if adapter["adapter_version"] == "rust-storage-complete-v3":
                    for stage_record in record["commands"]:
                        for key, expected in (("run_id", run_id), ("execution_nonce", execution_nonce), ("source_tree_sha256", source_hash)):
                            if stage_record.get(key) != expected:
                                record["errors"].append("schema3 actual process identity differs: " + key)
                receipt = _read(_path(adapter_output, "receipt.json"))
                record["errors"].extend(_validate_receipt(root, adapter_output, receipt, adapter, run_id,
                                                        started, finished, source_hash, artifact_hash, stdout,
                                                        expected_checks, expected_routes, registry["supported_upgrade_matrix"], execution_nonce))
                record["receipt"] = {"path": str(receipt_path.relative_to(root)), "sha256": _hash(receipt_path)}
                if _read(_path(adapter_output, "required-checks.json")) != expected_inventory:
                    record["errors"].append("runner changed its required assertion/route inventory")
            record["status"] = "failed" if record["errors"] else "passed"
            result["errors"].extend(f"{name}: {error}" for error in record["errors"])
        if _source_hash(root) != source_hash:
            result["errors"].append("production sources changed while storage commands were running")
        if artifact_path and _hash(artifact_path) != artifact_hash:
            result["errors"].append("final package changed during storage acceptance")
        if reader_path is not None:
            if _hash(reader_path) != result["old_reader_prerequisite"]["sha256"]:
                result["errors"].append("original schema2 reader prerequisite changed during acceptance")
            _validate_old_reader_manifest(root, reader_path)
        result.update(status="failed" if result["errors"] else "passed", source_tree_sha256=source_hash,
                      artifact_sha256=artifact_hash)
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as exc:
        result["status"] = "failed"
        result["errors"].append("storage execution input/runtime: " + str(exc))
    finally:
        if run_root is not None:
            (run_root / "execution.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--runtime", action="store_true")
    parser.add_argument("--adapter", action="append", default=[])
    parser.add_argument("--artifact", type=Path)
    parser.add_argument("--old-reader-manifest", type=Path,
                        help="Explicit frozen original schema2 reader prerequisite for schema3 acceptance")
    parser.add_argument("--output", type=Path, default=Path(".build/storage-acceptance"))
    args = parser.parse_args()
    if args.check:
        errors = validate_registry(args.root)
        result = {"kind": "storage_acceptance_registry_check", "status": "failed" if errors else "passed",
                  "product_storage_tests_executed": False, "errors": errors}
    else:
        result = run_product_storage(args.root, args.adapter, args.output, args.artifact, args.old_reader_manifest)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
