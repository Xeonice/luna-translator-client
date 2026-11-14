"""General visual-guard negatives. These tests never grant product/reference pass."""
import copy
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch
from PIL import Image
from current_general_visual_authority import ROOT,STATE_IDS,CANVASES,capture_cases,case_key,context,authority,source_inputs,validate_case,expected_mapping,PREFIX,APPROVAL,PROMOTION,RENDER_FILES
from current_general_visual_inspect import check_facts
from current_general_visual_recompute import pixels
from current_general_reference import assess


def facts():
 row=validate_case(STATE_IDS[0],'1000x720-dark-z1','primary');ctx=context(row,'candidate','0'*32)
 return row,{'context':ctx,'error':None,'facts':{'errors':[],'fontStatus':'loaded','theme':'dark','zoom':1,'geometry':{'frame':{'x':0,'y':0,'width':1000,'height':720}},'headings':['通用','阅读记录','本机偏好'],
 'switches':[{'label':label,'selected':v,'disabled':False} for label,v in zip(('可点击日语分词','显示日语注音','显示词性标记'),(True,True,False))],
 'scope':{'kind':'global','disabled':False,'expanded':False},'dirty':False,'menuOpen':False,'text':'','renderSource':'production-WindowRoot','startupWrites':[],'forbiddenEffects':[],
 'provenance':{'controlledTransport':True,'productionBootstrapImported':False,'osRuntimeVerified':False,'storageRuntimeVerified':False,'gameExecutionVerified':False},'controlledCommands':[],'saveCalls':[]}}


def native_display_guard_fixture():
 import base64,hashlib,struct
 profile=bytearray(128);profile[:4]=struct.pack('>I',128);profile[36:40]=b'acsp';profile=bytes(profile)
 raw={'schema_version':1,'kind':'general-native-display-inventory/v1','nonce':'0'*32,'phase':'pre_launch','pid':500,'ppid':400,'observed_at_us':1000000,'main_display_id':3,'active_display_count':1,'capabilities':{'read_only':True,'gui_constructed':False,'screen_capture_invoked':False,'system_mutation':False,'permissions_requested':False},'errors':[],'displays':[{'display_id':3,'uuid':'00000000-0000-0000-0000-000000000003','is_main':True,'is_active':True,'is_online':True,'is_builtin':False,'is_asleep':False,'vendor':1,'model':2,'serial':3,'rotation_degrees':0,'bounds':{'x':0,'y':0,'width':2560,'height':1440},'mode':{'width':2560,'height':1440,'pixel_width':5120,'pixel_height':2880,'refresh_rate':60,'io_flags':0},'color_space':{'name':'guard-only invented profile, never product evidence','model':1,'components':3,'icc':{'bytes':128,'sha256':hashlib.sha256(profile).hexdigest(),'base64':base64.b64encode(profile).decode()}}}]}
 return raw,profile

