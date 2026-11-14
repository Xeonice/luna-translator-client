#!/usr/bin/env python3
"""Independent schema-five inspection; SQL guards never grant product acceptance.

The expected shape is built exclusively from frozen design SQL. Runtime PASS
requires externally bound execution context and actual sealed SQLite artifacts.
The complete-v3 adapter is deliberately not extended by this module.
"""
from __future__ import annotations

import argparse
import copy
from datetime import datetime
import hashlib
import json
import re
import secrets
import shutil
import sqlite3
import subprocess
import tempfile
from contextlib import contextmanager
from pathlib import Path

import storage_schema as v1
import storage_schema_v4 as v4
from database_design import ROOT

DESIGN = 'docs/architecture/database/v5/005_source_selection_sets.design.sql'
CONTRACT = 'docs/architecture/database/v5/schema-contract.json'
BOUNDARIES = 'docs/architecture/database/v5/field-boundaries.json'
DESIGN_SHA256 = '114b32d5ee4dccbf511ebf3c489dffc0c4e7f96143b4f9656bb544a93fae6c18'
PRIOR_CONTRACT_SHA256 = '273760d1a330bbcb19e09e879597ecb894484e66a2fe68210d1f11415afdd470'
PROFILE_LEDGER_SHA256 = '1f26620f1b22307ae07851c1e09c7ed44f888f6b9f88e9d9c34b6f7f1f3302c7'
OLD4_MANIFEST_SHA256 = '79f66e48889567022af8b91d2a7d42d1b8f230fefca38f82a402cdb3419964d9'
OLD4_SOURCE_MANIFEST_SHA256 = 'fc5d5dfe6ed70de519714ea0d4cf00394b3f492a910f2fdbf2aeb8606c53d518'
OLD4_SOURCE_TREE_SHA256 = 'efcee0f066d620d91a851744a0d38c0a567ed976b70aac0f41804a7bffa842f0'
OLD_SUITE_REFS = {'dependent': {'path': '.build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/results.json',
               'sha256': '71c7f47cfd1c4b627089d706d65dc3711fd3c96dcaee63573378cc2de69dc373'},
 'lists': {'path': '.build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/inventory.json',
           'sha256': '2fca4feada8788ecf05b9fddb45777c3cc3aa122d31bef9a5118a0bc8d10e37e'},
 'schema': {'path': '.build/binding-journey-alignment-20261109/frozen-store4/original-schema-suite/result.json',
            'sha256': '9661e5535db91b7c414746609e93d226d6a6095e1212ba18bb26f9971123320f'}}
# Historical stdout/stderr byte identities from the independently sealed safe
# ancestor archive. These pins never derive product behavior or expected DDL.
OLD_SUITE_LOG_SHA256 = {
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/session_records_batch.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/session_records_batch.stdout.log": "65de4fef7b4baad3b02b71c9d05562036972357b6840a7ccbbfb1938410a26c5",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_acceptance_routes.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_acceptance_routes.stdout.log": "9069f935b996617211776c172834f13b67100cf401ca119162d357b241cb3dda",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_http_identity_crash.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_http_identity_crash.stdout.log": "2f05927ac00942d6d19222b7d4a8bcab60043a8a1787951ff57d9c03cebecbf6",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_http_identity_v3.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_http_identity_v3.stdout.log": "acd2fb5786a713adf5db58f5af1f8df68e82012c1b8bcf129b4e9fb1f0bb60cd",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_product.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_product.stdout.log": "20df833b511b549dde287fb7844546f8781ba27f1ecf84d07c4537e37aceb388",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_service_graph_crash.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_service_graph_crash.stdout.log": "490e6a9be09f3f27b2efa24afa785a203e3b728786012c80e8ae90a8507f2007",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_service_graph_v4.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_service_graph_v4.stdout.log": "2f2904d3b31c1c8ce2f67c2cccf625c3b3aa3ae8e5f85748affa1ead8c1e4e2d",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_session_v2.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/storage_session_v2.stdout.log": "0310afdc550b160afaf1997b23dde3644718c96f1ed382e4b05dbb4b28757e3d",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites/compile.stderr.log": "336fab24cc3a4e775f7811ffb7e79dba88e06819472fac9ea0de2a8056230a5b",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites/compile.stdout.log": "62e316e6668ade2bc18ee78324e2fa6193775c7656c56f2b7988b280491645ee",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-schema-suite/compile.stderr.log": "d58248a129accf5c569db8dc43cc7b42170dbaa9861e6d43c4a86a1b263ce527",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-schema-suite/compile.stdout.log": "ec2cb78afbf9fcb16eed702dffdbe9907c23bc4e792307ba292487c25d3879a4",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-schema-suite/tests.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-schema-suite/tests.stdout.log": "c0e442068a26cd2cd3d277959ac48bfef9136be619b2d2d1079349731ec12d02",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/session_records_batch.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/session_records_batch.stdout.log": "565cb19bbb9943a4310fd8f387479218216df7d4bba8e27bd14d9082193d4fae",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_acceptance_routes.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_acceptance_routes.stdout.log": "415c39deb3cfa4a8ec9d595905f7555f8a1d8c57f9b7e27d4bb3efac66fbc712",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_http_identity_crash.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_http_identity_crash.stdout.log": "fc541082314ad30195fd949de77f3ec4bb21795f3785a1b5a3de40025c34689b",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_http_identity_v3.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_http_identity_v3.stdout.log": "21385f87a6a949506c5053afe4fad5f7b44e9346a1f3e8821ae7286b11cadc1d",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_product.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_product.stdout.log": "4a151cbab552072ae45ecf59dcc3e32ce2390f2976b8196011b79b9b847d7a91",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_schema_contract.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_schema_contract.stdout.log": "ae6f805d3262436be1ac36b770263737f8483b5a638dd2457ec644e31926860c",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_service_graph_crash.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_service_graph_crash.stdout.log": "b11d0a01e5e22c00a30398606efde8128d69480ba248a4a9f1cbbad0798dd666",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_service_graph_v4.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_service_graph_v4.stdout.log": "a02387108a6714273feb6625047022e4c8e9fcb8853d3c2ce83edc27db86e05c",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_session_v2.stderr.log": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/storage_session_v2.stdout.log": "e7a5754e467a64710d9bb89a4fe8cc3602485646c7bf248ebda196c4878dcb20"
}
PRODUCTION_SOURCE = 'crates/app-infra/tests/source_selection_sets.rs'
RUNTIME_FILENAME = 'storage-v5-runtime.json'
RUNTIME_KIND = 'source-selection-schema5-runtime/v1'
RUNTIME_KIND_V2 = 'source-selection-schema5-runtime/v2'
CHILD_KIND = 'schema5-child-execution/v1'
TIMELINE_POLICY = 'openspec/changes/align-approved-binding-journey/phase-policy.json'
CHILD_TESTS = {
    'rollback': 'storage::database::source_selection_migration_fault::actual_schema5_host_rolls_back_after_ddl_and_recovers_same_original_profile',
    'readonly': 'storage::database::source_selection_migration_fault::actual_schema5_host_readonly_and_busy_leave_the_original_unchanged',
    'busy': 'storage::database::source_selection_migration_fault::actual_schema5_host_readonly_and_busy_leave_the_original_unchanged',
    'enospc': 'storage::database::source_selection_migration_fault::actual_schema5_host_full_isolated_volume_preserves_the_old_database',
}
CHILD_COMPILE = ['cargo', 'test', '--locked', '--offline', '-p', 'app-infra', '--lib', '--no-run', '--message-format=json']
BOUNDARY_MARKER = 'LUNA_STORAGE_V5_BOUNDARY '
FAULT_MARKER = 'LUNA_STORAGE_V5_FAULT '
CHILD_ID_MARKER = 'LUNA_STORAGE_V5_CHILD_IDENTITY '
CHILD_LAUNCH_MARKER = 'LUNA_STORAGE_V5_CHILD_LAUNCH '
OLD4_OBSERVER_MARKER = 'LUNA_STORAGE_V5_OLD4_OBSERVATION '
TEST_NAMES = ('bj_08_storage', 'bj_09_storage', 'bj_11_storage', 'bj_12_storage', 'bj_17_storage')
OLD_TEST_TARGETS = ('storage_schema_contract', 'storage_service_graph_v4', 'storage_service_graph_crash',
                    'storage_http_identity_v3', 'storage_http_identity_crash', 'storage_acceptance_routes',
                    'storage_session_v2', 'storage_product', 'session_records_batch')
NEW_COLUMNS = {
    'source_selection_sets': ('session_id', 'selection_set_revision', 'operation_id', 'origin_kind',
                              'persistence_kind', 'match_snapshot_json', 'selected_at_ms', 'member_count', 'state'),
    'source_selection_members': ('session_id', 'selection_set_revision', 'source_id', 'selection_revision', 'member_ordinal'),
}
TRIGGERS = ('source_selection_set_building_insert', 'source_selection_member_building_insert',
            'source_selection_set_seal', 'immutable_source_selection_members_update',
            'immutable_source_selection_members_delete', 'immutable_source_selection_sets_delete')
VIEW = 'current_source_selection_members'
FIELD_IDS = tuple('profile.' + table + '.' + column for table, columns in NEW_COLUMNS.items() for column in columns)
# Code-owned identities. These are preservation pins, never sources of expected DDL.
PRESERVED = {'crates/app-infra/src/storage/migrations/001_config.sql': '7199346c9b4329d7580d24f2edbaee0000fb40f4103ddccd14ce8fdd6c169293',
 'crates/app-infra/src/storage/migrations/001_dictionary.sql': 'ed11958cf6aa2533d76eec868077b085ef2b821bb530e8f67f0a91223b3276b3',
 'crates/app-infra/src/storage/migrations/001_profile.sql': 'cd70b9e815842c5eb5c2f8410cf88b31d0097eb65200fa94f85a7b92fb19bca5',
 'crates/app-infra/src/storage/migrations/002_session_records.sql': 'ec4d1470721b9d81e20a3c5e1bb12aa6f59a2b3c32f6310691f332394781ca2e',
 'crates/app-infra/src/storage/migrations/003_http_service_identity.sql': '813145f4923f28860ad2035abf5df8f5b9dbf68753b45510a2b794f80d3c43c2',
 'crates/app-infra/src/storage/migrations/004_service_graph.sql': '09eab3339ed56f535f72cc95d6d12cab069c73d6cd47a07197dd69f561351d55',
 'crates/app-infra/tests/fixtures/http-schema2-original-input.json': 'b6d97456861dfe12bda8d218c9ec105449d998edc939b89b20be4f32ab3a31a2',
 'crates/app-infra/tests/fixtures/http-schema2-original-loaded.json': 'fbea6b54620fdf51eea3b2f7f931fea2299f6538b15429a352dc2f1aeb0f128d',
 'crates/app-infra/tests/fixtures/http-schema2-original-manifest.json': '2d24b7ced2dfb0b50b3cf9d5fc590f33f480d32b0fbfc94cf6529f5eda71fbac',
 'crates/app-infra/tests/fixtures/http-schema2-original.sql': 'ac4b9dd97dc464c1a039170e529cf3e3db4490029ceb4342b5f76a7a4665b725',
 'docs/architecture/database/v2/002_session_records.design.sql': 'ec4d1470721b9d81e20a3c5e1bb12aa6f59a2b3c32f6310691f332394781ca2e',
 'docs/architecture/database/v2/README.md': 'c30af06e65b73ca4b0de522ceb1f9c572a0a74cd4e8a88cfe4daf2e2239750bc',
 'docs/architecture/database/v2/field-dictionary.csv': 'ee483a95f0f6535c9c52b7f3ce885ab64520839b49a8b08f54449e2c05de69f4',
 'docs/architecture/database/v2/field-dictionary.json': 'fae0561b8cd9113dfeaf9b34a4d9557ed523737b1217d9998a8ce68c5a143f20',
 'docs/architecture/database/v2/schema-contract.json': 'eb3b0e78869119f98eac6d08954f5b91f1131a3edf2af85fcd16af85d6154713',
 'docs/architecture/database/v2/storage-acceptance.json': '1ca95fec54ecb1af2f4f2a0791010605604bc5662a354e9ad1f2036bacc7de70',
 'docs/architecture/database/v2/storage-review.json': 'e0b37a166e9f74e5d493fc4f3c45f8c28f5da55f0fec6cccacce251a95335484',
 'docs/architecture/database/v3/003_http_service_identity.design.sql': '813145f4923f28860ad2035abf5df8f5b9dbf68753b45510a2b794f80d3c43c2',
 'docs/architecture/database/v3/README.md': '71caa0d8f1424d1e15d97eb7926f9d6a45dba925090bc0b4e1a1691a68e9a1df',
 'docs/architecture/database/v3/boundary-fixtures.json': '02ab9ee4c1e1f7361e6acc6eb0c768141c2f85af1ebd2dfe1d003608df96e9d1',
 'docs/architecture/database/v3/field-boundaries.json': '174d61829a681c204fb3b600354c010ccd31b0dfc50a5cfd25f91792e1d4038e',
 'docs/architecture/database/v3/field-dictionary.csv': 'f2008ab47ac90143e99d019da3072b371d4cf65af133b2ecb64f4fde67ea7c5d',
 'docs/architecture/database/v3/field-dictionary.json': 'b3144b3820ae1bd1c91ea06343bfa85a954eb7cdc6950f9bae21c5ae988d349a',
 'docs/architecture/database/v3/schema-contract.json': '90d3474fb23c4522845727307ea3b47b5a2ae0a5f0b6df40006516aa22fbe335',
 'docs/architecture/database/v3/storage-acceptance.json': '808822b6b335ab6e77daebb0fd4c44f29e6b1acc0fedbbef4bc7fd67a6b16b5d',
 'docs/architecture/database/v3/storage-review.json': '904e1a150f4748c3ff4eb1eefd41f6d1cc4afb1153d9740b4e30b1754227f981',
 'docs/architecture/database/v4/004_service_graph.design.sql': '09eab3339ed56f535f72cc95d6d12cab069c73d6cd47a07197dd69f561351d55',
 'docs/architecture/database/v4/README.md': 'a669587b821c7e276a8a2ec8d09716d63ca3240802ec83cfbe1cc4bd15b3197c',
 'docs/architecture/database/v4/boundary-fixtures.json': 'b49ebcd850d628e8e6a7cdd3f2de4c55bf715d74397751c65b46ffd12f6c2a99',
 'docs/architecture/database/v4/field-boundaries.json': '33449b111d00d8de721035d2980dd13f05f5c48b50509689fa5906a5209e9d07',
 'docs/architecture/database/v4/field-dictionary.csv': 'd8084c5388463db564aa70f5411f9275e77b14f97fe5c8bc079fd6aeb98d1d97',
 'docs/architecture/database/v4/field-dictionary.json': '65ece728e469c4b84e26a7df4d3b55af55fd06149cd99201aa910937152157dd',
 'docs/architecture/database/v4/schema-contract.json': '273760d1a330bbcb19e09e879597ecb894484e66a2fe68210d1f11415afdd470',
 'docs/architecture/database/v4/storage-acceptance.json': '4c93c3ea14345f64150c67a99cf81b4e0eab2a8ebac951f4d9d73ce8345bf12a',
 'docs/architecture/database/v4/storage-review.json': '7620d106e7551b2acb02785187ebe5d8b24509a6776aae7ac83518439ceee49c',
 'scripts/quality/storage_schema.py': 'aba76adf898f071def88e2ff758b3aad3c1f192138a14430a61e78a1afc1edcd',
 'scripts/quality/storage_schema_v2.py': '5c070bee74034dc8819bd876b34e884477f3077327e8f54d7b8e1f29ea9454c9',
 'scripts/quality/storage_schema_v3.py': '489380a8a349d5576d6a6478e47f02e9cd3f7f56a8689c0aadce0768c12efee8',
 'scripts/quality/storage_schema_v4.py': '2e0d1564e724a316bc7797aee8f2a41132746dae3e9749ef1108dff69e5a3199'}
