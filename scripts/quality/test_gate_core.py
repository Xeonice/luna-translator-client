import json
import tempfile
import subprocess
import sys
import unittest
from pathlib import Path

from gate_core import allowed, changed_sources, check_baseline, check_debt, check_scope, digest, safe_path, source_files
from product_specs import check as check_product_specs, render_specs
from verify import product_execution_gaps, validate_test_output


class GatesTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / 'openspec/changes/demo').mkdir(parents=True)
        (self.root / 'src').mkdir()
        self.source = self.root / 'src/main.ts'
        self.source.write_text('old_source()')

    def tearDown(self):
        self.tmp.cleanup()

    def write_json(self, relative, data):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data))

    def test_scope_rejects_unlisted_source_and_accepts_exact_declared_path(self):
        snapshot = 'openspec/changes/demo/source-snapshot.json'
        self.write_json(snapshot, {'files': {'src/main.ts': digest(self.source)}})
        self.source.write_text('new_source()')
        metadata = {'source_snapshot': snapshot, 'allowed_paths': ['openspec/changes/demo/**']}
        _, errors = check_scope(self.root, metadata)
        self.assertTrue(any('src/main.ts' in error for error in errors))
        metadata['allowed_paths'].append('src/main.ts')
        self.assertEqual(check_scope(self.root, metadata)[1], [])

    def test_new_unrequested_feature_file_is_not_hidden_by_an_existing_task(self):
        snapshot = 'openspec/changes/demo/source-snapshot.json'
        self.write_json(snapshot, {'files': {'src/main.ts': digest(self.source)}})
        self.write_json('src/cloud-sync/new-feature.json', {'capability': 'unrequested cloud synchronization'})
        _, errors = check_scope(self.root, {'source_snapshot': snapshot, 'allowed_paths': ['src/main.ts', 'openspec/changes/demo/**']})
        self.assertTrue(any('cloud-sync/new-feature.json' in error for error in errors))

    def test_product_ui_requires_actual_capture_not_guard_self_tests(self):
        metadata = {'kind': 'product', 'product_visual_checks': ['visual-guard-tests']}
        for relative in ['apps/desktop/src/features/library/View.tsx', 'apps/desktop/src/ui/tokens.ts', 'apps/desktop/src/ui/theme.json', 'apps/desktop/src/assets/surface.png', 'apps/desktop/src-tauri/src/platform/macos/material.rs']:
            with self.subTest(relative=relative):
                self.assertTrue(product_execution_gaps(metadata, [relative], 'development'))
        self.assertEqual(product_execution_gaps({'kind': 'tooling'}, ['scripts/quality/tool.py'], 'development'), [])

    def test_release_requires_real_execution_in_addition_to_static_evidence(self):
        self.assertTrue(product_execution_gaps({'kind': 'product'}, [], 'release'))

    def test_zero_or_skipped_python_checks_do_not_count_as_execution(self):
        code = "import unittest\n@unittest.skip('must not fake completion')\nclass Required(unittest.TestCase):\n def test_assertion(self): self.fail()\nunittest.main()\n"
        result = subprocess.run([sys.executable, '-c', code], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertTrue(validate_test_output('python', result.stdout + result.stderr)[1])
        result = subprocess.run([sys.executable, '-c', 'import unittest; unittest.main()'], capture_output=True, text=True)
        # Python versions differ on zero-test exit codes; our own receipt parser must reject both.
        self.assertTrue(validate_test_output('python', result.stdout + result.stderr)[1])

    def test_skipped_node_check_does_not_count_as_execution(self):
        target = self.root / 'skipped.test.mjs'
        target.write_text("import test from 'node:test';test.skip('required assertion',()=>{throw new Error('never executed')});")
        result = subprocess.run(['node', '--test', str(target)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertTrue(validate_test_output('node', result.stdout + result.stderr)[1])

    def test_ci_rejects_gitless_snapshot(self):
        with self.assertRaisesRegex(ValueError, 'CI may not'):
            check_scope(self.root, {'source_snapshot': 'unused', 'allowed_paths': []}, ci=True)

    def test_path_prefix_is_not_a_sibling_or_arbitrary_glob(self):
        self.assertTrue(allowed('src/ui/Button.tsx', ['src/ui/**']))
        self.assertFalse(allowed('src/ui-escape/Button.tsx', ['src/ui/**']))
        self.assertFalse(allowed('src/UI.tsx', ['**']))

    def test_baseline_rejects_missing_and_modified_authority(self):
        self.write_json('docs/development/iteration-baseline.json', {'files': [{'path': 'src/main.ts', 'sha256': digest(self.source)}]})
        self.assertEqual(check_baseline(self.root), [])
        self.source.write_text('drift')
        self.assertTrue(check_baseline(self.root))
        self.source.unlink()
        self.assertTrue(check_baseline(self.root))

    def test_exact_legacy_debt_cannot_expand_or_survive_source_edit(self):
        finding = {'path': 'src/main.ts', 'rule': 'FE_DIRECT_IO', 'evidence': 'fetch()'}
        self.write_json('openspec/legacy-debt.json', {'findings': [{**finding, 'source_sha256': digest(self.source)}]})
        self.trust_debt()
        self.assertEqual(check_debt(self.root, [finding])[1], [])
        self.assertTrue(check_debt(self.root, [{**finding, 'evidence': 'fetch(other)'}])[1])
        self.source.write_text('still_invalid_but_edited')
        self.assertTrue(check_debt(self.root, [finding])[1])

    def test_release_does_not_accept_frozen_legacy_debt(self):
        finding = {'path': 'src/main.ts', 'rule': 'FE_DIRECT_IO', 'evidence': 'fetch()'}
        self.write_json('openspec/legacy-debt.json', {'findings': [{**finding, 'source_sha256': digest(self.source)}]})
        self.trust_debt()
        self.assertTrue(check_debt(self.root, [finding], release=True)[1])

    def trust_debt(self):
        self.write_json('docs/development/iteration-baseline.json', {'files': [{'path': 'openspec/legacy-debt.json', 'sha256': digest(self.root / 'openspec/legacy-debt.json')}]})

    def test_self_expanding_debt_or_refreshing_hash_is_rejected(self):
        finding = {'path': 'src/main.ts', 'rule': 'FE_DIRECT_IO', 'evidence': 'fetch()'}
        self.write_json('openspec/legacy-debt.json', {'findings': []})
        self.trust_debt()
        self.write_json('openspec/legacy-debt.json', {'findings': [{**finding, 'source_sha256': digest(self.source)}]})
        self.assertTrue(check_debt(self.root, [finding])[1])

    def test_nested_runtime_diagnostics_reports_and_symlinks_are_not_silent(self):
        for relative in ['src/runtime/new.ts', 'src/diagnostics/new.ts', 'src/reports/new.ts']:
            target = self.root / relative; target.parent.mkdir(parents=True); target.write_text('new_feature()')
            self.assertIn(relative, source_files(self.root))
        (self.root / 'src/link.ts').symlink_to(self.source)
        self.assertTrue(source_files(self.root)['src/link.ts'].startswith('symlink:'))
        snapshot = 'openspec/changes/demo/source-snapshot.json'
        self.write_json(snapshot, {'files': {}})
        self.assertTrue(any('symlink' in error for error in check_scope(self.root, {'source_snapshot': snapshot, 'allowed_paths': ['src/**', 'openspec/changes/demo/**']})[1]))

    def test_git_base_option_injection_fails_and_real_base_detects_changes(self):
        def git(*args):
            return subprocess.run(['git', *args], cwd=self.root, capture_output=True, text=True, check=True)
        git('init', '-q')
        git('add', 'src/main.ts')
        git('-c', 'user.name=Gate Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'synthetic test fixture')
        self.source.write_text('changed()')
        with self.assertRaisesRegex(ValueError, 'actual commit'):
            changed_sources(self.root, 'unused', base='--output=hidden.diff', ci=True)
        self.assertFalse((self.root / 'hidden.diff').exists())
        self.assertIn('src/main.ts', changed_sources(self.root, 'unused', base='HEAD', ci=True))

    def test_evidence_and_authority_paths_cannot_escape_or_use_symlinks(self):
        for relative in ['../secret', '/tmp/secret', 'src/../../secret']:
            with self.assertRaises(ValueError):
                safe_path(self.root, relative)
        (self.root / 'link').symlink_to(self.root / 'src', target_is_directory=True)
        with self.assertRaises(ValueError):
            safe_path(self.root, 'link/main.ts')


if __name__ == '__main__':
    unittest.main()
