/** Real stopped-session UI. Private Vite copy and explicit transport projections. */
import test,{before,after} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {spawn} from 'node:child_process';
import {createServer} from 'node:net';
import {createServer as createHttpServer} from 'node:http';
import {launchLockedBrowser,verifyBrowserIdentity} from '../visual/locked-browser.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../../..');
const evidence=path.join(root,'.build/current-client-execution-20261105/layout-refinement-implementation-20261107');
const output=path.join(evidence,'session','run-'+Date.now()),workspace=path.join(output,'workspace'),desktop=path.join(workspace,'apps/desktop');
const leaves=['apps/desktop/src/features/reading/ReadingPanel.tsx','apps/desktop/src/features/reading/styles.module.css'];
const tracked=[...leaves,'apps/desktop/src/features/reading/session-presentation.ts','apps/desktop/tests/live-preview/verify-stopped-session-layout.mjs'];
const hash=b=>createHash('sha256').update(b).digest('hex');
let browser,identity,vite,server,origin,designOrigin,sourceBefore,fixedPid,viteLog='',browserIdentityVerified=false;
const cases=[];const check=(name,fn)=>test(name,{timeout:60000},async()=>{try{cases.push({name,status:'passed',value:await fn()});}catch(e){cases.push({name,status:'failed',error:e.stack});throw e;}});
async function health(){try{return await(await fetch('http://127.0.0.1:1421/__luna_live_preview_health',{signal:AbortSignal.timeout(500)})).json();}catch{return null;}}
async function sourceHashes(){return Object.fromEntries(await Promise.all(tracked.map(async n=>[n,hash(await fs.readFile(path.join(root,n)))])))}
async function shot(page,name){await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:path.join(output,name+'.png')});}
async function open({width=1040,height=720,theme='dark',sample=false}={}){
 const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1});page.setDefaultTimeout(7000);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(`${origin}/client.html?role=main&view=session&theme=${theme}&state=${sample?'design-reference':'normal'}`);await page.waitForFunction(()=>Boolean(window.liveUiPreview?.port));await page.locator('nav [aria-current=page]').filter({hasText:'当前会话'}).waitFor();return{page,errors};
}
async function seed(page,{state='stopped',bound=false,empty=false,failed=false,annotations=true}={}){
 return page.evaluate(options=>{
  const p=window.liveUiPreview.port;const session=p.state.session;if(!session)throw Error('Missing controlled session');
  p.bindingFixture=options.bound?{state:'bound',sessionId:session.sessionId,targetIdentitySha256:'b'.repeat(64),windowTitle:'受控已核实窗口',verifiedAtMs:'1792368000000',errorCode:null,detail:'Controlled binding transport; no Native proof.'}:{state:'unbound',sessionId:session.sessionId,targetIdentitySha256:null,windowTitle:null,verifiedAtMs:null,errorCode:null,detail:'Controlled unbound transport.'};
  if(options.bound&&!p.previewProjections.size)p.prepareSessionPreview(session.sessionId,'b'.repeat(64));
  const unit=p.state.current?structuredClone(p.state.current):null;const commitRevision=p.state.commitRevision+1;
  let tasks=p.state.tasks;
  if(unit&&options.failed){unit.translations=[];const task={taskId:unit.unitId+':controlled-failed',unitId:unit.unitId,textRevision:unit.textRevision,purpose:'translation',status:'failed',errorCode:'controlled_connection_failure',partialText:null,providerLabel:'受控服务',attemptId:'controlled-attempt'};tasks=[task];p.readingProjectionFixtures[JSON.stringify([unit.unitId,unit.textRevision])]={profileId:p.state.profileId,serverInstanceId:p.state.serverInstanceId,profileGeneration:p.state.profileGeneration,commitRevision,unit:structuredClone(unit),translationState:{unitId:unit.unitId,textRevision:unit.textRevision,sourceHash:unit.sourceHash,engines:[{scopeKey:'global',bindingId:'controlled-binding',engineId:'controlled-engine',providerLabel:task.providerLabel,status:{kind:'current',task}}]}};}
  p.publish({session:{...session,state:options.state},current:options.empty?null:unit,history:{...p.state.history,units:options.empty?p.state.history.units.filter(u=>u.sessionId!==session.sessionId):p.state.history.units},tasks,commitRevision,preferences:{...p.state.preferences,segmentationEnabled:options.annotations,showRuby:options.annotations,showPos:options.annotations}});
  return{sessionId:session.sessionId,unitId:unit?.unitId,textRevision:unit?.textRevision,sourceText:unit?.sourceText};
 },{state,bound,empty,failed,annotations});
}
before(async()=>{
 await fs.mkdir(output,{recursive:true});sourceBefore=await sourceHashes();fixedPid=(await health())?.pid??null;
 for(const relative of['apps/desktop/src','apps/desktop/tests/support','apps/desktop/tests/live-preview','apps/desktop/tsconfig.json','apps/desktop/package.json','docs/design/first-product-v1/assets']){const dest=path.join(workspace,relative);await fs.mkdir(path.dirname(dest),{recursive:true});await fs.cp(path.join(root,relative),dest,{recursive:true});}
 await fs.symlink(path.join(root,'apps/desktop/node_modules'),path.join(desktop,'node_modules'),'dir');await fs.symlink(path.join(root,'node_modules'),path.join(workspace,'node_modules'),'dir');
 const reservation=createServer();await new Promise(r=>reservation.listen(0,'127.0.0.1',r));const port=reservation.address().port;await new Promise(r=>reservation.close(r));origin='http://127.0.0.1:'+port;
 vite=spawn(process.execPath,[path.join(root,'apps/desktop/node_modules/vite/bin/vite.js'),'--config',path.join(desktop,'tests/live-preview/vite.config.ts'),'--port',String(port),'--strictPort'],{cwd:desktop,stdio:['ignore','pipe','pipe']});vite.stdout.on('data',d=>viteLog+=d);vite.stderr.on('data',d=>viteLog+=d);
 for(let i=0;i<150;i++){try{if((await fetch(origin)).ok)break;}catch{}if(i===149||vite.exitCode!==null)throw Error(viteLog);await new Promise(r=>setTimeout(r,100));}
 const designRoot=path.join(root,'docs/design/first-product-v1');server=createHttpServer(async(req,res)=>{try{const p=path.resolve(designRoot,'.'+decodeURIComponent(new URL(req.url,'http://local').pathname));if(!p.startsWith(designRoot+path.sep))throw Error('outside');const bytes=await fs.readFile(p);res.writeHead(200,{'Content-Type':({'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.png':'image/png'}[path.extname(p)]||'application/octet-stream')});res.end(bytes);}catch{res.writeHead(404);res.end();}});await new Promise(r=>server.listen(0,'127.0.0.1',r));designOrigin='http://127.0.0.1:'+server.address().port;
 ({browser,identity}=await launchLockedBrowser());
});
after(async()=>{
 if(browser){await verifyBrowserIdentity(identity);browserIdentityVerified=true;await browser.close();}if(vite?.exitCode===null){vite.kill('SIGTERM');await new Promise(r=>vite.once('exit',r));}if(server)await new Promise(r=>server.close(r));const sourceAfter=await sourceHashes(),fixedAfter=(await health())?.pid??null;const integrity={browserIdentityVerified,expectedCaseCount:7,completeCaseSet:cases.length===7,sourceStable:JSON.stringify(sourceBefore)===JSON.stringify(sourceAfter),fixedPreviewPidStable:fixedAfter===fixedPid};
 await fs.writeFile(path.join(output,'results.json'),JSON.stringify({kind:'real-production-stopped-session-checks',execution_status:integrity.completeCaseSet&&integrity.browserIdentityVerified&&cases.every(c=>c.status==='passed')&&integrity.sourceStable&&integrity.fixedPreviewPidStable?'passed':'failed',integrity,browser:identity,sourceBefore,sourceAfter,fixedPid,fixedAfter,cases,scope:'production React with controlled transport; not Native/game/release',native:'not_run',product_completion:'incomplete'},null,2)+'\n');await fs.writeFile(path.join(output,'vite.log'),viteLog);assert.deepEqual(sourceAfter,sourceBefore);assert.equal(fixedAfter,fixedPid);console.log('Stopped session evidence: '+output);
});
check('same-viewport production before/after snapshots preserve the supplied reading object',async()=>{
 const candidate=Object.fromEntries(await Promise.all(leaves.map(async n=>[n,await fs.readFile(path.join(root,n))])));const result=[];
 try{for(const phase of['before','after']){for(const n of leaves)await fs.writeFile(path.join(workspace,n),phase==='before'?await fs.readFile(path.join(evidence,'preimage',n)):candidate[n]);
  for(const [width,height]of[[1040,720],[760,560]]){const{page,errors}=await open({width,height,sample:true});const owner=await seed(page,{annotations:false});const actions=page.getByRole('group',{name:'当前句操作'});await page.waitForFunction(({phase})=>document.querySelectorAll('[aria-label="当前句操作"] button').length===(phase==='before'?7:3),{phase});
   await shot(page,phase+'-'+width);const geometry=await page.locator('[class*="sessionContent"][data-ui-scroll="main-content"]').evaluate(node=>({width:node.getBoundingClientRect().width,columns:getComputedStyle(node.querySelector('[class*="readingGrid"]')).gridTemplateColumns,previews:node.querySelectorAll('[class*="previewColumn"]').length,card:node.querySelector('[data-guided-current]').getBoundingClientRect().toJSON(),labels:[...node.querySelectorAll('[aria-label="当前句操作"] button')].map(n=>n.textContent),overflow:document.documentElement.scrollWidth>innerWidth}));if(phase==='after'){assert.equal(geometry.previews,0);assert.equal(geometry.labels.length,3);assert.equal(geometry.overflow,false);}assert.deepEqual(errors,[]);result.push({phase,width,height,owner,geometry});await page.close();}
 }}finally{for(const n of leaves)await fs.writeFile(path.join(workspace,n),candidate[n]);}return result;
});
check('stopped failed and crashed states expose saved-object actions but keep live controls intact',async()=>{
 const values=[];for(const state of['stopped','failed','crashed','active','opening']){const{page,errors}=await open();await seed(page,{state});const stopped=!['active','opening'].includes(state);await page.waitForFunction(stopped=>document.querySelector('[data-session-layout]')?.dataset.sessionLayout===(stopped?'stopped':'live'),stopped);const header=page.locator('[data-session-layout]>header');const labels=await page.getByRole('group',{name:'当前句操作'}).getByRole('button').allTextContents();assert.equal(labels.length,stopped?3:7);assert.equal(await header.getByRole('button',{name:'重新连接游戏…',exact:true}).count(),stopped?1:0);assert.equal(await header.getByRole('button',{name:'断开连接…',exact:true}).count(),stopped?0:1);assert.equal(await page.getByRole('group',{name:'当前句操作'}).locator('button:disabled').count(),stopped?0:state==='opening'?2:0);await shot(page,'state-'+state);assert.deepEqual(errors,[]);values.push({state,labels});await page.close();}return values;
});
check('empty visible unit does not claim that saved history is empty',async()=>{
 const{page,errors}=await open();await seed(page,{empty:true});await page.getByRole('heading',{name:'当前没有可显示的句子',exact:true}).waitFor();assert.equal(await page.getByRole('group',{name:'当前句操作'}).count(),0);assert.equal(await page.getByRole('article',{name:'当前原文与译文'}).count(),0);assert.equal(await page.getByText('这次会话还没有保存的句子',{exact:true}).count(),0);await shot(page,'stopped-empty');assert.deepEqual(errors,[]);await page.close();return{emptyCurrentIsNotEmptyHistory:true};
});
check('saved actions freeze old unit and use existing typed window/configuration ports',async()=>{
 const{page,errors}=await open();const old=await seed(page,{failed:true});const actions=page.getByRole('group',{name:'当前句操作'});
 await actions.getByRole('button',{name:'复制原文',exact:true}).click();await actions.getByRole('button',{name:'重新翻译本句',exact:true}).click();await actions.getByRole('button',{name:'分析本句文法',exact:true}).click();
 await page.getByRole('article',{name:'当前原文与译文'}).getByRole('button',{name:'翻译设置…',exact:true}).click();
 const facts=await page.evaluate(()=>{const p=window.liveUiPreview.port;return{copies:p.copiedReadingUnits,retranslations:p.retranslations,learning:p.state.learningObject,settings:p.settingsPageRequests,commands:p.commands.map(x=>x.command.kind)};});assert.deepEqual(facts.copies.at(-1),{unitId:old.unitId,textRevision:old.textRevision,text:old.sourceText});assert.equal(facts.retranslations.at(-1).unitId,old.unitId);assert.equal(facts.learning.unit.unitId,old.unitId);assert.equal(facts.settings.at(-1).intent,'translation');assert.equal(facts.commands.includes('set_capture_paused'),false);assert.equal(facts.commands.includes('set_translation_enabled'),false);await shot(page,'stopped-failed-recovery');assert.deepEqual(errors,[]);await page.close();return{old,facts};
});
check('verified preview stays subscribed across capture stop and is removed only after unbind',async()=>{
 const{page,errors}=await open();await seed(page,{state:'active',bound:true});await page.waitForFunction(()=>window.liveUiPreview.port.previewAttaches.length>0);const initial=await page.evaluate(()=>({attaches:window.liveUiPreview.port.previewAttaches.slice(),detaches:window.liveUiPreview.port.previewDetaches.slice()}));
 await seed(page,{state:'stopped',bound:true});await page.locator('[data-session-layout=stopped]').waitFor();assert.equal(await page.locator('[class*="previewColumn"]').count(),1);const afterStop=await page.evaluate(()=>({attaches:window.liveUiPreview.port.previewAttaches.slice(),detaches:window.liveUiPreview.port.previewDetaches.slice()}));assert.deepEqual(afterStop,initial);
 const management=page.locator('details').filter({has:page.locator('summary',{hasText:'窗口关联管理'})});await management.locator('summary').click();await management.getByRole('button',{name:'解除窗口关联',exact:true}).click();await page.waitForFunction(()=>window.liveUiPreview.port.bindingFixture.state==='unbound');await page.waitForFunction(()=>!document.querySelector('[class*="previewColumn"]'));const facts=await page.evaluate(()=>({session:window.liveUiPreview.port.state.session?.state,current:window.liveUiPreview.port.state.current?.unitId,commands:window.liveUiPreview.port.commands.map(x=>x.command.kind),detaches:window.liveUiPreview.port.previewDetaches.slice()}));assert.equal(facts.session,'stopped');assert.ok(facts.current);assert.ok(facts.commands.includes('unbind_window'));assert.equal(facts.commands.includes('disconnect'),false);await shot(page,'stopped-unbound-after');assert.deepEqual(errors,[]);await page.close();return{initial,afterStop,facts};
});
check('narrow light and200percent keep source and recovery controls reachable',async()=>{
 const facts=[];for(const zoom of[1,2]){const{page,errors}=await open({width:760,height:560,theme:'light'});await seed(page,{failed:true});if(zoom===2)await page.evaluate(()=>document.documentElement.style.zoom='2');for(const name of['重新连接游戏…','复制原文','重新翻译本句','分析本句文法']){const b=page.getByRole('button',{name,exact:true});await b.scrollIntoViewIfNeeded();const box=await b.boundingBox();assert.ok(box.x>=0&&box.x+box.width<=760+1&&box.y>=0&&box.y+box.height<=560+1);}
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await shot(page,'narrow-light-zoom-'+zoom);assert.deepEqual(errors,[]);facts.push({zoom,reachable:true});await page.close();}return facts;
});
check('independent approved design retains the same stopped hierarchy and one-column recovery',async()=>{
 const values=[];for(const[width,height]of[[1040,720],[760,560]]){const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1});const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(designOrigin+'/candidates/layout-refinements-20261107/session.html?scenario=reading&connection=disconnected&window=none');await page.locator('.layout-stopped-reading').waitFor();assert.equal(await page.locator('#main-content .session-controls button').count(),3);assert.equal(await page.locator('#main-content .rw-preview-column').count(),0);const box=await page.locator('.layout-last-sentence').boundingBox();await shot(page,'approved-source-'+width);assert.deepEqual(errors,[]);values.push({width,height,box});await page.close();}return{values,wholeWindowPixels:'diagnostic only; rich lexical and state facts remain independently visible, no auto reference approval'};
});