TARGET_PINS = {'docs/architecture/database/v5/field-boundaries.json': 'ce058ff610755a93bf6bed94331d8742f544e22129e1be5e015b8e0829d0cdb7',
 'docs/architecture/database/v5/schema-contract.json': 'c62428a569bc7cf1d6f217b17414bb02e750a4f8b8cd5da45f1a4049009476ed'}
HEX64 = re.compile(r'[0-9a-f]{64}\Z')
NONCE = re.compile(r'[0-9a-f]{32}\Z')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical_hash(value):
    return digest(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode())


def safe_bytes(root, relative):
    root = Path(root).resolve()
    path = Path(relative)
    if path.is_absolute() or not path.parts or any(part in ('..', '.') for part in path.parts):
        raise ValueError('repository-relative file required: ' + str(relative))
    target = root / path
    if target.is_symlink() or any(p.is_symlink() for p in target.parents) or not target.resolve().is_relative_to(root):
        raise ValueError('symbolic-link or escaping input: ' + str(relative))
    return target.read_bytes()


def read_json(root, path):
    return _loads(safe_bytes(root, path))


def _loads(data):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('duplicate JSON key: ' + key)
            result[key] = value
        return result
    def invalid(value):
        raise ValueError('nonfinite JSON number: ' + value)
    return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid)


def _milliseconds(value):
    parsed = datetime.fromisoformat(value)
    if parsed.tzinfo is None:
        raise ValueError('execution timestamp must include its timezone')
    return int(parsed.timestamp() * 1000)


@contextmanager
def design_database(root=ROOT):
    data = safe_bytes(root, DESIGN)
    if digest(data) != DESIGN_SHA256:
        raise ValueError('frozen independent005 design changed')
    with v4.design_database(Path(root)) as db:
        db.executescript(data.decode())
        if db.execute('PRAGMA foreign_key_check').fetchall():
            raise ValueError('schema5 independent design has bad foreign keys')
        yield db


def _preservation(before, after):
    if set(after['tables']) - set(before['tables']) != set(NEW_COLUMNS) or set(before['tables']) - set(after['tables']):
        raise ValueError('schema5 must add exactly the two selection tables')
    if any(after['tables'][name] != table for name, table in before['tables'].items()):
        raise ValueError('schema5 changed an original full table/index/FK/trigger shape')
    if {k: v for k, v in after['views'].items() if k != VIEW} != before['views'] or VIEW not in after['views']:
        raise ValueError('schema5 must retain every prior view and add exactly current members')
    if len(after['tables']) != 83 or sum(len(t['columns']) for t in after['tables'].values()) != 748:
        raise ValueError('schema5 complete profile inventory must be83 tables/748 fields')
    for table, columns in NEW_COLUMNS.items():
        if tuple(c['name'] for c in after['tables'][table]['columns']) != columns:
            raise ValueError('schema5 new field inventory differs: ' + table)
    triggers = {name for table in NEW_COLUMNS for name in after['tables'][table]['triggers']}
    if triggers != set(TRIGGERS) or sum(len(after['tables'][table]['foreign_keys']) for table in NEW_COLUMNS) != 5:
        raise ValueError('schema5 must have all five FKs and six exact triggers')


def design_contract(root=ROOT):
    prior = v4.design_contract(Path(root))
    with design_database(root) as db:
        profile = v1.inspect_connection(db)
    _preservation(prior['databases']['profile'], profile)
    result = copy.deepcopy(prior)
    result['databases']['profile'] = profile
    result['ddl_source_sha256'][DESIGN] = DESIGN_SHA256
    result['prior_contract_sha256'] = PRIOR_CONTRACT_SHA256
    result['phase'] = 'independent_schema_five_source_selection_design_pending_non_author_review'
    result['migration_version'] = 5
    return result


def validate_contract(root=ROOT):
    try:
        errors = []
        for path, expected in (PRESERVED | TARGET_PINS).items():
            if digest(safe_bytes(root, path)) != expected:
                errors.append('frozen identity changed: ' + path)
        if errors:
            return errors
        errors += v4.validate_schema_contract(Path(root))
        errors += v1._diff(read_json(root, CONTRACT), design_contract(root), 'schema_five_contract')
        boundary = read_json(root, BOUNDARIES)
        if boundary.get('field_ids') != list(FIELD_IDS) or boundary.get('field_count') != 14 or boundary.get('max_selection_members') != 64:
            errors.append('schema5 frozen fourteen-field/64-member boundary inventory differs')
        return errors
    except (OSError, ValueError, TypeError, KeyError, sqlite3.Error) as error:
        return ['schema5 contract: ' + str(error)]


validate_schema_contract = validate_contract


def compare_current_schema(root, candidate, database='profile'):
    return validate_contract(root) or v1.compare_schema_file(Path(root), candidate, database, CONTRACT)


def _report(kind, errors, **detail):
    return {'schema_version': 1, 'kind': kind, 'status': 'failed' if errors else 'passed',
            'product_migration_executed': False, 'product_acceptance': False,
            'complete_product_storage_adapter': False, 'errors': errors, **detail}


def assess(root=ROOT, candidate=None, database='profile'):
    errors = compare_current_schema(root, candidate, database) if candidate else validate_contract(root)
    return _report('schema_five_contract_receipt', errors, scope='independent_complete_shape_only')


def _clone(source):
    target = sqlite3.connect(':memory:')
    source.backup(target)
    target.execute('PRAGMA foreign_keys=ON')
    return target


def _insert(db, table, values):
    columns = ','.join(v1._name(name) for name in values)
    db.execute('INSERT INTO ' + v1._name(table) + '(' + columns + ') VALUES(' + ','.join('?' for _ in values) + ')', tuple(values.values()))


def _field_probes(source):
    rows = []
    bases = {
        'source_selection_sets': dict(session_id='guard-session', selection_set_revision=1, operation_id='guard-op',
                                      origin_kind='explicit_set', persistence_kind='temporary', match_snapshot_json='{}',
                                      selected_at_ms=0, member_count=1, state='building'),
        'source_selection_members': dict(session_id='guard-session', selection_set_revision=1, source_id='guard-source',
                                         selection_revision=1, member_ordinal=0),
    }
    domains = dict(session_id='', selection_set_revision=0, operation_id='', origin_kind='unknown',
                   persistence_kind='unknown', match_snapshot_json='[]', selected_at_ms=-1,
                   member_count=0, state='unknown', source_id='', selection_revision=0, member_ordinal=64)
    for table, columns in NEW_COLUMNS.items():
        ddl = source.execute("SELECT sql FROM sqlite_schema WHERE type='table' AND name=?", (table,)).fetchone()[0]
        for column in columns:
            outcomes = {}
            modes = ['valid', 'type', 'null', 'domain', 'coercion']
            if column in ('member_count', 'member_ordinal'):
                modes += ['maximum', 'above_maximum']
            for mode in modes:
                db = sqlite3.connect(':memory:')
                try:
                    db.execute('PRAGMA foreign_keys=OFF')
                    db.execute(ddl)
                    values = dict(bases[table])
                    expected = mode in ('valid', 'coercion', 'maximum')
                    if mode == 'type':
                        values[column] = b'not-a-scalar'
                    elif mode == 'null':
                        values[column] = None
                        expected = column == 'operation_id'
                        if expected:
                            values['origin_kind'] = 'legacy_singleton'
                    elif mode == 'domain':
                        values[column] = domains[column]
                    elif mode == 'coercion':
                        # SQLite STRICT accepts lossless numeric text for INTEGER;
                        # typed request validation must still run in the Rust producer.
                        if isinstance(values[column], int):
                            values[column] = str(values[column])
                    elif mode in ('maximum', 'above_maximum'):
                        values[column] = (64 if column == 'member_count' else 63) + int(mode == 'above_maximum')
                    try:
                        _insert(db, table, values)
                        actual = True
                    except sqlite3.IntegrityError:
                        actual = False
                    if actual != expected:
                        raise ValueError('field boundary differs: ' + table + '.' + column + '/' + mode)
                    outcomes[mode] = 'accept' if actual else 'reject'
                finally:
                    db.close()
            rows.append({'id': 'profile.' + table + '.' + column, 'outcomes': outcomes})
    return rows


def _seed(db):
    prefix = 'schema5-guard-' + secrets.token_hex(8)
    game, resource, session, unowned = (prefix + suffix for suffix in ('-game', '-resource', '-session', '-unowned'))
    db.execute('INSERT INTO games(game_id,display_name,row_revision,created_at_ms,updated_at_ms) VALUES(?,?,1,0,0)', (game, 'guard'))
    db.execute("INSERT INTO resource_snapshots VALUES(?,'core','guard',?,'1','{}',0)", (resource, digest(prefix.encode())))
    for sid, gid in ((session, game), (unowned, None)):
        db.execute("INSERT INTO sessions VALUES(?,?,'hook','active','guard-env','guard.exe',1,'1','x86','guard-broker',?,?,0,NULL)",
                   (sid, gid, resource, digest(sid.encode())))
    sources = [prefix + '-source-' + str(n) for n in range(3)]
    for n, source in enumerate(sources):
        db.execute("INSERT INTO capture_sources VALUES(?,?,'host','18446744073709551615','9007199254740993','1','code','hook','utf8',?,'{}',0)",
                   (source, session, digest(source.encode())))
        db.execute("INSERT INTO source_selections VALUES(?,?,?,'temporary','{}',0)", (session, n + 1, source))
    db.commit()
    return dict(prefix=prefix, game=game, resource=resource, session=session, unowned=unowned, sources=sources)


def _header(db, f, revision=1, count=1, **changes):
    values = dict(session_id=f['session'], selection_set_revision=revision, operation_id=f['prefix'] + '-op-' + str(revision),
                  origin_kind='explicit_set', persistence_kind='temporary', match_snapshot_json='{}', selected_at_ms=0,
                  member_count=count, state='building')
    values.update(changes)
    _insert(db, 'source_selection_sets', values)
    return values


def _member(db, f, revision=1, ordinal=0, selection=1, source=None, **changes):
    values = dict(session_id=f['session'], selection_set_revision=revision, source_id=source or f['sources'][selection - 1],
                  selection_revision=selection, member_ordinal=ordinal)
    values.update(changes)
    _insert(db, 'source_selection_members', values)


def _receipt(db, header):
    if header['operation_id'] is not None:
        db.execute("INSERT INTO operation_receipts VALUES(?,?,'begin_reading',1,'{}',0)", (header['operation_id'], 'a' * 64))


def _sealed(db, f, revision=1, selections=(1,)):
    header = _header(db, f, revision, len(selections))
    for ordinal, selection in enumerate(selections):
        _member(db, f, revision, ordinal, selection)
    db.execute("UPDATE source_selection_sets SET state='sealed' WHERE session_id=? AND selection_set_revision=?", (f['session'], revision))
    _receipt(db, header)
    return header


def _case(source, name, action, accepted=False, drop_triggers=False):
    db = _clone(source)
    try:
        if drop_triggers:
            for trigger in TRIGGERS:
                db.execute('DROP TRIGGER ' + v1._name(trigger))
        f = _seed(db)
        db.execute('BEGIN IMMEDIATE')
        try:
            action(db, f)
            db.commit()
            actual, reason = True, None
        except sqlite3.IntegrityError as error:
            actual, reason = False, str(error)
            db.rollback()
        if actual != accepted:
            raise ValueError('relational boundary differs: ' + name + ' (' + str(reason) + ')')
        if drop_triggers and not accepted and 'FOREIGN KEY constraint failed' not in (reason or ''):
            raise ValueError('foreign-key probe was rejected by another constraint: ' + name)
        return {'id': name, 'outcome': 'accept' if actual else 'reject', 'sqlite_error': reason}
    finally:
        db.close()


def _relational_probes(source):
    rows = []
    rows.append(_case(source, 'positive-sealed-two-members', lambda d, f: _sealed(d, f, selections=(1, 2)), True))
    rows.append(_case(source, 'fk:sets.session', lambda d, f: _header(d, f, session_id='missing', origin_kind='legacy_singleton', operation_id=None), drop_triggers=True))
    rows.append(_case(source, 'fk:sets.operation', lambda d, f: _header(d, f), drop_triggers=True))
    def member_fk(db, f, key):
        header = _header(db, f)
        _receipt(db, header)
        values = {'selection_set_revision': 2} if key == 'set' else ({'selection_revision': 99} if key == 'selection' else {'source': 'missing-source'})
        _member(db, f, **values)
    for key in ('set', 'selection', 'source'):
        rows.append(_case(source, 'fk:members.' + key, lambda d, f, k=key: member_fk(d, f, k), drop_triggers=True))
    rows.append(_case(source, TRIGGERS[0] + ':sealed-insert', lambda d, f: _header(d, f, state='sealed')))
    rows.append(_case(source, TRIGGERS[0] + ':not-next', lambda d, f: _header(d, f, revision=2)))
    rows.append(_case(source, TRIGGERS[0] + ':unowned-saved', lambda d, f: _header(d, f, session_id=f['unowned'], persistence_kind='saved_snapshot')))
    def wrong_member(db, f):
        _header(db, f)
        _member(db, f, source=f['sources'][1])
    rows.append(_case(source, TRIGGERS[1], wrong_member))
    def duplicate(db, f, what):
        _header(db, f, count=2)
        _member(db, f)
        _member(db, f, selection=1 if what == 'source' else 2, ordinal=1 if what == 'source' else 0)
    for key in ('source', 'ordinal'):
        rows.append(_case(source, 'duplicate-member-' + key, lambda d, f, k=key: duplicate(d, f, k)))
    def reused_revision(db, f):
        _sealed(db, f)
        _header(db, f, revision=2)
        _member(db, f, revision=2)
    rows.append(_case(source, 'duplicate-member-lineage', reused_revision))
    def cross_session(db, f):
        _header(db, f)
        _member(db, f, session_id=f['unowned'])
    rows.append(_case(source, 'cross-session-member', cross_session))
    def incomplete(db, f):
        _header(db, f, count=2)
        _member(db, f)
        db.execute("UPDATE source_selection_sets SET state='sealed' WHERE session_id=?", (f['session'],))
    rows.append(_case(source, TRIGGERS[2] + ':incomplete', incomplete))
    def gapped(db, f):
        _header(db, f, count=2)
        _member(db, f, ordinal=1)
        db.execute("UPDATE source_selection_sets SET state='sealed' WHERE session_id=?", (f['session'],))
    rows.append(_case(source, TRIGGERS[2] + ':gapped', gapped))
    def maximum(db, f):
        for number in range(4, 65):
            sid = f['prefix'] + '-source-' + str(number)
            db.execute("INSERT INTO capture_sources VALUES(?,?,'host','1','1','1','code','hook','utf8',?,'{}',0)",
                       (sid, f['session'], digest(sid.encode())))
            db.execute("INSERT INTO source_selections VALUES(?,?,?,'temporary','{}',0)", (f['session'], number, sid))
            f['sources'].append(sid)
        _sealed(db, f, selections=tuple(range(1, 65)))
    rows.append(_case(source, 'complete-64-members', maximum, True))
    rows.append(_case(source, 'reject-65-members', lambda d, f: _header(d, f, count=65)))
    rows.append(_case(source, 'reject-empty-set', lambda d, f: _header(d, f, count=0)))
    def mutate(db, f, statement):
        _sealed(db, f)
        db.execute(statement, (f['session'],))
    for name, statement in (
        (TRIGGERS[2] + ':sealed-update', "UPDATE source_selection_sets SET selected_at_ms=1 WHERE session_id=?"),
        (TRIGGERS[3], 'UPDATE source_selection_members SET member_ordinal=1 WHERE session_id=?'),
        (TRIGGERS[4], 'DELETE FROM source_selection_members WHERE session_id=?'),
        (TRIGGERS[5], 'DELETE FROM source_selection_sets WHERE session_id=?'),
    ):
        rows.append(_case(source, name, lambda d, f, sql=statement: mutate(d, f, sql)))
    db = _clone(source)
    try:
        f = _seed(db)
        _sealed(db, f, selections=(1, 2))
        db.commit()
        previous = [tuple(r) for r in db.execute('SELECT source_id,selection_revision FROM ' + VIEW + ' WHERE session_id=? ORDER BY member_ordinal', (f['session'],))]
        if previous != [(f['sources'][0], 1), (f['sources'][1], 2)]:
            raise ValueError('current-members view lost one selected lineage')
        header = _header(db, f, revision=2)
        if [tuple(r) for r in db.execute('SELECT source_id,selection_revision FROM ' + VIEW + ' WHERE session_id=? ORDER BY member_ordinal', (f['session'],))] != previous:
            raise ValueError('building set became current')
        _member(db, f, revision=2, selection=3)
        db.execute("UPDATE source_selection_sets SET state='sealed' WHERE session_id=? AND selection_set_revision=2", (f['session'],))
        _receipt(db, header)
        db.commit()
        if [tuple(r) for r in db.execute('SELECT source_id,selection_revision FROM ' + VIEW + ' WHERE session_id=?', (f['session'],))] != [(f['sources'][2], 3)]:
            raise ValueError('current-members view did not replace its entire head')
        rows.append({'id': VIEW, 'outcome': 'exact_current_sealed_members', 'members_before': 2, 'members_after': 1})
    finally:
        db.close()
    return rows


