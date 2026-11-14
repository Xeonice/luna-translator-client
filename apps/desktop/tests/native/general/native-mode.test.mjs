/** Guards only: these fixtures never grant Native/render/product acceptance. */
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {spawnSync} from 'node:child_process';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../../../..');
const require=createRequire(path.join(root,'apps/desktop/package.json'));
const ts=require('typescript');
const output=path.join(root,'.build/general-visual-closure-20261108/phase-2/native/node-guards-'+Date.now());await fs.mkdir(output,{recursive:true});
const source=await fs.readFile(path.join(root,'apps/desktop/tests/native/general/native-context.ts'),'utf8');
const javascript=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ES2022}}).outputText;
const modulePath=path.join(output,'native-context.mjs');await fs.writeFile(modulePath,javascript);
const {getGeneralVisualContext,reportGeneralVisualReady,GENERAL_VISUAL_STATE_IDS}=await import(pathToFileURL(modulePath));
const context=()=>({schemaVersion:1,kind:'general-native-visual-context/v1',transport:'real_react',side:'candidate',stateId:GENERAL_VISUAL_STATE_IDS[0],subcase:'primary',canvasId:'1000x720-dark-z1',nonce:'0123456789abcdef0123456789abcdef',width:1000,height:720,theme:'dark',zoom:1});
const {readGeneralVisualAssets,GENERAL_REFERENCE_FILES}=await import(pathToFileURL(path.join(root,'scripts/report/general_visual_assets.mjs')));
test('finite context accepts exactly14 states and8 canvases without source or system effects',()=>{
  for(const stateId of GENERAL_VISUAL_STATE_IDS)for(const [width,height]of [[1000,720],[760,560]])for(const theme of ['dark','light'])for(const zoom of [1,2]){
    const raw={...context(),stateId,width,height,theme,zoom,canvasId:`${width}x${height}-${theme}-z${zoom}`};assert.equal(getGeneralVisualContext(raw).stateId,stateId);
  }
});
test('missing or malformed context cannot enter a fixture transport',()=>{
  for(const value of [undefined,null,[],{},true,'context'])assert.throws(()=>getGeneralVisualContext(value));
  for(const patch of [{zoom:true},{zoom:'1'},{width:1000.5},{side:'http://example.invalid'},{transport:'http'},{nonce:'../escape'},{nonce:'A'.repeat(32)},{extra:'path'},{schemaVersion:2}])assert.throws(()=>getGeneralVisualContext({...context(),...patch}));
});
test('subcases are limited by actualstate and zoom',()=>{
  assert.throws(()=>getGeneralVisualContext({...context(),subcase:'initial'}));assert.throws(()=>getGeneralVisualContext({...context(),subcase:'menu-open'}));assert.throws(()=>getGeneralVisualContext({...context(),subcase:'content-end'}));
  assert.equal(getGeneralVisualContext({...context(),stateId:GENERAL_VISUAL_STATE_IDS[2],subcase:'initial'}).subcase,'initial');
  assert.equal(getGeneralVisualContext({...context(),stateId:GENERAL_VISUAL_STATE_IDS[9],subcase:'menu-open'}).subcase,'menu-open');
  assert.equal(getGeneralVisualContext({...context(),zoom:2,canvasId:'1000x720-dark-z2',subcase:'content-end'}).subcase,'content-end');
});
test('browser transport reports onlycontrolled facts and never invokes Native',async()=>{
  const calls=[];globalThis.window={__LUNA_GENERAL_VISUAL__:context(),__TAURI_INTERNALS__:{invoke:(...args)=>{calls.push(args);throw Error('must not invoke');}}};
  await reportGeneralVisualReady({text:'受控 General',dirty:false,provenance:{osRuntimeVerified:false}});
  assert.deepEqual(calls,[]);assert.equal(window.__GENERAL_VISUAL_RESULT__.context.transport,'real_react');assert.equal(window.__GENERAL_VISUAL_RESULT__.error,null);
});
test('Native exclusion requires the exact Tauri unknown-command reply, not a business resource error',async()=>{
  for(const [index,message]of ['Command application_snapshot not found','database not found','state not found','resource not found','Command another_command not found','application_snapshot not allowed. Command not found'].entries()){
    const calls=[];globalThis.window={__LUNA_GENERAL_VISUAL__:{...context(),transport:'packaged_native'},__TAURI_INTERNALS__:{invoke:async(command,args)=>{calls.push({command,args});if(command==='plugin:window|scale_factor')throw 'window.scale_factor not allowed';if(command==='application_snapshot')throw message;return undefined;}}};
    const fresh=await import(pathToFileURL(modulePath).href+'?acl='+index);
    await fresh.reportGeneralVisualReady({text:'受控 General'});
    const result=window.__GENERAL_VISUAL_RESULT__;
    assert.equal(result.facts.nativeAcl.coreDenied,true);
    assert.equal(result.facts.nativeAcl.businessError,message);
    assert.equal(result.facts.nativeAcl.businessUnknown,index===0,message);
    assert.equal(result.error===null,index===0,message);
    assert.equal(calls.at(-1).command,'general_visual_observed');
  }
});
test('observation cannot carry unknown top fields, unbounded values or nonJSON data',async()=>{
  globalThis.window={__LUNA_GENERAL_VISUAL__:context()};
  for(const value of [{url:'https://example.invalid'},{text:'x'.repeat(8193)},{geometry:{x:Infinity}},{errors:new Array(129).fill('x')},{dirty:()=>true}])await assert.rejects(reportGeneralVisualReady(value));
  let nested=null;for(let i=0;i<10;i++)nested=[nested];await assert.rejects(reportGeneralVisualReady({geometry:nested}));
});
test('approved reference closure is bytebound and not generated from React',()=>{
  const assets=readGeneralVisualAssets();assert.deepEqual(Object.keys(assets),GENERAL_REFERENCE_FILES);
  assert.ok(assets['settings.html'].bytes.toString().includes('source/settings.css'));
  for(const value of Object.values(assets))assert.ok(!value.bytes.toString().includes('WindowRoot'));
});
test('a changed source asset or a source symlink fails before bundling',async()=>{
  const isolated=path.join(output,'asset-fixture');const prefix='docs/design/first-product-v1/candidates/general-state-evidence-20261108/';
  const names=['docs/development/design-approval-general-states-20261108.json',prefix+'promotion.json',...GENERAL_REFERENCE_FILES.map(n=>prefix+n)];
  for(const relative of names){const target=path.join(isolated,relative);await fs.mkdir(path.dirname(target),{recursive:true});await fs.copyFile(path.join(root,relative),target);}
  const target=path.join(isolated,prefix,'state-gap.css');await fs.appendFile(target,'\n/* altered */');assert.throws(()=>readGeneralVisualAssets(isolated));
  await fs.rm(target);await fs.symlink(path.join(root,prefix,'state-gap.css'),target);assert.throws(()=>readGeneralVisualAssets(isolated));
});
test('driver refuses an old or unrelated binary before executing its sentinel',async()=>{
  const app=path.join(output,'old-app/Luna.app'),binary=path.join(app,'Contents/MacOS/luna-reader'),sentinel=path.join(output,'must-not-launch');await fs.mkdir(path.dirname(binary),{recursive:true});await fs.writeFile(binary,'#!/bin/sh\ntouch '+JSON.stringify(sentinel)+'\n');await fs.chmod(binary,0o700);
  const result=spawnSync('python3',[path.join(root,'scripts/report/verify_general_native_visual.py'),'--app',app,'--expected-source','a'.repeat(64),'--side','candidate','--state',GENERAL_VISUAL_STATE_IDS[0],'--canvas','1000x720-dark-z1','--output',path.join(output,'old-package-rejection')],{encoding:'utf8'});
  assert.equal(result.status,1,result.stdout+result.stderr);await assert.rejects(fs.stat(sentinel));
  const receipt=JSON.parse(await fs.readFile(path.join(output,'old-package-rejection/capture.json'),'utf8'));assert.ok(receipt.errors.some(e=>e.includes('no isolated General visual mode')));assert.equal(receipt.pid,undefined);assert.deepEqual(receipt.protected_processes_before,receipt.protected_processes_after);
});
test('independent process binding rejects changed path, parent, start identity and code signature',()=>{
  const script=String.raw`import copy,json,os,pathlib,sys
namespace={'__name__':'general_driver_guards','__file__':sys.argv[1]};exec(compile(pathlib.Path(sys.argv[1]).read_text(),sys.argv[1],'exec'),namespace)
binary=pathlib.Path(sys.argv[2]);binary.write_text('non-executable identity guard fixture');metadata=binary.stat()
signature={'identifier':'com.lunatranslator.reader','team_identifier':'fixture-team','cdhash':'a'*40}
row={'pid':123,'ppid':os.getpid(),'executable':str(binary),'device':metadata.st_dev,'inode':metadata.st_ino,'start_sec':1,'start_usec':0,'guest_code':{'guest_status':0,'validity':0,'information_status':0,'identifier':signature['identifier'],'team':signature['team_identifier'],'cdhash':signature['cdhash']}}
attestation={'binary':str(binary),'signature':signature};fn=namespace['process_identity'];fn({'processes':[row]},123,attestation,os.getpid())
patches=[{'executable':'/unrelated/client'},{'ppid':0},{'start_sec':None},{'inode':0},{'guest_code':dict(row['guest_code'],validity=-1)},{'guest_code':dict(row['guest_code'],cdhash='b'*40)}]
for patch in patches:
 candidate=copy.deepcopy(row);candidate.update(patch)
 try:fn({'processes':[candidate]},123,attestation,os.getpid())
 except ValueError:continue
 raise AssertionError('changed process identity accepted: '+repr(patch))
# An independent later inspector must bind the original controller, not itself.
independent=copy.deepcopy(row);independent['ppid']=789
fn({'processes':[independent]},123,attestation,789)
try:fn({'processes':[independent]},123,attestation,790)
except ValueError:pass
else:raise AssertionError('different original controller PID accepted')
print(json.dumps({'accepted_fixture':2,'rejected_changes':len(patches)+1,'native_gui_started':False}))`;
  const result=spawnSync('python3',['-c',script,path.join(root,'scripts/report/verify_general_native_visual.py'),path.join(output,'process-identity-fixture')],{encoding:'utf8'});
  assert.equal(result.status,0,result.stdout+result.stderr);assert.deepEqual(JSON.parse(result.stdout),{accepted_fixture:2,rejected_changes:7,native_gui_started:false});
});
test('trusted system temp root is canonical but fresh private descendants reject user symlinks',()=>{
  const script=String.raw`import json,pathlib,sys,tempfile
namespace={'__name__':'general_temp_guards','__file__':sys.argv[1]};exec(compile(pathlib.Path(sys.argv[1]).read_text(),sys.argv[1],'exec'),namespace)
fn=namespace['private_visual_directory'];nonce='a'*32
system_root=pathlib.Path(tempfile.gettempdir()).resolve(strict=True)
assert fn(nonce)==system_root/'luna-general-visual'/nonce
fixture=pathlib.Path(sys.argv[2]);fixture.mkdir();target=fixture/'target';target.mkdir()
namespace['tempfile'].gettempdir=lambda:str(fixture)
base=fixture/'luna-general-visual';base.symlink_to(target,target_is_directory=True)
try:fn(nonce)
except ValueError:pass
else:raise AssertionError('user-created private base symlink accepted')
base.unlink();base.mkdir();(base/nonce).symlink_to(target,target_is_directory=True)
try:fn(nonce)
except ValueError:pass
else:raise AssertionError('user-created private nonce symlink accepted')
print(json.dumps({'trusted_system_root_canonical':True,'rejected_private_symlinks':2,'native_gui_started':False}))`;
  const result=spawnSync('python3',['-c',script,path.join(root,'scripts/report/verify_general_native_visual.py'),path.join(output,'canonical-temp-fixture')],{encoding:'utf8'});
  assert.equal(result.status,0,result.stdout+result.stderr);assert.deepEqual(JSON.parse(result.stdout),{trusted_system_root_canonical:true,rejected_private_symlinks:2,native_gui_started:false});
});
test('normal bootstrap has no verification import and privateconfig creates no ordinary window',async()=>{
  const bootstrap=await fs.readFile(path.join(root,'apps/desktop/src/app/bootstrap.tsx'),'utf8');assert.ok(!bootstrap.includes('general/native-context'));assert.ok(!bootstrap.includes('GeneralVisualPort'));
  const config=JSON.parse(await fs.readFile(path.join(root,'apps/desktop/src-tauri/tauri.visual.conf.json'),'utf8'));assert.deepEqual(config.app.windows,[]);assert.deepEqual(config.app.security.capabilities,[]);
  const capabilities=JSON.parse(await fs.readFile(path.join(root,'apps/desktop/src-tauri/capabilities/default.json'),'utf8'));assert.ok(!capabilities.windows.includes('general-visual-verification'));
});
