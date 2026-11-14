#!/usr/bin/env python3
"""Execute finite production-component binding checks; never approve Native.

BJ obligations are cumulative with the old 153/RW/901 inventories. The seven
component observations below are a deliberately finite executable capability,
not a claim that every layer of all eighteen journeys has been implemented.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import sys

from gate_core import source_files
from binding_phase_guard import (CHANGE, POLICY, load, path_at, ref, sha,
                                 checked_ref, tree_sha, products)

ROOT = Path(__file__).resolve().parents[2]
PLAN = 'docs/development/binding-journey-plan.json'
CHECK_ID = 'binding-journey-execution'
PHASE_CHECK = 'binding-phase-admission'
STORAGE_V5_CHECKS = frozenset({'storage-schema-v5-tests', 'storage-schema-v5-contract',
                               'storage-schema-v5-boundaries', 'storage-schema-v5-runtime-inspection'})
COMPONENT_CASES = (
    'BJC-01-system-confirmation-state', 'BJC-02-same-target-reuse',
    'BJC-03-association-progress', 'BJC-04-selection-draft-confirmation',
    'BJC-05-source-details', 'BJC-06-remember-selection',
    'BJC-07-no-unrequested-picker',
)
CASE_JOURNEYS = {
    COMPONENT_CASES[0]: ('BJ-02',), COMPONENT_CASES[1]: ('BJ-05',),
    COMPONENT_CASES[2]: ('BJ-06',), COMPONENT_CASES[3]: ('BJ-08',),
    COMPONENT_CASES[4]: ('BJ-10',), COMPONENT_CASES[5]: ('BJ-09',),
    COMPONENT_CASES[6]: ('BJ-04',),
}
DIAGNOSTIC_SCENES = dict(zip(COMPONENT_CASES, (
    'BJS-02-first-confirmation', 'BJS-05-reuse', 'BJS-06-real-stage-order',
    'BJS-08-draft-later', 'BJS-10-details', 'BJS-09-owned-remember', 'BJS-04-idle-refresh-details')))
DIAGNOSTIC_METHODS = (
    ('snapshot', 'targets', 'reviewAssociation', 'previewCapability', 'startWindowPreview'),
    ('snapshot', 'targets', 'reviewAssociation', 'previewCapability', 'startWindowPreview'),
    ('snapshot', 'targets', 'reviewAssociation', 'associate', 'associationProgress', 'previewCapability', 'startWindowPreview'),
    ('snapshot', 'targets', 'reviewAssociation', 'associate', 'associationProgress', 'sourceSelection', 'beginReading', 'previewCapability', 'startWindowPreview'),
    ('snapshot', 'targets', 'reviewAssociation', 'associate', 'associationProgress', 'sourceSelection', 'previewCapability', 'startWindowPreview'),
    ('snapshot', 'targets', 'reviewAssociation', 'associate', 'associationProgress', 'sourceSelection', 'beginReading', 'previewCapability', 'startWindowPreview'),
    ('snapshot', 'targets', 'reviewAssociation', 'previewCapability', 'startWindowPreview', 'selectWindowPreview', 'controlWindowPreview'),
)
REQUIRED_LAYERS = {
    'BJ-01': ('component', 'native'), 'BJ-02': ('domain', 'component', 'native'),
    'BJ-03': ('component', 'native'), 'BJ-04': ('component', 'native'),
    'BJ-05': ('domain', 'component', 'native'),
    'BJ-06': ('domain', 'component', 'native'),
    'BJ-07': ('domain', 'component', 'native'),
    'BJ-08': ('domain', 'storage', 'component', 'native'),
    'BJ-09': ('domain', 'storage', 'component', 'native'),
    'BJ-10': ('domain', 'component'), 'BJ-11': ('storage',),
    'BJ-12': ('domain', 'storage'), 'BJ-13': ('domain', 'component', 'native'),
    'BJ-14': ('design', 'component', 'native'),
    'BJ-15': ('domain', 'component', 'native'),
    'BJ-16': ('game', 'service', 'native'),
    'BJ-17': ('domain', 'storage', 'component'), 'BJ-18': ('guard',),
}
EXPORTER = 'scripts/report/export_binding_journey_projection.py'
RUNNER = 'scripts/report/verify_binding_journey.mjs'
COMMAND = ('python3', 'scripts/quality/binding_journey.py', '--execute')
NONCE = re.compile(r'[a-f0-9]{32}\Z')
EXACT_EXECUTION_OBLIGATION_IDS = ('EX-LIB-GLOBAL', 'EX-LIB-SCOPED', 'EX-LIFE-CORE',
    'EX-LIFE-NATIVE-UNIT', 'EX-LEASE-NATIVE-UNIT', 'EX-STORAGE-TIMELINE',
    'EX-NATIVE-SUCCESS-ROLES', 'EX-NATIVE-PROGRESS-TRACES', 'EX-LIFE-SIGNED-NATIVE')
RUST_LAYERS = {
    'domain': {'package': 'app-core', 'target': 'binding_journey',
               'source': 'crates/app-core/tests/binding_journey.rs'},
    'storage': {'package': 'app-infra', 'target': 'source_selection_sets',
                'source': 'crates/app-infra/tests/source_selection_sets.rs'},
}
FIXED_NATIVE_UNIT_GROUPS = {
    'EX-LIFE-NATIVE-UNIT': tuple('platform::session_preview_coordinator::preview_cancellation_tests::' + name for name in (
        'bj_07_active_session_preview_reconciliation', 'bj_07_preview_authority_fence_races',
        'bj_07_preview_reconciliation_singleflight', 'bj_07_preview_no_implicit_picker',
        'bj_07_preview_existing_healthy_and_paused')),
    'EX-LEASE-NATIVE-UNIT': ('platform::window_preview::tests::bj_07_nonblocking_retirement_admission',
                             'platform::window_preview::tests::bj_07_old_retirement_cannot_release_new_candidate'),
}
TRANSPORT_PRODUCER = {'package': 'app-infra', 'target': 'binding_journey',
                      'source': 'crates/app-infra/tests/binding_journey.rs',
                      'test_name': 'bj_component_transport'}
NATIVE_STEPS = {
    'BJ-01': ('before', 'selected', 'associated'),
    'BJ-02': ('confirmation',), 'BJ-03': ('candidate', 'associated'),
    'BJ-04': ('before', 'after', 'cancel_intent', 'cancelled', 'timeout_intent', 'timeout'),
    'BJ-05': ('before', 'reviewed', 'after'),
    'BJ-06': ('candidate',),
    'BJ-07': ('before', 'closed', 'cancelled'),
    'BJ-08': ('before', 'draft', 'committed'),
    'BJ-09': ('before', 'temporary', 'save_intent', 'saved', 'saved_reopened', 'unowned'),
    'BJ-13': ('before', 'completed'),
    'BJ-14': ('dialog_default', 'dialog_narrow'),
    'BJ-15': ('before', 'new_frame', 'failed', 'revoked'),
}
SCENE_NAMES = {
    '01': ('library-window', 'library-process'), '02': ('first-confirmation', 'tcc'),
    '03': ('pairing-transfer',), '04': ('idle-refresh-details', 'picker-cancel', 'picker-timeout'),
    '05': ('reuse', 'switch-cancel', 'stale-review'), '06': ('real-stage-order', 'stale-events'),
    '07': ('cancel-candidate', 'close-operation'),
    '08': ('empty', 'draft-later', 'commit-set', 'commit-failure'),
    '09': ('owned-remember', 'unowned-remember', 'temporary-saved', 'requires-review'),
    '10': ('details',), '13': ('effects-order', 'effects-retry', 'stale-effects'),
    '14': ('layout-normal', 'layout-narrow', 'layout-200', 'other-bottle-warning'),
    '15': ('frame-state', 'revoke', 'late-metadata'),
    '17': ('association-retry', 'source-query-retry', 'source-expired', 'submit-retry'),
}
ORIGINAL_COMPONENT_SCENES = {f'BJS-{number}-{name}': ('BJ-' + number, name)
                             for number, names in SCENE_NAMES.items() for name in names}
LIBRARY_OWNER_SCENES = {'BJS-01-library-global-owner': ('BJ-01', 'library-global-owner'),
                        'BJS-01-scoped-reconnect-owner': ('BJ-01', 'scoped-reconnect-owner')}
COMPONENT_SCENES = {**ORIGINAL_COMPONENT_SCENES, **LIBRARY_OWNER_SCENES}
SCENE_FALLBACKS = frozenset({'library-window', 'library-process', 'first-confirmation', 'idle-refresh-details',
                            'reuse', 'cancel-candidate', 'empty', 'draft-later', 'owned-remember',
                            'layout-normal', 'layout-narrow', 'layout-200'})


def required_scene_methods(name):
    methods = ['snapshot', 'targets', 'reviewAssociation', 'previewCapability', 'startWindowPreview', 'controlWindowPreview']
    if name not in {'library-window', 'library-process', 'first-confirmation', 'tcc', 'reuse', 'cancel-candidate', 'switch-cancel',
                    'idle-refresh-details', 'picker-cancel', 'picker-timeout', 'layout-normal', 'layout-narrow', 'layout-200',
                    'other-bottle-warning', 'frame-state', 'revoke', 'late-metadata'}:
        methods += ['associate', 'associationProgress']
    if name in {'empty', 'draft-later', 'commit-set', 'commit-failure', 'owned-remember', 'unowned-remember', 'temporary-saved',
                'requires-review', 'details', 'effects-order', 'effects-retry', 'stale-effects', 'submit-retry', 'source-expired', 'source-query-retry'}:
        methods += ['sourceSelection', 'beginReading']
    if name in {'picker-cancel', 'picker-timeout', 'pairing-transfer'}:
        methods += ['selectWindowPreview']
    if name in {'pairing-transfer', 'effects-order', 'effects-retry', 'stale-effects'}:
        methods += ['previewSession', 'openWindow']
    return methods


def actual_scene_inputs(steps):
    if not steps:
        return {'protected_session_id': None, 'committed_operation_ids': [], 'replacement_session_id': None,
                'current_operation_id': None, 'current_session_id': None}
    messages = steps[-1]['facts']['rpcMessages']
    reset = next((row for row in messages if row.get('action') == 'reset' and row.get('outcome') == 'ok'), None)
    initial = ((reset or {}).get('response') or {}).get('initial_snapshot') or {}
    protected = (initial.get('session') or {}).get('sessionId')
    if protected is None and steps[0]['facts'].get('transportMode') == 'controlled_diagnostic':
        protected = (steps[0]['facts']['snapshot'].get('session') or {}).get('sessionId')
    find = lambda action: next((step for step in steps if step['action'] == action), None)
    committed_messages = find('association-returned')['facts']['rpcMessages'] if find('association-returned') else messages
    def stages(rows):
        return [event for message in rows for event in message['events'] if event.get('kind') == 'association_stage'
                and type(event.get('operation_id')) is str and event['operation_id']
                and type(event.get('stage')) is str and event['stage']]
    def new_messages(action):
        step = find(action)
        if step is None:
            return []
        index = steps.index(step)
        earlier = len(steps[index - 1]['facts']['rpcMessages']) if index else 0
        return step['facts']['rpcMessages'][earlier:]
    def published_session(rows):
        updates = [update for message in rows for update in message['publications']]
        return (((updates[-1] if updates else {}).get('snapshot') or {}).get('session') or {}).get('sessionId')
    current = new_messages('current-operation')
    current_stages = stages(current)
    return {'protected_session_id': protected,
            'committed_operation_ids': list(dict.fromkeys(row['operation_id'] for row in stages(committed_messages))),
            'replacement_session_id': published_session(new_messages('session-replaced')),
            'current_operation_id': current_stages[-1]['operation_id'] if current_stages else None,
            'current_session_id': published_session(current)}


def source_panel_evidence(step):
    """Join the visible source rows to the actual successful typed RPC result."""
    if not isinstance(step, dict):
        return False
    facts, dom = step.get('facts', {}), step.get('dom', {})
    snapshot = facts.get('snapshot') or {}
    session = snapshot.get('session') or {}
    replies = [row for row in facts.get('rpcMessages', []) if row.get('action') == 'invoke'
               and row.get('method') == 'sourceSelection' and row.get('outcome') == 'ok']
    projection = (replies[-1].get('response') or {}) if replies else {}
    profile = projection.get('profile') or {}
    sources = projection.get('sources')
    if (not session.get('sessionId') or projection.get('sessionId') != session['sessionId']
            or any(type(profile.get(key)) is not str or not profile[key] for key in ('profileId', 'serverInstanceId'))
            or type(profile.get('profileGeneration')) is not int or profile['profileGeneration'] < 0
            or any(profile.get(key) != snapshot.get(key) for key in
                   ('profileId', 'serverInstanceId', 'profileGeneration'))
            or not isinstance(sources, list) or not sources):
        return False
    ids = [row.get('sourceId') for row in sources]
    rows = dom.get('sourceRows', [])
    boxes = [row for row in dom.get('checkboxes', []) if '记住' not in row.get('label', '')]
    if (any(type(identity) is not str or not identity for identity in ids)
            or len(set(ids)) != len(ids) or len(rows) != len(ids) or len(boxes) != len(ids)
            or len({row.get('sourceId') for row in rows}) != len(ids)
            or len({row.get('sourceId') for row in boxes}) != len(ids)):
        return False
    return (all(sum(row.get('sourceId') == identity and row.get('checkboxCount') == 1
                    and row.get('visibleCheckboxCount') == 1 and row.get('enabledCheckboxCount') == 1
                    for row in rows) == 1
                and sum(row.get('sourceId') == identity and row.get('disabled') is False for row in boxes) == 1
                for identity in ids)
            and all(sum(row.get('label') == label for row in dom.get('buttons', [])) == 1
                    for label in ('开始阅读', '稍后选择')))


def candidate_ready_evidence(step):
    if not source_panel_evidence(step):
        return False
    facts = step['facts']
    replies = [row for row in facts.get('rpcMessages', []) if row.get('action') == 'invoke'
               and row.get('method') == 'associate' and row.get('outcome') == 'ok']
    receipt = (replies[-1].get('response') or {}) if replies else {}
    operation = receipt.get('operationId')
    commits = [row for row in facts.get('methodCalls', []) if row.get('method') == 'beginReading'
               or row.get('method') == 'dispatch' and ((row.get('args') or [{}])[0].get('command') or {}).get('kind')
               in {'select_source', 'select_sources', 'begin_reading'}]
    return (type(operation) is str and bool(operation)
            and receipt.get('sessionId') == facts['snapshot']['session']['sessionId']
            and any(row.get('kind') == 'production_side_effect'
                    and row.get('event', {}).get('kind') == 'association_stage'
                    and row['event'].get('operation_id') == operation
                    and row['event'].get('stage') == 'candidates_ready' for row in facts.get('events', []))
            and not commits)


def scoped_source_discharge(case, scene, nonce):
    scopes = ['advanced-source-typed-DTO-disclosure', 'source-u64-typed-DTO-projection']
    if not case or not scene or case.get('id') != COMPONENT_CASES[4] or scene.get('id') != 'BJS-10-details':
        return None
    observations = scene.get('observations') or {}
    steps = observations.get('steps') or []
    details = steps[-1].get('source_details') or {} if steps else {}
    if (case.get('status') != 'blocked' or (case.get('observations') or {}).get('blocked_obligations') != scopes
            or scene.get('status') != 'passed' or observations.get('transport_mode') != 'production_rpc'
            or details.get('blocked_obligations') != [] or details.get('advancedAvailable') is not True
            or details.get('precisionAvailable') is not True or not NONCE.fullmatch(nonce)):
        return None
    return {'kind': 'same-run-source-detail-discharge/v1', 'nonce': nonce, 'case_id': COMPONENT_CASES[4],
            'case_trace': case['trace'], 'scene_id': 'BJS-10-details', 'scene_trace': scene['trace'], 'obligations': scopes}


def library_owner_checks(name, steps):
    """Derive library ownership from actual seed results and full target IDs."""
    find = lambda action: next((step for step in steps if step.get('action') == action), {})
    first, last = steps[0], steps[-1]
    replies = last['facts'].get('rpcMessages', [])
    reset = next((row.get('response') or {} for row in replies if row.get('action') == 'reset'
                  and row.get('outcome') == 'ok'), {})
    initial = reset.get('initial_snapshot') or {}
    games, seeds = initial.get('games', []), reset.get('seed_commands', [])
    def key(target):
        return tuple(target.get(field) for field in ('environmentKey', 'canonicalExePath', 'architecture'))
    keys = [key(game.get('target') or {}) for game in games]
    seeded_ids = []
    valid_seed = len(games) >= 2 and len(set(keys)) == len(keys) and all(all(type(v) is str and v for v in identity) for identity in keys)
    for row in seeds:
        command = (row.get('request') or {}).get('command') or {}
        result = row.get('response') or {}
        items, results = (command.get('payload') or {}).get('items'), result.get('gameResults')
        valid_seed = valid_seed and command.get('kind') == 'save_games' and isinstance(items, list) and isinstance(results, list) and len(items) == len(results)
        if not isinstance(items, list) or not isinstance(results, list):
            continue
        for item in items:
            matches = [value for value in results if value.get('itemId') == item.get('itemId')
                       and type(value.get('rowRevision')) is int and value['rowRevision'] > 0
                       and type(value.get('reused')) is bool]
            actual = next((game for game in games if key(game.get('target') or {}) == key((item.get('game') or {}).get('target') or {})), None)
            valid_seed = valid_seed and len(matches) == 1 and actual is not None and matches[0].get('gameId') == actual.get('gameId')
            if len(matches) == 1: seeded_ids.append(matches[0].get('gameId'))
    valid_seed = bool(valid_seed and set(seeded_ids) == {game.get('gameId') for game in games})
    reviews = [row.get('response') or {} for row in replies if row.get('action') == 'invoke'
               and row.get('method') == 'reviewAssociation' and row.get('outcome') == 'ok']
    review = reviews[-1] if reviews else {}
    candidate = review.get('candidate') or {}
    owner_key = (candidate.get('environmentKey'), candidate.get('executablePath'), candidate.get('architecture'))
    owners = [game for game in games if key(game.get('target') or {}) == owner_key]
    owner = owners[0].get('gameId') if len(owners) == 1 else None
    selected = [game.get('gameId') for game in games if any(button.get('pressed') == 'true'
                and game.get('displayName') and game['displayName'] in button.get('label', '') for button in first['dom'].get('buttons', []))]
    selected_id = selected[0] if len(selected) == 1 else None
    calls = last['facts'].get('methodCalls', [])
    review_calls = [row for row in calls if row.get('method') == 'reviewAssociation']
    query = (review_calls[-1].get('args') or [{}])[0] if review_calls else {}
    saves = [row for row in calls if row.get('method') == 'dispatch'
             and ((row.get('args') or [{}])[0].get('command') or {}).get('kind') in {'save_game', 'save_games'}]
    associates = [row for row in calls if row.get('method') == 'associate']
    scope = (initial.get('session') or {}).get('gameId') if name == 'scoped-reconnect-owner' else None
    incompatible = name == 'scoped-reconnect-owner' and owner != scope
    identity_valid = (type(candidate.get('candidateId')) is str and candidate['candidateId'] == query.get('candidateId')
                      and type(candidate.get('windowsPid')) is int and candidate['windowsPid'] > 0
                      and type(candidate.get('processCreatedU64')) is str and bool(re.fullmatch(r'[1-9][0-9]*', candidate['processCreatedU64']))
                      and type(candidate.get('targetIdentitySha256')) is str and bool(re.fullmatch(r'[a-f0-9]{64}', candidate['targetIdentitySha256'])))
    expected_owner = scope if name == 'scoped-reconnect-owner' else owner
    return [('actual-two-game-SaveGames-seed', True, valid_seed),
            ('actual-library-A-selected-before-intent', True, selected_id is not None and selected_id != owner),
            ('actual-full-runtime-target-identity', True, bool(owner and identity_valid)),
            ('entry-scope-is-explicit', scope, query.get('gameId')),
            ('review-retains-correct-game-owner', expected_owner, review.get('gameId')),
            ('no-implicit-game-save-after-seed', 0, len(saves)),
            ('exact-association-count-for-scope', 0 if incompatible else 1, len(associates)),
            ('incompatible-scope-remains-visible', True, not incompatible or bool(find('scoped-incompatible').get('dom', {}).get('dialogs')))]


def scene_verdict(identity, observations):
    """Independent complete component oracle over original step facts/DOM.

    Code-owned identities/expectations cannot be reduced in verification JSON.
    Controlled diagnostic success remains blocked until the actual Rust RPC
    producer supplies that scene's real application DTOs and effects.
    """
    if identity not in COMPONENT_SCENES or not isinstance(observations, dict):
        raise ValueError('Known typed binding component scene required')
    name = COMPONENT_SCENES[identity][1]
    steps = observations.get('steps')
    if not isinstance(steps, list):
        raise ValueError('Original scene step list required')
    mode = observations.get('transport_mode')
    if mode not in {'unavailable', 'controlled_diagnostic', 'production_rpc'}:
        raise ValueError('Actual component transport mode required')
    blocked = ['actual-production-rpc-input:' + identity] if mode != 'production_rpc' else []
    if not steps:
        if mode != 'unavailable':
            raise ValueError('Executed scene cannot omit original observations')
        return {'id': identity, 'status': 'blocked', 'assertions': [], 'blocked_obligations': blocked}
    if mode == 'unavailable':
        raise ValueError('Unavailable scene cannot invent observations')
    for step in steps:
        if not isinstance(step.get('action'), str) or not isinstance(step.get('facts'), dict) or not isinstance(step.get('dom'), dict):
            raise ValueError('Typed real DOM/facts/action checkpoint required')
    first, last = steps[0], steps[-1]
    find = lambda action: next((step for step in steps if step['action'] == action), None)
    before = find('before-intent') or find('target-reviewed') or first
    checks = []
    def add(key, expected, actual):
        checks.append({'id': key, 'expected': expected, 'actual': actual,
                       'passed': json.dumps(expected, sort_keys=True) == json.dumps(actual, sort_keys=True)})
    def calls(step, methods):
        return [row for row in step['facts']['methodCalls'] if row['method'] in methods]
    def commits(step):
        return [row for row in step['facts']['methodCalls'] if row['method'] == 'beginReading'
                or row['method'] == 'dispatch' and row.get('args', [{}])[0].get('command', {}).get('kind') in
                {'select_source', 'select_sources', 'begin_reading'}]
    def choices(step):
        return [row for row in step['dom']['checkboxes'] if '记住' not in row['label']]
    def effects(step):
        return [row['event'] for row in step['facts']['events'] if row.get('kind') == 'production_side_effect']
    def diff(read, a=before, b=last):
        return len(read(b)) - len(read(a))
    def session(step):
        return (step or {}).get('facts', {}).get('snapshot', {}).get('session') or {}
    def sid(step):
        return session(step).get('sessionId')
    text = last['dom']['text']
    inputs = actual_scene_inputs(steps)
    if mode == 'production_rpc':
        if name in {'reuse', 'switch-cancel', 'cancel-candidate', 'revoke'} and not inputs['protected_session_id']:
            blocked.append('actual-protected-session-input:' + identity)
        if name == 'stale-events' and not inputs['current_operation_id']:
            blocked.append('actual-current-operation-input:' + identity)
        if name in {'close-operation', 'stale-events', 'real-stage-order'} and not inputs['committed_operation_ids']:
            blocked.append('actual-committed-operation-input:' + identity)
        if name in {'stale-review', 'stale-effects'} and not inputs['replacement_session_id']:
            blocked.append('actual-replacement-session-input:' + identity)
    clicked = lambda pattern: any(row.get('clicked') is True and re.search(pattern, row.get('label') or '') for row in last.get('ui_actions', []))
    buttons = lambda pattern: [row for row in last['dom']['buttons'] if re.search(pattern, row['label'])]
    remember = [row for row in last['dom']['checkboxes'] if '记住' in row['label']]
    if name in {'library-global-owner', 'scoped-reconnect-owner'}:
        for key, expected, actual in library_owner_checks(name, steps):
            add(key, expected, actual)
    elif name in {'library-window', 'library-process'}:
        add('library-is-entry', True, '游戏库' in first['dom']['headings'])
        add('candidate-selection-does-not-inject', 0, diff(lambda s: calls(s, ['associate', 'beginReading'])))
        add('rust-review-is-read', True, bool(calls(last, ['reviewAssociation'])))
        add('complete-identity-labels', True, all(value in text for value in ['目标完整路径', '程序架构', 'Windows 游戏进程']))
        add('preview-failure-does-not-disable-hook', True, any(not row['disabled'] for row in buttons(r'^(关联并接入|切换并接入|复用连接)')))
        if name == 'library-process':
            add('process-list-selected', True, any(row['pressed'] == 'true' for row in buttons(r'^进程列表$')))
    elif name == 'first-confirmation':
        add('first-confirmation-label', True, any(not row['disabled'] for row in buttons(r'^选择对应的游戏窗口…$')))
        add('first-confirmation-reason', True, any(bool(re.search('需要.*游戏窗口', row['text'])) for row in last['dom']['previews']))
        add('no-invented-first-frame', True, all(not row['images'] and row['dateTime'] is None and '尚无成功画面' in (row['time'] or '') for row in last['dom']['previews']))
    elif name == 'tcc':
        add('tcc-explains-recording-permission', True, any(bool(re.search('屏幕录制权限|录屏权限', row['text'])) for row in last['dom']['previews']))
        add('tcc-is-not-window-picker', True, not buttons('^选择对应的游戏窗口…$'))
        add('hook-remains-independent', True, any(not row['disabled'] for row in buttons(r'^(关联并接入|切换并接入)')))
    elif name == 'pairing-transfer':
        shared = find('candidate-shared')
        def decoded(step):
            return bool(step and any(any(image['complete'] and image['naturalWidth'] > 0 for image in row['images']) for row in step['dom']['previews']))
        add('candidate-had-decoded-frame', True, decoded(shared))
        add('transfer-does-not-open-second-picker', 0, diff(lambda s: calls(s, ['selectWindowPreview']), shared or before))
        add('session-has-decoded-frame', True, decoded(last))
        actual = [step for step in steps if step['action'] in {'session-frame', 'late-candidate-frame'}]
        add('late-candidate-frame-is-not-session', True, len(actual) == 2 and (actual[0]['dom']['previews'] or [{}])[0].get('dateTime') == (last['dom']['previews'] or [{}])[0].get('dateTime'))
    elif name == 'idle-refresh-details':
        add('no-picker-from-refresh-or-details', 0, diff(lambda s: calls(s, ['selectWindowPreview'])))
        add('no-association-from-refresh-or-details', 0, diff(lambda s: calls(s, ['associate'])))
        add('closing-details-keeps-target-panel', True, bool(last['dom']['dialogs']))
    elif name in {'picker-cancel', 'picker-timeout'}:
        count = diff(lambda s: calls(s, ['selectWindowPreview']))
        add('one-explicit-picker', 1, count)
        add('picker-has-terminal-recovery', True, not any(row['state'] in {'starting', 'requesting_permission'} for row in last['dom']['previews']) and bool(re.search('取消|超时|重新选择|选择对应', text)))
        add('picker-does-not-loop', True, count == 1)
    elif name == 'reuse':
        add('same-target-is-not-switch', False, bool(buttons('^切换并接入$')))
        add('reuse-explained', True, '复用' in text)
        add('relation-came-from-rust', True, bool(calls(last, ['reviewAssociation'])))
        if inputs['protected_session_id']:
            add('reuse-preserves-actual-session', inputs['protected_session_id'], sid(last))
    elif name in {'switch-cancel', 'cancel-candidate'}:
        add('candidate-cancel-closes-observer', 0, len(last['dom']['dialogs']))
        add('candidate-cancel-action-executed', True, clicked('^取消$'))
        if inputs['protected_session_id']:
            add('reset-session-was-rendered', inputs['protected_session_id'], sid(first))
            add('candidate-cancel-keeps-session', inputs['protected_session_id'], sid(last))
        add('candidate-cancel-does-not-inject', 0, diff(lambda s: calls(s, ['associate', 'beginReading'])))
        if name == 'switch-cancel':
            add('switch-impact-was-explicit', True, '切换' in before['dom']['text'] and '当前' in before['dom']['text'])
    elif name == 'stale-review':
        add('stale-review-action-executed', True, clicked(r'^(关联并接入|切换并接入|复用连接)'))
        if inputs['replacement_session_id']:
            add('stale-review-keeps-new-session', inputs['replacement_session_id'], sid(last))
        add('stale-review-demands-refresh', True, bool(re.search('重新核对|已变化|已失效|过期', text)))
        add('fresh-review-was-requested', True, len(calls(last, ['reviewAssociation'])) > len(calls(before, ['reviewAssociation'])))
    elif name == 'real-stage-order':
        pending = find('association-returned')
        add('operation-stage-view-is-present', True, bool(pending and len(pending['dom']['progressStages']) >= 4))
        add('promise-is-not-handshake', True, bool(pending and not any((row['current'] or row['complete']) and re.search('已连接|候选', row['text']) for row in pending['dom']['progressStages'])))
        add('stage-facts-have-origin', True, any(row['kind'] == 'association_stage' and type(row.get('operation_id')) is str and bool(row['operation_id']) and type(row.get('stage')) is str and bool(row['stage']) for row in effects(last)))
        add('core-stages-projected', True, all(any(label in step['dom']['text'] for step in steps) for label in ('窗口已关联', '自动接入', '选择正文')))
        handshake = find('stage-handshake')
        add('handshake-fact-drives-connected-stage', True, bool(handshake and any(row['current'] and re.search('已连接', row['text']) for row in handshake['dom']['progressStages'])))
        add('candidate-fact-drives-choice-stage', True, candidate_ready_evidence(find('stage-candidates_ready')))
    elif name == 'stale-events':
        current = find('current-operation')
        current_text = (current['dom']['dialogs'] or [{}])[0].get('text') if current else None
        add('old-operation-cannot-change-visible-stage', True, type(current_text) is str and bool(current_text) and current_text == (last['dom']['dialogs'] or [{}])[0].get('text'))
        add('old-event-input-is-observed', True, any(row['kind'] == 'association_event_rejected' and type(row.get('operation_id')) is str and bool(row['operation_id']) for row in effects(last)))
        if inputs['current_session_id']:
            add('old-operation-cannot-replace-session', inputs['current_session_id'], sid(last))
    elif name == 'close-operation':
        cancellation = [row for row in last['facts']['methodCalls'] if re.search('cancel.*association|disconnect', row['method'], re.I) or row['method'] == 'dispatch' and re.search('disconnect|stop_capture', row.get('args', [{}])[0].get('command', {}).get('kind', ''))]
        add('closing-observer-does-not-cancel-operation', 0, len(cancellation))
        add('close-operation-was-actually-associated', 1, len(calls(last, ['associate'])))
        if inputs['committed_operation_ids']:
            add('observer-can-close-while-operation-owned', 0, len(last['dom']['dialogs']))
            before_events = [event for message in before['facts']['rpcMessages'] for event in message['events'] if event.get('kind') == 'association_stage']
            later = [event for message in last['facts']['rpcMessages'][len(before['facts']['rpcMessages']):] for event in message['events']]
            def advanced(event):
                old = [row for row in before_events if row.get('operation_id') == event.get('operation_id')]
                return (event.get('kind') == 'association_stage' and event.get('operation_id') in inputs['committed_operation_ids']
                        and type(event.get('stage')) is str and bool(event['stage'])
                        and event['stage'] != (old[-1].get('stage') if old else None))
            add('real-committed-operation-advances-after-observer-closes', True, any(advanced(event) for event in later))
    elif name == 'empty':
        add('source-controls-are-multi-select', True, len(choices(last)) >= 2)
        add('no-default-selected-source', 0, sum(bool(row['checked']) for row in choices(last)))
        add('empty-begin-reading-disabled', True, any(row['disabled'] for row in buttons('^开始阅读$')))
        add('empty-selection-has-no-commit', 0, len(commits(last)))
    elif name == 'draft-later':
        add('source-controls-are-multi-select', True, len(choices(before)) >= 2)
        add('draft-and-later-do-not-commit', 0, diff(commits))
        add('draft-and-later-have-no-window-effects', 0, diff(lambda s: calls(s, ['openWindow', 'closeWindow'])))
        add('later-keeps-connected-session', sid(before), sid(last))
        add('later-action-executed', True, clicked('^稍后选择$'))
        add('later-closes-only-panel', 0, len(last['dom']['dialogs']))
    elif name in {'commit-set', 'commit-failure', 'owned-remember', 'unowned-remember', 'temporary-saved', 'requires-review', 'effects-order', 'effects-retry', 'stale-effects', 'submit-retry', 'source-expired'}:
        if name == 'unowned-remember':
            add('unowned-remember-explicitly-disabled', True, len(remember) == 1 and remember[0]['disabled'] and bool(re.search('游戏|归属', text)))
            add('unowned-does-not-silently-save', 0, len(commits(last)))
        elif name == 'requires-review':
            add('unstable-saved-choice-demands-review', True, bool(re.search('重新核对|需要核对', text)))
            add('requires-review-never-auto-selects', 0, sum(bool(row['checked']) for row in choices(last)))
            add('requires-review-never-auto-commits', 0, len(commits(last)))
        else:
            selected = find('draft-selected')
            add('two-source-draft-is-visible', True, bool(selected and sum(bool(row['checked']) for row in choices(selected)) == 2))
            add('no-submit-before-explicit-intent', 0, diff(commits, before, selected) if selected else 0)
            selected_ids = sorted(row['sourceId'] for row in choices(selected) if row['checked']) if selected else []
            add('explicit-begin-action-executed', True, clicked('^开始阅读$'))
            if mode == 'production_rpc':
                add('selected-source-identities-are-observable', True,
                    len(selected_ids) == 2 and all(type(value) is str and value for value in selected_ids) and len(set(selected_ids)) == 2)
            if name in {'commit-failure', 'submit-retry', 'source-expired'}:
                failed = find('submit-failed') or last
                add('failed-submit-retains-draft', True, sum(bool(row['checked']) for row in choices(failed)) == 2)
                if mode == 'production_rpc':
                    add('failed-submit-keeps-same-source-identities', selected_ids,
                        sorted(row['sourceId'] for row in choices(failed) if row['checked']))
                add('failed-submit-has-error', True, bool(re.search('失败|未完成|失效|重试', failed['dom']['text'])))
                add('failed-submit-has-no-window-effects', 0, diff(lambda s: calls(s, ['openWindow', 'closeWindow']), before, failed))
            if name not in {'commit-failure', 'source-expired'} and mode == 'production_rpc':
                committed = [row for row in effects(last) if row['kind'] == 'selection_commit']
                add('one-effective-selection-commit', 1, len(committed))
                add('selection-commit-matches-checked-members', selected_ids,
                    sorted(committed[0]['source_ids']) if committed and isinstance(committed[0].get('source_ids'), list) else [])
            if name == 'owned-remember':
                add('owned-remember-has-explicit-choice', True, bool(selected and any('记住' in row['label'] and row['checked'] and not row['disabled'] for row in selected['dom']['checkboxes'])))
                if mode == 'production_rpc':
                    add('remember-intent-was-committed', True, any(row['kind'] == 'selection_commit' and row.get('remember') is True for row in effects(last)))
            if name == 'temporary-saved':
                add('temporary-intent-was-committed', True, any(row['kind'] == 'selection_commit' and row.get('remember') is False for row in effects(last)))
                add('temporary-preserves-saved-descriptor', True, any(row['kind'] == 'saved_selection_comparison' and isinstance(row.get('before_hash'), str) and row.get('before_hash') == row.get('after_hash') for row in effects(last)))
            if name == 'effects-order':
                kinds = [row['kind'] for row in effects(last)]
                add('persist-before-show-before-hide', True, all(kind in kinds for kind in ('selection_commit', 'hud_shown', 'main_hidden')) and kinds.index('selection_commit') < kinds.index('hud_shown') < kinds.index('main_hidden'))
            if name == 'effects-retry':
                failed = find('effect-failed')
                add('effect-failure-is-distinct', True, bool(failed and re.search('选择已保存|已保存选择', failed['dom']['text']) and re.search('窗口|浮窗', failed['dom']['text'])))
                add('effect-retry-does-not-resubmit', 0, diff(commits, failed) if failed else len(commits(last)))
                add('effect-retry-is-observed', True, all(any(row['kind'] == kind for row in effects(last)) for kind in ('hud_shown', 'main_hidden')))
            if name == 'stale-effects':
                if inputs['replacement_session_id']:
                    add('old-effects-cannot-change-new-session', inputs['replacement_session_id'], sid(last))
                add('stale-effect-attempt-was-rejected', True, any(row['kind'] == 'stale_window_effect_rejected' for row in effects(last)))
    elif name == 'details':
        sources = (last['facts'].get('session') or {}).get('sources')
        add('full-sample-retained', True, isinstance(sources, list) and all(not source.get('sample') or source['sample'] in text for source in sources))
        details = last['source_details']
        for row in details['rows']:
            add('source-row-' + row['source_id'], 1, row['rowCount'])
            for field in row['fields']:
                if field['inputAvailable']:
                    add('source-field-' + row['source_id'] + '-' + field['label'], field['expected'],
                        field['actual'] if field['matchCount'] == 1 else None)
        blocked += details['blocked_obligations']
    elif name.startswith('layout-') or name == 'other-bottle-warning':
        add('root-does-not-scroll', True, all(step['dom']['geometry']['bodyScrollTop'] == 0 and step['dom']['geometry']['rootScrollTop'] == 0 for step in steps))
        add('no-root-horizontal-overflow', True, all(step['dom']['geometry']['rootScrollWidth'] <= step['dom']['geometry']['rootClientWidth'] + 1 for step in steps))
        add('identity-preview-confirmation-reachable', True, all(find(action) is not None for action in ('identity-reachable', 'preview-reachable', 'confirmation-reachable')))
        geometry = last['dom']['geometry']
        add('confirmation-is-in-viewport', True, any(re.search(r'^(关联并接入|切换并接入|复用连接)', row['label']) and row['rect']['x'] >= 0 and row['rect']['y'] >= 0 and row['rect']['right'] <= geometry['width'] + 1 and row['rect']['bottom'] <= geometry['height'] + 1 for row in last['dom']['buttons']))
        if name == 'other-bottle-warning':
            add('other-bottle-warning-is-secondary-disclosure', True, any(re.search('容器|Bottle|其他', row['text']) and not row['open'] for row in last['dom']['details']))
    elif name in {'frame-state', 'revoke', 'late-metadata'}:
        live = find('valid-frame')
        preview = lambda step: (step['dom']['previews'] or [{}])[0] if step else {}
        add('real-frame-decodes', True, any(image['complete'] and image['naturalWidth'] > 0 for image in preview(live).get('images', [])))
        if name == 'revoke':
            add('revocation-clears-image', 0, len(preview(last).get('images', [])))
            add('revocation-explained', True, bool(re.search('撤销|录屏|屏幕录制', preview(last).get('text', ''))))
            if inputs['protected_session_id']:
                add('revocation-does-not-disconnect-hook', inputs['protected_session_id'], sid(last))
            disconnects = [row for row in last['facts']['methodCalls'] if re.search('disconnect|stopCapture', row['method']) or row['method'] == 'dispatch' and re.search('disconnect|stop_capture', row.get('args', [{}])[0].get('command', {}).get('kind', ''))]
            add('revocation-sends-no-disconnect', 0, len(disconnects))
        elif name == 'late-metadata':
            add('metadata-or-old-frame-cannot-advance-time', preview(live).get('dateTime'), preview(last).get('dateTime'))
            add('metadata-does-not-claim-recovery', True, '实时预览' not in preview(last).get('text', ''))
        else:
            for action, key in [('pause', 'pause-control-was-invoked'), ('resume', 'resume-control-was-invoked')]:
                add(key, 1, sum(row['method'] == 'controlWindowPreview' and row.get('args', [{}])[0].get('action') == action for row in last['facts']['methodCalls']))
            add('pause-state-was-visible', 'paused', preview(find('paused')).get('state'))
            add('valid-frame-had-a-timestamp', True, type(preview(live).get('dateTime')) is str and bool(preview(live).get('dateTime')))
            add('pause-keeps-frame-time', preview(live).get('dateTime'), preview(find('paused')).get('dateTime'))
            add('resume-without-frame-keeps-time', preview(live).get('dateTime'), preview(find('resuming')).get('dateTime'))
            newer = preview(find('new-frame')).get('dateTime')
            add('new-frame-advances-time', True, bool(newer and newer != preview(live).get('dateTime')))
    elif name in {'association-retry', 'source-query-retry'}:
        failed = find('operation-failed')
        add('failure-is-visible', True, bool(failed and re.search('失败|未完成|重试', failed['dom']['text'])))
        add('retry-remains-explicit', True, bool(failed and any(re.search('重试', row['label']) and not row['disabled'] for row in failed['dom']['buttons'])))
        add('target-retained-on-failure', True, bool(failed and re.search('目标完整路径|窗口已关联|选择正文', failed['dom']['text'])))
        if name == 'association-retry':
            add('retry-reviews-current-target', True, len(calls(last, ['reviewAssociation'])) >= 2)
        else:
            add('source-query-retry-does-not-reinject', 0, diff(lambda s: calls(s, ['associate']), failed) if failed else 1)
    if not checks:
        raise ValueError('Registered scene has no independent assertions')
    failed = [row['id'] for row in checks if not row['passed']]
    return {'id': identity, 'status': 'failed' if failed else 'blocked' if blocked else 'passed',
            'assertions': checks, 'blocked_obligations': blocked, 'failed_assertions': failed}


def execute_transport_producer(root, output, nonce, source):
    """Read-only typed DTO fixture producer, never an app/game action runner."""
    spec = TRANSPORT_PRODUCER
    if spec['source'] not in source:
        return None
    directory = path_at(root, output)
    directory.mkdir(parents=True, exist_ok=False)
    def run(label, command, env=None):
        started = datetime.now(timezone.utc).isoformat()
        completed = subprocess.run(command, cwd=root, env=env, text=True, capture_output=True, timeout=600)
        (directory / (label + '.stdout.log')).write_text(completed.stdout)
        (directory / (label + '.stderr.log')).write_text(completed.stderr)
        execution = {'command': command, 'cwd': '.', 'started_at': started,
                     'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': completed.returncode,
                     'stdout': ref(root, output + '/' + label + '.stdout.log'),
                     'stderr': ref(root, output + '/' + label + '.stderr.log')}
        if env is not None:
            execution['fixture_environment'] = {key: env[key] for key in
                                               ('LUNA_BINDING_NONCE', 'LUNA_BINDING_OUTPUT', 'LUNA_BINDING_SOURCE_TREE')}
        _write(directory / (label + '.execution.json'), execution)
        if completed.returncode:
            raise ValueError('Actual registered production fixture command failed: ' + label)
        return execution, completed.stdout
    compile_command = ['cargo', 'test', '--locked', '--offline', '-p', spec['package'],
                       '--test', spec['target'], '--no-run', '--message-format=json']
    _, raw = run('compile', compile_command)
    artifacts = []
    for line in raw.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if (row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == spec['target']
                and row.get('profile', {}).get('test') is True and row.get('executable')):
            artifacts.append(row)
    if len(artifacts) != 1:
        raise ValueError('Exact production transport test artifact missing')
    original = Path(artifacts[0]['executable'])
    binary = directory / 'production-transport-test-binary'
    shutil.copy2(original, binary)
    _, inventory = run('inventory', [str(binary), '--list', '--ignored', '--format', 'terse'])
    if spec['test_name'] + ': test' not in inventory.splitlines():
        # The Native target may exist before its DTO writer; absence is a
        # capability gap, never a fallback to manufactured response objects.
        return None
    manifest = {'kind': 'binding-journey-runtime-transport/v1', 'nonce': nonce,
                'mode': 'ndjson', 'protocol': 'LUNA_BINDING_RPC_V1',
                'producer': dict(spec, source_sha256=source[spec['source']]),
                'compilation': ref(root, output + '/compile.execution.json'),
                'inventory': ref(root, output + '/inventory.execution.json'),
                'binary': ref(root, binary.relative_to(root).as_posix()),
                'source_tree_sha256': tree_sha(source)}
    _write(directory / 'manifest.json', manifest)
    return ref(root, output + '/manifest.json')


def rust_cases(layer):
    return {case: case.lower().replace('-', '_') + '_' + layer
            for case, layers in REQUIRED_LAYERS.items() if layer in layers}


def execute_rust_layers(root, output, nonce, source):
    """Compile and run exact production Cargo targets; absent targets are blocked."""
    directory = path_at(root, output)
    directory.mkdir(parents=True, exist_ok=False)
    records = []
    def run(name, command, environment=None):
        start = datetime.now(timezone.utc).isoformat()
        pid = None
        timed_out = False
        try:
            process = subprocess.Popen(command, cwd=root, env=environment, text=True,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            pid = process.pid
            try:
                stdout, stderr = process.communicate(timeout=600)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                stdout, stderr = process.communicate()
            code = process.returncode
        except OSError as error:
            stdout, stderr, code = '', str(error), -1
        (directory / (name + '.stdout.log')).write_text(stdout)
        (directory / (name + '.stderr.log')).write_text(stderr)
        record = {'command': command, 'cwd': '.', 'pid': pid, 'timed_out': timed_out, 'started_at': start,
                  'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': code,
                  'stdout': ref(root, output + '/' + name + '.stdout.log'),
                  'stderr': ref(root, output + '/' + name + '.stderr.log')}
        if environment is not None:
            record['fixture_environment'] = {key: environment[key] for key in
                                             ('LUNA_BINDING_NONCE', 'LUNA_BINDING_OUTPUT', 'LUNA_BINDING_SOURCE_TREE')}
        _write(directory / (name + '.execution.json'), record)
        return record, stdout
    for layer, spec in RUST_LAYERS.items():
        row = {'layer': layer, 'source': spec['source'], 'cases': rust_cases(layer),
               'status': 'blocked', 'reason': 'registered production Rust test target not implemented',
               'executions': [], 'binary': None}
        records.append(row)
        if spec['source'] not in source:
            continue
        command = ['cargo', 'test', '--locked', '--offline', '-p', spec['package'],
                   '--test', spec['target'], '--no-run', '--message-format=json']
        compiled, raw = run(layer + '-compile', command)
        row['executions'].append(ref(root, output + '/' + layer + '-compile.execution.json'))
        if compiled['exit_code'] != 0:
            row.update(status='failed', reason='production Cargo target compilation failed')
            continue
        artifacts = []
        for line in raw.splitlines():
            try:
                message = json.loads(line)
            except ValueError:
                continue
            if (message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == spec['target']
                    and message.get('profile', {}).get('test') is True and message.get('executable')):
                artifacts.append(message)
        if len(artifacts) != 1:
            row.update(status='failed', reason='Cargo did not emit one exact production test executable')
            continue
        artifact = artifacts[0]
        original = Path(artifact['executable'])
        if not original.is_file() or original.is_symlink():
            row.update(status='failed', reason='actual compiled test binary missing')
            continue
        copied = directory / (layer + '-test-binary')
        shutil.copy2(original, copied)
        row['binary'] = ref(root, copied.relative_to(root).as_posix())
        row['cargo_artifact'] = artifact
        listed, _ = run(layer + '-list', [str(copied), '--list', '--format', 'terse'])
        row['executions'].append(ref(root, output + '/' + layer + '-list.execution.json'))
        runtime_output = directory / (layer + '-runtime')
        runtime_output.mkdir(exist_ok=False)
        environment = dict(os.environ, LUNA_BINDING_NONCE=nonce,
                           LUNA_BINDING_SOURCE_TREE=tree_sha(source), LUNA_BINDING_OUTPUT=str(runtime_output))
        for case, name in row['cases'].items():
            run(layer + '-' + case, [str(copied), '--exact', name, '--nocapture'], environment)
            row['executions'].append(ref(root, output + '/' + layer + '-' + case + '.execution.json'))
        if layer == 'storage':
            runtime = runtime_output / 'storage-v5-runtime.json'
            row['storage_runtime'] = ref(root, runtime.relative_to(root).as_posix()) if runtime.is_file() else None
        row.update(status='executed_pending_independent_inspection', reason=None)
    report = {'schema_version': 1, 'kind': 'binding-journey-production-rust-layers/v1',
              'nonce': nonce, 'source_tree_sha256': tree_sha(source), 'source_map': source,
              'layers': records, 'product_acceptance_promoted': False, 'native_executed': False}
    _write(directory / 'execution.json', report)
    return ref(root, output + '/execution.json')


def execute_fixed_native_units(root, output, nonce, source):
    """Actual locked/listed exact Native unit tests; absent names stay blocked."""
    directory = path_at(root, output)
    directory.mkdir(parents=True, exist_ok=False)
    report = {'schema_version': 1, 'kind': 'binding-fixed-native-unit-executions/v1',
              'nonce': nonce, 'source_tree_sha256': tree_sha(source), 'source_map': source,
              'groups': {key: list(names) for key, names in FIXED_NATIVE_UNIT_GROUPS.items()},
              'compilation': None, 'inventory': None, 'binary': None, 'tests': [],
              'native_os_executed': False, 'product_acceptance_promoted': False}
    def run(name, command):
        start = datetime.now(timezone.utc).isoformat()
        pid, timed_out = None, False
        try:
            environment = dict(os.environ, LUNA_BINDING_NONCE=nonce, LUNA_BINDING_SOURCE_TREE=tree_sha(source),
                               LUNA_BINDING_OUTPUT=str(directory))
            process = subprocess.Popen(command, cwd=root, env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            pid = process.pid
            try:
                stdout, stderr = process.communicate(timeout=600)
            except subprocess.TimeoutExpired:
                timed_out = True; process.kill(); stdout, stderr = process.communicate()
            code = process.returncode
        except OSError as error:
            stdout, stderr, code = '', str(error), -1
        (directory / (name + '.stdout.log')).write_text(stdout)
        (directory / (name + '.stderr.log')).write_text(stderr)
        record = {'command': command, 'cwd': '.', 'pid': pid, 'timed_out': timed_out,
                  'fixture_environment': {'LUNA_BINDING_NONCE': nonce, 'LUNA_BINDING_SOURCE_TREE': tree_sha(source), 'LUNA_BINDING_OUTPUT': str(directory)},
                  'started_at': start, 'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': code,
                  'stdout': ref(root, output + '/' + name + '.stdout.log'), 'stderr': ref(root, output + '/' + name + '.stderr.log')}
        _write(directory / (name + '.execution.json'), record)
        return ref(root, output + '/' + name + '.execution.json'), record, stdout
    command = ['cargo', 'test', '--locked', '--offline', '-p', 'luna-reader', '--bin', 'luna-reader', '--no-run', '--message-format=json']
    report['compilation'], compiled, raw = run('compile', command)
    if compiled['exit_code'] == 0 and not compiled['timed_out']:
        artifacts = []
        for line in raw.splitlines():
            try: row = json.loads(line)
            except ValueError: continue
            if row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == 'luna-reader' and row.get('profile', {}).get('test') is True and row.get('executable'):
                artifacts.append(row)
        if len(artifacts) == 1:
            original = Path(artifacts[0]['executable'])
            if original.is_file() and not original.is_symlink():
                copied = directory / 'native-unit-binary'; shutil.copy2(original, copied)
                report['binary'] = ref(root, copied.relative_to(root).as_posix())
                report['cargo_artifact'] = artifacts[0]
                report['inventory'], listed, listing = run('list', [str(copied), '--list', '--format', 'terse'])
                names = {line[:-6] for line in listing.splitlines() if line.endswith(': test')}
                for number, name in enumerate(name for group in FIXED_NATIVE_UNIT_GROUPS.values() for name in group):
                    entry = {'test_name': name, 'execution': None, 'status': 'blocked'}
                    if listed['exit_code'] == 0 and name in names:
                        entry['execution'], _, _ = run('exact-' + str(number), [str(copied), '--exact', name, '--nocapture'])
                        entry['status'] = 'executed_pending_independent_inspection'
                    report['tests'].append(entry)
    _write(directory / 'execution.json', report)
    return ref(root, output + '/execution.json')


def affected_paths(paths):
    """Actual source impact, independent of a change's declared check names."""
    prefixes = ('apps/desktop/src/features/capture/', 'apps/desktop/src/bridge/',
                'apps/desktop/src/windows/main/', 'apps/desktop/src/ui/',
                'apps/desktop/src-tauri/src/ipc/',
                'apps/desktop/src-tauri/src/platform/window_preview',
                'apps/desktop/src-tauri/src/platform/window_collector/',
                'apps/desktop/src-tauri/src/platform/macos/',
                'apps/desktop/src-tauri/src/platform/window_manager',
                'apps/desktop/src-tauri/src/platform/window_geometry',
                'apps/desktop/src-tauri/src/platform/session_preview_coordinator',
                'crates/app-core/src/contracts/', 'crates/app-core/src/application/',
                'crates/app-core/src/ports/', 'crates/app-infra/src/hook_backend',
                'crates/app-infra/src/providers/',
                'crates/app-infra/src/storage/')
    exact = {'apps/desktop/src/features/library/LibraryScreen.tsx',
             'apps/desktop/src-tauri/src/bootstrap.rs', 'apps/desktop/src-tauri/src/main.rs'}
    return sorted(p for p in paths if p in exact or p.startswith(prefixes))


