"""Real-file positive/mutation tests for the technology and design byte lock."""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

_SOURCE = Path(__file__).with_name("technology_contract.py")
_SPEC = importlib.util.spec_from_file_location("quality_technology_contract", _SOURCE)
assert _SPEC and _SPEC.loader
contract = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(contract)


class TechnologyContractTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="luna-technology-contract-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.manifests = [
            "Cargo.toml", "Cargo.lock", "apps/desktop/package.json",
            "apps/desktop/package-lock.json", "apps/desktop/tsconfig.json",
            "apps/desktop/vite.config.ts", "apps/desktop/src-tauri/Cargo.toml",
            "apps/desktop/src-tauri/tauri.conf.json",
        ]
        self.write("Cargo.toml", '''[workspace]
members = ["apps/desktop/src-tauri"]
resolver = "3"
[workspace.package]
edition = "2024"
rust-version = "1.89"
''')
        self.write("Cargo.lock", '''version = 4
[[package]]
name = "tauri"
version = "2.9.0"
''')
        self.package = {
            "name": "reader", "private": True,
            "dependencies": {"react": "^19.3.1", "@tauri-apps/api": "2.11.1"},
            "devDependencies": {"typescript": "~5.9.3", "@tauri-apps/cli": "2.11.4"},
        }
        self.save_package()
        resolved = {**self.package["dependencies"], **self.package["devDependencies"]}
        self.write_json("apps/desktop/package-lock.json", {
            "name": "reader", "lockfileVersion": 3,
            "packages": {f"node_modules/{name}": {"version": version.lstrip("^~")} for name, version in resolved.items()},
        })
        self.write_json("apps/desktop/tsconfig.json", {"compilerOptions": {"strict": True}})
        self.write("apps/desktop/vite.config.ts", "export default {};\n")
        self.write("apps/desktop/src-tauri/Cargo.toml", '''[package]
name = "reader"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
[dependencies]
tauri = { version = "2", features = [] }
''')
        self.write_json("apps/desktop/src-tauri/tauri.conf.json", {"productName": "Reader"})
        self.design = "docs/design/first-product-v1/material-tokens.css"
        self.write(self.design, ":root { --text-primary: #e9e9ef; }\n")
        self.document = {
            "schema_version": 1,
            "approved_stack": {
                "packages": {"react": "19", "typescript": "5.9", "@tauri-apps/api": "2", "@tauri-apps/cli": "2"},
                "cargo": {"tauri": "2"},
                "rust": {"edition": "2024", "rust-version": "1.89"},
            },
            "manifests": [self.entry(relative) for relative in self.manifests],
            "design_sources": [self.entry(self.design)],
        }
        self.save_lock()

    def write(self, relative, text):
        target = self.root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        return target

    def write_json(self, relative, document):
        return self.write(relative, json.dumps(document, ensure_ascii=False, indent=2) + "\n")

    def entry(self, relative):
        return {"path": relative, "sha256": hashlib.sha256((self.root / relative).read_bytes()).hexdigest()}

    def save_lock(self):
        self.write_json(contract.LOCK_PATH, self.document)

    def save_package(self):
        self.write_json("apps/desktop/package.json", self.package)

    def refresh_hash(self, relative):
        # Mutation tests refresh only a chosen file's bytes. They must still
        # fail semantic or inventory gates where the approved decision differs.
        for entry in self.document["manifests"]:
            if entry["path"] == relative:
                entry.update(self.entry(relative))
        self.save_lock()

    def errors(self):
        return contract.check(self.root)

    def assert_error(self, text):
        errors = self.errors()
        self.assertTrue(any(text in error for error in errors), errors)

    def test_approved_real_manifests_and_design_pass(self):
        self.assertEqual(self.errors(), [])

    def test_vue_addition_changes_actual_product_manifest(self):
        self.package["dependencies"]["vue"] = "^3.5.0"
        self.save_package()
        self.assert_error("apps/desktop/package.json: manifests SHA-256 mismatch")
        self.assert_error("framework/state package vue has no approved_stack decision")

    def test_redux_addition_even_after_hash_refresh_requires_technology_decision(self):
        self.package["dependencies"]["redux"] = "^5.0.0"
        self.save_package()
        self.refresh_hash("apps/desktop/package.json")
        self.assert_error("framework/state package redux has no approved_stack decision")

    def test_tauri_replacement_with_electron_fails(self):
        del self.package["dependencies"]["@tauri-apps/api"]
        del self.package["devDependencies"]["@tauri-apps/cli"]
        self.package["dependencies"]["electron"] = "42.0.0"
        self.save_package()
        self.refresh_hash("apps/desktop/package.json")
        self.assert_error("framework/state package electron has no approved_stack decision")
        self.assert_error("required product package @tauri-apps/api is missing")

    def test_tauri_cargo_replacement_fails(self):
        relative = "apps/desktop/src-tauri/Cargo.toml"
        target = self.root / relative
        target.write_text(target.read_text().replace('tauri = { version = "2", features = [] }', 'iced = "0.14"'))
        self.refresh_hash(relative)
        self.assert_error("framework crate iced has no approved_stack decision")
        self.assert_error("required product crate tauri is missing")

    def test_react_major_change_fails_even_if_hash_refreshed(self):
        self.package["dependencies"]["react"] = "^20.0.0"
        self.save_package()
        self.refresh_hash("apps/desktop/package.json")
        self.assert_error("package react does not match approved version 19")

    def test_unknown_semver_range_fails_instead_of_guessing(self):
        self.package["dependencies"]["react"] = ">=19"
        self.save_package()
        self.refresh_hash("apps/desktop/package.json")
        self.assert_error("package react does not match approved version 19")

    def test_new_crate_manifest_must_be_explicitly_registered(self):
        self.write("crates/unreviewed/Cargo.toml", '[package]\nname="unreviewed"\nversion="0.1.0"\nedition="2024"\nrust-version="1.89"\n')
        self.assert_error("crates/unreviewed/Cargo.toml: unregistered product manifest/config")

    def test_new_product_npm_manifest_must_be_registered(self):
        self.write_json("apps/another-frontend/package.json", {"name": "unreviewed", "dependencies": {"vue": "3.5.0"}})
        self.assert_error("apps/another-frontend/package.json: unregistered product manifest/config")

    def test_new_package_manager_lock_and_framework_config_are_detected(self):
        self.write("apps/desktop/yarn.lock", "# unreviewed manager\n")
        self.write("apps/desktop/next.config.mjs", "export default {};\n")
        self.assert_error("apps/desktop/yarn.lock: unregistered product manifest/config")
        self.assert_error("apps/desktop/next.config.mjs: unregistered product manifest/config")

    def test_actual_confirmed_design_source_change_fails(self):
        self.write(self.design, ":root { --text-primary: #00f; }\n")
        self.assert_error(f"{self.design}: design_sources SHA-256 mismatch")

    def test_locked_config_missing_is_not_pass(self):
        (self.root / "apps/desktop/tsconfig.json").unlink()
        self.assert_error("apps/desktop/tsconfig.json: locked manifests file is missing")

    def test_npm_resolved_version_drift_fails_after_hash_refresh(self):
        relative = "apps/desktop/package-lock.json"
        lock = json.loads((self.root / relative).read_text())
        lock["packages"]["node_modules/react"]["version"] = "20.0.0"
        self.write_json(relative, lock)
        self.refresh_hash(relative)
        self.assert_error("resolved package react does not match approved version 19")

    def test_cargo_resolved_version_drift_fails_after_hash_refresh(self):
        relative = "Cargo.lock"
        self.write(relative, 'version=4\n[[package]]\nname="tauri"\nversion="3.0.0"\n')
        self.refresh_hash(relative)
        self.assert_error("resolved crate tauri does not match approved version 2")

    def test_workspace_inheritance_and_cargo_package_rename_are_supported(self):
        relative = "apps/desktop/src-tauri/Cargo.toml"
        target = self.root / relative
        target.write_text(target.read_text().replace('tauri = { version = "2", features = [] }', 'renamed_tauri = { package = "tauri", version = "^2.9" }'))
        self.refresh_hash(relative)
        self.assertEqual(self.errors(), [])

    def compatibility_fixture(self):
        self.document["approved_stack"]["cargo"]["base64"] = "0.22.1"
        path = "apps/desktop/src-tauri/Cargo.toml"
        self.write(path, (self.root / path).read_text() + '\nbase64 = "=0.22.1"\n')
        self.refresh_hash(path)
        item = {"name": "base64", "version": "0.21.7", "source": "registry+https://github.com/rust-lang/crates.io-index", "checksum": "a" * 64, "reason": "Exact pre-existing dependency, separately reviewed"}
        self.document["transitive_cargo_compatibility"] = [item]
        self.write("Cargo.lock", (self.root / "Cargo.lock").read_text() + '\n[[package]]\nname="base64"\nversion="0.22.1"\n\n[[package]]\nname="base64"\nversion="0.21.7"\nsource="registry+https://github.com/rust-lang/crates.io-index"\nchecksum="' + "a" * 64 + '"\n')
        self.refresh_hash("Cargo.lock")
        self.save_lock()
        return item

    def test_exact_transitive_coexistence_preserves_direct_version_rule(self):
        self.compatibility_fixture()
        self.assertEqual(self.errors(), [])
        path = "apps/desktop/src-tauri/Cargo.toml"
        self.write(path, (self.root / path).read_text().replace('base64 = "=0.22.1"', 'base64 = "=0.21.7"'))
        self.refresh_hash(path)
        self.assert_error("crate base64 does not match approved version 0.22.1")

    def test_transitive_coexistence_rejects_checksum_source_and_version_drift(self):
        self.compatibility_fixture()
        original = (self.root / "Cargo.lock").read_text()
        for before, after in [("a" * 64, "b" * 64), ('version="0.21.7"', 'version="0.21.8"'), ('source="registry+https://github.com/rust-lang/crates.io-index"', 'source="git+https://example.invalid"')]:
            with self.subTest(after=after):
                self.write("Cargo.lock", original.replace(before, after))
                self.refresh_hash("Cargo.lock")
                self.assert_error("resolved crate base64 does not match approved version 0.22.1")

    def test_transitive_coexistence_malformed_unused_duplicate_entries_rejected(self):
        item = self.compatibility_fixture()
        self.document["transitive_cargo_compatibility"] = [item, item]
        self.save_lock()
        self.assert_error("duplicate package identity")
        self.document["transitive_cargo_compatibility"] = [{**item, "name": []}]
        self.save_lock()
        self.assert_error("expected reviewed exact registry identity")
        self.document["transitive_cargo_compatibility"] = [{**item, "version": "0.21.8"}]
        self.save_lock()
        self.assert_error("unused or nonmatching exact package identity")

    def test_rust_workspace_edition_change_fails_even_after_hash_refresh(self):
        target = self.root / "Cargo.toml"
        target.write_text(target.read_text().replace('edition = "2024"', 'edition = "2021"'))
        self.refresh_hash("Cargo.toml")
        self.assert_error("workspace.package.edition does not match approved 2024")
        self.assert_error("package.edition does not match approved 2024")

    def test_root_openspec_tooling_and_dependency_cache_are_not_product_manifests(self):
        self.write_json("package.json", {"name": "engineering-tools", "devDependencies": {"@fission-ai/openspec": "1.2.0"}})
        self.write_json("apps/desktop/node_modules/some-tool/package.json", {"name": "some-tool"})
        self.write("apps/desktop/src-tauri/target/generated/Cargo.toml", "not a product source manifest")
        self.assertEqual(self.errors(), [])

    def test_unsafe_paths_duplicate_entries_and_unknown_stack_shape_fail(self):
        self.document["manifests"].append({"path": "../outside/package.json", "sha256": "0" * 64})
        self.document["manifests"].append(self.document["manifests"][0])
        self.document["approved_stack"]["ignored_name"] = {}
        self.save_lock()
        self.assert_error("unsafe repository-relative path")
        self.assert_error("duplicate path Cargo.toml")
        self.assert_error("approved_stack: expected packages, cargo and rust objects")

    def test_malformed_cargo_table_is_reported_without_checker_crash(self):
        self.write("Cargo.toml", 'workspace = "invalid table"\n')
        self.refresh_hash("Cargo.toml")
        self.assert_error("Cargo.toml: workspace must be a TOML table")

    def test_cli_checks_real_files_and_uses_portable_json(self):
        success = subprocess.run([sys.executable, str(_SOURCE), "--root", str(self.root), "--json"], text=True, capture_output=True)
        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertEqual(json.loads(success.stdout)["status"], "pass")
        self.package["dependencies"]["vue"] = "3.5.0"
        self.save_package()
        failure = subprocess.run([sys.executable, str(_SOURCE), "--root", str(self.root), "--json"], text=True, capture_output=True)
        self.assertEqual(failure.returncode, 1, failure.stderr)
        self.assertEqual(json.loads(failure.stdout)["status"], "fail")
        self.assertNotIn(str(self.root), failure.stdout)


if __name__ == "__main__":
    unittest.main()
