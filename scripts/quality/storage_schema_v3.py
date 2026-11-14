#!/usr/bin/env python3
"""Independent HTTP schema-three oracle; design checks never grant runtime approval."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import copy
import csv
import hashlib
import io
import json
from pathlib import Path
import sqlite3

import storage_schema as v1
import storage_schema_v2 as v2
from database_design import ROOT, SOURCES, _authorizer

BASE = "docs/architecture/database/v3/"
DESIGN = BASE + "003_http_service_identity.design.sql"
CONTRACT = BASE + "schema-contract.json"
DICTIONARY = BASE + "field-dictionary.json"
CSV = BASE + "field-dictionary.csv"
REGISTRY = BASE + "storage-acceptance.json"
DESIGN_SHA256 = "813145f4923f28860ad2035abf5df8f5b9dbf68753b45510a2b794f80d3c43c2"
PRIOR_FILES = {
    v2.CONTRACT: "eb3b0e78869119f98eac6d08954f5b91f1131a3edf2af85fcd16af85d6154713",
    v2.DICTIONARY: "fae0561b8cd9113dfeaf9b34a4d9557ed523737b1217d9998a8ce68c5a143f20",
    "docs/architecture/database/v2/storage-acceptance.json": "1ca95fec54ecb1af2f4f2a0791010605604bc5662a354e9ad1f2036bacc7de70",
    "docs/architecture/database/field-boundaries.json": "8e646019b98b4ca3af2ba805328c404c9a00795b12a5d0e31d4487ccf4c41504",
}
NEW_COLUMNS = {
    "config_revisions": ["codec_version"],
    "connections": ["adapter_binding_schema", "adapter_contract_revision", "compatibility_profile_id", "compatibility_profile_revision", "provider_preset_id", "provider_preset_revision"],
    "credential_refs": ["envelope_schema"],
    "provider_preset_versions": ["preset_id", "preset_revision", "namespace", "provider_id", "provider_label", "display_name", "protocol_id", "adapter_contract_revision", "compatibility_profile_id", "compatibility_profile_revision", "endpoint", "auth_kind", "options_json", "content_hash", "source_json", "created_at_ms"],
}
EXPANDED_ENUMS = ["profile.connections.protocol", "profile.connections.auth_mode", "profile.credential_refs.auth_kind"]
SEMANTIC_IDS = {"V3-" + key for key in ["PRODUCERS", "CODEC", "MIGRATION", "RELATIONSHIPS", "AUTHORITY", "CREDENTIALS", "PRESETS", "LEGACY-PRESERVATION"]}
PRODUCERS = {
    "config_revisions": "crates/app-infra/src/storage/queries/configuration.rs::save",
    "connections": "crates/app-infra/src/storage/queries/configuration.rs::save",
    "credential_refs": "crates/app-infra/src/storage/queries/credentials.rs::begin_credential_material",
    "provider_preset_versions": "crates/app-infra/src/storage/queries/provider_presets.rs::store_provider_preset",
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_bytes(root: Path, relative: str) -> bytes:
    root = Path(root).resolve()
    path = Path(relative)
    if path.is_absolute() or any(p in {".", ".."} for p in path.parts):
        raise ValueError("relative repository path required")
    path = root / path
    if not path.resolve().is_relative_to(root) or any(p.is_symlink() for p in [path, *path.parents]):
        raise ValueError("contract/source symlink or escape forbidden")
    return path.read_bytes()


def read_json(root: Path, relative: str):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON key: " + key)
            result[key] = value
        return result
    return json.loads(safe_bytes(root, relative), object_pairs_hook=pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError("non-JSON number: " + value)))


def validate_prior(root: Path) -> list[str]:
    errors = v2.validate_schema_contract(root)
    for path, expected in PRIOR_FILES.items():
        if digest(safe_bytes(root, path)) != expected:
            errors.append("original v1/v2 obligation bytes changed: " + path)
    return errors


@contextmanager
def design_database(root: Path):
    """Execute only independently reviewed design sources, only in memory."""
    source = safe_bytes(root, DESIGN)
    if digest(source) != DESIGN_SHA256:
        raise ValueError("schema-three design hash changed; independent review required")
    db = sqlite3.connect(":memory:")
    db.set_authorizer(_authorizer)
    try:
        db.execute("PRAGMA foreign_keys=OFF")
        for path in SOURCES["profile"]:
            db.executescript(safe_bytes(root, path).decode())
        db.executescript(safe_bytes(root, v2.DESIGN).decode())
        db.executescript(source.decode())
        db.execute("PRAGMA foreign_keys=ON")
        if db.execute("PRAGMA foreign_keys").fetchone()[0] != 1 or list(db.execute("PRAGMA foreign_key_check")):
            raise ValueError("independent design has invalid foreign-key relationships")
        yield db
    finally:
        db.close()


def _preservation(before: dict, after: dict) -> None:
    if set(after["tables"]) - set(before["tables"]) != {"provider_preset_versions"}:
        raise ValueError("schema-three adds exactly one reviewed provenance table")
    if after["views"] != before["views"]:
        raise ValueError("schema-three changed original views")
    for name, old in before["tables"].items():
        new = after["tables"][name]
        if new["columns"][:len(old["columns"])] != old["columns"]:
            raise ValueError("original field physical contract changed: " + name)
        if [c["name"] for c in new["columns"][len(old["columns"]):]] != NEW_COLUMNS.get(name, []):
            raise ValueError("unexpected added field: " + name)
        if not set(old["triggers"]) <= set(new["triggers"]):
            raise ValueError("old trigger removed: " + name)
        if name not in {"config_revisions", "connections", "credential_refs", "translations", "study_results"}:
            errors = v1._diff(old, new, "original_table." + name)
            if errors:
                raise ValueError("; ".join(errors))


def design_contract(root: Path) -> dict:
    original = v2.design_contract(root)
    with design_database(root) as db:
        profile = v1.inspect_connection(db)
    _preservation(original["databases"]["profile"], profile)
    result = copy.deepcopy(original)
    result["databases"]["profile"] = profile
    result["ddl_source_sha256"][DESIGN] = DESIGN_SHA256
    result["prior_contract_sha256"] = PRIOR_FILES[v2.CONTRACT]
    result["phase"] = "independent_schema_three_http_contract_pending_final_review"
    result["migration_version"] = 3
    return result


def field_rows(contract: dict) -> dict:
    tables = contract["databases"]["profile"]["tables"]
    return {f"profile.{table}.{name}": (table, next(c for c in tables[table]["columns"] if c["name"] == name))
            for table, names in NEW_COLUMNS.items() for name in names}


def validate_field_dictionary(root: Path, contract: dict | None = None) -> list[str]:
    data = read_json(root, DICTIONARY)
    wanted = field_rows(design_contract(root) if contract is None else contract)
    errors = []
    expected_header = {"schema_version": 1, "kind": "schema_three_http_field_delta", "previous_fields_retained": 460,
                       "new_fields": 24, "total_fields": 484, "schema_contract": CONTRACT,
                       "expanded_old_enums": EXPANDED_ENUMS, "product_tests_executed": False}
    if not isinstance(data, dict):
        return ["schema-three dictionary must be an object"]
    errors += v1._diff(expected_header, {k: data.get(k) for k in expected_header}, "field_dictionary_header")
    rows = data.get("fields")
    if not isinstance(rows, list) or len(rows) != 24 or any(not isinstance(r, dict) or not isinstance(r.get("id"), str) for r in rows):
        return errors + ["exactly 24 typed field rows required"]
    if len({r["id"] for r in rows}) != 24 or {r["id"] for r in rows} != set(wanted):
        return errors + ["field identities differ from the exact reviewed delta"]
    if set(data) != {*expected_header, "fields", "relationships", "runtime_field_proofs"}:
        errors.append("unknown/missing schema-three dictionary root properties")
    profile = (design_contract(root) if contract is None else contract)["databases"]["profile"]["tables"]
    relationships = {table: profile[table]["foreign_keys"] for table in NEW_COLUMNS}
    errors += v1._diff(relationships, data.get("relationships"), "field_dictionary_relationships")
    proofs = {"status": "not_run", "required_field_ids": ["RUST-MAP:" + row["id"] for row in rows],
              "required_SQL_subassertions": ["valid", "type", "null"], "counts_are_design_inventory_not_execution": True}
    errors += v1._diff(proofs, data.get("runtime_field_proofs"), "runtime_field_proofs")
    keys = {"id", "table", "field", "type", "nullable", "primary_key_position", "default_tokens", "description", "source", "producer", "producer_status", "runtime_proof_id"}
    for row in rows:
        table, column = wanted[row["id"]]
        expected = {"table": table, "field": column["name"], "type": column["type"], "nullable": not bool(column["notnull"] or column["pk"]),
                    "primary_key_position": column["pk"], "default_tokens": column["dflt_value"], "source": DESIGN,
                    "producer": PRODUCERS[table], "producer_status": "pending_runtime_verification", "runtime_proof_id": "RUST-MAP:" + row["id"]}
        if set(row) != keys:
            errors.append("unknown/missing field properties: " + row["id"])
        errors += v1._diff(expected, {k: row.get(k) for k in expected}, row["id"])
        if not isinstance(row.get("description"), str) or not row["description"].strip():
            errors.append("missing field description: " + row["id"])
    csv_rows = list(csv.DictReader(io.StringIO(safe_bytes(root, CSV).decode())))
    expected_csv = [{"id": r["id"], "table": r["table"], "field": r["field"], "type": r["type"], "nullable": str(r["nullable"]).lower(),
                     "primary_key_position": str(r["primary_key_position"]), "default_tokens": json.dumps(r["default_tokens"], separators=(",", ":")),
                     "producer": r["producer"], "producer_status": r["producer_status"], "description": r["description"]} for r in rows]
    errors += v1._diff(expected_csv, csv_rows, "field_dictionary_csv")
    return errors


def v3_check_ids(root: Path) -> set[str]:
    fields = set(field_rows(design_contract(root)))
    return SEMANTIC_IDS | {"RUST-MAP:" + f for f in fields} | {"V3-FIELD:" + f for f in fields} | {"V3-LEGACY-ENUM:" + f for f in EXPANDED_ENUMS}


def _v3_upgrade_matrix(prior: dict) -> dict:
    matrix = copy.deepcopy(prior["supported_upgrade_matrix"])
    matrix["status"] = "design_frozen_runtime_not_run"
    matrix["approval_ref"] = BASE + "storage-review.json"
    matrix["fresh_versions"] = {"profile": 3, "dictionary": 1}
    # Implementation bytes are bound by the outer adapter's current source
    # closure, not by reusing schema-two's historical database.rs digest.
    for entry in matrix["migration_chain"]:
        entry.pop("source_sha256", None)
    matrix["migration_chain"].append({"version": 3, "previous_version": 2,
        "source_path": "crates/app-infra/src/storage/database.rs",
        "sql_sources": [{"path": "crates/app-infra/src/storage/migrations/003_http_service_identity.sql", "sha256": DESIGN_SHA256}],
        "migration_sha256": DESIGN_SHA256})
    for route in matrix["routes"]:
        if route["from_version"] == 2:
            route["from_version"] = 3
        route["to_version"] = 3
        if route["source_schema"] == v2.CONTRACT:
            route["source_schema"] = CONTRACT
        route["target_schema"] = CONTRACT
    matrix["implementation_identity_policy"] = "outer current-source closure and actual compile/run; historical implementation hash is not current identity"
    return matrix


def acceptance_template(root: Path) -> dict:
    prior = read_json(root, "docs/architecture/database/v2/storage-acceptance.json")
    old_fields = {row["id"]: row for row in read_json(root, "docs/architecture/database/field-boundaries.json")["fields"]}
    expanded = [{"field_id": field, "original_probe_ids": old_fields[field]["probes"],
                 "original_probe_scope": "execute unchanged against exact v1/v2 fixtures",
                 "new_semantics": "SQL supports reviewed typed drafts; execute requires exact registered binding and auth/material agreement",
                 "runtime_proof_id": "V3-LEGACY-ENUM:" + field} for field in EXPANDED_ENUMS]
    return {"schema_version": 1, "id": "FP1-STORAGE-20261105-SCHEMA3-HTTP", "kind": "schema_three_http_storage_acceptance", "phase": "planning", "product_tests_executed": False,
            "schema_contract": CONTRACT, "boundary_contract": BASE + "field-boundaries.json", "prior_obligation_sources": PRIOR_FILES,
            "checks": prior["checks"], "plan": prior["plan"], "prerequisite_ids": prior["prerequisite_ids"],
            "maintenance_ids": prior["maintenance_ids"], "enablement_ids": prior["enablement_ids"],
            "required_fresh_routes": prior["required_fresh_routes"], "supported_upgrade_matrix": _v3_upgrade_matrix(prior),
            "original_checks": prior["checks"], "original_upgrade_matrix": prior["supported_upgrade_matrix"],
            "expanded_old_enum_obligations": expanded,
            "original_obligations_policy": "execute original applicable obligations; old v1/v2 enum negatives stay on exact old fixtures; never inherit old green receipts",
            "delta_check_ids": sorted(v3_check_ids(root)), "field_subassertions": ["valid", "type", "null"],
            "required_routes": ["fresh_0_to_3", "upgrade_1_to_3_with_backup", "upgrade_2_to_3_with_backup", "reopen_3", "old_client_rejects_3", "rollback_failed_3", "crash_atomic_2_or_3", "backup_restore", "reject_unknown_version", "reject_corrupt_hash_schema_fk_profile", "readonly_busy_disk_full"],
            "required_runtime_provenance": ["real_rust_compile", "unfiltered_registered_test_target", "run_id_and_nonce", "current_source_and_artifact_identity", "actual_database_snapshots", "old_rows_seals_heads_preserved", "backup_identity", "typed_vault_authority"],
            "runtime_policy": {"adapter_status": "pending_backend_registration_and_real_execution", "forbidden_substitutes": ["design_ddl_clone", "guard_fixture", "hash_only_receipt", "old_run_receipt", "unit_tests_only"], "product_status_source": "separate_runtime_receipts_only"}}


def derive_effective_v3_registry(root: Path) -> dict:
    """Pure explicit current-v3 matrix; never learn expected schema from a DB."""
    return acceptance_template(root)


def validate_acceptance_registry(root: Path, data=None) -> list[str]:
    try:
        expected = acceptance_template(root)
        return validate_prior(root) + v1._diff(expected, read_json(root, REGISTRY) if data is None else data, "schema_three_acceptance")
    except (OSError, ValueError, TypeError, KeyError, RecursionError, sqlite3.Error) as error:
        return ["schema three acceptance: " + str(error)]


def validate_delta_receipt(root: Path, receipt: dict) -> list[str]:
    """Semantic inventory only; caller must separately verify real adapter provenance."""
    errors = []
    rows = receipt.get("checks") if isinstance(receipt, dict) else None
    if not isinstance(rows, list) or any(not isinstance(r, dict) or not isinstance(r.get("id"), str) for r in rows):
        return ["v3 delta checks must be typed rows"]
    ids = [r["id"] for r in rows]
    if len(set(ids)) != len(ids):
        errors.append("duplicate v3 runtime check id")
    by_id = {r["id"]: r for r in rows}
    for identifier in sorted(v3_check_ids(root)):
        row = by_id.get(identifier, {})
        if (row.get("status") != "passed" or type(row.get("assertions")) is not int or row["assertions"] <= 0
                or type(row.get("skipped")) is not int or row["skipped"] != 0):
            errors.append("missing actual v3 assertion: " + identifier)
        if identifier.startswith("V3-FIELD:"):
            sub = row.get("field_assertions")
            if not isinstance(sub, dict) or set(sub) != {"valid", "type", "null"} or any(type(v) is not int or v <= 0 for v in sub.values()):
                errors.append("missing valid/type/null subassertions: " + identifier)
    return errors


def validate_schema_contract(root: Path) -> list[str]:
    try:
        expected = read_json(root, CONTRACT)
        actual = design_contract(root)
        return validate_prior(root) + v1._diff(expected, actual, "schema_three_contract") + validate_field_dictionary(root, actual)
    except (OSError, ValueError, TypeError, KeyError, RecursionError, sqlite3.Error) as error:
        return ["schema three contract: " + str(error)]


def compare_current_schema(root: Path, candidate: Path, database: str = "profile") -> list[str]:
    return validate_schema_contract(root) or v1.compare_schema_file(root, candidate, database, CONTRACT)


def assess(root: Path = ROOT, candidate: Path | None = None, database: str = "profile") -> dict:
    errors = compare_current_schema(root, candidate, database) if candidate else validate_schema_contract(root)
    return {"kind": "schema_three_http_contract_receipt", "status": "failed" if errors else "passed", "scope": "independent_shape_and_dictionary_only", "product_migration_executed": False, "product_acceptance": False, "errors": errors}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--database", choices=tuple(SOURCES), default="profile")
    result = assess(ROOT, **vars(parser.parse_args()))
    print(json.dumps(result, ensure_ascii=False))
    return int(result["status"] != "passed")


if __name__ == "__main__":
    raise SystemExit(main())
