"""Inspect actual General DOM, build graph, process and command evidence.

Expected semantics are recomputed here from code-owned cases, not producer pass
labels. Original PNG comparison is independently implemented in recompute.py.
"""
from __future__ import annotations
from datetime import datetime
from pathlib import Path
import re
from gate_core import safe_path,digest
from current_general_visual_authority import STATE_IDS,capture_cases,case_key,authority,candidate_inputs,context as visual_context,SCOPE
from current_general_reference import artifact,load,reference_gaps,entries

LEXICAL=('可点击日语分词','显示日语注音','显示词性标记')


def check_facts(row,observation,side,transport):
 c=observation['context'];f=observation['facts']
 if observation.get('error') is not None:raise ValueError('Actual renderer error: '+str(observation.get('error')))
 if c!=visual_context(row,side,c['nonce'],transport):raise ValueError('Actual finite visual context differs')
 if f.get('errors')!=[] or f.get('fontStatus')!='loaded' or f.get('theme')!=row['theme'] or f.get('zoom')!=row['zoom']:raise ValueError('Actual font/theme/zoom/runtime facts differ')
 if f.get('geometry',{}).get('frame')!={'x':0,'y':0,'width':row['width'],'height':row['height']}:raise ValueError('Actual whole-client frame differs')
 if '通用' not in f.get('headings',[]) or '阅读记录' not in f.get('headings',[]) or '本机偏好' not in f.get('headings',[]):raise ValueError('Required General page/groups absent')
 switches={v['label']:v for v in f['switches']};name=row['state_id'].removeprefix('all-page-settings-general-')
 initial=name=='loading' and row['subcase']=='initial'
 expected=(False,True,True) if name=='disabled' else (True,True,True) if name in ('error','save-failed') else (True,True,False)
 for label,selected in zip(LEXICAL,expected):
  if initial:
   if side=='candidate':
    if label in switches:raise ValueError('Initial no-cache candidate retains invented preference controls')
   elif f.get('geometry',{}).get('lexical_card') is not None or label in f.get('text',''):
    raise ValueError('Initial source hidden preference became visible in its box/text facts')
  else:
   actual=switches.get(label)
   if not actual or actual.get('selected') is not selected:raise ValueError('Actual lexical preference differs: '+label)
   disabled=name in ('loading','error') or (name=='disabled' and label==LEXICAL[2])
   if actual.get('disabled') is not disabled:raise ValueError('Actual effective disabled semantics differ: '+label)
 game=name in ('scope-game','restore-inheritance','long-text')
 dirty=name in ('error','save-failed','disabled','restore-inheritance')
 if f.get('scope',{}).get('kind')!=('game' if game else 'global') or f.get('dirty') is not dirty:raise ValueError('Actual scope/draft preservation differs')
 if f['scope'].get('disabled') is not (dirty or name=='loading'):raise ValueError('Actual dirty/pending scope protection differs')
 if f.get('menuOpen') is not (row['subcase']=='menu-open'):raise ValueError('Actual menu state differs')
 if name=='keyboard-focus-escape' and row['subcase']=='primary':
  focus=f.get('focused') or {}
  if focus.get('id')!='scope-button' and focus.get('label')!='阅读偏好作用范围':raise ValueError('Escape failed to restore real trigger focus')
 text=f.get('text','')
 if name=='loading' and '正在读取已保存的阅读偏好来源…' not in text:raise ValueError('Actual pending notice absent')
 if name=='error' and '阅读偏好来源读取失败：暂时无法读取已保存的偏好。已显示的编辑与已保存配置保留。' not in text:raise ValueError('Actual read-error retention notice absent')
 if name=='save-failed' and '阅读偏好保存失败，请重试。编辑内容已保留。' not in text:raise ValueError('Actual failed-save draft notice absent')
 if name=='long-text' and '長いゲーム名と保存された読書設定の表示確認' not in f['scope'].get('label',''):raise ValueError('Approved long data was not rendered')
 if side=='candidate':
  if f.get('renderSource')!='production-WindowRoot' or f.get('startupWrites')!=[] or f.get('forbiddenEffects')!=[]:raise ValueError('Candidate did not use controlled production renderer')
  p=f.get('provenance',{})
  if p.get('controlledTransport') is not True or any(p.get(k) is not False for k in ('productionBootstrapImported','osRuntimeVerified','storageRuntimeVerified','gameExecutionVerified')):raise ValueError('Fixture falsely grants business/OS/DB/game authority')
  if any(v.get('kind')!='save_scoped_preferences' for v in f.get('controlledCommands',[])):raise ValueError('Unexpected actual business command')
  if name in ('error','save-failed') and not f.get('saveCalls'):raise ValueError('Expected real memory-Port save was never called')
 else:
  if f.get('renderSource')!='independent-user-approved-general-html':raise ValueError('Product renderer substituted for independent source')
  p=f.get('provenance',{})
  if any(p.get(k) is not False for k in ('osRuntimeVerified','storageRuntimeVerified','gameTestsExecuted')):raise ValueError('Source design simulation falsely grants runtime authority')
 if transport=='packaged_native':
  acl=f.get('nativeAcl',{})
  if acl.get('coreCommand')!='plugin:window|scale_factor' or acl.get('coreDenied') is not True or not re.search(r'not allowed|not permitted|denied|forbidden',acl.get('coreError',''),re.I) or re.search(r'not found|unknown command',acl.get('coreError',''),re.I):raise ValueError('Actual core command ACL denial absent')
  if acl.get('businessCommand')!='application_snapshot' or acl.get('businessUnknown') is not True or acl.get('businessError')!='Command application_snapshot not found':raise ValueError('Business composition handler was unexpectedly constructed')
 return f


