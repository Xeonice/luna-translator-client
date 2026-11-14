"""Contract mutations and adapter evidence fixtures; never product acceptance."""
from __future__ import annotations

import base64
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "scripts/quality/translation_service_selection.py"
SPEC = importlib.util.spec_from_file_location("translation_selection_guards", SOURCE)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
exec(compile(SOURCE.read_bytes(), str(SOURCE), "exec"), guard.__dict__)


class TranslationServiceSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="luna-selection-design-guard-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.catalog = json.loads((ROOT / guard.CATALOG).read_text(encoding="utf-8"))
        old = guard.helpers
        paths = {guard.CATALOG, guard.PRD, guard.TECHNICAL, guard.TEST_PLAN, guard.CHECKER,
                 guard.TESTS, guard.OLD_CHECKER, old.LEGACY_CHECK, old.TEST_PLAN, old.DESIGN,
                 old.MODULE, "openspec/technology-lock.json", "docs/development/iteration-baseline.json"}
        paths.update(self.catalog["source_documents"])
        paths.update(relative for _, _, relative in old._legacy(ROOT).DOCS)
        self.assets = guard._references((ROOT / guard.DESIGN).read_text(encoding="utf-8"))
        parser = old._DesignReferences()
        parser.feed((ROOT / old.DESIGN).read_text(encoding="utf-8"))
        paths.update(guard.DESIGN_ROOT + "/" + asset for asset in self.assets + parser.assets)
        paths.update(str(path.relative_to(ROOT)) for path in (ROOT / guard.CANDIDATE).iterdir()
                     if path.is_file() and path.name != "manifest.json")
        for relative in paths:
            self.write(relative, (ROOT / relative).read_bytes())
        self.write(guard.RUNNER, "// Fixture is never executed as a browser or product.\n")
        self.refresh_manifest()
        self.assertEqual(guard.check(self.root), [], "Mutation fixture must start valid")

    def write(self, relative: str, body: str | bytes) -> None:
        target = self.root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(body.encode("utf-8") if isinstance(body, str) else body)

    def refresh_manifest(self) -> None:
        owned = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                 for path in (self.root / guard.CANDIDATE).iterdir()
                 if path.is_file() and path.name != "manifest.json"}
        prefix = "candidates/" + guard.CANDIDATE_ID + "/"
        shared = {relative: hashlib.sha256((self.root / guard.DESIGN_ROOT / relative).read_bytes()).hexdigest()
                  for relative in self.assets if not relative.startswith(prefix)}
        manifest = {"schema_version": 1, "candidate_id": guard.CANDIDATE_ID,
                    "status": "pending_review", "entry": guard.DESIGN, "runtime_base_href": "../../",
                    "simulation_boundary": "Design simulation only; no product acceptance.",
                    "owned_sources": owned, "shared_dependencies": shared}
        self.write(guard.MANIFEST, json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")

    def save_catalog(self) -> None:
        self.write(guard.CATALOG, json.dumps(self.catalog, ensure_ascii=False, indent=2) + "\n")

    def reject(self, mutate) -> None:
        catalog = copy.deepcopy(self.catalog)
        mutate(catalog)
        errors = guard.validate_catalog(catalog)
        self.assertTrue(errors, "Mutated contract was accepted")
        self.assertTrue(all(isinstance(error, str) and error for error in errors))

    def selection_mutation(self, section: str, **updates) -> None:
        self.reject(lambda c: c["service_selection_design"][section].update(updates))

    def adapter_success(self, command, **kwargs):
        output = Path(command[-1])
        # Real small PNG used only to exercise file/hash/dimension checks.
        data = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=")
        (output / "fixture.png").write_bytes(data)
        report = {"schema_version": 1, "kind": "translation_service_selection_design_verification",
                  "layer": "design", "status": "passed", "assertions": 12,
                  "checks": [{"id": identity, "status": "passed"} for identity in sorted(guard.SS_IDS)],
                  "screenshots": [{"path": "fixture.png", "sha256": hashlib.sha256(data).hexdigest(),
                                   "bytes": len(data), "width": 1, "height": 1}],
                  "page_errors": [], "network_requests": [], "product_tests_executed": False}
        (output / "report.json").write_text(json.dumps(report), encoding="utf-8")
        return SimpleNamespace(returncode=0)

    def report_fixture(self, name: str) -> Path:
        output = self.root / ".build" / name
        output.mkdir(parents=True)
        self.adapter_success([str(output)])
        return output

    def edit_report(self, output: Path, mutate) -> None:
        path = output / "report.json"
        report = json.loads(path.read_text())
        mutate(report)
        path.write_text(json.dumps(report))

    def test_valid_static_cli_is_readonly(self) -> None:
        self.assertEqual(guard.validate_catalog(self.catalog), [])
        before = {str(p.relative_to(self.root)): p.read_bytes() for p in self.root.rglob("*") if p.is_file()}
        run = subprocess.run([sys.executable, str(SOURCE), "--root", str(self.root), "--check", "--json"],
                             capture_output=True, text=True, check=False)
        self.assertEqual(run.returncode, 0, run.stderr + run.stdout)
        result = json.loads(run.stdout)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["product_acceptance"], "not_run")
        after = {str(p.relative_to(self.root)): p.read_bytes() for p in self.root.rglob("*") if p.is_file()}
        self.assertEqual(before, after, "Read-only check wrote source or caches")

    def test_ss01_same_shallow_and_hud_picker(self) -> None:
        self.selection_mutation("daily_selection", entrypoints=["deep_settings_only"])

    def test_ss02_templates_drafts_and_unsaved_resources_cannot_be_selected(self) -> None:
        for key, value in (("source", "all_templates"), ("template_catalog", "daily_picker"), ("draft_policy", "read_form")):
            self.selection_mutation("daily_selection", **{key: value})

    def test_ss03_missing_credentials_suspended_and_known_failure_block(self) -> None:
        for removed in ("local_fields_and_credential_reference_valid", "not_suspended", "no_known_failure_for_selected_revision"):
            self.reject(lambda c: c["service_selection_design"]["daily_selection"]["eligibility"].remove(removed))
        self.selection_mutation("daily_selection", known_failure="select anyway")

    def test_ss04_valid_untested_http_stays_unverified(self) -> None:
        for value in ("untested permanently blocked", "untested means passed", "reuse old test"):
            self.selection_mutation("daily_selection", http_untested=value)

    def test_ss05_unknown_local_health_cannot_use_http_exception(self) -> None:
        self.selection_mutation("daily_selection", local_unknown="unknown is available")
        self.reject(lambda c: c["service_selection_design"]["daily_selection"]["eligibility"].remove("required_local_runtime_and_resource_gates_pass"))

    def test_ss06_explicit_translation_purpose_and_capability(self) -> None:
        self.selection_mutation("daily_selection", llm_filter="infer translation from brand", purpose="grammar")
        self.reject(lambda c: c["offline_engines"][0].update(allowed_purposes=["translation", "grammar"]))

    def test_ss07_save_and_sample_test_cannot_change_binding(self) -> None:
        for field in ("save_changes_current_selection", "test_changes_current_selection"):
            self.selection_mutation("binding_contract_candidate", **{field: True})
        self.selection_mutation("binding_contract_candidate", resource_reference="mutable editor draft")

    def test_ss08_subsequent_sentences_preserved_enablement_explicit_retranslation(self) -> None:
        self.selection_mutation("binding_contract_candidate", select_changes_purpose_enabled=True)
        for key, value in (("selection_effect", "automatically retranslate current sentence"),
                           ("current_sentence_retranslation", "overwrite prior result"),
                           ("enable_disable", "selection enables all purposes")):
            self.selection_mutation("binding_contract_candidate", **{key: value})

    def test_ss09_scope_inheritance_and_concurrency_are_explicit(self) -> None:
        self.selection_mutation("binding_contract_candidate", scopes=["global_default"])
        self.selection_mutation("binding_contract_candidate", optimistic_concurrency="last writer wins across profiles")

    def test_ss10_cancelled_late_results_cannot_write_success_or_history(self) -> None:
        self.selection_mutation("binding_contract_candidate", cancellation="accept stale callbacks")
        self.selection_mutation("binding_contract_candidate", history="rewrite history to new service")
        self.selection_mutation("daily_selection", no_implicit_fallback=False)

    def test_ss11_single_main_ordered_stable_identity_deduplication(self) -> None:
        self.selection_mutation("binding_contract_candidate", main_and_comparison="deduplicate by display name; main may repeat")

    def test_ss12_configure_return_preserves_context_without_auto_selection(self) -> None:
        self.selection_mutation("binding_contract_candidate", configure_return_context=["origin"])
        self.selection_mutation("binding_contract_candidate", return_behavior="auto choose newly saved service and send text")

    def test_no_approval_or_production_authority_can_be_fabricated(self) -> None:
        for key, value in (("ui_direct_http_sql_ffi", True), ("prototype_only", False), ("installed_production_contract", True)):
            self.selection_mutation("projection_contract_candidate", **{key: value})
        self.reject(lambda c: c["service_selection_design"].update(candidate_design_status="approved"))
        self.selection_mutation("scope_boundaries", product_acceptance="passed")
        self.reject(lambda c: c["selection_acceptance"][0].update(status="passed"))

    def test_original_http_offline_counts_and_acceptance_still_guarded(self) -> None:
        original = copy.deepcopy(self.catalog)
        for key in ("protocols", "presets", "offline_engines", "offline_acceptance"):
            self.catalog = copy.deepcopy(original)
            self.catalog[key].pop()
            self.save_catalog()
            self.assertTrue(guard.check(self.root))
        for field in ("http_protocols", "http_presets", "offline_engines", "original_PA_acceptance", "original_OT_acceptance"):
            self.selection_mutation("scope_boundaries", **{field: 999})

    def test_malformed_catalog_shapes_return_errors_without_exceptions(self) -> None:
        for value in (None, True, "catalog", [], 1):
            self.assertTrue(guard.validate_catalog(value))
        for field in ("service_selection_design", "selection_acceptance"):
            for value in (None, [], "bad", [None], [{"id": []}]):
                self.reject(lambda c: c.update({field: value}))
        for field in ("daily_selection", "projection_contract_candidate", "binding_contract_candidate", "scope_boundaries"):
            for value in (None, [], "bad", True):
                self.reject(lambda c: c["service_selection_design"].update({field: value}))
        for key in ("id", "source_requirement_ids", "source_configuration_requirement_ids", "verification_layers"):
            self.reject(lambda c: c["selection_acceptance"][0].update({key: [{}, None]}))

    def test_missing_duplicate_untraceable_or_design_only_ss_is_rejected(self) -> None:
        self.reject(lambda c: c["selection_acceptance"].pop())
        self.reject(lambda c: c["selection_acceptance"][1].update(id="SS-01"))
        self.reject(lambda c: c["selection_acceptance"][0].update(source_requirement_ids=["FP-99"]))
        self.reject(lambda c: c["selection_acceptance"][0].update(source_configuration_requirement_ids=[]))
        self.reject(lambda c: c["selection_acceptance"][0].update(verification_layers=["design"]))

    def test_prd_technical_plan_anchors_and_not_run_rows_are_required(self) -> None:
        for relative, old, new in ((guard.PRD, "translation-service-selection-prd", "missing-anchor"),
                                   (guard.TECHNICAL, "translation-service-selection-contract", "missing-anchor"),
                                   (guard.TECHNICAL, "### 5.21 ", "### 5.99 "),
                                   (guard.TEST_PLAN, "| SS-12 |", "| SS-99 |"),
                                   (guard.TEST_PLAN, "；not_run |", "；passed |")):
            original = (self.root / relative).read_text()
            self.assertIn(old, original)
            self.write(relative, original.replace(old, new, 1))
            self.assertTrue(guard.check(self.root))
            self.write(relative, original)

    def test_manifest_requires_current_entry_complete_sources_and_real_hashes(self) -> None:
        path = self.root / guard.MANIFEST
        original = path.read_text()
        for mutate in (lambda m: m.update(candidate_id="local-offline-20261030"),
                       lambda m: m.update(status="approved"),
                       lambda m: m["owned_sources"].pop("service-picker.js"),
                       lambda m: m["shared_dependencies"].pop("controls.js"),
                       lambda m: m["owned_sources"].update({"service-picker.js": "0" * 64}),
                       lambda m: m.update(owned_sources=[]),
                       lambda m: m["shared_dependencies"].update({"../outside.js": "0" * 64})):
            manifest = json.loads(original)
            mutate(manifest)
            path.write_text(json.dumps(manifest))
            self.assertTrue(guard.check(self.root))
        path.write_text(original)
        self.write(guard.CANDIDATE + "/unregistered.js", "// accidental source\n")
        self.assertTrue(guard.check(self.root))

    def test_external_assets_shared_controls_and_missing_prototype_are_rejected(self) -> None:
        for relative, old, new in ((guard.DESIGN, '<base href="../../">', ''),
                                   (guard.DESIGN, 'src="controls.js"', 'src="https://example.test/controls.js"'),
                                   (guard.DESIGN, 'href="material-tokens.css"', 'href="../escape.css"'),
                                   (guard.MODULE, 'LunaTranslationServices', 'RemovedPrototypeAPI')):
            original = (self.root / relative).read_text()
            self.assertIn(old, original)
            self.write(relative, original.replace(old, new))
            self.refresh_manifest()
            self.assertTrue(guard.check(self.root))
            self.write(relative, original)
            self.refresh_manifest()

    def test_duplicate_json_and_symlink_sources_are_rejected(self) -> None:
        self.write(guard.CATALOG, '{"schema_version":1,"schema_version":1}')
        self.assertTrue(guard.check(self.root))
        self.save_catalog()
        path = self.root / guard.TEST_PLAN
        real = path.with_name("actual-plan.md")
        path.rename(real)
        path.symlink_to(real)
        self.assertTrue(guard.check(self.root))

    def test_output_rejects_outside_reused_symlink_and_traversal(self) -> None:
        existing = self.root / ".build/existing"
        existing.mkdir(parents=True)
        for output in (Path("reports/run"), Path(".build"), Path(".build/../escape"), existing, self.root.parent / "escape"):
            with self.subTest(output=output), self.assertRaises(ValueError):
                guard.execute(self.root, output)
        (self.root / ".build/link").symlink_to(self.root)
        with self.assertRaises(ValueError):
            guard.execute(self.root, Path(".build/link/new"))

    def test_static_failure_never_starts_adapter_and_keeps_receipt(self) -> None:
        self.catalog["service_selection_design"]["daily_selection"]["source"] = "drafts"
        self.save_catalog()
        with patch.object(guard.subprocess, "run") as run:
            receipt = guard.execute(self.root, Path(".build/static-failure"))
        run.assert_not_called()
        self.assertEqual(receipt["status"], "failed")
        self.assertIsNone(receipt["exit_code"])
        self.assertTrue((self.root / ".build/static-failure/execution.json").is_file())

    def test_execution_records_command_sources_logs_and_product_status(self) -> None:
        with patch.object(guard.subprocess, "run", side_effect=self.adapter_success) as run:
            receipt = guard.execute(self.root, Path(".build/pass"))
        self.assertEqual(receipt["status"], "passed", receipt)
        self.assertEqual(receipt["product_acceptance"], "not_run")
        self.assertEqual(receipt["source_hashes_before"], receipt["source_hashes_after"])
        for required in (guard.CATALOG, guard.PRD, guard.TECHNICAL, guard.TEST_PLAN, guard.CHECKER,
                         guard.TESTS, guard.RUNNER, guard.MANIFEST, guard.MODULE, guard.DESIGN_ROOT + "/controls.js"):
            self.assertIn(required, receipt["source_hashes_before"])
        command = ["node", str(self.root.resolve() / guard.RUNNER), str(self.root.resolve()), str(self.root.resolve() / ".build/pass")]
        self.assertEqual(run.call_args.args[0], command)
        self.assertEqual(receipt["command"], command)
        self.assertEqual(receipt["exit_code"], 0)

    def test_zero_exit_without_report_and_failed_report_are_rejected(self) -> None:
        with patch.object(guard.subprocess, "run", return_value=SimpleNamespace(returncode=0)):
            receipt = guard.execute(self.root, Path(".build/no-report"))
        self.assertEqual(receipt["status"], "failed")
        def failed_report(command, **kwargs):
            result = self.adapter_success(command, **kwargs)
            self.edit_report(Path(command[-1]), lambda r: r.update(status="failed"))
            return result
        with patch.object(guard.subprocess, "run", side_effect=failed_report):
            receipt = guard.execute(self.root, Path(".build/failed-report"))
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual(receipt["status"], "failed")

    def test_reports_require_all_unique_passed_ss_and_actual_assertions(self) -> None:
        for index, (field, value) in enumerate((("assertions", 0), ("assertions", 1), ("assertions", True), ("checks", []),
                                               ("checks", [{"id": []}]), ("product_tests_executed", True),
                                               ("page_errors", ["error"]), ("network_requests", ["https://example.test"]),
                                               ("kind", "local_offline_design_verification"), ("screenshots", []))):
            output = self.report_fixture("bad-report-" + str(index))
            self.edit_report(output, lambda r: r.update({field: value}))
            self.assertTrue(guard._adapter_report(output)[1])
        for name, mutate in (("missing-check", lambda r: r["checks"].pop()),
                             ("duplicate-check", lambda r: r["checks"].append(r["checks"][0])),
                             ("failed-check", lambda r: r["checks"][0].update(status="failed"))):
            output = self.report_fixture(name)
            self.edit_report(output, mutate)
            self.assertTrue(guard._adapter_report(output)[1])

    def test_screenshot_hash_bytes_dimensions_path_and_symlink_are_independent(self) -> None:
        for index, (field, value) in enumerate((("sha256", "0" * 64), ("bytes", 1), ("width", 2),
                                               ("height", True), ("path", "../fixture.png"), ("path", "/tmp/fake.png"))):
            output = self.report_fixture("bad-image-" + str(index))
            self.edit_report(output, lambda r: r["screenshots"][0].update({field: value}))
            self.assertTrue(guard._adapter_report(output)[1])
        output = self.report_fixture("symlink-image")
        image = output / "fixture.png"
        real = output / "real.png"
        image.rename(real)
        image.symlink_to(real)
        self.assertTrue(guard._adapter_report(output)[1])

    def test_source_mutation_during_successful_adapter_is_rejected(self) -> None:
        def changed(command, **kwargs):
            result = self.adapter_success(command, **kwargs)
            self.write("openspec/technology-lock.json", '{"unapproved":true}\n')
            return result
        with patch.object(guard.subprocess, "run", side_effect=changed):
            receipt = guard.execute(self.root, Path(".build/source-drift"))
        self.assertEqual(receipt["status"], "failed")
        self.assertNotEqual(receipt["source_hashes_before"], receipt["source_hashes_after"])

    def test_candidate_source_drift_keeps_actual_after_hashes_and_fails(self) -> None:
        def changed(command, **kwargs):
            result = self.adapter_success(command, **kwargs)
            body = (self.root / guard.MODULE).read_text()
            self.write(guard.MODULE, body + "\n// changed during browser execution\n")
            return result
        with patch.object(guard.subprocess, "run", side_effect=changed):
            receipt = guard.execute(self.root, Path(".build/candidate-drift"))
        self.assertEqual(receipt["status"], "failed")
        self.assertTrue(receipt["source_hashes_after"])
        self.assertNotEqual(receipt["source_hashes_before"][guard.MODULE], receipt["source_hashes_after"][guard.MODULE])
        self.assertTrue(receipt["static_errors_after"])

    def test_nonzero_timeout_missing_node_and_cli_failure_keep_evidence(self) -> None:
        def failed(command, **kwargs):
            kwargs["stderr"].write(b"actual assertion failure\n")
            return SimpleNamespace(returncode=23)
        with patch.object(guard.subprocess, "run", side_effect=failed):
            receipt = guard.execute(self.root, Path(".build/nonzero"))
        self.assertEqual(receipt["status"], "failed")
        self.assertEqual(receipt["exit_code"], 23)
        self.assertIn("actual assertion failure", (self.root / ".build/nonzero/stderr.log").read_text())
        for error, name, expected in ((subprocess.TimeoutExpired("node", 300), "timeout", 124),
                                      (FileNotFoundError("missing node"), "missing-node", 127)):
            with patch.object(guard.subprocess, "run", side_effect=error):
                result = guard.execute(self.root, Path(".build/" + name))
                self.assertEqual(result["status"], "failed")
                self.assertEqual(result["exit_code"], expected)
                self.assertTrue((self.root / ".build" / name / "execution.json").is_file())
        with patch.object(guard, "execute", return_value=receipt), patch("builtins.print"):
            self.assertEqual(guard.main(["--root", str(self.root), "--execute", "--output", ".build/cli"]), 1)


if __name__ == "__main__":
    unittest.main()
