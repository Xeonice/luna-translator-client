"""Native-only display/profile evidence and owned QA batch contract.

No GUI, screenshot, network, AX, display mutation or color conversion. The only
writes are an exclusive immutable lock inside a fresh owned .build batch.
"""
from __future__ import annotations
import base64
import binascii
import hashlib
import json
import math
import os
from pathlib import Path
import re
import secrets
import stat
import struct
import zlib

PHASES=('pre_launch','before_capture','after_capture')
MAX_ICC_BYTES=4*1024*1024
MAX_TOTAL_ICC_BYTES=16*1024*1024
SHA=re.compile(r'[0-9a-f]{64}\Z')
UUID=re.compile(r'[0-9A-F]{8}(?:-[0-9A-F]{4}){3}-[0-9A-F]{12}\Z')
PROBE_SOURCE='scripts/quality/general_native_display_probe.swift'
LOCK_NAME='.native-display-environment-lock.json'
CATALOGUE='docs/development/current-general-reference-catalogue.json'
REVIEW='docs/development/current-general-reference-review.json'

def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(value):return json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False).encode()
def identity(value):return sha(canonical(value))

def integer(value,minimum=0,maximum=2**63-1):
    if type(value) is not int or not minimum<=value<=maximum:raise ValueError('Typed bounded display integer required')
    return value

def number(value,minimum=-1e6,maximum=1e6):
    if type(value) not in (int,float) or not math.isfinite(value) or not minimum<=value<=maximum:raise ValueError('Finite bounded original display number required')
    return value

def boolean(value):
    if type(value) is not bool:raise ValueError('Typed original display boolean required')
    return value

def exact(value,keys):
    if not isinstance(value,dict) or set(value)!=set(keys):raise ValueError('Exact Native display evidence fields required')

def icc(data):
    if type(data) is not bytes or not 128<=len(data)<=MAX_ICC_BYTES or struct.unpack('>I',data[:4])[0]!=len(data) or data[36:40]!=b'acsp':
        raise ValueError('Original bounded ICC profile header/length required')
    return {'bytes':len(data),'sha256':sha(data)}

