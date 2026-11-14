#!/usr/bin/env python3
"""Resolve an explicit source-version decision without granting runtime approval.

The captured manifest/catalogue remains pending_review. A mutable consumer JSON
cannot authorize this decision: reviewed code pins both the complete record's
bytes and the original 57-source closure, then inspects every current file.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import re

ROOT = Path(__file__).resolve().parents[2]
RECORD = 'docs/development/design-approval-guided-20261031.json'
RECORD_SHA256 = 'bb68cdc9fa93f1b0b58e1d716e5269e8f17a989737acabeb6c8e5217764020b1'
TARGET = 'guided-service-reading-20261030'
ENTRY = 'docs/design/first-product-v1/candidates/' + TARGET + '/index.html'
DESIGN_CLOSURE_SHA256 = '5d3ab428f7af2c88b2a96293c20d14a6729fdcc0c12fff843bac298482255e9a'
NOT_GRANTED = ['independently_reviewed_reference_catalogue', 'per_state_reference_png_approval',
               'product_component_alignment', 'packaged_native_alignment',
               'real_service_execution', 'real_game_execution', 'release_acceptance']
FIXED = {
    'schema_version': 1, 'approval_id': 'guided-source-user-approval-20261031',
    'approval_type': 'whole_reviewed_guided_source_version', 'target_id': TARGET,
    'entry': ENTRY, 'status': 'approved_by_user', 'decision_date': '2026-10-31',
    'decision_basis': 'explicit_user_message',
    'user_message': '确定当前这套审核通过，其他页面你怎么确保跟我们现在设计稿一致呢？',
    'approved_scope': 'whole_reviewed_guided_source_version',
    'captured_candidate_status': 'pending_review',
    'parent_baseline': {'id': 'FP1-20261031-R76',
                        'sha256': '953a793fee29e910c1fdaa5f30c3832f518f2cbf044cc2256542a44e8276038f'},
    'design_source_count': 57, 'design_closure_sha256': DESIGN_CLOSURE_SHA256,
    'not_granted': NOT_GRANTED,
    'preservation': 'Captured pending_review labels and the complete 57-source bytes remain unchanged. This decision approves the reviewed source version and does not execute tests or grant the excluded approvals.',
}

# The legacy decision above retains its original 57-source semantics. The current
# decision composes that immutable parent with the separately reviewed R79 history
# overlay. Neither captured manifest is rewritten or promoted in place.
CURRENT_RECORD = 'docs/development/design-approval-complete-20261031.json'
CURRENT_RECORD_SHA256 = '2aefa59a3edf6adc9fa1287378702b026f7a4029660bf2c23f4780abac9beacc'
CURRENT_COMPOSITION_ID = 'complete-guided-history-20261031'
CURRENT_DESIGN_CLOSURE_SHA256 = '72e788d89b489eded79caafd836177b67fa04b463861bf96b110da05e0257ffd'
HISTORY_TARGET = 'history-session-paging-20261031'
HISTORY_ROOT = 'docs/design/first-product-v1/candidates/' + HISTORY_TARGET
CURRENT_HISTORY_SOURCES = {
    HISTORY_ROOT + '/index.html': '6c2cdfa3478822d8be6fed973b126cee82805b8a4a484af29e0df80155eab236',
    HISTORY_ROOT + '/history.js': '928bf2981914a4953fc05fe5f5557c4ccc8246586120039a0c26084a2ca81dce',
    HISTORY_ROOT + '/history.css': 'd9fc1490003359e745649904edb88b14e4f59b505ec63a27f85201523e9dfa6e',
    HISTORY_ROOT + '/manifest.json': '135289b6204f5bcf178f340e9e3fdff3f1384a5b51e154cd4a9bd6e7cdccfc34',
}
CURRENT_RUNTIME_DEPENDENCIES = {
    'docs/design/first-product-v1/assets/wallpaper-frame.png':
        '64f704c02b5172f7b18ae09e683c883f6f0913b4ea89b7399f975e9938cd6a8f',
}
CURRENT_AUTHORITY_DEPENDENCIES = {RECORD: RECORD_SHA256}
CURRENT_ROUTES = {
    'default': {'entry': ENTRY, 'query': {}},
    'main:history': {'entry': HISTORY_ROOT + '/index.html', 'query': {'scenario': 'history'}},
}
CURRENT_FIXED = {
    'schema_version': 1, 'approval_id': 'complete-design-source-user-approval-20261031',
    'approval_type': 'whole_current_reviewed_design_composition', 'target_id': TARGET,
    'composition_id': CURRENT_COMPOSITION_ID, 'entry': ENTRY, 'status': 'approved_by_user',
    'decision_date': '2026-10-31', 'decision_basis': 'explicit_user_message',
    'user_message': '可以了，确定所有设计稿 ok 了，重新设定一下门禁，确定能被审核卡住，然后，确定对应的客户端设计也改过了',
    'approved_scope': 'current_guided_and_history_source_composition',
    'parent_baseline': {'id': 'FP1-20261031-R79',
                        'sha256': 'b5754c102f3ba7c916385ed9128e1a6a1dcce139ab1b056d237bc12805a3cfcf'},
    'design_source_count': 61, 'design_closure_sha256': CURRENT_DESIGN_CLOSURE_SHA256,
    'runtime_dependencies': CURRENT_RUNTIME_DEPENDENCIES,
    'authority_dependencies': CURRENT_AUTHORITY_DEPENDENCIES,
    'routes': CURRENT_ROUTES,
    'captured_candidate_statuses': {TARGET: 'pending_review', HISTORY_TARGET: 'pending_review'},
    'not_granted': NOT_GRANTED,
    'preservation': 'The original guided57 decision and all 61 reviewed design-source bytes, including captured pending_review labels, remain unchanged. This current source-composition decision does not grant independent references, product/native alignment, real service/game execution or release acceptance.',
}
SHA = re.compile(r'[0-9a-f]{64}\Z')


def _identity(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                                     separators=(',', ':')).encode()).hexdigest()


def _digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _path(root, relative):
    if (not isinstance(relative, str) or not relative or relative != relative.strip()
            or any(ord(c) < 32 or ord(c) == 127 for c in relative)
            or any(c in relative for c in ('\\', ':', '?', '#'))
            or PurePosixPath(relative).is_absolute()
            or any(part in ('', '.', '..') for part in relative.split('/'))):
        raise ValueError('safe relative POSIX approval/source path required')
    path = Path(root).absolute()
    if path.is_symlink():
        raise ValueError('symlink approval root forbidden')
    for part in PurePosixPath(relative).parts:
        path /= part
        if path.is_symlink():
            raise ValueError('symlink approval/source forbidden')
    path.resolve().relative_to(Path(root).resolve())
    return path


def _unique(pairs):
    data = {}
    for key, value in pairs:
        if key in data:
            raise ValueError('duplicate approval JSON key: ' + key)
        data[key] = value
    return data


def _record(root):
    path = _path(root, RECORD)
    data = json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON approval value: ' + value)))
    if _digest(path) != RECORD_SHA256:
        raise ValueError('approval record raw hash differs from code-owned explicit decision')
    if not isinstance(data, dict) or set(data) != {*FIXED, 'design_sources'}:
        raise ValueError('exact source-version approval record fields required')
    for key, value in FIXED.items():
        if _identity(data[key]) != _identity(value):
            raise ValueError('explicit source-version approval identity differs: ' + key)
    sources = data['design_sources']
    if (not isinstance(sources, dict) or len(sources) != 57
            or _identity(sources) != DESIGN_CLOSURE_SHA256):
        raise ValueError('approval must bind the original complete 57-source map')
    for relative, expected in sources.items():
        if not isinstance(expected, str) or not SHA.fullmatch(expected):
            raise ValueError('exact approved source hash required: ' + relative)
        if _digest(_path(root, relative)) != expected:
            raise ValueError('approved source bytes changed: ' + relative)
    return data


def source_approval(root=ROOT):
    """Return effective authority and independent captured/runtime boundaries."""
    errors = []
    try:
        _record(root)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append('current guided source-version user approval invalid: ' + str(error))
    return {'target_id': TARGET, 'status': 'blocked' if errors else 'approved_by_user',
            'record': {'path': RECORD, 'sha256': RECORD_SHA256},
            'design_closure_sha256': DESIGN_CLOSURE_SHA256,
            'captured_candidate_status': 'pending_review',
            'product_tests_executed': False, 'product_alignment_approved': False,
            'not_granted': list(NOT_GRANTED), 'errors': errors}


def user_approval_gaps(root=ROOT):
    return source_approval(root)['errors']


def approval_inputs(root=ROOT):
    """Exact verified inputs for isolated guard fixtures, never a capture list."""
    return {RECORD, *_record(root)['design_sources']}


def current_record(root=ROOT):
    """Inspect the code-pinned complete decision and every composed input.

    Mutable manifests may describe dependencies but cannot authorize new bytes.
    Runtime and authority dependencies are bound separately from the 61 design
    files, preserving the exact scope of the original user's 57-source decision.
    """
    path = _path(root, CURRENT_RECORD)
    data = json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON approval value: ' + value)))
    if _digest(path) != CURRENT_RECORD_SHA256:
        raise ValueError('complete approval record raw hash differs from code-owned explicit decision')
    if not isinstance(data, dict) or set(data) != {*CURRENT_FIXED, 'design_sources'}:
        raise ValueError('exact complete composition approval record fields required')
    for key, value in CURRENT_FIXED.items():
        if _identity(data[key]) != _identity(value):
            raise ValueError('explicit complete composition approval identity differs: ' + key)
    sources = {**_record(root)['design_sources'], **CURRENT_HISTORY_SOURCES}
    if (len(sources) != 61 or _identity(sources) != CURRENT_DESIGN_CLOSURE_SHA256
            or data.get('design_sources') != sources):
        raise ValueError('complete approval must bind the original guided57 plus exact R79 history4')
    for relative, expected in sources.items():
        if not isinstance(expected, str) or not SHA.fullmatch(expected):
            raise ValueError('exact complete design source hash required: ' + relative)
        if _digest(_path(root, relative)) != expected:
            raise ValueError('complete approved source bytes changed: ' + relative)
    for group in ('runtime_dependencies', 'authority_dependencies'):
        for relative, expected in data[group].items():
            if _digest(_path(root, relative)) != expected:
                raise ValueError('complete approval dependency bytes changed: ' + relative)
    return data


def current_source_approval(root=ROOT):
    """Resolve current source approval without granting references or execution."""
    errors, data = [], None
    try:
        data = current_record(root)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append('current complete source-version user approval invalid: ' + str(error))
    return {
        'target_id': TARGET, 'composition_id': CURRENT_COMPOSITION_ID,
        'status': 'blocked' if errors else 'approved_by_user',
        'record': {'path': CURRENT_RECORD, 'sha256': CURRENT_RECORD_SHA256},
        'design_source_count': 61, 'design_closure_sha256': CURRENT_DESIGN_CLOSURE_SHA256,
        'design_sources': copy.deepcopy(data['design_sources']) if data is not None else {},
        'runtime_dependencies': copy.deepcopy(CURRENT_RUNTIME_DEPENDENCIES),
        'authority_dependencies': copy.deepcopy(CURRENT_AUTHORITY_DEPENDENCIES),
        'routes': copy.deepcopy(CURRENT_ROUTES),
        'captured_candidate_status': 'pending_review',
        'captured_candidate_statuses': copy.deepcopy(CURRENT_FIXED['captured_candidate_statuses']),
        'product_tests_executed': False, 'product_alignment_approved': False,
        'not_granted': list(NOT_GRANTED), 'errors': errors,
    }


def current_user_approval_gaps(root=ROOT):
    return current_source_approval(root)['errors']


def current_approval_inputs(root=ROOT):
    """All 64 source/runtime/authority inputs for isolated current guard fixtures."""
    data = current_record(root)
    return {CURRENT_RECORD, *data['design_sources'], *data['runtime_dependencies'],
            *data['authority_dependencies']}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--json', action='store_true')
    parser.add_argument('--current', action='store_true', help='Inspect the complete current composition; legacy parent is the default')
    args = parser.parse_args(argv)
    result = current_source_approval(args.root) if args.current else source_approval(args.root)
    print(json.dumps(result, ensure_ascii=False, indent=2) if args.json else result['status'])
    return bool(result['errors'])


if __name__ == '__main__':
    raise SystemExit(main())
