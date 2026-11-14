#!/usr/bin/env python3
"""Compile a read-only producer around the actual preview decision module.

The generated Cargo package is evidence plumbing, not another implementation of
the decisions.  Its only decision functions come from the production Rust file
via #[path].  A compiler/runtime failure is infrastructure failure, never a
semantic RED result and never permission to enter product implementation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/quality"))
from implementation_status import product_source_identity  # noqa: E402


def now():
    return datetime.now(timezone.utc).isoformat()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def ref(path):
    raw = path.read_bytes()
    return {"path": path.relative_to(ROOT).as_posix(), "sha256": sha(raw)}


def fresh_output(value):
    path = Path(value).absolute()
    if ".." in path.parts:
        raise ValueError("parent traversal in output paths is forbidden")
    if not path.is_relative_to(ROOT / ".build") or path.exists():
        raise ValueError("a fresh, nonexistent directory below repository .build is required")
    for parent in [path, *path.parents]:
        if parent == ROOT:
            break
        if parent.is_symlink():
            raise ValueError("symlink output paths are forbidden")
    path.mkdir(parents=True, exist_ok=False)
    return path


def production_lock_closure(root=ROOT):
    """Resolve Cargo.lock dependency identities without selecting cached versions."""
    raw = (root / "Cargo.lock").read_text()
    document = tomllib.loads(raw)
    packages = document["package"]
    by_name = {}
    for package in packages:
        by_name.setdefault(package["name"], []).append(package)

    def key(package):
        return package["name"], package["version"], package.get("source")

    def resolve(dependency):
        # Cargo writes an explicit version/source when a name is ambiguous.
        parts = dependency.split(" ", 2)
        candidates = by_name.get(parts[0], [])
        if len(parts) > 1:
            candidates = [value for value in candidates if value["version"] == parts[1]]
        if len(parts) > 2:
            candidates = [value for value in candidates if value.get("source") == parts[2].strip("()")]
        if len(candidates) != 1:
            raise ValueError("ambiguous production lock dependency: " + dependency)
        return candidates[0]

    queue, selected = [resolve("app-core")], {}
    # serde_json is the producer's direct serializer as well as app-core's.
    queue.append(resolve("serde_json"))
    while queue:
        package = queue.pop()
        identity = key(package)
        if identity in selected:
            continue
        selected[identity] = package
        queue.extend(resolve(dependency) for dependency in package.get("dependencies", []))
    chunks = re.split(r"(?=\[\[package\]\])", raw)
    subset = chunks[0]
    for chunk in chunks[1:]:
        parsed = tomllib.loads(chunk)["package"][0]
        if key(parsed) in selected:
            subset += chunk
    subset += ('\n[[package]]\nname = "binding-journey-projection-producer"\n'
               'version = "0.0.0"\ndependencies = ["app-core", "serde_json"]\n')
    return resolve("serde_json")["version"], selected, subset


def cargo_manifest_text(root=ROOT, serde_version=None):
    """Exact generated manifest bytes; dependency versions come from root lock."""
    if serde_version is None:
        serde_version = production_lock_closure(root)[0]
    core = root / "crates/app-core"
    return ('[package]\nname = "binding-journey-projection-producer"\nversion = "0.0.0"\nedition = "2024"\n'
                  '[workspace]\n\n[dependencies]\napp-core = { path = ' + json.dumps(str(core)) + ' }\n'
                  'serde_json = "=' + serde_version + '"\n')


def rust_source_text(root=ROOT):
    """Exact wrapper source; decisions remain imported production functions."""
    module = root / "apps/desktop/src-tauri/src/platform/window_preview/state.rs"
    return '''use app_core::contracts::window_capabilities::{PreviewPermission, PreviewProjection};
#[path = MODULE_PATH]
mod production_preview_decisions;

fn main() {
    let nonce = std::env::args().nth(1).expect("execution nonce");
    let decision = production_preview_decisions::collector_status_policy(
        "Unavailable", Some("preview_window_selection_required")
    ).expect("production unavailable decision");
    let projection = PreviewProjection {
        lease_id: Some("controlled-rust-preview-lease".into()), generation: "1".into(),
        consumer_id: "controlled-rust-candidate".into(), state: decision.state,
        permission: PreviewPermission::Granted, target_identity_hash: None,
        attempted_at_ms: Some("1794234889838".into()), captured_at_ms: None,
        content_changed_at_ms: None, permission_checked_at_ms: Some("1794234889838".into()),
        error_code: Some("preview_window_selection_required".into()), detail: decision.detail.into(),
    };
    let projection = production_preview_decisions::projection_snapshot(
        &projection, Some("阶段 win_pre · 请求 1 · Native操作 0")
    );
    let collector_cases: Vec<_> = [
        ("Selecting", None),
        ("Unavailable", Some("preview_window_selection_required")),
        ("Unavailable", Some("preview_selection_required")),
        ("Unavailable", Some("preview_permission_denied")),
        ("Unavailable", Some("preview_target_closed")),
    ].into_iter().map(|(phase, code)| {
        let value = production_preview_decisions::collector_status_policy(phase, code)
            .expect("production decision");
        serde_json::json!({"phase":phase,"code":code,"state":value.state,
            "detail":value.detail,"clearImage":value.clear_image})
    }).collect();
    let capture_cases: Vec<_> = ["preview_window_not_on_screen", "preview_permission_revoked",
        "preview_target_closed", "preview_capture_busy"].into_iter().map(|code| {
        let value = production_preview_decisions::capture_failure_policy(code, true);
        serde_json::json!({"code":code,"state":value.state,"detail":value.detail,
            "permissionDenied":value.permission_denied,"invalidatePairing":value.invalidate_pairing})
    }).collect();
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "schema_version":1,"kind":"binding-journey-production-projection-values/v1",
        "nonce":nonce,"selection_required":projection,
        "collector_decisions":collector_cases,"capture_failure_decisions":capture_cases,
        "scope":"Actual production Rust pure decision functions and serde DTO; no Native or Hook execution"
    })).expect("serialize production DTO"));
}
'''.replace("MODULE_PATH", json.dumps(str(module)))


def load_runtime_transport(path, nonce, root=ROOT):
    """Read a registered real-producer envelope; never fabricate its DTO fields.

    The independent inspector additionally verifies compilation, inventory,
    binary provenance and exact test execution. This importer only accepts
    intact repository evidence from the one registered production writer.
    """
    def checked_file(file):
        file = file.absolute()
        if ".." in file.parts:
            raise ValueError("parent traversal in runtime transport paths is forbidden")
        if not file.is_relative_to(root / ".build"):
            raise ValueError("runtime transport evidence must be below repository .build")
        for ancestor in [file, *file.parents]:
            if ancestor == root:
                break
            if ancestor.is_symlink():
                raise ValueError("symlink runtime transport evidence is forbidden")
        if not file.is_file():
            raise ValueError("runtime transport evidence is not a regular file")
        return file

    def checked_reference(value):
        file = checked_file(root / value["path"])
        if sha(file.read_bytes()) != value["sha256"]:
            raise ValueError("runtime transport evidence hash differs: " + value["path"])
        return file

    file = checked_file(path)
    manifest = json.loads(file.read_text())
    if manifest.get("kind") != "binding-journey-runtime-transport/v1" or manifest.get("nonce") != nonce:
        raise ValueError("runtime transport envelope kind or nonce differs")
    producer = manifest.get("producer", {})
    expected = {"package": "app-infra", "target": "binding_journey", "test_name": "bj_component_transport",
                "source": "crates/app-infra/tests/binding_journey.rs"}
    if any(producer.get(key) != value for key, value in expected.items()):
        raise ValueError("runtime transport producer is not registered")
    source = root / producer["source"]
    if source.is_symlink() or not source.is_file() or sha(source.read_bytes()) != producer.get("source_sha256"):
        raise ValueError("runtime transport registered producer source differs")
    if manifest.get("mode") == "ndjson":
        # The registered binary is compiled/listed by the gate adapter. The
        # actual persistent server execution belongs to the browser run, not
        # a fabricated once-only serializer receipt in this exporter.
        for key in ("compilation", "inventory", "binary"):
            checked_reference(manifest[key])
        if manifest.get("protocol") != "LUNA_BINDING_RPC_V1":
            raise ValueError("runtime transport NDJSON protocol is not registered")
        if not re.fullmatch(r"[a-f0-9]{64}", manifest.get("source_tree_sha256", "")):
            raise ValueError("runtime producer source tree identity is missing")
    else:
        exchange_file = checked_reference(manifest["file"])
        for key in ("execution", "compilation", "inventory", "binary"):
            checked_reference(manifest[key])
        exchanges = json.loads(exchange_file.read_text())
        if (exchanges.get("kind") != "binding-journey-runtime-exchanges/v1"
                or exchanges.get("schema_version") != 1 or exchanges.get("nonce") != nonce
                or exchanges.get("source_tree_sha256") != manifest.get("source_tree_sha256")):
            raise ValueError("runtime exchange producer identity or nonce differs")
        if not isinstance(exchanges.get("methods"), dict):
            raise ValueError("runtime exchanges must provide serialized method results")
    return manifest, ref(file)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--nonce", required=True)
    parser.add_argument("--runtime-transport", help="fresh actual app-infra producer envelope; omitted when that layer is blocked")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", args.nonce):
        parser.error("nonce must be 1-128 ASCII letters, digits, dot, underscore or hyphen")
    output = fresh_output(args.output)
    started = now()
    before = product_source_identity(ROOT)
    source_paths = ["Cargo.toml", "Cargo.lock", "crates/app-core/Cargo.toml",
                    "apps/desktop/src-tauri/src/platform/window_preview/state.rs"]
    source_paths.extend(path for path in before if path.startswith("crates/app-core/src/"))
    sources = {name: sha((ROOT / name).read_bytes()) for name in sorted(set(source_paths))}
    errors = []
    command_refs = []
    projection_ref = None
    binary_ref = None
    project = output / "producer"
    (project / "src").mkdir(parents=True)
    serde_version, locked_closure, lock_seed = production_lock_closure()
    cargo_text = cargo_manifest_text(ROOT, serde_version)
    (project / "Cargo.toml").write_text(cargo_text)
    # Start with exact production versions. Cargo may prune feature-unused
    # dependencies in this isolated package, but may not replace their versions.
    (project / "Cargo.lock").write_text(lock_seed)
    source = rust_source_text(ROOT)
    (project / "src/main.rs").write_text(source)
    target = ROOT / ".build/binding-journey-alignment-20261109/cache/producer-target"
    target.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "CARGO_TARGET_DIR": str(target), "CARGO_NET_OFFLINE": "true"}

    def execute(command, label):
        begin = now()
        try:
            result = subprocess.run(command, cwd=project, env=env, capture_output=True,
                                    text=True, timeout=240)
            code, stdout, stderr = result.returncode, result.stdout, result.stderr
        except (OSError, subprocess.TimeoutExpired) as error:
            code, stdout, stderr = -1, "", str(error)
        stdout_path, stderr_path = output / (label + ".stdout"), output / (label + ".stderr")
        stdout_path.write_text(stdout)
        stderr_path.write_text(stderr)
        artifact = output / (label + ".execution.json")
        write_json(artifact, {"command": command, "cwd": project.relative_to(ROOT).as_posix(),
                   "started_at": begin, "finished_at": now(), "exit_code": code,
                   "stdout": ref(stdout_path), "stderr": ref(stderr_path)})
        command_refs.append(ref(artifact))
        return code, stdout

    lock_comparison = None
    if sys.platform != "darwin":
        errors.append("production collector_status_policy is macOS-only; this platform cannot produce its fixture")
    else:
        code, _ = execute(["cargo", "metadata", "--offline", "--format-version", "1"], "lock")
        if code:
            errors.append("isolated producer lock resolution failed")
        else:
            mismatches = []
            generated = tomllib.loads((project / "Cargo.lock").read_text())["package"]
            for package in generated:
                if package["name"] == "binding-journey-projection-producer" and "source" not in package:
                    continue
                identity = (package["name"], package["version"], package.get("source"))
                expected = locked_closure.get(identity)
                if expected is None or expected.get("checksum") != package.get("checksum"):
                    mismatches.append({"name": package["name"], "version": package["version"],
                                       "source": package.get("source"), "checksum": package.get("checksum")})
            lock_file = output / "lock-comparison.json"
            write_json(lock_file, {"production_lock": ref(ROOT / "Cargo.lock"),
                       "isolated_lock": ref(project / "Cargo.lock"),
                       "serde_json_version": serde_version, "production_closure_count": len(locked_closure),
                       "resolved_package_count": len(generated), "mismatches": mismatches,
                       "policy": "Every resolved production dependency must have an exact name/version/source/checksum in the app-core production Cargo.lock closure; only generated producer is extra."})
            lock_comparison = ref(lock_file)
            if mismatches:
                errors.append("isolated producer dependencies differ from production lock closure")
            code, _ = (-1, "") if errors else execute(["cargo", "build", "--locked", "--offline", "--quiet"], "compile")
            if code:
                errors.append("production Rust producer compilation failed")
            else:
                built = target / "debug/binding-journey-projection-producer"
                binary = output / "binding-journey-projection-producer"
                shutil.copy2(built, binary)
                binary_ref = ref(binary)
                code, stdout = execute([str(binary), args.nonce], "run")
                if code:
                    errors.append("production Rust producer execution failed")
                else:
                    try:
                        values = json.loads(stdout)
                        if values["nonce"] != args.nonce or values["selection_required"]["errorCode"] != "preview_window_selection_required":
                            raise ValueError("producer nonce or selected input code differs")
                        projection = output / "production-projections.json"
                        write_json(projection, values)
                        projection_ref = ref(projection)
                    except (ValueError, KeyError, TypeError) as error:
                        errors.append("production Rust output is invalid: " + str(error))
    runtime_transport = None
    runtime_transport_ref = None
    if args.runtime_transport:
        try:
            runtime_transport, runtime_transport_ref = load_runtime_transport(Path(args.runtime_transport), args.nonce)
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append("runtime transport import failed: " + str(error))
    after = product_source_identity(ROOT)
    if before != after:
        errors.append("production source changed during exporter execution")
    report = {"schema_version": 1, "kind": "binding-journey-production-projections/v1",
              "nonce": args.nonce, "started_at": started, "finished_at": now(),
              "status": "failed" if errors else "passed", "sources": sources,
              "source_before": before, "source_after": after, "source_stable": before == after,
              "producer": ref(Path(__file__).resolve()), "generated_rust": ref(project / "src/main.rs"),
              "generated_cargo": ref(project / "Cargo.toml"), "binary": binary_ref,
              "isolated_lock": ref(project / "Cargo.lock"), "generated_lock": ref(project / "Cargo.lock"),
              "lock_comparison": lock_comparison,
              "projection_file": projection_ref,
              "command_execution": command_refs[-1] if command_refs else None,
              "executions": command_refs, "infrastructure_errors": errors,
              "runtime_transport": runtime_transport, "runtime_transport_manifest": runtime_transport_ref,
              "native_executed": False, "product_acceptance_promoted": False}
    write_json(output / "manifest.json", report)
    print(json.dumps({"status": report["status"], "manifest": str(output / "manifest.json"),
                      "infrastructure_errors": errors}, ensure_ascii=False))
    return 2 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
