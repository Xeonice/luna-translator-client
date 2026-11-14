"""Adversarial RW coverage tests using isolated roots and a real Node process.

The small producer here tests the guard protocol, never a product feature or GUI.
The separately registered design adapter executes the actual browser assertions.
"""
from __future__ import annotations

import base64
import copy
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import struct
import zlib
import tempfile
import time
import unittest

from gate_core import digest, source_files
from interaction_coverage import (
    ACCEPTANCE, ADAPTER_ID, ARCHITECTURE_REQUIREMENTS, DESIGN_SCRIPT, EVIDENCE_REQUIREMENTS, LAYERS, STATE_TAG_ROLE,
    REGISTRY, REQUIRED_DIMENSIONS, _identity, acceptance_rows,
    design_capabilities, release_coverage_gaps, required_layers,
    run_design_coverage, validate_design_receipt, validate_registry,
)

ROOT = Path(__file__).resolve().parents[2]
def _png_chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


PNG = b'\x89PNG\r\n\x1a\n' + _png_chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0)) + _png_chunk(b'IDAT', zlib.compress(b'\0\0\0\0\xff')) + _png_chunk(b'IEND', b'')


class InteractionCoverageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='rw-coverage-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        for name in (ACCEPTANCE, 'docs/architecture/engineering-modules.csv', 'docs/architecture/engineering-components.csv'):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, path)
        design = self.root / 'docs/design/first-product-v1/index.html'
        design.parent.mkdir(parents=True)
        design.write_text("<!doctype html><title>Protocol guard fixture</title><script>const scenario='components';if(scenario==='components'){};</script>")
        cases = acceptance_rows(self.root)
        applicable = [row['case_id'] for row in cases if 'design' in required_layers(row['case_id'])]
        self.capabilities = {'fixture_actual_assertions': {'dimensions': sorted(REQUIRED_DIMENSIONS)}}
        calls = "await check('fixture_actual_assertions', " + repr(applicable) + ", async () => { assert.equal(2 + 2, 4); });"
        script = ''''use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const assert=require('node:assert/strict');
/* RW_CAPABILITIES_BEGIN */
const DESIGN_CAPABILITIES = @@CAPABILITY_JSON@@;
/* RW_CAPABILITIES_END */
const start=new Date().toISOString();
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const manifest=JSON.parse(fs.readFileSync(process.env.LUNA_DESIGN_SOURCE_MANIFEST_JSON,'utf8'));
const sources=()=>Object.fromEntries(Object.keys(manifest.files).map(p=>[p,hash(p)]));
const before=sources();
let count=0;const checks=[];
async function check(id,requirements,action){await action();count++;checks.push({id,requirements,status:'passed',assertion_count:1,observations:[{kind:'assertion',method:'equal',actual:4,expected:4},...DESIGN_CAPABILITIES[id].dimensions.map(dimension=>({kind:'observation',dimension,value:{actual:4,expected:4}}))],capture_ids:['fixture-capture'],dimensions:DESIGN_CAPABILITIES[id].dimensions});}
(async()=>{
CALLS
const out=path.join(process.argv[2],'unique-run');fs.mkdirSync(out,{recursive:true});
fs.writeFileSync(path.join(out,'image.png'),Buffer.from('PNG_BASE64','base64'));
const report={schema_version:2,execution_nonce:process.env.LUNA_DESIGN_EXECUTION_NONCE,started_at:start,finished_at:new Date().toISOString(),source_tree_sha256_before:process.env.LUNA_DESIGN_SOURCE_TREE_SHA256,source_tree_sha256_after:process.env.LUNA_DESIGN_SOURCE_TREE_SHA256,source_files_sha256_before:before,source_files_sha256_after:sources(),qa_script_sha256:hash(__filename),coverage_registry_sha256:hash('REGISTRY_PATH'),design_only:true,product_regression_executed:false,real_game:false,native:false,global_hotkey:false,tokenizer:false,live_capture:false,status:'passed',artifact_qa_passed:true,checks,screenshots:[{id:'fixture-capture',path:'image.png',sha256:hash(path.join(out,'image.png')),check_ids:['fixture_actual_assertions']}]};
fs.writeFileSync(path.join(out,'verification.json'),JSON.stringify(report));process.stdout.write(JSON.stringify({output:out,count,verification_sha256:hash(path.join(out,'verification.json'))})+'\\n');
})().catch(error=>{console.error(error);process.exitCode=1;});
'''.replace('@@CAPABILITY_JSON@@', json.dumps(self.capabilities)).replace('CALLS', calls).replace('PNG_BASE64', base64.b64encode(PNG).decode()).replace('REGISTRY_PATH', REGISTRY)
        path = self.root / DESIGN_SCRIPT
        path.parent.mkdir(parents=True)
        path.write_text(script)
        registry = {'schema_version': 1, 'acceptance_source': ACCEPTANCE, 'design_script': DESIGN_SCRIPT,
                    'state_tags_role': STATE_TAG_ROLE, 'evidence_layers': list(LAYERS), 'required_dimensions': sorted(REQUIRED_DIMENSIONS), 'cases': []}
        for source in cases:
            applicable_layers = required_layers(source['case_id'])
            layers = {}
            for layer in LAYERS:
                applicable_layer = layer in applicable_layers
                layers[layer] = {'applicable': applicable_layer, 'reason': 'Explicit guard fixture applicability',
                                 'adapter_checks': [ADAPTER_ID] if layer == 'design' and applicable_layer else [],
                                 'required_evidence': sorted(EVIDENCE_REQUIREMENTS[layer]) if applicable_layer else [],
                                 'status': 'not_run' if applicable_layer else 'not_applicable'}
            minimum_modules, minimum_components = next((modules, components) for numbers, modules, components in ARCHITECTURE_REQUIREMENTS if int(source['case_id'].split('-')[1]) in numbers)
            registry['cases'].append({'id': source['case_id'], 'source_requirements': source['source_requirements'].split('/'),
                                     'design_expectation': source['expected'], 'design_states': ['normal', 'failure', 'cancellation'],
                                     'design_refs': ['docs/design/first-product-v1/index.html?scenario=components'],
                                     'module_ids': sorted(minimum_modules), 'component_ids': sorted(minimum_components), 'owner': 'UI/QA',
                                     'design_dimensions': sorted(REQUIRED_DIMENSIONS) if 'design' in applicable_layers else [],
                                     'design_checks': ['fixture_actual_assertions'] if 'design' in applicable_layers else [], 'layers': layers})
        self.registry = registry
        self.write_registry(registry)

    def write_registry(self, registry):
        path = self.root / REGISTRY
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(registry), encoding='utf-8')

    def row(self, case_id='RW-28'):
        return next(row for row in self.registry['cases'] if row['id'] == case_id)

    def assert_bad_registry(self, contains):
        self.write_registry(self.registry)
        errors = validate_registry(self.root)
        self.assertTrue(errors)
        self.assertTrue(any(contains in error for error in errors), errors)

    def valid_receipt(self):
        directory = self.root / '.build/evidence/design-run'
        directory.mkdir(parents=True, exist_ok=True)
        (directory / 'image.png').write_bytes(PNG)
        before = source_files(self.root)
        now = time.time()
        started, completed = now - 1, now + 1
        timestamp = datetime.fromtimestamp(now, timezone.utc).isoformat()
        receipt = {'schema_version': 2, 'execution_nonce': 'fresh-unique-nonce-12345', 'started_at': timestamp, 'finished_at': timestamp,
                   'source_tree_sha256_before': _identity(before), 'source_tree_sha256_after': _identity(before),
                   'source_files_sha256_before': before, 'source_files_sha256_after': before,
                   'qa_script_sha256': digest(self.root / DESIGN_SCRIPT), 'coverage_registry_sha256': digest(self.root / REGISTRY),
                   'design_only': True, 'product_regression_executed': False,
                   'real_game': False, 'native': False, 'global_hotkey': False, 'tokenizer': False, 'live_capture': False,
                   'status': 'passed', 'artifact_qa_passed': True,
                   'checks': [{'id': key, 'requirements': value['requirements'], 'status': 'passed', 'assertion_count': 1,
                               'observations': [{'kind': 'assertion', 'method': 'equal', 'actual': 4, 'expected': 4}] + [{'kind': 'observation', 'dimension': dimension, 'value': {'actual': 4}} for dimension in value['dimensions']], 'capture_ids': ['a'], 'dimensions': value['dimensions']}
                              for key, value in design_capabilities(self.root).items()],
                   'screenshots': [{'id': 'a', 'path': 'image.png', 'sha256': digest(directory / 'image.png'), 'check_ids': ['fixture_actual_assertions']}]}
        artifacts = {}
        for name in ('stdout', 'stderr', 'log'):
            path = directory / name
            path.write_text('actual protocol fixture output' if name != 'stderr' else '')
            artifacts[name] = {'path': str(path.relative_to(self.root)), 'sha256': digest(path)}
        manifest = directory / 'source-manifest.json'
        manifest.write_text(json.dumps({'files': before}))
        process = {'argv': ['node', DESIGN_SCRIPT, str(directory)], 'output_directory': str(directory), 'exit_code': 0,
                   'source_manifest': {'path': str(manifest.relative_to(self.root)), 'sha256': digest(manifest)},
                   'execution_nonce': receipt['execution_nonce'], 'source_tree_sha256': _identity(before),
                   'started_at_epoch': started, 'completed_at_epoch': completed, 'artifacts': artifacts}
        path = directory / 'verification.json'
        arguments = dict(nonce=receipt['execution_nonce'], started=started, completed=completed, source_before=before, process=process)
        return path, receipt, arguments

    def validate(self, receipt, path, arguments):
        path.write_text(json.dumps(receipt), encoding='utf-8')
        if 'stdout' in arguments['process'].get('artifacts', {}):
            artifact = arguments['process']['artifacts']['stdout']
            stdout = self.root / artifact['path']
            stdout.write_text(json.dumps({'output': str(path.parent), 'verification_sha256': digest(path)}))
            artifact['sha256'] = digest(stdout)
        return validate_design_receipt(self.root, path, **arguments)

    def test_complete_authoritative_csv_mapping_and_closed_layers(self):
        self.assertEqual(validate_registry(self.root), [])
        self.assertEqual(set(row['id'] for row in self.registry['cases']), set(row['case_id'] for row in acceptance_rows(self.root)))

    def test_missing_case_rejected(self):
        self.registry['cases'].pop()
        self.assert_bad_registry('every authoritative CSV')

    def test_new_authoritative_case_without_mapping_rejected(self):
        with (self.root / ACCEPTANCE).open('a') as stream:
            stream.write('RW-35,FP-12,new behavior,observable required,not_run\n')
        self.assert_bad_registry('every authoritative CSV')

    def test_duplicate_case_rejected(self):
        self.registry['cases'].append(copy.deepcopy(self.registry['cases'][0]))
        self.assert_bad_registry('exactly once')

    def test_missing_layer_rejected(self):
        del self.row()['layers']['real_language']
        self.assert_bad_registry('every closed evidence layer')

    def test_renamed_layer_rejected(self):
        self.registry['evidence_layers'][-1] = 'design_games'
        self.assert_bad_registry('closed')

    def test_disabling_required_layer_rejected(self):
        self.row()['layers']['real_language']['applicable'] = False
        self.assert_bad_registry('cannot be weakened')

    def test_design_adapter_cannot_substitute_native(self):
        self.row('RW-07')['layers']['native']['adapter_checks'] = [ADAPTER_ID]
        self.assert_bad_registry('cross-layer substitute')

    def test_unknown_runtime_adapter_rejected(self):
        self.row('RW-27')['layers']['real_language']['adapter_checks'] = ['worker-script-exists']
        self.assert_bad_registry('unknown adapter')

    def test_missing_required_evidence_rejected(self):
        self.row('RW-18')['layers']['real_pipeline']['required_evidence'].remove('persisted_sequence')
        self.assert_bad_registry('required evidence contract')

    def test_planning_passed_is_not_evidence(self):
        self.row()['layers']['design']['status'] = 'passed'
        self.assert_bad_registry('self-declare')

    def test_missing_unknown_or_duplicate_design_check_rejected(self):
        for values in ([], ['invented-check'], ['fixture_actual_assertions', 'fixture_actual_assertions']):
            with self.subTest(values=values):
                self.row()['design_checks'] = values
                self.assert_bad_registry('design check' if values != ['fixture_actual_assertions', 'fixture_actual_assertions'] else 'duplicate')

    def test_removed_behavior_dimension_rejected(self):
        self.registry['required_dimensions'].remove('lexical-001')
        self.assert_bad_registry('Mandatory behavior dimensions')

    def test_code_capability_dimensions_cannot_be_weakened(self):
        path = self.root / DESIGN_SCRIPT
        path.write_text(path.read_text().replace('"lexical-001", ', ''))
        self.assert_bad_registry('omits required dimensions')

    def test_code_capability_call_closure_rejects_registration_without_call(self):
        path = self.root / DESIGN_SCRIPT
        path.write_text(path.read_text().replace('"fixture_actual_assertions": {', '"never_executed": {', 1))
        self.assert_bad_registry('actual code-registered check calls')

    def test_owner_modules_components_and_states_required(self):
        for key, value, message in [('owner', '', 'Owner'), ('module_ids', ['M99'], 'module_ids'), ('component_ids', [], 'component_ids'), ('design_states', [], 'states/combinations')]:
            with self.subTest(key=key):
                old = self.row()[key]
                self.row()[key] = value
                self.assert_bad_registry(message)
                self.row()[key] = old

    def test_design_ref_cannot_escape_authorized_root(self):
        self.row()['design_refs'] = ['../../hidden.html?scenario=x']
        self.assert_bad_registry('invalid design reference')

    def test_design_expectation_cannot_replace_authoritative_requirement(self):
        self.row()['design_expectation'] = 'merely show a switch'
        self.assert_bad_registry('observable expectation')

    def test_valid_design_receipt_inspected_independently(self):
        path, receipt, args = self.valid_receipt()
        self.assertEqual(self.validate(receipt, path, args), [])

    def test_old_nonce_is_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['execution_nonce'] = 'old-unique-nonce-67890'
        self.assertTrue(any('nonce' in error for error in self.validate(receipt, path, args)))

    def test_old_source_or_source_drift_rejected(self):
        for mutation in ('receipt', 'source'):
            with self.subTest(mutation=mutation):
                path, receipt, args = self.valid_receipt()
                if mutation == 'receipt':
                    receipt['source_tree_sha256_before'] = '0' * 64
                else:
                    (self.root / 'new-unreviewed-source.txt').write_text('unexpected source')
                self.assertTrue(any('source' in error for error in self.validate(receipt, path, args)))

    def test_stale_receipt_timestamp_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['finished_at'] = '2001-01-01T00:00:00Z'
        self.assertTrue(any('stale' in error for error in self.validate(receipt, path, args)))

    def test_timezone_free_timestamp_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['started_at'] = '2026-10-19T00:00:00'
        self.assertTrue(any('timezone' in error for error in self.validate(receipt, path, args)))

    def test_zero_assertions_or_boolean_count_rejected(self):
        for count in (0, False, -1, 1.0):
            with self.subTest(count=count):
                path, receipt, args = self.valid_receipt()
                receipt['checks'][0]['assertion_count'] = count
                self.assertTrue(any('nonzero' in error for error in self.validate(receipt, path, args)))

    def test_missing_observable_results_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['observations'] = []
        receipt['checks'][0]['capture_ids'] = []
        receipt['screenshots'] = []
        self.assertTrue(any('cannot be absent' in error for error in self.validate(receipt, path, args)))

    def test_missing_unknown_duplicate_checks_rejected(self):
        for mode in ('missing', 'unknown', 'duplicate'):
            with self.subTest(mode=mode):
                path, receipt, args = self.valid_receipt()
                if mode == 'missing': receipt['checks'] = []
                elif mode == 'unknown': receipt['checks'][0]['id'] = 'not-run-check'
                else: receipt['checks'].append(copy.deepcopy(receipt['checks'][0]))
                self.assertTrue(any('closed code capabilities' in error for error in self.validate(receipt, path, args)))

    def test_check_failure_not_overridden_by_top_level_passed(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['status'] = 'not_run'
        self.assertTrue(any('nonzero passed' in error for error in self.validate(receipt, path, args)))

    def test_design_receipt_cannot_claim_native_or_game(self):
        for field in ('native', 'tokenizer', 'live_capture', 'global_hotkey', 'real_game', 'product_regression_executed'):
            with self.subTest(field=field):
                path, receipt, args = self.valid_receipt()
                receipt[field] = True
                self.assertTrue(self.validate(receipt, path, args))

    def test_design_check_cannot_rename_layer(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['layer'] = 'native'
        self.assertTrue(any('evidence layer' in error for error in self.validate(receipt, path, args)))

    def test_missing_or_modified_screenshot_rejected(self):
        for mode in ('missing', 'hash', 'nonpng', 'escape', 'symlink'):
            with self.subTest(mode=mode):
                path, receipt, args = self.valid_receipt()
                image = path.parent / 'image.png'
                if mode == 'missing': image.unlink()
                elif mode == 'hash': receipt['screenshots'][0]['sha256'] = '0' * 64
                elif mode == 'nonpng': image.write_text('not an image'); receipt['screenshots'][0]['sha256'] = digest(image)
                elif mode == 'escape': receipt['screenshots'][0]['path'] = '../image.png'
                else:
                    image.unlink(); image.symlink_to('/etc/hosts')
                self.assertTrue(any('Invalid design capture' in error for error in self.validate(receipt, path, args)))
                if image.is_symlink(): image.unlink()

    def test_unrelated_screenshot_cannot_cover_check(self):
        path, receipt, args = self.valid_receipt()
        receipt['screenshots'][0]['check_ids'] = ['never-ran']
        self.assertTrue(any('association' in error or 'registered checks' in error for error in self.validate(receipt, path, args)))

    def test_process_must_have_actual_logs_and_correct_command(self):
        for mode in ('logs', 'command', 'exit', 'nonce'):
            with self.subTest(mode=mode):
                path, receipt, args = self.valid_receipt()
                if mode == 'logs': args['process']['artifacts'] = {}
                elif mode == 'command': args['process']['argv'][1] = 'echo passed'
                elif mode == 'exit': args['process']['exit_code'] = 1
                else: args['process']['execution_nonce'] = 'old'
                self.assertTrue(self.validate(receipt, path, args))

    def test_hashed_log_mutation_and_missing_full_source_map_rejected(self):
        path, receipt, args = self.valid_receipt()
        log = self.root / args['process']['artifacts']['log']['path']
        log.write_text('overwritten passed')
        receipt['source_files_sha256_after'] = {}
        errors = self.validate(receipt, path, args)
        self.assertTrue(any('artifact invalid' in error for error in errors))
        self.assertTrue(any('full source closure' in error for error in errors))

    def test_forged_passed_summary_cannot_clear_runtime_or_design(self):
        gaps = release_coverage_gaps(self.root, {'validated_by': ADAPTER_ID, 'status': 'passed', 'errors': [], 'native': 'passed'})
        self.assertTrue(any(row['layer'] == 'design' for row in gaps))
        self.assertTrue(any(row['case_id'] == 'RW-28' and row['layer'] == 'real_language' for row in gaps))
        self.assertTrue(any(row['case_id'] == 'RW-18' and row['layer'] == 'real_pipeline' for row in gaps))

    def test_real_node_process_roundtrip_preserves_runtime_gaps(self):
        self.assertIsNotNone(shutil.which('node'), 'Node executable is a required guard-test prerequisite')
        summary = run_design_coverage(self.root, self.root / '.build/actual-guard-run')
        self.assertEqual(summary['errors'], [], summary)
        self.assertEqual(summary['status'], 'passed')
        self.assertTrue(Path(self.root / summary['receipt']).is_file())
        self.assertEqual(set(summary['runtime_layers'].values()), {'not_run'})
        self.assertFalse(any(row['layer'] == 'design' for row in summary['release_gaps']))
        self.assertTrue(any(row['layer'] == 'native' for row in summary['release_gaps']))
        self.assertTrue(any(row['layer'] == 'real_language' for row in summary['release_gaps']))
        self.assertTrue(any(row['layer'] == 'real_pipeline' for row in summary['release_gaps']))
        self.assertEqual(release_coverage_gaps(self.root, summary), summary['release_gaps'])
        (self.root / summary['receipt']).write_text('{}')
        self.assertTrue(any(row['layer'] == 'design' for row in release_coverage_gaps(self.root, summary)))


    def test_malformed_registry_fails_closed_without_exception(self):
        for invalid in ([], {'cases': [None]}, dict(self.registry, cases=[{'id': []}])):
            with self.subTest(invalid=type(invalid).__name__):
                self.write_registry(invalid)
                self.assertTrue(validate_registry(self.root))

    def test_duplicate_json_keys_rejected(self):
        path = self.root / REGISTRY
        path.write_text('{"schema_version": 1, "schema_version": 1}')
        self.assertTrue(any('Duplicate JSON' in error for error in validate_registry(self.root)))


    def test_self_reported_assertion_count_without_records_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['assertion_count'] = 999
        self.assertTrue(any('match actual assertion' in error for error in self.validate(receipt, path, args)))

    def test_declared_dimension_without_observable_result_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['observations'] = [item for item in receipt['checks'][0]['observations'] if item.get('dimension') != 'lexical-001']
        self.assertTrue(any('lack actual observed' in error for error in self.validate(receipt, path, args)))


    def test_unknown_actual_scenario_or_anchor_rejected(self):
        for reference in ('docs/design/first-product-v1/index.html?scenario=does-not-exist', 'docs/design/first-product-v1/index.html?scenario=components#unknown-anchor'):
            with self.subTest(reference=reference):
                self.row()['design_refs'] = [reference]
                self.assert_bad_registry('scenario/anchor')

    def test_known_but_unrelated_module_and_component_rejected(self):
        self.row()['module_ids'] = ['M16']
        self.row()['component_ids'] = ['C30']
        self.assert_bad_registry('omits reviewed responsibility')

    def test_planning_tags_do_not_replace_actual_dimensions(self):
        self.row()['design_states'] = ['arbitrary planning note']
        self.row()['design_dimensions'] = []
        self.assert_bad_registry('actual code-bound design dimensions')

    def test_contradictory_assertion_operands_are_actually_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['observations'][0]['expected'] = 5
        self.assertTrue(any('assertion replay rejected' in error for error in self.validate(receipt, path, args)))

    def test_unknown_assertion_method_rejected(self):
        path, receipt, args = self.valid_receipt()
        receipt['checks'][0]['observations'][0]['method'] = 'console.log'
        self.assertTrue(any('known methods' in error for error in self.validate(receipt, path, args)))

    def test_png_signature_without_actual_image_is_rejected(self):
        path, receipt, args = self.valid_receipt()
        image = path.parent / 'image.png'
        image.write_bytes(b'\x89PNG\r\n\x1a\n')
        receipt['screenshots'][0]['sha256'] = digest(image)
        self.assertTrue(any('actual PNG decode rejected' in error for error in self.validate(receipt, path, args)))

    def test_receipt_rewrite_without_actual_stdout_binding_rejected(self):
        path, receipt, args = self.valid_receipt()
        self.assertEqual(self.validate(receipt, path, args), [])
        receipt['checks'][0]['observations'][0]['expected'] = 5
        path.write_text(json.dumps(receipt))
        self.assertTrue(any('actual process stdout verification hash' in error for error in validate_design_receipt(self.root, path, **args)))


    def test_literal_dynamic_session_anchor_is_resolved_but_unknown_is_not(self):
        design = self.root / 'docs/design/first-product-v1/index.html'
        design.write_text(design.read_text() + "<script>const activeId=view.tab==='logs'?'hook-events':'session-history';const html='<section id=\"'+activeId+'\">';</script>")
        self.row()['design_refs'] = ['docs/design/first-product-v1/index.html?scenario=components#session-history']
        self.write_registry(self.registry)
        self.assertEqual(validate_registry(self.root), [])
        self.row()['design_refs'] = ['docs/design/first-product-v1/index.html?scenario=components#logs']
        self.assert_bad_registry('scenario/anchor')


    def test_boolean_exit_code_cannot_claim_process_success(self):
        path, receipt, args = self.valid_receipt()
        args['process']['exit_code'] = False
        self.assertTrue(any('successful code-registered process' in error for error in self.validate(receipt, path, args)))

    def test_output_directory_cannot_be_rebound_outside_this_run(self):
        for outside in ('/outside-not-this-run', str(self.root / '.build/different-run')):
            with self.subTest(outside=outside):
                path, receipt, args = self.valid_receipt()
                args['process']['output_directory'] = outside
                args['process']['argv'][2] = outside
                self.assertTrue(any('output directory' in error for error in self.validate(receipt, path, args)))

    def test_manifest_and_logs_cannot_be_rebound_to_another_run(self):
        path, receipt, args = self.valid_receipt()
        other = self.root / '.build/unrelated'
        other.mkdir()
        manifest = other / 'source-manifest.json'
        manifest.write_text(json.dumps({'files': args['source_before']}))
        args['process']['source_manifest'] = {'path': str(manifest.relative_to(self.root)), 'sha256': digest(manifest)}
        self.assertTrue(any('different run' in error for error in self.validate(receipt, path, args)))

    def test_registry_schema_version_requires_real_integer(self):
        self.registry['schema_version'] = True
        self.assert_bad_registry('authoritative CSV')

    def test_runner_refuses_invalid_registry_before_spawn(self):
        self.row()['layers']['real_language']['status'] = 'passed'
        self.write_registry(self.registry)
        summary = run_design_coverage(self.root, self.root / '.build/invalid')
        self.assertEqual(summary['status'], 'failed')
        self.assertFalse((self.root / '.build/invalid').exists())


if __name__ == '__main__':
    unittest.main()
