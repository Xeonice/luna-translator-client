"""Schema-comparator mutation controls; temporary design clones, not product runs."""
from pathlib import Path
import hashlib
import json
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from database_design import ROOT, SOURCES, _authorizer
from storage_schema import CONTRACT, compare_rejected_relationship_snapshot, compare_schema, inspect_connection, validate_schema_contract, sql_tokens


BAD_RELATIONSHIP = "PRAGMA foreign_keys=OFF; INSERT INTO sessions(session_id,game_id,source_kind,state,started_at_ms) VALUES('test-session','absent-game','manual','active',1);"


class StorageSchemaTests(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory(prefix='storage-schema-guard-')
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name).resolve()
        self.counter = 0

    def candidate(self, database='profile', substitutions=(), after=''):
        self.counter += 1
        path = self.folder / f'guard#{self.counter} candidate?.sqlite3'
        db = sqlite3.connect(path)
        db.set_authorizer(_authorizer)
        try:
            combined = '\n'.join((ROOT / name).read_text() for name in SOURCES[database])
            for before, replacement in substitutions:
                self.assertIn(before, combined, 'mutation must change real source')
                combined = combined.replace(before, replacement, 1)
            db.executescript(combined)
            if after:
                db.executescript(after)
            db.commit()
        finally:
            db.close()
        return path

    def assert_drift(self, substitutions=(), after='', contains=None):
        path = self.candidate(substitutions=substitutions, after=after)
        errors = compare_schema(ROOT, path, 'profile')
        self.assertTrue(errors, 'schema mutation must be rejected')
        if contains:
            self.assertTrue(any(contains in error for error in errors), errors[:5])

    def test_frozen_contract_matches_current_reviewable_design(self):
        self.assertEqual([], validate_schema_contract(ROOT))

    def test_matching_candidates_in_both_databases_pass_readonly(self):
        for name in SOURCES:
            with self.subTest(database=name):
                path = self.candidate(name)
                before = hashlib.sha256(path.read_bytes()).hexdigest()
                self.assertEqual([], compare_schema(ROOT, path, name))
                self.assertEqual(before, hashlib.sha256(path.read_bytes()).hexdigest())
                connection = sqlite3.connect(f'{path.as_uri()}?mode=ro', uri=True)
                try:
                    with self.assertRaises(sqlite3.OperationalError):
                        connection.execute('CREATE TABLE forbidden(x)')
                finally:
                    connection.close()

    def test_closed_wal_mode_snapshot_inspection_does_not_create_sidecars(self):
        path = self.candidate()
        connection = sqlite3.connect(path)
        self.assertEqual(connection.execute('PRAGMA journal_mode=WAL').fetchone(), ('wal',))
        connection.close()
        self.assertFalse(Path(str(path) + '-wal').exists())
        self.assertFalse(Path(str(path) + '-shm').exists())
        before = path.read_bytes()
        self.assertEqual(compare_schema(ROOT, path, 'profile'), [])
        self.assertEqual(path.read_bytes(), before)
        self.assertFalse(Path(str(path) + '-wal').exists())
        self.assertFalse(Path(str(path) + '-shm').exists())

    def test_active_wal_snapshot_is_rejected_without_checkpoint_or_mutation(self):
        path = self.candidate()
        connection = sqlite3.connect(path)
        self.addCleanup(connection.close)
        connection.execute('PRAGMA journal_mode=WAL')
        connection.execute('PRAGMA wal_autocheckpoint=0')
        connection.execute("INSERT INTO games VALUES('live','Live original',NULL,NULL,NULL,1,1,1,NULL)")
        connection.commit()
        wal = Path(str(path) + '-wal')
        shm = Path(str(path) + '-shm')
        self.assertGreater(wal.stat().st_size, 0)
        self.assertGreater(shm.stat().st_size, 0)
        before = [entry.read_bytes() for entry in (path, wal, shm)]
        errors = compare_schema(ROOT, path, 'profile')
        self.assertTrue(any('sealed snapshot' in error for error in errors), errors)
        self.assertEqual([entry.read_bytes() for entry in (path, wal, shm)], before)
        self.assertEqual(connection.execute('SELECT display_name FROM games WHERE game_id=?', ('live',)).fetchone(), ('Live original',))

    def test_nonempty_shm_and_sidecar_symlinks_cannot_bypass_sealed_snapshot_check(self):
        path = self.candidate()
        shm = Path(str(path) + '-shm')
        shm.write_bytes(b'isolated nonempty shared-memory stand-in')
        self.assertTrue(any('sealed snapshot' in error for error in compare_schema(ROOT,path,'profile')))
        shm.unlink()
        target = self.folder / 'sidecar-target'
        target.write_bytes(b'')
        shm.symlink_to(target)
        self.assertTrue(any('sealed snapshot' in error for error in compare_schema(ROOT,path,'profile')))

    def test_whitespace_comments_and_unquoted_keyword_case_are_cosmetic(self):
        path = self.candidate(substitutions=[('CREATE TABLE games (', 'create  table games ( /* formatting only */')])
        self.assertEqual([], compare_schema(ROOT, path, 'profile'))
        self.assertNotEqual(sql_tokens("DEFAULT 'ACTIVE'"), sql_tokens("DEFAULT 'active'"))

    def test_column_type_drift(self):
        self.assert_drift([('display_name TEXT NOT NULL, -- 用户显示名，不作为目标身份', 'display_name BLOB NOT NULL, -- 用户显示名，不作为目标身份')], contains='columns')

    def test_nullable_drift(self):
        self.assert_drift([('display_name TEXT NOT NULL, -- 用户显示名，不作为目标身份', 'display_name TEXT, -- 用户显示名，不作为目标身份')], contains='notnull')

    def test_default_expression_drift(self):
        self.assert_drift([("proxy_mode TEXT NOT NULL DEFAULT 'inherit'", "proxy_mode TEXT NOT NULL DEFAULT 'disabled'")], contains='dflt_value')

    def test_extra_and_missing_columns(self):
        self.assert_drift(after='ALTER TABLE games ADD COLUMN unexpected TEXT;', contains='columns')
        self.assert_drift(after='ALTER TABLE games DROP COLUMN original_title;', contains='columns')

    def test_extra_and_missing_tables_and_views(self):
        self.assert_drift(after='CREATE TABLE unexpected(id TEXT PRIMARY KEY) STRICT;', contains='unexpected')
        self.assert_drift(after='DROP TABLE cache_entries;', contains='cache_entries')
        self.assert_drift(after='CREATE VIEW unexpected_view AS SELECT game_id FROM games;', contains='views')

    def test_strict_flag_drift(self):
        self.assert_drift([(") STRICT;\nCREATE INDEX ix_games_active", ");\nCREATE INDEX ix_games_active")], contains='strict')

    def test_without_rowid_drift(self):
        self.assert_drift([(") STRICT;\nCREATE INDEX ix_games_active", ") STRICT, WITHOUT ROWID;\nCREATE INDEX ix_games_active")], contains='without_rowid')

    def test_primary_key_order_drift(self):
        self.assert_drift([('PRIMARY KEY (revision_id, connection_id)', 'PRIMARY KEY (connection_id, revision_id)')], contains='columns')

    def test_foreign_key_deleted(self):
        self.assert_drift([('    FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE RESTRICT\n', '    CHECK(game_id IS NOT NULL)\n')], contains='foreign_keys')

    def test_foreign_key_composite_child_order_drift(self):
        self.assert_drift([('FOREIGN KEY(event_id,session_id) REFERENCES raw_events(event_id,session_id)', 'FOREIGN KEY(session_id,event_id) REFERENCES raw_events(event_id,session_id)')], contains='foreign_keys')

    def test_foreign_key_delete_action_drift(self):
        self.assert_drift([('FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE RESTRICT', 'FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE CASCADE')], contains='on_delete')

    def test_deferred_foreign_key_semantics_drift(self):
        self.assert_drift([('DEFERRABLE INITIALLY DEFERRED', 'NOT DEFERRABLE INITIALLY IMMEDIATE')], contains='sql_tokens')

    def test_check_clause_drift(self):
        self.assert_drift([('row_revision INTEGER NOT NULL CHECK(row_revision>0)', 'row_revision INTEGER NOT NULL CHECK(row_revision>=0)')], contains='sql_tokens')

    def test_unique_partial_index_drift(self):
        self.assert_drift([('CREATE UNIQUE INDEX uq_game_target_environment_path', 'CREATE INDEX uq_game_target_environment_path')], contains='unique')
        self.assert_drift([('WHERE archived_at_ms IS NULL AND target_path_key IS NOT NULL;', 'WHERE archived_at_ms IS NULL;')], contains='sql_tokens')

    def test_index_expression_collation_direction_and_missing_index(self):
        for sql in ('DROP INDEX ix_games_active;', 'DROP INDEX ix_games_active; CREATE INDEX ix_games_active ON games(lower(display_name));', 'DROP INDEX ix_games_active; CREATE INDEX ix_games_active ON games(archived_at_ms, updated_at_ms ASC, game_id COLLATE NOCASE);'):
            with self.subTest(sql=sql):
                self.assert_drift(after=sql, contains='indexes')

    def test_trigger_deleted_or_body_changed(self):
        self.assert_drift(after='DROP TRIGGER attempt_initial_state;', contains='triggers')
        self.assert_drift([("RAISE(ABORT,'attempt_must_start')", "RAISE(ABORT,'changed_trigger')")], contains='triggers')

    def test_invalid_candidate_missing_identity_and_corrupt_database(self):
        self.assertTrue(compare_schema(ROOT, self.folder/'missing.sqlite3', 'profile'))
        bad = self.folder/'corrupt.sqlite3'; bad.write_bytes(b'this is not sqlite')
        self.assertTrue(compare_schema(ROOT, bad, 'profile'))
        self.assertTrue(compare_schema(ROOT, self.candidate(), 'unknown'))

    def test_existing_foreign_key_violations_are_rejected(self):
        path = self.candidate(after=BAD_RELATIONSHIP)
        errors = compare_schema(ROOT, path, 'profile')
        self.assertTrue(any('violations' in error for error in errors), errors)

    def test_relationship_rejection_fixture_requires_real_fk_without_mutating_it(self):
        path = self.candidate(after=BAD_RELATIONSHIP)
        before = path.read_bytes()
        self.assertTrue(any('foreign key violations' in error for error in compare_schema(ROOT, path, 'profile')))
        self.assertEqual([], compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT))
        self.assertEqual(before, path.read_bytes())
        self.assertFalse(Path(str(path) + '-wal').exists())
        self.assertFalse(Path(str(path) + '-shm').exists())
        # Accepted negative evidence is still refused as an ordinary candidate.
        self.assertTrue(any('foreign key violations' in error for error in compare_schema(ROOT, path, 'profile')))

    def test_relationship_rejection_fixture_cannot_be_healthy_or_empty(self):
        for after in ('', "INSERT INTO games VALUES('valid','Valid game',NULL,NULL,NULL,1,1,1,NULL);"):
            with self.subTest(after=after):
                path = self.candidate(after=after)
                self.assertEqual([], compare_schema(ROOT, path, 'profile'))
                errors = compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT)
                self.assertTrue(any('requires at least one real foreign key violation' in error for error in errors), errors)

    def test_relationship_rejection_fixture_cannot_substitute_a_different_schema(self):
        for substitutions, after, expected in (
            ([('display_name TEXT NOT NULL, -- 用户显示名，不作为目标身份', 'display_name BLOB NOT NULL, -- 用户显示名，不作为目标身份')], '', 'columns'),
            ([('row_revision INTEGER NOT NULL CHECK(row_revision>0)', 'row_revision INTEGER NOT NULL CHECK(row_revision>=0)')], '', 'sql_tokens'),
            ([], 'ALTER TABLE games ADD COLUMN unexpected TEXT;', 'columns'),
            ([], 'DROP TRIGGER attempt_initial_state;', 'triggers'),
            ([], 'DROP INDEX ix_games_active;', 'indexes'),
        ):
            with self.subTest(expected=expected, substitutions=substitutions, after=after):
                path = self.candidate(substitutions=substitutions, after=BAD_RELATIONSHIP + after)
                errors = compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT)
                self.assertTrue(any(expected in error for error in errors), errors[:5])

    def test_relationship_rejection_fixture_still_requires_database_integrity(self):
        path = self.candidate(after=BAD_RELATIONSHIP + "INSERT INTO games VALUES('valid','Valid game',NULL,NULL,NULL,1,1,1,NULL);")
        connection = sqlite3.connect(path)
        try:
            page_size = connection.execute('PRAGMA page_size').fetchone()[0]
            index_page = connection.execute("SELECT rootpage FROM sqlite_schema WHERE name='ix_games_active'").fetchone()[0]
        finally:
            connection.close()
        # Delete the physical leaf's cell-directory entry without updating its
        # indexed row. This is genuine index corruption, not a schema mutation.
        raw = bytearray(path.read_bytes())
        offset = (index_page - 1) * page_size
        self.assertEqual(raw[offset], 0x0a, 'isolated one-row index must be a leaf')
        self.assertEqual(raw[offset + 3:offset + 5], b'\x00\x01')
        raw[offset + 3:offset + 5] = b'\x00\x00'
        path.write_bytes(raw)
        before = path.read_bytes()
        errors = compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT)
        self.assertTrue(any('integrity_check failed' in error for error in errors), errors)
        self.assertEqual(before, path.read_bytes())

    def test_relationship_rejection_fixture_is_inspected_immutable_and_query_only(self):
        path = self.candidate(after=BAD_RELATIONSHIP)
        before = path.read_bytes()

        def inspect_readonly(connection):
            self.assertEqual(connection.execute('PRAGMA query_only').fetchone(), (1,))
            self.assertEqual(connection.execute('PRAGMA foreign_keys').fetchone(), (1,))
            with self.assertRaises(sqlite3.OperationalError):
                connection.execute("INSERT INTO games VALUES('forbidden','Write blocked',NULL,NULL,NULL,1,1,1,NULL)")
            return inspect_connection(connection)

        with patch('storage_schema.sqlite3.connect', wraps=sqlite3.connect) as connect, patch('storage_schema.inspect_connection', side_effect=inspect_readonly):
            self.assertEqual([], compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT))
            connect.assert_called_once_with(path.resolve().as_uri() + '?mode=ro&immutable=1', uri=True)
        self.assertEqual(before, path.read_bytes())

    def test_relationship_rejection_fixture_rejects_malformed_ddl_and_corrupt_file(self):
        path = self.candidate(after=BAD_RELATIONSHIP + "PRAGMA writable_schema=ON; UPDATE sqlite_schema SET sql='CREATE TABLE games (' WHERE type='table' AND name='games'; PRAGMA writable_schema=OFF;")
        before = path.read_bytes()
        self.assertTrue(compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT))
        self.assertEqual(before, path.read_bytes())
        corrupt = self.folder / 'rejection-corrupt.sqlite3'
        corrupt.write_bytes(b'not a sqlite snapshot')
        self.assertTrue(compare_rejected_relationship_snapshot(ROOT, corrupt, 'profile', CONTRACT))

    def test_relationship_rejection_fixture_rejects_live_wal_without_checkpoint(self):
        path = self.candidate(after=BAD_RELATIONSHIP)
        connection = sqlite3.connect(path)
        self.addCleanup(connection.close)
        connection.execute('PRAGMA journal_mode=WAL')
        connection.execute('PRAGMA wal_autocheckpoint=0')
        connection.execute("INSERT INTO games VALUES('live','Live original',NULL,NULL,NULL,1,1,1,NULL)")
        connection.commit()
        wal, shm = (Path(str(path) + suffix) for suffix in ('-wal', '-shm'))
        before = [entry.read_bytes() for entry in (path, wal, shm)]
        errors = compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT)
        self.assertTrue(any('sealed snapshot' in error for error in errors), errors)
        self.assertEqual([entry.read_bytes() for entry in (path, wal, shm)], before)

    def test_relationship_rejection_fixture_rejects_missing_and_symlink_snapshots(self):
        self.assertTrue(compare_rejected_relationship_snapshot(ROOT, self.folder / 'absent.sqlite3', 'profile', CONTRACT))
        path = self.candidate(after=BAD_RELATIONSHIP)
        alias = self.folder / 'alias.sqlite3'
        alias.symlink_to(path)
        self.assertTrue(compare_rejected_relationship_snapshot(ROOT, alias, 'profile', CONTRACT))
        shm = Path(str(path) + '-shm')
        # A dangling sidecar is forbidden even though exists() reports false.
        shm.symlink_to(self.folder / 'absent-sidecar')
        self.assertTrue(any('sealed snapshot' in error for error in compare_rejected_relationship_snapshot(ROOT, path, 'profile', CONTRACT)))

    def test_relationship_rejection_fixture_only_accepts_profile_identity(self):
        path = self.candidate(after=BAD_RELATIONSHIP)
        for database in ('dictionary', 'unknown'):
            with self.subTest(database=database):
                errors = compare_rejected_relationship_snapshot(ROOT, path, database, CONTRACT)
                self.assertTrue(any('requires profile database identity' in error for error in errors), errors)
        # Relabeling the dictionary artifact as a profile cannot bypass schema comparison.
        self.assertTrue(compare_rejected_relationship_snapshot(ROOT, self.candidate('dictionary'), 'profile', CONTRACT))

    def test_contract_cannot_be_replaced_by_fresh_automatic_design_export(self):
        root = self.folder/'small-root'; (root/CONTRACT).parent.mkdir(parents=True)
        for names in SOURCES.values():
            for name in names:
                target = root/name; target.parent.mkdir(parents=True,exist_ok=True)
                target.write_bytes((ROOT/name).read_bytes())
        (root/CONTRACT).write_bytes((ROOT/CONTRACT).read_bytes())
        changed = root/SOURCES['profile'][0]
        changed.write_text(changed.read_text().replace('row_revision>0','row_revision>=0',1))
        self.assertTrue(validate_schema_contract(root))
        # A checker invocation must not update the frozen expected identity.
        self.assertEqual((ROOT/CONTRACT).read_bytes(),(root/CONTRACT).read_bytes())


if __name__ == '__main__':
    unittest.main()
