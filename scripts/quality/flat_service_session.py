#!/usr/bin/env python3
"""Verify the pending flat presentation design; never run a product service."""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import translation_service_selection as parent

ROOT=Path(__file__).resolve().parents[2]
CATALOG=parent.CATALOG
DESIGN_ROOT=parent.DESIGN_ROOT
CANDIDATE_ID='flat-service-session-20261030'
CANDIDATE=DESIGN_ROOT+'/candidates/'+CANDIDATE_ID
DESIGN=CANDIDATE+'/index.html'
MANIFEST=CANDIDATE+'/manifest.json'
CHECKER='scripts/quality/flat_service_session.py'
TESTS='scripts/quality/test_flat_service_session.py'
RUNNER='scripts/report/verify_flat_service_session.cjs'
PLAN='docs/development/flat-service-session-tests.md'
FL_IDS={f'FL-{n:02d}' for n in range(1,11)}
_safe=parent._safe
_load_json=parent._load_json
_output_path=parent._output_path
_text=parent._text
_strings=parent._strings
_positive=parent._positive
SHA256=parent.SHA256


def validate_catalog(catalog):
    errors=parent.validate_catalog(catalog)
    if not isinstance(catalog,dict):return errors
    d=catalog.get('flat_presentation_design')
    if not isinstance(d,dict):return errors+['flat_presentation_design object required']
    expected={'change_id':'design-flat-service-and-session','phase':'design_documentation_only','candidate_design':DESIGN+'?scenario=reading','candidate_configuration':DESIGN+'?scenario=settings','candidate_design_status':'pending_review'}
    for k,v in expected.items():
        if d.get(k)!=v:errors.append(f'flat_presentation_design.{k}: expected {v}')
    for key,fields in {
        'configuration_management':{'layout':'one_added_service_list_with_same_page_editor'},
        'current_session':{'layout':'same_page_independent_regions','regions':['body_history','all_extraction_events','hook_diagnostics'],'independent_view_state':['query','filter','page','scroll','follow','new_count']},
        'scope_boundaries':{'http_protocols':13,'http_presets':21,'offline_engines':4,'original_arrays_and_matrix_unchanged':True,'rust_business_authority_and_narrow_tauri_unchanged':True,'production_source_or_package_changed':False,'database_or_contract_changed':False,'approved_png_tokens_and_prior_candidates_changed':False,'product_acceptance':'not_run'},
        'functional_audit':{'no_empty_actions_or_fabricated_success':True}
    }.items():
        value=d.get(key)
        if not isinstance(value,dict):errors.append(f'{key}: object required');continue
        for name,want in fields.items():
            if type(value.get(name)) is not type(want) or value.get(name)!=want:errors.append(f'{key}.{name}: incorrect flat contract')
    sources=d.get('source_documents')
    if not _strings(sources) or PLAN not in sources:errors.append('flat sources must bind the test plan')
    rows=catalog.get('flat_acceptance')
    if not isinstance(rows,list) or any(not isinstance(x,dict) for x in rows):return errors+['flat_acceptance object list required']
    ids=[r.get('id') for r in rows]
    if not _strings(ids) or set(ids)!=FL_IDS or len(ids)!=10:errors.append('FL-01 through FL-10 must occur exactly once')
    for r in rows:
        if r.get('status')!='not_run':errors.append(f'{r.get("id")}: product must remain not_run')
        for k in ['label','assertion','maintenance']:
            if not _text(r.get(k)):errors.append(f'{r.get("id")}: {k} required')
        layers=r.get('verification_layers')
        if not _strings(layers) or 'design' not in layers or len(layers)<2:errors.append(f'{r.get("id")}: design and future product layer required')
        if not r.get('source_requirement_ids'):errors.append(f'{r.get("id")}: nonempty FP mapping required')
        for k,pattern,limit in [('source_requirement_ids',r'FP-(\d{2})',12),('source_configuration_requirement_ids',r'AI-CFG-(\d{2})',16),('source_reading_requirement_ids',r'RW-(\d{2})',34)]:
            refs=r.get(k)
            if not isinstance(refs,list) or (refs and not _strings(refs)) or any(not isinstance(v,str) or not (m:=re.fullmatch(pattern,v)) or not 1<=int(m[1])<=limit for v in (refs or [])):errors.append(f'{r.get("id")}: invalid {k}')
    return errors


