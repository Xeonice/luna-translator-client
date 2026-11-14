"""Scoped-acceptance guard tests; fixture success never grants product approval.

The fixture makes real, differently coloured PNGs and actual finite-case facts.
Only the expensive signed-package/original Native-process boundary is replaced;
JSON, file/source identities, facts, geometry, pixels and dispatcher run for real.
The retained S5 package is separately inspected by the real acceptance check.
"""
from __future__ import annotations

import copy
from contextlib import ExitStack
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

import current_general_visual_acceptance as acceptance
from current_general_visual_authority import (
    ROOT, SCOPE, STATE_IDS, authority, capture_cases, case_key, source_inputs,
    context as visual_context,
)
from current_general_visual_inspect import check_facts
from current_general_visual_recompute import pixels
from gate_core import digest, source_files


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True,
                               allow_nan=False) + '\n', encoding='utf-8')


def finite_observation(case, side, nonce):
    """Produce facts that satisfy the real code-owned state expectations."""
    name = case['state_id'].removeprefix('all-page-settings-general-')
    initial = name == 'loading' and case['subcase'] == 'initial'
    dirty = name in ('error', 'save-failed', 'disabled', 'restore-inheritance')
    selected = ((False, True, True) if name == 'disabled' else
                (True, True, True) if name in ('error', 'save-failed') else
                (True, True, False))
    labels = ('可点击日语分词', '显示日语注音', '显示词性标记')
    switches = [] if initial else [
        {'label': label, 'selected': value,
         'disabled': name in ('loading', 'error') or
                     (name == 'disabled' and label == labels[2])}
        for label, value in zip(labels, selected)
    ]
    notices = {
        'loading': '正在读取已保存的阅读偏好来源…',
        'error': '阅读偏好来源读取失败：暂时无法读取已保存的偏好。已显示的编辑与已保存配置保留。',
        'save-failed': '阅读偏好保存失败，请重试。编辑内容已保留。',
    }
    facts = {
        'errors': [], 'fontStatus': 'loaded', 'theme': case['theme'],
        'zoom': case['zoom'], 'dirty': dirty,
        'menuOpen': case['subcase'] == 'menu-open',
        'scope': {
            'kind': 'game' if name in ('scope-game', 'restore-inheritance', 'long-text') else 'global',
            'disabled': dirty or name == 'loading',
            'expanded': case['subcase'] == 'menu-open',
            'label': '長いゲーム名と保存された読書設定の表示確認' if name == 'long-text' else '全局',
        },
        'headings': ['通用', '阅读记录', '本机偏好'],
        'switches': switches,
        'geometry': {
            'frame': {'x': 0, 'y': 0, 'width': case['width'], 'height': case['height']},
            'title': {'x': 200, 'y': 30, 'width': 120, 'height': 28},
            'lexical_card': None if initial else {'x': 200, 'y': 100, 'width': 450, 'height': 170},
        },
        'text': notices.get(name, ''),
        'focused': {'id': 'scope-button'},
        'nativeAcl': {
            'coreCommand': 'plugin:window|scale_factor', 'coreDenied': True,
            'coreError': 'not allowed by ACL',
            'businessCommand': 'application_snapshot', 'businessUnknown': True,
            'businessError': 'Command application_snapshot not found',
        },
    }
    if side == 'candidate':
        facts.update(
            renderSource='production-WindowRoot', startupWrites=[], forbiddenEffects=[],
            provenance={'controlledTransport': True, 'productionBootstrapImported': False,
                        'osRuntimeVerified': False, 'storageRuntimeVerified': False,
                        'gameExecutionVerified': False},
            controlledCommands=[],
            saveCalls=[{'kind': 'save_scoped_preferences'}] if name in ('error', 'save-failed') else [],
        )
    else:
        facts.update(renderSource='independent-user-approved-general-html',
                     provenance={'osRuntimeVerified': False, 'storageRuntimeVerified': False,
                                 'gameTestsExecuted': False})
    value = {'context': visual_context(case, side, nonce, 'packaged_native'),
             'error': None, 'facts': facts}
    check_facts(case, value, side, 'packaged_native')
    return value


