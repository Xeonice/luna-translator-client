#!/usr/bin/env python3
"""Check the saved-service selection design and verify real browser evidence.

Static validation never writes sources. Browser execution is design-only and
requires a fresh child of ``root/.build``; product acceptance remains not_run.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
CATALOG = "docs/architecture/model-service-registry-design.json"
PRD = "docs/product/model-configuration-prd.md"
TECHNICAL = "docs/architecture/japanese-study-and-llm.md"
TEST_PLAN = "docs/development/translation-service-selection-tests.md"
DESIGN_ROOT = "docs/design/first-product-v1"
CANDIDATE_ID = "translation-service-selection-20261030"
CANDIDATE = DESIGN_ROOT + "/candidates/" + CANDIDATE_ID
DESIGN = CANDIDATE + "/index.html"
MODULE = CANDIDATE + "/service-picker.js"
MANIFEST = CANDIDATE + "/manifest.json"
CHECKER = "scripts/quality/translation_service_selection.py"
TESTS = "scripts/quality/test_translation_service_selection.py"
RUNNER = "scripts/report/verify_translation_service_selection.cjs"
OLD_CHECKER = "scripts/quality/offline_translation_design.py"
SS_IDS = {f"SS-{number:02d}" for number in range(1, 13)}
SHA256 = re.compile(r"[0-9a-f]{64}")


def _load_helpers():
    """Compile the unchanged sibling helper without writing import caches."""
    path = Path(__file__).resolve().with_name("offline_translation_design.py")
    spec = importlib.util.spec_from_file_location("selection_offline_helpers", path)
    if spec is None or spec.loader is None:
        raise ValueError("Cannot load existing offline design helpers")
    module = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    return module


helpers = _load_helpers()
_safe = helpers._safe
_load_json = helpers._load_json
_output_path = helpers._output_path
_text = helpers._text
_strings = helpers._strings
_positive = helpers._positive


def validate_catalog(catalog: object) -> list[str]:
    """Validate the candidate contract; malformed JSON shapes return errors."""
    if not isinstance(catalog, dict):
        return ["Catalog must be a JSON object"]
    errors = helpers.validate_catalog(catalog)

    def obj(parent: dict, field: str) -> dict:
        value = parent.get(field)
        if not isinstance(value, dict):
            errors.append(f"{field}: JSON object required")
            return {}
        return value

    def fields(actual: dict, expected: dict, label: str) -> None:
        for key, value in expected.items():
            if type(actual.get(key)) is not type(value) or actual.get(key) != value:
                errors.append(f"{label}.{key}: must remain {value!r}")

    selection = obj(catalog, "service_selection_design")
    fields(selection, {
        "change_id": "design-translation-service-selection",
        "phase": "design_documentation_only",
        "user_decision": "统一浅层选择翻译服务；配置在别处管理，没有配置好不能选",
        "decision_scope_confirmed": True,
        "candidate_design": DESIGN + "?scenario=reading",
        "candidate_design_status": "pending_review",
        "source_documents": [PRD + "#translation-service-selection-prd",
                             TECHNICAL + "#translation-service-selection-contract",
                             "docs/architecture/first-product-engineering-baseline.md", TEST_PLAN],
        "configuration_management": ["add", "edit", "save", "check_local_conditions", "test_explicit_sample"],
    }, "service_selection_design")
    if not _text(selection.get("version")):
        errors.append("service_selection_design.version: explicit version required")
    fields(obj(selection, "daily_selection"), {
        "entrypoints": ["current_session_shallow", "hud_shared_picker"],
        "purpose": "translation",
        "service_families": ["cloud_translation", "local_non_llm_translation", "llm_translation"],
        "source": "current_profile_added_saved_immutable_resources_only",
        "template_catalog": "add_service_only",
        "draft_policy": "unsaved_drafts_never_selectable_or_read_as_saved",
        "eligibility": ["added", "saved_revision", "local_fields_and_credential_reference_valid",
                        "explicit_translation_purpose_supported", "not_suspended",
                        "no_known_failure_for_selected_revision", "required_local_runtime_and_resource_gates_pass"],
        "llm_filter": "templatePurpose === translation and explicit translation capability; grammar-only excluded without name inference",
        "http_untested": "valid saved supported HTTP configuration may be explicitly selected as unverified; untested is not unconfigured",
        "local_unknown": "unknown required system/resource/language/runtime_route health blocks selection; offline gate unchanged",
        "known_failure": "current resource/configuration revision known failure blocks selection with reason and recovery",
        "no_implicit_fallback": True,
    }, "daily_selection")
    fields(obj(selection, "projection_contract_candidate"), {
        "authority": "Rust app-core derives configuration eligibility and saved service projection; UI consumes DTO/application-port through narrow Tauri only",
        "item_fields": ["id", "name", "family", "available", "status", "reason", "revision", "configVersion"],
        "selection_fields": ["scope", "source", "mainId", "extraIds", "revision"],
        "prototype_api": ["list", "selection", "open", "choose", "setScope", "inheritGame", "configure", "returnToPicker"],
        "prototype_only": True, "ui_direct_http_sql_ffi": False, "installed_production_contract": False,
    }, "projection_contract_candidate")
    fields(obj(selection, "binding_contract_candidate"), {
        "command": "BindingCommand activate", "command_status": "candidate_not_implemented",
        "resource_reference": "saved immutable model_config or offline_engine_config revision plus frozen dependencies",
        "save_changes_current_selection": False, "test_changes_current_selection": False,
        "select_changes_purpose_enabled": False,
        "selection_effect": "create and activate new purpose binding; subsequent sentences only",
        "current_sentence_retranslation": "independent explicit command; previous result retained",
        "enable_disable": "separate explicit automatic_translation control, validate current binding before enabling",
        "scopes": ["global_default", "game_override", "restore_inheritance"],
        "optimistic_concurrency": "expected binding revision and profile generation; revalidate before activate, keep old binding on rejection",
        "main_and_comparison": "one main, ordered comparisons, deduplicate stable resource identity; moving to main removes duplicate comparison",
        "cancellation": "cancel superseded queue/in-flight tasks; reject stale profile/binding/task/attempt/unit/resource callbacks before current result/cache/history commit",
        "history": "preserve existing original sentence/service/version results; cancelled late results cannot write success",
        "configure_return_context": ["origin", "scope", "gameId", "purpose", "resourceId"],
        "return_behavior": "refresh saved list and return to original picker; no automatic selection, test, send or retranslation",
    }, "binding_contract_candidate")
    fields(obj(selection, "scope_boundaries"), {
        "http_protocols": 13, "http_presets": 21, "offline_engines": 4,
        "original_PA_acceptance": 12, "original_OT_acceptance": 16,
        "original_arrays_and_matrix_unchanged": True,
        "database": "no new DDL; existing candidate field schemes remain unapproved and unchanged",
        "production_source_or_package_changed": False,
        "approved_png_tokens_and_prior_candidates_changed": False, "product_acceptance": "not_run",
    }, "scope_boundaries")
    rows = catalog.get("selection_acceptance")
    if not isinstance(rows, list) or not rows or any(not isinstance(row, dict) for row in rows):
        errors.append("selection_acceptance: nonempty object list required")
        rows = []
    identities = [row.get("id") for row in rows]
    if not _strings(identities) or len(rows) != 12 or set(identities) != SS_IDS:
        errors.append("selection_acceptance must contain every unique SS-01 through SS-12")
    for row in rows:
        identity = row.get("id") if _text(row.get("id")) else "<invalid>"
        if row.get("status") != "not_run":
            errors.append(f"{identity}: product status must remain not_run")
        for key in ("label", "assertion", "maintenance"):
            if not _text(row.get(key)):
                errors.append(f"{identity}: traceable {key} required")
        layers = row.get("verification_layers")
        if not _strings(layers) or "design" not in layers or len(layers) < 2:
            errors.append(f"{identity}: design and future real verification_layers required")
        for key, pattern, limit in (("source_requirement_ids", r"FP-(\d{2})", 12),
                                    ("source_configuration_requirement_ids", r"AI-CFG-(\d{2})", 16)):
            values = row.get(key)
            if not _strings(values) or any(not (match := re.fullmatch(pattern, value))
                                           or not 1 <= int(match[1]) <= limit for value in values):
                errors.append(f"{identity}: valid nonempty unique {key} required")
    return errors


def _references(body: str) -> list[str]:
    parser = helpers._DesignReferences()
    parser.feed(body)
    if parser.bases != ["../../"]:
        raise ValueError("Selection candidate requires one ../../ shared-design base")
    if len(parser.assets) != len(set(parser.assets)):
        raise ValueError("Selection candidate cannot load duplicate assets")
    for asset in parser.assets:
        url = urlsplit(asset)
        path = Path(unquote(url.path))
        if (url.scheme or url.netloc or url.query or url.fragment or path.is_absolute()
                or ".." in path.parts or not _text(asset)):
            raise ValueError(f"Candidate design asset must stay local: {asset}")
    return parser.assets


def _manifest_sources(root: Path, manifest: object, assets: list[str]) -> set[str]:
    """Check the bounded dependency manifest and return its validated sources."""
    if not isinstance(manifest, dict):
        raise ValueError("Selection manifest must be a JSON object")
    for field, expected in (("schema_version", 1), ("candidate_id", CANDIDATE_ID),
                            ("entry", DESIGN), ("status", "pending_review"), ("runtime_base_href", "../../")):
        if type(manifest.get(field)) is not type(expected) or manifest.get(field) != expected:
            raise ValueError(f"Selection manifest {field} must remain {expected!r}")
    if not _text(manifest.get("simulation_boundary")):
        raise ValueError("Selection manifest must identify its simulation boundary")
    owned = manifest.get("owned_sources")
    shared = manifest.get("shared_dependencies")
    if not isinstance(owned, dict) or not owned or not isinstance(shared, dict) or not shared:
        raise ValueError("Selection manifest requires owned_sources and shared_dependencies hashes")
    actual = {path.name for path in (root / CANDIDATE).iterdir() if path.is_file() and path.name != "manifest.json"}
    if set(owned) != actual or not {"index.html", "service-picker.js", "service-picker.css"}.issubset(owned):
        raise ValueError("Selection manifest must identify every candidate top-level source exactly")
    paths: set[str] = set()
    for group, prefix in ((owned, CANDIDATE), (shared, DESIGN_ROOT)):
        for relative, digest in group.items():
            if not _text(relative) or not isinstance(digest, str) or not SHA256.fullmatch(digest):
                raise ValueError("Selection manifest requires safe paths and SHA-256 strings")
            if group is owned and len(Path(relative).parts) != 1:
                raise ValueError("Owned selection source must be a top-level file")
            source = prefix + "/" + relative
            if hashlib.sha256(_safe(root, source).read_bytes()).hexdigest() != digest:
                raise ValueError(f"Selection manifest source identity mismatch: {source}")
            paths.add(source)
    required = {DESIGN_ROOT + "/" + asset for asset in assets}
    if not required.issubset(paths):
        raise ValueError("Selection manifest must cover every loaded design dependency")
    return paths


def check(root: Path = ROOT) -> list[str]:
    """Perform read-only contract, traceability, old-boundary and asset checks."""
    root = Path(root).resolve()
    errors: list[str] = []
    try:
        catalog = _load_json(_safe(root, CATALOG))
        errors.extend(validate_catalog(catalog))
        errors.extend(helpers.check(root))
        documents = {name: _safe(root, name).read_text(encoding="utf-8")
                     for name in (PRD, TECHNICAL, TEST_PLAN, DESIGN, MODULE)}
        if '<a id="translation-service-selection-prd"></a>' not in documents[PRD] or "## 8." not in documents[PRD]:
            errors.append("Selection PRD chapter 8 and real translation-service-selection-prd anchor required")
        if ('<a id="translation-service-selection-contract"></a>' not in documents[TECHNICAL]
                or any(f"### 5.{number} " not in documents[TECHNICAL] for number in (19, 20, 21))):
            errors.append("Selection technical contract anchor and sections 5.19 through 5.21 required")
        rows = re.findall(r"^\|\s*(SS-\d{2})\s*\|.*$", documents[TEST_PLAN], re.M)
        if len(rows) != 12 or set(rows) != SS_IDS:
            errors.append("Selection test plan must contain exactly one row for every SS-01 through SS-12")
        full_rows = re.findall(r"^\|\s*SS-\d{2}\s*\|.*$", documents[TEST_PLAN], re.M)
        if any(not re.search(r"[;；]\s*not_run\s*\|\s*$", row) for row in full_rows):
            errors.append("Selection test plan product statuses must all remain not_run")
        for name in (PRD, TECHNICAL, TEST_PLAN):
            if CANDIDATE_ID not in documents[name] or "pending_review" not in documents[name]:
                errors.append(f"{name}: current pending-review selection candidate trace required")
        assets = _references(documents[DESIGN])
        required = {"material-tokens.css", "styles.css", "settings.css", "controls.css", "controls.js",
                    "icons.js", "data.js", "app.js", "learning.js", "game-library.js", "review-gaps.js"}
        # Explicitly bind the extension and its existing local configuration dependency.
        required.update("candidates/" + CANDIDATE_ID + "/" + name for name in
                        ("settings.js", "local-translation-design.js", "local-translation-design.css",
                         "service-picker.js", "service-picker.css"))
        if not required.issubset(assets):
            errors.append("Selection candidate must reuse the shared material, controls and complete design")
        for asset in assets:
            _safe(root, DESIGN_ROOT + "/" + asset)
        if "待评审" not in documents[DESIGN] or "LunaTranslationServices" not in documents[MODULE]:
            errors.append("Selection candidate must identify pending review and expose its prototype API")
        for api in ("list", "selection", "open", "choose", "setScope", "inheritGame", "configure", "returnToPicker"):
            if not re.search(r"\b" + api + r"\b", documents[MODULE]):
                errors.append(f"Selection prototype missing {api} API")
        _manifest_sources(root, _load_json(_safe(root, MANIFEST)), assets)
    except (OSError, ValueError, TypeError, KeyError, UnicodeError, AttributeError) as error:
        errors.append(f"Translation service selection design cannot be checked: {error}")
    return errors


def _source_hashes(root: Path) -> dict[str, str]:
    """Hash explicit documents plus the bounded new candidate dependency closure."""
    paths = {CATALOG, PRD, TECHNICAL, TEST_PLAN, CHECKER, TESTS, RUNNER, OLD_CHECKER,
             helpers.PRD, helpers.TECHNICAL, helpers.TEST_PLAN, helpers.DESIGN, helpers.MODULE,
             helpers.LEGACY_CHECK, MANIFEST, "openspec/technology-lock.json",
             "docs/development/iteration-baseline.json"}
    # Hash actual bytes independently from manifest assertions. A changed source
    # must produce an after identity even when its old manifest hash is stale.
    paths.update(str(path.relative_to(root)) for path in (root / CANDIDATE).iterdir() if path.is_file())
    manifest = _load_json(_safe(root, MANIFEST))
    if not isinstance(manifest, dict):
        raise ValueError("Selection source manifest must remain an object")
    for field, prefix in (("owned_sources", CANDIDATE), ("shared_dependencies", DESIGN_ROOT)):
        group = manifest.get(field)
        if not isinstance(group, dict):
            raise ValueError(f"Selection source manifest {field} must remain an object")
        paths.update(prefix + "/" + relative for relative in group)
    catalog = _load_json(_safe(root, CATALOG))
    if isinstance(catalog, dict):
        paths.update(catalog.get("source_documents", []))
    paths.update(relative for _, _, relative in helpers._legacy(root).DOCS)
    return {relative: hashlib.sha256(_safe(root, relative).read_bytes()).hexdigest() for relative in sorted(paths)}


def _adapter_report(destination: Path) -> tuple[dict, list[str]]:
    """Independently verify named scenarios and PNG identities, never exit alone."""
    path = _safe(destination, "report.json")
    report = _load_json(path)
    if not isinstance(report, dict):
        return {}, ["Node/Playwright report must be a JSON object"]
    errors: list[str] = []
    if (type(report.get("schema_version")) is not int or report.get("schema_version") != 1
            or report.get("kind") != "translation_service_selection_design_verification"
            or report.get("layer") != "design" or report.get("status") != "passed"
            or report.get("product_tests_executed") is not False):
        errors.append("Node/Playwright report must identify a passed selection design-only execution")
    if not _positive(report.get("assertions")) or report["assertions"] < len(SS_IDS):
        errors.append("Node/Playwright report must contain actual assertions for all twelve scenarios")
    checks = report.get("checks")
    if not isinstance(checks, list) or not checks or any(not isinstance(row, dict) for row in checks):
        errors.append("Node/Playwright report requires unique passed SS scenario checks")
        checks = []
    ids = [row.get("id") for row in checks]
    if (not _strings(ids) or not SS_IDS.issubset(ids)
            or any(row.get("status") != "passed" for row in checks)):
        errors.append("Node/Playwright report must pass every SS-01 through SS-12 exactly once")
    if report.get("page_errors") != [] or report.get("network_requests") != []:
        errors.append("Browser errors or external network requests reject design acceptance")
    screenshots = report.get("screenshots")
    if not isinstance(screenshots, list) or not screenshots or any(not isinstance(row, dict) for row in screenshots):
        errors.append("Node/Playwright report requires actual screenshot evidence")
        screenshots = []
    if not _strings([row.get("path") for row in screenshots]):
        errors.append("Node/Playwright screenshot paths must be unique strings")
    for row in screenshots:
        try:
            relative = row.get("path")
            if not _text(relative) or Path(relative).suffix.lower() != ".png":
                raise ValueError("Screenshot evidence must name a local PNG")
            data = _safe(destination, relative).read_bytes()
            if (not _positive(row.get("bytes")) or row.get("bytes") != len(data)
                    or row.get("sha256") != hashlib.sha256(data).hexdigest()):
                raise ValueError(f"Screenshot byte/hash identity mismatch: {relative}")
            if (len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR"
                    or not _positive(row.get("width")) or not _positive(row.get("height"))
                    or row["width"] != int.from_bytes(data[16:20], "big")
                    or row["height"] != int.from_bytes(data[20:24], "big")):
                raise ValueError(f"Screenshot PNG dimensions mismatch: {relative}")
        except (OSError, ValueError, TypeError) as error:
            errors.append(str(error))
    return {"path": "report.json", "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "assertions": report.get("assertions"), "checks": len(checks)}, errors


def execute(root: Path, output: Path) -> dict:
    """Run Node/Playwright and retain a new design-only receipt even on failure."""
    destination = _output_path(Path(root), Path(output))
    root = Path(root).resolve()
    errors = check(root)
    destination.mkdir(parents=True, exist_ok=False)
    receipt = {"schema_version": 1, "kind": "translation_service_selection_design_execution",
               "layer": "design_only", "product_acceptance": "not_run", "candidate_design_status": "pending_review",
               "status": "failed", "started_at": datetime.now(timezone.utc).isoformat(),
               "root": str(root), "output": str(destination), "static_errors": errors,
               "command": None, "exit_code": None, "source_hashes_before": {}, "source_hashes_after": {}, "errors": []}
    stdout_path, stderr_path = destination / "stdout.log", destination / "stderr.log"
    stdout_path.touch()
    stderr_path.touch()
    try:
        if not errors:
            before = _source_hashes(root)
            receipt["source_hashes_before"] = before
            command = ["node", str(_safe(root, RUNNER)), str(root), str(destination)]
            receipt["command"] = command
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                try:
                    completed = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr, check=False, timeout=300)
                    receipt["exit_code"] = completed.returncode
                except subprocess.TimeoutExpired:
                    receipt["errors"].append("Node/Playwright selection adapter exceeded the 300-second execution budget")
                    receipt["exit_code"] = 124
                except OSError as error:
                    receipt["errors"].append(f"Cannot execute Node/Playwright selection adapter: {error}")
                    receipt["exit_code"] = 127
            after = _source_hashes(root)
            receipt["source_hashes_after"] = after
            if before != after:
                receipt["errors"].append("Controlled source bytes changed during execution")
            receipt["static_errors_after"] = check(root)
            if receipt["exit_code"] != 0:
                receipt["errors"].append("Node/Playwright selection adapter returned a nonzero exit code")
            try:
                identity, report_errors = _adapter_report(destination)
                receipt["adapter_report"] = identity
                receipt["errors"].extend(report_errors)
            except (OSError, ValueError, TypeError, UnicodeError) as error:
                receipt["errors"].append(f"Cannot verify Node/Playwright evidence: {error}")
            if not receipt["errors"] and not receipt["static_errors_after"]:
                receipt["status"] = "passed"
    except (OSError, ValueError, TypeError, KeyError, UnicodeError) as error:
        receipt["errors"].append(str(error))
    receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
    for label, path in (("stdout", stdout_path), ("stderr", stderr_path)):
        receipt[label] = {"path": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    (destination / "execution.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--execute", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    if args.execute != (args.output is not None):
        parser.error("--execute requires --output; --output is valid only with --execute")
    try:
        if args.execute:
            result = execute(args.root, args.output)
        else:
            errors = check(args.root)
            result = {"kind": "translation_service_selection_design_static", "layer": "design_only",
                      "status": "failed" if errors else "passed", "errors": errors,
                      "product_acceptance": "not_run", "candidate_design_status": "pending_review"}
    except (OSError, ValueError) as error:
        result = {"status": "failed", "layer": "design_only", "product_acceptance": "not_run", "errors": [str(error)]}
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"Translation service selection design: {result['status']} (design only; product acceptance not_run)")
        for error in result.get("errors", []) + result.get("static_errors", []):
            print(f"- {error}")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
