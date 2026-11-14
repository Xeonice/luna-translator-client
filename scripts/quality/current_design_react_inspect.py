"""Inspect raw production-component provenance and measured semantic outcomes."""
from datetime import datetime
import hashlib
from pathlib import Path
import shutil

from gate_core import digest, safe_path
from current_reference_catalogue import canonical, load, assess
from current_design_react_adapter import RUNNER, STATE_IDS

COMMON = ('actual_production_csp', 'real_production_target_visible', 'same_controlled_source_unit',
          'same_controlled_translation', 'same_source_preferences')
ASSERTIONS = {
    'all-page-settings-general-normal': COMMON + ('general_segmentation', 'general_ruby', 'general_pos',
        'source_general_control:general.automatic', 'source_general_control:general.history', 'source_general_control:general.launch'),
    'all-page-main-session-normal': COMMON + ('actual_rendered_original_text', 'actual_rendered_translation'),
    'all-page-subpage-readingpanel-dataset-body-retired-surface-absent': COMMON + ('body_disclosure_open',
        'actual_rendered_original_text', 'actual_rendered_translation', 'retired_session_tablist_absent', 'same_source_body_rows'),
    'all-page-subpage-modelconfigscreen-tab-connections-retired-surface-absent': COMMON + ('retired_model_entity_surface_absent',),
}
MAPPING = {
    'all-page-settings-general-normal': ('settings', '#root > div'),
    'all-page-main-session-normal': ('main', 'article[data-guided-current="true"]'),
    'all-page-subpage-readingpanel-dataset-body-retired-surface-absent': ('main', 'details[data-flat-session-section="reading"]'),
    'all-page-subpage-modelconfigscreen-tab-connections-retired-surface-absent': ('settings', '[data-service-workspace]'),
}


def artifact(base, ref):
    if (not isinstance(ref, dict) or set(ref) != {'path', 'sha256', 'bytes'}
            or type(ref['bytes']) is not int or ref['bytes'] <= 0): raise ValueError('typed raw artifact required')
    p = safe_path(base, ref['path'])
    if not p.is_file() or p.stat().st_size != ref['bytes'] or digest(p) != ref['sha256']:
        raise ValueError('raw current React artifact identity differs')
    return p


def _file(root, ref):
    if not isinstance(ref, dict) or set(ref) != {'path', 'sha256'}: raise ValueError('exact report reference required')
    p = safe_path(root, ref['path'])
    if not p.is_file() or digest(p) != ref['sha256']: raise ValueError('current React report hash differs')
    return p


