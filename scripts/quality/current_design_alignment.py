#!/usr/bin/env python3
"""Current guided design alignment contract; checking metadata never executes UI.

Historical v3/v4 PNGs and the HTML design runner remain useful in their own
scopes. They have no capability in this registry. A future production adapter
needs reviewed execution, inspection and independent recomputation code before
it can satisfy any current-target state. This module cannot prove arbitrary UI
semantics from JSON, and never creates references or grants design approval.
"""
from __future__ import annotations

import argparse
import copy
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import secrets
from typing import Callable, Mapping
from design_authority import (CURRENT_RECORD, CURRENT_RECORD_SHA256,
                              CURRENT_COMPOSITION_ID, CURRENT_ROUTES,
                              CURRENT_DESIGN_CLOSURE_SHA256, CURRENT_HISTORY_SOURCES,
                              current_record, current_source_approval,
                              current_user_approval_gaps)
import all_page_design_coverage as all_pages

ROOT = Path(__file__).resolve().parents[2]
CONTRACT = 'docs/development/current-design-alignment.json'
TARGET = 'guided-service-reading-20261030'
DESIGN_ROOT = 'docs/design/first-product-v1'
CANDIDATE = DESIGN_ROOT + '/candidates/' + TARGET
MANIFEST = CANDIDATE + '/manifest.json'
COMPOSITION_ID = CURRENT_COMPOSITION_ID
SURFACE_ENTRIES = copy.deepcopy(CURRENT_ROUTES)
DESIGN_CLOSURE_SHA256 = CURRENT_DESIGN_CLOSURE_SHA256
APPROVAL = {'candidate_status': 'pending_review',
            'user_decision': {'path': CURRENT_RECORD, 'sha256': CURRENT_RECORD_SHA256},
            'independent_reference_review': None, 'approved_reference_catalogue': None}
SHA = re.compile(r'[0-9a-f]{64}\Z')
NONCE = re.compile(r'[0-9a-f]{32}\Z')

# Exact current obligations, not counts inferred from a capture producer. Paths
# map to production components already present; they do not claim implementation.
# GD IDs below are authoritative catalogue acceptance IDs. The design runner's
# journey numbers have different meanings and must never replace these sources.
READING = 'apps/desktop/src/features/reading/ReadingPanel.tsx'
RECORDS = 'apps/desktop/src/features/reading/SessionRecordPanel.tsx'
SOURCE = 'apps/desktop/src/features/language/SourceText.tsx'
HUD = 'apps/desktop/src/windows/hud/HudWindow.tsx'
CONFIG = 'apps/desktop/src/features/model-config/ModelConfigScreen.tsx'
SELECT = 'apps/desktop/src/ui/primitives/Select.tsx'
PREVIEW = 'apps/desktop/src/features/capture/WindowPreview.tsx'
STUDY = 'apps/desktop/src/features/study/StudyScreen.tsx'
PREFERENCES = 'apps/desktop/src/features/preferences/PreferencesScreen.tsx'


def _state(identity, scenario, cases, components, paths, owners=('UI', 'QA')):
    # C34's platform owner joins the UI/QA work rather than being dropped from
    # states which include a native window surface.
    owners = tuple(dict.fromkeys((*owners, *(('PL',) if 'C34' in components.split() else ()))))
    return {'id': identity, 'scenario': scenario, 'source_case_ids': cases.split(),
            'component_ids': components.split(), 'component_paths': paths,
            'owners': list(owners), 'required_layers': ['real_react', 'packaged_native']}


