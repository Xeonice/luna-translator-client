"""Self-contained readiness/dispatch guards. They never grant product acceptance."""
import copy
import unittest
from unittest.mock import patch

import binding_readiness_assessment as a
import binding_journey as b


def good_observation():
    external = set(a.EXTERNAL_OBLIGATIONS)
    groups = [{'id': identity, 'status': 'blocked' if identity in external else 'passed',
               'checks': [{'id': identity + '-actual', 'status': 'passed'}] if identity not in external else [],
               'artifacts': [{'path': '.build/original.json', 'sha256': 'a' * 64}] if identity not in external else [],
               'errors': []} for identity in b.EXACT_EXECUTION_OBLIGATION_IDS]
    layers = [{'case_id': case, 'layer': layer, 'status': 'passed'}
              for case, required in b.REQUIRED_LAYERS.items() for layer in required
              if layer not in a.EXTERNAL_LAYERS]
    inspection = {'schema_version': 1, 'kind': 'binding-journey-inspection/v1', 'status': 'blocked',
                  'source_tree_sha256': 'a' * 64, 'errors': [a.BLOCKED_SUMMARY],
                  'validation_errors': [], 'infrastructure_errors': [], 'semantic_failures': [],
                  'layer_failures': [], 'obligation_failures': [], 'eligible_semantic_red': False,
                  'case_count': len(b.COMPONENT_CASES) + len(b.COMPONENT_SCENES),
                  'passed': len(b.COMPONENT_CASES) + len(b.COMPONENT_SCENES), 'failed': 0, 'blocked': 0,
                  'diagnostic_count': len(b.COMPONENT_CASES), 'scene_count': len(b.COMPONENT_SCENES),
                  'layer_results': layers, 'execution_obligations': {'schema_version': 1,
                  'kind': 'binding-execution-obligation-assessment/v1', 'groups': groups,
                  'product_acceptance_promoted': False},
                  'storage_v5_inspection': {'status': 'passed', 'errors': [],
                    'child_execution': {'kind': 'schema5-child-execution/v1',
                      'parent_timeline_routes': list(range(9)), 'fault_routes': list(range(4)), 'child_tests': list(range(3))}},
                  'product_acceptance_promoted': False, 'native_acceptance_promoted': False,
                  'recomputation': {'kind': 'binding-journey-independent-recomputation/v1', 'errors': [],
                    'blocked_obligations': [], 'case_statuses': {case: 'passed' for case in (*b.COMPONENT_CASES, *b.COMPONENT_SCENES)}}}
    gaps = [{'case_id': case, 'layer': layer, 'status': 'not_run'}
            for case, required in b.REQUIRED_LAYERS.items() for layer in required
            if layer in a.EXTERNAL_LAYERS]
    gaps += [{'case_id': identity, 'layer': 'registered_execution_obligation', 'status': 'blocked'}
             for identity in b.EXACT_EXECUTION_OBLIGATION_IDS if identity in external]
    qualification = {'schema_version': 1, 'kind': 'binding-journey-global-assessment/v1',
                     'status': 'blocked', 'errors': [], 'release_gaps': gaps,
                     'execution_obligations': copy.deepcopy(inspection['execution_obligations']),
                     'covered_layers': copy.deepcopy(layers),
                     'native_acceptance_promoted': False, 'product_acceptance_promoted': False}
    return inspection, qualification, {'native_evidence': None, 'service_evidence': None}


