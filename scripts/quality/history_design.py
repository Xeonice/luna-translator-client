#!/usr/bin/env python3
"""Validate and execute the pending history design, never product acceptance."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path, PurePosixPath
import re
import secrets
import struct
import subprocess
import zlib
from urllib.parse import urljoin, urlsplit, unquote

import design_authority as authority

ROOT = Path(__file__).resolve().parents[2]
CANDIDATE_ID = 'history-session-paging-20261031'
CANDIDATE = 'docs/design/first-product-v1/candidates/' + CANDIDATE_ID
MANIFEST = CANDIDATE + '/manifest.json'
PLAN = 'docs/development/history-session-paging-plan.json'
CHECKER = 'scripts/quality/history_design.py'
TESTS = 'scripts/quality/test_history_design.py'
RUNNER = 'scripts/report/verify_history_design.cjs'
DIRECT_DEPENDENCIES = ('scripts/quality/design_authority.py',)
HP_IDS = {f'HP-{n:02d}' for n in range(1, 13)}
REVIEW_STATES = ['initial', 'records-loading', 'records-error', 'records-end',
                 'sessions-loading', 'sessions-error', 'sessions-empty', 'empty', 'long']
OBSERVATION_KEYS = {
    'HP-01': {'parent_shell', 'candidate_shell', 'initial_records', 'initial_sessions'},
    'HP-02': {'keyboard_game', 'session', 'manual_record_ids'},
    'HP-03': {'batch_sizes', 'record_count', 'selected_record', 'retained_scroll'},
    'HP-04': {'batch_sizes', 'total', 'record_ids'},
    'HP-05': {'failed_scope', 'successful_scope', 'record_count'},
    'HP-06': {'discarded_requests', 'latest_scope', 'session_scope'},
    'HP-07': {'history_scope', 'word_card', 'record_count'},
    'HP-08': {'draft_preserved_on_failure', 'caret', 'disabled_navigation', 'cancel_dialog'},
    'HP-09': {'first_unit', 'next_unit', 'late_translation_attached_to', 'outside_first_page_locate'},
    'HP-10': {'deep_link_states'},
    'HP-11': {'variants', 'zoom_method', 'narrow_width'},
    'HP-12': {'external_request_count', 'page_error_count', 'console_error_count', 'simulation_boundary', 'offline'},
}
# This existing wallpaper is loaded by the frozen parent's CSS but is outside
# that decision's original 57-source map. Bind it separately, without extending
# or rewriting the original user's source approval.
INHERITED_RUNTIME_ASSETS = {'docs/design/first-product-v1/assets/wallpaper-frame.png':
    '64f704c02b5172f7b18ae09e683c883f6f0913b4ea89b7399f975e9938cd6a8f'}
CAPTURES = {
    'HP-01': ['parent-shell', 'history-default'], 'HP-02': ['game-session-select'],
    'HP-03': ['records-end'], 'HP-04': ['sessions-end'],
    'HP-05': ['records-loading', 'records-error', 'replacement-retry'],
    'HP-06': ['sessions-loading', 'sessions-error', 'stale-requests-discarded'],
    'HP-07': ['parent-render-word-card'], 'HP-08': ['save-error-draft', 'cancel-edit'],
    'HP-09': ['late-translation-locate', 'located-outside-first-page-end'], 'HP-10': REVIEW_STATES,
    'HP-11': ['dark-long', 'light-long', 'narrow-long', 'zoom-200', 'session-menu-wheel'],
    'HP-12': ['offline-boundary'],
}
SHA = re.compile(r'[0-9a-f]{64}\Z')


def _sha(data):
    return hashlib.sha256(data).hexdigest()


def _identity(data):
    return _sha(json.dumps(data, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode())


def _path(root, relative):
    if not isinstance(relative, str) or not relative or '\\' in relative:
        raise ValueError('relative POSIX path required')
    path = PurePosixPath(relative)
    if path.is_absolute() or any(p in {'.', '..'} for p in path.parts):
        raise ValueError('path outside design workspace')
    root = Path(root).resolve()
    value = root.joinpath(*path.parts)
    for item in [value, *value.parents]:
        if item == root:
            break
        if item.is_symlink():
            raise ValueError('symlink forbidden: ' + relative)
    if not value.resolve().is_relative_to(root):
        raise ValueError('path escapes workspace')
    return value


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON field: ' + key)
        result[key] = value
    return result


def _load(path):
    return json.loads(Path(path).read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda v: (_ for _ in ()).throw(ValueError('invalid JSON number')))


class _Dependencies(HTMLParser):
    def __init__(self):
        super().__init__()
        self.refs = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        for key in (['href'] if tag == 'link' else ['src'] if tag in {'script', 'img', 'iframe', 'source', 'video', 'audio'} else []):
            if attrs.get(key):
                self.refs.append(attrs[key])


def _loaded_dependencies(root, html, sources):
    parser = _Dependencies()
    parser.feed(html)
    base = (Path(root) / 'docs/design/first-product-v1/').as_uri() + '/'
    queue = [(base, ref) for ref in parser.refs]
    seen = set()
    while queue:
        origin, ref = queue.pop()
        url = urlsplit(urljoin(origin, ref))
        if url.scheme == 'data':
            continue
        if url.scheme != 'file' or url.netloc:
            raise ValueError('external design dependency forbidden: ' + ref)
        path = Path(unquote(url.path))
        relative = path.relative_to(Path(root).resolve()).as_posix()
        actual = _path(root, relative)
        if relative not in sources or _sha(actual.read_bytes()) != sources[relative]:
            raise ValueError('unbound loaded design dependency: ' + relative)
        if relative in seen:
            continue
        seen.add(relative)
        if actual.suffix == '.css':
            css = actual.read_text()
            refs = re.findall(r'url\(\s*[\"\']?([^\"\')\s]+)', css)
            refs += re.findall(r'@import\s+[\"\']([^\"\']+)', css)
            queue.extend((actual.as_uri(), item) for item in refs)


def manifest_sources(root=ROOT):
    """Bind the new three sources to the immutable, separately approved parent."""
    root = Path(root).resolve()
    approved = authority._record(root)
    m = _load(_path(root, MANIFEST))
    fixed = {'schema_version': 1, 'candidate_id': CANDIDATE_ID,
             'status': 'pending_review', 'entry': CANDIDATE + '/index.html',
             'runtime_base_href': '../../', 'parent_candidate_id': authority.TARGET,
             'parent_source_count': 57}
    for key, value in fixed.items():
        if type(m.get(key)) is not type(value) or m.get(key) != value:
            raise ValueError('history manifest identity mismatch: ' + key)
    owned = m.get('owned_sources')
    if not isinstance(owned, dict) or set(owned) != {'index.html', 'history.js', 'history.css'}:
        raise ValueError('exact three history owned sources required')
    actual = {p.name for p in _path(root, CANDIDATE).iterdir() if p.is_file()}
    if actual != {*owned, 'manifest.json'}:
        raise ValueError('unbound history source file')
    if m.get('parent_sources') != approved['design_sources']:
        raise ValueError('candidate cannot rebind original approved parent sources')
    parent = 'docs/design/first-product-v1/candidates/' + authority.TARGET + '/manifest.json'
    if m.get('parent_manifest') != {'path': parent, 'sha256': approved['design_sources'][parent]}:
        raise ValueError('exact approved parent manifest required')
    a = m.get('parent_source_approval', {})
    if not isinstance(a, dict) or any(a.get(k) != v for k, v in {
            'path': authority.RECORD, 'sha256': authority.RECORD_SHA256,
            'approval_id': authority.FIXED['approval_id'], 'status': 'approved_by_user'}.items()):
        raise ValueError('separate parent source approval required')
    states = m.get('review_states')
    if not isinstance(states, list) or len(states) != 10 or set(states) != {'default', *REVIEW_STATES}:
        raise ValueError('all nine history review states plus ordinary default required')
    sources = dict(approved['design_sources'])
    sources[authority.RECORD] = authority.RECORD_SHA256
    for name, digest in owned.items():
        if not isinstance(digest, str) or not SHA.fullmatch(digest):
            raise ValueError('typed history source digest required')
        relative = CANDIDATE + '/' + name
        if _sha(_path(root, relative).read_bytes()) != digest:
            raise ValueError('history source bytes changed: ' + relative)
        sources[relative] = digest
    html = _path(root, CANDIDATE + '/index.html').read_text()
    if len(re.findall(r'<base\s+href=[\"\']\.\./\.\./[\"\']', html)) != 1:
        raise ValueError('history must inherit exact parent runtime base')
    for name in ('history.css', 'history.js'):
        if f'candidates/{CANDIDATE_ID}/{name}' not in html:
            raise ValueError('history owned adapter not loaded: ' + name)
    sources[MANIFEST] = _sha(_path(root, MANIFEST).read_bytes())
    for relative, digest in INHERITED_RUNTIME_ASSETS.items():
        if _sha(_path(root, relative).read_bytes()) != digest:
            raise ValueError('inherited runtime asset changed: ' + relative)
        sources[relative] = digest
    _loaded_dependencies(root, html, sources)
    return sources


def validate_plan(plan):
    errors = []
    if not isinstance(plan, dict) or type(plan.get('schema_version')) is not int or plan.get('schema_version') != 1:
        return ['typed history plan schema_version 1 required']
    rows = plan.get('rows')
    if not isinstance(rows, list) or any(not isinstance(r, dict) for r in rows):
        return ['history plan rows required']
    if len(rows) != 12 or {r.get('id') for r in rows} != HP_IDS:
        errors.append('exact HP-01 through HP-12 plan rows required')
    if any(r.get('product_status') != 'not_run' for r in rows):
        errors.append('all history plan product_status must remain not_run')
    return errors


def validate_contract(root=ROOT):
    """Metadata guard only. No browser or product status is granted by this API."""
    errors = []
    try:
        manifest_sources(root)
        errors.extend(validate_plan(_load(_path(root, PLAN))))
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append('history design contract invalid: ' + str(error))
    return errors


def source_context(root):
    sources = manifest_sources(root)
    for relative in (CHECKER, TESTS, RUNNER, PLAN, *DIRECT_DEPENDENCIES):
        sources[relative] = _sha(_path(root, relative).read_bytes())
    candidate = {p: v for p, v in sources.items() if p.startswith(CANDIDATE + '/')}
    return {'source_identity': _identity(sources), 'source_hashes': sources,
            'candidate_source_hash': _identity(candidate),
            'parent_source_hash': authority.DESIGN_CLOSURE_SHA256,
            'parent_manifest_sha256': sources['docs/design/first-product-v1/candidates/' + authority.TARGET + '/manifest.json']}


def output_path(root, value):
    root = Path(root).resolve()
    candidate = Path(value)
    if not candidate.is_absolute():
        candidate = root / candidate
    relative = candidate.relative_to(root).as_posix()
    path = _path(root, relative)
    if not relative.startswith('.build/') or relative == '.build':
        raise ValueError('fresh output must be inside workspace .build')
    return path


def png_info(data):
    """Check PNG chunks, CRCs, actual image payload and dimensions independently."""
    if not isinstance(data, bytes) or data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('PNG signature required')
    position, chunks, payload, size, ended = 8, [], b'', None, False
    while position < len(data):
        if position + 12 > len(data):
            raise ValueError('truncated PNG')
        length = struct.unpack('>I', data[position:position + 4])[0]
        kind = data[position + 4:position + 8]
        body = data[position + 8:position + 8 + length]
        if position + 12 + length > len(data) or zlib.crc32(kind + body) & 0xffffffff != struct.unpack('>I', data[position + 8 + length:position + 12 + length])[0]:
            raise ValueError('PNG chunk CRC/length invalid')
        if not chunks and kind != b'IHDR':
            raise ValueError('PNG IHDR must be first')
        if kind == b'IHDR':
            if chunks or length != 13:
                raise ValueError('PNG unique IHDR required')
            width, height, depth, color, compression, filtering, interlace = struct.unpack('>IIBBBBB', body)
            if not 1 <= width <= 12000 or not 1 <= height <= 12000 or depth != 8 or color not in (2, 6) or compression or filtering or interlace:
                raise ValueError('unsupported screenshot PNG format')
            size = (width, height, 3 if color == 2 else 4)
        if kind == b'IDAT':
            payload += body
        if kind == b'IEND':
            if length or position + 12 != len(data):
                raise ValueError('PNG trailing bytes forbidden')
            ended = True
        chunks.append(kind)
        position += 12 + length
    if not ended or not payload or size is None:
        raise ValueError('complete screenshot PNG required')
    raw = zlib.decompress(payload)
    if len(raw) != size[1] * (size[0] * size[2] + 1):
        raise ValueError('PNG decompressed dimensions differ')
    return size[:2]


def validate_execution(root, out, context, invocation):
    """Independently reject stale, partial, mislabeled or missing runner evidence."""
    errors = []
    try:
        r = _load(_path(out, 'report.json'))
        for key, value in {'schema_version': 1, 'kind': 'history-design-execution/v1',
                           'layer': 'design', 'status': 'passed', 'claims_product': False,
                           'product_tests_executed': False, 'real_service_executed': False,
                           'skipped_assertions': 0, 'exit_code': 0}.items():
            if type(r.get(key)) is not type(value) or r.get(key) != value:
                errors.append('typed history execution field mismatch: ' + key)
        for key in ('execution_nonce', 'source_identity', 'candidate_source_hash',
                    'parent_source_hash', 'parent_manifest_sha256', 'source_hashes', 'command'):
            if type(r.get(key)) is not type(context.get(key)) or r.get(key) != context.get(key):
                errors.append('history execution is not bound to current invocation: ' + key)
        if r.get('source_hashes_after') != context['source_hashes'] or source_context(root)['source_hashes'] != context['source_hashes']:
            errors.append('history sources changed during execution')
        expected_command = ['node', str(Path(root).resolve() / RUNNER), str(Path(root).resolve()), str(Path(out).resolve()), str(Path(out).resolve() / 'context.json')]
        if context.get('command') != expected_command or invocation.get('command') != expected_command or type(invocation.get('exit_code')) is not int or invocation.get('exit_code') != 0:
            errors.append('actual Node invocation command and zero exit required')
        for key in ('page_errors', 'console_errors', 'external_requests', 'unbound_requests'):
            if r.get(key) != []:
                errors.append('history runtime errors/network activity: ' + key)
        checks = r.get('checks')
        if not isinstance(checks, list) or any(not isinstance(c, dict) for c in checks):
            errors.append('history execution checks required')
            checks = []
        if len(checks) != 12 or {c.get('id') for c in checks} != HP_IDS:
            errors.append('all HP browser states must execute exactly once')
        screenshots = r.get('screenshots')
        if not isinstance(screenshots, list) or not screenshots:
            errors.append('actual history screenshots required')
            screenshots = []
        by_path = {}
        for shot in screenshots:
            if not isinstance(shot, dict):
                errors.append('typed screenshot required')
                continue
            relative = shot.get('path')
            if relative in by_path:
                errors.append('duplicate screenshot evidence path')
            by_path[relative] = shot
            data = _path(out, relative).read_bytes()
            dimensions = png_info(data)
            if not relative.endswith('.png') or dimensions[0] < 320 or dimensions[1] < 240 or shot.get('sha256') != _sha(data) or type(shot.get('bytes')) is not int or shot['bytes'] != len(data) or tuple(shot.get(k) for k in ('width', 'height')) != dimensions:
                errors.append('actual PNG hash/dimensions mismatch: ' + str(relative))
        total = 0
        for row in checks:
            count = row.get('assertions')
            if row.get('status') != 'passed' or type(count) is not int or count <= 0 or type(row.get('skipped')) is not int or row['skipped'] != 0:
                errors.append('every HP check needs positive actual assertions and zero skips')
            else:
                total += count
            refs = row.get('screenshots')
            if not isinstance(refs, list) or not refs or len(set(refs)) != len(refs) or any(ref not in by_path or not ref.startswith(str(row.get('id')) + '-') for ref in refs):
                errors.append('each HP check must bind its own existing screenshots')
            required = {str(row.get('id')) + '-' + suffix + '.png' for suffix in CAPTURES.get(row.get('id'), [])}
            if not isinstance(refs, list) or not required.issubset(refs):
                errors.append('complete required HP state screenshot set is missing: ' + str(row.get('id')))
            if not isinstance(row.get('observations'), dict) or not OBSERVATION_KEYS.get(row.get('id'), {'invalid'}).issubset(row['observations']):
                errors.append('each HP check must preserve actual observations')
        if type(r.get('assertions')) is not int or r['assertions'] != total or total <= 0:
            errors.append('actual positive assertion total must equal HP counts')
        hp10 = next((c for c in checks if c.get('id') == 'HP-10'), {})
        if hp10.get('observations', {}).get('deep_link_states') != REVIEW_STATES:
            errors.append('all nine distinct historyState deep links must execute')
        for name in ('stdout.log', 'stderr.log'):
            if invocation.get(name + '_sha256') != _sha(_path(out, name).read_bytes()):
                errors.append('actual invocation log hash mismatch')
    except (OSError, ValueError, TypeError, KeyError, AttributeError, struct.error, zlib.error) as error:
        errors.append('invalid history browser execution: ' + str(error))
    return errors


def execute(root=ROOT, output=None):
    root = Path(root).resolve()
    errors = validate_contract(root)
    if errors:
        return {'status': 'failed', 'errors': errors, 'claims_product': False, 'product_tests_executed': False}
    out = output_path(root, output or '.build/history-design-run-' + secrets.token_hex(8))
    if out.exists():
        raise ValueError('output directory already exists; stale evidence cannot be reused')
    out.mkdir(parents=True)
    context = {'schema_version': 1, 'execution_nonce': secrets.token_hex(16), **source_context(root),
               'started_at': datetime.now(timezone.utc).isoformat()}
    command = ['node', str(root / RUNNER), str(root), str(out), str(out / 'context.json')]
    context['command'] = command
    (out / 'context.json').write_text(json.dumps(context, ensure_ascii=False, indent=2) + '\n')
    with (out / 'stdout.log').open('w') as stdout, (out / 'stderr.log').open('w') as stderr:
        try:
            result = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr, timeout=240, check=False)
            code = result.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            stderr.write(str(error))
            code = 124
    invocation = {'command': command, 'exit_code': code, 'execution_nonce': context['execution_nonce'],
                  **{name + '_sha256': _sha((out / name).read_bytes()) for name in ('stdout.log', 'stderr.log')}}
    (out / 'invocation.json').write_text(json.dumps(invocation, ensure_ascii=False, indent=2) + '\n')
    errors = validate_execution(root, out, context, invocation)
    return {'schema_version': 1, 'kind': 'history-design-verification/v1',
            'status': 'failed' if errors else 'passed', 'scope': 'pending_design_candidate_only',
            'claims_product': False, 'product_tests_executed': False,
            'errors': errors, 'output': str(out), 'command': command, 'exit_code': code,
            'source_identity': context['source_identity'], 'execution_nonce': context['execution_nonce']}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check', action='store_true')
    mode.add_argument('--execute', action='store_true')
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        if args.execute:
            result = execute(args.root, args.output)
        else:
            errors = validate_contract(args.root)
            result = {'schema_version': 1, 'kind': 'history-design-metadata/v1',
                      'status': 'failed' if errors else 'passed', 'errors': errors,
                      'scope': 'metadata_only', 'claims_product': False, 'product_tests_executed': False}
    except (OSError, ValueError, TypeError) as error:
        result = {'status': 'failed', 'errors': [str(error)], 'claims_product': False, 'product_tests_executed': False}
    print(json.dumps(result, ensure_ascii=False, indent=2) if args.json else result['status'] + ': ' + '; '.join(result.get('errors', [])))
    return result['status'] != 'passed'


if __name__ == '__main__':
    raise SystemExit(main())