GUIDED_STATES = (
    _state('reading-current-card', 'reading', 'GD-01 GUI04', 'C16 C17 C18', [READING, SOURCE]),
    _state('reading-copy-retranslate-fixed-object', 'reading', 'GD-01 GUI04 SS-08', 'C16 C17 C18 C20', [READING, SOURCE, STUDY]),
    _state('reading-independent-comparison', 'reading', 'SS-11', 'C17', [READING]),
    _state('records-three-default-collapsed', 'reading', 'GD-02 GD-03', 'C36', [READING, RECORDS]),
    _state('records-body-expanded', 'reading', 'GD-02 GD-03', 'C36', [READING, RECORDS]),
    _state('records-extraction-expanded', 'reading', 'GD-02 GD-03', 'C36', [READING, RECORDS]),
    _state('records-hook-diagnostics-expanded', 'reading', 'GD-02 GD-03', 'C36', [READING, RECORDS]),
    _state('records-independent-query-page-follow-export', 'reading', 'GD-03', 'C36', [RECORDS]),
    _state('records-folding-preserves-data-preview', 'reading', 'GD-03', 'C35 C36', [READING, RECORDS, PREVIEW]),
    _state('preview-permission-lifecycle', 'reading', 'GD-03 FL-08', 'C35', [PREVIEW]),
    _state('service-selector-main', 'reading', 'GD-09 SS-01', 'C04 C27', [READING, CONFIG, SELECT]),
    _state('service-selector-hud', 'hud', 'GD-09 SS-01', 'C04 C27 C34', [HUD, CONFIG, SELECT]),
    _state('service-selector-eligibility-reasons', 'settings', 'GD-09 GD-10 SS-02 SS-03 SS-04 SS-05 SS-06', 'C04 C26 C27', [CONFIG, SELECT]),
    _state('service-selector-scope-version-enable-independent', 'settings', 'GD-09 SS-07 SS-08 SS-09 SS-10 SS-12 GUI03', 'C27 C31', [CONFIG, READING, HUD]),
    _state('configuration-purpose-brand-interface', 'settings', 'GD-04', 'C04 C26 C27', [CONFIG, SELECT]),
    _state('configuration-required-conditional-custom-fields', 'settings', 'GD-05 GD-06 GD-11', 'C26 C30', [CONFIG]),
    _state('configuration-masked-typed-credentials', 'settings', 'GD-07 GUI06', 'C26 C30', [CONFIG]),
    _state('configuration-save-test-cancel-stale-separated', 'settings', 'GD-09 GUI05', 'C29 C31', [CONFIG]),
    _state('configuration-offline-unknown-resource-route', 'settings', 'GD-10 SS-05', 'C26 C29', [CONFIG]),
    _state('configuration-preset-material-blocker', 'settings', 'GD-05 GD-11', 'C26 C29', [CONFIG]),
    _state('configuration-learning-template-context-policy', 'settings', 'GD-08 GUI02 GUI03 GUI08', 'C27 C28 C29', [CONFIG, STUDY]),
    _state('configuration-advanced-network-per-connection', 'settings', 'GUI01 GD-08', 'C26 C28 C29', [CONFIG]),
    _state('configuration-known-auth-failure-retained', 'settings', 'GUI06 GD-09', 'C29 C30', [CONFIG]),
    _state('configuration-traditional-purpose-preview', 'settings', 'GD-06 GUI07', 'C27 C28 C29', [CONFIG]),
    _state('hud-source-wordcard-learning-tools', 'hud', 'GD-01 GUI04', 'C16 C17 C18 C19 C20 C22', [HUD, SOURCE, STUDY]),
    _state('shared-select-keyboard-focus-escape', 'settings', 'GD-12', 'C04', [SELECT]),
    _state('layout-light-dark-narrow-long-text', 'reading settings hud', 'GD-12', 'C16 C17 C26 C34 C36', [READING, CONFIG, HUD, SOURCE]),
    _state('layout-200-percent-unclipped', 'reading settings hud', 'GD-12 FL-10', 'C16 C17 C26 C34 C36', [READING, CONFIG, HUD, SOURCE]),
    _state('native-effective-shortcuts-focus-window', 'reading settings hud', 'GD-12 FL-10', 'C18 C34', [HUD, PREFERENCES], ('UI', 'PL', 'QA')),
)
STATES = GUIDED_STATES + all_pages.ALL_PAGE_STATES
STATE_IDS = frozenset(row['id'] for row in STATES)
LAYERS = frozenset({'real_react', 'packaged_native'})


@dataclass(frozen=True)
class Adapter:
    """Code-owned capability. JSON cannot install commands or inspectors.

    Each callback is reviewed implementation, not a report boolean. Inspect and
    recompute return error strings after reading raw runtime/capture evidence.
    A future adapter is responsible for measured text/layout/keyboard outcomes;
    the envelope alone is never evidence that those UI semantics occurred.
    """
    target_id: str
    layer: str
    states: frozenset[str]
    command: tuple[str, ...]
    execute: Callable
    inspect: Callable
    recompute: Callable
    # A target crop never establishes conformance of the entire client window.
    # Future whole-client adapters must also inspect an independently reviewed
    # whole-client subject and a matching logical client context.
    capture_scope: str = 'target'
    game_execution: bool = True
    local_scope: str | None = None


# Exact finite real-component capability. All other states and the Native layer
# remain unimplemented; a registered checker does not imply a passing outcome.
from current_design_react_adapter import (CHECK_ID as CURRENT_REACT_CHECK, COMMAND as CURRENT_REACT_COMMAND,
    STATE_IDS as CURRENT_REACT_STATES, execute as execute_current_react)
from current_design_react_inspect import inspect as inspect_current_react
from current_design_react_recompute import recompute as recompute_current_react
from current_general_visual_adapter import (REACT_CHECK as GENERAL_REACT_CHECK, NATIVE_CHECK as GENERAL_NATIVE_CHECK,
    REACT_COMMAND as GENERAL_REACT_COMMAND, NATIVE_COMMAND as GENERAL_NATIVE_COMMAND, STATE_IDS as GENERAL_STATES,
    execute as execute_general_visual)
from current_general_visual_inspect import inspect as inspect_general_visual
from current_general_visual_recompute import recompute as recompute_general_visual
from current_general_visual_authority import SCOPE as GENERAL_SCOPE
ADAPTERS: Mapping[str, Adapter] = {
    GENERAL_REACT_CHECK: Adapter(TARGET, 'real_react', GENERAL_STATES, GENERAL_REACT_COMMAND, execute_general_visual, inspect_general_visual, recompute_general_visual, 'whole_client', False, GENERAL_SCOPE),
    GENERAL_NATIVE_CHECK: Adapter(TARGET, 'packaged_native', GENERAL_STATES, GENERAL_NATIVE_COMMAND, execute_general_visual, inspect_general_visual, recompute_general_visual, 'whole_client', False, GENERAL_SCOPE),
    CURRENT_REACT_CHECK: Adapter(TARGET, 'real_react', CURRENT_REACT_STATES, CURRENT_REACT_COMMAND,
                               execute_current_react, inspect_current_react, recompute_current_react),
}


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key: ' + key)
        result[key] = value
    return result


