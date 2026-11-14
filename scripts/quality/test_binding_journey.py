"""Executable guard negatives, independent from finite product diagnostics."""
import copy
from datetime import datetime, timezone
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zlib

import binding_journey as j
import binding_journey_inspect as i
import binding_journey_recompute as r
from binding_phase_guard import ref, tree_sha
from gate_core import source_files


def pixel_png():
    def chunk(name, data):
        return struct.pack('>I', len(data)) + name + data + struct.pack('>I', zlib.crc32(name + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(b'\x00\x00\x00\x00\xff')) + chunk(b'IEND', b''))


class BindingJourneyGuards(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def write(self, path, value):
        p = self.root / path
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(value))
        return p

    def observations(self):
        return [
            {'selectionButton': {'label': '选择对应的游戏窗口…', 'visible': True, 'enabled': True},
             'visibleText': '需要在系统选择对应游戏窗口', 'imageCount': 0, 'timeText': '尚无成功画面'},
            {'sameTargetSwitchVisible': False, 'visibleText': '复用现有连接'},
            {'stageLabels': ['窗口已关联', '自动接入', '选择正文'], 'pending': True, 'duplicateSubmitAvailable': False},
            {'sourceCheckboxCount': 2, 'beginReadingButton': {'visible': True},
             'draftClickCommandCount': 0, 'dialogStillOpen': True,
             'draftWindowEffectCount': 0, 'draftAssociationCount': 0},
            {'sourceFieldLabels': [], 'expandedText': '全文样本', 'sampleText': '全文样本',
             'longSampleRetained': True, 'productionDetailFixtureAvailable': False},
            {'rememberCheckbox': {'visible': True, 'enabled': True}, 'gameId': 'actual-owned-game'},
            {'pickerCallsBeforeIntent': 0, 'pickerCallsAfterIntent': 1, 'associationCallsBeforeIntent': 0}]

    def test_finite_oracles_do_not_grant_typed_details_or_native(self):
        for identity, obs in zip(j.COMPONENT_CASES, self.observations()):
            with self.subTest(identity=identity):
                self.assertEqual(j.observation_verdict(identity, obs)['status'], 'blocked' if identity == j.COMPONENT_CASES[4] else 'passed')
        value = j.observation_verdict(j.COMPONENT_CASES[4], self.observations()[4])
        self.assertEqual(len(value['blocked_obligations']), 2)

    def test_each_actual_ui_regression_is_a_failure(self):
        observations = self.observations()
        observations[0]['selectionButton']['label'] = '重新选择…'
        observations[1]['sameTargetSwitchVisible'] = True
        observations[2]['stageLabels'] = []
        observations[3]['draftClickCommandCount'] = 1
        observations[5]['rememberCheckbox']['visible'] = False
        for number in (0, 1, 2, 3, 5):
            self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[number], observations[number])['status'], 'failed')

    def test_bool_and_integers_cannot_impersonate_each_other(self):
        obs = self.observations()[6]
        obs['pickerCallsBeforeIntent'] = False
        with self.assertRaises(ValueError):
            j.observation_verdict(j.COMPONENT_CASES[6], obs)
        obs = self.observations()[1]
        obs['sameTargetSwitchVisible'] = 0
        with self.assertRaises(ValueError):
            j.observation_verdict(j.COMPONENT_CASES[1], obs)

    def test_unrelated_u64_text_cannot_prove_typed_source_detail(self):
        obs = self.observations()[4]
        obs['expandedText'] += ' 18446744073709551615'
        value = j.observation_verdict(j.COMPONENT_CASES[4], obs)
        self.assertEqual(len(value['assertions']), 1)
        self.assertIn('source-u64-typed-DTO-projection', value['blocked_obligations'])

    def test_claimed_detail_without_actual_rows_is_rejected(self):
        obs = self.observations()[4]
        obs['productionDetailFixtureAvailable'] = True
        with self.assertRaises(ValueError):
            j.observation_verdict(j.COMPONENT_CASES[4], obs)

    def test_actual_diff_forces_binding_checks_without_metadata(self):
        for path in ('apps/desktop/src/features/capture/TargetSheet.tsx',
                     'apps/desktop/src/bridge/tauri-port.ts',
                     'apps/desktop/src-tauri/src/platform/macos/window_capture.rs',
                     'apps/desktop/src-tauri/src/platform/macos/bridge.m',
                     'apps/desktop/src-tauri/src/platform/macos/mod.rs',
                     'apps/desktop/src-tauri/src/platform/window_manager.rs',
                     'apps/desktop/src-tauri/src/platform/window_geometry.rs',
                     'crates/app-infra/src/storage/queries/source_selections.rs',
                     'crates/app-core/src/contracts/binding.rs'):
            self.assertIn(j.CHECK_ID, j.required_checks('innocent-tooling-label', [path]))
        self.assertIn(j.PHASE_CHECK, j.required_checks(j.CHANGE, []))
        self.assertIn(j.CHECK_ID, j.required_checks('empty-release', [], 'release'))
        self.assertEqual(j.required_checks('unrelated-docs', ['docs/help.md']), frozenset())
        for path in ('crates/app-infra/src/storage/migrations/005_source_selection_sets.sql',
                     'docs/architecture/database/v5/schema-contract.json',
                     'crates/app-infra/tests/source_selection_sets.rs'):
            self.assertTrue(j.STORAGE_V5_CHECKS <= j.required_checks('innocent-metadata', [path]))

    def test_schema_five_check_names_are_the_frozen_plan_and_executable_root_names(self):
        import verify
        metadata = json.loads((j.ROOT / 'openspec/changes' / j.CHANGE / 'verification.json').read_text())
        declared = {check for row in metadata['requirements'] for check in row['checks']}
        self.assertTrue(j.STORAGE_V5_CHECKS <= verify.CHECK_IDS)
        self.assertTrue(j.STORAGE_V5_CHECKS <= declared)
        self.assertEqual(verify.TEST_CHECKS['storage-schema-v5-tests'], ('python', 'test_storage_schema_v5'))
        # Ignore all caller-declared checks: actual storage dependency selection
        # still produces the complete four code-owned obligations.
        self.assertTrue(j.STORAGE_V5_CHECKS <= j.required_checks('no-declared-checks',
                        ['crates/app-infra/src/storage/migrations/005_source_selection_sets.sql']))

    def test_schema_five_actual_callee_accepts_ref_and_runner_context_before_rejecting_fake_producer(self):
        import storage_schema_v5 as schema
        # This is a protocol wiring guard, deliberately not a production proof.
        # The real callee must read the ref and accept the runner's exact context
        # shape, then reject the deliberately nonexistent/stale actual producer.
        with tempfile.TemporaryDirectory(prefix='binding-v5-interface-', dir=j.ROOT / '.build') as directory:
            base = Path(directory)
            environment = {'LUNA_BINDING_OUTPUT': str(base), 'LUNA_BINDING_NONCE': 'a' * 32,
                           'LUNA_BINDING_SOURCE_TREE': 'b' * 64}
            context = {'nonce': 'a' * 32, 'source_tree_sha256': 'b' * 64, 'artifact_sha256': 'c' * 64,
                       'environment_sha256': schema.canonical_hash(environment), 'started_at_ms': 0, 'finished_at_ms': 1,
                       'test_names': list(schema.TEST_NAMES), 'executions': [],
                       'producer_source': schema.PRODUCTION_SOURCE, 'binary': {'path': 'not-real', 'sha256': 'c' * 64}}
            body = {'schema_version': 1, 'kind': schema.RUNTIME_KIND,
                    'nonce': context['nonce'], 'source_tree_sha256': context['source_tree_sha256'],
                    'artifact_sha256': context['artifact_sha256'], 'environment': environment,
                    'started_at_ms': 0, 'finished_at_ms': 1,
                    'producer': {'package': 'app-infra', 'target': 'source_selection_sets',
                                 'tests': list(schema.TEST_NAMES), 'source_sha256': '0' * 64,
                                 'binary': context['binary']},
                    'snapshots': {}, 'routes': [], 'field_facts': [], 'relationship_facts': [],
                    'old_writer': None, 'sqlite': {}, 'host': {}, 'route_trace': None,
                    'old_suite': None, 'selection_snapshot': None, 'selection_rows': []}
            candidate = base / schema.RUNTIME_FILENAME
            candidate.write_text(json.dumps(body))
            actual = schema.inspect_runtime(j.ROOT, ref(j.ROOT, candidate.relative_to(j.ROOT).as_posix()), context=context)
            self.assertEqual(actual['status'], 'failed')
            self.assertTrue(any('production test source is stale' in error for error in actual['errors']), actual)
            self.assertFalse(actual['product_migration_executed'])

    def test_actual_rust_libtest_framing_reaches_node_rpc_and_python_in_both_thread_modes(self):
        # A std-only neutral guard executable, never a production fixture writer.
        base = Path(tempfile.mkdtemp(prefix='libtest-protocol-guard-', dir=j.ROOT / '.build'))
        source, binary = base / 'probe.rs', base / 'probe'
        source.write_text(r'''#[test]
fn bj_08_storage() {
    assert_eq!(std::env::var("LUNA_BINDING_NONCE").unwrap().len(), 32);
    println!("LUNA_STORAGE_V5_EVENT {{\"event\":\"neutral_guard\"}}");
}
#[test] #[ignore]
fn bj_component_transport() {
    use std::io::Read;
    let message = r#"{"type":"ready","schema_version":1,"nonce":"@NONCE@","source_tree_sha256":"@SOURCE@","capabilities":{"scene_ids":["BJS-01-library-window"],"methods":["snapshot"],"controls":["reset"]}}"#
       .replace("@NONCE@", &std::env::var("LUNA_BINDING_NONCE").unwrap())
       .replace("@SOURCE@", &std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap());
    println!("LUNA_BINDING_RPC_V1 {}", message);
    std::io::stdin().read_to_end(&mut Vec::new()).unwrap();
}
''')
        command = ['rustc', '--edition=2021', '--test', str(source), '-o', str(binary)]
        compiled = subprocess.run(command, capture_output=True, text=True, timeout=60)
        (base / 'compile.stdout.log').write_text(compiled.stdout)
        (base / 'compile.stderr.log').write_text(compiled.stderr)
        self.assertEqual(compiled.returncode, 0, compiled.stderr)
        script = ("import {ProductionRpc} from './scripts/report/verify_binding_journey.mjs';"
                  "const rows=[];for(const mode of [null,'1']){if(mode===null)delete process.env.RUST_TEST_THREADS;else process.env.RUST_TEST_THREADS=mode;"
                  "const rpc=new ProductionRpc({binary:process.argv[1],nonce:'a'.repeat(32),sourceTree:'b'.repeat(64),cwd:process.cwd()});"
                  "const ready=await rpc.start();await rpc.close();rows.push({mode,ready,stdout:rpc.stdout,stderr:rpc.stderr,"
                  "command:rpc.command,environment:rpc.environment,exit_code:rpc.exitCode,errors:rpc.protocolErrors});}"
                  "process.stdout.write(JSON.stringify(rows),()=>process.exit(0));")
        node_command = ['node', '--input-type=module', '-e', script, str(binary)]
        node = subprocess.run(node_command, cwd=j.ROOT, text=True, capture_output=True, timeout=20)
        (base / 'node.stdout.json').write_text(node.stdout)
        (base / 'node.stderr.log').write_text(node.stderr)
        self.assertEqual(node.returncode, 0, node.stderr)
        results = json.loads(node.stdout)
        for result in results:
            with self.subTest(threads=result['mode']):
                self.assertTrue(result['ready'])
                self.assertEqual(result['errors'], [])
                self.assertTrue(i.exact_test_outcome(result['stdout'], 'bj_component_transport', result['exit_code']))
                payloads = [i.rpc_protocol_payload(line) for line in result['stdout'].splitlines()]
                messages = [i.strict_json_text(value) for value in payloads if value is not None]
                self.assertEqual(messages, [result['ready']])
                environment = dict(j.os.environ, LUNA_BINDING_NONCE='a' * 32)
                if result['mode'] is None:
                    environment.pop('RUST_TEST_THREADS', None)
                else:
                    environment['RUST_TEST_THREADS'] = result['mode']
                call = [str(binary), '--exact', 'bj_08_storage', '--nocapture']
                actual = subprocess.run(call, env=environment, capture_output=True, text=True, timeout=20)
                (base / ('storage-' + str(result['mode']) + '.stdout.log')).write_text(actual.stdout)
                (base / ('storage-' + str(result['mode']) + '.stderr.log')).write_text(actual.stderr)
                self.assertTrue(i.exact_test_outcome(actual.stdout + '\n' + actual.stderr, 'bj_08_storage', actual.returncode))
        (base / 'execution.json').write_text(json.dumps({'kind': 'guard-only-real-libtest-protocol/v1',
             'product_execution': False, 'recorded_at': datetime.now(timezone.utc).isoformat(),
             'compile_command': command, 'node_command': node_command, 'results': results}, ensure_ascii=False, indent=2))

    def test_exact_test_summary_rejects_empty_ignored_failed_and_wrong_named_results(self):
        valid = 'running 1 test\ntest bj_08_storage ... marker\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out;\n'
        self.assertTrue(i.exact_test_outcome(valid, 'bj_08_storage', 0))
        for raw, code in ((valid.replace('1 passed', '0 passed'), 0),
                          (valid.replace('0 ignored', '1 ignored'), 0),
                          (valid.replace('bj_08_storage', 'other_test'), 0), (valid, 101), (valid, False),
                          (valid.replace('\nok\n', '\nno-result\n'), 0)):
            with self.assertRaises(ValueError):
                i.exact_test_outcome(raw, 'bj_08_storage', code)
        self.assertIsNone(i.rpc_protocol_payload('arbitrary log LUNA_BINDING_RPC_V1 {}'))

    def plan_fixture(self):
        plan = json.loads((j.ROOT / j.PLAN).read_text())
        self.write(j.PLAN, plan)
        return plan

    def test_registry_exact_18_and_original_obligations_are_preserved(self):
        self.plan_fixture()
        self.assertEqual(j.validate_registry(self.root), [])
        self.assertEqual(set(j.REQUIRED_LAYERS), {'BJ-' + str(x).zfill(2) for x in range(1, 19)})

    def test_removed_case_layer_and_historical_obligations_are_rejected(self):
        original = self.plan_fixture()
        for mutate in (lambda p: p['cases'].pop(),
                       lambda p: p['cases'][0]['required_layers'].remove('native'),
                       lambda p: p['cases'][0].update(planned_status='passed'),
                       lambda p: p['preserved'].update(RBC74_RBC77=False),
                       lambda p: p['preserved'].update(five_negative_scenarios=False)):
            plan = copy.deepcopy(original)
            mutate(plan)
            self.write(j.PLAN, plan)
            self.assertTrue(j.validate_registry(self.root))

    def test_missing_native_and_real_service_cannot_be_cleared_by_finite_passed(self):
        self.plan_fixture()
        result = j.global_assessment(self.root, [{'id': j.CHECK_ID, 'execution': {'status': 'passed', 'cases': []}}])
        self.assertEqual(result['status'], 'blocked')
        self.assertTrue(any(row['layer'] == 'native' for row in result['release_gaps']))
        self.assertTrue(any(row['layer'] == 'service' for row in result['release_gaps']))
        self.assertFalse(result['product_acceptance_promoted'])

    def test_wrong_nonce_old_source_missing_execution_cannot_be_red(self):
        path = self.root / '.build/run'
        path.mkdir(parents=True)
        for body in ({'status': 'passed'}, {'kind': 'binding-journey-invocation/v1', 'schema_version': 1,
                                         'nonce': 'old', 'next_stage_allowed': False}):
            self.write('.build/run/execution.json', body)
            value = i.inspect_execution(self.root, path)
            self.assertFalse(value['eligible_semantic_red'])
            self.assertTrue(value['validation_errors'])

    def test_real_png_decoder_rejects_truncation_crc_and_fake_header(self):
        raw = pixel_png()
        self.assertEqual(i.png_dimensions(raw), (1, 1))
        for candidate in (raw[:33], raw[:-1], raw[:-10] + b'1234567890', b'not image'):
            with self.assertRaises(ValueError):
                i.png_dimensions(candidate)

    def test_dom_stage_matching_ignores_descriptive_substrings(self):
        html = '<html><body><p>关联成功、接入完成、收到候选与选择正文分别显示。</p></body></html>'
        obs = {'visibleText': '关联成功、接入完成、收到候选与选择正文分别显示。', 'stageLabels': []}
        self.assertEqual(r.dom_errors(j.COMPONENT_CASES[2], obs, html, []), [])
        obs['stageLabels'] = ['选择正文']
        self.assertTrue(r.dom_errors(j.COMPONENT_CASES[2], obs, html, []))

    def test_forged_reuse_label_and_frame_claim_conflict_with_dom(self):
        obs = self.observations()[1]
        self.assertTrue(r.dom_errors(j.COMPONENT_CASES[1], obs, '<html><button>切换并接入</button></html>', []))
        obs = self.observations()[0]
        html = '<html><button>选择对应的游戏窗口…</button><p>需要在系统选择对应游戏窗口 尚无成功画面</p><img src="data:image/png;base64,AAAA"></html>'
        self.assertTrue(r.dom_errors(j.COMPONENT_CASES[0], obs, html, []))

    def test_remember_checkbox_is_not_two_source_draft_controls(self):
        obs = self.observations()[3]
        obs.update(beforeFacts={'commands': [], 'transportCalls': [], 'windows': [], 'associations': [], 'events': []},
                   afterFacts={'commands': [], 'transportCalls': [], 'windows': [], 'associations': [], 'events': []})
        html = '<html><div role="dialog"><label>记住选择<input type="checkbox"></label><button>开始阅读</button></div></html>'
        self.assertTrue(r.dom_errors(j.COMPONENT_CASES[3], obs, html, []))

    def test_direct_transport_and_window_effects_cannot_hide_behind_empty_dispatch(self):
        before = {'commands': [], 'transportCalls': [], 'windows': [], 'associations': [], 'events': []}
        after = copy.deepcopy(before)
        after['transportCalls'] = [{'method': 'beginReading'}]
        after['windows'] = ['hud']
        obs = self.observations()[3]
        obs.update(beforeFacts=before, afterFacts=after)
        html = '<html><div role="dialog"><label>A<input type="checkbox"></label><label>B<input type="checkbox"></label><button>开始阅读</button></div></html>'
        errors = r.dom_errors(j.COMPONENT_CASES[3], obs, html, [])
        self.assertTrue(any('transport commit' in x for x in errors))
        self.assertTrue(any('effects' in x for x in errors))

    def test_missing_registered_rust_targets_are_blocked_not_test_pass(self):
        source = source_files(self.root)
        reference = j.execute_rust_layers(self.root, '.build/rust', 'a' * 32, source)
        result = i.inspect_rust_layers(self.root, reference, 'a' * 32, source, replay=True)
        self.assertTrue(result['results'])
        self.assertTrue(all(row['status'] == 'blocked' for row in result['results']))
        self.assertEqual(result['infrastructure_errors'], [])
        body = json.loads((self.root / reference['path']).read_text())
        body['layers'][0]['status'] = 'passed'
        self.write(reference['path'], body)
        with self.assertRaises(ValueError):
            i.inspect_rust_layers(self.root, ref(self.root, reference['path']), 'a' * 32, source)

    def test_rust_layer_inventory_is_code_owned_and_duplicate_layers_rejected(self):
        reference = j.execute_rust_layers(self.root, '.build/rust', 'a' * 32, source_files(self.root))
        body = json.loads((self.root / reference['path']).read_text())
        body['layers'].append(copy.deepcopy(body['layers'][0]))
        self.write(reference['path'], body)
        with self.assertRaises(ValueError):
            i.inspect_rust_layers(self.root, ref(self.root, reference['path']), 'a' * 32, source_files(self.root))

    def test_38_scenes_are_exact_and_limited_diagnostics_cannot_clear_a_journey(self):
        self.assertEqual(len(j.ORIGINAL_COMPONENT_SCENES), 38)
        self.assertEqual(len(j.COMPONENT_SCENES), 40)
        self.assertEqual(len({value[0] for value in j.COMPONENT_SCENES.values()}), 14)
        for identity in j.COMPONENT_SCENES:
            result = j.scene_verdict(identity, {'steps': [], 'transport_mode': 'unavailable'})
            self.assertEqual(result['status'], 'blocked')
            self.assertTrue(result['blocked_obligations'])

    def checker_steps(self, populated):
        """Synthetic checker inputs only; never accepted as production evidence."""
        actions = ('library-entry', 'target-reviewed', 'before-intent', 'candidate-shared',
                   'association-returned', 'draft-selected', 'submit-failed', 'effect-failed',
                   'session-replaced', 'current-operation', 'stage-handshake', 'stage-candidates_ready',
                   'session-frame', 'late-candidate-frame', 'valid-frame', 'paused', 'resuming',
                   'new-frame', 'identity-reachable', 'preview-reachable', 'confirmation-reachable', 'after')
        messages, steps = [], []
        if populated:
            messages = [{'action': 'reset', 'outcome': 'ok',
                         'response': {'initial_snapshot': {'session': {'sessionId': 'protected'}}},
                         'events': [], 'publications': []}]
        fields = [{'label': label, 'expected': value, 'actual': value, 'matchCount': 1, 'inputAvailable': True}
                  for label, value in zip(('Hook', 'Address', 'Context', 'Context 2'),
                                         ('RealHook', '18446744073709551615', '9007199254740993', '0'))]
        rect = {'x': 1, 'y': 1, 'right': 100, 'bottom': 30}
        for index, action in enumerate(actions):
            if populated and action in {'association-returned', 'session-replaced', 'current-operation', 'after'}:
                messages.append({'action': 'invoke', 'outcome': 'ok', 'response': {},
                                 'events': [{'kind': 'association_stage', 'operation_id': 'actual-op', 'stage': action}],
                                 'publications': [{'snapshot': {'session': {'sessionId': 'replacement'}}}]
                                 if action in {'session-replaced', 'current-operation'} else []})
            chosen = populated and index >= actions.index('draft-selected')
            choices = [{'label': 'Source ' + value, 'checked': chosen, 'disabled': False, 'sourceId': value}
                       for value in ('A', 'B')] if populated else []
            methods = ([{'method': 'reviewAssociation', 'args': []}] if populated else [])
            if populated and index >= actions.index('association-returned'):
                methods += [{'method': 'associate', 'args': []}]
            if populated and index >= actions.index('submit-failed'):
                methods += [{'method': 'beginReading', 'args': []}]
            if populated and index >= actions.index('paused'):
                methods += [{'method': 'controlWindowPreview', 'args': [{'action': 'pause'}]}]
            if populated and index >= actions.index('resuming'):
                methods += [{'method': 'controlWindowPreview', 'args': [{'action': 'resume'}]}]
            effects = ([{'kind': 'selection_commit', 'source_ids': ['A', 'B'], 'remember': True},
                        {'kind': 'hud_shown'}, {'kind': 'main_hidden'},
                        {'kind': 'saved_selection_comparison', 'before_hash': 'same', 'after_hash': 'same'},
                        {'kind': 'association_event_rejected', 'operation_id': 'old-op'},
                        {'kind': 'stale_window_effect_rejected'},
                        {'kind': 'association_stage', 'operation_id': 'actual-op', 'stage': action}]
                       if populated else [])
            stamp = '2026-11-10T00:00:00Z' if populated else None
            if populated and action == 'new-frame':
                stamp = '2026-11-10T00:00:01Z'
            steps.append({'action': action,
                          'facts': {'transportMode': 'production_rpc',
                                    'snapshot': {'session': {'sessionId': 'protected'} if populated else None},
                                    'session': {'sources': [{'sample': '完整样本'}]} if populated else None,
                                    'methodCalls': methods, 'rpcMessages': copy.deepcopy(messages),
                                    'events': [{'kind': 'production_side_effect', 'event': event} for event in effects]},
                          'dom': {'text': ('完整样本 目标完整路径 程序架构 Windows 游戏进程 复用 切换 当前 需要游戏窗口 '
                                           '窗口已关联 自动接入 选择正文 已连接 候选 取消 失败 未完成 失效 重试 '
                                           '重新核对 选择已保存 浮窗 撤销屏幕录制' if populated else ''),
                                  'buttons': [{'label': label, 'disabled': label == '开始阅读', 'pressed': 'true', 'rect': rect}
                                              for label in ('关联并接入', '选择对应的游戏窗口…', '进程列表', '开始阅读', '重试')]
                                             if populated else [],
                                  'checkboxes': choices + ([{'label': '记住选择', 'checked': True, 'disabled': False, 'sourceId': None}]
                                                          if populated else []),
                                  'previews': [{'state': 'paused' if action == 'paused' else 'unavailable',
                                                'text': '需要游戏窗口 撤销屏幕录制', 'time': '尚无成功画面', 'dateTime': stamp,
                                                'images': [{'complete': True, 'naturalWidth': 100}] if populated else []}],
                                  'headings': ['游戏库'] if populated else [],
                                  'dialogs': [{'text': 'actual-stage'}] if populated else [],
                                  'details': [{'text': '其他容器 Bottle', 'open': False}] if populated else [],
                                  'progressStages': [{'text': value, 'current': True, 'complete': False}
                                                     for value in ('核验', '自动接入', '已连接', '候选')] if populated else [],
                                  'geometry': {'bodyScrollTop': 0, 'rootScrollTop': 0, 'rootScrollWidth': 100,
                                               'rootClientWidth': 100, 'width': 1040, 'height': 720}},
                          'ui_actions': [{'clicked': True, 'label': label} for label in ('取消', '关联并接入', '开始阅读', '稍后选择')]
                                        if populated else [],
                          'source_details': {'rows': [{'source_id': 'A', 'rowCount': 1, 'fields': copy.deepcopy(fields)}]
                                             if populated else [], 'blocked_obligations': [] if populated else
                                             ['advanced-source-typed-DTO-disclosure', 'source-u64-typed-DTO-projection']}})
        return steps

    def test_actual_node_and_independent_python_oracles_agree_for_all_38_typed_branches(self):
        # Exercise the real exported runner checker. These deliberately synthetic
        # typed vectors test checker plumbing, not Rust/GUI/product acceptance.
        vectors = []
        for populated in (False, True):
            steps = self.checker_steps(populated)
            vectors.extend({'id': identity, 'name': name, 'steps': steps}
                           for identity, (_, name) in j.ORIGINAL_COMPONENT_SCENES.items())
        mutated = self.checker_steps(True)
        for step in mutated:
            step['facts']['rpcMessages'] = []
            step['ui_actions'] = []
            for row in step['source_details']['rows']:
                for field in row['fields']:
                    field.update(matchCount=0, actual=None)
            for row in step['facts']['events']:
                if row['event']['kind'] == 'selection_commit':
                    row['event']['source_ids'] = ['borrowed-source']
        vectors.extend({'id': identity, 'name': name, 'steps': mutated}
                       for identity, (_, name) in j.ORIGINAL_COMPONENT_SCENES.items())
        script = ("import fs from 'node:fs';import {sceneChecks,actualSceneInputs} from './scripts/report/verify_binding_journey.mjs';"
                  "const rows=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(rows.map(row=>"
                  "({id:row.id,assertions:sceneChecks(row,row.steps),inputs:actualSceneInputs(row.steps)}))));")
        completed = subprocess.run(['node', '--input-type=module', '-e', script], cwd=j.ROOT,
                                   input=json.dumps(vectors), text=True, capture_output=True, timeout=30)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        actual = json.loads(completed.stdout)
        self.assertEqual(len(actual), 114)
        for vector, observed in zip(vectors, actual):
            with self.subTest(identity=vector['id'], inputs=bool(vector['steps'][0]['facts']['rpcMessages'])):
                judgment = j.scene_verdict(vector['id'], {'steps': vector['steps'], 'transport_mode': 'production_rpc'})
                self.assertEqual(observed['id'], vector['id'])
                self.assertEqual(observed['assertions'], judgment['assertions'])
                self.assertEqual(observed['inputs'], j.actual_scene_inputs(vector['steps']))
        empty_close = j.scene_verdict('BJS-07-close-operation',
                                     {'steps': self.checker_steps(False), 'transport_mode': 'production_rpc'})
        self.assertEqual(empty_close['status'], 'failed')
        self.assertIn('close-operation-was-actually-associated', empty_close['failed_assertions'])
        self.assertIn('actual-committed-operation-input:BJS-07-close-operation', empty_close['blocked_obligations'])

    def test_recompute_missing_actual_diagnostic_capability_remains_blocked_without_fake_dom(self):
        trace = self.write('.build/diagnostic/trace.json',
                           {'observations': {'transport_mode': 'unavailable', 'blocked_obligations': ['producer-ready']}, 'events': []})
        row = {'id': j.COMPONENT_CASES[0], 'status': 'blocked', 'assertions': [],
               'trace': ref(self.root, trace.relative_to(self.root).as_posix()), 'dom': None, 'screenshot': None}
        bundle = {'producer': {'values': {'nonce': 'a' * 32}, 'binary': 'not-executed',
                               'manifest': {'runtime_transport': {'mode': 'ndjson'}}},
                  'report': {'cases': [row]}, 'actual_rpc': {'ready': None}}
        with (patch.object(r.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, json.dumps(bundle['producer']['values']), '')),
              patch.object(r, 'recompute_scene_layout', return_value={'errors': [], 'executed_steps': 0})):
            result = r.recompute(self.root, bundle)
        self.assertEqual(result['errors'], [])
        self.assertEqual(result['case_statuses'][row['id']], 'blocked')
        row['status'] = 'passed'
        with (patch.object(r.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, json.dumps(bundle['producer']['values']), '')),
              patch.object(r, 'recompute_scene_layout', return_value={'errors': [], 'executed_steps': 0})):
            self.assertTrue(r.recompute(self.root, bundle)['errors'])

    def test_original_dom_geometry_and_typed_controls_cannot_be_forged(self):
        self.assertTrue(r.close_values({'x': 10, 'checked': True}, {'x': 12, 'checked': True}))
        self.assertTrue(r.close_values({'checked': True}, {'checked': 1}))
        self.assertEqual(r.close_values({'rect': {'x': 10.0}}, {'rect': {'x': 10.01}}), [])

    def journal_fixture(self):
        source = {'fixture-source': 'a' * 64}
        manifest = {'main_pid': 123, 'profile_id': 'actual-profile', 'started_at_ms': 1000,
                    'finished_at_ms': 2000, 'binding_journal': []}
        begin = {'kind': 'binding_journal_begin', 'schema_version': 1, 'nonce': 'b' * 32,
                 'pid': 123, 'source_tree_sha256': tree_sha(source), 'profile_id': 'actual-profile',
                 'boot_sec': 1, 'boot_usec': 0}
        shared = {'nonce': 'b' * 32, 'pid': 123, 'profile_id': 'actual-profile', 'boot_sec': 1, 'boot_usec': 0}
        event = dict(shared, kind='binding_journal_event', sequence=1, observed_at_ms=1500,
                     action='context_transition', correlation_id='real-opaque-operation-correlation',
                     review_id=None, session_id=None, operation_id=None, selection_set_revision=None,
                     active_target_identity_sha256=None, candidate_target_identity_sha256=None)
        health = dict(shared, kind='binding_journal_health', dropped=0, invalid=0, write_failures=0,
                      accepted=1, written=1, last_sequence=1)
        path = self.root / '.build/native/journal.jsonl'
        path.parent.mkdir(parents=True)
        records = [begin, event, health]
        path.write_text(''.join(json.dumps(row) + '\n' for row in records))
        manifest['binding_journal'] = [ref(self.root, '.build/native/journal.jsonl')]
        return manifest, source, path, records

    def test_native_journal_allows_real_unallocated_phase_identity_without_test_case_in_product(self):
        manifest, source, path, records = self.journal_fixture()
        header, events = i.inspect_binding_journal(self.root, manifest, path.parent, 'b' * 32, source)
        self.assertEqual(len(events), 1)
        self.assertIsNone(events[0]['session_id'])
        self.assertNotIn('case_id', events[0])

    def test_native_journal_rejects_old_cohort_loss_and_sequence_omission(self):
        manifest, source, path, records = self.journal_fixture()
        for mutate in (lambda rows: rows[1].update(sequence=2),
                       lambda rows: rows[1].update(nonce='old'),
                       lambda rows: rows[2].update(dropped=1),
                       lambda rows: rows[2].update(written=0),
                       lambda rows: rows[0].update(source_tree_sha256='f' * 64)):
            values = copy.deepcopy(records)
            mutate(values)
            path.write_text(''.join(json.dumps(row) + '\n' for row in values))
            manifest['binding_journal'] = [ref(self.root, '.build/native/journal.jsonl')]
            with self.assertRaises(ValueError):
                i.inspect_binding_journal(self.root, manifest, path.parent, 'b' * 32, source)

    def test_current_native_set_cannot_be_proved_by_another_operation_history(self):
        def step(name):
            return {'id': name, 'calls': [], 'text': '开始阅读', 'tables': {'sessions': [], 'source_selection_sets': [], 'source_selection_members': []},
                    'context': {'operation_id': 'current-op', 'selection_set_revision': '3'}}
        steps = [step(name) for name in j.NATIVE_STEPS['BJ-08']]
        steps[-1]['tables']['source_selection_sets'] = [{'operation_id': 'old-op', 'selection_set_revision': 1, 'state': 'sealed', 'member_count': 2}]
        steps[-1]['calls'] = [{'action': 'port_call', 'method': 'beginReading'}]
        self.assertTrue(any('current multi-source' in error for error in i.native_semantics('BJ-08', steps)))

    def test_native_and_service_free_passed_json_are_rejected_before_promotion(self):
        path = self.write('.build/fake-native.json', {'status': 'passed', 'native_verified': True})
        with self.assertRaises(ValueError):
            i.inspect_native_collection(self.root, ref(self.root, path.relative_to(self.root).as_posix()), 'a' * 32, source_files(self.root))
        with self.assertRaises(ValueError):
            i.inspect_live_service(self.root, ref(self.root, path.relative_to(self.root).as_posix()), 'a' * 32, source_files(self.root))

    def test_bj10_detail_cannot_borrow_candidate_u64_by_using_whole_snapshot_as_source(self):
        reset = {'id': '1', 'action': 'reset', 'outcome': 'ok', 'response': {'initial_snapshot': {'session': {'sourceId': 's'}, 'candidate': {'created': '18446744073709551615'}}}}
        reset['response']['source_detail_obligations'] = [{'source_id': 's', 'source_pointer': '',
            'source_id_pointer': '/session/sourceId', 'fields': [{'label': 'Address', 'json_pointer': '/candidate/created', 'value': '18446744073709551615'}]}]
        step = {'facts': {'rpcMessages': [reset]}, 'source_details': {'metadata': reset['response']['source_detail_obligations'], 'rows': [{}]}}
        with self.assertRaisesRegex(ValueError, 'one actual source entity'):
            i.inspect_source_details(step, {'messages': []})

    def test_stage_phase_entry_cannot_be_used_as_release_or_global_product_verification(self):
        from types import SimpleNamespace
        from verify import binding_gate_phase
        with self.assertRaises(ValueError):
            binding_gate_phase(SimpleNamespace(change=j.CHANGE, profile='release', self_test=False))


