"""Preserve source identity while avoiding traversal of excluded build trees."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from gate_core import IGNORED_ROOTS, digest, source_files

LEGACY_ROOTS = frozenset({'.git', '.build', 'target', 'dist', 'reports', 'runtime', 'diagnostics'})


def legacy_inventory(root):
    # Pre-change algorithm, retained as an independent equivalence oracle.
    result = {}
    for p in root.rglob('*'):
        relative = p.relative_to(root)
        if relative.parts[0] in LEGACY_ROOTS or any(part in {'node_modules', '__pycache__'} for part in relative.parts):
            continue
        if relative.is_relative_to('apps/desktop/dist'):
            continue
        if p.is_symlink():
            result[str(relative)] = 'symlink:' + os.readlink(p)
        elif p.is_file():
            result[str(relative)] = hashlib.sha256(p.read_bytes()).hexdigest()
    return result


class SourceInventoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def put(self, path, content=b'bytes\x00\xff'):
        p = self.root / path
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(content)
        return p

    def equal_inventory(self):
        old, current = legacy_inventory(self.root), source_files(self.root)
        self.assertEqual(old, current)
        fingerprint = lambda data: hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest()
        self.assertEqual(fingerprint(old), fingerprint(current))
        return current

    def test_inclusion_contract_and_digest_do_not_drift_with_new_walker(self):
        self.assertEqual(LEGACY_ROOTS, IGNORED_ROOTS)
        p = self.put('source.rs')
        self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(), digest(p))

    def test_empty_hidden_unicode_and_lookalikes(self):
        self.assertEqual({}, self.equal_inventory())
        for name in ['normal.rs', '.hidden/日本語.txt', 'Target/code.rs', 'node_modules_extra/a', 'apps/desktop/distance/a']:
            self.put(name)
        self.assertEqual(5, len(self.equal_inventory()))

    def test_ignored_roots_are_root_only(self):
        for name in IGNORED_ROOTS:
            self.put(name + '/excluded')
            self.put('src/' + name + '/managed')
        data = self.equal_inventory()
        self.assertEqual({f'src/{name}/managed' for name in IGNORED_ROOTS}, set(data))

    def test_any_segment_exclusions_and_precise_desktop_dist(self):
        for name in ['node_modules/a', 'x/node_modules/a', 'x/y/__pycache__/a', 'apps/desktop/dist/a']:
            self.put(name)
        for name in ['apps/desktop/sub/dist/a', 'desktop/dist/a', 'apps/desktop/distribution/a']:
            self.put(name)
        self.assertEqual(3, len(self.equal_inventory()))

    def test_excluded_names_can_be_regular_files(self):
        for name in [*IGNORED_ROOTS, 'src/node_modules', 'src/__pycache__', 'apps/desktop/dist']:
            self.put(name)
        self.put('apps/desktop/sub/dist')
        self.assertEqual({'apps/desktop/sub/dist'}, set(self.equal_inventory()))

    def test_file_directory_dangling_and_external_links_keep_raw_identity(self):
        target = self.put('actual/value')
        for name, destination in [('file-link', 'actual/value'), ('dir-link', 'actual'),
                                  ('missing-link', 'missing'), ('external-link', '/not-a-managed-source')]:
            (self.root / name).symlink_to(destination)
        (self.root / 'ignored-target-link').symlink_to('.build')
        self.put('.build/private')
        data = self.equal_inventory()
        self.assertEqual('symlink:actual', data['dir-link'])
        self.assertEqual('symlink:/not-a-managed-source', data['external-link'])
        self.assertEqual('symlink:.build', data['ignored-target-link'])
        self.assertNotIn('dir-link/value', data)
        self.assertEqual(digest(target), data['actual/value'])

    def test_exclusion_applies_before_symlink_recording(self):
        self.put('actual/a')
        (self.root / 'target').symlink_to('actual')
        (self.root / 'node_modules').symlink_to('actual')
        (self.root / 'src').mkdir()
        (self.root / 'src/__pycache__').symlink_to('../actual')
        self.assertEqual({'actual/a'}, set(self.equal_inventory()))

    def test_ignored_directories_are_never_entered(self):
        ignored = {self.root / '.build', self.root / 'target', self.root / 'apps/desktop/dist',
                   self.root / 'src/node_modules', self.root / 'src/__pycache__'}
        for p in ignored:
            p.mkdir(parents=True, exist_ok=True)
            (p / 'large-cache').write_text('not source')
        self.put('src/dist/a')
        self.put('apps/desktop/distance/a')
        expected = legacy_inventory(self.root)
        visited = set()
        real_scandir = os.scandir

        def tripwire(path):
            path = Path(path)
            self.assertNotIn(path, ignored, 'excluded tree was traversed')
            visited.add(path)
            return real_scandir(path)

        with patch('os.scandir', side_effect=tripwire):
            actual = source_files(self.root)
        self.assertEqual(expected, actual)
        self.assertIn(self.root / 'src/dist', visited)
        self.assertIn(self.root / 'apps/desktop/distance', visited)

    def test_repeated_calls_never_reuse_cached_bytes_or_removed_sources(self):
        p = self.put('source.rs', b'one')
        first = self.equal_inventory()
        p.write_bytes(b'two')
        second = self.equal_inventory()
        self.assertNotEqual(first, second)
        p.unlink()
        self.assertEqual({}, self.equal_inventory())


if __name__ == '__main__':
    unittest.main()
