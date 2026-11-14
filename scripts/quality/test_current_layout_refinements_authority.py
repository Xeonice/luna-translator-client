"""Real source/descriptor mutations; guard fixtures grant no product acceptance."""
import copy
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout

import current_layout_refinements_authority as authority

ROOT = Path(__file__).resolve().parents[2]


class CurrentLayoutRefinementsAuthorityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = authority.approval_inputs(ROOT)

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        for relative in self.inputs:
            output = self.root / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, output)

    def descriptor(self, page='session', state='disconnected_saved', size='normal', theme='dark'):
        width, height = authority.VIEWPORTS['narrow' if size == 'narrow' else page]
        identity = f'{authority.SUPPLEMENT_ID}:{page}:{state}:{width}x{height}:{theme}'
        return authority.expected_descriptors()[identity]

    def assert_failed(self, descriptor=None):
        result = authority.assess(self.root, descriptor)
        self.assertEqual('failed', result['status'], result)
        self.assertIs(result['source_ready'], False)
        self.assertTrue(result['errors'], result)
        for key in ('reference_approval_granted', 'product_alignment_approved',
                    'real_component_verified', 'native_verified', 'release_accepted', 'next_stage_allowed'):
            self.assertIs(result[key], False, key)
        return result

    def test_exact_approval_is_source_only_and_manifest_stays_pending(self):
        result = authority.assess(self.root)
        self.assertEqual('approved_by_user', result['status'], result)
        self.assertIs(result['source_ready'], True)
        self.assertEqual(9, result['approved_source_count'])
        self.assertEqual({'complete': 61, 'C1': 3, 'C35': 5}, result['parent_source_counts'])
        self.assertEqual('not_inspected', result['component_evidence_status'])
        for key in ('reference_approval_granted', 'product_alignment_approved',
                    'real_component_verified', 'native_verified', 'release_accepted', 'next_stage_allowed'):
            self.assertIs(result[key], False, key)
        manifest = json.loads((self.root / authority.MANIFEST['path']).read_text())
        self.assertEqual('pending_user_review', manifest['status'])

    def test_all_code_owned_descriptor_variants_are_source_only(self):
        descriptors = authority.expected_descriptors()
        self.assertEqual(24, len(descriptors))
        for identity, descriptor in descriptors.items():
            with self.subTest(descriptor=identity):
                result = authority.assess(self.root, descriptor)
                self.assertEqual('approved_by_user', result['status'], result)
                self.assertIs(result['descriptor_validated'], True)
                self.assertIs(result['real_component_verified'], False)
                self.assertEqual('design_only', result['descriptor']['evidence_role'])

    def test_exact_supplement_mapping_derives_three_entries_and_grants_no_ui_success(self):
        mapping = authority.expected_mapping()
        self.assertEqual(mapping, authority.validate_mapping(mapping))
        entries = {descriptor['page']: descriptor['entry']
                   for descriptor in authority.expected_descriptors().values()}
        self.assertEqual(entries, mapping['source_entries'])
        self.assertEqual({'history', 'settings', 'session'}, set(entries))
        self.assertEqual(9, mapping['source_count'])
        self.assertEqual(len(authority.expected_sources()), mapping['source_count'])
        self.assertEqual({'path': authority.RECORD, 'sha256': authority.RECORD_SHA256,
                          'bytes': authority.RECORD_BYTES}, mapping['approval_record'])
        self.assertEqual(authority.MANIFEST, mapping['manifest'])
        result = authority.assess(self.root)
        self.assertIs(result['source_ready'], True)
        self.assertIs(result['real_component_verified'], False)
        self.assertIs(result['native_verified'], False)
        self.assertIs(result['release_accepted'], False)

    def test_supplement_mapping_requires_every_exact_field_and_source_count(self):
        original = authority.expected_mapping()
        for key in original:
            with self.subTest(missing=key):
                mapping = copy.deepcopy(original)
                del mapping[key]
                with self.assertRaises(ValueError):
                    authority.validate_mapping(mapping)
        for change in ({'supplement_id': 'preview-contract-20261104'}, {'source_count': 61},
                       {'source_count': 3}, {'source_count': 9.0}, {'source_count': True},
                       {'unknown': 'extra'}, {'approved_sources': authority.expected_sources()}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                authority.validate_mapping({**original, **change})

    def test_supplement_mapping_rejects_wrong_record_and_manifest_identity(self):
        original = authority.expected_mapping()
        for key in ('approval_record', 'manifest'):
            for field, value in (('path', 'docs/development/design-approval-complete-20261031.json'),
                                 ('sha256', '0' * 64), ('bytes', 1), ('bytes', float(original[key]['bytes']))):
                with self.subTest(record=key, field=field, value=value):
                    mapping = copy.deepcopy(original)
                    mapping[key][field] = value
                    with self.assertRaises(ValueError):
                        authority.validate_mapping(mapping)

    def test_supplement_mapping_rejects_missing_unknown_and_parent_entries(self):
        original = authority.expected_mapping()
        for page in original['source_entries']:
            for entry in ('docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html',
                          'docs/design/first-product-v1/candidates/history-session-paging-20261031/index.html',
                          authority.PREFIX + 'unknown.html'):
                with self.subTest(page=page, entry=entry):
                    mapping = copy.deepcopy(original)
                    mapping['source_entries'][page] = entry
                    with self.assertRaises(ValueError):
                        authority.validate_mapping(mapping)
            mapping = copy.deepcopy(original)
            del mapping['source_entries'][page]
            with self.subTest(missing_page=page), self.assertRaises(ValueError):
                authority.validate_mapping(mapping)
        mapping = copy.deepcopy(original)
        mapping['source_entries']['library'] = authority.ENTRY
        with self.assertRaises(ValueError):
            authority.validate_mapping(mapping)

    def test_supplement_mapping_cannot_self_grant_execution_or_acceptance(self):
        original = authority.expected_mapping()
        for key in ('source_ready', 'reference_approval_granted', 'product_alignment_approved',
                    'real_component_verified', 'native_verified', 'release_accepted', 'next_stage_allowed'):
            with self.subTest(claim=key), self.assertRaises(ValueError):
                authority.validate_mapping({**original, key: True})
        for value in (None, [], {}, 'approved', {'status': 'passed'},
                      {**original, 'receipt': {'status': 'passed'}}, {**original, 'unknown': set()}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                authority.validate_mapping(value)

    def test_mutating_supplement_mapping_cannot_install_new_authority(self):
        original = authority.expected_mapping()
        checked = authority.validate_mapping(original)
        checked['manifest']['bytes'] = 1
        checked['source_entries']['history'] = authority.ENTRY
        self.assertEqual(original, authority.expected_mapping())
        with self.assertRaises(ValueError):
            authority.validate_mapping(checked)

    def test_missing_approval_is_rejected(self):
        (self.root / authority.RECORD).unlink()
        self.assert_failed()

    def test_self_declared_approval_and_scope_expansion_are_rejected(self):
        path = self.root / authority.RECORD
        original = path.read_bytes()
        data = json.loads(original)
        changes = [
            {'status': 'approved_by_user', 'user_confirmation': '我自己批准'},
            {'approved_source_count': 10},
            {'approved_viewports': {**data['approved_viewports'], 'settings': [1040, 760]}},
            {'not_granted': []},
            {'approved_sources': {**data['approved_sources'], 'other.html': {'sha256': '0' * 64, 'bytes': 1}}},
        ]
        for change in changes:
            with self.subTest(change=change):
                path.write_text(json.dumps({**data, **change}, ensure_ascii=False))
                self.assert_failed()
        path.write_bytes(original)

    def test_identical_json_reencoding_cannot_replace_reviewed_raw_approval(self):
        path = self.root / authority.RECORD
        path.write_text(json.dumps(json.loads(path.read_text()), ensure_ascii=False, separators=(',', ':')))
        self.assert_failed()

    def test_every_source_hash_and_byte_length_are_enforced(self):
        for relative in authority.expected_sources():
            path = self.root / relative
            original = path.read_bytes()
            for mutation in (original + b'\n', bytes([original[0] ^ 1]) + original[1:]):
                with self.subTest(source=relative, length=len(mutation)):
                    path.write_bytes(mutation)
                    self.assert_failed()
            path.write_bytes(original)

    def test_missing_source_and_symlink_source_are_rejected(self):
        path = self.root / (authority.PREFIX + 'session.html')
        original = path.read_bytes()
        path.unlink()
        self.assert_failed()
        alias = self.root / 'same-bytes.html'
        alias.write_bytes(original)
        path.symlink_to(alias)
        self.assert_failed()

    def test_manifest_cannot_refresh_hashes_or_pending_status(self):
        path = self.root / authority.MANIFEST['path']
        original = path.read_bytes()
        data = json.loads(original)
        for changes in ({'status': 'approved_by_user'}, {'sources': {}}, {'runtime_package_modified': True}):
            with self.subTest(changes=changes):
                path.write_text(json.dumps({**data, **changes}))
                self.assert_failed()
        path.write_bytes(original)

    def test_each_parent_approval_is_byte_pinned(self):
        for relative in authority.PARENTS:
            with self.subTest(parent=relative):
                path = self.root / relative
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                self.assert_failed()
                path.write_bytes(original)

    def test_parent_61_C1_and_C35_sources_cannot_drift(self):
        paths = [
            'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html',
            'docs/design/first-product-v1/candidates/history-session-paging-20261031/history.css',
            'docs/design/first-product-v1/client-layout-20261105/client-c1.css',
            'docs/design/first-product-v1/candidates/preview-contract-20261104/preview-contract.js',
        ]
        for relative in paths:
            with self.subTest(source=relative):
                path = self.root / relative
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                self.assert_failed()
                path.write_bytes(original)

    def test_old_guided_parent_decision_and_runtime_asset_cannot_drift(self):
        for relative in ('docs/development/design-approval-guided-20261031.json',
                         'docs/design/first-product-v1/assets/wallpaper-frame.png'):
            with self.subTest(source=relative):
                path = self.root / relative
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                self.assert_failed()
                path.write_bytes(original)

    def test_independent_source_review_must_match_approved_identity(self):
        path = self.root / authority.PRIOR_REVIEW['path']
        path.write_bytes(path.read_bytes() + b'\n')
        self.assert_failed()

    def test_unknown_directory_source_and_entry_are_rejected(self):
        for entry in ('docs/design/first-product-v1/candidates/other/history.html',
                      authority.PREFIX + '../history-session-paging-20261031/index.html',
                      '/tmp/history.html', authority.ENTRY):
            with self.subTest(entry=entry):
                descriptor = self.descriptor()
                descriptor['entry'] = entry
                self.assert_failed(descriptor)

    def test_unknown_state_theme_and_dimensions_are_rejected(self):
        changes = [{'state': 'connected'}, {'page': 'library'}, {'viewport': [1040, 760]},
                   {'viewport': [1040.0, 720]}, {'source_count': True},
                   {'query': {'theme': 'unreviewed'}}, {'id': 'unknown'}]
        for change in changes:
            with self.subTest(change=change):
                descriptor = self.descriptor()
                descriptor.update(change)
                self.assert_failed(descriptor)

    def test_source_descriptor_cannot_assert_product_or_release_capability(self):
        for change in ({'kind': 'real-react-component-result/v1'},
                       {'evidence_role': 'real_component'}, {'status': 'passed'},
                       {'real_component_verified': True}, {'native_verified': True},
                       {'reference_approval_granted': True}, {'release_accepted': True},
                       {'production_source_before': {}}, {'receipt': {'status': 'passed'}}):
            with self.subTest(change=change):
                descriptor = self.descriptor()
                descriptor.update(change)
                self.assert_failed(descriptor)

    def test_older_reference_or_C35_candidate_cannot_substitute_for_new_source(self):
        for entry in ('docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html',
                      'docs/design/first-product-v1/candidates/preview-contract-20261104/index.html',
                      '.build/current-client-execution-20261105/cp00-layout-candidate-v1/index.html'):
            with self.subTest(entry=entry):
                descriptor = self.descriptor()
                descriptor.update(entry=entry, evidence_role='real_component')
                self.assert_failed(descriptor)

    def test_mutating_returned_descriptor_does_not_install_new_scope(self):
        descriptor = self.descriptor()
        original = copy.deepcopy(descriptor)
        descriptor['viewport'][0] = 999
        descriptor['query']['compare'] = 'before'
        self.assert_failed(descriptor)
        self.assertEqual(original, self.descriptor())
        self.assert_failed([original])

    def test_empty_or_whole_receipt_json_is_not_a_descriptor(self):
        for value in ({}, [], 'approved', {'status': 'passed', 'captures': []}):
            with self.subTest(value=value):
                self.assert_failed(value)

    def test_cli_writes_source_only_result_and_returns_failure_for_invalid_descriptor(self):
        output = self.root / '.build/source-authority.json'
        args = ['--root', str(self.root), '--output', str(output)]
        with redirect_stdout(io.StringIO()):
            self.assertEqual(0, authority.main(args))
        result = json.loads(output.read_text())
        self.assertIs(result['source_ready'], True)
        self.assertIs(result['real_component_verified'], False)
        descriptor = self.root / 'descriptor.json'
        descriptor.write_text('{"status":"passed","status":"passed"}')
        with redirect_stdout(io.StringIO()):
            self.assertEqual(1, authority.main([*args, '--descriptor', str(descriptor)]))
        self.assertIs(json.loads(output.read_text())['source_ready'], False)

    def test_cli_cannot_overwrite_approved_source_or_write_through_symlink(self):
        for output in (self.root / authority.ENTRY, self.root / '.build/escape/source.json'):
            if output.name == 'source.json':
                outside = self.root / 'outside'
                outside.mkdir()
                link = output.parent
                link.parent.mkdir(parents=True, exist_ok=True)
                link.symlink_to(outside, target_is_directory=True)
            with self.subTest(output=output), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                authority.main(['--root', str(self.root), '--output', str(output)])

    def test_executable_cli_uses_actual_return_code(self):
        result = subprocess.run([sys.executable, str(ROOT / 'scripts/quality/current_layout_refinements_authority.py'),
                                 '--root', str(self.root)], text=True, capture_output=True)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIs(json.loads(result.stdout)['source_ready'], True)
        (self.root / authority.RECORD).unlink()
        result = subprocess.run([sys.executable, str(ROOT / 'scripts/quality/current_layout_refinements_authority.py'),
                                 '--root', str(self.root)], text=True, capture_output=True)
        self.assertEqual(1, result.returncode, result.stderr)
        self.assertIs(json.loads(result.stdout)['source_ready'], False)


if __name__ == '__main__':
    unittest.main()