def inspect_build(root,report_path):
 root=Path(root);b=load(report_path.parent/report_path.name)
 if b.get('kind')!='general-independent-vite-build/v1' or b.get('exit_code')!=0 or b.get('operation')!='vite.build(fixed-General-entry-and-independent-source-only)':raise ValueError('Actual finite Vite operation differs')
 expected=candidate_inputs(root)
 if b.get('source_inputs_before')!=expected or b.get('source_inputs_after')!=expected:raise ValueError('Production/entry/fixture/loader source changed across actual build')
 dist=safe_path(root,b['dist']);manifest=load(dist/'general-assets.json')
 if digest(dist/'general-assets.json')!=b['asset_manifest_sha256'] or manifest!=b['assets']:raise ValueError('Actual closed built assets differ')
 for side in ('reference','candidate'):
  for resource,row in manifest[side].items():
   if not resource.startswith('/') or '..' in resource:raise ValueError('Unbounded artifact resource')
   p=safe_path(dist,resource[1:])
   if digest(p)!=row['sha256'] or p.stat().st_size!=row['bytes']:raise ValueError('Actual built artifact hash differs')
 source_modules=set()
 for file in manifest['reference']:
  if file[1:] in b['source_graph']:source_modules.update(b['source_graph'][file[1:]]['modules'])
 if source_modules!={'apps/desktop/tests/native/general/reference-replay.js','apps/desktop/tests/native/general/native-context.ts'}:raise ValueError('Independent source graph imported product/bootstrap code')
 production=set()
 for mapfile in sorted(dist.rglob('*.js.map')):
  mapping=load(mapfile)
  if len(mapping['sources'])!=len(mapping['sourcesContent']):raise ValueError('Production source map incomplete')
  for raw,text in zip(mapping['sources'],mapping['sourcesContent']):
   p=(mapfile.parent/raw).resolve()
   if p.is_relative_to(root/'apps/desktop/src'):
    relative=p.relative_to(root).as_posix()
    if text!=safe_path(root,relative).read_text():raise ValueError('Actual compiled production source text differs')
    production.add(relative)
 if not {'apps/desktop/src/app/WindowRoot.tsx','apps/desktop/src/features/preferences/PreferencesScreen.tsx','apps/desktop/src/features/preferences/GeneralSettings.tsx'}<=production:raise ValueError('Real production WindowRoot/General graph absent')
 if any(p.endswith('/main.tsx') or '/bootstrap' in p for p in production):raise ValueError('Normal bootstrap imported by private visual entry')
 return b


