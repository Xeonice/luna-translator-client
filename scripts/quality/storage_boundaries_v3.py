#!/usr/bin/env python3
"""HTTP v3 delta boundaries on disposable projections; never product acceptance."""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3

import storage_schema_v3 as schema
from database_design import ROOT, _authorizer

REGISTRY = schema.BASE + "field-boundaries.json"
FIXTURES = schema.BASE + "boundary-fixtures.json"
PROJECTION = "v3_exact_tables_without_triggers_foreign_keys_off"


def fixture_template() -> dict:
    return {"kind": "synthetic_http_v3_boundary_fixtures", "schema_version": 1, "not_product_data": True,
            "tables": {
                "config_revisions": {"revision_id": "revision", "parent_revision_id": None, "content_hash": "a" * 64, "created_at_ms": 1, "save_reason": "create", "local_validation_json": "{}", "codec_version": 2},
                "connections": {"revision_id": "revision", "connection_id": "connection", "display_name": "テスト", "protocol": "openai_compatible", "deployment": "local", "auth_mode": "api_key", "endpoint": None, "credential_pool_id": None, "credential_pool_revision": None, "proxy_mode": "disabled", "network_metadata_json": "{}", "availability_intent": 0, "adapter_binding_schema": 1, "adapter_contract_revision": 1, "compatibility_profile_id": "profile", "compatibility_profile_revision": 1, "provider_preset_id": "user.fixture", "provider_preset_revision": 1},
                "credential_refs": {"credential_id": "credential", "vault_ref": "opaque-vault-reference", "masked_label": "masked", "auth_kind": "api_key", "state": "pending", "created_at_ms": 1, "activated_at_ms": None, "revoked_at_ms": None, "cleanup_requested_at_ms": None, "cleaned_at_ms": None, "lifecycle_revision": 0, "envelope_schema": 1},
                "provider_preset_versions": {"preset_id": "user.fixture", "preset_revision": 1, "namespace": "user", "provider_id": "provider", "provider_label": "サービス", "display_name": "テスト", "protocol_id": "openai_compatible", "adapter_contract_revision": 1, "compatibility_profile_id": "profile", "compatibility_profile_revision": 1, "endpoint": "https://example.invalid/v1", "auth_kind": "api_key", "options_json": "{}", "content_hash": "b" * 64, "source_json": "[]", "created_at_ms": 1},
            }}