class ResumptionComponentAndExecutionGuards(unittest.TestCase):
    """Synthetic guard inputs only; actual product producers remain required."""
    def ready(self):
        profile = {'profileId':'profile','serverInstanceId':'server','profileGeneration':3}
        reply = {'profile':profile,'sessionId':'session','sources':[{'sourceId':'9007199254740993'},{'sourceId':'other'}]}
        return {'action':'stage-candidates_ready','facts':{'snapshot':{**profile,'session':{'sessionId':'session'}},
            'rpcMessages':[{'action':'invoke','method':'associate','outcome':'ok','response':{'operationId':'real-op','sessionId':'session'}},
                           {'action':'invoke','method':'sourceSelection','outcome':'ok','response':reply}],
            'events':[{'kind':'production_side_effect','event':{'kind':'association_stage','operation_id':'real-op','stage':'candidates_ready'}}],
            'methodCalls':[]},'dom':{'sourceRows':[{'sourceId':identity,'checkboxCount':1,'visibleCheckboxCount':1,'enabledCheckboxCount':1} for identity in ('9007199254740993','other')],
            'checkboxes':[{'sourceId':identity,'label':'same Hook label','disabled':False} for identity in ('9007199254740993','other')],
            'buttons':[{'label':'开始阅读'},{'label':'稍后选择'}]}}

    def node_candidates(self, values):
        script = ('import{candidateReadyEvidence}from "./scripts/report/verify_binding_journey.mjs";'
                  'let data="";for await(const chunk of process.stdin)data+=chunk;'
                  'process.stdout.write(JSON.stringify(JSON.parse(data).map(candidateReadyEvidence)));')
        result = subprocess.run(['node','--input-type=module','--eval',script],cwd=j.ROOT,
                                input=json.dumps(values),text=True,capture_output=True,timeout=30)
        self.assertEqual(result.returncode,0,result.stderr)
        return json.loads(result.stdout)

    def test_candidates_ready_independently_joins_actual_source_panel(self):
        good=self.ready();self.assertTrue(j.candidate_ready_evidence(good))
        self.assertEqual(self.node_candidates([good]),[True])

    def test_node_python_reject_wrong_operation_profile_and_checkbox_rows(self):
        variants=[]
        for mutation in ('wrong_op','wrong_session','wrong_profile','missing_profile','duplicate_row','duplicate_box','hidden_box','disabled_box','wrong_source','duplicate_begin','implicit_commit'):
            step=self.ready()
            if mutation=='wrong_op':step['facts']['events'][0]['event']['operation_id']='other-op'
            if mutation=='wrong_session':step['facts']['rpcMessages'][0]['response']['sessionId']='other-session'
            if mutation=='wrong_profile':step['facts']['rpcMessages'][1]['response']['profile']['serverInstanceId']='other-server'
            if mutation=='missing_profile':step['facts']['rpcMessages'][1]['response']['profile']={}
            if mutation=='duplicate_row':step['dom']['sourceRows'].append(copy.deepcopy(step['dom']['sourceRows'][0]))
            if mutation=='duplicate_box':step['dom']['sourceRows'][0]['checkboxCount']=2
            if mutation=='hidden_box':step['dom']['sourceRows'][0]['visibleCheckboxCount']=0
            if mutation=='disabled_box':step['dom']['checkboxes'][0]['disabled']=True
            if mutation=='wrong_source':step['dom']['checkboxes'][0]['sourceId']='copied-display-name'
            if mutation=='duplicate_begin':step['dom']['buttons'].append({'label':'开始阅读'})
            if mutation=='implicit_commit':step['facts']['methodCalls'].append({'method':'beginReading','args':[]})
            variants.append(step)
        expected=[j.candidate_ready_evidence(step) for step in variants]
        self.assertEqual(expected,[False]*len(variants));self.assertEqual(self.node_candidates(variants),expected)

    def test_pending_and_duplicate_submit_are_real_failures(self):
        for field,value in [('pending',False),('duplicateSubmitAvailable',True)]:
            observed={'stageLabels':['窗口已关联','自动接入','选择正文'],'pending':True,'duplicateSubmitAvailable':False}
            observed[field]=value
            self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[2],observed)['status'],'failed')

    def test_pending_stage_labels_use_semantic_list_without_number_or_note(self):
        labels=['窗口已关联','自动接入','选择正文']
        html='<html><body><p>选择正文是下一阶段</p><ol aria-label="接入流程">'+''.join('<li><b>'+str(index)+'</b>'+label+'</li>' for index,label in enumerate(labels,1))+'</ol></body></html>'
        self.assertEqual(r.dom_errors(j.COMPONENT_CASES[2],{'stageLabels':labels},html,[]),[])
        self.assertTrue(r.dom_errors(j.COMPONENT_CASES[2],{'stageLabels':labels[:-1]},html,[]))

    def test_observer_waits_actual_fresh_call_return_instead_of_forty_ms_or_expected_ui(self):
        before={'rpcMessages':[{'id':'1','method':'associate','outcome':'ok','response':{'operationId':'op'}},
            {'id':'2','method':'associationProgress','outcome':'ok','response':{'operationId':'op','stage':'waiting_text'}},
            {'id':'3','action':'deliver','events':[{'kind':'association_stage','operation_id':'op','stage':'candidates_ready'}]}],'events':[]}
        responded=copy.deepcopy(before)
        # A wrong actual state still completes the observation barrier; the
        # semantic oracle must inspect and reject its actual UI separately.
        responded['rpcMessages'].append({'id':'4','method':'associationProgress','outcome':'ok','response':{'operationId':'op','stage':'waiting_text'}})
        responded['events']=[{'kind':'port_call','id':'7','method':'associationProgress','args':[{'operationId':'op'}]},
            {'kind':'runtime_transport_call','responseId':'4','method':'associationProgress','args':[{'operationId':'op'}]},
            {'kind':'port_return','id':'7','method':'associationProgress','response':{'operationId':'op','stage':'waiting_text'}}]
        wrong=copy.deepcopy(responded);wrong['events'][1]['args'][0]['operationId']='old-op'
        pending=copy.deepcopy(responded);pending['events'].pop()
        script=('import{actualProgressReadReady}from "./scripts/report/verify_binding_journey.mjs";let raw="";'
                'for await(const chunk of process.stdin)raw+=chunk;process.stdout.write(JSON.stringify(JSON.parse(raw).map(facts=>actualProgressReadReady({facts,deliveryId:"3"}))));')
        result=subprocess.run(['node','--input-type=module','--eval',script],cwd=j.ROOT,input=json.dumps([before,responded,wrong,pending]),text=True,capture_output=True,timeout=30)
        self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(json.loads(result.stdout),[False,True,False,False])

    def test_actual_checkpoint_prefix_and_late_rpc_events_are_both_retained(self):
        reply={'id':'1','events':[{'kind':'capture_observed','value':True}]}
        checkpoint={'events':[{'kind':'production_side_effect','event':reply['events'][0]}],
                    'rpcMessages':[reply],'methodCalls':[],'transportCalls':[]}
        later={'id':'2','events':[{'kind':'source_query_finished'}]};post=copy.deepcopy(checkpoint)
        post['rpcMessages'].append(later);post['events'].append({'kind':'production_side_effect','event':later['events'][0]})
        actual={'1':{'response':reply},'2':{'response':later}}
        i.inspect_post_checkpoint_facts(checkpoint,post,actual)
        for mutation in ('dropped_late','rewrite_prefix','wrong_response','bool_number'):
            changed=copy.deepcopy(post)
            if mutation=='dropped_late':changed['events'].pop()
            if mutation=='rewrite_prefix':changed['methodCalls'].append({'method':'rewritten'});changed['events'][0]['event']['kind']='different'
            if mutation=='wrong_response':changed['rpcMessages'][-1]['id']='unknown'
            if mutation=='bool_number':changed['events'][0]['event']['value']=1
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):i.inspect_post_checkpoint_facts(checkpoint,changed,actual)

    def test_finite_picker_counts_real_port_rpc_not_controlled_marker(self):
        before={'events':[],'methodCalls':[],'transportCalls':[],'rpcMessages':[],'pickers':[],'associations':[]}
        args=[{'candidateId':'actual-candidate','consumer':'candidate','consumerId':'actual-consumer','expectedGeneration':'1'}]
        call={'kind':'port_call','id':'7','method':'selectWindowPreview','args':args}
        response={'id':'11','events':[],'outcome':'ok','response':{'state':'starting'}}
        transport={'method':'selectWindowPreview','args':args,'responseId':'11'}
        after={'events':[call,{'kind':'runtime_transport_call',**transport},{'kind':'port_return','id':'7','method':'selectWindowPreview','response':response['response']}],
               'methodCalls':[call],'transportCalls':[transport],'rpcMessages':[response],'pickers':[args[0]],'associations':[]}
        obs={'beforeFacts':before,'afterFacts':after,'pickerCallsBeforeIntent':0,'pickerCallsAfterIntent':1,'associationCallsBeforeIntent':0}
        rpc={'messages':[{'request':{'scenario':j.DIAGNOSTIC_SCENES[j.COMPONENT_CASES[6]],'action':'invoke','method':'selectWindowPreview','args':args},'response':response}]}
        i.inspect_finite_picker(obs,after['events'],rpc)
        self.assertEqual(r.dom_errors(j.COMPONENT_CASES[6],obs,'<html></html>',after['events']),[])
        absent=copy.deepcopy(obs);absent['afterFacts']=copy.deepcopy(before);absent['pickerCallsAfterIntent']=0
        i.inspect_finite_picker(absent,[],{'messages':[]})
        self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[6],absent)['status'],'failed')
        association_args=[{'reviewId':'actual-review'}]
        association_call={'kind':'port_call','id':'6','method':'associate','args':association_args}
        association_transport={'method':'associate','args':association_args,'responseId':'10'}
        association_response={'id':'10','events':[],'outcome':'ok','response':{'operationId':'actual-operation'}}
        association_events=[association_call,{'kind':'runtime_transport_call',**association_transport},
                            {'kind':'port_return','id':'6','method':'associate','response':association_response['response']}]
        associated=copy.deepcopy(obs)
        for facts in (associated['beforeFacts'],associated['afterFacts']):
            facts['events']=copy.deepcopy(association_events)+facts['events']
            facts['methodCalls'].insert(0,copy.deepcopy(association_call));facts['transportCalls'].insert(0,copy.deepcopy(association_transport))
            facts['rpcMessages'].insert(0,copy.deepcopy(association_response));facts['associations']=[association_args[0]]
        associated['associationCallsBeforeIntent']=1
        associated_rpc={'messages':[{'request':{'scenario':j.DIAGNOSTIC_SCENES[j.COMPONENT_CASES[6]],'action':'invoke','method':'associate','args':association_args},'response':association_response},*rpc['messages']]}
        i.inspect_finite_picker(associated,associated['afterFacts']['events'],associated_rpc)
        self.assertEqual(r.dom_errors(j.COMPONENT_CASES[6],associated,'<html></html>',associated['afterFacts']['events']),[])
        self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[6],associated)['status'],'failed')
        wrong_association=copy.deepcopy(associated_rpc);wrong_association['messages'][0]['request']['method']='sourceSelection'
        with self.assertRaises(ValueError):i.inspect_finite_picker(associated,associated['afterFacts']['events'],wrong_association)
        marker=[{'kind':'explicit_system_picker','request':args[0]}]
        self.assertTrue(r.dom_errors(j.COMPONENT_CASES[6],obs,'<html></html>',marker))
        for mutation in ('method','scenario','request','duplicate','typed_count'):
            changed=copy.deepcopy(obs);raw=copy.deepcopy(rpc)
            if mutation=='method':raw['messages'][0]['request']['method']='startWindowPreview'
            if mutation=='scenario':raw['messages'][0]['request']['scenario']='BJS-04-picker-cancel'
            if mutation=='request':raw['messages'][0]['request']['args'][0]['candidateId']='borrowed'
            if mutation=='duplicate':changed['afterFacts']['events'].insert(0,copy.deepcopy(call))
            if mutation=='typed_count':changed['pickerCallsAfterIntent']=True
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):
                i.inspect_finite_picker(changed,changed['afterFacts']['events'],raw)

    def test_pending_capture_keeps_first_consistent_cut_and_original_late_events(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory=root/'.build/capture';directory.mkdir(parents=True)
            image=directory/'pending.png';image.write_bytes(pixel_png())
            image_ref=ref(root,image.relative_to(root).as_posix())
            ui={'visibleText':'actual pending text','stageLabels':['one','two','three'],'dialogVisible':True,'duplicateSubmitAvailable':False}
            call={'kind':'port_call','id':'7','method':'associate','args':[{'reviewId':'actual-review'}]}
            facts={'events':[call],'methodCalls':[call],'transportCalls':[],'rpcMessages':[],'associations':[{'reviewId':'actual-review'}]}
            late=copy.deepcopy(facts);late['events'].append({'kind':'preview_subscription_detach','token':'real-token'})
            record={'schema_version':1,'kind':'binding-finite-pending-capture/v1','nonce':'a'*32,'case_id':j.COMPONENT_CASES[2],'attempt':1,
                    'before':{'at_ms':100,'ui':ui,'facts':facts},'after':{'at_ms':101,'ui':copy.deepcopy(ui),'facts':late},'screenshot':image_ref}
            observations={'visibleText':ui['visibleText'],'stageLabels':ui['stageLabels'],'duplicateSubmitAvailable':False,
                          'associationCalls':facts['associations'],'pending':True,'postCaptureFacts':late}
            row={'id':j.COMPONENT_CASES[2],'screenshot':image_ref}
            def inspect(records,obs=observations,current_row=row):
                value=copy.deepcopy(obs);refs=[]
                for number,item in enumerate(records,1):
                    path=directory/('capture-'+str(number)+'.json');path.write_text(json.dumps(item))
                    refs.append(ref(root,path.relative_to(root).as_posix()))
                value['captureRecords']=refs
                i.inspect_pending_capture(root,current_row,{'observations':value,'events':facts['events']},directory,'a'*32)
            inspect([record])
            # Consistent actual UI may itself be semantically wrong. The
            # collector must not wait for the approved labels to appear.
            self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[2],observations)['status'],'failed')
            for mutation in ('old_text','late_erased','borrowed_hash','nonce','ordinal','skip_consistent','post_facts'):
                changed=copy.deepcopy(record);obs=copy.deepcopy(observations);records=[changed];current_row=copy.deepcopy(row)
                if mutation=='old_text':obs['visibleText']='text from older render'
                if mutation=='late_erased':changed['after']['facts']['events']=[]
                if mutation=='borrowed_hash':current_row['screenshot']['sha256']='b'*64
                if mutation=='nonce':changed['nonce']='b'*32
                if mutation=='ordinal':changed['attempt']=2
                if mutation=='skip_consistent':
                    second=copy.deepcopy(record);second['attempt']=2;second['before']['at_ms']=102;second['after']['at_ms']=103;records.append(second)
                if mutation=='post_facts':obs['postCaptureFacts']['events'].pop()
                with self.subTest(mutation=mutation),self.assertRaises(ValueError):inspect(records,obs,current_row)
            self.assertTrue(r.dom_errors(j.COMPONENT_CASES[2],observations,'<html><p>an older rendered state</p></html>',facts['events']))

    def test_pending_capture_rejects_association_erasure_between_consistent_cuts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory=root/'.build/capture';directory.mkdir(parents=True)
            image=directory/'pending.png';image.write_bytes(pixel_png());image_ref=ref(root,image.relative_to(root).as_posix())
            ui={'visibleText':'actual final text','stageLabels':['窗口已关联','自动接入','选择正文'],'dialogVisible':True,'duplicateSubmitAvailable':False}
            first_call={'kind':'port_call','id':'1','method':'associate','args':[{'reviewId':'first-review'}]}
            second_call={'kind':'port_call','id':'2','method':'associate','args':[{'reviewId':'second-review'}]}
            def facts(calls):
                return {'events':copy.deepcopy(calls),'methodCalls':copy.deepcopy(calls),'transportCalls':[],'rpcMessages':[],
                        'associations':[row['args'][0] for row in calls],'commands':[],'windows':[],'previewStarts':[],'pickers':[]}
            one,two=facts([first_call]),facts([first_call,second_call]);earlier=copy.deepcopy(ui);earlier['visibleText']='previous actual render'
            initial={'schema_version':1,'kind':'binding-finite-pending-capture/v1','nonce':'a'*32,'case_id':j.COMPONENT_CASES[2],'attempt':1,
                     'before':{'at_ms':100,'ui':earlier,'facts':one},'after':{'at_ms':101,'ui':ui,'facts':two},'screenshot':image_ref}
            last=copy.deepcopy(initial);last.update(attempt=2,before={'at_ms':102,'ui':ui,'facts':two},after={'at_ms':103,'ui':ui,'facts':two})
            row={'id':j.COMPONENT_CASES[2],'screenshot':image_ref}
            def inspect(final,initial_record=initial):
                refs=[]
                for index,record in enumerate((initial_record,final) if initial_record is not None else (final,),1):
                    path=directory/('capture-'+str(index)+'.json');path.write_text(json.dumps(record));refs.append(ref(root,path.relative_to(root).as_posix()))
                actual=final['before']['facts']
                obs={'visibleText':ui['visibleText'],'stageLabels':ui['stageLabels'],'duplicateSubmitAvailable':False,
                     'associationCalls':actual['associations'],'pending':len(actual['associations'])==1,
                     'captureRecords':refs,'postCaptureFacts':final['after']['facts']}
                i.inspect_pending_capture(root,row,{'observations':obs,'events':actual['events']},directory,'a'*32)
                return obs
            self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[2],inspect(last))['status'],'failed')
            zero=copy.deepcopy(last);zero['attempt']=1;zero['before']['facts']=facts([]);zero['after']['facts']=facts([])
            self.assertEqual(j.observation_verdict(j.COMPONENT_CASES[2],inspect(zero,None))['status'],'failed')
            erased=copy.deepcopy(last);erased['before']['facts']=one;erased['after']['facts']=one
            with self.assertRaises(ValueError):inspect(erased)

    def test_source_discharge_is_exact_same_run_scope(self):
        case={'id':j.COMPONENT_CASES[4],'status':'blocked','trace':{'path':'case','sha256':'a'*64},'observations':{'blocked_obligations':['advanced-source-typed-DTO-disclosure','source-u64-typed-DTO-projection']}}
        scene={'id':'BJS-10-details','status':'passed','trace':{'path':'scene','sha256':'b'*64},'observations':{'transport_mode':'production_rpc','steps':[{'source_details':{'blocked_obligations':[],'advancedAvailable':True,'precisionAvailable':True}}]}}
        self.assertEqual(j.scoped_source_discharge(case,scene,'a'*32)['obligations'],case['observations']['blocked_obligations'])
        for key,value in [('id','BJS-08-commit-set'),('status','blocked')]:
            changed=copy.deepcopy(scene);changed[key]=value;self.assertIsNone(j.scoped_source_discharge(case,changed,'a'*32))
        scene['observations']['transport_mode']='controlled_diagnostic'
        self.assertIsNone(j.scoped_source_discharge(case,scene,'a'*32))

    def library_steps(self,scoped=False):
        games=[{'gameId':identity,'displayName':'Game '+identity,'target':{'environmentKey':'bottle:'+identity,'canonicalExePath':'C:/'+identity+'.exe','architecture':'x64'}} for identity in ('A','B')]
        initial={'games':games,'session':{'gameId':'A'} if scoped else None}
        seed={'request':{'operationId':'seed','command':{'kind':'save_games','payload':{'items':[{'itemId':game['gameId'],'game':game} for game in games]}}},
              'response':{'gameResults':[{'itemId':game['gameId'],'gameId':game['gameId'],'rowRevision':1,'reused':False} for game in games]}}
        candidate={'candidateId':'runtime-B','environmentKey':'bottle:B','executablePath':'C:/B.exe','architecture':'x64','windowsPid':87,'processCreatedU64':'9007199254740993','targetIdentitySha256':'c'*64}
        review={'gameId':'A' if scoped else 'B','candidate':candidate}
        return [{'action':'library-entry','facts':{},'dom':{'buttons':[{'label':'Game A','pressed':'true'},{'label':'Game B','pressed':'false'}]}},
                {'action':'scoped-incompatible' if scoped else 'global-associated','facts':{'rpcMessages':[{'action':'reset','outcome':'ok','response':{'initial_snapshot':initial,'seed_commands':[seed]}},
                    {'action':'invoke','method':'reviewAssociation','outcome':'ok','response':review}],
                    'methodCalls':[{'method':'reviewAssociation','args':[{'candidateId':'runtime-B','gameId':'A' if scoped else None}]}]+([] if scoped else [{'method':'associate','args':[]}])},'dom':{'dialogs':[{}]}}]

    def test_global_and_scoped_owner_are_separate_full_target_joins(self):
        for scoped in (False,True):
            checks=j.library_owner_checks('scoped-reconnect-owner' if scoped else 'library-global-owner',self.library_steps(scoped))
            self.assertTrue(all(expected==actual for _,expected,actual in checks),checks)

    def test_new_library_owner_oracles_independently_agree_in_node_and_python(self):
        values=[{'name':'library-global-owner','steps':self.library_steps()},
                {'name':'scoped-reconnect-owner','steps':self.library_steps(True)}]
        for bad in ('borrow_A','rename_B','implicit_scope','duplicate_save','duplicate_attach'):
            steps=self.library_steps();last=steps[-1]['facts']
            if bad=='borrow_A':last['rpcMessages'][1]['response']['gameId']='A'
            if bad=='rename_B':last['rpcMessages'][0]['response']['initial_snapshot']['games'][1]['displayName']='Game A'
            if bad=='implicit_scope':last['methodCalls'][0]['args'][0]['gameId']='A'
            if bad=='duplicate_save':last['methodCalls'].append({'method':'dispatch','args':[{'command':{'kind':'save_game'}}]})
            if bad=='duplicate_attach':last['methodCalls'].append({'method':'associate','args':[]})
            values.append({'name':'library-global-owner','steps':steps})
        script=('import{libraryOwnerChecks}from "./scripts/report/verify_binding_journey.mjs";let raw="";'
                'for await(const chunk of process.stdin)raw+=chunk;process.stdout.write(JSON.stringify(JSON.parse(raw).map(row=>libraryOwnerChecks(row.name,row.steps))));')
        run=subprocess.run(['node','--input-type=module','--eval',script],cwd=j.ROOT,input=json.dumps(values),text=True,capture_output=True,timeout=30)
        self.assertEqual(run.returncode,0,run.stderr)
        self.assertEqual(json.loads(run.stdout),json.loads(json.dumps([j.library_owner_checks(row['name'],row['steps']) for row in values])))

    def test_wrong_owner_duplicate_attach_and_implicit_save_fail(self):
        for mutation in ('owner','attach','save','target','seed'):
            steps=self.library_steps();last=steps[-1]['facts']
            if mutation=='owner':last['rpcMessages'][1]['response']['gameId']='A'
            if mutation=='attach':last['methodCalls'].append({'method':'associate','args':[]})
            if mutation=='save':last['methodCalls'].append({'method':'dispatch','args':[{'command':{'kind':'save_games'}}]})
            if mutation=='target':last['rpcMessages'][1]['response']['candidate']['environmentKey']='wrong-bottle'
            if mutation=='seed':last['rpcMessages'][0]['response']['seed_commands'][0]['response']['gameResults'][1]['gameId']='A'
            self.assertFalse(all(expected==actual for _,expected,actual in j.library_owner_checks('library-global-owner',steps)),mutation)

    def test_missing_new_library_producer_is_blocked_never_passed(self):
        for identity in j.LIBRARY_OWNER_SCENES:
            self.assertEqual(j.scene_verdict(identity,{'steps':[],'transport_mode':'unavailable'})['status'],'blocked')

    def test_fixed_native_units_missing_producer_remains_blocked(self):
        groups=i.inspect_fixed_native_units(j.ROOT,None,'a'*32,{})
        self.assertEqual({row['id'] for row in groups},set(j.FIXED_NATIVE_UNIT_GROUPS))
        self.assertTrue(all(row['status']=='blocked' for row in groups))

    def test_exact_exit_zero_does_not_replace_real_preview_observations(self):
        actual=i.inspect_preview_observations('test bj_07_domain ... ok\n','bj_07_domain',{'pid':42},'a'*32,{})
        self.assertEqual(actual['status'],'blocked')

    def test_free_preview_verdict_and_wrong_nonce_pid_rejected(self):
        base={'kind':'binding-preview-observations/v1','schema_version':1,'nonce':'a'*32,'source_tree_sha256':tree_sha({}),'test_name':'bj_07_domain','pid':42,'events':[]}
        for field,value in [('passed',True),('pid',43),('nonce','b'*32)]:
            changed=copy.deepcopy(base);changed[field]=value
            raw='LUNA_BINDING_PREVIEW_OBSERVATIONS_V1 '+json.dumps(changed)
            with self.assertRaises(ValueError):i.inspect_preview_observations(raw,'bj_07_domain',{'pid':42},'a'*32,{})

    def test_full_witness_requires_real_resource_identity(self):
        witness={field:'actual' for field in i.ACTIVE_WITNESS_FIELDS};witness.update(windows_pid=42,process_created_u64='9007199254740993',architecture='x64',target_identity_sha256='a'*64,
            resource={'snapshot_id':'resource','kind':'hook','version':'1','manifest_sha256':'b'*64,'contract_version':'1','metadata_json':'{}'})
        profile={'profileId':'p','serverInstanceId':'s','profileGeneration':1}
        self.assertTrue(i.valid_active_witness(profile,witness))
        for field in i.RESOURCE_WITNESS_FIELDS:
            changed=copy.deepcopy(witness);changed['resource'].pop(field);self.assertFalse(i.valid_active_witness(profile,changed))

    def preview_document(self):
        witness={field:'actual' for field in i.ACTIVE_WITNESS_FIELDS}
        witness.update(windows_pid=42,process_created_u64='9007199254740993',architecture='x64',target_identity_sha256='a'*64,
            resource={'snapshot_id':'resource','kind':'hook','version':'1','manifest_sha256':'b'*64,'contract_version':'1','metadata_json':'{}'})
        context={'profile':{'profileId':'p','serverInstanceId':'s','profileGeneration':1},'active_witness':witness}
        events=[]
        def event(branch,kind,data):
            n=len(events)+1;events.append({'sequence':n,'monotonic_ns':n,'wall_ms':2000,'thread_id':'ThreadId(1)','branch':branch,'kind':kind,'data':copy.deepcopy(data)})
        for branch in ('resource_failure','target_lease_failure'):
            event(branch,'s1_active_before',context);event(branch,'s2_failed_before_stop',{'code':branch})
            event(branch,'s1_active_after',context);event(branch,'fresh_authority_acquired',context)
            event(branch,'call_counters',{'stop_s1':0,'old_handoff_callbacks':0})
        for field in sorted(i.AUTHORITY_MUTATION_FIELDS):
            attempted=copy.deepcopy(context)
            if field in ('profileId','serverInstanceId','profileGeneration'):attempted['profile'][field]='changed'
            elif field.startswith('resource.'):attempted['active_witness']['resource'][field.split('.',1)[1]]='changed'
            else:attempted['active_witness'][field]='changed'
            event('authority_races','mutated_authority_attempt',{'mutated_field':field,'current_authority':context,'attempted_authority':attempted,'effect_callbacks':0,'before':context,'after':context})
        event('authority_races','fresh_authority_attempt',{'effect_callbacks':1})
        return {'kind':'binding-preview-observations/v1','schema_version':1,'nonce':'a'*32,'source_tree_sha256':tree_sha({}),'test_name':'bj_07_domain','pid':42,'events':events}

    def inspect_preview(self,document):
        raw='LUNA_BINDING_PREVIEW_OBSERVATIONS_V1 '+json.dumps(document)
        execution={'pid':42,'started_at':'1970-01-01T00:00:01+00:00','finished_at':'1970-01-01T00:00:03+00:00'}
        return i.inspect_preview_observations(raw,'bj_07_domain',execution,'a'*32,{})

    def test_actual_preview_observation_guard_recomputes_two_failures_and_all_fences(self):
        result=self.inspect_preview(self.preview_document())
        self.assertEqual(result['status'],'passed');self.assertTrue(result['checks'])

    def test_preview_sequence_clock_and_branch_inventory_tampering_rejected(self):
        for mutation in ('sequence','monotonic','wall','missing_branch','missing_resource'):
            raw=self.preview_document()
            if mutation=='sequence':raw['events'][1]['sequence']=1
            if mutation=='monotonic':raw['events'][1]['monotonic_ns']=1
            if mutation=='wall':raw['events'][1]['wall_ms']=0
            if mutation=='missing_branch':raw['events']=[row for row in raw['events'] if row['branch']!='target_lease_failure']
            if mutation=='missing_resource':raw['events'][0]['data']['active_witness']['resource']={}
            if mutation=='missing_resource':self.assertEqual(self.inspect_preview(raw)['status'],'failed')
            else:
                with self.subTest(mutation=mutation),self.assertRaises(ValueError):self.inspect_preview(raw)

    def test_old_handoff_callback_and_stop_s1_are_semantic_failures(self):
        for counter in ('stop_s1','old_handoff_callbacks'):
            raw=self.preview_document();next(row for row in raw['events'] if row['kind']=='call_counters')['data'][counter]=1
            self.assertEqual(self.inspect_preview(raw)['status'],'failed')

    def test_mutated_resource_fence_must_not_invoke_callback(self):
        raw=self.preview_document();next(row for row in raw['events'] if row['kind']=='mutated_authority_attempt' and row['data']['mutated_field']=='resource.manifest_sha256')['data']['effect_callbacks']=1
        self.assertEqual(self.inspect_preview(raw)['status'],'failed')

    def test_fresh_nonce_claim_is_exclusive_and_refuses_predecessor_nonce(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()
            def write(path,value):
                target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(json.dumps(value));return ref(root,path)
            old=write('.build/old/execution.json',{'nonce':'b'*32})
            packet=write('.build/old/packet.json',{'red_execution':old})
            policy={'receipts':{'plan':'oldplan','gates':'oldgate','implementation_admission':'oldadmit'},'resume_epoch':{
                'kind':'admitted-p3-plan-gates-resumption/v1','schema_version':1,'epoch_id':'r3','product_completion':'incomplete','next_stage_allowed':False,
                'ancestors':{'gates_r2':{'packet':packet}},'receipts':{
                    'plan':'.build/binding-journey-alignment-20261109/phase-1-r3/plan-review.json',
                    'gates':'.build/binding-journey-alignment-20261109/phase-2-r3/gate-review.json',
                    'implementation_admission':'.build/binding-journey-alignment-20261109/phase-3-r3/admission.json',
                    'gate_packet':'.build/binding-journey-alignment-20261109/phase-2-r3/packet/packet.json'}}}
            write(j.POLICY,policy)
            claim=j.claim_resumption_nonce(root,'a'*32,'.build/run',{})
            saved=(root/claim['path']).read_bytes()
            with self.assertRaises(FileExistsError):j.claim_resumption_nonce(root,'a'*32,'.build/another',{})
            self.assertEqual((root/claim['path']).read_bytes(),saved)
            with self.assertRaises(ValueError):j.claim_resumption_nonce(root,'b'*32,'.build/new',{})

    def test_real_lease_budget_and_release_order_are_recomputed(self):
        lease={'lease_id':'old','owner_kind':'session','owner_id':'S1','generation':'4','fence':'F','epoch':2,'filter':'filter','paused':False}
        name=j.FIXED_NATIVE_UNIT_GROUPS['EX-LEASE-NATIVE-UNIT'][0]
        kinds=[('lease_table_mutation',{'active':2,'draining_pending':1}),('retirement_mark',lease),('guard_exit',{}),
               ('collector_stop_dispatch',{}),('release_ack',lease),('session_admitted',dict(lease,lease_id='new')),
               ('call_counters',{'collector_start':1,'filter_start':1})]
        raw={'kind':'binding-preview-observations/v1','schema_version':1,'nonce':'a'*32,'source_tree_sha256':tree_sha({}),'test_name':name,'pid':42,
             'events':[{'sequence':n,'monotonic_ns':n,'wall_ms':2000,'thread_id':'ThreadId(1)','branch':'retirement_admission','kind':kind,'data':copy.deepcopy(data)} for n,(kind,data) in enumerate(kinds,1)]}
        execution={'pid':42,'started_at':'1970-01-01T00:00:01+00:00','finished_at':'1970-01-01T00:00:03+00:00'}
        def verdict(value):return i.inspect_preview_observations('LUNA_BINDING_PREVIEW_OBSERVATIONS_V1 '+json.dumps(value),name,execution,'a'*32,{})['status']
        self.assertEqual(verdict(raw),'passed')
        for mutation in ('budget','blocking_stop','release_before_guard','wrong_ack'):
            bad=copy.deepcopy(raw)
            if mutation=='budget':bad['events'][0]['data']['active']=3
            if mutation=='blocking_stop':bad['events'][2]['kind'],bad['events'][3]['kind']=bad['events'][3]['kind'],bad['events'][2]['kind']
            if mutation=='release_before_guard':bad['events'][2]['kind'],bad['events'][4]['kind']=bad['events'][4]['kind'],bad['events'][2]['kind']
            if mutation=='wrong_ack':bad['events'][4]['data']['fence']='new-candidate-fence'
            self.assertEqual(verdict(bad),'failed',mutation)

    def test_historical_workspace_dependent_rust_is_not_reexecuted_or_marked_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();target=root/'.build/trace.json';target.parent.mkdir()
            target.write_text(json.dumps({'observations':{'transport_mode':'unavailable','blocked_obligations':['producer-ready']},'events':[]}))
            case={'id':j.COMPONENT_CASES[0],'status':'blocked','assertions':[],'trace':ref(root,'.build/trace.json'),'dom':None,'screenshot':None}
            values={'nonce':'a'*32}
            bundle={'producer':{'values':values,'binary':'pure-serializer','manifest':{'runtime_transport':{'mode':'ndjson'}}},
                    'report':{'cases':[case]},'actual_rpc':{'ready':None},'rust_layers':{'path':'original-frozen-rust'},
                    'source_map':{},'rust_replay_disposition':'not_run_historical_workspace_dependent'}
            with (patch.object(r.subprocess,'run',return_value=subprocess.CompletedProcess([],0,json.dumps(values),'')),
                  patch.object(r,'recompute_scene_layout',return_value={'errors':[],'executed_steps':0}),
                  patch.object(i,'inspect_rust_layers') as rust):
                result=r.recompute(root,bundle);rust.assert_not_called()
            self.assertEqual(result['errors'],[])
            self.assertEqual(result['rust_replay'],'not_run_historical_workspace_dependent')
            bundle['rust_replay_disposition']='required_current_tree'
            with (patch.object(r.subprocess,'run',return_value=subprocess.CompletedProcess([],0,json.dumps(values),'')),
                  patch.object(r,'recompute_scene_layout',return_value={'errors':[],'executed_steps':0}),
                  patch.object(i,'inspect_rust_layers',return_value={'infrastructure_errors':[]}) as rust):
                r.recompute(root,bundle);self.assertIs(rust.call_args.kwargs['replay'],True)


class NativeObservationGuardTests(unittest.TestCase):
    """Synthetic verifier controls only; never a signed-package Native run."""
    def source(self, sid='sid', identity='s1', ordinal=1):
        return {'source_id': identity, 'session_id': sid, 'hook_address_u64': '18446744073709551615',
                'context_u64': str(ordinal), 'subcontext_u64': '0', 'hook_name': 'Hook', 'hook_code': 'HW',
                'encoding': 'UTF-16', 'stream_identity_sha256': str(ordinal)*64,
                'identity_extensions_json': '{}', 'discovered_at_ms': 1080}

    def event(self, sequence, action, context=None, **values):
        row = {'sequence': sequence, 'action': action, 'observed_at_ms': 1000+sequence*10,
               'correlation_id': 'read-1', 'review_id': None, 'session_id': None, 'operation_id': None,
               'selection_set_revision': None, 'active_target_identity_sha256': None,
               'candidate_target_identity_sha256': None}
        if context:
            row.update(context)
        row.update(values)
        return row

    def base(self):
        context = {'review_id': 'review', 'session_id': 'sid', 'operation_id': None,
                   'selection_set_revision': None, 'active_target_identity_sha256': 'a'*64,
                   'candidate_target_identity_sha256': 'a'*64}
        return {'id': 'before', 'context': context, 'process': {'pid': 123, 'start_sec': 1, 'start_usec': 2, 'executable': '/actual/Luna'},
                'main': {'pid': 123, 'window_id': 10, 'on_screen': True},
                'hud': {'pid': 123, 'window_id': 20, 'on_screen': False},
                'os': {'observed_at_ms': 1015, 'boot_sec': 1, 'boot_usec': 0},
                'tables': {'profile_meta': [{'profile_id':'profile','server_instance_id':'server','runtime_generation':3,'runtime_state':'active'}],
                           'sessions':[{'session_id':'sid','game_id':'game','target_identity_sha256':'a'*64}],
                           'capture_sources':[self.source(),self.source(identity='s2',ordinal=2)],
                           'capture_operations':[], 'hook_events':[], 'source_selection_sets':[],
                           'source_selection_members':[], 'source_selections':[]},
                'calls': [], 'journal':[self.event(1,'context_transition',context,correlation_id='initial')],
                'raw':{'journal_end_sequence':1},'text':'选择正文 开始阅读 记住选择','frames':[],'native_actions':[]}

    def append_selection(self, previous, op='read-1', revision=1, remember=False, start=1020):
        step = copy.deepcopy(previous)
        context = dict(step['context'],operation_id=op,selection_set_revision=str(revision))
        profile = i.native_profile(step)
        selected = {'sessionId':'sid','setRevision':str(revision),'operationId':op,
                    'persistence':'saved_snapshot' if remember else 'temporary','selectedAtMs':str(start),
                    'members':[{'sourceId':f's{n}','selectionRevision':2*(revision-1)+n,'ordinal':n-1} for n in (1,2)]}
        descriptors = [{'sourceId':s['source_id'],'streamIdentitySha256':s['stream_identity_sha256'],'hookName':s['hook_name'],
                        'hookCode':s['hook_code'],'encoding':s['encoding'],'address':s['hook_address_u64'],
                        'context':s['context_u64'],'context2':s['subcontext_u64'],'identityExtensionsJson':s['identity_extensions_json']}
                       for s in step['tables']['capture_sources']]
        snapshot = json.dumps({'schemaVersion':1,'matchCapability':'requires_review','targetIdentitySha256':'a'*64,'sources':descriptors})
        step['tables']['source_selection_sets'].append({'session_id':'sid','operation_id':op,'selection_set_revision':revision,
                'origin_kind':'explicit_set','state':'sealed','persistence_kind':selected['persistence'],
                'selected_at_ms':start,'member_count':2,'match_snapshot_json':snapshot})
        for member in selected['members']:
            step['tables']['source_selection_members'].append({'session_id':'sid','selection_set_revision':revision,
                    'source_id':member['sourceId'],'selection_revision':member['selectionRevision'],'member_ordinal':member['ordinal']})
            step['tables']['source_selections'].append({'session_id':'sid','source_id':member['sourceId'],
                    'selection_revision':member['selectionRevision'],'persistence_kind':selected['persistence'],'match_snapshot_json':snapshot})
        authority = {'profile':profile,'sessionId':'sid','setRevision':str(revision),'operationId':op}
        receipt = {'profile':profile,'operationId':op,'selection':selected,'replayed':False,'effectAuthority':authority}
        result = {'selection':receipt,'effects':{'authority':authority,'showHud':{'state':'succeeded','error':None},
                                               'hideMain':{'state':'succeeded','error':None},'retryable':False}}
        args = {'operationId':op,'sessionId':'sid','expectedProfileGeneration':3,
                'expectedSetRevision':None if revision==1 else str(revision-1),'sourceIds':['s1','s2'],'remember':remember}
        sequence = len(step['journal'])+1
        for offset, values in enumerate([
            dict(action='port_call',method='beginReading',args=args),
            dict(action='context_transition'), dict(action='persist_commit',response=receipt),
            dict(action='show_hud_completed',effect='show_hud',error_code=None),
            dict(action='hide_main_completed',effect='hide_main',error_code=None),
            dict(action='port_return',method='beginReading',response=result),
        ]):
            row = self.event(sequence+offset, values.pop('action'), context if offset else None,
                             correlation_id=op,observed_at_ms=start+10*offset,**values)
            step['journal'].append(row)
        step['context'] = context
        step['main']['on_screen'] = False; step['hud']['on_screen'] = True
        step['os']['observed_at_ms'] = start+60
        step['raw']['journal_end_sequence'] = step['journal'][-1]['sequence']
        step['calls'] = [e for e in step['journal'] if e['action'] in {'port_call','port_return'}]
        step['native_actions'] = []
        return step

    def pair(self):
        before=self.base(); after=self.append_selection(before); after['id']='completed'
        return before,after

    def test_bj13_actual_before_completed_full_receipt_is_valid(self):
        self.assertEqual(i.native_semantics('BJ-13',list(self.pair())),[])

    def test_current_full_authority_and_db_mutations_fail(self):
        mutations = [
            lambda s:s['tables']['profile_meta'][0].update(profile_id='other'),
            lambda s:s['tables']['profile_meta'][0].update(server_instance_id='other'),
            lambda s:s['tables']['profile_meta'][0].update(runtime_generation=4),
            lambda s:s['context'].update(session_id='other'),
            lambda s:s['context'].update(operation_id='old'),
            lambda s:s['context'].update(selection_set_revision='9'),
            lambda s:s['tables']['source_selection_members'].pop(),
            lambda s:s['tables']['source_selections'][0].update(source_id='wrong'),
            lambda s:s['tables']['sessions'][0].update(target_identity_sha256='f'*64),
            lambda s:s['journal'][3]['response']['effectAuthority']['profile'].update(profileGeneration=9),
        ]
        for mutate in mutations:
            with self.subTest(mutation=mutations.index(mutate)):
                a,b=self.pair();mutate(b)
                self.assertTrue(i.native_semantics('BJ-13',[a,b]))

    def test_manual_hide_after_return_cannot_repair_automatic_effect(self):
        a,b=self.pair();hide=b['journal'].pop(-2)
        hide.update(sequence=8,observed_at_ms=1075);b['journal'].append(hide)
        self.assertTrue(i.native_semantics('BJ-13',[a,b]))

    def test_same_operation_stale_context_or_wrong_effect_order_fails(self):
        for change in ('context','order','retry'):
            a,b=self.pair()
            if change=='context':b['journal'][4]['session_id']='other'
            elif change=='order':b['journal'][4]['sequence'],b['journal'][5]['sequence']=6,5
            else:b['journal'][-1]['response']['effects']['authority']['setRevision']='99'
            self.assertTrue(i.native_semantics('BJ-13',[a,b]))

    def test_show_failure_retries_only_same_authority_without_recommit(self):
        a,b=self.pair();context=b['context'];authority=b['journal'][-1]['response']['effects']['authority']
        success=copy.deepcopy(b['journal'][-1]['response']['effects'])
        failure={'code':'show_failed','stage':'window','message':'show failed'}
        b['journal'][4].update(action='window_effect_failed',effect='show_hud',error_code='show_failed')
        b['journal'].pop(5)
        b['journal'][-1]['response']['effects'].update(showHud={'state':'failed','error':failure},hideMain={'state':'not_attempted','error':None},retryable=True)
        b['journal'][-1]['sequence']=6
        b['journal'] += [self.event(7,'port_call',context,method='retryReadingWindowEffects',args=authority,observed_at_ms=1080),
                         self.event(8,'show_hud_completed',context,effect='show_hud',error_code=None,observed_at_ms=1090),
                         self.event(9,'hide_main_completed',context,effect='hide_main',error_code=None,observed_at_ms=1100),
                         self.event(10,'port_return',context,method='retryReadingWindowEffects',response=success,observed_at_ms=1110)]
        b['os']['observed_at_ms']=1120;b['raw']['journal_end_sequence']=10
        self.assertEqual(i.native_semantics('BJ-13',[a,b]),[])
        b['journal'][6]['args']=dict(authority,setRevision='99')
        self.assertTrue(i.native_semantics('BJ-13',[a,b]))

    def test_wrong_os_cohort_and_known_main_fail(self):
        for mutate in (lambda s:s['main'].update(window_id=99),lambda s:s['main'].update(on_screen=True),
                       lambda s:s['process'].update(pid=999),lambda s:s['os'].update(boot_sec=8),
                       lambda s:s['hud'].update(window_id=77),lambda s:s['hud'].update(on_screen=False)):
            a,b=self.pair();mutate(b)
            self.assertTrue(i.native_semantics('BJ-13',[a,b]))

    def test_only_registered_outcomes_allow_known_hidden_main(self):
        observed={'windows':[{'window_id':10,'pid':123,'on_screen':True},
                              {'window_id':20,'pid':123,'on_screen':False}]}
        known={};step={'id':'before','main_window_id':10,'hud_window_id':20}
        i.native_windows(observed,step,'BJ-13',123,{10,20},known)
        observed['windows']=[{'window_id':20,'pid':123,'on_screen':True,'capture':{'png':'hud.png','ocr':[{'text':'字幕'}]}}]
        step['id']='completed'
        self.assertFalse(i.native_windows(observed,step,'BJ-13',123,{10,20},known)[0]['on_screen'])
        with self.assertRaises(ValueError):i.native_windows(observed,step,'BJ-13',123,{10,20},{})
        step['id']='dialog_default'
        with self.assertRaises(ValueError):i.native_windows(observed,step,'BJ-14',123,{10,20},known)

    def test_native_role_flags_and_extra_steps_do_not_grant_exemption(self):
        for key in ('role','observation_role','allow_main_absent','branch','milestones'):
            with self.assertRaises(ValueError):i.native_inventory('BJ-13',[{'id':'before'},{'id':'completed',key:'hud'}])
        with self.assertRaises(ValueError):i.native_inventory('BJ-06',[{'id':'candidate'},{'id':'waiting'}])
        with self.assertRaises(ValueError):i.native_role('BJ-14','completed')

    def test_full_watermark_not_free_milestones(self):
        _a,b=self.pair();events=b['journal'];bounds={'start_sequence':1,'end_sequence':7}
        self.assertEqual(len(i.native_closed_events(events,bounds,1080,7)),7)
        for watermark in (True,0,3,8):
            with self.assertRaises(ValueError):i.native_closed_events(events,bounds,1080,watermark)
        events[2]['observed_at_ms']=2000
        with self.assertRaises(ValueError):i.native_closed_events(events,bounds,1080,7)

    def test_original_journal_health_correlation_and_raw_tail_are_strict(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);directory=root/'.build/native';directory.mkdir(parents=True)
            path=directory/'journal.jsonl';source={'fixture':'a'*64}
            shared={'nonce':'b'*32,'pid':123,'profile_id':'profile','boot_sec':1,'boot_usec':0}
            header=dict(shared,kind='binding_journal_begin',schema_version=1,source_tree_sha256=tree_sha(source))
            events=[dict(shared,**e,kind='binding_journal_event') for e in self.pair()[1]['journal']]
            health=dict(shared,kind='binding_journal_health',accepted=7,written=7,last_sequence=7,dropped=0,invalid=0,write_failures=0)
            rows=[header]+events+[health]
            manifest={'main_pid':123,'profile_id':'profile','started_at_ms':1000,'finished_at_ms':2000}
            def write(values,tail='\n'):
                path.write_text('\n'.join(json.dumps(x) for x in values)+tail)
                manifest['binding_journal']=[ref(root,'.build/native/journal.jsonl')]
            write(rows)
            self.assertEqual(len(i.inspect_binding_journal(root,manifest,directory,'b'*32,source)[1]),7)
            for mutate in (lambda r:r[-1].update(dropped=1),lambda r:r[-1].update(invalid=1),
                           lambda r:r[-1].update(write_failures=1),lambda r:r[-1].update(written=6),
                           lambda r:r[4].update(session_id='other'),lambda r:r[2].update(sequence=3),
                           lambda r:r[3].update(nonce='old'),lambda r:r[1].update(milestones={'passed':True})):
                changed=copy.deepcopy(rows);mutate(changed);write(changed)
                with self.assertRaises(ValueError):i.inspect_binding_journal(root,manifest,directory,'b'*32,source)
            write(rows,tail='')
            with self.assertRaises(ValueError):i.inspect_binding_journal(root,manifest,directory,'b'*32,source)

    def progress(self, waiting=False):
        candidate=self.base();candidate['id']='candidate';candidate['os']['observed_at_ms']=1160
        candidate['context']['operation_id']='association-op'
        candidate['tables']['capture_operations']=[{'operation_id':'association-op','session_id':'sid'}]
        hook=[{'event_id':'h1','event_sequence':1,'observed_at_ms':1010,'code':'host_ready','level':'info','message':'host','operation_id':'association-op','session_id':'sid'},
              {'event_id':'h2','event_sequence':2,'observed_at_ms':1020,'code':'process_connected','level':'info','message':'connected','operation_id':'association-op','session_id':'sid'},
              {'event_id':'h3','event_sequence':3,'observed_at_ms':1080,'code':'source_discovered','level':'info','message':'source','operation_id':'association-op','session_id':'sid'}]
        candidate['tables']['hook_events']=hook
        profile=i.native_profile(candidate)
        def response(stage,rows):return {'operationId':'association-op','profile':profile,'sessionId':'sid','generation':'e'*64,
                'stage':stage,'superseded':False,'error':None,'facts':[{'eventId':h['event_id'],'sequence':str(h['event_sequence']),
                'observedAtMs':str(h['observed_at_ms']),'code':h['code'],'level':h['level'],'message':h['message']} for h in rows]}
        events=[]
        if waiting:
            events=[self.event(1,'port_call',method='associationProgress',args={'operationId':'association-op','expectedProfileGeneration':3},correlation_id='p0',observed_at_ms=1030),
                    self.event(2,'port_return',method='associationProgress',response=response('waiting_text',hook[:2]),correlation_id='p0',observed_at_ms=1040)]
        n=len(events)+1
        events += [self.event(n,'port_call',method='associationProgress',args={'operationId':'association-op','expectedProfileGeneration':3},correlation_id='p1',observed_at_ms=1100),
                   self.event(n+1,'port_return',method='associationProgress',response=response('candidates_ready',hook),correlation_id='p1',observed_at_ms=1110)]
        candidate['journal']=events
        self.source_read(candidate,1120)
        if not waiting:return [candidate]
        earlier=copy.deepcopy(candidate);earlier['id']='waiting';earlier['text']='等待游戏正文'
        earlier['os']['observed_at_ms']=1050;earlier['journal']=events[:2];earlier['tables']['capture_sources']=[]
        earlier['tables']['hook_events']=hook[:2];earlier['raw']['journal_end_sequence']=2
        return [earlier,candidate]

    def source_read(self,step,start):
        profile=i.native_profile(step);sid=step['context']['session_id']
        sources=[{'sourceId':s['source_id'],'streamIdentitySha256':s['stream_identity_sha256'],'hookName':s['hook_name'],
                  'hookCode':s['hook_code'],'encoding':s['encoding'],'address':s['hook_address_u64'],'context':s['context_u64'],
                  'context2':s['subcontext_u64'],'identityExtensionsJson':s['identity_extensions_json']} for s in step['tables']['capture_sources']]
        sets=step['tables']['source_selection_sets']
        current=i.native_selection_receipt(step)['receipt']['selection'] if sets else None
        projection={'profile':profile,'sessionId':sid,'sources':sources,'current':current,
                    'rememberAllowed':bool(step['tables']['sessions'][0].get('game_id'))}
        n=len(step['journal'])+1;corr='source-'+str(n)
        step['journal'] += [self.event(n,'port_call',step['context'],method='sourceSelection',args={'sessionId':sid,'expectedProfileGeneration':3},correlation_id=corr,observed_at_ms=start),
                            self.event(n+1,'port_return',step['context'],method='sourceSelection',response=projection,correlation_id=corr,observed_at_ms=start+10)]
        step['raw']['journal_end_sequence']=n+1
        step['calls']=[e for e in step['journal'] if e['action'] in {'port_call','port_return'}]

    def test_bj06_actual_wait_and_fast_paths_are_both_valid(self):
        for waiting in (False,True):
            steps=self.progress(waiting)
            self.assertEqual(i.native_progress_trace(steps),'waiting-observed' if waiting else 'fast-candidates')
            self.assertEqual(i.native_semantics('BJ-06',steps),[])

    def test_fast_without_real_hook_or_current_source_fails(self):
        mutations=[lambda s:s['tables']['hook_events'].pop(0),
                   lambda s:s['tables']['capture_operations'][0].update(operation_id='old'),
                   lambda s:s['tables']['capture_sources'][0].update(source_id='old-source'),
                   lambda s:s['tables']['profile_meta'][0].update(runtime_generation=9),
                   lambda s:s['journal'][-1]['response']['sources'][0].update(address=18446744073709551615),
                   lambda s:s['journal'][1]['response']['facts'][0].update(sequence='999')]
        for mutate in mutations:
            steps=self.progress();mutate(steps[-1])
            self.assertTrue(i.native_semantics('BJ-06',steps))

    def test_unrelated_wait_word_and_premature_reading_fail(self):
        steps=self.progress(True);steps[0]['text']='预览等待 选择正文'
        self.assertTrue(i.native_semantics('BJ-06',steps))
        steps=self.progress();steps[-1]['journal'].append(self.event(5,'port_call',method='beginReading'))
        self.assertTrue(i.native_semantics('BJ-06',steps))

    def reopen(self,previous,name,start):
        step=copy.deepcopy(previous);step['id']=name;step['main']['on_screen']=True
        step['native_actions']=[{'id':'normal_show_main','started_at_ms':start,'finished_at_ms':start+10},
                                {'id':'open_current_sources','started_at_ms':start+20,'finished_at_ms':start+30}]
        self.source_read(step,start+40);step['os']['observed_at_ms']=start+60
        return step

    def remember(self):
        before=self.append_selection(self.base(),remember=True);before['id']='before';before['main']['on_screen']=True;before['os']['observed_at_ms']=1110
        temporary=self.append_selection(before,'temporary-op',2,False,1120);temporary['id']='temporary'
        intent=self.reopen(temporary,'save_intent',1190)
        saved=self.append_selection(intent,'save-op',3,True,1260);saved['id']='saved'
        reopened=self.reopen(saved,'saved_reopened',1330)
        unowned=copy.deepcopy(reopened);unowned['id']='unowned';unowned['context']['session_id']='unowned'
        unowned['context']['selection_set_revision']=None;unowned['context']['operation_id']='unowned-op'
        unowned['tables']['sessions']=[{'session_id':'unowned','game_id':None,'target_identity_sha256':'a'*64}]
        for key in ('source_selection_sets','source_selection_members','source_selections'):unowned['tables'][key]=[]
        for s in unowned['tables']['capture_sources']:s['session_id']='unowned'
        self.source_read(unowned,1400);unowned['os']['observed_at_ms']=1430;unowned['native_actions']=[]
        return [before,temporary,intent,saved,reopened,unowned]

    def test_saved_outcomes_and_normal_reopen_false_new_draft_are_valid(self):
        steps=self.remember();steps[4]['raw']['draft_checkbox_checked']=False
        self.assertEqual(i.native_semantics('BJ-09',steps),[])

    def test_reopen_without_actual_actions_or_with_extra_commit_fails(self):
        for mutation in ('action','authority','db','effects','cached'):
            steps=self.remember();target=steps[4]
            if mutation=='action':target['native_actions']=[]
            elif mutation=='authority':target['context']['operation_id']='other'
            elif mutation=='db':target['tables']['source_selection_members'].pop()
            elif mutation=='effects':target['journal'].append(self.event(24,'show_hud_completed',effect='show_hud'))
            else:target['journal']=target['journal'][:-2]
            self.assertTrue(i.native_semantics('BJ-09',steps))

    def test_remember_labels_empty_baseline_and_borrowed_game_fail(self):
        for mutation in ('baseline','intent','descriptor','borrowed','downgrade'):
            steps=self.remember()
            if mutation=='baseline':steps[0]['tables']['source_selection_sets']=[]
            elif mutation=='intent':
                next(e for e in steps[3]['journal'] if e.get('method')=='beginReading' and e['action']=='port_call' and e['args']['operationId']=='save-op')['args']['remember']=False
            elif mutation=='descriptor':steps[3]['tables']['source_selection_sets'][-1]['match_snapshot_json']='{}'
            elif mutation=='borrowed':steps[-1]['tables']['sessions'][0]['game_id']='borrowed'
            else:
                n=len(steps[-1]['journal'])+1
                steps[-1]['journal'] += [self.event(n,'port_call',method='beginReading',args={'sessionId':'unowned','remember':True},correlation_id='bad'),
                                        self.event(n+1,'port_return',method='beginReading',response={'temporary':True},correlation_id='bad')]
            self.assertTrue(i.native_semantics('BJ-09',steps))

    def test_missing_real_producers_remain_blocked_not_guard_pass(self):
        rows=i.inspect_native_obligations(Path('.'),None,'a'*32,{})
        self.assertEqual({r['status'] for r in rows},{'blocked'})
        self.assertEqual(len(rows),3)

    def shared_native_corpus(self):
        cases=[]
        def add(name,case,steps):cases.append({'id':name,'case_id':case,'steps':steps})
        a,b=self.pair();add('BJ13-current-before-completed','BJ-13',[a,b])
        a,b=self.pair();draft=copy.deepcopy(a);draft['id']='draft';b['id']='committed';add('BJ08-current-draft-committed','BJ-08',[a,draft,b])
        for waiting in (False,True):add('BJ06-waiting' if waiting else 'BJ06-fast','BJ-06',self.progress(waiting))
        add('BJ09-saved-reopen-new-draft','BJ-09',self.remember())
        a,b=self.pair();b['main']['window_id']=99;add('unobserved-Main','BJ-13',[a,b])
        a,b=self.pair();b['hud']['pid']=999;add('wrong-HUD-process','BJ-13',[a,b])
        a,b=self.pair();b['tables']['profile_meta'][0]['runtime_generation']=9;add('wrong-profile-generation','BJ-13',[a,b])
        a,b=self.pair();b['context']['operation_id']='old';add('wrong-operation','BJ-13',[a,b])
        a,b=self.pair();hide=b['journal'].pop(-2);hide.update(sequence=8,observed_at_ms=1075);b['journal'].append(hide);add('manual-hide-after-return','BJ-13',[a,b])
        a,b=self.pair();b['journal'][4]['session_id']='other';add('wrong-effect-session','BJ-13',[a,b])
        a,b=self.pair();b['journal'][-1]['response']['effects']['authority']['setRevision']='9';add('wrong-set-authority','BJ-13',[a,b])
        a,b=self.pair();b['observation_role']='hud';add('self-assigned-role','BJ-13',[a,b])
        a,b=self.pair();b['id']='main_hidden';add('unregistered-step','BJ-13',[a,b])
        s=self.progress();s[-1]['tables']['hook_events'].pop(0);add('fake-fast-no-handshake','BJ-06',s)
        s=self.progress();s[-1]['tables']['capture_sources'][0]['source_id']='old';add('wrong-source','BJ-06',s)
        s=self.progress(True);s[0]['text']='其他等待 选择正文';add('unrelated-wait','BJ-06',s)
        s=self.progress();s[-1]['journal'][-1]['response']['sources'][0]['address']=18446744073709551615;add('rounded-u64','BJ-06',s)
        s=self.progress();s[-1]['journal'].append(self.event(5,'port_call',method='beginReading'));add('premature-begin','BJ-06',s)
        s=self.remember();s[4]['native_actions']=[];add('invented-reopen','BJ-09',s)
        s=self.remember();s[4]['journal'].append(self.event(24,'hide_main_completed',effect='hide_main'));add('reopen-repeat-effect','BJ-09',s)
        s=self.remember();s[0]['tables']['source_selection_sets']=[];add('empty-saved-baseline','BJ-09',s)
        s=self.remember();s[3]['tables']['source_selection_sets'][-1]['match_snapshot_json']='{}';add('label-without-saved-descriptor','BJ-09',s)
        s=self.remember();s[-1]['tables']['sessions'][0]['game_id']='borrowed';add('unowned-borrowed-game','BJ-09',s)
        return cases

    def test_node_independently_agrees_on_same_raw_native_corpus(self):
        root=Path(__file__).resolve().parents[2]
        cases=self.shared_native_corpus()
        script=('import fs from "node:fs";import {pathToFileURL} from "node:url";'
                'const {nativeObservationVerdict}=await import(pathToFileURL(process.argv[2]).href);'
                'const rows=JSON.parse(fs.readFileSync(0,"utf8"));'
                'process.stdout.write(JSON.stringify(rows.map(r=>({id:r.id,...nativeObservationVerdict(r.case_id,r.steps)}))));')
        run=subprocess.run(['node','--input-type=module','--eval',script,'native-guard-oracle',str(root/'scripts/report/verify_binding_journey.mjs')],
                           input=json.dumps(cases),text=True,capture_output=True,cwd=root,timeout=30)
        self.assertEqual(run.returncode,0,run.stderr)
        observed=json.loads(run.stdout)
        self.assertEqual([r['id'] for r in observed],[r['id'] for r in cases])
        for raw,node in zip(cases,observed):
            with self.subTest(case=raw['id']):
                try:
                    i.native_inventory(raw['case_id'],raw['steps'])
                    errors=i.native_semantics(raw['case_id'],raw['steps'])
                except (ValueError,KeyError,TypeError) as error:errors=[str(error)]
                self.assertEqual(node['status'],'failed' if errors else 'passed')
        self.assertEqual(sum(r['status']=='passed' for r in observed),5)

    def test_node_raw_consumer_rejects_free_pass_before_native_or_sqlite_actions(self):
        root=Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as name:
            directory=Path(name);file=directory/'fake.json';file.write_text('{"status":"passed","native_verified":true}')
            import hashlib
            payload={'root':name,'reference':{'path':'fake.json','sha256':hashlib.sha256(file.read_bytes()).hexdigest()},
                     'nonce':'a'*32,'source_tree_sha256':'b'*64}
            script=('import fs from "node:fs";import {pathToFileURL} from "node:url";'
                    'const {inspectNativeRawCollection}=await import(pathToFileURL(process.argv[2]).href);'
                    'try{await inspectNativeRawCollection(JSON.parse(fs.readFileSync(0,"utf8")));process.exitCode=0;}'
                    'catch(e){process.stdout.write(String(e.message));process.exitCode=3;}')
            run=subprocess.run(['node','--input-type=module','--eval',script,'native-guard-oracle',str(root/'scripts/report/verify_binding_journey.mjs')],
                               input=json.dumps(payload),text=True,capture_output=True,cwd=root,timeout=30)
            self.assertEqual(run.returncode,3,run.stderr)
            self.assertIn('source/nonce/origin',run.stdout)

    def committed_failure_retry(self):
        _initial,b=self.pair();context=b['context']
        success=copy.deepcopy(b['journal'][-1]['response']['effects'])
        b['journal'][4].update(action='window_effect_failed',effect='show_hud',error_code='show_failed')
        b['journal'].pop(5)
        b['journal'][-1]['response']['effects'].update(showHud={'state':'failed','error':{'code':'show_failed','stage':'window','message':'failed'}},hideMain={'state':'not_attempted','error':None},retryable=True)
        b['journal'][-1]['sequence']=6;b['raw']['journal_end_sequence']=6
        b['main']['on_screen']=True;b['hud']['on_screen']=False;b['os']['observed_at_ms']=1080
        b['id']='before';a=copy.deepcopy(b)
        b['id']='completed';b['main']['on_screen']=False;b['hud']['on_screen']=True;b['os']['observed_at_ms']=1140
        b['journal'] += [self.event(7,'port_call',context,method='retryReadingWindowEffects',args=success['authority'],observed_at_ms=1090),
                         self.event(8,'show_hud_completed',context,effect='show_hud',error_code=None,observed_at_ms=1100),
                         self.event(9,'hide_main_completed',context,effect='hide_main',error_code=None,observed_at_ms=1110),
                         self.event(10,'port_return',context,method='retryReadingWindowEffects',response=success,observed_at_ms=1120)]
        b['raw']['journal_end_sequence']=10
        return a,b

    def large_revision_pair(self,revision=2**53+1):
        a,b=self.pair()
        old=copy.deepcopy(b['tables']['source_selection_sets'][0]);old.update(selection_set_revision=str(revision-1),operation_id='old-before',selected_at_ms=1000)
        old_members=copy.deepcopy(b['tables']['source_selection_members'])
        for m in old_members:m['selection_set_revision']=str(revision-1)
        old_selections=copy.deepcopy(b['tables']['source_selections'])
        def replace(value):
            if isinstance(value,dict):
                for key,item in list(value.items()):
                    if key=='selection_set_revision' and item in (1,'1'):value[key]=str(revision)
                    elif key=='setRevision' and item=='1':value[key]=str(revision)
                    elif key=='selection_revision' and item in (1,2):value[key]=item+2
                    elif key=='selectionRevision' and item in (1,2):value[key]=item+2
                    elif key=='expectedSetRevision':value[key]=str(revision-1)
                    else:replace(item)
            elif isinstance(value,list):
                for child in value:replace(child)
        # JSON round-trip removes aliasing shared by fixture request/return objects.
        b=json.loads(json.dumps(b));replace(b)
        b['tables']['source_selection_sets'].insert(0,old)
        b['tables']['source_selection_members']=old_members+b['tables']['source_selection_members']
        b['tables']['source_selections']=old_selections+b['tables']['source_selections']
        a['tables']['source_selection_sets']=[copy.deepcopy(old)]
        a['tables']['source_selection_members']=old_members;a['tables']['source_selections']=old_selections
        a['context'].update(operation_id='old-before',selection_set_revision=str(revision-1))
        a['journal'][0].update(a['context'])
        return a,b

    def closure_corpus(self):
        cases=[]
        def add(name,steps):cases.append({'id':name,'case_id':'BJ-13','steps':list(steps)})
        _a,b=self.pair();a=copy.deepcopy(b);a['id']='before';a['main']['on_screen']=True;a['hud']['on_screen']=False
        a['os']['observed_at_ms']=1090;b['os']['observed_at_ms']=1120;add('old-success-before-observation',(a,b))
        a,b=self.pair();new=copy.deepcopy(b['tables']['source_selection_sets'][0]);new.update(selection_set_revision=2,operation_id='new-op')
        b['tables']['source_selection_sets'].append(new);add('old-head-later-db',(a,b))
        add('committed-failure-fresh-effect-retry',self.committed_failure_retry())
        add('lossless-large-current-set',self.large_revision_pair())
        a,b=self.large_revision_pair();new=copy.deepcopy(b['tables']['source_selection_sets'][-1]);new.update(selection_set_revision=str(2**53+2),operation_id='adjacent-new-head');b['tables']['source_selection_sets'].append(new);add('adjacent-large-head',(a,b))
        a,b=self.pair();a['database_owner']={'pid':123,'path':'/db-one','inode':'11'};b['database_owner']={'pid':123,'path':'/db-two','inode':'12'};add('cross-database-snapshots',(a,b))
        a,b=self.pair();head=copy.deepcopy(b['tables']['source_selection_sets'][0]);head.update(selection_set_revision=2,operation_id='before-other');a['tables']['source_selection_sets']=[head];add('before-cas-head-changed',(a,b))
        a,b=self.committed_failure_retry();b['journal'][-1]['response']['authority']['profile']['serverInstanceId']='other';add('retry-other-profile',(a,b))
        return cases

    def test_reviewed_old_commit_and_noncurrent_head_are_rejected(self):
        rows=self.closure_corpus()
        for raw in rows[:2]:self.assertTrue(i.native_semantics(raw['case_id'],raw['steps']),raw['id'])
        self.assertEqual(i.native_semantics('BJ-13',list(self.committed_failure_retry())),[])

    def test_lossless_large_head_and_cross_snapshot_regressions(self):
        cases=self.closure_corpus()
        for raw in cases[3:]:
            errors=i.native_semantics(raw['case_id'],raw['steps'])
            self.assertEqual(bool(errors),raw['id']!='lossless-large-current-set',(raw['id'],errors))

    def fenced_sampling(self, step=None, *, no_session=False):
        """Raw synthetic controls; no command or signed Native execution claimed."""
        import base64
        step=copy.deepcopy(step or self.base())
        if no_session:
            step['context']=dict.fromkeys(step['context'])
            step['tables']['sessions']=[]
        step['tables']['profile_meta'][0]['commit_revision']=7
        cohort={'nonce':'b'*32,'pid':123,'profile_id':'profile','boot_sec':1,'boot_usec':0}
        header=dict(cohort,kind='binding_journal_begin',schema_version=1,source_tree_sha256='a'*64)
        events=[dict(event,**cohort,kind='binding_journal_event') for event in step['journal']]
        context=step['context'];profile=i.native_profile(step);correlation='actual-query-correlation'
        method='snapshot' if no_session else 'sourceSelection'
        args={} if no_session else {'sessionId':context['session_id'],'expectedProfileGeneration':profile['profileGeneration']}
        current=None;head=i.native_current_head(step)
        if head:
            members=sorted((m for m in step['tables']['source_selection_members'] if m['selection_set_revision']==head['selection_set_revision']),key=lambda m:m['member_ordinal'])
            current={'sessionId':context['session_id'],'setRevision':str(head['selection_set_revision']),
                     'operationId':head['operation_id'],'persistence':head['persistence_kind'],
                     'selectedAtMs':str(head['selected_at_ms']),'members':[{'sourceId':m['source_id'],'selectionRevision':m['selection_revision'],'ordinal':m['member_ordinal']} for m in members]}
        response=({**profile,'session':None,'commitRevision':7} if no_session else
                  {'profile':profile,'sessionId':context['session_id'],'current':current,'sources':[]})
        for offset,fields in enumerate([{'action':'context_transition','observed_at_ms':2013},
                                       {'action':'port_call','observed_at_ms':2014,'method':method,'args':args},
                                       {'action':'port_return','observed_at_ms':2016,'method':method,'response':response}],1):
            events.append(dict(cohort,kind='binding_journal_event',sequence=len(events)+1,correlation_id=correlation,**context,**fields))
        health=dict(cohort,kind='binding_journal_health',accepted=len(events),written=len(events),last_sequence=len(events),dropped=0,invalid=0,write_failures=0)
        records=[json.dumps(row,separators=(',',':')).encode()+b'\n' for row in [header,*events,health]]
        raw=b''.join(records);stats={'dev':'9007199254740993','ino':'9007199254740995','size':str(len(raw)),'mtime_ns':'1000000000000000001','ctime_ns':'1000000000000000002'}
        reader={'kind':'native_app_journal_fd_archive/v1','schema_version':1,'pid':456,'executable_path':'/actual/node','executable_sha256':'c'*64,
                'observed_started_at_ms':2020,'observed_finished_at_ms':2022,'path':'/actual/app-journal.jsonl',
                'before':stats,'after':copy.deepcopy(stats),'bytes_base64':base64.b64encode(raw).decode(),'sha256':i.sha(raw)}
        fence={'query_correlation_id':correlation,'cohort':cohort,
               'archive':{'sha256':i.sha(raw),'size':str(len(raw)),'health':health,'records_base64':[base64.b64encode(line).decode() for line in records]},
               'profile':profile,'tables':step['tables'],'held_before':{'pid':123,'fd':'19u','path':reader['path'],'inode':stats['ino']},
               'held_after':{'pid':123,'fd':'19u','path':reader['path'],'inode':stats['ino'],'dev':stats['dev']},'reader':reader,
               'fd_interval':[2018,2019],'archive_interval':[2020,2023],'archive_pid':456,
               'executable':{'path':reader['executable_path'],'sha256':reader['executable_sha256']},'writer_interval':[1000,3000]}
        return {'time':2001,'intervals':{'os':[2000,2002],'database':[2003,2010],'lsof':[2008,2012]},
                'events':events,'context':context,'fence':fence}

    def rewrite_fenced_records(self, case, *, reader_events=None, full_events=None):
        """Recompute bytes/hashes rather than editing a precomputed verdict."""
        import base64
        fence=case['fence'];header=json.loads(base64.b64decode(fence['archive']['records_base64'][0]))
        def encoded(events):
            health=dict(fence['cohort'],kind='binding_journal_health',accepted=len(events),written=len(events),last_sequence=len(events),dropped=0,invalid=0,write_failures=0)
            records=[json.dumps(row,separators=(',',':')).encode()+b'\n' for row in [header,*events,health]]
            return records,health
        records,health=encoded(full_events if full_events is not None else case['events']);raw=b''.join(records)
        fence['archive']={'sha256':i.sha(raw),'size':str(len(raw)),'health':health,'records_base64':[base64.b64encode(line).decode() for line in records]}
        reader_records,_=encoded(reader_events if reader_events is not None else case['events']);reader_raw=b''.join(reader_records)
        fence['reader']['bytes_base64']=base64.b64encode(reader_raw).decode();fence['reader']['sha256']=i.sha(reader_raw)
        fence['reader']['before']['size']=str(len(reader_raw));fence['reader']['after']['size']=str(len(reader_raw))

    def test_snapshot_actual_command_intervals_reject_mutations_and_inflight_writers(self):
        a,b=self.pair();case=self.fenced_sampling();stable=case['intervals']
        self.assertEqual(i.native_sampling_closure(case['time'],stable,case['events'],case['context'],case['fence'])['finished_at_ms'],2012)
        for intervals,events in [({'os':[1011,1019],'database':[1012,1045],'lsof':[1013,1017]},b['journal']),
                                  ({'os':[1051,1059],'database':[1052,1058],'lsof':[1053,1057]},[b['journal'][1],b['journal'][-1]])]:
            with self.assertRaises(ValueError):i.native_sampling_closure(1055 if intervals['os'][0]>1020 else 1015,intervals,events,a['context'],case['fence'])
        bad=copy.deepcopy(stable);bad['os']=[1020,1025]
        with self.assertRaises(ValueError):i.native_sampling_closure(1015,bad,b['journal'],a['context'],case['fence'])

    def test_node_python_agree_on_new_closure_controls_and_legal_retry(self):
        root=Path(__file__).resolve().parents[2];cases=self.closure_corpus()
        script=('import fs from "node:fs";import {pathToFileURL} from "node:url";const {nativeObservationVerdict}=await import(pathToFileURL(process.argv[2]).href);'
                'const rows=JSON.parse(fs.readFileSync(0,"utf8"));process.stdout.write(JSON.stringify(rows.map(r=>({id:r.id,...nativeObservationVerdict(r.case_id,r.steps)}))));')
        run=subprocess.run(['node','--input-type=module','--eval',script,'native-guard-oracle',str(root/'scripts/report/verify_binding_journey.mjs')],input=json.dumps(cases),text=True,capture_output=True,cwd=root,timeout=30)
        self.assertEqual(run.returncode,0,run.stderr)
        for raw,node in zip(cases,json.loads(run.stdout)):
            self.assertEqual(node['status'],'failed' if i.native_semantics(raw['case_id'],raw['steps']) else 'passed',raw['id'])

    def test_node_sqlite_reads_adjacent_i64_values_as_exact_text(self):
        import sqlite3
        root=Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as name:
            db=Path(name)/'integers.sqlite3';connection=sqlite3.connect(db)
            connection.execute('CREATE TABLE source_selection_sets(selection_set_revision INTEGER)')
            connection.executemany('INSERT INTO source_selection_sets VALUES(?)',[(2**53+1,),(2**53+2,)])
            connection.execute('CREATE TABLE source_selections(selection_revision INTEGER)')
            connection.executemany('INSERT INTO source_selections VALUES(?)',[(2**53+1,),(2**53+2,)])
            connection.execute('CREATE TABLE profile_meta(runtime_generation INTEGER)')
            connection.execute('INSERT INTO profile_meta VALUES(?)',(2**53+1,));connection.commit();connection.close()
            script=('import {pathToFileURL} from "node:url";const {inspectNativeSqliteFacts}=await import(pathToFileURL(process.argv[2]).href);'
                    'process.stdout.write(JSON.stringify(["source_selection_sets","source_selections","profile_meta"].map(t=>inspectNativeSqliteFacts(process.argv[3],t).rows)));')
            run=subprocess.run(['node','--input-type=module','--eval',script,'native-guard-oracle',str(root/'scripts/report/verify_binding_journey.mjs'),str(db)],text=True,capture_output=True,cwd=root,timeout=30)
            self.assertEqual(run.returncode,0,run.stderr);rows=json.loads(run.stdout)
            self.assertEqual([r['selection_set_revision'] for r in rows[0]],[str(2**53+1),str(2**53+2)])
            self.assertEqual([r['selection_revision'] for r in rows[1]],[str(2**53+1),str(2**53+2)])
            self.assertEqual(rows[2][0]['runtime_generation'],str(2**53+1))

    def test_node_python_independently_close_actual_snapshot_intervals(self):
        root=Path(__file__).resolve().parents[2];a,b=self.pair()
        cases=[self.fenced_sampling(),
               {'time':1015,'intervals':{'os':[1011,1019],'database':[1012,1045],'lsof':[1013,1017]},'events':b['journal'],'context':a['context']},
               {'time':1055,'intervals':{'os':[1051,1059],'database':[1052,1058],'lsof':[1053,1057]},'events':[b['journal'][1],b['journal'][-1]],'context':a['context']},
               {'time':1015,'intervals':{'os':[1020,1025],'database':[1012,1018],'lsof':[1013,1017]},'events':b['journal'],'context':a['context']}]
        script=('import fs from "node:fs";import {pathToFileURL} from "node:url";const {nativeObservationSampling}=await import(pathToFileURL(process.argv[2]).href);'
                'const rows=JSON.parse(fs.readFileSync(0,"utf8"));process.stdout.write(JSON.stringify(rows.map(r=>{try{nativeObservationSampling(r.time,r.intervals,r.events,r.context,r.fence);return true;}catch(e){return false;}})));')
        run=subprocess.run(['node','--input-type=module','--eval',script,'native-guard-oracle',str(root/'scripts/report/verify_binding_journey.mjs')],input=json.dumps(cases),text=True,capture_output=True,cwd=root,timeout=30)
        self.assertEqual(run.returncode,0,run.stderr);observed=json.loads(run.stdout)
        self.assertEqual(observed,[True,False,False,False])
        for raw,value in zip(cases,observed):
            try:i.native_sampling_closure(raw['time'],raw['intervals'],raw['events'],raw['context'],raw.get('fence'));accepted=True
            except ValueError:accepted=False
            self.assertEqual(value,accepted)

    def journal_fence_corpus(self):
        cases=[]
        def add(identity, raw, accepted=False):
            cases.append({'id':identity,'accepted':accepted,**raw})
        good=self.fenced_sampling();add('actual-post-probe-source-selection',good,True)
        add('actual-post-probe-sealed-current-head',self.fenced_sampling(self.pair()[1]),True)
        add('actual-post-probe-no-session-snapshot',self.fenced_sampling(no_session=True),True)
        stopped=self.fenced_sampling(no_session=True)
        stopped['fence']['tables']['sessions']=[{'session_id':'old-hook','game_id':'old-game','source_kind':'hook','state':'stopped','started_at_ms':900}]
        stopped['events'][-1]['response']['session']={'sessionId':'old-hook','gameId':'old-game','sourceKind':'hook','state':'stopped','paused':False,'translationEnabled':True,'sources':[]}
        self.rewrite_fenced_records(stopped);add('snapshot-keeps-real-stopped-session',stopped,True)
        later=copy.deepcopy(good)
        tail=dict(later['events'][-1],sequence=len(later['events'])+1,action='intent',correlation_id='later-step',observed_at_ms=2100)
        later['events'].append(tail);self.rewrite_fenced_records(later,reader_events=later['events'][:-1]);add('later-real-step-after-reader',later,True)
        for identity,path,value in [
            ('missing-real-fence',[],None),('wrong-query-correlation',['query_correlation_id'],'unrelated-query'),
            ('cross-application-pid',['cohort','pid'],124),('cross-nonce',['cohort','nonce'],'c'*32),
            ('cross-profile',['profile','serverInstanceId'],'other-instance'),
            ('cross-application-fd',['held_after','fd'],'20u'),('cross-inode',['held_after','inode'],'9007199254740996'),
            ('cross-device-adjacent-u64',['held_after','dev'],'9007199254740992'),
            ('rounded-fstat-inode',['reader','before','ino'],9007199254740995),
            ('mismatched-reader-pid',['archive_pid'],457),('mismatched-reader-executable',['executable','sha256'],'d'*64),
            ('fd-read-mutated-file',['reader','after','ctime_ns'],'1000000000000000003'),
            ('reader-size-not-exact',['reader','before','size'],'0'),
            ('reader-bytes-hash-substitution',['reader','sha256'],'d'*64),
            ('full-journal-hash-substitution',['archive','sha256'],'d'*64),
            ('fd-observed-before-query-return',['fd_interval'],[2015,2019]),
            ('reader-before-post-query-fd',['archive_interval'],[2017,2023]),
            ('reader-outer-interval-crossed',['reader','observed_started_at_ms'],2019),
            ('reader-outside-fixed-writer',['writer_interval'],[1000,2021]),
            ('fake-coverage-field',['covered_until_ms'],2022),
        ]:
            bad=copy.deepcopy(good)
            if not path:bad['fence']=value
            else:
                target=bad['fence']
                for part in path[:-1]:target=target[part]
                target[path[-1]]=value
            add(identity,bad)
        for identity,mutate in [
            ('query-call-before-sample-end',lambda r:r['events'][-2].update(observed_at_ms=2010)),
            ('query-return-before-call',lambda r:r['events'][-1].update(observed_at_ms=2013)),
            ('query-stale-profile-generation',lambda r:r['events'][-2]['args'].update(expectedProfileGeneration=2)),
            ('query-cross-session',lambda r:r['events'][-2]['args'].update(sessionId='other-sid')),
            ('query-extra-operation-field',lambda r:r['events'][-2]['args'].update(operationId='unrelated')),
            ('query-response-cross-profile',lambda r:r['events'][-1]['response']['profile'].update(serverInstanceId='other')),
            ('query-instead-mutates',lambda r:r['events'][-2].update(method='beginReading')),
            ('query-returns-error',lambda r:r['events'][-1].update(response={'errorCode':'stale'})),
            ('query-tail-after-reader-finish',lambda r:r['events'][-1].update(observed_at_ms=2024)),
        ]:
            bad=copy.deepcopy(good);mutate(bad);self.rewrite_fenced_records(bad);add(identity,bad)
        truncated=copy.deepcopy(good);self.rewrite_fenced_records(truncated,full_events=truncated['events'][:-2]);truncated['events']=truncated['events'][:-2]
        add('valid-health-prefix-truncates-real-readonly-tail',truncated)
        missing=copy.deepcopy(good);missing['events']=missing['events'][:-1];self.rewrite_fenced_records(missing);add('query-return-truncated-with-valid-health',missing)
        tail=copy.deepcopy(good);tail['events'].append(dict(tail['events'][0],sequence=len(tail['events'])+1,action='port_call',method='beginReading',args={},observed_at_ms=2005))
        self.rewrite_fenced_records(tail,reader_events=tail['events'][:-1]);add('sampling-mutation-hidden-in-late-timestamp-tail',tail)
        mutated=copy.deepcopy(good);mutated['events'].insert(1,dict(mutated['events'][0],action='persist_commit',observed_at_ms=2005))
        for n,event in enumerate(mutated['events'],1):event['sequence']=n
        self.rewrite_fenced_records(mutated);add('real-sampling-mutation-covered-by-read',mutated)
        in_flight=copy.deepcopy(good);in_flight['events'].insert(1,dict(in_flight['events'][0],action='port_call',method='beginReading',correlation_id='pending',args={},observed_at_ms=1900))
        for n,event in enumerate(in_flight['events'],1):event['sequence']=n
        self.rewrite_fenced_records(in_flight);add('inflight-mutation-with-post-query-fence',in_flight)
        stale=self.fenced_sampling(self.pair()[1]);stale['events'][-1]['response']['current']['setRevision']='2';self.rewrite_fenced_records(stale);add('query-current-head-substitution',stale)
        stale=self.fenced_sampling(self.pair()[1]);stale['events'][-1]['response']['current']['members'].reverse();self.rewrite_fenced_records(stale);add('query-set-member-order-substitution',stale)
        stale=self.fenced_sampling(self.pair()[1]);stale['events'][-1]['response']['current']['persistence']='saved_snapshot';self.rewrite_fenced_records(stale);add('query-set-persistence-substitution',stale)
        wrong=self.fenced_sampling(no_session=True);wrong['events'][-1]['response']['session']={'sessionId':'not-in-db'};self.rewrite_fenced_records(wrong);add('no-session-snapshot-fabricates-session',wrong)
        wrong=copy.deepcopy(stopped);wrong['events'][-1]['response']['session']=None;self.rewrite_fenced_records(wrong);add('snapshot-discards-actual-stopped-session',wrong)
        wrong=self.fenced_sampling(no_session=True);wrong['events'][-1]['response']['commitRevision']=6;self.rewrite_fenced_records(wrong);add('snapshot-stale-commit-revision',wrong)
        import base64
        altered=copy.deepcopy(good);line=base64.b64decode(altered['fence']['archive']['records_base64'][1]);altered['fence']['archive']['records_base64'][1]=base64.b64encode(b' '+line).decode()
        raw=b''.join(base64.b64decode(line) for line in altered['fence']['archive']['records_base64']);altered['fence']['archive'].update(sha256=i.sha(raw),size=str(len(raw)))
        add('semantic-equal-prefix-different-original-bytes',altered)
        return cases

    def test_journal_fence_corpus_requires_real_post_sample_prefix_closure(self):
        for raw in self.journal_fence_corpus():
            try:i.native_sampling_closure(raw['time'],raw['intervals'],raw['events'],raw['context'],raw['fence']);accepted=True
            except (ValueError,KeyError,TypeError):accepted=False
            self.assertEqual(accepted,raw['accepted'],raw['id'])

    def test_node_python_independent_raw_journal_fence_corpus(self):
        root=Path(__file__).resolve().parents[2];cases=self.journal_fence_corpus()
        script=('import fs from "node:fs";import {pathToFileURL} from "node:url";const {nativeObservationSampling}=await import(pathToFileURL(process.argv[2]).href);'
                'const rows=JSON.parse(fs.readFileSync(0,"utf8"));process.stdout.write(JSON.stringify(rows.map(r=>{try{nativeObservationSampling(r.time,r.intervals,r.events,r.context,r.fence);return {id:r.id,accepted:true};}catch(e){return {id:r.id,accepted:false,error:String(e.message)};}})));')
        run=subprocess.run(['node','--input-type=module','--eval',script,'native-fence-guard',str(root/'scripts/report/verify_binding_journey.mjs')],input=json.dumps(cases),text=True,capture_output=True,cwd=root,timeout=30)
        self.assertEqual(run.returncode,0,run.stderr)
        for raw,observed in zip(cases,json.loads(run.stdout)):
            self.assertEqual(observed['accepted'],raw['accepted'],(raw['id'],observed))

    def test_registered_prefix_and_sampling_mutation_tail_reject_missing_fence(self):
        # Code-owned controls run in a clean checkout. The unchanged independent
        # review originals are separately required and replayed by the recorder.
        _before,completed=self.pair()
        cohort={'nonce':'b'*32,'pid':123,'profile_id':'profile','boot_sec':1,'boot_usec':0}
        prefix={'time':2001,'intervals':{'os':[2000,2002],'database':[2003,2010],'lsof':[2008,2012]},
                'context':completed['context'],
                'events':[dict(event,**cohort,kind='binding_journal_event') for event in completed['journal']]}
        tail=copy.deepcopy(prefix)
        for sequence,action,observed in [(8,'port_call',2005),(9,'port_return',2007)]:
            tail['events'].append(dict(self.event(sequence,action,completed['context'],method='beginReading',
                                                  correlation_id='sampling-write',observed_at_ms=observed),**cohort,kind='binding_journal_event'))
        for raw in (prefix,tail):
            with self.assertRaisesRegex(ValueError,'post-sample query/FD archive fence'):
                i.native_sampling_closure(raw['time'],raw['intervals'],raw['events'],raw['context'])

    def test_fixed_fd_reader_matches_both_consumers_and_reads_real_original_bytes(self):
        import base64
        import os
        root=Path(__file__).resolve().parents[2]
        script=('import {pathToFileURL} from "node:url";const {nativeJournalReaderSource}=await import(pathToFileURL(process.argv[2]).href);process.stdout.write(nativeJournalReaderSource());')
        run=subprocess.run(['node','--input-type=module','--eval',script,'native-fence-guard',str(root/'scripts/report/verify_binding_journey.mjs')],text=True,capture_output=True,cwd=root,timeout=30)
        self.assertEqual(run.returncode,0,run.stderr);self.assertEqual(run.stdout,i.NATIVE_JOURNAL_FD_READER)
        raw_case=self.fenced_sampling();raw=b''.join(base64.b64decode(line) for line in raw_case['fence']['archive']['records_base64'])
        with tempfile.TemporaryDirectory() as name:
            directory=Path(name).resolve();journal=directory/'original.jsonl';journal.write_bytes(raw)
            with journal.open('rb') as held:
                read=subprocess.Popen(i.native_journal_read_command(str(journal)),stdout=subprocess.PIPE,stderr=subprocess.PIPE)
                stdout,stderr=read.communicate(timeout=30);self.assertEqual(read.returncode,0,stderr)
                value=json.loads(stdout);info=os.fstat(held.fileno())
                self.assertEqual(value['pid'],read.pid);self.assertEqual(value['before'],value['after'])
                self.assertEqual(value['before']['ino'],str(info.st_ino));self.assertEqual(value['before']['dev'],str(info.st_dev))
                self.assertEqual(base64.b64decode(value['bytes_base64']),raw);self.assertEqual(value['sha256'],i.sha(raw))
                link=directory/'alias.jsonl';link.symlink_to(journal)
                rejected=subprocess.run(i.native_journal_read_command(str(link)),capture_output=True,timeout=30)
                self.assertNotEqual(rejected.returncode,0)

    def test_journal_lsof_preserves_adjacent_large_device_inode_and_fixed_fd(self):
        text='p123\nf19u\nD0x20000000000001\ni9007199254740995\nn/actual/journal\n'
        observed=i.native_lsof_journal(text,123,'/actual/journal',device=True)
        self.assertEqual(observed,{'pid':123,'fd':'19u','dev':'9007199254740993','inode':'9007199254740995','path':'/actual/journal'})
        for wrong in (text.replace('p123','p124'),text+text.replace('f19u','f20u'),text.replace('D0x20000000000001','D9007199254740993')):
            with self.assertRaises(ValueError):i.native_lsof_journal(wrong,123,'/actual/journal',device=True)


if __name__ == '__main__':
    unittest.main()
