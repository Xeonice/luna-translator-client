"""Small fixture and mutation tests for design guards, never engine acceptance."""
from __future__ import annotations

import copy
import base64
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
SOURCE = ROOT / "scripts/quality/offline_translation_design.py"
SPEC = importlib.util.spec_from_file_location("offline_translation_design_guards", SOURCE)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
exec(compile(SOURCE.read_bytes(), str(SOURCE), "exec"), guard.__dict__)


class OfflineTranslationDesignTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="luna-offline-design-guard-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.catalog = json.loads((ROOT / guard.CATALOG).read_text(encoding="utf-8"))
        for relative in self.catalog["source_documents"]:
            self.write(relative, "# Documentation fixture; all product acceptance not_run.\n")
        self.write(guard.LEGACY_CHECK, (ROOT / guard.LEGACY_CHECK).read_text(encoding="utf-8"))
        legacy = guard._legacy(self.root.resolve())
        self.write(guard.TECHNICAL, "\n".join([
            "# Technical design fixture", "<!-- model-service-protocols:start -->",
            legacy.render_protocol_table(self.catalog), "<!-- model-service-protocols:end -->",
            "<!-- model-service-presets:start -->", legacy.render_preset_table(self.catalog),
            "<!-- model-service-presets:end -->", "### 5.13 Offline engines",
            *guard.ENGINE_BACKENDS,
        ]))
        rows = "\n".join(f"| {identity} | Planned assertion | not_run |" for identity in sorted(guard.OT_IDS))
        self.write(guard.PRD, "## 7. Offline translation\n" + "\n".join(guard.ENGINE_BACKENDS) + "\n" + rows)
        self.write(guard.TEST_PLAN, "# Offline product tests remain not_run\n" + rows)
        design = (ROOT / guard.DESIGN).read_text(encoding="utf-8")
        self.write(guard.DESIGN, design)
        references = guard._DesignReferences()
        references.feed(design)
        for asset in references.assets:
            self.write(guard.DESIGN_ROOT + "/" + asset, "/* Shared-design fixture. */\n")
        self.write(guard.MODULE, "/* 设计模拟 only. */\n" + "\n".join(guard.ENGINE_BACKENDS))
        for relative in (guard.RUNNER, "scripts/quality/offline_translation_design.py",
                         "scripts/quality/test_offline_translation_design.py"):
            self.write(relative, "// Fixture is never executed as a browser or product.\n")
        self.write("openspec/technology-lock.json", "{}\n")
        self.write("docs/development/iteration-baseline.json", "{}\n")
        self.save_catalog()

    def write(self, relative: str, body: str) -> None:
        target = self.root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(body, encoding="utf-8")

    def save_catalog(self) -> None:
        self.write(guard.CATALOG, json.dumps(self.catalog, ensure_ascii=False, indent=2) + "\n")

    def reject(self, mutate) -> list[str]:
        catalog = copy.deepcopy(self.catalog)
        mutate(catalog)
        errors = guard.validate_catalog(catalog)
        self.assertTrue(errors, "Mutated offline design contract was accepted")
        self.assertTrue(all(isinstance(error, str) and error for error in errors), errors)
        return errors

    def adapter_success(self, command, **kwargs):
        output = Path(command[-1])
        # A fixed one-pixel PNG exercises independent file/hash/dimension checks;
        # this fixture never pretends to be a real browser execution.
        data = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=")
        (output / "fixture.png").write_bytes(data)
        report = {"schema_version": 1, "kind": "local_offline_design_verification", "layer": "design",
                  "status": "passed", "assertions": 1, "checks": [{"id": "fixture-check", "status": "passed"}],
                  "screenshots": [{"path": "fixture.png", "sha256": hashlib.sha256(data).hexdigest(),
                                   "bytes": len(data), "width": 1, "height": 1}],
                  "page_errors": [], "network_requests": [], "product_tests_executed": False}
        (output / "report.json").write_text(json.dumps(report), encoding="utf-8")
        return SimpleNamespace(returncode=0)

    def test_complete_fixture_and_static_cli_pass_without_writes(self) -> None:
        self.assertEqual(guard.validate_catalog(self.catalog), [])
        self.assertEqual(guard.check(self.root), [])
        before = {str(path.relative_to(self.root)): path.read_bytes()
                  for path in self.root.rglob("*") if path.is_file()}
        completed = subprocess.run([sys.executable, str(SOURCE), "--root", str(self.root), "--check", "--json"],
                                   capture_output=True, text=True, check=False)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["product_acceptance"], "not_run")
        after = {str(path.relative_to(self.root)): path.read_bytes()
                 for path in self.root.rglob("*") if path.is_file()}
        self.assertEqual(before, after, "Static validation wrote source files or cache artifacts")

    def test_missing_engine_is_rejected(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"].pop())

    def test_duplicate_engine_is_rejected(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"][1].update(id="apple_translation"))

    def test_wrong_backend_and_missing_backend_are_rejected(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"][0].update(execution_backend="http"))
        self.reject(lambda catalog: catalog["execution_backends"].pop())

    def test_offline_engine_cannot_gain_learning_purposes(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"][1].update(allowed_purposes=["translation", "grammar"]))

    def test_fake_http_llm_and_apple_installation_fields_are_rejected(self) -> None:
        for field in ("endpoint", "url", "baseURL", "API Key", "model_id", "llm_model_id", "prompt"):
            with self.subTest(field=field):
                self.reject(lambda catalog: catalog["offline_engines"][1]["required_engine_fields"].append(field))
        self.reject(lambda catalog: catalog["offline_engines"][0]["required_common_fields"].append("installation_path"))
        self.reject(lambda catalog: catalog["offline_engines"][2].update(forbidden_required_fields=["endpoint", "api_key"]))

    def test_boolean_or_nonpositive_contract_revisions_are_rejected(self) -> None:
        for revision in (True, False, 0, -1, 1.0, "1", None, [], {}):
            with self.subTest(revision=revision):
                self.reject(lambda catalog: catalog["offline_engines"][0].update(engine_contract_revision=revision))

    def test_fabricated_implementation_live_distribution_and_ot_passes_are_rejected(self) -> None:
        for field, value in (("implementation_status", "implemented"), ("adapter_acceptance", "passed"),
                             ("live_acceptance", "passed"), ("distribution_status", "approved")):
            with self.subTest(field=field):
                self.reject(lambda catalog: catalog["offline_engines"][0].update({field: value}))
        self.reject(lambda catalog: catalog["offline_acceptance"][0].update(status="passed"))
        self.reject(lambda catalog: catalog["offline_scope"].update(candidate_design_status="approved"))

    def test_implicit_pivot_or_cloud_fallback_is_rejected(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"][1].update(default_language_route="auto_pivot"))
        self.reject(lambda catalog: catalog["offline_engines"][1].update(routing_policy="Automatically pivot when a direct pair is missing"))
        self.reject(lambda catalog: catalog["offline_engines"][1].update(network_fallback="cloud_on_failure"))

    def test_probe_is_not_translation_and_argos_python_is_unapproved(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"][0].update(probe_is_not_translation_acceptance=False))
        self.reject(lambda catalog: catalog["offline_engines"][1].update(production_python_approved=True))
        self.reject(lambda catalog: catalog["offline_engines"][0].update(model_identity="required"))

    def test_apple_cannot_silently_enable_intelligence(self) -> None:
        self.reject(lambda catalog: catalog["offline_engines"][0].update(non_llm_policy="Use highFidelity or Apple Intelligence by default"))
        self.reject(lambda catalog: catalog["offline_engines"][0].update(minimum_api_availability=[]))
        self.reject(lambda catalog: catalog["offline_engines"][0].update(runtime_route="highFidelity"))
        self.reject(lambda catalog: catalog["offline_engines"][0].update(engine_family="llm"))

    def test_malformed_catalog_shapes_are_errors_instead_of_exceptions(self) -> None:
        for value in (None, [], "catalog", 4, True):
            with self.subTest(value=value):
                self.assertTrue(guard.validate_catalog(value))
        for field in ("execution_backends", "offline_engines", "offline_acceptance", "offline_counts", "offline_scope",
                      "offline_runtime_contract_candidates"):
            for value in (None, "bad", [None], [{"id": []}]):
                with self.subTest(field=field, value=value):
                    self.reject(lambda catalog: catalog.update({field: value}))
        for field in ("id", "required_common_fields", "required_engine_fields", "forbidden_required_fields", "test_identity"):
            self.reject(lambda catalog: catalog["offline_engines"][0].update({field: [None, {}]}))
        self.reject(lambda catalog: catalog.update(schema_version=True))

    def test_missing_duplicate_or_untraceable_acceptance_is_rejected(self) -> None:
        self.reject(lambda catalog: catalog["offline_acceptance"].pop())
        self.reject(lambda catalog: catalog["offline_acceptance"][1].update(id="OT-01"))
        self.reject(lambda catalog: catalog["offline_acceptance"][0].update(source_requirement_ids=[]))
        self.reject(lambda catalog: catalog["offline_acceptance"][0].update(source_configuration_requirement_ids=["AI-CFG-99"]))

    def test_missing_ot_row_or_faked_test_plan_pass_is_rejected(self) -> None:
        original = (self.root / guard.TEST_PLAN).read_text(encoding="utf-8")
        self.write(guard.TEST_PLAN, re_sub_row(original, "OT-16"))
        self.assertTrue(guard.check(self.root))
        self.write(guard.TEST_PLAN, original.replace("not_run |", "passed |", 1))
        self.assertTrue(guard.check(self.root))

    def test_missing_engine_trace_in_each_document_or_module_is_rejected(self) -> None:
        for relative in (guard.PRD, guard.TECHNICAL, guard.MODULE):
            with self.subTest(relative=relative):
                original = (self.root / relative).read_text(encoding="utf-8")
                self.write(relative, original.replace("kingsoft_fastait", "removed_engine"))
                self.assertTrue(guard.check(self.root))
                self.write(relative, original)

    def test_candidate_requires_shared_base_material_and_custom_controls(self) -> None:
        original = (self.root / guard.DESIGN).read_text(encoding="utf-8")
        for old, new in (('<base href="../../">', ''), ('href="material-tokens.css"', 'href="another-theme.css"'),
                         ('src="controls.js"', 'src="https://example.test/controls.js"')):
            with self.subTest(old=old):
                self.write(guard.DESIGN, original.replace(old, new))
                self.assertTrue(guard.check(self.root))
        self.write(guard.DESIGN, original)
        (self.root / guard.DESIGN_ROOT / "controls.js").unlink()
        self.assertTrue(guard.check(self.root))

    def test_old_http_inventory_and_generated_tables_are_still_guarded(self) -> None:
        self.catalog["protocols"].pop()
        self.save_catalog()
        self.assertTrue(guard.check(self.root))

    def test_duplicate_json_keys_and_source_symlinks_are_rejected(self) -> None:
        self.write(guard.CATALOG, '{"schema_version": 1, "schema_version": 1}')
        self.assertTrue(guard.check(self.root))
        self.save_catalog()
        source = self.root / guard.TEST_PLAN
        target = self.root / "real-test-plan.md"
        source.rename(target)
        source.symlink_to(target)
        self.assertTrue(guard.check(self.root))

    def test_output_must_be_fresh_inside_build_without_symlinks(self) -> None:
        self.assertEqual(guard._output_path(self.root, Path(".build/new-run")), self.root.resolve() / ".build/new-run")
        existing = self.root / ".build/existing"
        existing.mkdir(parents=True)
        for output in (Path("reports/run"), Path(".build"), Path(".build/../escape"), existing, self.root.parent / "escape"):
            with self.subTest(output=output), self.assertRaises(ValueError):
                guard._output_path(self.root, output)
        (self.root / ".build/link").symlink_to(self.root)
        with self.assertRaises(ValueError):
            guard._output_path(self.root, Path(".build/link/new"))

    def test_system_temp_path_alias_is_accepted_after_resolving(self) -> None:
        lexical = Path(tempfile.gettempdir()) / self.root.name
        self.assertEqual(lexical.resolve(), self.root.resolve())
        output = lexical / ".build/new-run"
        self.assertEqual(guard._output_path(self.root.resolve(), output), self.root.resolve() / ".build/new-run")

    def test_failed_static_check_never_starts_node_and_keeps_receipt(self) -> None:
        self.catalog["offline_engines"].pop()
        self.save_catalog()
        with patch.object(guard.subprocess, "run") as run:
            receipt = guard.execute(self.root, Path(".build/static-failure"))
        run.assert_not_called()
        self.assertEqual(receipt["status"], "failed")
        self.assertIsNone(receipt["exit_code"])
        self.assertTrue((self.root / ".build/static-failure/execution.json").is_file())

    def test_execution_records_actual_command_hashes_and_separate_product_status(self) -> None:
        with patch.object(guard.subprocess, "run", side_effect=self.adapter_success) as run:
            receipt = guard.execute(self.root, Path(".build/pass"))
        self.assertEqual(receipt["status"], "passed", receipt)
        self.assertEqual(receipt["product_acceptance"], "not_run")
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual(receipt["source_hashes_before"], receipt["source_hashes_after"])
        command = ["node", str(self.root.resolve() / guard.RUNNER), str(self.root.resolve()), str(self.root.resolve() / ".build/pass")]
        self.assertEqual(run.call_args.args[0], command)
        self.assertEqual(receipt["command"], command)

    def test_zero_exit_without_real_report_or_screenshot_evidence_is_rejected(self) -> None:
        with patch.object(guard.subprocess, "run", return_value=SimpleNamespace(returncode=0)):
            receipt = guard.execute(self.root, Path(".build/no-report"))
        self.assertEqual(receipt["status"], "failed")
        for field, value in (("assertions", 0), ("assertions", True), ("checks", []),
                             ("product_tests_executed", True), ("page_errors", ["error"]),
                             ("network_requests", ["https://example.test"]), ("screenshots", [])):
            with self.subTest(field=field, value=value):
                output = self.root / ".build" / ("report-" + str(len(list((self.root / ".build").iterdir()))))
                output.mkdir()
                self.adapter_success([str(output)])
                report_path = output / "report.json"
                report = json.loads(report_path.read_text())
                report[field] = value
                report_path.write_text(json.dumps(report))
                self.assertTrue(guard._adapter_report(output)[1])

    def test_screenshot_hash_dimensions_and_source_path_are_independently_checked(self) -> None:
        for field, value in (("sha256", "0" * 64), ("bytes", 1), ("width", 2), ("height", True),
                             ("path", "../fixture.png")):
            with self.subTest(field=field):
                output = self.root / ".build" / ("screenshot-" + field)
                output.mkdir(parents=True)
                self.adapter_success([str(output)])
                report_path = output / "report.json"
                report = json.loads(report_path.read_text())
                report["screenshots"][0][field] = value
                report_path.write_text(json.dumps(report))
                self.assertTrue(guard._adapter_report(output)[1])

    def test_adapter_failure_keeps_exit_code_logs_and_cli_is_nonzero(self) -> None:
        def failed(command, **kwargs):
            kwargs["stdout"].write(b"browser assertion started\n")
            kwargs["stderr"].write(b"assertion failed\n")
            return SimpleNamespace(returncode=23)
        with patch.object(guard.subprocess, "run", side_effect=failed):
            receipt = guard.execute(self.root, Path(".build/browser-failure"))
        self.assertEqual(receipt["status"], "failed")
        self.assertEqual(receipt["exit_code"], 23)
        self.assertIn("assertion failed", (self.root / ".build/browser-failure/stderr.log").read_text())
        with patch.object(guard, "execute", return_value=receipt), patch("builtins.print"):
            self.assertEqual(guard.main(["--root", str(self.root), "--execute", "--output", ".build/cli"]), 1)

    def test_adapter_cannot_change_controlled_source_and_report_pass(self) -> None:
        def changed(command, **kwargs):
            self.write("openspec/technology-lock.json", '{"unapproved": true}\n')
            return SimpleNamespace(returncode=0)
        with patch.object(guard.subprocess, "run", side_effect=changed):
            receipt = guard.execute(self.root, Path(".build/source-drift"))
        self.assertEqual(receipt["status"], "failed")
        self.assertNotEqual(receipt["source_hashes_before"], receipt["source_hashes_after"])

    def test_timeout_and_missing_node_are_failures_with_receipts(self) -> None:
        for error, name, expected in ((subprocess.TimeoutExpired("node", 300), "timeout", 124),
                                      (FileNotFoundError("node missing"), "missing-node", 127)):
            with self.subTest(name=name), patch.object(guard.subprocess, "run", side_effect=error):
                receipt = guard.execute(self.root, Path(".build/" + name))
                self.assertEqual(receipt["status"], "failed")
                self.assertEqual(receipt["exit_code"], expected)
                self.assertTrue((self.root / ".build" / name / "execution.json").is_file())


def re_sub_row(body: str, identity: str) -> str:
    return "\n".join(row for row in body.splitlines() if not row.startswith(f"| {identity} |"))


if __name__ == "__main__":
    unittest.main()
