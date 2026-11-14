"""C35 source adoption guards; fixtures never become product/reference proof."""
import copy
from pathlib import Path
import shutil
import tempfile
import unittest

import current_preview_authority as p
from current_reference_catalogue import load

ROOT = Path(__file__).resolve().parents[2]


class CurrentPreviewAuthorityTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup); self.root = Path(tmp.name).resolve()
        self.record = load(ROOT / p.RECORD)
        paths = [p.RECORD, *self.record['approved_sources'], self.record['independent_review']['path']]
        for relative in paths:
            dest = self.root / relative; dest.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(ROOT / relative, dest)
        self.parent = {'status': 'approved_by_user', 'errors': []}

    def assess(self): return p._assess(self.root, self.parent)

    def test_exact_adoption_is_component_source_only(self):
        result = self.assess(); self.assertEqual('approved_by_user', result['status'], result)
        self.assertEqual(5, result['approved_source_count']); self.assertEqual(61, result['parent_design_source_count'])
        for key in ('reference_approval_granted', 'product_alignment_approved', 'native_verified'): self.assertIs(result[key], False)
        self.assertEqual('pending_review', load(self.root / (p.PREFIX + 'manifest.json'))['status'])

    def test_record_tamper_cannot_rebind_approval_or_add_capability(self):
        file = self.root / p.RECORD; original = file.read_bytes()
        for suffix in (b'\n', b' ', b'fake Native approved'):
            with self.subTest(suffix=suffix):
                file.write_bytes(original + suffix); self.assertEqual('failed', self.assess()['status'])
        file.write_bytes(original)

    def test_every_frozen_source_byte_and_length_are_required(self):
        for relative in self.record['approved_sources']:
            with self.subTest(source=relative):
                file = self.root / relative; original = file.read_bytes(); file.write_bytes(original + b'\n')
                result = self.assess(); self.assertEqual('failed', result['status']); self.assertTrue(any('source bytes' in e for e in result['errors']))
                file.write_bytes(original)

    def test_parent_failure_review_tamper_and_symlink_fail_closed(self):
        self.parent['status'] = 'failed'; self.assertEqual('failed', self.assess()['status']); self.parent['status'] = 'approved_by_user'
        review = self.root / self.record['independent_review']['path']; data = review.read_bytes(); review.write_bytes(data + b'\n')
        self.assertEqual('failed', self.assess()['status']); review.write_bytes(data)
        file = self.root / (p.PREFIX + 'preview-contract.js'); original = file.read_bytes(); file.unlink()
        other = self.root / 'copy.js'; other.write_bytes(original); file.symlink_to(other)
        self.assertEqual('failed', self.assess()['status'])

    def test_unrelated_cropped_references_do_not_inherit_c35_scope(self):
        for sid in p.UNCHANGED_REFERENCES:
            self.assertFalse(p.affects_preview_target({'id': sid, 'component_ids': ['C35']}))
        base = {'id': 'all-page-main-session-body-expanded', 'component_ids': ['C35'], 'target': {'selector': 'details[data-flat-session-section="reading"]'}}
        self.assertFalse(p.affects_preview_target(base))
        full = {**base, 'target': {'selector': '#main-window'}}; self.assertTrue(p.affects_preview_target(full))
        self.assertTrue(p.affects_preview_target({'id': 'all-page-overlay-previewdetails-normal', 'component_ids': ['C35']}))

    def test_parent_recipe_cannot_silently_satisfy_adopted_preview_target(self):
        from unittest.mock import patch
        row = {'id': 'preview-permission-lifecycle', 'component_ids': ['C35'], 'design_recipe': [{'action': 'navigate', 'path': 'old-parent/index.html'}]}
        with patch.object(p, 'assess', return_value={'errors': [], 'status': 'approved_by_user'}):
            gaps = p.source_mapping_gaps(self.root, [row], {row['id']})
            self.assertTrue(any(row['id'] in e and 'source-state recipes' in e for e in gaps))
            # A recipe itself grants no product/reference/native success. The
            # actual capture and current adapters must execute it separately.
            fresh = copy.deepcopy(row); fresh['design_recipe'][0]['path'] = p.PREFIX + 'index.html'
            self.assertEqual([], p.source_mapping_gaps(self.root, [fresh], {fresh['id']}))

    def test_actual_full_window_and_capture_steps_cannot_hide_behind_other_component_labels(self):
        import all_page_design_coverage as pages
        rows = {r['id']: r for page in pages.expected_contract(ROOT)['pages'] for r in page['states']}
        for sid in ('all-page-window-main-normal', 'all-page-window-settings-return-main-focus',
                    'all-page-subpage-capturesheet-step-select-normal'):
            with self.subTest(state=sid): self.assertTrue(p.affects_preview_target(rows[sid]))
        from current_reference_catalogue import authoritative_context
        descriptors = authoritative_context(ROOT)['descriptors']
        for sid in p.UNCHANGED_REFERENCES: self.assertIn(sid, descriptors)
        self.assertNotIn('all-page-window-main-normal', descriptors)


if __name__ == '__main__': unittest.main()
