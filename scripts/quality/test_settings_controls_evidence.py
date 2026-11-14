"""Positive and adversarial tests for independently rechecked settings evidence."""
from copy import deepcopy
import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest
import zlib

import settings_controls_evidence as gate


def switch_geometry_fixture():
    def rectangle(left, top, width, height):
        return {'left': left, 'top': top, 'right': left + width, 'bottom': top + height, 'width': width, 'height': height}
    rows, labels = [], []
    for index, name in enumerate(sorted(gate.DESCRIBED_SWITCHES)):
        y = index * 80; description_id = 'switch-description-' + str(index); description = '说明 ' + name
        rows.append({'name': name, 'title': name, 'description': description, 'descriptionId': description_id,
                     'describedBy': [description_id], 'descriptionLinked': True,
                     'strong': rectangle(10, y + 10, 250, 20), 'small': rectangle(10, y + 35, 300, 18),
                     'track': rectangle(340, y + 20, 32, 19)})
        labels.append(f'<label><input role="switch" aria-label="{name}" aria-describedby="{description_id}">'
                      f'<span data-switch-label><strong>{name}</strong><small id="{description_id}">{description}</small></span>'
                      '<span class="switch-track"><span></span></span></label>')
    return rows, '<div data-ui-page="general-settings">' + ''.join(labels) + '</div>'


def fixture(index):
    configuration = {'draftHeadRevision': 4, 'applied': None,
                     'draft': {'revisionId': 'saved-revision', 'connections': [],
                               'models': [{'displayName': 'original'}], 'bindings': [], 'templates': [], 'termSets': []}}
    before = {'configuration': configuration, 'commands': [], 'session': {'sessionId': 'original-session', 'state': 'active', 'paused': False, 'translationEnabled': True},
              'loginFixture': {'status': 'not_registered'}, 'loginQueries': 1, 'loginRequests': []}
    after = deepcopy(before); dom = '<body data-service-workspace></body>'
    actions = ['actual-user-action'] * gate.MIN_ACTIONS[index]
    if index == 0:
        saved = deepcopy(configuration['draft']); saved['models'][0]['displayName'] = 'SC precise model'
        after.update(addServiceReview={'invalidNames':['','   '],'disabled':[True,True],'blockedClicks':['element is not enabled','element is not enabled'],'focusReturned':True,'rowCountBefore':2,'rowCountAfter':2},retiredNavigationCount=0, protocols=[{'value': value} for value in ('openai_compatible', 'anthropic', 'azure_translator')],
                     commands=[{'command': {'kind': 'save_configuration', 'payload': {'configuration': saved, 'expectedHeadRevision': 4}}}])
    elif index == 1:
        after.update(seen=['ui-entered-A', 'ui-entered-B'], orphan='SC orphan'); dom += '<button data-service-connection="sc-orphan"></button>'
    elif index == 2:
        after.update(policy='round_robin', member=True); actions[-1] = 'collapse-switch-B-return-A'
    elif index == 3:
        after.update(tests=[{'revisionId': 'saved-revision', 'objectKind': 'model', 'objectId': 'ui-model-a'}], errorVisible=True)
        dom += '<p role="alert">SC explicit test failure</p>'
    elif index == 4:
        after['commands'] = [{'command': {'kind': 'set_translation_enabled', 'payload': {'enabled': False}}}]; after['session']['translationEnabled'] = False
        after['visible'] = {s: True for s in gate.SECTIONS}; dom += ''.join('<details data-configuration-section="' + s + '"></details>' for s in gate.SECTIONS)
    elif index == 5:
        after.update(commands=[{'command': {'kind': 'set_capture_paused', 'payload': {'paused': True}}}], checked=False)
        after['session']['paused'] = True
    elif index == 6:
        after.update(errorChecked=True, disabled=True, session=None,
                     afterLate={'session': {'sessionId': 'sc-new-session', 'paused': False}},
                     commands=[{'command': {'kind': 'set_capture_paused', 'payload': {'paused': True}}} for _ in range(2)])
        actions[-1] = 'replace-session-before-old-ack'
    elif index == 7:
        after.update(historyPolicy='always', historySwitches=0, openedMainPages=['history'])
        after['switchGeometry'], switch_dom = switch_geometry_fixture()
        dom += '<div data-history-policy="always">始终保存原文、译文及来源</div>' + switch_dom
    elif index == 8:
        before['responsePlan'] = {'setter': {'status': 'enabled'}, 'firstRequery': {'status': 'requires_approval'}}
        after.update(loginRequests=[True], loginQueries=3, loginFixture={'status': 'enabled'}, checked=True, afterSet={'status': 'requires_approval', 'text': '等待系统批准'})
        dom += '<p data-login-startup-status="enabled"></p>'
    elif index == 9:
        before['loginFixture']['status'] = 'requires_approval'
        after.update(status='requires_approval', loginFixture={'status': 'requires_approval'}, statusText='等待系统批准')
        dom += '<p data-login-startup-status="requires_approval">等待系统批准</p><small>已登记，尚未生效，等待系统批准</small>'
    elif index == 10:
        before['loginFixture']['status'] = 'enabled'
        after.update(afterRead=True, afterWrite=True, loginFixture={'status': 'enabled'}, loginRequests=[False], loginQueries=2)
        dom += '<p role="alert">SC login write failed</p>'
    elif index == 11:
        after.update(busyDisabled=True, rereadDisabled=True, duplicateAttempt={'blocked': True, 'error': 'element is not enabled'}, checked=False, loginRequests=[True], loginQueries=2)
        actions[-1] = 'leave-remount-before-late-old-set'
    return {'schemaVersion': 1, 'id': gate.CASES[index], 'nonce': 'test-invocation', 'inputs': before, 'actions': actions, 'outputs': after}, dom


