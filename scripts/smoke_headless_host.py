#!/usr/bin/env python3
"""Check a real headless host's startup and commands without attaching a game."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--runtime-dir", type=Path, required=True)
    parser.add_argument("--wine", type=Path, help="CrossOver bin/wine launcher on macOS")
    parser.add_argument("--bottle", help="Explicit CrossOver bottle, required with --wine")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if bool(args.wine) != bool(args.bottle):
        parser.error("--wine and --bottle must be supplied together")
    exe, runtime = args.exe.resolve(strict=True), args.runtime_dir.resolve(strict=True)
    command = [str(exe), "--runtime-dir", str(runtime)]
    if args.wine:
        command = [str(args.wine.resolve(strict=True)), "--bottle", args.bottle,
                   "--no-update", "--no-gui"] + command
    args.output.mkdir(parents=True, exist_ok=False)
    requests = [
        {"v": 1, "id": "smoke-ping", "method": "ping", "params": {}},
        {"v": 1, "id": "smoke-status", "method": "status", "params": {}},
        {"v": 1, "id": "smoke-shutdown", "method": "shutdown", "params": {}},
    ]
    payload = "".join(json.dumps(request) + "\n" for request in requests)
    (args.output / "requests.jsonl").write_text(payload)
    started = time.monotonic()
    with (args.output / "stderr.txt").open("wb") as diagnostics:
        try:
            completed = subprocess.run(command, input=payload.encode(), stdout=subprocess.PIPE,
                                       stderr=diagnostics, timeout=30, check=False)
        except subprocess.TimeoutExpired as error:
            (args.output / "stdout.jsonl").write_bytes(error.stdout or b"")
            raise RuntimeError("host exceeded 30-second smoke deadline") from error
    (args.output / "stdout.jsonl").write_bytes(completed.stdout)
    events = [json.loads(line) for line in completed.stdout.decode("utf-8").splitlines()]
    if completed.returncode != 0 or any(event.get("v") != 1 for event in events):
        raise RuntimeError(f"host exit/protocol failure: exit={completed.returncode}")
    ready = [event for event in events if event.get("event") == "ready"]
    errors = [event for event in events if event.get("event") == "error"]
    responses = {event.get("id"): event.get("result") for event in events
                 if event.get("event") == "response"}
    if len(ready) != 1 or errors:
        raise RuntimeError(f"expected one ready and no errors, got ready={ready}, errors={errors}")
    expected = {"smoke-ping": "pong", "smoke-status": "snapshot",
                "smoke-shutdown": "shutting_down"}
    if any(responses.get(key, {}).get("status") != status for key, status in expected.items()):
        raise RuntimeError(f"missing command results: {responses}")
    if responses["smoke-status"].get("targets") != []:
        raise RuntimeError("fresh host unexpectedly owns game targets")
    if any(event.get("event") in {"text", "process_connected", "injection_finished"}
           for event in events):
        raise RuntimeError("no-attach smoke unexpectedly received target activity")
    result = {"passed": True, "scope": "initialization_and_commands_only", "attached_games": 0,
              "elapsed_seconds": round(time.monotonic() - started, 3),
              "executable_sha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
              "command": command, "ready": ready[0], "responses": responses,
              "exit_code": completed.returncode}
    (args.output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: value for key, value in result.items() if key != "command"}))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Headless host smoke failed: {error}", file=sys.stderr)
        sys.exit(1)
