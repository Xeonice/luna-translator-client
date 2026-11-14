"""Exact independent review must survive real mutations, not status labels."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import change_review as guard
from gate_core import source_files


class ChangeReviewTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.cid = 'review-probe'
        self.meta_path = self.root / 'openspec/changes/review-probe/verification.json'
        self.snapshot = 'openspec/changes/review-probe/source-snapshot.json'
        self.meta = {'kind': 'tooling', 'source_snapshot': self.snapshot,
                     'allowed_paths': ['src/example.txt', str(self.meta_path.relative_to(self.root)), self.snapshot],
                     'requirements': [],
                     'review_evidence': {'path': '.build/review.json', 'authors': ['builder']}}
        self.write('src/example.txt', b'A')
        self.write(self.snapshot, b'{"files": {}}\n')
        self.write(str(self.meta_path.relative_to(self.root)), self.encoded(self.meta))
        self.paths = sorted(source_files(self.root))
        self.review = {'schema_version': 1, 'kind': 'independent-change-review/v1',
                       'change_id': self.cid, 'reviewer_id': 'auditor',
                       'author_ids': ['builder'], 'status': 'approved',
                       'independent_non_author_review': True,
                       'source_tree_sha256': guard.tree_identity(source_files(self.root)),
                       'files': [guard.file_identity(self.root, path) for path in self.paths],
                       'blocking_findings': []}
        self.save_review()

    @staticmethod
    def encoded(value):
        return (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()

    def write(self, relative, raw):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)

    def save_review(self):
        self.write('.build/review.json', self.encoded(self.review))

    def inspect(self, metadata=None, paths=None):
        return guard.validate_review(self.root, self.cid,
                                     self.meta if metadata is None else metadata,
                                     self.paths if paths is None else paths)

    def rejected(self, fragment, metadata=None, paths=None):
        result = self.inspect(metadata, paths)
        self.assertEqual(result['status'], 'failed')
        self.assertTrue(any(fragment in error for error in result['errors']), result)
        return result

    def cli(self):
        p = subprocess.run([sys.executable, str(Path(guard.__file__)), '--root',
                            str(self.root), '--change', self.cid, '--json'],
                           capture_output=True, text=True)
        return p, json.loads(p.stdout)

    def test_actual_cli_accepts_only_bound_non_author_review(self):
        process, result = self.cli()
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(result, self.inspect())
        self.assertEqual(result['reviewed_file_count'], 3)
        self.assertEqual(result['evidence']['sha256'], hashlib.sha256(
            (self.root / '.build/review.json').read_bytes()).hexdigest())
        self.assertIs(result['product_tests_executed'], False)
        self.assertIs(result['product_release_claim'], False)

    def test_real_source_mutation_fails_cli_without_rerouting_authority(self):
        self.write('src/example.txt', b'changed after review')
        process, result = self.cli()
        self.assertEqual(process.returncode, 1)
        self.assertTrue(any('stale' in error for error in result['errors']))
        self.assertTrue(any('changed-file' in error for error in result['errors']))

    def test_missing_record_fails_real_cli(self):
        (self.root / '.build/review.json').unlink()
        process, result = self.cli()
        self.assertEqual(process.returncode, 1)
        self.assertTrue(any('review invalid' in error for error in result['errors']))

    def test_self_review_fails_real_cli(self):
        self.review['reviewer_id'] = 'builder'
        self.save_review()
        process, result = self.cli()
        self.assertEqual(process.returncode, 1)
        self.assertTrue(any('non-author' in error for error in result['errors']))

    def test_tooling_kind_and_empty_check_list_do_not_bypass_review(self):
        metadata = copy.deepcopy(self.meta)
        metadata.pop('review_evidence')
        self.rejected('require exact review_evidence', metadata)

    def test_product_kind_does_not_bypass_review(self):
        metadata = copy.deepcopy(self.meta)
        metadata['kind'] = 'product'
        metadata.pop('review_evidence')
        self.rejected('require exact review_evidence', metadata)

    def test_no_actual_changes_requires_no_review(self):
        result = self.inspect({}, [])
        self.assertEqual(result['status'], 'passed')
        self.assertIs(result['required'], False)
        self.assertIsNone(result['evidence'])

    def test_omitted_file_rejected(self):
        self.review['files'].pop()
        self.save_review()
        self.rejected('changed-file coverage')

    def test_extra_file_rejected(self):
        self.review['files'].append({'path': 'not-actually-changed.txt', 'bytes': None, 'sha256': None})
        self.save_review()
        self.rejected('changed-file coverage')

    def test_duplicate_file_rejected(self):
        self.review['files'].append(self.review['files'][0])
        self.save_review()
        self.rejected('changed-file coverage')

    def test_wrong_file_bytes_or_hash_rejected(self):
        for key, value in [('bytes', 99999), ('sha256', 'f' * 64)]:
            with self.subTest(key=key):
                old = self.review['files'][0][key]
                self.review['files'][0][key] = value
                self.save_review()
                self.rejected('changed-file coverage')
                self.review['files'][0][key] = old

    def test_boolean_is_not_one_byte(self):
        self.review['files'][-1]['bytes'] = True  # src/example.txt is exactly one byte.
        self.save_review()
        self.rejected('changed-file coverage')

    def test_changed_context_outside_claimed_files_is_stale(self):
        self.write('src/other.txt', b'unreviewed context')
        self.rejected('complete current source tree')

    def test_status_label_alone_cannot_pass(self):
        self.write('.build/review.json', b'{"status":"approved"}')
        self.rejected('exact independent change review record fields')

    def test_failed_pending_or_blocked_review_rejected(self):
        for status in ('failed', 'pending_review', 'blocked'):
            with self.subTest(status=status):
                self.review['status'] = status
                self.save_review()
                self.rejected('not an independent approval')

    def test_blocking_finding_rejected(self):
        self.review['blocking_findings'] = ['unresolved design mismatch']
        self.save_review()
        self.rejected('without blocking findings')

    def test_independent_flag_requires_actual_boolean_true(self):
        for value in (False, 1, 'true'):
            with self.subTest(value=value):
                self.review['independent_non_author_review'] = value
                self.save_review()
                self.rejected('not an independent approval')

    def test_mismatched_authors_or_change_rejected(self):
        self.review['author_ids'] = ['someone_else']
        self.review['change_id'] = 'different-change'
        self.save_review()
        result = self.rejected('author set differs')
        self.assertTrue(any('change identity' in e for e in result['errors']))

    def test_invalid_author_declarations_rejected(self):
        for authors in ([], ['builder', 'builder'], [''], [' builder'], ['bad\nidentity'], [False]):
            with self.subTest(authors=authors):
                metadata = copy.deepcopy(self.meta)
                metadata['review_evidence']['authors'] = authors
                self.rejected('unique change author identities', metadata)

    def test_invalid_reviewer_rejected(self):
        self.review['reviewer_id'] = ''
        self.save_review()
        self.rejected('non-author identity')

    def test_evidence_path_must_stay_ignored_and_inside_repository(self):
        for path in ('../outside.json', '/tmp/review.json', 'C:\\review.json',
                     '.build/../review.json', './.build/review.json', 'src/review.json'):
            with self.subTest(path=path):
                metadata = copy.deepcopy(self.meta)
                metadata['review_evidence']['path'] = path
                self.rejected('review', metadata)

    def test_symlink_evidence_rejected(self):
        (self.root / '.build/linked.json').symlink_to(self.root / '.build/review.json')
        metadata = copy.deepcopy(self.meta)
        metadata['review_evidence']['path'] = '.build/linked.json'
        self.rejected('Symlink', metadata)

    def test_duplicate_json_key_and_nonfinite_value_rejected(self):
        for raw in (b'{"status":"approved","status":"approved"}', b'{"status":NaN}'):
            with self.subTest(raw=raw):
                self.write('.build/review.json', raw)
                self.rejected('JSON')

    def test_schema_version_boolean_rejected(self):
        self.review['schema_version'] = True
        self.save_review()
        self.rejected('kind/version/change')

    def test_extra_record_fields_rejected(self):
        self.review['skip_review'] = True
        self.save_review()
        self.rejected('exact independent change review record fields')

    def test_deleted_source_requires_explicit_tombstone(self):
        path = 'src/removed.txt'
        paths = sorted(self.paths + [path])
        self.review['files'] = [guard.file_identity(self.root, p) for p in paths]
        self.save_review()
        self.assertEqual(self.inspect(paths=paths)['status'], 'passed')
        self.review['files'][-1]['bytes'] = 0
        self.save_review()
        self.rejected('changed-file coverage', paths=paths)

    def test_duplicate_or_unsafe_actual_diff_rejected(self):
        self.rejected('unique actual changed', paths=self.paths + [self.paths[0]])
        self.rejected('safe relative', paths=['../escape'])


if __name__ == '__main__':
    unittest.main()
