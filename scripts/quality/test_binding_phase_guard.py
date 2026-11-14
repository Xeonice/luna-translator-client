"""Small isolated phase/negative fixtures; never product acceptance."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import binding_phase_guard as g
from gate_core import source_files


class PhaseGuardTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.product = 'apps/desktop/src/fixture.ts'
        self.gate = 'scripts/quality/fixture_gate.py'
        self.write(self.product, 'original product')
        self.write('docs/plan.md', 'before')
        self.write('docs/approved.html', 'approved immutable design')
        self.before = source_files(self.root)
        self.policy = {
            'schema_version': 1, 'kind': 'ordered-binding-journey-phases/v1',
            'change_id': g.CHANGE, 'required_sequence': ['plan', 'gates', 'implementation'],
            'before_snapshot': g.PREFIX + '/source-snapshot.json',
            'before_tree_sha256': g.tree_sha(self.before),
            'parent_product_source_map': g.products(self.before),
            'product_completion': 'incomplete', 'next_stage_allowed': False,
            'authors': ['fixture-author'],
            'plan_paths': ['docs/plan.md', g.POLICY, g.PREFIX + '/source-snapshot.json', g.PREFIX + '/verification.json'],
            'gate_paths': [self.gate], 'implementation_paths': [self.product],
            'receipts': {'plan': '.build/p1/review.json', 'gates': '.build/p2/review.json',
                         'implementation_admission': '.build/p3/admission.json'}}
        self.json(g.PREFIX + '/source-snapshot.json',
                  {'files': self.before, 'source_entries': len(self.before), 'source_tree_sha256': g.tree_sha(self.before)})
        self.json(g.POLICY, self.policy)
        self.metadata = {'id': g.CHANGE, 'kind': 'product', 'phase_policy': g.POLICY,
                         'allowed_paths': sum([self.policy[k] for k in ('plan_paths', 'gate_paths', 'implementation_paths')], []),
                         'review_evidence': {'authors': self.policy['authors']}}
        self.json(g.PREFIX + '/verification.json', self.metadata)
        self.write('docs/plan.md', 'current plan')
        self.p1 = source_files(self.root)
        self.json('.build/p1/map.json', self.p1)
        self.design = {'docs/approved.html': g.ref(self.root, 'docs/approved.html')['sha256']}
        self.review = {'schema_version': 1, 'kind': 'independent-binding-plan-review/v1',
                       'change_id': g.CHANGE, 'phase': 'plan', 'status': 'approved',
                       'independent_non_author_review': True, 'blocking_findings': [],
                       'author_ids': self.policy['authors'], 'reviewer_id': 'fixture-independent-reviewer',
                       'before_source_tree_sha256': g.tree_sha(self.before), 'phase_admission': 'passed',
                       'implementation_admitted': False, 'product_completion': 'incomplete', 'next_stage_allowed': False,
                       'phase_policy': g.ref(self.root, g.POLICY), 'source_map': g.ref(self.root, '.build/p1/map.json'),
                       'phase_source_tree_sha256': g.tree_sha(self.p1), 'phase_source_entries': len(self.p1),
                       'product_source_map_sha256': g.compact_sha(g.products(self.p1)),
                       'plan_basis_sha256': {p: g.ref(self.root, p)['sha256'] for p in self.policy['plan_paths']},
                       'design_basis_sha256': self.design, 'evidence': []}
        self.json(self.policy['receipts']['plan'], self.review)
        self.review_pin = patch.object(g, 'PLAN_REVIEW_SHA256', g.ref(self.root, self.policy['receipts']['plan'])['sha256'])
        self.review_pin.start()
        self.addCleanup(self.review_pin.stop)
        # Independent authority fixture is deliberately small. The actual full
        # approved source closure is tested separately against the real repo.
        self.design_patch = patch.object(g, 'approved_design_basis', return_value=self.design)
        self.design_patch.start()
        self.addCleanup(self.design_patch.stop)

    def write(self, relative, value):
        p = self.root / relative
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(value)
        return p

    def json(self, relative, value):
        return self.write(relative, json.dumps(value, sort_keys=True, indent=2) + '\n')

    def failed(self):
        result = g.validate_phase(self.root)
        self.assertEqual(result['status'], 'failed', result)
        self.assertIs(result['implementation_admitted'], False)
        self.assertIs(result['next_stage_allowed'], False)
        return result

    def test_p1_then_only_gate_delta_are_admitted_without_product_promotion(self):
        result = g.validate_phase(self.root)
        self.assertEqual(result['phase'], 'plan')
        self.write(self.gate, 'actual gate source')
        result = g.validate_phase(self.root)
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(result['phase'], 'gates')
        self.assertFalse(result['implementation_admitted'])

    def test_product_edit_before_admission_is_rejected(self):
        self.write(self.product, 'premature product implementation')
        self.assertIn('non-gate', self.failed()['errors'][0])

    def test_unadmitted_directory_is_rejected(self):
        self.write('native/new-module.c', 'premature directory expansion')
        self.failed()

    def test_missing_plan_review_is_rejected(self):
        (self.root / self.policy['receipts']['plan']).unlink()
        self.failed()

    def test_self_review_is_rejected(self):
        self.review['reviewer_id'] = 'fixture-author'
        self.json(self.policy['receipts']['plan'], self.review)
        self.failed()

    def test_plan_and_metadata_changes_invalidate_p1(self):
        for name in ('docs/plan.md', g.PREFIX + '/verification.json', g.POLICY):
            with self.subTest(name=name):
                p = self.root / name
                raw = p.read_bytes()
                p.write_bytes(raw + b' ')
                self.failed()
                p.write_bytes(raw)

    def test_declared_paths_cannot_waive_product_change(self):
        self.policy['gate_paths'].append(self.product)
        self.policy['implementation_paths'].remove(self.product)
        self.json(g.POLICY, self.policy)
        self.write(self.product, 'waived by metadata')
        self.failed()

    def test_removed_or_replaced_design_basis_entry_is_rejected(self):
        self.review['design_basis_sha256'] = {'docs/plan.md': g.ref(self.root, 'docs/plan.md')['sha256']}
        self.json(self.policy['receipts']['plan'], self.review)
        self.failed()

    def test_stale_or_forged_source_map_is_rejected(self):
        p = self.root / '.build/p1/map.json'
        p.write_text('{}')
        self.failed()

    def amended_plan(self):
        """Real before/after maps plus a distinct derived view in a tiny fixture."""
        old_policy = copy.deepcopy(self.policy)
        parent_ref = g.ref(self.root, self.policy['receipts']['plan'])
        self.write('.build/r1/review.json', (self.root / parent_ref['path']).read_text())
        parent_ref = g.ref(self.root, '.build/r1/review.json')
        archive = {'schema_version': 1, 'kind': 'binding-plan-parent-source-archive/v1',
                   'parent_plan_review': parent_ref, 'sources': {}}
        for path, digest in self.review['plan_basis_sha256'].items():
            name = '.build/r1/archive/' + path
            self.write(name, (self.root / path).read_text())
            archive['sources'][path] = {'source_path': path, 'source_sha256': digest,
                                        'archived_file': g.ref(self.root, name)}
        self.json('.build/r1/archive.json', archive)
        self.write(self.gate, 'actual unsigned first gate generation')
        actual_before = source_files(self.root)
        self.json('.build/r1/unsigned-map.json', actual_before)
        packet = {'schema_version': 1, 'kind': 'binding-gate-phase-packet/v1',
                  'change_id': g.CHANGE, 'phase': 'gates', 'status': 'pending_independent_review',
                  'authors': old_policy['authors'], 'phase_policy': self.review['phase_policy'],
                  'parent_plan_review': {'path': old_policy['receipts']['plan'], 'sha256': parent_ref['sha256']},
                  'before_source_tree_sha256': g.tree_sha(self.before),
                  'phase2_source_tree_sha256': g.tree_sha(actual_before),
                  'phase2_source_map': g.ref(self.root, '.build/r1/unsigned-map.json'),
                  'product_source_map_sha256': g.compact_sha(g.products(actual_before)),
                  'plan_basis_sha256': self.review['plan_basis_sha256'],
                  'gate_basis_sha256': {self.gate: actual_before[self.gate]},
                  'product_completion': 'incomplete', 'next_stage_allowed': False}
        self.json('.build/r1/unsigned-packet.json', packet)
        self.policy['plan_paths'].append('docs/v5-contract.json')
        self.policy['gate_paths'].append('scripts/quality/new_v5_guard.py')
        self.write('docs/v5-contract.json', '{"independent_design":"v5"}')
        self.json(g.POLICY, self.policy)
        self.metadata['allowed_paths'] = sum([self.policy[k] for k in ('plan_paths', 'gate_paths', 'implementation_paths')], [])
        self.json(g.PREFIX + '/verification.json', self.metadata)
        after = source_files(self.root)
        self.json('.build/r2/actual-after.json', after)
        view = dict(self.before)
        view.update({path: after[path] for path in self.policy['plan_paths']})
        self.json('.build/r2/derived.json', view)
        amended = copy.deepcopy(self.review)
        amended.update(kind='independent-binding-plan-amendment-review/v1',
                       source_map=g.ref(self.root, '.build/r2/actual-after.json'),
                       actual_after_source_map=g.ref(self.root, '.build/r2/actual-after.json'),
                       actual_before_source_map=g.ref(self.root, '.build/r1/unsigned-map.json'),
                       parent_plan_review=parent_ref,
                       parent_unsigned_gate_packet=g.ref(self.root, '.build/r1/unsigned-packet.json'),
                       parent_plan_archive=g.ref(self.root, '.build/r1/archive.json'),
                       amendment_before_source_tree_sha256=g.tree_sha(actual_before),
                       phase_source_tree_sha256=g.tree_sha(after), phase_source_entries=len(after),
                       phase_policy=g.ref(self.root, g.POLICY),
                       plan_basis_sha256={path: after[path] for path in self.policy['plan_paths']},
                       actual_changed_paths=sorted(g.delta(actual_before, after)),
                       derived_view={'source_map': g.ref(self.root, '.build/r2/derived.json'),
                                     'entries': len(view), 'tree_sha256': g.tree_sha(view),
                                     'is_historical_actual_tree': False})
        self.json(self.policy['receipts']['plan'], amended)
        return amended, actual_before, after, view

    def pin_amendment(self, amended):
        self.json(self.policy['receipts']['plan'], amended)
        return patch.object(g, 'PLAN_AMENDMENT_REVIEW_SHA256',
                            g.ref(self.root, self.policy['receipts']['plan'])['sha256'])

    def test_plan_amendment_keeps_actual_history_and_returns_only_derived_comparison_view(self):
        amended, actual_before, after, view = self.amended_plan()
        with self.pin_amendment(amended):
            review, returned = g.verify_plan(self.root)
            self.assertEqual(returned, view)
            self.assertNotEqual(returned, after)
            self.assertEqual(g.load(g.checked_ref(self.root, review['source_map'], ignored=True)), after)
            self.assertEqual(g.validate_phase(self.root)['status'], 'passed')
            self.write('scripts/quality/new_v5_guard.py', 'new actual gate')
            self.assertEqual(g.validate_phase(self.root)['status'], 'passed')
            self.write(self.product, 'forbidden product write')
            self.failed()

    def test_amendment_rejects_fake_history_or_trimmed_parent_archive(self):
        amended, actual_before, after, view = self.amended_plan()
        for key, value in [('is_historical_actual_tree', True), ('entries', len(after)), ('tree_sha256', g.tree_sha(after))]:
            changed = copy.deepcopy(amended)
            changed['derived_view'][key] = value
            with self.pin_amendment(changed):
                self.failed()
        archive = g.load(self.root / '.build/r1/archive.json')
        archive['sources'].pop('docs/plan.md')
        self.json('.build/r1/archive.json', archive)
        amended['parent_plan_archive'] = g.ref(self.root, '.build/r1/archive.json')
        with self.pin_amendment(amended):
            self.failed()

    def test_amendment_cannot_relabel_unsigned_gate_candidate_as_approved(self):
        amended, *_ = self.amended_plan()
        packet = g.load(self.root / '.build/r1/unsigned-packet.json')
        packet['status'] = 'approved'
        self.json('.build/r1/unsigned-packet.json', packet)
        amended['parent_unsigned_gate_packet'] = g.ref(self.root, '.build/r1/unsigned-packet.json')
        with self.pin_amendment(amended):
            self.failed()

    def test_amendment_rejects_changed_archived_bytes_and_original_p2_review_replay(self):
        amended, actual_before, *_ = self.amended_plan()
        preserved = self.root / '.build/r1/archive/docs/plan.md'
        raw = preserved.read_bytes()
        preserved.write_bytes(raw + b' changed historical plan')
        with self.pin_amendment(amended):
            self.failed()
        preserved.write_bytes(raw)
        old_review = {'schema_version': 1, 'kind': 'independent-binding-gate-review/v1',
                      'change_id': g.CHANGE, 'phase': 'gates', 'status': 'approved',
                      'independent_non_author_review': True, 'blocking_findings': [],
                      'author_ids': self.policy['authors'], 'reviewer_id': 'fixture-independent-reviewer',
                      'packet': amended['parent_unsigned_gate_packet'],
                      'phase_source_tree_sha256': g.tree_sha(actual_before)}
        self.json(self.policy['receipts']['gates'], old_review)
        with self.pin_amendment(amended):
            with self.assertRaisesRegex(ValueError, 'P2 packet/policy/source identity'):
                g.verify_gate_review(self.root, require_current=True)

    def test_amendment_cannot_hide_product_or_old_gate_changes_in_actual_after(self):
        amended, _, after, _ = self.amended_plan()
        for path in (self.product, self.gate):
            changed = dict(after)
            changed[path] = 'a' * 64
            self.json('.build/r2/forged-after.json', changed)
            review = copy.deepcopy(amended)
            review['source_map'] = review['actual_after_source_map'] = g.ref(self.root, '.build/r2/forged-after.json')
            review['phase_source_tree_sha256'] = g.tree_sha(changed)
            review['actual_changed_paths'].append(path)
            with self.pin_amendment(review):
                self.failed()

    def test_duplicate_json_keys_and_symlink_evidence_are_rejected(self):
        self.write('.build/duplicate.json', '{"status":"approved","status":"failed"}')
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            g.load(self.root / '.build/duplicate.json')
        target = self.root / '.build/p1/review.json'
        (self.root / '.build/link.json').symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            g.path_at(self.root, '.build/link.json')

    def guard_receipt(self):
        self.write('.build/guards/stdout.log', '')
        self.write('.build/guards/stderr.log', 'test_real_guard ... ok\n\nRan 1 test in 0.01s\n\nOK\n')
        source = g.tree_sha(source_files(self.root))
        body = {'kind': 'binding-gate-guard-execution/v1', 'exit_code': 0,
                'command': [g.sys.executable, '-m', 'unittest', '-v', *g.GUARD_MODULES],
                'cwd': 'scripts/quality', 'source_tree_before_sha256': source,
                'source_tree_after_sha256': source,
                'started_at': '2026-11-10T00:00:00+00:00', 'finished_at': '2026-11-10T00:00:01+00:00',
                'stdout': g.ref(self.root, '.build/guards/stdout.log'),
                'stderr': g.ref(self.root, '.build/guards/stderr.log')}
        self.json('.build/guards/execution.json', body)
        return body

    def test_guard_log_requires_actual_count_and_rejects_skip_expected_failure(self):
        body = self.guard_receipt()
        self.assertEqual(g.inspect_guard_run(self.root, g.ref(self.root, '.build/guards/execution.json'))['exit_code'], 0)
        for raw in ('OK', 'Ran 0 tests\nOK', 'Ran 2 tests\nOK (skipped=1)', 'Ran 2 tests\nOK (expected failures=1)'):
            self.write('.build/guards/stderr.log', raw)
            body['stderr'] = g.ref(self.root, '.build/guards/stderr.log')
            self.json('.build/guards/execution.json', body)
            with self.assertRaises(ValueError):
                g.inspect_guard_run(self.root, g.ref(self.root, '.build/guards/execution.json'))

    def reviewed_p2(self):
        self.write(self.gate, 'frozen fixture gate')
        current = source_files(self.root)
        self.guard_receipt()
        self.write('.build/red/execution.json', '{}')
        self.inspector = patch('binding_journey_inspect.inspect_execution', return_value={
            'eligible_semantic_red': True, 'source_tree_sha256': g.tree_sha(current)})
        self.inspector.start()
        self.addCleanup(self.inspector.stop)
        packet = g.prepare_gate_packet(self.root, '.build/p2/packet',
                                      g.ref(self.root, '.build/red/execution.json'),
                                      g.ref(self.root, '.build/guards/execution.json'))
        review = {'schema_version': 1, 'kind': 'independent-binding-gate-review/v1',
                  'change_id': g.CHANGE, 'phase': 'gates', 'status': 'approved',
                  'independent_non_author_review': True, 'blocking_findings': [],
                  'author_ids': self.policy['authors'], 'reviewer_id': 'fixture-independent-reviewer',
                  'packet': g.ref(self.root, '.build/p2/packet/packet.json'),
                  'phase_source_tree_sha256': packet['phase2_source_tree_sha256']}
        self.json(self.policy['receipts']['gates'], review)

    def test_infrastructure_failure_cannot_be_used_as_semantic_red(self):
        self.guard_receipt()
        self.write('.build/red/execution.json', '{}')
        with patch('binding_journey_inspect.inspect_execution', return_value={'eligible_semantic_red': False,
                   'infrastructure_errors': ['browser crashed']}):
            with self.assertRaisesRegex(ValueError, 'infrastructure failure'):
                g.prepare_gate_packet(self.root, '.build/p2/fake', g.ref(self.root, '.build/red/execution.json'),
                                      g.ref(self.root, '.build/guards/execution.json'))

    def test_first_product_write_requires_fresh_p2_tree_before_admission(self):
        self.reviewed_p2()
        self.write(self.product, 'already changed')
        with self.assertRaisesRegex(ValueError, 'precede all product changes'):
            g.admit_implementation(self.root)

    def test_exclusive_p3_admission_then_only_implementation_paths(self):
        self.reviewed_p2()
        g.admit_implementation(self.root)
        with self.assertRaises(FileExistsError):
            g.admit_implementation(self.root)
        self.write(self.product, 'admitted product delta')
        result = g.validate_phase(self.root)
        self.assertEqual(result['phase'], 'implementation', result)
        self.assertTrue(result['implementation_admitted'])
        self.write(self.gate, 'drifting frozen gate')
        self.failed()

    def test_fake_admission_without_p2_review_is_rejected(self):
        self.json(self.policy['receipts']['implementation_admission'], {'implementation_admitted': True})
        self.failed()


class ResumptionArchiveAndIdentityTests(unittest.TestCase):
    """Guard fixtures only; never implementation or product acceptance."""
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.origin = self.root / '.build/history/progress.json'
        self.archive = self.root / '.build/checkpoint/ancestor-artifacts/.build/history/progress.json'
        self.origin.parent.mkdir(parents=True)
        self.archive.parent.mkdir(parents=True)
        self.origin.write_text('historical progress')
        self.archive.write_bytes(self.origin.read_bytes())
        self.row = {'original_path': self.origin.relative_to(self.root).as_posix(),
                    'archive_path': self.archive.relative_to(self.root).as_posix(),
                    'sha256': g.file_sha(self.archive), 'bytes': self.archive.stat().st_size}
        self.archive_root = self.root / '.build/checkpoint/ancestor-artifacts'

    def verify(self, row=None, safe=True):
        return g.verify_canonical_record(self.root, row or self.row, self.archive_root, is_safe=safe)

    def test_non_authority_progress_can_advance_while_archive_stays_exact(self):
        self.origin.write_text('new ordinary progress, not approval authority')
        self.verify()
        self.assertEqual(g.file_sha(self.archive), self.row['sha256'])

    def test_archive_changed_bytes_rejected(self):
        self.archive.write_text('changed archive')
        with self.assertRaises(ValueError): self.verify()

    def test_archive_changed_size_rejected(self):
        with self.assertRaises(ValueError): self.verify(dict(self.row, bytes=self.row['bytes'] + 1))

    def test_archive_dotdot_rejected(self):
        with self.assertRaises(ValueError): self.verify(dict(self.row, archive_path='.build/checkpoint/ancestor-artifacts/../escape'))

    def test_archive_outside_fixed_root_rejected(self):
        with self.assertRaises(ValueError): self.verify(dict(self.row, archive_path=self.row['original_path']))

    def test_origin_parent_symlink_rejected(self):
        self.origin.unlink(); self.origin.parent.rmdir()
        self.origin.parent.symlink_to(self.archive.parent, target_is_directory=True)
        with self.assertRaises(ValueError): self.verify()

    def test_origin_leaf_symlink_rejected(self):
        self.origin.unlink(); self.origin.symlink_to(self.archive)
        with self.assertRaises(ValueError): self.verify()

    def test_archive_symlink_rejected(self):
        self.archive.unlink(); self.archive.symlink_to(self.origin)
        with self.assertRaises(ValueError): self.verify()

    def test_archive_hardlink_rejected(self):
        self.archive.unlink(); self.archive.hardlink_to(self.origin)
        with self.assertRaises(ValueError): self.verify()

    def test_quarantined_origin_never_opened(self):
        self.verify(dict(self.row, original_path='../../unsafe/link', origin='untrusted_not_revalidated',
                         current_authority=False, tool_input_allowed=False, target_followed_by_reviewer=False), False)

    def test_quarantine_cannot_promote_authority(self):
        row = dict(self.row, origin='untrusted_not_revalidated', current_authority=True,
                   tool_input_allowed=False, target_followed_by_reviewer=False)
        with self.assertRaises(ValueError): self.verify(row, False)

    def test_quarantine_cannot_become_tool_input(self):
        row = dict(self.row, origin='untrusted_not_revalidated', current_authority=False,
                   tool_input_allowed=True, target_followed_by_reviewer=False)
        with self.assertRaises(ValueError): self.verify(row, False)

    def test_current_approval_reference_requires_exact_original(self):
        reference = g.ref(self.root, self.row['original_path'])
        self.origin.write_text('new approval claim')
        with self.assertRaises(ValueError): g.checked_ref(self.root, reference, ignored=True)

    def test_failure_reference_requires_exact_original(self):
        reference = g.ref(self.root, self.row['original_path'])
        self.origin.write_text('rewritten failure as success')
        with self.assertRaises(ValueError): g.checked_ref(self.root, reference, ignored=True)

    def test_historical_alias_is_only_lexical(self):
        self.assertEqual(g.lexical_archive_path('.build/a/../history/./progress.json'), self.row['original_path'])

    def test_historical_root_escape_rejected(self):
        for value in ('../escape', 'a/../../escape', '/absolute', 'C:\\escape', 'a\\b'):
            with self.subTest(value=value), self.assertRaises(ValueError): g.lexical_archive_path(value)

    def epoch_policy(self):
        return {'receipts': {'plan': 'old-plan', 'gates': 'old-gates', 'implementation_admission': 'old-admission'},
                'resume_epoch': {'schema_version': 1, 'kind': 'admitted-p3-plan-gates-resumption/v1',
                    'epoch_id': 'r3', 'product_completion': 'incomplete', 'next_stage_allowed': False,
                    'receipts': {'plan': '.build/binding-journey-alignment-20261109/phase-1-r3/plan-review.json',
                      'gates': '.build/binding-journey-alignment-20261109/phase-2-r3/gate-review.json',
                      'implementation_admission': '.build/binding-journey-alignment-20261109/phase-3-r3/admission.json',
                      'gate_packet': '.build/binding-journey-alignment-20261109/phase-2-r3/packet/packet.json'}}}

    def test_new_epoch_never_reuses_old_admission(self):
        policy = self.epoch_policy()
        self.assertNotEqual(g.phase_receipts(policy), policy['receipts'])
        policy['resume_epoch']['receipts']['implementation_admission'] = policy['receipts']['implementation_admission']
        with self.assertRaises(ValueError): g.phase_receipts(policy)

    def test_epoch_identity_and_product_completion_cannot_change(self):
        for key, bad in [('epoch_id', 'r4'), ('kind', 'reset-r92'), ('product_completion', 'complete'), ('next_stage_allowed', True)]:
            policy = self.epoch_policy(); policy['resume_epoch'][key] = bad
            with self.subTest(key=key), self.assertRaises(ValueError): g.phase_receipts(policy)

    def test_preservation_cannot_grant_next_stage(self):
        with self.assertRaises(ValueError):
            g.require_preservation_only({'product_completion': 'incomplete', 'next_stage_allowed': True})

    def test_legacy_receipt_paths_remain_exact(self):
        policy = {'receipts': {'plan': 'r2-plan', 'gates': 'r2-gate', 'implementation_admission': 'r2-admit'}}
        self.assertEqual(g.phase_receipts(policy), policy['receipts'])

    def historical_fixture(self):
        def write(path,value):
            target=self.root/path;target.parent.mkdir(parents=True,exist_ok=True)
            target.write_text(json.dumps(value,sort_keys=True));return g.ref(self.root,path)
        policy=self.epoch_policy();policy.update(authors=['author'],plan_paths=['docs/plan.md',g.POLICY],
            gate_paths=['scripts/quality/gate.py'],implementation_paths=['apps/desktop/src/fixture.ts'])
        checkpoint=write('.build/checkpoint.json',{'historical':'sealed'})
        checkpoint_review=write('.build/checkpoint-review.json',{'independently_preserved':True})
        parent=write('.build/original-admission.json',{'original_p3':True})
        policy['resume_epoch'].update(checkpoint={'manifest':checkpoint,'independent_review':checkpoint_review},
            ancestors={'original_implementation':{'admission':parent}})
        for path in ['docs/plan.md','scripts/quality/gate.py','apps/desktop/src/fixture.ts']:
            target=self.root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_text('frozen')
        write(g.POLICY,policy)
        receipts=g.phase_receipts(policy)
        plan=write(receipts['plan'],{'fixture':'actual prevalidated P1 record'})
        frozen=source_files(self.root);source=write('.build/source-map.json',frozen)
        packet={'epoch_id':'r3','phase2_source_tree_sha256':g.tree_sha(frozen),'phase2_source_map':source,
            'plan_basis_sha256':{path:frozen[path] for path in policy['plan_paths']},
            'gate_basis_sha256':{path:frozen[path] for path in policy['gate_paths']}}
        packet_ref=write('.build/packet.json',packet)
        review={'schema_version':1,'kind':'independent-binding-gate-review/v1','change_id':g.CHANGE,'phase':'gates','status':'approved',
            'author_ids':['author'],'reviewer_id':'independent','independent_non_author_review':True,'blocking_findings':[],
            'epoch_id':'r3','phase_source_tree_sha256':g.tree_sha(frozen),'packet':packet_ref}
        gate=write(receipts['gates'],review)
        admission={'kind':'binding-implementation-resume-admission/v1','schema_version':1,'change_id':g.CHANGE,'epoch_id':'r3',
            'implementation_admitted':True,'phase2_source_tree_sha256':g.tree_sha(frozen),'phase2_source_map':source,
            'gate_review':gate,'plan_review':plan,'phase_policy':g.ref(self.root,g.POLICY),'checkpoint':checkpoint,
            'checkpoint_review':checkpoint_review,'parent_admission':parent,'product_completion':'incomplete','next_stage_allowed':False,
            'plan_basis_sha256':packet['plan_basis_sha256'],'gate_basis_sha256':packet['gate_basis_sha256']}
        write(receipts['implementation_admission'],admission)
        self.enterContext(patch.object(g,'PLAN_RESUMPTION_REVIEW_SHA256',plan['sha256']))
        return policy,frozen,receipts

    def test_legal_admitted_r3_product_advance_allows_historical_read_only(self):
        policy,frozen,receipts=self.historical_fixture()
        (self.root/'apps/desktop/src/fixture.ts').write_text('legally implemented after admission')
        actual=g.verify_r3_historical_admission(self.root,policy,frozen,source_files(self.root))
        self.assertEqual(actual,g.ref(self.root,receipts['implementation_admission']))

    def test_historical_exception_cannot_hide_gate_or_plan_drift(self):
        for path in ['docs/plan.md','scripts/quality/gate.py']:
            policy,frozen,_=self.historical_fixture();target=self.root/path;raw=target.read_bytes();target.write_bytes(raw+b' changed')
            with self.assertRaises(ValueError):g.verify_r3_historical_admission(self.root,policy,frozen,source_files(self.root))
            target.write_bytes(raw)

    def test_historical_exception_cannot_hide_unadmitted_path(self):
        policy,frozen,_=self.historical_fixture();target=self.root/'unexpected.rs';target.write_text('unadmitted')
        with self.assertRaises(ValueError):g.verify_r3_historical_admission(self.root,policy,frozen,source_files(self.root))

    def test_historical_exception_keeps_original_map_and_review_hashes_strict(self):
        policy,frozen,_=self.historical_fixture();target=self.root/'.build/source-map.json';target.write_text('{}')
        with self.assertRaises(ValueError):g.verify_r3_historical_admission(self.root,policy,frozen,source_files(self.root))

    def test_historical_exception_requires_real_new_admission(self):
        policy,frozen,receipts=self.historical_fixture();(self.root/receipts['implementation_admission']).unlink()
        with self.assertRaises(ValueError):g.verify_r3_historical_admission(self.root,policy,frozen,source_files(self.root))


class ActualDesignClosureTest(unittest.TestCase):
    def test_current_complete_approved_design_closure_is_exact_74(self):
        # Read-only actual approved inputs; no screenshot or approval is changed.
        basis = g.approved_design_basis(g.ROOT)
        self.assertEqual(len(basis), 74)
        review = g.load(g.ROOT / '.build/binding-journey-alignment-20261109/phase-1/plan-review.json')
        self.assertEqual(review['design_basis_sha256'], basis)


if __name__ == '__main__':
    unittest.main()


class SuccessorHandoffGuards(unittest.TestCase):
    """New successor controls do not change or stand in for old phase tests."""
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()

    def put(self, path, value):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(value)
        return {'path': path, 'sha256': g.sha(target.read_bytes()), 'bytes': target.stat().st_size}

    def test_new_id_legacy_noop_never_grants_successor_permission(self):
        legacy = g.validate_phase(self.root, g.SUCCESSOR_CHANGE)
        self.assertFalse(legacy['implementation_admitted'])
        result = g.validate_successor_handoff(self.root, g.SUCCESSOR_CHANGE)
        self.assertEqual(result['status'], 'failed')
        self.assertFalse(result['product_write_allowed'])
        self.assertFalse(result['next_stage_allowed'])

    def test_requested_phase_alone_does_not_admit_and_actual_paths_require_handoff(self):
        self.assertTrue(g.successor_handoff_required(self.root, 'renamed-bypass', ['scripts/quality/verify.py']))
        self.assertTrue(g.successor_handoff_required(self.root, g.SUCCESSOR_CHANGE, []))
        self.assertFalse(g.successor_handoff_required(self.root, 'unrelated', ['docs/unrelated.md']))

    def test_scope_accepts_only_reviewed_qa_and_zero_product_diff(self):
        before = {'apps/a.rs': 'a' * 64, 'qa.py': 'b' * 64, 'plan.md': 'c' * 64}
        review = {'canonical_plan_basis_sha256': {'plan.md': 'c' * 64},
                  'qa_code_and_doc_phase_paths': ['qa.py'], 'deferred_independent_adoption_paths': ['baseline.json']}
        current = {**before, 'qa.py': 'd' * 64}
        self.assertEqual(g.successor_scope_errors(self.root, review, before, before, current), [])
        for path in ('apps/a.rs', 'plan.md', 'extra.py', 'baseline.json'):
            with self.subTest(path=path):
                bad = {**current, path: 'e' * 64}
                self.assertTrue(g.successor_scope_errors(self.root, review, before, before, bad))

    def test_ordinary_exact_snapshot_can_outlive_changed_origin(self):
        original = self.put('.build/ordinary.md', 'old')
        snap = self.put('.build/snapshot.md', 'old')
        disposition = {'original_ref': original, 'snapshot': snap,
                       'classification': 'ordinary_context_summary_historical_only',
                       'future_original_path_hash_required': False, 'current_authority_granted': False}
        self.put('.build/ordinary.md', 'legitimate updated notes')
        self.assertIsNone(g.check_successor_retained_refs(self.root, [original], [disposition]))
        self.put('.build/snapshot.md', 'tampered archive')
        with self.assertRaises(ValueError):
            g.check_successor_retained_refs(self.root, [original], [disposition])

    def test_nonordinary_current_raw_and_failed_original_stay_exact(self):
        raw = self.put('.build/failure.log', 'actual failed run')
        self.put('.build/failure.log', 'replaced with pass')
        with self.assertRaises(ValueError):
            g.check_successor_retained_refs(self.root, [raw], [])

    def test_alias_escape_and_symlink_never_receive_ordinary_exemption(self):
        raw = self.put('.build/original.json', '{}')
        for bad in ({**raw, 'path': '../outside.json'}, {**raw, 'path': '.build/../original.json'}):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                g.successor_checked_ref(self.root, bad)
        (self.root / '.build/alias.json').symlink_to(self.root / raw['path'])
        with self.assertRaises(ValueError):
            g.successor_checked_ref(self.root, {**raw, 'path': '.build/alias.json'})

    def test_whole_retained_index_cannot_receive_ordinary_exemption(self):
        raw = self.put('.build/raw.log', 'raw')
        snap = self.put('.build/snapshot.log', 'raw')
        disposition = {'original_ref': raw, 'snapshot': snap, 'classification': 'ordinary_context_summary_historical_only',
                       'future_original_path_hash_required': True, 'current_authority_granted': False}
        with self.assertRaises(ValueError):
            g.check_successor_retained_refs(self.root, [raw], [disposition])

    def test_mutated_archive_ref_and_actual_guard_raw_are_rejected(self):
        raw = self.put('.build/oldguard.json', '{"status":"passed"}')
        self.assertEqual(g.successor_checked_ref(self.root, raw).read_text(), '{"status":"passed"}')
        with self.assertRaises(ValueError):
            g.successor_checked_ref(self.root, {**raw, 'bytes': raw['bytes'] + 1})
        self.put(raw['path'], '{"status":"failed"}')
        with self.assertRaises(ValueError):
            g.successor_checked_ref(self.root, raw)


class CompleteSuccessorEnvelopeGuards(unittest.TestCase):
    """Small complete sealed cohort, all guard bytes generated in TemporaryDirectory."""
    put = SuccessorHandoffGuards.put

    def setUp(self):
        SuccessorHandoffGuards.setUp(self)
        self.authors = ['fixture-author']
        self.qa = ['scripts/quality/verify.py'] + ['qa/allowed-' + str(n) + '.py' for n in range(18)]
        self.canonical = [g.SUCCESSOR_PREFIX + '/' + p for p in (
            'proposal.md', 'design.md', 'tasks.md', 'iteration-card.md', 'verification.json',
            'source-snapshot.json', 'specs/binding-engineering-integration/spec.md')]
        self.deferred = ['docs/development/iteration-baseline.json', 'docs/development/baselines/FP1-20261109-R94.json',
                         'docs/development/binding-engineering-integration-adoption-20261110.json']
        archives = {}
        for group, count in [('plan', 23), ('gates', 16), ('approved_design', 74), ('adopted_outputs', 8)]:
            archives[group] = []
            for n in range(count):
                original = self.put('parent/' + group + '/' + str(n) + '.txt', group + str(n))
                saved = self.put('.build/archive/' + group + '/' + str(n) + '.txt', group + str(n))
                archives[group].append({'original_path': original['path'], 'sha256': original['sha256'],
                                        'bytes': original['bytes'], 'archive': saved})
        self.put('apps/game.rs', 'frozen product')
        self.parent = source_files(self.root)
        self.jput('.build/oldguard/before.json', self.parent)
        self.jput('.build/oldguard/after.json', self.parent)
        raw = {'kind': 'binding-phase-admission-check/v1', 'change_id': g.CHANGE, 'required': True,
               'implementation_admitted': True, 'status': 'passed', 'errors': [],
               'source_tree_sha256': g.tree_sha(self.parent), 'product_completion': 'incomplete', 'next_stage_allowed': False}
        stdout = self.jput('.build/oldguard/stdout.log', raw)
        stderr = self.put('.build/oldguard/stderr.log', '')
        oldguard = self.jput('.build/oldguard/execution.json', {'kind': 'actual-command-execution/v1',
            'argv': ['python3', 'scripts/quality/binding_phase_guard.py', '--check', '--change', g.CHANGE, '--json'],
            'cwd': str(self.root), 'pid': 12345, 'started_at': '2026-11-10T10:00:00+00:00',
            'finished_at': '2026-11-10T10:00:01+00:00', 'exit_code': 0, 'source_stable': True,
            'before_tree_sha256': g.tree_sha(self.parent), 'after_tree_sha256': g.tree_sha(self.parent),
            'before_count': len(self.parent), 'after_count': len(self.parent),
            'stdout_sha256': stdout['sha256'], 'stderr_sha256': stderr['sha256']})
        self.failure = self.put('.build/actual-failed.log', 'retained failed run')
        ordinary = self.put('.build/ordinary.md', 'ordinary at seal time')
        snapshot = self.put('.build/ordinary-snapshot.md', 'ordinary at seal time')
        self.dispositions = [{'original_ref': ordinary, 'snapshot': snapshot,
            'classification': 'ordinary_context_summary_historical_only', 'future_original_path_hash_required': False,
            'current_authority_granted': False}]
        dummy = self.jput('.build/dummy.json', {})
        seal = {'kind': 'binding-engineering-integration-parent-seal/v1', 'parent_change_id': g.CHANGE,
            'proposed_successor_change_id': g.SUCCESSOR_CHANGE, 'author_ids': self.authors,
            'source_tree_sha256': g.tree_sha(self.parent), 'source_entries': len(self.parent),
            'source_map': self.jput('.build/parent-map.json', self.parent),
            'source_snapshot': self.jput('.build/parent-snapshot.json', {'files': self.parent}),
            'product_map': self.jput('.build/product-map.json', g.products(self.parent)),
            'product_source_count': len(g.products(self.parent)), 'product_map_canonical_sha256': g.compact_sha(g.products(self.parent)),
            'actual_old_guard': oldguard, 'actual_old_guard_raw_stdout': stdout,
            'actual_adoption': self.jput('.build/adoption.json', {'status': 'adopted',
                'source_tree_sha256': g.tree_sha(self.parent), 'source_entries': len(self.parent),
                'product_acceptance_promoted': False, 'outputs': [
                    {'path': row['original_path'], 'sha256': row['sha256'], 'bytes': row['bytes']}
                    for row in archives['adopted_outputs']]}),
            'authority_archives': self.jput('.build/archives.json', archives),
            'direct_parent_chain': self.jput('.build/chain.json', {oldguard['path']: oldguard}),
            'historical_boundaries': self.jput('.build/history.json', {'origin_fields_followed': False,
                'raw_aliases_accepted_as_current_authority': False, 'quarantined_declared_claims_promoted': False,
                'documents': [dummy]}),
            'original_failed_refs': self.jput('.build/failed-index.json', {'count': 1, 'refs': [self.failure]}),
            'current_execution_failure_refs': self.jput('.build/current-index.json', {'count': 2, 'refs': [ordinary, oldguard]})}
        for key in ('actual_adoption_execution', 'post_adoption_non_author_review', 'actual_pin_delta',
                    'unchanged_parent_pins', 'approved_design_map', 'adopted_R93_source_review',
                    'Native_source_controlled_units_review', 'successor_plan02_review', 'seal_checks', 'schema', 'generator'):
            seal[key] = dummy
        self.seal_ref = self.jput('.build/seal.json', seal)
        parent_review = {'kind': 'independent-binding-engineering-parent-seal-review/v1', 'status': 'approved',
            'independent_non_author_review': True, 'parent_seal': self.seal_ref, 'reviewer_id': 'fixture-independent',
            'ordinary_ref_dispositions': self.dispositions, 'ordinary_disposition_scope': 'exact reviewed ordinary fixture'}
        self.parent_review_ref = self.jput('.build/parent-review.json', parent_review)
        self.allowed = self.qa + self.canonical + self.deferred
        self.roles = [{'path': p, 'owner_actor': 'fixture-author'} for p in self.allowed]
        metadata = {'allowed_paths': self.allowed, 'review_evidence': {'authors': self.authors}, 'path_responsibilities': self.roles,
            'successor_handoff': {'kind': 'binding-successor-handoff-request/v1',
                'canonical_plan_review_path': '.build/canonical-review.json', 'parent_seal': self.seal_ref,
                'parent_seal_independent_review': self.parent_review_ref,
                'ordinary_ref_dispositions': self.dispositions, 'ordinary_disposition_scope': 'exact reviewed ordinary fixture',
                'approved_plan_basis': {'actual_old_guard': oldguard}}}
        for p in self.canonical:
            self.put(p, 'approved canonical ' + p)
        self.jput(g.SUCCESSOR_PREFIX + '/verification.json', metadata)
        self.jput(g.SUCCESSOR_PREFIX + '/source-snapshot.json', {'files': self.parent})
        canonical = source_files(self.root)
        self.review = {'schema_version': 1, 'kind': 'binding-successor-canonical-plan-review/v1',
            'status': 'approved', 'independent_non_author_review': True, 'change_id': g.SUCCESSOR_CHANGE,
            'parent_change_id': g.CHANGE, 'reviewer_id': 'fixture-independent', 'author_ids': self.authors,
            'approval_scope': 'exact_29_path_QA_engineering_maintenance_only',
            'canonical_plan_mutation_allowed_after_this_review': False, 'product_write_allowed': False,
            'source_tree_sha256': g.tree_sha(canonical), 'source_entries': len(canonical),
            'parent_source_tree_sha256': g.tree_sha(self.parent),
            'source_map': self.jput('.build/canonical-map.json', canonical),
            'allowed_paths': self.allowed, 'path_responsibilities': self.roles,
            'parent_seal': self.seal_ref, 'parent_seal_review': self.parent_review_ref,
            'ordinary_ref_dispositions': self.dispositions, 'ordinary_disposition_scope': 'exact reviewed ordinary fixture',
            'canonical_plan_basis_sha256': {p: canonical[p] for p in self.canonical},
            'qa_code_and_doc_phase_paths': self.qa, 'deferred_independent_adoption_paths': self.deferred}
        review_ref = self.jput('.build/canonical-review.json', self.review)
        review_ref.pop('bytes')
        authorization = {'kind': 'root-explicit-qa-maintenance-authorization/v1', 'authorizer': '/root',
            'change_id': g.SUCCESSOR_CHANGE, 'canonical_plan_review': review_ref,
            'source_tree_at_authorization': g.tree_sha(canonical), 'allowed_paths': self.allowed,
            'path_responsibilities': self.roles, 'product_write_allowed': False}
        auth_ref = self.jput('.build/root-authorization.json', authorization)
        auth_ref.pop('bytes')
        for key, value in [('SUCCESSOR_REVIEW', review_ref), ('SUCCESSOR_AUTHORIZATION', auth_ref)]:
            patched = patch.object(g, key, value)
            patched.start()
            self.addCleanup(patched.stop)
        self.put(self.qa[0], 'authorized QA implementation')

    def jput(self, path, value):
        return self.put(path, json.dumps(value, sort_keys=True))

    def check(self):
        return g.validate_successor_handoff(self.root, g.SUCCESSOR_CHANGE)

    def test_complete_sealed_parent_and_exact_qa_only_delta_pass(self):
        result = self.check()
        self.assertEqual(result['status'], 'passed', result)
        self.assertTrue(result['qa_maintenance_admitted'])
        self.assertFalse(result['implementation_admitted'])

    def test_completed_parent_raw_map_and_archived_copy_cannot_be_tampered(self):
        self.put('.build/archive/gates/0.txt', 'replaced gate')
        self.assertEqual(self.check()['status'], 'failed')

    def test_author_self_review_and_mutated_parent_approval_rejected(self):
        self.put('.build/parent-review.json', '{"status":"approved"}')
        self.assertEqual(self.check()['status'], 'failed')

    def test_actual_parent_failed_raw_and_current_product_tamper_rejected(self):
        self.put(self.failure['path'], 'promoted pass')
        self.assertEqual(self.check()['status'], 'failed')

    def test_safe_ordinary_original_update_is_allowed_but_frozen_snapshot_still_checked(self):
        self.put('.build/ordinary.md', 'new ordinary notes')
        self.assertEqual(self.check()['status'], 'passed')
        self.put('.build/ordinary-snapshot.md', 'rewritten sealed note')
        self.assertEqual(self.check()['status'], 'failed')

    def test_actual_old_guard_complete_before_map_cannot_be_replaced_by_projection(self):
        self.jput('.build/oldguard/before.json', {k: v for k, v in self.parent.items() if not k.startswith('apps/')})
        self.assertEqual(self.check()['status'], 'failed')


class SuccessorAdoptionGuards(unittest.TestCase):
    put = SuccessorHandoffGuards.put

    def setUp(self):
        SuccessorHandoffGuards.setUp(self)
        self.paths = ['docs/development/iteration-baseline.json', 'docs/development/baselines/FP1-20261109-R94.json',
                      'docs/development/binding-engineering-integration-adoption-20261110.json']
        self.review = {'author_ids': ['fixture-author'], 'deferred_independent_adoption_paths': self.paths}
        self.put('qa/current.py', 'reviewed source')
        old_baseline = {'baseline_id': 'FP1-20261109-R93', 'kept': {'number': 1, 'empty': []}, 'gone': None}
        new_baseline = {'baseline_id': 'FP1-20261109-R94', 'kept': {'number': 1, 'empty': []}, 'added': True}
        self.jput(self.paths[0], old_baseline)
        old_ref = self.jput('.build/old-baseline.json', old_baseline)
        patched = patch.object(g, 'R93_BASELINE_SHA256', old_ref['sha256'])
        patched.start()
        self.addCleanup(patched.stop)
        self.before = source_files(self.root)
        self.jput(self.paths[0], new_baseline)
        self.jput(self.paths[1], new_baseline)
        self.jput(self.paths[2], {'adoption': 'guard fixture'})
        candidate_ref = self.jput('.build/candidate-baseline.json', new_baseline)
        pin_delta = self.jput('.build/pin-delta.json', {'algorithm': 'json-leaves/v1',
            'before_baseline_ref': old_ref, 'after_baseline_ref': candidate_ref,
            'rows': [{'path': '/added', 'before_exists': False, 'before': None, 'after_exists': True, 'after': True},
                     {'path': '/baseline_id', 'before_exists': True, 'before': 'FP1-20261109-R93',
                      'after_exists': True, 'after': 'FP1-20261109-R94'},
                     {'path': '/gone', 'before_exists': True, 'before': None, 'after_exists': False, 'after': None}]})
        untouched = self.jput('.build/untouched.json', {'algorithm': 'json-leaves/v1',
            'before_baseline_ref': old_ref, 'after_baseline_ref': candidate_ref,
            'rows': {'/kept/empty': [], '/kept/number': 1}})
        self.current = source_files(self.root)
        self.base = g.SUCCESSOR_ADOPTION
        tool = self.put('.build/tool.py', 'frozen adoption tool')
        before = self.jput('.build/input-map.json', self.before)
        after = self.jput('.build/after-map.json', self.current)
        input_review = self.jput('.build/input-review.json', {'schema_version': 1,
            'kind': 'independent-binding-engineering-source-review/v1', 'status': 'approved',
            'independent_non_author_review': True, 'author_ids': ['fixture-author'], 'reviewer_id': 'fixture-independent',
            'source_tree_sha256': g.tree_sha(self.before), 'source_entries': len(self.before), 'source_map': before,
            'product_source_delta': [], 'product_acceptance_promoted': False, 'runtime_package_modified': False,
            'product_completion': 'incomplete'})
        self.manifest = {'schema_version': 1, 'kind': 'binding-engineering-exact-adoption-packet/v1',
            'change_id': g.SUCCESSOR_CHANGE, 'authors': ['fixture-author'],
            'parent_baseline': {'id': 'FP1-20261109-R93', 'sha256': g.R93_BASELINE_SHA256},
            'input_source_map': before, 'after_source_map': after,
            'input_source_tree_sha256': g.tree_sha(self.before), 'input_source_entries': len(self.before),
            'after_source_tree_sha256': g.tree_sha(self.current), 'after_source_entries': len(self.current),
            'outputs': [self.identity(p) for p in self.paths], 'prepare': tool, 'generator': tool,
            'pin_delta': pin_delta, 'untouched_pins': untouched, 'source_review': input_review}
        self.publish()

    def jput(self, path, value):
        return self.put(path, json.dumps(value, sort_keys=True))

    def identity(self, path):
        target = self.root / path
        return {'path': path, 'sha256': g.sha(target.read_bytes()), 'bytes': target.stat().st_size}

    def publish(self):
        manifest_ref = self.jput(self.base + '/packet/manifest.json', self.manifest)
        manifest_ref.pop('bytes')
        self.approval = {'schema_version': 1, 'kind': 'independent-binding-engineering-adoption-packet-review/v1',
            'status': 'approved', 'independent_non_author_review': True, 'author_ids': ['fixture-author'],
            'reviewer_id': 'fixture-independent', 'manifest': manifest_ref,
            **{key: self.manifest[key] for key in ('prepare', 'generator', 'input_source_tree_sha256',
                'after_source_tree_sha256', 'pin_delta', 'outputs')}}
        approval_ref = self.jput(self.base + '/packet-review.json', self.approval)
        self.receipt = {'schema_version': 1, 'status': 'adopted',
            'source_tree_sha256': g.tree_sha(self.current), 'source_entries': len(self.current),
            'outputs': self.manifest['outputs'], 'product_acceptance_promoted': False, 'runtime_package_modified': False,
            'independent_review': {'review_path': approval_ref['path'], 'review_sha256': approval_ref['sha256'],
                'reviewer_id': 'fixture-independent', 'manifest_sha256': manifest_ref['sha256']}}
        self.jput(self.base + '/transaction/receipt.json', self.receipt)

    def errors(self):
        return g.successor_adoption_errors(self.root, self.review, source_files(self.root))

    def test_exact_independently_approved_actual_three_output_transaction(self):
        self.assertEqual(self.errors(), [])

    def test_missing_review_or_receipt_never_authorizes_three_allowed_paths(self):
        (self.root / (self.base + '/transaction/receipt.json')).unlink()
        self.assertTrue(self.errors())

    def test_projected_after_without_actual_current_bytes_rejected(self):
        self.put(self.paths[1], 'rolled back output')
        self.assertTrue(self.errors())

    def test_changed_nonoutput_source_and_stale_input_review_rejected(self):
        self.put('qa/current.py', 'after-review edit')
        self.assertTrue(self.errors())

    def test_self_review_and_wrong_manifest_receipt_are_rejected(self):
        self.approval['reviewer_id'] = 'fixture-author'
        self.jput(self.base + '/packet-review.json', self.approval)
        self.assertTrue(self.errors())
        self.publish()
        self.receipt['independent_review']['manifest_sha256'] = 'f' * 64
        self.jput(self.base + '/transaction/receipt.json', self.receipt)
        self.assertTrue(self.errors())

    def test_changed_pin_delta_or_generator_invalidates_packet_approval(self):
        self.put('.build/tool.py', 'changed approved generator or pin proof')
        self.assertTrue(self.errors())

    def test_after_projection_cannot_stand_in_for_actual_input_source_review(self):
        source_review = json.loads((self.root / '.build/input-review.json').read_text())
        source_review['source_tree_sha256'] = g.tree_sha(self.current)
        self.manifest['source_review'] = self.jput('.build/input-review.json', source_review)
        self.publish()
        self.assertTrue(self.errors())

    def test_typed_pin_delta_cannot_treat_true_as_one_or_null_as_missing(self):
        proof = json.loads((self.root / '.build/untouched.json').read_text())
        proof['rows']['/kept/number'] = True
        self.manifest['untouched_pins'] = self.jput('.build/untouched.json', proof)
        self.publish()
        self.assertTrue(self.errors())
        changed = json.loads((self.root / '.build/pin-delta.json').read_text())
        changed['rows'][0]['before_exists'] = True
        self.manifest['pin_delta'] = self.jput('.build/pin-delta.json', changed)
        self.publish()
        self.assertTrue(self.errors())

    def test_json_leaf_escaping_and_empty_containers_are_not_omitted(self):
        self.assertEqual(g.successor_json_leaves({'a/b': {'~': None}, 'empty': [], 'object': {}}),
                         {'/a~1b/~0': None, '/empty': [], '/object': {}})


class R2AuthorityAndStageGuards(unittest.TestCase):
    """The active epoch and phase permissions are code-owned, not metadata grants."""
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()

    def contracts(self):
        actors=['/root','/root/binding_resumption_gate_implementation','/root/binding_source_audit','/root/storage_integration_fix']
        canonical=[g.SUCCESSOR_PREFIX+'/'+p for p in ('proposal.md','design.md','tasks.md','iteration-card.md','verification.json','source-snapshot.json','specs/binding-engineering-integration/spec.md')]
        qa=list(g.R2_GATE_PATHS)+list(g.R2_ROOT_DOC_PATHS)+['qa/retained-'+str(n)+'.py' for n in range(11)]
        old_outputs=list(g.R2_ATOMIC_OUTPUTS[3:])
        old_paths=qa+canonical+old_outputs
        paths=old_paths+list(g.R2_CATALOGUE_PATHS)
        parent={'path':'.build/checkpoint.json','sha256':'a'*64}
        roles=[{'path':p,'owner_actor':actors[0]} for p in paths]
        r1={'allowed_paths':old_paths,'author_ids':actors,'ordinary_ref_dispositions':[],'ordinary_disposition_scope':'fixture ordinary scope'}
        basis={p:'d'*64 for p in canonical}
        review={'schema_version':1,'kind':'binding-successor-canonical-plan-amendment-review/v1','status':'approved',
            'approval_scope':'exact_r2_canonical_plan_gate3_and_conditioned_Root_docs_only',
            'change_id':g.SUCCESSOR_CHANGE,'epoch_id':g.R2_EPOCH,'predecessor_epoch_id':'engineering-qa-r1',
            'reviewer_id':'/root/independent_fixture','author_ids':actors,'independent_non_author_review':True,
            'source_tree_sha256':'c'*64,'source_entries':50,'allowed_paths':paths,'path_responsibilities':roles,
            'canonical_plan_basis_sha256':basis,'frozen_canonical_plan_paths':canonical,
            'gate_only_paths':list(g.R2_GATE_PATHS),'Root_current_doc_eligible_paths':list(g.R2_ROOT_DOC_PATHS),
            'deferred_atomic_output_paths':list(g.R2_ATOMIC_OUTPUTS),'implemented_parent':parent,
            'predecessor_canonical_plan_review':dict(g.SUCCESSOR_REVIEW),
            'predecessor_Root_QA_authorization':dict(g.SUCCESSOR_AUTHORIZATION),
            'predecessor_implemented_source_review':{'path':'.build/r1-source.json','sha256':'b'*64},
            'ordinary_ref_dispositions':[],'ordinary_disposition_scope':'fixture ordinary scope',
            'product_write_allowed':False,'product_completion':'incomplete','next_stage_allowed':False}
        authority={'schema_version':2,'kind':'root-binding-engineering-r2-gate-authorization/v1','authorizer':'/root',
            'change_id':g.SUCCESSOR_CHANGE,'epoch_id':g.R2_EPOCH,'canonical_plan_review':dict(g.SUCCESSOR_R2_REVIEW),
            'source_before_sha256':'c'*64,'source_entries':50,'allowed_scope_paths':paths,
            'immediately_allowed_paths':list(g.R2_GATE_PATHS),'gate_only_paths':list(g.R2_GATE_PATHS),
            'frozen_canonical_plan_basis':basis,'deferred_atomic_outputs':list(g.R2_ATOMIC_OUTPUTS),
            'Root_current_doc_update':{'paths':list(g.R2_ROOT_DOC_PATHS),'allowed_now':False,'gate_source_review_path':g.R2_GATE_SOURCE_REVIEW},
            'catalogue_write_allowed_now':False,'baseline_or_adoption_allowed_now':False,'no_R1_authority_fallback':True,
            'no_old_failed_packet_or_tool_reuse_for_new_outputs':True,'product_write_allowed':False,
            'runtime_package_modified':False,'product_completion':'incomplete','next_stage_allowed':False}
        predecessor={'canonical_plan_review':dict(g.SUCCESSOR_REVIEW),'Root_QA_authorization':dict(g.SUCCESSOR_AUTHORIZATION),
            'implemented_source_review':review['predecessor_implemented_source_review']}
        metadata={'allowed_paths':paths,'path_responsibilities':roles,'review_evidence':{'authors':actors},
            'successor_handoff':{'schema_version':2,'kind':'binding-successor-amended-handoff-request/v2','active_epoch':g.R2_EPOCH,
                'canonical_plan_review_path':g.SUCCESSOR_R2_REVIEW['path'],'Root_QA_authorization_path':g.SUCCESSOR_R2_AUTHORIZATION['path'],
                'implemented_QA_parent':parent,'predecessor_QA_authority':predecessor,
                'reviewed_scope_paths_compact_sha256':g.compact_sha(old_paths),
                'ordinary_ref_dispositions':[],'ordinary_disposition_scope':'fixture ordinary scope'},
            'plan_amendment':{'kind':'binding-engineering-plan-amendment/v1','schema_version':1,'epoch_id':g.R2_EPOCH,
                'predecessor_epoch_id':'engineering-qa-r1','implemented_parent':parent,'R1_authority':predecessor,
                'scope_paths':paths,'new_catalogue_paths':list(g.R2_CATALOGUE_PATHS),'gate_only_paths':list(g.R2_GATE_PATHS),
                'atomic_output_paths':list(g.R2_ATOMIC_OUTPUTS),'Root_current_doc_eligible_paths':list(g.R2_ROOT_DOC_PATHS),
                'new_adoption_namespace':g.SUCCESSOR_ADOPTION_R2,'packet_kind':'binding-engineering-exact-adoption-packet/v2',
                'packet_review_kind':'independent-binding-engineering-adoption-packet-review/v2'}}
        return copy.deepcopy((review,authority,metadata,r1))

    def test_exact_r2_contract_preserves_r1_hash_namespace(self):
        review, authorization, metadata, r1 = self.contracts()
        g.r2_plan_contract(review, authorization, metadata, r1)
        self.assertEqual(metadata['successor_handoff']['reviewed_scope_paths_compact_sha256'], g.compact_sha(r1['allowed_paths']))
        self.assertNotEqual(g.compact_sha(review['allowed_paths']), g.compact_sha(r1['allowed_paths']))

    def test_old_29_hash_cannot_be_refreshed_to_active32(self):
        review, authorization, metadata, r1 = self.contracts()
        metadata['successor_handoff']['reviewed_scope_paths_compact_sha256'] = g.compact_sha(review['allowed_paths'])
        with self.assertRaises(ValueError):
            g.r2_plan_contract(review, authorization, metadata, r1)

    def test_active_scope_extra_or_omitted_amendment_rejected(self):
        for mutation in ('extra', 'missing_amendment', 'r1_kind'):
            with self.subTest(mutation=mutation):
                review, authorization, metadata, r1 = self.contracts()
                if mutation == 'extra':metadata['allowed_paths'].append('apps/unauthorized.rs')
                elif mutation == 'missing_amendment':metadata.pop('plan_amendment')
                else:metadata['successor_handoff']['kind']='binding-successor-handoff-request/v1'
                with self.assertRaises(ValueError):
                    g.r2_plan_contract(review, authorization, metadata, r1)

    def test_root_authorization_cannot_be_scope32_blanket_permission(self):
        review, authorization, metadata, r1 = self.contracts()
        authorization['immediately_allowed_paths']=review['allowed_paths']
        with self.assertRaises(ValueError):
            g.r2_plan_contract(review, authorization, metadata, r1)

    def test_self_review_dropped_authors_wrong_pins_and_docs_subset_rejected(self):
        for mutation in ('self', 'authors', 'pin', 'docs'):
            with self.subTest(mutation=mutation):
                review, authorization, metadata, r1 = self.contracts()
                if mutation == 'self':review['reviewer_id']=review['author_ids'][0]
                elif mutation == 'authors':metadata['review_evidence']['authors']=[]
                elif mutation == 'pin':authorization['canonical_plan_review']['sha256']='f'*64
                else:authorization['Root_current_doc_update']['paths'].append('docs/outside.md')
                with self.assertRaises(ValueError):
                    g.r2_plan_contract(review, authorization, metadata, r1)

    def test_active_cli_cannot_call_historical_r1_even_without_amendment(self):
        import contextlib,io
        response={'status':'failed','errors':['actual active r2 required'],'product_completion':'incomplete','next_stage_allowed':False}
        with patch.object(g,'validate_successor_current_handoff',return_value=response) as active, \
             patch.object(g,'validate_successor_handoff',side_effect=AssertionError('historical fallback forbidden')), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(g.main(['--successor-check','--change',g.SUCCESSOR_CHANGE,'--json']),1)
        active.assert_called_once()

    def test_verify_actual_impact_routes_to_current_not_legacy_or_requested_name(self):
        import verify
        response={'status':'passed','errors':[],'qa_maintenance_admitted':True,'product_completion':'incomplete','next_stage_allowed':False}
        with patch.object(g,'validate_successor_current_handoff',return_value=response) as active, \
             patch.object(g,'validate_successor_handoff',side_effect=AssertionError('historical fallback forbidden')):
            result=verify.check_current_binding_handoff(self.root,'renamed', ['docs/architecture/implementation-status.json'],set())
        self.assertEqual(result,response)
        active.assert_called_once_with(self.root,'renamed')

    def test_missing_r2_pins_never_fall_back_to_a_valid_historical_api(self):
        with patch.object(g,'validate_successor_handoff',side_effect=AssertionError('no fallback')):
            result=g.validate_successor_current_handoff(self.root,g.SUCCESSOR_CHANGE)
        self.assertEqual(result['status'],'failed')
        self.assertFalse(result['product_write_allowed'])

    def test_gate_only_stage_allows_only_three_paths_without_adoption(self):
        review, authorization, metadata, r1=self.contracts()
        before={p:'a'*64 for p in review['allowed_paths'] if p not in review['deferred_atomic_output_paths']}
        before['apps/game.rs']='e'*64
        current={**before,g.R2_GATE_PATHS[0]:'b'*64}
        stage=g.r2_stage_scope(self.root,review,authorization,before,current)
        self.assertEqual(stage['phase'],'gate_only')
        self.assertFalse(stage['atomic_adoption_verified'])
        self.assertEqual(stage['errors'],[])
        for path in (authorization['Root_current_doc_update']['paths'][0],review['deferred_atomic_output_paths'][0], 'apps/game.rs'):
            with self.subTest(path=path):
                stage=g.r2_stage_scope(self.root,review,authorization,before,{**current,path:'c'*64})
                self.assertTrue(stage['errors'])


class R2DocStageGuards(unittest.TestCase):
    contracts=R2AuthorityAndStageGuards.contracts
    put=SuccessorHandoffGuards.put

    def setUp(self):
        R2AuthorityAndStageGuards.setUp(self)
        self.review,self.authorization,self.metadata,self.r1=self.contracts()
        self.canonical={p:'a'*64 for p in self.review['allowed_paths'] if p not in g.R2_ATOMIC_OUTPUTS[4:]}
        self.canonical['apps/game.rs']='f'*64
        self.gate={**self.canonical,g.R2_GATE_PATHS[0]:'b'*64}

    def jput(self,path,value):return self.put(path,json.dumps(value,sort_keys=True))

    def publish_gate_proof(self, mapping=None):
        mapping=mapping or self.gate
        source_ref=self.jput('.build/gate-map.json',mapping)
        self.proof={'schema_version':1,'kind':'independent-binding-engineering-gate-source-review/v1',
            'change_id':g.SUCCESSOR_CHANGE,'epoch_id':g.R2_EPOCH,'status':'approved','blocking_findings':[],
            'independent_non_author_review':True,'author_ids':self.review['author_ids'],'reviewer_id':'/root/independent_fixture',
            'source_tree_sha256':g.tree_sha(mapping),'source_entries':len(mapping),'source_map':source_ref,
            'product_source_delta':[],'product_acceptance_promoted':False,'runtime_package_modified':False,
            'product_completion':'incomplete','next_stage_allowed':False,
            'canonical_plan_review':dict(g.SUCCESSOR_R2_REVIEW),'Root_QA_authorization':dict(g.SUCCESSOR_R2_AUTHORIZATION),
            'gate_only_paths':list(g.R2_GATE_PATHS),'gate_basis_sha256':{p:mapping[p] for p in g.R2_GATE_PATHS}}
        return self.jput(g.R2_GATE_SOURCE_REVIEW,self.proof)

    def stage(self,current):return g.r2_stage_scope(self.root,self.review,self.authorization,self.canonical,current)

    def test_immutable_valid_gate_proof_releases_docs_despite_original_allowed_now_false(self):
        self.publish_gate_proof()
        current={**self.gate,g.R2_ROOT_DOC_PATHS[0]:'c'*64}
        stage=self.stage(current)
        self.assertEqual(stage['errors'],[],stage)
        self.assertEqual(stage['phase'],'Root_docs_after_gate_review')
        self.assertFalse(stage['atomic_adoption_verified'])
        self.assertFalse(self.authorization['Root_current_doc_update']['allowed_now'])

    def test_root_doc_cannot_be_baked_into_gate_stage_or_gate_changed_after_approval(self):
        bad={**self.gate,g.R2_ROOT_DOC_PATHS[0]:'c'*64}
        self.publish_gate_proof(bad)
        self.assertTrue(self.stage(bad)['errors'])
        self.publish_gate_proof()
        self.assertTrue(self.stage({**bad,g.R2_GATE_PATHS[0]:'d'*64})['errors'])

    def test_doc_subset_and_prior19_do_not_become_blanket_rewrite(self):
        self.publish_gate_proof()
        self.authorization['Root_current_doc_update']['paths']=[g.R2_ROOT_DOC_PATHS[0]]
        self.assertTrue(self.stage({**self.gate,g.R2_ROOT_DOC_PATHS[1]:'c'*64})['errors'])
        self.assertTrue(self.stage({**self.gate,'qa/retained-0.py':'c'*64})['errors'])

    def test_existing_bad_gate_review_never_falls_back_to_gate_only(self):
        self.publish_gate_proof()
        self.proof['reviewer_id']=self.proof['author_ids'][0]
        self.jput(g.R2_GATE_SOURCE_REVIEW,self.proof)
        self.assertTrue(self.stage(self.gate)['errors'])

    def test_approved_summary_without_complete_source_map_or_wrong_parent_rejected(self):
        self.publish_gate_proof()
        self.proof['source_map']={'path':'.build/missing-map.json','sha256':'e'*64}
        self.jput(g.R2_GATE_SOURCE_REVIEW,self.proof)
        self.assertTrue(self.stage({**self.gate,g.R2_ROOT_DOC_PATHS[0]:'c'*64})['errors'])
        self.publish_gate_proof()
        self.proof['Root_QA_authorization']['sha256']='e'*64
        self.jput(g.R2_GATE_SOURCE_REVIEW,self.proof)
        self.assertTrue(self.stage(self.gate)['errors'])


class R2SixOutputEnvelopeGuards(unittest.TestCase):
    """Actual outer six-output binding; QA catalogue semantics tested separately.

    Only the inner catalogue preflight is mocked here. No Rust, product or
    catalogue runtime acceptance is asserted by this temporary guard fixture.
    """
    put=SuccessorHandoffGuards.put
    jput=SuccessorAdoptionGuards.jput
    identity=SuccessorAdoptionGuards.identity

    def setUp(self):
        SuccessorHandoffGuards.setUp(self)
        self.authors=['/root','/root/binding_resumption_gate_implementation','/root/binding_source_audit','/root/storage_integration_fix']
        for p in g.R2_GATE_PATHS+self.doc_paths():self.put(p,'frozen source '+p)
        self.put('apps/game.rs','product unchanged')
        self.old_baseline={'baseline_id':'FP1-20261109-R93','kept':1}
        self.new_baseline={'baseline_id':'FP1-20261109-R94','kept':1}
        self.old_ref=self.jput('.build/old-baseline.json',self.old_baseline)
        patcher=patch.object(g,'R93_BASELINE_SHA256',self.old_ref['sha256']);patcher.start();self.addCleanup(patcher.stop)
        self.jput(g.R2_ATOMIC_OUTPUTS[3],self.old_baseline)
        self.jput(g.R2_CATALOGUE_PATHS[0],{'old':'catalogue'})
        self.put(g.R2_CATALOGUE_PATHS[1],'old markdown')
        self.put(g.R2_CATALOGUE_PATHS[2],'old modules')
        self.put('docs/architecture/engineering-components.csv','unchanged component export')
        self.protected=self.jput('.build/before-catalogue.json',{'old':'catalogue'})
        self.components=self.identity('docs/architecture/engineering-components.csv')
        self.canonical=source_files(self.root)
        self.review={'author_ids':self.authors,'source_map':self.jput('.build/canonical-map.json',self.canonical),
                     'implemented_parent':self.jput('.build/checkpoint.json',{'guard':'implemented parent'})}
        self.root_auth={'Root_current_doc_update':{'paths':list(self.doc_paths())}}
        auth=self.jput(g.SUCCESSOR_R2_AUTHORIZATION['path'],self.root_auth)
        auth.pop('bytes');patcher=patch.object(g,'SUCCESSOR_R2_AUTHORIZATION',auth);patcher.start();self.addCleanup(patcher.stop)
        self.put(g.R2_GATE_PATHS[0],'new reviewed gate code')
        gate_map=source_files(self.root)
        gate_proof=self.source_proof('independent-binding-engineering-gate-source-review/v1',gate_map,'.build/gate-map.json')
        gate_proof.update(epoch_id=g.R2_EPOCH,canonical_plan_review=dict(g.SUCCESSOR_R2_REVIEW),
            Root_QA_authorization=dict(g.SUCCESSOR_R2_AUTHORIZATION),gate_only_paths=list(g.R2_GATE_PATHS),
            gate_basis_sha256={p:gate_map[p] for p in g.R2_GATE_PATHS})
        self.gate_ref=self.jput(g.R2_GATE_SOURCE_REVIEW,gate_proof)
        self.put(self.doc_paths()[0],'later authorized Root document')
        self.before=source_files(self.root)
        final_proof=self.source_proof('independent-binding-engineering-source-review/v1',self.before,'.build/input-map.json')
        self.input_ref=self.jput(g.R2_FINAL_INPUT_REVIEW,final_proof)
        for p,text in zip(g.R2_CATALOGUE_PATHS,('{"new":"QA catalogue"}','new markdown','new modules')):self.put(p,text)
        self.jput(g.R2_ATOMIC_OUTPUTS[3],self.new_baseline)
        self.jput(g.R2_ATOMIC_OUTPUTS[4],self.new_baseline)
        self.jput(g.R2_ATOMIC_OUTPUTS[5],{'guard':'six-output adoption'})
        self.current=source_files(self.root)
        candidate=self.jput('.build/candidate-baseline.json',self.new_baseline)
        delta=self.jput('.build/pin-delta.json',{'algorithm':'json-leaves/v1','before_baseline_ref':self.old_ref,'after_baseline_ref':candidate,
            'rows':[{'path':'/baseline_id','before_exists':True,'before':'FP1-20261109-R93','after_exists':True,'after':'FP1-20261109-R94'}]})
        untouched=self.jput('.build/untouched.json',{'algorithm':'json-leaves/v1','before_baseline_ref':self.old_ref,'after_baseline_ref':candidate,'rows':{'/kept':1}})
        self.base=g.SUCCESSOR_ADOPTION_R2
        tool=self.put(self.base+'/tool/writer.py','reviewed six-output tool')
        tests=self.put(self.base+'/tool/test_writer.py','independently inspected synthetic guard tests')
        out=self.put(self.base+'/tool/test.stdout.log','')
        err=self.put(self.base+'/tool/test.stderr.log','Ran 17 tests in 0.2s\n\nOK\n')
        actual=self.jput(self.base+'/tool/actual-tests.json',{'kind':'actual-command-execution/v1','argv':['python3',tests['path']],
            'cwd':str(self.root),'pid':2345,'exit_code':0,'started_at':'2026-11-10T10:00:00+00:00',
            'finished_at':'2026-11-10T10:00:01+00:00','timed_out':False,'stdout':out,'stderr':err})
        self.tool_proof={'schema_version':2,'kind':'independent-binding-engineering-adoption-tool-review/v2','epoch_id':g.R2_EPOCH,
            'status':'approved','independent_non_author_review':True,'blocking_findings':[],
            'author_ids':[self.authors[2]],'reviewer_id':'/root/independent_fixture','tool':tool,'tests':tests,
            'canonical_plan_review':dict(g.SUCCESSOR_R2_REVIEW),'actual_execution':actual,
            'product_acceptance_promoted':False,'runtime_package_modified':False}
        tool_review=self.jput(self.base+'/tool/review.json',self.tool_proof)
        self.manifest={'schema_version':2,'kind':'binding-engineering-exact-adoption-packet/v2','epoch_id':g.R2_EPOCH,
            'change_id':g.SUCCESSOR_CHANGE,'authors':self.authors,
            'parent_baseline':{'id':'FP1-20261109-R93','sha256':g.R93_BASELINE_SHA256},
            'input_source_map':self.jput('.build/input-map.json',self.before),'after_source_map':self.jput('.build/after-map.json',self.current),
            'input_source_tree_sha256':g.tree_sha(self.before),'input_source_entries':len(self.before),
            'after_source_tree_sha256':g.tree_sha(self.current),'after_source_entries':len(self.current),
            'outputs':[self.identity(p) for p in g.R2_ATOMIC_OUTPUTS],
            'prepare':tool,'generator':tool,'tool_review':tool_review,'pin_delta':delta,'untouched_pins':untouched,
            'amendment_review':dict(g.SUCCESSOR_R2_REVIEW),'Root_authorization':dict(g.SUCCESSOR_R2_AUTHORIZATION),
            'implemented_parent':self.review['implemented_parent'],'actual_gate_source_review':self.gate_ref,'source_review':self.input_ref,
            'catalog_source_audit':self.jput('.build/QA-proof.json',{'guard':'inner independent preflight separately tested'}),
            'protected_catalogue':self.protected,'protected_components_export':self.components}
        self.publish()
        self.inner=patch.object(g,'qa_catalogue_semantics_errors',return_value=[])
        self.catalogue=self.inner.start();self.addCleanup(self.inner.stop)

    @staticmethod
    def doc_paths():return g.R2_ROOT_DOC_PATHS

    def source_proof(self,kind,mapping,path):
        return {'schema_version':1,'kind':kind,'change_id':g.SUCCESSOR_CHANGE,'status':'approved','independent_non_author_review':True,
            'blocking_findings':[],'author_ids':self.authors,'reviewer_id':'/root/independent_fixture',
            'source_tree_sha256':g.tree_sha(mapping),'source_entries':len(mapping),'source_map':self.jput(path,mapping),
            'product_source_delta':[],'product_acceptance_promoted':False,'runtime_package_modified':False,
            'product_completion':'incomplete','next_stage_allowed':False}

    def publish(self):
        manifest=self.jput(self.base+'/packet/manifest.json',self.manifest);manifest.pop('bytes')
        fields=('prepare','generator','tool_review','input_source_tree_sha256','after_source_tree_sha256','pin_delta','untouched_pins',
                'source_review','actual_gate_source_review','catalog_source_audit','amendment_review','Root_authorization',
                'implemented_parent','protected_catalogue','protected_components_export','outputs')
        self.approval={'schema_version':2,'kind':'independent-binding-engineering-adoption-packet-review/v2','epoch_id':g.R2_EPOCH,
            'status':'approved','blocking_findings':[],'independent_non_author_review':True,'author_ids':self.authors,
            'reviewer_id':'/root/independent_fixture','manifest':manifest,**{k:self.manifest[k] for k in fields}}
        approval=self.jput(self.base+'/packet-review.json',self.approval)
        self.receipt={'schema_version':1,'status':'adopted','source_tree_sha256':g.tree_sha(self.current),'source_entries':len(self.current),
            'outputs':self.manifest['outputs'],'product_acceptance_promoted':False,'runtime_package_modified':False,
            'independent_review':{'review_path':approval['path'],'review_sha256':approval['sha256'],
                'manifest_sha256':manifest['sha256'],'reviewer_id':self.approval['reviewer_id']}}
        self.jput(self.base+'/transaction/receipt.json',self.receipt)

    def errors(self):return g.successor_adoption_errors_v2(self.root,self.review,source_files(self.root))

    def test_exact_six_retains_both_gate_and_final_input_and_calls_QA_consumer(self):
        self.assertEqual(self.errors(),[])
        self.catalogue.assert_called_once()
        self.assertNotEqual(self.manifest['actual_gate_source_review']['path'],self.manifest['source_review']['path'])

    def test_missing_extra_or_old_three_outputs_rejected(self):
        for outputs in (self.manifest['outputs'][:3],self.manifest['outputs']+[self.manifest['outputs'][0]],self.manifest['outputs'][1:]):
            with self.subTest(length=len(outputs)):
                original=self.manifest['outputs'];self.manifest['outputs']=outputs;self.publish()
                self.assertTrue(self.errors());self.manifest['outputs']=original
        self.publish()

    def test_r1_packet_or_old_tool_proof_cannot_approve_six(self):
        self.manifest['kind']='binding-engineering-exact-adoption-packet/v1';self.publish();self.assertTrue(self.errors())
        self.manifest['kind']='binding-engineering-exact-adoption-packet/v2'
        self.tool_proof['kind']='independent-binding-engineering-adoption-tool-review/v1'
        self.manifest['tool_review']=self.jput(self.base+'/tool/review.json',self.tool_proof);self.publish();self.assertTrue(self.errors())

    def test_gate_proof_cannot_be_relabelled_as_post_doc_input_proof(self):
        self.manifest['source_review']=self.gate_ref;self.publish();self.assertTrue(self.errors())

    def test_rolledback_missing_or_old_receipt_and_catalogue_errors_rejected(self):
        (self.root/(self.base+'/transaction/receipt.json')).unlink();self.assertTrue(self.errors())
        self.publish();self.receipt['status']='rolled_back';self.jput(self.base+'/transaction/receipt.json',self.receipt);self.assertTrue(self.errors())
        self.publish();self.catalogue.return_value=['actual QA/body source evidence failed'];self.assertTrue(self.errors())

    def test_mutated_after_bytes_pin_or_unreviewed_source_rejected(self):
        self.put(g.R2_CATALOGUE_PATHS[1],'late changed output');self.assertTrue(self.errors())

    def test_printed_OK_is_not_actual_execution_of_frozen_tool_tests(self):
        actual=json.loads((self.root/self.tool_proof['actual_execution']['path']).read_text())
        actual['argv']=['echo','Ran 17 tests in 0.2s OK']
        self.tool_proof['actual_execution']=self.jput(self.base+'/tool/actual-tests.json',actual)
        self.manifest['tool_review']=self.jput(self.base+'/tool/review.json',self.tool_proof);self.publish()
        self.assertTrue(self.errors())


class R2CatalogueSemanticGuards(unittest.TestCase):
    def setUp(self):
        self.before={'schema_version':1,'product_sources_sha256':{'apps/a.rs':'a'*64},'runtime_verified_this_audit':False,
            'audit_inputs':[],'incremental_source_review':{},'components':[{'id':'C01','owner':'same','status':'blocked'}],
            'capabilities':[{'id':'FP-01','target':'unchanged'}],
            'modules':[{'id':'M13','next_step':'按当前整页计划与累计901项义务（原809项及92项增量均保留）完成剩余状态，逐页真实组件与同包捕获。','status':'blocked'},
                       {'id':'M15','current_behavior':'old QA behavior','gap':'old gap','source_evidence':[{'old':'evidence'}],
                        'target_behavior':'same target','owner':'same','status':'source_only','historical_evidence':['same']} ]}
        self.candidate=copy.deepcopy(self.before)
        self.candidate['modules'][0]['next_step']='按当前整页计划与累计944项义务（原901项逐ID保留，另增43项真实入口状态义务）完成剩余状态，逐页真实组件与同包捕获。'
        self.candidate['modules'][1].update(current_behavior='active r2 and atomic six QA semantics',gap='944 obligations remain blocked',
                                          source_evidence=[{'current':'actual active source'}])

    def check(self):return g.r2_catalogue_protected_fields(self.before,self.candidate,coverage_state_count=944)

    def test_exact_count_only_M13_and_QA_only_M15_changes_are_permitted(self):self.assertIsNone(self.check())

    def test_actual_M13_counterexample_cannot_hide_product_behavior_in_next_step(self):
        self.candidate['modules'][0]['next_step']='关闭全部原生、游戏和翻译准出，发布产品。'
        with self.assertRaises(ValueError):self.check()

    def test_count_is_derived_from_real_contract_not_author_guessed_text(self):
        self.candidate['modules'][0]['next_step']=self.candidate['modules'][0]['next_step'].replace('944','999').replace('43项','98项')
        with self.assertRaises(ValueError):self.check()

    def test_protected_product_status_owner_target_history_and_component_rejected(self):
        for group,index,key,value in [('modules',0,'status','passed'),('modules',1,'owner','someone'),
                                      ('modules',1,'target_behavior','new product'),('modules',1,'historical_evidence',[]),
                                      ('components',0,'status','passed'),('capabilities',0,'target','new')]:
            with self.subTest(group=group,key=key):
                original=self.candidate[group][index][key];self.candidate[group][index][key]=value
                with self.assertRaises(ValueError):self.check()
                self.candidate[group][index][key]=original

    def test_only_hash_refresh_cannot_replace_current_QA_semantic_review(self):
        self.candidate['modules'][1]['current_behavior']=self.before['modules'][1]['current_behavior']
        with self.assertRaises(ValueError):self.check()


class R2CodeSourceSemanticsGuards(unittest.TestCase):
    put=SuccessorHandoffGuards.put

    def setUp(self):
        import ast
        SuccessorHandoffGuards.setUp(self)
        by_file={}
        for role,(path,symbol) in g.R2_CODE_ROLES.items():by_file.setdefault(path,[]).append(symbol)
        for path,symbols in by_file.items():self.put(path,'\n\n'.join('def '+symbol+'():\n    return None' for symbol in symbols)+'\n')
        self.input=source_files(self.root);self.anchors=[];self.evidence=[]
        for role,(path,symbol) in g.R2_CODE_ROLES.items():
            text=(self.root/path).read_text();node=next(n for n in ast.parse(text).body if isinstance(n,ast.FunctionDef) and n.name==symbol)
            anchor={'role':role,'path':path,'symbol':symbol,'line':node.lineno,'end_line':node.end_lineno,
                    'sha256':self.input[path],'function_sha256':g.sha(ast.get_source_segment(text,node).encode()),
                    'quote':text.splitlines()[node.lineno-1].strip(),'meaning':'Synthetic source binding only; not product execution'}
            self.anchors.append(anchor);self.evidence.append({k:anchor[k] for k in ('path','symbol','line','quote','sha256')})
        self.semantics={'schema_version':1,'kind':'binding-engineering-code-source-semantics/v1',
                        'input_source_tree_sha256':g.tree_sha(self.input),'anchors':self.anchors}

    def errors(self):return g.code_source_semantics_errors(self.root,self.semantics,self.input,self.evidence)

    def test_all_actual_roles_bind_current_full_functions_not_old_ten_limit(self):
        self.assertGreater(len(self.anchors),10)
        self.assertEqual(self.errors(),[])

    def test_old_v1_function_cannot_stand_in_for_active_or_six_output_role(self):
        for role in ('current_handoff','six_output_transaction'):
            with self.subTest(role=role):
                row=next(a for a in self.anchors if a['role']==role);original=row['symbol'];row['symbol']='validate_successor_handoff'
                self.assertTrue(self.errors());row['symbol']=original

    def test_missing_actual_role_duplicate_or_stale_source_range_hash_rejected(self):
        saved=self.anchors.pop();self.assertTrue(self.errors());self.anchors.append(saved)
        self.anchors.append(saved);self.assertTrue(self.errors());self.anchors.pop()
        for key,value in [('line',999),('end_line',999),('sha256','b'*64),('function_sha256','c'*64)]:
            with self.subTest(key=key):
                previous=self.anchors[0][key];self.anchors[0][key]=value;self.assertTrue(self.errors());self.anchors[0][key]=previous

    def test_whole_function_proof_and_M15_actual_source_evidence_must_both_exist(self):
        self.evidence.pop()
        self.assertTrue(self.errors())

    def test_extra_actual_QA_function_is_allowed_without_upper_ceiling(self):
        import ast
        path='scripts/quality/additional.py';symbol='extra_actual_semantic_boundary'
        self.put(path,'def '+symbol+'():\n    return 1\n');self.input=source_files(self.root)
        text=(self.root/path).read_text();node=ast.parse(text).body[0]
        row={'role':'additional_current_boundary','path':path,'symbol':symbol,'line':node.lineno,'end_line':node.end_lineno,
             'sha256':self.input[path],'function_sha256':g.sha(ast.get_source_segment(text,node).encode()),
             'quote':text.splitlines()[node.lineno-1].strip(),'meaning':'Additional real current boundary'}
        self.anchors.append(row);self.evidence.append({k:row[k] for k in ('path','symbol','line','quote','sha256')})
        self.semantics['input_source_tree_sha256']=g.tree_sha(self.input)
        self.assertEqual(self.errors(),[])


class R2DeletedGateRegression(unittest.TestCase):
    def test_deleted_gate_and_null_basis_do_not_unlock_docs_or_gate_phase(self):
        fixture=R2DocStageGuards(methodName='test_immutable_valid_gate_proof_releases_docs_despite_original_allowed_now_false')
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        missing=dict(fixture.gate);missing.pop('scripts/quality/verify.py')
        self.assertTrue(fixture.stage(missing)['errors'])
        # Construct the exact former None==None counterexample without using the
        # fixture producer's expectation, including its malformed null basis.
        source_ref=fixture.jput('.build/deleted-map.json',missing)
        fixture.publish_gate_proof()
        fixture.proof['source_map']=source_ref
        fixture.proof['source_tree_sha256']=g.tree_sha(missing)
        fixture.proof['source_entries']=len(missing)
        fixture.proof['gate_basis_sha256']['scripts/quality/verify.py']=None
        fixture.jput(g.R2_GATE_SOURCE_REVIEW,fixture.proof)
        changed={**missing,g.R2_ROOT_DOC_PATHS[0]:'c'*64}
        self.assertTrue(fixture.stage(changed)['errors'])


class R2TypedToolRegression(unittest.TestCase):
    def test_false_exit_wrong_kind_or_skipped_guard_receipt_never_closes_tool_review(self):
        fixture=R2SixOutputEnvelopeGuards(methodName='test_exact_six_retains_both_gate_and_final_input_and_calls_QA_consumer')
        fixture.setUp();self.addCleanup(fixture.doCleanups)
        actual_path=fixture.root/fixture.tool_proof['actual_execution']['path']
        original=json.loads(actual_path.read_text())
        for key,value in [('exit_code',False),('kind','not-an-execution')]:
            with self.subTest(key=key):
                actual={**original,key:value}
                fixture.tool_proof['actual_execution']=fixture.jput(fixture.base+'/tool/actual-tests.json',actual)
                fixture.manifest['tool_review']=fixture.jput(fixture.base+'/tool/review.json',fixture.tool_proof)
                fixture.publish();self.assertTrue(fixture.errors())
        fixture.tool_proof['actual_execution']=fixture.jput(fixture.base+'/tool/actual-tests.json',original)
        fixture.manifest['tool_review']=fixture.jput(fixture.base+'/tool/review.json',fixture.tool_proof);fixture.publish()
        fixture.receipt['schema_version']=True;fixture.jput(fixture.base+'/transaction/receipt.json',fixture.receipt)
        self.assertTrue(fixture.errors())

    def test_actual_skipped_tool_guards_are_not_closed_by_OK_or_positive_count(self):
        fixture=R2SixOutputEnvelopeGuards(methodName='test_exact_six_retains_both_gate_and_final_input_and_calls_QA_consumer')
        fixture.setUp();self.addCleanup(fixture.doCleanups)
        actual=json.loads((fixture.root/fixture.tool_proof['actual_execution']['path']).read_text())
        actual['stderr']=fixture.put(fixture.base+'/tool/test.stderr.log','Ran 17 tests in 0.2s\n\nOK (skipped=1)\n')
        fixture.tool_proof['actual_execution']=fixture.jput(fixture.base+'/tool/actual-tests.json',actual)
        fixture.manifest['tool_review']=fixture.jput(fixture.base+'/tool/review.json',fixture.tool_proof)
        fixture.publish();self.assertTrue(fixture.errors())