def validate_probe(raw,nonce,phase):
    exact(raw,('schema_version','kind','nonce','phase','pid','ppid','observed_at_us','main_display_id','active_display_count','displays','capabilities','errors'))
    if type(raw['schema_version']) is not int or raw['schema_version']!=1 or raw['kind']!='general-native-display-inventory/v1' or not re.fullmatch('[0-9a-f]{32}',nonce) or raw['nonce']!=nonce or phase not in PHASES or raw['phase']!=phase:
        raise ValueError('Original Native display probe schema/nonce/phase differs')
    if raw['capabilities']!={'read_only':True,'gui_constructed':False,'screen_capture_invoked':False,'system_mutation':False,'permissions_requested':False} or raw['errors']!=[]:
        raise ValueError('Display probe capability differs')
    # JSON bool must not masquerade as numeric API evidence.
    for key,value in raw['capabilities'].items():boolean(value)
    for key in ('pid','ppid','observed_at_us','main_display_id'):integer(raw[key],1)
    count=integer(raw['active_display_count'],1,32)
    if not isinstance(raw['displays'],list) or len(raw['displays'])!=count:raise ValueError('Complete bounded active display inventory required')
    seen=set();uuids=set();total=0;primary=[]
    for row in raw['displays']:
        exact(row,('display_id','uuid','is_main','is_active','is_online','is_builtin','is_asleep','vendor','model','serial','rotation_degrees','bounds','mode','color_space'))
        display=integer(row['display_id'],1,2**32-1)
        if display in seen or not isinstance(row['uuid'],str) or not UUID.fullmatch(row['uuid']) or row['uuid'] in uuids:raise ValueError('Unique actual display ID and UUID required')
        seen.add(display);uuids.add(row['uuid'])
        for key in ('is_main','is_active','is_online','is_builtin','is_asleep'):boolean(row[key])
        if row['is_main']!=(display==raw['main_display_id']) or not row['is_active'] or not row['is_online']:raise ValueError('Display primary/active/online API facts inconsistent')
        for key in ('vendor','model','serial'):integer(row[key],0,2**32-1)
        rotation=number(row['rotation_degrees'],0,360)
        if rotation not in (0,90,180,270):raise ValueError('Unverified display rotation; fixed Native context unavailable')
        exact(row['bounds'],('x','y','width','height'))
        for key in ('x','y'):number(row['bounds'][key])
        for key in ('width','height'):number(row['bounds'][key],1,32768)
        mode=row['mode'];exact(mode,('width','height','pixel_width','pixel_height','refresh_rate','io_flags'))
        for key in ('width','height','pixel_width','pixel_height'):integer(mode[key],1,65536)
        integer(mode['io_flags'],0,2**32-1);number(mode['refresh_rate'],0,1000)
        color=row['color_space'];exact(color,('name','model','components','icc'))
        if not isinstance(color['name'],str) or len(color['name'])>256:raise ValueError('Bounded actual color-space name required')
        integer(color['model'],-1,16);integer(color['components'],1,16)
        profile=color['icc'];exact(profile,('bytes','sha256','base64'));integer(profile['bytes'],128,MAX_ICC_BYTES)
        if not isinstance(profile['sha256'],str) or not SHA.fullmatch(profile['sha256']) or not isinstance(profile['base64'],str) or len(profile['base64'])!=4*((profile['bytes']+2)//3):raise ValueError('Original bounded ICC bytes/hash required')
        try:data=base64.b64decode(profile['base64'],validate=True)
        except (ValueError,binascii.Error) as error:raise ValueError('Original ICC base64 invalid') from error
        if base64.b64encode(data).decode()!=profile['base64'] or icc(data)!={k:profile[k] for k in ('bytes','sha256')}:raise ValueError('Original ICC bytes do not recompute')
        total+=len(data)
        if total>MAX_TOTAL_ICC_BYTES:raise ValueError('Display inventory ICC evidence exceeds aggregate bound')
        if row['is_main']:primary.append(row)
    if len(primary)!=1 or primary[0]['is_asleep']:raise ValueError('Exactly one active awake primary display required')
    return raw

def primary_environment(raw):
    validate_probe(raw,raw['nonce'],raw['phase'])
    row=next(value for value in raw['displays'] if value['is_main'])
    result={key:row[key] for key in ('display_id','uuid','is_builtin','vendor','model','serial','bounds','mode','rotation_degrees')}
    result=json.loads(json.dumps(result))
    result['color_space']={key:row['color_space'][key] for key in ('name','model','components')}
    result['color_space']['icc']={key:row['color_space']['icc'][key] for key in ('bytes','sha256')}
    px,py=row['mode']['pixel_width'],row['mode']['pixel_height']
    if row['rotation_degrees'] in (90,270):px,py=py,px
    result['backing_scale']={'x':px/row['bounds']['width'],'y':py/row['bounds']['height']}
    return {'kind':'general-native-capture-environment/v1','primary':result}

def require_same_environment(expected,actual):
    if canonical(expected)!=canonical(actual):raise ValueError('Native display environment differs; no capture/visual acceptance permitted')

def require_primary_window(environment,window_bounds,backing_scale):
    bounds=environment['primary']['bounds']
    if not isinstance(window_bounds,dict) or set(window_bounds)!=set(('X','Y','Width','Height')):raise ValueError('Original owned CG window bounds required')
    for key in ('X','Y'):number(window_bounds[key])
    for key in ('Width','Height'):number(window_bounds[key],1,4096)
    cx=window_bounds['X']+window_bounds['Width']/2;cy=window_bounds['Y']+window_bounds['Height']/2
    if not bounds['x']<=cx<bounds['x']+bounds['width'] or not bounds['y']<=cy<bounds['y']+bounds['height']:raise ValueError('Owned QA window centroid is outside approved primary display')
    if window_bounds['X']<bounds['x'] or window_bounds['Y']<bounds['y'] or window_bounds['X']+window_bounds['Width']>bounds['x']+bounds['width'] or window_bounds['Y']+window_bounds['Height']>bounds['y']+bounds['height']:
        raise ValueError('Complete owned QA window crosses approved primary display bounds')
    scale=environment['primary']['backing_scale'];number(backing_scale,0.1,16)
    if scale['x']!=scale['y'] or backing_scale!=scale['x']:raise ValueError('Owned QA window backing scale differs from primary display mode')

def png_iccp(data):
    # Extract and hash original bytes only. Never transform or emit an image.
    if type(data) is not bytes or data[:8]!=b'\x89PNG\r\n\x1a\n':raise ValueError('Original PNG signature required')
    at=8;profile=None;ended=False;header=False;image_data=False;palette=False;srgb=False
    while at<len(data):
        if at+12>len(data):raise ValueError('Truncated original PNG chunk')
        length=struct.unpack('>I',data[at:at+4])[0];kind=data[at+4:at+8];body=data[at+8:at+8+length]
        if at+length+12>len(data) or zlib.crc32(kind+body)&0xffffffff!=struct.unpack('>I',data[at+8+length:at+12+length])[0]:raise ValueError('Original PNG length/CRC differs')
        if not header:
            if kind!=b'IHDR' or length!=13:raise ValueError('Original PNG IHDR required')
            header=True
        elif kind==b'IHDR':raise ValueError('Duplicate original PNG IHDR')
        if kind==b'iCCP':
            if profile is not None:raise ValueError('Unique original PNG iCCP required')
            if image_data or palette or srgb:raise ValueError('Original PNG iCCP is late or conflicts with another color profile')
            if b'\0' not in body:raise ValueError('Original PNG profile name missing')
            name,payload=body.split(b'\0',1)
            if not 1<=len(name)<=79 or not payload or payload[0]!=0:raise ValueError('Original PNG profile name/compression invalid')
            decoder=zlib.decompressobj()
            try:original=decoder.decompress(payload[1:],MAX_ICC_BYTES+1)
            except zlib.error as error:raise ValueError('Original PNG ICC compression stream invalid') from error
            if len(original)>MAX_ICC_BYTES or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:raise ValueError('Original PNG ICC stream incomplete/oversized/trailing')
            profile={'kind':'original-PNG-iCCP/v1','name':name.decode('latin1'),'compression_method':0,**icc(original),'compressed_sha256':sha(payload[1:])}
        if kind==b'sRGB':
            if profile is not None or srgb:raise ValueError('Original PNG sRGB/iCCP profile conflict')
            srgb=True
        if kind==b'PLTE':palette=True
        if kind==b'IDAT':image_data=True
        if kind==b'IEND':
            if length or at+12!=len(data):raise ValueError('Original PNG IEND/trailing bytes differ')
            ended=True
        at+=length+12
    if not ended or profile is None:raise ValueError('Complete original PNG with actual iCCP required')
    return profile

def require_png_profile(environment,profile):
    if {key:profile[key] for key in ('bytes','sha256')}!=environment['primary']['color_space']['icc']:raise ValueError('Original PNG ICC differs from observed/approved primary display')

def load(path,limit=24*1024*1024):
    def unique(pairs):
        result={}
        for key,value in pairs:
            if key in result:raise ValueError('Duplicate Native display JSON key')
            result[key]=value
        return result
    path=Path(path).absolute()
    if any(part.is_symlink() for part in (path,*path.parents)):raise ValueError('Native display JSON symlink forbidden')
    metadata=path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_size>limit:raise ValueError('Native display JSON must be a bounded regular original file')
    descriptor=os.open(path,os.O_RDONLY|getattr(os,'O_NOFOLLOW',0))
    with os.fdopen(descriptor,'rb') as stream:data=stream.read(limit+1)
    if len(data)>limit:raise ValueError('Native display JSON exceeds bounded evidence size')
    return json.loads(data.decode('utf-8'),object_pairs_hook=unique,parse_constant=lambda value:(_ for _ in ()).throw(ValueError('Nonfinite Native display JSON')))

def checked_path(root,relative):
    from gate_core import safe_path
    return safe_path(Path(root),relative)

def file_ref(root,path):
    path=Path(path);return {'path':path.relative_to(root).as_posix(),'sha256':sha(path.read_bytes())}

def artifact(root,reference):
    exact(reference,('path','sha256'))
    if not isinstance(reference['sha256'],str) or not SHA.fullmatch(reference['sha256']):raise ValueError('Exact Native display artifact hash required')
    path=checked_path(root,reference['path'])
    if not reference['path'].startswith('.build/') or not path.is_file() or sha(path.read_bytes())!=reference['sha256']:raise ValueError('Native display original artifact differs')
    return path

def validate_environment(value):
    exact(value,('kind','primary'))
    if value['kind']!='general-native-capture-environment/v1':raise ValueError('Native environment contract differs')
    p=value['primary'];exact(p,('display_id','uuid','is_builtin','vendor','model','serial','bounds','mode','rotation_degrees','color_space','backing_scale'))
    integer(p['display_id'],1,2**32-1)
    if not isinstance(p['uuid'],str) or not UUID.fullmatch(p['uuid']):raise ValueError('Original approved display UUID required')
    boolean(p['is_builtin'])
    for key in ('vendor','model','serial'):integer(p[key],0,2**32-1)
    exact(p['bounds'],('x','y','width','height'))
    for key in ('x','y'):number(p['bounds'][key])
    for key in ('width','height'):number(p['bounds'][key],1,32768)
    exact(p['mode'],('width','height','pixel_width','pixel_height','refresh_rate','io_flags'))
    for key in ('width','height','pixel_width','pixel_height'):integer(p['mode'][key],1,65536)
    number(p['mode']['refresh_rate'],0,1000);integer(p['mode']['io_flags'],0,2**32-1)
    if type(p['rotation_degrees']) not in (int,float) or p['rotation_degrees'] not in (0,90,180,270):raise ValueError('Verified approved display rotation required')
    c=p['color_space'];exact(c,('name','model','components','icc'))
    if not isinstance(c['name'],str) or len(c['name'])>256:raise ValueError('Actual approved color-space name required')
    integer(c['model'],-1,16);integer(c['components'],1,16);exact(c['icc'],('bytes','sha256'));integer(c['icc']['bytes'],128,MAX_ICC_BYTES)
    if not isinstance(c['icc']['sha256'],str) or not SHA.fullmatch(c['icc']['sha256']):raise ValueError('Actual approved ICC identity required')
    exact(p['backing_scale'],('x','y'));px,py=p['mode']['pixel_width'],p['mode']['pixel_height']
    for key in ('x','y'):number(p['backing_scale'][key],0.1,16)
    if p['rotation_degrees'] in (90,270):px,py=py,px
    if p['backing_scale']!={'x':px/p['bounds']['width'],'y':py/p['bounds']['height']}:raise ValueError('Approved backing scale does not recompute')
    return value

def approved_environment(root):
    """Read fixed independently reviewed catalogue+lock, never recursively assess."""
    from current_general_reference import AUTHOR_IDS
    from current_general_visual_authority import authority,SCOPE
    root=Path(root);cp=checked_path(root,CATALOGUE);rp=checked_path(root,REVIEW);c,r=load(cp),load(rp)
    catalogue_ref={'path':CATALOGUE,'sha256':sha(cp.read_bytes())};review_ref={'path':REVIEW,'sha256':sha(rp.read_bytes())}
    if c.get('kind')!='general-independent-reference-catalogue/v1' or c.get('scope')!=SCOPE or c.get('source_authority')!=authority(root) or r.get('kind')!='general-independent-reference-review/v1' or r.get('status')!='approved' or r.get('catalogue')!=catalogue_ref:
        raise ValueError('Approved Native display environment requires the fixed exact catalogue/review')
    authors=r.get('author_ids',[]);reviewer=r.get('reviewer_id')
    if not isinstance(authors,list) or any(not isinstance(v,str) for v in authors) or len(set(authors))!=len(authors) or not AUTHOR_IDS<=set(authors) or not isinstance(reviewer,str) or not reviewer.strip() or reviewer in authors:
        raise ValueError('Native environment approval must be independent from all authors')
    if any(r.get(key) is not False for key in ('product_alignment_approved','native_verified','game_tests_executed','os_runtime_verified','storage_runtime_verified')):raise ValueError('Native environment approval overclaims runtime capability')
    layer=c.get('layers',{}).get('packaged_native',{})
    if 'environment' not in layer:raise ValueError('Approved Native display environment missing; independently review a fresh source batch')
    environment=validate_environment(layer['environment']);value=identity(environment)
    if layer.get('environment_sha256')!=value or r.get('approved_native_environment_sha256')!=value:raise ValueError('Native display environment lacks exact independent approval')
    lock_ref=layer['environment_lock'];path=artifact(root,lock_ref);lock,actual_ref=_read_lock(root,path.parent)
    if actual_ref!=lock_ref or lock.get('parent')!=_owned_parent(root,path.parent) or r.get('reviewed_environment_artifacts_sha256',{}).get(lock_ref['path'])!=lock_ref['sha256'] or lock.get('kind')!='general-native-environment-batch-lock/v1' or type(lock.get('schema_version')) is not int or lock['schema_version']!=1 or lock.get('side')!='reference' or lock.get('environment')!=environment or lock.get('environment_sha256')!=value:
        raise ValueError('Approved Native environment differs from independently reviewed original source lock')
    return environment,{'catalogue':catalogue_ref,'review':review_ref,'environment_sha256':value,'source_environment_lock':lock_ref}

def coordinator_cohort(observation,pid):
    if observation.get('kind')!='independent_macos_runtime_observation' or type(observation.get('schema_version')) is not int or observation['schema_version']!=1 or type(observation.get('boot_status')) is not int or observation['boot_status']!=0:raise ValueError('Original batch coordinator OS observation unavailable')
    rows=[row for row in observation.get('processes',[]) if row.get('pid')==pid]
    if len(rows)!=1:raise ValueError('One original batch coordinator process required')
    p=rows[0]
    for key in ('pid','ppid','start_sec','bsd_bytes','path_bytes'):integer(p.get(key),1)
    integer(p.get('start_usec'),0,999999);integer(p.get('device'),1);integer(p.get('inode'),1)
    path=Path(p['executable'])
    if not path.is_absolute() or path!=path.resolve(strict=True) or path.stat().st_dev!=p['device'] or path.stat().st_ino!=p['inode']:raise ValueError('Original coordinator executable/device/inode differs')
    guest=p.get('guest_code',{})
    if any(type(guest.get(key)) is not int or guest[key]!=0 for key in ('guest_status','validity','information_status')) or not isinstance(guest.get('identifier'),str) or not re.fullmatch('[0-9a-f]{40}',guest.get('cdhash','')):raise ValueError('Original batch coordinator code identity invalid')
    return {'process':{key:p[key] for key in ('pid','ppid','executable','device','inode','start_sec','start_usec','guest_code')},'boot':{'sec':integer(observation.get('boot_sec'),1),'usec':integer(observation.get('boot_usec'),0,999999)}}

def _owned_parent(root,parent):
    root=Path(root).resolve();parent=Path(parent)
    if not parent.is_absolute() or not parent.is_relative_to(root/'.build') or parent!=checked_path(root,parent.relative_to(root).as_posix()) or parent!=parent.resolve(strict=True) or not parent.is_dir():raise ValueError('Owned canonical .build Native batch parent required')
    metadata=parent.stat()
    if metadata.st_uid!=os.getuid() or metadata.st_mode&0o022:raise ValueError('Native batch parent ownership/mode differs')
    return {'path':parent.relative_to(root).as_posix(),'device':metadata.st_dev,'inode':metadata.st_ino}

def _read_lock(root,parent):
    path=Path(parent)/LOCK_NAME;checked_path(root,path.relative_to(root).as_posix());metadata=path.stat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid!=os.getuid() or stat.S_IMODE(metadata.st_mode)!=0o600:raise ValueError('Native batch lock ownership/mode differs')
    lock=load(path,1024*1024)
    exact(lock,('kind','schema_version','batch_nonce','parent','owner_uid','side','source_tree_sha256','producer_inputs','environment','environment_sha256','coordinator','approval'))
    if lock['kind']!='general-native-environment-batch-lock/v1' or type(lock['schema_version']) is not int or lock['schema_version']!=1 or lock['side'] not in ('reference','candidate') or not isinstance(lock['source_tree_sha256'],str) or not SHA.fullmatch(lock['source_tree_sha256']) or not re.fullmatch('[0-9a-f]{32}',lock['batch_nonce']):raise ValueError('Typed original Native batch lock identity required')
    integer(lock['owner_uid']);validate_environment(lock['environment'])
    if lock['environment_sha256']!=identity(lock['environment']):raise ValueError('Native batch original environment digest differs')
    return lock,file_ref(root,path)

def batch_lock(root,directory,row,side,source,producer,environment,coordinator,approval):
    """Create only a fresh first-case lock; otherwise require completed prefix."""
    from current_general_visual_authority import capture_cases,case_key
    root=Path(root);directory=Path(directory);parent=directory.parent;parent_info=_owned_parent(root,parent);rows=list(capture_cases())
    if row not in rows or directory.name!=case_key(row) or side not in ('reference','candidate'):raise ValueError('Fixed27 Native batch case/output required')
    index=rows.index(row);path=parent/LOCK_NAME;validate_environment(environment)
    expected={'parent':parent_info,'owner_uid':os.getuid(),'side':side,'source_tree_sha256':source,'producer_inputs':producer,'environment':environment,'environment_sha256':identity(environment),'coordinator':coordinator,'approval':approval}
    if not path.exists() and not path.is_symlink():
        if index!=0 or {p.name for p in parent.iterdir()}!={directory.name}:raise ValueError('Native batch must start at fixed first case in a fresh empty parent')
        value={'kind':'general-native-environment-batch-lock/v1','schema_version':1,'batch_nonce':secrets.token_hex(16),**expected}
        descriptor=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|getattr(os,'O_NOFOLLOW',0),0o600)
        try:
            with os.fdopen(descriptor,'w') as stream:stream.write(json.dumps(value,ensure_ascii=False,sort_keys=True,indent=2)+'\n');stream.flush();os.fsync(stream.fileno())
        except BaseException:
            # A partial failed lock is retained and can never be reused.
            raise
    lock,reference=_read_lock(root,parent)
    if lock.get('kind')!='general-native-environment-batch-lock/v1' or type(lock.get('schema_version')) is not int or lock['schema_version']!=1 or not re.fullmatch('[0-9a-f]{32}',lock.get('batch_nonce','')) or any(lock.get(key)!=value for key,value in expected.items()):raise ValueError('Original Native batch lock/context/coordinator changed; new batch required')
    expected_names={LOCK_NAME,directory.name}|{name for previous in rows[:index] for name in (case_key(previous),case_key(previous)+'.command.json')}
    if {p.name for p in parent.iterdir()}!=expected_names:raise ValueError('Native batch is not the complete fixed27 successful prefix')
    for previous in rows[:index]:_completed_prefix_case(root,parent,previous,lock,reference)
    return reference

