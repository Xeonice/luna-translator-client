"""Regression evidence guards: actual geometry/identity, never a pixel waiver."""
import copy
import unittest

import cp01_component_evidence as gate


def rectangle(left, top, width, height):
    return {'left': left, 'top': top, 'right': left + width, 'bottom': top + height, 'width': width, 'height': height}


class CP01EvidenceTests(unittest.TestCase):
    def geometry(self, variant='dark'):
        theme, width, height, zoom = gate.VARIANTS[variant]
        return {'theme': theme, 'zoom': zoom, 'viewport': {'width': width, 'height': height, 'deviceScaleFactor': 1},
                'outer': [0, 0], 'current': rectangle(20, 20, 500, 300), 'source': rectangle(40, 40, 460, 100),
                'preview': rectangle(20, 350, 282, 250),
                'tokens': [{'tag': 'RUBY', 'text': '放課後', 'rects': [rectangle(60, 60, 80, 60)]}],
                'accessibility': [{**rectangle(40 + i * 10, 400, 8, 25), 'name': name, 'hit': True,
                                   'viewportWidth': width, 'viewportHeight': height} for i, name in enumerate(sorted(gate.ACTIONS))]}

    def hidden(self):
        a = {'sequence': '92', 'text': 'Original record', 'offset': -13.21875, 'rasterOffset': -13,
             'highWatermark': '140', 'viewport': {'width': 1160, 'height': 900, 'deviceScaleFactor': 1}}
        b = {**copy.deepcopy(a), 'offset': -13.15625}
        return [{'hiddenBy': hidden, 'anchorBefore': copy.deepcopy(a), 'anchorAfter': copy.deepcopy(b),
                 'rawOffsetDelta': .0625, 'scopeFacts': {'freshReads': 1,
                 'scopes': [{'snapshotId': 'original', 'highWatermark': '140', 'signature': 'body:query'}]}}
                for hidden in ('collapse', 'main_navigation')]

    def test_all_variants_measured_without_zoom_exception(self):
        for variant in gate.VARIANTS:
            with self.subTest(variant=variant): self.assertEqual(gate.validate_geometry(self.geometry(variant), variant), [])

    def test_reviewed_real_zoom_failure_is_rejected(self):
        g = self.geometry('zoom200')
        g.update(current=rectangle(434, 340, 84, 2782), source=rectangle(476, 382, 0, 2340),
                 preview=rectangle(560, 340, 564, 600),
                 tokens=[{'tag': 'BUTTON', 'text': '放課後', 'rects': [rectangle(476, 382, 112, 78)]}])
        self.assertTrue(gate.validate_geometry(g, 'zoom200'))

    def test_preview_overlap_rejected(self):
        g = self.geometry(); g['preview'] = rectangle(300, 30, 282, 250)
        self.assertIn('current reading/preview clipping or overlap: dark', gate.validate_geometry(g, 'dark'))

    def test_token_overflow_rejected(self):
        g = self.geometry(); g['tokens'][0]['rects'] = [rectangle(450, 50, 100, 30)]
        self.assertTrue(any('token/ruby' in s for s in gate.validate_geometry(g, 'dark')))

    def test_zero_source_width_rejected(self):
        g = self.geometry(); g['source'] = rectangle(40, 40, 0, 100)
        self.assertTrue(gate.validate_geometry(g, 'dark'))

    def test_zero_preview_dimensions_rejected(self):
        for width, height in ((0, 250), (282, 0), (0, 0)):
            g = self.geometry(); g['preview'] = rectangle(0, 0, width, height)
            self.assertTrue(gate.validate_geometry(g, 'dark'))

    def test_zero_current_source_or_token_area_rejected(self):
        for key in ('current', 'source'):
            g = self.geometry(); g[key] = rectangle(20, 20, 500, 0)
            self.assertTrue(gate.validate_geometry(g, 'dark'))
        g = self.geometry(); g['tokens'][0]['rects'] = [rectangle(60, 60, 0, 0)]
        self.assertTrue(gate.validate_geometry(g, 'dark'))

    def test_inconsistent_rectangle_dimensions_rejected(self):
        for key in ('current', 'source', 'preview'):
            g = self.geometry(); g[key]['right'] = g[key]['left']
            with self.assertRaises(ValueError): gate.validate_geometry(g, 'dark')

    def test_actual_zoom_and_viewport_required(self):
        for key in ('zoom', 'viewport'):
            g = self.geometry('zoom200'); g[key] = 1 if key == 'zoom' else {'width': 2000, 'height': 900, 'deviceScaleFactor': 1}
            self.assertTrue(gate.validate_geometry(g, 'zoom200'))

    def test_actions_cannot_be_obscured(self):
        g = self.geometry(); g['accessibility'][0]['hit'] = False
        self.assertTrue(any('obscured' in s for s in gate.validate_geometry(g, 'dark')))

    def test_missing_or_duplicate_action_identity_rejected(self):
        retired_label = self.geometry()['accessibility']
        next(action for action in retired_label if action['name'] == '暂停')['name'] = '暂停预览'
        for actions in ([self.geometry()['accessibility'][0]] * 12, self.geometry()['accessibility'][:-1], retired_label):
            g = self.geometry(); g['accessibility'] = actions
            with self.assertRaises(ValueError): gate.validate_geometry(g, 'dark')

    def test_missing_preview_details_entry_rejected(self):
        g = self.geometry()
        g['accessibility'] = [action for action in g['accessibility'] if action['name'] != '预览详情']
        with self.assertRaises(ValueError): gate.validate_geometry(g, 'dark')

    def test_boolean_device_scale_rejected(self):
        g = self.geometry(); g['viewport']['deviceScaleFactor'] = True
        self.assertTrue(gate.validate_geometry(g, 'dark'))

    def test_actions_must_be_within_actual_viewport(self):
        g = self.geometry(); g['accessibility'][0].update(rectangle(1150, 100, 40, 30))
        self.assertTrue(gate.validate_geometry(g, 'dark'))

    def test_zero_or_missing_tokens_rejected(self):
        for tokens in ([], [{'rects': []}]):
            g = self.geometry();g['tokens'] = tokens
            with self.assertRaises(ValueError): gate.validate_geometry(g, 'dark')

    def test_nonfinite_and_boolean_geometry_rejected(self):
        for width in (float('nan'), float('inf'), True):
            g = self.geometry();g['current']['width'] = width
            with self.assertRaises(ValueError): gate.validate_geometry(g, 'dark')

    def test_raw_fraction_kept_but_zero_physical_shift(self):
        rows = self.hidden();self.assertNotEqual(rows[0]['anchorBefore']['offset'], rows[0]['anchorAfter']['offset'])
        self.assertEqual(gate.validate_hidden(rows), [])

    def test_same_offset_different_record_review_failure_rejected(self):
        rows = self.hidden(); rows[1]['anchorAfter'] = {**rows[1]['anchorBefore'], 'sequence': '42'};rows[1]['rawOffsetDelta'] = 0
        self.assertTrue(any('identity' in s for s in gate.validate_hidden(rows)))

    def test_hidden_navigation_cannot_be_omitted(self):
        self.assertTrue(gate.validate_hidden(self.hidden()[:1]))

    def test_forged_raster_position_rejected(self):
        rows = self.hidden();rows[0]['anchorAfter']['offset'] = -12.0;rows[0]['rawOffsetDelta'] = 1.21875
        self.assertTrue(any('raw offset' in s for s in gate.validate_hidden(rows)))

    def test_hidden_range_replacement_rejected(self):
        rows = self.hidden();rows[0]['scopeFacts']['freshReads'] = 2
        self.assertTrue(any('reset' in s for s in gate.validate_hidden(rows)))

    def test_hidden_cursor_scope_change_rejected(self):
        rows = self.hidden();rows[0]['scopeFacts']['scopes'].append({'snapshotId': 'different'})
        self.assertTrue(gate.validate_hidden(rows))

    def test_raw_delta_tamper_rejected(self):
        rows = self.hidden();rows[0]['rawOffsetDelta'] = 0
        self.assertTrue(gate.validate_hidden(rows))

    def test_missing_or_invalid_anchor_identity_rejected(self):
        for key, value in (('sequence', None), ('sequence', True), ('sequence', '1e2'),
                           ('highWatermark', None), ('highWatermark', '0'),
                           ('text', ''), ('viewport', {'deviceScaleFactor': 1})):
            rows = self.hidden()
            for anchor in ('anchorBefore', 'anchorAfter'):
                if value is None: rows[0][anchor].pop(key)
                else: rows[0][anchor][key] = value
            with self.assertRaises(ValueError): gate.validate_hidden(rows)

    def test_empty_or_incomplete_scope_identity_rejected(self):
        for scope in ({}, {'snapshotId': 'x', 'highWatermark': '140'},
                      {'snapshotId': '', 'highWatermark': '140', 'signature': 'x'},
                      {'snapshotId': 'x', 'highWatermark': '139', 'signature': 'x'},
                      {'snapshotId': 'x', 'highWatermark': True, 'signature': 'x'}):
            rows = self.hidden(); rows[0]['scopeFacts']['scopes'] = [scope]
            self.assertTrue(gate.validate_hidden(rows))


if __name__ == '__main__': unittest.main()
