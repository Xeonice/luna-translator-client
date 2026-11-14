"""Guard fixtures only: these commands/DBs are never product adapters or acceptance."""
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch

import storage_acceptance as acceptance
import storage_product_adapter as product_adapter
from storage_schema import inspect_connection


# A temporary executable named cargo exercises the executor's two actual
# subprocess calls. It does not compile Rust and is NEVER in STORAGE_ADAPTERS.
FIXTURE_CARGO = r'''
import hashlib,json,os,pathlib,shutil,sqlite3,sys,time
out=pathlib.Path(os.environ['FP_STORAGE_OUTPUT'])
mode=pathlib.Path('fixture-mode').read_text()
compile_stage='--no-run' in sys.argv
with (out/'actual-commands.txt').open('a') as file:file.write(('compile' if compile_stage else 'run')+'\n')
if compile_stage:
 if mode=='compile_fail':sys.exit(17)
 if mode=='compile_receipt':pathlib.Path(os.environ['FP_STORAGE_RECEIPT']).write_text('{}')
 print('guard fixture compile command executed')
 sys.exit(0)
if mode=='timeout_tree':
 import signal
 print('partial parent stdout before timeout',flush=True)
 print('partial parent stderr before timeout',file=sys.stderr,flush=True)
 (out/'timeout-parent.pid').write_text(str(os.getpid()))
 grandchild="import os,pathlib,signal,sys,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);p=pathlib.Path(os.environ['FP_STORAGE_OUTPUT']);(p/'timeout-grandchild.pid').write_text(str(os.getpid()));print('partial grandchild stdout',flush=True);time.sleep(120)"
 child="import os,pathlib,signal,subprocess,sys,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);p=pathlib.Path(os.environ['FP_STORAGE_OUTPUT']);(p/'timeout-child.pid').write_text(str(os.getpid()));subprocess.Popen([sys.executable,'-c',"+repr(grandchild)+"]);time.sleep(120)"
 __import__('subprocess').Popen([sys.executable,'-c',child])
 time.sleep(120)
if mode=='run_fail':sys.exit(18)
if mode=='missing_receipt':sys.exit(0)
inventory=json.loads(pathlib.Path(os.environ['FP_STORAGE_INVENTORY']).read_text())
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def database(name,version,wrong=False):
 path=out/name
 connection=sqlite3.connect(path)
 connection.execute('CREATE TABLE guard_data (id TEXT NOT NULL PRIMARY KEY, value INTEGER NOT NULL CHECK(value>=0)) STRICT')
 connection.execute('CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,previous_version INTEGER,migration_sha256 TEXT)')
 connection.execute('CREATE TABLE dictionary_meta(singleton INTEGER PRIMARY KEY,index_schema_version INTEGER,resource_version TEXT,manifest_sha256 TEXT)')
 if mode.startswith('relationship_'):
  connection.execute('CREATE TABLE guard_parent(id TEXT PRIMARY KEY)')
  connection.execute('CREATE TABLE guard_child(id TEXT PRIMARY KEY,parent_id TEXT REFERENCES guard_parent(id))')
  broken=name.startswith('guard-reject-') or mode=='relationship_fresh' and name.startswith('fresh_profile-')
  if broken and mode!='relationship_missing':connection.execute("INSERT INTO guard_child VALUES('child','missing-parent')")
 matrix=json.loads(pathlib.Path('docs/development/storage-acceptance.json').read_text())['supported_upgrade_matrix']
 for entry in matrix['migration_chain']:
  if version is not None and entry['version']<=version:connection.execute('INSERT INTO schema_migrations VALUES(?,?,?)',(entry['version'],entry['previous_version'],entry['migration_sha256']))
 if mode=='relationship_ledger_wrong' and name.startswith('guard-reject-'):connection.execute("UPDATE schema_migrations SET migration_sha256='invalid-ledger'")
 resource=matrix['dictionary_versions']['1']
 connection.execute("INSERT INTO dictionary_meta VALUES(1,?,?,?)",(version,resource['resource_version'],resource['manifest_sha256']))
 if mode=='wrong_version':connection.execute('UPDATE schema_migrations SET previous_version=999 WHERE version=1')
 if mode=='wrong_dictionary_manifest':connection.execute("UPDATE dictionary_meta SET manifest_sha256=?",('b'*64,))
 if wrong:connection.execute('CREATE TABLE unexpected(value TEXT)')
 connection.commit()
 connection.close()
 return {'path':name,'sha256':sha(path)}
routes=[]
for spec in inventory['migration_routes']:
 identifier=spec['id']
 before=None if spec['kind']=='fresh' else database(identifier+'-before.sqlite3',spec['from_version'])
 backup=database(identifier+'-backup.sqlite3',spec['from_version']) if spec['kind']=='paired_recovery' else None
 after_version=spec['from_version'] if spec['expected']=='rejected' else spec['to_version']
 after=database(identifier+'-after.sqlite3',after_version,mode=='wrong_schema' and identifier=='fresh_profile')
 if spec['expected']=='rejected' and mode=='rejected_modified':
  connection=sqlite3.connect(out/after['path']);connection.execute("INSERT INTO guard_data VALUES('changed',1)");connection.commit();connection.close();after['sha256']=sha(out/after['path'])
 if spec['kind']=='paired_recovery' and mode=='recovery_modified':
  connection=sqlite3.connect(out/after['path']);connection.execute("INSERT INTO guard_data VALUES('changed',1)");connection.commit();connection.close();after['sha256']=sha(out/after['path'])
 routes.append(dict(id=identifier,database=spec['database'],source_schema=spec['source_schema'],target_schema=spec['target_schema'],
  source_schema_sha256=sha(pathlib.Path(spec['source_schema'])) if spec['source_schema'] else None,
  target_schema_sha256=sha(pathlib.Path(spec['target_schema'])),outcome=spec['expected'],error_code=spec['expected_error'],before=before,after=after,backup=backup))
checks=[dict(id=identifier,status='passed',assertions=1,skipped=0) for identifier in inventory['checks']]
if mode=='missing_check':checks.pop()
if mode=='skip':checks[0]['skipped']=1
if mode=='zero_assertion':checks[0]['assertions']=0
if mode=='duplicate_check':checks.append(checks[0])
for row in checks:
 if mode=='fake_no_assertions' or mode=='missing_route_assertion' and row['id'].startswith('MIGRATION-ROUTE:'):continue
 print('STORAGE_ASSERTION '+row['id']+' '+str(row['assertions'])+' PASS')
print('test result: ok. 1 passed; 0 failed; '+('1' if mode=='skip' else '0')+' ignored; 0 measured; 0 filtered out; finished in 0.01s')
dbs=[]
for identity in ('profile','dictionary'):
 route=next(row for row in routes if row['id']=='fresh_'+identity)
 dbs.append(dict(database=identity,path=route['after']['path'],sha256=route['after']['sha256'],open_mode='read_only_snapshot',origin='registered_production_migrations'))
if mode=='database_symlink':
 original=out/dbs[0]['path'];link=out/'symlink.sqlite3';link.symlink_to(original);dbs[0]['path']='symlink.sqlite3'
if mode=='reuse_route':routes[-1]['after']=routes[0]['after']
if mode.startswith('rejected_sidecar_'):
 route=next(row for row in routes if row['id']=='guard-reject')
 _,_,suffix,target_kind=mode.split('_')
 target=out/('empty-target' if target_kind=='empty' else 'missing-target')
 if target_kind=='empty':target.write_bytes(b'')
 pathlib.Path(str(out/route['before']['path'])+'-'+suffix).symlink_to(target)
if mode=='missing_route':routes.pop()
if mode=='forged_db_hash':dbs[0]['sha256']='0'*64
if mode=='mutate_inventory':pathlib.Path(os.environ['FP_STORAGE_INVENTORY']).write_text('{}')
receipt=dict(schema_version=1,kind='product_storage_runtime_receipt',run_id=os.environ['FP_STORAGE_RUN_ID'],status='passed',
 source_tree_sha256=os.environ['FP_STORAGE_SOURCE_SHA256'],artifact_sha256=os.environ['FP_STORAGE_ARTIFACT_SHA256'] or None,
 adapter_version='guard-only-v1',sqlite_version=sqlite3.sqlite_version,sqlite_source_id='guard-source',
 started_at_ms=int(os.environ['FP_STORAGE_STARTED_AT_MS']),finished_at_ms=time.time_ns()//1000000,
 checks=checks,migration_routes=routes,databases=dbs)
if mode=='stale_run':receipt['run_id']='previous-run'
if mode=='stale_time':receipt['started_at_ms']=1
if mode=='wrong_source':receipt['source_tree_sha256']='0'*64
if mode=='wrong_artifact':receipt['artifact_sha256']='0'*64
if mode=='wrong_engine':receipt['sqlite_version']='different-engine'
pathlib.Path(os.environ['FP_STORAGE_RECEIPT']).write_text(json.dumps(receipt))
'''


class StorageAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.registry = json.loads((acceptance.ROOT / acceptance.REGISTRY_PATH).read_text())

    def tearDown(self):
        self.temp.cleanup()

    def rejected_registry(self, document, text):
        errors = acceptance.validate_registry(acceptance.ROOT, document)
        self.assertTrue(any(text in error for error in errors), errors)

    def test_current_registry_is_static_and_preserves_not_run_product_inventory(self):
        self.assertEqual(acceptance.validate_registry(acceptance.ROOT), [])
        plan = json.loads((acceptance.ROOT / self.registry['plan']).read_text())
        self.assertEqual(len(plan['cases']), 128)
        self.assertTrue(all(case['status'] == 'not_run' for case in plan['cases']))
        self.assertEqual(sum(len(case['variant_ids']) for case in plan['cases']), 153)
        # ACD-007 adds the explicit schema3 target; both historical targets and
        # their unfiltered command/source assertions remain mandatory.
        self.assertEqual(set(acceptance.STORAGE_ADAPTERS), {"rust-storage-complete-v1", "rust-storage-complete-v2", "rust-storage-complete-v3"})
        for name, adapter in acceptance.STORAGE_ADAPTERS.items():
            self.assertEqual(acceptance._adapter_errors(acceptance.ROOT, name, adapter), [])
            self.assertIn("--lib", adapter["compile_command"])
            self.assertIn("--lib", adapter["run_command"])
            self.assertIn("--test-threads=1", adapter["run_command"])
            self.assertNotIn("guard", adapter["adapter_version"])

    def test_missing_storage_subcheck_is_rejected(self):
        self.registry['checks'].pop()
        self.rejected_registry(self.registry, 'missing/duplicate/unknown')

    def test_unknown_execution_variant_is_rejected(self):
        self.registry['checks'][0]['execution_ids'] = ['TP-01@NATIVE']
        self.rejected_registry(self.registry, 'unknown or missing case@variant')

    def test_removing_actual_storage_case_links_is_rejected(self):
        for row in self.registry['checks']:
            row['execution_ids'] = ['TP-01@INFRA']
        self.rejected_registry(self.registry, 'required execution links are missing')

    def test_product_execution_claim_and_fabricated_pending_versions_are_rejected(self):
        self.registry['product_tests_executed'] = True
        # The live engineering matrix can be reviewed/frozen; this negative
        # fixture must explicitly exercise an unreviewed pending matrix.
        self.registry['supported_upgrade_matrix']['status'] = 'pending_review'
        self.registry['supported_upgrade_matrix']['routes'] = [{'id': 'unreviewed-old-version'}]
        self.rejected_registry(self.registry, 'execution claim')
        self.rejected_registry(self.registry, 'cannot invent supported versions')

    def test_all_boundary_probes_and_each_field_mapping_are_derived(self):
        ids = acceptance.boundary_check_ids(acceptance.ROOT)
        boundary = json.loads((acceptance.ROOT / acceptance.BOUNDARY_PATH).read_text())
        self.assertEqual(ids, {row['id'] for row in boundary['probes']} |
                         {'RUST-MAP:' + row['id'] for row in boundary['fields']})

    def test_design_tooling_is_allowed_but_storage_product_and_release_are_blocked(self):
        self.assertEqual(acceptance.storage_requirement_gaps(acceptance.ROOT, {'kind': 'tooling'}, ['docs/architecture/database/profile.sql'], 'development'), [])
        for metadata, paths, profile in [({'kind': 'product'}, ['crates/app-infra/src/storage/database/mod.rs'], 'development'),
                                         ({'kind': 'product', 'requirements': [{'module_ids': ['M10']}]}, [], 'development'),
                                         ({'kind': 'tooling'}, [], 'release')]:
            with self.subTest(paths=paths, profile=profile):
                self.assertTrue(acceptance.storage_requirement_gaps(acceptance.ROOT, metadata, paths, profile))

    def test_runtime_without_real_adapter_returns_blocked_and_no_execution(self):
        receipt = acceptance.run_product_storage(acceptance.ROOT, ['guard-fixture'], '.build/storage-acceptance')
        self.assertEqual(receipt['status'], 'blocked')
        self.assertFalse(receipt['product_storage_tests_executed'])
        self.assertTrue(any('not implemented/registered' in error for error in receipt['errors']))

    def test_real_adapter_cannot_turn_into_filtered_or_list_only_execution(self):
        original = acceptance.STORAGE_ADAPTERS['rust-storage-complete-v1']
        for forbidden in ('--list', '--ignored', '--include-ignored', '--skip', '--exact'):
            adapter = copy.deepcopy(original)
            adapter['run_command'].append(forbidden)
            self.assertTrue(any('cannot substitute' in error for error in
                                acceptance._adapter_errors(acceptance.ROOT, 'rust-storage-complete-v1', adapter)))

    def test_production_argv_is_closed_against_position_filters_equals_flags_and_missing_lib(self):
        original = acceptance.STORAGE_ADAPTERS['rust-storage-complete-v1']
        mutations = []
        for stage in ('compile_command', 'run_command'):
            for value in ('configuration_lifecycle', '--skip=configuration_lifecycle',
                          '--exact=one_case', '--test-threads=2', '--include-ignored=true'):
                for position in (1, -1):
                    candidate = copy.deepcopy(original)
                    if position == 1:
                        candidate[stage].insert(1, value)
                    else:
                        candidate[stage].append(value)
                    mutations.append(candidate)
            candidate = copy.deepcopy(original)
            candidate[stage].remove('--lib')
            mutations.append(candidate)
        candidate = copy.deepcopy(original)
        for stage in ('compile_command', 'run_command'):
            candidate[stage].remove('--lib')
        mutations.append(candidate)
        candidate = copy.deepcopy(original)
        candidate['run_command'][-1] = '--test-threads=2'
        mutations.append(candidate)
        candidate = copy.deepcopy(original)
        candidate['run_command'].remove('--')
        mutations.append(candidate)
        for index, adapter in enumerate(mutations):
            with self.subTest(mutation=index):
                self.assertTrue(any('closed production argv' in error for error in
                                    acceptance._adapter_errors(acceptance.ROOT, 'rust-storage-complete-v1', adapter)))

    def test_actual_runtime_budget_is_code_registered_and_always_bounded(self):
        original = acceptance.STORAGE_ADAPTERS['rust-storage-complete-v1']
        self.assertEqual(original['compile_timeout_seconds'], 600)
        self.assertEqual(original['run_timeout_seconds'], 1800)
        for value in (0, -1, True, False, 1801, 99999999, None, '1800'):
            adapter = copy.deepcopy(original)
            adapter['run_timeout_seconds'] = value
            self.assertTrue(any('bounded timeout' in error for error in
                                acceptance._adapter_errors(acceptance.ROOT, 'rust-storage-complete-v1', adapter)))

    def test_real_adapter_cannot_omit_direct_indirect_or_library_test_sources(self):
        original = acceptance.STORAGE_ADAPTERS['rust-storage-complete-v1']
        required = acceptance._test_source_closure(acceptance.ROOT, original['rust_test_target'], True)
        self.assertEqual(acceptance._adapter_errors(acceptance.ROOT, 'rust-storage-complete-v1', original), [])
        self.assertTrue(required <= set(original['test_sources']))
        self.assertIn('crates/app-infra/tests/configuration_lifecycle.rs', required)
        self.assertIn('crates/app-infra/tests/storage_acceptance_support/evidence.rs', required)
        self.assertIn('crates/app-infra/src/storage/database_fault_tests.rs', required)
        self.assertIn('crates/app-infra/src/language/tests.rs', required)
        reviewed_increment = {
            'crates/app-infra/src/offline_translation/inspection.rs',
            'crates/app-infra/src/providers/google_execution.rs',
            'crates/app-infra/src/providers/typed_http.rs',
            'crates/app-infra/src/providers/typed_http_tests.rs',
            'crates/app-infra/src/storage/queries/google_authorization.rs',
            'crates/app-infra/tests/storage_acceptance_support/frozen_hosts.rs',
        }
        self.assertTrue(reviewed_increment <= required)
        for missing in required:
            adapter = copy.deepcopy(original)
            adapter['test_sources'].remove(missing)
            with self.subTest(source=missing):
                self.assertEqual(acceptance._adapter_errors(acceptance.ROOT, 'rust-storage-complete-v1', adapter), [
                    'rust-storage-complete-v1: reviewed test_sources omits compiled test import closure: ' + missing,
                ])

    def test_real_adapter_rejects_unknown_and_out_of_scope_test_source_paths(self):
        original = acceptance.STORAGE_ADAPTERS['rust-storage-complete-v1']
        for source, reason in (
            ('crates/app-infra/tests/__missing_storage_guard__.rs', 'missing regular file'),
            ('crates/app-core/src/lib.rs', 'reviewed app-infra Rust source'),
            ('crates/app-infra/Cargo.toml', 'reviewed app-infra Rust source'),
            ('../Cargo.lock', 'unsafe path'),
            (str(acceptance.ROOT / 'crates/app-infra/src/lib.rs'), 'unsafe path'),
        ):
            with self.subTest(source=source):
                adapter = copy.deepcopy(original)
                adapter['test_sources'].append(source)
                self.assertTrue(any(reason in error for error in
                                    acceptance._adapter_errors(acceptance.ROOT, 'rust-storage-complete-v1', adapter)))

    @unittest.skipUnless(os.name == 'posix', 'POSIX process-group cleanup proof; Windows uses taskkill /T')
    def test_timeout_kills_owned_parent_child_grandchild_and_persists_partial_failure(self):
        receipt = self.run_fixture('timeout_tree', mutate_adapter=lambda adapter: adapter.update(run_timeout_seconds=1))
        self.assertEqual(receipt['status'], 'failed')
        self.assertTrue(receipt['product_storage_tests_executed'])
        run_root = self.root / receipt['output']
        persisted = json.loads((run_root / 'execution.json').read_text())
        self.assertEqual(persisted, receipt)
        stage = persisted['adapters'][0]['commands'][-1]
        self.assertEqual(stage['stage'], 'run')
        self.assertTrue(stage['timed_out'])
        self.assertEqual(stage['timeout_seconds'], 1)
        self.assertEqual(stage['status'], 'failed')
        self.assertTrue(stage['termination']['verified'])
        self.assertIn('SIGTERM', stage['termination']['delivered_signals'])
        self.assertIn('SIGKILL', stage['termination']['delivered_signals'])
        self.assertIn('partial parent stdout', (self.root / stage['stdout']).read_text())
        self.assertIn('partial parent stderr', (self.root / stage['stderr']).read_text())
        self.assertIn('partial grandchild stdout', (self.root / stage['log']).read_text())
        output = run_root / 'guard-fixture'
        for member in ('parent', 'child', 'grandchild'):
            pid = int((output / ('timeout-' + member + '.pid')).read_text())
            process = subprocess.run(['ps', '-p', str(pid), '-o', 'stat='], capture_output=True, text=True)
            self.assertTrue(process.returncode != 0 or process.stdout.strip().startswith('Z'),
                            f'owned {member} still executes after timeout: {pid} {process.stdout}')
        self.assertTrue((output / 'run.process.json').is_file())
        self.assertFalse((output / 'receipt.json').exists())

    def test_metadata_cannot_register_guard_fixture_as_a_production_command(self):
        metadata = {'kind': 'product', 'product_storage_checks': ['guard-fixture'],
                    'compile_command': ['cargo', 'test', '--no-run'],
                    'run_command': ['cargo', 'test']}
        errors = acceptance.storage_requirement_gaps(acceptance.ROOT, metadata, [], 'release')
        self.assertTrue(any('not implemented/registered' in error for error in errors))
        self.assertNotIn('guard-fixture', acceptance.STORAGE_ADAPTERS)

    def test_actual_case_to_source_map_remains_reviewable(self):
        mapped = product_adapter.source_map()
        self.assertEqual(mapped['status'], 'passed', mapped['errors'])
        self.assertEqual({row['case'] for row in mapped['cases']}, set(product_adapter.CASE_SOURCES) | set(product_adapter.V2_CASE_SOURCES))

    def test_product_proof_audit_rejects_missing_scenarios_and_inventory_only_fields(self):
        # Isolated negative guard data, never registered or issued by the real
        # Cargo target: a list of expected names must not become producer proof.
        directory = self.root / 'proof-fixture'
        directory.mkdir()
        (directory / 'asserted-cases').mkdir()
        fields = [dict(id='RUST-MAP:fixture.field_' + str(index), assertions=1,
                       status='passed', skipped=0) for index in range(431)]
        (directory / 'receipt.json').write_text(json.dumps(dict(
            kind='product_storage_runtime_receipt', status='passed',
            run_id='guard-only', source_tree_sha256='fixture-only', checks=fields)))
        receipt = product_adapter.audit_execution(directory)
        self.assertEqual(receipt['status'], 'failed')
        self.assertTrue(any('required real scenario missing' in value for value in receipt['errors']))
        self.assertTrue(any('lacks its concrete producer invariant' in value for value in receipt['errors']))

    def test_cli_missing_adapter_is_nonzero(self):
        process = subprocess.run([sys.executable, str(acceptance.ROOT / 'scripts/quality/storage_acceptance.py'), '--runtime', '--adapter', 'missing'], capture_output=True, text=True)
        self.assertNotEqual(process.returncode, 0)
        self.assertEqual(json.loads(process.stdout)['status'], 'blocked')

    def fixture(self, mode='pass'):
        """Build isolated guard-only sources, approved-schema stand-in and commands."""
        registry_path = self.root / acceptance.REGISTRY_PATH
        registry_path.parent.mkdir(parents=True, exist_ok=True)
        contract = self.root / acceptance.SCHEMA_PATH
        contract.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(':memory:')
        connection.execute('CREATE TABLE guard_data (id TEXT NOT NULL PRIMARY KEY, value INTEGER NOT NULL CHECK(value>=0)) STRICT')
        connection.execute('CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,previous_version INTEGER,migration_sha256 TEXT)')
        connection.execute('CREATE TABLE dictionary_meta(singleton INTEGER PRIMARY KEY,index_schema_version INTEGER,resource_version TEXT,manifest_sha256 TEXT)')
        if mode.startswith('relationship_'):
            connection.execute('CREATE TABLE guard_parent(id TEXT PRIMARY KEY)')
            connection.execute('CREATE TABLE guard_child(id TEXT PRIMARY KEY,parent_id TEXT REFERENCES guard_parent(id))')
        shape = inspect_connection(connection)
        connection.execute('ALTER TABLE guard_data ADD COLUMN future_column TEXT')
        future_shape = inspect_connection(connection)
        connection.close()
        contract.write_text(json.dumps({'schema_version': 1, 'kind': 'frozen_storage_schema_contract', 'databases': {'profile': shape, 'dictionary': shape}}))
        future_contract = contract.with_name('guard-future-schema.json')
        future_contract.write_text(json.dumps({'schema_version': 1, 'kind': 'frozen_storage_schema_contract', 'databases': {'profile': future_shape, 'dictionary': future_shape}}))
        approval = self.root / 'docs/development/guard-pre14-approval.json'
        approval.write_text('{"scope":"guard fixture only"}')
        routes = []
        for identifier, kind, expected in [('guard-upgrade', 'upgrade', 'accepted'), ('guard-reject', 'downgrade_rejection', 'rejected'), ('guard-recover', 'paired_recovery', 'accepted')]:
            routes.append(dict(id=identifier, kind=kind, database='profile', from_version=1,
                               to_version=3 if kind == 'downgrade_rejection' else 1 if kind == 'paired_recovery' else 2,
                               source_state='supported', expected_error='unsupported_schema' if expected == 'rejected' else None,
                               source_schema=acceptance.SCHEMA_PATH,
                               target_schema=str(future_contract.relative_to(self.root)) if kind == 'downgrade_rejection' else acceptance.SCHEMA_PATH,
                               expected=expected))
        sources = ['crates/app-infra/src/storage/migrations/guard.rs', 'crates/app-infra/tests/storage_acceptance_guard.rs']
        for relative in sources:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('// Guard fixture metadata only; not production Rust or a migration.\n')
        source_hash = hashlib.sha256((self.root / sources[0]).read_bytes()).hexdigest()
        sql = self.root / 'crates/app-infra/src/storage/migrations/guard.sql'
        sql.write_text('-- Guard fixture SQL content, not a production migration.\n')
        sql_hash = hashlib.sha256(sql.read_bytes()).hexdigest()
        manifest = self.root / 'resources/language/guard-provenance.json'
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(json.dumps({'index_schema_version':1,'resource_version':'guard-dictionary'}))
        manifest_hash = hashlib.sha256(manifest.read_bytes()).hexdigest()
        self.registry['supported_upgrade_matrix'] = {
            'status': 'frozen', 'approval_ref': str(approval.relative_to(self.root)), 'routes': routes,
            'fresh_versions': {'profile': 2, 'dictionary': 1},
            'migration_chain': [{'version': version, 'previous_version': version - 1, 'source_path': sources[0], 'source_sha256': source_hash,
                                 'sql_sources': [{'path': str(sql.relative_to(self.root)), 'sha256': sql_hash}],
                                 'migration_sha256': sql_hash} for version in (1, 2)],
            'dictionary_versions': {'1': {'resource_version': 'guard-dictionary', 'manifest_path':str(manifest.relative_to(self.root)), 'manifest_sha256': manifest_hash}}}
        registry_path.write_text(json.dumps(self.registry))
        executable = self.root / 'tools/cargo'
        executable.parent.mkdir()
        executable.write_text('#!' + sys.executable + '\n' + textwrap.dedent(FIXTURE_CARGO).lstrip())
        executable.chmod(0o755)
        (self.root / 'fixture-mode').write_text(mode)
        artifact = self.root / '.build/fixture-package'
        artifact.parent.mkdir(exist_ok=True)
        artifact.write_bytes(b'guard-only package fixture')
        command = [str(executable), 'test', '--package', 'app-infra', '--test', 'storage_acceptance_guard', '--locked']
        adapter = dict(compile_command=command + ['--no-run'], run_command=command + ['--', '--nocapture', '--test-threads=1'],
                       rust_test_target='storage_acceptance_guard', adapter_version='guard-only-v1', sqlite_version=sqlite3.sqlite_version,
                       sqlite_source_id='guard-source', migration_sources=[sources[0]], test_sources=[sources[1]])
        return adapter, artifact

    def run_fixture(self, mode='pass', mutate_adapter=None, mutate_registry=None):
        adapter, artifact = self.fixture(mode)
        if mutate_adapter:
            mutate_adapter(adapter)
        if mutate_registry:
            mutate_registry(self.registry)
            (self.root / acceptance.REGISTRY_PATH).write_text(json.dumps(self.registry))
        # Only this test scope uses a stand-in. Formal code registry stays empty.
        with patch.dict(acceptance.STORAGE_ADAPTERS, {'guard-fixture': adapter}), \
             patch.object(acceptance, 'inspect_package', return_value={'kind': 'guard_only_artifact_stub_not_product_acceptance'}), \
             patch.object(acceptance, 'validate_registry', return_value=[]), \
             patch.object(acceptance, '_design_checks', return_value=[]), \
             patch.object(acceptance, 'boundary_check_ids', return_value={'GUARD-BOUNDARY:one', 'RUST-MAP:guard_data.value'}):
            return acceptance.run_product_storage(self.root, ['guard-fixture'], '.build/storage-guard', artifact)

    def validate_guard_matrix(self, mutate=None):
        """Only validate an isolated fixture; never freeze the product registry."""
        self.fixture()
        plan = self.root / self.registry['plan']
        plan.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(acceptance.ROOT / self.registry['plan'], plan)
        if mutate:
            mutate(self.registry['supported_upgrade_matrix'])
        with patch.object(acceptance, '_design_checks', return_value=[]), \
             patch.object(acceptance, 'boundary_check_ids', return_value={'GUARD-BOUNDARY:one'}):
            return acceptance.validate_registry(self.root, self.registry)

    def test_reviewed_matrix_separately_binds_rust_source_and_sql_content(self):
        self.assertEqual(self.validate_guard_matrix(), [])

    def test_old_ambiguous_migration_hash_contract_is_rejected(self):
        def mutate(matrix):
            first = matrix['migration_chain'][0]
            first['sha256'] = first.pop('source_sha256')
        errors = self.validate_guard_matrix(mutate)
        self.assertTrue(any('increasing and linked' in error for error in errors), errors)

    def test_rust_file_hash_cannot_stand_in_for_actual_sql_ledger_hash(self):
        errors = self.validate_guard_matrix(lambda matrix: matrix['migration_chain'][0].update(
            migration_sha256=matrix['migration_chain'][0]['source_sha256']))
        self.assertTrue(any('ordered SQL content, not Rust source' in error for error in errors), errors)

    def test_sql_content_hash_cannot_stand_in_for_rust_source_hash(self):
        errors = self.validate_guard_matrix(lambda matrix: matrix['migration_chain'][0].update(
            source_sha256=matrix['migration_chain'][0]['migration_sha256']))
        self.assertTrue(any('actual production Rust' in error for error in errors), errors)

    def test_sql_source_tampering_is_rejected_independently_of_ledger_hash(self):
        def mutate(matrix):
            matrix['migration_chain'][0]['sql_sources'][0]['sha256'] = '0' * 64
        errors = self.validate_guard_matrix(mutate)
        self.assertTrue(any('SQL source/hash differs' in error for error in errors), errors)

    def test_dictionary_provenance_is_bound_to_its_actual_file_and_index_version(self):
        errors = self.validate_guard_matrix(lambda matrix: matrix['dictionary_versions']['1'].update(
            manifest_sha256='0' * 64))
        self.assertTrue(any('path/hash differs' in error for error in errors), errors)

    def test_dictionary_outer_resource_manifest_cannot_replace_index_provenance(self):
        def mutate(matrix):
            resource = matrix['dictionary_versions']['1']
            path = self.root / resource['manifest_path']
            path.write_text(json.dumps({'resource_version':'outer-language-bundle','files':[]}))
            resource['manifest_sha256'] = acceptance._hash(path)
        errors = self.validate_guard_matrix(mutate)
        self.assertTrue(any('provenance version differs' in error for error in errors), errors)

    def test_unversioned_rejection_requires_null_source_version_and_unknown_shape(self):
        def mutate(matrix):
            route = matrix['routes'][1]
            route.update(kind='compatibility_rejection', source_state='unversioned', from_version=None,
                         source_schema=None, to_version=2, expected_error='unsupported_schema')
        self.assertEqual(self.validate_guard_matrix(mutate), [])

    def test_unversioned_input_cannot_be_relabelled_as_positive_old_version(self):
        def mutate(matrix):
            route = matrix['routes'][1]
            route.update(kind='compatibility_rejection', source_state='unversioned', from_version=1,
                         source_schema=None, expected_error='unsupported_schema')
        errors = self.validate_guard_matrix(mutate)
        self.assertTrue(any('null for unversioned/corrupt' in error for error in errors), errors)

    def test_unknown_source_cannot_claim_frozen_approved_shape(self):
        def mutate(matrix):
            matrix['routes'][1].update(kind='compatibility_rejection', source_state='unknown_version')
        errors = self.validate_guard_matrix(mutate)
        self.assertTrue(any('may not claim an approved schema' in error for error in errors), errors)

    def test_same_version_recovery_cannot_claim_nonexistent_upgrade(self):
        errors = self.validate_guard_matrix(lambda matrix: matrix['routes'][2].update(to_version=2))
        self.assertTrue(any('cannot imply a nonexistent migration' in error for error in errors), errors)

    def test_rejection_requires_actual_production_error_code(self):
        errors = self.validate_guard_matrix(lambda matrix: matrix['routes'][1].update(expected_error=None))
        self.assertTrue(any('exact production error code' in error for error in errors), errors)

    def test_rejected_source_state_cannot_use_an_unrelated_error_code(self):
        def mutate(matrix):
            matrix['routes'][1].update(kind='compatibility_rejection', source_state='unversioned',
                from_version=None, source_schema=None, expected_error='migration_mismatch')
        errors = self.validate_guard_matrix(mutate)
        self.assertTrue(any('incompatible with its declared source state' in error for error in errors), errors)

    def test_unversioned_and_corrupt_are_distinct_real_sqlite_inputs(self):
        self.fixture()
        matrix = self.registry['supported_upgrade_matrix']
        spec = dict(database='profile', source_state='unversioned', from_version=None)
        path = self.root / 'actual-unversioned.sqlite3'
        connection = sqlite3.connect(path)
        connection.execute('CREATE TABLE old_user_data(original TEXT)')
        connection.commit(); connection.close()
        original_hash = acceptance._hash(path)
        self.assertEqual(acceptance._rejected_source_errors(self.root, path, spec, matrix), [])
        self.assertEqual(acceptance._hash(path), original_hash)
        corrupt = self.root / 'actual-corrupt.sqlite3'
        corrupt.write_bytes(b'not a SQLite file')
        self.assertTrue(acceptance._rejected_source_errors(self.root, corrupt, spec, matrix))
        self.assertEqual(acceptance._rejected_source_errors(self.root, corrupt, spec | {'source_state':'corrupt'}, matrix), [])
        self.assertTrue(acceptance._rejected_source_errors(self.root, path, spec | {'source_state':'corrupt'}, matrix))

    def test_version_one_is_not_an_unknown_version_fixture(self):
        self.fixture()
        matrix = self.registry['supported_upgrade_matrix']
        path = self.root / 'supported.sqlite3'
        connection = sqlite3.connect(path)
        connection.execute('CREATE TABLE schema_migrations(version INTEGER,previous_version INTEGER,migration_sha256 TEXT)')
        connection.execute('INSERT INTO schema_migrations VALUES(1,0,?)', (matrix['migration_chain'][0]['migration_sha256'],))
        connection.commit(); connection.close()
        spec = dict(database='profile', source_state='unknown_version', from_version=1)
        self.assertTrue(any('supported version' in error for error in acceptance._rejected_source_errors(self.root,path,spec,matrix)))

    def test_null_schema_rejection_route_cannot_bypass_zero_length_or_dangling_sidecar_links(self):
        def unversioned(registry):
            registry['supported_upgrade_matrix']['routes'][1].update(kind='compatibility_rejection',
                source_state='unversioned', from_version=None, source_schema=None)
        # Each variant uses its own fixture root so links cannot interfere with
        # another variant or turn a prior rejection into the next test's cause.
        for suffix in ('wal','shm'):
            for target in ('empty','dangling'):
                with self.subTest(sidecar=suffix,target=target), tempfile.TemporaryDirectory() as folder:
                    previous = self.root
                    self.root = Path(folder).resolve()
                    try:
                        result = self.run_fixture(f'rejected_sidecar_{suffix}_{target}',mutate_registry=unversioned)
                        self.assertNotEqual(result['status'],'passed',result)
                        self.assertTrue(any('symbolic-link sidecars' in error for error in result['errors']),result['errors'])
                    finally:
                        self.root = previous

    def test_all_immutable_version_and_rejection_readers_share_sealed_precondition(self):
        self.fixture()
        matrix=self.registry['supported_upgrade_matrix']
        for state in ('unversioned','unknown_version','corrupt'):
            for suffix in ('wal','shm'):
                for target_kind in ('empty','dangling'):
                    with self.subTest(state=state,suffix=suffix,target=target_kind):
                        path=self.root/f'{state}-{suffix}-{target_kind}.sqlite3'
                        if state=='corrupt':path.write_bytes(b'not a sqlite database')
                        else:
                            connection=sqlite3.connect(path)
                            if state=='unversioned':connection.execute('CREATE TABLE original_user_data(text TEXT)')
                            else:
                                connection.execute('CREATE TABLE schema_migrations(version INTEGER)')
                                connection.execute('INSERT INTO schema_migrations VALUES(99)')
                            connection.commit();connection.close()
                        target=self.root/f'{path.name}-target'
                        if target_kind=='empty':target.write_bytes(b'')
                        sidecar=Path(str(path)+'-'+suffix);sidecar.symlink_to(target)
                        spec=dict(database='profile',source_state=state,from_version=99 if state=='unknown_version' else None)
                        with patch.object(acceptance.sqlite3,'connect',side_effect=AssertionError('immutable reader must not open an unsafe snapshot')):
                            errors=acceptance._rejected_source_errors(self.root,path,spec,matrix)
                            self.assertTrue(any('symbolic-link sidecars' in error for error in errors),errors)
                            errors=acceptance._version_errors(path,'profile',1,matrix)
                            self.assertTrue(any('symbolic-link sidecars' in error for error in errors),errors)

    def assert_fixture_failed(self, mode, text=None):
        result = self.run_fixture(mode)
        self.assertNotEqual(result['status'], 'passed', result)
        if text:
            self.assertTrue(any(text in error for error in result['errors']), result['errors'])

    def test_guard_fixture_executes_both_commands_and_independently_compares_real_sqlite(self):
        result = self.run_fixture()
        self.assertEqual(result['status'], 'passed', result)
        logs = list((self.root / result['output']).rglob('actual-commands.txt'))
        self.assertEqual(len(logs), 1)
        self.assertEqual(logs[0].read_text().splitlines(), ['compile', 'run'])
        self.assertEqual([command['stage'] for command in result['adapters'][0]['commands']], ['compile', 'run'])
        self.assertNotIn('guard-fixture', acceptance.STORAGE_ADAPTERS)
        self.assertEqual(set(acceptance.STORAGE_ADAPTERS), {'rust-storage-complete-v1', 'rust-storage-complete-v2', 'rust-storage-complete-v3'})

    def test_compile_failure_stops_runtime(self):
        result = self.run_fixture('compile_fail')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(len(result['adapters'][0]['commands']), 1)
        self.assertFalse(result['product_storage_tests_executed'])

    def test_compile_cannot_fabricate_runtime_receipt(self):
        self.assert_fixture_failed('compile_receipt', 'compile stage must not fabricate')

    def test_actual_execution_failure_and_missing_receipt_fail(self):
        self.assert_fixture_failed('run_fail', 'run command failed')

    def test_zero_or_omitted_runtime_assertions_fail(self):
        self.assert_fixture_failed('zero_assertion', 'did not actually pass')

    def test_missing_runtime_subcheck_fails(self):
        self.assert_fixture_failed('missing_check', 'missing/duplicate/unknown')

    def test_duplicate_runtime_subcheck_fails(self):
        self.assert_fixture_failed('duplicate_check', 'missing/duplicate/unknown')

    def test_ignored_test_or_skipped_assertion_fails(self):
        self.assert_fixture_failed('skip', 'zero ignored/failed')

    def test_passed_json_without_real_assertion_log_fails(self):
        self.assert_fixture_failed('fake_no_assertions', 'actual runner assertion marker')

    def test_old_receipt_cannot_be_replayed(self):
        self.assert_fixture_failed('stale_run', 'run_id')

    def test_old_receipt_timestamp_fails(self):
        self.assert_fixture_failed('stale_time', 'fresh command execution interval')

    def test_wrong_source_identity_fails(self):
        self.assert_fixture_failed('wrong_source', 'source_tree_sha256')

    def test_wrong_final_package_fails(self):
        self.assert_fixture_failed('wrong_artifact', 'artifact_sha256')

    def test_wrong_sqlite_engine_identity_fails(self):
        self.assert_fixture_failed('wrong_engine', 'sqlite_version')

    def test_actual_migration_version_and_previous_chain_cannot_be_receipt_only(self):
        self.assert_fixture_failed('wrong_version', 'migration version/previous/source hash chain')

    def test_actual_dictionary_manifest_must_match_frozen_resource(self):
        self.assert_fixture_failed('wrong_dictionary_manifest', 'dictionary index/resource/manifest version')

    def test_rejected_cross_version_route_keeps_source_schema_and_bytes(self):
        result = self.run_fixture()
        self.assertEqual(result['status'], 'passed', result)
        self.assertNotEqual(self.registry['supported_upgrade_matrix']['routes'][1]['source_schema'],
                            self.registry['supported_upgrade_matrix']['routes'][1]['target_schema'])

    def relationship_rejection_fixture(self, mode):
        def specify(document):
            route = next(row for row in document['supported_upgrade_matrix']['routes'] if row['id'] == 'guard-reject')
            route.update(kind='compatibility_rejection', to_version=1,
                         source_state='relationship_corrupt', expected_error='database_relationship_corrupt',
                         target_schema=acceptance.SCHEMA_PATH)
            if mode == 'relationship_wrong_error':
                route['expected_error'] = 'unsupported_schema'
        return self.run_fixture(mode, mutate_registry=specify)

    def test_relationship_rejection_proves_invalid_data_without_accepting_it_as_a_valid_candidate(self):
        result = self.relationship_rejection_fixture('relationship_proof')
        self.assertEqual(result['status'], 'passed', result['errors'])
        output = self.root / result['output'] / 'guard-fixture'
        receipt = json.loads((output / 'receipt.json').read_text())
        route = next(row for row in receipt['migration_routes'] if row['id'] == 'guard-reject')
        self.assertEqual(route['before']['sha256'], route['after']['sha256'])
        from storage_schema import compare_schema
        for position in ('before', 'after'):
            self.assertTrue(any('foreign key violations' in error for error in
                                compare_schema(self.root, output / route[position]['path'], 'profile')))

    def test_relationship_rejection_requires_actual_fk_failure_valid_ledger_and_frozen_error(self):
        for mode, text in [('relationship_missing', 'no actual foreign key violation'),
                           ('relationship_ledger_wrong', 'invalid migration ledger'),
                           ('relationship_wrong_error', 'foreign key violations'),
                           ('relationship_fresh', 'foreign key violations')]:
            with self.subTest(mode=mode):
                # Each stand-in run needs its own isolated fixture root.
                folder = tempfile.TemporaryDirectory(); self.addCleanup(folder.cleanup)
                self.root = Path(folder.name).resolve()
                result = self.relationship_rejection_fixture(mode)
                self.assertEqual(result['status'], 'failed', result)
                self.assertTrue(any(text in error for error in result['errors']), result['errors'])

    def test_wrong_actual_database_schema_is_rejected_by_real_comparator(self):
        self.assert_fixture_failed('wrong_schema', 'unexpected')

    def test_forged_candidate_hash_fails(self):
        self.assert_fixture_failed('forged_db_hash')

    def test_symlink_database_fails(self):
        self.assert_fixture_failed('database_symlink')

    def test_routes_require_actual_assertions_not_only_ids(self):
        self.assert_fixture_failed('missing_route_assertion', 'actual runner assertion marker')

    def test_missing_route_is_rejected(self):
        self.assert_fixture_failed('missing_route', 'migration route coverage differs')

    def test_reusing_fresh_database_for_another_route_is_rejected(self):
        self.assert_fixture_failed('reuse_route', 'must not reuse')

    def test_rejected_migration_must_preserve_original_database(self):
        self.assert_fixture_failed('rejected_modified', 'rejected migration changed')

    def test_paired_recovery_must_match_verified_backup(self):
        self.assert_fixture_failed('recovery_modified', 'does not match its verified backup')

    def test_runner_cannot_change_required_inventory(self):
        self.assert_fixture_failed('mutate_inventory', 'changed its required assertion/route inventory')

    def test_adapter_cannot_compile_design_sql_or_just_list_tests(self):
        result = self.run_fixture(mutate_adapter=lambda adapter: adapter['run_command'].append('--list'))
        self.assertEqual(result['status'], 'blocked')
        self.assertFalse(result['product_storage_tests_executed'])

    def test_unregistered_json_commands_are_not_run(self):
        errors = acceptance.storage_requirement_gaps(acceptance.ROOT, {
            'kind': 'product', 'product_storage_checks': ['json-invented'],
            'compile_command': ['arbitrary-command'], 'run_command': ['arbitrary-command']}, [], 'release')
        self.assertTrue(any('not implemented/registered' in error for error in errors))

    def test_standalone_runtime_calls_both_frozen_schema_and_boundary_execution_checks(self):
        with patch('storage_schema.validate_schema_contract', return_value=['schema drift']) as schema, \
             patch('storage_boundaries.validate', return_value={'status': 'failed', 'errors': ['removed probe']}) as boundary, \
             patch.object(acceptance, 'storage_requirement_gaps', return_value=[]):
            result = acceptance.run_product_storage(self.root, ['missing'], '.build/test')
        self.assertEqual(result['status'], 'blocked')
        self.assertTrue(any('schema drift' in error for error in result['errors']))
        self.assertTrue(any('removed probe' in error for error in result['errors']))
        schema.assert_called_once_with(self.root.resolve())
        boundary.assert_called_once_with(self.root.resolve())
        self.assertEqual(result['adapters'], [])

    def test_standalone_runtime_rejects_blocked_boundary_without_claiming_execution(self):
        with patch('storage_schema.validate_schema_contract', return_value=[]), \
             patch('storage_boundaries.validate', return_value={'status': 'blocked', 'errors': []}), \
             patch.object(acceptance, 'storage_requirement_gaps', return_value=[]):
            result = acceptance.run_product_storage(self.root, ['missing'], '.build/test')
        self.assertEqual(result['status'], 'blocked')
        self.assertFalse(result['product_storage_tests_executed'])
        self.assertTrue(result['errors'])


if __name__ == '__main__':
    unittest.main()
