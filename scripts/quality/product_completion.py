"""Derive product completion from existing authorities, never a green subtotal.

Contract/guard/source-review success is distinct from product readiness. This
module does not run a game, approve a reference, or invent a runtime adapter.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

from gate_core import safe_path, source_files

SELF_CHECKS = frozenset({'product-completion-assessment', 'product-acceptance-completion',
                        'product-completion-finalization'})
DERIVED_GATE_STEPS = frozenset({'product-acceptance-completion', 'product-completion-finalization'})
STATUSES = frozenset({'passed', 'failed', 'blocked', 'not_run'})
SHA = re.compile(r'[0-9a-f]{64}\Z')


def source_identity(root):
    return hashlib.sha256(json.dumps(source_files(Path(root)), sort_keys=True).encode()).hexdigest()


def declared_checks(metadata):
    checks = {name for requirement in metadata['requirements'] for name in requirement['checks']}
    checks.update(metadata.get('product_visual_checks', []))
    checks.update(metadata.get('product_runtime_checks', []))
    return checks - SELF_CHECKS


def requires_product_completion(metadata, profile, explicit_flag):
    """One condition for both initial gating and final epoch/late-failure gating."""
    metadata = metadata or {}
    return (explicit_flag is True or profile == 'release' or metadata.get('kind') == 'product'
            or any('product-acceptance-completion' in row.get('checks', [])
                   for row in metadata.get('requirements', [])))


def derive_verdict(*, source_tree_sha256, execution_source_sha256, required_checks,
                   steps, design_assessment, product_assessment, capture_gaps):
    """Pure aggregation of already inspected facts; public CLI always rebuilds them."""
    reasons = []
    if (not isinstance(source_tree_sha256, str) or not SHA.fullmatch(source_tree_sha256)
            or execution_source_sha256 is not None and
            (not isinstance(execution_source_sha256, str) or not SHA.fullmatch(execution_source_sha256))):
        raise ValueError('typed source identity required')
    if not execution_source_sha256:
        reasons.append({'code': 'engineering_execution_missing'})
    elif execution_source_sha256 != source_tree_sha256:
        reasons.append({'code': 'execution_source_mismatch'})
    if not isinstance(steps, list):
        raise ValueError('actual engineering step list required')
    by_id = {}
    for step in steps:
        if (not isinstance(step, dict) or not isinstance(step.get('id'), str)
                or not step['id'] or step['id'] in by_id
                or type(step.get('status')) is not str or step['status'] not in STATUSES):
            raise ValueError('unique typed actual engineering steps required')
        by_id[step['id']] = step
    if any(type(name) is not str or not name for name in required_checks):
        raise ValueError('typed required check identities needed')
    required_checks = set(required_checks) - SELF_CHECKS
    if not required_checks:
        reasons.append({'code': 'required_engineering_scope_missing'})
    for name in sorted(required_checks):
        step = by_id.get(name)
        if step is None:
            reasons.append({'code': 'required_execution_not_run', 'check_id': name})
        elif step['status'] != 'passed':
            reasons.append({'code': 'required_execution_not_passed', 'check_id': name,
                            'actual_status': step['status']})
    # An undeclared failed unconditional check cannot disappear in a subtotal.
    for name, step in sorted(by_id.items()):
        if name not in required_checks | DERIVED_GATE_STEPS and step['status'] != 'passed':
            reasons.append({'code': 'engineering_failure', 'check_id': name,
                            'actual_status': step['status']})
    for label, assessment in (('design', design_assessment), ('product', product_assessment)):
        if (not isinstance(assessment, dict) or assessment.get('status') not in {'passed', 'blocked', 'failed'}
                or not isinstance(assessment.get('errors'), list)
                or not isinstance(assessment.get('release_gaps'), list)):
            raise ValueError('actual current readiness assessment required: ' + label)
        if assessment['status'] != 'passed' or assessment['errors'] or assessment['release_gaps']:
            reasons.append({'code': label + '_readiness_incomplete',
                            'actual_status': assessment['status'],
                            'error_count': len(assessment['errors']),
                            'gap_count': len(assessment['release_gaps'])})
    if not isinstance(capture_gaps, list):
        raise ValueError('code-owned capture scope gaps required')
    if capture_gaps:
        reasons.append({'code': 'whole_client_or_layer_coverage_missing',
                        'gap_count': len(capture_gaps)})
    complete = not reasons
    # Both completion gates are projections of the same underlying facts.
    # Ignore their expected failure when already incomplete, but reject either
    # gate contradicting the base verdict. Check against one immutable expected
    # status so a contradictory first gate cannot change the second's oracle.
    expected_gate_status = 'passed' if complete else 'failed'
    for name in sorted(DERIVED_GATE_STEPS):
        step = by_id.get(name)
        if step and step['status'] != expected_gate_status:
            reasons.append({'code': ('final_completion_step_contradicts_derived_facts'
                                     if name == 'product-completion-finalization'
                                     else 'acceptance_completion_step_contradicts_derived_facts'),
                            'actual_status': step['status']})
            complete = False
    return {'schema_version': 1, 'kind': 'product-completion-verdict/v1',
            'source_tree_sha256': source_tree_sha256,
            'execution_source_sha256': execution_source_sha256,
            'product_completion': 'complete' if complete else 'incomplete',
            'product_gate_status': 'passed' if complete else 'incomplete',
            'next_stage_allowed': complete,
            'required_engineering_checks': sorted(required_checks),
            # Self steps are a projection of this verdict. Counting or using
            # their expected failure as an input creates an unstable hash/claim
            # cycle when the final step is appended to the stored receipt.
            'actual_engineering_steps': sum(name not in SELF_CHECKS for name in by_id),
            'completion_self_check_ids': sorted(SELF_CHECKS),
            'reasons': reasons, 'capture_scope_gaps': capture_gaps,
            'design_assessment': design_assessment,
            'product_assessment': product_assessment,
            'guard_tests_are_product_acceptance': False,
            'source_review_is_product_acceptance': False,
            'browser_is_native_acceptance': False}


def capture_scope_gaps(root):
    """Use the existing state/adapter registry; do not create another case list."""
    from all_page_design_coverage import expected_contract
    from current_design_alignment import ADAPTERS
    gaps = []
    for page in expected_contract(root)['pages']:
        if not page['surface_id'].startswith(('main:', 'settings:', 'window:')):
            continue
        for state in page['states']:
            if not state['applicable']:
                continue
            for layer in state['required_layers']:
                adapters = [adapter for adapter in ADAPTERS.values()
                            if adapter.layer == layer and state['id'] in adapter.states]
                if not any(getattr(adapter, 'capture_scope', 'target') == 'whole_client'
                           for adapter in adapters):
                    gaps.append({'state_id': state['id'], 'surface_id': page['surface_id'],
                                 'layer': layer, 'required_scope': 'whole_client',
                                 'available_scopes': sorted({getattr(a, 'capture_scope', 'target') for a in adapters}),
                                 'reason': 'target crop or desktop viewport cannot prove whole client'})
    return gaps


def assess_completion(root, metadata, steps, *, execution_source_sha256=None,
                      evidence=None, artifact=None):
    """Recompute global current product readiness, not merely this change's subset.

    Existing release inspectors own source/package/process cohort validation.
    Missing Native adapters/evidence remain gaps; caller booleans cannot replace
    those inspectors or grant acceptance to the current user's different app.
    """
    from current_design_alignment import assessment_report as design_readiness
    from current_product_coverage import assessment_report as product_readiness
    root = Path(root)
    design = design_readiness(root, evidence, artifact)
    product = product_readiness(root, evidence, artifact)
    from binding_journey import global_assessment
    binding = global_assessment(root, steps)
    # Add, never replace, the old inventories and their Native requirements.
    product = merge_binding_readiness(product, binding)
    return derive_verdict(source_tree_sha256=source_identity(root),
                          execution_source_sha256=execution_source_sha256,
                          required_checks=declared_checks(metadata), steps=steps,
                          design_assessment=design, product_assessment=product,
                          capture_gaps=capture_scope_gaps(root))


def binding_execution_gaps(assessment):
    """A green aggregate cannot erase the fixed independently inspected groups.

    This validates the aggregation boundary only. Real source/artifact/test
    semantics still come from binding_journey.global_assessment and its raw
    inspectors; a guard fixture is never execution evidence.
    """
    from binding_journey import EXACT_EXECUTION_OBLIGATION_IDS
    missing = {'case_id': 'binding-execution-obligations',
               'layer': 'registered_execution_obligation', 'status': 'blocked',
               'reason': 'Complete typed independently inspected binding execution groups are required'}
    if (not isinstance(assessment, dict)
            or type(assessment.get('schema_version')) is not int
            or assessment['schema_version'] != 1
            or assessment.get('kind') != 'binding-execution-obligation-assessment/v1'
            or not isinstance(assessment.get('registry_sha256'), str)
            or not re.fullmatch('[0-9a-f]{64}', assessment['registry_sha256'])
            or assessment.get('product_acceptance_promoted') is not False):
        return [missing]
    groups = assessment.get('groups')
    if (not isinstance(groups, list)
            or [row.get('id') if isinstance(row, dict) else None for row in groups]
            != list(EXACT_EXECUTION_OBLIGATION_IDS)):
        return [missing]
    gaps = []
    for row in groups:
        if (set(row) != {'id', 'status', 'checks', 'errors', 'artifacts'}
                or row['status'] not in ('passed', 'failed', 'blocked', 'not_run')
                or any(not isinstance(row[key], list) for key in ('checks', 'errors', 'artifacts'))):
            return [missing]
        if (row['status'] != 'passed' or row['errors']
                or not row['checks'] or not row['artifacts']):
            gaps.append({'case_id': row['id'], 'layer': 'registered_execution_obligation',
                         'status': row['status'] if row['status'] != 'passed' else 'blocked',
                         'reason': 'Fixed binding execution group is missing, empty or not passed'})
    return gaps


def merge_binding_readiness(product, binding):
    result = dict(product)
    result['errors'] = list(product['errors']) + list(binding['errors'])
    result['release_gaps'] = list(product['release_gaps']) + [
        dict(row, obligation_family='approved-binding-journey') for row in binding['release_gaps']]
    # Already-blocked assessments retain their original gap inventory. A
    # claimed pass additionally requires every fixed machine group, even if a
    # caller has accidentally dropped those gaps from its summary.
    if binding['status'] == 'passed':
        known = {(row.get('case_id'), row.get('layer')) for row in binding['release_gaps']
                 if isinstance(row, dict)}
        result['release_gaps'].extend(
            dict(row, obligation_family='approved-binding-journey')
            for row in binding_execution_gaps(binding.get('execution_obligations'))
            if (row['case_id'], row['layer']) not in known)
    if result['errors']:
        result['status'] = 'failed'
    elif result['release_gaps'] or binding['status'] != 'passed':
        result['status'] = 'blocked'
    return result


def advancement_errors(verdict):
    if (not isinstance(verdict, dict) or verdict.get('kind') != 'product-completion-verdict/v1'
            or verdict.get('product_completion') != 'complete'
            or verdict.get('product_gate_status') != 'passed'
            or verdict.get('next_stage_allowed') is not True
            or verdict.get('reasons') != [] or verdict.get('capture_scope_gaps') != []):
        return ['Product acceptance is incomplete; advancing or claiming completion is forbidden']
    return []


def validate_report(root, metadata, steps, report, **kwargs):
    expected = assess_completion(root, metadata, steps, **kwargs)
    if type(report) is not dict or json.dumps(report, sort_keys=True) != json.dumps(expected, sort_keys=True):
        return ['Product completion differs from independently recomputed current obligations and evidence']
    return []


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--change', required=True)
    parser.add_argument('--receipt', help='actual engineering receipt, never a producer completion claim')
    parser.add_argument('--evidence')
    parser.add_argument('--artifact')
    parser.add_argument('--require-complete', action='store_true')
    parser.add_argument('--claim', help='optional claimed verdict; independently recomputed, never trusted')
    args = parser.parse_args()
    root = args.root.resolve()
    if '/' in args.change or '\\' in args.change or args.change in ('', '.', '..'):
        parser.error('safe change id required')
    metadata = json.loads(safe_path(root, f'openspec/changes/{args.change}/verification.json').read_text())
    receipt = json.loads(safe_path(root, args.receipt).read_text()) if args.receipt else {}
    verdict = assess_completion(root, metadata, receipt.get('steps', []),
                                execution_source_sha256=receipt.get('source_tree_start_sha256', receipt.get('source_tree_sha256')),
                                evidence=args.evidence, artifact=args.artifact)
    print(json.dumps(verdict, ensure_ascii=False, indent=2))
    if args.claim:
        claim = json.loads(safe_path(root, args.claim).read_text())
        if json.dumps(claim, sort_keys=True) != json.dumps(verdict, sort_keys=True):
            print('Claim rejected: product completion does not equal current inspected facts', file=sys.stderr)
            return 1
    return 1 if args.require_complete and advancement_errors(verdict) else 0


if __name__ == '__main__':
    raise SystemExit(main())
