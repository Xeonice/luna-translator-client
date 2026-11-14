#!/usr/bin/env python3
"""Build a self-contained macOS release app around the pinned Windows Hook host."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
import time
import zipfile
sys.path.insert(0, str(Path(__file__).resolve().parent / "quality"))
from gate_core import safe_path, source_files
from package_identity import resolve_signing_identity, sign_application, signing_contract


ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / "apps/desktop"
TARGET = "x86_64-pc-windows-gnullvm"


def run(command, cwd=ROOT, env=None):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(list(map(str, command)), cwd=cwd, env=env, check=True)


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-windows-build", action="store_true")
    parser.add_argument("--no-install", action="store_true")
    parser.add_argument("--output-root", default="dist", help="Repository-relative output directory; explicit candidate outputs must be fresh")
    signing = parser.add_mutually_exclusive_group()
    signing.add_argument("--signing-identity", help="Exact valid code signing identity name or SHA1 (overrides APPLE_SIGNING_IDENTITY)")
    signing.add_argument("--ad-hoc", action="store_true", help="Explicitly create an ad-hoc development candidate")
    return parser, parser.parse_args(argv)


def tauri_signing_environment(selection, environ=None):
    environment = dict(os.environ if environ is None else environ)
    # Explicit ad-hoc overrides an inherited certificate as deliberately as an
    # explicit certificate overrides an inherited identity name.
    environment["APPLE_SIGNING_IDENTITY"] = selection["identity"]
    return environment


def main():
    parser, args = parse_args()
    if sys.platform != "darwin":
        parser.error("this packaging entry point builds the macOS .app on macOS")
    output = safe_path(ROOT, args.output_root)
    if not (output.is_relative_to(ROOT / ".build") or output == ROOT / "dist"):
        parser.error("package output must be dist or a dedicated directory under .build")
    if args.output_root != "dist" and output.exists() and any(output.iterdir()):
        parser.error("explicit candidate output already contains files; preserve it and choose a fresh directory")
    previous_manifest = output / "build-manifest.json"
    if previous_manifest.exists() or previous_manifest.is_symlink():
        # Preserve the prior receipt, but a failed rebuild must not expose it as
        # this attempt's successful canonical manifest.
        previous_manifest.rename(output / f"build-manifest.previous-{time.time_ns()}.json")
    selection = resolve_signing_identity(args.signing_identity, args.ad_hoc)
    source_before = source_files(ROOT)
    build_started_at_ms = time.time_ns() // 1_000_000
    preparation = [sys.executable, str(ROOT / "scripts/resources/prepare_core_runtime.py")]
    if args.skip_windows_build:
        preparation.append("--skip-windows-build")
    run(preparation)
    resources = DESKTOP / "src-tauri/resources"
    run(["cargo", "build", "--release", "--locked", "-p", "language-worker"])
    run([sys.executable, ROOT / "scripts/resources/prepare_language_resources.py", "--resources", resources])
    manifest = json.loads((ROOT / "native/core-runtime.lock.json").read_text())
    if not args.no_install:
        run(["npm", "ci"], cwd=DESKTOP)
    if source_files(ROOT) != source_before:
        raise RuntimeError("Source or staged inputs changed during preparation; inspect and rerun on stable sources")
    run(["npm", "run", "tauri", "--", "build", "--bundles", "app"], cwd=DESKTOP,
        env=tauri_signing_environment(selection))
    source_after = source_files(ROOT)
    if source_before != source_after:
        changed = sorted(k for k in source_before.keys() | source_after.keys() if source_before.get(k) != source_after.get(k))
        raise RuntimeError("Source changed during packaged build; inspect and rebuild: " + ", ".join(changed))
    app = ROOT / "target/release/bundle/macos/Luna.app"
    output.mkdir(parents=True, exist_ok=True)
    destination = output / "Luna.app"
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(app, destination, symlinks=True)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    source_hash = hashlib.sha256(json.dumps(source_before, sort_keys=True).encode()).hexdigest()
    language_hash = hashlib.file_digest((resources / "language/manifest.json").open("rb"), "sha256").hexdigest()
    core_hash = hashlib.file_digest((resources / "core-runtime.lock.json").open("rb"), "sha256").hexdigest()
    # Source identity belongs inside the resource seal. Changing only the two
    # external JSON records must never relabel an older signed app as this build.
    sealed_identity = {
        "schema_version": 1, "kind": "luna_sealed_source_identity",
        "source_tree_sha256": source_hash, "version": version,
        "core_manifest_sha256": core_hash, "language_manifest_sha256": language_hash,
        **signing_contract(selection),
    }
    sealed_bytes = (json.dumps(sealed_identity, ensure_ascii=False, indent=2) + "\n").encode()
    (destination / "Contents/Resources/luna-source-identity.json").write_bytes(sealed_bytes)
    signature_metadata = sign_application(destination, selection)
    executable_hash = hashlib.file_digest((destination / "Contents/MacOS/luna-reader").open("rb"), "sha256").hexdigest()
    package_identity = {
        "schema_version": 1, "kind": "luna_packaged_build_identity",
        "version": version, "platform": "macos", "architecture": "arm64",
        "bundle_id": "com.lunatranslator.reader",
        "source_tree_sha256": source_hash,
        "executable_entry": "Luna.app/Contents/MacOS/luna-reader",
        "executable_sha256": executable_hash,
        "core_manifest_sha256": core_hash,
        "language_manifest_sha256": language_hash,
        "sealed_source_identity_sha256": hashlib.sha256(sealed_bytes).hexdigest(),
        "build_started_at_ms": build_started_at_ms,
        "build_finished_at_ms": time.time_ns() // 1_000_000,
        "distribution_status": "development_candidate_not_release_approved",
        **signing_contract(selection), "signature_metadata": signature_metadata,
    }
    identity_bytes = (json.dumps(package_identity, ensure_ascii=False, indent=2) + "\n").encode()
    archive = output / f"Luna-Reader-{version}-macos-arm64.zip"
    if archive.exists():
        archive.unlink()
    run(["/usr/bin/ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", destination, archive])
    # A sibling entry avoids the circular hash that embedding an executable
    # digest in the resource-sealed .app would create when codesign rewrites it.
    with zipfile.ZipFile(archive, "a", compression=zipfile.ZIP_DEFLATED) as package:
        package.writestr("luna-build-identity.json", identity_bytes)
    if source_files(ROOT) != source_before:
        raise RuntimeError("Source changed while sealing or archiving; do not accept this package")
    result = {
        "version": version,
        "source_tree_sha256": source_hash,
        "executable_sha256": executable_hash,
        "distribution_status": "development_candidate_not_release_approved",
        "language_manifest_sha256": language_hash,
        "package_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
        "sealed_source_identity_sha256": hashlib.sha256(sealed_bytes).hexdigest(),
        "app": str(destination), "archive": str(archive),
        "archive_sha256": hashlib.file_digest(archive.open("rb"), "sha256").hexdigest(),
        "upstream_commit": manifest["commit"],
        "core_abi": manifest["abi_version"],
        "protocol_version": manifest["protocol_version"],
        "brokers": json.loads((resources / "core-staging-receipt.json").read_text())["brokers"],
        **signing_contract(selection), "signature_metadata": signature_metadata,
        "signature": signature_metadata["mode"],
        "bundle_resource_signature_verified": True,
    }
    (output / "build-manifest.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