class BindingReadinessTests(unittest.TestCase):
    def setUp(self):
        self.i, self.q, self.context = good_observation()

    def result(self):
        return a.derive_readiness(self.i, self.q, self.context)

    def rejected(self):
        result = self.result()
        self.assertEqual(result['assessment_status'], 'failed', result)
        self.assertFalse(result['next_stage_allowed'])
        self.assertEqual(result['product_completion'], 'incomplete')

    def test_absent_external_layers_remain_nested_blocked(self):
        result = self.result()
        self.assertEqual(result['assessment_status'], 'passed', result)
        self.assertEqual(result['binding_qualification']['status'], 'blocked')
        self.assertFalse(result['product_acceptance_promoted'])
        self.assertFalse(result['next_stage_allowed'])

    def test_semantic_error_never_becomes_readiness(self):
        self.i['semantic_failures'] = [{'case_id': 'BJ-03', 'assertion_ids': ['actual-ui-error']}]
        self.rejected()

    def test_infrastructure_error_is_not_expected_absence(self):
        self.i['infrastructure_errors'] = ['compiler failed']
        self.rejected()

    def test_validation_error_is_not_expected_absence(self):
        self.i['validation_errors'] = ['source mismatch']
        self.rejected()

    def test_local_machine_obligation_blocked_is_failure(self):
        self.i['execution_obligations']['groups'][0]['status'] = 'blocked'
        self.rejected()

    def test_strict_v5_failed_or_missing_or_v1_is_failure(self):
        for mutation in ({'status': 'failed', 'errors': ['old9 artifact mismatch']},
                         {'status': 'blocked', 'errors': []},
                         {'status': 'passed', 'errors': [], 'child_execution': {}}):
            with self.subTest(mutation=mutation):
                self.i['storage_v5_inspection'] = mutation
                self.rejected()

    def test_missing_local_layer_is_failure(self):
        self.i['layer_results'].pop(0)
        self.rejected()

    def test_unknown_or_duplicate_layer_is_failure(self):
        for row in ({'case_id': 'BJ-99', 'layer': 'native', 'status': 'passed'}, self.i['layer_results'][0]):
            with self.subTest(row=row):
                self.i['layer_results'].append(row)
                self.rejected()
                self.i['layer_results'].pop()

    def test_unknown_duplicate_or_missing_ex_group_is_failure(self):
        for replacement in (self.i['execution_obligations']['groups'][:-1],
                            self.i['execution_obligations']['groups'] + [self.i['execution_obligations']['groups'][0]],
                            self.i['execution_obligations']['groups'][:-1] + [{'id': 'renamed', 'status': 'passed'}]):
            with self.subTest(replacement=replacement):
                saved = self.i['execution_obligations']['groups']
                self.i['execution_obligations']['groups'] = replacement
                self.rejected()
                self.i['execution_obligations']['groups'] = saved

    def test_provided_invalid_native_is_not_absence(self):
        self.context['native_evidence'] = {'path': '.build/invalid-native.json', 'sha256': 'a' * 64}
        self.rejected()

    def test_provided_service_cannot_keep_service_game_gap(self):
        self.context['service_evidence'] = {'path': '.build/service.json', 'sha256': 'a' * 64}
        # The original global adapter still needs packaged game evidence. Service
        # can be provided while game remains absent, but its service layer may not.
        self.rejected()

    def test_generic_blocked_summary_cannot_hide_an_extra_error(self):
        self.i['errors'].append('Actual unknown blocked fault')
        self.rejected()

    def test_omitted_renamed_duplicate_global_gap_is_failure(self):
        for gap in ([], self.q['release_gaps'][1:], self.q['release_gaps'] + [self.q['release_gaps'][0]],
                    self.q['release_gaps'] + [{'case_id': 'local-missing', 'layer': 'domain', 'status': 'blocked'}]):
            with self.subTest(gap=gap):
                saved = self.q['release_gaps']
                self.q['release_gaps'] = gap
                self.rejected()
                self.q['release_gaps'] = saved

    def test_passed_group_requires_actual_checks_and_artifacts(self):
        for key in ('checks', 'artifacts'):
            with self.subTest(key=key):
                saved = self.i['execution_obligations']['groups'][0][key]
                self.i['execution_obligations']['groups'][0][key] = []
                self.rejected()
                self.i['execution_obligations']['groups'][0][key] = saved

    def test_false_green_raw_and_product_promotion_are_rejected(self):
        self.i['status'] = 'passed'
        self.rejected()
        self.i['status'] = 'blocked'
        self.q['product_acceptance_promoted'] = True
        self.rejected()

    def test_same_typed_projection_ignores_only_replay_receipt_identity(self):
        other = copy.deepcopy(self.i)
        other['recomputation']['independent_component_layout'] = {'execution': {'path': '.build/new-replay.json'}}
        self.assertEqual(a.inspection_projection(self.i), a.inspection_projection(other))
        other['semantic_failures'] = [{'case_id': 'BJ-01'}]
        self.assertNotEqual(a.inspection_projection(self.i), a.inspection_projection(other))


