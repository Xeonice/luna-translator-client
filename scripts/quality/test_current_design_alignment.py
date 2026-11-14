"""Guard mutation tests only. No fixture here is a product/reference executor."""
import ast
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import current_design_alignment as c
import design_authority as authority


class CurrentDesignAlignmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source_paths = set(authority.current_approval_inputs(c.ROOT)) | {
            c.all_pages.CONTRACT,
            c.all_pages.MODEL_CONFIG_LINEAGE,
            'docs/product/first-product-scope.md',
            'docs/product/model-configuration-prd.md',
            'docs/product/game-library-window-acceptance.csv',
            'docs/product/reading-window-session-acceptance.csv',
            'docs/product/model-configuration-acceptance.csv',
            c.CONTRACT, 'docs/architecture/engineering-components.csv',
            'docs/development/current-product-test-coverage.json',
            *(path for state in c.STATES for path in state['component_paths']),
        }
        from current_general_visual_authority import source_inputs
        cls.source_paths.update(source_inputs(c.ROOT))
        cls.source_paths.update(path.relative_to(c.ROOT).as_posix()
                                for path in (c.ROOT / 'apps/desktop/src').rglob('*') if path.is_file())

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for relative in self.source_paths:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(c.ROOT / relative, path)
        self.data = c._read(self.root / c.CONTRACT)
        self.registered_before = dict(c.ADAPTERS)

    def write(self, relative, value):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value if isinstance(value, str) else json.dumps(value, ensure_ascii=False) + '\n')
        return path

    def save(self):
        self.write(c.CONTRACT, self.data)

    def reference(self, path):
        return {'path': path.relative_to(self.root).as_posix(), 'sha256': c._digest(path)}

    def fixture_adapter(self, layer='real_react', states=c.STATE_IDS, inspect=None, recompute=None):
        # Temporary code capability only, patched for guard tests. It is never
        # installed in the production registry and does not render a product.
        def execute(*args):
            raise AssertionError('read-only guard assessment must not execute a runner')
        def default_inspect(*args):
            return []
        def default_recompute(*args):
            return []
        return c.Adapter(c.TARGET, layer, states, ('guard-fixture-never-a-production-command',),
                         execute, inspect or default_inspect, recompute or default_recompute)

    def receipt(self, adapter):
        return {'schema_version': 1, 'adapter_id': 'guard-only', 'target_id': c.TARGET,
                'composition_id': c.COMPOSITION_ID,
                'layer': adapter.layer, 'producer_kind': adapter.layer,
                'design_closure_sha256': c.DESIGN_CLOSURE_SHA256,
                'source_tree_sha256': c.source_identity(self.root), 'artifact_sha256': 'a' * 64,
                'command': list(adapter.command), 'status': 'passed', 'errors': [],
                'prototype_substitution': False, 'native_verified': adapter.layer == 'packaged_native',
                'execution_nonce': 'b' * 32, 'started_at': '2026-10-31T00:00:00Z',
                'finished_at': '2026-10-31T00:00:01Z', 'state_ids': sorted(adapter.states),
                'assertion_count': 2, 'skipped_assertions': 0}

    def inspect(self, receipt, adapter):
        with patch.dict(c.ADAPTERS, {'guard-only': adapter}, clear=True):
            return c._inspect_receipt(self.root, receipt, 'a' * 64)

    def test_new_service_overlay_appends_without_changing_original_802_definitions(self):
        original_ids={row['id'] for row in c.all_pages.model_config_lineage(self.root)['original_current_states']}
        prefix='all-page-overlay-modelconfigscreen-5fb902a57e-'
        retained=[row for row in c.STATES if row['id'] in original_ids and not row['id'].startswith(prefix)]
        self.assertEqual(802,len(retained))
        self.assertEqual('727fe58f1eef5436a9c732af0e327e91b756ac2a76a97b7cf6adc70fb9b064f3',hashlib.sha256(json.dumps(retained,sort_keys=True,separators=(',',':')).encode()).hexdigest())
        new_rows=[row for row in c.STATES if row['id'] in original_ids and row['id'].startswith(prefix)]
        self.assertEqual({'normal','disabled','long-text','light','narrow','zoom-200','keyboard-focus-escape'},{row['id'][len(prefix):] for row in new_rows})
        for row in new_rows:
            self.assertEqual(['real_react','packaged_native'],row['required_layers'])
        original=c.all_pages.model_config_lineage(self.root)['original_current_states']
        self.assertEqual(809,len(original));self.assertEqual(original,[row for row in c.STATES if row['id'] in original_ids])
        additions=[row for row in c.STATES if row['id'] not in original_ids]
        self.assertEqual(92,len(additions))
        self.assertEqual(3,sum(row['id'].endswith('-retired-surface-absent') for row in additions))
        self.assertEqual(8,len({row['id'].rsplit('-',1)[0] for row in additions if row['id'].endswith('-normal')}))

    def test_contract_exact_sources_approved_version_captured_pending_and_no_product_claim(self):
        self.assertEqual([], c.validate_contract(self.root))
        from current_design_react_adapter import CHECK_ID, STATE_IDS
        from current_general_visual_adapter import REACT_CHECK, NATIVE_CHECK, STATE_IDS as GENERAL_STATE_IDS
        self.assertEqual({CHECK_ID, REACT_CHECK, NATIVE_CHECK}, set(c.ADAPTERS))
        for general_check, layer in ((REACT_CHECK, 'real_react'), (NATIVE_CHECK, 'packaged_native')):
            adapter = c.ADAPTERS[general_check]
            self.assertEqual(GENERAL_STATE_IDS, adapter.states)
            self.assertEqual(14, len(adapter.states))
            self.assertEqual(layer, adapter.layer)
            self.assertEqual('whole_client', adapter.capture_scope)
            self.assertFalse(adapter.game_execution)
        self.assertEqual(STATE_IDS, c.ADAPTERS[CHECK_ID].states)
        self.assertEqual('real_react', c.ADAPTERS[CHECK_ID].layer)
        self.assertLess(STATE_IDS, c.STATE_IDS)
        self.assertEqual('pending_review', self.data['approval']['candidate_status'])
        self.assertEqual('approved_by_user', authority.current_source_approval(self.root)['status'])
        self.assertEqual({'path': authority.CURRENT_RECORD, 'sha256': authority.CURRENT_RECORD_SHA256},
                         self.data['approval']['user_decision'])
        self.assertEqual(2, self.data['schema_version'])
        self.assertEqual(c.COMPOSITION_ID, self.data['composition_id'])
        self.assertEqual(c.SURFACE_ENTRIES, self.data['surface_entries'])
        self.assertEqual(61, len(self.data['design_sources']))
        self.assertIsNone(self.data['approval']['approved_reference_catalogue'])
        self.assertEqual(c.STATE_IDS, {state['id'] for state in self.data['states']})

    def test_source_approval_clears_only_its_own_gap_and_old_record_is_not_inherited(self):
        gaps = c._approval_gaps(self.root)
        self.assertIn('independently reviewed approved current-target reference catalogue is absent', gaps)
        self.assertIn('General independent whole-client reference catalogue/review missing', gaps)
        (self.root / authority.CURRENT_RECORD).unlink()
        self.assertEqual([], authority.user_approval_gaps(self.root))
        self.assertTrue(any('source-version user approval invalid' in gap for gap in c._approval_gaps(self.root)))
        self.assertTrue(any('reference catalogue' in gap for gap in c._approval_gaps(self.root)))
        self.assertTrue(c.validate_contract(self.root))

    def test_all_page_contract_is_unconditionally_checked_for_tooling(self):
        path = self.root / c.all_pages.CONTRACT
        data = c._read(path)
        data['pages'].pop()
        self.write(c.all_pages.CONTRACT, data)
        self.assertTrue(any('all-page' in gap for gap in c.product_change_gaps(self.root, {'kind': 'tooling'})))

    def test_missing_design_state_remains_a_product_gap_despite_source_approval(self):
        # Preserve the frozen original809 definitions. Exercise source-anchored
        # missing-state refusal on a new unmapped Google dialog state instead.
        self.assertEqual(frozenset(), c.all_pages.missing_design_state_ids(self.root))
        source = c.CANDIDATE + '/settings.js'
        gap = {('overlay:ModelConfigScreen:6da213484c', 'revoke_failed'): {
            'path': source, 'anchor': 'function guardedNavigate(app,change)',
            'quote': "confirmLabel:'保存并切换',cancelLabel:'继续编辑'",
            'reason': 'Guard-only source audit: approved guardedNavigate contains save/discard navigation, no Google revocation failure Sheet.',
        }}
        with patch.dict(c.all_pages.VERIFIED_DESIGN_GAPS, gap, clear=True):
            self.write(c.all_pages.CONTRACT, c.all_pages.expected_contract(self.root))
            self.assertEqual([], c.all_pages.validate_contract(self.root))
            self.assert_missing_and_unmapped_refusal()

    def assert_missing_and_unmapped_refusal(self):
        missing = c.all_pages.missing_design_state_ids(self.root)
        unmapped = c.all_pages.unmapped_design_state_ids(self.root)
        self.assertTrue(missing)
        self.assertTrue(unmapped)
        self.assertFalse(missing & unmapped)
        self.assertEqual(missing | unmapped, c.all_pages.unresolved_design_state_ids(self.root))
        self.assertEqual([], authority.current_user_approval_gaps(self.root))
        self.assertTrue(c._design_state_gaps(self.root, missing))
        mapping_gaps = c._design_state_gaps(self.root, unmapped)
        self.assertTrue(any('needs_mapping' in gap for gap in mapping_gaps))
        self.assertFalse(any('states are missing' in gap for gap in mapping_gaps))
        self.assertEqual([], c._design_state_gaps(self.root, {'reading-current-card'}))
        errors = c.product_change_gaps(self.root, {'kind': 'product'}, mode='release')
        self.assertTrue(any('page design states are missing' in e for e in errors))
        self.assertTrue(any('needs_mapping' in e for e in errors))
        adapter = self.fixture_adapter(states=frozenset({next(iter(missing))}))
        with patch.dict(c.ADAPTERS, {'guard-only': adapter}, clear=True), patch.object(c, '_approval_gaps', return_value=[]):
            result = c.run_current_adapter(self.root, 'guard-only', '.build/would-be-execution')
        self.assertEqual('failed', result['status'])
        self.assertTrue(any('page design states are missing' in e for e in result['errors']))
        self.assertFalse((self.root / '.build/would-be-execution').exists())
        adapter = self.fixture_adapter(states=frozenset({next(iter(unmapped))}))
        with patch.dict(c.ADAPTERS, {'guard-only': adapter}, clear=True), patch.object(c, '_approval_gaps', return_value=[]):
            result = c.run_current_adapter(self.root, 'guard-only', '.build/would-be-unmapped-execution')
        self.assertEqual('failed', result['status'])
        self.assertTrue(any('needs_mapping' in e for e in result['errors']))
        self.assertFalse(any('states are missing' in e for e in result['errors']))
        self.assertFalse((self.root / '.build/would-be-unmapped-execution').exists())
        missing_adapter = self.fixture_adapter(states=frozenset(missing))
        with patch.dict(c.ADAPTERS, {'guard-only': missing_adapter}, clear=True), patch.object(c, '_approval_gaps', return_value=[]):
            result = c.run_current_adapter(self.root, 'guard-only', '.build/would-be-missing-execution')
        self.assertEqual('failed', result['status'])
        self.assertTrue(any('states are missing' in error for error in result['errors']))
        self.assertFalse((self.root / '.build/would-be-missing-execution').exists())

    def test_new_page_sources_use_existing_authorities_without_expanding_old_70(self):
        cases = {case['id'] for case in c._read(self.root / 'docs/development/current-product-test-coverage.json')['cases']}
        self.assertEqual(70, len(cases))
        self.assertTrue(any(set(state['source_case_ids']) - cases for state in c.all_pages.ALL_PAGE_STATES))
        self.assertEqual([], c.validate_contract(self.root))

    def test_shared_actual_imports_expand_across_pages_and_global_tokens_require_all(self):
        for relative in ('apps/desktop/src/ui/primitives/Controls.tsx',
                         'apps/desktop/src/ui/tokens/generated.css'):
            states, _ = c.required_states([relative], root=self.root)
            self.assertTrue(states & c.all_pages.STATE_IDS)
            self.assertIn('shared-select-keyboard-focus-escape', states)
        states, _ = c.required_states(['apps/desktop/src/ui/tokens/generated.css'], root=self.root)
        self.assertEqual(c.STATE_IDS, states)

    def test_known_history_impact_uses_real_dependencies_without_global_fallback(self):
        path = 'apps/desktop/src/features/history/HistoryScreen.tsx'
        states, native = c.required_states([path], root=self.root)
        self.assertFalse(native)
        self.assertIn('all-page-main-history-session-filter', states)
        self.assertIn('all-page-main-history-pagination', states)
        self.assertTrue(c.all_pages.affected_state_ids(self.root, [path]) <= states)
        self.assertLess(len(states), len(c.STATE_IDS))
        self.assertNotIn('configuration-purpose-brand-interface', states)

    def test_transitive_guided_components_keep_original_obligations_and_unknown_is_global(self):
        states, _ = c.required_states(['apps/desktop/src/features/reading/session-presentation.ts'], root=self.root)
        self.assertIn('reading-current-card', states)
        self.assertIn('records-independent-query-page-follow-export', states)
        for path in ('apps/desktop/src/features/unknown/new-screen.tsx',
                     'apps/desktop/src/ui/primitives/Controls.tsx',
                     'apps/desktop/src/bridge/application-port.ts'):
            with self.subTest(path=path):
                self.assertEqual(c.STATE_IDS, c.required_states([path], root=self.root)[0])

    def test_composition_and_surface_routes_cannot_be_rebound_by_json(self):
        original = copy.deepcopy(self.data)
        for field, value in [('schema_version', 1), ('composition_id', 'legacy-guided-only'),
                             ('surface_entries', {'default': {'entry': authority.HISTORY_ROOT + '/index.html', 'query': {}}})]:
            self.data = copy.deepcopy(original)
            self.data[field] = value
            self.save()
            self.assertTrue(c.validate_contract(self.root), field)

    def test_every_state_omission_duplicate_unknown_and_mapping_mutation_rejected(self):
        original = copy.deepcopy(self.data)
        # The dedicated all-page guard checks its complete exact obligation map;
        # preserve per-state mutation coverage for every original guided state.
        indices = list(range(len(c.GUIDED_STATES)))
        indices += [len(c.GUIDED_STATES), len(c.STATES) - 1]
        for index in indices:
            with self.subTest(missing=index):
                self.data = copy.deepcopy(original)
                self.data['states'].pop(index)
                self.save()
                self.assertTrue(c.validate_contract(self.root))
        for field, value in [('id', 'unknown-state'), ('source_case_ids', ['GD-99']),
                             ('component_ids', ['C04']), ('component_paths', [c.SELECT]),
                             ('owners', ['QA']), ('required_layers', ['real_react'])]:
            self.data = copy.deepcopy(original)
            self.data['states'][0][field] = value
            self.save()
            self.assertTrue(c.validate_contract(self.root), field)
        self.data = copy.deepcopy(original)
        self.data['states'].append(copy.deepcopy(self.data['states'][0]))
        self.save()
        self.assertTrue(c.validate_contract(self.root))

    def test_candidate_target_old_v4_and_json_adapter_installation_rejected(self):
        original = copy.deepcopy(self.data)
        for field, value in [('target_id', 'reviewed-v4-original-sources-v1'),
                             ('entry', 'docs/design/first-product-v1/index.html'),
                             ('adapters', {'fake': {'states': sorted(c.STATE_IDS)}})]:
            self.data = copy.deepcopy(original)
            self.data[field] = value
            self.save()
            self.assertTrue(c.validate_contract(self.root), field)

    def test_json_cannot_approve_design_or_self_generated_reference(self):
        for field, value in [('candidate_status', 'approved'), ('user_decision', {'decision': 'approved'}),
                             ('independent_reference_review', {'producer': 'same HTML prototype'}),
                             ('approved_reference_catalogue', {'path': 'acceptance-v4/catalogue.json'})]:
            self.data = c._read(c.ROOT / c.CONTRACT)
            self.data['approval'][field] = value
            self.save()
            self.assertTrue(c.validate_contract(self.root), field)

    def test_source_drift_even_rebound_manifest_and_json_cannot_self_approve(self):
        path = self.root / (c.CANDIDATE + '/guided-config.js')
        path.write_text(path.read_text() + '\n/* guard-only mutation */\n')
        self.assertTrue(c.validate_contract(self.root))
        manifest = c._read(self.root / c.MANIFEST)
        manifest['owned_sources']['guided-config.js'] = c._digest(path)
        self.write(c.MANIFEST, manifest)
        with self.assertRaisesRegex(ValueError, 'approved source bytes changed'):
            c.design_sources(self.root)
        self.data['design_sources'][c.CANDIDATE + '/guided-config.js'] = c._digest(path)
        self.data['design_sources'][c.MANIFEST] = c._digest(self.root / c.MANIFEST)
        self.data['design_closure_sha256'] = c._identity(self.data['design_sources'])
        self.save()
        self.assertTrue(any('approved source bytes changed' in error for error in c.validate_contract(self.root)))

    def test_shared_asset_bytes_and_symlink_changes_rejected(self):
        asset = next(path for path in self.data['design_sources'] if path.endswith('.png'))
        path = self.root / asset
        original = path.read_bytes()
        path.write_bytes(original + b'guard-only mutation')
        self.assertTrue(c.validate_contract(self.root))
        path.unlink()
        external = self.write('.build/external.png', 'guard-only')
        path.symlink_to(external)
        self.assertTrue(c.validate_contract(self.root))

    def test_missing_component_and_authority_source_rejected(self):
        (self.root / c.SELECT).unlink()
        self.assertTrue(c.validate_contract(self.root))
        self.write(c.SELECT, 'guard-only placeholder')
        registry_path = 'docs/development/current-product-test-coverage.json'
        registry = c._read(self.root / registry_path)
        registry['cases'] = [row for row in registry['cases'] if row['id'] != 'GD-12']
        self.write(registry_path, registry)
        self.assertTrue(c.validate_contract(self.root))

    def test_old_v4_alone_does_not_satisfy_product_changes(self):
        metadata = {'kind': 'product', 'product_visual_checks': ['product-ui-capture-v4']}
        errors = c.product_change_gaps(self.root, metadata, changed_paths=[c.READING])
        self.assertTrue(any('no registered current-target real_react' in error for error in errors))
        self.assertFalse(any('source-version user approval invalid' in error for error in errors))
        self.assertTrue(any('reference catalogue' in error for error in errors))
        self.assertTrue(any('approved current-target reference' in error for error in errors))

    def test_empty_requested_checks_allowed_paths_and_claimed_states_cannot_suppress(self):
        metadata = {'kind': 'product', 'requirements': [{'checks': []}],
                    'allowed_paths': ['docs/**'], 'current_alignment_required': False,
                    'required_states': [], 'product_visual_checks': ['product-ui-capture-v4']}
        errors = c.product_change_gaps(self.root, metadata)
        self.assertTrue(any('no registered current-target' in error for error in errors))
        self.assertTrue(all(any(state in error for error in errors) for state in c.STATE_IDS))

    def test_actual_paths_conservative_unknown_desktop_rust_native_and_deletion(self):
        for path in ['apps/desktop/src/unknown-new.tsx', 'apps/desktop/src/theme/new.css',
                     'apps/desktop/src-tauri/src/platform/unknown.rs', 'crates/app-core/src/unknown.rs',
                     'native/new.mm', 'Cargo.lock']:
            states, native = c.required_states([path], root=self.root)
            self.assertEqual(c.STATE_IDS, states, path)
            if path.startswith(('crates/', 'native/', 'apps/desktop/src-tauri/')) or path == 'Cargo.lock':
                self.assertTrue(native)
        states, native = c.required_states([c.RECORDS], root=self.root)
        self.assertIn('records-three-default-collapsed', states)
        self.assertFalse(native)
        self.assertEqual((frozenset(), False), c.required_states(['docs/note.md'], root=self.root))

    def test_actual_pure_tests_and_docs_are_contract_only_even_for_product_kind(self):
        paths=['apps/desktop/tests/visual/capture-reviewed-states.mjs',
               'apps/desktop/src/ui/Select.test.tsx',
               'docs/README.md','README.md','scripts/quality/docs/notes.md','scripts/quality/verify.py',
               'scripts/contracts/compare_product_capture.mjs']
        self.assertEqual((frozenset(),False),c.required_states(paths,root=self.root))
        self.assertEqual([],c.product_change_gaps(self.root,{'kind':'product'},changed_paths=paths))
        self.assertEqual(c.STATE_IDS,c.required_states(['apps/desktop/README.md'],root=self.root)[0])
        self.assertEqual(c.STATE_IDS,c.required_states(['scripts/contracts/compare_product_capture-v2.mjs'],root=self.root)[0])

    def test_native_test_names_cannot_claim_exclusion_without_a_native_dependency_graph(self):
        for path in ['native/tests/core_fixture.cpp','crates/app-core/tests/runtime_fixture.rs',
                     'apps/desktop/src-tauri/tests/window.test.rs','native/docs/window.swift',
                     'crates/app-infra/src/docs/helpers.rs','apps/desktop/src-tauri/src/doc/window.cpp',
                     'native/docs/tests/window.test.hpp','crates/app-infra/src/doc/test_helpers.rs',
                     'unknown-root/doc/runtime.inc','unknown-root/docs/runtime.inl','unknown-root/doc/test_runtime.ipp']:
            with self.subTest(path=path):
                self.assertEqual((c.STATE_IDS,True),c.required_states([path],root=self.root))

    def test_nested_doc_names_cannot_exempt_other_product_resource_roots(self):
        for path in ['packages/foo/docs/asset.js','resources/docs/asset.bin','assets/doc/icon.svg',
                     'runtime/doc/x','apps/desktop/src/docs/new.ts']:
            with self.subTest(path=path):
                self.assertEqual((c.STATE_IDS,False),c.required_states([path],root=self.root))

    def test_production_imported_test_path_uses_actual_dependency_closure(self):
        fixture='apps/desktop/tests/current-imported.test.ts'
        self.write(fixture,'export const productionFixture = 1;\n')
        reading=self.root/c.READING
        reading.write_text(reading.read_text()+"\nimport '../../../tests/current-imported.test';\n")
        states,native=c.required_states([fixture],root=self.root)
        self.assertIn('reading-current-card',states)
        self.assertIn(c.all_pages._state_id('main:session','normal'),states)
        self.assertNotEqual(c.STATE_IDS,states)
        self.assertFalse(native)

    def test_bootstrap_reachable_test_path_preserves_all_current_obligations(self):
        fixture='apps/desktop/tests/bootstrap-imported.test.ts'
        self.write(fixture,'export const productionFixture = 1;\n')
        bootstrap=self.root/'apps/desktop/src/app/bootstrap.tsx'
        bootstrap.write_text(bootstrap.read_text()+"\nimport '../../tests/bootstrap-imported.test';\n")
        self.assertEqual(c.STATE_IDS,c.required_states([fixture],root=self.root)[0])

    def test_code_owned_execution_selection_requires_complete_each_layer(self):
        # Frozen R83 inventory remains an obligation; the independent C35
        # Sheet appends eight states rather than replacing or erasing any ID.
        added=frozenset({
            'all-page-overlay-previewdetails-c72c668099-normal',
            'all-page-overlay-previewdetails-c72c668099-loading',
            'all-page-overlay-previewdetails-c72c668099-error',
            'all-page-overlay-previewdetails-c72c668099-long-text',
            'all-page-overlay-previewdetails-c72c668099-light',
            'all-page-overlay-previewdetails-c72c668099-narrow',
            'all-page-overlay-previewdetails-c72c668099-zoom-200',
            'all-page-overlay-previewdetails-c72c668099-keyboard-focus-escape',
        })
        new_overlay={row['id'] for row in c.all_pages.model_config_lineage(self.root)['original_current_states'] if row['id'].startswith('all-page-overlay-modelconfigscreen-5fb902a57e-')}
        self.assertEqual(7,len(new_overlay))
        original_ids={row['id'] for row in c.all_pages.model_config_lineage(self.root)['original_current_states']}
        self.assertEqual(809,len(original_ids));self.assertLess(original_ids,c.STATE_IDS)
        self.assertTrue(added <= c.STATE_IDS)
        retained=sorted(original_ids-added-new_overlay)
        self.assertEqual(794,len(retained))
        self.assertEqual('6f7df2f0521f5998403c91acce98d4636bc8f33f3f4e75b39222e87b25a89eb7',hashlib.sha256(json.dumps(retained,separators=(',',':')).encode()).hexdigest())
        for row in c.STATES:
            if row['id'] in added:
                self.assertEqual(['real_react','packaged_native'],row['required_layers'])
        checks,gaps=c.registered_execution_checks(c.STATE_IDS)
        from current_design_react_adapter import CHECK_ID, STATE_IDS
        from current_general_visual_adapter import REACT_CHECK, NATIVE_CHECK
        self.assertEqual(frozenset({CHECK_ID, REACT_CHECK, NATIVE_CHECK}),checks)
        self.assertLess(STATE_IDS,c.STATE_IDS)
        self.assertTrue(any('real_react' in error for error in gaps))
        self.assertTrue(any('packaged_native' in error for error in gaps))
        required=frozenset({'reading-current-card','records-three-default-collapsed'})
        registry={name:self.fixture_adapter(layer,frozenset({state}))
                  for layer in c.LAYERS for state in required
                  for name in [layer+'-'+state]}
        with patch.dict(c.ADAPTERS,registry,clear=True):
            checks,gaps=c.registered_execution_checks(required)
            self.assertEqual(frozenset(registry),checks)
            self.assertEqual([],gaps)
            checks,gaps=c.registered_execution_checks(required,{'real_react'})
            self.assertEqual({key for key in registry if key.startswith('real_react-')},checks)
            self.assertEqual([],gaps)
            del c.ADAPTERS['packaged_native-reading-current-card']
            checks,gaps=c.registered_execution_checks(required)
            self.assertTrue(any('packaged_native' in error and 'reading-current-card' in error for error in gaps))
            self.assertFalse(any('no registered current-target real_react' in error for error in gaps))

    def test_execution_selection_rejects_fake_target_layer_callback_and_states(self):
        valid=self.fixture_adapter()
        invalid=[{'target_id':c.TARGET,'layer':'real_react','states':sorted(c.STATE_IDS)},
                 c.Adapter('acceptance-v4',valid.layer,valid.states,valid.command,valid.execute,valid.inspect,valid.recompute),
                 c.Adapter(c.TARGET,'html_prototype',valid.states,valid.command,valid.execute,valid.inspect,valid.recompute),
                 c.Adapter(c.TARGET,{},valid.states,valid.command,valid.execute,valid.inspect,valid.recompute),
                 c.Adapter(c.TARGET,valid.layer,valid.states,valid.command,valid.execute,valid.inspect,valid.inspect),
                 c.Adapter(c.TARGET,valid.layer,frozenset({'unknown-state'}),valid.command,valid.execute,valid.inspect,valid.recompute)]
        for adapter in invalid:
            with self.subTest(adapter=adapter),patch.dict(c.ADAPTERS,{'fake':adapter},clear=True):
                checks,gaps=c.registered_execution_checks(c.STATE_IDS)
                self.assertEqual(frozenset(),checks)
                self.assertTrue(gaps)
                self.assertTrue(c.adapter_capability_errors('fake'))
        for states,layers in [('reading-current-card',None),({'unknown'},None),(c.STATE_IDS,[]),(c.STATE_IDS,{'prototype'})]:
            self.assertTrue(c.registered_execution_checks(states,layers)[1])
        with patch.dict(c.ADAPTERS,{'react':valid},clear=True):
            self.assertTrue(c.adapter_capability_errors('react','packaged_native'))

    def test_path_escape_symlink_and_invalid_mode_fail_closed(self):
        for path in ['../outside.tsx', '/absolute.tsx', 'apps/desktop/../outside', 'apps//desktop/new.tsx']:
            self.assertTrue(c.product_change_gaps(self.root, {'kind': 'product'}, changed_paths=[path]))
        self.assertTrue(c.product_change_gaps(self.root, {'kind': 'product'}, mode='unknown'))
        self.assertTrue(c.product_change_gaps(self.root, {'kind': 'product'}, changed_paths=c.READING))
        link = self.root / 'apps/desktop/src/fake'
        link.symlink_to(self.root / '.build')
        self.assertTrue(c.product_change_gaps(self.root, {'kind': 'product'}, changed_paths=['apps/desktop/src/fake/new.tsx']))

    def test_tooling_contract_can_pass_but_release_never_uses_kind_or_requested_switch(self):
        self.assertEqual([], c.product_change_gaps(self.root, {'kind': 'tooling'}))
        errors = c.product_change_gaps(self.root, {'kind': 'tooling', 'requirements': []}, mode='release', changed_paths=[])
        self.assertTrue(any('packaged_native' in error for error in errors))
        self.assertTrue(all(any(state in error for error in errors) for state in c.STATE_IDS))

    def test_capabilities_require_separate_code_inspect_recompute_not_json(self):
        self.assertTrue(c._adapter_errors({'target_id': c.TARGET, 'states': sorted(c.STATE_IDS)}))
        adapter = self.fixture_adapter()
        self.assertEqual([], c._adapter_errors(adapter))
        copied = c.Adapter(c.TARGET, 'real_react', c.STATE_IDS, adapter.command,
                           adapter.execute, adapter.inspect, adapter.inspect)
        self.assertTrue(any('separate' in error for error in c._adapter_errors(copied)))
        malformed = c.Adapter(c.TARGET, 'real_react', 42, adapter.command,
                              adapter.execute, adapter.inspect, adapter.recompute)
        with patch.dict(c.ADAPTERS, {'guard-only': malformed}, clear=True):
            result = c.run_current_adapter(self.root, 'guard-only', '.build/no-malformed-execution')
        self.assertEqual('failed', result['status'])
        self.assertTrue(any('missing/unknown current state' in e for e in result['errors']))

    def test_missing_state_capability_and_source_component_cannot_satisfy_native(self):
        adapter = self.fixture_adapter(states=frozenset({'reading-current-card'}))
        with patch.dict(c.ADAPTERS, {'guard-only': adapter}, clear=True):
            errors = c.product_change_gaps(self.root, {'kind': 'product', 'product_visual_checks': ['guard-only']}, changed_paths=[c.READING])
            self.assertTrue(any('no registered current-target real_react' in error for error in errors))
        full = self.fixture_adapter()
        with patch.dict(c.ADAPTERS, {'guard-only': full}, clear=True):
            errors = c.product_change_gaps(self.root, {'kind': 'product', 'product_visual_checks': ['guard-only']}, changed_paths=['crates/app-core/src/lib.rs'])
            self.assertTrue(any('packaged_native' in error for error in errors))
            self.assertFalse(any('source-version user approval invalid' in error for error in errors))
            self.assertTrue(any('reference catalogue' in error for error in errors))

    def test_receipt_target_layer_prototype_old_source_old_package_rejected(self):
        adapter = self.fixture_adapter()
        original = self.receipt(adapter)
        for key, value in [('target_id', 'acceptance-v4'), ('composition_id', 'legacy-guided-only'),
                           ('layer', 'packaged_native'),
                           ('producer_kind', 'html_prototype'), ('prototype_substitution', True),
                           ('native_verified', True), ('source_tree_sha256', 'c' * 64),
                           ('artifact_sha256', 'c' * 64), ('design_closure_sha256', 'c' * 64),
                           ('command', ['true'])]:
            receipt = copy.deepcopy(original)
            receipt[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.inspect(receipt, adapter)

    def test_receipt_state_missing_duplicate_unknown_and_zero_skip_nonce_rejected(self):
        adapter = self.fixture_adapter()
        original = self.receipt(adapter)
        for key, value in [('state_ids', []), ('state_ids', original['state_ids'] + [original['state_ids'][0]]),
                           ('state_ids', original['state_ids'] + ['unknown']), ('assertion_count', 0),
                           ('assertion_count', True), ('skipped_assertions', 1), ('skipped_assertions', False),
                           ('execution_nonce', 'old'), ('finished_at', '2026-10-30T00:00:00Z')]:
            receipt = copy.deepcopy(original)
            receipt[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                self.inspect(receipt, adapter)

    def test_independent_inspection_and_recompute_failures_propagate(self):
        calls = []
        def inspect(*args):
            calls.append('inspect')
            return []
        def recompute(*args):
            calls.append('recompute')
            return ['raw PNG/layout evidence differs']
        adapter = self.fixture_adapter(inspect=inspect, recompute=recompute)
        with self.assertRaisesRegex(ValueError, 'raw PNG/layout'):
            self.inspect(self.receipt(adapter), adapter)
        self.assertEqual(['inspect', 'recompute'], calls)

    def test_release_no_package_no_refs_no_current_adapter_is_blocked(self):
        gaps = c.release_gaps(self.root)
        for text in ['reference catalogue', 'final package', 'no registered current-target real_react', 'no registered current-target packaged_native']:
            self.assertTrue(any(text in gap for gap in gaps), text)

    def test_sidecar_old_hash_duplicate_receipt_and_unregistered_prototype_rejected(self):
        artifact = self.write('.build/guard-package', 'guard-only never a real app')
        receipt_path = self.write('.build/receipt.json', {'adapter_id': 'product-ui-capture-v4'})
        sidecar = {'schema_version': 1, 'target_id': c.TARGET, 'composition_id': c.COMPOSITION_ID,
                   'artifact_sha256': c._digest(artifact), 'contract_sha256': c._digest(self.root / c.CONTRACT),
                   'receipts': [self.reference(receipt_path)]}
        sidecar_path = self.write('.build/sidecar.json', sidecar)
        outer = {'current_design_alignment': self.reference(sidecar_path)}
        gaps = c.release_gaps(self.root, outer, '.build/guard-package')
        self.assertTrue(any('code-owned Adapter' in gap for gap in gaps))
        outer['current_design_alignment']['sha256'] = 'c' * 64
        self.assertTrue(any('old-hash' in gap for gap in c.release_gaps(self.root, outer, '.build/guard-package')))

    def test_duplicate_registered_receipts_cannot_fill_native_or_state_coverage(self):
        artifact = self.write('.build/guard-package', 'guard-only never a real app')
        adapter = self.fixture_adapter()
        receipt = self.receipt(adapter)
        receipt['artifact_sha256'] = c._digest(artifact)
        receipt_path = self.write('.build/receipt.json', receipt)
        reference = self.reference(receipt_path)
        sidecar_path = self.write('.build/sidecar.json', {
            'schema_version': 1, 'target_id': c.TARGET, 'composition_id': c.COMPOSITION_ID,
            'artifact_sha256': c._digest(artifact),
            'contract_sha256': c._digest(self.root / c.CONTRACT), 'receipts': [reference, reference]})
        with patch.dict(c.ADAPTERS, {'guard-only': adapter}, clear=True):
            gaps = c.release_gaps(self.root, {'current_design_alignment': self.reference(sidecar_path)}, '.build/guard-package')
        self.assertTrue(any('duplicate alignment receipt' in gap for gap in gaps))

    def test_cli_check_pass_assess_blocked_read_only_and_no_product_claim(self):
        before = {p.relative_to(self.root).as_posix(): c._digest(p) for p in self.root.rglob('*') if p.is_file()}
        env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}
        script = str(c.ROOT / 'scripts/quality/current_design_alignment.py')
        for mode, status, code in [('--check', 'passed', 0), ('--assess', 'blocked', 1)]:
            executed = subprocess.run([sys.executable, script, mode, '--root', str(self.root), '--json'], env=env, capture_output=True, text=True)
            self.assertEqual(code, executed.returncode, executed.stderr)
            report = json.loads(executed.stdout)
            self.assertEqual(status, report['status'])
            self.assertIs(report['product_tests_executed'], False)
            self.assertIs(report['product_alignment_approved'], False)
        after = {p.relative_to(self.root).as_posix(): c._digest(p) for p in self.root.rglob('*') if p.is_file()}
        self.assertEqual(before, after)

    def test_tooling_label_cannot_skip_real_production_diff(self):
        errors = c.product_change_gaps(self.root, {'kind':'tooling'}, changed_paths=[c.HUD])
        self.assertFalse(any('source-version user approval invalid' in error for error in errors))
        self.assertTrue(any('reference catalogue' in error for error in errors))
        self.assertTrue(any('packaged_native' in error for error in errors))

    def test_resources_runtime_root_locks_and_unknown_product_paths_block(self):
        for path in ('resources/language/new-resource.bin', 'runtime/core/LunaHook32.dll',
                     'package.json', 'package-lock.json', 'new-product-package/src/new.ts',
                     'new-product-package/asset.bin','scripts/build_desktop.py'):
            with self.subTest(path=path):
                errors = c.product_change_gaps(self.root, {'kind':'product'}, changed_paths=[path])
                self.assertFalse(any('source-version user approval invalid' in error for error in errors))
                self.assertTrue(any('reference catalogue' in error for error in errors))
                self.assertTrue(any('real_react' in error for error in errors))
                self.assertTrue(any('packaged_native' in error for error in errors))

    def test_real_react_capability_cannot_replace_required_native_layer(self):
        adapter=self.fixture_adapter(states=c.STATE_IDS)
        with patch.dict(c.ADAPTERS, {'guard-only':adapter},clear=True):
            errors=c.product_change_gaps(self.root, {'kind':'product','product_visual_checks':['guard-only']},changed_paths=[c.HUD])
        self.assertFalse(any('no registered current-target real_react' in error for error in errors))
        self.assertTrue(any('no registered current-target packaged_native' in error for error in errors))

    def dispatch_fixture(self, change=None, inspect_change=None, raw_inspect=None, raw_recompute=None, lifecycle_context=None):
        package=self.write('.build/package','guard-only never a product package')
        calls=[]
        def execute(root,context):
            calls.append('execute')
            if change: change(context,package)
            value=self.receipt(adapter)
            value.update(artifact_sha256=context['artifact_sha256'],execution_nonce=context['execution_nonce'],
                         started_at=context['started_at'],finished_at=context['started_at'])
            return value
        def inspect(root,receipt,context):
            calls.append('inspect')
            if inspect_change: inspect_change(receipt,context)
            if raw_inspect: raw_inspect(root)
            return []
        def recompute(root,receipt,context):
            calls.append('recompute')
            if raw_recompute: raw_recompute(root)
            return []
        adapter=c.Adapter(c.TARGET,'real_react',c.STATE_IDS,('guard-fixture-never-a-production-command',),execute,inspect,recompute)
        with patch.dict(c.ADAPTERS,{'guard-only':adapter},clear=True),patch.object(c,'_approval_gaps',return_value=[]),patch.object(c,'_design_state_gaps',return_value=[]):
            result=c.run_current_adapter(self.root,'guard-only','.build/fresh-run','.build/package',lifecycle_context=lifecycle_context)
        return result,calls

    def test_dispatch_calls_actual_code_and_both_independent_inspectors(self):
        result,calls=self.dispatch_fixture()
        self.assertEqual('passed',result['status'],result)
        self.assertEqual(['execute','inspect','recompute'],calls)
        request=json.loads((self.root/'.build/fresh-run/request.json').read_text())
        receipt=json.loads((self.root/result['receipt']).read_text())
        self.assertEqual(request['execution_nonce'],receipt['execution_nonce'])
        self.assertEqual(request['artifact_sha256'],receipt['artifact_sha256'])
        invocation=c._read(c._hashed(self.root,{'path':result['invocation'],'sha256':result['invocation_sha256']}))
        self.assertEqual({**request,'finished_at':invocation['finished_at']},invocation)
        self.assertEqual('guard-only',invocation['adapter_id'])
        self.assertEqual(c.TARGET,invocation['target_id'])
        self.assertEqual(c.COMPOSITION_ID,invocation['composition_id'])
        self.assertEqual(sorted(c.STATE_IDS),invocation['state_ids'])
        self.assertLessEqual(invocation['started_at'],invocation['finished_at'])
        self.assertEqual(self.registered_before,c.ADAPTERS)

    def test_final_invocation_reinspection_rejects_tampering_and_old_context(self):
        result,_=self.dispatch_fixture()
        invocation=c._read(self.root/result['invocation'])
        receipt=c._read(self.root/result['receipt'])
        adapter=self.fixture_adapter()
        with patch.dict(c.ADAPTERS,{'guard-only':adapter},clear=True):
            self.assertEqual(('real_react',c.STATE_IDS),c._inspect_receipt(self.root,receipt,invocation['artifact_sha256'],invocation))
            for key,value in [('execution_nonce','a'*32),('adapter_id','another-adapter'),
                              ('target_id','acceptance-v4'),('composition_id','old-composition'),
                              ('layer','packaged_native'),('state_ids',[]),('command',['fake']),
                              ('source_tree_sha256','c'*64),('artifact_sha256','a'*64),
                              ('artifact_path','.build/no-package'),('output','docs/output'),
                              ('started_at','2099-01-01T00:00:00+00:00')]:
                with self.subTest(key=key):
                    changed=copy.deepcopy(invocation); changed[key]=value
                    with self.assertRaises((ValueError,OSError)):
                        c._inspect_receipt(self.root,receipt,invocation['artifact_sha256'],changed)
        path=self.root/result['invocation']
        path.write_text(path.read_text()+'\n')
        with self.assertRaises(ValueError):
            c._hashed(self.root,{'path':result['invocation'],'sha256':result['invocation_sha256']})

    def test_dispatch_cannot_overwrite_executor_installed_final_invocation(self):
        old='old context from executor; never dispatcher evidence\n'
        result,calls=self.dispatch_fixture(lambda context,package:self.write('.build/fresh-run/invocation.json',old))
        self.assertEqual('failed',result['status'])
        self.assertNotIn('invocation',result)
        self.assertEqual(old,(self.root/'.build/fresh-run/invocation.json').read_text())
        self.assertEqual(['execute','inspect','recompute'],calls)

    def test_dispatch_binds_fresh_typed_lifecycle_context_without_rewriting_it(self):
        lifecycle={'execution_nonce':'c'*32,'started_at':c.datetime.now(c.timezone.utc).isoformat()}
        original=copy.deepcopy(lifecycle)
        result,calls=self.dispatch_fixture(lifecycle_context=lifecycle)
        self.assertEqual('passed',result['status'],result)
        invocation=c._read(self.root/result['invocation'])
        self.assertEqual(lifecycle['execution_nonce'],invocation['lifecycle_execution_nonce'])
        self.assertEqual(lifecycle['started_at'],invocation['lifecycle_started_at'])
        self.assertEqual(original,lifecycle)
        self.assertEqual(['execute','inspect','recompute'],calls)

    def test_dispatch_rejects_untyped_future_or_executor_rewritten_lifecycle_context(self):
        now=c.datetime.now(c.timezone.utc).isoformat()
        invalid=[[],{}, {'execution_nonce':True,'started_at':now},
                 {'execution_nonce':'c'*31,'started_at':now},
                 {'execution_nonce':'c'*32,'started_at':None},
                 {'execution_nonce':'c'*32,'started_at':'2026-11-04T00:00:00'},
                 {'execution_nonce':'c'*32,'started_at':'2099-01-01T00:00:00+00:00'}]
        for context in invalid:
            with self.subTest(context=context):
                result,calls=self.dispatch_fixture(lifecycle_context=context)
                self.assertEqual('failed',result['status'])
                self.assertEqual([],calls)
                self.assertNotIn('invocation',result)
                self.assertFalse((self.root/'.build/fresh-run').exists())
        lifecycle={'execution_nonce':'c'*32,'started_at':now}
        for field,value in [('lifecycle_execution_nonce','d'*32),('lifecycle_started_at','2000-01-01T00:00:00+00:00')]:
            with self.subTest(field=field):
                result,calls=self.dispatch_fixture(change=lambda context,package:context.__setitem__(field,value),lifecycle_context=lifecycle)
                self.assertEqual('failed',result['status'])
                self.assertEqual(['execute'],calls)
                self.assertTrue(any('private dispatcher' in error for error in result['errors']))
                self.assertNotIn('invocation',result)
                shutil.rmtree(self.root/'.build/fresh-run')

    def test_dispatch_rejects_executor_rewriting_private_nonce_package_and_interval(self):
        for field,value in [('execution_nonce','b'*32),('artifact_sha256','a'*64),('started_at','2000-01-01T00:00:00+00:00')]:
            with self.subTest(field=field):
                result,calls=self.dispatch_fixture(lambda context,package:context.__setitem__(field,value))
                self.assertEqual('failed',result['status'])
                self.assertTrue(any('private dispatcher' in e for e in result['errors']))
                self.assertEqual(['execute'],calls)
                self.assertFalse((self.root/'.build/fresh-run/invocation.json').exists())
                shutil.rmtree(self.root/'.build/fresh-run')

    def test_dispatch_rejects_mutating_inspector_inputs(self):
        for change in [lambda receipt,context:receipt.__setitem__('assertion_count',999),
                       lambda receipt,context:context.__setitem__('execution_nonce','a'*32)]:
            result,calls=self.dispatch_fixture(inspect_change=change)
            self.assertEqual('failed',result['status'])
            self.assertTrue(any('without mutating' in e for e in result['errors']))
            self.assertEqual(['execute','inspect'],calls)
            shutil.rmtree(self.root/'.build/fresh-run')

    def test_dispatch_rejects_package_mutation_and_old_output(self):
        result,calls=self.dispatch_fixture(lambda context,package:package.write_text('changed during guarded execution'))
        self.assertEqual('failed',result['status'])
        self.assertTrue(any('changed during' in e for e in result['errors']))
        adapter=self.fixture_adapter()
        with patch.dict(c.ADAPTERS,{'guard-only':adapter},clear=True),patch.object(c,'_approval_gaps',return_value=[]),patch.object(c,'_design_state_gaps',return_value=[]):
            result=c.run_current_adapter(self.root,'guard-only','.build/fresh-run','.build/package')
        self.assertTrue(any('already exists' in e for e in result['errors']))

    def test_dispatch_independent_checks_cannot_change_source_or_package(self):
        for hook in ['raw_inspect','raw_recompute']:
            for relative in ['.build/package',c.SELECT]:
                with self.subTest(hook=hook,relative=relative):
                    original=(self.root/relative).read_bytes() if (self.root/relative).exists() else b'guard-only'
                    result,calls=self.dispatch_fixture(**{hook:lambda root,p=relative:(root/p).write_text('changed during independent check')})
                    self.assertEqual('failed',result['status'])
                    self.assertTrue(any('changed during independent' in e for e in result['errors']))
                    (self.root/relative).write_bytes(original)
                    shutil.rmtree(self.root/'.build/fresh-run')

    def test_assessment_rejects_self_consistent_empty_gaps_against_actual_current_facts(self):
        report=c.assessment_report(self.root)
        self.assertEqual('blocked',report['status'])
        self.assertTrue(report['release_gaps'])
        self.assertEqual(c.COMPOSITION_ID,report['composition_id'])
        self.assertEqual([],c.assessment_errors(self.root,report,1))
        forged=copy.deepcopy(report)
        forged.update(status='passed',release_gaps=[],composition_id='acceptance-v4')
        self.assertTrue(c.assessment_errors(self.root,forged,0))
        with patch.dict(c.ADAPTERS,{'guard-only':self.fixture_adapter()},clear=True):
            self.assertTrue(c.assessment_errors(self.root,report,1))
        self.assertEqual(self.registered_before,c.ADAPTERS)

    def test_assessment_typed_fields_adapters_authority_and_exit_cannot_be_forged(self):
        report=c.assessment_report(self.root)
        approval=copy.deepcopy(report['effective_design_approval']);approval['record']['sha256']='a'*64
        cases=[('schema_version',True),('required_states',True),('required_states',0),
               ('target_id','acceptance-v4'),('composition_id','acceptance-v4'),
               ('registered_adapters',['fake-current-adapter']),('registered_adapters',{}),
               ('release_gaps',[]),('release_gaps',[{}]),('errors',{}),('status',{}),
               ('scope','actual_product_execution'),('effective_design_approval',approval),
               ('effective_design_approval',[]),('product_tests_executed',True),
               ('product_alignment_approved',True)]
        # The expected facts came from the real source above; reuse that fixed
        # fact snapshot to exercise every typed-field adversary cheaply.
        with patch.object(c,'assessment_report',return_value=report):
            for key,value in cases:
                with self.subTest(key=key,value=value):
                    changed=copy.deepcopy(report);changed[key]=value
                    self.assertTrue(c.assessment_errors(self.root,changed,1))
            for code in [0,True,2,'1']:
                with self.subTest(code=code): self.assertTrue(c.assessment_errors(self.root,report,code))

    def test_assessment_uses_actual_gap_outcome_without_permanent_blocked_requirement(self):
        # Guard-only substitution for future independently verified evidence,
        # never a production adapter or a current acceptance claim.
        with patch.object(c,'release_gaps',return_value=[]):
            report=c.assessment_report(self.root)
            self.assertEqual('passed',report['status'])
            self.assertEqual([],c.assessment_errors(self.root,report,0))
            self.assertIs(report['product_tests_executed'],False)
            self.assertIs(report['product_alignment_approved'],False)

    def test_actual_verify_readiness_branch_rejects_false_pass_and_malformed_reports(self):
        tree=ast.parse((c.ROOT/'scripts/quality/verify.py').read_text())
        branch=next(node for node in ast.walk(tree) if isinstance(node,ast.If)
                    and isinstance(node.test,ast.Compare) and isinstance(node.test.left,ast.Constant)
                    and node.test.left.value=='current-design-readiness-assessment')
        actual=c.assessment_report(self.root)
        forged=copy.deepcopy(actual);forged.update(status='passed',release_gaps=[],composition_id='acceptance-v4')
        for report,code in [(actual,1),(forged,0),([],0)]:
            with self.subTest(report_type=type(report).__name__,code=code):
                observed=[]
                namespace={'ROOT':self.root,'sys':sys,'json':json,
                           'run':lambda *args,**kwargs:(json.dumps(report),code),
                           'checks':lambda identity,errors,**kwargs:observed.append((identity,errors))}
                exec(compile(ast.fix_missing_locations(ast.Module(body=copy.deepcopy(branch.body),type_ignores=[])),
                             'actual-verify-current-design-readiness','exec'),namespace)
                self.assertEqual(1,len(observed))
                self.assertEqual('current-design-readiness-contract',observed[0][0])
                self.assertEqual(report is not actual,bool(observed[0][1]))


    def test_design_state_expected_failures_are_nonempty_failed_gaps(self):
        for error in (OSError('source unavailable'), ValueError('AST surface/mapping mismatch'),
                      KeyError('pages'), TypeError('invalid source rows')):
            with self.subTest(error=type(error).__name__), patch.object(c.all_pages, 'expected_contract', side_effect=error):
                gaps = c._design_state_gaps(self.root, c.STATE_IDS)
            self.assertTrue(any('current design-state assessment failed' in value
                                and type(error).__name__ in value and str(error) in value for value in gaps))

    def test_injected_global_ast_failure_remains_failed_without_aborting_product_preflight(self):
        from current_general_visual_adapter import REACT_CHECK, NATIVE_CHECK
        metadata = {'kind': 'product', 'product_visual_checks': [REACT_CHECK, NATIVE_CHECK]}
        # The boundary may fail independently of the present inventory. Never
        # require this historical production defect to persist after repair.
        failure = ValueError('AST surface/mapping mismatch; injected guard-only source boundary')
        with patch.object(c.all_pages, 'expected_contract', side_effect=failure):
            strict_errors = c.validate_contract(self.root)
            self.assertTrue(any(str(failure) in value for value in strict_errors))
            with patch.object(c, '_approval_gaps', return_value=['independent references still required']):
                errors = c.product_change_gaps(self.root, metadata, mode='release')
        self.assertTrue(set(strict_errors) <= set(errors))
        self.assertIn('independent references still required', errors)
        self.assertTrue(any('current design-state assessment failed' in value
                            and 'AST surface/mapping mismatch' in value for value in errors))
        original_ids={row['id'] for row in c.all_pages.model_config_lineage(self.root)['original_current_states']}
        self.assertEqual(809,len(original_ids));self.assertLess(original_ids,c.STATE_IDS)
        for identity, layer in ((REACT_CHECK, 'real_react'), (NATIVE_CHECK, 'packaged_native')):
            missing = c.STATE_IDS - c.ADAPTERS[identity].states
            expected = ('no registered current-target ' + layer
                        + ' executor with independent inspection/recompute for states: ' + ','.join(sorted(missing)))
            self.assertIn(expected, errors)
            self.assertEqual(795,len(original_ids-c.ADAPTERS[identity].states))
            self.assertEqual(len(c.STATE_IDS)-14,len(missing))
            self.assertFalse(c.ADAPTERS[identity].game_execution)


if __name__ == '__main__':
    unittest.main()
