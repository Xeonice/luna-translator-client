#!/usr/bin/env python3
"""Export prerequisite, source ownership and execution mappings, never results."""
import argparse
import csv
import io
import json
from pathlib import Path
from test_operations_contract import validate_operations, dependencies_for

ROOT = Path(__file__).resolve().parents[2]


def encode(rows):
    stream=io.StringIO(newline='')
    writer=csv.DictWriter(stream,fieldnames=list(rows[0]),lineterminator='\n')
    writer.writeheader();writer.writerows(rows)
    return stream.getvalue()


def render(root):
    plan=json.loads((root/'docs/development/first-product-test-plan.json').read_text())
    ops=json.loads((root/'docs/development/first-product-test-operations.json').read_text())
    case_areas={r['case_id']:r for r in ops['case_maintenance']}
    execution_rows=[];affected={r['id']:[] for r in ops['prerequisites']}
    for case in plan['cases']:
        for variant in case['variant_ids']:
            execution=case['id']+'@'+variant
            prerequisites=[r['id'] for r in dependencies_for(case,variant,ops)]
            for id in prerequisites:affected[id].append(execution)
            execution_rows.append(dict(execution_id=execution,case_id=case['id'],variant_id=variant,prerequisite_ids=';'.join(prerequisites),maintenance_area_ids=';'.join(case_areas[case['id']]['area_ids']),module_owner=case['owner'],product_test_status=case['status']))
    prerequisites=[]
    for row in ops['prerequisites']:
        prerequisites.append(dict(id=row['id'],title=row['title'],provider=row['provider'],owners=';'.join(row['owners']),status=row['status'],required_by_gates=';'.join(row['required_by_gates']),deliverable=row['deliverable'],user_action=row['user_action'],validation=row['validation'],current_evidence=row['current_evidence'],affected_execution_count=len(affected[row['id']]),affected_execution_ids=';'.join(affected[row['id']])))
    areas=[]
    for row in ops['maintenance_areas']:
        areas.append({key:(';'.join(value) if isinstance(value,list) else value) for key,value in row.items()})
    return {'docs/development/first-product-test-prerequisites.csv':encode(prerequisites),'docs/development/first-product-test-maintenance.csv':encode(areas),'docs/development/first-product-test-execution-dependencies.csv':encode(execution_rows)}


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--root',type=Path,default=ROOT);parser.add_argument('--check',action='store_true');args=parser.parse_args()
    errors=validate_operations(args.root)
    if not errors:
        outputs=render(args.root)
        for relative,content in outputs.items():
            target=args.root/relative
            if args.check:
                if not target.is_file() or target.read_text()!=content:errors.append('Test operations export drift: '+relative)
            else:target.write_text(content)
    print(json.dumps(dict(status='failed' if errors else 'passed',mode='check' if args.check else 'explicit_export',errors=errors,product_tests_executed=False),ensure_ascii=False))
    return int(bool(errors))


if __name__=='__main__':raise SystemExit(main())
