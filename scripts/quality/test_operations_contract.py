#!/usr/bin/env python3
"""Check test prerequisites and maintenance inventory, never run the product."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
PLAN_PATH = "docs/development/first-product-test-plan.json"
OPERATIONS_PATH = "docs/development/first-product-test-operations.json"
OPERATIONS_ID = "FP1-TEST-OPS-20261019-R1"
OWNERS = {"AP", "HK", "PL", "AI", "JA", "UI", "QA"}
PREREQUISITE_IDS = {f"PRE-{n:02}" for n in range(1, 19)}
AREA_IDS = {f"MA-{n:02}" for n in range(1, 18)}
STATUSES = {"pending_review", "needs_confirmation", "prior_evidence_recheck",
            "team_preparation_pending", "not_configured"}
SELECTORS = {"case_ids", "variant_ids", "layer_ids", "module_ids", "feature_ids"}
# Reviewed source ownership for this revision. Documentation/artifacts may be
# associated with a case but cannot substitute its executable source area.
SOURCE_AREA_LAYERS = {
    "MA-01": {"L0"}, "MA-02": {"L1"}, "MA-03": {"L2"},
    "MA-04": {"L2"}, "MA-05": {"L3"}, "MA-06": {"L4"},
    "MA-07": {"L5"}, "MA-08": {"L5"}, "MA-09": {"L6"},
    "MA-10": {"L6", "L8"}, "MA-11": {"L7"}, "MA-12": {"L8"},
}
SOURCE_AREA_PATHS = {
    "MA-01": "scripts/quality/",
    "MA-02": "crates/<crate>/src/ 内 #[cfg(test)]",
    "MA-03": "crates/{hook-contracts,hook-client,lunahook-host,language-worker}/tests/",
    "MA-04": "tests/contracts/", "MA-05": "crates/app-infra/tests/",
    "MA-06": "tests/integration/application/",
    "MA-07": "apps/desktop/tests/components/",
    "MA-08": "apps/desktop/tests/visual/",
    "MA-09": "tests/integration/desktop/",
    "MA-10": "tests/integration/native/",
    "MA-11": "tests/integration/games/",
    "MA-12": "tests/integration/quality/",
}
TOP_FIELDS = {"schema_version", "id", "phase", "product_tests_executed",
              "prerequisites", "maintenance_areas", "case_maintenance", "rules"}
PREREQUISITE_FIELDS = {"id", "title", "provider", "owners", "status", "required_by_gates",
                       "deliverable", "user_action", "validation", "current_evidence", "applies_to"}
AREA_FIELDS = {"id", "title", "path", "state", "owners", "layer_ids", "responsibility",
               "trigger", "command_status"}
CASE_FIELDS = {"case_id", "area_ids", "module_owners", "implementation_status"}
RULE_FIELDS = {"applicability", "status_semantics", "secrets", "retention", "baseline_updates"}
SECRET_KEYS = {"apikey", "secret", "password", "privatekey", "accesstoken", "authorization",
               "clientsecret", "credentialvalue", "rawkey", "certificatepassword"}
SECRET_VALUE = re.compile(
    r"(?:\bsk-[A-Za-z0-9_-]{16,}|\bAKIA[A-Z0-9]{16}\b|"
    r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|"
    r"\bBearer\s+[A-Za-z0-9._~+/=-]{16,}|https?://[^\s/:@]+:[^\s/@]+@)"
)


def _matches(case: dict[str, Any], variant_id: str, rule: Any) -> bool:
    if not isinstance(rule, dict) or not rule or not set(rule) <= SELECTORS:
        return False
    actual = {"case_ids": {case.get("id")}, "variant_ids": {variant_id},
              "layer_ids": {case.get("layer")},
              "module_ids": set(case.get("module_ids", [])),
              "feature_ids": set(case.get("feature_ids", []))}
    return all(isinstance(values, list) and values and
               all(isinstance(value, str) for value in values) and
               bool(actual[key].intersection(values)) for key, values in rule.items())


def dependencies_for(case: dict[str, Any], variant_id: str, ops: dict[str, Any]) -> list[dict[str, Any]]:
    """Return applicable prerequisite records: OR rules, AND fields, ANY values."""
    rows = ops.get("prerequisites")
    if not isinstance(rows, list):
        return []
    return [row for row in rows if isinstance(row, dict)
            and isinstance(row.get("applies_to"), list)
            and any(_matches(case, variant_id, rule) for rule in row["applies_to"])]


def validate_operations(root: Path, data: dict[str, Any] | None = None) -> list[str]:
    root = Path(root).resolve()
    errors: list[str] = []

    def read(relative):
        path = root / relative
        for parent in [path, *path.parents]:
            if parent == root:
                break
            if parent.is_symlink():
                raise ValueError("symbolic links are not valid operations/source inputs: " + relative)
        return json.loads(path.read_text())

    try:
        plan = read(PLAN_PATH)
        data = read(OPERATIONS_PATH) if data is None else data
    except (OSError, ValueError, TypeError) as error:
        return [f"operations input: {error}"]
    if not isinstance(data, dict) or not isinstance(plan, dict):
        return ["operations and plan must be objects"]

    def shape(row, expected, label):
        if not isinstance(row, dict):
            errors.append(label + ": expected object")
            return False
        if set(row) != expected:
            errors.append(label + ": missing or unknown fields " + str(sorted(set(row) ^ expected)))
        return True

    def text(row, field, label):
        if not isinstance(row.get(field), str) or not row[field].strip():
            errors.append(label + "." + field + ": expected nonempty text")

    def refs(values, allowed, label):
        if not isinstance(values, list) or not values or not all(isinstance(v, str) for v in values):
            errors.append(label + ": expected nonempty reference list")
            return set()
        if len(values) != len(set(values)):
            errors.append(label + ": duplicate reference")
        unknown = set(values) - allowed
        if unknown:
            errors.append(label + ": unknown reference " + str(sorted(unknown)))
        return set(values)

    def inventory(key, expected_ids, identifier="id"):
        rows = data.get(key)
        if not isinstance(rows, list):
            errors.append(key + ": expected list")
            return []
        identifiers = [row.get(identifier) for row in rows if isinstance(row, dict)]
        string_ids = [value for value in identifiers if isinstance(value, str)]
        if len(string_ids) != len(set(string_ids)):
            errors.append(key + ": duplicate ID")
        if set(string_ids) != expected_ids or len(identifiers) != len(rows):
            errors.append(key + ": missing or unknown IDs")
        return rows

    def secrets(value, label="operations"):
        if isinstance(value, dict):
            for key, item in value.items():
                if re.sub(r"[^a-z]", "", key.lower()) in SECRET_KEYS:
                    errors.append(label + ": secret field is forbidden")
                secrets(item, label + "." + key)
        elif isinstance(value, list):
            for index, item in enumerate(value):
                secrets(item, f"{label}[{index}]")
        elif isinstance(value, str) and SECRET_VALUE.search(value):
            errors.append(label + ": possible raw secret value is forbidden")

    shape(data, TOP_FIELDS, "operations")
    secrets(data)
    if type(data.get("schema_version")) is not int or data["schema_version"] != 1:
        errors.append("schema_version: expected integer 1")
    if data.get("id") != OPERATIONS_ID:
        errors.append("id: unreviewed operations revision")
    if data.get("phase") != "planning" or data.get("product_tests_executed") is not False:
        errors.append("planning status: operations cannot claim product execution")

    cases = plan.get("cases", [])
    if not isinstance(cases, list) or any(not isinstance(case, dict) for case in cases):
        return errors + ["plan.cases: expected case objects"]
    case_map = {case.get("id"): case for case in cases if isinstance(case.get("id"), str)}
    if not case_map or len(case_map) != len(cases):
        errors.append("plan.cases: nonempty unique case IDs required")
    variant_ids = {row.get("id") for row in plan.get("variants", []) if isinstance(row, dict)
                   and isinstance(row.get("id"), str)}
    layer_ids = {row.get("id") for row in plan.get("layers", []) if isinstance(row, dict)
                 and isinstance(row.get("id"), str)}
    references = {"case_ids": set(case_map), "variant_ids": variant_ids, "layer_ids": layer_ids,
                  "module_ids": {v for c in cases for v in c.get("module_ids", [])},
                  "feature_ids": {v for c in cases for v in c.get("feature_ids", [])}}

    prerequisites = inventory("prerequisites", PREREQUISITE_IDS)
    for index, row in enumerate(prerequisites):
        label = f"prerequisites[{index}]"
        if not shape(row, PREREQUISITE_FIELDS, label):
            continue
        for field in ("title", "deliverable", "user_action", "validation", "current_evidence"):
            text(row, field, label)
        refs(row.get("owners"), OWNERS, label + ".owners")
        refs(row.get("required_by_gates"), {f"B{n}" for n in range(6)}, label + ".required_by_gates")
        if not isinstance(row.get("provider"), str) or row["provider"] not in {"user", "team", "shared"}:
            errors.append(label + ".provider: unknown provider")
        if not isinstance(row.get("status"), str) or row["status"] not in STATUSES:
            errors.append(label + ".status: unreviewed readiness or execution claim")
        rules = row.get("applies_to")
        if not isinstance(rules, list) or not rules:
            errors.append(label + ".applies_to: expected nonempty rules")
            continue
        for rule_index, rule in enumerate(rules):
            rule_label = f"{label}.applies_to[{rule_index}]"
            if not isinstance(rule, dict) or not rule or not set(rule) <= SELECTORS:
                errors.append(rule_label + ": empty or unknown selector rule")
                continue
            for field, values in rule.items():
                refs(values, references[field], rule_label + "." + field)

    areas = inventory("maintenance_areas", AREA_IDS)
    area_map = {row.get("id"): row for row in areas if isinstance(row, dict)
                and isinstance(row.get("id"), str)}
    for index, row in enumerate(areas):
        label = f"maintenance_areas[{index}]"
        if not shape(row, AREA_FIELDS, label):
            continue
        for field in ("title", "path", "responsibility", "trigger", "command_status"):
            text(row, field, label)
        refs(row.get("owners"), OWNERS, label + ".owners")
        actual_layers = refs(row.get("layer_ids"), layer_ids, label + ".layer_ids")
        if not isinstance(row.get("state"), str) or row["state"] not in {"existing", "mixed", "planned", "generated", "external"}:
            errors.append(label + ".state: unreviewed readiness or execution claim")
        identifier = row.get("id")
        if isinstance(identifier, str) and identifier in SOURCE_AREA_PATHS:
            if row.get("path") != SOURCE_AREA_PATHS[identifier]:
                errors.append(label + ".path: executable source maintenance path drift")
            if actual_layers != SOURCE_AREA_LAYERS[identifier]:
                errors.append(label + ".layer_ids: executable source layer drift")
            if row.get("state") in {"generated", "external"}:
                errors.append(label + ": generated or external area cannot replace client test source")

    maintenance = inventory("case_maintenance", set(case_map), "case_id")
    for index, row in enumerate(maintenance):
        label = f"case_maintenance[{index}]"
        if not shape(row, CASE_FIELDS, label):
            continue
        used_areas = refs(row.get("area_ids"), set(area_map), label + ".area_ids")
        case_id = row.get("case_id")
        case = case_map.get(case_id) if isinstance(case_id, str) else None
        if case is None:
            continue
        if row.get("module_owners") != case.get("owner"):
            errors.append(label + ".module_owners: must match the case Owner")
        if row.get("implementation_status") != "planned":
            errors.append(label + ".implementation_status: must remain planned")
        layer = case.get("layer")
        if not any(layer in SOURCE_AREA_LAYERS.get(area, set()) for area in used_areas):
            errors.append(label + ": missing executable source area for case layer")
        for area_id in used_areas & set(area_map):
            if layer not in area_map[area_id].get("layer_ids", []):
                errors.append(label + ": maintenance area does not cover case layer")

    if shape(data.get("rules"), RULE_FIELDS, "rules"):
        for field in RULE_FIELDS:
            text(data["rules"], field, "rules")
    covered_prerequisites = set()
    affected_executions = {identifier: set() for identifier in PREREQUISITE_IDS}
    display_executions, pool_executions = set(), set()
    for case in cases:
        for variant in case.get("variant_ids", []):
            execution = (case.get("id"), variant)
            if variant == "DISPLAYS":
                display_executions.add(execution)
            if case.get("id") == "MC-04":
                pool_executions.add(execution)
            applicable = dependencies_for(case, variant, data)
            if not applicable:
                errors.append(f"execution {case.get('id')}@{variant}: missing prerequisite mapping")
            covered_prerequisites.update(row.get("id") for row in applicable if isinstance(row.get("id"), str))
            for row in applicable:
                identifier = row.get("id")
                if isinstance(identifier, str) and identifier in affected_executions:
                    affected_executions[identifier].add(execution)
                if isinstance(identifier, str) and identifier in {"PRE-12", "PRE-17"} and variant in {"UNIT", "PROTOCOL"}:
                    errors.append(f"{identifier}: real resources cannot block controlled UNIT/PROTOCOL executions")
    # Equipment and multi-key credentials are optional for unrelated journeys.
    # These scoped obligations prevent broad readiness rules from overblocking
    # deterministic tests or requiring two keys for every real provider.
    if affected_executions["PRE-16"] != display_executions:
        errors.append("PRE-16: dual-display prerequisite must cover only DISPLAYS executions")
    if affected_executions["PRE-18"] != pool_executions:
        errors.append("PRE-18: multiple-key prerequisite must cover only MC-04 executions")
    unused = PREREQUISITE_IDS - covered_prerequisites
    if unused:
        errors.append("prerequisites: no affected execution for " + str(sorted(unused)))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    errors = validate_operations(args.root)
    receipt = {"check_scope": "static_test_operations_only", "product_tests_executed": False,
               "status": "failed" if errors else "passed", "errors": errors}
    if args.json:
        print(json.dumps(receipt, ensure_ascii=False, indent=2))
    else:
        print("\n".join(errors) if errors else "Test operations inventory valid; no product tests executed.")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
