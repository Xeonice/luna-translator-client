"""Bounded owned-process-tree execution for actual storage acceptance commands."""
from __future__ import annotations

import json
import os
from pathlib import Path
import plistlib
import re
import signal
import subprocess
import sys
import time


def _write(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def _group_members(group: int) -> list[dict]:
    scan = subprocess.run(["ps", "-axo", "pid=,pgid=,stat="], capture_output=True,
                          text=True, timeout=5)
    if scan.returncode:
        raise OSError("cannot verify owned process-group cleanup")
    result = []
    for line in scan.stdout.splitlines():
        values = line.split()
        if len(values) == 3 and values[1].isdigit() and int(values[1]) == group:
            result.append(dict(pid=int(values[0]), state=values[2], running=not values[2].startswith("Z")))
    return result


def _terminate_owned_group(process: subprocess.Popen) -> dict:
    if os.name != "posix":
        killed = subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                capture_output=True, text=True, timeout=10)
        process.wait(timeout=5)
        return dict(method="taskkill-owned-tree", exit_code=killed.returncode,
                    verified=killed.returncode == 0, stdout=killed.stdout, stderr=killed.stderr)
    group = process.pid  # start_new_session=True made this exact child its leader.
    delivered = []
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(group, sig)
            delivered.append(sig.name)
        except ProcessLookupError:
            pass
        if sig == signal.SIGTERM:
            # Leave a bounded grace period for descendants. Do not reap the
            # leader before sending KILL: its reserved PID prevents group reuse.
            deadline = time.monotonic() + 1
            while time.monotonic() < deadline:
                if not any(row["running"] for row in _group_members(group)):
                    break
                time.sleep(0.025)
    process.wait(timeout=5)
    deadline = time.monotonic() + 2
    remaining = _group_members(group)
    while any(row["running"] for row in remaining) and time.monotonic() < deadline:
        time.sleep(0.025)
        remaining = _group_members(group)
    # Descendant zombies can briefly await the system reaper after their parent
    # dies. Record them explicitly; they execute no work and hold no file locks.
    return dict(method="new-session-owned-process-group", group_id=group,
                delivered_signals=delivered, verified=not any(row["running"] for row in remaining),
                remaining_processes=remaining)


def _cleanup_owned_disk_images(output: Path) -> list[dict]:
    if sys.platform != "darwin":
        return []
    directory = output / "components/disk-full"
    candidates = list(directory.glob("*/limited-64m.dmg")) if directory.is_dir() else []
    if not candidates:
        return []
    info = subprocess.run(["hdiutil", "info", "-plist"], capture_output=True, timeout=10)
    if info.returncode:
        return [dict(status="failed", error="cannot enumerate this run's owned disk images")]
    images = plistlib.loads(info.stdout).get("images", [])
    result = []
    for image in candidates:
        if image.is_symlink() or not image.resolve().is_relative_to(output.resolve()):
            result.append(dict(status="failed", error="refused unowned disk image path"))
            continue
        attached = [row for row in images if Path(row.get("image-path", "")).resolve() == image.resolve()]
        for row in attached:
            devices = [entry.get("dev-entry", "") for entry in row.get("system-entities", [])]
            device = next((value for value in devices if re.fullmatch(r"/dev/disk[0-9]+", value)), None)
            if device is None:
                result.append(dict(status="failed", image=str(image), error="owned image has no exact root device"))
                continue
            detached = subprocess.run(["hdiutil", "detach", device], capture_output=True,
                                      text=True, timeout=30)
            result.append(dict(status="passed" if detached.returncode == 0 else "failed",
                               image=str(image), device=device, exit_code=detached.returncode,
                               stdout=detached.stdout, stderr=detached.stderr))
    return result


def execute(command: list[str], *, root: Path, output: Path, stage: str,
            env: dict, timeout: int) -> tuple[dict, str]:
    """Capture output immediately; timeout cannot discard partial diagnostics."""
    started = time.monotonic()
    stdout_path, stderr_path = output / (stage + ".stdout"), output / (stage + ".stderr")
    record = dict(stage=stage, command=command, status="running", timeout_seconds=timeout,
                  timed_out=False, exit_code=None, started_at_ms=time.time_ns() // 1_000_000)
    if env.get("FP_STORAGE_ADAPTER_VERSION") == "rust-storage-complete-v3":
        # Bind the real owned process record to this invocation, not merely to
        # a descriptor copied from an earlier storage run.
        for key, variable in (("run_id", "FP_STORAGE_RUN_ID"),
                              ("execution_nonce", "FP_STORAGE_EXECUTION_NONCE"),
                              ("source_tree_sha256", "FP_STORAGE_SOURCE_SHA256")):
            value = env.get(variable)
            if not isinstance(value, str) or not value:
                raise ValueError("schema3 process requires " + variable)
            record[key] = value
    state = output / (stage + ".process.json")
    _write(state, record)
    process = None
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            process = subprocess.Popen(command, cwd=root, env=env, stdout=stdout, stderr=stderr,
                                       start_new_session=os.name == "posix",
                                       creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
            record.update(pid=process.pid, process_group=process.pid if os.name == "posix" else None)
            _write(state, record)
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                record["timed_out"] = True
                record["termination"] = _terminate_owned_group(process)
                record["owned_disk_image_cleanup"] = _cleanup_owned_disk_images(output)
            record["exit_code"] = process.returncode
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        record["process_error"] = str(exc)
        if process is not None and process.returncode is None:
            try:
                record["termination"] = _terminate_owned_group(process)
            except (OSError, subprocess.SubprocessError) as cleanup:
                record["cleanup_error"] = str(cleanup)
    finally:
        record.update(status="failed" if record["timed_out"] or record["exit_code"] != 0 or record.get("process_error") else "passed",
                      finished_at_ms=time.time_ns() // 1_000_000, elapsed_seconds=round(time.monotonic() - started, 3),
                      stdout=str(stdout_path.relative_to(root)), stderr=str(stderr_path.relative_to(root)))
        _write(state, record)
    stdout = stdout_path.read_text(errors="replace") if stdout_path.exists() else ""
    stderr = stderr_path.read_text(errors="replace") if stderr_path.exists() else ""
    return record, stdout + "\n" + stderr
