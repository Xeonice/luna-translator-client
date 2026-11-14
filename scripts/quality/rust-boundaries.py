#!/usr/bin/env python3
"""Offline Cargo boundary gate for the approved first-product architecture.

Checks every declared normal/dev/build/target/optional dependency, including
workspace inheritance, aliases and transitive *local* path dependencies. It
does not resolve registry/git dependency trees, evaluate Rust source semantics,
or claim that process/FFI/feature isolation has been proved by compilation.

Layer identification is anchored to repository paths, not dependency aliases.
Unknown local crates fail closed until their architectural role is reviewed.
Dev/build/target dependencies receive the same rules; cfg is no exemption.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
from pathlib import Path
import sys
import tomllib


LAYERS = {
    "crates/app-core": "app-core",
    "crates/app-infra": "app-infra",
    "crates/hook-contracts": "hook-contracts",
    "crates/hook-client": "hook-client",
    "crates/lunahook-host": "lunahook-host",
    "crates/language-worker": "language-worker",
    "crates/offline-translation-worker": "offline-translation-worker",
    "apps/desktop/src-tauri": "desktop",
}

# These are direct crate relationships; broker/worker are subprocess binaries,
# not linked implementation libraries. app-core declares ports, infra implements
# them, desktop composes them. Language worker gets only a contract feature.
ALLOWED_LOCAL = {
    "app-core": set(),
    "app-infra": {"app-core", "hook-client"},
    "hook-contracts": set(),
    "hook-client": {"hook-contracts"},
    "lunahook-host": {"hook-contracts"},
    "language-worker": {"app-core"},
    "offline-translation-worker": set(),
    "desktop": {"app-core", "app-infra"},
}

CATEGORIES = {
    "ui": ("tauri", "tauri-*", "wry", "wry-*", "dioxus*", "iced", "slint", "egui", "eframe"),
    "http": (
        "reqwest", "ureq", "hyper", "hyper-*", "http", "http-*", "surf",
        "attohttpc", "isahc", "curl", "curl-sys", "awc", "actix-web",
        "axum", "warp", "rocket", "tonic", "tungstenite", "tokio-tungstenite",
        "openai", "openai-*", "async-openai", "anthropic*", "rig-core",
    ),
    "database": (
        "rusqlite", "sqlite", "sqlite-*", "sqlite3-sys", "libsqlite3-sys",
        "sqlx", "sqlx-*", "diesel", "diesel-*", "sea-orm", "sea-orm-*",
        "duckdb", "libduckdb-sys", "libsql", "libsql-*", "redb", "sled",
        "rocksdb", "rocksdb-*", "tokio-postgres", "postgres", "mysql*",
    ),
    "ffi-platform": (
        "libloading", "libc", "windows", "windows-*", "winapi", "winapi-*",
        "objc", "objc2", "objc2-*", "cocoa", "core-foundation", "core-foundation-*",
        "security-framework", "security-framework-*", "keyring", "keyring-*",
        "mecab", "mecab-*", "bindgen", "cbindgen", "cxx", "cxx-*", "cc",
        "libffi", "libffi-*", "jni", "jni-*",
    ),
    "lexical": ("mecab", "mecab-*", "lindera", "lindera-*", "vibrato", "sudachi*", "unidic*"),
}

FORBIDDEN = {
    "app-core": {"ui", "http", "database", "ffi-platform", "lexical"},
    "app-infra": {"ui"},
    "hook-contracts": set(CATEGORIES),
    "hook-client": {"ui", "http", "database", "lexical"},
    "lunahook-host": {"ui", "http", "database", "lexical"},
    "language-worker": {"ui", "http", "database"},
    "offline-translation-worker": {"ui", "http", "database", "lexical"},
    "desktop": {"http", "database", "lexical"},
}


def normalized(name: str) -> str:
    return name.lower().replace("_", "-")


def display_path(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def dependency_tables(manifest: dict):
    for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
        if kind in manifest:
            yield kind, manifest[kind], None
    for target, tables in manifest.get("target", {}).items():
        for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            if kind in tables:
                yield f"target.{target}.{kind}", tables[kind], target


def check(root: Path) -> dict:
    """Return structured findings without modifying files or invoking Cargo."""
    root = root.resolve()
    findings = []
    manifests: dict[Path, dict] = {}
    packages: dict[Path, dict] = {}
    edges: dict[Path, list[dict]] = {}
    parse_errors = False

    def finding(rule, path, message, evidence, **extra):
        item = {"rule": rule, "path": display_path(path, root), "message": message, "evidence": evidence}
        item.update(extra)
        findings.append(item)

    def read(path: Path):
        nonlocal parse_errors
        path = path.resolve()
        if path not in manifests:
            try:
                with path.open("rb") as stream:
                    manifests[path] = tomllib.load(stream)
            except (OSError, tomllib.TOMLDecodeError) as error:
                parse_errors = True
                finding("RUST-MANIFEST", path, "Cargo 清单无法读取或解析", str(error))
                manifests[path] = {}
        return manifests[path]

    workspace_path = root / "Cargo.toml"
    workspace_doc = read(workspace_path)
    workspace = workspace_doc.get("workspace", {})
    excluded = workspace.get("exclude", [])
    member_paths = set()
    if "package" in workspace_doc:
        member_paths.add(workspace_path)
    for pattern in workspace.get("members", []):
        matches = sorted(root.glob(pattern))
        if not matches:
            finding("RUST-MEMBER", workspace_path, "Workspace 成员不存在", str(pattern))
        for member in matches:
            relative = display_path(member.resolve(), root)
            if any(fnmatch.fnmatchcase(relative, exclude) for exclude in excluded):
                continue
            member_paths.add((member / "Cargo.toml").resolve())
    if not member_paths and not parse_errors:
        finding("RUST-MEMBER", workspace_path, "没有可检查的 Cargo package", "workspace.members and root package are empty")

    def inspect_package(path: Path):
        path = path.resolve()
        if path in packages:
            return
        doc = read(path)
        name = doc.get("package", {}).get("name")
        if not isinstance(name, str):
            if doc:
                finding("RUST-PACKAGE", path, "成员或本地路径依赖缺少 package.name", "package.name is absent or not a string")
            return
        relative_dir = display_path(path.parent, root)
        layer = LAYERS.get(relative_dir)
        packages[path] = {"name": name, "layer": layer, "manifest": path}
        edges[path] = []
        if layer is None:
            finding("RUST-LAYER-UNKNOWN", path, "本地 crate 尚未登记架构层，不能作为绕过边界的包装器", f"package={name}; directory={relative_dir}", package=name)
        elif layer != "desktop" and normalized(name) != layer:
            finding("RUST-LAYER-IDENTITY", path, "目录和架构包身份不一致", f"directory layer={layer}; package={name}", package=name)
        for table_name, table, target in dependency_tables(doc):
            if not isinstance(table, dict):
                finding("RUST-DEPENDENCY", path, "依赖表必须为 TOML table", table_name, package=name)
                continue
            for alias, value in table.items():
                if not isinstance(value, (str, dict)):
                    finding("RUST-DEPENDENCY", path, "不支持的 Cargo 依赖声明", f"{table_name}.{alias}={value!r}", package=name)
                    continue
                declaration = {} if isinstance(value, str) else value.copy()
                inherited = declaration.get("workspace") is True
                source_dir = path.parent
                if inherited:
                    base = workspace.get("dependencies", {}).get(alias)
                    if not isinstance(base, (str, dict)):
                        finding("RUST-WORKSPACE-DEPENDENCY", path, "继承的依赖不在 workspace.dependencies 中", f"{table_name}.{alias}.workspace=true", package=name)
                        continue
                    base = {} if isinstance(base, str) else base.copy()
                    if any(key in declaration for key in ("package", "path", "git", "version")):
                        finding("RUST-WORKSPACE-DEPENDENCY", path, "继承依赖不得改写来源或包身份", f"{table_name}.{alias}={declaration!r}", package=name)
                    # Cargo features are additive on inherited dependencies.
                    features = list(dict.fromkeys(base.get("features", []) + declaration.get("features", [])))
                    declaration = {**base, **declaration, "features": features}
                    if base.get("default-features", True):
                        declaration["default-features"] = True
                    source_dir = root
                dependency_name = declaration.get("package", alias)
                if not isinstance(dependency_name, str):
                    finding("RUST-DEPENDENCY", path, "依赖 package 身份必须为字符串", f"{table_name}.{alias}", package=name)
                    continue
                local_path = None
                if "path" in declaration:
                    local_path = (source_dir / declaration["path"] / "Cargo.toml").resolve()
                    local_doc = read(local_path)
                    actual = local_doc.get("package", {}).get("name")
                    if isinstance(actual, str):
                        if normalized(actual) != normalized(dependency_name):
                            finding("RUST-DEPENDENCY-IDENTITY", path, "路径依赖的 package 身份与目标清单不一致", f"{table_name}.{alias}: declared={dependency_name}; actual={actual}", package=name)
                        dependency_name = actual
                edge = {
                    "alias": alias, "package": normalized(dependency_name), "table": table_name,
                    "target": target, "local": local_path, "declaration": declaration,
                }
                edges[path].append(edge)
                if local_path is not None:
                    inspect_package(local_path)

    for member in sorted(member_paths):
        inspect_package(member)

    seen = set()
    for origin, package in packages.items():
        layer = package["layer"]
        if layer is None:
            continue

        def walk(current: Path, trail: list[dict], visited: set[Path]):
            for edge in edges.get(current, []):
                route = trail + [edge]
                categories = sorted(category for category, patterns in CATEGORIES.items()
                                    if any(fnmatch.fnmatchcase(edge["package"], pattern) for pattern in patterns))
                forbidden = sorted(set(categories) & FORBIDDEN[layer])
                route_names = [package["name"]] + [part["package"] for part in route]
                evidence = f"{display_path(current, root)} [{edge['table']}] {edge['alias']} (package={edge['package']}); route={' -> '.join(route_names)}"
                if forbidden:
                    key = (origin, current, edge["table"], edge["alias"], "forbidden")
                    if key not in seen:
                        seen.add(key)
                        finding("RUST-LAYER-DEPENDENCY", origin, f"{layer} 禁止依赖此基础设施类别：{', '.join(forbidden)}", evidence,
                                package=package["name"], dependency=edge["package"], categories=forbidden,
                                declaration_path=display_path(current, root), table=edge["table"], via=route_names)
                local = edge["local"]
                target_package = packages.get(local)
                # A registry/git declaration of a reserved first-party name
                # must not bypass dependency direction merely by omitting path.
                target_layer = target_package["layer"] if target_package else (
                    edge["package"] if edge["package"] in ALLOWED_LOCAL else None
                )
                known_target = target_package is not None or target_layer is not None
                if current == origin and known_target:
                    if target_layer not in ALLOWED_LOCAL[layer]:
                        finding("RUST-LAYER-DIRECTION", origin, f"{layer} 不得直接链接 {target_layer or '未登记层'}；通过规定端口或进程协议交互", evidence,
                                package=package["name"], dependency=edge["package"], table=edge["table"], via=route_names)
                    if layer == "language-worker" and target_layer == "app-core" and edge["declaration"].get("default-features", True):
                        finding("RUST-WORKER-CONTRACT-FEATURE", origin, "language-worker 使用 app-core 时必须关闭默认 feature，仅显式选择已审核的最小语言契约 feature", evidence,
                                package=package["name"], dependency=edge["package"], table=edge["table"])
                # Composition is allowed to link app-infra, whose purpose is
                # HTTP/SQL. Its own layer gate checks its subtree. Do not apply
                # the desktop's *direct adapter* restriction to that approved
                # implementation; unknown wrapper crates remain traversed.
                composition_boundary = layer == "desktop" and target_package and target_package["layer"] == "app-infra"
                if local in packages and local not in visited and not composition_boundary:
                    walk(local, route, visited | {local})

        walk(origin, [], {origin})

    findings.sort(key=lambda item: (item["path"], item["rule"], item["evidence"]))
    return {
        "schema_version": 1, "check": "rust-boundaries", "status": "failed" if findings else "passed",
        "coverage": {
            "manifest_scopes": ["normal", "dev", "build", "target", "optional", "workspace-inherited", "renamed"],
            "graph": "declared and transitive local path dependencies; desktop direct-adapter ban stops at approved app-infra composition; no registry/git resolution",
            "conditions": "All declared cfg/optional edges checked conservatively; feature semantics and Rust source/FFI require separate compile/review gates",
            "unknown_local_crates": "fail closed; review and register architectural role",
        },
        "checked_packages": [{"name": value["name"], "layer": value["layer"], "path": display_path(path, root)} for path, value in sorted(packages.items())],
        "findings": findings,
        "exit_code": 2 if parse_errors else (1 if findings else 0),
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--json", action="store_true", help="emit structured findings")
    args = parser.parse_args(argv)
    result = check(args.root)
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        for item in result["findings"]:
            print(f"{item['path']}: {item['rule']}: {item['message']}\n  {item['evidence']}")
        print(f"rust-boundaries: {result['status']}; {len(result['checked_packages'])} packages; {len(result['findings'])} findings")
    return result["exit_code"]


if __name__ == "__main__":
    sys.exit(main())
