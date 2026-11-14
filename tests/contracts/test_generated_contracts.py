import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('generated_contract_verifier', ROOT / 'scripts/contracts/verify_contracts.py')
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class GeneratedContractTests(unittest.TestCase):
    def test_real_token_source_matches_deterministic_css(self):
        self.assertEqual(module.verify(ROOT, tokens_only=True)['status'], 'passed')

    def test_drift_is_rejected_without_rewriting_source_or_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            base = root / 'apps/desktop/src/ui/tokens'
            base.mkdir(parents=True)
            design = root / 'docs/design/first-product-v1'
            design.mkdir(parents=True)
            for filename in ['material-tokens.css', 'styles.css']:
                (design / filename).write_bytes((ROOT / 'docs/design/first-product-v1' / filename).read_bytes())
            tokens = {'dark': {'--color': '#111'}, 'light': {'--color': '#eee'}}
            source = json.dumps(tokens)
            (base / 'tokens.json').write_text(source)
            (base / 'generated.css').write_text('stale CSS')
            self.assertEqual(module.verify(root, tokens_only=True)['status'], 'failed')
            self.assertEqual((base / 'tokens.json').read_text(), source)
            self.assertEqual((base / 'generated.css').read_text(), 'stale CSS')

    def test_tokens_reject_injected_css_and_incomplete_themes(self):
        for tokens in [{}, {'dark': {}, 'light': {}}, {'dark': {'--x': 'red; color:blue'}, 'light': {'--x': 'white'}}]:
            with self.assertRaises(ValueError):
                module.render_tokens(tokens)

    def test_synchronized_token_and_css_tampering_still_fails_design_parity(self):
        tokens = json.loads((ROOT / 'apps/desktop/src/ui/tokens/tokens.json').read_text())
        tokens['dark']['--material-accent'] = '#ff00ff'
        self.assertTrue(module.design_token_errors(ROOT, tokens))

    def test_order_and_exact_css_values_are_preserved(self):
        result = module.render_tokens({'dark': {'--a': 'rgba(1, 2, 3, .4)', '--b': '7px'}, 'light': {'--a': '#eee'}})
        self.assertIn('  --a: rgba(1, 2, 3, .4);\n  --b: 7px;', result)
        self.assertIn('html[data-theme="light"] {\n', result)


if __name__ == '__main__':
    unittest.main()
