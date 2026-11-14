"""Fail-closed RW coverage and fresh, design-only execution evidence.

The product CSV owns acceptance identities.  This module owns adapter capabilities
and layer boundaries; neither a registry flag nor design screenshots can invent a
native, language-worker, or game execution adapter.
"""
from __future__ import annotations

import ast
import csv
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import subprocess
import time
from typing import Any, Mapping
import uuid
from urllib.parse import parse_qs, urlsplit

from gate_core import digest, safe_path, source_files

REGISTRY = 'docs/development/reading-window-test-coverage.json'
ACCEPTANCE = 'docs/product/reading-window-session-acceptance.csv'
DESIGN_SCRIPT = 'scripts/report/verify_interaction_gap_design.cjs'
ADAPTER_ID = 'reading-window-design-coverage'
LAYERS = ('design', 'product_component', 'real_pipeline', 'real_language', 'native', 'game')
# Empty registries are intentional: these real execution adapters do not exist.
LAYER_ADAPTERS: dict[str, frozenset[str]] = {
    'design': frozenset({ADAPTER_ID}),
    'product_component': frozenset(),
    'real_pipeline': frozenset(),
    'real_language': frozenset(),
    'native': frozenset(),
    'game': frozenset(),
}
REQUIRED_DIMENSIONS = frozenset({
    *(f'lexical-{number:03b}' for number in range(8)),
    'select-all-entrypoints', 'select-pointer-keyboard-single-highlight',
    'select-internal-long-scroll', 'select-empty-loading-error',
    'session-paused-visible-sequence', 'session-full-paging',
    'preview-cancel-stale-frame', 'hud-toolbar-500-560',
})
EVIDENCE_REQUIREMENTS = {
    'design': frozenset({'fresh_process', 'asserted_observations', 'source_identity', 'capture_integrity'}),
    'product_component': frozenset({'real_product_component', 'action_and_observable_result', 'source_identity'}),
    'real_pipeline': frozenset({'actual_ingress', 'repository_identity', 'persisted_sequence', 'pagination_reconstruction', 'source_identity'}),
    'real_language': frozenset({'actual_worker', 'packaged_resource_identity', 'span_reconstruction', 'source_identity'}),
    'native': frozenset({'final_package_identity', 'os_action_and_result', 'native_window_identity', 'source_identity'}),
    'game': frozenset({'final_package_identity', 'core_resource_identity', 'real_callback_identity', 'game_identity', 'source_identity'}),
}
SHA256 = re.compile(r'^[0-9a-f]{64}$')

# Module/component semantics are reviewed group responsibilities, not just known IDs.
ARCHITECTURE_REQUIREMENTS = (
    (range(1, 7), {'M02', 'M12', 'M13'}, {'C08', 'C13', 'C14', 'C35'}),
    (range(7, 13), {'M11', 'M12', 'M13'}, {'C18', 'C32', 'C34'}),
    (range(13, 17), {'M13'}, {'C04', 'C06', 'C07'}),
    (range(17, 24), {'M02', 'M03', 'M10', 'M13', 'M14', 'M15'}, {'C23', 'C33', 'C36'}),
    (range(24, 31), {'M07', 'M08', 'M11', 'M12', 'M13'}, {'C01', 'C16', 'C18'}),
    (range(31, 35), {'M13', 'M15'}, {'C01', 'C02', 'C04', 'C06', 'C10', 'C18'}),
)
STATE_TAG_ROLE = 'planning_only; actual coverage is code-bound design_dimensions'
ASSERT_METHODS = frozenset({'ok', 'equal', 'notEqual', 'deepEqual', 'match'})
# Reuse Node's own assertions and existing PNG decoder. No second Python model of JS
# equality/RegExp semantics, no screenshot rewrites, and no user-supplied commands.
PROOF_INSPECTOR = r"""
'use strict';
const fs=require('node:fs'),assert=require('node:assert/strict');
const input=JSON.parse(fs.readFileSync(0,'utf8'));
const {PNG}=require(input.pngjs_module);
const methods=new Set(['ok','equal','notEqual','deepEqual','match']);
const errors=[];
for(const row of input.assertions){
 try{
  if(!methods.has(row.method))throw new Error('Unknown assertion method');
  if(!Object.hasOwn(row,'actual')||!Object.hasOwn(row,'expected'))throw new Error('Missing assertion operands');
  if(row.method==='match'){
   if(typeof row.expected!=='string')throw new Error('Serialized RegExp required');
   const last=row.expected.lastIndexOf('/');
   if(!row.expected.startsWith('/')||last<=0)throw new Error('Invalid serialized RegExp');
   assert.match(row.actual,new RegExp(row.expected.slice(1,last),row.expected.slice(last+1)));
  }else if(row.method==='ok')assert.ok(row.actual);
  else assert[row.method](row.actual,row.expected);
 }catch(error){errors.push(row.check_id+': assertion replay rejected: '+error.message);}
}
for(const row of input.captures){
 try{
  const size=fs.statSync(row.path).size;
  if(size<33||size>33554432)throw new Error('PNG byte limit');
  const data=fs.readFileSync(row.path);
  if(data.subarray(0,8).toString('hex')!=='89504e470d0a1a0a'||data.toString('ascii',12,16)!=='IHDR')throw new Error('PNG header');
  const width=data.readUInt32BE(16),height=data.readUInt32BE(20);
  if(width<1||height<1||width>8192||height>8192||width*height>16777216)throw new Error('PNG dimension limit');
  const decoded=PNG.sync.read(data,{checkCRC:true});
  if(decoded.width!==width||decoded.height!==height||decoded.data.length!==width*height*4)throw new Error('PNG decoded geometry mismatch');
 }catch(error){errors.push(row.id+': actual PNG decode rejected: '+error.message);}
}
process.stdout.write(JSON.stringify({errors,assertions:input.assertions.length,captures:input.captures.length}));
"""