def required_checks(change_id, paths, profile='development'):
    storage = ('crates/app-infra/src/storage/', 'docs/architecture/database/',
               'crates/app-infra/tests/storage_', 'crates/app-infra/tests/source_selection_sets.rs')
    storage_required = change_id == CHANGE or profile == 'release' or any(path.startswith(storage) for path in paths)
    if change_id == CHANGE or affected_paths(paths) or profile == 'release' or storage_required:
        result = {CHECK_ID, 'binding-journey-guard-tests'}
        if change_id == CHANGE:
            result.update({PHASE_CHECK, 'binding-phase-guard-tests'})
        if storage_required:
            result.update(STORAGE_V5_CHECKS)
        return frozenset(result)
    return frozenset()


def validate_registry(root=ROOT):
    errors = []
    try:
        plan = load(path_at(root, PLAN))
        rows = plan['cases']
        if (type(plan.get('schema_version')) is not int or plan['schema_version'] != 1
                or plan.get('kind') != 'approved-binding-journey-implementation-plan/v1'
                or plan.get('change_id') != CHANGE
                or not isinstance(rows, list) or len(rows) != len(REQUIRED_LAYERS)):
            raise ValueError('Exact eighteen binding journey obligations required')
        found = {}
        for row in rows:
            if (not isinstance(row, dict) or row.get('id') in found
                    or row.get('id') not in REQUIRED_LAYERS
                    or tuple(row.get('required_layers', ())) != REQUIRED_LAYERS[row['id']]
                    or row.get('planned_status') != 'not_run'
                    or not isinstance(row.get('steps'), list) or not row['steps']
                    or not isinstance(row.get('expected'), list) or not row['expected']):
                raise ValueError('Missing/duplicate/changed binding obligation or planned status')
            found[row['id']] = row
        if set(found) != set(REQUIRED_LAYERS):
            raise ValueError('Binding obligation set differs')
        preserved = plan['preserved']
        for key in ('original_153_release_executions', 'existing_70_sources_576_observations',
                    'RBC74_RBC77', 'five_negative_scenarios', 'old_source_and_runtime_failure_records'):
            if preserved.get(key) is not True:
                raise ValueError('An existing binding obligation was removed: ' + key)
        if plan.get('next_stage_allowed') is not False or plan.get('current_product_alignment') != 'incomplete':
            raise ValueError('A plan cannot declare current product acceptance')
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        errors.append(str(error))
    return errors


