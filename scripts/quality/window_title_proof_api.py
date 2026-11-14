#!/usr/bin/env python3
"""Exercise the current Rust Windows probe only against owned offscreen fixtures.

No real application HWND is accepted from callers. Receipts are evidence, not a
native game, product visual or release approval.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {"x86": ("i686-pc-windows-gnullvm", "i686-w64-mingw32-clang"),
           "x64": ("x86_64-pc-windows-gnullvm", "x86_64-w64-mingw32-clang")}
SOURCES = ("crates/lunahook-host/examples/target_probe.rs",
           "crates/hook-client/src/window_binding.rs", "scripts/quality/window_title_proof_api.py",
           "scripts/quality/fixtures/window_title_fixture.c", "Cargo.lock", "Cargo.toml",
           "crates/lunahook-host/Cargo.toml")
SCOPE = "actual_current_Rust_target_probe_against_owned_nonactivating_WinAPI_fixtures_only"
EXTRA = ("observer-eof-restores", "observer-thirdparty-preserved", "identity-pid-rejected",
         "identity-path-rejected", "identity-creation-rejected", "caption-truncated-prewrite-rejected",
         "caption-invalid-utf16-prewrite-rejected", "handler-rejected-explicit-restore",
         "stdout-failed-explicit-restore", "restore-invalid-utf16-not-acknowledged",
         "timeout-late-write-not-falsely-restored")


def digest(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def source_map() -> dict:
    return {path: digest(ROOT / path) for path in SOURCES}


def save(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


def artifact(path: Path, output: Path) -> dict:
    return {"path": str(path.relative_to(output)), "sha256": digest(path)}


class OwnedChildSetupError(RuntimeError):
    """Return ownership when setup fails after a child has actually spawned."""
    def __init__(self, child, error):
        self.child = child
        super().__init__("owned child reader setup failed: " + type(error).__name__ + ": " + str(error))


class Lines:
    """Retain exact child bytes; parsed lines are only an additional view."""
    def __init__(self, command, environment, broken_stdout=False):
        self.command = command
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, env=environment, cwd=ROOT)
        self.chunks = {"stdout": [], "stderr": []}
        self.rows = []
        self.messages = queue.Queue()
        self.threads = {}
        self.reader_states = {name: {"state": "pending", "error": None}
                              for name in ("stdout", "stderr")}
        self.setup_error = None
        try:
            if broken_stdout:
                self.process.stdout.close()
                self.reader_states["stdout"]["state"] = "intentionally_closed"
            for name in (("stderr",) if broken_stdout else ("stdout", "stderr")):
                thread = threading.Thread(target=self.read, args=(name,), daemon=True)
                thread.start()
                self.threads[name] = thread
        except (OSError, ValueError, RuntimeError) as error:
            self.setup_error = type(error).__name__ + ": " + str(error)
            for name in ("stdout", "stderr"):
                if name not in self.threads and self.reader_states[name]["state"] != "intentionally_closed":
                    self.reader_states[name] = {"state": "setup_error", "error": self.setup_error}
                    try:
                        getattr(self.process, name).close()
                    except (OSError, ValueError) as close_error:
                        self.reader_states[name]["error"] += "; close: " + str(close_error)
            raise OwnedChildSetupError(self, error) from error

    def read(self, name):
        stream = getattr(self.process, name)
        try:
            for line in iter(stream.readline, b""):
                self.chunks[name].append(line)
                if name == "stdout":
                    try:
                        row = json.loads(line)
                    except (ValueError, UnicodeDecodeError):
                        continue
                    if isinstance(row, dict):
                        self.rows.append(row)
                        self.messages.put(row)
            self.reader_states[name]["state"] = "eof"
        except (OSError, ValueError) as error:
            self.reader_states[name] = {"state": "read_error", "error": type(error).__name__ + ": " + str(error)}
        finally:
            try:
                stream.close()
            except (OSError, ValueError) as error:
                self.reader_states[name] = {"state": "read_error", "error": type(error).__name__ + ": " + str(error)}
            if name == "stdout":
                self.messages.put(None)

    def matching(self, predicate, seconds=10):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            try:
                row = self.messages.get(timeout=max(0.01, until - time.monotonic()))
            except queue.Empty:
                break
            if row is None:
                break
            if predicate(row):
                return row
        raise RuntimeError("child did not deliver required actual receipt")

    def send(self, command):
        self.process.stdin.write(command.encode("ascii") + b"\n")
        self.process.stdin.flush()

    def eof(self):
        if self.process.stdin and not self.process.stdin.closed:
            self.process.stdin.close()

    def completion(self):
        readers = {name: {**self.reader_states[name],
                          "alive": self.threads[name].is_alive() if name in self.threads else False}
                   for name in ("stdout", "stderr")}
        return {"process_exited": self.process.poll() is not None,
                "exit_code": self.process.poll(),
                "setup_error": self.setup_error,
                "stdin_closed": self.process.stdin is None or self.process.stdin.closed,
                "readers": readers,
                "reader_completion_confirmed": all(not row["alive"] and row["state"] in ("eof", "intentionally_closed")
                                                   for row in readers.values())}

    def finish(self, seconds=12, reader_seconds=2):
        code = self.process.wait(timeout=seconds)
        until = time.monotonic() + reader_seconds
        for thread in self.threads.values():
            thread.join(timeout=max(0, until - time.monotonic()))
        if not self.completion()["reader_completion_confirmed"]:
            raise RuntimeError("owned child exited but raw output reader completion is unconfirmed")
        return code

    def retain(self, directory):
        paths = []
        for name in ("stdout", "stderr"):
            path = directory / (name + ".bin")
            path.write_bytes(b"".join(self.chunks[name]))
            paths.append(path)
        path = directory / "process.json"
        save(path, {"command": self.command, "exit_code": self.process.poll(),
                    "parsed_stdout": self.rows, "stdout_bytes": sum(map(len, self.chunks["stdout"])),
                    "stderr_bytes": sum(map(len, self.chunks["stderr"])), "completion": self.completion()})
        return paths + [path]


def close_owned_child(child, *, fixture=False):
    """Attempt every normal close step even when an earlier pipe step fails."""
    errors = []
    if fixture and child.process.poll() is None:
        try:
            child.send("close")
        except (OSError, ValueError) as error:
            errors.append("close command: " + type(error).__name__ + ": " + str(error))
    try:
        child.eof()
    except (OSError, ValueError) as error:
        errors.append("stdin EOF: " + type(error).__name__ + ": " + str(error))
    try:
        child.finish(seconds=35 if fixture else 12)
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        errors.append("bounded exit/readers: " + type(error).__name__ + ": " + str(error))
    completion = child.completion()
    normal_fixture_exit = not fixture or (completion["exit_code"] == 0 and any(r.get("kind") == "normal_exit" for r in child.rows))
    return {**completion, "normal_fixture_exit": normal_fixture_exit,
            "confirmed": completion["process_exited"] and completion["reader_completion_confirmed"] and normal_fixture_exit,
            "errors": errors}


def compile_all(output):
    toolchain = ROOT / ".build/tools/llvm-mingw/bin"
    if not toolchain.is_dir():
        raise RuntimeError("existing llvm-mingw toolchain required; no installation attempted")
    environment = dict(os.environ)
    environment["PATH"] = str(toolchain) + os.pathsep + environment.get("PATH", "")
    environment["RUSTFLAGS"] = "-C target-feature=+crt-static"
    builds = []
    binaries = {}
    for arch, (target, compiler) in TARGETS.items():
        cc = toolchain / compiler
        if not cc.is_file():
            raise RuntimeError("missing existing cross compiler: " + str(cc))
        environment["CARGO_TARGET_" + target.upper().replace("-", "_") + "_LINKER"] = str(cc)
        for kind in ("probe", "fixture"):
            binary = output / "bin" / (kind + "-" + arch + ".exe")
            if kind == "probe":
                command = ["cargo", "build", "--locked", "--offline", "--release", "-p", "lunahook-host",
                           "--example", "target_probe", "--target", target]
                built = ROOT / "target" / target / "release/examples/target_probe.exe"
            else:
                command = [str(cc), "-O0", "-static", "-municode",
                           str(ROOT / SOURCES[3]), "-o", str(binary), "-luser32", "-lbcrypt"]
                built = binary
            began = time.monotonic()
            result = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, timeout=600)
            log = output / "builds" / (kind + "-" + arch + ".log")
            log.write_bytes((json.dumps({"command": command, "exit_code": result.returncode}) + "\nSTDOUT\n").encode()
                            + result.stdout + b"\nSTDERR\n" + result.stderr)
            if result.returncode:
                raise RuntimeError("actual " + kind + " build failed: " + str(log))
            if kind == "probe":
                shutil.copy2(built, binary)
            builds.append({"kind": kind, "probe_arch": arch, "target": target, "command": command,
                           "exit_code": result.returncode, "elapsed_seconds": round(time.monotonic()-began, 3),
                           "exe": artifact(binary, output), "log": artifact(log, output),
                           "compiler_path": str(cc), "compiler_sha256": digest(cc)})
            binaries[kind, arch] = binary
    return builds, binaries


def required_cases():
    for fixture in TARGETS:
        for probe in TARGETS:
            for name in ("ansi-prewrite-rejected", "unicode-exact-restore"):
                yield fixture, probe, name
    for probe in TARGETS:
        for name in EXTRA:
            yield "x86", probe, name


def one_scenario(output, directory, fixture_arch, probe_arch, name, binaries, launch, environment, execution_nonce, slow=None):
    directory.mkdir()
    count = 0
    def expect(value, message):
        nonlocal count
        count += 1
        if not value:
            raise AssertionError(message)
    modes = {"ansi-prewrite-rejected": "ansi", "caption-truncated-prewrite-rejected": "long",
             "caption-invalid-utf16-prewrite-rejected": "invalid", "handler-rejected-explicit-restore": "reject",
             "restore-invalid-utf16-not-acknowledged": "corrupt-restore",
             "timeout-late-write-not-falsely-restored": slow or "slow650"}
    mode = modes.get(name, "unicode")
    own_class = "LunaOwnedApiFixture-" + hashlib.sha256((execution_nonce+str(uuid.uuid4())).encode()).hexdigest()[:32]
    nonce = "LunaWindowProof-" + hashlib.sha256((execution_nonce+str(uuid.uuid4())).encode()).hexdigest()[:48]
    fixture_exe = binaries["fixture", fixture_arch]
    probe_exe = binaries["probe", probe_arch]
    expected_fixture_hash, expected_probe_hash = digest(fixture_exe), digest(probe_exe)
    fixture = probe = None
    baseline = final = ready = None
    errors = []
    retained = []
    exact_guard = False
    safety_preflight_failed = False
    cleanup = {}
    probe_code = None
    began = time.monotonic()
    try:
        fixture = Lines(launch + [str(fixture_exe), mode, own_class], environment)
        ready = fixture.matching(lambda r: r.get("kind") == "ready")
        baseline = fixture.matching(lambda r: r.get("kind") == "snapshot" and r.get("tag") == "baseline")
        expect(bytes.fromhex(ready["class_utf16le_hex"]).decode("utf-16le") == own_class, "fixture random class")
        expect(ready["bits"] == (32 if fixture_arch == "x86" else 64), "fixture real architecture")
        expect(ready["pid"] > 0 and int(ready["hwnd"]) > 0 and int(ready["creation"]) > 0, "exact owned process/window lifetime")
        expect(baseline["pid"] == ready["pid"] and baseline["hwnd"] == ready["hwnd"] and baseline["class_utf16le_hex"] == ready["class_utf16le_hex"], "fixture snapshot identity")
        safety_preflight_failed = baseline["is_visible"] != 0 or bool(baseline["is_foreground"]) or baseline["rect"][2] >= -10000
        expect(not safety_preflight_failed, "hidden nonactivating offscreen real WinAPI fixture")
        expect(digest(fixture_exe) == expected_fixture_hash and digest(probe_exe) == expected_probe_hash and ready["image_sha256"] == expected_fixture_hash, "exact loaded fixture and built probe binary bytes")
        path = bytes.fromhex(ready["path_utf16le_hex"]).decode("utf-16le")
        expect(path.lower().endswith("\\fixture-" + fixture_arch + ".exe"), "owned fixture image path")
        exact_guard = True
        pid, creation, argument_path = str(ready["pid"]), ready["creation"], path
        if name == "identity-pid-rejected": pid = str(ready["pid"] + 1)
        if name == "identity-path-rejected": argument_path = path + ".wrong"
        if name == "identity-creation-rejected": creation = str(int(creation) + 1)
        arguments = ["--window-title-challenge", ready["hwnd"], pid, argument_path, creation, nonce, fixture_arch, own_class]
        probe = Lines(launch + [str(probe_exe), *arguments], environment, broken_stdout=name == "stdout-failed-explicit-restore")
        interactive = name in ("unicode-exact-restore", "observer-eof-restores", "observer-thirdparty-preserved", "restore-invalid-utf16-not-acknowledged")
        if interactive:
            proof_ready = probe.matching(lambda r: r.get("status") == "ready")
            expect(proof_ready["status"] == "ready", "actual Rust proof ready")
            if name == "observer-thirdparty-preserved":
                fixture.send("thirdparty thirdparty-visible")
                third = fixture.matching(lambda r: r.get("tag") == "thirdparty-visible")
                expect(bytes.fromhex(third["caption_utf16le_hex"]).decode("utf-16le") == "ThirdParty-日本語-caption", "actual owner third party update")
            if name == "observer-eof-restores": probe.eof()
            else: probe.send("restore")
        else:
            probe.eof()
        code = probe.finish()
        probe_code = code
        # Owner snapshot is processed after any pending fixture callback. No sleep/retry conceals a late mutation.
        fixture.send("snapshot final")
        final = fixture.matching(lambda r: r.get("kind") == "snapshot" and r.get("tag") == "final")
        expect(final["pid"] == ready["pid"] and final["hwnd"] == ready["hwnd"] and final["class_utf16le_hex"] == ready["class_utf16le_hex"], "same final fixture identity")
        expect(not final["is_foreground"], "fixture never foreground at final observation")
        stdout_rows = probe.rows
        stderr = b"".join(probe.chunks["stderr"]).decode("utf-8", errors="replace")
        restored = [r for r in stdout_rows if r.get("status") == "restored"]
        same = baseline["caption_utf16le_hex"] == final["caption_utf16le_hex"]
        if name == "ansi-prewrite-rejected":
            expect(code == 0 and any(r.get("status") == "unavailable" and r.get("errorCode") == "window_title_proof_requires_unicode" for r in stdout_rows), "actual typed ANSI unavailability")
            expect(final["wm_settext_count"] == 0 and same, "ANSI rejected before any WM_SETTEXT; exact original unchanged")
        elif name in ("unicode-exact-restore", "observer-eof-restores"):
            expect(code == 0 and len(restored) == 1, "actual restored receipt and normal probe exit")
            expect(restored[0].get("titleRestored") is True and restored[0].get("identityVerified") is True, "restoration and identity acknowledged")
            expect(restored[0].get("requested") is (name != "observer-eof-restores"), "real observer EOF/request distinguished")
            expect(same and final["wm_settext_count"] == 2, "Unicode exact original restored after precisely challenge and restoration")
        elif name == "observer-thirdparty-preserved":
            expect(code != 0 and not restored and "title_restore_failed" in stderr, "third party title cannot be acknowledged as restored")
            expect(final["caption_utf16le_hex"] == third["caption_utf16le_hex"] and final["wm_settext_count"] == 1, "unrelated title preserved without second mutation")
        elif name.startswith("identity-"):
            expect(code != 0 and final["wm_settext_count"] == 0 and same, "wrong process identity rejected before mutation")
        elif name.startswith("caption-"):
            expected = "caption_truncated" if "truncated" in name else "caption_invalid_utf16"
            expect(code != 0 and expected in stderr and not restored, "actual strict caption preflight rejection")
            expect(final["wm_settext_count"] == 0 and same, "unsafe backup never written")
        elif name == "handler-rejected-explicit-restore":
            expect(code != 0 and "title_restore_failed" in stderr and "window_title_update_rejected" in stderr and "original_title_restored" not in stderr, "both rejected calls explicitly leave restoration unconfirmed")
            expect(final["wm_settext_count"] == 2 and same and not restored, "initial and single explicit restoration rejected; exact original unchanged with no success acknowledgement or Drop retry")
        elif name == "stdout-failed-explicit-restore":
            expect(code != 0 and "proof output failed" in stderr and "original_title_restored" in stderr, "ready pipe failure explicitly restored")
            expect(final["wm_settext_count"] == 2 and same, "broken output cannot leave nonce")
        elif name == "restore-invalid-utf16-not-acknowledged":
            expect(code != 0 and "title_restore_failed" in stderr and "invalid_utf16" in stderr and not restored, "lossy restoration cannot be acknowledged")
            expect(final["caption_utf16le_hex"] == "00d8" and final["wm_settext_count"] == 2, "actual invalid surrogate reaches strict readback once; no Drop retry")
        elif name == "timeout-late-write-not-falsely-restored":
            claimed = "original_title_restored" in stderr or any(r.get("titleRestored") is True for r in restored)
            expect(code != 0 and "window_title_update_failed" in stderr, "actual initial 400ms timeout observed")
            expect(not claimed or same, "late callback must not falsify original_title_restored")
            if not same:
                expect("title_restore_failed" in stderr or "unconfirmed" in stderr, "out of budget restoration explicitly unconfirmed")
            if slow == "slow650":
                expect(same and final["wm_settext_count"] == 2, "650ms pending write is followed by one acknowledged restoration")
            else:
                expect(not claimed and ("title_restore_failed" in stderr or "unconfirmed" in stderr), "2400ms exceeds two bounded calls; must not promise restoration")
                expect(final["wm_settext_count"] <= 2, "no unbounded or Drop restoration retry")
    except OwnedChildSetupError as error:
        # A successful Popen transfers ownership even if reader setup fails
        # before assignment. Retain that child for the same protected teardown.
        if fixture is None:
            fixture = error.child
        else:
            probe = error.child
        errors.append(type(error).__name__ + ": " + str(error))
    except (OSError, ValueError, RuntimeError, AssertionError, subprocess.TimeoutExpired) as error:
        errors.append(type(error).__name__ + ": " + str(error))
    finally:
        if probe:
            cleanup["probe"] = close_owned_child(probe)
            errors.extend("probe teardown: " + error for error in cleanup["probe"]["errors"])
        if fixture:
            cleanup["fixture"] = close_owned_child(fixture, fixture=True)
            errors.extend("fixture teardown: " + error for error in cleanup["fixture"]["errors"])
            try:
                expect(cleanup["fixture"]["normal_fixture_exit"], "fixture normal bounded exit")
            except AssertionError as error:
                errors.append("fixture teardown: " + str(error))
        for label, child in (("probe", probe), ("fixture", fixture)):
            if child:
                child_dir = directory / label;child_dir.mkdir()
                retained += child.retain(child_dir)
    cleanup_confirmed = all(item["confirmed"] for item in cleanup.values())
    if not cleanup_confirmed:
        errors.append("owned child cleanup is unconfirmed; no subsequent fixture may be created")
    observation = {"mode": mode, "assertion_count": count, "fixture_ready": ready, "expected_fixture_class": own_class,
                   "fixture_exe_sha256": expected_fixture_hash, "baseline": baseline, "final": final,
                   "cleanup": cleanup, "cleanup_confirmed": cleanup_confirmed,
                   "fixture_owner_events": fixture.rows if fixture else [],
                   "fixture_exit_code": fixture.process.poll() if fixture else None,
                   "fixture_stdout_artifact": artifact(directory/"fixture/stdout.bin", output) if fixture else None,
                   "fixture_process_artifact": artifact(directory/"fixture/process.json", output) if fixture else None,
                   "attempt": {"command": probe.command if probe else None,
                               "probe_exe_sha256": expected_probe_hash,
                               "exit_code": probe.process.poll() if probe else probe_code,
                               "parsed_stdout": probe.rows if probe else [],
                               "stderr": b"".join(probe.chunks["stderr"]).decode("utf-8", errors="replace") if probe else None,
                               "stdout_artifact": artifact(directory/"probe/stdout.bin", output) if probe else None,
                               "process_artifact": artifact(directory/"probe/process.json", output) if probe else None,
                               "stderr_artifact": artifact(directory/"probe/stderr.bin", output) if probe else None}}
    observation_path = directory/"observation.json";save(observation_path, observation)
    retained.append(observation_path)
    record = {"status": "passed" if not errors else "failed", "errors": errors, "assertion_count": count,
              "exact_identity_guard": exact_guard, "safety_preflight_failed": safety_preflight_failed,
              "cleanup_confirmed": cleanup_confirmed, "cleanup": cleanup, "fixture_arch": fixture_arch, "probe_arch": probe_arch,
              "probe_exe_sha256": expected_probe_hash, "fixture_exe_sha256": expected_fixture_hash,
              "mode": mode, "fixture_ready": ready, "baseline": baseline, "final": final, "observation": artifact(observation_path, output),
              "original_caption_sha256": hashlib.sha256(bytes.fromhex(baseline["caption_utf16le_hex"])).hexdigest() if baseline else None,
              "final_caption_sha256": hashlib.sha256(bytes.fromhex(final["caption_utf16le_hex"])).hexdigest() if final else None,
              "wm_settext_count": final["wm_settext_count"] if final else 0,
              "elapsed_seconds": round(time.monotonic()-began, 3), "artifacts": [artifact(p, output) for p in retained]}
    path = directory / "scenario.json";save(path, record);record["artifacts"].append(artifact(path, output))
    return record


def scenario_blocker(scenario):
    if scenario["safety_preflight_failed"]:
        return "fixture safety prerequisite failed"
    if not scenario["cleanup_confirmed"]:
        return "owned child cleanup is unconfirmed"
    return None


def run_scenarios(output, directory, fixture, probe, name, binaries, launch, environment, execution_nonce):
    modes = ["slow650", "slow2400"] if name == "timeout-late-write-not-falsely-restored" else [None]
    scenarios = []
    for mode in modes:
        scenario = one_scenario(output, directory/(mode or "actual"), fixture, probe, name,
                                binaries, launch, environment, execution_nonce, mode)
        scenarios.append(scenario)
        if scenario_blocker(scenario):
            break
    return scenarios


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--execution-nonce", required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / ".build") or output.exists():
        parser.error("output must be a new directory under this workspace .build")
    if not re.fullmatch(r"[A-Za-z0-9_-]{8,128}", args.execution_nonce):
        parser.error("invalid execution nonce")
    output.mkdir(parents=True)
    for part in ("builds", "bin", "cases"): (output / part).mkdir()
    before = source_map()
    receipt = {"schema_version": 1, "adapter_id": "product-window-title-proof-api-tests",
               "execution_nonce": args.execution_nonce, "scope": SCOPE, "native_game_verified": False,
               "status": "failed", "errors": [], "source_sha256": before, "builds": [], "cases": [],
               "dosdevices_path": str(Path.home()/"Library/Application Support/CrossOver/Bottles/Steam/dosdevices")}
    try:
        receipt["builds"], binaries = compile_all(output)
        if source_map() != before:
            raise RuntimeError("source changed during actual Windows builds; retain evidence and freeze before rerun")
        if sys.platform != "darwin":
            raise RuntimeError("this adapter requires the existing CrossOver macOS fixture runtime")
        wine = Path.home() / "Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"
        if not wine.is_file(): raise RuntimeError("existing CrossOver runtime missing; not installed by test")
        launch = [str(wine), "--bottle", "Steam", "--no-update", "--no-gui"]
        environment = dict(os.environ, WINEDEBUG="-all")
        for fixture, probe, name in required_cases():
            case_id = "WTPA-" + fixture + "-" + probe + "-" + name
            directory = output / "cases" / case_id;directory.mkdir()
            scenarios = run_scenarios(output, directory, fixture, probe, name, binaries, launch, environment, args.execution_nonce)
            first = scenarios[0]
            row = {key:first[key] for key in ("fixture_arch", "probe_arch", "probe_exe_sha256", "fixture_exe_sha256",
                    "original_caption_sha256", "final_caption_sha256")}
            row.update(id=case_id, status="passed" if all(s["status"] == "passed" for s in scenarios) else "failed",
                       errors=[e for s in scenarios for e in s["errors"]], assertion_count=sum(s["assertion_count"] for s in scenarios),
                       exact_identity_guard=all(s["exact_identity_guard"] for s in scenarios),
                       cleanup_confirmed=all(s["cleanup_confirmed"] for s in scenarios),
                       wm_settext_count=sum(s["wm_settext_count"] for s in scenarios), scenarios=scenarios,
                       artifacts=[a for s in scenarios for a in s["artifacts"]])
            observation_path = directory/"observation.json"
            save(observation_path, {"scenarios": [json.loads((output/s["observation"]["path"]).read_text()) for s in scenarios]})
            row["observation"] = artifact(observation_path, output)
            row["artifacts"].append(row["observation"])
            receipt["cases"].append(row)
            print(json.dumps({"id": case_id, "status": row["status"], "errors": row["errors"]}), flush=True)
            blocker = next((scenario_blocker(s) for s in scenarios if scenario_blocker(s)), None)
            if blocker:
                raise RuntimeError(blocker + "; remaining scenarios/cases not_run, no more fixture creation")
        receipt["errors"].extend(r["id"] + ": " + "; ".join(r["errors"]) for r in receipt["cases"] if r["status"] != "passed")
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        receipt["errors"].append(type(error).__name__ + ": " + str(error))
    receipt["source_sha256_after"] = source_map()
    receipt["source_before_after_equal"] = receipt["source_sha256_after"] == before
    if not receipt["source_before_after_equal"]: receipt["errors"].append("producer/fixture/probe/caller/dependency sources changed during execution")
    receipt["case_count"] = len(receipt["cases"])
    receipt["assertion_count"] = sum(r["assertion_count"] for r in receipt["cases"])
    receipt["status"] = "passed" if not receipt["errors"] and receipt["case_count"] == 30 else "failed"
    save(output / "receipt.json", receipt)
    print(json.dumps({"status": receipt["status"], "receipt": str(output / "receipt.json"), "case_count": receipt["case_count"],
                      "assertion_count": receipt["assertion_count"], "errors": receipt["errors"]}), flush=True)
    return 0 if receipt["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
