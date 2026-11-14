#!/usr/bin/env python3
"""Shared, fail-closed gate verdicts; no command execution or approval creation.

The command runner owns argv registration and actual execution. This module
checks its receipts, real inputs, visual report evidence and review scope. Hook
adapters and release callers use the same verdicts. A hash or a declared actor
is an audit aid, not proof of a person's identity: the caller must obtain the
review from the user or an explicitly delegated independent reviewer.
"""
from __future__ import annotations

from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import tempfile
import zlib

from gate_core import allowed, digest, safe_path, source_files

GATE_IDS = tuple(f'G{i:02d}' for i in range(1, 10))
STATUSES = {'passed', 'failed', 'blocked', 'not_run'}
SHA = re.compile(r'[0-9a-f]{64}\Z')
DEFAULT_REQUIRED_CHECKS = {
    'G01': ('spec-contract',),
    'G02': ('frontend-boundaries', 'rust-boundaries'),
    'G04': ('technology-contract',),
    'G05': ('typescript-check', 'rust-check', 'rust-test-compile'),
    'G06': ('affected-functional-tests',),
    'G07': ('storage-contract', 'storage-runtime'),
    'G08': ('artifact-package-check', 'real-game-regression', 'real-translation-service'),
}


class GatePolicyError(ValueError):
    """Malformed or unsafe policy input, never a successful violation test."""


def gate_result(gate_id, status, reason, details=None, required=True):
    if gate_id not in GATE_IDS or status not in STATUSES:
        raise GatePolicyError('invalid gate id/status')
    return {'gate_id': gate_id, 'status': status, 'reason': reason,
            'details': [] if details is None else details, 'required': bool(required)}


# Kept as a function alias so consumers need not serialize a separate class.
GateResult = gate_result


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise GatePolicyError('duplicate JSON key: ' + key)
        result[key] = value
    return result


def read_json(path):
    try:
        return json.loads(Path(path).read_text(encoding='utf-8'),
                          object_pairs_hook=_unique,
                          parse_constant=lambda _: (_ for _ in ()).throw(
                              GatePolicyError('non-finite JSON value')))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise GatePolicyError('cannot read JSON: ' + str(path)) from exc


def _path(root, relative):
    if (not isinstance(relative, str) or not relative or relative != relative.strip()
            or '\\' in relative or any(ord(c) < 32 for c in relative)
            or any(part in ('', '.', '..') for part in relative.split('/'))):
        raise GatePolicyError('path.unsafe: ' + repr(relative))
    try:
        return safe_path(Path(root), relative)
    except ValueError as exc:
        raise GatePolicyError(str(exc)) from exc


def _identity(value):
    return isinstance(value, str) and SHA.fullmatch(value) is not None


def map_identity(files):
    return hashlib.sha256(json.dumps(files, sort_keys=True,
                                    separators=(',', ':')).encode()).hexdigest()


def snapshot_inputs(root, paths=None, *, exclude=()):
    """Full identity for delivery, or explicit inputs for inexpensive hook probes.

    A subset probe is not a complete source proof. Callers must run a complete
    diff for delivery; mtimes are never used as evidence of unchanged content.
    """
    root = Path(root).resolve()
    if paths is None:
        return {p: h for p, h in source_files(root).items() if p not in exclude}
    result = {}
    for relative in sorted(set(paths)):
        if relative in exclude:
            continue
        path = _path(root, relative)
        if path.is_file():
            result[relative] = digest(path)
        elif not path.exists():
            result[relative] = None
        else:
            raise GatePolicyError('source.not_regular_file: ' + relative)
    return result


def _source_map(root, value):
    if isinstance(value, str):
        value = read_json(_path(root, value))
    if isinstance(value, dict) and 'files' in value:
        value = value['files']
    if not isinstance(value, dict):
        raise GatePolicyError('source.snapshot_missing')
    for relative, identity in value.items():
        _path(root, relative)
        if not (_identity(identity) or identity is None
                or isinstance(identity, str) and identity.startswith('symlink:')):
            raise GatePolicyError('source.invalid_identity: ' + relative)
    return value


def changed_paths(root, before, *, exclude=()):
    before = _source_map(root, before)
    current = snapshot_inputs(root, exclude=exclude)
    return sorted(p for p in before.keys() | current.keys()
                  if p not in exclude and before.get(p) != current.get(p))


