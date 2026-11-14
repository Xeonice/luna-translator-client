#!/usr/bin/env python3
"""Capture one finite General state from an already inspected, same-source package.

This is controlled Native rendering only. It never starts the product composition
root, accesses a profile, sets permissions, or upgrades OS/DB/game acceptance.
"""
from __future__ import annotations
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import plistlib
import secrets
import shutil
import struct
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/quality'))
import current_general_native_display as display_contract
MODE_CONTRACT = b'luna-general-native-isolated-renderer/v1:20261108'
STATES = tuple('all-page-settings-general-' + name for name in ('normal','empty','loading','error','disabled','long-text','light','narrow','zoom-200','keyboard-focus-escape','scope-global','scope-game','restore-inheritance','save-failed'))
CANVASES = tuple(f'{w}x{h}-{theme}-z{zoom}' for w,h in ((1000,720),(760,560)) for theme in ('dark','light') for zoom in (1,2))
SUBCASES = ('primary','initial','menu-open','content-end')

def now(): return dt.datetime.now(dt.timezone.utc).isoformat()
def sha(path):
    with Path(path).open('rb') as stream: return hashlib.file_digest(stream,'sha256').hexdigest()
def strict_pairs(pairs):
    result={}
    for key,value in pairs:
        if key in result: raise ValueError('duplicate JSON key: '+key)
        result[key]=value
    return result

def load(path):
    return json.loads(Path(path).read_text(),object_pairs_hook=strict_pairs,parse_constant=lambda token: (_ for _ in ()).throw(ValueError('non-finite JSON: '+token)))

def dump(path,value):
    path=Path(path)
    with path.open('x') as stream: stream.write(json.dumps(value,ensure_ascii=False,indent=2)+'\n')

def finite(args):
    if args.state not in STATES or args.canvas not in CANVASES or args.side not in ('reference','candidate') or args.subcase not in SUBCASES:
        raise ValueError('finite General state/side/canvas/subcase required')
    if args.subcase=='initial' and args.state!=STATES[2]: raise ValueError('initial belongs only to loading')
    if args.subcase=='menu-open' and args.state!=STATES[9]: raise ValueError('menu-open belongs only to keyboard state')
    if args.subcase=='content-end' and (args.state!=STATES[0] or not args.canvas.endswith('-z2')): raise ValueError('content-end belongs only to normal at zoom2')
    if len(args.expected_source)!=64 or any(c not in '0123456789abcdef' for c in args.expected_source): raise ValueError('exact source identity required')

def no_symlink(path):
    path=Path(path).absolute()
    for candidate in [path,*path.parents]:
        if candidate.is_symlink(): raise ValueError('symlink evidence or package path forbidden')
    return path

def private_visual_directory(nonce):
    # macOS's trusted system temp root can use /var -> /private/var. Resolve
    # that existing root once; all fresh descendants still forbid symlinks.
    temp_root=Path(tempfile.gettempdir()).resolve(strict=True)
    return no_symlink(temp_root/'luna-general-visual'/nonce)

def protected_processes():
    result={}
    for row in subprocess.check_output(['ps','-axo','pid=,command='],text=True).splitlines():
        parts=row.strip().split(None,1)
        if len(parts)!=2: continue
        command=parts[1]
        if command.endswith('/Applications/Luna.app/Contents/MacOS/luna-reader') or ('/vite/bin/vite.js' in command and '/tests/live-preview/vite.config.ts' in command): result[parts[0]]=command
    return result

def front():
    try:
        result=subprocess.run(['/usr/bin/lsappinfo','front'],text=True,capture_output=True,timeout=5)
        return {'exit_code':result.returncode,'stdout':result.stdout.strip(),'stderr':result.stderr.strip()}
    except (OSError,subprocess.SubprocessError) as error:
        return {'status':'unavailable','error':str(error)}

