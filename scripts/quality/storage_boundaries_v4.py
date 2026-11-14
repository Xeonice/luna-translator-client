#!/usr/bin/env python3
"""Schema4 differential SQL boundaries from frozen independent design only.

Field projections preserve exact CREATE TABLE checks; FK projections isolate one
actual FK at a time so an unrelated parent cannot mask its enforcement. Neither
projection is a producer/migration/runtime acceptance receipt.
"""
from __future__ import annotations
import argparse,copy,hashlib,itertools,json,re,sqlite3
from contextlib import closing
from pathlib import Path
import storage_schema_v4 as schema
from database_design import ROOT,_authorizer
from storage_boundaries import decode,ident,checks

REGISTRY=schema.BASE+'field-boundaries.json'
FIXTURES=schema.BASE+'boundary-fixtures.json'
SEMANTIC_PROBE_IDS=['V4-SEMANTIC:R4-connection-GC-child-first-then-parent', 'V4-SEMANTIC:R4-connection-GC-parent-delete-before-child-FK', 'V4-SEMANTIC:R4-connection-HTTP-then-orphan-proof-conflict', 'V4-SEMANTIC:R4-connection-HTTP-two-models-conflicting-origin', 'V4-SEMANTIC:R4-connection-HTTP-two-models-consistent-origin', 'V4-SEMANTIC:R4-connection-cannot-borrow-running-control', 'V4-SEMANTIC:R4-connection-codec3-source-chain-forbidden', 'V4-SEMANTIC:R4-connection-direct-effective-FK', 'V4-SEMANTIC:R4-connection-direct-source-FK', 'V4-SEMANTIC:R4-connection-effective-has-model-before-proof', 'V4-SEMANTIC:R4-connection-effective-must-be-codec3', 'V4-SEMANTIC:R4-connection-late-proof-after-seal', 'V4-SEMANTIC:R4-connection-model-added-after-proof-rejects-seal', 'V4-SEMANTIC:R4-connection-new-unique-index-is-enforced', 'V4-SEMANTIC:R4-connection-not-execution-attempt_execution_targets', 'V4-SEMANTIC:R4-connection-not-execution-binding_engines', 'V4-SEMANTIC:R4-connection-not-execution-task_execution_targets', 'V4-SEMANTIC:R4-connection-old-direct-sealed-no-model-valid', 'V4-SEMANTIC:R4-connection-orphan-proof-then-HTTP-conflict', 'V4-SEMANTIC:R4-connection-proof-is-immutable', 'V4-SEMANTIC:R4-connection-sealed-proof-delete', 'V4-SEMANTIC:R4-connection-self-source-forbidden', 'V4-SEMANTIC:R4-connection-shape-control-config-nonnull', 'V4-SEMANTIC:R4-connection-shape-control-connection-null', 'V4-SEMANTIC:R4-connection-shape-effective-connection-null', 'V4-SEMANTIC:R4-connection-shape-effective-model', 'V4-SEMANTIC:R4-connection-shape-effective-offline', 'V4-SEMANTIC:R4-connection-shape-reason-collision', 'V4-SEMANTIC:R4-connection-shape-reason-selected', 'V4-SEMANTIC:R4-connection-shape-source-connection-null', 'V4-SEMANTIC:R4-connection-shape-source-model', 'V4-SEMANTIC:R4-connection-shape-source-offline', 'V4-SEMANTIC:R4-connection-shape-target-unknown', 'V4-SEMANTIC:R4-connection-source-GC-before-proof', 'V4-SEMANTIC:R4-connection-source-GC-while-child-live', 'V4-SEMANTIC:R4-connection-source-GC-without-child-control', 'V4-SEMANTIC:R4-connection-source-is-not-orphan', 'V4-SEMANTIC:R4-connection-source-seal-hash-mismatch', 'V4-SEMANTIC:R4-connection-source-unsealed', 'V4-SEMANTIC:R4-connection-two-distinct-orphan-connections', 'V4-SEMANTIC:R4-connection-without-proof-cannot-seal', 'V4-SEMANTIC:activation-preserves-draft', 'V4-SEMANTIC:alias-cannot-borrow-live-control', 'V4-SEMANTIC:config-installation-resource-runtime', 'V4-SEMANTIC:dedup-codec3-LegacyV2-source-duplicate', 'V4-SEMANTIC:dedup-codec3-TypedV3-alias-duplicate', 'V4-SEMANTIC:dedup-codec3-TypedV3-direct-duplicate', 'V4-SEMANTIC:dedup-codec3-TypedV3-distinct', 'V4-SEMANTIC:dedup-codec3-TypedV3-distinct-sources', 'V4-SEMANTIC:dedup-legacy-codec1-duplicate-disabled', 'V4-SEMANTIC:dedup-legacy-codec2-duplicate-disabled', 'V4-SEMANTIC:dedup-mixed-legacy-cannot-hide-typed-duplicate', 'V4-SEMANTIC:dedup-mixed-legacy-duplicate-typed-distinct', 'V4-SEMANTIC:grant-client-immutable', 'V4-SEMANTIC:grant-cursor-owned-activation', 'V4-SEMANTIC:grant-initial-authority-forbidden', 'V4-SEMANTIC:grant-stale-CAS', 'V4-SEMANTIC:installation-engine-runtime-match', 'V4-SEMANTIC:installation-environment-architecture', 'V4-SEMANTIC:lease-no-fake-target', 'V4-SEMANTIC:lease-real-test-required', 'V4-SEMANTIC:legacy-policy-original-bytes', 'V4-SEMANTIC:oauth-final-fk-clean', 'V4-SEMANTIC:offline-full-graph-FK-clean', 'V4-SEMANTIC:opaque-source-identity-kept', 'V4-SEMANTIC:origin-site-custom-absent', 'V4-SEMANTIC:origin-site-custom-reject-array', 'V4-SEMANTIC:origin-site-custom-reject-boolean', 'V4-SEMANTIC:origin-site-custom-reject-fraction', 'V4-SEMANTIC:origin-site-custom-reject-integer', 'V4-SEMANTIC:origin-site-custom-reject-nul', 'V4-SEMANTIC:origin-site-custom-reject-null', 'V4-SEMANTIC:origin-site-custom-reject-object', 'V4-SEMANTIC:origin-site-custom-reject-unknown-key', 'V4-SEMANTIC:origin-site-custom-text', 'V4-SEMANTIC:origin-site-preset-absent', 'V4-SEMANTIC:origin-site-preset-reject-array', 'V4-SEMANTIC:origin-site-preset-reject-boolean', 'V4-SEMANTIC:origin-site-preset-reject-fraction', 'V4-SEMANTIC:origin-site-preset-reject-integer', 'V4-SEMANTIC:origin-site-preset-reject-nul', 'V4-SEMANTIC:origin-site-preset-reject-null', 'V4-SEMANTIC:origin-site-preset-reject-object', 'V4-SEMANTIC:origin-site-preset-reject-unknown-key', 'V4-SEMANTIC:origin-site-preset-text', 'V4-SEMANTIC:owned-refresh-cannot-cross-grant', 'V4-SEMANTIC:payload-protocol-anthropic', 'V4-SEMANTIC:payload-protocol-azure_translator', 'V4-SEMANTIC:payload-protocol-baidu_vip', 'V4-SEMANTIC:payload-protocol-deepl_v2', 'V4-SEMANTIC:payload-protocol-gemini_generate_content', 'V4-SEMANTIC:payload-protocol-google_translate_v2', 'V4-SEMANTIC:payload-protocol-google_translate_v3', 'V4-SEMANTIC:payload-protocol-lmstudio_native_v1', 'V4-SEMANTIC:payload-protocol-ollama_native', 'V4-SEMANTIC:payload-protocol-openai_compatible', 'V4-SEMANTIC:payload-protocol-openai_responses', 'V4-SEMANTIC:payload-protocol-tencent_tokenhub_translation', 'V4-SEMANTIC:payload-protocol-youdao_v3', 'V4-SEMANTIC:payload-reject-header', 'V4-SEMANTIC:payload-reject-old-kind', 'V4-SEMANTIC:payload-reject-shadow-version', 'V4-SEMANTIC:pivot-explicit-same-runtime-valid', 'V4-SEMANTIC:pivot-no-mixed-runtime', 'V4-SEMANTIC:refresh-immutable', 'V4-SEMANTIC:resource-GC-reference', 'V4-SEMANTIC:route-GC-reference', 'V4-SEMANTIC:scope-policy-unknown-key', 'V4-SEMANTIC:sealed-resource-file-insert', 'V4-SEMANTIC:sealed-route-hop-delete', 'V4-SEMANTIC:selector-transaction-rollback', 'V4-SEMANTIC:snapshot-fixed-scope', 'V4-SEMANTIC:snapshot-immutable', 'V4-SEMANTIC:source-GC-referenced-config', 'V4-SEMANTIC:source-materialization-immutable', 'V4-SEMANTIC:source-stable-control-stays-stopped', 'V4-SEMANTIC:typed-policy-no-legacy-shadow']
PROJECTION='frozen_v4_exact_table_without_triggers_fk_off'

def _table_sql(root:Path)->dict:
 with schema.design_database(root)as db:return dict(db.execute("SELECT name,sql FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'"))

