#!/usr/bin/env python3
"""Additional current-design acceptance closure; never rewrites the frozen 153 plan.

Checking this inventory is not product execution. Only code-owned adapters with
explicit capabilities and inspected per-observation receipts may satisfy release.
"""
from __future__ import annotations
import argparse
import csv
from datetime import datetime
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
from typing import Any
from gate_core import digest, source_files
from design_authority import current_source_approval as source_approval, current_user_approval_gaps as user_approval_gaps
from design_authority import TARGET as DESIGN_TARGET, CURRENT_COMPOSITION_ID

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = 'docs/development/current-product-test-coverage.json'
CURRENT_SCOPE = 'macos-arm64-crossover'
CATALOG = 'docs/architecture/model-service-registry-design.json'
PRESENTATION = 'docs/architecture/service-configuration-presentation.json'
GUI_SOURCE = 'scripts/report/verify_guided_service_reading.cjs'
DESIGN = 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html'
GROUPS = {'acceptance': ('PA', 12), 'offline_acceptance': ('OT', 16),
          'selection_acceptance': ('SS', 12), 'flat_acceptance': ('FL', 10),
          'guided_acceptance': ('GD', 12)}
LEGACY_FILES = ('docs/development/first-product-test-plan.json',
                'docs/development/first-product-test-executions.csv',
                'docs/development/first-product-test-coverage.csv',
                'docs/development/client-regression.csv')
# Reviewed layer normalization preserves the original layer vocabulary separately.
LAYER_MAP = {
 'design':'design', 'document_and_catalog':'catalog', 'catalog_contract':'catalog',
 'rust_catalog':'catalog', 'rust_domain':'domain', 'configuration_projection':'domain',
 'purpose_authorization':'domain', 'rust_configuration':'domain', 'rust_runtime':'domain',
 'rust_scope':'domain', 'rust_race':'domain', 'rust_multi_engine':'domain',
 'rust_purpose':'domain', 'rust_session':'domain', 'rust_reading':'domain',
 'rust_binding_runtime':'domain', 'real_component':'component',
 'design_and_product_ui':'component', 'rust_contract_and_ui':'domain',
 'runtime_and_ui':'domain', 'real_storage':'storage', 'runtime_and_storage':'storage',
 'identity_cache_and_storage':'storage', 'storage_migration':'storage',
 'import_export_and_storage':'storage', 'history_cache':'storage',
 'real_pipeline':'pipeline', 'real_http':'service', 'real_service':'service',
 'actual_service':'service', 'actual_transport_cancellation':'service',
 'actual_study':'service', 'vault_and_actual_failure':'credentials',
 'actual_credentials':'credentials', 'actual_vault_oauth':'credentials',
 'native_multiwindow':'native', 'real_platform':'native', 'native_preview':'native',
 'native_accessibility':'native', 'apple_native':'native',
 'resource_worker':'worker', 'resource_and_runtime':'worker',
 'resource_and_routing':'worker', 'windows_and_crossover_worker':'worker',
 'private_protocol_and_process':'worker', 'real_offline_runtime':'worker',
 'runtime_ui_and_history':'domain', 'final_package':'native',
 'signed_package_and_real_quality':'game', 'independent_review':'review',
 'independent_visual_review':'review',
}
# Composite source names require all constituent responsibilities, not one alias.
LAYER_MAP.update({
 'design_and_product_ui':('design','component'),
 'rust_contract_and_ui':('domain','component'),
 'runtime_and_ui':('domain','component'),
 'resource_and_runtime':('worker','domain'),
 'resource_and_routing':('worker','domain'),
 'private_protocol_and_process':('protocol','worker'),
 'runtime_and_storage':('domain','storage'),
 'identity_cache_and_storage':('domain','storage'),
 'import_export_and_storage':('component','storage'),
 'runtime_ui_and_history':('domain','component','storage'),
 'signed_package_and_real_quality':('game','worker','review'),
 'vault_and_actual_failure':('credentials','service'),
})
PA_LAYERS = {
 'PA-01':['catalog','domain'], 'PA-02':['domain','protocol','credentials'],
 'PA-03':['domain','protocol'], 'PA-04':['protocol','service'],
 'PA-05':['protocol','service'], 'PA-06':['domain','service','storage'],
 'PA-07':['protocol','credentials'], 'PA-08':['storage'],
 'PA-09':['domain','storage'], 'PA-10':['component','domain'],
 'PA-11':['service'], 'PA-12':['game'],
}
# Each GUI ID corresponds to a literal regression() in the existing design runner.
GUI = {
 'GUI01': ('当前服务连接的代理可达且独立保存', ['AI-CFG-01','AI-CFG-11'], [], ['domain','component','storage','protocol'], ['M04','M06','M13'], ['C26','C31'], 'AI', '代理仅改当前连接；保存重开仍保留，其他连接和当前绑定/自动开关不变。'),
 'GUI02': ('学习用途各自保留温度三态、模板覆盖与预览', ['AI-CFG-03','AI-CFG-07','AI-CFG-08','AI-CFG-09','AI-CFG-16'], [], ['domain','component','service'], ['M04','M09','M13'], ['C27','C28','C29'], 'AI', 'grammar/context_word逐用途验证继承/设置/省略温度、模板覆盖与预览身份，互不改写且不外发未授权正文。'),
 'GUI03': ('资源草稿隔离用途策略并只在明确选用后应用', ['AI-CFG-03','AI-CFG-04','AI-CFG-10','AI-CFG-13'], [], ['domain','component','storage','service'], ['M04','M05','M13'], ['C26','C27','C31'], 'AP', '新增/保存未绑定资源不改任何用途；预览区分草稿/已应用，明确选用后仅后续句冻结新版本。'),
 'GUI04': ('主阅读复制与重译使用现有记录处理流程', ['AI-CFG-13'], ['RW-17','RW-31','RW-32'], ['component','domain','storage','game'], ['M03','M05','M10','M13'], ['C17','C18','C23'], 'UI', '复制准确当前原文；独立重译新建记录并保留旧记录/来源，不改服务和自动开关；新句到达不偷换目标。'),
 'GUI05': ('失效测试退出运行态且资源任务独立结算', ['AI-CFG-04','AI-CFG-05','AI-CFG-12','AI-CFG-13'], [], ['domain','component','service'], ['M04','M05','M13'], ['C29','C31'], 'AI', '运行中修改令精确任务stale并可重新测试；A/B完成与取消独立，迟到不覆盖新版本或其他资源。'),
 'GUI06': ('凭据组观察与显示名不得洗掉已知认证失败', ['AI-CFG-05','AI-CFG-11','AI-CFG-15'], [], ['domain','component','credentials','service'], ['M04','M06','M16','M13'], ['C29','C30'], 'AI', '池观察/显示名变更不改请求凭据身份或原失败fingerprint；已知认证失败继续禁选且保留原因。'),
 'GUI07': ('传统翻译用途预览只展示适用参数', ['AI-CFG-01','AI-CFG-06','AI-CFG-07','AI-CFG-08'], [], ['catalog','component','protocol','service'], ['M04','M06','M13'], ['C27','C28','C29'], 'AI', '真实传统请求和预览均不带LLM prompt/model/temperature/history，保留适用语言与运行策略。'),
 'GUI08': ('明确选用大模型后上下文范围可编辑且预览与保存隔离', ['AI-CFG-03','AI-CFG-04','AI-CFG-09','AI-CFG-13'], [], ['domain','component','storage','service'], ['M04','M05','M13'], ['C27','C28','C29'], 'AI', '已选LLM翻译用途可编辑0/正上限与原文/译文/both范围；保存不应用，不串学习；明确重选才冻结策略。'),
}
GUI_FP = {'GUI01':['FP-06','FP-11'],'GUI02':['FP-06','FP-08','FP-11'],'GUI03':['FP-06','FP-11'],'GUI04':['FP-04','FP-06','FP-09'],'GUI05':['FP-06','FP-11','FP-12'],'GUI06':['FP-06','FP-11','FP-12'],'GUI07':['FP-06','FP-11'],'GUI08':['FP-06','FP-09','FP-11']}
LAYERS = {'design','catalog','domain','component','storage','protocol','pipeline',
          'service','credentials','worker','native','game','review'}
