#!/usr/bin/env python3
"""Collect a real signed-package stability run. No fixture or UI injection path.

Human barriers declare ordinary actions/windows, never their result. All pass
judgments are recomputed by runtime_stability_evidence.py from raw observations.
"""
from __future__ import annotations
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import time
import threading
import uuid
from gate_core import digest, safe_path
from lifecycle_impact import unique_json, tree_hash
from package_identity import inspect_package, read_signature_metadata

CHECK_ID='product-packaged-stability-tests'
BUDGET_SECONDS=7200
STEP_WAIT_SECONDS=600
OBSERVATION_CLOSE_SECONDS=15
GAMES=('sr','mo','never7','sakura10th')
ROLES={'game','main','hud','preview','wordcard'}
MAX_RAW_BYTES=64*1024*1024

def plan_steps():
    result=[]
    for game in GAMES:
        result += [(game+'/cold','正常启动游戏，进入对白，尚不关联 Hook'),(game+'/connected','从当前客户端真实入口核对并关联游戏，显式选择正文流')]
        result += [(game+f'/reading-{i}','正常推进一句后停住，保持游戏、主窗、HUD与预览可核对') for i in range(1,6)]
        if game!='sr':
            result += [(game+'/wordcard-open','点当前真实日文词语打开词卡，保持词卡窗口'),(game+'/wordcard-retained','只切回游戏，继续推进长短句触发自然内容自适应；不点击HUD、不切换注音、不拖动，保持旧词卡')]
        if game=='sakura10th':
            result += [(game+'/isolation-before','保持预览运行，记录消费者切换前'),(game+'/isolation-after','切换页签，关闭并从菜单恢复主窗，不重选/重启采集'),(game+'/sleep-before','保持稳定正文与预览，准备真实系统睡眠'),(game+'/wake-after','执行真实系统睡眠并唤醒，保持同应用/游戏，自动恢复后推进一句；不重选/重启')]
            for action in ('user-geometry','escape','outside-pointer','owner-scroll'):
                result += [(game+'/card-'+action+'-open','再次正常点词打开词卡'),(game+'/card-'+action+'-closed','正常执行 '+action+'，保留窗口现状')]
        result.append((game+'/exited','正常退出该游戏，保留 Luna 运行；未确认旧 Host 和锁释放前不启动下一游戏'))
    return result

def existing_file(value):
    p=Path(value)
    if not p.is_absolute() or p.is_symlink() or not p.is_file(): raise ValueError('explicit regular absolute file required: '+str(value))
    return p.resolve()

def load_inputs(path, root):
    x=unique_json(path)
    keys={'schema_version','artifact','application','profile_database','profile_registry','profile_id','log_directory','games'}
    if set(x)!=keys or x['schema_version']!=1: raise ValueError('runtime inputs have invalid/unknown fields')
    safe_path(root,x['artifact'])
    for key in ('profile_database','profile_registry'): existing_file(x[key])
    app=Path(x['application'])
    if not app.is_absolute() or app.is_symlink() or not app.is_dir(): raise ValueError('explicit installed regular application required')
    existing_file(str(app/'Contents/MacOS/luna-reader'))
    if not Path(x['log_directory']).is_absolute() or Path(x['log_directory']).is_symlink(): raise ValueError('explicit regular app log directory required')
    if Path(x['profile_database']).parent.name!=x['profile_id']: raise ValueError('profile DB and ID differ')
    profiles=unique_json(x['profile_registry'])
    if profiles.get('activeProfileId')!=x['profile_id']: raise ValueError('runtime profile is not the actual active profile')
    if [g.get('id') for g in x['games']]!=list(GAMES): raise ValueError('exact ordered four-game inventory required; Ever17 excluded')
    for g in x['games']:
        if set(g)!={'id','environment_key','target_exe_path','architecture','source_language'} or not all(isinstance(v,str) and v for v in g.values()): raise ValueError('full explicit game identity required')
        if g['architecture'] not in ('x86','x64') or g['source_language']!=('zh-CN' if g['id']=='sr' else 'ja'): raise ValueError('game architecture/language invalid')
    return x