def _boundary_facts(source):
    return {'fields': _field_probes(source), 'relationships_and_triggers': _relational_probes(source)}


def _backfill_probe(root):
    with v4.design_database(Path(root)) as db:
        f = _seed(db)
        original = _rows(db, 'source_selections')
        db.executescript(safe_bytes(root, DESIGN).decode())
        _legacy_backfill(db, original)
        current = [tuple(row) for row in db.execute('SELECT source_id,selection_revision FROM ' + VIEW + ' WHERE session_id=?', (f['session'],))]
        if current != [(f['sources'][-1], 3)]:
            raise ValueError('legacy latest singleton head was not preserved')
        return {'legacy_rows': len(original), 'original_selection_bytes_preserved': _rows(db, 'source_selections') == original,
                'legacy_operation_ids': None, 'current_member_revision': 3}


def _legacy_backfill(after, old_selections):
    for session, revision, source, persistence, snapshot, selected_at in old_selections:
        expected = (session, revision, None, 'legacy_singleton', persistence, snapshot, selected_at, 1, 'sealed')
        row = after.execute('SELECT * FROM source_selection_sets WHERE session_id=? AND selection_set_revision=?', (session, revision)).fetchone()
        member = after.execute('SELECT * FROM source_selection_members WHERE session_id=? AND selection_set_revision=?', (session, revision)).fetchone()
        if tuple(row or ()) != expected or tuple(member or ()) != (session, revision, source, revision, 0):
            raise ValueError('legacy singleton migration changed original selection lineage/snapshot/time')
    if after.execute('SELECT count(*) FROM source_selection_sets').fetchone()[0] != len(old_selections) or after.execute('SELECT count(*) FROM source_selection_members').fetchone()[0] != len(old_selections):
        raise ValueError('legacy backfill invented or omitted a set/member')


def assess_boundaries(root=ROOT):
    errors, facts = validate_contract(root), None
    if not errors:
        try:
            with design_database(root) as db:
                facts = _boundary_facts(db)
            facts['legacy_backfill'] = _backfill_probe(root)
        except (ValueError, sqlite3.Error, KeyError, TypeError) as error:
            errors.append('schema5 design boundary: ' + str(error))
    return _report('schema_five_design_boundary_receipt', errors, scope='independent_design_sql_guards_only', facts=facts)


def _artifact(base, ref):
    if not isinstance(ref, dict) or set(ref) != {'path', 'sha256'} or not HEX64.fullmatch(str(ref['sha256'])):
        raise ValueError('artifact must have exact path/sha256')
    data = safe_bytes(base, ref['path'])
    if digest(data) != ref['sha256']:
        raise ValueError('artifact hash mismatch: ' + str(ref['path']))
    return Path(base).resolve() / ref['path']


def _readonly(path):
    errors = v1.sealed_snapshot_errors(path)
    if errors:
        raise ValueError('; '.join(errors))
    db = sqlite3.connect(Path(path).resolve().as_uri() + '?mode=ro&immutable=1', uri=True)
    db.execute('PRAGMA query_only=ON')
    if db.execute('PRAGMA integrity_check').fetchall() != [('ok',)] or db.execute('PRAGMA foreign_key_check').fetchall():
        db.close()
        raise ValueError('actual SQLite integrity or FK failure')
    return db


def _rows(db, table):
    return sorted((tuple(row) for row in db.execute('SELECT * FROM ' + v1._name(table))), key=repr)


def _old_rows_preserved(before, after, *, now_ms=None, server_id=None):
    # Reopen recovery legitimately changes only these lifecycle columns. Every
    # other original field is projected by name, including nonempty rows in
    # tables that acquired columns during the approved2/3/4 migrations.
    tables = [r[0] for r in before.execute("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'")]
    for table in tables:
        if table == 'schema_migrations':
            continue
        columns = [r[1] for r in before.execute('PRAGMA table_info(' + v1._name(table) + ')')]
        current_columns = {r[1] for r in after.execute('PRAGMA table_info(' + v1._name(table) + ')')}
        if not set(columns).issubset(current_columns):
            raise ValueError('migration removed an original column: ' + table)
        query = 'SELECT ' + ','.join(v1._name(c) for c in columns) + ' FROM ' + v1._name(table)
        original = before.execute(query).fetchall()
        expected = []
        for values in original:
            row = dict(zip(columns, values))
            changes = {}
            if table == 'profile_meta':
                if type(now_ms) is not int or not isinstance(server_id, str) or not server_id:
                    raise ValueError('actual writer recovery arguments missing')
                changes = dict(runtime_generation=row['runtime_generation'] + 1, server_instance_id=server_id,
                               runtime_state='active', updated_at_ms=now_ms)
            elif table == 'request_tasks' and row['state'] in ('queued', 'running'):
                changes = dict(state='invalidated', invalidation_code='application_restarted', finished_at_ms=now_ms)
            elif table == 'request_attempts' and row['state'] == 'started':
                changes = dict(state='cancelled', error_code='application_restarted', finished_at_ms=now_ms)
            elif table == 'sessions' and row['state'] in ('opening', 'active'):
                changes = dict(state='crashed', ended_at_ms=now_ms)
            elif table == 'capture_operations' and row['state'] in ('opening', 'active'):
                changes = dict(state='failed' if row['session_id'] is None else 'recovered', ended_at_ms=now_ms)
            elif table == 'reading_units' and row['state'] == 'open':
                changes = dict(state='finalized')
            if any(value is None for value in changes.values()):
                raise ValueError('actual writer recovery time missing')
            row.update(changes)
            expected.append(tuple(row[column] for column in columns))
        if sorted(expected, key=repr) != sorted(after.execute(query).fetchall(), key=repr):
            raise ValueError('migration changed old immutable rows: ' + table)
    for row in _rows(before, 'schema_migrations'):
        if row not in _rows(after, 'schema_migrations'):
            raise ValueError('migration rewrote old ledger row')


def _check_ledger(db, version):
    rows = db.execute('SELECT version,previous_version,migration_sha256 FROM schema_migrations ORDER BY version').fetchall()
    expected = [
        (1, 0, PROFILE_LEDGER_SHA256),
        (2, 1, PRESERVED['crates/app-infra/src/storage/migrations/002_session_records.sql']),
        (3, 2, PRESERVED['crates/app-infra/src/storage/migrations/003_http_service_identity.sql']),
        (4, 3, PRESERVED['crates/app-infra/src/storage/migrations/004_service_graph.sql']),
        (5, 4, DESIGN_SHA256),
    ][:version]
    if rows != expected:
        raise ValueError('actual ledger does not contain the exact unchanged1..4 and reviewed5 chain')


def _snapshot(root, base, ref, version):
    path = _artifact(base, ref)
    if version == 5:
        errors = compare_current_schema(root, path)
    else:
        contracts = {1: v1.CONTRACT, 2: v4.v3.v2.CONTRACT, 3: v4.v3.CONTRACT, 4: v4.CONTRACT}
        errors = v1.compare_schema_file(root, path, 'profile', contracts[version])
    if errors:
        raise ValueError('actual snapshot schema mismatch: ' + '; '.join(errors[:3]))
    db = _readonly(path)
    try:
        _check_ledger(db, version)
    except Exception:
        db.close()
        raise
    return db


def _owned_path(root, value):
    root = Path(root).resolve()
    path = Path(value)
    if not path.is_absolute():
        path = root / path
    if not path.resolve().is_relative_to(root) or path.is_symlink() or any(p.is_symlink() for p in path.parents):
        raise ValueError('owned artifact escapes or contains symbolic links')
    return path


def _file_hash(path):
    with Path(path).open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def _verify_old_writer(root, base, ref):
    manifest_path = _artifact(root, ref)
    if digest(manifest_path.read_bytes()) != OLD4_MANIFEST_SHA256:
        raise ValueError('old4 prerequisite differs from the exact pre-product frozen artifact')
    manifest = json.loads(manifest_path.read_text())
    if manifest.get('kind') != 'actual_old_sqlite_store_reader_v4' or type(manifest.get('schema_version')) is not int or manifest['schema_version'] != 4:
        raise ValueError('actual frozen Store4 manifest required')
    source = _owned_path(root, manifest['source_root'])
    source_manifest_path = _owned_path(root, manifest['source_manifest_path'])
    if _file_hash(source_manifest_path) != OLD4_SOURCE_MANIFEST_SHA256 or manifest['source_manifest_sha256'] != OLD4_SOURCE_MANIFEST_SHA256:
        raise ValueError('old writer full frozen source manifest changed')
    frozen = json.loads(source_manifest_path.read_text())
    files = frozen.get('files')
    if (frozen.get('kind') != 'pre-product-write-actual-source-freeze/v1' or frozen.get('profile_schema_version') != 4
            or frozen.get('source_entries') != 2797 or not isinstance(files, dict) or len(files) != 2797
            or frozen.get('source_tree_sha256') != OLD4_SOURCE_TREE_SHA256
            or digest(json.dumps(files, sort_keys=True).encode()) != OLD4_SOURCE_TREE_SHA256
            or _owned_path(root, frozen['source_root']) != source):
        raise ValueError('old writer exact efcee0/2797-source closure differs')
    for name, expected in files.items():
        if not HEX64.fullmatch(str(expected)) or _file_hash(_owned_path(source, name)) != expected:
            raise ValueError('old writer full frozen source changed: ' + name)
    compiled_sources = manifest.get('source_files')
    if not isinstance(compiled_sources, dict) or not compiled_sources:
        raise ValueError('old writer full source closure missing')
    for name, expected in compiled_sources.items():
        if not HEX64.fullmatch(str(expected)) or _file_hash(_owned_path(source, name)) != expected:
            raise ValueError('old writer source identity mismatch')
        if name in files and files[name] != expected:
            raise ValueError('old compilation source differs from frozen source closure')
    if 'crates/app-infra/src/storage/database.rs' not in compiled_sources or 'Cargo.lock' not in compiled_sources or compiled_sources['Cargo.lock'] != manifest['original_cargo_lock_sha256']:
        raise ValueError('old writer production host/lock closure incomplete')
    binary = _owned_path(root, manifest['binary_path'])
    if _file_hash(binary) != manifest['binary_sha256']:
        raise ValueError('actual frozen Store4 binary changed')
    compilation, artifact = manifest['compilation'], manifest['compiler_artifact']
    command = ['cargo', 'build', '--locked', '--offline', '--manifest-path', str(source.relative_to(Path(root).resolve()) / 'Cargo.toml'), '-p', 'app-infra', '--example', 'frozen_store4_probe', '--message-format=json']
    if (compilation.get('command') != command or compilation.get('cwd') != str(Path(root).resolve())
            or compilation.get('exit_code') != 0 or not isinstance(compilation.get('environment'), dict)
            or artifact.get('reason') != 'compiler-artifact' or artifact.get('target', {}).get('name') != 'frozen_store4_probe'
            or artifact.get('target', {}).get('kind') != ['example']
            or _owned_path(root, artifact['target']['src_path']) != source / 'crates/app-infra/examples/frozen_store4_probe.rs'):
        raise ValueError('old writer actual locked Cargo command/artifact identity differs')
    stdout = _owned_path(root, compilation['stdout'])
    _owned_path(root, compilation['stderr']).read_bytes()
    outputs = [json.loads(line) for line in stdout.read_text().splitlines() if line.strip()]
    if outputs.count(artifact) != 1 or not any(row.get('reason') == 'build-finished' and row.get('success') is True for row in outputs):
        raise ValueError('old writer actual Cargo log does not contain its exact successful artifact')
    if _file_hash(_owned_path(root, artifact['executable'])) != manifest['binary_sha256']:
        raise ValueError('old writer copied binary differs from Cargo executable')
    return binary


def _old_writer(root, base, ref, candidate):
    binary = _verify_old_writer(root, base, ref)
    with tempfile.TemporaryDirectory(prefix='schema5-old4-replay-') as directory:
        copied = Path(directory) / 'candidate.sqlite3'
        shutil.copyfile(candidate, copied)
        original = copied.read_bytes()
        db = _readonly(copied)
        try:
            profile = db.execute('SELECT profile_id FROM profile_meta WHERE singleton=1').fetchone()[0]
        finally:
            db.close()
        process = subprocess.run([str(binary), str(copied), profile, 'reject'], capture_output=True, timeout=15, check=False)
        result = _loads(process.stdout)
        if process.returncode != 0 or result.get('error_code') != 'unsupported_schema' or copied.read_bytes() != original:
            raise ValueError('actual old4 replay failed to reject5 without modifying the original')


def _execution_logs(root, execution):
    if execution.get('cwd') not in ('.', str(Path(root).resolve())) or type(execution.get('exit_code')) is not int:
        raise ValueError('actual execution working directory/exit missing')
    started, finished = _milliseconds(execution['started_at']), _milliseconds(execution['finished_at'])
    if started > finished:
        raise ValueError('actual execution interval reversed')
    values = []
    for key in ('stdout', 'stderr'):
        ref = execution[key]
        path = _artifact(root, ref) if isinstance(ref, dict) else _owned_path(root, ref)
        values.append(path.read_text())
    return started, finished, values[0], values[1]


def _actual_test_summary(stdout, names=None):
    summaries = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', stdout)
    if len(summaries) != 1 or summaries[0][0] != 'ok' or int(summaries[0][1]) <= 0 or summaries[0][2:] != ('0', '0'):
        raise ValueError('actual unskipped Rust test result missing or failed')
    if names is not None and (int(summaries[0][1]) != len(names) or any(not re.search(r'\btest ' + re.escape(n) + r' \.\.\.', stdout) for n in names)):
        raise ValueError('actual Rust test stdout does not contain the exact requested names')
    return int(summaries[0][1])


def _execution_events(stdout, test_name):
    marker = 'LUNA_STORAGE_V5_EVENT '
    prefixes = (marker, 'test ' + test_name + ' ... ' + marker)
    events = []
    for line in stdout.splitlines():
        prefix = next((value for value in prefixes if line.startswith(value)), None)
        if prefix is not None:
            events.append(_loads(line[len(prefix):]))
        elif marker in line:
            raise ValueError('route event has an unregistered log/harness prefix')
    return events


