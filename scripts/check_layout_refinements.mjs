/** Proposed design-source checks only. No production/native adapters are used. */
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {launchLockedBrowser,verifyBrowserIdentity} from '../apps/desktop/tests/visual/locked-browser.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const source=path.join(root,'docs/design/first-product-v1');
const candidate='candidates/layout-refinements-20261107/';
const output=path.join(root,'.build/current-client-execution-20261105/layout-refinements-20261107/run-'+Date.now());
await fs.mkdir(output,{recursive:true});
const cases=[],errors=[];
const server=http.createServer(async(req,res)=>{
  try{const target=path.resolve(source,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));
    if(!target.startsWith(source+path.sep))throw Error('outside source');
    const bytes=await fs.readFile(target);res.writeHead(200,{'Content-Type':({'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.png':'image/png','.jpg':'image/jpeg','.json':'application/json'}[path.extname(target)]||'application/octet-stream')});res.end(bytes);
  }catch{res.writeHead(404);res.end('not found');}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const origin='http://127.0.0.1:'+server.address().port;
const {browser,identity}=await launchLockedBrowser();
async function open(screen,extra={}){
 const width=extra.width??(screen==='settings'?1000:1040),height=extra.height??720;
 const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1});page.setDefaultTimeout(6000);
 page.on('pageerror',e=>errors.push({screen,error:e.message}));
 await page.clock.setFixedTime(new Date('2026-11-07T13:00:00+08:00'));
 const q=new URLSearchParams({scenario:screen==='history'?'history':screen==='settings'?'settings':'reading',settingsSize:'native',theme:extra.theme??'dark',...extra});q.delete('width');q.delete('height');
 await page.goto(origin+'/'+candidate+screen+'.html?'+q);
 await page.waitForFunction(()=>window.LunaLayoutRefinement);
 if(screen==='history')await page.locator('.hx-record').first().waitFor();
 if(screen==='settings')await page.locator('[data-settings-field="general.segmentation"]').waitFor();
 await page.evaluate(()=>document.fonts.ready);return page;
}
async function capture(page,name){await page.screenshot({path:path.join(output,name+'.png')});}
async function run(name,fn){try{cases.push({name,status:'passed',value:await fn()});}catch(e){cases.push({name,status:'failed',error:e.stack});throw e;}}
try{
 await run('composed entries inherit frozen source and C1 without rewriting them',async()=>{
  const extraHead='<link rel="stylesheet" href="client-layout-20261105/client-c1.css"><link rel="stylesheet" data-layout-delta href="candidates/layout-refinements-20261107/history-settings.css"><link rel="stylesheet" data-layout-delta href="candidates/layout-refinements-20261107/session.css">';
  const extraBody='<script src="client-layout-20261105/client-c1.js"></script><script src="candidates/layout-refinements-20261107/refinement.js"></script>';
  const values=[];
  for(const [screen,parent]of[['history','history-session-paging-20261031'],['settings','guided-service-reading-20261030'],['session','guided-service-reading-20261030']]){
   const original=await fs.readFile(path.join(source,'candidates',parent,'index.html'),'utf8'),actual=await fs.readFile(path.join(source,candidate,screen+'.html'),'utf8');
   assert.equal(actual.replace(' data-layout-refinement="'+screen+'"','').replace(extraHead,'').replace(extraBody,''),original);values.push({screen,parent});
  }return values;
 });
 await run('history shares tracks and16px content starts at both client widths',async()=>{
  const facts=[];
  for(const [width,height]of[[1040,720],[760,560]])for(const compare of['before','after']){
   const page=await open('history',{width,height,compare});
   const value=await page.evaluate(()=>{const rect=s=>document.querySelector(s).getBoundingClientRect().toJSON();return{game:rect('#hx-game-filter--trigger').left,session:rect('#hx-session-filter--trigger').left,list:rect('.hx-list-column'),detail:rect('.hx-detail'),filter:rect('.hx-filters'),body:rect('.hx-body'),gameTrigger:rect('#hx-game-filter--trigger'),sessionTrigger:rect('#hx-session-filter--trigger'),overflow:document.documentElement.scrollWidth>innerWidth};});
   if(compare==='after'){assert.ok(Math.abs(value.game-(value.list.x+16))<1);assert.ok(Math.abs(value.session-(value.detail.x+16))<1);assert.ok(Math.abs(value.gameTrigger.y-value.sessionTrigger.y)<.1);assert.equal(value.overflow,false);}
   await capture(page,'history-'+compare+'-'+width);facts.push({width,compare,value});await page.close();
  }return facts;
 });
 await run('history append preserves selected record and explicit directory wrapper',async()=>{
  const page=await open('history');const prior=await page.evaluate(()=>window.LunaHistoryDesign.getState());
  await page.locator('#hx-records-more').click();await page.waitForFunction(n=>window.LunaHistoryDesign.getState().recordIds.length>n,prior.recordIds.length);
  const after=await page.evaluate(()=>window.LunaHistoryDesign.getState());assert.equal(after.selectedRecordId,prior.selectedRecordId);assert.equal(await page.locator('.layout-history-session-tools').count(),1);await capture(page,'history-records-appended');await page.close();return{before:prior.recordIds.length,after:after.recordIds.length,selectionPreserved:true};
 });
 await run('general setting rows have safe insets and independent toggle state',async()=>{
  const facts=[];
  for(const [width,height]of[[1000,720],[760,560]])for(const compare of['before','after']){
   const page=await open('settings',{width,height,compare});
   await capture(page,'settings-'+compare+'-'+width);
   if(compare==='after'){
    const value=await page.locator('.layout-toggle-row').evaluateAll(rows=>rows.map(row=>{const css=getComputedStyle(row),input=row.querySelector('input'),control=input.getBoundingClientRect(),group=row.parentElement.getBoundingClientRect(),label=row.querySelector('.ls-row-label').getBoundingClientRect();return{paddingLeft:parseFloat(css.paddingLeft),paddingRight:parseFloat(css.paddingRight),labelInset:label.x-group.x,switchInset:group.right-control.right,width:control.width,height:control.height};}));
    assert.equal(value.length,3);for(const row of value){assert.ok(row.paddingLeft>0&&row.labelInset>=row.paddingLeft);assert.ok(row.switchInset>=row.paddingRight);assert.equal(row.width,32);assert.equal(row.height,19);}assert.equal(await page.locator('[data-settings-field="general.history"]').count(),0);
    const input=page.locator('[data-settings-field="general.segmentation"]');await input.scrollIntoViewIfNeeded();const old=await input.isChecked();const ruby=await page.locator('[data-settings-field="general.furigana"]').isChecked();await input.click();assert.equal(await page.locator('[data-settings-field="general.segmentation"]').isChecked(),!old);assert.equal(await page.locator('[data-settings-field="general.furigana"]').isChecked(),ruby);assert.equal(await page.locator('[data-settings-field="general.pos"]').isDisabled(),old);facts.push({width,value,independentToggle:true});
   }
   await page.locator('.ls-section').filter({hasText:'内置分词与词典'}).scrollIntoViewIfNeeded();await capture(page,'settings-toggle-detail-'+compare+'-'+width);await page.close();
  }return facts;
 });
 await run('stopped session shows one reconnect action and three saved-sentence actions',async()=>{
  const facts=[];
  for(const mode of['disconnected','exited','empty','connected','verified']){
   const page=await open('session',{connection:mode==='verified'?'disconnected':mode,window:mode==='verified'?'verified':'none',translation:'failed'});
   if(mode!=='connected')assert.equal(await page.locator('#app-toast:not([hidden])').count(),0);
   const controls=page.locator('#main-content .session-controls button');const labels=await controls.allTextContents();const previewCount=await page.locator('#main-content .rw-preview-column').count();
   if(mode==='connected'){assert.ok(labels.length>=7);assert.equal(previewCount,1);}else{
    assert.equal(await page.locator('#main-content .page-header [data-action="reconnect"]').count(),1);assert.equal(await page.locator('#main-content .page-header [data-action="disconnect"]').count(),0);
    if(mode==='empty'){assert.equal(await page.getByRole('heading',{name:'当前没有可显示的句子',exact:true}).count(),1);assert.equal(await page.getByText('这次会话还没有保存的句子',{exact:true}).count(),0);}
    assert.equal(labels.length,mode==='empty'?0:3);assert.equal(await page.locator('#main-content .session-controls button:disabled').count(),0);assert.equal(previewCount,mode==='verified'?1:0);
   }
   await capture(page,'session-'+mode+'-1040');facts.push({mode,labels,previewCount});await page.close();
  }
  for(const [width,height]of[[1040,720],[760,560]])for(const compare of['before','after']){const page=await open('session',{width,height,compare,connection:'disconnected',translation:'failed'});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await capture(page,'session-'+compare+'-'+width);await page.close();}
  return facts;
 });
 await run('saved sentence auxiliary actions are visible and unavailable translation has recovery',async()=>{
  const page=await open('session',{connection:'disconnected',window:'none'});const prior=await page.evaluate(()=>structuredClone(window.LunaDesign.state.current));
  await page.locator('#main-content .session-controls [data-action="learn-current"]').click();await page.locator('#learning-window').waitFor({state:'visible'});await capture(page,'session-learning-canvas');
  await page.locator('#learning-window [data-action="close-learning"]').click();await page.locator('#main-window').waitFor({state:'visible'});
  await page.locator('#main-content .page-header [data-action="show-hud"]').click();await page.locator('#hud-window').waitFor({state:'visible'});const hudGeometry=await page.locator('#hud-window').evaluate(node=>{const h=node.querySelector('.hud-header').getBoundingClientRect(),r=node.querySelector('.hud-reading').getBoundingClientRect(),f=node.querySelector('.hud-controls').getBoundingClientRect();return{headerBottom:h.bottom,readingTop:r.top,readingBottom:r.bottom,footerTop:f.top,readingWidth:r.width,width:node.getBoundingClientRect().width};});assert.ok(hudGeometry.headerBottom<=hudGeometry.readingTop+1&&hudGeometry.readingBottom<=hudGeometry.footerTop+1);assert.ok(hudGeometry.readingWidth>=hudGeometry.width*.8);await capture(page,'session-hud-canvas');await page.locator('#hud-hide').click();
  await page.locator('#main-content .session-controls [data-action="retranslate-current"]').click();await page.waitForFunction(()=>!['pending','streaming'].includes(window.LunaDesign.state.current?.translationStatus));
  const result=await page.evaluate(()=>({current:structuredClone(window.LunaDesign.state.current),oldPreserved:window.LunaDesign.state.history.some(u=>u.recordId===window.LunaDesign.state.current.recordId)||false,ids:window.LunaDesign.state.history.map(u=>u.recordId)}));
  assert.ok(result.ids.includes(prior.recordId));assert.notEqual(result.current.recordId,prior.recordId);
  if(result.current.translationStatus==='disabled'){await page.locator('.layout-translation-recovery [data-action="open-translation-settings"]').waitFor({state:'visible'});assert.equal(await page.getByText('自动翻译已关闭',{exact:true}).count(),0);}
  else assert.equal(result.current.translationStatus,'complete');
  await capture(page,'session-retranslate-result');await page.close();return{status:result.current.translationStatus,oldRecordPreserved:true,learningVisible:true,hudVisible:true};
 });
 await run('unbinding removes the stopped session preview and OCR does not deny preview permission',async()=>{
  const page=await open('session',{connection:'disconnected',window:'verified'});assert.equal(await page.locator('#main-content .rw-preview-column').count(),1);
  const management=page.locator('.flat-preview-management');await management.locator('summary').click();await management.locator('[data-action="unbind-window"]').click();await page.waitForFunction(()=>!window.LunaLibrary.getBinding(window.LunaDesign.state.session.gameId,window.LunaDesign.game()));assert.equal(await page.locator('#main-content .rw-preview-column').count(),0);await capture(page,'session-unbound-after');await page.close();
  const settings=await open('settings');await settings.locator('[data-settings-action="category"][data-value="ocr"]').click();await settings.getByText('后续范围',{exact:true}).waitFor();assert.equal(await settings.getByText('OCR文字识别属于后续范围；本期游戏窗口预览独立于OCR，按系统要求申请屏幕录制权限。',{exact:true}).count(),1);await capture(settings,'settings-ocr-scope');await settings.close();return{previewRemovedAfterUnbind:true,ocrPermissionSeparate:true};
 });
 await run('light theme and200percent remain horizontally contained and final settings row reachable',async()=>{
  const facts=[];
  for(const screen of['history','settings','session']){
   const page=await open(screen,{width:760,height:560,theme:'light',connection:'disconnected'});await page.evaluate(()=>document.documentElement.style.zoom='2');await page.waitForTimeout(80);
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
   if(screen==='settings'){const last=page.locator('.ls-row').last();await last.scrollIntoViewIfNeeded();const value=await last.evaluate(node=>{const a=node.getBoundingClientRect(),b=node.closest('.ls-scroll').getBoundingClientRect();return{top:a.top,bottom:a.bottom,scrollTop:b.top,scrollBottom:b.bottom};});assert.ok(value.top>=value.scrollTop-1&&value.bottom<=value.scrollBottom+1);facts.push({screen,value});}
   await capture(page,screen+'-light-narrow-200');await page.close();
  }return facts;
 });
 await run('review portal offers all three pages and original-draft comparison outside product frame',async()=>{
  const page=await browser.newPage({viewport:{width:1440,height:1200}});await page.goto(origin+'/'+candidate+'index.html');
  for(const screen of['history','settings','session']){await page.locator('[data-review-page="'+screen+'"]').click();await page.frameLocator('#design-frame').locator('html[data-layout-refinement="'+screen+'"]').waitFor();assert.equal(await page.locator('#design-frame').getAttribute('width'),screen==='settings'?'1000':'1040');}
  await page.locator('[data-compare="before"]').click();await page.frameLocator('#design-frame').locator('html[data-layout-compare="before"]').waitFor();await page.locator('[data-compare="after"]').click();await page.frameLocator('#design-frame').locator('html[data-layout-compare="after"]').waitFor();await capture(page,'review-portal');await page.close();return{pages:3,comparison:true};
 });
 assert.deepEqual(errors,[]);
}finally{
 await verifyBrowserIdentity(identity);await browser.close();await new Promise(resolve=>server.close(resolve));
 const manifest={kind:'proposed-design-layout-verification',browser:identity,cases,errors,design_only:true,client_modified:false,approved_reference_replaced:false,product_completion:'incomplete',pixel_approval:'pending_user_review'};
 await fs.writeFile(path.join(output,'results.json'),JSON.stringify(manifest,null,2)+'\n');console.log('Layout design evidence: '+output);
}
