import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from render_test_plan import ROOT, PLAN, render


class TestPlanExportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for relative in [PLAN, 'docs/development/client-regression.csv',
                         'docs/product/model-configuration-acceptance.csv',
                         'docs/product/game-library-window-acceptance.csv',
                         'docs/architecture/engineering-modules.csv',
                         'docs/architecture/engineering-components.csv']:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        for relative, content in render(self.root).items():
            (self.root / relative).write_text(content)

    def run_check(self):
        return subprocess.run([sys.executable, str(ROOT / 'scripts/quality/render_test_plan.py'), '--root', str(self.root), '--check'], text=True, capture_output=True)

    def test_actual_cli_accepts_unchanged_exports_without_execution(self):
        before = hashlib.sha256((self.root / PLAN).read_bytes()).hexdigest()
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
        self.assertIs(json.loads(result.stdout)['product_tests_executed'], False)
        self.assertEqual(hashlib.sha256((self.root / PLAN).read_bytes()).hexdigest(), before)

    def test_cli_rejects_hand_edited_expected_result_without_rewriting(self):
        path = self.root / 'docs/development/first-product-test-executions.csv'
        path.write_text(path.read_text().replace('not_run', 'passed', 1))
        changed = path.read_bytes()
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn('export drift', result.stdout)
        self.assertEqual(path.read_bytes(), changed)

    def test_cli_rejects_missing_review_table(self):
        (self.root / 'docs/development/first-product-test-coverage.csv').unlink()
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn('coverage.csv', result.stdout)

    def test_cli_cannot_export_a_plan_with_unmapped_environment(self):
        path = self.root / PLAN
        data = json.loads(path.read_text())
        data['cases'][0]['variant_ids'] = ['UNREGISTERED-ENVIRONMENT']
        path.write_text(json.dumps(data))
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn('unknown reference', result.stdout)


if __name__ == '__main__':
    unittest.main()
