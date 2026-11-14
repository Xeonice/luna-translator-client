#!/usr/bin/env python3
"""Execute gate-effectiveness experiments, never product acceptance.

The experiment process owns its assertions and raw command logs. Production
checkers are invoked on disposable sources or real read-only database snapshots;
an arbitrary non-zero exit never counts as a successful negative control.
"""
from __future__ import annotations

import argparse
import copy
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import traceback
from typing import Any, Callable
import uuid
import zlib

ROOT = Path(__file__).resolve().parents[2]
CATALOGUE = (
    ('GV-01', 'G01', 'Product scope and real CLI mapping'),
    ('GV-02', 'G01/G06', 'Correct IDs do not substitute production behavior'),
    ('GV-03', 'G09', 'Comparable real implementation and design report'),
    ('GV-04', 'G02', 'Actual frontend and Rust dependency boundaries'),
    ('GV-05', 'G03', 'Actual paths, new files and path escapes'),
    ('GV-06', 'G04', 'Approved dependency and manifest identity'),
    ('GV-07', 'G04/G08', 'Core architecture and packaged resources'),
    ('GV-08', 'G05', 'Actual TypeScript compilation'),
    ('GV-09', 'G05', 'Actual Rust test compilation'),
    ('GV-10', 'G06', 'Production operations execute exactly once'),
    ('GV-11', 'G06', 'Cancellation and late-result isolation'),
    ('GV-12', 'G07', 'Real SQLite field types and boundary schema'),
    ('GV-13', 'G07', 'Real SQLite relationships and delete policies'),
    ('GV-14', 'G07', 'Production migrations and failure recovery'),
    ('GV-15', 'G07', 'Production persistence and backup failure behavior'),
    ('GV-16', 'G08', 'Fixture-only qualification aggregation'),
    ('GV-17', 'HOOK', 'Actual SessionStart context loading'),
    ('GV-18', 'HOOK/G03', 'Actual PreToolUse prevents a write'),
    ('GV-19', 'HOOK/G02', 'Actual PostToolUse detects completed mutation'),
    ('GV-20', 'HOOK', 'Actual read-only Stop does not loop'),
    ('GV-21', 'HOOK', 'Actual changed-work Stop checks missing verification'),
    ('GV-22', 'HOOK', 'Actual Stop bounded continuation'),
    ('GV-23', 'HOOK', 'Hook trust, errors and global-hook coexistence'),
    ('GV-24', 'ALL', 'Results become stale on relevant input changes'),
    ('GV-25', 'ALL', 'Actual command and assertion evidence is required'),
    ('GV-26', 'ALL', 'Concurrent runs cannot share or overwrite evidence'),
    ('GV-27', 'ALL', 'Impact dispatch is complete without unnecessary work'),
    ('GV-28', 'ALL', 'Reject and freshness mutations are detected'),
    ('GV-29', 'ALL', 'Required dispatch and skipped tests cannot disappear'),
    ('GV-30', 'HOOK', 'Real Codex events are distinct from script simulation'),
    ('GV-31', 'G09', 'Manual visual review participates in qualification'),
    ('GV-32', 'G09', 'Visual approval has exact scope and expires'),
)
CASE_IDS = tuple(row[0] for row in CATALOGUE)
HOOK_CASES = frozenset([*(f'GV-{n:02}' for n in range(17, 24)), 'GV-30'])
HOOK_REQUIRED_SOURCES = frozenset({
    '.codex/hooks.json', 'scripts/quality/codex_hooks.py',
    'scripts/quality/test_codex_hooks.py', 'scripts/quality/gate_policy.py',
    'scripts/quality/development_gates.py', 'scripts/quality/gate_core.py',
    'scripts/quality/gate_checks.py',
})


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def dump(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n')


def owned(root: Path, relative: str) -> Path:
    """Resolve evidence without trusting symlinks or relative traversal."""
    from gate_core import safe_path
    return safe_path(root, relative)


def png(width: int, height: int, colour=(34, 48, 69)) -> bytes:
    """Tiny deterministic PNG test input, not a product screenshot."""
    def chunk(kind, value):
        return struct.pack('>I', len(value)) + kind + value + struct.pack('>I', zlib.crc32(kind + value) & 0xffffffff)
    raw = b''.join(b'\0' + bytes(colour) * width for _ in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


class Blocked(RuntimeError):
    """A genuine unavailable external execution prerequisite."""


class Experiment:
    def __init__(self, root: Path, output: Path):
        self.root = root.resolve()
        self.output = output.resolve()
        self.output.mkdir(parents=True, exist_ok=False)
        self.run_id = uuid.uuid4().hex
        self.records: list[dict] = []
        self.commands: list[dict] = []
        self.evidence: list[dict] = []
        self.case_id = ''
        self.branch = 'positive'
        self.counter = 0

    def expect(self, actual, expected, label):
        matched = type(actual) is type(expected) and actual == expected
        observation = {'branch': self.branch, 'label': label, 'actual': actual,
                       'expected': expected, 'matched': matched}
        self.records.append(observation)
        if not matched:
            raise AssertionError(f'{label}: expected {expected!r}; got {actual!r}')

    def require(self, condition, label):
        self.expect(bool(condition), True, label)

    @contextmanager
    def phase(self, name):
        old = self.branch
        self.branch = name
        try:
            yield
        finally:
            self.branch = old

    def command(self, argv, *, cwd=None, timeout=60) -> dict:
        self.counter += 1
        name = f'{self.counter:03}-{self.branch}'
        target = self.output / self.case_id
        target.mkdir(exist_ok=True)
        stdout, stderr = target / f'{name}.stdout', target / f'{name}.stderr'
        start = now()
        try:
            result = subprocess.run([str(x) for x in argv], cwd=cwd or self.root,
                                    capture_output=True, text=True, timeout=timeout,
                                    env={**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'})
            out, err, code = result.stdout, result.stderr, result.returncode
        except subprocess.TimeoutExpired as error:
            out = error.stdout or b''
            err = error.stderr or b''
            if isinstance(out, bytes): out = out.decode(errors='replace')
            if isinstance(err, bytes): err = err.decode(errors='replace')
            stdout.write_text(out); stderr.write_text(err)
            raise AssertionError(f'actual checker timed out: {argv!r}') from error
        except FileNotFoundError as error:
            raise Blocked(f'actual execution tool unavailable: {argv[0]}') from error
        stdout.write_text(out)
        stderr.write_text(err)
        item = {'command': [str(x) for x in argv], 'cwd': str(cwd or self.root),
                'started_at': start, 'finished_at': now(), 'exit_code': code,
                'stdout': {'path': str(stdout), 'sha256': sha(stdout)},
                'stderr': {'path': str(stderr), 'sha256': sha(stderr)}}
        self.commands.append(item)
        return {**item, 'out': out, 'err': err}

    def json_command(self, argv, *, expected_exit, expected_error=None, expected_rule=None, cwd=None):
        result = self.command(argv, cwd=cwd)
        self.expect(result['exit_code'], expected_exit, 'actual command exit code')
        try:
            value = json.loads(result['out'])
        except (ValueError, TypeError) as error:
            raise AssertionError('checker must return parseable JSON, not crash or infrastructure output') from error
        if expected_error:
            self.require(expected_error in json.dumps(value, ensure_ascii=False), 'expected rejection identity / location')
        if expected_rule:
            self.require(expected_rule in {x.get('rule') for x in value.get('findings', [])}, 'expected structured rule')
        return value

    def run_case(self, case_id: str, fn: Callable[[], None]) -> dict:
        self.case_id, self.branch, self.counter = case_id, 'positive', 0
        self.records, self.commands, self.evidence = [], [], []
        start = now()
        status, errors = 'passed', []
        try:
            fn()
            branches = {row['branch'] for row in self.records}
            if not {'positive', 'negative'} <= branches:
                raise AssertionError('experiment lacks independent positive and negative observations')
            if len(self.records) < 3:
                raise AssertionError('experiment has no meaningful assertion set')
        except Blocked as error:
            status, errors = 'blocked', [str(error)]
        except Exception as error:
            status, errors = 'failed', [f'{type(error).__name__}: {error}']
            target = self.output / case_id
            target.mkdir(exist_ok=True)
            (target / 'exception.log').write_text(traceback.format_exc())
        row = {'id': case_id, 'status': status, 'started_at': start, 'finished_at': now(),
               'assertion_count': len(self.records), 'observations': list(self.records),
               'commands': list(self.commands), 'evidence': list(self.evidence), 'errors': errors,
               'fixture_only': True, 'product_acceptance_promoted': False}
        dump(self.output / case_id / 'observation.json', row)
        return row


def _test_fixture(cls):
    fixture = cls()
    fixture.setUp()
    return fixture


def _close_fixture(fixture):
    try:
        fixture.tearDown()
    finally:
        fixture.doCleanups()


def scope_cli(e: Experiment):
    from test_spec_contract import ContractTests
    fixture = _test_fixture(ContractTests)
    try:
        fixture.product(); fixture.save()
        argv = [sys.executable, e.root / 'scripts/quality/spec_contract.py', '--root', fixture.root,
                '--change', fixture.change_id, '--json']
        valid = e.json_command(argv, expected_exit=0)
        e.require(not valid.get('errors'), 'approved product references are accepted')
        document = fixture.document
        delta = fixture.root / fixture.prefix / 'specs/engineering-enforcement/spec.md'
        before = delta.read_text()
        with e.phase('negative'):
            delta.write_text(before.replace('FP-03', 'FP-99'))
            document['requirements'][0]['id'] = 'FP-99'; fixture.save()
            e.json_command(argv, expected_exit=1, expected_error='FP-99')
        with e.phase('recovery'):
            delta.write_text(before); document['requirements'][0]['id'] = 'FP-03'; fixture.save()
            e.json_command(argv, expected_exit=0)
    finally:
        _close_fixture(fixture)


def frontend_cli(e: Experiment):
    with tempfile.TemporaryDirectory(prefix='luna-gv-boundary-') as folder:
        root = Path(folder); source = root / 'apps/desktop/src'
        (source / 'app').mkdir(parents=True)
        target = source / 'app/bootstrap.tsx'
        target.write_text('export const ready = true;\n')
        dump(root / 'apps/desktop/tsconfig.json', {'compilerOptions': {'strict': True, 'target': 'ES2022', 'module': 'ESNext', 'moduleResolution': 'Bundler'}, 'include': ['src']})
        argv = ['node', e.root / 'scripts/quality/frontend-boundaries.mjs', '--root', root, '--json']
        value = e.json_command(argv, expected_exit=0)
        e.expect(value['findings'], [], 'real AST accepts layered source')
        with e.phase('negative'):
            target.write_text("export const leak = fetch('https://example.invalid');\n")
            e.json_command(argv, expected_exit=1, expected_rule='FE_HTTP_ACCESS')
        with e.phase('recovery'):
            target.write_text('export const ready = true;\n')
            e.json_command(argv, expected_exit=0)
        # Independent Rust dependency path uses the same production scanner.
        (root / 'crates/app-core').mkdir(parents=True)
        (root / 'Cargo.toml').write_text('[workspace]\nmembers=["crates/app-core"]\n')
        manifest = root / 'crates/app-core/Cargo.toml'
        manifest.write_text('[package]\nname="app-core"\nversion="0.1.0"\n')
        argv = [sys.executable, e.root / 'scripts/quality/rust-boundaries.py', '--root', root, '--json']
        e.json_command(argv, expected_exit=0)
        with e.phase('negative'):
            manifest.write_text(manifest.read_text() + '[dependencies]\nreqwest="0.13"\n')
            e.json_command(argv, expected_exit=1, expected_rule='RUST-LAYER-DEPENDENCY')


def actual_scope(e: Experiment):
    from gate_core import check_scope, digest, safe_path
    with tempfile.TemporaryDirectory(prefix='luna-gv-scope-') as folder:
        root = Path(folder); source = root / 'src/main.ts'; source.parent.mkdir(); source.write_text('before')
        snapshot = 'openspec/changes/fixture/source-snapshot.json'
        dump(root / snapshot, {'files': {'src/main.ts': digest(source)}})
        metadata = {'source_snapshot': snapshot, 'allowed_paths': ['src/main.ts', 'openspec/changes/fixture/**']}
        source.write_text('after')
        e.expect(check_scope(root, metadata)[1], [], 'declared source change passes')
        e.expect(str(safe_path(root, 'src/main.ts')), str(source.resolve()), 'canonical source identity')
        with e.phase('negative'):
            other = root / 'src-escape/new.ts'; other.parent.mkdir(); other.write_text('unrequested')
            errors = check_scope(root, metadata)[1]
            e.require(any('src-escape/new.ts' in item for item in errors), 'untracked sibling-prefix escape rejected')
            outside = root.parent / f'not-owned-{uuid.uuid4().hex}'
            outside.write_text('private control')
            try:
                link = root / 'src/link.ts'; link.symlink_to(outside)
                try:
                    safe_path(root, 'src/link.ts')
                except ValueError:
                    e.require(True, 'symlink escape is explicitly rejected')
                else:
                    e.require(False, 'symlink escape is explicitly rejected')
                e.expect(outside.read_text(), 'private control', 'path test never mutates external target')
            finally:
                outside.unlink()
        with e.phase('recovery'):
            other.unlink(); link.unlink()
            e.expect(check_scope(root, metadata)[1], [], 'scope recovers after isolated violation removal')


def technology_cli(e: Experiment):
    from test_technology_contract import TechnologyContractTests
    fixture = _test_fixture(TechnologyContractTests)
    try:
        argv = [sys.executable, e.root / 'scripts/quality/technology_contract.py', '--root', fixture.root, '--json']
        e.json_command(argv, expected_exit=0)
        with e.phase('negative'):
            fixture.package['dependencies']['vue'] = '^3.5.0'; fixture.save_package()
            e.json_command(argv, expected_exit=1, expected_error='framework/state package vue has no approved_stack decision')
        with e.phase('recovery'):
            del fixture.package['dependencies']['vue']; fixture.save_package()
            e.json_command(argv, expected_exit=0)
        with e.phase('negative'):
            fixture.write('apps/desktop/next.config.mjs', 'export default {};')
            e.json_command(argv, expected_exit=1, expected_error='unregistered product manifest/config')
    finally:
        _close_fixture(fixture)


def compile_ts(e: Experiment):
    compiler = e.root / 'apps/desktop/node_modules/typescript/bin/tsc'
    if not compiler.is_file():
        raise Blocked('locked product TypeScript compiler is unavailable')
    with tempfile.TemporaryDirectory(prefix='luna-gv-compile-ts-') as folder:
        root = Path(folder); target = root / 'main.ts'
        target.write_text('export const value: number = 42;\n')
        dump(root / 'tsconfig.json', {'compilerOptions': {'strict': True, 'noEmit': True, 'types': [], 'skipLibCheck': True}, 'files': ['main.ts']})
        argv = ['node', compiler, '--project', root / 'tsconfig.json']
        e.expect(e.command(argv)['exit_code'], 0, 'real TypeScript positive compiles')
        with e.phase('negative'):
            target.write_text('export const value: number = "wrong";\n')
            result = e.command(argv)
            e.expect(result['exit_code'], 2, 'TypeScript rejects a real type mismatch')
            e.require('TS2322' in result['out'], 'compiler rejected the intended type error')
        with e.phase('recovery'):
            target.write_text('export const value: number = 42;\n')
            e.expect(e.command(argv)['exit_code'], 0, 'fixed isolated source recompiles')


def compile_rust_tests(e: Experiment):
    with tempfile.TemporaryDirectory(prefix='luna-gv-compile-rust-') as folder:
        root = Path(folder); (root / 'src').mkdir()
        (root / 'Cargo.toml').write_text('[package]\nname="gate-compile-fixture"\nversion="0.0.0"\nedition="2024"\n[workspace]\n')
        target = root / 'src/lib.rs'
        before = 'pub struct Reply { pub value: u32 }\n#[cfg(test)] mod tests { #[test] fn dto() { assert_eq!(super::Reply { value: 7 }.value, 7); } }\n'
        target.write_text(before)
        e.expect(e.command(['cargo', 'generate-lockfile', '--offline', '--manifest-path', root / 'Cargo.toml'])['exit_code'], 0, 'isolated standard-library-only lock generated offline')
        argv = ['cargo', 'test', '--manifest-path', root / 'Cargo.toml', '--no-run', '--locked', '--offline']
        e.expect(e.command(argv)['exit_code'], 0, 'actual Rust tests compile')
        with e.phase('negative'):
            # Production code still compiles; only stale tests consume removed DTO.
            target.write_text(before.replace('pub value: u32', 'pub current: u32'))
            prod = e.command(['cargo', 'check', '--manifest-path', root / 'Cargo.toml', '--locked', '--offline'])
            e.expect(prod['exit_code'], 0, 'production library remains compilable')
            result = e.command(argv)
            e.expect(result['exit_code'], 101, 'test-target compilation rejects stale DTO')
            e.require('E0560' in result['err'] or 'E0609' in result['err'], 'specific stale DTO diagnostic is present')
        with e.phase('recovery'):
            target.write_text(before)
            e.expect(e.command(argv)['exit_code'], 0, 'fixed test target compiles again')


def schema_cli(e: Experiment, relationship=False):
    from test_storage_schema import StorageSchemaTests
    fixture = _test_fixture(StorageSchemaTests)
    try:
        original = fixture.candidate()
        before = sha(original)
        argv = [sys.executable, e.root / 'scripts/quality/storage_schema.py', '--root', e.root,
                '--candidate', original, '--database', 'profile']
        e.json_command(argv, expected_exit=0)
        e.expect(sha(original), before, 'schema comparison is read-only')
        if not relationship:
            boundary_output = e.output / e.case_id / 'field-boundaries.json'
            boundary = e.json_command([sys.executable, e.root / 'scripts/quality/storage_boundaries.py',
                                       '--check', '--output', boundary_output], expected_exit=0)
            e.expect(boundary['status'], 'passed', 'registered field boundaries execute on actual SQLite projections')
            detail = json.loads(boundary_output.read_text())
            e.require(bool(detail.get('checks')) and all(row.get('status') == 'passed' and not row.get('skipped') for row in detail['checks']),
                      'NULL / type / Unicode / length controls really execute without skips')
        with e.phase('negative'):
            if relationship:
                substitutions = [('FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE RESTRICT', 'FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE CASCADE')]
                marker = 'on_delete'
            else:
                substitutions = [('display_name TEXT NOT NULL, -- 用户显示名，不作为目标身份', 'display_name BLOB NOT NULL, -- 用户显示名，不作为目标身份')]
                marker = 'columns'
            candidate = fixture.candidate(substitutions=substitutions)
            candidate_before = sha(candidate)
            bad = list(argv); bad[bad.index('--candidate') + 1] = candidate
            e.json_command(bad, expected_exit=1, expected_error=marker)
            e.expect(sha(candidate), candidate_before, 'rejected candidate is not repaired by checker')
            if relationship:
                from test_storage_schema import BAD_RELATIONSHIP
                orphan = fixture.candidate(after=BAD_RELATIONSHIP)
                orphan_argv = list(argv); orphan_argv[orphan_argv.index('--candidate') + 1] = orphan
                e.json_command(orphan_argv, expected_exit=1, expected_error='violations')
        with e.phase('recovery'):
            e.json_command(argv, expected_exit=0)
    finally:
        _close_fixture(fixture)


def recorded_unittest(e: Experiment, names: list[str], *, label: str):
    """Execute existing production guard tests; do not call this product runtime."""
    result = e.command([sys.executable, '-m', 'unittest', '-v', *names], cwd=e.root / 'scripts/quality', timeout=120)
    e.expect(result['exit_code'], 0, label)
    text = result['out'] + result['err']
    import re
    match = re.search(r'Ran (\d+) tests?', text)
    e.require(match is not None and int(match.group(1)) == len(names), 'all named actual tests executed')
    e.require('skipped=' not in text and '... skipped' not in text, 'no required test skipped')
    with e.phase('negative'):
        e.require(all(name.rsplit('.', 1)[-1] in text for name in names), 'specific mutation / refusal tests have real output')


def test_output_integrity(e: Experiment):
    from verify import validate_test_output
    good = e.command([sys.executable, '-c', 'import unittest\nclass C(unittest.TestCase):\n def test_real(self): self.assertEqual(2+2,4)\nunittest.main()'])
    e.expect(good['exit_code'], 0, 'real assertion control succeeded')
    e.expect(validate_test_output('python', good['out'] + good['err'])[1], [], 'production parser accepts actual test execution')
    with e.phase('negative'):
        for code in ['import unittest; unittest.main()', "import unittest\n@unittest.skip('isolated negative')\nclass C(unittest.TestCase):\n def test_required(self): self.fail()\nunittest.main()", 'raise SystemExit(0)']:
            result = e.command([sys.executable, '-c', code])
            e.require(bool(validate_test_output('python', result['out'] + result['err'])[1]), 'zero / skipped / early-success output is rejected independently of exit code')


def visual_fixture_inputs(root: Path):
    """Code-owned rendering closure for this isolated fixture, never production."""
    from gate_policy import snapshot_inputs
    return snapshot_inputs(root, ['ui.ts', 'design.css', 'shared.css'])


def visual_fixture(root: Path):
    """Clearly marked isolated review samples; forbidden in real qualification."""
    from gate_policy import map_identity, snapshot_inputs
    files = {'ui.ts': 'export const sample = true;\n', 'design.css': ':root{color:#223045}',
             'shared.css': '.shared-control{padding:12px}',
             'comparison.html': '<h1>ISOLATED GATE FIXTURE, NOT PRODUCT ACCEPTANCE</h1>',
             'confirmation.txt': 'Isolated validation authorizes fixture review only; no user product approval.'}
    for relative, text in files.items():
        (root / relative).write_text(text)
    (root / 'design.png').write_bytes(png(20, 12))
    (root / 'implementation.png').write_bytes(png(20, 12, (35, 49, 70)))
    def ref(relative): return {'path': relative, 'sha256': sha(root / relative)}
    inputs = visual_fixture_inputs(root)
    manifest = {'kind': 'unified-visual-report/v1', 'fixture': True,
                'report': ref('comparison.html'), 'inputs': inputs,
                'rows': [{'scope': 'reading:connected', 'design_image': ref('design.png'),
                          'implementation_image': ref('implementation.png'),
                          'viewport': {'width': 20, 'height': 12}, 'theme': 'dark',
                          'data_id': 'isolated-text', 'design_state': 'connected',
                          'implementation_state': 'connected', 'source_kind': 'real_component'}]}
    review = {'kind': 'gate-human-review/v1', 'fixture': True,
              'review_decision': 'approved', 'reviewer_id': 'isolated-reviewer',
              'reviewer_kind': 'delegated_reviewer', 'author_ids': ['isolated-author'],
              'reviewed_at': now(), 'confirmation_source': ref('confirmation.txt'),
              'scope': ['reading:connected'], 'inputs': inputs,
              'report_sha256': manifest['report']['sha256'], 'unresolved_findings': []}
    review['manifest_sha256'] = map_identity(manifest)
    return manifest, review


def visual_comparability(e: Experiment):
    from gate_policy import validate_visual
    with tempfile.TemporaryDirectory(prefix='luna-gv-visual-') as folder:
        root = Path(folder); manifest, review = visual_fixture(root)
        def validate(value):
            return validate_visual(root, value, review, required_scope=['reading:connected'],
                                   authorized_reviewers=['isolated-reviewer'],
                                   expected_inputs=visual_fixture_inputs(root), fixture=True)
        e.expect(validate(manifest)['status'], 'passed', 'complete comparable isolated report is accepted')
        e.expect(len(manifest['rows']), 1, 'actual design and candidate image pair exists')
        with e.phase('negative'):
            value = copy.deepcopy(manifest); value['rows'][0]['viewport']['height'] = 13
            e.expect(validate(value)['reason'], 'visual.size_mismatch', 'wrong viewport rejected by real image decoding')
            value = copy.deepcopy(manifest); value['rows'][0]['implementation_state'] = 'disconnected'
            e.expect(validate(value)['reason'], 'visual.state_mismatch', 'wrong actual state cannot be compared')
            value = copy.deepcopy(manifest); value['rows'][0]['implementation_image'] = value['rows'][0]['design_image']
            e.expect(validate(value)['reason'], 'visual.same_capture_file', 'same design screenshot cannot be relabelled implementation')
        with e.phase('recovery'):
            e.expect(validate(manifest)['status'], 'passed', 'correct report recovers')


def visual_review_effect(e: Experiment):
    from gate_policy import build_summary, gate_result, validate_visual
    with tempfile.TemporaryDirectory(prefix='luna-gv-visual-review-') as folder:
        root = Path(folder); manifest, review = visual_fixture(root)
        def validate(value, fixture=True):
            return validate_visual(root, manifest, value, required_scope=['reading:connected'],
                                   authorized_reviewers=['isolated-reviewer'],
                                   expected_inputs=visual_fixture_inputs(root), fixture=fixture)
        value = validate(review)
        e.expect(value['status'], 'passed', 'fixture review is accepted only in isolated verifier context')
        gates = [gate_result(f'G{i:02}', 'passed', 'isolated control') for i in range(1, 9)]
        e.expect(build_summary(gates + [value])['incomplete'], False, 'all-nine fixture aggregation control passes')
        with e.phase('negative'):
            for decision, expected_status, reason in [(None, 'blocked', 'review.missing'), ('pending', 'blocked', 'review.pending'), ('rejected', 'failed', 'review.rejected')]:
                sample = None if decision is None else {**review, 'review_decision': decision}
                result = validate(sample)
                e.expect(result['status'], expected_status, 'manual missing/pending/rejected state is honest')
                e.expect(result['reason'], reason, 'specific manual state refusal')
                e.expect(build_summary(gates + [result])['incomplete'], True, 'eight green gates cannot override G09')
            e.expect(validate(review, False)['reason'], 'visual.fixture_forbidden', 'test approval cannot enter product qualification')
            forged = {**review, 'reviewer_id': 'isolated-author'}
            e.expect(validate(forged)['reason'], 'review.unauthorized_or_author', 'implementer cannot self-approve visual review')


def visual_freshness(e: Experiment):
    from gate_policy import map_identity, validate_visual
    with tempfile.TemporaryDirectory(prefix='luna-gv-visual-stale-') as folder:
        root = Path(folder); manifest, review = visual_fixture(root)
        def validate(value=manifest, expected=['reading:connected'], selected_review=review):
            return validate_visual(root, value, selected_review, required_scope=expected,
                                   authorized_reviewers=['isolated-reviewer'],
                                   expected_inputs=visual_fixture_inputs(root), fixture=True)
        e.expect(validate()['status'], 'passed', 'unchanged reviewed fixture passes')
        (root / 'unrelated-note.md').write_text('No UI dependency')
        e.expect(validate()['status'], 'passed', 'unrelated note does not invalidate explicit rendering input closure')
        with e.phase('negative'):
            narrowed = copy.deepcopy(manifest)
            del narrowed['inputs']['shared.css']
            forged_review = {**review, 'inputs': narrowed['inputs'],
                             'manifest_sha256': map_identity(narrowed)}
            e.expect(validate(narrowed, selected_review=forged_review)['reason'], 'visual.stale_inputs',
                     'report and matching review cannot self-select a smaller rendering dependency closure')
            shared = root / 'shared.css'; original_shared = shared.read_text()
            shared.write_text(original_shared + '\n.shared-control{padding:99px}')
            e.expect(validate(narrowed, selected_review=forged_review)['reason'], 'visual.stale_inputs',
                     'omitted shared dependency change cannot preserve an old visual approval')
            shared.write_text(original_shared)
            e.expect(validate()['status'], 'passed', 'full independent rendering closure recovers')
            e.expect(validate(expected=['reading:connected', 'reading:error'])['reason'], 'visual.coverage_missing', 'one approved state cannot close another')
            original = (root / 'ui.ts').read_text(); (root / 'ui.ts').write_text(original + '// shared rendering change')
            e.expect(validate()['reason'], 'visual.stale_inputs', 'actual UI input mutation invalidates approval')
            (root / 'ui.ts').write_text(original)
            original = (root / 'comparison.html').read_text(); (root / 'comparison.html').write_text(original + '<p>changed</p>')
            e.expect(validate()['status'], 'failed', 'actual report mutation invalidates recorded report identity')
            (root / 'comparison.html').write_text(original)
            (root / 'implementation.png').write_bytes(png(20, 12, (99, 88, 77)))
            e.expect(validate()['status'], 'failed', 'screenshot replacement invalidates reviewed candidate')


def make_receipt(e: Experiment, root: Path):
    from gate_policy import snapshot_inputs
    target = root / 'check.py'
    target.write_text('import unittest\nclass Control(unittest.TestCase):\n def test_assertion(self): self.assertEqual(7,7)\nunittest.main()\n')
    command = [sys.executable, str(target)]
    result = e.command(command, cwd=root)
    for stream in ('stdout', 'stderr'):
        shutil.copyfile(result[stream]['path'], root / stream)
    return {'schema_version': 1, 'kind': 'gate-command-receipt/v1', 'fixture': True,
            'run_id': uuid.uuid4().hex, 'check_id': 'isolated-tests', 'gate_id': 'G05',
            'change_id': 'isolated-gate-experiment', 'command': command,
            'started_at': result['started_at'], 'finished_at': result['finished_at'],
            'exit_code': result['exit_code'], 'inputs': snapshot_inputs(root, ['check.py']),
            'stdout': {'path': 'stdout', 'sha256': sha(root / 'stdout')},
            'stderr': {'path': 'stderr', 'sha256': sha(root / 'stderr')},
            'check_kind': 'tests', 'test_count': 1, 'skipped_count': 0}


def receipt_freshness(e: Experiment):
    from gate_policy import validate_receipt
    with tempfile.TemporaryDirectory(prefix='luna-gv-receipt-') as folder:
        root = Path(folder); receipt = make_receipt(e, root)
        def validate(value=receipt):
            return validate_receipt(root, value, expected_command=receipt['command'],
                                    change_id=receipt['change_id'], fixture=True)
        e.expect(validate()['status'], 'passed', 'real command receipt and raw files are current')
        e.expect(validate()['reason'], 'receipt.executed_current', 'actual registered command is accepted')
        with e.phase('negative'):
            source = root / 'check.py'; before = source.read_text(); source.write_text(before + '# relevant source changed')
            e.expect(validate()['reason'], 'receipt.stale_inputs', 'real source mutation expires old execution')
            source.write_text(before)
            sample = {**receipt, 'change_id': 'another-work-item'}
            e.expect(validate(sample)['reason'], 'receipt.wrong_change', 'other work-item result cannot be borrowed')
            (root / 'stderr').write_text('replaced old result')
            e.expect(validate()['status'], 'failed', 'raw output corruption is rejected')


def concurrent_evidence(e: Experiment):
    from gate_policy import atomic_write_json, locked_state
    with tempfile.TemporaryDirectory(prefix='luna-gv-concurrent-') as folder:
        root = Path(folder)
        atomic_write_json(root, 'one/result.json', {'run_id': 'first', 'counter': 1})
        atomic_write_json(root, 'two/result.json', {'run_id': 'second', 'counter': 2})
        e.expect(json.loads((root / 'one/result.json').read_text())['run_id'], 'first', 'first run has separate evidence path')
        e.expect(json.loads((root / 'two/result.json').read_text())['run_id'], 'second', 'second run has separate evidence path')
        with e.phase('negative'):
            target = root / 'target'; target.mkdir(); (root / 'escape').symlink_to(target, target_is_directory=True)
            try:
                atomic_write_json(root, 'escape/result.json', {'run_id': 'pollution'})
            except ValueError:
                e.require(True, 'atomic writer rejects symbolic evidence location')
            else:
                e.require(False, 'atomic writer rejects symbolic evidence location')
            e.expect((target / 'result.json').exists(), False, 'rejected location has no write side effect')
        # Lock serialization is exercised by separate OS processes, not two
        # in-memory calls that accidentally share a lock owner.
        helper = "import sys,time;from pathlib import Path;sys.path.insert(0,sys.argv[1]);from gate_policy import locked_state,atomic_write_json; r=Path(sys.argv[2]);\nwith locked_state(r,'counter.lock'):\n p=r/'counter.json';v=__import__('json').loads(p.read_text()) if p.exists() else {'counter':0};time.sleep(.05);v['counter']+=1;atomic_write_json(r,'counter.json',v)\n"
        argv = [sys.executable, '-c', helper, str(e.root / 'scripts/quality'), str(root)]
        began = now()
        processes = [subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(3)]
        for index, process in enumerate(processes):
            out, err = process.communicate(timeout=10)
            raw = {}
            for stream, text in (('stdout', out), ('stderr', err)):
                target = e.output / e.case_id / f'concurrent-{index}.{stream}'
                target.parent.mkdir(exist_ok=True); target.write_text(text)
                raw[stream] = {'path': str(target), 'sha256': sha(target)}
            e.commands.append({'command': argv, 'cwd': str(e.root), 'started_at': began,
                               'finished_at': now(), 'exit_code': process.returncode, **raw})
            e.expect(process.returncode, 0, 'independent process obeys evidence lock: ' + err)
        e.expect(json.loads((root / 'counter.json').read_text())['counter'], 3, 'concurrent locked updates retain all operations')


def impact_dispatch(e: Experiment):
    from gate_policy import classify_impact
    docs = classify_impact(['docs/development/explanation.md'])
    e.expect('G09' in docs['required_gates'], False, 'independent documentation does not trigger visual acceptance')
    ui = classify_impact(['apps/desktop/src/ui/primitives/Select.tsx'])
    e.require({'G02', 'G05', 'G06', 'G09'} <= set(ui['required_gates']), 'shared UI requires functional and visual verification')
    with e.phase('negative'):
        storage = classify_impact(['crates/app-infra/src/storage/migrations/005.sql'])
        e.require({'G06', 'G07', 'G09'} <= set(storage['required_gates']), 'migration cannot be downgraded to single-page test')
        unknown = classify_impact(['unknown-production/capability.rs'])
        e.require(unknown['unknown'] and set(unknown['required_gates']) >= {'G07', 'G08', 'G09'}, 'unknown production impact is conservative')


def qualifier_fixture(e: Experiment):
    from product_completion import merge_binding_readiness
    from test_product_completion import ProductCompletionTests
    fixture = ProductCompletionTests()
    assessment = fixture.binding_group_fixture()
    def verdict(value):
        return merge_binding_readiness({'status': 'passed', 'errors': [], 'release_gaps': []},
                {'status': 'passed', 'errors': [], 'release_gaps': [], 'execution_obligations': value})
    e.expect(verdict(assessment)['status'], 'passed', 'fixture-only complete group aggregation works')
    e.expect(assessment['product_acceptance_promoted'], False, 'fixture explicitly does not promote product')
    with e.phase('negative'):
        for mutate in ['missing', 'duplicate', 'not_run', 'promoted']:
            value = copy.deepcopy(assessment)
            if mutate == 'missing': value['groups'].pop()
            elif mutate == 'duplicate': value['groups'][1]['id'] = value['groups'][0]['id']
            elif mutate == 'not_run': value['groups'][0]['status'] = 'not_run'
            else: value['product_acceptance_promoted'] = True
            e.expect(verdict(value)['status'], 'blocked', 'production aggregation rejects ' + mutate)


def rejecting_mutation(e: Experiment):
    """Patch only an isolated checker copy; external oracle must turn red."""
    from test_technology_contract import TechnologyContractTests
    fixture = _test_fixture(TechnologyContractTests)
    try:
        fixture.package['dependencies']['vue'] = '^3.5.0'; fixture.save_package()
        argv = [sys.executable, e.root / 'scripts/quality/technology_contract.py', '--root', fixture.root, '--json']
        with e.phase('positive'):
            e.json_command(argv, expected_exit=1, expected_error='framework/state package vue has no approved_stack decision')
        with e.phase('negative'):
            with tempfile.TemporaryDirectory(prefix='luna-gv-mutant-') as folder:
                mutant = Path(folder) / 'technology_contract.py'
                source = (e.root / 'scripts/quality/technology_contract.py').read_text()
                # Mutate the actual production CLI's result dispatch, not a
                # replacement checker. This changes only the isolated copy.
                needle = 'errors = check(args.root)'
                if needle not in source:
                    raise AssertionError('actual checker mutation anchor changed; review experiment')
                mutant.write_text(source.replace(needle, 'errors = []  # isolated deliberate mutant', 1))
                result = e.command([sys.executable, mutant, '--root', fixture.root, '--json'])
                e.expect(result['exit_code'], 0, 'deliberate isolated mutant incorrectly permits invalid framework')
                observed = json.loads(result['out'])
                e.expect('framework/state package vue has no approved_stack decision' in json.dumps(observed), False, 'external expected-refusal oracle detects missing diagnostic')
                e.expect(result['exit_code'] == 1 and bool(observed.get('errors')), False, 'original experiment would fail against broken gate')
    finally:
        _close_fixture(fixture)


def missing_dispatch_mutation(e: Experiment):
    from gate_policy import validate_receipt
    from verify import validate_test_output
    with tempfile.TemporaryDirectory(prefix='luna-gv-dispatch-mutant-') as folder:
        root = Path(folder); receipt = make_receipt(e, root)
        result = validate_receipt(root, receipt, expected_command=receipt['command'], fixture=True)
        e.expect(result['status'], 'passed', 'registered actual test execution is accepted')
        e.expect(receipt['test_count'], 1, 'positive control has one required executed test')
        with e.phase('negative'):
            # Run an actual empty suite whose process returns success; do not
            # rely on version-dependent unittest.main() empty-suite exit codes.
            zero = e.command([sys.executable, '-c', 'import unittest;unittest.TextTestRunner().run(unittest.TestSuite())'])
            e.expect(zero['exit_code'], 0, 'actual zero-assertion process returns success')
            for stream in ('stdout', 'stderr'):
                shutil.copyfile(zero[stream]['path'], root / stream)
            mutant = {**receipt, 'stdout': {'path': 'stdout', 'sha256': sha(root / 'stdout')},
                      'stderr': {'path': 'stderr', 'sha256': sha(root / 'stderr')},
                      'exit_code': 0, 'test_count': 1}
            # Deliberate wrong claimed count is independently checked against
            # actual raw output. The policy must reject even an exit-0 claim.
            refused = validate_receipt(root, mutant, expected_command=receipt['command'], fixture=True)
            e.expect(refused['status'], 'failed', 'hidden missing dispatch cannot retain an old passing count')
            e.require('test' in refused['reason'], 'failure specifically identifies missing / unobserved tests')
            e.require(bool(validate_test_output('python', zero['out'] + zero['err'])[1]), 'existing root parser independently catches zero-test mutant')
            downgraded = {**mutant, 'check_kind': 'command', 'test_count': 0,
                          'command': zero['command']}
            refused = validate_receipt(root, downgraded, expected_command=zero['command'], fixture=True)
            e.expect(refused['status'], 'failed', 'zero-assertion test cannot be relabelled as a successful command')
            e.expect(refused['reason'], 'receipt.check_kind_mismatch',
                     'code-owned required test kind rejects tests-to-command downgrade')


PRODUCTION_CHECK_IDS = {
    'functional-component': 'functional-component',
    'functional-timing': 'affected-functional-tests',
    'storage-migration': 'storage-migration',
    'storage-recovery': 'storage-runtime',
}


def _external_document(root: Path, external):
    from gate_policy import read_json
    if external is None:
        return {}
    path = Path(external)
    if path.is_absolute():
        try:
            # Preserve the lexical path so owned() can reject a symlink in
            # the supplied evidence path rather than silently resolving it.
            relative = path.absolute().relative_to(root.resolve()).as_posix()
        except ValueError as error:
            raise ValueError('external observation index must be inside the project evidence directory') from error
    else:
        relative = path.as_posix()
    value = read_json(owned(root, relative))
    if not isinstance(value, dict) or type(value.get('schema_version')) is not int or value['schema_version'] != 1:
        raise ValueError('external execution index schema invalid')
    return value


def _raw_reference(root: Path, reference) -> Path:
    if not isinstance(reference, dict) or not isinstance(reference.get('path'), str):
        raise AssertionError('external observer raw reference missing')
    path = owned(root, reference['path'])
    if not path.is_file() or sha(path) != reference.get('sha256'):
        raise AssertionError('external observer raw evidence missing or changed: ' + str(reference['path']))
    return path


def production_evidence(e: Experiment, key: str, *, markers=()):
    """Consume real production test evidence, not its hand-written status."""
    from gate_policy import load_context, read_json, snapshot_inputs, validate_receipt
    from development_gates import command_registry
    items = getattr(e, 'external', {}).get('items', {})
    reference = items.get(key)
    if reference is None:
        raise Blocked('actual production test evidence unavailable: ' + key)
    receipt = read_json(owned(e.root, reference)) if isinstance(reference, str) else reference
    if isinstance(reference, str):
        e.evidence.append({'path': reference, 'sha256': sha(owned(e.root, reference))})
    check_id = PRODUCTION_CHECK_IDS.get(key)
    if check_id is None:
        raise AssertionError('external command has no code-owned expected registration')
    e.expect(receipt.get('check_id'), check_id, 'external observation belongs to registered production check')
    context = load_context(e.root)
    change_id = context['change_id']
    # Only the execution process chooses the source closure. A receipt must
    # not narrow it to keep an old result alive after omitted source changes.
    # run() owns this before-map and rejects any after-map drift separately;
    # direct consumers take a fresh complete snapshot here.
    current_inputs = getattr(e, '_execution_inputs', None)
    if current_inputs is None:
        current_inputs = snapshot_inputs(e.root)
    expected = command_registry(e.root, change_id, receipt['run_id'])[check_id]
    result = validate_receipt(e.root, receipt, expected_command=expected,
                              expected_inputs=current_inputs, current_inputs=current_inputs,
                              change_id=change_id)
    e.expect(result['status'], 'passed', 'actual current production receipt is valid: ' + result['reason'])
    e.require(receipt.get('fixture') is not True and receipt.get('gate_fixture') is not True,
              'production execution is not a gate fixture')
    raw = ''.join(_raw_reference(e.root, receipt[stream]).read_text() for stream in ('stdout', 'stderr'))
    e.evidence.extend(receipt[stream] for stream in ('stdout', 'stderr'))
    if receipt.get('test_observation'):
        e.evidence.append(receipt['test_observation'])
    e.require(bool(raw.strip()), 'production raw output is nonempty')
    for marker in markers:
        e.require(marker in raw, 'specific production behavior has actual test output: ' + marker)
    with e.phase('negative'):
        # The test process already exercised the production rejection paths.
        # Here also demonstrate that the consumer cannot accept a green label
        # with no raw output / wrong command.
        missing = copy.deepcopy(receipt); missing['stdout'] = {'path': 'missing-evidence.log', 'sha256': '0' * 64}
        refused = validate_receipt(e.root, missing, expected_command=expected,
                                   expected_inputs=current_inputs, current_inputs=current_inputs,
                                   change_id=change_id)
        e.expect(refused['status'], 'failed', 'claimed production success without actual raw output is rejected')
    return receipt


def component_behavior(e: Experiment, *, semantics=False):
    from gate_policy import read_json
    receipt = production_evidence(e, 'functional-component')
    result_path = _raw_reference(e.root, receipt.get('test_observation'))
    if not result_path.is_file():
        raise Blocked('real component observer result is missing')
    result = read_json(result_path)
    e.expect(result.get('kind'), 'binding-ui-controlled-component-check/v1', 'real React observer identity')
    e.expect(result.get('status'), 'passed', 'actual production component checks completed')
    checks = {row['id']: row for row in result['checks']}
    positive = 'actual-tauri-port-boundary' if not semantics else 'multi-choice-is-draft-only'
    negative = 'failed-submit-keeps-same-draft-and-no-effects' if not semantics else 'promise-is-not-handshake'
    e.expect(checks.get(positive, {}).get('status'), 'passed', 'specific real component positive control')
    with e.phase('negative'):
        e.expect(checks.get(negative, {}).get('status'), 'passed', 'actual component rejection semantics executed')
        e.require('controlled transport' in result.get('scope', '').lower(), 'controlled component test cannot become game / native acceptance')


def timing_behavior(e: Experiment):
    production_evidence(e, 'functional-timing', markers=('bj_06_domain', 'bj_07_domain'))
    with e.phase('negative'):
        production_evidence(e, 'storage-recovery', markers=('cancellation_wins_and_late_result_cannot_enter_history',))


HOOK_PROTOCOL_TESTS = {
    'GV-17': ['test_session_loads_scope_without_compiling', 'test_project_root_resolves_from_nested_cwd'],
    'GV-18': ['test_known_allowed_write_runs_and_is_observed', 'test_known_outside_write_denied_before_effect'],
    'GV-19': ['test_known_allowed_write_runs_and_is_observed', 'test_post_violation_reports_real_change_and_does_not_roll_back'],
    'GV-20': ['test_readonly_stop_does_not_assess_historical_gates', 'test_later_readonly_question_may_end_while_work_stays_incomplete'],
    'GV-21': ['test_valid_verification_clears_dirty_state', 'test_missing_checks_continue_at_most_twice_per_failure'],
    'GV-22': ['test_missing_checks_continue_at_most_twice_per_failure', 'test_external_manual_blocker_does_not_loop'],
    'GV-23': ['test_no_context_does_not_grant_unknown_write', 'test_cli_invalid_json_is_documented_blocking_exit'],
    'GV-30': ['test_unpaired_result_does_not_claim_success', 'test_journal_does_not_include_raw_command_or_tool_response'],
}


def _jsonl(path: Path) -> list[dict]:
    if path.stat().st_size > 64 * 1024 * 1024:
        raise AssertionError('hook raw event stream exceeds bounded observation size')
    result = []
    for line in path.read_text().splitlines():
        if not line.strip(): continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError as error:
            raise AssertionError('hook event evidence is not actual JSONL') from error
        if not isinstance(item, dict):
            raise AssertionError('hook raw event must be a structured object')
        result.append(item)
    return result


def derive_hook_observations(root: Path, row: dict) -> dict:
    """Recompute L3 facts from raw client/journal/file observations.

    This does not create an observation or grant hook trust. The separate
    source-bound live observer owns actual Codex processes and temporary files.
    Its raw streams are consumed independently instead of trusting green labels.
    """
    from gate_policy import read_json
    refs = row.get('evidence')
    if not isinstance(refs, list) or not refs:
        raise AssertionError('actual hook case has no raw evidence')
    files = {}
    for reference in refs:
        path = _raw_reference(root, reference)
        name = path.name
        if name in files:
            raise AssertionError('duplicate hook raw evidence name')
        files[name] = path
    required = {'actual-hook-events.jsonl', 'external-file-observations.json',
                'readonly.stdout.jsonl', 'write-probes.stdout.jsonl'}
    if not required <= files.keys():
        raise AssertionError('hook proof missing actual journal/client/file raw observations')
    journal = _jsonl(files['actual-hook-events.jsonl'])
    names = ['readonly', 'write-probes']
    if 'hooks-disabled.stdout.jsonl' in files:
        names.append('hooks-disabled')
    clients = {key: _jsonl(files[key + '.stdout.jsonl']) for key in names}
    sessions = {}
    commands = {}
    for name, rows in clients.items():
        starts = [item.get('thread_id') for item in rows if item.get('type') == 'thread.started']
        if len(starts) != 1 or not isinstance(starts[0], str) or not starts[0]:
            raise AssertionError('actual Codex stream has no unique thread identity')
        sessions[name] = starts[0]
        if not any(item.get('type') == 'turn.completed' for item in rows):
            raise AssertionError('actual Codex stream has not completed its turn')
        commands[name] = [item['item'] for item in rows if item.get('type') == 'item.completed'
                          and isinstance(item.get('item'), dict)
                          and item['item'].get('type') == 'command_execution']
    if len(set(sessions.values())) != len(sessions):
        raise AssertionError('independent real Codex controls must have distinct sessions')
    if not journal:
        raise AssertionError('actual hook journal is empty')
    for event in journal:
        if (event.get('session_id') not in set(sessions.values())
                or event.get('event_name') not in {'SessionStart', 'PreToolUse', 'PostToolUse', 'Stop'}
                or not isinstance(event.get('decision'), str)):
            raise AssertionError('raw hook journal has unrelated session/event identity')
    claimed = row.get('observations')
    case_id = row.get('case_id')
    if case_id not in HOOK_CASES:
        raise AssertionError('unknown actual hook scenario identity')
    scope_name = 'readonly' if case_id == 'GV-20' else 'hooks-disabled' if case_id == 'GV-23' else 'write-probes'
    expected_session = sessions.get(scope_name)
    if not isinstance(claimed, dict) or claimed.get('scope_session_id') != expected_session or expected_session is None:
        raise AssertionError('hook case scope does not match actual client session')
    projection = [{key: event.get(key) for key in ('event_name', 'session_id', 'tool_use_id', 'decision')}
                  for event in journal if event['session_id'] == expected_session]
    if claimed.get('events') != projection:
        raise AssertionError('hook observations.events differ from raw journal')
    file_observations = read_json(files['external-file-observations.json'])
    if (not isinstance(file_observations, dict)
            or not isinstance(file_observations.get('before'), dict)
            or not isinstance(file_observations.get('after'), dict)
            or claimed.get('files') != file_observations):
        raise AssertionError('hook file observations differ from external observer raw artifact')
    before, after = file_observations['before'], file_observations['after']
    expected_names = {'allowed/positive.txt', 'denied.txt', 'post-only.txt'}
    if not expected_names <= before.keys() or not expected_names <= after.keys():
        raise AssertionError('hook file observer omitted required test targets')
    if any(before[name] is not None for name in expected_names):
        raise AssertionError('live write controls were not fresh isolated targets')
    readonly_events = [event for event in journal if event['session_id'] == sessions['readonly']]
    action_events = [event for event in journal if event['session_id'] == sessions['write-probes']]
    continued = [event for event in action_events if event['event_name'] == 'Stop' and event['decision'] == 'continue_verification']
    per_fingerprint = {}
    for event in continued:
        fingerprint = event.get('observations', {}).get('fingerprint')
        if not isinstance(fingerprint, str) or not fingerprint:
            raise AssertionError('raw continuation omits failure fingerprint')
        per_fingerprint[fingerprint] = per_fingerprint.get(fingerprint, 0) + 1
    max_continuations = max(per_fingerprint.values(), default=0)
    expected_continuations = 0 if case_id in {'GV-20', 'GV-23'} else max_continuations
    if claimed.get('continuations') != expected_continuations:
        raise AssertionError('claimed hook continuation count differs from real Stop events')
    if 'continuations_total' in claimed and claimed['continuations_total'] != len(continued):
        raise AssertionError('claimed total hook continuation count differs from raw Stop events')
    denied_absent = after['denied.txt'] is None
    retained = after['post-only.txt'] == hashlib.sha256(b'observed side effect').hexdigest()
    positive = after['allowed/positive.txt'] == hashlib.sha256(b'').hexdigest()
    if 'positive.txt' in files and sha(files['positive.txt']) != after['allowed/positive.txt']:
        raise AssertionError('preserved actual positive file does not match observer hash')
    if 'post-only.txt' in files and sha(files['post-only.txt']) != after['post-only.txt']:
        raise AssertionError('preserved actual post-write file does not match observer hash')
    if claimed.get('denied_target_absent') is not denied_absent or claimed.get('write_retained') is not retained:
        raise AssertionError('claimed hook write effect differs from observed file identities')
    denied = [event for event in action_events if event['event_name'] == 'PreToolUse' and event['decision'] == 'deny'
              and 'hook.scope_violation' in event.get('observations', {}).get('reason', '')]
    for event in action_events:
        if event['event_name'] == 'PostToolUse':
            if not any(pre['event_name'] == 'PreToolUse' and pre['tool_use_id'] == event.get('tool_use_id')
                       and pre['decision'] == 'allow' for pre in action_events):
                raise AssertionError('actual post-tool event has no matching allowed pre-tool identity')
    post_violation = [event for event in action_events if event['event_name'] == 'PostToolUse'
                      and event['decision'] == 'violation'
                      and 'post-only.txt' in event.get('observations', {}).get('outside_scope', [])]
    limits = [event for event in action_events if event['event_name'] == 'Stop' and event['decision'] == 'continuation_limit']
    host_positive = any('touch allowed/positive.txt' in event.get('command', '')
                        and event.get('exit_code') == 0 for event in commands['write-probes'])
    host_post = any('post-only.txt' in event.get('command', '') and event.get('exit_code') == 0
                   for event in commands['write-probes'])
    outcome = {
        'GV-17': all(any(event['event_name'] == 'SessionStart' and event['decision'] == 'context_loaded'
                         for event in events) for events in (readonly_events, action_events)),
        'GV-18': positive and host_positive and denied_absent and bool(denied),
        'GV-19': retained and host_post and bool(post_violation),
        'GV-20': not commands['readonly'] and any(event['event_name'] == 'Stop' and event['decision'] == 'readonly_stop' for event in readonly_events),
        'GV-21': bool(continued),
        'GV-22': bool(limits) and 1 <= max_continuations <= 2,
        'GV-30': bool(commands['write-probes']) and bool(post_violation) and bool(denied),
    }
    # Disabled-hooks is an actual runtime control. The absence of journal
    # records is meaningful only with a complete distinct client invocation.
    if 'hooks-disabled' in sessions and 'disabled-independent-assessment.json' in files:
        disabled_session = sessions['hooks-disabled']
        no_events = not any(event.get('session_id') == disabled_session for event in journal)
        independent = read_json(files['disabled-independent-assessment.json'])
        gates = independent.get('gates', [])
        scope = next((item for item in gates if item.get('gate_id') == 'G03'), {})
        refusal = (independent.get('kind') == 'development-gates-summary/v1'
                   and independent.get('incomplete') is True
                   and independent.get('next_stage_allowed') is False
                   and scope.get('status') == 'failed'
                   and scope.get('reason') == 'scope.actual_diff_rejected'
                   and 'post-only.txt' in json.dumps(scope.get('details', [])))
        if case_id == 'GV-23':
            journal_size = files['actual-hook-events.jsonl'].stat().st_size
            if (claimed.get('disabled_hook_event_bytes_before') != journal_size
                    or claimed.get('disabled_hook_event_bytes_after') != journal_size):
                raise AssertionError('claimed disabled hook interval differs from actual journal size')
            if claimed.get('disabled_independent_status') != independent.get('status') or claimed.get('disabled_independent_rule') != scope.get('reason'):
                raise AssertionError('claimed independent rejection differs from actual assessment raw')
        outcome['GV-23'] = no_events and refusal and bool(commands['hooks-disabled'])
    return {'case_results': outcome, 'events': projection, 'sessions': sessions,
            'files': file_observations, 'continuations': expected_continuations,
            'continuations_total': len(continued),
            'denied_target_absent': denied_absent, 'write_retained': retained}


def validate_hook_source_identity(root: Path, proof: dict) -> dict:
    """Compare relative source identities across checkouts without rebasing hashes."""
    from gate_policy import snapshot_inputs
    inputs = proof.get('source_inputs')
    if not isinstance(inputs, dict) or not HOOK_REQUIRED_SOURCES <= inputs.keys():
        raise AssertionError('real hook observation binds actual adapter / observer / policy sources')
    for name in HOOK_REQUIRED_SOURCES:
        value = inputs[name]
        if (not isinstance(value, str) or len(value) != 64
                or any(character not in '0123456789abcdef' for character in value)):
            raise AssertionError('actual hook source is missing a concrete file hash: ' + name)
    current = snapshot_inputs(root, inputs)
    if current != inputs:
        raise AssertionError('actual hook execution source is still current: input hash mismatch')
    if proof.get('source_inputs_before') != inputs or proof.get('source_stable') is not True:
        raise AssertionError('actual hook execution source changed during observation')
    return current


def validate_hook_configuration(root: Path, proof: dict, row: dict) -> None:
    """Bind observed handlers to the complete current project definition.

    A fixed portable command may materialize the recorded execution source root
    and substitute its disposable --root only. Matchers, command executables,
    relative handler path, timeouts and every other definition remain exact.
    """
    from codex_hooks import materialize_hook_command
    from gate_policy import read_json
    command = proof.get('command', [])
    execution_source_root = proof.get('execution_source_root')
    probe_root = row.get('observations', {}).get('probe_root')
    if (not isinstance(probe_root, str) or not Path(probe_root).is_absolute()
            or '\x00' in probe_root or '..' in Path(probe_root).parts
            or Path(probe_root).resolve().is_relative_to(root.resolve())
            or command.count('-C') != 1):
        raise AssertionError('actual hook configuration has no isolated observer root')
    # The shared helper accepts only the fixed project command and a canonical
    # absolute source root. It never evaluates a shell expression from proof.
    try:
        source_root = Path(execution_source_root)
        if not source_root.is_absolute() or '..' in source_root.parts:
            raise ValueError('noncanonical execution source root')
        if Path(probe_root).resolve().is_relative_to(source_root.resolve()):
            raise ValueError('probe root is inside execution source')
    except (TypeError, ValueError) as error:
        raise AssertionError('actual hook configuration has no valid execution source root') from error
    index = command.index('-C') + 1
    if index >= len(command) or command[index] != probe_root:
        raise AssertionError('actual Codex root differs from isolated hook definition root')
    original = read_json(owned(root, '.codex/hooks.json'))
    expected = copy.deepcopy(original)
    if set(expected.get('hooks', {})) != {'SessionStart', 'PreToolUse', 'PostToolUse', 'Stop'}:
        raise AssertionError('actual hook experiment must cover the complete project lifecycle definition')
    for groups in expected['hooks'].values():
        for group in groups:
            for handler in group['hooks']:
                if handler.get('type') != 'command':
                    raise AssertionError('actual hook experiment cannot silently omit a non-command handler')
                try:
                    argv = materialize_hook_command(handler['command'], execution_source_root)
                except (TypeError, ValueError) as error:
                    raise AssertionError('configured hook command does not match the fixed portable handler') from error
                root_index = argv.index('--root') + 1
                argv[root_index] = probe_root
                handler['command'] = shlex.join(argv)
    references = [ref for ref in row.get('evidence', [])
                  if Path(ref.get('path', '')).name == 'isolated-hook-definition.json']
    if len(references) != 1:
        raise AssertionError('actual hook proof missing one unambiguous isolated configuration original')
    observed = read_json(_raw_reference(root, references[0]))
    if observed != expected:
        raise AssertionError('actual hook configuration differs beyond fixed source-root materialization and isolated --root')


def actual_hook_evidence(e: Experiment, case_id):
    from gate_policy import read_json, snapshot_inputs
    recorded_unittest(e, ['test_codex_hooks.HookTests.' + name for name in HOOK_PROTOCOL_TESTS[case_id]],
                      label='actual adapter protocol tests passed; this is L1, not L3')
    reference = getattr(e, 'external', {}).get('hooks')
    if reference is None:
        raise Blocked('actual Codex L3 observation not provided; simulated adapter protocol is only L1')
    proof = read_json(owned(e.root, reference)) if isinstance(reference, str) else reference
    if (not isinstance(proof, dict) or proof.get('scope') != 'actual_codex_hook_execution'
            or proof.get('kind') != 'codex-hook-live-verification/v1'
            or type(proof.get('schema_version')) is not int or proof['schema_version'] != 1):
        raise AssertionError('simulated hook input cannot count as actual Codex execution')
    e.expect(proof.get('product_acceptance_promoted'), False, 'live hook validation does not promote product acceptance')
    command = proof.get('command')
    e.require(isinstance(command, list) and len(command) > 1
              and Path(command[0]).name == 'codex' and any(x in command for x in ('exec', 'app-server')),
              'external observation was launched through actual Codex, not a direct hook script')
    e.expect(proof.get('exit_code'), 0, 'actual Codex observer command completed')
    inputs = proof.get('source_inputs')
    e.require(isinstance(inputs, dict) and HOOK_REQUIRED_SOURCES <= inputs.keys(),
              'real hook observation binds actual adapter / observer / policy sources')
    e.expect(validate_hook_source_identity(e.root, proof), inputs, 'actual hook execution source is still current')
    e.expect(proof.get('source_inputs_before'), inputs, 'live observation did not change source during execution')
    e.expect(proof.get('source_stable'), True, 'external observer captured a stable source interval')
    cases = proof.get('cases')
    e.require(isinstance(cases, list), 'actual hook producer supplied case observations')
    matches = [row for row in cases if row.get('case_id') == case_id]
    e.expect(len(matches), 1, 'one unambiguous actual hook case observation')
    row = matches[0]
    e.expect(row.get('status'), 'passed', 'actual Codex observer verified this case')
    validate_hook_configuration(e.root, proof, row)
    derived = derive_hook_observations(e.root, row)
    e.evidence.extend(row['evidence'])
    observations = row['observations']
    events = derived['events']
    e.expect(derived['case_results'].get(case_id), True, 'raw Codex / journal / file observations independently establish this case')
    with e.phase('negative'):
        e.expect(row.get('expected_verification_result'), 'passed', 'external hook observer matched the designed counterexample')
        if case_id == 'GV-18':
            e.require(any(item.get('event_name') == 'PreToolUse' and item.get('decision') in {'deny', 'scope_denied', 'hook.scope_violation'} for item in events), 'actual event denied intended write')
            e.require(observations.get('denied_target_absent') is True, 'external file observation confirms denied write did not happen')
        elif case_id == 'GV-19':
            e.require(observations.get('write_retained') is True, 'PostToolUse feedback did not pretend to undo completed write')
        elif case_id in {'GV-20', 'GV-22'}:
            e.require(type(observations.get('continuations')) is int and observations['continuations'] <= 2, 'actual continuation is bounded')


def run(root: Path, output: Path, *, selected=None, external=None) -> dict:
    from gate_policy import map_identity, snapshot_inputs
    root = Path(root).resolve()
    wanted = list(selected) if selected else list(CASE_IDS)
    if len(wanted) != len(set(wanted)) or set(wanted) - set(CASE_IDS):
        raise ValueError('unknown or duplicate effectiveness scenario')
    experiment = Experiment(root, output)
    experiment.external = _external_document(root, external)
    before = snapshot_inputs(root)
    experiment._execution_inputs = before
    handlers = {
        'GV-01': scope_cli, 'GV-02': lambda e: component_behavior(e, semantics=True),
        'GV-03': visual_comparability,
        'GV-04': frontend_cli, 'GV-05': actual_scope,
        'GV-06': technology_cli,
        'GV-07': lambda e: recorded_unittest(e, [
            'test_package_identity.PackageIdentityTests.test_valid_shape_requires_actual_signature_verifier',
            'test_package_identity.PackageIdentityTests.test_wrong_executable_architecture_is_rejected',
            'test_package_identity.PackageIdentityTests.test_wrong_resource_version_and_unrelated_app_are_rejected',
        ], label='actual package inspector architecture/resource controls'),
        'GV-08': compile_ts, 'GV-09': compile_rust_tests,
        'GV-10': component_behavior,
        'GV-11': timing_behavior,
        'GV-12': lambda e: schema_cli(e), 'GV-13': lambda e: schema_cli(e, True),
        'GV-14': lambda e: production_evidence(e, 'storage-migration', markers=('actual_migration_host_sql_failure_after_ddl_rolls_back_and_then_recovers',)),
        'GV-15': lambda e: production_evidence(e, 'storage-recovery', markers=('backup_is_consistent_and_never_overwrites_existing',)),
        'GV-16': qualifier_fixture, 'GV-24': receipt_freshness,
        'GV-25': test_output_integrity, 'GV-26': concurrent_evidence,
        'GV-27': impact_dispatch, 'GV-28': rejecting_mutation,
        'GV-29': missing_dispatch_mutation,
        'GV-31': visual_review_effect, 'GV-32': visual_freshness,
    }
    handlers.update({case_id: (lambda e, cid=case_id: actual_hook_evidence(e, cid)) for case_id in HOOK_CASES})
    rows = []
    for case_id, gate, title in CATALOGUE:
        if case_id not in wanted: continue
        fn = handlers.get(case_id)
        if fn is None:
            def fn(e, cid=case_id):
                raise Blocked(f'{cid}: corresponding production / hook experiment adapter not available')
        row = experiment.run_case(case_id, lambda fn=fn: fn(experiment))
        rows.append({**row, 'gate_id': gate, 'title': title})
    counts = {key: sum(row['status'] == key for row in rows) for key in ['passed', 'failed', 'blocked', 'not_run']}
    after = snapshot_inputs(root)
    source_changed = before != after
    status = 'failed' if counts['failed'] or source_changed else 'blocked' if counts['blocked'] or counts['not_run'] else 'passed'
    result = {'schema_version': 1, 'kind': 'gate-effectiveness-run/v1', 'run_id': experiment.run_id,
              'scope': 'isolated gate experiments, never product qualification', 'fixture_only': True,
              'product_acceptance_promoted': False, 'next_stage_allowed': False,
              'source_tree_sha256': map_identity(before), 'source_unchanged': not source_changed,
              'source_inputs': before,
              'errors': ['source_changed_during_execution'] if source_changed else [],
              'status': status, 'expected_total_groups': len(CASE_IDS), 'selected_groups': wanted,
              'full_catalogue_executed': set(wanted) == set(CASE_IDS),
              'counts': counts, 'scenarios': rows}
    dump(output / 'verification.json', result)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--external-evidence', type=Path)
    parser.add_argument('--case', action='append', choices=CASE_IDS)
    parser.add_argument('--list', action='store_true')
    args = parser.parse_args(argv)
    if args.list:
        print(json.dumps({'kind': 'gate-effectiveness-catalogue/v1', 'scenarios': [dict(zip(('id', 'gate_id', 'title'), row)) for row in CATALOGUE]}, ensure_ascii=False))
        return 0
    output = args.output or args.root / '.build/quality/gate-effectiveness' / uuid.uuid4().hex
    result = run(args.root, output, selected=args.case, external=args.external_evidence)
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
