"""Inspect finite C35 real-React evidence; preserve every pixel/reference failure.

This adapter grants neither an all-state catalogue nor packaged/native approval.
"""
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

from cp01_component_evidence import load
from gate_core import digest, safe_path

RUNNER = 'scripts/report/verify_preview_visuals.mjs'
VARIANTS = ('narrow-dark-live', 'wide-dark-live', 'boundary519-dark-live',
            'boundary520-dark-live', 'content519-dark-live', 'content520-dark-live',
            'narrow-light-live', 'wide-light-live', 'narrow-dark-paused',
            'narrow-dark-stale', 'narrow-dark-permission', 'narrow-dark-closed',
            'narrow-dark-unavailable', 'narrow-dark-long-title', 'wide-light-long-title',
            'narrow-dark-zoom200', 'narrow-light-long-title-zoom200', 'narrow-dark-reduced')
CASES = tuple('VCP-capture-' + v for v in VARIANTS) + (
    'VCP-details-copy-escape-focus-without-capture-churn',
    'VCP-candidate-session-two-consumers-are-isolated')
DETAILS = tuple(v + '-details' for v in ('narrow-dark-live', 'narrow-light-live',
                                      'narrow-dark-zoom200', 'narrow-light-long-title-zoom200'))


def validate_policy(r, nonce, started, completed):
    errors = []
    exact = {'schemaVersion': 1, 'kind': 'current-preview-visuals/v1',
             'executionNonce': nonce, 'referenceApproval': 'pending_independent_review',
             'productAcceptance': 'blocked', 'packagedNativeVerified': False,
             'allStateAdapterRegistered': False, 'skippedAssertions': 0}
    if not re.fullmatch(r'[0-9a-f]{32}', nonce): errors.append('fresh dispatcher nonce required')
    for k, v in exact.items():
        if type(r.get(k)) is not type(v) or r.get(k) != v: errors.append('typed identity/capability differs: ' + k)
    dates = [datetime.fromisoformat(r[k].replace('Z', '+00:00')) for k in ('startedAt', 'finishedAt')]
    if any(d.tzinfo is None for d in dates) or not started <= dates[0].timestamp() <= dates[1].timestamp() <= completed:
        errors.append('receipt outside actual invocation interval')
    checks = r['checks']
    if tuple(c.get('id') for c in checks) != CASES: errors.append('all exact twenty behavior cases required')
    if any(c.get('status') != 'passed' or type(c.get('assertionCount')) is not int or c['assertionCount'] <= 0 for c in checks):
        errors.append('behavior assertions failed or skipped')
    if type(r.get('assertionCount')) is not int or r['assertionCount'] < sum(c.get('assertionCount', 0) for c in checks):
        errors.append('positive actual assertion total required')
    if tuple(c.get('id') for c in r['comparisons']) != VARIANTS or tuple(c.get('id') for c in r['detailComparisons']) != DETAILS:
        errors.append('all card/details captures required')
    for key in ('sourceDrift', 'pageErrors', 'requestViolations', 'cspViolations'):
        if r.get(key) != []: errors.append('actual runtime/source violation: ' + key)
    if r.get('behaviorStatus') != 'passed': errors.append('behavior status failed')
    # A retained red result is a successful observation, never visual acceptance.
    expected = 'passed' if r.get('behaviorStatus') == 'passed' and r.get('pixelStatus') == 'passed' else 'failed'
    if r.get('status') != expected: errors.append('overall status hides a failed layer')
    if r.get('pixelStatus') != 'passed': errors.append('strict current-source pixel/reference checks remain failed or blocked')
    return errors


def artifact(out, descriptor):
    p = safe_path(out, descriptor['path'])
    if (not p.is_file() or digest(p) != descriptor['sha256']
            or type(descriptor['bytes']) is not int or p.stat().st_size != descriptor['bytes']):
        raise ValueError('raw artifact identity differs: ' + str(p))
    return p


