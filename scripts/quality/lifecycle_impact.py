"""Derive lifecycle coverage from real changed paths and close actual execution evidence.

This is an engineering selector/receipt validator, not a proof of arbitrary business
semantics. Case-oracle and contract changes still require independent review.
"""
from __future__ import annotations
import hashlib
import csv
import json
from datetime import datetime, timezone
from pathlib import Path
import re
import secrets
from gate_core import digest, safe_path, source_files

CONTRACT = 'docs/development/lifecycle-impact-contract.json'
OBLIGATIONS = frozenset('SRF-'+x for x in ('PV-IDENTITY','PV-CONTEXT','PV-FRAME-TRUTH','PV-ISOLATION','DISCOVERY','WORDCARD-GEOMETRY','SESSION-DRAIN','READING-VERSION','WRITER-BOUNDARY'))
FINAL_CHANGE = 'verify-refactor-stability-rc'
BASE_CHECKS = frozenset(('source-scope','baseline-integrity','spec-contract-tests','rust-boundary-tests','frontend-boundary-tests','technology-contract-tests','generated-contract-drift','generated-contract-tests','typescript-compile','rust-compile'))

def unique_json(path):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            if key in out: raise ValueError('duplicate JSON key: '+key)
            out[key] = value
        return out
    return json.loads(Path(path).read_text(), object_pairs_hook=unique,
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError('non-JSON '+x)))

def tree_hash(root):
    return hashlib.sha256(json.dumps(source_files(root), sort_keys=True).encode()).hexdigest()

def critical(path, contract):
    return any(path == p.rstrip('/') or path.startswith(p.rstrip('/')+'/') for p in contract['critical_roots'])

def registered_final_cohorts(contract):
    """Read only authority-registered IDs; metadata cannot appoint a final cohort."""
    version = contract.get('schema_version')
    if type(version) is not int or version != 2:
        raise ValueError('invalid lifecycle contract version')
    if contract.get('final_change_id') != FINAL_CHANGE:
        raise ValueError('final cohort selection may not be renamed')
    extra = contract.get('additional_final_change_ids')
    if (not isinstance(extra, list) or not extra
            or any(not isinstance(item, str) or not re.fullmatch(r'[a-z0-9]+(?:-[a-z0-9]+)*', item) for item in extra)):
        raise ValueError('additional final cohort IDs must be a nonempty canonical ID list')
    phase_ids = {row['change_id'] for row in contract.get('phases', {}).values()}
    if len(set(extra)) != len(extra) or FINAL_CHANGE in extra or set(extra).intersection(phase_ids):
        raise ValueError('additional final cohort IDs must be unique and distinct from primary and historical phases')
    return frozenset((contract['final_change_id'], *extra))

