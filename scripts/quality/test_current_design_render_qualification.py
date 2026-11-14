"""Finite bridge guard fixtures; not current-product acceptance."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import current_design_render_qualification as q
from render_evidence import unavailable
import test_current_reference_catalogue as reference_test_fixtures


class CurrentRenderQualificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve(); self.out = self.root / '.build/capture'
        self.directory = self.out / 'render/general'; self.directory.mkdir(parents=True)
        self.report = self.out / 'verification.json'
        self.report.write_text(json.dumps({'source_tree_sha256': 'a' * 64}))
        self.row = {'state_id': 'all-page-settings-general-normal', 'capture_scope': 'target',
                    'full_png_scope': 'browser_viewport', 'render_status': 'missing_render_evidence',
                    'same_data_eligibility': {'status': 'incomparable'}}
        self.value = unavailable('effective region/form mapping not yet implemented for this state')
        self.value.update(state_id=self.row['state_id'], diagnostic_context=False,
                          artifact_origin_source_sha256='a' * 64, tooling_source_sha256='a' * 64,
                          capture_scope='target', full_png_scope='browser_viewport', whole_client_eligible=False)

    def inspect(self):
        path = self.directory / 'qualification.json'
        path.write_text(json.dumps(self.value))
        self.row['render_qualification'] = q.file_ref(self.root, path)
        # Component artifacts use capture-root-relative paths; the bridge index
        # separately uses repository-relative paths and the actual JS converts.
        self.row['render_qualification']['path'] = path.relative_to(self.out).as_posix()
        with patch.object(q, 'source_identity', return_value={'sha256': 'a' * 64}):
            return q.inspect_qualification(self.root, self.report, self.row)

    def test_missing_mapping_remains_an_error_not_an_eligible_page(self):
        self.assertTrue(self.inspect())

    def test_forced_eligible_with_missing_regions_rejected(self):
        self.row['same_data_eligibility']['status'] = 'eligible'
        with self.assertRaisesRegex(ValueError, 'forced eligible'):
            self.inspect()

    def test_component_or_viewport_cannot_be_promoted_to_whole_client(self):
        for key, value in [('capture_scope', 'whole_client'), ('full_png_scope', 'whole_scrollable_page')]:
            with self.subTest(field=key):
                old = self.row[key]; self.row[key] = value
                with self.assertRaisesRegex(ValueError, 'promoted'):
                    self.inspect()
                self.row[key] = old

    def test_native_reference_or_completion_promotion_rejected(self):
        for key in ('native_verified', 'reference_approved', 'product_alignment_approved', 'completion_allowed', 'whole_client_eligible'):
            with self.subTest(field=key):
                self.value[key] = True
                with self.assertRaisesRegex(ValueError, 'scope/approval'):
                    self.inspect()
                self.value[key] = False

    def test_forged_display_status_cannot_hide_the_raw_missing_verdict(self):
        self.row['render_status'] = 'comparable'
        with self.assertRaisesRegex(ValueError, 'displayed render status'):
            self.inspect()

    def test_rehashed_false_comparable_mapping_is_rejected(self):
        self.value['status'] = 'comparable'; self.row['render_status'] = 'comparable'
        with self.assertRaisesRegex(ValueError, 'falsely qualified'):
            self.inspect()

    def test_diagnostic_old_source_is_not_current_formal_acceptance(self):
        self.value['diagnostic_context'] = True
        with self.assertRaisesRegex(ValueError, 'identity/scope'):
            self.inspect()

    def test_old_source_hash_and_unbound_recompute_rejected(self):
        self.value['artifact_origin_source_sha256'] = 'b' * 64
        with self.assertRaisesRegex(ValueError, 'identity/scope'):
            self.inspect()
        self.value['artifact_origin_source_sha256'] = 'a' * 64
        self.value['independently_recomputed'] = True
        with self.assertRaisesRegex(ValueError, 'falsely qualified'):
            self.inspect()

    def test_only_hashed_inert_render_assets_enter_the_offline_browser(self):
        files = {}
        for name in ('a.css', 'run.js', 'index.html', 'info.json', 'image.svg'):
            path = self.root / name; path.write_text('fixture bytes')
            files[name] = path
        result = q.assets(self.root, 'https://retained.invalid/page', files)
        self.assertEqual(['text/css', 'image/svg+xml'], [row['media_type'] for row in result])
        self.assertEqual(['a.css', 'image.svg'], [row['file']['path'] for row in result])

    def test_unknown_duplicate_or_traversal_states_rejected_before_output_creation(self):
        for values in (['../outside'], ['all-page-settings-general-normal'] * 4, []):
            with self.subTest(values=values):
                report = {'kind': 'current-design-real-react/v1', 'native_verified': False,
                          'results': [{'state_id': value} for value in values]}
                self.report.write_text(json.dumps(report))
                output = self.root / '.build/qualification-output'
                with self.assertRaisesRegex(ValueError, 'finite current state'):
                    q.qualify_report(self.root, self.report, output, diagnostic=True)
                self.assertFalse(output.exists())

    def test_python_bridge_rejects_lexical_symlink_ancestors_without_foreign_writes(self):
        real = self.root / '.build/real'; real.mkdir()
        marker = real / 'marker'; marker.write_text('foreign original')
        alias = self.root / '.build/alias'; alias.symlink_to(real, target_is_directory=True)
        self.report.write_text(json.dumps({'kind': 'current-design-real-react/v1', 'native_verified': False,
                                           'results': [{'state_id': name} for name in sorted(q.STATE_IDS)],
                                           'source_tree_sha256': 'a' * 64}))
        with self.assertRaisesRegex(ValueError, 'symlink|符链|symbolic|alias'):
            q.qualify_report(self.root, self.report, alias / 'child', diagnostic=True)
        self.assertEqual('foreign original', marker.read_text())
        self.assertFalse((real / 'child').exists())

    def test_retained_report_symlink_is_not_resolved_into_trusted_evidence(self):
        alias = self.out / 'alias-report.json'; alias.symlink_to(self.report)
        output = self.root / '.build/new-qualification'
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            q.qualify_report(self.root, alias, output, diagnostic=True)
        self.assertFalse(output.exists())


class WholeClientRenderQualificationTests(unittest.TestCase):
    def setUp(self):
        self.fixture = reference_test_fixtures.CurrentClientReferenceTests(); self.fixture.setUp(); self.addCleanup(self.fixture.doCleanups)
        self.reference = copy.deepcopy(self.fixture.facts)
        self.surface = self.fixture.surface

    def test_effective_equality_does_not_approve_pixels_reference_or_product(self):
        value = q.compare_client_render_facts(self.reference, copy.deepcopy(self.reference), self.surface)
        self.assertEqual('comparable', value['status'])
        for key in ('native_verified', 'product_alignment_approved', 'reference_approved', 'completion_allowed', 'whole_client_eligible'):
            self.assertFalse(value[key])

    def test_real_font_and_lexical_differences_are_incomparable_even_with_same_preferences(self):
        for mutate, expected in [(lambda f: f['text_runs'][0].update(font_size='23px'), 'text_runs'),
                                 (lambda f: f['lexical']['rt'].append('ふりがな'), 'lexical'),
                                 (lambda f: f['fonts']['checks'][0].update(font='700 23px other-font'), 'fonts')]:
            candidate = copy.deepcopy(self.reference); mutate(candidate)
            value = q.compare_client_render_facts(self.reference, candidate, self.surface)
            self.assertEqual('incomparable', value['status']); self.assertIn(expected, value['differences'])

    def test_missing_effective_facts_or_target_scope_are_rejected_not_eligible(self):
        for key, value in [('fonts', {}), ('forms', None), ('capture_scope', 'target'), ('lexical', None)]:
            with self.subTest(key=key):
                candidate = copy.deepcopy(self.reference); candidate[key] = value
                with self.assertRaises((ValueError, TypeError)):
                    q.compare_client_render_facts(self.reference, candidate, self.surface)

    def test_whole_client_reference_audit_retains_source_only_boundary(self):
        h = self.fixture
        value = q.inspect_client_reference(h.root, '.build/reference-fixture', h.entry['generation'], 'fixture-normal', context=h.context)
        self.assertEqual('measured', value['status']); self.assertFalse(value['independently_recomputed'])
        self.assertFalse(value['whole_client_eligible'])


class UiFontRenderComparisonTests(unittest.TestCase):
    """Execution origins are not font geometry; actual bytes/context remain strict."""
    def setUp(self):
        h = reference_test_fixtures.CurrentClientReferenceTests(); h.setUp(); self.addCleanup(h.doCleanups)
        self.surface = h.surface; self.reference = copy.deepcopy(h.facts)
        run = self.reference['text_runs'][0]
        run.update(selector='#main-window > p', font_style='normal', font_stretch='100%', font_optical_sizing='auto', font_variation_settings='normal')
        self.reference['fonts']['observations'] = [{**{k:run[k] for k in ('selector','text','font_family','font_size','font_weight','font_style','font_stretch','font_optical_sizing','font_variation_settings')}, 'platform_font': copy.deepcopy(self.reference['fonts']['platform'][0])}]
        self.reference['fonts']['files'][0].update(resolution='coretext_ui_instance_exact', ui_probe={'path':'one-proof.json','sha256':'a'*64,'bytes':1})

    def test_separate_fresh_native_proof_origin_is_not_render_difference(self):
        other = copy.deepcopy(self.reference); other['fonts']['files'][0]['ui_probe'] = {'path':'two-proof.json','sha256':'b'*64,'bytes':2}
        result = q.compare_client_render_facts(self.reference, other, self.surface)
        self.assertEqual('comparable',result['status']); self.assertFalse(result['whole_client_eligible'])

    def test_actual_font_bytes_and_node_context_stay_strict(self):
        for mutation in (lambda f:f['fonts']['files'][0]['file'].update(sha256='f'*64), lambda f:f['fonts']['observations'][0].update(font_weight='700')):
            other = copy.deepcopy(self.reference); mutation(other)
            self.assertIn('fonts',q.compare_client_render_facts(self.reference,other,self.surface)['differences'])


class ApprovedC1QualificationTests(unittest.TestCase):
    def setUp(self):
        h=reference_test_fixtures.CurrentClientReferenceTests();h.setUp();self.addCleanup(h.doCleanups)
        self.h=h;self.surface=copy.deepcopy(h.surface)
        self.surface.update(projection='approved_client_container_c1/v1',layout_contract_id='client-layout-c1-20261105')

    def test_c1_facts_have_explicit_v3_scope_without_native_or_approval(self):
        result=q.compare_client_render_facts(self.h.facts,copy.deepcopy(self.h.facts),self.surface)
        self.assertEqual(3,result['schema_version'])
        self.assertEqual('whole-client-effective-render-comparison/v3',result['kind'])
        self.assertEqual('comparable',result['status'])
        self.assertFalse(result['whole_client_eligible']);self.assertFalse(result['native_verified'])
        self.assertFalse(result['completion_allowed'])

    def test_c1_audit_keeps_independent_layout_replay_and_approval_separate(self):
        capture={'surface':self.surface,'facts':self.h.facts,'approved_layout':{'contract':{'id':'client-layout-c1-20261105'}},
            'independent_layout_replay':{'status':'recomputed','reference_approved':False}}
        with patch.object(q,'validate_client_capture',return_value=capture):
            result=q.inspect_client_reference(self.h.root,'ignored',{},'fixture-normal')
        self.assertEqual(3,result['schema_version'])
        self.assertEqual(capture['independent_layout_replay'],result['independent_layout_replay'])
        self.assertFalse(result['whole_client_eligible']);self.assertFalse(result['product_alignment_approved'])