def inspect(root, receipt, context):
    errors = []
    try:
        from current_design_alignment import source_identity
        root = Path(root); rp = _file(root, receipt['report']); out = rp.parent
        if rp.name != 'verification.json' or out.name != 'capture': raise ValueError('fixed raw report location required')
        r = load(rp); execution = load(_file(root, receipt['execution']))
        if r.get('schema_version') != 1 or r.get('kind') != 'current-design-real-react/v1': raise ValueError('real component report schema differs')
        exact = {'layer': 'real_react', 'producer_kind': 'real_react', 'prototype_substitution': False,
                 'controlled_transport': True, 'native_verified': False, 'packaged_native_tests_executed': False,
                 'formal_adapter_receipt': False, 'diagnostic_context': False, 'collection_complete': True}
        for key, value in exact.items():
            if type(r.get(key)) is not type(value) or r[key] != value: errors.append('typed real component capability differs: ' + key)
        for key in ('adapter_id', 'target_id', 'composition_id', 'execution_nonce', 'design_closure_sha256', 'artifact_sha256', 'source_tree_sha256'):
            if r.get(key) != receipt.get(key): raise ValueError('raw invocation identity differs: ' + key)
        if len(r['state_ids']) != len(STATE_IDS) or set(r['state_ids']) != STATE_IDS: raise ValueError('finite state closure differs')
        expected_source = source_identity(root)
        if any(r.get(k) != expected_source for k in ('source_tree_sha256', 'source_tree_before', 'source_tree_after')):
            errors.append('actual whole source epoch differs')
        parent = out.parent
        node = str(Path(shutil.which('node')).resolve(strict=True))
        command = [node, str(root / RUNNER), '--request', str(parent / 'request.json'), '--output', str(parent)]
        if r['command'] != command or execution['command'] != command or execution['cwd'] != str(root): raise ValueError('actual fixed current React command differs')
        if execution['exit_code'] != 0: errors.append('actual production component command failed')
        if (r['workspace_root'] != str(root) or r['original_request'] != str(parent / 'request.json')
                or r['original_output'] != str(parent) or r['capture_root'] != str(out)):
            raise ValueError('raw execution paths differ')
        request = load(artifact(out, r['request']))
        if request != load(parent / 'request.json'): raise ValueError('actual request copy differs')
        for key in ('execution_nonce', 'artifact_sha256', 'source_tree_sha256', 'target_id', 'composition_id', 'design_closure_sha256'):
            if request.get(key) != r[key]: raise ValueError('raw request identity differs')
        dates = [datetime.fromisoformat(v.replace('Z', '+00:00')) for v in
                 (execution['started_at'], r['started_at'], r['finished_at'], execution['finished_at'])]
        if any(d.tzinfo is None for d in dates) or dates != sorted(dates): raise ValueError('actual execution interval differs')
        for key in ('errors', 'page_errors', 'console_errors', 'external_requests', 'request_errors', 'csp_violations', 'source_drift'):
            if r.get(key) != []: errors.append('actual current component violation: ' + key)
        paths = {p.relative_to(root).as_posix() for folder in ('apps/desktop/src', 'apps/desktop/tests')
                 for p in (root / folder).rglob('*') if p.is_file()} | {RUNNER}
        expected = {p: digest(safe_path(root, p)) for p in paths}
        if r['sources_before'] != expected or r['sources_after'] != expected: raise ValueError('complete production/test source closure differs')
        raw_sources = load(out / 'source-inputs.json')
        if set(raw_sources) != paths: raise ValueError('raw production/test source closure differs')
        for name, value in raw_sources.items():
            if value != {'sha256': expected[name], 'content': safe_path(root, name).read_text()}:
                raise ValueError('raw production source bytes differ: ' + name)
        package = safe_path(root, request['artifact_path'])
        package_fact = {'path': request['artifact_path'], 'sha256': digest(package), 'bytes': package.stat().st_size}
        if r['artifact_before'] != package_fact or r['artifact_after'] != package_fact or package_fact['sha256'] != receipt['artifact_sha256']:
            raise ValueError('actual package bytes changed')
        security = root / 'apps/desktop/src-tauri/tauri.conf.json'; csp = load(security)['app']['security']['csp']
        if r['production_csp'] != {'source_file': security.relative_to(root).as_posix(), 'sha256': digest(security),
                                  'value': csp, 'delivery': 'actual document HTTP response header'}:
            raise ValueError('actual production CSP differs')
        desktop = root / 'apps/desktop'
        commands = [[node, str(desktop / 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'],
                    [node, str(desktop / 'node_modules/vite/bin/vite.js'), 'build', '--config', str(desktop / 'tests/visual/vite.config.ts'),
                     '--outDir', str(out / 'controlled-build'), '--sourcemap', '--manifest']]
        if [b['command'] for b in r['builds']] != commands: raise ValueError('actual fixed test/production build differs')
        for b in r['builds']:
            raw = load(artifact(out, b['log']))
            if raw['exit_code'] != 0 or b['exit_code'] != 0 or raw['command'] != b['command'] or raw['cwd'] != str(desktop):
                raise ValueError('actual production build failed or log differs')
        graph = set()
        for m in r['source_maps']:
            parsed = load(artifact(out, {k: m[k] for k in ('path', 'sha256', 'bytes')}))
            for s in m['production_sources']:
                p = safe_path(root, s['source']); i = s['source_map_index']
                if (type(i) is not int or parsed['sourcesContent'][i] != p.read_text() or digest(p) != s['sha256']
                        or not parsed['sources'][i].endswith('/' + s['source'].removeprefix('apps/desktop/'))):
                    raise ValueError('actual built production source map differs')
                graph.add(p.name)
        if not {'SettingsWindow.tsx', 'ReadingPanel.tsx', 'SessionRecordPanel.tsx', 'ModelConfigScreen.tsx', 'WindowRoot.tsx'} <= graph:
            raise ValueError('actual production component graph incomplete')
        approval = assess(root, STATE_IDS)
        if approval['errors'] or approval['gaps']: raise ValueError('current independently reviewed reference closure invalid')
        catalogue = load(root / 'docs/development/current-reference-catalogue.json'); pack = safe_path(root, catalogue['pack_root'])
        refs = {e['state_id']: e for e in catalogue['entries']}
        if (r['references']['catalogue'] != approval['catalogue'] or r['references']['review'] != {k: approval['review'][k] for k in ('path', 'sha256')}):
            raise ValueError('raw approved reference version differs')
        rows = r['results']
        if len(rows) != len(STATE_IDS) or {row['state_id'] for row in rows} != STATE_IDS: raise ValueError('actual finite state results omitted/duplicated')
        total = 0
        for row in rows:
            sid = row['state_id']; names = ASSERTIONS[sid]
            transcript = load(artifact(out, row['assertions'])); inputs = load(artifact(out, row['inputs']))
            dom = load(artifact(out, row['production_dom'])); geometry = load(artifact(out, row['geometry']))
            actions = load(artifact(out, row['actions']))
            if (row['required_assertion_ids'] != list(names) or row['not_run_assertion_ids'] != []
                    or row['assertion_count'] != len(names) or type(row['assertion_count']) is not int
                    or type(row['skipped_assertions']) is not int or row['skipped_assertions'] != 0
                    or len(transcript) != len(names) or {t['name'] for t in transcript} != set(names)):
                raise ValueError('exact actual semantic assertion closure differs: ' + sid)
            if not actions or actions[0].get('action') != 'navigate' or actions[0].get('status') != 200:
                raise ValueError('actual production route/action missing: ' + sid)
            role, selector = MAPPING[sid]
            if dom['selector'] != selector or geometry['selector'] != selector or '?role=' + role not in dom['url']:
                raise ValueError('actual production target differs: ' + sid)
            source_geometry = load(artifact(pack, refs[sid]['geometry'])); source = source_geometry['observations_after']['designState']
            if inputs['source_geometry'] != source_geometry or inputs['source_state'] != source:
                raise ValueError('actual input is not the exact independently reviewed source state')
            current = inputs['current']; actual = inputs['actual_visible']; snapshot = dom['controlled_snapshot']
            if (current['sourceText'] != source['current']['source'] or current['translations'][0]['text'] != source['current']['translation']
                    or snapshot['current']['sourceText'] != current['sourceText']
                    or snapshot['current']['translations'][0]['text'] != current['translations'][0]['text']
                    or snapshot['preferences'] != inputs['preferences']):
                raise ValueError('actual source/translation/preferences input projection differs')
            defaults = {'actual_production_csp': csp, 'real_production_target_visible': True,
                        'same_controlled_source_unit': source['current']['source'], 'same_controlled_translation': source['current']['translation'],
                        'same_source_preferences': inputs['preferences'], 'actual_rendered_original_text': source['current']['source'],
                        'actual_rendered_translation': source['current']['translation'], 'body_disclosure_open': True,
                        'retired_session_tablist_absent': 0, 'retired_model_entity_surface_absent': 0,
                        'same_source_body_rows': [{k: value[k] for k in ('sequence', 'source', 'translation')}
                                                  for value in inputs['visible_source_dom']['body_rows']],
                        'general_segmentation': source['settings']['general']['segmentation'],
                        'general_ruby': source['settings']['general']['furigana'], 'general_pos': source['settings']['general']['pos']}
            source_controls = {c['field']: c['checked'] for c in inputs['visible_source_dom']['general_controls']}
            for observation in transcript:
                name = observation['name']
                expected_value = ({'present': True, 'checked': source_controls[name.split(':', 1)[1]]}
                                  if name.startswith('source_general_control:') else defaults[name])
                if canonical(observation['expected']) != canonical(expected_value): raise ValueError('semantic expected value changed: ' + name)
                if observation['status'] != 'passed' or canonical(observation['actual']) != canonical(expected_value):
                    errors.append(sid + ': measured semantic failure: ' + name)
            if (actual['current_source_from_port'] != current['sourceText'] or actual['current_translation_from_port'] != current['translations'][0]['text']
                    or actual['preferences'] != inputs['preferences']): raise ValueError('raw actual input observations contradict')
            if digest(artifact(out, row['reference_png'])) != refs[sid]['target_png']['sha256']:
                raise ValueError('original independent reference PNG was substituted')
            for field in ('full_png', 'target_png'): artifact(out, row[field])
            from current_design_render_qualification import inspect_qualification
            errors.extend(inspect_qualification(root, rp, row))
            if row['same_data_eligibility'].get('status') != 'eligible' or row['same_data_eligibility'].get('gaps') != []:
                errors.append(sid + ': same-data/surface mapping remains blocked')
            if row.get('semantic_status') != 'passed' or row.get('status') != 'passed' or row.get('errors') != []:
                errors.append(sid + ': actual current state failed')
            total += len(transcript)
        if type(receipt['assertion_count']) is not int or receipt['assertion_count'] != total or receipt['skipped_assertions'] != 0:
            raise ValueError('receipt semantic totals differ from raw complete assertions')
        if r.get('status') != 'passed': errors.append('finite actual current React alignment remains failed')
    except (OSError, ValueError, TypeError, KeyError, AttributeError, IndexError) as error:
        errors.append('current React raw inspection invalid: ' + str(error))
    return errors