def recompute_pixels(root, pairs):
    """Independent invocation reads original PNGs, fixed pixelmatch options, no masks."""
    js = """const fs=require('fs'),{PNG}=require('pngjs'),pm=require('pixelmatch').default;
const rows=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(rows.map(r=>{
const a=PNG.sync.read(fs.readFileSync(r.a)),b=PNG.sync.read(fs.readFileSync(r.b));
const width=Math.max(a.width,b.width),height=Math.max(a.height,b.height),l=new PNG({width,height}),h=new PNG({width,height}),d=new PNG({width,height});
l.data.fill(0);h.data.fill(0);PNG.bitblt(a,l,0,0,a.width,a.height,0,0);PNG.bitblt(b,h,0,0,b.width,b.height,0,0);
const pixels=pm(l.data,h.data,d.data,width,height,{threshold:.1,includeAA:true});
return {expectedSize:[a.width,a.height],actualSize:[b.width,b.height],pixels,ratio:pixels/(width*height),diff:require('crypto').createHash('sha256').update(PNG.sync.write(d)).digest('hex')};})));"""
    run = subprocess.run([shutil.which('node'), '-e', js], input=json.dumps(pairs),
                         cwd=root, capture_output=True, text=True, timeout=90, check=True)
    return json.loads(run.stdout)


