"""Adversarial shape/identity tests; synthetic ZIPs never count as product runs."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import plistlib
import stat
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr
from unittest.mock import patch
import warnings
import zipfile

import package_identity as identity

_builder_spec = importlib.util.spec_from_file_location("luna_build_desktop", Path(__file__).resolve().parents[1] / "build_desktop.py")
builder = importlib.util.module_from_spec(_builder_spec)
_builder_spec.loader.exec_module(builder)


class PackageIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / ".build/inspection"
        self.artifact = self.root / ".build/candidate.zip"
        self.source = "a" * 64
        self.binary = bytes.fromhex("cffaedfe0c000001") + b"guard shape only, never executed"
        self.resource = b'{"guard":"only"}'
        for relative in ("native/core-runtime.lock.json", "resources/language/manifest.json"):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(self.resource)
        self.record = {
            "schema_version": 1, "kind": "luna_packaged_build_identity", "version": "0.2.0",
            "platform": "macos", "architecture": "arm64", "bundle_id": identity.BUNDLE_ID,
            "source_tree_sha256": self.source, "executable_entry": identity.EXECUTABLE_ENTRY,
            "executable_sha256": self.sha(self.binary),
            "core_manifest_sha256": self.sha(self.resource), "language_manifest_sha256": self.sha(self.resource),
        }
        self.members = {
            identity.EXECUTABLE_ENTRY: self.binary,
            "Luna.app/Contents/Info.plist": plistlib.dumps({
                "CFBundleIdentifier": identity.BUNDLE_ID, "CFBundleExecutable": "luna-reader",
                "CFBundleShortVersionString": "0.2.0",
            }),
            "Luna.app/Contents/Resources/core-runtime.lock.json": self.resource,
            "Luna.app/Contents/Resources/language/manifest.json": self.resource,
        }
        self.sealed = {key: self.record[key] for key in (
            "source_tree_sha256", "version", "core_manifest_sha256", "language_manifest_sha256")}
        self.sealed.update(schema_version=1, kind="luna_sealed_source_identity")
        sealed_bytes = json.dumps(self.sealed).encode()
        self.members["Luna.app/Contents/Resources/luna-source-identity.json"] = sealed_bytes
        self.record["sealed_source_identity_sha256"] = self.sha(sealed_bytes)

    @staticmethod
    def sha(data):
        return hashlib.sha256(data).hexdigest()

    def write(self, extra=None):
        self.artifact.parent.mkdir(parents=True, exist_ok=True)
        encoded = json.dumps(self.record).encode()
        with zipfile.ZipFile(self.artifact, "w") as archive:
            for name, data in self.members.items():
                archive.writestr(name, data)
            archive.writestr(identity.IDENTITY_ENTRY, encoded)
            if extra:
                extra(archive)
        self.outer = {
            "source_tree_sha256": self.source, "archive_sha256": self.sha(self.artifact.read_bytes()),
            "executable_sha256": self.record["executable_sha256"],
            "package_identity_sha256": self.sha(encoded),
            "sealed_source_identity_sha256": self.record["sealed_source_identity_sha256"],
        }
        self.write_outer()

    def write_outer(self):
        (self.artifact.parent / "build-manifest.json").write_text(json.dumps(self.outer))

    def inspect(self):
        # This mock is confined to synthetic unit fixtures. Formal inspection
        # always invokes the actual OS signature verifier on the extracted app.
        with patch.object(identity, "_verify_signature") as signature:
            result = identity.inspect_package(self.root, self.artifact, self.source, self.output)
            signature.assert_called_once()
            return result

    def test_valid_shape_requires_actual_signature_verifier(self):
        self.write()
        result = self.inspect()
        self.assertEqual(result["source_tree_sha256"], self.source)
        self.assertEqual(result["executable_sha256"], self.sha(self.binary))
        self.assertFalse(list(self.output.glob("package-inspection-*")))

    def test_previous_source_is_rejected_before_signature(self):
        self.record["source_tree_sha256"] = "b" * 64
        self.write()
        with self.assertRaisesRegex(ValueError, "package source identity"):
            self.inspect()

    def test_invalid_json_shapes_and_missing_versions_fail_closed(self):
        for value in (None, [], "not an object"):
            with self.subTest(outer=value):
                self.write()
                (self.artifact.parent / "build-manifest.json").write_text(json.dumps(value))
                with self.assertRaisesRegex(ValueError, "must be a JSON object"):
                    self.inspect()
        for value in (None, "", 2):
            with self.subTest(version=value):
                self.record["version"] = value
                self.write()
                with self.assertRaisesRegex(ValueError, "version must be a nonempty string"):
                    self.inspect()

    def test_arbitrary_text_file_is_not_a_package(self):
        self.write()
        self.artifact.write_bytes(b"not a product package")
        with self.assertRaisesRegex(ValueError, "invalid packaged build identity"):
            self.inspect()

    def test_relabelling_only_external_records_cannot_relabel_signed_source(self):
        self.sealed["source_tree_sha256"] = "b" * 64
        sealed_bytes = json.dumps(self.sealed).encode()
        self.members["Luna.app/Contents/Resources/luna-source-identity.json"] = sealed_bytes
        self.record["sealed_source_identity_sha256"] = self.sha(sealed_bytes)
        self.write()
        with self.assertRaisesRegex(ValueError, "signed source identity differs: source_tree_sha256"):
            self.inspect()

    def test_missing_or_changed_signed_source_identity_is_rejected(self):
        path = "Luna.app/Contents/Resources/luna-source-identity.json"
        del self.members[path]
        self.write()
        with self.assertRaisesRegex(ValueError, "invalid packaged build identity"):
            self.inspect()
        self.members[path] = json.dumps(self.sealed).encode() + b" "
        self.write()
        with self.assertRaisesRegex(ValueError, "signed source identity hash differs"):
            self.inspect()

    def test_executable_hash_is_computed_from_zip_bytes(self):
        self.members[identity.EXECUTABLE_ENTRY] = self.binary + b"changed"
        self.write()
        with self.assertRaisesRegex(ValueError, "executable bytes differ"):
            self.inspect()

    def test_wrong_executable_architecture_is_rejected(self):
        self.members[identity.EXECUTABLE_ENTRY] = b"PE32, not arm64 Mach-O"
        self.write()
        with self.assertRaisesRegex(ValueError, "not the registered arm64"):
            self.inspect()

    def test_outer_manifest_source_and_archive_are_bound(self):
        for key in ("source_tree_sha256", "archive_sha256", "executable_sha256", "package_identity_sha256"):
            with self.subTest(key=key):
                self.write()
                self.outer[key] = "0" * 64
                self.write_outer()
                with self.assertRaisesRegex(ValueError, "external build manifest differs"):
                    self.inspect()

    def test_wrong_resource_version_and_unrelated_app_are_rejected(self):
        self.members["Luna.app/Contents/Resources/language/manifest.json"] = b"wrong pinned resource"
        self.write()
        with self.assertRaisesRegex(ValueError, "pinned resource manifest differs"):
            self.inspect()
        self.members["Luna.app/Contents/Resources/language/manifest.json"] = self.resource
        self.record["bundle_id"] = "unrelated.app"
        self.write()
        with self.assertRaisesRegex(ValueError, "bundle/executable identity"):
            self.inspect()

    def test_unsafe_and_noncanonical_paths_are_rejected(self):
        for path in ("Luna.app/../escape", "/absolute", "Luna.app/./alias", "Luna.app//alias", "Luna.app\\escape"):
            with self.subTest(path=path):
                self.write(lambda archive: archive.writestr(path, b"unsafe"))
                with self.assertRaisesRegex(ValueError, "ZIP path"):
                    self.inspect()
                self.assertFalse((self.root / "escape").exists())

    def test_duplicate_and_symlink_members_are_rejected(self):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            self.write(lambda archive: archive.writestr(identity.EXECUTABLE_ENTRY, b"duplicate"))
        with self.assertRaisesRegex(ValueError, "duplicate ZIP"):
            self.inspect()
        link = zipfile.ZipInfo("Luna.app/link")
        link.external_attr = (stat.S_IFLNK | 0o777) << 16
        self.write(lambda archive: archive.writestr(link, b"../../outside"))
        with self.assertRaisesRegex(ValueError, "symlink members"):
            self.inspect()

    def test_invalid_signature_and_mid_inspection_change_reject(self):
        self.write()
        with patch.object(identity, "_verify_signature", side_effect=ValueError("signature rejected")):
            with self.assertRaisesRegex(ValueError, "signature rejected"):
                identity.inspect_package(self.root, self.artifact, self.source, self.output)
        def changed(_app):
            with self.artifact.open("ab") as handle:
                handle.write(b"changed after capture")
        with patch.object(identity, "_verify_signature", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "changed during identity"):
                identity.inspect_package(self.root, self.artifact, self.source, self.output)

    # All signing responses below are synthetic OS fixtures. They test rejection
    # decisions and cannot establish that a product was signed or authorized.
    def certificate_metadata(self):
        certificate = b"synthetic public DER for signing unit tests"
        return {
            "mode": "certificate", "identifier": identity.BUNDLE_ID, "team_identifier": "UNITTEST",
            "authorities": ["Synthetic Developer", "Synthetic Issuer"], "cdhash": "c" * 40,
            "designated_requirement": 'identifier "' + identity.BUNDLE_ID + '" and certificate leaf[subject.CN] = "Synthetic Developer"',
            "leaf_sha1": hashlib.sha1(certificate).hexdigest().upper(),
        }

    def signature_process(self, command, **_kwargs):
        metadata = self.actual_signature
        if "--extract-certificates" in command:
            # codesign's prefix is optional: a separate token is a target, not
            # the option value. Reproduce that rejection instead of accepting
            # a made-up subprocess contract in this fixture.
            return subprocess.CompletedProcess(command, 1, "", "certificate prefix was parsed as an input target")
        extraction = [argument for argument in command if argument.startswith("--extract-certificates=")]
        if extraction:
            prefix = extraction[0].split("=", 1)[1]
            Path(prefix + "0").write_bytes(b"synthetic public DER for signing unit tests")
        output = ""
        if "--verbose=4" in command:
            output = "\n".join([
                "Identifier=" + metadata["identifier"], "TeamIdentifier=" + (metadata["team_identifier"] or "not set"),
                "CDHash=" + metadata["cdhash"], *["Authority=" + value for value in metadata["authorities"]],
                *(["Signature=adhoc"] if metadata["mode"] == "ad-hoc" else []),
            ])
        if "--requirements" in command:
            output = "designated => " + metadata["designated_requirement"] + "\n"
        return subprocess.CompletedProcess(command, 0, "", output)

    def signed_fixture(self, mode="certificate"):
        self.actual_signature = self.certificate_metadata()
        if mode == "ad-hoc":
            self.actual_signature.update(mode=mode, team_identifier=None, authorities=[], leaf_sha1=None,
                                         designated_requirement='cdhash H"' + "c" * 40 + '"')
        selection = {"mode": mode, "expected_leaf_sha1": self.actual_signature["leaf_sha1"]}
        contract = identity.signing_contract(selection)
        self.sealed.update(contract)
        sealed_bytes = json.dumps(self.sealed).encode()
        self.members["Luna.app/Contents/Resources/luna-source-identity.json"] = sealed_bytes
        self.record.update(contract, signature_metadata=dict(self.actual_signature), sealed_source_identity_sha256=self.sha(sealed_bytes))
        self.write_signed_fixture()

    def write_signed_fixture(self):
        self.write()
        self.outer.update({key: self.record[key] for key in identity.SIGNING_FIELDS if key in self.record})
        if "signature_metadata" in self.record:
            self.outer["signature_metadata"] = self.record["signature_metadata"]
        self.outer["signature"] = self.actual_signature["mode"]
        self.write_outer()

    def inspect_signed_fixture(self):
        with patch.object(identity.subprocess, "run", side_effect=self.signature_process) as process:
            result = identity.inspect_package(self.root, self.artifact, self.source, self.output)
            self.assertTrue(any("--verify" in call.args[0] and "--deep" in call.args[0] and "--strict" in call.args[0]
                                for call in process.call_args_list))
            return result

    def test_identity_selection_prefers_cli_then_environment_then_unique_valid_identity(self):
        first, second = "1" * 40, "2" * 40
        response = subprocess.CompletedProcess([], 0, f'  1) {first} "First Developer"\n  2) {second} "Second Developer"\n     2 valid identities found\n', "")
        with patch.object(identity.subprocess, "run", return_value=response) as process:
            selected = identity.resolve_signing_identity("First Developer", environ={"APPLE_SIGNING_IDENTITY": second})
            self.assertEqual(selected["identity"], first)
            self.assertEqual(identity.resolve_signing_identity(environ={"APPLE_SIGNING_IDENTITY": second})["identity"], second)
            self.assertEqual(process.call_args.args[0], ["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"])
        response.stdout = f'  1) {first} "First Developer"\n  1 valid identities found\n'
        with patch.object(identity.subprocess, "run", return_value=response):
            self.assertEqual(identity.resolve_signing_identity(environ={"APPLE_SIGNING_IDENTITY": " "})["identity"], first)

    def test_identity_discovery_missing_multiple_invalid_and_ambiguous_fail_closed(self):
        first, second = "1" * 40, "2" * 40
        for stdout, requested, message in (
            ("  0 valid identities found\n", None, "no valid"),
            (f'  1) {first} "Developer"\n  2) {second} "Developer"\n', None, "multiple valid"),
            (f'  1) {first} "Developer"\n  2) {second} "Developer"\n', "Developer", "multiple valid"),
            (f'  1) {first} "Developer"\n', "Missing Developer", "no valid"),
            (f'  1) {first} "Developer" (CSSMERR_TP_CERT_EXPIRED)\n', "Developer", "no valid"),
        ):
            with self.subTest(requested=requested, stdout=stdout):
                with patch.object(identity.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, stdout, "")):
                    with self.assertRaisesRegex(ValueError, message):
                        identity.resolve_signing_identity(requested, environ={})

    def test_invalid_explicit_identity_never_falls_back_to_environment(self):
        for requested in ("Missing", "-", " "):
            with self.subTest(requested=requested):
                with patch.object(identity.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, '  1) ' + "1" * 40 + ' "Developer"\n', "")):
                    with self.assertRaises(ValueError):
                        identity.resolve_signing_identity(requested, environ={"APPLE_SIGNING_IDENTITY": "Developer"})
        with self.assertRaisesRegex(ValueError, "--ad-hoc explicitly"):
            identity.resolve_signing_identity(environ={"APPLE_SIGNING_IDENTITY": "-"})

    def test_ad_hoc_is_explicit_and_conflicting_cli_is_rejected(self):
        with patch.object(identity.subprocess, "run") as process:
            selection = identity.resolve_signing_identity(ad_hoc=True, environ={"APPLE_SIGNING_IDENTITY": "Inherited Developer"})
            self.assertEqual(selection, {"mode": "ad-hoc", "identity": "-", "expected_leaf_sha1": None})
            process.assert_not_called()
        with self.assertRaisesRegex(ValueError, "mutually exclusive"):
            identity.resolve_signing_identity("Developer", ad_hoc=True)
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as failure:
            builder.parse_args(["--signing-identity", "Developer", "--ad-hoc"])
        self.assertEqual(failure.exception.code, 2)

    def test_identity_discovery_os_failure_and_timeout_do_not_downgrade(self):
        for response in (subprocess.CompletedProcess([], 1, "", "security failed"),
                         subprocess.TimeoutExpired("security", 30), OSError("security unavailable")):
            with self.subTest(response=response):
                options = {"side_effect": response} if isinstance(response, Exception) else {"return_value": response}
                with patch.object(identity.subprocess, "run", **options):
                    with self.assertRaises(ValueError):
                        identity.resolve_signing_identity(environ={})

    def test_invalid_identity_fails_before_any_build_or_source_preparation(self):
        with patch.object(builder.sys, "argv", ["build_desktop.py"]), patch.object(builder.sys, "platform", "darwin"), \
                patch.object(builder, "ROOT", self.root.resolve()), \
                patch.object(builder, "resolve_signing_identity", side_effect=ValueError("missing signing identity")), \
                patch.object(builder, "run") as run, patch.object(builder, "source_files") as source:
            with self.assertRaisesRegex(ValueError, "missing signing identity"):
                builder.main()
            run.assert_not_called()
            source.assert_not_called()

    def test_tauri_environment_uses_resolved_identity_and_explicit_ad_hoc(self):
        inherited = {"APPLE_SIGNING_IDENTITY": "Inherited Developer", "PATH": "/unit-test"}
        for selected in ({"identity": "1" * 40}, {"identity": "-"}):
            environment = builder.tauri_signing_environment(selected, inherited)
            self.assertEqual(environment["APPLE_SIGNING_IDENTITY"], selected["identity"])
            self.assertEqual(environment["PATH"], inherited["PATH"])
        self.assertEqual(inherited["APPLE_SIGNING_IDENTITY"], "Inherited Developer")

    def test_failed_rebuild_preserves_prior_manifest_outside_canonical_success_path(self):
        output = self.root / "dist"
        output.mkdir()
        previous = output / "build-manifest.json"
        for stage in ("identity", "preparation"):
            with self.subTest(stage=stage):
                prior_bytes = (stage + ": prior success receipt, preserve exact bytes").encode()
                previous.write_bytes(prior_bytes)
                resolution = {"side_effect": ValueError("missing signing identity")} if stage == "identity" else {"return_value": {"identity": "1" * 40}}
                with patch.object(builder.sys, "argv", ["build_desktop.py"]), patch.object(builder.sys, "platform", "darwin"), \
                        patch.object(builder, "ROOT", self.root.resolve()), patch.object(builder, "source_files", return_value={}), \
                        patch.object(builder, "resolve_signing_identity", **resolution), \
                        patch.object(builder, "run", side_effect=subprocess.CalledProcessError(1, "prepare")):
                    with self.assertRaises(ValueError if stage == "identity" else subprocess.CalledProcessError):
                        builder.main()
                self.assertFalse(previous.exists())
                backups = list(output.glob("build-manifest.previous-*.json"))
                self.assertTrue(any(backup.read_bytes() == prior_bytes for backup in backups))
        self.assertEqual(len(backups), 2)

    def test_signing_failure_stops_without_ad_hoc_fallback(self):
        selection = {"identity": "1" * 40, "mode": "certificate", "expected_leaf_sha1": "1" * 40}
        with patch.object(identity.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "signing failed")) as process:
            with self.assertRaisesRegex(ValueError, "signing failed"):
                identity.sign_application(self.root / "Luna.app", selection)
            process.assert_called_once()
            command = process.call_args.args[0]
            self.assertEqual(command[command.index("--sign") + 1], selection["identity"])
            self.assertIn("--preserve-metadata=identifier,entitlements,flags,runtime", command)
            self.assertNotIn("requirements", command)

    def test_signature_readback_extracts_real_leaf_and_preserves_stable_requirement(self):
        self.actual_signature = self.certificate_metadata()
        selection = {"mode": "certificate", "identity": self.actual_signature["leaf_sha1"],
                     "expected_leaf_sha1": self.actual_signature["leaf_sha1"]}
        with patch.object(identity.subprocess, "run", side_effect=self.signature_process) as process:
            metadata = identity.sign_application(self.root / "Luna.app", selection)
            self.assertEqual(metadata, self.actual_signature)
            commands = [call.args[0] for call in process.call_args_list]
            self.assertEqual(commands[0][commands[0].index("--sign") + 1], selection["identity"])
            extraction = next(command for command in commands if any(argument.startswith("--extract-certificates=") for argument in command))
            certificate_prefix = Path(next(argument.split("=", 1)[1] for argument in extraction if argument.startswith("--extract-certificates=")))
            self.assertFalse(certificate_prefix.parent.exists())
            self.assertFalse(certificate_prefix.is_relative_to(self.root))

    def test_certificate_extraction_attaches_optional_prefix_to_the_same_cli_argument(self):
        self.actual_signature = self.certificate_metadata()
        app = self.root / "Luna.app"
        with patch.object(identity.subprocess, "run", side_effect=self.signature_process) as process:
            metadata = identity.read_signature_metadata(app)
            commands = [call.args[0] for call in process.call_args_list]
            extraction = next(command for command in commands if any(argument.startswith("--extract-certificates=") for argument in command))
            self.assertEqual(len(extraction), 4)
            self.assertEqual(extraction[:2], ["/usr/bin/codesign", "--display"])
            self.assertTrue(extraction[2].startswith("--extract-certificates="))
            self.assertEqual(extraction[3], str(app))
            self.assertNotIn("--extract-certificates", extraction)
            self.assertEqual(metadata["leaf_sha1"], self.actual_signature["leaf_sha1"])
            with self.assertRaisesRegex(ValueError, "prefix was parsed as an input target"):
                identity._codesign_output(["--display", "--extract-certificates", self.root / "certificate-", app])

    def test_signature_readback_rejects_old_cdhash_requirement_and_wrong_leaf(self):
        self.actual_signature = self.certificate_metadata()
        with patch.object(identity.subprocess, "run", side_effect=self.signature_process):
            selection = {"identity": "1" * 40, "mode": "certificate", "expected_leaf_sha1": "1" * 40}
            with self.assertRaisesRegex(ValueError, "leaf certificate differs"):
                identity.sign_application(self.root / "Luna.app", selection)
            self.actual_signature["designated_requirement"] = 'cdhash H"' + "c" * 40 + '"'
            with self.assertRaisesRegex(ValueError, "must not pin a cdhash"):
                identity.read_signature_metadata(self.root / "Luna.app")

    def test_new_certificate_and_explicit_ad_hoc_records_are_independently_read_back(self):
        for mode in ("certificate", "ad-hoc"):
            with self.subTest(mode=mode):
                self.signed_fixture(mode)
                result = self.inspect_signed_fixture()
                self.assertEqual(result["signature_metadata"], self.actual_signature)
                self.assertEqual(result["signing_mode"], mode)

    def test_each_actual_signature_metadata_field_is_checked(self):
        changes = {"mode": "ad-hoc", "identifier": "unrelated.app", "team_identifier": "WRONGTEAM",
                   "authorities": ["Wrong Developer"], "cdhash": "d" * 40,
                   "designated_requirement": "identifier \"unrelated.app\"", "leaf_sha1": "1" * 40}
        for key, changed in changes.items():
            with self.subTest(key=key):
                self.signed_fixture()
                self.record["signature_metadata"][key] = changed
                self.write_signed_fixture()
                with self.assertRaisesRegex(ValueError, "metadata differs from actual"):
                    self.inspect_signed_fixture()

    def test_metadata_deletion_and_external_only_claims_cannot_fall_back(self):
        for location in ("sealed", "record", "outer"):
            for key in (*identity.SIGNING_FIELDS, "signature_metadata"):
                if location == "sealed" and key == "signature_metadata":
                    continue
                with self.subTest(location=location, key=key):
                    self.signed_fixture()
                    record = getattr(self, location)
                    del record[key]
                    if location == "sealed":
                        sealed_bytes = json.dumps(self.sealed).encode()
                        self.members["Luna.app/Contents/Resources/luna-source-identity.json"] = sealed_bytes
                        self.record["sealed_source_identity_sha256"] = self.sha(sealed_bytes)
                        self.write_signed_fixture()
                    elif location == "record":
                        self.write_signed_fixture()
                    else:
                        self.write_outer()
                    with self.assertRaisesRegex(ValueError, "signature (contract|metadata) is incomplete"):
                        self.inspect_signed_fixture()

    def test_sealed_marker_rejects_stripping_both_external_signature_records(self):
        self.signed_fixture()
        for key in (*identity.SIGNING_FIELDS, "signature_metadata"):
            del self.record[key]
        self.write()
        with self.assertRaisesRegex(ValueError, "signature contract is incomplete"):
            self.inspect_signed_fixture()

    def test_each_signature_metadata_field_is_mandatory_in_both_external_records(self):
        for location in ("record", "outer"):
            for key in identity.SIGNATURE_FIELDS:
                with self.subTest(location=location, key=key):
                    self.signed_fixture()
                    del getattr(self, location)["signature_metadata"][key]
                    if location == "record":
                        self.write_signed_fixture()
                    else:
                        self.write_outer()
                    with self.assertRaisesRegex(ValueError, "signature metadata is incomplete"):
                        self.inspect_signed_fixture()

    def test_signature_contract_changes_and_unselected_actual_leaf_are_rejected(self):
        for key, value in (("signature_contract", "unknown-contract"), ("signing_mode", "ad-hoc"),
                           ("expected_leaf_sha1", "1" * 40)):
            with self.subTest(key=key):
                self.signed_fixture()
                self.record[key] = value
                self.write_signed_fixture()
                with self.assertRaisesRegex(ValueError, "signature contract (is unsupported|differs)"):
                    self.inspect_signed_fixture()
        self.signed_fixture()
        self.sealed["expected_leaf_sha1"] = "1" * 40
        sealed_bytes = json.dumps(self.sealed).encode()
        self.members["Luna.app/Contents/Resources/luna-source-identity.json"] = sealed_bytes
        self.record.update(expected_leaf_sha1="1" * 40, sealed_source_identity_sha256=self.sha(sealed_bytes))
        self.write_signed_fixture()
        with self.assertRaisesRegex(ValueError, "leaf certificate differs"):
            self.inspect_signed_fixture()

    def test_outer_only_metadata_and_forged_ad_hoc_label_are_rejected(self):
        self.write()
        self.outer["signature_metadata"] = self.certificate_metadata()
        self.write_outer()
        with self.assertRaisesRegex(ValueError, "signature contract is incomplete"):
            self.inspect()
        self.signed_fixture()
        self.outer["signature"] = "ad-hoc"
        self.write_outer()
        with self.assertRaisesRegex(ValueError, "metadata differs between"):
            self.inspect_signed_fixture()

    def test_new_signature_verification_failure_and_changed_actual_signer_reject(self):
        self.signed_fixture()
        with patch.object(identity.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "invalid resource seal")):
            with self.assertRaisesRegex(ValueError, "resource signature is invalid"):
                identity.inspect_package(self.root, self.artifact, self.source, self.output)
        self.signed_fixture("ad-hoc")
        self.actual_signature = self.certificate_metadata()
        with self.assertRaisesRegex(ValueError, "signing mode differs"):
            self.inspect_signed_fixture()


if __name__ == "__main__":
    unittest.main()
