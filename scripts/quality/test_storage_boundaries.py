"""Guard the boundary verifier itself; these are not product acceptance tests."""
from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

from database_design import ROOT, build_metadata
from storage_boundaries import (CHECK_ERROR, FIXTURES, PROJECTION, REGISTRY,
                                Projection, _enums, _row, build_registry, canonical,
                                execute_registry, redundancy_certificate,
                                semantic_domain, sha, table_map, validate)


class StorageBoundaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry, errors = build_registry()
        if errors:
            raise AssertionError(errors)
        metadata, errors = build_metadata(ROOT)
        if errors:
            raise AssertionError(errors)
        cls.tables = table_map(metadata)
        cls.fixtures = json.loads((ROOT / FIXTURES).read_text())["tables"]

    def sandbox(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        files = [REGISTRY, FIXTURES,
                 "docs/architecture/database/profile.sql",
                 "docs/architecture/database/config.sql",
                 "docs/architecture/database/dictionary.sql",
                 "docs/product/first-product-scope.md",
                 "docs/architecture/engineering-modules.csv"]
        for relative in files:
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        return root

    def mutate_registry(self, mutation):
        root = self.sandbox()
        value = json.loads((root / REGISTRY).read_text())
        mutation(value)
        (root / REGISTRY).write_text(json.dumps(value, ensure_ascii=False))
        return validate(root)

    def test_exact_field_set_and_executable_obligations(self):
        expected = {f"{key}.{field['name']}" for key, table in self.tables.items() for field in table["fields"]}
        self.assertEqual(expected, {field["id"] for field in self.registry["fields"]})
        probes = {probe["id"]: probe for probe in self.registry["probes"]}
        self.assertEqual(len(probes), len(self.registry["probes"]))
        for field in self.registry["fields"]:
            kinds = {probes[key]["kind"] for key in field["probes"]}
            self.assertTrue({"field_positive_control", "field_type_reject", "nonnull_positive"} <= kinds, field["id"])
            self.assertTrue(kinds & {"nullable_accept", "required_null_reject", "integer_primary_key_null_allocates"}, field["id"])
            if field["default"] is not None:
                self.assertIn("declared_default", kinds)

    def test_all_registered_boundaries_really_execute_with_controls(self):
        receipt = execute_registry(self.registry)
        self.assertEqual(receipt["status"], "passed", receipt["errors"][:10])
        self.assertEqual({case["id"] for case in receipt["checks"]}, {probe["id"] for probe in self.registry["probes"]})
        self.assertTrue(all(case["assertions"] >= 3 and not case["skipped"] and case["status"] == "passed" for case in receipt["checks"]))
        self.assertEqual(receipt["projection"], PROJECTION)
        self.assertTrue(receipt["not_product_acceptance"])

    def test_each_check_has_isolation_or_sound_redundancy_proof(self):
        probes = {probe["id"]: probe for probe in self.registry["probes"]}
        for check in self.registry["check_constraints"]:
            cases = [probes[key] for key in check["probes"]]
            self.assertIn("check_positive_control", {case["kind"] for case in cases})
            if check["independence"] == "isolated":
                self.assertTrue(any(case["expected"].get("isolated_check") == check["expression"] for case in cases))
            else:
                self.assertEqual(check["independence"], "logically_redundant")
                self.assertIsNotNone(check["redundancy_certificate"])
                self.assertTrue(any(check["expression"] in case["expected"].get("joint_checks", []) for case in cases))
        self.assertEqual(self.registry["counts"]["blocked"], 0)
        self.assertEqual(self.registry["counts"]["isolated_checks"] + self.registry["counts"]["logically_redundant_checks"], len(self.registry["check_constraints"]))

    def test_projection_preserves_original_checks_unique_and_columns(self):
        for table in self.tables.values():
            projection = Projection(table)
            try:
                self.assertEqual(projection.db.execute("SELECT sql FROM sqlite_schema WHERE type='table'").fetchone()[0], table["create_sql"])
                self.assertEqual(projection.db.execute("PRAGMA foreign_keys").fetchone()[0], 0)
                self.assertEqual(projection.db.execute("PRAGMA ignore_check_constraints").fetchone()[0], 0)
                self.assertEqual(projection.db.execute("SELECT count(*) FROM sqlite_schema WHERE type='trigger'").fetchone()[0], 0)
                self.assertEqual(len(projection.db.execute(f'PRAGMA table_info("{table["name"]}")').fetchall()), len(table["fields"]))
            finally:
                projection.close()

    def test_all_enum_members_have_accepted_full_row_context(self):
        for key, table in self.tables.items():
            for field in table["fields"]:
                for member in _enums(field):
                    self.assertTrue(any(probe.get("field") == field["name"] and probe["database"] == table["database"] and probe["table"] == table["name"] and probe["mutations"].get(field["name"]) == member and probe["expected"]["outcome"] == "accept" for probe in self.registry["probes"]), f"{key}.{field['name']}={member!r}")

    def test_integer_primary_key_null_is_recorded_as_rowid_allocation(self):
        cases = [probe for probe in self.registry["probes"] if probe["kind"] == "integer_primary_key_null_allocates"]
        self.assertTrue(cases)
        self.assertTrue(all(case["expected"]["outcome"] == "accept" and case["expected"]["stored_type"] == "integer" for case in cases))

    def test_lossless_coercion_is_accepted_and_semantic_rejection_is_not_type_error(self):
        accepted = [probe for probe in self.registry["probes"] if probe["kind"] == "sqlite_lossless_coercion_accept"]
        rejected = [probe for probe in self.registry["probes"] if probe["kind"] == "coercion_rejected_by_check"]
        self.assertTrue(accepted)
        self.assertTrue(rejected)
        self.assertTrue(all(probe["expected"]["outcome"] == "accept" for probe in accepted))
        self.assertTrue(all(probe["expected"]["sqlite_errorcode"] == CHECK_ERROR for probe in rejected))

    def test_u64_oracle_rejects_overflow_noncanonical_and_nul(self):
        field = next(field for field in self.tables["profile.capture_sources"]["fields"] if field["name"] == "hook_address_u64")
        for value in ("0", "1", str(2**64-1)):
            self.assertTrue(semantic_domain(field, value)[0])
        for value in ("", "01", "+1", "-1", "1.0", " 1", "1 ", "１", str(2**64), "1\0evil"):
            self.assertFalse(semantic_domain(field, value)[0], repr(value))

    def test_hash_oracle_requires_entire_lowercase_64_hex_string(self):
        field = next(field for field in self.tables["profile.resource_snapshots"]["fields"] if field["name"] == "manifest_sha256")
        self.assertTrue(semantic_domain(field, "a"*64)[0])
        for value in ("a"*63, "a"*65, "A"*64, "g"*64, "a"*64+"\0evil", "\0"+"a"*64):
            self.assertFalse(semantic_domain(field, value)[0], repr(value))

    def test_json_oracle_checks_whole_document_top_level_and_nul(self):
        field = next(field for field in self.tables["profile.resource_snapshots"]["fields"] if field["name"] == "metadata_json")
        self.assertTrue(semantic_domain(field, '{"text":"図"}')[0])
        for value in ("[]", "null", "NaN", "{}tail", "{}\0evil", "{", "\0{}"):
            self.assertFalse(semantic_domain(field, value)[0], repr(value))

    def test_declared_numeric_limits_have_independent_oracle(self):
        field = next(field for field in self.tables["profile.sessions"]["fields"] if field["name"] == "windows_pid")
        for value in (1, 4294967295):
            self.assertTrue(semantic_domain(field, value)[0])
        for value in (0, -1, 4294967296):
            self.assertFalse(semantic_domain(field, value)[0])

    def test_physical_numeric_boundaries_are_distinguished_from_api_contracts(self):
        probes = self.registry["probes"]
        integers = [field for field in self.registry["fields"] if field["type"] == "INTEGER"]
        for field in integers:
            cases = [probe for probe in probes if f"{probe['database']}.{probe['table']}.{probe.get('field')}" == field["id"]]
            self.assertEqual(len([case for case in cases if case["kind"] == "physical_integer_binding_boundary"]), 2)
            self.assertTrue(any(case["kind"] == "numeric_type_reject" and case["mutations"][field["field"]] == 1.5 for case in cases))
        self.assertTrue(any(probe["kind"] == "sqlite_real_nonfinite_storage" and probe["expected"]["outcome"] == "accept" for probe in probes))
        self.assertTrue(any(probe["kind"] == "sqlite_nan_binds_null" and probe["expected"]["sqlite_errorcode"] == 1299 for probe in probes))

    def test_utf8_lengths_use_bytes_and_preserve_supplementary_characters(self):
        table = self.tables["profile.reading_unit_revisions"]
        projection = Projection(table)
        self.addCleanup(projection.close)
        baseline = self.fixtures["profile.reading_unit_revisions"]["valid_row"]
        for text in ("図", "𠮷", "🌸", "が", "\0", ""):
            stored, error = projection.insert(baseline | {"source_text": text, "source_byte_length": len(text.encode("utf-8"))})
            self.assertIsNone(error)
            self.assertEqual(stored["source_text"]["value"], text)
            if text and len(text) != len(text.encode("utf-8")):
                _, error = projection.insert(baseline | {"source_text": text, "source_byte_length": len(text)})
                self.assertEqual(error["sqlite_errorcode"], CHECK_ERROR)
                self.assertIn("source_byte_length=length(CAST(source_text AS BLOB))", error["error"])

    def test_escaped_json_nul_is_legal_but_literal_nul_is_rejected(self):
        field = next(field for field in self.tables["profile.resource_snapshots"]["fields"] if field["name"] == "metadata_json")
        self.assertTrue(semantic_domain(field, '{"text":"\\u0000"}')[0])
        self.assertFalse(semantic_domain(field, '{"text":"\0"}')[0])
        projection = Projection(self.tables["profile.resource_snapshots"])
        self.addCleanup(projection.close)
        baseline = self.fixtures["profile.resource_snapshots"]["valid_row"]
        self.assertIsNone(projection.insert(baseline | {"metadata_json": '{"text":"\\u0000"}'})[1])
        self.assertEqual(projection.insert(baseline | {"metadata_json": '{}\0tail'})[1]["sqlite_errorcode"], CHECK_ERROR)

    def test_missing_field_registration_fails(self):
        self.assertEqual(self.mutate_registry(lambda value: value["fields"].pop())["status"], "failed")

    def test_extra_field_registration_fails(self):
        self.assertEqual(self.mutate_registry(lambda value: value["fields"].append({"id": "profile.fake.field"}))["status"], "failed")

    def test_missing_executable_probe_fails(self):
        self.assertEqual(self.mutate_registry(lambda value: value["probes"].pop())["status"], "failed")

    def test_unknown_probe_rule_fails(self):
        self.assertEqual(self.mutate_registry(lambda value: value["probes"][0].update(kind="skip-all-boundaries"))["status"], "failed")

    def test_missing_check_proof_fails(self):
        self.assertEqual(self.mutate_registry(lambda value: value["check_constraints"].pop())["status"], "failed")

    def test_unexpected_approval_of_invalid_case_fails(self):
        self.assertEqual(self.mutate_registry(lambda value: next(probe for probe in value["probes"] if probe["expected"]["outcome"] == "reject")["expected"].update(outcome="accept"))["status"], "failed")

    def test_schema_change_without_registration_review_fails(self):
        root = self.sandbox()
        path = root / "docs/architecture/database/profile.sql"
        path.write_text(path.read_text().replace("CHECK(version>0)", "CHECK(version>=0)", 1))
        self.assertEqual(validate(root)["status"], "failed")

    def test_invalid_fixture_cannot_turn_rejections_into_success(self):
        root = self.sandbox()
        value = json.loads((root / FIXTURES).read_text())
        value["tables"]["profile.profile_meta"]["valid_row"]["singleton"] = 0
        (root / FIXTURES).write_text(json.dumps(value))
        registry, errors = build_registry(root)
        self.assertTrue(any("illegal positive fixture" in error for error in errors))

    def test_new_field_requires_explicit_fixture_and_registration(self):
        root = self.sandbox()
        path = root / "docs/architecture/database/profile.sql"
        path.write_text(path.read_text().replace("    app_build TEXT NOT NULL,", "    new_boundary TEXT, -- 新边界说明\n    app_build TEXT NOT NULL,", 1))
        registry, errors = build_registry(root)
        self.assertTrue(any("explicitly define every original column" in error for error in errors))

    def test_nul_constraint_removal_fails_independent_oracle_even_with_write(self):
        root = self.sandbox()
        path = root / "docs/architecture/database/profile.sql"
        path.write_text(path.read_text().replace("instr(hook_address_u64,char(0))=0 AND ", "", 1))
        registry, errors = build_registry(root)
        self.assertTrue(any("independent canonical_u64 oracle requires rejection" in error for error in errors), errors[:5])

    def test_unprovable_new_check_is_blocked_instead_of_counted_passed(self):
        root = self.sandbox()
        path = root / "docs/architecture/database/dictionary.sql"
        path.write_text(path.read_text().replace("    resource_version TEXT NOT NULL,", "    resource_version TEXT NOT NULL CHECK(1),", 1))
        registry, errors = build_registry(root)
        self.assertFalse(errors, errors)
        self.assertTrue(any(item["reason"].startswith("no isolated target") for item in registry["blocked"]))

    def test_check_command_does_not_rewrite_registry(self):
        root = self.sandbox()
        raw = (root / REGISTRY).read_bytes()
        receipt = validate(root)
        self.assertEqual(receipt["status"], "passed", receipt["errors"][:5])
        self.assertEqual((root / REGISTRY).read_bytes(), raw)

    def test_wrong_error_attribution_fails_even_when_insert_rejects(self):
        value = deepcopy(self.registry)
        probe = next(probe for probe in value["probes"] if probe["kind"] == "field_type_reject")
        probe["expected"]["error_contains"] = "unrelated_table.unrelated_column"
        value["probes"] = [probe]
        receipt = execute_registry(value)
        self.assertEqual(receipt["status"], "failed")
        self.assertTrue(any("wrong rejection attribution" in item for item in receipt["checks"][0]["log"]))

    def test_false_claim_of_isolation_fails(self):
        value = deepcopy(self.registry)
        probe = next(probe for probe in value["probes"] if probe["kind"] == "logically_redundant_check_reject")
        expression = probe["expected"]["error_contains"].removeprefix("CHECK constraint failed: ")
        probe["expected"].pop("joint_checks")
        probe["expected"]["isolated_check"] = expression
        value["probes"] = [probe]
        receipt = execute_registry(value)
        self.assertEqual(receipt["status"], "failed")
        self.assertTrue(any("predicate attribution mismatch" in item for item in receipt["checks"][0]["log"]))


if __name__ == "__main__":
    unittest.main()
