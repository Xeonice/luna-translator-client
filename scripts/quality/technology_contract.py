#!/usr/bin/env python3
"""Check reviewed product technology manifests and design-source byte locks.

This freezes the current dependency snapshot, including explicitly recorded
legacy choices. It does not certify that those legacy choices implement the
target architecture. Changes to this contract still require the OpenSpec and
review gates; a local script is not protection against malicious rule edits.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import tomllib


LOCK_PATH = "openspec/technology-lock.json"
PRODUCT_ROOTS = ("apps", "crates", "native", "resources")
ROOT_MANIFESTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rust-toolchain")
MANIFEST_NAMES = frozenset({
    "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json",
    "npm-shrinkwrap.json", "pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb",
    "rust-toolchain.toml", "rust-toolchain", "tauri.conf.json", "angular.json",
})
CONFIG_NAME = re.compile(r"^(?:tsconfig(?:\.[\w-]+)*\.json|(?:vite|next|nuxt|svelte|astro)\.config\.(?:[cm]?[jt]s))$")
IGNORED_DIRECTORIES = frozenset({".git", "node_modules", "target", ".build", "dist", ".venv", "venv", "__pycache__", ".cache"})
# Known alternate frameworks/state architectures require an explicit entry in
# approved_stack, even if someone has refreshed only the file hashes.
FRAMEWORK_PACKAGES = frozenset({
    "vue", "vue-router", "@angular/core", "svelte", "solid-js", "preact", "electron",
    "redux", "@reduxjs/toolkit", "react-redux", "zustand", "react-router", "react-router-dom",
    "@tanstack/react-query", "jotai", "recoil", "mobx", "mobx-react", "mobx-react-lite",
})
FRAMEWORK_CRATES = frozenset({"iced", "eframe", "egui", "dioxus", "slint"})
VERSION = re.compile(r"^[\^~=]?\s*(\d+)(?:\.(\d+))?(?:\.(\d+))?$")
APPROVED_VERSION = re.compile(r"^\d+(?:\.\d+){0,2}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


def _json(path: Path):
    def reject_duplicates(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON field: {key}")
            result[key] = value
        return result
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates)


def _safe_path(root: Path, value: object, errors: list[str], label: str) -> Path | None:
    if not isinstance(value, str) or not value or "\\" in value:
        errors.append(f"{label}: expected nonempty repository-relative POSIX path")
        return None
    relative = PurePosixPath(value)
    if relative.is_absolute() or any(part in {".", ".."} for part in value.split("/")) or "//" in value:
        errors.append(f"{label}: unsafe repository-relative path")
        return None
    candidate = root / value
    if not candidate.resolve().is_relative_to(root.resolve()):
        errors.append(f"{label}: path escapes repository")
        return None
    if candidate.is_symlink():
        errors.append(f"{label}: source symlink is not supported")
        return None
    return candidate


def _inventory(root: Path, errors: list[str]) -> set[str]:
    inventory = {name for name in ROOT_MANIFESTS if (root / name).is_file()}
    # Root package.json belongs to the OpenSpec/quality toolchain, not the
    # product frontend. It may still be explicitly locked by manifests[].
    for name in PRODUCT_ROOTS:
        directory = root / name
        if not directory.exists():
            continue
        if directory.is_symlink():
            errors.append(f"{name}: product source directory may not be a symlink")
            continue
        for current, directories, filenames in os.walk(directory, followlinks=False):
            for child in list(directories):
                target = Path(current) / child
                if child in IGNORED_DIRECTORIES:
                    directories.remove(child)
                elif target.is_symlink():
                    errors.append(f"{target.relative_to(root).as_posix()}: product source directory may not be a symlink")
                    directories.remove(child)
            for filename in filenames:
                if filename in MANIFEST_NAMES or CONFIG_NAME.fullmatch(filename):
                    inventory.add((Path(current) / filename).relative_to(root).as_posix())
    return inventory


def _version_matches(actual: object, expected: str) -> bool:
    """Recognize simple stable single versions, ^, ~ and =; reject guesses."""
    if not isinstance(actual, str):
        return False
    match = VERSION.fullmatch(actual.strip())
    if not match:
        return False
    actual_parts = tuple(int(part) for part in match.groups() if part is not None)
    expected_parts = tuple(int(part) for part in expected.split("."))
    return actual_parts[:len(expected_parts)] == expected_parts and len(actual_parts) >= len(expected_parts)


def _approved_stack(document: dict, errors: list[str]) -> dict:
    approved = document.get("approved_stack")
    if not isinstance(approved, dict) or set(approved) != {"packages", "cargo", "rust"}:
        errors.append("approved_stack: expected packages, cargo and rust objects")
        return {}
    for ecosystem in ("packages", "cargo", "rust"):
        entries = approved[ecosystem]
        if not isinstance(entries, dict) or not entries:
            errors.append(f"approved_stack.{ecosystem}: expected nonempty version map")
            continue
        for name, version in entries.items():
            if not isinstance(name, str) or not name or not isinstance(version, str) or not APPROVED_VERSION.fullmatch(version):
                errors.append(f"approved_stack.{ecosystem}: invalid package/version declaration")
    if isinstance(approved.get("rust"), dict) and set(approved["rust"]) != {"edition", "rust-version"}:
        errors.append("approved_stack.rust: expected edition and rust-version")
    return approved if not errors else {}


def _lock_entries(root: Path, document: dict, key: str, errors: list[str]) -> set[str]:
    entries = document.get(key, [] if key == "design_sources" else None)
    if not isinstance(entries, list) or (key == "manifests" and not entries):
        errors.append(f"{key}: expected {'nonempty ' if key == 'manifests' else ''}array")
        return set()
    recorded = set()
    for index, entry in enumerate(entries):
        label = f"{key}[{index}]"
        if not isinstance(entry, dict):
            errors.append(f"{label}: expected path/sha256 object")
            continue
        value, digest = entry.get("path"), entry.get("sha256")
        candidate = _safe_path(root, value, errors, label)
        if candidate is None:
            continue
        if value in recorded:
            errors.append(f"{label}: duplicate path {value}")
        recorded.add(value)
        if not isinstance(digest, str) or not SHA256.fullmatch(digest):
            errors.append(f"{value}: expected lowercase SHA-256")
            continue
        if not candidate.is_file():
            errors.append(f"{value}: locked {key} file is missing")
            continue
        if hashlib.sha256(candidate.read_bytes()).hexdigest() != digest:
            errors.append(f"{value}: {key} SHA-256 mismatch; review the OpenSpec change before updating the lock")
    return recorded


def _dependencies(manifest: dict):
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        entries = manifest.get(section, {})
        if isinstance(entries, dict):
            yield from entries.items()
    workspace = manifest.get("workspace", {})
    if isinstance(workspace, dict) and isinstance(workspace.get("dependencies"), dict):
        yield from workspace["dependencies"].items()
    targets = manifest.get("target", {})
    if isinstance(targets, dict):
        for target in targets.values():
            if isinstance(target, dict):
                yield from _dependencies(target)


def _transitive_cargo_compatibility(document: dict, approved: dict, errors: list[str]) -> set[tuple[str, str, str, str]]:
    """Exact pre-existing resolutions may coexist; this never permits a direct dependency drift."""
    entries = document.get("transitive_cargo_compatibility", [])
    if not isinstance(entries, list):
        errors.append("transitive_cargo_compatibility: expected exact package identity array")
        return set()
    result = set()
    for item in entries:
        if not isinstance(item, dict) or set(item) != {"name", "version", "source", "checksum", "reason"}:
            errors.append("transitive_cargo_compatibility: invalid package identity")
            continue
        if (not all(isinstance(item[field], str) for field in item)
                or item["name"] not in approved.get("cargo", {})
                or not isinstance(item["version"], str) or not re.fullmatch(r"\d+\.\d+\.\d+", item["version"])
                or item["source"] != "registry+https://github.com/rust-lang/crates.io-index"
                or not isinstance(item["checksum"], str) or not SHA256.fullmatch(item["checksum"])
                or not isinstance(item["reason"], str) or not item["reason"].strip()):
            errors.append("transitive_cargo_compatibility: expected reviewed exact registry identity and reason")
            continue
        identity = (item["name"], item["version"], item["source"], item["checksum"])
        if identity in result:
            errors.append("transitive_cargo_compatibility: duplicate package identity")
        result.add(identity)
    return result


def _semantic_stack(root: Path, inventory: set[str], approved: dict, errors: list[str], compatibility: set[tuple[str, str, str, str]] = frozenset()):
    packages = approved["packages"]
    cargo = approved["cargo"]
    found_packages, found_cargo, used_compatibility = set(), set(), set()
    workspace = {}
    if (root / "Cargo.toml").is_file():
        try:
            workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8")).get("workspace", {})
        except (ValueError, OSError):
            pass  # Reported by the full manifest parse below.
    workspace_package = workspace.get("package", {}) if isinstance(workspace, dict) else {}
    workspace_dependencies = workspace.get("dependencies", {}) if isinstance(workspace, dict) else {}
    rust_manifests = []
    for relative in sorted(inventory):
        candidate = root / relative
        if relative.endswith("package.json"):
            try:
                manifest = _json(candidate)
                if not isinstance(manifest, dict):
                    raise ValueError("package root must be object")
            except (ValueError, OSError) as error:
                errors.append(f"{relative}: invalid package JSON ({type(error).__name__})")
                continue
            for section in ("dependencies", "devDependencies", "optionalDependencies", "peerDependencies"):
                dependencies = manifest.get(section, {})
                if not isinstance(dependencies, dict):
                    errors.append(f"{relative}: {section} must be object")
                    continue
                for name, version in dependencies.items():
                    if name in FRAMEWORK_PACKAGES and name not in packages:
                        errors.append(f"{relative}: framework/state package {name} has no approved_stack decision")
                    if name in packages:
                        found_packages.add(name)
                        if not _version_matches(version, packages[name]):
                            errors.append(f"{relative}: package {name} does not match approved version {packages[name]}")
        elif relative.endswith(("package-lock.json", "npm-shrinkwrap.json")):
            try:
                lock = _json(candidate)
                if not isinstance(lock, dict) or not isinstance(lock.get("packages"), dict):
                    raise ValueError("expected npm lock v2/v3 packages")
            except (ValueError, OSError) as error:
                errors.append(f"{relative}: unsupported/invalid npm package lock ({type(error).__name__})")
                continue
            for location, entry in lock["packages"].items():
                if "/node_modules/" not in f"/{location}" or not isinstance(entry, dict):
                    continue
                name = location.rsplit("node_modules/", 1)[-1]
                if name in packages and not entry.get("link") and not _version_matches(entry.get("version"), packages[name]):
                    errors.append(f"{relative}: resolved package {name} does not match approved version {packages[name]}")
        elif relative.endswith("Cargo.toml"):
            try:
                manifest = tomllib.loads(candidate.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                errors.append(f"{relative}: invalid Cargo TOML ({type(error).__name__})")
                continue
            if "workspace" in manifest and not isinstance(manifest["workspace"], dict):
                errors.append(f"{relative}: workspace must be a TOML table")
                continue
            if "package" in manifest and not isinstance(manifest["package"], dict):
                errors.append(f"{relative}: package must be a TOML table")
                continue
            rust_manifests.append((relative, manifest))
            for alias, declaration in _dependencies(manifest):
                specification = declaration
                if isinstance(declaration, dict) and declaration.get("workspace") is True:
                    specification = workspace_dependencies.get(alias)
                name = specification.get("package", alias) if isinstance(specification, dict) else alias
                if name in FRAMEWORK_CRATES and name not in cargo:
                    errors.append(f"{relative}: framework crate {name} has no approved_stack decision")
                if name in cargo:
                    found_cargo.add(name)
                    version = specification.get("version") if isinstance(specification, dict) else specification
                    if not _version_matches(version, cargo[name]):
                        errors.append(f"{relative}: crate {name} does not match approved version {cargo[name]}")
        elif relative.endswith("Cargo.lock"):
            try:
                lock = tomllib.loads(candidate.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                errors.append(f"{relative}: invalid Cargo lock ({type(error).__name__})")
                continue
            locked_packages = lock.get("package", [])
            if not isinstance(locked_packages, list):
                errors.append(f"{relative}: Cargo lock package must be array of tables")
                continue
            for entry in locked_packages:
                if not isinstance(entry, dict):
                    continue
                name = entry.get("name")
                if name in cargo and not _version_matches(entry.get("version"), cargo[name]):
                    identity = (name, entry.get("version"), entry.get("source"), entry.get("checksum"))
                    if all(isinstance(value, str) for value in identity) and identity in compatibility:
                        used_compatibility.add(identity)
                    else:
                        errors.append(f"{relative}: resolved crate {name} does not match approved version {cargo[name]}")
    if compatibility - used_compatibility:
        errors.append("transitive_cargo_compatibility: unused or nonmatching exact package identity")
    for name in sorted(set(packages) - found_packages):
        errors.append(f"approved_stack.packages: required product package {name} is missing")
    for name in sorted(set(cargo) - found_cargo):
        errors.append(f"approved_stack.cargo: required product crate {name} is missing")
    for relative, manifest in rust_manifests:
        for section in ("package", "workspace.package"):
            package = manifest.get("package") if section == "package" else manifest.get("workspace", {}).get("package")
            if not isinstance(package, dict):
                continue
            for name, version in approved["rust"].items():
                value = package.get(name)
                if isinstance(value, dict) and value.get("workspace") is True:
                    value = workspace_package.get(name)
                if not isinstance(value, str) or (value != version if name == "edition" else not _version_matches(value, version)):
                    errors.append(f"{relative}: {section}.{name} does not match approved {version}")


def check(root: Path | str) -> list[str]:
    root = Path(root).resolve()
    errors: list[str] = []
    try:
        document = _json(root / LOCK_PATH)
    except (ValueError, OSError) as error:
        return [f"{LOCK_PATH}: missing/invalid technology lock ({type(error).__name__})"]
    if not isinstance(document, dict):
        return [f"{LOCK_PATH}: expected object"]
    if type(document.get("schema_version")) is not int or document["schema_version"] != 1:
        errors.append(f"{LOCK_PATH}: schema_version must be integer 1")
    approved = _approved_stack(document, errors)
    recorded = _lock_entries(root, document, "manifests", errors)
    _lock_entries(root, document, "design_sources", errors)
    inventory = _inventory(root, errors)
    for relative in sorted(inventory - recorded):
        errors.append(f"{relative}: unregistered product manifest/config; add a reviewed OpenSpec technology decision")
    if not (root / "Cargo.toml").is_file() or not (root / "apps/desktop/package.json").is_file():
        errors.append("required product entry manifests Cargo.toml/apps/desktop/package.json are missing")
    if approved:
        compatibility = _transitive_cargo_compatibility(document, approved, errors)
        _semantic_stack(root, inventory, approved, errors, compatibility)
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    errors = check(args.root)
    if args.json:
        print(json.dumps({"schema_version": 1, "checker": "technology-contract", "status": "fail" if errors else "pass", "errors": errors}, ensure_ascii=False, indent=2))
    else:
        print(f"technology-contract: {'fail' if errors else 'pass'}")
        for error in errors:
            print(error)
    raise SystemExit(1 if errors else 0)


if __name__ == "__main__":
    main()