def build_registry(root: Path = ROOT) -> dict:
    fixtures = schema.read_json(root, FIXTURES)
    if fixtures != fixture_template():
        raise ValueError("independently declared v3 fixture changed; review required")
    contract = schema.design_contract(root)
    probes = []
    def add(name, table, changes, outcome="reject", code=sqlite3.SQLITE_CONSTRAINT_CHECK, contains="CHECK constraint failed", kind="field"):
        probes.append({"id": "V3-PROBE:" + table + "." + name, "table": table, "kind": kind, "changes": changes,
                       "expected": {"outcome": outcome, **({"sqlite_errorcode": code, "error_contains": contains} if outcome == "reject" else {})}})
    for identifier, (table, column) in schema.field_rows(contract).items():
        name = column["name"]
        add(name + ".valid", table, {}, "accept")
        add(name + ".type", table, {name: {"$bytes_hex": "00ff"}}, code=3091, contains=table + "." + name)
        nullable = not bool(column["notnull"] or column["pk"])
        mutations = {name: None}
        if table == "connections":
            mutations = {key: None for key in schema.NEW_COLUMNS["connections"]}
        add(name + ".null", table, mutations, "accept" if nullable else "reject", sqlite3.SQLITE_CONSTRAINT_NOTNULL, table + "." + name)
        if column["type"] == "INTEGER":
            for suffix, value in [("upper", 2**63), ("lower", -(2**63)-1)]:
                probes.append({"id": "V3-PROBE:" + table + "." + name + ".physical_i64_" + suffix,
                               "table": table, "kind": "physical_driver_boundary", "changes": {name: {"$integer": str(value)}},
                               "expected": {"outcome": "binding_reject", "error_class": "OverflowError"}})
            add(name + ".fraction", table, {name: 1.5}, code=3091, contains=table + "." + name)
            if name in {"codec_version", "envelope_schema"}:
                for suffix, value in [("zero", 0), ("unknown", 3)]:
                    add(name + "." + suffix, table, {name: value})
            elif name == "created_at_ms":
                add(name + ".negative", table, {name: -1})
                add(name + ".zero", table, {name: 0}, "accept")
            elif name == "adapter_binding_schema":
                add(name + ".unknown", table, {name: 2})
            else:
                for suffix, value, outcome in [("zero", 0, "reject"), ("negative", -1, "reject"), ("u32_max", 4294967295, "accept"), ("u32_overflow", 4294967296, "reject")]:
                    add(name + "." + suffix, table, {name: value}, outcome)
        if name in {"preset_id", "provider_id", "protocol_id", "compatibility_profile_id", "content_hash"}:
            base = fixtures["tables"][table][name]
            add(name + ".nul", table, {name: base + "\0tail"})
            add(name + ".empty", table, {name: ""})
            if name == "content_hash":
                for suffix, value in [("short", "a" * 63), ("upper", "A" * 64), ("nonhex", "g" * 64)]:
                    add(name + "." + suffix, table, {name: value})
            else:
                add(name + ".invalid_ascii", table, {name: base + "/escape"})
                add(name + ".too_long", table, {name: base + "x" * 161})
        if name in {"provider_label", "display_name"}:
            add(name + ".empty", table, {name: ""})
            add(name + ".nul", table, {name: "visible\0tail"})
        if name in {"options_json", "source_json"}:
            for suffix, value in [("malformed", "{"), ("wrong_top", "[]" if name == "options_json" else "{}"), ("nul", ("{}" if name == "options_json" else "[]") + "\0tail")]:
                add(name + "." + suffix, table, {name: value})
        if name == "endpoint":
            for suffix, value in [("nul", "https://example.invalid\0tail"), ("query", "https://example.invalid?key=x"), ("fragment", "https://example.invalid#x")]:
                add(name + "." + suffix, table, {name: value})
        if name == "namespace":
            add("namespace.unknown", table, {name: "arbitrary"})
            add("namespace.mismatch", table, {name: "builtin"})
            add("namespace.builtin", table, {name: "builtin", "preset_id": "builtin.fixture"}, "accept")
        if name == "auth_kind":
            add("auth_kind.unknown", table, {name: "headers"})
    binding = schema.NEW_COLUMNS["connections"][:4]
    for mask in range(16):
        mutations = {key: fixtures["tables"]["connections"][key] if mask & (1 << index) else None for index, key in enumerate(binding)}
        mutations.update(provider_preset_id=None, provider_preset_revision=None)
        add("binding_mask." + str(mask), "connections", mutations, "accept" if mask in {0, 15} else "reject", kind="nullable_group")
    for mask in range(4):
        add("preset_mask." + str(mask), "connections", {key: fixtures["tables"]["connections"][key] if mask & (1 << i) else None for i, key in enumerate(schema.NEW_COLUMNS["connections"][4:])}, "accept" if mask in {0, 3} else "reject", kind="nullable_group")
    add("envelope.legacy_raw", "credential_refs", {"envelope_schema": None}, "accept", kind="metadata")
    add("envelope.typed_app", "credential_refs", {"auth_kind": "baidu_app_secret"}, "accept", kind="metadata")
    add("envelope.app_without_schema", "credential_refs", {"auth_kind": "baidu_app_secret", "envelope_schema": None}, kind="metadata")
    for name, table, changes in [("protocol", "connections", {"protocol": "deepl_v2"}), ("auth_mode", "connections", {"auth_mode": "baidu_app_secret"}), ("auth_kind", "credential_refs", {"auth_kind": "youdao_app_secret"})]:
        add("expanded_old_enum." + name, table, changes, "accept", kind="expanded_old_enum")
    return {"schema_version": 1, "kind": "schema_three_http_boundary_registry", "projection": PROJECTION, "not_product_acceptance": True,
            "source_design_sha256": schema.DESIGN_SHA256, "fixtures_sha256": hashlib.sha256(schema.safe_bytes(root, FIXTURES)).hexdigest(),
            "field_ids": sorted(schema.field_rows(contract)), "expanded_old_enums": schema.EXPANDED_ENUMS,
            "relationship_ids": ["preset_fk_protocol", "preset_fk_revision", "preset_fk_profile", "preset_immutable_update", "preset_immutable_delete", "preset_secret_option", "preset_nested_option", "credential_schema_immutable", "connection_immutable", "sealed_connection_insert", "sealed_connection_delete", "legacy_codec_extension"],
            "probes": probes}


