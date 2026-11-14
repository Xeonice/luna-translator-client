#!/usr/bin/env python3
"""Verify the inherited product snapshot and complete mapping into the new plan."""

import csv
import hashlib
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]


def original_requirements(prd):
    expected = {}
    screen_map = {}
    for line in prd.splitlines():
        found = re.match(r"^- \*\*(REQ-\d{3}) (.*?)\*\* — (P[012])；", line)
        if found:
            key, title, priority = found.groups()
            if key in expected:
                raise ValueError(f"Duplicate requirement in original PRD: {key}")
            expected[key] = (title, priority)
        # The PRD explicitly defines this table as the complete screen mapping;
        # individual requirement paragraphs list only the main entry points.
        columns = [column.strip() for column in line.split("|")]
        if len(columns) == 6 and re.fullmatch(r"SCR-\d{2}", columns[1]):
            for number in re.findall(r"\b\d{3}\b", columns[4]):
                screen_map.setdefault(f"REQ-{number}", set()).add(columns[1])
    if len(expected) != 56 or set(screen_map) != set(expected):
        raise ValueError("Expected the complete imported 56-requirement and screen baseline")
    return expected, screen_map


def check(root=ROOT):
    product = root / "docs/product"
    manifest = json.loads((product / "source-manifest.json").read_text())
    for record in manifest["files"]:
        path = product / record["localPath"]
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Missing/nonregular original product file: {path}")
        data = path.read_bytes()
        if len(data) != record["bytes"] or hashlib.sha256(data).hexdigest() != record["sha256"]:
            raise ValueError(f"Original product snapshot changed: {path}")

    prd = (product / "original/docs/01-prd.md").read_text()
    expected, screen_map = original_requirements(prd)

    plan = (root / "docs/development/plan.md").read_text()
    packages = set(re.findall(r"^\| ([A-Z]\d+) \|", plan, flags=re.MULTILINE))
    design = json.loads((product / "original/design/manifest.json").read_text())
    screens = {screen["id"] for screen in design["screens"]}
    seen = set()
    mapped_screens = set()
    with (root / "docs/development/requirements.csv").open(newline="") as stream:
        for row in csv.DictReader(stream):
            key = row["requirement_id"]
            if key in seen or key not in expected:
                raise ValueError(f"Duplicate or foreign requirement mapping: {key}")
            seen.add(key)
            if (row["title"], row["priority"]) != expected[key]:
                raise ValueError(f"Requirement title/priority changed: {key}")
            if row["work_package"] not in packages:
                raise ValueError(f"Missing/unknown primary work package in {key}")
            assigned = [row["work_package"]] + row["related_work_packages"].split(";")
            if any(package and package not in packages for package in assigned):
                raise ValueError(f"Unknown work package in {key}: {assigned}")
            assigned_screens = set(row["design_screens"].split(";"))
            if assigned_screens != screen_map[key]:
                raise ValueError(f"Incomplete/changed original screen mapping in {key}")
            if not assigned_screens <= screens:
                raise ValueError(f"Unknown design screen in {key}")
            mapped_screens.update(assigned_screens)
            if row["status"] not in {"planned", "in_progress", "verified"}:
                raise ValueError(f"Unknown requirement status in {key}")
            if row["status"] == "verified" and not row["evidence"].strip():
                raise ValueError(f"Verified requirement needs evidence: {key}")
    if seen != set(expected):
        raise ValueError(f"Unmapped requirements: {sorted(set(expected) - seen)}")
    if mapped_screens != screens:
        raise ValueError(f"Unmapped design screens: {sorted(screens - mapped_screens)}")
    return {"original_files_verified": len(manifest["files"]), "requirements_mapped": len(seen),
            "design_screens": len(screens), "work_packages": len(packages)}


if __name__ == "__main__":
    try:
        print(json.dumps(check()))
    except (OSError, ValueError, KeyError) as error:
        print(f"Project plan validation failed: {error}", file=sys.stderr)
        sys.exit(1)

