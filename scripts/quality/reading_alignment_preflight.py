#!/usr/bin/env python3
"""Read-only current-design/production-React measurement, never UI acceptance.

The two producers retain different layouts. Successful evidence collection is
not a successful comparison, a reference approval, or a packaged-native test.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
from html.parser import HTMLParser
import json
import math
import os
from pathlib import Path
import secrets
import shutil
import signal
import struct
import subprocess
import zlib
from urllib.parse import parse_qs, urlsplit

from gate_core import safe_path, source_files
from design_authority import CURRENT_DESIGN_CLOSURE_SHA256, current_source_approval


def enclosing_png_size(box, scroll):
    """Pinned Playwright enclosingIntRect; geometry only, no pixel tolerance.

    Element screenshots use document coordinates and cover the enclosing integer
    rectangle (playwright-core/lib/server/helper.js). Fractional position and
    size can legitimately add two pixels; a width/height-only margin is wrong.
    """
    values = [box[k] for k in ('x', 'y', 'width', 'height')] + [scroll[k] for k in ('x', 'y')]
    if any(type(v) not in (int, float) or not math.isfinite(v) for v in values) or box['width'] <= 0 or box['height'] <= 0:
        raise ValueError('finite actual screenshot geometry required')
    x, y = box['x'] + scroll['x'], box['y'] + scroll['y']
    size = [math.ceil(x + box['width'] - 1e-3) - math.floor(x + 1e-3),
            math.ceil(y + box['height'] - 1e-3) - math.floor(y + 1e-3)]
    if any(value <= 0 for value in size): raise ValueError('positive enclosing PNG dimensions required')
    return size

ROOT = Path(__file__).resolve().parents[2]
RUNNER = 'scripts/report/capture_current_reading_preflight.mjs'
VARIANTS = [
    {'id': 'dark', 'viewport': {'width': 1040, 'height': 760}, 'theme': 'dark', 'zoom': 1},
    {'id': 'light', 'viewport': {'width': 1040, 'height': 760}, 'theme': 'light', 'zoom': 1},
    {'id': 'narrow', 'viewport': {'width': 740, 'height': 1200}, 'theme': 'dark', 'zoom': 1},
    {'id': 'zoom200', 'viewport': {'width': 1040, 'height': 760}, 'theme': 'dark', 'zoom': 2},
]
KIND = 'current-reading-preflight/v1'
STRUCTURE_SELECTORS = {
    'current_card': '[data-guided-current="true"]',
    'body_disclosure': 'details[data-flat-session-section="reading"]',
    'extraction_disclosure': 'details[data-flat-session-section="texts"]',
    'hook_disclosure': 'details[data-flat-session-section="logs"]',
    'mutually_exclusive_tabs': '[role="tablist"]',
}


class RawReadingHTML(HTMLParser):
    """Source-specific diagnostic markers, not general component conformance."""
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.structure = {key: False for key in STRUCTURE_SELECTORS}
        self.text = []
        self.ruby_depth = 0

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'rt':
            self.ruby_depth += 1
        self.structure['current_card'] |= attrs.get('data-guided-current') == 'true'
        for key, value in [('body_disclosure', 'reading'), ('extraction_disclosure', 'texts'), ('hook_disclosure', 'logs')]:
            self.structure[key] |= tag == 'details' and attrs.get('data-flat-session-section') == value
        self.structure['mutually_exclusive_tabs'] |= attrs.get('role') == 'tablist'

    def handle_endtag(self, tag):
        if tag == 'rt':
            self.ruby_depth = max(0, self.ruby_depth - 1)

    def handle_data(self, data):
        if not self.ruby_depth:
            self.text.append(data)


def _inspect_build(root, out, report):
    errors = []
    desktop = Path(root) / 'apps/desktop'
    node = str(Path(shutil.which('node') or '/nonexistent-node').resolve())
    commands = [
        [node, str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
        [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'),
         '--outDir', str(Path(out) / 'test-build'), '--sourcemap', '--manifest'],
    ]
    rows = report.get('build_commands')
    if not isinstance(rows, list) or len(rows) != 2:
        return ['two actual fixed component build commands required']
    for row, expected, name in zip(rows, commands, ('typecheck-tests', 'controlled-production-components-build')):
        log = json.loads(_artifact(out, row['log'], f'logs/{name}.json'), object_pairs_hook=_unique)
        actual = row.get('command')
        if isinstance(actual, list) and actual:
            actual = [str(Path(actual[0]).resolve()), *actual[1:]]
        if (actual != expected or log.get('command') != row.get('command')
                or not _typed(row.get('exit_code'), 0) or not _typed(log.get('exit_code'), 0)
                or log.get('cwd') != 'apps/desktop' or any(not isinstance(log.get(s), str) for s in ('stdout', 'stderr'))):
            errors.append('actual component build command/log differs: ' + name)
    maps = report.get('source_maps')
    if not isinstance(maps, list) or not maps:
        return errors + ['actual built production source maps required']
    required = {'src/app/WindowRoot.tsx', 'src/features/reading/ReadingPanel.tsx', 'src/bridge/projections/application-store.ts'}
    found, seen = set(), set()
    for ref in maps:
        path = ref.get('path')
        if not isinstance(path, str) or not path.startswith('test-build/') or not path.endswith('.js.map') or path in seen:
            raise ValueError('unique actual component source-map artifact required')
        seen.add(path)
        source_map = json.loads(_artifact(out, ref, path), object_pairs_hook=_unique)
        sources, contents = source_map.get('sources'), source_map.get('sourcesContent')
        if not isinstance(sources, list) or not isinstance(contents, list) or len(sources) != len(contents) or ref.get('sources') != sources:
            raise ValueError('actual built source-map sources/content differ')
        for source, content in zip(sources, contents):
            for suffix in required:
                if isinstance(source, str) and source.endswith('/' + suffix):
                    if not isinstance(content, str) or sha(content.encode()) != sha(safe_path(root, 'apps/desktop/' + suffix).read_bytes()):
                        errors.append('built source-map content differs from actual production source: ' + suffix)
                    else:
                        found.add(suffix)
    if found != required:
        errors.append('built graph lacks actual production WindowRoot/ReadingPanel/ApplicationStore content')
    return errors


def sha(data):
    return hashlib.sha256(data).hexdigest()


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key: ' + key)
        result[key] = value
    return result


def load(path):
    return json.loads(path.read_text(), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON number')))


def _typed(actual, expected):
    return type(actual) is type(expected) and actual == expected


def source_context(root):
    files = source_files(Path(root))
    return {'source_hashes': files,
            'source_tree_sha256': sha(json.dumps(files, sort_keys=True).encode()),
            'design_closure_sha256': CURRENT_DESIGN_CLOSURE_SHA256}


def expected_input(root):
    text = safe_path(root, 'docs/design/first-product-v1/data.js').read_text()
    assignment = 'window.LunaDesignData = '
    if text.count(assignment) != 1:
        raise ValueError('exact current design-data assignment required')
    data, _ = json.JSONDecoder(object_pairs_hook=_unique).raw_decode(text.split(assignment, 1)[1].lstrip())
    sample = data['manualSample']
    return {'source_text': sample['source'], 'translation': sample['translation']}


def png_pixels(data, *, decode_pixels=True):
    """Independently decode actual 8-bit RGB/RGBA PNGs, including row filters."""
    if type(decode_pixels) is not bool:
        raise ValueError('typed PNG decoding mode required')
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('PNG signature required')
    at, size, payload, ended = 8, None, bytearray(), False
    while at < len(data):
        if at + 12 > len(data):
            raise ValueError('truncated PNG')
        n = struct.unpack('>I', data[at:at + 4])[0]
        kind = data[at + 4:at + 8]
        body = data[at + 8:at + 8 + n]
        if (at + n + 12 > len(data)
                or zlib.crc32(kind + body) & 0xffffffff != struct.unpack('>I', data[at + 8 + n:at + n + 12])[0]):
            raise ValueError('PNG length/CRC differs')
        if size is None and kind != b'IHDR':
            raise ValueError('PNG first chunk must be IHDR')
        if kind == b'IHDR':
            if size is not None or n != 13:
                raise ValueError('unique PNG header required')
            width, height, depth, color, compression, filtering, interlace = struct.unpack('>IIBBBBB', body)
            if not (0 < width <= 4096 and 0 < height <= 4096 and depth == 8
                    and color in (2, 6) and not compression and not filtering and not interlace):
                raise ValueError('unsupported/bounded screenshot PNG required')
            size = width, height, 3 if color == 2 else 4
        elif kind == b'IDAT':
            payload.extend(body)
        elif kind == b'IEND':
            if n or at + 12 != len(data):
                raise ValueError('PNG trailing bytes forbidden')
            ended = True
        at += n + 12
    if not ended or size is None or not payload:
        raise ValueError('complete PNG required')
    width, height, channels = size
    length = height * (width * channels + 1)
    decoder = zlib.decompressobj()
    raw = decoder.decompress(payload, length + 1)
    if len(raw) != length or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
        raise ValueError('PNG decompressed size/stream differs')
    if not decode_pixels:
        if any(mode > 4 for mode in raw[::width * channels + 1]):
            raise ValueError('invalid PNG row filter')
        return width, height, None
    stride, previous, rgba = width * channels, bytearray(width * channels), bytearray()
    for row in range(height):
        offset = row * (stride + 1)
        mode = raw[offset]
        if mode > 4:
            raise ValueError('invalid PNG row filter')
        current = bytearray(raw[offset + 1:offset + 1 + stride])
        for x in range(stride):
            left = current[x - channels] if x >= channels else 0
            up = previous[x]
            upper_left = previous[x - channels] if x >= channels else 0
            if mode == 1:
                delta = left
            elif mode == 2:
                delta = up
            elif mode == 3:
                delta = (left + up) // 2
            elif mode == 4:
                p = left + up - upper_left
                distances = abs(p - left), abs(p - up), abs(p - upper_left)
                delta = (left, up, upper_left)[distances.index(min(distances))]
            else:
                delta = 0
            current[x] = (current[x] + delta) & 255
        if channels == 4:
            rgba.extend(current)
        else:
            for x in range(0, stride, 3):
                rgba.extend(current[x:x + 3]); rgba.append(255)
        previous = current
    return width, height, bytes(rgba)


def _artifact(out, ref, expected_path):
    if not isinstance(ref, dict) or ref.get('path') != expected_path:
        raise ValueError('exact independent artifact path required: ' + expected_path)
    path = safe_path(out, expected_path)
    if not path.is_file():
        raise ValueError('missing capture artifact: ' + expected_path)
    data = path.read_bytes()
    if type(ref.get('bytes')) is not int or ref['bytes'] != len(data) or ref.get('sha256') != sha(data):
        raise ValueError('artifact bytes/hash differs: ' + expected_path)
    return data


def inspect_execution(root, out, context, invocation):
    """Recompute from raw artifacts; no report-provided pass can approve product."""
    errors, observations = [], []
    try:
        relative_output = Path(out).absolute().relative_to(Path(root).absolute()).as_posix()
        if not relative_output.startswith('.build/') or safe_path(root, relative_output) != Path(out).absolute():
            raise ValueError('capture artifacts must remain inside the actual repository .build')
        report = load(safe_path(out, 'report.json'))
        fixed = {'schema_version': 1, 'kind': KIND, 'status': 'capture_complete',
                 'reference_approval': 'pending_independent_review', 'claims_product': False,
                 'product_alignment_approved': False, 'native_verified': False,
                 'packaged_native_tests_executed': False, 'skipped_assertions': 0}
        for key, value in fixed.items():
            if not _typed(report.get(key), value):
                errors.append('typed capture boundary differs: ' + key)
        for key in ('execution_nonce', 'source_tree_sha256', 'design_closure_sha256', 'source_hashes', 'command'):
            if not _typed(report.get(key), context.get(key)):
                errors.append('capture does not bind current invocation: ' + key)
        if not isinstance(context.get('execution_nonce'), str) or len(context['execution_nonce']) != 32 or any(c not in '0123456789abcdef' for c in context['execution_nonce']):
            errors.append('fresh valid nonce required')
        if not _typed(context.get('variants'), VARIANTS):
            errors.append('exact frozen capture conditions required')
        now_sources = source_context(root)
        for key in ('source_hashes', 'source_tree_sha256', 'design_closure_sha256'):
            if not _typed(context.get(key), now_sources.get(key)):
                errors.append('current sources drifted: ' + key)
        if report.get('source_hashes_after') != context['source_hashes']:
            errors.append('capture source files changed')
        expected_command = ['node', str(Path(root).resolve() / RUNNER), str(Path(root).resolve()), str(Path(out).resolve()), str(Path(out).resolve() / 'context.json')]
        if context.get('command') != expected_command or invocation.get('command') != expected_command or not _typed(invocation.get('exit_code'), 0):
            errors.append('actual fixed command/exit required')
        for stream in ('stdout', 'stderr'):
            if invocation.get(stream + '_sha256') != sha(safe_path(out, stream + '.log').read_bytes()):
                errors.append('actual raw invocation log differs: ' + stream)
        times = [datetime.fromisoformat(value.replace('Z', '+00:00')) for value in
                 (context['started_at'], report['started_at'], report['finished_at'], invocation['finished_at'])]
        if any(value.tzinfo is None for value in times) or times != sorted(times):
            errors.append('actual capture interval must be within invocation')
        if type(report.get('assertion_count')) is not int or report['assertion_count'] <= 0:
            errors.append('positive actual capture assertions required')
        if not isinstance(report.get('browser_version'), str) or not report['browser_version'].strip():
            errors.append('actual browser version required')
        errors += _inspect_build(root, out, report)
        for key in ('page_errors', 'console_errors', 'external_requests'):
            if report.get(key) != []:
                errors.append('capture runtime/network errors: ' + key)
        rows = report.get('variants')
        if not isinstance(rows, list) or [row.get('id') for row in rows] != [row['id'] for row in VARIANTS]:
            raise ValueError('four exact capture variants required once each')
        common_input = None
        fixed_input = expected_input(root)
        for row, variant in zip(rows, VARIANTS):
            identity = variant['id']
            for key in ('viewport', 'theme', 'zoom'):
                if not _typed(row.get(key), variant[key]):
                    errors.append(identity + ': actual capture condition differs: ' + key)
            data = row.get('input')
            if (not isinstance(data, dict) or set(data) != {'source_text', 'translation'}
                    or any(not isinstance(s, str) or not s.strip() for s in data.values())):
                raise ValueError('common actual reading data required')
            if common_input is not None and data != common_input:
                errors.append('reading data changed across variants')
            common_input = data
            if data != fixed_input:
                errors.append('reading data differs from the frozen design sample')
            decoded, structures = {}, {}
            for producer, label in [('design', 'design'), ('react', 'production_react')]:
                dom = load(safe_path(out, f'raw/{identity}-{producer}.dom.json'))
                _artifact(out, row[producer]['dom'], f'raw/{identity}-{producer}.dom.json')
                if dom.get('producer') != label:
                    errors.append(identity + ': independent producer differs')
                for key in ('viewport', 'theme', 'zoom'):
                    if not _typed(dom.get(key), variant[key]):
                        errors.append(identity + ': raw DOM conditions differ: ' + key)
                for key in ('source_text', 'translation'):
                    if dom.get(key) != data[key] or data[key] not in dom.get('page_text', ''):
                        errors.append(identity + ': actual rendered reading data differs: ' + key)
                provenance = dom.get('provenance', {})
                expected_entry = ('docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html'
                                  if producer == 'design' else 'apps/desktop/tests/visual/index.html')
                if (provenance.get('entry') != expected_entry
                        or not _typed(provenance.get('production_styles_modified'), False)
                        or not _typed(provenance.get('controlled_transport'), producer == 'react')):
                    errors.append(identity + ': producer/style provenance differs')
                origins = report.get('server_origins', {})
                origin = urlsplit(origins.get(producer, ''))
                actual_url = urlsplit(dom.get('url', ''))
                expected_url_path = '/' + expected_entry if producer == 'design' else '/tests/visual/index.html'
                query = parse_qs(actual_url.query)
                if (origin.scheme != 'http' or origin.hostname != '127.0.0.1' or not origin.port
                        or origin.username or origin.password or origin.path or origin.query or origin.fragment
                        or actual_url.scheme != origin.scheme or actual_url.netloc != origin.netloc
                        or actual_url.path != expected_url_path or actual_url.fragment
                        or query.get('scenario') != ['reading']
                        or (producer == 'react' and query.get('role') != ['main'])
                        or (producer == 'design' and query.get('theme') != [variant['theme']])):
                    errors.append(identity + ': raw DOM is not the actual local producer entry')
                geometry, computed = dom.get('geometry', {}), dom.get('computed_conditions', {})
                for key, expected in [('viewport_width', variant['viewport']['width']), ('viewport_height', variant['viewport']['height'])]:
                    if not _typed(geometry.get(key), expected) or not _typed(computed.get(key), expected):
                        errors.append(identity + ': actual measured viewport differs')
                if (computed.get('theme_attribute') != variant['theme']
                        or dom.get('document_attributes', {}).get('data-theme') != variant['theme']
                        or type(computed.get('document_zoom')) not in (int, float) or computed['document_zoom'] != variant['zoom']
                        or type(computed.get('device_scale_factor')) not in (int, float) or computed['device_scale_factor'] != 1):
                    errors.append(identity + ': actual document theme/zoom/scale differs')
                structure = dom.get('structure')
                keys = {'current_card', 'body_disclosure', 'extraction_disclosure', 'hook_disclosure', 'mutually_exclusive_tabs'}
                if not isinstance(structure, dict) or not keys.issubset(structure) or any(type(structure[k]) is not bool for k in keys):
                    raise ValueError('actual typed structural observations required')
                if dom.get('structure_selectors') != STRUCTURE_SELECTORS or not isinstance(dom.get('root_html'), str) or not dom['root_html'].strip():
                    raise ValueError('actual raw reading HTML and code-owned marker queries required')
                parsed = RawReadingHTML(); parsed.feed(dom['root_html']); parsed.close()
                if structure != parsed.structure:
                    errors.append(identity + ': raw HTML structural markers differ from claimed facts')
                raw_text = ''.join(parsed.text)
                for key in ('source_text', 'translation'):
                    if data[key] not in raw_text or dom.get('observed_' + key) != data[key]:
                        errors.append(identity + ': raw HTML rendered source/translation differs')
                structures[producer] = parsed.structure
                dimensions = png_pixels(_artifact(out, row[producer]['screenshot'], f'captures/{identity}-{producer}.png'))
                expected_dimensions = variant['viewport']['width'], variant['viewport']['height']
                if dimensions[:2] != expected_dimensions or (row[producer]['screenshot'].get('width'), row[producer]['screenshot'].get('height')) != expected_dimensions:
                    errors.append(identity + ': actual PNG dimensions differ')
                decoded[producer] = dimensions[2]
            if row['design']['dom']['sha256'] == row['react']['dom']['sha256']:
                errors.append(identity + ': independent raw DOM artifacts required')
            a, b = decoded['design'], decoded['react']
            if len(a) != len(b):
                raise ValueError('comparison dimensions differ')
            different = sum(a[i:i + 4] != b[i:i + 4] for i in range(0, len(a), 4))
            total = len(a) // 4
            comparison = row['comparison']
            if (not _typed(comparison.get('pixel_comparison_eligible'), False)
                    or comparison.get('scope') != 'diagnostic_full_viewport_hosts_differ_not_release_metric'):
                errors.append(identity + ': host-dependent pixels cannot be product fidelity evidence')
            if (not _typed(comparison.get('different_pixels'), different)
                    or not _typed(comparison.get('total_pixels'), total)
                    or type(comparison.get('ratio')) not in (int, float)
                    or abs(comparison['ratio'] - different / total) > 1e-12):
                errors.append(identity + ': raw PNG comparison differs from claimed metrics')
            diff = png_pixels(_artifact(out, comparison['diff'], f'captures/{identity}-diff.png'))
            if diff[:2] != (variant['viewport']['width'], variant['viewport']['height']):
                errors.append(identity + ': diff dimensions differ')
            # Red is an exact changed pixel; unchanged pixels are neutral gray.
            expected_diff = bytearray()
            for i in range(0, len(a), 4):
                expected_diff.extend((255, 0, 0, 255) if a[i:i + 4] != b[i:i + 4] else (48, 48, 48, 255))
            if diff[2] != expected_diff:
                errors.append(identity + ': diff artifact does not represent raw pixel comparison')
            structural_differences = [key for key in sorted(structures['design']) if structures['design'][key] != structures['react'].get(key)]
            status = 'failed' if different or structural_differences else 'blocked'
            if row.get('alignment_status') != status:
                errors.append(identity + ': diagnostic alignment status differs')
            observations.append({'id': identity, 'different_pixels': different, 'total_pixels': total,
                                 'structural_differences': structural_differences, 'alignment_status': status})
    except (OSError, ValueError, KeyError, TypeError, AttributeError, zlib.error, struct.error, OverflowError) as error:
        errors.append('invalid reading preflight evidence: ' + str(error))
    return {'status': 'failed' if errors else 'passed', 'errors': errors, 'observations': observations,
            'scope': 'read_only_measurement_integrity_only', 'reference_approved': False,
            'product_alignment_approved': False, 'product_tests_executed': False,
            'next_product_stage_permitted': False}


def execute(root, output):
    root = Path(root).resolve()
    if not isinstance(output, str) or not output.startswith('.build/'):
        raise ValueError('fresh repository-relative .build output required')
    out = safe_path(root, output)
    if out.exists():
        raise ValueError('output already exists; previous/approved artifacts cannot be overwritten')
    approval = current_source_approval(root)
    if approval.get('status') != 'approved_by_user' or approval.get('errors'):
        raise ValueError('exact current design source approval required')
    out.mkdir(parents=True)
    context = {**source_context(root), 'schema_version': 1, 'execution_nonce': secrets.token_hex(16),
               'variants': VARIANTS, 'started_at': datetime.now(timezone.utc).isoformat()}
    command = ['node', str(root / RUNNER), str(root), str(out), str(out / 'context.json')]
    context['command'] = command
    (out / 'context.json').write_text(json.dumps(context, ensure_ascii=False, indent=2) + '\n')
    try:
        child = subprocess.Popen(command, cwd=root, text=True, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = child.communicate(timeout=300)
            code = child.returncode
        except subprocess.TimeoutExpired:
            # Only this diagnostic's isolated group; never Luna, Wine or games.
            try: os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError: pass
            try:
                stdout, stderr = child.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                try: os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError: pass
                stdout, stderr = child.communicate()
            stderr += '\nDiagnostic command timed out; its isolated process group was terminated.\n'
            code = -1
    except OSError as error:
        stdout, stderr, code = '', str(error), -1
    (out / 'stdout.log').write_text(stdout)
    (out / 'stderr.log').write_text(stderr)
    invocation = {'command': command, 'exit_code': code, 'finished_at': datetime.now(timezone.utc).isoformat(),
                  'stdout_sha256': sha(stdout.encode()), 'stderr_sha256': sha(stderr.encode())}
    (out / 'invocation.json').write_text(json.dumps(invocation, indent=2) + '\n')
    result = inspect_execution(root, out, context, invocation)
    result.update(output=output, execution_nonce=context['execution_nonce'],
                  source_tree_sha256=context['source_tree_sha256'], command=command, exit_code=code)
    (out / 'inspection.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--execute', action='store_true', required=True)
    p.add_argument('--output', required=True)
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--json', action='store_true')
    args = p.parse_args()
    try:
        result = execute(args.root, args.output)
    except (OSError, ValueError, KeyError, TypeError) as error:
        result = {'status': 'failed', 'errors': [str(error)], 'product_tests_executed': False,
                  'product_alignment_approved': False, 'next_product_stage_permitted': False}
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return result['status'] != 'passed'


if __name__ == '__main__':
    raise SystemExit(main())
