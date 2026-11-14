import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from lifecycle_impact import (OBLIGATIONS,FINAL_CHANGE,CONTRACT,load_contract,plan_impact,close_execution,cargo_case_rows,literal_dependencies,tree_hash)

ROOT=Path(__file__).resolve().parents[2]


class BindingGateEpochSelectionGuards(unittest.TestCase):
    """Receipt-selection fixtures do not grant actual phase admission."""
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        # macOS /var is a system symlink. Guard-owned artifact roots are
        # canonical, just like the real workspace; do not weaken path checks.
        self.root = Path(self.tmp.name).resolve()
        self.prefix = '.build/binding-journey-alignment-20261109/'
        self.policy = {'receipts': {
            'plan': self.prefix + 'phase-1/plan-review.json',
            'gates': self.prefix + 'phase-2/gate-review.json',
            'implementation_admission': self.prefix + 'phase-3/admission.json'}}
        self.before = {'crates/app-core/src/lib.rs': 'a' * 64}
        self.plan = {'status': 'approved'}

    def resume(self):
        self.policy['resume_epoch'] = {
            'schema_version': 1, 'epoch_id': 'r3',
            'kind': 'admitted-p3-plan-gates-resumption/v1',
            'product_completion': 'incomplete', 'next_stage_allowed': False,
            'receipts': {
                'plan': self.prefix + 'phase-1-r3/plan-review.json',
                'gates': self.prefix + 'phase-2-r3/gate-review.json',
                'implementation_admission': self.prefix + 'phase-3-r3/admission.json',
                'gate_packet': self.prefix + 'phase-2-r3/packet/packet.json'}}

    def admission(self, relative):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('{}')

    def state(self):
        from verify import binding_gate_phase_state
        # The actual phase checker owns approval and byte verification. Isolate
        # only the subsequent receipt-slot selection under test here.
        with patch('binding_phase_guard.preimplementation',
                   return_value=(self.policy, self.plan, self.before)) as actual:
            result = binding_gate_phase_state(self.root)
            actual.assert_called_once_with(self.root)
            return result

    def test_original_epoch_keeps_original_slots_and_rejects_admitted_write(self):
        self.assertEqual(self.state()[3], self.prefix.rstrip('/') + '/phase-2')
        self.admission(self.policy['receipts']['implementation_admission'])
        with self.assertRaisesRegex(ValueError, 'after P3'):
            self.state()

    def test_reviewed_resumption_preserves_old_admission_and_selects_fresh_slots(self):
        self.resume()
        self.admission(self.policy['receipts']['implementation_admission'])
        policy, plan, before, output = self.state()
        self.assertIs(policy, self.policy)
        self.assertIs(plan, self.plan)
        self.assertIs(before, self.before)
        self.assertEqual(output, self.prefix.rstrip('/') + '/phase-2-r3')

    def test_new_epoch_admission_blocks_further_gate_execution(self):
        self.resume()
        self.admission(self.policy['resume_epoch']['receipts']['implementation_admission'])
        with self.assertRaisesRegex(ValueError, 'after P3'):
            self.state()

    def test_receipt_alias_cannot_make_old_admission_current(self):
        self.resume()
        self.policy['resume_epoch']['receipts']['implementation_admission'] = (
            self.policy['receipts']['implementation_admission'])
        with self.assertRaisesRegex(ValueError, 'predecessor receipt'):
            self.state()

    def contract_fixture(self, files=None):
        base = self.root / self.prefix / 'frozen-store4'
        source = base / 'source-root'
        source.mkdir(parents=True, exist_ok=True)
        frozen = base / 'source-manifest.json'
        frozen.write_text(json.dumps({'files': self.before if files is None else files}))
        (base / 'reader-manifest.json').write_text(json.dumps({
            'source_root': str(source), 'source_manifest_path': str(frozen)}))
        policy = dict(self.policy, parent_product_source_map=self.before)
        return base, source, policy

    def test_contract_only_guard_fixture_is_verified_data_not_product_evidence(self):
        from verify import binding_contract_guard_fixture
        base, source, policy = self.contract_fixture()
        with patch('storage_schema_v5._verify_old_writer') as verify_frozen:
            result = binding_contract_guard_fixture(self.root, policy)
        verify_frozen.assert_called_once()
        self.assertEqual(verify_frozen.call_args.args[:2], (self.root, base))
        self.assertEqual(result['source_root'], str(source))
        self.assertEqual(result['base_product_source_count'], 1)
        self.assertIs(result['product_execution'], False)
        self.assertIs(result['current_product_gaps_discharged'], False)

    def test_contract_fixture_cannot_rebase_a_changed_parent_product(self):
        from verify import binding_contract_guard_fixture
        changed = {p: 'b' * 64 for p in self.before}
        _base, _source, policy = self.contract_fixture(changed)
        with patch('storage_schema_v5._verify_old_writer'), self.assertRaisesRegex(ValueError, 'exact original R92'):
            binding_contract_guard_fixture(self.root, policy)

    def test_missing_frozen_verification_never_falls_back_to_current_draft(self):
        from verify import binding_contract_guard_fixture
        _base, _source, policy = self.contract_fixture()
        with patch('storage_schema_v5._verify_old_writer', side_effect=ValueError('frozen bytes changed')):
            with self.assertRaisesRegex(ValueError, 'frozen bytes changed'):
                binding_contract_guard_fixture(self.root, policy)

    def test_current_product_preflight_still_keeps_actual_contract_drift(self):
        import current_design_alignment as alignment
        marker = 'actual current product mapping remains unadopted'
        with patch.object(alignment, 'validate_contract', return_value=[marker]), \
                patch.object(alignment, 'required_states', return_value=(frozenset(), False)):
            result = alignment.product_change_gaps(
                self.root, {'kind': 'product'}, changed_paths=['docs/README.md'])
        self.assertEqual(result, [marker])


