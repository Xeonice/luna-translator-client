#!/usr/bin/env python3
"""Single executable entry point for specification, source, compiler and evidence gates."""
import argparse
import copy
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import time

from gate_core import check_baseline, check_debt, check_scope, digest, safe_path, source_files
from product_specs import check as check_product_specs
from spec_contract import validate_change, validate_release
from product_execution import PRODUCT_CAPTURE_CHECKS, PRODUCT_RUNTIME_CHECKS, PRODUCT_TEST_CHECKS, run_product_check
from reading_components import CHECK_ID as READING_COMPONENT_CHECK, run_reading_components

ROOT = Path(__file__).resolve().parents[2]
TEST_CHECKS = {
    'binding-phase-guard-tests': ('python', 'test_binding_phase_guard'),
    'binding-journey-guard-tests': ('python', 'test_binding_journey'),
    'storage-schema-v5-tests': ('python', 'test_storage_schema_v5'),
    'service-management-projection-tests': ('node', 'apps/desktop/tests/visual/service-management-projection.test.mjs'),
    'service-management-component-tests': ('node', 'apps/desktop/tests/visual/verify-service-management-list.mjs'),
    'engineering-gate-closure-tests': ('python', 'test_engineering_gate_closure'),
    'general-visual-acceptance-tests': ('python', 'test_current_general_visual_acceptance'),
    'general-visual-contract-tests': ('python', 'test_current_general_visual.GeneralVisualTests'),
    'general-settings-test-version-tests': ('python', 'test_verify_general_settings_supersession.VerifyGeneralSettingsSupersessionTests'),
    'general-settings-authority-tests': ('python', 'test_current_general_settings_authority.CurrentGeneralSettingsAuthorityTests'),
    'general-settings-structure-tests': ('node', 'apps/desktop/tests/live-preview/verify-general-settings-structure.mjs'),
    'live-ui-preview-tests': ('node', 'apps/desktop/tests/live-preview/verify-live-preview.mjs'),
    'reading-ui-alignment-tests': ('node', 'apps/desktop/tests/live-preview/verify-reading-ui-alignment.mjs'),
    'reading-typography-tests': ('node', 'apps/desktop/tests/live-preview/reading-typography.test.mjs'),
    'layout-supplement-authority-tests': ('python', 'test_current_layout_refinements_authority.CurrentLayoutRefinementsAuthorityTests'),
    'history-layout-refinement-tests': ('node', 'apps/desktop/tests/live-preview/verify-history-layout-refinement.mjs'),
    'settings-layout-refinement-tests': ('node', 'apps/desktop/tests/live-preview/verify-settings-layout-refinement.mjs'),
    'stopped-session-layout-tests': ('node', 'apps/desktop/tests/live-preview/verify-stopped-session-layout.mjs'),
    'select-zoom-containment-tests': ('node', 'apps/desktop/tests/live-preview/verify-select-zoom-containment.mjs'),
    'layout-refinements-design-tests': ('node', 'scripts/check_layout_refinements.mjs'),
    'hud-header-spacing-tests': ('node', 'apps/desktop/tests/live-preview/verify-hud-header-spacing.mjs'),
    'manual-ui-alignment-tests': ('node', 'apps/desktop/tests/live-preview/verify-manual-ui-alignment.mjs'),
    'settings-navigation-tests': ('node', 'apps/desktop/tests/live-preview/verify-settings-navigation.mjs'),
    'current-reference-catalogue-tests': ('python', 'test_current_reference_catalogue.CurrentReferenceCatalogueTests'),
    'current-design-react-guard-tests': ('python', 'test_current_design_react_adapter.CurrentDesignReactTests'),
    'current-preview-authority-tests': ('python', 'test_current_preview_authority.CurrentPreviewAuthorityTests'),
    'preview-visual-evidence-tests': ('python', 'test_preview_visual_evidence.PreviewVisualEvidenceTests'),
    'cp01-component-evidence-tests': ('python', 'test_cp01_component_evidence.CP01EvidenceTests'),
    'settings-controls-evidence-tests': ('python', 'test_settings_controls_evidence'),
    'source-inventory-tests': ('python', 'test_source_inventory.SourceInventoryTests'),
    'product-completion-tests': ('python', 'test_product_completion'),
    'render-evidence-tests': ('python', 'test_render_evidence'),
    'current-render-qualification-tests': ('python', 'test_current_design_render_qualification'),
    'reading-alignment-preflight-tests': ('python', 'test_reading_alignment_preflight.ReadingAlignmentPreflightTests'),
    'change-review-tests': ('python', 'test_change_review.ChangeReviewTests'),
    'history-design-tests': ('python', 'test_history_design.HistoryDesignGuards'),
    'design-authority-tests': ('python', 'test_design_authority'),
    'all-page-design-coverage-tests': ('python', 'test_all_page_design_coverage.AllPageDesignCoverageTests'),
    'documentation-contract-tests': ('python', 'test_documentation_contract.DocumentationContractTests'),
    'current-design-alignment-tests': ('python', 'test_current_design_alignment.CurrentDesignAlignmentTests'),
    'translation-service-selection-tests': ('python', 'test_translation_service_selection.TranslationServiceSelectionTests'),
    'flat-service-session-tests': ('python', 'test_flat_service_session.FlatServiceSessionTests'),
    'guided-service-reading-tests': ('python', 'test_guided_service_reading.GuidedServiceReadingTests'),
    'current-product-coverage-tests': ('python', 'test_current_product_coverage.CurrentProductCoverageTests'),
    'offline-translation-design-tests': ('python', 'test_offline_translation_design.OfflineTranslationDesignTests'),
    'model-service-design-docs-tests': ('python', 'test_model_service_design_docs.ModelServiceDesignDocsTests'),
    'lifecycle-impact-guard-tests': ('python', 'test_lifecycle_impact.LifecycleImpactGuards'),
    'runtime-stability-guard-tests': ('python', 'test_runtime_stability.RuntimeStabilityGuards'),
    'implementation-status-tests': ('python', 'test_implementation_status.ImplementationStatusTests'),
    'interaction-coverage-tests': ('python', 'test_interaction_coverage.InteractionCoverageTests'),
    'package-identity-tests': ('python', 'test_package_identity.PackageIdentityTests'),
    'product-execution-guard-tests': ('python', 'test_product_execution.ProductExecutionTests'),
    'product-v4-execution-guard-tests': ('python', 'test_product_capture_v4.ReviewedCaptureContractTests'),
    'design-source-version-node-tests': ('node', 'apps/desktop/tests/visual/design-source-versions.test.mjs'),
    'reading-component-guard-tests': ('python', 'test_reading_components.ReadingComponentGuards'),
    'generated-contract-tests': ('python-discover', 'tests/contracts'),
    'storage-schema-tests': ('python', 'test_storage_schema.StorageSchemaTests'),
    'storage-schema-v2-tests': ('python', 'test_storage_schema_v2.SchemaTwoGuards'),
    'storage-schema-v3-tests': ('python', 'test_storage_schema_v3.SchemaThreeGuards'),
    'storage-boundaries-v3-tests': ('python', 'test_storage_boundaries_v3.HttpBoundaryGuards'),
    'storage-acceptance-v3-tests': ('python', 'test_storage_acceptance_v3.HttpAcceptanceGuards'),
    'storage-schema-v4-tests': ('python', 'test_storage_schema_v4.SchemaFourGuards'),
    'storage-boundaries-v4-tests': ('python', 'test_storage_boundaries_v4.ServiceBoundaryGuards'),
    'storage-acceptance-v4-tests': ('python', 'test_storage_acceptance_v4.ServiceAcceptanceGuards'),
    'storage-acceptance-v2-tests': ('python', 'test_storage_acceptance_v2.SchemaTwoAcceptanceGuards'),
    'storage-boundary-tests': ('python', 'test_storage_boundaries.StorageBoundaryTests'),
    'storage-acceptance-tests': ('python', 'test_storage_acceptance.StorageAcceptanceTests'),
    'database-report-evidence-tests': ('python', 'test_engineering_report_evidence.ReportEvidenceTests'),
    'database-design-tests': ('python', 'test_database_design.DatabaseDesignTests'),
    'test-operations-contract-tests': ('python', 'test_test_operations_contract.TestOperationsContractTests'),
    'test-plan-export-tests': ('python', 'test_render_test_plan.TestPlanExportTests'),
    'test-plan-contract-tests': ('python', 'test_test_plan_contract.TestPlanContractTests'),
    'spec-contract-tests': ('python', 'test_spec_contract.ContractTests'),
    'release-evidence-tests': ('python', 'test_spec_contract.ReleaseTests'),
    'rust-boundary-tests': ('python', 'test_rust_boundaries.RustBoundariesTests'),
    'gate-unit-tests': ('python', 'test_gate_core.GatesTest'),
    'technology-contract-tests': ('python', 'test_technology_contract.TechnologyContractTests'),
    'frontend-boundary-tests': ('node', 'scripts/quality/frontend-boundaries.test.mjs'),
    'visual-guard-tests': ('node', 'scripts/quality/visual-compare.test.mjs'),
}


