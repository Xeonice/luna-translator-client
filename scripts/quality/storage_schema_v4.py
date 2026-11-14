#!/usr/bin/env python3
"""Independent schema-four design oracle. No production SQL is read to learn expected data."""
from __future__ import annotations
import argparse,copy,csv,hashlib,io,json,sqlite3
from contextlib import contextmanager
from pathlib import Path
import storage_schema as v1
import storage_schema_v3 as v3
from database_design import ROOT,_authorizer
from storage_boundaries import checks

BASE='docs/architecture/database/v4/'
DESIGN=BASE+'004_service_graph.design.sql'
CONTRACT=BASE+'schema-contract.json'
DICTIONARY=BASE+'field-dictionary.json'
CSV=BASE+'field-dictionary.csv'
REGISTRY=BASE+'storage-acceptance.json'
DESIGN_SHA256='09eab3339ed56f535f72cc95d6d12cab069c73d6cd47a07197dd69f561351d55'
PRIOR_FILES={
 v3.CONTRACT:'90d3474fb23c4522845727307ea3b47b5a2ae0a5f0b6df40006516aa22fbe335',
 v3.DICTIONARY:'b3144b3820ae1bd1c91ea06343bfa85a954eb7cdc6950f9bae21c5ae988d349a',
 v3.CSV:'f2008ab47ac90143e99d019da3072b371d4cf65af133b2ecb64f4fde67ea7c5d',
 v3.REGISTRY:'808822b6b335ab6e77daebb0fd4c44f29e6b1acc0fedbbef4bc7fd67a6b16b5d',
 'docs/architecture/database/v3/field-boundaries.json':'174d61829a681c204fb3b600354c010ccd31b0dfc50a5cfd25f91792e1d4038e',
 'docs/architecture/database/v3/boundary-fixtures.json':'02ab9ee4c1e1f7361e6acc6eb0c768141c2f85af1ebd2dfe1d003608df96e9d1',
}
NEW_COLUMNS={'activation_resource_materializations': ['materialization_id', 'effective_revision_id', 'target_kind', 'effective_model_id', 'effective_offline_config_id', 'source_revision_id', 'source_model_id', 'source_offline_config_id', 'reason', 'effective_connection_id', 'source_connection_id', 'control_connection_id', 'control_config_id'], 'attempt_execution_targets': ['attempt_id', 'task_id', 'config_revision_id', 'binding_id', 'engine_id', 'target_kind', 'http_model_id', 'http_connection_id', 'offline_engine_config_id', 'snapshot_id', 'route_id', 'observation_id', 'observation_epoch', 'runtime_build_sha256', 'worker_instance_id', 'worker_protocol_revision', 'provenance_sha256'], 'binding_engines': ['target_kind', 'offline_engine_config_id', 'policy_overrides_json'], 'cache_entries': ['origin_attempt_id'], 'config_revisions': ['revision_role'], 'connections': ['http_options_schema', 'http_options_json', 'origin_schema', 'origin_json', 'oauth_grant_id', 'oauth_authorization_epoch'], 'execution_authorization_leases': ['lease_id', 'attempt_id', 'task_id', 'test_id', 'origin_kind', 'grant_id', 'authorization_epoch', 'material_revision', 'pool_id', 'pool_revision', 'member_id', 'credential_id', 'auth_kind', 'lifecycle_revision', 'scopes_sha256', 'client_policy_sha256', 'target_policy_sha256', 'minted_at_ms', 'expires_at_ms'], 'models': ['template_purpose'], 'oauth_authorization_snapshots': ['grant_id', 'authorization_epoch', 'state', 'scope', 'scopes_sha256', 'previous_epoch', 'created_at_ms'], 'oauth_client_profiles': ['client_profile_id', 'client_profile_revision', 'issuer', 'public_client_id', 'client_mode', 'redirect_policy', 'policy_sha256', 'setup_state', 'created_at_ms'], 'oauth_grants': ['grant_id', 'client_profile_id', 'client_profile_revision', 'principal_state', 'principal_subject', 'created_at_ms', 'authorization_epoch', 'material_revision', 'control_revision'], 'oauth_token_material_revisions': ['grant_id', 'material_revision', 'credential_id', 'auth_kind', 'parent_material_revision', 'created_at_ms'], 'offline_config_runtime_states': ['engine_config_id', 'control_revision', 'is_suspended', 'reason_code', 'updated_at_ms'], 'offline_engine_configs': ['revision_id', 'engine_config_id', 'engine_id', 'engine_contract_revision', 'backend', 'display_name', 'source_language', 'target_language', 'route_policy', 'consent_revision', 'installation_id', 'installation_revision', 'resolution', 'snapshot_id', 'route_id'], 'offline_execution_environments': ['environment_id', 'environment_revision', 'platform', 'architecture', 'machine_fingerprint', 'os_build', 'bottle_identity', 'local_prefix_path', 'wine_build', 'created_at_ms'], 'offline_installation_heads': ['installation_id', 'installation_revision', 'row_revision', 'enabled'], 'offline_installation_versions': ['installation_id', 'installation_revision', 'engine_id', 'engine_contract_revision', 'environment_id', 'environment_revision', 'local_native_path', 'local_grant_id', 'runtime_profile_id', 'runtime_profile_revision', 'created_at_ms'], 'offline_local_grants': ['grant_id', 'machine_fingerprint', 'opaque_locator', 'authorization_epoch', 'state', 'created_at_ms'], 'offline_observation_heads': ['subject_key', 'observation_epoch', 'observation_id', 'row_revision'], 'offline_resource_observations': ['observation_id', 'subject_key', 'observation_epoch', 'installation_id', 'installation_revision', 'snapshot_id', 'local_grant_id', 'local_grant_epoch', 'state', 'reason_code', 'observed_at_ms'], 'offline_resource_pairs': ['snapshot_id', 'source_language', 'target_language', 'pipeline_profile_id'], 'offline_resource_seals': ['snapshot_id', 'content_hash', 'sealed_at_ms'], 'offline_resource_snapshot_files': ['snapshot_id', 'relative_key', 'byte_length', 'sha256', 'role'], 'offline_resource_snapshots': ['snapshot_id', 'engine_id', 'engine_contract_revision', 'backend', 'runtime_profile_id', 'runtime_profile_revision', 'identity_kind', 'manifest_sha256', 'os_build', 'bridge_build_sha256', 'api_profile', 'strategy', 'source_language', 'target_language', 'observation_epoch', 'content_hash', 'created_at_ms'], 'offline_route_seals': ['route_id', 'content_hash', 'sealed_at_ms'], 'offline_runtime_profiles': ['runtime_profile_id', 'runtime_profile_revision', 'engine_id', 'engine_contract_revision', 'backend', 'platform', 'architecture', 'private_protocol_revision', 'abi_profile_id', 'verification_state', 'policy_sha256', 'created_at_ms'], 'offline_test_targets': ['test_id', 'config_revision_id', 'engine_config_id', 'phase', 'snapshot_id', 'route_id', 'observation_id', 'observation_epoch', 'runtime_build_sha256'], 'offline_translation_route_hops': ['route_id', 'hop_ordinal', 'snapshot_id', 'engine_id', 'engine_contract_revision', 'backend', 'source_language', 'target_language', 'pipeline_profile_id'], 'offline_translation_routes': ['route_id', 'engine_id', 'engine_contract_revision', 'backend', 'route_kind', 'source_language', 'target_language', 'consent_revision', 'content_hash', 'created_at_ms'], 'purpose_bindings': ['semantics_version', 'selection_revision', 'policy_overrides_json'], 'service_default_fallbacks': ['revision_id', 'defaults_id', 'position', 'target_kind', 'source_revision_id', 'source_model_id', 'source_offline_config_id'], 'service_request_defaults': ['revision_id', 'defaults_id', 'target_kind', 'model_id', 'offline_engine_config_id', 'purpose', 'source_mode', 'source_language', 'target_mode', 'target_language', 'context_enabled', 'context_sentences', 'context_mode', 'timeout_ms', 'concurrency', 'retries', 'cache_enabled'], 'task_allowed_fallback_targets': ['task_id', 'fallback_ordinal', 'config_revision_id', 'binding_id', 'engine_id', 'target_kind'], 'task_execution_targets': ['task_id', 'config_revision_id', 'binding_id', 'engine_id', 'purpose', 'target_kind', 'http_model_id', 'http_connection_id', 'offline_engine_config_id', 'snapshot_id', 'route_id', 'observation_id', 'observation_epoch', 'selection_scope', 'selection_revision', 'effective_scope', 'effective_selection_revision', 'target_sha256'], 'test_records': ['target_kind']}
EXPANDED_OLD_FIELDS=['profile.config_revisions.codec_version','profile.connections.auth_mode','profile.credential_refs.auth_kind','profile.provider_preset_versions.auth_kind','profile.binding_engines.model_id']
SEMANTIC_IDS={'V4-'+s for s in ['PRODUCERS','CODEC','MIGRATION','RELATIONSHIPS','AUTHORITY','CREDENTIALS','LEGACY-PRESERVATION','SELECTION','OFFLINE','GOOGLE','CAS-GC','BACKUP-RECOVERY']}
# Stable IDs bind complete R3 FK payloads, not SQLite FK enumeration or counts.
R3_RELATIONSHIPS_SHA256='70e779c09ce2a9cffcc4a4ee04a5f3d682359989e07d93ec254d8d064308ff58'
R4_DIRECT_RELATIONSHIPS=[{'id': 'V4-FK:activation_resource_materializations:8', 'table': 'activation_resource_materializations', 'foreign_key': {'columns': ['effective_revision_id', 'effective_connection_id'], 'parent': 'connections', 'parent_columns': ['revision_id', 'connection_id'], 'on_update': 'NO ACTION', 'on_delete': 'RESTRICT', 'match': 'NONE'}, 'prior_relationship': False}, {'id': 'V4-FK:activation_resource_materializations:9', 'table': 'activation_resource_materializations', 'foreign_key': {'columns': ['source_revision_id', 'source_connection_id'], 'parent': 'connections', 'parent_columns': ['revision_id', 'connection_id'], 'on_update': 'NO ACTION', 'on_delete': 'RESTRICT', 'match': 'NONE'}, 'prior_relationship': False}]
safe_bytes=v3.safe_bytes
read_json=v3.read_json
digest=v3.digest

