"""Bridge retained current-component artifacts to independent render evidence.

No source preferences are changed. Old packs lacking replayable fonts, form
properties or client context remain missing/incomparable. Original images and
their strict pixel results are never modified by this diagnostic bridge.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import mimetypes
import os
import subprocess
from pathlib import Path
import uuid
from urllib.parse import urljoin

from gate_core import safe_path, digest
from current_reference_catalogue import CATALOGUE, load
from current_reference_catalogue import (CLIENT_RUNNER, client_surface,
    validate_client_capture, validate_client_facts)
from current_design_react_adapter import STATE_IDS
from render_evidence import collect, compare, inspect, recompute, source_identity, unavailable, FLAGS

CURRENT = 'all-page-main-session-normal'
COLLECTOR = 'scripts/report/measure_render_evidence.mjs'


def file_ref(root, path):
    path = Path(path)
    relative = path.relative_to(root).as_posix()
    path = safe_path(root, relative)
    return {'path': relative, 'sha256': digest(path), 'bytes': path.stat().st_size}


def retained(root, base, ref):
    if not isinstance(ref, dict) or set(ref) != {'path', 'sha256', 'bytes'}:
        raise ValueError('exact retained artifact descriptor required')
    path = safe_path(base, ref['path'])
    if type(ref['bytes']) is not int or path.stat().st_size != ref['bytes'] or digest(path) != ref['sha256']:
        raise ValueError('retained render input differs')
    path.relative_to(root)
    return path


def assets(root, base_url, files):
    result = []
    for url_path, file in sorted(files.items()):
        kind = mimetypes.guess_type(url_path)[0] or 'application/octet-stream'
        # The independent collector deliberately refuses executable assets.
        # Page scripts stay disabled; only original inert render bytes are used.
        if kind not in {'text/css', 'font/woff2', 'font/woff', 'font/ttf', 'font/otf',
                        'image/png', 'image/jpeg', 'image/svg+xml', 'image/webp'}:
            continue
        result.append({'url': urljoin(base_url, '/' + url_path),
                       'file': file_ref(root, file), 'media_type': kind})
    return result


def request_for_current(root, report_path, report, row, destination):
    """Exact finite region mapping; other states retain explicit missing mappings."""
    if row['state_id'] != CURRENT:
        raise ValueError('effective region/form mapping not yet implemented for this state')
    catalogue = load(root / CATALOGUE)
    if report['references']['catalogue'] != {'path': CATALOGUE, 'sha256': digest(root / CATALOGUE)}:
        raise ValueError('independent current reference catalogue differs')
    pack = safe_path(root, catalogue['pack_root'])
    generation = load(safe_path(pack, catalogue['generation']['path']))
    if digest(pack / catalogue['generation']['path']) != catalogue['generation']['sha256']:
        raise ValueError('source generation bytes differ')
    original = next(item for item in generation['results'] if item['state_id'] == CURRENT)
    source_path = retained(root, pack, original['source_dom'])
    product_path = retained(root, report_path.parent, row['production_dom'])
    source_dom, product_dom = load(source_path), load(product_path)
    geometry = load(retained(root, report_path.parent, row['geometry']))
    original_geometry = geometry['source_geometry']
    viewport = original_geometry['viewport']
    source_files = {}
    for relative, sha in {**catalogue['design_sources'], **catalogue['runtime_assets']}.items():
        path = safe_path(root, relative)
        if digest(path) != sha:
            raise ValueError('pinned original design asset changed: ' + relative)
        source_files[relative] = path
    build = report_path.parent / 'controlled-build'
    product_files = {p.relative_to(build).as_posix(): p for p in build.rglob('*') if p.is_file()}
    if not product_files:
        raise ValueError('retained actual controlled build assets missing')
    source_target = row['target_mapping']['source_selector']
    product_target = row['target_mapping']['target']
    inputs = load(retained(root, report_path.parent, row['inputs']))
    def subject(name, path, dom, asset_files, regions, token, properties):
        return {'id': name, 'evidence_kind': 'immutable_snapshot',
                'document': {'file': file_ref(root, path), 'format': 'dom_json'},
                'document_url': dom['url'],
                'viewport': {'width': viewport['width'], 'height': viewport['height'],
                             'device_scale_factor': viewport['deviceScaleFactor']},
                'assets': assets(root, dom['url'], asset_files),
                'regions': regions, 'token_selector': token, 'pos_selector': '[data-pos]',
                'media_queries': ['(max-width: 1100px)', '(max-width: 1024px)'],
                'required_scope': 'target',
                'required_properties': {'form_properties': False, 'focus': False},
                'declared_state': properties}
    preferences = inputs['preferences']
    source_preferences = inputs['source_state']['settings']['general']
    # State declarations are preserved as declarations, never browser mutations.
    request = {'schema_version': 1, 'kind': 'offline-render-request/v1',
               'nonce': uuid.uuid4().hex, 'source_tree_sha256': source_identity(root)['sha256'],
               'subjects': [
                   subject('reference', source_path, source_dom, source_files,
                           {'window': '#main-window', 'layout': '#main-content',
                            'source': source_target + ' .source',
                            'translation': source_target + ' .translation'},
                           '[data-token-index]',
                           {'showRuby': source_preferences['furigana'], 'showPos': source_preferences['pos']}),
                   subject('product', product_path, product_dom, product_files,
                           {'window': '#root > div', 'layout': '#root > div',
                            'source': product_target + ' [data-testid="source-text"]',
                            'translation': product_target + ' article > p'},
                           '[data-token-index]',
                           {'showRuby': preferences['showRuby'], 'showPos': preferences['showPos'],
                            'sourceFontPx': preferences['sourceFontPt'],
                            'translationFontPx': preferences['translationFontPt']})],
               'pairs': [['reference', 'product']]}
    destination.write_text(json.dumps(request, ensure_ascii=False, indent=2) + '\n')
    return destination


def qualify_report(root, report_path, output, *, diagnostic=False):
    from render_evidence import output_path
    root = Path(root).resolve()
    # Preserve lexical descendants until lstat/safe_path has rejected aliases.
    # resolve() first would silently bypass the collector's symlink guard.
    raw_report = Path(report_path)
    if '..' in raw_report.parts:
        raise ValueError('raw parent traversal in retained report path')
    report_path = Path(os.path.abspath(raw_report))
    report_path = safe_path(root, report_path.relative_to(root).as_posix())
    output = output_path(root, output)
    if not report_path.is_relative_to(root / '.build'):
        raise ValueError('retained component report must be in repository ignored evidence')
    if not output.is_relative_to(root / '.build') or output.exists():
        raise ValueError('fresh ignored qualification output required')
    report = load(report_path)
    if (report.get('kind') != 'current-design-real-react/v1' or report.get('native_verified') is not False
            or not isinstance(report.get('results'), list)):
        raise ValueError('retained real-component report required')
    identities = [row.get('state_id') for row in report['results'] if isinstance(row, dict)]
    if (len(identities) != len(report['results']) or len(identities) != len(STATE_IDS)
            or any(type(value) is not str for value in identities)
            or len(set(identities)) != len(STATE_IDS) or set(identities) != STATE_IDS):
        raise ValueError('exact finite current state closure required before writing any output')
    current_source = source_identity(root)['sha256']
    if not diagnostic and report['source_tree_sha256'] != current_source:
        raise ValueError('current component source differs from qualification tool context')
    output.mkdir()
    rows = []
    collector = root / COLLECTOR
    for row in report['results']:
        result = unavailable('effective region/form mapping not yet implemented for this state')
        directory = output / row['state_id']; directory.mkdir()
        if row['state_id'] == CURRENT:
            try:
                request = request_for_current(root, report_path, report, row, directory / 'request.json')
                execution = collect(root, request, directory / 'measurement', collector)
                if execution.returncode not in (0, 1):
                    raise ValueError('offline collector failed: ' + execution.stderr[-1000:])
                result = recompute(root, directory / 'measurement', directory / 'independent-replay', collector)
            except (OSError, ValueError, KeyError, TypeError, AttributeError, StopIteration) as error:
                result = unavailable(str(error))
        result.update(state_id=row['state_id'],
                      artifact_origin_source_sha256=report['source_tree_sha256'],
                      tooling_source_sha256=current_source,
                      diagnostic_context=diagnostic,
                      capture_scope='target', full_png_scope='browser_viewport',
                      whole_client_eligible=False)
        path = directory / 'qualification.json'
        path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        rows.append({'state_id': row['state_id'], 'status': result['status'],
                     'independently_recomputed': result['independently_recomputed'],
                     'artifact': file_ref(root, path)})
    index = {'schema_version': 1, 'kind': 'current-react-render-qualification-index/v1',
             'raw_report': file_ref(root, report_path), 'states': rows,
             'capture_scope': 'target', 'whole_client_eligible': False,
             'diagnostic_context': diagnostic, 'native_verified': False,
             'product_alignment_approved': False, 'completion_allowed': False}
    (output / 'index.json').write_text(json.dumps(index, ensure_ascii=False, indent=2) + '\n')
    return index


def inspect_qualification(root, report_path, row, *, fresh=False):
    """Rebuild region/assets from authority; optionally run a new browser replay."""
    root, report_path = Path(root).resolve(), Path(report_path)
    out = report_path.parent
    path = retained(root, out, row['render_qualification'])
    qualified = load(path)
    expected_source = load(report_path)['source_tree_sha256']
    if (qualified.get('state_id') != row['state_id'] or qualified.get('diagnostic_context') is not False
            or qualified.get('artifact_origin_source_sha256') != expected_source
            or qualified.get('tooling_source_sha256') != source_identity(root)['sha256']
            or qualified.get('capture_scope') != 'target'
            or qualified.get('full_png_scope') != 'browser_viewport'
            or qualified.get('whole_client_eligible') is not False
            or any(qualified.get(key) is not False for key in FLAGS)):
        raise ValueError('render qualification identity/scope/approval differs')
    if row.get('render_status') != qualified.get('status'):
        raise ValueError('displayed render status contradicts retained qualification')
    if row.get('capture_scope') != 'target' or row.get('full_png_scope') != 'browser_viewport':
        raise ValueError('component crop/viewport was promoted to a whole client')
    if row['state_id'] != CURRENT:
        expected = unavailable('effective region/form mapping not yet implemented for this state')
        for key in ('status', 'errors', 'gaps', 'independently_recomputed'):
            if qualified.get(key) != expected[key]:
                raise ValueError('missing region/form mapping was falsely qualified')
        if row['same_data_eligibility'].get('status') == 'eligible':
            raise ValueError('forced eligible despite missing effective region/form mapping')
        return [row['state_id'] + ': effective render mapping missing']
    measurement = path.parent / 'measurement'
    original, request, errors = inspect(root, measurement, root / COLLECTOR)
    if errors:
        return [row['state_id'] + ': render measurements invalid: ' + str(errors)]
    # A correctly rehashed alternate CSS/selector request is still unauthorized.
    directory = out / 'independent-render-contract-check' / uuid.uuid4().hex
    directory.mkdir(parents=True)
    expected_path = request_for_current(root, report_path, load(report_path), row, directory / 'expected-request.json')
    expected_request = load(expected_path)
    expected_request['nonce'] = request['nonce']
    if expected_request != request:
        raise ValueError('render request does not equal authoritative original DOM/assets/regions')
    actual = (recompute(root, measurement, directory / 'fresh-browser', root / COLLECTOR)
              if fresh else compare(root, measurement, root / COLLECTOR))
    for key in ('status', 'errors', 'gaps', 'pairs'):
        if qualified.get(key) != actual.get(key):
            raise ValueError('claimed render qualification differs from independent facts: ' + key)
    if qualified.get('independently_recomputed') is not True:
        raise ValueError('producer-only style comparison cannot substitute for a fresh replay')
    if fresh and actual.get('independently_recomputed') is not True:
        errors.append('independent fresh render replay unavailable')
    eligibility = row['same_data_eligibility']
    if eligibility.get('scope') != 'retained_dom_target' or eligibility.get('whole_client_eligible') is not False:
        raise ValueError('eligibility promoted a target to a whole client')
    if actual['status'] != 'comparable':
        errors.append(row['state_id'] + ': effective render state is ' + actual['status'])
        if eligibility.get('status') == 'eligible':
            raise ValueError('forced eligible despite independently missing/incomparable render facts')
    return errors


def compare_client_render_facts(reference, product, surface):
    """Compare measured whole-client state, never nominal preferences/pass labels.

    Producer/asset/DOM provenance is inspected separately by each layer's adapter.
    This function cannot convert a raw JSON dictionary into product evidence.
    Strict original PNG comparison remains the caller's independent obligation.
    """
    validate_client_facts(reference, surface); validate_client_facts(product, surface)
    keys = ('viewport', 'device_pixel_ratio', 'css_zoom', 'surface_box', 'content_box',
            'fonts', 'text_runs', 'lexical', 'forms', 'focus', 'running_animations')
    def effective(key, facts):
        value = copy.deepcopy(facts[key])
        if key == 'fonts':
            # Execution/compilation records have different absolute origins and
            # binary hashes on each fresh run. Their authority is independently
            # audited; they are not the rendered font. Keep actual file identity,
            # observed per-node CSS/platform values and all loaded-state facts.
            for font in value['files']:
                if font.get('resolution') == 'coretext_ui_instance_exact':
                    font.pop('ui_probe')
        return value
    differences = [key for key in keys if effective(key, reference) != effective(key, product)]
    version = 3 if surface['projection'] == 'approved_client_container_c1/v1' else 2
    return {'schema_version': version, 'kind': 'whole-client-effective-render-comparison/v' + str(version),
            'status': 'incomparable' if differences else 'comparable',
            'differences': differences, 'capture_scope': 'whole_client',
            'independently_recomputed': False, 'whole_client_eligible': False,
            **{key: False for key in FLAGS}}


def inspect_client_reference(root, pack_root, generation_ref, state_id, *, context=None):
    """Strict source-side input audit. Missing facts are not an eligible subject."""
    capture = validate_client_capture(root, pack_root, generation_ref, state_id, context=context)
    version = 3 if 'approved_layout' in capture else 2
    return {'schema_version': version, 'kind': 'whole-client-source-render-audit/v' + str(version),
            **({'approved_layout': capture['approved_layout'], 'independent_layout_replay': capture['independent_layout_replay']} if version == 3 else {}),
            'status': 'measured', 'state_id': state_id, 'capture_scope': 'whole_client',
            'surface': capture['surface'], 'facts': capture['facts'],
            'independently_recomputed': False, 'whole_client_eligible': False,
            **{key: False for key in FLAGS}}


def recompute_client_reference(root, pack_root, generation_ref, state_id, output, *, context=None):
    """Fresh source recipe replay, preserving the original pack and failed reruns.

    The independent command is bound to the current source runner and nonce;
    it cannot approve a reference or attest any production/native execution.
    """
    from datetime import datetime, timezone
    from render_evidence import output_path
    root = Path(root).resolve()
    original = validate_client_capture(root, pack_root, generation_ref, state_id, context=context)
    pack = safe_path(root, pack_root); generation = original['generation']
    request_path = safe_path(pack, generation['request']['path'])
    request = load(request_path); request['execution_nonce'] = uuid.uuid4().hex
    out = output_path(root, output); out.mkdir(parents=True)
    destination = out / 'request.json'; destination.write_text(json.dumps(request, ensure_ascii=False, indent=2) + '\n')
    command = ['node', str(root / CLIENT_RUNNER), '--request', str(destination), '--output', str(out)]
    started = datetime.now(timezone.utc).isoformat()
    try:
        completed = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=300)
    except (OSError, subprocess.SubprocessError) as error:
        def captured(value): return value.decode('utf-8', errors='replace') if isinstance(value, bytes) else value or ''
        (out / 'actual-command.json').write_text(json.dumps({'command': command, 'started_at': started,
            'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': None,
            'error': str(error), 'stdout': captured(getattr(error, 'stdout', None)),
            'stderr': captured(getattr(error, 'stderr', None))}, ensure_ascii=False, indent=2) + '\n')
        raise ValueError('independent client replay failed; raw command retained') from error
    (out / 'actual-command.json').write_text(json.dumps({'command': command, 'started_at': started,
        'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': completed.returncode,
        'stdout': completed.stdout, 'stderr': completed.stderr}, ensure_ascii=False, indent=2) + '\n')
    if completed.returncode != 0: raise ValueError('independent client replay failed; original and rerun retained')
    repeated = validate_client_capture(root, out.relative_to(root).as_posix(),
        {'path': 'verification.json', 'sha256': digest(out / 'verification.json')}, state_id, context=context)
    comparison = compare_client_render_facts(original['facts'], repeated['facts'], original['surface'])
    first_png = retained(root, pack, original['row']['client_png'])
    replay_png = retained(root, out, repeated['row']['client_png'])
    if digest(first_png) != digest(replay_png): comparison['differences'].append('actual_original_png')
    comparison.update(status='incomparable' if comparison['differences'] else 'comparable',
                      independently_recomputed=True,
                      original_generation_sha256=generation_ref['sha256'],
                      replay_generation=file_ref(root, out / 'verification.json'))
    (out / 'independent-comparison.json').write_text(json.dumps(comparison, ensure_ascii=False, indent=2) + '\n')
    return comparison


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--diagnostic', action='store_true')
    args = parser.parse_args()
    result = qualify_report(args.root, args.report, args.output, diagnostic=args.diagnostic)
    print(json.dumps(result, ensure_ascii=False))
    return 0  # Collection result; actual qualification and product verdict remain separate.


if __name__ == '__main__':
    raise SystemExit(main())
