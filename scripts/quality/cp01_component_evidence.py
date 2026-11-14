"""Independent evidence checks for the limited CP-01 real React adapter.

This grants neither current-design pixel approval nor packaged/native capability.
"""
from datetime import datetime
import hashlib
import json
import math
import re
import struct
import zlib
from pathlib import Path
import shutil

from gate_core import safe_path, digest
from history_design import png_info

CHECK_ID = 'product-cp01-reading-tests'
RUNNER = 'scripts/report/verify_cp01_reading.mjs'
CASES = (
    'CP01-01-independent-current-and-default-collapsed-datasets',
    'CP01-02-filter-page-scroll-and-follow-survive-collapse',
    'CP01-03-hidden-page-completion-preserves-anchor-and-errors',
    'CP01-04-current-annotations-use-real-preference-commands',
    'CP01-05-old-word-card-survives-long-new-dialogue',
    'CP01-06-current-actions-and-preview-retain-ownership',
    'CP01-07-queued-running-stream-completed-no-generic-empty-flash',
    'CP01-visual-dark', 'CP01-visual-light', 'CP01-visual-narrow', 'CP01-visual-zoom200',
)
VARIANTS = {'dark': ('dark', 1160, 900, 1), 'light': ('light', 1160, 900, 1),
            'narrow': ('dark', 740, 1200, 1), 'zoom200': ('dark', 1160, 900, 2)}
ACTIONS = frozenset(('复制原文', '重新翻译本句', '可点击分词', '假名注音', '词性提示', '分析本句文法',
                     '暂停自动输入', '关闭自动翻译', '重新选择文本流…', '查看历史', '刷新', '暂停', '预览详情'))


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise ValueError('duplicate JSON key: ' + key)
        result[key] = value
    return result


