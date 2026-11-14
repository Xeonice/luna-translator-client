#!/usr/bin/env python3
"""Independent schema-two contract from reviewed design, never production DDL."""
from __future__ import annotations
import argparse
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import storage_schema as v1
from database_design import SOURCES, ROOT, _authorizer

DESIGN = "docs/architecture/database/v2/002_session_records.design.sql"
CONTRACT = "docs/architecture/database/v2/schema-contract.json"
DICTIONARY = "docs/architecture/database/v2/field-dictionary.json"
DESIGN_SHA256 = "ec4d1470721b9d81e20a3c5e1bb12aa6f59a2b3c32f6310691f332394781ca2e"
TABLES = {"capture_operations", "hook_events", "body_record_positions", "reading_preference_settings"}

def design_contract(root: Path) -> dict:
    original = v1.design_contract(root)
    source = (root / DESIGN).read_bytes()
    if hashlib.sha256(source).hexdigest() != DESIGN_SHA256:
        raise ValueError("schema-two reviewed design changed; independent review required")
    with closing(sqlite3.connect(":memory:")) as db:
        db.set_authorizer(_authorizer)
        db.execute("PRAGMA foreign_keys=ON")
        for name in SOURCES["profile"]:
            db.executescript((root / name).read_text())
        before = v1.inspect_connection(db)
        db.executescript(source.decode())
        after = v1.inspect_connection(db)
    if set(after["tables"]) - set(before["tables"]) != TABLES:
        raise ValueError("schema-two must add exactly the four reviewed tables")
    old_projection = json.loads(json.dumps(after))
    for name in TABLES:
        del old_projection["tables"][name]
    # The single reviewed insert trigger is attached to the immutable original
    # revisions table. Every other old field/index/FK/trigger remains exact.
    old_projection["tables"]["reading_unit_revisions"]["triggers"].pop("body_record_position")
    differences = v1._diff(before, old_projection, "schema_one_preservation")
    if differences:
        raise ValueError("; ".join(differences))
    original["databases"]["profile"] = after
    original["ddl_source_sha256"][DESIGN] = DESIGN_SHA256
    original["phase"] = "independently_reviewed_schema_two_implementation_contract"
    original["migration_version"] = 2
    original["prior_contract_sha256"] = hashlib.sha256((root / v1.CONTRACT).read_bytes()).hexdigest()
    return original

def validate_schema_contract(root: Path) -> list[str]:
    try:
        expected = v1._read_contract(root, CONTRACT)
        actual = design_contract(root)
        return (v1.validate_schema_contract(root)
                + v1._diff(expected, actual, "schema_two_contract")
                + validate_field_dictionary(root, actual))
    except (OSError, ValueError, sqlite3.Error) as error:
        return [f"schema two contract: {error}"]

def validate_field_dictionary(root: Path, contract: dict | None = None) -> list[str]:
    """The readable delta must name exactly every reviewed column and its codec."""
    contract = design_contract(root) if contract is None else contract
    dictionary_path = root / DICTIONARY
    if (any(parent.is_symlink() for parent in [dictionary_path, *dictionary_path.parents])
            or not dictionary_path.resolve().is_relative_to(root.resolve())):
        raise ValueError("schema-two dictionary path escapes repository")
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate schema-two dictionary key: " + key)
            result[key] = value
        return result
    data = json.loads(dictionary_path.read_text(), object_pairs_hook=pairs)
    errors = []
    if not isinstance(data, dict):
        return ["schema-two field dictionary must be an object"]
    if (type(data.get("schema_version")) is not int or data.get("schema_version") != 1 or data.get("kind") != "reviewed_schema_two_field_delta"
            or data.get("previous_fields_unchanged") != 431 or data.get("new_fields") != 29
            or data.get("total_fields") != 460 or data.get("schema_contract") != CONTRACT):
        errors.append("schema-two field dictionary identity/count differs")
    rows = data.get("fields", [])
    if not isinstance(rows, list):
        return errors + ["schema-two field dictionary fields must be a list"]
    wanted = {}
    for table in TABLES:
        for column in contract["databases"]["profile"]["tables"][table]["columns"]:
            wanted[f"profile.{table}.{column['name']}"] = (table, column)
    ids = [row.get("id") for row in rows if isinstance(row, dict)]
    if (len(rows) != 29 or len(ids) != len(rows)
            or any(not isinstance(identifier, str) for identifier in ids)
            or len(set(ids)) != len(ids) or set(ids) != set(wanted)):
        return errors + ["schema-two field dictionary must contain exactly the 29 unique reviewed fields"]
    for row in rows:
        if set(row) != {"id", "table", "field", "type", "nullable", "primary_key_position", "default_tokens", "description", "source"}:
            errors.append(f"schema-two field {row['id']} has missing or unknown dictionary properties")
        table, column = wanted[row["id"]]
        expected = {"table": table, "field": column["name"], "type": column["type"],
                    # INTEGER PRIMARY KEY accepts NULL as an allocation request,
                    # but its persisted value is always an integer identity.
                    "nullable": not bool(column["notnull"] or column["pk"]), "primary_key_position": column["pk"],
                    "default_tokens": column["dflt_value"], "source": DESIGN}
        for key, value in expected.items():
            if row.get(key) != value or type(row.get(key)) is not type(value):
                errors.append(f"schema-two field {row['id']} {key} differs from independently reviewed DDL")
        if not isinstance(row.get("description"), str) or not row["description"].strip():
            errors.append(f"schema-two field {row['id']} lacks description")
    return errors

def compare_current_schema(root: Path, candidate: Path, database: str) -> list[str]:
    errors = validate_schema_contract(root)
    return errors or v1.compare_schema_file(root, candidate, database, CONTRACT)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--freeze-reviewed", action="store_true")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--database", choices=tuple(SOURCES), default="profile")
    args = parser.parse_args()
    if args.freeze_reviewed:
        (ROOT / CONTRACT).write_text(json.dumps(design_contract(ROOT), ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    errors = compare_current_schema(ROOT, args.candidate, args.database) if args.candidate else validate_schema_contract(ROOT)
    print(json.dumps({"status":"failed" if errors else "passed", "scope":"independent_schema_two_shape_only", "runtime_migration_verified":False, "errors":errors}, ensure_ascii=False))
    return int(bool(errors))

if __name__ == "__main__":
    raise SystemExit(main())