def binding_gate_phase_state(root):
    """Resolve the verified current epoch without reusing predecessor slots."""
    from binding_phase_guard import preimplementation, phase_receipts, path_at
    policy, plan, before = preimplementation(root)
    receipts = phase_receipts(policy)
    if path_at(root, receipts['implementation_admission']).exists():
        raise ValueError('P2-only execution cannot run after P3 write admission')
    # phase_receipts rejects aliases and pins the separate r3 receipt slots.
    output_root = Path(receipts['gates']).parent.as_posix()
    return policy, plan, before, output_root


def binding_contract_guard_fixture(root, policy):
    """Use sealed valid data for a guard, never old data as product evidence.

    The guard's unchanged setUp copies production files as fixture input. The
    admitted P3 draft intentionally has unresolved AST/adoption gaps; using it
    as a supposedly valid positive fixture conflates product and guard state.
    The original frozen Store4 archive preserves the exact R92 product data.
    Its separately attested probe is not an input to this guard.
    """
    from binding_phase_guard import path_at, ref, products
    from storage_schema_v5 import _verify_old_writer, _owned_path
    root = Path(root)
    relative = '.build/binding-journey-alignment-20261109/frozen-store4/reader-manifest.json'
    manifest_path = path_at(root, relative)
    reference = ref(root, relative)
    _verify_old_writer(root, manifest_path.parent, reference)
    manifest = json.loads(manifest_path.read_text())
    frozen_path = _owned_path(root, manifest['source_manifest_path'])
    frozen = json.loads(frozen_path.read_text())
    if products(frozen['files']) != policy['parent_product_source_map']:
        raise ValueError('Contract-only guard fixture must retain exact original R92 product bytes')
    return {'kind': 'sealed-contract-only-guard-fixture/v1',
            'source_root': str(_owned_path(root, manifest['source_root'])),
            'source_manifest': ref(root, frozen_path.relative_to(root).as_posix()),
            'reader_manifest': reference,
            'base_product_source_count': len(policy['parent_product_source_map']),
            'test_code': 'scripts/quality/test_current_design_alignment.py',
            'test_name': 'CurrentDesignAlignmentTests.test_actual_pure_tests_and_docs_are_contract_only_even_for_product_kind',
            'product_execution': False, 'current_product_gaps_discharged': False}


def run_binding_contract_fixture_guard(source_root, source_manifest):
    """Run current guard code on original data, keeping every old assertion."""
    import unittest
    import current_design_alignment as alignment
    import test_current_design_alignment as guard
    original = alignment.ROOT
    try:
        alignment.ROOT = Path(source_root)
        files = json.loads(Path(source_manifest).read_text())['files']
        guard.CurrentDesignAlignmentTests.setUpClass()
        # Do not let an unattested extra probe/new source enter the fixture.
        for relative in guard.CurrentDesignAlignmentTests.source_paths:
            path = safe_path(alignment.ROOT, relative)
            if relative not in files or hashlib.sha256(path.read_bytes()).hexdigest() != files[relative]:
                raise ValueError('Unsealed contract-only guard data input: ' + relative)
        suite = unittest.defaultTestLoader.loadTestsFromName(
            'test_current_design_alignment.CurrentDesignAlignmentTests.'
            'test_actual_pure_tests_and_docs_are_contract_only_even_for_product_kind')
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    finally:
        alignment.ROOT = original


def binding_gate_phase(args):
    """Generate limited P2 review inputs, preserving genuine semantic RED."""
    from binding_phase_guard import CHANGE, ref, tree_sha, run_guard_tests, products
    from binding_journey import execute as execute_binding, validate_registry
    if (args.change != CHANGE or args.profile != 'development' or args.self_test
            or any(getattr(args, key) for key in ('artifact', 'evidence', 'runtime_inputs', 'base',
                                                 'binding_native_evidence', 'binding_service_evidence',
                                                 'require_product_complete'))):
        raise ValueError('--binding-gate-phase is restricted to the named P2 change without release/live-product options')
    policy, _plan, before, output_root = binding_gate_phase_state(ROOT)
    registry_errors = validate_registry(ROOT)
    if registry_errors:
        raise ValueError('; '.join(registry_errors))
    run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    relative = output_root + '/root-' + run_id
    directory = ROOT / relative
    directory.mkdir(parents=True, exist_ok=False)
    executions, errors = [], []
    def actual(name, command, cwd, framework=None):
        start = datetime.now(timezone.utc).isoformat()
        process = subprocess.Popen(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE)
        timed_out = False
        try:
            stdout, stderr = process.communicate(timeout=600)
        except subprocess.TimeoutExpired:
            timed_out = True
            process.kill()
            stdout, stderr = process.communicate()
            errors.append(name + ': actual owned command timed out')
        result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
        (directory / (name + '.stdout.log')).write_text(result.stdout)
        (directory / (name + '.stderr.log')).write_text(result.stderr)
        record = {'name': name, 'command': command, 'cwd': str(Path(cwd).relative_to(ROOT)),
                  'started_at': start, 'finished_at': datetime.now(timezone.utc).isoformat(),
                  'exit_code': result.returncode, 'pid': process.pid, 'parent_pid': os.getpid(),
                  'timed_out': timed_out, 'stdout': ref(ROOT, relative + '/' + name + '.stdout.log'),
                  'stderr': ref(ROOT, relative + '/' + name + '.stderr.log')}
        if framework:
            count, problems = validate_test_output(framework, result.stdout + '\n' + result.stderr)
            record['test_count'] = count
            errors.extend(problems)
        if result.returncode:
            errors.append(name + ': actual command exited ' + str(result.returncode))
        executions.append(record)
        return result
    run_guard_tests(ROOT, relative + '/guards')
    affected = [sys.executable, '-m', 'unittest', '-v', 'test_binding_phase_guard',
                'test_binding_journey', 'test_product_completion', 'test_lifecycle_impact',
                'test_storage_schema_v5']
    fixture = None
    if policy.get('resume_epoch') is None:
        # The original phase did not have an admitted P3 draft. Preserve its
        # original data source and exact old test invocation.
        affected.append('test_current_design_alignment.CurrentDesignAlignmentTests.'
                        'test_actual_pure_tests_and_docs_are_contract_only_even_for_product_kind')
    actual('affected-guards', affected, ROOT / 'scripts/quality', 'python')
    if policy.get('resume_epoch') is not None:
        fixture = binding_contract_guard_fixture(ROOT, policy)
        (directory / 'contract-only-guard-fixture.json').write_text(json.dumps(fixture, indent=2) + '\n')
        script = ('import sys;from verify import run_binding_contract_fixture_guard;'
                  'sys.exit(run_binding_contract_fixture_guard(sys.argv[1],sys.argv[2]))')
        actual('current-design-contract-only-guard', [sys.executable, '-c', script,
               fixture['source_root'], str(ROOT / fixture['source_manifest']['path'])],
               ROOT / 'scripts/quality', 'python')
    # Reuse the real five-fault CLI runner in an invocation-owned output path.
    # Its assertions, mutation cases and actual subprocesses remain unchanged.
    five_script = ('import sys;from pathlib import Path;'
                   'sys.path.insert(0,str(Path.cwd()/"scripts/quality"));'
                   'import verify_five_scenarios as five;five.OUT=Path(sys.argv[1]);five.main()')
    five_output = directory / 'five-scenarios'
    five = actual('five-negative-scenarios', [sys.executable, '-c', five_script, str(five_output)], ROOT)
    if five.returncode == 0:
        proof = json.loads(five.stdout)
        categories = proof.get('categories', [])
        if ([row.get('category') for row in categories] != ['01-outside-prd', '02-design-mismatch', '03-module-boundary', '04-directory-layout', '05-unapproved-framework']
                or any(row.get('positive_exit_code') != 0 or row.get('violation_exit_code') != 1 or row.get('rejected') is not True for row in categories)):
            errors.append('Original exact five negative CLI obligations did not all execute/reject')
    from storage_schema_v5 import validate_contract, assess_boundaries
    schema_errors = validate_contract(ROOT)
    schema_boundaries = assess_boundaries(ROOT)
    if schema_errors or schema_boundaries.get('status') != 'passed' or schema_boundaries.get('errors'):
        errors.extend(schema_errors + schema_boundaries.get('errors', []))
        if not schema_errors and not schema_boundaries.get('errors'):
            errors.append('Actual independent schema5 boundary checks did not pass')
    (directory / 'storage-v5-boundaries.json').write_text(json.dumps(schema_boundaries, ensure_ascii=False, indent=2) + '\n')
    semantic = execute_binding(ROOT, relative + '/component-closure', nonce=args.binding_nonce)
    if source_files(ROOT) != before:
        errors.append('P2 root source epoch changed during actual checks')
    eligible = semantic.get('eligible_semantic_red') is True and not errors
    receipt = {'schema_version': 1, 'kind': 'binding-gate-phase-root-execution/v1',
               'change_id': CHANGE, 'profile': 'phase_admission', 'phase': 'gates',
               'status': 'failed', 'semantic_red': eligible, 'errors': errors,
               'source_tree_sha256': tree_sha(before), 'product_source_count': len(products(before)),
               'executions': executions, 'guard_execution': ref(ROOT, relative + '/guards/execution.json'),
               'binding_execution': semantic.get('execution'), 'binding_inspection': semantic,
               'storage_v5_design_errors': schema_errors,
               'storage_v5_boundaries': ref(ROOT, relative + '/storage-v5-boundaries.json'),
               'storage_v5_runtime_inspection': semantic.get('storage_v5_inspection'),
               'product_completion': 'incomplete', 'next_stage_allowed': False,
               'implementation_admitted': False, 'complete_verify_executed': False}
    if policy.get('resume_epoch') is not None:
        receipt['epoch_id'] = policy['resume_epoch']['epoch_id']
        receipt['contract_only_guard_fixture'] = ref(ROOT, relative + '/contract-only-guard-fixture.json')
    (directory / 'receipt.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'status': receipt['status'], 'semantic_red': eligible, 'errors': errors,
                      'semantic_case_failures': semantic.get('failed'), 'blocked': semantic.get('blocked'),
                      'infrastructure_errors': semantic.get('infrastructure_errors'),
                      'validation_errors': semantic.get('validation_errors'),
                      'receipt': relative + '/receipt.json', 'product_completion': 'incomplete', 'next_stage_allowed': False}, ensure_ascii=False))
    return 1  # A genuine RED is retained nonzero; this is never product approval.