def _production_executions(root, context, environment, binary):
    if context.get('test_names') != list(TEST_NAMES) or context.get('producer_source') != PRODUCTION_SOURCE:
        raise ValueError('independent runner target/source/test set differs')
    if _artifact(root, context.get('binary')) != binary:
        raise ValueError('independent copied binary reference differs')
    executions = context.get('executions')
    if not isinstance(executions, list) or len(executions) != len(TEST_NAMES):
        raise ValueError('all five original execution references required')
    events, seen = [], set()
    for ref in executions:
        execution = _loads(_artifact(root, ref).read_bytes())
        command = execution['command']
        if not isinstance(command, list) or len(command) != 4 or _owned_path(root, command[0]) != binary or command[1] != '--exact' or command[2] not in TEST_NAMES or command[3] != '--nocapture' or command[2] in seen:
            raise ValueError('actual test command is filtered/duplicated/wrong target')
        seen.add(command[2])
        started, finished, stdout, _ = _execution_logs(root, execution)
        if execution['exit_code'] != 0 or execution.get('fixture_environment') != environment or not context['started_at_ms'] <= started <= finished <= context['finished_at_ms']:
            raise ValueError('actual same-run execution interval/environment/exit differs')
        _actual_test_summary(stdout, [command[2]])
        events.extend(_execution_events(stdout, command[2]))
    return events


def _route_events(base, ref, actual_events, context, routes):
    path = _artifact(base, ref)
    events = [_loads(line) for line in path.read_text().splitlines() if line.strip()]
    if not events or events != actual_events:
        raise ValueError('route NDJSON is absent or differs from actual executed Rust stdout')
    for sequence, event in enumerate(events, 1):
        fixed = {'sequence', 'nonce', 'source_tree_sha256', 'artifact_sha256', 'route', 'event', 'at_ms', 'facts'}
        if (not isinstance(event, dict) or set(event) != fixed or type(event['sequence']) is not int or event['sequence'] != sequence
                or event['nonce'] != context['nonce'] or event['source_tree_sha256'] != context['source_tree_sha256']
                or event['artifact_sha256'] != context['artifact_sha256'] or type(event['at_ms']) is not int
                or not context['started_at_ms'] <= event['at_ms'] <= context['finished_at_ms']
                or event['route'] not in {row['id'] for row in routes} or not isinstance(event['facts'], dict)):
            raise ValueError('route events are stale, malformed or from another execution')
    recovery = {}
    for route in routes:
        identifier = route['id']
        trace = [e for e in events if e['route'] == identifier]
        kinds = [e['event'] for e in trace]
        if not kinds or kinds[0] != 'writer_open_started' or trace[0]['facts'].get('repository') != 'SqliteStore::open':
            raise ValueError('route never called the actual production writer: ' + identifier)
        arguments = trace[0]['facts']
        if type(arguments.get('now_ms')) is not int or arguments['now_ms'] < 0 or not isinstance(arguments.get('server_id'), str) or not arguments['server_id']:
            raise ValueError('actual writer arguments missing: ' + identifier)
        recovery[identifier] = {'now_ms': arguments['now_ms'], 'server_id': arguments['server_id']}
        expected = {'rollback': ('storage_failure', 'transaction_check', 19),
                    'readonly': ('storage_failure', 'readonly', 8),
                    'busy': ('storage_busy', 'exclusive_lock', 5),
                    'enospc': ('storage_failure', 'filesystem_full', 13)}
        if identifier in expected:
            code, fault, sqlite_code = expected[identifier]
            armed = next((e for e in trace if e['event'] == 'fault_armed'), None)
            failed = next((e for e in trace if e['event'] == 'writer_open_failed'), None)
            if (route['error_code'] != code or armed is None or failed is None
                    or armed['facts'].get('fault') != fault or failed['facts'].get('error_code') != code
                    or failed['facts'].get('sqlite_code') != sqlite_code or kinds.index('fault_armed') >= kinds.index('writer_open_failed')):
                raise ValueError('actual fixed writer fault/error/SQLite code missing: ' + identifier)
            if identifier == 'rollback' and not {'migration_transaction_started', 'migration_ddl_applied', 'transaction_rolled_back'}.issubset(kinds):
                raise ValueError('actual005 rollback transaction boundaries missing')
            if identifier == 'busy' and (type(armed['facts'].get('holder_pid')) is not int or armed['facts']['holder_pid'] <= 0 or armed['facts'].get('lock') != 'BEGIN IMMEDIATE'):
                raise ValueError('actual held SQLite writer lock missing')
            if identifier == 'readonly' and armed['facts'].get('read_only') is not True:
                raise ValueError('actual read-only fault boundary missing')
            if identifier == 'enospc' and (type(armed['facts'].get('available_bytes')) is not int or armed['facts']['available_bytes'] < 0 or not isinstance(armed['facts'].get('volume'), str) or not armed['facts']['volume']):
                raise ValueError('actual bounded filesystem fault evidence missing')
        elif identifier == 'kill':
            selected = [next((e for e in trace if e['event'] == name), None) for name in ('child_started', 'child_boundary_observed', 'kill_sent', 'child_waited')]
            if any(e is None for e in selected):
                raise ValueError('actual child/boundary/kill/wait events missing')
            child, boundary, kill, waited = selected
            pid = child['facts'].get('pid')
            if (type(pid) is not int or pid <= 0 or any(e['facts'].get('pid') != pid for e in selected)
                    or kill['facts'].get('signal') != 9 or waited['facts'].get('exit_signal') != 9
                    or route['error_code'] != 'process_killed'
                    or [e['sequence'] for e in selected] != sorted(e['sequence'] for e in selected)
                    or boundary['facts'].get('boundary') not in ('migration_transaction', 'migration_committed')):
                raise ValueError('actual SIGKILL/child exit authority differs')
        elif identifier == 'old4-rejects5':
            if route['error_code'] != 'unsupported_schema' or 'old_writer_rejected' not in kinds:
                raise ValueError('actual old4 rejection event/error missing')
        else:
            if route['error_code'] is not None or kinds[-1] != 'writer_open_committed' or trace[-1]['facts'].get('schema_version') != 5:
                raise ValueError('actual current writer did not complete the route: ' + identifier)
            if identifier == 'restore':
                restored = next((e for e in trace if e['event'] == 'backup_restored'), None)
                if restored is None or restored['facts'].get('backup_snapshot') != route['backup'] or kinds.index('backup_restored') >= kinds.index('writer_open_committed'):
                    raise ValueError('actual backup restoration action missing')
    return recovery


def _verify_old_suite(root, base, ref):
    """Verify the frozen execution, not the later contents of Cargo's cache.

    The pinned reports bind the immutable copied executables to the actual
    historical Cargo artifact records. Cargo's original target path is location
    metadata, not a retained artifact: subsequent builds may replace it.
    """
    if not isinstance(ref, dict) or ref != OLD_SUITE_REFS:
        raise ValueError('both actual frozen Store4 original suite reports required')
    schema = _loads(_artifact(root, ref['schema']).read_bytes())
    dependent = _loads(_artifact(root, ref['dependent']).read_bytes())
    lists = _loads(_artifact(root, ref['lists']).read_bytes())
    if schema.get('kind') != 'unmodified-frozen-Store4-original-schema-test-execution/v1' or dependent.get('kind') != 'unmodified-frozen-Store4-original-dependent-test-execution/v1':
        raise ValueError('unmodified original Store4 suite report identity differs')
    frozen_path = Path(root) / '.build/binding-journey-alignment-20261109/frozen-store4/source-manifest.json'
    if _file_hash(frozen_path) != OLD4_SOURCE_MANIFEST_SHA256:
        raise ValueError('old suite full original source closure changed')
    frozen = _loads(frozen_path.read_bytes())
    source_root = _owned_path(root, frozen['source_root'])
    # Locked compilation depends on these original inputs as well as each test.
    # The original full source manifest is already a code-owned preservation pin.
    for path in ('Cargo.lock', 'Cargo.toml', 'crates/app-infra/Cargo.toml'):
        if _file_hash(_owned_path(source_root, path)) != frozen['files'][path]:
            raise ValueError('old suite original Cargo source changed: ' + path)
    rows = list(dependent.get('results', []))
    rows.append({'target': 'storage_schema_contract', 'command': schema['test_execution']['command'],
                 'source_sha256': schema['test_source_sha256'], 'binary_sha256': schema['binary_sha256'], **schema['test_execution']})
    by_target = {r['target']: r for r in rows}
    if len(by_target) != len(rows) or set(by_target) != set(OLD_TEST_TARGETS):
        raise ValueError('old suite omitted/duplicated original field/route/fault targets')
    if lists.get('kind') != 'actual-frozen-Store4-original-test-inventories/v1' or not isinstance(lists.get('records'), list):
        raise ValueError('actual original old suite binary inventories missing')
    listed = {row['target']: row for row in lists['records']}
    if len(listed) != len(lists['records']) or set(listed) != set(OLD_TEST_TARGETS):
        raise ValueError('old suite exact nine-target inventory differs')
    compilations = [schema['compilation'], _loads(_artifact(root, dependent['reused_actual_compile_execution']).read_bytes())]
    artifacts = {}
    seen_logs = set()
    def historical_logs(execution):
        for key in ('stdout', 'stderr'):
            path = execution.get(key)
            if not isinstance(path, str) or path not in OLD_SUITE_LOG_SHA256:
                raise ValueError('old suite historical raw log identity is not registered')
            if _file_hash(_owned_path(root, path)) != OLD_SUITE_LOG_SHA256[path]:
                raise ValueError('old suite historical raw log changed: ' + path)
            seen_logs.add(path)
        return _execution_logs(root, execution)
    for index, compilation in enumerate(compilations):
        command = compilation['command']
        prefix = ['cargo', 'test', '--locked', '--offline', '--manifest-path', '.build/binding-journey-alignment-20261109/frozen-store4/source-root/Cargo.toml', '-p', 'app-infra']
        targets = OLD_TEST_TARGETS[:1] if index == 0 else OLD_TEST_TARGETS[1:]
        suffix = (['--test', targets[0], '--no-run', '--message-format=json'] if index == 0
                  else ['--no-run', '--message-format=json'] + [value for target in targets for value in ('--test', target)])
        if (command != prefix + suffix or compilation['exit_code'] != 0
                or compilation.get('environment', {}).get('CARGO_TARGET_DIR') != str(Path(root).resolve() / 'target')):
            raise ValueError('old suite actual locked Cargo compilation differs')
        _, compile_finished, stdout, _ = historical_logs(compilation)
        messages = [_loads(line) for line in stdout.splitlines() if line.strip()]
        if not any(m.get('reason') == 'build-finished' and m.get('success') is True for m in messages):
            raise ValueError('old suite compilation has no actual successful build')
        for message in messages:
            if message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('kind') == ['test']:
                name = message['target']['name']
                if name not in targets or name in artifacts:
                    raise ValueError('old suite duplicated or unexpected Cargo test artifact')
                artifacts[name] = (message, compile_finished)
    if set(artifacts) != set(OLD_TEST_TARGETS):
        raise ValueError('old suite actual Cargo test artifact set differs')
    total_tests = 0
    for target in OLD_TEST_TARGETS:
        row = by_target[target]
        artifact, compile_finished = artifacts[target]
        source_path = 'crates/app-infra/tests/' + target + '.rs'
        if (row['source_sha256'] != frozen['files'][source_path]
                or _file_hash(_owned_path(source_root, source_path)) != row['source_sha256']):
            raise ValueError('old suite source/binary compilation closure differs: ' + target)
        command = row['command']
        if not isinstance(command, list) or len(command) != 2 or command[1] != '--nocapture' or row['exit_code'] != 0:
            raise ValueError('old suite must run the original unfiltered complete target')
        copy_directory = ('original-schema-suite' if target == 'storage_schema_contract'
                          else 'original-dependent-suites-corrected-v3')
        copy_name = ('actual-frozen-storage-schema-contract' if target == 'storage_schema_contract'
                     else 'actual-frozen-' + target)
        copied = _owned_path(root, command[0])
        expected_copy = Path(root).resolve() / '.build/binding-journey-alignment-20261109/frozen-store4' / copy_directory / copy_name
        if copied != expected_copy or _file_hash(copied) != row['binary_sha256']:
            raise ValueError('old suite immutable copied executable changed')
        # Validate the historical artifact path lexically without opening it or
        # resolving a present-day target/cache entry. Only the retained copy is
        # executable evidence; no copy is ever written back into Cargo's cache.
        executable = Path(artifact['executable'])
        expected_directory = Path(root).resolve() / 'target/debug/deps'
        if (not executable.is_absolute() or executable.parent != expected_directory
                or not re.fullmatch(re.escape(target) + r'-[0-9a-f]{16}', executable.name)
                or artifact.get('filenames') != [str(executable)]):
            raise ValueError('old suite historical Cargo artifact location differs')
        original_source = source_root / source_path
        expected_manifest = source_root / 'crates/app-infra/Cargo.toml'
        expected_package = 'path+' + (source_root / 'crates/app-infra').as_uri() + '#0.2.0'
        if (artifact.get('package_id') != expected_package
                or _owned_path(root, artifact['manifest_path']) != expected_manifest
                or artifact['target'].get('name') != target
                or artifact['target'].get('crate_types') != ['bin']
                or artifact['target'].get('test') is not True
                or _owned_path(root, artifact['target']['src_path']) != original_source
                or artifact.get('profile', {}).get('test') is not True):
            raise ValueError('old suite Cargo artifact was compiled from another test source')
        inventory = listed[target]
        if inventory.get('command') != [command[0], '--list', '--format', 'terse'] or inventory.get('exit_code') != 0 or inventory.get('binary_sha256') != row['binary_sha256']:
            raise ValueError('old suite actual test-list invocation differs')
        inventory_started, _, inventory_stdout, _ = historical_logs(inventory)
        names = re.findall(r'^(.+): test$', inventory_stdout, re.M)
        if not names or names != inventory.get('test_names') or len(set(names)) != len(names):
            raise ValueError('old suite actual complete test-name list differs')
        test_started, _, stdout, _ = historical_logs(row)
        if test_started < compile_finished or inventory_started < compile_finished:
            raise ValueError('old suite test/list predates its actual compilation')
        total_tests += _actual_test_summary(stdout, names)
    if total_tests != 126:
        raise ValueError('old suite original complete 126-test inventory differs')
    if seen_logs != set(OLD_SUITE_LOG_SHA256):
        raise ValueError('old suite historical raw log set is incomplete')


def _positive_int(value, label):
    if type(value) is not int or value <= 0:
        raise ValueError('actual positive integer required: ' + label)
    return value


def _raw_frames(stdout, marker, exact_test):
    """Only standalone or the exact Rust harness prefix may frame raw facts."""
    result = []
    prefixes = (marker, 'test ' + exact_test + ' ... ' + marker)
    for line in stdout.splitlines():
        prefix = next((p for p in prefixes if line.startswith(p)), None)
        if prefix:
            result.append(_loads(line[len(prefix):]))
        elif marker in line:
            raise ValueError('unregistered raw child frame prefix')
    return result


def _timeline_policy(root):
    policy = read_json(root, TIMELINE_POLICY)['resume_epoch']['observation_contracts']['storage_parent_child']
    if policy.get('kind') != CHILD_KIND:
        raise ValueError('approved child observation contract missing')
    timeline = policy['production_timeline']
    if len(timeline['events']) != 19 or len(timeline['routes']) != 8:
        raise ValueError('approved production boundary/route closure differs')
    return timeline


