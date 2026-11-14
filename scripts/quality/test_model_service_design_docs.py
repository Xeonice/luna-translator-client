"""Positive and mutation tests for documentation guards, never product acceptance.

Each test uses a small temporary catalog/Markdown fixture. No provider calls,
database, application process, browser or product build is involved.
"""
from __future__ import annotations

import builtins
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "scripts/report/build_model_service_design_report.py"
SPEC = importlib.util.spec_from_file_location("model_service_design_report_guards", SOURCE)
assert SPEC and SPEC.loader
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)

CATALOG = "docs/architecture/model-service-registry-design.json"
PRIMARY = "docs/architecture/japanese-study-and-llm.md"
PROTOCOL_START = "<!-- model-service-protocols:start -->"
PROTOCOL_END = "<!-- model-service-protocols:end -->"
PRESET_START = "<!-- model-service-presets:start -->"
PRESET_END = "<!-- model-service-presets:end -->"


class ModelServiceDesignDocsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="luna-model-service-docs-guard-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        # Use the actual reviewed-design shape, not a second hand-copied schema.
        # Every mutation below changes an independent copy inside the fixture.
        self.catalog = json.loads((ROOT / CATALOG).read_text(encoding="utf-8"))
        for relative in self.catalog["source_documents"]:
            self.write(relative, "# Synthetic documentation guard fixture\n\nDocumentation only; product acceptance not_run.\n")
        self.save_catalog()
        self.render_markers()

    def write(self, relative, content):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def save_catalog(self):
        self.write(CATALOG, json.dumps(self.catalog, ensure_ascii=False, indent=2) + "\n")

    def render_markers(self):
        self.write(PRIMARY, "\n".join([
            "# Synthetic technical design fixture",
            "",
            "This is documentation planning only, not implemented or product acceptance.",
            "",
            PROTOCOL_START,
            report.render_protocol_table(self.catalog),
            PROTOCOL_END,
            "",
            PRESET_START,
            report.render_preset_table(self.catalog),
            PRESET_END,
            "",
        ]))

    def invalid_catalog(self, mutate):
        catalog = copy.deepcopy(self.catalog)
        mutate(catalog)
        errors = report.validate_catalog(self.root, catalog)
        self.assertTrue(errors, "Mutated documentation contract was accepted")
        self.assertTrue(all(isinstance(error, str) and error for error in errors), errors)
        return errors

    def test_complete_design_catalog_and_exact_generated_markers_pass(self):
        self.assertEqual(report.validate_catalog(self.root, self.catalog), [])
        self.assertEqual(report.check(self.root), [])

    def test_generated_tables_include_every_protocol_and_route_identity(self):
        protocols = report.render_protocol_table(self.catalog)
        presets = report.render_preset_table(self.catalog)
        for row in self.catalog["protocols"]:
            self.assertIn(row["id"], protocols)
        for row in self.catalog["presets"]:
            self.assertIn(row["id"], presets)

    def test_missing_protocol_is_rejected(self):
        self.invalid_catalog(lambda catalog: catalog["protocols"].pop())

    def test_duplicate_protocol_identity_is_rejected(self):
        self.invalid_catalog(lambda catalog: catalog["protocols"][1].update(id=catalog["protocols"][0]["id"]))

    def test_missing_route_is_rejected(self):
        self.invalid_catalog(lambda catalog: catalog["presets"].pop())

    def test_duplicate_route_identity_is_rejected(self):
        self.invalid_catalog(lambda catalog: catalog["presets"][1].update(id=catalog["presets"][0]["id"]))

    def test_self_reported_counts_do_not_replace_actual_inventory(self):
        self.invalid_catalog(lambda catalog: catalog["counts"].update(protocols=14))

    def test_route_cannot_reference_an_absent_protocol(self):
        self.invalid_catalog(lambda catalog: catalog["presets"][0].update(protocol_id="nonexistent-protocol"))

    def test_route_must_match_protocol_contract_revision(self):
        self.invalid_catalog(lambda catalog: catalog["presets"][0].update(adapter_contract_revision=999))

    def test_route_purposes_cannot_exceed_protocol_capabilities(self):
        def expand(catalog):
            row = next(row for row in catalog["presets"] if row["direction_id"] == "tencent-tokenhub-translation")
            row["allowed_purposes"].append("grammar")
        self.invalid_catalog(expand)

    def test_tokenhub_chat_route_cannot_silently_switch_protocol(self):
        def switch(catalog):
            row = next(row for row in catalog["presets"] if row["direction_id"] == "tencent-tokenhub-chat")
            row["protocol_id"] = "openai_responses"
        self.invalid_catalog(switch)

    def test_tokenhub_dedicated_route_must_keep_shared_provider_identity(self):
        def switch(catalog):
            row = next(row for row in catalog["presets"] if row["direction_id"] == "tencent-tokenhub-translation")
            row["provider_id"] = "different-provider"
        self.invalid_catalog(switch)

    def test_tmt_cannot_return_through_a_route_direction(self):
        self.invalid_catalog(lambda catalog: catalog["presets"][0].update(direction_id="tencent-legacy"))

    def test_tmt_cannot_return_as_a_protocol(self):
        self.invalid_catalog(lambda catalog: catalog["protocols"][0].update(id="tencent_tmt_legacy"))

    def test_explicit_tmt_exclusion_cannot_be_removed(self):
        self.invalid_catalog(lambda catalog: catalog["authority"].update(excluded_protocol_ids=[]))

    def test_protocol_adapter_and_live_acceptance_cannot_be_forged_passed(self):
        for field in ("adapter_acceptance", "live_acceptance"):
            with self.subTest(field=field):
                self.invalid_catalog(lambda catalog, field=field: catalog["protocols"][0].update({field: "passed"}))

    def test_route_acceptance_and_implementation_cannot_be_promoted(self):
        for field, value in (("acceptance_status", "passed"), ("implementation_status", "implemented")):
            with self.subTest(field=field):
                self.invalid_catalog(lambda catalog, field=field, value=value: catalog["presets"][0].update({field: value}))

    def test_new_acceptance_rows_must_all_remain_not_run(self):
        for status in ("passed", "blocked", "failed"):
            with self.subTest(status=status):
                self.invalid_catalog(lambda catalog, status=status: catalog["acceptance"][0].update(status=status))

    def test_duplicate_acceptance_identity_is_rejected(self):
        self.invalid_catalog(lambda catalog: catalog["acceptance"][1].update(id=catalog["acceptance"][0]["id"]))

    def test_documentation_phase_cannot_claim_runtime_implementation_or_acceptance(self):
        for field, value in (("registry", "implemented"), ("new_protocols", "implemented"),
                             ("typed_credential_migration", "implemented"), ("product_acceptance", "passed")):
            with self.subTest(field=field):
                self.invalid_catalog(lambda catalog, field=field, value=value: catalog["implementation_state"].update({field: value}))

    def test_missing_source_document_is_rejected(self):
        relative = self.catalog["source_documents"][-1]
        (self.root / relative).unlink()
        self.assertTrue(report.check(self.root))

    def test_source_document_reference_cannot_escape_the_fixture_root(self):
        self.invalid_catalog(lambda catalog: catalog["source_documents"].__setitem__(0, "../outside.md"))

    def test_generated_markdown_table_drift_is_rejected(self):
        path = self.root / PRIMARY
        original = path.read_text(encoding="utf-8")
        path.write_text(original.replace(self.catalog["protocols"][0]["id"], "edited-protocol-id", 1), encoding="utf-8")
        self.assertTrue(report.check(self.root))

    def test_duplicate_generated_marker_is_rejected(self):
        path = self.root / PRIMARY
        path.write_text(path.read_text(encoding="utf-8") + "\n" + PROTOCOL_START + "\n", encoding="utf-8")
        self.assertTrue(report.check(self.root))

    def test_missing_generated_marker_is_rejected(self):
        path = self.root / PRIMARY
        path.write_text(path.read_text(encoding="utf-8").replace(PRESET_END, "", 1), encoding="utf-8")
        self.assertTrue(report.check(self.root))

    def test_static_check_does_not_import_markdown_renderer(self):
        real_import = builtins.__import__
        def guarded_import(name, *args, **kwargs):
            if name == "markdown" or name.startswith("markdown."):
                raise AssertionError("Static --check attempted to import the optional renderer")
            return real_import(name, *args, **kwargs)
        with patch("builtins.__import__", side_effect=guarded_import):
            self.assertEqual(report.check(self.root), [])

    def test_report_refuses_an_existing_directory_without_touching_it(self):
        self.assertEqual(report.check(self.root), [])
        output = self.root / "reports/existing"
        output.mkdir(parents=True)
        sentinel = output / "preserve.txt"
        sentinel.write_bytes(b"historical report bytes\n")
        with self.assertRaises(ValueError):
            report.build_report(self.root, output)
        self.assertEqual(sentinel.read_bytes(), b"historical report bytes\n")

    def test_report_refuses_paths_outside_report_and_build_roots(self):
        self.assertEqual(report.check(self.root), [])
        for output in (self.root / "docs/replacement", self.root.parent / (self.root.name + "-outside")):
            with self.subTest(output=str(output)):
                with self.assertRaises(ValueError):
                    report.build_report(self.root, output)
                self.assertFalse(output.exists())

    def test_report_refuses_a_symlinked_output_parent(self):
        self.assertEqual(report.check(self.root), [])
        with tempfile.TemporaryDirectory(prefix="luna-model-service-docs-outside-") as directory:
            outside = Path(directory)
            (self.root / "reports").symlink_to(outside, target_is_directory=True)
            output = self.root / "reports/redirected-report"
            with self.assertRaises(ValueError):
                report.build_report(self.root, output)
            self.assertFalse((outside / "redirected-report").exists())

    def test_malformed_reference_and_capability_fields_return_rejections(self):
        mutations = [
            lambda catalog: catalog["protocols"][0].update(family=[]),
            lambda catalog: catalog["presets"][0].update(protocol_id=[]),
            lambda catalog: catalog["protocols"][0].update(allowed_purposes=[{}]),
            lambda catalog: catalog["implementation_state"].update(production_protocol_source=[{}]),
            lambda catalog: catalog["protocols"][0].update(official_source="https://["),
        ]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self.invalid_catalog(mutate)

    def test_export_rejects_source_change_after_checked_snapshot_before_writing(self):
        output = self.root / "reports/source-changed-before-write"
        source = self.root / self.catalog["source_documents"][1]
        def concurrent_change(*_args):
            source.write_text(source.read_text(encoding="utf-8") + "\nConcurrent source edit\n", encoding="utf-8")
            return SimpleNamespace(markdown=lambda _text, **_kwargs: "<p>controlled renderer</p>")
        with patch.object(report, "_markdown", side_effect=concurrent_change):
            with self.assertRaisesRegex(ValueError, "changed during export"):
                report.build_report(self.root, output)
        self.assertFalse(output.exists())

    def test_export_source_change_during_write_cannot_leave_a_passed_manifest(self):
        output = self.root / "reports/source-changed-during-write"
        source = self.root / self.catalog["source_documents"][1]
        original_write = Path.write_text
        def concurrent_change(path, text, *args, **kwargs):
            result = original_write(path, text, *args, **kwargs)
            if path == (output / "index.html").resolve():
                original_write(source, source.read_text(encoding="utf-8") + "\nConcurrent source edit\n", encoding="utf-8")
            return result
        renderer = SimpleNamespace(markdown=lambda _text, **_kwargs: "<p>controlled renderer</p>")
        with patch.object(report, "_markdown", return_value=renderer), patch.object(Path, "write_text", concurrent_change):
            with self.assertRaisesRegex(ValueError, "changed during export"):
                report.build_report(self.root, output)
        self.assertTrue((output / "index.html").is_file())
        self.assertFalse((output / "source-manifest.json").exists())


if __name__ == "__main__":
    unittest.main()