class AcceptanceFixture:
    """Small source epoch and 27 real visual cases, all confined to a temp root."""

    def __init__(self, root):
        self.root = Path(root).resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        self.record = {}
        self.immutable_paths = set()
        self.original_paths = {}
        self.cases = list(capture_cases())
        self.artifact = '.build/fixture-package/Luna.zip'
        artifact = self.root / self.artifact
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b'unit-test package boundary; never executable or product evidence')
        self.artifact_sha = digest(artifact)
        # Use real approved-source bytes, so authority() itself is not mocked.
        for relative in source_inputs():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, path)
        production = self.root / 'apps/desktop/src/accepted-product.ts'
        production.parent.mkdir(parents=True, exist_ok=True)
        production.write_text('export const acceptedSource = 1;\n')
        self.before = source_files(self.root)
        self.source_sha = acceptance.tree_sha(self.before)
        self.snapshot_path = 'openspec/changes/consume-scoped-visual-acceptance/source-snapshot.json'
        self.put(self.snapshot_path, {'schema_version': 1, 'files': self.before,
                                    'source_tree_sha256': self.source_sha})
        evaluator = self.root / 'scripts/quality/current_general_visual_acceptance.py'
        evaluator.parent.mkdir(parents=True, exist_ok=True)
        evaluator.write_text('# Fixture verifier-source identity; never executed.\n')
        self.observations = []
        self.errors = []
        for number, case in enumerate(self.cases):
            raw_directories = {}
            for side in ('reference', 'candidate'):
                relative = '.build/fixture-' + side + '/' + case_key(case)
                directory = self.root / relative
                directory.mkdir(parents=True)
                raw_directories[side] = relative
                obs = finite_observation(case, side, f'{number * 2 + (side == "candidate") + 1:032x}')
                self.put(relative + '/observation.json', {'observation': obs})
                image = Image.new('RGBA', (case['width'], case['height']), (10, 20, 30, 255))
                if side == 'candidate':
                    image.putpixel((0, 0), (11, 20, 30, 255))
                frames = []
                for frame in ('frame-1.png', 'frame-2.png'):
                    image.save(directory / frame)
                    self.immutable_paths.add(relative + '/' + frame)
                    frames.append({'path': frame, 'sha256': digest(directory / frame),
                                   'bytes': (directory / frame).stat().st_size,
                                   'pixel_width': case['width'], 'pixel_height': case['height']})
                image.close()
                self.put(relative + '/capture.json', {
                    'kind': 'general-packaged-native-render-capture/v1', 'status': 'captured',
                    'errors': [], 'side': side, **case, 'frames': frames,
                    'source_tree_sha256': self.source_sha,
                    'package': {'source_tree_sha256': self.source_sha,
                                'archive_sha256': self.artifact_sha},
                    'game_tests_executed': False, 'os_runtime_verified': False,
                    'storage_runtime_verified': False,
                })
                self.put(relative + '.command.json', {'exit_code': 0, 'cwd': str(self.root)})
            comparison = {'different_rgba_pixels': 1, 'width': case['width'],
                          'height': case['height'], 'threshold': 0, 'mask': False,
                          'crop': False, 'resampled': False}
            self.observations.append({
                'case': case, 'candidate_raw_directory': raw_directories['candidate'],
                'reference_raw_directory': raw_directories['reference'],
                'comparisons': [comparison.copy(), comparison.copy()],
            })
            self.errors.extend([
                f'General original packaged_native {case_key(case)}: 1 RGBA pixels differ (required 0)'
            ] * 2)
        self.build_originals()

    def put(self, relative, value):
        write_json(self.root / relative, value)
        self.immutable_paths.add(relative)
        return self.reference(relative)

    def reference(self, relative):
        return {'path': relative, 'sha256': digest(self.root / relative)}

    def load(self, relative):
        return json.loads((self.root / relative).read_text())

    def build_originals(self):
        # The acceptance implementation reads these real files; no return value
        # from evaluate/inspect/pixels is ever replaced with a pass label.
        source_authority = authority(self.root)
        nonce = 'a' * 32
        started, finished = '2026-11-08T00:00:01+00:00', '2026-11-08T00:00:02+00:00'
        original_context = {'execution_nonce': nonce, 'source_tree_sha256': self.source_sha,
                            'artifact_sha256': self.artifact_sha}
        rows = [{'case': row['case'], 'raw_directory': row['candidate_raw_directory']}
                for row in self.observations]
        report = {
            'kind': 'general-packaged-native-execution/v1', 'scope': SCOPE,
            'layer': 'packaged_native', 'source_authority': source_authority,
            'source_tree_sha256': self.source_sha, 'artifact_sha256': self.artifact_sha,
            'execution_nonce': nonce, 'context': original_context,
            'started_at': started, 'finished_at': finished,
            'observations': rows, 'errors': [], 'collection_complete': True,
            'game_tests_executed': False, 'os_runtime_verified': False,
            'storage_runtime_verified': False,
        }
        values = {
            'root_receipt': {'status': 'failed', 'source_tree_sha256': self.source_sha,
                             'steps': [{'id': acceptance.ADAPTER_ID, 'status': 'failed'},
                                       {'id': 'product-general-visual-react-tests', 'status': 'passed'},
                                       {'id': 'unrelated-global-gap', 'status': 'failed',
                                        'errors': ['unrelated global product obligation remains']}]},
            'native_report': report,
            'native_request': original_context,
            'native_execution': {'exit_code': 0, 'cwd': str(self.root),
                                 'started_at': started, 'finished_at': finished},
            'native_receipt': {'status': 'failed', 'errors': self.errors,
                               'adapter_id': acceptance.ADAPTER_ID, 'execution_nonce': nonce,
                               'source_tree_sha256': self.source_sha,
                               'artifact_sha256': self.artifact_sha},
            'native_validation': {'inspection': [], 'independent_recomputation': self.errors},
            'react_report': {'status': 'passed', 'errors': [], 'observations': rows,
                             'collection_complete': True},
            'react_receipt': {'status': 'passed', 'errors': [], 'layer': 'real_react',
                              'source_tree_sha256': self.source_sha, 'artifact_sha256': self.artifact_sha},
            'react_validation': {'inspection': [], 'independent_recomputation': []},
            'reference_catalogue': {'kind': 'general-independent-reference-catalogue/v1',
                                    'scope': SCOPE, 'source_authority': source_authority},
            'reference_review': {'kind': 'general-independent-reference-review/v1',
                                 'status': 'approved'},
            'reference_generation': {'kind': 'general-reference-candidates/v1',
                                     'layer': 'packaged_native', 'authority': source_authority,
                                     'captures': [{'case': row['case'], 'raw_directory': row['reference_raw_directory']}
                                                  for row in self.observations]},
        }
        for key, value in values.items():
            relative = '.build/fixture-originals/' + key + '.json'
            self.original_paths[key] = relative
            self.put(relative, value)
        decision = {
            'kind': 'explicit-scoped-native-visual-user-decision',
            'status': 'accepted_by_user_with_retained_bit_exact_failure',
            'artifact_sha256': self.artifact_sha, 'source_tree_sha256': self.source_sha,
            'raw_comparison_threshold': 0,
            'user_statement': '卡在 99% 我理解就可以了，不用刷的更高',
            **{key: False for key in (
                'approved_source_or_reference_modified', 'accepts_missing_content_or_layout_errors',
                'functionality_or_interaction_acceptance_promoted', 'full_product_acceptance_promoted',
                'future_changed_source_or_artifact_automatically_accepted')},
        }
        self.put(acceptance.DECISION_PATH, decision)
        self.record = {
            'schema_version': 1,
            'kind': 'general-native-scoped-visual-acceptance/v1',
            'status': 'accepted_by_user', 'scope': SCOPE,
            'candidate_source_tree_sha256': self.source_sha,
            'artifact': self.reference(self.artifact),
            'source_snapshot': self.reference(self.snapshot_path),
            'decision': self.reference(acceptance.DECISION_PATH),
            'allowed_current_delta_paths': list(acceptance.ALLOWED_CURRENT_DELTA_PATHS),
            'originals': {key: self.reference(relative)
                          for key, relative in self.original_paths.items()},
            'observations': self.observations,
        }
        self.seal()

    def seal(self):
        """Fixture-only trusted registration; production pins remain untouched."""
        def update_original(key, **fields):
            relative = self.original_paths[key]
            value = self.load(relative)
            value.update(fields)
            write_json(self.root / relative, value)

        catalogue_entries = [
            {'case': row['case'], 'case_key': case_key(row['case']),
             'png': self.reference(row['reference_raw_directory'] + '/frame-1.png'),
             'facts': self.reference(row['reference_raw_directory'] + '/observation.json')}
            for row in self.observations
        ]
        update_original('reference_catalogue', layers={'packaged_native': {
            'generation': self.reference(self.original_paths['reference_generation']),
            'entries': catalogue_entries,
        }})
        update_original('reference_review',
            catalogue=self.reference(self.original_paths['reference_catalogue']),
            reviewed_png_sha256={row['png']['path']: row['png']['sha256'] for row in catalogue_entries},
            reviewed_facts_sha256={row['facts']['path']: row['facts']['sha256'] for row in catalogue_entries})
        update_original('native_receipt', report=self.reference(self.original_paths['native_report']),
                        execution=self.reference(self.original_paths['native_execution']))
        update_original('react_receipt', report=self.reference(self.original_paths['react_report']))
        self.record['immutable_files'] = [
            {**self.reference(relative), 'bytes': (self.root / relative).stat().st_size}
            for relative in sorted(self.immutable_paths)
            if (self.root / relative).is_file()
        ]
        self.record['originals'] = {key: self.reference(relative)
                                    for key, relative in self.original_paths.items()}
        write_json(self.root / acceptance.RECORD_PATH, self.record)

    def inspect_fixture_case(self, root, directory, case, side, batch):
        """Only replace original OS/process provenance, never visual judgement."""
        directory = Path(root) / directory
        capture = acceptance.load(directory / 'capture.json')
        observed = acceptance.load(directory / 'observation.json')['observation']
        if capture.get('side') != side or any(capture.get(k) != case[k]
                for k in ('state_id', 'canvas_id', 'subcase')):
            raise ValueError('Fixture original case identity differs')
        frames = capture['frames']
        if len(frames) != 2 or [row['path'] for row in frames] != ['frame-1.png', 'frame-2.png']:
            raise ValueError('Fixture fixed original frame order differs')
        check_facts(case, observed, side, 'packaged_native')
        return [directory / row['path'] for row in frames], observed, capture

    def patches(self, *, native=True):
        stack = ExitStack()
        for key, value in (
            ('RECORD_SHA256', digest(self.root / acceptance.RECORD_PATH)),
            ('ACCEPTED_ARTIFACT_SHA256', self.artifact_sha),
            ('CANDIDATE_SOURCE_SHA256', self.source_sha),
            ('DECISION_SHA256', digest(self.root / acceptance.DECISION_PATH)),
        ):
            stack.enter_context(patch.object(acceptance, key, value))
        if native:
            stack.enter_context(patch.object(acceptance, '_inspect_native_case',
                                             side_effect=self.inspect_fixture_case))
            stack.enter_context(patch('current_general_native_display.validate_generation_environment',
                                      return_value=None))
        return stack


class CurrentGeneralVisualAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = None
        self.fresh()
        self.addCleanup(self.cleanup_fixture)

    def cleanup_fixture(self):
        if self.temporary is not None:
            self.temporary.cleanup()

    def fresh(self):
        self.cleanup_fixture()
        self.temporary = tempfile.TemporaryDirectory(prefix='scoped-visual-guard-')
        self.fixture = AcceptanceFixture(Path(self.temporary.name))
        return self.fixture

    def refused(self, result, message=None):
        self.assertEqual('failed', result['status'], result)
        self.assertTrue(result['errors'], result)
        if message:
            self.assertIn(message, '\n'.join(result['errors']), result)

    def evaluate(self, fixture=None, **kwargs):
        fixture = fixture or self.fixture
        with fixture.patches():
            return acceptance.evaluate(fixture.root, **kwargs)

    def test_real_different_pixels_and_finite_facts_accept_only_the_registered_result(self):
        f = self.fixture
        immutable_before = {row['path']: digest(f.root / row['path'])
                            for row in f.record['immutable_files']}
        result = self.evaluate()
        self.assertEqual('passed', result['status'], result)
        self.assertEqual(54, result['comparison_count'])
        self.assertEqual([1] * 54, [v['different_rgba_pixels'] for v in result['comparisons']])
        self.assertEqual(f.errors, result['retained_raw_errors'])
        self.assertEqual('failed_retained_unchanged', result['raw_bit_exact_status'])
        self.assertEqual('accepted_by_user', result['acceptance_status'])
        for field in ('new_product_capture_executed', 'native_verified', 'full_product_acceptance_promoted',
                      'release_acceptance_promoted', 'global_product_gate_passed'):
            self.assertIs(False, result[field], field)
        self.assertEqual(immutable_before, {p: digest(f.root / p) for p in immutable_before})
        self.assertEqual('failed', f.load(f.original_paths['native_receipt'])['status'])
        self.assertEqual('failed', f.load(f.original_paths['root_receipt'])['status'])

    def test_registered_record_and_original_user_decision_bytes_cannot_be_rebound(self):
        for relative in (acceptance.RECORD_PATH, acceptance.DECISION_PATH):
            f = self.fresh()
            with self.subTest(path=relative), f.patches():
                path = f.root / relative
                path.write_bytes(path.read_bytes() + b' ')
                self.refused(acceptance.evaluate(f.root), 'hash changed')

    def test_decision_scope_and_future_policy_fields_are_independently_checked(self):
        mutations = {
            'accepts_missing_content_or_layout_errors': True,
            'full_product_acceptance_promoted': True,
            'future_changed_source_or_artifact_automatically_accepted': True,
            'raw_comparison_threshold': 1,
            'source_tree_sha256': 'f' * 64,
            'artifact_sha256': 'f' * 64,
        }
        for key, value in mutations.items():
            f = self.fresh()
            decision = f.load(acceptance.DECISION_PATH)
            decision[key] = value
            f.put(acceptance.DECISION_PATH, decision)
            f.record['decision'] = f.reference(acceptance.DECISION_PATH)
            f.seal()
            with self.subTest(field=key):
                self.refused(self.evaluate(f))

    def test_unknown_changed_and_future_archives_never_inherit_acceptance(self):
        f = self.fixture
        unknown = f.root / '.build/future.zip'
        unknown.write_bytes(b'another package')
        with f.patches():
            for value in ('f' * 64, None, True, 123):
                self.assertFalse(acceptance.is_registered_artifact(value))
            self.assertTrue(acceptance.is_registered_artifact(f.artifact_sha))
            self.refused(acceptance.evaluate(f.root, '.build/future.zip'), 'Only exact accepted artifact')
            (f.root / f.artifact).write_bytes(b'changed original archive')
            self.refused(acceptance.evaluate(f.root), 'hash changed')

    def test_candidate_snapshot_must_recompute_and_current_product_source_cannot_drift(self):
        for mutation in ('changed', 'deleted', 'new-product', 'new-unknown', 'snapshot'):
            f = self.fresh()
            production = f.root / 'apps/desktop/src/accepted-product.ts'
            if mutation == 'changed':
                production.write_text('export const acceptedSource = 2;\n')
            elif mutation == 'deleted':
                production.unlink()
            elif mutation == 'new-product':
                production.with_name('new-product.ts').write_text('export const injected = true;\n')
            elif mutation == 'new-unknown':
                (f.root / 'unknown-new-source.py').write_text('print(1)\n')
            else:
                snapshot = f.load(f.snapshot_path)
                snapshot['files']['apps/desktop/src/accepted-product.ts'] = 'e' * 64
                f.put(f.snapshot_path, snapshot)
                f.record['source_snapshot'] = f.reference(f.snapshot_path)
                f.seal()
            with self.subTest(mutation=mutation):
                self.refused(self.evaluate(f), 'source' if mutation == 'snapshot' else 'outside accepted QA/document')

    def test_only_the_explicit_qa_increment_is_allowed(self):
        f = self.fixture
        allowed = f.root / 'scripts/quality/test_current_general_visual_acceptance.py'
        allowed.write_text('# Explicitly admitted QA-only source addition.\n')
        result = self.evaluate()
        self.assertEqual('passed', result['status'], result)
        f.record['allowed_current_delta_paths'].append('apps/desktop/src/accepted-product.ts')
        f.seal()
        self.refused(self.evaluate(), 'cannot enlarge')

    def test_missing_duplicate_unknown_and_reordered_case_sets_are_rejected(self):
        for source in ('accepted', 'candidate', 'reference'):
            for mutation in ('missing', 'duplicate', 'unknown', 'reordered'):
                f = self.fresh()
                if source == 'accepted':
                    rows = f.record['observations']
                else:
                    name = 'native_report' if source == 'candidate' else 'reference_generation'
                    original = f.load(f.original_paths[name])
                    rows = original['observations' if source == 'candidate' else 'captures']
                if mutation == 'missing':
                    rows.pop()
                elif mutation == 'duplicate':
                    rows[-1] = copy.deepcopy(rows[0])
                elif mutation == 'unknown':
                    rows[-1]['case'] = {**rows[-1]['case'], 'state_id': 'unknown-general-state'}
                else:
                    rows[0], rows[1] = rows[1], rows[0]
                if source != 'accepted':
                    f.put(f.original_paths[name], original)
                # Do not regenerate the reference catalogue from an intentionally
                # invalid accepted case; only register the mutated record bytes.
                if source == 'accepted':
                    write_json(f.root / acceptance.RECORD_PATH, f.record)
                else:
                    f.seal()
                with self.subTest(source=source, mutation=mutation):
                    self.refused(self.evaluate(f), 'closure')

    def test_missing_duplicate_and_reordered_original_frame_slots_are_rejected(self):
        for mutation in ('missing', 'duplicate', 'reordered', 'foreign-directory'):
            f = self.fresh()
            row = f.record['observations'][0]
            relative = row['candidate_raw_directory'] + '/capture.json'
            capture = f.load(relative)
            if mutation == 'missing':
                capture['frames'].pop()
            elif mutation == 'duplicate':
                capture['frames'][1] = copy.deepcopy(capture['frames'][0])
            elif mutation == 'reordered':
                capture['frames'].reverse()
            else:
                row['candidate_raw_directory'] = f.record['observations'][1]['candidate_raw_directory']
            f.put(relative, capture)
            f.seal()
            with self.subTest(mutation=mutation):
                self.refused(self.evaluate(f))

    def test_png_byte_or_alpha_changes_are_rejected_even_at_the_same_path(self):
        for alpha in (False, True):
            f = self.fresh()
            path = f.root / (f.observations[0]['candidate_raw_directory'] + '/frame-1.png')
            with self.subTest(alpha=alpha), f.patches():
                with Image.open(path) as image:
                    image.putpixel((0, 0), (11, 20, 30, 254) if alpha else (12, 20, 30, 255))
                    image.save(path)
                self.refused(acceptance.evaluate(f.root), 'hash changed')

    def test_corrupt_resized_and_new_pixel_differences_fail_real_png_recomputation(self):
        for mutation in ('corrupt', 'dimensions', 'pixels', 'reference-calibration'):
            f = self.fresh()
            row = f.observations[0]
            path = f.root / (row['candidate_raw_directory'] + '/frame-1.png')
            if mutation == 'corrupt':
                path.write_bytes(b'not a PNG\x00')
            elif mutation == 'dimensions':
                Image.new('RGBA', (999, 720), (10, 20, 30, 255)).save(path)
            elif mutation == 'pixels':
                with Image.open(path) as image:
                    image.putpixel((1, 0), (11, 20, 30, 255))
                    image.save(path)
            else:
                path = f.root / (row['reference_raw_directory'] + '/frame-2.png')
                with Image.open(path) as image:
                    image.putpixel((1, 0), (11, 20, 30, 255))
                    image.save(path)
            f.seal()
            with self.subTest(mutation=mutation):
                self.refused(self.evaluate(f))

    def test_geometry_and_visible_semantics_are_recomputed_after_valid_hash_registration(self):
        mutations = {
            'geometry': lambda value: value['facts']['geometry']['title'].update(x=201),
            'switch': lambda value: value['facts']['switches'][0].update(selected=False),
            'heading': lambda value: value['facts'].update(headings=['通用', '本机偏好']),
            'scope': lambda value: value['facts']['scope'].update(kind='game'),
            'runtime': lambda value: value['facts'].update(errors=['renderer failure']),
            'command': lambda value: value['facts'].update(controlledCommands=[{'kind': 'launch_game'}]),
        }
        for name, mutate in mutations.items():
            f = self.fresh()
            relative = f.observations[0]['candidate_raw_directory'] + '/observation.json'
            value = f.load(relative)
            mutate(value['observation'])
            f.put(relative, value)
            f.seal()
            with self.subTest(mutation=name):
                self.refused(self.evaluate(f))

    def test_nonpixel_inspection_command_and_original_outcomes_cannot_be_waived(self):
        mutations = (
            ('native_validation', 'inspection', ['Actual Native ACL failure']),
            ('native_execution', 'exit_code', 1),
            ('native_execution', 'cwd', '/another/root'),
            ('native_receipt', 'status', 'passed'),
            ('root_receipt', 'status', 'passed'),
            ('react_validation', 'independent_recomputation', ['Original component mismatch']),
        )
        for original, field, replacement in mutations:
            f = self.fresh()
            relative = f.original_paths[original]
            value = f.load(relative)
            value[field] = replacement
            f.put(relative, value)
            f.seal()
            with self.subTest(original=original, field=field):
                self.refused(self.evaluate(f))

    def test_claimed_pixel_error_strings_cannot_replace_the_actual_54_differences(self):
        for replacement in ([], ['passed'], ['General original packaged_native forged: 1 RGBA pixels differ (required 0)'] * 54):
            f = self.fresh()
            for name, key in (('native_receipt', 'errors'), ('native_validation', 'independent_recomputation')):
                relative = f.original_paths[name]
                value = f.load(relative)
                value[key] = replacement
                f.put(relative, value)
            f.seal()
            self.refused(self.evaluate(f), 'exact retained Native failure')

    def test_immutable_closure_rejects_omitted_raw_files_duplicate_entries_and_new_sidecars(self):
        for mutation in ('omitted', 'duplicate', 'new-sidecar'):
            f = self.fresh()
            if mutation == 'omitted':
                relative = f.observations[0]['candidate_raw_directory'] + '/observation.json'
                f.record['immutable_files'] = [row for row in f.record['immutable_files'] if row['path'] != relative]
            elif mutation == 'duplicate':
                f.record['immutable_files'].append(copy.deepcopy(f.record['immutable_files'][0]))
            else:
                (f.root / f.observations[0]['candidate_raw_directory'] / 'unregistered.json').write_text('{}\n')
            write_json(f.root / acceptance.RECORD_PATH, f.record)
            with self.subTest(mutation=mutation):
                self.refused(self.evaluate(f), 'closure')

    def test_duplicate_keys_and_nonfinite_json_are_rejected_after_hash_validation(self):
        for payload in ('{"scope":1,"scope":2}', '{"value":NaN}', '{"value":Infinity}',
                        '{"value":-Infinity}', '{"value":1e999}'):
            f = self.fresh()
            relative = f.observations[0]['candidate_raw_directory'] + '/extra.json'
            path = f.root / relative
            path.write_text(payload)
            f.immutable_paths.add(relative)
            f.seal()
            with self.subTest(payload=payload):
                self.refused(self.evaluate(f))

    def test_unsafe_evidence_paths_and_symlinks_are_rejected(self):
        f = self.fixture
        for relative in ('../escape', '/tmp/escape', '.build/../escape', '.build//double',
                         '.build/./dot', '.build/back\\slash', '.build/question?',
                         '.build/fragment#', '.build/colon:', '.build/trailing '):
            with self.subTest(path=relative), self.assertRaises((ValueError, OSError)):
                acceptance._path(f.root, relative)
        target = f.root / '.build/fixture-originals/native_validation.json'
        link = f.root / '.build/linked-original.json'
        link.symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'Symlink|symlink'):
            acceptance._ref(f.root, {'path': link.relative_to(f.root).as_posix(), 'sha256': digest(target)})
        source_link = f.root / 'scripts/quality/test_current_general_visual_acceptance.py'
        source_link.symlink_to(f.root / 'scripts/quality/current_general_visual_acceptance.py')
        self.refused(self.evaluate(), 'Symlink')

    def test_real_original_native_parser_rejects_bad_json_and_nonpixel_fields_before_package_boundary(self):
        f = self.fixture
        relative = f.observations[0]['candidate_raw_directory']
        path = f.root / relative / 'capture.json'
        original = path.read_bytes()
        for payload in (b'{"package":{},"package":{}}', b'{"package":{},"value":NaN}'):
            path.write_bytes(payload)
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                acceptance._inspect_native_case(f.root, relative, f.cases[0], 'candidate', acceptance._Batch())
        for field, replacement in (('status', 'failed'), ('errors', ['ACL failure']),
                                   ('side', 'reference'), ('game_tests_executed', True),
                                   ('state_id', 'unknown')):
            path.write_bytes(original)
            value = json.loads(original)
            value[field] = replacement
            write_json(path, value)
            with self.subTest(field=field), patch.object(acceptance, '_inspect_package_batch') as package:
                with self.assertRaises(ValueError):
                    acceptance._inspect_native_case(f.root, relative, f.cases[0], 'candidate', acceptance._Batch())
                package.assert_not_called()

    def test_dispatch_creates_fresh_receipt_preserves_raw_failures_and_refuses_replay(self):
        f = self.fixture
        before = {name: (f.root / path).read_bytes() for name, path in f.original_paths.items()}
        with f.patches():
            first = acceptance.dispatch(f.root, '.build/new-evaluation', f.artifact)
            self.assertEqual('passed', first['status'], first)
            self.assertNotEqual('a' * 32, first['execution_nonce'])
            receipt = acceptance.load(f.root / first['receipt'])
            invocation = acceptance.load(f.root / first['invocation'])
            layer, states = acceptance.inspect_receipt(f.root, receipt, f.artifact_sha, invocation)
            self.assertEqual('packaged_native', layer)
            self.assertEqual(frozenset(STATE_IDS), states)
            self.refused(acceptance.dispatch(f.root, '.build/new-evaluation', f.artifact), 'output exists')
            replay = copy.deepcopy(receipt)
            replay['execution_nonce'] = 'b' * 32
            with self.assertRaises(ValueError):
                acceptance.inspect_receipt(f.root, replay, f.artifact_sha, invocation)
            invocation['execution_nonce'] = 'c' * 32
            with self.assertRaises(ValueError):
                acceptance.inspect_receipt(f.root, receipt, f.artifact_sha, invocation)
            with patch.object(acceptance.secrets, 'token_hex', return_value='a' * 32):
                output = '.build/reused-original-capture-nonce'
                self.refused(acceptance.dispatch(f.root, output, f.artifact))
                report = acceptance.load(f.root / output / 'acceptance-evaluation.json')
                self.refused(report, 'Original capture nonce cannot become a fresh evaluator execution')
        self.assertEqual(before, {name: (f.root / path).read_bytes() for name, path in f.original_paths.items()})

    def test_receipt_independent_reinspection_rejects_changed_raw_fact_and_false_product_claim(self):
        f = self.fixture
        with f.patches():
            result = acceptance.dispatch(f.root, '.build/receipt-reinspection', f.artifact)
            self.assertEqual('passed', result['status'], result)
            receipt = acceptance.load(f.root / result['receipt'])
            for field, value in (('schema_version', True), ('native_verified', True), ('new_product_capture_executed', True),
                                 ('state_ids', sorted(STATE_IDS)[:-1]), ('assertion_count', True)):
                changed = copy.deepcopy(receipt)
                changed[field] = value
                with self.subTest(field=field), self.assertRaises(ValueError):
                    acceptance.inspect_receipt(f.root, changed, f.artifact_sha)
            path = f.root / f.observations[0]['candidate_raw_directory'] / 'observation.json'
            path.write_bytes(path.read_bytes() + b' ')
            with self.assertRaises(ValueError):
                acceptance.inspect_receipt(f.root, receipt, f.artifact_sha)

    def test_registered_native_dispatch_consumes_real_evaluation_and_keeps_global_errors(self):
        import current_design_alignment as alignment
        import verify
        f = self.fixture
        global_errors = ['Unrelated all-page mapping remains incomplete']
        self.assertIn(acceptance.CHECK_ID, verify.CHECK_IDS)
        self.assertEqual(('python', 'test_current_general_visual_acceptance'),
                         verify.TEST_CHECKS['general-visual-acceptance-tests'])
        with f.patches(), patch.object(alignment, 'validate_contract', return_value=global_errors):
            result = alignment.run_current_adapter(f.root, alignment.GENERAL_NATIVE_CHECK,
                                                   '.build/registered-native-dispatch', f.artifact)
            self.assertEqual('passed', result['status'], result)
            self.assertEqual('accepted_by_user', result['acceptance_status'])
            self.assertEqual(global_errors, result['global_contract_errors'])
            self.assertIs(False, result['global_product_gate_passed'])
            self.assertIs(False, result['product_packaged_native_visual_tests_executed'])
            receipt = acceptance.load(f.root / result['receipt'])
            invocation = acceptance.load(f.root / result['invocation'])
            layer, states = alignment._inspect_receipt(f.root, receipt, f.artifact_sha, invocation)
            self.assertEqual(('packaged_native', frozenset(STATE_IDS)), (layer, states))

    def test_nonmatching_package_uses_original_strict_collector_and_cannot_consume_user_acceptance(self):
        from dataclasses import replace
        import current_design_alignment as alignment
        f = self.fixture
        foreign = f.root / '.build/new-unaccepted.zip'
        foreign.write_bytes(b'future unaccepted package')

        def original_collector(root, context):
            raise ValueError('original strict Native collector reached')

        adapter = replace(alignment.ADAPTERS[alignment.GENERAL_NATIVE_CHECK], execute=original_collector)
        with f.patches(), patch.object(alignment, 'validate_contract', return_value=[]), \
                patch.dict(alignment.ADAPTERS, {alignment.GENERAL_NATIVE_CHECK: adapter}), \
                patch('current_general_reference.reference_gaps', return_value=[]), \
                patch.object(acceptance, 'dispatch', side_effect=AssertionError('unaccepted package entered acceptance')):
            result = alignment.run_current_adapter(f.root, alignment.GENERAL_NATIVE_CHECK,
                '.build/unaccepted-native-dispatch', foreign.relative_to(f.root).as_posix())
            self.refused(result, 'original strict Native collector reached')
        row = f.observations[0]
        self.assertEqual(1, pixels(f.root / row['reference_raw_directory'] / 'frame-1.png',
                                  f.root / row['candidate_raw_directory'] / 'frame-1.png')['different_rgba_pixels'])

    def test_dispatch_rejects_unsafe_output_and_invalid_lifecycle_without_reusing_a_run(self):
        f = self.fixture
        with f.patches():
            for number, lifecycle in enumerate((True, {}, {'execution_nonce': True, 'started_at': '2026-11-08T00:00:00Z'},
                    {'execution_nonce': 'd' * 32, 'started_at': '2999-01-01T00:00:00Z'},
                    {'execution_nonce': 'd' * 32, 'started_at': '2026-11-08T00:00:00'})):
                with self.subTest(lifecycle=lifecycle):
                    self.refused(acceptance.dispatch(f.root, f'.build/invalid-lifecycle-{number}', f.artifact,
                                                     lifecycle_context=lifecycle))
            for output in ('.build/../escape', '.build/double//slash', '.build/./dot', 'reports/not-build'):
                with self.subTest(output=output):
                    self.refused(acceptance.dispatch(f.root, output, f.artifact))


if __name__ == '__main__':
    unittest.main()
