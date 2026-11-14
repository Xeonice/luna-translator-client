"""Non-product schema5 guards. Fixture SQLite never proves a Rust migration."""
import copy
from contextlib import closing
import json
from pathlib import Path
import re
import shutil
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

import storage_schema_v5 as s


class SchemaFiveGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='schema5-guard-')
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name).resolve()
        self.counter = 0

    def candidate(self):
        self.counter += 1
        path = self.folder / (str(self.counter) + '.sqlite3')
        with s.design_database() as source:
            target = sqlite3.connect(path)
            source.backup(target)
            target.close()
        return path

    def mutant(self, original):
        self.counter += 1
        path = self.folder / ('mutant-' + str(self.counter) + '.sqlite3')
        shutil.copyfile(original, path)
        return path

    def test_full_independent_contract_preserves_all_old_objects(self):
        self.assertEqual(s.validate_contract(), [])
        contract = s.design_contract()
        before = s.v4.design_contract(s.ROOT)
        self.assertEqual(len(contract['databases']['profile']['tables']), 83)
        self.assertEqual(sum(len(t['columns']) for t in contract['databases']['profile']['tables'].values()), 748)
        self.assertEqual(contract['databases']['dictionary'], before['databases']['dictionary'])
        s._preservation(before['databases']['profile'], contract['databases']['profile'])
        for table, shape in before['databases']['profile']['tables'].items():
            self.assertEqual(contract['databases']['profile']['tables'][table], shape)
        self.assertEqual(len(s.PRESERVED), 39)
        self.assertEqual(len(s.FIELD_IDS), 14)

    def test_design_expected_never_reads_production_ddl(self):
        visited = []
        original = s.safe_bytes
        old_reader = s.v4.safe_bytes
        def own(root, path):
            visited.append(path)
            self.assertFalse(str(path).startswith('crates/'))
            return original(root, path)
        def prior(root, path):
            visited.append(path)
            self.assertFalse(str(path).startswith('crates/'))
            return old_reader(root, path)
        with patch.object(s, 'safe_bytes', side_effect=own), patch.object(s.v4, 'safe_bytes', side_effect=prior):
            s.design_contract()
        self.assertIn(s.DESIGN, visited)
        self.assertIn(s.v4.DESIGN, visited)

    def test_all39_original_pins_and_both_target_pins_reject_drift(self):
        original = s.safe_bytes
        for path in s.PRESERVED | s.TARGET_PINS:
            with self.subTest(path=path):
                def changed(root, relative):
                    data = original(root, relative)
                    return data + b'\n-- drift\n' if relative == path else data
                with patch.object(s, 'safe_bytes', side_effect=changed):
                    self.assertTrue(s.validate_contract())

    def test_candidate_is_exact_readonly_and_old4_stays_strict(self):
        path = self.candidate()
        before = path.read_bytes()
        self.assertEqual(s.compare_current_schema(s.ROOT, path), [])
        self.assertTrue(s.v4.compare_current_schema(s.ROOT, path))
        self.assertEqual(path.read_bytes(), before)
        with closing(sqlite3.connect(path)) as db, db:
            db.execute('CREATE TABLE forbidden_extension(value TEXT)')
        self.assertTrue(s.compare_current_schema(s.ROOT, path))

    def test_all14_type_and_null_shape_mutants_are_rejected(self):
        original = self.candidate()
        for table, columns in s.NEW_COLUMNS.items():
            for column in columns:
                for mode in ('type', 'null'):
                    with self.subTest(table=table, column=column, mode=mode):
                        path = self.mutant(original)
                        with closing(sqlite3.connect(path)) as db, db:
                            ddl = db.execute("SELECT sql FROM sqlite_schema WHERE type='table' AND name=?", (table,)).fetchone()[0]
                            pattern = r'(?<![A-Za-z0-9_])' + re.escape(column) + r'\s+(TEXT|INTEGER)(\s+NOT\s+NULL)?'
                            def replace(match):
                                return column + ' ' + ('BLOB' if mode == 'type' else match[1]) + ((match[2] or '') if mode == 'type' else ('' if match[2] else ' NOT NULL'))
                            changed, count = re.subn(pattern, replace, ddl, count=1, flags=re.I)
                            self.assertEqual(count, 1)
                            db.execute('PRAGMA writable_schema=ON')
                            db.execute("UPDATE sqlite_schema SET sql=? WHERE type='table' AND name=?", (changed, table))
                            db.execute('PRAGMA schema_version=999')
                        self.assertTrue(s.v1.compare_schema_file(s.ROOT, path, 'profile', s.CONTRACT))

    def test_every_trigger_view_and_fk_definition_is_required(self):
        original = self.candidate()
        for name in s.TRIGGERS + (s.VIEW,):
            with self.subTest(name=name):
                path = self.mutant(original)
                with closing(sqlite3.connect(path)) as db, db:
                    db.execute(('DROP VIEW ' if name == s.VIEW else 'DROP TRIGGER ') + s.v1._name(name))
                self.assertTrue(s.v1.compare_schema_file(s.ROOT, path, 'profile', s.CONTRACT))
        contract = s.design_contract()['databases']['profile']
        before = s.v4.design_contract(s.ROOT)['databases']['profile']
        for table in s.NEW_COLUMNS:
            for index in range(len(contract['tables'][table]['foreign_keys'])):
                changed = copy.deepcopy(contract)
                changed['tables'][table]['foreign_keys'][index]['on_delete'] = 'CASCADE'
                self.assertTrue(s.v1._diff(contract, changed))
        changed = copy.deepcopy(contract)
        changed['tables']['games']['columns'][0]['type'] = 'BLOB'
        with self.assertRaises(ValueError):
            s._preservation(before, changed)

    def test_sql_boundaries_are_actual_guards_and_never_product_pass(self):
        result = s.assess_boundaries()
        self.assertEqual(result['status'], 'passed', result['errors'])
        self.assertFalse(result['product_migration_executed'])
        self.assertFalse(result['product_acceptance'])
        facts = result['facts']
        self.assertEqual([r['id'] for r in facts['fields']], list(s.FIELD_IDS))
        for row in facts['fields']:
            self.assertEqual(row['outcomes']['valid'], 'accept')
            self.assertEqual(row['outcomes']['type'], 'reject')
            self.assertEqual(row['outcomes']['null'], 'accept' if row['id'].endswith('.operation_id') else 'reject')
            if row['id'].endswith(('.member_count', '.member_ordinal')):
                self.assertEqual(row['outcomes']['maximum'], 'accept')
                self.assertEqual(row['outcomes']['above_maximum'], 'reject')
        ids = {r['id'] for r in facts['relationships_and_triggers']}
        self.assertTrue({'complete-64-members', 'reject-65-members', 'reject-empty-set', 'duplicate-member-source',
                         'duplicate-member-ordinal', 'duplicate-member-lineage', 'cross-session-member',
                         'source_selection_set_seal:gapped', s.VIEW}.issubset(ids))
        self.assertEqual(len([name for name in ids if name.startswith('fk:')]), 5)
        self.assertTrue(facts['legacy_backfill']['original_selection_bytes_preserved'])

    def test_pending_wal_and_symbolic_candidate_cannot_be_inspected(self):
        path = self.candidate()
        sidecar = Path(str(path) + '-wal')
        sidecar.write_bytes(b'pending actual data')
        self.assertTrue(s.compare_current_schema(s.ROOT, path))
        sidecar.unlink()
        link = self.folder / 'link.sqlite3'
        link.symlink_to(path)
        self.assertTrue(s.compare_current_schema(s.ROOT, link))

    def test_runtime_absence_is_blocked_but_present_marker_is_failed(self):
        self.assertEqual(s.inspect_runtime()['status'], 'blocked')
        path = self.folder / s.RUNTIME_FILENAME
        path.write_text('{"status":"passed","assertions":999,"exit_code":0}')
        ref = {'path': path.name, 'sha256': s.digest(path.read_bytes())}
        result = s.inspect_runtime(self.folder, ref, context={})
        self.assertEqual(result['status'], 'failed')
        self.assertFalse(result['product_migration_executed'])
        self.assertTrue(result['errors'])

    def test_json_duplicate_key_nan_and_path_escape_are_rejected(self):
        for text in ('{"status":"failed","status":"passed"}', '{"count":NaN}', '{"count":Infinity}'):
            with self.assertRaises(ValueError):
                s._loads(text)
        for path in ('../escape', '/absolute'):
            with self.assertRaises(ValueError):
                s.safe_bytes(self.folder, path)

    def test_nonempty_old_rows_project_original_columns_after_addition(self):
        before, after = sqlite3.connect(':memory:'), sqlite3.connect(':memory:')
        self.addCleanup(before.close)
        self.addCleanup(after.close)
        for db in (before, after):
            db.executescript("CREATE TABLE example(id TEXT PRIMARY KEY,value TEXT); INSERT INTO example VALUES('old','原字节'); CREATE TABLE schema_migrations(version INTEGER,hash TEXT); INSERT INTO schema_migrations VALUES(1,'kept');")
        after.execute('ALTER TABLE example ADD COLUMN newly_reviewed TEXT')
        after.execute("UPDATE example SET newly_reviewed='new approved extension'")
        s._old_rows_preserved(before, after)
        after.execute("UPDATE example SET value='changed'")
        with self.assertRaises(ValueError):
            s._old_rows_preserved(before, after)

    def test_recovery_is_exact_and_completed_rows_cannot_change(self):
        before, after = sqlite3.connect(':memory:'), sqlite3.connect(':memory:')
        self.addCleanup(before.close)
        self.addCleanup(after.close)
        ddl = """CREATE TABLE schema_migrations(version INTEGER,hash TEXT);
        CREATE TABLE profile_meta(singleton INTEGER,runtime_generation INTEGER,server_instance_id TEXT,runtime_state TEXT,updated_at_ms INTEGER,profile_id TEXT);
        INSERT INTO profile_meta VALUES(1,4,'old','active',10,'fixed-profile');
        CREATE TABLE request_tasks(task_id TEXT,state TEXT,invalidation_code TEXT,finished_at_ms INTEGER);
        INSERT INTO request_tasks VALUES('done','completed',NULL,9),('waiting','queued',NULL,NULL);
        CREATE TABLE sessions(session_id TEXT,state TEXT,ended_at_ms INTEGER);
        INSERT INTO sessions VALUES('stopped','stopped',8),('active','active',NULL);"""
        before.executescript(ddl)
        after.executescript(ddl)
        after.executescript("UPDATE profile_meta SET runtime_generation=5,server_instance_id='new',updated_at_ms=20; UPDATE request_tasks SET state='invalidated',invalidation_code='application_restarted',finished_at_ms=20 WHERE state='queued'; UPDATE sessions SET state='crashed',ended_at_ms=20 WHERE state='active';")
        s._old_rows_preserved(before, after, now_ms=20, server_id='new')
        after.execute("UPDATE request_tasks SET invalidation_code='erased' WHERE task_id='done'")
        with self.assertRaises(ValueError):
            s._old_rows_preserved(before, after, now_ms=20, server_id='new')

    def test_old_ledger_rewrite_is_not_accepted_as_migration(self):
        before, after = sqlite3.connect(':memory:'), sqlite3.connect(':memory:')
        self.addCleanup(before.close)
        self.addCleanup(after.close)
        before.executescript("CREATE TABLE schema_migrations(version INTEGER,hash TEXT);INSERT INTO schema_migrations VALUES(1,'original');")
        after.executescript("CREATE TABLE schema_migrations(version INTEGER,hash TEXT);INSERT INTO schema_migrations VALUES(1,'rewritten');")
        with self.assertRaises(ValueError):
            s._old_rows_preserved(before, after)

    def test_route_markers_not_run_and_missing_kill_cannot_qualify_faults(self):
        context = dict(nonce='a' * 32, source_tree_sha256='b' * 64, artifact_sha256='c' * 64, started_at_ms=0, finished_at_ms=100)
        routes = [{'id': 'readonly', 'error_code': 'not_run'}]
        def event(sequence, kind, facts):
            return {'sequence': sequence, 'nonce': context['nonce'], 'source_tree_sha256': context['source_tree_sha256'],
                    'artifact_sha256': context['artifact_sha256'], 'route': 'readonly', 'event': kind, 'at_ms': sequence, 'facts': facts}
        events = [event(1, 'writer_open_started', {'repository': 'SqliteStore::open', 'now_ms': 1, 'server_id': 'guard'}),
                  event(2, 'writer_open_failed', {'error_code': 'not_run'})]
        path = self.folder / 'events.jsonl'
        path.write_text(''.join(json.dumps(e) + '\n' for e in events))
        ref = {'path': path.name, 'sha256': s.digest(path.read_bytes())}
        with self.assertRaises(ValueError):
            s._route_events(self.folder, ref, events, context, routes)
        routes = [{'id': 'kill', 'error_code': 'process_killed'}]
        for event_row in events:
            event_row['route'] = 'kill'
        path.write_text(''.join(json.dumps(e) + '\n' for e in events))
        ref['sha256'] = s.digest(path.read_bytes())
        with self.assertRaises(ValueError):
            s._route_events(self.folder, ref, events, context, routes)

    def test_summary_requires_actual_unskipped_named_execution(self):
        for stdout in ('exit_code=0 status=passed', 'test result: ok. 0 passed; 0 failed; 1 ignored;',
                       'test other ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;'):
            with self.assertRaises(ValueError):
                s._actual_test_summary(stdout, ['bj_17_storage'])
        self.assertEqual(s._actual_test_summary('test bj_17_storage ...\nactual emitted data\nok\ntest result: ok. 1 passed; 0 failed; 0 ignored;', ['bj_17_storage']), 1)

    def test_event_frames_allow_only_exact_standalone_or_registered_harness_prefix(self):
        payload = '{"sequence":1,"event":"guard-only"}'
        marker = 'LUNA_STORAGE_V5_EVENT '
        expected = [{'sequence': 1, 'event': 'guard-only'}]
        self.assertEqual(s._execution_events(marker + payload, 'bj_08_storage'), expected)
        self.assertEqual(s._execution_events('test bj_08_storage ... ' + marker + payload, 'bj_08_storage'), expected)
        self.assertEqual(s._execution_events('ordinary log without an event', 'bj_08_storage'), [])
        for prefix in ('debug message: ', 'test bj_09_storage ... ', 'test bj_08_storage ... extra ', ' '):
            with self.subTest(prefix=prefix), self.assertRaises(ValueError):
                s._execution_events(prefix + marker + payload, 'bj_08_storage')

    def test_complete_thirteen_route_guard_fixture_reaches_all_event_branches(self):
        # This is explicitly only an inspector fixture: no production receipt,
        # writer execution or migration PASS is issued by _route_events.
        context = dict(nonce='a' * 32, source_tree_sha256='b' * 64, artifact_sha256='c' * 64,
                       started_at_ms=0, finished_at_ms=1000)
        events, routes = [], []
        def add(route, name, facts):
            number = len(events) + 1
            events.append(dict(sequence=number, nonce=context['nonce'], source_tree_sha256=context['source_tree_sha256'],
                               artifact_sha256=context['artifact_sha256'], route=route, event=name, at_ms=number, facts=facts))
        faults = {'rollback': ('storage_failure', 'transaction_check', 19), 'readonly': ('storage_failure', 'readonly', 8),
                  'busy': ('storage_busy', 'exclusive_lock', 5), 'enospc': ('storage_failure', 'filesystem_full', 13)}
        for name in ('fresh', 'from1', 'from2', 'from3', 'from4', 'reopen5', 'old4-rejects5', 'rollback', 'readonly', 'busy', 'enospc', 'kill', 'restore'):
            route = {'id': name, 'error_code': None, 'backup': 'guard-backup'}
            routes.append(route)
            add(name, 'writer_open_started', {'repository': 'SqliteStore::open', 'now_ms': 10, 'server_id': 'guard-only'})
            if name in faults:
                code, fault, sqlite_code = faults[name]
                route['error_code'] = code
                add(name, 'fault_armed', dict(fault=fault, read_only=True, holder_pid=123, lock='BEGIN IMMEDIATE', available_bytes=0, volume='guard-only'))
                if name == 'rollback':
                    for event in ('migration_transaction_started', 'migration_ddl_applied', 'transaction_rolled_back'):
                        add(name, event, {})
                add(name, 'writer_open_failed', {'error_code': code, 'sqlite_code': sqlite_code})
            elif name == 'kill':
                route['error_code'] = 'process_killed'
                for event in ('child_started', 'child_boundary_observed', 'kill_sent', 'child_waited'):
                    add(name, event, dict(pid=123, signal=9, exit_signal=9, boundary='migration_transaction'))
            elif name == 'old4-rejects5':
                route['error_code'] = 'unsupported_schema'
                add(name, 'old_writer_rejected', {})
            else:
                if name == 'restore':
                    add(name, 'backup_restored', {'backup_snapshot': route['backup']})
                add(name, 'writer_open_committed', {'schema_version': 5})
        path = self.folder / 'all-events.jsonl'
        path.write_text(''.join(json.dumps(e) + '\n' for e in events))
        ref = {'path': path.name, 'sha256': s.digest(path.read_bytes())}
        self.assertEqual(set(s._route_events(self.folder, ref, events, context, routes)), {r['id'] for r in routes})
        for name in faults:
            changed = copy.deepcopy(routes)
            next(r for r in changed if r['id'] == name)['error_code'] = 'not_run'
            with self.subTest(fault=name), self.assertRaises(ValueError):
                s._route_events(self.folder, ref, events, context, changed)
        with self.assertRaises(ValueError):
            s._route_events(self.folder, ref, events[:-1], context, routes)


