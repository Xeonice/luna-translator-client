import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile


SPEC = importlib.util.spec_from_file_location(
    "prepare_runtime", Path(__file__).resolve().parents[1] / "scripts/prepare_lunahook_runtime.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class RuntimePreparationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "release.zip"
        payload = b"fixture bytes; never executed"
        with zipfile.ZipFile(self.archive, "w") as source:
            source.writestr("release/Host.dll", payload)
        self.lock = self.root / "lock.json"
        self.data = {"schema_version": 1, "abi_profile": "fixture-only",
                     "archive_sha256": hashlib.sha256(self.archive.read_bytes()).hexdigest(),
                     "files": [{"archive_path": "release/Host.dll", "runtime_path": "Host.dll",
                                "size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}]}
        self.write_lock()
        self.output = self.root / "runtime"

    def write_lock(self):
        self.lock.write_text(json.dumps(self.data))

    def test_verified_members_and_manifest_only(self):
        result = MODULE.prepare(self.archive, self.output, self.lock)
        self.assertFalse(result["executed"])
        self.assertEqual({p.name for p in self.output.iterdir()}, {"Host.dll", "runtime-manifest.json"})
        self.assertEqual(json.loads((self.output / "runtime-manifest.json").read_text()), self.data)

    def test_rejects_wrong_archive_before_creating_destination(self):
        self.archive.write_bytes(b"untrusted replacement")
        with self.assertRaisesRegex(ValueError, "Archive SHA-256"):
            MODULE.prepare(self.archive, self.output, self.lock)
        self.assertFalse(self.output.exists())

    def test_rejects_wrong_member_even_if_archive_hash_matches(self):
        self.data["files"][0]["sha256"] = "0" * 64
        self.write_lock()
        with self.assertRaisesRegex(ValueError, "Pinned file verification"):
            MODULE.prepare(self.archive, self.output, self.lock)
        self.assertFalse(self.output.exists())

    def test_existing_destination_is_preserved(self):
        self.output.mkdir()
        marker = self.output / "keep.txt"
        marker.write_text("user data")
        with self.assertRaisesRegex(ValueError, "already exists"):
            MODULE.prepare(self.archive, self.output, self.lock)
        self.assertEqual(marker.read_text(), "user data")

    def test_manifest_cannot_escape_destination(self):
        self.data["files"][0]["runtime_path"] = "../outside.dll"
        self.write_lock()
        with self.assertRaisesRegex(ValueError, "Unsafe"):
            MODULE.prepare(self.archive, self.output, self.lock)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()