def validate_prior(root:Path)->list[str]:
 errors=v3.validate_schema_contract(root)+v3.validate_acceptance_registry(root)
 for path,expected in PRIOR_FILES.items():
  if digest(safe_bytes(root,path))!=expected:errors.append('original v1/v2/v3 obligation bytes changed: '+path)
 return errors

@contextmanager
def design_database(root:Path):
 data=safe_bytes(root,DESIGN)
 if digest(data)!=DESIGN_SHA256:raise ValueError('schema-four frozen design hash changed; independent review required')
 with v3.design_database(root) as db:
  db.execute('PRAGMA foreign_keys=OFF')
  db.executescript('BEGIN IMMEDIATE;\n'+data.decode()+'\nCOMMIT;')
  db.execute('PRAGMA foreign_keys=ON')
  if db.execute('PRAGMA foreign_keys').fetchone()[0]!=1 or db.execute('PRAGMA foreign_key_check').fetchall():raise ValueError('schema-four independent design foreign-key failure')
  for name, in db.execute("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").fetchall():
   first=db.execute('PRAGMA table_info('+v1._name(name)+')').fetchone()[1]
   db.execute('EXPLAIN UPDATE '+v1._name(name)+' SET '+v1._name(first)+'=NULL WHERE 0')
   db.execute('EXPLAIN DELETE FROM '+v1._name(name)+' WHERE 0')
  db.execute('SELECT * FROM v4_attempt_target_authority LIMIT 0')
  yield db