def _completed_prefix_case(root,parent,row,lock,lock_ref):
    from current_general_visual_authority import case_key
    directory=checked_path(root,(parent/case_key(row)).relative_to(root).as_posix());r=load(directory/'capture.json');command=load(parent/(case_key(row)+'.command.json'));display=r.get('display_environment',{})
    if r.get('kind')!='general-packaged-native-render-capture/v1' or type(r.get('schema_version')) is not int or r['schema_version']!=1 or r.get('status')!='captured' or r.get('errors')!=[] or type(r.get('exit_code')) is not int or r['exit_code']!=0 or r.get('side')!=lock['side'] or r.get('source_tree_sha256')!=lock['source_tree_sha256'] or r.get('controller_parent_pid')!=lock['coordinator']['process']['pid']:
        raise ValueError('Failed/incomplete Native prefix case cannot be resumed')
    if any(r.get(key)!=row[key] for key in ('state_id','canvas_id','subcase')) or display.get('batch_lock')!=lock_ref or display.get('environment')!=lock['environment'] or display.get('producer_inputs_before')!=lock['producer_inputs'] or display.get('producer_inputs_after')!=lock['producer_inputs']:raise ValueError('Native prefix case/source/env/producer differs')
    expected_tail=['scripts/report/verify_general_native_visual.py','--app',r['package']['app'],'--expected-source',lock['source_tree_sha256'],'--side',lock['side'],'--state',row['state_id'],'--canvas',row['canvas_id'],'--subcase',row['subcase'],'--output',str(directory)]
    actual=command.get('command',[])
    if len(actual)==len(expected_tail)+1 and actual[1]==str(Path(root)/expected_tail[0]):expected_tail[0]=actual[1]
    if actual[1:]!=expected_tail or type(command.get('exit_code')) is not int or command['exit_code']!=0 or command.get('pid')!=r['controller_pid'] or command.get('parent_pid')!=r['controller_parent_pid'] or command.get('cwd')!=str(root):raise ValueError('Original successful Native prefix Popen sidecar missing/differs')
    frames=r.get('frames',[])
    if len(frames)!=2 or [frame.get('path') for frame in frames]!=['frame-1.png','frame-2.png'] or any(sha(checked_path(root,(directory/frame['path']).relative_to(root).as_posix()).read_bytes())!=frame['sha256'] for frame in frames):raise ValueError('Native prefix original frame closure incomplete')
    nonce=r.get('execution_nonce','');finished=load(directory/'finished.json')
    if not re.fullmatch('[0-9a-f]{32}',nonce) or checked_path(root,(directory/'finish-marker.txt').relative_to(root).as_posix()).read_bytes()!=nonce.encode() or finished.get('pid')!=r.get('pid') or finished.get('status')!='finished_by_owned_controller':raise ValueError('Native prefix owned completion/nonce incomplete')

