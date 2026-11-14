"""No planning metadata or partial receipt may authorize HTTP v3 delivery."""
import copy
import unittest

import storage_schema_v3 as s


class HttpAcceptanceGuards(unittest.TestCase):
    def fixture(self):
        rows = [{"id": i, "status": "passed", "assertions": 1, "skipped": 0,
                 **({"field_assertions": {"valid": 1, "type": 1, "null": 1}} if i.startswith("V3-FIELD:") else {})}
                for i in sorted(s.v3_check_ids(s.ROOT))]
        return {"kind": "guard_only_not_runtime_evidence", "checks": rows}

    def test_all_old_obligations_and_new_matrix_are_frozen_separately(self):
        self.assertEqual(s.validate_acceptance_registry(s.ROOT), [])
        template = s.acceptance_template(s.ROOT)
        prior = s.read_json(s.ROOT, "docs/architecture/database/v2/storage-acceptance.json")
        self.assertEqual(template["original_checks"], prior["checks"])
        self.assertEqual(template["original_upgrade_matrix"], prior["supported_upgrade_matrix"])
        self.assertEqual(template["supported_upgrade_matrix"]["fresh_versions"], {"profile": 3, "dictionary": 1})
        self.assertEqual([r["version"] for r in template["supported_upgrade_matrix"]["migration_chain"]], [1, 2, 3])
        self.assertEqual({r["id"] for r in template["supported_upgrade_matrix"]["routes"]}, {r["id"] for r in prior["supported_upgrade_matrix"]["routes"]})
        self.assertEqual(len(template["expanded_old_enum_obligations"]), 3)
        self.assertTrue(all(r["original_probe_ids"] for r in template["expanded_old_enum_obligations"]))

    def test_plan_cannot_drop_a_field_route_old_check_or_claim_runtime_passed(self):
        template = s.acceptance_template(s.ROOT)
        for key in ["checks", "original_checks", "original_upgrade_matrix", "delta_check_ids", "required_routes", "prior_obligation_sources", "supported_upgrade_matrix", "expanded_old_enum_obligations"]:
            altered = copy.deepcopy(template)
            altered[key] = None
            self.assertTrue(s.validate_acceptance_registry(s.ROOT, altered))
        altered = copy.deepcopy(template)
        altered["product_tests_executed"] = True
        self.assertTrue(s.validate_acceptance_registry(s.ROOT, altered))

    def test_complete_semantic_fixture_is_not_runtime_provenance(self):
        fixture = self.fixture()
        self.assertEqual(len(s.v3_check_ids(s.ROOT)), 59)
        self.assertEqual(s.validate_delta_receipt(s.ROOT, fixture), [])
        # This validator deliberately does not return an approval or authenticate
        # the fixture; outer registered execution remains independently required.
        self.assertEqual(s.assess(s.ROOT)["product_acceptance"], False)
        self.assertTrue(s.validate_delta_receipt(s.ROOT, {"status": "passed", "checks": []}))

    def test_missing_or_duplicate_assertions_boolean_counts_and_skips_fail(self):
        for mode in ["missing", "duplicate", "zero", "bool_count", "bool_skip", "skipped", "failed"]:
            receipt = self.fixture()
            if mode == "missing": receipt["checks"].pop()
            elif mode == "duplicate": receipt["checks"].append(receipt["checks"][0])
            elif mode == "zero": receipt["checks"][0]["assertions"] = 0
            elif mode == "bool_count": receipt["checks"][0]["assertions"] = True
            elif mode == "bool_skip": receipt["checks"][0]["skipped"] = False
            elif mode == "skipped": receipt["checks"][0]["skipped"] = 1
            else: receipt["checks"][0]["status"] = "failed"
            self.assertTrue(s.validate_delta_receipt(s.ROOT, receipt), mode)

    def test_each_field_requires_its_own_three_actual_subassertions(self):
        receipt = self.fixture()
        row = next(r for r in receipt["checks"] if r["id"].startswith("V3-FIELD:"))
        for value in [None, {}, {"valid": 1, "type": 1}, {"valid": 1, "type": True, "null": 1}, {"valid": 1, "type": 0, "null": 1}]:
            altered = copy.deepcopy(receipt)
            next(r for r in altered["checks"] if r["id"] == row["id"])["field_assertions"] = value
            self.assertTrue(s.validate_delta_receipt(s.ROOT, altered))


if __name__ == "__main__":
    unittest.main()