def _enum(sql:str,name:str)->list:
 match=re.search(r'\b'+re.escape(name)+r'\s+IN\s*\(([^)]*)\)',sql,re.I)
 if match:return [v.replace("''", "'")for v in re.findall(r"'((?:''|[^'])*)'",match.group(1))]
 match=re.search(r'CHECK\s*\(\s*'+re.escape(name)+r"\s*=\s*'([^']+)'",sql,re.I)
 return [match.group(1)]if match else[]

def _value(name:str,kind:str,sql:str):
 if kind=='INTEGER':
  if name=='expires_at_ms':return 10
  if name=='semantics_version':return 2
  if name=='timeout_ms':return 60000
  if name in {'position','hop_ordinal','fallback_ordinal','context_sentences','context_enabled','cache_enabled','is_suspended','enabled','selection_revision','created_at_ms','updated_at_ms','observed_at_ms','minted_at_ms','sealed_at_ms'}:return 0
  return 1
 if name.endswith('_json'):return '[]'if name=='source_json'else '{}'
 if 'sha256'in name or name in {'content_hash','machine_fingerprint','cache_key'}:return 'a'*64
 if name=='source_language':return 'ja'
 if name=='target_language':return 'zh-Hans'
 if name=='scope':return 'https://www.googleapis.com/auth/cloud-translation'
 if name=='issuer':return 'google_accounts'
 if name=='redirect_policy':return 'loopback_pkce'
 if name=='backend':return 'local_worker'
 if name=='engine_id':return 'argos_translate'
 if name=='strategy':return 'traditional_low_latency'
 if name=='relative_key':return 'engine.bin'
 if name in {'local_native_path','local_prefix_path'}:return '/synthetic/fixture'
 options=_enum(sql,name)
 if options:return options[0]
 return 'fixture-'+name

def _insert(db,table:str,row:dict):
 names=list(row);db.execute('INSERT INTO '+ident(table)+'('+','.join(map(ident,names))+') VALUES('+','.join('?'for _ in names)+')',[decode(row[n])for n in names])

def _outcome(db,table,row)->dict:
 try:
  _insert(db,table,row);db.commit();result={'outcome':'accept'}
 except OverflowError:db.rollback();return{'outcome':'binding_reject','error_class':'OverflowError'}
 except sqlite3.Error as e:db.rollback();return{'outcome':'reject','sqlite_errorcode':e.sqlite_errorcode}
 db.execute('DELETE FROM '+ident(table));db.commit();return result

def _projection(sql:str):
 db=sqlite3.connect(':memory:');db.set_authorizer(_authorizer);db.execute('PRAGMA foreign_keys=OFF');db.execute(sql)
 if db.execute('PRAGMA foreign_keys').fetchone()[0]or db.execute("SELECT count(*) FROM sqlite_schema WHERE type='trigger'").fetchone()[0]:raise ValueError('field projection contains masking FK/trigger')
 return db

def _variants(table:str,base:dict)->list[dict]:
 candidates=[{}]
 if table=='connections':
  candidates += [dict(auth_mode='google_oauth',oauth_grant_id='grant',oauth_authorization_epoch=1,credential_pool_id=None,credential_pool_revision=None),dict(http_options_schema=1,http_options_json='{"protocol":"openai_compatible"}',origin_schema=1,origin_json='{"kind":"custom"}')]
 if table=='purpose_bindings':candidates +=[dict(game_id='game',scope_key='game:game')]
 if table=='oauth_grants':candidates +=[dict(principal_state='verified',principal_subject='synthetic-subject')]
 if table=='oauth_authorization_snapshots':candidates +=[dict(authorization_epoch=2,previous_epoch=1)]
 if table=='oauth_token_material_revisions':candidates +=[dict(material_revision=2,parent_material_revision=1)]
 if table=='offline_execution_environments':candidates +=[dict(platform='crossover',bottle_identity='bottle',local_prefix_path='/synthetic',wine_build='fixture')]
 if table=='offline_resource_snapshots':candidates +=[dict(identity_kind='apple_observation',backend='platform_native',engine_id='apple_translation',manifest_sha256=None,os_build='fixture',bridge_build_sha256='a'*64,api_profile='fixture',strategy='traditional_low_latency',source_language='ja',target_language='zh-Hans',observation_epoch=1)]
 if table in {'offline_translation_routes','offline_engine_configs'}:candidates +=[dict(route_kind='explicit_pivot',consent_revision=1)if table.endswith('routes')else dict(route_policy='explicit_pivot',consent_revision=1)]
 if table=='offline_engine_configs':candidates +=[dict(resolution='resolved',snapshot_id='snapshot',route_id='route',installation_id='installation',installation_revision=1)]
 if table=='offline_resource_observations':candidates +=[dict(installation_id='installation',installation_revision=1,local_grant_id='grant',local_grant_epoch=1,snapshot_id='snapshot')]
 if table=='service_request_defaults':candidates +=[dict(source_mode='explicit',source_language='ja',target_mode='explicit',target_language='zh-Hans'),dict(target_kind='offline',model_id=None,offline_engine_config_id='offline',source_mode='config_pair',target_mode='config_pair',source_language=None,target_language=None,context_enabled=0,context_sentences=0)]
 if table=='service_default_fallbacks':candidates +=[dict(target_kind='offline',source_model_id=None,source_offline_config_id='offline')]
 if table=='binding_engines':candidates +=[dict(target_kind='offline',model_id=None,offline_engine_config_id='offline',template_id=None)]
 if table=='activation_resource_materializations':candidates +=[dict(target_kind='offline',effective_model_id=None,source_model_id=None,effective_offline_config_id='offline',source_offline_config_id='offline-source',effective_connection_id=None,source_connection_id=None,control_connection_id=None,control_config_id='control')]
 if table=='activation_resource_materializations':candidates +=[dict(target_kind='connection',reason='legacy_materialization',effective_model_id=None,source_model_id=None,effective_offline_config_id=None,source_offline_config_id=None,control_config_id=None)]
 if table in {'task_execution_targets','attempt_execution_targets'}:candidates +=[dict(target_kind='offline',http_model_id=None,http_connection_id=None,offline_engine_config_id='offline',snapshot_id='snapshot',route_id='route',observation_id='observation',observation_epoch=1),dict(selection_scope='global',selection_revision=1,effective_scope='global',effective_selection_revision=1)if table.startswith('task_')else dict(target_kind='offline',http_model_id=None,http_connection_id=None,offline_engine_config_id='offline',snapshot_id='snapshot',route_id='route',observation_id='observation',observation_epoch=1,runtime_build_sha256='a'*64,provenance_sha256='a'*64,worker_instance_id='worker',worker_protocol_revision=1)]
 if table=='offline_test_targets':candidates +=[dict(phase='translation',snapshot_id='snapshot',route_id='route',observation_id='observation',observation_epoch=1,runtime_build_sha256='a'*64)]
 if table=='execution_authorization_leases':candidates +=[dict(origin_kind='user_oauth',auth_kind='google_oauth_refresh',grant_id='grant',authorization_epoch=1,material_revision=1,client_policy_sha256='a'*64,pool_id=None,pool_revision=None,member_id=None),dict(attempt_id='attempt',task_id='task',test_id=None)]
 # Combinations are needed e.g. offline attempt with worker identity, user OAuth attempt.
 original=candidates[:]
 for a in original:
  for b in original:candidates.append(a|b)
 result=[]
 for changes in candidates:
  row=base|{k:v for k,v in changes.items()if k in base}
  if row not in result:result.append(row)
 return result

def fixture_template(root:Path=ROOT)->dict:
 contract=schema.design_contract(root);prior=schema.v3.design_contract(root);tables=_table_sql(root)
 old=schema.read_json(root,'docs/architecture/database/boundary-fixtures.json')['tables'];http=schema.read_json(root,'docs/architecture/database/v3/boundary-fixtures.json')['tables'];output={}
 for table,names in schema.new_columns(contract,prior).items():
  sql=tables[table];columns=contract['databases']['profile']['tables'][table]['columns'];base={c['name']:(None if not c['notnull']and not c['pk']else _value(c['name'],c['type'],sql))for c in columns}
  if 'profile.'+table in old:base.update(old['profile.'+table]['valid_row'])
  if table in http:base.update(http[table])
  base.update({k:v for k,v in {'model_id':'model','http_model_id':'model','http_connection_id':'connection','source_model_id':'model','effective_model_id':'model','source_connection_id':'connection','effective_connection_id':'connection','control_connection_id':'connection','manifest_sha256':'a'*64,'http_options_schema':1,'http_options_json':'{"protocol":"openai_compatible"}','origin_schema':1,'origin_json':'{"kind":"custom"}'}.items()if k in base})
  if table=='test_records':base['model_id']=None
  if table=='execution_authorization_leases':base.update(test_id='test',origin_kind='service_account',auth_kind='google_service_account',pool_id='pool',pool_revision=1,member_id='member')
  if table=='offline_resource_snapshots':base.update(identity_kind='file_manifest',backend='local_worker')
  if table in {'task_execution_targets','attempt_execution_targets'}:base.update(target_kind='http')
  if table=='service_request_defaults':base.update(target_kind='http',source_mode='inherit_source',target_mode='inherit_preference',source_language=None,target_language=None)
  if table=='activation_resource_materializations':base.update(target_kind='http')
  variants=[]
  for row in _variants(table,base):
   with closing(_projection(sql))as db:
    if _outcome(db,table,row)['outcome']=='accept':variants.append(row)
  if not variants:raise ValueError('no independently valid SQL fixture: '+table+' '+str(base))
  per_field={}
  for name in names:
   choices=[r for r in variants if r[name]is not None]
   if not choices:
    for row in variants:
     trial=row|{name:_value(name,next(c['type']for c in columns if c['name']==name),sql)}
     with closing(_projection(sql))as db:
      if _outcome(db,table,trial)['outcome']=='accept':choices.append(trial)
   if not choices:raise ValueError('no non-NULL positive control for '+table+'.'+name)
   per_field[name]=choices[0]
  output[table]={'valid_variants':variants,'field_controls':per_field}
 return{'schema_version':1,'kind':'synthetic_schema_four_boundary_fixtures','not_product_data':True,'source_design_sha256':schema.DESIGN_SHA256,'scope':'independent frozen design SQL only; no production writer or filesystem resources','tables':output}