class SettingsControlsEvidenceTests(unittest.TestCase):
    def test_new_service_empty_names_cannot_be_enabled_or_created(self):
        for key,value in [('invalidNames',['','name']),('disabled',[True,False]),('disabled',[1,1]),('blockedClicks',['forced click','not enabled']),('focusReturned',False),('rowCountAfter',3)]:
            with self.subTest(field=key,value=value):
                raw,dom=fixture(0)
                raw['outputs']['addServiceReview'][key]=value
                self.assertTrue(gate.validate_case(gate.CASES[0],raw,dom))

    def test_all_twelve_finite_oracles_accept_complete_actual_shapes(self):
        for index, case in enumerate(gate.CASES):
            with self.subTest(case=case):
                raw, dom = fixture(index); self.assertEqual(gate.validate_case(case, raw, dom), [])

    def reject(self, index, mutate):
        raw, dom = fixture(index); replacement = mutate(raw, dom)
        self.assertTrue(gate.validate_case(gate.CASES[index], raw, replacement if isinstance(replacement, str) else dom))

    def test_summary_passed_without_raw_is_rejected(self):
        for case in gate.CASES:
            with self.subTest(case=case): self.assertTrue(gate.validate_case(case, {'status': 'passed'}, '<body/>'))

    def test_missing_original_inputs_rejected(self):
        self.reject(0, lambda raw, dom: raw.pop('inputs'))

    def test_missing_actual_actions_rejected(self):
        self.reject(2, lambda raw, dom: raw.update(actions=[]))

    def test_unknown_case_rejected(self):
        self.assertTrue(gate.validate_case('new-unreviewed-case', {}, ''))

    def test_exact_save_rejects_unrelated_model_mutation(self):
        self.reject(0, lambda raw, dom: raw['outputs']['commands'][0]['command']['payload']['configuration'].update(bindings=[{'enabled': True}]))

    def test_save_original_cas_identity_cannot_be_refreshed(self):
        self.reject(0, lambda raw, dom: raw['outputs']['commands'][0]['command']['payload'].update(expectedHeadRevision=5))

    def test_save_cannot_apply_configuration(self):
        self.reject(0, lambda raw, dom: raw['outputs']['configuration'].update(applied={'revisionId': 'unexpected'}))

    def test_retired_navigation_cannot_be_hidden_in_dom(self):
        self.reject(0, lambda raw, dom: dom + '<div hidden aria-label="模型配置实体"></div>')

    def test_unimplemented_provider_registry_not_claimed_as_three_protocols(self):
        self.reject(0, lambda raw, dom: raw['outputs']['protocols'].append({'value': 'unimplemented'}))

    def test_orphan_connection_must_have_actual_row(self):
        self.reject(1, lambda raw, dom: '<body/>')

    def test_both_models_must_be_independently_reachable(self):
        self.reject(1, lambda raw, dom: raw['outputs'].update(seen=['ui-entered-A', 'ui-entered-A']))

    def test_pool_member_loss_is_rejected(self):
        self.reject(2, lambda raw, dom: raw['outputs'].update(member=False))

    def test_pool_draft_cannot_implicitly_issue_save(self):
        self.reject(2, lambda raw, dom: raw['outputs'].update(commands=[{'command': {'kind': 'save_configuration'}}]))

    def test_model_test_cannot_be_connection_enumeration(self):
        self.reject(3, lambda raw, dom: raw['outputs']['tests'][0].update(objectKind='connection'))

    def test_model_test_cannot_target_other_model(self):
        self.reject(3, lambda raw, dom: raw['outputs']['tests'][0].update(objectId='ui-model-b'))

    def test_failure_requires_real_visible_error_dom(self):
        self.reject(3, lambda raw, dom: '<body/>')

    def test_missing_learning_editor_rejected(self):
        self.reject(4, lambda raw, dom: raw['outputs']['visible'].update(learning=False))

    def test_pause_must_not_patch_preference(self):
        self.reject(5, lambda raw, dom: raw['outputs']['commands'][0].update(command={'kind': 'save_preferences', 'payload': {'paused': True}}))

    def test_boolean_command_payload_cannot_be_numeric_one(self):
        self.reject(5, lambda raw, dom: raw['outputs']['commands'][0]['command']['payload'].update(paused=1))

    def test_boolean_translation_payload_cannot_be_numeric_zero(self):
        self.reject(4, lambda raw, dom: raw['outputs']['commands'][0]['command']['payload'].update(enabled=0))

    def test_boolean_login_request_cannot_be_numeric_one(self):
        self.reject(8, lambda raw, dom: raw['outputs'].update(loginRequests=[1]))

    def test_pause_payload_must_not_resume(self):
        self.reject(5, lambda raw, dom: raw['outputs']['commands'][0]['command']['payload'].update(paused=False))

    def test_late_pause_must_not_mutate_new_session(self):
        self.reject(6, lambda raw, dom: raw['outputs']['afterLate']['session'].update(paused=True))

    def test_no_session_must_be_disabled(self):
        self.reject(6, lambda raw, dom: raw['outputs'].update(disabled=False))

    def test_history_off_switch_rejected(self):
        self.reject(7, lambda raw, dom: raw['outputs'].update(historySwitches=1))

    def test_history_navigation_requires_real_intent(self):
        self.reject(7, lambda raw, dom: raw['outputs'].update(openedMainPages=[]))

    def test_descriptions_require_actual_separate_line_geometry(self):
        self.reject(7, lambda raw, dom: raw['outputs']['switchGeometry'][0]['small'].update(top=20, bottom=38))

    def test_switch_track_cannot_cover_text(self):
        self.reject(7, lambda raw, dom: raw['outputs']['switchGeometry'][0]['track'].update(left=20, right=52))

    def test_switch_description_aria_link_cannot_be_removed(self):
        self.reject(7, lambda raw, dom: raw['outputs']['switchGeometry'][0].update(describedBy=[]))

    def test_switch_description_raw_id_must_match_actual_dom(self):
        self.reject(7, lambda raw, dom: dom.replace('aria-describedby="switch-description-0"', 'aria-describedby="other-description"'))

    def test_switch_description_cannot_be_replaced_by_another_text(self):
        self.reject(7, lambda raw, dom: raw['outputs']['switchGeometry'][0].update(description='unrelated description'))

    def test_all_described_switches_must_have_geometry(self):
        self.reject(7, lambda raw, dom: raw['outputs']['switchGeometry'].pop())

    def test_duplicate_switch_geometry_cannot_hide_missing_switch(self):
        self.reject(7, lambda raw, dom: raw['outputs']['switchGeometry'].__setitem__(1, deepcopy(raw['outputs']['switchGeometry'][0])))

    def test_zero_text_area_is_rejected(self):
        rows, dom = switch_geometry_fixture(); rows[0]['strong'].update(width=0, right=10)
        with self.assertRaises(ValueError): gate.validate_switch_geometry(rows, dom)

    def test_nonfinite_switch_geometry_is_rejected(self):
        rows, dom = switch_geometry_fixture(); rows[0]['small']['top'] = float('nan')
        with self.assertRaises(ValueError): gate.validate_switch_geometry(rows, dom)

    def test_description_same_line_is_rejected_even_with_positive_area(self):
        rows, dom = switch_geometry_fixture(); rows[0]['small'].update(top=10, bottom=28)
        self.assertTrue(gate.validate_switch_geometry(rows, dom))

    def test_login_set_without_requery_rejected(self):
        self.reject(8, lambda raw, dom: raw['outputs'].update(loginQueries=1))

    def test_ignoring_independent_getter_result_is_rejected(self):
        self.reject(8, lambda raw, dom: raw['outputs'].update(afterSet={'status': 'enabled', 'text': '已启用'}))

    def test_login_intention_cannot_be_confirmed_enabled(self):
        self.reject(8, lambda raw, dom: raw['outputs']['loginFixture'].update(status='not_registered'))

    def test_requires_approval_cannot_be_labelled_enabled(self):
        self.reject(9, lambda raw, dom: raw['outputs'].update(statusText='已启用'))

    def test_failed_read_must_keep_last_known_state(self):
        self.reject(10, lambda raw, dom: raw['outputs'].update(afterRead=False))

    def test_failed_write_must_not_change_known_state(self):
        self.reject(10, lambda raw, dom: raw['outputs']['loginFixture'].update(status='not_registered'))

    def test_busy_duplicate_login_request_rejected(self):
        self.reject(11, lambda raw, dom: raw['outputs'].update(loginRequests=[True, True]))

    def test_late_login_result_cannot_contaminate_new_mount(self):
        self.reject(11, lambda raw, dom: raw['outputs'].update(checked=True))

    def test_duplicate_json_keys_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'raw.json'; path.write_text('{"status":"failed","status":"passed"}')
            with self.assertRaises(ValueError): gate.load(path)

    def test_non_json_numbers_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'raw.json'; path.write_text('{"assertionCount":NaN}')
            with self.assertRaises(ValueError): gate.load(path)


