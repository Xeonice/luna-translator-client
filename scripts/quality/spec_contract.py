"""Offline OpenSpec/verification-sidecar and release-evidence validation.

This validates mappings and evidence identity. It does not execute checks, approve
designs, or infer implementation conformance from OpenSpec task checkboxes.
"""

from __future__ import annotations

import argparse
import csv
from datetime import datetime
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path, PurePosixPath
import re
import sys
from typing import Any, Iterable


_CHANGE_ID = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
_CHECK_ID = re.compile(r"[a-z0-9]+(?:[-_.][a-z0-9]+)*\Z")
_REQUIREMENT_ID = re.compile(r"[A-Z][A-Z0-9]*(?:-[A-Za-z0-9]+)+\Z")
_SHA256 = re.compile(r"[0-9a-fA-F]{64}\Z")
_HEADING = re.compile(r"^\s{0,3}#{2,6}\s+(Requirement|Scenario):\s*(.*?)\s*#*\s*$")
_TASK = re.compile(r"^\s*[-*]\s+\[([ xX])]\s+\S", re.MULTILINE)
_LEGACY_REFERENCE_SHA256 = "3a7f8f8751db82a99721b399d2deec670912d5fe9a7f4c6edfc229b116bed1e7"


def _text(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip()) and not any(
        ord(char) < 32 for char in value
    )


def _relative(value: Any, label: str, errors: list[str]) -> PurePosixPath | None:
    if not _text(value) or value != value.strip():
        errors.append(f"{label}: expected a nonempty relative POSIX path")
        return None
    if "\\" in value or ":" in value or "\x7f" in value:
        errors.append(f"{label}: path must use relative POSIX syntax")
        return None
    parts = value.split("/")
    if any(part in {"", ".", ".."} for part in parts) or value.startswith("/"):
        errors.append(f"{label}: absolute paths, empty segments, and traversal are forbidden")
        return None
    return PurePosixPath(value)


def _safe_path(
    root: Path, value: Any, label: str, errors: list[str], *, must_exist: bool = True
) -> Path | None:
    relative = _relative(value, label, errors)
    if relative is None:
        return None
    current = root
    for part in relative.parts:
        current /= part
        if current.is_symlink():
            errors.append(f"{label}: symbolic links are forbidden ({value})")
            return None
    try:
        current.resolve().relative_to(root)
    except (OSError, ValueError, RuntimeError):
        errors.append(f"{label}: path escapes the repository")
        return None
    if must_exist and not current.is_file():
        errors.append(f"{label}: missing or nonregular file ({value})")
        return None
    return current


def _json_object(path: Path | None, label: str, errors: list[str]) -> dict[str, Any] | None:
    if path is None:
        return None
    try:
        def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
            result: dict[str, Any] = {}
            for key, value in pairs:
                if key in result:
                    raise ValueError(f"duplicate JSON key {key!r}")
                result[key] = value
            return result

        def reject_constant(value: str) -> None:
            raise ValueError(f"non-JSON numeric constant {value}")

        document = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object, parse_constant=reject_constant)
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        errors.append(f"{label}: invalid JSON ({error})")
        return None
    if not isinstance(document, dict):
        errors.append(f"{label}: expected a JSON object")
        return None
    return document


def _strings(value: Any, label: str, errors: list[str]) -> list[str]:
    if not isinstance(value, list) or not value:
        errors.append(f"{label}: expected a nonempty list")
        return []
    if any(not _text(item) or item != item.strip() for item in value):
        errors.append(f"{label}: entries must be nonempty strings without control characters")
        return []
    if len(set(value)) != len(value):
        errors.append(f"{label}: duplicate entries")
    return value


def _allowed_paths(root: Path, value: Any, change_id: str, kind: str, errors: list[str]) -> None:
    for index, pattern in enumerate(_strings(value, "allowed_paths", errors)):
        label = f"allowed_paths[{index}]"
        directory = pattern.endswith("/**")
        base = pattern[:-3] if directory else pattern
        if any(char in base for char in "*?[]{}") or "#" in base:
            errors.append(f"{label}: only an exact path or a trailing /** is supported")
            continue
        parsed = _relative(base, label, errors)
        if parsed is None:
            continue
        _safe_path(root, base, label, errors, must_exist=False)
        parts = parsed.parts
        if directory and len(parts) < 2:
            errors.append(f"{label}: broad top-level directory grants are forbidden")
        if parts[0] == "openspec":
            if directory and base == "openspec/changes":
                errors.append(f"{label}: a change may not grant access to all changes")
            elif len(parts) >= 3 and parts[1] == "changes" and parts[2] != change_id:
                errors.append(f"{label}: may not grant another change's directory")
            elif directory and len(parts) == 2 and parts[1] != "specs":
                errors.append(f"{label}: broad OpenSpec directory grant")
        if kind == "tooling" and parts[0] in {"apps", "crates", "native", "runtime"}:
            errors.append(f"{label}: tooling changes may not grant product source/runtime paths")
        if directory and parts[0] in {"apps", "crates"} and len(parts) < 3:
            errors.append(f"{label}: product directory grant is too broad")


