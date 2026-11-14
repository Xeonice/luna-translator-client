"""Independent v3 contract mutations, never production migration execution."""
import copy
from contextlib import closing
import json
from pathlib import Path
import re
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

import storage_schema_v3 as s


class SchemaThreeGuards(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="schema3-guard-")
        self.addCleanup(temporary.cleanup)
        self.folder = Path(temporary.name).resolve()
        self.counter = 0

    def candidate(self, sql=None, after=""):
        self.counter += 1
        path = self.folder / (str(self.counter) + ".sqlite3")
        with closing(sqlite3.connect(path)) as db:
            for relative in [*s.SOURCES["profile"], s.v2.DESIGN]:
                db.executescript(s.safe_bytes(s.ROOT, relative).decode())
            db.executescript(sql if sql is not None else s.safe_bytes(s.ROOT, s.DESIGN).decode())
            db.executescript(after)
        return path

    def mutated_column(self, table, field, mode):
        sql = s.safe_bytes(s.ROOT, s.DESIGN).decode()
        pattern = r"\b" + re.escape(field) + r"\s+(INTEGER|TEXT)( NOT NULL)?"
        def replace(match):
            return (field + " BLOB" + (match[2] or "") if mode == "type" else
                    field + " " + match[1] + ("" if match[2] else " NOT NULL"))
        if table == "config_revisions":
            changed, count = re.subn(pattern, replace, sql, count=1)
        else:
            name = table + "_new" if table in {"connections", "credential_refs"} else table
            match = re.search(r"CREATE TABLE " + name + r" \([\s\S]*?\) STRICT;", sql)
            self.assertIsNotNone(match)
            block, count = re.subn(pattern, replace, match[0], count=1)
            changed = sql[:match.start()] + block + sql[match.end():]
        self.assertEqual(count, 1)
        return changed

    def test_full_contract_has_484_fields_and_preserves_old_460_physical_fields(self):
        self.assertEqual(s.validate_schema_contract(s.ROOT), [])
        before, after = s.v2.design_contract(s.ROOT), s.design_contract(s.ROOT)
        self.assertEqual(sum(len(t["columns"]) for db in after["databases"].values() for t in db["tables"].values()), 484)
        self.assertEqual(len(s.field_rows(after)), 24)
        self.assertEqual(before["databases"]["dictionary"], after["databases"]["dictionary"])
        for name, table in before["databases"]["profile"]["tables"].items():
            self.assertEqual(table["columns"], after["databases"]["profile"]["tables"][name]["columns"][:len(table["columns"])])

    def test_exact_candidate_is_readonly_and_v2_cannot_accept_v3(self):
        path = self.candidate()
        before = path.read_bytes()
        self.assertEqual(s.compare_current_schema(s.ROOT, path), [])
        self.assertTrue(s.v2.compare_current_schema(s.ROOT, path, "profile"))
        self.assertEqual(path.read_bytes(), before)

    def test_each_of_24_column_types_and_nullability_is_checked(self):
        for field, (table, column) in s.field_rows(s.design_contract(s.ROOT)).items():
            for mode in ["type", "null"]:
                with self.subTest(field=field, mode=mode):
                    candidate = self.candidate(self.mutated_column(table, column["name"], mode))
                    self.assertTrue(s.compare_current_schema(s.ROOT, candidate))

    def test_relations_triggers_indexes_nul_check_and_extra_objects_are_exact(self):
        raw = s.safe_bytes(s.ROOT, s.DESIGN).decode()
        for sql in [raw.replace("AND instr(preset_id,char(0))=0", "", 1),
                    raw.replace("preset_revision,protocol,adapter_contract_revision", "preset_revision,protocol,compatibility_profile_revision", 1)]:
            self.assertTrue(s.compare_current_schema(s.ROOT, self.candidate(sql)))
        for after in ["DROP TRIGGER connections_codec_shape;", "DROP TRIGGER credential_refs_identity_immutable;", "DROP TRIGGER provider_presets_safe_options;", "DROP INDEX connections_by_credential_pool;", "CREATE TABLE unreviewed(id TEXT);", "ALTER TABLE connections ADD COLUMN unreviewed TEXT;"]:
            with self.subTest(after=after):
                self.assertTrue(s.compare_current_schema(s.ROOT, self.candidate(after=after)))

    def test_dictionary_rejects_dropped_extra_boolean_and_relationship_drift(self):
        original = s.read_json(s.ROOT, s.DICTIONARY)
        variants = []
        for action in ["missing", "duplicate", "bool_type", "extra", "relation", "producer_claim"]:
            value = copy.deepcopy(original)
            if action == "missing": value["fields"].pop()
            elif action == "duplicate": value["fields"][-1] = value["fields"][0]
            elif action == "bool_type": value["fields"][0]["primary_key_position"] = True
            elif action == "extra": value["fields"][0]["unreviewed"] = True
            elif action == "relation": value["relationships"]["connections"] = []
            else: value["fields"][0]["producer_status"] = "passed"
            variants.append(value)
        (self.folder / "dictionary.csv").write_bytes(s.safe_bytes(s.ROOT, s.CSV))
        for value in [*variants, [], {"fields": None}]:
            (self.folder / "dictionary.json").write_text(json.dumps(value))
            with patch.object(s, "DICTIONARY", "dictionary.json"), patch.object(s, "CSV", "dictionary.csv"):
                self.assertTrue(s.validate_field_dictionary(self.folder, s.design_contract(s.ROOT)))

    def test_source_hash_drift_and_candidate_sidecars_or_symlinks_fail_closed(self):
        with patch.object(s, "DESIGN_SHA256", "0" * 64):
            self.assertTrue(s.validate_schema_contract(s.ROOT))
        path = self.candidate()
        link = self.folder / "linked.sqlite3"
        link.symlink_to(path)
        self.assertTrue(s.compare_current_schema(s.ROOT, link))
        Path(str(path) + "-wal").write_bytes(b"uncheckpointed")
        self.assertTrue(s.compare_current_schema(s.ROOT, path))

    def test_csv_is_checked_and_candidate_fk_damage_is_not_usable_schema(self):
        (self.folder / "dictionary.json").write_bytes(s.safe_bytes(s.ROOT, s.DICTIONARY))
        original = s.safe_bytes(s.ROOT, s.CSV).decode()
        (self.folder / "dictionary.csv").write_text(original.replace("INTEGER", "BLOB", 1))
        with patch.object(s, "DICTIONARY", "dictionary.json"), patch.object(s, "CSV", "dictionary.csv"):
            self.assertTrue(s.validate_field_dictionary(self.folder, s.design_contract(s.ROOT)))
        # Construct an actually corrupt negative fixture. Ordinary candidates
        # retain FK enforcement; the read-only inspector must reject this one.
        path = self.candidate(after="PRAGMA foreign_keys=OFF; INSERT INTO config_revisions(revision_id,content_hash,created_at_ms,save_reason) VALUES('fk','" + "a" * 64 + "',1,'create'); INSERT INTO models(revision_id,model_id,connection_id,display_name) VALUES('fk','model','missing','broken');")
        before = path.read_bytes()
        self.assertTrue(any("foreign key" in error for error in s.compare_current_schema(s.ROOT, path)))
        self.assertEqual(path.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