SOURCE_LAYERS = {'catalog','domain','component','storage','protocol','pipeline','credentials'}
EVIDENCE = {
 'design':['candidate_source_identity','action_and_observable_result','capture_and_limitations'],
 'catalog':['exact_catalog_tuple','all_referenced_objects','actual_assertions'],
 'domain':['production_rust_rule','action_and_observable_result','identity_and_revision'],
 'component':['real_product_component','action_and_observable_result','capture_and_limitations'],
 'storage':['real_sqlite_or_repository','persisted_sequence','restart_or_failure_recovery'],
 'protocol':['actual_socket_request_bytes_redacted','protocol_termination','identity_and_revision'],
 'pipeline':['actual_callback_identity','persisted_sequence','pagination_reconstruction'],
 'service':['actual_provider_request_redacted','actual_result_and_recovery','purpose_and_revision'],
 'credentials':['actual_vault_identity','masked_export_log_observations','revocation_or_failure'],
 'worker':['actual_worker_and_resource_identity','offline_egress_observation','actual_result_and_recovery'],
 'native':['os_action_and_result','native_window_identity','capture_and_limitations'],
 'game':['game_client_pairs','actual_callback_identity','distinct_states_and_pages','language_branch'],
 'review':['independent_reviewer','explicit_scope_and_decision','capture_and_limitations'],
}
# Deliberately empty. Existing design commands are references, not new runtime
# coverage. Adding an adapter requires reviewed code, dispatch and inspection.
# JSON/verification metadata cannot install a command or grant capabilities.
ADAPTERS: dict[str, dict[str, Any]] = {}
SHA = re.compile(r'[0-9a-f]{64}\Z')
NONCE = re.compile(r'[0-9a-f]{32}\Z')


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise ValueError('duplicate JSON key: '+key)
        result[key] = value
    return result


def _read(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=_unique,
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError('non-JSON value: '+x)))


def _path(root, value):
    if not isinstance(value,str) or not value or '\\' in value: raise ValueError('relative POSIX path required')
    rel = PurePosixPath(value)
    if rel.is_absolute() or '..' in rel.parts or rel.as_posix()!=value: raise ValueError('path escapes repository')
    path = Path(root)
    for part in rel.parts:
        path = path/part
        if path.is_symlink(): raise ValueError('symbolic-link evidence/source forbidden')
    path.resolve().relative_to(Path(root).resolve())
    return path


def _strings(value):
    return isinstance(value,list) and bool(value) and all(isinstance(x,str) and x.strip()==x and x for x in value) and len(value)==len(set(value))