def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f'Duplicate JSON object key: {key}')
        result[key] = value
    return result


def _read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique_object)


def _unique(values: Any, label: str, errors: list[str]) -> set[str]:
    if not isinstance(values, list) or any(not isinstance(value, str) or not value for value in values):
        errors.append(f'{label}: nonempty string list required')
        return set()
    if len(values) != len(set(values)):
        errors.append(f'{label}: duplicate values')
    return set(values)


def _identity(files: Mapping[str, str]) -> str:
    return hashlib.sha256(json.dumps(dict(files), sort_keys=True).encode()).hexdigest()


def acceptance_rows(root: Path) -> list[dict[str, str]]:
    """Read identities from the product authority, never a duplicate ID list."""
    with safe_path(root, ACCEPTANCE).open(encoding='utf-8', newline='') as stream:
        return list(csv.DictReader(stream))


def _number(case_id: str) -> int:
    match = re.fullmatch(r'RW-(\d{2})', case_id)
    if not match:
        raise ValueError(f'Invalid acceptance identity: {case_id}')
    return int(match.group(1))


def required_layers(case_id: str) -> frozenset[str]:
    """Reviewed applicability policy, separate from authority case enumeration."""
    number = _number(case_id)
    if not 1 <= number <= 34:
        raise ValueError(f'Acceptance needs reviewed layer policy: {case_id}')
    result: set[str] = set()
    if number not in {6, 12}:  # Explicit real-device proof requirements.
        result.update(('design', 'product_component'))
    if 17 <= number <= 23 or number == 33:
        result.add('real_pipeline')
    if number in {26, 27, 28, 29, 30, 33}:
        result.add('real_language')
    if number <= 12 or number in {24, 25, 30, 31, 32, 33}:
        result.add('native')
    if number <= 12 or 17 <= number <= 30 or number == 33:
        result.add('game')
    return frozenset(result)


def design_capabilities(root: Path) -> dict[str, dict[str, Any]]:
    """Inspect code-registered calls and literal capabilities, not registry claims."""
    source = safe_path(root, DESIGN_SCRIPT).read_text(encoding='utf-8')
    pairs = re.findall(r"await check\('([^']+)',\s*(\[[^\n]*?\])\s*,", source)
    if not pairs:
        raise ValueError('Design adapter has no code-registered check calls')
    calls: dict[str, list[str]] = {}
    for check_id, literal in pairs:
        if check_id in calls:
            raise ValueError(f'Duplicate code-registered design check: {check_id}')
        requirements = ast.literal_eval(literal)
        if not isinstance(requirements, list) or not requirements or len(requirements) != len(set(requirements)):
            raise ValueError(f'Invalid design requirements: {check_id}')
        calls[check_id] = requirements
    match = re.search(r'/\* RW_CAPABILITIES_BEGIN \*/\s*const DESIGN_CAPABILITIES\s*=\s*(\{.*?\})\s*;\s*/\* RW_CAPABILITIES_END \*/', source, re.S)
    if not match:
        raise ValueError('Design adapter lacks a closed literal capability registry')
    capabilities = json.loads(match.group(1), object_pairs_hook=_unique_object)
    if not isinstance(capabilities, dict) or set(capabilities) != set(calls):
        raise ValueError('Literal design capabilities differ from actual code-registered check calls')
    known_ids = {row['case_id'] for row in acceptance_rows(root)}
    for check_id, values in capabilities.items():
        if not isinstance(values, dict):
            raise ValueError(f'Invalid capability: {check_id}')
        dimensions = values.get('dimensions')
        errors: list[str] = []
        _unique(dimensions, f'{check_id} dimensions', errors)
        if errors or not set(calls[check_id]) <= known_ids:
            raise ValueError(f'Invalid capability requirements/dimensions: {check_id}: {errors}')
        values['requirements'] = calls[check_id]
    return capabilities


