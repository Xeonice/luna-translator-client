"""Deterministic gate tests: no Cargo/network/product execution required."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("rust-boundaries.py")
SPEC = importlib.util.spec_from_file_location("rust_boundaries", SCRIPT)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class RustBoundariesTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def workspace(self, members, dependencies=""):
        self.write("Cargo.toml", f"[workspace]\nmembers={json.dumps(members)}\n[workspace.dependencies]\n{dependencies}\n")

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def package(self, directory, name, dependencies=""):
        self.write(f"{directory}/Cargo.toml", f'[package]\nname="{name}"\nversion="0.1.0"\n{dependencies}\n')

    def rules(self):
        return [item["rule"] for item in GUARD.check(self.root)["findings"]]

    def test_offline_translation_worker_is_independent_of_business_network_and_storage(self):
        self.workspace(["crates/*"])
        self.package("crates/app-core", "app-core", '[dependencies]\nserde="1"')
        self.package("crates/offline-translation-worker", "offline-translation-worker", '[dependencies]\nserde_json="1"\nsha2="0.10"')
        self.assertEqual(self.rules(), [])
        for dependency in ('reqwest="0.13"', 'rusqlite="0.40"', 'tauri="2"', 'app-core={path="../app-core"}'):
            with self.subTest(dependency=dependency):
                self.package("crates/offline-translation-worker", "offline-translation-worker", '[dependencies]\n' + dependency)
                self.assertTrue(self.rules(), "worker must not become a second business/network/storage client")

    def test_valid_core_infra_composition_and_native_broker(self):
        self.workspace(["crates/*", "apps/desktop/src-tauri"])
        self.package("crates/app-core", "app-core", '[dependencies]\nserde="1"')
        self.package("crates/hook-contracts", "hook-contracts", '[dependencies]\nserde="1"')
        self.package("crates/hook-client", "hook-client", '[dependencies]\nhook-contracts={path="../hook-contracts"}')
        self.package("crates/app-infra", "app-infra", '[dependencies]\napp-core={path="../app-core"}\nhook-client={path="../hook-client"}\nreqwest="0.13"\nrusqlite="0.37"')
        self.package("crates/lunahook-host", "lunahook-host", '[dependencies]\nhook-contracts={path="../hook-contracts"}\n[target.\'cfg(windows)\'.dependencies]\nwindows-sys="0.59"\nlibloading="0.8"')
        self.package("apps/desktop/src-tauri", "luna-reader", '[dependencies]\napp-core={path="../../../crates/app-core"}\napp-infra={path="../../../crates/app-infra"}\ntauri="2"')
        # Desktop legitimately composes infra; restrictions apply to its direct
        # adapters, not infra's transitive HTTP/SQL implementation.
        result = GUARD.check(self.root)
        desktop = [f for f in result["findings"] if f.get("package") == "luna-reader"]
        self.assertFalse(desktop, desktop)
        self.assertEqual(result["findings"], [])

    def test_core_cannot_depend_on_io_aliased_package(self):
        self.workspace(["crates/app-core"])
        self.package("crates/app-core", "app-core", '[dependencies]\ntransport={package="reqwest",version="0.13"}')
        result = GUARD.check(self.root)
        self.assertEqual(result["exit_code"], 1)
        self.assertEqual(result["findings"][0]["dependency"], "reqwest")
        self.assertIn("transport", result["findings"][0]["evidence"])

    def test_target_dev_build_optional_all_checked(self):
        self.workspace(["crates/app-core"])
        self.package("crates/app-core", "app-core", '''[target.'cfg(windows)'.dependencies]
db={package="rusqlite",version="0.37",optional=true}
[dev-dependencies]
mock_db={package="sqlx",version="0.8"}
[build-dependencies]
bindgen="0.72"
[dependencies]
ui={package="tauri",version="2",optional=true}
''')
        findings = GUARD.check(self.root)["findings"]
        self.assertEqual({f["dependency"] for f in findings}, {"rusqlite", "sqlx", "bindgen", "tauri"})

    def test_inherited_alias_resolves_canonical_package_and_root_path(self):
        self.workspace(["crates/app-core"], 'net={package="reqwest",version="0.13"}')
        self.package("crates/app-core", "app-core", '[dependencies]\nnet.workspace=true')
        findings = GUARD.check(self.root)["findings"]
        self.assertEqual(findings[0]["dependency"], "reqwest")
        self.workspace(["crates/app-core", "crates/app-infra"], 'infra={package="app-infra",path="crates/app-infra"}')
        self.package("crates/app-core", "app-core", '[dependencies]\ninfra.workspace=true')
        self.package("crates/app-infra", "app-infra")
        self.assertIn("RUST-LAYER-DIRECTION", self.rules())

    def test_reverse_direction_and_process_linkage_are_forbidden(self):
        self.workspace(["crates/*"])
        self.package("crates/app-core", "app-core", '[dependencies]\napp-infra={path="../app-infra"}')
        self.package("crates/app-infra", "app-infra", '[dependencies]\nlunahook-host={path="../lunahook-host"}')
        self.package("crates/lunahook-host", "lunahook-host")
        self.assertEqual(self.rules().count("RUST-LAYER-DIRECTION"), 2)

    def test_registry_first_party_alias_cannot_bypass_layer_direction(self):
        self.workspace(["crates/app-core"])
        self.package("crates/app-core", "app-core", '[dependencies]\nbackend={package="app-infra",version="1"}')
        self.assertIn("RUST-LAYER-DIRECTION", self.rules())

    def test_manifest_identity_cannot_be_hidden_by_path_alias(self):
        self.workspace(["crates/app-core", "crates/app-infra"])
        self.package("crates/app-core", "app-core", '[dependencies]\nserde={path="../app-infra"}')
        self.package("crates/app-infra", "app-infra")
        self.assertIn("RUST-DEPENDENCY-IDENTITY", self.rules())
        self.assertIn("RUST-LAYER-DIRECTION", self.rules())

    def test_broker_and_language_worker_reject_network_ui_database(self):
        self.workspace(["crates/*"])
        for name in ("lunahook-host", "language-worker"):
            self.package(f"crates/{name}", name, '[dependencies]\na={package="tauri",version="2"}\nb={package="rusqlite",version="0.37"}\nc={package="reqwest",version="0.13"}')
        self.assertEqual(self.rules().count("RUST-LAYER-DEPENDENCY"), 6)

    def test_worker_contracts_must_disable_default_features(self):
        self.workspace(["crates/*"])
        self.package("crates/app-core", "app-core")
        self.package("crates/language-worker", "language-worker", '[dependencies]\napp-core={path="../app-core"}')
        self.assertIn("RUST-WORKER-CONTRACT-FEATURE", self.rules())
        self.package("crates/language-worker", "language-worker", '[dependencies]\napp-core={path="../app-core",default-features=false,features=["language-contracts"]}\nmecab="0.1"')
        self.assertEqual(self.rules(), [])

    def test_local_wrapper_does_not_hide_forbidden_dependency(self):
        self.workspace(["crates/app-core", "crates/transport"])
        self.package("crates/app-core", "app-core", '[dependencies]\ntransport={path="../transport"}')
        self.package("crates/transport", "transport", '[dependencies]\nreqwest="0.13"')
        findings = GUARD.check(self.root)["findings"]
        self.assertIn("RUST-LAYER-UNKNOWN", {f["rule"] for f in findings})
        transitive = [f for f in findings if f.get("dependency") == "reqwest"]
        self.assertEqual(transitive[0]["via"], ["app-core", "transport", "reqwest"])

    def test_workspace_default_features_cannot_be_disabled_by_inheritor(self):
        self.workspace(["crates/*"], 'app-core={path="crates/app-core",features=["application"]}')
        self.package("crates/app-core", "app-core")
        self.package("crates/language-worker", "language-worker", '[dependencies]\napp-core={workspace=true,default-features=false,features=["language-contracts"]}')
        self.assertIn("RUST-WORKER-CONTRACT-FEATURE", self.rules())

    def test_workspace_exclude_and_glob(self):
        self.write("Cargo.toml", '[workspace]\nmembers=["crates/*"]\nexclude=["crates/ignored"]')
        self.package("crates/app-core", "app-core")
        self.package("crates/ignored", "ignored", '[dependencies]\nreqwest="0.13"')
        result = GUARD.check(self.root)
        self.assertEqual(len(result["checked_packages"]), 1)
        self.assertEqual(result["findings"], [])

    def test_bad_manifest_and_missing_member_fail_closed(self):
        self.workspace(["crates/missing"])
        self.assertIn("RUST-MEMBER", self.rules())
        self.write("Cargo.toml", "[broken")
        self.assertEqual(GUARD.check(self.root)["exit_code"], 2)

    def test_cli_json_and_exit_code(self):
        self.workspace(["crates/app-core"])
        self.package("crates/app-core", "app-core", '[dependencies]\nreqwest="0.13"')
        process = subprocess.run([sys.executable, str(SCRIPT), "--json", "--root", str(self.root)], capture_output=True, text=True)
        self.assertEqual(process.returncode, 1)
        self.assertEqual(json.loads(process.stdout)["status"], "failed")


if __name__ == "__main__":
    unittest.main()