class LifecycleImpactGuards(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
        self.c=json.loads((ROOT/CONTRACT).read_text());self.c['historical_selection_sha256']={}
        for name in [CONTRACT,'docs/development/stability-refactor-plan.json','docs/architecture/runtime-lifecycle-contracts.json','docs/architecture/engineering-modules.csv','docs/architecture/engineering-components.csv']:
            p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((ROOT/name).read_bytes())
        for r in self.c['cases']:
            for name in r.get('source_candidates',[r['source']]):
                p=self.root/name
                if (ROOT/name).exists():p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((ROOT/name).read_bytes())
        self.path='apps/desktop/src/bridge/preview-subscriptions.ts';p=self.root/self.path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((ROOT/self.path).read_bytes())
        self.m={'id':'new-preview-change','requirements':[{'module_ids':['M12','M13','M02','M14','M16','M04','M10'],'component_ids':sorted({v for rule in self.c['rules'] for v in rule['component_ids']}),'checks':sorted({c['check_id'] for c in self.c['cases']} | {'source-scope','baseline-integrity','spec-contract-tests','rust-boundary-tests','frontend-boundary-tests','technology-contract-tests','generated-contract-drift','generated-contract-tests','typescript-compile','rust-compile','reading-component-guard-tests'})}],'product_visual_checks':['product-ui-capture-v4'],'product_runtime_checks':[]}
        # Controlled selector fixture isolates lifecycle scheduling/closure tests.
        # Genuine current authority/capability is exercised separately below.
        self.selector = patch('lifecycle_impact.current_visual_selection', side_effect=self.current_selection_fixture)
        self.selector.start(); self.addCleanup(self.selector.stop)
        self.m['requirements'][0]['checks'] += ['guard-current-react', 'guard-current-native']
        self.m['requirements'][0]['checks'] += ['binding-journey-execution', 'binding-journey-guard-tests']
        from binding_journey import STORAGE_V5_CHECKS
        self.m['requirements'][0]['checks'] += sorted(STORAGE_V5_CHECKS)
    def current_selection_fixture(self, root, paths, *, full=False):
        from current_design_alignment import STATE_IDS
        return {'policy':'current_design_actual_execution', 'target_id':'guard-current',
                'composition_id':'guard-composition', 'design_closure_sha256':'a'*64,
                'authority':{'path':'guard-authority','sha256':'b'*64},
                'full_current_closure':full,
                'required_states':sorted(STATE_IDS) if full else ['reading-current-card'],
                'required_layers':['packaged_native','real_react'],
                'required_checks':['guard-current-native','guard-current-react'], 'errors':[]}
    def plan(self,paths=None):return plan_impact(self.root,self.m,[self.path] if paths is None else paths,contract=self.c)
    def test_binding_storage_literal_closure_keeps_real_modules_and_writer_checks(self):
        c=load_contract(self.root,require_pin=False);rules={r['path']:r for r in c['rules']}
        parent='crates/app-infra/src/storage/queries/mod.rs'
        pending=[parent];seen=set()
        while pending:
            path=pending.pop()
            if path in seen:continue
            seen.add(path)
            for dependency in literal_dependencies(ROOT,path):
                self.assertIn(dependency,rules,dependency)
                self.assertIn('M10',rules[dependency]['module_ids'])
                self.assertTrue({'SRF-READING-VERSION','SRF-WRITER-BOUNDARY'} <= set(rules[dependency]['obligations']))
                pending.append(dependency)
        self.assertGreater(len(seen),12)
        self.assertNotIn('M14', {m for path in seen for m in rules[path]['module_ids']})

    def test_shared_native_query_keeps_translation_and_binding_owners_and_derived_checks(self):
        path='apps/desktop/src-tauri/src/ipc/queries.rs'
        rule=next(r for r in self.c['rules'] if r['path']==path)
        self.assertTrue({'C17','C13','C14','C15','C35'} <= set(rule['component_ids']))
        self.assertIn('M12',rule['module_ids'])
        selected=self.plan([path])
        self.assertEqual([],selected['errors'])
        expected={case['check_id'] for case in self.c['cases'] if case['obligation_id'] in rule['obligations']}
        self.assertTrue(expected <= set(selected['required_checks']))
        self.assertTrue({'binding-journey-execution','binding-journey-guard-tests'} <= set(selected['required_checks']))

    def test_missing_recursive_storage_literal_rule_is_rejected(self):
        parent='crates/app-infra/src/storage/queries/google_authorization.rs'
        child='crates/app-infra/src/storage/queries/google_authorization/authority.rs'
        for path in (parent,child):
            target=self.root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((ROOT/path).read_bytes())
        self.c['rules']=[rule for rule in self.c['rules'] if rule['path']!=child]
        self.assertTrue(any(child in e for e in self.plan([parent])['errors']))

    def test_actual_paths_derive_four_preview_obligations(self):
        p=self.plan();self.assertEqual(p['errors'],[]);self.assertEqual(len(p['obligations']),4);self.assertIn('product-reading-component-tests',p['required_checks'])
    def test_binding_actual_impact_cannot_be_waived_by_removing_metadata_check(self):
        self.m['requirements'][0]['checks'].remove('binding-journey-execution')
        p=self.plan()
        self.assertIn('binding-journey-execution',p['required_checks'])
        self.assertTrue(any('binding-journey-execution' in error for error in p['errors']))
    def test_schema_five_actual_migration_impact_cannot_drop_metadata_checks(self):
        from binding_journey import STORAGE_V5_CHECKS
        self.m['requirements'][0]['checks'] = [value for value in self.m['requirements'][0]['checks'] if value not in STORAGE_V5_CHECKS]
        p=self.plan(['crates/app-infra/src/storage/migrations/005_source_selection_sets.sql'])
        self.assertTrue(STORAGE_V5_CHECKS <= set(p['required_checks']))
        self.assertTrue(any('storage-schema-v5-runtime-inspection' in value for value in p['errors']))
    def test_unknown_new_critical_file_rejected(self):
        p=self.plan(['apps/desktop/src/bridge/new-preview-v2.ts']);self.assertTrue(any('Unregistered critical' in e for e in p['errors']))
    def test_acceptance_runtime_paths_keep_bootstrap_lifecycle_obligations(self):
        path='apps/desktop/src-tauri/src/platform/runtime_paths.rs'
        rules={r['path']:r for r in self.c['rules']}
        rule=rules[path]
        self.assertEqual(rule['module_ids'],['M12'])
        self.assertEqual(rule['component_ids'],rules['apps/desktop/src-tauri/src/bootstrap.rs']['component_ids'])
        self.assertEqual(rule['obligations'],rules['apps/desktop/src-tauri/src/bootstrap.rs']['obligations'])
        self.assertEqual(self.plan([path])['errors'],[])
        self.c['rules']=[r for r in self.c['rules'] if r['path']!=path]
        self.assertTrue(any('Unregistered critical' in e for e in self.plan([path])['errors']))
    def test_preview_details_keeps_all_four_preview_obligations(self):
        path='apps/desktop/src/features/capture/PreviewDetails.tsx'
        rules={r['path']:r for r in self.c['rules']}
        self.assertEqual(rules[path]['module_ids'],['M13'])
        self.assertEqual(rules[path]['component_ids'],['C35'])
        self.assertEqual(set(rules[path]['obligations']),{'SRF-PV-IDENTITY','SRF-PV-CONTEXT','SRF-PV-FRAME-TRUTH','SRF-PV-ISOLATION'})
        p=self.plan([path]);self.assertEqual(p['errors'],[])
        self.assertEqual(set(p['obligations']),set(rules['apps/desktop/src/features/capture/WindowPreview.tsx']['obligations']))
        # Preview's four frozen obligations map to Native IPC + React cases;
        # core/infra checks are additionally selected by the full reading slice.
        self.assertTrue({'product-reading-component-tests','product-native-ipc-tests'} <= set(p['required_checks']))
    def test_removing_details_registration_is_not_a_ui_exemption(self):
        path='apps/desktop/src/features/capture/PreviewDetails.tsx'
        self.c['rules']=[r for r in self.c['rules'] if r['path']!=path]
        self.assertTrue(any('Unregistered critical' in e for e in self.plan([path])['errors']))
    def test_deleted_old_and_added_new_both_examined(self):
        p=self.plan([self.path,'apps/desktop/src/bridge/renamed-preview.ts']);self.assertIn(self.path,p['changed_hashes']);self.assertIn('apps/desktop/src/bridge/renamed-preview.ts',p['changed_hashes']);self.assertTrue(p['errors'])
    def test_unrelated_module_cannot_waive_actual_path(self):
        self.m['requirements'][0]['module_ids']=['M01'];self.assertTrue(any('ownership' in e for e in self.plan()['errors']))
    def test_missing_component_check_rejected(self):
        self.m['requirements'][0]['checks'].remove('product-reading-component-tests');self.assertTrue(any('required checks' in e for e in self.plan()['errors']))
    def test_final_empty_diff_still_forces_nine_and_runtime(self):
        self.m['id']=FINAL_CHANGE;p=self.plan([]);self.assertEqual(set(p['obligations']),OBLIGATIONS);self.assertEqual(len(p['invariant_links']),14);self.assertTrue(any('product-packaged' in e for e in p['errors']))
    def test_final_claimed_coverage_does_not_replace_execution(self):
        self.m['id']=FINAL_CHANGE;self.m['product_runtime_checks']=['product-packaged-stability-tests'];p=self.plan([]);p['claimed_coverage']=list(OBLIGATIONS);self.assertTrue(close_execution(self.root,p,[]))
    def test_changed_source_after_selection_rejected(self):
        p=self.plan();(self.root/self.path).write_text('changed');self.assertTrue(any('source cohort' in e for e in close_execution(self.root,p,[])))
    def test_exact_target_qualified_cases_not_just_count(self):
        text='Running unittests src/lib.rs (/private/t/app_infra-abc)\nrunning 1 test\ntest real::case ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n';r=cargo_case_rows(text);self.assertEqual(r,{('unittests src/lib.rs','real::case'):'ok'});self.assertNotIn(('tests/other.rs','real::case'),r)
    def test_zero_filtered_ignored_and_failed_cases_rejected(self):
        for summary in ['ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;','ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out;','ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;','FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;']:
            with self.subTest(summary=summary):
                with self.assertRaises(ValueError):cargo_case_rows('test result: '+summary)
    def test_duplicate_real_case_rejected(self):
        with self.assertRaises(ValueError):cargo_case_rows('test x::y ... ok\ntest x::y ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;')
    def test_unresolved_raw_order_cannot_get_a_target(self):
        r=cargo_case_rows('test x::y ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;');self.assertNotIn(('unittests src/lib.rs','x::y'),r)
    def test_literal_new_dependency_must_be_registered(self):
        p=self.root/'apps/desktop/src/bridge/tauri-port.ts';p.write_text("import './unknown-client';");(p.parent/'unknown-client.ts').write_text('export {};');q=self.plan([p.relative_to(self.root).as_posix()]);self.assertTrue(any('literal dependency' in e for e in q['errors']))
    def test_rust_path_attribute_resolves_moved_module(self):
        p=self.root/'apps/desktop/src-tauri/src/platform/window_manager.rs';p.write_text('#[path = "owner_geometry.rs"] mod content_fit_policy;');(p.parent/'owner_geometry.rs').write_text('');self.assertIn('apps/desktop/src-tauri/src/platform/owner_geometry.rs',literal_dependencies(self.root,p.relative_to(self.root).as_posix()))
    def test_contract_requires_exact_authority_pin(self):
        p=self.root/'docs/development/iteration-baseline.json';p.write_text(json.dumps({'files':[]}))
        with self.assertRaisesRegex(ValueError,'authority pin'):load_contract(self.root)
    def test_complete_contract_checks_all_named_case_definitions(self):
        self.assertEqual(set(load_contract(self.root,require_pin=False)['obligations']),OBLIGATIONS)
    def test_removed_component_id_is_rejected_even_if_total_stays_100(self):
        p=self.root/'apps/desktop/tests/visual/verify-reviewed-reading.mjs';p.write_text(p.read_text().replace('RBC-91-single-flight-pull-and-late-finally-do-not-delete-new-context','RBC-91-removed'))
        with self.assertRaisesRegex(ValueError,'case removed'):load_contract(self.root,require_pin=False)
    def test_unknown_contract_fields_and_duplicate_rules_rejected(self):
        for mutate in [lambda c:c.update(claimed_pass=True),lambda c:c['rules'].append(copy.deepcopy(c['rules'][0]))]:
            c=copy.deepcopy(self.c);mutate(c);(self.root/CONTRACT).write_text(json.dumps(c))
            with self.assertRaises(ValueError):load_contract(self.root,require_pin=False)
    def test_m14_cannot_be_relabelled_storage(self):
        p=self.root/'docs/architecture/engineering-modules.csv';p.write_text(p.read_text().replace('M14,native-broker','M14,storage'))
        with self.assertRaisesRegex(ValueError,'M14'):load_contract(self.root,require_pin=False)
    def test_historical_phase_selection_cannot_be_rewritten(self):
        self.c['historical_selection_sha256'][self.m['id']]='a'*64;self.assertTrue(any('historical' in e for e in self.plan()['errors']))
    def test_old_nonce_and_source_receipt_rejected(self):
        p=self.plan();case=next(c for c in p['cases'] if c['kind']=='component');step={'id':case['check_id'],'status':'passed','execution':{'status':'passed','exit_code':0,'source_tree_sha256':'old'}}
        self.assertTrue(any('old/mixed' in e for e in close_execution(self.root,p,[step])))
    def test_storage_guard_still_runs_in_gate_and_m14_not_a_storage_waiver(self):
        text=(ROOT/'scripts/quality/verify.py').read_text();self.assertIn('storage_requirement_gaps(ROOT, metadata, paths, args.profile)',text);self.assertIn('product-storage-execution',text)

    def test_legal_but_wrong_component_cannot_claim_preview_path(self):
        self.m['requirements'][0]['component_ids']=['C18']
        self.assertTrue(any('component ownership' in e for e in self.plan()['errors']))
    def test_component_and_module_must_belong_to_same_requirement(self):
        self.m['requirements'][0]['component_ids']=['C18']
        self.m['requirements'].append({'module_ids':['M01'],'component_ids':['C35'],'checks':[]})
        self.assertTrue(any('component ownership' in e for e in self.plan()['errors']))

    def test_reviewed_wordcard_geometry_and_hud_components_are_legitimate_owners(self):
        for path,module,component in [('apps/desktop/src-tauri/src/platform/owner_geometry.rs','M12','C19'),('apps/desktop/src/features/language/WordCard.tsx','M13','C19'),('apps/desktop/src/windows/hud/HudWindow.tsx','M13','C18')]:
            with self.subTest(path=path):
                self.m['requirements'][0].update(module_ids=[module],component_ids=[component])
                self.assertEqual(self.plan([path])['errors'],[])
    def test_preview_component_cannot_claim_wordcard_and_credentials_cannot_claim_storage(self):
        for path,module,component in [('apps/desktop/src/features/language/WordCard.tsx','M13','C35'),('apps/desktop/src-tauri/src/platform/owner_geometry.rs','M12','C35'),('crates/app-infra/src/storage/database.rs','M16','C35')]:
            with self.subTest(path=path):
                self.m['requirements'][0].update(module_ids=[module],component_ids=[component])
                self.assertTrue(any('ownership' in e for e in self.plan([path])['errors']))
    def test_canonical_storage_owner_is_m10_and_m16_only_credential_query(self):
        rules={r['path']:r for r in self.c['rules']}
        self.assertEqual(rules['crates/app-infra/src/storage/database.rs']['module_ids'],['M10'])
        self.assertIn('M16',rules['crates/app-infra/src/storage/queries/credentials.rs']['module_ids'])

    def finalize_actual_gate(self,declared,required=('source-scope',)):
        from verify import finalize_execution
        # This fixture bypasses unrelated compiler/runtime work; finalization and
        # lifecycle receipt validation are the actual production functions.
        steps=[{'id':'source-scope','status':'passed','errors':[]}]
        plan={'errors':[],'source_tree_sha256':tree_hash(self.root),'required_checks':list(required),'required_automatic_checks':[],'cases':[],'final_cohort':False}
        def record(name,errors,**details):
            steps.append({'id':name,'status':'failed' if errors else 'passed','errors':list(errors),**details})
        finalize_execution(self.root,plan,steps,set(declared),record,'independent-plan-hash')
        return steps
    def test_real_gate_finalization_records_declared_lifecycle_before_declaration_check(self):
        steps=self.finalize_actual_gate(['source-scope','lifecycle-impact-execution'])
        self.assertEqual([r['id'] for r in steps],['source-scope','lifecycle-impact-execution','scenario-check-execution'])
        self.assertTrue(all(r['status']=='passed' and not r['errors'] for r in steps))
    def test_real_gate_finalization_still_rejects_truly_missing_declared_check(self):
        steps=self.finalize_actual_gate(['source-scope','lifecycle-impact-execution','typescript-compile'])
        actual={r['id']:r for r in steps}
        self.assertEqual(actual['lifecycle-impact-execution']['status'],'passed')
        self.assertEqual(actual['scenario-check-execution']['errors'],['Declared check did not execute: typescript-compile'])
        self.assertEqual(actual['scenario-check-execution']['status'],'failed')
    def test_real_gate_finalization_keeps_failed_closure_as_failure_even_when_executed(self):
        steps=self.finalize_actual_gate(['source-scope','lifecycle-impact-execution'],('source-scope','rust-compile'))
        actual={r['id']:r for r in steps}
        self.assertEqual(actual['lifecycle-impact-execution']['status'],'failed')
        self.assertEqual(actual['lifecycle-impact-execution']['errors'],['Required lifecycle execution missing or failed: rust-compile'])
        self.assertEqual(actual['scenario-check-execution']['errors'],[])
        self.assertTrue(any(r['status']=='failed' for r in steps))

    def test_registered_additional_final_empty_diff_has_identical_complete_selection(self):
        self.m['requirements'][0]['checks']=sorted(set(self.m['requirements'][0]['checks']) | set(self.c['phases']['SR-6']['checks']))
        self.m['product_runtime_checks']=[self.c['runtime_check']]
        self.m['id']=FINAL_CHANGE;primary=self.plan([])
        self.m['id']=self.c['additional_final_change_ids'][0];additional=self.plan([])
        for key in ('final_cohort','obligations','invariant_links','required_checks','required_automatic_checks','cases','errors'):
            self.assertEqual(additional[key],primary[key],key)
        self.assertTrue(additional['final_cohort']);self.assertEqual(set(additional['obligations']),OBLIGATIONS)
        self.assertEqual(len(additional['invariant_links']),14);self.assertEqual(additional['errors'],[])

    def test_registered_additional_final_cannot_omit_runtime_or_component_check(self):
        self.m['id']=self.c['additional_final_change_ids'][0]
        p=self.plan([]);self.assertTrue(any('product-packaged-stability-tests' in e for e in p['errors']))
        self.m['product_runtime_checks']=[self.c['runtime_check']]
        self.m['requirements'][0]['checks'].remove('product-reading-component-tests')
        p=self.plan([]);self.assertTrue(any('product-reading-component-tests' in e for e in p['errors']))
        self.assertEqual(set(p['obligations']),OBLIGATIONS)

    def test_registered_additional_final_claims_do_not_replace_actual_execution(self):
        self.m['id']=self.c['additional_final_change_ids'][0];self.m['product_runtime_checks']=[self.c['runtime_check']]
        p=self.plan([]);p['claimed_coverage']=list(OBLIGATIONS)
        self.assertTrue(close_execution(self.root,p,[]))

    def test_unregistered_runtime_cohort_is_rejected_even_on_empty_diff(self):
        self.m['id']='unregistered-final-verification';self.m['product_runtime_checks']=[self.c['runtime_check']]
        self.m['final_cohort']=True;self.m['claimed_coverage']=list(OBLIGATIONS)
        p=self.plan([]);self.assertFalse(p['final_cohort'])
        self.assertTrue(any('Unregistered final runtime cohort' in e for e in p['errors']))

    def test_unknown_ordinary_change_still_uses_actual_diff_without_name_inference(self):
        self.m['id']='verify-refactor-stability-unregistered'
        p=self.plan([]);self.assertFalse(p['final_cohort']);self.assertEqual(p['obligations'],[]);self.assertEqual(p['errors'],[])
        p=self.plan();self.assertEqual(len(p['obligations']),4);self.assertEqual(p['errors'],[])

    def test_additional_final_registry_requires_nonempty_canonical_unique_nonphase_ids(self):
        registered=self.c['additional_final_change_ids'][0]
        historical=next(row['change_id'] for row in self.c['phases'].values() if row['change_id'] != FINAL_CHANGE)
        for value in (None,True,{},'new-final',[],[''],[' leading'],['Upper-case'],['bad/path'],[registered,registered],[FINAL_CHANGE],[historical],[{'id':'fake-approved','approved':True}]):
            with self.subTest(value=value):
                c=copy.deepcopy(self.c);c['additional_final_change_ids']=value
                (self.root/CONTRACT).write_text(json.dumps(c))
                with self.assertRaises(ValueError):load_contract(self.root,require_pin=False)

    def test_schema_two_cannot_omit_registration_or_accept_unknown_override_fields(self):
        c=copy.deepcopy(self.c);del c['additional_final_change_ids'];(self.root/CONTRACT).write_text(json.dumps(c))
        with self.assertRaises(ValueError):load_contract(self.root,require_pin=False)
        c=copy.deepcopy(self.c);c['final_checks_override']=[];(self.root/CONTRACT).write_text(json.dumps(c))
        with self.assertRaises(ValueError):load_contract(self.root,require_pin=False)

    def test_schema_one_cannot_downgrade_final_cohort_registration(self):
        c=copy.deepcopy(self.c);c['schema_version']=1
        for omit_registration in (False,True):
            with self.subTest(omit_registration=omit_registration):
                candidate=copy.deepcopy(c)
                if omit_registration:del candidate['additional_final_change_ids']
                (self.root/CONTRACT).write_text(json.dumps(candidate))
                with self.assertRaises(ValueError):load_contract(self.root,require_pin=False)

    def test_actual_pinned_registry_selects_new_final_and_refuses_unapproved_append(self):
        path=self.root/CONTRACT;actual=json.loads(path.read_text())
        baseline=self.root/'docs/development/iteration-baseline.json'
        baseline.write_text(json.dumps({'files':[{'path':CONTRACT,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}]}))
        approved=load_contract(self.root)
        self.m['id']=approved['additional_final_change_ids'][0]
        p=plan_impact(self.root,self.m,[],contract=approved);self.assertTrue(p['final_cohort']);self.assertEqual(set(p['obligations']),OBLIGATIONS)
        actual['additional_final_change_ids'].append('unapproved-additional-final');path.write_text(json.dumps(actual))
        with self.assertRaisesRegex(ValueError,'authority pin'):load_contract(self.root)

    def test_registry_contract_pin_must_be_unique_in_both_orders(self):
        path=self.root/CONTRACT;good={'path':CONTRACT,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()};bad={'path':CONTRACT,'sha256':'0'*64}
        baseline=self.root/'docs/development/iteration-baseline.json'
        for rows in ([good,bad],[bad,good],[good,good]):
            with self.subTest(rows=rows):
                baseline.write_text(json.dumps({'files':rows}))
                with self.assertRaisesRegex(ValueError,'authority pin'):load_contract(self.root)

    def test_additional_final_registry_does_not_rewrite_primary_phase_or_obligation_policy(self):
        c=copy.deepcopy(self.c);c['final_change_id']='renamed-primary';(self.root/CONTRACT).write_text(json.dumps(c))
        with self.assertRaisesRegex(ValueError,'may not be renamed'):load_contract(self.root,require_pin=False)
        self.m['id']=self.c['additional_final_change_ids'][0];self.m['product_runtime_checks']=[self.c['runtime_check']]
        self.assertTrue(self.plan([])['required_automatic_checks'])
        self.assertEqual(len(self.plan([])['invariant_links']),14)

    def test_release_keeps_existing_all_obligations_for_unregistered_change(self):
        self.m['id']='ordinary-release-check'
        self.m['requirements'][0]['checks']=sorted(set(self.m['requirements'][0]['checks']) | set(self.c['phases']['SR-6']['checks']))
        self.m['product_runtime_checks']=[self.c['runtime_check']]
        p=plan_impact(self.root,self.m,[],'release',contract=self.c)
        self.assertTrue(p['final_cohort']);self.assertEqual(set(p['obligations']),OBLIGATIONS)
        self.assertEqual(p['errors'],[])

    def test_all_registered_final_cohorts_retain_identical_complete_empty_diff_selection(self):
        self.m['requirements'][0]['checks']=sorted(set(self.m['requirements'][0]['checks']) | set(self.c['phases']['SR-6']['checks']))
        self.m['product_runtime_checks']=[self.c['runtime_check']]
        self.m['id']=FINAL_CHANGE;primary=self.plan([])
        for registered in self.c['additional_final_change_ids']:
            with self.subTest(registered=registered):
                self.m['id']=registered;actual=self.plan([])
                for key in ('final_cohort','obligations','invariant_links','required_checks','required_automatic_checks','cases','errors'):
                    self.assertEqual(actual[key],primary[key],key)
                self.assertEqual(actual['errors'],[])
                self.assertEqual(set(actual['obligations']),OBLIGATIONS)
                self.assertEqual(len(actual['invariant_links']),14)
    def test_every_registered_final_rejects_missing_native_component_and_runtime_checks(self):
        checks=sorted(set(self.m['requirements'][0]['checks']) | set(self.c['phases']['SR-6']['checks']))
        for registered in (FINAL_CHANGE,*self.c['additional_final_change_ids']):
            for omitted in ('product-native-ipc-tests','product-reading-component-tests',self.c['runtime_check']):
                with self.subTest(registered=registered,omitted=omitted):
                    self.m['id']=registered
                    self.m['requirements'][0]['checks']=[x for x in checks if x!=omitted]
                    self.m['product_runtime_checks']=[] if omitted==self.c['runtime_check'] else [self.c['runtime_check']]
                    self.assertTrue(any(omitted in e for e in self.plan([])['errors']))
    def test_translation_projection_helper_has_registered_critical_writer_and_reading_edges(self):
        helper='crates/app-infra/src/storage/queries/translation_status.rs'
        parent='crates/app-infra/src/storage/queries/mod.rs'
        self.assertIn(helper,literal_dependencies(ROOT,parent))
        row=next(r for r in self.c['rules'] if r['path']==helper)
        self.assertEqual(set(row['obligations']),{'SRF-READING-VERSION','SRF-WRITER-BOUNDARY'})
        self.assertEqual(set(row['module_ids']),{'M10','M05'})
        self.assertIn('C17',row['component_ids'])


    def test_new_lifecycle_requires_current_actual_checks_not_legacy_v4(self):
        p=self.plan()
        self.assertEqual(p['errors'],[])
        self.assertNotIn('product-ui-capture-v4',p['required_checks'])
        self.assertTrue({'guard-current-react','guard-current-native'} <= set(p['required_checks']))
        self.assertEqual(p['visual_execution']['required_states'],['reading-current-card'])
        self.m['requirements'][0]['checks'].remove('guard-current-react')
        self.m['requirements'][0]['checks'].append('current-design-product-alignment')
        self.assertTrue(any('guard-current-react' in e for e in self.plan()['errors']))

    def test_all_final_and_release_select_complete_current_without_sr6_v4_reintroduction(self):
        from current_design_alignment import STATE_IDS
        self.m['requirements'][0]['checks'] += self.c['phases']['SR-6']['checks']
        self.m['product_runtime_checks']=[self.c['runtime_check']]
        for cid,profile in [(FINAL_CHANGE,'development'),*[(x,'development') for x in self.c['additional_final_change_ids']],('ordinary-release','release')]:
            with self.subTest(cid=cid,profile=profile):
                self.m['id']=cid
                p=plan_impact(self.root,self.m,[],profile,contract=self.c)
                self.assertEqual(p['errors'],[])
                self.assertEqual(set(p['obligations']),OBLIGATIONS)
                self.assertEqual(set(p['visual_execution']['required_states']),STATE_IDS)
                self.assertEqual(set(p['visual_execution']['required_layers']),{'real_react','packaged_native'})
                self.assertNotIn('product-ui-capture-v4',p['required_checks'])
                self.assertTrue((set(self.c['phases']['SR-6']['checks'])-{'product-ui-capture-v4'}) <= set(p['required_checks']))
                self.assertIn(self.c['runtime_check'],p['required_checks'])

    def test_historical_nonfinal_phase_keeps_exact_visual_selection(self):
        for phase in self.c['phases'].values():
            if phase['change_id']==FINAL_CHANGE: continue
            with self.subTest(phase=phase['change_id']):
                self.m['id']=phase['change_id']
                p=self.plan([])
                self.assertEqual(p['required_checks'],sorted(phase['checks']))
                self.assertEqual(p['visual_execution'],{'policy':'historical_phase_selection','required_checks':phase['visual']})

    def test_actual_current_authority_missing_adapters_stays_blocked(self):
        self.selector.stop()
        try:
            from lifecycle_impact import current_visual_selection
            selection=current_visual_selection(ROOT,[self.path])
        finally:
            self.selector.start()
        self.assertEqual(selection['target_id'],'guided-service-reading-20261030')
        self.assertEqual(selection['composition_id'],'complete-guided-history-20261031')
        self.assertTrue(selection['required_states'])
        from current_design_react_adapter import CHECK_ID, STATE_IDS as FINITE_REACT_STATES
        from current_general_visual_adapter import REACT_CHECK, NATIVE_CHECK, STATE_IDS as GENERAL_STATES
        from current_design_alignment import ADAPTERS
        self.assertEqual(selection['required_checks'],sorted([CHECK_ID,REACT_CHECK,NATIVE_CHECK]))
        required=set(selection['required_states'])
        self.assertLess(set(FINITE_REACT_STATES),required)
        self.assertLess(set(GENERAL_STATES),required)
        expected_coverage={'real_react':set(FINITE_REACT_STATES)|set(GENERAL_STATES),'packaged_native':set(GENERAL_STATES)}
        for layer,coverage in expected_coverage.items():
            actual=set().union(*(ADAPTERS[check].states for check in selection['required_checks'] if ADAPTERS[check].layer==layer))
            self.assertEqual(coverage,actual)
            missing=required-coverage
            self.assertTrue(missing)
            prefix=f'no registered current-target {layer} executor with independent inspection/recompute for states: '
            gaps=[e[len(prefix):] for e in selection['errors'] if e.startswith(prefix)]
            self.assertEqual(len(gaps),1)
            self.assertEqual(set(gaps[0].split(',')),missing)
        self.assertFalse(any('authority is not approved' in e for e in selection['errors']))

    def test_green_step_without_actual_current_dispatch_cannot_close(self):
        from lifecycle_impact import inspect_current_visual_execution
        selection=self.current_selection_fixture(self.root,[])
        steps={k:{'id':k,'status':'passed','execution':{'status':'passed','errors':[], 'adapter_id':k}}
               for k in selection['required_checks']}
        errors=inspect_current_visual_execution(self.root,selection,steps)
        self.assertTrue(any('invocation' in e for e in errors))
        self.assertTrue(any('actual real_react state coverage missing' in e for e in errors))
        self.assertTrue(any('actual packaged_native state coverage missing' in e for e in errors))

    def test_wrong_selected_dispatch_adapter_cannot_close(self):
        from lifecycle_impact import inspect_current_visual_execution
        selection=self.current_selection_fixture(self.root,[])
        steps={k:{'id':k,'status':'passed','execution':{'status':'passed','errors':[], 'adapter_id':'other-registered'}}
               for k in selection['required_checks']}
        self.assertTrue(any('adapter differs' in e for e in inspect_current_visual_execution(self.root,selection,steps)))

    def test_selected_current_authority_cannot_be_rewritten_after_planning(self):
        p=self.plan();p['visual_execution']['composition_id']='old-v4'
        self.assertTrue(any('selection is incomplete or changed' in e for e in close_execution(self.root,p,[])))

    def guard_visual_receipts(self):
        """Envelope-only adversarial fixture; not a product capture executor."""
        import current_design_alignment as d
        def execute(*args): return None
        def inspect(*args): return []
        def recompute(*args): return []
        selection={'policy':'current_design_actual_execution','target_id':d.TARGET,
                   'composition_id':d.COMPOSITION_ID,'design_closure_sha256':d.DESIGN_CLOSURE_SHA256,
                   'required_states':['reading-current-card'],'required_layers':['real_react','packaged_native'],
                   'required_checks':['guard-current-react','guard-current-native']}
        package=self.root/'.build/guard-package.zip';package.parent.mkdir(exist_ok=True);package.write_bytes(b'actual-guard-package')
        adapters={}; steps={}
        for check,layer in [('guard-current-react','real_react'),('guard-current-native','packaged_native')]:
            adapter=d.Adapter(d.TARGET,layer,frozenset(selection['required_states']),('guard-only',layer),execute,inspect,recompute)
            adapters[check]=adapter;output='.build/'+check;directory=self.root/output;directory.mkdir()
            context={'schema_version':1,'adapter_id':check,'target_id':d.TARGET,'composition_id':d.COMPOSITION_ID,
                     'design_closure_sha256':d.DESIGN_CLOSURE_SHA256,'source_tree_sha256':tree_hash(self.root),
                     'layer':layer,'state_ids':selection['required_states'],'command':list(adapter.command),
                     'execution_nonce':('a' if layer=='real_react' else 'b')*32,
                     'artifact_path':'.build/guard-package.zip','artifact_sha256':hashlib.sha256(package.read_bytes()).hexdigest(),
                     'output':output,'started_at':'2026-11-04T00:00:00+00:00','finished_at':'2026-11-04T00:00:02+00:00'}
            receipt={**context,'status':'passed','errors':[],'producer_kind':layer,'prototype_substitution':False,
                     'native_verified':layer=='packaged_native','assertion_count':1,'skipped_assertions':0,
                     'started_at':'2026-11-04T00:00:00.500000+00:00','finished_at':'2026-11-04T00:00:01+00:00'}
            ip=directory/'invocation.json';rp=directory/'receipt.json';ip.write_text(json.dumps(context));rp.write_text(json.dumps(receipt))
            steps[check]={'id':check,'status':'passed','execution':{'status':'passed','errors':[],'adapter_id':check,
                          'execution_nonce':context['execution_nonce'],'invocation':output+'/invocation.json',
                          'invocation_sha256':hashlib.sha256(ip.read_bytes()).hexdigest(),'receipt':output+'/receipt.json',
                          'receipt_sha256':hashlib.sha256(rp.read_bytes()).hexdigest()}}
        return d,selection,steps,adapters

    def rewrite_guard_evidence(self,execution,field,mutate):
        p=self.root/execution[field];data=json.loads(p.read_text());mutate(data);p.write_text(json.dumps(data))
        execution[field+'_sha256']=hashlib.sha256(p.read_bytes()).hexdigest()

    def test_current_dispatch_envelope_closes_only_after_both_raw_inspections(self):
        from lifecycle_impact import inspect_current_visual_execution
        d,selection,steps,adapters=self.guard_visual_receipts()
        with patch.dict(d.ADAPTERS,adapters,clear=True):
            self.assertEqual(inspect_current_visual_execution(self.root,selection,steps),[])
            for field,mutation in [('receipt',lambda r:r.update(execution_nonce='c'*32)),
                                   ('receipt',lambda r:r.update(finished_at='2026-11-04T00:00:03+00:00')),
                                   ('receipt',lambda r:r.update(adapter_id='guard-current-native')),
                                   ('invocation',lambda r:r.update(adapter_id='guard-current-native')),
                                   ('invocation',lambda r:r.update(command=['different']))]:
                with self.subTest(field=field,mutation=mutation):
                    ex=steps['guard-current-react']['execution'];p=self.root/ex[field];raw=p.read_bytes();old=ex[field+'_sha256']
                    self.rewrite_guard_evidence(ex,field,mutation)
                    self.assertTrue(inspect_current_visual_execution(self.root,selection,steps))
                    p.write_bytes(raw);ex[field+'_sha256']=old

    def test_current_dispatch_rejects_actual_changed_or_mixed_package_and_bad_hash(self):
        from lifecycle_impact import inspect_current_visual_execution
        d,selection,steps,adapters=self.guard_visual_receipts()
        with patch.dict(d.ADAPTERS,adapters,clear=True):
            package=self.root/'.build/guard-package.zip';original=package.read_bytes();package.write_bytes(b'changed')
            self.assertTrue(any('package identity differs' in e for e in inspect_current_visual_execution(self.root,selection,steps)))
            package.write_bytes(original)
            ex=steps['guard-current-react']['execution'];good=ex['invocation_sha256'];ex['invocation_sha256']='0'*64
            self.assertTrue(any('invocation hash mismatch' in e for e in inspect_current_visual_execution(self.root,selection,steps)))
            ex['invocation_sha256']=good
            other=self.root/'.build/other.zip';other.write_bytes(b'other-actual-package');sha=hashlib.sha256(other.read_bytes()).hexdigest()
            self.rewrite_guard_evidence(ex,'invocation',lambda r:r.update(artifact_path='.build/other.zip',artifact_sha256=sha))
            self.rewrite_guard_evidence(ex,'receipt',lambda r:r.update(artifact_sha256=sha))
            self.assertTrue(any('different packages' in e for e in inspect_current_visual_execution(self.root,selection,steps)))

    def test_current_dispatch_rejects_raw_inspection_failure_and_coverage_loss(self):
        from lifecycle_impact import inspect_current_visual_execution
        from dataclasses import replace
        d,selection,steps,adapters=self.guard_visual_receipts()
        adapters['guard-current-react']=replace(adapters['guard-current-react'],recompute=lambda *args:['actual raw mismatch'])
        with patch.dict(d.ADAPTERS,adapters,clear=True):
            errors=inspect_current_visual_execution(self.root,selection,steps)
        self.assertTrue(any('actual raw mismatch' in e for e in errors))
        self.assertTrue(any('actual real_react state coverage missing' in e for e in errors))

    def test_release_of_historical_phase_still_requires_complete_current(self):
        self.m['id']=self.c['phases']['SR-2']['change_id']
        self.m['requirements'][0]['checks']+=self.c['phases']['SR-6']['checks']
        self.m['product_runtime_checks']=[self.c['runtime_check']]
        p=plan_impact(self.root,self.m,[],'release',contract=self.c)
        self.assertEqual(p['visual_execution']['policy'],'current_design_actual_execution')
        self.assertTrue(p['visual_execution']['full_current_closure'])
        self.assertNotIn('product-ui-capture-v4',p['required_checks'])

    def fresh_guard_visual_receipts(self):
        from datetime import datetime, timezone, timedelta
        d,selection,steps,adapters=self.guard_visual_receipts()
        lower=datetime.now(timezone.utc)-timedelta(seconds=10)
        context={'execution_nonce':'d'*32,'started_at':lower.isoformat()}
        for step in steps.values():
            ex=step['execution']
            self.rewrite_guard_evidence(ex,'invocation',lambda r:r.update(lifecycle_execution_nonce=context['execution_nonce'], lifecycle_started_at=context['started_at'],
                                       started_at=(lower+timedelta(seconds=1)).isoformat(),finished_at=(lower+timedelta(seconds=3)).isoformat()))
            self.rewrite_guard_evidence(ex,'receipt',lambda r:r.update(started_at=(lower+timedelta(seconds=1)).isoformat(),
                                       finished_at=(lower+timedelta(seconds=2)).isoformat()))
        return d,selection,steps,adapters,context

    def test_whole_old_current_group_cannot_replay_into_new_lifecycle_invocation(self):
        from lifecycle_impact import inspect_current_visual_execution
        d,selection,steps,adapters,context=self.fresh_guard_visual_receipts()
        with patch.dict(d.ADAPTERS,adapters,clear=True):
            self.assertEqual(inspect_current_visual_execution(self.root,selection,steps,execution_context=context),[])
            # All old inner hashes/nonces remain internally consistent. The
            # private current lifecycle invocation is the independent anchor.
            for step in steps.values():
                ex=step['execution']
                self.rewrite_guard_evidence(ex,'invocation',lambda r:r.update(started_at='2020-01-01T00:00:00+00:00',finished_at='2020-01-01T00:00:02+00:00'))
                self.rewrite_guard_evidence(ex,'receipt',lambda r:r.update(started_at='2020-01-01T00:00:00+00:00',finished_at='2020-01-01T00:00:01+00:00'))
            errors=inspect_current_visual_execution(self.root,selection,steps,execution_context=context)
            self.assertTrue(any('actual lifecycle execution interval' in e for e in errors))

    def test_current_group_must_bind_fresh_private_cohort_nonce_and_same_runtime_package(self):
        from lifecycle_impact import inspect_current_visual_execution
        d,selection,steps,adapters,context=self.fresh_guard_visual_receipts()
        with patch.dict(d.ADAPTERS,adapters,clear=True):
            package_sha=hashlib.sha256((self.root/'.build/guard-package.zip').read_bytes()).hexdigest()
            self.assertEqual(inspect_current_visual_execution(self.root,selection,steps,execution_context=context,expected_artifact_sha256=package_sha),[])
            errors=inspect_current_visual_execution(self.root,selection,steps,execution_context=context,expected_artifact_sha256='0'*64)
            self.assertTrue(any('four-game runtime use different packages' in e for e in errors))
            old_context={**context,'execution_nonce':'e'*32}
            self.assertTrue(any('fresh lifecycle invocation' in e for e in inspect_current_visual_execution(self.root,selection,steps,execution_context=old_context)))

    def test_final_runtime_is_recomputed_and_not_accepted_from_green_flags(self):
        from lifecycle_impact import inspect_final_runtime
        from datetime import datetime, timezone, timedelta
        now=datetime.now(timezone.utc);source=tree_hash(self.root);nonce='f'*32
        directory=self.root/'.build/runtime-envelope';directory.mkdir(parents=True)
        report={'status':'passed','errors':[],'cases':[{'id':'guard-only'}], 'source_tree_sha256':source,
                'artifact_sha256':'a'*64,'main_pid':42,'fixture_substitution':False,
                'product_packaged_native_game_tests_executed':True,'execution_nonce':nonce,'executed_at_ms':int(now.timestamp()*1000)}
        rp=directory/'verification.json';rp.write_text(json.dumps(report));(directory/'collection.json').write_text(json.dumps({'started_at_ms':int((now-timedelta(seconds=2)).timestamp()*1000)}))
        ex={'status':'passed','errors':[],'exit_code':0,'adapter_id':'product-packaged-stability-tests','execution_nonce':nonce,
            'source_tree_sha256':source,'artifact_sha256':'a'*64,'runtime_receipt':str(rp.relative_to(self.root)),
            'runtime_receipt_sha256':hashlib.sha256(rp.read_bytes()).hexdigest()}
        ep=directory/'execution.json';ep.write_text(json.dumps(ex));ex.update(receipt=str(ep.relative_to(self.root)),receipt_sha256=hashlib.sha256(ep.read_bytes()).hexdigest())
        index={'product-packaged-stability-tests':{'status':'passed','execution':ex}}
        plan={'source_tree_sha256':source,'execution_context':{'execution_nonce':'d'*32,'started_at':(now-timedelta(seconds=3)).isoformat()}}
        # Isolate the cross-check. This fixture never grants game acceptance.
        with patch('runtime_stability_evidence.validate_run',return_value=report) as recompute:
            artifact,errors=inspect_final_runtime(self.root,plan,index)
            self.assertEqual((artifact,errors),('a'*64,[]));recompute.assert_called_once_with(self.root,directory.resolve(),nonce,source)
            bad={**report,'artifact_sha256':'b'*64};recompute.return_value=bad
            self.assertTrue(inspect_final_runtime(self.root,plan,index)[1])
            recompute.return_value=report
            (directory/'collection.json').write_text(json.dumps({'started_at_ms':0}))
            self.assertTrue(any('actual lifecycle invocation' in e for e in inspect_final_runtime(self.root,plan,index)[1]))
        ex['runtime_receipt_sha256']='0'*64
        self.assertTrue(inspect_final_runtime(self.root,plan,index)[1])
