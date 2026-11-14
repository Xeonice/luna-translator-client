import base64
import copy
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch
from runtime_stability import GAMES,plan_steps,load_inputs,validate_response,snapshot_database,archive_trace,wait_trace_interval,freeze_trace,CHECK_ID
from runtime_stability_evidence import EvidenceUnavailable,inspect_body,inspect_lexical,inspect_trace,visible_text,preview_text,new_frames,power_sleep_wake,lsof_entries,validate_run,CASE_IDS,validate_frame_metadata,trace_interval,load_trace_archive
from product_execution import run_product_check
ROOT=Path(__file__).resolve().parents[2]
NONCE='a'*32

class RuntimeStabilityGuards(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name);self.out=self.root/'.build/run';self.out.mkdir(parents=True)
    def write(self,name,value):
        p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(value));return p
    def inputs(self):
        app=self.root/'Luna.app';exe=app/'Contents/MacOS/luna-reader';exe.parent.mkdir(parents=True);exe.write_text('guard-only never executed')
        db=self.root/'profiles/profile-guard/reader.sqlite3';db.parent.mkdir(parents=True);db.touch();registry=self.write('profiles.json',{'activeProfileId':'profile-guard'})
        (self.root/'guard.zip').write_text('guard-only not package acceptance')
        x={'schema_version':1,'artifact':'guard.zip','application':str(app),'profile_database':str(db),'profile_registry':str(registry),'profile_id':'profile-guard','log_directory':str(self.root/'logs'),'games':[{'id':g,'environment_key':'guard-'+g,'target_exe_path':'C:\\guard\\'+g+'.exe','architecture':'x86' if g in ('sr','mo') else 'x64','source_language':'zh-CN' if g=='sr' else 'ja'} for g in GAMES]}
        return x,self.write('inputs.json',x)
    def body(self,text='春の空。'):
        b=text.encode();h=hashlib.sha256(b).hexdigest()
        return {'body':[{'unit_id':'u','text_revision':1,'source_text':text,'source_sha256':h,'source_byte_length':len(b)}],'raw':[{'event_id':'r','integrity_state':'intact','payload_sha256':h,'payload_byte_length':len(b)}],'payloads':[{'event_id':'r','base64':base64.b64encode(b).decode()}],'refs':[{'unit_id':'u','text_revision':1,'ref_ordinal':0,'event_id':'r','byte_start':0,'byte_end':len(b)}]}
    def trace(self,order=(2,1)):
        common={'execution_nonce':NONCE,'run_id':'r','segment_index':0};begin={**common,'kind':'observation_begin','pid':10,'initialized_at_ms':100}
        facts=[{**common,'kind':'runtime_fact','attempt_sequence':n,'observed_at_ms':101+n,'fact':{'kind':'worker'}} for n in order]
        health={**common,'kind':'observation_health','at_ms':110,'attempted':5,'enqueued':3,'processed':len(facts),'written_facts':len(facts),'dropped':0,'invalid':0,'write_failures':0,'max_observed_attempt':max(order),'last_written_attempt':order[-1],'rotation':0,'terminal':False}
        rows=[begin,*facts,health];p=self.out/'trace/segment-00000000.jsonl';p.parent.mkdir();m={'trace_files':{},'archive_errors':[]};self.rewrite_trace(rows,p,m);return rows,p,m
    def rewrite_trace(self,rows,p,m):
        p.write_text(''.join(json.dumps(r)+'\n' for r in rows));m['trace_files']['trace/'+p.name]=hashlib.sha256(p.read_bytes()).hexdigest()
    def close_trace_fixture(self,rows,p,m):
        seen={r['attempt_sequence'] for r in rows if r['kind']=='runtime_fact'}
        for n in sorted(set(range(1,6))-seen):rows.append({**rows[1],'attempt_sequence':n,'observed_at_ms':111+n})
        actual=[r for r in rows if r['kind']=='runtime_fact']
        h={**rows[3],'kind':'observation_health','attempted':5,'enqueued':5,'processed':5,'written_facts':5,'max_observed_attempt':5,'last_written_attempt':actual[-1]['attempt_sequence']}
        rows.extend([{**h,'at_ms':120},{**h,'at_ms':130}])
        m.update(observation_completed_at_ms=119,completed_at_ms=131,steps=[],observation_intervals=[{'step_id':'final','boundary_ms':119,'opening_health':[0,len(rows)-2],'closing_health':[0,len(rows)-1]}])
        self.rewrite_trace(rows,p,m)
        live=self.out/'trace-live'/p.name;live.parent.mkdir(exist_ok=True);live.write_bytes(p.read_bytes());m['live_trace_files']={'trace-live/'+p.name:hashlib.sha256(live.read_bytes()).hexdigest()}
        return rows

    def image_snapshot(self):
        windows=[{'window_id':i,'pid':20 if i==1 else 10,'on_screen':True,'capture':{'png':str(i)+'.png','ocr':[{'text':'春の空。'}]}} for i in (1,2,3)]
        windows[1]['capture']['preview_crops']=[{'png':'preview.png','description':'actual · 目标窗口当前画面','ocr':[{'text':'春の空。'}]}]
        return {'pid':10,'action':{'game_host_pid':20,'windows':{'game':1,'main':2,'hud':3,'preview':2}},'os':{'observed_at_ms':500,'windows':windows}}
    def frames(self):
        f={'kind':'collector_frame','accepted':True,'host_pid':20,'host_start':'100','window_id':1,'target_candidate_id':'candidate','source_id':'source','source_epoch':'1','frame_epoch':'1','capture_id':'capture','worker_sequence':4,'captured_at_ms':300,'content_hash':'c'*64,'width':640,'height':480,'encoded_len':999}
        h=hashlib.sha256(b'candidate\x0020\x00100\x001').hexdigest();g={'lease_id':'lease','generation':'1','service_sequence':'9','target_identity_hash':h,'captured_at_ms':'300','content_hash':'c'*64,'width':640,'height':480,'encoded_len':999}
        return [{'observed_at_ms':310,'fact':f},{'observed_at_ms':320,'fact':{'kind':'projection','stage':'service_committed','projection':{'state':'live'},'frame':g}},{'observed_at_ms':330,'fact':{'kind':'projection','stage':'server_delivered','projection':{'state':'live'},'frame':copy.deepcopy(g)}}]
    def test_exact_four_game_inputs_and_active_profile(self):
        x,p=self.inputs();self.assertEqual(load_inputs(p,self.root),x)
        for mutate in [lambda x:x['games'].pop(),lambda x:x['games'].append(dict(x['games'][0],id='ever17')),lambda x:x.update(profile_id='other')]:
            v=copy.deepcopy(x);mutate(v);p.write_text(json.dumps(v))
            with self.assertRaises(ValueError):load_inputs(p,self.root)
    def test_unknown_keys_fixture_claims_and_arbitrary_commands_rejected(self):
        x,p=self.inputs()
        for key,v in [('pass',True),('command',['sh']),('fixturePurpose','guard-only')]:
            p.write_text(json.dumps({**x,key:v}))
            with self.assertRaises(ValueError):load_inputs(p,self.root)
    def test_symlink_executable_rejected(self):
        x,p=self.inputs();e=Path(x['application'])/'Contents/MacOS/luna-reader';e.unlink();e.symlink_to(p)
        with self.assertRaises(ValueError):load_inputs(p,self.root)
    def test_barrier_only_accepts_current_action_window_identity(self):
        a={'execution_nonce':NONCE,'step_id':'sr/cold','game_host_pid':20,'windows':{'game':1}};self.assertEqual(validate_response(a,NONCE,'sr/cold'),a)
        for mutate in [lambda x:x.update(status='passed'),lambda x:x.update(execution_nonce='b'*32),lambda x:x.update(step_id='mo/cold'),lambda x:x['windows'].update(secret='x')]:
            v=copy.deepcopy(a);mutate(v)
            with self.assertRaises(ValueError):validate_response(v,NONCE,'sr/cold')
    def test_fixed_steps_include_four_games_sleep_wake_and_all_closures(self):
        ids=[s for s,_ in plan_steps()];self.assertEqual(len(ids),len(set(ids)));self.assertTrue(all(g+'/reading-5' in ids and g+'/exited' in ids for g in GAMES));self.assertIn('sakura10th/wake-after',ids);self.assertIn('sakura10th/card-owner-scroll-closed',ids)
    def test_actual_source_refs_reconstruct_utf8_bytes(self):self.assertEqual(inspect_body(self.body())[0]['source_text'],'春の空。')
    def test_hash_payload_integrity_and_utf8_provenance_mutations_rejected(self):
        for mutate in [lambda s:s['body'][0].update(source_sha256='0'*64),lambda s:s['payloads'].clear(),lambda s:s['refs'][0].update(byte_start=1),lambda s:s['raw'][0].update(integrity_state='lost'),lambda s:s['refs'].clear()]:
            s=self.body();mutate(s)
            with self.assertRaises((ValueError,UnicodeError)):inspect_body(s)
    def test_lexical_spans_fixed_version_reading_and_surface(self):
        s=self.body('春');b=s['body'][0];s['lexical']=[{'lexical_id':'l','unit_id':'u','text_revision':1,'source_sha256':b['source_sha256'],'token_count':1}];s['tokens']=[{'lexical_id':'l','token_index':0,'byte_start':0,'byte_end':3,'surface':'春','lemma':'春','reading_kana':'ハル','token_kind':'word','is_known':1}];self.assertEqual(len(inspect_lexical(s,b)),1)
        for change in [{'surface':'夏'},{'byte_start':1},{'reading_kana':None},{'token_index':2}]:
            q=copy.deepcopy(s);q['tokens'][0].update(change)
            with self.assertRaises(ValueError):inspect_lexical(q,b)
    def test_out_of_order_attempts_and_relaxed_pending_health_are_valid(self):
        rows,p,m=self.trace((2,1));self.close_trace_fixture(rows,p,m);all_f,h=inspect_trace(self.out,m,NONCE,10,90);self.assertEqual({r['attempt_sequence'] for r in all_f},set(range(1,6)));f=all_f[:2];self.assertEqual({r['attempt_sequence'] for r in f},{1,2});self.assertFalse(h['terminal'])
    def test_maximum_is_not_contiguous_prefix(self):
        rows,p,m=self.trace((4,2));self.close_trace_fixture(rows,p,m);all_f,h=inspect_trace(self.out,m,NONCE,10,90);self.assertEqual({r['attempt_sequence'] for r in all_f},set(range(1,6)));f=all_f[:2];self.assertEqual({r['attempt_sequence'] for r in f},{2,4})
    def test_missing_old_record_not_hidden_by_newer_record(self):
        rows,p,m=self.trace();rows.pop(1);rows.append({'kind':'runtime_fact','execution_nonce':NONCE,'run_id':'r','attempt_sequence':3,'observed_at_ms':112,'fact':{'kind':'worker'},'segment_index':0});self.rewrite_trace(rows,p,m)
        with self.assertRaisesRegex(ValueError,'original segment count'):inspect_trace(self.out,m,NONCE,10,90)
    def test_loss_write_failure_old_nonce_pid_and_tamper_rejected(self):
        rows,p,m=self.trace()
        for key in ('dropped','invalid','write_failures'):
            modified=copy.deepcopy(rows);modified[-1][key]=1;self.rewrite_trace(modified,p,m)
            with self.assertRaises(ValueError):inspect_trace(self.out,m,NONCE,10,90)
        self.rewrite_trace(rows,p,m)
        with self.assertRaises(ValueError):inspect_trace(self.out,m,'b'*32,10,90)
        with self.assertRaises(ValueError):inspect_trace(self.out,m,NONCE,11,90)
        p.write_text('changed')
        with self.assertRaisesRegex(ValueError,'hash'):inspect_trace(self.out,m,NONCE,10,90)
    def test_rotation_gap_and_duplicate_attempt_rejected(self):
        rows,p,m=self.trace((1,1));self.rewrite_trace(rows,p,m)
        with self.assertRaisesRegex(ValueError,'duplicate'):inspect_trace(self.out,m,NONCE,10,90)
        rows[2]['attempt_sequence']=2;rows[-1].update(segment_index=2,rotation=2);self.rewrite_trace(rows,p,m)
        with self.assertRaises(ValueError):inspect_trace(self.out,m,NONCE,10,90)
    def test_visible_game_requires_correct_process_and_current_ocr(self):
        s=self.image_snapshot();self.assertEqual(visible_text(s,'game','春の空。')['window_id'],1);s['os']['windows'][0]['pid']=99
        with self.assertRaisesRegex(ValueError,'different process'):visible_text(s,'game','春の空。')
    def test_main_text_cannot_substitute_for_actual_preview_crop(self):
        s=self.image_snapshot();s['os']['windows'][1]['capture']['preview_crops'][0]['ocr']=[{'text':'古い画像'}];visible_text(s,'main','春の空。')
        with self.assertRaisesRegex(ValueError,'preview image'):preview_text(s,'春の空。')
    def test_preview_missing_or_ambiguous_crop_blocked(self):
        for crops in ([],[{},{}]):
            s=self.image_snapshot();s['os']['windows'][1]['capture']['preview_crops']=crops
            with self.assertRaises(EvidenceUnavailable):preview_text(s,'春の空。')
    def test_guard_commit_delivery_chain_has_separate_sequences(self):
        r=new_frames(self.frames(),self.image_snapshot());self.assertEqual(r[0][0]['worker_sequence'],4);self.assertEqual(r[0][1]['service_sequence'],'9')
    def test_metadata_only_is_not_fresh_committed_or_delivered_frame(self):
        for index in (1,2):
            f=self.frames();f[index]['fact']['frame']=None
            with self.assertRaises(EvidenceUnavailable):new_frames(f,self.image_snapshot())
    def test_rejection_old_timestamp_wrong_native_hash_or_empty_payload_rejected(self):
        for mutate in [lambda f:f[0]['fact'].update(accepted=False),lambda f:f[1]['fact']['frame'].update(captured_at_ms='2'),lambda f:f[1]['fact']['frame'].update(target_identity_hash='0'*64),lambda f:f[1]['fact']['frame'].update(encoded_len=0)]:
            f=self.frames();mutate(f)
            with self.assertRaises(EvidenceUnavailable):new_frames(f,self.image_snapshot())
    def test_true_sleep_then_wake_only_within_run_interval(self):
        start=int(datetime(2026,10,27,tzinfo=timezone.utc).timestamp()*1000);text='2026-10-27 00:01:00 +0000 Sleep Entering Sleep state due to Software Sleep\n2026-10-27 00:02:00 +0000 Wake Wake from Normal Sleep due to User\n';self.assertTrue(power_sleep_wake(text,start,start+180000))
        for wrong in [text.replace('Wake Wake from','DarkWake DarkWake from'),text.replace('Sleep Entering Sleep state','DisplaySleep display sleep'),text.replace('2026-10-27','2026-10-26')]:self.assertFalse(power_sleep_wake(wrong,start,start+180000))
    def test_lsof_requires_actual_fd_inode_and_path(self):self.assertEqual(lsof_entries('p10\nf8u\ni123\nn/private/target.lock\n')[0],{'fd':'8u','inode':'123','name':'/private/target.lock'})
    def test_missing_raw_material_cannot_pass_claimed_coverage(self):
        self.write('.build/run/verification.json',{'status':'passed','covered_obligations':9})
        with patch('runtime_stability_evidence.tree_hash',return_value='source'):r=validate_run(self.root,self.out,NONCE,'source')
        self.assertNotEqual(r['status'],'passed');self.assertFalse(r['product_packaged_native_game_tests_executed']);self.assertTrue(r['errors'])
    def test_missing_runtime_inputs_never_launches_or_uses_cargo_parser(self):
        with patch('product_execution.subprocess.run') as run,patch('product_execution.validate_cargo_output',side_effect=AssertionError('Cargo parser forbidden')):r=run_product_check(self.root,CHECK_ID,self.root/'.build/executions')
        run.assert_not_called();self.assertEqual(r['status'],'failed');self.assertFalse(r['product_packaged_native_game_tests_executed'])
    def test_runtime_dispatch_is_separate_and_fake_zero_assertions_rejected(self):
        x,p=self.inputs();fake={'status':'passed','errors':[],'cases':[],'fixture_substitution':False,'product_packaged_native_game_tests_executed':False,'assertion_count':0}
        def run(command,**kwargs):
            out=Path(command[command.index('--output')+1]);out.mkdir();r={**fake,'execution_nonce':command[-1],'executed_at_ms':int(time.time()*1000)};(out/'verification.json').write_text(json.dumps(r));return subprocess.CompletedProcess(command,0,'tool',None)
        with patch('product_execution.subprocess.run',side_effect=run),patch('product_execution.tree_digest',return_value='source'),patch('runtime_stability_evidence.validate_run',return_value=fake),patch('product_execution.validate_cargo_output',side_effect=AssertionError('Cargo parser forbidden')):r=run_product_check(self.root,CHECK_ID,self.root/'.build/executions','inputs.json')
        self.assertEqual(r['status'],'failed');self.assertTrue(any('concrete case' in e for e in r['errors']));self.assertTrue(any('substitution' in e for e in r['errors']))
    def test_snapshot_uses_actual_sqlite_schema_read_only(self):
        p=self.root/'profile.sqlite3';c=sqlite3.connect(p);c.executescript((ROOT/'crates/app-infra/src/storage/migrations/001_profile.sql').read_text());c.execute("insert into profile_meta values(1,'guard',0,1,'guard-server','active',0,0)");c.commit();c.close();before=p.read_bytes();r=snapshot_database(p,1);self.assertEqual(r['sessions'],[]);self.assertEqual(r['profile_meta'][0]['profile_id'],'guard');self.assertEqual(p.read_bytes(),before)
    def test_archive_preserves_prefix_and_rejects_rewritten_segment(self):
        logs=self.root/'logs';logs.mkdir();row={'execution_nonce':NONCE,'segment_index':0,'kind':'observation_begin'};p=logs/'runtime-observation.jsonl';p.write_text(json.dumps(row)+'\n');archive_trace(logs,self.out/'trace',NONCE);archive_trace(logs,self.out/'trace',NONCE);row['kind']='tampered';p.write_text(json.dumps(row)+'\n')
        with self.assertRaisesRegex(ValueError,'changed'):archive_trace(logs,self.out/'trace',NONCE)
    def test_case_inventory_cannot_shrink_to_generic_pass(self):self.assertEqual(len(CASE_IDS),10);self.assertIn('SR6-NATURAL-EXIT-SWITCH',CASE_IDS);self.assertIn('SR6-REAL-SLEEP-WAKE',CASE_IDS)

    def test_same_service_sequence_cannot_replace_payload_or_timestamp(self):
        f=self.frames();frame=f[1]['fact']['frame']
        for r in f[1:]: r['fact']['projection'].update({k:v for k,v in frame.items() if k in ('lease_id','generation','target_identity_hash','captured_at_ms')})
        self.assertEqual(len(validate_frame_metadata(f)[0]),1)
        changed=copy.deepcopy(f[2]);changed['fact']['frame']['content_hash']='d'*64;f.append(changed)
        with self.assertRaisesRegex(ValueError,'same service sequence'):validate_frame_metadata(f)
    def test_delivery_cursor_cannot_be_satisfied_by_equal_or_older_service_sequence(self):
        f=self.frames();frame=f[1]['fact']['frame']
        for r in f[1:]:r['fact']['projection'].update({k:v for k,v in frame.items() if k in ('lease_id','generation','target_identity_hash','captured_at_ms')})
        f[2]['fact']['requested_after_sequence']='9'
        with self.assertRaisesRegex(ValueError,'cursor'):validate_frame_metadata(f)
    def test_different_generations_may_retain_same_frame_time_after_pause(self):
        f=self.frames();frame=f[1]['fact']['frame']
        for r in f[1:]:r['fact']['projection'].update({k:v for k,v in frame.items() if k in ('lease_id','generation','target_identity_hash','captured_at_ms')})
        pause=copy.deepcopy(f[2]);pause['fact']['frame']['generation']='2';pause['fact']['projection']['generation']='2';f.append(pause)
        self.assertEqual(len(validate_frame_metadata(f)[0]),2)

    def test_old_running_health_cannot_close_final_observation_interval(self):
        rows,p,m=self.trace();self.close_trace_fixture(rows,p,m);m['observation_completed_at_ms']=1000
        m['observation_intervals']=[{'step_id':'final','boundary_ms':1000}]
        with self.assertRaisesRegex(ValueError,'interval|health'):inspect_trace(self.out,m,NONCE,10,90)
    def test_trailing_fact_after_last_health_is_not_accounted(self):
        rows,p,m=self.trace((1,2));self.close_trace_fixture(rows,p,m)
        rows.append({**rows[1],'attempt_sequence':6,'observed_at_ms':131})
        self.rewrite_trace(rows,p,m)
        with self.assertRaisesRegex(ValueError,'tail|health|interval'):inspect_trace(self.out,m,NONCE,10,90)
    def test_final_pending_attempt_cannot_be_promoted_by_old_maximum(self):
        rows,p,m=self.trace((1,2));self.close_trace_fixture(rows,p,m);rows[-1].update(attempted=6,enqueued=6)
        self.rewrite_trace(rows,p,m)
        with self.assertRaisesRegex(ValueError,'health|interval'):inspect_trace(self.out,m,NONCE,10,90)

    def host_tail(self):
        sid='actual-SID';pid=42;broker='broker';streams=[];sources=[];raw=[];payloads=[];stdout=[];refs=[];body=[]
        for n,(address,text,wire) in enumerate([('10','同じ。',7),('20','同じ。',11),('20','同じ。',16)],1):
            identity={'pid':pid,'address':address,'context':'2','context2':'3'}
            h=hashlib.sha256(f'{sid}:{pid}:{address}:2:3'.encode()).hexdigest();source_id='source-'+h
            if not any(r['source_id']==source_id for r in sources):
                sources.append({'source_id':source_id,'session_id':sid,'host_instance_id':broker,'hook_address_u64':address,'context_u64':'2','subcontext_u64':'3','hook_code':'HW0','hook_name':'TextOutW','stream_identity_sha256':h,'identity_extensions_json':json.dumps({'pid':pid})})
                streams.append({'id':identity,'key':f'{pid}:{address}:2:3','hook_code':'HW0','hook_name':'TextOutW','text_events':0,'history':[]})
            stream=next(r for r in streams if r['id']==identity);stream['text_events']+=1;stream['history'].append({'sequence':wire,'text':text})
            event_id=f'{sid}:{wire}';b=text.encode();h=hashlib.sha256(b).hexdigest()
            raw.append({'event_id':event_id,'session_id':sid,'source_id':source_id,'event_kind':'text','ingest_sequence':n,'wire_sequence_u64':str(wire),'integrity_state':'intact','payload_sha256':h,'payload_byte_length':len(b)})
            payloads.append({'event_id':event_id,'base64':base64.b64encode(b).decode()})
            stdout.append({'v':2,'event':'text','stream':identity,'hook_code':'HW0','hook_name':'TextOutW','text':text})
            body.append({'unit_id':str(n),'session_id':sid,'text_revision':1,'source_text':text,'source_sha256':h,'source_byte_length':len(b),'selection_revision':n})
            refs.append({'unit_id':str(n),'session_id':sid,'text_revision':1,'ref_ordinal':0,'event_id':event_id,'byte_start':0,'byte_end':len(b)})
        session={'session':{'session_id':sid,'windows_pid':pid,'broker_instance_id':broker,'target_exe_path':r'C:\game.exe','process_created_u64':'123','architecture':'x86'},'sources':sources,'raw':raw,'payloads':payloads,'refs':refs,'body':body,'revisions':body,'selections':[{'session_id':sid,'selection_revision':i+1,'source_id':raw[i]['source_id']} for i in range(len(raw))]}
        final={'session_id':sid,'target':{'pid':pid,'full_image_path':r'C:\game.exe','creation_filetime':'123','architecture':'x86'},'text_events':len(stdout),'streams':streams,'sequence':20}
        return session,stdout,final
    def test_host_tail_order_and_complete_stream_identity_cannot_be_substituted(self):
        from runtime_stability_evidence import inspect_host_tail
        for mutate in [lambda s,w,f:w.reverse(),lambda s,w,f:w[1]['stream'].update(context='9'),lambda s,w,f:w[0].update(hook_name='other')]:
            s,w,f=self.host_tail();mutate(s,w,f)
            with self.assertRaises(ValueError):inspect_host_tail(s,w,f)
    def test_host_tail_wire_sid_and_repeat_multiplicity_cannot_be_substituted(self):
        from runtime_stability_evidence import inspect_host_tail
        for mutate in [lambda s,w,f:s['raw'][1].update(wire_sequence_u64='12'),lambda s,w,f:s['raw'][1].update(session_id='other'),lambda s,w,f:s['raw'].pop(),lambda s,w,f:w.append(copy.deepcopy(w[-1]))]:
            s,w,f=self.host_tail();mutate(s,w,f)
            with self.assertRaises(ValueError):inspect_host_tail(s,w,f)
    def test_host_tail_accepts_real_duplicate_text_and_noncontiguous_client_wire(self):
        from runtime_stability_evidence import inspect_host_tail
        s,w,f=self.host_tail();self.assertEqual(inspect_host_tail(s,w,f),3)

    def test_missing_final_health_or_tail_written_fact_rejected(self):
        for remove in (-1,-3):
            rows,p,m=self.trace();self.close_trace_fixture(rows,p,m);rows.pop(remove);self.rewrite_trace(rows,p,m)
            with self.assertRaises(ValueError):inspect_trace(self.out,m,NONCE,10,90)
            import shutil
            shutil.rmtree(p.parent);shutil.rmtree(self.out/'trace-live')
    def test_every_step_requires_independent_boundary_health(self):
        rows,p,m=self.trace();self.close_trace_fixture(rows,p,m);m['steps']=[{'step_id':'sr/reading-1','collected_at_ms':115}]
        with self.assertRaisesRegex(ValueError,'every required step'):inspect_trace(self.out,m,NONCE,10,90)
    def test_frozen_trace_cannot_claim_normalized_but_different_live_bytes(self):
        rows,p,m=self.trace();self.close_trace_fixture(rows,p,m);live=self.out/next(iter(m['live_trace_files']));live.write_text(live.read_text().replace(': ',':'));m['live_trace_files']={str(live.relative_to(self.out)):hashlib.sha256(live.read_bytes()).hexdigest()}
        with self.assertRaisesRegex(ValueError,'bytes'):inspect_trace(self.out,m,NONCE,10,90)
    def test_readonly_closure_waits_for_later_actual_health_and_keeps_full_live_archive(self):
        import threading
        rows,p,m=self.trace();logs=self.root/'actual-log';logs.mkdir();source=logs/'runtime-observation.jsonl';source.write_bytes(p.read_bytes());self.close_trace_fixture(rows,p,m)
        completed=p.read_bytes()
        def writer():
            time.sleep(0.05);source.write_bytes(completed)
        worker=threading.Thread(target=writer);worker.start()
        cp=wait_trace_interval(logs,self.out/'new-live',NONCE,10,90,119,'final',time.monotonic()+2,threading.Lock());worker.join()
        self.assertEqual(cp['closing_health'],[0,len(rows)-1]);freeze_trace(self.out/'new-live',self.out/'new-frozen',cp['closing_health']);self.assertEqual((self.out/'new-frozen'/p.name).read_bytes(),completed)
    def test_pending_tail_bounded_readonly_wait_fails_without_business_wait(self):
        import threading
        rows,p,m=self.trace();logs=self.root/'actual-log';logs.mkdir();(logs/'runtime-observation.jsonl').write_bytes(p.read_bytes())
        with self.assertRaisesRegex(ValueError,'bounded read-only'):wait_trace_interval(logs,self.out/'new-live',NONCE,10,90,119,'final',time.monotonic(),threading.Lock())
    def test_host_history_truncation_wrong_wire_and_cross_stream_source_refs_rejected(self):
        from runtime_stability_evidence import inspect_host_tail
        for mutate in [lambda s,w,f:f['streams'][1]['history'].pop(0),lambda s,w,f:s['raw'][1].update(event_id='actual-SID:12',wire_sequence_u64='12'),lambda s,w,f:s['refs'][0].update(event_id=s['raw'][1]['event_id']),lambda s,w,f:s['sources'][0].update(host_instance_id='other')]:
            s,w,f=self.host_tail();mutate(s,w,f)
            with self.assertRaises((ValueError,KeyError)):inspect_host_tail(s,w,f)
    def test_wordcard_retention_action_does_not_request_legal_outside_click(self):
        actions=dict(plan_steps());self.assertIn('不点击HUD',actions['mo/wordcard-retained']);self.assertNotIn('并切换注音',actions['mo/wordcard-retained'])

    def test_wrong_wire_rejected_even_when_all_event_id_joins_are_consistently_rewritten(self):
        from runtime_stability_evidence import inspect_host_tail
        s,w,f=self.host_tail();old=s['raw'][1]['event_id'];new='actual-SID:12'
        for rows in (s['raw'],s['payloads'],s['refs']):
            for row in rows:
                if row['event_id']==old:row['event_id']=new
        s['raw'][1]['wire_sequence_u64']='12'
        with self.assertRaisesRegex(ValueError,'actual client wire'):inspect_host_tail(s,w,f)

    def zero_trace(self):
        common={'execution_nonce':NONCE,'run_id':'r','segment_index':0}
        begin={**common,'kind':'observation_begin','pid':10,'initialized_at_ms':100}
        health={**common,'kind':'observation_health','attempted':0,'enqueued':0,'processed':0,'written_facts':0,'dropped':0,'invalid':0,'write_failures':0,'max_observed_attempt':0,'last_written_attempt':0,'rotation':0,'terminal':False}
        rows=[begin,{**health,'at_ms':120},{**health,'at_ms':130}]
        p=self.out/'trace/segment-00000000.jsonl';p.parent.mkdir();m={'trace_files':{},'archive_errors':[],'steps':[],'observation_completed_at_ms':110,'completed_at_ms':131,'observation_intervals':[{'step_id':'final','boundary_ms':110,'opening_health':[0,1],'closing_health':[0,2]}]};self.rewrite_trace(rows,p,m)
        live=self.out/'trace-live'/p.name;live.parent.mkdir();live.write_bytes(p.read_bytes());m['live_trace_files']={'trace-live/'+p.name:hashlib.sha256(live.read_bytes()).hexdigest()}
        return rows,p,m
    def test_cold_zero_fact_interval_closes_from_actual_zero_health(self):
        import threading
        rows,p,m=self.zero_trace();logs=self.root/'actual-log';logs.mkdir();(logs/'runtime-observation.jsonl').write_bytes(p.read_bytes())
        cp=wait_trace_interval(logs,self.out/'new-live',NONCE,10,90,110,'sr/cold',time.monotonic(),threading.Lock())
        self.assertEqual(cp,{'step_id':'sr/cold','boundary_ms':110,'opening_health':[0,1],'closing_health':[0,2]})
    def test_no_fact_nonzero_pending_attempt_or_missing_health_cannot_close_cold(self):
        for mutation in ('attempted','enqueued','written_facts','missing_health','fake_maximum'):
            rows,p,m=self.zero_trace()
            if mutation=='missing_health':rows=rows[:1]
            elif mutation=='fake_maximum':rows[-1]['max_observed_attempt']=1
            else:rows[-1][mutation]=1
            self.rewrite_trace(rows,p,m);parsed=load_trace_archive(self.out,m['trace_files'],NONCE)
            with self.assertRaises(ValueError):trace_interval(parsed,10,90,110,'sr/cold')
            import shutil
            shutil.rmtree(p.parent);shutil.rmtree(self.out/'trace-live')
    def test_whole_run_with_only_zero_health_still_lacks_actual_facts(self):
        rows,p,m=self.zero_trace()
        with self.assertRaisesRegex(ValueError,'actual metadata facts'):inspect_trace(self.out,m,NONCE,10,90)


    def test_native_preview_observer_frame_geometry_and_diagnostics(self):
        # Guard-only generated data. Compile the current observer source itself;
        # do not substitute JSON fixtures for native AXValue/CGFloat storage.
        observer=ROOT/'scripts/quality/runtime_observer.swift'
        source=observer.read_text()
        original_sha=hashlib.sha256(observer.read_bytes()).hexdigest()
        self.assertIn('row["frame"]=serializedAXFrame(pt,sz)',source)
        self.assertIn('let captureArguments=windowCaptureArguments(wid,target)',source)
        self.assertIn('task.arguments=captureArguments',source)
        self.assertIn('let observed=observeCapturedWindowText(bounds,cg)',source)
        harness=r"""import AppKit
import ApplicationServices
import Foundation

struct NativeVerificationFailure: Error { let message: String }
struct NativePreviewVerification {
    static func main() throws {
        var checks=[[String:Any]]()
        func check(_ value: Bool, _ name: String, _ evidence: [String:Any] = [:]) throws {
            guard value else { throw NativeVerificationFailure(message:name) }
            var item=evidence;item["id"]=name;item["status"]="passed";checks.append(item)
        }
        func rejects(_ expected: PreviewCropFailure, _ name: String, _ run: () throws -> Void) throws {
            do {try run();throw NativeVerificationFailure(message:name+" unexpectedly accepted")}
            catch let failure as PreviewCropFailure {try check(failure==expected,name,["reason":failure.rawValue])}
        }
        func rectArray(_ rect: CGRect) -> [Double] { [Double(rect.minX),Double(rect.minY),Double(rect.width),Double(rect.height)] }
        // Real AXValue extraction, native CGFloat storage, and Any-valued
        // dictionaries. There is deliberately no JSON decoding in this path.
        var originalPoint=CGPoint(x:2260,y:296), originalSize=CGSize(width:280,height:176)
        let position=AXValueCreate(.cgPoint,&originalPoint)!, size=AXValueCreate(.cgSize,&originalSize)!
        var extractedPoint=CGPoint.zero, extractedSize=CGSize.zero
        try check(AXValueGetValue(position,.cgPoint,&extractedPoint) && AXValueGetValue(size,.cgSize,&extractedSize),"native_axvalue_extract")
        let legacy:[String:Any]=["frame":[extractedPoint.x,extractedPoint.y,extractedSize.width,extractedSize.height]]
        try check(legacy["frame"] as? [Double] == nil,"red_original_native_frame_cast",[
            "observed_native_type":String(describing:type(of:legacy["frame"]!)),"double_array_cast_succeeded":false,
            "expected_original_failure":"ax_frame_type_or_count_invalid"])
        let fixed:[String:Any]=["frame":serializedAXFrame(extractedPoint,extractedSize),"description":"native generated image · 目标窗口当前画面"]
        try check(fixed["frame"] as? [Double] == [2260,296,280,176],"green_fixed_native_frame_cast",[
            "observed_native_type":String(describing:type(of:fixed["frame"]!))])
        let mainBounds:[String:Any]=["X":Double(0),"Y":Double(30),"Width":Double(2560),"Height":Double(2850)]
        let frame=fixed["frame"] as! [Double]
        let expected=CGRect(x:2260,y:266,width:280,height:176)
        let legacyShadow=CGRect(x:2260.0*2672.0/2560.0,y:266.0*2962.0/2850.0,
            width:280.0*2672.0/2560.0,height:176.0*2962.0/2850.0).integral
        try check(legacyShadow != expected,"red_shadow_inclusive_geometry",["old_pixel_rect":rectArray(legacyShadow),"correct_shadow_free_rect":rectArray(expected)])
        let noShadow=try previewPixelRect(mainBounds,frame,2560,2850)
        try check(noShadow==expected,"green_shadow_free_geometry",["pixel_rect":rectArray(noShadow)])
        let retina=try previewPixelRect(mainBounds,frame,5120,5700)
        try check(retina==CGRect(x:4520,y:532,width:560,height:352),"retina_scale_geometry",["pixel_rect":rectArray(retina)])
        let negativeBounds:[String:Any]=["X":Double(-128),"Y":Double(30),"Width":Double(1030),"Height":Double(748)]
        let negative=try previewPixelRect(negativeBounds,serializedAXFrame(CGPoint(x:-100,y:60),CGSize(width:200,height:100)),1030,748)
        try check(negative==CGRect(x:28,y:30,width:200,height:100),"negative_desktop_origin_geometry")
        // Independent x/y pixel scales, with a nonzero offset and negative
        // desktop origin: (28,30,200,100) logical becomes (56,90,400,300).
        let nonuniform=try previewPixelRect(negativeBounds,
            serializedAXFrame(CGPoint(x:-100,y:60),CGSize(width:200,height:100)),2060,2244)
        try check(nonuniform==CGRect(x:56,y:90,width:400,height:300),
            "nonuniform_xy_scale_geometry",["pixel_rect":rectArray(nonuniform)])
        let args=windowCaptureArguments(8016,URL(fileURLWithPath:"/test-only/output.png"))
        try check(args == ["-x","-a","-o","-l","8016","/test-only/output.png"],"shadow_exclusion_arguments",["arguments":args])
        // A white image generated in memory: no display, AX inventory, file
        // capture, or screencapture subprocess is used by this test binary.
        let context=CGContext(data:nil,width:320,height:240,bitsPerComponent:8,bytesPerRow:320*4,
            space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(CGColor(gray:1,alpha:1));context.fill(CGRect(x:0,y:0,width:320,height:240))
        let blank=context.makeImage()!
        // Actual generated CGImage dimensions exercise the production canvas
        // gate. These are synthetic guard fixtures, never live window captures.
        let retinaBounds:[String:Any]=["X":-20.0,"Y":-10.0,"Width":160.0,"Height":120.0]
        let retinaCanvas=validateCaptureCanvas(retinaBounds,blank)
        try check(retinaCanvas.failure==nil && retinaCanvas.diagnostic["pixel_scale"] as? Double == 2,
            "capture_canvas_uniform_retina",["diagnostic":retinaCanvas.diagnostic])
        let nonstandardBounds:[String:Any]=["X":0.0,"Y":0.0,"Width":320.0,"Height":240.0]
        let nonstandardCanvas=validateCaptureCanvas(nonstandardBounds,generatedImage(480,360))
        try check(nonstandardCanvas.failure==nil && nonstandardCanvas.diagnostic["pixel_scale"] as? Double == 1.5,
            "capture_canvas_uniform_nonstandard_scale",["diagnostic":nonstandardCanvas.diagnostic])
        func generatedImage(_ width:Int,_ height:Int) -> CGImage {
            CGContext(data:nil,width:width,height:height,bitsPerComponent:8,bytesPerRow:width*4,
                space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue)!.makeImage()!
        }
        let family=generatedImage(1120,1104)
        let hudBounds:[String:Any]=["X":794.0,"Y":363.0,"Width":560.0,"Height":205.0]
        let familyHUD=validateCaptureCanvas(hudBounds,family)
        try check(familyHUD.failure=="capture_canvas_scale_mismatch",
            "capture_canvas_hud_union_rejected",["diagnostic":familyHUD.diagnostic])
        let cardBounds:[String:Any]=["X":963.0,"Y":455.0,"Width":360.0,"Height":460.0]
        let familyCard=validateCaptureCanvas(cardBounds,family)
        try check(familyCard.failure=="capture_canvas_scale_mismatch",
            "capture_canvas_wordcard_union_rejected",["diagnostic":familyCard.diagnostic])
        let isolatedCard=validateCaptureCanvas(cardBounds,generatedImage(720,920))
        try check(isolatedCard.failure==nil && isolatedCard.diagnostic["pixel_scale"] as? Double == 2,
            "capture_canvas_isolated_card_accepts",["diagnostic":isolatedCard.diagnostic])
        let invalidCanvas=validateCaptureCanvas(["X":0.0,"Y":0.0,"Width":0.0,"Height":120.0],blank)
        try check(invalidCanvas.failure=="capture_canvas_invalid_bounds",
            "capture_canvas_invalid_bounds_rejected",["diagnostic":invalidCanvas.diagnostic])
        let overflowBounds:[String:Any]=["X":0.0,"Y":0.0,"Width":Double.greatestFiniteMagnitude,"Height":Double.greatestFiniteMagnitude]
        let overflowCanvas=validateCaptureCanvas(overflowBounds,blank)
        try check(overflowCanvas.failure=="capture_canvas_scale_mismatch",
            "capture_canvas_finite_product_overflow_rejected",["diagnostic":overflowCanvas.diagnostic])
        let tinyBounds:[String:Any]=["X":0.0,"Y":0.0,"Width":Double.leastNonzeroMagnitude,"Height":Double.leastNonzeroMagnitude]
        let tinyCanvas=validateCaptureCanvas(tinyBounds,blank)
        try check(tinyCanvas.failure=="capture_canvas_invalid_scale" && JSONSerialization.isValidJSONObject(tinyCanvas.diagnostic),
            "capture_canvas_scale_overflow_json_safe",["diagnostic":tinyCanvas.diagnostic])
        let nanCanvas=validateCaptureCanvas(["X":Double.nan,"Y":0.0,"Width":160.0,"Height":120.0],blank)
        let infinityCanvas=validateCaptureCanvas(["X":0.0,"Y":0.0,"Width":Double.infinity,"Height":120.0],blank)
        try check(nanCanvas.failure=="capture_canvas_invalid_bounds" && infinityCanvas.failure=="capture_canvas_invalid_bounds" &&
            JSONSerialization.isValidJSONObject(nanCanvas.diagnostic) && JSONSerialization.isValidJSONObject(infinityCanvas.diagnostic),
            "capture_canvas_nonfinite_bounds_json_safe",["nan_diagnostic":nanCanvas.diagnostic,"infinity_diagnostic":infinityCanvas.diagnostic])
        var recognitionCalls=0
        let rejectedText=observeCapturedWindowText(hudBounds,family,recognize:{_ in recognitionCalls+=1;return []})
        let rejectedCalls=recognitionCalls
        let acceptedText=observeCapturedWindowText(retinaBounds,blank,recognize:{_ in recognitionCalls+=1;return []})
        try check(rejectedText.failure=="capture_canvas_scale_mismatch" && rejectedText.lines==nil &&
            rejectedCalls==0 && acceptedText.failure==nil && acceptedText.lines?.isEmpty==true && recognitionCalls==1,
            "capture_canvas_rejection_skips_ocr",["rejected_ocr_calls":rejectedCalls,"accepted_ocr_calls":recognitionCalls])
        try check(retinaCanvas.diagnostic["actual_canvas_origin_observed"] as? Bool == false &&
            retinaCanvas.diagnostic["status"] as? String == "uniform_dimensions",
            "capture_canvas_origin_not_claimed_observed")
        let smallBounds:[String:Any]=["X":Double(0),"Y":Double(0),"Width":Double(320),"Height":Double(240)]
        let noTextNode:[String:Any]=["frame":serializedAXFrame(CGPoint(x:20,y:30),CGSize(width:280,height:176)),"description":"native generated blank · 目标窗口当前画面"]
        let prepared=try preparePreviewCrop(blank,smallBounds,noTextNode)
        let words=prepared.metadata["ocr"] as? [[String:Any]]
        try check(words != nil && words!.isEmpty && !prepared.png.isEmpty && prepared.reason=="crop_saved_ocr_empty",
            "native_vision_no_text_retains_crop",["crop_width":prepared.metadata["width"]!,"crop_height":prepared.metadata["height"]!,
            "ocr_lines":words!.count,"png_bytes":prepared.png.count,"diagnostic":prepared.reason,"product_sentence_evidence":false])
        let artifacts=URL(fileURLWithPath:CommandLine.arguments[1],isDirectory:true)
        let blankPath=artifacts.appendingPathComponent("native-generated-blank-preview.png")
        try prepared.png.write(to:blankPath,options:.atomic)
        try rejects(.axFrameTypeOrCountInvalid,"native_legacy_frame_diagnostic") { _=try preparePreviewCrop(blank,smallBounds,legacy) }
        let outside:[String:Any]=["frame":serializedAXFrame(CGPoint(x:300,y:220),CGSize(width:40,height:40))]
        try rejects(.axFrameOutsideWindow,"native_out_of_bounds_diagnostic") { _=try preparePreviewCrop(blank,smallBounds,outside) }
        let malformed:[String:Any]=["frame":[Double(0),Double(0),Double(4)]]
        try rejects(.axFrameTypeOrCountInvalid,"native_malformed_frame_diagnostic") { _=try preparePreviewCrop(blank,smallBounds,malformed) }
        let nonfinite:[String:Any]=["frame":[Double.nan,Double(0),Double(40),Double(40)]]
        try rejects(.axFrameInvalid,"native_nonfinite_frame_diagnostic") { _=try preparePreviewCrop(blank,smallBounds,nonfinite) }
        try rejects(.windowBoundsInvalid,"native_invalid_bounds_diagnostic") { _=try preparePreviewCrop(blank,["X":0.0],noTextNode) }
        try rejects(.cropOCRUnavailable,"injected_ocr_exception_diagnostic") { _=try preparePreviewCrop(blank,smallBounds,noTextNode,recognize:{_ in nil}) }
        let report:[String:Any]=["schema_version":1,"kind":"native_observer_guard_verification",
            "runtime_capture_executed":false,"json_roundtrip_in_native_data_path":false,"product_runtime_acceptance":"not_run",
            "checks":checks,"assertions":checks.count,"native_generated_blank_crop":blankPath.lastPathComponent,
            "empty_crop_metadata":prepared.metadata]
        let data=try JSONSerialization.data(withJSONObject:report,options:[.prettyPrinted,.sortedKeys])
        FileHandle.standardOutput.write(data);FileHandle.standardOutput.write(Data("\n".utf8))
    }
}

try NativePreviewVerification.main()
"""
        native_source=self.root/'native-preview-main.swift'
        native_source.write_text(source+'\n'+harness)
        executable=self.root/'native-preview-tests'
        module_cache=self.root/'swift-module-cache'
        commands=[
            ('production-typecheck',['xcrun','swiftc','-module-cache-path',str(module_cache),'-typecheck',str(observer)]),
            ('native-test-compile',['xcrun','swiftc','-module-cache-path',str(module_cache),'-D','RUNTIME_OBSERVER_NATIVE_TEST',str(native_source),'-o',str(executable)]),
            ('native-test-run',[str(executable),str(self.root)]),
        ]
        native=None
        for label,command in commands:
            completed=subprocess.run(command,text=True,capture_output=True,timeout=120)
            (self.root/(label+'.stdout')).write_text(completed.stdout)
            (self.root/(label+'.stderr')).write_text(completed.stderr)
            self.assertEqual(completed.returncode,0,label+' failed:\n'+completed.stdout[-4000:]+'\n'+completed.stderr[-8000:])
            if label=='native-test-run':native=json.loads(completed.stdout)
        expected_ids=[
            'native_axvalue_extract','red_original_native_frame_cast','green_fixed_native_frame_cast',
            'red_shadow_inclusive_geometry','green_shadow_free_geometry','retina_scale_geometry',
            'negative_desktop_origin_geometry','nonuniform_xy_scale_geometry','shadow_exclusion_arguments',
            'capture_canvas_uniform_retina','capture_canvas_uniform_nonstandard_scale','capture_canvas_hud_union_rejected',
            'capture_canvas_wordcard_union_rejected','capture_canvas_isolated_card_accepts','capture_canvas_invalid_bounds_rejected',
            'capture_canvas_finite_product_overflow_rejected','capture_canvas_scale_overflow_json_safe',
            'capture_canvas_nonfinite_bounds_json_safe','capture_canvas_rejection_skips_ocr',
            'capture_canvas_origin_not_claimed_observed','native_vision_no_text_retains_crop',
            'native_legacy_frame_diagnostic','native_out_of_bounds_diagnostic','native_malformed_frame_diagnostic',
            'native_nonfinite_frame_diagnostic','native_invalid_bounds_diagnostic','injected_ocr_exception_diagnostic',
        ]
        self.assertEqual(native['kind'],'native_observer_guard_verification')
        self.assertEqual(native['assertions'],len(expected_ids))
        self.assertEqual([check['id'] for check in native['checks']],expected_ids)
        self.assertTrue(all(check['status']=='passed' for check in native['checks']))
        self.assertIs(native['runtime_capture_executed'],False)
        self.assertIs(native['json_roundtrip_in_native_data_path'],False)
        self.assertEqual(native['product_runtime_acceptance'],'not_run')
        by_id={check['id']:check for check in native['checks']}
        self.assertEqual(by_id['red_original_native_frame_cast']['observed_native_type'],'Array<CGFloat>')
        self.assertIs(by_id['red_original_native_frame_cast']['double_array_cast_succeeded'],False)
        self.assertEqual(by_id['green_fixed_native_frame_cast']['observed_native_type'],'Array<Double>')
        no_text=by_id['native_vision_no_text_retains_crop']
        self.assertEqual(no_text['ocr_lines'],0)
        self.assertEqual(no_text['diagnostic'],'crop_saved_ocr_empty')
        self.assertIs(no_text['product_sentence_evidence'],False)
        blank=self.root/native['native_generated_blank_crop']
        blob=blank.read_bytes()
        self.assertEqual(len(blob),no_text['png_bytes'])
        self.assertGreater(len(blob),24)
        self.assertEqual(blob[:8],b'\x89PNG\r\n\x1a\n')
        import struct
        self.assertEqual(struct.unpack('>II',blob[16:24]),(280,176))
        self.assertEqual(native['empty_crop_metadata']['ocr'],[])
        snapshot=self.image_snapshot()
        snapshot['os']['windows'][1]['capture']['preview_crops']=[dict(native['empty_crop_metadata'],png=blank.name)]
        # The image exists, but its real Vision result is empty. Keep the
        # independent product evidence assertion unchanged and require refusal.
        with self.assertRaisesRegex(EvidenceUnavailable,'^preview crop OCR unavailable$'):
            preview_text(snapshot,'guard-only independent sentence; generated blank has no text')
        self.assertEqual(hashlib.sha256(observer.read_bytes()).hexdigest(),original_sha)
