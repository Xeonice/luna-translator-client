"""Adversarial checks over a copied real plan and authoritative source files."""

from __future__ import annotations

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

_SOURCE = Path(__file__).with_name("test_plan_contract.py")
_SPEC = importlib.util.spec_from_file_location("quality_test_plan_contract", _SOURCE)
assert _SPEC and _SPEC.loader
contract = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(contract)
REPOSITORY = Path(__file__).resolve().parents[2]


class TestPlanContractTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for relative in [contract.PLAN_PATH, *contract.SOURCE_FILES,
                         "docs/architecture/engineering-modules.csv",
                         "docs/architecture/engineering-components.csv"]:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPOSITORY / relative, target)
        self.document = json.loads((self.root / contract.PLAN_PATH).read_text())

    def errors(self):
        return contract.validate_plan(self.root, self.document)

    def assert_rejected(self, fragment):
        errors = self.errors()
        self.assertTrue(any(fragment in error for error in errors), errors)

    def new_case(self):
        return next(case for case in self.document["cases"] if case["source"] is None)

    def test_current_real_plan_passes_without_executing_product(self):
        self.assertEqual([], self.errors())
        self.assertEqual("NOT_RUN", self.document["product_execution"])
        self.assertTrue(all(case["status"] == "not_run" for case in self.document["cases"]))

    def test_required_execution_inventory_expands_exact_case_variant_pairs(self):
        inventory = contract.required_executions(self.document)
        expected = set()
        for case in self.document["cases"]:
            for variant in case["variant_ids"]:
                expected.add(case["id"] + "@" + variant)
        self.assertEqual(expected, inventory)
        self.assertGreaterEqual(len(inventory), len(self.document["cases"]))
        optional = deepcopy(self.document)
        optional["cases"][0]["release_required"] = False
        for variant in optional["cases"][0]["variant_ids"]:
            self.assertNotIn(optional["cases"][0]["id"] + "@" + variant,
                             contract.required_executions(optional))

    def test_cli_reports_only_static_plan_validation(self):
        result = subprocess.run([sys.executable, str(_SOURCE), "--root", str(self.root), "--json"],
                                capture_output=True, text=True)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual("static_test_plan_only", receipt["check_scope"])
        self.assertIs(False, receipt["product_tests_executed"])

    def test_schema_boolean_and_execution_claim_are_rejected(self):
        original = deepcopy(self.document)
        for field, value, message in [("schema_version", True, "schema_version"),
                                      ("phase", "release", "phase"),
                                      ("product_execution", "PASSED", "product_execution")]:
            with self.subTest(field=field):
                self.document = deepcopy(original)
                self.document[field] = value
                self.assert_rejected(message)

    def test_duplicate_case_and_environment_id_are_rejected(self):
        self.document["cases"].append(deepcopy(self.document["cases"][0]))
        self.assert_rejected("duplicate ID")
        self.document["environments"].append(deepcopy(self.document["environments"][0]))
        self.assert_rejected("environments: duplicate ID")

    def test_unknown_feature_and_ai_requirements_are_rejected(self):
        self.new_case()["feature_ids"] = ["FP-99"]
        self.assert_rejected("unknown reference (FP-99)")
        self.new_case()["ai_config_ids"] = ["AI-CFG-99"]
        self.assert_rejected("unknown reference (AI-CFG-99)")

    def test_missing_first_product_feature_coverage_is_rejected(self):
        for case in self.document["cases"]:
            case["feature_ids"] = [value for value in case["feature_ids"] if value != "FP-12"]
        self.assert_rejected("missing coverage (FP-12)")

    def test_missing_model_configuration_coverage_is_rejected(self):
        for case in self.document["cases"]:
            case["ai_config_ids"] = [value for value in case["ai_config_ids"] if value != "AI-CFG-16"]
        self.assert_rejected("missing coverage (AI-CFG-16)")

    def test_source_byte_drift_is_rejected_even_when_ids_unchanged(self):
        path = self.root / next(iter(contract.SOURCE_FILES))
        path.write_bytes(path.read_bytes() + b"\n")
        self.assert_rejected("source fingerprint mismatch")

    def test_source_id_inventory_cannot_silently_drop_an_acceptance(self):
        self.document["sources"][0]["case_ids"].pop()
        self.assert_rejected("exactly match eligible authoritative IDs")

    def test_each_original_case_must_be_imported_once(self):
        inherited = next(case for case in self.document["cases"] if case["source"] is not None)
        self.document["cases"].remove(inherited)
        self.assert_rejected("authoritative case must be imported exactly once")

    def test_imported_case_cannot_be_renamed_to_hide_missing_coverage(self):
        inherited = next(case for case in self.document["cases"] if case["source"] is not None)
        inherited["id"] = "TP-RELABELED"
        self.assert_rejected("preserve their authoritative source ID")

    def test_duplicate_source_reference_with_new_case_id_is_rejected(self):
        inherited = next(case for case in self.document["cases"] if case["source"] is not None)
        duplicate = deepcopy(inherited)
        duplicate["id"] = "TP-DUPLICATE-SOURCE"
        self.document["cases"].append(duplicate)
        self.assert_rejected("authoritative case must be imported exactly once")

    def test_m3_cannot_be_added_to_first_product_inventory(self):
        case = self.new_case()
        case["id"] = "CR-11-REQ-007"
        case["source"] = {"path": "docs/development/client-regression.csv", "id": "CR-11-REQ-007"}
        self.assert_rejected("M3 is not first-product scope")

    def test_ever17_game_case_is_out_of_scope(self):
        self.new_case()["title"] = "Ever17 正式包游戏取文"
        self.assert_rejected("Ever17 is excluded")

    def test_new_case_needs_known_module_and_component_mappings(self):
        case = self.new_case()
        case["module_ids"] = ["M99"]
        self.assert_rejected("unknown reference (M99)")
        case["component_ids"] = ["C99"]
        self.assert_rejected("unknown reference (C99)")

    def test_layer_dependency_cycle_is_rejected(self):
        layers = {row["id"]: row for row in self.document["layers"]}
        layers["L0"]["depends_on"] = ["L1"]
        layers["L1"]["depends_on"] = ["L0"]
        self.assert_rejected("dependency cycle")

    def test_missing_layer_and_unknown_dependency_are_rejected(self):
        self.document["layers"].pop()
        self.assert_rejected("exactly L0 through L8")
        self.document["layers"][0]["depends_on"] = ["L99"]
        self.assert_rejected("unknown reference (L99)")

    def test_unknown_environment_variant_and_scope_are_rejected(self):
        self.document["variants"][0]["environment_ids"] = ["MISSING-ENV"]
        self.assert_rejected("unknown reference (MISSING-ENV)")
        self.new_case()["variant_ids"] = ["MISSING-VARIANT"]
        self.assert_rejected("unknown reference (MISSING-VARIANT)")
        self.document["variants"][0]["evidence_scope"] = "any_package"
        self.assert_rejected("expected source_build or final_package")

    def test_missing_steps_and_preconditions_are_rejected(self):
        self.new_case()["steps"] = []
        self.assert_rejected("steps: expected a nonempty")
        self.new_case()["preconditions"] = []
        self.assert_rejected("preconditions: expected a nonempty")

    def test_plan_cannot_premark_product_case_passed_or_harness_ready(self):
        case = self.new_case()
        case["status"] = "passed"
        case["harness_status"] = "implemented"
        case["release_required"] = False
        self.assert_rejected("planning record must remain not_run")
        self.assert_rejected("planning record must remain planned")
        self.assert_rejected("first-product plan cases must be required")

    def test_old_visual_source_requires_explicit_current_design_override(self):
        case = next(case for case in self.document["cases"] if case["id"] == "CR-10-VISUAL")
        case.pop("acceptance_override", None)
        self.assert_rejected("acceptance_override")
        case["acceptance_override"] = "docs/design/aligned-v1; use old height"
        self.assert_rejected("current docs/design/first-product-v1")

    def test_source_path_traversal_is_rejected(self):
        self.document["sources"][0]["path"] = "../private.csv"
        self.assert_rejected("path escapes")

    def test_source_symlink_is_rejected_even_for_same_content(self):
        relative = next(iter(contract.SOURCE_FILES))
        original = self.root / relative
        sibling = original.with_name("source-copy.csv")
        shutil.copyfile(original, sibling)
        original.unlink()
        original.symlink_to(sibling.name)
        self.assert_rejected("symbolic links")

    def test_malformed_schema_values_fail_without_crashing(self):
        original = deepcopy(self.document)
        for field in ("layer", "gate", "owner"):
            with self.subTest(field=field):
                self.document = deepcopy(original)
                self.new_case()[field] = []
                self.assertTrue(self.errors())
        self.document = deepcopy(original)
        self.document["variants"][0]["evidence_scope"] = []
        self.assertTrue(self.errors())

    def test_missing_source_is_not_equivalent_to_explicit_new_case(self):
        self.new_case().pop("source")
        self.assert_rejected("explicit source object or null")

    def test_corrupt_json_returns_error_instead_of_traceback(self):
        (self.root / contract.PLAN_PATH).write_text("{broken", encoding="utf-8")
        self.assertTrue(any("cannot read JSON" in error for error in contract.validate_plan(self.root)))

    def test_inherited_cr_steps_and_expected_cannot_be_rewritten_to_green_lights(self):
        original = deepcopy(self.document)
        for field in ("steps", "expected"):
            with self.subTest(field=field):
                self.document = deepcopy(original)
                case = next(case for case in self.document["cases"] if case["id"] == "CR-03-MO")
                case[field] = ["只要显示绿灯就通过，不核对正文"]
                self.assert_rejected(f"cannot rewrite inherited CR acceptance {field if field == 'steps' else 'expectations'}")

    def test_inherited_cr_precondition_cannot_drop_final_package_requirement(self):
        case = next(case for case in self.document["cases"] if case["id"] == "CR-03-MO")
        case["preconditions"][0] = "任意fixture即可，不需要实际游戏与正式包"
        self.assert_rejected("authoritative CR precondition")

    def test_inherited_model_and_library_expectations_remain_source_exact(self):
        original = deepcopy(self.document)
        for identifier, message in [("MC-01", "authoritative MC steps and expected"),
                                    ("GL-01", "authoritative GL acceptance")]:
            with self.subTest(identifier=identifier):
                self.document = deepcopy(original)
                case = next(case for case in self.document["cases"] if case["id"] == identifier)
                case["expected"] = ["任意结果均通过"]
                self.assert_rejected(message)
        self.document = deepcopy(original)
        next(case for case in self.document["cases"] if case["id"] == "MC-01")["steps"] = ["跳过实际操作"]
        self.assert_rejected("authoritative MC steps and expected")

    def test_real_game_variant_cannot_be_redefined_as_a_rust_fixture(self):
        variant = next(row for row in self.document["variants"] if row["id"] == "GAME-MO")
        variant["environment_ids"] = ["ENV-RUST"]
        variant["evidence_scope"] = "source_build"
        self.assert_rejected("cannot replace the reviewed variant contract")

    def test_environment_identity_cannot_be_kept_while_real_kind_becomes_controlled(self):
        environment = next(row for row in self.document["environments"] if row["id"] == "ENV-MO")
        environment["kind"] = "controlled"
        self.assert_rejected("cannot substitute controlled and real runtime environments")

    def test_all_layer_dependencies_cannot_be_deleted(self):
        for row in self.document["layers"]:
            row["depends_on"] = []
        self.assert_rejected("missing mandatory layer prerequisite")

    def test_additional_acyclic_layer_dependency_is_allowed(self):
        row = next(row for row in self.document["layers"] if row["id"] == "L2")
        row["depends_on"].append("L1")
        self.assertEqual([], self.errors())

    def test_release_policy_cannot_predeclare_passed_approved_or_implemented(self):
        original = deepcopy(self.document)
        for field, value in [("current_status", "passed"), ("design_approval", "approved"),
                             ("product_adapter_status", "implemented"),
                             ("evidence_validation", "all tests really ran")]:
            with self.subTest(field=field):
                self.document = deepcopy(original)
                self.document["release_policy"][field] = value
                self.assert_rejected("release_policy: must preserve")

    def test_metric_policies_cannot_be_deleted_or_claim_unapproved_thresholds(self):
        original = deepcopy(self.document)
        self.document["metric_policies"] = []
        self.assert_rejected("all three reviewed metric policies")
        for field, value in [("status", "approved"), ("rule", ""), ("p95_ms", 1)]:
            with self.subTest(field=field):
                self.document = deepcopy(original)
                self.document["metric_policies"][1][field] = value
                self.assert_rejected("no unapproved numeric thresholds")

    def test_scope_cannot_expand_to_windows_native_or_native_macos_extraction(self):
        original = deepcopy(self.document)
        self.document["scope"] = "Windows native及原生macOS游戏完整支持"
        self.assert_rejected("macos-arm64-crossover-only")
        self.document = deepcopy(original)
        self.document["supported_lanes"].append("windows-native")
        self.assert_rejected("macos-arm64-crossover-only")

    def test_final_package_layers_cannot_use_only_unit_evidence(self):
        case = next(case for case in self.document["cases"] if case["id"] == "MC-01")
        case["variant_ids"] = ["UNIT"]
        self.assert_rejected("L6-L8 require an actual final_package variant")

    def test_game_case_cannot_escape_final_package_check_by_changing_its_layer(self):
        case = next(case for case in self.document["cases"] if case["id"] == "CR-03-MO")
        case["layer"] = "L1"
        case["variant_ids"] = ["UNIT"]
        self.assert_rejected("real-game acceptance must remain L7")

    def test_visual_override_cannot_be_used_as_a_general_acceptance_escape(self):
        case = next(case for case in self.document["cases"] if case["id"] == "MC-01")
        case["acceptance_override"] = contract.VISUAL_OVERRIDE
        self.assert_rejected("only the reviewed CR-10-VISUAL override")
        visual = next(case for case in self.document["cases"] if case["id"] == "CR-10-VISUAL")
        visual["expected"] = ["只要当前稿能打开就算通过"]
        self.assert_rejected("cannot rewrite inherited CR acceptance expectations")

    def test_mechanism_cases_cannot_be_deleted_while_source_cases_still_exist(self):
        self.document["cases"] = [case for case in self.document["cases"] if case["source"] is not None]
        self.assert_rejected("all reviewed TP-01 through TP-31")

    def test_moving_domain_and_infrastructure_mechanisms_cannot_empty_layers(self):
        for case in self.document["cases"]:
            if case["layer"] in {"L1", "L3"}:
                case["layer"] = "L4"
        self.assert_rejected("mechanism ID/layer/AI traceability")
        self.assert_rejected("every layer L0 through L8")

    def test_ai_coverage_cannot_be_laundered_into_installation_case(self):
        for case in self.document["cases"]:
            case["ai_config_ids"] = []
        install = next(case for case in self.document["cases"] if case["id"] == "CR-01-INSTALL")
        install["ai_config_ids"] = sorted(contract.AI_IDS)
        self.assert_rejected("source-specific AI traceability")

    def test_model_case_ai_traceability_must_match_its_own_source(self):
        case = next(case for case in self.document["cases"] if case["id"] == "MC-01")
        case["ai_config_ids"].append("AI-CFG-16")
        self.assert_rejected("source-specific AI traceability")

    def test_mechanism_ai_references_cannot_be_reassigned_to_unrelated_feature(self):
        case = next(case for case in self.document["cases"] if case["id"] == "TP-01")
        case["ai_config_ids"] = ["AI-CFG-16"]
        self.assert_rejected("mechanism ID/layer/AI traceability")


if __name__ == "__main__":
    unittest.main()