def validate_generation_environment(root,captures,side,environment=None):
    from current_general_visual_authority import capture_cases,case_key
    root=Path(root);rows=list(capture_cases())
    if [value.get('case') for value in captures]!=rows:raise ValueError('Native environment generation must contain the fixed27 ordered cases')
    directories=[checked_path(root,value['raw_directory']) for value in captures]
    parent=directories[0].parent
    if any(directory.parent!=parent or directory.name!=case_key(row) for directory,row in zip(directories,rows)):raise ValueError('Native generation split/substituted its owned batch parent')
    lock,reference=_read_lock(root,parent);validate_environment(lock.get('environment'))
    if lock.get('kind')!='general-native-environment-batch-lock/v1' or lock.get('side')!=side or lock.get('parent')!=_owned_parent(root,parent):raise ValueError('Native generation original batch lock differs')
    if environment is not None:require_same_environment(environment,lock['environment'])
    extras={'generation.json','catalogue-candidate.json'} if side=='reference' else {'report.json'}
    required={LOCK_NAME}|{name for row in rows for name in (case_key(row),case_key(row)+'.command.json')}
    names={value.name for value in parent.iterdir()}
    if not required<=names or names-required-extras:raise ValueError('Native completed generation contains an invalid/failed prefix or extra case')
    for index,(row,directory) in enumerate(zip(rows,directories)):
        _completed_prefix_case(root,parent,row,lock,reference)
        raw=load(directory/'capture.json');d=raw['display_environment']
        if integer(d.get('batch_index'),0,26)!=index or d.get('completed_case_keys')!=[case_key(value) for value in rows[:index]]:raise ValueError('Native original successful prefix declaration differs')
        if captures[index].get('execution') is not None and captures[index]['execution']!=raw:raise ValueError('Native generation execution differs from its original capture.json')
    return lock['environment'],reference