def observation_verdict(identity, value):
    """Code-owned semantic oracle, rebuilt from raw browser observations.

    Controlled input is legal for branch testing, but neither this oracle nor a
    component success establishes a real Hook, SQLite migration or system share.
    """
    if identity not in COMPONENT_CASES or not isinstance(value, dict):
        raise ValueError('Known component case and typed observations required')
    checks = []
    def expect(name, actual, expected, passed):
        checks.append({'id': name, 'actual': actual, 'expected': expected,
                       'passed': type(passed) is bool and passed})
    def text(key):
        actual = value.get(key)
        if not isinstance(actual, str):
            raise ValueError('Typed observation text required: ' + key)
        return actual
    def integer(key):
        actual = value.get(key)
        if type(actual) is not int or actual < 0:
            raise ValueError('Nonnegative integer observation required: ' + key)
        return actual
    if identity == COMPONENT_CASES[0]:
        label = value.get('selectionButton', {}).get('label')
        visible = text('visibleText')
        expect('first-system-confirmation-label', label, '选择对应的游戏窗口…', label == '选择对应的游戏窗口…')
        button = value.get('selectionButton', {})
        if any(type(button.get(key)) is not bool for key in ('visible', 'enabled')):
            raise ValueError('Typed first-confirmation control availability required')
        expect('first-confirmation-control-available', button['visible'] and button['enabled'], True,
               button['visible'] and button['enabled'])
        expect('first-confirmation-reason-visible', visible, '需要 + 游戏窗口',
               '需要' in visible and '游戏窗口' in visible)
        count = integer('imageCount')
        expect('no-invented-first-frame', count, 0, count == 0)
        time_text = text('timeText')
        expect('no-invented-first-frame-time', time_text, '尚无成功画面', '尚无成功画面' in time_text)
    elif identity == COMPONENT_CASES[1]:
        actual = value.get('sameTargetSwitchVisible')
        if type(actual) is not bool:
            raise ValueError('Typed same-target switch observation required')
        expect('same-target-is-not-switch', actual, False, actual is False)
        visible = text('visibleText')
        expect('reuse-explained', visible, '复用', '复用' in visible)
    elif identity == COMPONENT_CASES[2]:
        labels = value.get('stageLabels')
        if not isinstance(labels, list) or any(type(x) is not str for x in labels):
            raise ValueError('Stage label observations required')
        for label in ('窗口已关联', '自动接入', '选择正文'):
            expect('stage-' + label, labels, label, any(label in x for x in labels))
        if type(value.get('pending')) is not bool or type(value.get('duplicateSubmitAvailable')) is not bool:
            raise ValueError('Actual pending/duplicate-submit availability required')
        expect('actual-association-remains-pending', value['pending'], True, value['pending'])
        expect('pending-cannot-submit-twice', value['duplicateSubmitAvailable'], False,
               value['duplicateSubmitAvailable'] is False)
    elif identity == COMPONENT_CASES[3]:
        count = integer('sourceCheckboxCount')
        begin = value.get('beginReadingButton')
        visible = begin.get('visible') if isinstance(begin, dict) else begin
        if type(visible) is not bool or type(value.get('dialogStillOpen')) is not bool:
            raise ValueError('Typed draft/confirmation visibility observations required')
        commands = integer('draftClickCommandCount')
        expect('multiple-source-draft-controls', count, '>=2', count >= 2)
        expect('explicit-begin-reading', visible, True, visible)
        expect('draft-does-not-commit', commands, 0, commands == 0)
        for field in ('draftWindowEffectCount', 'draftAssociationCount'):
            if field in value:
                actual = integer(field)
                expect(field, actual, 0, actual == 0)
        expect('draft-keeps-dialog-open', value['dialogStillOpen'], True, value['dialogStillOpen'])
    elif identity == COMPONENT_CASES[4]:
        labels = value.get('sourceFieldLabels')
        if not isinstance(labels, list) or any(type(x) is not str for x in labels):
            raise ValueError('Typed source field labels required')
        text('expandedText')
        available = value.get('productionDetailFixtureAvailable')
        if type(available) is not bool:
            raise ValueError('Explicit typed source-detail capability required')
        if available:
            rows = value.get('sourceDetails')
            if not isinstance(rows, list) or not rows:
                raise ValueError('Per-source actual detail observations required')
            for row in rows:
                for field in row['fields']:
                    actual, expected = field.get('actual'), field.get('expected')
                    expect('source-' + row['source_id'] + '-' + field['label'], actual, expected,
                           type(actual) is str and type(expected) is str and actual == expected)
        actual = value.get('longSampleRetained')
        if type(actual) is not bool:
            raise ValueError('Typed sample retention observation required')
        expect('complete-sample', actual, True, actual)
    elif identity == COMPONENT_CASES[5]:
        checkbox = value.get('rememberCheckbox')
        if not isinstance(checkbox, dict) or any(type(checkbox.get(k)) is not bool for k in ('visible', 'enabled')):
            raise ValueError('Typed remember control observation required')
        game = value.get('gameId')
        expect('remember-control-visible', checkbox['visible'], True, checkbox['visible'])
        expect('owned-game-can-choose-save', checkbox['enabled'], True,
               isinstance(game, str) and bool(game) and checkbox['enabled'])
    else:
        before = integer('pickerCallsBeforeIntent')
        after = integer('pickerCallsAfterIntent')
        associations = integer('associationCallsBeforeIntent')
        expect('no-implicit-system-picker', before, 0, before == 0)
        expect('no-implicit-association', associations, 0, associations == 0)
        expect('explicit-intent-opens-one-picker', after, 1, after == 1)
        if 'explicitButtonCount' in value:
            count = integer('explicitButtonCount')
            if any(type(value.get(key)) is not bool for key in ('explicitActionExecuted', 'explicitButtonVisible', 'explicitButtonEnabled')):
                raise ValueError('Typed explicit picker control/action observations required')
            expect('one-explicit-picker-control', count, 1, count == 1)
            available = value['explicitButtonVisible'] and value['explicitButtonEnabled']
            expect('explicit-picker-control-available', available, True, available)
            expect('explicit-picker-action-executed', value['explicitActionExecuted'], True, value['explicitActionExecuted'])
    blocked = (['advanced-source-typed-DTO-disclosure', 'source-u64-typed-DTO-projection']
               if identity == COMPONENT_CASES[4] and not value['productionDetailFixtureAvailable'] else [])
    return {'id': identity, 'status': 'failed' if not all(x['passed'] for x in checks) else 'blocked' if blocked else 'passed',
            'blocked_obligations': blocked,
            'assertions': checks, 'failed_assertions': [x['id'] for x in checks if not x['passed']]}


