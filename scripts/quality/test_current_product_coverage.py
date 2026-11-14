"""Mutation tests on inventory and actual receipt inspection, never product adapters."""
import ast,copy,hashlib,json,shutil,struct,subprocess,sys,tempfile,unittest,zlib
from pathlib import Path
from unittest.mock import patch
import current_product_coverage as c
import design_authority as authority

class CurrentProductCoverageTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
        paths={c.CATALOG,c.PRESENTATION,c.GUI_SOURCE,c.DESIGN,c.REGISTRY,*c.LEGACY_FILES,*authority.current_approval_inputs(c.ROOT),'docs/architecture/engineering-modules.csv','docs/architecture/engineering-components.csv','docs/development/first-product-test-operations.json'}
        for rel in paths:
            target=self.root/rel;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(c.ROOT/rel,target)
        self.data=c._read(self.root/c.REGISTRY)
    def write(self,rel,value):
        path=self.root/rel;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(value if isinstance(value,str) else json.dumps(value,ensure_ascii=False)+'\n');return path
    def ref(self,path):return {'path':str(path.relative_to(self.root)),'sha256':c.digest(path)}
    def expected(self,condition):return next(r for r in c.required_executions(self.root).values() if condition(r))
    def receipt(self,e):
        proof=self.ref(self.write('.build/proof','guard fixture only'))
        row={**{k:e[k] for k in ('execution_id','case_id','layer','variant_id','dimensions','evidence_scope','environment_ids')},'source_tree_sha256':c.source_identity(self.root),'artifact_sha256':'a'*64,'execution_nonce':'b'*32,'fixture_substitution':False,'status':'passed','assertion_count':2,'skipped_assertions':0,'observed_at':'2026-10-31T00:00:01Z','evidence':[{'kind':kind,**proof} for kind in e['required_evidence']],'observations':{}}
        scope=e['evidence_scope'];flag='design_only_executed' if scope=='design_only' else 'product_component_tests_executed' if scope=='source_build' else 'product_packaged_native_game_tests_executed'
        r={'schema_version':1,'adapter_id':'guard-fixture-only','command':['owned-fixture'],'status':'passed','errors':[],'exit_code':0,'source_tree_sha256':row['source_tree_sha256'],'artifact_sha256':'a'*64,'execution_nonce':'b'*32,'fixture_substitution':False,'started_at':'2026-10-31T00:00:00Z','finished_at':'2026-10-31T00:00:02Z',flag:True,'execution_results':[row],'log':'.build/log'}
        a={'command':('owned-fixture',),'scope':scope,'evidence_schema':'current_product_execution_v1','layers':{e['layer']},'case_ids':{e['case_id']},'execution_ids':{e['execution_id']}}
        self.sync(r);return r,row,a
    def sync(self,r):
        path=self.write(r['log'],''.join('LUNA_CURRENT_PRODUCT_RESULT '+json.dumps(row)+'\n' for row in r['execution_results']));r['log_sha256']=c.digest(path)
    def inspect(self,r,e,a):
        with patch.dict(c.ADAPTERS,{'guard-fixture-only':a},clear=True):return c._inspect_receipt(self.root,r,e,'a'*64)
    def test_static_closed_inventory_preserves_old_plan_without_product_execution(self):
        self.assertEqual([],c.validate_registry(self.root));self.assertEqual({},c.ADAPTERS);self.assertEqual(70,len(self.data['cases']));self.assertEqual(153,self.data['legacy_contract']['executions']);self.assertEqual(25,len(self.data['inventory']['services']))
    def test_assessment_binds_actual_obligations_gaps_and_registered_adapters(self):
        report=c.assessment_report(self.root)
        self.assertEqual('blocked',report['status']);self.assertTrue(report['release_gaps'])
        self.assertEqual(len(c.required_executions(self.root)),report['required_executions'])
        self.assertEqual([],c.assessment_errors(self.root,report,1))
        forged=copy.deepcopy(report);forged.update(status='passed',release_gaps=[],required_executions=0,current_scope='acceptance-v4',composition_id='acceptance-v4')
        self.assertTrue(c.assessment_errors(self.root,forged,0))
        with patch.dict(c.ADAPTERS,{'fake-current':{}},clear=True):
            self.assertTrue(c.assessment_errors(self.root,report,1))
        self.assertEqual({},c.ADAPTERS)
    def test_assessment_typed_counts_scope_qualifications_approval_flags_and_exit(self):
        report=c.assessment_report(self.root)
        approval=copy.deepcopy(report['effective_design_approval']);approval['record']['sha256']='a'*64
        cases=[('schema_version',True),('required_executions',True),('required_executions',0),
               ('separate_platform_qualifications',False),('separate_platform_qualifications',0),
               ('current_scope','acceptance-v4'),('target_id','acceptance-v4'),('composition_id','acceptance-v4'),
               ('registered_adapters',['fake-current']),('registered_adapters',{}),('status',{}),
               ('errors',{}),('release_gaps',[]),('release_gaps',['fake gap']),
               ('effective_design_approval',approval),('effective_design_approval',[]),
               ('not_product_execution',False),('product_tests_executed',True),
               ('product_alignment_approved',True),('legacy_153_independently_required',False)]
        with patch.object(c,'assessment_report',return_value=report):
            for key,value in cases:
                with self.subTest(key=key,value=value):
                    changed=copy.deepcopy(report);changed[key]=value
                    self.assertTrue(c.assessment_errors(self.root,changed,1))
            for code in [0,True,2,'1']:
                with self.subTest(code=code):self.assertTrue(c.assessment_errors(self.root,report,code))
    def test_assessment_accepts_actual_empty_gap_outcome_without_claiming_execution(self):
        # An isolated fact-provider fixture proves status is derived from gaps;
        # this does not install evidence, an adapter or any acceptance approval.
        with patch.object(c,'release_coverage_gaps',return_value=[]):
            report=c.assessment_report(self.root);self.assertEqual('passed',report['status'])
            self.assertEqual([],c.assessment_errors(self.root,report,0))
            self.assertIs(report['not_product_execution'],True)
            self.assertIs(report['product_tests_executed'],False)
            self.assertIs(report['product_alignment_approved'],False)
    def test_actual_verify_readiness_branch_rejects_false_pass_and_malformed_reports(self):
        tree=ast.parse((c.ROOT/'scripts/quality/verify.py').read_text())
        branch=next(node for node in ast.walk(tree) if isinstance(node,ast.If)
                    and isinstance(node.test,ast.Compare) and isinstance(node.test.left,ast.Constant)
                    and node.test.left.value=='current-product-readiness-assessment')
        actual=c.assessment_report(self.root)
        forged=copy.deepcopy(actual);forged.update(status='passed',release_gaps=[],required_executions=0,current_scope='acceptance-v4')
        for report,code in [(actual,1),(forged,0),([],0)]:
            with self.subTest(report_type=type(report).__name__,code=code):
                observed=[]
                namespace={'ROOT':self.root,'sys':sys,'json':json,
                           'run':lambda *args,**kwargs:(json.dumps(report),code),
                           'checks':lambda identity,errors,**kwargs:observed.append((identity,errors))}
                exec(compile(ast.fix_missing_locations(ast.Module(body=copy.deepcopy(branch.body),type_ignores=[])),
                             'actual-verify-current-product-readiness','exec'),namespace)
                self.assertEqual(1,len(observed));self.assertEqual('current-product-readiness-contract',observed[0][0])
                self.assertEqual(report is not actual,bool(observed[0][1]))
    def test_every_source_omission_rejected(self):
        for i in range(70):
            with self.subTest(index=i):
                data=copy.deepcopy(self.data);data['cases'].pop(i);self.assertTrue(any('70 source' in e for e in c.validate_registry(self.root,data)))
    def test_identity_assertion_layers_status_and_mapping_mutations_rejected(self):
        for field,value in [('id','GD-99'),('status','passed'),('required_layers',['design']),('owners',['QA']),('module_ids',['M01']),('component_ids',['C11']),('requirement_ids',['FP-01']),('prerequisite_ids',['PRE-01']),('design_refs',[c.DESIGN+'#absent'])]:
            with self.subTest(field=field):
                data=copy.deepcopy(self.data);data['cases'][-1][field]=value;self.assertTrue(c.validate_registry(self.root,data))
        data=copy.deepcopy(self.data);data['cases'][-1]['source_record']['assertion']='always passed';self.assertTrue(c.validate_registry(self.root,data))
        data=copy.deepcopy(self.data);data['cases'][-1]=data['cases'][0];self.assertTrue(c.validate_registry(self.root,data))
    def test_each_service_protocol_engine_omission_and_exact_tuple_drift_rejected(self):
        for field in ('services','protocol_ids','offline_engine_ids'):
            for i in range(len(self.data['inventory'][field])):
                with self.subTest(field=field,index=i):
                    data=copy.deepcopy(self.data);data['inventory'][field].pop(i);self.assertTrue(c.validate_registry(self.root,data))
        data=copy.deepcopy(self.data);data['inventory']['services'][0]['adapter_contract_revision']+=1;self.assertTrue(c.validate_registry(self.root,data))
    def test_json_cannot_register_adapter_or_approve_design(self):
        for field,value in [('adapters',{'fake':['true']}),('design_approval','approved'),('product_execution','passed')]:
            data=copy.deepcopy(self.data);data[field]=value;self.assertTrue(c.validate_registry(self.root,data))
    def test_gui_literal_and_old_plan_bytes_bound(self):
        path=self.root/c.GUI_SOURCE;path.write_text(path.read_text().replace("regression('GUI08'","regression('GUI99'"));self.assertTrue(c.validate_registry(self.root))
        shutil.copyfile(c.ROOT/c.GUI_SOURCE,path);path=self.root/c.LEGACY_FILES[0];path.write_text(path.read_text()+' ');self.assertTrue(any('legacy' in e for e in c.validate_registry(self.root)))
    def test_variants_are_meaningful_and_preserve_four_games_pages_permissions_vault_zoom(self):
        rows=list(c.required_executions(self.root).values());games=[r for r in rows if r['case_id']=='PA-12'];self.assertEqual({'GAME-SR','GAME-MO','GAME-N7','GAME-SAKURA'},{r['variant_id'] for r in games});self.assertTrue(all(r['dimensions']['minimum_distinct_states']==5 for r in games));self.assertEqual(3,next(r for r in games if r['variant_id']=='GAME-N7')['dimensions']['minimum_confirmed_pages'])
        for case,layer,count in [('GD-04','catalog',25),('PA-04','protocol',13),('GD-10','worker',4),('OT-08','worker',2)]:self.assertEqual(count,len([r for r in rows if r['case_id']==case and r['layer']==layer]))
        for key,value in [('display_condition','ZOOM-200-UNCLIPPED'),('permission_state','PERMISSION-CANCELLED'),('credential_condition','EXPORT-LOG-NO-SECRET'),('storage_condition','MIGRATION-ROLLBACK')]:self.assertTrue(any(r['dimensions'].get(key)==value for r in rows))
    def test_purpose_and_runtime_closure_and_authority_pointers_cannot_be_weakened(self):
        rows=list(c.required_executions(self.root).values())
        service_rows=[r for r in rows if r['case_id']=='PA-11' and r['layer']=='service']
        expected={(r['id'],purpose) for r in self.data['inventory']['services'] if r['kind']=='http' for purpose in r['allowed_purposes']}
        self.assertEqual(expected,{(r['dimensions']['service_id'],r['dimensions']['purpose']) for r in service_rows})
        for case in ('OT-08','OT-16'):
            actual={(r['dimensions']['engine_id'],r['dimensions'].get('runtime')) for r in rows if r['case_id']==case and r['layer']=='worker'}
            self.assertTrue({('jbeijing','CROSSOVER'),('kingsoft_fastait','CROSSOVER')}<=actual)
            self.assertFalse(any(runtime=='WINDOWS' for _,runtime in actual))
        for key in ('design_target','source_catalog','gui_source'):
            data=copy.deepcopy(self.data);data[key]='elsewhere'
            self.assertTrue(any('pointer mismatch' in error for error in c.validate_registry(self.root,data)))
    def test_composite_layers_and_current_reading_keep_each_responsibility(self):
        cases={r['id']:r for r in self.data['cases']}
        for cid,layers in {'OT-02':{'design','component'},'OT-03':{'domain','component'},'OT-04':{'domain','component'},'OT-10':{'domain','storage'},'OT-12':{'domain','storage'},'OT-15':{'domain','component','storage'}}.items():
            self.assertTrue(layers<=set(cases[cid]['required_layers']),cid)
        self.assertTrue({'M03','M04','M05'}<=set(cases['GD-01']['module_ids']))
        self.assertIn('AI',cases['GD-01']['owners'])
        self.assertIn('M09',cases['GD-08']['module_ids'])
        private=self.expected(lambda r:r['case_id']=='OT-09' and r['layer']=='protocol')
        http=self.expected(lambda r:r['case_id']=='PA-04' and r['layer']=='protocol')
        self.assertIn('actual_private_ipc_request_response_bytes_redacted',private['required_evidence'])
        self.assertNotIn('actual_socket_request_bytes_redacted',private['required_evidence'])
        self.assertIn('actual_socket_request_bytes_redacted',http['required_evidence'])
    def test_only_four_original_windows_qualifications_are_separate_and_not_run(self):
        qualifications=self.data['separate_platform_qualifications']
        self.assertEqual(4,len(qualifications));self.assertEqual('macos-arm64-crossover',self.data['current_scope'])
        self.assertEqual({('OT-08','jbeijing'),('OT-08','kingsoft_fastait'),('OT-16','jbeijing'),('OT-16','kingsoft_fastait')},{(r['case_id'],r['engine_id']) for r in qualifications})
        for mutation in ('remove','extra','passed','scope'):
            data=copy.deepcopy(self.data)
            if mutation=='remove':data['separate_platform_qualifications'].pop()
            elif mutation=='extra':data['separate_platform_qualifications'].append({**qualifications[0],'case_id':'GD-10'})
            elif mutation=='passed':data['separate_platform_qualifications'][0]['status']='passed'
            else:data['current_scope']='windows'
            self.assertTrue(c.validate_registry(self.root,data))
    def test_actual_observed_receipt_positive_and_log_conflict_rejected(self):
        e=self.expected(lambda r:r['layer']=='domain');r,row,a=self.receipt(e);self.assertEqual(row,self.inspect(r,e,a));self.assertEqual({},c.ADAPTERS);row['assertion_count']+=1
        with self.assertRaisesRegex(ValueError,'summary differs'):self.inspect(r,e,a)
    def test_raw_log_boolean_count_cannot_equal_integer_summary(self):
        e=self.expected(lambda row:row['layer']=='domain');r,row,a=self.receipt(e);row['assertion_count']=1;self.sync(r)
        self.assertEqual(row,self.inspect(r,e,a))
        logged=copy.deepcopy(row);logged['assertion_count']=True
        path=self.write(r['log'],'LUNA_CURRENT_PRODUCT_RESULT '+json.dumps(logged)+'\n');r['log_sha256']=c.digest(path)
        with self.assertRaisesRegex(ValueError,'summary differs'):self.inspect(r,e,a)
    def test_receipt_source_package_nonce_fixture_command_interval_flags_mutations_rejected(self):
        e=self.expected(lambda r:r['layer']=='domain')
        for key,value in [('source_tree_sha256','c'*64),('artifact_sha256','c'*64),('execution_nonce','invalid'),('fixture_substitution',True),('command',['true']),('exit_code',True),('status','blocked'),('product_component_tests_executed',False),('started_at','2026-10-31T00:00:03Z')]:
            with self.subTest(key=key):
                r,row,a=self.receipt(e);r[key]=value
                with self.assertRaises(ValueError):self.inspect(r,e,a)
    def test_assertion_count_skip_environment_and_time_mutations_rejected(self):
        e=self.expected(lambda r:r['layer']=='domain')
        for key,value in [('assertion_count',0),('assertion_count',True),('skipped_assertions',1),('environment_ids',['OTHER']),('observed_at','2026-10-31T00:01:00Z')]:
            with self.subTest(key=key):
                r,row,a=self.receipt(e);row[key]=value
                with self.assertRaises(ValueError):self.inspect(r,e,a)
    def test_unregistered_cross_layer_variant_and_missing_observation_rejected(self):
        e=self.expected(lambda r:r['layer']=='native');r,row,a=self.receipt(e)
        with self.assertRaisesRegex(ValueError,'not implemented'):c._inspect_receipt(self.root,r,e,'a'*64)
        for key,value in [('layers',{'component'}),('execution_ids',set()),('scope','source_build'),('evidence_schema','self-granted')]:
            with self.assertRaises(ValueError):self.inspect(r,e,{**a,key:value})
        row['evidence'].pop()
        with self.assertRaisesRegex(ValueError,'observation kind'):self.inspect(r,e,a)
    def test_200_percent_permission_and_secret_need_actual_result_observations(self):
        targets=[self.expected(lambda r:r['dimensions'].get('display_condition')=='ZOOM-200-UNCLIPPED'),self.expected(lambda r:r['dimensions'].get('permission_state')=='PERMISSION-CANCELLED'),self.expected(lambda r:r['layer']=='credentials')]
        for e in targets:
            with self.subTest(execution=e['execution_id']):
                r,row,a=self.receipt(e)
                with self.assertRaises(ValueError):self.inspect(r,e,a)
    def game_states(self):
        """Distinct real PNG files and callback bytes, only guard fixtures."""
        states=[]
        for n in range(5):
            captures={}
            for role in ('game_capture','client_capture'):
                color=hashlib.sha256((role+str(n)).encode()).digest()[:3]
                def chunk(kind,payload):return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload)&0xffffffff)
                image=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,2,8,2,0,0,0))+chunk(b'IDAT',zlib.compress((b'\x00'+color*2)*2))+chunk(b'IEND',b'')
                path=self.root/f'.build/{role}-{n}.png';path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(image);captures[role]=self.ref(path)
            callback=self.ref(self.write(f'.build/callback-{n}.json',{'event_id':f'E{n}','state_id':f'S{n}','text':f'guard fixture subtitle {n}'}))
            states.append({'state_id':f'S{n}','screen_text_sha256':hashlib.sha256(str(n).encode()).hexdigest(),'page_id':f'P{n%3}',**captures,'callback_evidence':callback})
        return states
    def test_reused_game_or_client_capture_rejected_even_with_fresh_claimed_state_ids(self):
        e=self.expected(lambda r:r['case_id']=='PA-12' and r['variant_id']=='GAME-N7')
        for role in ('game_capture','client_capture'):
            for renamed in (False,True):
                with self.subTest(role=role,renamed=renamed):
                    r,row,a=self.receipt(e);states=self.game_states();row['observations']={'states':states,'language_branch':'japanese'}
                    if renamed:
                        original=self.root/states[0][role]['path'];duplicate=self.root/f'.build/renamed-{role}.png';duplicate.write_bytes(original.read_bytes());states[1][role]=self.ref(duplicate)
                    else:states[1][role]=copy.deepcopy(states[0][role])
                    self.sync(r)
                    with self.assertRaisesRegex(ValueError,'reused '+role):self.inspect(r,e,a)
    def test_game_five_unique_subtitles_and_never7_three_pages_not_five_events(self):
        e=self.expected(lambda r:r['case_id']=='PA-12' and r['variant_id']=='GAME-N7');r,row,a=self.receipt(e);states=self.game_states();row['observations']={'states':states,'language_branch':'japanese'};self.sync(r);self.assertEqual(row,self.inspect(r,e,a))
        states[1]['screen_text_sha256']=states[0]['screen_text_sha256'];self.sync(r)
        with self.assertRaisesRegex(ValueError,'distinct subtitle'):self.inspect(r,e,a)
        states[1]['screen_text_sha256']=hashlib.sha256(b'1').hexdigest()
        for state in states:state['page_id']='P0'
        self.sync(r)
        with self.assertRaisesRegex(ValueError,'pages/language'):self.inspect(r,e,a)
    def test_ot16_requires_all_engine_game_pairs_and_owned_results_resources_quality(self):
        rows=[row for row in c.required_executions(self.root).values() if row['case_id']=='OT-16' and row['layer']=='game']
        self.assertEqual(16,len(rows));self.assertEqual({(engine,game) for engine in self.data['inventory']['offline_engine_ids'] for game in ('GAME-SR','GAME-MO','GAME-N7','GAME-SAKURA')},{(row['dimensions']['engine_id'],row['dimensions']['game_id']) for row in rows})
        e=next(row for row in rows if row['dimensions']['game_id']=='GAME-N7' and row['dimensions']['engine_id']=='argos_translate')
        for mutation in ('none','identity','typed_revision','missing_state','other_engine','resource','quality'):
            with self.subTest(mutation=mutation):
                r,row,a=self.receipt(e);states=self.game_states();identity={key:e['dimensions'][key] for key in ('engine_id','engine_contract_revision','execution_backend','runtime')};row['observations']={'states':states,'language_branch':'japanese','engine_identity':copy.deepcopy(identity)}
                proof=self.ref(self.root/'.build/proof');resources={'engine_identity':copy.deepcopy(identity),'resource_version':'guard-fixture-1','resource_artifacts':[proof],'license_evidence':proof,'status':'ready','authorized':True,'route_supported':True}
                translations=[{'state_id':state['state_id'],'engine_identity':copy.deepcopy(identity),'game_id':'GAME-N7','artifact_sha256':'a'*64,'execution_nonce':'b'*32,'status':'completed','source_language':'ja','target_language':'zh','execution_kind':'engine_translation','engine_invoked':True,'translation_evidence':self.ref(self.write('.build/translation-'+str(n)+'.txt','guard fixture translation '+str(n)))} for n,state in enumerate(states)]
                quality={'engine_identity':copy.deepcopy(identity),'game_id':'GAME-N7','artifact_sha256':'a'*64,'decision':'accepted','reviewer':'isolated guard reviewer','quality_scope':'actual_japanese_translation_quality','translation_quality_assessed':True}
                if mutation=='identity':row['observations']['engine_identity']['engine_id']='apple_translation'
                elif mutation=='typed_revision':row['observations']['engine_identity']['engine_contract_revision']=True
                elif mutation=='missing_state':translations.pop()
                elif mutation=='other_engine':translations[0]['engine_identity']['engine_id']='apple_translation'
                elif mutation=='resource':resources['engine_identity']['runtime']='WINDOWS'
                elif mutation=='quality':quality['game_id']='GAME-SR'
                values={'actual_engine_resource_identity':resources,'actual_engine_translation_results':translations,'independent_engine_quality_review':quality}
                for item in row['evidence']:
                    if item['kind'] in values:item.update(self.ref(self.write('.build/'+item['kind']+'.json',values[item['kind']])))
                self.sync(r)
                if mutation=='none':self.assertEqual(row,self.inspect(r,e,a))
                else:
                    with self.assertRaises(ValueError):self.inspect(r,e,a)
    def test_nested_json_identity_distinguishes_boolean_versions_and_zero_flags(self):
        data=copy.deepcopy(self.data);data['inventory']['services'][0]['adapter_contract_revision']=True
        self.assertTrue(any('inventory' in error for error in c.validate_registry(self.root,data)))
        data=copy.deepcopy(self.data);data['separate_platform_qualifications'][0]['release_required_for_current_scope']=0
        self.assertTrue(c.validate_registry(self.root,data))
        self.assertFalse(c._same({'revision':True},{'revision':1}));self.assertFalse(c._same({'count':False},{'count':0}))
        e=self.expected(lambda row:row['case_id']=='GUI08' and row['layer']=='domain' and row['dimensions']['context_count']==0)
        r,row,a=self.receipt(e);row['dimensions']=copy.deepcopy(row['dimensions']);row['dimensions']['context_count']=False
        with self.assertRaisesRegex(ValueError,'dimensions'):self.inspect(r,e,a)
    def test_sr_passthrough_is_only_non_japanese_branch_and_unknown_resource_blocks(self):
        e=self.expected(lambda row:row['case_id']=='OT-16' and row['layer']=='game' and row['dimensions']['game_id']=='GAME-SR' and row['dimensions']['engine_id']=='argos_translate')
        for mutation in ('none','false_japanese_quality','uninvoked_translation','unauthorized','unsupported_route'):
            with self.subTest(mutation=mutation):
                r,row,a=self.receipt(e);states=self.game_states();identity={key:e['dimensions'][key] for key in ('engine_id','engine_contract_revision','execution_backend','runtime')};row['observations']={'states':states,'language_branch':'non_japanese','engine_identity':identity}
                proof=self.ref(self.root/'.build/proof');resources={'engine_identity':identity,'resource_version':'guard-1','resource_artifacts':[proof],'license_evidence':proof,'status':'ready','authorized':True,'route_supported':True}
                translations=[{'state_id':state['state_id'],'engine_identity':identity,'game_id':'GAME-SR','artifact_sha256':'a'*64,'execution_nonce':'b'*32,'status':'passthrough','source_language':'zh','target_language':'zh','execution_kind':'same_language_passthrough','engine_invoked':False,'translation_evidence':self.ref(self.write('.build/sr-'+str(n)+'.txt','guard only Chinese source '+str(n)))} for n,state in enumerate(states)]
                quality={'engine_identity':identity,'game_id':'GAME-SR','artifact_sha256':'a'*64,'decision':'accepted','reviewer':'guard reviewer','quality_scope':'non_japanese_branch_only','translation_quality_assessed':False}
                if mutation=='false_japanese_quality':quality.update({'quality_scope':'actual_japanese_translation_quality','translation_quality_assessed':True})
                elif mutation=='uninvoked_translation':translations[0].update({'execution_kind':'engine_translation','status':'completed'})
                elif mutation=='unauthorized':resources['authorized']=False
                elif mutation=='unsupported_route':resources['route_supported']=False
                values={'actual_engine_resource_identity':resources,'actual_engine_translation_results':translations,'independent_engine_quality_review':quality}
                for item in row['evidence']:
                    if item['kind'] in values:item.update(self.ref(self.write('.build/'+item['kind']+'.json',values[item['kind']])))
                self.sync(r)
                if mutation=='none':self.assertEqual(row,self.inspect(r,e,a))
                else:
                    with self.assertRaises(ValueError):self.inspect(r,e,a)
    def test_sidecar_link_positive_and_hash_unknown_duplicate_missing_rejected(self):
        e=self.expected(lambda r:r['layer']=='domain');r,row,a=self.receipt(e);receipt=self.write('.build/receipt.json',r);side={'schema_version':1,'artifact_sha256':'a'*64,'registry_sha256':c.digest(self.root/c.REGISTRY),'execution_results':[{'execution_id':e['execution_id'],'receipt':self.ref(receipt)}]};path=self.write('.build/current.json',side);outer={'current_product_coverage':self.ref(path)}
        with patch.dict(c.ADAPTERS,{'guard-fixture-only':a},clear=True),patch.object(c,'required_executions',return_value={e['execution_id']:e}):
            gaps=c.release_coverage_gaps(self.root,outer,'a'*64);self.assertEqual([],gaps);self.assertFalse(any(g['execution_id']==e['execution_id'] for g in gaps))
            approval=(self.root/authority.RECORD);original=approval.read_bytes();approval.unlink()
            gaps=c.release_coverage_gaps(self.root,outer,'a'*64)
            self.assertEqual(1,sum(g['layer']=='design_approval' for g in gaps))
            # Removing an authority source also changes the actual tree; the
            # earlier execution receipt must become stale rather than inherited.
            self.assertTrue(any(g['execution_id']==e['execution_id'] for g in gaps))
            approval.write_bytes(original)
        self.assertEqual({},c.ADAPTERS);outer['current_product_coverage']['sha256']='c'*64;self.assertTrue(c.release_coverage_gaps(self.root,outer,'a'*64))
        for mutation in ('unknown','duplicate','empty'):
            data=copy.deepcopy(side)
            if mutation=='unknown':data['execution_results'][0]['execution_id']='GUI99@native@DEFAULT'
            elif mutation=='duplicate':data['execution_results']*=2
            else:data['execution_results']=[]
            path=self.write('.build/current.json',data);self.assertTrue(c.release_coverage_gaps(self.root,{'current_product_coverage':self.ref(path)},'a'*64))
    def test_path_hash_empty_symlink_and_duplicate_json_fail_closed(self):
        path=self.write('.build/a','observed')
        for reference in ({'path':'../outside','sha256':'a'*64},{'path':str(path),'sha256':c.digest(path)},{'path':'.build/a','sha256':'a'*64}):
            with self.assertRaises(ValueError):c._hashed(self.root,reference)
        empty=self.write('.build/empty','')
        with self.assertRaises(ValueError):c._hashed(self.root,self.ref(empty))
        link=self.root/'.build/link';link.symlink_to(path)
        with self.assertRaises(ValueError):c._hashed(self.root,{'path':'.build/link','sha256':c.digest(path)})
        self.write(c.REGISTRY,'{"schema_version":1,"schema_version":1}');self.assertTrue(c.validate_registry(self.root))
    def test_cli_static_pass_and_release_block_do_not_claim_execution(self):
        command=[sys.executable,str(c.ROOT/'scripts/quality/current_product_coverage.py'),'--root',str(self.root),'--json'];check=subprocess.run(command+['--check'],capture_output=True,text=True);self.assertEqual(0,check.returncode,check.stdout+check.stderr);report=json.loads(check.stdout);self.assertTrue(report['not_product_execution']);self.assertFalse(report['product_tests_executed']);assess=subprocess.run(command+['--assess'],capture_output=True,text=True);self.assertEqual(1,assess.returncode,assess.stderr);report=json.loads(assess.stdout);self.assertEqual('blocked',report['status']);self.assertTrue(report['legacy_153_independently_required'])
    def test_explicit_source_approval_clears_only_approval_not_execution_or_package(self):
        gaps=c.release_coverage_gaps(self.root)
        self.assertFalse(any(g['layer']=='design_approval' for g in gaps))
        self.assertTrue(any(g['layer']=='evidence' for g in gaps))
        self.assertEqual(576,sum(g['status']=='not_run' for g in gaps))
        self.assertEqual('pending_review',self.data['design_approval'])
        (self.root/authority.RECORD).unlink()
        missing=c.release_coverage_gaps(self.root)
        self.assertEqual(len(gaps)+1,len(missing))
        self.assertEqual(576,sum(g['status']=='not_run' for g in missing))
        self.assertTrue(any(g['layer']=='design_approval' for g in missing))
    def test_complete_current_decision_required_even_when_legacy57_still_valid(self):
        effective=c.source_approval(self.root)
        self.assertEqual(authority.CURRENT_COMPOSITION_ID,effective['composition_id'])
        self.assertEqual(61,len(effective['design_sources']))
        self.assertFalse(effective['product_tests_executed'])
        (self.root/authority.CURRENT_RECORD).unlink()
        self.assertEqual([],authority.user_approval_gaps(self.root))
        gaps=c.release_coverage_gaps(self.root)
        self.assertTrue(any(g['layer']=='design_approval' for g in gaps))
        self.assertEqual(576,sum(g['status']=='not_run' for g in gaps))
    def test_history_source_rebinding_cannot_inherit_product_design_approval(self):
        css=self.root/authority.HISTORY_ROOT/'history.css';css.write_bytes(css.read_bytes()+b'\nchanged design source\n')
        manifest=self.root/authority.HISTORY_ROOT/'manifest.json';data=json.loads(manifest.read_text())
        data['owned_sources']['history.css']=c.digest(css);manifest.write_text(json.dumps(data))
        self.assertEqual([],authority.user_approval_gaps(self.root))
        gaps=c.release_coverage_gaps(self.root)
        self.assertTrue(any(g['layer']=='design_approval' and 'complete approved source bytes changed' in g['reason'] for g in gaps))
        self.assertEqual(576,sum(g['status']=='not_run' for g in gaps))

if __name__=='__main__':unittest.main()