def _actual_design_entries(root: Path) -> tuple[set[str], set[str]]:
    directory = safe_path(root, 'docs/design/first-product-v1')
    source = '\n'.join(path.read_text(encoding='utf-8') for path in directory.iterdir() if path.is_file() and path.suffix in {'.js', '.html'})
    scenarios = set(re.findall(r"(?:scenario|params\.get\(['\"]scenario['\"]\))\s*===\s*['\"]([^'\"]+)['\"]", source))
    for array in re.findall(r"(\[[^\]\n]+\])\.includes\(params\.get\(['\"]scenario['\"]\)\)", source):
        scenarios.update(ast.literal_eval(array))
    anchors = set(re.findall(r"\bid=['\"]([A-Za-z][\w.-]*)['\"]", source))
    # The session view uses a finite, literal ternary for its actual section ID.
    # Resolve only variables used directly as id, never every string in the file.
    for variable in re.findall(r"\bid=['\"][ '\"]*\+\s*([A-Za-z]\w*)\s*\+", source):
        for first, second in re.findall(r"\b" + re.escape(variable) + r"\s*=[^;\n]+?\?\s*['\"]([^'\"]+)['\"]\s*:\s*['\"]([^'\"]+)['\"]", source):
            anchors.update((first, second))
    return scenarios, anchors


def _inspect_actual_proofs(root: Path, assertions: list[dict[str, Any]], captures: list[dict[str, str]]) -> list[str]:
    module = Path(__file__).resolve().parents[2] / 'node_modules/pngjs'
    try:
        result = subprocess.run(['node', '-e', PROOF_INSPECTOR], cwd=root, input=json.dumps({'assertions': assertions, 'captures': captures, 'pngjs_module': str(module)}), capture_output=True, text=True, timeout=30, check=False)
        if result.returncode:
            return ['Independent assertion/PNG inspector failed: ' + result.stderr[-2000:]]
        data = json.loads(result.stdout)
        if data.get('assertions') != len(assertions) or data.get('captures') != len(captures) or not isinstance(data.get('errors'), list):
            return ['Independent proof inspector returned a mismatched inventory']
        return data['errors']
    except (OSError, ValueError, TypeError, subprocess.TimeoutExpired) as error:
        return [f'Independent proof inspector failed: {error}']


def _known_csv(root: Path, path: str, column: str) -> set[str]:
    with safe_path(root, path).open(encoding='utf-8', newline='') as stream:
        return {row[column] for row in csv.DictReader(stream)}