def _mutations(column:dict,control:dict)->list[tuple]:
 name=column['name'];value=control[name];kind=column['type'];cases=[('valid',{}),('type',{name:{'$bytes_hex':'00ff'}}),('null',{name:None})]
 if kind=='INTEGER':
  for label,v in [('fraction',1.5),('negative',-1),('zero',0),('u32_max',4294967295),('u32_overflow',4294967296)]:cases.append((label,{name:v}))
  for label,v in [('physical_i64_over',2**63),('physical_i64_under',-(2**63)-1)]:cases.append((label,{name:{'$integer':str(v)}}))
 else:
  for label,v in [('empty',''),('nul',str(value)+'\0tail'),('long_utf8','語'*8193)]:cases.append((label,{name:v}))
  if name.endswith('_json'):
   for label,v in [('malformed_json','{'),('wrong_json_top','[]'),('unknown_json_key','{"unexpected":true}'),('wrong_inner_json_type','{"context":{"mode":"set","value":{"enabled":"bad"}}}')]:cases.append((label,{name:v}))
 return cases

def build_registry(root:Path=ROOT)->dict:
 fixture=fixture_template(root);tables=_table_sql(root);contract=schema.design_contract(root);prior=schema.v3.design_contract(root);probes=[]
 for identifier,(table,column)in schema.field_rows(contract,prior).items():
  control=fixture['tables'][table]['field_controls'][column['name']]
  for label,changes in _mutations(column,control):
   with closing(_projection(tables[table]))as reference:expected=_outcome(reference,table,control|changes)
   probes.append({'id':'V4-PROBE:'+table+'.'+column['name']+'.'+label,'field_id':identifier,'table':table,'label':label,'control':control,'changes':changes,'expected':expected,'expected_source':'exact independently frozen design SQL, never candidate/production','rust_obligation':('strict_typed_json_inner_grammar'if 'json'in label else'reject_nul'if label=='nul'else'revalidate_u32'if label=='u32_overflow'else None)})
 old_controls=schema.read_json(root,'docs/architecture/database/v3/boundary-fixtures.json')['tables']
 expanded=[
  ('config_revisions','codec_version',fixture['tables']['config_revisions']['field_controls']['revision_role'],[('new3',{'codec_version':3}),('future4',{'codec_version':4})]),
  ('connections','auth_mode',fixture['tables']['connections']['field_controls']['origin_json']|{'protocol':'google_translate_v3','http_options_json':'{"protocol":"google_translate_v3","projectId":"project","location":"global"}','provider_preset_id':None,'provider_preset_revision':None},[('google_oauth',{'auth_mode':'google_oauth'}),('unknown',{'auth_mode':'headers'})]),
  ('credential_refs','auth_kind',old_controls['credential_refs'],[('google_oauth_refresh',{'auth_kind':'google_oauth_refresh'}),('unknown',{'auth_kind':'headers'})]),
  ('provider_preset_versions','auth_kind',old_controls['provider_preset_versions'],[('google_oauth',{'auth_kind':'google_oauth'}),('unknown',{'auth_kind':'headers'})]),
  ('binding_engines','model_id',fixture['tables']['binding_engines']['valid_variants'][0],[('http_missing',{'target_kind':'http','model_id':None}),('offline_xor',{'target_kind':'offline','model_id':None,'offline_engine_config_id':'offline','template_id':None})]),
 ]
 for table,name,control,cases in expanded:
  for label,changes in cases:
   with closing(_projection(tables[table]))as reference:expected=_outcome(reference,table,control|changes)
   probes.append({'id':'V4-OLD-FIELD:'+table+'.'+name+'.'+label,'field_id':None,'table':table,'label':'expanded_old_field','control':control,'changes':changes,'expected':expected,'expected_source':'frozen schema4 independent design; original negatives retained on old fixtures','rust_obligation':'explicit_reviewed_old_enum_or_HTTP_offline_XOR'})
 groups={
 'connections':[['http_options_schema','http_options_json'],['origin_schema','origin_json'],['oauth_grant_id','oauth_authorization_epoch']],
 'task_execution_targets':[['selection_scope','selection_revision','effective_scope','effective_selection_revision']],
 'offline_resource_observations':[['installation_id','installation_revision'],['local_grant_id','local_grant_epoch']],
 'offline_engine_configs':[['installation_id','installation_revision'],['snapshot_id','route_id']],
 'execution_authorization_leases':[['grant_id','authorization_epoch','material_revision'],['pool_id','pool_revision','member_id'],['attempt_id','task_id']],
 }
 for table,collections in groups.items():
  for group in collections:
   control=fixture['tables'][table]['field_controls'][group[0]]
   for mask in range(1<<len(group)):
    changes={name:control[name]if mask&(1<<index)else None for index,name in enumerate(group)}
    with closing(_projection(tables[table]))as reference:expected=_outcome(reference,table,control|changes)
    probes.append({'id':'V4-COMBINATION:'+table+'.'+','.join(group)+'.'+str(mask),'field_id':None,'table':table,'label':'nullable_tuple_mask','control':control,'changes':changes,'expected':expected,'expected_source':'independently frozen design SQL','rust_obligation':'closed_nullable_reference_and_target_union'})
 # Exhaust all eight nullable shape components for the new internal connection-only
 # proof, retaining every existing mask ID and its HTTP/offline controls unchanged.
 table='activation_resource_materializations';group=['effective_model_id','source_model_id','effective_offline_config_id','source_offline_config_id','effective_connection_id','source_connection_id','control_connection_id','control_config_id']
 control=next(r for r in fixture['tables'][table]['valid_variants']if r['target_kind']=='connection')
 columns={r['name']:r['type']for r in contract['databases']['profile']['tables'][table]['columns']}
 for mask in range(256):
  changes={name:(control[name]if control[name]is not None else _value(name,columns[name],tables[table]))if mask&(1<<index)else None for index,name in enumerate(group)}
  with closing(_projection(tables[table]))as reference:expected=_outcome(reference,table,control|changes)
  probes.append({'id':'V4-R4-COMBINATION:connection-only:'+str(mask),'field_id':None,'table':table,'label':'connection_only_nullable_mask','control':control,'changes':changes,'expected':expected,'expected_source':'independently frozen R4 design SQL','rust_obligation':'connection_only_is_private_provenance_never_execution_target'})
 return{'schema_version':1,'kind':'schema_four_boundary_registry','source_design_sha256':schema.DESIGN_SHA256,'projection':PROJECTION,'not_product_acceptance':True,'field_ids':sorted(schema.field_rows(contract,prior)),'expanded_old_fields':schema.EXPANDED_OLD_FIELDS,'relationship_inventory':schema.relationship_rows(contract,prior),'semantic_probe_ids':SEMANTIC_PROBE_IDS,'trigger_runtime_obligations':schema.trigger_rows(contract,prior),'view_runtime_obligations':schema.view_rows(contract,prior),'probe_count':len(probes),'field_probe_count':sum(p['field_id']is not None for p in probes),'combination_probe_count':sum(p['label']=='nullable_tuple_mask'for p in probes),'expanded_old_probe_count':sum(p['label']=='expanded_old_field'for p in probes),'R4_connection_only_mask_count':sum(p['label']=='connection_only_nullable_mask'for p in probes),'probes':probes,'relationship_projection':'one exact FK isolated with real child CHECKs, parent key identity only; full semantic trigger probes separately','runtime_policy':'SQL accepts do not grant Rust producer or runtime eligibility; required typed/authority checks remain pending'}

def _field_probe(sql:str,probe:dict)->list[str]:
 with closing(_projection(sql))as db:
  if _outcome(db,probe['table'],probe['control'])!={'outcome':'accept'}:return['positive control rejected before mutation']
  actual=_outcome(db,probe['table'],probe['control']|probe['changes'])
  if actual!=probe['expected']:return['boundary differs: expected '+str(probe['expected'])+' actual '+str(actual)]
  if _outcome(db,probe['table'],probe['control'])!={'outcome':'accept'}:return['post-negative positive control failed']
 return[]