def _write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n')


def claim_resumption_nonce(root, nonce, output, source):
    policy_file = path_at(root, POLICY)
    if not policy_file.is_file(): return None
    policy = load(policy_file)
    if policy.get('resume_epoch') is None: return None
    from binding_phase_guard import phase_receipts
    phase_receipts(policy)
    old_packet = load(checked_ref(root, policy['resume_epoch']['ancestors']['gates_r2']['packet'], ignored=True))
    old_execution = load(checked_ref(root, old_packet['red_execution'], ignored=True))
    if old_execution.get('nonce') == nonce:
        raise ValueError('A predecessor execution nonce cannot authorize the resumed epoch')
    relative = '.build/binding-journey-alignment-20261109/phase-2-r3/nonce-ledger/' + nonce + '.json'
    path = path_at(root, relative)
    path.parent.mkdir(parents=True, exist_ok=True)
    claim = {'kind': 'binding-resumption-nonce-claim/v1', 'schema_version': 1, 'nonce': nonce,
             'output': output, 'source_tree_sha256': tree_sha(source), 'pid': os.getpid(), 'claimed_at': datetime.now(timezone.utc).isoformat()}
    with path.open('x') as stream:
        stream.write(json.dumps(claim, sort_keys=True, indent=2) + '\n')
    return ref(root, relative)