def _delta(root: Path, change_id: str, errors: list[str]) -> dict[str, list[tuple[str, list[str]]]]:
    base = root / "openspec" / "changes" / change_id
    specs = base / "specs"
    parsed: dict[str, list[tuple[str, list[str]]]] = {}
    if not specs.is_dir() or specs.is_symlink():
        errors.append("delta specs: missing or symbolic-link specs directory")
        return parsed
    paths = sorted(specs.rglob("spec.md"))
    for candidate in specs.rglob("*"):
        if candidate.is_symlink():
            errors.append(f"delta specs: symbolic links are forbidden ({candidate.relative_to(base).as_posix()})")
    if not paths:
        errors.append("delta specs: no specs/<capability>/spec.md files")
    for path in paths:
        relative = path.relative_to(base).as_posix()
        checked = _safe_path(root, path.relative_to(root).as_posix(), f"delta {relative}", errors)
        if checked is None:
            continue
        try:
            body = checked.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"delta {relative}: cannot read ({error})")
            continue
        requirements: list[tuple[str, list[str]]] = []
        current: tuple[str, list[str]] | None = None
        fence: tuple[str, int] | None = None
        for line in body.splitlines():
            marker = re.match(r"^\s{0,3}(`{3,}|~{3,})(.*)$", line)
            if fence is not None:
                if marker and marker[1][0] == fence[0] and len(marker[1]) >= fence[1] and not marker[2].strip():
                    fence = None
                continue
            if marker:
                # Backtick fence info strings may not contain backticks.
                if marker[1][0] != "`" or "`" not in marker[2]:
                    fence = (marker[1][0], len(marker[1]))
                    continue
            match = _HEADING.match(line)
            if match is None:
                continue
            heading, title = match.groups()
            if not _text(title):
                errors.append(f"delta {relative}: empty {heading} heading")
                continue
            if heading == "Requirement":
                current = (title, [])
                requirements.append(current)
            elif current is None:
                errors.append(f"delta {relative}: Scenario without a Requirement ({title})")
            else:
                if title in current[1]:
                    errors.append(f"delta {relative}: duplicate Scenario ({title})")
                current[1].append(title)
        if not requirements:
            errors.append(f"delta {relative}: missing Requirement headings")
        for title, scenarios in requirements:
            if not scenarios:
                errors.append(f"delta {relative}: Requirement has no Scenario ({title})")
        if len({title for title, _ in requirements}) != len(requirements):
            errors.append(f"delta {relative}: duplicate Requirement headings")
        parsed[relative] = requirements
    return parsed


def _matrix_ids(root: Path, filename: str, column: str, errors: list[str]) -> set[str]:
    path = _safe_path(root, f"docs/architecture/{filename}", filename, errors)
    if path is None:
        return set()
    try:
        with path.open(newline="", encoding="utf-8") as stream:
            rows = list(csv.DictReader(stream))
        ids = [row[column] for row in rows]
        if not ids or any(not _text(item) for item in ids) or len(set(ids)) != len(ids):
            raise ValueError("empty or duplicate IDs")
        return set(ids)
    except (OSError, UnicodeError, KeyError, ValueError, csv.Error) as error:
        errors.append(f"{filename}: invalid matrix ({error})")
        return set()


