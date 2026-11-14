"""Control the experiment harness, including false-green counterexamples."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import shlex
import sys
import tempfile
import unittest
from unittest.mock import patch

import gate_effectiveness as experiments
from gate_policy import validate_visual


class EffectivenessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='luna-effectiveness-test-')
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name).resolve()

    def experiment(self):
        return experiments.Experiment(experiments.ROOT, self.folder / 'output')

    def test_catalogue_is_exactly_the_reviewed_32_unique_groups(self):
        source = json.loads((experiments.ROOT / 'docs/development/gate-cases.json').read_text())
        expected = [row['id'] for row in source['cases']]
        self.assertEqual(list(experiments.CASE_IDS), expected)
        self.assertEqual(len(set(expected)), 32)
        self.assertTrue(all(row['plan_status'] == 'planned' and row['execution_status'] == 'not_run' for row in source['cases']))

    def test_case_without_negative_observations_is_not_successful(self):
        experiment = self.experiment()
        def broken():
            for index in range(3): experiment.expect(index, index, 'positive only')
        result = experiment.run_case('GV-01', broken)
        self.assertEqual(result['status'], 'failed')
        self.assertIn('negative', result['errors'][0])

    def test_wrong_expectation_is_kept_as_failed_not_relabelled_blocked(self):
        experiment = self.experiment()
        result = experiment.run_case('GV-01', lambda: experiment.expect(3, 4, 'deliberate harness negative'))
        self.assertEqual(result['status'], 'failed')
        self.assertFalse(result['observations'][0]['matched'])
        self.assertTrue((experiment.output / 'GV-01/exception.log').is_file())

    def test_external_block_is_distinct_from_missing_implementation_or_assertion_failure(self):
        experiment = self.experiment()
        def blocked(): raise experiments.Blocked('actual external tool not available')
        result = experiment.run_case('GV-30', blocked)
        self.assertEqual(result['status'], 'blocked')
        self.assertEqual(result['assertion_count'], 0)
        self.assertFalse(result['product_acceptance_promoted'])

    def test_arbitrary_nonzero_or_unrelated_diagnostic_cannot_pass_negative_control(self):
        experiment = self.experiment()
        def wrong_rule():
            with experiment.phase('positive'):
                experiment.expect(0, 0, 'control')
            with experiment.phase('negative'):
                experiment.json_command([sys.executable, '-c', 'import json;print(json.dumps({"errors":["different-rule"]}));raise SystemExit(1)'],
                                        expected_exit=1, expected_error='required-specific-rule')
        result = experiment.run_case('GV-01', wrong_rule)
        self.assertEqual(result['status'], 'failed')
        self.assertIn('expected rejection identity', result['errors'][0])
        self.assertEqual(result['commands'][0]['exit_code'], 1)

    def test_checker_crash_is_not_successful_rejection(self):
        experiment = self.experiment()
        def crash():
            with experiment.phase('negative'):
                experiment.json_command([sys.executable, '-c', 'raise RuntimeError("checker crashed")'], expected_exit=1)
        result = experiment.run_case('GV-01', crash)
        self.assertEqual(result['status'], 'failed')
        self.assertIn('parseable JSON', result['errors'][0])

    def test_valid_counterexample_requires_positive_negative_and_real_raw_files(self):
        experiment = self.experiment()
        def matched():
            with experiment.phase('positive'):
                value = experiment.json_command([sys.executable, '-c', 'import json;print(json.dumps({"errors":[]}))'], expected_exit=0)
                experiment.expect(value['errors'], [], 'control has no errors')
            with experiment.phase('negative'):
                experiment.json_command([sys.executable, '-c', 'import json;print(json.dumps({"errors":["specific-rejection"]}));raise SystemExit(1)'],
                                        expected_exit=1, expected_error='specific-rejection')
        result = experiment.run_case('GV-01', matched)
        self.assertEqual(result['status'], 'passed')
        for command in result['commands']:
            for stream in ('stdout', 'stderr'):
                path = Path(command[stream]['path'])
                self.assertTrue(path.is_file())
                self.assertEqual(experiments.sha(path), command[stream]['sha256'])

    def test_previous_experiment_directory_cannot_be_overwritten(self):
        experiment = self.experiment()
        (experiment.output / 'original-failure.txt').write_text('preserve this')
        with self.assertRaises(FileExistsError):
            experiments.Experiment(experiments.ROOT, experiment.output)
        self.assertEqual((experiment.output / 'original-failure.txt').read_text(), 'preserve this')

    def test_fixture_approval_never_counts_as_real_product_approval(self):
        manifest, review = experiments.visual_fixture(self.folder)
        approved = validate_visual(self.folder, manifest, review, required_scope=['reading:connected'],
                                   authorized_reviewers=['isolated-reviewer'],
                                   expected_inputs=experiments.visual_fixture_inputs(self.folder), fixture=True)
        real = validate_visual(self.folder, manifest, review, required_scope=['reading:connected'],
                               authorized_reviewers=['isolated-reviewer'],
                               expected_inputs=experiments.visual_fixture_inputs(self.folder))
        self.assertEqual(approved['status'], 'passed')
        self.assertEqual(real['reason'], 'visual.fixture_forbidden')

    def test_fixture_screenshot_and_manifest_changes_are_detected(self):
        manifest, review = experiments.visual_fixture(self.folder)
        modified = copy.deepcopy(manifest)
        modified['rows'][0]['data_id'] = 'changed-reviewed-data'
        result = validate_visual(self.folder, modified, review, required_scope=['reading:connected'],
                                 authorized_reviewers=['isolated-reviewer'],
                                 expected_inputs=experiments.visual_fixture_inputs(self.folder), fixture=True)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['reason'], 'review.stale_manifest')

    def test_report_cannot_shrink_dependency_closure_even_with_matching_review(self):
        from gate_policy import map_identity
        manifest, review = experiments.visual_fixture(self.folder)
        narrowed = copy.deepcopy(manifest)
        del narrowed['inputs']['shared.css']
        narrowed_review = {**review, 'inputs': narrowed['inputs'],
                           'manifest_sha256': map_identity(narrowed)}
        trusted = experiments.visual_fixture_inputs(self.folder)
        result = validate_visual(self.folder, narrowed, narrowed_review,
                                 required_scope=['reading:connected'], authorized_reviewers=['isolated-reviewer'],
                                 expected_inputs=trusted, fixture=True)
        self.assertEqual(result['reason'], 'visual.stale_inputs')
        (self.folder / 'shared.css').write_text('.shared-control{padding:99px}')
        result = validate_visual(self.folder, narrowed, narrowed_review,
                                 required_scope=['reading:connected'], authorized_reviewers=['isolated-reviewer'],
                                 expected_inputs=experiments.visual_fixture_inputs(self.folder), fixture=True)
        self.assertEqual(result['reason'], 'visual.stale_inputs')

    def test_registered_test_cannot_downgrade_to_command_to_hide_zero_assertions(self):
        from gate_policy import validate_receipt
        experiment = self.experiment()
        root = self.folder / 'repository'; root.mkdir()
        receipt = experiments.make_receipt(experiment, root)
        self.assertEqual(validate_receipt(root, receipt, expected_command=receipt['command'], fixture=True)['status'], 'passed')
        zero = experiment.command([sys.executable, '-c', 'import unittest;unittest.TextTestRunner().run(unittest.TestSuite())'])
        self.assertEqual(zero['exit_code'], 0)
        for stream in ('stdout', 'stderr'):
            target = root / stream
            target.write_bytes(Path(zero[stream]['path']).read_bytes())
            receipt[stream] = {'path': stream, 'sha256': experiments.sha(target)}
        receipt.update(check_kind='command', test_count=0, command=zero['command'],
                       started_at=zero['started_at'], finished_at=zero['finished_at'])
        result = validate_receipt(root, receipt, expected_command=receipt['command'], fixture=True)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['reason'], 'receipt.check_kind_mismatch')

    def test_selected_run_cannot_claim_full_catalogue_or_product_completion(self):
        root = self.folder / 'repository'; root.mkdir()
        result = experiments.run(root, self.folder / 'partial', selected=['GV-31'])
        self.assertEqual(result['status'], 'passed')
        self.assertFalse(result['full_catalogue_executed'])
        self.assertFalse(result['product_acceptance_promoted'])
        self.assertFalse(result['next_stage_allowed'])
        self.assertEqual(result['selected_groups'], ['GV-31'])

    def test_unknown_duplicate_scenario_ids_are_rejected(self):
        for index, selected in enumerate([['GV-99'], ['GV-01', 'GV-01']]):
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                experiments.run(experiments.ROOT, self.folder / str(index), selected=selected)

    def test_missing_production_execution_does_not_become_green(self):
        root = self.folder / 'repository'; root.mkdir()
        result = experiments.run(root, self.folder / 'missing', selected=['GV-10'])
        self.assertEqual(result['status'], 'blocked')
        self.assertEqual(result['counts']['passed'], 0)
        self.assertIn('actual production test evidence unavailable', result['scenarios'][0]['errors'][0])

    def test_production_receipt_cannot_be_reused_from_another_current_change(self):
        import development_gates as runner
        from gate_policy import load_context
        root = self.folder / 'repository'; root.mkdir()
        experiment = experiments.Experiment(root, root / '.build/experiment')
        receipt = experiments.make_receipt(experiment, root)
        # The actual context loader reads the current change and its metadata /
        # source snapshot. The receipt cannot choose which iteration is current.
        current_change = 'new-current-iteration'
        context = root / '.codex/gate-context.json'; context.parent.mkdir()
        context.write_text(json.dumps({'schema_version': 1, 'change_id': current_change}))
        snapshot = root / '.build/current-source.json'
        snapshot.write_text(json.dumps({'files': {'check.py': experiments.sha(root / 'check.py')}}))
        metadata = root / f'openspec/changes/{current_change}/verification.json'
        metadata.parent.mkdir(parents=True)
        metadata.write_text(json.dumps({'schema_version': 1, 'id': current_change,
                                        'allowed_paths': ['scripts/quality/**'],
                                        'source_snapshot': '.build/current-source.json'}))
        self.assertEqual(load_context(root)['change_id'], current_change)
        receipt.pop('fixture')
        receipt.update(check_id='affected-functional-tests', change_id='old-previous-iteration')
        experiment.external = {'items': {'functional-timing': receipt}}
        with patch.object(runner, 'command_registry', wraps=runner.command_registry) as registry:
            result = experiment.run_case('GV-11', lambda: experiments.production_evidence(experiment, 'functional-timing'))
        self.assertEqual(result['status'], 'failed')
        self.assertIn('receipt.wrong_change', result['errors'][0])
        registry.assert_called_once_with(root, current_change, receipt['run_id'])

    def production_receipt_fixture(self):
        """Real command/raw/context with only the command boundary controlled."""
        from gate_policy import snapshot_inputs
        root = self.folder / 'repository'; root.mkdir()
        experiment = experiments.Experiment(root, root / '.build/experiment')
        receipt = experiments.make_receipt(experiment, root)
        current_change = 'current-input-closure'
        context = root / '.codex/gate-context.json'; context.parent.mkdir()
        context.write_text(json.dumps({'schema_version': 1, 'change_id': current_change}))
        dependency = root / 'shared-production-dependency.ts'
        dependency.write_text('export const decision = "original";\n')
        snapshot = root / '.build/current-source.json'
        snapshot.write_text(json.dumps({'files': snapshot_inputs(root)}))
        metadata = root / f'openspec/changes/{current_change}/verification.json'
        metadata.parent.mkdir(parents=True)
        metadata.write_text(json.dumps({'schema_version': 1, 'id': current_change,
                                        'allowed_paths': ['scripts/quality/**'],
                                        'source_snapshot': '.build/current-source.json'}))
        receipt.pop('fixture')
        receipt.update(check_id='affected-functional-tests', change_id=current_change,
                       inputs=snapshot_inputs(root))
        experiment.external = {'items': {'functional-timing': receipt}}
        # A deterministic code-owned test registry entry avoids running Cargo
        # in a unit test. Its argv is chosen here, never read from receipt JSON;
        # command execution, context loading and verdict validation are real.
        registered = {'affected-functional-tests': [sys.executable, str(root / 'check.py')]}
        return root, experiment, receipt, dependency, registered

    def test_production_receipt_cannot_self_select_a_smaller_source_closure(self):
        import development_gates as runner
        root, experiment, receipt, dependency, registered = self.production_receipt_fixture()
        raw_before = {name: experiments.sha(root / name) for name in ('stdout', 'stderr')}
        with patch.object(runner, 'command_registry', return_value=registered):
            positive = experiment.run_case('GV-11', lambda: experiments.production_evidence(experiment, 'functional-timing'))
            self.assertEqual(positive['status'], 'passed')
            narrowed = copy.deepcopy(receipt)
            del narrowed['inputs'][dependency.name]
            dependency.write_text('export const decision = "changed";\n')
            experiment.external['items']['functional-timing'] = narrowed
            negative = experiment.run_case('GV-11', lambda: experiments.production_evidence(experiment, 'functional-timing'))
        self.assertEqual(negative['status'], 'failed')
        self.assertIn('receipt.stale_inputs', negative['errors'][0])
        self.assertEqual(narrowed['command'], receipt['command'])
        self.assertEqual(raw_before, {name: experiments.sha(root / name) for name in ('stdout', 'stderr')})

    def test_run_owned_before_map_rejects_deleted_dependency_and_shrunken_receipt(self):
        import development_gates as runner
        from gate_policy import snapshot_inputs
        root, experiment, receipt, dependency, registered = self.production_receipt_fixture()
        experiment._execution_inputs = snapshot_inputs(root)
        with patch.object(runner, 'command_registry', return_value=registered):
            positive = experiment.run_case('GV-11', lambda: experiments.production_evidence(experiment, 'functional-timing'))
            self.assertEqual(positive['status'], 'passed')
            narrowed = copy.deepcopy(receipt)
            del narrowed['inputs'][dependency.name]
            dependency.unlink()
            experiment.external['items']['functional-timing'] = narrowed
            negative = experiment.run_case('GV-11', lambda: experiments.production_evidence(experiment, 'functional-timing'))
        self.assertEqual(negative['status'], 'failed')
        self.assertIn('receipt.stale_inputs', negative['errors'][0])

    def test_simulated_hook_json_cannot_count_as_l3(self):
        experiment = self.experiment()
        experiment.external = {'hooks': {'scope': 'simulated-hook-unit-test'}}
        with patch.object(experiments, 'recorded_unittest'):
            result = experiment.run_case('GV-30', lambda: experiments.actual_hook_evidence(experiment, 'GV-30'))
        self.assertEqual(result['status'], 'failed')
        self.assertIn('simulated hook input', result['errors'][0])

    def test_l3_source_identity_cannot_omit_production_hook_configuration(self):
        experiment = self.experiment()
        experiment.external = {'hooks': {
            'kind': 'codex-hook-live-verification/v1', 'schema_version': 1,
            'scope': 'actual_codex_hook_execution', 'product_acceptance_promoted': False,
            'command': ['codex', 'exec'], 'exit_code': 0,
            'source_inputs': {path: '0' * 64 for path in (
                'scripts/quality/codex_hooks.py', 'scripts/quality/test_codex_hooks.py',
                'scripts/quality/gate_policy.py')},
        }}
        with patch.object(experiments, 'recorded_unittest'):
            result = experiment.run_case('GV-30', lambda: experiments.actual_hook_evidence(experiment, 'GV-30'))
        self.assertEqual(result['status'], 'failed')
        self.assertIn('binds actual adapter / observer / policy', result['errors'][0])

    def test_l3_source_identity_cannot_omit_transitive_scope_checker(self):
        experiment = self.experiment()
        experiment.external = {'hooks': {
            'kind': 'codex-hook-live-verification/v1', 'schema_version': 1,
            'scope': 'actual_codex_hook_execution', 'product_acceptance_promoted': False,
            'command': ['codex', 'exec'], 'exit_code': 0,
            'source_inputs': {path: '0' * 64 for path in experiments.HOOK_REQUIRED_SOURCES
                              if path != 'scripts/quality/gate_checks.py'},
        }}
        with patch.object(experiments, 'recorded_unittest'):
            result = experiment.run_case('GV-30', lambda: experiments.actual_hook_evidence(experiment, 'GV-30'))
        self.assertEqual(result['status'], 'failed')
        self.assertIn('binds actual adapter / observer / policy', result['errors'][0])

    def hook_configuration_fixture(self, root=None, execution_source_root=None):
        from codex_hooks import PORTABLE_HOOK_COMMAND, materialize_hook_command
        root = root or self.folder
        source = root / '.codex/hooks.json'; source.parent.mkdir(parents=True)
        handler = {'type': 'command', 'command': PORTABLE_HOOK_COMMAND, 'timeout': 30}
        definition = {'hooks': {event: [{'matcher': 'Bash', 'hooks': [copy.deepcopy(handler)]}]
                                for event in ('SessionStart', 'PreToolUse', 'PostToolUse', 'Stop')}}
        source.write_text(json.dumps(definition))
        for relative in experiments.HOOK_REQUIRED_SOURCES - {'.codex/hooks.json'}:
            target = root / relative; target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text('# isolated source identity control: ' + relative + '\n')
        execution_source_root = str(execution_source_root or root)
        disposable = '/tmp/luna-isolated-hook-control'
        isolated = copy.deepcopy(definition)
        for groups in isolated['hooks'].values():
            argv = materialize_hook_command(groups[0]['hooks'][0]['command'], execution_source_root)
            argv[argv.index('--root') + 1] = disposable
            groups[0]['hooks'][0]['command'] = shlex.join(argv)
        original = root / 'isolated-hook-definition.json'; original.write_text(json.dumps(isolated))
        reference = {'path': original.name, 'sha256': experiments.sha(original)}
        inputs = {relative: experiments.sha(root / relative) for relative in experiments.HOOK_REQUIRED_SOURCES}
        proof = {'command': ['codex', 'exec', '-C', disposable],
                 'execution_source_root': execution_source_root,
                 'source_inputs': inputs, 'source_inputs_before': inputs, 'source_stable': True}
        row = {'observations': {'probe_root': disposable}, 'evidence': [reference]}
        return proof, row, definition, isolated

    def test_l3_isolated_configuration_may_only_rebase_project_root(self):
        proof, row, definition, isolated = self.hook_configuration_fixture()
        root = self.folder
        source = root / '.codex/hooks.json'
        original = root / 'isolated-hook-definition.json'
        reference = row['evidence'][0]
        valid_isolated = copy.deepcopy(isolated)
        experiments.validate_hook_configuration(root, proof, row)
        # A correctly re-hashed but relaxed actual configuration is still wrong.
        isolated['hooks']['PreToolUse'][0]['matcher'] = 'Read'
        original.write_text(json.dumps(isolated)); reference['sha256'] = experiments.sha(original)
        with self.assertRaisesRegex(AssertionError, 'beyond fixed source-root'):
            experiments.validate_hook_configuration(root, proof, row)
        # A new current project timeout also expires the old isolated definition.
        source_definition = copy.deepcopy(definition)
        source_definition['hooks']['Stop'][0]['hooks'][0]['timeout'] = 60
        source.write_text(json.dumps(source_definition))
        original.write_text(json.dumps(valid_isolated)); reference['sha256'] = experiments.sha(original)
        with self.assertRaisesRegex(AssertionError, 'beyond fixed source-root'):
            experiments.validate_hook_configuration(root, proof, row)

    def test_l3_same_sources_validate_across_distinct_checkouts(self):
        producer = self.folder / 'producer'; producer.mkdir()
        consumer = self.folder / 'consumer'; consumer.mkdir()
        producer_proof, _, _, _ = self.hook_configuration_fixture(producer)
        proof, row, _, _ = self.hook_configuration_fixture(consumer, producer)
        self.assertEqual(producer_proof['source_inputs'], proof['source_inputs'])
        self.assertNotEqual(Path(proof['execution_source_root']), consumer)
        experiments.validate_hook_source_identity(consumer, proof)
        original_bytes = (consumer / 'isolated-hook-definition.json').read_bytes()
        experiments.validate_hook_configuration(consumer, proof, row)
        self.assertEqual((consumer / 'isolated-hook-definition.json').read_bytes(), original_bytes)

    def test_l3_cross_checkout_cannot_normalize_changed_source_hash(self):
        proof, _, _, _ = self.hook_configuration_fixture(execution_source_root='/opt/ci/luna-source')
        experiments.validate_hook_source_identity(self.folder, proof)
        target = self.folder / 'scripts/quality/gate_checks.py'
        target.write_text(target.read_text() + '# changed scope decision\n')
        with self.assertRaisesRegex(AssertionError, 'input hash mismatch'):
            experiments.validate_hook_source_identity(self.folder, proof)

    def test_l3_cross_checkout_cannot_rebase_wrong_handler_or_execution_root(self):
        proof, row, _, isolated = self.hook_configuration_fixture(execution_source_root='/opt/ci/luna-source')
        original = self.folder / 'isolated-hook-definition.json'
        reference = row['evidence'][0]
        altered = copy.deepcopy(isolated)
        command = altered['hooks']['Stop'][0]['hooks'][0]['command']
        altered['hooks']['Stop'][0]['hooks'][0]['command'] = command.replace('codex_hooks.py', 'other_handler.py')
        original.write_text(json.dumps(altered)); reference['sha256'] = experiments.sha(original)
        with self.assertRaisesRegex(AssertionError, 'beyond fixed source-root'):
            experiments.validate_hook_configuration(self.folder, proof, row)
        original.write_text(json.dumps(isolated)); reference['sha256'] = experiments.sha(original)
        wrong_root = {**proof, 'execution_source_root': '/opt/ci/other-source'}
        with self.assertRaisesRegex(AssertionError, 'beyond fixed source-root'):
            experiments.validate_hook_configuration(self.folder, wrong_root, row)
        wrong_process_root = {**proof, 'command': ['codex', 'exec', '-C', '/tmp/another-probe']}
        with self.assertRaisesRegex(AssertionError, 'Codex root differs'):
            experiments.validate_hook_configuration(self.folder, wrong_process_root, row)

    def test_l3_cannot_claim_absent_required_source_as_matching_null_hash(self):
        proof, _, _, _ = self.hook_configuration_fixture()
        proof['source_inputs']['scripts/quality/gate_checks.py'] = None
        (self.folder / 'scripts/quality/gate_checks.py').unlink()
        with self.assertRaisesRegex(AssertionError, 'concrete file hash'):
            experiments.validate_hook_source_identity(self.folder, proof)

    def test_raw_reference_rejects_drift_and_symlink(self):
        actual = self.folder / 'raw.log'; actual.write_text('observed')
        ref = {'path': 'raw.log', 'sha256': experiments.sha(actual)}
        self.assertEqual(experiments._raw_reference(self.folder, ref), actual)
        actual.write_text('changed')
        with self.assertRaises(AssertionError): experiments._raw_reference(self.folder, ref)
        link = self.folder / 'link.log'; link.symlink_to(actual)
        with self.assertRaises(ValueError):
            experiments._raw_reference(self.folder, {'path': 'link.log', 'sha256': experiments.sha(actual)})

    def test_absolute_external_index_cannot_resolve_away_a_symlink(self):
        actual = self.folder / 'index.json'; actual.write_text(json.dumps({'schema_version': 1}))
        link = self.folder / 'alias.json'; link.symlink_to(actual)
        with self.assertRaises(ValueError):
            experiments._external_document(self.folder, link)

    def live_raw_fixture(self):
        """Parser controls only: these JSON samples never claim actual L3."""
        def event(name, session, decision, tool=None, facts=None):
            return {'event_name': name, 'session_id': session, 'tool_use_id': tool,
                    'decision': decision, 'observations': facts or {}}
        journal = [event('SessionStart', 'readonly-session', 'context_loaded'),
                   event('Stop', 'readonly-session', 'readonly_stop'),
                   event('SessionStart', 'action-session', 'context_loaded'),
                   event('PreToolUse', 'action-session', 'allow', 'positive'),
                   event('PostToolUse', 'action-session', 'changes_recorded', 'positive'),
                   event('PreToolUse', 'action-session', 'deny', 'denied', {'reason': 'hook.scope_violation: denied.txt'}),
                   event('PreToolUse', 'action-session', 'allow', 'dynamic'),
                   event('PostToolUse', 'action-session', 'violation', 'dynamic', {'outside_scope': ['post-only.txt']}),
                   event('Stop', 'action-session', 'continue_verification', facts={'fingerprint': 'same-failure'}),
                   event('Stop', 'action-session', 'continue_verification', facts={'fingerprint': 'same-failure'}),
                   event('Stop', 'action-session', 'continuation_limit')]
        observed_files = {'before': {'allowed/positive.txt': None, 'denied.txt': None, 'post-only.txt': None},
                          'after': {'allowed/positive.txt': hashlib.sha256(b'').hexdigest(), 'denied.txt': None,
                                    'post-only.txt': hashlib.sha256(b'observed side effect').hexdigest()}}
        readonly = [{'type': 'thread.started', 'thread_id': 'readonly-session'}, {'type': 'turn.completed'}]
        action = [{'type': 'thread.started', 'thread_id': 'action-session'},
                  {'type': 'item.completed', 'item': {'type': 'command_execution', 'command': 'touch allowed/positive.txt', 'exit_code': 0}},
                  {'type': 'item.completed', 'item': {'type': 'command_execution', 'command': 'python3 -c "write post-only.txt"', 'exit_code': 0}},
                  {'type': 'turn.completed'}]
        artifacts = {'actual-hook-events.jsonl': journal, 'readonly.stdout.jsonl': readonly,
                     'write-probes.stdout.jsonl': action}
        refs = []
        for name, values in artifacts.items():
            target = self.folder / name
            target.write_text(''.join(json.dumps(value) + '\n' for value in values))
            refs.append({'path': name, 'sha256': experiments.sha(target)})
        target = self.folder / 'external-file-observations.json'
        target.write_text(json.dumps(observed_files)); refs.append({'path': target.name, 'sha256': experiments.sha(target)})
        projection = [{key: value.get(key) for key in ('event_name', 'session_id', 'tool_use_id', 'decision')} for value in journal if value['session_id'] == 'action-session']
        row = {'case_id': 'GV-18', 'evidence': refs, 'observations': {'scope_session_id': 'action-session', 'events': projection, 'files': observed_files,
               'continuations': 2, 'denied_target_absent': True, 'write_retained': True}}
        return row

    def test_hook_raw_parser_recomputes_event_and_file_effects(self):
        row = self.live_raw_fixture()
        derived = experiments.derive_hook_observations(self.folder, row)
        self.assertTrue(all(derived['case_results'].values()))
        self.assertEqual(derived['continuations'], 2)
        self.assertTrue(derived['denied_target_absent'])
        self.assertNotIn('GV-23', derived['case_results'])

    def test_hook_raw_parser_rejects_unrelated_nonempty_logs(self):
        target = self.folder / 'unrelated.log'; target.write_text('nonempty unrelated content')
        row = {'evidence': [{'path': target.name, 'sha256': experiments.sha(target)}], 'observations': {'events': []}}
        with self.assertRaisesRegex(AssertionError, 'missing actual journal'):
            experiments.derive_hook_observations(self.folder, row)

    def test_hook_raw_parser_rejects_claimed_events_different_from_raw(self):
        row = self.live_raw_fixture()
        row['observations']['events'][0]['session_id'] = 'forged-session'
        with self.assertRaisesRegex(AssertionError, 'differ from raw journal'):
            experiments.derive_hook_observations(self.folder, row)

    def test_hook_raw_parser_rejects_actual_journal_unrelated_session(self):
        row = self.live_raw_fixture()
        target = self.folder / 'actual-hook-events.jsonl'
        values = [json.loads(line) for line in target.read_text().splitlines()]
        values[0]['session_id'] = 'unrelated-session'
        target.write_text(''.join(json.dumps(value) + '\n' for value in values))
        next(ref for ref in row['evidence'] if ref['path'] == target.name)['sha256'] = experiments.sha(target)
        with self.assertRaisesRegex(AssertionError, 'unrelated session'):
            experiments.derive_hook_observations(self.folder, row)

    def test_hook_raw_parser_rejects_false_side_effect_and_continuation_claim(self):
        row = self.live_raw_fixture()
        row['observations']['continuations'] = 0
        with self.assertRaisesRegex(AssertionError, 'continuation count'):
            experiments.derive_hook_observations(self.folder, row)
        row['observations']['continuations'] = 2
        row['observations']['write_retained'] = False
        with self.assertRaisesRegex(AssertionError, 'write effect'):
            experiments.derive_hook_observations(self.folder, row)


if __name__ == '__main__':
    unittest.main()
