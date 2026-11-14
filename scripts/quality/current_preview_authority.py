"""Effective C35 source supplement; it never grants reference or runtime approval."""
from pathlib import Path

from gate_core import digest, safe_path
from current_reference_catalogue import load
from design_authority import (CURRENT_RECORD, CURRENT_RECORD_SHA256, CURRENT_COMPOSITION_ID,
    CURRENT_DESIGN_CLOSURE_SHA256, TARGET, current_source_approval)

RECORD = 'docs/development/design-approval-preview-20261104.json'
RECORD_SHA256 = 'badc91c0a62af6947fa28974933694da94900b5ba17e43188014e198bac2e462'
PREFIX = 'docs/design/first-product-v1/candidates/preview-contract-20261104/'
SOURCE_HASHES = {
    'index.html': ('d2ea83e4f845fe410509fd927828e7ba319ec45570adaa1dbd94d535903ae18b', 8000),
    'preview-contract.css': ('c75fda32c755be7d86195eb951da3cbd64bb3e261b965e21df8fc312e7749338', 11498),
    'preview-contract.js': ('2e1c62e01a52ee05d87765bdfb36a29a7c1e293faeb4d6afc35a7c74abd8b38e', 18467),
    'manifest.json': ('65a5159cfc9e5bd934329c3ba532e62d2b6829132abdb09e2216e2902d51d41b', 6605),
    'review-packet.json': ('3d2d3a8b001eaa1317ea1d480fc806bb2a21d2dd08e3f1aa3cf7b1a37596969b', 13747),
}
# These are independent cropped non-preview references. They do not become
# complete-main/C35 proof merely because the containing page also uses C35.
UNCHANGED_REFERENCES = frozenset({
    'all-page-settings-general-normal', 'all-page-main-session-normal',
    'all-page-subpage-readingpanel-dataset-body-retired-surface-absent',
    'all-page-subpage-modelconfigscreen-tab-connections-retired-surface-absent',
})
NOT_GRANTED = (
    'whole802_state_design_reference_approval', 'per_state_C35_reference_approval',
    'product_component_alignment', 'production_capture_adapter_approval', 'packaged_native_alignment',
    'real_game_execution_SR_MO_Never7_Sakura', 'release_acceptance', 'unreviewed_public_DTO_database_Core_ABI_changes',
)


def _assess(root, parent):
    errors = []; root = Path(root)
    result = {'component_id': 'C35', 'supplement_id': 'preview-contract-20261104', 'status': 'failed',
              'record': {'path': RECORD, 'sha256': RECORD_SHA256}, 'errors': errors,
              'reference_approval_granted': False, 'product_alignment_approved': False, 'native_verified': False}
    try:
        if parent['status'] != 'approved_by_user' or parent['errors']: raise ValueError('parent source approval invalid')
        p = safe_path(root, RECORD)
        if not p.is_file() or digest(p) != RECORD_SHA256: raise ValueError('exact explicit C35 user decision identity differs')
        r = load(p)
        exact = {'schema_version': 1, 'kind': 'user-approved-component-design-supplement/v1',
                 'target_id': TARGET, 'composition_id': CURRENT_COMPOSITION_ID, 'component_id': 'C35',
                 'supplement_id': 'preview-contract-20261104', 'status': 'approved_by_user',
                 'decision_date': '2026-11-04', 'decision_basis': 'explicit_user_message',
                 'approved_source_count': 5, 'entry': PREFIX + 'index.html', 'captured_candidate_status': 'pending_review'}
        for key, value in exact.items():
            if type(r.get(key)) is not type(value) or r[key] != value: raise ValueError('typed C35 source decision differs: ' + key)
        if r['parent_approval'] != {'path': CURRENT_RECORD, 'sha256': CURRENT_RECORD_SHA256, 'design_source_count': 61,
                                  'design_closure_sha256': CURRENT_DESIGN_CLOSURE_SHA256}:
            raise ValueError('C35 supplement parent scope differs')
        if r['user_confirmation']['answer'] != '采用这五项增量，继续更新': raise ValueError('explicit five-item user adoption missing')
        if [s['id'] for s in r['approved_scope']] != ['PC-01', 'PC-02', 'PC-03', 'PC-04', 'PC-05']:
            raise ValueError('exact five C35 decisions required')
        if set(r['unchanged_non_C35_reference_ids']) != UNCHANGED_REFERENCES or r['not_granted'] != list(NOT_GRANTED):
            raise ValueError('C35 source adoption must not grant reference/product/native capabilities')
        expected = {PREFIX + name: {'sha256': h, 'bytes': size} for name, (h, size) in SOURCE_HASHES.items()}
        if r['approved_sources'] != expected: raise ValueError('exact frozen C35 five-source closure required')
        for path, fact in expected.items():
            source = safe_path(root, path)
            if not source.is_file() or source.stat().st_size != fact['bytes'] or digest(source) != fact['sha256']:
                raise ValueError('approved C35 source bytes drift: ' + path)
        independent = r['independent_review']; review = safe_path(root, independent['path'])
        if not review.is_file() or digest(review) != independent['sha256']:
            raise ValueError('exact independent C35 candidate readiness evidence missing')
        if independent['status'] != 'ready_for_user_review' or independent['reviewer_id'] == '/root/preview_design_resolution':
            raise ValueError('independent C35 review is not source-author self-review')
        result.update(status='approved_by_user', entry=r['entry'], approved_source_count=5,
                      parent_design_source_count=61, source_scope='component_supplement_only', approved_sources=expected)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append('C35 effective source approval invalid: ' + str(error))
    return result


def assess(root):
    return _assess(root, current_source_approval(root))


def affects_preview_target(row):
    """Conservative target-level impact, keeping unrelated cropped evidence valid."""
    if row['id'] in UNCHANGED_REFERENCES: return False
    selector = row.get('target', {}).get('selector', '')
    # A full capture can contain the preview even when inventory ownership is
    # assigned to the shell or selection step. Component labels cannot exempt
    # actual full-window/parent-dialog evidence from the adopted source target.
    if selector == '#main-window' or selector.startswith('#rw-preview-details'): return True
    if any(path.endswith('/CaptureSheet.tsx') for path in row.get('component_paths', [])): return True
    if 'C35' not in row.get('component_ids', []): return False
    if row['id'] in ('preview-permission-lifecycle', 'records-folding-preserves-data-preview'): return True
    if row['id'].startswith('all-page-main-session-'):
        target = row.get('target', {}).get('selector')
        # These crops exclude the independent preview; a complete #main-window
        # image includes it and must receive a supplement-aware recipe.
        return not (target and (target.startswith('#main-content .reading-unit')
            or target.startswith('details[data-flat-session-section=') or target.startswith('#main-content .empty-content')
            or target.startswith('#main-content .status-banner')))
    return True


def source_mapping_gaps(root, rows, required):
    affected = [r for r in rows if r['id'] in required and affects_preview_target(r)]
    if not affected: return []
    approval = assess(root)
    if approval['errors']: return approval['errors']
    # The approved side-by-side caseboard is not a complete-main composition.
    # A state needs an explicit reviewed source recipe; parent61 cannot silently
    # continue as the new C35 target, and JSON metadata cannot install recipes.
    missing = [r['id'] for r in affected if not any(s.get('action') == 'navigate'
               and s.get('path') == PREFIX + 'index.html' for s in r.get('design_recipe', []))]
    return (['approved C35 supplement requires exact current source-state recipes: ' + ','.join(sorted(missing))]
            if missing else [])