def inspect(root, out, nonce, started, completed):
    errors, details = [], {}
    try:
        root, out = Path(root), Path(out)
        report = safe_path(out, 'verification.json'); r = load(report)
        errors += validate_policy(r, nonce, started, completed)
        command = r['command']
        if (len(command) != 6 or Path(command[0]).resolve() != Path(shutil.which('node')).resolve()
                or Path(command[1]).resolve() != safe_path(root, RUNNER)
                or command[2] != '--output' or Path(root, command[3]).resolve() != out.resolve()
                or command[4:] != ['--nonce', nonce]):
            raise ValueError('actual fixed dispatcher invocation differs')
        expected_paths = {p.relative_to(root).as_posix() for folder in ('apps/desktop/src', 'apps/desktop/tests')
                          for p in (root / folder).rglob('*') if p.is_file()} | {
            RUNNER, 'package.json', 'package-lock.json', 'apps/desktop/package.json',
            'apps/desktop/package-lock.json', 'apps/desktop/tsconfig.json',
            'apps/desktop/vite.config.ts', 'apps/desktop/src-tauri/tauri.conf.json'}
        if set(r['sourceSha256']) != expected_paths: raise ValueError('complete actual source closure required')
        raw_sources = load(artifact(out, r['sourceInputs']))
        if set(raw_sources) != expected_paths: raise ValueError('raw source input closure differs')
        for name, value in r['sourceSha256'].items():
            raw = raw_sources[name]
            if (digest(safe_path(root, name)) != value or raw['sha256'] != value
                    or raw['encoding'] != 'utf8' or hashlib.sha256(raw['content'].encode()).hexdigest() != value):
                errors.append('actual source bytes drift: ' + name)
        approval = r['approvedDesign']; a = load(safe_path(root, approval['approvalFile']))
        if (a['status'] != 'approved_by_user' or approval['sourceCount'] != 61
                or digest(safe_path(root, approval['approvalFile'])) != approval['approvalSha256']
                or a['design_sources'] != approval['sources']): raise ValueError('approved independent source identity differs')
        if any(digest(safe_path(root, p)) != h for p, h in a['design_sources'].items()): raise ValueError('approved source drift')
        frame = r['sharedFrame']; fixture = root / 'docs/design/first-product-v1/acceptance-v4'
        frame_input = load(fixture / 'preview-frame-input.json')
        if ({k: v for k, v in frame.items() if k != 'artifact'} != frame_input
                or digest(artifact(out, frame['artifact'])) != frame_input['sha256']
                or digest(fixture / frame_input['file']) != frame_input['sha256']):
            raise ValueError('shared original image fixture differs')
        for b in r['builds']:
            raw = load(artifact(out, b['log']))
            if b['exitCode'] != 0 or raw['exitCode'] != 0 or raw['command'] != b['command']: raise ValueError('actual build command/log differs')
        node = str(Path(shutil.which('node')).resolve()); desktop = root / 'apps/desktop'
        commands = [[node, str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
                    [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'), '--outDir', str(out / 'controlled-build'), '--sourcemap']]
        actual = [[str(Path(b['command'][0]).resolve()), *b['command'][1:]] for b in r['builds']]
        if actual != commands: raise ValueError('fixed actual production test build required')
        found = set()
        for entry in r['sourceMaps']:
            m = load(safe_path(out, entry['file']))
            if digest(safe_path(out, entry['file'])) != entry['sha256']: raise ValueError('built source map identity differs')
            for source in entry['productionSources']:
                i = source['sourceMapIndex']; name = source['file']
                if (not m['sources'][i].endswith('/' + name.removeprefix('apps/desktop/'))
                        or m['sourcesContent'][i] != safe_path(root, name).read_text()
                        or digest(safe_path(root, name)) != source['sha256']): raise ValueError('built production component differs: ' + name)
                found.add(name)
        if not {'apps/desktop/src/features/capture/WindowPreview.tsx', 'apps/desktop/src/features/capture/PreviewDetails.tsx'} <= found:
            raise ValueError('actual preview graph missing')
        rows, pairs = [], []
        for row in r['comparisons'] + r['detailComparisons']:
            artifact(out, row['candidate']); geometry = load(artifact(out, row['geometry']))
            if 'actualDom' in row: artifact(out, row['actualDom'])
            if row in r['comparisons'] and geometry['actualLayoutIssues'] != []: errors.append('actual layout overlaps: ' + row['id'])
            if row.get('reference') is None:
                if row['id'] != 'narrow-dark-stale' or row['referenceStatus'] != 'blocked_missing_source_state' or row['status'] != 'blocked_missing_source_state':
                    raise ValueError('missing independent reference incorrectly accepted')
                errors.append('missing approved-source state: ' + row['id']); continue
            a_png = artifact(out, row['reference']); b_png = artifact(out, row['candidate']); artifact(out, row['diff'])
            if row['options'] != {'threshold': .1, 'includeAA': True, 'maxRatio': 0}: raise ValueError('strict pixel options changed')
            if row['referenceStatus'] != 'candidate_pending_independent_review': raise ValueError('unreviewed reference promoted')
            rows.append(row); pairs.append({'a': str(a_png), 'b': str(b_png)})
        for row, actual in zip(rows, recompute_pixels(root, pairs)):
            for k in ('expectedSize', 'actualSize', 'pixels', 'ratio'):
                if row[k] != actual[k]: raise ValueError('independent PNG recomputation differs: ' + row['id'] + ':' + k)
            if row['diff']['sha256'] != actual['diff'] or row['recomputedPixels'] != actual['pixels']: raise ValueError('diff bytes/count differ')
            status = 'passed' if actual['pixels'] == 0 and actual['expectedSize'] == actual['actualSize'] else 'failed'
            if row['status'] != status: raise ValueError('pixel failure promoted: ' + row['id'])
            if status != 'passed': errors.append('strict pixel mismatch: ' + row['id'])
        old = r['historicalInventory']; actual_old = load(safe_path(root, old['receipt']))
        if digest(safe_path(root, old['receipt'])) != old['sha256'] or old['total'] != 37 or [(s['id'], s['status']) for s in old['states']] != [(s['id'], s['status']) for s in actual_old['captures']]:
            raise ValueError('historical original obligations/results altered')
        details = {'test_count': len(r['checks']), 'assertion_count': r['assertionCount'],
                   'behavior_status': r['behaviorStatus'], 'visual_status': r['pixelStatus'],
                   'component_receipt': str(report.relative_to(root)), 'component_receipt_sha256': digest(report),
                   'independently_recomputed_comparisons': len(rows),
                   'current_all_state_or_reference_approval': False,
                   'product_packaged_native_game_tests_executed': False}
    except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, subprocess.SubprocessError) as error:
        errors.append('C35 evidence invalid: ' + str(error))
    return errors, details