def _validate_registry(root: Path) -> list[str]:
    """Reject missing cases, removed layers, substitute adapters, and weak maps."""
    root = Path(root).resolve()
    errors: list[str] = []
    try:
        data = _read_json(safe_path(root, REGISTRY))
        rows = acceptance_rows(root)
        capabilities = design_capabilities(root)
        modules = _known_csv(root, 'docs/architecture/engineering-modules.csv', 'module_id')
        components = _known_csv(root, 'docs/architecture/engineering-components.csv', 'component_id')
        scenarios, anchors = _actual_design_entries(root)
    except (OSError, ValueError, TypeError, KeyError, SyntaxError) as error:
        return [f'RW coverage input invalid: {error}']
    if type(data.get('schema_version')) is not int or data.get('schema_version') != 1 or data.get('acceptance_source') != ACCEPTANCE or data.get('design_script') != DESIGN_SCRIPT:
        errors.append('RW coverage must use the authoritative CSV and code-registered design script')
    if data.get('state_tags_role') != STATE_TAG_ROLE:
        errors.append('Design state labels must remain explicit planning tags, not executed proof')
    if data.get('evidence_layers') != list(LAYERS):
        errors.append('RW evidence layers are closed and cannot be renamed or removed')
    authority = [row.get('case_id') for row in rows]
    if not authority or len(authority) != len(set(authority)):
        errors.append('Authoritative acceptance CSV is empty or contains duplicate IDs')
    cases = data.get('cases', [])
    if not isinstance(cases, list) or any(not isinstance(row, dict) for row in cases):
        return errors + ['RW coverage cases must be objects']
    case_ids = [row.get('id') for row in cases]
    if len(case_ids) != len(set(case_ids)) or set(case_ids) != set(authority):
        errors.append('RW coverage must map every authoritative CSV case exactly once')
    if data.get('required_dimensions') != sorted(REQUIRED_DIMENSIONS):
        errors.append('Mandatory behavior dimensions cannot be removed or renamed')
    code_dimensions = {dimension for item in capabilities.values() for dimension in item['dimensions']}
    if not REQUIRED_DIMENSIONS <= code_dimensions:
        errors.append('Design adapter omits required dimensions: ' + ', '.join(sorted(REQUIRED_DIMENSIONS - code_dimensions)))
    authority_map = {row['case_id']: row for row in rows}
    for row in cases:
        case_id = row.get('id', '')
        if case_id not in authority_map:
            continue
        label = f'RW coverage {case_id}'
        try:
            required = required_layers(case_id)
        except ValueError as error:
            errors.append(str(error)); continue
        if not _unique(row.get('design_states'), f'{label} design_states', errors):
            errors.append(f'{label}: explicit states/combinations required')
        if row.get('design_expectation') != authority_map[case_id]['expected']:
            errors.append(f'{label}: observable expectation differs from authority')
        if row.get('source_requirements') != authority_map[case_id]['source_requirements'].split('/'):
            errors.append(f'{label}: source requirement mapping differs from authority')
        minimum_modules, minimum_components = next((modules, components) for numbers, modules, components in ARCHITECTURE_REQUIREMENTS if _number(case_id) in numbers)
        for key, known, minimum in [('module_ids', modules, minimum_modules), ('component_ids', components, minimum_components)]:
            values = _unique(row.get(key), f'{label} {key}', errors)
            if not values or not values <= known:
                errors.append(f'{label}: missing or unknown {key}')
            if not minimum <= values:
                errors.append(f'{label}: {key} omits reviewed responsibility: {sorted(minimum - values)}')
        if not isinstance(row.get('owner'), str) or not row['owner'].strip():
            errors.append(f'{label}: responsible Owner required')
        refs = row.get('design_refs')
        if not isinstance(refs, list) or not refs:
            errors.append(f'{label}: reachable design entry/state required')
        else:
            for ref in refs:
                try:
                    parsed = urlsplit(ref)
                    query = parse_qs(parsed.query)
                    if parsed.path != 'docs/design/first-product-v1/index.html' or parsed.scheme or parsed.netloc or set(query) != {'scenario'} or len(query['scenario']) != 1 or query['scenario'][0] not in scenarios or (parsed.fragment and parsed.fragment not in anchors) or not safe_path(root, parsed.path).is_file():
                        raise ValueError('unreachable or unregistered design scenario/anchor')
                except (ValueError, TypeError, AttributeError) as error:
                    errors.append(f'{label}: invalid design reference: {error}')
        design_ids = _unique(row.get('design_checks'), f'{label} design_checks', errors)
        expected_design = {key for key, value in capabilities.items() if case_id in value['requirements']}
        expected_dimensions = {dimension for key in expected_design for dimension in capabilities[key]['dimensions']}
        if _unique(row.get('design_dimensions'), f'{label} design_dimensions', errors) != expected_dimensions:
            errors.append(f'{label}: actual code-bound design dimensions differ')
        if design_ids != expected_design:
            errors.append(f'{label}: design check mapping differs from actual registered capabilities')
        if 'design' in required and not design_ids:
            errors.append(f'{label}: applicable design layer has no executable check')
        layers = row.get('layers', {})
        if not isinstance(layers, dict) or set(layers) != set(LAYERS):
            errors.append(f'{label}: every closed evidence layer needs explicit applicability'); continue
        for layer in LAYERS:
            value = layers[layer]
            if not isinstance(value, dict):
                errors.append(f'{label}/{layer}: layer must be an object'); continue
            applicable = layer in required
            if value.get('applicable') is not applicable:
                errors.append(f'{label}/{layer}: reviewed applicability cannot be weakened')
            if not isinstance(value.get('reason'), str) or not value['reason'].strip():
                errors.append(f'{label}/{layer}: applicability explanation required')
            checks = _unique(value.get('adapter_checks'), f'{label}/{layer} adapter_checks', errors)
            if not checks <= LAYER_ADAPTERS[layer] or (layer == 'design' and applicable and checks != {ADAPTER_ID}):
                errors.append(f'{label}/{layer}: unknown adapter or cross-layer substitute')
            expected_evidence = EVIDENCE_REQUIREMENTS[layer] if applicable else frozenset()
            if _unique(value.get('required_evidence'), f'{label}/{layer} required_evidence', errors) != expected_evidence:
                errors.append(f'{label}/{layer}: required evidence contract missing or changed')
            expected_status = 'not_run' if applicable else 'not_applicable'
            if value.get('status') != expected_status:
                errors.append(f'{label}/{layer}: planning registry cannot self-declare execution passed')
    return errors


