/** Independent-source reference candidates only; approval is a separate non-author action. */
import fs from 'node:fs/promises';import path from 'node:path';import assert from 'node:assert/strict';import {createHash}from'node:crypto';import {execFileSync}from'node:child_process';
import {root,json,run,python,fresh,buildGeneralArtifacts,wkCapture} from './verify_general_visual_react.mjs';
const args=process.argv.slice(2),options={};for(let i=0;i<args.length;i+=2){assert.ok(['--output','--layer','--app','--expected-source','--source-map'].includes(args[i]));assert.ok(args[i+1]);assert.equal(options[args[i]],undefined);options[args[i]]=args[i+1];}
const layer=options['--layer']??'real_react';assert.ok(['real_react','packaged_native'].includes(layer));assert.ok(options['--output']);
if(layer==='packaged_native'){assert.ok(options['--app']);assert.match(options['--expected-source'],/^[0-9a-f]{64}$/);assert.ok(options['--source-map']?.startsWith('.build/'));assert.ok(!options['--source-map'].split('/').includes('..'));}else{assert.equal(options['--app'],undefined);assert.equal(options['--expected-source'],undefined);assert.equal(options['--source-map'],undefined);}
const out=await fresh(path.resolve(options['--output'])),started_at=new Date().toISOString(),inputs=python(`reference_inputs(layer='${layer}')`),authority=python('authority()'),captures=[];
let build,nativeSourceBinding=null;
if(layer==='packaged_native'){
 const mapPath=options['--source-map'],mapSha=createHash('sha256').update(await fs.readFile(path.join(root,mapPath))).digest('hex');
 const code='import sys,json;from pathlib import Path;sys.path.insert(0,"scripts/quality");from current_general_reference import native_source_binding;a=json.loads(sys.argv[1]);print(json.dumps(native_source_binding(Path.cwd(),a["map"],a["source"]),ensure_ascii=False))';
 nativeSourceBinding=JSON.parse(execFileSync('python3',['-c',code,JSON.stringify({map:{path:mapPath,sha256:mapSha},source:options['--expected-source']})],{cwd:root,encoding:'utf8'}));
}
if(layer==='real_react')build=await buildGeneralArtifacts(out);
for(const row of python('list(capture_cases())')){
 const key=python(`case_key(${JSON.stringify(row)})`),directory=path.join(out,key);
 if(layer==='real_react')captures.push(await wkCapture(directory,build,'reference',row));
 else{
  const command=['python3','scripts/report/verify_general_native_visual.py','--app',path.resolve(options['--app']),'--expected-source',options['--expected-source'],'--side','reference','--state',row.state_id,'--canvas',row.canvas_id,'--subcase',row.subcase,'--output',directory];
  await run(command,root,directory+'.command.json');const raw=JSON.parse(await fs.readFile(path.join(directory,'capture.json'),'utf8'));assert.equal(raw.status,'captured');captures.push({case:row,side:'reference',raw_directory:path.relative(root,directory),execution:raw});
 }
}
assert.deepEqual(python(`reference_inputs(layer='${layer}')`),inputs,'Reference producer inputs changed');
const generation={kind:'general-reference-candidates/v1',layer,scope:authority.scope,authority,producer_inputs:inputs,native_source_binding:nativeSourceBinding,producer_epoch:layer==='real_react'?'source-renderer-only; final product source is not an input requirement':'Explicit source producer package epoch; final candidate source epoch may differ only outside this exact renderer/source closure',build:build?'build.json':null,captures,started_at,finished_at:new Date().toISOString(),approved:false,product_accepted:false,native_verified:false,game_tests_executed:false,os_runtime_verified:false,storage_runtime_verified:false};
await json(path.join(out,'generation.json'),generation);
const sha=async file=>createHash('sha256').update(await fs.readFile(file)).digest('hex'),artifact=async file=>({path:path.relative(root,file),sha256:await sha(file)});
const entries=[];
for(const capture of captures){
 const directory=path.join(root,capture.raw_directory),key=python(`case_key(${JSON.stringify(capture.case)})`);
 entries.push({case_key:key,case:capture.case,layer,capture_scope:'whole_client',renderer:layer==='real_react'?'public-WKWebView':'signed-package-isolated-WKWebView',
  png:await artifact(path.join(directory,layer==='real_react'?'document-1-frame-1.png':'frame-1.png')),
  facts:await artifact(path.join(directory,layer==='real_react'?'native-result.json':'observation.json'))});
}
await json(path.join(out,'catalogue-candidate.json'),{schema_version:1,kind:'general-independent-reference-catalogue/v1',scope:authority.scope,source_authority:authority,layers:{[layer]:{generation:await artifact(path.join(out,'generation.json')),entries}},approved:false});
console.log(JSON.stringify({status:'captured_candidates_pending_independent_review',generation:path.relative(root,path.join(out,'generation.json')),catalogue:path.relative(root,path.join(out,'catalogue-candidate.json')),layer,case_count:captures.length}));
