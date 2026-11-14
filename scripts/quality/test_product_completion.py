"""Guard fixtures verify aggregation, never assert that the product is accepted."""
import copy
import unittest
from unittest.mock import patch
import product_completion as c


class ProductCompletionTests(unittest.TestCase):
    def binding_group_fixture(self):
        from binding_journey import EXACT_EXECUTION_OBLIGATION_IDS
        # Pure aggregator input; these are not raw product execution artifacts.
        return {'schema_version': 1,
                'kind': 'binding-execution-obligation-assessment/v1',
                'registry_sha256': 'c' * 64, 'product_acceptance_promoted': False,
                'groups': [{'id': identity, 'status': 'passed',
                            'checks': [{'id': 'controlled-guard-assertion'}],
                            'errors': [], 'artifacts': [{'guard_fixture_only': True}]}
                           for identity in EXACT_EXECUTION_OBLIGATION_IDS]}

    def merge_claimed_binding_pass(self, assessment):
        product = {'status': 'passed', 'errors': [], 'release_gaps': []}
        binding = {'status': 'passed', 'errors': [], 'release_gaps': [],
                   'execution_obligations': assessment}
        return c.merge_binding_readiness(product, binding)

    def test_binding_green_summary_requires_all_fixed_execution_groups(self):
        assessment = self.binding_group_fixture()
        result = self.merge_claimed_binding_pass(assessment)
        self.assertEqual(result['release_gaps'], [])
        self.assertEqual(result['status'], 'passed')
        # This asserts aggregation only; actual assess_completion reexecutes the
        # source-owned raw inspectors before supplying this structure.

    def test_binding_green_summary_cannot_omit_duplicate_or_rename_a_group(self):
        for mutation in ('missing', 'duplicate', 'extra', 'renamed'):
            assessment = self.binding_group_fixture()
            if mutation == 'missing':
                assessment['groups'].pop()
            elif mutation == 'duplicate':
                assessment['groups'][1]['id'] = assessment['groups'][0]['id']
            elif mutation == 'extra':
                assessment['groups'].append(copy.deepcopy(assessment['groups'][0]))
            else:
                assessment['groups'][0]['id'] = 'caller-appointed-group'
            with self.subTest(mutation=mutation):
                result = self.merge_claimed_binding_pass(assessment)
                self.assertEqual(result['status'], 'blocked')
                f = self.facts(); f['product_assessment'] = result
                self.assertIncomplete(c.derive_verdict(**f))

    def test_binding_green_summary_cannot_hide_unexecuted_or_empty_proof(self):
        for key, value in [('status', 'not_run'), ('status', 'blocked'),
                           ('status', 'failed'), ('status', True),
                           ('checks', []), ('artifacts', []),
                           ('errors', ['actual inspector rejected raw'])]:
            assessment = self.binding_group_fixture()
            assessment['groups'][0][key] = value
            with self.subTest(key=key, value=value):
                result = self.merge_claimed_binding_pass(assessment)
                self.assertEqual(result['status'], 'blocked')
                self.assertTrue(result['release_gaps'])

    def test_binding_green_summary_requires_typed_current_registry_assessment(self):
        fixtures = [None, {}, False]
        for key, value in [('schema_version', True), ('registry_sha256', None),
                           ('kind', 'legacy-r2-proof'), ('product_acceptance_promoted', True)]:
            assessment = self.binding_group_fixture()
            assessment[key] = value
            fixtures.append(assessment)
        for assessment in fixtures:
            with self.subTest(assessment=assessment):
                self.assertEqual(self.merge_claimed_binding_pass(assessment)['status'], 'blocked')

    def test_binding_layer_gaps_are_additive_and_prevent_product_promotion(self):
        product={'status':'passed','errors':[],'release_gaps':[{'id':'original-Native-gap'}]}
        binding={'status':'blocked','errors':[],'release_gaps':[{'case_id':'BJ-16','layer':'native','status':'not_run'}]}
        merged=c.merge_binding_readiness(product,binding)
        self.assertEqual(merged['status'],'blocked')
        self.assertEqual(len(merged['release_gaps']),2)
        self.assertEqual(product['release_gaps'],[{'id':'original-Native-gap'}])
        f=self.facts();f['product_assessment']=merged
        self.assertIncomplete(c.derive_verdict(**f))
    def facts(self):
        return dict(source_tree_sha256='a' * 64, execution_source_sha256='a' * 64,
                    required_checks={'compile', 'ui'},
                    steps=[{'id': 'compile', 'status': 'passed'}, {'id': 'ui', 'status': 'passed'}],
                    design_assessment={'status': 'passed', 'errors': [], 'release_gaps': []},
                    product_assessment={'status': 'passed', 'errors': [], 'release_gaps': []},
                    capture_gaps=[])

    def test_complete_internal_fixture_requires_every_condition(self):
        result = c.derive_verdict(**self.facts())
        self.assertEqual('complete', result['product_completion'])
        self.assertIs(result['next_stage_allowed'], True)
        self.assertEqual([], c.advancement_errors(result))
        self.assertIs(result['guard_tests_are_product_acceptance'], False)

    def test_failed_blocked_not_run_missing_case_prevent_advancement(self):
        for status in ('failed', 'blocked', 'not_run', None):
            with self.subTest(status=status):
                f = self.facts()
                if status is None: f['steps'].pop()
                else: f['steps'][1]['status'] = status
                self.assertIncomplete(c.derive_verdict(**f))

    def assertIncomplete(self, value):
        self.assertEqual('incomplete', value['product_completion'])
        self.assertEqual('incomplete', value['product_gate_status'])
        self.assertIs(value['next_stage_allowed'], False)
        self.assertTrue(value['reasons'])
        self.assertTrue(c.advancement_errors(value))

    def test_correct_blocked_assessment_contract_step_is_not_product_success(self):
        f = self.facts(); f['steps'].append({'id': 'readiness-contract', 'status': 'passed'})
        f['design_assessment'] = {'status': 'blocked', 'errors': [], 'release_gaps': ['Native not_run']}
        self.assertIncomplete(c.derive_verdict(**f))

    def test_all_green_subtotal_cannot_erase_reference_runtime_or_case_gaps(self):
        for label, gap in [('design_assessment', 'missing reference'), ('product_assessment', {'status': 'not_run', 'reason': 'Native'}), ('product_assessment', {'status': 'blocked', 'reason': 'old process/executable'}), ('design_assessment', 'render_state_incomparable')]:
            with self.subTest(gap=gap):
                f = self.facts(); f[label]['release_gaps'] = [gap]
                self.assertIncomplete(c.derive_verdict(**f))

    def test_article_or_viewport_cannot_complete_whole_client(self):
        f = self.facts(); f['capture_gaps'] = [{'state_id': 'page', 'available_scopes': ['target'], 'required_scope': 'whole_client'}]
        self.assertIncomplete(c.derive_verdict(**f))

    def test_wrong_or_missing_source_epoch_prevents_advancement(self):
        for value in (None, 'b' * 64):
            f = self.facts(); f['execution_source_sha256'] = value
            self.assertIncomplete(c.derive_verdict(**f))

    def test_unconditional_failure_cannot_be_hidden_by_requested_subset(self):
        f = self.facts(); f['steps'].append({'id': 'identity', 'status': 'failed'})
        self.assertIncomplete(c.derive_verdict(**f))

    def test_source_review_browser_and_guard_success_cannot_fill_native_case(self):
        f = self.facts(); f['required_checks'].add('native')
        f['steps'] += [{'id': x, 'status': 'passed'} for x in ('source-review', 'browser', 'guard-tests')]
        self.assertIncomplete(c.derive_verdict(**f))

    def test_duplicate_unknown_or_coerced_status_is_rejected(self):
        for rows in ([{'id': 'ui', 'status': True}], [{'id': 'ui', 'status': 'skipped'}], [{'id': 'ui', 'status': 'passed'}] * 2):
            f = self.facts(); f['steps'] = rows
            with self.assertRaises(ValueError): c.derive_verdict(**f)

    def test_empty_required_scope_is_not_complete(self):
        f = self.facts(); f['required_checks'] = set()
        self.assertIncomplete(c.derive_verdict(**f))

    def test_source_identity_and_required_names_are_typed(self):
        for key, value in [('source_tree_sha256', True), ('execution_source_sha256', 1), ('required_checks', {True})]:
            f = self.facts(); f[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                c.derive_verdict(**f)

    def test_forged_completion_and_next_stage_are_rejected_by_recomputation(self):
        f = self.facts(); f['product_assessment']['release_gaps'] = [{'reason': 'missing actual Native'}]
        expected = c.derive_verdict(**f); forged = copy.deepcopy(expected)
        forged.update(product_completion='complete', product_gate_status='passed', next_stage_allowed=True, reasons=[], capture_scope_gaps=[])
        with patch.object(c, 'assess_completion', return_value=expected):
            self.assertTrue(c.validate_report(None, None, [], forged))
            self.assertEqual([], c.validate_report(None, None, [], expected))

    def test_missing_errors_and_gaps_cannot_be_coerced_to_ready(self):
        f = self.facts(); f['design_assessment'] = {'status': 'passed', 'errors': False, 'release_gaps': []}
        with self.assertRaises(ValueError): c.derive_verdict(**f)

    def test_declared_scope_includes_every_requirement_without_self_cycle(self):
        metadata = {'requirements': [{'checks': ['a', 'product-completion-assessment']}, {'checks': ['b']}], 'product_visual_checks': ['visual'], 'product_runtime_checks': ['native']}
        self.assertEqual({'a', 'b', 'visual', 'native'}, c.declared_checks(metadata))

    def test_explicit_tooling_completion_requirement_survives_final_epoch_change(self):
        metadata = {'kind': 'tooling', 'requirements': [{'checks': ['product-acceptance-completion']}]}
        required = c.requires_product_completion(metadata, 'development', False)
        self.assertIs(required, True)
        initial = c.derive_verdict(**self.facts())
        self.assertEqual([], c.advancement_errors(initial))
        f = self.facts(); f['source_tree_sha256'] = 'b' * 64
        final = c.derive_verdict(**f)
        self.assertIncomplete(final)
        self.assertTrue(c.advancement_errors(final) if required else [])

    def test_completion_requirement_is_shared_across_scope_modes(self):
        self.assertFalse(c.requires_product_completion({'kind': 'tooling'}, 'development', False))
        self.assertTrue(c.requires_product_completion({'kind': 'product'}, 'development', False))
        self.assertTrue(c.requires_product_completion({'kind': 'tooling'}, 'release', False))
        self.assertTrue(c.requires_product_completion({'kind': 'tooling'}, 'development', True))

    def test_saved_complete_and_incomplete_receipts_recompute_without_self_cycle(self):
        for incomplete in (False, True):
            with self.subTest(incomplete=incomplete):
                f = self.facts()
                if incomplete: f['product_assessment']['release_gaps'] = ['Native not_run']
                before = c.derive_verdict(**f)
                status = 'failed' if incomplete else 'passed'
                f['steps'] += [{'id': 'product-completion-assessment', 'status': 'passed'},
                               {'id': 'product-acceptance-completion', 'status': status},
                               {'id': 'product-completion-finalization', 'status': status}]
                self.assertEqual(before, c.derive_verdict(**f))

    def test_failed_assessment_or_false_final_step_cannot_be_ignored(self):
        for name, status in [('product-completion-assessment', 'failed'),
                             ('product-completion-finalization', 'failed')]:
            f = self.facts(); f['steps'].append({'id': name, 'status': status})
            self.assertIncomplete(c.derive_verdict(**f))

    def test_failed_acceptance_step_cannot_be_hidden_by_absent_or_passed_final_step(self):
        for final_status in (None, 'passed'):
            with self.subTest(final_status=final_status):
                f = self.facts()
                f['steps'].append({'id': 'product-acceptance-completion', 'status': 'failed'})
                if final_status:
                    f['steps'].append({'id': 'product-completion-finalization', 'status': final_status})
                self.assertIncomplete(c.derive_verdict(**f))

    def test_passing_derived_step_cannot_contradict_incomplete_base_facts(self):
        for name in sorted(c.DERIVED_GATE_STEPS):
            with self.subTest(name=name):
                f = self.facts(); f['product_assessment']['release_gaps'] = ['Native not_run']
                f['steps'].append({'id': name, 'status': 'passed'})
                result = c.derive_verdict(**f)
                self.assertIncomplete(result)
                self.assertTrue(any('contradicts_derived_facts' in row['code']
                                    for row in result['reasons']))
