"""Approval identity guard tests only; no reference or product is executed."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import design_authority as a


class DesignAuthorityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.paths = a.approval_inputs(a.ROOT)

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for relative in self.paths:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(a.ROOT / relative, path)
        self.data = json.loads((self.root / a.RECORD).read_text())

    def write(self, data):
        path = self.root / a.RECORD
        path.write_text(data if isinstance(data, str) else json.dumps(data, ensure_ascii=False) + '\n')
        return path

    def test_current_whole_source_approval_preserves_snapshot_and_runtime_boundaries(self):
        result = a.source_approval(self.root)
        self.assertEqual([], result['errors'])
        self.assertEqual('approved_by_user', result['status'])
        self.assertEqual('pending_review', result['captured_candidate_status'])
        self.assertEqual(57, len(self.data['design_sources']))
        self.assertEqual(a.DESIGN_CLOSURE_SHA256, a._identity(self.data['design_sources']))
        self.assertFalse(result['product_tests_executed'])
        self.assertFalse(result['product_alignment_approved'])
        self.assertIn('per_state_reference_png_approval', result['not_granted'])
        manifest = self.root / ('docs/design/first-product-v1/candidates/' + a.TARGET + '/manifest.json')
        self.assertEqual('pending_review', json.loads(manifest.read_text())['status'])

    def test_missing_record_old_approval_and_source_presence_cannot_substitute(self):
        (self.root / a.RECORD).unlink()
        old = self.root / 'docs/development/design-approval-20261019.json'
        shutil.copyfile(a.ROOT / 'docs/development/design-approval-20261019.json', old)
        self.assertTrue(a.user_approval_gaps(self.root))
        self.assertEqual('blocked', a.source_approval(self.root)['status'])

    def test_raw_record_bytes_are_pinned_even_for_semantically_identical_json(self):
        self.write(self.data)
        self.assertTrue(any('raw hash' in e for e in a.user_approval_gaps(self.root)))

    def test_wrong_target_date_scope_message_basis_parent_and_flags_rejected(self):
        for key, value in [('target_id', 'acceptance-v4'), ('entry', 'docs/design/first-product-v1/index.html'),
                           ('decision_date', '2026-10-19'), ('decision_basis', 'derived_from_screenshot'),
                           ('approved_scope', 'all_future_designs'), ('user_message', 'looks good'),
                           ('parent_baseline', {'id': 'FP1-20261030-R75', 'sha256': 'a' * 64}),
                           ('captured_candidate_status', 'approved_by_user'),
                           ('not_granted', []), ('schema_version', True)]:
            with self.subTest(key=key):
                data = copy.deepcopy(self.data)
                data[key] = value
                path = self.write(data)
                # Even hypothetical record-byte reauthorization does not erase
                # independently fixed decision scope/source/runtime boundaries.
                with patch.object(a, 'RECORD_SHA256', a._digest(path)):
                    self.assertTrue(any('identity differs' in e for e in a.user_approval_gaps(self.root)))

    def test_original_source_map_cannot_lose_add_or_rebind_entries(self):
        for mode in ('missing', 'extra', 'rebound'):
            data = copy.deepcopy(self.data)
            source = next(iter(data['design_sources']))
            if mode == 'missing':
                del data['design_sources'][source]
            elif mode == 'extra':
                data['design_sources']['docs/extra-approved-source.html'] = 'a' * 64
            else:
                data['design_sources'][source] = 'a' * 64
            path = self.write(data)
            with patch.object(a, 'RECORD_SHA256', a._digest(path)):
                self.assertTrue(any('original complete 57-source map' in e for e in a.user_approval_gaps(self.root)))

    def test_current_source_drift_missing_and_symlink_rejected(self):
        relative = 'docs/design/first-product-v1/candidates/' + a.TARGET + '/guided-config.js'
        path = self.root / relative
        original = path.read_bytes()
        path.write_bytes(original + b'\nchanged after approval\n')
        self.assertTrue(any('source bytes changed' in e for e in a.user_approval_gaps(self.root)))
        path.unlink()
        self.assertTrue(a.user_approval_gaps(self.root))
        path.symlink_to(a.ROOT / relative)
        self.assertTrue(any('symlink' in e for e in a.user_approval_gaps(self.root)))

    def test_captured_manifest_cannot_be_promoted_to_approved(self):
        path = self.root / ('docs/design/first-product-v1/candidates/' + a.TARGET + '/manifest.json')
        data = json.loads(path.read_text())
        data['status'] = 'approved_by_user'
        path.write_text(json.dumps(data))
        self.assertTrue(any('source bytes changed' in e for e in a.user_approval_gaps(self.root)))

    def test_duplicate_key_invalid_record_and_escape_rejected(self):
        self.write('{"schema_version":1,"schema_version":1}')
        self.assertTrue(any('duplicate approval' in e for e in a.user_approval_gaps(self.root)))
        for relative in ('../outside', '/absolute', 'docs//bad', 'docs/../bad', 'docs/a\\b', 'docs/b?x'):
            with self.subTest(relative=relative), self.assertRaises(ValueError):
                a._path(self.root, relative)

    def test_assessment_is_read_only(self):
        before = {p: a._digest(self.root / p) for p in self.paths}
        self.assertEqual([], a.user_approval_gaps(self.root))
        self.assertEqual(before, {p: a._digest(self.root / p) for p in self.paths})


class CurrentDesignAuthorityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.paths = a.current_approval_inputs(a.ROOT)

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for relative in self.paths:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(a.ROOT / relative, target)
        self.data = json.loads((self.root / a.CURRENT_RECORD).read_text())

    def write_record(self, data):
        path = self.root / a.CURRENT_RECORD
        path.write_text(data if isinstance(data, str) else json.dumps(data, ensure_ascii=False) + '\n')
        return path

    def test_complete_composition_approves_exact_sources_and_preserves_legacy_boundaries(self):
        result = a.current_source_approval(self.root)
        self.assertEqual([], result['errors'])
        self.assertEqual('approved_by_user', result['status'])
        self.assertEqual(a.TARGET, result['target_id'])
        self.assertEqual(a.CURRENT_COMPOSITION_ID, result['composition_id'])
        self.assertEqual(61, len(result['design_sources']))
        self.assertEqual(a.CURRENT_DESIGN_CLOSURE_SHA256, a._identity(result['design_sources']))
        self.assertEqual(64, len(a.current_approval_inputs(self.root)))
        self.assertEqual(57, len(a._record(self.root)['design_sources']))
        self.assertEqual('approved_by_user', a.source_approval(self.root)['status'])
        self.assertEqual(a.DESIGN_CLOSURE_SHA256, a.source_approval(self.root)['design_closure_sha256'])
        self.assertEqual(a.CURRENT_ROUTES, result['routes'])
        self.assertFalse(result['product_tests_executed'])
        self.assertFalse(result['product_alignment_approved'])
        self.assertIn('independently_reviewed_reference_catalogue', result['not_granted'])
        for candidate in (a.TARGET, a.HISTORY_TARGET):
            manifest = self.root / ('docs/design/first-product-v1/candidates/' + candidate + '/manifest.json')
            self.assertEqual('pending_review', json.loads(manifest.read_text())['status'])

    def test_legacy_approval_cannot_substitute_for_missing_complete_decision(self):
        (self.root / a.CURRENT_RECORD).unlink()
        self.assertEqual([], a.user_approval_gaps(self.root))
        self.assertEqual('blocked', a.current_source_approval(self.root)['status'])
        self.assertTrue(a.current_user_approval_gaps(self.root))

    def test_complete_raw_bytes_are_pinned_even_for_equivalent_json(self):
        self.write_record(self.data)
        self.assertTrue(any('raw hash' in error for error in a.current_user_approval_gaps(self.root)))

    def test_complete_decision_identity_routes_dependencies_and_boundaries_are_code_owned(self):
        mutations = [
            ('composition_id', 'another-composition'), ('target_id', a.HISTORY_TARGET),
            ('entry', a.HISTORY_ROOT + '/index.html'), ('schema_version', True),
            ('decision_basis', 'derived_from_screenshot'), ('user_message', 'looks good'),
            ('approved_scope', 'all_future_designs'), ('design_source_count', 57),
            ('parent_baseline', {'id': 'FP1-20261031-R78', 'sha256': 'a' * 64}),
            ('routes', {'default': {'entry': a.HISTORY_ROOT + '/index.html', 'query': {}}}),
            ('runtime_dependencies', {}), ('authority_dependencies', {}),
            ('captured_candidate_statuses', {a.TARGET: 'approved_by_user'}), ('not_granted', []),
        ]
        for key, value in mutations:
            with self.subTest(key=key):
                data = copy.deepcopy(self.data)
                data[key] = value
                path = self.write_record(data)
                with patch.object(a, 'CURRENT_RECORD_SHA256', a._digest(path)):
                    self.assertTrue(any('identity differs' in error for error in a.current_user_approval_gaps(self.root)))

    def test_complete_source_map_cannot_omit_add_or_rebind_history(self):
        for mode in ('omit', 'extra', 'rebind'):
            with self.subTest(mode=mode):
                data = copy.deepcopy(self.data)
                source = a.HISTORY_ROOT + '/history.css'
                if mode == 'omit':
                    del data['design_sources'][source]
                elif mode == 'extra':
                    data['design_sources']['docs/extra-approved.html'] = 'a' * 64
                else:
                    data['design_sources'][source] = 'a' * 64
                path = self.write_record(data)
                with patch.object(a, 'CURRENT_RECORD_SHA256', a._digest(path)):
                    self.assertTrue(any('guided57 plus exact R79 history4' in error for error in a.current_user_approval_gaps(self.root)))

    def test_history_css_and_manifest_rebinding_cannot_inherit_complete_approval(self):
        css = self.root / a.HISTORY_ROOT / 'history.css'
        css.write_bytes(css.read_bytes() + b'\nchanged after complete source approval\n')
        manifest = self.root / a.HISTORY_ROOT / 'manifest.json'
        data = json.loads(manifest.read_text())
        data['owned_sources']['history.css'] = a._digest(css)
        manifest.write_text(json.dumps(data))
        self.assertEqual([], a.user_approval_gaps(self.root))
        self.assertTrue(any('complete approved source bytes changed' in error for error in a.current_user_approval_gaps(self.root)))

    def test_wallpaper_runtime_drift_is_bound_without_expanding_the_legacy57(self):
        relative = next(iter(a.CURRENT_RUNTIME_DEPENDENCIES))
        (self.root / relative).write_bytes(b'changed runtime asset')
        self.assertEqual([], a.user_approval_gaps(self.root))
        self.assertTrue(any('dependency bytes changed' in error for error in a.current_user_approval_gaps(self.root)))

    def test_legacy_record_is_still_a_required_complete_authority_dependency(self):
        path = self.root / a.RECORD
        path.write_bytes(path.read_bytes() + b' ')
        self.assertTrue(a.current_user_approval_gaps(self.root))

    def test_complete_unknown_product_acceptance_field_is_rejected(self):
        data = copy.deepcopy(self.data)
        data['product_alignment_approved'] = True
        path = self.write_record(data)
        with patch.object(a, 'CURRENT_RECORD_SHA256', a._digest(path)):
            self.assertTrue(any('record fields' in error for error in a.current_user_approval_gaps(self.root)))

    def test_current_input_or_record_symlink_and_duplicate_json_rejected(self):
        path = self.root / a.HISTORY_ROOT / 'history.css'
        path.unlink()
        path.symlink_to(a.ROOT / a.HISTORY_ROOT / 'history.css')
        self.assertTrue(any('symlink' in error for error in a.current_user_approval_gaps(self.root)))
        self.write_record('{"schema_version":1,"schema_version":1}')
        self.assertTrue(any('duplicate approval' in error for error in a.current_user_approval_gaps(self.root)))

    def test_current_result_does_not_share_mutable_authority_state(self):
        result = a.current_source_approval(self.root)
        result['routes']['main:history']['query']['scenario'] = 'settings'
        result['design_sources'].clear()
        result['runtime_dependencies'].clear()
        again = a.current_source_approval(self.root)
        self.assertEqual('history', again['routes']['main:history']['query']['scenario'])
        self.assertEqual(61, len(again['design_sources']))
        self.assertEqual(1, len(again['runtime_dependencies']))
        self.assertEqual([], again['errors'])

    def test_current_cli_and_read_only_assessment_bind_the_real_record(self):
        before = {p: a._digest(self.root / p) for p in self.paths}
        result = subprocess.run([sys.executable, str(Path(a.__file__).resolve()), '--check', '--current',
                                 '--root', str(self.root), '--json'], text=True, capture_output=True, check=False)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual(a.current_source_approval(self.root), json.loads(result.stdout))
        self.assertEqual([], a.current_user_approval_gaps(self.root))
        self.assertEqual(before, {p: a._digest(self.root / p) for p in self.paths})


if __name__ == '__main__':
    unittest.main()