def load_contract(root, *, require_pin=True):
    path = safe_path(root, CONTRACT)
    c = unique_json(path)
    keys = {'schema_version','kind','authority','critical_roots','rules','obligations','invariants','phases','historical_selection_sha256','cases','runtime_check','automatic_checks','final_change_id'}
    keys.add('additional_final_change_ids')
    if set(c) != keys or c.get('kind') != 'executable_lifecycle_impact_contract': raise ValueError('invalid lifecycle contract fields/version')
    registered_final_cohorts(c)
    if set(c['obligations']) != OBLIGATIONS or len(c['obligations']) != len(OBLIGATIONS): raise ValueError('nine lifecycle obligations required')
    if set(c['invariants']) != {f'INV-{i:02}' for i in range(1,15)} or any(not v or not set(v) <= OBLIGATIONS for v in c['invariants'].values()): raise ValueError('fourteen invariant links required')
    if c['final_change_id'] != FINAL_CHANGE or c['runtime_check'] != 'product-packaged-stability-tests': raise ValueError('final cohort selection may not be renamed')
    if c['authority']['plan_sha256']!=digest(root/'docs/development/stability-refactor-plan.json') or c['authority']['invariant_contract_sha256']!=digest(root/'docs/architecture/runtime-lifecycle-contracts.json'): raise ValueError('lifecycle plan/invariant identity drift')
    with (root/'docs/architecture/engineering-modules.csv').open(newline='') as stream:
        modules={r['module_id']:r['name'] for r in csv.DictReader(stream)}
    if modules.get('M14')!='native-broker': raise ValueError('M14 native-broker ownership cannot be relabeled as storage')
    if modules.get('M10')!='history-persistence' or modules.get('M16')!='credentials':raise ValueError('canonical persistence/credential ownership differs')
    with (root/'docs/architecture/engineering-components.csv').open(newline='') as stream:
        components={r['component_id'] for r in csv.DictReader(stream)}
    seen = set()
    for row in c['rules']:
        if set(row) != {'path','module_ids','component_ids','obligations'} or row['path'] in seen or not set(row['obligations']) <= OBLIGATIONS or not row['obligations']: raise ValueError('invalid/duplicate lifecycle path rule')
        if not set(row['module_ids'])<=set(modules) or not set(row['component_ids'])<=components:raise ValueError('unregistered canonical module/component in lifecycle rule')
        safe_path(root, row['path']); seen.add(row['path'])
    for row in c['cases']:
        if row['obligation_id'] not in OBLIGATIONS or row['kind'] not in ('rust','component'): raise ValueError('invalid lifecycle case')
        files=[safe_path(root,p) for p in row.get('source_candidates',[row['source']])]
        text='\n'.join(p.read_text() for p in files if p.is_file())
        if row['kind'] == 'rust' and not re.search(r'\bfn\s+'+re.escape(row['function'])+r'\s*\(', text): raise ValueError('required maintained Rust case removed: '+row['function'])
        if row['kind'] == 'component' and row['case_id'] not in text: raise ValueError('required maintained component case removed: '+row['case_id'])
    if require_pin:
        baseline = unique_json(root/'docs/development/iteration-baseline.json')
        pins = [row for row in baseline['files'] if row.get('path') == CONTRACT]
        if len(pins) != 1 or pins[0].get('sha256') != digest(path): raise ValueError('lifecycle contract requires independent exact authority pin')
    return c

def literal_dependencies(root, path):
    """Resolve only explicit relative source edges; the boundary AST guard remains required."""
    p = safe_path(root,path)
    if not p.is_file(): return []
    text = p.read_text(); candidates = []
    if p.suffix == '.rs':
        for m in re.finditer(r'(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;', text):
            if m.group(1): opts = [p.parent/m.group(1)]
            else:
                parent = p.parent if p.name in ('mod.rs','main.rs','lib.rs') else p.parent/p.stem
                opts = [parent/(m.group(2)+'.rs'), parent/m.group(2)/'mod.rs']
            candidates.extend(q for q in opts if q.is_file())
    elif p.suffix in ('.ts','.tsx','.mjs'):
        for m in re.finditer(r'(?:from\s*|import\s*)[\'\"](\.[^\'\"]+)[\'\"]', text):
            q = p.parent/m.group(1)
            candidates.extend(x for x in [q,*[Path(str(q)+s) for s in ('.ts','.tsx','.mjs')],q/'index.ts',q/'index.tsx'] if x.is_file())
    return sorted({q.resolve().relative_to(root.resolve()).as_posix() for q in candidates})

def current_visual_selection(root, paths, *, full=False):
    """Select real current-target execution, never a preflight or legacy image.

    Missing capability is an error, not an empty successful selection. Historical
    phase receipts keep their own identity; they cannot grant current coverage.
    """
    import current_design_alignment as design
    from design_authority import current_source_approval
    authority = current_source_approval(root)
    errors = list(authority['errors']) + design.validate_contract(root)
    if authority.get('status') != 'approved_by_user':
        errors.append('Current lifecycle design source authority is not approved')
    states, _native = design.required_states(None if full else paths, root=root)
    layers = sorted({layer for row in design.STATES if row['id'] in states
                     for layer in row['required_layers']})
    checks, gaps = design.registered_execution_checks(states, layers)
    errors.extend(gaps)
    if not states:
        errors.append('Lifecycle production impact has no current design state mapping')
    return {'policy': 'current_design_actual_execution',
            'target_id': design.TARGET, 'composition_id': design.COMPOSITION_ID,
            'design_closure_sha256': design.DESIGN_CLOSURE_SHA256,
            'authority': authority['record'], 'full_current_closure': full,
            'required_states': sorted(states), 'required_layers': layers,
            'required_checks': sorted(checks), 'errors': list(dict.fromkeys(errors))}