def execute(root=ROOT, output=None, *, nonce=None, env=None, native_evidence=None, service_evidence=None):
    root = Path(root).resolve()
    registry_errors = validate_registry(root)
    if registry_errors:
        raise ValueError('Binding plan/registry differs: ' + '; '.join(registry_errors))
    if not isinstance(output, str) or not output.startswith('.build/'):
        raise ValueError('Fresh ignored execution directory required')
    directory = path_at(root, output)
    directory.mkdir(parents=True, exist_ok=False)
    nonce = nonce or secrets.token_hex(16)
    if not isinstance(nonce, str) or not NONCE.fullmatch(nonce):
        raise ValueError('Exact 32-hex execution nonce required')
    before = source_files(root)
    nonce_claim = claim_resumption_nonce(root, nonce, output, before)
    _write(directory / 'source-before.json', before)
    context = {'schema_version': 1, 'kind': 'binding-journey-invocation/v1',
               'nonce': nonce, 'pid': os.getpid(), 'parent_pid': os.getppid(), 'started_at': datetime.now(timezone.utc).isoformat(),
               'finished_at': None, 'source_before': ref(root, output + '/source-before.json'),
               'source_tree_sha256': tree_sha(before), 'executions': [],
               'infrastructure_errors': [], 'component_report': None,
               'product_completion': 'incomplete', 'next_stage_allowed': False}
    context['native_evidence'] = ref(root, native_evidence) if native_evidence else None
    context['service_evidence'] = ref(root, service_evidence) if service_evidence else None
    def run(role, command):
        started = datetime.now(timezone.utc).isoformat()
        try:
            result = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True, timeout=600)
            stdout, stderr, code = result.stdout, result.stderr, result.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            stdout, stderr, code = '', str(error), -1
        (directory / (role + '.stdout.log')).write_text(stdout)
        (directory / (role + '.stderr.log')).write_text(stderr)
        row = {'role': role, 'command': command, 'cwd': '.', 'started_at': started,
               'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': code,
               'stdout': ref(root, output + '/' + role + '.stdout.log'),
               'stderr': ref(root, output + '/' + role + '.stderr.log')}
        context['executions'].append(row)
        return code
    context['nonce_claim'] = nonce_claim
    context['rust_layers'] = execute_rust_layers(root, output + '/rust-layers', nonce, before)
    context['fixed_native_units'] = execute_fixed_native_units(root, output + '/fixed-native-units', nonce, before)
    context['runtime_transport_producer'] = execute_transport_producer(root, output + '/runtime-transport', nonce, before)
    exporter_command = [sys.executable, EXPORTER, '--output', output + '/projections', '--nonce', nonce]
    if context['runtime_transport_producer']:
        exporter_command += ['--runtime-transport', context['runtime_transport_producer']['path']]
    code = run('production-projections', exporter_command)
    if code != 0:
        context['infrastructure_errors'].append('Production Rust serializer exporter did not execute successfully')
    else:
        manifest = path_at(root, output + '/projections/manifest.json')
        if not manifest.is_file():
            context['infrastructure_errors'].append('Production projection manifest missing')
        else:
            node = shutil.which('node')
            if not node:
                context['infrastructure_errors'].append('Node executable not available')
            else:
                runner_command = [node, RUNNER, '--output', output + '/component', '--nonce', nonce,
                                  '--projection-fixtures', output + '/projections/manifest.json']
                run('real-components', runner_command)
                report_file = path_at(root, output + '/component/verification.json')
                if report_file.is_file():
                    context['component_report'] = ref(root, output + '/component/verification.json')
                else:
                    context['infrastructure_errors'].append('Real component report missing')
    after = source_files(root)
    _write(directory / 'source-after.json', after)
    context['source_after'] = ref(root, output + '/source-after.json')
    context['finished_at'] = datetime.now(timezone.utc).isoformat()
    from binding_phase_guard import run_guard_tests
    run_guard_tests(root, output + '/guards')
    context['guard_execution'] = ref(root, output + '/guards/execution.json')
    context['finished_at'] = datetime.now(timezone.utc).isoformat()
    if after != before:
        context['infrastructure_errors'].append('Source epoch changed during binding execution')
    _write(directory / 'execution.json', context)
    from binding_journey_inspect import inspect_execution
    result = inspect_execution(root, directory, require_current=True)
    _write(directory / 'inspection.json', result)
    return result


