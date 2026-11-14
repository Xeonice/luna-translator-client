"""Regression guards for reviewed source adoption; never product acceptance."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import implementation_status as status
import current_general_visual_acceptance as visual
from gate_core import digest, source_files, check_scope
from change_review import file_identity

ROOT = Path(__file__).resolve().parents[2]


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()


class SourceReviewSupplementGuards(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='source-review-guard-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.proof_path = self.root / 'reviews/source-audit.json'
        self.proof_path.parent.mkdir()
        products = {'crates/guard/src/lib.rs': hashlib.sha256(b'fixture').hexdigest()}
        self.catalog = {
            'audit_date': '2026-10-20', 'modules': [{'id': 'M01'}],
            'components': [], 'capabilities': [], 'product_sources_sha256': products,
            'incremental_source_review': {
                'schema_version': 1, 'scope': 'source_only', 'review_date': '2026-11-09',
                'original_audit_date': '2026-10-20', 'author': '/fixture/author',
                'product_source_count': 1, 'semantic_record_count': 1,
            },
        }
        body = hashlib.sha256(canonical(self.catalog)).hexdigest()
        self.catalog['incremental_source_review']['catalog_body_sha256'] = body
        self.proof = {
            'schema_version': 1, 'kind': 'independent-implementation-source-audit/v1',
            'status': 'approved', 'independent_non_author_review': True,
            'author_ids': ['/fixture/author'], 'reviewer_id': '/fixture/reviewer',
            'blocking_findings': [], 'review_date': '2026-11-09',
            'catalog_body_sha256': body,
            'product_sources_sha256': hashlib.sha256(canonical(products)).hexdigest(),
        }
        self.write_proof()

    def write_proof(self, text=None):
        self.proof_path.write_text(json.dumps(self.proof) if text is None else text)
        self.catalog['audit_inputs'] = [{
            'path': 'reviews/source-audit.json', 'sha256': digest(self.proof_path),
        }]

    def errors(self):
        errors = []
        status._incremental_source_review(self.root, self.catalog, errors)
        return errors

    def test_independent_body_product_and_origin_bindings_are_valid(self):
        self.assertEqual([], self.errors())
        self.assertEqual('2026-10-20', self.catalog['audit_date'])

    def test_missing_and_duplicate_current_review_are_rejected(self):
        self.catalog['audit_inputs'] = []
        self.assertIn('exactly one', str(self.errors()))
        self.write_proof()
        second = self.root / 'reviews/second.json'
        second.write_bytes(self.proof_path.read_bytes())
        self.catalog['audit_inputs'].append({'path': 'reviews/second.json', 'sha256': digest(second)})
        self.assertIn('exactly one', str(self.errors()))

    def test_same_author_aliases_and_control_actors_are_rejected(self):
        original = copy.deepcopy(self.proof)
        for actor in ('/fixture/author', '/fixture/author ', ' /fixture/author',
                      '/fixture/reviewer\n', '/fixture/reviewer\x7f', 'x' * 241):
            with self.subTest(actor=repr(actor)):
                self.proof = {**original, 'reviewer_id': actor}
                self.write_proof()
                self.assertTrue(self.errors())
        self.proof = {**original, 'author_ids': ['/fixture/author ']}
        self.write_proof()
        self.assertTrue(self.errors())
        self.proof = original
        self.catalog['incremental_source_review']['author'] = '/fixture/author '
        self.write_proof()
        self.assertTrue(self.errors())

    def test_approved_with_blockers_or_wrong_types_cannot_be_used(self):
        original = copy.deepcopy(self.proof)
        for changes in ({'blocking_findings': ['unresolved']}, {'blocking_findings': None},
                        {'schema_version': True}, {'status': 'pending'},
                        {'independent_non_author_review': 1}):
            with self.subTest(changes=changes):
                self.proof = {**original, **changes}
                self.write_proof()
                self.assertTrue(self.errors())

    def test_body_product_map_and_date_drift_are_rejected_even_after_rehashing_proof(self):
        original = copy.deepcopy(self.proof)
        for key, value in (('catalog_body_sha256', '0' * 64),
                           ('product_sources_sha256', '0' * 64),
                           ('review_date', '2026-11-10')):
            with self.subTest(key=key):
                self.proof = {**original, key: value}
                self.write_proof()
                self.assertTrue(self.errors())
        self.proof = original
        self.write_proof()
        self.catalog['modules'][0]['claim'] = 'changed after review'
        self.assertTrue(self.errors())

    def test_proof_hash_duplicate_keys_and_nonfinite_json_are_rejected(self):
        original = json.dumps(self.proof)
        self.proof_path.write_text(original + ' ')
        self.assertIn('hash differs', str(self.errors()))
        for tail in (',"status":"approved"}', ',"untrusted":NaN}',
                     ',"untrusted":Infinity}', ',"untrusted":1e999}'):
            with self.subTest(tail=tail):
                self.write_proof(original[:-1] + tail)
                self.assertTrue(self.errors())

    def test_unrelated_historical_inputs_keep_their_original_byte_contract(self):
        for content in (b'not JSON', b'\xff\xfe\x00',
                        b'{"kind":"historical","metric":NaN}',
                        b'{"kind":"historical","metric":1e999}',
                        b'{"kind":"historical","x":1,"x":2}'):
            with self.subTest(content=content):
                self.write_proof()
                old = self.root / 'reviews/historical.json'
                old.write_bytes(content)
                self.catalog['audit_inputs'].append({
                    'path': 'reviews/historical.json', 'sha256': digest(old)})
                self.assertEqual([], self.errors())

    def test_legacy_origin_without_supplement_remains_separate(self):
        self.catalog.pop('incremental_source_review')
        self.catalog.pop('audit_inputs')
        self.assertEqual([], self.errors())


class ReviewedToolingScopeGuards(unittest.TestCase):
    def increment_fixture(self):
        temporary = tempfile.TemporaryDirectory(prefix='reviewed-verifier-increment-')
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        prefix = 'openspec/changes/' + visual.REVIEWED_TOOLING_CHANGE + '/'
        for source in (ROOT / prefix).rglob('*'):
            if source.is_file():
                target = root / source.relative_to(ROOT)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
        old = {'docs/README.md': hashlib.sha256(b'old').hexdigest()}
        (root / 'docs').mkdir(exist_ok=True)
        (root / 'docs/README.md').write_bytes(b'reviewed verifier notes')
        snapshot_path = root / prefix / 'source-snapshot.json'
        snapshot_path.write_text(json.dumps({'files': old, 'source_tree_sha256': visual.tree_sha(old)}))
        metadata = json.loads((root / prefix / 'verification.json').read_text())
        paths, errors = check_scope(root, metadata)
        self.assertEqual([], errors)
        review_path = root / metadata['review_evidence']['path']
        review_path.parent.mkdir(parents=True)
        review = {
            'schema_version': 1, 'kind': 'independent-change-review/v1',
            'change_id': visual.REVIEWED_TOOLING_CHANGE,
            'reviewer_id': '/fixture/independent-reviewer',
            'author_ids': metadata['review_evidence']['authors'], 'status': 'approved',
            'independent_non_author_review': True,
            'source_tree_sha256': visual.tree_sha(source_files(root)),
            'files': [file_identity(root, p) for p in paths], 'blocking_findings': [],
        }
        review_path.write_text(json.dumps(review))
        return root, old, metadata, review_path, review

    def test_exact_non_author_verifier_increment_is_accepted(self):
        root, old, metadata, _, _ = self.increment_fixture()
        with patch.object(visual, 'REVIEWED_TOOLING_BASE_SOURCE_SHA256', visual.tree_sha(old)):
            allowed = visual._reviewed_tooling_increment(root, old, source_files(root))
        self.assertEqual(set(metadata['allowed_paths']), allowed)

    def test_increment_missing_stale_or_self_review_is_rejected(self):
        root, old, metadata, path, review = self.increment_fixture()
        with patch.object(visual, 'REVIEWED_TOOLING_BASE_SOURCE_SHA256', visual.tree_sha(old)):
            path.unlink()
            with self.assertRaisesRegex(ValueError, 'independent exact whole-source review'):
                visual._reviewed_tooling_increment(root, old, source_files(root))
            path.write_text(json.dumps({**review, 'reviewer_id': review['author_ids'][0]}))
            with self.assertRaisesRegex(ValueError, 'independent exact whole-source review'):
                visual._reviewed_tooling_increment(root, old, source_files(root))
            path.write_text(json.dumps(review))
            (root / 'docs/README.md').write_bytes(b'changed after review')
            with self.assertRaisesRegex(ValueError, 'independent exact whole-source review'):
                visual._reviewed_tooling_increment(root, old, source_files(root))

    def test_increment_cannot_expand_into_product_or_unknown_sources(self):
        root, old, metadata, _, _ = self.increment_fixture()
        metadata['allowed_paths'].append('apps/desktop/src/main.tsx')
        path = root / 'openspec/changes' / visual.REVIEWED_TOOLING_CHANGE / 'verification.json'
        path.write_text(json.dumps(metadata))
        with patch.object(visual, 'REVIEWED_TOOLING_BASE_SOURCE_SHA256', visual.tree_sha(old)):
            with self.assertRaisesRegex(ValueError, 'tooling-only'):
                visual._reviewed_tooling_increment(root, old, source_files(root))
        metadata['allowed_paths'].pop()
        path.write_text(json.dumps(metadata))
        (root / 'docs/unknown-source.md').write_text('unreviewed')
        with patch.object(visual, 'REVIEWED_TOOLING_BASE_SOURCE_SHA256', visual.tree_sha(old)):
            with self.assertRaisesRegex(ValueError, 'exact scope'):
                visual._reviewed_tooling_increment(root, old, source_files(root))

    def test_namespace_valid_unknown_path_cannot_be_granted_by_metadata_or_review(self):
        root, old, metadata, review_path, review = self.increment_fixture()
        unknown = 'docs/development/unknown-policy.json'
        (root / unknown).parent.mkdir(parents=True, exist_ok=True)
        (root / unknown).write_text('{}')
        metadata['allowed_paths'].append(unknown)
        (root / 'openspec/changes' / visual.REVIEWED_TOOLING_CHANGE / 'verification.json').write_text(json.dumps(metadata))
        paths, errors = check_scope(root, metadata)
        self.assertEqual([], errors)
        review['files'] = [file_identity(root, p) for p in paths]
        review['source_tree_sha256'] = visual.tree_sha(source_files(root))
        review_path.write_text(json.dumps(review))
        with patch.object(visual, 'REVIEWED_TOOLING_BASE_SOURCE_SHA256', visual.tree_sha(old)):
            with self.assertRaisesRegex(ValueError, 'exact reviewed tooling-only'):
                visual._reviewed_tooling_increment(root, old, source_files(root))

    def test_empty_git_scope_does_not_skip_frozen_delta_review(self):
        root, old, _, path, _ = self.increment_fixture()
        path.unlink()
        with patch.object(visual, 'REVIEWED_TOOLING_BASE_SOURCE_SHA256', visual.tree_sha(old)), \
                patch('gate_core.check_scope', return_value=([], [])):
            with self.assertRaisesRegex(ValueError, 'independent exact whole-source review'):
                visual._reviewed_tooling_increment(root, old, source_files(root))

    def test_only_verifier_namespaces_can_extend_the_accepted_candidate(self):
        for path in ('scripts/quality/example.py', 'docs/architecture/implementation-status.json',
                     'docs/development/iteration-baseline.json', 'docs/README.md',
                     'openspec/changes/close-engineering-gate-drift/verification.json'):
            with self.subTest(path=path):
                self.assertTrue(visual._tooling_increment_path(path))
        for path in ('apps/desktop/src/main.tsx', 'crates/app-core/src/lib.rs', 'Cargo.toml',
                     'native/runtime.lock.json', 'resources/dictionary.json',
                     'docs/design/first-product-v1/settings.html',
                     'docs/development/design-approval-complete-20261031.json',
                     'docs/development/current-general-reference-review.json',
                     'openspec/legacy-debt.json', 'scripts/quality/**',
                     'docs/development/../../apps/main.tsx', '/tmp/override',
                     'docs//development/x', 'docs/development/x\n'):
            with self.subTest(path=path):
                self.assertFalse(visual._tooling_increment_path(path))

    def test_current_source_audit_is_machine_bound_to_its_independent_review(self):
        catalog = json.loads((ROOT / status.CATALOG).read_text())
        self.assertEqual([], status.validate_catalog(ROOT))
        supplement = catalog['incremental_source_review']
        self.assertEqual(supplement['catalog_body_sha256'], status.incremental_catalog_body_sha256(catalog))
        self.assertFalse(catalog['runtime_verified_this_audit'])
        self.assertFalse(catalog['current_release_approved'])
        self.assertTrue(all(row['visual_alignment_status'] == 'not_assessed'
                            for group in ('modules', 'components', 'capabilities')
                            for row in catalog[group]))


if __name__ == '__main__':
    unittest.main()