class SchemaFiveChildProofGuards(unittest.TestCase):
    """Synthetic inspector inputs only: no Rust, mounting or product PASS."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='schema5-r3-guard-', dir=s.ROOT / '.build')
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name).resolve()
        self.policy = s._timeline_policy(s.ROOT)
        self.nonce = 'a' * 32
        self.source = 'b' * 64

    def ref(self, path):
        return {'path': str(Path(path).relative_to(s.ROOT)), 'sha256': s.digest(Path(path).read_bytes())}

    def write(self, name, value):
        path = self.folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, ensure_ascii=False))
        return self.ref(path)

    def timeline(self, route, phase='begin'):
        rule = self.policy['routes'][route]
        binding = dict(route=route, case_id='guard-' + route, invocation_id='invocation-' + route,
                       exact_test=s.CHILD_TESTS.get(route, 'bj_11_storage'), nonce=self.nonce,
                       source_tree_sha256=self.source, child_pid=200, child_ppid=100,
                       original_database_path=str(self.folder / (route + '.sqlite3')),
                       database_file_device_identity={'device': 123, 'inode': 456},
                       profile_id='migration-fault-profile', server_id='controlled-fault', scope_id='scope-' + route)
        kinds = list(rule.get('strict_causal_order', rule['required_events']))
        for kind in rule['required_events']:
            if kind not in kinds:
                kinds.append(kind)
        if route in s.CHILD_TESTS:
            if route != 'rollback':
                conditional = ['transaction_attempted'] if phase == 'begin' else []
                if phase in ('ddl', 'post_ddl', 'commit'):
                    conditional = ['transaction_attempted', 'transaction_started']
                if phase in ('post_ddl', 'commit'):
                    conditional.append('ddl_applied')
                i = kinds.index('native_failure')
                kinds[i:i] = conditional
                if phase in ('ddl', 'post_ddl', 'commit'):
                    kinds.insert(kinds.index('writer_error_published'), 'transaction_scope_exited_after_drop')
            forbidden = set(rule.get('not_applicable', {})) | {'commit_succeeded'}
            if route != 'rollback':
                forbidden.add('recovery_complete')
            missing = set(self.policy['events']) - set(kinds)
            absent = {k: {'event': None, 'reason': 'route_not_applicable' if k in forbidden else 'no_transaction_started' if k == 'transaction_scope_exited_after_drop' else 'not_reached', 'failure_phase': phase} for k in missing}
        else:
            absent = {k: {'event': None, 'reason': 'route_not_applicable', 'failure_phase': None} for k in rule.get('not_applicable', {})}
        events = []
        for index, kind in enumerate(kinds, 1):
            payload = {'production_symbol': self.policy['events'][kind]['production_symbol']} if kind in self.policy['events'] else {}
            if kind == 'api_entry':
                payload['arguments'] = {k: binding[k] for k in ('original_database_path', 'profile_id', 'server_id')} | {'now_ms': 100}
            if kind == 'native_failure':
                payload.update(failure_phase=phase, native_primary_code=rule['native_primary_code'],
                               native_extended_code=rule['native_primary_code'], error_code=rule['typed_error'])
            events.append(dict(binding, event_kind=kind, event_sequence=index, monotonic_ns=100 + index,
                               observed_wall_ms=105, actual_phase=phase if kind == 'native_failure' else kind,
                               actual_thread_id='writer' if kind in ('database_writer_entry', 'backup_ready', 'before_write_barrier', 'writer_resumed', 'transaction_attempted', 'transaction_started', 'ddl_applied', 'commit_succeeded', 'native_failure', 'actual_failure_phase', 'transaction_scope_exited_after_drop', 'writer_error_published') else 'caller', payload=payload))
        return binding, {'kind': 'private-schema5-production-timeline/v1', 'events': events, 'not_reached': absent}

    def test_all_four_private_fault_timelines_accept_equal_wall_ms_with_real_sequence(self):
        for route in s.CHILD_TESTS:
            with self.subTest(route=route):
                binding, timeline = self.timeline(route, 'post_ddl' if route == 'rollback' else 'begin')
                events = s.ex_storage_timeline_consumer(s.ROOT, timeline, binding, (100, 110))
                self.assertIn('backup_ready', events)
                self.assertEqual(len(events), len(timeline['events']))

    def test_scope_backup0_fake_parent_and_out_of_order_timeline_mutants_fail(self):
        binding, original = self.timeline('rollback', 'post_ddl')
        def event(value, name):
            return next(e for e in value['events'] if e['event_kind'] == name)
        mutations = {
            'scope clock is not API entry': lambda t: event(t, 'api_entry')['payload'].update(production_symbol='source_selection_migration_fault::Scope::new'),
            'backup0': lambda t: event(t, 'backup_ready').update(observed_wall_ms=0),
            'missing backup': lambda t: t['events'].remove(event(t, 'backup_ready')),
            'fake parent': lambda t: event(t, 'native_failure').update(child_ppid=999),
            'old source': lambda t: event(t, 'api_entry').update(source_tree_sha256='0' * 64),
            'wrong SQLite code': lambda t: event(t, 'native_failure')['payload'].update(native_primary_code=8),
            'wrong path': lambda t: event(t, 'native_failure').update(original_database_path='/other.sqlite3'),
            'parent relay phase': lambda t: event(t, 'native_failure').update(actual_phase='parent_relay'),
            'future callback': lambda t: event(t, 'native_failure').update(observed_wall_ms=111),
            'sequence gap': lambda t: event(t, 'native_failure').update(event_sequence=90),
            'fake rollback clock': lambda t: event(t, 'transaction_scope_exited_after_drop').update(monotonic_ns=1),
            'wrong arguments': lambda t: event(t, 'api_entry')['payload']['arguments'].update(now_ms=99),
        }
        for name, mutate in mutations.items():
            changed = copy.deepcopy(original)
            mutate(changed)
            with self.subTest(name=name), self.assertRaises(ValueError):
                s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110))

    def test_legitimately_unreached_transaction_cannot_be_claimed_or_free_null(self):
        binding, original = self.timeline('readonly', 'pragmas')
        s.ex_storage_timeline_consumer(s.ROOT, original, binding, (100, 110))
        for name in ('transaction_started', 'transaction_scope_exited_after_drop', 'ddl_applied'):
            changed = copy.deepcopy(original)
            changed['not_reached'][name]['reason'] = 'user_says_not_applicable'
            with self.subTest(name=name), self.assertRaises(ValueError):
                s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110))
        changed = copy.deepcopy(original)
        changed['not_reached']['transaction_started']['event'] = 0
        with self.assertRaises(ValueError):
            s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110))

    def test_success_reopen_preflight_and_external_kill_consume_registered_applicability(self):
        for route_class in ('successful_upgrade_0_1_2_3_4_to5', 'reopen5', 'preflight_rejection', 'external_kill'):
            binding, timeline = self.timeline(route_class)
            if route_class.startswith('successful'):
                binding['route'] = 'from4'
                for kind, before in (('backup_ready', 'transaction_attempted'), ('ddl_applied', 'commit_succeeded')):
                    item = copy.deepcopy(timeline['events'][0]); item['event_kind'] = kind
                    item['actual_thread_id'] = 'writer'
                    item['payload'] = {'production_symbol': self.policy['events'][kind]['production_symbol']}
                    timeline['events'].insert(next(i for i, e in enumerate(timeline['events']) if e['event_kind'] == before), item)
            elif route_class == 'preflight_rejection':
                item = copy.deepcopy(timeline['events'][0]); item['event_kind'] = 'database_writer_entry'
                item['actual_thread_id'] = 'writer'
                item['payload'] = {'production_symbol': self.policy['events']['database_writer_entry']['production_symbol']}
                timeline['events'].insert(1, item)
                next(e for e in timeline['events'] if e['event_kind'] == 'actual_failure_phase')['payload'] = {'failure_phase': 'schema', 'error_code': 'unsupported_schema'}
                timeline['not_reached'] = {k: {'event': None, 'reason': 'not_reached', 'failure_phase': 'schema'} for k in ('backup_ready', 'ddl_applied', 'native_failure', 'transaction_started')}
            elif route_class == 'external_kill':
                for e in timeline['events'][:3]:
                    e['payload'] = {'pid': binding['child_pid'], 'signal': 9, 'exit_signal': 9}
            for i, e in enumerate(timeline['events'], 1):
                e.update(binding, event_sequence=i, monotonic_ns=100 + i)
            with self.subTest(route_class=route_class):
                s.ex_storage_timeline_consumer(s.ROOT, timeline, binding, (100, 110), route_class=route_class)
                changed = copy.deepcopy(timeline)
                changed['events'].pop(0)
                with self.assertRaises(ValueError):
                    s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110), route_class=route_class)

    def upgrade_timeline_guard(self):
        binding, timeline = self.timeline('successful_upgrade_0_1_2_3_4_to5')
        binding['route'] = 'from4'
        for kind, before in (('backup_ready', 'transaction_attempted'), ('ddl_applied', 'commit_succeeded')):
            item = copy.deepcopy(timeline['events'][0]); item.update(event_kind=kind, actual_thread_id='writer')
            item['payload'] = {'production_symbol': self.policy['events'][kind]['production_symbol']}
            timeline['events'].insert(next(i for i, e in enumerate(timeline['events']) if e['event_kind'] == before), item)
        for i, event in enumerate(timeline['events'], 1):
            event.update(binding, event_sequence=i, monotonic_ns=100 + i)
        return binding, timeline

    def preflight_native_timeline_guard(self):
        binding, timeline = self.timeline('preflight_rejection')
        for kind in ('database_writer_entry', 'backup_ready', 'native_failure'):
            item = copy.deepcopy(timeline['events'][0]); item.update(event_kind=kind, actual_thread_id='writer')
            item['payload'] = {'production_symbol': self.policy['events'][kind]['production_symbol']}
            if kind == 'native_failure':
                item['actual_phase'] = 'connection_open'
                item['payload'].update(failure_phase='connection_open', native_primary_code=8, native_extended_code=8, error_code='storage_failure')
            timeline['events'].insert(next(i for i, e in enumerate(timeline['events']) if e['event_kind'] == 'actual_failure_phase'), item)
        next(e for e in timeline['events'] if e['event_kind'] == 'actual_failure_phase')['payload'] = dict(failure_phase='connection_open', error_code='storage_failure', native_primary_code=8)
        timeline['not_reached'] = {k: {'event': None, 'reason': 'not_reached', 'failure_phase': 'connection_open'} for k in ('ddl_applied', 'transaction_started')}
        for i, event in enumerate(timeline['events'], 1):
            event.update(binding, event_sequence=i, monotonic_ns=100 + i)
        return binding, timeline

    def moved_boundary(self, original, moving, before=None):
        timeline = copy.deepcopy(original)
        event = next(e for e in timeline['events'] if e['event_kind'] == moving)
        timeline['events'].remove(event)
        index = next(i for i, e in enumerate(timeline['events']) if e['event_kind'] == before) if before else len(timeline['events'])
        timeline['events'].insert(index, event)
        for i, item in enumerate(timeline['events'], 1):
            item.update(event_sequence=i, monotonic_ns=100 + i)
        return timeline

    def test_review_r3_01_successful_upgrade_conditional_boundaries_obey_full_partial_order(self):
        binding, timeline = self.upgrade_timeline_guard()
        category = 'successful_upgrade_0_1_2_3_4_to5'
        s.ex_storage_timeline_consumer(s.ROOT, timeline, binding, (100, 110), route_class=category)
        for moving, before in (('backup_ready', 'api_entry'), ('backup_ready', 'database_writer_entry'),
                               ('transaction_attempted', 'backup_ready'), ('transaction_started', 'transaction_attempted'),
                               ('ddl_applied', 'transaction_started'), ('commit_succeeded', 'ddl_applied'),
                               ('api_success_return_ready', 'commit_succeeded'),
                               ('independent_current5_db_ledger_invariants', 'caller_received_success')):
            with self.subTest(moving=moving, before=before), self.assertRaises(ValueError):
                s.ex_storage_timeline_consumer(s.ROOT, self.moved_boundary(timeline, moving, before), binding, (100, 110), route_class=category)
        for kind in ('backup_ready', 'ddl_applied', 'commit_succeeded'):
            changed = copy.deepcopy(timeline)
            next(e for e in changed['events'] if e['event_kind'] == kind)['actual_thread_id'] = 'caller'
            with self.subTest(thread=kind), self.assertRaises(ValueError):
                s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110), route_class=category)

    def test_review_r3_02_preflight_native_failure_cannot_follow_return_or_readback(self):
        binding, timeline = self.preflight_native_timeline_guard()
        s.ex_storage_timeline_consumer(s.ROOT, timeline, binding, (100, 110), route_class='preflight_rejection')
        for before in ('actual_api_return_error', 'independent_original_bytes_and_backup_policy_invariant', None):
            changed = self.moved_boundary(timeline, 'native_failure', before)
            with self.subTest(before=before), self.assertRaises(ValueError):
                s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110), route_class='preflight_rejection')
        changed = copy.deepcopy(timeline)
        next(e for e in changed['events'] if e['event_kind'] == 'native_failure')['actual_thread_id'] = 'caller'
        with self.assertRaises(ValueError):
            s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110), route_class='preflight_rejection')

    def test_nonfault_conditional_transactions_cannot_omit_actual_predecessors(self):
        binding, timeline = self.timeline('reopen5')
        for kind in ('transaction_started', 'ddl_applied', 'commit_succeeded', 'writer_resumed', 'transaction_scope_exited_after_drop'):
            changed = copy.deepcopy(timeline)
            event = copy.deepcopy(changed['events'][1]); event.update(event_kind=kind, actual_thread_id='writer')
            event['payload'] = {'production_symbol': self.policy['events'][kind]['production_symbol']}
            changed['events'].insert(2, event)
            for i, item in enumerate(changed['events'], 1):
                item.update(event_sequence=i, monotonic_ns=100 + i)
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110), route_class='reopen5')

    def test_actual_child_launch_requires_exact_parent_stdout_binding(self):
        stdout = self.folder / 'guard.stdout'; stdout.write_text('raw guard input')
        stderr = self.folder / 'guard.stderr'; stderr.write_text('')
        command = ['/owned/guard-binary', '--exact', s.CHILD_TESTS['rollback'], '--nocapture']
        environment = {'LUNA_BINDING_NONCE': self.nonce}
        row = dict(command=command, cwd='.', fixture_environment=environment, pid=201, timed_out=False,
                   started_at='1970-01-01T00:00:00.100+00:00', finished_at='1970-01-01T00:00:00.110+00:00',
                   exit_code=0, stdout=self.ref(stdout), stderr=self.ref(stderr))
        ref = self.write('execution.json', row)
        launches = [{'command': command, 'pid': 201, 'execution': ref}]
        self.assertEqual(s._child_execution(s.ROOT, ref, command, environment, (90, 120), launches)[0], row)
        for changed in ([], [dict(launches[0], pid=999)], [launches[0], launches[0]]):
            with self.assertRaises(ValueError):
                s._child_execution(s.ROOT, ref, command, environment, (90, 120), changed)
        for key, value in (('pid', True), ('timed_out', True), ('exit_code', 1), ('fixture_environment', {})):
            bad = dict(row, **{key: value}); badref = self.write('bad.json', bad)
            with self.subTest(key=key), self.assertRaises(ValueError):
                s._child_execution(s.ROOT, badref, command, environment, (90, 120), [{'command': command, 'pid': value if key == 'pid' else 201, 'execution': badref}])

    def test_child_raw_frames_reject_free_log_prefixes_and_ignored_zero_results(self):
        name = s.CHILD_TESTS['rollback']
        row = {'pid': 1}
        self.assertEqual(s._raw_frames(s.CHILD_ID_MARKER + json.dumps(row), s.CHILD_ID_MARKER, name), [row])
        for prefix in ('debug: ', 'test wrong ... ', ' '):
            with self.assertRaises(ValueError):
                s._raw_frames(prefix + s.CHILD_ID_MARKER + json.dumps(row), s.CHILD_ID_MARKER, name)
        for summary in ('test result: ok. 0 passed; 0 failed; 1 ignored;', 'test result: ok. 0 passed; 0 failed; 0 ignored;', 'test result: FAILED. 0 passed; 1 failed; 0 ignored;'):
            with self.assertRaises(ValueError):
                s._actual_test_summary('test ' + name + ' ... ignored\n' + summary, [name])

    def test_missing_free_child_refs_and_untrusted_context_cannot_reach_product(self):
        missing = self.write('missing-child.json', {'kind': s.CHILD_KIND, 'status': 'passed', 'child_refs': []})
        with self.assertRaises(ValueError):
            s._inspect_child_proof(s.ROOT, self.folder, missing, {}, {}, self.folder / 'parent', {}, [], [])
        for name in ('rollback', 'readonly', 'busy', 'enospc'):
            binding, timeline = self.timeline(name, 'post_ddl' if name == 'rollback' else 'begin')
            for key in ('child_pid', 'child_ppid', 'nonce', 'source_tree_sha256'):
                changed = copy.deepcopy(timeline)
                changed['events'][0][key] = 999 if 'pid' in key else '0' * len(str(binding[key]))
                with self.subTest(route=name, key=key), self.assertRaises(ValueError):
                    s.ex_storage_timeline_consumer(s.ROOT, changed, binding, (100, 110))
        result = s.inspect_runtime()
        self.assertEqual(result['status'], 'blocked')
        self.assertFalse(result['product_migration_executed'])

    def complete_child_guard(self):
        """Construct coherent raw inspector fixtures, never execute a producer."""
        from binding_phase_guard import tree_sha
        from datetime import datetime, timezone
        root = s.ROOT
        source_names = ['Cargo.toml', 'Cargo.lock', 'crates/app-infra/src/lib.rs',
                        'crates/app-infra/src/storage/database.rs', s.PRODUCTION_SOURCE]
        source_names.append(str(next((root / 'crates/app-core/src').glob('*.rs')).relative_to(root)))
        sources = {p: s.digest(s.safe_bytes(root, p)) for p in source_names}
        self.source = tree_sha(sources)
        environment = dict(LUNA_BINDING_NONCE=self.nonce, LUNA_BINDING_SOURCE_TREE=self.source,
                           LUNA_BINDING_OUTPUT=str(self.folder))
        parent_binary = self.folder / 'parent-guard-binary'; parent_binary.write_bytes(b'guard parent bytes, not executable')
        child_binary = self.folder / 'child-guard-binary'; child_binary.write_bytes(b'guard child bytes, not executable')
        child_ref, parent_ref = self.ref(child_binary), self.ref(parent_binary)
        old_writer = self.ref(root / '.build/binding-journey-alignment-20261109/frozen-store4/reader-manifest.json')
        launches, exact_refs, records, routes, snapshots, relays = [], [], {}, [], {}, []
        def execution(label, command, pid, start, end, stdout):
            path = self.folder / (label + '.stdout'); path.write_text(stdout)
            error = self.folder / (label + '.stderr'); error.write_text('')
            clock = lambda ms: datetime.fromtimestamp(ms / 1000, timezone.utc).isoformat()
            row = dict(command=command, cwd='.', pid=pid, timed_out=False, started_at=clock(start),
                       finished_at=clock(end), exit_code=0, fixture_environment=environment,
                       stdout=self.ref(path), stderr=self.ref(error))
            return self.write(label + '.execution.json', row), row
        cargo = [{'reason': 'compiler-artifact', 'package_id': 'path+file://' + str(root / 'crates/app-infra') + '#app-infra@0.2.0',
                  'target': {'name': 'app_infra', 'kind': ['lib'], 'src_path': str(root / 'crates/app-infra/src/lib.rs')},
                  'profile': {'test': True}, 'executable': str(child_binary)}, {'reason': 'build-finished', 'success': True}]
        compile_ref, _ = execution('compile', s.CHILD_COMPILE, 110, 100, 110, '\n'.join(json.dumps(x) for x in cargo))
        launches.append({'pid': 110, 'command': s.CHILD_COMPILE, 'execution': compile_ref})
        list_command = [str(child_binary), '--list', '--format', 'terse']
        list_ref, _ = execution('list', list_command, 111, 120, 130, '\n'.join(n + ': test' for n in sorted(set(s.CHILD_TESTS.values()))))
        launches.append({'pid': 111, 'command': list_command, 'execution': list_ref})
        for index, name in enumerate(dict.fromkeys(s.CHILD_TESTS.values())):
            pid = 200 + index
            begin, end = ((150, 300), (400, 600), (700, 900))[index]
            identity = dict(pid=pid, ppid=100, binary=child_ref, nonce=self.nonce,
                            source_tree_sha256=self.source, output=str(self.folder), old_writer_manifest=old_writer)
            lines = ['test ' + name + ' ...', s.CHILD_ID_MARKER + json.dumps(identity)]
            for route in (r for r, test in s.CHILD_TESTS.items() if test == name):
                binding, timeline = self.timeline(route, 'post_ddl' if route == 'rollback' else 'begin')
                binding.update(child_pid=pid, source_tree_sha256=self.source)
                case = self.folder / route; case.mkdir()
                original = case / ('mount/original-four.sqlite3' if route == 'enospc' else 'original-four.sqlite3')
                original.parent.mkdir(parents=True, exist_ok=True)
                before = case / 'before.sqlite3'
                with s.v4.design_database(root) as db:
                    db.execute("INSERT INTO profile_meta VALUES(1,'migration-fault-profile',0,1,'old','active',0,0)")
                    hashes = [s.PROFILE_LEDGER_SHA256] + [s.PRESERVED['crates/app-infra/src/storage/migrations/' + p] for p in ('002_session_records.sql', '003_http_service_identity.sql', '004_service_graph.sql')]
                    for version, sha in enumerate(hashes, 1):
                        db.execute('INSERT INTO schema_migrations VALUES(?,?,0,?,?)', (version, sha, 'guard', version - 1))
                    s._seed(db)
                    target = sqlite3.connect(before); db.backup(target); target.close()
                after, backup = case / 'after.sqlite3', case / 'backup.sqlite3'
                shutil.copyfile(before, original); shutil.copyfile(before, after); shutil.copyfile(before, backup)
                binding.update(original_database_path=str(original), database_file_device_identity={'device': original.stat().st_dev, 'inode': original.stat().st_ino})
                if route == 'enospc':
                    # Synthetic detached-volume fact only; no disk is mounted.
                    binding['database_file_device_identity']['device'] = 990
                for event in timeline['events']:
                    event.update(binding, observed_wall_ms=200 if index == 0 else 500 if index == 1 else 800)
                    if event['event_kind'] == 'api_entry':
                        event['payload']['arguments'] = {k: binding[k] for k in ('original_database_path', 'profile_id', 'server_id')} | {'now_ms': 100}
                if route == 'enospc':
                    times = {'native_failure': 810, 'writer_error_published': 811, 'api_error_return_ready': 812,
                             'caller_received_error': 820, 'post_error_invariants': 880, 'cleanup_complete': 890}
                    for event in timeline['events']:
                        event['observed_wall_ms'] = times.get(event['event_kind'], 800)
                refs = dict(before=self.ref(before), after=self.ref(after), backup=self.ref(backup))
                next(e for e in timeline['events'] if e['event_kind'] == 'post_error_invariants')['payload']['snapshots'] = refs
                record = dict(binding, kind='actual-schema5-migration-host-fault/v2', now_ms=100,
                              writer='SqliteStore::open', writer_process_binary=child_ref,
                              **refs,
                              recovery=None, timeline=self.write(route + '.timeline.json', timeline), os_facts={})
                event_times = {e['event_kind']: e['observed_wall_ms'] for e in timeline['events']}
                record['captures'] = {key: dict(source_path=str(original.with_name(original.name + '.pre-v5-100.sqlite3') if key == 'backup' else original),
                    source_identity=dict(binding['database_file_device_identity'], inode=backup.stat().st_ino) if key == 'backup' else binding['database_file_device_identity'],
                    snapshot=refs[key], captured_at_ms=begin + 46 if key == 'before' else event_times['caller_received_error'] if key == 'after' else event_times['post_error_invariants']) for key in refs}
                if route == 'rollback':
                    recovered = case / 'recovered.sqlite3'; shutil.copyfile(before, recovered)
                    with closing(sqlite3.connect(recovered)) as db, db:
                        db.executescript(s.safe_bytes(root, s.DESIGN).decode())
                        db.execute('INSERT INTO schema_migrations VALUES(5,?,101,?,4)', (s.DESIGN_SHA256, 'guard'))
                        db.execute("UPDATE profile_meta SET runtime_generation=runtime_generation+1,server_instance_id='recovered',runtime_state='active',updated_at_ms=101")
                        db.execute("UPDATE sessions SET state='crashed',ended_at_ms=101 WHERE state IN ('opening','active')")
                    record['recovery'] = dict(snapshot=self.ref(recovered), original_database_path=str(original), profile_id=binding['profile_id'], server_id='recovered', now_ms=101)
                elif route == 'readonly':
                    record['os_facts'] = dict(before_mode=0o644, applied_mode=0o444, restored_mode=0o644, applied_at_ms=500, restored_at_ms=500)
                elif route == 'busy':
                    record['os_facts'] = dict(holder_pid=pid, connection_id='real-connection-guard-input', acquired_at_ms=500, released_at_ms=500,
                                              begin_sql='BEGIN IMMEDIATE', release_sql='ROLLBACK', begin_success=True, release_success=True)
                else:
                    import plistlib
                    image, mount = case / 'limited-64m.dmg', case / 'mount'
                    plist = plistlib.dumps({'system-entities': [{'dev-entry': '/dev/disk900s1', 'mount-point': str(mount)}]}).decode()
                    commands = []
                    for label, args, cstart, cend, raw in (
                        ('create', ['create', '-size', '64m', '-fs', 'HFS+', '-volname', 'LunaSchema5Full', '-type', 'UDIF', '-nospotlight', str(image)], 710, 720, ''),
                        ('attach', ['attach', '-plist', '-nobrowse', '-mountpoint', str(mount), str(image)], 730, 740, plist),
                        ('detach', ['detach', '/dev/disk900'], 886, 888, '')):
                        command_ref, _ = execution('volume-' + label, ['hdiutil'] + args, 300 + len(commands), cstart, cend, raw)
                        commands.append(command_ref)
                    record['os_facts'] = dict(commands=commands, image=str(image), mount=str(mount), device='/dev/disk900',
                        mount_device=990, output_device=self.folder.stat().st_dev, capacity_bytes=64*1024**2, available_bytes_at_fault=8192,
                        observed_os_error=28, os_error_at_ms=800, capacity_sampled_at_ms=800, filler_path=str(mount / 'filler'),
                        filler_bytes=60*1024**2, filler_removed_at_ms=850, evidence_copied_at_ms=885)
                records[route] = self.write(route + '.fault.json', record)
                for key, value in (('before', before), ('after', after), ('backup', backup)):
                    snapshots[route + '-' + key] = {'path': str(value.relative_to(self.folder)), 'sha256': self.ref(value)['sha256']}
                routes.append(dict(id=route, before=route + '-before', after=route + '-after', backup=route + '-backup'))
                relay_map = {'writer_open_started': 'api_entry', 'fault_armed': 'fault_applied', 'writer_open_failed': 'native_failure',
                             'migration_transaction_started': 'transaction_started', 'migration_ddl_applied': 'ddl_applied', 'transaction_rolled_back': 'transaction_scope_exited_after_drop'}
                for relay, event_kind in relay_map.items():
                    event = next((e for e in timeline['events'] if e['event_kind'] == event_kind), None)
                    if event:
                        relays.append(dict(route=route, event=relay, at_ms=event['observed_wall_ms']))
                lines.extend(s.BOUNDARY_MARKER + json.dumps(e) for e in timeline['events'])
                lines.append(s.FAULT_MARKER + json.dumps(record))
            lines.extend(['ok', 'test result: ok. 1 passed; 0 failed; 0 ignored;'])
            command = [str(child_binary), '--exact', name, '--nocapture']
            exact_ref, _ = execution('exact-' + str(index), command, pid, begin, end, '\n'.join(lines))
            exact_refs.append(exact_ref); launches.append(dict(pid=pid, command=command, execution=exact_ref))
        parent_executions, parent_refs = {}, []
        for name in s.TEST_NAMES:
            raw = '\n'.join(s.CHILD_LAUNCH_MARKER + json.dumps(e) for e in launches) if name == 'bj_11_storage' else ''
            raw += '\ntest ' + name + ' ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;'
            pref, row = execution(name, [str(parent_binary), '--exact', name, '--nocapture'], 100 if name == 'bj_11_storage' else 400 + len(parent_refs), 90, 1000, raw)
            parent_executions[name] = row; parent_refs.append(pref)
        context = dict(nonce=self.nonce, source_tree_sha256=self.source, parent_executions=parent_executions,
                       source_files=sources, executions=parent_refs)
        proof = dict(kind=s.CHILD_KIND, parent=dict(execution=parent_refs[list(s.TEST_NAMES).index('bj_11_storage')], binary=parent_ref, pid=100),
                     source_files=self.write('sources.json', sources), cargo_lock=self.ref(root / 'Cargo.lock'),
                     old_writer_manifest=old_writer,
                     compilation=compile_ref, binary=child_ref, listing=list_ref, executions=exact_refs, fault_records=records)
        return proof, context, environment, parent_binary, snapshots, routes, relays

    def test_complete_child_guard_consumes_compile_list_three_exact_four_faults_and_databases(self):
        values = self.complete_child_guard()
        proof, context, environment, parent, snapshots, routes, relays = values
        ref = self.write('child-proof.json', proof)
        result = s._inspect_child_proof(s.ROOT, self.folder, ref, context, environment, parent, snapshots, routes, relays)
        self.assertEqual(result['fault_routes'], sorted(s.CHILD_TESTS))
        self.assertEqual(len(result['child_tests']), 3)
        self.assertGreater(result['production_timeline_events'], 50)
        self.assertNotIn('product_acceptance', result)
        for label, mutate in (
            ('missingchild', lambda p: p.pop('executions')),
            ('fakeparent', lambda p: p['parent'].update(pid=999)),
            ('freesupplement', lambda p: p.update(extra_child_refs=[])),
            ('parent_as_child', lambda p: p.update(binary=proof['parent']['binary'])),
            ('missingfault', lambda p: p['fault_records'].pop('busy')),
            ('reusedfault', lambda p: p['fault_records'].update(busy=p['fault_records']['readonly'])),
        ):
            changed = copy.deepcopy(proof); mutate(changed)
            with self.subTest(label=label), self.assertRaises(ValueError):
                s._inspect_child_proof(s.ROOT, self.folder, self.write('mutant-' + label + '.json', changed), context, environment, parent, snapshots, routes, relays)

    def current_five_guard_database(self, name):
        path = self.folder / name
        with s.design_database(s.ROOT) as source:
            source.execute("INSERT INTO profile_meta VALUES(1,'migration-fault-profile',0,1,'guard','active',0,0)")
            hashes = [s.PROFILE_LEDGER_SHA256] + [s.PRESERVED['crates/app-infra/src/storage/migrations/' + p] for p in ('002_session_records.sql', '003_http_service_identity.sql', '004_service_graph.sql')] + [s.DESIGN_SHA256]
            for version, sha in enumerate(hashes, 1):
                source.execute('INSERT INTO schema_migrations VALUES(?,?,0,?,?)', (version, sha, 'guard', version - 1))
            s._seed(source)
            with closing(sqlite3.connect(path)) as target:
                source.backup(target)
        return path, {'path': path.name, 'sha256': s.digest(path.read_bytes())}

    def old4_observation_parts(self, database, snapshot):
        # Simulated raw parent observations: the frozen reader is not executed.
        manifest_ref = self.ref(s.ROOT / '.build/binding-journey-alignment-20261109/frozen-store4/reader-manifest.json')
        manifest = s._loads(s._artifact(s.ROOT, manifest_ref).read_bytes())
        binary = s._owned_path(s.ROOT, manifest['binary_path'])
        binary_ref = {'path': str(binary.relative_to(s.ROOT)), 'sha256': manifest['binary_sha256']}
        binding = dict(route='old4-rejects5', case_id='case-old4', invocation_id='invoke-old4', exact_test='bj_11_storage',
                       nonce=self.nonce, source_tree_sha256=self.source, observer_pid=400, reader_pid=201,
                       profile_id='migration-fault-profile', original_database_path=str(database),
                       database_file_device_identity={'device': database.stat().st_dev, 'inode': database.stat().st_ino})
        stdout = self.folder / 'old-reader.stdout'; stdout.write_text(json.dumps({'opened': False, 'schema_version': 4, 'error_code': 'unsupported_schema', 'error': 'guard unsupported_schema'}))
        stderr = self.folder / 'old-reader.stderr'; stderr.write_text('')
        environment = dict(LUNA_BINDING_OUTPUT=str(self.folder), LUNA_BINDING_NONCE=self.nonce, LUNA_BINDING_SOURCE_TREE=self.source)
        command = [str(binary), str(database), binding['profile_id'], 'reject']
        reader = dict(command=command, cwd='.', pid=201, parent_pid=400, timed_out=False,
                      started_at='1970-01-01T00:00:00.100+00:00', finished_at='1970-01-01T00:00:00.105+00:00',
                      fixture_environment=environment, exit_code=0, stdout=self.ref(stdout), stderr=self.ref(stderr))
        reader_ref = self.write('old-reader.execution.json', reader)
        payloads = [{'command': command, 'binary': binary_ref, 'execution': reader_ref}, {'exit_code': 0},
                    {'stdout': reader['stdout'], 'stderr': reader['stderr'], 'error_code': 'unsupported_schema'},
                    {'before': snapshot, 'after': snapshot, 'original_database_path': str(database),
                     'before_identity': binding['database_file_device_identity'], 'after_identity': binding['database_file_device_identity']}]
        observations = [dict(kind=kind, sequence=i, monotonic_ns=100 + i, observed_wall_ms=105,
                             **{k: binding[k] for k in ('observer_pid', 'reader_pid', 'route', 'invocation_id', 'nonce', 'source_tree_sha256')}, payload=payload)
                        for i, (kind, payload) in enumerate(zip(('reader_spawned', 'reader_waited', 'reader_result_captured', 'original_database_verified'), payloads), 1)]
        envelope = dict(kind='schema5-frozen-old-reader-observation/v1', binding=binding, old_writer_manifest=manifest_ref,
                        reader_execution=reader_ref, observations=observations)
        return envelope, environment

    def test_parent_timeline_raw_envelopes_cover_all_nine_preserved_routes(self):
        # This isolated parser fixture does not bypass the separate full-schema
        # runtime oracle, which still executes for every real route.
        database, snapshot = self.current_five_guard_database('parent-after.sqlite3')
        old4_envelope, environment = self.old4_observation_parts(database, snapshot)
        routes, frames, prepared, relays = [], [], {}, []
        names = ('fresh', 'from1', 'from2', 'from3', 'from4', 'reopen5', 'old4-rejects5', 'kill', 'restore')
        for name in names:
            if name == 'old4-rejects5':
                routes.append(dict(id=name, before='snapshot', after='snapshot', backup=None))
                continue
            category = 'external_kill' if name == 'kill' else 'reopen5' if name == 'reopen5' else 'preflight_rejection' if name == 'old4-rejects5' else 'successful_upgrade_0_1_2_3_4_to5'
            binding, timeline = self.timeline(category)
            binding.update(route=name, exact_test='bj_11_storage', child_pid=201 if name == 'kill' else 400,
                           child_ppid=400 if name == 'kill' else 300, case_id='case-' + name,
                           invocation_id='invoke-' + name, scope_id='scope-' + name, original_database_path=str(database))
            if category.startswith('successful'):
                for kind, before in (('backup_ready', 'transaction_attempted'), ('ddl_applied', 'commit_succeeded')):
                    if kind == 'backup_ready' and name == 'fresh':
                        timeline['not_reached']['backup_ready'] = {'event': None, 'reason': 'no_old_database', 'failure_phase': None}
                        continue
                    event = copy.deepcopy(timeline['events'][0]); event['event_kind'] = kind
                    event['actual_thread_id'] = 'writer'
                    event['payload'] = {'production_symbol': self.policy['events'][kind]['production_symbol']}
                    timeline['events'].insert(next(i for i, e in enumerate(timeline['events']) if e['event_kind'] == before), event)
            elif category == 'preflight_rejection':
                event = copy.deepcopy(timeline['events'][0]); event['event_kind'] = 'database_writer_entry'
                event['actual_thread_id'] = 'writer'
                event['payload'] = {'production_symbol': self.policy['events']['database_writer_entry']['production_symbol']}
                timeline['events'].insert(1, event)
                next(e for e in timeline['events'] if e['event_kind'] == 'actual_failure_phase')['payload'] = {'failure_phase': 'schema', 'error_code': 'unsupported_schema'}
                timeline['not_reached'] = {k: {'event': None, 'reason': 'not_reached', 'failure_phase': 'schema'} for k in ('backup_ready', 'ddl_applied', 'native_failure', 'transaction_started')}
            elif category == 'external_kill':
                for event in timeline['events'][:3]:
                    event['payload'] = {'pid': 201, 'signal': 9, 'exit_signal': 9}
                next(e for e in timeline['events'] if e['event_kind'] == 'actual_recovery')['payload']['snapshot'] = snapshot
            before = None if name == 'fresh' else 'snapshot'
            backup = 'snapshot' if category.startswith('successful') and name != 'fresh' else None
            routes.append(dict(id=name, before=before, after='snapshot', backup=backup))
            kind = 'independent_post_kill_db_ledger_backup_invariants' if name == 'kill' else 'independent_original_bytes_and_backup_policy_invariant' if name == 'old4-rejects5' else 'independent_current5_db_ledger_invariants'
            next(e for e in timeline['events'] if e['event_kind'] == kind)['payload'].update(before=snapshot if before else None, after=snapshot, backup=snapshot if backup else None)
            for i, event in enumerate(timeline['events'], 1):
                event.update(binding, event_sequence=i, monotonic_ns=100 + i)
            frames.extend(timeline['events'])
            prepared[name] = (binding, timeline)
            relays.append(dict(route=name, event='writer_open_started', facts={'server_id': binding['server_id']}))
            if name == 'kill':
                relays.append(dict(route=name, event='child_started', facts={'pid': 201}))
        stdout = self.folder / 'parent-timelines.stdout'; stdout.write_text('\n'.join(
            [s.BOUNDARY_MARKER + json.dumps(e) for e in frames] +
            [s.OLD4_OBSERVER_MARKER + json.dumps(e) for e in old4_envelope['observations']]))
        stderr = self.folder / 'parent-timelines.stderr'; stderr.write_text('')
        execution = dict(command=[str(self.folder / 'parent'), '--exact', 'bj_11_storage', '--nocapture'], cwd='.', pid=400,
                         started_at='1970-01-01T00:00:00.100+00:00', finished_at='1970-01-01T00:00:00.110+00:00', exit_code=0,
                         fixture_environment=environment, stdout=self.ref(stdout), stderr=self.ref(stderr))
        execution_ref = self.write('parent-timelines.execution.json', execution)
        refs = {name: self.write(name + '.parent-timeline.json', dict(kind='schema5-parent-route-timeline/v1', execution=execution_ref, binding=binding, timeline=timeline)) for name, (binding, timeline) in prepared.items()}
        old4_envelope['execution'] = execution_ref
        refs['old4-rejects5'] = self.write('old4.parent-observation.json', old4_envelope)
        context = dict(parent_executions={'bj_11_storage': execution}, nonce=self.nonce, source_tree_sha256=self.source)
        self.assertEqual(s._inspect_parent_timelines(s.ROOT, refs, context, routes, {'snapshot': snapshot}, relays), sorted(names))
        for changed in ({}, dict(refs, from1=refs['from2'])):
            with self.assertRaises(ValueError):
                s._inspect_parent_timelines(s.ROOT, changed, context, routes, {'snapshot': snapshot}, relays)
        wrong = copy.deepcopy(context); wrong['parent_executions']['bj_11_storage']['pid'] = 401
        with self.assertRaises(ValueError):
            s._inspect_parent_timelines(s.ROOT, refs, wrong, routes, {'snapshot': snapshot}, relays)

    def test_review_r3_03_old4_uses_real_separate_actor_and_external_observation_only(self):
        database, snapshot = self.current_five_guard_database('old4-current-five.sqlite3')
        envelope, environment = self.old4_observation_parts(database, snapshot)
        stdout = self.folder / 'old4-parent.stdout'
        stderr = self.folder / 'old4-parent.stderr'; stderr.write_text('')
        parent = dict(command=[str(self.folder / 'parent'), '--exact', 'bj_11_storage', '--nocapture'], cwd='.', pid=400,
                      started_at='1970-01-01T00:00:00.100+00:00', finished_at='1970-01-01T00:00:00.110+00:00',
                      exit_code=0, fixture_environment=environment, stderr=self.ref(stderr))
        context = dict(nonce=self.nonce, source_tree_sha256=self.source)
        route = dict(id='old4-rejects5', before='snapshot', after='snapshot', backup=None)
        def consume(candidate):
            stdout.write_text('\n'.join(s.OLD4_OBSERVER_MARKER + json.dumps(e) for e in candidate.get('observations', [])))
            actual_parent = dict(parent, stdout=self.ref(stdout))
            candidate['execution'] = self.write('old4-parent.execution.json', actual_parent)
            return s._inspect_frozen_old_reader(s.ROOT, candidate, actual_parent, context, route, {'snapshot': snapshot})
        result = consume(copy.deepcopy(envelope))
        self.assertEqual(result, {'observer_pid': 400, 'reader_pid': 201, 'external_observation_only': True})
        for mutate in (
            lambda e: e['binding'].update(reader_pid=400),
            lambda e: e['binding'].update(observer_pid=999),
            lambda e: e.update(kind='schema5-parent-route-timeline/v1'),
            lambda e: e['observations'].reverse(),
            lambda e: e['observations'][1].update(observed_wall_ms=99),
            lambda e: e['observations'][0]['payload'].update(command=['fake-current-writer']),
            lambda e: e['observations'][2]['payload'].update(error_code='passed'),
        ):
            candidate = copy.deepcopy(envelope); mutate(candidate)
            with self.assertRaises(ValueError):
                consume(candidate)
        actual = s._loads(s._artifact(s.ROOT, envelope['reader_execution']).read_bytes())
        for key, value in (('pid', 400), ('parent_pid', 999), ('exit_code', 1), ('timed_out', True),
                           ('command', [actual['command'][0], str(database), 'wrong-profile', 'reject'])):
            candidate = copy.deepcopy(envelope)
            bad = dict(actual, **{key: value})
            candidate['reader_execution'] = self.write('old4-bad-reader.execution.json', bad)
            with self.subTest(key=key), self.assertRaises(ValueError):
                consume(candidate)
        wrong_stdout = self.folder / 'old4-opened.stdout'
        wrong_stdout.write_text(json.dumps({'opened': True, 'schema_version': 4, 'error_code': 'unsupported_schema', 'error': 'guard'}))
        candidate = copy.deepcopy(envelope)
        candidate['reader_execution'] = self.write('old4-opened.execution.json', dict(actual, stdout=self.ref(wrong_stdout)))
        with self.assertRaises(ValueError):
            consume(candidate)

    def test_old4_external_observation_still_checks_all_original_sqlite_values(self):
        before, before_ref = self.current_five_guard_database('old4-before.sqlite3')
        after = self.folder / 'old4-after.sqlite3'; shutil.copyfile(before, after)
        with closing(sqlite3.connect(after)) as db, db:
            db.execute("UPDATE profile_meta SET server_instance_id='changed-by-reader'")
        after_ref = {'path': after.name, 'sha256': s.digest(after.read_bytes())}
        envelope, environment = self.old4_observation_parts(before, before_ref)
        envelope['observations'][-1]['payload']['after'] = after_ref
        stdout = self.folder / 'old4-values-parent.stdout'; stdout.write_text('\n'.join(s.OLD4_OBSERVER_MARKER + json.dumps(e) for e in envelope['observations']))
        stderr = self.folder / 'old4-values-parent.stderr'; stderr.write_text('')
        parent = dict(command=[str(self.folder / 'parent'), '--exact', 'bj_11_storage', '--nocapture'], cwd='.', pid=400,
                      started_at='1970-01-01T00:00:00.100+00:00', finished_at='1970-01-01T00:00:00.110+00:00', exit_code=0,
                      fixture_environment=environment, stdout=self.ref(stdout), stderr=self.ref(stderr))
        envelope['execution'] = self.write('old4-values-parent.execution.json', parent)
        with self.assertRaises(ValueError):
            s._inspect_frozen_old_reader(s.ROOT, envelope, parent, dict(nonce=self.nonce, source_tree_sha256=self.source),
                dict(before='before', after='after', backup=None), {'before': before_ref, 'after': after_ref})

    def test_os_fact_controls_reject_early_release_missing_detach_and_host_volume(self):
        proof, _, _, _, _, _, _ = self.complete_child_guard()
        for route in ('readonly', 'busy', 'enospc'):
            record = s._loads(s._artifact(s.ROOT, proof['fault_records'][route]).read_bytes())
            timeline = s._loads(s._artifact(s.ROOT, record['timeline']).read_bytes())
            events = {e['event_kind']: e for e in timeline['events']}
            period = (400, 600) if route != 'enospc' else (700, 900)
            s._fault_os_facts(s.ROOT, record, events, period, record['child_pid'])
            mutations = [lambda r: r['os_facts'].update(restored_mode=0o444)] if route == 'readonly' else [
                lambda r: r['os_facts'].update(released_at_ms=499),
                lambda r: r['os_facts'].update(holder_pid=999),
            ] if route == 'busy' else [
                lambda r: r['os_facts'].update(commands=r['os_facts']['commands'][:2]),
                lambda r: r['os_facts'].update(mount_device=r['os_facts']['output_device']),
                lambda r: r['os_facts'].update(observed_os_error=0),
                lambda r: r['os_facts'].update(evidence_copied_at_ms=899),
            ]
            for mutate in mutations:
                changed = copy.deepcopy(record); mutate(changed)
                with self.subTest(route=route), self.assertRaises(ValueError):
                    s._fault_os_facts(s.ROOT, changed, events, period, record['child_pid'])

    def test_database_equivalence_preserves_row_factory_and_every_original_value(self):
        before, after = sqlite3.connect(':memory:'), sqlite3.connect(':memory:')
        self.addCleanup(before.close); self.addCleanup(after.close)
        for db in (before, after):
            db.executescript("CREATE TABLE guard_history(id INTEGER PRIMARY KEY,source_text TEXT);INSERT INTO guard_history VALUES(1,'原文');")
        s._same_database(before, after)
        self.assertIsNone(before.row_factory)
        self.assertIsNone(after.row_factory)
        after.execute("UPDATE guard_history SET source_text='改动'")
        with self.assertRaises(ValueError):
            s._same_database(before, after)
        self.assertIsNone(before.row_factory)
        self.assertIsNone(after.row_factory)

    def test_fault_snapshot_capture_lineage_rejects_wrong_original_and_backup_alias(self):
        proof, _, _, _, _, _, _ = self.complete_child_guard()
        record = s._loads(s._artifact(s.ROOT, proof['fault_records']['rollback']).read_bytes())
        timeline = s._loads(s._artifact(s.ROOT, record['timeline']).read_bytes())
        events = {e['event_kind']: e for e in timeline['events']}
        s._fault_capture_lineage(s.ROOT, record, events, (150, 300))
        for mutate in (
            lambda r: r['captures']['after'].update(source_path=str(self.folder / 'other.sqlite3')),
            lambda r: r['captures']['backup'].update(source_identity=r['database_file_device_identity']),
            lambda r: r['captures']['after'].update(captured_at_ms=199),
            lambda r: r['captures']['before'].update(captured_at_ms=201),
        ):
            changed = copy.deepcopy(record); mutate(changed)
            with self.assertRaises(ValueError):
                s._fault_capture_lineage(s.ROOT, changed, events, (150, 300))


class FrozenStoreFourSuiteProvenanceGuards(unittest.TestCase):
    """Synthetic file-level negatives; these never grant migration/product credit."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='schema5-old-suite-guard-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.base = self.root / '.build/binding-journey-alignment-20261109/frozen-store4'
        self.source = self.base / 'source-root'
        self.counts = (6, 15, 3, 5, 6, 2, 5, 65, 19)
        self.rows, self.artifacts, self.inventories = [], {}, []
        files = {}
        for name in ('Cargo.lock', 'Cargo.toml', 'crates/app-infra/Cargo.toml'):
            path = self.source / name
            self.write(path, ('synthetic fixed ' + name).encode())
            files[name] = s._file_hash(path)
        for index, (target, count) in enumerate(zip(s.OLD_TEST_TARGETS, self.counts)):
            source_name = 'crates/app-infra/tests/' + target + '.rs'
            self.write(self.source / source_name, ('synthetic original ' + target).encode())
            files[source_name] = s._file_hash(self.source / source_name)
            folder = self.base / ('original-schema-suite' if index == 0 else 'original-dependent-suites-corrected-v3')
            copied = folder / ('actual-frozen-storage-schema-contract' if index == 0 else 'actual-frozen-' + target)
            self.write(copied, ('synthetic retained binary ' + target).encode())
            names = [target + '_case_' + str(n) for n in range(count)]
            stdout = folder / (target + '.stdout.log')
            stderr = folder / (target + '.stderr.log')
            self.write(stdout, ('\n'.join('test ' + n + ' ... ok' for n in names)
                                + '\ntest result: ok. ' + str(count) + ' passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n').encode())
            self.write(stderr, b'')
            row = self.execution([str(copied), '--nocapture'], stdout, stderr)
            row.update(target=target, source_sha256=files[source_name], binary_sha256=s._file_hash(copied))
            self.rows.append(row)
            listing_stdout = self.base / 'lists' / (target + '.stdout.log')
            listing_stderr = self.base / 'lists' / (target + '.stderr.log')
            self.write(listing_stdout, ('\n'.join(n + ': test' for n in names) + '\n').encode())
            self.write(listing_stderr, b'')
            listing = self.execution([str(copied), '--list', '--format', 'terse'], listing_stdout, listing_stderr)
            listing.update(target=target, binary_sha256=row['binary_sha256'], test_names=names)
            self.inventories.append(listing)
            executable = self.root / 'target/debug/deps' / (target + '-0123456789abcdef')
            self.artifacts[target] = dict(reason='compiler-artifact',
                package_id='path+' + (self.source / 'crates/app-infra').as_uri() + '#0.2.0',
                manifest_path=str(self.source / 'crates/app-infra/Cargo.toml'),
                target=dict(kind=['test'], crate_types=['bin'], name=target, src_path=str(self.source / source_name), test=True),
                profile=dict(test=True), filenames=[str(executable)], executable=str(executable))
        self.frozen = dict(source_root=str(self.source.relative_to(self.root)), files=files)
        self.frozen_path = self.base / 'source-manifest.json'
        self.write_json(self.frozen_path, self.frozen)
        self.compilations = [self.compilation(s.OLD_TEST_TARGETS[:1], 0), self.compilation(s.OLD_TEST_TARGETS[1:], 1)]
        compile_path = self.base / 'compile-execution.json'
        self.write_json(compile_path, self.compilations[1])
        first = self.rows[0]
        self.schema = dict(kind='unmodified-frozen-Store4-original-schema-test-execution/v1',
            test_source_sha256=first['source_sha256'], binary_sha256=first['binary_sha256'],
            compilation=self.compilations[0], test_execution={k:v for k,v in first.items() if k not in ('target', 'source_sha256', 'binary_sha256')})
        self.dependent = dict(kind='unmodified-frozen-Store4-original-dependent-test-execution/v1',
            reused_actual_compile_execution=self.ref(compile_path), results=self.rows[1:])
        self.lists = dict(kind='actual-frozen-Store4-original-test-inventories/v1', records=self.inventories)
        self.sync()
        self.refresh_log_pins()

    def write(self, path, data):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    def write_json(self, path, data):
        self.write(path, (json.dumps(data) + '\n').encode())

    def ref(self, path):
        return dict(path=str(path.relative_to(self.root)), sha256=s._file_hash(path))

    def execution(self, command, stdout, stderr, compilation=False):
        return dict(command=command, cwd=str(self.root), exit_code=0,
            environment={'CARGO_TARGET_DIR':str(self.root / 'target')},
            started_at='2026-01-01T00:00:00+00:00' if compilation else '2026-01-01T00:00:02+00:00',
            finished_at='2026-01-01T00:00:01+00:00' if compilation else '2026-01-01T00:00:03+00:00',
            stdout=str(stdout.relative_to(self.root)), stderr=str(stderr.relative_to(self.root)))

    def compilation(self, targets, index):
        prefix = ['cargo', 'test', '--locked', '--offline', '--manifest-path',
            '.build/binding-journey-alignment-20261109/frozen-store4/source-root/Cargo.toml', '-p', 'app-infra']
        suffix = (['--test', targets[0], '--no-run', '--message-format=json'] if index == 0
            else ['--no-run', '--message-format=json'] + [part for target in targets for part in ('--test', target)])
        stdout, stderr = self.base / ('compile' + str(index) + '.stdout'), self.base / ('compile' + str(index) + '.stderr')
        self.write(stdout, ('\n'.join(json.dumps(self.artifacts[t]) for t in targets)
            + '\n' + json.dumps(dict(reason='build-finished', success=True)) + '\n').encode())
        self.write(stderr, b'')
        return self.execution(prefix + suffix, stdout, stderr, compilation=True)

    def sync(self):
        self.refs = {}
        for name, value in (('schema', self.schema), ('dependent', self.dependent), ('lists', self.lists)):
            path = self.base / (name + '.json')
            self.write_json(path, value)
            self.refs[name] = self.ref(path)

    def refresh_log_pins(self):
        executions = self.compilations + [self.schema['test_execution']] + self.dependent['results'] + self.lists['records']
        paths = {e[k] for e in executions for k in ('stdout', 'stderr')}
        self.log_pins = {p:s._file_hash(self.root / p) for p in paths}

    def inspect(self):
        with patch.object(s, 'OLD_SUITE_REFS', self.refs), patch.object(s, 'OLD4_SOURCE_MANIFEST_SHA256', s._file_hash(self.frozen_path)), patch.object(s, 'OLD_SUITE_LOG_SHA256', self.log_pins):
            s._verify_old_suite(self.root, self.root, self.refs)

    def rewrite_compilation_logs(self):
        self.compilation(s.OLD_TEST_TARGETS[:1], 0)
        self.compilation(s.OLD_TEST_TARGETS[1:], 1)
        self.refresh_log_pins()

    def test_missing_mutable_target_is_valid_and_no_target_is_created(self):
        self.inspect()
        self.assertFalse((self.root / 'target').exists())

    def test_later_compilation_bytes_are_untouched_and_never_read(self):
        origins = {Path(a['executable']) for a in self.artifacts.values()}
        for path in origins:
            self.write(path, b'newer legitimate compile output')
        original = s._file_hash
        visited = []
        def record(path):
            visited.append(Path(path))
            return original(path)
        with patch.object(s, '_file_hash', side_effect=record):
            self.inspect()
        self.assertFalse(origins.intersection(visited))
        self.assertTrue(all(p.read_bytes() == b'newer legitimate compile output' for p in origins))

    def test_changed_immutable_copy_is_rejected(self):
        Path(self.rows[2]['command'][0]).write_bytes(b'wrong binary')
        with self.assertRaises(ValueError): self.inspect()

    def test_copy_symlink_is_rejected(self):
        copied = Path(self.rows[2]['command'][0]); contents = copied.read_bytes()
        other = copied.with_name('other-copy'); other.write_bytes(contents); copied.unlink(); copied.symlink_to(other)
        with self.assertRaises(ValueError): self.inspect()

    def test_moving_copy_into_target_or_another_cohort_is_rejected(self):
        for relative in ('target/copied-old-binary', 'another-cohort/copied-old-binary'):
            with self.subTest(relative=relative):
                original = self.dependent['results'][0]['command'][0]
                other = self.root / relative; self.write(other, Path(original).read_bytes())
                self.dependent['results'][0]['command'][0] = str(other); self.sync()
                with self.assertRaises(ValueError): self.inspect()
                self.dependent['results'][0]['command'][0] = original; self.sync()

    def test_original_lock_or_test_source_change_is_rejected(self):
        for relative in ('Cargo.lock', 'Cargo.toml', 'crates/app-infra/Cargo.toml', 'crates/app-infra/tests/storage_product.rs'):
            with self.subTest(relative=relative):
                path = self.source / relative; original = path.read_bytes(); path.write_bytes(original + b'drift')
                with self.assertRaises(ValueError): self.inspect()
                path.write_bytes(original)

    def test_unpinned_report_edit_is_rejected(self):
        path = self.root / self.refs['schema']['path']; path.write_bytes(path.read_bytes() + b' ')
        with self.assertRaises(ValueError): self.inspect()

    def test_wrong_cargo_package_manifest_target_source_and_profile_are_rejected(self):
        target = s.OLD_TEST_TARGETS[1]; original = copy.deepcopy(self.artifacts[target])
        changes = (
            lambda a: a.update(package_id='path+file:///wrong#0.2.0'),
            lambda a: a.update(manifest_path=str(self.source / 'Cargo.toml')),
            lambda a: a['target'].update(src_path=str(self.source / 'other.rs')),
            lambda a: a['target'].update(test=False),
            lambda a: a['target'].update(crate_types=['lib']),
            lambda a: a['profile'].update(test=False),
            lambda a: a.update(executable=str(self.root / 'target/debug/deps/wrong-0123456789abcdef')),
        )
        for change in changes:
            with self.subTest(change=change):
                self.artifacts[target] = copy.deepcopy(original); change(self.artifacts[target]); self.rewrite_compilation_logs()
                with self.assertRaises(ValueError): self.inspect()
        self.artifacts[target] = original

    def test_missing_duplicate_or_unknown_original_target_is_rejected(self):
        original = copy.deepcopy(self.dependent['results'])
        for rows in (original[:-1], original + [original[0]], original + [dict(original[0], target='unexpected')]):
            self.dependent['results'] = rows; self.sync()
            with self.assertRaises(ValueError): self.inspect()

    def test_missing_failed_or_duplicate_cargo_artifact_is_rejected(self):
        path = self.root / self.compilations[0]['stdout']; original = path.read_bytes()
        for data in (json.dumps(dict(reason='build-finished', success=True)).encode(),
                     original.replace(b'"success": true', b'"success": false'),
                     original + json.dumps(self.artifacts[s.OLD_TEST_TARGETS[0]]).encode() + b'\n'):
            path.write_bytes(data)
            self.refresh_log_pins()
            with self.assertRaises(ValueError): self.inspect()
        path.write_bytes(original)

    def test_unlocked_or_filtered_compile_or_test_is_rejected(self):
        original = copy.deepcopy(self.schema)
        for change in (lambda d:d['compilation']['command'].remove('--locked'),
                       lambda d:d['compilation']['environment'].update(CARGO_TARGET_DIR=str(self.root / 'other-target')),
                       lambda d:d['test_execution']['command'].append('some_test'),
                       lambda d:d['test_execution'].update(exit_code=1)):
            self.schema = copy.deepcopy(original); change(self.schema); self.sync()
            with self.assertRaises(ValueError): self.inspect()

    def test_test_or_list_before_compile_is_rejected(self):
        original = copy.deepcopy(self.schema)
        self.schema['test_execution']['started_at'] = '2025-12-31T23:59:59+00:00'; self.sync()
        with self.assertRaises(ValueError): self.inspect()
        self.schema = original
        self.lists['records'][0]['started_at'] = '2025-12-31T23:59:59+00:00'; self.sync()
        with self.assertRaises(ValueError): self.inspect()

    def test_list_name_drift_skipped_test_or_wrong_total_is_rejected(self):
        for mode in ('name', 'skip', 'total'):
            with self.subTest(mode=mode):
                row = self.rows[0]; raw_path = self.root / row['stdout']; old_raw = raw_path.read_bytes()
                inv = self.lists['records'][0]; old_inv = copy.deepcopy(inv)
                if mode == 'name': inv['test_names'][0] = 'forged_case'
                elif mode == 'skip': raw_path.write_bytes(old_raw.replace(b'0 ignored', b'1 ignored'))
                else:
                    names = inv['test_names'][:-1]; inv['test_names'] = names
                    self.write(self.root / inv['stdout'], ('\n'.join(n + ': test' for n in names) + '\n').encode())
                    self.write(raw_path, ('\n'.join('test ' + n + ' ... ok' for n in names)
                        + '\ntest result: ok. 5 passed; 0 failed; 0 ignored;\n').encode())
                self.sync()
                self.refresh_log_pins()
                with self.assertRaises(ValueError): self.inspect()
                self.lists['records'][0] = old_inv; raw_path.write_bytes(old_raw)
                self.write(self.root / old_inv['stdout'], ('\n'.join(n + ': test' for n in old_inv['test_names']) + '\n').encode())
                self.sync()


    def test_tampered_original_compile_test_and_list_raw_are_rejected(self):
        examples = (self.compilations[0]['stdout'], self.schema['test_execution']['stdout'], self.lists['records'][0]['stdout'])
        for relative in examples:
            with self.subTest(relative=relative):
                path = self.root / relative; original = path.read_bytes()
                path.write_bytes(original + b'\n')
                with self.assertRaisesRegex(ValueError, 'historical raw log changed'):
                    self.inspect()
                path.write_bytes(original)

    def test_missing_or_extra_code_owned_log_identity_is_rejected(self):
        original = dict(self.log_pins)
        self.log_pins.pop(self.compilations[0]['stdout'])
        with self.assertRaisesRegex(ValueError, 'raw log identity is not registered'):
            self.inspect()
        self.log_pins = original
        self.log_pins['extra-not-executed-raw.log'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'raw log set is incomplete'):
            self.inspect()


if __name__ == '__main__':
    unittest.main()
