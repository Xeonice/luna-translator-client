"""Mutation tests for report evidence validation using synthetic guard fixtures.

The miniature receipts and logs here test rejection logic. They are never
published as product acceptance or copied from a successful repository run.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "report"))
# The source-tree-hash mock omits its real-repository sys.path side effect;
# import the actual log validator from tooling, without copying it into fixtures.
sys.path.insert(0, str(ROOT / "scripts" / "quality"))
import build_engineering_baseline_report as report


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class ReportEvidenceTests(unittest.TestCase):
    REQUIRED_STEPS = (
        "change-contract", "baseline-integrity", "source-scope",
        "database-design-contract", "database-design-export-identity",
        "database-design-tests", "typescript-compile", "rust-compile",
        "scenario-check-execution", "storage-schema-contract", "storage-boundaries",
        "storage-boundary-execution", "storage-acceptance-contract", "storage-schema-tests",
        "storage-boundary-tests", "storage-acceptance-tests",
    )

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="luna-report-proof-guard-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.current_hash = sha256(b"synthetic-current-source-tree")
        self.baseline_path = self.root / "docs/development/iteration-baseline.json"
        self.baseline_path.parent.mkdir(parents=True)
        self.baseline_bytes = (json.dumps({"baseline_id": "SYNTHETIC-BASELINE"}) + "\n").encode()
        self.baseline_path.write_bytes(self.baseline_bytes)
        self.receipt_path = self.root / ".build/quality/synthetic-receipt.json"
        self.receipt_path.parent.mkdir(parents=True)
        self.summary_path = self.root / ".build/quality/synthetic-summary.json"
        steps = []
        for step_id in self.REQUIRED_STEPS:
            log = self.root / f".build/quality/{step_id}.log"
            log.write_text("", encoding="utf-8")
            steps.append({"id": step_id, "status": "passed", "errors": [], "exit_code": 0,
                          "test_count": None, "command": ["synthetic-guard-fixture", step_id],
                          "log": str(log.relative_to(self.root))})
        database_step = next(step for step in steps if step["id"] == "database-design-tests")
        database_step.update(command=[sys.executable, "-m", "unittest", "-v",
                                      "test_database_design.DatabaseDesignTests"], test_count=3)
        (self.root / database_step["log"]).write_text(self.unittest_log(3), encoding="utf-8")
        guard_log = self.root / ".build/quality/synthetic-other-guards.log"
        guard_log.write_text(self.unittest_log(2), encoding="utf-8")
        steps.append({"id": "synthetic-other-guards", "status": "passed", "errors": [],
                      "exit_code": 0, "test_count": 2,
                      "command": [sys.executable, "-m", "unittest", "-v", "synthetic_guard.Tests"],
                      "log": str(guard_log.relative_to(self.root))})
        self.receipt = {"schema_version": 1, "profile": "development", "status": "passed",
                        "source_tree_sha256": self.current_hash,
                        "product_runtime_tests_executed": False, "steps": steps}
        self.summary = {"status": "passed", "receipt_path": str(self.receipt_path.relative_to(self.root)),
                        "source_tree_sha256": self.current_hash,
                        "baseline_id": "SYNTHETIC-BASELINE", "baseline_sha256": sha256(self.baseline_bytes),
                        "database_design_tests": 3, "all_engineering_guard_tests": 5,
                        "product_tests_executed": False}
        for check_id, suite in [("storage-schema-tests","test_storage_schema.StorageSchemaTests"), ("storage-boundary-tests","test_storage_boundaries.StorageBoundaryTests"), ("storage-acceptance-tests","test_storage_acceptance.StorageAcceptanceTests")]:
            step = next(row for row in self.receipt["steps"] if row["id"] == check_id)
            step.update(command=[sys.executable,"-m","unittest","-v",suite],test_count=1)
            (self.root / step["log"]).write_text(self.unittest_log(1))
        registry = {"probes":[{"id":"synthetic.field.probe"}]}
        registry_file = self.root / "docs/architecture/database/field-boundaries.json"
        registry_file.parent.mkdir(parents=True)
        registry_file.write_text(json.dumps(registry))
        self.boundary = {"registry_sha256":sha256(json.dumps(registry,ensure_ascii=False,sort_keys=True,separators=(",", ":")).encode()), "status":"passed", "errors":[], "not_product_acceptance":True, "checks":[{"id":"synthetic.field.probe","status":"passed","skipped":False,"assertions":3}]}
        self.boundary_file = self.root / ".build/quality/synthetic-boundary.json"
        self.boundary_file.write_text(json.dumps(self.boundary))
        step = next(row for row in self.receipt["steps"] if row["id"] == "storage-boundary-execution")
        step.update(evidence_path=str(self.boundary_file.relative_to(self.root)),evidence_sha256=sha256(self.boundary_file.read_bytes()),executed_probes=1,assertions=3)
        self.summary.update(storage_boundary_probes=1,storage_boundary_assertions=3,product_storage_acceptance="blocked",all_engineering_guard_tests=8)
        self.save()

    @staticmethod
    def unittest_log(count):
        cases = "".join(f"test_fixture_{index} (synthetic_guard.Tests.test_fixture_{index}) ... ok\n"
                        for index in range(count))
        return ("SYNTHETIC GUARD FIXTURE; NO PRODUCT ACCEPTANCE\n" + cases
                + f"\n----------------------------------------------------------------------\nRan {count} tests in 0.001s\n\nOK\n")

    def save(self, *, update_receipt_hash=True):
        self.receipt_path.write_text(json.dumps(self.receipt, indent=2) + "\n", encoding="utf-8")
        if update_receipt_hash:
            self.summary["receipt_sha256"] = sha256(self.receipt_path.read_bytes())
        self.summary_path.write_text(json.dumps(self.summary, indent=2) + "\n", encoding="utf-8")

    def validate(self):
        with patch.object(report, "current_source_tree_hash", return_value=self.current_hash) as current:
            result = report.validate_current_database_proof(self.root, self.summary_path)
            current.assert_called_once_with(self.root.resolve())
            return result

    def assert_invalid(self):
        with patch.object(report, "current_source_tree_hash", return_value=self.current_hash):
            with self.assertRaises(ValueError):
                report.validate_current_database_proof(self.root, self.summary_path)

    def test_current_matching_proof_is_accepted_with_exact_hash_steps_and_counts(self):
        result = self.validate()
        self.assertEqual(sha256(self.receipt_path.read_bytes()), result["receipt_sha256"])
        self.assertEqual(sha256(self.baseline_path.read_bytes()), result["baseline_sha256"])
        self.assertEqual(self.current_hash, result["source_tree_sha256"])
        self.assertEqual(3, result["database_design_tests"])
        self.assertEqual(8, result["all_engineering_guard_tests"])
        actual_ids = {step["id"] for step in self.receipt["steps"]}
        self.assertTrue(set(self.REQUIRED_STEPS).issubset(actual_ids))
        self.assertFalse(result["product_tests_executed"])

    def test_failed_summary_or_receipt_is_rejected(self):
        baseline_summary, baseline_receipt = copy.deepcopy(self.summary), copy.deepcopy(self.receipt)
        for object_name in ("summary", "receipt"):
            with self.subTest(object=object_name):
                self.summary, self.receipt = copy.deepcopy(baseline_summary), copy.deepcopy(baseline_receipt)
                getattr(self, object_name)["status"] = "failed"
                self.save()
                self.assert_invalid()

    def test_old_source_is_rejected_even_when_summary_and_receipt_agree(self):
        obsolete = sha256(b"synthetic-obsolete-source-tree")
        self.receipt["source_tree_sha256"] = obsolete
        self.summary["source_tree_sha256"] = obsolete
        self.save()
        self.assert_invalid()

    def test_receipt_hash_mismatch_is_rejected(self):
        self.receipt["run_note"] = "changed after summary was signed"
        self.save(update_receipt_hash=False)
        self.assert_invalid()

    def test_every_required_database_and_compile_step_must_exist(self):
        steps = copy.deepcopy(self.receipt["steps"])
        for step_id in self.REQUIRED_STEPS:
            with self.subTest(missing=step_id):
                self.receipt["steps"] = [copy.deepcopy(step) for step in steps if step["id"] != step_id]
                self.save()
                self.assert_invalid()

    def test_count_mismatches_cannot_be_reported_as_passed(self):
        original = copy.deepcopy(self.summary)
        for field in ("database_design_tests", "all_engineering_guard_tests"):
            with self.subTest(field=field):
                self.summary = copy.deepcopy(original)
                self.summary[field] += 1
                self.save()
                self.assert_invalid()
        self.summary = original
        step = next(item for item in self.receipt["steps"] if item["id"] == "database-design-tests")
        step["test_count"] = 4
        self.summary.update(database_design_tests=4, all_engineering_guard_tests=6)
        self.save()
        self.assert_invalid()  # Reconciled JSON still disagrees with the actual log's three tests.

    def test_baseline_id_or_hash_mismatch_is_rejected(self):
        original = copy.deepcopy(self.summary)
        for field, invalid in (("baseline_id", "OTHER-SYNTHETIC-BASELINE"),
                               ("baseline_sha256", sha256(b"another baseline"))):
            with self.subTest(field=field):
                self.summary = copy.deepcopy(original)
                self.summary[field] = invalid
                self.save()
                self.assert_invalid()
        self.summary = original
        self.save()
        self.baseline_path.write_bytes(self.baseline_bytes + b" ")
        self.assert_invalid()

    def test_product_runtime_status_cannot_be_promoted_by_engineering_report(self):
        original_summary, original_receipt = copy.deepcopy(self.summary), copy.deepcopy(self.receipt)
        for object_name, field in (("summary", "product_tests_executed"),
                                   ("receipt", "product_runtime_tests_executed")):
            with self.subTest(object=object_name):
                self.summary, self.receipt = copy.deepcopy(original_summary), copy.deepcopy(original_receipt)
                getattr(self, object_name)[field] = True
                self.save()
                self.assert_invalid()

    def test_failed_steps_errors_exit_codes_and_failed_logs_are_rejected(self):
        original = copy.deepcopy(self.receipt)
        for field, value in (("status", "failed"), ("errors", ["synthetic failure"]), ("exit_code", 1)):
            with self.subTest(field=field):
                self.receipt = copy.deepcopy(original)
                step = next(item for item in self.receipt["steps"] if item["id"] == "database-design-tests")
                step[field] = value
                self.save()
                self.assert_invalid()
        self.receipt = original
        self.save()
        step = next(item for item in self.receipt["steps"] if item["id"] == "database-design-tests")
        (self.root / step["log"]).write_text("Ran 3 tests in 0.001s\n\nFAILED (failures=1)\n", encoding="utf-8")
        self.assert_invalid()

    def test_database_step_must_execute_the_registered_database_suite(self):
        step = next(item for item in self.receipt["steps"] if item["id"] == "database-design-tests")
        step["command"] = [sys.executable, "-m", "unittest", "-v", "unrelated.Tests"]
        self.save()
        self.assert_invalid()

    def test_positive_test_count_requires_existing_log_and_real_success_summary(self):
        step = next(item for item in self.receipt["steps"] if item["id"] == "database-design-tests")
        log = self.root / step["log"]
        log.unlink()
        self.assert_invalid()
        log.write_text("SYNTHETIC RECEIPT: no tests actually executed\n", encoding="utf-8")
        self.assert_invalid()

    def test_duplicate_step_ids_are_rejected_instead_of_overwriting_failure(self):
        duplicate = copy.deepcopy(next(item for item in self.receipt["steps"] if item["id"] == "database-design-tests"))
        duplicate.update(status="failed", errors=["synthetic conflicting duplicate"])
        self.receipt["steps"].insert(0, duplicate)
        self.save()
        self.assert_invalid()

    def test_storage_boundary_receipt_cannot_be_missing_skipped_or_relabelled(self):
        original = copy.deepcopy(self.boundary)
        for mutation in (lambda d: d.update(status="failed"), lambda d: d["checks"].clear(), lambda d: d["checks"][0].update(skipped=True), lambda d: d["checks"][0].update(assertions=0), lambda d: d.update(registry_sha256="0"*64)):
            self.boundary = copy.deepcopy(original)
            mutation(self.boundary)
            self.boundary_file.write_text(json.dumps(self.boundary))
            step = next(row for row in self.receipt["steps"] if row["id"] == "storage-boundary-execution")
            step["evidence_sha256"] = sha256(self.boundary_file.read_bytes())
            self.save()
            self.assert_invalid()

    def test_storage_approval_cannot_replace_blocked_product_state(self):
        self.summary["product_storage_acceptance"] = "passed"
        self.save()
        self.assert_invalid()


if __name__ == "__main__":
    unittest.main()
