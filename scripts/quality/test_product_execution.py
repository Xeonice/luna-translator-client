import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from product_execution import (PRODUCT_RUNTIME_CHECKS, PRODUCT_TEST_CHECKS, REFERENCE_CATALOGUE, REQUIRED_BEHAVIORS, REQUIRED_CAPTURES,
                               evidence_path, load_reference_catalogue, run_product_check,
                               validate_capture_report, validate_cargo_output, validate_freshness,
                               WINDOW_TITLE_PROOF_CHECK, WINDOW_TITLE_PROOF_SCOPE, WINDOW_TITLE_PROOF_SOURCES,
                               WINDOW_TITLE_PROOF_TARGETS, REQUIRED_WINDOW_TITLE_PROOF_CASES,
                               validate_window_title_proof_report)

ROOT = Path(__file__).resolve().parents[2]


class ProductExecutionTests(unittest.TestCase):
    def current_capability_fixture(self, layer='real_react'):
        import current_design_alignment as current
        def execute(*args):
            raise AssertionError('capability validation must never execute product callbacks')
        def inspect(*args):
            return []
        def recompute(*args):
            return []
        return current.Adapter(current.TARGET,layer,current.STATE_IDS,
                               ('guard-only-never-production',),execute,inspect,recompute)

    def test_execution_gaps_accept_real_code_owned_current_layer_capabilities(self):
        import current_design_alignment as current
        import verify
        registry={'current-react':self.current_capability_fixture(),
                  'current-native':self.current_capability_fixture('packaged_native')}
        metadata={'kind':'product','product_visual_checks':['current-react'],
                  'product_runtime_checks':['current-native']}
        with patch.dict(current.ADAPTERS,registry,clear=True):
            self.assertEqual([],verify.product_execution_gaps(metadata,[current.READING],'development'))
            self.assertEqual([],verify.product_execution_gaps(metadata,[current.READING],'release'))
            self.assertTrue(verify.product_execution_gaps({**metadata,'product_visual_checks':['current-native']},[current.READING],'development'))
            self.assertTrue(verify.product_execution_gaps({**metadata,'product_runtime_checks':['current-react']},[],'release'))

    def test_execution_gaps_reject_metadata_prototypes_incomplete_callbacks_and_wrong_target(self):
        import current_design_alignment as current
        import verify
        from dataclasses import replace
        adapter=self.current_capability_fixture()
        invalid=[{'target_id':current.TARGET,'layer':'real_react','states':sorted(current.STATE_IDS)},
                 replace(adapter,target_id='acceptance-v4'),replace(adapter,layer='html_prototype'),
                 replace(adapter,layer={}),
                 replace(adapter,recompute=adapter.inspect),replace(adapter,execute=None),
                 replace(adapter,states=frozenset({'unknown-state'}))]
        for capability in invalid:
            with self.subTest(capability=capability),patch.dict(current.ADAPTERS,{'fake-current':capability},clear=True):
                self.assertTrue(verify.product_execution_gaps({'kind':'product','product_visual_checks':['fake-current']},[current.READING],'development'))
        for checks in [['reading-alignment-preflight-execution'],['cp01-component-evidence'],
                       ['reading-component-tests'],['visual-guard-tests'],[{}],'current-react',[]]:
            with self.subTest(checks=checks):
                self.assertTrue(verify.product_execution_gaps({'kind':'product','product_visual_checks':checks},[current.READING],'development'))

    def test_execution_gaps_follow_actual_impact_instead_of_product_label_or_desktop_prefix(self):
        import current_design_alignment as current
        import verify
        self.assertEqual([],verify.product_execution_gaps({'kind':'product'},
                         ['apps/desktop/tests/visual/capture-reviewed-states.mjs',
                          'apps/desktop/src/ui/Select.test.tsx','docs/README.md'],'development'))
        self.assertTrue(verify.product_execution_gaps({'kind':'tooling'},[current.READING],'development'))
        self.assertTrue(verify.product_execution_gaps({'kind':'tooling'},['native/new-runtime.mm'],'development'))
        self.assertTrue(verify.product_execution_gaps({'kind':'tooling'},['scripts/build_desktop.py'],'development'))
        self.assertTrue(verify.product_execution_gaps({'kind':'tooling'},['new-product-package/asset.bin'],'development'))
        self.assertEqual([],verify.product_execution_gaps({'kind':'product'},['scripts/contracts/compare_product_capture.mjs'],'development'))
        self.assertTrue(verify.product_execution_gaps({'kind':'tooling'},['scripts/contracts/compare_product_capture-v2.mjs'],'development'))
        for path in ['native/docs/window.swift','crates/app-infra/src/docs/helpers.rs',
                     'apps/desktop/src-tauri/src/doc/window.cpp','packages/foo/docs/asset.js',
                     'resources/docs/asset.bin','assets/doc/icon.svg','runtime/doc/x','apps/desktop/src/docs/new.ts']:
            with self.subTest(path=path):
                self.assertTrue(verify.product_execution_gaps({'kind':'tooling'},[path],'development'))
        self.assertTrue(verify.product_execution_gaps({'kind':'product'},[],'release'))

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.directory = self.root / '.build/actual/capture'
        self.directory.mkdir(parents=True)
        # Guard fixtures are intentionally NOT PNGs and do not prove product rendering.
        # The real loader inspects their full fifteen-state provenance and hashes;
        # independent PNG decoding is exercised separately below with actual PNGs.
        self.catalogue = dict(schemaVersion=3, status='registered_independently_reviewed',
                              fixturePurpose='guard_only_not_product_acceptance',
                              pixelmatch=dict(threshold=0.1, includeAA=True, maxRatio=0),
                              frozenFiles={}, states=[])
        self.source = 'docs/design/first-product-v1/acceptance-v3/guard-only-design.html'
        self.adapter = 'docs/design/first-product-v1/acceptance-v3/guard-only-adapter.mjs'
        self.adapter_manifest = 'docs/design/first-product-v1/acceptance-v3/guard-only-adapter.json'
        self.freeze(self.source, b'guard-only independently authored design input, not product source')
        self.freeze(self.adapter, b'guard-only design renderer, not product code')
        adapter_bytes = json.dumps({'files': {self.adapter: self.catalogue['frozenFiles'][self.adapter]}}).encode()
        self.freeze(self.adapter_manifest, adapter_bytes)
        self.catalogue['designAdapterManifest'] = self.adapter_manifest
        self.rows, self.comparisons = [], []
        for identity in REQUIRED_CAPTURES:
            state = dict(id=identity,
                         designSourceHashes={self.source: self.catalogue['frozenFiles'][self.source]},
                         designAdapterHash=self.catalogue['frozenFiles'][self.adapter_manifest],
                         dataState={'fixture': 'guard-only-' + identity},
                         clientBoundary='continuous client content including tools and footer',
                         nativeExclusions=['system_traffic_lights'], variants=[])
            variants = ['top', 'bottom'] if identity == 'add-game' else ['top']
            children = []
            for variant in variants:
                reference = 'docs/design/first-product-v1/acceptance-v3/' + identity + '.' + variant + '.png'
                self.freeze(reference, ('guard-only non-PNG reference: ' + identity + '/' + variant).encode())
                sha = self.catalogue['frozenFiles'][reference]
                names = [identity + '.' + variant + suffix for suffix in ['.baseline.png', '.candidate.png', '.diff.png']]
                for name in names:
                    (self.directory / name).write_bytes((self.root / reference).read_bytes())
                viewport = dict(width=1040, height=720)
                conditions = dict(windowRole='hud' if identity == 'reading-hud' else 'main',
                                  viewport=viewport, theme='dark', transparency='glass')
                verified = copy.deepcopy(conditions)
                if identity == 'reading-hud':
                    conditions.update(sourceFont=18, hudMenu='open')
                    verified.update(sourceFonts=[{'size': 18}, {'size': 18}],
                                    visibleMenus=1, menuTriggerExpanded='true')
                    backdrop = 'docs/design/first-product-v1/acceptance-v3/guard-only-underlay.png'
                    self.freeze(backdrop, b'guard-only not a PNG, never a product backdrop')
                    state['underlay'] = {'fixture': 'guard-only common backdrop', 'file': backdrop,
                                         'sha256': self.catalogue['frozenFiles'][backdrop],
                                         'size': [2000, 1400], 'sampleOrigin': [320, 451], 'scale': 1}
                if identity == 'add-game':
                    conditions['fields'] = [{'role': 'textbox', 'name': '游戏名称', 'value': 'guard-only game'},
                                            {'role': 'textbox', 'name': '启动路径', 'value': 'C:\\guard-only.exe'}]
                    verified['fields'] = copy.deepcopy(conditions['fields'])
                scroll = [] if variant == 'top' else [{'className': 'dialog-body', 'scrollTop': 120,
                                                        'clientHeight': 320, 'scrollHeight': 440}]
                bounds = dict(x=0, y=0, width=64, height=32)
                spec = dict(variantId=variant, reference=reference, referenceSha256=sha,
                            expectedSize=[64, 32], viewport=viewport, deviceScaleFactor=1,
                            selector='[data-guard-only-client]', expectedBounds=bounds,
                            scrollTop=scroll, requiredConditions=conditions)
                state['variants'].append(spec)
                actual_conditions = dict(viewport=copy.deepcopy(viewport), deviceScaleFactor=1,
                                         selector=spec['selector'], bounds=copy.deepcopy(bounds),
                                         scrollTop=copy.deepcopy(scroll), verifiedConditions=verified)
                if 'underlay' in state:
                    actual_conditions['underlay'] = copy.deepcopy(state['underlay'])
                children.append(dict(id=identity, variantId=variant, status='passed', reference=reference,
                                     referenceSha256=sha, baselineCopy=names[0], candidate=names[1],
                                     candidateSha256=sha, diff=names[2], diffSha256=sha, maxRatio=0,
                                     pixelmatchThreshold=0.1, includeAA=True, differentPixels=0, ratio=0,
                                     expectedSize=[64, 32], actualSize=[64, 32], captureConditions=actual_conditions))
                self.comparisons.append(dict(id=identity, variantId=variant, diffSha256=sha,
                                             differentPixels=0, ratio=0, expectedSize=[64, 32], actualSize=[64, 32]))
            parent, *remaining = children
            if remaining:
                parent['variants'] = remaining
            self.rows.append(parent)
            self.catalogue['states'].append(state)
        self.report = dict(version=1, status='passed', behaviorStatus='passed', visualStatus='passed',
                           behavior=[dict(id=identity, status='passed') for identity in sorted(REQUIRED_BEHAVIORS)],
                           captures=self.rows, browserErrors=[])
        self.save_catalogue()

    def freeze(self, relative, contents):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)
        self.catalogue['frozenFiles'][relative] = hashlib.sha256(contents).hexdigest()

    def save_catalogue(self):
        # This helper registers isolated guard fixtures only; it never touches the
        # repository's reviewed baseline or any real reference image.
        path = self.root / REFERENCE_CATALOGUE
        path.write_text(json.dumps(self.catalogue, sort_keys=True))
        sha = hashlib.sha256(path.read_bytes()).hexdigest()
        self.report['referenceCatalogueSha256'] = sha
        baseline = self.root / 'docs/development/iteration-baseline.json'
        baseline.parent.mkdir(parents=True, exist_ok=True)
        baseline.write_text(json.dumps({'fixturePurpose': 'guard_only_not_product_acceptance',
                                       'files': [{'path': REFERENCE_CATALOGUE, 'sha256': sha}]}))

    def row(self, identity):
        return next(row for row in self.rows if row['id'] == identity)

    def state(self, identity):
        return next(row for row in self.catalogue['states'] if row['id'] == identity)

    def tearDown(self):
        self.tmp.cleanup()

    def validate(self, code=0):
        return validate_capture_report(self.root, self.directory, self.report, self.comparisons, code)[0]

    def test_complete_inspected_inventory_has_no_validation_errors(self):
        self.assertEqual(self.validate(), [])

    def test_catalogue_is_loaded_and_bound_to_actual_fifteen_state_inputs(self):
        states, sha = load_reference_catalogue(self.root)
        self.assertEqual(set(states), set(REQUIRED_CAPTURES))
        self.assertEqual(len(states), 15)
        self.assertEqual(sha, self.report['referenceCatalogueSha256'])
        self.assertEqual(len(states['add-game']['variants']), 2)

    def test_malformed_or_contradictory_registered_bounds_are_rejected(self):
        spec = self.state('add-game')['variants'][0]
        valid = copy.deepcopy(spec['expectedBounds'])
        for bounds in [{}, {**valid, 'width': -1}, {**valid, 'x': float('nan')},
                       {**valid, 'y': float('inf')}, {**valid, 'x': 1040}, {**valid, 'width': 1}]:
            with self.subTest(bounds=bounds):
                spec['expectedBounds'] = bounds
                self.save_catalogue()
                with self.assertRaisesRegex(ValueError, 'design client geometry'):
                    self.validate()

    def test_required_and_variant_viewports_must_be_consistent(self):
        spec = self.state('add-game')['variants'][0]
        spec['requiredConditions']['viewport'] = {'width': 1, 'height': 1}
        self.save_catalogue()
        with self.assertRaisesRegex(ValueError, 'design client geometry'):
            self.validate()

    def test_underlay_requires_frozen_input_and_finite_full_coverage(self):
        state = self.state('reading-hud')
        valid = copy.deepcopy(state['underlay'])
        mutations = [('file', '.build/unfrozen.png'), ('sha256', '0' * 64), ('scale', 2),
                     ('size', [0, 1400]), ('size', [float('inf'), 1400]),
                     ('sampleOrigin', [float('nan'), 0]), ('sampleOrigin', [-1, 0]),
                     ('sampleOrigin', [1999, 0])]
        for key, value in mutations:
            with self.subTest(key=key, value=value):
                state['underlay'] = {**valid, key: value}
                self.save_catalogue()
                with self.assertRaisesRegex(ValueError, 'underlay'):
                    self.validate()

    def test_explicit_shared_cover_input_is_distinct_from_fixed_desktop_sampling(self):
        state = self.state('reading-hud')
        state['underlay'].pop('scale')
        state['underlay'].pop('sampleOrigin')
        state['underlay']['placement'] = 'center / cover'
        self.row('reading-hud')['captureConditions']['underlay'] = copy.deepcopy(state['underlay'])
        self.save_catalogue()
        self.assertEqual(self.validate(), [])
        state['underlay']['scale'] = 1
        self.save_catalogue()
        with self.assertRaisesRegex(ValueError, 'mix cover and fixed'):
            self.validate()
        state['underlay'].pop('scale')
        state['underlay']['placement'] = 'stretch to hide mismatch'
        self.save_catalogue()
        with self.assertRaisesRegex(ValueError, 'underlay sampling'):
            self.validate()

    def test_missing_catalogue_fails_without_legacy_preview_fallback(self):
        (self.root / REFERENCE_CATALOGUE).unlink()
        with self.assertRaises(OSError):
            self.validate()

    def test_catalogue_missing_or_duplicate_state_cannot_be_registered(self):
        original = copy.deepcopy(self.catalogue['states'])
        for mutation in [original[:-1], original[:-1] + [original[0]]]:
            self.catalogue['states'] = mutation
            self.save_catalogue()
            with self.assertRaisesRegex(ValueError, 'fifteen distinct states'):
                self.validate()

    def test_unreviewed_catalogue_and_changed_comparator_contract_fail(self):
        for key, value in [('status', 'candidate'), ('schemaVersion', 2),
                           ('pixelmatch', dict(threshold=0.2, includeAA=True, maxRatio=0)),
                           ('pixelmatch', dict(threshold=0.1, includeAA=False, maxRatio=0)),
                           ('pixelmatch', dict(threshold=0.1, includeAA=True, maxRatio=0.01))]:
            with self.subTest(key=key, value=value):
                before = copy.deepcopy(self.catalogue[key])
                self.catalogue[key] = value
                self.save_catalogue()
                with self.assertRaisesRegex(ValueError, 'independently registered'):
                    self.validate()
                self.catalogue[key] = before

    def test_changed_catalogue_metadata_cannot_self_register_from_receipt_hash(self):
        path = self.root / REFERENCE_CATALOGUE
        self.catalogue['states'][0]['dataState'] = 'other scene'
        path.write_text(json.dumps(self.catalogue))
        self.report['referenceCatalogueSha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, 'baseline registration'):
            self.validate()

    def test_missing_duplicate_or_wrong_baseline_pin_fails(self):
        path = self.root / 'docs/development/iteration-baseline.json'
        valid = json.loads(path.read_text())['files'][0]
        for files in [[], [valid, valid], [{**valid, 'sha256': '0' * 64}]]:
            with self.subTest(files=files):
                path.write_text(json.dumps({'files': files}))
                with self.assertRaisesRegex(ValueError, 'baseline registration'):
                    self.validate()

    def test_receipt_catalogue_hash_cannot_be_stale(self):
        self.report['referenceCatalogueSha256'] = '0' * 64
        self.assertTrue(self.validate())

    def test_changed_design_source_or_adapter_bytes_fail(self):
        for relative in [self.source, self.adapter, self.adapter_manifest]:
            with self.subTest(relative=relative):
                path = self.root / relative
                before = path.read_bytes()
                path.write_bytes(before + b' unreviewed edit')
                with self.assertRaisesRegex(ValueError, 'Frozen design input/hash differs'):
                    self.validate()
                path.write_bytes(before)

    def test_unfrozen_reference_hash_or_path_fails(self):
        variant = self.catalogue['states'][0]['variants'][0]
        original = copy.deepcopy(variant)
        for key, value in [('referenceSha256', '0' * 64),
                           ('reference', 'docs/design/first-product-v1/previews/legacy.png'),
                           ('reference', '.build/actual/capture/not-a-design.png')]:
            with self.subTest(key=key):
                variant.clear()
                variant.update(original, **{key: value})
                self.save_catalogue()
                with self.assertRaisesRegex(ValueError, 'Unfrozen design variant'):
                    self.validate()

    def test_source_and_adapter_manifest_membership_are_checked(self):
        state = self.catalogue['states'][0]
        state['designSourceHashes'][self.source] = '0' * 64
        self.save_catalogue()
        with self.assertRaisesRegex(ValueError, 'design provenance'):
            self.validate()
        state['designSourceHashes'][self.source] = self.catalogue['frozenFiles'][self.source]
        state['designAdapterHash'] = '0' * 64
        self.save_catalogue()
        with self.assertRaisesRegex(ValueError, 'design provenance'):
            self.validate()
        state['designAdapterHash'] = self.catalogue['frozenFiles'][self.adapter_manifest]
        self.freeze(self.adapter_manifest, json.dumps({'files': {self.adapter: '0' * 64}}).encode())
        for row in self.catalogue['states']:
            row['designAdapterHash'] = self.catalogue['frozenFiles'][self.adapter_manifest]
        self.save_catalogue()
        with self.assertRaisesRegex(ValueError, 'adapter manifest'):
            self.validate()

    def test_changed_frozen_file_directory_or_symlink_is_rejected(self):
        path = self.root / self.source
        original = path.read_bytes()
        other = self.root / 'same-bytes-outside-design.html'
        other.write_bytes(original)
        path.unlink()
        path.symlink_to(other)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            self.validate()

    def test_missing_or_duplicate_registered_variant_fails(self):
        state = self.state('add-game')
        original = copy.deepcopy(state['variants'])
        for variants in [[], [original[0], original[0]]]:
            state['variants'] = variants
            self.save_catalogue()
            with self.assertRaisesRegex(ValueError, 'design variants'):
                self.validate()

    def test_incomplete_registered_geometry_and_preconditions_fail(self):
        state = self.catalogue['states'][0]
        original = copy.deepcopy(state['variants'][0])
        for key in ['viewport', 'deviceScaleFactor', 'selector', 'expectedBounds',
                    'scrollTop', 'expectedSize', 'requiredConditions']:
            with self.subTest(key=key):
                incomplete = copy.deepcopy(original)
                incomplete.pop(key)
                state['variants'][0] = incomplete
                self.save_catalogue()
                with self.assertRaisesRegex(ValueError, 'design geometry/state'):
                    self.validate()
        for key in ['windowRole', 'viewport', 'theme', 'transparency']:
            with self.subTest(requiredCondition=key):
                incomplete = copy.deepcopy(original)
                incomplete['requiredConditions'].pop(key)
                state['variants'][0] = incomplete
                self.save_catalogue()
                with self.assertRaisesRegex(ValueError, 'design geometry/state'):
                    self.validate()

    def test_unknown_omitted_duplicate_or_reordered_actual_variant_fails(self):
        parent = self.row('add-game')
        original = copy.deepcopy(parent['variants'])
        for variants in [[], [{**original[0], 'variantId': 'unregistered'}],
                         [original[0], original[0]]]:
            with self.subTest(variants=variants):
                parent['variants'] = variants
                self.assertTrue(self.validate())
        parent['variants'] = original
        parent['variantId'], parent['variants'][0]['variantId'] = 'bottom', 'top'
        self.assertTrue(self.validate())

    def test_comparison_must_cover_every_variant_once(self):
        original = self.comparisons[:]
        self.comparisons = [row for row in original if (row['id'], row['variantId']) != ('add-game', 'bottom')]
        self.assertTrue(self.validate())
        self.comparisons = original + [copy.deepcopy(original[0])]
        self.assertTrue(self.validate())
        self.comparisons = original + [{**original[0], 'variantId': 'unregistered'}]
        self.assertTrue(self.validate())

    def test_capture_geometry_and_scroll_must_match_registered_boundary(self):
        parent = self.row('add-game')
        original = copy.deepcopy(parent['captureConditions'])
        mutations = dict(viewport={'width': 1041, 'height': 720}, deviceScaleFactor=2,
                         selector='[data-cropped-field]', bounds={'x': 0, 'y': 0, 'width': 32, 'height': 32},
                         scrollTop=[{'className': 'dialog-body', 'scrollTop': 99}])
        for key, value in mutations.items():
            with self.subTest(key=key):
                parent['captureConditions'] = {**copy.deepcopy(original), key: value}
                self.assertTrue(self.validate())

    def test_scroll_diagnostics_distinguish_every_field_and_preserve_array_strictness(self):
        parent = self.row('add-game')
        row = parent['variants'][0]
        expected = copy.deepcopy(row['captureConditions']['scrollTop'])
        for field, value in [('className', 'another-surface'), ('scrollTop', 119), ('clientHeight', 319), ('scrollHeight', 441)]:
            with self.subTest(field=field):
                actual = copy.deepcopy(expected)
                actual[0][field] = value
                row['captureConditions']['scrollTop'] = actual
                errors = self.validate()
                detail = next(error for error in errors if 'scroll[0].' + field in error)
                self.assertIn('expected=' + json.dumps(expected[0][field]), detail)
                self.assertIn('actual=' + json.dumps(value), detail)
                for other in set(expected[0]) - {field}:
                    self.assertFalse(any('scroll[0].' + other + ' expected=' in error for error in errors))
        for mutation in [[], expected + expected, [{**expected[0], 'extra': True}], [{key:value for key,value in expected[0].items() if key != 'clientHeight'}]]:
            with self.subTest(array=mutation):
                row['captureConditions']['scrollTop'] = mutation
                self.assertTrue(any('scroll' in error for error in self.validate()))
        row['captureConditions']['scrollTop'] = expected
        self.assertEqual(self.validate(), [])

    def test_rendered_role_theme_transparency_and_viewport_are_required(self):
        row = self.row('reading-hud')
        original = copy.deepcopy(row['captureConditions']['verifiedConditions'])
        for key, value in [('windowRole', 'settings'), ('theme', 'light'),
                           ('transparency', 'opaque'), ('viewport', {'width': 1, 'height': 1})]:
            with self.subTest(key=key):
                row['captureConditions']['verifiedConditions'] = {**copy.deepcopy(original), key: value}
                self.assertTrue(self.validate())
        row['captureConditions']['verifiedConditions'] = {}
        self.assertTrue(self.validate())

    def test_every_rendered_source_font_and_menu_state_are_checked(self):
        row = self.row('reading-hud')
        original = copy.deepcopy(row['captureConditions']['verifiedConditions'])
        for key, value in [('sourceFonts', []), ('sourceFonts', [{'size': 18}, {'size': 17}]),
                           ('visibleMenus', 0), ('menuTriggerExpanded', 'false')]:
            with self.subTest(key=key, value=value):
                row['captureConditions']['verifiedConditions'] = {**copy.deepcopy(original), key: value}
                self.assertTrue(self.validate())

    def test_form_fields_must_match_name_role_value_and_order(self):
        row = self.row('add-game')
        original = copy.deepcopy(row['captureConditions']['verifiedConditions']['fields'])
        for fields in [[], [{**original[0], 'name': 'different'}],
                       [{**original[0], 'value': 'different'}], [{**original[0], 'role': 'combobox'}],
                       [original[0], original[0]], list(reversed(original))]:
            with self.subTest(fields=fields):
                row['captureConditions']['verifiedConditions']['fields'] = fields
                self.assertTrue(self.validate())

    def test_select_option_values_labels_and_order_are_part_of_same_data(self):
        row = self.row('add-game')
        field = self.state('add-game')['variants'][0]['requiredConditions']['fields'][0]
        field.update(role='combobox', name='guard-only scope', value='global', options=[
            {'value': 'global', 'label': 'All games'}, {'value': 'game:sample', 'label': 'Sample game'}])
        actual = row['captureConditions']['verifiedConditions']['fields']
        actual[0] = copy.deepcopy(field)
        self.save_catalogue()
        self.assertEqual(self.validate(), [])
        for options in [None, [], list(reversed(field['options'])),
                        [{'value': 'global', 'label': 'Different game'}, field['options'][1]]]:
            with self.subTest(options=options):
                actual[0] = copy.deepcopy(field)
                if options is None:
                    actual[0].pop('options')
                else:
                    actual[0]['options'] = options
                self.assertTrue(self.validate())

    def test_shared_underlay_identity_and_coordinates_cannot_change(self):
        conditions = self.row('reading-hud')['captureConditions']
        original = copy.deepcopy(conditions['underlay'])
        for key, value in [('fixture', 'other backdrop'), ('x', 321), ('y', 450)]:
            conditions['underlay'] = {**original, key: value}
            self.assertTrue(self.validate())
        conditions.pop('underlay')
        self.assertTrue(self.validate())

    def test_variant_failure_cannot_be_hidden_by_passed_parent_and_report(self):
        child = self.row('add-game')['variants'][0]
        actual = next(row for row in self.comparisons if (row['id'], row['variantId']) == ('add-game', 'bottom'))
        child.update(differentPixels=1, ratio=1 / (64 * 32))
        actual.update(differentPixels=1, ratio=1 / (64 * 32))
        errors = self.validate()
        self.assertTrue(errors)
        self.assertTrue(any('aggregate visual result contradicts' in error for error in errors))

    def test_child_candidate_hash_and_diff_are_checked_independently(self):
        child = self.row('add-game')['variants'][0]
        (self.directory / child['candidate']).write_bytes(b'changed child candidate')
        self.assertTrue(self.validate())
        (self.directory / child['candidate']).write_bytes((self.root / child['reference']).read_bytes())
        (self.directory / child['diff']).write_bytes(b'changed child diff')
        self.assertTrue(self.validate())

    def test_per_variant_pixel_comparator_options_cannot_be_relaxed(self):
        child = self.row('add-game')['variants'][0]
        original = copy.deepcopy(child)
        for key, value in [('pixelmatchThreshold', 0.2), ('includeAA', False), ('maxRatio', 0.5)]:
            with self.subTest(key=key):
                child.clear()
                child.update(copy.deepcopy(original), **{key: value})
                self.assertTrue(self.validate())

    def test_required_product_regressions_cannot_be_omitted(self):
        original = self.report['behavior']
        for identity in REQUIRED_BEHAVIORS:
            with self.subTest(identity=identity):
                self.report['behavior'] = [row for row in original if row['id'] != identity]
                self.assertTrue(self.validate())
        self.report['behavior'] = original

    def test_current_forty_product_regression_numbers_are_registered(self):
        self.assertTrue(set(range(1, 43)) <= {int(identity.split('-')[1]) for identity in REQUIRED_BEHAVIORS})

    def test_name_only_unknown_adapter_is_not_executed(self):
        with patch('product_execution.subprocess.run') as execute:
            with self.assertRaisesRegex(ValueError, 'Unregistered'):
                run_product_check(self.root, 'visual-guard-tests', self.root / '.build/out')
            execute.assert_not_called()

    def test_missing_candidate_cannot_be_accepted_from_passed_receipt(self):
        (self.directory / self.rows[0]['candidate']).unlink()
        with self.assertRaisesRegex(ValueError, 'Missing'):
            self.validate()

    def test_forged_candidate_hash_fails(self):
        self.rows[0]['candidateSha256'] = '0' * 64
        self.assertTrue(self.validate())

    def test_modified_or_stale_design_reference_fails(self):
        (self.root / self.rows[0]['reference']).write_bytes(b'new design')
        with self.assertRaisesRegex(ValueError, 'Frozen design input/hash differs'):
            self.validate()

    def test_replaced_baseline_copy_fails(self):
        (self.directory / self.rows[0]['baselineCopy']).write_bytes(b'fresh candidate pretending to be reference')
        self.assertTrue(self.validate())

    def test_missing_reference_raises(self):
        (self.root / self.rows[0]['reference']).unlink()
        with self.assertRaises(OSError):
            self.validate()

    def test_duplicate_or_omitted_capture_fails(self):
        self.rows.append(copy.deepcopy(self.rows[0]))
        self.assertTrue(self.validate())
        self.rows.pop()
        self.rows.pop()
        self.assertTrue(self.validate())

    def test_changed_registered_design_state_fails(self):
        self.rows[0]['reference'] = self.rows[1]['reference']
        self.assertTrue(self.validate())

    def test_diff_hash_and_pixel_values_are_independently_checked(self):
        self.comparisons[0].update(differentPixels=1, ratio=1)
        self.assertTrue(self.validate())
        self.comparisons[0]['diffSha256'] = '0' * 64
        self.assertTrue(self.validate())

    def test_dimension_change_is_failure_even_when_zero_different_pixels(self):
        self.rows[0]['actualSize'] = self.comparisons[0]['actualSize'] = [65, 32]
        self.assertTrue(self.validate())

    def test_threshold_relaxation_fails(self):
        self.rows[0]['maxRatio'] = 0.9
        self.assertTrue(self.validate())

    def test_infinite_or_nan_ratio_fails(self):
        for value in [float('nan'), float('inf')]:
            self.rows[0]['ratio'] = value
            self.assertTrue(self.validate())

    def test_additional_capture_is_not_silently_ignored(self):
        extra = copy.deepcopy(self.rows[0])
        extra['id'] = 'new-product-state'
        self.rows.append(extra)
        self.assertTrue(self.validate())

    def test_skipped_missing_or_duplicate_behavior_fails(self):
        self.report['behavior'][0]['skipped'] = True
        self.assertTrue(self.validate())
        self.report['behavior'][0].pop('skipped')
        self.report['behavior'].append(copy.deepcopy(self.report['behavior'][0]))
        self.assertTrue(self.validate())

    def test_browser_error_and_exit_disagreement_fail(self):
        self.assertTrue(self.validate(code=1))
        self.report['browserErrors'] = ['actual JS exception']
        self.assertTrue(self.validate())

    def test_evidence_cannot_escape_or_alias_symlink(self):
        with self.assertRaises(ValueError):
            evidence_path(self.directory, '../../secret')
        (self.directory / 'link').symlink_to(self.directory / self.rows[0]['candidate'])
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            evidence_path(self.directory, 'link')

    def test_zero_tests_ignored_tests_filtered_tests_and_failure_rejected(self):
        self.assertTrue(validate_cargo_output('test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;')[1])
        for text in ['ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;',
                     'ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out;',
                     'FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;']:
            self.assertTrue(validate_cargo_output('test result: ' + text)[1])
        self.assertEqual(validate_cargo_output('test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;'), (3, []))

    def test_old_or_forged_receipt_cannot_match_new_execution(self):
        fresh = {'executionNonce': 'actual-nonce', 'executedAt': '2026-10-19T00:00:05Z'}
        self.assertEqual(validate_freshness(fresh, 'actual-nonce', 1792368000, 1792368010), [])
        self.assertTrue(validate_freshness(fresh, 'different-run', 1792368000, 1792368010))
        self.assertTrue(validate_freshness(fresh, 'actual-nonce', 1792368100, 1792368110))
        self.assertTrue(validate_freshness({}, 'actual-nonce', 1792368000, 1792368010))

    def test_no_native_package_adapter_is_implied_by_component_checks(self):
        self.assertEqual(set(PRODUCT_RUNTIME_CHECKS), {'product-packaged-stability-tests'})
        self.assertTrue(set(PRODUCT_RUNTIME_CHECKS).isdisjoint(PRODUCT_TEST_CHECKS))
        self.assertEqual(PRODUCT_RUNTIME_CHECKS['product-packaged-stability-tests'], ('python3', 'scripts/quality/runtime_stability.py'))

    def test_actual_png_recomputation_detects_different_images(self):
        setup = "const fs=require('fs'),{PNG}=require('pngjs');const d=process.argv[1];for(const [n,c] of [['before',0],['after',255]]){const p=new PNG({width:2,height:2});p.data.fill(c);for(let i=3;i<p.data.length;i+=4)p.data[i]=255;fs.writeFileSync(d+'/'+n+'.png',PNG.sync.write(p));}"
        subprocess.run(['node', '-e', setup, str(self.directory)], cwd=ROOT, check=True, capture_output=True)
        report_path = self.directory / 'pixels.json'
        report_path.write_text(json.dumps({'captures': [{'id': 'actual', 'baselineCopy': 'before.png', 'candidate': 'after.png'}]}))
        result = subprocess.run(['node', 'scripts/contracts/compare_product_capture.mjs', str(report_path)], cwd=ROOT, capture_output=True, text=True, check=True)
        actual = json.loads(result.stdout)[0]
        self.assertEqual(actual['differentPixels'], 4)
        self.assertEqual(actual['ratio'], 1)

    def test_actual_png_recomputation_visits_every_scroll_variant(self):
        setup = "const fs=require('fs'),{PNG}=require('pngjs');const d=process.argv[1];for(const [n,c] of [['before',0],['after',255]]){const p=new PNG({width:2,height:2});p.data.fill(c);for(let i=3;i<p.data.length;i+=4)p.data[i]=255;fs.writeFileSync(d+'/'+n+'.png',PNG.sync.write(p));}"
        subprocess.run(['node', '-e', setup, str(self.directory)], cwd=ROOT, check=True, capture_output=True)
        report_path = self.directory / 'variant-pixels.json'
        report_path.write_text(json.dumps({'captures': [
            {'id': 'guard-only-with-variant', 'variantId': 'top', 'baselineCopy': 'before.png',
             'candidate': 'before.png', 'variants': [
                 {'variantId': 'bottom', 'baselineCopy': 'before.png', 'candidate': 'after.png'}]}]}))
        result = subprocess.run(['node', 'scripts/contracts/compare_product_capture.mjs', str(report_path)],
                                cwd=ROOT, capture_output=True, text=True, check=True)
        actual = json.loads(result.stdout)
        self.assertEqual([(row['id'], row['variantId']) for row in actual],
                         [('guard-only-with-variant', 'top'), ('guard-only-with-variant', 'bottom')])
        self.assertEqual([row['differentPixels'] for row in actual], [0, 4])
        self.assertEqual([row['ratio'] for row in actual], [0, 1])

    def test_guard_only_non_png_fixture_cannot_pass_actual_png_decoder(self):
        report_path = self.directory / 'not-product-pixels.json'
        report_path.write_text(json.dumps({'captures': [self.rows[0]]}))
        result = subprocess.run(['node', 'scripts/contracts/compare_product_capture.mjs', str(report_path)],
                                cwd=ROOT, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('"differentPixels":0', result.stdout)


class MaintainedProbeProducerTests(unittest.TestCase):
    """Execution guard fixtures only; these never establish product test success."""

    commands = {
        'product-hook-client-tests': ['cargo', 'test', '--locked', '--offline', '-p', 'hook-client', '--lib', '--', '--nocapture'],
        'product-target-probe-tests': ['cargo', 'test', '--locked', '--offline', '-p', 'lunahook-host', '--example', 'target_probe', '--', '--nocapture'],
    }

    def run_guard_fixture(self, adapter, stdout, exit_code=0, mutate_source=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / 'maintained-source.rs'
            source.write_text('// execution guard fixture, never product evidence\n')

            def execute(command, **kwargs):
                self.assertEqual(command, self.commands[adapter])
                self.assertEqual(kwargs['cwd'], root)
                self.assertTrue(kwargs['env']['LUNA_PRODUCT_EXECUTION_NONCE'])
                self.assertEqual(kwargs['timeout'], 900)
                if mutate_source:
                    source.write_text('// changed while producer was executing\n')
                return subprocess.CompletedProcess(command, exit_code, stdout, '')

            with patch('product_execution.subprocess.run', side_effect=execute) as actual_command:
                result = run_product_check(root, adapter, root / '.build/guard-only')
            self.assertEqual(actual_command.call_count, 1)
            receipt_path = root / result['receipt']
            self.assertEqual(hashlib.sha256(receipt_path.read_bytes()).hexdigest(), result['receipt_sha256'])
            self.assertEqual(result['command'], self.commands[adapter])
            self.assertFalse(result['product_packaged_native_game_tests_executed'])
            log = root / result['log']
            self.assertEqual(hashlib.sha256(log.read_bytes()).hexdigest(), result['log_sha256'])
            self.assertIn(stdout, log.read_text())
            return result

    def test_maintained_probe_commands_are_exact_and_registered(self):
        for adapter, command in self.commands.items():
            with self.subTest(adapter=adapter):
                self.assertEqual(list(PRODUCT_TEST_CHECKS[adapter]), command)
                self.assertNotIn(adapter, PRODUCT_RUNTIME_CHECKS)

    def test_both_actual_command_receipts_bind_count_and_log(self):
        for adapter in self.commands:
            with self.subTest(adapter=adapter):
                result = self.run_guard_fixture(adapter, 'test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n')
                self.assertEqual(result['status'], 'passed')
                self.assertEqual(result['test_count'], 3)
                self.assertEqual(result['errors'], [])

    def test_both_producers_reject_zero_skipped_filtered_failure_and_source_drift(self):
        invalid = [
            ('test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 0, False),
            ('test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 0, False),
            ('test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n', 0, False),
            ('test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 101, False),
            ('test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 1, False),
            ('test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 0, True),
        ]
        for adapter in self.commands:
            for stdout, code, changed in invalid:
                with self.subTest(adapter=adapter, stdout=stdout, code=code, changed=changed):
                    result = self.run_guard_fixture(adapter, stdout, code, changed)
                    self.assertEqual(result['status'], 'failed')
                    self.assertTrue(result['errors'])


class WindowTitleProofReceiptGuards(unittest.TestCase):
    """Synthetic execution receipts verify rejection logic, never WinAPI behavior."""

    def prepare_guard_fixture(self, root, directory, nonce='guard-only-nonce'):
        directory.mkdir(parents=True)
        dosdevices = root / '.build/guard-only-dosdevices'
        dosdevices.mkdir(parents=True, exist_ok=True)
        if not (dosdevices / 'z:').is_symlink():
            (dosdevices / 'z:').symlink_to('/')
        sources = {}
        for name in WINDOW_TITLE_PROOF_SOURCES:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('guard-only maintained source placeholder: ' + name)
            sources[name] = hashlib.sha256(path.read_bytes()).hexdigest()

        def artifact(name, data):
            path = directory / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            return {'path': name, 'sha256': hashlib.sha256(data).hexdigest()}

        builds, hashes = [], {}
        for kind in ('probe', 'fixture'):
            for arch, target in WINDOW_TITLE_PROOF_TARGETS.items():
                # Minimal PE headers exercise the architecture guard, not executable code.
                body = bytearray(144)
                body[:2], body[60:64], body[128:132] = b'MZ', (128).to_bytes(4, 'little'), b'PE\0\0'
                body[132:134] = {'x86': 0x14c, 'x64': 0x8664}[arch].to_bytes(2, 'little')
                body[136:144] = kind.encode().ljust(8, b'\0')
                exe = artifact(kind + '-' + arch + '.exe', bytes(body))
                hashes[kind, arch] = exe['sha256']
                command = ['cargo', 'build', '--locked', '--offline', '-p', 'lunahook-host', '--example', 'target_probe', '--target', target] if kind == 'probe' else ['guard-fixture-clang', '-o', kind + '-' + arch + '.exe']
                builds.append({'kind': kind, 'probe_arch': arch, 'target': target, 'exit_code': 0,
                               'command': command, 'exe': exe,
                               'log': artifact(kind + '-' + arch + '.log', b'guard-only build log\n')})
        cases = []
        for identity in sorted(REQUIRED_WINDOW_TITLE_PROOF_CASES):
            _, fixture, probe, _ = identity.split('-', 3)
            restored = identity.endswith(('unicode-exact-restore', 'observer-eof-restores'))
            cases.append({'id': identity, 'fixture_arch': fixture, 'probe_arch': probe,
                          'probe_exe_sha256': hashes['probe', probe], 'fixture_exe_sha256': hashes['fixture', fixture],
                          'status': 'passed', 'assertion_count': 1, 'exact_identity_guard': True,
                          'wm_settext_count': 2 if restored else 0,
                          'original_caption_sha256': 'a' * 64, 'final_caption_sha256': 'a' * 64,
                          'artifacts': [artifact(identity + '.log', b'guard-only raw call evidence\n')]})
        def json_artifact(name, data):
            return artifact(name, json.dumps(data).encode())

        def stream_artifact(name, values):
            return artifact(name, b''.join(json.dumps(row).encode() + b'\n' for row in values))

        def artifact_bytes(row):
            return (directory / row['path']).read_bytes()

        for case in cases:
            name = case['id'].split('-', 3)[3]
            scenarios = []
            for mode in (('slow650', 'slow2400') if name.startswith('timeout-') else ('unicode',)):
                prefix = case['id'] + '/' + mode + '/'
                fixture_build = next(b for b in builds if b['kind'] == 'fixture' and b['probe_arch'] == case['fixture_arch'])
                probe_build = next(b for b in builds if b['kind'] == 'probe' and b['probe_arch'] == case['probe_arch'])
                fixture_exe = directory / fixture_build['exe']['path']
                probe_exe = directory / probe_build['exe']['path']
                owned_class = 'LunaOwnedApiFixture-' + 'b' * 32
                image = 'Z:' + str(fixture_exe).replace('/', '\\')
                ready = dict(kind='ready', pid=10, hwnd='100', creation='42', bits=32 if case['fixture_arch'] == 'x86' else 64,
                             path_utf16le_hex=image.encode('utf-16le').hex(), class_utf16le_hex=owned_class.encode('utf-16le').hex(),
                             image_sha256=fixture_build['exe']['sha256'])
                base = dict(kind='snapshot', tag='baseline', pid=10, hwnd='100', class_utf16le_hex=ready['class_utf16le_hex'],
                            is_unicode=0 if name == 'ansi-prewrite-rejected' else 1, is_visible=0, is_foreground=0,
                            rect=[-30000, -30000, -29740, -29860], wm_settext_count=0,
                            caption_utf16le_hex='日本語fixture'.encode('utf-16le').hex())
                final = dict(base, tag='final')
                stdout_rows, stderr, code = [], '', 1
                if name == 'ansi-prewrite-rejected':
                    stdout_rows, code = [dict(status='unavailable', errorCode='window_title_proof_requires_unicode')], 0
                elif name in {'unicode-exact-restore', 'observer-eof-restores'}:
                    stdout_rows, code = [dict(status='ready'), dict(status='restored', titleRestored=True, identityVerified=True, requested=name != 'observer-eof-restores')], 0
                    final['wm_settext_count'] = 2
                elif name == 'observer-thirdparty-preserved':
                    stdout_rows, stderr = [dict(status='ready')], 'title_restore_failed'
                    final.update(wm_settext_count=1, caption_utf16le_hex='ThirdParty-日本語-caption'.encode('utf-16le').hex())
                elif name.startswith('caption-'):
                    stderr = 'caption_truncated' if 'truncated' in name else 'caption_invalid_utf16'
                elif name.startswith('identity-'):
                    stderr = 'identity rejected'
                elif name == 'handler-rejected-explicit-restore':
                    final['wm_settext_count'], stderr = 2, 'window_title_update_rejected title_restore_failed'
                elif name == 'stdout-failed-explicit-restore':
                    final['wm_settext_count'], stderr = 2, 'proof output failed original_title_restored'
                elif name == 'restore-invalid-utf16-not-acknowledged':
                    stdout_rows, stderr = [dict(status='ready')], 'title_restore_failed invalid_utf16'
                    final.update(wm_settext_count=2, caption_utf16le_hex='00d8')
                elif mode == 'slow650':
                    final['wm_settext_count'], stderr = 2, 'window_title_update_failed original_title_restored'
                else:
                    final.update(wm_settext_count=2, caption_utf16le_hex='pending nonce'.encode('utf-16le').hex())
                    stderr = 'window_title_update_failed title_restore_failed unconfirmed'
                fixture_rows = [ready, base]
                if name == 'observer-thirdparty-preserved':
                    fixture_rows.append(dict(final, tag='thirdparty-visible'))
                fixture_rows += [final, dict(kind='normal_exit')]
                fixture_stdout = stream_artifact(prefix + 'fixture/stdout.bin', fixture_rows)
                fixture_completion = dict(process_exited=True, exit_code=0, setup_error=None, stdin_closed=True,
                                          readers={key: dict(state='eof', error=None, alive=False) for key in ('stdout', 'stderr')},
                                          reader_completion_confirmed=True)
                fixture_process = json_artifact(prefix + 'fixture/process.json', dict(command=[str(fixture_exe), mode, owned_class], exit_code=0, parsed_stdout=fixture_rows, stdout_bytes=len(artifact_bytes(fixture_stdout)), completion=fixture_completion))
                args = ['100', '11' if name == 'identity-pid-rejected' else '10', image + '.wrong' if name == 'identity-path-rejected' else image, '43' if name == 'identity-creation-rejected' else '42', 'LunaWindowProof-' + 'c' * 32, case['fixture_arch'], owned_class]
                command = [str(probe_exe), '--window-title-challenge', *args]
                probe_stdout = stream_artifact(prefix + 'probe/stdout.bin', stdout_rows)
                probe_stderr = artifact(prefix + 'probe/stderr.bin', stderr.encode())
                probe_completion = copy.deepcopy(fixture_completion)
                probe_completion['exit_code'] = code
                if name == 'stdout-failed-explicit-restore':
                    probe_completion['readers']['stdout']['state'] = 'intentionally_closed'
                probe_process = json_artifact(prefix + 'probe/process.json', dict(command=command, exit_code=code, parsed_stdout=stdout_rows, stdout_bytes=len(artifact_bytes(probe_stdout)), completion=probe_completion))
                scenarios.append(dict(fixture_stdout_artifact=fixture_stdout, fixture_process_artifact=fixture_process,
                                      fixture_ready=ready, expected_fixture_class=owned_class, baseline=base, final=final,
                                      fixture_owner_events=fixture_rows, mode=mode, assertion_count=1, cleanup_confirmed=True,
                                      cleanup={label: dict(value, confirmed=True, errors=[], normal_fixture_exit=True) for label, value in (('fixture', fixture_completion), ('probe', probe_completion))},
                                      attempt=dict(stdout_artifact=probe_stdout, stderr_artifact=probe_stderr, process_artifact=probe_process,
                                                   parsed_stdout=stdout_rows, command=command, exit_code=code, stderr=stderr, probe_exe_sha256=probe_build['exe']['sha256'])))
            case['observation'] = json_artifact(case['id'] + '/observation.json', dict(scenarios=scenarios))
            case['original_caption_sha256'] = hashlib.sha256(bytes.fromhex(scenarios[0]['baseline']['caption_utf16le_hex'])).hexdigest()
            case['final_caption_sha256'] = hashlib.sha256(bytes.fromhex(scenarios[0]['final']['caption_utf16le_hex'])).hexdigest()
            case['wm_settext_count'] = sum(s['final']['wm_settext_count'] - s['baseline']['wm_settext_count'] for s in scenarios)
            case['assertion_count'] = len(scenarios)
            case['cleanup_confirmed'] = True
        report = {'schema_version': 1, 'adapter_id': WINDOW_TITLE_PROOF_CHECK, 'execution_nonce': nonce,
                  'dosdevices_path': str(dosdevices),
                  'scope': WINDOW_TITLE_PROOF_SCOPE, 'native_game_verified': False,
                  'source_before_after_equal': True, 'source_sha256': sources, 'status': 'passed', 'errors': [],
                  'fixture_purpose': 'synthetic_guard_only_not_WinAPI_execution',
                  'builds': builds, 'cases': cases, 'case_count': len(cases), 'assertion_count': sum(row['assertion_count'] for row in cases)}
        (directory / 'receipt.json').write_text(json.dumps(report))
        return report

    def test_synthetic_receipt_schema_can_be_inspected_without_claiming_game_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory = root / '.build/guard'
            self.prepare_guard_fixture(root, directory)
            errors, details = validate_window_title_proof_report(root, directory, 'guard-only-nonce')
            self.assertEqual(errors, [])
            self.assertEqual(details['test_count'], 30)
            self.assertEqual(details['assertion_count'], 32)
            self.assertGreater(details['independently_recomputed_assertion_count'], 700)
            self.assertFalse(details['native_game_verified'])

    def test_missing_cases_false_counts_changed_sources_or_exes_and_wrong_scope_are_rejected(self):
        def change_original(report):
            next(row for row in report['cases'] if row['id'].endswith('unicode-exact-restore'))['final_caption_sha256'] = 'b' * 64

        def write_ansi(report):
            next(row for row in report['cases'] if row['id'].endswith('ansi-prewrite-rejected'))['wm_settext_count'] = 1

        mutations = [
            lambda r: r.update(execution_nonce='stale'),
            lambda r: r.update(native_game_verified=True),
            lambda r: r.update(source_before_after_equal=False),
            lambda r: r['source_sha256'].pop('Cargo.lock'),
            lambda r: r['source_sha256'].update({'Cargo.lock': '0' * 64}),
            lambda r: r['cases'].pop(),
            lambda r: r['cases'].__setitem__(0, copy.deepcopy(r['cases'][1])),
            lambda r: r['cases'][0].update(assertion_count=0),
            lambda r: r['cases'][0].update(probe_exe_sha256='0' * 64),
            lambda r: r.update(assertion_count=999),
            lambda r: r['builds'][0].update(command=['c-operator-instead-of-Rust']),
            lambda r: r['builds'][0]['exe'].update(sha256='0' * 64),
            lambda r: r['builds'][0].update(probe_arch='x64'),
            lambda r: r['cases'][0]['artifacts'][0].update(path='../outside.log'),
            change_original, write_ansi,
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(mutation=index), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                directory = root / '.build/guard'
                report = self.prepare_guard_fixture(root, directory)
                mutate(report)
                (directory / 'receipt.json').write_text(json.dumps(report))
                try:
                    errors, _ = validate_window_title_proof_report(root, directory, 'guard-only-nonce')
                except (ValueError, KeyError, TypeError) as error:
                    errors = [str(error)]
                self.assertTrue(errors)

    def test_registered_command_requires_fresh_nonce_and_inspects_receipt_instead_of_cargo_text(self):
        for emit_receipt in (True, False):
            with self.subTest(emit_receipt=emit_receipt), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                self.prepare_guard_fixture(root, root / '.build/initial-schema')

                def execute(command, **kwargs):
                    self.assertEqual(command[:2], ['python3', 'scripts/quality/window_title_proof_api.py'])
                    nonce = command[command.index('--execution-nonce') + 1]
                    self.assertEqual(nonce, kwargs['env']['LUNA_PRODUCT_EXECUTION_NONCE'])
                    if emit_receipt:
                        self.prepare_guard_fixture(root, Path(command[command.index('--output') + 1]), nonce)
                    return subprocess.CompletedProcess(command, 0, 'test result: ok. 999 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n', '')

                with patch('product_execution.subprocess.run', side_effect=execute):
                    receipt = run_product_check(root, WINDOW_TITLE_PROOF_CHECK, root / '.build/guard-execution')
                self.assertEqual(receipt['status'], 'passed' if emit_receipt else 'failed')
                self.assertFalse(receipt['product_packaged_native_game_tests_executed'])
                if emit_receipt:
                    self.assertEqual(receipt['test_count'], 30)

    def test_case_summary_cannot_disagree_with_unchanged_raw_observations(self):
        for field in ('captions', 'writes', 'assertions'):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                directory = root / '.build/guard'
                report = self.prepare_guard_fixture(root, directory)
                case = next(row for row in report['cases'] if row['id'].endswith('unicode-exact-restore'))
                if field == 'captions':
                    case['original_caption_sha256'] = case['final_caption_sha256'] = 'c' * 64
                elif field == 'writes':
                    case['wm_settext_count'] = 3
                else:
                    case['assertion_count'] += 1
                    report['assertion_count'] += 1
                (directory / 'receipt.json').write_text(json.dumps(report))
                errors, _ = validate_window_title_proof_report(root, directory, 'guard-only-nonce')
                self.assertTrue(any('raw observation case' in error for error in errors))

    def test_copied_identity_and_probe_status_cannot_override_original_stdout_bytes(self):
        for changed in ('fixture_pid', 'probe_status', 'foreground', 'loaded_image'):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                directory = root / '.build/guard'
                report = self.prepare_guard_fixture(root, directory)
                case = next(row for row in report['cases'] if row['id'].endswith('unicode-exact-restore'))
                observation_path = directory / case['observation']['path']
                observation = json.loads(observation_path.read_text())
                scenario = observation['scenarios'][0]
                if changed == 'fixture_pid':
                    scenario['fixture_ready']['pid'] += 1
                elif changed == 'probe_status':
                    scenario['attempt']['parsed_stdout'] = [{'status': 'unavailable'}]
                else:
                    raw_path = directory / scenario['fixture_stdout_artifact']['path']
                    raw_rows = [json.loads(row) for row in raw_path.read_bytes().splitlines()]
                    if changed == 'foreground':
                        next(row for row in raw_rows if row.get('tag') == 'final')['is_foreground'] = 1
                    else:
                        next(row for row in raw_rows if row.get('kind') == 'ready')['image_sha256'] = '0' * 64
                    raw_path.write_bytes(b''.join(json.dumps(row).encode() + b'\n' for row in raw_rows))
                    scenario['fixture_stdout_artifact']['sha256'] = hashlib.sha256(raw_path.read_bytes()).hexdigest()
                observation_path.write_text(json.dumps(observation))
                case['observation']['sha256'] = hashlib.sha256(observation_path.read_bytes()).hexdigest()
                (directory / 'receipt.json').write_text(json.dumps(report))
                errors, _ = validate_window_title_proof_report(root, directory, 'guard-only-nonce')
                self.assertTrue(any('raw observation' in error for error in errors))

    def test_handler_refusal_and_owned_reader_completion_cannot_be_weakened(self):
        for changed in ('one-write', 'drop-retry', 'success-ack', 'success-text', 'reader-alive', 'reader-no-eof', 'setup-error'):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                directory = root / '.build/guard'
                report = self.prepare_guard_fixture(root, directory)
                case = next(row for row in report['cases'] if row['id'].endswith('handler-rejected-explicit-restore'))
                observation_path = directory / case['observation']['path']
                observation = json.loads(observation_path.read_text())
                scenario = observation['scenarios'][0]

                def save_ref(ref, value, raw=False):
                    path = directory / ref['path']
                    path.write_bytes(value if raw else json.dumps(value).encode())
                    ref['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()

                if changed in ('one-write', 'drop-retry'):
                    count = 1 if changed == 'one-write' else 3
                    fixture_rows = copy.deepcopy(scenario['fixture_owner_events'])
                    next(row for row in fixture_rows if row.get('tag') == 'final')['wm_settext_count'] = count
                    fixture_bytes = b''.join(json.dumps(row).encode() + b'\n' for row in fixture_rows)
                    save_ref(scenario['fixture_stdout_artifact'], fixture_bytes, raw=True)
                    fixture_process = json.loads((directory / scenario['fixture_process_artifact']['path']).read_text())
                    fixture_process.update(parsed_stdout=fixture_rows, stdout_bytes=len(fixture_bytes))
                    save_ref(scenario['fixture_process_artifact'], fixture_process)
                    scenario['fixture_owner_events'] = fixture_rows
                    scenario['final']['wm_settext_count'] = case['wm_settext_count'] = count
                elif changed == 'success-ack':
                    attempt = scenario['attempt']
                    rows = [{'status': 'restored', 'titleRestored': True, 'identityVerified': True, 'requested': True}]
                    data = b''.join(json.dumps(row).encode() + b'\n' for row in rows)
                    save_ref(attempt['stdout_artifact'], data, raw=True)
                    process = json.loads((directory / attempt['process_artifact']['path']).read_text())
                    process.update(parsed_stdout=rows, stdout_bytes=len(data))
                    save_ref(attempt['process_artifact'], process)
                    attempt['parsed_stdout'] = rows
                elif changed == 'success-text':
                    scenario['attempt']['stderr'] += ' original_title_restored'
                    save_ref(scenario['attempt']['stderr_artifact'], scenario['attempt']['stderr'].encode(), raw=True)
                else:
                    process_ref = scenario['attempt']['process_artifact']
                    process = json.loads((directory / process_ref['path']).read_text())
                    completion = process['completion']
                    if changed == 'reader-alive':
                        completion['readers']['stdout']['alive'] = True
                    elif changed == 'reader-no-eof':
                        completion['readers']['stdout']['state'] = 'reading'
                    else:
                        completion['setup_error'] = 'controlled reader setup failure'
                    scenario['cleanup']['probe'].update(copy.deepcopy(completion))
                    save_ref(process_ref, process)
                save_ref(case['observation'], observation)
                (directory / 'receipt.json').write_text(json.dumps(report))
                errors, _ = validate_window_title_proof_report(root, directory, 'guard-only-nonce')
                self.assertTrue(any('raw observation' in error for error in errors))


if __name__ == '__main__':
    unittest.main()
