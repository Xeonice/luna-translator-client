"""Non-product guard fixtures; schema4 production migrations never supply the oracle."""
import copy,json,re,shutil,sqlite3,tempfile,unittest
from contextlib import closing
from pathlib import Path
from unittest.mock import patch
import storage_schema_v4 as s

class SchemaFourGuards(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='schema4-guard-');self.addCleanup(self.temp.cleanup);self.folder=Path(self.temp.name).resolve();self.counter=0
 def candidate(self,after=''):
  self.counter+=1;path=self.folder/(str(self.counter)+'.sqlite3')
  with s.design_database(s.ROOT)as source:
   with closing(sqlite3.connect(path))as target:source.backup(target)
  if after:
   with closing(sqlite3.connect(path))as db:db.executescript(after)
  return path
 def test_expected_design_and_registry_never_read_production_migration_sql(self):
  original=s.safe_bytes;prior_reader=s.v3.safe_bytes;visited=[]
  def controlled(root,path):
   visited.append(path)
   self.assertFalse(str(path).startswith('crates/app-infra/src/storage/migrations/'))
   return original(root,path)
  def controlled_prior(root,path):
   visited.append(path)
   self.assertFalse(str(path).startswith('crates/app-infra/src/storage/migrations/'))
   return prior_reader(root,path)
  with patch.object(s,'safe_bytes',side_effect=controlled),patch.object(s.v3,'safe_bytes',side_effect=controlled_prior):
   s.design_contract(s.ROOT);s.acceptance_template(s.ROOT)
  self.assertIn(s.DESIGN,visited);self.assertIn(s.v3.DESIGN,visited)
 def test_full_766_field_inventory_and_only_reviewed_old_physical_relaxation(self):
  self.assertEqual(s.validate_schema_contract(s.ROOT),[]);new=s.design_contract(s.ROOT);old=s.v3.design_contract(s.ROOT)
  self.assertEqual(sum(len(t['columns'])for d in new['databases'].values()for t in d['tables'].values()),766)
  self.assertEqual(len(s.field_rows(new)),282);self.assertEqual(new['databases']['dictionary'],old['databases']['dictionary'])
  self.assertEqual(len(s.relationship_rows(new,old)),89);self.assertEqual(len(s.trigger_rows(new,old)),69);self.assertEqual(len(s.view_rows(new,old)),1)
  s._preservation(old['databases']['profile'],new['databases']['profile'])
  with patch.object(s,'NEW_COLUMNS',{'connections':[]}):self.assertTrue(s.validate_schema_contract(s.ROOT))
 def test_R4_full_R3_FK_identity_is_preserved_and_naive_ordinal_rebinding_is_rejected(self):
  contract=s.design_contract(s.ROOT);prior=s.v3.design_contract(s.ROOT);rows=s.relationship_rows(contract,prior)
  self.assertEqual(s.validate_relationship_identity_rows(rows),[])
  self.assertEqual([r['id']for r in rows[-2:]],['V4-FK:activation_resource_materializations:8','V4-FK:activation_resource_materializations:9'])
  self.assertEqual(len(rows[:-2]),87)
  # Every original full row remains bound, not merely its ID/count. Mutate one
  # complete relationship at a time without changing any IDs or row counts.
  for index,row in enumerate(rows[:-2]):
   changed=copy.deepcopy(rows);changed[index]['foreign_key']['on_delete']='CASCADE'if row['foreign_key']['on_delete']!='CASCADE'else'RESTRICT'
   self.assertTrue(s.validate_relationship_identity_rows(changed),row['id'])
  naive=[]
  for table in sorted(s.new_columns(contract,prior)):
   for index,fk in enumerate(contract['databases']['profile']['tables'][table]['foreign_keys']):naive.append({'id':f'V4-FK:{table}:{index}','table':table,'foreign_key':fk,'prior_relationship':fk in prior['databases']['profile']['tables'].get(table,{}).get('foreign_keys',[])})
  self.assertEqual({r['id']for r in naive},{r['id']for r in rows});self.assertEqual(len(naive),89)
  self.assertTrue(s.validate_relationship_identity_rows(naive))
  by_id={r['id']:r for r in naive};wrong=[r['id']for r in rows[:-2]if by_id[r['id']]!=r]
  self.assertEqual(wrong,[f'V4-FK:activation_resource_materializations:{i}'for i in range(2,8)])
  # A changed FK definition cannot acquire an old identity during regeneration.
  changed=copy.deepcopy(contract);changed['databases']['profile']['tables']['activation_resource_materializations']['foreign_keys'][0]['on_delete']='CASCADE'
  with self.assertRaises(ValueError):s.relationship_rows(changed,prior)
 def test_exact_candidate_readonly_and_old_oracle_rejects_new_schema(self):
  p=self.candidate();before=p.read_bytes();self.assertEqual(s.compare_current_schema(s.ROOT,p),[]);self.assertTrue(s.v3.compare_current_schema(s.ROOT,p));self.assertEqual(before,p.read_bytes())
 def test_each_282_field_type_and_nullable_declaration_drift_is_rejected(self):
  original=self.candidate();contract=s.design_contract(s.ROOT)
  for identifier,(table,column)in s.field_rows(contract).items():
   for mode in ['type','null']:
    with self.subTest(field=identifier,mode=mode):
     p=self.folder/('mutant-'+str(self.counter)+'.sqlite3');self.counter+=1;shutil.copyfile(original,p)
     with closing(sqlite3.connect(p))as db:
      sql=db.execute("SELECT sql FROM sqlite_schema WHERE type='table' AND name=?",(table,)).fetchone()[0]
      pattern=r'(?<![A-Za-z0-9_])'+re.escape(column['name'])+r'\s+(TEXT|INTEGER)(\s+NOT\s+NULL)?'
      def replacement(match):return column['name']+' '+('BLOB'if mode=='type'else match[1])+((match[2]or'')if mode=='type'else(''if match[2]else' NOT NULL'))
      changed,count=re.subn(pattern,replacement,sql,count=1,flags=re.I);self.assertEqual(count,1)
      db.execute('PRAGMA writable_schema=ON');db.execute("UPDATE sqlite_schema SET sql=? WHERE type='table' AND name=?",(changed,table));db.execute('PRAGMA schema_version=999');db.commit()
     self.assertTrue(s.v1.compare_schema_file(s.ROOT,p,'profile',s.CONTRACT))
 def test_every_new_trigger_and_view_is_required(self):
  original=self.candidate();c=s.design_contract(s.ROOT);old=s.v3.design_contract(s.ROOT)
  for item in s.trigger_rows(c,old)+s.view_rows(c,old):
   with self.subTest(item=item['id']):
    p=self.folder/('drop-'+str(self.counter)+'.sqlite3');self.counter+=1;shutil.copyfile(original,p)
    with closing(sqlite3.connect(p))as db:db.execute(('DROP VIEW 'if item['id'].startswith('V4-VIEW:')else'DROP TRIGGER ')+s.v1._name(item['name']));db.commit()
    self.assertTrue(s.v1.compare_schema_file(s.ROOT,p,'profile',s.CONTRACT))
 def test_dictionary_rows_relations_producer_claims_and_csv_are_exact(self):
  original=s.read_json(s.ROOT,s.DICTIONARY);reader=s.read_json;bytes_reader=s.safe_bytes
  for mode in ['field','relation','producer','trigger','view','type','unknown']:
   changed=copy.deepcopy(original)
   if mode=='field':changed['fields'].pop()
   elif mode=='relation':changed['relationships'].pop()
   elif mode=='producer':changed['fields'][0]['producer_status']='passed'
   elif mode=='trigger':changed['trigger_obligations'].pop()
   elif mode=='view':changed['view_obligations'].pop()
   elif mode=='type':changed['fields'][0]['primary_key_position']=True
   else:changed['unapproved']=True
   with patch.object(s,'read_json',side_effect=lambda root,path:changed if path==s.DICTIONARY else reader(root,path)):
    self.assertTrue(s.validate_field_dictionary(s.ROOT),mode)
  with patch.object(s,'safe_bytes',side_effect=lambda root,path:b'bad CSV'if path==s.CSV else bytes_reader(root,path)):
   self.assertTrue(s.validate_field_dictionary(s.ROOT))
 def test_frozen_design_prior_bytes_sidecars_symlink_and_corrupt_fk_fail(self):
  with patch.object(s,'DESIGN_SHA256','0'*64):self.assertTrue(s.validate_schema_contract(s.ROOT))
  p=self.candidate();before=p.read_bytes();link=self.folder/'link.sqlite3';link.symlink_to(p);self.assertTrue(s.compare_current_schema(s.ROOT,link))
  Path(str(p)+'-wal').write_bytes(b'uncheckpointed');self.assertTrue(s.compare_current_schema(s.ROOT,p));self.assertEqual(before,p.read_bytes())
  bad=self.candidate("PRAGMA foreign_keys=OFF;INSERT INTO oauth_grants(grant_id,client_profile_id,client_profile_revision,principal_state,created_at_ms) VALUES('orphan','missing',1,'unknown',0);")
  self.assertTrue(any('foreign key'in e.lower()for e in s.compare_current_schema(s.ROOT,bad)))
 def test_unapproved_table_or_index_drift_fails(self):
  self.assertTrue(s.compare_current_schema(s.ROOT,self.candidate('CREATE TABLE unreviewed(id TEXT);')))
  self.assertTrue(s.compare_current_schema(s.ROOT,self.candidate('DROP INDEX v4_binding_selection_identity;')))

if __name__=='__main__':unittest.main()
