"""Actual source/mapping mutations for source-only approval, not product tests."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import current_general_settings_authority as authority

ROOT = Path(__file__).resolve().parents[2]


class CurrentGeneralSettingsAuthorityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = authority.approval_inputs(ROOT) | {authority.LEGACY_TEST['path']}

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        for relative in self.inputs:
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)

    def assert_not_granted(self, result):
        self.assertEqual('not_inspected', result['component_evidence_status'])
        self.assertEqual('incomplete', result['product_completion'])
        for key in ('reference_approval_granted', 'product_alignment_approved',
                    'real_component_verified', 'native_verified', 'release_accepted',
                    'next_stage_allowed'):
            self.assertIs(result[key], False, key)

    def assert_failed(self, root=None, mapping=None):
        result = authority.assess(self.root if root is None else root, mapping)
        self.assertEqual('failed', result['status'], result)
        self.assertIs(result['source_ready'], False, result)
        self.assertTrue(result['errors'], result)
        self.assert_not_granted(result)
        return result

    def changed_file_rejected(self, relative):
        path = self.root / relative
        original = path.read_bytes()
        try:
            path.write_bytes(original + b'\n')
            self.assert_failed()
        finally:
            path.write_bytes(original)

    def cli(self, *args):
        return subprocess.run([sys.executable, str(ROOT / 'scripts/quality/current_general_settings_authority.py'),
                               '--root', str(self.root), *map(str, args)], capture_output=True, text=True)

    def test_exact_permanent_closure_requires_no_old_build_directory(self):
        self.assertFalse((self.root / '.build').exists())
        result = authority.assess(self.root)
        self.assertIs(result['source_ready'], True, result)
        self.assertEqual('approved_by_user', result['status'])
        self.assertEqual(6, result['approved_source_count'])
        self.assertEqual(5, result['captured_source_count'])
        self.assertEqual({'complete': 61, 'C1': 3, 'C35': 5, 'layout': 9}, result['parent_source_counts'])
        self.assertIs(result['mapping_validated'], False)
        self.assert_not_granted(result)
        self.assertTrue(all(not relative.startswith('.build/') for relative in self.inputs))

    def test_exact_mapping_is_validated_without_ui_or_runtime_grants(self):
        mapping = authority.expected_mapping()
        result = authority.assess(self.root, mapping)
        self.assertIs(result['source_ready'], True, result)
        self.assertIs(result['mapping_validated'], True)
        self.assertEqual(mapping, result['mapping'])
        self.assertEqual(mapping, authority.validate_mapping(mapping))
        self.assert_not_granted(result)

    def test_mapping_requires_each_exact_field(self):
        expected = authority.expected_mapping()
        for key in expected:
            with self.subTest(missing=key):
                mapping = copy.deepcopy(expected)
                del mapping[key]
                with self.assertRaises(ValueError):
                    authority.validate_mapping(mapping)
                self.assert_failed(mapping=mapping)

    def test_mapping_rejects_wrong_record_or_manifest_identity_and_types(self):
        expected = authority.expected_mapping()
        for field in ('approval_record', 'manifest'):
            for key, value in (('path', authority.PREFIX + 'unknown.html'), ('sha256', '0' * 64),
                               ('bytes', 1), ('bytes', float(expected[field]['bytes'])),
                               ('bytes', True)):
                with self.subTest(field=field, key=key, value=value):
                    mapping = copy.deepcopy(expected)
                    mapping[field][key] = value
                    with self.assertRaises(ValueError):
                        authority.validate_mapping(mapping)

    def test_mapping_rejects_unknown_parent_paths_and_wrong_scope(self):
        expected = authority.expected_mapping()
        for delta in (
            {'supplement_id': 'layout-refinements-20261107'},
            {'entry': 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html'},
            {'entry': '../settings.html'}, {'entry': 'file:///settings.html'},
            {'source_count': 5}, {'viewports': {'normal': [1040, 760], 'narrow': [760, 560]}},
            {'themes': ['dark']}, {'zoom': [1]}, {'evidence_role': 'production_component'},
            {'page': 'history'},
        ):
            with self.subTest(delta=delta), self.assertRaises(ValueError):
                authority.validate_mapping({**expected, **delta})

    def test_mapping_rejects_non_json_scalars_and_containers(self):
        expected = authority.expected_mapping()
        for mapping in (None, [], {}, 'approved', 6, True, {1: 'bad'},
                        {**expected, 'source_count': True}, {**expected, 'source_count': 6.0},
                        {**expected, 'themes': ('dark', 'light')},
                        {**expected, 'zoom': [1.0, 2]},
                        {**expected, 'zoom': [True, 2]},
                        {**expected, 'zoom': [float('nan'), 2]},
                        {**expected, 'zoom': [1, float('inf')]},
                        {**expected, 'unknown': set()}):
            with self.subTest(mapping=mapping), self.assertRaises(ValueError):
                authority.validate_mapping(mapping)

    def test_mapping_cannot_self_grant_approval_or_execution(self):
        expected = authority.expected_mapping()
        for key in ('status', 'source_ready', 'product_alignment_approved', 'real_component_verified',
                    'native_verified', 'release_accepted', 'product_completion', 'next_stage_allowed',
                    'reference_approval_granted', 'receipt'):
            with self.subTest(key=key), self.assertRaises(ValueError):
                authority.validate_mapping({**expected, key: True})

    def test_mapping_return_values_cannot_mutate_authority(self):
        original = authority.expected_mapping()
        checked = authority.validate_mapping(original)
        checked['approval_record']['bytes'] = 1
        checked['viewports']['normal'][0] = 1040
        expected = authority.expected_mapping()
        self.assertEqual(original, expected)
        with self.assertRaises(ValueError):
            authority.validate_mapping(checked)
        original['themes'].append('invented')
        self.assertNotEqual(original, authority.expected_mapping())

    def test_missing_approval_rejected(self):
        (self.root / authority.RECORD).unlink()
        self.assert_failed()

    def test_reencoded_approval_does_not_replace_raw_user_decision(self):
        path = self.root / authority.RECORD
        path.write_text(json.dumps(json.loads(path.read_text()), ensure_ascii=False, separators=(',', ':')))
        self.assert_failed()

    def test_approval_cannot_change_quote_scope_or_self_authorize(self):
        path = self.root / authority.RECORD
        original = path.read_bytes()
        data = json.loads(original)
        for delta in ({'user_confirmation': '我自己批准'}, {'approved_scope': []},
                      {'viewports': {'normal': [1040, 760]}}, {'not_granted': []},
                      {'real_component_verified': True}, {'status': 'pending_user_review'}):
            with self.subTest(delta=delta):
                path.write_text(json.dumps({**data, **delta}, ensure_ascii=False))
                self.assert_failed()
        path.write_bytes(original)

    def test_every_promoted_source_hash_and_byte_size_is_enforced(self):
        for relative in authority.PROMOTED_SOURCES:
            with self.subTest(relative=relative):
                self.changed_file_rejected(relative)

    def test_every_captured_source_and_capture_record_is_enforced(self):
        relatives = [entry['captured']['path'] for entry in authority.RELOCATION]
        relatives += [entry['path'] for entry in authority.CAPTURED_RECORDS]
        for relative in relatives:
            with self.subTest(relative=relative):
                self.changed_file_rejected(relative)

    def test_manifest_cannot_widen_relocation_or_source_set(self):
        path = self.root / authority.MANIFEST
        data = json.loads(path.read_bytes())
        data['sources'][authority.PREFIX + 'extra.js'] = {'sha256': '0' * 64, 'bytes': 0}
        data['promotion'][1]['transformation'] = 'arbitrary_rewrite'
        data['product_accepted'] = True
        path.write_text(json.dumps(data, ensure_ascii=False))
        self.assert_failed()

    def test_html_relocation_only_allows_two_recorded_literal_urls(self):
        html = self.root / authority.ENTRY
        data = html.read_bytes()
        self.assertIn(b'../../material-tokens.css', data)
        self.assertIn(b'../../icons.js', data)
        html.write_bytes(data.replace(b'../../icons.js', b'../../different.js'))
        self.assert_failed()

    def test_material_and_icon_dependencies_are_not_mutable(self):
        for entry in authority.RUNTIME_DEPENDENCIES:
            with self.subTest(path=entry['path']):
                self.changed_file_rejected(entry['path'])

    def test_parent_layout_and_complete_c1_c35_records_are_pinned(self):
        for relative in [authority.PARENT_APPROVAL['path'], *authority.PARENT_RECORDS]:
            with self.subTest(path=relative):
                self.changed_file_rejected(relative)
        self.changed_file_rejected('docs/design/first-product-v1/candidates/layout-refinements-20261107/settings.html')

    def test_same_bytes_leaf_symlink_is_not_evidence(self):
        path = self.root / authority.ENTRY
        other = self.root / 'same-bytes.html'
        shutil.copyfile(path, other)
        path.unlink()
        path.symlink_to(other)
        self.assert_failed()

    def test_symlink_ancestor_is_not_a_source(self):
        path = self.root / authority.PREFIX.rstrip('/')
        real = self.root / 'detached-source'
        path.rename(real)
        path.symlink_to(real, target_is_directory=True)
        self.assert_failed()

    def test_relative_parent_traversal_missing_and_symlink_roots_rejected(self):
        for root in (Path('.'), self.root / '..' / self.root.name, self.root / 'missing'):
            with self.subTest(root=str(root)):
                self.assert_failed(root=root)
        with tempfile.TemporaryDirectory() as link_directory:
            link = Path(link_directory).resolve() / 'root-link'
            link.symlink_to(self.root, target_is_directory=True)
            self.assert_failed(root=link)

    def test_canonical_source_paths_only(self):
        fact = authority.PINNED_INPUTS[authority.RECORD]
        for relative in ('', '/tmp/source', '../source', './source', 'a//b', 'a/../b',
                         'a\\b', 'a:b', 'a?query', 'a#fragment', 'a/./b', ' source'):
            with self.subTest(path=relative), self.assertRaises(ValueError):
                authority._file(self.root, relative, fact)

    def test_capture_pending_labels_are_retained_without_old_build_inputs(self):
        for entry in authority.CAPTURED_RECORDS:
            captured = json.loads((self.root / entry['path']).read_bytes())
            self.assertEqual('pending', captured['source_approval'])
            self.assertIs(captured['runtime_modified_by_candidate'], False)
        self.assertFalse((self.root / '.build').exists())
        self.assertIs(authority.assess(self.root)['source_ready'], True)

    def test_cli_positive_actual_mapping_has_source_only_result(self):
        mapping = self.root / 'mapping.json'
        mapping.write_text(json.dumps(authority.expected_mapping(), ensure_ascii=False))
        output = self.root / '.build/authority/assessment.json'
        process = self.cli('--mapping', mapping, '--output', output)
        self.assertEqual(0, process.returncode, process.stderr)
        result = json.loads(process.stdout)
        self.assertEqual(result, json.loads(output.read_text()))
        self.assertIs(result['source_ready'], True)
        self.assertIs(result['mapping_validated'], True)
        self.assert_not_granted(result)

    def test_cli_actual_missing_and_mutated_inputs_exit_nonzero(self):
        for relative in (authority.RECORD, authority.MANIFEST, authority.ENTRY,
                         authority.RELOCATION[1]['captured']['path'], authority.RUNTIME_DEPENDENCIES[0]['path']):
            with self.subTest(relative=relative):
                path = self.root / relative
                original = path.read_bytes()
                path.unlink()
                process = self.cli()
                self.assertEqual(1, process.returncode, process.stderr)
                result = json.loads(process.stdout)
                self.assertIs(result['source_ready'], False)
                self.assertTrue(result['errors'])
                self.assert_not_granted(result)
                path.write_bytes(original)

    def test_cli_malformed_duplicate_nan_unknown_and_null_mapping_rejected(self):
        mapping = self.root / 'bad-mapping.json'
        for raw in ('{', '{"entry":1,"entry":2}', '{"zoom":NaN}',
                    json.dumps({**authority.expected_mapping(), 'native_verified': True}), 'null'):
            with self.subTest(raw=raw):
                mapping.write_text(raw)
                process = self.cli('--mapping', mapping)
                self.assertEqual(1, process.returncode, process.stderr)
                result = json.loads(process.stdout)
                self.assertIs(result['source_ready'], False)
                self.assertTrue(result['errors'])
                self.assert_not_granted(result)

    def test_cli_output_cannot_escape_or_replace_symlink(self):
        outside = self.root / 'outside.json'
        process = self.cli('--output', outside)
        self.assertEqual(2, process.returncode)
        self.assertFalse(outside.exists())
        actual = self.root / 'actual.json'
        actual.write_text('untouched')
        output = self.root / '.build/linked.json'
        output.parent.mkdir(parents=True)
        output.symlink_to(actual)
        process = self.cli('--output', output)
        self.assertEqual(2, process.returncode)
        self.assertEqual('untouched', actual.read_text())


    def test_exact_test_migration_preserves_original_and_grants_no_execution(self):
        original = authority.expected_test_supersession()
        checked = authority.validate_test_supersession(original, self.root)
        self.assertEqual(original, checked)
        self.assertEqual(5, checked['superseded_check']['case_count'])
        self.assertEqual('settings-layout-refinement-tests', checked['superseded_check']['id'])
        self.assertEqual('general-settings-structure-tests', checked['replacement_check']['id'])
        self.assertIs(checked['replacement_execution_required'], True)
        self.assertIs(checked['acceptance_granted'], False)
        self.assertEqual('test_definition_migration_only', checked['evidence_role'])
        self.assertEqual(6, len(checked['preserved_obligations']))
        self.assert_not_granted(authority.assess(self.root))

    def test_test_migration_rejects_missing_unknown_and_skip_claims(self):
        original = authority.expected_test_supersession()
        for key in original:
            mapping = copy.deepcopy(original)
            del mapping[key]
            with self.subTest(missing=key), self.assertRaises(ValueError):
                authority.validate_test_supersession(mapping, self.root)
        for delta in ({'skip': True}, {'status': 'passed'}, {'old_test_passed': True},
                      {'replacement_execution_required': False}, {'acceptance_granted': True},
                      {'preserved_obligations': []}, {'native_verified': True},
                      {'evidence_role': 'product_execution'}):
            with self.subTest(delta=delta), self.assertRaises(ValueError):
                authority.validate_test_supersession({**original, **delta}, self.root)

    def test_test_migration_cannot_choose_arbitrary_legacy_or_replacement_tests(self):
        original = authority.expected_test_supersession()
        for branch, field, value in (
            ('superseded_check', 'id', 'any-test'), ('superseded_check', 'case_count', 5.0),
            ('superseded_check', 'case_count', True), ('replacement_check', 'id', 'unregistered'),
            ('replacement_check', 'test', 'apps/desktop/tests/fake.mjs'),
            ('approval_record', 'sha256', '0' * 64),
        ):
            mapping = copy.deepcopy(original)
            mapping[branch][field] = value
            with self.subTest(branch=branch, field=field, value=value), self.assertRaises(ValueError):
                authority.validate_test_supersession(mapping, self.root)
        for value in ('path', 'sha256', 'bytes'):
            mapping = copy.deepcopy(original)
            mapping['superseded_check']['test'][value] = 'other' if value != 'bytes' else 1
            with self.subTest(legacy_field=value), self.assertRaises(ValueError):
                authority.validate_test_supersession(mapping, self.root)

    def test_test_migration_requires_original_test_bytes_and_actual_source_approval(self):
        original = authority.expected_test_supersession()
        path = self.root / authority.LEGACY_TEST['path']
        raw = path.read_bytes()
        path.write_bytes(raw + b'\n')
        with self.assertRaises(ValueError):
            authority.validate_test_supersession(original, self.root)
        path.unlink()
        with self.assertRaises(ValueError):
            authority.validate_test_supersession(original, self.root)
        path.write_bytes(raw)
        other = self.root / 'legacy-copy.mjs'
        other.write_bytes(raw)
        path.unlink()
        path.symlink_to(other)
        with self.assertRaises(ValueError):
            authority.validate_test_supersession(original, self.root)
        path.unlink()
        path.write_bytes(raw)
        (self.root / authority.RECORD).unlink()
        with self.assertRaises(ValueError):
            authority.validate_test_supersession(original, self.root)

    def test_test_migration_copy_and_json_type_safety(self):
        original = authority.expected_test_supersession()
        checked = authority.validate_test_supersession(original, self.root)
        checked['superseded_check']['test']['bytes'] = 1
        checked['preserved_obligations'].clear()
        self.assertEqual(original, authority.expected_test_supersession())
        for value in (None, [], True, {**original, 'preserved_obligations': tuple(original['preserved_obligations'])},
                      {**original, 'unknown': float('nan')}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                authority.validate_test_supersession(value, self.root)


if __name__ == '__main__':
    unittest.main()