class GeneralVisualTests(unittest.TestCase):
 def test_finite_matrix_is_14_states_27_cases_and_8_base_canvases(self):
  rows=capture_cases();self.assertEqual(len(rows),27);self.assertEqual(set(v['state_id'] for v in rows),set(STATE_IDS));self.assertEqual(len({case_key(v) for v in rows}),27)
  self.assertEqual({v['canvas_id'] for v in rows if v['state_id']==STATE_IDS[0] and v['subcase']=='primary'},{v['canvas_id'] for v in CANVASES})
  self.assertEqual({v['subcase'] for v in rows if v['state_id']==STATE_IDS[2]},{'primary','initial'})
  for state,cid,sub in [('unknown','1000x720-dark-z1','primary'),(STATE_IDS[0],'https://evil','primary'),(STATE_IDS[0],'1000x720-dark-z1','initial'),(STATE_IDS[2],'1000x720-dark-z1','menu-open'),(STATE_IDS[0],'1000x720-dark-z1','content-end')]:
   with self.subTest(state=state,cid=cid,sub=sub),self.assertRaises(ValueError):validate_case(state,cid,sub)
 def test_context_rejects_second_canvas_representation_unknown_side_and_nonce(self):
  row=capture_cases()[0]
  for key,value in [('width',760),('height',560),('theme','light'),('zoom',2)]:
   changed={**row,key:value}
   with self.subTest(key=key),self.assertRaises(ValueError):context(changed,'candidate','0'*32)
  for side,nonce,transport in [('fake','0'*32,'real_react'),('candidate','z'*32,'real_react'),('candidate','0'*32,'prototype')]:
   with self.assertRaises(ValueError):context(row,side,nonce,transport)
 def test_approved_source_closure_is_exact_and_tampering_fails(self):
  original=source_inputs();self.assertEqual(len(original),9);self.assertEqual(authority()['rgba_threshold'],0);self.assertFalse(authority()['mask'])
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp)
   for relative in original:
    target=root/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/relative,target)
   self.assertEqual(source_inputs(root),original)
   target=root/PREFIX/RENDER_FILES[0];target.write_bytes(target.read_bytes()+b' ')
   with self.assertRaises(ValueError):source_inputs(root)
 def test_actual_facts_derive_expected_semantics_not_pass_labels(self):
  row,observation=facts();check_facts(row,observation,'candidate','real_react')
  mutations=[lambda f:f.update(fontStatus='loading'),lambda f:f.update(errors=['real failure']),lambda f:f.update(dirty=True),lambda f:f.update(menuOpen=True),lambda f:f['scope'].update(disabled=True),lambda f:f['switches'][0].update(selected=False),lambda f:f['geometry']['frame'].update(width=999),lambda f:f.update(startupWrites=['OS registration']),lambda f:f.update(forbiddenEffects=['database']),lambda f:f['provenance'].update(osRuntimeVerified=True),lambda f:f['provenance'].update(storageRuntimeVerified=True),lambda f:f['provenance'].update(gameExecutionVerified=True),lambda f:f.update(controlledCommands=[{'kind':'launch_game'}])]
  for mutate in mutations:
   broken=copy.deepcopy(observation);mutate(broken['facts']);broken['passed']=True
   with self.subTest(mutate=str(mutate)),self.assertRaises(ValueError):check_facts(row,broken,'candidate','real_react')
 def test_native_requires_actual_acl_denial_and_unknown_business_handler(self):
  row,observation=facts();observation['context']=context(row,'candidate','1'*32,'packaged_native')
  with self.assertRaises(ValueError):check_facts(row,observation,'candidate','packaged_native')
  observation['facts']['nativeAcl']={'coreCommand':'plugin:window|scale_factor','coreDenied':True,'coreError':'not allowed by ACL','businessCommand':'application_snapshot','businessUnknown':True,'businessError':'Command application_snapshot not found'}
  check_facts(row,observation,'candidate','packaged_native')
  for key,value in [('coreDenied',False),('coreError','unknown command'),('businessUnknown',False),('businessError','unexpectedly registered'),('businessError','database not found'),('businessError','state not found'),('businessError','resource not found')]:
   broken=copy.deepcopy(observation);broken['facts']['nativeAcl'][key]=value
   with self.assertRaises(ValueError):check_facts(row,broken,'candidate','packaged_native')
 def test_independent_original_pixels_reject_one_rgba_pixel_and_resize(self):
  with tempfile.TemporaryDirectory() as temp:
   a,b=Path(temp)/'a.png',Path(temp)/'b.png';Image.new('RGBA',(2,2),(1,2,3,255)).save(a);shutil.copyfile(a,b)
   self.assertEqual(pixels(a,b)['different_rgba_pixels'],0)
   image=Image.open(b);image.putpixel((1,1),(1,2,3,254));image.save(b);self.assertEqual(pixels(a,b)['different_rgba_pixels'],1)
   Image.new('RGBA',(1,2),(1,2,3,255)).save(b)
   with self.assertRaises(ValueError):pixels(a,b)
 def test_missing_reference_and_review_never_pass(self):
  with tempfile.TemporaryDirectory() as temp:
   result=assess(Path(temp),STATE_IDS,'real_react');self.assertNotEqual(result['status'],'passed');self.assertTrue(result['errors'] or result['gaps']);self.assertFalse(result['product_alignment_approved'])
 def test_registry_has_separate_inspect_recompute_and_native_never_grants_game(self):
  import current_design_alignment as alignment
  for identity in ('product-general-visual-react-tests','product-general-visual-native-tests'):
   adapter=alignment.ADAPTERS[identity];self.assertEqual(adapter.states,frozenset(STATE_IDS));self.assertEqual(adapter.capture_scope,'whole_client');self.assertFalse(adapter.game_execution)
   self.assertEqual(len({id(adapter.execute),id(adapter.inspect),id(adapter.recompute)}),3)
  source=(ROOT/'scripts/quality/current_design_alignment.py').read_text()
  self.assertIn('errors = all_pages.validate_contract(root)',source);self.assertIn("errors += _approval_gaps(root) + _design_state_gaps(root, STATE_IDS)",source)
 def test_g01_obligations_and_original_chromium_diagnostics_are_retained(self):
  source=(ROOT/'apps/desktop/tests/live-preview/verify-general-settings-structure.mjs').read_text()
  self.assertIn("for (const width of [1000, 760]) for (const theme of ['dark', 'light']) for (const zoom of [1, 2])",source)
  self.assertIn('originalHtml.replace(',source);self.assertIn('assert.equal(game.differingPixels, 0)',source)
  self.assertGreaterEqual(source.count('assert.equal(diff.differingPixels, 0'),3)
  self.assertIn("qualification: 'not_qualified'",source);self.assertIn('savedGame: true',source);self.assertIn('base: blank',source)
  self.assertNotIn("test.skip('GENERAL-01",source);self.assertNotIn("test.skip('GENERAL-17",source)
 def test_real_requirement_metadata_validator_rejects_missing_wrong_source_and_typed_canvas(self):
  from current_general_visual_authority import metadata_gaps
  metadata=load_metadata=json.loads((ROOT/'openspec/changes/close-general-page-visual-contract/verification.json').read_text())
  self.assertEqual([],metadata_gaps(ROOT,metadata))
  index=next(i for i,v in enumerate(metadata['requirements']) if v.get('id')=='GVC-002')
  for change in ('missing','entry','state','typed'):
   value=copy.deepcopy(metadata);mapping=value['requirements'][index]['general_visual_source']
   if change=='missing':value['requirements'][index].pop('general_visual_source')
   if change=='entry':mapping['entry']='docs/design/unreviewed/index.html'
   if change=='state':mapping['state_ids'].pop()
   if change=='typed':mapping['schema_version']=True
   self.assertTrue(metadata_gaps(ROOT,value),change)
 def test_initial_source_visibility_is_derived_and_candidate_controls_are_absent(self):
  from current_general_visual_recompute import compare_semantics
  row,source=facts();row=validate_case(STATE_IDS[2],'1000x720-dark-z1','initial')
  source['context']=context(row,'reference','2'*32);f=source['facts'];f.update(text='通用 正在读取已保存的阅读偏好来源… 阅读记录 本机偏好',renderSource='independent-user-approved-general-html');f['scope']['disabled']=True;f['geometry']['lexical_card']=None
  f['provenance']={'osRuntimeVerified':False,'storageRuntimeVerified':False,'gameTestsExecuted':False}
  candidate=copy.deepcopy(source);candidate['context']=context(row,'candidate','3'*32);candidate['facts'].update(renderSource='production-WindowRoot',switches=[]);candidate['facts']['provenance']={'controlledTransport':True,'productionBootstrapImported':False,'osRuntimeVerified':False,'storageRuntimeVerified':False,'gameExecutionVerified':False}
  check_facts(row,source,'reference','real_react');check_facts(row,candidate,'candidate','real_react');compare_semantics(source,candidate)
  for change in ('source_card','source_label','candidate_card','candidate_hidden_control'):
   a,b=copy.deepcopy(source),copy.deepcopy(candidate)
   if change=='source_card':a['facts']['geometry']['lexical_card']={'width':100,'height':100}
   if change=='source_label':a['facts']['text']+=' 可点击日语分词'
   if change=='candidate_card':b['facts']['geometry']['lexical_card']={'width':100,'height':100}
   if change=='candidate_hidden_control':b['facts']['switches']=[{'label':'可点击日语分词','selected':True,'disabled':True}]
   with self.subTest(change=change),self.assertRaises(ValueError):
    check_facts(row,a,'reference','real_react');check_facts(row,b,'candidate','real_react');compare_semantics(a,b)
 def test_native_original_process_binding_rejects_exe_start_code_parent_and_boot_confusion(self):
  from current_general_visual_inspect import native_process_binding
  with tempfile.TemporaryDirectory() as temporary:
   binary,controller=Path(temporary)/'renderer',Path(temporary)/'controller';binary.write_bytes(b'fixture executable identity');controller.write_bytes(b'fixture controller identity')
   signature={'identifier':'com.lunatranslator.reader','team_identifier':'fixture-team','cdhash':'a'*40}
   def process(path,pid,ppid,usec):
    stat=path.stat();return {'pid':pid,'ppid':ppid,'executable':str(path),'device':stat.st_dev,'inode':stat.st_ino,'start_sec':2,'start_usec':usec,'bsd_bytes':136,'path_bytes':len(str(path)),'guest_code':{'guest_status':0,'validity':0,'information_status':0,'identifier':signature['identifier'],'team':signature['team_identifier'],'cdhash':signature['cdhash']}}
   original={'kind':'independent_macos_runtime_observation','schema_version':1,'boot_status':0,'boot_sec':1,'boot_usec':0,'screen_capture_preflight':True,'processes':[process(binary,123,456,100),process(controller,456,789,0)]}
   call=lambda o:native_process_binding(o,123,binary,signature,456,str(controller),789,1500000,3000000)
   call(original)
   mutations=[lambda o:o['processes'][0].update(executable='/other/exe'),lambda o:o['processes'][0].update(start_sec=1),lambda o:o['processes'][0].update(start_usec=None),lambda o:o['processes'][0].update(ppid=999),lambda o:o['processes'][0].update(device=0),lambda o:o['processes'][0].update(inode=0),lambda o:o['processes'][0]['guest_code'].update(cdhash='b'*40),lambda o:o['processes'][0]['guest_code'].update(validity=-1),lambda o:o['processes'][0]['guest_code'].update(identifier='other'),lambda o:o['processes'][0]['guest_code'].update(team='other'),lambda o:o['processes'][1].update(ppid=999),lambda o:o.update(boot_status=False),lambda o:o.update(schema_version=True)]
   for mutate in mutations:
    changed=copy.deepcopy(original);mutate(changed)
    with self.assertRaises(ValueError):call(changed)
 def test_native_reference_and_candidate_both_reject_wrong_original_source_package(self):
  from current_general_visual_inspect import raw_capture
  from current_general_visual_recompute import reference_observation
  from gate_core import digest
  with tempfile.TemporaryDirectory() as temporary:
   root=Path(temporary).resolve();out=root/'.build/native-case';out.mkdir(parents=True)
   app=root/'.build/invalid-package/Luna.app';binary=app/'Contents/MacOS/luna-reader';binary.parent.mkdir(parents=True);binary.write_text('luna-general-native-isolated-renderer/v1:20261108');archive=app.parent/'invalid.zip';archive.write_text('not a signed source archive')
   package={'app':str(app),'binary':str(binary),'archive':str(archive),'source_tree_sha256':'a'*64}
   row=capture_cases()[0]
   for side in ('reference','candidate'):
    raw={'kind':'general-packaged-native-render-capture/v1','status':'captured','errors':[],'side':side,'state_id':row['state_id'],'canvas_id':row['canvas_id'],'subcase':row['subcase'],'game_tests_executed':False,'os_runtime_verified':False,'storage_runtime_verified':False,'source_tree_sha256':'b'*64,'package':package}
    (out/'capture.json').write_text(json.dumps(raw));(out/'observation.json').write_text(json.dumps({'kind':'general-native-raw-observation/v1'}))
    if side=='candidate':
     with self.assertRaisesRegex(ValueError,'source differs'):raw_capture(root,{'case':row,'raw_directory':out.relative_to(root).as_posix()},'packaged_native',side)
    else:
     ref={'case':row,'facts':{'path':(out/'observation.json').relative_to(root).as_posix(),'sha256':digest(out/'observation.json')},'png':{'path':(out/'frame-1.png').relative_to(root).as_posix(),'sha256':'0'*64}}
     with self.assertRaisesRegex(ValueError,'source differs'):reference_observation(root,ref,'packaged_native')
 def test_actual_framework_controller_and_direct_runtime_keep_exact_os_and_code_identity(self):
  from current_general_visual_inspect import controller_runtime_identity,native_process_binding
  declared=str(Path(sys.executable).resolve());identity=controller_runtime_identity(declared);runtime=Path(identity['runtime_executable'])
  self.assertEqual('cpython_framework_launcher',identity['kind']);self.assertEqual(runtime,Path(declared).parent.parent/'Resources/Python.app/Contents/MacOS/Python')
  direct=controller_runtime_identity(str(runtime));self.assertEqual('direct',direct['kind']);self.assertEqual(identity['runtime_signature'],direct['runtime_signature'])
  with tempfile.TemporaryDirectory() as temporary:
   binary=Path(temporary)/'renderer';binary.write_text('pure process-binding fixture, never executable');signature={'identifier':'fixture-renderer','team_identifier':'fixture','cdhash':'a'*40}
   def process(path,pid,ppid,usec,code):
    stat=path.stat();return {'pid':pid,'ppid':ppid,'executable':str(path),'device':stat.st_dev,'inode':stat.st_ino,'start_sec':2,'start_usec':usec,'bsd_bytes':136,'path_bytes':len(str(path)),'guest_code':{'guest_status':0,'validity':0,'information_status':0,'identifier':code['identifier'],'team':code['team_identifier'],'cdhash':code['cdhash']}}
   original={'kind':'independent_macos_runtime_observation','schema_version':1,'boot_status':0,'boot_sec':1,'boot_usec':0,'screen_capture_preflight':True,'processes':[process(binary,123,456,100,signature),process(runtime,456,789,0,identity['runtime_signature'])]}
   def model(raw,expected):return native_process_binding(raw,123,binary,signature,456,expected['declared_executable'],789,1500000,3000000,expected)
   model(original,identity);model(original,direct)
   for key,value in [('executable',str(runtime).replace('Python.framework','Other.framework')),('executable',str(runtime).replace('/Versions/'+identity['framework_version']+'/', '/Versions/99.99/')),('executable',str(runtime.with_name('Python-other'))),('executable','/tmp/Python.app/Contents/MacOS/Python'),('inode',0),('device',0),('ppid',999),('start_sec',1)]:
    raw=copy.deepcopy(original);raw['processes'][1][key]=value
    with self.subTest(key=key,value=value),self.assertRaises(ValueError):model(raw,identity)
   for key,value in [('cdhash','f'*40),('identifier','another-code'),('team','another-team'),('validity',-1),('information_status',False)]:
    raw=copy.deepcopy(original);raw['processes'][1]['guest_code'][key]=value
    with self.subTest(code_field=key),self.assertRaises(ValueError):model(raw,identity)
 def test_actual_controller_mapping_rejects_wrong_framework_version_metadata_and_links(self):
  from current_general_visual_inspect import controller_runtime_identity
  import plistlib
  declared=Path(sys.executable).resolve();version_root=declared.parent.parent;runtime=version_root/'Resources/Python.app/Contents/MacOS/Python'
  with tempfile.TemporaryDirectory() as temporary:
   for name in ('wrong_version','wrong_bundle_version','foreign_framework_link'):
    version='99.99' if name=='wrong_version' else version_root.name
    base=Path(temporary).resolve()/name/'Python.framework/Versions'/version;launcher=base/'bin'/('python'+version);target=base/'Resources/Python.app/Contents/MacOS/Python'
    for source,destination in ((declared,launcher),(runtime,target),(version_root/'Python',base/'Python'),(version_root/'Resources/Info.plist',base/'Resources/Info.plist'),(runtime.parent.parent/'Info.plist',target.parent.parent/'Info.plist')):
     destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,destination)
    if name=='wrong_bundle_version':
     info=plistlib.loads((target.parent.parent/'Info.plist').read_bytes());info['CFBundleVersion']='99.99.1';(target.parent.parent/'Info.plist').write_bytes(plistlib.dumps(info))
    # Even copied genuine Mach-O code is rejected when its install-name still
    # links the original Framework rather than this claimed same-version root.
    reason={'wrong_version':'version does not match launcher','wrong_bundle_version':'executable/version metadata differs','foreign_framework_link':'do not link the same exact Framework library'}[name]
    with self.subTest(name=name),self.assertRaisesRegex(ValueError,reason):controller_runtime_identity(str(launcher))
 def test_actual_controller_helper_normalizes_timeout_with_bounded_command_diagnostics(self):
  import subprocess
  from current_general_visual_inspect import controller_runtime_identity
  runtime=controller_runtime_identity(str(Path(sys.executable).resolve()))['runtime_executable']
  for output,stderr in [(b'x'*100000+b'UNBOUNDED_SENTINEL',b'y'*100000+b'UNBOUNDED_SENTINEL'),('x'*100000+'UNBOUNDED_SENTINEL','y'*100000+'UNBOUNDED_SENTINEL'),(None,None)]:
   error=subprocess.TimeoutExpired(['/usr/bin/codesign'],15,output=output,stderr=stderr)
   with self.subTest(output_type=type(output).__name__),patch('subprocess.run',side_effect=error) as external,self.assertRaises(ValueError) as caught:controller_runtime_identity(runtime)
   message=str(caught.exception);self.assertIn('timed out',message);self.assertIn('/usr/bin/codesign',message);self.assertIn('timeout=15',message);self.assertIn('stdout=',message);self.assertIn('stderr=',message);self.assertNotIn('UNBOUNDED_SENTINEL',message);self.assertLess(len(message),5000);self.assertIs(caught.exception.__cause__,error)
   self.assertEqual(15,external.call_args.kwargs['timeout'])
 def test_native_uniform_fixed_settle_is_recorded_and_recomputed_without_wait_or_gui(self):
  import runpy
  from unittest.mock import Mock
  from current_general_visual_inspect import inspect_native_settle,_micros
  namespace=runpy.run_path(str(ROOT/'scripts/report/verify_general_native_visual.py'),run_name='general_settle_guard');settle=namespace['settle_before_frames']
  start='2026-11-08T00:00:00.100000+00:00';end='2026-11-08T00:00:03.350000+00:00'
  with patch.dict(settle.__globals__,{'now':Mock(side_effect=[start,end])}),patch.object(namespace['time'],'monotonic',side_effect=[10,13.25]),patch.object(namespace['time'],'sleep') as sleep:
   actual=settle();sleep.assert_called_once_with(3)
  self.assertEqual({'kind':'fixed-native-pre-capture-settle/v1','planned_seconds':3,'started_at':start,'finished_at':end,'started_monotonic':10,'finished_monotonic':13.25,'elapsed_monotonic_seconds':3.25},actual)
  raw={'pre_capture_settle':actual,'frames':[{'captured_at':'2026-11-08T00:00:03.500000+00:00'}]};before={'observed_at_ms':_micros('2026-11-08T00:00:00+00:00')//1000};after={'observed_at_ms':_micros('2026-11-08T00:00:04+00:00')//1000};inspect_native_settle(raw,before,after)
  for name in ('missing','short_plan','bool_plan','short_monotonic','wrong_monotonic_end','not_finite','short_wall_clock','after_first_frame','before_os'):
   broken=copy.deepcopy(raw);s=broken['pre_capture_settle']
   if name=='missing':broken.pop('pre_capture_settle')
   elif name=='short_plan':s['planned_seconds']=0
   elif name=='bool_plan':s['planned_seconds']=True
   elif name=='short_monotonic':s['elapsed_monotonic_seconds']=2.99
   elif name=='wrong_monotonic_end':s['finished_monotonic']=14
   elif name=='not_finite':s['elapsed_monotonic_seconds']=float('inf')
   elif name=='short_wall_clock':s['finished_at']='2026-11-08T00:00:03+00:00'
   elif name=='after_first_frame':s['finished_at']='2026-11-08T00:00:03.600000+00:00'
   elif name=='before_os':s['started_at']='2026-11-07T23:59:59+00:00'
   with self.subTest(name=name),self.assertRaises(ValueError):inspect_native_settle(broken,before,after)
 def test_source_and_product_graphs_remain_separate_entries(self):
  entry=(ROOT/'apps/desktop/tests/native/general/entry.tsx').read_text();self.assertIn('WindowRoot',entry);self.assertIn('ApplicationStore',entry)
  replay=(ROOT/'apps/desktop/tests/native/general/reference-replay.js').read_text();self.assertNotIn('/src/',replay);self.assertNotIn('react-dom',replay)
  for p in (ROOT/'apps/desktop/src').rglob('*'):
   if p.suffix in ('.ts','.tsx','.js'):self.assertNotIn('tests/native/general',p.read_text(),str(p))
 def test_global_reference_assessment_splits_general_without_inheriting_old_general(self):
  import current_reference_catalogue as catalogue
  from current_design_alignment import STATE_IDS as all_ids
  general=set(STATE_IDS);other=set(all_ids)-general
  def receipt(approved,*,errors=None,gaps=None):
   return {'status':'failed' if errors else 'blocked' if gaps else 'passed','errors':errors or [],'gaps':gaps or [],'approved_state_ids':sorted(approved),'product_alignment_approved':False,'native_verified':False}
  # The old authority may truthfully retain its reviewed historical normal row;
  # it must never cover any part of the newly approved General14 authority.
  legacy=receipt(other|{STATE_IDS[0]});new=receipt(general)
  with patch.object(catalogue,'authoritative_context',return_value={'descriptors':{'guard-state':{}}}),patch.object(catalogue,'_validate',return_value=copy.deepcopy(legacy)) as validator:
   result=catalogue._assess_legacy(ROOT,other)
   self.assertNotIn(STATE_IDS[0],result['approved_state_ids']);self.assertEqual(other,set(result['approved_state_ids']))
   self.assertEqual(other,validator.call_args.args[1])
  with patch.object(catalogue,'_assess_legacy',return_value=legacy) as old_call,patch('current_general_reference.assess',return_value=new) as new_call:
   result=catalogue.assess(ROOT)
   self.assertEqual('passed',result['status']);self.assertEqual(set(all_ids),set(result['approved_state_ids']))
   old_call.assert_called_once_with(ROOT,other);new_call.assert_called_once_with(ROOT,general)
   self.assertFalse(result['product_alignment_approved']);self.assertFalse(result['native_verified'])
  # General cannot come from old evidence when its own authority is missing,
  # invalid or incomplete; General success cannot repair the other authority.
  for old,new_value,expected in [
   (legacy,receipt(set(),gaps=['new General absent']),'blocked'),
   (legacy,receipt(set(),errors=['new General source mismatch']),'failed'),
   (receipt(set(),gaps=['other795 absent']),new,'blocked'),
   (receipt(set(),errors=['global AST drift']),new,'failed'),
   (legacy,receipt(general-{STATE_IDS[0]}),'blocked')]:
   with self.subTest(expected=expected),patch.object(catalogue,'_assess_legacy',return_value=old),patch('current_general_reference.assess',return_value=new_value):
    result=catalogue.assess(ROOT);self.assertEqual(expected,result['status'])
    if STATE_IDS[0] not in new_value['approved_state_ids']:self.assertNotIn(STATE_IDS[0],result['approved_state_ids'])
    self.assertEqual(set(old['approved_state_ids']) & other,set(result['approved_state_ids']) & other)
 def test_reference_split_preserves_exact_scope_rejects_unknown_and_deduplicates_gaps(self):
  import current_reference_catalogue as catalogue
  from current_design_alignment import STATE_IDS as all_ids
  other=next(iter(set(all_ids)-set(STATE_IDS)));wanted={STATE_IDS[0],other}
  receipt={'status':'blocked','errors':[],'gaps':['missing independent reference'],'approved_state_ids':[]}
  with patch.object(catalogue,'_assess_legacy',return_value=copy.deepcopy(receipt)) as old_call,patch('current_general_reference.assess',return_value=copy.deepcopy(receipt)) as new_call:
   result=catalogue.assess(ROOT,wanted);self.assertEqual(sorted(wanted),result['required_state_ids'])
   old_call.assert_called_once_with(ROOT,{other});new_call.assert_called_once_with(ROOT,{STATE_IDS[0]})
  with patch.object(catalogue,'assess',return_value=receipt) as assessment,patch('current_general_reference.reference_gaps',side_effect=AssertionError('General checked twice')):
   self.assertEqual(receipt['gaps'],catalogue.reference_gaps(ROOT,wanted));assessment.assert_called_once_with(ROOT,wanted)
  for wanted in (set(),{'unknown-reference-state'}):
   with patch.object(catalogue,'_assess_legacy',side_effect=AssertionError('unknown scope reached legacy')),patch('current_general_reference.assess',side_effect=AssertionError('unknown scope reached General')):
    result=catalogue.assess(ROOT,wanted);self.assertEqual('failed',result['status']);self.assertEqual([],result['approved_state_ids'])
 def test_frozen_general_lineage_validates_old_bytes_but_does_not_replace_current_authority(self):
  import current_reference_catalogue as catalogue
  with tempfile.TemporaryDirectory() as temporary:
   root=Path(temporary)
   nongeneral={'descriptors':{'guard-state':{'id':'guard-state'}}}
   self.assertIs(nongeneral,catalogue._legacy_context(root,nongeneral))
   target=root/catalogue.GENERAL_LINEAGE;target.parent.mkdir(parents=True);shutil.copyfile(ROOT/catalogue.GENERAL_LINEAGE,target)
   original={'descriptors':{STATE_IDS[0]:{'id':STATE_IDS[0],'design_recipe':'current approved General source'}}}
   result=catalogue._legacy_context(root,original)
   self.assertEqual('current approved General source',original['descriptors'][STATE_IDS[0]]['design_recipe'])
   self.assertEqual(14,len(result['descriptors']))
   self.assertIn('guided-service-reading-20261030/index.html',result['descriptors'][STATE_IDS[0]]['design_recipe'][0]['path'])
   target.write_bytes(target.read_bytes()+b' ')
   with self.assertRaisesRegex(ValueError,'historical General descriptors changed'):catalogue._legacy_context(root,original)

 def test_pixel_cache_rereads_complete_bytes_at_same_path_mtime_and_returns_fresh_results(self):
  import os
  import current_general_visual_recompute as recompute
  recompute._cached_png_comparison.cache_clear();self.addCleanup(recompute._cached_png_comparison.cache_clear)
  with tempfile.TemporaryDirectory() as temporary:
   a,b=Path(temporary)/'a.png',Path(temporary)/'b.png';Image.new('RGBA',(2,2),(1,2,3,255)).save(a);shutil.copyfile(a,b);stamp=b.stat()
   with patch.object(recompute,'png_pixels',wraps=recompute.png_pixels) as decode:
    first=recompute.pixels(a,b);self.assertEqual(2,decode.call_count);self.assertEqual(0,first['different_rgba_pixels'])
    first.update(different_rgba_pixels=999,width=999,mask=True)
    second=recompute.pixels(a,b);self.assertIsNot(first,second);self.assertEqual(0,second['different_rgba_pixels']);self.assertEqual(2,second['width']);self.assertFalse(second['mask']);self.assertEqual(2,decode.call_count)
    image=Image.open(b);image.putpixel((1,1),(1,2,3,254));image.save(b);os.utime(b,ns=(stamp.st_atime_ns,stamp.st_mtime_ns));self.assertEqual(stamp.st_mtime_ns,b.stat().st_mtime_ns)
    self.assertEqual(1,recompute.pixels(a,b)['different_rgba_pixels']);self.assertEqual(4,decode.call_count)
 def test_pixel_cache_identical_corrupt_bytes_and_dimension_errors_are_never_cached(self):
  import current_general_visual_recompute as recompute
  recompute._cached_png_comparison.cache_clear();self.addCleanup(recompute._cached_png_comparison.cache_clear)
  with tempfile.TemporaryDirectory() as temporary:
   a,b=Path(temporary)/'a.png',Path(temporary)/'b.png';Image.new('RGBA',(2,2),(1,2,3,255)).save(a);b.write_bytes(a.read_bytes()[:-1])
   with patch.object(recompute,'png_pixels',wraps=recompute.png_pixels) as decode:
    for _ in range(2):
     with self.assertRaises(ValueError):recompute.pixels(b,b)
    self.assertEqual(2,decode.call_count)
    Image.new('RGBA',(1,2),(1,2,3,255)).save(b)
    for _ in range(2):
     with self.assertRaisesRegex(ValueError,'dimensions differ'):recompute.pixels(a,b)
    self.assertEqual(6,decode.call_count)
   self.assertEqual(0,recompute._cached_png_comparison.cache_info().currsize);self.assertEqual(0,recompute._cached_png_comparison.cache_info().hits)
 def test_pixel_cache_capacity_is_bounded_and_large_valid_bytes_keep_uncached_validation(self):
  import io,struct,zlib
  import current_general_visual_recompute as recompute
  recompute._cached_png_comparison.cache_clear();self.addCleanup(recompute._cached_png_comparison.cache_clear)
  def image(color):
   data=io.BytesIO();Image.new('RGBA',(1,1),color).save(data,format='PNG');return data.getvalue()
  left=image((255,255,255,255));rights=[image((i,0,0,255)) for i in range(130)]
  for right in rights:self.assertIsInstance(recompute._cached_png_comparison(left,right),tuple)
  info=recompute._cached_png_comparison.cache_info();self.assertEqual(128,info.maxsize);self.assertEqual(128,info.currsize)
  recompute._cached_png_comparison(left,rights[-1]);self.assertEqual(info.hits+1,recompute._cached_png_comparison.cache_info().hits)
  recompute._cached_png_comparison(left,rights[0]);self.assertEqual(info.misses+1,recompute._cached_png_comparison.cache_info().misses)
  payload=b'x'*recompute.PNG_COMPARISON_CACHE_MAX_BYTES;kind=b'vpAg';chunk=struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload)&0xffffffff);large=left[:-12]+chunk+left[-12:]
  with tempfile.TemporaryDirectory() as temporary:
   a,b=Path(temporary)/'a.png',Path(temporary)/'b.png';a.write_bytes(left);b.write_bytes(large);before=recompute._cached_png_comparison.cache_info()
   with patch.object(recompute,'png_pixels',wraps=recompute.png_pixels) as decode:
    for _ in range(2):self.assertEqual(0,recompute.pixels(a,b)['different_rgba_pixels'])
    self.assertEqual(4,decode.call_count)
   self.assertEqual(before,recompute._cached_png_comparison.cache_info())

 def test_native_display_raw_schema_whole_primary_bounds_and_profile_drift_reject(self):
  import current_general_native_display as display
  raw,_=native_display_guard_fixture();display.validate_probe(raw,'0'*32,'pre_launch');environment=display.primary_environment(raw);display.validate_environment(environment)
  window={'X':780,'Y':375,'Width':1000,'Height':720};display.require_primary_window(environment,window,2)
  for key,value in [('schema_version',True),('nonce','f'*32),('phase','after_capture'),('active_display_count',0),('main_display_id',True)]:
   altered=copy.deepcopy(raw);altered[key]=value
   with self.subTest(field=key),self.assertRaises(ValueError):display.validate_probe(altered,'0'*32,'pre_launch')
  for key,value in [('is_main',1),('is_active',False),('is_asleep',True),('uuid','wrong'),('rotation_degrees',45)]:
   altered=copy.deepcopy(raw);altered['displays'][0][key]=value
   with self.subTest(field=key),self.assertRaises(ValueError):display.primary_environment(altered)
  for key,value in [('sha256','f'*64),('base64','x'*20),('bytes',True)]:
   altered=copy.deepcopy(raw);altered['displays'][0]['color_space']['icc'][key]=value
   with self.subTest(icc=key),self.assertRaises(ValueError):display.primary_environment(altered)
  for bounds,scale in [({'X':1900,'Y':375,'Width':1000,'Height':720},2),({'X':3000,'Y':375,'Width':1000,'Height':720},2),(window,1)]:
   with self.assertRaises(ValueError):display.require_primary_window(environment,bounds,scale)
  for key,value in [('display_id',4),('uuid','00000000-0000-0000-0000-000000000004'),('rotation_degrees',180)]:
   altered=copy.deepcopy(environment);altered['primary'][key]=value
   with self.assertRaises(ValueError):display.require_same_environment(environment,altered)
 def test_native_original_png_iccp_is_hashed_without_conversion_and_corruption_rejected(self):
  import io,struct,zlib
  import current_general_native_display as display
  raw,profile=native_display_guard_fixture();environment=display.primary_environment(raw);buffer=io.BytesIO();Image.new('RGBA',(1,1),(1,2,3,255)).save(buffer,format='PNG');plain=buffer.getvalue()
  def chunk(kind,body):return struct.pack('>I',len(body))+kind+body+struct.pack('>I',zlib.crc32(kind+body)&0xffffffff)
  payload=b'Guard profile\0\0'+zlib.compress(profile);tag=chunk(b'iCCP',payload);png=plain[:33]+tag+plain[33:];original=bytes(png);observed=display.png_iccp(png);display.require_png_profile(environment,observed);self.assertEqual(original,png)
  damaged=bytearray(png);damaged[45]^=1
  srgb=chunk(b'sRGB',b'\0');palette=chunk(b'PLTE',b'\0\0\0')
  cases=[plain,png[:33]+tag+png[33:],bytes(damaged),png[:-1],plain[:33]+chunk(b'iCCP',b'Guard\0\1'+zlib.compress(profile))+plain[33:],plain[:33]+chunk(b'iCCP',payload+b'trailing')+plain[33:],plain[:33]+srgb+tag+plain[33:],png[:33]+tag+srgb+plain[33:],plain[:-12]+tag+plain[-12:],plain[:33]+palette+tag+plain[33:]]
  for value in cases:
   with self.assertRaises(ValueError):display.png_iccp(value)
  altered=copy.deepcopy(observed);altered['sha256']='f'*64
  with self.assertRaises(ValueError):display.require_png_profile(environment,altered)
 def test_native_batch_lock_requires_fresh_fixed_first_case_and_original_parent_cohort(self):
  import os
  import current_general_native_display as display
  environment=display.primary_environment(native_display_guard_fixture()[0]);row=capture_cases()[0];producer={'guard-only':{'sha256':'a'*64,'bytes':1}};coordinator={'process':{'pid':700,'start_sec':1}}
  with tempfile.TemporaryDirectory() as temporary:
   root=Path(temporary).resolve();parent=root/'.build/batch';directory=parent/case_key(row);directory.mkdir(parents=True,mode=0o700)
   reference=display.batch_lock(root,directory,row,'reference','a'*64,producer,environment,coordinator,None);lock=parent/display.LOCK_NAME;before=lock.read_bytes()
   self.assertEqual(before,lock.read_bytes());self.assertEqual(0o600,lock.stat().st_mode&0o777)
   extra=json.loads(before);extra['unexpected_environment_override']='https://untrusted.invalid';lock.write_text(json.dumps(extra))
   with self.assertRaises(ValueError):display.batch_lock(root,directory,row,'reference','a'*64,producer,environment,coordinator,None)
   lock.write_bytes(before)
   for changes in [('coordinator',{'process':{'pid':701,'start_sec':1}}),('source','b'*64),('producer',{}),('side','candidate')]:
    args={'side':'reference','source':'a'*64,'producer':producer,'environment':environment,'coordinator':coordinator,'approval':None};args[changes[0]]=changes[1]
    with self.subTest(field=changes[0]),self.assertRaises(ValueError):display.batch_lock(root,directory,row,**args)
   lock.chmod(0o644)
   with self.assertRaises(ValueError):display.batch_lock(root,directory,row,'reference','a'*64,producer,environment,coordinator,None)
   lock.chmod(0o600);saved=root/'.build/saved-lock';lock.rename(saved);lock.symlink_to(saved)
   with self.assertRaises(ValueError):display.batch_lock(root,directory,row,'reference','a'*64,producer,environment,coordinator,None)
   lock.unlink();saved.rename(lock);self.assertEqual(reference,display.file_ref(root,lock))
   copied=root/'.build/copied';newdir=copied/case_key(row);newdir.mkdir(parents=True);shutil.copyfile(lock,copied/display.LOCK_NAME);(copied/display.LOCK_NAME).chmod(0o600)
   with self.assertRaises(ValueError):display.batch_lock(root,newdir,row,'reference','a'*64,producer,environment,coordinator,None)
   late=root/'.build/late'/case_key(capture_cases()[1]);late.mkdir(parents=True)
   with self.assertRaises(ValueError):display.batch_lock(root,late,capture_cases()[1],'reference','a'*64,producer,environment,coordinator,None)
 def test_native_batch_complete_successful_prefix_and_sidecars_cannot_resume_failure(self):
  import current_general_native_display as display
  raw,_=native_display_guard_fixture();environment=display.primary_environment(raw);rows=list(capture_cases());producer={'guard-only':{'sha256':'a'*64,'bytes':1}};coordinator={'process':{'pid':700,'start_sec':1}}
  with tempfile.TemporaryDirectory() as temporary:
   root=Path(temporary).resolve();parent=root/'.build/batch';first=parent/case_key(rows[0]);first.mkdir(parents=True);reference=display.batch_lock(root,first,rows[0],'reference','a'*64,producer,environment,coordinator,None)
   frames=[]
   for name in ('frame-1.png','frame-2.png'):
    file=first/name;file.write_bytes(b'guard-only fake frame; never Native evidence');frames.append({'path':name,'sha256':display.sha(file.read_bytes())})
   record={'schema_version':1,'kind':'general-packaged-native-render-capture/v1','status':'captured','errors':[],'exit_code':0,'side':'reference','source_tree_sha256':'a'*64,'controller_parent_pid':700,'controller_pid':600,'pid':500,'execution_nonce':'1'*32,'package':{'app':str(root/'.build/guard.app')},'display_environment':{'batch_lock':reference,'environment':environment,'producer_inputs_before':producer,'producer_inputs_after':producer},'frames':frames,**{key:rows[0][key] for key in ('state_id','canvas_id','subcase')}}
   command={'command':['python3','scripts/report/verify_general_native_visual.py','--app',record['package']['app'],'--expected-source','a'*64,'--side','reference','--state',rows[0]['state_id'],'--canvas',rows[0]['canvas_id'],'--subcase',rows[0]['subcase'],'--output',str(first)],'exit_code':0,'pid':600,'parent_pid':700,'cwd':str(root)}
   def write(path,value):path.write_text(json.dumps(value))
   write(first/'capture.json',record);write(first/'finished.json',{'pid':500,'status':'finished_by_owned_controller'});(first/'finish-marker.txt').write_text('1'*32);sidecar=parent/(case_key(rows[0])+'.command.json');write(sidecar,command);second=parent/case_key(rows[1]);second.mkdir()
   self.assertEqual(reference,display.batch_lock(root,second,rows[1],'reference','a'*64,producer,environment,coordinator,None))
   for change in ('failed_capture','bool_exit','failed_command','missing_sidecar','frame_tampered','finish_tampered','extra_case'):
    broken=copy.deepcopy(record);badcommand=copy.deepcopy(command)
    if change=='failed_capture':broken['errors']=['actual failure']
    elif change=='bool_exit':broken['exit_code']=False
    elif change=='failed_command':badcommand['exit_code']=1
    elif change=='frame_tampered':(first/'frame-1.png').write_bytes(b'changed')
    elif change=='finish_tampered':(first/'finish-marker.txt').write_text('2'*32)
    elif change=='extra_case':(parent/case_key(rows[2])).mkdir()
    write(first/'capture.json',broken);write(sidecar,badcommand)
    if change=='missing_sidecar':sidecar.unlink()
    with self.subTest(change=change),self.assertRaises((ValueError,OSError)):display.batch_lock(root,second,rows[1],'reference','a'*64,producer,environment,coordinator,None)
    write(first/'capture.json',record);write(sidecar,command);(first/'frame-1.png').write_bytes(b'guard-only fake frame; never Native evidence');(first/'finish-marker.txt').write_text('1'*32)
    if change=='extra_case':(parent/case_key(rows[2])).rmdir()
   backups=root/'.build/leaf-backups';backups.mkdir()
   for leaf in (first/'capture.json',first/'finished.json',sidecar):
    original=leaf.read_bytes();target=backups/(leaf.name+'-original');target.write_bytes(original);leaf.unlink();leaf.symlink_to(target)
    with self.subTest(symlink_leaf=leaf.name),self.assertRaisesRegex(ValueError,'symlink'):display.batch_lock(root,second,rows[1],'reference','a'*64,producer,environment,coordinator,None)
    leaf.unlink();leaf.write_bytes(original)
 def test_native_probe_rejects_inventory_count_changes_without_gui(self):
  import subprocess
  with tempfile.TemporaryDirectory(prefix='native-display-count-guard-',dir=ROOT/'.build') as temporary:
   directory=Path(temporary);harness=directory/'CountGuard.swift';binary=directory/'count-guard'
   # Compile the exact pure-count prefix from the real probe. The complete
   # normal CLI is compiled/probed separately; no CG query runs in this guard.
   source=ROOT/'scripts/quality/general_native_display_probe.swift'
   prefix=source.read_text().split('#if !GENERAL_NATIVE_DISPLAY_PROBE_TEST',1)[0]
   harness.write_text(prefix+'''
@main struct Guard {
 static func main() {
  precondition(stableDisplayInventoryCounts(2,2,2))
  precondition(stableDisplayInventoryCounts(32,32,32))
  for values: (UInt32,UInt32,UInt32) in [(0,0,0),(33,33,33),(2,1,2),(2,3,2),(2,2,3),(2,2,1)] {
   precondition(!stableDisplayInventoryCounts(values.0,values.1,values.2))
  }
  print("8 count guards; no CG query or GUI")
 }
}''')
   command=['/usr/bin/xcrun','swiftc','-parse-as-library','-swift-version','5','-framework','CoreGraphics','-framework','ColorSync','-framework','CryptoKit',str(harness),'-o',str(binary)]
   built=subprocess.run(command,text=True,capture_output=True,timeout=30);self.assertEqual(0,built.returncode,built.stderr)
   checked=subprocess.run([str(binary)],text=True,capture_output=True,timeout=10);self.assertEqual(0,checked.returncode,checked.stderr);self.assertIn('8 count guards',checked.stdout)
 def test_native_expected_environment_only_comes_from_fixed_independent_approval(self):
  import current_general_native_display as display
  from current_general_reference import AUTHOR_IDS
  from current_general_visual_authority import SCOPE
  environment=display.primary_environment(native_display_guard_fixture()[0]);row=capture_cases()[0]
  with tempfile.TemporaryDirectory() as temporary:
   root=Path(temporary).resolve()
   for relative in source_inputs(ROOT):
    path=root/relative;path.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/relative,path)
   directory=root/'.build/source'/case_key(row);directory.mkdir(parents=True);lock=display.batch_lock(root,directory,row,'reference','a'*64,{},environment,{'process':{'pid':700}},None)
   catalogue={'kind':'general-independent-reference-catalogue/v1','scope':SCOPE,'source_authority':authority(root),'layers':{'packaged_native':{'environment':environment,'environment_sha256':display.identity(environment),'environment_lock':lock}}}
   cp=root/display.CATALOGUE;cp.parent.mkdir(parents=True,exist_ok=True);cp.write_text(json.dumps(catalogue));review={'kind':'general-independent-reference-review/v1','status':'approved','reviewer_id':'independent-guard-only-reviewer','author_ids':sorted(AUTHOR_IDS),'catalogue':display.file_ref(root,cp),'approved_native_environment_sha256':display.identity(environment),'reviewed_environment_artifacts_sha256':{lock['path']:lock['sha256']},**{key:False for key in ('product_alignment_approved','native_verified','game_tests_executed','os_runtime_verified','storage_runtime_verified')}};rp=root/display.REVIEW;rp.write_text(json.dumps(review))
   with patch('current_general_reference.assess',side_effect=AssertionError('must not recursively assess54')):
    expected,binding=display.approved_environment(root);self.assertEqual(environment,expected);self.assertEqual(display.file_ref(root,cp),binding['catalogue'])
   for key,value in [('status','pending_review'),('reviewer_id','/root'),('catalogue',{'path':'https://untrusted.invalid','sha256':'a'*64}),('approved_native_environment_sha256','f'*64),('reviewed_environment_artifacts_sha256',{})]:
    altered=copy.deepcopy(review);altered[key]=value;rp.write_text(json.dumps(altered))
    with self.subTest(key=key),self.assertRaises(ValueError):display.approved_environment(root)
   rp.write_text(json.dumps(review));catalogue['layers']['packaged_native'].pop('environment');cp.write_text(json.dumps(catalogue));review['catalogue']=display.file_ref(root,cp);rp.write_text(json.dumps(review))
   with self.assertRaisesRegex(ValueError,'environment missing'):display.approved_environment(root)

if __name__=='__main__':unittest.main()
