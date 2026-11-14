"""Negative guards for finite real-component evidence, not product acceptance."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import subprocess
import struct
import unittest
import zipfile
import zlib
from unittest.mock import patch

from current_design_react_adapter import STATE_IDS, CHECK_ID, execute
from current_design_react_inspect import ASSERTIONS, artifact, inspect
from current_design_react_recompute import DocumentFacts, pixel_facts, recompute, enclosing_png_size
from test_current_reference_catalogue import png


class CurrentDesignReactTests(unittest.TestCase):
    def test_capability_is_exact_finite_layer_not_all_state_or_native(self):
        from current_design_alignment import STATE_IDS as all_states
        self.assertEqual(4, len(STATE_IDS)); self.assertLess(STATE_IDS, all_states)
        self.assertEqual(STATE_IDS, set(ASSERTIONS))
        self.assertEqual(34, sum(len(v) for v in ASSERTIONS.values()))
        self.assertEqual(34, sum(len(set(v)) for v in ASSERTIONS.values()))

    def test_original_rgba_comparison_accepts_only_identical_originals(self):
        data = png(4, 3); fact = pixel_facts(data, data)
        self.assertEqual('passed', fact['status']); self.assertEqual(0, fact['different_pixels'])
        self.assertEqual(0, fact['absolute_channel_delta'])

    def test_dimension_change_fails_without_resizing(self):
        result = pixel_facts(png(4, 3), png(3, 3))
        self.assertEqual('failed', result['status']); self.assertEqual([4, 3], result['comparison_canvas'])
        self.assertEqual([4, 3], result['reference_size']); self.assertEqual([3, 3], result['candidate_size'])
        self.assertEqual(3, result['different_pixels'])

    def test_forged_diff_and_corrupt_original_fail(self):
        with self.assertRaisesRegex(ValueError, 'diff PNG'): pixel_facts(png(4, 3), png(3, 3), png(4, 3))
        with self.assertRaises(ValueError): pixel_facts(b'invalid', png(3, 3))

    def test_ruby_is_not_copied_into_actual_original_text(self):
        d = DocumentFacts('<p data-testid="source-text"><ruby>春<rt>はる</rt><rp>(</rp></ruby>の空。</p>')
        self.assertEqual(['春の空。'], d.source_texts())

    def test_hidden_retired_entity_is_still_a_retired_entity(self):
        d = DocumentFacts('<div style="display:none" role="tablist" aria-label="模型配置实体"></div>')
        self.assertEqual(1, d.count(**{'aria-label': '模型配置实体'}))

    def test_all_body_rows_and_order_are_recomputed(self):
        d = DocumentFacts('<details data-flat-session-section="reading" open><article data-record-kind="body" data-record-sequence="3"><div><p lang="ja">春。</p><p>春天。<small> · 服务</small></p></div></article><article data-record-kind="body" data-record-sequence="4"><div><p lang="ja">空。</p><p>天空。</p></div></article></details>')
        expected = [{'sequence': '3', 'source': '春。', 'translation': '春天。'}, {'sequence': '4', 'source': '空。', 'translation': '天空。'}]
        self.assertEqual(expected, d.body_rows()); self.assertNotEqual(expected[:1], d.body_rows())
        self.assertNotEqual(list(reversed(expected)), d.body_rows())

    def test_original_design_body_rows_have_independent_sequence(self):
        d = DocumentFacts('<div data-flat-history-record="r"><span class="rw-sequence">#3</span><p>春。</p><div class="rw-history-translation">春天。</div></div>')
        self.assertEqual([{'sequence': '3', 'source': '春。', 'translation': '春天。'}], d.body_rows(design=True))

    def test_fractional_target_uses_enclosing_coordinates_without_resizing(self):
        self.assertEqual([11, 7], enclosing_png_size({'x': .25, 'y': .75, 'width': 10.5, 'height': 5.5}, {'x': 0, 'y': 0}))
        self.assertEqual([10, 5], enclosing_png_size({'x': 10, 'y': 20, 'width': 10, 'height': 5}, {'x': 0, 'y': 0}))
        self.assertEqual([796, 356], enclosing_png_size({'x': 257, 'y': 403.734375, 'width': 796, 'height': 354.96875}, {'x': 0, 'y': 0}))
        for value in (True, float('nan'), float('inf')):
            with self.subTest(value=value), self.assertRaises(ValueError):
                enclosing_png_size({'x': value, 'y': 0, 'width': 10, 'height': 5}, {'x': 0, 'y': 0})

    def test_mapped_normal_switches_are_not_duplicate_missing_source_controls(self):
        facts = DocumentFacts('<input aria-label="可点击日语分词" role="switch" type="checkbox" checked><input aria-label="显示日语注音" role="switch" type="checkbox"><button aria-label="显示词性标记" role="switch" aria-checked="true"></button>')
        self.assertIs(facts.switch_value('可点击日语分词'), True)
        self.assertIs(facts.switch_value('显示日语注音'), False)
        self.assertIs(facts.switch_value('显示词性标记'), True)
        with self.assertRaises(ValueError): facts.switch_value('未配置的行为')

    def test_raw_asset_tamper_and_type_lies_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve(); p = root / 'raw'; p.write_bytes(b'actual')
            ref = {'path': 'raw', 'sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': 6}
            self.assertEqual(p, artifact(root, ref))
            for changed in ({**ref, 'bytes': True}, {**ref, 'sha256': '0' * 64}, {**ref, 'path': '../raw'}):
                with self.subTest(changed=changed):
                    with self.assertRaises(ValueError): artifact(root, changed)
            p.write_bytes(b'changed')
            with self.assertRaises(ValueError): artifact(root, ref)

    def test_no_raw_report_cannot_become_a_passed_receipt(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); receipt = {'status': 'passed', 'native_verified': True}
            before = copy.deepcopy(receipt)
            self.assertTrue(inspect(root, receipt, {})); self.assertTrue(recompute(root, receipt, {}))
            self.assertEqual(before, receipt)

    def test_json_cannot_expand_the_code_owned_executor(self):
        with tempfile.TemporaryDirectory() as folder:
            context = {'adapter_id': CHECK_ID, 'command': ['node', 'scripts/report/verify_current_design_react.mjs'],
                       'layer': 'packaged_native', 'state_ids': sorted(STATE_IDS)}
            with self.assertRaisesRegex(ValueError, 'code-owned'): execute(Path(folder), context)
            context['layer'] = 'real_react'; context['state_ids'].append('unregistered-current-state')
            with self.assertRaisesRegex(ValueError, 'code-owned'): execute(Path(folder), context)

    def test_timeout_retains_actual_failure_and_never_creates_success_evidence(self):
        from current_design_react_adapter import COMMAND, RUNNER
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve(); output = root / '.build/run'; output.mkdir(parents=True)
            runner = root / RUNNER; runner.parent.mkdir(parents=True); runner.write_text('// fault-injection guard only\n')
            package = root / '.build/guard.zip'; package.write_bytes(b'guard-only; package inspector is isolated below')
            context = {'adapter_id': CHECK_ID, 'command': list(COMMAND), 'layer': 'real_react',
                       'state_ids': sorted(STATE_IDS), 'artifact_path': '.build/guard.zip',
                       'source_tree_sha256': 'a' * 64, 'output': '.build/run'}
            (output / 'request.json').write_text(json.dumps(context))
            failure = subprocess.TimeoutExpired(['guard-command'], 600, output=b'partial output', stderr=b'partial error')
            target = root / 'actual-node-executable'; target.write_text('guard: subprocess is fault-injected and never launched')
            alias = root / 'node-alias'; alias.symlink_to(target)
            with patch('package_identity.inspect_package', return_value={'guard_fixture': True}), patch('subprocess.run', side_effect=failure), patch('shutil.which', return_value=str(alias)):
                with self.assertRaisesRegex(ValueError, 'raw command failure retained'): execute(root, context)
            actual = json.loads((output / 'actual-command.json').read_text())
            self.assertIs(actual['timed_out'], True); self.assertIsNone(actual['exit_code'])
            self.assertEqual('partial output', actual['stdout']); self.assertEqual('partial error', actual['stderr'])
            self.assertEqual(str(target.resolve()), actual['command'][0])
            self.assertFalse((output / 'receipt.json').exists()); self.assertFalse((output / 'capture/verification.json').exists())

    def test_crc_valid_but_corrupt_compressed_pixels_return_structured_failure(self):
        def chunk(kind, data): return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
        broken = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
                  + chunk(b'IDAT', b'not a zlib stream') + chunk(b'IEND', b''))
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve(); out = root / '.build/run/capture'; out.mkdir(parents=True)
            pack = root / '.build/references'; pack.mkdir()
            def write(relative, value):
                p = root / relative; p.parent.mkdir(parents=True, exist_ok=True)
                p.write_bytes(value if isinstance(value, bytes) else json.dumps(value).encode()); return p
            def ref(p, base, size=True):
                value = {'path': p.relative_to(base).as_posix(), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
                return {**value, 'bytes': p.stat().st_size} if size else value
            source = 'a' * 64; executable = b'guard fixture only, not a real accepted package'
            package = root / '.build/guard.zip'
            with zipfile.ZipFile(package, 'w') as archive:
                archive.writestr('luna-build-identity.json', json.dumps({'source_tree_sha256': source, 'executable_sha256': hashlib.sha256(executable).hexdigest()}))
                archive.writestr('Luna.app/Contents/Resources/luna-source-identity.json', json.dumps({'source_tree_sha256': source}))
                archive.writestr('Luna.app/Contents/MacOS/luna-reader', executable)
            request = write('.build/run/capture/request-copy.json', {'artifact_path': '.build/guard.zip'})
            good = write('.build/run/capture/reference.png', png(1, 1)); bad = write('.build/run/capture/candidate.png', broken)
            diff = write('.build/run/capture/diff.png', png(1, 1))
            comparison = {'algorithm': 'strict_rgba_all_channels', 'tolerance': 0, 'max_different_pixels': 0,
                          'originals_unchanged': True, 'dimension_padding': 'diagnostic canvas only, never image resize', 'diff': ref(diff, out)}
            rows = [{'state_id': sid, 'pixel_comparison': comparison, 'reference_png': ref(good, out), 'target_png': ref(bad, out)} for sid in sorted(STATE_IDS)]
            write('docs/development/current-reference-catalogue.json', {'pack_root': '.build/references',
                  'entries': [{'state_id': sid, 'target_png': ref(good, out)} for sid in sorted(STATE_IDS)]})
            rp = write('.build/run/capture/verification.json', {'execution_nonce': 'b' * 32, 'layer': 'real_react', 'request': ref(request, out), 'results': rows})
            receipt = {'report': ref(rp, root, False), 'source_tree_sha256': source}
            errors = recompute(root, receipt, {'execution_nonce': 'b' * 32, 'artifact_sha256': hashlib.sha256(package.read_bytes()).hexdigest()})
            self.assertTrue(errors); self.assertTrue(any('independent recomputation invalid' in e for e in errors), errors)


if __name__ == '__main__': unittest.main()