def manifest_sources(root):
    m=_load_json(_safe(root,MANIFEST));paths=set()
    if not isinstance(m,dict):raise ValueError('flat manifest object required')
    for k,v in {'schema_version':1,'candidate_id':CANDIDATE_ID,'entry':DESIGN,'status':'pending_review','runtime_base_href':'../../','parent_candidate_id':parent.CANDIDATE_ID}.items():
        if type(m.get(k)) is not type(v) or m.get(k)!=v:raise ValueError('flat manifest invalid '+k)
    owned=m.get('owned_sources');shared=m.get('shared_dependencies');prior=m.get('parent_sources_unchanged')
    if not isinstance(owned,dict) or not isinstance(shared,dict) or not shared or not isinstance(prior,dict):raise ValueError('flat manifest dependency groups required')
    actual={p.name for p in (root/CANDIDATE).iterdir() if p.is_file() and p.name!='manifest.json'}
    if set(owned)!=actual or not {'flat-layout.js','flat-layout.css','service-picker.js','settings.js','index.html'}.issubset(owned):raise ValueError('flat manifest must bind every owned source')
    expected_prior={p.name for p in (root/parent.CANDIDATE).iterdir() if p.is_file()}
    if set(prior)!=expected_prior:raise ValueError('flat parent dependency list incomplete')
    for group,prefix in [(owned,CANDIDATE),(shared,DESIGN_ROOT),(prior,parent.CANDIDATE)]:
        for rel,digest in group.items():
            if not _text(rel) or not isinstance(digest,str) or not SHA256.fullmatch(digest):raise ValueError('flat dependency invalid hash/path')
            if group is not shared and len(Path(rel).parts)!=1:raise ValueError('flat owned/parent source must be top-level')
            p=prefix+'/'+rel
            if hashlib.sha256(_safe(root,p).read_bytes()).hexdigest()!=digest:raise ValueError('flat dependency identity mismatch: '+p)
            paths.add(p)
    assets=parent._references(_safe(root,DESIGN).read_text())
    if not {DESIGN_ROOT+'/'+a for a in assets}.issubset(paths):raise ValueError('flat manifest missing loaded dependency')
    if not {'candidates/'+CANDIDATE_ID+'/flat-layout.js','candidates/'+CANDIDATE_ID+'/flat-layout.css'}.issubset(assets):raise ValueError('flat presentation adapter must be loaded')
    return paths


def validate_plan(body):
    errors=[]
    rows=re.findall(r'^\|\s*(FL-\d{2})\s*\|.*$',body,re.M)
    if len(rows)!=10 or set(rows)!=FL_IDS:errors.append('flat plan must contain FL-01 through FL-10 once')
    full=re.findall(r'^\|\s*FL-\d{2}\s*\|.*$',body,re.M)
    if any(not re.search(r'[;；]\s*not_run\s*\|\s*$',row) for row in full):errors.append('flat plan product states must remain not_run')
    return errors


def check(root=ROOT):
    root=Path(root).resolve();errors=[]
    try:
        errors.extend(parent.check(root));catalog=_load_json(_safe(root,CATALOG));errors.extend(validate_catalog(catalog));manifest_sources(root)
        errors.extend(validate_plan(_safe(root,PLAN).read_text()))
        for ref in catalog['flat_presentation_design']['source_documents']:
            path,_,anchor=ref.partition('#');body=_safe(root,path).read_text()
            if anchor and f'id="{anchor}"' not in body:errors.append('flat anchor missing: '+ref)
    except (OSError,ValueError,TypeError,KeyError,UnicodeError,AttributeError) as e:errors.append('Flat design cannot be checked: '+str(e))
    return errors


def _source_hashes(root):
    paths=manifest_sources(root)|{CATALOG,PLAN,CHECKER,TESTS,RUNNER,MANIFEST,'scripts/quality/translation_service_selection.py','scripts/quality/offline_translation_design.py','openspec/technology-lock.json','docs/development/iteration-baseline.json'}
    for ref in _load_json(_safe(root,CATALOG))['flat_presentation_design']['source_documents']:paths.add(ref.split('#')[0])
    return {p:hashlib.sha256(_safe(root,p).read_bytes()).hexdigest() for p in sorted(paths)}


