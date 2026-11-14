#!/usr/bin/env python3
"""Public-API WK observer A. It never runs/labels the packaged Native B layer.

The executable is compiled from this checkout, whose root is compiled into it.
Only finite General modes/cases and hash-closed private Vite artifacts are loaded.
No URL, JavaScript, source root, pixel mask, crop, scale or retry is an argument.
"""
from __future__ import annotations
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/quality'))
HOST=ROOT/'scripts/report/general_visual_webkit_host.swift'


def sha(path: Path) -> str:return hashlib.sha256(path.read_bytes()).hexdigest()
def now():return dt.datetime.now(dt.timezone.utc).isoformat()
def write(path: Path,value):
    with path.open('x') as f:json.dump(value,f,ensure_ascii=False,indent=2);f.write('\n')
    path.chmod(0o600)
def safe_output(value: str) -> Path:
    p=Path(value).absolute()
    if p!=p.resolve() or not p.is_relative_to(ROOT/'.build') or p==ROOT/'.build':raise ValueError('Private .build output in this checkout required')
    if p.exists():raise ValueError('Fresh observer output required')
    p.mkdir(parents=True,mode=0o700);p.chmod(0o700)
    return p

def command(args: list[str], output: Path,timeout=60):
    start=now()
    try:
        child=subprocess.run(args,cwd=ROOT,capture_output=True,timeout=timeout)
        out,err,code=child.stdout,child.stderr,child.returncode
    except subprocess.TimeoutExpired as e:out,err,code=e.stdout or b'',e.stderr or b'',-1
    output.with_suffix('.stdout.log').write_bytes(out);output.with_suffix('.stderr.log').write_bytes(err)
    record={'command':args,'started_at':start,'finished_at':now(),'exit_code':code,'stdout_sha256':hashlib.sha256(out).hexdigest(),'stderr_sha256':hashlib.sha256(err).hexdigest()}
    write(output.with_suffix('.json'),record)
    return record

def protected():
    # Observation only; this driver never sends a signal or event to these apps.
    rows=subprocess.check_output(['ps','-axo','pid=,command='],text=True).splitlines()
    return [r.strip() for r in rows if r.strip().endswith('/Applications/Luna.app/Contents/MacOS/luna-reader')]


def exact_rgba(left: Path,right: Path) -> dict:
    from PIL import Image
    with Image.open(left) as a,Image.open(right) as b:
        if a.size!=b.size:raise ValueError('Unscaled full-canvas dimensions differ')
        aa,bb=a.convert('RGBA').tobytes(),b.convert('RGBA').tobytes()
        different=sum(aa[i:i+4]!=bb[i:i+4] for i in range(0,len(aa),4))
        return {'different_rgba_pixels':different,'width':a.width,'height':a.height,'threshold':0,'mask':False,'crop':False,'resampled':False,
                'left_sha256':sha(left),'right_sha256':sha(right)}