def package_digest(path):
    # A final ZIP can exceed 1GB. Preserve SHA256 without whole-file allocation.
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def inspect_current_visual_execution(root, selection, steps, *, execution_context=None,
                                     expected_artifact_sha256=None):
    """Reinspect dispatch identity and raw artifacts, rather than a green step."""
    import current_design_alignment as design
    errors, covered, package_hashes = [], {layer: set() for layer in selection['required_layers']}, set()
    for check in selection['required_checks']:
        try:
            step = steps.get(check, {})
            execution = step['execution']
            if step.get('status') != 'passed' or execution.get('status') != 'passed' or execution.get('errors') != []:
                raise ValueError('actual current adapter execution failed or absent')
            if execution.get('adapter_id') != check:
                raise ValueError('actual dispatcher adapter differs from selected check')
            invocation_path = safe_path(root, execution['invocation'])
            if digest(invocation_path) != execution['invocation_sha256']:
                raise ValueError('actual dispatcher invocation hash mismatch')
            invocation = unique_json(invocation_path)
            if invocation.get('schema_version') != 1 or type(invocation.get('schema_version')) is not int or invocation.get('adapter_id') != check:
                raise ValueError('actual invocation schema/adapter identity differs')
            if execution_context is not None:
                nonce = execution_context['execution_nonce']
                if (not isinstance(nonce, str) or not re.fullmatch('[0-9a-f]{32}', nonce)
                        or invocation.get('lifecycle_execution_nonce') != nonce
                        or invocation.get('lifecycle_started_at') != execution_context['started_at']):
                    raise ValueError('current dispatch does not belong to this fresh lifecycle invocation')
                lower = datetime.fromisoformat(execution_context['started_at'])
                started = datetime.fromisoformat(invocation['started_at'])
                finished = datetime.fromisoformat(invocation['finished_at'])
                upper = datetime.now(timezone.utc)
                if lower.tzinfo is None or started.tzinfo is None or finished.tzinfo is None or not lower <= started <= finished <= upper:
                    raise ValueError('current dispatch is outside this actual lifecycle execution interval')
            adapter = design.ADAPTERS.get(check)
            capability_errors = design.adapter_capability_errors(check)
            if capability_errors:
                raise ValueError('; '.join(capability_errors))
            expected = {'target_id': selection['target_id'], 'composition_id': selection['composition_id'],
                        'design_closure_sha256': selection['design_closure_sha256'],
                        'source_tree_sha256': tree_hash(root), 'layer': adapter.layer,
                        'state_ids': sorted(adapter.states), 'command': list(adapter.command)}
            if any(invocation.get(key) != value for key, value in expected.items()):
                raise ValueError('current dispatch target/source/layer/states/command differs')
            if invocation.get('execution_nonce') != execution.get('execution_nonce'):
                raise ValueError('current dispatch nonce differs')
            if invocation_path != safe_path(root, invocation['output']) / 'invocation.json':
                raise ValueError('current dispatch output identity differs')
            package = safe_path(root, invocation['artifact_path'])
            package_hash = package_digest(package)
            if not package.is_file() or not package.stat().st_size or package_hash != invocation['artifact_sha256']:
                raise ValueError('actual current package identity differs')
            if expected_artifact_sha256 is not None and package_hash != expected_artifact_sha256:
                raise ValueError('current visual and actual four-game runtime use different packages')
            receipt_path = safe_path(root, execution['receipt'])
            if receipt_path != invocation_path.parent / 'receipt.json' or digest(receipt_path) != execution['receipt_sha256']:
                raise ValueError('actual current receipt path/hash differs')
            receipt = unique_json(receipt_path)
            if receipt.get('adapter_id') != check:
                raise ValueError('receipt adapter differs from selected current check')
            layer, states = design._inspect_receipt(root, receipt, package_hash, invocation)
            if package_digest(package) != package_hash or tree_hash(root) != invocation['source_tree_sha256']:
                raise ValueError('current source/package changed during lifecycle inspection')
            package_hashes.add(package_hash)
            covered[layer].update(states)
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            errors.append(check + ': ' + str(error))
    if len(package_hashes) > 1:
        errors.append('Current lifecycle layers were executed against different packages')
    for layer, states in covered.items():
        missing = set(selection['required_states']) - states
        if missing:
            errors.append('Current lifecycle actual ' + layer + ' state coverage missing: ' + ','.join(sorted(missing)))
    return errors


