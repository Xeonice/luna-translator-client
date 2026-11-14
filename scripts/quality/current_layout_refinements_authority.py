#!/usr/bin/env python3
"""Exact, local source authority for the approved three-page supplement.

This module owns the accepted source closure and design descriptors. It cannot
install a global capture recipe or accept a JSON assertion of component success.
Real browser executions and independent PNG comparisons are separate evidence.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path

from design_authority import current_record
from gate_core import safe_path

ROOT = Path(__file__).resolve().parents[2]
SUPPLEMENT_ID = 'layout-refinements-20261107'
TARGET_ID = 'guided-service-reading-20261030'
COMPOSITION_ID = 'complete-guided-history-20261031'
PREFIX = 'docs/design/first-product-v1/candidates/' + SUPPLEMENT_ID + '/'
ENTRY = PREFIX + 'index.html'
RECORD = 'docs/development/design-approval-layout-refinements-20261107.json'
RECORD_SHA256 = '08b61a71a685f73e2bd2f7dd8f3917ba212bb7c437509ab797b74b446e68cb54'
RECORD_BYTES = 4811
MANIFEST = {'path': PREFIX + 'manifest.json',
            'sha256': '0af63c192e7777fe98bb23a472c59b18ec7718541a2cbbcffba8953c2148baf6', 'bytes': 3963}
SOURCE_HASHES = {
    'history-settings.css': ('166c73ba929f0c32d4c7c9ec26c2b23d545b0575ef40148e7bb986f56028381b', 10156),
    'history.html': ('d54f2b90357d8f29ac96ed4550866b32de5188b3c11d748b8413aa34d5b37f96', 12528),
    'index.html': ('52ee8eda51ae93996bb71c595ed014128628c5da353fef9382277aa550d31896', 3124),
    'refinement.js': ('dc26e915ec1f86baced3da2a147817741c32db30f699ec1115842ee63503bd35', 9536),
    'review.css': ('1ac523e5f4b40305898cce4024559842e0078bae6dd02561ffe945bbf5b73635', 2813),
    'review.js': ('196ff29abea89b5563e2c9bbdd0306872cfd9b894155afd444e1ade966a7a8be', 2953),
    'session.css': ('ecb240d4281647b3911325e15ee52781c6941d9f17352a295ff13630a8b09631', 4633),
    'session.html': ('6858dd83b2a7d82fbde451da79d8d02283dbc638d2e53f04c68ba530291e6b85', 12072),
    'settings.html': ('709b17917a356fbf3728151e9c2669fb44c4ceacff691ffc7046ff250e1e9b57', 12073),
}
PARENTS = {
    'docs/development/design-approval-complete-20261031.json': {
        'sha256': '2aefa59a3edf6adc9fa1287378702b026f7a4029660bf2c23f4780abac9beacc', 'bytes': 10791},
    'docs/development/design-approval-client-layout-20261105.json': {
        'sha256': 'ce8071da9fa6b429703ad3a658cef98de17047e2fdad01ed6fbd79ed1ebe0f01', 'bytes': 3200},
    'docs/development/design-approval-preview-20261104.json': {
        'sha256': 'badc91c0a62af6947fa28974933694da94900b5ba17e43188014e198bac2e462', 'bytes': 6257},
}
VIEWPORTS = {'history': [1040, 720], 'settings': [1000, 720], 'session': [1040, 720], 'narrow': [760, 560]}
SCOPES = {
    'history': 'Same column tracks and16px pane content lines for filters/list/detail; light directory action remains within session track.',
    'settings': 'Local switch row safe insets, group rhythm, wrapping; history is always-saved status; unsupported OCR identification remains separate from window preview permission.',
    'session': 'Stopped+saved object focuses source and three saved-sentence actions, one primary reconnect, no unbound placeholder; existing verified preview follows current binding independently of Hook. Empty current is not absence of saved history. Auxiliary prototype canvases visibly open/close; disabled service has an honest recovery link.',
}
NOT_GRANTED = [
    'automatic approval of independent reference PNGs',
    'whole current-design adapter readiness',
    'implementation or Native/game acceptance',
    'whole-product or release acceptance',
]
PRIOR_REVIEW = {
    'path': '.build/current-client-execution-20261105/layout-refinements-20261107/review/final-design-review.json',
    'sha256': '5306e95b4eb4effadce6fc989c13960e0509e8f7087760e9d2f0f66c4375ab22',
}


def _unique(pairs):
    data = {}
    for key, value in pairs:
        if key in data:
            raise ValueError('duplicate JSON key: ' + key)
        data[key] = value
    return data


def _load(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON value: ' + value)))


def _same(actual, expected):
    # Equality must preserve JSON types: True, 1 and 1.0 are different claims.
    return json.dumps(actual, sort_keys=True, ensure_ascii=False, allow_nan=False) == json.dumps(
        expected, sort_keys=True, ensure_ascii=False, allow_nan=False)


def _file(root, relative, fact):
    if not isinstance(relative, str) or any(c in relative for c in ('\\', ':', '?', '#')):
        raise ValueError('plain relative evidence/source path required')
    if relative != relative.strip() or any(part in ('', '.', '..') for part in relative.split('/')):
        raise ValueError('canonical evidence/source path required')
    path = safe_path(root, relative)
    if not path.is_file():
        raise ValueError('missing approved input: ' + relative)
    raw = path.read_bytes()
    if 'bytes' in fact and (type(fact['bytes']) is not int or len(raw) != fact['bytes']):
        raise ValueError('approved input byte length changed: ' + relative)
    if hashlib.sha256(raw).hexdigest() != fact['sha256']:
        raise ValueError('approved input hash changed: ' + relative)
    return path


def expected_sources():
    """Return a copy; callers cannot expand the code-owned nine-source closure."""
    return {PREFIX + name: {'sha256': digest, 'bytes': size} for name, (digest, size) in SOURCE_HASHES.items()}


def expected_descriptors():
    """Exact design-only routes for reviewed states, sizes and appearances.

    These routes describe approved source samples. They neither identify a
    production component capture nor substitute for an independently approved PNG.
    """
    states = {
        'history': {'normal': {'scenario': 'history'}},
        'settings': {'general': {'scenario': 'settings'}},
        'session': {
            'disconnected_saved': {'scenario': 'reading', 'connection': 'disconnected', 'window': 'none', 'translation': 'failed'},
            'exited_saved': {'scenario': 'reading', 'connection': 'exited', 'window': 'none'},
            'disconnected_empty': {'scenario': 'reading', 'connection': 'empty', 'window': 'none'},
            'disconnected_verified_preview': {'scenario': 'reading', 'connection': 'disconnected', 'window': 'verified'},
        },
    }
    descriptors = {}
    for page, page_states in states.items():
        for state, query in page_states.items():
            for size in ('normal', 'narrow'):
                viewport = VIEWPORTS['narrow' if size == 'narrow' else page]
                for theme in ('dark', 'light'):
                    identity = f'{SUPPLEMENT_ID}:{page}:{state}:{viewport[0]}x{viewport[1]}:{theme}'
                    descriptors[identity] = {
                        'id': identity, 'kind': 'approved-layout-design-source-descriptor/v1',
                        'supplement_id': SUPPLEMENT_ID, 'target_id': TARGET_ID, 'composition_id': COMPOSITION_ID,
                        'page': page, 'state': state, 'entry': PREFIX + page + '.html',
                        'viewport': list(viewport), 'query': {**query, 'theme': theme, 'settingsSize': 'native', 'compare': 'after'},
                        'target_selector': '#settings-window' if page == 'settings' else '#main-window',
                        'source_count': 9, 'evidence_role': 'design_only',
                    }
    return descriptors


def validate_descriptor(descriptor):
    """Reject unknown paths, states, dimensions and runtime capability claims."""
    if not isinstance(descriptor, dict):
        raise ValueError('exact code-owned design source descriptor required')
    identity = descriptor.get('id')
    if not isinstance(identity, str) or identity not in expected_descriptors():
        raise ValueError('unknown local design descriptor state/viewport')
    if not _same(descriptor, expected_descriptors()[identity]):
        raise ValueError('local design descriptor differs from code-owned source/state/viewport scope')
    return copy.deepcopy(descriptor)


def expected_mapping():
    """Exact supplement traceability beside the unchanged parent design refs.

    Source entries come from this module's reviewed design descriptors. The map
    links requirements to nine approved inputs; it grants no execution capability.
    """
    entries = {}
    for descriptor in expected_descriptors().values():
        page, entry = descriptor['page'], descriptor['entry']
        if page in entries and entries[page] != entry:
            raise ValueError('code-owned supplement descriptors disagree on page entry')
        entries[page] = entry
    return {'supplement_id': SUPPLEMENT_ID,
            'approval_record': {'path': RECORD, 'sha256': RECORD_SHA256, 'bytes': RECORD_BYTES},
            'manifest': copy.deepcopy(MANIFEST), 'source_entries': entries,
            'source_count': len(expected_sources())}


def validate_mapping(mapping):
    """Reject altered, incomplete or self-authorizing requirement supplement maps.

    Call assess(root) separately to verify the actual approved source bytes. This
    validator checks only an exact requirement-to-supplement reference identity.
    """
    if not isinstance(mapping, dict):
        raise ValueError('exact code-owned supplement mapping required')
    try:
        matches = _same(mapping, expected_mapping())
    except (TypeError, ValueError) as error:
        raise ValueError('supplement mapping must contain exact JSON fields') from error
    if not matches:
        raise ValueError('supplement mapping differs from exact approval/manifest/nine-source entries')
    return copy.deepcopy(mapping)


def _approved_inputs(root):
    """Validate immutable parents without widening existing global authorities."""
    root = Path(root)
    if root.is_symlink():
        raise ValueError('symlink evidence root forbidden')
    paths = {RECORD, MANIFEST['path'], PRIOR_REVIEW['path'], *expected_sources(), *PARENTS}
    record = _load(_file(root, RECORD, {'sha256': RECORD_SHA256, 'bytes': RECORD_BYTES}))
    fixed = {'schema_version': 1, 'kind': 'user-approved-three-page-layout-supplement/v1',
             'approval_id': 'layout-refinements-design-user-approval-20261107', 'status': 'approved_by_user',
             'decision_date': '2026-11-07', 'decision_basis': 'explicit_user_message',
             'user_confirmation': '目前没有问题了，开始做更新修改吧', 'reviewed_entry': ENTRY,
             'target_id': TARGET_ID, 'composition_id': COMPOSITION_ID, 'supplement_id': SUPPLEMENT_ID,
             'parent_approvals': PARENTS, 'approved_source_count': 9, 'approved_sources': expected_sources(),
             'approved_manifest': MANIFEST, 'approved_scope': SCOPES, 'approved_viewports': VIEWPORTS,
             'prior_independent_design_review': PRIOR_REVIEW, 'not_granted': NOT_GRANTED,
             'captured_pending_labels': 'Frozen source/manifest review labels retain original captured bytes; this independent decision establishes the exact user-approved source identity.'}
    if not _same(record, fixed):
        raise ValueError('exact explicit user decision fields and nine-source closure required')
    for relative, fact in expected_sources().items():
        _file(root, relative, fact)
    manifest = _load(_file(root, MANIFEST['path'], MANIFEST))
    for key, expected in {'schema_version': 1, 'kind': 'proposed-three-page-layout-refinement',
                          'status': 'pending_user_review', 'entry': ENTRY, 'parent_target': TARGET_ID,
                          'parent_composition': COMPOSITION_ID, 'sources': expected_sources(),
                          'client_viewports': VIEWPORTS, 'changes': SCOPES,
                          'runtime_package_modified': False, 'product_completion': 'incomplete',
                          'next_stage_allowed': False}.items():
        if not _same(manifest.get(key), expected):
            raise ValueError('frozen supplement manifest scope differs: ' + key)
    for relative, fact in PARENTS.items():
        _file(root, relative, fact)
    complete = current_record(root)  # Existing strict 61-source authority remains authoritative.
    paths.update(complete['design_sources'])
    paths.update(complete['runtime_dependencies'])
    paths.update(complete['authority_dependencies'])
    for relative, count in [('docs/development/design-approval-client-layout-20261105.json', 3),
                            ('docs/development/design-approval-preview-20261104.json', 5)]:
        parent = _load(safe_path(root, relative))
        if parent['status'] != 'approved_by_user' or len(parent['approved_sources']) != count:
            raise ValueError('exact C1/C35 parent source approval required')
        for source, fact in parent['approved_sources'].items():
            _file(root, source, fact)
            paths.add(source)
    _file(root, PRIOR_REVIEW['path'], PRIOR_REVIEW)
    # Manifest parent links remain frozen and must still name the validated inputs.
    for parent in manifest['parent_sources'].values():
        _file(root, parent['path'], parent)
        paths.add(parent['path'])
    return paths


def approval_inputs(root=ROOT):
    """Validated fixture inputs only; this is never a product capture manifest."""
    return set(_approved_inputs(root))


def assess(root=ROOT, descriptor=None):
    """Assess local source readiness only; no receipt can self-authorize UI success."""
    result = {'schema_version': 1, 'kind': 'scoped-layout-source-authority/v1',
              'supplement_id': SUPPLEMENT_ID, 'target_id': TARGET_ID, 'composition_id': COMPOSITION_ID,
              'status': 'failed', 'source_ready': False, 'source_scope': 'three_page_supplement_only',
              'record': {'path': RECORD, 'sha256': RECORD_SHA256, 'bytes': RECORD_BYTES},
              'entry': ENTRY, 'approved_source_count': 9, 'approved_manifest': copy.deepcopy(MANIFEST),
              'parent_source_counts': {'complete': 61, 'C1': 3, 'C35': 5},
              'descriptor_validated': False, 'component_evidence_status': 'not_inspected',
              'reference_approval_granted': False, 'product_alignment_approved': False,
              'real_component_verified': False, 'native_verified': False, 'release_accepted': False,
              'product_completion': 'incomplete', 'next_stage_allowed': False,
              'not_granted': list(NOT_GRANTED), 'errors': []}
    try:
        _approved_inputs(root)
        if descriptor is not None:
            result['descriptor'] = validate_descriptor(descriptor)
            result['descriptor_validated'] = True
        result.update(status='approved_by_user', source_ready=True)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        result['errors'].append('local layout source approval invalid: ' + str(error))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--descriptor', type=Path, help='validate one exact code-owned design descriptor')
    parser.add_argument('--list-descriptors', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    try:
        descriptor = _load(args.descriptor) if args.descriptor else None
        result = assess(args.root, descriptor)
    except (OSError, ValueError, TypeError) as error:
        result = assess(args.root)
        result.update(status='failed', source_ready=False)
        result['errors'].append('invalid descriptor input: ' + str(error))
    if args.list_descriptors:
        result['design_only_descriptors'] = expected_descriptors()
    rendered = json.dumps(result, ensure_ascii=False, indent=2) + '\n'
    if args.output:
        output = args.output.absolute()
        if output.is_symlink() or not output.is_relative_to(args.root.absolute() / '.build'):
            parser.error('authority output must be a nonsymlink file under the selected root .build')
        try:
            safe_path(args.root, str(output.relative_to(args.root.absolute())))
        except ValueError as error:
            parser.error(str(error))
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered, encoding='utf-8')
    print(rendered, end='')
    return 0 if result['source_ready'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