def raw_capture(root,row,layer,side):
 if layer=='real_react':
  import sys
  sys.path.insert(0,str(Path(root)/'scripts/report'))
  from general_visual_webkit import inspect_raw
  result_path=safe_path(root,row['result_file']);execution=load(result_path);directory=safe_path(root,execution['output']);request=load(directory/'request.json')
  if request['mode']!='general-'+side or request['state_id']!=row['case']['state_id'] or request['subcase']!=row['case']['subcase']:raise ValueError('Original WK request differs')
  inspect_raw(directory,request)
  original=load(directory/'native-result.json')
  bundle=safe_path(root,request['bundle']);manifest=load(bundle/'general-assets.json')
  if digest(bundle/'general-assets.json')!=request['bundle_manifest_sha256']:raise ValueError('Loaded closed bundle differs from exact request')
  expected_assets=manifest[side]
  if side=='reference':
   # A reference may outlive a product epoch, but its actual built source graph
   # and every original approved asset remain exact. Do not rebind it to today's
   # unrelated production source inputs or trust only a catalogue's pass label.
   inspect_reference_build(root,bundle,manifest)
  if original.get('source_assets')!={key:{'sha256':value['sha256'],'bytes':value['bytes']} for key,value in expected_assets.items()}:raise ValueError('WK original loaded assets differ from built graph')
  for frame in original['captures']:
   loaded={v['path']:v['sha256'] for v in frame['resources'] if v.get('decision')=='served'}
   if not loaded or any(path not in expected_assets or expected_assets[path]['sha256']!=value for path,value in loaded.items()):raise ValueError('Actual WK resource bytes outside finite source graph')
  for capture in original['captures']:check_facts(row['case'],capture['facts']['observation'],side,layer)
  return directory/'document-1-frame-1.png',original['captures'][0]['facts']['observation'],execution
 return inspect_native_originals(root,safe_path(root,row['raw_directory']),row['case'],side)


def inspect_reference_build(root,bundle,manifest):
 from current_general_visual_authority import source_inputs,PREFIX,RENDER_FILES
 root=Path(root);build=load(bundle.parent/'build.json')
 if build.get('kind')!='general-independent-vite-build/v1' or build.get('operation')!='vite.build(fixed-General-entry-and-independent-source-only)' or build.get('exit_code')!=0 or build.get('normal_bootstrap_imported') is not False:
  raise ValueError('Original independent source build operation differs')
 if safe_path(root,build['dist'])!=bundle or build.get('source_authority')!=authority(root) or build.get('assets')!=manifest or digest(bundle/'general-assets.json')!=build.get('asset_manifest_sha256'):
  raise ValueError('Original independent source build/bundle authority differs')
 allowed={'apps/desktop/tests/native/general/reference-replay.js','apps/desktop/tests/native/general/native-context.ts'}
 modules=set();assets=manifest['reference'];approved=source_inputs(root)
 originals={'/general-reference/'+name:PREFIX+name for name in RENDER_FILES}
 for resource,data in assets.items():
  if not resource.startswith('/') or '..' in resource or '\\' in resource:raise ValueError('Unbounded reference asset path')
  path=safe_path(bundle,resource[1:])
  if digest(path)!=data['sha256'] or path.stat().st_size!=data['bytes']:raise ValueError('Original independent source built asset changed')
  if resource in originals:
   expected=approved[originals[resource]]
   if data['sha256']!=expected['sha256'] or data['bytes']!=expected['bytes']:raise ValueError('Built independent reference substituted an approved source asset')
  else:
   graph=build['source_graph'].get(resource[1:])
   if not resource.endswith('.js') or not graph or not set(graph['modules'])<=allowed:raise ValueError('Independent reference graph imported product/bootstrap code')
   modules.update(graph['modules'])
   mapping=load(safe_path(bundle,resource[1:]+'.map'))
   if len(mapping['sources'])!=len(mapping['sourcesContent']):raise ValueError('Independent compiled source map incomplete')
   actual_sources=set()
   for source,text in zip(mapping['sources'],mapping['sourcesContent']):
    original=(path.parent/source).resolve()
    if not original.is_relative_to(root):raise ValueError('Independent source map escaped original repository')
    relative=original.relative_to(root).as_posix()
    if relative not in allowed or text!=safe_path(root,relative).read_text():raise ValueError('Independent compiled source text differs')
    actual_sources.add(relative)
   if actual_sources!=set(graph['modules']):raise ValueError('Independent actual source map and graph differ')
 if not set(originals)<=set(assets) or modules!=allowed:raise ValueError('Independent approved source closure incomplete')
 return build


