"""Independent finite settings behavior oracle. Never grants native/pixel approval."""
from copy import deepcopy
from datetime import datetime
from html.parser import HTMLParser
import json
import math
from pathlib import Path
import shutil
import struct
import zlib

from gate_core import safe_path, digest, source_files
from history_design import png_info

CHECK_ID = 'product-settings-controls-tests'
RUNNER = 'scripts/report/verify_settings_controls.mjs'
CASES = (
    'SC-01-workspace-three-protocols-and-exact-save',
    'SC-02-shared-connection-and-orphan-draft-reachable',
    'SC-03-collapse-and-service-switch-preserve-pool-draft',
    'SC-04-explicit-test-target-and-failure-preserve-lifecycle',
    'SC-05-advanced-purpose-learning-terms-reachable',
    'SC-06-current-session-pause-exact-authority',
    'SC-07-pause-no-session-error-and-late-isolation',
    'SC-08-history-always-and-real-navigation',
    'SC-09-login-query-set-requery-projection',
    'SC-10-login-approval-is-not-enabled',
    'SC-11-login-failure-preserves-known-state',
    'SC-12-login-busy-and-late-isolation',
)
SECTIONS = ('service-advanced', 'translation', 'learning', 'testing', 'terms')
MIN_ACTIONS = (3, 3, 2, 2, 6, 1, 3, 2, 3, 1, 2, 3)
DESCRIBED_SWITCHES = frozenset(('新台词自动加入当前会话', '可点击日语分词', '显示日语注音', '显示词性标记'))
COMPONENTS = {
    'apps/desktop/src/features/model-config/ModelConfigScreen.tsx',
    'apps/desktop/src/features/model-config/ConnectionEditor.tsx',
    'apps/desktop/src/features/model-config/ModelEditor.tsx',
    'apps/desktop/src/features/preferences/GeneralSettings.tsx',
    'apps/desktop/src/windows/settings/SettingsWindow.tsx',
    'apps/desktop/src/ui/primitives/Controls.tsx',
}


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise ValueError('duplicate JSON key: ' + key)
        result[key] = value
    return result


