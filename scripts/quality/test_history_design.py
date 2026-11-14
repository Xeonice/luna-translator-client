"""Adversarial metadata/evidence guards. These tests do not execute the product."""
import copy
import hashlib
import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest
import zlib

import history_design as h


class HistoryDesignGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        record = json.loads((h.ROOT / h.authority.RECORD).read_text())
        inputs = {*record['design_sources'], *h.INHERITED_RUNTIME_ASSETS, h.authority.RECORD, h.MANIFEST,
                  h.CHECKER, h.TESTS, h.RUNNER, *h.DIRECT_DEPENDENCIES}
        inputs.update(h.CANDIDATE + '/' + name for name in ('index.html', 'history.js', 'history.css'))
        for name in inputs:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(h.ROOT / name, target)
        self.plan = {'schema_version': 1, 'rows': [
            {'id': identifier, 'product_status': 'not_run'} for identifier in sorted(h.HP_IDS)]}
        self.write(h.PLAN, self.plan)
        self.out = self.root / '.build/run'
        self.out.mkdir(parents=True)

    def write(self, name, data):
        target = self.root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')

    def manifest(self, mutate):
        value = json.loads((self.root / h.MANIFEST).read_text())
        mutate(value)
        self.write(h.MANIFEST, value)

    def receipt(self):
        def chunk(kind, data):
            return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
        width, height = 400, 300
        data = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress((b'\0' + b'\xff\xff\xff\xff' * width) * height)) + chunk(b'IEND', b''))
        (self.out / 'capture.png').write_bytes(data)
        command = ['node', str(self.root / h.RUNNER), str(self.root), str(self.out), str(self.out / 'context.json')]
        context = {'schema_version': 1, 'execution_nonce': 'a' * 32, **h.source_context(self.root), 'command': command}
        checks = [{'id': identifier, 'status': 'passed', 'assertions': 1, 'skipped': 0,
                   'screenshots': [identifier + '-' + name + '.png' for name in h.CAPTURES[identifier]],
                   'observations': {key: 'guard-fixture' for key in h.OBSERVATION_KEYS[identifier]}}
                  for identifier in sorted(h.HP_IDS)]
        for row in checks:
            for name in row['screenshots']:
                (self.out / name).write_bytes(data)
        next(c for c in checks if c['id'] == 'HP-10')['observations'] = {'deep_link_states': h.REVIEW_STATES}
        report = {'schema_version': 1, 'kind': 'history-design-execution/v1', 'layer': 'design',
                  'status': 'passed', 'claims_product': False, 'product_tests_executed': False,
                  'real_service_executed': False, 'exit_code': 0, 'assertions': 12,
                  'skipped_assertions': 0, 'checks': checks, 'page_errors': [],
                  'console_errors': [], 'external_requests': [], 'unbound_requests': [],
                  **{k: context[k] for k in ('execution_nonce', 'source_identity', 'candidate_source_hash',
                                             'parent_source_hash', 'parent_manifest_sha256', 'source_hashes', 'command')},
                  'source_hashes_after': copy.deepcopy(context['source_hashes']),
                  'screenshots': [{'path': name, 'bytes': len(data), 'sha256': h._sha(data),
                                   'width': width, 'height': height} for row in checks for name in row['screenshots']]}
        for name in ('stdout.log', 'stderr.log'):
            (self.out / name).write_text('')
        invocation = {'command': command, 'exit_code': 0,
                      **{name + '_sha256': h._sha(b'') for name in ('stdout.log', 'stderr.log')}}
        return report, context, invocation

    def evidence(self, mutate=None, context_mutate=None, invocation_mutate=None):
        report, context, invocation = self.receipt()
        if mutate:
            mutate(report)
        if context_mutate:
            context_mutate(context)
        if invocation_mutate:
            invocation_mutate(invocation)
        (self.out / 'report.json').write_text(json.dumps(report))
        return h.validate_execution(self.root, self.out, context, invocation)

    def test_valid_metadata_fixture(self):
        self.assertEqual(h.validate_contract(self.root), [])

    def test_actual_current_contract(self):
        self.assertEqual(h.validate_contract(h.ROOT), [])

    def test_manifest_cannot_self_approve_new_sources(self):
        self.manifest(lambda m: m.update(status='approved_by_user'))
        self.assertTrue(h.validate_contract(self.root))

    def test_missing_review_state_rejected(self):
        self.manifest(lambda m: m['review_states'].remove('empty'))
        self.assertTrue(h.validate_contract(self.root))

    def test_original_parent_change_cannot_be_rebound_by_manifest(self):
        parent = 'docs/design/first-product-v1/app.js'
        (self.root / parent).write_text('changed')
        self.manifest(lambda m: m['parent_sources'].update({parent: h._sha(b'changed')}))
        self.assertTrue(h.validate_contract(self.root))

    def test_approval_record_is_code_pinned(self):
        value = json.loads((self.root / h.authority.RECORD).read_text())
        value['design_closure_sha256'] = '0' * 64
        self.write(h.authority.RECORD, value)
        self.assertTrue(h.validate_contract(self.root))

    def test_unbound_candidate_source_rejected(self):
        (self.root / h.CANDIDATE / 'history.js').write_text('changed')
        self.assertTrue(h.validate_contract(self.root))

    def test_rebinding_html_cannot_load_unbound_local_script(self):
        source = self.root / h.CANDIDATE / 'index.html'
        source.write_text(source.read_text() + '<script src="unbound.js"></script>')
        (self.root / 'docs/design/first-product-v1/unbound.js').write_text('window.unbound = true;')
        self.manifest(lambda m: m['owned_sources'].update({'index.html': h._sha(source.read_bytes())}))
        self.assertTrue(h.validate_contract(self.root))

    def test_inherited_wallpaper_is_separately_bound(self):
        relative = next(iter(h.INHERITED_RUNTIME_ASSETS))
        self.assertIn(relative, h.source_context(self.root)['source_hashes'])
        (self.root / relative).write_bytes(b'changed')
        self.assertTrue(h.validate_contract(self.root))

    def test_illegal_owned_path_rejected(self):
        self.manifest(lambda m: m['owned_sources'].update({'../escape.js': 'a' * 64}))
        self.assertTrue(h.validate_contract(self.root))

    def test_extra_source_cannot_hide_outside_manifest(self):
        (self.root / h.CANDIDATE / 'unexpected.js').write_text('extra')
        self.assertTrue(h.validate_contract(self.root))

    def test_product_plan_cannot_be_promoted(self):
        self.plan['rows'][0]['product_status'] = 'passed'
        self.assertTrue(h.validate_plan(self.plan))

    def test_plan_missing_or_duplicate_state_rejected(self):
        self.plan['rows'][0] = copy.deepcopy(self.plan['rows'][1])
        self.assertTrue(h.validate_plan(self.plan))

    def test_valid_evidence_fixture_only(self):
        self.assertEqual(self.evidence(), [])

    def test_missing_executed_hp_state_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['checks'].pop()))

    def test_zero_assertion_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['checks'][0].update(assertions=0)))

    def test_skipped_assertion_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['checks'][0].update(skipped=1)))

    def test_numeric_false_is_not_typed_false(self):
        self.assertTrue(self.evidence(lambda r: r.update(claims_product=0)))

    def test_string_false_is_not_typed_false(self):
        self.assertTrue(self.evidence(lambda r: r.update(product_tests_executed='false')))

    def test_product_claim_rejected(self):
        self.assertTrue(self.evidence(lambda r: r.update(claims_product=True)))

    def test_old_nonce_rejected(self):
        self.assertTrue(self.evidence(lambda r: r.update(execution_nonce='b' * 32)))

    def test_old_source_identity_rejected(self):
        self.assertTrue(self.evidence(lambda r: r.update(source_identity='b' * 64)))

    def test_candidate_change_and_manifest_rebind_invalidates_old_execution(self):
        report, context, invocation = self.receipt()
        source = self.root / h.CANDIDATE / 'history.js'
        source.write_text(source.read_text() + '\n// candidate revision\n')
        self.manifest(lambda m: m['owned_sources'].update({'history.js': h._sha(source.read_bytes())}))
        self.assertEqual(h.validate_contract(self.root), [])
        self.assertEqual(h.authority.source_approval(self.root)['status'], 'approved_by_user')
        self.assertEqual(json.loads((self.root / h.MANIFEST).read_text())['status'], 'pending_review')
        (self.out / 'report.json').write_text(json.dumps(report))
        self.assertTrue(h.validate_execution(self.root, self.out, context, invocation))

    def test_direct_approval_checker_change_invalidates_old_execution(self):
        report, context, invocation = self.receipt()
        relative = h.DIRECT_DEPENDENCIES[0]
        self.assertIn(relative, context['source_hashes'])
        checker = self.root / relative
        checker.write_text(checker.read_text() + '\n# changed direct validation dependency\n')
        (self.out / 'report.json').write_text(json.dumps(report))
        self.assertTrue(h.validate_execution(self.root, self.out, context, invocation))

    def test_missing_screenshot_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['screenshots'][0].update(path='missing.png')))

    def test_screenshot_path_escape_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['screenshots'][0].update(path='../capture.png')))

    def test_screenshot_hash_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['screenshots'][0].update(sha256='0' * 64)))

    def test_unbound_check_capture_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['checks'][0].update(screenshots=[])))

    def test_nine_deep_links_cannot_be_replaced_by_labels(self):
        self.assertTrue(self.evidence(lambda r: next(c for c in r['checks'] if c['id'] == 'HP-10')['observations'].update(deep_link_states=[])))

    def test_missing_one_deep_link_screenshot_is_rejected(self):
        self.assertTrue(self.evidence(lambda r: next(c for c in r['checks'] if c['id'] == 'HP-10')['screenshots'].pop()))

    def test_missing_loading_screenshot_is_rejected(self):
        self.assertTrue(self.evidence(lambda r: next(c for c in r['checks'] if c['id'] == 'HP-05')['screenshots'].remove('HP-05-records-loading.png')))

    def test_unbound_file_request_is_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['unbound_requests'].append('file:///tmp/unbound.js')))

    def test_external_network_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['external_requests'].append('https://example.invalid')))

    def test_console_runtime_error_rejected(self):
        self.assertTrue(self.evidence(lambda r: r['console_errors'].append('failure')))

    def test_actual_command_cannot_self_register_arbitrary_adapter(self):
        self.assertTrue(self.evidence(context_mutate=lambda c: c.update(command=['true']),
                                      invocation_mutate=lambda i: i.update(command=['true'])))

    def test_nonzero_actual_exit_rejected(self):
        self.assertTrue(self.evidence(invocation_mutate=lambda i: i.update(exit_code=1)))

    def test_no_json_command_output_rejected(self):
        _, context, invocation = self.receipt()
        (self.out / 'report.json').write_text('all passed without JSON')
        self.assertTrue(h.validate_execution(self.root, self.out, context, invocation))

    def test_truncated_png_rejected(self):
        with self.assertRaises(ValueError):
            h.png_info(b'\x89PNG\r\n\x1a\n' + b'\0' * 20)

    def test_output_reuse_rejected_before_node_runs(self):
        with self.assertRaises(ValueError):
            h.execute(self.root, self.out)

    def test_output_escape_and_symlink_rejected(self):
        with self.assertRaises(ValueError):
            h.output_path(self.root, 'reports/bad')
        (self.root / '.build/link').symlink_to(self.out, target_is_directory=True)
        with self.assertRaises(ValueError):
            h.output_path(self.root, '.build/link/new')


if __name__ == '__main__':
    unittest.main()
