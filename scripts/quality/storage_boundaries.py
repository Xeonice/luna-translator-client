#!/usr/bin/env python3
"""Execute field-level design boundaries on exact table DDL in disposable SQLite.

The projection deliberately omits triggers and disables foreign keys. It retains
all original columns, CHECK, primary keys, and UNIQUE constraints/indexes, so an
immutable-snapshot UPDATE trigger cannot conceal a broken column constraint.
This layer never claims relationship, migration-adapter, or product acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import math
from pathlib import Path
import re
import sqlite3
from typing import Any

from database_design import ROOT, build_metadata

REGISTRY = "docs/architecture/database/field-boundaries.json"
FIXTURES = "docs/architecture/database/boundary-fixtures.json"
PROJECTION = "original_tables_without_triggers_foreign_keys_off"
TYPE_ERROR = 3091
CHECK_ERROR = sqlite3.SQLITE_CONSTRAINT_CHECK
NULL_ERROR = sqlite3.SQLITE_CONSTRAINT_NOTNULL
INTEGER_MIN, INTEGER_MAX = -(2**63), 2**63 - 1


def canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)


def sha(value: str | bytes) -> str:
    return hashlib.sha256(value.encode() if isinstance(value, str) else value).hexdigest()


def ident(value: str) -> str:
    return '"' + value.replace('"', '""') + '"'


def decode(value: Any) -> Any:
    if isinstance(value, dict) and set(value) == {"$bytes_hex"}:
        return bytes.fromhex(value["$bytes_hex"])
    if isinstance(value, dict) and set(value) == {"$integer"}:
        return int(value["$integer"])
    if isinstance(value, dict) and set(value) == {"$float"}:
        return {"positive_infinity": math.inf, "negative_infinity": -math.inf, "nan": math.nan}[value["$float"]]
    return value


def encode(value: Any) -> Any:
    if isinstance(value, bytes):
        return {"$bytes_hex": value.hex()}
    if isinstance(value, float) and not math.isfinite(value):
        return {"$float": "nan" if math.isnan(value) else "positive_infinity" if value > 0 else "negative_infinity"}
    return value


def checks(sql: str) -> list[str]:
    """Extract balanced CHECK expressions while respecting SQL string literals."""
    result = []
    for match in re.finditer(r"\bCHECK\s*\(", sql, re.I):
        start, i, depth, quoted = match.end(), match.end(), 1, False
        while depth and i < len(sql):
            char = sql[i]
            if char == "'":
                if quoted and sql[i:i + 2] == "''":
                    i += 2
                    continue
                quoted = not quoted
            elif not quoted and char == "(":
                depth += 1
            elif not quoted and char == ")":
                depth -= 1
            i += 1
        if depth:
            raise ValueError("unbalanced CHECK expression")
        result.append(sql[start:i - 1].strip())
    return result


def table_map(metadata: dict) -> dict[str, dict]:
    return {f"{db['name']}.{table['name']}": {**table, "database": db["name"]}
            for db in metadata["databases"] for table in db["tables"]}


class Projection:
    def __init__(self, table: dict):
        self.table = table
        self.db = sqlite3.connect(":memory:")
        self.db.execute("PRAGMA foreign_keys=OFF")
        self.db.execute(table["create_sql"])
        for index in table["indexes"]:
            if index["sql"]:
                self.db.execute(index["sql"])
        actual = self.db.execute("SELECT sql FROM sqlite_schema WHERE type='table'").fetchone()[0]
        if actual != table["create_sql"]:
            raise ValueError(f"{table['name']}: projection changed original CREATE TABLE")
        if self.db.execute("SELECT count(*) FROM sqlite_schema WHERE type='trigger'").fetchone()[0]:
            raise ValueError("projection must not contain triggers")
        if self.db.execute("PRAGMA foreign_keys").fetchone()[0]:
            raise ValueError("field projection must isolate foreign keys")

    def close(self):
        self.db.close()

    def violations(self, row: dict) -> list[str] | None:
        # Diagnostic truth table only; this connection never supplies a passing
        # enforcement receipt. CASE WHEN NOT(expr) exactly preserves SQLite
        # CHECK semantics: zero fails, nonzero and NULL pass, AND short-circuits.
        diagnostic = sqlite3.connect(":memory:")
        try:
            diagnostic.execute("PRAGMA foreign_keys=OFF")
            diagnostic.execute(self.table["create_sql"])
            diagnostic.execute("PRAGMA ignore_check_constraints=ON")
            names = list(row)
            diagnostic.execute(f"INSERT INTO {ident(self.table['name'])} ({','.join(map(ident,names))}) VALUES ({','.join('?' for _ in names)})", tuple(decode(row[name]) for name in names))
            return [expression for expression in checks(self.table["create_sql"])
                    if diagnostic.execute("SELECT CASE WHEN NOT (" + expression + ") THEN 0 ELSE 1 END FROM " + ident(self.table["name"])).fetchone()[0] == 0]
        except sqlite3.Error:
            return None
        finally:
            diagnostic.close()


    def insert(self, row: dict) -> tuple[dict | None, dict | None]:
        self.db.execute("DELETE FROM " + ident(self.table["name"]))
        names = list(row)
        sql = f"INSERT INTO {ident(self.table['name'])} ({','.join(map(ident, names))}) VALUES ({','.join('?' for _ in names)})"
        try:
            self.db.execute(sql, tuple(decode(row[name]) for name in names))
            names = [field["name"] for field in self.table["fields"]]
            result = self.db.execute("SELECT " + ",".join(f"{ident(name)},typeof({ident(name)})" for name in names) + " FROM " + ident(self.table["name"])).fetchone()
            return {name: {"value": encode(result[i * 2]), "type": result[i * 2 + 1]} for i, name in enumerate(names)}, None
        except sqlite3.Error as exc:
            return None, {"sqlite_errorcode": exc.sqlite_errorcode, "error": str(exc)}
        except OverflowError as exc:
            return None, {"phase": "python_sqlite_binding", "error_class": "OverflowError", "error": str(exc)}


def _enums(field: dict) -> list:
    match = re.search(r"\b" + re.escape(field["name"]) + r"\s+IN\s*\(([^()]*)\)", field["declaration"], re.I)
    if not match:
        return []
    return [item[1:-1].replace("''", "'") if item.startswith("'") else int(item)
            for item in (part.strip() for part in match[1].split(","))]


def candidates(field: dict) -> list:
    name, declaration, kind = field["name"], field["declaration"], field["type"]
    if kind == "BLOB":
        values = [{"$bytes_hex": ""}, {"$bytes_hex": "00ff01"}]
    elif kind == "INTEGER":
        values = [0, 1, -1, 2, INTEGER_MAX, INTEGER_MIN, 4294967295, 4294967296]
    elif kind == "REAL":
        values = [1.0, 0.5, 0.0, -0.5, 1.7976931348623157e308]
    else:
        values = ["boundary-value", "", "図書館", "\U0001f338", "𠮷", "が", "a\n'b\"c", "1", "0", "-1", "01", "+1", "1.0", " 1", "1 ", "１", "1e3", "\0", "x\0tail", "x\0", "18446744073709551615", "18446744073709551616", "9" * 21]
        if "json" in declaration.lower() or name.endswith("_json"):
            values = ["{}", "[]", "0", "null", '"値"', "true", "{", '{"nested":["図",0,null]}', '["図",0,null]', '{"text":"\\u0000"}', '["\\u0000"]', '\0{}', '{\0}', '{}\0tail', '[]\0tail', '0\0tail'] + values
        if "sha256" in name or "hash" in name or name == "cache_key":
            values = ["a" * 64, "0" * 64, "a" * 63, "a" * 65, "A" * 64, "g" * 64, "\0" + "a" * 64, "a" * 32 + "\0" + "a" * 32, "a" * 64 + "\0tail"] + values
        if name.endswith("_u64"):
            values = ["0", "1", "18446744073709551615", "18446744073709551616", "9007199254740991", "9007199254740992", "", "01", "-1", "+1", "1.0", " 1", "1 ", "１", "1e3", "\01", "1\0tail", "18446744073709551615\0tail", "9" * 21] + values
    result = []
    for value in [*_enums(field), *values, None]:
        if canonical(value) not in {canonical(item) for item in result}:
            result.append(value)
    return result


def _unparenthesize(expression: str) -> str:
    expression = expression.strip()
    while expression.startswith("(") and expression.endswith(")"):
        depth, quote, end = 0, False, None
        for index, char in enumerate(expression):
            if char == "'":
                quote = not quote
            elif not quote and char == "(":
                depth += 1
            elif not quote and char == ")":
                depth -= 1
                if depth == 0:
                    end = index
                    break
        if end != len(expression) - 1:
            break
        expression = expression[1:-1].strip()
    return expression


def _boolean_parts(expression: str, operator: str) -> list[str]:
    depth, quote, start, result, index = 0, False, 0, [], 0
    while index < len(expression):
        char = expression[index]
        if char == "'":
            if quote and expression[index:index+2] == "''":
                index += 2
                continue
            quote = not quote
        elif not quote:
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            elif depth == 0 and expression[index:index+len(operator)].upper() == operator and (index == 0 or not expression[index-1].isalnum()) and (index+len(operator) == len(expression) or not expression[index+len(operator)].isalnum()):
                result.append(expression[start:index].strip())
                index += len(operator)
                start = index
                continue
        index += 1
    return [*result, expression[start:].strip()] if result else [expression]


def _required_enum_values(expression: str, column: str) -> set[str] | None:
    """Sound finite-value implication for =/IN joined by AND/OR, not sampling."""
    expression = _unparenthesize(expression)
    parts = _boolean_parts(expression, "OR")
    if len(parts) > 1:
        branches = [_required_enum_values(part, column) for part in parts]
        return None if any(branch is None for branch in branches) else set().union(*branches)
    parts = _boolean_parts(expression, "AND")
    if len(parts) > 1:
        constrained = [values for part in parts if (values := _required_enum_values(part, column)) is not None]
        return set.intersection(*constrained) if constrained else None
    equal = re.fullmatch(re.escape(column) + r"\s*=\s*'([^']*)'", expression)
    if equal:
        return {equal[1]}
    enum = re.fullmatch(re.escape(column) + r"\s+IN\s*\(([^)]+)\)", expression, re.I)
    if enum and re.fullmatch(r"\s*'[^']*'(?:\s*,\s*'[^']*')*\s*", enum[1]):
        return set(re.findall(r"'([^']*)'", enum[1]))
    return None


def redundancy_certificate(table: dict, target: str, expression_map: dict) -> dict | None:
    enum = re.fullmatch(r"(\w+)\s+IN\s*\(([^)]+)\)", target, re.I)
    if enum and re.fullmatch(r"\s*'[^']*'(?:\s*,\s*'[^']*')*\s*", enum[2]):
        column, allowed = enum[1], set(re.findall(r"'([^']*)'", enum[2]))
        notnull = next(field for field in table["fields"] if field["name"] == column)["nullable"] is False
        if notnull:
            for other in expression_map:
                values = _required_enum_values(other, column)
                if other != target and values is not None and values <= allowed:
                    return {"kind": "boolean_finite_value_implication", "premises": [expression_map[other]], "column": column, "not_null": True, "required_values": sorted(values), "target_values": sorted(allowed)}
    # For non-null numeric columns: a>=b and b>=k entail a>=k.
    lower = re.fullmatch(r"(\w+) IS NULL OR \1\s*>=\s*(-?\d+)", target, re.I)
    if lower:
        column, minimum = lower[1], int(lower[2])
        fields = {field["name"]: field for field in table["fields"]}
        for other in expression_map:
            relation = re.fullmatch(re.escape(column) + r"\s*>=\s*(\w+)", other)
            if not relation or fields[column]["nullable"] or fields[relation[1]]["nullable"]:
                continue
            for bound in expression_map:
                match = re.fullmatch(re.escape(relation[1]) + r" IS NULL OR " + re.escape(relation[1]) + r"\s*>=\s*(-?\d+)", bound, re.I)
                if match and int(match[1]) >= minimum:
                    return {"kind": "nonnull_numeric_transitivity", "premises": [expression_map[other], expression_map[bound]], "columns": [column, relation[1]], "minimum": minimum}
    return None


def semantic_domain(field: dict, value: Any) -> tuple[bool | None, str]:
    """Independent domain oracle; known invalids cannot be learnt as successes.

    None means no declared semantic family, not that a boundary is untested.
    Domain-valid values still need a legal full-row context (cross-field rules).
    """
    if value is None:
        return None, "sql_null_is_checked_separately"
    name, declaration = field["name"], field["declaration"]
    enum = _enums(field)
    if enum:
        return value in enum, "declared_enum"
    if name.endswith("_u64"):
        return isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value) is not None and int(value) <= 2**64-1, "canonical_u64"
    if "sha256" in name or name in {"content_hash", "request_semantics_hash", "cache_key"}:
        return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None, "lowercase_sha256"
    if "json_valid" in declaration.lower() or name.endswith("_json"):
        def reject_constant(value):
            raise ValueError(value)
        try:
            if not isinstance(value, str) or "\0" in value:
                return False, "whole_json"
            parsed = json.loads(value, parse_constant=reject_constant)
        except (ValueError, TypeError):
            return False, "whole_json"
        top = re.search(r"json_type\([^)]*\)\s*=\s*'(object|array)'", declaration)
        valid = not top or (isinstance(parsed, dict) if top[1] == "object" else isinstance(parsed, list))
        return bool(valid), "whole_json_" + (top[1] if top else "value")
    if field["type"] in {"INTEGER", "REAL"} and isinstance(value, (int, float)):
        between = re.search(r"\b" + re.escape(name) + r"\s+BETWEEN\s+(-?\d+)\s+AND\s+(-?\d+)", declaration, re.I)
        if between:
            return int(between[1]) <= value <= int(between[2]), "declared_numeric_between"
        lower = re.search(r"\b" + re.escape(name) + r"\s*(>=|>)\s*(-?\d+)(?![\w.])", declaration)
        if lower:
            return value >= int(lower[2]) if lower[1] == ">=" else value > int(lower[2]), "declared_numeric_lower"
    nonempty = re.search(r"length\(\s*" + re.escape(name) + r"\s*\)\s*>\s*0", declaration)
    if nonempty and isinstance(value, str):
        # SQLite TEXT length counts Unicode scalars only before U+0000. This
        # family has no blanket NUL ban; do not invent one for original text.
        return len(value.split("\0", 1)[0]) > 0, "declared_nonempty_text"
    return None, "unconstrained_scalar_or_cross_field"


def _row(fixture: dict, context: int, mutations: dict) -> dict:
    return fixture["valid_row"] | (fixture["valid_contexts"][context] if context >= 0 else {}) | mutations


def _contexts(fixture: dict):
    return range(-1, len(fixture["valid_contexts"]))


def _known_type(field: dict) -> str:
    return {"INTEGER": "integer", "REAL": "real", "TEXT": "text", "BLOB": "blob"}[field["type"]]


def build_registry(root: Path = ROOT) -> tuple[dict, list[str]]:
    metadata, errors = build_metadata(root)
    try:
        fixtures = json.loads((root / FIXTURES).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return {}, [*errors, f"invalid boundary fixtures: {exc}"]
    tables = table_map(metadata)
    for table in tables.values():
        for field in table["fields"]:
            if field["type"] not in {"TEXT", "INTEGER", "REAL", "BLOB"}:
                errors.append(f"{table['name']}.{field['name']}: unknown field type boundary rule {field['type']}")
        declared_checks = checks(table["create_sql"])
        if len(declared_checks) != len(set(declared_checks)):
            errors.append(f"{table['name']}: duplicate CHECK expressions need explicit occurrence identities")
    if set(fixtures.get("tables", {})) != set(tables):
        errors.append("boundary fixtures must cover precisely every original table")
    if errors:
        return {}, errors
    fields_out, checks_out, probes, blocked = [], [], [], []
    for key, table in tables.items():
        fixture = fixtures["tables"][key]
        if set(fixture["valid_row"]) != {field["name"] for field in table["fields"]}:
            errors.append(f"{key}: valid_row must explicitly define every original column")
            continue
        if any(set(context) - set(fixture["valid_row"]) for context in fixture["valid_contexts"]):
            errors.append(f"{key}: context contains unknown columns")
            continue
        projection = Projection(table)
        try:
            for context in _contexts(fixture):
                _, failure = projection.insert(_row(fixture, context, {}))
                if failure:
                    errors.append(f"{key}: illegal positive fixture context {context}: {failure}")
            if errors:
                continue
            expression_map = {expression: f"{key}.check.{sha(expression)[:16]}" for expression in checks(table["create_sql"])}
            table_probes = []

            def add(suffix, kind, context, mutations, expected, field=None, check_id=None):
                probe = {"id": f"{key}.{suffix}", "database": table["database"], "table": table["name"], "kind": kind, "context": context, "mutations": mutations, "expected": expected}
                if field:
                    probe["field"] = field
                if check_id:
                    probe["check_id"] = check_id
                table_probes.append(probe)
                return probe["id"]

            def expected_check(failure):
                if not failure or failure.get("sqlite_errorcode") != CHECK_ERROR:
                    return None
                prefix = "CHECK constraint failed: "
                expression = failure["error"][len(prefix):] if failure["error"].startswith(prefix) else ""
                return expression_map.get(expression)

            for field in table["fields"]:
                name = field["name"]
                field_id = f"{key}.{name}"
                own = []
                own.append(add(f"field.{name}.positive", "field_positive_control", -1, {}, {"outcome": "accept"}, name))
                # Use INSERT, never UPDATE: immutable triggers cannot mask types.
                bad = "not-a-blob" if field["type"] == "BLOB" else {"$bytes_hex": "ff00"}
                primary_integer = field["type"] == "INTEGER" and bool(field["primary_key_position"]) and len(table["primary_key"]) == 1
                code, message = (sqlite3.SQLITE_MISMATCH, "datatype mismatch") if primary_integer else (TYPE_ERROR, f"{table['name']}.{name}")
                own.append(add(f"field.{name}.wrong_type", "field_type_reject", -1, {name: bad}, {"outcome": "reject", "sqlite_errorcode": code, "error_contains": message}, name))
                if field["type"] in {"INTEGER", "REAL"}:
                    bad_values = ["not-a-number"] + ([1.5, "1.5"] if field["type"] == "INTEGER" else [])
                    for number, value in enumerate(bad_values):
                        own.append(add(f"field.{name}.wrong_numeric.{number}", "numeric_type_reject", -1, {name: value}, {"outcome": "reject", "sqlite_errorcode": code, "error_contains": message}, name))
                if field["type"] == "INTEGER":
                    for direction, value in (("upper", INTEGER_MAX + 1), ("lower", INTEGER_MIN - 1)):
                        own.append(add(f"field.{name}.bind_overflow.{direction}", "physical_integer_binding_boundary", -1, {name: {"$integer": str(value)}},
                            {"outcome": "binding_reject", "phase": "python_sqlite_binding", "error_class": "OverflowError"}, name))
                if field["type"] == "REAL":
                    # SQLite REAL itself accepts positive infinity. The future
                    # API must reject non-finite geometry inputs explicitly.
                    own.append(add(f"field.{name}.positive_infinity", "sqlite_real_nonfinite_storage", -1, {name: {"$float": "positive_infinity"}},
                        {"outcome": "accept", "stored_type": "real", "stored_value": {"$float": "positive_infinity"}}, name))
                    own.append(add(f"field.{name}.nan", "sqlite_nan_binds_null", -1, {name: {"$float": "nan"}},
                        {"outcome": "reject", "sqlite_errorcode": NULL_ERROR, "error_contains": f"{table['name']}.{name}"}, name))
                # NULL is checked against a valid entire row. Conditional fields
                # use a legal context; INTEGER PRIMARY KEY allocates a rowid.
                null_found = False
                for context in _contexts(fixture):
                    stored, failure = projection.insert(_row(fixture, context, {name: None}))
                    if stored is not None:
                        expectation = {"outcome": "accept", "stored_type": stored[name]["type"], "stored_value": stored[name]["value"]}
                        kind = "integer_primary_key_null_allocates" if primary_integer else "nullable_accept"
                        own.append(add(f"field.{name}.null", kind, context, {name: None}, expectation, name))
                        null_found = True
                        break
                    if not field["nullable"] and not primary_integer:
                        own.append(add(f"field.{name}.null", "required_null_reject", context, {name: None}, {"outcome": "reject", "sqlite_errorcode": NULL_ERROR, "error_contains": f"{table['name']}.{name}"}, name))
                        null_found = True
                        break
                if not null_found:
                    blocked.append({"id": f"{field_id}.null", "reason": "no independently legal NULL context; needs explicit fixture"})
                # A non-null positive value is required even for optional fields.
                found = False
                for context in _contexts(fixture):
                    base = _row(fixture, context, {})
                    for value in [base[name], *candidates(field)]:
                        if value is None:
                            continue
                        stored, failure = projection.insert(base | {name: value})
                        if stored is not None:
                            own.append(add(f"field.{name}.nonnull", "nonnull_positive", context, {name: value}, {"outcome": "accept", "stored_type": _known_type(field), "stored_value": encode(decode(value))}, name))
                            found = True
                            break
                    if found:
                        break
                if not found:
                    blocked.append({"id": f"{field_id}.nonnull", "reason": "no legal non-NULL context; needs explicit fixture"})
                if field["default"] is not None:
                    # Explicit omission, not passing the expected default as input.
                    own.append(add(f"field.{name}.default", "declared_default", -1, {}, {"outcome": "accept", "default_sql": field["default"]}, name))
                # SQLite's lossless coercions are positives where the complete
                # declaration permits them; rejected semantic coercions retain
                # the exact CHECK predicate and are not reported as type errors.
                if field["type"] in {"INTEGER", "REAL", "TEXT"}:
                    coercions = ["1"] if field["type"] in {"INTEGER", "REAL"} else [1]
                    for value in coercions:
                        for context in _contexts(fixture):
                            stored, failure = projection.insert(_row(fixture, context, {name: value}))
                            if stored is not None:
                                own.append(add(f"field.{name}.coercion", "sqlite_lossless_coercion_accept", context, {name: value}, {"outcome": "accept", "stored_type": stored[name]["type"], "stored_value": stored[name]["value"]}, name))
                                break
                        else:
                            _, failure = projection.insert(_row(fixture, -1, {name: value}))
                            check_id = expected_check(failure)
                            if check_id:
                                own.append(add(f"field.{name}.coercion", "coercion_rejected_by_check", -1, {name: value}, {"outcome": "reject", "sqlite_errorcode": CHECK_ERROR, "error_contains": failure["error"]}, name, check_id))
                            else:
                                blocked.append({"id": f"{field_id}.coercion", "reason": f"unclassified coercion result {failure}"})
                # Exhaust enum members and declared grammar/type boundaries.
                for number, value in enumerate(candidates(field)):
                    if value is None:
                        continue
                    found = None
                    for context in _contexts(fixture):
                        stored, failure = projection.insert(_row(fixture, context, {name: value}))
                        if stored is not None:
                            found = (context, {"outcome": "accept", "stored_type": stored[name]["type"], "stored_value": stored[name]["value"]}, None)
                            break
                    if found is None:
                        _, failure = projection.insert(_row(fixture, -1, {name: value}))
                        check_id = expected_check(failure)
                        if check_id:
                            expression = next(expr for expr, cid in expression_map.items() if cid == check_id)
                            if re.search(r"\b" + re.escape(name) + r"\b", expression):
                                found = (-1, {"outcome": "reject", "sqlite_errorcode": CHECK_ERROR, "error_contains": failure["error"]}, check_id)
                    domain_valid, domain_rule = semantic_domain(field, value)
                    if domain_valid is False and (found is None or found[1]["outcome"] != "reject"):
                        errors.append(f"{field_id}: independent {domain_rule} oracle requires rejection of {value!r}; SQLite accepted it or rejected an unrelated constraint")
                    if domain_valid is True and (domain_rule in {"declared_enum", "lowercase_sha256", "canonical_u64"} or domain_rule.startswith("whole_json")) and (found is None or found[1]["outcome"] != "accept"):
                        blocked.append({"id": f"{field_id}.value.{number}", "reason": f"domain-valid {domain_rule} input lacks legal full-row context: {value!r}"})
                    if found:
                        context, expectation, check_id = found
                        expectation = {**expectation, "domain_rule": domain_rule, "domain_valid": domain_valid, "rejection_scope": "first_error_only" if expectation["outcome"] == "reject" else None}
                        own.append(add(f"field.{name}.value.{number}", "declared_value_boundary", context, {name: value}, expectation, name, check_id))
                fields_out.append({"id": field_id, "database": table["database"], "table": table["name"], "field": name, "type": field["type"], "nullable": field["nullable"], "default": field["default"], "declaration_sha256": sha(field["declaration"]), "probes": own,
                    "application_boundary": "SQL scalar constraints only; application API types, finite REAL values, safe integer transport, capacity policies, semantic hashes and actual migrations require storage adapter acceptance; unfrozen capacity maxima are not invented."})
            # Distinguish truly isolated predicate violations from checks that
            # are logically implied by other original predicates. No DDL edits.
            for expression, check_id in expression_map.items():
                involved = [field["name"] for field in table["fields"] if re.search(r"\b" + re.escape(field["name"]) + r"\b", expression)]
                first_error_probes = [probe for probe in table_probes if probe.get("check_id") == check_id and probe["expected"]["outcome"] == "reject"]
                isolated = None
                for probe in first_error_probes:
                    if projection.violations(_row(fixture, probe["context"], probe["mutations"])) == [expression]:
                        isolated = (probe["context"], probe["mutations"])
                        break
                certificate = redundancy_certificate(table, expression, expression_map)
                if isolated is None and certificate is None:
                    # Search nearby correlated fields as well as direct ones.
                    # E.g. byte_end==byte_start must set surface='' so the UTF8
                    # length CHECK stays true while byte_end>byte_start fails.
                    connected = set(involved)
                    for related in expression_map:
                        if any(re.search(r"\b"+re.escape(name)+r"\b", related) for name in involved):
                            connected.update(field["name"] for field in table["fields"] if re.search(r"\b"+re.escape(field["name"])+r"\b", related))
                    connected = [field["name"] for field in table["fields"] if field["name"] in connected]
                    examined = 0
                    for width in (1, 2, 3):
                        for names in itertools.combinations(connected, width):
                            if not set(names) & set(involved):
                                continue
                            for context in _contexts(fixture):
                                domains = []
                                for name in names:
                                    field = next(field for field in table["fields"] if field["name"] == name)
                                    domain = [None, *candidates(field)[:8]] if field["nullable"] else candidates(field)[:8]
                                    domains.append(domain)
                                for values in itertools.product(*domains):
                                    examined += 1
                                    if examined > 30000:
                                        break
                                    mutations = dict(zip(names, values))
                                    _, failure = projection.insert(_row(fixture, context, mutations))
                                    if expected_check(failure) == check_id and projection.violations(_row(fixture, context, mutations)) == [expression]:
                                        isolated = (context, mutations)
                                        break
                                if isolated or examined > 30000:
                                    break
                            if isolated or examined > 30000:
                                break
                        if isolated or examined > 30000:
                            break
                positive = add(f"check.{sha(expression)[:16]}.positive", "check_positive_control", -1, {}, {"outcome": "accept"}, check_id=check_id)
                proof_ids = []
                if isolated:
                    context, mutations = isolated
                    proof_ids.append(add(f"check.{sha(expression)[:16]}.isolated", "isolated_check_reject", context, mutations,
                        {"outcome": "reject", "sqlite_errorcode": CHECK_ERROR, "error_contains": "CHECK constraint failed: " + expression, "isolated_check": expression}, check_id=check_id))
                    independence = "isolated"
                elif certificate and first_error_probes:
                    probe = first_error_probes[0]
                    violations = projection.violations(_row(fixture, probe["context"], probe["mutations"]))
                    proof_ids.append(add(f"check.{sha(expression)[:16]}.redundant", "logically_redundant_check_reject", probe["context"], probe["mutations"],
                        {"outcome": "reject", "sqlite_errorcode": CHECK_ERROR, "error_contains": "CHECK constraint failed: " + expression, "joint_checks": violations}, check_id=check_id))
                    independence = "logically_redundant"
                else:
                    blocked.append({"id": check_id, "reason": "no isolated target CHECK rejection or sound redundancy certificate; add explicit context, never accept unrelated failure"})
                    independence = "blocked"
                checks_out.append({"id": check_id, "database": table["database"], "table": table["name"], "expression": expression, "expression_sha256": sha(expression), "fields": [f"{key}.{name}" for name in involved], "probes": [positive, *proof_ids], "independence": independence, "redundancy_certificate": certificate if independence == "logically_redundant" else None})
            probes.extend(table_probes)
        finally:
            projection.close()
    registry = {"schema_version": 1, "kind": "storage_field_boundary_registry", "projection": PROJECTION, "not_product_acceptance": True,
                "source_schema_sha256": sha(canonical({key: {"create_sql": table["create_sql"], "indexes": table["indexes"]} for key, table in tables.items()})),
                "fixtures_sha256": sha((root / FIXTURES).read_bytes()), "fields": fields_out, "check_constraints": checks_out, "probes": probes, "blocked": blocked,
                "counts": {"tables": len(tables), "fields": len(fields_out), "check_constraints": len(checks_out), "probes": len(probes), "blocked": len(blocked), "isolated_checks": sum(item["independence"] == "isolated" for item in checks_out), "logically_redundant_checks": sum(item["independence"] == "logically_redundant" for item in checks_out)}}
    return registry, errors


def execute_registry(registry: dict, root: Path = ROOT) -> dict:
    metadata, errors = build_metadata(root)
    tables = table_map(metadata)
    fixtures = json.loads((root / FIXTURES).read_text())["tables"]
    projections = {key: Projection(table) for key, table in tables.items()}
    results = []
    try:
        for probe in registry["probes"]:
            key = f"{probe['database']}.{probe['table']}"
            projection, fixture = projections[key], fixtures[key]
            control = _row(fixture, probe["context"], {})
            before, failure = projection.insert(control)
            assertions = 1
            messages = []
            if failure:
                messages.append(f"positive control failed: {failure}")
            row = _row(fixture, probe["context"], probe["mutations"])
            expected = probe["expected"]
            if probe["kind"] == "declared_default":
                row.pop(probe["field"])
            stored, failure = projection.insert(row)
            assertions += 1
            if expected["outcome"] == "binding_reject":
                assertions += 2
                if not failure or failure.get("phase") != expected["phase"] or failure.get("error_class") != expected["error_class"]:
                    messages.append(f"wrong driver boundary rejection: {failure or stored}")
            elif expected["outcome"] == "accept":
                if failure:
                    messages.append(f"expected accepted row, got {failure}")
                elif "stored_type" in expected:
                    assertions += 2
                    actual = stored[probe["field"]]
                    if actual["type"] != expected["stored_type"] or actual["value"] != expected["stored_value"]:
                        messages.append(f"stored affinity/value mismatch: {actual}")
                if stored is not None and "default_sql" in expected:
                    assertions += 1
                    default = projection.db.execute("SELECT " + expected["default_sql"]).fetchone()[0]
                    if stored[probe["field"]]["value"] != default:
                        messages.append("omitted field did not store its declared default")
            else:
                assertions += 2
                if not failure or failure.get("sqlite_errorcode") != expected["sqlite_errorcode"] or expected["error_contains"] not in failure["error"]:
                    messages.append(f"wrong rejection attribution: expected {expected}, got {failure or stored}")
            if "isolated_check" in expected or "joint_checks" in expected:
                assertions += 1
                actual_violations = projection.violations(row)
                expected_violations = [expected["isolated_check"]] if "isolated_check" in expected else expected["joint_checks"]
                if actual_violations != expected_violations:
                    messages.append(f"predicate attribution mismatch: {actual_violations}")
            if projection.db.execute("PRAGMA ignore_check_constraints").fetchone()[0] != 0:
                messages.append("enforcement connection unexpectedly disabled CHECK constraints")
            after, failure = projection.insert(control)
            assertions += 1
            if failure or after != before:
                messages.append("post-negative positive control differs or failed")
            results.append({"id": probe["id"], "status": "failed" if messages else "passed", "assertions": assertions, "skipped": False, "log": messages})
    finally:
        for projection in projections.values():
            projection.close()
    errors.extend(f"{case['id']}: {case['log']}" for case in results if case["status"] != "passed")
    if registry["blocked"]:
        errors.extend(f"blocked: {item}" for item in registry["blocked"])
    return {"schema_version": 1, "kind": "storage_field_boundary_receipt", "projection": PROJECTION, "not_product_acceptance": True, "sqlite_version": sqlite3.sqlite_version,
            "registry_sha256": sha(canonical(registry)), "counts": registry["counts"], "checks": results, "errors": errors, "status": "failed" if errors else "passed"}


def validate(root: Path = ROOT, output: Path | None = None) -> dict:
    candidate, errors = build_registry(root)
    try:
        registered = json.loads((root / REGISTRY).read_text())
    except (OSError, ValueError) as exc:
        registered = None
        errors.append(f"boundary registry unreadable: {exc}")
    if registered != candidate:
        errors.append("field boundary registry differs from DDL/fixtures/probe obligations; review and explicitly --write; --check never refreshes it")
    if errors:
        receipt = {"schema_version": 1, "kind": "storage_field_boundary_receipt", "status": "failed", "projection": PROJECTION, "not_product_acceptance": True, "checks": [], "errors": errors}
    else:
        receipt = execute_registry(registered, root)
    if output:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n")
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.write:
        registry, errors = build_registry()
        if errors or registry.get("blocked"):
            print(json.dumps({"errors": errors, "blocked": registry.get("blocked", [])}, ensure_ascii=False))
            return 1
        (ROOT / REGISTRY).write_text(json.dumps(registry, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps({"written": REGISTRY, "counts": registry["counts"]}))
        return 0
    receipt = validate(output=args.output)
    print(json.dumps({key: receipt[key] for key in ("kind", "status", "counts", "errors") if key in receipt}, ensure_ascii=False))
    return 0 if receipt["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