def inspect_raw(folder: Path,request: dict) -> dict:
    raw=json.loads((folder/'native-result.json').read_text())
    if raw['status']!='captured' or raw['execution_nonce']!=request['execution_nonce'] or raw['mode']!=request['mode'] or raw['packaged_native'] is not False or raw['product_accepted'] is not False:
        raise ValueError('WK observer capture identity/capability differs')
    if raw['activation_requested'] is not False:raise ValueError('Unexpected foreground activation')
    probe=raw.get('post_capture_security_probe',{})
    actual_probe=probe.get('local_virtual_fetch_csp',{})
    if probe.get('status')!='passed' or probe.get('resource_handler_received_probe') is not False or probe.get('external_network_attempted') is not False or probe.get('system_permission_probe_attempted') is not False or actual_probe.get('localFetchResolved') is not False or actual_probe.get('popupReturned') is not False or not any(v.get('directive','').startswith('connect-src') for v in actual_probe.get('cspViolations',[])):
        raise ValueError('Actual post-capture CSP/popup refusal proof absent')
    captures=raw['captures']
    if len(captures)!=4:raise ValueError('Two fixed frames from two fresh documents required')
    names=[f'document-{d}-frame-{f}' for d in (1,2) for f in (1,2)]
    if [r['name'] for r in captures]!=names:raise ValueError('Capture ordering changed')
    pids=set()
    for frame in captures:
        path=folder/(frame['name']+'.png')
        if sha(path)!=frame['png_sha256'] or path.stat().st_size!=frame['png_bytes']:raise ValueError('Raw PNG bytes differ')
        if frame['nonpersistent_data_store'] is not True:raise ValueError('Persistent observer store forbidden')
        native=frame['native'];pids.add(native['pid'])
        if native['pid']!=raw['pid'] or not isinstance(native['window_id'],int) or native['window_id']<=0:raise ValueError('Actual process/window identity absent')
        if native['content_bounds']!=[0,0,request['width'],request['height']]:raise ValueError('Whole content bounds differ')
        scale=native['backing_scale']
        if frame['pixel_width']!=request['width']*scale or frame['pixel_height']!=request['height']*scale:raise ValueError('Raw backing pixels changed')
        facts=frame['facts']
        if facts['fonts']!='loaded' or facts['animations']!=0 or facts['diagnostics']['errors'] or facts['diagnostics']['csp']:raise ValueError('Document unsettled or contains errors')
        if facts['viewport']!={'width':request['width'],'height':request['height'],'dpr':scale}:raise ValueError('Actual viewport/backing scale differs')
        if request['mode'].startswith('source-'):
            state=facts['state'];game=request['mode']=='source-saved-game'
            if state.get('scope')!=('game' if game else 'global') or state.get('dirty') is not False or state.get('draft')!={'segmentation':True,'ruby':not game,'pos':False} or state.get('draftOverrides')!=({'ruby':False} if game else {}):
                raise ValueError('Actual source saved-game/default state differs')
            saved=facts.get('saved',{})
            if saved.get('global')!={'segmentation':True,'ruby':True,'pos':False} or saved.get('game',{}).get('overrides')!=({'ruby':False} if game else {}) or facts.get('effectiveGame')!={'segmentation':True,'ruby':not game,'pos':False}:
                raise ValueError('Source-only saved override/readback differs; not database proof')
        if not frame['resources'] or any(row.get('decision')!='served' for row in frame['resources']):raise ValueError('Resource allowlist failed')
    if len(pids)!=1:raise ValueError('Capture process changed')
    comparisons=[]
    for left,right in ((0,1),(2,3),(0,2),(1,3)):
        diff=exact_rgba(folder/(names[left]+'.png'),folder/(names[right]+'.png'))
        comparisons.append({'left':names[left],'right':names[right],**diff})
    return {'kind':'general-wk-observer-independent-recompute/v1','status':'passed' if all(r['different_rgba_pixels']==0 for r in comparisons) else 'failed',
            'native_result_sha256':sha(folder/'native-result.json'),'comparisons':comparisons,'packaged_native':False,'product_accepted':False,
            'game_tests_executed':False,'os_runtime_verified':False,'storage_runtime_verified':False}


def source_files():
    from current_general_settings_authority import approval_inputs
    paths=approval_inputs(ROOT)
    return {relative:{'sha256':sha(ROOT/relative),'bytes':(ROOT/relative).stat().st_size} for relative in sorted(paths)}