def _preservation(before:dict,after:dict)->None:
 if len(set(after['tables'])-set(before['tables']))!=28 or set(before['tables'])-set(after['tables']):raise ValueError('schema-four must retain all old tables and add exactly28')
 if len(after['tables'])!=81 or sum(len(t['columns'])for t in after['tables'].values())!=734:raise ValueError('schema-four profile inventory must be 81 tables /734 columns')
 for table,old in before['tables'].items():
  new={c['name']:c for c in after['tables'][table]['columns']}
  for column in old['columns']:
   wanted=copy.deepcopy(column)
   if table=='binding_engines' and column['name']=='model_id':wanted['notnull']=0
   if wanted!=new.get(column['name']):raise ValueError('old physical field changed beyond reviewed HTTP/offline XOR: '+table+'.'+column['name'])

def design_contract(root:Path)->dict:
 before=v3.design_contract(root)
 with design_database(root) as db:profile=v1.inspect_connection(db)
 _preservation(before['databases']['profile'],profile)
 result=copy.deepcopy(before);result['databases']['profile']=profile
 result['ddl_source_sha256'][DESIGN]=DESIGN_SHA256
 result['prior_contract_sha256']=PRIOR_FILES[v3.CONTRACT]
 result['phase']='independent_schema_four_design_frozen_contract_pending_non_author_review'
 result['migration_version']=4
 if new_columns(result,before)!=NEW_COLUMNS:raise ValueError('code-owned new field inventory differs from frozen independent design')
 return result

