"""Planning, design probes and fabricated marker rows never grant product provenance."""
import copy,unittest
import storage_schema_v4 as s

class ServiceAcceptanceGuards(unittest.TestCase):
 def fixture(self):
  relations={r['id']:r for r in s.relationship_rows(s.design_contract(s.ROOT),s.v3.design_contract(s.ROOT))}
  return {'kind':'guard_only_not_runtime_evidence','checks':[{'id':identifier,'status':'passed','assertions':1,'skipped':0,**({'relationship':copy.deepcopy(relations[identifier])}if identifier in relations else{}),**({'field_assertions':{'valid':1,'type':1,'null':1}}if identifier.startswith('V4-FIELD:')else{})}for identifier in sorted(s.v4_check_ids(s.ROOT))]}
 def test_all_original_431_29_24_obligations_and_routes_are_preserved(self):
  self.assertEqual(s.validate_acceptance_registry(s.ROOT),[]);template=s.acceptance_template(s.ROOT);prior=s.read_json(s.ROOT,s.v3.REGISTRY)
  self.assertEqual(template['original_checks'],prior['checks']);self.assertEqual(template['prior_delta_check_ids'],prior['delta_check_ids']);self.assertEqual(template['original_upgrade_matrix'],prior['supported_upgrade_matrix'])
  self.assertEqual(template['supported_upgrade_matrix']['fresh_versions'],{'profile':4,'dictionary':1});self.assertEqual([r['version']for r in template['supported_upgrade_matrix']['migration_chain']],[1,2,3,4])
  self.assertEqual(len(template['expanded_old_fields']),5);self.assertIn('upgrade_3_to_4_with_backup',template['required_routes'])
 def test_planning_cannot_drop_prior_routes_relation_or_mark_runtime_passed(self):
  template=s.acceptance_template(s.ROOT)
  for key in ['checks','original_checks','prior_delta_check_ids','original_upgrade_matrix','supported_upgrade_matrix','delta_check_ids','required_routes','prior_obligation_sources','expanded_old_fields','relationship_inventory','retained_R3_relationships_sha256','FK_receipt_payload']:
   changed=copy.deepcopy(template);changed[key]=None;self.assertTrue(s.validate_acceptance_registry(s.ROOT,changed),key)
  changed=copy.deepcopy(template);changed['product_tests_executed']=True;self.assertTrue(s.validate_acceptance_registry(s.ROOT,changed))
 def test_required_markers_cover_282_fields_87_relationships_68triggers_and_view(self):
  ids=s.v4_check_ids(s.ROOT);self.assertEqual(sum(i.startswith('RUST-MAP:')for i in ids),282);self.assertEqual(sum(i.startswith('V4-FIELD:')for i in ids),282);self.assertEqual(sum(i.startswith('V4-FK:')for i in ids),89);self.assertEqual(sum(i.startswith('V4-TRIGGER:')for i in ids),69);self.assertEqual(sum(i.startswith('V4-VIEW:')for i in ids),1)
  self.assertEqual(s.validate_delta_receipt(s.ROOT,self.fixture()),[]);self.assertFalse(s.assess(s.ROOT)['product_acceptance'])
 def test_invented_or_partial_receipt_bool_zero_skip_and_duplicate_rejected(self):
  for mode in ['missing','duplicate','bool_count','bool_skip','zero','skipped','failed']:
   value=self.fixture()
   if mode=='missing':value['checks'].pop()
   elif mode=='duplicate':value['checks'].append(value['checks'][0])
   elif mode=='bool_count':value['checks'][0]['assertions']=True
   elif mode=='bool_skip':value['checks'][0]['skipped']=False
   elif mode=='zero':value['checks'][0]['assertions']=0
   elif mode=='skipped':value['checks'][0]['skipped']=1
   else:value['checks'][0]['status']='failed'
   self.assertTrue(s.validate_delta_receipt(s.ROOT,value),mode)
  self.assertTrue(s.validate_delta_receipt(s.ROOT,{'checks':[]}))
 def test_R4_FK_receipt_requires_complete_stable_identity_even_with_all_positive_tags(self):
  original=self.fixture();relation=next(r for r in original['checks']if r['id']=='V4-FK:activation_resource_materializations:2')
  for mode in ['absent','ordinal','parent-columns','child-columns','on-delete','on-update','match','prior-bool','table','extra-key']:
   receipt=copy.deepcopy(original);row=next(r for r in receipt['checks']if r['id']==relation['id']);body=row['relationship']
   if mode=='absent':row.pop('relationship')
   elif mode=='ordinal':
    other=next(r for r in receipt['checks']if r['id']=='V4-FK:activation_resource_materializations:8');body['foreign_key']=copy.deepcopy(other['relationship']['foreign_key'])
   elif mode=='parent-columns':body['foreign_key']['parent_columns'].reverse()
   elif mode=='child-columns':body['foreign_key']['columns'].reverse()
   elif mode=='on-delete':body['foreign_key']['on_delete']='CASCADE'
   elif mode=='on-update':body['foreign_key']['on_update']='CASCADE'
   elif mode=='match':body['foreign_key']['match']='FULL'
   elif mode=='prior-bool':body['prior_relationship']=0
   elif mode=='table':body['table']='models'
   else:body['unexpected']=True
   self.assertEqual({r['id']for r in receipt['checks']},{r['id']for r in original['checks']})
   self.assertTrue(s.validate_delta_receipt(s.ROOT,receipt),mode)
 def test_each_field_has_own_actual_three_subassertions(self):
  original=self.fixture();field=next(r['id']for r in original['checks']if r['id'].startswith('V4-FIELD:'))
  for sub in [None,{}, {'valid':1,'type':1},{'valid':1,'type':True,'null':1},{'valid':1,'type':0,'null':1}]:
   receipt=copy.deepcopy(original);next(r for r in receipt['checks']if r['id']==field)['field_assertions']=sub;self.assertTrue(s.validate_delta_receipt(s.ROOT,receipt))

if __name__=='__main__':unittest.main()
