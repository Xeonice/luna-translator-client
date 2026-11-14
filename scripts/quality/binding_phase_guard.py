#!/usr/bin/env python3
"""Ordered plan -> gates -> product-write admission for the binding repair.

The three phases share one product change, but not one checkbox or approval.
Evidence stays ignored to avoid a source/hash cycle. Actor independence is an
auditable declaration, not a cryptographic identity or remote branch protection.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys

from gate_core import source_files

ROOT = Path(__file__).resolve().parents[2]
CHANGE = 'align-approved-binding-journey'
PREFIX = 'openspec/changes/' + CHANGE
POLICY = PREFIX + '/phase-policy.json'
SHA = re.compile(r'[a-f0-9]{64}\Z')
PRODUCT_ROOTS = frozenset({'apps', 'crates', 'native', 'resources'})
GUARD_MODULES = ('test_binding_phase_guard', 'test_binding_journey', 'test_storage_schema_v5')
PLAN_REVIEW_SHA256 = 'fdc1ffbff96c2191d228c034dc426add838f90e2792812f6cd9a48136869c658'
PLAN_AMENDMENT_REVIEW_SHA256 = 'd4dda2582ee39ed4cb1c8ab9de0db6c43e85f146b0130ef9b69fa6706f2a2658'
PLAN_RESUMPTION_REVIEW_SHA256 = 'f7a9610ba0d051f085d74901a3c1b3a4de2a0c8f5b0efd5f5f39266ec4b5d7a0'
PREDECESSOR_PINS = {
    'plan_review': PLAN_AMENDMENT_REVIEW_SHA256,
    'gate_review': '59448aacf4fd409dbba622d76094a67d8b153b1eb15657c7e194bb19861d32a9',
    'admission': '74024e311eca0cdaff66870fa76a62bf17de4d74872f1be14d640504a9955e4c',
    'policy': 'eb238c9077716b9710a2d6267116f338c335e5a0273873bbce9000c306265e34',
}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def tree_sha(mapping):
    return sha(json.dumps(mapping, sort_keys=True).encode())


def compact_sha(value):
    return sha(json.dumps(value, sort_keys=True, separators=(',', ':')).encode())


def now():
    return datetime.now(timezone.utc).isoformat()


def unique(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError('Duplicate binding phase JSON key: ' + key)
        out[key] = value
    return out


def load(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(
                          ValueError('Non-JSON phase value: ' + value)))


def path_at(root, relative):
    if (not isinstance(relative, str) or not relative or relative != relative.strip()
            or any(ord(c) < 32 or ord(c) == 127 for c in relative)
            or any(c in relative for c in ('\\', ':', '?', '#'))
            or PurePosixPath(relative).is_absolute()
            or any(p in ('', '.', '..') for p in relative.split('/'))):
        raise ValueError('Safe relative phase path required')
    result = Path(root).resolve()
    for part in relative.split('/'):
        result /= part
        if result.is_symlink():
            raise ValueError('Symlink phase evidence/source forbidden: ' + relative)
    return result


def strings(value):
    if (not isinstance(value, list) or not value
            or any(type(x) is not str or not x or x != x.strip() for x in value)
            or len(value) != len(set(value))):
        raise ValueError('Nonempty unique string list required')
    return value


def source_map(value):
    if (not isinstance(value, dict) or not value
            or any(type(p) is not str or not isinstance(h, str) or not SHA.fullmatch(h)
                   for p, h in value.items())):
        raise ValueError('Complete typed source map required')
    return value


def products(mapping):
    return {p: h for p, h in mapping.items() if PurePosixPath(p).parts[0] in PRODUCT_ROOTS}


def ref(root, relative):
    p = path_at(root, relative)
    if not p.is_file():
        raise ValueError('Missing phase input: ' + relative)
    return {'path': relative, 'sha256': sha(p.read_bytes())}


def checked_ref(root, value, *, ignored=False):
    if (not isinstance(value, dict) or set(value) != {'path', 'sha256'}
            or not isinstance(value['sha256'], str) or not SHA.fullmatch(value['sha256'])):
        raise ValueError('Exact path/hash phase reference required')
    if ignored and not value['path'].startswith('.build/'):
        raise ValueError('Phase proof must stay ignored')
    p = path_at(root, value['path'])
    if not p.is_file() or sha(p.read_bytes()) != value['sha256']:
        raise ValueError('Stale/missing phase reference: ' + value['path'])
    return p


def delta(before, after):
    return {p for p in before.keys() | after.keys() if before.get(p) != after.get(p)}


def policy_at(root):
    policy = load(path_at(root, POLICY))
    if (type(policy.get('schema_version')) is not int or policy['schema_version'] != 1
            or policy.get('kind') != 'ordered-binding-journey-phases/v1'
            or policy.get('change_id') != CHANGE
            or policy.get('required_sequence') != ['plan', 'gates', 'implementation']
            or policy.get('before_snapshot') != PREFIX + '/source-snapshot.json'
            or policy.get('product_completion') != 'incomplete'
            or policy.get('next_stage_allowed') is not False):
        raise ValueError('Binding phase policy identity/order differs')
    groups = [set(strings(policy[k])) for k in ('plan_paths', 'gate_paths', 'implementation_paths')]
    if any(groups[i] & groups[j] for i in range(3) for j in range(i)):
        raise ValueError('Phase path groups must be disjoint')
    for p in set.union(*groups):
        path_at(root, p)
    metadata = load(path_at(root, PREFIX + '/verification.json'))
    if (metadata.get('id') != CHANGE or metadata.get('kind') != 'product'
            or metadata.get('phase_policy') != POLICY
            or set(strings(metadata['allowed_paths'])) != set.union(*groups)
            or metadata.get('review_evidence', {}).get('authors') != policy.get('authors')):
        raise ValueError('Exact product admission/phase path union differs')
    strings(policy['authors'])
    before = load(path_at(root, policy['before_snapshot']))
    source_map(before['files'])
    if (before.get('source_entries') != len(before['files'])
            or before.get('source_tree_sha256') != tree_sha(before['files'])
            or policy.get('before_tree_sha256') != before['source_tree_sha256']
            or source_map(policy['parent_product_source_map']) != products(before['files'])):
        raise ValueError('Parent source/product map identity differs')
    receipts = policy.get('receipts', {})
    if set(receipts) != {'plan', 'gates', 'implementation_admission'}:
        raise ValueError('Exact three phase receipt paths required')
    for p in receipts.values():
        if not isinstance(p, str) or not p.startswith('.build/'):
            raise ValueError('Phase receipts must be ignored')
        path_at(root, p)
    return policy, before['files']


def independent(review, policy, kind, phase):
    if (type(review.get('schema_version')) is not int or review['schema_version'] != 1
            or review.get('kind') != kind or review.get('change_id') != CHANGE
            or review.get('phase') != phase or review.get('status') != 'approved'
            or review.get('independent_non_author_review') is not True
            or review.get('blocking_findings') != []
            or review.get('author_ids') != policy['authors']
            or not isinstance(review.get('reviewer_id'), str)
            or not review['reviewer_id'].strip()
            or review['reviewer_id'] in policy['authors']):
        raise ValueError('Independent non-author ' + phase + ' approval required')


def phase_receipts(policy):
    """Original receipts remain history; a reviewed epoch has separate slots."""
    epoch = policy.get('resume_epoch')
    if epoch is None:
        return policy['receipts']
    if (not isinstance(epoch, dict) or type(epoch.get('schema_version')) is not int or epoch.get('schema_version') != 1
            or epoch.get('epoch_id') != 'r3'
            or epoch.get('kind') != 'admitted-p3-plan-gates-resumption/v1'
            or epoch.get('product_completion') != 'incomplete'
            or epoch.get('next_stage_allowed') is not False):
        raise ValueError('Exact independently reviewed r3 resumption identity required')
    expected = {
        'plan': '.build/binding-journey-alignment-20261109/phase-1-r3/plan-review.json',
        'gates': '.build/binding-journey-alignment-20261109/phase-2-r3/gate-review.json',
        'implementation_admission': '.build/binding-journey-alignment-20261109/phase-3-r3/admission.json',
        'gate_packet': '.build/binding-journey-alignment-20261109/phase-2-r3/packet/packet.json',
    }
    if epoch.get('receipts') != expected:
        raise ValueError('New epoch cannot overwrite or reuse a predecessor receipt slot')
    return {key: expected[key] for key in policy['receipts']}


def file_sha(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(4 * 1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def lexical_archive_path(value):
    """Normalize a retained historical spelling, without following any link."""
    if (not isinstance(value, str) or not value or PurePosixPath(value).is_absolute()
            or any(ord(c) < 32 for c in value) or any(c in value for c in ('\\', ':'))):
        raise ValueError('Historical alias must stay a relative repository spelling')
    parts = []
    for part in value.split('/'):
        if part in ('', '.'):
            continue
        if part == '..':
            if not parts:
                raise ValueError('Historical alias escaped repository')
            parts.pop()
        else:
            parts.append(part)
    if not parts:
        raise ValueError('Empty historical canonical path')
    return '/'.join(parts)


def verify_canonical_record(root, row, archive_root, *, is_safe):
    """Verify immutable archived history without promoting mutable origins.

    Safe origin components must still be ordinary paths. Current authority
    and failure-original references are separately hash-checked by checked_ref.
    Quarantined origins are never opened or resolved.
    """
    archive_root = path_at(root, Path(archive_root).relative_to(Path(root)).as_posix())
    target = path_at(root, row['archive_path'])
    if (not target.is_relative_to(archive_root)
            or not stat.S_ISREG(target.lstat().st_mode) or target.stat().st_nlink != 1
            or file_sha(target) != row['sha256'] or target.stat().st_size != row['bytes']):
        raise ValueError('Archived canonical regular bytes differ: ' + row['archive_path'])
    if is_safe:
        origin = path_at(root, row['original_path'])
        if not stat.S_ISREG(origin.lstat().st_mode):
            raise ValueError('Safe canonical origin acquired a nonregular component: ' + row['original_path'])
    elif (row.get('origin') != 'untrusted_not_revalidated'
          or row.get('current_authority') is not False or row.get('tool_input_allowed') is not False
          or row.get('target_followed_by_reviewer') is not False):
        raise ValueError('Quarantined original/tool authority must never be followed or promoted')


def require_preservation_only(value):
    if (value.get('product_completion') != 'incomplete'
            or value.get('next_stage_allowed') is not False):
        raise ValueError('Preservation proof cannot promote product completion')


def verify_resumption_checkpoint(root, policy):
    """Consume the sealed real P3, including safe lineage and quarantine.

    Historical aliases and rejected claims remain retained bytes. They never
    relax path_at/checked_ref or become current execution/approval authority.
    """
    phase_receipts(policy)
    epoch = policy['resume_epoch']
    binding = epoch.get('checkpoint')
    if (not isinstance(binding, dict)
            or binding.get('status') != 'verified_preservation_for_plan_resumption_only'
            or binding.get('source_correctness_approved') is not False
            or binding.get('implementation_admitted') is not False
            or binding.get('gate_amendment_admitted') is not False):
        raise ValueError('Reviewed predecessor preservation, not a new write grant, is required')
    checkpoint_path = checked_ref(root, binding['manifest'], ignored=True)
    checkpoint = load(checkpoint_path)
    review = load(checked_ref(root, binding['independent_review'], ignored=True))
    authors = strings(checkpoint.get('actual_implementation_authors'))
    if (review.get('kind') != 'independent-binding-predecessor-checkpoint-review/v1'
            or type(review.get('schema_version')) is not int or review.get('schema_version') != 1 or review.get('change_id') != CHANGE
            or review.get('phase') != 'predecessor_checkpoint'
            or review.get('epoch_id') != 'implementation-resumption-r3'
            or review.get('status') != 'approved'
            or review.get('approval_scope') != 'preservation_for_plan_resumption_only'
            or review.get('independent_non_author_review') is not True
            or review.get('author_ids') != authors
            or not isinstance(review.get('reviewer_id'), str) or not review['reviewer_id']
            or review['reviewer_id'] in set(authors) | set(policy['authors'])
            or review.get('blocking_findings') != []
            or review.get('checkpoint') != binding['manifest']
            or review.get('raw_checkpoint_and_inventories_modified') is not False):
        raise ValueError('Independent exact predecessor checkpoint review required')
    require_preservation_only(checkpoint)
    require_preservation_only(review)
    if (checkpoint.get('kind') != 'binding-predecessor-implementation-checkpoint-candidate/v1'
            or type(checkpoint.get('schema_version')) is not int or checkpoint.get('schema_version') != 1 or checkpoint.get('change_id') != CHANGE
            or checkpoint.get('status') != 'pending_independent_checkpoint_review'
            or checkpoint.get('independent_review') is not None
            or checkpoint.get('new_package_built') is not False
            or checkpoint.get('new_package_replaced') is not False
            or checkpoint.get('source_stable_through_archive') is not True):
        raise ValueError('Original predecessor checkpoint bytes/status cannot be rewritten')
    full = source_map(load(checked_ref(root, checkpoint['full_source_map'], ignored=True)))
    product = source_map(load(checked_ref(root, checkpoint['product_source_map'], ignored=True)))
    if (checkpoint['full_source_map'] != binding['full_source_map']
            or checkpoint['product_source_map'] != binding['product_source_map']
            or product != products(full) or len(product) != checkpoint.get('product_source_entries')
            or tree_sha(full) != checkpoint.get('source_tree_sha256')
            or len(full) != checkpoint.get('source_entries')
            or review.get('source_tree_sha256') != tree_sha(full)
            or review.get('full_source_map') != checkpoint['full_source_map']
            or review.get('product_source_map') != checkpoint['product_source_map']
            or binding.get('source_tree_sha256') != tree_sha(full)
            or binding.get('source_entries') != len(full)
            or binding.get('product_source_entries') != len(product)):
        raise ValueError('Complete predecessor source/product closure differs')
    inventory = load(checked_ref(root, checkpoint['archive_inventory'], ignored=True))
    if (not isinstance(inventory, list) or len(inventory) != len(full)
            or {row.get('path') for row in inventory} != set(full)):
        raise ValueError('Every checkpoint source file needs one independent archived original')
    archive_root = checkpoint_path.parent / checkpoint['source_archive']
    for row in inventory:
        relative = row['path']
        archived = path_at(root, (archive_root / relative).relative_to(root).as_posix())
        if (not stat.S_ISREG(archived.lstat().st_mode) or archived.stat().st_nlink != 1
                or file_sha(archived) != full[relative] or row.get('sha256') != full[relative]
                or archived.stat().st_size != row.get('bytes')
                or row.get('copy_kind') not in {'apfs_cow_clone', 'independent_copy'}):
            raise ValueError('Checkpoint source archive is missing/aliased/stale: ' + relative)
    actual_archive = {p.relative_to(archive_root).as_posix() for p in archive_root.rglob('*')
                      if p.is_file() or p.is_symlink()}
    if actual_archive != set(full):
        raise ValueError('Checkpoint source archive has extra/missing files')
    preserved = checkpoint.get('preserved_predecessor_records')
    if not isinstance(preserved, dict) or set(preserved) != set(PREDECESSOR_PINS):
        raise ValueError('All four exact predecessor records must be preserved')
    parents = {}
    for name, pin in PREDECESSOR_PINS.items():
        if (checkpoint['predecessor'].get(name, {}).get('sha256') != pin
                or preserved[name].get('sha256') != pin):
            raise ValueError('Predecessor record does not match independently frozen identity: ' + name)
        saved = checked_ref(root, preserved[name], ignored=True)
        parents[name] = load(saved)
        original = checkpoint['predecessor'][name]
        if name != 'policy':
            checked_ref(root, original, ignored=True)
        elif original['path'] != POLICY or full.get(POLICY) != pin:
            raise ValueError('Historical policy must be archived, never replaced as R92')
    old_policy = parents['policy']
    old_plan, old_gate, old_admission = (parents[key] for key in ('plan_review', 'gate_review', 'admission'))
    if (policy['before_snapshot'] != old_policy['before_snapshot']
            or policy['before_tree_sha256'] != old_policy['before_tree_sha256']
            or policy['parent_product_source_map'] != old_policy['parent_product_source_map']
            or strings(old_policy['implementation_paths']) != strings(epoch['path_amendment']['original_implementation_paths'])
            or set(policy['implementation_paths']) != set(old_policy['implementation_paths']) | {'apps/desktop/src-tauri/src/platform/window_preview.rs'}
            or old_policy['plan_paths'] != policy['plan_paths'] or old_policy['gate_paths'] != policy['gate_paths']):
        raise ValueError('Resumption cannot reset original history or expand checkpoint write scope')
    for item in (old_plan, old_gate):
        if (item.get('status') != 'approved' or item.get('independent_non_author_review') is not True
                or item.get('author_ids') != old_policy['authors']
                or item.get('reviewer_id') in old_policy['authors']):
            raise ValueError('Predecessor P1/P2 independent authority differs')
        require_preservation_only(item)
    packet = load(checked_ref(root, old_gate['packet'], ignored=True))
    p2 = source_map(load(checked_ref(root, old_admission['phase2_source_map'], ignored=True)))
    if (old_admission.get('kind') != 'binding-implementation-write-admission/v1'
            or old_admission.get('implementation_admitted') is not True
            or old_admission['gate_review'] != checkpoint['predecessor']['gate_review']
            or old_admission['phase_policy'] != checkpoint['predecessor']['policy']
            or packet['parent_plan_review'] != checkpoint['predecessor']['plan_review']
            or tree_sha(p2) != old_admission['phase2_source_tree_sha256']
            or tree_sha(p2) != packet['phase2_source_tree_sha256']):
        raise ValueError('Exact actual predecessor P1/P2/P3 admission chain differs')
    changed = sorted(delta(p2, full))
    if (changed != checkpoint.get('actual_changed_paths_since_parent_p2')
            or changed != binding.get('actual_changed_paths_since_parent_p2')
            or not set(changed) <= set(old_policy['implementation_paths'])):
        raise ValueError('Predecessor P3 edits exceed the originally admitted implementation scope')
    for group, field in [('plan_paths', 'plan_basis_sha256'), ('gate_paths', 'gate_basis_sha256')]:
        expected = {p: full[p] for p in old_policy[group]}
        if (old_admission.get(field) != expected or packet.get(field) != expected
                or any(p2.get(path) != digest for path, digest in expected.items())):
            raise ValueError('Original frozen plan/gate bytes drifted before checkpoint')
    if checkpoint.get('approved_design_basis_sha256') != approved_design_basis(root):
        raise ValueError('Predecessor design closure changed')
    canonical = binding['canonical_archive']
    raw = load(checked_ref(root, canonical['raw_inventory'], ignored=True))
    view = load(checked_ref(root, canonical['view'], ignored=True))
    aliases = load(checked_ref(root, canonical['alias_mapping'], ignored=True))
    quarantine = load(checked_ref(root, canonical['quarantine_inventory'], ignored=True))
    claims = load(checked_ref(root, canonical['reference_claim_quarantine'], ignored=True))
    if (view.get('kind') != 'independent-canonical-ancestor-artifact-view/v2'
            or view.get('raw_inventory') != canonical['raw_inventory']
            or view.get('is_historical_actual_inventory') is not False
            or aliases.get('raw_inventory') != canonical['raw_inventory']
            or aliases.get('errors') != [] or claims.get('errors') != []
            or review.get('canonical_ancestor_artifact_view') != canonical['view']
            or review.get('alias_classification') != canonical['alias_mapping']
            or review.get('reference_quarantine_classification') != canonical['reference_claim_quarantine']
            or review.get('parent_symlink_quarantine') != canonical['quarantine_inventory']):
        raise ValueError('Independently classified raw/alias/quarantine preservation required')
    records, archival = view.get('records'), view.get('additional_archival_only')
    if (not isinstance(raw, list) or not isinstance(records, list) or not isinstance(archival, list)
            or len(raw) != view.get('raw_entries')
            or len(records) != view.get('safe_file_records')
            or len(archival) != view.get('archival_only_records') or archival != quarantine.get('rows')):
        raise ValueError('Exact safe and archival-only preservation partitions required')
    seen, origins, archives, safe_by_path = set(), set(), set(), {}
    for is_safe, rows in ((True, records), (False, archival)):
        for row in rows:
            origin, archive = row['original_path'], row['archive_path']
            if origin in origins or archive in archives:
                raise ValueError('Canonical ancestor targets cannot conflict or silently collapse aliases')
            origins.add(origin); archives.add(archive)
            verify_canonical_record(root, row, checkpoint_path.parent / 'ancestor-artifacts', is_safe=is_safe)
            if is_safe:
                safe_by_path[origin] = row
            indexes = row.get('raw_inventory_indexes')
            if not isinstance(indexes, list) or not indexes:
                raise ValueError('Every raw historical alias needs an exact retained mapping')
            for index in indexes:
                if type(index) is not int or not 0 <= index < len(raw) or index in seen:
                    raise ValueError('Raw ancestor entry mapped twice or omitted')
                seen.add(index)
                original = raw[index]
                if (lexical_archive_path(original['original_path']) != origin
                        or lexical_archive_path(original['archive_path']) != archive
                        or original.get('sha256') != row['sha256'] or original.get('bytes') != row['bytes']):
                    raise ValueError('Canonical alias mapping altered original bytes or provenance')
    if seen != set(range(len(raw))) or len(origins) != view.get('canonical_entries'):
        raise ValueError('Raw ancestor inventory requires complete one-to-one partition mapping')
    for item in checkpoint['failed_execution_original_refs']:
        checked_ref(root, item, ignored=True)
        if safe_by_path.get(item['path'], {}).get('sha256') != item['sha256']:
            raise ValueError('Failed discovery original omitted from immutable safe archive')
    for item in checkpoint['predecessor'].values():
        if item['path'].startswith('.build/') and safe_by_path.get(item['path'], {}).get('sha256') != item['sha256']:
            raise ValueError('Current direct predecessor authority is not in safe canonical partition')
    access = load(checked_ref(root, canonical['independent_access_observation'], ignored=True))
    if (access.get('quarantined_parent_link_files_opened') != []
            or access.get('unaccepted_historical_claim_documents_opened') != []
            or set(access.get('quarantined_claim_documents_opened', [])) - {
                '.build/binding-journey-alignment-20261109/phase-1-r1-preserved/unsigned-p2-packet.json'}
            or access.get('exit_code') != 0):
        raise ValueError('Current authority must not consume quarantined historical claims/tools')
    old = load(checked_ref(root, checkpoint['old_guard_execution'], ignored=True))
    old_stdout = load(checked_ref(root, old['stdout'], ignored=True))
    checked_ref(root, old['stderr'], ignored=True)
    if (old.get('argv') != ['python3', 'scripts/quality/binding_phase_guard.py', '--check', '--change', CHANGE, '--json']
            or old.get('cwd') != '.' or old.get('exit_code') != 0
            or old.get('source_before_after_equal') is not True
            or load(checked_ref(root, old['source_before'], ignored=True)) != full
            or load(checked_ref(root, old['source_after'], ignored=True)) != full
            or old.get('producer_sources') != {p: full[p] for p in ('scripts/quality/binding_phase_guard.py', 'scripts/quality/gate_core.py')}
            or any(old_stdout.get(k) != value for k, value in {
                'status': 'passed', 'phase': 'implementation', 'implementation_admitted': True,
                'source_tree_sha256': tree_sha(full), 'product_completion': 'incomplete', 'next_stage_allowed': False}.items())
            or not all(type(old.get(key)) is int for key in ('started_at_ms', 'finished_at_ms'))
            or not old['started_at_ms'] <= old['finished_at_ms'] <= checkpoint.get('sealed_at_ms', -1)
            or datetime.fromisoformat(review['reviewed_at']).timestamp() * 1000 < checkpoint['sealed_at_ms']):
        raise ValueError('Actual old guard command/source/time/raw result cannot be reconstructed by metadata')
    return checkpoint, review, full, product


def active_product_basis(root, policy):
    if policy.get('resume_epoch') is None:
        return policy['parent_product_source_map']
    return verify_resumption_checkpoint(root, policy)[3]


def verify_plan_resumption(root, policy):
    receipts = phase_receipts(policy)
    path = path_at(root, receipts['plan'])
    if file_sha(path) != PLAN_RESUMPTION_REVIEW_SHA256:
        raise ValueError('Exact independently approved r3 plan review bytes differ')
    review = load(path)
    independent(review, policy, 'independent-binding-plan-resumption-review/v1', 'plan')
    if (review.get('epoch_id') != 'r3' or review.get('approval_scope') != 'plan_only_gate_preparation_allowed'
            or review.get('phase_admission') != 'passed' or review.get('gate_preparation_allowed') is not True
            or review.get('implementation_admitted') is not False
            or review.get('implementation_resumption_admitted') is not False
            or review.get('source_correctness_approved') is not False):
        raise ValueError('r3 plan review grants only gate preparation')
    require_preservation_only(review)
    checkpoint, checkpoint_review, old, product = verify_resumption_checkpoint(root, policy)
    current_plan = source_map(load(checked_ref(root, review['source_map'], ignored=True)))
    if (review['checkpoint'] != policy['resume_epoch']['checkpoint']['manifest']
            or review['checkpoint_review'] != policy['resume_epoch']['checkpoint']['independent_review']
            or review.get('checkpoint_source_tree_sha256') != tree_sha(old)
            or review.get('phase_source_tree_sha256') != tree_sha(current_plan)
            or len(current_plan) != review.get('phase_source_entries')
            or products(current_plan) != product
            or not delta(old, current_plan) <= set(policy['plan_paths'])
            or any(old.get(p) != current_plan.get(p) for p in policy['gate_paths'])
            or review.get('phase_policy') != ref(root, POLICY)
            or review.get('plan_basis_sha256') != {p: ref(root, p)['sha256'] for p in policy['plan_paths']}
            or any(current_plan.get(p) != h for p, h in review['plan_basis_sha256'].items())
            or review.get('gate_basis_sha256') != {p: old[p] for p in policy['gate_paths']}
            or review.get('design_basis_sha256') != approved_design_basis(root)
            or datetime.fromisoformat(review['reviewed_at']) < datetime.fromisoformat(checkpoint_review['reviewed_at'])):
        raise ValueError('Actual r3 plan-only full maps, frozen products/gates or review basis differ')
    for value in review.get('preserved_direct_artifacts_sha256', {}).items():
        checked_ref(root, {'path': value[0], 'sha256': value[1]}, ignored=True)
    publications = review.get('publication_chain')
    if not isinstance(publications, list) or len(publications) != 2 or publications[-1] != review.get('publication'):
        raise ValueError('Both actual r3 publications and the rejected first review must be retained')
    previous = old
    for index, publication_ref in enumerate(publications):
        publication = load(checked_ref(root, publication_ref, ignored=True))
        before_ref, after_ref = publication['source_before'], publication['actual_after_source_map']
        actual_before = source_map(load(checked_ref(root, before_ref, ignored=True)))
        actual_after = source_map(load(checked_ref(root, after_ref, ignored=True)))
        changed = delta(actual_before, actual_after)
        declared = publication['actual_delta']
        declared_paths = set(declared if index == 0 else [row['path'] for row in declared])
        if (actual_before != previous or changed != declared_paths
                or len(changed) != publication['actual_delta_count']
                or not changed <= set(policy['plan_paths'])
                or products(actual_before) != product or products(actual_after) != product
                or any(actual_before.get(p) != actual_after.get(p) for p in policy['gate_paths'])
                or tree_sha(actual_before) != publication['source_before_tree_sha256']
                or tree_sha(actual_after) != publication.get('actual_after_tree_sha256', publication.get('source_after_tree_sha256'))
                or publication['checkpoint'] != binding_ref(policy)
                or publication['checkpoint_review'] != policy['resume_epoch']['checkpoint']['independent_review']):
            raise ValueError('Actual plan publication chain changed products/gates, erased history or hid a delta')
        if index == 1 and any(row['before_sha256'] != actual_before.get(row['path'])
                              or row['after_sha256'] != actual_after.get(row['path']) for row in declared):
            raise ValueError('Published plan revision per-file before/after hashes differ')
        previous = actual_after
    if (previous != current_plan
            or review['actual_before_source_map'] != load(checked_ref(root, publications[-1], ignored=True))['source_before']
            or review['before_source_tree_sha256'] != tree_sha(source_map(load(checked_ref(root, review['actual_before_source_map'], ignored=True))))):
        raise ValueError('Plan review does not bind the actual final publication before/after maps')
    previous_review = load(checked_ref(root, review['previous_changes_required_review'], ignored=True))
    checked_ref(root, review['previous_changes_required_markdown'], ignored=True)
    if previous_review.get('status') != 'changes_required' or review.get('previous_review_not_rewritten') is not True:
        raise ValueError('The actual rejected first r3 review must remain rejected and exact')
    registry = policy['resume_epoch'].get('execution_obligations')
    if (not isinstance(registry, dict) or registry.get('schema_version') != 1
            or registry.get('p2_only_registration_and_consumers_no_product_writes') is not True
            or tree_sha(registry) != review.get('obligation_registry_sha256')):
        raise ValueError('Reviewed machine execution obligation registry differs')
    return review, current_plan


def binding_ref(policy):
    return policy['resume_epoch']['checkpoint']['manifest']


def approved_design_basis(root):
    """Derive the complete approved closure; a review cannot choose a subset."""
    from design_authority import CURRENT_RECORD, CURRENT_RECORD_SHA256, current_record
    from current_preview_authority import RECORD as preview_record, RECORD_SHA256 as preview_hash
    records = {CURRENT_RECORD: CURRENT_RECORD_SHA256,
               'docs/development/design-approval-client-layout-20261105.json':
                   'ce8071da9fa6b429703ad3a658cef98de17047e2fdad01ed6fbd79ed1ebe0f01',
               preview_record: preview_hash}
    complete = current_record(root)
    basis = dict(records)
    for field in ('design_sources', 'runtime_dependencies', 'authority_dependencies'):
        basis.update(complete[field])
    for record in records:
        checked_ref(root, {'path': record, 'sha256': records[record]})
        if record != CURRENT_RECORD:
            for path, fact in load(path_at(root, record))['approved_sources'].items():
                basis[path] = fact['sha256']
    for path, digest in basis.items():
        checked_ref(root, {'path': path, 'sha256': digest})
    return basis


def verify_plan_amendment(root, policy, before, review):
    """Verify a real plan amendment after an unsigned gate candidate.

    The reviewed actual-after tree includes the old gates. The derived plan-only
    view is a comparison basis, explicitly not fabricated historical execution.
    Archived policy/metadata must be read from their immutable original bytes.
    """
    independent(review, policy, 'independent-binding-plan-amendment-review/v1', 'plan')
    parent_ref = review['parent_plan_review']
    if parent_ref.get('sha256') != PLAN_REVIEW_SHA256:
        raise ValueError('Amendment must retain the exact original independent P1 approval')
    parent = load(checked_ref(root, parent_ref, ignored=True))
    archive = load(checked_ref(root, review['parent_plan_archive'], ignored=True))
    if (archive.get('kind') != 'binding-plan-parent-source-archive/v1'
            or archive.get('schema_version') != 1 or archive.get('parent_plan_review') != parent_ref):
        raise ValueError('Original plan archive identity differs')
    rows = archive.get('sources')
    if not isinstance(rows, dict) or set(rows) != set(source_map(parent.get('plan_basis_sha256'))):
        raise ValueError('Archive must preserve every exact original plan source')
    archived = {}
    for path, row in rows.items():
        if (set(row) != {'source_path', 'source_sha256', 'archived_file'}
                or row['source_path'] != path or row['source_sha256'] != parent['plan_basis_sha256'][path]
                or row['archived_file'].get('sha256') != row['source_sha256']):
            raise ValueError('Archived original plan source identity differs: ' + path)
        archived[path] = checked_ref(root, row['archived_file'], ignored=True)
    old_policy = load(archived[POLICY])
    old_metadata = load(archived[PREFIX + '/verification.json'])
    independent(parent, old_policy, 'independent-binding-plan-review/v1', 'plan')
    old_groups = [set(strings(old_policy[key])) for key in ('plan_paths', 'gate_paths', 'implementation_paths')]
    old_before = load(archived[old_policy['before_snapshot']])
    if (old_policy['before_snapshot'] != policy['before_snapshot'] or old_before['files'] != before
            or old_policy['before_tree_sha256'] != tree_sha(before)
            or old_policy['parent_product_source_map'] != policy['parent_product_source_map']
            or old_policy['authors'] != policy['authors']
            or set(rows) != old_groups[0] or any(old_groups[a] & old_groups[b] for a in range(3) for b in range(a))
            or old_metadata.get('id') != CHANGE or old_metadata.get('kind') != 'product'
            or old_metadata.get('phase_policy') != POLICY
            or set(strings(old_metadata['allowed_paths'])) != set.union(*old_groups)
            or old_metadata.get('review_evidence', {}).get('authors') != old_policy['authors']
            or parent.get('phase_policy') != {'path': POLICY, 'sha256': sha(archived[POLICY].read_bytes())}):
        raise ValueError('Original archived policy/metadata/source history differs')
    old_plan = source_map(load(checked_ref(root, parent['source_map'], ignored=True)))
    if (tree_sha(old_plan) != parent.get('phase_source_tree_sha256')
            or len(old_plan) != parent.get('phase_source_entries')
            or parent.get('before_source_tree_sha256') != tree_sha(before)
            or delta(before, old_plan) != old_groups[0]
            or products(old_plan) != policy['parent_product_source_map']
            or any(old_plan.get(path) != row['source_sha256'] for path, row in rows.items())
            or parent.get('design_basis_sha256') != approved_design_basis(root)
            or parent.get('phase_admission') != 'passed'
            or parent.get('product_completion') != 'incomplete' or parent.get('next_stage_allowed') is not False):
        raise ValueError('Original independent P1 tree/basis cannot be rewritten by amendment')
    for item in parent.get('evidence', []):
        checked_ref(root, item, ignored=True)
    packet = load(checked_ref(root, review['parent_unsigned_gate_packet'], ignored=True))
    actual_before = source_map(load(checked_ref(root, review['actual_before_source_map'], ignored=True)))
    packet_source = source_map(load(checked_ref(root, packet['phase2_source_map'], ignored=True)))
    if (packet.get('kind') != 'binding-gate-phase-packet/v1' or packet.get('schema_version') != 1
            or packet.get('change_id') != CHANGE or packet.get('phase') != 'gates'
            or packet.get('status') != 'pending_independent_review' or packet.get('authors') != old_policy['authors']
            or packet.get('parent_plan_review') != {'path': old_policy['receipts']['plan'], 'sha256': PLAN_REVIEW_SHA256}
            or packet.get('phase_policy') != parent['phase_policy']
            or packet.get('before_source_tree_sha256') != tree_sha(before)
            or packet_source != actual_before or packet.get('phase2_source_tree_sha256') != tree_sha(actual_before)
            or review.get('amendment_before_source_tree_sha256') != tree_sha(actual_before)
            or not delta(old_plan, actual_before) <= old_groups[1]
            or products(actual_before) != policy['parent_product_source_map']
            or packet.get('product_source_map_sha256') != compact_sha(products(actual_before))
            or packet.get('plan_basis_sha256') != parent['plan_basis_sha256']
            or set(packet.get('gate_basis_sha256', {})) != old_groups[1]
            or any(actual_before.get(path) != value for path, value in packet['gate_basis_sha256'].items())
            or packet.get('product_completion') != 'incomplete' or packet.get('next_stage_allowed') is not False):
        raise ValueError('Amendment parent must be the actual unsigned original P2 candidate')
    actual_after = source_map(load(checked_ref(root, review['actual_after_source_map'], ignored=True)))
    if (review['actual_after_source_map'] != review['source_map']
            or tree_sha(actual_after) != review.get('phase_source_tree_sha256')
            or len(actual_after) != review.get('phase_source_entries')
            or delta(actual_before, actual_after) != set(strings(review.get('actual_changed_paths')))
            or not delta(actual_before, actual_after) <= set(policy['plan_paths'])
            or products(actual_after) != policy['parent_product_source_map']
            or any(actual_before.get(path) != actual_after.get(path) for path in old_groups[1])
            or not old_groups[0] <= set(policy['plan_paths'])
            or not old_groups[1] <= set(policy['gate_paths'])
            or not old_groups[2] <= set(policy['implementation_paths'])):
        raise ValueError('Actual amendment must change plan only, retaining original gates and product bytes')
    derived = dict(before)
    for path in policy['plan_paths']:
        if path not in actual_after:
            raise ValueError('Current amended plan source is missing: ' + path)
        derived[path] = actual_after[path]
    view = review['derived_view']
    if (view.get('is_historical_actual_tree') is not False
            or view.get('entries') != len(derived) or view.get('tree_sha256') != tree_sha(derived)
            or source_map(load(checked_ref(root, view['source_map'], ignored=True))) != derived):
        raise ValueError('Derived plan-only view must be independently reconstructed, never asserted as actual history')
    return derived


def verify_plan(root, policy=None):
    policy, before = policy_at(root) if policy is None else (policy, load(path_at(root, policy['before_snapshot']))['files'])
    if policy.get('resume_epoch') is not None:
        return verify_plan_resumption(root, policy)
    review_path = path_at(root, policy['receipts']['plan'])
    raw_hash = sha(review_path.read_bytes())
    if raw_hash not in {PLAN_REVIEW_SHA256, PLAN_AMENDMENT_REVIEW_SHA256}:
        raise ValueError('Exact independently approved P1 review bytes differ')
    review = load(review_path)
    amended = review.get('kind') == 'independent-binding-plan-amendment-review/v1'
    if amended != (raw_hash == PLAN_AMENDMENT_REVIEW_SHA256):
        raise ValueError('P1 approval kind differs from independently pinned review generation')
    independent(review, policy, review['kind'] if amended else 'independent-binding-plan-review/v1', 'plan')
    if (review.get('before_source_tree_sha256') != tree_sha(before)
            or review.get('phase_admission') != 'passed'
            or review.get('implementation_admitted') is not False
            or review.get('product_completion') != 'incomplete'
            or review.get('next_stage_allowed') is not False):
        raise ValueError('Plan approval cannot grant product implementation/release')
    checked_ref(root, review['phase_policy'])
    if review['phase_policy'] != ref(root, POLICY):
        raise ValueError('Reviewed phase policy differs')
    phase_map = (verify_plan_amendment(root, policy, before, review) if amended else
                 source_map(load(checked_ref(root, review['source_map'], ignored=True))))
    if ((not amended and (tree_sha(phase_map) != review.get('phase_source_tree_sha256')
                         or len(phase_map) != review.get('phase_source_entries')))
            or delta(before, phase_map) != set(policy['plan_paths'])
            or products(phase_map) != policy['parent_product_source_map']
            or compact_sha(products(phase_map)) != review.get('product_source_map_sha256')):
        raise ValueError('Reviewed plan tree/delta/product identity differs')
    expected = {p: ref(root, p)['sha256'] for p in policy['plan_paths']}
    if (review.get('plan_basis_sha256') != expected
            or any(phase_map.get(p) != h for p, h in expected.items())):
        raise ValueError('Frozen plan/metadata/oracle changed after P1 review')
    design = source_map(review.get('design_basis_sha256'))
    if design != approved_design_basis(root):
        raise ValueError('Plan review must bind the exact complete approved design closure')
    for item in review.get('evidence', []):
        checked_ref(root, item, ignored=True)
    return review, phase_map


def preimplementation(root):
    policy, _ = policy_at(root)
    plan, phase_map = verify_plan(root, policy)
    current = source_files(Path(root))
    changes = delta(phase_map, current)
    if not changes <= set(policy['gate_paths']):
        raise ValueError('P2 changed non-gate paths before write admission: ' + ', '.join(sorted(changes - set(policy['gate_paths']))))
    # verify_plan has just independently validated the conditional checkpoint.
    # Reuse its exact complete plan map, never a caller-supplied product basis.
    if products(current) != products(phase_map):
        raise ValueError('P2 product source must remain byte-exact to its verified active basis')
    return policy, plan, current


def run_guard_tests(root, output):
    output_path = fresh_directory(root, output)
    command = [sys.executable, '-m', 'unittest', '-v', *GUARD_MODULES]
    before = source_files(Path(root))
    started = now()
    result = subprocess.run(command, cwd=Path(root) / 'scripts/quality', text=True,
                            capture_output=True, timeout=180)
    (output_path / 'stdout.log').write_text(result.stdout)
    (output_path / 'stderr.log').write_text(result.stderr)
    record = {'schema_version': 1, 'kind': 'binding-gate-guard-execution/v1',
              'command': command, 'cwd': 'scripts/quality', 'started_at': started,
              'finished_at': now(), 'exit_code': result.returncode,
              'source_tree_before_sha256': tree_sha(before),
              'source_tree_after_sha256': tree_sha(source_files(Path(root))),
              'stdout': ref(root, output + '/stdout.log'),
              'stderr': ref(root, output + '/stderr.log')}
    (output_path / 'execution.json').write_text(json.dumps(record, indent=2) + '\n')
    inspect_guard_run(root, ref(root, output + '/execution.json'))
    return record


def inspect_guard_run(root, reference):
    record = load(checked_ref(root, reference, ignored=True))
    expected = [sys.executable, '-m', 'unittest', '-v', *GUARD_MODULES]
    if (record.get('kind') != 'binding-gate-guard-execution/v1'
            or type(record.get('exit_code')) is not int or record['exit_code'] != 0
            or record.get('command') != expected or record.get('cwd') != 'scripts/quality'
            or record.get('source_tree_before_sha256') != record.get('source_tree_after_sha256')):
        raise ValueError('Actual exact guard command must pass on one source epoch')
    output = checked_ref(root, record['stdout'], ignored=True).read_text() + '\n' + checked_ref(root, record['stderr'], ignored=True).read_text()
    count = re.findall(r'Ran (\d+) tests?\b', output)
    if (not count or int(count[-1]) < 1 or not re.search(r'^OK\s*$', output, re.M)
            or re.search(r'skipped=|expected failures=|^FAILED\b', output, re.M)):
        raise ValueError('Real non-skipped guard assertions did not all pass')
    if datetime.fromisoformat(record['finished_at']) < datetime.fromisoformat(record['started_at']):
        raise ValueError('Guard command interval is invalid')
    return record


def fresh_directory(root, relative):
    if not isinstance(relative, str) or not relative.startswith('.build/'):
        raise ValueError('Fresh ignored output directory required')
    p = path_at(root, relative)
    p.mkdir(parents=True, exist_ok=False)
    return p


def prepare_gate_packet(root, output, red_execution, guard_execution):
    policy, plan, current = preimplementation(root)
    # Import lazily: tests of phase identity do not execute a browser.
    from binding_journey_inspect import inspect_execution
    red_file = checked_ref(root, red_execution, ignored=True)
    red = inspect_execution(root, red_file.parent, require_current=True)
    if not red.get('eligible_semantic_red'):
        raise ValueError('A real production-component semantic RED is required; infrastructure failure is not RED')
    guards = inspect_guard_run(root, guard_execution)
    if guards['source_tree_before_sha256'] != tree_sha(current):
        raise ValueError('Guard execution source epoch differs from frozen P2')
    if red.get('source_tree_sha256') != tree_sha(current):
        raise ValueError('Semantic RED source epoch differs from frozen P2')
    output_path = fresh_directory(root, output)
    (output_path / 'source-map.json').write_text(json.dumps(current, sort_keys=True, indent=2) + '\n')
    packet = {'schema_version': 1, 'kind': 'binding-gate-phase-packet/v1',
              'change_id': CHANGE, 'phase': 'gates', 'prepared_at': now(),
              'status': 'pending_independent_review', 'authors': policy['authors'],
              'phase_policy': ref(root, POLICY),
              'parent_plan_review': ref(root, phase_receipts(policy)['plan']),
              'before_source_tree_sha256': policy['before_tree_sha256'],
              'phase2_source_tree_sha256': tree_sha(current),
              'phase2_source_map': ref(root, output + '/source-map.json'),
              'product_source_map_sha256': compact_sha(products(current)),
              'plan_basis_sha256': plan['plan_basis_sha256'],
              'gate_basis_sha256': {p: ref(root, p)['sha256'] for p in policy['gate_paths']},
              'red_execution': red_execution, 'guard_execution': guard_execution,
              'product_completion': 'incomplete', 'next_stage_allowed': False}
    if policy.get('resume_epoch') is not None:
        packet.update(epoch_id='r3', checkpoint=policy['resume_epoch']['checkpoint']['manifest'],
                      checkpoint_review=policy['resume_epoch']['checkpoint']['independent_review'])
    if source_files(Path(root)) != current:
        raise ValueError('Source changed while preparing P2 packet; discard candidate')
    (output_path / 'packet.json').write_text(json.dumps(packet, ensure_ascii=False, indent=2) + '\n')
    return packet


def verify_r3_historical_admission(root, policy, frozen_source, current_source):
    """Authorize reading historical P2 bytes, never running them on new code."""
    receipts = phase_receipts(policy)
    if policy.get('resume_epoch') is None:
        raise ValueError('Historical workspace-dependent replay exception is r3 only')
    if not delta(frozen_source, current_source) <= set(policy['implementation_paths']):
        raise ValueError('Historical P2 proof cannot hide frozen gate/plan or out-of-scope changes')
    admission_ref = ref(root, receipts['implementation_admission'])
    admission = load(checked_ref(root, admission_ref, ignored=True))
    review_ref = ref(root, receipts['gates'])
    review = load(checked_ref(root, review_ref, ignored=True))
    independent(review, policy, 'independent-binding-gate-review/v1', 'gates')
    packet = load(checked_ref(root, review['packet'], ignored=True))
    source = source_map(load(checked_ref(root, packet['phase2_source_map'], ignored=True)))
    plan_ref = ref(root, receipts['plan'])
    if (plan_ref['sha256'] != PLAN_RESUMPTION_REVIEW_SHA256
            or admission.get('kind') != 'binding-implementation-resume-admission/v1'
            or type(admission.get('schema_version')) is not int or admission['schema_version'] != 1
            or admission.get('change_id') != CHANGE or admission.get('epoch_id') != 'r3'
            or admission.get('implementation_admitted') is not True
            or admission.get('phase2_source_tree_sha256') != tree_sha(frozen_source) or source != frozen_source
            or admission.get('phase2_source_map') != packet['phase2_source_map']
            or admission.get('gate_review') != review_ref or admission.get('plan_review') != plan_ref
            or admission.get('phase_policy') != ref(root, POLICY)
            or admission.get('checkpoint') != policy['resume_epoch']['checkpoint']['manifest']
            or admission.get('checkpoint_review') != policy['resume_epoch']['checkpoint']['independent_review']
            or admission.get('parent_admission') != policy['resume_epoch']['ancestors']['original_implementation']['admission']
            or admission.get('product_completion') != 'incomplete' or admission.get('next_stage_allowed') is not False
            or review.get('epoch_id') != 'r3' or review.get('phase_source_tree_sha256') != tree_sha(frozen_source)
            or packet.get('epoch_id') != 'r3' or packet.get('phase2_source_tree_sha256') != tree_sha(frozen_source)):
        raise ValueError('Actual exclusive r3 admission and approved frozen P2 source are required for historical read')
    for group, field in [('plan_paths','plan_basis_sha256'),('gate_paths','gate_basis_sha256')]:
        basis = {path: frozen_source[path] for path in policy[group]}
        if (packet.get(field) != basis or admission.get(field) != basis
                or any(current_source.get(path) != digest for path, digest in basis.items())):
            raise ValueError('Historical workspace exception cannot change a frozen plan/gate byte')
    return admission_ref


def verify_gate_review(root, *, require_current=False):
    policy, _ = policy_at(root)
    _, p1_map = verify_plan(root, policy)
    receipts = phase_receipts(policy)
    review = load(path_at(root, receipts['gates']))
    independent(review, policy, 'independent-binding-gate-review/v1', 'gates')
    packet = load(checked_ref(root, review['packet'], ignored=True))
    if (packet.get('kind') != 'binding-gate-phase-packet/v1'
            or packet.get('change_id') != CHANGE or packet.get('phase') != 'gates'
            or packet.get('phase_policy') != ref(root, POLICY)
            or packet.get('parent_plan_review') != ref(root, receipts['plan'])
            or packet.get('authors') != policy['authors']
            or review.get('phase_source_tree_sha256') != packet.get('phase2_source_tree_sha256')):
        raise ValueError('Reviewed P2 packet/policy/source identity differs')
    source = source_map(load(checked_ref(root, packet['phase2_source_map'], ignored=True)))
    if (tree_sha(source) != packet['phase2_source_tree_sha256']
            or products(source) != products(p1_map)
            or compact_sha(products(source)) != packet.get('product_source_map_sha256')):
        raise ValueError('P2 complete source map or unchanged product proof differs')
    if policy.get('resume_epoch') is not None and (
            packet.get('epoch_id') != 'r3'
            or packet.get('checkpoint') != policy['resume_epoch']['checkpoint']['manifest']
            or packet.get('checkpoint_review') != policy['resume_epoch']['checkpoint']['independent_review']
            or review.get('epoch_id') != 'r3'):
        raise ValueError('New P2 must bind the exact independently sealed predecessor epoch')
    if not delta(p1_map, source) <= set(policy['gate_paths']):
        raise ValueError('P2 packet contains product or non-gate changes')
    for group, field in [('plan_paths', 'plan_basis_sha256'), ('gate_paths', 'gate_basis_sha256')]:
        if packet.get(field) != {p: ref(root, p)['sha256'] for p in policy[group]}:
            raise ValueError('Frozen ' + field + ' changed after P2 approval')
        if any(source.get(p) != h for p, h in packet[field].items()):
            raise ValueError('P2 frozen basis does not belong to reviewed source map')
    # Reinspect the original RED against its reviewed historical source epoch,
    # not against later P3 product code.
    from binding_journey_inspect import inspect_execution
    red_path = checked_ref(root, packet['red_execution'], ignored=True)
    red = inspect_execution(root, red_path.parent, require_current=False,
                            expected_source_tree=packet['phase2_source_tree_sha256'])
    guards = inspect_guard_run(root, packet['guard_execution'])
    if (not red.get('eligible_semantic_red')
            or guards['source_tree_before_sha256'] != packet['phase2_source_tree_sha256']):
        raise ValueError('Reviewed P2 does not retain actual semantic RED and passing guards')
    if require_current and source_files(Path(root)) != source:
        raise ValueError('Implementation write admission must precede all product changes')
    return policy, review, packet, source


def admit_implementation(root):
    policy, review, packet, current = verify_gate_review(root, require_current=True)
    receipts = phase_receipts(policy)
    relative = receipts['implementation_admission']
    p = path_at(root, relative)
    # Exclusive directory creation prevents re-signing an old admission in place.
    p.parent.mkdir(parents=True, exist_ok=False)
    body = {'schema_version': 1, 'kind': 'binding-implementation-write-admission/v1',
            'change_id': CHANGE, 'admitted_at': now(), 'implementation_admitted': True,
            'phase2_source_tree_sha256': tree_sha(current),
            'phase2_source_map': packet['phase2_source_map'],
            'gate_review': ref(root, receipts['gates']),
            'phase_policy': ref(root, POLICY),
            'plan_basis_sha256': packet['plan_basis_sha256'],
            'gate_basis_sha256': packet['gate_basis_sha256'],
            'product_completion': 'incomplete', 'next_stage_allowed': False}
    if policy.get('resume_epoch') is not None:
        body.update(kind='binding-implementation-resume-admission/v1', epoch_id='r3',
                    checkpoint=policy['resume_epoch']['checkpoint']['manifest'],
                    checkpoint_review=policy['resume_epoch']['checkpoint']['independent_review'],
                    parent_admission=policy['resume_epoch']['ancestors']['original_implementation']['admission'],
                    plan_review=ref(root, receipts['plan']))
    if source_files(Path(root)) != current:
        raise ValueError('Source changed before exclusive write-admission commit')
    with p.open('x') as output:
        json.dump(body, output, ensure_ascii=False, indent=2)
        output.write('\n')
    return body


def validate_phase(root, change_id=CHANGE):
    result = {'schema_version': 1, 'kind': 'binding-phase-admission-check/v1',
              'change_id': change_id, 'status': 'passed', 'required': change_id == CHANGE,
              'phase': None, 'errors': [], 'implementation_admitted': False,
              'product_completion': 'incomplete', 'next_stage_allowed': False}
    if change_id != CHANGE:
        return result
    try:
        policy, _ = policy_at(root)
        current = source_files(Path(root))
        receipts = phase_receipts(policy)
        admission_path = path_at(root, receipts['implementation_admission'])
        if not admission_path.exists():
            plan, p1_map = verify_plan(root, policy)
            changes = delta(p1_map, current)
            if (not changes <= set(policy['gate_paths'])
                    or products(current) != products(p1_map)):
                raise ValueError('P2 changed non-gate paths before write admission: ' + ', '.join(sorted(changes - set(policy['gate_paths']))))
            result['phase'] = 'gates' if delta(p1_map, current) else 'plan'
        else:
            policy, review, packet, p2_map = verify_gate_review(root)
            admission = load(admission_path)
            expected_kind = ('binding-implementation-resume-admission/v1' if policy.get('resume_epoch') is not None
                             else 'binding-implementation-write-admission/v1')
            if (admission.get('kind') != expected_kind
                    or type(admission.get('schema_version')) is not int or admission['schema_version'] != 1
                    or admission.get('change_id') != CHANGE
                    or admission.get('implementation_admitted') is not True
                    or admission.get('phase2_source_tree_sha256') != tree_sha(p2_map)
                    or admission.get('phase2_source_map') != packet['phase2_source_map']
                    or admission.get('gate_review') != ref(root, receipts['gates'])
                    or admission.get('phase_policy') != ref(root, POLICY)
                    or admission.get('plan_basis_sha256') != packet['plan_basis_sha256']
                    or admission.get('gate_basis_sha256') != packet['gate_basis_sha256']
                    or admission.get('product_completion') != 'incomplete'
                    or admission.get('next_stage_allowed') is not False):
                raise ValueError('Missing or stale exact P3 write-before admission')
            if policy.get('resume_epoch') is not None and (
                    admission.get('epoch_id') != 'r3'
                    or admission.get('checkpoint') != policy['resume_epoch']['checkpoint']['manifest']
                    or admission.get('checkpoint_review') != policy['resume_epoch']['checkpoint']['independent_review']
                    or admission.get('parent_admission') != policy['resume_epoch']['ancestors']['original_implementation']['admission']
                    or admission.get('plan_review') != ref(root, receipts['plan'])):
                raise ValueError('Fresh resume write admission cannot inherit an old epoch')
            unexpected = delta(p2_map, current) - set(policy['implementation_paths'])
            if unexpected:
                raise ValueError('P3 changed frozen plan/gates or unadmitted source: ' + ', '.join(sorted(unexpected)))
            result.update(phase='implementation', implementation_admitted=True)
        result['source_tree_sha256'] = tree_sha(current)
        result['plan_review'] = ref(root, receipts['plan'])
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        result.update(status='failed', errors=[str(error)])
    return result


# A successor is a separately reviewed QA maintenance epoch, never a renamed
# invocation of validate_phase(required=False). Original phase logic stays above.
SUCCESSOR_CHANGE = 'register-binding-impact-surface-mappings'
SUCCESSOR_PREFIX = 'openspec/changes/' + SUCCESSOR_CHANGE
SUCCESSOR_REVIEW = {'path': '.build/binding-engineering-integration-20261110/canonical-plan-review.json',
                    'sha256': 'a0bc4d9d146ae32056f062c81b8cb7e65b35fcbbfe6962482d75ea05de36d1ba'}
SUCCESSOR_AUTHORIZATION = {'path': '.build/binding-engineering-integration-20261110/qa-authorization.json',
                           'sha256': 'b11c32d7c032667c61d7faf3d0c774c14623861a6a133b51cee5d72b89d03113'}
SUCCESSOR_SENSITIVE_PATHS = frozenset({
    'scripts/quality/verify.py', 'scripts/quality/binding_phase_guard.py',
    'scripts/quality/test_binding_phase_guard.py', 'scripts/quality/binding_readiness_assessment.py',
    'scripts/quality/test_binding_readiness_assessment.py', 'scripts/quality/storage_schema_v5.py',
    'scripts/quality/test_storage_schema_v5.py', 'scripts/quality/all_page_design_coverage.py',
    'scripts/quality/test_all_page_design_coverage.py', 'scripts/quality/test_lifecycle_impact.py',
    'docs/development/lifecycle-impact-contract.json'})
SUCCESSOR_ADOPTION = '.build/binding-engineering-integration-20261110/adoption-r94'
R93_BASELINE_SHA256 = 'e74521a539d215422ce67f36c2032ee9c82833541c11aad190248791612e04f1'


def successor_checked_ref(root, value):
    """Strict safe immutable ref; bytes are additionally checked when declared."""
    if (not isinstance(value, dict) or set(value) not in ({'path', 'sha256'}, {'path', 'sha256', 'bytes'})
            or not isinstance(value.get('sha256'), str) or not SHA.fullmatch(value['sha256'])
            or 'bytes' in value and (type(value['bytes']) is not int or value['bytes'] < 0)):
        raise ValueError('Exact successor path/SHA/optional byte-size reference required')
    path = path_at(root, value['path'])
    if not path.is_file() or 'bytes' in value and path.stat().st_size != value['bytes']:
        raise ValueError('Missing or resized successor artifact: ' + value['path'])
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    if digest.hexdigest() != value['sha256']:
        raise ValueError('Successor artifact SHA differs: ' + value['path'])
    return path


def check_successor_retained_refs(root, refs, dispositions):
    ordinary = {}
    for row in dispositions:
        original, snapshot = row['original_ref'], row['snapshot']
        path_at(root, original['path'])  # Even exempt origins may not escape/symlink.
        if (row.get('classification') != 'ordinary_context_summary_historical_only'
                or row.get('future_original_path_hash_required') is not False
                or row.get('current_authority_granted') is not False
                or original['sha256'] != snapshot['sha256'] or original['bytes'] != snapshot['bytes']
                or original['path'] in ordinary):
            raise ValueError('Exact reviewed ordinary historical-only disposition required')
        successor_checked_ref(root, snapshot)
        ordinary[original['path']] = original
    seen = set()
    for value in refs:
        if value['path'] in seen:
            raise ValueError('Duplicate retained artifact identity')
        seen.add(value['path'])
        if value['path'] in ordinary:
            if value != ordinary[value['path']]:
                raise ValueError('Ordinary disposition is not the original exact indexed identity')
        else:
            successor_checked_ref(root, value)
    if not set(ordinary) <= seen:
        raise ValueError('Ordinary exemptions are outside the exact retained index')


def successor_handoff_required(root, change_id, changed_paths):
    if change_id == SUCCESSOR_CHANGE:
        return True
    touched = set(changed_paths)
    if touched & (SUCCESSOR_SENSITIVE_PATHS | set(R2_CATALOGUE_PATHS)) or any(p.startswith(SUCCESSOR_PREFIX + '/') for p in touched):
        return True
    # The original frozen plan/gate closure is an additional actual-impact
    # trigger. It does not call the original current-source phase validator.
    policy_path = path_at(root, POLICY)
    if policy_path.is_file():
        policy = load(policy_path)
        return bool(touched & set(policy['plan_paths'] + policy['gate_paths']))
    return False


def successor_json_leaves(value, pointer=''):
    """RFC6901 leaves, retaining empty containers and JSON type identity."""
    if isinstance(value, dict) and value:
        result = {}
        for key, child in value.items():
            escaped = key.replace('~', '~0').replace('/', '~1')
            result.update(successor_json_leaves(child, pointer + '/' + escaped))
        return result
    if isinstance(value, list) and value:
        result = {}
        for index, child in enumerate(value):
            result.update(successor_json_leaves(child, pointer + '/' + str(index)))
        return result
    return {pointer: value}


def successor_pin_proof_errors(root, manifest):
    try:
        change = load(successor_checked_ref(root, manifest['pin_delta']))
        untouched = load(successor_checked_ref(root, manifest['untouched_pins']))
        if (set(change) != {'algorithm', 'before_baseline_ref', 'after_baseline_ref', 'rows'}
                or set(untouched) != set(change) or change['algorithm'] != 'json-leaves/v1'
                or untouched['algorithm'] != 'json-leaves/v1'
                or change['before_baseline_ref'] != untouched['before_baseline_ref']
                or change['after_baseline_ref'] != untouched['after_baseline_ref']):
            raise ValueError('Exact typed JSON-leaf delta and untouched pin proof required')
        before_ref, after_ref = change['before_baseline_ref'], change['after_baseline_ref']
        active = next(row for row in manifest['outputs'] if row['path'] == 'docs/development/iteration-baseline.json')
        historical = next(row for row in manifest['outputs'] if row['path'] == 'docs/development/baselines/FP1-20261109-R94.json')
        if (before_ref['sha256'] != R93_BASELINE_SHA256 or after_ref['sha256'] != active['sha256']
                or active['sha256'] != historical['sha256'] or not after_ref['path'].startswith('.build/')):
            raise ValueError('Delta must compare actual immutable R93 with the exact reviewed R94 candidate bytes')
        before = successor_json_leaves(load(successor_checked_ref(root, before_ref)))
        after = successor_json_leaves(load(successor_checked_ref(root, after_ref)))
        def same(a, b):
            return json.dumps(a, sort_keys=True, separators=(',', ':')) == json.dumps(b, sort_keys=True, separators=(',', ':'))
        expected = [{'path': key, 'before_exists': key in before, 'before': before.get(key),
                     'after_exists': key in after, 'after': after.get(key)}
                    for key in sorted(set(before) | set(after))
                    if key not in before or key not in after or not same(before[key], after[key])]
        retained = {key: before[key] for key in sorted(set(before) & set(after)) if same(before[key], after[key])}
        if not same(change['rows'], expected) or not same(untouched['rows'], retained):
            raise ValueError('Pin delta/untouched rows do not equal independent typed baseline leaf comparison')
        return []
    except (OSError, ValueError, KeyError, TypeError, AttributeError, StopIteration) as error:
        return ['Independent pin proof rejected: ' + str(error)]


def successor_adoption_errors(root, review, current):
    """R94 may change exactly three outputs only after a reviewed exact packet.

    Manifest/review schema is deliberately explicit so a path allowlist alone
    cannot authorize a baseline refresh. Root's actual receipt remains separate.
    """
    try:
        base = SUCCESSOR_ADOPTION
        manifest_ref, review_ref = ref(root, base + '/packet/manifest.json'), ref(root, base + '/packet-review.json')
        manifest, adoption_review = load(successor_checked_ref(root, manifest_ref)), load(successor_checked_ref(root, review_ref))
        receipt = load(path_at(root, base + '/transaction/receipt.json'))
        expected_paths = set(review['deferred_independent_adoption_paths'])
        outputs = manifest['outputs']
        if (manifest.get('schema_version') != 1 or manifest.get('kind') != 'binding-engineering-exact-adoption-packet/v1'
                or manifest.get('change_id') != SUCCESSOR_CHANGE or manifest.get('authors') != review['author_ids']
                or manifest.get('parent_baseline') != {'id': 'FP1-20261109-R93', 'sha256': R93_BASELINE_SHA256}
                or len(outputs) != 3 or {row['path'] for row in outputs} != expected_paths):
            raise ValueError('Exact three-output R93-to-R94 packet required')
        before = source_map(load(successor_checked_ref(root, manifest['input_source_map'])))
        after = source_map(load(successor_checked_ref(root, manifest['after_source_map'])))
        if (manifest.get('input_source_tree_sha256') != tree_sha(before) or manifest.get('input_source_entries') != len(before)
                or manifest.get('after_source_tree_sha256') != tree_sha(after) or manifest.get('after_source_entries') != len(after)
                or after != current or products(before) != products(after)
                or delta(before, after) != expected_paths or any(after.get(row['path']) != row['sha256'] for row in outputs)):
            raise ValueError('Adoption input/after maps do not describe the actual exact output transaction')
        for row in outputs:
            successor_checked_ref(root, row)
        for name in ('prepare', 'generator', 'pin_delta', 'untouched_pins', 'source_review'):
            successor_checked_ref(root, manifest[name])
        pin_errors = successor_pin_proof_errors(root, manifest)
        if pin_errors:
            raise ValueError('; '.join(pin_errors))
        input_review = load(successor_checked_ref(root, manifest['source_review']))
        if (input_review.get('schema_version') != 1
                or input_review.get('kind') != 'independent-binding-engineering-source-review/v1'
                or input_review.get('status') != 'approved' or input_review.get('independent_non_author_review') is not True
                or input_review.get('author_ids') != review['author_ids']
                or not input_review.get('reviewer_id') or input_review['reviewer_id'] in review['author_ids']
                or input_review.get('source_tree_sha256') != tree_sha(before)
                or input_review.get('source_entries') != len(before)
                or source_map(load(successor_checked_ref(root, input_review['source_map']))) != before
                or input_review.get('product_source_delta') != []
                or input_review.get('product_acceptance_promoted') is not False
                or input_review.get('runtime_package_modified') is not False
                or input_review.get('product_completion') != 'incomplete'):
            raise ValueError('Independent actual input-source approval cannot be replaced by an after projection')
        if (adoption_review.get('schema_version') != 1
                or adoption_review.get('kind') != 'independent-binding-engineering-adoption-packet-review/v1'
                or adoption_review.get('status') != 'approved' or adoption_review.get('independent_non_author_review') is not True
                or adoption_review.get('author_ids') != review['author_ids']
                or adoption_review.get('reviewer_id') in review['author_ids'] or not adoption_review.get('reviewer_id')
                or adoption_review.get('manifest') != manifest_ref
                or any(adoption_review.get(key) != manifest[key] for key in (
                    'prepare', 'generator', 'input_source_tree_sha256', 'after_source_tree_sha256', 'pin_delta', 'outputs'))):
            raise ValueError('Independent exact packet/prepare/generator/pin-delta/output approval required')
        independent = receipt.get('independent_review') or {}
        if (receipt.get('schema_version') != 1 or receipt.get('status') != 'adopted'
                or receipt.get('source_tree_sha256') != tree_sha(after) or receipt.get('source_entries') != len(after)
                or receipt.get('outputs') != outputs or receipt.get('product_acceptance_promoted') is not False
                or receipt.get('runtime_package_modified') is not False
                or independent.get('review_path') != review_ref['path'] or independent.get('review_sha256') != review_ref['sha256']
                or independent.get('manifest_sha256') != manifest_ref['sha256']
                or independent.get('reviewer_id') != adoption_review['reviewer_id']):
            raise ValueError('Actual exclusive adopted transaction does not match approved exact R94 packet')
        return []
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        return ['Independent R94 adoption rejected: ' + str(error)]


def successor_scope_errors(root, review, parent, canonical, current):
    errors = []
    if products(current) != products(parent):
        errors.append('Successor QA maintenance requires exact zero product source delta')
    basis = review['canonical_plan_basis_sha256']
    if any(current.get(path) != digest for path, digest in basis.items()):
        errors.append('Approved seven canonical plan files are frozen')
    changed = delta(canonical, current)
    ordinary_qa = set(review['qa_code_and_doc_phase_paths'])
    deferred = set(review['deferred_independent_adoption_paths'])
    if changed - ordinary_qa - deferred:
        errors.append('Current source changed outside exact reviewed QA/doc scope: ' + ', '.join(sorted(changed - ordinary_qa - deferred)))
    if changed & deferred:
        errors += successor_adoption_errors(root, review, current)
    return errors


def _successor_parent_guard(root, seal, parent):
    execution_path = successor_checked_ref(root, seal['actual_old_guard'])
    actual = load(execution_path)
    stdout_path = successor_checked_ref(root, seal['actual_old_guard_raw_stdout'])
    stdout, stderr_path = stdout_path.read_bytes(), execution_path.parent / 'stderr.log'
    value = json.loads(stdout, object_pairs_hook=unique)
    if (actual.get('kind') != 'actual-command-execution/v1'
            or actual.get('argv') != ['python3', 'scripts/quality/binding_phase_guard.py', '--check', '--change', CHANGE, '--json']
            or actual.get('cwd') != str(Path(root).resolve())
            or type(actual.get('pid')) is not int or actual['pid'] < 1 or actual.get('exit_code') != 0
            or actual.get('before_tree_sha256') != tree_sha(parent) or actual.get('after_tree_sha256') != tree_sha(parent)
            or actual.get('before_count') != len(parent) or actual.get('after_count') != len(parent)
            or actual.get('source_stable') is not True or actual.get('stdout_sha256') != sha(stdout)
            or actual.get('stderr_sha256') != sha(stderr_path.read_bytes())
            or datetime.fromisoformat(actual['started_at']) > datetime.fromisoformat(actual['finished_at'])
            or load(execution_path.parent / 'before.json') != parent or load(execution_path.parent / 'after.json') != parent
            or value.get('kind') != 'binding-phase-admission-check/v1' or value.get('required') is not True
            or value.get('implementation_admitted') is not True or value.get('status') != 'passed'
            or value.get('errors') != [] or value.get('change_id') != CHANGE
            or value.get('source_tree_sha256') != tree_sha(parent)
            or value.get('product_completion') != 'incomplete' or value.get('next_stage_allowed') is not False):
        raise ValueError('Actual original old guard must pass against the complete real adopted parent source map')


def _successor_parent_closure(root, review, handoff, change_id):
    seal = load(successor_checked_ref(root, review['parent_seal']))
    parent_review = load(successor_checked_ref(root, review['parent_seal_review']))
    if (parent_review.get('kind') != 'independent-binding-engineering-parent-seal-review/v1'
            or parent_review.get('status') != 'approved' or parent_review.get('independent_non_author_review') is not True
            or parent_review.get('parent_seal') != review['parent_seal']
            or parent_review.get('reviewer_id') in seal['author_ids']
            or parent_review.get('ordinary_ref_dispositions') != review['ordinary_ref_dispositions']
            or parent_review.get('ordinary_disposition_scope') != review['ordinary_disposition_scope']
            or seal.get('kind') != 'binding-engineering-integration-parent-seal/v1'
            or seal.get('parent_change_id') != CHANGE or seal.get('proposed_successor_change_id') != change_id):
        raise ValueError('Original exact ParentSeal and independent non-author disposition required')
    parent = source_map(load(successor_checked_ref(root, seal['source_map'])))
    snapshot = load(successor_checked_ref(root, seal['source_snapshot']))
    canonical = source_map(load(successor_checked_ref(root, review['source_map'])))
    if (seal['source_tree_sha256'] != tree_sha(parent) or seal['source_entries'] != len(parent)
            or snapshot['files'] != parent or review['parent_source_tree_sha256'] != tree_sha(parent)
            or review['source_tree_sha256'] != tree_sha(canonical) or review['source_entries'] != len(canonical)
            or products(parent) != load(successor_checked_ref(root, seal['product_map']))
            or len(products(parent)) != seal['product_source_count']
            or compact_sha(products(parent)) != seal['product_map_canonical_sha256']
            or load(path_at(root, SUCCESSOR_PREFIX + '/source-snapshot.json'))['files'] != parent):
        raise ValueError('Actual full parent/canonical/product source maps differ; projection is not a Base')
    current = source_files(root)
    for path, digest in review['canonical_plan_basis_sha256'].items():
        if canonical.get(path) != digest or ref(root, path)['sha256'] != digest:
            raise ValueError('Canonical source review basis changed: ' + path)
    for value in handoff['approved_plan_basis'].values():
        successor_checked_ref(root, value)
    for key in ('actual_adoption', 'actual_adoption_execution', 'post_adoption_non_author_review',
                'actual_pin_delta', 'unchanged_parent_pins', 'approved_design_map',
                'adopted_R93_source_review', 'Native_source_controlled_units_review',
                'successor_plan02_review', 'seal_checks', 'schema', 'generator'):
        successor_checked_ref(root, seal[key])
    adoption = load(successor_checked_ref(root, seal['actual_adoption']))
    if (adoption.get('status') != 'adopted' or adoption.get('source_tree_sha256') != tree_sha(parent)
            or adoption.get('source_entries') != len(parent) or adoption.get('product_acceptance_promoted') is not False
            or len(adoption.get('outputs', [])) != 8
            or any(parent.get(row['path']) != row['sha256'] for row in adoption['outputs'])):
        raise ValueError('Actual R93 adopted transaction and eight outputs required')
    archives = load(successor_checked_ref(root, seal['authority_archives']))
    if {key: len(value) for key, value in archives.items()} != {'plan': 23, 'gates': 16, 'approved_design': 74, 'adopted_outputs': 8}:
        raise ValueError('Original plan/gate/design/adopted closure inventory changed')
    for rows in archives.values():
        for row in rows:
            path_at(root, row['original_path'])
            successor_checked_ref(root, row['archive'])
            if (row['archive']['sha256'] != row['sha256'] or row['archive']['bytes'] != row['bytes']
                    or parent.get(row['original_path']) != row['sha256']):
                raise ValueError('Archived original authority/source identity differs')
    for value in load(successor_checked_ref(root, seal['direct_parent_chain'])).values():
        successor_checked_ref(root, value)
    historical = load(successor_checked_ref(root, seal['historical_boundaries']))
    if (historical.get('origin_fields_followed') is not False
            or historical.get('raw_aliases_accepted_as_current_authority') is not False
            or historical.get('quarantined_declared_claims_promoted') is not False):
        raise ValueError('Historical alias/quarantine boundaries may not become current authority')
    for value in historical['documents']:
        successor_checked_ref(root, value)  # Never recursively follow origin/alias fields.
    failed = load(successor_checked_ref(root, seal['original_failed_refs']))
    retained = load(successor_checked_ref(root, seal['current_execution_failure_refs']))
    if failed['count'] != len(failed['refs']) or retained['count'] != len(retained['refs']):
        raise ValueError('Original failure/retained inventory changed')
    check_successor_retained_refs(root, failed['refs'], [])
    check_successor_retained_refs(root, retained['refs'], parent_review['ordinary_ref_dispositions'])
    _successor_parent_guard(root, seal, parent)
    return parent, canonical, current


def validate_successor_handoff(root=ROOT, change_id=SUCCESSOR_CHANGE):
    result = {'schema_version': 1, 'kind': 'binding-successor-handoff-check/v1',
              'change_id': change_id, 'required': True, 'status': 'failed', 'errors': [],
              'qa_maintenance_admitted': False, 'implementation_admitted': False,
              'product_write_allowed': False, 'product_completion': 'incomplete', 'next_stage_allowed': False}
    try:
        root = Path(root).resolve()
        if change_id != SUCCESSOR_CHANGE:
            raise ValueError('A renamed change cannot inherit the approved exact successor scope')
        review = load(successor_checked_ref(root, SUCCESSOR_REVIEW))
        authorization = load(successor_checked_ref(root, SUCCESSOR_AUTHORIZATION))
        if (review.get('schema_version') != 1 or review.get('kind') != 'binding-successor-canonical-plan-review/v1'
                or review.get('status') != 'approved' or review.get('independent_non_author_review') is not True
                or review.get('change_id') != change_id or review.get('parent_change_id') != CHANGE
                or not review.get('reviewer_id') or review['reviewer_id'] in review['author_ids']
                or review.get('approval_scope') != 'exact_29_path_QA_engineering_maintenance_only'
                or review.get('canonical_plan_mutation_allowed_after_this_review') is not False
                or review.get('product_write_allowed') is not False
                or authorization.get('kind') != 'root-explicit-qa-maintenance-authorization/v1'
                or authorization.get('authorizer') != '/root' or authorization.get('change_id') != change_id
                or authorization.get('canonical_plan_review') != SUCCESSOR_REVIEW
                or authorization.get('source_tree_at_authorization') != review['source_tree_sha256']
                or authorization.get('allowed_paths') != review['allowed_paths']
                or authorization.get('path_responsibilities') != review['path_responsibilities']
                or authorization.get('product_write_allowed') is not False):
            raise ValueError('Pinned independent canonical plan and explicit Root QA-only authorization required')
        metadata = load(path_at(root, SUCCESSOR_PREFIX + '/verification.json'))
        handoff = metadata['successor_handoff']
        if (metadata['allowed_paths'] != review['allowed_paths']
                or metadata['review_evidence']['authors'] != review['author_ids']
                or metadata['path_responsibilities'] != review['path_responsibilities']
                or handoff.get('kind') != 'binding-successor-handoff-request/v1'
                or handoff['canonical_plan_review_path'] != SUCCESSOR_REVIEW['path']
                or handoff['parent_seal'] != review['parent_seal']
                or handoff['parent_seal_independent_review'] != review['parent_seal_review']
                or handoff['ordinary_ref_dispositions'] != review['ordinary_ref_dispositions']
                or handoff['ordinary_disposition_scope'] != review['ordinary_disposition_scope']):
            raise ValueError('Canonical handoff/scope/actors must equal the exact approved plan')
        parent, canonical, current = _successor_parent_closure(root, review, handoff, change_id)
        errors = successor_scope_errors(root, review, parent, canonical, current)
        if errors:
            raise ValueError('; '.join(errors))
        result.update(status='passed', qa_maintenance_admitted=True, source_tree_sha256=tree_sha(current),
                      parent_source_tree_sha256=tree_sha(parent), source_entries=len(current),
                      product_source_count=len(products(current)), canonical_plan_review=SUCCESSOR_REVIEW,
                      root_QA_authorization=SUCCESSOR_AUTHORIZATION)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError) as error:
        result['errors'] = [str(error)]
    return result


# Current r2 authority is explicit and code-owned. The r1 API above remains a
# historical contract; production CLI and verify never fall back to it.
R2_EPOCH = 'engineering-qa-r2'
SUCCESSOR_R2_REVIEW = {'path': '.build/binding-engineering-integration-20261110/canonical-plan-review-r2.json',
                       'sha256': '304cf613541278c3cf3e7a410f04c042b86e1f86390d3203abec6d31f4e26102'}
SUCCESSOR_R2_AUTHORIZATION = {'path': '.build/binding-engineering-integration-20261110/qa-authorization-r2.json',
                              'sha256': '5bf00ffc0c886192bba48f772a194fe18dc416f072cf8cfa2b172542e72706f7'}
R2_GATE_PATHS = ('scripts/quality/binding_phase_guard.py', 'scripts/quality/test_binding_phase_guard.py', 'scripts/quality/verify.py')
R2_ROOT_DOC_PATHS = ('docs/development/engineering-gates.md', 'docs/development/current-product-test-coverage.md',
                     'docs/development/current-client-alignment-plan.md', 'docs/README.md', 'docs/development/current-phase.json')
R2_CATALOGUE_PATHS = ('docs/architecture/implementation-status.json', 'docs/architecture/implementation-status.md',
                      'docs/architecture/engineering-modules.csv')
R2_ATOMIC_OUTPUTS = R2_CATALOGUE_PATHS + ('docs/development/iteration-baseline.json',
                    'docs/development/baselines/FP1-20261109-R94.json',
                    'docs/development/binding-engineering-integration-adoption-20261110.json')
R2_GATE_SOURCE_REVIEW = '.build/binding-engineering-integration-20261110/gate-source-review-r2.json'
R2_FINAL_INPUT_REVIEW = '.build/binding-engineering-integration-20261110/input-source-review-r2.json'
SUCCESSOR_ADOPTION_R2 = '.build/binding-engineering-integration-20261110/adoption-r94-r2'


def _same_ref(left, right):
    return (isinstance(left, dict) and isinstance(right, dict)
            and left.get('path') == right.get('path') and left.get('sha256') == right.get('sha256')
            and ('bytes' not in left or 'bytes' not in right or type(left['bytes']) is int and left['bytes'] == right['bytes']))


def _approved_review(value, authors, kind, *, schema_version=1):
    if (type(value.get('schema_version')) is not int or value.get('schema_version') != schema_version or value.get('kind') != kind or value.get('status') != 'approved'
            or value.get('independent_non_author_review') is not True or value.get('author_ids') != authors
            or not value.get('reviewer_id') or value['reviewer_id'] in authors
            or value.get('blocking_findings', []) != []):
        raise ValueError('Exact independent non-author approval required: ' + kind)


def r2_plan_contract(review, authorization, metadata, r1):
    """Separate the old signed29 namespace from current32 phase permissions."""
    authors = review['author_ids']
    _approved_review(review, authors, 'binding-successor-canonical-plan-amendment-review/v1')
    if not isinstance(metadata.get('successor_handoff'), dict) or not isinstance(metadata.get('plan_amendment'), dict):
        raise ValueError('Current r2 handoff and explicit amendment are required')
    handoff, amendment = metadata['successor_handoff'], metadata['plan_amendment']
    if (review.get('approval_scope') != 'exact_r2_canonical_plan_gate3_and_conditioned_Root_docs_only'
            or review.get('change_id') != SUCCESSOR_CHANGE or review.get('epoch_id') != R2_EPOCH
            or review.get('predecessor_epoch_id') != 'engineering-qa-r1' or r1.get('author_ids') != authors
            or len(review['allowed_paths']) != 32 or len(set(review['allowed_paths'])) != 32
            or set(review['allowed_paths']) != set(r1['allowed_paths']) | set(R2_CATALOGUE_PATHS)
            or review.get('gate_only_paths') != list(R2_GATE_PATHS)
            or review.get('Root_current_doc_eligible_paths') != list(R2_ROOT_DOC_PATHS)
            or review.get('deferred_atomic_output_paths') != list(R2_ATOMIC_OUTPUTS)
            or metadata.get('allowed_paths') != review['allowed_paths']
            or metadata.get('path_responsibilities') != review['path_responsibilities']
            or metadata.get('review_evidence', {}).get('authors') != authors):
        raise ValueError('Current exact32 scope/owners cannot inherit or expand old signed29 permissions')
    if (authorization.get('schema_version') != 2
            or authorization.get('kind') != 'root-binding-engineering-r2-gate-authorization/v1'
            or authorization.get('authorizer') != '/root' or authorization.get('change_id') != SUCCESSOR_CHANGE
            or authorization.get('epoch_id') != R2_EPOCH
            or not _same_ref(authorization.get('canonical_plan_review'), SUCCESSOR_R2_REVIEW)
            or authorization.get('source_before_sha256') != review['source_tree_sha256']
            or authorization.get('source_entries') != review['source_entries']
            or authorization.get('allowed_scope_paths') != review['allowed_paths']
            or authorization.get('immediately_allowed_paths') != list(R2_GATE_PATHS)
            or authorization.get('gate_only_paths') != list(R2_GATE_PATHS)
            or authorization.get('frozen_canonical_plan_basis') != review['canonical_plan_basis_sha256']
            or authorization.get('deferred_atomic_outputs') != list(R2_ATOMIC_OUTPUTS)
            or authorization.get('no_R1_authority_fallback') is not True
            or authorization.get('no_old_failed_packet_or_tool_reuse_for_new_outputs') is not True
            or authorization.get('catalogue_write_allowed_now') is not False
            or authorization.get('baseline_or_adoption_allowed_now') is not False
            or authorization.get('product_write_allowed') is not False
            or authorization.get('runtime_package_modified') is not False
            or authorization.get('product_completion') != 'incomplete' or authorization.get('next_stage_allowed') is not False):
        raise ValueError('Root current r2 authorization permits gate3 only, not scope32 writes')
    docs = authorization['Root_current_doc_update']
    if (not isinstance(docs.get('paths'), list) or len(docs['paths']) != len(set(docs['paths']))
            or not set(docs['paths']) <= set(R2_ROOT_DOC_PATHS) or docs.get('allowed_now') is not False
            or docs.get('gate_source_review_path') != R2_GATE_SOURCE_REVIEW):
        raise ValueError('Root current-doc subset requires the separate immutable gate-stage proof')
    expected_predecessor = {'canonical_plan_review': review['predecessor_canonical_plan_review'],
                            'Root_QA_authorization': review['predecessor_Root_QA_authorization'],
                            'implemented_source_review': review['predecessor_implemented_source_review']}
    if (handoff.get('schema_version') != 2 or handoff.get('kind') != 'binding-successor-amended-handoff-request/v2'
            or handoff.get('active_epoch') != R2_EPOCH
            or handoff.get('canonical_plan_review_path') != SUCCESSOR_R2_REVIEW['path']
            or handoff.get('Root_QA_authorization_path') != SUCCESSOR_R2_AUTHORIZATION['path']
            or not _same_ref(review['predecessor_canonical_plan_review'], SUCCESSOR_REVIEW)
            or not _same_ref(review['predecessor_Root_QA_authorization'], SUCCESSOR_AUTHORIZATION)
            or any(not _same_ref(handoff.get('predecessor_QA_authority', {}).get(k), v) for k, v in expected_predecessor.items())
            or handoff.get('reviewed_scope_paths_compact_sha256') != compact_sha(r1['allowed_paths'])
            or not _same_ref(handoff.get('implemented_QA_parent'), review['implemented_parent'])
            or handoff.get('ordinary_ref_dispositions') != review.get('ordinary_ref_dispositions')
            or handoff.get('ordinary_disposition_scope') != review.get('ordinary_disposition_scope')
            or handoff.get('ordinary_ref_dispositions') != r1.get('ordinary_ref_dispositions')
            or handoff.get('ordinary_disposition_scope') != r1.get('ordinary_disposition_scope')):
        raise ValueError('Active r2 handoff cannot omit amendment or rewrite the historical signed29 namespace')
    if (amendment.get('schema_version') != 1 or amendment.get('kind') != 'binding-engineering-plan-amendment/v1'
            or amendment.get('epoch_id') != R2_EPOCH or amendment.get('predecessor_epoch_id') != 'engineering-qa-r1'
            or not _same_ref(amendment.get('implemented_parent'), review['implemented_parent'])
            or any(not _same_ref(amendment.get('R1_authority', {}).get(k), v) for k, v in expected_predecessor.items())
            or amendment.get('scope_paths') != review['allowed_paths']
            or amendment.get('new_catalogue_paths') != list(R2_CATALOGUE_PATHS)
            or amendment.get('gate_only_paths') != list(R2_GATE_PATHS)
            or amendment.get('Root_current_doc_eligible_paths') != list(R2_ROOT_DOC_PATHS)
            or amendment.get('atomic_output_paths') != list(R2_ATOMIC_OUTPUTS)
            or amendment.get('new_adoption_namespace') != SUCCESSOR_ADOPTION_R2
            or amendment.get('packet_kind') != 'binding-engineering-exact-adoption-packet/v2'
            or amendment.get('packet_review_kind') != 'independent-binding-engineering-adoption-packet-review/v2'):
        raise ValueError('Exact reviewed r2 amendment and atomic-six protocol required')


def _r2_source_approval(root, reference, authors, kind):
    value = load(successor_checked_ref(root, reference))
    _approved_review(value, authors, kind)
    mapping = source_map(load(successor_checked_ref(root, value['source_map'])))
    if (value.get('change_id') != SUCCESSOR_CHANGE or value.get('source_tree_sha256') != tree_sha(mapping)
            or type(value.get('source_entries')) is not int or value.get('source_entries') != len(mapping) or value.get('product_source_delta') != []
            or value.get('product_acceptance_promoted') is not False or value.get('runtime_package_modified') is not False
            or value.get('product_completion') != 'incomplete' or value.get('next_stage_allowed') is not False):
        raise ValueError('Independent r2 source proof does not bind an actual complete source map')
    return value, mapping


def _r2_gate_basis(mapping):
    if any(not isinstance(mapping.get(path), str) or not SHA.fullmatch(mapping[path]) for path in R2_GATE_PATHS):
        raise ValueError('All three current gate source files must exist with exact non-null SHA identities')
    return {path: mapping[path] for path in R2_GATE_PATHS}


def _r2_gate_source_approval(root, review, canonical, current):
    reference = ref(root, R2_GATE_SOURCE_REVIEW)
    proof, mapping = _r2_source_approval(root, reference, review['author_ids'],
                                        'independent-binding-engineering-gate-source-review/v1')
    if (proof.get('epoch_id') != R2_EPOCH or not _same_ref(proof.get('canonical_plan_review'), SUCCESSOR_R2_REVIEW)
            or not _same_ref(proof.get('Root_QA_authorization'), SUCCESSOR_R2_AUTHORIZATION)
            or proof.get('gate_only_paths') != list(R2_GATE_PATHS)
            or delta(canonical, mapping) - set(R2_GATE_PATHS)
            or proof.get('gate_basis_sha256') != _r2_gate_basis(mapping)
            or _r2_gate_basis(mapping) != _r2_gate_basis(current)):
        raise ValueError('Gate-stage review is stale or already contains later Root-doc/catalogue writes')
    return proof, mapping, reference


def r2_stage_scope(root, review, authorization, canonical, current):
    result = {'phase': 'gate_only', 'errors': [], 'atomic_adoption_verified': False,
              'gate_source_review': None, 'Root_doc_changed_paths': []}
    try:
        _r2_gate_basis(canonical)
        _r2_gate_basis(current)
        changed = delta(canonical, current)
        gate_paths = set(R2_GATE_PATHS)
        docs = set(authorization['Root_current_doc_update']['paths'])
        if any(not isinstance(current.get(p), str) or not SHA.fullmatch(current[p]) for p in R2_ROOT_DOC_PATHS):
            raise ValueError('Current documentation update cannot delete the five maintained entry/contract files')
        outputs = set(R2_ATOMIC_OUTPUTS)
        if changed - gate_paths - docs - outputs:
            raise ValueError('Current r2 changed frozen prior QA/canonical or unapproved source: ' + ', '.join(sorted(changed - gate_paths - docs - outputs)))
        if products(current) != products(canonical):
            raise ValueError('Product source changed during r2 QA maintenance')
        gate_file = path_at(root, R2_GATE_SOURCE_REVIEW)
        if changed & docs or changed & outputs or gate_file.exists():
            _, _, reference = _r2_gate_source_approval(root, review, canonical, current)
            result['gate_source_review'] = reference
        if changed & docs:
            result.update(phase='Root_docs_after_gate_review', Root_doc_changed_paths=sorted(changed & docs))
        if changed & outputs:
            errors = successor_adoption_errors_v2(root, review, current)
            if errors:
                raise ValueError('; '.join(errors))
            result.update(phase='atomic_six_adopted', atomic_adoption_verified=True)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        result['errors'] = [str(error)]
    return result


def _r2_preserved_implementation(root, review, r1, parent):
    checkpoint = load(successor_checked_ref(root, review['implemented_parent']))
    implemented = source_map(load(successor_checked_ref(root, checkpoint['source_map'])))
    if (checkpoint.get('kind') != 'binding-engineering-QA-amendment-parent-observation/v1'
            or checkpoint.get('change_id') != SUCCESSOR_CHANGE or checkpoint.get('source_stable') is not True
            or checkpoint.get('source_tree_sha256') != tree_sha(implemented)
            or checkpoint.get('source_entries') != len(implemented)
            or review.get('implemented_parent_source_tree_sha256') != tree_sha(implemented)
            or review.get('implemented_parent_source_entries') != len(implemented)
            or source_map(load(successor_checked_ref(root, checkpoint['source_after']))) != implemented
            or products(implemented) != products(parent)
            or load(successor_checked_ref(root, checkpoint['product_map'])) != products(implemented)
            or checkpoint.get('product_map_compact_sha256') != compact_sha(products(implemented))):
        raise ValueError('Already implemented legal r1 source4515 must be retained as a complete actual map')
    archives = load(successor_checked_ref(root, checkpoint['archive_index']))
    groups = {}
    for row in archives:
        original = row['original_path']; path_at(root, original)
        successor_checked_ref(root, row['archive'])
        if row['archive']['sha256'] != implemented.get(original) or row['original_sha256'] != implemented.get(original):
            raise ValueError('Preserved r1 source archive differs from actual implemented parent')
        groups.setdefault(row['classification'], []).append(original)
    if (set(groups.get('implemented_QA19_source_snapshot', [])) != set(r1['qa_code_and_doc_phase_paths'])
            or set(groups.get('canonical_r1_seven_snapshot', [])) != set(r1['frozen_canonical_plan_paths'])
            or set(groups.get('R93_catalogue_before_r2_snapshot', [])) != set(R2_CATALOGUE_PATHS) | {'docs/architecture/engineering-components.csv'}):
        raise ValueError('The legal19/seven canonical/R93 catalogue preservation inventory changed')
    authority = checkpoint['authority']
    for value in authority.values():
        successor_checked_ref(root, value)
    for key, pin in [('r1_canonical_review', SUCCESSOR_REVIEW), ('r1_Root_QA_authorization', SUCCESSOR_AUTHORIZATION),
                     ('r1_input_source_review', review['predecessor_implemented_source_review'])]:
        if not _same_ref(authority[key], pin):
            raise ValueError('Amendment cannot replace already approved r1 authority')
    _, old_source = _r2_source_approval(root, authority['r1_input_source_review'], review['author_ids'],
                                      'independent-binding-engineering-source-review/v1')
    if old_source != implemented:
        raise ValueError('Old source review did not actually approve the implemented19 parent')
    old_index = load(successor_checked_ref(root, checkpoint['old_R94_authority_index']))
    if old_index['count'] != len(old_index['refs']):
        raise ValueError('Old failed R94 artifact inventory changed')
    check_successor_retained_refs(root, old_index['refs'], [])
    failed = load(successor_checked_ref(root, authority['actual_failed_adopt']))
    failed_before = load(successor_checked_ref(root, authority['actual_failed_before_map']))
    failed_after = load(successor_checked_ref(root, authority['actual_failed_after_map']))
    stderr = successor_checked_ref(root, authority['actual_failed_stderr']).read_bytes()
    packet = load(successor_checked_ref(root, authority['old_packet']))
    if (failed.get('kind') != 'actual-command-execution/v1' or type(failed.get('pid')) is not int or failed['pid'] <= 0
            or failed.get('exit_code') != 1 or failed.get('changed_paths') != []
            or failed.get('before_tree_sha256') != tree_sha(implemented) or failed.get('after_tree_sha256') != tree_sha(implemented)
            or failed_before != implemented or failed_after != implemented or failed.get('stderr_sha256') != sha(stderr)
            or packet.get('input_source_tree_sha256') != tree_sha(implemented)
            or checkpoint.get('attempted_after_was_adopted') is not False
            or checkpoint.get('attempted_after_source_tree_sha256') != packet.get('after_source_tree_sha256')):
        raise ValueError('Actual failed R94 must be a retained, completely rolled-back transaction, not a Base')
    if path_at(root, SUCCESSOR_ADOPTION + '/transaction/receipt.json').exists():
        raise ValueError('Original failed R94 namespace cannot later acquire an adoption receipt')
    r1_actual_path = successor_checked_ref(root, authority['r1_actual_successor_check'])
    r1_actual = load(r1_actual_path)
    raw = load(successor_checked_ref(root, r1_actual['stdout']))
    successor_checked_ref(root, r1_actual['stderr'])
    if (r1_actual.get('kind') != 'actual-QA-slice-command/v1'
            or r1_actual.get('command') != ['python3', 'scripts/quality/binding_phase_guard.py', '--successor-check', '--change', SUCCESSOR_CHANGE, '--json']
            or type(r1_actual.get('pid')) is not int or r1_actual['pid'] < 1 or r1_actual.get('exit_code') != 0
            or r1_actual.get('timed_out') is not False or raw.get('status') != 'passed'
            or raw.get('qa_maintenance_admitted') is not True or raw.get('source_tree_sha256') != tree_sha(implemented)
            or not _same_ref(raw.get('canonical_plan_review'), SUCCESSOR_REVIEW)):
        raise ValueError('Legal implemented r1 requires its actual original same-tree successor CLI proof')
    return checkpoint, implemented


def validate_successor_current_handoff(root=ROOT, change_id=SUCCESSOR_CHANGE):
    """Only current r2 is production authority; legacy r1 is never a fallback."""
    result = {'schema_version': 2, 'kind': 'binding-successor-current-handoff-check/v2',
              'change_id': change_id, 'epoch_id': R2_EPOCH, 'required': True,
              'status': 'failed', 'errors': [], 'qa_maintenance_admitted': False,
              'implementation_admitted': False, 'product_write_allowed': False,
              'atomic_adoption_verified': False, 'product_completion': 'incomplete', 'next_stage_allowed': False}
    try:
        root = Path(root).resolve()
        if change_id != SUCCESSOR_CHANGE:
            raise ValueError('Renamed change/legacy required=false cannot inherit current r2 authority')
        review = load(successor_checked_ref(root, SUCCESSOR_R2_REVIEW))
        authorization = load(successor_checked_ref(root, SUCCESSOR_R2_AUTHORIZATION))
        r1 = load(successor_checked_ref(root, SUCCESSOR_REVIEW))
        successor_checked_ref(root, SUCCESSOR_AUTHORIZATION)
        metadata = load(path_at(root, SUCCESSOR_PREFIX + '/verification.json'))
        r2_plan_contract(review, authorization, metadata, r1)
        parent, canonical, current = _successor_parent_closure(root, review, metadata['successor_handoff'], change_id)
        _, implemented = _r2_preserved_implementation(root, review, r1, parent)
        publication_before = source_map(load(successor_checked_ref(root, review['actual_publication_before_source_map'])))
        publication_after = source_map(load(successor_checked_ref(root, review['actual_publication_after_source_map'])))
        expected = set(review['frozen_canonical_plan_paths']) - {SUCCESSOR_PREFIX + '/source-snapshot.json'}
        if publication_before != implemented or publication_after != canonical or delta(implemented, canonical) != expected:
            raise ValueError('R2 plan publication must be exactly the actual authorized six-change/seven-basis transition')
        for key in ('amendment_protocol_review', 'Root_plan_publication_authorization', 'actual_canonical_publication'):
            successor_checked_ref(root, review[key])
        stage = r2_stage_scope(root, review, authorization, canonical, current)
        if stage['errors']:
            raise ValueError('; '.join(stage['errors']))
        result.update(status='passed', errors=[], qa_maintenance_admitted=True, phase=stage['phase'],
                      atomic_adoption_verified=stage['atomic_adoption_verified'], gate_source_review=stage['gate_source_review'],
                      Root_doc_changed_paths=stage['Root_doc_changed_paths'], source_tree_sha256=tree_sha(current),
                      source_entries=len(current), product_source_count=len(products(current)),
                      canonical_plan_review=SUCCESSOR_R2_REVIEW, Root_QA_authorization=SUCCESSOR_R2_AUTHORIZATION)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError) as error:
        result['errors'] = [str(error)]
    return result


R2_CODE_ROLES = {
    'effective_checks': ('scripts/quality/verify.py', 'effective_binding_checks'),
    'schema5_dispatch': ('scripts/quality/verify.py', 'dispatch_schema_v5_checks'),
    'binding_dispatch': ('scripts/quality/verify.py', 'dispatch_binding_checks'),
    'current_verify_routing': ('scripts/quality/verify.py', 'check_current_binding_handoff'),
    'current_cli_routing': ('scripts/quality/binding_phase_guard.py', 'main'),
    'current_handoff': ('scripts/quality/binding_phase_guard.py', 'validate_successor_current_handoff'),
    'phase_permissions': ('scripts/quality/binding_phase_guard.py', 'r2_stage_scope'),
    'six_output_transaction': ('scripts/quality/binding_phase_guard.py', 'successor_adoption_errors_v2'),
    'typed_pin_proof': ('scripts/quality/binding_phase_guard.py', 'successor_pin_proof_errors'),
    'native_missing_classification': ('scripts/quality/binding_readiness_assessment.py', 'derive_readiness'),
    'full_binding_execution': ('scripts/quality/binding_readiness_assessment.py', 'execute_assessment'),
    'original_raw_reinspection': ('scripts/quality/binding_readiness_assessment.py', 'assessment_errors'),
    'historical_store4': ('scripts/quality/storage_schema_v5.py', '_verify_old_suite'),
}


def _typed_equal(left, right):
    return json.dumps(left, sort_keys=True, separators=(',', ':'), ensure_ascii=False) == json.dumps(right, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def _evidence_identity(value):
    if not isinstance(value, dict):
        raise ValueError('Evidence reference object required')
    return {key: value[key] for key in ('path', 'sha256', 'bytes') if key in value}


def r2_catalogue_protected_fields(before, candidate, *, coverage_state_count=None):
    """Permit QA prose/evidence, not a second product audit or acceptance change."""
    if set(before) != set(candidate):
        raise ValueError('QA catalogue update cannot add/remove unrelated top-level facts')
    for key in before:
        if key not in {'modules', 'components', 'capabilities', 'audit_inputs', 'incremental_source_review'} and not _typed_equal(before[key], candidate[key]):
            raise ValueError('QA catalogue changed protected top-level fact: ' + key)
    for group in ('modules', 'components', 'capabilities'):
        old, new = before[group], candidate[group]
        if len(old) != len(new) or [row['id'] for row in old] != [row['id'] for row in new]:
            raise ValueError('All78 product responsibility identities/order must retain R93')
        for previous, current in zip(old, new):
            allowed = {'current_behavior', 'gap', 'source_evidence'} if group == 'modules' and previous['id'] == 'M15' else {'next_step'} if group == 'modules' and previous['id'] == 'M13' else set()
            if set(previous) != set(current):
                raise ValueError('Catalogue record shape changed: ' + previous['id'])
            for key in previous:
                if key not in allowed and not _typed_equal(previous[key], current[key]):
                    raise ValueError('QA update changed protected product/status/owner/history fact: ' + previous['id'] + '.' + key)
    previous_m13 = next(row for row in before['modules'] if row['id'] == 'M13')['next_step']
    current_m13 = next(row for row in candidate['modules'] if row['id'] == 'M13')['next_step']
    count_note = re.fullmatch(r'(.*?累计)([0-9]+)(项义务)（[^）]*）(.*)', previous_m13, re.DOTALL)
    if not count_note or type(coverage_state_count) is not int or coverage_state_count < int(count_note[2]):
        raise ValueError('M13 count-only edit requires the actual frozen cumulative state count')
    expected_m13 = (count_note[1] + str(coverage_state_count) + count_note[3]
                    + '（原' + count_note[2] + '项逐ID保留，另增' + str(coverage_state_count-int(count_note[2]))
                    + '项真实入口状态义务）' + count_note[4])
    if current_m13 != expected_m13:
        raise ValueError('M13 next_step may change only the reviewed cumulative-count note, never product behavior/acceptance')
    old_m15 = next(row for row in before['modules'] if row['id'] == 'M15')
    new_m15 = next(row for row in candidate['modules'] if row['id'] == 'M15')
    if old_m15['current_behavior'] == new_m15['current_behavior'] or old_m15['source_evidence'] == new_m15['source_evidence']:
        raise ValueError('Four hash refreshes alone do not register current r2/six-output semantics')


def code_source_semantics_errors(root, semantics, input_map, source_evidence):
    """Independently bind named current roles to actual complete Python functions."""
    import ast
    errors = []
    try:
        if (semantics.get('schema_version') != 1 or semantics.get('kind') != 'binding-engineering-code-source-semantics/v1'
                or semantics.get('input_source_tree_sha256') != tree_sha(input_map)
                or not isinstance(semantics.get('anchors'), list)):
            raise ValueError('Typed current code-source semantics and actual input epoch required')
        roles = set()
        cache = {}
        for anchor in semantics['anchors']:
            role, relative, symbol = anchor['role'], anchor['path'], anchor['symbol']
            if not isinstance(role, str) or not role or role in roles:
                raise ValueError('Missing or duplicate semantic source role')
            roles.add(role)
            if role in R2_CODE_ROLES and (relative, symbol) != R2_CODE_ROLES[role]:
                raise ValueError('Current semantic role points at historical/wrong function: ' + role)
            if not relative.startswith('scripts/quality/') or not relative.endswith('.py'):
                raise ValueError('QA semantic evidence must refer to actual quality source')
            path = path_at(root, relative)
            if relative not in cache:
                text = path.read_text()
                functions = {node.name: node for node in ast.parse(text).body if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))}
                cache[relative] = (text, functions)
            text, functions = cache[relative]
            node = functions[symbol]
            if (anchor.get('sha256') != input_map.get(relative) or sha(path.read_bytes()) != anchor['sha256']
                    or type(anchor.get('line')) is not int or anchor['line'] != node.lineno
                    or type(anchor.get('end_line')) is not int or anchor['end_line'] != node.end_lineno
                    or anchor.get('function_sha256') != sha(ast.get_source_segment(text, node).encode())
                    or anchor.get('quote') != text.splitlines()[node.lineno-1].strip()
                    or not isinstance(anchor.get('meaning'), str) or not anchor['meaning'].strip()):
                raise ValueError('Actual full function/range/file/line evidence differs: ' + role)
            if not any(row.get('path') == relative and row.get('line') == anchor['line']
                       and row.get('quote') == anchor['quote'] and row.get('symbol') == symbol
                       and row.get('sha256') == anchor['sha256'] for row in source_evidence):
                raise ValueError('M15 catalogue does not actually cite its current semantic function: ' + role)
        if not set(R2_CODE_ROLES) <= roles:
            raise ValueError('Current active/routing/v2 roles missing; old ten anchors are not a ceiling or substitute')
    except (OSError, ValueError, KeyError, TypeError, AttributeError, SyntaxError) as error:
        errors.append(str(error))
    return errors


def qa_catalogue_semantics_errors(root, proof_ref, manifest, input_map, review):
    """Use the original read-only catalogue helpers; never patch Main validators."""
    try:
        import implementation_status as catalogue
        proof = load(successor_checked_ref(root, proof_ref))
        _approved_review(proof, review['author_ids'], 'independent-implementation-source-audit/v1')
        if (proof.get('change_id') != SUCCESSOR_CHANGE or proof.get('epoch_id') != R2_EPOCH
                or proof.get('review_scope') != 'QA_only_retaining_R93_product_identity'
                or proof.get('product_source_delta') != [] or proof.get('product_acceptance_promoted') is not False
                or proof.get('native_acceptance_promoted') is not False or proof.get('runtime_verified_this_review') is not False
                or proof.get('runtime_package_modified') is not False or proof.get('product_completion') != 'incomplete'
                or proof.get('next_stage_allowed') is not False):
            raise ValueError('The new catalogue proof may review QA semantics only, retaining product R93')
        candidate_ref = next(row for row in manifest['outputs'] if row['path'] == catalogue.CATALOG)
        candidate = load(successor_checked_ref(root, candidate_ref))
        before_ref = manifest['protected_catalogue']
        before = load(successor_checked_ref(root, before_ref))
        if (before_ref['sha256'] != input_map.get(catalogue.CATALOG)
                or not _same_ref(proof.get('before_catalogue'), before_ref)
                or candidate.get('product_sources_sha256') != products(input_map)
                or proof.get('product_sources_sha256') != compact_sha(products(input_map))
                or proof.get('product_source_count') != len(products(input_map))
                or proof.get('record_count') != sum(len(candidate[group]) for group in ('modules', 'components', 'capabilities'))
                or proof['record_count'] != 78 or proof.get('catalog_body_sha256') != catalogue.incremental_catalog_body_sha256(candidate)):
            raise ValueError('QA body proof differs from exact R93 before catalogue/current body/product maps')
        current_proofs, previous_proofs = [], []
        for rows, selected in ((candidate['audit_inputs'], current_proofs), (before['audit_inputs'], previous_proofs)):
            for value in rows:
                identity = _evidence_identity(value)
                path = successor_checked_ref(root, identity)
                if path.suffix == '.json':
                    value_body = load(path)
                    if isinstance(value_body, dict) and value_body.get('kind') == 'independent-implementation-source-audit/v1':
                        selected.append(identity)
        if (len(current_proofs) != 1 or len(previous_proofs) != 1 or not _same_ref(current_proofs[0], proof_ref)
                or not _same_ref(proof.get('prior_R93_source_review'), previous_proofs[0])):
            raise ValueError('Exactly one current QA-only proof and exact nested R93 lineage required')
        before_other = [row for row in before['audit_inputs'] if not _same_ref(row, previous_proofs[0])]
        current_other = [row for row in candidate['audit_inputs'] if not _same_ref(row, proof_ref)]
        if not _typed_equal(before_other, current_other):
            raise ValueError('QA source audit cannot drop or rewrite other inherited audit inputs')
        supplement = candidate['incremental_source_review']
        if (supplement.get('catalog_before_sha256') != before_ref['sha256']
                or not _same_ref(supplement.get('prior_independent_source_review'), previous_proofs[0])):
            raise ValueError('Before catalogue and prior current source-review identity must remain explicit')
        m15 = next(row for row in candidate['modules'] if row['id'] == 'M15')
        semantic_errors = code_source_semantics_errors(root, proof['code_source_semantics'], input_map, m15['source_evidence'])
        if semantic_errors:
            raise ValueError('; '.join(semantic_errors))
        alignment_ref = proof['coverage_contract']
        if (alignment_ref['path'] != 'docs/development/current-design-alignment.json'
                or alignment_ref['sha256'] != input_map.get(alignment_ref['path'])):
            raise ValueError('Catalogue count wording must retain actual frozen cumulative state contract')
        alignment = load(successor_checked_ref(root, alignment_ref))
        state_ids = [row['id'] for row in alignment['states']]
        if len(state_ids) != len(set(state_ids)) or proof.get('coverage_state_count') != len(state_ids):
            raise ValueError('QA proof cumulative state count does not match the actual retained contract')
        r2_catalogue_protected_fields(before, candidate, coverage_state_count=len(state_ids))
        errors = []
        authority = catalogue._authority(root)
        catalogue._current_runtime_claims(candidate, errors)
        catalogue._current_artifact(root, candidate, errors)
        catalogue._audit_provenance(root, candidate, errors)
        for group in ('modules', 'components', 'capabilities'):
            for index, row in enumerate(candidate[group]):
                catalogue._record(root, row, group, index, authority, errors)
        if errors:
            raise ValueError('; '.join(errors))
        rendered = catalogue.render(root, candidate)
        for relative, expected in rendered.items():
            if relative in R2_CATALOGUE_PATHS:
                output = next(row for row in manifest['outputs'] if row['path'] == relative)
                if successor_checked_ref(root, output).read_text() != expected:
                    raise ValueError('Actual deterministic catalogue export differs: ' + relative)
            else:
                component = manifest['protected_components_export']
                if (relative != catalogue.COMPONENTS or component['sha256'] != input_map.get(relative)
                        or successor_checked_ref(root, component).read_text() != expected
                        or path_at(root, relative).read_text() != expected):
                    raise ValueError('ComponentsCSV must remain byte-exact, not a seventh output')
        return []
    except (OSError, ValueError, KeyError, TypeError, AttributeError, StopIteration) as error:
        return ['Current QA catalogue proof rejected: ' + str(error)]


def successor_adoption_errors_v2(root, review, current):
    """Current r2 requires one real v2 transaction of exactly SIX outputs.

    The r1 three-output reader remains historical only. This consumer preserves
    actual gate and final-input proofs separately and never grants adoption from
    a candidate status, original failed namespace or declared allowed32 paths.
    """
    try:
        base = SUCCESSOR_ADOPTION_R2
        manifest_ref = ref(root, base + '/packet/manifest.json')
        packet_review_ref = ref(root, base + '/packet-review.json')
        manifest = load(successor_checked_ref(root, manifest_ref))
        packet_review = load(successor_checked_ref(root, packet_review_ref))
        receipt = load(path_at(root, base + '/transaction/receipt.json'))
        outputs = manifest['outputs']
        if (manifest.get('schema_version') != 2 or manifest.get('kind') != 'binding-engineering-exact-adoption-packet/v2'
                or manifest.get('change_id') != SUCCESSOR_CHANGE or manifest.get('epoch_id') != R2_EPOCH
                or manifest.get('authors') != review['author_ids']
                or manifest.get('parent_baseline') != {'id': 'FP1-20261109-R93', 'sha256': R93_BASELINE_SHA256}
                or len(outputs) != 6 or [row['path'] for row in outputs] != list(R2_ATOMIC_OUTPUTS)
                or not _same_ref(manifest.get('amendment_review'), SUCCESSOR_R2_REVIEW)
                or not _same_ref(manifest.get('Root_authorization'), SUCCESSOR_R2_AUTHORIZATION)
                or not _same_ref(manifest.get('implemented_parent'), review['implemented_parent'])):
            raise ValueError('Actual v2 packet must bind the reviewed r2 chain and exactly six ordered outputs')
        before = source_map(load(successor_checked_ref(root, manifest['input_source_map'])))
        after = source_map(load(successor_checked_ref(root, manifest['after_source_map'])))
        expected_changed = {row['path'] for row in outputs if before.get(row['path']) != row['sha256']}
        if (manifest.get('input_source_tree_sha256') != tree_sha(before) or manifest.get('input_source_entries') != len(before)
                or manifest.get('after_source_tree_sha256') != tree_sha(after) or manifest.get('after_source_entries') != len(after)
                or after != current or products(before) != products(after)
                or delta(before, after) != expected_changed
                or any(after.get(row['path']) != row['sha256'] for row in outputs)):
            raise ValueError('Actual input/after maps differ from complete atomic-six candidate bytes')
        for row in outputs:
            successor_checked_ref(root, row)
        canonical = source_map(load(successor_checked_ref(root, review['source_map'])))
        root_auth = load(successor_checked_ref(root, SUCCESSOR_R2_AUTHORIZATION))
        docs = set(root_auth['Root_current_doc_update']['paths'])
        if delta(canonical, before) - set(R2_GATE_PATHS) - docs or products(before) != products(canonical):
            raise ValueError('Adoption input contains premature catalogue/metadata or unrelated source changes')
        _, gate_map, gate_ref = _r2_gate_source_approval(root, review, canonical, before)
        if not _same_ref(manifest.get('actual_gate_source_review'), gate_ref):
            raise ValueError('Actual immutable gate-stage proof is distinct from final input-source review')
        if manifest['source_review']['path'] != R2_FINAL_INPUT_REVIEW or _same_ref(manifest['source_review'], gate_ref):
            raise ValueError('Final input-source review must have its own current full-source identity')
        _, reviewed_input = _r2_source_approval(root, manifest['source_review'], review['author_ids'],
                                               'independent-binding-engineering-source-review/v1')
        if reviewed_input != before or delta(gate_map, before) - docs:
            raise ValueError('Final input proof must bind the exact post-doc freeze and retain all reviewed gate bytes')
        for name in ('prepare', 'generator', 'tool_review', 'pin_delta', 'untouched_pins', 'catalog_source_audit',
                     'protected_catalogue', 'protected_components_export'):
            successor_checked_ref(root, manifest[name])
        tool_review = load(successor_checked_ref(root, manifest['tool_review']))
        tool_authors = tool_review.get('author_ids')
        if (not isinstance(tool_authors, list) or not tool_authors or len(tool_authors) != len(set(tool_authors))
                or not set(tool_authors) <= set(review['author_ids'])):
            raise ValueError('Fresh v2 tool authors must be the actual reviewed canonical actors')
        _approved_review(tool_review, tool_authors, 'independent-binding-engineering-adoption-tool-review/v2', schema_version=2)
        if (tool_review.get('epoch_id') != R2_EPOCH or not _same_ref(tool_review.get('tool'), manifest['generator'])
                or not _same_ref(tool_review.get('canonical_plan_review'), SUCCESSOR_R2_REVIEW)
                or tool_review.get('product_acceptance_promoted') is not False
                or tool_review.get('runtime_package_modified') is not False):
            raise ValueError('Original c72 or another tool cannot approve current atomic-six writer')
        tests_path = successor_checked_ref(root, tool_review['tests'])
        if (not manifest['generator']['path'].startswith(base + '/tool/')
                or not tool_review['tests']['path'].startswith(base + '/tool/')):
            raise ValueError('Current six-output writer and actual tests must use the fresh r2 tool namespace')
        actual_tool = load(successor_checked_ref(root, tool_review['actual_execution']))
        if (actual_tool.get('kind') != 'actual-command-execution/v1'
                or type(actual_tool.get('pid')) is not int or actual_tool['pid'] <= 0
                or type(actual_tool.get('exit_code')) is not int or actual_tool.get('exit_code') != 0
                or actual_tool.get('timed_out', False) is not False
                or datetime.fromisoformat(actual_tool['started_at']) > datetime.fromisoformat(actual_tool['finished_at'])):
            raise ValueError('Fresh v2 tool guards did not actually execute successfully')
        tool_command = actual_tool.get('argv', actual_tool.get('command'))
        script_tail = ([str(tests_path)], [tool_review['tests']['path']])
        discovery_tail = (['-m', 'unittest', 'discover', '-s', str(tests_path.parent), '-p', tests_path.name],
                          ['-m', 'unittest', 'discover', '-s', tests_path.parent.relative_to(Path(root).resolve()).as_posix(), '-p', tests_path.name])
        if (not isinstance(tool_command, list) or not tool_command
                or not Path(tool_command[0]).name.startswith('python')
                or tool_command[1:] not in (*script_tail, *discovery_tail)
                or actual_tool.get('cwd') != str(Path(root).resolve())):
            raise ValueError('Actual tool guard command must invoke the exact frozen tests, not an OK-printing command')
        tool_raw = successor_checked_ref(root, actual_tool['stdout']).read_text() + successor_checked_ref(root, actual_tool['stderr']).read_text()
        from verify import validate_test_output
        actual_test_count, actual_test_errors = validate_test_output('python', tool_raw)
        if actual_test_errors or not actual_test_count or not re.search(r'(?m)^OK\s*$', tool_raw):
            raise ValueError('Actual fresh v2 tool test execution is zero, skipped or not a passing test runner')
        pin_errors = successor_pin_proof_errors(root, manifest)
        if pin_errors:
            raise ValueError('; '.join(pin_errors))
        catalogue_errors = qa_catalogue_semantics_errors(root, manifest['catalog_source_audit'], manifest, before, review)
        if catalogue_errors:
            raise ValueError('; '.join(catalogue_errors))
        _approved_review(packet_review, review['author_ids'], 'independent-binding-engineering-adoption-packet-review/v2', schema_version=2)
        bound_fields = ('prepare', 'generator', 'tool_review', 'input_source_tree_sha256', 'after_source_tree_sha256', 'pin_delta',
                        'untouched_pins', 'source_review', 'actual_gate_source_review', 'catalog_source_audit',
                        'amendment_review', 'Root_authorization', 'implemented_parent', 'protected_catalogue',
                        'protected_components_export', 'outputs')
        if (not _same_ref(packet_review.get('manifest'), manifest_ref)
                or any(not _typed_equal(packet_review.get(key), manifest[key]) for key in bound_fields)
                or packet_review.get('epoch_id') != R2_EPOCH or packet_review.get('schema_version') != 2):
            raise ValueError('Fresh independent v2 packet/QA-proof/full-pin/six-output approval required')
        independent = receipt.get('independent_review') or {}
        if (type(receipt.get('schema_version')) is not int or receipt.get('schema_version') != 1 or receipt.get('status') != 'adopted'
                or receipt.get('source_tree_sha256') != tree_sha(after) or receipt.get('source_entries') != len(after)
                or not _typed_equal(receipt.get('outputs'), outputs) or receipt.get('product_acceptance_promoted') is not False
                or receipt.get('runtime_package_modified') is not False
                or independent.get('review_path') != packet_review_ref['path'] or independent.get('review_sha256') != packet_review_ref['sha256']
                or independent.get('manifest_sha256') != manifest_ref['sha256']
                or independent.get('reviewer_id') != packet_review['reviewer_id']):
            raise ValueError('Missing/currently rolled-back/old receipt cannot authorize actual six-output adoption')
        return []
    except (OSError, ValueError, KeyError, TypeError, AttributeError, StopIteration) as error:
        return ['Current r2 six-output adoption rejected: ' + str(error)]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--check', action='store_true')
    action.add_argument('--successor-check', action='store_true')
    action.add_argument('--run-guards', action='store_true')
    action.add_argument('--prepare-gates', action='store_true')
    action.add_argument('--admit-implementation', action='store_true')
    parser.add_argument('--change', default=CHANGE)
    parser.add_argument('--output')
    parser.add_argument('--red-execution')
    parser.add_argument('--guard-execution')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        if args.successor_check:
            result = validate_successor_current_handoff(ROOT, args.change)
        elif args.check:
            result = validate_phase(ROOT, args.change)
        elif args.run_guards:
            if not args.output:
                parser.error('--run-guards requires --output')
            result = run_guard_tests(ROOT, args.output)
        elif args.prepare_gates:
            if not all((args.output, args.red_execution, args.guard_execution)):
                parser.error('--prepare-gates requires --output, --red-execution, --guard-execution')
            result = prepare_gate_packet(ROOT, args.output, ref(ROOT, args.red_execution), ref(ROOT, args.guard_execution))
        else:
            result = admit_implementation(ROOT)
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
        return 1 if result.get('status') == 'failed' else 0
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        print(json.dumps({'status': 'failed', 'errors': [str(error)],
                          'implementation_admitted': False, 'product_completion': 'incomplete',
                          'next_stage_allowed': False}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    sys.exit(main())
