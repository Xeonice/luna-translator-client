"""Inspect/replay retained render inputs. This module cannot approve product UI."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import struct
import subprocess
import sys


FLAGS = ("native_verified", "product_alignment_approved", "reference_approved", "completion_allowed")
HEX = re.compile(r"^[0-9a-f]{64}$")


def load(path):
    def invalid(value):
        raise ValueError("nonfinite JSON constant: " + value)
    return json.loads(Path(path).read_text(), parse_constant=invalid)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, allow_nan=False, separators=(",", ":"))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(value, *, positive=False):
    return type(value) in (int, float) and math.isfinite(value) and (not positive or value > 0)


def output_path(root, output, *, fresh=True):
    """Validate lexical output before resolving could erase a symlink ancestor."""
    root = Path(root).resolve()
    raw = os.fspath(output)
    require(isinstance(raw, str) and raw and "\\" not in raw and "\0" not in raw
            and ".." not in raw.split("/"), "unsafe lexical output path")
    lexical = Path(raw)
    if not lexical.is_absolute():
        lexical = Path.cwd() / lexical
    base = root / ".build"
    require(lexical != base and lexical.is_relative_to(base), "fresh ignored output required inside root/.build")
    current = root
    for part in lexical.relative_to(root).parts:
        current = current / part
        try:
            info = current.lstat()
        except FileNotFoundError:
            continue
        require(not stat.S_ISLNK(info.st_mode), "symlink output ancestor forbidden: " + str(current))
        if current == lexical:
            require(not fresh, "fresh ignored output required; existing evidence is immutable")
            require(stat.S_ISDIR(info.st_mode), "owned output must remain a directory")
        else:
            require(stat.S_ISDIR(info.st_mode), "output ancestor must be a directory")
    return lexical


def pinned(root, descriptor):
    require(type(descriptor) is dict and type(descriptor.get("path")) is str, "file descriptor missing")
    relative = Path(descriptor["path"])
    require(not relative.is_absolute() and all(p not in (".", "..") for p in relative.parts)
            and str(relative) == descriptor["path"] and "\\" not in str(relative), "unsafe file descriptor")
    require(type(descriptor.get("bytes")) is int and descriptor["bytes"] > 0
            and HEX.fullmatch(descriptor.get("sha256", "")), "typed hash/size required")
    path = root
    for part in relative.parts:
        path = path / part
        require(not path.is_symlink(), "symlink evidence forbidden")
    require(path.is_file() and path.stat().st_size == descriptor["bytes"]
            and digest(path) == descriptor["sha256"], "retained file hash/size differs: " + str(relative))
    return path


def source_identity(root):
    sys.path.insert(0, str(root / "scripts/quality"))
    from gate_core import source_files
    files = source_files(root)
    return {"sha256": hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest(), "file_count": len(files)}


def unavailable(reason):
    return {"schema_version": 1, "kind": "offline-render-comparison/v1", "status": "missing_render_evidence",
            "errors": [], "gaps": [reason], "pairs": [], "scope": "offline_dom_remeasurement_only", "independently_recomputed": False,
            **{key: False for key in FLAGS}}


def _style(style):
    for key in ("font_size_px", "line_height_px"):
        require(number(style[key], positive=True), "finite positive effective CSS px required: " + key)
    require(type(style["font_weight_css"]) is str and type(style["font_family_css"]) is str,
            "actual CSS font strings required")
    require(number(style["opacity"]) and 0 <= style["opacity"] <= 1, "typed actual opacity required")


def _box(box):
    require(set(box) == {"x", "y", "width", "height"} and all(number(v) for v in box.values())
            and box["width"] >= 0 and box["height"] >= 0, "finite actual CSS rectangle required")


def inspect(root, evidence, collector):
    """Validate provenance and schema; does not trust measurement status as a pass."""
    root, evidence, collector = Path(root).resolve(), Path(evidence).resolve(), Path(collector).resolve()
    errors = []
    try:
        report = load(evidence / "measurement.json")
        require(type(report["schema_version"]) is int and report["schema_version"] == 1
                and report["kind"] == "offline-render-measurement/v1", "measurement schema differs")
        require(all(type(report[k]) is bool and report[k] is False for k in FLAGS), "measurement promoted approval/completion")
        require(report["javascript_enabled"] is False and report["network_policy"] == "offline_allowlisted_bytes_only"
                and report["measurement_source"] == "independent_browser_on_retained_dom"
                and report["scope"] == "offline_dom_remeasurement_only", "offline independent measurement policy differs")
        require(report["errors"] == [] and report["status"] in ("measured", "missing_render_evidence"), "collector failed")
        request_path = pinned(root, report["request"])
        request = load(request_path)
        require(type(request["schema_version"]) is int and request["schema_version"] == 1
                and request["kind"] == "offline-render-request/v1", "request schema differs")
        require(re.fullmatch(r"[0-9a-f]{32}", request["nonce"]) and report["nonce"] == request["nonce"], "execution nonce differs")
        require(report["collector"] == {"path": str(collector.relative_to(root)), "sha256": digest(collector)}, "collector bytes differ")
        require(report["command"][1:] == [str(collector), "--root", str(root), "--request", str(request_path), "--output", str(evidence)],
                "actual collector argv differs")
        require(Path(report["command"][0]).name == "node", "actual node executable required")
        start, end = (datetime.fromisoformat(report[k].replace("Z", "+00:00")) for k in ("started_at", "finished_at"))
        require(0 <= (end - start).total_seconds() <= 300, "bounded actual measurement interval required")
        require(report["source_before"] == report["source_after"] == source_identity(root)
                and report["source_before"]["sha256"] == request["source_tree_sha256"], "managed source drift")
        exe = report["environment"]["browser_executable"]
        require(Path(exe["path"]).is_file() and digest(exe["path"]) == exe["sha256"], "browser executable bytes differ")
        for key in ("node", "platform", "architecture", "os_release", "playwright", "browser_version", "locale", "timezone"):
            require(type(report["environment"][key]) is str and report["environment"][key], "actual environment missing: " + key)
        subjects = request["subjects"]
        require(type(subjects) is list and 0 < len(subjects) <= 32
                and len({s["id"] for s in subjects}) == len(subjects), "bounded unique input subjects required")
        require([s["id"] for s in report["subjects"]] == [s["id"] for s in subjects], "exact original subject sequence differs")
        rows = {s["id"]: s for s in report["subjects"]}
        require(type(request["pairs"]) is list and all(type(p) is list and len(p) == 2 and all(v in rows for v in p) for p in request["pairs"]), "exact subject pairs required")
        for subject in subjects:
            pinned(root, subject["document"]["file"])
            # A missing/changed asset is evidence absence, never a comparable pair.
            row = rows[subject["id"]]
            require(row["status"] in ("measured", "missing_render_evidence") and row["granted_product_scope"] is None,
                    "subject status/scope differs")
            require(type(row["gaps"]) is list and bool(row["gaps"]) == (row["status"] == "missing_render_evidence"), "subject status does not equal actual gaps")
            if row["status"] != "measured":
                continue
            require(row["declared_state"] == subject.get("declared_state"), "retained input declarations differ")
            for asset in subject["assets"]:
                pinned(root, asset["file"])
            require(row["requested_scope"] == subject["required_scope"] == "target"
                    and row["measured_scope"] == "retained_dom_target", "whole-client/native scope cannot be granted")
            require(subject["required_properties"] == {"form_properties": False, "focus": False}, "unreconstructed original properties")
            require(row["blocked_requests"] == [] and row["original_properties"] == {"form": "not_reconstructed", "focus": "not_reconstructed", "animation_clock": "not_reconstructed"}, "unsupported render reconstruction")
            facts = row["facts"]
            require(facts["capture"] == {"kind": "browser_viewport", "full_page": False, "native_attestation": False}, "actual screenshot scope differs")
            vp = subject["viewport"]
            require(type(vp["width"]) is int and type(vp["height"]) is int
                    and type(vp["device_scale_factor"]) in (int, float) and vp["device_scale_factor"] in (1, 2), "typed CSS px viewport required")
            require(facts["browser_viewport"] == {"width": vp["width"], "height": vp["height"]}
                    and type(facts["device_pixel_ratio"]) in (int, float) and facts["device_pixel_ratio"] == vp["device_scale_factor"], "observed CSS viewport/DPR differs")
            require(all(facts[k] is None for k in ("desktop_viewport", "native_window_rect", "native_client_rect")), "DOM replay falsely attests native geometry")
            require(facts["running_animations"] == 0 and facts["fonts"]["status"] == "loaded", "unreconstructed active animation/font")
            require([m["query"] for m in facts["media"]] == subject["media_queries"]
                    and all(type(m["matches"]) is bool for m in facts["media"]), "actual media observations differ")
            for key in ("window", "layout", "source", "translation"):
                region = facts["regions"][key]
                require(region["selector"] == subject["regions"][key] and type(region["matches"]) is int
                        and region["matches"] == 1 and region["visible"] is True, "actual unique visible region required")
                _box(region["rect"])
                if key in ("source", "translation"):
                    _style(region["style"])
                    for run in region["text_runs"]:
                        _box(run["rect"]); _style(run["style"])
            screenshot = row["screenshot"]
            require(Path(screenshot["path"]).name == screenshot["path"], "safe original screenshot name required")
            png = evidence / screenshot["path"]
            require(digest(png) == screenshot["sha256"] and png.stat().st_size == screenshot["bytes"], "actual replay PNG differs")
            raw = png.read_bytes()
            require(raw[:8] == b"\x89PNG\r\n\x1a\n" and struct.unpack(">II", raw[16:24]) == (vp["width"] * vp["device_scale_factor"], vp["height"] * vp["device_scale_factor"]), "replay PNG dimensions differ from CSS viewport/DPR")
        expected = "measured" if all(s["status"] == "measured" for s in rows.values()) else "missing_render_evidence"
        require(report["status"] == expected, "aggregate measurement status differs")
        return report, request, errors
    except (ValueError, OSError, KeyError, TypeError, AttributeError, IndexError, struct.error) as error:
        errors.append(str(error))
        return None, None, errors


def state_facts(row):
    f = row["facts"]
    # No original producer styleJSON or declared prefs enters the comparison.
    result = {k: f[k] for k in ("browser_viewport", "document_client", "visual_viewport", "device_pixel_ratio", "css_zoom", "media", "platform_fonts", "capture")}
    # Source and product may have different selectors. Their identities are
    # validated against each request; qualification compares observed state.
    result["regions"] = {name: {k: v for k, v in region.items() if k not in ("selector", "matches", "visibility_method")}
                         for name, region in f["regions"].items()}
    return result


def declared_gaps(row):
    state = row.get("declared_state") or {}
    source, translation = (row["facts"]["regions"][k] for k in ("source", "translation"))
    actual = {"showRuby": any(r["visible"] for r in source["rt"]), "showPos": any(p["visual_effect"] for p in source["pos"]),
              "sourceFontPx": source["style"]["font_size_px"], "translationFontPx": translation["style"]["font_size_px"]}
    return ["declared_state_contradicts_actual:" + k for k, v in actual.items()
            if k in state and (type(state[k]) is not type(v) and not (number(state[k]) and number(v)) or state[k] != v)]


def compare(root, evidence, collector):
    report, request, errors = inspect(root, evidence, collector)
    result = unavailable("offline evidence absent or invalid")
    result["errors"] = errors
    if errors:
        return result
    result.update({"measurement_sha256": digest(Path(evidence) / "measurement.json"), "request_sha256": report["request"]["sha256"],
                   "source_tree_sha256": request["source_tree_sha256"], "nonce": request["nonce"], "gaps": [], "pairs": []})
    rows = {s["id"]: s for s in report["subjects"]}
    for left_id, right_id in request["pairs"]:
        left, right = rows[left_id], rows[right_id]
        missing = left["status"] != "measured" or right["status"] != "measured"
        reasons = list(left["gaps"] + right["gaps"]) if missing else declared_gaps(left) + declared_gaps(right)
        if not missing and state_facts(left) != state_facts(right):
            reasons.append("independently_measured_render_state_differs")
        result["pairs"].append({"subjects": [left_id, right_id], "status": "missing_render_evidence" if missing else "incomparable" if reasons else "comparable",
                                 "actual_render_state_equal": not missing and not reasons, "reasons": sorted(set(reasons)),
                                 "scope": "retained_dom_target_only", "whole_client_eligible": False})
    statuses = [p["status"] for p in result["pairs"]]
    result["status"] = "missing_render_evidence" if not statuses or "missing_render_evidence" in statuses else "incomparable" if "incomparable" in statuses else "comparable"
    return result


def collect(root, request, output, collector):
    root = Path(root).resolve()
    output = output_path(root, output)
    request, collector = Path(request).resolve(), Path(collector).resolve()
    command = ["node", str(collector), "--root", str(root), "--request", str(request), "--output", str(output)]
    start = datetime.now(timezone.utc).isoformat()
    process = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=240, check=False)
    end = datetime.now(timezone.utc).isoformat()
    try:
        output_created = json.loads(process.stdout.strip().splitlines()[-1]).get("output_created") is True
    except (ValueError, KeyError, IndexError, AttributeError):
        output_created = False
    process.output_created = output_created
    if output_created:
        output = output_path(root, output, fresh=False)
        (output / "command-execution.json").write_text(json.dumps({"command": command, "cwd": str(root), "started_at": start,
            "finished_at": end, "exit_code": process.returncode, "stdout": process.stdout, "stderr": process.stderr,
            "scope": "offline_dom_remeasurement_only", **{k: False for k in FLAGS}}, indent=2) + "\n")
    return process


def recompute(root, evidence, output, collector):
    root = Path(root).resolve()
    output = output_path(root, output)
    original, request, errors = inspect(root, evidence, collector)
    if errors:
        result = unavailable("retained measurement invalid before independent replay")
        result["errors"] = errors
        return result
    process = collect(root, pinned(Path(root), original["request"]), output, collector)
    collection = {"attempted": True, "output_owned": process.output_created, "exit_code": process.returncode}
    if not process.output_created:
        result = unavailable("collector did not create the requested output; no foreign receipt was read")
        result["collection"] = collection
        return result
    result = compare(root, output, collector)
    result["collection"] = collection
    fresh, _, fresh_errors = inspect(root, output, collector)
    if fresh_errors:
        result["errors"].extend(fresh_errors)
    elif canonical([{k: r.get(k) for k in ("id", "status", "gaps", "facts", "screenshot")} for r in original["subjects"]]) != canonical([{k: r.get(k) for k in ("id", "status", "gaps", "facts", "screenshot")} for r in fresh["subjects"]]):
        result["errors"].append("retained facts differ from independent fresh browser replay")
    result["independent_replay"] = {"original_measurement_sha256": digest(Path(evidence) / "measurement.json"),
                                    "fresh_measurement_sha256": digest(Path(output) / "measurement.json"), "exit_code": process.returncode}
    if result["errors"]:
        result["status"] = "missing_render_evidence"
    else:
        result["independently_recomputed"] = True
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("measure", "inspect", "recompute"))
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--collector", type=Path, required=True)
    parser.add_argument("--request", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        return _run_cli(args)
    except (ValueError, OSError) as error:
        print(json.dumps({"status": "missing_render_evidence", "errors": [str(error)], "output": None}))
        return 1


def _run_cli(args):
    args.root = Path(args.root).resolve()
    if args.output is not None:
        args.output = output_path(args.root, args.output)
    if args.mode in ("measure", "recompute"):
        require(args.output, "fresh ignored output required")
    if args.mode == "measure":
        require(args.request and args.output, "measure needs request and fresh output")
        execution = collect(args.root, args.request, args.output, args.collector)
        if execution.output_created:
            result = compare(args.root, args.output, args.collector)
        else:
            result = unavailable("collector did not create the requested output; no foreign receipt was read")
        result["collection"] = {"attempted": True, "output_owned": execution.output_created, "exit_code": execution.returncode}
    elif args.mode == "recompute":
        require(args.evidence and args.output, "recompute needs evidence and fresh output")
        result = recompute(args.root, args.evidence, args.output, args.collector)
    else:
        require(args.evidence, "inspect needs evidence")
        result = compare(args.root, args.evidence, args.collector)
        if not args.output:
            print(json.dumps(result, ensure_ascii=False))
            return 0 if result["status"] == "comparable" and not result["errors"] else 1
    collection = result.get("collection")
    if collection and not collection["output_owned"]:
        print(json.dumps({"status": result["status"], "errors": result["errors"], "output": None, "gaps": result["gaps"]}))
        return 1
    directory = output_path(args.root, args.output, fresh=not bool(collection))
    if not collection:  # Invalid original evidence or read-only inspect: claim our own fresh directory.
        try:
            directory.parent.mkdir(parents=True, exist_ok=True)
            directory = output_path(args.root, directory)
            directory.mkdir(exist_ok=False)
        except FileExistsError:
            print(json.dumps({"status": "missing_render_evidence", "errors": ["output ownership lost"], "output": None}))
            return 1
    directory = output_path(args.root, directory, fresh=False)
    destination = directory / "comparison.json"
    with destination.open("x") as stream:
        stream.write(json.dumps(result, indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    print(json.dumps({"status": result["status"], "errors": result["errors"], "output": str(destination)}, ensure_ascii=False))
    return 0 if result["status"] == "comparable" and not result["errors"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