def assess(root:Path=ROOT,candidate:Path|None=None)->dict:
 errors=[];results=[]
 try:
  errors+=schema.validate_schema_contract(root)
  expected=build_registry(root);errors+=schema.v1._diff(expected,schema.read_json(root,REGISTRY),'v4_boundaries');errors+=schema.v1._diff(fixture_template(root),schema.read_json(root,FIXTURES),'v4_fixtures')
  before=None
  if candidate:
   errors+=schema.compare_current_schema(root,candidate)
   if not errors:
    before=hashlib.sha256(candidate.read_bytes()).hexdigest()
    with closing(sqlite3.connect(candidate.resolve().as_uri()+'?mode=ro&immutable=1',uri=True))as db:
     db.set_authorizer(_authorizer);tables=dict(db.execute("SELECT name,sql FROM sqlite_schema WHERE type='table'"))
  else:tables=_table_sql(root)
  if not errors:
   for probe in expected['probes']:
    failures=_field_probe(tables[probe['table']],probe);results.append({'id':probe['id'],'status':'failed'if failures else'passed','assertions':3,'errors':failures})
   # Full semantic and isolated FK results are added by the following reviewable helpers.
   relationship_results=relationship_checks(root);semantic_results=semantic_checks(root)
   expected_relations={r['id']for r in expected['relationship_inventory']}
   relation_ids=[r['id']for r in relationship_results];semantic_ids=[r['id']for r in semantic_results]
   if len(relation_ids)!=len(expected_relations)or set(relation_ids)!=expected_relations:errors.append('missing, duplicate or unexpected actual FK projection result')
   if len(semantic_ids)!=len(expected['semantic_probe_ids'])or set(semantic_ids)!=set(expected['semantic_probe_ids']):errors.append('missing, duplicate or unexpected actual complete-design semantic result')
   relation_by_id={r['id']:r for r in expected['relationship_inventory']}
   for row in relationship_results:errors+=schema.v1._diff(relation_by_id.get(row['id']),row.get('relationship'),'actual_FK_projection_identity.'+row['id'])
   results+=relationship_results+semantic_results
   errors +=[r['id']+': '+'; '.join(r['errors'])for r in results if r['errors']]
   if candidate:
    errors+=schema.v1.sealed_snapshot_errors(candidate)
    if hashlib.sha256(candidate.read_bytes()).hexdigest()!=before:errors.append('candidate changed during read-only inspection')
 except(OSError,ValueError,TypeError,KeyError,RecursionError,sqlite3.Error)as e:errors.append('schema-four boundaries: '+str(e))
 return{'kind':'schema_four_boundary_receipt','status':'failed'if errors else'passed','scope':'independent SQL projections and design semantic probes only','not_product_acceptance':True,'candidate_readonly':candidate is not None,'checks':results,'errors':errors}

def _elements(sql:str)->list[str]:
 tokens=schema.v1.sql_tokens(sql);start=tokens.index('(');depth=0;parts=[];current=[]
 for token in tokens[start+1:]:
  if token==')'and depth==0:
   if current:parts.append(' '.join(current))
   break
  if token==','and depth==0:parts.append(' '.join(current));current=[];continue
  current.append(token)
  if token=='(':depth+=1
  elif token==')':depth-=1
 return parts

def _strip_reference(element:str)->str:
 # Token grammar, not arbitrary regex deletion from quoted SQL literals.
 tokens=schema.v1.sql_tokens(element)
 if 'references'not in tokens:return element
 i=tokens.index('references');j=i+2
 if j<len(tokens)and tokens[j]=='(':
  depth=1;j+=1
  while depth:
   if tokens[j]=='(':depth+=1
   elif tokens[j]==')':depth-=1
   j+=1
 while j<len(tokens):
  if tokens[j]=='on':
   j+=2
   if tokens[j]in{'set','no'}:j+=2
   else:j+=1
  elif tokens[j]=='match':j+=2
  elif tokens[j]=='deferrable':j+=1
  elif tokens[j:j+2]==['not','deferrable']:j+=2
  elif tokens[j]=='initially':j+=2
  else:break
 return ' '.join(tokens[:i]+tokens[j:])

def _fk_projection(sql:str,table:str,fk:dict,parent_types:dict):
 elements=[_strip_reference(e)for e in _elements(sql)if not e.lstrip().lower().startswith('foreign key')]
 columns=','.join(map(ident,fk['columns']));parents=','.join(map(ident,fk['parent_columns']))
 elements.append('FOREIGN KEY('+columns+') REFERENCES parent_projection('+parents+') ON DELETE '+fk['on_delete']+' ON UPDATE '+fk['on_update'])
 db=sqlite3.connect(':memory:');db.set_authorizer(_authorizer);db.execute('PRAGMA foreign_keys=ON')
 parent='CREATE TABLE parent_projection('+','.join(ident(n)+' '+parent_types[n]+' NOT NULL'for n in fk['parent_columns'])+',PRIMARY KEY('+parents+')) STRICT'
 db.execute(parent);db.execute('CREATE TABLE '+ident(table)+'('+','.join(elements)+') STRICT')
 return db

def _fk_control(table:str,fk:dict,fixtures:dict,sql:str,columns:dict)->dict:
 values=fixtures['tables'].get(table)
 if values is None:raise ValueError('FK child has no v4 fixture: '+table)
 variants=values['valid_variants']+list(values['field_controls'].values())
 for base in variants:
  row=base|{name:base[name]if base[name]is not None else _value(name,columns[name],sql)for name in fk['columns']}
  with closing(_projection(sql))as d:
   if _outcome(d,table,row)=={'outcome':'accept'}:return row
 raise ValueError('no non-null fixture for isolated FK: '+table+str(fk))

def relationship_checks(root:Path)->list[dict]:
 contract=schema.design_contract(root);prior=schema.v3.design_contract(root);tables=contract['databases']['profile']['tables'];sqls=_table_sql(root);fixtures=fixture_template(root);results=[]
 for item in schema.relationship_rows(contract,prior):
  table=item['table'];fk=item['foreign_key'];errors=[];assertions=0
  types={c['name']:c['type']for c in tables[table]['columns']};parent_types={c['name']:c['type']for c in tables[fk['parent']]['columns']}
  control=_fk_control(table,fk,fixtures,sqls[table],types)
  parent={name:control[child]for name,child in zip(fk['parent_columns'],fk['columns'])}
  with closing(_fk_projection(sqls[table],table,fk,parent_types))as d:
   _insert(d,'parent_projection',parent);d.commit();out=_outcome(d,table,control);assertions+=1
   if out!={'outcome':'accept'}:errors.append('exact positive parent key did not satisfy isolated FK: '+str(out))
  with closing(_fk_projection(sqls[table],table,fk,parent_types))as d:
   out=_outcome(d,table,control);assertions+=1
   if out!={'outcome':'reject','sqlite_errorcode':sqlite3.SQLITE_CONSTRAINT_FOREIGNKEY}:errors.append('missing-parent FK rejection masked or absent: '+str(out))
  for name in fk['parent_columns']:
   wrong=parent|{name:parent[name]+1 if isinstance(parent[name],int)else str(parent[name])+'.wrong-owner'}
   with closing(_fk_projection(sqls[table],table,fk,parent_types))as d:
    _insert(d,'parent_projection',wrong);d.commit();out=_outcome(d,table,control);assertions+=1
    if out!={'outcome':'reject','sqlite_errorcode':sqlite3.SQLITE_CONSTRAINT_FOREIGNKEY}:errors.append('wrong owner/component accepted: '+name+' '+str(out))
  with closing(_fk_projection(sqls[table],table,fk,parent_types))as d:
   _insert(d,'parent_projection',parent);_insert(d,table,control);d.commit();failure=None
   try:d.execute('DELETE FROM parent_projection');d.commit()
   except sqlite3.IntegrityError as e:d.rollback();failure=e
   assertions+=1
   if fk['on_delete']in{'RESTRICT','NO ACTION'}and (failure is None or failure.sqlite_errorcode not in {sqlite3.SQLITE_CONSTRAINT_FOREIGNKEY,sqlite3.SQLITE_CONSTRAINT_TRIGGER}):errors.append('parent delete did not enforce '+fk['on_delete'])
   if fk['on_delete']=='CASCADE'and(failure is not None or d.execute('SELECT count(*) FROM '+ident(table)).fetchone()[0]!=0):errors.append('CASCADE did not delete exact child')
   if fk['on_delete']=='SET NULL'and failure is not None:errors.append('SET NULL action failed with retained child constraints')
  results.append({'id':item['id'],'relationship':copy.deepcopy(item),'status':'failed'if errors else'passed','assertions':assertions,'errors':errors,'scope':'isolated exact FK with child CHECKs; no unrelated FK can hide orphan or wrong-owner failure'})
 return results