TEST_CHECKS['binding-readiness-assessment-tests'] = ('python', 'test_binding_readiness_assessment')

CHECK_IDS = set(TEST_CHECKS) | {'storage-schema-contract', 'storage-boundaries', 'storage-boundary-execution', 'storage-acceptance-contract', 'database-design-contract', 'database-design-export-identity', 'typescript-compile', 'rust-compile', 'source-scope', 'baseline-integrity', 'test-plan-contract', 'test-plan-export-identity', 'test-operations-contract', 'test-operations-export-identity'}
# These IDs execute code-registered commands; guard screenshots never substitute.
CHECK_IDS.update(PRODUCT_CAPTURE_CHECKS)
CHECK_IDS.update(PRODUCT_RUNTIME_CHECKS)
CHECK_IDS.update(PRODUCT_TEST_CHECKS)
CHECK_IDS.add('generated-contract-drift')
CHECK_IDS.add('rust-tests-compile')
CHECK_IDS.add('model-service-design-docs')
CHECK_IDS.add('offline-translation-design')
CHECK_IDS.add('translation-service-selection-design')
CHECK_IDS.add('flat-service-session-design')
CHECK_IDS.add('guided-service-reading-design')
CHECK_IDS.update({'current-product-coverage-contract','current-product-readiness-assessment','current-product-readiness-contract','current-product-release-coverage'})
CHECK_IDS.update({'general-settings-source-authority','layout-supplement-source-authority','documentation-contract','current-design-alignment-contract','current-design-product-alignment','current-design-release-alignment','current-design-readiness-assessment','current-design-readiness-contract'})
from current_design_alignment import ADAPTERS as CURRENT_DESIGN_ADAPTERS
from current_design_alignment import adapter_capability_errors, required_states as current_required_states
CHECK_IDS.update(CURRENT_DESIGN_ADAPTERS)
CHECK_IDS.update({'guided-design-authority','guided-design-authority-contract','all-page-design-coverage-contract','all-page-design-readiness-assessment','all-page-design-readiness-contract'})
CHECK_IDS.update({'history-design-contract','history-design-execution','history-design-execution-contract'})
CHECK_IDS.update({'current-design-authority','current-design-authority-contract',
                  'change-review-execution','change-review-contract'})
CHECK_IDS.update({'reading-alignment-preflight-execution', 'reading-alignment-preflight-contract'})
CHECK_IDS.add(READING_COMPONENT_CHECK)
CHECK_IDS.add('general-native-visual-acceptance')
CHECK_IDS.update({'lifecycle-impact-contract','lifecycle-impact-execution'})
CHECK_IDS.update({'binding-phase-admission', 'binding-journey-execution', 'binding-successor-handoff',
                  'binding-journey-readiness-assessment', 'binding-journey-readiness-contract'})
CHECK_IDS.update({'storage-schema-v5-contract', 'storage-schema-v5-boundaries', 'storage-schema-v5-runtime-inspection'})
CHECK_IDS.update({'product-completion-assessment', 'product-acceptance-completion'})
CHECK_IDS.update({'interaction-coverage-contract', 'interaction-design-execution', 'implementation-status-contract'})


def check_current_binding_handoff(root, change, paths, requested):
    from binding_phase_guard import successor_handoff_required, validate_successor_current_handoff
    if 'binding-successor-handoff' in requested or successor_handoff_required(root, change, paths):
        return validate_successor_current_handoff(root, change)
    return None


def effective_binding_checks(requested, automatic):
    effective = set(requested) | set(automatic)
    unknown = effective - CHECK_IDS
    if unknown:
        raise ValueError('Unknown executable binding/requested check: ' + ', '.join(sorted(unknown)))
    return effective


def dispatch_schema_v5_checks(effective, *, validate_contract, assess_boundaries):
    effective = effective_binding_checks(effective, ())
    if not effective & {'storage-schema-v5-contract', 'storage-schema-v5-boundaries'}:
        return []
    errors = validate_contract()
    boundaries = assess_boundaries()
    return [{'id': 'storage-schema-v5-contract', 'errors': errors, 'product_storage_tests_executed': False},
            {'id': 'storage-schema-v5-boundaries', 'errors': boundaries.get('errors', []) or
             ([] if boundaries.get('status') == 'passed' else ['Independent schema5 SQL boundary checks did not pass']),
             'execution': boundaries, 'product_storage_tests_executed': False}]


def dispatch_binding_checks(effective, *, execute_full, execute_assessment, inspect_assessment=None):
    """Run one actual full producer, sharing facts without downgrading full BIND.

    Each requested check has an actual handler. An assessment creates only its
    own QA step; automatically required or explicitly requested full BIND keeps
    every original error and its nonzero qualification.
    """
    effective = effective_binding_checks(effective, ())
    assessment_ids = {'binding-journey-readiness-assessment', 'binding-journey-readiness-contract'}
    actual = None
    rows = []
    if effective & assessment_ids:
        assessment = execute_assessment()
        actual = assessment.get('full_binding_result') or {}
        rows.append({'id': 'binding-journey-readiness-assessment', 'errors': assessment.get('errors', [])
                     or ([] if assessment.get('assessment_status') == 'passed' else ['Actual binding readiness assessment failed']),
                     'execution': assessment, 'product_tests_executed': False,
                     'handler': 'binding_readiness_assessment.execute_assessment'})
        if 'binding-journey-readiness-contract' in effective:
            if inspect_assessment is None:
                raise ValueError('Requested readiness contract needs its actual independent consumer')
            rows.append({'id': 'binding-journey-readiness-contract', 'errors': inspect_assessment(assessment),
                         'handler': 'binding_readiness_assessment.assessment_errors', 'product_tests_executed': False})
    if effective & {'binding-journey-execution', 'storage-schema-v5-runtime-inspection'}:
        if actual is None:
            actual = execute_full()
        if 'binding-journey-execution' in effective:
            rows.append({'id': 'binding-journey-execution', 'errors': actual.get('errors', [])
                         or ([] if actual.get('status') == 'passed' else ['Actual complete binding qualification remains incomplete']),
                         'execution': actual, 'scope': 'Original full BIND: all errors and blocked layers remain blocking'})
        if 'storage-schema-v5-runtime-inspection' in effective:
            v5 = actual.get('storage_v5_inspection') or {'status': 'blocked', 'errors': ['Actual schema5 runtime evidence missing']}
            rows.append({'id': 'storage-schema-v5-runtime-inspection', 'errors': v5.get('errors', [])
                         or ([] if v5.get('status') == 'passed' else ['Actual strict schema5 runtime must pass']),
                         'execution': v5, 'product_storage_tests_executed': v5.get('status') == 'passed',
                         'scope': 'Strict current schema5 runtime; assessment cannot waive a blocked or failed storage result'})
    return rows