def global_assessment(root=ROOT, steps=()):
    """Code-owned full layer gaps. Finite semantic checks cannot fill Native."""
    errors = validate_registry(root)
    finite = next((row.get('execution') for row in steps
                   if row.get('id') == CHECK_ID and isinstance(row.get('execution'), dict)), None)
    gaps, covered = [], {}
    try:
        obligation_registry = load(path_at(root, POLICY))['resume_epoch']['execution_obligations']
        registry_hash = tree_sha(obligation_registry)
    except (OSError, ValueError, KeyError, TypeError):
        registry_hash = None
    obligations = {'schema_version': 1, 'kind': 'binding-execution-obligation-assessment/v1',
                   'registry_sha256': registry_hash, 'product_acceptance_promoted': False,
                   'groups': [{'id': identity, 'status': 'not_run', 'checks': [], 'errors': [], 'artifacts': []}
                              for identity in EXACT_EXECUTION_OBLIGATION_IDS]}
    if finite and finite.get('execution'):
        try:
            from binding_journey_inspect import inspect_execution
            execution_path = checked_ref(root, finite['execution'], ignored=True)
            inspected = inspect_execution(root, execution_path.parent, require_current=True)
            if not inspected['validation_errors'] and not inspected['infrastructure_errors']:
                actual_obligations = inspected.get('execution_obligations')
                if (not isinstance(actual_obligations, dict)
                        or actual_obligations.get('registry_sha256') != registry_hash
                        or [row.get('id') for row in actual_obligations.get('groups', [])] != list(EXACT_EXECUTION_OBLIGATION_IDS)):
                    raise ValueError('Actual binding execution omitted a reviewed machine obligation group')
                obligations = actual_obligations
                for item in inspected.get('layer_results', []):
                    if item['status'] == 'passed':
                        covered[(item['case_id'], item['layer'])] = item
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append('Binding layer evidence rejected: ' + str(error))
    # Reuse the existing package/OS/database/Hook/frame inspector where its exact
    # assertions cover a journey. It does not prove the new picker/draft stages.
    runtime = next((x.get('execution') for x in steps if x.get('id') == 'product-packaged-stability-tests'), None)
    if isinstance(runtime, dict) and runtime.get('runtime_receipt'):
        try:
            from runtime_stability_evidence import validate_run
            path = path_at(root, runtime['runtime_receipt'])
            inspected = validate_run(root, path.parent, runtime['execution_nonce'], tree_sha(source_files(Path(root))))
            if inspected['status'] == 'passed' and not inspected['errors']:
                for layer in ('game', 'native'):
                    covered[('BJ-16', layer)] = {'case_id': 'BJ-16', 'layer': layer,
                                                'status': 'passed', 'adapter': 'existing-SR6-inspector'}
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append('Packaged binding cohort rejected: ' + str(error))
    for case_id, layers in REQUIRED_LAYERS.items():
        for layer in layers:
            if (case_id, layer) in covered:
                continue
            gaps.append({'case_id': case_id, 'layer': layer, 'status': 'not_run',
                         'reason': ('registered production Cargo execution missing or not passed' if layer in RUST_LAYERS
                                    else 'complete binding journey layer adapter/evidence not implemented'),
                         'finite_component_diagnostic_available': layer == 'component'
                         and any(case_id in x for x in CASE_JOURNEYS.values())})
    for group in obligations['groups']:
        if group['status'] != 'passed':
            gaps.append({'case_id': group['id'], 'layer': 'registered_execution_obligation',
                         'status': group['status'], 'reason': 'Actual fixed producer/independent execution evidence is missing or not passed'})
        if group['status'] == 'failed':
            errors.append('Actual machine execution obligation failed: ' + group['id'])
    return {'schema_version': 1, 'kind': 'binding-journey-global-assessment/v1',
            'status': 'failed' if errors else 'blocked' if gaps else 'passed', 'errors': errors,
            'release_gaps': gaps, 'finite_component_execution': finite,
            'execution_obligations': obligations,
            'covered_layers': list(covered.values()),
            'component_case_capability': list(COMPONENT_CASES),
            'native_acceptance_promoted': False, 'product_acceptance_promoted': False}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--execute', action='store_true')
    action.add_argument('--inspect', action='store_true')
    action.add_argument('--assess', action='store_true')
    action.add_argument('--check-registry', action='store_true')
    parser.add_argument('--output')
    parser.add_argument('--nonce')
    parser.add_argument('--native-evidence', help='original fixed-producer Native collection; never a passed flag')
    parser.add_argument('--service-evidence', help='actual same-source package/game/service chain')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        if args.execute:
            if not args.output:
                parser.error('--execute requires --output')
            result = execute(ROOT, args.output, nonce=args.nonce, native_evidence=args.native_evidence,
                             service_evidence=args.service_evidence)
        elif args.inspect:
            from binding_journey_inspect import inspect_execution
            result = inspect_execution(ROOT, path_at(ROOT, args.output), require_current=True)
        elif args.assess:
            result = global_assessment(ROOT)
        else:
            errors = validate_registry(ROOT)
            result = {'status': 'failed' if errors else 'passed', 'errors': errors}
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
        return 0 if result.get('status') == 'passed' else 1
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(json.dumps({'status': 'failed', 'errors': [str(error)],
                          'infrastructure_errors': [str(error)], 'eligible_semantic_red': False,
                          'product_acceptance_promoted': False}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    sys.exit(main())
