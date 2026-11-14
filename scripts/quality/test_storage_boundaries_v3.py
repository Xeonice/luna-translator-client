"""Delta boundary enforcement rejects mutations rather than learning success."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import storage_boundaries_v3 as b


class HttpBoundaryGuards(unittest.TestCase):
    def test_all_24_fields_have_valid_type_null_and_all_boundaries_execute(self):
        registry = b.build_registry()
        self.assertEqual(len(registry["field_ids"]), 24)
        ids = {p["id"] for p in registry["probes"]}
        for field in registry["field_ids"]:
            _, table, name = field.split(".")
            for suffix in ["valid", "type", "null"]:
                self.assertIn("V3-PROBE:" + table + "." + name + "." + suffix, ids)
        result = b.assess()
        self.assertEqual(result["errors"], [])
        self.assertEqual(len(result["checks"]), 213)
        self.assertTrue(result["not_product_acceptance"])

    def test_removed_probe_or_wrong_positive_expectation_cannot_be_approved(self):
        registered = b.schema.read_json(b.ROOT, b.REGISTRY)
        for mode in ["drop", "expect", "identity"]:
            altered = copy.deepcopy(registered)
            if mode == "drop": altered["probes"].pop()
            elif mode == "expect": altered["probes"][0]["expected"] = {"outcome": "reject"}
            else: altered["field_ids"].pop()
            original_read = b.schema.read_json
            with patch.object(b.schema, "read_json", side_effect=lambda root, path: altered if path == b.REGISTRY else original_read(root, path)):
                self.assertEqual(b.assess()["status"], "failed")

    def test_nul_or_unknown_version_cannot_learn_an_acceptance(self):
        with b.schema.design_database(b.ROOT) as db:
            sql = db.execute("SELECT sql FROM sqlite_schema WHERE name='provider_preset_versions'").fetchone()[0]
        control = b.fixture_template()["tables"]["provider_preset_versions"]
        expected = {"outcome": "reject", "sqlite_errorcode": 275, "error_contains": "CHECK constraint failed"}
        changes = {"preset_id": "user.fixture\0tail"}
        self.assertEqual(b._probe(sql, "provider_preset_versions", control, changes, expected), [])
        unsafe = sql.replace("AND instr(preset_id,char(0))=0", "")
        self.assertTrue(b._probe(unsafe, "provider_preset_versions", control, changes, expected))
        self.assertEqual(b._probe(sql, "provider_preset_versions", control, {"preset_revision": 4294967296}, expected), [])

    def test_fixture_tamper_does_not_redefine_the_oracle(self):
        original = b.schema.read_json
        value = b.fixture_template()
        value["tables"]["connections"]["adapter_contract_revision"] = 0
        with patch.object(b.schema, "read_json", side_effect=lambda root, path: value if path == b.FIXTURES else original(root, path)):
            self.assertEqual(b.assess()["status"], "failed")

    def test_each_registered_relation_and_trigger_has_its_own_result(self):
        registry = b.build_registry()
        rows = b.relationship_checks(b.ROOT, b.fixture_template()["tables"])
        self.assertEqual({r["id"] for r in rows}, {"V3-RELATION:" + i for i in registry["relationship_ids"]})
        self.assertTrue(all(r["status"] == "passed" for r in rows))


if __name__ == "__main__":
    unittest.main()