def validate_test_output(framework, output):
    pattern = r'Ran (\d+) tests?\b' if framework == 'python' else r'^# tests (\d+)$'
    matches = re.findall(pattern, output, re.M)
    count = int(matches[-1]) if matches else 0
    skipped_pattern = r'skipped=(\d+)' if framework == 'python' else r'^# (?:skipped|todo|cancelled) (\d+)$'
    omitted = sum(int(value) for value in re.findall(skipped_pattern, output, re.M))
    errors = []
    if count == 0:
        errors.append('no actual tests executed')
    if omitted or framework == 'python' and re.search(r'expected failures=\d+', output):
        errors.append('required guard assertions may not be skipped, todo, cancelled or expected-failure')
    if framework == 'node':
        passed = re.findall(r'^# pass (\d+)$', output, re.M)
        if not passed or int(passed[-1]) != count:
            errors.append('all registered guard tests must actually pass')
    return count, errors


def product_execution_gaps(metadata, changed_paths, profile):
    errors = []
    # Actual production dependency impact owns selection, including fixtures
    # imported from tests. Claimed kind and allowed_paths cannot erase it.
    try:
        states, _ = current_required_states(changed_paths, root=ROOT)
    except (OSError, ValueError, TypeError) as error:
        return ['Invalid actual product execution impact: ' + str(error)]
    visual_change = bool(states)
    def registered_checks(values, legacy, layer):
        if not isinstance(values, list) or not values:
            return False
        return all(isinstance(identity, str) and
                   (not adapter_capability_errors(identity, layer)
                    if identity in CURRENT_DESIGN_ADAPTERS else identity in legacy)
                   for identity in values)
    if visual_change:
        captures = metadata.get('product_visual_checks', [])
        if not registered_checks(captures, PRODUCT_CAPTURE_CHECKS, 'real_react'):
            errors.append('Product UI change requires a registered actual product capture/comparison check; guard screenshots cannot substitute')
    if profile == 'release':
        runtime = metadata.get('product_runtime_checks', [])
        if not registered_checks(runtime, PRODUCT_RUNTIME_CHECKS, 'packaged_native'):
            errors.append('Release requires registered real product runtime checks; evidence hashes alone cannot prove execution')
    return errors


def required_release_cases(root):
    with (root / 'docs/development/client-regression.csv').open(newline='') as stream:
        required = {row['case_id'] for row in csv.DictReader(stream) if row['milestone'] in {'M1', 'M2', 'M2-L'}}
    for relative, key in [('docs/product/model-configuration-acceptance.csv', 'acceptance_id'),
                          ('docs/product/game-library-window-acceptance.csv', 'case_id')]:
        with (root / relative).open(newline='') as stream:
            required.update(row[key] for row in csv.DictReader(stream))
    if not required:
        raise ValueError('Release acceptance inventory is empty')
    return required


def finalize_execution(root, lifecycle_plan, steps, requested, checks, plan_sha256, *, deferred_checks=()):
    """Record actual lifecycle closure and check declared execution at gate end."""
    from lifecycle_impact import close_execution
    checks('lifecycle-impact-execution',close_execution(root,lifecycle_plan,steps),plan_sha256=plan_sha256,scope='actual current-source raw execution closure; not first-product release approval')
    executed = {step['id'] for step in steps}
    checks('scenario-check-execution', [f'Declared check did not execute: {item}' for item in sorted(requested - executed - set(deferred_checks))])