def inspect(root,receipt,context):
 errors=[]
 try:
  import sys
  sys.path.insert(0,str(Path(root)/'scripts/report'))
  rp=artifact(root,receipt['report']);r=load(rp);layer=receipt['layer'];expected=list(capture_cases())
  if r.get('scope')!=SCOPE or r.get('layer')!=layer or r.get('source_authority')!=authority(root) or r.get('context')!=load(rp.parent.parent/'request.json'):raise ValueError('Actual scoped report/context/source differs')
  if r.get('execution_nonce')!=context['execution_nonce'] or r.get('source_tree_sha256')!=receipt['source_tree_sha256'] or r.get('artifact_sha256')!=receipt['artifact_sha256']:raise ValueError('Actual invocation source/package/nonce differs')
  if any(r.get(k) is not False for k in ('game_tests_executed','os_runtime_verified','storage_runtime_verified')) or r.get('collection_complete') is not True:raise ValueError('Actual General completeness/capability differs')
  command=load(artifact(root,receipt['execution']))
  if command.get('exit_code')!=0 or command.get('cwd')!=str(Path(root)):raise ValueError('Actual General producer command did not complete')
  if datetime.fromisoformat(command['started_at'].replace('Z','+00:00'))>datetime.fromisoformat(r['started_at'].replace('Z','+00:00')) or datetime.fromisoformat(command['finished_at'].replace('Z','+00:00'))<datetime.fromisoformat(r['finished_at'].replace('Z','+00:00')):raise ValueError('General raw capture outside actual command interval')
  if layer=='real_react':inspect_build(root,rp.parent/r['build'])
  rows=r.get('observations',[])
  if len(rows)!=27 or [v['case'] for v in rows]!=expected:raise ValueError('Actual fixed27 case closure omitted/duplicated/reordered')
  if layer=='packaged_native':
   from current_general_native_display import validate_generation_environment
   validate_generation_environment(root,rows,'candidate')
  nonces=set();pids=set()
  for row in rows:
   _,obs,raw=raw_capture(root,row,layer,'candidate');nonce=obs['context']['nonce']
   if layer=='real_react':
    request=load(safe_path(root,raw['output'])/'request.json')
    if request['bundle']!=load(rp.parent/r['build'])['dist']:raise ValueError('Actual component loaded a different build artifact')
   elif raw.get('source_tree_sha256')!=r['source_tree_sha256'] or raw.get('package',{}).get('archive_sha256')!=r['artifact_sha256']:raise ValueError('Actual Native used a different final source/package')
   if nonce in nonces:raise ValueError('Case reused an earlier execution nonce')
   nonces.add(nonce)
  errors+=reference_gaps(root,STATE_IDS,layer)
 except (OSError,ValueError,KeyError,TypeError,AttributeError,ImportError) as error:errors.append('General actual inspection failed: '+str(error))
 return errors


def _micros(value):
    parsed=datetime.fromisoformat(value.replace('Z','+00:00'))
    if parsed.tzinfo is None:raise ValueError('Timezone-aware original Native interval required')
    return int(parsed.timestamp()*1000000)