def validate_registry(root: Path) -> list[str]:
    """Return errors for malformed registrations as well as semantic omissions."""
    try:
        return _validate_registry(root)
    except (ValueError, OSError, TypeError, KeyError, AttributeError, IndexError, SyntaxError) as error:
        return [f'RW coverage input invalid: {error}']


def release_coverage_gaps(root: Path, design_receipt: Mapping[str, Any] | None = None) -> list[dict[str, str]]:
    """Return real missing layers; this function never promotes design into runtime."""
    errors = validate_registry(root)
    if errors:
        return [{'case_id': '*', 'layer': 'registry', 'status': 'invalid', 'reason': error} for error in errors]
    design_verified = _validated_design_summary(root, design_receipt)
    result: list[dict[str, str]] = []
    for row in _read_json(safe_path(root, REGISTRY))['cases']:
        for layer, details in row['layers'].items():
            if not details['applicable']:
                continue
            # A caller-supplied dict is never proof of an actual fresh design run.
            # The runner owns validated status, but release still needs all real layers.
            if layer == 'design' and design_verified:
                continue
            result.append({'case_id': row['id'], 'layer': layer, 'status': 'not_run',
                           'reason': 'Code-registered adapter not implemented' if not LAYER_ADAPTERS[layer] else 'Fresh validated design execution required'})
    return result


def _validated_design_summary(root: Path, summary: Mapping[str, Any] | None) -> bool:
    """Reinspect immutable artifacts; a hand-filled passed summary proves nothing."""
    if not isinstance(summary, Mapping) or summary.get('validated_by') != ADAPTER_ID or summary.get('status') != 'passed' or summary.get('errors'):
        return False
    try:
        receipt = safe_path(root, summary['receipt'])
        manifest = safe_path(root, summary['source_manifest']['path'])
        if digest(receipt) != summary['receipt_sha256'] or digest(manifest) != summary['source_manifest']['sha256']:
            return False
        process = summary['process']
        if process.get('source_manifest') != summary['source_manifest']:
            return False
        return not validate_design_receipt(root, receipt, nonce=summary['execution_nonce'],
            started=process['started_at_epoch'], completed=process['completed_at_epoch'],
            source_before=_read_json(manifest)['files'], process=process)
    except (KeyError, ValueError, TypeError, OSError):
        return False


def _timestamp(value: Any) -> float:
    if not isinstance(value, str):
        raise ValueError('timestamp string required')
    date = datetime.fromisoformat(value.replace('Z', '+00:00'))
    if date.tzinfo is None:
        raise ValueError('UTC/offset timezone required')
    return date.timestamp()