def general_settings_test_version(root, steps, metadata=None):
    """Validate exact design-version migration after real replacement execution.

    The old five-case run is not passed or replayed. A replacement failure remains
    a gate failure; neither a metadata claim nor source approval can skip its run.
    """
    from current_general_settings_authority import expected_test_supersession, validate_test_supersession
    mapping = expected_test_supersession()
    rows = [row for row in (metadata or {}).get('requirements', []) if mapping['replacement_check']['id'] in row.get('checks', [])]
    if rows:
        if len(rows) != 1:
            raise ValueError('Exact single general-settings replacement requirement required')
        mapping = rows[0].get('test_supersession')
    trace = validate_test_supersession(mapping, root=root)
    replacement = trace['replacement_check']
    executions = [step for step in steps if step.get('id') == replacement['id']]
    if len(executions) != 1:
        raise ValueError('Real replacement execution is missing or duplicated')
    execution = executions[0]
    if (execution.get('status') != 'passed' or type(execution.get('exit_code')) is not int
            or execution['exit_code'] != 0 or type(execution.get('test_count')) is not int
            or execution['test_count'] <= 0
            or execution.get('command') != ['node', '--test', replacement['test']]):
        raise ValueError('Replacement must actually execute its registered test set without failure')
    return {'migration': trace, 'historical_execution': 'not_run_superseded_design',
            'replacement_execution': copy.deepcopy(execution), 'product_acceptance_granted': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--change', help='OpenSpec change ID; otherwise openspec/active-change.json')
    parser.add_argument('--profile', choices=['development', 'release'], default='development')
    parser.add_argument('--base', help='Git base SHA/ref; required in CI')
    parser.add_argument('--artifact', help='Actual final package, relative to repository')
    parser.add_argument('--evidence', help='Artifact-bound release evidence JSON')
    parser.add_argument('--runtime-inputs', help='Explicit frozen inputs for actual packaged stability runner')
    parser.add_argument('--binding-native-evidence', help='Original fixed-producer Native binding collection')
    parser.add_argument('--binding-service-evidence', help='Actual same-package four-game translation chain')
    parser.add_argument('--binding-nonce', help='Exact nonce shared with explicitly supplied binding live collections')
    parser.add_argument('--binding-gate-phase', action='store_true', help='Restricted P2 gate review inputs; retains real semantic RED and never grants P3')
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--require-product-complete', action='store_true',
                        help='Reject incomplete product acceptance, including missing/blocked/Native/whole-client evidence')
    args = parser.parse_args()
    if args.binding_gate_phase:
        try:
            return binding_gate_phase(args)
        except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
            print(json.dumps({'kind': 'binding-gate-phase-root-execution/v1', 'status': 'failed', 'errors': [str(error)],
                              'product_completion': 'incomplete', 'next_stage_allowed': False}, ensure_ascii=False))
            return 1
    env = {**os.environ, 'OPENSPEC_TELEMETRY': '0', 'DO_NOT_TRACK': '1', 'OPENSPEC_NO_UPDATE_CHECK': '1'}
    out = ROOT / '.build/quality'
    out.mkdir(parents=True, exist_ok=True)
    run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    steps, failures = [], []
    from product_completion import source_identity as completion_source_identity
    source_tree_start_sha256 = completion_source_identity(ROOT)
    metadata = None

    def checks(name, errors, **details):
        errors = list(errors)
        status = 'failed' if errors else 'passed'
        steps.append({'id': name, 'status': status, 'errors': errors, **details})
        print(f'[{status}] {name}', flush=True)
        for error in errors:
            print('  ' + str(error), flush=True)
        failures.extend(errors)

    def run(name, command, cwd=ROOT, accepted_codes=(0,), framework=None):
        started = time.monotonic()
        try:
            result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, timeout=600)
            stdout, stderr, code = result.stdout, result.stderr, result.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            stdout, stderr, code = '', str(error), -1
        logfile = out / f'{run_id}-{name}.log'
        logfile.write_text(stdout + '\n' + stderr)
        errors = [] if code in accepted_codes else [f'{name}: command exited {code}; see {logfile.relative_to(ROOT)}']
        test_count = None
        if framework:
            test_count, test_errors = validate_test_output(framework, stdout + '\n' + stderr)
            errors.extend(f'{name}: {error}' for error in test_errors)
        checks(name, errors, command=command, exit_code=code, test_count=test_count, elapsed_seconds=round(time.monotonic()-started, 3), log=str(logfile.relative_to(ROOT)))
        if errors:
            print((stdout + stderr)[-2500:], flush=True)
        return stdout, code

    # These tests prove the guards reject violations. They do not exercise the product.
    def self_tests():
        for check_id, (framework, selector) in TEST_CHECKS.items():
            if check_id == 'settings-layout-refinement-tests':
                try:
                    version = general_settings_test_version(ROOT, steps, metadata)
                    checks('settings-layout-test-version-contract', [], **version)
                except (OSError, ValueError, TypeError, KeyError) as error:
                    checks('settings-layout-test-version-contract', [str(error)])
                continue
            if framework == 'python':
                run(check_id, [sys.executable, '-m', 'unittest', '-v', selector], cwd=ROOT / 'scripts/quality', framework=framework)
            elif framework == 'python-discover':
                run(check_id, [sys.executable, '-m', 'unittest', 'discover', '-s', selector, '-p', 'test_*.py', '-v'], framework='python')
            else:
                run(check_id, ['node', '--test', selector], framework=framework)
        run('five-scenario-execution', [sys.executable, 'scripts/quality/verify_five_scenarios.py'])

    try:
        if args.self_test:
            self_tests()
        else:
            change = args.change or json.loads((ROOT / 'openspec/active-change.json').read_text())['change_id']
            errors = validate_change(ROOT, change)
            checks('change-contract', errors)
            if errors:
                raise ValueError('OpenSpec change contract is invalid; dependent checks stopped')
            metadata = json.loads((ROOT / 'openspec/changes' / change / 'verification.json').read_text())
            requested = {check for row in metadata['requirements'] for check in row['checks']}
            if 'general-visual-contract-tests' in requested:
                from current_general_visual_authority import metadata_gaps as general_metadata_gaps
                checks('general-visual-source-mapping', general_metadata_gaps(ROOT,metadata),
                       scope='Exact requirement-level approved General source and renderer context; no product acceptance')
            unknown = [f'Unknown executable check: {item}' for item in sorted(requested - CHECK_IDS)]
            checks('registered-checks', unknown)
            if unknown:
                raise ValueError('A scenario references an unregistered executable check')
            # The current complete source decision and non-author review are
            # unconditional. A tooling kind or omitted requested check cannot
            # exempt actual source changes from either preflight.
            from design_authority import current_source_approval
            authority_stdout, authority_code = run('current-design-authority',
                [sys.executable, 'scripts/quality/design_authority.py', '--check', '--current', '--json'])
            authority = json.loads(authority_stdout)
            expected_authority = current_source_approval(ROOT)
            authority_errors = []
            if (authority_code != 0 or expected_authority.get('status') != 'approved_by_user'
                    or expected_authority.get('errors') != []
                    or json.dumps(authority, sort_keys=True) != json.dumps(expected_authority, sort_keys=True)):
                authority_errors.append('Complete design source approval must match the pinned current explicit decision')
            checks('current-design-authority-contract', authority_errors,
                   effective_design_approval=authority, product_tests_executed=False,
                   product_alignment_approved=False)
            if authority_errors:
                raise ValueError('Current complete design approval failed; dependent checks stopped')
            ci = os.getenv('CI', '').lower() not in {'', '0', 'false', 'no', 'off'}
            paths, scope_errors = check_scope(ROOT, metadata, args.base, ci)
            checks('source-scope', scope_errors, changed_paths=paths,
                   source_mode='git-base' if ci else 'git-or-local-snapshot')
            from binding_journey import required_checks as binding_required_checks
            binding_required = binding_required_checks(change, paths, args.profile)
            requested = effective_binding_checks(requested, binding_required)
            successor = check_current_binding_handoff(ROOT, change, paths, requested)
            if successor is not None:
                checks('binding-successor-handoff', successor['errors'], execution=successor,
                       product_tests_executed=False, product_write_allowed=False)
                if successor['status'] != 'passed' or not successor.get('qa_maintenance_admitted'):
                    raise ValueError('Actual successor handoff failed before dependent QA execution')
            if 'binding-phase-admission' in requested:
                from binding_phase_guard import validate_phase
                phase = validate_phase(ROOT, change)
                checks('binding-phase-admission', phase['errors'], execution=phase,
                       scope='ordered phase admission only; not product acceptance')
                if phase['errors']:
                    raise ValueError('Binding phase write order/admission failed; dependent checks stopped')
            from change_review import validate_review
            review_command = [sys.executable, 'scripts/quality/change_review.py', '--change', change, '--json']
            if args.base:
                review_command.extend(['--base', args.base])
            review_stdout, review_code = run('change-review-execution', review_command)
            actual_review = json.loads(review_stdout)
            review = validate_review(ROOT, change, metadata, paths)
            review['errors'] = scope_errors + review['errors']
            review['status'] = 'failed' if review['errors'] else 'passed'
            review_errors = list(review['errors'])
            if (review_code != 0 or json.dumps(actual_review, sort_keys=True)
                    != json.dumps(review, sort_keys=True)):
                review_errors.append('Actual independent-review CLI differs from the recomputed current source/diff review')
            checks('change-review-contract', review_errors, review=review,
                   scope='actual source changes and complete source tree; non-author review required',
                   product_tests_executed=False)
            if review_errors or scope_errors:
                raise ValueError('Independent source review/scope failed; dependent checks stopped')
            if 'model-service-design-docs' in requested:
                run('model-service-design-docs', [sys.executable, 'scripts/report/build_model_service_design_report.py', '--check', '--json'])
            if 'offline-translation-design' in requested:
                run('offline-translation-design', [sys.executable, 'scripts/quality/offline_translation_design.py', '--execute', '--output', '.build/quality/' + run_id + '-offline-design', '--json'])
            if 'guided-service-reading-design' in requested:
                run('guided-service-reading-design', [sys.executable, 'scripts/quality/guided_service_reading.py', '--execute', '--output', '.build/quality/' + run_id + '-guided-design', '--json'])
            if 'flat-service-session-design' in requested:
                run('flat-service-session-design', [sys.executable, 'scripts/quality/flat_service_session.py', '--execute', '--output', '.build/quality/' + run_id + '-flat-design', '--json'])
            if 'translation-service-selection-design' in requested:
                run('translation-service-selection-design', [sys.executable, 'scripts/quality/translation_service_selection.py', '--execute', '--output', '.build/quality/' + run_id + '-service-selection', '--json'])
            if 'reading-alignment-preflight-execution' in requested:
                from reading_alignment_preflight import inspect_execution as inspect_reading_preflight, load as load_reading_preflight
                reading_output = '.build/quality/' + run_id + '-reading-alignment-preflight'
                reading_started = datetime.now(timezone.utc)
                reading_stdout, reading_code = run('reading-alignment-preflight-execution',
                    [sys.executable, 'scripts/quality/reading_alignment_preflight.py', '--execute', '--output', reading_output, '--json'])
                reading_finished = datetime.now(timezone.utc)
                reading_report = json.loads(reading_stdout)
                reading_path = ROOT / reading_output
                reading_context = load_reading_preflight(reading_path / 'context.json')
                reading_invocation = load_reading_preflight(reading_path / 'invocation.json')
                reading_inspection = inspect_reading_preflight(ROOT, reading_path,
                    reading_context, reading_invocation)
                reading_errors = list(reading_inspection['errors'])
                if not (reading_started <= datetime.fromisoformat(reading_context['started_at'])
                        <= datetime.fromisoformat(reading_invocation['finished_at']) <= reading_finished):
                    reading_errors.append('Reading capture evidence lies outside the actual gate command interval')
                if (reading_code != 0 or reading_report.get('status') != 'passed'
                        or any(reading_report.get(key) != value for key, value in reading_inspection.items())):
                    reading_errors.append('Actual reading capture CLI differs from independent raw-artifact recomputation')
                checks('reading-alignment-preflight-contract', reading_errors,
                       execution=reading_inspection, output=reading_output,
                       scope='read-only diagnostic capture, not approved references or product alignment',
                       product_tests_executed=False, product_alignment_approved=False)
            cli = ROOT / 'node_modules/.bin/openspec'
            run('openspec-strict', [str(cli), 'validate', '--all', '--strict', '--no-interactive'])
            checks('baseline-integrity', check_baseline(ROOT))
            checks('product-spec-identity', check_product_specs(ROOT))
            from implementation_status import validate_catalog
            checks('implementation-status-contract', validate_catalog(ROOT))
            from interaction_coverage import validate_registry as validate_rw_coverage, run_design_coverage, release_coverage_gaps
            checks('interaction-coverage-contract', validate_rw_coverage(ROOT))
            # Current design-derived obligations supplement the frozen 153; they
            # apply to every change, including release calls without requested IDs.
            from current_product_coverage import validate_registry as validate_current_coverage, release_coverage_gaps as current_release_gaps
            checks('current-product-coverage-contract', validate_current_coverage(ROOT))
            from documentation_contract import validate_contract as validate_current_docs
            from current_design_alignment import validate_contract as validate_current_design, product_change_gaps as design_product_gaps, release_gaps as design_release_gaps
            checks('documentation-contract', validate_current_docs(ROOT))
            checks('current-design-alignment-contract', validate_current_design(ROOT))
            if 'general-settings-source-authority' in requested:
                from current_general_settings_authority import assess as assess_general_source, validate_mapping as validate_general_mapping, validate_test_supersession as validate_general_test_version
                general_stdout, general_code = run('general-settings-source-authority', [sys.executable, 'scripts/quality/current_general_settings_authority.py'])
                general_source = json.loads(general_stdout)
                general_errors = [] if general_code == 0 and general_source == assess_general_source(ROOT) and general_source.get('source_ready') is True else ['General settings source identity or actual assessment mismatch']
                general_requirements = [item for item in metadata.get('requirements', []) if 'general-settings-source-authority' in item.get('checks', [])]
                if not general_requirements:
                    general_errors.append('General settings check has no requirement mapping')
                for item in general_requirements:
                    try:
                        validate_general_mapping(item.get('design_supplement'))
                        validate_general_test_version(item.get('test_supersession'), root=ROOT)
                    except ValueError as error:
                        general_errors.append(str(error))
                checks('general-settings-source-contract', general_errors, product_tests_executed=False, source_scope='general_settings_supplement_only')
            if 'layout-supplement-source-authority' in requested:
                from current_layout_refinements_authority import assess as assess_layout_source, validate_mapping as validate_layout_mapping
                layout_stdout, layout_code = run('layout-supplement-source-authority', [sys.executable, 'scripts/quality/current_layout_refinements_authority.py'])
                layout_source = json.loads(layout_stdout)
                expected_layout_source = assess_layout_source(ROOT)
                layout_errors = [] if layout_code == 0 and layout_source == expected_layout_source and layout_source.get('source_ready') is True else ['Layout supplement source identity or executable assessment mismatch']
                layout_requirements = [item for item in metadata.get('requirements', []) if 'layout-supplement-source-authority' in item.get('checks', [])]
                if not layout_requirements:
                    layout_errors.append('Layout supplement check has no requirement mapping')
                for item in layout_requirements:
                    try:
                        validate_layout_mapping(item.get('design_supplement'))
                    except ValueError as error:
                        layout_errors.append(str(error))
                checks('layout-supplement-source-contract', layout_errors, product_tests_executed=False, source_scope='three_page_supplement_only')
            from history_design import validate_contract as validate_history_design
            checks('history-design-contract', validate_history_design(ROOT), scope='pending-review history design increment only; parent approved version unchanged', product_tests_executed=False)
            if 'history-design-execution' in requested:
                import history_design as history_checks
                history_relative_output = '.build/quality/' + run_id + '-history-design'
                history_stdout, history_code = run('history-design-execution', [sys.executable, 'scripts/quality/history_design.py', '--execute', '--output', history_relative_output, '--json'])
                history_errors = []
                history_execution = {}
                try:
                    history_execution = json.loads(history_stdout)
                    history_output = history_checks.output_path(ROOT, history_relative_output)
                    expected_node_command = ['node', str(ROOT / history_checks.RUNNER), str(ROOT), str(history_output), str(history_output / 'context.json')]
                    typed_history = {'schema_version':1, 'kind':'history-design-verification/v1',
                                     'scope':'pending_design_candidate_only', 'status':'passed', 'errors':[],
                                     'claims_product':False, 'product_tests_executed':False, 'exit_code':0,
                                     'command':expected_node_command, 'output':str(history_output)}
                    for history_key, history_value in typed_history.items():
                        if type(history_execution.get(history_key)) is not type(history_value) or history_execution.get(history_key) != history_value:
                            history_errors.append('Typed actual history verification differs: ' + history_key)
                    history_context = history_checks._load(history_output / 'context.json')
                    history_invocation = history_checks._load(history_output / 'invocation.json')
                    if (history_code != 0
                        or not isinstance(history_execution.get('execution_nonce'),str)
                        or re.fullmatch(r'[0-9a-f]{32}',history_execution['execution_nonce']) is None
                        or history_execution.get('execution_nonce') != history_context.get('execution_nonce')
                        or history_execution.get('execution_nonce') != history_invocation.get('execution_nonce')
                        or history_execution.get('source_identity') != history_checks.source_context(ROOT)['source_identity']
                        or history_execution.get('source_identity') != history_context.get('source_identity')):
                        history_errors.append('Actual history CLI nonce/source/exit identity differs from the fresh invocation')
                    history_errors.extend(history_checks.validate_execution(ROOT,history_output,history_context,history_invocation))
                except (OSError,ValueError,KeyError,TypeError) as history_error:
                    history_errors.append('Cannot independently inspect actual history output: ' + str(history_error))
                checks('history-design-execution-contract', history_errors, execution=history_execution,
                       scope='actual design-only browser execution independently reinspected; no production/native/game/reference acceptance', product_tests_executed=False)
            from all_page_design_coverage import validate_contract as validate_all_pages, assessment_errors as all_page_assessment_errors
            checks('all-page-design-coverage-contract', validate_all_pages(ROOT), product_tests_executed=False)
            if 'guided-design-authority' in requested:
                authority_stdout, authority_code = run('guided-design-authority', [sys.executable, 'scripts/quality/design_authority.py', '--check', '--json'])
                authority = json.loads(authority_stdout)
                from design_authority import source_approval
                authority_errors = [] if authority == source_approval(ROOT) and authority.get('status') == 'approved_by_user' and authority.get('errors') == [] else ['Source-version approval must match the independently pinned explicit user decision']
                checks('guided-design-authority-contract', authority_errors, effective_design_approval=authority,
                       product_tests_executed=False, product_alignment_approved=False)
            if 'all-page-design-readiness-assessment' in requested:
                all_pages_stdout, all_pages_code = run('all-page-design-readiness-assessment', [sys.executable, 'scripts/quality/all_page_design_coverage.py', '--assess', '--json'], accepted_codes=(0,1))
                all_pages_assessment = json.loads(all_pages_stdout)
                all_pages_errors = all_page_assessment_errors(ROOT, all_pages_assessment, all_pages_code)
                checks('all-page-design-readiness-contract', all_pages_errors, assessment=all_pages_assessment,
                       product_tests_executed=False)

            if 'current-design-readiness-assessment' in requested:
                design_stdout, design_code = run('current-design-readiness-assessment', [sys.executable, 'scripts/quality/current_design_alignment.py', '--assess', '--json'], accepted_codes=(0, 1))
                design_assessment = json.loads(design_stdout)
                from current_design_alignment import assessment_errors as design_assessment_errors
                design_errors = design_assessment_errors(ROOT, design_assessment, design_code)
                design_facts = design_assessment if isinstance(design_assessment, dict) else {}
                design_gaps = design_facts.get('release_gaps')
                checks('current-design-readiness-contract', design_errors, current_readiness=design_facts.get('status'),
                       target_id=design_facts.get('target_id'), required_states=design_facts.get('required_states'),
                       release_gaps=design_gaps, product_tests_executed=False)
            if args.profile == 'release':
                checks('current-design-release-alignment', design_release_gaps(ROOT,args.evidence,args.artifact), product_tests_executed=False,
                       scope='current target independent references, real React and packaged-native state closure; historical v4 cannot substitute')

            if 'current-product-readiness-assessment' in requested:
                assessment_stdout, assessment_code = run('current-product-readiness-assessment', [sys.executable, 'scripts/quality/current_product_coverage.py', '--assess', '--json'], accepted_codes=(0, 1))
                assessment = json.loads(assessment_stdout)
                from current_product_coverage import assessment_errors as product_assessment_errors
                assessment_facts = assessment if isinstance(assessment, dict) else {}
                gaps = assessment_facts.get('release_gaps')
                checks('current-product-readiness-contract', product_assessment_errors(ROOT, assessment, assessment_code),
                       current_readiness=assessment_facts.get('status'), required_executions=assessment_facts.get('required_executions'),
                       current_scope=assessment_facts.get('current_scope'), separate_platform_qualifications=assessment_facts.get('separate_platform_qualifications'),
                       release_gap_count=len(gaps) if isinstance(gaps, list) else None, product_tests_executed=False)
            if args.profile == 'release':
                current_gaps = current_release_gaps(ROOT, evidence=args.evidence, artifact=args.artifact)
                checks('current-product-release-coverage', [f'{row.get("execution_id", row.get("case_id", "current"))}: {row.get("status")}: {row.get("reason")}' for row in current_gaps],
                       release_gaps=current_gaps, scope='additional current Mac/CrossOver obligations; original 153/RW/storage gates remain independently required', product_tests_executed=False)

            rw_execution = None
            if 'interaction-design-execution' in requested:
                rw_execution = run_design_coverage(ROOT, out / 'interaction-design')
                checks('interaction-design-execution', rw_execution.get('errors', []) or ([] if rw_execution.get('status') == 'passed' else ['Actual RW design execution did not pass']), execution=rw_execution,
                       product_runtime_tests_executed=False, scope='design-only; real pipeline, language worker, native and game results cannot be inherited')
            if args.profile == 'release':
                rw_gaps = release_coverage_gaps(ROOT, rw_execution)
                checks('reading-window-release-coverage', [f'{item["case_id"]}/{item["layer"]}: {item["status"]}: {item["reason"]}' for item in rw_gaps], release_gaps=rw_gaps)
            from test_plan_contract import validate_plan
            checks('test-plan-contract', validate_plan(ROOT))
            run('test-plan-export-identity', [sys.executable, 'scripts/quality/render_test_plan.py', '--check'])
            from test_operations_contract import validate_operations
            checks('test-operations-contract', validate_operations(ROOT))
            run('test-operations-export-identity', [sys.executable, 'scripts/quality/render_test_operations.py', '--check'])
            from database_design import validate_design
            checks('database-design-contract', validate_design(ROOT))
            run('database-design-export-identity', [sys.executable, 'scripts/quality/database_design.py', '--check'])
            from storage_schema import validate_schema_contract
            checks('storage-schema-contract', validate_schema_contract(ROOT))
            boundary_path = out / f'{run_id}-field-boundaries.json'
            run('storage-boundaries', [sys.executable, 'scripts/quality/storage_boundaries.py', '--check', '--output', str(boundary_path)])
            boundary = json.loads(boundary_path.read_text())
            registry = json.loads((ROOT / 'docs/architecture/database/field-boundaries.json').read_text())
            executed_probes = boundary.get('checks', [])
            boundary_errors = []
            if boundary.get('status') != 'passed' or boundary.get('errors') or boundary.get('not_product_acceptance') is not True:
                boundary_errors.append('Field boundary execution did not pass as an isolated design check')
            if {row['id'] for row in executed_probes} != {row['id'] for row in registry['probes']} or len(executed_probes) != len(registry['probes']):
                boundary_errors.append('Field boundary execution must cover every registered probe exactly once')
            if any(row.get('status') != 'passed' or row.get('skipped') is not False or type(row.get('assertions')) is not int or row['assertions'] <= 0 for row in executed_probes):
                boundary_errors.append('Field boundary checks must actually execute assertions without skip')
            checks('storage-boundary-execution', boundary_errors, evidence_path=str(boundary_path.relative_to(ROOT)), evidence_sha256=digest(boundary_path),
                   executed_probes=len(executed_probes), assertions=sum(row.get('assertions', 0) for row in executed_probes), product_storage_tests_executed=False)
            from storage_schema_v3 import validate_schema_contract as validate_schema_three
            checks('storage-schema-v3-contract', validate_schema_three(ROOT), product_storage_tests_executed=False)
            from storage_schema_v5 import validate_contract, assess_boundaries
            for row in dispatch_schema_v5_checks(requested, validate_contract=lambda: validate_contract(ROOT),
                                                 assess_boundaries=lambda: assess_boundaries(ROOT)):
                checks(row['id'], row['errors'], **{key: value for key, value in row.items() if key not in {'id', 'errors'}})
            from storage_boundaries_v3 import assess as assess_schema_three_boundaries
            schema_three_boundary = assess_schema_three_boundaries(ROOT)
            schema_three_path = out / f'{run_id}-field-boundaries-v3.json'
            schema_three_path.write_text(json.dumps(schema_three_boundary, ensure_ascii=False, indent=2) + '\n')
            checks('storage-boundaries-v3-execution', schema_three_boundary['errors'],
                   evidence_path=str(schema_three_path.relative_to(ROOT)), evidence_sha256=digest(schema_three_path),
                   executed_probes=len(schema_three_boundary['checks']), product_storage_tests_executed=False)
            from storage_acceptance import validate_registry, storage_requirement_gaps, run_product_storage
            checks('storage-acceptance-contract', validate_registry(ROOT))
            checks('required-product-execution', product_execution_gaps(metadata, paths, args.profile))
            checks('current-design-product-alignment', design_product_gaps(ROOT,metadata,args.profile,paths), actual_changed_paths=paths, product_tests_executed=False)
            from lifecycle_impact import load_contract, plan_impact
            lifecycle_plan=plan_impact(ROOT,metadata,paths,args.profile)
            lifecycle_path=out/f'{run_id}-lifecycle-impact.json'
            lifecycle_path.write_text(json.dumps(lifecycle_plan,ensure_ascii=False,indent=2)+'\n')
            checks('lifecycle-impact-contract',lifecycle_plan['errors'],plan=str(lifecycle_path.relative_to(ROOT)),plan_sha256=digest(lifecycle_path),obligations=lifecycle_plan['obligations'])
            from binding_journey import execute as execute_binding
            from binding_readiness_assessment import execute_assessment, assessment_errors
            binding_options = {'nonce': args.binding_nonce, 'native_evidence': args.binding_native_evidence,
                               'service_evidence': args.binding_service_evidence}
            binding_rows = dispatch_binding_checks(requested,
                execute_full=lambda: execute_binding(ROOT, '.build/quality/' + run_id + '-binding-journey', **binding_options),
                execute_assessment=lambda: execute_assessment(ROOT, '.build/quality/' + run_id + '-binding-readiness', **binding_options),
                inspect_assessment=lambda assessment: assessment_errors(ROOT, assessment))
            for row in binding_rows:
                checks(row['id'], row['errors'], **{key: value for key, value in row.items() if key not in {'id', 'errors'}})
            from current_design_alignment import run_current_adapter
            current_declared = set(metadata.get('product_visual_checks', []) + metadata.get('product_runtime_checks', [])) | requested
            if 'general-native-visual-acceptance' in requested:
                acceptance_command = [sys.executable, 'scripts/quality/current_general_visual_acceptance.py', '--check',
                                      '--output', '.build/quality/' + run_id + '-general-native-visual-acceptance', '--json']
                if args.artifact:
                    acceptance_command.extend(['--artifact', args.artifact])
                acceptance_stdout, acceptance_code = run('general-native-visual-acceptance',
                    acceptance_command)
                acceptance_execution = json.loads(acceptance_stdout)
                acceptance_errors = list(acceptance_execution.get('errors', []))
                if acceptance_code != 0 or acceptance_execution.get('status') != 'passed' or acceptance_execution.get('acceptance_status') != 'accepted_by_user':
                    acceptance_errors.append('Actual fixed retained General visual acceptance did not pass')
                checks('general-native-visual-acceptance-contract', acceptance_errors, execution=acceptance_execution,
                       scope='exact retained user-accepted General S5 visual obligation only',
                       product_tests_executed=False, new_product_capture_executed=False,
                       product_packaged_native_game_tests_executed=False, product_storage_tests_executed=False)
            for current_id in sorted(current_declared & set(CURRENT_DESIGN_ADAPTERS)):
                current_execution = run_current_adapter(ROOT,current_id,'.build/quality/' + run_id + '-current-' + current_id,args.artifact,
                                                        lifecycle_context=lifecycle_plan['execution_context'])
                checks(current_id,current_execution['errors'],execution=current_execution,
                       scope='actual current-target registered dispatch, independently inspected and recomputed')
            actual_checks = requested & (set(PRODUCT_CAPTURE_CHECKS) | set(PRODUCT_TEST_CHECKS) | set(PRODUCT_RUNTIME_CHECKS))
            declared_visual = metadata.get('product_visual_checks', [])
            declared_runtime = metadata.get('product_runtime_checks', [])
            actual_checks.update(item for item in declared_visual + declared_runtime if item in PRODUCT_CAPTURE_CHECKS or item in PRODUCT_RUNTIME_CHECKS)
            for check_id in sorted(actual_checks):
                execution = run_product_check(ROOT, check_id, out / 'product-execution',args.runtime_inputs)
                checks(check_id, execution['errors'], execution=execution, command=execution['command'], exit_code=execution['exit_code'], test_count=execution.get('test_count'), log=execution['log'])
            if READING_COMPONENT_CHECK in requested:
                execution = run_reading_components(ROOT, env=env)
                checks(READING_COMPONENT_CHECK, execution['errors'], execution=execution,
                       command=execution['command'], exit_code=execution['exit_code'],
                       test_count=execution['test_count'], log=execution['log'])
            if metadata['kind'] == 'product' or 'generated-contract-drift' in requested:
                run('generated-contract-drift', [sys.executable, 'scripts/contracts/verify_contracts.py'])
            storage_gaps = storage_requirement_gaps(ROOT, metadata, paths, args.profile)
            checks('required-storage-execution', storage_gaps, scope='production storage implementation/release requires actual compile/run adapter; tooling checks are separate')
            if not storage_gaps and (args.profile == 'release' or metadata.get('product_storage_checks')):
                storage_result = run_product_storage(ROOT, metadata.get('product_storage_checks', []), out / 'storage-product', args.artifact)
                checks('product-storage-execution', storage_result.get('errors', []) or ([] if storage_result.get('status') == 'passed' else ['Actual production storage checks did not pass']), execution=storage_result)


            from technology_contract import check as check_technology
            checks('technology-and-design-contract', check_technology(ROOT))
            findings = []
            for name, command in [
                ('frontend-raw-scan', ['node', 'scripts/quality/frontend-boundaries.mjs', '--json']),
                ('rust-raw-scan', [sys.executable, 'scripts/quality/rust-boundaries.py', '--json']),
            ]:
                stdout, code = run(name, command, accepted_codes=(0, 1))
                if code not in (0, 1):
                    raise ValueError(f'{name} could not execute')
                data = json.loads(stdout)
                if not isinstance(data.get('findings'), list):
                    raise ValueError(f'{name}: invalid checker result')
                if bool(data['findings']) != (code == 1):
                    raise ValueError(f'{name}: exit code does not match findings')
                findings.extend(data['findings'])
            tolerated, errors = check_debt(ROOT, findings, release=args.profile == 'release', base=args.base, ci=ci)
            checks('architecture-debt-ratchet', errors, raw_findings=len(findings), frozen_legacy_findings=len(tolerated), product_architecture_conformant=not findings)
            self_tests()
            run('inherited-plan-integrity', [sys.executable, 'scripts/check_client_rebuild_plan.py'])
            run('typescript-compile', ['npm', 'run', 'typecheck', '--prefix', 'apps/desktop'])
            run('rust-compile', ['cargo', 'check', '--workspace', '--locked', '--offline'])
            # Library checking omits integration-test targets: stale DTO/schema
            # consumers must fail this same gate before runtime acceptance.
            run('rust-tests-compile', ['cargo', 'test', '--workspace', '--no-run', '--locked', '--offline'])

            if args.profile == 'release':
                release_errors = []
                design = json.loads((ROOT / 'docs/design/first-product-v1/manifest.json').read_text())
                if design.get('user_review') != 'full_layout_approved':
                    release_errors.append('Full product design review has not been approved')
                if not args.artifact or not args.evidence:
                    release_errors.append('Release requires --artifact and --evidence; no synthetic passed result is accepted')
                else:
                    artifact = safe_path(ROOT, args.artifact)
                    if not artifact.is_file() or not artifact.stat().st_size:
                        release_errors.append('Final artifact is missing or empty')
                    else:
                        release_errors.extend(validate_release(ROOT, args.evidence, digest(artifact), required_release_cases(ROOT)))
                checks('product-release-evidence', release_errors)
            finalize_execution(ROOT,lifecycle_plan,steps,requested,checks,digest(lifecycle_path),
                               deferred_checks={'product-completion-assessment', 'product-acceptance-completion'})
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        checks('gate-input-or-runtime', [str(error)])

    # A contract step can correctly pass while reporting blocked readiness.
    # Rebuild the product verdict independently of green check subtotals.
    from product_completion import assess_completion, advancement_errors, requires_product_completion
    completion_required = requires_product_completion(metadata, args.profile, args.require_product_complete)
    product_completion = None
    try:
        product_completion = assess_completion(
            ROOT, metadata or {'requirements': []}, steps,
            execution_source_sha256=source_tree_start_sha256,
            evidence=args.evidence, artifact=args.artifact)
        completion_errors = ([] if product_completion['source_tree_sha256'] == source_tree_start_sha256
                             else ['Source epoch changed during verification'])
        checks('product-completion-assessment', completion_errors,
               product_completion=product_completion['product_completion'],
               product_gate_status=product_completion['product_gate_status'],
               next_stage_allowed=product_completion['next_stage_allowed'],
               scope='derived global current product obligations and actual evidence; not guard/source approval')
        if completion_required:
            checks('product-acceptance-completion', advancement_errors(product_completion),
                   product_completion=product_completion['product_completion'],
                   next_stage_allowed=product_completion['next_stage_allowed'])
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        product_completion = {'schema_version': 1, 'kind': 'product-completion-verdict/v1',
                              'product_completion': 'incomplete', 'product_gate_status': 'incomplete',
                              'next_stage_allowed': False,
                              'reasons': [{'code': 'completion_assessment_error', 'detail': str(error)}]}
        checks('product-completion-assessment', [str(error)], next_stage_allowed=False)
    if isinstance(metadata, dict):
        from product_completion import SELF_CHECKS
        expected_completion = {name for row in metadata.get('requirements', [])
                               for name in row.get('checks', []) if name in SELF_CHECKS}
        actual_ids = {step['id'] for step in steps}
        checks('completion-check-execution',
               [f'Declared completion check did not execute: {name}'
                for name in sorted(expected_completion - actual_ids)])

    # A receipt records what actually ran; it is not a declaration of product support.
    tree_hash = hashlib.sha256(json.dumps(source_files(ROOT), sort_keys=True).encode()).hexdigest()
    checks('verification-source-epoch', [] if tree_hash == source_tree_start_sha256
           else ['Managed source changed during verification; prior execution cannot accept the final source'])
    if all(key in product_completion for key in ('design_assessment', 'product_assessment', 'capture_scope_gaps', 'required_engineering_checks')):
        from product_completion import derive_verdict
        # Include late unconditional failures and the final epoch as well. The
        # verdict can never remain complete after a later check failed.
        product_completion = derive_verdict(
            source_tree_sha256=tree_hash, execution_source_sha256=source_tree_start_sha256,
            required_checks=product_completion['required_engineering_checks'], steps=steps,
            design_assessment=product_completion['design_assessment'],
            product_assessment=product_completion['product_assessment'],
            capture_gaps=product_completion['capture_scope_gaps'])
        if completion_required:
            checks('product-completion-finalization', advancement_errors(product_completion),
                   product_completion=product_completion['product_completion'],
                   next_stage_allowed=product_completion['next_stage_allowed'])
    component_executed = any(step.get('execution', {}).get('product_component_tests_executed') is True for step in steps)
    native_executed = any(step.get('execution', {}).get('product_packaged_native_game_tests_executed') is True for step in steps)
    storage_executed = any(step.get('id') == 'product-storage-execution' and step.get('execution', {}).get('product_storage_tests_executed') is True for step in steps)
    receipt = {'schema_version': 1, 'run_id': run_id, 'profile': 'guard-self-tests' if args.self_test else args.profile,
               'status': 'failed' if failures else 'passed', 'source_tree_sha256': tree_hash,
               'source_tree_start_sha256': source_tree_start_sha256, 'steps': steps,
               'product_completion': product_completion,
               'product_runtime_tests_executed': storage_executed or component_executed or native_executed, 'product_component_tests_executed': component_executed, 'product_packaged_native_game_tests_executed': native_executed, 'product_storage_tests_executed': storage_executed, 'product_release_claim': 'not_issued_by_engineering_tooling'}
    (out / f'{run_id}.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    (out / 'latest.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    print(f'Gate result: {receipt["status"]}; receipt: .build/quality/{run_id}.json', flush=True)
    print(f'Product gate: {product_completion["product_gate_status"]}; next stage allowed: {product_completion["next_stage_allowed"]}', flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
