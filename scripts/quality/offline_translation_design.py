#!/usr/bin/env python3
"""Validate offline-translation design sources and run their browser adapter.

``--check --json`` is read-only. ``--execute`` records a real Node/Playwright
design run in a new .build directory; neither mode approves product behavior.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
from html.parser import HTMLParser
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
TEST_PLAN = "docs/development/local-offline-translation-tests.md"
DESIGN_ROOT = "docs/design/first-product-v1"
CANDIDATE = DESIGN_ROOT + "/candidates/local-offline-20261030"
DESIGN = CANDIDATE + "/index.html"
MODULE = CANDIDATE + "/local-translation-design.js"
LEGACY_CHECK = "scripts/report/build_model_service_design_report.py"
RUNNER = "scripts/report/verify_local_offline_design.cjs"
ENGINE_BACKENDS = {
    "apple_translation": "platform_native",
    "argos_translate": "local_worker",
    "jbeijing": "local_worker",
    "kingsoft_fastait": "local_worker",
}
ENGINE_FAMILIES = {
    "apple_translation": "system_traditional_translation",
    "argos_translate": "dedicated_nmt",
    "jbeijing": "proprietary_local_translation",
    "kingsoft_fastait": "proprietary_local_translation",
}
BACKENDS = {"http", "platform_native", "local_worker"}
OT_IDS = {f"OT-{number:02d}" for number in range(1, 17)}
COMMON_FIELDS = {
    "engine_id", "engine_contract_revision", "display_name", "source_language",
    "target_language", "route_policy", "draft_revision", "applied_revision",
}
TEST_IDENTITY = {
    "profile_id", "config_revision", "engine_id", "engine_contract_revision",
    "execution_backend", "language_pair", "resource_snapshot_id", "route",
    "request_semantics_hash",
}
ENGINE_FIELDS = {
    "apple_translation": {"system_api_status", "traditional_strategy_status",
                          "language_pair_availability", "resource_observation_epoch"},
    "argos_translate": {"runtime_route_status", "resource_location", "package_versions",
                        "package_integrity", "tokenizer_version", "sentence_boundary_version",
                        "route_policy"},
    "jbeijing": {"installation_path", "detected_version", "engine_architecture",
                 "worker_architecture", "execution_environment", "installation_resource_fingerprint"},
    "kingsoft_fastait": {"installation_path", "detected_version", "engine_architecture",
                        "worker_architecture", "execution_environment", "installation_resource_fingerprint"},
}
REQUIRED_FORBIDDEN_FIELDS = {"endpoint", "api_key"}


def _text(value: object) -> bool:
    return (isinstance(value, str) and bool(value.strip())
            and not any(ord(character) < 32 for character in value))


def _strings(value: object) -> bool:
    return (isinstance(value, list) and bool(value)
            and all(_text(item) for item in value) and len(value) == len(set(value)))


def _positive(value: object) -> bool:
    return type(value) is int and value > 0


def _field_key(value: str) -> str:
    return re.sub(r"[^a-z0-9]", "", value.lower())


def validate_catalog(catalog: object) -> list[str]:
    """Return contract violations, including malformed JSON shapes, without I/O."""
    if not isinstance(catalog, dict):
        return ["Catalog must be a JSON object"]
    errors: list[str] = []

    def rows(name: str) -> list[dict]:
        value = catalog.get(name)
        if not isinstance(value, list) or not value or any(not isinstance(row, dict) for row in value):
            errors.append(f"{name}: nonempty object list required")
            return []
        identities = [row.get("id") for row in value]
        if not _strings(identities):
            errors.append(f"{name}: nonempty unique string IDs required")
        return value

    if type(catalog.get("schema_version")) is not int or catalog.get("schema_version") != 1:
        errors.append("schema_version must be integer 1")
    if catalog.get("kind") != "model_service_registry_design" or catalog.get("phase") != "design_documentation_only":
        errors.append("Catalog must remain a design_documentation_only model-service registry")
    backends, engines, acceptance = (rows(name) for name in
                                    ("execution_backends", "offline_engines", "offline_acceptance"))
    if {row.get("id") for row in backends if _text(row.get("id"))} != BACKENDS or len(backends) != 3:
        errors.append("execution_backends must contain exactly http/platform_native/local_worker")
    for row in backends:
        identity = str(row.get("id"))
        if not _positive(row.get("contract_revision")):
            errors.append(f"backend {identity}: positive integer contract_revision required")
        if not _text(row.get("boundary")) or not _strings(row.get("catalog_source")):
            errors.append(f"backend {identity}: boundary and catalog_source required")
        expected = ["protocols", "presets"] if identity == "http" else ["offline_engines"]
        if row.get("catalog_source") != expected or row.get("acceptance_status") != "not_run":
            errors.append(f"backend {identity}: catalog source or not_run boundary changed")
        expected_status = "existing_sources_require_registry_migration" if identity == "http" else "planned"
        if row.get("implementation_status") != expected_status:
            errors.append(f"backend {identity}: implementation_status must remain {expected_status}")
    identities = {row.get("id") for row in engines if _text(row.get("id"))}
    if identities != set(ENGINE_BACKENDS) or len(engines) != 4:
        errors.append("offline_engines must contain exactly the four authorized engine IDs")
    for row in engines:
        identity = row.get("id") if _text(row.get("id")) else "<invalid>"
        if row.get("execution_backend") != ENGINE_BACKENDS.get(identity):
            errors.append(f"engine {identity}: incorrect execution_backend")
        if row.get("engine_family") != ENGINE_FAMILIES.get(identity):
            errors.append(f"engine {identity}: non-LLM engine_family must preserve its explicit classification")
        if not _positive(row.get("engine_contract_revision")):
            errors.append(f"engine {identity}: positive integer engine_contract_revision required; bool is invalid")
        for key, value in {
            "model_identity": "not_applicable", "allowed_purposes": ["translation"],
            "implementation_status": "planned", "adapter_acceptance": "not_run",
            "live_acceptance": "not_run", "distribution_status": "not_approved",
            "default_language_route": "direct_only",
            "network_fallback": "none_unless_separately_explicitly_configured",
        }.items():
            if row.get(key) != value:
                errors.append(f"engine {identity}: {key} must remain {value!r}")
        if row.get("probe_is_not_translation_acceptance") is not True:
            errors.append(f"engine {identity}: probe must not count as translation acceptance")
        for field in ("label", "engine_family", "runtime_route", "resource_policy", "license_status", "language_support"):
            if not _text(row.get(field)):
                errors.append(f"engine {identity}: {field} is required")
        runtime_route = row.get("runtime_route")
        if not _text(runtime_route) or not runtime_route.startswith("candidate_"):
            errors.append(f"engine {identity}: runtime_route must remain a candidate rather than an approved route")
        if not _strings(row.get("supported_platforms")):
            errors.append(f"engine {identity}: explicit candidate platforms required")
        for key, required in (("required_common_fields", COMMON_FIELDS),
                              ("test_identity", TEST_IDENTITY),
                              ("required_engine_fields", ENGINE_FIELDS.get(identity, set()))):
            actual = row.get(key)
            if not _strings(actual) or not required.issubset(actual):
                errors.append(f"engine {identity}: incomplete or malformed {key}")
        forbidden = row.get("forbidden_required_fields")
        if (not _strings(forbidden) or not REQUIRED_FORBIDDEN_FIELDS.issubset(forbidden)
                or not any(value in forbidden for value in ("model_id", "llm_model_id"))):
            errors.append(f"engine {identity}: forbidden_required_fields must exclude URL/Key/model identity")
        fake = {"url", "baseurl", "endpoint", "apikey", "key", "model", "modelid", "llmmodelid",
                "prompt", "temperature", "credentials", "credentialid", "connectionid", "protocol",
                "accesstoken", "authtoken", "apitoken", "accesskey", "password", "secretkey"}
        if _strings(forbidden):
            fake.update(_field_key(value) for value in forbidden)
        for key in ("required_common_fields", "required_engine_fields"):
            fields = row.get(key)
            if _strings(fields) and any(_field_key(field) in fake for field in fields):
                errors.append(f"engine {identity}: {key} contains a fake HTTP/LLM or forbidden field")
        if identity == "apple_translation":
            policy = row.get("non_llm_policy")
            terms = ("lowLatency", "highFidelity/Apple Intelligence excluded",
                     "unknown route not executable as non-LLM")
            if not _text(policy) or not all(term in policy for term in terms):
                errors.append("Apple non_llm_policy must explicitly exclude Intelligence/highFidelity and unknown routes")
            if not _strings(forbidden) or "installation_path" not in forbidden:
                errors.append("Apple must not require an installation_path")
            availability = row.get("minimum_api_availability")
            if not isinstance(availability, dict) or any(not _text(availability.get(key)) for key in (
                "translation_session_and_prepare_translation", "installed_source_initializer_and_session_cancel",
                "explicit_low_latency_strategy", "availability_source",
            )):
                errors.append("Apple minimum_api_availability must retain version-specific capability evidence")
        if identity == "argos_translate":
            if row.get("production_python_approved") is not False:
                errors.append("Argos production Python must remain explicitly unapproved")
            routing = row.get("routing_policy")
            if not _text(routing) or not all(term in routing for term in (
                "direct_only default", "explicitly selected and applied pivot", "never invoke upstream automatic pivot silently",
            )):
                errors.append("Argos routing_policy must prohibit implicit pivot and require explicit applied paths")
            if not _text(row.get("pipeline_policy")):
                errors.append("Argos complete pipeline policy is required")
    if len(acceptance) != 16 or {row.get("id") for row in acceptance if _text(row.get("id"))} != OT_IDS:
        errors.append("offline_acceptance must contain every unique OT-01 through OT-16")
    for row in acceptance:
        identity = str(row.get("id"))
        if row.get("status") != "not_run":
            errors.append(f"{identity}: product acceptance must remain not_run")
        for field in ("label", "assertion", "verification_layer", "maintenance"):
            if not _text(row.get(field)):
                errors.append(f"{identity}: traceable {field} required")
        for field, pattern, limit in (("source_requirement_ids", r"FP-(\d{2})", 12),
                                      ("source_configuration_requirement_ids", r"AI-CFG-(\d{2})", 16)):
            values = row.get(field)
            if not _strings(values) or any(not (match := re.fullmatch(pattern, value))
                                           or not 1 <= int(match[1]) <= limit for value in values):
                errors.append(f"{identity}: valid nonempty unique {field} required")
    counts = catalog.get("offline_counts")
    expected_counts = {"execution_backends": 3, "engines": 4, "acceptance_definitions": 16}
    if not isinstance(counts, dict) or any(type(counts.get(key)) is not int or counts.get(key) != value
                                            for key, value in expected_counts.items()):
        errors.append("offline_counts must match actual 3 backends/4 engines/16 acceptance definitions")
    scope = catalog.get("offline_scope")
    if not isinstance(scope, dict):
        errors.append("offline_scope object required")
    else:
        if (scope.get("change_id") != "design-local-offline-translation"
                or scope.get("http_inventory_unchanged") is not True
                or scope.get("candidate_design_status") != "pending_review"
                or scope.get("candidate_design") != DESIGN + "?scenario=settings"
                or scope.get("allowed_purposes") != ["translation"]
                or scope.get("excluded_purposes") != ["grammar", "context_word"]):
            errors.append("offline_scope must preserve design-only, pending-review, translation-only and HTTP boundaries")
    contracts = catalog.get("offline_runtime_contract_candidates")
    if (not isinstance(contracts, dict) or contracts.get("production_runtime_status") != "not_implemented"
            or contracts.get("product_acceptance") != "not_run"):
        errors.append("offline runtime remains not_implemented and product acceptance not_run")
    return errors


def _safe(root: Path, relative: str) -> Path:
    root = Path(root).resolve()
    if not _text(relative) or Path(relative).is_absolute() or ".." in Path(relative).parts:
        raise ValueError(f"Unsafe source path: {relative!r}")
    target = root / relative
    for component in (target, *target.parents):
        if component == root:
            break
        if component.is_symlink():
            raise ValueError(f"Source symlink forbidden: {relative}")
    if not target.resolve().is_relative_to(root) or not target.is_file():
        raise ValueError(f"Missing source: {relative}")
    return target


def _load_json(path: Path) -> object:
    def unique(pairs: list[tuple[str, object]]) -> dict:
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"Duplicate JSON key: {key}")
            result[key] = value
        return result

    def reject_constant(value: str) -> None:
        raise ValueError(f"Non-JSON constant: {value}")

    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique, parse_constant=reject_constant)


def _legacy(root: Path):
    source = _safe(root, LEGACY_CHECK)
    spec = importlib.util.spec_from_file_location("offline_existing_model_service_check", source)
    if spec is None or spec.loader is None:
        raise ValueError("Cannot load the existing HTTP documentation checker")
    module = importlib.util.module_from_spec(spec)
    # Compile source directly so a read-only --check cannot create __pycache__.
    exec(compile(source.read_bytes(), str(source), "exec"), module.__dict__)
    return module


class _DesignReferences(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.bases: list[str] = []
        self.assets: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = dict(attrs)
        if tag == "base":
            self.bases.append(attributes.get("href") or "")
        if tag == "script" or (tag == "link" and attributes.get("rel") == "stylesheet"):
            self.assets.append(attributes.get("src" if tag == "script" else "href") or "")


def check(root: Path = ROOT) -> list[str]:
    """Read and validate the catalog, traceability and shared design references."""
    root = Path(root).resolve()
    errors: list[str] = []
    try:
        catalog = _load_json(_safe(root, CATALOG))
        errors.extend(validate_catalog(catalog))
        errors.extend(_legacy(root).check(root))
        documents = {name: _safe(root, name).read_text(encoding="utf-8")
                     for name in (PRD, TECHNICAL, TEST_PLAN, DESIGN, MODULE)}
        prd = documents[PRD].split("## 7.", 1)
        technical = documents[TECHNICAL].split("### 5.13", 1)
        if len(prd) != 2 or len(technical) != 2:
            errors.append("PRD chapter 7 and technical section 5.13 must exist")
        for name, body in ((PRD, prd[-1]), (TECHNICAL, technical[-1]), (MODULE, documents[MODULE])):
            for identity in ENGINE_BACKENDS:
                if not re.search(r"(?<![\w])" + re.escape(identity) + r"(?![\w])", body):
                    errors.append(f"{name}: missing traceable engine {identity}")
        for name in (PRD, TEST_PLAN):
            actual = re.findall(r"^\|\s*(OT-\d{2})\s*\|", documents[name], re.M)
            if len(actual) != 16 or set(actual) != OT_IDS:
                errors.append(f"{name}: every OT-01 through OT-16 must have exactly one acceptance row")
            if name == TEST_PLAN:
                rows = re.findall(r"^\|\s*OT-\d{2}\s*\|.*$", documents[name], re.M)
                if any(not re.search(r"\|\s*not_run\s*\|\s*$", row) for row in rows):
                    errors.append("OT test plan product statuses must all remain not_run")
        parsed = _DesignReferences()
        parsed.feed(documents[DESIGN])
        if parsed.bases != ["../../"]:
            errors.append("Candidate must retain one ../../ base to reuse the complete canonical design")
        required = {"material-tokens.css", "styles.css", "settings.css", "controls.css", "controls.js",
                    "icons.js", "data.js", "app.js", "learning.js", "game-library.js",
                    "candidates/local-offline-20261030/settings.js",
                    "candidates/local-offline-20261030/local-translation-design.js",
                    "candidates/local-offline-20261030/local-translation-design.css"}
        if not required.issubset(parsed.assets) or len(parsed.assets) != len(set(parsed.assets)):
            errors.append("Candidate must reuse shared material, controls and complete-design assets without duplicate loads")
        for asset in parsed.assets:
            parsed_url = urlsplit(asset)
            path = Path(unquote(parsed_url.path))
            if parsed_url.scheme or parsed_url.netloc or path.is_absolute() or ".." in path.parts:
                errors.append(f"Candidate design asset must stay local: {asset}")
            else:
                _safe(root, DESIGN_ROOT + "/" + str(path))
        if "待评审" not in documents[DESIGN] or "设计模拟" not in documents[MODULE]:
            errors.append("Candidate must explicitly identify pending review and simulated design behavior")
    except (OSError, ValueError, TypeError, KeyError, UnicodeError, AttributeError) as error:
        errors.append(f"Offline translation design cannot be checked: {error}")
    return errors


def _output_path(root: Path, output: Path) -> Path:
    """Accept system path aliases, but never an internal symlink or reused run."""
    lexical_root = root.absolute()
    physical_root = lexical_root.resolve()
    output = output if output.is_absolute() else lexical_root / output
    try:
        relative = output.relative_to(lexical_root)
    except ValueError:
        try:
            relative = output.relative_to(physical_root)
        except ValueError:
            # /var and /private/var can spell the same macOS ancestor. Locate
            # that root boundary before checking every component *inside* it.
            aliases = [ancestor for ancestor in output.parents
                       if ancestor.name == physical_root.name
                       and ancestor.parent.resolve() == physical_root.parent
                       and ancestor.resolve() == physical_root]
            if not aliases:
                raise ValueError("Execution output must stay inside root/.build/") from None
            relative = output.relative_to(aliases[-1])
    if not relative.parts or relative.parts[0] != ".build" or len(relative.parts) < 2 or ".." in relative.parts:
        raise ValueError("Execution output must be a fresh child of root/.build/")
    target = physical_root / relative
    for component in (target, *target.parents):
        if component == physical_root:
            break
        if component.is_symlink():
            raise ValueError("Execution output symlinks are forbidden")
    if target.exists():
        raise ValueError("Execution output already exists; use a fresh run directory")
    return target


def _source_hashes(root: Path) -> dict[str, str]:
    """Fingerprint current bytes; never refresh approvals, tokens or aliases."""
    paths = {CATALOG, PRD, TECHNICAL, TEST_PLAN, LEGACY_CHECK, RUNNER,
             "scripts/quality/offline_translation_design.py",
             "scripts/quality/test_offline_translation_design.py",
             "openspec/technology-lock.json", "docs/development/iteration-baseline.json"}
    paths.update(str(path.relative_to(root)) for path in (root / DESIGN_ROOT).rglob("*") if path.is_file())
    # Preserve approval/alias metadata even when outside the shared design tree.
    paths.update(str(path.relative_to(root)) for path in (root / "docs/development").rglob("*.json"))
    legacy = _legacy(root)
    paths.update(relative for _, _, relative in legacy.DOCS)
    return {relative: hashlib.sha256(_safe(root, relative).read_bytes()).hexdigest() for relative in sorted(paths)}


def _adapter_report(destination: Path) -> tuple[dict, list[str]]:
    """Independently check the real adapter's assertion and screenshot evidence."""
    path = _safe(destination, "report.json")
    report = _load_json(path)
    errors: list[str] = []
    if not isinstance(report, dict):
        return {}, ["Node/Playwright report must be a JSON object"]
    if (type(report.get("schema_version")) is not int or report.get("schema_version") != 1
            or report.get("kind") != "local_offline_design_verification"
            or report.get("layer") != "design" or report.get("status") != "passed"
            or report.get("product_tests_executed") is not False):
        errors.append("Node/Playwright report must identify a passed design-only execution")
    if not _positive(report.get("assertions")):
        errors.append("Node/Playwright report must contain actual nonzero assertions")
    checks = report.get("checks")
    if (not isinstance(checks, list) or not checks or any(not isinstance(row, dict) for row in checks)
            or not _strings([row.get("id") for row in checks])
            or any(row.get("status") != "passed" for row in checks)):
        errors.append("Node/Playwright report requires unique passed check IDs")
    if report.get("page_errors") != [] or report.get("network_requests") != []:
        errors.append("Browser page errors or external network requests reject design acceptance")
    screenshots = report.get("screenshots")
    if not isinstance(screenshots, list) or not screenshots or any(not isinstance(row, dict) for row in screenshots):
        errors.append("Node/Playwright report requires actual screenshot evidence")
    else:
        names = [row.get("path") for row in screenshots]
        if not _strings(names):
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
            except (OSError, ValueError) as error:
                errors.append(str(error))
    identity = {"path": "report.json", "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "assertions": report.get("assertions"), "checks": len(checks) if isinstance(checks, list) else 0}
    return identity, errors


def execute(root: Path, output: Path) -> dict:
    """Run the real browser design adapter and keep failure evidence on disk."""
    destination = _output_path(Path(root), Path(output))
    root = Path(root).resolve()
    errors = check(root)
    destination.mkdir(parents=True, exist_ok=False)
    receipt = {
        "schema_version": 1, "kind": "offline_translation_design_execution",
        "layer": "design_only", "product_acceptance": "not_run",
        "candidate_design_status": "pending_review", "status": "failed",
        "started_at": datetime.now(timezone.utc).isoformat(), "root": str(root),
        "output": str(destination), "static_errors": errors, "command": None,
        "exit_code": None, "source_hashes_before": {}, "source_hashes_after": {}, "errors": [],
    }
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
                    completed = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr,
                                               check=False, timeout=300)
                    receipt["exit_code"] = completed.returncode
                except subprocess.TimeoutExpired:
                    receipt["errors"].append("Node/Playwright design adapter exceeded the 300-second execution budget")
                    receipt["exit_code"] = 124
                except OSError as error:
                    receipt["errors"].append(f"Cannot execute Node/Playwright adapter: {error}")
                    receipt["exit_code"] = 127
            after = _source_hashes(root)
            receipt["source_hashes_after"] = after
            if before != after:
                receipt["errors"].append("Controlled source/approval/token/alias bytes changed during execution")
            receipt["static_errors_after"] = check(root)
            if receipt["exit_code"] != 0:
                receipt["errors"].append("Node/Playwright design adapter returned a nonzero exit code")
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
    receipt["stdout"] = {"path": "stdout.log", "sha256": hashlib.sha256(stdout_path.read_bytes()).hexdigest()}
    receipt["stderr"] = {"path": "stderr.log", "sha256": hashlib.sha256(stderr_path.read_bytes()).hexdigest()}
    (destination / "execution.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="read-only static validation (default)")
    mode.add_argument("--execute", action="store_true", help="run the real design-only browser adapter")
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
            result = {"kind": "offline_translation_design_static", "layer": "design_only",
                      "status": "failed" if errors else "passed", "errors": errors,
                      "product_acceptance": "not_run", "candidate_design_status": "pending_review"}
    except (OSError, ValueError) as error:
        result = {"status": "failed", "layer": "design_only", "product_acceptance": "not_run", "errors": [str(error)]}
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"Offline translation design: {result['status']} (design only; product acceptance not_run)")
        for error in result.get("errors", []) + result.get("static_errors", []):
            print(f"- {error}")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