def controller_runtime_identity(declared):
    """Bind a direct executable or CPython's exact same-version macOS launcher.

    This validates controller Mach-O code only, not Python.app bundle resources.
    An OS-observed path is never an input to selecting the expected runtime.
    """
    import subprocess,plistlib
    path=Path(declared)
    if not path.is_absolute() or path!=path.resolve(strict=True):raise ValueError('Canonical original controller launcher required')
    def command(argv):
        try:result=subprocess.run(argv,text=True,capture_output=True,timeout=15)
        except subprocess.TimeoutExpired as error:
            def bounded(value):
                if isinstance(value,bytes):return value[:2048].decode('utf-8',errors='replace')
                return '' if value is None else str(value)[:2048]
            raise ValueError('Controller public metadata/code check timed out: argv='+repr(argv)+'; timeout='+repr(error.timeout)+'; stdout='+repr(bounded(error.stdout))+'; stderr='+repr(bounded(error.stderr))) from error
        if result.returncode:raise ValueError('Controller public metadata/code check failed: '+repr(argv)+': '+result.stderr)
        return result.stdout+result.stderr
    def code(file):
        command(['/usr/bin/codesign','--verify','--strict','--ignore-resources',str(file)])
        output=command(['/usr/bin/codesign','-d','--verbose=4',str(file)])
        def field(key):
            values=re.findall(r'^'+re.escape(key)+r'=(.+)$',output,re.M)
            if len(values)!=1:raise ValueError('Exact controller code metadata required: '+key)
            return values[0]
        cdhash=field('CDHash')
        if not re.fullmatch('[0-9a-f]{40}',cdhash):raise ValueError('Actual controller CDHash invalid')
        team=field('TeamIdentifier')
        return {'identifier':field('Identifier'),'team_identifier':None if team=='not set' else team,'cdhash':cdhash}
    runtime=path;result={'kind':'direct','declared_executable':str(path),'code_validation_scope':'Mach-O code only; bundle resources not certified'}
    version_root=path.parent.parent
    if path.parent.name=='bin' and version_root.parent.name=='Versions' and version_root.parent.parent.name=='Python.framework':
        version=version_root.name
        if not re.fullmatch(r'\d+\.\d+',version) or path.name!='python'+version:raise ValueError('Exact same-version CPython Framework launcher layout required')
        runtime=version_root/'Resources/Python.app/Contents/MacOS/Python';library=version_root/'Python'
        if runtime!=runtime.resolve(strict=True) or library!=library.resolve(strict=True):raise ValueError('Canonical same-version CPython runtime required')
        app_info=plistlib.loads((runtime.parent.parent/'Info.plist').read_bytes())
        framework_info=plistlib.loads((version_root/'Resources/Info.plist').read_bytes())
        bundle_version=app_info.get('CFBundleVersion')
        for info,package_type in ((app_info,'APPL'),(framework_info,'FMWK')):
            if info.get('CFBundleIdentifier')!='org.python.python' or info.get('CFBundleExecutable')!='Python' or info.get('CFBundlePackageType')!=package_type or info.get('CFBundleVersion')!=bundle_version or info.get('CFBundleShortVersionString')!=bundle_version:
                raise ValueError('Controller Framework/app executable/version metadata differs')
        if not isinstance(bundle_version,str) or not re.fullmatch(re.escape(version)+r'\.\d+',bundle_version):raise ValueError('Controller Framework/app version does not match launcher')
        for file in (path,runtime):
            output=command(['/usr/bin/otool','-L',str(file)])
            dependencies=[line.strip().split(' (compatibility version ',1)[0] for line in output.splitlines()[1:] if line.startswith('\t')]
            python_libraries=[value for value in dependencies if '/Python.framework/' in value]
            if python_libraries!=[str(library)]:raise ValueError('Controller launcher/runtime do not link the same exact Framework library')
        result.update(kind='cpython_framework_launcher',framework_version=version,bundle_version=bundle_version,launcher_signature=code(path),framework_library_signature=code(library))
    result.update(runtime_executable=str(runtime),runtime_signature=code(runtime))
    return result


