"""The runner cannot replace a historical UI contract with unexecuted checks."""
import copy
from pathlib import Path
import shutil
import tempfile
import unittest

import current_general_settings_authority as authority
from verify import general_settings_test_version

ROOT = Path(__file__).resolve().parents[2]


class VerifyGeneralSettingsSupersessionTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='luna-general-test-version-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.mapping = authority.expected_test_supersession()
        for relative in authority.approval_inputs(ROOT) | {self.mapping['superseded_check']['test']['path']}:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        replacement = self.mapping['replacement_check']
        self.step = {'id': replacement['id'], 'status': 'passed', 'exit_code': 0,
                     'test_count': 13, 'command': ['node', '--test', replacement['test']]}
        self.metadata = {'requirements': [{'checks': [replacement['id']],
                                         'test_supersession': self.mapping}]}

    def test_exact_migration_requires_a_real_execution_and_grants_no_product_acceptance(self):
        result = general_settings_test_version(self.root, [self.step], self.metadata)
        self.assertEqual(result['historical_execution'], 'not_run_superseded_design')
        self.assertEqual(result['replacement_execution'], self.step)
        self.assertIs(result['product_acceptance_granted'], False)
        self.assertIs(result['migration']['acceptance_granted'], False)
        self.assertEqual(general_settings_test_version(self.root, [self.step])['migration'], self.mapping)

    def test_no_execution_duplicate_or_wrong_check_cannot_retire_old_definition(self):
        for steps in ([], [self.step, self.step], [{**self.step, 'id': 'unrelated-test'}]):
            with self.subTest(steps=steps), self.assertRaises(ValueError):
                general_settings_test_version(self.root, steps, self.metadata)

    def test_failure_zero_tests_wrong_command_or_loose_json_types_are_rejected(self):
        for patch in ({'status': 'failed'}, {'exit_code': 1}, {'exit_code': False},
                      {'test_count': 0}, {'test_count': True}, {'test_count': 13.0},
                      {'command': ['node', '--test', self.mapping['superseded_check']['test']['path']]}):
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                general_settings_test_version(self.root, [{**self.step, **patch}], self.metadata)

    def test_metadata_cannot_omit_widen_or_duplicate_the_fixed_migration(self):
        invalid = copy.deepcopy(self.metadata)
        del invalid['requirements'][0]['test_supersession']
        extra = copy.deepcopy(self.metadata)
        extra['requirements'][0]['test_supersession']['skip_tests'] = True
        duplicate = copy.deepcopy(self.metadata)
        duplicate['requirements'] *= 2
        for metadata in (invalid, extra, duplicate):
            with self.subTest(metadata=metadata), self.assertRaises(ValueError):
                general_settings_test_version(self.root, [self.step], metadata)

    def test_missing_approval_or_mutated_old_test_is_not_a_valid_migration(self):
        old = self.root / self.mapping['superseded_check']['test']['path']
        raw = old.read_bytes()
        old.write_bytes(raw + b'\n')
        with self.assertRaises(ValueError):
            general_settings_test_version(self.root, [self.step], self.metadata)
        old.write_bytes(raw)
        (self.root / self.mapping['approval_record']['path']).unlink()
        with self.assertRaises(ValueError):
            general_settings_test_version(self.root, [self.step], self.metadata)

    def test_other_failed_steps_and_actual_input_records_are_not_erased(self):
        steps = [{'id': 'earlier-failed-check', 'status': 'failed', 'errors': ['retained']}, self.step]
        before = copy.deepcopy(steps)
        result = general_settings_test_version(self.root, steps, self.metadata)
        result['replacement_execution']['status'] = 'changed-copy'
        self.assertEqual(steps, before)
        self.assertEqual(steps[0]['errors'], ['retained'])
        failed = [{**self.step, 'status': 'failed', 'exit_code': 1}]
        before_failed = copy.deepcopy(failed)
        with self.assertRaises(ValueError):
            general_settings_test_version(self.root, failed, self.metadata)
        self.assertEqual(failed, before_failed)


if __name__ == '__main__':
    unittest.main()
