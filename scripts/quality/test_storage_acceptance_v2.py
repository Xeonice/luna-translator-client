"""Delta adapter rejection tests; fixtures never stand in for production execution."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import storage_acceptance as s
import storage_product_adapter as audit

class SchemaTwoAcceptanceGuards(unittest.TestCase):
    def setUp(self):
        self.registry=json.loads((s.ROOT/s.V2_REGISTRY_PATH).read_text())

    def errors(self,data):
        return s._v2_registry_errors(s.ROOT,data)

    def test_original_22_and_431_obligations_cannot_be_rewritten(self):
        for key in ['checks','boundary_contract','plan','prerequisite_ids','maintenance_ids','enablement_ids','runtime_policy','required_fresh_routes']:
            candidate=copy.deepcopy(self.registry);candidate[key]=None
            self.assertTrue(any(key in e for e in self.errors(candidate)))

    def test_migration_one_and_dictionary_identity_remain_exact(self):
        for key in ['version','previous_version','source_path','sql_sources','migration_sha256']:
            candidate=copy.deepcopy(self.registry);candidate['supported_upgrade_matrix']['migration_chain'][0][key]=None
            self.assertTrue(any('migration-one' in e for e in self.errors(candidate)))
        candidate=copy.deepcopy(self.registry);candidate['supported_upgrade_matrix']['dictionary_versions']={}
        self.assertTrue(any('dictionary' in e for e in self.errors(candidate)))

    def test_new_migration_hash_and_source_span_cannot_be_forged(self):
        for key in ['version','previous_version','migration_sha256','sql_sources']:
            candidate=copy.deepcopy(self.registry);candidate['supported_upgrade_matrix']['migration_chain'][1][key]=None
            self.assertTrue(any('exact SQL' in e for e in self.errors(candidate)))
        candidate=copy.deepcopy(self.registry);candidate['supported_upgrade_matrix']['migration_chain'].pop()
        self.assertTrue(any('exactly' in e for e in self.errors(candidate)))

    def test_coverage_cannot_drop_or_relabel_any_old_route(self):
        for mutation in ['drop','relabel','shape']:
            candidate=copy.deepcopy(self.registry);routes=candidate['supported_upgrade_matrix']['routes']
            if mutation=='drop':routes.pop()
            elif mutation=='relabel':routes[0]['from_version']=1
            else:routes[0]['source_schema']=s.SCHEMA_PATH
            self.assertTrue(any('every real route' in e for e in self.errors(candidate)))

    def test_delta_inventory_requires_all_29_typed_and_29_boundary_checks(self):
        ids=s.v2_check_ids(s.ROOT)
        fields=json.loads((s.ROOT/'docs/architecture/database/v2/field-dictionary.json').read_text())['fields']
        self.assertEqual(len(fields),29)
        self.assertEqual(ids,{'V2-PRODUCERS','V2-RELATIONSHIPS','V2-MIGRATION','V2-EXPORT','V2-UNBIND-SERIALIZATION','V2-UNBIND-IDENTITY','V2-UNBIND-COMPENSATION'}|{'RUST-MAP:'+r['id'] for r in fields}|{'V2-FIELD:'+r['id'] for r in fields})
        self.assertTrue(s.boundary_check_ids(s.ROOT).isdisjoint(ids))

    def test_unbind_cases_cannot_borrow_another_scenarios_semantic_assertion(self):
        expected={
            'schema-two-unbind-serialization-replay':'V2-UNBIND-SERIALIZATION',
            'schema-two-unbind-stale-identity':'V2-UNBIND-IDENTITY',
            'schema-two-unbind-commit-compensation':'V2-UNBIND-COMPENSATION',
        }
        self.assertTrue(set(expected.values()) <= s.v2_check_ids(s.ROOT))
        for case, semantic_id in expected.items():
            self.assertEqual(audit.V2_CASE_SOURCES[case][2],semantic_id)
            with self.subTest(case=case),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);(root/'asserted-cases').mkdir()
                # A guard-only malformed receipt, never a runtime execution.
                (root/'receipt.json').write_text(json.dumps({'kind':'product_storage_runtime_receipt','status':'passed','adapter_version':'rust-storage-complete-v2','run_id':'guard-only','source_tree_sha256':'a'*64,'checks':[]}))
                proof={'case':case,'run_id':'guard-only','source_tree_sha256':'a'*64,'checks':[{'id':semantic_id,'status':'passed','assertions':1,'skipped':0}]}
                path=root/'asserted-cases'/f'{case}.json'
                for mutation in ('missing','wrong-id','zero','skipped','failed','boolean-count','boolean-skip','malformed'):
                    malformed=copy.deepcopy(proof)
                    if mutation=='missing':malformed['checks']=[]
                    elif mutation=='wrong-id':malformed['checks'][0]['id']='V2-PRODUCERS'
                    elif mutation=='zero':malformed['checks'][0]['assertions']=0
                    elif mutation=='skipped':malformed['checks'][0]['skipped']=1
                    elif mutation=='failed':malformed['checks'][0]['status']='failed'
                    elif mutation=='boolean-count':malformed['checks'][0]['assertions']=True
                    elif mutation=='boolean-skip':malformed['checks'][0]['skipped']=False
                    else:malformed['checks']=None
                    path.write_text(json.dumps(malformed))
                    errors=audit.audit_execution(root)['errors']
                    self.assertIn('v2 unbind case lacks its own actual semantic assertion: '+case,errors)
                path.unlink()
                self.assertIn('required real scenario missing: '+case,audit.audit_execution(root)['errors'])

    def test_v2_commands_are_closed_against_filters_and_skip(self):
        original=s.STORAGE_ADAPTERS['rust-storage-complete-v2']
        self.assertEqual(s._adapter_errors(s.ROOT,'rust-storage-complete-v2',original),[])
        for token in ['schema_two','--skip=schema_two','--exact','--list','--ignored','--test-threads=2']:
            candidate=copy.deepcopy(original);candidate['run_command'].append(token)
            self.assertTrue(s._adapter_errors(s.ROOT,'rust-storage-complete-v2',candidate))

    def test_v2_scope_target_preserves_v1_and_requires_complete_actual_source_closure(self):
        v1=s.STORAGE_ADAPTERS['rust-storage-complete-v1']
        v2=s.STORAGE_ADAPTERS['rust-storage-complete-v2']
        required=s._test_source_closure(s.ROOT,v2['rust_test_target'],True)
        prior=s._test_source_closure(s.ROOT,v1['rust_test_target'],True)
        self.assertTrue(prior < required)
        self.assertEqual(required-prior,{
            'crates/app-infra/tests/storage_product_acceptance_v2.rs',
            'crates/app-infra/tests/scoped_preferences.rs',
        })
        self.assertNotIn('crates/app-infra/tests/scoped_preferences.rs',v1['test_sources'])
        self.assertEqual(v1['rust_test_target'],'storage_product_acceptance')
        self.assertEqual(v2['rust_test_target'],'storage_product_acceptance_v2')
        self.assertEqual(s._adapter_errors(s.ROOT,'rust-storage-complete-v2',v2),[])
        reviewed_increment={
            'crates/app-infra/src/offline_translation/inspection.rs',
            'crates/app-infra/src/providers/google_execution.rs',
            'crates/app-infra/src/providers/typed_http.rs',
            'crates/app-infra/src/providers/typed_http_tests.rs',
            'crates/app-infra/src/storage/queries/google_authorization.rs',
            'crates/app-infra/tests/storage_acceptance_support/frozen_hosts.rs',
        }
        self.assertTrue(reviewed_increment <= prior)
        for source in required:
            candidate=copy.deepcopy(v2);candidate['test_sources'].remove(source)
            with self.subTest(source=source):
                self.assertEqual(s._adapter_errors(s.ROOT,'rust-storage-complete-v2',candidate),[
                    'rust-storage-complete-v2: reviewed test_sources omits compiled test import closure: '+source,
                ])
        candidate=copy.deepcopy(v2)
        for key in ('rust_test_target','compile_command','run_command'):
            candidate[key]=copy.deepcopy(v1[key])
        self.assertTrue(any('closed production' in e for e in s._adapter_errors(s.ROOT,'rust-storage-complete-v2',candidate)))

    def test_v2_rejects_unknown_and_out_of_scope_test_source_paths(self):
        original=s.STORAGE_ADAPTERS['rust-storage-complete-v2']
        for source,reason in (
            ('crates/app-infra/tests/__missing_storage_guard__.rs','missing regular file'),
            ('crates/app-core/src/lib.rs','reviewed app-infra Rust source'),
            ('crates/app-infra/Cargo.toml','reviewed app-infra Rust source'),
            ('../Cargo.lock','unsafe path'),
            (str(s.ROOT/'crates/app-infra/src/lib.rs'),'unsafe path'),
        ):
            with self.subTest(source=source):
                candidate=copy.deepcopy(original);candidate['test_sources'].append(source)
                self.assertTrue(any(reason in e for e in s._adapter_errors(s.ROOT,'rust-storage-complete-v2',candidate)))

    def test_missing_delta_proofs_and_migration_snapshot_are_not_passes(self):
        errors=s._validate_v2_delta_receipt(s.ROOT,s.ROOT,{'checks':[]},self.registry['supported_upgrade_matrix'])
        self.assertEqual(sum('producer codec' in e for e in errors),29)
        self.assertEqual(sum('valid/type/null' in e for e in errors),29)
        self.assertTrue(any('backup evidence is missing' in e for e in errors))

    def test_current_new_case_source_map_uses_actual_rust_functions(self):
        result=audit.source_map(s.ROOT)
        self.assertEqual(result['errors'],[])
        self.assertTrue(set(audit.V2_CASE_SOURCES)<={r['case'] for r in result['cases']})

if __name__=='__main__':unittest.main()
