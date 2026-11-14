"""Independent schema-two mutation guards. These are guards, not product runs."""
import copy
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

import storage_schema_v2 as v2
from database_design import ROOT, SOURCES


class SchemaTwoGuards(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory(prefix="schema2-guard-")
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name).resolve()
        self.counter = 0

    def candidate(self, replacements=(), after=""):
        self.counter += 1
        path = self.folder / f"candidate-{self.counter}.sqlite3"
        sql = "\n".join((ROOT / name).read_text() for name in SOURCES["profile"])
        sql += "\n" + (ROOT / v2.DESIGN).read_text()
        for before, replacement in replacements:
            self.assertIn(before, sql)
            sql = sql.replace(before, replacement, 1)
        with closing(sqlite3.connect(path)) as db:
            db.executescript(sql)
            db.executescript(after)
        return path

    def test_reviewed_contract_and_all_29_readable_fields_agree(self):
        self.assertEqual(v2.validate_schema_contract(ROOT), [])
        self.assertEqual(v2.validate_field_dictionary(ROOT), [])
        old = v2.v1.design_contract(ROOT)
        new = v2.design_contract(ROOT)
        def fields(c):
            return sum(len(t["columns"]) for db in c["databases"].values() for t in db["tables"].values())
        self.assertEqual(fields(old), 431)
        self.assertEqual(fields(new), 460)
        self.assertEqual(old["databases"]["dictionary"], new["databases"]["dictionary"])
        self.assertEqual(hashlib.sha256((ROOT / v2.DESIGN).read_bytes()).hexdigest(), v2.DESIGN_SHA256)

    def test_exact_new_structure_is_readonly_and_old_contract_cannot_accept_it(self):
        path = self.candidate()
        before = path.read_bytes()
        self.assertEqual(v2.compare_current_schema(ROOT, path, "profile"), [])
        self.assertTrue(v2.v1.compare_schema(ROOT, path, "profile"))
        self.assertEqual(path.read_bytes(), before)

    def test_each_new_field_type_and_nullability_drift_is_rejected(self):
        mutations = [
            ("operation_id TEXT NOT NULL PRIMARY KEY", "operation_id BLOB NOT NULL PRIMARY KEY"),
            ("request_sha256 TEXT NOT NULL CHECK", "request_sha256 TEXT CHECK"),
            ("ended_at_ms INTEGER CHECK(ended_at_ms IS NULL OR ended_at_ms>=0)", "ended_at_ms REAL CHECK(ended_at_ms IS NULL OR ended_at_ms>=0)"),
            ("hud_background_opacity REAL NOT NULL DEFAULT 0.70", "hud_background_opacity REAL DEFAULT 0.70"),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.assertTrue(v2.compare_current_schema(ROOT, self.candidate([mutation]), "profile"))

    def test_hash_nul_u64_range_enum_and_opacity_checks_cannot_be_dropped(self):
        mutations = [
            ("instr(request_sha256,char(0))=0 AND ", ""),
            ("wire_sequence_u64<='18446744073709551615'", "wire_sequence_u64<='99999999999999999999'"),
            ("segmentation_enabled IN (0,1)", "segmentation_enabled IN (0,1,2)"),
            ("hud_background_opacity<=1.0", "hud_background_opacity<=2.0"),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.assertTrue(v2.compare_current_schema(ROOT, self.candidate([mutation]), "profile"))

    def test_composite_parent_order_and_delete_action_are_exact(self):
        for mutation in [
            ("FOREIGN KEY(source_id,session_id) REFERENCES capture_sources(source_id,session_id)", "FOREIGN KEY(session_id,source_id) REFERENCES capture_sources(session_id,source_id)"),
            ("FOREIGN KEY(preference_revision_id) REFERENCES preference_revisions(preference_revision_id) ON DELETE CASCADE", "FOREIGN KEY(preference_revision_id) REFERENCES preference_revisions(preference_revision_id) ON DELETE RESTRICT"),
        ]:
            with self.subTest(mutation=mutation):
                self.assertTrue(v2.compare_current_schema(ROOT, self.candidate([mutation]), "profile"))

    def test_every_new_trigger_and_index_is_mandatory_and_extra_objects_are_refused(self):
        contract = v2.design_contract(ROOT)["databases"]["profile"]["tables"]
        for table in v2.TABLES | {"reading_unit_revisions"}:
            for trigger in contract[table]["triggers"]:
                if table == "reading_unit_revisions" and trigger != "body_record_position":
                    continue
                with self.subTest(trigger=trigger):
                    self.assertTrue(v2.compare_current_schema(ROOT, self.candidate(after=f'DROP TRIGGER "{trigger}";'), "profile"))
        for after in ["DROP INDEX ix_hook_events_filter;", "ALTER TABLE capture_operations ADD COLUMN unauthorized TEXT;", "CREATE TABLE unauthorized(x TEXT);"]:
            self.assertTrue(v2.compare_current_schema(ROOT, self.candidate(after=after), "profile"))

    def test_dictionary_rejects_missing_duplicate_type_and_nullable_changes(self):
        data = json.loads((ROOT / v2.DICTIONARY).read_text())
        variants = []
        missing = copy.deepcopy(data); missing["fields"].pop(); variants.append(missing)
        duplicate = copy.deepcopy(data); duplicate["fields"][-1] = duplicate["fields"][0]; variants.append(duplicate)
        typed = copy.deepcopy(data); typed["fields"][0]["type"] = "REAL"; variants.append(typed)
        nullable = copy.deepcopy(data); nullable["fields"][0]["nullable"] = True; variants.append(nullable)
        boolean_pk = copy.deepcopy(data); boolean_pk["fields"][0]["primary_key_position"] = True; variants.append(boolean_pk)
        unknown = copy.deepcopy(data); unknown["fields"][0]["unreviewed"] = True; variants.append(unknown)
        bad_id = copy.deepcopy(data); bad_id["fields"][0]["id"] = []; variants.append(bad_id)
        malformed = copy.deepcopy(data); malformed["fields"] = {}; variants.append(malformed)
        variants.append([])
        for value in variants:
            file = self.folder / "dictionary.json"; file.write_text(json.dumps(value))
            with patch.object(v2, "DICTIONARY", file.name):
                self.assertTrue(v2.validate_field_dictionary(self.folder, v2.design_contract(ROOT)))


if __name__ == "__main__":
    unittest.main()