def _product_sources(root: Path, errors: list[str]) -> tuple[set[str], set[str]]:
    """Read known product IDs from the authoritative local documents, not a list of IDs in code."""
    known: set[str] = set()
    authorities: set[str] = set()
    markdown_sources = {
        "docs/product/first-product-scope.md": r"^\|\s*(FP-\d+)\s*\|",
        "docs/product/model-configuration-prd.md": r"^###\s+(AI-CFG-\d+)\b",
        "docs/product/ai-reading-prd.md": r"^\|\s*(AI-SC-\d+)\b",
    }
    csv_sources = {
        "docs/product/model-configuration-acceptance.csv": "acceptance_id",
        "docs/product/game-library-window-acceptance.csv": "case_id",
        "docs/product/reading-window-session-acceptance.csv": "case_id",
        "docs/development/requirements.csv": "requirement_id",
        "docs/development/client-learning-extensions.csv": "extension_id",
    }
    for relative, pattern in markdown_sources.items():
        path = _safe_path(root, relative, "product source catalog", errors)
        if path is None:
            continue
        try:
            identifiers = re.findall(pattern, path.read_text(encoding="utf-8"), re.MULTILINE)
            if not identifiers or len(identifiers) != len(set(identifiers)):
                raise ValueError("missing or duplicate authoritative IDs")
            known.update(identifiers)
            authorities.add(relative)
        except (OSError, UnicodeError, ValueError) as error:
            errors.append(f"product source catalog: {relative}: {error}")
    for relative, column in csv_sources.items():
        path = _safe_path(root, relative, "product source catalog", errors)
        if path is None:
            continue
        try:
            with path.open(newline="", encoding="utf-8") as stream:
                identifiers = [row[column] for row in csv.DictReader(stream)]
            if not identifiers or any(not _text(identifier) for identifier in identifiers) or len(identifiers) != len(set(identifiers)):
                raise ValueError("missing or duplicate authoritative IDs")
            known.update(identifiers)
            authorities.add(relative)
        except (OSError, UnicodeError, KeyError, ValueError, csv.Error) as error:
            errors.append(f"product source catalog: {relative}: {error}")
    authorities.update({"docs/product/game-library-window-prd.md", "docs/product/reading-window-session-prd.md", "docs/product/original/docs/01-prd.md"})
    return known, authorities


