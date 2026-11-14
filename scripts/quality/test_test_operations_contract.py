"""Meaningful positive and adversarial checks against copied real registries."""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).with_name("test_operations_contract.py")
SPEC = importlib.util.spec_from_file_location("test_operations_contract", SOURCE)
assert SPEC and SPEC.loader
contract = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(contract)
REPOSITORY = Path(__file__).resolve().parents[2]


class TestOperationsContractTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for relative in (contract.PLAN_PATH, contract.OPERATIONS_PATH):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPOSITORY / relative, target)
        self.ops = json.loads((self.root / contract.OPERATIONS_PATH).read_text())
        self.plan = json.loads((self.root / contract.PLAN_PATH).read_text())

    def errors(self):
        return contract.validate_operations(self.root, self.ops)

    def rejected(self, text):
        errors = self.errors()
        self.assertTrue(any(text in error for error in errors), errors)

    def test_current_inventory_covers_real_cases_and_combinations(self):
        self.assertEqual([], self.errors())
        for case in self.plan["cases"]:
            for variant in case["variant_ids"]:
                self.assertTrue(contract.dependencies_for(case, variant, self.ops), case["id"])
        self.assertIs(False, self.ops["product_tests_executed"])

    def test_applicability_or_between_rules(self):
        ops = {"prerequisites": [{"id": "PRE-X", "applies_to": [
            {"case_ids": ["OTHER"]}, {"variant_ids": ["REMOTE"]}]}]}
        self.assertEqual(["PRE-X"], [r["id"] for r in contract.dependencies_for(
            {"id": "ONE", "layer": "L6"}, "REMOTE", ops)])

    def test_applicability_and_between_fields(self):
        ops = {"prerequisites": [{"id": "PRE-X", "applies_to": [
            {"case_ids": ["ONE"], "variant_ids": ["REMOTE"]}]}]}
        self.assertEqual([], contract.dependencies_for({"id": "ONE"}, "LOCAL", ops))
        self.assertEqual([], contract.dependencies_for({"id": "OTHER"}, "REMOTE", ops))
        self.assertEqual(1, len(contract.dependencies_for({"id": "ONE"}, "REMOTE", ops)))

    def test_applicability_any_value_and_set_intersection(self):
        ops = {"prerequisites": [{"id": "PRE-X", "applies_to": [
            {"module_ids": ["M01", "M08"], "feature_ids": ["FP-07", "FP-08"],
             "layer_ids": ["L3", "L6"]}]}]}
        case = {"id": "ONE", "layer": "L6", "module_ids": ["M08", "M09"], "feature_ids": ["FP-07"]}
        self.assertEqual(1, len(contract.dependencies_for(case, "NATIVE", ops)))
        case["feature_ids"] = ["FP-01"]
        self.assertEqual([], contract.dependencies_for(case, "NATIVE", ops))

    def test_cli_receipt_static_scope_and_failure_exit(self):
        def run():
            return subprocess.run([sys.executable, str(SOURCE), "--root", str(self.root), "--json"],
                                  capture_output=True, text=True)
        result = run()
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual("static_test_operations_only", receipt["check_scope"])
        self.assertIs(False, receipt["product_tests_executed"])
        self.ops["phase"] = "release"
        (self.root / contract.OPERATIONS_PATH).write_text(json.dumps(self.ops))
        self.assertEqual(1, run().returncode)

    def test_missing_or_duplicate_prerequisite_rejected(self):
        original = deepcopy(self.ops)
        self.ops["prerequisites"].pop()
        self.rejected("missing or unknown IDs")
        self.ops = original
        self.ops["prerequisites"].append(deepcopy(self.ops["prerequisites"][0]))
        self.rejected("duplicate ID")

    def test_unknown_references_and_selectors_rejected(self):
        original = deepcopy(self.ops)
        for selector in contract.SELECTORS:
            with self.subTest(selector=selector):
                self.ops = deepcopy(original)
                self.ops["prerequisites"][0]["applies_to"] = [{selector: ["UNKNOWN"]}]
                self.rejected("unknown reference")
        self.ops = original
        self.ops["prerequisites"][0]["applies_to"] = [{"anything": ["ALL"]}]
        self.rejected("unknown selector rule")

    def test_empty_delivery_owner_gate_and_actions_rejected(self):
        original = deepcopy(self.ops)
        for field in ("deliverable", "user_action", "validation", "current_evidence", "title"):
            with self.subTest(field=field):
                self.ops = deepcopy(original)
                self.ops["prerequisites"][0][field] = "  "
                self.rejected("nonempty text")
        for field in ("owners", "required_by_gates"):
            self.ops = deepcopy(original)
            self.ops["prerequisites"][0][field] = []
            self.rejected("nonempty reference")

    def test_false_readiness_and_product_pass_claims_rejected(self):
        original = deepcopy(self.ops)
        for status in ("ready", "passed", "available", "verified"):
            self.ops = deepcopy(original)
            self.ops["prerequisites"][0]["status"] = status
            self.rejected("readiness or execution claim")
        for field, value in (("product_tests_executed", True), ("phase", "done"), ("schema_version", True)):
            self.ops = deepcopy(original)
            self.ops[field] = value
            self.assertTrue(self.errors())

    def test_empty_or_malformed_rules_cannot_mean_everything(self):
        original = deepcopy(self.ops)
        for rules in ([], [{}], [{"case_ids": []}], [{"case_ids": "TP-31"}]):
            self.ops = deepcopy(original)
            self.ops["prerequisites"][0]["applies_to"] = rules
            self.assertTrue(self.errors())
        self.assertEqual([], contract.dependencies_for({"id": "ONE"}, "ANY",
                         {"prerequisites": [{"applies_to": [{}]}]}))
        for key in ("prerequisites", "maintenance_areas", "case_maintenance", "rules"):
            self.ops = deepcopy(original)
            self.ops[key] = None
            self.assertTrue(self.errors())
        for field in ("id", "provider", "status"):
            self.ops = deepcopy(original)
            self.ops["prerequisites"][0][field] = []
            self.assertTrue(self.errors())

    def test_uncovered_execution_rejected(self):
        for row in self.ops["prerequisites"]:
            row["applies_to"] = [{"case_ids": ["TP-31"]}]
        self.rejected("missing prerequisite mapping")

    def test_internally_valid_but_unused_prerequisite_rejected(self):
        self.ops["prerequisites"][0]["applies_to"] = [
            {"case_ids": ["TP-31"], "variant_ids": ["GAME-SR"]}]
        self.rejected("no affected execution")

    def test_missing_or_duplicate_case_maintenance_rejected(self):
        original = deepcopy(self.ops)
        self.ops["case_maintenance"].pop()
        self.rejected("case_maintenance: missing or unknown IDs")
        self.ops = original
        self.ops["case_maintenance"].append(deepcopy(self.ops["case_maintenance"][0]))
        self.rejected("case_maintenance: duplicate ID")

    def test_unknown_or_wrong_layer_source_area_rejected(self):
        self.ops["case_maintenance"][0]["area_ids"] = ["MA-99"]
        self.rejected("unknown reference")
        self.ops["case_maintenance"][0]["area_ids"] = ["MA-02"]
        self.rejected("missing executable source area")

    def test_generated_reports_or_external_core_cannot_replace_source(self):
        original = deepcopy(self.ops)
        for area in ("MA-15", "MA-16", "MA-17"):
            self.ops = deepcopy(original)
            self.ops["case_maintenance"][0]["area_ids"] = [area]
            self.rejected("missing executable source area")
        self.ops = original
        self.ops["maintenance_areas"][0]["path"] = "reports/generated-tests.html"
        self.rejected("source maintenance path drift")

    def test_area_inventory_owner_and_layer_drift_rejected(self):
        original = deepcopy(self.ops)
        self.ops["maintenance_areas"].pop()
        self.rejected("maintenance_areas: missing or unknown IDs")
        self.ops = deepcopy(original)
        self.ops["maintenance_areas"][0]["layer_ids"] = ["L8"]
        self.rejected("source layer drift")
        self.ops = original
        self.ops["case_maintenance"][0]["module_owners"] = "UI"
        self.rejected("must match the case Owner")
        self.ops["case_maintenance"][0]["implementation_status"] = "implemented"
        self.rejected("must remain planned")

    def test_secret_fields_and_values_rejected_without_echoing_value(self):
        original = deepcopy(self.ops)
        self.ops["prerequisites"][0]["api_key"] = "placeholder"
        self.rejected("secret field")
        self.ops = original
        example = "sk-" + "SyntheticForGuardOnly0123456789"
        self.ops["prerequisites"][0]["deliverable"] = example
        self.rejected("possible raw secret")
        self.assertTrue(all(example not in error for error in self.errors()))

    def test_invalid_json_and_missing_inputs_fail_closed(self):
        (self.root / contract.OPERATIONS_PATH).write_text("{")
        self.assertTrue(contract.validate_operations(self.root))
        (self.root / contract.PLAN_PATH).unlink()
        self.assertTrue(contract.validate_operations(self.root, self.ops))

    def test_dual_display_and_key_pool_requirements_do_not_block_unrelated_runs(self):
        matches = {"PRE-16": [], "PRE-18": []}
        for case in self.plan["cases"]:
            for variant in case["variant_ids"]:
                for row in contract.dependencies_for(case, variant, self.ops):
                    if row["id"] in matches:
                        matches[row["id"]].append((case["id"], variant))
        self.assertEqual(3, len(matches["PRE-16"]))
        self.assertTrue(all(variant == "DISPLAYS" for _, variant in matches["PRE-16"]))
        self.assertEqual({"MC-04"}, {case for case, _ in matches["PRE-18"]})
        row = next(row for row in self.ops["prerequisites"] if row["id"] == "PRE-16")
        row["applies_to"] = [{"variant_ids": ["NATIVE", "DISPLAYS"]}]
        self.rejected("dual-display prerequisite must cover only DISPLAYS")

    def test_controlled_rule_and_protocol_tests_do_not_require_real_resource_artifacts(self):
        case = next(case for case in self.plan["cases"] if case["id"] == "TP-27")
        actual = {row["id"] for row in contract.dependencies_for(case, "UNIT", self.ops)}
        self.assertFalse(actual.intersection({"PRE-12", "PRE-17"}))
        row = next(row for row in self.ops["prerequisites"] if row["id"] == "PRE-12")
        row["applies_to"].append({"variant_ids": ["UNIT"]})
        self.rejected("real resources cannot block controlled UNIT/PROTOCOL")


if __name__ == "__main__":
    unittest.main()