def _production_partial_order(events):
    """Every reached callback obeys causality, including conditional events."""
    dependencies = {'backup_ready': ('database_writer_entry',), 'before_write_barrier': ('backup_ready',),
                    'writer_resumed': ('before_write_barrier', 'fault_applied'),
                    'transaction_attempted': ('database_writer_entry',), 'transaction_started': ('transaction_attempted',),
                    'ddl_applied': ('transaction_started',), 'commit_succeeded': ('transaction_started',),
                    'transaction_scope_exited_after_drop': ('transaction_started',), 'native_failure': ('database_writer_entry',),
                    'writer_error_published': ('database_writer_entry',), 'api_error_return_ready': ('writer_error_published',),
                    'api_success_return_ready': ('database_writer_entry',)}
    for kind, prerequisites in dependencies.items():
        if kind in events and any(key not in events for key in prerequisites):
            raise ValueError('reached production boundary omitted its actual predecessor: ' + kind)
    if 'database_writer_entry' in events and not {'api_entry', 'actual_api_entry_or_argument_rejection'} & set(events):
        raise ValueError('actual writer entry has no actual API invocation')
    def before(left, right):
        if left in events and right in events and events[left]['event_sequence'] >= events[right]['event_sequence']:
            raise ValueError('actual production causality differs: ' + left + ' -> ' + right)
    prefix = [('scope_created',), ('api_entry', 'actual_api_entry_or_argument_rejection'),
              ('database_writer_entry',), ('backup_ready',), ('before_write_barrier',),
              ('transaction_attempted',), ('transaction_started',), ('ddl_applied',), ('commit_succeeded',)]
    for index, group in enumerate(prefix):
        for later in prefix[index + 1:]:
            for left in group:
                for right in later:
                    before(left, right)
    returns = ('api_error_return_ready', 'api_success_return_ready', 'actual_api_return_error',
               'caller_received_error', 'caller_received_success')
    invariants = ('post_error_invariants', 'independent_current5_db_ledger_invariants',
                  'independent_original_bytes_and_backup_policy_invariant', 'independent_post_kill_db_ledger_backup_invariants')
    for left in (kind for group in prefix for kind in group):
        for right in ('native_failure', 'actual_failure_phase', 'writer_error_published') + returns + invariants:
            before(left, right)
    for left, right in (('before_write_barrier', 'fault_applied'), ('fault_applied', 'writer_resumed'),
                        ('writer_resumed', 'transaction_attempted'), ('writer_resumed', 'native_failure'),
                        ('fault_applied', 'native_failure'), ('native_failure', 'actual_failure_phase'),
                        ('native_failure', 'transaction_scope_exited_after_drop'),
                        ('transaction_started', 'transaction_scope_exited_after_drop'),
                        ('transaction_scope_exited_after_drop', 'writer_error_published'),
                        ('actual_failure_phase', 'writer_error_published'),
                        ('api_error_return_ready', 'caller_received_error'),
                        ('api_success_return_ready', 'caller_received_success')):
        before(left, right)
    for left in ('native_failure', 'actual_failure_phase', 'transaction_scope_exited_after_drop', 'writer_error_published'):
        for right in returns + invariants:
            before(left, right)
    for left in returns:
        for right in invariants:
            before(left, right)
    for left in invariants:
        before(left, 'cleanup_complete')
        before(left, 'recovery_complete')
        before(left, 'actual_recovery')
    before('cleanup_complete', 'recovery_complete')
    for kind in (kind for group in prefix for kind in group):
        before('actual_writer_started', kind)
        before(kind, 'actual_kill_command')
    for left, right in (('actual_writer_started', 'actual_kill_command'), ('actual_kill_command', 'actual_wait_status'),
                        ('actual_wait_status', 'independent_post_kill_db_ledger_backup_invariants')):
        before(left, right)
    if 'database_writer_entry' in events:
        writer = events['database_writer_entry']['actual_thread_id']
        for kind in ('api_entry', 'actual_api_entry_or_argument_rejection'):
            if kind in events and events[kind]['actual_thread_id'] == writer:
                raise ValueError('actual API and writer thread identity collapsed')
        for kind in ('backup_ready', 'before_write_barrier', 'writer_resumed', 'transaction_attempted', 'transaction_started',
                     'ddl_applied', 'commit_succeeded', 'native_failure', 'actual_failure_phase',
                     'transaction_scope_exited_after_drop', 'writer_error_published'):
            if kind in events and events[kind]['actual_thread_id'] != writer:
                raise ValueError('actual reached writer callback emitted by another thread: ' + kind)
        for kind in returns + invariants:
            if kind in events and events[kind]['actual_thread_id'] == writer:
                raise ValueError('post-writer return/readback emitted by actual writer thread: ' + kind)


def ex_storage_timeline_consumer(root, timeline, binding, interval, *, route_class=None):
    """Consume actual boundary facts; a guard fixture never grants product PASS.

    The plan supplies route applicability, not a producer's claimed PASS list.
    Null boundaries require the independently observed failure phase. Scope,
    business now_ms and relay timestamps are never interpreted as API clocks.
    """
    policy = _timeline_policy(root)
    route_class = route_class or binding['route']
    if route_class not in policy['routes']:
        raise ValueError('unregistered production timeline route')
    rule = policy['routes'][route_class]
    if set(timeline) != {'kind', 'events', 'not_reached'} or timeline['kind'] != 'private-schema5-production-timeline/v1':
        raise ValueError('fixed actual production timeline required')
    events, absent = timeline['events'], timeline['not_reached']
    if not isinstance(events, list) or not events or not isinstance(absent, dict):
        raise ValueError('actual production events/applicability missing')
    required_fields = set(policy['event_envelope']['required'])
    allowed = set(policy['events']) | set(rule['required_events'])
    by_kind, last_wall, last_mono = {}, interval[0], 0
    fixed_identity = required_fields - {'event_kind', 'event_sequence', 'monotonic_ns', 'observed_wall_ms', 'actual_phase', 'payload', 'actual_thread_id'}
    for sequence, event in enumerate(events, 1):
        if not isinstance(event, dict) or set(event) != required_fields:
            raise ValueError('actual callback envelope differs from approved fields')
        kind = event['event_kind']
        if kind not in allowed or kind in by_kind or event['event_sequence'] != sequence or type(event['event_sequence']) is not int:
            raise ValueError('actual callback duplicate/gap/unknown boundary')
        if canonical_hash({k: event[k] for k in fixed_identity}) != canonical_hash({k: binding[k] for k in fixed_identity}):
            raise ValueError('actual callback identity/source/path/PID differs')
        if not isinstance(event['actual_thread_id'], str) or not event['actual_thread_id'] or not isinstance(event['actual_phase'], str) or not event['actual_phase'] or not isinstance(event['payload'], dict):
            raise ValueError('actual callback phase/thread/payload missing')
        if kind in policy['events'] and event['payload'].get('production_symbol') != policy['events'][kind]['production_symbol']:
            raise ValueError('actual callback has no registered production boundary provenance')
        wall = _positive_int(event['observed_wall_ms'], kind + ' wall time')
        mono = _positive_int(event['monotonic_ns'], kind + ' monotonic clock')
        if not interval[0] <= last_wall <= wall <= interval[1] or mono < last_mono:
            raise ValueError('actual callback time reversed/outside actual execution')
        last_wall, last_mono = wall, mono
        by_kind[kind] = event
    if set(rule['required_events']) - set(by_kind):
        raise ValueError('applicable production boundary missing')
    ordered = rule.get('strict_causal_order', rule['required_events'])
    order = [by_kind[k]['event_sequence'] for k in ordered if k in by_kind]
    if order != sorted(order):
        raise ValueError('actual production boundary causal order differs')
    forbidden = set(rule.get('not_applicable', {}))
    if route_class == 'external_kill':
        forbidden -= {'native_failure', 'transaction_scope_exited_after_drop'}
    if route_class in CHILD_TESTS:
        forbidden.add('commit_succeeded')
        if route_class != 'rollback':
            forbidden.add('recovery_complete')
    if forbidden & set(by_kind):
        raise ValueError('unreached production boundary falsely claimed')
    if route_class not in CHILD_TESTS:
        # The other four route classes consume real journals too. They do not
        # inherit fault-only Scope/rollback requirements or fabricated returns.
        conditional = set(rule.get('conditional_events', {}))
        if set(absent) - (set(rule.get('not_applicable', {})) | conditional):
            raise ValueError('free not-applicable declarations cannot close production routes')
        for kind, value in absent.items():
            if kind in by_kind or not isinstance(value, dict) or set(value) != {'event', 'reason', 'failure_phase'} or value['event'] is not None or value['reason'] not in ('route_not_applicable', 'not_reached', 'no_old_database', 'no_transaction_started', 'incomplete_backup_failure'):
                raise ValueError('unreached production boundary requires typed actual applicability')
        if route_class == 'successful_upgrade_0_1_2_3_4_to5':
            needs_backup = binding['route'] != 'fresh'
            if ('backup_ready' in by_kind) != needs_backup or not needs_backup and absent.get('backup_ready') != {'event': None, 'reason': 'no_old_database', 'failure_phase': None} or 'ddl_applied' not in by_kind:
                raise ValueError('actual upgrade version/backup/005 applicability differs')
            if needs_backup and by_kind['backup_ready']['event_sequence'] >= by_kind['transaction_attempted']['event_sequence']:
                raise ValueError('actual upgrade BEGIN preceded backup completion')
            if not by_kind['transaction_started']['event_sequence'] < by_kind['ddl_applied']['event_sequence'] < by_kind['commit_succeeded']['event_sequence']:
                raise ValueError('actual upgrade DDL/commit order differs')
        elif route_class == 'reopen5':
            if any(k in by_kind for k in ('fault_applied', 'native_failure', 'ddl_applied', 'backup_ready')):
                raise ValueError('current5 reopen cannot invent migration/fault boundaries')
        elif route_class == 'preflight_rejection':
            failure = by_kind['actual_failure_phase']['payload']
            phase = failure.get('failure_phase')
            phases = {'arguments': 0, 'lease': 1, 'identity': 2, 'schema': 3, 'backup': 4, 'connection_open': 5, 'pragmas': 6, 'begin': 7, 'ddl': 8, 'post_ddl': 9, 'commit': 10}
            if phase not in phases or not isinstance(failure.get('error_code'), str) or not failure['error_code']:
                raise ValueError('actual raw preflight rejection phase/error missing')
            expected_reached = {'database_writer_entry': phase != 'arguments', 'backup_ready': phases[phase] >= 5,
                                'transaction_started': phases[phase] >= 8, 'ddl_applied': phases[phase] >= 9,
                                'native_failure': type(failure.get('native_primary_code')) is int}
            for kind, reached in expected_reached.items():
                if (kind in by_kind) != reached or not reached and kind not in absent:
                    raise ValueError('preflight phase-derived applicability differs: ' + kind)
            if any(value['failure_phase'] != phase for value in absent.values()):
                raise ValueError('preflight null reason differs from actual raw phase')
            if 'native_failure' in by_kind:
                native = by_kind['native_failure']
                code = failure['native_primary_code']
                extended = native['payload'].get('native_extended_code')
                if not 1 <= code <= 255 or native['actual_phase'] != phase or native['payload'].get('failure_phase') != phase or type(native['payload'].get('native_primary_code')) is not int or native['payload']['native_primary_code'] != code or type(extended) is not int or extended & 255 != code or native['payload'].get('error_code') != failure['error_code']:
                    raise ValueError('actual preflight native failure phase/code differs from raw rejection')
        elif route_class == 'external_kill':
            for kind in ('actual_writer_started', 'actual_kill_command', 'actual_wait_status'):
                if by_kind[kind]['payload'].get('pid') != binding['child_pid']:
                    raise ValueError('actual external kill actor PID differs')
            if by_kind['actual_kill_command']['payload'].get('signal') != 9 or by_kind['actual_wait_status']['payload'].get('exit_signal') != 9:
                raise ValueError('actual SIGKILL/wait raw status missing')
            if any(kind in by_kind for kind in ('api_error_return_ready', 'caller_received_error')):
                raise ValueError('killed actor cannot invent API return or normal Drop')
            for kind in ('native_failure', 'transaction_scope_exited_after_drop'):
                if kind in by_kind and by_kind[kind]['event_sequence'] >= by_kind['actual_kill_command']['event_sequence']:
                    raise ValueError('kill cannot cause a normal native failure/Drop callback')
            if 'transaction_scope_exited_after_drop' in by_kind and 'transaction_started' not in by_kind:
                raise ValueError('pre-kill transaction Drop had no actual successful BEGIN')
    # Fault routes have a closed nineteen-boundary applicability dictionary.
    # Other route classes retain their registered success/kill/preflight facts.
    if route_class in CHILD_TESTS:
        if set(by_kind) | set(absent) != set(policy['events']) or set(by_kind) & set(absent):
            raise ValueError('every private production boundary requires actual event or typed null')
        failure = by_kind['native_failure']
        phase = failure['payload'].get('failure_phase')
        phases = {'connection_open': 0, 'pragmas': 1, 'begin': 2, 'ddl': 3, 'post_ddl': 4, 'commit': 5}
        if phase not in phases or failure['actual_phase'] != phase:
            raise ValueError('raw actual SQLite failure phase missing')
        if failure['payload'].get('native_primary_code') != rule['native_primary_code'] or type(failure['payload'].get('native_primary_code')) is not int or failure['payload'].get('error_code') != rule['typed_error']:
            raise ValueError('actual native SQLite/typed error differs')
        extended = failure['payload'].get('native_extended_code')
        if type(extended) is not int or extended & 255 != rule['native_primary_code']:
            raise ValueError('actual extended SQLite error differs')
        conditional = {'transaction_attempted': phases[phase] >= 2,
                       'transaction_started': phases[phase] >= 3,
                       'ddl_applied': phases[phase] >= 4,
                       'transaction_scope_exited_after_drop': phases[phase] >= 3}
        if route_class == 'rollback' and phase != 'post_ddl':
            raise ValueError('rollback fault was not after actual005 DDL')
        for kind, reached in conditional.items():
            if (kind in by_kind) != reached:
                raise ValueError('phase-derived transaction/DDL applicability differs: ' + kind)
        for kind, value in absent.items():
            reason = 'route_not_applicable' if kind in forbidden else 'no_transaction_started' if kind == 'transaction_scope_exited_after_drop' else 'not_reached'
            if kind not in forbidden and kind not in conditional:
                raise ValueError('unsupported missing production boundary: ' + kind)
            if value != {'event': None, 'reason': reason, 'failure_phase': phase}:
                raise ValueError('unreached boundary must have typed null and actual phase')
        def before(left, right):
            if left in by_kind and right in by_kind and by_kind[left]['event_sequence'] >= by_kind[right]['event_sequence']:
                raise ValueError('actual production causality differs: ' + left + ' -> ' + right)
        for left, right in (('database_writer_entry', 'backup_ready'), ('backup_ready', 'transaction_attempted'),
                            ('writer_resumed', 'transaction_attempted'), ('transaction_attempted', 'transaction_started'),
                            ('transaction_started', 'ddl_applied'), ('ddl_applied', 'native_failure'),
                            ('native_failure', 'transaction_scope_exited_after_drop'),
                            ('transaction_scope_exited_after_drop', 'writer_error_published'),
                            ('post_error_invariants', 'cleanup_complete')):
            before(left, right)
        if by_kind['api_entry']['actual_thread_id'] == by_kind['database_writer_entry']['actual_thread_id']:
            raise ValueError('actual API and writer thread identity collapsed')
        writer_thread = by_kind['database_writer_entry']['actual_thread_id']
        for kind in ('backup_ready', 'before_write_barrier', 'writer_resumed', 'transaction_attempted', 'transaction_started', 'ddl_applied', 'native_failure', 'transaction_scope_exited_after_drop', 'writer_error_published'):
            if kind in by_kind and by_kind[kind]['actual_thread_id'] != writer_thread:
                raise ValueError('actual writer callback emitted by another thread')
        if by_kind['api_entry']['payload'].get('arguments') != {k: binding[k] for k in ('original_database_path', 'profile_id', 'server_id')} | {'now_ms': 100}:
            raise ValueError('actual API boundary did not observe exact writer arguments')
    _production_partial_order(by_kind)
    return by_kind