def load(path):
    return json.loads(path.read_text(), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON number')))


def _rectangle(value):
    keys = ('left', 'top', 'right', 'bottom', 'width', 'height')
    if (not isinstance(value, dict) or any(type(value.get(k)) not in (int, float)
            or not math.isfinite(value[k]) for k in keys)
            or value['width'] < 0 or value['height'] < 0
            or value['right'] < value['left'] or value['bottom'] < value['top']
            or value['width'] != value['right'] - value['left']
            or value['height'] != value['bottom'] - value['top']):
        raise ValueError('actual finite rectangle required')
    return value


def _contains(inner, outer):
    inner, outer = _rectangle(inner), _rectangle(outer)
    return (inner['left'] >= outer['left'] and inner['right'] <= outer['right']
            and inner['top'] >= outer['top'] and inner['bottom'] <= outer['bottom'])


def _overlaps(a, b):
    a, b = _rectangle(a), _rectangle(b)
    return min(a['right'], b['right']) > max(a['left'], b['left']) and min(a['bottom'], b['bottom']) > max(a['top'], b['top'])


def validate_geometry(row, variant):
    errors = []
    theme, width, height, zoom = VARIANTS[variant]
    if (row.get('theme') != theme or type(row.get('zoom')) not in (int, float) or row['zoom'] != zoom
            or row.get('viewport') != {'width': width, 'height': height, 'deviceScaleFactor': 1}
            or type(row.get('viewport', {}).get('deviceScaleFactor')) not in (int, float)
            or row.get('outer') != [0, 0] or any(type(v) not in (int, float) for v in row.get('outer', []))):
        errors.append('actual variant/theme/zoom/viewport differs: ' + variant)
    card, source, preview = (_rectangle(row[k]) for k in ('current', 'source', 'preview'))
    if (any(rect[axis] <= 0 for rect in (card, source, preview) for axis in ('width', 'height'))
            or card['left'] < 0 or card['right'] > width
            or preview['left'] < 0 or preview['right'] > width or not _contains(source, card)
            or _overlaps(card, preview)):
        errors.append('current reading/preview clipping or overlap: ' + variant)
    tokens = row.get('tokens')
    if not isinstance(tokens, list) or not tokens: raise ValueError('actual token/ruby geometry required')
    for token in tokens:
        if not isinstance(token.get('rects'), list) or not token['rects']: raise ValueError('actual token rectangles required')
        for rect in token['rects']:
            if (not _contains(rect, source) or _overlaps(rect, preview)
                    or rect['width'] <= 0 or rect['height'] <= 0):
                errors.append('source token/ruby overflow or preview covering text: ' + variant)
    actions = row.get('accessibility')
    if (not isinstance(actions, list) or len(actions) != len(ACTIONS)
            or {a.get('name') for a in actions} != ACTIONS):
        raise ValueError('all exact current/preview actions must be uniquely measured')
    for action in actions:
        _rectangle(action)
        if (action['width'] <= 0 or action['height'] <= 0 or action['left'] < 0 or action['right'] > width
                or action['top'] < 0 or action['bottom'] > height or action.get('viewportWidth') != width
                or action.get('viewportHeight') != height or action.get('hit') is not True):
            errors.append('action clipped or obscured: ' + str(action.get('name')))
    return errors


def validate_hidden(rows):
    errors = []
    if not isinstance(rows, list) or [r.get('hiddenBy') for r in rows] != ['collapse', 'main_navigation']:
        return ['both hidden-details and hidden-main completion paths required']
    for row in rows:
        a, b = row['anchorBefore'], row['anchorAfter']
        if {k: v for k, v in a.items() if k != 'offset'} != {k: v for k, v in b.items() if k != 'offset'}:
            errors.append('hidden completion changed exact reading identity/range/position')
        for anchor in (a, b):
            if (not isinstance(anchor, dict) or any(not isinstance(anchor.get(k), str)
                    or re.fullmatch(r'[0-9]+', anchor[k]) is None for k in ('sequence', 'highWatermark'))
                    or not isinstance(anchor.get('text'), str) or not anchor['text'].strip()
                    or anchor.get('viewport') != {'width': 1160, 'height': 900, 'deviceScaleFactor': 1}
                    or int(anchor['sequence']) > int(anchor['highWatermark'])):
                raise ValueError('complete actual anchor identity/text/range/viewport required')
            offset = anchor.get('offset'); scale = anchor['viewport'].get('deviceScaleFactor')
            if type(offset) not in (int, float) or not math.isfinite(offset) or type(scale) not in (int, float) or scale <= 0:
                raise ValueError('actual anchor/scale required')
            if type(anchor.get('rasterOffset')) is not int or anchor['rasterOffset'] != math.floor(offset * scale + 0.5):
                errors.append('physical anchor does not match raw offset and actual scale')
        if row.get('rawOffsetDelta') != b['offset'] - a['offset']:
            errors.append('raw anchor offset delta differs')
        facts = row.get('scopeFacts', {})
        scopes = facts.get('scopes')
        if type(facts.get('freshReads')) is not int or facts['freshReads'] != 1 or not isinstance(scopes, list) or not scopes or any(scope != scopes[0] for scope in scopes):
            errors.append('hidden completion reset or changed original query range')
        if not isinstance(scopes, list) or not scopes or any(not isinstance(scope, dict)
                or any(not isinstance(scope.get(k), str) or not scope[k].strip()
                       for k in ('snapshotId', 'highWatermark', 'signature'))
                or scope['highWatermark'] != a['highWatermark'] for scope in scopes):
            errors.append('complete nonempty original opaque scope identity and matching watermark required')
    return errors


def inspect(root, out, nonce, started, completed):
    errors, details = [], {}
    try:
        root, out = Path(root), Path(out)
        report_path = safe_path(out, 'verification.json'); r = load(report_path)
        exact = {'schemaVersion': 1, 'id': 'cp01-current-reading-real-react', 'nonce': nonce,
                 'status': 'passed', 'layer': 'real_react_controlled_transport', 'skippedAssertions': 0,
                 'nativeOrGameVerified': False, 'productReleaseApproved': False, 'pixelAcceptance': False,
                 'referenceApproval': 'not_granted'}
        for key, value in exact.items():
            if type(r.get(key)) is not type(value) or r.get(key) != value: errors.append('CP01 typed identity/capability differs: ' + key)
        times = [datetime.fromisoformat(r[k].replace('Z', '+00:00')) for k in ('executedAt', 'finishedAt')]
        if any(t.tzinfo is None for t in times) or not started <= times[0].timestamp() <= times[1].timestamp() <= completed:
            errors.append('CP01 report outside actual invocation interval')
        if r.get('errors') != [] or r.get('externalRequests') != []: errors.append('CP01 runtime/network errors')
        expected_paths = {p.relative_to(root).as_posix() for folder in ('apps/desktop/src', 'apps/desktop/tests') for p in (root / folder).rglob('*') if p.is_file()} | {RUNNER}
        sources = r.get('sourceSha256')
        if not isinstance(sources, dict) or set(sources) != expected_paths or r.get('sourceSha256After') != sources:
            raise ValueError('exact actual production/test source closure required')
        if any(digest(safe_path(root, path)) != value for path, value in sources.items()): errors.append('CP01 actual source hash drift')
        checks = r.get('checks')
        if not isinstance(checks, list) or tuple(c.get('id') for c in checks) != CASES: raise ValueError('all eleven exact CP01 cases required')
        counts = [c.get('assertionCount') for c in checks]
        if any(c.get('status') != 'passed' or type(n) is not int or n <= 0 for c, n in zip(checks, counts)):
            errors.append('CP01 case must actually execute positive assertions without skipping')
        if type(r.get('assertionCount')) is not int or r['assertionCount'] < sum(n for n in counts if type(n) is int):
            errors.append('CP01 total assertion count inconsistent')
        errors += validate_hidden(checks[2]['observations'])
        for observation in checks[2]['observations']:
            raw = load(safe_path(out, 'hidden-' + observation['hiddenBy'] + '.geometry.json'))
            if raw != {'before': observation['anchorBefore'], 'after': observation['anchorAfter'],
                       'rawOffsetDelta': observation['rawOffsetDelta']}:
                errors.append('CP01 raw hidden measurements differ from reported observations')
        for case in checks[7:]:
            variant = case['id'].removeprefix('CP01-visual-')
            measured = load(safe_path(out, 'current-reading-' + variant + '.geometry.json'))
            actions = load(safe_path(out, 'current-reading-' + variant + '.actions.json'))
            observation = case['observations']
            if {**measured, 'accessibility': actions} != observation: errors.append('CP01 raw measurements differ from reported observations')
            errors += validate_geometry(observation, variant)
        captures = r.get('captures')
        required = {'three-independent-sections.png', 'fixed-old-learning-long-new-current.png'} | {'current-reading-' + v + '.png' for v in VARIANTS}
        if not isinstance(captures, list) or len(captures) != len(required) or {c.get('file') for c in captures} != required:
            raise ValueError('all six actual CP01 captures required')
        for c in captures:
            path = safe_path(out, c['file']); actual = png_info(path.read_bytes())
            if (digest(path) != c.get('sha256') or actual != (c['viewport']['width'], c['viewport']['height'])
                    or c.get('producer') != 'production_react_controlled_transport'
                    or c.get('approvedReference') is not False or c.get('nativeVerified') is not False):
                errors.append('CP01 actual PNG/source/capability differs')
        builds = r.get('builds')
        if not isinstance(builds, list) or len(builds) != 2: raise ValueError('actual test typecheck and component build required')
        node = Path(shutil.which('node')).resolve(); desktop = root / 'apps/desktop'
        commands = [[str(node), str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
                    [str(node), str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'), '--outDir', str(out / 'controlled-build'), '--sourcemap']]
        for build, command, name in zip(builds, commands, ('typecheck-tests.json', 'component-build.json')):
            log = load(safe_path(out, name)); actual = build.get('command', [])
            if actual: actual = [str(Path(actual[0]).resolve()), *actual[1:]]
            if actual != command or build.get('log') != name or log.get('command') != build.get('command') or type(build.get('exitCode')) is not int or build['exitCode'] != 0 or log.get('exitCode') != 0:
                errors.append('CP01 actual fixed build command/log differs')
        required_sources = {'ReadingPanel.tsx', 'SessionRecordPanel.tsx', 'SourceText.tsx'}; found = set()
        for p in (out / 'controlled-build').rglob('*.js.map'):
            m = load(p)
            for name, content in zip(m['sources'], m['sourcesContent']):
                for filename in required_sources:
                    if name.endswith('/' + filename):
                        folder = 'language' if filename == 'SourceText.tsx' else 'reading'
                        actual = safe_path(root, f'apps/desktop/src/features/{folder}/{filename}').read_text()
                        if content != actual: errors.append('CP01 built component content differs: ' + filename)
                        else: found.add(filename)
        if found != required_sources: errors.append('CP01 graph missing actual production components')
        details = {'test_count': len(checks), 'assertion_count': r['assertionCount'],
                   'component_receipt': str(report_path.relative_to(root)), 'component_receipt_sha256': digest(report_path),
                   'behavior_status': 'passed' if not errors else 'failed', 'visual_status': 'geometry_only_no_pixel_approval',
                   'product_packaged_native_game_tests_executed': False}
    except (OSError, ValueError, KeyError, TypeError, AttributeError, OverflowError, struct.error, zlib.error) as error:
        errors.append('CP01 component evidence invalid: ' + str(error))
    return errors, details
