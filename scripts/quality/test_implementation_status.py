"""Isolated adversarial tests for status identity, never product acceptance."""
from __future__ import annotations

import copy
import csv
import hashlib
from html.parser import HTMLParser
import io
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest

from gate_core import digest
from implementation_status import (
    CATALOG, COMPONENTS, CURRENT_MANIFEST, MARKDOWN, MODULES, RW_ACCEPTANCE, SCOPE,
    _cell, product_source_identity, render, validate_catalog,
)


class ImplementationStatusTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="implementation-status-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.write(MODULES, "module_id,name,primary_owner,responsibility,target_paths,implementation_status\n"
                   "M01,fixture-module,AP,Preserve responsibility,crates/fixture,planned\n")
        self.write(COMPONENTS, "component_id,component_family,primary_owner,input_contract,state_ownership,implementation_status\n"
                   "C01,FixtureComponent,UI,Preserve contract,Local focus,planned\n")
        self.write(SCOPE, "| 编号 | 首版功能 | 行为 |\n| FP-01 | Fixture capability | Scope fixture |\n")
        self.write(RW_ACCEPTANCE, "case_id,expected\nRW-01,Guard fixture requirement\n")
        self.source = "crates/fixture/src/lib.rs"
        self.write(self.source, '// Guard fixture only; no product/runtime acceptance.\npub const FIXTURE: &str = "inventory";\n')
        self.history = ".build/history/fixture.json"
        self.write(self.history, '{"kind":"historical guard evidence fixture"}\n')
        current_artifact = self.artifact_fixture()
        self.catalog = {
            "schema_version": 1, "audit_date": "2026-10-20",
            "audit_scope": "source_inspection_with_historical_evidence",
            "runtime_verified_this_audit": False, "current_release_approved": False,
            "current_artifact": current_artifact,
            "product_sources_sha256": product_source_identity(self.root),
            "modules": [self.record("M01")],
            "components": [self.record("C01")],
            "capabilities": [self.record("FP-01")],
        }
        self.save()
        self.export()

    def write(self, relative: str, text: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def record(self, identity: str) -> dict:
        return {
            "id": identity, "name": "Inventory guard fixture",
            "implementation_status": "partial", "alignment_status": "partial",
            "verification_status": "source_only", "visual_alignment_status": "not_assessed",
            "current_behavior": "A source fixture is present.",
            "target_behavior": "Validate inventory identity without product claims.",
            "gap": "This fixture does not execute a product.",
            "next_step": "Continue independent product verification.", "owner": "QA",
            "requirement_ids": ["FP-01", "RW-01"],
            "module_ids": ["M01"], "component_ids": ["C01"],
            "source_evidence": [{"path": self.source, "line": 2,
                                 "quote": 'pub const FIXTURE: &str = "inventory";',
                                 "sha256": digest(self.root / self.source)}],
            "historical_evidence": [],
        }

    def artifact_fixture(self) -> dict:
        """Create byte identities only; these files are deliberately not runnable."""
        app = self.root / "dist/Fixture.app"
        archive = self.root / "dist/inventory-fixture.zip"
        executable = app / "Contents/MacOS/inventory-fixture"
        self.write(executable.relative_to(self.root).as_posix(), "Not a product executable.\n")
        self.write(archive.relative_to(self.root).as_posix(), "Not a product archive; hash fixture only.\n")
        info = {"CFBundleExecutable": executable.name, "CFBundleShortVersionString": "fixture-0"}
        self.write((app / "Contents/Info.plist").relative_to(self.root).as_posix(),
                   plistlib.dumps(info).decode())
        artifact = {
            "version": "fixture-0",
            "source_tree_sha256": hashlib.sha256(b"source identity guard fixture").hexdigest(),
            "executable_sha256": digest(executable), "archive_sha256": digest(archive),
            "distribution_status": "development_candidate_not_release_approved",
        }
        self.write((app / "Contents/Resources/luna-source-identity.json").relative_to(self.root).as_posix(),
                   json.dumps({"source_tree_sha256": artifact["source_tree_sha256"], "version": artifact["version"]}))
        self.write(CURRENT_MANIFEST, json.dumps({**artifact, "app": str(app), "archive": str(archive)}))
        return artifact

    def save(self) -> None:
        self.write(CATALOG, json.dumps(self.catalog, ensure_ascii=False))

    def export(self) -> None:
        for path, content in render(self.root, self.catalog).items():
            self.write(path, content)

    def assert_rejected(self, phrase: str) -> None:
        self.save()
        errors = validate_catalog(self.root, check_exports=False)
        self.assertTrue(any(phrase in error for error in errors), errors)

    def test_valid_inventory_and_deterministic_export_preserve_matrix_contract(self) -> None:
        self.assertEqual([], validate_catalog(self.root))
        outputs = render(self.root, self.catalog)
        self.assertEqual(outputs, render(self.root, copy.deepcopy(self.catalog)))
        module = next(csv.DictReader(io.StringIO(outputs[MODULES])))
        component = next(csv.DictReader(io.StringIO(outputs[COMPONENTS])))
        self.assertEqual("M01", module["module_id"])
        self.assertEqual("AP", module["primary_owner"])
        self.assertEqual("Preserve responsibility", module["responsibility"])
        self.assertEqual("crates/fixture", module["target_paths"])
        self.assertEqual("Preserve contract", component["input_contract"])
        self.assertEqual("Local focus", component["state_ownership"])
        self.assertEqual("partial", module["implementation_status"])
        self.assertIn("当前实机：未执行；视觉：未评估", outputs[MARKDOWN])
        self.assertIn("<details><summary>查看 1 处源码</summary>", outputs[MARKDOWN])
        self.assertIn('href="../../crates/fixture/src/lib.rs#L2"', outputs[MARKDOWN])
        self.catalog["capabilities"].append(self.record("CAP-PREVIEW"))
        self.save()
        self.export()
        self.assertEqual([], validate_catalog(self.root))
        self.assertIn("CAP-PREVIEW", render(self.root, self.catalog)[MARKDOWN])

    def test_invalid_status_and_malformed_values_fail_without_runtime_claims(self) -> None:
        for key, value in (("implementation_status", "complete"),
                           ("alignment_status", []), ("verification_status", "passed"),
                           ("visual_alignment_status", "aligned")):
            with self.subTest(key=key):
                saved = copy.deepcopy(self.catalog)
                self.catalog["modules"][0][key] = value
                self.assert_rejected(key)
                self.catalog = saved

    def test_each_authority_group_must_be_complete_without_duplicate_ids(self) -> None:
        for group in ("modules", "components", "capabilities"):
            with self.subTest(group=group):
                saved = copy.deepcopy(self.catalog)
                self.catalog[group] = []
                self.assert_rejected(f"{group} authority closure mismatch")
                self.catalog = saved
        self.catalog["modules"].append(copy.deepcopy(self.catalog["modules"][0]))
        self.assert_rejected("Duplicate modules")
        self.catalog["modules"].pop()
        capability = self.record("CAP-PREVIEW")
        self.catalog["capabilities"].append(capability)
        for requirements in ([], ["RW-01"]):
            with self.subTest(requirements=requirements):
                capability["requirement_ids"] = requirements
                self.assert_rejected("capability requires at least one known FP")
        capability["requirement_ids"] = ["FP-99"]
        self.assert_rejected("unknown or duplicate requirement_ids")
        capability["requirement_ids"] = ["FP-01"]
        capability["id"] = "CAP-lowercase"
        self.assert_rejected("capabilities authority closure mismatch")
        capability["id"] = "FP-99"
        self.assert_rejected("capabilities authority closure mismatch")

    def test_full_product_source_closure_detects_addition_deletion_and_changes(self) -> None:
        original = (self.root / self.source).read_text()
        self.write("resources/new.txt", "new product resource")
        self.assert_rejected("Product source closure drift")
        (self.root / "resources/new.txt").unlink()
        (self.root / self.source).unlink()
        self.assert_rejected("Product source closure drift")
        self.write(self.source, original + "// changed\n")
        self.assert_rejected("Product source closure drift")
        self.write(self.source, original)
        self.write("dist/package.txt", "generated package excluded from product closure")
        self.assertEqual([], validate_catalog(self.root))

    def test_source_evidence_requires_current_hash_and_exact_real_line(self) -> None:
        evidence = self.catalog["modules"][0]["source_evidence"][0]
        evidence["line"] = 1
        self.assert_rejected("source line/quote mismatch")
        evidence["line"] = 2
        evidence["sha256"] = "0" * 64
        self.assert_rejected("evidence SHA-256 drift")
        evidence["sha256"] = digest(self.root / self.source)
        evidence["path"] = "../escape.rs"
        self.assert_rejected("invalid evidence path")
        self.write("docs/self-assertion.md", "This document claims implementation.\n")
        evidence.update({"path": "docs/self-assertion.md", "line": 1,
                         "quote": "This document claims implementation.",
                         "sha256": digest(self.root / "docs/self-assertion.md")})
        self.assert_rejected("not audit prose")

    def test_current_runtime_or_release_success_is_explicitly_rejected(self) -> None:
        self.catalog["runtime_verified_this_audit"] = True
        self.assert_rejected("runtime_verified_this_audit")
        self.catalog["runtime_verified_this_audit"] = False
        self.catalog["current_release_approved"] = True
        self.assert_rejected("current_release_approved")
        self.catalog["current_release_approved"] = False
        self.catalog["modules"][0]["runtime_passed"] = True
        self.assert_rejected("cannot claim current runtime/release success")

    def test_historical_status_requires_hashed_historical_only_artifact(self) -> None:
        record = self.catalog["modules"][0]
        record["verification_status"] = "historical_mixed"
        self.assert_rejected("requires historical_evidence")
        historical = {"path": self.history, "sha256": digest(self.root / self.history),
                      "scope": "current", "claim": "Guard fixture protocol history only."}
        record["historical_evidence"] = [historical]
        self.assert_rejected("scope must be historical_only")
        historical["scope"] = "historical_only"
        self.save()
        self.assertEqual([], validate_catalog(self.root, check_exports=False))
        self.write(self.history, '{"kind":"changed guard fixture"}\n')
        self.assert_rejected("evidence SHA-256 drift")

    def test_incomplete_or_mismatched_records_need_a_gap_and_known_references(self) -> None:
        record = self.catalog["modules"][0]
        record["gap"] = ""
        self.assert_rejected("requires a gap")
        record["gap"] = "Known gap"
        for field, value in (("requirement_ids", ["FP-99"]),
                             ("module_ids", ["M99"]), ("component_ids", ["C99"])):
            with self.subTest(field=field):
                saved = record[field]
                record[field] = value
                self.assert_rejected(f"unknown or duplicate {field}")
                record[field] = saved
        record["next_step"] = "Consult RW-99."
        self.assert_rejected("unknown references in next_step")

    def test_export_drift_is_rejected_without_implicit_writes(self) -> None:
        self.write(MARKDOWN, "Unreviewed manual export edit\n")
        before = (self.root / MARKDOWN).read_bytes()
        self.assertTrue(any("export drift" in item for item in validate_catalog(self.root)))
        self.assertEqual(before, (self.root / MARKDOWN).read_bytes())
        self.assertEqual([], validate_catalog(self.root, check_exports=False))

    def test_audit_text_is_escaped_without_creating_html_controls(self) -> None:
        literal = '<select><option>Choice</option></select>\nA|B & C > D'
        self.assertEqual(
            '&lt;select&gt;&lt;option&gt;Choice&lt;/option&gt;&lt;/select&gt;<br>A\\|B &amp; C &gt; D',
            _cell(literal),
        )
        injected = '<script>alert("audit text")</script><input value="literal">'
        for group in ("modules", "components", "capabilities"):
            record = self.catalog[group][0]
            record["gap"] = literal
            record["current_behavior"] = injected
        output = render(self.root, self.catalog)
        markdown = output[MARKDOWN]
        tags = []

        class Elements(HTMLParser):
            def handle_starttag(self, tag: str, attrs: list) -> None:
                tags.append(tag)

        Elements().feed(markdown)
        self.assertFalse({"select", "option", "script", "input"}.intersection(tags))
        self.assertIn("br", tags, "Generated presentation line breaks must remain markup")
        self.assertIn("&lt;select&gt;", markdown)
        self.assertIn("&lt;script&gt;", markdown)
        self.assertIn('>crates/fixture/src/lib.rs:2</a>', markdown)
        module = next(csv.DictReader(io.StringIO(output[MODULES])))
        self.assertEqual(literal, module["remaining_gap"], "The audit text itself must remain intact")

    def test_current_artifact_cannot_be_relabelled_as_an_older_or_different_package(self) -> None:
        current = self.catalog["current_artifact"]
        for field, wrong in (("source_tree_sha256", "1" * 64),
                             ("executable_sha256", "2" * 64), ("archive_sha256", "3" * 64),
                             ("version", "older-fixture"), ("distribution_status", "release_approved")):
            with self.subTest(field=field):
                original = current[field]
                current[field] = wrong
                self.assert_rejected(f"current_artifact.{field}")
                current[field] = original
        current["archive_path"] = "dist/another-package.zip"
        self.assert_rejected("archive_path: differs from actual distribution path")
        del current["archive_path"]
        self.write("dist/inventory-fixture.zip", "Actual other package bytes.\n")
        self.assert_rejected("archive_sha256: actual distribution bytes differ")
        self.write("dist/inventory-fixture.zip", "Not a product archive; hash fixture only.\n")
        self.write("dist/Fixture.app/Contents/MacOS/inventory-fixture", "Actual other executable bytes.\n")
        self.assert_rejected("executable_sha256: actual distribution bytes differ")

    def test_audit_inputs_and_historical_language_provenance_require_real_identity(self) -> None:
        self.catalog["audit_inputs"] = [{"path": self.history, "sha256": "0" * 64}]
        self.assert_rejected("audit_inputs[0]: evidence SHA-256 drift")
        self.catalog["audit_inputs"][0]["sha256"] = digest(self.root / self.history)
        directory = ".build/language-history-fixture"
        summary = directory + "/summary.json"
        output = directory + "/worker.stdout.jsonl"
        self.write(summary, json.dumps({"quality_cases": {"fixture-case": "failed_retained"}}))
        self.write(output, '{"kind":"retained worker output fixture"}\n')
        self.write(directory + "/file-manifest.json", json.dumps([
            {"path": Path(path).name, "sha256": digest(self.root / path)} for path in (summary, output)
        ]))
        self.catalog["historical_evidence"] = [{"path": summary, "sha256": digest(self.root / summary),
                                                 "scope": "historical_only", "claim": "Historical protocol fixture only."}]
        detail = {"path": summary, "status": "historical_failed", "current_status": "not_run",
                  "json_pointers": ["/quality_cases/fixture-case"],
                  "cases": [{"id": "fixture-case", "status": "failed_retained",
                             "observed": "Human note", "expected_semantics": "Human review only"}],
                  "actual_output": output}
        self.catalog["historical_language_quality_detail"] = detail
        self.save()
        self.assertEqual([], validate_catalog(self.root, check_exports=False))
        detail["cases"][0]["status"] = "passed"
        self.assert_rejected("case/status differs from historical JSON pointer")
        detail["cases"][0]["status"] = "failed_retained"
        self.write(output, '{"kind":"changed retained output fixture"}\n')
        self.assert_rejected("language provenance: evidence SHA-256 drift")

    def test_cli_defaults_read_only_and_write_never_touches_catalog_or_baseline(self) -> None:
        script = Path(__file__).with_name("implementation_status.py")
        self.write(MARKDOWN, "Stale derived output\n")
        self.write("docs/development/iteration-baseline.json", '{"fixture":"must stay unchanged"}\n')
        protected = {path: digest(self.root / path) for path in (
            CATALOG, "docs/development/iteration-baseline.json",
        )}
        command = [sys.executable, str(script), "--root", str(self.root)]
        check = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertEqual(1, check.returncode, check.stdout + check.stderr)
        self.assertEqual("Stale derived output\n", (self.root / MARKDOWN).read_text())
        write = subprocess.run([*command, "--write"], capture_output=True, text=True, check=False)
        self.assertEqual(0, write.returncode, write.stdout + write.stderr)
        self.assertEqual([], validate_catalog(self.root))
        self.assertEqual(protected, {path: digest(self.root / path) for path in protected})


if __name__ == "__main__":
    unittest.main()
