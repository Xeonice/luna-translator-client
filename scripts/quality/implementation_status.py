"""Validate and explicitly export the source-audited implementation status.

This inventory binds a human review to source lines and historical artifacts.
It does not infer business correctness, execute the product, or approve release.
The default command is read-only; only ``--write`` updates derived documents.
"""
from __future__ import annotations

import argparse
import csv
from datetime import date
import hashlib
import html
import io
import json
import math
import os
from pathlib import Path
import plistlib
import re
from typing import Any
from urllib.parse import quote

from gate_core import digest, safe_path, source_files
from change_review import _actor as canonical_review_actor


CATALOG = "docs/architecture/implementation-status.json"
MARKDOWN = "docs/architecture/implementation-status.md"
MODULES = "docs/architecture/engineering-modules.csv"
COMPONENTS = "docs/architecture/engineering-components.csv"
SCOPE = "docs/product/first-product-scope.md"
RW_ACCEPTANCE = "docs/product/reading-window-session-acceptance.csv"
CURRENT_MANIFEST = "dist/build-manifest.json"
PRODUCT_ROOTS = frozenset({"apps", "crates", "native", "resources"})
IMPLEMENTATION = {
    "implemented": "已实现",
    "partial": "部分实现",
    "not_implemented": "尚未实现",
}
ALIGNMENT = {
    "aligned": "源码对齐",
    "partial": "部分对齐",
    "mismatch": "与要求不一致",
    "not_assessed": "未评估",
}
VERIFICATION = {
    "source_only": "仅源码核对",
    "historical_pass": "历史包范围通过",
    "historical_mixed": "历史包结果不一",
    "historical_failed": "历史包范围失败",
    "not_run": "未执行",
    "blocked": "受阻",
}
EXPORT_FIELDS = (
    "alignment_status", "verification_status", "current_implementation",
    "remaining_gap", "evidence_refs", "status_record",
)
RECORD_FIELDS = frozenset({
    "id", "name", "implementation_status", "alignment_status",
    "verification_status", "current_behavior", "target_behavior", "gap",
    "next_step", "owner", "requirement_ids", "module_ids", "component_ids",
    "source_evidence", "historical_evidence", "visual_alignment_status",
})
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
REFERENCE = re.compile(r"(?<![A-Za-z0-9-])(?:FP-\d+|RW-\d+|M\d+|C\d+)(?![A-Za-z0-9-])")
CAPABILITY_ID = re.compile(r"CAP-[A-Z][A-Z0-9-]*\Z")


def product_source_identity(root: Path) -> dict[str, str]:
    """Freeze the complete product source set, excluding generated build roots."""
    return {
        path: sha for path, sha in source_files(root).items()
        if Path(path).parts[0] in PRODUCT_ROOTS
    }


def _csv(root: Path, relative: str) -> tuple[list[str], list[dict[str, str]]]:
    with safe_path(root, relative).open(newline="", encoding="utf-8-sig") as stream:
        reader = csv.DictReader(stream)
        if not reader.fieldnames:
            raise ValueError(f"Missing CSV header: {relative}")
        return list(reader.fieldnames), list(reader)


def _authority(root: Path) -> dict[str, set[str]]:
    _, modules = _csv(root, MODULES)
    _, components = _csv(root, COMPONENTS)
    _, acceptance = _csv(root, RW_ACCEPTANCE)
    scope = safe_path(root, SCOPE).read_text(encoding="utf-8")
    result = {
        "modules": {row["module_id"] for row in modules},
        "components": {row["component_id"] for row in components},
        "capabilities": set(re.findall(r"^\|\s*(FP-\d+)\s*\|", scope, re.MULTILINE)),
        "rw": {row["case_id"] for row in acceptance},
    }
    if any(not values or any(not _nonempty(value) for value in values)
           for values in result.values()):
        raise ValueError("Implementation status authority sets must be nonempty")
    for rows, field in ((modules, "module_id"), (components, "component_id"),
                        (acceptance, "case_id")):
        identities = [row[field] for row in rows]
        if len(identities) != len(set(identities)):
            raise ValueError(f"Duplicate authority ID in {field}")
    return result