def native_process_binding(os,pid,binary,signature,controller_pid,controller_executable,parent_pid,lower_us,upper_us,controller_identity=None):
    """Pure independent check of retained OS facts; never uses inspector PID."""
    import math
    def integer(v):return type(v) is int
    if os.get('kind')!='independent_macos_runtime_observation' or type(os.get('schema_version')) is not int or os.get('schema_version')!=1 or type(os.get('boot_status')) is not int or os.get('boot_status')!=0 or os.get('screen_capture_preflight') is not True:
        raise ValueError('Independent original OS observation unavailable')
    if not all(integer(v) and v>0 for v in (pid,controller_pid,parent_pid)) or len({pid,controller_pid,parent_pid})!=3:
        raise ValueError('Original execution/controller/parent PID cohort invalid')
    processes=os.get('processes',[])
    renderers=[p for p in processes if p.get('pid')==pid];controllers=[p for p in processes if p.get('pid')==controller_pid]
    if len(renderers)!=1 or len(controllers)!=1:raise ValueError('Unique original renderer and controller OS records required')
    p,c=renderers[0],controllers[0]
    actual_controller=controller_executable
    if controller_identity is not None:
        if controller_identity.get('declared_executable')!=controller_executable:raise ValueError('Controller declared launcher differs from original command')
        actual_controller=controller_identity['runtime_executable']
    for row,path,parent in ((p,Path(binary),controller_pid),(c,Path(actual_controller),parent_pid)):
        stat=path.stat()
        if row.get('executable')!=str(path) or row.get('device')!=stat.st_dev or row.get('inode')!=stat.st_ino or row.get('ppid')!=parent:
            raise ValueError('Actual original executable/device/inode/parent differs')
        if not integer(row.get('bsd_bytes')) or row['bsd_bytes']<=0 or not integer(row.get('path_bytes')) or row['path_bytes']<=0:
            raise ValueError('Original OS process lookup was unavailable')
        if not integer(row.get('start_sec')) or not integer(row.get('start_usec')) or row['start_sec']<=0 or not 0<=row['start_usec']<1000000:
            raise ValueError('Original process start identity missing')
    ps=p['start_sec']*1000000+p['start_usec'];cs=c['start_sec']*1000000+c['start_usec']
    if not lower_us<=cs<=ps<=upper_us:raise ValueError('Original process start outside actual launch interval or PID reused')
    if not integer(os.get('boot_sec')) or not integer(os.get('boot_usec')) or not 0<os['boot_sec']<=c['start_sec']:
        raise ValueError('Original OS boot cohort invalid')
    guest=p.get('guest_code',{})
    if any(type(guest.get(k)) is not int or guest[k]!=0 for k in ('guest_status','validity','information_status')):
        raise ValueError('Running Native guest code failed actual OS validity')
    if guest.get('identifier')!=signature['identifier'] or guest.get('team')!=signature['team_identifier'] or guest.get('cdhash')!=signature['cdhash']:
        raise ValueError('Running Native code identifier/team/CDHash differs from inspected signed package')
    if controller_identity is not None:
        code=c.get('guest_code',{});expected=controller_identity['runtime_signature']
        if any(type(code.get(k)) is not int or code[k]!=0 for k in ('guest_status','validity','information_status')) or code.get('identifier')!=expected['identifier'] or code.get('team')!=expected['team_identifier'] or code.get('cdhash')!=expected['cdhash']:
            raise ValueError('Running original controller code identifier/team/CDHash differs from exact runtime')
    cohort=('pid','ppid','executable','device','inode','start_sec','start_usec','guest_code')
    return {'renderer':{k:p[k] for k in cohort},'controller':{k:c[k] for k in cohort if k in c},'boot':(os['boot_sec'],os['boot_usec'])}