def new_columns(contract:dict,prior:dict)->dict[str,list[str]]:
 old=prior['databases']['profile']['tables'];new=contract['databases']['profile']['tables']
 return {t:[c['name']for c in value['columns']if c['name']not in{r['name']for r in old.get(t,{}).get('columns',[])}]for t,value in new.items()if t not in old or any(c['name']not in {r['name']for r in old[t]['columns']}for c in value['columns'])}

def field_rows(contract:dict,prior:dict|None=None)->dict:
 # Prior shape is the pinned independent v3 oracle, never a production DB.
 if prior is None:prior=v3.design_contract(ROOT)
 wanted=new_columns(contract,prior);tables=contract['databases']['profile']['tables']
 return {f'profile.{t}.{c}':(t,next(r for r in tables[t]['columns']if r['name']==c))for t,names in wanted.items()for c in names}

def producer(table:str)->str:
 stem='crates/app-infra/src/storage/queries/'
 if table.startswith('oauth_')or table=='execution_authorization_leases':return stem+'google_authorization.rs'
 if table in {'task_execution_targets','attempt_execution_targets','task_allowed_fallback_targets','cache_entries'}:return stem+'tasks.rs'
 if table=='activation_resource_materializations':return stem+'service_selection.rs'
 if table.startswith('offline_')and table!='offline_engine_configs':return stem+'offline_services.rs'
 return stem+'service_graph.rs'