def _hash_object(value):
    return hashlib.sha256(json.dumps(value,sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()



def _same(actual, expected):
    """JSON identity must distinguish True/1 and False/0 recursively."""
    try:return _hash_object(actual)==_hash_object(expected)
    except (TypeError,ValueError):return False


def source_identity(root):
    return hashlib.sha256(json.dumps(source_files(Path(root)),sort_keys=True).encode()).hexdigest()


def source_rows(root):
    catalog = _read(_path(root,CATALOG)); result = {}
    for group,(prefix,count) in GROUPS.items():
        rows=catalog[group]
        if len(rows)!=count or {r['id'] for r in rows}!={f'{prefix}-{n:02}' for n in range(1,count+1)}: raise ValueError('source acceptance closure changed: '+group)
        for row in rows:
            if row.get('status')!='not_run': raise ValueError('source plan status changed: '+row['id'])
            result[row['id']]={'source_group':group,'source_record':row}
    gui=_path(root,GUI_SOURCE).read_text()
    calls=re.findall(r"await regression\('(GUI\d{2})','([^']+)'",gui)
    if len(calls)!=8 or dict(calls)!={k:v[0] for k,v in GUI.items()}: raise ValueError('GUI01..08 must reference the eight real design regression calls')
    for case_id,details in GUI.items():
        result[case_id]={'source_group':'gui_regressions','source_record':{'id':case_id,'label':details[0],'assertion':details[-1],'status':'not_run','source_requirement_ids':GUI_FP[case_id],'source_configuration_requirement_ids':details[1],'source_reading_requirement_ids':details[2]}}
    return result


def required_layers(case_id, source):
    if case_id in GUI: values=set(GUI[case_id][3])|{'design'}
    elif case_id in PA_LAYERS: values=set(PA_LAYERS[case_id])
    else:
        raw=source.get('verification_layers',[source.get('verification_layer')])
        if any(x not in LAYER_MAP for x in raw): raise ValueError('unreviewed source layer: '+case_id)
        values=set()
        for name in raw:
            mapped=LAYER_MAP[name]
            values.update([mapped] if isinstance(mapped,str) else mapped)
    # Observations below require concrete evidence beyond a generic final-package tag.
    additions={'GD-03':{'pipeline','storage','native'},'GD-04':{'catalog'},
               'GD-07':{'credentials'},'GD-09':{'storage'},'GD-10':{'worker'},
               'GD-12':{'game'},'OT-11':{'worker'},'OT-16':{'worker','game'},
               'OT-13':{'storage'},'FL-10':{'native'},'SS-05':{'worker'}}
    return sorted(values|additions.get(case_id,set()))



def case_mapping(case_id, source, layers):
    """Code-reviewed responsibility and prerequisite policy, not runtime behavior."""
    if case_id in GUI:
        details=GUI[case_id]; modules=set(details[4]); components=set(details[5]); owners={details[6],'QA'}
    else:
        reading=bool(source.get('source_reading_requirement_ids'))
        modules={'M03','M10','M13','M15'} if reading else {'M04','M05','M13'}
        components={'C16','C17','C18','C36'} if reading else {'C26','C27','C29','C31'}
        owners={'AP','UI','QA'}
    if source.get('source_configuration_requirement_ids') or 'FP-06' in source.get('source_requirement_ids',[]):
        modules.update({'M04','M05'});components.update({'C26','C27','C29'})
    if 'FP-08' in source.get('source_requirement_ids',[]):
        modules.add('M09');components.add('C22')
    prerequisites={'PRE-01','PRE-13'}
    if 'credentials' in layers:
        modules.add('M16');components.add('C30');owners.add('AI');prerequisites.add('PRE-14')
    if 'protocol' in layers or 'service' in layers:
        modules.add('M06');components.add('C29');owners.add('AI')
    if 'service' in layers: prerequisites.update({'PRE-06','PRE-11','CURRENT-SERVICE-MATERIALS'})
    if 'storage' in layers: modules.add('M10');prerequisites.add('PRE-14')
    if 'pipeline' in layers: modules.update({'M02','M14'});components.add('C33');owners.add('HK')
    if 'worker' in layers: modules.update({'M06','M12','M15'});components.update({'C32','C33'});owners.add('PL');prerequisites.update({'CURRENT-OFFLINE-LICENSE','PRE-11'})
    if 'native' in layers or 'game' in layers: modules.add('M12');components.update({'C34','C35'});owners.add('PL');prerequisites.update({'PRE-08','PRE-10'})
    if 'game' in layers: prerequisites.update({'PRE-07','PRE-12','PRE-17'})
    if 'review' in layers: prerequisites.add('CURRENT-GUI-REVIEW')
    if case_id in {'GUI06','GD-07'}:prerequisites.add('PRE-18')
    module_owners={'M02':'AP','M03':'AP','M04':'AP','M05':'AI','M06':'AI','M09':'AI','M10':'AP','M12':'PL','M13':'UI','M14':'HK','M15':'QA','M16':'PL'}
    owners.update(module_owners[module] for module in modules)
    return {'module_ids':sorted(modules),'component_ids':sorted(components),'owners':sorted(owners),'prerequisite_ids':sorted(prerequisites)}


def _variants(case_id,layer,registry):
    inventory=registry['inventory']
    if layer=='catalog' and case_id in {'PA-01','GD-04'}:
        return [(f'SERVICE-{r["id"]}',{'service_id':r['id']}) for r in inventory['services']]
    if layer=='protocol' and case_id in {'PA-02','PA-03','PA-04','PA-05','PA-07','GUI07'}:
        protocol_ids=inventory['protocol_ids']
        if case_id=='GUI07':protocol_ids=['azure_translator']
        elif case_id=='PA-03':
            protocol_ids=list(dict.fromkeys(r['protocol_id'] for r in inventory['services'] if r['kind']=='http' and r['allowed_purposes']==['translation']))
        return [(f'PROTOCOL-{x}',{'protocol_id':x}) for x in protocol_ids]
    if layer=='service' and case_id=='PA-11':
        return [(f'HTTP-{r["id"]}-PURPOSE-{purpose}',{'service_id':r['id'],'purpose':purpose}) for r in inventory['services'] if r['kind']=='http' for purpose in r['allowed_purposes']]
    if layer=='service' and case_id in {'GD-05','GD-11'}:
        return [(f'HTTP-{r["id"]}',{'service_id':r['id']}) for r in inventory['services'] if r['kind']=='http']
    if layer=='worker' and case_id in {'OT-06','OT-08','OT-09','OT-11','OT-16','SS-05','GD-10'}:
        engine_ids=inventory['offline_engine_ids']
        if case_id=='OT-08':engine_ids=[x for x in engine_ids if x in {'jbeijing','kingsoft_fastait'}]
        elif case_id=='OT-09':engine_ids=[x for x in engine_ids if x!='apple_translation']
        if case_id in {'OT-08','OT-16'}:
            return [(f'ENGINE-{x}-RUNTIME-{runtime}',{'engine_id':x,'runtime':runtime}) for x in engine_ids for runtime in (('CROSSOVER',) if x in {'jbeijing','kingsoft_fastait'} else ('MACOS',))]
        return [(f'ENGINE-{x}',{'engine_id':x}) for x in engine_ids]
    if case_id=='GUI02' and layer in {'domain','component','service'}:
        return [(f'PURPOSE-{purpose}',{'purpose':purpose}) for purpose in ('grammar','context_word')]
    if case_id=='GUI08' and layer in {'domain','component','service'}:
        return [(f'CONTEXT-{scope}-{count}',{'purpose':'translation','context_range':scope,'context_count':count}) for scope in ('original','translation','both') for count in (0,2)]
    if layer=='game':
        games=[(x,{'game_id':x,'minimum_distinct_states':5,'minimum_confirmed_pages':3 if x=='GAME-N7' else 1,'language_branch':'non_japanese' if x=='GAME-SR' else 'japanese'}) for x in ('GAME-SR','GAME-MO','GAME-N7','GAME-SAKURA')]
        if case_id=='OT-16':
            return [(game+'-ENGINE-'+engine['id'],{**dims,'engine_id':engine['id'],'engine_contract_revision':engine['engine_contract_revision'],'execution_backend':engine['execution_backend'],'runtime':'CROSSOVER' if engine['id'] in {'jbeijing','kingsoft_fastait'} else 'MACOS'}) for game,dims in games for engine in inventory['services'] if engine['kind']=='offline']
        return games
    if layer=='native' and case_id in {'GD-03','GD-10','FL-08'}:
        return [(x,{'permission_state':x}) for x in ('PERMISSION-FIRST','PERMISSION-DENIED','PERMISSION-CANCELLED','PERMISSION-REVOKED','PERMISSION-RESTORED')]
    if layer in {'native','component','review'} and case_id in {'GD-12','FL-10'}:
        return [(x,{'display_condition':x}) for x in ('DARK','LIGHT','NARROW-LONG','KEYBOARD-FOCUS-ESCAPE','ZOOM-200-UNCLIPPED')]
    if layer=='credentials': return [(x,{'credential_condition':x}) for x in ('MASKED-INPUT','VAULT-RESTART','CANCEL-REVOKE','EXPORT-LOG-NO-SECRET','TYPED-AUTH-FAILURE')]
    if layer=='storage': return [(x,{'storage_condition':x}) for x in ('SAVE-RESTART','READONLY-DISKFULL','MIGRATION-ROLLBACK','FULL-FILTERED-EXPORT')]
    return [('DEFAULT',{})]


def required_executions(root_or_registry):
    """Return only the additional current inventory, never the legacy 153 IDs."""
    registry=root_or_registry if isinstance(root_or_registry,dict) else _read(_path(root_or_registry,REGISTRY))
    result={}
    if registry.get('current_scope')!=CURRENT_SCOPE:raise ValueError('unreviewed current release scope')
    for case in registry['cases']:
        for layer in case['required_layers']:
            for variant,dimensions in _variants(case['id'],layer,registry):
                identifier=f'{case["id"]}@{layer}@{variant}'
                scope='design_only' if layer=='design' else 'source_build' if layer in SOURCE_LAYERS else 'final_package'
                result[identifier]={'execution_id':identifier,'case_id':case['id'],'layer':layer,'variant_id':variant,'dimensions':dimensions,'evidence_scope':scope,'environment_ids':['ENV-CURRENT-'+layer.upper(),variant], 'required_evidence':(['actual_private_ipc_request_response_bytes_redacted','protocol_termination','identity_and_revision'] if case['id']=='OT-09' and layer=='protocol' else EVIDENCE[layer]+['actual_engine_resource_identity','actual_engine_translation_results','independent_engine_quality_review'] if case['id']=='OT-16' and layer=='game' else EVIDENCE[layer])}
    return result


def _inventory(root):
    catalog=_read(_path(root,CATALOG)); presentation=_read(_path(root,PRESENTATION))
    from guided_service_reading import validate_catalog
    descriptor_errors=validate_catalog(catalog,presentation)
    if descriptor_errors:raise ValueError('source catalog/presentation contract: '+ '; '.join(descriptor_errors))
    protocols=[r['id'] for r in catalog['protocols']]; offline=[r['id'] for r in catalog['offline_engines']]
    services=[{key:r[key] for key in ('id','kind','allowed_purposes','model_identity')+ (('protocol_id','profile_id','adapter_contract_revision','profile_revision','preset_revision','auth_kind') if r['kind']=='http' else ('execution_backend','engine_contract_revision'))} for r in presentation['services']]
    if len(protocols)!=13 or len(set(protocols))!=13 or len(offline)!=4 or len(set(offline))!=4 or len(services)!=25 or {r['id'] for r in services}!={r['id'] for r in catalog['presets']+catalog['offline_engines']}: raise ValueError('exact 21 HTTP/13 protocols/4 engines/25 services required')
    return {'counts':{'http_presets':21,'http_protocols':13,'offline_engines':4,'services':25},'protocol_ids':protocols,'offline_engine_ids':offline,'services':services}



def separate_platform_qualifications():
    """Preserve only the four explicit Windows obligations outside this Mac RC."""
    return [{'execution_id':f'{case}@worker@ENGINE-{engine}-RUNTIME-WINDOWS',
             'case_id':case,'layer':'worker','engine_id':engine,'runtime':'WINDOWS',
             'scope':'separate_windows_platform_qualification',
             'environment_ids':['ENV-WINDOWS-NATIVE','ENGINE-'+engine],
             'abi_policy':'same_bitness_isolated_worker_and_engine_dll',
             'resource_policy':'licensed_engine_dll_version_architecture_and_content_hash',
             'artifact_policy':'independent_windows_worker_artifact_and_resources; never reuse macOS app hash',
             'required_evidence':['actual_windows_worker_identity','engine_abi_and_licensed_resource_identity','actual_result_fault_and_recovery'],
             'status':'not_run','release_required_for_current_scope':False}
            for case in ('OT-08','OT-16') for engine in ('jbeijing','kingsoft_fastait')]


def validate_registry(root, registry=None):
    errors=[];root=Path(root)
    try:
        data=registry if registry is not None else _read(_path(root,REGISTRY))
        if data.get('current_scope')!=CURRENT_SCOPE:errors.append('current release scope must remain macos-arm64-crossover')
        if not _same(data.get('separate_platform_qualifications'),separate_platform_qualifications()): errors.append('only the four original OT-08/OT-16 Windows DLL qualifications may remain separate/not_run; no arbitrary current-scope omission')
        if data.get('schema_version')!=1 or type(data.get('schema_version')) is not int: errors.append('schema_version integer 1 required')
        for key,value in {'design_target':DESIGN,'source_catalog':CATALOG,'gui_source':GUI_SOURCE}.items():
            if data.get(key)!=value: errors.append('authoritative pointer mismatch: '+key)
        if data.get('phase')!='additional_current_design_plan' or data.get('product_execution')!='not_run' or data.get('design_approval')!='pending_review': errors.append('static planning cannot grant execution/design approval')
        if not _same(data.get('inventory'),_inventory(root)): errors.append('exact service/protocol/engine inventory or immutable tuple drift')
        source=source_rows(root); cases=data.get('cases',[])
        ids=[r.get('id') for r in cases if isinstance(r,dict)]
        if len(cases)!=70 or len(ids)!=70 or len(set(ids))!=70 or set(ids)!=set(source): errors.append('exact 70 source cases required without duplicate/missing/unknown IDs')
        legacy=data.get('legacy_contract',{})
        if not _same({k:legacy.get(k) for k in ('definitions','executions','source_cases','original_client_cases')},{'definitions':128,'executions':153,'source_cases':97,'original_client_cases':111}): errors.append('legacy inventory must remain independent 128/153/97/111')
        if legacy.get('files')!={p:digest(_path(root,p)) for p in LEGACY_FILES}: errors.append('frozen legacy plan/CSV identity drift')
        modules={r['module_id'] for r in csv.DictReader(_path(root,'docs/architecture/engineering-modules.csv').read_text().splitlines())}
        components={r['component_id'] for r in csv.DictReader(_path(root,'docs/architecture/engineering-components.csv').read_text().splitlines())}
        prereqs={r['id'] for r in _read(_path(root,'docs/development/first-product-test-operations.json'))['prerequisites']}|{'CURRENT-SERVICE-MATERIALS','CURRENT-OFFLINE-LICENSE','CURRENT-GUI-REVIEW'}
        for row in cases:
            cid=row.get('id'); actual=source.get(cid)
            if not actual: continue
            if row.get('source_group')!=actual['source_group'] or not _same(row.get('source_record'),actual['source_record']): errors.append(f'{cid}: source assertion/requirements/layers/status must remain exact')
            if row.get('required_layers')!=required_layers(cid,actual['source_record']): errors.append(f'{cid}: required real layers cannot be weakened or invented')
            if row.get('status')!='not_run' or row.get('release_required') is not True: errors.append(f'{cid}: planning is not_run and release required')
            mapping=case_mapping(cid,actual['source_record'],required_layers(cid,actual['source_record']))
            for key,value in mapping.items():
                if not _same(row.get(key),value): errors.append(f'{cid}: reviewed module/component/Owner/prerequisite mapping changed: {key}')
            expected_ids=sorted({value for key in ('source_requirement_ids','source_configuration_requirement_ids','source_reading_requirement_ids') for value in actual['source_record'].get(key,[])})
            if row.get('requirement_ids')!=expected_ids:errors.append(f'{cid}: exact authoritative requirement mapping required')
            for key,allowed in [('module_ids',modules),('component_ids',components),('prerequisite_ids',prereqs),('owners',{'AP','AI','UI','PL','HK','QA','JA'})]:
                if not _strings(row.get(key)) or not set(row[key])<=allowed: errors.append(f'{cid}: invalid/missing {key}')
            for key in ('steps','observable_expectations'):
                if not _strings(row.get(key)): errors.append(f'{cid}: nonempty {key} required')
            refs=row.get('design_refs')
            if not _strings(refs): errors.append(f'{cid}: exact current design references required')
            else:
                for ref in refs:
                    path,sep,anchor=ref.partition('#')
                    if path!=DESIGN or not sep or not re.search(r'\bid=["\']'+re.escape(anchor)+r'["\']',_path(root,path).read_text()): errors.append(f'{cid}: missing current design anchor')
            if row.get('design_checks')!=['guided-service-reading-design']: errors.append(f'{cid}: design references cannot register product adapters')
        if 'adapters' in data or 'adapter_registry' in data: errors.append('registry cannot self-authorize adapter commands/capabilities')
    except (OSError,ValueError,KeyError,TypeError,AttributeError) as error:
        errors.append('current coverage input invalid: '+str(error))
    return errors


def _hashed(root,reference):
    if not isinstance(reference,dict) or set(reference)!={'path','sha256'} or not isinstance(reference['sha256'],str) or not SHA.fullmatch(reference['sha256']): raise ValueError('exact path/hash reference required')
    path=_path(root,reference['path'])
    if not path.is_file() or not path.stat().st_size or digest(path)!=reference['sha256']: raise ValueError('evidence missing/empty/hash drift')
    return path


def _artifact(root,artifact):
    if isinstance(artifact,str) and SHA.fullmatch(artifact): return artifact
    if artifact is None: raise ValueError('final artifact identity required')
    path=_path(root,str(artifact))
    if not path.is_file() or not path.stat().st_size: raise ValueError('final artifact missing/empty')
    return digest(path)


def _inspect_receipt(root, receipt, expected, artifact):
    """Inspect code-owned capabilities, time/nonce/log observations, and dimensions."""
    adapter=ADAPTERS.get(receipt.get('adapter_id'))
    if not adapter: raise ValueError('real adapter not implemented/registered for this current coverage')
    if expected['layer'] not in adapter['layers'] or expected['case_id'] not in adapter['case_ids'] or expected['execution_id'] not in adapter.get('execution_ids',()): raise ValueError('adapter has no exact case/layer/variant capability')
    if receipt.get('command')!=list(adapter['command']) or adapter.get('evidence_schema')!='current_product_execution_v1': raise ValueError('adapter command/evidence schema differs from code registration')
    for key,value in {'schema_version':1,'status':'passed','errors':[],'exit_code':0,'source_tree_sha256':source_identity(root),'artifact_sha256':artifact,'fixture_substitution':False}.items():
        if receipt.get(key)!=value or type(receipt.get(key))is not type(value): raise ValueError('receipt identity/status mismatch: '+key)
    nonce=receipt.get('execution_nonce')
    if not isinstance(nonce,str) or not NONCE.fullmatch(nonce): raise ValueError('fresh execution nonce required')
    started=datetime.fromisoformat(receipt['started_at'].replace('Z','+00:00'));finished=datetime.fromisoformat(receipt['finished_at'].replace('Z','+00:00'))
    if started.tzinfo is None or finished.tzinfo is None or started>finished: raise ValueError('timezone-aware actual interval required')
    scope=expected['evidence_scope'];flag='design_only_executed' if scope=='design_only' else 'product_component_tests_executed' if scope=='source_build' else 'product_packaged_native_game_tests_executed'
    if receipt.get(flag)is not True or adapter.get('scope')!=scope: raise ValueError('cross-layer/fixture substitution forbidden')
    rows=receipt.get('execution_results')
    if not isinstance(rows,list) or any(not isinstance(x,dict) for x in rows) or len({x.get('execution_id') for x in rows})!=len(rows): raise ValueError('unique actual execution results required')
    matches=[r for r in rows if r.get('execution_id')==expected['execution_id']]
    if len(matches)!=1: raise ValueError('missing unique actual per-execution assertions')
    row=matches[0]
    for key,value in {**{k:expected[k] for k in ('execution_id','case_id','layer','variant_id','dimensions','evidence_scope','environment_ids')},'source_tree_sha256':receipt['source_tree_sha256'],'artifact_sha256':artifact,'execution_nonce':nonce,'fixture_substitution':False,'status':'passed'}.items():
        if not _same(row.get(key),value) or type(row.get(key))is not type(value): raise ValueError('actual assertion identity/environment mismatch: '+key)
    if type(row.get('assertion_count'))is not int or row['assertion_count']<=0 or type(row.get('skipped_assertions'))is not int or row['skipped_assertions']!=0: raise ValueError('positive assertions and zero skipped required')
    observed=datetime.fromisoformat(row['observed_at'].replace('Z','+00:00'))
    if observed.tzinfo is None or not started<=observed<=finished: raise ValueError('observation outside actual execution interval')
    evidence=row.get('evidence',[])
    if not isinstance(evidence,list) or not evidence or len({x.get('kind') for x in evidence if isinstance(x,dict)})!=len(evidence) or {x.get('kind') for x in evidence if isinstance(x,dict)}!=set(expected['required_evidence']): raise ValueError('every required observation kind must have unique evidence')
    for item in evidence: _hashed(root,{'path':item['path'],'sha256':item['sha256']})
    log=_hashed(root,{'path':receipt['log'],'sha256':receipt['log_sha256']});logged=[]
    for line in log.read_text().splitlines():
        if line.startswith('LUNA_CURRENT_PRODUCT_RESULT '): logged.append(json.loads(line[len('LUNA_CURRENT_PRODUCT_RESULT '):],object_pairs_hook=_unique,parse_constant=lambda x: (_ for _ in ()).throw(ValueError('non-JSON log value'))))
    if any(not isinstance(x,dict) for x in logged) or len({x.get('execution_id') for x in logged})!=len(logged): raise ValueError('duplicate/malformed actual log records')
    if not _same([x for x in logged if x.get('execution_id')==expected['execution_id']],[row]): raise ValueError('summary differs from actual adapter log')
    dims=expected['dimensions'];observations=row.get('observations',{})
    if 'game_id' in dims:
        states=observations.get('states',[])
        if not isinstance(states,list) or len(states)<dims['minimum_distinct_states'] or any(not isinstance(x,dict) for x in states): raise ValueError('five distinct actual game/client states required')
        identities=[x.get('state_id') for x in states];texts=[x.get('screen_text_sha256') for x in states]
        if not _strings(identities) or not _strings(texts) or any(not SHA.fullmatch(x) for x in texts): raise ValueError('duplicate events cannot stand in for distinct subtitle states')
        capture_hashes={'game_capture':set(),'client_capture':set()}
        for state in states:
            for key in ('game_capture','client_capture','callback_evidence'):
                _hashed(root,state[key])
                if key in capture_hashes:
                    sha=state[key]['sha256']
                    if sha in capture_hashes[key]:raise ValueError('reused '+key+' cannot prove distinct actual subtitle states')
                    capture_hashes[key].add(sha)
            if not state.get('page_id'): raise ValueError('confirmed game page identity required')
        if len({x['page_id'] for x in states})<dims['minimum_confirmed_pages'] or observations.get('language_branch')!=dims['language_branch']: raise ValueError('confirmed pages/language branch requirement not met')
    if expected['case_id']=='OT-16' and expected['layer']=='game':
        engine_identity={key:dims[key] for key in ('engine_id','engine_contract_revision','execution_backend','runtime')}
        if not _same(observations.get('engine_identity'),engine_identity):raise ValueError('OT-16 requires this exact engine/runtime/resource contract, not aggregate worker or game proof')
        by_kind={item['kind']:item for item in evidence}
        def content(kind):
            item=by_kind[kind]
            return _read(_hashed(root,{'path':item['path'],'sha256':item['sha256']}))
        resources=content('actual_engine_resource_identity')
        if not isinstance(resources,dict) or not _same(resources.get('engine_identity'),engine_identity) or not isinstance(resources.get('resource_version'),str) or not resources['resource_version'].strip():raise ValueError('OT-16 resource identity/version must belong to exact engine/runtime')
        if resources.get('status')!='ready' or resources.get('authorized')is not True or resources.get('route_supported')is not True:raise ValueError('OT-16 missing authorization or unsupported/unknown route remains blocked')
        files=resources.get('resource_artifacts')
        if not isinstance(files,list) or not files:raise ValueError('OT-16 requires actual engine/platform resource file identity')
        for reference in files:_hashed(root,reference)
        _hashed(root,resources.get('license_evidence'))
        translations=content('actual_engine_translation_results')
        if not isinstance(translations,list) or len(translations)!=len(states) or any(not isinstance(item,dict) for item in translations):raise ValueError('OT-16 needs this engine translation for every observed game state')
        targets=[item.get('state_id') for item in translations]
        if not _strings(targets) or set(targets)!={state['state_id'] for state in states}:raise ValueError('OT-16 engine translation target states differ from actual game states')
        result_kinds=set()
        for item in translations:
            identity={'engine_identity':engine_identity,'game_id':dims['game_id'],'artifact_sha256':artifact,'execution_nonce':nonce}
            if any(not _same(item.get(key),value) for key,value in identity.items()):raise ValueError('OT-16 translation belongs to another engine/game/package/run')
            source_language='zh' if dims['game_id']=='GAME-SR' else 'ja'
            kind=item.get('execution_kind');result_kinds.add(kind)
            if item.get('source_language')!=source_language or not isinstance(item.get('target_language'),str) or not item['target_language'].strip():raise ValueError('OT-16 actual source/target language must be explicit')
            if kind=='engine_translation':
                if item.get('engine_invoked')is not True or item.get('status')!='completed':raise ValueError('OT-16 translation quality requires actual engine invocation/completion')
            elif dims['game_id']=='GAME-SR' and kind in {'same_language_passthrough','non_japanese_downgrade'}:
                if item.get('engine_invoked')is not False or item.get('status') not in {'passthrough','downgraded_non_japanese'}:raise ValueError('SR passthrough/downgrade is a non-Japanese branch, not invoked translation')
                if kind=='same_language_passthrough' and item['target_language']!=source_language:raise ValueError('same-language passthrough requires equal source/target languages')
            else:raise ValueError('OT-16 unsupported route/uninvoked Japanese translation remains blocked')
            text=_hashed(root,item.get('translation_evidence')).read_text(encoding='utf-8')
            if not text.strip():raise ValueError('OT-16 actual translation must not be empty')
        quality=content('independent_engine_quality_review')
        if not isinstance(quality,dict) or not _same(quality.get('engine_identity'),engine_identity) or quality.get('game_id')!=dims['game_id'] or quality.get('artifact_sha256')!=artifact or quality.get('decision')!='accepted' or not isinstance(quality.get('reviewer'),str) or not quality['reviewer'].strip():raise ValueError('OT-16 needs independent quality evidence for exact engine/game/package')
        invoked_only=result_kinds=={'engine_translation'}
        required_quality='actual_japanese_translation_quality' if dims['game_id']!='GAME-SR' else 'actual_non_japanese_translation_quality' if invoked_only else 'non_japanese_branch_only'
        if quality.get('quality_scope')!=required_quality or quality.get('translation_quality_assessed')is not invoked_only:raise ValueError('SR Chinese/passthrough cannot claim Japanese translation quality; actual engine quality and branch checks remain separate')
    if dims.get('display_condition')=='ZOOM-200-UNCLIPPED' and (observations.get('zoom_percent')!=200 or observations.get('content_unclipped')is not True or observations.get('all_actions_reachable')is not True): raise ValueError('200% requires actual unclipped content and reachable controls; DOM presence is insufficient')
    if dims.get('permission_state') in {'PERMISSION-DENIED','PERMISSION-CANCELLED','PERMISSION-REVOKED'} and (observations.get('claimed_live')is not False or observations.get('hook_independent')is not True): raise ValueError('permission denial/cancellation must not claim live preview or break Hook')
    if expected['layer']=='credentials' and observations.get('plaintext_secret_exposed')is not False: raise ValueError('Vault/export/log secret boundary must be observed')
    return row


def release_coverage_gaps(root,evidence=None,artifact=None):
    root=Path(root);errors=validate_registry(root)
    if errors: return [{'case_id':'*','execution_id':'*','layer':'registry','status':'invalid','reason':e} for e in errors]
    required=required_executions(root)
    # The static registry retains its captured pending_review identity. Only the
    # separately pinned explicit source-version decision can clear this gap.
    gaps=[{'case_id':'*','execution_id':'*','layer':'design_approval','status':'blocked','reason':reason}
          for reason in user_approval_gaps(root)]
    sidecar=None;artifact_hash=None
    try:
        artifact_hash=_artifact(root,artifact)
        outer=evidence if isinstance(evidence,dict) else _read(_path(root,str(evidence))) if evidence is not None else {}
        sidecar=_read(_hashed(root,outer.get('current_product_coverage')))
        if sidecar.get('schema_version')!=1 or type(sidecar.get('schema_version'))is not int or sidecar.get('artifact_sha256')!=artifact_hash or sidecar.get('registry_sha256')!=digest(_path(root,REGISTRY)): raise ValueError('sidecar schema/artifact/registry identity drift')
        rows=sidecar.get('execution_results')
        if not isinstance(rows,list) or any(not isinstance(x,dict) for x in rows): raise ValueError('sidecar execution_results required')
        ids=[x.get('execution_id') for x in rows]
        if len(set(ids))!=len(ids) or set(ids)-set(required): raise ValueError('duplicate/unknown current execution identity')
    except (OSError,ValueError,TypeError,KeyError,AttributeError) as error:
        rows=[];gaps.append({'case_id':'*','execution_id':'*','layer':'evidence','status':'blocked','reason':str(error)})
    records={r['execution_id']:r for r in rows};receipts={}
    for identifier,expected in required.items():
        reason=None;status='not_run';record=records.get(identifier)
        if record is None: reason='No current per-execution receipt; old 153 and design checks cannot substitute'
        else:
            try:
                path=_hashed(root,record.get('receipt'))
                if path not in receipts: receipts[path]=_read(path)
                _inspect_receipt(root,receipts[path],expected,artifact_hash)
            except (OSError,ValueError,KeyError,TypeError,AttributeError) as error: reason=str(error);status='blocked'
        if reason:gaps.append({'case_id':expected['case_id'],'execution_id':identifier,'layer':expected['layer'],'status':status,'reason':reason})
    return gaps


def _assessment_record(root,errors,gaps):
    return {'schema_version':1,'target_id':DESIGN_TARGET,'composition_id':CURRENT_COMPOSITION_ID,
            'status':'failed' if errors else 'blocked' if gaps else 'passed',
            'check_scope':'current_plan_and_evidence_identity_only','not_product_execution':True,
            'product_tests_executed':False,'product_alignment_approved':False,'errors':errors,'release_gaps':gaps,
            'required_executions':len(required_executions(root)) if not errors else 0,
            'registered_adapters':sorted(ADAPTERS),'legacy_153_independently_required':True,
            'current_scope':CURRENT_SCOPE,'separate_platform_qualifications':len(separate_platform_qualifications()),
            'effective_design_approval':source_approval(root)}

def assessment_report(root=ROOT,evidence=None,artifact=None):
    """Recompute read-only current obligations, authority and actual evidence gaps."""
    errors=validate_registry(root)
    return _assessment_record(root,errors,release_coverage_gaps(root,evidence,artifact))

def assessment_errors(root,report,exit_code,evidence=None,artifact=None):
    if not isinstance(report,dict): return ['current product assessment must be an object']
    errors=[]
    if (type(report.get('schema_version')) is not int or report['schema_version']!=1
            or any(type(report.get(key)) is not int or report[key]<0
                   for key in ('required_executions','separate_platform_qualifications'))
            or not isinstance(report.get('errors'),list) or any(not isinstance(value,str) for value in report['errors'])
            or not isinstance(report.get('registered_adapters'),list) or any(not isinstance(value,str) for value in report['registered_adapters'])
            or not isinstance(report.get('release_gaps'),list) or any(not isinstance(value,dict) for value in report['release_gaps'])
            or not isinstance(report.get('effective_design_approval'),dict)
            or report.get('not_product_execution') is not True or report.get('product_tests_executed') is not False
            or report.get('product_alignment_approved') is not False
            or report.get('legacy_153_independently_required') is not True):
        errors.append('current product assessment requires typed obligations, gaps, authority and read-only facts')
    status=report.get('status')
    if (not isinstance(status,str) or status not in {'passed','blocked','failed'}
            or type(exit_code) is not int or exit_code not in (0,1)
            or (exit_code==0)!=(status=='passed')
            or status!='failed' and isinstance(report.get('release_gaps'),list)
            and bool(report['release_gaps'])!=(status=='blocked')):
        errors.append('current product assessment exit/status must match actual release gaps')
    if not _same(report,assessment_report(root,evidence,artifact)):
        errors.append('current product assessment differs from independently recomputed target/composition/obligations/adapters/approval/release gaps')
    if report.get('errors')!=[]: errors.append('current product assessment metadata contract is invalid')
    return errors

def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--check',action='store_true');mode.add_argument('--assess',action='store_true')
    parser.add_argument('--root',type=Path,default=ROOT);parser.add_argument('--evidence');parser.add_argument('--artifact');parser.add_argument('--json',action='store_true')
    args=parser.parse_args(argv)
    result=assessment_report(args.root,args.evidence,args.artifact) if args.assess else _assessment_record(args.root,validate_registry(args.root),[])
    status=result['status']
    print(json.dumps(result,ensure_ascii=False,indent=2) if args.json else f'Current product coverage: {status}; no product tests executed')
    return 0 if status=='passed' else 1

if __name__=='__main__':raise SystemExit(main())