def plan_impact(root, metadata, changed_paths, profile='development', *, contract=None):
    execution_context = {'execution_nonce': secrets.token_hex(16),
                         'started_at': datetime.now(timezone.utc).isoformat()}
    c = contract or load_contract(root); errors=[]; current=source_files(root)
    rules={r['path']:r for r in c['rules']}; paths=sorted(set(changed_paths)); affected=[]
    for path in paths:
        if path in rules: affected.append(rules[path])
        elif critical(path,c): errors.append('Unregistered critical changed/deleted path: '+path)
        if path in rules:
            for dep in literal_dependencies(root,path):
                if critical(dep,c) and dep not in rules: errors.append('Unregistered critical literal dependency: '+path+' -> '+dep)
    registered = registered_final_cohorts(c)
    final = metadata['id'] in registered or profile=='release'
    phase = next((v for v in c['phases'].values() if v['change_id']==metadata['id']), None)
    declared={x for r in metadata['requirements'] for x in r['checks']} | set(metadata.get('product_visual_checks',[])) | set(metadata.get('product_runtime_checks',[]))
    if c['runtime_check'] in declared and metadata['id'] not in registered and profile != 'release':
        errors.append('Unregistered final runtime cohort: '+metadata['id'])
    obligations=set(c['obligations']) if final else {o for r in affected for o in r['obligations']}
    required=set(BASE_CHECKS) if obligations else set()
    historical_phase = phase if phase and not final else None
    visual = None
    if historical_phase:
        required.update(historical_phase['checks'])
        visual = {'policy': 'historical_phase_selection', 'required_checks': historical_phase['visual']}
    else:
        for case in c['cases']:
            if case['obligation_id'] in obligations: required.add(case['check_id'])
        if obligations:
            visual = current_visual_selection(root, paths, full=final)
            required.update(visual['required_checks'])
            errors.extend(visual['errors'])
    if final:
        # SR-6's nonvisual/native/runtime obligations remain complete. Its old
        # visual target cannot be reintroduced as a second mandatory dependency.
        required.update(set(c['phases']['SR-6']['checks']) - set(c['phases']['SR-6']['visual']))
        required.add(c['runtime_check'])
    mods={m for r in metadata['requirements'] for m in r.get('module_ids',[])}
    for row in affected:
        if row['module_ids'] and not mods.intersection(row['module_ids']): errors.append('Lifecycle path/module ownership mismatch: '+row['path'])
        owners=[r for r in metadata['requirements'] if not row['module_ids'] or set(r.get('module_ids',[])).intersection(row['module_ids'])]
        if row['component_ids'] and not any(set(r.get('component_ids',[])).intersection(row['component_ids']) for r in owners): errors.append('Lifecycle path/component ownership mismatch: '+row['path'])
    historical=c['historical_selection_sha256'].get(metadata['id'])
    if historical:
        old=root/'openspec/changes'/metadata['id']/'verification.json'
        if not old.is_file() or digest(old)!=historical: errors.append('Frozen historical phase metadata changed; use a new integration change')
    from binding_journey import required_checks as binding_required_checks
    required.update(binding_required_checks(metadata['id'], paths, profile))
    missing=required-declared
    if missing: errors.append('Lifecycle required checks missing: '+', '.join(sorted(missing)))
    # Exact historical metadata keeps its original phase selection, not a broad exemption.
    # A new final change still selects all obligations and current executions.
    cases=[r for r in c['cases'] if r['obligation_id'] in obligations and r['check_id'] in required]
    source=tree_hash(root)
    return {'schema_version':1,'kind':'actual_diff_lifecycle_impact','change_id':metadata['id'],'final_cohort':final,
            'contract_sha256':digest(root/CONTRACT),'source_tree_sha256':source,
            'changed_paths':paths,'changed_hashes':{p:current.get(p) for p in paths},
            'obligations':sorted(obligations),'invariant_links':{k:v for k,v in c['invariants'].items() if set(v).intersection(obligations)},
            'required_checks':sorted(required),'required_automatic_checks':c['automatic_checks'] if obligations else [],
            'cases':cases,'visual_execution':visual,'execution_context':execution_context,
            'errors':errors,'status':'failed' if errors else 'selected_not_executed'}

