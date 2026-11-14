"""Reference-authority mutation guards, not reference or product executors."""
import copy
from datetime import datetime
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import current_reference_catalogue as r


def png(width, height):
    def chunk(kind, data): return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress((b'\x00' + b'\x20\x30\x40\xff' * width) * height)) + chunk(b'IEND', b''))


class CurrentReferenceCatalogueTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup); self.root = Path(tmp.name)
        self.pack = self.root / '.build/reference-fixture'; self.pack.mkdir(parents=True)
        self.write('fixture-source.html', '<p>Guard fixture, not current approved source.</p>')
        self.write(r.RUNNER, '// Guard fixture, never registered as a product runner.\n')
        self.row = {'id': 'fixture-normal', 'scenario': 'guard', 'source_case_ids': ['FP-01'], 'state': 'normal',
                    'mapping_status': 'mapped_recipe', 'design_recipe': [{'action': 'set_viewport', 'width': 16, 'height': 12}, {'action': 'navigate', 'path': 'fixture-source.html', 'query': {'scenario': 'guard', 'theme': 'dark'}}, {'action': 'assert_visible', 'selector': 'p'}],
                    'target': {'selector': 'p', 'visibility': 'required_visible', 'container': 'main'},
                    'fixture': {'kind': 'page_local_design_simulation'}, 'variant_measurements': None}
        other = {**copy.deepcopy(self.row), 'id': 'fixture-other'}
        self.context = {'target_id': 'fixture-target', 'composition_id': 'fixture-composition', 'design_closure_sha256': 'a' * 64,
                        'design_sources': {'fixture-source.html': self.sha(self.root / 'fixture-source.html')}, 'runtime_assets': {},
                        'known_states': {'fixture-normal', 'fixture-other'}, 'descriptors': {'fixture-normal': self.row, 'fixture-other': other},
                        'generator_sha256': self.sha(self.root / r.RUNNER)}
        self.request = {'schema_version': 1, 'kind': 'current-design-reference-request/v1',
                        **{k: self.context[k] for k in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets')},
                        'authors': ['guard-author'], 'execution_nonce': 'b' * 32, 'clock_iso': '2026-10-18T10:24:08.000Z',
                        'generator': {'path': r.RUNNER, 'sha256': self.context['generator_sha256']},
                        'state_descriptors': [{'state_id': self.row['id'], 'descriptor_json': r.canonical(self.row), 'descriptor_sha256': r.identity(self.row)}]}
        self.pack_write('request.json', self.request)
        for name, data in [('target.png', png(4, 3)), ('full.png', png(16, 12))]: (self.pack / name).write_bytes(data)
        self.geometry = {'viewport': {'width': 16, 'height': 12, 'deviceScaleFactor': 1},
                         'target_box': {'x': 1, 'y': 1, 'width': 4, 'height': 3},
                         'clip': {'document_scroll': {'x': 0, 'y': 0}}}
        self.pack_write('geometry.json', self.geometry)
        self.pack_write('assertions.json', [{'status': 'passed', 'step_index': 2, 'action': 'assert_visible', 'selector': 'p', 'actual': True}])
        self.pack_write('source-dom.json', {'url': 'http://127.0.0.1:8123/fixture-source.html?scenario=guard&theme=dark', 'selector': 'p', 'document_html': '<html><p>Guard</p></html>', 'target_html': '<p>Guard</p>'})
        artifacts = {k: self.artifact(v) for k, v in [('target_png', 'target.png'), ('full_png', 'full.png'), ('geometry', 'geometry.json'), ('assertions', 'assertions.json')]}
        self.result = {'state_id': self.row['id'], 'descriptor_sha256': r.identity(self.row), 'status': 'passed', 'errors': [],
                       'assertion_count': 1, 'skipped_assertions': 0, 'artifacts': artifacts, 'pixel_sizes': {'target_png': [4, 3], 'full_png': [16, 12]}}
        self.result['source_dom'] = self.artifact('source-dom.json')
        self.result['actions'] = [
            {'index': 0, 'phase': 'recipe', 'step': self.row['design_recipe'][0], 'status': 'passed', 'actual': {'viewport': {'width': 16, 'height': 12}}},
            {'index': 1, 'phase': 'recipe', 'step': self.row['design_recipe'][1], 'status': 'passed', 'actual': {'url': 'http://127.0.0.1:8123/fixture-source.html?scenario=guard&theme=dark', 'status': 200}},
            {'index': 2, 'phase': 'recipe', 'step': self.row['design_recipe'][2], 'status': 'passed', 'actual': True}]
        self.result['post_capture_consistency'] = [{'status': 'passed', 'step_index': 2, 'action': 'assert_visible', 'selector': 'p', 'actual': True}]
        self.inputs = {'request_descriptor': self.request['state_descriptors'][0], 'descriptor': self.row,
                       'clock_iso': self.request['clock_iso'], 'device_scale_factor': 1,
                       'original_source_identity': self.context['design_sources']}
        self.pack_write('inputs.json', self.inputs); self.result['inputs'] = self.artifact('inputs.json')
        self.generation = {'schema_version': 1, 'kind': 'current-design-reference-generation/v1',
                           **{k: self.request[k] for k in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets', 'authors', 'execution_nonce')},
                           'status': 'passed', 'collection_complete': True, 'prototype_reference_only': True,
                           'product_alignment_approved': False, 'native_verified': False,
                           'started_at': '2026-11-04T00:00:00Z', 'finished_at': '2026-11-04T00:00:01Z',
                           'workspace_root': str(self.root), 'original_output': str(self.pack), 'original_request': str(self.pack / 'request.json'),
                           'command': ['/guard/bin/node', str(self.root / r.RUNNER), '--request', str(self.pack / 'request.json'), '--output', str(self.pack)],
                           'request': self.ref(self.pack / 'request.json', self.pack),
                           'sources_before': self.context['design_sources'], 'sources_after': self.context['design_sources'],
                           'page_errors': [], 'console_errors': [], 'request_errors': [], 'fatal_errors': [],
                           'external_requests': [], 'source_drift': [], 'results': [self.result]}
        self.entry = {'state_id': self.row['id'], 'descriptor_sha256': r.identity(self.row), **{k: artifacts[k] for k in ('target_png', 'full_png', 'geometry', 'assertions')}}
        self.catalogue = {'schema_version': 1, 'kind': 'current-design-reference-catalogue/v1',
                          **{k: self.request[k] for k in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets', 'authors')},
                          'pack_root': '.build/reference-fixture', 'generation': None, 'entries': [self.entry]}
        self.review = {'schema_version': 1, 'kind': 'independent-current-reference-review/v1',
                       **{k: self.context[k] for k in ('target_id', 'composition_id', 'design_closure_sha256')},
                       'status': 'approved', 'independent_non_author_review': True, 'reviewer_id': 'guard-independent-reviewer',
                       'author_ids': ['guard-author'], 'reviewed_at': '2026-11-04T00:00:02Z', 'catalogue': None,
                       'source_approval_extended': False, 'product_alignment_approved': False, 'native_verified': False,
                       'reviewed_entries': {self.row['id']: r.identity(self.entry)}}
        self.save()

    @staticmethod
    def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
    def write(self, name, value):
        p = self.root / name; p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, allow_nan=False) + '\n'); return p
    def pack_write(self, name, value): return self.write('.build/reference-fixture/' + name, value)
    def ref(self, p, base): return {'path': p.relative_to(base).as_posix(), 'sha256': self.sha(p)}
    def artifact(self, name):
        p = self.pack / name; return {**self.ref(p, self.pack), 'bytes': p.stat().st_size}
    def save(self):
        self.pack_write('verification.json', self.generation)
        self.catalogue['generation'] = self.ref(self.pack / 'verification.json', self.pack)
        cp = self.write(r.CATALOGUE, self.catalogue); self.review['catalogue'] = self.ref(cp, self.root)
        self.write(r.REVIEW, self.review)
    def assess(self, required=None): return r._validate(self.root, set(required or {'fixture-normal'}), self.context)
    def rejected(self, fragment):
        result = self.assess(); self.assertEqual('failed', result['status']); self.assertTrue(any(fragment in e for e in result['errors']), result)

    def test_reviewed_fixture_is_reference_only_not_native_or_product(self):
        value = self.assess(); self.assertEqual('passed', value['status'], value)
        self.assertEqual(['fixture-normal'], value['approved_state_ids'])
        self.assertIs(value['native_verified'], False); self.assertIs(value['product_alignment_approved'], False)

    def test_partial_catalogue_cannot_grant_other_states(self):
        value = self.assess({'fixture-normal', 'fixture-other'}); self.assertEqual('blocked', value['status'])
        self.assertIn('fixture-other', value['gaps'][0])

    def test_missing_catalogue_remains_original_absence(self):
        (self.root / r.CATALOGUE).unlink()
        result = r.assess(self.root)
        self.assertIn(r.ABSENT, result['gaps'])
        self.assertEqual('blocked', result['status'])
        self.assertEqual([], result['approved_state_ids'])
        self.assertNotEqual('passed', result['assessments']['general']['status'])
        self.assertEqual([], result['assessments']['general']['approved_state_ids'])

    def test_missing_or_pending_review_never_approves(self):
        (self.root / r.REVIEW).unlink(); self.assertEqual('blocked', self.assess()['status'])
        self.review['status'] = 'pending_review'; self.save(); self.assertEqual('blocked', self.assess()['status'])

    def test_same_author_false_independence_and_missing_authors_rejected(self):
        for key, value in [('reviewer_id', 'guard-author'), ('independent_non_author_review', False), ('author_ids', [])]:
            with self.subTest(key=key):
                old = self.review[key]; self.review[key] = value; self.save(); self.rejected('independent'); self.review[key] = old

    def test_source_and_native_approval_cannot_be_added(self):
        for key in ('source_approval_extended', 'product_alignment_approved', 'native_verified'):
            with self.subTest(key=key):
                self.review[key] = True; self.save(); self.rejected('capability'); self.review[key] = False

    def test_old_or_mixed_source_and_composition_rejected(self):
        for key in ('target_id', 'composition_id', 'design_closure_sha256'):
            with self.subTest(key=key):
                old = self.catalogue[key]; self.catalogue[key] = 'other'; self.save(); self.rejected('authority'); self.catalogue[key] = old

    def test_raw_source_and_generator_drift_rejected(self):
        source = self.root / 'fixture-source.html'; old = source.read_bytes(); source.write_bytes(b'changed'); self.rejected('source bytes'); source.write_bytes(old)
        (self.root / r.RUNNER).write_text('different generator'); self.rejected('generator')

    def test_recipe_and_input_forgery_rejected_even_when_rehashed(self):
        self.request['state_descriptors'][0]['descriptor_json'] = r.canonical({**self.row, 'scenario': 'fake'})
        self.pack_write('request.json', self.request); self.generation['request'] = self.ref(self.pack / 'request.json', self.pack); self.save(); self.rejected('descriptor')

    def test_source_container_selector_and_recipe_changes_invalidate_old_reference(self):
        self.assertIn('container',r.descriptor(self.row)['target'])
        original=copy.deepcopy(self.context['descriptors'])
        mutations=[lambda d:d['target'].update(container='settings'),
                   lambda d:d['target'].update(selector='article'),
                   lambda d:d['design_recipe'].append({'action':'assert_absent','selector':'aside'})]
        for mutate in mutations:
            with self.subTest(mutation=mutations.index(mutate)):
                self.context['descriptors']=copy.deepcopy(original)
                mutate(self.context['descriptors']['fixture-normal'])
                self.rejected('descriptor')
        self.context['descriptors']=original
        self.assertEqual('passed',self.assess()['status'])

    def test_omitted_duplicate_unknown_state_rejected(self):
        for rows in ([], [self.result, self.result], [{**self.result, 'state_id': 'unknown'}]):
            with self.subTest(rows=len(rows)):
                old = self.generation['results']; self.generation['results'] = rows; self.save(); self.rejected('state'); self.generation['results'] = old

    def test_failed_or_zero_skipped_source_assertions_never_make_references(self):
        for key, value in [('assertion_count', 0), ('assertion_count', True), ('skipped_assertions', 1), ('errors', ['failed'])]:
            with self.subTest(key=key):
                old = self.result[key]; self.result[key] = value; self.save(); self.rejected('positive'); self.result[key] = old

    def test_generation_failure_cannot_be_hidden_or_published(self):
        self.result['status'] = 'failed'; self.save(); self.rejected('overall status')
        self.generation['status'] = 'failed'; self.save(); self.rejected('positive')

    def test_frame_tamper_and_corrupt_png_after_rehash_rejected(self):
        p = self.pack / 'target.png'; original = p.read_bytes(); p.write_bytes(b'bad'); self.rejected('hash')
        self.entry['target_png'] = self.result['artifacts']['target_png'] = self.artifact('target.png')
        self.review['reviewed_entries'][self.row['id']] = r.identity(self.entry); self.save(); self.rejected('PNG')
        p.write_bytes(original)

    def test_viewport_and_geometry_lies_rejected(self):
        self.geometry['viewport']['width'] = 17; self.pack_write('geometry.json', self.geometry)
        self.entry['geometry'] = self.result['artifacts']['geometry'] = self.artifact('geometry.json')
        self.review['reviewed_entries'][self.row['id']] = r.identity(self.entry); self.save(); self.rejected('geometry')

    def test_forged_actual_assertion_transcript_rejected(self):
        self.pack_write('assertions.json', [{'status': 'failed', 'action': 'assert_visible'}])
        self.entry['assertions'] = self.result['artifacts']['assertions'] = self.artifact('assertions.json')
        self.review['reviewed_entries'][self.row['id']] = r.identity(self.entry); self.save(); self.rejected('transcript')

    def test_passed_boolean_cannot_replace_raw_failed_observation(self):
        self.pack_write('assertions.json', [{'status': 'passed', 'step_index': 2, 'action': 'assert_visible', 'selector': 'p', 'actual': False}])
        self.entry['assertions'] = self.result['artifacts']['assertions'] = self.artifact('assertions.json')
        self.review['reviewed_entries'][self.row['id']] = r.identity(self.entry); self.save(); self.rejected('transcript')

    def test_post_capture_failure_or_omission_cannot_hide_behind_passed(self):
        original = copy.deepcopy(self.result['post_capture_consistency'])
        for value in (None, [], [{**original[0], 'status': 'failed', 'actual': False}],
                      [{**original[0], 'status': 'passed', 'actual': False}],
                      [{**original[0], 'step_index': True}]):
            with self.subTest(value=value):
                self.result['post_capture_consistency'] = value; self.save(); self.rejected('post-capture')
        self.result['post_capture_consistency'] = original

    def test_action_failure_omission_reorder_or_false_result_cannot_hide_behind_passed(self):
        original = copy.deepcopy(self.result['actions'])
        for value in (None, [], original[:-1], list(reversed(original)),
                      [{**original[0], 'status': 'failed'}, *original[1:]],
                      [original[0], {**original[1], 'actual': {'url': 'http://127.0.0.1:8123/wrong.html', 'status': 200}}, original[2]],
                      [*original[:2], {**original[2], 'actual': False}]):
            with self.subTest(value=value):
                self.result['actions'] = value; self.save(); self.rejected('action transcript')
        self.result['actions'] = original

    def test_inputs_cannot_be_missing_corrupt_or_rehashed_different(self):
        original = copy.deepcopy(self.inputs)
        for key, value in [('clock_iso', '2026-10-18T11:24:08.000Z'), ('device_scale_factor', True),
                           ('descriptor', {**self.row, 'scenario': 'wrong'}),
                           ('request_descriptor', {}), ('original_source_identity', {})]:
            with self.subTest(key=key):
                self.inputs = {**original, key: value}; self.pack_write('inputs.json', self.inputs)
                self.result['inputs'] = self.artifact('inputs.json'); self.save(); self.rejected('state input')
        self.inputs = original; self.pack_write('inputs.json', original); self.result['inputs'] = self.artifact('inputs.json')
        (self.pack / 'inputs.json').unlink(); self.save(); self.rejected('hash')

    def test_explicit_runtime_error_arrays_override_passed_summary(self):
        for key in ('page_errors', 'console_errors', 'request_errors', 'fatal_errors', 'external_requests', 'source_drift'):
            with self.subTest(key=key):
                self.generation[key] = [{'error': 'actual runtime failure'}]; self.save(); self.rejected('runtime violation')
                self.generation[key] = []

    def test_final_navigation_folds_scenario_changes_and_still_rejects_wrong_dom(self):
        step = {'action': 'navigate_scenario', 'scenario': 'guard-next'}
        self.row['design_recipe'].insert(2, step)
        descriptor = self.request['state_descriptors'][0]
        descriptor.update(descriptor_json=r.canonical(self.row), descriptor_sha256=r.identity(self.row))
        self.pack_write('request.json', self.request); self.generation['request'] = self.ref(self.pack / 'request.json', self.pack)
        self.result['descriptor_sha256'] = self.entry['descriptor_sha256'] = r.identity(self.row)
        actions = self.result['actions']; actions[-1]['index'] = 3
        url = 'http://127.0.0.1:8123/fixture-source.html?scenario=guard-next&theme=dark'
        actions.insert(2, {'index': 2, 'phase': 'recipe', 'step': step, 'status': 'passed', 'actual': {'url': url, 'status': 200}})
        assertion = {**self.result['post_capture_consistency'][0], 'step_index': 3}
        self.result['post_capture_consistency'] = [assertion]; self.pack_write('assertions.json', [assertion])
        self.result['artifacts']['assertions'] = self.entry['assertions'] = self.artifact('assertions.json')
        self.pack_write('inputs.json', self.inputs); self.result['inputs'] = self.artifact('inputs.json')
        dom = json.loads((self.pack / 'source-dom.json').read_text()); dom['url'] = url
        self.pack_write('source-dom.json', dom); self.result['source_dom'] = self.artifact('source-dom.json')
        self.review['reviewed_entries'][self.row['id']] = r.identity(self.entry); self.save()
        self.assertEqual('passed', self.assess()['status'], self.assess())
        for bad in (url.replace('guard-next', 'guard'), url.replace('/fixture-source.html', '/prefix/fixture-source.html'), url + '&extra=undeclared'):
            with self.subTest(url=bad):
                dom['url'] = bad; self.pack_write('source-dom.json', dom)
                self.result['source_dom'] = self.artifact('source-dom.json'); self.save(); self.rejected('DOM/route')

    def test_structure_only_png_path_keeps_crc_stream_and_filter_checks(self):
        from reading_alignment_preflight import png_pixels
        data = png(4, 3)
        self.assertEqual((4, 3, None), png_pixels(data, decode_pixels=False))
        self.assertEqual(4 * 3 * 4, len(png_pixels(data)[2]))
        corrupt = bytearray(data); corrupt[-1] ^= 1
        for mode in (False, True):
            with self.subTest(mode=mode):
                with self.assertRaises(ValueError): png_pixels(bytes(corrupt), decode_pixels=mode)
        with self.assertRaises(ValueError): png_pixels(data, decode_pixels=0)

    def test_review_hash_entry_scope_and_time_must_match(self):
        for key, value in [('catalogue', {'path': r.CATALOGUE, 'sha256': 'f' * 64}), ('reviewed_entries', {'fixture-other': 'a' * 64}), ('reviewed_at', '2026-11-03T00:00:00Z')]:
            with self.subTest(key=key):
                old = self.review[key]; self.review[key] = value
                self.write(r.REVIEW, self.review); self.rejected('review' if key != 'reviewed_entries' else 'entry'); self.review[key] = old

    def test_nonce_command_external_request_and_source_epoch_lies_rejected(self):
        mutations = [('execution_nonce', '0' * 32), ('command', ['node', 'fake']), ('external_requests', ['https://outside.invalid']), ('sources_after', {})]
        for key, value in mutations:
            with self.subTest(key=key):
                old = self.generation[key]; self.generation[key] = value; self.save()
                self.assertEqual('failed', self.assess()['status']); self.generation[key] = old

    def test_path_escape_duplicate_json_and_boolean_schema_rejected(self):
        self.catalogue['pack_root'] = '../escape'; self.save(); self.rejected('Unsafe')
        self.catalogue['pack_root'] = '.build/reference-fixture'; self.save()
        p = self.root / r.CATALOGUE; p.write_text('{"schema_version":1,"schema_version":1}')
        self.rejected('duplicate')
        self.catalogue['schema_version'] = True; self.save(); self.rejected('schema')


class CurrentClientReferenceTests(unittest.TestCase):
    """Mutation fixtures only. No fixture grants a production reference."""

    def setUp(self):
        self.h = CurrentReferenceCatalogueTests(); self.h.setUp(); self.addCleanup(self.h.doCleanups)
        self.root, self.pack, self.context = self.h.root, self.h.pack, self.h.context
        self.h.write(r.CLIENT_RUNNER, '// New independent client capture fixture.\n')
        binary = self.h.write('.build/reference-fixture/guard-browser', 'guard browser bytes')
        self.browser = {'version': '141.0.0.0', 'executable': str(binary), 'sha256': self.h.sha(binary), 'playwright': '1.56.1'}
        self.context['client_browser'] = self.browser
        self.surface = r.client_surface('main')
        self.facts = {'viewport': {'width': 1040, 'height': 720}, 'device_pixel_ratio': 1,
            'css_zoom': 1, 'surface_box': {'x': 0, 'y': 0, 'width': 1040, 'height': 720},
            'content_box': {'x': 1, 'y': 44, 'width': 1038, 'height': 675},
            'fonts': {'status': 'loaded', 'checks': [{'font': '400 18px sans-serif', 'text': 'Fixture', 'loaded': True}]},
            'text_runs': [{'text': 'Fixture', 'tag': 'P', 'font_family': 'sans-serif', 'font_size': '18px',
                           'font_weight': '400', 'line_height': '27px', 'color': 'rgb(1, 2, 3)', 'display': 'block', 'visibility': 'visible'}],
            'lexical': {'ruby': [], 'rt': [], 'tokens': [], 'pos': []}, 'forms': [], 'focus': None,
            'running_animations': 0, 'capture_scope': 'whole_client', 'native_attestation': False}
        html = '<section id="main-window"><div class="main-layout"><p>Fixture</p></div></section>'
        style = 'body{margin:0}'
        self.original = {'url': 'http://127.0.0.1:8123/fixture-source.html?scenario=guard&theme=dark',
            'document_base_uri': 'http://127.0.0.1:8123/fixture-source.html?scenario=guard&theme=dark',
            'surface_selector': '#main-window', 'surface_html': html,
            'document_html': '<html><head><style>' + style + '</style></head><body>' + html + '</body></html>'}
        self.derived = {'surface_selector': '#main-window', 'projection': self.surface['projection'],
            'original_surface_html': html, 'surface_html': html,
            'document_html': '<html><head><base href="http://127.0.0.1:8123/fixture-source.html?scenario=guard&amp;theme=dark"><style>' + style + '</style><style data-client-projection="coordinate-only-v1">' + r.client_projection_css('#main-window') + '</style></head><body>' + html + '</body></html>'}
        self.measurements = {'client_surface': self.surface, 'projection': self.surface['projection'],
            'original_surface_box': {'x': 280, 'y': 85, 'width': 1040, 'height': 720},
            'original_viewport': {'width': 16, 'height': 12}, 'facts': self.facts}
        (self.pack / 'fixture.font').write_bytes(b'\x00\x01\x00\x00guard-font-bytes-not-product')
        self.facts['fonts'].update(platform=[{'familyName': 'Guard Font', 'postScriptName': 'GuardFont', 'isCustomFont': False, 'glyphCount': 7}],
            files=[{'postscript_name': 'GuardFont', 'original_path': '/System/Library/Fonts/GuardFont.ttf', 'file': self.h.artifact('fixture.font'), 'resolution': 'inventory_unique', 'coretext_probe': None}], gaps=[])
        inventory = {'command': ['system_profiler', 'SPFontsDataType', '-json'], 'exit_code': 0, 'error': None, 'stderr': '',
            'stdout': json.dumps({'SPFontsDataType': [{'path': '/System/Library/Fonts/GuardFont.ttf', 'enabled': 'yes', 'valid': 'yes',
                'typefaces': [{'_name': 'GuardFont', 'enabled': 'yes', 'valid': 'yes'}]}]})}
        self.h.pack_write('font-inventory.json', inventory)
        self.request = copy.deepcopy(self.h.request)
        self.request.update(schema_version=2, kind='current-client-reference-request/v2', client_surface=self.surface, browser=self.browser,
            generator={'path': r.CLIENT_RUNNER, 'sha256': self.h.sha(self.root / r.CLIENT_RUNNER)})
        self.row = {'state_id': 'fixture-normal', 'descriptor_sha256': self.h.result['descriptor_sha256'],
            'capture_scope': 'whole_client', 'status': 'passed', 'errors': [], 'actions': copy.deepcopy(self.h.result['actions'])}
        (self.pack / 'client.png').write_bytes(png(1040, 720))
        (self.pack / 'original.png').write_bytes(png(16, 12))
        self.generation = copy.deepcopy(self.h.generation)
        self.generation.update(schema_version=2, kind='current-client-reference-generation/v2', generator=self.request['generator'],
            command=['/guard/bin/node', str(self.root / r.CLIENT_RUNNER), '--request', str(self.pack / 'request.json'), '--output', str(self.pack)],
            font_inventory=self.h.artifact('font-inventory.json'), browser=copy.deepcopy(self.browser), results=[self.row])
        self.catalogue_path = '.build/reference-fixture/client-catalogue.json'; self.review_path = '.build/reference-fixture/client-review.json'
        self.entry = {'state_id': 'fixture-normal', 'pack_root': '.build/reference-fixture', 'generation': None, 'result_sha256': None}
        self.catalogue = {'schema_version': 2, 'kind': 'current-client-reference-catalogue/v2',
            **{k: self.context[k] for k in ('target_id', 'composition_id', 'design_closure_sha256')}, 'entries': [self.entry]}
        self.review = {**{k: self.h.review[k] for k in ('target_id', 'composition_id', 'design_closure_sha256', 'reviewer_id', 'author_ids', 'reviewed_at', 'independent_non_author_review', 'source_approval_extended', 'product_alignment_approved', 'native_verified')},
            'schema_version': 2, 'kind': 'independent-current-client-reference-review/v2', 'status': 'approved'}
        self.sync()

    def sync(self):
        for name, value in [('request.json', self.request), ('original-dom.json', self.original),
                            ('client-dom.json', self.derived), ('measurements.json', self.measurements)]:
            self.h.pack_write(name, value)
        self.row.update({k: self.h.artifact(name) for k, name in [('original_dom', 'original-dom.json'),
            ('client_dom', 'client-dom.json'), ('measurements', 'measurements.json'), ('client_png', 'client.png'), ('original_png', 'original.png')]})
        self.generation['request'] = self.h.ref(self.pack / 'request.json', self.pack)
        self.h.pack_write('verification.json', self.generation)
        self.entry.update(generation=self.h.ref(self.pack / 'verification.json', self.pack), result_sha256=r.identity(self.row))
        self.h.write(self.catalogue_path, self.catalogue)
        self.review.update(catalogue=self.h.ref(self.root / self.catalogue_path, self.root), reviewed_entries={'fixture-normal': r.identity(self.entry)})
        self.h.write(self.review_path, self.review)

    def assess(self):
        return r.assess_client_catalogue(self.root, self.catalogue_path, self.review_path, context=self.context)

    def rejected(self, phrase):
        value = self.assess(); self.assertEqual('failed', value['status'], value)
        self.assertTrue(any(phrase in e for e in value['errors']), value)

    def test_independently_reviewed_client_fixture_remains_reference_only(self):
        value = self.assess(); self.assertEqual('passed', value['status'], value)
        self.assertEqual(['fixture-normal'], value['approved_state_ids'])
        self.assertFalse(value['native_verified']); self.assertFalse(value['product_alignment_approved'])

    def test_client_request_uses_exact_current_runner_without_rewriting_legacy(self):
        from unittest.mock import patch
        with patch.object(r, 'authoritative_context', return_value=self.context), patch.object(r, 'client_browser_identity', return_value=self.browser):
            request = r.client_capture_request(self.root, ['fixture-normal'], ['author'], 'b' * 32, role='main')
        self.assertEqual(r.CLIENT_RUNNER, request['generator']['path'])
        self.assertNotEqual(r.RUNNER, request['generator']['path'])
        self.assertEqual(self.h.sha(self.root / r.RUNNER), self.context['generator_sha256'])
        with self.assertRaisesRegex(ValueError, 'different client role'):
            with patch.object(r, 'authoritative_context', return_value=self.context), patch.object(r, 'client_browser_identity', return_value=self.browser):
                r.client_capture_request(self.root, ['fixture-normal'], ['author'], 'b' * 32, role='settings')

    def test_old_target_schema_cannot_be_promoted_by_scope_or_new_fields(self):
        self.catalogue['schema_version'] = 1; self.sync(); self.rejected('target catalogue')
        self.catalogue['schema_version'] = 2; self.request['schema_version'] = 1; self.sync(); self.rejected('target request')
        self.request['schema_version'] = 2; self.request['capture_scope'] = 'whole_client'; self.sync(); self.rejected('exact client request')

    def test_legacy_runner_cannot_generate_whole_client(self):
        self.request['generator'] = {'path': r.RUNNER, 'sha256': self.context['generator_sha256']}
        self.generation['generator'] = self.request['generator']; self.sync(); self.rejected('generator')

    def test_missing_or_forged_browser_version_binary_hash_or_pw_are_rejected(self):
        original = copy.deepcopy(self.generation['browser'])
        for field, value in [('version', '999.0.0.0'), ('executable', '/other/browser'),
                             ('sha256', 'f' * 64), ('playwright', '0.0.1')]:
            with self.subTest(field=field):
                self.generation['browser'] = {**original, field: value}; self.sync(); self.rejected('browser')
        del self.generation['browser']; self.sync(); self.rejected('browser')

    def test_request_cannot_choose_a_different_browser_even_when_report_agrees(self):
        alternate = {**self.browser, 'version': '999.0.0.0'}
        self.request['browser'] = self.generation['browser'] = alternate
        self.sync(); self.rejected('browser')

    def test_managed_browser_binary_drift_is_rejected_after_metadata_rehash(self):
        Path(self.browser['executable']).write_text('changed binary bytes')
        self.sync(); self.rejected('browser binary')

    def test_browser_identity_rejects_installed_pw_or_binary_version_outside_repo_lock(self):
        from unittest.mock import patch
        from subprocess import CompletedProcess
        self.h.write('package.json', {'devDependencies': {'playwright-core': '1.56.1'}})
        self.h.write('package-lock.json', {'packages': {'node_modules/playwright-core': {'version': '1.56.1'}}})
        self.h.write('node_modules/playwright-core/browsers.json', {'browsers': [{'name': 'chromium', 'browserVersion': '141.0.0.0'}]})
        with patch.object(r.subprocess, 'run', return_value=CompletedProcess([], 0, json.dumps(self.browser), '')):
            self.assertEqual(self.browser, r.client_browser_identity(self.root))
        for field, value in [('playwright', '9.0.0'), ('version', '999.0.0.0')]:
            with self.subTest(field=field):
                returned = {**self.browser, field: value}
                with patch.object(r.subprocess, 'run', return_value=CompletedProcess([], 0, json.dumps(returned), '')):
                    with self.assertRaisesRegex(ValueError, 'browser identity unavailable'):
                        r.client_browser_identity(self.root)

    def test_logical_viewport_dpr_scope_and_surface_are_not_boolean_labels(self):
        for field, value in [('viewport', {'width': 1600, 'height': 1100}), ('device_pixel_ratio', True),
                             ('capture_scope', 'target'), ('native_attestation', True), ('css_zoom', 2),
                             ('surface_box', {'x': 280, 'y': 85, 'width': 1040, 'height': 720})]:
            with self.subTest(field=field):
                old = self.facts[field]; self.facts[field] = value; self.sync(); self.rejected('client'); self.facts[field] = old

    def test_source_and_derived_geometry_mismatches_are_not_resized(self):
        self.facts['surface_box']['width'] = 980; self.sync(); self.rejected('client_geometry_mismatch')
        self.facts['surface_box']['width'] = 1040
        self.measurements['original_surface_box']['height'] = 700; self.sync(); self.rejected('original client dimensions')

    def test_fonts_typography_lexical_and_live_forms_must_be_present(self):
        for field, value in [('fonts', {'status': 'loading', 'checks': []}), ('text_runs', []), ('lexical', {}),
                             ('forms', None), ('focus', 0), ('running_animations', 1)]:
            with self.subTest(field=field):
                old = self.facts[field]; self.facts[field] = value; self.sync(); self.rejected('client' if field == 'fonts' else '')
                self.facts[field] = old

    def test_rehashed_dom_replacement_and_internal_style_mutation_rejected(self):
        for key, value in [('surface_html', '<section>Changed content</section>'), ('original_surface_html', '<section>Different source</section>'),
                           ('document_html', self.derived['document_html'].replace('left:0!important;', 'left:0!important;width:1040px!important;'))]:
            with self.subTest(key=key):
                old = self.derived[key]; self.derived[key] = value; self.sync(); self.rejected('DOM' if key != 'document_html' else 'styles'); self.derived[key] = old

    def test_rehashed_ancestor_theme_or_extra_dom_cannot_hide_behind_same_surface(self):
        for value in [self.derived['document_html'].replace('<html>', '<html data-theme="light">'),
                      self.derived['document_html'].replace('</body>', '<div>Forged sibling</div></body>')]:
            self.derived['document_html'] = value; self.sync(); self.rejected('coordinate-only')

    def test_document_doctype_whitespace_does_not_change_rendered_dom(self):
        self.original['document_html'] = '<!doctype html>\n' + self.original['document_html']
        self.derived['document_html'] = '<!DOCTYPE html>' + self.derived['document_html']
        self.sync(); self.assertEqual('passed', self.assess()['status'], self.assess())

    def test_missing_dom_or_raw_png_cannot_be_hidden_by_rehashed_catalogue(self):
        (self.pack / 'client-dom.json').unlink(); self.rejected('hash')
        self.sync(); (self.pack / 'client.png').write_bytes(png(980, 720)); self.sync(); self.rejected('PNG dimensions')

    def test_raw_source_drift_and_recipe_forgery_rejected(self):
        self.row['actions'][-1]['actual'] = False; self.sync(); self.rejected('transcript')
        self.row['actions'][-1]['actual'] = True
        self.h.write('fixture-source.html', 'Changed'); self.sync(); self.rejected('source bytes')

    def test_resolved_font_bytes_and_inventory_are_not_readiness_labels(self):
        self.facts['fonts']['platform'] = []; self.sync(); self.rejected('font bytes')

    def test_missing_or_rehashed_corrupt_font_bytes_are_rejected(self):
        (self.pack / 'fixture.font').write_bytes(b'not-font-bytes')
        self.facts['fonts']['files'][0]['file'] = self.h.artifact('fixture.font')
        self.sync(); self.rejected('font bytes')

    def coretext_resolution(self, *, resolved='GuardFont', located='/System/Library/Fonts/GuardFont.ttf'):
        inventory = json.loads((self.pack / 'font-inventory.json').read_text())
        rows = json.loads(inventory['stdout'])
        rows['SPFontsDataType'].append({**copy.deepcopy(rows['SPFontsDataType'][0]), 'path': '/Library/Fonts/GuardFont.ttf'})
        inventory['stdout'] = json.dumps(rows); self.h.pack_write('font-inventory.json', inventory)
        self.generation['font_inventory'] = self.h.artifact('font-inventory.json')
        self.h.pack_write('probe.json', {'command': ['/usr/bin/swift', '-e', r.CORETEXT_FONT_PROBE, 'GuardFont'],
            'exit_code': 0, 'error': None, 'stderr': '', 'stdout': json.dumps({'requested': 'GuardFont', 'resolved': resolved, 'family': 'Guard', 'url': located})})
        commands = [['/usr/bin/swift', '--version'], ['/usr/bin/xcrun', '--show-sdk-path'], ['/usr/bin/xcrun', '--show-sdk-version'], ['/usr/bin/sw_vers']]
        self.h.pack_write('environment.json', {'commands': [{'command': command, 'exit_code': 0, 'stdout': 'Fixture system identity', 'stderr': '', 'error': None} for command in commands]})
        self.generation['font_probe_environment'] = self.h.artifact('environment.json')
        self.facts['fonts']['files'][0].update(resolution='coretext_exact', coretext_probe=self.h.artifact('probe.json'))
        self.sync()

    def test_coretext_exact_path_can_resolve_duplicate_inventory_without_guessing(self):
        self.coretext_resolution(); self.assertEqual('passed', self.assess()['status'], self.assess())

    def test_coretext_fallback_variable_name_or_conflicting_url_remains_rejected(self):
        self.coretext_resolution(resolved='FallbackFont'); self.rejected('CoreText fallback')
        self.coretext_resolution(located='/Library/Fonts/GuardFont.ttf'); self.rejected('CoreText fallback')

    def test_ambiguous_inventory_cannot_claim_unique_or_omit_sdk_command(self):
        self.coretext_resolution(); font = self.facts['fonts']['files'][0]
        font.update(resolution='inventory_unique', coretext_probe=None); self.sync(); self.rejected('ambiguous')
        self.coretext_resolution(); env = json.loads((self.pack / 'environment.json').read_text()); env['commands'] = []
        self.h.pack_write('environment.json', env); self.generation['font_probe_environment'] = self.h.artifact('environment.json')
        self.sync(); self.rejected('SDK/system')

    def test_non_author_review_and_reviewed_entry_are_mandatory(self):
        self.review['reviewer_id'] = 'guard-author'; self.sync(); self.rejected('independent')
        self.review['reviewer_id'] = 'reviewer'; self.review['status'] = 'pending_review'; self.sync()
        self.assertEqual('blocked', self.assess()['status'])
        self.review['status'] = 'approved'; self.review['native_verified'] = True; self.sync(); self.rejected('independent')

    def test_per_entry_pack_origin_path_and_generation_digest_are_bound(self):
        self.entry['pack_root'] = '../other'; self.sync(); self.rejected('Unsafe')
        self.entry['pack_root'] = '.build/reference-fixture'; self.sync()
        self.entry['generation']['sha256'] = 'f' * 64
        self.h.write(self.catalogue_path, self.catalogue); self.rejected('hash')




class PublicUiFontEvidenceTests(unittest.TestCase):
    """Synthetic mutation fixtures, never font or product acceptance."""
    def setUp(self):
        self.h = CurrentClientReferenceTests(); self.h.setUp(); self.addCleanup(self.h.doCleanups)
        self.pack, self.root = self.h.pack, self.h.root
        self.name = '.GuardSystemInstance'
        self.observation = {'selector': '#main-window > p', 'text': 'Fixture',
            'font_family': '-apple-system, sans-serif', 'font_size': '18px', 'font_weight': '600',
            'font_style': 'normal', 'font_stretch': '100%', 'font_optical_sizing': 'auto',
            'font_variation_settings': 'normal', 'platform_font': {'familyName': 'Guard Font',
                'postScriptName': self.name, 'isCustomFont': False, 'glyphCount': 7}}
        self.inventory = [{'path': '/System/Library/Fonts/GuardFont.ttf', 'enabled': 'yes', 'valid': 'yes',
            'typefaces': [{'_name': 'GuardFont', 'enabled': 'yes', 'valid': 'yes'}]}]
        font = self.h.facts['fonts']['files'][0]
        font.update(postscript_name=self.name, resolution='coretext_ui_instance_exact', ui_probe={})
        native = {'postscript': self.name, 'family': 'Guard Font', 'full_name': 'Guard UI',
            'size': 18, 'ui_type': 3, 'url': font['original_path'],
            'axes': [{'identifier': 1, 'tag': 'wght', 'name': 'Weight', 'minimum': 1, 'maximum': 1000, 'default': 400}],
            'variations': [{'identifier': 1, 'tag': 'wght', 'value': 700}],
            'file': {'real_path': font['original_path'], 'sha256': font['file']['sha256'], 'bytes': font['file']['bytes'],
                'owner': 'root', 'owner_id': 0, 'group': 'wheel', 'group_id': 0, 'mode': 0o644}}
        self.match = {'api': 'CTFontCreateUIFontForLanguage', 'parameters': {'ui_type': 'emphasizedSystem', 'size': 18, 'language': None},
            'native': native, 'base_native': {**copy.deepcopy(native), 'postscript': 'GuardFont'}}
        self.result = {'kind': 'coretext-ui-instance-exact/v1', 'results': [{'index': 0,
            'source_observation': self.observation, 'attempts': [self.match], 'exact_match': self.match,
            'status': 'exact_native_match'}]}
        self.files = {self.name: font}

    def check(self):
        r.validate_ui_font_result(self.result, [self.observation], self.files, self.inventory)

    def test_css_weight_is_a_candidate_not_actual_native_axis(self):
        self.check()
        self.assertEqual('600', self.observation['font_weight'])
        self.assertEqual(700, self.match['native']['variations'][0]['value'])

    def test_unknown_context_is_rejected_even_with_matching_ps(self):
        for field, value in [('font_style', 'italic'), ('font_stretch', '75%'), ('font_variation_settings', '"wght" 700'),
                             ('font_optical_sizing', 'mystery'), ('font_family', 'serif'), ('font_size', 'NaNpx'),
                             ('font_weight', 'Infinity')]:
            with self.subTest(field=field):
                before = self.observation[field]; self.observation[field] = value
                with self.assertRaises(ValueError): self.check()
                self.observation[field] = before

    def test_fallback_or_unresolved_cannot_attest_instance(self):
        self.match['native']['postscript'] = 'TimesNewRomanPSMT'
        with self.assertRaisesRegex(ValueError, 'fallback'): self.check()
        self.match['native']['postscript'] = self.name
        self.result['results'][0]['status'] = 'unresolved'
        with self.assertRaisesRegex(ValueError, 'unresolved'): self.check()

    def test_url_hash_and_actual_owner_are_required(self):
        for field, value in [('url', '/foreign/font.ttf'), ('size', 21)]:
            old = self.match['native'][field]; self.match['native'][field] = value
            with self.assertRaises(ValueError): self.check()
            self.match['native'][field] = old
        for field, value in [('sha256', 'f'*64), ('bytes', 1), ('real_path', '/foreign/font.ttf'), ('owner_id', 501), ('mode', 0o666)]:
            old = self.match['native']['file'][field]; self.match['native']['file'][field] = value
            with self.assertRaises(ValueError): self.check()
            self.match['native']['file'][field] = old

    def test_inventory_cannot_use_dynamic_name_in_place_of_real_base_owner(self):
        self.inventory[0]['typefaces'][0]['_name'] = self.name
        with self.assertRaisesRegex(ValueError, 'base font owner'): self.check()

    def test_native_variation_must_belong_to_real_axis_and_range(self):
        for field, value in [('value', 1001), ('identifier', 2), ('tag', 'opsz')]:
            old = self.match['native']['variations'][0][field]; self.match['native']['variations'][0][field] = value
            with self.assertRaisesRegex(ValueError, 'axis'): self.check()
            self.match['native']['variations'][0][field] = old

    def prepare_execution(self):
        from unittest.mock import patch
        self.patch = patch
        h = self.h; fonts = h.facts['fonts']; fonts['platform'][0]['postScriptName'] = self.name
        fonts['observations'] = [self.observation]
        h.facts['text_runs'][0].update({k: v for k,v in self.observation.items() if k != 'platform_font'})
        self.h.h.pack_write('ui-input.json', [self.observation])
        self.h.h.pack_write('font-inventory.json', {'command': ['system_profiler', 'SPFontsDataType', '-json'],
            'exit_code': 0, 'error': None, 'stdout': json.dumps({'SPFontsDataType': self.inventory})})
        h.generation['font_inventory'] = h.h.artifact('font-inventory.json')
        (self.pack/'probe.swift').write_text(r.CORETEXT_UI_FONT_PROBE)
        binary = self.pack/'probe'; binary.write_bytes(b'never-executed-synthetic-binary'); binary.chmod(0o755)
        sdk = self.pack/'sdk'; header = sdk/'System/Library/Frameworks/CoreText.framework/Headers/CTFont.h'
        header.parent.mkdir(parents=True); header.write_text('CTFontCreateUIFontForLanguage() fixture')
        commands = [['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '--version'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version'], ['/usr/bin/sw_vers']]
        self.outputs = ['Fixture compiler\n', str(sdk)+'\n', 'Fixture sdk\n', 'Fixture system\n']
        h.h.pack_write('ui-environment.json', {'sdk_selection': 'macosx', 'commands': [{'command': c, 'exit_code': 0, 'error': None, 'stdout': o} for c,o in zip(commands,self.outputs)],
            'sdk_authority': {'path': str(header), 'sha256': h.h.sha(header), 'bytes': header.stat().st_size}})
        h.h.pack_write('ui-compilation.json', {'command': ['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '-sdk', str(sdk), str(self.pack/'probe.swift'), '-o', str(binary)], 'exit_code':0, 'error': None})
        self.result['input_sha256'] = h.h.sha(self.pack/'ui-input.json')
        self.proof = {'kind': 'coretext-ui-font-execution/v1', **{k:h.h.artifact(f) for k,f in [('source','probe.swift'),('executable','probe'),('input','ui-input.json'),('environment','ui-environment.json'),('compilation','ui-compilation.json')]},
            'execution': {'command': [str(binary), str(self.pack/'ui-input.json')], 'exit_code':0, 'error':None, 'stdout': json.dumps(self.result)}}
        self.refresh_proof()

    def refresh_proof(self):
        self.h.h.pack_write('ui-proof.json', self.proof)
        self.files[self.name]['ui_probe'] = self.h.h.artifact('ui-proof.json')
        self.h.sync()

    def audit(self, replay=None):
        with self.patch.object(r, '_actual_font_command', side_effect=self.outputs), self.patch.object(r, 'replay_ui_font_probe', return_value=replay or self.result) as replay_mock:
            result = r.validate_client_font_evidence(self.root, self.pack, self.h.generation, self.h.facts)
            replay_mock.assert_called_once_with([self.observation])
            return result

    def test_strict_new_branch_replays_owned_probe_and_retains_scope(self):
        self.prepare_execution(); result = self.audit()
        self.assertEqual(1, result['public_ui_probes_independently_replayed'])
        self.assertFalse(result['native_verified']); self.assertFalse(result['product_alignment_approved'])

    def test_rehashed_forged_axis_is_rejected_by_independent_native_replay(self):
        self.prepare_execution(); actual = copy.deepcopy(self.result)
        self.match['native']['variations'][0]['value'] = 600
        self.proof['execution']['stdout'] = json.dumps(self.result); self.refresh_proof()
        with self.assertRaisesRegex(ValueError, 'independent native UI replay differs'): self.audit(actual)

    def test_rehashed_foreign_probe_source_or_binary_command_is_rejected(self):
        self.prepare_execution(); self.proof['execution']['command'][0] = '/foreign/untrusted-binary'; self.refresh_proof()
        with self.assertRaisesRegex(ValueError, 'controlled public UI'): self.audit()

    def test_observation_css_and_aggregate_glyphs_are_bound(self):
        self.prepare_execution(); self.h.facts['text_runs'][0]['font_weight'] = '400'
        with self.assertRaisesRegex(ValueError, 'measured text context'): self.audit()
        self.h.facts['text_runs'][0]['font_weight'] = '600'; self.h.facts['fonts']['platform'][0]['glyphCount'] = 9
        with self.assertRaisesRegex(ValueError, 'glyph closure'): self.audit()




class ApprovedC1ContainerTests(unittest.TestCase):
    """Authority/DOM mutation tests; these cannot approve candidate PNGs."""
    @classmethod
    def setUpClass(cls):
        import shutil
        cls.repository = Path(__file__).resolve().parents[2]
        cls.temp = tempfile.TemporaryDirectory(); cls.root = Path(cls.temp.name).resolve()
        approval = r.load(cls.repository/r.CLIENT_C1_APPROVAL)
        parent = r.load(cls.repository/approval['parent_approval']['path'])
        paths = [r.CLIENT_C1_APPROVAL, approval['parent_approval']['path'], *approval['approved_sources'], *parent['design_sources']]
        for relative in paths:
            target = cls.root/relative; target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(cls.repository/relative,target)
        cls.authority = r.client_c1_authority(cls.root)

    @classmethod
    def tearDownClass(cls): cls.temp.cleanup()

    def mutate_rejected(self, relative, data, expected):
        path = self.root/relative; original = path.read_bytes()
        try:
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError,expected): r.client_c1_authority(self.root)
        finally: path.write_bytes(original)

    def test_v3_generation_cannot_reuse_coordinate_only_v2_request(self):
        h=CurrentClientReferenceTests();h.setUp();self.addCleanup(h.doCleanups)
        h.generation.update(schema_version=3,kind='current-client-reference-generation/v3')
        h.request['approved_layout']={}  # Adversary supplies the v3 field but retains v2 identity.
        h.sync()
        result=h.assess()
        self.assertEqual('failed',result['status'])
        self.assertIn('target request cannot grant',str(result['errors']))

    def test_exact_user_approval_and_parent_61_remain_separate(self):
        self.assertEqual(61,len(self.authority['parent_design_sources']))
        self.assertEqual(3,len(self.authority['sources']))
        self.assertEqual({'settingsSize':'native'},self.authority['contract']['required_query'])

    def test_rehashed_approval_self_declaration_is_not_authority(self):
        approval = r.load(self.root/r.CLIENT_C1_APPROVAL)
        approval['user_confirmation']['answer'] = 'settings B'
        self.mutate_rejected(r.CLIENT_C1_APPROVAL,json.dumps(approval).encode(),'approval bytes')

    def test_css_js_contract_and_parent_source_byte_drift_are_rejected(self):
        for relative in [*self.authority['sources'], next(iter(self.authority['parent_design_sources']))]:
            with self.subTest(path=relative):
                self.mutate_rejected(relative,(self.root/relative).read_bytes()+b'\n/* drift */','hash differs')

    def test_settings_b_and_unknown_modes_cannot_be_normal(self):
        self.assertEqual({'width':1000,'height':720,'device_scale_factor':1},r.client_c1_surface(self.root,'settings')['viewport'])
        for role, variant in [('settings','flow'),('settings','1040x760'),('hud','normal'),('main','unknown')]:
            with self.subTest(role=role,variant=variant):
                with self.assertRaises(ValueError): r.client_c1_surface(self.root,role,variant)
        self.assertEqual(1040,r.client_surface('settings')['viewport']['width'])  # old v2 unchanged

    def test_required_native_query_is_added_without_mutating_frozen_recipe(self):
        descriptor = {'target':{'container':'settings'},'design_recipe':[{'action':'navigate','path':'source.html','query':{'scenario':'settings','theme':'dark'}},{'action':'click','selector':'#general'}]}
        original = copy.deepcopy(descriptor)
        effective = r.client_c1_recipe(descriptor,self.authority)
        self.assertEqual(original,descriptor)
        self.assertEqual({'scenario':'settings','theme':'dark','settingsSize':'native'},effective[0]['query'])
        descriptor['design_recipe'][0]['query']['scenario'] = 'main'
        with self.assertRaisesRegex(ValueError,'explicit settings'):r.client_c1_recipe(descriptor,self.authority)

    def projection(self):
        h = CurrentClientReferenceTests();h.setUp();self.addCleanup(h.doCleanups)
        raw = copy.deepcopy(h.original);raw['url']+='&settingsSize=native';raw['document_base_uri']=raw['url']
        derived = copy.deepcopy(h.derived)
        derived['document_html']=derived['document_html'].replace('scenario=guard&amp;theme=dark','scenario=guard&amp;theme=dark&amp;settingsSize=native')
        derived['document_html']=derived['document_html'].replace('<html>','<html data-client-candidate="c1" data-client-role="main" data-client-settings="native">')
        css=(self.root/self.authority['contract']['styles']).read_text()
        derived['document_html']=derived['document_html'].replace('data-client-projection="coordinate-only-v1"','data-client-projection="approved-client-c1-v1"').replace(r.client_projection_css('#main-window'),css)
        surface=r.client_c1_surface(self.root,'main')
        derived['projection']=surface['projection']
        return raw,derived,surface,{'css':css}

    def test_independent_tree_rebuild_accepts_exact_approved_c1_transform(self):
        raw,derived,surface,layout=self.projection()
        r.validate_client_projection(raw,derived,surface,approved_layout=layout)

    def test_arbitrary_style_sidebar_width_content_class_and_crop_substitutes_rejected(self):
        for mutation in [lambda d:d.update(document_html=d['document_html'].replace('190px','189px')),
                         lambda d:d.update(document_html=d['document_html'].replace('<p>Fixture','<p>changed')),
                         lambda d:d.update(document_html=d['document_html'].replace('main-layout','main-layout other')),
                         lambda d:d.update(document_html=d['document_html'].replace('</head>','<style>body{width:1600px}</style></head>'))]:
            raw,derived,surface,layout=self.projection();mutation(derived)
            with self.assertRaisesRegex(ValueError,'approved C1'):r.validate_client_projection(raw,derived,surface,approved_layout=layout)

    def test_duplicate_class_or_style_attributes_cannot_hide_different_browser_semantics(self):
        raw,derived,surface,layout=self.projection()
        derived['document_html']=derived['document_html'].replace('class="main-layout"','class="changed-first" class="main-layout"')
        with self.assertRaisesRegex(ValueError,'duplicate client DOM attributes'):
            r.validate_client_projection(raw,derived,surface,approved_layout=layout)

    def test_missing_native_query_or_fake_coordinate_only_label_is_rejected(self):
        raw,derived,surface,layout=self.projection();raw['url']=raw['url'].replace('&settingsSize=native','');raw['document_base_uri']=raw['url']
        with self.assertRaisesRegex(ValueError,'settingsSize=native'):r.validate_client_projection(raw,derived,surface,approved_layout=layout)
        raw,derived,surface,layout=self.projection();surface['projection']='original_surface_coordinate_only/v1'
        with self.assertRaisesRegex(ValueError,'mislabeled'):r.validate_client_projection(raw,derived,surface,approved_layout=layout)

    def test_actual_geometry_requires_A_sidebar_and_narrow_preview_placement(self):
        surface=r.client_c1_surface(self.root,'main','narrow')
        geometry={'viewport':{'width':760,'height':560},'surface':{'x':0,'y':0,'width':760,'height':560},'sidebar':{'x':0,'y':40,'width':57,'height':520},
            'reading':{'x':75,'y':100,'width':600,'height':300},'preview':{'x':75,'y':420,'width':282,'height':346},'content_scroll':{},'preview_reachability':{'fully_visible':True,'restored':True,'visible_width':282,'visible_height':346}}
        r.validate_c1_layout_geometry(geometry,surface,self.authority)
        for key,value in [('sidebar',{'x':0,'y':40,'width':190,'height':520}),('preview',{'x':75,'y':100,'width':282,'height':346})]:
            other=copy.deepcopy(geometry);other[key]=value
            with self.assertRaises(ValueError):r.validate_c1_layout_geometry(other,surface,self.authority)
        other=copy.deepcopy(geometry);other['preview_reachability']['fully_visible']=False
        with self.assertRaisesRegex(ValueError,'real content scroller'):r.validate_c1_layout_geometry(other,surface,self.authority)
        other=copy.deepcopy(geometry);other['viewport']['width']=1600
        with self.assertRaises(ValueError):r.validate_c1_layout_geometry(other,surface,self.authority)


if __name__ == '__main__': unittest.main()
