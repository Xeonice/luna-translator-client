#!/usr/bin/env python3
"""Current unified gate runner: fixed commands, real receipts, independent verdicts."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import uuid

from gate_policy import (atomic_write_json, classify_impact, evaluate, load_context,
                         map_identity, read_json, registered_check_kind, snapshot_inputs)
from gate_core import safe_path
from gate_checks import (RELEASE_CHECKS, release_input_identity,
                         verified_scope as check_scope)

ROOT = Path(__file__).resolve().parents[2]
PREFIX = '.build/quality/effective-gates'
GATE_NAMES = dict(zip((f'G{i:02d}' for i in range(1, 10)), (
    '需求与产品范围', '模块边界', '目录与修改范围', '技术栈与资源', '真实编译',
    '功能与交互', '数据库与存储', '正式包与实机', '视觉对照与人工确认')))
MANDATORY = {
    'G01': ('spec-contract', 'openspec-strict'),
    'G02': ('frontend-boundaries', 'rust-boundaries'),
    'G04': ('technology-contract',),
    'G05': ('typescript-check', 'typescript-test-check', 'rust-check', 'rust-test-compile'),
    'G06': ('affected-functional-tests', 'functional-component'),
    'G07': ('storage-contract', 'storage-runtime'),
    'G08': ('artifact-package-check', 'real-game-regression', 'real-translation-service'),
}


def now():
    return datetime.now(timezone.utc).isoformat()


def run_directory(change_id, run_id):
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]*', change_id) or not re.fullmatch(r'[a-zA-Z0-9_-]+', run_id):
        raise ValueError('invalid run identity')
    return f'{PREFIX}/{change_id}/{run_id}'


def command_registry(root, change_id, run_id, *, base=None, release=False):
    """Only code-owned argv. Metadata cannot add or replace an executable."""
    root = Path(root)
    out = run_directory(change_id, run_id)
    metadata_path = root / 'openspec/changes' / change_id / 'verification.json'
    metadata = read_json(metadata_path) if metadata_path.is_file() else {}
    external = metadata.get('effective_gate_external_evidence')
    effectiveness = [sys.executable, 'scripts/quality/gate_effectiveness.py', '--root', str(root),
                     '--output', out + '/effectiveness']
    if external:
        safe_path(root, external)
        effectiveness += ['--external-evidence', external]
    def adapter(name):
        result = [sys.executable, 'scripts/quality/gate_checks.py', '--check', name, '--change', change_id]
        if base is not None:
            result += ['--base', base]
        if release:
            result.append('--release')
        return result
    return {
        'spec-contract': adapter('spec-contract'),
        'source-scope': adapter('source-scope'),
        'openspec-strict': ['node', 'node_modules/@fission-ai/openspec/bin/openspec.js',
                            'validate', change_id, '--strict', '--no-interactive'],
        'frontend-boundaries': adapter('frontend-boundaries'),
        'rust-boundaries': adapter('rust-boundaries'),
        'technology-contract': adapter('technology-contract'),
        'typescript-check': ['npm', 'run', 'typecheck', '--prefix', 'apps/desktop'],
        'typescript-test-check': ['npm', 'run', 'typecheck:tests', '--prefix', 'apps/desktop'],
        'rust-check': ['cargo', 'check', '--workspace', '--locked', '--offline'],
        'rust-test-compile': ['cargo', 'test', '--workspace', '--no-run', '--locked', '--offline'],
        'affected-functional-tests': ['cargo', 'test', '-p', 'app-core', '--test', 'binding_journey',
                                      '--locked', '--offline', '--', '--nocapture'],
        'functional-component': ['node', 'apps/desktop/tests/visual/verify-library-binding.mjs',
                                 '--output', out + '/production-component'],
        'storage-contract': adapter('storage-contract'),
        'storage-runtime': ['cargo', 'test', '-p', 'app-infra', '--test', 'storage_product',
                             '--locked', '--offline', '--', '--nocapture'],
        'storage-migration': ['cargo', 'test', '-p', 'app-infra', '--lib',
                              'actual_migration_host_sql_failure_after_ddl_rolls_back_and_then_recovers',
                              '--locked', '--offline', '--', '--nocapture'],
        'gate-policy-tests': [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/quality',
                               '-p', 'test_gate_policy.py', '-v'],
        'codex-hook-tests': [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/quality',
                              '-p', 'test_codex_hooks.py', '-v'],
        'gate-runner-tests': [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/quality',
                                '-p', 'test_development_gates.py', '-v'],
        'gate-effectiveness-tests': [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/quality',
                                       '-p', 'test_gate_effectiveness.py', '-v'],
        'github-ci-tests': [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/quality',
                             '-p', 'test_github_ci.py', '-v'],
        'gate-effectiveness-execution': effectiveness,
        'artifact-package-check': adapter('artifact-package-check'),
        'real-game-regression': adapter('real-game-regression'),
        'real-translation-service': adapter('real-translation-service'),
    }


def test_counts(output):
    python = re.findall(r'Ran (\d+) tests?\b', output)
    rust = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', output)
    tap = re.findall(r'^# tests (\d+)$', output, re.M)
    if python:
        return int(python[-1]), sum(map(int, re.findall(r'skipped=(\d+)', output)))
    if rust:
        return sum(int(x[0]) for x in rust), sum(int(x[2]) for x in rust)
    if tap:
        return int(tap[-1]), sum(map(int, re.findall(r'^# (?:skipped|todo|cancelled) (\d+)$', output, re.M)))
    # Production component runner publishes its own actual check list/count.
    for start in reversed([m.start() for m in re.finditer(r'(?m)^\{', output)]):
        try:
            item = json.loads(output[start:].strip())
        except json.JSONDecodeError:
            continue
        if (item.get('status') == 'passed' and type(item.get('checks')) is int
                and item.get('failed') == [] and item.get('errors') == []):
            return item['checks'], 0
        if (item.get('kind') == 'gate-effectiveness-run/v1'
                and item.get('full_catalogue_executed') is True
                and isinstance(item.get('scenarios'), list)):
            return len(item['scenarios']), sum(row.get('status') == 'not_run' for row in item['scenarios'])
    return 0, 0


def execute(root, change_id, run_id, check_id, gate_id, inputs, *, timeout=900, base=None, release=False):
    argv = command_registry(root, change_id, run_id, base=base, release=release)[check_id]
    relative = run_directory(change_id, run_id)
    directory = safe_path(root, relative)
    directory.mkdir(parents=True, exist_ok=True)
    start = now()
    began = time.monotonic()
    environment = {**os.environ, 'OPENSPEC_TELEMETRY': '0', 'DO_NOT_TRACK': '1',
                   'OPENSPEC_NO_UPDATE_CHECK': '1'}
    external_inputs, stdout, stderr = None, '', ''
    try:
        if check_id in RELEASE_CHECKS:
            external_inputs = release_input_identity(root, load_context(root, change_id))
        result = subprocess.run(argv, cwd=root, env=environment, text=True,
                                capture_output=True, timeout=timeout)
        stdout, stderr, code = result.stdout, result.stderr, result.returncode
        if check_id in RELEASE_CHECKS and external_inputs != release_input_identity(root, load_context(root, change_id)):
            raise ValueError('release.external_inputs_changed_during_execution')
    except (OSError, ValueError, TypeError, KeyError, subprocess.TimeoutExpired) as exc:
        stderr, code = stderr + '\n' + str(exc), -1
    streams = {}
    for name, content in (('stdout', stdout), ('stderr', stderr)):
        target = directory / f'{check_id}.{name}.log'
        target.write_text(content)
        streams[name] = {'path': target.relative_to(root).as_posix(),
                         'sha256': hashlib.sha256(target.read_bytes()).hexdigest()}
    count, skipped = test_counts(stdout + '\n' + stderr)
    is_test = registered_check_kind(check_id) == 'tests'
    receipt = {'kind': 'gate-command-receipt/v1', 'schema_version': 1,
               'run_id': run_id, 'check_id': check_id, 'gate_id': gate_id,
               'change_id': change_id, 'command': argv, 'started_at': start,
               'finished_at': now(), 'exit_code': code, 'inputs': inputs,
               **streams, 'test_count': count, 'skipped_count': skipped,
               'check_kind': 'tests' if is_test else 'command', 'fixture': False,
               'elapsed_seconds': round(time.monotonic() - began, 3)}
    if check_id in RELEASE_CHECKS:
        receipt['external_inputs'] = external_inputs
    if check_id == 'functional-component':
        observation = directory / 'production-component/result.json'
        if observation.is_file():
            receipt['test_observation'] = {
                'path': observation.relative_to(root).as_posix(),
                'sha256': hashlib.sha256(observation.read_bytes()).hexdigest()}
    atomic_write_json(root, f'{relative}/{check_id}.receipt.json', receipt)
    print(f'[{"passed" if code == 0 else "failed"}] {check_id} ({receipt["elapsed_seconds"]}s)', flush=True)
    if code:
        print((stdout + '\n' + stderr)[-1600:], flush=True)
    return receipt


def current_record(root, change_id):
    path = safe_path(root, f'{PREFIX}/{change_id}/latest.json')
    return read_json(path) if path.exists() else {}


def required_visual_scope(root, paths, *, release=False):
    """Use the existing actual production dependency/state registry."""
    impact = classify_impact(paths, stage='release' if release else 'delivery')
    if not (impact['ui'] or impact['native'] or release):
        return []
    from current_design_alignment import required_states
    states, _ = required_states(None if release else paths, root=Path(root))
    return sorted(states)


def mandatory_checks(paths, *, full_engineering=False):
    result = dict(MANDATORY)
    if full_engineering or any(path.startswith(('scripts/quality/', '.codex/', '.githooks/')) for path in paths):
        result['G02'] = (*result['G02'], 'gate-policy-tests', 'codex-hook-tests',
                         'gate-runner-tests', 'gate-effectiveness-tests', 'github-ci-tests', 'gate-effectiveness-execution')
    return result


def assess_current(root, context=None, stage='delivery'):
    root = Path(root).resolve()
    context = dict(context or load_context(root))
    change = context['change_id']
    previous = current_record(root, change)
    run_id = previous.get('run_id', 'not-run')
    context['receipts'] = previous.get('receipts', {})
    context['reviews'] = {**previous.get('reviews', {}), **context.get('reviews', {})}
    context['visual'] = {**previous.get('visual', {}), **context.get('visual', {})}
    review_ref = context.get('review_evidence', {}).get('path')
    if 'G01' not in context['reviews'] and review_ref and safe_path(root, review_ref).is_file():
        context['reviews']['G01'] = review_ref
    context['stage'] = 'development' if stage in {'quick', 'engineering', 'ci'} else 'delivery'
    context['require_product'] = stage == 'release'
    ci = os.environ.get('CI', '').lower() not in {'', 'false', '0', 'no', 'off'}
    actual_paths, scope_errors = check_scope(root, context, previous.get('base'), ci=ci)
    visual_scope = []
    visual_scope_error = None
    if not scope_errors:
        try:
            visual_scope = required_visual_scope(root, actual_paths, release=stage == 'release')
        except (ValueError, OSError, KeyError, TypeError) as exc:
            # Missing impact/capture mapping is an explicit G09 blocker. It
            # must not obscure a distinct G03 violation or become an approval.
            visual_scope_error = str(exc)
    result = evaluate(root, context, expected_commands=command_registry(root, change, run_id,
                      base=previous.get('base'), release=stage == 'release'),
                      mandatory_checks=mandatory_checks(actual_paths, full_engineering=stage == 'ci'),
                      full_engineering=stage == 'ci',
                      actual_changed_paths=actual_paths,
                      visual_required_scope=visual_scope,
                      visual_authorized_reviewers={'user'},
                      authorized_reviewers={'user', 'effective_gates_independent'})
    if visual_scope_error:
        result.setdefault('errors', []).append('visual_scope_unavailable: ' + visual_scope_error)
    if scope_errors:
        gate = next(row for row in result['gates'] if row['gate_id'] == 'G03')
        gate.update(status='failed', reason='scope.actual_diff_rejected', details=scope_errors)
        result.update(status='failed', incomplete=True, next_stage_allowed=False)
    if 'G08' in result.get('impact', {}).get('required_gates', []):
        external_errors = []
        try:
            actual_external = release_input_identity(root, context)
            for receipt in context['receipts'].get('G08', []):
                if isinstance(receipt, str):
                    receipt = read_json(safe_path(root, receipt))
                if (receipt.get('check_id') in RELEASE_CHECKS
                        and receipt.get('external_inputs') != actual_external):
                    external_errors.append('release.external_inputs_changed:' + receipt['check_id'])
        except (ValueError, OSError, TypeError, KeyError) as exc:
            external_errors.append('release.input_identity_unavailable:' + str(exc))
        if external_errors:
            gate = next(row for row in result['gates'] if row['gate_id'] == 'G08')
            gate.update(status='failed', reason='release.external_inputs_changed', details=external_errors)
            result.update(status='failed', incomplete=True, next_stage_allowed=False)
    if result.get('source_inputs_sha256') != map_identity(snapshot_inputs(root)):
        result.update(status='failed', incomplete=True, next_stage_allowed=False)
        result.setdefault('errors', []).append('source_changed_during_assessment')
    release_complete = stage == 'release' and result.get('next_stage_allowed') is True
    result.update(stage=stage, run_id=run_id, assessed_at=now(),
                  overall=result.get('status'), product_completion='complete' if release_complete else 'incomplete',
                  next_product_stage_allowed=release_complete, product_acceptance_promoted=False)
    return result


def run_current(root, context=None, stage='engineering', *, base=None):
    root = Path(root).resolve()
    context = dict(context or load_context(root))
    change = context['change_id']
    run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S') + '-' + uuid.uuid4().hex[:8]
    inputs = snapshot_inputs(root)
    ci = os.environ.get('CI', '').lower() not in {'', 'false', '0', 'no', 'off'}
    paths, scope_errors = check_scope(root, context, base, ci=ci)
    impact = classify_impact(paths, stage='release' if stage == 'release' else
                             ('development' if stage in {'quick', 'engineering', 'ci'} else 'delivery'),
                             full_engineering=stage == 'ci')
    selected = []
    for gate_id in impact['required_gates']:
        for check_id in mandatory_checks(paths, full_engineering=stage == 'ci').get(gate_id, ()):
            if stage == 'quick' and gate_id not in {'G01', 'G02', 'G04'}:
                continue
            selected.append((gate_id, check_id))
    if stage == 'quick':
        selected = [(gate, check) for gate, check in selected if check not in {
            'gate-policy-tests', 'codex-hook-tests', 'gate-runner-tests',
            'gate-effectiveness-tests', 'github-ci-tests', 'gate-effectiveness-execution'}]
    old = current_record(root, change)
    record = {'schema_version': 1, 'run_id': run_id, 'change_id': change,
              'receipts': {}, 'reviews': {**old.get('reviews', {}), **context.get('reviews', {})},
              'visual': {**old.get('visual', {}), **context.get('visual', {})},
              'base': base,
              'actual_changed_paths': paths, 'product_acceptance_promoted': False}
    for gate_id, check_id in selected:
        receipt = execute(root, change, run_id, check_id, gate_id, inputs, base=base, release=stage == 'release')
        record['receipts'].setdefault(gate_id, []).append(receipt)
    # The actual source-scope CLI independently recomputes Git/snapshot diff.
    scope_receipt = execute(root, change, run_id, 'source-scope', 'G03', inputs, base=base, release=stage == 'release')
    record['receipts']['G03'] = [scope_receipt]
    atomic_write_json(root, f'{PREFIX}/{change}/latest.json', record)
    result = assess_current(root, context, stage=stage)
    result['execution_source_changed'] = snapshot_inputs(root) != inputs
    if result['execution_source_changed']:
        result.update(status='failed', overall='failed', product_completion='incomplete', next_stage_allowed=False)
        result.setdefault('errors', []).append('source_changed_during_execution')
    if any(receipt['exit_code'] not in {0, 2} for rows in record['receipts'].values() for receipt in rows):
        result.update(status='failed', overall='failed', incomplete=True, next_stage_allowed=False)
    elif any(receipt['exit_code'] == 2 for rows in record['receipts'].values() for receipt in rows) and result['status'] == 'passed':
        result.update(status='blocked', overall='blocked', incomplete=True, next_stage_allowed=False)
    if stage == 'quick':
        quick = [receipt for rows in record['receipts'].values() for receipt in rows]
        result['quick_check_status'] = 'passed' if quick and all(r['exit_code'] == 0 for r in quick) and not scope_errors and not result['execution_source_changed'] else 'failed'
        result['quick_check_grants_delivery'] = False
    atomic_write_json(root, run_directory(change, run_id) + '/result.json', result)
    from render_gate_report import render
    render(root, result, record)
    return result


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--change')
    p.add_argument('--stage', choices=['quick', 'engineering', 'ci', 'delivery', 'release'], default='engineering')
    p.add_argument('--profile', choices=['development', 'release'])
    p.add_argument('--base')
    p.add_argument('--assess', action='store_true')
    p.add_argument('--self-test', action='store_true')
    p.add_argument('--require-product-complete', action='store_true')
    args = p.parse_args(argv)
    try:
        context = load_context(args.root, args.change)
        if args.self_test:
            run_id = 'selftest-' + uuid.uuid4().hex[:12]
            return subprocess.call(command_registry(args.root, context['change_id'], run_id)[
                'gate-effectiveness-execution'], cwd=args.root)
        stage = 'release' if args.profile == 'release' or args.require_product_complete else args.stage
        result = assess_current(args.root, context, stage) if args.assess else run_current(args.root, context, stage, base=args.base)
        print(json.dumps(result, ensure_ascii=False))
        verdict = result.get('quick_check_status') if stage == 'quick' and not args.assess else result.get('status')
        return 0 if verdict == 'passed' else 1
    except (ValueError, OSError, TypeError, KeyError) as exc:
        print(json.dumps({'status': 'failed', 'error': str(exc), 'product_completion': 'incomplete',
                          'next_stage_allowed': False}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