def semantic_checks(root:Path)->list[dict]:
 results=[]
 def record(identifier,passed,detail=None):
  results.append({'id':'V4-SEMANTIC:'+identifier,'status':'passed'if passed else'failed','assertions':1,'errors':[]if passed else[str(detail or 'required semantic assertion failed')],'scope':'complete frozen independent design with actual FK and triggers'})
 def reject(db,identifier,sql,args=(),message=None):
  db.execute('SAVEPOINT semantic_probe')
  try:db.execute(sql,args)
  except sqlite3.IntegrityError as error:record(identifier,message is None or message in str(error),str(error))
  else:record(identifier,False,'unexpected accepted')
  finally:db.execute('ROLLBACK TO semantic_probe');db.execute('RELEASE semantic_probe')
 def config(db,revision,model,connection,version=2):
  _insert(db,'config_revisions',{'revision_id':revision,'content_hash':'a'*64,'created_at_ms':0,'save_reason':'create','local_validation_json':'{}','codec_version':version})
  _insert(db,'connections',{'revision_id':revision,'connection_id':connection,'display_name':'fixture','protocol':'openai_compatible','deployment':'remote','auth_mode':'none','endpoint':'https://example.invalid','proxy_mode':'inherit','network_metadata_json':'{}','availability_intent':0})
  _insert(db,'models',{'revision_id':revision,'model_id':model,'connection_id':connection,'actual_model_id':'fixture','display_name':'fixture','selection_source':'manual','capability_json':'{}'})
 try:
  with schema.design_database(root)as db:
   for client in ['client','other']:_insert(db,'oauth_client_profiles',{'client_profile_id':client,'client_profile_revision':1,'issuer':'google_accounts','public_client_id':'synthetic-client','client_mode':'custom','redirect_policy':'loopback_pkce','policy_sha256':'a'*64,'setup_state':'verified','created_at_ms':0})
   for grant in ['grant-a','grant-b']:_insert(db,'oauth_grants',{'grant_id':grant,'client_profile_id':'client','client_profile_revision':1,'principal_state':'unknown','created_at_ms':0})
   _insert(db,'credential_refs',{'credential_id':'owned-refresh','vault_ref':'synthetic-vault-ref','masked_label':'fixture','auth_kind':'google_oauth_refresh','state':'active','created_at_ms':0,'activated_at_ms':0,'lifecycle_revision':1,'envelope_schema':1})
   _insert(db,'oauth_token_material_revisions',{'grant_id':'grant-a','material_revision':1,'credential_id':'owned-refresh','auth_kind':'google_oauth_refresh','created_at_ms':0})
   reject(db,'owned-refresh-cannot-cross-grant',"INSERT INTO oauth_token_material_revisions VALUES('grant-b',1,'owned-refresh','google_oauth_refresh',NULL,0)",message='UNIQUE')
   _insert(db,'oauth_authorization_snapshots',{'grant_id':'grant-a','authorization_epoch':1,'state':'active','scopes_sha256':'b'*64,'created_at_ms':0})
   db.execute("UPDATE oauth_grants SET authorization_epoch=1,material_revision=1,control_revision=1 WHERE grant_id='grant-a' AND control_revision=0")
   record('grant-cursor-owned-activation',tuple(db.execute("SELECT authorization_epoch,material_revision,control_revision FROM oauth_grants WHERE grant_id='grant-a'").fetchone())==(1,1,1))
   _insert(db,'oauth_authorization_snapshots',{'grant_id':'grant-a','authorization_epoch':2,'state':'active','scopes_sha256':'b'*64,'previous_epoch':1,'created_at_ms':1})
   reject(db,'grant-stale-CAS',"UPDATE oauth_grants SET authorization_epoch=2,control_revision=1 WHERE grant_id='grant-a'",message='stale_or_changed')
   reject(db,'grant-client-immutable',"UPDATE oauth_grants SET client_profile_id='other',authorization_epoch=2,control_revision=2 WHERE grant_id='grant-a'",message='stale_or_changed')
   reject(db,'grant-initial-authority-forbidden',"INSERT INTO oauth_grants(grant_id,client_profile_id,client_profile_revision,principal_state,created_at_ms,authorization_epoch) VALUES('new','client',1,'unknown',0,1)",message='pending')
   reject(db,'snapshot-fixed-scope',"INSERT INTO oauth_authorization_snapshots(grant_id,authorization_epoch,state,scope,scopes_sha256,created_at_ms) VALUES('grant-b',1,'pending','cloud-platform',?,0)",('b'*64,),message='CHECK')
   reject(db,'snapshot-immutable',"UPDATE oauth_authorization_snapshots SET state='revoked' WHERE grant_id='grant-a'",message='immutable')
   reject(db,'refresh-immutable',"UPDATE oauth_token_material_revisions SET material_revision=2",message='immutable')
   reject(db,'lease-no-fake-target',"INSERT INTO execution_authorization_leases(lease_id,origin_kind,grant_id,authorization_epoch,material_revision,credential_id,auth_kind,lifecycle_revision,scopes_sha256,client_policy_sha256,target_policy_sha256,minted_at_ms,expires_at_ms) VALUES('lease','user_oauth','grant-a',1,1,'owned-refresh','google_oauth_refresh',1,?,?,?,0,10)",('a'*64,)*3,message='CHECK')
   reject(db,'lease-real-test-required',"INSERT INTO execution_authorization_leases(lease_id,test_id,origin_kind,grant_id,authorization_epoch,material_revision,credential_id,auth_kind,lifecycle_revision,scopes_sha256,client_policy_sha256,target_policy_sha256,minted_at_ms,expires_at_ms) VALUES('lease','absent','user_oauth','grant-a',1,1,'owned-refresh','google_oauth_refresh',1,?,?,?,0,10)",('a'*64,)*3,message='FOREIGN KEY')
   record('oauth-final-fk-clean',not db.execute('PRAGMA foreign_key_check').fetchall())
 except(sqlite3.Error,ValueError,AssertionError)as error:record('oauth-setup',False,error)
 try:
  with schema.design_database(root)as db:
   config(db,'旧 配置','旧 模型','旧 连接');config(db,'other','model','other-control');config(db,'derived','alias-model','alias-connection',3)
   for r in ['旧 配置','other']:_insert(db,'config_revision_seals',{'revision_id':r,'content_hash':'a'*64,'sealed_at_ms':0})
   _insert(db,'connection_runtime_states',{'connection_id':'旧 连接','config_revision_id':'旧 配置','is_suspended':1,'control_revision':1,'reason_code':'stopped','updated_at_ms':0})
   _insert(db,'connection_runtime_states',{'connection_id':'other-control','config_revision_id':'other','is_suspended':0,'control_revision':1,'updated_at_ms':0})
   row={'materialization_id':'map','effective_revision_id':'derived','target_kind':'http','effective_model_id':'alias-model','source_revision_id':'旧 配置','source_model_id':'旧 模型','reason':'legacy_materialization','effective_connection_id':'alias-connection','source_connection_id':'旧 连接','control_connection_id':'旧 连接'}
   _insert(db,'activation_resource_materializations',row)
   record('opaque-source-identity-kept',True)
   record('source-stable-control-stays-stopped',db.execute("SELECT gate.is_suspended FROM activation_resource_materializations m JOIN connection_runtime_states gate ON gate.connection_id=m.control_connection_id WHERE materialization_id='map'").fetchone()[0]==1)
   reject(db,'alias-cannot-borrow-live-control',"INSERT INTO activation_resource_materializations(materialization_id,effective_revision_id,target_kind,effective_model_id,source_revision_id,source_model_id,reason,effective_connection_id,source_connection_id,control_connection_id) VALUES('bad','derived','http','alias-model','旧 配置','旧 模型','legacy_materialization','alias-connection','旧 连接','other-control')",message='control_bypass')
   _insert(db,'config_revision_seals',{'revision_id':'derived','content_hash':'a'*64,'sealed_at_ms':0})
   reject(db,'source-materialization-immutable',"UPDATE activation_resource_materializations SET source_model_id='changed' WHERE materialization_id='map'",message='immutable')
   _insert(db,'config_heads',{'head_kind':'draft','revision_id':'other','head_revision':0,'updated_at_ms':0});_insert(db,'config_heads',{'head_kind':'applied','revision_id':'旧 配置','head_revision':0,'updated_at_ms':0});db.commit()
   db.execute('BEGIN IMMEDIATE');db.execute("UPDATE config_heads SET revision_id='derived',head_revision=1,updated_at_ms=1 WHERE head_kind='applied' AND head_revision=0");db.commit()
   record('activation-preserves-draft',db.execute("SELECT revision_id FROM config_heads WHERE head_kind='draft'").fetchone()[0]=='other')
   db.execute('BEGIN IMMEDIATE');db.execute("UPDATE config_heads SET revision_id='other',head_revision=2,updated_at_ms=2 WHERE head_kind='applied' AND head_revision=1")
   try:db.execute("INSERT INTO offline_local_grants VALUES('bad',?,'fixture',0,'unknown',0)",('a'*64,))
   except sqlite3.IntegrityError:db.rollback()
   else:record('selector-transaction-rollback',False)
   record('selector-transaction-rollback',db.execute("SELECT revision_id,head_revision FROM config_heads WHERE head_kind='applied'").fetchone()==('derived',1))
   reject(db,'source-GC-referenced-config',"DELETE FROM config_revisions WHERE revision_id='旧 配置'",message='FOREIGN KEY')
   config(db,'policy','policy-model','policy-connection',3)
   reject(db,'typed-policy-no-legacy-shadow',"INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,purpose,context_json,semantics_version) VALUES('policy','b','global','translation','{\"enabled\":false}',3)",message='legacy_shadow')
   reject(db,'scope-policy-unknown-key',"INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,purpose,policy_overrides_json) VALUES('policy','b','global','translation','{\"unknown\":1}')",message='unknown_policy_override')
   db.execute("INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,purpose,context_json,semantics_version) VALUES('policy','legacy','global','translation','{\"mode\":\"translation\"}',2)")
   record('legacy-policy-original-bytes',db.execute("SELECT context_json FROM purpose_bindings WHERE binding_id='legacy'").fetchone()[0]=='{"mode":"translation"}')
 except(sqlite3.Error,ValueError,AssertionError)as error:record('source-control-setup',False,error)
 try:
  with schema.design_database(root)as db:
   for runtime in ['runtime-a','runtime-b']:_insert(db,'offline_runtime_profiles',{'runtime_profile_id':runtime,'runtime_profile_revision':1,'engine_id':'argos_translate','engine_contract_revision':1,'backend':'local_worker','platform':'macos','architecture':'arm64','private_protocol_revision':1,'abi_profile_id':'pinned','verification_state':'verified','policy_sha256':'a'*64,'created_at_ms':0})
   _insert(db,'offline_execution_environments',{'environment_id':'environment','environment_revision':1,'platform':'macos','architecture':'arm64','machine_fingerprint':'a'*64,'os_build':'synthetic','created_at_ms':0})
   _insert(db,'offline_local_grants',{'grant_id':'local','machine_fingerprint':'a'*64,'opaque_locator':'synthetic','authorization_epoch':1,'state':'active','created_at_ms':0})
   installation={'installation_id':'installation','installation_revision':1,'engine_id':'argos_translate','engine_contract_revision':1,'environment_id':'environment','environment_revision':1,'local_native_path':'/synthetic','local_grant_id':'local','runtime_profile_id':'runtime-a','runtime_profile_revision':1,'created_at_ms':0};_insert(db,'offline_installation_versions',installation)
   reject(db,'installation-engine-runtime-match',"INSERT INTO offline_installation_versions VALUES('bad',1,'kingsoft',1,'environment',1,'/synthetic','local','runtime-a',1,0)",message='FOREIGN KEY')
   _insert(db,'offline_execution_environments',{'environment_id':'wrong-arch','environment_revision':1,'platform':'macos','architecture':'x86','machine_fingerprint':'a'*64,'os_build':'synthetic','created_at_ms':0})
   reject(db,'installation-environment-architecture',"INSERT INTO offline_installation_versions VALUES('bad',1,'argos_translate',1,'wrong-arch',1,'/synthetic','local','runtime-a',1,0)",message='environment_runtime_mismatch')
   def resource(snapshot,runtime,source,target):
    _insert(db,'offline_resource_snapshots',{'snapshot_id':snapshot,'engine_id':'argos_translate','engine_contract_revision':1,'backend':'local_worker','runtime_profile_id':runtime,'runtime_profile_revision':1,'identity_kind':'file_manifest','manifest_sha256':'a'*64,'content_hash':'b'*64,'created_at_ms':0})
    _insert(db,'offline_resource_snapshot_files',{'snapshot_id':snapshot,'relative_key':'engine.bin','byte_length':1,'sha256':'c'*64,'role':'engine'})
    _insert(db,'offline_resource_pairs',{'snapshot_id':snapshot,'source_language':source,'target_language':target,'pipeline_profile_id':'pipe'})
    _insert(db,'offline_resource_seals',{'snapshot_id':snapshot,'content_hash':'b'*64,'sealed_at_ms':0})
   resource('ja-en','runtime-a','ja','en');resource('en-zh','runtime-b','en','zh-Hans');resource('same-en-zh','runtime-a','en','zh-Hans')
   def route(route,kind,source,target,hops):
    _insert(db,'offline_translation_routes',{'route_id':route,'engine_id':'argos_translate','engine_contract_revision':1,'backend':'local_worker','route_kind':kind,'source_language':source,'target_language':target,'consent_revision':1 if kind=='explicit_pivot'else None,'content_hash':'d'*64,'created_at_ms':0})
    for index,(snapshot,src,dst)in enumerate(hops):_insert(db,'offline_translation_route_hops',{'route_id':route,'hop_ordinal':index,'snapshot_id':snapshot,'engine_id':'argos_translate','engine_contract_revision':1,'backend':'local_worker','source_language':src,'target_language':dst,'pipeline_profile_id':'pipe'})
   route('mixed','explicit_pivot','ja','zh-Hans',[('ja-en','ja','en'),('en-zh','en','zh-Hans')])
   reject(db,'pivot-no-mixed-runtime',"INSERT INTO offline_route_seals VALUES('mixed',?,0)",('d'*64,),message='mixed_runtime_route')
   route('valid-pivot','explicit_pivot','ja','zh-Hans',[('ja-en','ja','en'),('same-en-zh','en','zh-Hans')]);_insert(db,'offline_route_seals',{'route_id':'valid-pivot','content_hash':'d'*64,'sealed_at_ms':0});record('pivot-explicit-same-runtime-valid',True)
   reject(db,'sealed-resource-file-insert',"INSERT INTO offline_resource_snapshot_files VALUES('ja-en','extra.bin',1,?,'engine')",('c'*64,),message='sealed_service_closure')
   reject(db,'sealed-route-hop-delete',"DELETE FROM offline_translation_route_hops WHERE route_id='valid-pivot'",message='sealed_service_closure')
   reject(db,'resource-GC-reference',"DELETE FROM offline_resource_seals WHERE snapshot_id='ja-en'",message='resource_still_referenced')
   _insert(db,'config_revisions',{'revision_id':'offline','content_hash':'a'*64,'created_at_ms':0,'save_reason':'create','local_validation_json':'{}','codec_version':3})
   cfg={'revision_id':'offline','engine_config_id':'offline','engine_id':'argos_translate','engine_contract_revision':1,'backend':'local_worker','display_name':'synthetic','source_language':'ja','target_language':'zh-Hans','route_policy':'explicit_pivot','consent_revision':1,'installation_id':'installation','installation_revision':1,'resolution':'resolved','snapshot_id':'ja-en','route_id':'valid-pivot'};_insert(db,'offline_engine_configs',cfg)
   reject(db,'route-GC-reference',"DELETE FROM offline_route_seals WHERE route_id='valid-pivot'",message='route_still_referenced')
   reject(db,'config-installation-resource-runtime',"INSERT INTO offline_engine_configs VALUES('offline','wrong','argos_translate',1,'local_worker','fixture','ja','zh-Hans','explicit_pivot',1,'installation',1,'resolved','en-zh','valid-pivot')",message='runtime_mismatch')
   record('offline-full-graph-FK-clean',not db.execute('PRAGMA foreign_key_check').fetchall())
 except(sqlite3.Error,ValueError,AssertionError)as error:record('offline-runtime-setup',False,error)
 # R2 compatibility: new main/comparison dedup applies per typed binding, never retroactively to old slots.
 for label,codec,semantics,aliases,distinct,mixed,typed_two,should_accept in [
  ('legacy-codec1-duplicate-disabled',1,2,False,False,False,True,True),
  ('legacy-codec2-duplicate-disabled',2,2,False,False,False,True,True),
  ('codec3-LegacyV2-source-duplicate',3,2,True,False,False,True,True),
  ('codec3-TypedV3-direct-duplicate',3,3,False,False,False,True,False),
  ('codec3-TypedV3-alias-duplicate',3,3,True,False,False,True,False),
  ('codec3-TypedV3-distinct',3,3,False,True,False,True,True),
  ('codec3-TypedV3-distinct-sources',3,3,True,True,False,True,True),
  ('mixed-legacy-duplicate-typed-distinct',3,3,True,False,True,False,True),
  ('mixed-legacy-cannot-hide-typed-duplicate',3,3,True,False,True,True,False),
 ]:
  try:
   with schema.design_database(root)as db:
    if aliases:
     config(db,'old-source','source-model','source-connection',2)
     _insert(db,'models',{'revision_id':'old-source','model_id':'source-other','connection_id':'source-connection','display_name':'fixture','selection_source':'manual','capability_json':'{}'})
     _insert(db,'config_revision_seals',{'revision_id':'old-source','content_hash':'a'*64,'sealed_at_ms':0})
     _insert(db,'connection_runtime_states',{'connection_id':'source-connection','config_revision_id':'old-source','is_suspended':0,'control_revision':1,'updated_at_ms':0})
    _insert(db,'config_revisions',{'revision_id':'compat','content_hash':'a'*64,'created_at_ms':0,'save_reason':'create','local_validation_json':'{}','codec_version':codec})
    connection={'revision_id':'compat','connection_id':'connection','display_name':'fixture','protocol':'openai_compatible','deployment':'remote','auth_mode':'none','proxy_mode':'inherit','network_metadata_json':'{}','availability_intent':0}
    if codec==3 and not aliases:connection.update(adapter_binding_schema=1,adapter_contract_revision=1,compatibility_profile_id='openai-chat-text',compatibility_profile_revision=1,http_options_schema=1,http_options_json='{"protocol":"openai_compatible"}',origin_schema=1,origin_json='{"kind":"custom"}')
    _insert(db,'connections',connection)
    for model in ['model','other']:
     row={'revision_id':'compat','model_id':model,'connection_id':'connection','display_name':'fixture','selection_source':'manual','capability_json':'{}'}
     if codec==3 and not aliases:row['template_purpose']='translation'
     _insert(db,'models',row)
     if aliases:_insert(db,'activation_resource_materializations',{'materialization_id':'map-'+model,'effective_revision_id':'compat','target_kind':'http','effective_model_id':model,'source_revision_id':'old-source','source_model_id':'source-other'if distinct and model=='other'else'source-model','reason':'legacy_materialization','effective_connection_id':'connection','source_connection_id':'source-connection','control_connection_id':'source-connection'})
    _insert(db,'purpose_bindings',{'revision_id':'compat','binding_id':'b','scope_key':'global','purpose':'translation','binding_mode':'override','enabled':1,'semantics_version':semantics})
    _insert(db,'binding_engines',{'revision_id':'compat','binding_id':'b','engine_id':'first','purpose':'translation','model_id':'model','position':0,'enabled':1})
    if typed_two:_insert(db,'binding_engines',{'revision_id':'compat','binding_id':'b','engine_id':'second','purpose':'translation','model_id':'other'if aliases or distinct else'model','position':1,'enabled':0})
    if mixed:
     _insert(db,'games',{'game_id':'legacy-game','display_name':'fixture','row_revision':1,'created_at_ms':0,'updated_at_ms':0})
     _insert(db,'purpose_bindings',{'revision_id':'compat','binding_id':'old','scope_key':'game:legacy-game','game_id':'legacy-game','purpose':'translation','binding_mode':'override','enabled':1,'semantics_version':2})
     for pos in [0,1]:_insert(db,'binding_engines',{'revision_id':'compat','binding_id':'old','engine_id':'old-'+str(pos),'purpose':'translation','model_id':'model','position':pos,'enabled':1 if pos==0 else 0})
    if should_accept:
     _insert(db,'config_revision_seals',{'revision_id':'compat','content_hash':'a'*64,'sealed_at_ms':0})
     record('dedup-'+label,not db.execute('PRAGMA foreign_key_check').fetchall())
    else:reject(db,'dedup-'+label,"INSERT INTO config_revision_seals VALUES('compat',?,0)",('a'*64,),message='duplicate_logical_service')
  except(sqlite3.Error,ValueError,AssertionError)as error:record('dedup-'+label,False,error)
 # R3 origin.site is a non-secret optional text; presence cannot accept other JSON types/NUL.
 try:
  with schema.design_database(root)as db:
   _insert(db,'config_revisions',{'revision_id':'origin-site','content_hash':'a'*64,'created_at_ms':0,'save_reason':'create','local_validation_json':'{}','codec_version':3})
   preset=schema.read_json(root,'docs/architecture/database/v3/boundary-fixtures.json')['tables']['provider_preset_versions'];_insert(db,'provider_preset_versions',preset)
   count=0
   def site_connection(origin):
    nonlocal count
    count+=1;row={'revision_id':'origin-site','connection_id':'origin-'+str(count),'display_name':'fixture','protocol':'openai_compatible','deployment':'cloud','auth_mode':'none','proxy_mode':'inherit','network_metadata_json':'{}','availability_intent':0,'http_options_schema':1,'http_options_json':'{"protocol":"openai_compatible"}','origin_schema':1,'origin_json':json.dumps(origin,ensure_ascii=False,separators=(',',':'))}
    if origin['kind']=='preset':row.update(adapter_binding_schema=1,adapter_contract_revision=1,compatibility_profile_id='profile',compatibility_profile_revision=1,provider_preset_id='user.fixture',provider_preset_revision=1)
    return row
   for kind in ['custom','preset']:
    base={'kind':kind}
    if kind=='preset':base['key']={'presetId':'user.fixture','presetRevision':1}
    for label,origin in [('absent',base),('text',base|{'site':'dashscope_intl'})]:
     row=site_connection(origin)
     try:
      _insert(db,'connections',row)
      record('origin-site-'+kind+'-'+label,db.execute('SELECT origin_json FROM connections WHERE revision_id=? AND connection_id=?',('origin-site',row['connection_id'])).fetchone()[0]==row['origin_json'])
     except(sqlite3.Error,ValueError,AssertionError)as error:record('origin-site-'+kind+'-'+label,False,error)
    for label,value in [('null',None),('boolean',False),('integer',1),('fraction',1.5),('array',[]),('object',{}),('nul','dashscope_intl\0tail')]:
     row=site_connection(base|{'site':value});names=list(row);sql='INSERT INTO connections('+','.join(map(ident,names))+')VALUES('+','.join('?'for _ in names)+')'
     reject(db,'origin-site-'+kind+'-reject-'+label,sql,tuple(row.values()),message='http_origin_mismatch')
    row=site_connection(base|{'site':'dashscope_intl','unexpected':'blocked'});names=list(row);sql='INSERT INTO connections('+','.join(map(ident,names))+')VALUES('+','.join('?'for _ in names)+')'
    reject(db,'origin-site-'+kind+'-reject-unknown-key',sql,tuple(row.values()),message='http_origin_mismatch')
 except(sqlite3.Error,ValueError,AssertionError)as error:record('origin-site-setup',False,error)
 # Payload carrier checks are intentionally narrower than the Rust typed decoder.
 try:
  with schema.design_database(root)as db:
   _insert(db,'config_revisions',{'revision_id':'payload','content_hash':'a'*64,'created_at_ms':0,'save_reason':'create','local_validation_json':'{}','codec_version':3})
   protocols={'openai_compatible':{'organizationId':'fixture','projectId':'project'},'anthropic':{'workspaceId':'workspace'},'azure_translator':{'resourceType':'global'},'openai_responses':{'projectId':'project'},'gemini_generate_content':{},'ollama_native':{},'lmstudio_native_v1':{},'deepl_v2':{'plan':'free'},'google_translate_v2':{},'google_translate_v3':{'projectId':'project','location':'global'},'baidu_vip':{},'youdao_v3':{},'tencent_tokenhub_translation':{}}
   command="INSERT INTO connections(revision_id,connection_id,display_name,protocol,deployment,auth_mode,endpoint,proxy_mode,network_metadata_json,availability_intent,http_options_schema,http_options_json,origin_schema,origin_json) VALUES('payload',?,'fixture',?,'cloud','none','https://example.invalid','inherit','{}',0,1,?,1,'{\"kind\":\"custom\"}')"
   for protocol,fields in protocols.items():db.execute(command,(protocol,protocol,json.dumps({'protocol':protocol}|fields)));record('payload-protocol-'+protocol,True)
   for name,payload in [('old-kind',{'kind':'openai_compatible','schemaVersion':1}),('header',{'protocol':'openai_compatible','headers':{'Authorization':'forbidden'}}),('shadow-version',{'protocol':'openai_compatible','schemaVersion':1})]:reject(db,'payload-reject-'+name,command,(name,'openai_compatible',json.dumps(payload)))
 except(sqlite3.Error,ValueError,AssertionError)as error:record('payload-setup',False,error)
 return results+r4_connection_semantic_checks(root)