def load(path):
    return json.loads(path.read_text(), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON number')))


def _same(actual, expected):
    # Python considers True == 1; evidence must preserve JSON primitive types.
    return json.dumps(actual, sort_keys=True, ensure_ascii=False) == json.dumps(expected, sort_keys=True, ensure_ascii=False)


class _SwitchDom(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = {'tag': 'root', 'attrs': {}, 'children': [], 'text': ''}; self.stack = [self.root]

    def handle_starttag(self, tag, attrs):
        node = {'tag': tag, 'attrs': dict(attrs), 'children': [], 'text': ''}
        self.stack[-1]['children'].append(node)
        if tag not in {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr'}: self.stack.append(node)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index]['tag'] == tag:
                del self.stack[index:]; break

    def handle_data(self, data):
        for node in self.stack: node['text'] += data


def _descendants(node):
    for child in node['children']:
        yield child
        yield from _descendants(child)


def validate_switch_geometry(rows, dom):
    """Read raw DOM identities again; compare exact finite layout rectangles."""
    errors = []
    if (not isinstance(rows, list) or len(rows) != len(DESCRIBED_SWITCHES)
            or {r.get('name') for r in rows} != DESCRIBED_SWITCHES):
        return ['all four described general switches must be uniquely measured']
    parsed = _SwitchDom(); parsed.feed(dom)
    general = [n for n in _descendants(parsed.root) if n['attrs'].get('data-ui-page') == 'general-settings']
    if len(general) != 1: return ['one actual general settings DOM root required for switch geometry']
    nodes = list(_descendants(general[0]))
    described = []
    for label in [n for n in nodes if 'data-switch-label' in n['attrs']]:
        children = list(_descendants(label)); small = [n for n in children if n['tag'] == 'small']
        if small:
            strong = [n for n in children if n['tag'] == 'strong']
            if len(small) != 1 or len(strong) != 1: return ['each actual switch label needs one title and description']
            described.append((strong[0]['text'], small[0]))
    if len(described) != len(rows) or {name for name, _ in described} != DESCRIBED_SWITCHES:
        errors.append('measured switch inventory differs from actual described DOM labels')
    def rectangle(value):
        keys = ('left', 'top', 'right', 'bottom', 'width', 'height')
        if (not isinstance(value, dict) or any(type(value.get(k)) not in (int, float) or not math.isfinite(value[k]) for k in keys)
                or value['width'] <= 0 or value['height'] <= 0
                or value['width'] != value['right'] - value['left'] or value['height'] != value['bottom'] - value['top']):
            raise ValueError('actual finite positive switch/text rectangle required')
        return value
    for row in rows:
        title, description, track = [rectangle(row[key]) for key in ('strong', 'small', 'track')]
        if description['top'] < title['bottom']: errors.append('switch description overlaps title instead of following on a separate line: ' + row['name'])
        for text in (title, description):
            if min(text['right'], track['right']) > max(text['left'], track['left']) and min(text['bottom'], track['bottom']) > max(text['top'], track['top']):
                errors.append('switch track overlaps label text: ' + row['name'])
        small = [n for name, n in described if name == row['name']]
        controls = [n for n in nodes if n['attrs'].get('role') == 'switch' and n['attrs'].get('aria-label') == row['name']]
        if (len(small) != 1 or len(controls) != 1 or row.get('title') != row['name'] or not row.get('description')
                or row.get('descriptionLinked') is not True or not row.get('descriptionId')
                or not isinstance(row.get('describedBy'), list) or row['descriptionId'] not in row['describedBy']):
            errors.append('actual accessible switch title/description identity missing: ' + row['name']); continue
        actual_ids = (controls[0]['attrs'].get('aria-describedby') or '').split()
        if (small[0]['attrs'].get('id') != row['descriptionId'] or small[0]['text'] != row['description']
                or actual_ids != row['describedBy'] or row['descriptionId'] not in actual_ids
                or len([n for n in nodes if n['attrs'].get('id') == row['descriptionId']]) != 1):
            errors.append('accessible description differs from actual DOM association: ' + row['name'])
    return errors


def validate_case(case_id, raw, dom):
    """Recompute behavior from original input, actual actions, commands and DOM."""
    errors = []
    if case_id not in CASES: return ['unknown settings case']
    index = CASES.index(case_id)
    if (type(raw.get('schemaVersion')) is not int or raw['schemaVersion'] != 1
            or raw.get('id') != case_id or not isinstance(raw.get('inputs'), dict)
            or not isinstance(raw.get('outputs'), dict) or not isinstance(raw.get('actions'), list)
            or len(raw['actions']) < MIN_ACTIONS[index]
            or any(not isinstance(a, str) or not a.strip() for a in raw['actions'])):
        return ['actual typed input/actions/output required']
    a, b = raw['inputs'], raw['outputs']
    def require(condition, reason):
        if not condition: errors.append(reason)
    try:
        if index == 0:
            require(b['retiredNavigationCount'] == 0 and type(b['retiredNavigationCount']) is int,
                    'six-tab navigation was not retired')
            require('aria-label="模型配置实体"' not in dom and 'data-service-workspace' in dom,
                    'real service workspace DOM required')
            require([p['value'] for p in b['protocols']] == ['openai_compatible', 'anthropic', 'azure_translator'],
                    'exact existing three protocol choices required')
            review = b['addServiceReview']
            require(_same(review.get('invalidNames'), ['', '   ']) and _same(review.get('disabled'), [True, True]),
                    'empty and whitespace names must disable local creation')
            require(isinstance(review.get('blockedClicks'), list) and len(review['blockedClicks']) == 2
                    and all(isinstance(error, str) and 'not enabled' in error.lower() for error in review['blockedClicks']),
                    'disabled creation must reject actual clicks without force')
            require(review.get('focusReturned') is True and type(review.get('rowCountBefore')) is int
                    and review['rowCountBefore'] > 0 and type(review.get('rowCountAfter')) is int
                    and review['rowCountAfter'] == review['rowCountBefore'],
                    'cancel must restore trigger focus without creating a service')
            commands = b['commands']
            require(len(commands) == 1, 'save must issue exactly one command')
            command = commands[0]['command']
            expected = deepcopy(a['configuration']['draft']); expected['models'][0]['displayName'] = 'SC precise model'
            require(_same(command, {'kind': 'save_configuration', 'payload': {'configuration': expected,
                    'expectedHeadRevision': a['configuration']['draftHeadRevision']}}), 'exact configuration save/CAS payload differs')
            require(b['configuration']['applied'] == a['configuration']['applied'], 'save changed applied configuration')
        elif index == 1:
            require(b['seen'] == ['ui-entered-A', 'ui-entered-B'] and b['orphan'] == 'SC orphan',
                    'both shared models and orphan connection must be reachable')
            require(b['commands'] == [] and b['configuration'] == a['configuration'], 'selection mutated authority')
            require('data-service-connection="sc-orphan"' in dom, 'actual orphan row missing')
        elif index == 2:
            require(b['policy'] == 'round_robin' and b['member'] is True, 'temporary pool edits lost across collapse/service switch')
            require(b['commands'] == [] and b['configuration'] == a['configuration'], 'temporary pool change was saved/applied implicitly')
            require('collapse-switch-B-return-A' in raw['actions'], 'real collapse/service switch action missing')
        elif index == 3:
            require(b['tests'] == [{'revisionId': a['configuration']['draft']['revisionId'], 'objectKind': 'model', 'objectId': 'ui-model-a'}],
                    'explicit exact model test request differs')
            require(b['commands'] == [] and b['configuration'] == a['configuration'], 'failed test saved/applied/mutated configuration')
            require(b['errorVisible'] is True and 'SC explicit test failure' in dom, 'real failure must remain visible')
        elif index == 4:
            require(b['visible'] == {s: True for s in SECTIONS} and all(type(v) is bool for v in b['visible'].values()),
                    'all advanced/purpose/learning/test/terms editors must be reachable')
            require(all('data-configuration-section="' + s + '"' in dom for s in SECTIONS), 'actual mounted disclosure DOM incomplete')
            require(len(b['commands']) == 1 and _same(b['commands'][0]['command'], {'kind': 'set_translation_enabled', 'payload': {'enabled': False}}), 'purpose current-session translation must use exact authorized command')
            require(b['session']['sessionId'] == a['session']['sessionId'] and b['session']['translationEnabled'] is False and b['configuration'] == a['configuration'], 'session translation must preserve configuration authority')
        elif index == 5:
            require(a['session']['state'] == 'active' and a['session']['paused'] is False, 'active input fixture required')
            require(len(b['commands']) == 1 and _same(b['commands'][0]['command'], {'kind': 'set_capture_paused', 'payload': {'paused': True}}),
                    'pause must use exact current-session command')
            require(b['session']['sessionId'] == a['session']['sessionId'] and b['session']['paused'] is True and b['checked'] is False,
                    'pause must project actual session result')
            require(b['configuration'] == a['configuration'], 'pause must not save preferences/configuration')
        elif index == 6:
            require(b['errorChecked'] is True and b['disabled'] is True and b['session'] is None,
                    'pause error/current no-session state must remain truthful')
            require(b['afterLate']['session']['sessionId'] == 'sc-new-session' and b['afterLate']['session']['paused'] is False,
                    'old pause completion contaminated replacement session')
            require(len(b['commands']) == 2 and all(_same(e['command'], {'kind': 'set_capture_paused', 'payload': {'paused': True}}) for e in b['commands']),
                    'pause error/late path command count or payload differs')
            require('replace-session-before-old-ack' in raw['actions'], 'held old session completion action missing')
        elif index == 7:
            require(b['historyPolicy'] == 'always' and type(b['historySwitches']) is int and b['historySwitches'] == 0,
                    'always-save history policy must have no disable switch')
            require(b['openedMainPages'] == ['history'] and b['commands'] == [], 'history must use real openMainPage intent')
            require('data-history-policy="always"' in dom and '始终保存原文、译文及来源' in dom, 'actual history policy DOM missing')
            errors.extend(validate_switch_geometry(b['switchGeometry'], dom))
        elif index == 8:
            require(a['loginFixture']['status'] == 'not_registered' and _same(b['loginRequests'], [True]), 'explicit enable request required')
            require(type(b['loginQueries']) is int and b['loginQueries'] >= a['loginQueries'] + 2, 'setter and explicit read must independently requery system authority')
            require(a['responsePlan']['setter']['status'] == 'enabled' and a['responsePlan']['firstRequery']['status'] == 'requires_approval' and b['afterSet'] == {'status': 'requires_approval', 'text': '等待系统批准'}, 'independent getter must override the distinct setter receipt')
            require(b['loginFixture']['status'] == 'enabled' and b['checked'] is True, 'actual confirmed enabled projection required')
            require('data-login-startup-status="enabled"' in dom, 'enabled DOM projection differs')
        elif index == 9:
            require(b['status'] == 'requires_approval' and b['loginFixture']['status'] == 'requires_approval', 'approval status must not be enabled')
            require(b['statusText'] == '等待系统批准' and b['loginRequests'] == [], 'approval must be explicit and not auto-enabled')
            require('data-login-startup-status="requires_approval"' in dom and '已登记，尚未生效，等待系统批准' in dom, 'actual approval DOM projection and registered-but-inactive explanation required')
        elif index == 10:
            require(a['loginFixture']['status'] == 'enabled' and b['afterRead'] is True and b['afterWrite'] is True,
                    'read/write failures must retain last known state')
            require(_same(b['loginRequests'], [False]) and b['loginFixture']['status'] == 'enabled', 'failed disable must not fabricate system change')
            require(b['loginQueries'] == a['loginQueries'] + 1 and 'SC login write failed' in dom,
                    'query failure and write failure must be actual distinct operations')
        elif index == 11:
            require(b['busyDisabled'] is True and b['rereadDisabled'] is True and b['duplicateAttempt']['blocked'] is True and 'element is not enabled' in b['duplicateAttempt']['error'], 'busy must prevent actual duplicate interaction and requery')
            require(_same(b['loginRequests'], [True]) and b['checked'] is False and b['loginFixture']['status'] == 'not_registered',
                    'late prior-mount setter completion contaminated current state')
            require(b['loginQueries'] > a['loginQueries'] and 'leave-remount-before-late-old-set' in raw['actions'],
                    'actual remount/new system query/held old completion required')
    except (KeyError, TypeError, IndexError, AttributeError) as error:
        errors.append('settings raw behavior incomplete: ' + str(error))
    return errors


def inspect(root, out, nonce, started, completed):
    errors, details = [], {}
    try:
        root, out = Path(root), Path(out)
        report_path = safe_path(out, 'verification.json'); report = load(report_path)
        exact = {'schemaVersion': 1, 'id': 'current-settings-controls-real-react', 'nonce': nonce,
                 'status': 'passed', 'layer': 'real_react_controlled_transport', 'skippedAssertions': 0,
                 'nativeOrGameVerified': False, 'productReleaseApproved': False, 'pixelAcceptance': False,
                 'referenceApproval': 'not_granted'}
        for key, value in exact.items():
            if type(report.get(key)) is not type(value) or report.get(key) != value:
                errors.append('settings typed identity/capability differs: ' + key)
        times = [datetime.fromisoformat(report[k].replace('Z', '+00:00')) for k in ('executedAt', 'finishedAt')]
        if any(t.tzinfo is None for t in times) or not started <= times[0].timestamp() <= times[1].timestamp() <= completed:
            errors.append('settings report outside actual invocation interval')
        if report.get('errors') != [] or report.get('externalRequests') != []: errors.append('settings runtime/network errors')
        sources = report.get('sourceSha256')
        if not isinstance(sources, dict) or sources != source_files(root) or report.get('sourceSha256After') != sources:
            errors.append('settings full current source tree/before-after identity differs')
        actual_csp = load(root / 'apps/desktop/src-tauri/tauri.conf.json')['app']['security']['csp']
        if report.get('productionCsp') != actual_csp: errors.append('production CSP changed or not enforced')
        cases = report.get('checks')
        if not isinstance(cases, list) or tuple(c.get('id') for c in cases) != CASES: raise ValueError('all twelve exact settings cases required')
        total = 0; captures = []
        for case in cases:
            count = case.get('assertionCount')
            if case.get('status') != 'passed' or type(count) is not int or count <= 0: errors.append('actual positive assertions required per settings case')
            else: total += count
            raw_file = safe_path(out, case['raw']['file'])
            if raw_file.name != case['id'] + '.raw.json' or digest(raw_file) != case['raw']['sha256']: errors.append('settings raw identity/hash differs')
            raw = load(raw_file)
            if raw.get('nonce') != nonce: errors.append('settings raw invocation nonce differs')
            capture = case['capture']; captures.append(capture)
            if capture.get('file') != case['id'] + '.candidate.png' or capture.get('domFile') != case['id'] + '.dom.html': errors.append('per-case actual PNG/DOM required')
            png, dom = safe_path(out, capture['file']), safe_path(out, capture['domFile'])
            if (digest(png) != capture.get('sha256') or digest(dom) != capture.get('domSha256')
                    or png_info(png.read_bytes()) != (1160, 900) or capture.get('viewport') != {'width': 1160, 'height': 900}
                    or capture.get('producer') != 'production_react_controlled_transport'
                    or capture.get('approvedReference') is not False or capture.get('nativeVerified') is not False):
                errors.append('actual settings PNG/DOM dimensions/source/hash/capability differs')
            errors.extend(case['id'] + ': ' + e for e in validate_case(case['id'], raw, dom.read_text()))
        if report.get('captures') != captures: errors.append('all twelve unique per-case capture identities required')
        node = str(Path(shutil.which('node')).resolve()); desktop = root / 'apps/desktop'
        commands = [
            [node, str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
            [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--outDir', str(out / 'production-build'), '--sourcemap'],
            [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'), '--outDir', str(out / 'controlled-build'), '--sourcemap'],
        ]
        builds = report.get('builds')
        if not isinstance(builds, list) or len(builds) != 3: raise ValueError('actual typecheck/production/controlled builds required')
        # Each fixed build contributes one separately executed assertion.
        # The producer cannot inflate the total or omit these build assertions.
        if type(report.get('assertionCount')) is not int or report['assertionCount'] != total + len(commands):
            errors.append('settings assertion total must equal exact cases plus fixed builds')
        for build, command, name in zip(builds, commands, ('typecheck-tests.json', 'production-build.json', 'component-build.json')):
            log = load(safe_path(out, name)); actual = build.get('command', [])
            if actual: actual = [str(Path(actual[0]).resolve()), *actual[1:]]
            if (actual != command or build.get('log') != name or log.get('command') != build.get('command')
                    or type(build.get('exitCode')) is not int or build['exitCode'] != 0 or log.get('exitCode') != 0):
                errors.append('settings fixed actual build command/log differs')
        for folder in ('production-build', 'controlled-build'):
            found = set()
            for file in (out / folder).rglob('*.js.map'):
                m = load(file)
                if len(m.get('sources', [])) != len(m.get('sourcesContent', [])): raise ValueError('complete actual build sourcemap required')
                for name, content in zip(m['sources'], m['sourcesContent']):
                    for source in COMPONENTS:
                        if name.endswith('/' + source.removeprefix('apps/desktop/')):
                            if safe_path(root, source).read_text() != content: errors.append('built production component content differs: ' + source)
                            else: found.add(source)
            if found != COMPONENTS: errors.append('actual production component graph incomplete: ' + folder)
        details = {'test_count': len(cases), 'assertion_count': report['assertionCount'],
                   'component_receipt': report_path.relative_to(root).as_posix(), 'component_receipt_sha256': digest(report_path),
                   'behavior_status': 'passed' if not errors else 'failed', 'visual_status': 'candidate_only_no_pixel_approval',
                   'product_packaged_native_game_tests_executed': False}
    except (OSError, ValueError, KeyError, TypeError, AttributeError, OverflowError, struct.error, zlib.error) as error:
        errors.append('settings component evidence invalid: ' + str(error))
    return errors, details