def run_raw(command, path, *, timeout=60):
    result=subprocess.run(command,text=True,capture_output=True,timeout=timeout)
    raw=result.stdout+'\n'+result.stderr
    if len(raw.encode())>MAX_RAW_BYTES: raise ValueError('bounded raw command output exceeded; evidence truncated is forbidden')
    path.write_text(raw)
    return {'command':command,'exit_code':result.returncode,'path':path.name,'sha256':digest(path)}

def snapshot_database(path, since_ms):
    c=sqlite3.connect(Path(path).as_uri()+'?mode=ro',uri=True,timeout=3)
    c.row_factory=sqlite3.Row
    try:
        c.execute('pragma query_only=on');c.execute('begin')
        def rows(sql,args=()): return [dict(r) for r in c.execute(sql,args)]
        sessions=rows("select * from sessions where source_kind='hook' and started_at_ms>=? order by started_at_ms,session_id",(since_ms,))
        result={'kind':'short_readonly_sqlite_snapshot','observed_at_ms':time.time_ns()//1000000,'profile_meta':rows('select * from profile_meta'),'sessions':[]}
        for s in sessions:
            sid=s['session_id'];d={'session':s}
            d['operations']=rows('select * from capture_operations where session_id=?',(sid,))
            d['body']=rows('select p.record_sequence,r.*,u.state unit_state from body_record_positions p join reading_unit_revisions r on r.unit_id=p.unit_id and r.text_revision=p.text_revision join reading_units u on u.unit_id=r.unit_id where p.session_id=? order by p.record_sequence',(sid,))
            d['sources']=rows('select * from capture_sources where session_id=?',(sid,));d['selections']=rows('select * from source_selections where session_id=?',(sid,))
            d['revisions']=rows('select r.* from reading_unit_revisions r join reading_units u on u.unit_id=r.unit_id where u.session_id=? order by r.unit_id,r.text_revision',(sid,))
            d['raw']=rows('select * from raw_events where session_id=? order by ingest_sequence',(sid,));d['hook_events']=rows('select * from hook_events where session_id=? order by event_sequence',(sid,))
            d['refs']=rows('select * from source_refs where session_id=? order by unit_id,text_revision,ref_ordinal',(sid,))
            payloads=rows('select p.event_id,p.payload_bytes from raw_event_payloads p join raw_events r on r.event_id=p.event_id where r.session_id=?',(sid,))
            d['payloads']=[{'event_id':r['event_id'],'base64':base64.b64encode(r['payload_bytes']).decode()} for r in payloads]
            d['lexical']=rows('select l.* from lexical_snapshots l join reading_units r on r.unit_id=l.unit_id where r.session_id=?',(sid,))
            d['tokens']=rows('select t.* from lexical_tokens t join lexical_snapshots l on l.lexical_id=t.lexical_id join reading_units r on r.unit_id=l.unit_id where r.session_id=? order by t.lexical_id,t.token_index',(sid,))
            result['sessions'].append(d)
        return result
    finally: c.rollback();c.close()

def children(pid):
    # Do not capture other applications' command-line arguments.
    raw=subprocess.run(['/bin/ps','-axo','pid=,ppid='],text=True,capture_output=True,check=True,timeout=10).stdout
    parents={int(p):int(pp) for p,pp in (line.split() for line in raw.splitlines())};owned=set()
    changed=True
    while changed:
        old=len(owned)
        owned.update(p for p,pp in parents.items() if pp==pid or pp in owned)
        changed=len(owned)!=old
    out=[]
    for p in sorted(owned):
        command=subprocess.run(['/bin/ps','-p',str(p),'-o','command='],text=True,capture_output=True,timeout=5).stdout.strip()
        if command: out.append({'pid':p,'command':command})
    return out

def copy_raw(source, dest):
    if source.is_symlink() or not source.is_file(): return False
    data=source.read_bytes()
    if len(data)>MAX_RAW_BYTES: raise ValueError('raw runtime evidence exceeds bounded file size: '+source.name)
    dest.write_bytes(data);return True

def archive_trace(logs, directory, nonce):
    directory.mkdir(exist_ok=True)
    grouped={}
    for name in ('runtime-observation.previous.jsonl','runtime-observation.jsonl'):
        path=logs/name
        if not path.is_file() or path.is_symlink(): continue
        for line in path.read_bytes().split(b'\n')[:-1]:
            if not line: continue
            row=json.loads(line)
            if row.get('execution_nonce')!=nonce: continue
            segment=row.get('segment_index')
            if type(segment) is not int or segment<0: raise ValueError('invalid actual observer segment')
            grouped.setdefault(segment,[]).append(line+b'\n')
    for segment,lines in grouped.items():
        target=directory/f'segment-{segment:08}.jsonl';data=b''.join(lines)
        if target.exists():
            old=target.read_bytes()
            if len(data)<len(old):
                if not old.startswith(data): raise ValueError('observer archived segment changed')
                continue
            if not data.startswith(old): raise ValueError('observer archived segment changed')
        if len(data)>MAX_RAW_BYTES: raise ValueError('observer archive segment exceeds bound')
        target.write_bytes(data)

def wait_trace_interval(logs,directory,nonce,pid,started_ms,boundary_ms,step_id,deadline,lock):
    from runtime_stability_evidence import EvidenceUnavailable,load_trace_archive,trace_interval
    end=min(deadline,time.monotonic()+OBSERVATION_CLOSE_SECONDS)
    while True:
        try:
            with lock:
                archive_trace(logs,directory,nonce)
                files={p.relative_to(directory.parent).as_posix():digest(p) for p in directory.glob('*.jsonl')}
                rows=load_trace_archive(directory.parent,files,nonce)
            return trace_interval(rows,pid,started_ms,boundary_ms,step_id)
        except EvidenceUnavailable as e:
            if time.monotonic()>=end:raise EvidenceUnavailable('bounded read-only observation interval did not close: '+str(e)) from e
            # This only waits for actual writer health, never for business exit.
            time.sleep(min(0.2,max(0,end-time.monotonic())))

def freeze_trace(live,directory,closing):
    directory.mkdir(exist_ok=False)
    segment,offset=closing
    for p in sorted(live.glob('*.jsonl')):
        number=int(p.stem.split('-')[1])
        if number>segment:continue
        data=p.read_bytes()
        if number==segment:data=b''.join(data.splitlines(keepends=True)[:offset+1])
        (directory/p.name).write_bytes(data)

def collect_step(root, directory, x, nonce, started_ms, pid, swift, step, response):
    directory.mkdir(mode=0o700)
    db=snapshot_database(x['profile_database'],started_ms)
    (directory/'database.json').write_text(json.dumps(db,ensure_ascii=False,indent=2)+'\n')
    own=children(pid);(directory/'owned-processes.json').write_text(json.dumps(own,indent=2)+'\n')
    pids=sorted({pid,response['game_host_pid'],*[r['pid'] for r in own if 'lunahook-host.exe' in r['command']]})
    command=[str(swift),'--output',str(directory/'os.json'),'--pids',','.join(map(str,pids)),'--windows',','.join(map(str,sorted(set(response['windows'].values()))))]
    r=subprocess.run(command,capture_output=True,text=True,timeout=60)
    (directory/'os-command.json').write_text(json.dumps({'command':command,'exit_code':r.returncode,'stderr':r.stderr},indent=2)+'\n')
    if r.returncode: raise ValueError('independent OS observation failed: '+r.stderr[:400])
    commands=[run_raw(['/usr/sbin/lsof','-nP','-p',str(pid),'-F','pfin'],directory/'lsof.txt',timeout=30)]
    if step.endswith(('/sleep-before','/wake-after')): commands.append(run_raw(['/usr/bin/pmset','-g','log'],directory/'power.txt',timeout=60))
    (directory/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
    logs=Path(x['log_directory'])
    for name in ('runtime-observation.previous.jsonl','runtime-observation.jsonl','geometry-lifecycle.jsonl'):copy_raw(logs/name,directory/name)
    for d in db['sessions']:
        sid=d['session']['session_id']
        if not re.fullmatch(r'[A-Za-z0-9_.:-]{1,128}',sid): raise ValueError('unsafe actual SID')
        target=directory/'host'/sid;target.mkdir(parents=True)
        for name in ('requests.jsonl','stdout.jsonl','stderr.txt','target-probe.json','final-snapshot.json'):copy_raw(logs/'sessions'/sid/name,target/name)
    profile=unique_json(x['profile_registry'])
    seal=Path(x['application'])/'Contents/Resources/luna-source-identity.json'
    identity={'profile_id':profile.get('activeProfileId'),'signature':read_signature_metadata(Path(x['application'])),'source_seal':unique_json(seal),'seal_sha256':digest(seal),'executable_sha256':digest(Path(x['application'])/'Contents/MacOS/luna-reader'),'source_tree_sha256':tree_hash(root)}
    (directory/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')
    (directory/'barrier-action.json').write_text(json.dumps(response,indent=2)+'\n')
    files={p.relative_to(directory).as_posix():digest(p) for p in directory.rglob('*') if p.is_file()}
    return {'step_id':step,'directory':directory.name,'collected_at_ms':time.time_ns()//1000000,'files':files}

def validate_response(value, nonce, step):
    if set(value)!={'execution_nonce','step_id','game_host_pid','windows'} or value['execution_nonce']!=nonce or value['step_id']!=step: raise ValueError('barrier accepts only current action identity, not pass/status or free-form claims')
    if type(value['game_host_pid']) is not int or value['game_host_pid']<=0: raise ValueError('actual native game PID required')
    windows=value['windows']
    if not isinstance(windows,dict) or not set(windows)<=ROLES or any(type(v) is not int or v<=0 for v in windows.values()): raise ValueError('actual role/window IDs required')
    return value

def execute(root, inputs, output, nonce):
    from runtime_stability_evidence import validate_run
    x=load_inputs(inputs,root)
    if not re.fullmatch('[0-9a-f]{32}',nonce): raise ValueError('gate-generated nonce required')
    output.mkdir(parents=True,exist_ok=False,mode=0o700)
    frozen=output/'inputs.json';shutil.copyfile(inputs,frozen)
    started=time.time_ns()//1000000;source=tree_hash(root)
    artifact=safe_path(root,x['artifact']);inspection=inspect_package(root,artifact,source,output/'inspection')
    (output/'package-inspection.json').write_text(json.dumps(inspection,indent=2)+'\n')
    # Installed bits must be exactly those OS-verified in the inspected package.
    exe=Path(x['application'])/'Contents/MacOS/luna-reader';seal=exe.parent.parent/'Resources/luna-source-identity.json'
    if digest(exe)!=inspection['executable_sha256'] or digest(seal)!=inspection['sealed_source_identity_sha256']: raise ValueError('installed app differs from exact inspected RC')
    if inspection.get('signing_mode')!='certificate': raise ValueError('stable certificate-signed RC required')
    # Refuse adopting an old process; launcher owns one exact cold run.
    ps=subprocess.run(['/bin/ps','-axo','comm='],capture_output=True,text=True,check=True).stdout
    if str(exe) in ps.splitlines(): raise ValueError('existing Luna must normally exit before cold RC run; no forced termination')
    swift=output/'runtime-observer'
    compile_cmd=['/usr/bin/xcrun','swiftc','-O',str(root/'scripts/quality/runtime_observer.swift'),'-o',str(swift)]
    compile_result=run_raw(compile_cmd,output/'observer-build.log',timeout=180)
    if compile_result['exit_code']: raise ValueError('independent OS observer did not compile')
    env={**os.environ,'LUNA_PRODUCT_EXECUTION_NONCE':nonce}
    log=(output/'application-launch.log').open('w')
    process=subprocess.Popen([str(exe)],cwd=str(exe.parent),env=env,stdout=log,stderr=subprocess.STDOUT)
    manifest={'schema_version':1,'kind':'real_packaged_stability_collection','execution_nonce':nonce,'started_at_ms':started,'source_tree_sha256':source,'main_pid':process.pid,'inputs_sha256':digest(frozen),'artifact_sha256':digest(artifact),'observer_source_sha256':digest(root/'scripts/quality/runtime_observer.swift'),'observer_executable_sha256':digest(swift),'steps':[],'observation_intervals':[],'completed':False}
    manifest_path=output/'collection.json';manifest_path.write_text(json.dumps(manifest,indent=2)+'\n')
    stop=threading.Event();archive_errors=[];archive_lock=threading.Lock();live=output/'trace-live'
    def observe_ledger():
        while not stop.wait(0.5):
            try:
                with archive_lock:archive_trace(Path(x['log_directory']),live,nonce)
            except (OSError,ValueError) as e: archive_errors.append(str(e)); return
    watcher=threading.Thread(target=observe_ledger,daemon=True);watcher.start()
    deadline=time.monotonic()+BUDGET_SECONDS
    try:
        for i,(step,action) in enumerate(plan_steps()):
            request={'execution_nonce':nonce,'step_id':step,'action':action,'response_file':'continue.json','required_fields':['execution_nonce','step_id','game_host_pid','windows'],'deadline_seconds':min(STEP_WAIT_SECONDS,max(0,int(deadline-time.monotonic()))),'result_is_not_user_supplied':True}
            (output/'pending-action.json').write_text(json.dumps(request,ensure_ascii=False,indent=2)+'\n')
            print('LUNA_STABILITY_ACTION '+json.dumps(request,ensure_ascii=False),flush=True)
            response_path=output/'continue.json';step_end=min(deadline,time.monotonic()+STEP_WAIT_SECONDS)
            while not response_path.exists():
                if process.poll() is not None: raise ValueError('owned Luna exited during stability run')
                if time.monotonic()>=step_end: raise ValueError('action barrier timed out without runtime proof')
                time.sleep(1)
            response=validate_response(unique_json(response_path),nonce,step)
            response_path.rename(output/f'action-{i:03}.json')
            record=collect_step(root,output/f'step-{i:03}',x,nonce,started,process.pid,swift,step,response)
            manifest['steps'].append(record)
            interval=wait_trace_interval(Path(x['log_directory']),live,nonce,process.pid,started,record['collected_at_ms'],step,deadline,archive_lock)
            manifest['observation_intervals'].append(interval);manifest_path.write_text(json.dumps(manifest,indent=2)+'\n')
        manifest['completed']=True
    except (OSError,ValueError,sqlite3.Error,subprocess.SubprocessError) as e:
        manifest['collection_error']=str(e)
    finally:
        stop.set();watcher.join(timeout=2)
        if watcher.is_alive(): archive_errors.append('archive watcher did not stop within observation budget')
        manifest['observation_completed_at_ms']=time.time_ns()//1000000
        try:
            final=wait_trace_interval(Path(x['log_directory']),live,nonce,process.pid,started,manifest['observation_completed_at_ms'],'final',deadline,archive_lock)
            manifest['observation_intervals'].append(final)
            freeze_trace(live,output/'trace',final['closing_health'])
        except (OSError,ValueError) as e:
            manifest['closure_error']=str(e);manifest['completed']=False
        manifest['live_trace_files']={p.relative_to(output).as_posix():digest(p) for p in live.glob('*.jsonl')}
        manifest['trace_files']={p.relative_to(output).as_posix():digest(p) for p in (output/'trace').glob('*.jsonl')}
        manifest['archive_errors']=archive_errors
        manifest['completed_at_ms']=time.time_ns()//1000000;manifest['source_tree_sha256_after']=tree_hash(root)
        manifest_path.write_text(json.dumps(manifest,indent=2)+'\n');log.close()
    result=validate_run(root,output,nonce,source)
    (output/'verification.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(result,ensure_ascii=False),flush=True)
    return 0 if result['status']=='passed' else 1

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--inputs',required=True,type=Path);p.add_argument('--output',required=True,type=Path);p.add_argument('--execution-nonce',required=True);a=p.parse_args()
    root=Path(__file__).resolve().parents[2]
    output=a.output.resolve()
    if not output.is_relative_to(root/'.build'): raise ValueError('runtime evidence must use a fresh repository .build directory')
    return execute(root,a.inputs,output,a.execution_nonce)
if __name__=='__main__':
    try: raise SystemExit(main())
    except (OSError,ValueError,sqlite3.Error,subprocess.SubprocessError) as e: print(str(e),file=sys.stderr);raise SystemExit(2)