def _inspect_frozen_old_reader(root, envelope, parent, context, route, snapshots):
    """Observe the immutable Store4 executable externally; never invent hooks.

    The old reader has no current cfg(test) journal. Spawn/wait/stdout/readback
    are observations by the actual parent, with a distinct reader process.
    Its source/binary/build closure and independent replay remain mandatory in
    the unchanged outer _verify_old_writer/_old_writer validators.
    """
    fields = {'kind', 'execution', 'binding', 'old_writer_manifest', 'reader_execution', 'observations'}
    if set(envelope) != fields or envelope['kind'] != 'schema5-frozen-old-reader-observation/v1':
        raise ValueError('old4 requires actual parent-observed independent reader evidence')
    binding = envelope['binding']
    keys = {'route', 'case_id', 'invocation_id', 'exact_test', 'nonce', 'source_tree_sha256', 'observer_pid',
            'reader_pid', 'profile_id', 'original_database_path', 'database_file_device_identity'}
    if not isinstance(binding, dict) or set(binding) != keys or binding['route'] != 'old4-rejects5' or binding['exact_test'] != 'bj_11_storage' or parent['command'][2] != 'bj_11_storage' or binding['nonce'] != context['nonce'] or binding['source_tree_sha256'] != context['source_tree_sha256']:
        raise ValueError('old4 observation actual source/parent/route identity differs')
    for key in ('case_id', 'invocation_id', 'profile_id'):
        if not isinstance(binding[key], str) or not binding[key]:
            raise ValueError('actual old4 observation invocation/profile missing')
    observer_pid = _positive_int(binding['observer_pid'], 'actual old4 parent observer PID')
    reader_pid = _positive_int(binding['reader_pid'], 'actual independent old4 reader PID')
    if observer_pid != parent['pid'] or reader_pid == observer_pid:
        raise ValueError('frozen old4 reader must be a distinct actual child process')
    output = Path(parent['fixture_environment']['LUNA_BINDING_OUTPUT']).resolve()
    original = _owned_path(root, binding['original_database_path'])
    if not original.is_relative_to(output):
        raise ValueError('actual old4 original DB escaped owned run output')
    identity = binding['database_file_device_identity']
    if not isinstance(identity, dict) or set(identity) != {'device', 'inode'} or any(type(v) is not int or v <= 0 for v in identity.values()):
        raise ValueError('actual old4 original DB file/device identity missing')
    manifest_path = _artifact(root, envelope['old_writer_manifest'])
    if _file_hash(manifest_path) != OLD4_MANIFEST_SHA256:
        raise ValueError('old4 observation must use exact frozen reader manifest')
    manifest = _loads(manifest_path.read_bytes())
    binary = _owned_path(root, manifest['binary_path'])
    binary_ref = {'path': str(binary.relative_to(Path(root).resolve())), 'sha256': _file_hash(binary)}
    if binary_ref['sha256'] != manifest['binary_sha256']:
        raise ValueError('actual old4 reader binary changed')
    actual = _loads(_artifact(root, envelope['reader_execution']).read_bytes())
    argv = [str(binary), str(original), binding['profile_id'], 'reject']
    if actual.get('command') != argv or actual.get('pid') != reader_pid or type(actual.get('pid')) is not int or actual.get('parent_pid') != observer_pid or type(actual.get('parent_pid')) is not int or actual.get('fixture_environment') != parent['fixture_environment'] or actual.get('timed_out') is not False:
        raise ValueError('actual old4 spawn PID/argv/binary/parent environment differs')
    parent_start, parent_end, parent_stdout, _ = _execution_logs(root, parent)
    start, end, stdout, _ = _execution_logs(root, actual)
    if actual['exit_code'] != 0 or not parent_start <= start <= end <= parent_end:
        raise ValueError('actual old4 child failed/timed out/outside real parent interval')
    result = _loads(stdout)
    if not isinstance(result, dict) or set(result) != {'opened', 'schema_version', 'error_code', 'error'} or result['opened'] is not False or type(result['schema_version']) is not int or result['schema_version'] != 4 or result['error_code'] != 'unsupported_schema' or not isinstance(result['error'], str) or not result['error']:
        raise ValueError('actual frozen old4 raw stdout did not reject schema5')
    observations = envelope['observations']
    expected_kinds = ['reader_spawned', 'reader_waited', 'reader_result_captured', 'original_database_verified']
    if not isinstance(observations, list) or len(observations) != 4 or [e.get('kind') for e in observations] != expected_kinds:
        raise ValueError('complete actual old4 spawn/wait/result/readback observations required')
    frames = _raw_frames(parent_stdout, OLD4_OBSERVER_MARKER, parent['command'][2])
    if frames != observations:
        raise ValueError('old4 observations differ from actual parent raw stdout')
    last_wall, last_mono = parent_start, 0
    for sequence, event in enumerate(observations, 1):
        fixed = {'kind', 'sequence', 'monotonic_ns', 'observed_wall_ms', 'observer_pid', 'reader_pid', 'route', 'invocation_id', 'nonce', 'source_tree_sha256', 'payload'}
        if not isinstance(event, dict) or set(event) != fixed or type(event['sequence']) is not int or event['sequence'] != sequence or any(event[k] != binding[k] for k in ('observer_pid', 'reader_pid', 'route', 'invocation_id', 'nonce', 'source_tree_sha256')):
            raise ValueError('actual old4 parent observer identity/sequence differs')
        wall = _positive_int(event['observed_wall_ms'], 'old4 parent observation wall time')
        mono = _positive_int(event['monotonic_ns'], 'old4 parent observation monotonic clock')
        if not last_wall <= wall <= parent_end or mono < last_mono:
            raise ValueError('actual old4 observer clock reversed/outside parent')
        last_wall, last_mono = wall, mono
    spawned, waited, captured, verified = observations
    if not start <= spawned['observed_wall_ms'] <= end <= waited['observed_wall_ms']:
        raise ValueError('actual old4 child lifetime differs from parent spawn/wait observations')
    if spawned['payload'] != {'command': argv, 'binary': binary_ref, 'execution': envelope['reader_execution']} or waited['payload'] != {'exit_code': 0} or captured['payload'] != {'stdout': actual['stdout'], 'stderr': actual['stderr'], 'error_code': 'unsupported_schema'}:
        raise ValueError('actual old4 spawn/wait/raw output payload differs')
    before_ref, after_ref = snapshots[route['before']], snapshots[route['after']]
    if route['backup'] is not None or verified['payload'] != {'before': before_ref, 'after': after_ref, 'original_database_path': str(original), 'before_identity': identity, 'after_identity': identity}:
        raise ValueError('old4 parent readback lacks exact unchanged original lineage')
    before = _snapshot(root, output, before_ref, 5)
    after = _snapshot(root, output, after_ref, 5)
    try:
        _same_database(before, after)
        if before_ref['sha256'] != after_ref['sha256'] or after.execute('SELECT profile_id FROM profile_meta WHERE singleton=1').fetchone() != (binding['profile_id'],):
            raise ValueError('old4 rejection changed original SQLite bytes/profile')
    finally:
        before.close(); after.close()
    return {'observer_pid': observer_pid, 'reader_pid': reader_pid, 'external_observation_only': True}


def _inspect_parent_timelines(root, refs, context, routes, snapshots, route_events):
    expected = {r['id'] for r in routes} - set(CHILD_TESTS)
    if not isinstance(refs, dict) or set(refs) != expected:
        raise ValueError('every preserved non-fault route needs its actual production timeline')
    parent_map = context['parent_executions']
    used_refs, identities = set(), set()
    for route, ref in refs.items():
        path = _artifact(root, ref)
        if str(path) in used_refs:
            raise ValueError('one parent production timeline reused for two routes')
        used_refs.add(str(path))
        envelope = _loads(path.read_bytes())
        if not isinstance(envelope, dict):
            raise ValueError('fixed typed parent production timeline required')
        execution = _loads(_artifact(root, envelope['execution']).read_bytes())
        name = execution['command'][2]
        if execution != parent_map.get(name):
            raise ValueError('parent route timeline has no actual inspected execution')
        if route == 'old4-rejects5':
            row = next(r for r in routes if r['id'] == route)
            _inspect_frozen_old_reader(root, envelope, execution, context, row, snapshots)
            identity = (envelope['binding']['case_id'], envelope['binding']['invocation_id'])
            if identity in identities:
                raise ValueError('old4 observer invocation reused another parent route')
            identities.add(identity)
            continue
        if set(envelope) != {'kind', 'execution', 'binding', 'timeline'} or envelope['kind'] != 'schema5-parent-route-timeline/v1':
            raise ValueError('fixed typed parent production timeline required')
        binding = envelope['binding']
        keys = set(_timeline_policy(root)['event_envelope']['required']) - {'event_kind', 'event_sequence', 'monotonic_ns', 'observed_wall_ms', 'actual_phase', 'payload', 'actual_thread_id'}
        if not isinstance(binding, dict) or set(binding) != keys or binding['route'] != route or binding['exact_test'] != name or binding['nonce'] != context['nonce'] or binding['source_tree_sha256'] != context['source_tree_sha256']:
            raise ValueError('actual parent timeline source/route/arguments identity differs')
        for key in ('case_id', 'invocation_id', 'scope_id', 'profile_id', 'server_id'):
            if not isinstance(binding[key], str) or not binding[key]:
                raise ValueError('actual parent route invocation/arguments missing')
        key = (binding['case_id'], binding['invocation_id'])
        if key in identities:
            raise ValueError('duplicate parent route invocation')
        identities.add(key)
        original_path = _owned_path(root, binding['original_database_path'])
        output = Path(parent_map[name]['fixture_environment']['LUNA_BINDING_OUTPUT']).resolve()
        if not original_path.is_relative_to(output):
            raise ValueError('actual parent original DB escapes owned output')
        identity = binding['database_file_device_identity']
        if not isinstance(identity, dict) or set(identity) != {'device', 'inode'} or any(type(v) is not int or v <= 0 for v in identity.values()):
            raise ValueError('actual parent DB file/device identity missing')
        trace = [e for e in route_events if e['route'] == route]
        if binding['server_id'] != trace[0]['facts']['server_id']:
            raise ValueError('parent actual writer server arguments differ from raw route')
        actor = next((e['facts']['pid'] for e in trace if e['event'] == 'child_started'), None) if route == 'kill' else execution.get('pid')
        if _positive_int(binding['child_pid'], 'parent route actual actor PID') != _positive_int(actor, 'outer actor PID') or binding['child_ppid'] != execution.get('pid') and route == 'kill':
            raise ValueError('actual parent route writer/kill actor differs')
        interval = _execution_logs(root, execution)
        frames = _raw_frames(interval[2], BOUNDARY_MARKER, name)
        if [e for e in frames if e.get('invocation_id') == binding['invocation_id']] != envelope['timeline']['events']:
            raise ValueError('parent production timeline absent from actual raw execution')
        route_class = 'external_kill' if route == 'kill' else 'reopen5' if route == 'reopen5' else 'preflight_rejection' if route == 'old4-rejects5' else 'successful_upgrade_0_1_2_3_4_to5'
        events = ex_storage_timeline_consumer(root, envelope['timeline'], binding, interval[:2], route_class=route_class)
        row = next(r for r in routes if r['id'] == route)
        actual_after = _readonly(_artifact(output, snapshots[row['after']]))
        try:
            if actual_after.execute('SELECT profile_id FROM profile_meta WHERE singleton=1').fetchone() != (binding['profile_id'],):
                raise ValueError('parent timeline profile differs from actual DB')
        finally:
            actual_after.close()
        invariant_kind = 'independent_post_kill_db_ledger_backup_invariants' if route == 'kill' else 'independent_original_bytes_and_backup_policy_invariant' if route == 'old4-rejects5' else 'independent_current5_db_ledger_invariants'
        payload = events[invariant_kind]['payload']
        if payload.get('before') != (snapshots[row['before']] if row['before'] is not None else None) or payload.get('after') != snapshots[row['after']] or payload.get('backup') != (snapshots[row['backup']] if row['backup'] is not None else None):
            raise ValueError('parent production invariant is not exact route DB/backup')
        if route == 'kill':
            restore = next(r for r in routes if r['id'] == 'restore')
            if events['actual_recovery']['payload'].get('snapshot') != snapshots[restore['after']]:
                raise ValueError('actual kill recovery not bound to independent restore route')
    return sorted(expected)


def _same_database(before, after):
    # The shared schema inspector installs sqlite3.Row. Preserve the caller's
    # factory so subsequent exact tuple/profile checks retain their semantics.
    factories = before.row_factory, after.row_factory
    try:
        shape = v1.inspect_connection(before)
        if shape != v1.inspect_connection(after) or any(_rows(before, t) != _rows(after, t) for t in shape['tables']):
            raise ValueError('actual backup/fault changed original full schema/ledger/data')
    finally:
        before.row_factory, after.row_factory = factories


def _child_execution(root, ref, command, environment, parent_interval, launches):
    execution = _loads(_artifact(root, ref).read_bytes())
    if execution.get('command') != command or execution.get('fixture_environment') != environment:
        raise ValueError('actual child argv or same-run environment differs')
    pid = _positive_int(execution.get('pid'), 'child execution PID')
    start, end, stdout, stderr = _execution_logs(root, execution)
    if execution['exit_code'] != 0 or execution.get('timed_out', False) is not False or not parent_interval[0] <= start <= end <= parent_interval[1]:
        raise ValueError('child raw execution failed/outside actual parent')
    expected = {'pid': pid, 'command': command, 'execution': ref}
    if launches.count(expected) != 1:
        raise ValueError('child has no unique actual parent spawn/raw execution binding')
    return execution, (start, end), stdout, stderr


