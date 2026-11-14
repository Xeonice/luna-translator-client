"""Regression tests for real command receipts and root dispatch failure paths."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import development_gates as runner
from gate_policy import gate_result, map_identity, snapshot_inputs, validate_receipt


class RunnerTests(unittest.TestCase):
    def test_registry_has_all_actual_required_handlers(self):
        registry = runner.command_registry(runner.ROOT, 'some-change', 'run-01')
        for ids in runner.MANDATORY.values():
            self.assertTrue(set(ids) <= registry.keys())
        self.assertTrue(all(isinstance(argv, list) and argv for argv in registry.values()))

    def test_base_is_separate_argv_and_reaches_scope_and_boundaries(self):
        registry = runner.command_registry(runner.ROOT, 'some-change', 'run-01', base='--not-an-option', release=True)
        for name in ('source-scope', 'frontend-boundaries', 'rust-boundaries'):
            argv = registry[name]
            self.assertEqual(argv[argv.index('--base') + 1], '--not-an-option')
            self.assertIn('--release', argv)

    def test_run_identity_cannot_escape_output(self):
        for change, run in [('valid', '../x'), ('../bad', 'run-01')]:
            with self.assertRaises(ValueError): runner.run_directory(change, run)

    def test_native_impact_reaches_the_actual_visual_state_registry(self):
        import current_design_alignment
        with patch.object(current_design_alignment, 'required_states', return_value=(['native/current'], [])) as registry:
            paths = ['crates/app-core/src/lib.rs']
            self.assertEqual(runner.required_visual_scope(runner.ROOT, paths), ['native/current'])
            registry.assert_called_once_with(paths, root=runner.ROOT)

    def test_ci_profile_requires_compilation_and_full_guards_for_document_only_diff(self):
        paths = ['README.md']
        impact = runner.classify_impact(paths, stage='development', full_engineering=True)
        self.assertEqual(impact['required_gates'], ['G01', 'G02', 'G03', 'G04', 'G05'])
        checks = runner.mandatory_checks(paths, full_engineering=True)
        self.assertIn('gate-effectiveness-execution', checks['G02'])
        self.assertIn('github-ci-tests', checks['G02'])
        self.assertIn('rust-test-compile', checks['G05'])
        product = runner.classify_impact(['apps/desktop/src/ui/view.tsx'],
                                        stage='delivery', full_engineering=True)
        self.assertTrue({'G06', 'G08', 'G09'}.issubset(product['required_gates']))

    def test_real_receipt_atomic_write_and_raw_test_count(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / 'source.txt').write_text('stable input')
            inputs = snapshot_inputs(root, ['source.txt'])
            argv = [sys.executable, '-c', 'import unittest; s=unittest.TestSuite([unittest.FunctionTestCase(lambda:None)]); unittest.TextTestRunner().run(s)']
            with patch.object(runner, 'command_registry', return_value={'unit-tests': argv}):
                receipt = runner.execute(root, 'safe-change', 'run-01', 'unit-tests', 'G05', inputs)
            self.assertEqual(receipt['exit_code'], 0)
            self.assertEqual(receipt['test_count'], 1)
            self.assertTrue((root / runner.run_directory('safe-change', 'run-01') / 'unit-tests.receipt.json').is_file())
            self.assertEqual(validate_receipt(root, receipt, expected_command=argv)['status'], 'passed')
            altered = copy.deepcopy(receipt); altered['test_count'] = 40
            self.assertEqual(validate_receipt(root, altered, expected_command=argv)['status'], 'failed')

    def test_test_counts_do_not_invent_assertions_for_empty_success(self):
        self.assertEqual(runner.test_counts('exit zero without a test runner'), (0, 0))

    def test_production_component_summary_count_is_supported(self):
        output = json.dumps({'status': 'passed', 'checks': 15, 'failed': [], 'errors': []})
        self.assertEqual(runner.test_counts(output), (15, 0))

    def test_ci_missing_base_is_rejected_not_snapshot_fallback(self):
        context = {'change_id': 'safe-change', 'source_snapshot': 'missing.json', 'allowed_paths': ['scripts/quality/**']}
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict('os.environ', {'CI': 'true'}):
                with self.assertRaises(ValueError): runner.assess_current(Path(directory), context)

    def test_g08_missing_actual_inputs_returns_blocked_not_unknown(self):
        from gate_checks import check
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); folder = root / 'openspec/changes/safe-change'
            folder.mkdir(parents=True); (folder / 'verification.json').write_text('{}')
            for name in ('artifact-package-check', 'real-game-regression', 'real-translation-service'):
                actual = check(root, name, 'safe-change')
                self.assertEqual(actual['status'], 'blocked')
                self.assertIn('release.real_artifact_inputs_missing', actual['errors'])

    def test_g08_real_command_cannot_replace_its_external_input_during_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); folder = root / '.build/package'
            folder.mkdir(parents=True)
            (folder / 'candidate.zip').write_bytes(b'original')
            (folder / 'build-manifest.json').write_text('{}')
            context = {'release_inputs': {'artifact': '.build/package/candidate.zip'}}
            argv = [sys.executable, '-c', "from pathlib import Path; Path('.build/package/candidate.zip').write_bytes(b'replaced')"]
            with patch.object(runner, 'load_context', return_value=context), patch.object(
                    runner, 'command_registry', return_value={'artifact-package-check': argv}):
                receipt = runner.execute(root, 'safe-change', 'run-01', 'artifact-package-check', 'G08', {})
            self.assertNotEqual(receipt['exit_code'], 0)
            self.assertIn('external_inputs_changed_during_execution',
                          (root / receipt['stderr']['path']).read_text())

    def test_g08_passive_assessment_rechecks_current_external_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); folder = root / '.build/package'
            folder.mkdir(parents=True)
            artifact = folder / 'candidate.zip'; artifact.write_bytes(b'original')
            (folder / 'build-manifest.json').write_text('{}')
            context = {'change_id': 'safe-change', 'allowed_paths': ['scripts/**'],
                       'source_snapshot': {}, 'release_inputs': {'artifact': '.build/package/candidate.zip'}}
            old = {'receipts': {'G08': [{'check_id': 'artifact-package-check',
                    'external_inputs': runner.release_input_identity(root, context)}]}}
            def assessment(*args, **kwargs):
                return {'gates': [gate_result(f'G{i:02d}', 'passed', 'isolated_other_checks') for i in range(1, 10)],
                        'impact': {'required_gates': ['G08']}, 'status': 'passed',
                        'next_stage_allowed': True, 'source_inputs_sha256': map_identity(snapshot_inputs(root))}
            with patch.object(runner, 'current_record', return_value=old), patch.object(
                    runner, 'check_scope', return_value=([], [])), patch.object(
                    runner, 'required_visual_scope', return_value=[]), patch.object(runner, 'evaluate', side_effect=assessment):
                self.assertEqual(runner.assess_current(root, context)['status'], 'passed')
                artifact.write_bytes(b'replaced at the same path')
                result = runner.assess_current(root, context)
                self.assertEqual(result['status'], 'failed')
                self.assertEqual(next(g for g in result['gates'] if g['gate_id'] == 'G08')['reason'],
                                 'release.external_inputs_changed')
                artifact.write_bytes(b'original')
                self.assertEqual(runner.assess_current(root, context)['status'], 'passed')

    def test_g08_external_inputs_are_full_current_bytes_not_receipt_subset(self):
        from gate_checks import release_input_identity, validate_release_input_identity
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            def write(name, content):
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content); return path
            artifact = write('dist/Luna.zip', 'original package bytes')
            outer = write('dist/build-manifest.json', '{"kind":"actual-package-manifest"}')
            raw = write('.build/run/raw.log', 'original native observations')
            write('.build/run/nested.json', json.dumps({'stdout': '.build/run/raw.log'}))
            evidence = write('.build/run/evidence.json', json.dumps({'evidence': [
                {'path': '.build/run/nested.json', 'sha256': 'irrelevant-self-declared-hash'}]}))
            metadata = {'release_inputs': {'artifact': 'dist/Luna.zip',
                        'evidence': '.build/run/evidence.json',
                        'native_evidence': '.build/run/nested.json',
                        'service_evidence': '.build/run/nested.json'}}
            captured = release_input_identity(root, metadata)
            self.assertEqual(set(captured), {'dist/Luna.zip', 'dist/build-manifest.json',
                             '.build/run/raw.log', '.build/run/nested.json', '.build/run/evidence.json'})
            self.assertEqual(validate_release_input_identity(root, metadata, captured), [])
            for path in (artifact, outer, evidence, raw):
                original = path.read_text(); path.write_text(original + '\nchanged')
                self.assertTrue(validate_release_input_identity(root, metadata, captured), path.name)
                path.write_text(original)
                self.assertEqual(validate_release_input_identity(root, metadata, captured), [])
            subset = {key: value for key, value in captured.items() if key != '.build/run/raw.log'}
            self.assertEqual(validate_release_input_identity(root, metadata, subset), ['release.external_inputs_changed'])
            raw.unlink()
            self.assertTrue(validate_release_input_identity(root, metadata, captured))

    def test_g08_directory_inventory_and_symlink_are_not_silently_ignored(self):
        from gate_checks import release_input_identity, validate_release_input_identity
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); (root / 'dist').mkdir(); (root / '.build/run').mkdir(parents=True)
            (root / 'dist/Luna.zip').write_text('package')
            (root / 'dist/build-manifest.json').write_text('{}')
            metadata = {'release_inputs': {'artifact': 'dist/Luna.zip', 'evidence': '.build/run'}}
            captured = release_input_identity(root, metadata)
            raw = root / '.build/run/new.log'; raw.write_text('new raw')
            self.assertTrue(validate_release_input_identity(root, metadata, captured))
            raw.unlink(); self.assertEqual(validate_release_input_identity(root, metadata, captured), [])
            raw.symlink_to(root / 'dist/Luna.zip')
            self.assertTrue(validate_release_input_identity(root, metadata, captured))

    def test_existing_git_hook_installation_in_isolated_repository(self):
        from install_git_hooks import install
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            self.assertEqual(install(root)['status'], 'blocked')
            subprocess.run(['git', 'init', '-q', str(root)], check=True, capture_output=True)
            (root / '.githooks').mkdir()
            for name in ('pre-commit', 'pre-push'):
                (root / '.githooks' / name).write_bytes((runner.ROOT / '.githooks' / name).read_bytes())
            (root / 'scripts/quality').mkdir(parents=True)
            (root / 'scripts/quality/development_gates.py').symlink_to(runner.ROOT / 'scripts/quality/development_gates.py')
            self.assertTrue(install(root)['installed'])
            config = subprocess.check_output(['git', 'config', '--local', 'core.hooksPath'], cwd=root, text=True).strip()
            self.assertEqual(config, '.githooks')
            # The real project hook executes the real runner. An undeclared
            # work item must be rejected, not merely an arbitrary exit-1 shim.
            hook = subprocess.run(['git', 'hook', 'run', 'pre-commit'], cwd=root, capture_output=True)
            self.assertNotEqual(hook.returncode, 0)
            # git hook run routes the child streams to its own stderr.
            verdict = json.loads(hook.stdout or hook.stderr)
            self.assertEqual(verdict['status'], 'failed')
            self.assertIn('gate-context.json', verdict['error'])


if __name__ == '__main__':
    unittest.main()