def execute(output: Path,canvas_id: str,mode: str,state_id='all-page-settings-general-normal',subcase='primary',bundle='') -> dict:
    from current_general_visual_authority import canvas,validate_case
    c=canvas(canvas_id);validate_case(state_id,canvas_id,subcase)
    source_before=source_files() if mode.startswith('source-') else None
    bundle_manifest=''
    if not mode.startswith('source-'):
        relative=Path(bundle)
        if relative.is_absolute() or not bundle.startswith('.build/') or '..' in relative.parts:raise ValueError('Managed finite bundle must be in .build')
        directory=ROOT/relative
        if directory!=directory.resolve() or not directory.is_dir():raise ValueError('Bundle path/symlink invalid')
        bundle_manifest=sha(directory/'general-assets.json')
    output.mkdir(mode=0o700) if not output.exists() else None
    output.chmod(0o700)
    nonce=secrets.token_hex(16);folder=output/nonce;folder.mkdir(mode=0o700)
    request={'schema_version':1,'kind':'general-webkit-observer-request/v1','execution_nonce':nonce,**{k:c[k] for k in ('width','height','theme','zoom')},
             'mode':mode,'state_id':state_id,'subcase':subcase,'bundle':bundle,'bundle_manifest_sha256':bundle_manifest}
    (folder/'request.json').write_text(json.dumps(request,ensure_ascii=False,sort_keys=True,separators=(',',':'))+'\n');(folder/'request.json').chmod(0o600)
    producer_paths=('scripts/report/general_visual_webkit_host.swift','scripts/report/general_visual_webkit.py',
                    'scripts/quality/current_general_visual_authority.py','scripts/quality/current_general_settings_authority.py',
                    'scripts/quality/gate_core.py','scripts/quality/general_capture_provider.py',
                    'apps/desktop/tests/visual/general-source-capture-provider.mjs')
    producer_before={p:sha(ROOT/p) for p in producer_paths}
    protected_before=protected();host_before=sha(HOST);driver_before=sha(Path(__file__))
    binary=output/'general-webkit-observer'
    build=command(['xcrun','swiftc','-swift-version','5','-framework','AppKit','-framework','WebKit','-framework','CryptoKit',str(HOST),'-o',str(binary)],output/'build',90)
    result={'kind':'general-wk-observer-execution/v1','mode':mode,'canvas_id':canvas_id,'state_id':state_id,'subcase':subcase,'execution_nonce':nonce,
            'output':str(folder.relative_to(ROOT)),'build':build,'host_source_sha256':host_before,'driver_sha256':driver_before,
            'source_inputs_before':source_before,'producer_inputs_before':producer_before,'started_at':now(),'controlled_visual_only':True,'packaged_native':False,'product_accepted':False,
            'game_tests_executed':False,'os_runtime_verified':False,'storage_runtime_verified':False}
    if build['exit_code']!=0:result['status']='failed_build'
    else:
        result['executable_sha256']=sha(binary)
        execution=command([str(binary),'--request',str(folder/'request.json')],folder/'execution',40);result['execution']=execution
        if execution['exit_code']!=0:result['status']='failed_capture'
        else:
            recomputed=inspect_raw(folder,request);write(folder/'comparison.json',recomputed);result['recomputed']=recomputed;result['status']=recomputed['status']
    result['source_inputs_after']=source_files() if source_before else None
    result['source_stable']=result['source_inputs_before']==result['source_inputs_after']
    result['producer_inputs_after']={p:sha(ROOT/p) for p in producer_paths}
    result['producer_stable']=result['producer_inputs_after']==producer_before
    result['protected_apps_unchanged']=protected_before==protected()
    if not all(result[k] for k in ('source_stable','producer_stable','protected_apps_unchanged')):result['status']='failed_epoch_or_environment_drift'
    result['finished_at']=now();write(output/'result.json',result)
    return result


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--qualification',choices=('normal','saved-game'));parser.add_argument('--side',choices=('reference','candidate'))
    parser.add_argument('--state',default='all-page-settings-general-normal');parser.add_argument('--subcase',default='primary');parser.add_argument('--canvas',required=True)
    parser.add_argument('--bundle',default='');parser.add_argument('--output',required=True);args=parser.parse_args()
    if bool(args.qualification)==bool(args.side):parser.error('Choose source qualification or finite General side')
    output=safe_output(args.output)
    mode=('source-saved-game' if args.qualification=='saved-game' else 'source-qualification') if args.qualification else 'general-'+args.side
    result=execute(output,args.canvas,mode,args.state,args.subcase,args.bundle)
    print(json.dumps({'status':result['status'],'result':str((output/'result.json').relative_to(ROOT)),'output':result['output']}))
    return 0 if result['status']=='passed' else 1

if __name__=='__main__':
    try:sys.exit(main())
    except (ValueError,KeyError,FileNotFoundError) as e:print(json.dumps({'status':'rejected','reason':str(e)}),file=sys.stderr);sys.exit(2)
