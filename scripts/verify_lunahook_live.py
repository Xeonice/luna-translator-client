#!/usr/bin/env python3
"""Capture a real Rust LunaHook host session and verify a small original-text sample.

This driver does not generate text events. It starts only the supplied host (and
optional CrossOver launcher), attaches the exact probed Windows process identity,
and preserves stdout, stderr, requests and parsed evidence in a new directory.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import ntpath
from pathlib import Path
import queue
import subprocess
import sys
import threading
import time


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def decimal_integer(value, field, maximum):
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        raise ValueError(f"{field} must be an integer or a decimal string, not a float")
    spelling = str(value)
    if not spelling.isascii() or not spelling.isdecimal():
        raise ValueError(f"{field} must contain decimal digits only")
    number = int(spelling)
    if not 0 < number <= maximum:
        raise ValueError(f"{field} is outside its nonzero unsigned integer range")
    return number


def select_target(probe, game_name):
    if isinstance(probe, dict) and "pid" in probe:
        candidates = [probe]
    elif isinstance(probe, dict):
        candidates = probe.get("targets", probe.get("processes"))
    else:
        candidates = None
    if not isinstance(candidates, list) or not all(isinstance(row, dict) for row in candidates):
        raise ValueError("target JSON must be a target object or contain a targets/processes list")
    if game_name:
        wanted = game_name.casefold()
        candidates = [row for row in candidates if wanted in {
            str(row.get("exe_name", "")).casefold(),
            str(row.get("name", "")).casefold(),
            ntpath.basename(str(row.get("full_image_path", ""))).casefold(),
            ntpath.splitext(ntpath.basename(str(row.get("full_image_path", ""))))[0].casefold(),
        }]
    if len(candidates) != 1:
        raise ValueError(f"target selection matched {len(candidates)} processes; use an exact --game-name")
    row = candidates[0]
    pid = decimal_integer(row.get("pid"), "pid", (1 << 32) - 1)
    creation = decimal_integer(row.get("creation_filetime"), "creation_filetime", (1 << 64) - 1)
    path = row.get("full_image_path")
    if not isinstance(path, str) or "\0" in path or not (
        path.startswith("\\\\") or (len(path) >= 3 and path[0].isascii()
        and path[0].isalpha() and path[1] == ":" and path[2] in "\\/")
    ):
        raise ValueError("full_image_path must be an absolute Windows path without NUL")
    return {"pid": pid, "full_image_path": path, "creation_filetime": str(creation)}


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def capture_pipe(pipe, destination, channel, messages):
    try:
        with destination.open("wb") as output:
            while True:
                line = pipe.readline()
                if not line:
                    break
                output.write(line)
                output.flush()
                if channel == "stdout":
                    messages.put(("line", line))
    except (OSError, ValueError) as error:
        messages.put(("reader_error", {"channel": channel, "message": str(error)}))
    finally:
        pipe.close()
        messages.put(("eof", channel))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--runtime-dir", type=Path, required=True)
    parser.add_argument("--wine", type=Path, help="CrossOver bin/wine launcher")
    parser.add_argument("--bottle", help="CrossOver bottle; required with --wine")
    parser.add_argument("--target-json", type=Path, required=True,
                        help="fresh Windows guest probe JSON with exact process identity")
    parser.add_argument("--game-name", help="exact executable name or stem when the probe has several targets")
    parser.add_argument("--output", type=Path, required=True, help="new evidence directory")
    parser.add_argument("--minimum-texts", type=int, default=5, help="minimum distinct nonempty original texts (default: 5)")
    parser.add_argument("--timeout", type=float, default=180, help="capture deadline in seconds (default: 180)")
    parser.add_argument("--hook-name", help="accept text only from this exact hook name (case insensitive)")
    args = parser.parse_args()
    if bool(args.wine) != bool(args.bottle):
        parser.error("--wine and --bottle must be supplied together")
    if args.minimum_texts < 1 or not 0 < args.timeout <= 86400:
        parser.error("--minimum-texts must be positive and --timeout must be in (0, 86400]")
    if args.hook_name is not None and not args.hook_name.strip():
        parser.error("--hook-name must not be blank")

    exe = args.exe.resolve(strict=True)
    runtime = args.runtime_dir.resolve(strict=True)
    if not exe.is_file() or not runtime.is_dir():
        raise ValueError("--exe must be a file and --runtime-dir must be a directory")
    probe_bytes = args.target_json.resolve(strict=True).read_bytes()
    target = select_target(json.loads(probe_bytes), args.game_name)
    command = [str(exe), "--runtime-dir", str(runtime)]
    if args.wine:
        command = [str(args.wine.resolve(strict=True)), "--bottle", args.bottle,
                   "--no-update", "--no-gui"] + command
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "target-probe.json").write_bytes(probe_bytes)
    started = time.monotonic()
    result = {
        "schema_version": 1, "passed": False, "scope": "minimum_live_original_text_sample",
        "long_duration_stability_verified": False, "started_utc": utc_now(),
        "target": target, "minimum_unique_texts": args.minimum_texts, "hook_name_filter": args.hook_name,
        "command": command, "executable_sha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
        "target_probe_sha256": hashlib.sha256(probe_bytes).hexdigest(), "launcher_pid": None,
        "ready": [], "connected": [], "disconnected": [], "injection_finished": [],
        "hook_inserted": [], "streams": [], "responses": {}, "errors": [], "overflow": [],
        "protocol_errors": [], "driver_errors": [], "integrity_lost": False,
        "text_entries": 0, "matching_text_entries": 0, "unique_text_count": 0,
        "unique_texts": [], "exit_code": None, "forced_launcher_cleanup": False,
        "shutdown_requested": False, "shutdown_acknowledged": False,
    }
    write_json(args.output / "result.json", result)
    messages = queue.Queue()
    process = None
    readers = []
    eof = set()
    stream_keys = set()
    unique = set()
    attach_sent = False
    shutdown_at = None
    known_events = {"ready", "response", "error", "process_connected", "process_disconnected",
                    "stream_created", "stream_destroyed", "text", "log", "hook_inserted",
                    "injection_finished", "overflow"}

    with (args.output / "requests.jsonl").open("w", encoding="utf-8") as requests, \
            (args.output / "events.jsonl").open("w", encoding="utf-8") as events, \
            (args.output / "texts.jsonl").open("w", encoding="utf-8") as texts:

        def send(method, params):
            request = {"v": 1, "id": f"live-{method}", "method": method, "params": params}
            wire = json.dumps(request, ensure_ascii=False) + "\n"
            requests.write(wire)
            requests.flush()
            process.stdin.write(wire.encode("utf-8"))
            process.stdin.flush()

        def shutdown(reason):
            nonlocal shutdown_at
            if shutdown_at is not None:
                return
            shutdown_at = time.monotonic()
            result["stop_reason"] = reason
            if process.poll() is None:
                try:
                    send("status", {})
                    send("shutdown", {})
                    result["shutdown_requested"] = True
                except (OSError, ValueError) as error:
                    result["driver_errors"].append(f"shutdown request: {error}")

        try:
            process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE)
            result["launcher_pid"] = process.pid
            for channel in ("stdout", "stderr"):
                destination = args.output / ("stdout.jsonl" if channel == "stdout" else "stderr.txt")
                reader = threading.Thread(target=capture_pipe,
                                          args=(getattr(process, channel), destination, channel, messages),
                                          daemon=True)
                reader.start()
                readers.append(reader)
            print(json.dumps({"state": "capturing", "target": target, "output": str(args.output)}, ensure_ascii=False), flush=True)
            while True:
                now = time.monotonic()
                if shutdown_at is None and now - started >= args.timeout:
                    shutdown("capture_timeout")
                if shutdown_at is not None and now - shutdown_at >= 15 and process.poll() is None:
                    result["forced_launcher_cleanup"] = True
                    result["driver_errors"].append("host launcher did not exit within 15 seconds after shutdown")
                    process.terminate()  # Only the Popen-owned launcher; never a process group or bottle.
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=3)
                if process.poll() is not None and len(eof) == 2 and messages.empty():
                    break
                if process.poll() is not None and shutdown_at is None:
                    shutdown_at = now
                    result["stop_reason"] = "host_exited"
                if process.poll() is not None and shutdown_at is not None and now - shutdown_at > 20:
                    result["driver_errors"].append("capture pipes remained open after launcher exit")
                    break
                try:
                    kind, payload = messages.get(timeout=0.2)
                except queue.Empty:
                    continue
                if kind == "eof":
                    eof.add(payload)
                    continue
                if kind == "reader_error":
                    result["driver_errors"].append(payload)
                    shutdown("capture_reader_error")
                    continue
                try:
                    event = json.loads(payload.decode("utf-8"))
                    if (not isinstance(event, dict) or event.get("v") != 1
                            or event.get("event") not in known_events):
                        raise ValueError("expected a version 1 host event")
                except (UnicodeError, ValueError) as error:
                    result["protocol_errors"].append(str(error))
                    shutdown("invalid_host_protocol")
                    continue
                events.write(json.dumps({"received_utc": utc_now(), "host_event": event}, ensure_ascii=False) + "\n")
                events.flush()
                name = event["event"]
                if name == "ready":
                    result["ready"].append(event)
                    if len(result["ready"]) != 1:
                        result["protocol_errors"].append("host emitted duplicate ready events")
                        shutdown("duplicate_ready")
                    elif shutdown_at is None:
                        send("attach", {"pid": target["pid"], "expected_image_path": target["full_image_path"],
                                        "expected_creation_time": target["creation_filetime"]})
                        attach_sent = True
                elif name == "response":
                    result["responses"][str(event.get("id"))] = event.get("result")
                    response = event.get("result", {})
                    if not isinstance(response, dict):
                        result["protocol_errors"].append("response result must be an object")
                    elif event.get("id") == "live-shutdown" and response.get("status") == "shutting_down":
                        result["shutdown_acknowledged"] = True
                    elif response.get("integrity_lost") is True:
                        result["integrity_lost"] = True
                elif name == "error":
                    result["errors"].append(event)
                    shutdown("host_error")
                elif name == "overflow":
                    result["overflow"].append(event)
                    result["integrity_lost"] = True
                    shutdown("capture_integrity_lost")
                elif name in {"process_connected", "process_disconnected", "injection_finished", "hook_inserted"}:
                    if event.get("pid") == target["pid"]:
                        key = {"process_connected": "connected", "process_disconnected": "disconnected"}.get(name, name)
                        result[key].append(event)
                        if name == "process_disconnected" and shutdown_at is None:
                            shutdown("target_disconnected")
                elif name in {"stream_created", "text"}:
                    stream = event.get("stream")
                    if not isinstance(stream, dict) or stream.get("pid") != target["pid"]:
                        continue
                    stream_key = json.dumps(stream, sort_keys=True)
                    if stream_key not in stream_keys:
                        stream_keys.add(stream_key)
                        result["streams"].append({"stream": stream, "hook_code": event.get("hook_code"),
                                                  "hook_name": event.get("hook_name")})
                    if name == "text":
                        result["text_entries"] += 1
                        text = event.get("text")
                        if not isinstance(text, str) or not isinstance(event.get("hook_name"), str):
                            result["protocol_errors"].append("text and hook_name must be strings")
                            shutdown("invalid_text_event")
                            continue
                        matches = (attach_sent and bool(text.strip()) and
                                   (args.hook_name is None or event["hook_name"].casefold() == args.hook_name.casefold()))
                        texts.write(json.dumps({"received_utc": utc_now(), "matches_acceptance_filter": matches,
                                                "host_event": event}, ensure_ascii=False) + "\n")
                        texts.flush()
                        if matches:
                            result["matching_text_entries"] += 1
                            if text.strip() not in unique:
                                unique.add(text.strip())
                                result["unique_texts"].append(text.strip())
                                result["unique_text_count"] = len(unique)
                                print(json.dumps({"state": "original_text", "unique_text_count": len(unique),
                                                  "hook_name": event["hook_name"], "text": text}, ensure_ascii=False), flush=True)
                            if len(unique) >= args.minimum_texts:
                                shutdown("minimum_text_sample_received")
                result["elapsed_seconds"] = round(time.monotonic() - started, 3)
                write_json(args.output / "result.json", result)
        except KeyboardInterrupt:
            result["driver_errors"].append("capture interrupted by operator")
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            result["driver_errors"].append(str(error))
        finally:
            if process is not None:
                if process.poll() is None:
                    shutdown("driver_exit")
                    try:
                        process.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        result["forced_launcher_cleanup"] = True
                        process.terminate()
                        try:
                            process.wait(timeout=3)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=3)
                if process.stdin:
                    try:
                        process.stdin.close()
                    except OSError as error:
                        result["driver_errors"].append(f"stdin close: {error}")
                for reader in readers:
                    reader.join(timeout=1)
                result["exit_code"] = process.poll()
            result["capture_complete"] = len(eof) == 2 and not any(reader.is_alive() for reader in readers)
            result["finished_utc"] = utc_now()
            result["elapsed_seconds"] = round(time.monotonic() - started, 3)
            attach_response = result["responses"].get("live-attach", {})
            result["passed"] = bool(
                len(result["ready"]) == 1 and result["connected"] and attach_sent
                and isinstance(attach_response, dict) and attach_response.get("status") == "pending"
                and result["unique_text_count"] >= args.minimum_texts
                and result["shutdown_acknowledged"] and result["exit_code"] == 0
                and result["capture_complete"]
                and not result["forced_launcher_cleanup"] and not result["integrity_lost"]
                and not any(result[key] for key in ("errors", "overflow", "protocol_errors", "driver_errors"))
            )
            write_json(args.output / "result.json", result)
    print(json.dumps({"passed": result["passed"], "scope": result["scope"],
                      "unique_text_count": result["unique_text_count"], "exit_code": result["exit_code"],
                      "result": str(args.output / "result.json")}, ensure_ascii=False), flush=True)
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Live LunaHook verification failed: {error}", file=sys.stderr)
        sys.exit(1)