def _fault_os_facts(root, record, events, execution_interval, child_pid):
    route, facts = record['route'], record['os_facts']
    applied = events['fault_applied']['observed_wall_ms']
    failed = events['native_failure']['observed_wall_ms']
    received = events['caller_received_error']['observed_wall_ms']
    invariants = events['post_error_invariants']['observed_wall_ms']
    cleanup = events['cleanup_complete']['observed_wall_ms']
    if route == 'rollback':
        if facts != {}:
            raise ValueError('rollback must not invent OS permission/volume facts')
        return
    if route == 'readonly':
        keys = {'before_mode', 'applied_mode', 'restored_mode', 'applied_at_ms', 'restored_at_ms'}
        if set(facts) != keys or any(type(facts[k]) is not int or facts[k] < 0 for k in keys):
            raise ValueError('actual permission mode readbacks missing')
        if facts['applied_mode'] & 0o222 or not facts['before_mode'] & 0o222 or facts['before_mode'] != facts['restored_mode'] or not facts['applied_at_ms'] <= applied <= failed <= received <= facts['restored_at_ms'] <= invariants <= cleanup:
            raise ValueError('actual readonly permission lifetime/restoration differs')
    elif route == 'busy':
        keys = {'holder_pid', 'connection_id', 'acquired_at_ms', 'released_at_ms', 'begin_sql', 'release_sql', 'begin_success', 'release_success'}
        if set(facts) != keys or facts['holder_pid'] != child_pid or type(facts['holder_pid']) is not int or not isinstance(facts['connection_id'], str) or not facts['connection_id'] or facts['begin_sql'] != 'BEGIN IMMEDIATE' or facts['release_sql'] != 'ROLLBACK' or facts['begin_success'] is not True or facts['release_success'] is not True:
            raise ValueError('actual same-process independent SQLite holder facts missing')
        if not execution_interval[0] <= facts['acquired_at_ms'] <= applied <= failed <= received <= facts['released_at_ms'] <= invariants <= cleanup:
            raise ValueError('actual SQLite failure outside held lock interval')
    else:
        keys = {'commands', 'image', 'mount', 'device', 'mount_device', 'output_device', 'capacity_bytes', 'available_bytes_at_fault', 'observed_os_error', 'os_error_at_ms', 'capacity_sampled_at_ms', 'filler_path', 'filler_bytes', 'filler_removed_at_ms', 'evidence_copied_at_ms'}
        if set(facts) != keys or facts['observed_os_error'] != 28 or type(facts['observed_os_error']) is not int:
            raise ValueError('actual OS ENOSPC/capacity command evidence missing')
        for key in ('mount_device', 'output_device', 'capacity_bytes', 'filler_bytes'):
            _positive_int(facts[key], 'bounded volume ' + key)
        if facts['mount_device'] == facts['output_device'] or not 16 * 1024**2 < facts['capacity_bytes'] <= 64 * 1024**2 or type(facts['available_bytes_at_fault']) is not int or not 0 <= facts['available_bytes_at_fault'] < 65536:
            raise ValueError('owned bounded volume device/capacity differs')
        image, mount, filler = (_owned_path(root, facts[k]) for k in ('image', 'mount', 'filler_path'))
        original = _owned_path(root, record['original_database_path'])
        output = _artifact(root, record['before']).parent
        if facts['mount_device'] != record['database_file_device_identity']['device'] or facts['output_device'] != output.stat().st_dev:
            raise ValueError('actual bounded database/output device identity differs')
        if image.suffix != '.dmg' or not image.is_relative_to(output) or not mount.is_relative_to(output) or not filler.is_relative_to(mount) or not original.is_relative_to(mount) or not re.fullmatch(r'/dev/disk[0-9]+', facts['device']) or facts['filler_bytes'] > facts['capacity_bytes'] + 65536:
            raise ValueError('bounded volume image/mount/filler ownership differs')
        if not events['backup_ready']['observed_wall_ms'] <= facts['os_error_at_ms'] <= facts['capacity_sampled_at_ms'] <= applied <= failed <= received <= facts['filler_removed_at_ms'] <= invariants <= facts['evidence_copied_at_ms'] <= cleanup:
            raise ValueError('actual fill/native error/recovery/copy order differs')
        if not isinstance(facts['commands'], list) or len(facts['commands']) != 3:
            raise ValueError('all actual hdiutil commands required')
        commands = []
        for ref in facts['commands']:
            row = _loads(_artifact(root, ref).read_bytes())
            start, end, raw, _ = _execution_logs(root, row)
            _positive_int(row.get('pid'), 'actual hdiutil PID')
            argv = row.get('command')
            if row['exit_code'] != 0 or not isinstance(argv, list) or not argv or argv[0] not in ('hdiutil', '/usr/bin/hdiutil') or not execution_interval[0] <= start <= end <= execution_interval[1]:
                raise ValueError('actual hdiutil argv/raw/exit/interval differs')
            commands.append((argv[1:], start, end, raw))
        if commands[0][0] != ['create', '-size', '64m', '-fs', 'HFS+', '-volname', 'LunaSchema5Full', '-type', 'UDIF', '-nospotlight', str(image)] or commands[1][0] != ['attach', '-plist', '-nobrowse', '-mountpoint', str(mount), str(image)] or commands[2][0] != ['detach', facts['device']]:
            raise ValueError('bounded volume commands differ from fixed owned route')
        import plistlib
        plist = plistlib.loads(commands[1][3].encode())
        if not any(e.get('mount-point') == str(mount) and (e.get('dev-entry') == facts['device'] or str(e.get('dev-entry', '')).startswith(facts['device'] + 's')) for e in plist.get('system-entities', [])):
            raise ValueError('actual attach plist does not identify owned volume')
        if not commands[0][2] <= commands[1][1] <= commands[1][2] <= events['scope_created']['observed_wall_ms'] or not facts['evidence_copied_at_ms'] <= commands[2][1] <= commands[2][2] <= cleanup:
            raise ValueError('actual attach/detach causal lifetime differs')


def _fault_capture_lineage(root, record, events, interval):
    captures = record['captures']
    if not isinstance(captures, dict) or set(captures) != {'before', 'after', 'backup'}:
        raise ValueError('actual original-to-snapshot capture lineage missing')
    original = _owned_path(root, record['original_database_path'])
    backup = original.with_name(original.name + '.pre-v5-100.sqlite3')
    for kind, capture in captures.items():
        if not isinstance(capture, dict) or set(capture) != {'source_path', 'source_identity', 'snapshot', 'captured_at_ms'}:
            raise ValueError('fixed actual SQLite capture metadata required')
        if _owned_path(root, capture['source_path']) != (backup if kind == 'backup' else original) or capture['snapshot'] != record[kind]:
            raise ValueError('snapshot was captured from another actual DB/backup')
        identity = capture['source_identity']
        if not isinstance(identity, dict) or set(identity) != {'device', 'inode'} or any(type(v) is not int or v <= 0 for v in identity.values()):
            raise ValueError('actual capture source file/device identity missing')
        if kind != 'backup' and identity != record['database_file_device_identity'] or kind == 'backup' and (identity['device'] != record['database_file_device_identity']['device'] or identity['inode'] == record['database_file_device_identity']['inode']):
            raise ValueError('actual snapshot original/independent backup file identity differs')
        clock = _positive_int(capture['captured_at_ms'], 'actual SQLite capture time')
        if kind == 'before':
            bounds = interval[0], events['scope_created']['observed_wall_ms']
        else:
            bounds = (events['caller_received_error']['observed_wall_ms'] if kind == 'after' else events['backup_ready']['observed_wall_ms']), events['post_error_invariants']['observed_wall_ms']
        if not bounds[0] <= clock <= bounds[1]:
            raise ValueError('actual SQLite snapshot capture preceded its real boundary')


def _inspect_child_proof(root, base, ref, context, environment, parent_binary, snapshots, routes, route_events):
    """Strict v2 child execution path; the legacy v1 consumer stays unchanged."""
    proof = _loads(_artifact(root, ref).read_bytes())
    fields = {'kind', 'parent', 'source_files', 'cargo_lock', 'old_writer_manifest', 'compilation', 'binary', 'listing', 'executions', 'fault_records'}
    if not isinstance(proof, dict) or set(proof) != fields or proof['kind'] != CHILD_KIND:
        raise ValueError('fixed typed child execution proof required')
    from binding_phase_guard import tree_sha
    sources = context.get('source_files')
    if not isinstance(sources, dict) or tree_sha(sources) != context['source_tree_sha256'] or _loads(_artifact(root, proof['source_files']).read_bytes()) != sources:
        raise ValueError('actual external full child source closure missing/stale')
    for path, sha in sources.items():
        if not HEX64.fullmatch(str(sha)) or digest(safe_bytes(root, path)) != sha:
            raise ValueError('actual child compilation source changed: ' + path)
    needed = {'Cargo.toml', 'Cargo.lock', 'crates/app-infra/src/lib.rs', 'crates/app-infra/src/storage/database.rs', PRODUCTION_SOURCE}
    if not needed <= set(sources) or not any(p.startswith('crates/app-core/src/') for p in sources):
        raise ValueError('actual complete app-infra/Core/Cargo source closure missing')
    if _artifact(root, proof['cargo_lock']) != Path(root).resolve() / 'Cargo.lock' or proof['cargo_lock']['sha256'] != sources['Cargo.lock']:
        raise ValueError('actual locked child compilation Cargo.lock differs')
    if proof['old_writer_manifest']['sha256'] != OLD4_MANIFEST_SHA256 or _file_hash(_artifact(root, proof['old_writer_manifest'])) != OLD4_MANIFEST_SHA256:
        raise ValueError('actual child frozen Store4 reader manifest differs')
    parent_map = context.get('parent_executions')
    if not isinstance(parent_map, dict) or set(parent_map) != set(TEST_NAMES):
        raise ValueError('independently inspected parent execution map required')
    actual_parents = {}
    for execution_ref in context['executions']:
        execution = _loads(_artifact(root, execution_ref).read_bytes())
        name = execution['command'][2]
        if execution != parent_map.get(name):
            raise ValueError('parent evidence differs from actual outer inspected process')
        actual_parents[name] = execution_ref
    parent = proof['parent']
    if not isinstance(parent, dict) or set(parent) != {'execution', 'binary', 'pid'} or parent['execution'] != actual_parents['bj_11_storage'] or _artifact(root, parent['binary']) != parent_binary:
        raise ValueError('child parent is not actual bj_11_storage invocation')
    parent_execution = parent_map['bj_11_storage']
    if _positive_int(parent['pid'], 'parent PID') != _positive_int(parent_execution.get('pid'), 'actual outer parent PID'):
        raise ValueError('self-reported parent PID differs from actual Popen process')
    start, end, parent_stdout, _ = _execution_logs(root, parent_execution)
    launches = _raw_frames(parent_stdout, CHILD_LAUNCH_MARKER, 'bj_11_storage')
    if len(launches) != 5:
        raise ValueError('actual compile/list/three exact child launch closure missing')
    compile_row, compile_period, cargo_stdout, _ = _child_execution(root, proof['compilation'], CHILD_COMPILE, environment, (start, end), launches)
    messages = [_loads(line) for line in cargo_stdout.splitlines() if line.strip()]
    if sum(m.get('reason') == 'build-finished' and m.get('success') is True for m in messages) != 1 or any(m.get('reason') == 'build-finished' and m.get('success') is not True for m in messages):
        raise ValueError('actual successful locked child Cargo build missing')
    artifacts = [m for m in messages if m.get('reason') == 'compiler-artifact' and m.get('target', {}).get('name') == 'app_infra' and m.get('target', {}).get('kind') == ['lib'] and m.get('profile', {}).get('test') is True and m.get('executable')]
    if len(artifacts) != 1:
        raise ValueError('exact app-infra private libtest Cargo artifact required')
    artifact = artifacts[0]
    if not re.search(r'(^|[#/])app-infra(?:@|[ #]|$)', str(artifact.get('package_id', ''))) or _owned_path(root, artifact['target']['src_path']) != Path(root).resolve() / 'crates/app-infra/src/lib.rs':
        raise ValueError('child Cargo artifact package/source differs')
    child_binary = _artifact(root, proof['binary'])
    if child_binary == parent_binary or _file_hash(child_binary) == _file_hash(parent_binary) or not child_binary.is_relative_to(base.resolve()) or _file_hash(_owned_path(root, artifact['executable'])) != proof['binary']['sha256']:
        raise ValueError('actual child executable bytes/ownership substituted by parent')
    listed, list_period, inventory, _ = _child_execution(root, proof['listing'], [str(child_binary), '--list', '--format', 'terse'], environment, (start, end), launches)
    if compile_period[1] > list_period[0]:
        raise ValueError('child inventory preceded actual successful compilation')
    names = re.findall(r'^(.+): test$', inventory, re.M)
    if not names or len(set(names)) != len(names) or not set(CHILD_TESTS.values()) <= set(names) or re.search(r': benchmark$', inventory, re.M):
        raise ValueError('actual unfiltered libtest inventory omits fixed tests')
    if not isinstance(proof['executions'], list) or len(proof['executions']) != 3 or not isinstance(proof['fault_records'], dict) or set(proof['fault_records']) != set(CHILD_TESTS):
        raise ValueError('three exact child executions/four distinct faults required')
    executions, pids, seen_faults, before_paths = {}, {parent['pid'], compile_row['pid'], listed['pid']}, set(), set()
    if len(pids) != 3:
        raise ValueError('actual parent/compile/list PIDs are not distinct')
    invocation_ids, database_identities = set(), set()
    for execution_ref in proof['executions']:
        row = _loads(_artifact(root, execution_ref).read_bytes())
        command = row.get('command')
        if not isinstance(command, list) or len(command) != 4 or command[0] != str(child_binary) or command[1] != '--exact' or command[2] not in CHILD_TESTS.values() or command[3] != '--nocapture' or command[2] in executions:
            raise ValueError('wrong/filtered/duplicated exact child libtest invocation')
        row, period, stdout, _ = _child_execution(root, execution_ref, command, environment, (start, end), launches)
        if list_period[1] > period[0]:
            raise ValueError('exact child execution preceded complete actual inventory')
        if row['pid'] in pids:
            raise ValueError('actual parent/child process identities reused')
        pids.add(row['pid'])
        _actual_test_summary(stdout, [command[2]])
        identities = _raw_frames(stdout, CHILD_ID_MARKER, command[2])
        expected_id = {'pid': row['pid'], 'ppid': parent['pid'], 'binary': proof['binary'], 'nonce': context['nonce'], 'source_tree_sha256': context['source_tree_sha256'], 'output': environment['LUNA_BINDING_OUTPUT'], 'old_writer_manifest': proof['old_writer_manifest']}
        if identities != [expected_id]:
            raise ValueError('actual child getpid/getppid/current_exe/source/nonce differs')
        executions[command[2]] = (row, period, stdout)
    used_events = []
    for route, fault_ref in proof['fault_records'].items():
        path = _artifact(root, fault_ref)
        if str(path) in seen_faults:
            raise ValueError('one fault record reused for another route')
        seen_faults.add(str(path))
        record = _loads(path.read_bytes())
        fields = {'kind', 'route', 'case_id', 'invocation_id', 'exact_test', 'nonce', 'source_tree_sha256', 'child_pid', 'child_ppid', 'original_database_path', 'database_file_device_identity', 'profile_id', 'server_id', 'scope_id', 'now_ms', 'writer', 'writer_process_binary', 'before', 'after', 'backup', 'captures', 'recovery', 'timeline', 'os_facts'}
        if not isinstance(record, dict) or set(record) != fields or record['kind'] != 'actual-schema5-migration-host-fault/v2' or record['route'] != route or record['exact_test'] != CHILD_TESTS[route] or record['writer'] != 'SqliteStore::open' or record['writer_process_binary'] != proof['binary'] or record['nonce'] != context['nonce'] or record['source_tree_sha256'] != context['source_tree_sha256']:
            raise ValueError('fixed actual child fault identity/source/writer differs')
        row, period, stdout = executions[record['exact_test']]
        if record['child_pid'] != row['pid'] or type(record['child_pid']) is not int or record['child_ppid'] != parent['pid'] or type(record['child_ppid']) is not int or record['profile_id'] != 'migration-fault-profile' or record['server_id'] != 'controlled-fault' or type(record['now_ms']) is not int or record['now_ms'] != 100:
            raise ValueError('actual fault process/arguments differ')
        for key in ('case_id', 'invocation_id', 'scope_id'):
            if not isinstance(record[key], str) or not record[key]:
                raise ValueError('actual per-route invocation identity missing')
        if record['invocation_id'] in invocation_ids:
            raise ValueError('actual child invocation reused across fault routes')
        invocation_ids.add(record['invocation_id'])
        if '..' in Path(record['original_database_path']).parts:
            raise ValueError('actual original database path contains traversal')
        original = _owned_path(root, record['original_database_path'])
        identity = record['database_file_device_identity']
        if not original.is_relative_to(base.resolve()) or str(original) in before_paths or set(identity) != {'device', 'inode'} or any(type(v) is not int or v <= 0 for v in identity.values()):
            raise ValueError('distinct actual original DB path/file/device identity required')
        before_paths.add(str(original))
        file_identity = (identity['device'], identity['inode'])
        if file_identity in database_identities:
            raise ValueError('different fault route reused the same actual DB file')
        database_identities.add(file_identity)
        raw_faults = _raw_frames(stdout, FAULT_MARKER, record['exact_test'])
        if raw_faults.count(record) != 1:
            raise ValueError('fault record absent from actual child raw stdout')
        timeline = _loads(_artifact(root, record['timeline']).read_bytes())
        binding = {k: record[k] for k in _timeline_policy(root)['event_envelope']['required'] if k in record}
        events = ex_storage_timeline_consumer(root, timeline, binding, period)
        emitted = _raw_frames(stdout, BOUNDARY_MARKER, record['exact_test'])
        if [e for e in emitted if e.get('invocation_id') == record['invocation_id']] != timeline['events']:
            raise ValueError('boundary journal differs from actual child stdout callbacks')
        used_events.extend(timeline['events'])
        _fault_os_facts(root, record, events, period, row['pid'])
        _fault_capture_lineage(root, record, events, period)
        if events['post_error_invariants']['payload'].get('snapshots') != {key: record[key] for key in ('before', 'after', 'backup')}:
            raise ValueError('post-error boundary did not bind complete actual DB/backup invariants')
        runtime_route = next(r for r in routes if r['id'] == route)
        for key in ('before', 'after', 'backup'):
            if runtime_route[key] is None or _file_hash(_artifact(root, record[key])) != _file_hash(_artifact(base, snapshots[runtime_route[key]])):
                raise ValueError('child fault DB snapshot differs from exact runtime route: ' + key)
        before = _snapshot(root, root, record['before'], 4)
        after = _snapshot(root, root, record['after'], 4)
        backup = _snapshot(root, root, record['backup'], 4)
        try:
            _same_database(before, after)
            _same_database(before, backup)
            if record['before']['sha256'] != record['after']['sha256'] or before.execute('SELECT profile_id FROM profile_meta WHERE singleton=1').fetchone() != (record['profile_id'],):
                raise ValueError('actual original bytes/profile changed')
        finally:
            before.close(); after.close(); backup.close()
        relay_map = {'writer_open_started': 'api_entry', 'fault_armed': 'fault_applied', 'writer_open_failed': 'native_failure', 'migration_transaction_started': 'transaction_started', 'migration_ddl_applied': 'ddl_applied', 'transaction_rolled_back': 'transaction_scope_exited_after_drop'}
        for relay in (e for e in route_events if e['route'] == route):
            if relay['event'] in relay_map and relay['at_ms'] != events[relay_map[relay['event']]]['observed_wall_ms']:
                raise ValueError('parent relay time substituted for actual child callback')
        if route == 'rollback':
            recovery = record['recovery']
            if not isinstance(recovery, dict) or set(recovery) != {'snapshot', 'original_database_path', 'profile_id', 'server_id', 'now_ms'} or recovery['original_database_path'] != record['original_database_path'] or recovery['profile_id'] != record['profile_id'] or recovery['server_id'] != 'recovered' or recovery['now_ms'] != 101:
                raise ValueError('same original fault recovery actual arguments missing')
            recovered = _snapshot(root, root, recovery['snapshot'], 5)
            original_db = _snapshot(root, root, record['before'], 4)
            try:
                _old_rows_preserved(original_db, recovered, now_ms=101, server_id='recovered')
                _legacy_backfill(recovered, _rows(original_db, 'source_selections'))
                _boundary_facts(recovered)
            finally:
                original_db.close(); recovered.close()
        elif record['recovery'] is not None:
            raise ValueError('unexecuted recovery must not be invented')
    for name, (_, _, stdout) in executions.items():
        emitted = _raw_frames(stdout, BOUNDARY_MARKER, name)
        if any(e not in used_events for e in emitted):
            raise ValueError('unconsumed actual child boundary invocation')
        faults = _raw_frames(stdout, FAULT_MARKER, name)
        if len(faults) != (2 if 'actual_schema5_host_readonly_and_busy' in name else 1):
            raise ValueError('actual child raw fault record closure differs')
    return {'kind': CHILD_KIND, 'parent_pid': parent['pid'], 'child_tests': sorted(executions), 'fault_routes': sorted(CHILD_TESTS), 'production_timeline_events': len(used_events)}