def cargo_case_rows(text):
    """Inspect ordered actual Cargo target sections; summary counts alone cannot pass."""
    target=None; rows={}; summaries=[]
    for line in text.splitlines():
        m=re.search(r'\bRunning (unittests .+?|tests/[^ ]+\.rs) \(([^)]+)\)',line)
        if m: target=m.group(1)
        m=re.fullmatch(r'test ([\w:]+) \.\.\. (ok|FAILED|ignored.*)',line)
        if m:
            key=(target,m.group(1))
            if key in rows: raise ValueError('duplicate actual Cargo case: '+str(key))
            rows[key]=m.group(2)
        if line.startswith('test result:'): summaries.append(line)
    from product_execution import validate_cargo_output
    _,errors=validate_cargo_output('\n'.join(summaries))
    if errors: raise ValueError('; '.join(errors))
    return rows

def inspect_final_runtime(root, plan, index):
    """Read the real runtime collection again and return its actual package hash."""
    try:
        from runtime_stability_evidence import validate_run
        step = index.get('product-packaged-stability-tests', {})
        ex = step['execution']
        if (step.get('status') != 'passed' or ex.get('status') != 'passed'
                or ex.get('exit_code') != 0 or ex.get('errors') != []
                or ex.get('adapter_id') != 'product-packaged-stability-tests'
                or ex.get('source_tree_sha256') != plan['source_tree_sha256']):
            raise ValueError('actual final runtime dispatcher failed or differs')
        nonce = ex['execution_nonce']
        if not isinstance(nonce, str) or not re.fullmatch('[0-9a-f]{32}', nonce):
            raise ValueError('fresh actual final runtime nonce required')
        envelope_path = safe_path(root, ex['receipt'])
        if digest(envelope_path) != ex['receipt_sha256']:
            raise ValueError('final runtime execution envelope hash mismatch')
        if unique_json(envelope_path) != {k: v for k, v in ex.items() if k not in ('receipt', 'receipt_sha256')}:
            raise ValueError('final runtime execution differs from actual dispatcher envelope')
        report_path = safe_path(root, ex['runtime_receipt'])
        if digest(report_path) != ex['runtime_receipt_sha256']:
            raise ValueError('final runtime raw report hash mismatch')
        report = unique_json(report_path)
        collection = unique_json(report_path.parent / 'collection.json')
        lower = datetime.fromisoformat(plan['execution_context']['started_at'])
        if lower.tzinfo is None or not lower.timestamp()*1000 <= collection['started_at_ms'] <= report['executed_at_ms'] <= datetime.now(timezone.utc).timestamp()*1000:
            raise ValueError('final runtime collection is outside this actual lifecycle invocation')
        if report.get('execution_nonce') != nonce:
            raise ValueError('final runtime report nonce differs from dispatcher')
        recomputed = validate_run(root, report_path.parent, nonce, plan['source_tree_sha256'])
        for key in ('status','errors','cases','source_tree_sha256','artifact_sha256','main_pid',
                    'fixture_substitution','product_packaged_native_game_tests_executed'):
            if report.get(key) != recomputed.get(key):
                raise ValueError('independent final runtime recomputation differs: ' + key)
        if (recomputed.get('status') != 'passed' or recomputed.get('errors') != []
                or recomputed.get('product_packaged_native_game_tests_executed') is not True
                or recomputed.get('fixture_substitution') is not False):
            raise ValueError('final runtime lacks actual packaged four-game execution')
        package_hash = recomputed.get('artifact_sha256')
        if not isinstance(package_hash, str) or not re.fullmatch('[0-9a-f]{64}', package_hash) or ex.get('artifact_sha256') != package_hash:
            raise ValueError('final runtime actual package identity differs')
        return package_hash, []
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        return None, ['Cannot independently close actual final runtime: ' + str(error)]


