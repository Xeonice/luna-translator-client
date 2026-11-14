"""Frozen design vs actual projection, not production acceptance."""
import copy,unittest
from contextlib import contextmanager
from pathlib import Path
from unittest.mock import patch
import storage_boundaries_v4 as b

class ServiceBoundaryGuards(unittest.TestCase):
 def test_every_field_has_valid_type_null_and_all_SQL_checks_execute(self):
  registry=b.build_registry();self.assertEqual(len(registry['field_ids']),282);ids={p['id']for p in registry['probes']}
  for field in registry['field_ids']:
   _,table,name=field.split('.')
   for label in ['valid','type','null']:self.assertIn('V4-PROBE:'+table+'.'+name+'.'+label,ids)
  result=b.assess();self.assertEqual(result['errors'],[]);self.assertTrue(result['not_product_acceptance']);self.assertGreater(len(result['checks']),2100)
 def test_removed_probe_changed_expectation_or_fixture_cannot_redefine_boundary(self):
  original=b.schema.read_json;registry=original(b.ROOT,b.REGISTRY);fixture=original(b.ROOT,b.FIXTURES)
  # Expected always comes from frozen independent design, never these altered files.
  expected=b.build_registry();expected_fixture=b.fixture_template()
  for mode in ['drop','expect','fields','fixture']:
   altered=copy.deepcopy(registry);f=copy.deepcopy(fixture)
   if mode=='drop':altered['probes'].pop()
   elif mode=='expect':altered['probes'][0]['expected']={'outcome':'reject'}
   elif mode=='fields':altered['field_ids'].pop()
   else:f['tables']['oauth_grants']['valid_variants'][0]['control_revision']=-1
   with patch.object(b,'build_registry',return_value=expected),patch.object(b,'fixture_template',return_value=expected_fixture),patch.object(b.schema,'read_json',side_effect=lambda root,path:altered if path==b.REGISTRY else f if path==b.FIXTURES else original(root,path)):
    self.assertEqual(b.assess()['status'],'failed',mode)
 def test_NUL_and_u32_constraints_are_not_learned_from_candidate(self):
  registry=b.build_registry();sqls=b._table_sql(b.ROOT)
  for table,field,label in [('oauth_client_profiles','client_profile_id','nul'),('oauth_client_profiles','client_profile_revision','u32_overflow')]:
   probe=next(p for p in registry['probes']if p['id']=='V4-PROBE:'+table+'.'+field+'.'+label);self.assertEqual(probe['expected']['outcome'],'reject');self.assertEqual(b._field_probe(sqls[table],probe),[])
   if label=='nul':unsafe=sqls[table].replace('AND instr(client_profile_id,char(0))=0','')
   else:unsafe=sqls[table].replace('client_profile_revision BETWEEN 1 AND 4294967295','client_profile_revision>0')
   self.assertTrue(b._field_probe(unsafe,probe))
 def test_87_FKs_each_have_exact_positive_orphan_wrong_owner_and_delete_assertions(self):
  registry=b.build_registry();rows=b.relationship_checks(b.ROOT)
  self.assertEqual({r['id']for r in rows},{r['id']for r in registry['relationship_inventory']});self.assertEqual(len(rows),89)
  self.assertTrue(all(r['status']=='passed'and r['assertions']>=4 for r in rows))
 def test_full_design_CAS_GC_seal_source_and_runtime_semantics_are_executed(self):
  rows=b.semantic_checks(b.ROOT);self.assertTrue(rows);self.assertTrue(all(r['status']=='passed'for r in rows),[r for r in rows if r['errors']])
  ids={r['id']for r in rows}
  for key in ['grant-stale-CAS','owned-refresh-cannot-cross-grant','alias-cannot-borrow-live-control','selector-transaction-rollback','typed-policy-no-legacy-shadow','pivot-no-mixed-runtime','installation-environment-architecture','resource-GC-reference','route-GC-reference']:
   self.assertIn('V4-SEMANTIC:'+key,ids)
 def test_missing_or_duplicate_relationship_semantic_execution_cannot_be_credited(self):
  registry=b.build_registry();fixtures=b.fixture_template();relations=b.relationship_checks(b.ROOT);semantics=b.semantic_checks(b.ROOT)
  for kind,rows in [('relationship',[]),('relationship',relations+[relations[0]]),('semantic',[]),('semantic',semantics+[semantics[0]])]:
   with patch.object(b,'build_registry',return_value=registry),patch.object(b,'fixture_template',return_value=fixtures),patch.object(b,'_field_probe',return_value=[]),patch.object(b,'relationship_checks',return_value=rows if kind=='relationship'else relations),patch.object(b,'semantic_checks',return_value=rows if kind=='semantic'else semantics):
    self.assertEqual(b.assess()['status'],'failed')
 def test_r2_legacy_slot_compatibility_and_typed_alias_dedup_cannot_regress(self):
  rows=b.semantic_checks(b.ROOT);by_id={r['id']:r for r in rows}
  for key in ['legacy-codec1-duplicate-disabled','legacy-codec2-duplicate-disabled','codec3-LegacyV2-source-duplicate','codec3-TypedV3-alias-duplicate','mixed-legacy-duplicate-typed-distinct','mixed-legacy-cannot-hide-typed-duplicate']:
   self.assertEqual(by_id['V4-SEMANTIC:dedup-'+key]['status'],'passed')
  raw=b.schema.safe_bytes(b.ROOT,b.schema.DESIGN).decode()
  gate=" AND EXISTS(SELECT 1 FROM purpose_bindings b JOIN config_revisions c ON c.revision_id=b.revision_id WHERE b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.semantics_version=3 AND c.codec_version=3)"
  self.assertEqual(raw.count(gate),1);old=raw.replace(gate,'')
  @contextmanager
  def old_behavior(root):
   with b.schema.v3.design_database(root)as db:
    db.execute('PRAGMA foreign_keys=OFF');db.executescript('BEGIN IMMEDIATE;\n'+old+'\nCOMMIT;');db.execute('PRAGMA foreign_keys=ON');yield db
  with patch.object(b.schema,'design_database',side_effect=old_behavior):broken={r['id']:r for r in b.semantic_checks(b.ROOT)}
  for key in ['legacy-codec1-duplicate-disabled','legacy-codec2-duplicate-disabled','codec3-LegacyV2-source-duplicate','mixed-legacy-duplicate-typed-distinct']:
   self.assertEqual(broken['V4-SEMANTIC:dedup-'+key]['status'],'failed')
  self.assertEqual(broken['V4-SEMANTIC:dedup-codec3-TypedV3-alias-duplicate']['status'],'passed')
 def test_R3_origin_site_text_NUL_types_and_unknown_key_guards_are_real(self):
  rows=b.semantic_checks(b.ROOT);by_id={r['id']:r for r in rows};site={k:v for k,v in by_id.items()if k.startswith('V4-SEMANTIC:origin-site-')}
  self.assertEqual(len(site),20);self.assertTrue(all(r['status']=='passed'for r in site.values()))
  raw=b.schema.safe_bytes(b.ROOT,b.schema.DESIGN).decode()
  guard="(json_type(NEW.origin_json,'$.site') IS NOT NULL AND (json_type(NEW.origin_json,'$.site') IS NOT 'text' OR instr(json_extract(NEW.origin_json,'$.site'),char(0))<>0))\n OR "
  self.assertEqual(raw.count(guard),1)
  @contextmanager
  def mutant(root,sql):
   with b.schema.v3.design_database(root)as db:
    db.execute('PRAGMA foreign_keys=OFF');db.executescript('BEGIN IMMEDIATE;\n'+sql+'\nCOMMIT;');db.execute('PRAGMA foreign_keys=ON');yield db
  no_type=raw.replace(guard,'')
  with patch.object(b.schema,'design_database',side_effect=lambda root:mutant(root,no_type)):
   weak={r['id']:r for r in b.semantic_checks(b.ROOT)}
  for kind in ['custom','preset']:
   for wrong in ['boolean','integer','fraction','null','array','object','nul']:
    self.assertEqual(weak['V4-SEMANTIC:origin-site-'+kind+'-reject-'+wrong]['status'],'failed')
   self.assertEqual(weak['V4-SEMANTIC:origin-site-'+kind+'-reject-unknown-key']['status'],'passed')
  old_custom=raw.replace("key NOT IN('kind','copiedFrom','site')","key NOT IN('kind','copiedFrom')")
  self.assertNotEqual(old_custom,raw)
  with patch.object(b.schema,'design_database',side_effect=lambda root:mutant(root,old_custom)):
   old={r['id']:r for r in b.semantic_checks(b.ROOT)}
  self.assertEqual(old['V4-SEMANTIC:origin-site-custom-text']['status'],'failed')
  self.assertEqual(old['V4-SEMANTIC:origin-site-custom-absent']['status'],'passed')
 def test_R4_connection_shape_has_all_256_NULL_masks_and_keeps_execution_targets_closed(self):
  probes=[p for p in b.build_registry()['probes']if p['label']=='connection_only_nullable_mask']
  self.assertEqual(len(probes),256);accepted=[p for p in probes if p['expected']=={'outcome':'accept'}]
  self.assertEqual([p['id']for p in accepted],['V4-R4-COMBINATION:connection-only:112'])
  rows=b.r4_connection_semantic_checks(b.ROOT);self.assertEqual(len(rows),41);self.assertTrue(all(r['status']=='passed'for r in rows),[r for r in rows if r['errors']])
 def test_R4_FK_projection_results_cannot_rebind_old_tags_or_omit_full_payload(self):
  registry=b.build_registry();fixtures=b.fixture_template();relations=b.relationship_checks(b.ROOT);semantics=b.semantic_checks(b.ROOT)
  for mode in ['missing-payload','ordinal-rebind']:
   altered=copy.deepcopy(relations);old=next(r for r in altered if r['id']=='V4-FK:activation_resource_materializations:2')
   if mode=='missing-payload':old.pop('relationship')
   else:old['relationship']['foreign_key']=copy.deepcopy(next(r for r in altered if r['id']=='V4-FK:activation_resource_materializations:8')['relationship']['foreign_key'])
   with patch.object(b,'build_registry',return_value=registry),patch.object(b,'fixture_template',return_value=fixtures),patch.object(b,'_field_probe',return_value=[]),patch.object(b,'relationship_checks',return_value=altered),patch.object(b,'semantic_checks',return_value=semantics):
    result=b.assess();self.assertEqual(result['status'],'failed');self.assertTrue(any('actual_FK_projection_identity' in e for e in result['errors']))
 def test_R4_source_control_GC_unique_and_direct_FK_guards_are_not_masked(self):
  raw=b.schema.safe_bytes(b.ROOT,b.schema.DESIGN).decode()
  @contextmanager
  def mutant(root,sql):
   with b.schema.v3.design_database(root)as db:
    db.execute('PRAGMA foreign_keys=OFF');db.executescript('BEGIN IMMEDIATE;\n'+sql+'\nCOMMIT;');db.execute('PRAGMA foreign_keys=ON');yield db
  cases=[('control',raw+'\nDROP TRIGGER v4_materialized_control;',['source-unsealed','source-seal-hash-mismatch','source-GC-before-proof','source-is-not-orphan','self-source-forbidden','codec3-source-chain-forbidden','effective-must-be-codec3','cannot-borrow-running-control','HTTP-two-models-conflicting-origin','HTTP-then-orphan-proof-conflict','orphan-proof-then-HTTP-conflict']),('GC',raw+'\nDROP TRIGGER config_gc_unreferenced;',['source-GC-while-child-live']),('index',raw+'\nDROP INDEX activation_connection_alias;',['new-unique-index-is-enforced'])]
  seal_guard=" OR EXISTS(SELECT 1 FROM activation_resource_materializations a JOIN models m ON m.revision_id=a.effective_revision_id AND m.connection_id=a.effective_connection_id WHERE a.effective_revision_id=NEW.revision_id AND a.target_kind='connection')\n"
  self.assertEqual(raw.count(seal_guard),1);cases.append(('orphan-seal',raw.replace(seal_guard,''),['model-added-after-proof-rejects-seal']))
  for side in ['effective','source']:
   line=' FOREIGN KEY('+side+'_revision_id,'+side+'_connection_id) REFERENCES connections(revision_id,connection_id) ON DELETE RESTRICT,\n'
   self.assertEqual(raw.count(line),1);cases.append((side+'-FK',raw.replace(line,''),['direct-'+side+'-FK']))
  for name,sql,keys in cases:
   with patch.object(b.schema,'design_database',side_effect=lambda root,sql=sql:mutant(root,sql)):rows={r['id']:r for r in b.r4_connection_semantic_checks(b.ROOT)}
   for key in keys:self.assertEqual(rows['V4-SEMANTIC:R4-connection-'+key]['status'],'failed',(name,key))
 def test_SQL_carrier_acceptance_remains_distinct_from_strict_Rust_obligation(self):
  probes=b.build_registry()['probes'];json_rows=[p for p in probes if p['label']=='wrong_inner_json_type']
  self.assertTrue(json_rows);self.assertTrue(any(p['expected']['outcome']=='accept'for p in json_rows))
  self.assertTrue(all(p['rust_obligation']=='strict_typed_json_inner_grammar'for p in json_rows))

if __name__=='__main__':unittest.main()
