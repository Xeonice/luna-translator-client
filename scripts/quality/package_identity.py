"""Inspect a real macOS ZIP against this source and its sealed application.

The source identity is inside the signed app resource seal. The executable hash
is in a ZIP sibling entry to avoid a recursive signature hash. Actual signature
metadata is local build provenance, not authenticated distribution or release approval.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import plistlib
import re
import shutil
import stat
import subprocess
import tempfile
import zipfile

IDENTITY_ENTRY = "luna-build-identity.json"
APP_PREFIX = "Luna.app/"
EXECUTABLE_ENTRY = APP_PREFIX + "Contents/MacOS/luna-reader"
BUNDLE_ID = "com.lunatranslator.reader"
MAX_EXPANDED_BYTES = 4 * 1024**3
SIGNATURE_CONTRACT = "luna_macos_signature_v1"
SIGNING_FIELDS = ("signature_contract", "signing_mode", "expected_leaf_sha1")
SIGNATURE_FIELDS = {"mode", "identifier", "team_identifier", "authorities", "cdhash",
                    "designated_requirement", "leaf_sha1"}


def resolve_signing_identity(signing_identity: str | None = None, ad_hoc: bool = False,
                             environ: dict | None = None) -> dict:
    """Resolve one valid local identity before building; never downgrade failures."""
    if ad_hoc:
        if signing_identity is not None:
            raise ValueError("--signing-identity and --ad-hoc are mutually exclusive")
        return {"mode": "ad-hoc", "identity": "-", "expected_leaf_sha1": None}
    environment = os.environ if environ is None else environ
    requested = signing_identity if signing_identity is not None else environment.get("APPLE_SIGNING_IDENTITY", "").strip() or None
    if requested is not None:
        requested = requested.strip()
        if not requested or requested == "-":
            raise ValueError("a development signing identity is required; use --ad-hoc explicitly for ad-hoc signing")
    try:
        process = subprocess.run(["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"],
                                 text=True, capture_output=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValueError("could not discover valid code signing identities") from error
    if process.returncode:
        raise ValueError("valid code signing identity discovery failed: " + process.stderr[:1000])
    identities = {}
    for line in process.stdout.splitlines():
        match = re.fullmatch(r'\s*\d+\)\s+([0-9A-Fa-f]{40})\s+"(.*)"\s*', line)
        if match:
            identities[match[1].upper()] = match[2]
    matches = [sha for sha, name in identities.items()
               if requested is None or requested.upper() == sha or requested == name]
    if not matches:
        raise ValueError("no valid code signing identity matches the requested identity" if requested is not None
                         else "no valid code signing identity; install one or use --ad-hoc explicitly")
    if len(matches) != 1:
        raise ValueError("multiple valid code signing identities; select one with --signing-identity")
    return {"mode": "certificate", "identity": matches[0], "expected_leaf_sha1": matches[0]}


def signing_contract(selection: dict) -> dict:
    return {"signature_contract": SIGNATURE_CONTRACT, "signing_mode": selection["mode"],
            "expected_leaf_sha1": selection["expected_leaf_sha1"]}


def _codesign_output(arguments: list) -> str:
    try:
        process = subprocess.run(["/usr/bin/codesign", *map(str, arguments)], text=True,
                                 capture_output=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValueError("code signature inspection or signing could not complete") from error
    if process.returncode:
        raise ValueError("code signature inspection or signing failed: " + process.stderr[:1000])
    return process.stdout + "\n" + process.stderr


def read_signature_metadata(app: Path) -> dict:
    """Read OS-verified signature and public leaf certificate, never a private key."""
    _verify_signature(app)
    display = _codesign_output(["--display", "--verbose=4", app])
    def one(key):
        values = [line[len(key) + 1:] for line in display.splitlines() if line.startswith(key + "=")]
        if len(values) != 1 or not values[0]:
            raise ValueError("actual signature is missing a unique " + key)
        return values[0]
    identifier, team, cdhash = one("Identifier"), one("TeamIdentifier"), one("CDHash")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", cdhash):
        raise ValueError("actual signature has an invalid CDHash")
    authorities = [line.removeprefix("Authority=") for line in display.splitlines() if line.startswith("Authority=")]
    ad_hoc = "Signature=adhoc" in display.splitlines()
    if (ad_hoc and authorities) or (not ad_hoc and (not authorities or not all(authorities))):
        raise ValueError("actual signature mode and authority chain are inconsistent")
    requirements = _codesign_output(["--display", "--requirements", "-", app])
    designated = [line.removeprefix("designated => ").strip() for line in requirements.splitlines()
                  if line.startswith("designated => ")]
    if len(designated) != 1 or not designated[0]:
        raise ValueError("actual signature lacks a unique designated requirement")
    leaf_sha1 = None
    if not ad_hoc:
        if re.search(r"\bcdhash\b", designated[0], re.IGNORECASE):
            raise ValueError("certificate signature designated requirement must not pin a cdhash")
        with tempfile.TemporaryDirectory(prefix="luna-public-signing-certificate-") as temporary:
            prefix = Path(temporary) / "certificate-"
            _codesign_output(["--display", "--extract-certificates=" + str(prefix), app])
            leaf = Path(str(prefix) + "0")
            if not leaf.is_file() or not leaf.stat().st_size:
                raise ValueError("actual signature has no extracted leaf certificate")
            leaf_sha1 = hashlib.sha1(leaf.read_bytes()).hexdigest().upper()
    return {"mode": "ad-hoc" if ad_hoc else "certificate", "identifier": identifier,
            "team_identifier": None if team == "not set" else team, "authorities": authorities,
            "cdhash": cdhash.lower(), "designated_requirement": designated[0], "leaf_sha1": leaf_sha1}


def _match_signature_contract(metadata: dict, contract: dict) -> None:
    if metadata["identifier"] != BUNDLE_ID or metadata["mode"] != contract["signing_mode"]:
        raise ValueError("actual signature identifier or signing mode differs from signed contract")
    if metadata["leaf_sha1"] != contract["expected_leaf_sha1"]:
        raise ValueError("actual signature leaf certificate differs from selected signing identity")


def sign_application(app: Path, selection: dict) -> dict:
    # Regenerate requirements for the selected identity; retaining the linker's
    # ad-hoc cdhash requirement would make identity change with every executable.
    _codesign_output(["--force", "--sign", selection["identity"],
                      "--preserve-metadata=identifier,entitlements,flags,runtime", app])
    metadata = read_signature_metadata(app)
    _match_signature_contract(metadata, signing_contract(selection))
    return metadata


def _check_signature_records(sealed: dict, identity: dict, outer: dict) -> bool:
    """A sealed marker prevents stripping both external records into legacy mode."""
    records = (sealed, identity, outer)
    if not any(key in record for record in records for key in (*SIGNING_FIELDS, "signature_metadata")):
        return False
    for record in records:
        if any(key not in record for key in SIGNING_FIELDS):
            raise ValueError("package signature contract is incomplete")
        if record["signature_contract"] != SIGNATURE_CONTRACT:
            raise ValueError("package signature contract is unsupported")
        if any(record[key] != sealed[key] for key in SIGNING_FIELDS):
            raise ValueError("package signature contract differs from signed source seal")
    mode, leaf = sealed["signing_mode"], sealed["expected_leaf_sha1"]
    if mode not in ("certificate", "ad-hoc") or (mode == "ad-hoc" and leaf is not None):
        raise ValueError("package signature contract has an invalid signing mode or leaf")
    if mode == "certificate" and (not isinstance(leaf, str) or not re.fullmatch(r"[0-9A-F]{40}", leaf)):
        raise ValueError("package signature contract has an invalid signing leaf")
    for record in (identity, outer):
        metadata = record.get("signature_metadata")
        if not isinstance(metadata, dict) or set(metadata) != SIGNATURE_FIELDS:
            raise ValueError("package signature metadata is incomplete")
    if identity["signature_metadata"] != outer["signature_metadata"] or outer.get("signature") != mode:
        raise ValueError("package signature metadata differs between external records")
    return True


def _sha(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def _verify_signature(app: Path) -> None:
    try:
        process = subprocess.run(
            ["/usr/bin/codesign", "--verify", "--deep", "--strict", str(app)],
            text=True, capture_output=True, timeout=60,
        )
    except subprocess.TimeoutExpired as error:
        raise ValueError("package signature verification exceeded its inspection budget") from error
    if process.returncode:
        raise ValueError("package application resource signature is invalid: " + process.stderr[:1000])


def inspect_package(root: Path, artifact: Path, expected_source: str, output: Path) -> dict:
    """Reject stale, unrelated, changed or unsigned packages before test commands."""
    root, artifact = Path(root).resolve(), Path(artifact).resolve()
    output = Path(output).resolve()
    if not output.is_relative_to(root / ".build"):
        raise ValueError("package inspection output must be in repository .build")
    artifact_hash = _sha(artifact)
    outer_path = artifact.parent / "build-manifest.json"
    try:
        if outer_path.is_symlink():
            raise ValueError("external package manifest must not be a symlink")
        outer_hash = _sha(outer_path)
        outer = json.loads(outer_path.read_text())
        if not isinstance(outer, dict):
            raise ValueError("external build manifest must be a JSON object")
        with zipfile.ZipFile(artifact) as archive:
            members = archive.infolist()
            names = [member.filename for member in members]
            if len(names) != len(set(names)):
                raise ValueError("package contains duplicate ZIP member identities")
            if sum(member.file_size for member in members) > MAX_EXPANDED_BYTES:
                raise ValueError("package exceeds the bounded expanded inspection size")
            for member in members:
                path = PurePosixPath(member.filename)
                if path.is_absolute() or ".." in path.parts or "\\" in member.filename:
                    raise ValueError("package contains an unsafe ZIP path")
                if member.filename.rstrip("/") != str(path):
                    raise ValueError("package contains a noncanonical ZIP path")
                if stat.S_ISLNK(member.external_attr >> 16):
                    raise ValueError("registered standalone package must not contain symlink members")
                if not (member.filename == IDENTITY_ENTRY or member.filename.startswith((APP_PREFIX, "__MACOSX/"))):
                    raise ValueError("package contains an unrelated top-level member")
            identity_info = archive.getinfo(IDENTITY_ENTRY)
            if identity_info.file_size > 128 * 1024:
                raise ValueError("package build identity is unexpectedly large")
            identity_bytes = archive.read(identity_info)
            identity = json.loads(identity_bytes)
            if not isinstance(identity, dict):
                raise ValueError("package build identity must be a JSON object")
            if not isinstance(identity.get("version"), str) or not identity["version"].strip():
                raise ValueError("package version must be a nonempty string")
            if identity.get("schema_version") != 1 or identity.get("kind") != "luna_packaged_build_identity":
                raise ValueError("package has no registered build identity contract")
            if identity.get("source_tree_sha256") != expected_source:
                raise ValueError("package source identity does not match this acceptance source")
            if identity.get("platform") != "macos" or identity.get("architecture") != "arm64":
                raise ValueError("package platform or architecture differs from registered candidate")
            if identity.get("bundle_id") != BUNDLE_ID or identity.get("executable_entry") != EXECUTABLE_ENTRY:
                raise ValueError("package bundle/executable identity differs from registered product")
            for key, expected in (("source_tree_sha256", expected_source), ("archive_sha256", artifact_hash),
                                  ("executable_sha256", identity.get("executable_sha256"))):
                if outer.get(key) != expected:
                    raise ValueError("external build manifest differs from package: " + key)
            if outer.get("package_identity_sha256") != hashlib.sha256(identity_bytes).hexdigest():
                raise ValueError("external build manifest differs from embedded identity hash")
            output.mkdir(parents=True, exist_ok=True)
            with tempfile.TemporaryDirectory(prefix="package-inspection-", dir=output) as temporary:
                directory = Path(temporary)
                for member in members:
                    if not member.filename.startswith(APP_PREFIX):
                        continue
                    target = directory.joinpath(*PurePosixPath(member.filename).parts)
                    if member.is_dir():
                        target.mkdir(parents=True, exist_ok=True)
                        continue
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with archive.open(member) as source, target.open("xb") as destination:
                        shutil.copyfileobj(source, destination, length=1024 * 1024)
                    mode = (member.external_attr >> 16) & 0o777
                    if mode:
                        target.chmod(mode)
                executable = directory / EXECUTABLE_ENTRY
                with executable.open("rb") as handle:
                    header = handle.read(8)
                if header != bytes.fromhex("cffaedfe0c000001"):
                    raise ValueError("package executable is not the registered arm64 Mach-O")
                executable_hash = _sha(executable)
                if executable_hash != identity.get("executable_sha256"):
                    raise ValueError("package executable bytes differ from build identity")
                try:
                    info = plistlib.loads((directory / APP_PREFIX / "Contents/Info.plist").read_bytes())
                except Exception as error:
                    raise ValueError("package Info.plist cannot be parsed") from error
                if not isinstance(info, dict):
                    raise ValueError("package Info.plist must be a dictionary")
                if info.get("CFBundleIdentifier") != BUNDLE_ID or info.get("CFBundleExecutable") != "luna-reader":
                    raise ValueError("package Info.plist differs from product identity")
                if info.get("CFBundleShortVersionString") != identity.get("version"):
                    raise ValueError("package Info.plist version differs from build identity")
                resources = directory / APP_PREFIX / "Contents/Resources"
                sealed_path = resources / "luna-source-identity.json"
                sealed_bytes = sealed_path.read_bytes()
                sealed_hash = hashlib.sha256(sealed_bytes).hexdigest()
                sealed = json.loads(sealed_bytes)
                if not isinstance(sealed, dict):
                    raise ValueError("package signed source identity must be a JSON object")
                if sealed.get("schema_version") != 1 or sealed.get("kind") != "luna_sealed_source_identity":
                    raise ValueError("package lacks a signed source identity contract")
                if identity.get("sealed_source_identity_sha256") != sealed_hash or outer.get("sealed_source_identity_sha256") != sealed_hash:
                    raise ValueError("package signed source identity hash differs from external records")
                for key in ("source_tree_sha256", "version", "core_manifest_sha256", "language_manifest_sha256"):
                    if sealed.get(key) != identity.get(key):
                        raise ValueError("package signed source identity differs: " + key)
                for packaged, local, key in (
                    (resources / "core-runtime.lock.json", root / "native/core-runtime.lock.json", "core_manifest_sha256"),
                    (resources / "language/manifest.json", root / "resources/language/manifest.json", "language_manifest_sha256"),
                ):
                    if _sha(packaged) != _sha(local) or _sha(packaged) != identity.get(key):
                        raise ValueError("package pinned resource manifest differs: " + key)
                new_signature_contract = _check_signature_records(sealed, identity, outer)
                signature_metadata = None
                if new_signature_contract:
                    signature_metadata = read_signature_metadata(directory / "Luna.app")
                    _match_signature_contract(signature_metadata, sealed)
                    if signature_metadata != identity["signature_metadata"]:
                        raise ValueError("package signature metadata differs from actual application signature")
                else:
                    _verify_signature(directory / "Luna.app")
            if _sha(artifact) != artifact_hash:
                raise ValueError("package changed during identity inspection")
            if _sha(outer_path) != outer_hash:
                raise ValueError("external package manifest changed during identity inspection")
    except (OSError, KeyError, RuntimeError, zipfile.BadZipFile, json.JSONDecodeError, plistlib.InvalidFileException) as error:
        raise ValueError("invalid packaged build identity: " + str(error)) from error
    result = {
        "kind": "inspected_luna_packaged_build_identity", "source_tree_sha256": expected_source,
        "archive_sha256": artifact_hash, "executable_sha256": executable_hash,
        "package_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
        "external_build_manifest_sha256": outer_hash,
        "sealed_source_identity_sha256": sealed_hash,
        "bundle_resource_signature_verified": True, "version": identity["version"],
        "scope": "Same-source local development package identity; not release approval or authenticated distribution.",
    }
    if new_signature_contract:
        result.update({key: sealed[key] for key in SIGNING_FIELDS})
        result["signature_metadata"] = signature_metadata
    return result