def package(args):
    app=no_symlink(args.app).resolve(strict=True)
    if not app.is_dir() or app.suffix!='.app': raise ValueError('exact packaged application required')
    binary=no_symlink(app/'Contents/MacOS/luna-reader')
    binary_bytes=binary.read_bytes()
    # Old clients ignore an unknown flag and would enter their personal runtime.
    # Reject them before execution, even if a caller supplied their old source ID.
    if MODE_CONTRACT not in binary_bytes: raise ValueError('package has no isolated General visual mode; it will not be launched')
    identity_path=no_symlink(app/'Contents/Resources/luna-source-identity.json'); identity=load(identity_path)
    if identity.get('kind')!='luna_sealed_source_identity' or identity.get('source_tree_sha256')!=args.expected_source: raise ValueError('sealed package source differs')
    info=plistlib.loads((app/'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier')!='com.lunatranslator.reader' or info.get('CFBundleExecutable')!='luna-reader': raise ValueError('unrelated bundle identity')
    outer_path=no_symlink(app.parent/'build-manifest.json'); outer=load(outer_path)
    if outer.get('source_tree_sha256')!=args.expected_source or outer.get('executable_sha256')!=hashlib.sha256(binary_bytes).hexdigest(): raise ValueError('external package/executable identity differs')
    artifact=no_symlink(outer.get('archive','')).resolve(strict=True)
    if not artifact.is_relative_to(ROOT/'.build') or sha(artifact)!=outer.get('archive_sha256'): raise ValueError('actual archive identity differs')
    subprocess.run(['/usr/bin/codesign','--verify','--deep','--strict',str(app)],capture_output=True,text=True,check=True,timeout=30)
    sys.path.insert(0,str(ROOT/'scripts/quality'))
    from package_identity import read_signature_metadata
    signature=read_signature_metadata(app)
    if signature.get('mode')!='certificate' or signature.get('identifier')!='com.lunatranslator.reader' or signature.get('leaf_sha1')!=outer.get('expected_leaf_sha1') or signature!=outer.get('signature_metadata'): raise ValueError('package signature identity differs')
    if identity.get('signing_mode')!='certificate' or identity.get('expected_leaf_sha1')!=signature['leaf_sha1']: raise ValueError('sealed signing contract differs')
    return app,{'app':str(app),'binary':str(binary),'executable_sha256':hashlib.sha256(binary_bytes).hexdigest(),'archive':str(artifact),'archive_sha256':outer['archive_sha256'],'source_tree_sha256':args.expected_source,'sealed_identity_sha256':sha(identity_path),'external_manifest_sha256':sha(outer_path),'signature':signature,'mode_contract':MODE_CONTRACT.decode()}

def compile_observer(output):
    source=ROOT/'scripts/quality/runtime_observer.swift'; target=output/'runtime-observer'
    command=['/usr/bin/swiftc',str(source),'-o',str(target)]
    result=subprocess.run(command,text=True,capture_output=True,timeout=90)
    dump(output/'observer-build.json',{'command':command,'source':str(source),'source_sha256':sha(source),'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
    if result.returncode: raise ValueError('independent OS observer did not compile')
    return target

def observe(observer,pid,path):
    ids=','.join(str(value) for value in pid) if isinstance(pid,(list,tuple)) else str(pid)
    command=[str(observer),'--pids',ids,'--output',str(path)]
    started=now();result=subprocess.run(command,text=True,capture_output=True,timeout=15)
    dump(path.with_suffix('.command.json'),{'command':command,'started_at':started,'finished_at':now(),'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
    if result.returncode: raise ValueError('independent OS observation failed')
    return load(path)

def compile_display_probe(output):
    source=ROOT/display_contract.PROBE_SOURCE;binary=output/'native-display-probe'
    command=['/usr/bin/xcrun','swiftc','-swift-version','5','-framework','CoreGraphics','-framework','ColorSync','-framework','CryptoKit',str(source),'-o',str(binary)]
    result=subprocess.run(command,text=True,capture_output=True,timeout=90)
    record={'kind':'general-native-display-probe-build/v1','command':command,'source_sha256':sha(source),'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
    if result.returncode==0:record['binary_sha256']=sha(binary)
    dump(output/'display-probe-build.json',record)
    if result.returncode:raise ValueError('Native display inventory probe did not compile')
    return binary

def probe_display(binary,nonce,phase,output):
    raw_path=output/('display-'+phase+'.json');command_path=output/('display-'+phase+'.command.json')
    command=[str(binary),'--nonce',nonce,'--phase',phase];started=now()
    child=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:stdout,stderr=child.communicate(timeout=15)
    except subprocess.TimeoutExpired:
        child.terminate();stdout,stderr=child.communicate(timeout=5)
        dump(command_path,{'command':command,'pid':child.pid,'parent_pid':os.getpid(),'started_at':started,'finished_at':now(),'exit_code':child.returncode,'timed_out':True,'stderr':stderr[:2048].decode(errors='replace')})
        raise ValueError('Native read-only display probe timed out')
    with raw_path.open('xb') as stream:stream.write(stdout)
    record={'command':command,'pid':child.pid,'parent_pid':os.getpid(),'started_at':started,'finished_at':now(),'exit_code':child.returncode,'stdout_sha256':sha(raw_path),'binary_sha256':sha(binary),'stderr':stderr[:2048].decode(errors='replace')}
    dump(command_path,record)
    if child.returncode:raise ValueError('Native display environment unavailable: '+record['stderr'])
    raw=display_contract.validate_probe(display_contract.load(raw_path),nonce,phase)
    if raw['pid']!=child.pid or raw['ppid']!=os.getpid():raise ValueError('Native display inventory process identity differs')
    return raw,{'raw':display_contract.file_ref(ROOT,raw_path),'execution':display_contract.file_ref(ROOT,command_path)}

def png_shape(path):
    head=Path(path).read_bytes()[:33]
    if len(head)<33 or head[:8]!=b'\x89PNG\r\n\x1a\n' or head[12:16]!=b'IHDR': raise ValueError('original OS PNG required')
    return struct.unpack('>II',head[16:24])

def process_identity(observation,pid,attestation,expected_controller_pid=None):
    controller_pid=os.getpid() if expected_controller_pid is None else expected_controller_pid
    if type(controller_pid) is not int or controller_pid<=0: raise ValueError('actual original controller PID required')
    rows=[row for row in observation.get('processes',[]) if row.get('pid')==pid]
    if len(rows)!=1: raise ValueError('exactly one independently observed renderer process required')
    row=rows[0];binary=Path(attestation['binary']);metadata=binary.stat();guest=row.get('guest_code',{});signature=attestation['signature']
    if row.get('executable')!=str(binary) or row.get('device')!=metadata.st_dev or row.get('inode')!=metadata.st_ino or row.get('ppid')!=controller_pid: raise ValueError('actual renderer executable/parent identity differs')
    if not isinstance(row.get('start_sec'),int) or row['start_sec']<=0 or not isinstance(row.get('start_usec'),int): raise ValueError('actual renderer start identity unavailable')
    if any(guest.get(key)!=0 for key in ('guest_status','validity','information_status')) or guest.get('identifier')!=signature['identifier'] or guest.get('team')!=signature['team_identifier'] or guest.get('cdhash')!=signature['cdhash']: raise ValueError('actual renderer process code signature differs')
    return {key:row[key] for key in ('pid','ppid','executable','device','inode','start_sec','start_usec','guest_code')}

def unchanged_package(attestation):
    app=Path(attestation['app'])
    for file,expected in ((Path(attestation['binary']),attestation['executable_sha256']),(app/'Contents/Resources/luna-source-identity.json',attestation['sealed_identity_sha256']),(app.parent/'build-manifest.json',attestation['external_manifest_sha256'])):
        if sha(no_symlink(file))!=expected: raise ValueError('package identity changed during Native capture')
    sys.path.insert(0,str(ROOT/'scripts/quality'))
    from package_identity import read_signature_metadata
    if read_signature_metadata(app)!=attestation['signature']: raise ValueError('sealed package signature changed during Native capture')

def finish(directory,nonce):
    if not directory.is_dir() or directory.is_symlink(): return False
    target=directory/'finish'
    flags=os.O_WRONLY|os.O_CREAT|os.O_EXCL|getattr(os,'O_NOFOLLOW',0)
    descriptor=os.open(target,flags,0o600)
    try: os.write(descriptor,nonce.encode())
    finally: os.close(descriptor)
    return True

def settle_before_frames():
    # One unconditional observation schedule for both sides and every state.
    started_at=now();started=time.monotonic();time.sleep(3)
    finished=time.monotonic();elapsed=finished-started
    return {'kind':'fixed-native-pre-capture-settle/v1','planned_seconds':3,'started_at':started_at,'finished_at':now(),'started_monotonic':started,'finished_monotonic':finished,'elapsed_monotonic_seconds':elapsed}

def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=Path,required=True);parser.add_argument('--expected-source',required=True)
    parser.add_argument('--side',required=True);parser.add_argument('--state',required=True);parser.add_argument('--canvas',required=True)
    parser.add_argument('--subcase',default='primary');parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(argv)
    # Argument/path validation precedes any child launch or observer construction.
    finite(args);output=no_symlink(args.output).resolve()
    if not output.is_relative_to(ROOT/'.build') or output.exists(): raise ValueError('fresh repository .build output required')
    output.mkdir(parents=True,mode=0o700)
    result={'schema_version':1,'kind':'general-packaged-native-render-capture/v1','started_at':now(),'status':'failed','controlled_transport':True,'native_visual_rendering_only':True,'os_runtime_verified':False,'storage_runtime_verified':False,'game_tests_executed':False,'source_tree_sha256':args.expected_source,'state_id':args.state,'canvas_id':args.canvas,'subcase':args.subcase,'side':args.side,'errors':[],'frames':[],'controller_pid':os.getpid(),'controller_executable':str(Path(sys.executable).resolve()),'controller_parent_pid':os.getppid()}
    child=None;private=None;nonce=None
    before=protected_processes();result['protected_processes_before']=before;result['front_before']=front()
    try:
        app,attestation=package(args);result['package']=attestation
        observer=compile_observer(output)
        display_probe=compile_display_probe(output)
        nonce=secrets.token_hex(16);private=private_visual_directory(nonce)
        if private.exists() or private.is_symlink(): raise ValueError('new visual nonce unexpectedly exists')
        from current_general_visual_authority import reference_inputs,capture_cases,case_key,validate_case
        producer=reference_inputs(ROOT,'packaged_native');row=validate_case(args.state,args.canvas,args.subcase)
        prelaunch,prelaunch_ref=probe_display(display_probe,nonce,'pre_launch',output)
        environment=display_contract.primary_environment(prelaunch);approval=None
        if args.side=='candidate':
            approved,approval=display_contract.approved_environment(ROOT)
            display_contract.require_same_environment(approved,environment)
        parent_observation=observe(observer,(os.getpid(),os.getppid()),output/'batch-parent.json')
        coordinator=display_contract.coordinator_cohort(parent_observation,os.getppid())
        lock=display_contract.batch_lock(ROOT,output,row,args.side,args.expected_source,producer,environment,coordinator,approval)
        index=list(capture_cases()).index(row)
        result['display_environment']={'kind':'general-native-capture-display-binding/v1','environment':environment,'environment_sha256':display_contract.identity(environment),'approval':approval,'batch_lock':lock,'coordinator':coordinator,'coordinator_observation':display_contract.file_ref(ROOT,output/'batch-parent.json'),'coordinator_execution':display_contract.file_ref(ROOT,output/'batch-parent.command.json'),'coordinator_observer_build':display_contract.file_ref(ROOT,output/'observer-build.json'),'coordinator_observer_binary_sha256':sha(observer),'probe_build':display_contract.file_ref(ROOT,output/'display-probe-build.json'),'probes':{'pre_launch':prelaunch_ref},'producer_inputs_before':producer,'batch_index':index,'completed_case_keys':[case_key(value) for value in capture_cases()[:index]]}
        command=[str(app/'Contents/MacOS/luna-reader'),'--general-visual-verification',args.side,args.state,args.canvas,args.subcase,nonce]
        result.update(execution_nonce=nonce,command=command,private_directory=str(private))
        with (output/'process.stdout.txt').open('x') as stdout,(output/'process.stderr.txt').open('x') as stderr:
            result['display_environment']['renderer_launch_started_at']=now()
            child=subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr,close_fds=True)
            result['pid']=child.pid
            deadline=time.monotonic()+40
            while not (private/'observation.json').is_file():
                if child.poll() is not None: raise ValueError('isolated renderer exited before observation')
                if time.monotonic()>deadline: raise ValueError('isolated renderer observation timeout')
                time.sleep(.05)
            metadata=private.stat()
            if private.is_symlink() or metadata.st_uid!=os.getuid() or metadata.st_mode & 0o077: raise ValueError('private visual directory ownership differs')
            started=load(private/'started.json');observed=load(private/'observation.json')
            if started.get('mode_contract')!=MODE_CONTRACT.decode() or started.get('pid')!=child.pid or started.get('business_runtime_constructed') is not False: raise ValueError('isolated composition start differs')
            ctx=started.get('context',{})
            expected={'side':args.side,'stateId':args.state,'canvasId':args.canvas,'subcase':args.subcase,'nonce':nonce,'transport':'packaged_native'}
            if any(ctx.get(k)!=v for k,v in expected.items()) or observed.get('pid')!=child.pid or observed.get('observation',{}).get('context')!=ctx: raise ValueError('actual Native context identity differs')
            if observed.get('source_identity',{}).get('source_tree_sha256')!=args.expected_source or observed.get('window_label')!='general-visual-verification': raise ValueError('actual Native source/window differs')
            if any(observed.get(k) is not False for k in ('os_runtime_verified','storage_runtime_verified','game_tests_executed')): raise ValueError('rendering receipt cannot grant OS/DB/game verification')
            shutil.copyfile(private/'started.json',output/'started.json');shutil.copyfile(private/'observation.json',output/'observation.json')
            result['observation_sha256']=sha(output/'observation.json')
            if observed['observation'].get('error') is not None: raise ValueError('real renderer reported: '+observed['observation']['error'])
            acl=observed['observation'].get('facts',{}).get('nativeAcl',{})
            if acl.get('coreCommand')!='plugin:window|scale_factor' or acl.get('coreDenied') is not True or acl.get('businessCommand')!='application_snapshot' or acl.get('businessUnknown') is not True:
                raise ValueError('actual core ACL denial/business exclusion was not observed')
            os_before=observe(observer,(child.pid,os.getpid()),output/'os-before.json')
            result['observed_process_identity']=process_identity(os_before,child.pid,attestation,os.getpid())
            if os_before.get('screen_capture_preflight') is not True: raise ValueError('screen capture preflight unavailable; no permission request or screenshot attempted')
            windows=[w for w in os_before['windows'] if w.get('pid')==child.pid and w.get('layer')==0 and w.get('on_screen') is True]
            if len(windows)!=1: raise ValueError('exactly one visible owned Native window required')
            window=windows[0];wid=window['window_id'];bounds=window['bounds'];w=ctx['width'];h=ctx['height'];scale=observed['window']['backing_scale']
            if bounds.get('Width')!=w or bounds.get('Height')!=h: raise ValueError('actual owned Native logical dimensions differ')
            result['window_id']=wid;result['window']=window
            before_display,before_ref=probe_display(display_probe,nonce,'before_capture',output)
            display_contract.require_same_environment(environment,display_contract.primary_environment(before_display))
            display_contract.require_primary_window(environment,bounds,scale)
            result['display_environment']['probes']['before_capture']=before_ref
            result['pre_capture_settle']=settle_before_frames()
            for index in (1,2):
                time.sleep(.15)
                frame=output/f'frame-{index}.png';capture=['/usr/sbin/screencapture','-x','-a','-o','-l',str(wid),str(frame)]
                completed=subprocess.run(capture,text=True,capture_output=True,timeout=8)
                if completed.returncode: raise ValueError('OS original window capture failed: '+completed.stderr)
                width,height=png_shape(frame)
                if width!=int(w*scale) or height!=int(h*scale): raise ValueError('OS PNG backing scale/canvas differs')
                profile=display_contract.png_iccp(frame.read_bytes());display_contract.require_png_profile(environment,profile)
                result['frames'].append({'path':frame.name,'sha256':sha(frame),'bytes':frame.stat().st_size,'pixel_width':width,'pixel_height':height,'captured_at':now(),'command':capture,'display_profile':profile})
            after_display,after_ref=probe_display(display_probe,nonce,'after_capture',output)
            display_contract.require_same_environment(environment,display_contract.primary_environment(after_display))
            result['display_environment']['probes']['after_capture']=after_ref
            os_after=observe(observer,(child.pid,os.getpid()),output/'os-after.json')
            if process_identity(os_after,child.pid,attestation,os.getpid())!=result['observed_process_identity']: raise ValueError('actual renderer process changed during capture')
            after_windows=[w for w in os_after['windows'] if w.get('window_id')==wid and w.get('pid')==child.pid]
            if len(after_windows)!=1 or not after_windows[0].get('on_screen') or after_windows[0]['bounds']!=bounds: raise ValueError('owned Native window changed during capture')
            display_contract.require_primary_window(environment,after_windows[0]['bounds'],scale)
            after_producer=reference_inputs(ROOT,'packaged_native');result['display_environment']['producer_inputs_after']=after_producer
            if after_producer!=producer:raise ValueError('Native display/capture producer changed during original capture')
            if load(private/'observation.json')!=observed: raise ValueError('Native observation changed while capturing')
            unchanged_package(attestation)
            finish(private,nonce);result['finish_requested']=True
            child.wait(timeout=8);result['exit_code']=child.returncode
            if child.returncode!=0: raise ValueError('isolated renderer did not exit cleanly')
            finished=load(private/'finished.json')
            if finished.get('status')!='finished_by_owned_controller': raise ValueError('isolated renderer lifecycle ended unexpectedly')
            shutil.copyfile(private/'finished.json',output/'finished.json')
            marker=no_symlink(private/'finish').read_bytes()
            if marker!=nonce.encode(): raise ValueError('owned finish marker nonce differs')
            (output/'finish-marker.txt').write_bytes(marker);result['finish_marker_sha256']=sha(output/'finish-marker.txt')
            result['os_exited']=observe(observer,(child.pid,os.getpid()),output/'os-exited.json')
            exited=[p for p in result['os_exited']['processes'] if p.get('pid')==child.pid]
            if len(exited)!=1 or exited[0].get('path_bytes',0)>0 or exited[0].get('bsd_bytes',0)>0 or any(w.get('pid')==child.pid for w in result['os_exited']['windows']): raise ValueError('independent OS observer still sees the owned renderer after clean exit')
        result['status']='captured'
    except Exception as error:
        result['errors'].append(type(error).__name__+': '+str(error))
    finally:
        if child is not None and child.poll() is None:
            try:
                if private is not None and nonce is not None and not (private/'finish').exists(): finish(private,nonce)
                child.wait(timeout=8)
            except Exception as error:
                result['errors'].append('owned renderer cleanup: '+str(error))
                # No SIGKILL or unrelated process operation. Its bounded watchdog owns exit.
                try: child.wait(timeout=48)
                except subprocess.TimeoutExpired:
                    result['errors'].append('owned renderer remained after its watchdog')
                    child.terminate();result['owned_sigterm_used']=True
                    try: child.wait(timeout=5)
                    except subprocess.TimeoutExpired: result['errors'].append('owned renderer did not exit after SIGTERM; no force-kill used')
            result['exit_code']=child.poll()
        if private is not None and private.is_dir() and not private.is_symlink():
            for name in ('started.json','observation.json','finished.json'):
                source=private/name;target=output/('cleanup-'+name)
                if source.is_file() and not source.is_symlink() and not target.exists(): shutil.copyfile(source,target)
        result['protected_processes_after']=protected_processes();result['front_after']=front();result['finished_at']=now()
        if result['protected_processes_after']!=before: result['errors'].append('existing product/live-preview process identity changed')
        if result['errors']: result['status']='failed'
        dump(output/'capture.json',result)
    print(json.dumps({'status':result['status'],'output':str(output),'errors':result['errors']},ensure_ascii=False))
    return 0 if result['status']=='captured' else 1

if __name__=='__main__':
    try: raise SystemExit(main())
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        print(json.dumps({'status':'rejected_before_launch','error':str(error)},ensure_ascii=False),file=sys.stderr);raise SystemExit(2)