def inspect_native_originals(root,directory,row,side):
    """Both reference and candidate use this same independent Native raw path."""
    import hashlib,shutil,tempfile,sys
    import zipfile
    from package_identity import inspect_package,read_signature_metadata
    from reading_alignment_preflight import png_pixels
    root=Path(root);directory=Path(directory);r=load(directory/'capture.json')
    if r.get('kind')!='general-packaged-native-render-capture/v1' or r.get('status')!='captured' or r.get('errors')!=[] or r.get('side')!=side or any(r.get(k) is not False for k in ('game_tests_executed','os_runtime_verified','storage_runtime_verified')):
        raise ValueError('Actual controlled Native original failed or overclaims runtime')
    for k in ('state_id','canvas_id','subcase'):
        if r.get(k)!=row[k]:raise ValueError('Native original case identity differs')
    package=r['package'];app=Path(package['app']);binary=Path(package['binary']);archive=Path(package['archive'])
    for path in (app,binary,archive):
        if path!=path.resolve() or not path.is_relative_to(root/'.build'):raise ValueError('Original package path outside managed immutable build')
        safe_path(root,path.relative_to(root).as_posix())
    if app!=archive.parent/'Luna.app' or binary!=app/'Contents/MacOS/luna-reader':raise ValueError('Original app/executable/archive are not the inspected same package')
    if package.get('source_tree_sha256')!=r.get('source_tree_sha256'):raise ValueError('Native producer source differs from its own exact archive')
    if b'luna-general-native-isolated-renderer/v1:20261108' not in binary.read_bytes():raise ValueError('Original package does not contain the isolated Native composition')
    if digest(binary)!=package['executable_sha256'] or digest(archive)!=package['archive_sha256'] or digest(app/'Contents/Resources/luna-source-identity.json')!=package['sealed_identity_sha256'] or digest(app.parent/'build-manifest.json')!=package['external_manifest_sha256']:
        raise ValueError('Original source package/archive/executable/seal changed')
    with tempfile.TemporaryDirectory(prefix='general-native-independent-package-',dir=root/'.build') as tmp:
        inspected=inspect_package(root,archive,package['source_tree_sha256'],Path(tmp))
    signature=read_signature_metadata(app)
    if signature!=package['signature'] or signature.get('mode')!='certificate' or signature.get('identifier')!='com.lunatranslator.reader' or inspected['executable_sha256']!=package['executable_sha256']:
        raise ValueError('Actual Native package signature/source differs')
    if package['source_tree_sha256']!=r['source_tree_sha256']:raise ValueError('Native producer source differs from its own exact archive')
    started,observed,finished=(load(directory/name) for name in ('started.json','observation.json','finished.json'))
    ctx=observed['observation']['context'];nonce=r['execution_nonce'];pid=r['pid'];controller=r['controller_pid'];parent=r['controller_parent_pid']
    if ctx!=visual_context(row,side,nonce,'packaged_native') or started.get('context')!=ctx or started.get('pid')!=pid or observed.get('pid')!=pid or finished.get('pid')!=pid:
        raise ValueError('Actual Native start/observation/finish PID/nonce/context differs')
    if started.get('mode_contract')!='luna-general-native-isolated-renderer/v1:20261108' or started.get('business_runtime_constructed') is not False or observed.get('kind')!='general-native-raw-observation/v1' or observed.get('window_label')!='general-visual-verification' or observed.get('executable')!=str(binary):
        raise ValueError('Native isolated composition/executable/window differs')
    if observed.get('source_identity')!=load(app/'Contents/Resources/luna-source-identity.json') or digest(directory/'observation.json')!=r['observation_sha256']:
        raise ValueError('Original Native source seal or observation changed')
    marker=(directory/'finish-marker.txt').read_bytes()
    if marker!=nonce.encode() or hashlib.sha256(marker).hexdigest()!=r['finish_marker_sha256'] or finished.get('status')!='finished_by_owned_controller' or r.get('finish_requested') is not True or r.get('exit_code')!=0:
        raise ValueError('Original owned finish nonce/clean lifecycle not proven')
    expected_command=[str(binary),'--general-visual-verification',side,row['state_id'],row['canvas_id'],row['subcase'],nonce]
    if r.get('command')!=expected_command:raise ValueError('Native original launch command differs')
    command_path=directory.with_name(directory.name+'.command.json');command=load(command_path)
    actual_command=command.get('command',[])
    expected_tail=[str(root/'scripts/report/verify_general_native_visual.py'),'--app',str(app),'--expected-source',r['source_tree_sha256'],'--side',side,'--state',row['state_id'],'--canvas',row['canvas_id'],'--subcase',row['subcase'],'--output',str(directory)]
    # Native reference JS uses a repository-relative script argument, while the
    # Python adapter supplies its resolved path; both code-owned forms are exact.
    if len(actual_command)==len(expected_tail)+1 and actual_command[1]=='scripts/report/verify_general_native_visual.py':expected_tail[0]='scripts/report/verify_general_native_visual.py'
    if actual_command[1:]!=expected_tail or command.get('exit_code')!=0 or command.get('pid')!=controller or command.get('parent_pid')!=parent or command.get('cwd')!=str(root):
        raise ValueError('Original Popen driver/controller process binding absent')
    controller_executable=str(Path(shutil.which(actual_command[0]) or actual_command[0]).resolve())
    if r.get('controller_executable')!=controller_executable:raise ValueError('Actual driver interpreter differs from original invoked executable')
    controller_identity=controller_runtime_identity(controller_executable)
    before,after,exited=(load(directory/name) for name in ('os-before.json','os-after.json','os-exited.json'))
    low,high=_micros(command['started_at']),_micros(command['finished_at'])
    if not low<=_micros(r['started_at'])<=_micros(r['finished_at'])<=high:raise ValueError('Native raw interval outside original actual controller command')
    first=native_process_binding(before,pid,binary,signature,controller,controller_executable,parent,low,high,controller_identity)
    second=native_process_binding(after,pid,binary,signature,controller,controller_executable,parent,low,high,controller_identity)
    if first!=second:raise ValueError('Native executable/start/code/boot cohort changed between original captures')
    if not low<=before['observed_at_ms']*1000<=after['observed_at_ms']*1000<=exited['observed_at_ms']*1000<=high:
        raise ValueError('Independent Native OS observation order differs')
    for os in (before,after):
        windows=[w for w in os.get('windows',[]) if w.get('pid')==pid and w.get('window_id')==r['window_id']]
        visible=[w for w in os.get('windows',[]) if w.get('pid')==pid and w.get('on_screen') is True and w.get('layer')==0]
        if len(windows)!=1 or len(visible)!=1 or windows[0]!=visible[0] or windows[0]!=r['window'] or windows[0]['bounds']['Width']!=row['width'] or windows[0]['bounds']['Height']!=row['height']:
            raise ValueError('Actual complete owned window bounds/visibility differ')
    ended=[p for p in exited.get('processes',[]) if p.get('pid')==pid]
    if len(ended)!=1 or ended[0].get('bsd_bytes',0)>0 or ended[0].get('path_bytes',0)>0 or any(w.get('pid')==pid for w in exited.get('windows',[])):
        raise ValueError('Independent OS still sees the owned renderer after exit')
    if (exited['boot_sec'],exited['boot_usec'])!=first['boot']:raise ValueError('Native boot cohort changed at exit')
    scale=observed['window']['backing_scale'];dimensions={'width':row['width']*scale,'height':row['height']*scale}
    if type(scale) not in (int,float) or scale<=0 or observed['window']['inner_pixels']!=dimensions or observed['window']['outer_pixels']!=dimensions:
        raise ValueError('Native complete logical/inner/outer/backing geometry differs')
    frames=r['frames']
    if len(frames)!=2 or [v['path'] for v in frames]!=['frame-1.png','frame-2.png']:raise ValueError('Native fixed two original frame slots missing')
    inspect_native_settle(r,before,after)
    for frame in frames:
        p=safe_path(directory,frame['path']);w,h,_=png_pixels(p.read_bytes(),decode_pixels=False)
        if digest(p)!=frame['sha256'] or p.stat().st_size!=frame['bytes'] or [w,h]!=[row['width']*scale,row['height']*scale] or [w,h]!=[frame['pixel_width'],frame['pixel_height']]:raise ValueError('Actual Native original full PNG/DPR differs')
        if frame['command']!=['/usr/sbin/screencapture','-x','-a','-o','-l',str(r['window_id']),str(p)]:raise ValueError('Native capture command not bound to original actual window')
        if not before['observed_at_ms']*1000<=_micros(frame['captured_at'])<=after['observed_at_ms']*1000:raise ValueError('Native original frame outside observed process/window interval')
    from current_general_native_display import validate_capture_environment
    validate_capture_environment(root,directory,r,before,after,scale,low,high)
    check_facts(row,observed['observation'],side,'packaged_native')
    return directory/'frame-1.png',observed['observation'],r


def inspect_native_settle(capture,before,after):
    import math
    settle=capture.get('pre_capture_settle',{})
    elapsed=settle.get('elapsed_monotonic_seconds')
    if settle.get('kind')!='fixed-native-pre-capture-settle/v1' or type(settle.get('planned_seconds')) is not int or settle['planned_seconds']!=3 or type(elapsed) not in (int,float) or not math.isfinite(elapsed) or elapsed<3:
        raise ValueError('Fixed three-second Native pre-capture settle absent or incomplete')
    endpoints=[settle.get(k) for k in ('started_monotonic','finished_monotonic')]
    if any(type(v) not in (int,float) or not math.isfinite(v) for v in endpoints) or endpoints[1]-endpoints[0]!=elapsed:
        raise ValueError('Native fixed settle monotonic endpoints do not recompute')
    started,finished=_micros(settle['started_at']),_micros(settle['finished_at'])
    if finished-started<3000000 or not before['observed_at_ms']*1000<=started<=finished<=_micros(capture['frames'][0]['captured_at'])<=after['observed_at_ms']*1000:
        raise ValueError('Native fixed settle interval was not before the first original frame')