def close_execution(root, plan, steps):
    errors=[]
    if plan['errors']: errors.append('Lifecycle selection itself did not pass')
    if tree_hash(root)!=plan['source_tree_sha256']: errors.append('Lifecycle source cohort changed after selection')
    index={}
    for step in steps:
        if step['id'] in index: errors.append('Duplicate gate step: '+step['id'])
        index[step['id']]=step
    for check in plan['required_checks']+plan['required_automatic_checks']:
        if index.get(check,{}).get('status')!='passed': errors.append('Required lifecycle execution missing or failed: '+check)
    runtime_hash = None
    if plan['final_cohort']:
        runtime_hash, runtime_errors = inspect_final_runtime(root, plan, index)
        errors.extend(runtime_errors)
    visual = plan.get('visual_execution')
    if visual and visual.get('policy') == 'current_design_actual_execution':
        try:
            current = current_visual_selection(root, plan['changed_paths'], full=plan['final_cohort'])
            if current != visual or current['errors']:
                errors.append('Current lifecycle visual authority/capability selection is incomplete or changed')
            context = plan['execution_context']
            if (not isinstance(context, dict) or set(context) != {'execution_nonce','started_at'}
                    or not isinstance(context['execution_nonce'], str)
                    or not re.fullmatch('[0-9a-f]{32}', context['execution_nonce'])):
                raise ValueError('fresh root lifecycle invocation context missing or invalid')
            errors.extend(inspect_current_visual_execution(root, current, index,
                          execution_context=context, expected_artifact_sha256=runtime_hash))
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            errors.append('Cannot independently close current lifecycle visual execution: ' + str(error))
    elif visual and visual.get('policy') == 'historical_phase_selection':
        try:
            contract = load_contract(root)
            phase = next((row for row in contract['phases'].values() if row['change_id'] == plan['change_id']), None)
            if (plan['final_cohort'] or phase is None
                    or visual['required_checks'] != phase['visual']):
                errors.append('Historical visual selection cannot replace a current/final cohort')
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append('Historical visual selection authority invalid: ' + str(error))
    elif plan.get('obligations'):
        errors.append('Lifecycle visual execution selection is missing or invalid')
    inspected={}
    for case in plan['cases']:
        check=case['check_id'];step=index.get(check,{})
        try:
            if check not in inspected:
                ex=step['execution']
                if ex.get('status')!='passed' or ex.get('exit_code')!=0: raise ValueError('actual adapter failed')
                source=ex.get('source_tree_sha256',ex.get('source_tree_sha256_before'))
                if source!=plan['source_tree_sha256']: raise ValueError('old/mixed source cohort')
                nonce=ex.get('execution_nonce',ex.get('nonce'))
                if not isinstance(nonce,str) or not re.fullmatch('[0-9a-f]{32}',nonce): raise ValueError('missing launcher nonce')
                raw=safe_path(root,ex['log'])
                if ex.get('log_sha256') and digest(raw)!=ex['log_sha256']: raise ValueError('raw log hash mismatch')
                if case['kind']=='rust':
                    from product_execution import PRODUCT_TEST_CHECKS
                    if ex['command']!=list(PRODUCT_TEST_CHECKS[check]): raise ValueError('command differs from code registry')
                    inspected[check]=cargo_case_rows(raw.read_text())
                else:
                    from reading_components import inspect_current_session_fixtures, validate_report, component_sources
                    report_path=safe_path(root,ex['component_receipt'])
                    if digest(report_path)!=ex['component_receipt_sha256']: raise ValueError('component report hash mismatch')
                    report=unique_json(report_path)
                    # Re-run the genuine existing validator, including source and fixture relations.
                    es=validate_report(report_path.parent,report,nonce,component_sources(root),0,ex['current_session_fixtures'])
                    if es: raise ValueError('; '.join(es))
                    inspected[check]={r['id']:r for r in report['checks']}
            records=inspected[check]
            if case['kind']=='rust':
                if records.get((case['target'],case['test_name']))!='ok': raise ValueError('exact package/target/qualified case missing or not ok: '+case['test_name'])
            else:
                row=records.get(case['case_id'],{})
                if row.get('status')!='passed' or type(row.get('assertionCount')) is not int or row['assertionCount']<=0: raise ValueError('actual component assertions missing: '+case['case_id'])
        except (OSError,ValueError,KeyError,TypeError) as e: errors.append(check+': '+str(e))
    if plan['final_cohort']:
        runtime=index.get('product-packaged-stability-tests',{}).get('execution',{})
        if runtime.get('product_packaged_native_game_tests_executed') is not True or runtime.get('fixture_substitution') is not False: errors.append('Final lifecycle cohort lacks actual packaged four-game execution')
    return list(dict.fromkeys(errors))
