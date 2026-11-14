"""Adversarial evidence guards; synthetic PNGs are never product acceptance."""
import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest
from unittest.mock import patch
import zlib

import reading_alignment_preflight as gate


def png(width, height, rgba, mode=0):
    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body) & 0xffffffff)
    stride = width * 4
    rows = b''
    previous = bytes(stride)
    for y in range(height):
        line = rgba[y * stride:(y + 1) * stride]
        filtered = bytes((value - (line[x - 4] if x >= 4 else 0)) & 255 for x, value in enumerate(line)) if mode == 1 else bytes((value - previous[x]) & 255 for x, value in enumerate(line)) if mode == 2 else line
        rows += bytes([mode]) + filtered
        previous = line
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')


class ReadingAlignmentPreflightTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.out = self.root / '.build/run'
        self.out.mkdir(parents=True)
        self.variants = [{**copy.deepcopy(row), 'viewport': {'width': 3, 'height': 2}} for row in gate.VARIANTS]
        self.addCleanup(patch.stopall)
        patch.object(gate, 'VARIANTS', self.variants).start()
        self.sources = {'source_hashes': {'src.tsx': 'a' * 64}, 'source_tree_sha256': 'b' * 64,
                        'design_closure_sha256': gate.CURRENT_DESIGN_CLOSURE_SHA256}
        design_data = self.root / 'docs/design/first-product-v1/data.js'
        design_data.parent.mkdir(parents=True)
        design_data.write_text('window.LunaDesignData = ' + json.dumps({'manualSample': {'source': '春の空。', 'translation': '春天的天空。'}}) + ';')
        patch.object(gate, 'source_context', return_value=self.sources).start()
        now = datetime.now(timezone.utc)
        command = ['node', str(self.root / gate.RUNNER), str(self.root), str(self.out), str(self.out / 'context.json')]
        self.context = {**copy.deepcopy(self.sources), 'schema_version': 1, 'execution_nonce': 'd' * 32,
                        'variants': self.variants, 'command': command, 'started_at': now.isoformat()}
        self.invocation = {'command': command, 'exit_code': 0, 'finished_at': (now + timedelta(seconds=2)).isoformat(),
                           'stdout_sha256': gate.sha(b''), 'stderr_sha256': gate.sha(b'')}
        (self.out / 'stdout.log').write_bytes(b''); (self.out / 'stderr.log').write_bytes(b'')
        self.report = {**copy.deepcopy(self.sources), 'schema_version': 1, 'kind': gate.KIND,
                       'status': 'capture_complete', 'reference_approval': 'pending_independent_review',
                       'claims_product': False, 'product_alignment_approved': False, 'native_verified': False,
                       'packaged_native_tests_executed': False, 'skipped_assertions': 0,
                       'execution_nonce': self.context['execution_nonce'], 'command': command,
                       'started_at': now.isoformat(), 'finished_at': (now + timedelta(seconds=1)).isoformat(),
                       'source_hashes_after': self.sources['source_hashes'], 'browser_version': 'test-guard-only',
                       'assertion_count': 8, 'page_errors': [], 'console_errors': [], 'external_requests': [], 'variants': [],
                       'server_origins': {'design': 'http://127.0.0.1:12345', 'react': 'http://127.0.0.1:12345'}}
        desktop = self.root / 'apps/desktop'; node = str(Path(shutil.which('node')).resolve())
        commands = [[node, str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
                    [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'), '--outDir', str(self.out / 'test-build'), '--sourcemap', '--manifest']]
        self.report['build_commands'] = []
        for command, name in zip(commands, ('typecheck-tests', 'controlled-production-components-build')):
            log = self.write(f'logs/{name}.json', json.dumps({'command': command, 'cwd': 'apps/desktop', 'exit_code': 0, 'stdout': '', 'stderr': ''}).encode())
            self.report['build_commands'].append({'command': command, 'exit_code': 0, 'log': log})
        sources = ['src/app/WindowRoot.tsx', 'src/features/reading/ReadingPanel.tsx', 'src/bridge/projections/application-store.ts']
        for source in sources:
            p = desktop / source; p.parent.mkdir(parents=True, exist_ok=True); p.write_text('guard-only original ' + source)
        source_map = {'sources': ['../../../' + source for source in sources], 'sourcesContent': [(desktop / source).read_text() for source in sources]}
        self.report['source_maps'] = [{**self.write('test-build/assets/component.js.map', json.dumps(source_map).encode()), 'sources': source_map['sources']}]
        for variant in self.variants:
            identity = variant['id']
            row = {**variant, 'input': {'source_text': '春の空。', 'translation': '春天的天空。'}, 'alignment_status': 'failed'}
            for producer in ('design', 'react'):
                structure = {key: producer == 'design' for key in ('current_card', 'body_disclosure', 'extraction_disclosure', 'hook_disclosure')}
                structure['mutually_exclusive_tabs'] = producer == 'react'
                dom = {**variant, 'producer': 'design' if producer == 'design' else 'production_react',
                       'source_text': row['input']['source_text'], 'translation': row['input']['translation'],
                       'page_text': '春の空。春天的天空。', 'structure': structure,
                       'provenance': {'entry': 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html' if producer == 'design' else 'apps/desktop/tests/visual/index.html',
                                      'production_styles_modified': False, 'controlled_transport': producer == 'react'}}
                dom['url'] = 'http://127.0.0.1:12345/' + ('docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading&theme=' + variant['theme'] if producer == 'design' else 'tests/visual/index.html?role=main&scenario=reading')
                dom['root_html'] = '<section>' + ('<article data-guided-current="true">春の空。春天的天空。</article><details data-flat-session-section="reading"></details><details data-flat-session-section="texts"></details><details data-flat-session-section="logs"></details>' if producer == 'design' else '<article>春の空。春天的天空。</article><nav role="tablist"></nav>') + '</section>'
                dom['structure_selectors'] = gate.STRUCTURE_SELECTORS
                dom['observed_source_text'] = row['input']['source_text']; dom['observed_translation'] = row['input']['translation']
                dom['document_attributes'] = {'data-theme': variant['theme']}
                dom['geometry'] = {'viewport_width': 3, 'viewport_height': 2}
                dom['computed_conditions'] = {'viewport_width': 3, 'viewport_height': 2, 'theme_attribute': variant['theme'], 'document_zoom': variant['zoom'], 'device_scale_factor': 1}
                pixels = bytes([0, 0, 0, 255] if producer == 'design' else [1, 0, 0, 255]) * 6
                row[producer] = {'dom': self.write(f'raw/{identity}-{producer}.dom.json', json.dumps(dom).encode()),
                                 'screenshot': {**self.write(f'captures/{identity}-{producer}.png', png(3, 2, pixels)), 'width': 3, 'height': 2}}
            row['comparison'] = {'different_pixels': 6, 'total_pixels': 6, 'ratio': 1.0,
                                 'pixel_comparison_eligible': False, 'scope': 'diagnostic_full_viewport_hosts_differ_not_release_metric',
                                 'diff': {**self.write(f'captures/{identity}-diff.png', png(3, 2, bytes([255, 0, 0, 255]) * 6)), 'width': 3, 'height': 2}}
            self.report['variants'].append(row)

    def write(self, relative, data):
        path = self.out / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return {'path': relative, 'sha256': gate.sha(data), 'bytes': len(data)}

    def inspect(self):
        (self.out / 'report.json').write_text(json.dumps(self.report))
        return gate.inspect_execution(self.root, self.out, self.context, self.invocation)

    def reject(self, phrase=None):
        result = self.inspect()
        self.assertEqual(result['status'], 'failed', result)
        self.assertTrue(result['errors'])
        if phrase:
            self.assertIn(phrase, '\n'.join(result['errors']))

    def test_actual_difference_is_retained_without_acceptance(self):
        result = self.inspect()
        self.assertEqual(result['status'], 'passed', result)
        self.assertEqual(len(result['observations']), 4)
        self.assertTrue(all(row['alignment_status'] == 'failed' and row['different_pixels'] == 6 for row in result['observations']))
        self.assertFalse(result['next_product_stage_permitted'])
        self.assertFalse(result['reference_approved'])

    def test_missing_variant(self):
        self.report['variants'].pop(); self.reject('four exact')

    def test_duplicate_variant(self):
        self.report['variants'][-1] = self.report['variants'][0]; self.reject('four exact')

    def test_zero_assertions(self):
        self.report['assertion_count'] = 0; self.reject('positive actual')

    def test_bool_assertions(self):
        self.report['assertion_count'] = True; self.reject('positive actual')

    def test_skipped_assertions(self):
        self.report['skipped_assertions'] = 1; self.reject('skipped_assertions')

    def test_product_claim(self):
        self.report['product_alignment_approved'] = True; self.reject('product_alignment_approved')

    def test_fake_reference_approval(self):
        self.report['reference_approval'] = 'approved'; self.reject('reference_approval')

    def test_stale_nonce(self):
        self.report['execution_nonce'] = 'e' * 32; self.reject('execution_nonce')

    def test_stale_context(self):
        self.context['source_tree_sha256'] = 'c' * 64; self.reject('source_tree_sha256')

    def test_source_changes_during_run(self):
        self.report['source_hashes_after'] = {}; self.reject('source files changed')

    def test_wrong_command(self):
        self.invocation['command'] = ['echo', 'passed']; self.reject('actual fixed')

    def test_nonzero_exit(self):
        self.invocation['exit_code'] = 1; self.reject('actual fixed')

    def test_bool_exit(self):
        self.invocation['exit_code'] = False; self.reject('actual fixed')

    def test_wrong_viewport(self):
        self.report['variants'][0]['viewport'] = {'width': 4, 'height': 2}; self.reject('condition differs')

    def test_wrong_zoom(self):
        self.report['variants'][3]['zoom'] = 1; self.reject('condition differs')

    def test_network_error(self):
        self.report['external_requests'] = ['https://example.com/']; self.reject('network errors')

    def test_page_error(self):
        self.report['page_errors'] = ['TypeError']; self.reject('runtime/network')

    def test_png_tamper(self):
        (self.out / 'captures/dark-react.png').write_bytes(b'fake'); self.reject('bytes/hash')

    def test_forged_zero_difference(self):
        self.report['variants'][0]['comparison']['different_pixels'] = 0; self.reject('raw PNG comparison')

    def test_forged_diff_artifact_with_rebound_hash(self):
        self.report['variants'][0]['comparison']['diff'].update(self.write('captures/dark-diff.png', png(3, 2, bytes([48, 48, 48, 255]) * 6)))
        self.reject('diff artifact')

    def test_reference_replaced_by_candidate_path(self):
        self.report['variants'][0]['design']['screenshot'] = self.report['variants'][0]['react']['screenshot']; self.reject('independent artifact')

    def test_raw_dom_wrong_data_with_rebound_hash(self):
        row = self.report['variants'][0]['react']
        path = self.out / row['dom']['path']; dom = json.loads(path.read_text()); dom['source_text'] = '別の文。'
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('rendered reading data')

    def test_raw_dom_wrong_producer(self):
        row = self.report['variants'][0]['react']
        path = self.out / row['dom']['path']; dom = json.loads(path.read_text()); dom['producer'] = 'design'
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('independent producer')

    def test_raw_style_injection(self):
        row = self.report['variants'][0]['react']
        dom = json.loads((self.out / row['dom']['path']).read_text()); dom['provenance']['production_styles_modified'] = True
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('style provenance')

    def test_symlink_png(self):
        path = self.out / 'captures/dark-react.png'; target = self.out / 'other.png'; path.rename(target); path.symlink_to(target)
        self.reject('Symlink')

    def test_actual_pixel_decoder_filters(self):
        raw = bytes(range(24))
        for mode in (0, 1, 2):
            with self.subTest(mode=mode):
                self.assertEqual(gate.png_pixels(png(3, 2, raw, mode)), (3, 2, raw))

    def test_raw_html_structures_cannot_be_forged(self):
        row = self.report['variants'][0]['react']
        dom = json.loads((self.out / row['dom']['path']).read_text()); dom['structure'] = {k: False for k in gate.STRUCTURE_SELECTORS}
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('raw HTML structural')

    def test_unrelated_raw_dom_url(self):
        row = self.report['variants'][0]['react']
        dom = json.loads((self.out / row['dom']['path']).read_text()); dom['url'] = 'https://unrelated.invalid/fake'
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('local producer entry')

    def test_raw_measured_geometry_tamper(self):
        row = self.report['variants'][0]['react']
        dom = json.loads((self.out / row['dom']['path']).read_text()); dom['geometry']['viewport_width'] = 1
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('measured viewport')

    def test_raw_measured_zoom_tamper(self):
        row = self.report['variants'][3]['react']
        dom = json.loads((self.out / row['dom']['path']).read_text()); dom['computed_conditions']['document_zoom'] = 1
        row['dom'] = self.write(row['dom']['path'], json.dumps(dom).encode()); self.reject('document theme/zoom')

    def test_build_source_paths_without_actual_content(self):
        row = self.report['source_maps'][0]
        source_map = json.loads((self.out / row['path']).read_text()); source_map['sourcesContent'][0] = 'forged production component'
        row.update(self.write(row['path'], json.dumps(source_map).encode())); self.reject('source-map content')

    def test_missing_actual_build(self):
        self.report['build_commands'] = []; self.reject('actual fixed component build')

    def test_raw_invocation_log_tamper(self):
        (self.out / 'stdout.log').write_text('fake passed'); self.reject('raw invocation log')

    def test_host_pixels_are_never_product_fidelity(self):
        self.report['variants'][0]['comparison']['pixel_comparison_eligible'] = True; self.reject('product fidelity')

    def test_wrong_shared_input(self):
        self.report['variants'][0]['input'] = {'source_text': '違う文。', 'translation': '别的句子。'}
        self.reject('frozen design sample')

    def test_png_crc_and_trailing_data(self):
        data = png(3, 2, bytes(range(24)))
        for bad in (data[:-1], data + b'junk', data[:40] + b'X' + data[41:]):
            with self.assertRaises(ValueError): gate.png_pixels(bad)

    def test_duplicate_json_rejected(self):
        (self.out / 'report.json').write_text('{"status":"passed","status":"capture_complete"}')
        result = gate.inspect_execution(self.root, self.out, self.context, self.invocation)
        self.assertIn('duplicate JSON', '\n'.join(result['errors']))

    def test_existing_output_is_never_overwritten(self):
        with self.assertRaisesRegex(ValueError, 'already exists'):
            gate.execute(self.root, '.build/run')


if __name__ == '__main__':
    unittest.main()