def validate_capture_environment(root,directory,capture,before,after,scale,low_us,high_us):
    """Recheck original probe bytes, code, commands, windows, ICC and batch."""
    from datetime import datetime
    from current_general_visual_authority import reference_inputs
    root=Path(root);directory=Path(directory);d=capture.get('display_environment',{});environment=validate_environment(d.get('environment'))
    if d.get('kind')!='general-native-capture-display-binding/v1' or d.get('environment_sha256')!=identity(environment):raise ValueError('Actual Native display binding absent/differs')
    inputs=reference_inputs(root,'packaged_native')
    if d.get('producer_inputs_before')!=inputs or d.get('producer_inputs_after')!=inputs:raise ValueError('Native display source/finite producer closure changed')
    build_path=artifact(root,d['probe_build']);build=load(build_path);binary=directory/'native-display-probe'
    expected_build=['/usr/bin/xcrun','swiftc','-swift-version','5','-framework','CoreGraphics','-framework','ColorSync','-framework','CryptoKit',str(root/PROBE_SOURCE),'-o',str(binary)]
    if build_path!=directory/'display-probe-build.json' or build.get('command')!=expected_build or type(build.get('exit_code')) is not int or build['exit_code']!=0 or build.get('source_sha256')!=sha((root/PROBE_SOURCE).read_bytes()) or build.get('binary_sha256')!=sha(checked_path(root,binary.relative_to(root).as_posix()).read_bytes()):raise ValueError('Original Native display probe source/compiler/binary differs')
    def micros(text):
        parsed=datetime.fromisoformat(text.replace('Z','+00:00'))
        if parsed.tzinfo is None:raise ValueError('Timezone-aware Native display command interval required')
        return int(parsed.timestamp()*1000000)
    records={};exact(d['probes'],PHASES)
    for phase in PHASES:
        pair=d['probes'][phase];exact(pair,('raw','execution'));raw_path=artifact(root,pair['raw']);execution_path=artifact(root,pair['execution']);raw=load(raw_path);execution=load(execution_path)
        if raw_path!=directory/('display-'+phase+'.json') or execution_path!=directory/('display-'+phase+'.command.json'):raise ValueError('Original Native display probe slot substituted')
        validate_probe(raw,capture['execution_nonce'],phase);require_same_environment(environment,primary_environment(raw))
        if execution.get('command')!=[str(binary),'--nonce',capture['execution_nonce'],'--phase',phase] or type(execution.get('exit_code')) is not int or execution['exit_code']!=0 or execution.get('pid')!=raw['pid'] or execution.get('parent_pid')!=capture['controller_pid'] or raw['ppid']!=capture['controller_pid'] or execution.get('stdout_sha256')!=sha(raw_path.read_bytes()) or execution.get('binary_sha256')!=build['binary_sha256']:raise ValueError('Actual Native display probe command/PID/parent/stdout differs')
        start,end=micros(execution['started_at']),micros(execution['finished_at'])
        if not low_us<=start<=raw['observed_at_us']<=end<=high_us:raise ValueError('Native display probe time outside actual controller interval')
        records[phase]=(start,end)
    parent_path=artifact(root,d['coordinator_observation']);parent_observation=load(parent_path)
    if parent_path!=directory/'batch-parent.json' or coordinator_cohort(parent_observation,capture['controller_parent_pid'])!=d['coordinator']:raise ValueError('Native batch original coordinator cohort differs')
    parent_command_path=artifact(root,d['coordinator_execution']);parent_command=load(parent_command_path);observer_build_path=artifact(root,d['coordinator_observer_build']);observer_build=load(observer_build_path);observer=directory/'runtime-observer'
    if parent_command_path!=directory/'batch-parent.command.json' or parent_command.get('command')!=[str(observer),'--pids',str(capture['controller_pid'])+','+str(capture['controller_parent_pid']),'--output',str(parent_path)] or type(parent_command.get('exit_code')) is not int or parent_command['exit_code']!=0:
        raise ValueError('Original coordinator OS command binding differs')
    if observer_build_path!=directory/'observer-build.json' or observer_build.get('command')!=['/usr/bin/swiftc',str(root/'scripts/quality/runtime_observer.swift'),'-o',str(observer)] or observer_build.get('source_sha256')!=sha((root/'scripts/quality/runtime_observer.swift').read_bytes()) or observer_build.get('exit_code')!=0 or d.get('coordinator_observer_binary_sha256')!=sha(checked_path(root,observer.relative_to(root).as_posix()).read_bytes()):raise ValueError('Original coordinator OS observer source/compiler/binary differs')
    if not records['pre_launch'][1]<=micros(parent_command['started_at'])<=parent_observation['observed_at_ms']*1000<=micros(parent_command['finished_at'])<=micros(d['renderer_launch_started_at']) or d['coordinator']['boot']!={'sec':before['boot_sec'],'usec':before['boot_usec']} or (before['boot_sec'],before['boot_usec'])!=(after['boot_sec'],after['boot_usec']):raise ValueError('Original coordinator observation was replayed/outside case/boot interval')
    lock_path=artifact(root,d['batch_lock']);lock=load(lock_path)
    if lock_path!=directory.parent/LOCK_NAME or lock.get('parent')!=_owned_parent(root,directory.parent) or lock.get('owner_uid')!=os.getuid() or lock.get('coordinator')!=d['coordinator'] or lock.get('producer_inputs')!=inputs or lock.get('side')!=capture['side'] or lock.get('source_tree_sha256')!=capture['source_tree_sha256'] or lock.get('environment')!=environment or lock.get('environment_sha256')!=identity(environment) or lock.get('approval')!=d.get('approval'):raise ValueError('Actual Native original batch lock/context differs')
    _read_lock(root,directory.parent)
    from current_general_visual_authority import capture_cases,case_key
    cases=list(capture_cases());row=next((value for value in cases if all(capture.get(k)==value[k] for k in ('state_id','canvas_id','subcase'))),None)
    if row is None or integer(d.get('batch_index'),0,26)!=cases.index(row) or d.get('completed_case_keys')!=[case_key(value) for value in cases[:cases.index(row)]]:raise ValueError('Native original fixed-prefix identity differs')
    if capture['side']=='candidate':
        approved,binding=approved_environment(root);require_same_environment(approved,environment)
        if d.get('approval')!=binding:raise ValueError('Candidate expected environment is not exact approved permanent authority')
    elif d.get('approval') is not None:raise ValueError('Source cannot self-approve its environment')
    native_start=next(p for p in before['processes'] if p['pid']==capture['pid']);spawn=native_start['start_sec']*1000000+native_start['start_usec']
    if not records['pre_launch'][1]<=micros(d['renderer_launch_started_at'])<=spawn<=records['before_capture'][0] or not before['observed_at_ms']*1000<=records['before_capture'][0]<=records['before_capture'][1]<=micros(capture['pre_capture_settle']['started_at']) or not micros(capture['frames'][-1]['captured_at'])<=records['after_capture'][0]<=records['after_capture'][1]<=after['observed_at_ms']*1000:raise ValueError('Native display phase/capture/OS ordering differs')
    for observed in (before,after):
        window=next(w for w in observed['windows'] if w['pid']==capture['pid'] and w['window_id']==capture['window_id']);require_primary_window(environment,window['bounds'],scale)
    for frame in capture['frames']:
        profile=png_iccp(checked_path(root,(directory/frame['path']).relative_to(root).as_posix()).read_bytes());require_png_profile(environment,profile)
        if frame.get('display_profile')!=profile:raise ValueError('Recorded original PNG profile does not independently recompute')
    return environment