def _nonempty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _evidence_path(root: Path, item: dict[str, Any], label: str,
                   errors: list[str]) -> Path | None:
    relative = item.get("path")
    try:
        path = safe_path(root, relative)
        if not path.is_file():
            errors.append(f"{label}: evidence file is missing: {relative}")
            return None
        if not isinstance(item.get("sha256"), str) or not SHA256.fullmatch(item["sha256"]):
            errors.append(f"{label}: sha256 must be a lowercase SHA-256 digest")
        elif digest(path) != item["sha256"]:
            errors.append(f"{label}: evidence SHA-256 drift: {relative}")
        return path
    except (TypeError, ValueError, OSError) as error:
        errors.append(f"{label}: invalid evidence path: {error}")
        return None


def _source_evidence(root: Path, items: Any, label: str,
                     errors: list[str]) -> None:
    if not isinstance(items, list) or not items:
        errors.append(f"{label}: source_evidence must contain an actual source line")
        return
    for index, item in enumerate(items):
        location = f"{label}.source_evidence[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{location}: evidence must be an object")
            continue
        path = _evidence_path(root, item, location, errors)
        relative = Path(item["path"]) if isinstance(item.get("path"), str) else None
        if relative is not None:
            parts = relative.parts
            source_root = bool(parts) and parts[0] in PRODUCT_ROOTS | {"scripts"}
            root_manifest = relative.as_posix() in {
                "package.json", "package-lock.json", "Cargo.toml", "Cargo.lock", "build.rs",
            }
            packaged_manifest = (bool(parts) and parts[0] == "dist"
                                 and (relative.name == "Info.plist" or relative.suffix == ".json"
                                      and any(term in relative.stem.lower()
                                              for term in ("manifest", "identity", "build", "source"))))
            if not (source_root or root_manifest or packaged_manifest):
                errors.append(f"{location}: source evidence must be product/tooling source or manifest, not audit prose")
        line, quote = item.get("line"), item.get("quote")
        if type(line) is not int or line < 1 or not _nonempty(quote):
            errors.append(f"{location}: line must be positive and quote nonempty")
            continue
        if path is None:
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            errors.append(f"{location}: evidence must be readable source text: {error}")
            continue
        if line > len(lines) or lines[line - 1].strip() != quote:
            errors.append(f"{location}: source line/quote mismatch: {item.get('path')}:{line}")


def _historical_evidence(root: Path, items: Any, status: str,
                         label: str, errors: list[str]) -> None:
    if not isinstance(items, list):
        errors.append(f"{label}: historical_evidence must be a list")
        return
    if status.startswith("historical_") and not items:
        errors.append(f"{label}: historical verification requires historical_evidence")
    for index, item in enumerate(items):
        location = f"{label}.historical_evidence[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{location}: evidence must be an object")
            continue
        _evidence_path(root, item, location, errors)
        if item.get("scope") != "historical_only":
            errors.append(f"{location}: scope must be historical_only, never current runtime")
        if not _nonempty(item.get("claim")):
            errors.append(f"{location}: historical claim must be nonempty")
        for key in ("source_tree_sha256", "package_sha256"):
            if key in item and (not isinstance(item[key], str) or not SHA256.fullmatch(item[key])):
                errors.append(f"{location}: invalid optional {key}")