class SettingsControlsInspectionTests(unittest.TestCase):
    """Synthetic guard fixture; never a product adapter result or approved PNG."""
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve(); self.out = self.root / '.build/guard-fixture'; self.out.mkdir(parents=True)
        for source in gate.COMPONENTS | {gate.RUNNER}:
            file = self.root / source; file.parent.mkdir(parents=True, exist_ok=True); file.write_text('guard fixture ' + source)
        config = self.root / 'apps/desktop/src-tauri/tauri.conf.json'; config.parent.mkdir(parents=True)
        config.write_text(json.dumps({'app': {'security': {'csp': "default-src 'self'"}}}))
        def chunk(tag, data): return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data))
        png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1160, 900, 8, 6, 0, 0, 0))
        png += chunk(b'IDAT', zlib.compress((b'\0' + b'\0' * (1160 * 4)) * 900)) + chunk(b'IEND', b'')
        checks, captures = [], []
        for index, case in enumerate(gate.CASES):
            raw, dom = fixture(index); self.write(case + '.raw.json', raw)
            (self.out / (case + '.dom.html')).write_text(dom); (self.out / (case + '.candidate.png')).write_bytes(png)
            capture = {'file': case + '.candidate.png', 'sha256': gate.digest(self.out / (case + '.candidate.png')),
                       'domFile': case + '.dom.html', 'domSha256': gate.digest(self.out / (case + '.dom.html')),
                       'viewport': {'width': 1160, 'height': 900}, 'producer': 'production_react_controlled_transport',
                       'approvedReference': False, 'nativeVerified': False}
            captures.append(capture); checks.append({'id': case, 'status': 'passed', 'assertionCount': 1, 'capture': capture,
                        'raw': {'file': case + '.raw.json', 'sha256': gate.digest(self.out / (case + '.raw.json'))}})
        desktop = self.root / 'apps/desktop'; node = str(Path(shutil.which('node')).resolve())
        commands = [
            [node, str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
            [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--outDir', str(self.out / 'production-build'), '--sourcemap'],
            [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'), '--outDir', str(self.out / 'controlled-build'), '--sourcemap']]
        builds = []
        for command, name in zip(commands, ('typecheck-tests.json', 'production-build.json', 'component-build.json')):
            self.write(name, {'command': command, 'exitCode': 0, 'stdout': 'guard fixture', 'stderr': ''})
            builds.append({'command': command, 'exitCode': 0, 'log': name})
        for folder in ('production-build', 'controlled-build'):
            (self.out / folder).mkdir()
            self.write(folder + '/component.js.map', {'sources': ['../../../' + p.removeprefix('apps/desktop/') for p in gate.COMPONENTS],
                       'sourcesContent': [(self.root / p).read_text() for p in gate.COMPONENTS]})
        self.report = {'schemaVersion': 1, 'id': 'current-settings-controls-real-react', 'nonce': 'test-invocation',
                       'status': 'passed', 'executedAt': '2026-11-05T00:00:00Z', 'finishedAt': '2026-11-05T00:00:01Z',
                       'layer': 'real_react_controlled_transport', 'skippedAssertions': 0, 'assertionCount': 15,
                       'nativeOrGameVerified': False, 'productReleaseApproved': False, 'pixelAcceptance': False,
                       'referenceApproval': 'not_granted', 'errors': [], 'externalRequests': [],
                       'checks': checks, 'captures': captures, 'builds': builds, 'productionCsp': "default-src 'self'",
                       'sourceSha256': gate.source_files(self.root), 'sourceSha256After': gate.source_files(self.root)}

    def write(self, relative, value): (self.out / relative).write_text(json.dumps(value))

    def inspect(self):
        self.write('verification.json', self.report)
        return gate.inspect(self.root, self.out, 'test-invocation', 1793836799, 1793836802)[0]

    def test_inflated_total_is_rejected_even_with_all_passed_cases(self):
        self.report['assertionCount'] += 1
        self.assertTrue(self.inspect())

    def test_deflated_total_is_rejected_even_when_it_covers_all_case_counts(self):
        self.report['assertionCount'] -= 1
        self.assertTrue(self.inspect())

    def test_build_assertions_cannot_be_omitted_from_total(self):
        self.report['assertionCount'] = sum(row['assertionCount'] for row in self.report['checks'])
        self.assertTrue(self.inspect())

    def test_complete_synthetic_guard_evidence_is_valid(self): self.assertEqual(self.inspect(), [])

    def test_summary_only_passed_report_is_rejected(self):
        self.report['checks'] = [{'id': case, 'status': 'passed', 'assertionCount': 1} for case in gate.CASES]
        self.assertTrue(self.inspect())

    def test_replayed_nonce_rejected(self):
        self.report['nonce'] = 'other-invocation'; self.assertTrue(self.inspect())

    def test_outside_actual_invocation_rejected(self):
        self.report['finishedAt'] = '2026-11-06T00:00:01Z'; self.assertTrue(self.inspect())

    def test_boolean_count_cannot_masquerade_as_one(self):
        self.report['checks'][0]['assertionCount'] = True; self.assertTrue(self.inspect())

    def test_missing_case_rejected(self):
        self.report['checks'].pop(); self.assertTrue(self.inspect())

    def test_duplicate_case_rejected(self):
        self.report['checks'][1] = deepcopy(self.report['checks'][0]); self.assertTrue(self.inspect())

    def test_source_tree_drift_rejected(self):
        (self.root / gate.RUNNER).write_text('changed after build'); self.assertTrue(self.inspect())

    def test_before_after_source_mismatch_rejected(self):
        self.report['sourceSha256After'] = {}; self.assertTrue(self.inspect())

    def test_weakened_csp_rejected(self):
        self.report['productionCsp'] = "default-src * 'unsafe-eval'"; self.assertTrue(self.inspect())

    def test_raw_hash_tamper_rejected(self):
        self.write(gate.CASES[0] + '.raw.json', {'status': 'passed'}); self.assertTrue(self.inspect())

    def test_correct_hash_cannot_bypass_semantic_oracle(self):
        case = gate.CASES[7]; raw = gate.load(self.out / (case + '.raw.json')); raw['outputs']['historySwitches'] = 1
        self.write(case + '.raw.json', raw); self.report['checks'][7]['raw']['sha256'] = gate.digest(self.out / (case + '.raw.json'))
        self.assertTrue(self.inspect())

    def test_dom_tamper_rejected(self):
        (self.out / (gate.CASES[0] + '.dom.html')).write_text('different DOM'); self.assertTrue(self.inspect())

    def test_corrupt_png_fails_closed(self):
        file = self.out / (gate.CASES[0] + '.candidate.png'); file.write_bytes(b'broken PNG')
        self.report['checks'][0]['capture']['sha256'] = gate.digest(file); self.assertTrue(self.inspect())

    def test_build_command_cannot_be_arbitrary_passed_script(self):
        self.report['builds'][0]['command'] = ['node', 'fake-pass.js']; self.assertTrue(self.inspect())

    def test_failed_build_cannot_be_hidden(self):
        self.report['builds'][1]['exitCode'] = 1; self.assertTrue(self.inspect())

    def test_old_built_component_content_rejected(self):
        file = 'controlled-build/component.js.map'; value = gate.load(self.out / file); value['sourcesContent'][0] = 'old source'
        self.write(file, value); self.assertTrue(self.inspect())

    def test_missing_actual_component_graph_rejected(self):
        (self.out / 'production-build/component.js.map').unlink(); self.assertTrue(self.inspect())

    def test_native_capability_cannot_be_promoted(self):
        self.report['nativeOrGameVerified'] = True; self.assertTrue(self.inspect())

    def test_pixel_capability_cannot_be_promoted(self):
        self.report['pixelAcceptance'] = True; self.assertTrue(self.inspect())


if __name__ == '__main__': unittest.main()
