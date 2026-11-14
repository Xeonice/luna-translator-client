#!/usr/bin/env python3
"""Require an independently reviewed, exact current change before acceptance.

The caller supplies the actual Git/snapshot diff, never allowed_paths or a
claimed product/tooling kind. The ignored evidence file cannot form a hash cycle
with source metadata: this checker computes its raw hash into the run receipt.
Actor independence is an auditable review declaration, not cryptographic proof
of a human identity or a substitute for protected remote checks.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re

from gate_core import check_scope, safe_path, source_files

ROOT = Path(__file__).resolve().parents[2]
SHA = re.compile(r'[0-9a-f]{64}\Z')
FIELDS = {'schema_version', 'kind', 'change_id', 'reviewer_id', 'author_ids',
          'status', 'independent_non_author_review', 'source_tree_sha256',
          'files', 'blocking_findings'}


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate review JSON key: ' + key)
        result[key] = value
    return result


def _read(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(
                          ValueError('non-JSON review value: ' + value)))


def _actor(value):
    return (isinstance(value, str) and 0 < len(value) <= 240
            and value == value.strip()
            and not any(ord(char) < 32 or ord(char) == 127 for char in value))


def _path(root, relative):
    if (not isinstance(relative, str) or not relative or relative != relative.strip()
            or any(ord(char) < 32 or ord(char) == 127 for char in relative)
            or any(char in relative for char in ('\\', ':', '?', '#'))
            or PurePosixPath(relative).is_absolute()
            or any(part in ('', '.', '..') for part in relative.split('/'))):
        raise ValueError('safe relative POSIX review/source path required')
    return safe_path(Path(root), relative)


def tree_identity(sources):
    # Match the complete source identity in the engineering receipt.
    return hashlib.sha256(json.dumps(sources, sort_keys=True).encode()).hexdigest()


def file_identity(root, relative):
    path = _path(root, relative)
    if not path.exists():
        return {'path': relative, 'bytes': None, 'sha256': None}
    if not path.is_file():
        raise ValueError('review source is not a regular file: ' + relative)
    raw = path.read_bytes()
    return {'path': relative, 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def validate_review(root, change_id, metadata, changed_paths):
    """Validate actual changed bytes and their complete current source context."""
    result = {'schema_version': 1, 'kind': 'independent-change-review-check/v1',
              'change_id': change_id, 'status': 'failed', 'required': True,
              'errors': [], 'evidence': None, 'source_tree_sha256': None,
              'reviewer_id': None, 'reviewed_file_count': 0,
              'product_tests_executed': False, 'product_release_claim': False}
    errors = result['errors']
    try:
        if (not isinstance(changed_paths, (list, tuple))
                or any(not isinstance(path, str) for path in changed_paths)
                or len(changed_paths) != len(set(changed_paths))):
            raise ValueError('unique actual changed source paths required')
        paths = sorted(changed_paths)
        for path in paths:
            _path(root, path)
        sources = source_files(Path(root))
        result['source_tree_sha256'] = tree_identity(sources)
        if not paths:
            result.update(status='passed', required=False)
            return result
        evidence = metadata.get('review_evidence')
        if not isinstance(evidence, dict) or set(evidence) != {'path', 'authors'}:
            raise ValueError('actual source changes require exact review_evidence path/authors')
        authors = evidence['authors']
        if (not isinstance(authors, list) or not authors
                or any(not _actor(actor) for actor in authors)
                or len(authors) != len(set(authors))):
            raise ValueError('explicit unique change author identities required')
        relative = evidence['path']
        path = _path(root, relative)
        # Review evidence must remain outside the managed source closure so its
        # exact hash can be computed without a source/record self-reference.
        if not relative.startswith('.build/') or relative in sources:
            raise ValueError('review evidence must use an ignored .build path')
        raw = path.read_bytes()
        result['evidence'] = {'path': relative, 'bytes': len(raw),
                              'sha256': hashlib.sha256(raw).hexdigest()}
        review = _read(path)
        if not isinstance(review, dict) or set(review) != FIELDS:
            raise ValueError('exact independent change review record fields required')
        if (type(review['schema_version']) is not int or review['schema_version'] != 1
                or review['kind'] != 'independent-change-review/v1'
                or review['change_id'] != change_id):
            errors.append('review kind/version/change identity differs')
        if (review['status'] != 'approved'
                or review['independent_non_author_review'] is not True
                or review['blocking_findings'] != []):
            errors.append('review is not an independent approval without blocking findings')
        reviewer = review['reviewer_id']
        if not _actor(reviewer) or reviewer in authors:
            errors.append('reviewer must be a distinct non-author identity')
        else:
            result['reviewer_id'] = reviewer
        if review['author_ids'] != authors:
            errors.append('review author set differs from the declared change authors')
        if (not isinstance(review['source_tree_sha256'], str)
                or not SHA.fullmatch(review['source_tree_sha256'])
                or review['source_tree_sha256'] != result['source_tree_sha256']):
            errors.append('review is stale: complete current source tree identity differs')
        expected = [file_identity(root, path) for path in paths]
        rows = review['files']
        # JSON booleans compare equal to integers in Python. Validate bytes
        # before equality so true/false cannot forge one/zero byte identities.
        if (not isinstance(rows, list) or any(
                not isinstance(row, dict) or set(row) != {'path', 'bytes', 'sha256'}
                or not isinstance(row['path'], str)
                or (row['bytes'] is not None and type(row['bytes']) is not int)
                or (row['bytes'] is None) != (row['sha256'] is None)
                or (row['sha256'] is not None and (not isinstance(row['sha256'], str)
                                                   or not SHA.fullmatch(row['sha256'])))
                for row in rows) or rows != expected):
            errors.append('review exact changed-file coverage/bytes/hash differs; omitted or extra files forbidden')
        else:
            result['reviewed_file_count'] = len(expected)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        errors.append('independent change review invalid: ' + str(error))
    result['status'] = 'failed' if errors else 'passed'
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--change', required=True)
    parser.add_argument('--base')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        relative = 'openspec/changes/' + args.change + '/verification.json'
        metadata = _read(_path(args.root, relative))
        ci = os.getenv('CI', '').lower() not in {'', '0', 'false', 'no', 'off'}
        paths, scope_errors = check_scope(args.root, metadata, args.base, ci)
        result = validate_review(args.root, args.change, metadata, paths)
        result['errors'] = scope_errors + result['errors']
        result['status'] = 'failed' if result['errors'] else 'passed'
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        result = {'status': 'failed', 'errors': [str(error)],
                  'product_tests_executed': False, 'product_release_claim': False}
    print(json.dumps(result, ensure_ascii=False, indent=2) if args.json else result['status'])
    return int(bool(result['errors']))


if __name__ == '__main__':
    raise SystemExit(main())
