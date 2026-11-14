#!/usr/bin/env python3
"""Assess a fresh full BIND run without turning missing OS evidence into acceptance.

Only the original producer and independent inspector establish behavior. This
adapter preserves their nonzero result and separates a QA assessment from the
unchanged full product qualification matrix.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys

from binding_phase_guard import load, path_at, ref, sha, source_map, tree_sha, checked_ref, unique
from gate_core import source_files
from binding_journey import (CHECK_ID, COMPONENT_CASES, COMPONENT_SCENES,
                             EXACT_EXECUTION_OBLIGATION_IDS, REQUIRED_LAYERS)

ROOT = Path(__file__).resolve().parents[2]
EXTERNAL_LAYERS = frozenset({'native', 'game', 'service', 'design'})
EXTERNAL_OBLIGATIONS = frozenset({'EX-NATIVE-SUCCESS-ROLES', 'EX-NATIVE-PROGRESS-TRACES', 'EX-LIFE-SIGNED-NATIVE'})
BLOCKED_SUMMARY = 'Actual component scene input/capability remains blocked'
MISSING_NATIVE = 'Actual fixed signed-package Native producer evidence is missing'
NONCE = re.compile(r'[a-f0-9]{32}\Z')
# Recomputed Rust receipts have fresh replay IDs. Everything affecting the
# verdict, source, raw invocation and complete layer/obligation inventory stays.
PROJECTION_KEYS = ('schema_version', 'kind', 'status', 'errors', 'validation_errors',
                   'infrastructure_errors', 'semantic_failures', 'source_tree_sha256',
                   'case_count', 'passed', 'failed', 'blocked', 'diagnostic_count',
                   'scene_count', 'layer_results', 'layer_failures', 'obligation_failures',
                   'execution_obligations', 'storage_v5_inspection', 'eligible_semantic_red',
                   'product_acceptance_promoted', 'native_acceptance_promoted', 'execution',
                   'component_report', 'assertion_count')


def inspection_projection(value):
    result = {key: value.get(key) for key in PROJECTION_KEYS}
    recomputation = value.get('recomputation') or {}
    result['recomputation'] = {key: recomputation.get(key) for key in (
        'kind', 'schema_version', 'errors', 'blocked_obligations', 'case_statuses',
        'native_executed', 'product_acceptance_promoted', 'rust_replay')}
    result['independent_layout'] = {key: (recomputation.get('independent_component_layout') or {}).get(key)
                                    for key in ('errors', 'executed_steps', 'scope')}
    return result


def derive_readiness(inspection, qualification, context):
    """Typed independent facts only; unknown, malformed or failed facts fail closed."""
    errors = []
    try:
        if (inspection.get('schema_version') != 1 or inspection.get('kind') != 'binding-journey-inspection/v1'
                or qualification.get('schema_version') != 1
                or qualification.get('kind') != 'binding-journey-global-assessment/v1'):
            raise ValueError('Original full inspection/global qualification identity required')
        for key in ('validation_errors', 'infrastructure_errors', 'semantic_failures', 'layer_failures', 'obligation_failures'):
            if inspection.get(key) != []:
                errors.append('Actual full ' + key + ' must be empty')
        if (inspection.get('failed') != 0 or inspection.get('blocked') != 0
                or inspection.get('case_count') != len(COMPONENT_CASES) + len(COMPONENT_SCENES)
                or inspection.get('passed') != inspection.get('case_count')
                or inspection.get('diagnostic_count') != len(COMPONENT_CASES)
                or inspection.get('scene_count') != len(COMPONENT_SCENES)
                or inspection.get('eligible_semantic_red') is not False):
            errors.append('All original diagnostics and registered component scenes must actually pass')
        for value in (inspection, qualification):
            if value.get('product_acceptance_promoted') is not False or value.get('native_acceptance_promoted') is not False:
                errors.append('A readiness assessment cannot promote Native/product acceptance')
        recomputation = inspection.get('recomputation') or {}
        if (recomputation.get('kind') != 'binding-journey-independent-recomputation/v1'
                or recomputation.get('errors') != [] or recomputation.get('blocked_obligations') != []
                or recomputation.get('case_statuses') != {case: 'passed' for case in (*COMPONENT_CASES, *COMPONENT_SCENES)}):
            errors.append('Independent recomputation must cover every original diagnostic and scene without errors or blocked inputs')
        native_absent, service_absent = context.get('native_evidence') is None, context.get('service_evidence') is None
        expected = {(case, layer) for case, layers in REQUIRED_LAYERS.items() for layer in layers}
        observed = {}
        for row in inspection['layer_results']:
            pair = (row['case_id'], row['layer'])
            if pair not in expected or pair in observed:
                raise ValueError('Unknown or duplicate full BIND layer')
            if row['status'] != 'passed' or row.get('errors', []):
                raise ValueError('An observed full layer failed or remains incomplete')
            observed[pair] = row
        if any((native_absent and layer in {'native', 'design'}) or (service_absent and layer in {'service', 'game'})
               for _, layer in observed):
            errors.append('External covered layers cannot be borrowed without actual provided input')
        missing = expected - set(observed)
        allowed_missing = {(case, layer) for case, layer in expected
                           if (native_absent and layer in {'native', 'design'})
                           or (service_absent and layer in {'service', 'game'})}
        if not missing <= allowed_missing:
            errors.append('A local or provided-external full layer is missing')
        if set((row['case_id'], row['layer']) for row in qualification['covered_layers']) != set(observed):
            errors.append('Global coverage differs from actual independently inspected layers')
        obligations = inspection['execution_obligations']
        groups = obligations['groups']
        if (obligations.get('kind') != 'binding-execution-obligation-assessment/v1'
                or obligations.get('schema_version') != 1 or obligations.get('product_acceptance_promoted') is not False
                or [row.get('id') for row in groups] != list(EXACT_EXECUTION_OBLIGATION_IDS)
                or qualification.get('execution_obligations') != obligations):
            raise ValueError('All nine original machine obligations must match the independent global assessment')
        incomplete = []
        for group in groups:
            identity, status = group['id'], group['status']
            if status == 'passed':
                if group.get('errors') != [] or not group.get('checks') or not group.get('artifacts'):
                    errors.append('Passed obligation lacks actual checks/artifacts: ' + identity)
                continue
            incomplete.append((identity, status))
            absent = native_absent and identity in EXTERNAL_OBLIGATIONS
            # With Native supplied, the signed group can remain blocked only for
            # the independently missing four-game/service evidence. Its precise
            # inspector error is retained; arbitrary blocked strings never qualify.
            signed_service = identity == 'EX-LIFE-SIGNED-NATIVE' and not native_absent and service_absent
            accepted_errors = [MISSING_NATIVE] if absent else [
                'Native BJ07 does not discharge separate same-package four-game/BJ16 and real service obligations']
            if (status != 'blocked' or not (absent or signed_service)
                    or group.get('errors') not in ([], accepted_errors)):
                errors.append('Failed or nonexternal incomplete execution obligation: ' + identity)
        storage = inspection['storage_v5_inspection']
        child = storage.get('child_execution') or {}
        if (storage.get('status') != 'passed' or storage.get('errors') != []
                or child.get('kind') != 'schema5-child-execution/v1'
                or [len(child.get(key, [])) for key in ('parent_timeline_routes', 'fault_routes', 'child_tests')] != [9, 4, 3]):
            errors.append('Strict current schema5 runtime and actual parent/child timeline proof must pass')
        actual_gaps = [(row['case_id'], row['layer'], row['status']) for row in qualification['release_gaps']]
        expected_gaps = [(case, layer, 'not_run') for case, layer in missing]
        expected_gaps += [(identity, 'registered_execution_obligation', status) for identity, status in incomplete]
        if len(actual_gaps) != len(set(actual_gaps)) or set(actual_gaps) != set(expected_gaps):
            errors.append('Global release gaps omitted, renamed or invented a fixed layer/obligation')
        raw_blocked = bool(incomplete)
        if (inspection.get('status') != ('blocked' if raw_blocked else 'passed')
                or inspection.get('errors') != ([BLOCKED_SUMMARY] if raw_blocked else [])
                or qualification.get('status') != ('blocked' if expected_gaps else 'passed')
                or qualification.get('errors') != []):
            errors.append('Original full exit/status/errors differ from the exact typed missing evidence')
    except (ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append(str(error))
    status = 'failed' if errors else 'passed'
    return {'schema_version': 1, 'kind': 'binding-journey-readiness-assessment/v1',
            'status': status, 'assessment_status': status, 'errors': errors,
            'binding_qualification': qualification, 'full_binding_result': inspection,
            'product_completion': 'incomplete', 'next_stage_allowed': False,
            'product_acceptance_promoted': False, 'native_acceptance_promoted': False}


def _write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def _command(output, nonce, native, service):
    command = [sys.executable, 'scripts/quality/binding_journey.py', '--execute',
               '--output', output + '/full', '--nonce', nonce, '--json']
    if native:
        command += ['--native-evidence', native]
    if service:
        command += ['--service-evidence', service]
    return command


def _independent(root, directory, execution):
    """Read original raw process facts and independently reconstruct all verdicts."""
    from binding_journey_inspect import inspect_execution
    from binding_journey import global_assessment
    relative = directory.relative_to(root).as_posix()
    context = load(directory / 'context.json')
    before = source_map(load(checked_ref(root, context['source_before'], ignored=True)))
    after = source_map(load(checked_ref(root, context['source_after'], ignored=True)))
    if (context.get('kind') != 'binding-readiness-actual-invocation/v1'
            or context.get('schema_version') != 1 or not NONCE.fullmatch(context.get('nonce', ''))
            or before != after or before != source_files(root)
            or context.get('source_tree_sha256') != tree_sha(before)):
        raise ValueError('Readiness must retain a fresh, unchanged current source epoch')
    actual = load(checked_ref(root, execution, ignored=True))
    native, service = context['native_evidence'], context['service_evidence']
    expected = _command(relative, context['nonce'], native['path'] if native else None, service['path'] if service else None)
    if (actual.get('kind') != 'actual-binding-readiness-child/v1'
            or actual.get('command') != expected or actual.get('cwd') != str(root)
            or type(actual.get('pid')) is not int or actual['pid'] <= 0
            or actual.get('timed_out') is not False
            or actual.get('source_tree_sha256') != tree_sha(before)
            or not datetime.fromisoformat(context['started_at']) <= datetime.fromisoformat(actual['started_at'])
                <= datetime.fromisoformat(actual['finished_at']) <= datetime.fromisoformat(context['finished_at'])):
        raise ValueError('Original full CLI actual PID/argv/source/time binding differs')
    for value in (native, service):
        if value:
            checked_ref(root, value, ignored=True)
    stdout = checked_ref(root, actual['stdout'], ignored=True).read_text()
    checked_ref(root, actual['stderr'], ignored=True)
    reported = json.loads(stdout, object_pairs_hook=unique)
    invocation = load(directory / 'full/execution.json')
    if (invocation.get('pid') != actual['pid'] or invocation.get('nonce') != context['nonce']
            or invocation.get('source_tree_sha256') != tree_sha(before)
            or invocation.get('native_evidence') != native or invocation.get('service_evidence') != service
            or not datetime.fromisoformat(actual['started_at']) <= datetime.fromisoformat(invocation['started_at'])
                <= datetime.fromisoformat(invocation['finished_at']) <= datetime.fromisoformat(actual['finished_at'])):
        raise ValueError('Full producer belongs to another process/nonce/source/input cohort')
    inspected = inspect_execution(root, directory / 'full', require_current=True)
    if (inspection_projection(reported) != inspection_projection(inspected)
            or actual['exit_code'] != (0 if inspected.get('status') == 'passed' else 1)):
        raise ValueError('Raw full CLI exit/typed verdict differs from independent inspection')
    qualification = global_assessment(root, [{'id': CHECK_ID, 'execution': inspected}])
    report = derive_readiness(inspected, qualification, context)
    report.update(nonce=context['nonce'], source_tree_sha256=tree_sha(before),
                  actual_full_exit_code=actual['exit_code'], actual_full_execution=execution,
                  context=ref(root, relative + '/context.json'))
    return report


def execute_assessment(root=ROOT, output=None, *, nonce=None, native_evidence=None, service_evidence=None):
    root = Path(root).resolve()
    if not isinstance(output, str) or not output.startswith('.build/'):
        raise ValueError('Fresh ignored readiness output required')
    directory = path_at(root, output)
    directory.mkdir(parents=True, exist_ok=False)
    nonce = nonce or secrets.token_hex(16)
    if not NONCE.fullmatch(nonce):
        raise ValueError('Fresh 32-hex nonce required')
    before = source_files(root)
    _write(directory / 'source-before.json', before)
    context = {'schema_version': 1, 'kind': 'binding-readiness-actual-invocation/v1',
               'nonce': nonce, 'started_at': datetime.now(timezone.utc).isoformat(),
               'source_tree_sha256': tree_sha(before), 'source_before': ref(root, output + '/source-before.json'),
               'native_evidence': ref(root, native_evidence) if native_evidence else None,
               'service_evidence': ref(root, service_evidence) if service_evidence else None}
    command = _command(output, nonce, native_evidence, service_evidence)
    started = datetime.now(timezone.utc).isoformat()
    process = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=3600)
    except subprocess.TimeoutExpired:
        timed_out = True
        process.kill()
        stdout, stderr = process.communicate()
    (directory / 'full.stdout.log').write_text(stdout)
    (directory / 'full.stderr.log').write_text(stderr)
    actual = {'schema_version': 1, 'kind': 'actual-binding-readiness-child/v1',
              'command': command, 'cwd': str(root), 'pid': process.pid, 'started_at': started,
              'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': process.returncode,
              'timed_out': timed_out, 'source_tree_sha256': tree_sha(before),
              'stdout': ref(root, output + '/full.stdout.log'), 'stderr': ref(root, output + '/full.stderr.log'),
              'allowlisted_environment': {key: os.environ[key] for key in (
                  'LUNA_HTTP_OLD_READER_MANIFEST', 'FP_STORAGE_OLD_STORE3_READER_MANIFEST',
                  'FP_STORAGE_OLD_STORE4_READER_MANIFEST') if key in os.environ}}
    _write(directory / 'actual-full-execution.json', actual)
    _write(directory / 'source-after.json', source_files(root))
    context.update(finished_at=datetime.now(timezone.utc).isoformat(), source_after=ref(root, output + '/source-after.json'))
    _write(directory / 'context.json', context)
    try:
        result = _independent(root, directory, ref(root, output + '/actual-full-execution.json'))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        result = {'schema_version': 1, 'kind': 'binding-journey-readiness-assessment/v1',
                  'status': 'failed', 'assessment_status': 'failed', 'errors': [str(error)],
                  'actual_full_exit_code': process.returncode, 'full_binding_result': {},
                  'binding_qualification': {'status': 'failed'}, 'product_completion': 'incomplete',
                  'product_acceptance_promoted': False, 'next_stage_allowed': False}
    result['assessment_receipt'] = output + '/assessment.json'
    _write(directory / 'assessment.json', result)
    return result


def assessment_errors(root, result):
    """Reopen raw authority; never accept an author status-only summary."""
    try:
        root = Path(root).resolve()
        path = path_at(root, result['assessment_receipt'])
        if not path.is_relative_to(root / '.build') or load(path) != result:
            raise ValueError('Exact ignored assessment receipt required')
        rebuilt = _independent(root, path.parent, result['actual_full_execution'])
        if (rebuilt['assessment_status'] != result['assessment_status'] or rebuilt['errors'] != result['errors']
                or inspection_projection(rebuilt['full_binding_result']) != inspection_projection(result['full_binding_result'])
                or rebuilt['binding_qualification']['release_gaps'] != result['binding_qualification']['release_gaps']
                or result.get('next_stage_allowed') is not False or result.get('product_completion') != 'incomplete'
                or result.get('product_acceptance_promoted') is not False):
            raise ValueError('Assessment summary differs from independently reconstructed full facts')
        return rebuilt['errors']
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        return [str(error)]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute', action='store_true', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--nonce')
    parser.add_argument('--native-evidence')
    parser.add_argument('--service-evidence')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        result = execute_assessment(output=args.output, nonce=args.nonce,
                                    native_evidence=args.native_evidence, service_evidence=args.service_evidence)
    except (OSError, ValueError, KeyError, TypeError) as error:
        result = {'status': 'failed', 'assessment_status': 'failed', 'errors': [str(error)],
                  'product_completion': 'incomplete', 'next_stage_allowed': False}
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result['assessment_status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