def validate_design_receipt(root: Path, receipt: Path, *, nonce: str,
                            started: float, completed: float,
                            source_before: Mapping[str, str],
                            process: Mapping[str, Any]) -> list[str]:
    """Independently inspect per-check claims and per-image bytes of one fresh run."""
    root, receipt = Path(root).resolve(), Path(receipt).absolute()
    errors = validate_registry(root)
    try:
        if not receipt.is_relative_to(root):
            raise ValueError('Design receipt must be repository-contained evidence')
        safe_path(root, str(receipt.relative_to(root)))
        report = _read_json(receipt)
        capabilities = design_capabilities(root)
        if type(report.get('schema_version')) is not int or report.get('schema_version') != 2 or report.get('execution_nonce') != nonce or not isinstance(nonce, str) or len(nonce) < 16:
            errors.append('Design receipt lacks this actual execution nonce/schema')
        emitted_start, emitted_end = _timestamp(report.get('started_at')), _timestamp(report.get('finished_at'))
        if not all(math.isfinite(value) for value in (started, completed, emitted_start, emitted_end)) or not started <= emitted_start <= emitted_end <= completed:
            errors.append('Design receipt is stale or outside actual process lifetime')
        expected_source = _identity(source_before)
        current = source_files(root)
        if current != source_before:
            errors.append('RW source closure changed during/after actual design execution')
        for key in ('source_tree_sha256_before', 'source_tree_sha256_after'):
            if report.get(key) != expected_source:
                errors.append(f'Design receipt has stale/wrong source identity: {key}')
        for key in ('source_files_sha256_before', 'source_files_sha256_after'):
            if report.get(key) != dict(source_before):
                errors.append(f'Design receipt did not independently hash the full source closure: {key}')
        if report.get('qa_script_sha256') != digest(safe_path(root, DESIGN_SCRIPT)) or report.get('coverage_registry_sha256') != digest(safe_path(root, REGISTRY)):
            errors.append('Design script/coverage registry identity mismatch')
        if report.get('layer', 'design') != 'design':
            errors.append('Design receipt cannot rename its evidence layer')
        if report.get('design_only') is not True or report.get('product_regression_executed') is not False:
            errors.append('Design evidence may not claim product execution')
        for key in ('real_game', 'native', 'global_hotkey', 'tokenizer', 'live_capture'):
            if report.get(key) is not False:
                errors.append(f'Design evidence may not substitute runtime layer: {key}')
        if process.get('argv') != ['node', DESIGN_SCRIPT, process.get('output_directory')] or type(process.get('exit_code')) is not int or process.get('exit_code') != 0:
            errors.append('Design receipt lacks successful code-registered process execution')
        if process.get('execution_nonce') != nonce or process.get('source_tree_sha256') != expected_source:
            errors.append('Actual process identity does not match design receipt')
        if process.get('started_at_epoch') != started or process.get('completed_at_epoch') != completed:
            errors.append('Actual process lifetime mismatch')
        output_directory = Path(process.get('output_directory', '')).absolute()
        if not output_directory.is_relative_to(root):
            raise ValueError('Actual process output directory escapes repository')
        safe_path(root, str(output_directory.relative_to(root)))
        if not output_directory.is_dir() or not receipt.is_relative_to(output_directory):
            errors.append('Actual process output directory does not contain this receipt')
        source_manifest = process.get('source_manifest', {})
        manifest = safe_path(root, source_manifest['path'])
        if not manifest.is_file() or digest(manifest) != source_manifest.get('sha256') or _read_json(manifest).get('files') != dict(source_before):
            errors.append('Actual process source manifest identity differs from this execution')
        if manifest.name != 'source-manifest.json' or manifest.parent not in {output_directory, output_directory.parent}:
            errors.append('Actual process source manifest belongs to a different run')
        process_paths = process.get('artifacts', {})
        process_stdout = None
        if not isinstance(process_paths, dict) or set(process_paths) != {'stdout', 'stderr', 'log'}:
            errors.append('Actual process stdout/stderr/log artifacts required')
        else:
            for name, item in process_paths.items():
                try:
                    actual = safe_path(root, item['path'])
                    if not actual.is_file() or digest(actual) != item['sha256'] or (name != 'stderr' and actual.stat().st_size == 0):
                        raise ValueError('missing/empty/modified process artifact')
                    if actual.parent != manifest.parent:
                        raise ValueError('process artifacts belong to a different run')
                    if name == 'stdout':
                        process_stdout = _read_json(actual)
                except (ValueError, OSError, TypeError, KeyError) as error:
                    errors.append(f'Design process {name} artifact invalid: {error}')
        if not isinstance(process_stdout, dict) or process_stdout.get('verification_sha256') != digest(receipt) or Path(process_stdout.get('output', '')).absolute() != receipt.parent:
            errors.append('Design receipt is not bound to actual process stdout verification hash/output')
        actual_assertions = []
        actual_captures = []
        checks = report.get('checks', [])
        if not isinstance(checks, list) or any(not isinstance(check, dict) for check in checks):
            raise ValueError('Design check receipts must be objects')
        ids = [check.get('id') for check in checks]
        if len(ids) != len(set(ids)) or set(ids) != set(capabilities):
            errors.append('Actual design check receipt differs from closed code capabilities')
        screenshot_rows = report.get('screenshots', [])
        if not isinstance(screenshot_rows, list) or any(not isinstance(row, dict) for row in screenshot_rows):
            raise ValueError('Design screenshot manifest must contain objects')
        screenshot_ids = [row.get('id') for row in screenshot_rows]
        if not screenshot_ids or len(screenshot_ids) != len(set(screenshot_ids)):
            errors.append('Design screenshots must be nonempty and uniquely identified')
        screenshots = {row.get('id'): row for row in screenshot_rows}
        for capture_id, row in screenshots.items():
            try:
                path = safe_path(receipt.parent, row['path'])
                if path.suffix.lower() != '.png' or not path.is_file() or path.read_bytes()[:8] != b'\x89PNG\r\n\x1a\n' or not SHA256.fullmatch(row.get('sha256', '')) or digest(path) != row['sha256']:
                    raise ValueError('missing/modified/non-PNG capture')
                actual_captures.append({'id': capture_id, 'path': str(path)})
                linked = _unique(row.get('check_ids'), f'Capture {capture_id} check_ids', errors)
                if not linked or not linked <= set(capabilities):
                    raise ValueError('capture must belong to actual registered checks')
            except (ValueError, OSError, KeyError, TypeError) as error:
                errors.append(f'Invalid design capture {capture_id}: {error}')
        actual_dimensions: set[str] = set()
        for check in checks:
            check_id = check.get('id')
            if check_id not in capabilities:
                continue
            capability = capabilities[check_id]
            if check.get('layer', 'design') != 'design':
                errors.append(f'{check_id}: design check cannot claim another evidence layer')
            if check.get('status') != 'passed' or type(check.get('assertion_count')) is not int or check['assertion_count'] <= 0:
                errors.append(f'{check_id}: real nonzero passed assertions required')
            if check.get('requirements') != capability['requirements']:
                errors.append(f'{check_id}: receipt changed acceptance mapping')
            dimensions = _unique(check.get('dimensions'), f'{check_id} dimensions', errors)
            if dimensions != set(capability['dimensions']):
                errors.append(f'{check_id}: receipt omitted/renamed capability dimensions')
            actual_dimensions.update(dimensions)
            captures = _unique(check.get('capture_ids'), f'{check_id} capture_ids', errors)
            observations = check.get('observations')
            if not isinstance(observations, list) or any(not isinstance(value, (str, dict)) or not value for value in observations):
                errors.append(f'{check_id}: actual observable results must be explicit')
            elif not observations and not captures:
                errors.append(f'{check_id}: screenshots/observations cannot be absent')
            else:
                assertion_records = [item for item in observations if isinstance(item, dict) and item.get('kind') == 'assertion']
                if len(assertion_records) != check.get('assertion_count') or any(item.get('method') not in ASSERT_METHODS or 'actual' not in item or 'expected' not in item for item in assertion_records):
                    errors.append(f'{check_id}: assertion_count must match actual assertion observations with known methods and operands')
                actual_assertions.extend(dict(item, check_id=check_id) for item in assertion_records)
                observed_dimensions = {item.get('dimension') for item in observations if isinstance(item, dict) and item.get('kind') == 'observation' and item.get('value') is not None}
                if not dimensions <= observed_dimensions:
                    errors.append(f'{check_id}: declared dimensions lack actual observed results')
            for capture_id in captures:
                if capture_id not in screenshots or check_id not in screenshots[capture_id].get('check_ids', []):
                    errors.append(f'{check_id}: capture association is missing or unrelated')
        for capture_id, row in screenshots.items():
            for check_id in row.get('check_ids', []):
                if not any(check.get('id') == check_id and capture_id in check.get('capture_ids', []) for check in checks):
                    errors.append(f'{capture_id}: screenshot reverse association is not reciprocal')
        if not REQUIRED_DIMENSIONS <= actual_dimensions:
            errors.append('Actual execution omits mandatory behavior dimensions')
        if not errors:
            errors.extend(_inspect_actual_proofs(root, actual_assertions, actual_captures))
        if report.get('status') != 'passed' or report.get('artifact_qa_passed') is not True:
            errors.append('Design adapter did not execute all required checks successfully')
    except (ValueError, OSError, KeyError, TypeError, AttributeError) as error:
        errors.append(f'Design evidence invalid: {error}')
    return errors