def _insert(db, table, row):
    names = list(row)
    def decode(value):
        if isinstance(value, dict) and set(value) == {"$bytes_hex"}:
            return bytes.fromhex(value["$bytes_hex"])
        if isinstance(value, dict) and set(value) == {"$integer"}:
            return int(value["$integer"])
        return value
    values = [decode(v) for v in row.values()]
    db.execute('INSERT INTO "' + table + '" (' + ','.join('"' + n + '"' for n in names) + ') VALUES (' + ','.join('?' for _ in names) + ')', values)


def _probe(sql: str, table: str, control: dict, changes: dict, expected: dict) -> list[str]:
    with closing(sqlite3.connect(":memory:")) as db:
        db.set_authorizer(_authorizer)
        db.execute("PRAGMA foreign_keys=OFF")
        db.execute(sql)
        errors = []
        try:
            _insert(db, table, control)
            db.execute('DELETE FROM "' + table + '"')
        except sqlite3.Error as error:
            return ["positive control failed: " + str(error)]
        failure = None
        try:
            _insert(db, table, control | changes)
        except (sqlite3.Error, OverflowError) as error:
            failure = error
        if expected["outcome"] == "accept" and failure:
            errors.append("expected accepted row: " + str(failure))
        if expected["outcome"] == "reject" and (failure is None or getattr(failure, "sqlite_errorcode", None) != expected["sqlite_errorcode"] or expected["error_contains"] not in str(failure)):
            errors.append("wrong or missing rejection: " + str(failure))
        if expected["outcome"] == "binding_reject" and type(failure).__name__ != expected["error_class"]:
            errors.append("wrong driver rejection: " + str(failure))
        db.execute('DELETE FROM "' + table + '"')
        _insert(db, table, control)
        if db.execute("PRAGMA ignore_check_constraints").fetchone()[0] or db.execute("SELECT count(*) FROM sqlite_schema WHERE type='trigger'").fetchone()[0]:
            errors.append("projection unexpectedly masked field enforcement")
        return errors