def _adapter_report(destination: Path) -> tuple[dict, list[str]]:
    """Independently verify named scenarios and PNG identities, never exit alone."""
    path = _safe(destination, "report.json")
    report = _load_json(path)
    if not isinstance(report, dict):
        return {}, ["Node/Playwright report must be a JSON object"]
    errors: list[str] = []
    if (type(report.get("schema_version")) is not int or report.get("schema_version") != 1
            or report.get("kind") != "flat_service_session_design_verification"
            or report.get("layer") != "design" or report.get("status") != "passed"
            or report.get("product_tests_executed") is not False):
        errors.append("Node/Playwright report must identify a passed selection design-only execution")
    if not _positive(report.get("assertions")) or report["assertions"] < len(FL_IDS):
        errors.append("Node/Playwright report must contain actual assertions for all ten scenarios")
    checks = report.get("checks")
    if not isinstance(checks, list) or not checks or any(not isinstance(row, dict) for row in checks):
        errors.append("Node/Playwright report requires unique passed FL scenario checks")
        checks = []
    ids = [row.get("id") for row in checks]
    if (not _strings(ids) or set(ids) != FL_IDS
            or any(row.get("status") != "passed" for row in checks)):
        errors.append("Node/Playwright report must pass every FL-01 through FL-10 exactly once")
    if any(not _positive(row.get("assertions")) for row in checks):
        errors.append("Every FL scenario must have positive actual assertions")
    if report.get("page_errors") != [] or report.get("network_requests") != [] or report.get("console_errors") != []:
        errors.append("Browser errors or external network requests reject design acceptance")
    screenshots = report.get("screenshots")
    if not isinstance(screenshots, list) or not screenshots or any(not isinstance(row, dict) for row in screenshots):
        errors.append("Node/Playwright report requires actual screenshot evidence")
        screenshots = []
    if not _strings([row.get("path") for row in screenshots]):
        errors.append("Node/Playwright screenshot paths must be unique strings")
    for row in screenshots:
        try:
            relative = row.get("path")
            if not _text(relative) or Path(relative).suffix.lower() != ".png":
                raise ValueError("Screenshot evidence must name a local PNG")
            data = _safe(destination, relative).read_bytes()
            if (not _positive(row.get("bytes")) or row.get("bytes") != len(data)
                    or row.get("sha256") != hashlib.sha256(data).hexdigest()):
                raise ValueError(f"Screenshot byte/hash identity mismatch: {relative}")
            if (len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR"
                    or not _positive(row.get("width")) or not _positive(row.get("height"))
                    or row["width"] != int.from_bytes(data[16:20], "big")
                    or row["height"] != int.from_bytes(data[20:24], "big")):
                raise ValueError(f"Screenshot PNG dimensions mismatch: {relative}")
        except (OSError, ValueError, TypeError) as error:
            errors.append(str(error))
    return {"path": "report.json", "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "assertions": report.get("assertions"), "checks": len(checks)}, errors


def execute(root: Path, output: Path) -> dict:
    """Run Node/Playwright and retain a new design-only receipt even on failure."""
    destination = _output_path(Path(root), Path(output))
    root = Path(root).resolve()
    errors = check(root)
    destination.mkdir(parents=True, exist_ok=False)
    receipt = {"schema_version": 1, "kind": "flat_service_session_design_execution",
               "layer": "design_only", "product_acceptance": "not_run", "candidate_design_status": "pending_review",
               "status": "failed", "started_at": datetime.now(timezone.utc).isoformat(),
               "root": str(root), "output": str(destination), "static_errors": errors,
               "command": None, "exit_code": None, "source_hashes_before": {}, "source_hashes_after": {}, "errors": []}
    stdout_path, stderr_path = destination / "stdout.log", destination / "stderr.log"
    stdout_path.touch()
    stderr_path.touch()
    try:
        if not errors:
            before = _source_hashes(root)
            receipt["source_hashes_before"] = before
            command = ["node", str(_safe(root, RUNNER)), str(root), str(destination)]
            receipt["command"] = command
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                try:
                    completed = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr, check=False, timeout=300)
                    receipt["exit_code"] = completed.returncode
                except subprocess.TimeoutExpired:
                    receipt["errors"].append("Node/Playwright selection adapter exceeded the 300-second execution budget")
                    receipt["exit_code"] = 124
                except OSError as error:
                    receipt["errors"].append(f"Cannot execute Node/Playwright selection adapter: {error}")
                    receipt["exit_code"] = 127
            after = _source_hashes(root)
            receipt["source_hashes_after"] = after
            if before != after:
                receipt["errors"].append("Controlled source bytes changed during execution")
            receipt["static_errors_after"] = check(root)
            if receipt["exit_code"] != 0:
                receipt["errors"].append("Node/Playwright selection adapter returned a nonzero exit code")
            try:
                identity, report_errors = _adapter_report(destination)
                receipt["adapter_report"] = identity
                receipt["errors"].extend(report_errors)
            except (OSError, ValueError, TypeError, UnicodeError) as error:
                receipt["errors"].append(f"Cannot verify Node/Playwright evidence: {error}")
            if not receipt["errors"] and not receipt["static_errors_after"]:
                receipt["status"] = "passed"
    except (OSError, ValueError, TypeError, KeyError, UnicodeError) as error:
        receipt["errors"].append(str(error))
    receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
    for label, path in (("stdout", stdout_path), ("stderr", stderr_path)):
        receipt[label] = {"path": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    (destination / "execution.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--execute", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    if args.execute != (args.output is not None):
        parser.error("--execute requires --output; --output is valid only with --execute")
    try:
        if args.execute:
            result = execute(args.root, args.output)
        else:
            errors = check(args.root)
            result = {"kind": "flat_service_session_design_static", "layer": "design_only",
                      "status": "failed" if errors else "passed", "errors": errors,
                      "product_acceptance": "not_run", "candidate_design_status": "pending_review"}
    except (OSError, ValueError) as error:
        result = {"status": "failed", "layer": "design_only", "product_acceptance": "not_run", "errors": [str(error)]}
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"Flat service/session design: {result['status']} (design only; product acceptance not_run)")
        for error in result.get("errors", []) + result.get("static_errors", []):
            print(f"- {error}")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