def run_design_coverage(root: Path, out: Path) -> dict[str, Any]:
    """Run only the registered Node adapter in a unique evidence namespace."""
    root, out = Path(root).resolve(), Path(out).absolute()
    initial_errors = validate_registry(root)
    if initial_errors:
        return {'status': 'failed', 'validated_by': ADAPTER_ID, 'errors': initial_errors, 'runtime_layers': 'not_run'}
    if not out.is_relative_to(root):
        return {'status': 'failed', 'validated_by': ADAPTER_ID, 'errors': ['Output must be repository-contained'], 'runtime_layers': 'not_run'}
    safe_path(root, str(out.relative_to(root)))
    run = out / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ') + '-' + uuid.uuid4().hex[:8])
    run.mkdir(parents=True, exist_ok=False)
    before = source_files(root)
    source_identity = _identity(before)
    nonce = uuid.uuid4().hex
    manifest = run / 'source-manifest.json'
    manifest.write_text(json.dumps({'files': before}, sort_keys=True) + '\n', encoding='utf-8')
    node_output = run / 'design-artifacts'
    command = ['node', DESIGN_SCRIPT, str(node_output)]
    environment = dict(os.environ, LUNA_DESIGN_EXECUTION_NONCE=nonce,
                       LUNA_DESIGN_SOURCE_TREE_SHA256=source_identity,
                       LUNA_DESIGN_SOURCE_MANIFEST_JSON=str(manifest),
                       LUNA_DESIGN_COVERAGE_REGISTRY_SHA256=digest(safe_path(root, REGISTRY)))
    started = time.time()
    try:
        result = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True, timeout=300, check=False)
        stdout, stderr, code = result.stdout, result.stderr, result.returncode
    except (OSError, subprocess.TimeoutExpired) as error:
        stdout, stderr, code = '', str(error), -1
    completed = time.time()
    paths = {'stdout': run / 'run.stdout', 'stderr': run / 'run.stderr', 'log': run / 'run.log'}
    paths['stdout'].write_text(stdout, encoding='utf-8')
    paths['stderr'].write_text(stderr, encoding='utf-8')
    paths['log'].write_text('COMMAND ' + json.dumps(command) + '\n' + stdout + '\nSTDERR\n' + stderr, encoding='utf-8')
    process = {'argv': command, 'output_directory': str(node_output), 'exit_code': code,
               'source_manifest': {'path': str(manifest.relative_to(root)), 'sha256': digest(manifest)},
               'execution_nonce': nonce, 'source_tree_sha256': source_identity,
               'started_at_epoch': started, 'completed_at_epoch': completed,
               'artifacts': {name: {'path': str(path.relative_to(root)), 'sha256': digest(path)} for name, path in paths.items()}}
    (run / 'process.json').write_text(json.dumps(process, indent=2) + '\n', encoding='utf-8')
    receipts = list(node_output.rglob('verification.json')) if node_output.is_dir() else []
    errors = []
    if code != 0:
        errors.append(f'Design adapter process failed: exit {code}')
    if len(receipts) != 1:
        errors.append('Design adapter must emit exactly one fresh verification.json')
    else:
        errors.extend(validate_design_receipt(root, receipts[0], nonce=nonce, started=started,
                                              completed=completed, source_before=before, process=process))
    summary = {'schema_version': 1, 'validated_by': ADAPTER_ID, 'status': 'failed' if errors else 'passed',
               'errors': errors, 'scope': 'design-only; no product/native/language/game acceptance',
               'execution_nonce': nonce, 'source_tree_sha256': source_identity,
               'registry_sha256': digest(safe_path(root, REGISTRY)), 'process': process,
               'source_manifest': {'path': str(manifest.relative_to(root)), 'sha256': digest(manifest)},
               'receipt': str(receipts[0].relative_to(root)) if len(receipts) == 1 else None,
               'receipt_sha256': digest(receipts[0]) if len(receipts) == 1 else None,
               'runtime_layers': {layer: 'not_run' for layer in LAYERS if layer != 'design'}}
    summary['release_gaps'] = release_coverage_gaps(root, summary)
    (run / 'coverage-result.json').write_text(json.dumps(summary, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    return summary
