import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import { launchLockedBrowser,verifyBrowserIdentity } from '../../../../apps/desktop/tests/visual/locked-browser.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const server=http.createServer(async(req,res)=>{try{const name=decodeURIComponent(new URL(req.url,'http://localhost').pathname);const target=path.resolve(root,'.'+name);if(!target.startsWith(root+path.sep))throw Error('path');const data=await fs.readFile(target);const ext=path.extname(target);res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.json':'application/json','.png':'image/png','.md':'text/plain'})[ext]||'application/octet-stream');res.end(data);}catch{res.statusCode=404;res.end('not found');}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const origin=`http://127.0.0.1:${server.address().port}`;
const {browser,identity}=await launchLockedBrowser();
const inputs=['settings.html','source/settings.css','source/settings.js','assets/material-tokens.css','assets/icons.js','state-gap.css','state-gap.js'];
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const sourceIdentity=Object.fromEntries(await Promise.all(inputs.map(async name=>[name,hash(await fs.readFile(path.join(root,name)))])));
const records=[],errors=[];
try{
for(const state of ['normal','empty','loading','loading-initial','read-error','save-failed'])for(const theme of ['dark','light'])for(const viewport of [{width:1000,height:720},{width:760,height:560}])for(const zoom of [1,2]){
 const name=`${state}-${theme}-${viewport.width}x${viewport.height}-z${zoom}`;
 const page=await browser.newPage({viewport,deviceScaleFactor:1});
 const runtimeErrors=[];page.on('pageerror',e=>runtimeErrors.push(e.message));
 await page.goto(`${origin}/settings.html?${new URLSearchParams({state,theme,zoom:String(zoom)})}`);await page.waitForFunction(()=>window.GeneralStateGapCandidate);await page.evaluate(()=>document.fonts.ready);
 const initial=await page.evaluate(()=>{
  const box=el=>{const r=el.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right};};
  const content=document.querySelector('.content'),footer=document.querySelector('.footer');
  return {documentWidth:document.documentElement.scrollWidth,viewportWidth:innerWidth,viewportHeight:innerHeight,window:box(document.querySelector('.window')),footer:box(footer),content:box(content),contentClientWidth:content.clientWidth,contentScrollWidth:content.scrollWidth,fontWeight:getComputedStyle(document.querySelector('h1')).fontWeight,bodyZoom:getComputedStyle(document.documentElement).zoom,saveDisabled:document.querySelector('#save').disabled,discardDisabled:document.querySelector('#discard').disabled,segmentationDisabled:document.querySelector('#seg-switch').disabled,scopeDisabled:document.querySelector('#scope-button').disabled,dirty:window.GeneralStateGapCandidate.draft.dirty,gameOptions:document.querySelectorAll('[data-scope="game"]').length,posChecked:document.querySelector('#pos-switch').getAttribute('aria-checked')};
 });
 const checks=[];const check=(key,pass)=>{checks.push({key,pass});if(!pass)errors.push({name,key,initial});};
 check('no_document_horizontal_overflow',initial.documentWidth<=viewport.width);
 check('content_no_horizontal_overflow',initial.contentScrollWidth<=initial.contentClientWidth+1);
 check('fixed_footer_in_viewport',initial.footer.y>=0&&initial.footer.bottom<=viewport.height+1);
 check('title_font_700',initial.fontWeight==='700');
 check('window_physical_height',Math.abs(initial.window.height-viewport.height)<1);
 check('no_page_errors',runtimeErrors.length===0);
 if(state==='empty')check('empty_library_only_global_scope',initial.gameOptions===0);
 if(state==='loading'||state==='loading-initial')check('loading_controls_independent_and_disabled',initial.saveDisabled&&initial.discardDisabled&&initial.segmentationDisabled&&initial.scopeDisabled);
 if(state==='loading-initial')check('initial_read_never_claims_mock_defaults',await page.locator('.lexical-card').isHidden());
 if(state==='read-error')check('read_error_retains_unsaved_draft',initial.dirty&&initial.posChecked==='true'&&initial.saveDisabled&&!initial.discardDisabled);
 if(state==='save-failed')check('save_error_retains_edit_and_retry',initial.dirty&&initial.posChecked==='true'&&!initial.saveDisabled&&!initial.discardDisabled);
 await page.screenshot({path:path.join(root,'png',name+'.png')});
 const reachable=[];
 for(const selector of [...(state==='loading-initial'?[]:['#lexical-title']),'#records-title','#local-title',...(state==='read-error'?['#retry-source']:[])]){
  await page.locator(selector).scrollIntoViewIfNeeded();
  const position=await page.locator(selector).evaluate(el=>{const r=el.getBoundingClientRect(),c=document.querySelector('.content').getBoundingClientRect();return {top:r.top,bottom:r.bottom,contentTop:c.top,contentBottom:c.bottom};});
  const pass=position.top>=position.contentTop-1&&position.bottom<=position.contentBottom+1;check('scroll_reachable_'+selector,pass);reachable.push({selector,...position,pass});
 }
 await page.locator('.sidebar-foot').scrollIntoViewIfNeeded();
 const sidebar=await page.locator('.sidebar-foot').evaluate(el=>{const r=el.getBoundingClientRect(),a=document.querySelector('.sidebar').getBoundingClientRect();return {top:r.top,bottom:r.bottom,parentTop:a.top,parentBottom:a.bottom};});
 check('sidebar_footer_reachable',sidebar.top>=sidebar.parentTop-1&&sidebar.bottom<=sidebar.parentBottom+1);
 if(state!=='normal'&&theme==='dark'&&zoom===2)await page.screenshot({path:path.join(root,'png',name+'-scrolled.png')});
 if(state==='save-failed'){
  await page.locator('#save').click();await page.waitForTimeout(1050);check('save_retry_keeps_draft_after_failure',await page.evaluate(()=>window.GeneralStateGapCandidate.draft.dirty&&document.querySelector('#pos-switch').getAttribute('aria-checked')==='true'&&!document.querySelector('#save').disabled));
 }
 if(state==='read-error'){
  await page.locator('#retry-source').click();await page.waitForTimeout(1050);check('read_retry_keeps_draft_after_failure',await page.evaluate(()=>window.GeneralStateGapCandidate.draft.dirty&&document.querySelector('#pos-switch').getAttribute('aria-checked')==='true'&&!document.querySelector('#retry-source').disabled));
 }
 records.push({name,state,theme,viewport,zoom,deviceScaleFactor:1,screenshot:'png/'+name+'.png',initial,reachable,sidebar,checks,runtimeErrors});await page.close();
}
for(const theme of ['dark','light']){
 const page=await browser.newPage({viewport:{width:1000,height:720},deviceScaleFactor:1});await page.goto(`${origin}/settings.html?state=read-error&theme=${theme}&context=native-proposal`);await page.waitForFunction(()=>window.GeneralStateGapCandidate);await page.screenshot({path:path.join(root,'png',`native-context-browser-only-${theme}.png`)});const context=await page.evaluate(()=>({body:getComputedStyle(document.body).backgroundColor,traffic:getComputedStyle(document.querySelector('.traffic')).visibility,fill:getComputedStyle(document.querySelector('.window')).backgroundColor,backdrop:getComputedStyle(document.querySelector('.window')).backdropFilter}));records.push({name:`native-context-browser-only-${theme}`,kind:'browser_simulation_not_native_capture',theme,viewport:{width:1000,height:720},context,screenshot:`png/native-context-browser-only-${theme}.png`,nativeVerified:false});await page.close();
}
await verifyBrowserIdentity(identity);
for(const name of inputs)assert.equal(hash(await fs.readFile(path.join(root,name))),sourceIdentity[name],`Candidate changed during capture: ${name}`);
}finally{await browser.close();await new Promise(resolve=>server.close(resolve));}
const report={schemaVersion:1,kind:'independent-design-candidate-layout-review',status:errors.length?'needs_correction':'layout_review_passed_pending_user_review',referenceApproved:false,productAccepted:false,nativeVerified:false,browser:identity,sourceIdentity,checks:records.reduce((n,r)=>n+(r.checks?.length||0),0),failedChecks:errors,records};
await fs.writeFile(path.join(root,'qa','report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({records:records.length,checks:report.checks,failures:errors.map(x=>({name:x.name,key:x.key}))}));if(errors.length)process.exitCode=1;