class BindingDispatchTests(unittest.TestCase):
    def test_explicit_storage_and_binding_checks_dispatch_without_auto_requirement(self):
        import verify
        events = []
        actual = lambda: events.append('full') or {'errors': ['actual blocked'], 'storage_v5_inspection': {'status': 'passed', 'errors': []}}
        rows = verify.dispatch_binding_checks({'binding-journey-execution', 'storage-schema-v5-runtime-inspection'},
                                              execute_full=actual, execute_assessment=lambda: self.fail('unrequested assessment'))
        self.assertEqual(events, ['full'])
        self.assertEqual([r['id'] for r in rows], ['binding-journey-execution', 'storage-schema-v5-runtime-inspection'])
        self.assertEqual(rows[0]['errors'], ['actual blocked'])

    def test_assessment_does_not_downgrade_required_full_or_repeat_execution(self):
        import verify
        full = {'errors': ['actual blocked'], 'status': 'blocked'}
        observed = []
        report = {'assessment_status': 'passed', 'status': 'passed', 'errors': [],
                  'full_binding_result': full}
        rows = verify.dispatch_binding_checks({'binding-journey-execution', 'binding-journey-readiness-assessment'},
            execute_full=lambda: self.fail('must reuse actual same full invocation'),
            execute_assessment=lambda: observed.append('assessment') or report)
        self.assertEqual(observed, ['assessment'])
        self.assertEqual(next(r for r in rows if r['id'] == 'binding-journey-execution')['errors'], ['actual blocked'])

    def test_requested_registered_ids_are_in_effective_union_and_unknown_rejected(self):
        import verify
        self.assertEqual(verify.effective_binding_checks({'storage-schema-v5-contract'}, {'binding-journey-execution'}),
                         {'storage-schema-v5-contract', 'binding-journey-execution'})
        with self.assertRaises(ValueError):
            verify.effective_binding_checks({'invented-check'}, set())