def _current_runtime_claims(value: Any, errors: list[str], location: str = "catalog") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if child is True and any(term in key.lower() for term in (
                "runtime", "release_approved", "product_regression", "real_game", "native_verified",
            )):
                errors.append(f"{location}.{key}: this audit cannot claim current runtime/release success")
            _current_runtime_claims(child, errors, f"{location}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _current_runtime_claims(child, errors, f"{location}[{index}]")


def _sha_file(path: Path) -> str:
    """Hash large packaged artifacts without loading them into memory."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def _distribution_path(root: Path, value: Any, label: str) -> Path:
    if not _nonempty(value):
        raise ValueError(f"{label} must declare an actual distribution path")
    path = Path(value)
    relative = path.relative_to(root) if path.is_absolute() else path
    if not relative.parts or relative.parts[0] != "dist":
        raise ValueError(f"{label} must remain in repository dist")
    return safe_path(root, relative.as_posix())


def _current_artifact(root: Path, catalog: dict[str, Any], errors: list[str]) -> None:
    """Compare audit labels with existing bytes, not a fresh release acceptance."""
    current = catalog.get("current_artifact")
    if not isinstance(current, dict):
        errors.append("current_artifact must describe the existing distribution")
        return
    try:
        manifest_path = safe_path(root, CURRENT_MANIFEST)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if not isinstance(manifest, dict):
            raise ValueError("current distribution manifest must be an object")
        for field in ("version", "source_tree_sha256", "executable_sha256",
                      "archive_sha256", "distribution_status"):
            if not _nonempty(current.get(field)) or current[field] != manifest.get(field):
                errors.append(f"current_artifact.{field}: differs from actual build manifest")
        app = _distribution_path(root, manifest.get("app"), "manifest app")
        archive = _distribution_path(root, manifest.get("archive"), "manifest archive")
        if not app.is_dir() or not archive.is_file():
            raise ValueError("current distribution app/archive is missing")
        info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
        executable_name = info.get("CFBundleExecutable") if isinstance(info, dict) else None
        if not _nonempty(executable_name) or Path(executable_name).name != executable_name:
            raise ValueError("current distribution executable identity is invalid")
        executable = safe_path(root, (app / "Contents/MacOS" / executable_name).relative_to(root).as_posix())
        for field, actual in (("executable_sha256", _sha_file(executable)),
                              ("archive_sha256", _sha_file(archive))):
            if current.get(field) != actual or manifest.get(field) != actual:
                errors.append(f"current_artifact.{field}: actual distribution bytes differ")
        if info.get("CFBundleShortVersionString") != current.get("version"):
            errors.append("current_artifact.version: differs from actual app Info.plist")
        sealed_path = safe_path(root, (app / "Contents/Resources/luna-source-identity.json").relative_to(root).as_posix())
        sealed = json.loads(sealed_path.read_text(encoding="utf-8"))
        if not isinstance(sealed, dict):
            raise ValueError("current distribution source identity must be an object")
        for field in ("source_tree_sha256", "version"):
            if sealed.get(field) != current.get(field):
                errors.append(f"current_artifact.{field}: differs from existing app source identity")
        declared_paths = {
            "manifest_path": manifest_path, "app_path": app,
            "archive_path": archive, "executable_path": executable,
        }
        for field, actual in declared_paths.items():
            if field in current and _distribution_path(root, current[field], field) != actual:
                errors.append(f"current_artifact.{field}: differs from actual distribution path")
    except (OSError, ValueError, TypeError, KeyError, plistlib.InvalidFileException) as error:
        errors.append(f"Cannot verify current_artifact identity: {error}")


def _json_pointer(document: Any, pointer: str) -> Any:
    if not isinstance(pointer, str) or not pointer.startswith("/"):
        raise ValueError("historical JSON pointer must begin with /")
    result = document
    for token in pointer[1:].split("/"):
        token = token.replace("~1", "/").replace("~0", "~")
        result = result[int(token)] if isinstance(result, list) else result[token]
    return result


def incremental_catalog_body_sha256(catalog: dict[str, Any]) -> str:
    """Bind current source claims without recursively including their review."""
    body = json.loads(json.dumps(catalog, ensure_ascii=False))
    body.pop("audit_inputs", None)
    supplement = body.get("incremental_source_review")
    if isinstance(supplement, dict):
        supplement.pop("catalog_body_sha256", None)
    return hashlib.sha256(json.dumps(body, ensure_ascii=False, sort_keys=True,
                                    separators=(",", ":")).encode()).hexdigest()


def _incremental_source_review(root: Path, catalog: dict[str, Any], errors: list[str]) -> None:
    supplement = catalog.get("incremental_source_review")
    if supplement is None:
        return  # Retained historical/isolated fixtures keep their original contract.
    try:
        if (not isinstance(supplement, dict)
                or type(supplement.get("schema_version")) is not int
                or supplement["schema_version"] != 1
                or supplement.get("scope") != "source_only"
                or supplement.get("original_audit_date") != catalog["audit_date"]):
            raise ValueError("typed source-only supplement and original audit identity required")
        reviewed = supplement.get("review_date")
        if (not isinstance(reviewed, str) or date.fromisoformat(reviewed).isoformat() != reviewed
                or reviewed < catalog["audit_date"]):
            raise ValueError("valid incremental source review date required")
        body_hash = incremental_catalog_body_sha256(catalog)
        products = catalog.get("product_sources_sha256")
        if (supplement.get("catalog_body_sha256") != body_hash
                or type(supplement.get("product_source_count")) is not int
                or supplement["product_source_count"] != len(products)
                or type(supplement.get("semantic_record_count")) is not int
                or supplement["semantic_record_count"] != sum(len(catalog[k]) for k in ("modules", "components", "capabilities"))):
            raise ValueError("incremental source review body/count binding differs")
        product_hash = hashlib.sha256(json.dumps(products, ensure_ascii=False, sort_keys=True,
                                               separators=(",", ":")).encode()).hexdigest()
        proofs = []
        def unique(pairs):
            result = {}
            for key, value in pairs:
                if key in result:
                    raise ValueError("duplicate source-review JSON key")
                result[key] = value
            return result
        def finite_float(value):
            number = float(value)
            if not math.isfinite(number):
                raise ValueError("nonfinite source-review JSON")
            return number
        for ref in catalog.get("audit_inputs", []):
            path = safe_path(root, ref["path"])
            if path.suffix != ".json":
                continue
            # Historical input bytes keep their own schema and hash contract.
            # Only the newly claimed source-review proof gets this parser.
            try:
                text = path.read_text()
                preview = json.loads(text)
            except (UnicodeError, ValueError):
                continue
            if isinstance(preview, dict) and preview.get("kind") == "independent-implementation-source-audit/v1":
                value = json.loads(text, object_pairs_hook=unique,
                                   parse_float=finite_float,
                                   parse_constant=lambda value: (_ for _ in ()).throw(ValueError("nonfinite source-review JSON")))
                if digest(path) != ref.get("sha256"):
                    raise ValueError("independent source-review evidence hash differs")
                proofs.append(value)
        if len(proofs) != 1:
            raise ValueError("exactly one independently approved current source review required")
        proof = proofs[0]
        authors, reviewer = proof.get("author_ids"), proof.get("reviewer_id")
        if (type(proof.get("schema_version")) is not int or proof["schema_version"] != 1
                or proof.get("status") != "approved"
                or proof.get("independent_non_author_review") is not True
                or proof.get("blocking_findings") != []
                or not isinstance(authors, list) or not authors
                or any(not canonical_review_actor(actor) for actor in authors)
                or len(authors) != len(set(authors))
                or not canonical_review_actor(supplement.get("author"))
                or supplement.get("author") not in authors
                or not canonical_review_actor(reviewer) or reviewer in authors
                or proof.get("review_date") != reviewed
                or proof.get("catalog_body_sha256") != body_hash
                or proof.get("product_sources_sha256") != product_hash):
            raise ValueError("independent source review actor/body/product/date binding differs")
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append("Incremental source review invalid: " + str(error))


def _audit_provenance(root: Path, catalog: dict[str, Any], errors: list[str]) -> None:
    """Check retained artifact identities; language semantics remain human review."""
    audit_inputs = catalog.get("audit_inputs", [])
    if not isinstance(audit_inputs, list):
        errors.append("audit_inputs must be a path/hash list")
    else:
        for index, item in enumerate(audit_inputs):
            if not isinstance(item, dict):
                errors.append(f"audit_inputs[{index}] must be an evidence object")
            else:
                _evidence_path(root, item, f"audit_inputs[{index}]", errors)
    _incremental_source_review(root, catalog, errors)
    history = catalog.get("historical_evidence", [])
    _historical_evidence(root, history, "source_only", "catalog", errors)
    detail = catalog.get("historical_language_quality_detail")
    if detail is None:
        return
    if not isinstance(detail, dict):
        errors.append("historical_language_quality_detail must be an object")
        return
    try:
        if detail.get("status") not in ("historical_pass", "historical_mixed", "historical_failed"):
            raise ValueError("language detail status must remain historical")
        if detail.get("current_status") != "not_run":
            raise ValueError("language detail current_status must remain not_run")
        summary_path = safe_path(root, detail.get("path"))
        history_rows = history if isinstance(history, list) else []
        if not any(isinstance(row, dict) and row.get("path") == detail.get("path")
                   and row.get("sha256") == digest(summary_path) for row in history_rows):
            raise ValueError("language detail summary must bind to hashed historical evidence")
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        pointers, cases = detail.get("json_pointers"), detail.get("cases")
        if not isinstance(pointers, list) or not isinstance(cases, list) or not pointers or len(pointers) != len(cases):
            raise ValueError("language detail pointers and cases must be nonempty and paired")
        for pointer, case in zip(pointers, cases):
            if (not isinstance(case, dict) or not _nonempty(case.get("id"))
                    or pointer != "/quality_cases/" + case["id"]
                    or _json_pointer(summary, pointer) != case.get("status")):
                raise ValueError("language detail case/status differs from historical JSON pointer")
        manifest_path = summary_path.parent / "file-manifest.json"
        retained = json.loads(manifest_path.read_text(encoding="utf-8"))
        if not isinstance(retained, list) or not retained:
            raise ValueError("historical language file manifest must be a nonempty list")
        names = set()
        for item in retained:
            if not isinstance(item, dict) or not _nonempty(item.get("path")):
                raise ValueError("historical language file manifest row is invalid")
            relative = Path(item["path"])
            if relative.is_absolute() or ".." in relative.parts or item["path"] in names:
                raise ValueError("historical language file manifest paths must be unique and safe")
            names.add(item["path"])
            path = (summary_path.parent / relative).relative_to(root).as_posix()
            _evidence_path(root, {"path": path, "sha256": item.get("sha256")}, "language provenance", errors)
        output = safe_path(root, detail.get("actual_output"))
        if output.parent != summary_path.parent or output.name not in names or summary_path.name not in names:
            raise ValueError("language summary/stdout must belong to retained provenance manifest")
    except (OSError, ValueError, TypeError, KeyError, IndexError) as error:
        errors.append(f"Cannot verify historical language provenance: {error}")


def _record(root: Path, record: Any, group: str, index: int,
            authority: dict[str, set[str]], errors: list[str]) -> str | None:
    location = f"{group}[{index}]"
    if not isinstance(record, dict):
        errors.append(f"{location}: record must be an object")
        return None
    unknown = set(record) - RECORD_FIELDS
    if unknown:
        errors.append(f"{location}: unsupported record fields: {', '.join(sorted(unknown))}")
    identity = record.get("id")
    if not _nonempty(identity):
        errors.append(f"{location}: id must be nonempty")
        identity = None
    else:
        location = str(identity)
    for key in ("name", "current_behavior", "target_behavior", "next_step", "owner"):
        if not _nonempty(record.get(key)):
            errors.append(f"{location}: {key} must be nonempty")
    for key, allowed in (("implementation_status", IMPLEMENTATION),
                         ("alignment_status", ALIGNMENT), ("verification_status", VERIFICATION)):
        if not isinstance(record.get(key), str) or record[key] not in allowed:
            errors.append(f"{location}: invalid {key}: {record.get(key)!r}")
    if "visual_alignment_status" in record and record["visual_alignment_status"] != "not_assessed":
        errors.append(f"{location}: this source audit visual_alignment_status must be not_assessed")
    gap_required = (record.get("implementation_status") != "implemented"
                    or record.get("alignment_status") in ("partial", "mismatch"))
    if not isinstance(record.get("gap"), str) or gap_required and not _nonempty(record["gap"]):
        errors.append(f"{location}: incomplete or mismatched implementation requires a gap")
    known_requirements = authority["capabilities"] | authority["rw"]
    for field, known in (("requirement_ids", known_requirements),
                         ("module_ids", authority["modules"]),
                         ("component_ids", authority["components"])):
        references = record.get(field)
        if not isinstance(references, list) or any(not _nonempty(ref) for ref in references):
            errors.append(f"{location}: {field} must be a string list")
        elif len(references) != len(set(references)) or set(references) - known:
            errors.append(f"{location}: unknown or duplicate {field}: {references}")
    if group == "capabilities":
        requirements = record.get("requirement_ids")
        if (not isinstance(requirements, list)
                or not any(isinstance(reference, str) and reference in authority["capabilities"]
                           for reference in requirements)):
            errors.append(f"{location}: capability requires at least one known FP requirement")
    known_ids = known_requirements | authority["modules"] | authority["components"]
    for field in ("current_behavior", "target_behavior", "gap", "next_step"):
        text = record.get(field, "")
        if isinstance(text, str):
            unknown_ids = set(REFERENCE.findall(text)) - known_ids
            if unknown_ids:
                errors.append(f"{location}: unknown references in {field}: {sorted(unknown_ids)}")
    _source_evidence(root, record.get("source_evidence"), location, errors)
    _historical_evidence(root, record.get("historical_evidence"),
                         str(record.get("verification_status", "")), location, errors)
    return identity


def validate_catalog(root: Path, check_exports: bool = True) -> list[str]:
    """Check the inventory identity, authority closure and deterministic exports."""
    root = Path(root).resolve()
    errors: list[str] = []
    try:
        catalog = json.loads(safe_path(root, CATALOG).read_text(encoding="utf-8"))
        authority = _authority(root)
    except (OSError, ValueError, KeyError, TypeError, csv.Error) as error:
        return [f"Cannot read implementation status or authority: {error}"]
    if not isinstance(catalog, dict):
        return ["Implementation status catalog must be an object"]
    if type(catalog.get("schema_version")) is not int or catalog["schema_version"] != 1:
        errors.append("Implementation status schema_version must be 1")
    if catalog.get("audit_date") != "2026-10-20":
        errors.append("Implementation status audit_date must be 2026-10-20")
    if catalog.get("audit_scope") != "source_inspection_with_historical_evidence":
        errors.append("Implementation status audit_scope must separate source from historical evidence")
    for field in ("runtime_verified_this_audit", "current_release_approved"):
        if catalog.get(field) is not False:
            errors.append(f"{field} must be explicitly false for this source audit")
    _current_runtime_claims(catalog, errors)
    _current_artifact(root, catalog, errors)
    _audit_provenance(root, catalog, errors)
    frozen = catalog.get("product_sources_sha256")
    actual = product_source_identity(root)
    if not isinstance(frozen, dict) or not frozen:
        errors.append("product_sources_sha256 must freeze the complete nonempty product source set")
    elif any(not isinstance(path, str) or not isinstance(sha, str) or not SHA256.fullmatch(sha)
             for path, sha in frozen.items()):
        errors.append("product_sources_sha256 must map repository paths to lowercase SHA-256 digests")
    elif frozen != actual:
        changed = sorted(path for path in set(frozen) | set(actual) if frozen.get(path) != actual.get(path))
        errors.append("Product source closure drift: " + ", ".join(changed))
    for group in ("modules", "components", "capabilities"):
        records = catalog.get(group)
        if not isinstance(records, list):
            errors.append(f"{group} must be a list")
            continue
        identities = [_record(root, record, group, index, authority, errors)
                      for index, record in enumerate(records)]
        valid_ids = [identity for identity in identities if identity is not None]
        if len(valid_ids) != len(set(valid_ids)):
            errors.append(f"Duplicate {group} record ID")
        missing = authority[group] - set(valid_ids)
        extra = set(valid_ids) - authority[group]
        if group == "capabilities":
            extra = {identity for identity in extra if not CAPABILITY_ID.fullmatch(identity)}
        if missing or extra:
            errors.append(f"{group} authority closure mismatch: missing={sorted(missing)}, extra={sorted(extra)}")
    if check_exports and not errors:
        for relative, expected in render(root, catalog).items():
            path = safe_path(root, relative)
            if not path.is_file() or path.read_text(encoding="utf-8") != expected:
                errors.append(f"Implementation status export drift: {relative}; run --write after review")
    return errors


def _cell(value: Any) -> str:
    escaped = html.escape(str(value), quote=False)
    return escaped.replace("|", "\\|").replace("\r", "").replace("\n", "<br>")


def _link(relative: str, line: int | None = None) -> str:
    target = Path(os.path.relpath(relative, "docs/architecture")).as_posix()
    suffix = f":{line}" if line else ""
    return f"[{_cell(relative + suffix)}](<{target}{'#L' + str(line) if line else ''}>)"


def _sources(record: dict[str, Any]) -> str:
    links = []
    for item in record["source_evidence"]:
        relative = item["path"]
        target = Path(os.path.relpath(relative, "docs/architecture")).as_posix()
        href = html.escape(quote(target, safe="/") + f"#L{item['line']}", quote=True)
        label = _cell(f"{relative}:{item['line']}")
        links.append(f'<a href="{href}">{label}</a>')
    return (f"<details><summary>查看 {len(links)} 处源码</summary>"
            + "<br>".join(links) + "</details>")


def _history(record: dict[str, Any]) -> str:
    evidence = record["historical_evidence"]
    if not evidence:
        return "无历史执行回执；本轮仅源码核对"
    return "<br>".join(f"{_link(item['path'])}：{_cell(item['claim'])}（仅历史范围）" for item in evidence)


def _csv_export(root: Path, relative: str, group: str,
                catalog: dict[str, Any]) -> str:
    fields, rows = _csv(root, relative)
    fields.extend(field for field in EXPORT_FIELDS if field not in fields)
    identity_field = "module_id" if group == "modules" else "component_id"
    records = {record["id"]: (index, record) for index, record in enumerate(catalog[group])}
    for row in rows:
        index, record = records[row[identity_field]]
        row.update({
            "implementation_status": record["implementation_status"],
            "alignment_status": record["alignment_status"],
            "verification_status": record["verification_status"],
            "current_implementation": record["current_behavior"],
            "remaining_gap": record["gap"],
            "evidence_refs": ";".join(f"{item['path']}:{item['line']}@{item['sha256']}"
                                      for item in record["source_evidence"]),
            "status_record": f"implementation-status.json#/{group}/{index}",
        })
    stream = io.StringIO(newline="")
    writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return stream.getvalue()


def render(root: Path, catalog: dict[str, Any]) -> dict[str, str]:
    """Derive readable status and CSV columns without changing responsibility."""
    lines = [
        "# 当前实现与设计差距核对", "",
        (f"原始登记日期：{catalog['audit_date']}；本次源码复核日期：{catalog['incremental_source_review']['review_date']}。"
         if catalog.get("incremental_source_review") else f"核对日期：{catalog['audit_date']}。")
        + "机器登记见[implementation-status.json](implementation-status.json)。", "",
        "本清单只核对源码与历史证据，不包含本批另外执行的产品、游戏或模型实测；这些运行结果以各自绑定源码和包的独立回执为准，当前发布未获批准。", "",
        "“已实现”表示在登记的源码中存在对应实现，不等于已通过当前设计、完整功能或同包实机验收。"
        "“源码对齐”是人工核对结论，工具只校验结构、文件身份、行引用与导出一致性，不能自动理解业务。", "",
        "源码语义对齐不代表视觉对齐；本轮所有视觉对齐状态为未评估，当前实机验证均为未执行。", "",
        "现有包的路径、构建清单、EXE/归档字节和源身份只做只读交叉核对，不重做签名、发行或跨平台验收。"
        "语言历史只程序核对保留文件身份及已记录状态；observed、expected_semantics 和资源/源码比较说明仍属人工审阅 provenance，"
        "工具不自动判断语言语义或重演历史。", "",
        "## 状态图例", "",
        "- 实现状态：已实现 / 部分实现 / 尚未实现。",
        "- 设计对齐：源码对齐 / 部分对齐 / 与要求不一致 / 未评估。",
        "- 验证状态：仅源码核对 / 历史包范围通过 / 历史包结果不一 / 历史包范围失败 / 未执行 / 受阻。",
        "- 历史证据只适用于回执声明的源码、包和范围，不能继承为当前版本通过。", "",
        "## 首版功能核对", "",
        "| 功能 | 实现 / 设计 / 验证 | 当前源码行为 | 目标行为与尚存差距 | 源码证据 | Owner / 下一步 | 历史边界 |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for record in catalog["capabilities"]:
        state = " / ".join((IMPLEMENTATION[record["implementation_status"]],
                             ALIGNMENT[record["alignment_status"]],
                             VERIFICATION[record["verification_status"]])) + "<br>当前实机：未执行；视觉：未评估"
        gap = record["gap"] or "本次源码范围内未发现差距；仍须按计划验证"
        lines.append("| " + " | ".join((
            _cell(f"{record['id']} {record['name']}"), state,
            _cell(record["current_behavior"]),
            f"目标：{_cell(record['target_behavior'])}<br>差距：{_cell(gap)}",
            _sources(record), f"{_cell(record['owner'])}<br>{_cell(record['next_step'])}",
            _history(record),
        )) + " |")
    for group, title in (("modules", "模块状态"), ("components", "组件状态")):
        lines.extend(["", f"## {title}", "",
                      "| ID / 名称 | 实现 / 设计 / 验证 | 当前实现 | 尚存差距 | 源码证据 | Owner / 下一步 |",
                      "| --- | --- | --- | --- | --- | --- |"])
        for record in catalog[group]:
            state = " / ".join((IMPLEMENTATION[record["implementation_status"]],
                                 ALIGNMENT[record["alignment_status"]],
                                 VERIFICATION[record["verification_status"]])) + "<br>当前实机：未执行；视觉：未评估"
            lines.append("| " + " | ".join((
                _cell(f"{record['id']} {record['name']}"), state,
                _cell(record["current_behavior"]),
                _cell(record["gap"] or "本次源码范围内未发现差距；不代表产品验收"),
                _sources(record), f"{_cell(record['owner'])}<br>{_cell(record['next_step'])}",
            )) + " |")
    lines.extend(["", "## 维护与边界", "",
                  "修改机器登记后运行 `python3 scripts/quality/implementation_status.py --write` 显式更新本页和矩阵状态列。"
                  "默认 `--check` 只读校验，既不改实现状态，也不刷新迭代基线。",
                  "产品源码集合、证据文件 SHA-256、真实行号/整行引用、权威功能及模块组件全集均受校验。"
                  "产品源码变动后应重新人工核对，不能仅刷新哈希延续原结论。", ""])
    return {
        MARKDOWN: "\n".join(lines),
        MODULES: _csv_export(root, MODULES, "modules", catalog),
        COMPONENTS: _csv_export(root, COMPONENTS, "components", catalog),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="read-only validation (default)")
    mode.add_argument("--write", action="store_true", help="explicitly regenerate derived documents")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    errors = validate_catalog(root, check_exports=not arguments.write)
    if not errors and arguments.write:
        catalog = json.loads(safe_path(root, CATALOG).read_text(encoding="utf-8"))
        for relative, content in render(root, catalog).items():
            safe_path(root, relative).write_text(content, encoding="utf-8", newline="")
        errors = validate_catalog(root)
    print(json.dumps({"status": "failed" if errors else "passed", "errors": errors,
                      "mode": "write" if arguments.write else "check",
                      "runtime_verified_this_audit": False,
                      "current_release_approved": False}, ensure_ascii=False))
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