def _historical_design_references(root: Path, change_id: str) -> frozenset[str]:
    """Compatibility is limited to byte-exact, code-pinned historical changes."""
    try:
        raw = Path(__file__).with_name("legacy_product_references.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != _LEGACY_REFERENCE_SHA256:
            return frozenset()
        frozen = json.loads(raw)["changes"].get(change_id)
        if not frozen:
            return frozenset()
        failures: list[str] = []
        metadata = _safe_path(root, f"openspec/changes/{change_id}/verification.json", "historical metadata", failures)
        if metadata is None or hashlib.sha256(metadata.read_bytes()).hexdigest() != frozen["verification_sha256"]:
            return frozenset()
        change_directory = root / "openspec" / "changes" / change_id
        actual = {path.relative_to(root).as_posix() for path in change_directory.rglob('*') if path.is_file() or path.is_symlink()}
        if actual != set(frozen['change_sources']):
            return frozenset()
        for relative, expected in {**frozen["design_sources"], **frozen['change_sources']}.items():
            failures: list[str] = []
            path = _safe_path(root, relative, "historical design", failures)
            if path is None or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                return frozenset()
        return frozenset(frozen["references"])
    except (OSError, ValueError, KeyError, TypeError):
        return frozenset()


def _literal_design_anchor(path: Path, anchor: str, label: str, errors: list[str]) -> None:
    """Parse the anchor after authority and path safety have been established."""
    class Anchors(HTMLParser):
        def __init__(self) -> None:
            super().__init__(convert_charrefs=True)
            self.values: set[str] = set()
            self.inert: list[str] = []

        def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
            if tag in {"template", "noscript"}:
                self.inert.append(tag)
                return
            if self.inert:
                return
            self.values.update(value for name, value in attrs if value is not None and (name == "id" or name == "name" and tag == "a"))

        def handle_endtag(self, tag: str) -> None:
            if self.inert and self.inert[-1] == tag:
                self.inert.pop()

    try:
        parser = Anchors()
        parser.feed(path.read_text(encoding="utf-8"))
        parser.close()
    except (OSError, UnicodeError, ValueError) as error:
        errors.append(f"{label}: cannot parse current HTML design ({error})")
        return
    if anchor not in parser.values:
        errors.append(f"{label}: missing literal HTML id/name #{anchor} in {path.name}; dynamic placeholders are unsupported")


def _design_reference(root: Path, reference: str, label: str, errors: list[str], *, historical: frozenset[str] = frozenset()) -> None:
    """Bind a product reference to a literal anchor in the current HTML design."""
    root = root.resolve()
    value, separator, anchor = reference.partition("#")
    if not separator or not anchor or "#" in anchor or not _text(anchor) or any(char.isspace() for char in anchor):
        errors.append(f"{label}: expected file#id with a literal HTML id/name, not a selector ({reference})")
        return
    parsed = _relative(value, label, errors)
    if parsed is None:
        return
    if reference not in historical:
        # A root-HTML path is not an approval. New and modified product changes
        # must name the exact approved composition and source closure.
        from design_authority import CURRENT_ROUTES, current_source_approval
        entries = {route['entry'] for route in CURRENT_ROUTES.values()}
        if value not in entries:
            errors.append(f"{label}: must reference current code-owned design entry, not an arbitrary candidate, archive or report")
            return
        approval = current_source_approval(root)
        if approval.get('status') != 'approved_by_user' or approval.get('errors'):
            errors.append(f"{label}: current design source approval or exact source closure is invalid")
            return
    path = _safe_path(root, value, label, errors)
    if path is None:
        return

    _literal_design_anchor(path, anchor, label, errors)


def validate_change(root: Path | str, change_id: str) -> list[str]:
    """Return validation errors; an empty list means only these static checks passed."""
    errors: list[str] = []
    root = Path(root).resolve()
    if not isinstance(change_id, str) or _CHANGE_ID.fullmatch(change_id) is None:
        return ["change_id: expected lowercase kebab-case without traversal"]
    prefix = f"openspec/changes/{change_id}"
    for filename in ("proposal.md", "design.md", "tasks.md"):
        path = _safe_path(root, f"{prefix}/{filename}", filename, errors)
        if path is not None:
            try:
                if not path.read_text(encoding="utf-8").strip():
                    errors.append(f"{filename}: file must not be empty")
            except (OSError, UnicodeError) as error:
                errors.append(f"{filename}: cannot read ({error})")
    document = _json_object(
        _safe_path(root, f"{prefix}/verification.json", "verification.json", errors),
        "verification.json", errors,
    )
    if document is None:
        return errors
    if type(document.get("schema_version")) is not int or document["schema_version"] != 1:
        errors.append("schema_version: expected integer 1")
    if document.get("id") != change_id:
        errors.append("id: must equal the requested change_id")
    kind = document.get("kind")
    if not isinstance(kind, str) or kind not in {"tooling", "product"}:
        errors.append("kind: expected tooling or product")
    _allowed_paths(root, document.get("allowed_paths"), change_id, kind, errors)
    snapshot = f"{prefix}/source-snapshot.json"
    if document.get("source_snapshot") != snapshot:
        errors.append(f"source_snapshot: must equal {snapshot}")
    else:
        _json_object(_safe_path(root, snapshot, "source_snapshot", errors), "source_snapshot", errors)

    deltas = _delta(root, change_id, errors)
    entries = document.get("requirements")
    if not isinstance(entries, list) or not entries:
        errors.append("requirements: expected a nonempty list of mappings")
        entries = []
    modules = _matrix_ids(root, "engineering-modules.csv", "module_id", errors) if kind == "product" else set()
    components = _matrix_ids(root, "engineering-components.csv", "component_id", errors) if kind == "product" else set()
    source_ids, authority_paths = _product_sources(root, errors) if kind == "product" else (set(), set())
    historical = _historical_design_references(root, change_id) if kind == "product" else frozenset()
    coverage: set[tuple[str, str, str]] = set()
    seen: set[tuple[str, str, str]] = set()
    for index, entry in enumerate(entries):
        label = f"requirements[{index}]"
        if not isinstance(entry, dict):
            errors.append(f"{label}: expected an object")
            continue
        identifier, spec, scenario = entry.get("id"), entry.get("spec"), entry.get("scenario")
        if not isinstance(identifier, str) or _REQUIREMENT_ID.fullmatch(identifier) is None:
            errors.append(f"{label}.id: expected a requirement ID such as ENG-001")
            continue
        parsed = _relative(spec, f"{label}.spec", errors)
        if parsed is None or len(parsed.parts) < 3 or parsed.parts[0] != "specs" or parsed.parts[-1] != "spec.md":
            errors.append(f"{label}.spec: expected specs/<capability>/spec.md within this change")
            continue
        if not _text(scenario) or scenario != scenario.strip():
            errors.append(f"{label}.scenario: expected an exact Scenario heading")
            continue
        key = (spec, identifier, scenario)
        if key in seen:
            errors.append(f"{label}: duplicate spec/requirement/scenario mapping")
        seen.add(key)
        titles = [
            title for title, scenarios in deltas.get(spec, [])
            if re.search(rf"(?<![A-Za-z0-9-]){re.escape(identifier)}(?![A-Za-z0-9-])", title)
            and scenario in scenarios
        ]
        if len(titles) != 1:
            errors.append(f"{label}: requirement ID and Scenario must match exactly one delta heading")
        else:
            coverage.add((spec, titles[0], scenario))
        for check in _strings(entry.get("checks"), f"{label}.checks", errors):
            if _CHECK_ID.fullmatch(check) is None:
                errors.append(f"{label}.checks: invalid check ID ({check})")
        if kind == "product":
            if identifier not in source_ids:
                source_requirements = _strings(entry.get("source_requirement_ids"), f"{label}.source_requirement_ids", errors)
                if not source_requirements:
                    errors.append(f"{label}.id: unknown product requirement ({identifier}); explicit authoritative source_requirement_ids are required")
            elif "source_requirement_ids" in entry:
                source_requirements = _strings(entry["source_requirement_ids"], f"{label}.source_requirement_ids", errors)
            else:
                source_requirements = [identifier]
            for source_identifier in source_requirements:
                if source_identifier not in source_ids:
                    errors.append(f"{label}.source_requirement_ids: unknown authoritative product requirement ({source_identifier})")
            source_refs = _strings(entry.get("source_refs"), f"{label}.source_refs", errors)
            for ref in source_refs:
                _safe_path(root, ref, f"{label}.source_refs", errors)
            if not set(source_refs) & authority_paths:
                errors.append(f"{label}.source_refs: must cite an authoritative product PRD or requirements source; a README alone is insufficient")
            for ref in _strings(entry.get("design_refs"), f"{label}.design_refs", errors):
                _design_reference(root, ref, f"{label}.design_refs", errors, historical=historical)
            for field, known in (("module_ids", modules), ("component_ids", components)):
                for identifier_ref in _strings(entry.get(field), f"{label}.{field}", errors):
                    if identifier_ref not in known:
                        errors.append(f"{label}.{field}: unknown ID ({identifier_ref})")
    for spec, requirements in deltas.items():
        for title, scenarios in requirements:
            for scenario in scenarios:
                if (spec, title, scenario) not in coverage:
                    errors.append(f"unmapped delta Scenario: {spec} / {title} / {scenario}")

    claimed_verified = document.get("verified") is True or any(
        document.get(field) in {"verified", "passed"} for field in ("status", "implementation_status")
        if isinstance(document.get(field), str)
    )
    if claimed_verified:
        path = _safe_path(root, f"{prefix}/tasks.md", "tasks.md", errors)
        if path is not None:
            try:
                tasks = _TASK.findall(path.read_text(encoding="utf-8"))
                if not tasks or not any(mark.lower() == "x" for mark in tasks):
                    errors.append("verified claim: tasks are absent or all unchecked; checkboxes never prove product conformance")
            except (OSError, UnicodeError) as error:
                errors.append(f"tasks.md: cannot read ({error})")
    return errors


def _hashed_evidence(
    root: Path, record: Any, label: str, errors: list[str]
) -> Path | None:
    """Validate an existing, nonempty, nonsymlink evidence file independently."""
    if not isinstance(record, dict):
        errors.append(f"{label}: expected a file/hash object")
        return None
    path = _safe_path(root, record.get("path"), label, errors)
    digest = record.get("sha256")
    if not isinstance(digest, str) or _SHA256.fullmatch(digest) is None:
        errors.append(f"{label}.sha256: expected a SHA-256")
        return None
    if path is None:
        return None
    try:
        data = path.read_bytes()
        if not data:
            errors.append(f"{label}: empty evidence files cannot support a passed result")
            return None
        if hashlib.sha256(data).hexdigest() != digest.lower():
            errors.append(f"{label}.sha256: evidence file hash mismatch")
            return None
    except OSError as error:
        errors.append(f"{label}: cannot read evidence ({error})")
        return None
    return path


def _release_adapter_registry() -> dict[str, tuple[tuple[str, ...], str]]:
    """Only executable adapters registered in code can support release evidence.

    Registering a command does not supply execution coverage. Its receipt must
    additionally carry the case/variant assertions inspected below. In
    particular, a browser/component adapter cannot grant native/game coverage.
    """
    from product_execution import (
        PRODUCT_CAPTURE_CHECKS, PRODUCT_RUNTIME_CHECKS, PRODUCT_TEST_CHECKS,
    )

    return {
        **{key: (tuple(command), "source_build") for key, command in
           {**PRODUCT_CAPTURE_CHECKS, **PRODUCT_TEST_CHECKS}.items()},
        **{key: (tuple(command), "final_package") for key, command in
           PRODUCT_RUNTIME_CHECKS.items()},
    }


def _registered_release_command(
    adapter_id: str, registration: tuple[tuple[str, ...], str], receipt_path: Path
) -> list[str]:
    """Mirror the existing runner's sole bounded capture-output argument."""
    from product_execution import PRODUCT_CAPTURE_CHECKS, RUNTIME_STABILITY_CHECK

    command = list(registration[0])
    if adapter_id in PRODUCT_CAPTURE_CHECKS:
        command += ["--output", str(receipt_path.parent / "capture")]
    if adapter_id == RUNTIME_STABILITY_CHECK:
        receipt=json.loads(receipt_path.read_text())
        nonce=receipt.get("execution_nonce")
        if not isinstance(nonce,str) or not re.fullmatch("[0-9a-f]{32}",nonce): return []
        command += ["--inputs",str(receipt_path.parent/"runtime-inputs.json"),"--output",str(receipt_path.parent/"runtime"),"--execution-nonce",nonce]
    return command


def _release_plan(root: Path, errors: list[str]) -> dict[str, Any] | None:
    from test_plan_contract import PLAN_PATH, validate_plan

    plan = _json_object(_safe_path(root, PLAN_PATH, "release test plan", errors),
                        "release test plan", errors)
    if plan is None:
        return None
    plan_errors = validate_plan(root, plan)
    errors.extend(f"release test plan: {error}" for error in plan_errors)
    return None if plan_errors else plan


def _execution_log_rows(path: Path, errors: list[str]) -> dict[str, dict[str, Any]]:
    """Read adapter-emitted assertion records, rather than trusting a summary."""
    rows: dict[str, dict[str, Any]] = {}

    def unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise ValueError(f"duplicate JSON key {key!r}")
            value[key] = item
        return value

    def reject_constant(value: str) -> None:
        raise ValueError(f"non-JSON constant {value}")

    try:
        for line in path.read_text(encoding="utf-8").splitlines():
            if not line.startswith("LUNA_RELEASE_RESULT "):
                continue
            row = json.loads(line.removeprefix("LUNA_RELEASE_RESULT "),
                             object_pairs_hook=unique,
                             parse_constant=reject_constant)
            identifier = row.get("execution_id") if isinstance(row, dict) else None
            if not _text(identifier) or identifier in rows:
                errors.append("release execution log: empty or duplicate execution ID")
            else:
                rows[identifier] = row
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        errors.append(f"release execution log: cannot inspect assertion records ({error})")
    if not rows:
        errors.append("release execution log: missing actual per-combination assertion records")
    return rows


def _release_execution_results(
    root: Path, document: dict[str, Any], expected: str, errors: list[str]
) -> None:
    """Require the exact validated plan inventory and inspect bound runner output."""
    from product_execution import tree_digest
    from test_plan_contract import required_executions

    plan = _release_plan(root, errors)
    if plan is None:
        return
    required = required_executions(plan)
    variants = {row["id"]: row for row in plan["variants"]}
    rows = document.get("execution_results")
    if not isinstance(rows, list) or not rows:
        errors.append("release execution_results: expected all planned case@variant combinations")
        rows = []
    registry = _release_adapter_registry()
    current_source = tree_digest(root)
    seen: set[str] = set()
    inspected: dict[Path, tuple[dict[str, Any] | None, dict[str, dict[str, Any]]]] = {}
    for index, row in enumerate(rows):
        label = f"execution_results[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{label}: expected an object")
            continue
        identifier = row.get("execution_id")
        if not isinstance(identifier, str) or identifier not in required:
            errors.append(f"{label}.execution_id: unknown planned combination ({identifier})")
            continue
        if identifier in seen:
            errors.append(f"{label}.execution_id: duplicate planned combination ({identifier})")
        seen.add(identifier)
        case_id, variant_id = identifier.rsplit("@", 1)
        variant = variants[variant_id]
        scope = variant["evidence_scope"]
        adapter_id = row.get("adapter_id")
        adapter = registry.get(adapter_id) if isinstance(adapter_id, str) else None
        if adapter is None or adapter[1] != scope:
            errors.append(f"{label}.adapter_id: requires a code-registered {scope} adapter; fixture/component cannot substitute for native runtime")
        receipt_path = _hashed_evidence(root, row.get("receipt"), f"{label}.receipt", errors)
        if receipt_path is None:
            continue
        if receipt_path not in inspected:
            receipt = _json_object(receipt_path, f"{label}.receipt", errors)
            log_rows: dict[str, dict[str, Any]] = {}
            if receipt is not None:
                if receipt.get("schema_version") != 1 or type(receipt.get("schema_version")) is not int:
                    errors.append(f"{label}.receipt: expected schema_version integer 1")
                log_path = _hashed_evidence(root, {"path": receipt.get("log"), "sha256": receipt.get("log_sha256")}, f"{label}.receipt.log", errors)
                if log_path is not None:
                    log_rows = _execution_log_rows(log_path, errors)
            inspected[receipt_path] = receipt, log_rows
        receipt, log_rows = inspected[receipt_path]
        if receipt is None:
            continue
        if receipt.get("adapter_id") != adapter_id or adapter is not None and receipt.get("command") != _registered_release_command(adapter_id, adapter, receipt_path):
            errors.append(f"{label}.receipt: adapter identity/command differs from code registration")
        if receipt.get("status") != "passed" or receipt.get("errors") != [] or type(receipt.get("exit_code")) is not int or receipt["exit_code"] != 0:
            errors.append(f"{label}.receipt: actual adapter must exit zero with no errors")
        nonce = receipt.get("execution_nonce")
        if not isinstance(nonce, str) or re.fullmatch(r"[0-9a-f]{32}", nonce) is None:
            errors.append(f"{label}.receipt: missing actual execution nonce")
        if receipt.get("source_tree_sha256") != current_source:
            errors.append(f"{label}.receipt: source tree differs from current validated sources")
        if receipt.get("artifact_sha256") != expected:
            errors.append(f"{label}.receipt: belongs to another candidate artifact")
        flag = "product_packaged_native_game_tests_executed" if scope == "final_package" else "product_component_tests_executed"
        if receipt.get(flag) is not True or receipt.get("fixture_substitution") is not False:
            errors.append(f"{label}.receipt: actual {scope} execution required; fixture/no-runtime substitution is forbidden")
        results = receipt.get("execution_results")
        results = results if isinstance(results, list) else []
        identities = [item.get("execution_id") for item in results if isinstance(item, dict)]
        if len(identities) != len(results) or any(not isinstance(item, str) for item in identities) or len(set(item for item in identities if isinstance(item, str))) != len(identities):
            errors.append(f"{label}.receipt: malformed or duplicate assertion result identities")
        matches = [item for item in results if isinstance(item, dict) and item.get("execution_id") == identifier]
        if len(matches) != 1:
            errors.append(f"{label}.receipt: missing unique actual assertions for {identifier}")
            continue
        actual = matches[0]
        for key, value in {"case_id": case_id, "variant_id": variant_id, "status": "passed", "evidence_scope": scope,
                           "source_tree_sha256": current_source, "artifact_sha256": expected, "execution_nonce": nonce,
                           "environment_ids": variant["environment_ids"], "fixture_substitution": False}.items():
            if actual.get(key) != value or isinstance(value, bool) and type(actual.get(key)) is not bool:
                errors.append(f"{label}.receipt.{key}: actual assertion identity/environment differs from planned execution")
        if type(actual.get("assertion_count")) is not int or actual["assertion_count"] <= 0 or type(actual.get("skipped_assertions")) is not int or actual["skipped_assertions"] != 0:
            errors.append(f"{label}.receipt: actual nonzero assertions with zero skipped assertions are required")
        try:
            started, observed, finished = (datetime.fromisoformat(value.replace("Z", "+00:00")) for value in
                                            (receipt.get("started_at"), actual.get("observed_at"), receipt.get("finished_at")))
            if any(value.tzinfo is None for value in (started, observed, finished)) or not started <= observed <= finished:
                raise ValueError("assertion timestamp outside actual execution")
        except (TypeError, ValueError, AttributeError):
            errors.append(f"{label}.receipt: timezone-aware execution/assertion timestamps must bind the actual run")
        if log_rows.get(identifier) != actual:
            errors.append(f"{label}.receipt: assertion summary differs from actual adapter log record")
        evidence = actual.get("evidence")
        if not isinstance(evidence, list) or not evidence:
            errors.append(f"{label}.receipt: assertions require nonempty observed file/hash evidence")
        else:
            evidence_seen: set[str] = set()
            for evidence_index, record in enumerate(evidence):
                path = _hashed_evidence(root, record, f"{label}.receipt.evidence[{evidence_index}]", errors)
                if path is not None:
                    relative = str(path.relative_to(root))
                    if relative in evidence_seen:
                        errors.append(f"{label}.receipt: duplicate observed evidence path")
                    evidence_seen.add(relative)
    for missing in sorted(required - seen):
        errors.append(f"release execution_results: missing required planned combination ({missing})")