def r4_connection_semantic_checks(root:Path)->list[dict]:
 """Full reviewed design, synthetic provenance only; no candidate/prod SQL input."""
 results=[]
 def connection(db,revision,key):
  _insert(db,'connections',{'revision_id':revision,'connection_id':key,'display_name':'synthetic old connection','protocol':'openai_compatible','deployment':'remote','auth_mode':'none','endpoint':'https://example.invalid','proxy_mode':'inherit','network_metadata_json':'{}','availability_intent':0})
 def revision(db,key,codec=2,hashed='b'*64):
  _insert(db,'config_revisions',{'revision_id':key,'content_hash':hashed,'created_at_ms':0,'save_reason':'create','local_validation_json':'{}','codec_version':codec})
 def seal(db,key):
  db.execute('INSERT INTO config_revision_seals SELECT revision_id,content_hash,0 FROM config_revisions WHERE revision_id=?',(key,))
 def model(db,rev,key,conn,purpose=None):
  row={'revision_id':rev,'model_id':key,'connection_id':conn,'display_name':'fixture','selection_source':'manual','capability_json':'{}'}
  if purpose is not None:row['template_purpose']=purpose
  _insert(db,'models',row)
 def proof(db,row):_insert(db,'activation_resource_materializations',row)
 def setup(db):
  _insert(db,'operation_receipts',{'operation_id':'synthetic-retention','request_sha256':'a'*64,'command_kind':'synthetic-GC-fixture','commit_revision':1,'result_refs_json':'{}','committed_at_ms':0})
  revision(db,'old');connection(db,'old','orphan');seal(db,'old')
  # Runtime controls live in a separate sealed snapshot, so GC tests are not
  # accidentally rejected by unrelated runtime/head references to old source.
  revision(db,'gate')
  for key in ['orphan','other','absent','http-origin']:connection(db,'gate',key)
  seal(db,'gate')
  for key in ['orphan','other','absent','http-origin']:_insert(db,'connection_runtime_states',{'connection_id':key,'config_revision_id':'gate','is_suspended':0 if key=='other'else 1,'control_revision':1,'reason_code':'stopped'if key!='other'else None,'updated_at_ms':0})
  revision(db,'effective',3);connection(db,'effective','copy')
  return {'materialization_id':'proof','effective_revision_id':'effective','target_kind':'connection','effective_model_id':None,'effective_offline_config_id':None,'source_revision_id':'old','source_model_id':None,'source_offline_config_id':None,'reason':'legacy_materialization','effective_connection_id':'copy','source_connection_id':'orphan','control_connection_id':'orphan','control_config_id':None}
 def expect(name,action,accept=False,message=None):
  failure=None;assertions=1
  try:
   with schema.design_database(root)as db:
    row=setup(db)
    try:action(db,row)
    except sqlite3.Error as error:failure=error
    passed=(failure is None)==accept and(message is None or failure is not None and message in str(failure))
    # A successful action also needs an intact whole-design FK graph.
    if accept:passed=passed and not db.execute('PRAGMA foreign_key_check').fetchall();assertions+=1
    results.append({'id':'V4-SEMANTIC:R4-connection-'+name,'status':'passed'if passed else'failed','assertions':assertions,'errors':[]if passed else[str(failure or 'unexpected accepted or FK failure')],'scope':'complete frozen R4 design; synthetic SQL provenance, not Rust authorization'})
  except(sqlite3.Error,ValueError,AssertionError)as error:results.append({'id':'V4-SEMANTIC:R4-connection-'+name,'status':'failed','assertions':assertions,'errors':['fixture/control failure: '+str(error)],'scope':'complete frozen R4 design'})
 def valid(db,p):
  proof(db,p);seal(db,'effective')
  assert db.execute("SELECT count(*) FROM models WHERE revision_id='effective'").fetchone()[0]==0
  assert tuple(db.execute("SELECT is_suspended,control_revision FROM connection_runtime_states WHERE connection_id='orphan'").fetchone())==(1,1)
  assert db.execute("SELECT control_connection_id FROM activation_resource_materializations WHERE materialization_id='proof'").fetchone()[0]=='orphan'
 expect('old-direct-sealed-no-model-valid',valid,True)
 expect('without-proof-cannot-seal',lambda d,p:seal(d,'effective'),message='invalid_codec3_graph')
 for name,key,value in [('effective-model','effective_model_id','invented'),('source-model','source_model_id','invented'),('effective-offline','effective_offline_config_id','invented'),('source-offline','source_offline_config_id','invented'),('reason-selected','reason','selected_closure'),('reason-collision','reason','activation_collision'),('effective-connection-null','effective_connection_id',None),('source-connection-null','source_connection_id',None),('control-connection-null','control_connection_id',None),('control-config-nonnull','control_config_id','invented'),('target-unknown','target_kind','task')]:
  expect('shape-'+name,lambda d,p,k=key,v=value:proof(d,p|{k:v}))
 expect('direct-effective-FK',lambda d,p:proof(d,p|{'effective_connection_id':'absent'}),message='FOREIGN KEY')
 expect('direct-source-FK',lambda d,p:proof(d,p|{'source_connection_id':'absent','control_connection_id':'absent'}),message='FOREIGN KEY')
 expect('cannot-borrow-running-control',lambda d,p:proof(d,p|{'control_connection_id':'other'}),message='control_bypass')
 def unsealed(db,p):
  revision(db,'unsealed');connection(db,'unsealed','orphan');proof(db,p|{'source_revision_id':'unsealed'})
 expect('source-unsealed',unsealed,message='control_bypass')
 def hash_mismatch(db,p):
  # Deliberate corrupt-source injection isolates the R4 source hash check;
  # normal immutable writes are independently tested and are not bypassed in product.
  db.execute('DROP TRIGGER config_revisions_no_update');db.execute("UPDATE config_revisions SET content_hash=? WHERE revision_id='old'",('c'*64,));proof(db,p)
 expect('source-seal-hash-mismatch',hash_mismatch,message='control_bypass')
 expect('source-GC-before-proof',lambda d,p:(d.execute("INSERT INTO config_gc_marks VALUES('old','synthetic-retention',0)"),proof(d,p)),message='control_bypass')
 def source_model(db,p):
  revision(db,'with-model');connection(db,'with-model','orphan');model(db,'with-model','model','orphan');seal(db,'with-model');proof(db,p|{'source_revision_id':'with-model'})
 expect('source-is-not-orphan',source_model,message='control_bypass')
 expect('effective-has-model-before-proof',lambda d,p:(model(d,'effective','model','copy'),proof(d,p)),message='control_bypass')
 expect('model-added-after-proof-rejects-seal',lambda d,p:(proof(d,p),model(d,'effective','model','copy','translation'),seal(d,'effective')),message='invalid_codec3_graph')
 # Self identity must fail as control_bypass even without any eligible model.
 expect('self-source-forbidden',lambda d,p:proof(d,p|{'source_revision_id':'effective','source_connection_id':'copy'}),message='control_bypass')
 def chain(db,p):
  proof(db,p);seal(db,'effective');revision(db,'next',3);connection(db,'next','copy-next')
  proof(db,p|{'materialization_id':'chain','effective_revision_id':'next','effective_connection_id':'copy-next','source_revision_id':'effective','source_connection_id':'copy'})
 expect('codec3-source-chain-forbidden',chain,message='control_bypass')
 def wrong_effective_codec(db,p):
  revision(db,'old-effective',2);connection(db,'old-effective','copy');proof(db,p|{'effective_revision_id':'old-effective'})
 expect('effective-must-be-codec3',wrong_effective_codec,message='control_bypass')
 expect('proof-is-immutable',lambda d,p:(proof(d,p),d.execute("UPDATE activation_resource_materializations SET source_connection_id='other' WHERE materialization_id='proof'")),message='immutable_config_child')
 expect('late-proof-after-seal',lambda d,p:(proof(d,p),seal(d,'effective'),d.execute('DROP TRIGGER v4_materialized_control'),proof(d,p|{'materialization_id':'late'})),message='sealed_config_snapshot')
 expect('sealed-proof-delete',lambda d,p:(proof(d,p),seal(d,'effective'),d.execute("DELETE FROM activation_resource_materializations WHERE materialization_id='proof'")),message='sealed_config_snapshot')
 def index_duplicate(db,p):
  proof(db,p);db.execute('DROP TRIGGER v4_materialized_control');proof(db,p|{'materialization_id':'duplicate'})
 expect('new-unique-index-is-enforced',index_duplicate,message='UNIQUE')
 def http(db,p,source='http-source'):
  revision(db,source);connection(db,source,'http-origin')
  for key in ['sm1','sm2']:model(db,source,key,'http-origin')
  seal(db,source)
  for key in ['em1','em2']:
   if not db.execute('SELECT 1 FROM models WHERE revision_id=? AND model_id=?',('effective',key)).fetchone():model(db,'effective',key,'copy')
  return p|{'materialization_id':'http-one','target_kind':'http','effective_model_id':'em1','source_revision_id':source,'source_model_id':'sm1','source_connection_id':'http-origin','control_connection_id':'http-origin'}
 def http_same(db,p):
  h=http(db,p);proof(db,h);proof(db,h|{'materialization_id':'http-two','effective_model_id':'em2','source_model_id':'sm2'});seal(db,'effective')
 expect('HTTP-two-models-consistent-origin',http_same,True)
 def http_conflict(db,p):
  h=http(db,p);other=http(db,p,'http-other');proof(db,h);proof(db,other|{'materialization_id':'http-two','effective_model_id':'em2','source_model_id':'sm2'})
 expect('HTTP-two-models-conflicting-origin',http_conflict,message='control_bypass')
 expect('HTTP-then-orphan-proof-conflict',lambda d,p:(proof(d,http(d,p)),proof(d,p)),message='control_bypass')
 expect('orphan-proof-then-HTTP-conflict',lambda d,p:(proof(d,p),proof(d,http(d,p))),message='control_bypass')
 expect('source-GC-while-child-live',lambda d,p:(proof(d,p),d.execute("INSERT INTO config_gc_marks VALUES('old','synthetic-retention',0)")),message='config_gc_still_referenced')
 expect('source-GC-without-child-control',lambda d,p:d.execute("INSERT INTO config_gc_marks VALUES('old','synthetic-retention',0)"),True)
 def gc(db,p,remove_child):
  proof(db,p);seal(db,'effective');db.execute("INSERT INTO config_gc_marks VALUES('effective','synthetic-retention',0)");db.execute("INSERT INTO config_gc_marks VALUES('old','synthetic-retention',0)")
  if remove_child:db.execute("DELETE FROM activation_resource_materializations WHERE materialization_id='proof'")
  db.execute("DELETE FROM connections WHERE revision_id='old'")
 expect('GC-parent-delete-before-child-FK',lambda d,p:gc(d,p,False),message='FOREIGN KEY')
 expect('GC-child-first-then-parent',lambda d,p:gc(d,p,True),True)
 def two(db,p):
  proof(db,p);connection(db,'effective','copy-two');proof(db,p|{'materialization_id':'proof-two','effective_connection_id':'copy-two','source_revision_id':'gate','source_connection_id':'other','control_connection_id':'other'});seal(db,'effective')
  assert db.execute("SELECT count(*) FROM activation_resource_materializations WHERE effective_revision_id='effective'").fetchone()[0]==2
 expect('two-distinct-orphan-connections',two,True)
 for table in ['binding_engines','task_execution_targets','attempt_execution_targets']:
  try:
   sql=_table_sql(root)[table];row=fixture_template(root)['tables'][table]['valid_variants'][0]
   with closing(_projection(sql))as db:
    positive=_outcome(db,table,row);negative=_outcome(db,table,row|{'target_kind':'connection'})
   passed=positive=={'outcome':'accept'}and negative=={'outcome':'reject','sqlite_errorcode':sqlite3.SQLITE_CONSTRAINT_CHECK}
   results.append({'id':'V4-SEMANTIC:R4-connection-not-execution-'+table,'status':'passed'if passed else'failed','assertions':2,'errors':[]if passed else[str((positive,negative))],'scope':'exact execution-table projection; connection proof grants no execution target'})
  except(sqlite3.Error,ValueError,AssertionError)as error:results.append({'id':'V4-SEMANTIC:R4-connection-not-execution-'+table,'status':'failed','assertions':1,'errors':[str(error)],'scope':'exact execution-table projection'})
 return results

def main()->int:
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--candidate',type=Path);r=assess(ROOT,p.parse_args().candidate);print(json.dumps(r,ensure_ascii=False));return int(r['status']!='passed')
if __name__=='__main__':raise SystemExit(main())
