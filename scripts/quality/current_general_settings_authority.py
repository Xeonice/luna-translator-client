#!/usr/bin/env python3
"""Exact source-only authority for the user-approved general settings structure.

The small permanent input closure includes the literal captured source and its
recorded relocation. Historical .build paths identify what the user reviewed;
reading those old build directories is not a clean-checkout prerequisite.
This authority does not execute a component, approve a PNG or accept a release.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
from pathlib import Path

from gate_core import safe_path

ROOT = Path(__file__).resolve().parents[2]
SUPPLEMENT_ID = 'general-settings-structure-20261107'
TARGET_ID = 'guided-service-reading-20261030'
COMPOSITION_ID = 'complete-guided-history-20261031'
PREFIX = 'docs/design/first-product-v1/candidates/' + SUPPLEMENT_ID + '/'
ENTRY = PREFIX + 'settings.html'
RECORD = 'docs/development/design-approval-general-settings-20261107.json'
MANIFEST = PREFIX + 'manifest.json'
USER_CONFIRMATION = '判断设计稿没问题，进行修正，然后重新升级客户端'

PINNED_INPUTS = {'docs/design/first-product-v1/candidates/general-settings-structure-20261107/README.md': {'bytes': 3544,
                                                                                           'sha256': 'f13c23c8a818489a13b4fffe2e84fa722fecd376aa13c3218c29361f76bab549'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/README.md': {'bytes': 3544,
                                                                                                    'sha256': 'f13c23c8a818489a13b4fffe2e84fa722fecd376aa13c3218c29361f76bab549'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/capture-freeze.json': {'bytes': 3866,
                                                                                                              'sha256': 'a2f576edd5b114a3b8312d70b0634b93af05909c97461deee8478317d8404703'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/capture-manifest.json': {'bytes': 5041,
                                                                                                                'sha256': '8bf2b8a3eb51a055c4c9150bb280f8eb5bb6eb9c2795b430a0056bc08490bcc0'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/index.html': {'bytes': 4622,
                                                                                                     'sha256': '2d45b76a71cd0ab1e4b9384fdbd658b0851c64e014ed129262f79656ed028add'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/settings.css': {'bytes': 11112,
                                                                                                       'sha256': '976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/settings.html': {'bytes': 6970,
                                                                                                        'sha256': '6734155af5a9cd9251b21f665156c67a0c97bb473c162ddb9856e3efcd674b9a'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/settings.js': {'bytes': 9570,
                                                                                                      'sha256': '74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/index.html': {'bytes': 4622,
                                                                                            'sha256': '2d45b76a71cd0ab1e4b9384fdbd658b0851c64e014ed129262f79656ed028add'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/manifest.json': {'bytes': 5653,
                                                                                               'sha256': 'efd9e1b52c908a60ddfaa3f4e66c4efb6a8cba494d744db67ea010574005398a'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/reported-native-before.png': {'bytes': 412122,
                                                                                                            'sha256': '5302991f2187e6a92ab15eb3a66245a1add521e1b9024d6e704c2f5aeb3d3363'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.css': {'bytes': 11112,
                                                                                              'sha256': '976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.html': {'bytes': 6900,
                                                                                               'sha256': 'e02ecc6fcd4abca9951a79f4d6b4708107848a4e5d8a3430d8de3be1c7b6209c'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.js': {'bytes': 9570,
                                                                                             'sha256': '74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/history-settings.css': {'bytes': 10156,
                                                                                              'sha256': '166c73ba929f0c32d4c7c9ec26c2b23d545b0575ef40148e7bb986f56028381b'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/history.html': {'bytes': 12528,
                                                                                      'sha256': 'd54f2b90357d8f29ac96ed4550866b32de5188b3c11d748b8413aa34d5b37f96'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/index.html': {'bytes': 3124,
                                                                                    'sha256': '52ee8eda51ae93996bb71c595ed014128628c5da353fef9382277aa550d31896'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/manifest.json': {'bytes': 3963,
                                                                                       'sha256': '0af63c192e7777fe98bb23a472c59b18ec7718541a2cbbcffba8953c2148baf6'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/refinement.js': {'bytes': 9536,
                                                                                       'sha256': 'dc26e915ec1f86baced3da2a147817741c32db30f699ec1115842ee63503bd35'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/review.css': {'bytes': 2813,
                                                                                    'sha256': '1ac523e5f4b40305898cce4024559842e0078bae6dd02561ffe945bbf5b73635'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/review.js': {'bytes': 2953,
                                                                                   'sha256': '196ff29abea89b5563e2c9bbdd0306872cfd9b894155afd444e1ade966a7a8be'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/session.css': {'bytes': 4633,
                                                                                     'sha256': 'ecb240d4281647b3911325e15ee52781c6941d9f17352a295ff13630a8b09631'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/session.html': {'bytes': 12072,
                                                                                      'sha256': '6858dd83b2a7d82fbde451da79d8d02283dbc638d2e53f04c68ba530291e6b85'},
 'docs/design/first-product-v1/candidates/layout-refinements-20261107/settings.html': {'bytes': 12073,
                                                                                       'sha256': '709b17917a356fbf3728151e9c2669fb44c4ceacff691ffc7046ff250e1e9b57'},
 'docs/design/first-product-v1/icons.js': {'bytes': 3501,
                                           'sha256': '4dab92462e14e85bbd541ebdaf81c669d8e078fe8b5461b3cf5125ad219423da'},
 'docs/design/first-product-v1/material-tokens.css': {'bytes': 2770,
                                                      'sha256': '3406083383b5a58d2045fdcbbb26624b04b7f49fe3184599719826a207457a03'},
 'docs/development/design-approval-client-layout-20261105.json': {'bytes': 3200,
                                                                  'sha256': 'ce8071da9fa6b429703ad3a658cef98de17047e2fdad01ed6fbd79ed1ebe0f01'},
 'docs/development/design-approval-complete-20261031.json': {'bytes': 10791,
                                                             'sha256': '2aefa59a3edf6adc9fa1287378702b026f7a4029660bf2c23f4780abac9beacc'},
 'docs/development/design-approval-general-settings-20261107.json': {'bytes': 9455,
                                                                     'sha256': '4e5fc7f285de9d58d585fb0e0f46df39631ea38a1cfa261b4cb60114969b9989'},
 'docs/development/design-approval-layout-refinements-20261107.json': {'bytes': 4811,
                                                                       'sha256': '08b61a71a685f73e2bd2f7dd8f3917ba212bb7c437509ab797b74b446e68cb54'},
 'docs/development/design-approval-preview-20261104.json': {'bytes': 6257,
                                                            'sha256': 'badc91c0a62af6947fa28974933694da94900b5ba17e43188014e198bac2e462'}}

PROMOTED_SOURCES = {'docs/design/first-product-v1/candidates/general-settings-structure-20261107/README.md': {'bytes': 3544,
                                                                                           'sha256': 'f13c23c8a818489a13b4fffe2e84fa722fecd376aa13c3218c29361f76bab549'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/index.html': {'bytes': 4622,
                                                                                            'sha256': '2d45b76a71cd0ab1e4b9384fdbd658b0851c64e014ed129262f79656ed028add'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/reported-native-before.png': {'bytes': 412122,
                                                                                                            'sha256': '5302991f2187e6a92ab15eb3a66245a1add521e1b9024d6e704c2f5aeb3d3363'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.css': {'bytes': 11112,
                                                                                              'sha256': '976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.html': {'bytes': 6900,
                                                                                               'sha256': 'e02ecc6fcd4abca9951a79f4d6b4708107848a4e5d8a3430d8de3be1c7b6209c'},
 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.js': {'bytes': 9570,
                                                                                             'sha256': '74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f'}}

RELOCATION = [{'captured': {'bytes': 4622,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/index.html',
               'sha256': '2d45b76a71cd0ab1e4b9384fdbd658b0851c64e014ed129262f79656ed028add'},
  'promoted': {'bytes': 4622,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/index.html',
               'sha256': '2d45b76a71cd0ab1e4b9384fdbd658b0851c64e014ed129262f79656ed028add'},
  'transformation': 'verbatim'},
 {'captured': {'bytes': 6970,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/settings.html',
               'sha256': '6734155af5a9cd9251b21f665156c67a0c97bb473c162ddb9856e3efcd674b9a'},
  'promoted': {'bytes': 6900,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.html',
               'sha256': 'e02ecc6fcd4abca9951a79f4d6b4708107848a4e5d8a3430d8de3be1c7b6209c'},
  'transformation': 'two_dependency_relative_urls_only'},
 {'captured': {'bytes': 11112,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/settings.css',
               'sha256': '976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd'},
  'promoted': {'bytes': 11112,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.css',
               'sha256': '976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd'},
  'transformation': 'verbatim'},
 {'captured': {'bytes': 9570,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/settings.js',
               'sha256': '74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f'},
  'promoted': {'bytes': 9570,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.js',
               'sha256': '74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f'},
  'transformation': 'verbatim'},
 {'captured': {'bytes': 3544,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/README.md',
               'sha256': 'f13c23c8a818489a13b4fffe2e84fa722fecd376aa13c3218c29361f76bab549'},
  'promoted': {'bytes': 3544,
               'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/README.md',
               'sha256': 'f13c23c8a818489a13b4fffe2e84fa722fecd376aa13c3218c29361f76bab549'},
  'transformation': 'verbatim'}]

CAPTURED_RECORDS = [{'bytes': 5041,
  'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/capture-manifest.json',
  'sha256': '8bf2b8a3eb51a055c4c9150bb280f8eb5bb6eb9c2795b430a0056bc08490bcc0'},
 {'bytes': 3866,
  'path': 'docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/capture-freeze.json',
  'sha256': 'a2f576edd5b114a3b8312d70b0634b93af05909c97461deee8478317d8404703'}]

RUNTIME_DEPENDENCIES = [{'bytes': 2770,
  'path': 'docs/design/first-product-v1/material-tokens.css',
  'sha256': '3406083383b5a58d2045fdcbbb26624b04b7f49fe3184599719826a207457a03'},
 {'bytes': 3501,
  'path': 'docs/design/first-product-v1/icons.js',
  'sha256': '4dab92462e14e85bbd541ebdaf81c669d8e078fe8b5461b3cf5125ad219423da'}]

PARENT_APPROVAL = {'bytes': 4811,
 'path': 'docs/development/design-approval-layout-refinements-20261107.json',
 'sha256': '08b61a71a685f73e2bd2f7dd8f3917ba212bb7c437509ab797b74b446e68cb54'}

PARENT_RECORDS = {'docs/development/design-approval-client-layout-20261105.json': {'bytes': 3200,
                                                                  'sha256': 'ce8071da9fa6b429703ad3a658cef98de17047e2fdad01ed6fbd79ed1ebe0f01'},
 'docs/development/design-approval-complete-20261031.json': {'bytes': 10791,
                                                             'sha256': '2aefa59a3edf6adc9fa1287378702b026f7a4029660bf2c23f4780abac9beacc'},
 'docs/development/design-approval-preview-20261104.json': {'bytes': 6257,
                                                            'sha256': 'badc91c0a62af6947fa28974933694da94900b5ba17e43188014e198bac2e462'}}

REVIEWED_SOURCES = {'README.md': {'bytes': 3544,
               'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/README.md',
               'sha256': 'f13c23c8a818489a13b4fffe2e84fa722fecd376aa13c3218c29361f76bab549'},
 'index.html': {'bytes': 4622,
                'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/index.html',
                'sha256': '2d45b76a71cd0ab1e4b9384fdbd658b0851c64e014ed129262f79656ed028add'},
 'settings.css': {'bytes': 11112,
                  'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/settings.css',
                  'sha256': '976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd'},
 'settings.html': {'bytes': 6970,
                   'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/settings.html',
                   'sha256': '6734155af5a9cd9251b21f665156c67a0c97bb473c162ddb9856e3efcd674b9a'},
 'settings.js': {'bytes': 9570,
                 'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/settings.js',
                 'sha256': '74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f'}}

REVIEWED_MANIFEST = {'bytes': 5041,
 'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/manifest.json',
 'sha256': '8bf2b8a3eb51a055c4c9150bb280f8eb5bb6eb9c2795b430a0056bc08490bcc0'}

REVIEWED_FREEZE = {'bytes': 3866,
 'path': '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/freeze.json',
 'sha256': 'a2f576edd5b114a3b8312d70b0634b93af05909c97461deee8478317d8404703'}

APPROVED_SCOPE = ['General settings three groups: lexical, always-saved reading records and local OS preference',
 'Session immediate input/translation controls remain in current session; general only typed navigation link',
 'Per-field global/game inheritance, source visibility and restore/discard/save semantics retained',
 'Login startup remains true OS authority with independent pending/error/readback; collapsed detail is '
 'presentation, not system registration recovery']

VIEWPORTS = {'narrow': [760, 560], 'normal': [1000, 720]}

NOT_GRANTED = ['reference PNG approval',
 'real component or Native acceptance',
 'system login-startup recovery verified',
 'whole-product release acceptance']


LEGACY_TEST = {
    'path': 'apps/desktop/tests/live-preview/verify-settings-layout-refinement.mjs',
    'sha256': '202f60b66744c93051d47f3bd362d76d9e464d942e5ad181f1faaccf17de1d65',
    'bytes': 25683,
}
LEGACY_CHECK = 'settings-layout-refinement-tests'
REPLACEMENT_CHECK = 'general-settings-structure-tests'
REPLACEMENT_TEST = 'apps/desktop/tests/live-preview/verify-general-settings-structure.mjs'
AUTHORITY_CHECK = 'general-settings-source-authority'
AUTHORITY_SCRIPT = 'scripts/quality/current_general_settings_authority.py'
PRESERVED_TEST_OBLIGATIONS = [
    'Actual WindowRoot components at normal/narrow, dark/light and200 percent with approved source comparison, safe rows and reachable footer',
    'Exact admitted before-image and source/browser/runtime-preview epochs; retained original red artifacts',
    'POS disabled while segmentation is off retains its preference, ruby remains independent, save and discard retain real preference semantics',
    'Failed save, dirty scope lock, per-field game inheritance, restore-draft/save/discard, CAS and stale request isolation',
    'Always-saved history and real typed history/session navigation; session immediate controls remain in current session, not a duplicate general switch',
    'OS startup states, unavailable/error/last-known/pending and write-then-confirm remain independent from preference saving',
]


def _json_value(value):
    """Accept only actual JSON container/scalar types, never tuple coercion."""
    if value is None or type(value) in (str, bool, int):
        return
    if type(value) is float:
        import math
        if math.isfinite(value):
            return
        raise ValueError('finite JSON numbers required')
    if type(value) is list:
        for item in value:
            _json_value(item)
        return
    if type(value) is dict and all(type(key) is str for key in value):
        for item in value.values():
            _json_value(item)
        return
    raise ValueError('exact JSON types required')


def _same(actual, expected):
    return json.dumps(actual, sort_keys=True, ensure_ascii=False, allow_nan=False) == json.dumps(
        expected, sort_keys=True, ensure_ascii=False, allow_nan=False)


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key: ' + key)
        result[key] = value
    return result


def _load(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON value: ' + value)))


def _root(root):
    root = Path(root)
    if not root.is_absolute() or '..' in root.parts or not root.is_dir():
        raise ValueError('existing absolute nonsymlink root required')
    if any(path.is_symlink() for path in (root, *root.parents)):
        raise ValueError('symlink root or ancestor forbidden')
    return root


def _file(root, relative, fact):
    if (not isinstance(relative, str) or not relative or relative != relative.strip()
            or any(c in relative for c in ('\\', ':', '?', '#'))
            or any(part in ('', '.', '..') for part in relative.split('/'))):
        raise ValueError('canonical relative source path required')
    path = safe_path(root, relative)
    if not path.is_file():
        raise ValueError('missing approved input: ' + relative)
    data = path.read_bytes()
    if type(fact.get('bytes')) is not int or len(data) != fact['bytes']:
        raise ValueError('approved input byte length changed: ' + relative)
    if hashlib.sha256(data).hexdigest() != fact['sha256']:
        raise ValueError('approved input hash changed: ' + relative)
    return path


def expected_mapping():
    """A closed design trace beside unchanged global source/adapter contracts."""
    return {
        'supplement_id': SUPPLEMENT_ID,
        'approval_record': {'path': RECORD, **copy.deepcopy(PINNED_INPUTS[RECORD])},
        'manifest': {'path': MANIFEST, **copy.deepcopy(PINNED_INPUTS[MANIFEST])},
        'entry': ENTRY,
        'source_count': len(PROMOTED_SOURCES),
        'viewports': copy.deepcopy(VIEWPORTS),
        'themes': ['dark', 'light'],
        'zoom': [1, 2],
        'evidence_role': 'design_source_only',
    }


def validate_mapping(mapping):
    if type(mapping) is not dict:
        raise ValueError('exact general settings source mapping required')
    try:
        _json_value(mapping)
        matches = _same(mapping, expected_mapping())
    except (TypeError, ValueError) as error:
        raise ValueError('mapping must contain exact finite JSON fields and types') from error
    if not matches:
        raise ValueError('general settings mapping differs from approved source identity or scope')
    return copy.deepcopy(mapping)


def expected_test_supersession():
    """Exact test-definition migration, never a runtime skip or pass receipt.

    The caller must still require and actually execute the replacement check,
    validate its complete case set and retain any failed overall gate result.
    This trace alone cannot label an unexecuted historical test as passed.
    """
    return {
        'kind': 'approved-general-settings-test-definition-migration/v1',
        'supplement_id': SUPPLEMENT_ID,
        'approval_record': {'path': RECORD, **copy.deepcopy(PINNED_INPUTS[RECORD])},
        'authority_check': AUTHORITY_CHECK, 'authority_script': AUTHORITY_SCRIPT,
        'superseded_check': {
            'id': LEGACY_CHECK, 'test': copy.deepcopy(LEGACY_TEST), 'case_count': 5,
        },
        'replacement_check': {'id': REPLACEMENT_CHECK, 'test': REPLACEMENT_TEST},
        'preserved_obligations': list(PRESERVED_TEST_OBLIGATIONS),
        'prior_geometry_supplement': 'layout-refinements-20261107',
        'replacement_geometry_supplement': SUPPLEMENT_ID,
        'replacement_execution_required': True,
        'evidence_role': 'test_definition_migration_only',
        'acceptance_granted': False,
    }


def validate_test_supersession(mapping, root=ROOT):
    """Validate exact migration authority and unchanged old test bytes only."""
    if type(mapping) is not dict:
        raise ValueError('exact general settings test-definition migration required')
    try:
        _json_value(mapping)
        matches = _same(mapping, expected_test_supersession())
    except (TypeError, ValueError) as error:
        raise ValueError('test-definition migration requires exact JSON fields and types') from error
    if not matches:
        raise ValueError('test-definition migration differs from the exact old/new checks and obligations')
    _approved_inputs(root)
    _file(_root(root), LEGACY_TEST['path'], LEGACY_TEST)
    return copy.deepcopy(mapping)


def _approved_inputs(root):
    root = _root(root)
    paths = {relative: _file(root, relative, fact) for relative, fact in PINNED_INPUTS.items()}
    record = _load(paths[RECORD])
    for key, expected in {
        'schema_version': 1, 'kind': 'user-approved-general-settings-structure/v1',
        'status': 'approved_by_user', 'decision_basis': 'explicit_user_message',
        'user_confirmation': USER_CONFIRMATION, 'target_id': TARGET_ID,
        'composition_id': COMPOSITION_ID, 'supplement_id': SUPPLEMENT_ID,
        'promoted_sources': PROMOTED_SOURCES, 'approved_scope': APPROVED_SCOPE,
        'viewports': VIEWPORTS, 'runtime_dependencies': RUNTIME_DEPENDENCIES,
        'parent_approval': PARENT_APPROVAL, 'relocation': RELOCATION,
        'captured_records': CAPTURED_RECORDS, 'reviewed_sources': REVIEWED_SOURCES,
        'reviewed_manifest': REVIEWED_MANIFEST, 'reviewed_freeze': REVIEWED_FREEZE,
        'not_granted': NOT_GRANTED,
    }.items():
        if not _same(record.get(key), expected):
            raise ValueError('explicit source approval differs: ' + key)
    manifest = _load(paths[MANIFEST])
    for key, expected in {
        'schema_version': 1, 'kind': 'relocated-user-reviewed-general-settings-source/v1',
        'supplement_id': SUPPLEMENT_ID, 'entry': PREFIX + 'index.html', 'canvas': ENTRY,
        'sources': PROMOTED_SOURCES, 'promotion': RELOCATION,
        'captured_records': CAPTURED_RECORDS, 'dependencies': RUNTIME_DEPENDENCIES,
        'viewports': VIEWPORTS, 'themes': ['dark', 'light'], 'zoom': [1, 2],
        'product_accepted': False,
    }.items():
        if not _same(manifest.get(key), expected):
            raise ValueError('promoted design manifest differs: ' + key)
    # Verify relocation from captured bytes, not a metadata claim of equivalence.
    for relocation in RELOCATION:
        captured = paths[relocation['captured']['path']].read_bytes()
        promoted = paths[relocation['promoted']['path']].read_bytes()
        if relocation['transformation'] == 'two_dependency_relative_urls_only':
            for old, new in (
                (b'../../../../docs/design/first-product-v1/material-tokens.css', b'../../material-tokens.css'),
                (b'../../../../docs/design/first-product-v1/icons.js', b'../../icons.js'),
            ):
                if captured.count(old) != 1:
                    raise ValueError('captured HTML dependency relocation is not unique')
                captured = captured.replace(old, new)
        elif relocation['transformation'] != 'verbatim':
            raise ValueError('unapproved design transformation')
        if promoted != captured:
            raise ValueError('promoted source differs beyond the exact dependency relocation')
    captured_manifest = _load(paths[CAPTURED_RECORDS[0]['path']])
    captured_freeze = _load(paths[CAPTURED_RECORDS[1]['path']])
    for captured in (captured_manifest, captured_freeze):
        if captured.get('candidate_id') != 'general-settings-structure-v3-20261107' or captured.get('source_approval') != 'pending':
            raise ValueError('original captured labels or candidate identity changed')
        if captured.get('runtime_modified_by_candidate') is not False:
            raise ValueError('captured design cannot assert runtime modification')
    if not _same(captured_manifest.get('sources'), REVIEWED_SOURCES):
        raise ValueError('captured manifest source closure differs')
    if not _same(captured_freeze.get('design_files'), REVIEWED_SOURCES):
        raise ValueError('captured freeze source closure differs')
    if not _same(captured_freeze.get('manifest'), REVIEWED_MANIFEST):
        raise ValueError('captured freeze manifest differs')
    parent = _load(paths[PARENT_APPROVAL['path']])
    if (parent.get('status') != 'approved_by_user' or parent.get('approved_source_count') != 9
            or parent.get('target_id') != TARGET_ID or parent.get('composition_id') != COMPOSITION_ID
            or not _same(parent.get('parent_approvals'), PARENT_RECORDS)):
        raise ValueError('three-page supplement parent chain differs')
    for relative, count in (
        ('docs/development/design-approval-complete-20261031.json', 61),
        ('docs/development/design-approval-client-layout-20261105.json', 3),
        ('docs/development/design-approval-preview-20261104.json', 5),
    ):
        parent = _load(paths[relative])
        sources = parent.get('design_sources') if count == 61 else parent.get('approved_sources')
        if parent.get('status') != 'approved_by_user' or not isinstance(sources, dict) or len(sources) != count:
            raise ValueError('61-source/C1/C35 parent approval differs')
    return set(paths)


def approval_inputs(root=ROOT):
    """Return validated permanent fixture inputs; no old .build directory needed."""
    return _approved_inputs(root)


def assess(root=ROOT, mapping=None):
    result = {
        'schema_version': 1, 'kind': 'general-settings-source-authority/v1',
        'supplement_id': SUPPLEMENT_ID, 'target_id': TARGET_ID, 'composition_id': COMPOSITION_ID,
        'status': 'failed', 'source_ready': False, 'entry': ENTRY,
        'approval_record': {'path': RECORD, **copy.deepcopy(PINNED_INPUTS[RECORD])},
        'approved_source_count': len(PROMOTED_SOURCES),
        'captured_source_count': len(RELOCATION),
        'source_scope': 'general_settings_supplement_only', 'mapping_validated': False,
        'parent_source_counts': {'complete': 61, 'C1': 3, 'C35': 5, 'layout': 9},
        'parent_closure_scope': 'pinned parent approvals and nine-source layout; full global 61-source integrity remains a separate mandatory gate',
        'component_evidence_status': 'not_inspected', 'reference_approval_granted': False,
        'product_alignment_approved': False, 'real_component_verified': False,
        'native_verified': False, 'release_accepted': False,
        'product_completion': 'incomplete', 'next_stage_allowed': False,
        'not_granted': list(NOT_GRANTED), 'errors': [],
    }
    try:
        _approved_inputs(root)
        if mapping is not None:
            result['mapping'] = validate_mapping(mapping)
            result['mapping_validated'] = True
        result.update(status='approved_by_user', source_ready=True)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        result['errors'].append('general settings source invalid: ' + str(error))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--mapping', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    try:
        mapping = _load(args.mapping) if args.mapping else None
        result = assess(args.root, mapping)
        if args.mapping and mapping is None:
            raise ValueError('mapping file must contain the exact object')
    except (OSError, ValueError, TypeError) as error:
        result = assess(args.root)
        result.update(status='failed', source_ready=False)
        result['errors'].append('invalid mapping input: ' + str(error))
    rendered = json.dumps(result, ensure_ascii=False, indent=2) + '\n'
    if args.output:
        try:
            root = _root(args.root)
            output = args.output.absolute()
            relative = str(output.relative_to(root))
            if not output.is_relative_to(root / '.build'):
                raise ValueError('authority output must be under the selected root .build')
            safe_path(root, relative)
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(rendered, encoding='utf-8')
        except (OSError, ValueError) as error:
            parser.error(str(error))
    print(rendered, end='')
    return 0 if result['source_ready'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