def _read(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON value: ' + value)))


def _path(root, value):
    if (not isinstance(value, str) or not value or value != value.strip()
            or any(ord(c) < 32 or ord(c) == 127 for c in value)
            or any(c in value for c in ('\\', ':', '?', '#'))):
        raise ValueError('safe relative POSIX path required')
    rel = PurePosixPath(value)
    if rel.is_absolute() or any(part in ('', '.', '..') for part in value.split('/')):
        raise ValueError('path escapes repository')
    path = Path(root).absolute()
    if path.is_symlink():
        raise ValueError('symlink repository forbidden')
    for part in rel.parts:
        path /= part
        if path.is_symlink():
            raise ValueError('symlink source/evidence forbidden')
    path.resolve().relative_to(Path(root).resolve())
    return path


def _digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()


def _identity(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()


def design_sources(root):
    """Inspect the complete declared design closure, including local underlays."""
    manifest = _read(_path(root, MANIFEST))
    if (manifest.get('candidate_id') != TARGET or manifest.get('status') != 'pending_review'
            or manifest.get('entry') != CANDIDATE + '/index.html'):
        raise ValueError('current candidate manifest identity/status differs')
    paths = {MANIFEST}
    for key, prefix in [('owned_sources', CANDIDATE), ('shared_dependencies', DESIGN_ROOT),
                        ('parent_sources_unchanged', DESIGN_ROOT + '/candidates/flat-service-session-20261030')]:
        group = manifest[key]
        if not isinstance(group, dict) or not group:
            raise ValueError('exact design dependency group required')
        for relative, expected in group.items():
            path = prefix + '/' + relative
            if not isinstance(expected, str) or not SHA.fullmatch(expected) or _digest(_path(root, path)) != expected:
                raise ValueError('design source hash drift: ' + path)
            paths.add(path)
    for key in ('presentation_catalog', 'registry_source'):
        row = manifest[key]
        if _digest(_path(root, row['path'])) != row['sha256']:
            raise ValueError('design catalogue hash drift: ' + row['path'])
        paths.add(row['path'])
    assets = _read(_path(root, DESIGN_ROOT + '/assets/manifest.json'))
    for row in assets['files']:
        path = DESIGN_ROOT + '/' + row['file']
        if _digest(_path(root, path)) != row['sha256']:
            raise ValueError('design underlay hash drift: ' + path)
        paths.add(path)
    guided = {path: _digest(_path(root, path)) for path in sorted(paths)}
    complete = current_record(root)['design_sources']
    if (set(complete) != set(guided) | set(CURRENT_HISTORY_SOURCES)
            or any(complete.get(path) != digest for path, digest in guided.items())):
        raise ValueError('current composition must retain the exact guided closure and history override')
    return dict(sorted(complete.items()))


def validate_contract(root):
    # Every caller validates the full actual page discovery, including tooling
    # metadata checks. A product consumer cannot opt out through requested checks.
    errors = all_pages.validate_contract(root)
    try:
        data = _read(_path(root, CONTRACT))
        expected_keys = {'schema_version', 'target_id', 'composition_id', 'entry', 'surface_entries', 'design_sources',
                         'design_closure_sha256', 'approval', 'states'}
        if set(data) != expected_keys or type(data.get('schema_version')) is not int or data['schema_version'] != 2:
            errors.append('exact current alignment schema required')
        if data.get('target_id') != TARGET or data.get('entry') != CANDIDATE + '/index.html':
            errors.append('code-owned current target differs')
        if (data.get('composition_id') != COMPOSITION_ID
                or _identity(data.get('surface_entries')) != _identity(SURFACE_ENTRIES)):
            errors.append('code-owned complete composition and per-surface entry routes differ')
        sources = design_sources(root)
        if (data.get('design_sources') != sources or data.get('design_closure_sha256') != DESIGN_CLOSURE_SHA256
                or _identity(sources) != DESIGN_CLOSURE_SHA256):
            errors.append('current design closure differs from independent code-owned identity')
        if _identity(data.get('approval')) != _identity(APPROVAL):
            errors.append('captured candidate, pinned explicit source decision and independent references must stay separate')
        errors += current_user_approval_gaps(root)
        if _identity(data.get('states')) != _identity(STATES):
            errors.append('exact complete current state/component/Owner/source mapping required; missing/duplicate/unknown state forbidden')
        for state in STATES:
            for path in state['component_paths']:
                if not _path(root, path).is_file():
                    errors.append('mapped production component missing: ' + path)
        import csv
        components = {row['component_id'] for row in csv.DictReader(_path(root, 'docs/architecture/engineering-components.csv').read_text().splitlines())}
        cases = ({row['id'] for row in _read(_path(root, 'docs/development/current-product-test-coverage.json'))['cases']}
                 | all_pages.authoritative_case_ids(root))
        for state in STATES:
            if not set(state['component_ids']) <= components or not set(state['source_case_ids']) <= cases:
                errors.append('current state has unknown authoritative component/source: ' + state['id'])
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append('current alignment contract invalid: ' + str(error))
    return errors


def required_states(changed_paths=None, *, root=ROOT):
    """Use actual diff paths. Missing/unknown product impact conservatively means all."""
    if changed_paths is None:
        return STATE_IDS, True
    if not isinstance(changed_paths, (list, tuple)) or any(not isinstance(path, str) for path in changed_paths):
        raise ValueError('actual changed paths must be a list/tuple of relative paths')
    states, native, global_impact = set(), False, False
    for path in changed_paths:
        _path(root, path)  # Syntax/symlink checks; no existence requirement for deletions.
        relevant = all_pages._potential_product_path(path)
        if not relevant:
            continue
        native |= all_pages._native_product_path(path) or path in ('Cargo.toml', 'Cargo.lock')
        global_impact |= path.startswith(('apps/desktop/src/ui/', 'apps/desktop/src/app/',
                                          'apps/desktop/src/bridge/'))
        matched = {row['id'] for row in GUIDED_STATES if path in row['component_paths']}
        states.update(matched)
    affected_pages = all_pages.affected_state_ids(root, changed_paths)
    states.update(affected_pages)
    if global_impact or affected_pages == all_pages.STATE_IDS:
        # A shared/global or unknown impact cannot leave any original guided
        # obligation behind when the complete page closure is required.
        states.update(STATE_IDS)
    else:
        # The AST-discovered page closures also cover transitive dependencies of
        # original guided components. Preserve those original obligations while
        # allowing known page-only changes to use their actual dependency scope.
        components = {path for row in all_pages.ALL_PAGE_STATES
                      if row['id'] in affected_pages for path in row['component_paths']}
        states.update(row['id'] for row in GUIDED_STATES
                      if components.intersection(row['component_paths']))
    return frozenset(states), native


def _adapter_errors(adapter):
    if not isinstance(adapter, Adapter):
        return ['adapter must be a code-owned Adapter, never JSON capabilities']
    errors = []
    if adapter.target_id != TARGET or not isinstance(adapter.layer,str) or adapter.layer not in LAYERS:
        errors.append('adapter target/layer is not current real React or packaged native')
    if not isinstance(adapter.states, frozenset) or not adapter.states or not adapter.states <= STATE_IDS:
        errors.append('adapter has missing/unknown current state capabilities')
    if adapter.capture_scope not in ('target', 'whole_client'):
        errors.append('adapter requires an explicit code-owned capture scope')
    if (not isinstance(adapter.command, tuple) or not adapter.command
            or any(not isinstance(value, str) or not value for value in adapter.command)):
        errors.append('adapter requires an exact executable command')
    callbacks = (adapter.execute, adapter.inspect, adapter.recompute)
    if not all(callable(value) for value in callbacks) or len({id(value) for value in callbacks}) != 3:
        errors.append('adapter requires separate code-owned execute/inspect/independent recompute')
    return errors


def adapter_capability_errors(identity, layer=None):
    """Validate a registered code capability, without approving or executing it."""
    if not isinstance(identity, str) or not identity:
        return ['adapter requires a code-owned registry check ID']
    adapter = ADAPTERS.get(identity)
    errors = _adapter_errors(adapter)
    if layer is not None and (not isinstance(layer,str) or layer not in LAYERS):
        errors.append('unknown required current execution layer')
    elif not errors and layer is not None and adapter.layer != layer:
        errors.append('adapter does not implement required current execution layer: ' + layer)
    return errors


def registered_execution_checks(required_states, layers=None):
    """Return only real registered capabilities covering each required layer.

    This read-only selection does not clear source/reference/product gaps and
    cannot turn metadata or a preflight check into an executable capability.
    """
    if (not isinstance(required_states, (set, frozenset, list, tuple))
            or any(not isinstance(state, str) for state in required_states)):
        return frozenset(), ['actual current state IDs required for execution selection']
    required = frozenset(required_states)
    if not required <= STATE_IDS:
        return frozenset(), ['unknown current state IDs in execution selection']
    if layers is None:
        required_layers = {layer for row in STATES if row['id'] in required
                           for layer in row['required_layers']}
    elif (not isinstance(layers, (set, frozenset, list, tuple))
          or any(not isinstance(layer, str) for layer in layers)
          or not set(layers) <= LAYERS or required and not layers):
        return frozenset(), ['known nonempty current execution layers required']
    else:
        required_layers = set(layers)
    selected, errors = set(), []
    supported = {layer: set() for layer in required_layers}
    for identity in ADAPTERS:
        problems = adapter_capability_errors(identity)
        if problems:
            errors.extend(str(identity) + ': ' + problem for problem in problems)
            continue
        adapter = ADAPTERS[identity]
        if adapter.layer in supported and required.intersection(adapter.states):
            selected.add(identity)
            supported[adapter.layer].update(adapter.states)
    for layer in sorted(required_layers):
        missing = required - supported[layer]
        if missing:
            errors.append('no registered current-target ' + layer
                          + ' executor with independent inspection/recompute for states: '
                          + ','.join(sorted(missing)))
    return frozenset(selected), errors


def _approval_gaps(root, required=None):
    # Source approval clears only its own gap. No reference/adapter/runtime
    # authority is inherited from an approved source version or old v4.
    from current_reference_catalogue import reference_gaps
    return current_user_approval_gaps(root) + reference_gaps(root, STATE_IDS if required is None else required)


def _design_state_gaps(root, required):
    gaps = []
    try:
        unresolved = all_pages.unresolved_design_state_ids(root, required)
        missing = all_pages.missing_design_state_ids(root, required)
        unmapped = all_pages.unmapped_design_state_ids(root, required)
        if missing:
            gaps.append('verified current page design states are missing; source-anchored design resolution and independent review required before product acceptance: '
                        + ','.join(sorted(missing)))
        if unmapped:
            gaps.append('current page design-state mapping needs_mapping; reviewed executable state mapping required before product acceptance: '
                        + ','.join(sorted(unmapped)))
        if unresolved != missing | unmapped or missing & unmapped:
            gaps.append('current unresolved design-state classification is inconsistent; product acceptance remains blocked')
        from current_preview_authority import source_mapping_gaps
        page_rows = [row for page in all_pages.expected_contract(root)['pages'] for row in page['states']]
        gaps += source_mapping_gaps(root, [*GUIDED_STATES, *page_rows], required)
    except (OSError, ValueError, KeyError, TypeError) as error:
        gaps.append('current design-state assessment failed; product acceptance remains blocked: '
                    + type(error).__name__ + ': ' + str(error))
    return gaps


def product_change_gaps(root, metadata, mode='development', changed_paths=None):
    """Preflight only. Root must execute every declared current adapter afterward.

    requirements[].checks cannot suppress this check. Only actual changed paths
    may narrow its obligation; allowed_paths and claimed state lists are ignored.
    """
    errors = validate_contract(root)
    # Binding semantics are a separate finite capability. A passing diagnostic
    # never becomes a visual state/PNG or Native adapter in ADAPTERS.
    from binding_journey import CHANGE as binding_change, affected_paths, validate_registry
    if metadata.get('id') == binding_change:
        errors += validate_registry(root)
    if mode not in {'development', 'release'}:
        return errors + ['unknown alignment mode']
    # A claimed tooling kind must not exempt actual production/asset edits.
    # For a genuine tooling check with no diff supplied, remain contract-only.
    if mode != 'release' and metadata.get('kind') != 'product' and changed_paths is None:
        return errors
    try:
        required, native = required_states(None if mode == 'release' else changed_paths, root=root)
    except (OSError, ValueError, TypeError) as error:
        return errors + ['invalid actual changed paths: ' + str(error)]
    if not required:
        return errors
    if metadata.get('id') != binding_change and (affected_paths(changed_paths or ()) or mode == 'release'):
        errors += validate_registry(root)
    errors += _approval_gaps(root, required) + _design_state_gaps(root, required)
    if required & GENERAL_STATES and any(identity in metadata.get('product_visual_checks', []) for identity in (GENERAL_REACT_CHECK, GENERAL_NATIVE_CHECK)):
        from current_general_visual_authority import metadata_gaps as general_metadata_gaps
        errors += general_metadata_gaps(root,metadata)
    declared = set()
    for field in ('product_visual_checks', 'product_runtime_checks'):
        values = metadata.get(field, [])
        if not isinstance(values, list) or any(not isinstance(value, str) for value in values):
            errors.append('invalid declared checks: ' + field)
        else:
            declared.update(values)
    layers = {layer for state in STATES if state['id'] in required for layer in state['required_layers']}
    for layer in sorted(layers):
        supported = set()
        for identity in sorted(declared):
            adapter = ADAPTERS.get(identity)
            if adapter is None:
                continue  # Historical checks retain their scope, never current capability.
            problems = _adapter_errors(adapter)
            errors += [identity + ': ' + problem for problem in problems]
            if not problems and adapter.layer == layer:
                supported.update(adapter.states)
        missing = required - supported
        if missing:
            errors.append('no registered current-target ' + layer + ' executor with independent inspection/recompute for states: ' + ','.join(sorted(missing)))
    return errors


def _hashed(root, reference):
    if (not isinstance(reference, dict) or set(reference) != {'path', 'sha256'}
            or not isinstance(reference['sha256'], str) or not SHA.fullmatch(reference['sha256'])):
        raise ValueError('exact evidence path/hash required')
    path = _path(root, reference['path'])
    if not path.is_file() or not path.stat().st_size or _digest(path) != reference['sha256']:
        raise ValueError('missing/empty/old-hash evidence')
    return path


def source_identity(root):
    from gate_core import source_files
    return hashlib.sha256(json.dumps(source_files(Path(root)), sort_keys=True).encode()).hexdigest()


def _inspect_receipt(root, receipt, artifact_sha256, execution_context=None):
    # A retained user disposition is a fresh read-only evaluation. It cannot
    # impersonate a newly executed Native capture or inherit its old nonce.
    from current_general_visual_acceptance import RECEIPT_KIND, inspect_receipt as inspect_user_acceptance
    if receipt.get('kind') == RECEIPT_KIND:
        return inspect_user_acceptance(root, receipt, artifact_sha256, execution_context)
    adapter = ADAPTERS.get(receipt.get('adapter_id'))
    problems = _adapter_errors(adapter)
    if problems:
        raise ValueError('; '.join(problems))
    expected = {'schema_version': 1, 'target_id': TARGET, 'composition_id': COMPOSITION_ID, 'layer': adapter.layer,
                'design_closure_sha256': DESIGN_CLOSURE_SHA256,
                'source_tree_sha256': source_identity(root), 'artifact_sha256': artifact_sha256,
                'command': list(adapter.command), 'status': 'passed', 'errors': [],
                'producer_kind': adapter.layer, 'prototype_substitution': False,
                'native_verified': adapter.layer == 'packaged_native'}
    for key, value in expected.items():
        if _identity(receipt.get(key)) != _identity(value):
            raise ValueError('receipt current target/source/package/layer identity differs: ' + key)
    nonce = receipt.get('execution_nonce')
    if not isinstance(nonce, str) or not NONCE.fullmatch(nonce):
        raise ValueError('actual execution nonce required')
    started = datetime.fromisoformat(receipt['started_at'].replace('Z', '+00:00'))
    finished = datetime.fromisoformat(receipt['finished_at'].replace('Z', '+00:00'))
    if started.tzinfo is None or finished.tzinfo is None or started > finished:
        raise ValueError('actual timezone-aware execution interval required')
    if execution_context is not None:
        invocation_expected = {key: expected[key] for key in
                               ('schema_version','target_id','composition_id','layer',
                                'design_closure_sha256','source_tree_sha256','artifact_sha256','command')}
        invocation_expected.update(adapter_id=receipt['adapter_id'], state_ids=sorted(adapter.states))
        for key, value in invocation_expected.items():
            if _identity(execution_context.get(key)) != _identity(value):
                raise ValueError('dispatcher invocation identity differs: ' + key)
        package = _path(root, execution_context['artifact_path'])
        if not package.is_file() or _digest(package) != artifact_sha256:
            raise ValueError('dispatcher invocation package is missing or changed')
        output = execution_context['output']
        if not isinstance(output,str) or not output.startswith('.build/'):
            raise ValueError('dispatcher invocation output must remain in .build')
        _path(root, output)
        if nonce != execution_context['execution_nonce']:
            raise ValueError('receipt nonce does not bind the actual dispatcher invocation')
        lower = datetime.fromisoformat(execution_context['started_at'])
        upper = datetime.fromisoformat(execution_context['finished_at'])
        if not lower <= started <= finished <= upper:
            raise ValueError('receipt time is outside actual dispatcher invocation')
        if 'lifecycle_execution_nonce' in execution_context or 'lifecycle_started_at' in execution_context:
            lifecycle_nonce=execution_context.get('lifecycle_execution_nonce')
            lifecycle_started=execution_context.get('lifecycle_started_at')
            if not isinstance(lifecycle_nonce,str) or not NONCE.fullmatch(lifecycle_nonce) or not isinstance(lifecycle_started,str):
                raise ValueError('typed lifecycle invocation binding required')
            lifecycle_time=datetime.fromisoformat(lifecycle_started.replace('Z','+00:00'))
            if lifecycle_time.tzinfo is None or lifecycle_time>lower:
                raise ValueError('lifecycle invocation must precede actual dispatcher interval')
    states = receipt.get('state_ids')
    if (not isinstance(states, list) or any(not isinstance(value, str) for value in states)
            or len(states) != len(set(states)) or set(states) != adapter.states):
        raise ValueError('receipt missing/duplicate/unknown registered state')
    if (type(receipt.get('assertion_count')) is not int or receipt['assertion_count'] <= 0
            or type(receipt.get('skipped_assertions')) is not int or receipt['skipped_assertions'] != 0):
        raise ValueError('positive actual assertions and zero skipped required')
    # Both independent code paths must inspect the actual raw artifacts. No
    # receipt-provided inspector name, command, boolean or JSON installs one.
    context = {'target_id': TARGET, 'composition_id': COMPOSITION_ID,
               'design_closure_sha256': DESIGN_CLOSURE_SHA256,
               'layer': adapter.layer, 'state_ids': sorted(adapter.states),
               'artifact_sha256': artifact_sha256, 'execution_nonce': nonce}
    for label, inspect in [('inspection', adapter.inspect), ('independent recomputation', adapter.recompute)]:
        inspected_receipt, inspected_context = copy.deepcopy(receipt), copy.deepcopy(context)
        result = inspect(Path(root), inspected_receipt, inspected_context)
        if _identity(inspected_receipt) != _identity(receipt) or _identity(inspected_context) != _identity(context):
            raise ValueError(label + ' must inspect without mutating invocation or raw receipt')
        if not isinstance(result, list) or any(not isinstance(value, str) for value in result) or result:
            raise ValueError(label + ' failed or did not return inspected error list: ' + str(result))
    return adapter.layer, adapter.states


def run_current_adapter(root, identity, output, artifact=None, *, lifecycle_context=None):
    """Dispatch reviewed production code, then independently inspect raw evidence.

    Only code-registered finite capabilities are installed; metadata cannot
    provide commands/callbacks or enlarge their states. A live nonce
    and invocation interval bind the returned receipt, rather than accepting a
    prewritten success file. Release still checks the complete state closure.
    """
    global_errors = validate_contract(root)
    adapter = ADAPTERS.get(identity)
    adapter_errors = _adapter_errors(adapter)
    if identity == GENERAL_NATIVE_CHECK and not adapter_errors and isinstance(artifact, str):
        from current_general_visual_acceptance import is_registered_artifact, dispatch as dispatch_user_acceptance
        try:
            package = _path(root, artifact)
            registered = package.is_file() and is_registered_artifact(_digest(package))
        except (OSError, ValueError, TypeError):
            registered = False
        if registered:
            result = dispatch_user_acceptance(root, output, artifact, lifecycle_context=lifecycle_context)
            result.update(global_contract_errors=global_errors, global_product_gate_passed=False)
            return result
    # Only the two code-registered finite General visual collectors may collect
    # scoped evidence while unrelated global AST mappings remain blocked. Root
    # product_change_gaps/release_gaps still call the full validator unchanged.
    scoped_general = identity in (GENERAL_REACT_CHECK, GENERAL_NATIVE_CHECK) and not adapter_errors and adapter.states == GENERAL_STATES and adapter.local_scope == GENERAL_SCOPE
    if scoped_general:
        from current_general_visual_authority import authority as general_authority
        from current_general_reference import reference_gaps as general_reference_gaps
        errors = list(adapter_errors)
        try: general_authority(root)
        except (OSError, ValueError, KeyError, TypeError) as error: errors.append('General source authority invalid: '+str(error))
        errors += general_reference_gaps(root, adapter.states, adapter.layer)
    else:
        errors = list(global_errors) + adapter_errors
        errors += _approval_gaps(root, adapter.states if not adapter_errors else None)
        if not adapter_errors: errors += _design_state_gaps(root, adapter.states)
    result = {'status':'failed', 'errors':errors, 'adapter_id':identity,
              'scope': GENERAL_SCOPE if scoped_general else 'whole_product',
              'global_contract_errors': global_errors, 'global_product_gate_passed': False,
              'product_component_tests_executed':False,
              'product_packaged_native_game_tests_executed':False}
    if errors:
        return result
    try:
        if not isinstance(output,str) or not output.startswith('.build/'):
            raise ValueError('fresh .build execution directory required')
        destination = _path(root,output)
        if destination.exists():
            raise ValueError('current adapter output already exists; old receipts cannot be reused')
        package = _path(root,artifact) if isinstance(artifact,str) else None
        if package is None or not package.is_file() or not package.stat().st_size:
            raise ValueError('actual final package is required for both current layers')
        context={'schema_version':1, 'adapter_id':identity,
                 'target_id':TARGET, 'composition_id':COMPOSITION_ID,
                 'layer':adapter.layer, 'state_ids':sorted(adapter.states),
                 'execution_nonce':secrets.token_hex(16), 'source_tree_sha256':source_identity(root),
                 'design_closure_sha256':DESIGN_CLOSURE_SHA256, 'artifact_sha256':_digest(package),
                 'artifact_path':artifact, 'command':list(adapter.command), 'output':output,
                 'started_at':datetime.now(timezone.utc).isoformat()}
        if scoped_general:
            from current_general_visual_authority import expected_mapping as expected_general_mapping
            context.update(scope=GENERAL_SCOPE, general_visual_source=expected_general_mapping(root))
        if lifecycle_context is not None:
            if not isinstance(lifecycle_context,dict):
                raise ValueError('typed lifecycle invocation context required')
            lifecycle_nonce=lifecycle_context.get('execution_nonce')
            lifecycle_started=lifecycle_context.get('started_at')
            if not isinstance(lifecycle_nonce,str) or not NONCE.fullmatch(lifecycle_nonce) or not isinstance(lifecycle_started,str):
                raise ValueError('actual lifecycle nonce and timezone-aware start required')
            lifecycle_time=datetime.fromisoformat(lifecycle_started.replace('Z','+00:00'))
            if lifecycle_time.tzinfo is None or lifecycle_time>datetime.fromisoformat(context['started_at']):
                raise ValueError('lifecycle start must precede actual current dispatch')
            context.update(lifecycle_execution_nonce=lifecycle_nonce,lifecycle_started_at=lifecycle_started)
        destination.mkdir(parents=True)
        (destination/'request.json').write_text(json.dumps(context,ensure_ascii=False,indent=2)+'\n')
        # The registered callback executes the actual fixed command. Its own
        # independent inspectors must recompute semantic/layout/pixel outcomes.
        callback_context=copy.deepcopy(context)
        receipt=adapter.execute(Path(root),callback_context)
        if _identity(callback_context) != _identity(context):
            raise ValueError('executor cannot rewrite the private dispatcher invocation')
        context['finished_at']=datetime.now(timezone.utc).isoformat()
        if _digest(package) != context['artifact_sha256'] or source_identity(root) != context['source_tree_sha256']:
            raise ValueError('source or package changed during current adapter execution')
        if not isinstance(receipt,dict):
            raise ValueError('executed current adapter must return its actual typed receipt')
        if receipt.get('adapter_id')!=identity:
            raise ValueError('executed receipt adapter differs from actual dispatch')
        _inspect_receipt(root,receipt,context['artifact_sha256'],context)
        if _digest(package) != context['artifact_sha256'] or source_identity(root) != context['source_tree_sha256']:
            raise ValueError('source or package changed during independent current inspection')
        receipt_path=_path(root,output+'/receipt.json')
        invocation_path=_path(root,output+'/invocation.json')
        # Neither an executor nor an old run may install the dispatcher's final
        # context. Save it only after both raw inspections and freshness checks.
        with receipt_path.open('x',encoding='utf-8') as stream:
            stream.write(json.dumps(receipt,ensure_ascii=False,indent=2)+'\n')
        with invocation_path.open('x',encoding='utf-8') as stream:
            stream.write(json.dumps(context,ensure_ascii=False,indent=2)+'\n')
        result.update(status='passed',receipt=receipt_path.relative_to(Path(root)).as_posix(),
                      receipt_sha256=_digest(receipt_path),execution_nonce=context['execution_nonce'],
                      invocation=invocation_path.relative_to(Path(root)).as_posix(),
                      invocation_sha256=_digest(invocation_path))
        result['product_component_tests_executed']=adapter.layer=='real_react'
        result['product_packaged_native_game_tests_executed']=adapter.layer=='packaged_native' and adapter.game_execution
        result['product_packaged_native_visual_tests_executed']=adapter.layer=='packaged_native'
    except (OSError,ValueError,KeyError,TypeError,AttributeError) as error:
        result['errors'].append('current actual adapter execution failed: '+str(error))
    return result


def release_gaps(root, evidence=None, artifact=None):
    """Read-only assessment; never execute an adapter or issue product approval."""
    errors = validate_contract(root)
    if errors:
        return errors
    errors += _approval_gaps(root) + _design_state_gaps(root, STATE_IDS)
    # A partial registry is not an all-layer release capability. Report this
    # independently of missing package/sidecar evidence, so one finite checker
    # cannot hide the still-unimplemented Native or remaining React states.
    _checks, capability_gaps = registered_execution_checks(STATE_IDS)
    errors += capability_gaps
    try:
        if artifact is None:
            raise ValueError('actual final package required')
        artifact_path = _path(root, str(artifact))
        if not artifact_path.is_file() or not artifact_path.stat().st_size:
            raise ValueError('actual final package missing/empty')
        artifact_hash = _digest(artifact_path)
        source_hash = source_identity(root)
        outer = evidence if isinstance(evidence, dict) else _read(_path(root, str(evidence))) if evidence is not None else {}
        sidecar = _read(_hashed(root, outer.get('current_design_alignment')))
        if (type(sidecar.get('schema_version')) is not int or sidecar['schema_version'] != 1
                or sidecar.get('target_id') != TARGET or sidecar.get('composition_id') != COMPOSITION_ID
                or sidecar.get('artifact_sha256') != artifact_hash
                or sidecar.get('contract_sha256') != _digest(_path(root, CONTRACT))):
            raise ValueError('alignment sidecar target/contract/package identity differs')
        references = sidecar.get('receipts')
        if not isinstance(references, list) or not references:
            raise ValueError('current-target actual capture receipts required')
        seen, covered = set(), {layer: set() for layer in LAYERS}
        for reference in references:
            path = _hashed(root, reference)
            if path in seen:
                raise ValueError('duplicate alignment receipt')
            seen.add(path)
            layer, states = _inspect_receipt(root, _read(path), artifact_hash)
            covered[layer].update(states)
        if _digest(artifact_path) != artifact_hash or source_identity(root) != source_hash:
            raise ValueError('source or package changed during release evidence inspection')
        for layer in sorted(LAYERS):
            if covered[layer] != STATE_IDS:
                errors.append('missing exact current ' + layer + ' captured states: ' + ','.join(sorted(STATE_IDS - covered[layer])))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        errors.append('current-target execution blocked: ' + str(error))
    if not ADAPTERS:
        errors.append('no production current-target React/native adapters registered; historical/prototype results cannot substitute')
    return errors


def _assessment_record(root, errors, gaps):
    from current_preview_authority import assess as preview_source_approval
    status = 'failed' if errors else 'blocked' if gaps else 'passed'
    return {'schema_version': 1, 'target_id': TARGET, 'status': status,
            'scope': 'current_design_contract_and_evidence_inspection_only',
            'product_tests_executed': False, 'product_alignment_approved': False,
            'required_states': len(STATE_IDS), 'registered_adapters': sorted(ADAPTERS),
            'errors': errors, 'release_gaps': gaps, 'composition_id': COMPOSITION_ID,
            'effective_design_approval': current_source_approval(root),
            'effective_preview_supplement': preview_source_approval(root)}


def assessment_report(root=ROOT, evidence=None, artifact=None):
    """Read current code/authority/evidence facts; never execute or approve UI."""
    errors = validate_contract(root)
    gaps = release_gaps(root, evidence, artifact) if not errors else []
    return _assessment_record(root, errors, gaps)


def assessment_errors(root, report, exit_code, evidence=None, artifact=None):
    """Independently bind a reported assessment to the actual current facts."""
    if not isinstance(report, dict):
        return ['current design assessment must be an object']
    errors = []
    if (type(report.get('schema_version')) is not int or report['schema_version'] != 1
            or type(report.get('required_states')) is not int or report['required_states'] < 0
            or any(not isinstance(report.get(key), list)
                   or any(not isinstance(value, str) for value in report[key])
                   for key in ('registered_adapters', 'errors', 'release_gaps'))
            or not isinstance(report.get('effective_design_approval'), dict)
            or not isinstance(report.get('effective_preview_supplement'), dict)
            or report.get('product_tests_executed') is not False
            or report.get('product_alignment_approved') is not False):
        errors.append('current design assessment requires typed inventory, gaps, authority and read-only facts')
    status = report.get('status')
    if (not isinstance(status, str) or status not in {'passed', 'blocked', 'failed'}
            or type(exit_code) is not int or exit_code not in (0, 1)
            or (exit_code == 0) != (status == 'passed')
            or status != 'failed' and isinstance(report.get('release_gaps'), list)
            and bool(report['release_gaps']) != (status == 'blocked')):
        errors.append('current design assessment exit/status must match actual release gaps')
    expected = assessment_report(root, evidence, artifact)
    try:
        matches = _identity(report) == _identity(expected)
    except (TypeError, ValueError):
        matches = False
    if not matches:
        errors.append('current design assessment differs from independently recomputed target/composition/adapters/approval/release gaps')
    if report.get('errors') != []:
        errors.append('current design assessment metadata contract is invalid')
    return errors


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check', action='store_true')
    mode.add_argument('--assess', action='store_true')
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--evidence')
    parser.add_argument('--artifact')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    result = (assessment_report(args.root, args.evidence, args.artifact) if args.assess
              else _assessment_record(args.root, validate_contract(args.root), []))
    status = result['status']
    print(json.dumps(result, ensure_ascii=False, indent=2) if args.json else 'Current design alignment: ' + status + '; no product tests executed')
    return 0 if status == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
