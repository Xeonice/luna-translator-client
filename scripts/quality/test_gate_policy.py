"""Adversarial verdict tests on disposable files, not product acceptance."""
from copy import deepcopy
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zlib

from gate_core import digest
from gate_policy import (GatePolicyError, atomic_write_json, build_summary,
                         changed_paths, classify_impact, evaluate, gate_result,
                         load_context, locked_state, map_identity,
                         observed_test_counts, read_json, snapshot_inputs,
                         validate_receipt, validate_visual)


def png(width=2, height=2, color=(10, 30, 80, 255), *, decoded=None):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    rows = decoded if decoded is not None else (b'\0' + bytes(color) * width) * height
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))


class PolicyFixtures(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='luna-gate-policy-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.write('scripts/checker.py', 'print("ok")\n')
        self.write('src/view.tsx', 'export const view = "reading";\n')
        self.write('docs/design/reference.html', '<p>reading</p>\n')
        self.before = snapshot_inputs(self.root)
        self.write('.build/confirmation.txt', 'isolated gate_fixture: delegated-reviewer approved test evidence\n')
        self.write('.build/stdout.log', 'Ran 2 tests in 0.001s\n\nOK\n')
        self.write('.build/stderr.log', '')
        self.write('reports/visual/index.html', '<p>isolated visual comparison fixture</p>')
        self.write('reports/visual/design.png', png(), binary=True)
        self.write('reports/visual/implementation.png', png(color=(20, 30, 80, 255)), binary=True)

    def write(self, relative, content, *, binary=False):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if binary:
            path.write_bytes(content)
        else:
            path.write_text(content, encoding='utf-8')
        return path

    def ref(self, relative):
        return {'path': relative, 'sha256': digest(self.root / relative)}

    def receipt(self, **updates):
        now = datetime.now(timezone.utc).isoformat()
        result = {'schema_version': 1, 'kind': 'gate-command-receipt/v1',
                  'fixture': True, 'gate_id': 'G05', 'check_id': 'unit-tests',
                  'run_id': 'isolated-run', 'change_id': 'fixture-change',
                  'command': [sys.executable, 'scripts/checker.py'],
                  'started_at': now, 'finished_at': now, 'exit_code': 0,
                  'inputs': snapshot_inputs(self.root),
                  'stdout': self.ref('.build/stdout.log'),
                  'stderr': self.ref('.build/stderr.log'),
                  'check_kind': 'tests', 'test_count': 2, 'skipped_count': 0}
        result.update(updates)
        return result

    def check_receipt(self, receipt, **options):
        options.setdefault('expected_command', [sys.executable, 'scripts/checker.py'])
        options.setdefault('fixture', True)
        return validate_receipt(self.root, receipt, **options)

    def visual(self):
        return {'kind': 'unified-visual-report/v1', 'fixture': True,
                'report': self.ref('reports/visual/index.html'),
                'inputs': snapshot_inputs(self.root),
                'rows': [{'scope': 'reading/default',
                          'design_image': self.ref('reports/visual/design.png'),
                          'implementation_image': self.ref('reports/visual/implementation.png'),
                          'viewport': {'width': 2, 'height': 2},
                          'theme': 'dark', 'data_id': 'fixture-reading',
                          'design_state': 'default', 'implementation_state': 'default',
                          'source_kind': 'real_component'}]}

    def review(self, manifest=None, gate_id='G09'):
        result = {'kind': 'gate-human-review/v1', 'fixture': True,
                  'review_decision': 'approved', 'reviewer_id': 'delegated-reviewer',
                  'reviewer_kind': 'delegated_reviewer', 'author_ids': ['implementer'],
                  'reviewed_at': datetime.now(timezone.utc).isoformat(),
                  'confirmation_source': self.ref('.build/confirmation.txt'),
                  'scope': ['reading/default'] if gate_id == 'G09' else [gate_id],
                  'inputs': snapshot_inputs(self.root), 'unresolved_findings': []}
        if manifest:
            result['report_sha256'] = manifest['report']['sha256']
            result['manifest_sha256'] = map_identity(manifest)
        return result

    def check_visual(self, manifest, review, **options):
        options.setdefault('authorized_reviewers', ['delegated-reviewer'])
        options.setdefault('required_scope', ['reading/default'])
        options.setdefault('fixture', True)
        return validate_visual(self.root, manifest, review, **options)


class GatePolicyTests(PolicyFixtures):
    def test_actual_diff_includes_new_modified_and_deleted_files(self):
        self.write('scripts/checker.py', 'changed\n')
        self.write('scripts/new.py', 'new\n')
        (self.root / 'src/view.tsx').unlink()
        self.assertEqual(changed_paths(self.root, self.before),
                         ['scripts/checker.py', 'scripts/new.py', 'src/view.tsx'])

    def test_runtime_results_do_not_change_source_identity(self):
        self.write('.build/run.json', '{}')
        self.write('reports/generated.json', '{}')
        self.assertEqual(snapshot_inputs(self.root), self.before)

    def test_generated_tauri_staging_is_not_managed_source(self):
        self.write('apps/desktop/src-tauri/resources/language/dictionary.sqlite3', 'generated copy')
        self.assertEqual(snapshot_inputs(self.root), self.before)
        self.write('resources/language/manifest.json', '{}')
        self.assertIn('resources/language/manifest.json', snapshot_inputs(self.root))

    def test_finder_metadata_is_not_source_or_checkout_authority(self):
        self.write('.DS_Store', 'local Finder metadata')
        self.write('docs/.DS_Store', 'nested local metadata')
        self.assertEqual(snapshot_inputs(self.root), self.before)

    def test_gitignore_is_tooling_not_a_product_ui_change(self):
        result = classify_impact(['.gitignore'])
        self.assertTrue(result['tooling'])
        self.assertFalse(result['product'])
        self.assertNotIn('G09', result['required_gates'])

    def test_source_subset_uses_bytes_not_mtime(self):
        before = snapshot_inputs(self.root, ['scripts/checker.py'])
        self.write('scripts/checker.py', 'different\n')
        self.assertNotEqual(before, snapshot_inputs(self.root, before))

    def test_safe_inputs_reject_escape_and_symlink(self):
        with self.assertRaises(GatePolicyError):
            snapshot_inputs(self.root, ['../outside'])
        (self.root / 'linked.py').symlink_to(self.root / 'scripts/checker.py')
        with self.assertRaises(GatePolicyError):
            snapshot_inputs(self.root, ['linked.py'])

    def test_impact_is_from_paths_and_unknown_is_conservative(self):
        document = classify_impact(['docs/development/notes.md'])
        self.assertNotIn('G09', document['required_gates'])
        self.assertNotIn('G05', document['required_gates'])
        ui = classify_impact(['apps/desktop/src/ui/primitives/Select.tsx'])
        self.assertIn('G09', ui['required_gates'])
        self.assertIn('G08', ui['required_gates'])
        self.assertEqual(classify_impact(['unknown/production.py'], stage='release')['required_gates'], list(f'G{i:02d}' for i in range(1, 10)))

    def test_storage_impact_requires_real_storage_checks(self):
        result = classify_impact(['crates/app-infra/src/storage/migrations.rs'])
        self.assertIn('G07', result['required_gates'])

    def test_receipt_positive_has_actual_logs_and_counts(self):
        self.assertEqual(self.check_receipt(self.receipt())['status'], 'passed')

    def test_formal_receipt_rejects_fixture(self):
        result = self.check_receipt(self.receipt(), fixture=False)
        self.assertEqual(result['reason'], 'receipt.fixture_forbidden')

    def test_receipt_cannot_self_register_a_command(self):
        result = validate_receipt(self.root, self.receipt(), fixture=True)
        self.assertEqual(result['reason'], 'receipt.command_unregistered')

    def test_receipt_cannot_change_argv(self):
        result = self.check_receipt(self.receipt(command=['sh', '-c', 'echo fake']))
        self.assertEqual(result['reason'], 'receipt.command_mismatch')

    def test_receipt_requires_current_source_inputs(self):
        receipt = self.receipt()
        self.write('scripts/checker.py', 'changed\n')
        self.assertEqual(self.check_receipt(receipt)['reason'], 'receipt.stale_inputs')

    def test_receipt_cannot_omit_required_inputs(self):
        receipt = self.receipt(inputs=snapshot_inputs(self.root, ['scripts/checker.py']))
        self.assertEqual(self.check_receipt(receipt, expected_inputs=snapshot_inputs(self.root))['reason'],
                         'receipt.stale_inputs')

    def test_receipt_rejects_changed_raw_log(self):
        receipt = self.receipt()
        self.write('.build/stdout.log', 'pretend passed')
        self.assertTrue(self.check_receipt(receipt)['reason'].startswith('evidence.changed:'))

    def test_zero_tests_cannot_be_hidden_by_receipt_count(self):
        self.write('.build/stdout.log', 'Ran 0 tests in 0.001s\nOK\n')
        self.assertEqual(self.check_receipt(self.receipt(test_count=2))['reason'], 'receipt.test_count_mismatch')

    def test_test_obligation_cannot_be_downgraded_to_command(self):
        self.write('.build/stdout.log', 'Ran 0 tests in 0.001s\nOK\n')
        forged = self.receipt(check_kind='command', test_count=0)
        self.assertEqual(self.check_receipt(forged)['reason'], 'receipt.check_kind_mismatch')

    def test_skips_cannot_be_hidden_by_receipt_count(self):
        self.write('.build/stdout.log', 'Ran 2 tests in 0.001s\nOK (skipped=1)\n')
        self.assertEqual(self.check_receipt(self.receipt())['reason'], 'receipt.test_count_mismatch')

    def test_tests_need_observed_runner_summary(self):
        self.write('.build/stdout.log', '{"status":"passed","test_count":2}')
        self.assertEqual(self.check_receipt(self.receipt())['reason'], 'receipt.test_count_unobserved')

    def test_nonzero_exit_is_a_failed_gate_not_a_successful_negative(self):
        self.assertEqual(self.check_receipt(self.receipt(exit_code=1))['reason'], 'receipt.command_failed')

    def test_receipt_run_and_change_must_match(self):
        self.assertEqual(self.check_receipt(self.receipt(), change_id='another-change')['reason'], 'receipt.wrong_change')
        self.assertEqual(self.check_receipt(self.receipt(run_id=''))['reason'], 'receipt.run_id_missing')

    def test_receipt_times_are_ordered_and_not_future(self):
        now = datetime.now(timezone.utc)
        self.assertEqual(self.check_receipt(self.receipt(started_at=now.isoformat(),
                         finished_at=(now - timedelta(seconds=10)).isoformat()))['reason'], 'receipt.time_reversed')
        self.assertEqual(self.check_receipt(self.receipt(finished_at=(now + timedelta(days=1)).isoformat()))['reason'], 'receipt.time_in_future')

    def test_rust_and_tap_raw_counts(self):
        self.assertEqual(observed_test_counts('test result: ok. 3 passed; 0 failed; 0 ignored;\n'
                                             'test result: ok. 2 passed; 0 failed; 1 ignored;'), (6, 1))
        self.assertEqual(observed_test_counts('# tests 5\n# skipped 1\n'), (5, 1))

    def test_component_counts_consume_real_checks_artifact(self):
        artifact = {'kind': 'binding-ui-controlled-component-check/v1', 'status': 'passed',
                    'checks': [{'id': 'one', 'status': 'passed'}, {'id': 'two', 'status': 'passed'}],
                    'errors': [], 'sources': [{'path': 'scripts/checker.py', 'sha256': digest(self.root / 'scripts/checker.py')}]}
        self.write('.build/component.json', json.dumps(artifact))
        summary = {'status': 'passed', 'checks': 2, 'failed': [], 'errors': [], 'output': '.build'}
        self.write('.build/stdout.log', json.dumps(summary, indent=2) + '\n')
        receipt = self.receipt(test_observation=self.ref('.build/component.json'))
        self.assertEqual(self.check_receipt(receipt)['status'], 'passed')
        artifact['checks'][0]['status'] = 'failed'
        self.write('.build/component.json', json.dumps(artifact))
        receipt['test_observation'] = self.ref('.build/component.json')
        self.assertEqual(self.check_receipt(receipt)['reason'], 'receipt.component_summary_mismatch')

    def test_missing_visual_report_is_not_run(self):
        self.assertEqual(self.check_visual(None, None)['status'], 'not_run')

    def test_visual_human_approval_matches_current_report(self):
        manifest = self.visual()
        self.assertEqual(self.check_visual(manifest, self.review(manifest))['status'], 'passed')

    def test_visual_pending_or_missing_review_blocks(self):
        manifest = self.visual()
        self.assertEqual(self.check_visual(manifest, None)['status'], 'blocked')
        review = self.review(manifest)
        review['review_decision'] = 'pending'
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'review.pending')

    def test_visual_rejection_is_failed(self):
        manifest = self.visual()
        review = self.review(manifest)
        review['review_decision'] = 'rejected'
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'review.rejected')

    def test_visual_formal_product_rejects_fixtures(self):
        manifest = self.visual()
        self.assertEqual(self.check_visual(manifest, self.review(manifest), fixture=False)['reason'], 'visual.fixture_forbidden')

    def test_visual_reviewer_must_be_authorized_and_independent(self):
        manifest = self.visual()
        review = self.review(manifest)
        self.assertEqual(self.check_visual(manifest, review, authorized_reviewers=[])['reason'], 'review.unauthorized_or_author')
        review['author_ids'] = ['delegated-reviewer']
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'review.unauthorized_or_author')

    def test_visual_review_requires_real_confirmation_record(self):
        manifest = self.visual()
        review = self.review(manifest)
        (self.root / '.build/confirmation.txt').unlink()
        self.assertTrue(self.check_visual(manifest, review)['reason'].startswith('evidence.missing:'))

    def test_visual_cannot_extend_approved_scope(self):
        manifest = self.visual()
        self.assertEqual(self.check_visual(manifest, self.review(manifest),
                         required_scope=['reading/default', 'reading/failed'])['reason'], 'visual.coverage_missing')

    def test_visual_states_must_be_comparable(self):
        manifest = self.visual()
        manifest['rows'][0]['implementation_state'] = 'error'
        self.assertEqual(self.check_visual(manifest, None)['reason'], 'visual.state_mismatch')

    def test_visual_dimensions_are_read_from_actual_png(self):
        manifest = self.visual()
        manifest['rows'][0]['viewport']['width'] = 5
        self.assertEqual(self.check_visual(manifest, None)['reason'], 'visual.size_mismatch')

    def test_visual_image_crc_and_pixel_data_are_read(self):
        manifest = self.visual()
        self.write('reports/visual/implementation.png', png(decoded=b'not a valid pixel array'), binary=True)
        manifest['rows'][0]['implementation_image'] = self.ref('reports/visual/implementation.png')
        self.assertEqual(self.check_visual(manifest, None)['reason'], 'visual.image_stream')
        self.write('reports/visual/implementation.png', png()[:-1] + b'x', binary=True)
        manifest['rows'][0]['implementation_image'] = self.ref('reports/visual/implementation.png')
        self.assertEqual(self.check_visual(manifest, None)['reason'], 'visual.image_crc')

    def test_visual_same_file_or_prototype_cannot_be_actual_capture(self):
        manifest = self.visual()
        manifest['rows'][0]['implementation_image'] = deepcopy(manifest['rows'][0]['design_image'])
        self.assertEqual(self.check_visual(manifest, None)['reason'], 'visual.same_capture_file')
        manifest = self.visual()
        manifest['rows'][0]['source_kind'] = 'html_design'
        self.assertEqual(self.check_visual(manifest, None)['reason'], 'visual.real_capture_required')

    def test_visual_source_and_report_changes_invalidate_approval(self):
        manifest = self.visual()
        review = self.review(manifest)
        self.write('src/view.tsx', 'changed visual\n')
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'visual.stale_inputs')
        manifest = self.visual()
        review = self.review(manifest)
        self.write('reports/visual/index.html', 'changed report')
        self.assertTrue(self.check_visual(manifest, review)['reason'].startswith('evidence.changed:'))

    def test_visual_cannot_choose_a_partial_source_closure(self):
        manifest = self.visual()
        manifest['inputs'] = snapshot_inputs(self.root, ['scripts/checker.py'])
        review = self.review(manifest)
        review['inputs'] = manifest['inputs']
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'visual.stale_inputs')
        self.write('src/view.tsx', 'changed outside self-declared closure\n')
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'visual.stale_inputs')

    def test_visual_image_manifest_changes_invalidate_old_review(self):
        manifest = self.visual()
        review = self.review(manifest)
        self.write('reports/visual/implementation.png', png(color=(100, 50, 20, 255)), binary=True)
        manifest['rows'][0]['implementation_image'] = self.ref('reports/visual/implementation.png')
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'review.stale_manifest')

    def test_unresolved_visual_findings_do_not_pass(self):
        manifest = self.visual()
        review = self.review(manifest)
        review['unresolved_findings'] = ['missing button']
        self.assertEqual(self.check_visual(manifest, review)['reason'], 'review.unresolved_findings')

    def test_evaluate_scope_is_actual_and_cannot_be_suppressed(self):
        self.write('scripts/undeclared.py', 'outside\n')
        context = {'change_id': 'fixture-change', 'source_snapshot': self.before,
                   'allowed_paths': ['scripts/checker.py'], 'snapshot_excludes': ['scripts/undeclared.py'],
                   'actual_changed_paths': [], 'ui': False, 'status': 'passed'}
        summary = evaluate(self.root, context, fixture=True)
        g3 = next(row for row in summary['gates'] if row['gate_id'] == 'G03')
        self.assertEqual(g3['reason'], 'scope.undeclared')
        self.assertTrue(summary['incomplete'])

    def test_evaluate_visual_scope_must_come_from_trusted_registry(self):
        self.write('src/new.tsx', 'new UI\n')
        context = {'change_id': 'fixture-change', 'source_snapshot': self.before,
                   'allowed_paths': ['src/new.tsx'], 'visual': {'required_scope': ['fake']}}
        summary = evaluate(self.root, context, fixture=True)
        g9 = next(row for row in summary['gates'] if row['gate_id'] == 'G09')
        self.assertEqual(g9['reason'], 'visual.required_scope_unregistered')

    def test_evaluate_document_only_marks_visual_inapplicable_not_passed(self):
        self.write('docs/development/explanation.md', '# Explanation\n')
        context = {'change_id': 'fixture-change', 'source_snapshot': self.before,
                   'allowed_paths': ['docs/development/explanation.md']}
        summary = evaluate(self.root, context, fixture=True)
        g9 = next(row for row in summary['gates'] if row['gate_id'] == 'G09')
        self.assertFalse(g9['required'])
        self.assertEqual(g9['status'], 'not_run')

    def test_summary_never_ignores_a_required_manual_gate(self):
        rows = [gate_result(f'G{i:02d}', 'passed', 'fixture') for i in range(1, 10)]
        rows[-1] = gate_result('G09', 'blocked', 'review.pending')
        self.assertFalse(build_summary(rows)['next_stage_allowed'])
        self.assertEqual(build_summary(rows)['status'], 'blocked')

    def test_summary_requires_all_nine_unique_rows(self):
        with self.assertRaises(GatePolicyError):
            build_summary([gate_result('G01', 'passed', 'fixture')])

    def test_json_rejects_duplicate_keys_and_nonfinite_values(self):
        path = self.write('.build/invalid.json', '{"passed":true,"passed":false}')
        with self.assertRaises(GatePolicyError):
            read_json(path)
        path.write_text('{"x":NaN}')
        with self.assertRaises(GatePolicyError):
            read_json(path)

    def test_atomic_state_writes_and_lock_use_regular_safe_paths(self):
        with locked_state(self.root, '.build/state.json'):
            atomic_write_json(self.root, '.build/state.json', {'state': 'blocked'})
        self.assertEqual(read_json(self.root / '.build/state.json'), {'state': 'blocked'})
        with self.assertRaises(GatePolicyError):
            atomic_write_json(self.root, '../escape.json', {})

    def test_context_cannot_redirect_to_arbitrary_command(self):
        self.write('.codex/gate-context.json', json.dumps({'change_id': '../escape'}))
        with self.assertRaises(GatePolicyError):
            load_context(self.root)


if __name__ == '__main__':
    unittest.main()
