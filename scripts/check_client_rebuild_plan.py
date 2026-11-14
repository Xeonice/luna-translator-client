#!/usr/bin/env python3
"""Validate the planning snapshot, task DAG and product regression traceability."""

from collections import Counter
import csv
import json
from pathlib import Path
import re
import sys

from check_project_plan import check as check_product

ROOT = Path(__file__).resolve().parents[1]


def read_csv(path):
    with path.open(newline="") as stream:
        rows = list(csv.DictReader(stream))
    if not rows or any(None in row or any(value is None for value in row.values()) for row in rows):
        raise ValueError(f"Empty or malformed CSV: {path.name}")
    return rows


def values(row, key):
    return set(filter(None, row[key].split(";")))


def check(root=ROOT):
    baseline = check_product(root)
    requirements = {row["requirement_id"]: row for row in read_csv(root / "docs/development/requirements.csv")}
    screens = set().union(*(values(row, "design_screens") for row in requirements.values()))
    plan = (root / "docs/development/plan.md").read_text()
    packages = set(re.findall(r"^\| ([A-Z]\d+) \|", plan, re.M))
    tasks = read_csv(root / "docs/development/client-rebuild-tasks.csv")
    cases = read_csv(root / "docs/development/client-regression.csv")
    extensions = read_csv(root / "docs/development/client-learning-extensions.csv")

    def unique(rows, key):
        ids = [row[key] for row in rows]
        if len(set(ids)) != len(ids) or "" in ids:
            raise ValueError(f"Duplicate/empty {key}")
        return set(ids)

    task_ids = unique(tasks, "task_id")
    case_ids = unique(cases, "case_id")
    extension_ids = unique(extensions, "extension_id")
    for row in extensions:
        if row["status"] != "planned" or row["evidence"]:
            raise ValueError(f"Extension is a planning entry, not a product result: {row['extension_id']}")
        if not values(row, "related_original_requirements") <= requirements.keys() or not values(row, "design_screens") <= screens:
            raise ValueError(f"Extension has unknown REQ/SCR: {row['extension_id']}")
        if any(not row[field].strip() for field in ["title", "kind", "related_original_requirements", "design_screens", "delivery_slice", "acceptance"]):
            raise ValueError(f"Extension missing scope or acceptance: {row['extension_id']}")
    groups = {row["group"] for row in cases}
    task_req, task_screens, task_ext = set(), set(), set()
    for row in tasks:
        key = row["task_id"]
        if row["status"] not in {"ready", "planned"} or row["evidence"]:
            raise ValueError(f"Planning task cannot claim implementation evidence: {key}")
        required = ["milestone", "work_packages", "title", "owner_modules", "requirements", "design_screens", "reuse_basis", "acceptance", "regression_groups"]
        if any(not row[field].strip() for field in required):
            raise ValueError(f"Task missing execution information: {key}")
        if not values(row, "work_packages") <= packages:
            raise ValueError(f"Task has unknown work package: {key}")
        if not values(row, "depends_on") <= task_ids or key in values(row, "depends_on"):
            raise ValueError(f"Task has unknown/self dependency: {key}")
        if not values(row, "regression_groups") <= groups:
            raise ValueError(f"Task has unknown regression group: {key}")
        if not values(row, "extension_ids") <= extension_ids:
            raise ValueError(f"Task has unknown extension: {key}")
        task_ext |= values(row, "extension_ids")
        if not values(row, "requirements") <= requirements.keys() or not values(row, "design_screens") <= screens:
            raise ValueError(f"Task has unknown REQ/SCR: {key}")
        task_req |= values(row, "requirements")
        task_screens |= values(row, "design_screens")
    if task_req != requirements.keys() or task_screens != screens:
        raise ValueError("Task plan does not cover the complete inherited product baseline")

    graph = {row["task_id"]: values(row, "depends_on") for row in tasks}
    visiting, visited = set(), set()

    def visit(node):
        if node in visiting:
            raise ValueError(f"Task dependency cycle: {node}")
        if node in visited:
            return
        visiting.add(node)
        for dependency in graph[node]:
            visit(dependency)
        visiting.remove(node)
        visited.add(node)

    for task in graph:
        visit(task)

    def ancestors(node):
        return graph[node] | set().union(*(ancestors(parent) for parent in graph[node])) if graph[node] else set()

    for row in tasks:
        if row["milestone"] == "M2-L" and row["task_id"] != "M2-L01":
            if not {"M2-L01", "M2-12"} <= ancestors(row["task_id"]):
                raise ValueError(f"Learning implementation needs design review and M2 baseline: {row['task_id']}")
    p0 = {key for key, row in requirements.items() if row["priority"] == "P0"}
    p0_gate = next(row for row in tasks if row["task_id"] == "M3-08")
    if values(p0_gate, "requirements") != p0:
        raise ValueError("Full P0 release gate must retain exactly the inherited P0 requirements")

    case_req, case_screens, case_ext = set(), set(), set()
    for row in cases:
        key = row["case_id"]
        if row["status"] != "not_run" or row["evidence"] or row["blocker"]:
            raise ValueError(f"Planning matrix must remain separate from future run results: {key}")
        required = ["group", "title", "milestone", "requirement_ids", "design_screens", "design_acceptance", "priority", "execution_environment", "method", "preconditions", "steps", "expected_result"]
        if any(not row[field].strip() for field in required):
            raise ValueError(f"Case missing executable information: {key}")
        if not values(row, "requirement_ids") <= requirements.keys() or not values(row, "design_screens") <= screens:
            raise ValueError(f"Case has unknown REQ/SCR: {key}")
        if not values(row, "extension_ids") <= extension_ids:
            raise ValueError(f"Case has unknown extension: {key}")
        case_ext |= values(row, "extension_ids")
        if values(row, "design_acceptance") != {screen.replace("SCR-", "DES-") for screen in values(row, "design_screens")}:
            raise ValueError(f"Case DES mapping differs from SCR mapping: {key}")
        case_req |= values(row, "requirement_ids")
        case_screens |= values(row, "design_screens")
    if case_req != requirements.keys() or case_screens != screens:
        raise ValueError("Regression matrix does not cover the complete inherited product baseline")
    if task_ext != extension_ids or case_ext != extension_ids:
        raise ValueError("Learning/AI extensions need both implementation tasks and regression coverage")
    for key, requirement in requirements.items():
        case = next((row for row in cases if row["case_id"] == "CR-11-" + key), None)
        if not case or values(case, "requirement_ids") != {key}:
            raise ValueError(f"Missing full requirement regression: {key}")
        if case["priority"] != requirement["priority"] or values(case, "design_screens") != values(requirement, "design_screens"):
            raise ValueError(f"Changed full requirement priority/screen coverage: {key}")
    for game in ["SR", "MO", "N7", "SAKURA"]:
        if not {"CR-03-" + game, "CR-05-" + game} <= case_ids:
            raise ValueError(f"Missing independent original-text or translation case: {game}")
    if any("EVER17" in key.upper() for key in case_ids):
        raise ValueError("Ever17 is excluded from this regression scope")
    return {**baseline, "tasks": len(tasks), "task_dag": "acyclic",
            "regression_cases": len(cases), "cases_by_milestone": dict(Counter(row["milestone"] for row in cases)),
            "planning_cases_not_run": len(cases), "full_requirement_regressions": len(requirements),
            "supplemental_entries": len(extensions)}


if __name__ == "__main__":
    try:
        print(json.dumps(check(), ensure_ascii=False, indent=2))
    except (OSError, ValueError, KeyError, StopIteration) as error:
        print(f"Client rebuild plan validation failed: {error}", file=sys.stderr)
        sys.exit(1)
