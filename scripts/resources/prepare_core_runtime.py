#!/usr/bin/env python3
"""Stage the pinned ABI 1 Core and matching Rust brokers; never use legacy Host bytes."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {"x86": ("i686-pc-windows-gnullvm", "i686-w64-mingw32-clang"),
           "x64": ("x86_64-pc-windows-gnullvm", "x86_64-w64-mingw32-clang")}
LICENSES = {"LICENSE": "LICENSE", "THIRD_PARTY_NOTICES.md": "THIRD_PARTY_NOTICES.md",
            "licenses/CMakeHelpers-LICENSE.txt": "CMakeHelpers-LICENSE.txt",
            "licenses/windows-classic-samples-LICENSE.txt": "windows-classic-samples-LICENSE.txt",
            "vendor/minhook/LICENSE.txt": "minhook-LICENSE.txt",
            "vendor/uchardet/COPYING": "uchardet-COPYING"}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def broker_source_hash() -> str:
    hasher = hashlib.sha256()
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "native/core-runtime.lock.json"]
    paths.extend(path for path in [ROOT / "rust-toolchain.toml", ROOT / ".cargo/config.toml"] if path.is_file())
    for name in ["hook-contracts", "hook-client", "lunahook-host"]:
        paths.extend(path for path in (ROOT / "crates" / name).rglob("*") if path.is_file())
    for path in sorted(paths):
        hasher.update(str(path.relative_to(ROOT)).encode())
        hasher.update(b"\0")
        hasher.update(path.read_bytes())
    return hasher.hexdigest()


def run(argv: list[str], **kwargs) -> None:
    subprocess.run(argv, cwd=ROOT, check=True, **kwargs)


def prepare(core_root: Path, resources: Path, build: bool = True) -> dict:
    manifest = json.loads((ROOT / "native/core-runtime.lock.json").read_text())
    commit = subprocess.check_output(["git", "-C", str(core_root), "rev-parse", "HEAD"], text=True).strip()
    if commit != manifest["commit"]:
        raise ValueError(f"Core checkout differs from pinned commit: {commit}")
    dirty = subprocess.check_output(["git", "-C", str(core_root), "status", "--porcelain", "--untracked-files=no"], text=True)
    if dirty.strip():
        raise ValueError("Core checkout has tracked modifications; refusing an unreviewed runtime")
    # Check every source before changing staging. The six known files alone are
    # allowed: an old Host/DLL set can never satisfy the new lock.
    for entry in manifest["files"]:
        source = core_root / "dist" / entry["architecture"] / "bin" / entry["runtime_path"]
        if source.is_symlink() or not source.is_file() or source.stat().st_size != entry["size"] or digest(source) != entry["sha256"]:
            raise ValueError(f"pinned Core verification failed: {source}")
    if {entry["source_path"] for entry in manifest["licenses"]} != set(LICENSES):
        raise ValueError("pinned license inventory differs from staging policy")
    for entry in manifest["licenses"]:
        path = core_root / entry["source_path"]
        if path.is_symlink() or not path.is_file() or path.stat().st_size != entry["size"] or digest(path) != entry["sha256"]:
            raise ValueError(f"pinned Core license verification failed: {path}")
    fingerprint = broker_source_hash()
    evidence_root = ROOT / ".build/core-brokers"
    evidence_root.mkdir(parents=True, exist_ok=True)
    if build:
        for arch, (target, compiler) in TARGETS.items():
            toolchain = ROOT / ".build/tools/llvm-mingw/bin"
            linker = toolchain / compiler
            if not linker.is_file():
                raise FileNotFoundError(f"matching cross linker is missing: {linker}")
            run(["rustup", "target", "add", target])
            env = os.environ.copy()
            env["PATH"] = str(toolchain) + os.pathsep + env["PATH"]
            env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(linker)
            env["RUSTFLAGS"] = "-C target-feature=+crt-static"
            run(["cargo", "build", "--locked", "--release", "-p", "lunahook-host", "--bin", "lunahook-host", "--example", "target_probe", "--example", "core_fixture_probe", "--target", target], env=env)
            outputs = {name: digest(ROOT / "target" / target / "release" / relative) for name, relative in
                       [("lunahook-host.exe", "lunahook-host.exe"), ("target_probe.exe", "examples/target_probe.exe")]}
            (evidence_root / f"{arch}.json").write_text(json.dumps({"source_sha256": fingerprint, "target":target,"files":outputs},indent=2)+"\n")
    if broker_source_hash() != fingerprint:
        raise ValueError("broker sources changed during compilation; rebuild a stable source tree")
    resources.mkdir(parents=True, exist_ok=True)
    for arch in TARGETS:
        generated_bin = resources / "core" / arch / "bin"
        if generated_bin.is_symlink():
            raise ValueError(f"Core staging directory cannot be a symbolic link: {generated_bin}")
        if generated_bin.exists(): shutil.rmtree(generated_bin)
    for entry in manifest["files"]:
        destination = resources / "core" / entry["architecture"] / "bin" / entry["runtime_path"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(core_root / "dist" / entry["architecture"] / "bin" / entry["runtime_path"], destination)
    brokers = []
    for arch, (target, _) in TARGETS.items():
        destination = resources / "brokers" / arch
        destination.mkdir(parents=True, exist_ok=True)
        for relative, filename in [("lunahook-host.exe", "lunahook-host.exe"), ("examples/target_probe.exe", "target_probe.exe")]:
            source = ROOT / "target" / target / "release" / relative
            if not source.is_file():
                raise FileNotFoundError(f"matching new broker has not been built: {source}")
            evidence = json.loads((evidence_root / f"{arch}.json").read_text())
            if evidence["source_sha256"] != fingerprint or evidence["target"] != target or evidence["files"].get(filename) != digest(source):
                raise ValueError("broker is stale or lacks a matching successful source build receipt; rebuild without --skip-windows-build")
            shutil.copy2(source, destination / filename)
            brokers.append({"architecture": arch, "file": str((destination / filename).relative_to(resources)), "sha256": digest(source)})
    # Remove only superseded generated Hook resources. Language data and other
    # collaborators' staged resources remain untouched.
    for obsolete in ["lunahook-host.exe", "target_probe.exe", "upstream.lock.json", "runtime"]:
        path = resources / obsolete
        if path.is_dir(): shutil.rmtree(path)
        elif path.exists(): path.unlink()
    shutil.copy2(ROOT / "native/core-runtime.lock.json", resources / "core-runtime.lock.json")
    licenses = resources / "licenses/luna-hooks-core"
    licenses.mkdir(parents=True, exist_ok=True)
    for relative, filename in LICENSES.items():
        shutil.copy2(core_root / relative, licenses / filename)
    receipt = {"core_commit": commit, "core_abi": manifest["abi_version"], "protocol_version": manifest["protocol_version"], "files": manifest["files"], "brokers": brokers}
    (resources / "core-staging-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core-root", type=Path, default=ROOT.parent / "luna-hooks-core")
    parser.add_argument("--resources", type=Path, default=ROOT / "apps/desktop/src-tauri/resources")
    parser.add_argument("--skip-windows-build", action="store_true")
    args = parser.parse_args()
    print(json.dumps(prepare(args.core_root.resolve(), args.resources.resolve(), not args.skip_windows_build), indent=2))


if __name__ == "__main__": main()
