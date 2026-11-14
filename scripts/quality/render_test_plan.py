#!/usr/bin/env python3
"""Deterministic review tables from the test plan, never runtime results."""
import argparse
import csv
import io
import json
from pathlib import Path
from test_plan_contract import validate_plan

ROOT = Path(__file__).resolve().parents[2]
PLAN = 'docs/development/first-product-test-plan.json'


def encode(fields, rows):
    stream = io.StringIO(newline='')
    writer = csv.DictWriter(stream, fieldnames=fields, lineterminator='\n')
    writer.writeheader()
    writer.writerows(rows)
    return stream.getvalue()


def render(root):
    data = json.loads((root / PLAN).read_text())
    variants = {row['id']: row for row in data['variants']}
    matrix, executions, coverage = [], [], []
    for case in data['cases']:
        matrix.append({key: case[key] for key in ['id', 'title', 'layer', 'gate', 'owner', 'status']} | {'variants': ';'.join(case['variant_ids'])})
        for variant in case['variant_ids']:
            row = {key: case[key] for key in ['title', 'layer', 'gate', 'owner', 'method', 'status']}
            row.update(execution_id=case['id']+'@'+variant, case_id=case['id'], variant_id=variant,
                       evidence_scope=variants[variant]['evidence_scope'], environment_ids=';'.join(variants[variant]['environment_ids']),
                       source_case=case['source']['id'] if case['source'] else '', release_required='true')
            for field in ['feature_ids', 'ai_config_ids', 'module_ids', 'component_ids', 'preconditions', 'steps', 'expected', 'evidence_types']:
                row[field] = '\n'.join(case[field])
            executions.append(row)
    for kind, field in [('feature', 'feature_ids'), ('model-requirement', 'ai_config_ids'), ('module', 'module_ids'), ('component', 'component_ids')]:
        ids = sorted({value for case in data['cases'] for value in case[field]})
        for identifier in ids:
            matched = [case for case in data['cases'] if identifier in case[field]]
            coverage.append(dict(kind=kind, id=identifier, case_ids=';'.join(case['id'] for case in matched),
                                 execution_count=sum(len(case['variant_ids']) for case in matched), status='planned_not_executed'))
    for source in data['sources']:
        for identifier in source['case_ids']:
            matched = [case for case in data['cases'] if case['source'] and case['source']['path']==source['path'] and case['source']['id']==identifier]
            coverage.append(dict(kind='source-acceptance', id=identifier, case_ids=';'.join(case['id'] for case in matched),
                                 execution_count=sum(len(case['variant_ids']) for case in matched), status='planned_not_executed'))
    return {
        'docs/development/first-product-test-matrix.csv': encode(['id','title','layer','gate','owner','variants','status'], matrix),
        'docs/development/first-product-test-executions.csv': encode(['execution_id','case_id','title','layer','gate','owner','variant_id','evidence_scope','environment_ids','source_case','feature_ids','ai_config_ids','module_ids','component_ids','method','preconditions','steps','expected','evidence_types','release_required','status'], executions),
        'docs/development/first-product-test-coverage.csv': encode(['kind','id','case_ids','execution_count','status'], coverage),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--check', action='store_true', help='Reject missing or hand-edited exports instead of writing')
    args = parser.parse_args()
    errors = validate_plan(args.root)
    if errors:
        print('\n'.join(errors))
        return 1
    outputs = render(args.root)
    for relative, content in outputs.items():
        path = args.root / relative
        if args.check:
            if not path.is_file() or path.read_text()!=content:
                errors.append('Test-plan export drift: '+relative)
        else:
            path.write_text(content)
    print(json.dumps(dict(status='failed' if errors else 'passed', mode='check' if args.check else 'explicit_export', files=list(outputs), errors=errors, product_tests_executed=False), ensure_ascii=False))
    return int(bool(errors))


if __name__ == '__main__':
    raise SystemExit(main())
