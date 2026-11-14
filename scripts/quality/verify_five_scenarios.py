#!/usr/bin/env python3
"""Execute actual guard CLIs on five temporary positive/negative source fixtures."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from test_spec_contract import ContractTests
from test_technology_contract import TechnologyContractTests

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / '.build/quality/five-scenarios'


def command(name, argv):
    result = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True)
    (OUT / f'{name}.log').write_text(result.stdout + '\n' + result.stderr)
    return result.returncode


def pair(key, argv, mutate):
    positive = command(key + '-positive', argv)
    if positive != 0:
        raise ValueError(f'{key}: conforming fixture exited {positive}')
    mutate()
    negative = command(key + '-negative', argv)
    if negative != 1:
        raise ValueError(f'{key}: injected violation exited {negative}, expected rejection 1')
    return {'category': key, 'positive_exit_code': positive, 'violation_exit_code': negative, 'rejected': True}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    case = ContractTests(); case.setUp()
    try:
        case.product(); case.save()
        argv = [sys.executable, str(ROOT / 'scripts/quality/spec_contract.py'), '--root', str(case.root), '--change', case.change_id, '--json']
        def unrequested():
            delta = case.root / case.prefix / 'specs/engineering-enforcement/spec.md'
            delta.write_text(delta.read_text().replace('FP-03', 'FP-99'))
            case.document['requirements'][0]['id'] = 'FP-99'; case.save()
        results.append(pair('01-outside-prd', argv, unrequested))
    finally:
        case.tearDown(); case.doCleanups()

    for name, mutate_kind in [('02-design-mismatch', 'design'), ('05-unapproved-framework', 'framework')]:
        case = TechnologyContractTests(); case.setUp()
        try:
            argv = [sys.executable, str(ROOT / 'scripts/quality/technology_contract.py'), '--root', str(case.root), '--json']
            def mutate():
                if mutate_kind == 'design':
                    p = case.root / case.design; p.write_text(p.read_text() + '\n.surface{background:#ff00c8}\n')
                else:
                    case.package['dependencies']['vue'] = '^3.5.0'; case.save_package()
            results.append(pair(name, argv, mutate))
        finally:
            case.tearDown(); case.doCleanups()

    for name in ['03-module-boundary', '04-directory-layout']:
        with tempfile.TemporaryDirectory(prefix='luna-five-') as directory:
            root = Path(directory)
            source = root / 'apps/desktop/src'
            (source / 'app').mkdir(parents=True)
            (source / 'app/bootstrap.tsx').write_text('export const ready = true;\n')
            (root / 'apps/desktop/tsconfig.json').write_text(json.dumps({'compilerOptions': {'target': 'ES2022', 'module': 'ESNext', 'moduleResolution': 'Bundler', 'strict': True}, 'include': ['src']}))
            argv = ['node', str(ROOT / 'scripts/quality/frontend-boundaries.mjs'), '--root', str(root), '--json']
            def mutate():
                if name == '03-module-boundary':
                    feature = source / 'features/reading/private.ts'; feature.parent.mkdir(parents=True)
                    feature.write_text('export const privateState = 1;\n')
                    ui = source / 'ui/primitives/Button.ts'; ui.parent.mkdir(parents=True)
                    ui.write_text("import {privateState} from '../../features/reading/private';export const value = privateState;\n")
                else:
                    hidden = source / 'unclassified/Hidden.ts'; hidden.parent.mkdir()
                    hidden.write_text('export const hidden = true;\n')
            results.append(pair(name, argv, mutate))

    results.sort(key=lambda row: row['category'])
    result = {'schema_version': 1, 'scope': 'guard CLI fault injection on temporary source/manifests; not product acceptance',
              'categories': results, 'status': 'passed', 'visual_screenshot_proof': '../visual-adversarial/verification.json'}
    (OUT / 'verification.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(result, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, AssertionError, KeyError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