def relationship_checks(root: Path, fixtures: dict) -> list[dict]:
    results = []
    def run(name, operation, contains):
        with schema.design_database(root) as db:
            for table in ["config_revisions", "provider_preset_versions", "credential_refs"]:
                _insert(db, table, fixtures[table])
            try:
                operation(db)
                errors = ["required relationship/trigger rejection absent"]
            except sqlite3.IntegrityError as error:
                errors = [] if contains in str(error) else ["wrong rejection: " + str(error)]
            results.append({"id": "V3-RELATION:" + name, "status": "failed" if errors else "passed", "assertions": 1, "errors": errors})
    connection = fixtures["connections"]
    for name, changes in [("preset_fk_protocol", {"protocol": "anthropic"}), ("preset_fk_revision", {"provider_preset_revision": 2}), ("preset_fk_profile", {"compatibility_profile_revision": 2})]:
        run(name, lambda db, changes=changes: _insert(db, "connections", connection | changes), "FOREIGN KEY constraint failed")
    run("preset_immutable_update", lambda db: db.execute("UPDATE provider_preset_versions SET display_name='other'"), "immutable_provider_preset")
    run("preset_immutable_delete", lambda db: db.execute("DELETE FROM provider_preset_versions"), "immutable_provider_preset")
    for name, options in [("preset_secret_option", '{"api_key":"forbidden"}'), ("preset_nested_option", '{"nested":{"value":1}}')]:
        run(name, lambda db, options=options: _insert(db, "provider_preset_versions", fixtures["provider_preset_versions"] | {"preset_id": "user.other", "options_json": options}), "protected_provider_preset_option")
    run("credential_schema_immutable", lambda db: db.execute("UPDATE credential_refs SET envelope_schema=NULL,lifecycle_revision=1"), "immutable_credential_identity")
    def connection_update(db):
        _insert(db, "connections", connection)
        db.execute("UPDATE connections SET display_name='other'")
    run("connection_immutable", connection_update, "immutable_connection_snapshot")
    def sealed(db, delete):
        _insert(db, "connections", connection)
        db.execute("INSERT INTO config_revision_seals VALUES('revision',?,1)", ["a" * 64])
        if delete:
            db.execute("DELETE FROM connections")
        else:
            _insert(db, "connections", connection | {"connection_id": "another"})
    run("sealed_connection_insert", lambda db: sealed(db, False), "sealed_config_snapshot")
    run("sealed_connection_delete", lambda db: sealed(db, True), "sealed_config_snapshot")
    def legacy(db):
        row = fixtures["config_revisions"] | {"revision_id": "legacy", "codec_version": 1}
        _insert(db, "config_revisions", row)
        _insert(db, "connections", connection | {"revision_id": "legacy"})
    run("legacy_codec_extension", legacy, "legacy_codec_extension_forbidden")
    return results


def assess(root: Path = ROOT, candidate: Path | None = None) -> dict:
    errors, results = [], []
    try:
        errors += schema.validate_schema_contract(root)
        registry = build_registry(root)
        errors += schema.v1._diff(registry, schema.read_json(root, REGISTRY), "v3_boundaries")
        fixtures = schema.read_json(root, FIXTURES)["tables"]
        candidate_digest = None
        if candidate:
            errors += schema.compare_current_schema(root, candidate)
            if not errors:
                with candidate.open("rb") as stream:
                    candidate_digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if not errors:
            if candidate:
                db = sqlite3.connect(candidate.resolve().as_uri() + "?mode=ro&immutable=1", uri=True)
                db.set_authorizer(_authorizer)
                try:
                    tables = {n: sql for n, sql in db.execute("SELECT name,sql FROM sqlite_schema WHERE type='table'")}
                finally:
                    db.close()
            else:
                with schema.design_database(root) as db:
                    tables = {n: sql for n, sql in db.execute("SELECT name,sql FROM sqlite_schema WHERE type='table'")}
            for probe in registry["probes"]:
                failures = _probe(tables[probe["table"]], probe["table"], fixtures[probe["table"]], probe["changes"], probe["expected"])
                results.append({"id": probe["id"], "status": "failed" if failures else "passed", "assertions": 3, "errors": failures})
            # Relationship probes execute reviewed full design, not a live candidate.
            # Candidate relationship integrity and exact trigger bytes were checked above.
            results += relationship_checks(root, fixtures)
            errors += [r["id"] + ": " + "; ".join(r["errors"]) for r in results if r["errors"]]
            if candidate:
                errors += schema.v1.sealed_snapshot_errors(candidate)
                with candidate.open("rb") as stream:
                    if hashlib.file_digest(stream, "sha256").hexdigest() != candidate_digest:
                        errors.append("candidate changed during read-only boundary inspection")
    except (OSError, ValueError, TypeError, KeyError, RecursionError, sqlite3.Error) as error:
        errors.append("v3 boundaries: " + str(error))
    return {"kind": "schema_three_http_boundary_receipt", "status": "failed" if errors else "passed", "scope": "delta_SQL_boundaries_only", "not_product_acceptance": True,
            "candidate_readonly": candidate is not None, "relationship_probe_source": "independent_design_only", "checks": results, "errors": errors}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path)
    result = assess(ROOT, parser.parse_args().candidate)
    print(json.dumps(result, ensure_ascii=False))
    return int(result["status"] != "passed")


if __name__ == "__main__":
    raise SystemExit(main())