def validate_relationship_identity_rows(rows:list[dict])->list[str]:
 if not isinstance(rows,list)or any(not isinstance(r,dict)or not isinstance(r.get('id'),str)for r in rows):return['FK inventory must have typed complete relationship rows']
 if len(rows)!=89 or len({r['id']for r in rows})!=89:return['FK inventory must retain87 complete original identities and append2 unique direct identities']
 added={r['id']:r for r in R4_DIRECT_RELATIONSHIPS};old=sorted([r for r in rows if r['id']not in added],key=lambda r:r['id']);errors=[]
 canonical=json.dumps(old,sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()
 if len(old)!=87 or digest(canonical)!=R3_RELATIONSHIPS_SHA256:errors.append('original R3 ID to full FK payload identity changed; ordinal reassignment is forbidden')
 for identifier,expected in added.items():errors+=v1._diff(expected,next((r for r in rows if r['id']==identifier),None),'stable_R4_relationship.'+identifier)
 return errors

def relationship_rows(contract:dict,prior:dict)->list[dict]:
 tables=contract['databases']['profile']['tables'];old=prior['databases']['profile']['tables'];rows=[];added=R4_DIRECT_RELATIONSHIPS
 for table in sorted(new_columns(contract,prior)):
  fresh=[r['foreign_key']for r in added if r['table']==table]
  for fk in fresh:
   if tables[table]['foreign_keys'].count(fk)!=1:raise ValueError('reviewed R4 direct relationship missing, duplicated or changed: '+table+str(fk))
  # The same conservative canonical order as the original inspector, excluding only
  # the exact reviewed R4 additions; old meaning is then pinned by full-row hash.
  preserved=sorted([fk for fk in tables[table]['foreign_keys']if fk not in fresh],key=lambda fk:json.dumps(fk,sort_keys=True))
  for index,fk in enumerate(preserved):rows.append({'id':f'V4-FK:{table}:{index}','table':table,'foreign_key':fk,'prior_relationship':fk in old.get(table,{}).get('foreign_keys',[])})
 rows+=copy.deepcopy(added)
 errors=validate_relationship_identity_rows(rows)
 if errors:raise ValueError('; '.join(errors))
 return rows

def trigger_rows(contract:dict,prior:dict)->list[dict]:
 rows=[]
 for table,data in contract['databases']['profile']['tables'].items():
  old=prior['databases']['profile']['tables'].get(table,{})
  for name,tokens in data['triggers'].items():
   if tokens!=old.get('triggers',{}).get(name):rows.append({'id':'V4-TRIGGER:'+table+'.'+name,'table':table,'name':name,'sql_tokens':tokens,'producer':producer(table),'runtime_status':'not_run','required_assertion':'actual trigger path with positive control and exact rejection; source token presence is insufficient'})
 return rows

def view_rows(contract:dict,prior:dict)->list[dict]:
 return [{'id':'V4-VIEW:'+name,'name':name,'sql_tokens':tokens,'runtime_status':'not_run','required_assertion':'actual current target/credential/control/epoch authority and late result rejection'}for name,tokens in contract['databases']['profile']['views'].items()if tokens!=prior['databases']['profile']['views'].get(name)]

def dictionary_template(root:Path,contract:dict|None=None)->dict:
 contract=contract or design_contract(root);prior=v3.design_contract(root)
 with design_database(root)as db:sqls=dict(db.execute("SELECT name,sql FROM sqlite_schema WHERE type='table'"))
 relations=relationship_rows(contract,prior)
 fields=[]
 for identifier,(table,column)in field_rows(contract,prior).items():
  fields.append({'id':identifier,'table':table,'field':column['name'],'type':column['type'],'nullable':not bool(column['notnull']or column['pk']),'primary_key_position':column['pk'],'default_tokens':column['dflt_value'],'source':DESIGN,'producer':producer(table),'producer_status':'pending_runtime_verification','runtime_proof_id':'RUST-MAP:'+identifier,'sql_boundary_proof_id':'V4-FIELD:'+identifier,'description':f'{table}.{column["name"]}；{column["type"]}，'+('允许NULL但须满足引用/目标联合条件'if not(column['notnull']or column['pk'])else'必填')+'；字段carrier、身份和关系遵循下面独立SQL约束，不等于服务已就绪。','sql_checks':[condition for condition in checks(sqls[table])if column['name']in v1.sql_tokens(condition)],'relationship_ids':[r['id']for r in relations if r['table']==table and column['name']in r['foreign_key']['columns']]})
 return {'schema_version':1,'kind':'schema_four_service_field_delta','previous_fields_retained':484,'new_fields':282,'total_fields':766,'profile_fields':734,'dictionary_fields':32,'schema_contract':CONTRACT,'expanded_old_fields':EXPANDED_OLD_FIELDS,'fields':fields,'relationships':relations,'trigger_obligations':trigger_rows(contract,prior),'view_obligations':view_rows(contract,prior),'product_tests_executed':False,'runtime_field_proofs':{'status':'not_run','required_field_ids':['RUST-MAP:'+r['id']for r in fields],'required_SQL_subassertions':['valid','type','null'],'counts_are_design_inventory_not_execution':True}}

def csv_text(dictionary:dict)->str:
 output=io.StringIO();keys=['id','table','field','type','nullable','primary_key_position','default_tokens','producer','producer_status','description','sql_checks','relationship_ids'];writer=csv.DictWriter(output,fieldnames=keys,lineterminator='\n');writer.writeheader()
 for r in dictionary['fields']:
  row={k:r[k]for k in keys};row['nullable']=str(row['nullable']).lower();row['default_tokens']=json.dumps(row['default_tokens'],separators=(',',':'));row['sql_checks']=json.dumps(row['sql_checks'],ensure_ascii=False,separators=(',',':'));row['relationship_ids']=json.dumps(row['relationship_ids'],separators=(',',':'));writer.writerow(row)
 return output.getvalue()

def validate_field_dictionary(root:Path,contract:dict|None=None)->list[str]:
 expected=dictionary_template(root,contract)
 return v1._diff(expected,read_json(root,DICTIONARY),'v4_dictionary')+([]if safe_bytes(root,CSV).decode()==csv_text(expected)else['v4 dictionary CSV exact content differs'])

def v4_check_ids(root:Path)->set[str]:
 fields=set(field_rows(design_contract(root),v3.design_contract(root)))
 contract=design_contract(root);prior=v3.design_contract(root);relationships=relationship_rows(contract,prior)
 return {r['id']for r in trigger_rows(contract,prior)+view_rows(contract,prior)}|SEMANTIC_IDS|{'RUST-MAP:'+f for f in fields}|{'V4-FIELD:'+f for f in fields}|{'V4-LEGACY-FIELD:'+f for f in EXPANDED_OLD_FIELDS}|{r['id']for r in relationships}

def acceptance_template(root:Path)->dict:
 prior=read_json(root,v3.REGISTRY);matrix=copy.deepcopy(prior['supported_upgrade_matrix']);matrix['status']='design_frozen_runtime_not_run';matrix['approval_ref']=BASE+'storage-review.json';matrix['fresh_versions']={'profile':4,'dictionary':1}
 matrix['migration_chain'].append({'version':4,'previous_version':3,'source_path':'crates/app-infra/src/storage/database.rs','sql_sources':[{'path':'crates/app-infra/src/storage/migrations/004_service_graph.sql','sha256':DESIGN_SHA256}],'migration_sha256':DESIGN_SHA256})
 for route in matrix['routes']:
  if route['from_version']==3:route['from_version']=4
  route['to_version']=4;route['target_schema']=CONTRACT
  if route['source_schema']==v3.CONTRACT:route['source_schema']=CONTRACT
 for version,source in [(1,v1.CONTRACT),(2,v3.v2.CONTRACT),(3,v3.CONTRACT)]:
  matrix['routes'].append({'id':f'upgrade_profile_v{version}_to_v4','kind':'upgrade','database':'profile','from_version':version,'to_version':4,'source_state':'supported','source_schema':source,'target_schema':CONTRACT,'expected':'accepted','expected_error':None})
 return {'schema_version':1,'id':'FP1-STORAGE-20261106-SCHEMA4-SERVICES','kind':'schema_four_service_storage_acceptance','phase':'planning','product_tests_executed':False,'schema_contract':CONTRACT,'boundary_contract':BASE+'field-boundaries.json','prior_obligation_sources':PRIOR_FILES,'checks':prior['checks'],'plan':prior['plan'],'prerequisite_ids':prior['prerequisite_ids'],'maintenance_ids':prior['maintenance_ids'],'enablement_ids':prior['enablement_ids'],'required_fresh_routes':prior['required_fresh_routes'],'supported_upgrade_matrix':matrix,'original_checks':prior['checks'],'original_upgrade_matrix':prior['supported_upgrade_matrix'],'prior_delta_check_ids':prior['delta_check_ids'],'delta_check_ids':sorted(v4_check_ids(root)),'field_subassertions':['valid','type','null'],'relationship_inventory':relationship_rows(design_contract(root),v3.design_contract(root)),'retained_R3_relationships_sha256':R3_RELATIONSHIPS_SHA256,'FK_receipt_payload':'each actual FK check carries relationship complete row read independently from actual candidate; ID or assertion count alone is insufficient','expanded_old_fields':EXPANDED_OLD_FIELDS,'required_routes':['fresh_0_to_4','upgrade_1_to_4_with_backup','upgrade_2_to_4_with_backup','upgrade_3_to_4_with_backup','reopen_4','real_store3_rejects_4','old_store2_rejects_4','rollback_failed_4','crash_atomic_3_or_4','backup_restore','reject_unknown_version','reject_corrupt_hash_schema_fk_profile','readonly_busy_physical_disk_full'],'required_runtime_provenance':['real_rust_compile','unfiltered_registered_test_target','run_id_and_nonce','current_source_and_artifact_identity','actual_database_snapshots','old_rows_seals_heads_preserved','single_file_backup_identity','typed_vault_authority','no_draft_movement_atomic_selection','actual_dispatch_target_and_lease_provenance'],'runtime_policy':{'adapter_status':'pending_backend_registration_and_real_execution','forbidden_substitutes':['design_ddl_clone','guard_fixture','hash_only_receipt','old_run_receipt','unit_tests_only'],'product_status_source':'separate_runtime_receipts_only'}}

def derive_effective_v4_registry(root:Path)->dict:return acceptance_template(root)
def validate_acceptance_registry(root:Path,data=None)->list[str]:
 try:return validate_prior(root)+v1._diff(acceptance_template(root),read_json(root,REGISTRY)if data is None else data,'schema_four_acceptance')
 except(OSError,ValueError,TypeError,KeyError,RecursionError,sqlite3.Error)as e:return['schema-four acceptance: '+str(e)]

def validate_delta_receipt(root:Path,receipt:dict)->list[str]:
 rows=receipt.get('checks')if isinstance(receipt,dict)else None
 if not isinstance(rows,list)or any(not isinstance(r,dict)or not isinstance(r.get('id'),str)for r in rows):return['v4 delta checks must be typed rows']
 ids=[r['id']for r in rows];errors=[]
 if len(ids)!=len(set(ids)):errors.append('duplicate v4 runtime check id')
 by_id={r['id']:r for r in rows}
 relationships={r['id']:r for r in relationship_rows(design_contract(root),v3.design_contract(root))}
 for identifier in sorted(v4_check_ids(root)):
  row=by_id.get(identifier,{})
  if row.get('status')!='passed'or type(row.get('assertions'))is not int or row['assertions']<=0 or type(row.get('skipped'))is not int or row['skipped']!=0:errors.append('missing actual v4 assertion: '+identifier)
  if identifier in relationships:errors+=v1._diff(relationships[identifier],row.get('relationship'),'actual_FK_identity.'+identifier)
  if identifier.startswith('V4-FIELD:'):
   sub=row.get('field_assertions')
   if not isinstance(sub,dict)or set(sub)!={'valid','type','null'}or any(type(v)is not int or v<=0 for v in sub.values()):errors.append('missing valid/type/null subassertions: '+identifier)
 return errors

def validate_schema_contract(root:Path)->list[str]:
 try:
  actual=design_contract(root)
  return validate_prior(root)+v1._diff(read_json(root,CONTRACT),actual,'schema_four_contract')+validate_field_dictionary(root,actual)
 except(OSError,ValueError,TypeError,KeyError,RecursionError,sqlite3.Error)as e:return['schema-four contract: '+str(e)]
def compare_current_schema(root:Path,candidate:Path,database:str='profile')->list[str]:return validate_schema_contract(root)or v1.compare_schema_file(root,candidate,database,CONTRACT)
def assess(root:Path=ROOT,candidate:Path|None=None,database:str='profile')->dict:
 errors=compare_current_schema(root,candidate,database)if candidate else validate_schema_contract(root)
 return {'kind':'schema_four_contract_receipt','status':'failed'if errors else'passed','scope':'independent_design_shape_and_dictionary_only','product_migration_executed':False,'product_acceptance':False,'errors':errors}
def main()->int:
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--candidate',type=Path);parser.add_argument('--database',choices=['profile','dictionary'],default='profile');r=assess(ROOT,**vars(parser.parse_args()));print(json.dumps(r,ensure_ascii=False));return int(r['status']!='passed')
if __name__=='__main__':raise SystemExit(main())