def classify_impact(paths, *, stage='delivery', full_engineering=False):
    """Conservative path classification; metadata cannot suppress real impact."""
    if stage not in {'development', 'delivery', 'release'}:
        raise GatePolicyError('impact.unknown_stage')
    flags = dict(ui=False, native=False, storage=False, product=False,
                 tooling=False, unknown=False)
    reasons = []
    for path in paths:
        if not isinstance(path, str):
            raise GatePolicyError('impact.invalid_path')
        if path.startswith(('apps/desktop/src/', 'docs/design/')):
            flags['ui'] = flags['product'] = True
        elif path.startswith('apps/desktop/src-tauri/'):
            flags['native'] = flags['product'] = flags['ui'] = True
        elif path.startswith(('crates/', 'resources/', 'assets/')):
            flags['native'] = flags['product'] = True
            if any(part in path.lower() for part in ('storage', 'migration', 'database', 'schema')):
                flags['storage'] = True
        elif path.startswith(('scripts/', 'tests/', '.codex/', '.github/', '.githooks/', 'openspec/changes/')):
            flags['tooling'] = True
        elif path.startswith(('docs/product/', 'docs/architecture/')):
            flags['product'] = True
            if 'database' in path or 'storage' in path:
                flags['storage'] = True
        elif path.startswith('docs/') or path in {'AGENTS.md', 'README.md', 'LICENSE'}:
            pass
        elif path in {'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'rust-toolchain.toml', '.gitignore'}:
            flags['tooling'] = True
        elif path.startswith('openspec/'):
            flags['tooling'] = True
        else:
            flags['unknown'] = flags['product'] = flags['ui'] = flags['native'] = flags['storage'] = True
            reasons.append('unknown production impact: ' + path)
    required = {'G01', 'G03'}
    if flags['tooling'] or flags['product']:
        required.update(('G02', 'G04', 'G05'))
    if full_engineering:
        required.update(('G01', 'G02', 'G03', 'G04', 'G05'))
    if flags['product']:
        required.add('G06')
    if flags['storage']:
        required.add('G07')
    if flags['ui'] or flags['native']:
        required.add('G09')
    if flags['product'] and stage == 'delivery' or stage == 'release':
        required.add('G08')
    if stage == 'release':
        required.update(GATE_IDS)
    flags.update(required_gates=[g for g in GATE_IDS if g in required],
                 reasons=reasons or ['derived from actual changed paths'], stage=stage)
    return flags


def load_context(root, change_id=None):
    root = Path(root).resolve()
    configuration = read_json(_path(root, '.codex/gate-context.json')) if change_id is None else {}
    change_id = change_id or configuration.get('change_id')
    if not isinstance(change_id, str) or not re.fullmatch(r'[a-z0-9][a-z0-9-]*', change_id):
        raise GatePolicyError('context.invalid_change_id')
    relative = f'openspec/changes/{change_id}/verification.json'
    metadata = read_json(_path(root, relative))
    if not isinstance(metadata, dict):
        raise GatePolicyError('context.invalid_verification')
    result = dict(metadata)
    result['change_id'] = change_id
    result['verification_path'] = relative
    result['context_path'] = '.codex/gate-context.json'
    if not isinstance(result.get('allowed_paths'), list) or not result['allowed_paths']:
        raise GatePolicyError('context.allowed_paths_missing')
    _source_map(root, result.get('source_snapshot'))
    # Receipt/report locations may be declared, but callers independently check
    # their producer, actual command and current inputs before granting a pass.
    for key in ('receipts', 'reviews', 'visual'):
        if key in configuration:
            result[key] = configuration[key]
    return result


def _time(value):
    if not isinstance(value, str):
        raise GatePolicyError('receipt.invalid_time')
    try:
        parsed = datetime.fromisoformat(value.replace('Z', '+00:00'))
    except ValueError as exc:
        raise GatePolicyError('receipt.invalid_time') from exc
    if parsed.tzinfo is None:
        raise GatePolicyError('receipt.timezone_missing')
    return parsed


def _file_ref(root, reference):
    if not isinstance(reference, dict) or not _identity(reference.get('sha256')):
        raise GatePolicyError('evidence.invalid_reference')
    path = _path(root, reference.get('path'))
    if not path.is_file():
        raise GatePolicyError('evidence.missing: ' + reference['path'])
    if digest(path) != reference['sha256']:
        raise GatePolicyError('evidence.changed: ' + reference['path'])
    return path


def observed_test_counts(output):
    """Independently read ordinary test-runner summaries from raw logs."""
    python = re.findall(r'\bRan (\d+) tests? in\b', output)
    rust = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', output)
    tap = re.findall(r'^# tests (\d+)\s*$', output, re.MULTILINE)
    if python:
        skipped = sum(int(n) for n in re.findall(r'\bskipped=(\d+)', output))
        return sum(map(int, python)), skipped
    if rust:
        return sum(int(p) + int(f) + int(i) for p, f, i in rust), sum(int(i) for _, _, i in rust)
    if tap:
        return sum(map(int, tap)), sum(map(int, re.findall(r'^# skipped (\d+)\s*$', output, re.MULTILINE)))
    try:
        result = json.loads(output.strip())
    except (ValueError, TypeError):
        result = None
    if (isinstance(result, dict) and result.get('kind') == 'gate-effectiveness-run/v1'
            and result.get('full_catalogue_executed') is True
            and isinstance(result.get('scenarios'), list)):
        rows = result['scenarios']
        ids = [row.get('id', row.get('case_id')) for row in rows]
        if len(rows) != 32 or len(set(ids)) != 32:
            raise GatePolicyError('receipt.effectiveness_catalogue_incomplete')
        if result.get('status') != 'passed' or any(row.get('status') != 'passed' for row in rows):
            raise GatePolicyError('receipt.effectiveness_not_passed')
        return len(rows), 0
    raise GatePolicyError('receipt.test_count_unobserved')


def _component_test_counts(root, receipt, output):
    """Consume the registered component runner's actual per-check artifact."""
    path = _file_ref(root, receipt.get('test_observation'))
    observation = read_json(path)
    if (not isinstance(observation, dict)
            or observation.get('kind') != 'binding-ui-controlled-component-check/v1'
            or not isinstance(observation.get('checks'), list)):
        raise GatePolicyError('receipt.component_observation_invalid')
    rows = observation['checks']
    ids = [row.get('id') for row in rows if isinstance(row, dict)]
    if (not rows or len(ids) != len(rows) or len(set(ids)) != len(rows)
            or any(not isinstance(name, str) or not name for name in ids)
            or any(row.get('status') not in {'passed', 'failed', 'skipped'} for row in rows)):
        raise GatePolicyError('receipt.component_checks_invalid')
    terminal = None
    decoder = json.JSONDecoder()
    for match in re.finditer(r'\{', output):
        try:
            candidate, end = decoder.raw_decode(output, match.start())
        except json.JSONDecodeError:
            continue
        if (not output[end:].strip() and isinstance(candidate, dict)
                and type(candidate.get('checks')) is int and 'status' in candidate):
            terminal = candidate
            break
    failed = [row for row in rows if row['status'] == 'failed']
    if (terminal is None or terminal.get('checks') != len(rows)
            or terminal.get('status') != observation.get('status')
            or terminal.get('errors') != observation.get('errors')
            or len(terminal.get('failed', [])) != len(failed)):
        raise GatePolicyError('receipt.component_summary_mismatch')
    if receipt.get('exit_code') == 0 and (failed or observation.get('status') != 'passed'
                                         or observation.get('errors') != []):
        raise GatePolicyError('receipt.component_failure_hidden')
    for source in observation.get('sources', []):
        _file_ref(root, source)
    return len(rows), sum(row['status'] == 'skipped' for row in rows)


def registered_check_kind(check_id):
    """Test obligations come from code, never from an editable receipt."""
    return 'tests' if check_id.endswith('-tests') or check_id in {
        'storage-runtime', 'storage-migration', 'functional-component',
        'gate-effectiveness-execution'} else 'command'


def validate_receipt(root, receipt, *, expected_command=None, expected_check_kind=None,
                     expected_inputs=None, current_inputs=None, change_id=None, fixture=False):
    gate_id = receipt.get('gate_id', 'G01') if isinstance(receipt, dict) else 'G01'
    if gate_id not in GATE_IDS:
        gate_id = 'G01'
    if receipt is None:
        return gate_result(gate_id, 'not_run', 'receipt.missing')
    try:
        if isinstance(receipt, str):
            receipt = read_json(_path(root, receipt))
            gate_id = receipt.get('gate_id', gate_id)
        if (not isinstance(receipt, dict) or receipt.get('kind') != 'gate-command-receipt/v1'
                or type(receipt.get('schema_version')) is not int or receipt['schema_version'] != 1):
            raise GatePolicyError('receipt.schema_invalid')
        if receipt.get('gate_fixture') is True or receipt.get('fixture') is True:
            if not fixture:
                raise GatePolicyError('receipt.fixture_forbidden')
        if change_id is not None and receipt.get('change_id') != change_id:
            raise GatePolicyError('receipt.wrong_change')
        if not isinstance(receipt.get('run_id'), str) or not receipt['run_id']:
            raise GatePolicyError('receipt.run_id_missing')
        if not isinstance(receipt.get('check_id'), str) or not receipt['check_id']:
            raise GatePolicyError('receipt.check_id_missing')
        command = receipt.get('command')
        if (not isinstance(command, list) or not command
                or any(not isinstance(arg, str) or '\x00' in arg for arg in command)):
            raise GatePolicyError('receipt.command_invalid')
        if expected_command is None:
            raise GatePolicyError('receipt.command_unregistered')
        if command != list(expected_command):
            raise GatePolicyError('receipt.command_mismatch')
        required_kind = expected_check_kind or registered_check_kind(receipt['check_id'])
        if required_kind not in {'command', 'tests'} or receipt.get('check_kind') != required_kind:
            raise GatePolicyError('receipt.check_kind_mismatch')
        started = _time(receipt.get('started_at'))
        finished = _time(receipt.get('finished_at'))
        if finished < started:
            raise GatePolicyError('receipt.time_reversed')
        if finished > datetime.now(timezone.utc) + timedelta(seconds=30):
            raise GatePolicyError('receipt.time_in_future')
        inputs = receipt.get('inputs')
        if not isinstance(inputs, dict) or not inputs:
            raise GatePolicyError('receipt.inputs_missing')
        _source_map(root, inputs)
        # A caller may supply the independently computed map for this same
        # synchronous assessment. Never take this cache from receipt/context.
        current = (snapshot_inputs(root, inputs) if current_inputs is None else
                   {path: current_inputs.get(path) for path in inputs})
        if current != inputs or expected_inputs is not None and inputs != expected_inputs:
            raise GatePolicyError('receipt.stale_inputs')
        logs = []
        for stream in ('stdout', 'stderr'):
            logs.append(_file_ref(root, receipt.get(stream)).read_text(encoding='utf-8'))
        if type(receipt.get('exit_code')) is not int:
            raise GatePolicyError('receipt.exit_invalid')
        if receipt.get('check_kind') not in {'command', 'tests'}:
            raise GatePolicyError('receipt.check_kind_invalid')
        if receipt.get('check_kind') == 'tests':
            if (type(receipt.get('test_count')) is not int or receipt['test_count'] < 1
                    or type(receipt.get('skipped_count')) is not int or receipt['skipped_count'] != 0):
                raise GatePolicyError('receipt.tests_missing_or_skipped')
            output = '\n'.join(logs)
            if 'test_observation' in receipt:
                observed = _component_test_counts(root, receipt, output)
            else:
                observed = observed_test_counts(output)
            if observed != (receipt['test_count'], receipt['skipped_count']):
                raise GatePolicyError('receipt.test_count_mismatch')
        if receipt.get('exit_code') != 0:
            if receipt['exit_code'] == 2 and receipt.get('check_kind') == 'command':
                try:
                    observed = json.loads(logs[0])
                except (ValueError, TypeError):
                    observed = None
                if (isinstance(observed, dict) and observed.get('kind') == 'registered-gate-check/v1'
                        and observed.get('check_id') == receipt.get('check_id')
                        and observed.get('status') == 'blocked' and observed.get('errors')):
                    return gate_result(gate_id, 'blocked', 'receipt.external_inputs_missing', observed['errors'])
            return gate_result(gate_id, 'failed', 'receipt.command_failed',
                               {'check_id': receipt['check_id'], 'exit_code': receipt['exit_code']})
        return gate_result(gate_id, 'passed', 'receipt.executed_current',
                           {'check_id': receipt['check_id'], 'run_id': receipt['run_id']})
    except (ValueError, OSError, KeyError, TypeError) as exc:
        return gate_result(gate_id if gate_id in GATE_IDS else 'G01', 'failed', str(exc))


def _png_size(path):
    """Verify PNG framing, chunk CRC and compressed image stream, without PIL."""
    raw = path.read_bytes()
    if not raw.startswith(b'\x89PNG\r\n\x1a\n'):
        raise GatePolicyError('visual.image_unreadable')
    offset, dimensions, compressed, ended = 8, None, [], False
    depth = color = interlace = None
    palette = False
    while offset + 12 <= len(raw):
        length = struct.unpack('>I', raw[offset:offset + 4])[0]
        kind = raw[offset + 4:offset + 8]
        end = offset + 12 + length
        if end > len(raw):
            raise GatePolicyError('visual.image_truncated')
        data = raw[offset + 8:offset + 8 + length]
        crc = struct.unpack('>I', raw[offset + 8 + length:end])[0]
        if zlib.crc32(kind + data) & 0xffffffff != crc:
            raise GatePolicyError('visual.image_crc')
        if kind == b'IHDR':
            if dimensions is not None or length != 13:
                raise GatePolicyError('visual.image_header')
            dimensions = struct.unpack('>II', data[:8])
            depth, color, compression, filtering, interlace = data[8:]
            if not all(dimensions) or max(dimensions) > 32768:
                raise GatePolicyError('visual.image_dimensions')
            if (compression != 0 or filtering != 0 or interlace not in {0, 1}
                    or color not in {0, 2, 3, 4, 6}
                    or depth not in ({1, 2, 4, 8, 16} if color == 0 else
                                     {1, 2, 4, 8} if color == 3 else {8, 16})):
                raise GatePolicyError('visual.image_header')
        elif kind == b'IDAT':
            if dimensions is None:
                raise GatePolicyError('visual.image_header')
            compressed.append(data)
        elif kind == b'PLTE':
            palette = bool(data) and len(data) % 3 == 0 and len(data) <= 768
        elif kind == b'IEND':
            ended = True
            if end != len(raw):
                raise GatePolicyError('visual.image_trailing_data')
            break
        offset = end
    if not ended or dimensions is None or not compressed:
        raise GatePolicyError('visual.image_incomplete')
    if color == 3 and not palette:
        raise GatePolicyError('visual.image_palette')
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color]
    width, height = dimensions
    passes = [(0, 0, 1, 1)] if not interlace else [
        (0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
        (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
    rows = []
    for x, y, dx, dy in passes:
        pass_width = max(0, (width - x + dx - 1) // dx)
        pass_height = max(0, (height - y + dy - 1) // dy)
        if pass_width and pass_height:
            rows.append(((pass_width * channels * depth + 7) // 8, pass_height))
    expected_bytes = sum((row_bytes + 1) * count for row_bytes, count in rows)
    if expected_bytes > 128 * 1024 * 1024:
        raise GatePolicyError('visual.image_too_large')
    try:
        stream = zlib.decompressobj()
        # Bound decompression before allocating attacker-controlled image data.
        decoded = stream.decompress(b''.join(compressed), expected_bytes + 1)
        if len(decoded) != expected_bytes or not stream.eof or stream.unused_data:
            raise GatePolicyError('visual.image_stream')
        cursor = 0
        for row_bytes, count in rows:
            for _ in range(count):
                if decoded[cursor] > 4:
                    raise GatePolicyError('visual.image_filter')
                cursor += row_bytes + 1
    except zlib.error as exc:
        raise GatePolicyError('visual.image_stream') from exc
    return dimensions


def _review(root, gate_id, review, *, expected_inputs, required_scope,
            authorized_reviewers, fixture, report_sha256=None, manifest_sha256=None):
    if review is None:
        return gate_result(gate_id, 'blocked', 'review.missing')
    if isinstance(review, str):
        review = read_json(_path(root, review))
    if not isinstance(review, dict) or review.get('kind') != 'gate-human-review/v1':
        raise GatePolicyError('review.schema_invalid')
    if review.get('fixture') is True or review.get('gate_fixture') is True:
        if not fixture:
            raise GatePolicyError('review.fixture_forbidden')
    decision = review.get('review_decision')
    if decision == 'pending':
        return gate_result(gate_id, 'blocked', 'review.pending')
    if decision == 'rejected':
        return gate_result(gate_id, 'failed', 'review.rejected')
    if decision != 'approved':
        raise GatePolicyError('review.decision_invalid')
    reviewer = review.get('reviewer_id')
    authors = review.get('author_ids')
    if (not isinstance(authors, list) or not authors or len(authors) != len(set(authors))
            or any(not isinstance(actor, str) or not actor.strip() for actor in authors)
            or not isinstance(reviewer, str) or not reviewer.strip()):
        raise GatePolicyError('review.actor_invalid')
    if (not authorized_reviewers or reviewer not in authorized_reviewers
            or reviewer in authors
            or review.get('reviewer_kind') not in {'user', 'delegated_reviewer'}):
        raise GatePolicyError('review.unauthorized_or_author')
    _time(review.get('reviewed_at'))
    _file_ref(root, review.get('confirmation_source'))
    scope = review.get('scope')
    if not isinstance(scope, list) or len(scope) != len(set(scope)):
        raise GatePolicyError('review.scope_invalid')
    if not set(required_scope).issubset(scope):
        raise GatePolicyError('review.scope_missing')
    if review.get('inputs') != expected_inputs:
        raise GatePolicyError('review.stale_inputs')
    if report_sha256 is not None and review.get('report_sha256') != report_sha256:
        raise GatePolicyError('review.stale_report')
    if manifest_sha256 is not None and review.get('manifest_sha256') != manifest_sha256:
        raise GatePolicyError('review.stale_manifest')
    if review.get('unresolved_findings') != []:
        raise GatePolicyError('review.unresolved_findings')
    return gate_result(gate_id, 'passed', 'review.current_authorized',
                       {'reviewer_id': reviewer, 'scope': list(required_scope)})


def validate_visual(root, manifest, review=None, *, required_scope=None,
                    authorized_reviewers=None, expected_inputs=None,
                    current_inputs=None, fixture=False):
    """G09: real evidence completeness plus explicit, scoped human decision.

    Manifest shape: kind=unified-visual-report/v1; report={path,sha256};
    inputs={relative:sha}; rows=[{scope, design_image:{path,sha256},
    implementation_image:{path,sha256}, viewport:{width,height}, theme,
    data_id, design_state, implementation_state, source_kind}]. Images are
    compared at the same real size; semantic/visual approval remains human.
    """
    if manifest is None:
        return gate_result('G09', 'not_run', 'visual.report_missing')
    try:
        if isinstance(manifest, str):
            manifest = read_json(_path(root, manifest))
        if (not isinstance(manifest, dict)
                or manifest.get('kind') != 'unified-visual-report/v1'):
            raise GatePolicyError('visual.schema_invalid')
        if manifest.get('fixture') is True or manifest.get('gate_fixture') is True:
            if not fixture:
                raise GatePolicyError('visual.fixture_forbidden')
        _file_ref(root, manifest.get('report'))
        inputs = manifest.get('inputs')
        if not isinstance(inputs, dict) or not inputs:
            raise GatePolicyError('visual.inputs_missing')
        _source_map(root, inputs)
        # A report cannot choose a small dependency subset and keep an old
        # approval alive after an omitted UI/module changes. The caller owns
        # the full execution map; standalone validation uses the full tree.
        current = snapshot_inputs(root) if current_inputs is None else current_inputs
        expected = current if expected_inputs is None else expected_inputs
        if inputs != expected or {path: current.get(path) for path in inputs} != inputs:
            raise GatePolicyError('visual.stale_inputs')
        rows = manifest.get('rows')
        if not isinstance(rows, list) or not rows:
            raise GatePolicyError('visual.rows_missing')
        scopes = []
        for row in rows:
            if not isinstance(row, dict) or not isinstance(row.get('scope'), str) or not row['scope']:
                raise GatePolicyError('visual.scope_invalid')
            scopes.append(row['scope'])
            if row.get('source_kind') not in {'real_component', 'signed_package'}:
                raise GatePolicyError('visual.real_capture_required')
            for field in ('theme', 'data_id', 'design_state', 'implementation_state'):
                if not isinstance(row.get(field), str) or not row[field]:
                    raise GatePolicyError('visual.state_missing')
            if row['design_state'] != row['implementation_state']:
                raise GatePolicyError('visual.state_mismatch')
            viewport = row.get('viewport')
            if (not isinstance(viewport, dict)
                    or any(type(viewport.get(k)) is not int or viewport[k] <= 0 for k in ('width', 'height'))):
                raise GatePolicyError('visual.viewport_invalid')
            design = _file_ref(root, row.get('design_image'))
            candidate = _file_ref(root, row.get('implementation_image'))
            if design.resolve() == candidate.resolve():
                raise GatePolicyError('visual.same_capture_file')
            size = (viewport['width'], viewport['height'])
            if _png_size(design) != size or _png_size(candidate) != size:
                raise GatePolicyError('visual.size_mismatch')
        if len(scopes) != len(set(scopes)):
            raise GatePolicyError('visual.duplicate_scope')
        required = required_scope if required_scope is not None else scopes
        if not isinstance(required, (list, tuple)) or not required or not set(required).issubset(scopes):
            raise GatePolicyError('visual.coverage_missing')
        return _review(root, 'G09', review, expected_inputs=inputs,
                       required_scope=required, authorized_reviewers=authorized_reviewers,
                       fixture=fixture, report_sha256=manifest['report']['sha256'],
                       manifest_sha256=map_identity(manifest))
    except (ValueError, OSError, KeyError, TypeError) as exc:
        return gate_result('G09', 'failed', str(exc))


def _merge(gate_id, results):
    if not results:
        return gate_result(gate_id, 'not_run', 'checks.not_run')
    for state in ('failed', 'blocked', 'not_run'):
        found = [r for r in results if r['status'] == state]
        if found:
            return gate_result(gate_id, state, found[0]['reason'], results)
    return gate_result(gate_id, 'passed', 'checks.current_complete', results)


def build_summary(gates, *, change_id=None, impact=None):
    """Summarize this gate run, never promote the separate product lifecycle.

    ``next_stage_allowed`` concerns the evaluated work-item/check stage only;
    the outer product/release authority must still evaluate its own complete
    final-package, game, service and visual obligations.
    """
    if len(gates) != 9 or {r.get('gate_id') for r in gates} != set(GATE_IDS):
        raise GatePolicyError('summary.nine_unique_gates_required')
    required = [r for r in gates if r['required']]
    passed = bool(required) and all(r['status'] == 'passed' for r in required)
    status = 'passed' if passed else next((s for s in ('failed', 'blocked', 'not_run')
                                          if any(r['status'] == s for r in required)), 'failed')
    return {'schema_version': 1, 'kind': 'development-gates-summary/v1',
            'change_id': change_id, 'gates': gates, 'status': status,
            'incomplete': not passed, 'next_stage_allowed': passed,
            'impact': impact or {}, 'product_acceptance_promoted': False}


def evaluate(root, context, *, expected_commands=None, mandatory_checks=None,
             expected_inputs=None, authorized_reviewers=None,
             visual_required_scope=None, visual_authorized_reviewers=None, actual_changed_paths=None,
             snapshot_excludes=(), full_engineering=False, fixture=False):
    """Derive required gates, never trust context.status or UI=false declarations.

    expected_commands and mandatory_checks must come from the code-owned caller
    registry, not the context file. This function never executes caller argv.
    """
    try:
        if not isinstance(context, dict):
            raise GatePolicyError('context.invalid')
        before = _source_map(root, context.get('source_snapshot'))
        current = snapshot_inputs(root, exclude=snapshot_excludes)
        paths = sorted(p for p in before.keys() | current.keys()
                       if p not in snapshot_excludes and before.get(p) != current.get(p))
        if actual_changed_paths is not None:
            if (not isinstance(actual_changed_paths, list)
                    or len(actual_changed_paths) != len(set(actual_changed_paths))):
                raise GatePolicyError('context.invalid_actual_diff')
            for path in actual_changed_paths:
                _path(root, path)
            # Only a code-owned Git/snapshot inspector may supply this list.
            paths = sorted(actual_changed_paths)
        execution_inputs = current if expected_inputs is None else expected_inputs
        if (not isinstance(execution_inputs, dict) or not execution_inputs
                or {path: current.get(path) for path in execution_inputs} != execution_inputs):
            raise GatePolicyError('context.execution_inputs_stale')
        impact = classify_impact(paths, stage='release' if context.get('require_product')
                                 else context.get('stage', 'delivery'), full_engineering=full_engineering)
        patterns = context.get('allowed_paths')
        if not isinstance(patterns, list) or not patterns:
            raise GatePolicyError('scope.allowed_paths_missing')
        for pattern in patterns:
            _path(root, pattern[:-3] if pattern.endswith('/**') else pattern)
        scope_errors = []
        for path in paths:
            try:
                _path(root, path)
            except GatePolicyError as exc:
                scope_errors.append(str(exc))
            if not allowed(path, patterns):
                scope_errors.append('scope.undeclared: ' + path)
        declared = context.get('receipts', {})
        reviews = context.get('reviews', {})
        if not isinstance(declared, dict) or not isinstance(reviews, dict):
            raise GatePolicyError('context.evidence_invalid')
        # The trusted registry argument defines mandatory tests; a work item
        # cannot remove tests by editing its metadata/check list.
        required_checks = DEFAULT_REQUIRED_CHECKS if mandatory_checks is None else mandatory_checks
        registry = expected_commands or {}
        gates = []
        for gate_id in GATE_IDS:
            if gate_id not in impact['required_gates']:
                gates.append(gate_result(gate_id, 'not_run',
                                         'impact.not_applicable', impact, required=False))
                continue
            if gate_id == 'G03':
                gates.append(gate_result(gate_id, 'failed' if scope_errors else 'passed',
                                         'scope.undeclared' if scope_errors else 'scope.actual_diff_allowed',
                                         scope_errors or paths))
                continue
            if gate_id == 'G09':
                visual = context.get('visual') or {}
                if not visual_required_scope:
                    gates.append(gate_result('G09', 'blocked', 'visual.required_scope_unregistered'))
                    continue
                gates.append(validate_visual(root, visual.get('manifest'), visual.get('review'),
                                             required_scope=visual_required_scope,
                                             authorized_reviewers=(authorized_reviewers if visual_authorized_reviewers is None
                                                                   else visual_authorized_reviewers),
                                             expected_inputs=execution_inputs,
                                             current_inputs=current,
                                             fixture=fixture))
                continue
            receipts = declared.get(gate_id, [])
            if not isinstance(receipts, list):
                raise GatePolicyError('receipt.list_required')
            loaded = [read_json(_path(root, r)) if isinstance(r, str) else r for r in receipts]
            by_id = {}
            for receipt in loaded:
                if not isinstance(receipt, dict):
                    raise GatePolicyError('receipt.schema_invalid')
                name = receipt.get('check_id')
                if name in by_id:
                    raise GatePolicyError('receipt.duplicate_check')
                if receipt.get('gate_id') != gate_id:
                    raise GatePolicyError('receipt.wrong_gate')
                by_id[name] = receipt
            checks = required_checks.get(gate_id, ())
            checks_results = []
            for check_id in checks:
                receipt = by_id.get(check_id)
                if receipt is None:
                    checks_results.append(gate_result(gate_id, 'not_run', 'receipt.missing:' + check_id))
                    continue
                checks_results.append(validate_receipt(root, receipt,
                                                       expected_command=registry.get(check_id),
                                                       expected_inputs=execution_inputs,
                                                       current_inputs=current,
                                                       change_id=context.get('change_id'), fixture=fixture))
            if gate_id == 'G01' or gate_id in {'G06', 'G08'}:
                review = reviews.get(gate_id)
                inputs = execution_inputs
                if {path: current.get(path) for path in inputs} != inputs:
                    checks_results.append(gate_result(gate_id, 'failed', 'review.stale_inputs'))
                else:
                    checks_results.append(_review(root, gate_id, review,
                                                  expected_inputs=inputs, required_scope=[gate_id],
                                                  authorized_reviewers=authorized_reviewers,
                                                  fixture=fixture))
            gates.append(_merge(gate_id, checks_results))
        result = build_summary(gates, change_id=context.get('change_id'), impact=impact)
        result['source_inputs_sha256'] = map_identity(current)
        return result
    except (ValueError, OSError, TypeError, KeyError) as exc:
        return build_summary([gate_result(g, 'failed' if g == 'G03' else 'not_run',
                                           str(exc) if g == 'G03' else 'context.invalid') for g in GATE_IDS],
                             change_id=context.get('change_id') if isinstance(context, dict) else None)


def atomic_write_json(root, relative, value):
    destination = _path(root, relative)
    destination.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix='.' + destination.name + '.', dir=destination.parent)
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as stream:
            json.dump(value, stream, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, destination)
    finally:
        if os.path.exists(name):
            os.unlink(name)


@contextmanager
def locked_state(root, relative):
    """Serialize one logical state file; independent run files remain separate."""
    path = _path(root, relative)
    path.parent.mkdir(parents=True, exist_ok=True)
    lock = _path(root, relative + '.lock')
    with lock.open('a+', encoding='utf-8') as stream:
        fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
        try:
            yield path
        finally:
            fcntl.flock(stream.fileno(), fcntl.LOCK_UN)