class ActualReadinessEnvelopeGuards(unittest.TestCase):
    """Exercise the adapter's raw envelope checks with a synthetic inner oracle.

    The original full inspector has its own independent production guards; these
    fixtures test only this adapter's process/source/input binding, never Rust or
    browser acceptance.
    """
    def setUp(self):
        import tempfile
        from pathlib import Path
        from gate_core import source_files
        from binding_phase_guard import tree_sha, ref
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        (self.root / 'apps').mkdir()
        (self.root / 'apps/fixture.rs').write_text('synthetic guard source')
        self.output = '.build/assessment'
        self.directory = self.root / self.output
        (self.directory / 'full').mkdir(parents=True)
        self.i, self.q, _ = good_observation()
        before = source_files(self.root)
        self.i['source_tree_sha256'] = tree_sha(before)
        self.write('source-before.json', before)
        self.write('source-after.json', before)
        self.context = {'schema_version': 1, 'kind': 'binding-readiness-actual-invocation/v1',
                        'nonce': 'a' * 32, 'started_at': '2026-11-10T10:00:00+00:00',
                        'finished_at': '2026-11-10T10:00:04+00:00',
                        'source_tree_sha256': tree_sha(before),
                        'source_before': ref(self.root, self.output + '/source-before.json'),
                        'source_after': ref(self.root, self.output + '/source-after.json'),
                        'native_evidence': None, 'service_evidence': None}
        self.write('context.json', self.context)
        self.write('full/execution.json', {'pid': 1234, 'nonce': self.context['nonce'],
            'started_at': '2026-11-10T10:00:01.100000+00:00', 'finished_at': '2026-11-10T10:00:02.900000+00:00',
            'source_tree_sha256': tree_sha(before), 'native_evidence': None, 'service_evidence': None})
        (self.directory / 'stdout.log').write_text(__import__('json').dumps(self.i))
        (self.directory / 'stderr.log').write_text('')
        self.actual = {'schema_version': 1, 'kind': 'actual-binding-readiness-child/v1',
                       'command': a._command(self.output, self.context['nonce'], None, None),
                       'cwd': str(self.root), 'pid': 1234,
                       'started_at': '2026-11-10T10:00:01+00:00', 'finished_at': '2026-11-10T10:00:03+00:00',
                       'exit_code': 1, 'timed_out': False, 'source_tree_sha256': tree_sha(before),
                       'stdout': ref(self.root, self.output + '/stdout.log'),
                       'stderr': ref(self.root, self.output + '/stderr.log')}
        self.write('actual.json', self.actual)
        self.inspector = patch('binding_journey_inspect.inspect_execution', return_value=self.i)
        self.global_inspector = patch('binding_journey.global_assessment', return_value=self.q)
        self.inspector.start()
        self.global_inspector.start()
        self.addCleanup(self.inspector.stop)
        self.addCleanup(self.global_inspector.stop)

    def write(self, name, value):
        import json
        (self.directory / name).write_text(json.dumps(value))

    def inspect(self):
        from binding_phase_guard import ref
        return a._independent(self.root, self.directory, ref(self.root, self.output + '/actual.json'))

    def test_valid_blocked_child_is_retained_as_exit_one_not_full_pass(self):
        result = self.inspect()
        self.assertEqual(result['assessment_status'], 'passed')
        self.assertEqual(result['actual_full_exit_code'], 1)
        self.assertEqual(result['full_binding_result']['status'], 'blocked')

    def test_actual_exit_pid_command_timeout_and_time_drift_rejected(self):
        for key, value in [('exit_code', 0), ('pid', 0), ('pid', 999), ('command', ['echo', 'passed']),
                           ('timed_out', True), ('finished_at', '2026-11-10T10:00:05+00:00'),
                           ('source_tree_sha256', 'f' * 64)]:
            with self.subTest(key=key, value=value):
                saved = self.actual[key]
                self.actual[key] = value
                self.write('actual.json', self.actual)
                with self.assertRaises(ValueError):
                    self.inspect()
                self.actual[key] = saved
        self.write('actual.json', self.actual)

    def test_cross_nonce_or_cross_input_full_producer_rejected(self):
        import json
        original = json.loads((self.directory / 'full/execution.json').read_text())
        for key, value in [('nonce', 'b' * 32), ('native_evidence', {'path': '.build/another.json'}),
                           ('source_tree_sha256', 'b' * 64)]:
            with self.subTest(key=key):
                self.write('full/execution.json', {**original, key: value})
                with self.assertRaises(ValueError):
                    self.inspect()
        self.write('full/execution.json', original)

    def test_current_source_changes_cannot_reuse_prior_readiness(self):
        (self.root / 'apps/fixture.rs').write_text('changed synthetic source')
        with self.assertRaises(ValueError):
            self.inspect()

    def test_summary_tampering_and_missing_original_fail_closed(self):
        import json
        from binding_phase_guard import ref
        tampered = copy.deepcopy(self.i)
        tampered['status'] = 'passed'
        (self.directory / 'stdout.log').write_text(json.dumps(tampered))
        self.actual['stdout'] = ref(self.root, self.output + '/stdout.log')
        self.write('actual.json', self.actual)
        with self.assertRaises(ValueError):
            self.inspect()
        self.assertTrue(a.assessment_errors(self.root, {'assessment_receipt': '.build/missing.json'}))