def inspect_runtime(root=ROOT, evidence=None, *, context=None):
    """Inspect production evidence under a runner-supplied immutable context.

    Context keys are fixed and must come from the coordinator's actual binary
    execution, never from the producer receipt. The producer writes the fixed
    RUNTIME_FILENAME under LUNA_BINDING_OUTPUT using the same nonce/source.
    """
    report = _report('schema_five_runtime_inspection', [], scope='selection_schema5_runtime_only')
    report['status'] = 'blocked'
    if evidence is None:
        report['errors'] = ['schema5 actual production producer/context missing; not_run/blocked']
        return report
    try:
        if isinstance(evidence, dict):
            runtime_path = _artifact(root, evidence)
            if runtime_path.name != RUNTIME_FILENAME:
                raise ValueError('fixed runtime producer filename required')
            evidence = runtime_path.parent
        elif not (Path(evidence) / RUNTIME_FILENAME).is_file():
            report['errors'] = ['schema5 actual production producer missing; not_run/blocked']
            return report
    except (OSError, ValueError, TypeError) as error:
        report.update(status='failed', errors=['schema5 runtime reference: ' + str(error)])
        return report
    # An existing malformed or incomplete manifest is a failure. It must not
    # disappear behind the ordinary absence/blocked branch.
    report['status'] = 'failed'
    try:
        errors = validate_contract(root)
        if errors:
            raise ValueError('; '.join(errors))
        required_context = {'nonce', 'source_tree_sha256', 'artifact_sha256', 'environment_sha256', 'started_at_ms', 'finished_at_ms'}
        if not isinstance(context, dict) or not required_context.issubset(context) or set(context) - required_context - {'test_names', 'executions', 'binary', 'producer_source', 'parent_executions', 'source_files'} or not NONCE.fullmatch(str(context['nonce'])):
            raise ValueError('exact externally supplied runtime context required')
        if any(not HEX64.fullmatch(str(context[key])) for key in ('source_tree_sha256', 'artifact_sha256', 'environment_sha256')):
            raise ValueError('runtime context source/artifact identities malformed')
        if any(type(context[k]) is not int or context[k] < 0 for k in ('started_at_ms', 'finished_at_ms')) or context['started_at_ms'] > context['finished_at_ms']:
            raise ValueError('runtime execution interval malformed')
        base = Path(evidence).absolute()
        if base.is_symlink() or any(p.is_symlink() for p in base.parents):
            raise ValueError('runtime evidence directory contains a symbolic link')
        receipt = read_json(base, RUNTIME_FILENAME)
        required = {'schema_version', 'kind', 'nonce', 'source_tree_sha256', 'artifact_sha256', 'environment',
                    'started_at_ms', 'finished_at_ms', 'producer', 'snapshots', 'routes', 'field_facts', 'relationship_facts', 'old_writer', 'sqlite', 'host', 'route_trace', 'old_suite', 'selection_snapshot', 'selection_rows'}
        child_mode = receipt.get('kind') == RUNTIME_KIND_V2
        if child_mode:
            required.add('child_proof')
            required.add('production_timelines')
        if set(receipt) != required or type(receipt['schema_version']) is not int or receipt['schema_version'] != (2 if child_mode else 1) or receipt['kind'] != (RUNTIME_KIND_V2 if child_mode else RUNTIME_KIND):
            raise ValueError('schema5 fixed runtime receipt schema differs')
        if not child_mode and {'parent_executions', 'source_files'} & set(context):
            raise ValueError('legacy runtime context cannot smuggle unconsumed child evidence')
        if child_mode and not {'parent_executions', 'source_files'} <= set(context):
            raise ValueError('v2 requires independently inspected actual parent/source context')
        for key in ('nonce', 'source_tree_sha256', 'artifact_sha256'):
            if receipt[key] != context[key]:
                raise ValueError('runtime external context mismatch: ' + key)
        if not context['started_at_ms'] <= receipt['started_at_ms'] <= receipt['finished_at_ms'] <= context['finished_at_ms']:
            raise ValueError('runtime receipt is outside actual execution interval')
        producer = receipt['producer']
        if set(producer) != {'package', 'target', 'tests', 'source_sha256', 'binary'} or producer['package'] != 'app-infra' or producer['target'] != 'source_selection_sets' or producer['tests'] != list(TEST_NAMES):
            raise ValueError('exact actual source_selection_sets production target required')
        if not (Path(root) / PRODUCTION_SOURCE).is_file() or producer['source_sha256'] != digest(safe_bytes(root, PRODUCTION_SOURCE)):
            raise ValueError('production test source is stale')
        binary = _artifact(root, producer['binary'])
        if digest(binary.read_bytes()) != context['artifact_sha256']:
            raise ValueError('compiled production artifact differs from actual runner')
        environment = receipt['environment']
        if not isinstance(environment, dict) or set(environment) != {'LUNA_BINDING_OUTPUT', 'LUNA_BINDING_NONCE', 'LUNA_BINDING_SOURCE_TREE'} or any(not isinstance(v, str) or not v for v in environment.values()):
            raise ValueError('actual exact three-variable execution environment required')
        if canonical_hash(environment) != context['environment_sha256'] or environment['LUNA_BINDING_NONCE'] != context['nonce'] or environment['LUNA_BINDING_SOURCE_TREE'] != context['source_tree_sha256'] or Path(environment['LUNA_BINDING_OUTPUT']).resolve() != base.resolve():
            raise ValueError('actual execution environment does not match the independent runner')
        for key, fields in (('sqlite', {'version', 'source_id'}), ('host', {'platform', 'architecture', 'os_release'})):
            if not isinstance(receipt[key], dict) or set(receipt[key]) != fields or any(not isinstance(v, str) or not v for v in receipt[key].values()):
                raise ValueError('actual typed SQLite and platform environment required')
        actual_events = _production_executions(root, context, environment, binary)
        snapshots = receipt['snapshots']
        routes = receipt['routes']
        expected_routes = {'fresh': (None, 5), 'from1': (1, 5), 'from2': (2, 5), 'from3': (3, 5), 'from4': (4, 5),
                           'reopen5': (5, 5), 'old4-rejects5': (5, 5), 'rollback': (4, 4), 'readonly': (4, 4),
                           'busy': (4, 4), 'enospc': (4, 4), 'kill': (4, None), 'restore': (4, 5)}
        if not isinstance(snapshots, dict) or not isinstance(routes, list) or len(routes) != len(expected_routes) or {r.get('id') for r in routes} != set(expected_routes):
            raise ValueError('all exact schema5 migration/failure/old-reader routes required')
        recovery = _route_events(base, receipt['route_trace'], actual_events, context, routes)
        child_report = None
        if child_mode:
            child_report = _inspect_child_proof(root, base, receipt['child_proof'], context, environment, binary, snapshots, routes, actual_events)
            child_report['parent_timeline_routes'] = _inspect_parent_timelines(root, receipt['production_timelines'], context, routes, snapshots, actual_events)
        used = set()
        for route in routes:
            if set(route) != {'id', 'before', 'after', 'after_version', 'backup', 'error_code'}:
                raise ValueError('fixed route shape differs')
            identifier = route['id']
            before_version, after_version = expected_routes[identifier]
            actual_version = route['after_version']
            if type(actual_version) is not int or (actual_version not in (4, 5) if identifier == 'kill' else actual_version != after_version):
                raise ValueError('actual route version differs: ' + identifier)
            after_ref = snapshots[route['after']]
            used.add(route['after'])
            after = _snapshot(root, base, after_ref, actual_version)
            before = None
            try:
                if before_version is None:
                    if route['before'] is not None or route['backup'] is not None:
                        raise ValueError('fresh route must prove an absent original without invented backup')
                else:
                    used.add(route['before'])
                    before_ref = snapshots[route['before']]
                    before = _snapshot(root, base, before_ref, before_version)
                    if identifier in ('from1', 'from2', 'from3', 'from4', 'restore'):
                        _old_rows_preserved(before, after, **recovery[identifier])
                        _legacy_backfill(after, _rows(before, 'source_selections'))
                        if route['backup'] is None:
                            raise ValueError('actual old version backup missing')
                        used.add(route['backup'])
                        backup = _snapshot(root, base, snapshots[route['backup']], before_version)
                        try:
                            if v1.inspect_connection(before) != v1.inspect_connection(backup) or any(_rows(before, t) != _rows(backup, t) for t in v1.inspect_connection(before)['tables']):
                                raise ValueError('preupgrade backup did not preserve every original table/row')
                        finally:
                            backup.close()
                    elif identifier in ('old4-rejects5', 'rollback', 'readonly', 'busy', 'enospc'):
                        if before_ref['sha256'] != after_ref['sha256']:
                            raise ValueError('failed/rejected route changed original closed SQLite bytes')
                        if not isinstance(route['error_code'], str) or not route['error_code']:
                            raise ValueError('actual rejected/failure error missing')
                        if child_mode and identifier in CHILD_TESTS:
                            # The new child path must independently consume its
                            # real preupgrade backup, not merely attach a ref.
                            if route['backup'] is None:
                                raise ValueError('actual fault preupgrade backup missing')
                            used.add(route['backup'])
                            backup = _snapshot(root, base, snapshots[route['backup']], before_version)
                            try:
                                _same_database(before, backup)
                            finally:
                                backup.close()
                if actual_version == 5:
                    # Recompute constraints from the actual production snapshot;
                    # neither marker rows nor claimed assertion counts suffice.
                    facts = _boundary_facts(after)
                    if identifier == 'fresh':
                        if receipt['field_facts'] != facts['fields'] or receipt['relationship_facts'] != facts['relationships_and_triggers']:
                            raise ValueError('actual production field/FK/trigger/view facts differ from independent replay')
            finally:
                after.close()
                if before is not None:
                    before.close()
        used.add(receipt['selection_snapshot'])
        selection = _snapshot(root, base, snapshots[receipt['selection_snapshot']], 5)
        try:
            selection.row_factory = sqlite3.Row
            selected_sets = [dict(r) for r in selection.execute("SELECT * FROM source_selection_sets WHERE origin_kind='explicit_set' AND state='sealed' ORDER BY session_id,selection_set_revision")]
            members = [dict(r) for r in selection.execute('SELECT * FROM source_selection_members ORDER BY session_id,selection_set_revision,member_ordinal')]
            if not any(row['member_count'] >= 2 for row in selected_sets) or receipt['selection_rows'] != {'sets': selected_sets, 'members': members}:
                raise ValueError('actual Repository multi-member field rows missing or differ from SQLite')
        finally:
            selection.close()
        if set(snapshots) != used:
            raise ValueError('unconsumed or omitted runtime snapshot identities')
        candidate = _artifact(base, snapshots[next(r['after'] for r in routes if r['id'] == 'old4-rejects5')])
        _old_writer(root, base, receipt['old_writer'], candidate)
        _verify_old_suite(root, base, receipt['old_suite'])
        report.update(status='passed', product_migration_executed=True, inspected_routes=sorted(expected_routes),
                      inspected_fields=list(FIELD_IDS), nonce=context['nonce'], source_tree_sha256=context['source_tree_sha256'])
        if child_report is not None:
            report['child_execution'] = child_report
    except (OSError, ValueError, TypeError, KeyError, AttributeError, IndexError, OverflowError, sqlite3.Error, subprocess.SubprocessError) as error:
        report['errors'] = ['schema5 runtime evidence: ' + str(error)]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--database', choices=('profile', 'dictionary'), default='profile')
    parser.add_argument('--boundaries', action='store_true')
    parser.add_argument('--runtime', type=Path)
    parser.add_argument('--context', type=Path)
    args = parser.parse_args()
    if args.runtime is not None or args.context is not None:
        result = inspect_runtime(args.root, args.runtime, context=json.loads(args.context.read_text()) if args.context else None)
    elif args.boundaries:
        result = assess_boundaries(args.root)
    else:
        result = assess(args.root, args.candidate, args.database)
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
