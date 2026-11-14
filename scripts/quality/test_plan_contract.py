#!/usr/bin/env python3
"""Validate a first-product test *plan* without asserting product execution."""

from __future__ import annotations

import argparse
from collections import Counter
import csv
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
PLAN_PATH = "docs/development/first-product-test-plan.json"
PLAN_ID = "FP1-TEST-20261018-R1"
SOURCE_FILES = {
    "docs/development/client-regression.csv": ("case_id", {"M1", "M2", "M2-L"}),
    "docs/product/model-configuration-acceptance.csv": ("acceptance_id", None),
    "docs/product/game-library-window-acceptance.csv": ("case_id", None),
}
LAYER_IDS = {f"L{index}" for index in range(9)}
GATE_IDS = {f"B{index}" for index in range(6)}
FEATURE_IDS = {f"FP-{index:02}" for index in range(1, 13)}
AI_IDS = {f"AI-CFG-{index:02}" for index in range(1, 17)}
OWNERS = {"AP", "HK", "PL", "AI", "JA", "UI", "QA"}
IDENTIFIER = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*\Z")
VARIANT_IDENTIFIER = re.compile(r"[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)*\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
# These are reviewed planning contracts, not expected runtime results. Changing
# their scope or prerequisites requires a new reviewed plan/schema revision.
SUPPORTED_LANES = ["macos-arm64-crossover"]
SUPPORTED_SCOPE = "macOS arm64 + CrossOver Windows游戏；Rust PE32/64 broker均必需；不含Ever17"
REQUIRED_LAYER_EDGES = {
    "L0": set(), "L1": {"L0"}, "L2": {"L0"}, "L3": {"L1"},
    "L4": {"L1", "L2", "L3"}, "L5": {"L0"}, "L6": {"L4", "L5"},
    "L7": {"L6"}, "L8": {"L6"},
}
ENVIRONMENT_KINDS = {
    "ENV-BASELINE": "governance", "ENV-RUST": "controlled", "ENV-PROTOCOL": "controlled",
    "ENV-CORE32": "real_core", "ENV-CORE64": "real_core", "ENV-INFRA": "real_io",
    "ENV-WEBVIEW": "component", "ENV-MAC": "real_native", "ENV-DISPLAYS": "real_native",
    "ENV-REMOTE-COMPAT": "real_provider", "ENV-ANTHROPIC": "real_provider",
    "ENV-AZURE": "real_provider", "ENV-LOCAL": "real_provider", "ENV-SR": "real_game",
    "ENV-MO": "real_game", "ENV-N7": "real_game", "ENV-SAKURA": "real_game",
    "ENV-CLEAN-MAC": "distribution",
}
VARIANT_CONTRACTS = {
    "BASELINE": ({"ENV-BASELINE"}, "source_build"),
    "UNIT": ({"ENV-RUST"}, "source_build"),
    "PROTOCOL": ({"ENV-PROTOCOL"}, "source_build"),
    "CORE32": ({"ENV-CORE32"}, "source_build"),
    "CORE64": ({"ENV-CORE64"}, "source_build"),
    "INFRA": ({"ENV-INFRA", "ENV-PROTOCOL"}, "source_build"),
    "UI": ({"ENV-WEBVIEW"}, "source_build"),
    "NATIVE": ({"ENV-MAC"}, "final_package"),
    "DISPLAYS": ({"ENV-MAC", "ENV-DISPLAYS"}, "final_package"),
    "REMOTE-COMPAT": ({"ENV-MAC", "ENV-REMOTE-COMPAT"}, "final_package"),
    "ANTHROPIC": ({"ENV-MAC", "ENV-ANTHROPIC"}, "final_package"),
    "TRADITIONAL": ({"ENV-MAC", "ENV-AZURE"}, "final_package"),
    "LOCAL-COMPAT": ({"ENV-MAC", "ENV-LOCAL"}, "final_package"),
    "GAME-SR": ({"ENV-MAC", "ENV-SR"}, "final_package"),
    "GAME-MO": ({"ENV-MAC", "ENV-MO"}, "final_package"),
    "GAME-N7": ({"ENV-MAC", "ENV-N7"}, "final_package"),
    "GAME-SAKURA": ({"ENV-MAC", "ENV-SAKURA"}, "final_package"),
    "DISTRIBUTION": ({"ENV-CLEAN-MAC"}, "final_package"),
}
VISUAL_OVERRIDE = (
    "本首版使用 docs/design/first-product-v1 当前最终批准版本；替代旧CSV的aligned-v1/ocr-selection-v2与固定500×132断言。"
    "舒适默认560/18/24，紧凑500/16/22，控制行32，按长文自适应高度；设计未最终批准不能放行。"
)
VISUAL_EXPECTATIONS = [
    VISUAL_OVERRIDE,
    "当前稿所有适用状态、主题、长文/200%、最小窗与真实原生材质按预定mask/阈值比较；候选、基准、diff与人工结论齐全。",
]
METRIC_POLICIES = {
    "LOCAL-FEEDBACK": {
        "id": "LOCAL-FEEDBACK", "source": "原PRD本地关键反馈≤200ms",
        "status": "measurement_method_pending_B0",
        "rule": "本地暂停/取消/切换等反馈，不含云端完成时间；负载、样本量、统计口径先冻结",
    },
    "PERFORMANCE": {
        "id": "PERFORMANCE", "status": "approval_required_before_B0_exit",
        "rule": "冻结冷启/原文/词法/查词p95与最大值、内存/队列/CPU预算及恢复时间；禁止看结果后调线",
    },
    "LANGUAGE-QUALITY": {
        "id": "LANGUAGE-QUALITY", "status": "approval_required_before_B0_exit",
        "rule": "授权/自制分层语料，人工日语复核，严重语义/上下文串台/泄密阻断；样本量、分级与可接受比例先批准",
    },
}
RELEASE_POLICY = {
    "required_case_status": "passed_for_every_frozen_execution_id",
    "current_status": "blocked_not_executed",
    "design_approval": "full_layout_approved_required_not_granted",
    "source_build_vs_final_package": "低层测试绑定同源构建；正式包旅程全部绑定同一最终RC及实际bundle，不混包",
    "product_adapter_status": "not_implemented",
    "evidence_validation": "Plan lint and evidence hashes prove inventory/identity only; real runtime/capture adapters and human review remain required",
}
MECHANISM_CONTRACTS = {
    "TP-01": ("L3", set()),
    "TP-02": ("L3", {"AI-CFG-11", "AI-CFG-15"}),
    "TP-03": ("L3", set()),
    "TP-04": ("L3", {"AI-CFG-04", "AI-CFG-13", "AI-CFG-15"}),
    "TP-05": ("L3", {"AI-CFG-04", "AI-CFG-11", "AI-CFG-13"}),
    "TP-06": ("L4", set()),
    "TP-07": ("L4", set()),
    "TP-08": ("L4", {"AI-CFG-04", "AI-CFG-10", "AI-CFG-13"}),
    "TP-09": ("L3", {"AI-CFG-10", "AI-CFG-15"}),
    "TP-10": ("L1", {"AI-CFG-13"}),
    "TP-11": ("L1", {"AI-CFG-06", "AI-CFG-08", "AI-CFG-10"}),
    "TP-12": ("L2", set()),
    "TP-13": ("L3", {"AI-CFG-01", "AI-CFG-05", "AI-CFG-06", "AI-CFG-12"}),
    "TP-14": ("L4", {"AI-CFG-04", "AI-CFG-12", "AI-CFG-13"}),
    "TP-15": ("L3", {"AI-CFG-01", "AI-CFG-02", "AI-CFG-03", "AI-CFG-05", "AI-CFG-07", "AI-CFG-09", "AI-CFG-14"}),
    "TP-16": ("L1", {"AI-CFG-03", "AI-CFG-07", "AI-CFG-09", "AI-CFG-13"}),
    "TP-17": ("L2", set()),
    "TP-18": ("L2", set()),
    "TP-19": ("L8", set()),
    "TP-20": ("L8", {"AI-CFG-05", "AI-CFG-06", "AI-CFG-07", "AI-CFG-16"}),
    "TP-21": ("L6", {"AI-CFG-04", "AI-CFG-11"}),
    "TP-22": ("L5", {"AI-CFG-06", "AI-CFG-16"}),
    "TP-23": ("L8", {"AI-CFG-01", "AI-CFG-05", "AI-CFG-16"}),
    "TP-24": ("L8", {"AI-CFG-04", "AI-CFG-11", "AI-CFG-15"}),
    "TP-25": ("L3", {"AI-CFG-13"}),
    "TP-26": ("L3", set()),
    "TP-27": ("L1", set()),
    "TP-28": ("L5", set()),
    "TP-29": ("L8", set()),
    "TP-30": ("L8", set()),
    "TP-31": ("L0", set()),
}


def _text(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip()) and value == value.strip()


def _path(root: Path, value: Any, label: str, errors: list[str]) -> Path | None:
    if not _text(value) or "\\" in value:
        errors.append(f"{label}: expected a repository-relative POSIX path")
        return None
    relative = PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts or relative.as_posix() != value:
        errors.append(f"{label}: path escapes or is not normalized")
        return None
    candidate = root.joinpath(*relative.parts)
    try:
        candidate.resolve().relative_to(root.resolve())
        # Check only components within root; its filesystem may itself be aliased.
        current = root
        for part in relative.parts:
            current /= part
            if current.is_symlink():
                errors.append(f"{label}: symbolic links are not evidence/source paths")
                return None
    except (OSError, ValueError):
        errors.append(f"{label}: path escapes repository")
        return None
    return candidate


def _strings(value: Any, label: str, errors: list[str], *, nonempty: bool = True) -> list[str]:
    if not isinstance(value, list) or (nonempty and not value):
        errors.append(f"{label}: expected {'a nonempty' if nonempty else 'an'} array of strings")
        return []
    valid = [item for item in value if _text(item)]
    if len(valid) != len(value):
        errors.append(f"{label}: entries must be nonempty trimmed strings")
    if len(set(valid)) != len(valid):
        errors.append(f"{label}: duplicate entries")
    return valid


def _rows(value: Any, label: str, errors: list[str]) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not value:
        errors.append(f"{label}: expected a nonempty object array")
        return []
    result = []
    for index, row in enumerate(value):
        if not isinstance(row, dict):
            errors.append(f"{label}[{index}]: expected an object")
        else:
            result.append(row)
    return result


def _identities(rows: list[dict[str, Any]], label: str, errors: list[str], pattern=IDENTIFIER) -> set[str]:
    identities = []
    for index, row in enumerate(rows):
        value = row.get("id")
        if not isinstance(value, str) or pattern.fullmatch(value) is None:
            errors.append(f"{label}[{index}].id: invalid identifier")
        else:
            identities.append(value)
    for identifier, count in Counter(identities).items():
        if count > 1:
            errors.append(f"{label}: duplicate ID ({identifier})")
    return set(identities)


def _reference_list(row: dict[str, Any], field: str, allowed: set[str], label: str,
                    errors: list[str], *, nonempty: bool = True) -> list[str]:
    values = _strings(row.get(field), f"{label}.{field}", errors, nonempty=nonempty)
    for value in values:
        if value not in allowed:
            errors.append(f"{label}.{field}: unknown reference ({value})")
    return values


def _csv(root: Path, relative: str, key: str, errors: list[str]) -> list[dict[str, str]]:
    path = _path(root, relative, relative, errors)
    if path is None:
        return []
    try:
        with path.open(encoding="utf-8", newline="") as stream:
            reader = csv.DictReader(stream)
            if key not in (reader.fieldnames or []):
                errors.append(f"{relative}: missing CSV column ({key})")
                return []
            rows = list(reader)
        identifiers = [row.get(key) for row in rows]
        if any(not _text(value) for value in identifiers) or len(set(identifiers)) != len(identifiers):
            errors.append(f"{relative}: empty or duplicate authoritative IDs")
        return rows
    except (OSError, UnicodeError, csv.Error) as error:
        errors.append(f"{relative}: cannot read authoritative CSV ({error})")
        return []


def required_executions(plan: dict[str, Any]) -> set[str]:
    """Expand required planned combinations; this is not an execution receipt.

    Call validate_plan before using this inventory for a release decision. This
    helper deliberately reads no filesystem and produces no passed result.
    """
    return {
        f"{case['id']}@{variant}"
        for case in plan.get("cases", [])
        if case.get("release_required") is True
        for variant in case.get("variant_ids", [])
    }


def validate_plan(root: Path | str, data: Any = None) -> list[str]:
    """Return static plan errors; an empty list never means the product passed."""
    root = Path(root).resolve()
    errors: list[str] = []
    if data is None:
        path = _path(root, PLAN_PATH, "plan", errors)
        if path is None:
            return errors
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            return [f"plan: cannot read JSON ({error})"]
    if not isinstance(data, dict):
        return ["plan: expected a JSON object"]
    if type(data.get("schema_version")) is not int or data["schema_version"] != 1:
        errors.append("schema_version: expected integer 1")
    for key, expected in [("plan_id", PLAN_ID), ("phase", "planning"), ("product_execution", "NOT_RUN")]:
        if data.get(key) != expected:
            errors.append(f"{key}: expected {expected}; a plan cannot claim product execution")
    if data.get("scope") != SUPPORTED_SCOPE or data.get("supported_lanes") != SUPPORTED_LANES:
        errors.append("scope/supported_lanes: must preserve the reviewed macos-arm64-crossover-only release scope")
    if data.get("release_policy") != RELEASE_POLICY:
        errors.append("release_policy: must preserve blocked/not-implemented/not-granted planning safeguards; no approval or execution claims")
    metrics = _rows(data.get("metric_policies"), "metric_policies", errors)
    if _identities(metrics, "metric_policies", errors) != set(METRIC_POLICIES):
        errors.append("metric_policies: all three reviewed metric policies are required")
    for index, row in enumerate(metrics):
        identifier = row.get("id")
        if not isinstance(identifier, str) or row != METRIC_POLICIES.get(identifier):
            errors.append(f"metric_policies[{index}]: preserve the reviewed pending status/rule; no unapproved numeric thresholds")

    layers = _rows(data.get("layers"), "layers", errors)
    layer_ids = _identities(layers, "layers", errors)
    if layer_ids != LAYER_IDS:
        errors.append("layers: must contain exactly L0 through L8")
    graph: dict[str, list[str]] = {}
    for index, row in enumerate(layers):
        label = f"layers[{index}]"
        if not _text(row.get("title")):
            errors.append(f"{label}.title: required nonempty text")
        dependencies = _reference_list(row, "depends_on", layer_ids, label, errors, nonempty=False)
        identifier = row.get("id")
        if isinstance(identifier, str):
            graph[identifier] = dependencies
            if not REQUIRED_LAYER_EDGES.get(identifier, set()).issubset(dependencies):
                errors.append(f"{label}.depends_on: missing mandatory layer prerequisite")
        for key in ("entry", "exit"):
            value = row.get(key)
            if not _text(value):
                _strings(value, f"{label}.{key}", errors)
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(identifier: str) -> None:
        if identifier in visiting:
            errors.append(f"layers: dependency cycle at {identifier}")
            return
        if identifier in visited:
            return
        visiting.add(identifier)
        for dependency in graph.get(identifier, []):
            visit(dependency)
        visiting.remove(identifier)
        visited.add(identifier)

    for identifier in graph:
        visit(identifier)

    environments = _rows(data.get("environments"), "environments", errors)
    environment_ids = _identities(environments, "environments", errors)
    if environment_ids != set(ENVIRONMENT_KINDS):
        errors.append("environments: must retain the reviewed controlled/real environment identities")
    for index, row in enumerate(environments):
        for key in ("kind", "description"):
            if not _text(row.get(key)):
                errors.append(f"environments[{index}].{key}: required nonempty text")
        if "ever17" in str(row.get("id", "")).lower():
            errors.append(f"environments[{index}]: Ever17 is excluded from this first-product plan")
        identifier = row.get("id")
        if isinstance(identifier, str) and row.get("kind") != ENVIRONMENT_KINDS.get(identifier):
            errors.append(f"environments[{index}].kind: cannot substitute controlled and real runtime environments")
    variants = _rows(data.get("variants"), "variants", errors)
    variant_ids = _identities(variants, "variants", errors, VARIANT_IDENTIFIER)
    if variant_ids != set(VARIANT_CONTRACTS):
        errors.append("variants: must retain the reviewed eighteen execution variants")
    variant_scopes: dict[str, str] = {}
    for index, row in enumerate(variants):
        label = f"variants[{index}]"
        environments_for_variant = _reference_list(row, "environment_ids", environment_ids, label, errors)
        if not isinstance(row.get("evidence_scope"), str) or row["evidence_scope"] not in {"source_build", "final_package"}:
            errors.append(f"{label}.evidence_scope: expected source_build or final_package")
        identifier = row.get("id")
        if isinstance(identifier, str):
            expected_variant = VARIANT_CONTRACTS.get(identifier)
            if expected_variant is None or (set(environments_for_variant), row.get("evidence_scope")) != expected_variant:
                errors.append(f"{label}: environment/evidence_scope cannot replace the reviewed variant contract")
            if isinstance(row.get("evidence_scope"), str):
                variant_scopes[identifier] = row["evidence_scope"]
        if "ever17" in str(row.get("id", "")).lower():
            errors.append(f"{label}: Ever17 is excluded from this first-product plan")

    authoritative: dict[str, set[str]] = {}
    authoritative_rows: dict[str, dict[str, dict[str, str]]] = {}
    for relative, (key, milestones) in SOURCE_FILES.items():
        rows = _csv(root, relative, key, errors)
        if milestones is not None:
            rows = [row for row in rows if row.get("milestone") in milestones]
        authoritative[relative] = {row[key] for row in rows if _text(row.get(key))}
        authoritative_rows[relative] = {row[key]: row for row in rows if _text(row.get(key))}
    counts = [len(authoritative[path]) for path in SOURCE_FILES]
    if counts != [55, 22, 20]:
        errors.append(f"sources: expected the preserved 55 CR + 22 MC + 20 GL first-product IDs, got {counts}")
    source_rows = _rows(data.get("sources"), "sources", errors)
    source_paths: list[str] = []
    for index, row in enumerate(source_rows):
        label = f"sources[{index}]"
        relative = row.get("path")
        path = _path(root, relative, f"{label}.path", errors)
        if not isinstance(relative, str) or relative not in authoritative:
            errors.append(f"{label}.path: not one of the three authoritative acceptance CSVs")
            continue
        source_paths.append(relative)
        values = _strings(row.get("case_ids"), f"{label}.case_ids", errors)
        if set(values) != authoritative[relative]:
            errors.append(f"{label}.case_ids: must exactly match eligible authoritative IDs")
        fingerprint = row.get("sha256")
        if not isinstance(fingerprint, str) or SHA256.fullmatch(fingerprint) is None:
            errors.append(f"{label}.sha256: expected lowercase SHA-256")
        elif path is not None:
            try:
                if hashlib.sha256(path.read_bytes()).hexdigest() != fingerprint:
                    errors.append(f"{label}.sha256: source fingerprint mismatch")
            except OSError as error:
                errors.append(f"{label}: cannot hash source ({error})")
    if set(source_paths) != set(SOURCE_FILES) or len(source_paths) != len(SOURCE_FILES):
        errors.append("sources: must declare each of the three authoritative CSVs exactly once")

    modules = {row["module_id"] for row in _csv(root, "docs/architecture/engineering-modules.csv", "module_id", errors)}
    components = {row["component_id"] for row in _csv(root, "docs/architecture/engineering-components.csv", "component_id", errors)}
    cases = _rows(data.get("cases"), "cases", errors)
    _identities(cases, "cases", errors)
    feature_coverage: set[str] = set()
    ai_coverage: set[str] = set()
    case_layers: set[str] = set()
    mechanism_ids: set[str] = set()
    imports: Counter[tuple[str, str]] = Counter()
    all_source_ids = set().union(*authoritative.values())
    for index, case in enumerate(cases):
        label = f"cases[{index}] ({case.get('id', '?')})"
        for key in ("title", "method"):
            if not _text(case.get(key)):
                errors.append(f"{label}.{key}: required nonempty text")
        if not isinstance(case.get("layer"), str) or case["layer"] not in LAYER_IDS:
            errors.append(f"{label}.layer: unknown layer")
        else:
            case_layers.add(case["layer"])
        if not isinstance(case.get("gate"), str) or case["gate"] not in GATE_IDS:
            errors.append(f"{label}.gate: expected B0 through B5")
        if not isinstance(case.get("owner"), str) or case["owner"] not in OWNERS:
            errors.append(f"{label}.owner: unknown architecture owner")
        feature_coverage.update(_reference_list(case, "feature_ids", FEATURE_IDS, label, errors))
        case_ai_ids = _reference_list(case, "ai_config_ids", AI_IDS, label, errors, nonempty=False)
        ai_coverage.update(case_ai_ids)
        _reference_list(case, "module_ids", modules, label, errors)
        _reference_list(case, "component_ids", components, label, errors)
        case_variants = _reference_list(case, "variant_ids", variant_ids, label, errors)
        if case.get("layer") in ("L6", "L7", "L8") and not any(variant_scopes.get(value) == "final_package" for value in case_variants):
            errors.append(f"{label}.variant_ids: L6-L8 require an actual final_package variant")
        for key in ("preconditions", "steps", "expected", "evidence_types"):
            _strings(case.get(key), f"{label}.{key}", errors)
        for key, expected in [("status", "not_run"), ("harness_status", "planned")]:
            if case.get(key) != expected:
                errors.append(f"{label}.{key}: a planning record must remain {expected}")
        if case.get("release_required") is not True:
            errors.append(f"{label}.release_required: first-product plan cases must be required")
        identifier = case.get("id")
        if "ever17" in f"{identifier} {case.get('title', '')}".lower():
            errors.append(f"{label}: Ever17 is excluded from this first-product plan")
        if "source" not in case:
            errors.append(f"{label}.source: explicit source object or null is required")
        source = case.get("source")
        if source is None:
            if isinstance(identifier, str) and (identifier in all_source_ids or identifier.startswith(("CR-", "MC-", "GL-"))):
                errors.append(f"{label}.source: inherited/legacy IDs cannot be declared as new cases")
            if isinstance(identifier, str):
                mechanism_ids.add(identifier)
                mechanism = MECHANISM_CONTRACTS.get(identifier)
                if mechanism is None or (case.get("layer"), set(case_ai_ids)) != mechanism:
                    errors.append(f"{label}: mechanism ID/layer/AI traceability must match the reviewed first-product plan")
        elif not isinstance(source, dict):
            errors.append(f"{label}.source: expected path/id object or null")
        else:
            relative, source_id = source.get("path"), source.get("id")
            if not isinstance(relative, str) or relative not in authoritative or not isinstance(source_id, str) or source_id not in authoritative.get(relative, set()):
                errors.append(f"{label}.source: unknown or ineligible authoritative source ID (M3 is not first-product scope)")
            else:
                imports[(relative, source_id)] += 1
                if identifier != source_id:
                    errors.append(f"{label}.id: imported cases must preserve their authoritative source ID")
                original = authoritative_rows[relative][source_id]
                if relative == "docs/development/client-regression.csv":
                    if not isinstance(case.get("preconditions"), list) or not case["preconditions"] or case["preconditions"][0] != original.get("preconditions"):
                        errors.append(f"{label}.preconditions: must retain the authoritative CR precondition as the first item")
                    if case.get("steps") != [original.get("steps")]:
                        errors.append(f"{label}.steps: cannot rewrite inherited CR acceptance steps")
                    expected = VISUAL_EXPECTATIONS if source_id == "CR-10-VISUAL" else [original.get("expected_result")]
                    if case.get("expected") != expected:
                        errors.append(f"{label}.expected: cannot rewrite inherited CR acceptance expectations")
                elif relative == "docs/product/model-configuration-acceptance.csv":
                    if case.get("steps") != [original.get("steps")] or case.get("expected") != [original.get("expected_result")]:
                        errors.append(f"{label}: must retain authoritative MC steps and expected result exactly")
                elif case.get("expected") != [original.get("expected")]:
                    errors.append(f"{label}.expected: must retain authoritative GL acceptance exactly")
                expected_ai_ids = set(original.get("related_requirements", "").split(";")) if relative == "docs/product/model-configuration-acceptance.csv" else set()
                if set(case_ai_ids) != expected_ai_ids:
                    errors.append(f"{label}.ai_config_ids: preserve the source-specific AI traceability; unrelated cases cannot absorb coverage")
        game_variants = None
        for prefix in ("CR-03-", "CR-05-", "CR-13-FINAL-"):
            suffix = identifier[len(prefix):] if isinstance(identifier, str) and identifier.startswith(prefix) else ""
            if suffix in {"SR", "MO", "N7", "SAKURA"}:
                game_variants = {f"GAME-{suffix}"}
        if identifier == "CR-07-SOAK":
            game_variants = {"GAME-SR", "GAME-MO", "GAME-N7", "GAME-SAKURA"}
        if game_variants is not None and (case.get("layer") != "L7" or set(case_variants) != game_variants):
            errors.append(f"{label}: real-game acceptance must remain L7 with its exact game variants; UNIT is not a substitute")
        if identifier == "CR-10-VISUAL":
            override = case.get("acceptance_override")
            if override != VISUAL_OVERRIDE:
                errors.append(f"{label}.acceptance_override: explicitly replace old visual expectations with current docs/design/first-product-v1")
        elif "acceptance_override" in case:
            errors.append(f"{label}.acceptance_override: only the reviewed CR-10-VISUAL override is allowed")
    for relative, identifiers in authoritative.items():
        for identifier in sorted(identifiers):
            if imports[(relative, identifier)] != 1:
                errors.append(f"cases: authoritative case must be imported exactly once ({identifier})")
    if mechanism_ids != set(MECHANISM_CONTRACTS):
        errors.append("cases: must retain all reviewed TP-01 through TP-31 mechanism cases")
    if case_layers != LAYER_IDS:
        errors.append("cases: every layer L0 through L8 needs its own planned cases")
    for field, missing in [("feature_ids", FEATURE_IDS - feature_coverage), ("ai_config_ids", AI_IDS - ai_coverage)]:
        if missing:
            errors.append(f"cases.{field}: missing coverage ({', '.join(sorted(missing))})")
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    arguments = parser.parse_args(argv)
    errors = validate_plan(arguments.root)
    result = {"status": "failed" if errors else "passed", "check_scope": "static_test_plan_only",
              "product_tests_executed": False, "errors": errors}
    if arguments.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print("\n".join(errors) if errors else "Test plan contract: passed (plan only; product tests NOT_RUN)")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