class ReadinessAdditionalFailures(unittest.TestCase):
    def test_recomputation_error_or_hidden_blocked_input_is_failure(self):
        for key, value in [('errors', ['independent DOM failed']), ('blocked_obligations', ['missing actual RPC']),
                           ('case_statuses', {})]:
            with self.subTest(key=key):
                inspected, qualified, context = good_observation()
                inspected['recomputation'][key] = value
                self.assertEqual(a.derive_readiness(inspected, qualified, context)['assessment_status'], 'failed')

    def test_observed_visual_failure_is_not_a_missing_native_input(self):
        inspected, qualified, context = good_observation()
        inspected['layer_results'].append({'case_id': 'BJ-14', 'layer': 'design', 'status': 'blocked',
                                           'errors': ['actual RGBA comparison failed']})
        self.assertEqual(a.derive_readiness(inspected, qualified, context)['assessment_status'], 'failed')

    def test_only_readiness_does_not_emit_an_original_full_pass_step(self):
        import verify
        inspected, qualified, context = good_observation()
        report = a.derive_readiness(inspected, qualified, context)
        rows = verify.dispatch_binding_checks({'binding-journey-readiness-assessment'},
            execute_full=lambda: self.fail('one assessment owns the actual full execution'), execute_assessment=lambda: report)
        self.assertEqual([row['id'] for row in rows], ['binding-journey-readiness-assessment'])
        self.assertIs(rows[0]['product_tests_executed'], False)

    def test_requested_contract_has_an_actual_consumer_and_strict_v5_cannot_be_waived(self):
        import verify
        seen = []
        inspected, qualified, context = good_observation()
        inspected['storage_v5_inspection']['status'] = 'failed'
        report = a.derive_readiness(inspected, qualified, context)
        rows = verify.dispatch_binding_checks({'binding-journey-readiness-contract', 'storage-schema-v5-runtime-inspection'},
            execute_full=lambda: self.fail('cannot repeat/replace the shared actual run'), execute_assessment=lambda: report,
            inspect_assessment=lambda value: seen.append(value) or ['actual independent rejection'])
        self.assertEqual(seen, [report])
        self.assertTrue(next(row for row in rows if row['id'] == 'binding-journey-readiness-contract')['errors'])
        self.assertTrue(next(row for row in rows if row['id'] == 'storage-schema-v5-runtime-inspection')['errors'])

    def test_real_product_and_release_automatic_full_cannot_be_downgraded_by_metadata(self):
        import verify
        for paths, profile in [(['apps/desktop/src/features/capture/CaptureSheet.tsx'], 'development'), (['crates/app-infra/src/storage/queries.rs'], 'development'),
                               ([], 'release')]:
            with self.subTest(paths=paths, profile=profile):
                automatic = b.required_checks('fixture-QA-id', paths, profile)
                effective = verify.effective_binding_checks({'binding-journey-readiness-assessment'}, automatic)
                self.assertIn('binding-journey-execution', effective)

    def test_explicit_v5_static_checks_dispatch_both_actual_registered_handlers(self):
        import verify
        seen = []
        for requested in ({'storage-schema-v5-contract'}, {'storage-schema-v5-boundaries'}):
            with self.subTest(requested=requested):
                rows = verify.dispatch_schema_v5_checks(requested,
                    validate_contract=lambda: seen.append('contract') or [],
                    assess_boundaries=lambda: seen.append('boundaries') or {'status': 'passed', 'errors': []})
                self.assertEqual([r['id'] for r in rows], ['storage-schema-v5-contract', 'storage-schema-v5-boundaries'])
                self.assertTrue(all(row['errors'] == [] for row in rows))
        self.assertEqual(seen, ['contract', 'boundaries', 'contract', 'boundaries'])
        rows = verify.dispatch_schema_v5_checks({'storage-schema-v5-contract'},
            validate_contract=lambda: ['real contract failed'],
            assess_boundaries=lambda: {'status': 'blocked', 'errors': []})
        self.assertTrue(all(row['errors'] for row in rows))