def validate_release(
    root: Path | str,
    evidence_path: Path | str,
    expected_artifact_sha256: str,
    required_case_ids: Iterable[str],
) -> list[str]:
    """Inspect source cases and every planned execution for one exact artifact.

    This validates identity and evidence consistency. The caller must still run
    registered adapters; accepting a JSON document does not attest human or
    operating-system events independently of those trusted executors.
    """
    errors: list[str] = []
    root = Path(root).resolve()
    if not isinstance(expected_artifact_sha256, str) or _SHA256.fullmatch(expected_artifact_sha256) is None:
        return ["expected_artifact_sha256: expected a 64-character SHA-256"]
    expected = expected_artifact_sha256.lower()
    if isinstance(required_case_ids, (str, bytes)):
        return ["required_case_ids: expected a nonempty collection of unique case IDs"]
    try:
        required_list = list(required_case_ids)
    except TypeError:
        return ["required_case_ids: expected a nonempty collection of unique case IDs"]
    required = _strings(required_list, "required_case_ids", errors)
    path_value = str(evidence_path)
    if Path(path_value).is_absolute():
        try:
            path_value = Path(path_value).relative_to(root).as_posix()
        except ValueError:
            errors.append("release evidence: absolute path escapes repository")
            return errors
    document = _json_object(_safe_path(root, path_value, "release evidence", errors), "release evidence", errors)
    if document is None:
        return errors
    if type(document.get("schema_version")) is not int or document["schema_version"] != 2:
        errors.append("release schema_version: expected integer 2 with per-combination execution evidence")
    artifact = document.get("artifact_sha256")
    if not isinstance(artifact, str) or artifact.lower() != expected:
        errors.append("release artifact_sha256: does not match the expected artifact")
    rows = document.get("results")
    if not isinstance(rows, list) or not rows:
        errors.append("release results: expected a nonempty list")
        rows = []
    seen: set[str] = set()
    for index, row in enumerate(rows):
        label = f"results[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{label}: expected an object")
            continue
        case_id = row.get("case_id")
        if not _text(case_id) or case_id != case_id.strip():
            errors.append(f"{label}.case_id: expected a nonempty case ID")
        elif case_id in seen:
            errors.append(f"{label}.case_id: duplicate case ({case_id})")
        else:
            seen.add(case_id)
        if row.get("status") != "passed":
            errors.append(f"{label}.status: required release result must be passed")
        row_artifact = row.get("artifact_sha256")
        if not isinstance(row_artifact, str) or row_artifact.lower() != expected:
            errors.append(f"{label}.artifact_sha256: result belongs to another artifact")
        records = row.get("evidence")
        if not isinstance(records, list) or not records:
            errors.append(f"{label}.evidence: passed requires nonempty file/hash evidence")
            records = []
        evidence_seen: set[str] = set()
        for evidence_index, record in enumerate(records):
            record_label = f"{label}.evidence[{evidence_index}]"
            if not isinstance(record, dict):
                errors.append(f"{record_label}: expected an object")
                continue
            ref, digest = record.get("path"), record.get("sha256")
            if isinstance(ref, str):
                if ref in evidence_seen:
                    errors.append(f"{record_label}: duplicate evidence path")
                evidence_seen.add(ref)
            checked = _safe_path(root, ref, record_label, errors)
            if not isinstance(digest, str) or _SHA256.fullmatch(digest) is None:
                errors.append(f"{record_label}.sha256: expected a SHA-256")
            elif checked is not None:
                try:
                    data = checked.read_bytes()
                    if not data:
                        errors.append(f"{record_label}: empty evidence files cannot support a passed result")
                    actual = hashlib.sha256(data).hexdigest()
                    if actual != digest.lower():
                        errors.append(f"{record_label}.sha256: evidence file hash mismatch")
                except OSError as error:
                    errors.append(f"{record_label}: cannot read evidence ({error})")
    for missing in sorted(set(required) - seen):
        errors.append(f"release results: missing required case ({missing})")
    _release_execution_results(root, document, expected, errors)
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--change")
    mode.add_argument("--release-evidence", type=Path)
    parser.add_argument("--artifact-sha256")
    parser.add_argument("--required-case", action="append", default=[])
    parser.add_argument("--json", action="store_true")
    arguments = parser.parse_args(argv)
    if arguments.change is not None:
        errors = validate_change(arguments.root, arguments.change)
    else:
        errors = validate_release(arguments.root, arguments.release_evidence, arguments.artifact_sha256, arguments.required_case)
    if arguments.json:
        print(json.dumps({"valid": not errors, "errors": errors, "conformance_claim": "static mappings/evidence identity only"}, ensure_ascii=False))
    elif errors:
        for error in errors:
            print(error, file=sys.stderr)
    else:
        print("Static contract checks passed; no implementation conformance is implied.")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
