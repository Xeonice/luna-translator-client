// Real offline Chrome journeys for a pending design. No service or engine is run.
'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {pathToFileURL}=require('node:url');
const root=path.resolve(process.argv[2]||'.'),out=path.resolve(process.argv[3]||'');
const source='docs/design/first-product-v1/candidates/translation-service-selection-20261030';
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const report={schema_version:1,kind:'translation_service_selection_design_verification',layer:'design',status:'failed',assertions:0,checks:[],screenshots:[],page_errors:[],network_requests:[],console_errors:[],fixtures:[],product_tests_executed:false};
function ok(value,message){report.assertions++;assert.ok(value,message);}
function eq(value,expected,message){report.assertions++;assert.deepEqual(value,expected,message);}
(async()=>{
 let browser,page,outputOwned=false;
 try {
  assert(process.argv[3],'explicit output directory is required');
  const relative=path.relative(root,out);
  assert(relative.startsWith('.build'+path.sep)&&!relative.split(path.sep).includes('..'),'output must be inside root/.build');
  for(let component=out;component!==root;component=path.dirname(component)){
   if(fs.existsSync(component))assert(!fs.lstatSync(component).isSymbolicLink(),'output symlinks forbidden');
  }
  if(fs.existsSync(out)){
   assert(fs.statSync(out).isDirectory(),'output must be a directory');
   const entries=fs.readdirSync(out).sort();
   // Python creates only these two empty streams immediately before spawning.
   assert.deepEqual(entries,['stderr.log','stdout.log'],'fresh Python log directory required');
   for(const name of entries){const entry=fs.lstatSync(path.join(out,name));assert(entry.isFile()&&!entry.isSymbolicLink()&&entry.size===0,'fresh regular empty logs required');}
  } else fs.mkdirSync(out,{recursive:true});
  outputOwned=true;
  let modules=root;
  while(!fs.existsSync(path.join(modules,'node_modules/playwright-core'))){const parent=path.dirname(modules);if(parent===modules)throw Error('Existing playwright-core unavailable');modules=parent;}
  const {chromium}=require(path.join(modules,'node_modules/playwright-core'));
  browser=await chromium.launch({executablePath:process.env.REPORT_CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-background-networking']});
  const context=await browser.newContext({offline:true,viewport:{width:1600,height:1100},reducedMotion:'reduce'});
  page=await context.newPage();page.setDefaultTimeout(6500);
  page.on('pageerror',error=>report.page_errors.push(error.message));
  page.on('console',message=>{if(message.type()==='error')report.console_errors.push(message.text());});
  page.on('request',request=>{if(/^https?:/.test(request.url()))report.network_requests.push(request.url());});
  const state=fn=>page.evaluate(fn);
  const picker=()=>page.locator('#translation-service-picker');
  const row=id=>picker().locator('[data-service-id="'+id+'"]');
  const pick=id=>row(id).locator('[data-service-action=choose]');
  const configure=id=>row(id).locator('[data-service-action=configure]').click();
  const settingsAction=(action,value)=>page.locator('#settings-content [data-settings-action="'+action+'"]'+(value?'[data-value="'+value+'"]':''));
  const field=key=>page.locator('#settings-content [data-settings-field="'+key+'"]');
  async function choose(selector,value){
   const select=page.locator(selector),id=await select.getAttribute('id');
   const trigger=id?page.locator('[data-select-for="'+id+'"]'):null;
   if(trigger&&await trigger.count()){await trigger.click();await page.locator('.luna-select-panel [role=option][data-value="'+value+'"]').click();}
   else await select.selectOption(value);
  }
  async function open(){
   await page.goto(pathToFileURL(path.join(root,source,'index.html')).href+'?scenario=reading');
   await page.waitForFunction(()=>!!window.LunaTranslationServices);
   await page.evaluate(()=>document.fonts.ready);
   await page.locator('#dock [data-action=open-main]').click();
   await page.locator('#main-window [data-page=session]').click();
  }
  async function openPicker(origin='main'){if(origin==='hud')await page.locator('#main-window [data-action=show-hud]').click();else await page.locator('#dock [data-action=open-main]').click();await page.locator(origin==='hud'?'#hud-translation-service':'#session-translation-service').click();ok(await picker().isVisible(),origin+' opens the shared dialog');}
  async function closePicker(){await picker().locator('[data-service-action=close]').first().click();}
  async function returnPicker(){await page.locator('#settings-content [data-service-action=return]').first().click();ok(await picker().isVisible(),'return enters original selection dialog');}
  async function save(){const button=settingsAction('save');ok(!await button.isDisabled(),'edited draft has a real save action');await button.click();eq(await state(()=>window.LunaDesign.state.settings.dirty),false,'save completes without activating');}
  async function capture(name,selector='#translation-service-picker'){
   const filename=name+'.png';await page.locator(selector).screenshot({path:path.join(out,filename),animations:'disabled'});
   const bytes=fs.readFileSync(path.join(out,filename));report.screenshots.push({path:filename,sha256:sha(bytes),bytes:bytes.length,width:bytes.readUInt32BE(16),height:bytes.readUInt32BE(20)});
  }
  async function check(id,fn){try{await fn();report.checks.push({id,status:'passed'});}catch(error){report.checks.push({id,status:'failed',error:error.stack||error.message});console.error(id+': '+(error.stack||error.message));}}
  async function fixture(id,fn){report.fixtures.push({id,kind:'explicit_page_local_fault_injection',product_tests_executed:false});await page.evaluate(fn);}
  async function selectScope(scope){await choose('#translation-service-scope',scope);}
  async function sample(outcome){await settingsAction('tab','runtime').click();await choose('[data-settings-field=testTarget]','model');await choose('[data-settings-field=testOutcome]',outcome);await settingsAction('test').click();await page.waitForFunction(()=>window.LunaDesign.state.settings.testState==='idle');}
  async function waitCurrent(){await page.waitForFunction(()=>!['pending','streaming'].includes(window.LunaDesign.state.current.translationStatus));}
  const selection=()=>state(()=>window.LunaTranslationServices.selection());
  const effective=()=>state(()=>window.LunaSettings.effective(window.LunaDesign.state.settings,'translation',window.LunaDesign.state.session.gameId));

  await check('SS-01',async()=>{
   await open();await openPicker();
   const main=await state(()=>({list:window.LunaTranslationServices.list(),selection:window.LunaTranslationServices.selection()}));
   eq(main.list.map(item=>item.family),['cloud','llm'],'saved cloud and LLM share one list');
   await capture('01-main-shared-picker');await closePicker();await openPicker('hud');
   eq(await state(()=>({list:window.LunaTranslationServices.list(),selection:window.LunaTranslationServices.selection()})),main,'HUD consumes the same saved projection and binding');
   eq(await page.locator('#translation-service-picker').count(),1,'one actual dialog instance');
   await capture('02-hud-shared-picker');await pick('local').click();
   eq((await selection()).mainId,'local','HUD selection updates binding');
   ok((await page.locator('#session-translation-service').innerText()).includes('本机日文模型'),'main reflects HUD choice');
  });
  await check('SS-02',async()=>{
   await open();await openPicker();eq(await state(()=>window.LunaTranslationServices.list().map(item=>item.id)),['traditional','local'],'catalog templates and grammar-only resources excluded');
   const before=await selection();await configure('local');await settingsAction('new-model').click();
   const id=await state(()=>window.LunaDesign.state.settings.selectedModel);
   await field('model.name').fill('已保存翻译配置 · 长名称验证');await field('model.modelId').fill('design-translation-model');
   eq(await state(()=>window.LunaTranslationServices.list().some(item=>item.id===window.LunaDesign.state.settings.selectedModel)),false,'unsaved new draft never enters list');
   await save();eq(await selection(),before,'adding and saving does not select the new resource');
   await returnPicker();ok(await row(id).isVisible(),'saved valid translation resource appears');
   eq(await pick(id).isDisabled(),false,'saved configuration may be explicitly chosen');
   await capture('03-saved-only-list');
   await open();await fixture('empty-saved-services',()=>{const app=window.LunaDesign;app.state.settings.saved.models={};app.state.settings.saved.localEngines={};app.render();});
   await openPicker();eq(await picker().locator('.service-item').count(),0,'empty saved directory does not expose templates');
   ok(await picker().locator('.service-empty [data-service-action=configure]').isVisible(),'empty state has recovery entry');await capture('04-empty-saved-directory');
  });
  await check('SS-03',async()=>{
   await open();await openPicker();await configure('traditional');await settingsAction('tab','connections').click();
   await choose('[data-settings-field="connection.credential"]','');await save();await returnPicker();
   ok(await pick('traditional').isDisabled(),'missing saved credential blocks choice');ok((await row('traditional').innerText()).includes('凭据'),'specific credential reason is visible');
   await capture('05-disabled-credential-reason');
   await open();await openPicker();await configure('local');await sample('auth');await returnPicker();
   ok(await pick('local').isDisabled(),'known failure on this saved revision blocks choice');ok((await row('local').innerText()).includes('认证失败'),'known failure gives recovery reason');
   await open();await openPicker();await configure('traditional');await settingsAction('tab','connections').click();await settingsAction('disable-connection').click();await page.getByRole('button',{name:'停用连接',exact:true}).click();
   eq((await effective()).enabled,false,'explicit connection suspension immediately blocks existing binding');await returnPicker();ok(await pick('traditional').isDisabled(),'suspended saved connection is disabled');
   await open();const original=await state(()=>window.LunaDesign.state.history[0]);await openPicker();await configure('traditional');await settingsAction('delete-model').click();await page.getByRole('button',{name:'停用引用并删除',exact:true}).click();
   eq((await effective()).enabled,false,'deleting selected model immediately revokes runtime before save');
   eq(await state(()=>window.LunaDesign.state.settings.dirty),true,'deletion is still an editor draft');
   eq(await state(()=>window.LunaDesign.state.history[0]),original,'deletion preserves existing history provenance');await capture('06-delete-immediate-revocation','#settings-window');
  });
  await check('SS-04',async()=>{
   await open();await openPicker();eq(await state(()=>window.LunaTranslationServices.list().find(item=>item.id==='local').verification),'unverified','untested HTTP is labeled unverified');
   eq(await pick('local').isDisabled(),false,'untested but valid HTTP is selectable');await pick('local').click();eq((await selection()).mainId,'local','explicit untested HTTP selection works');
   await openPicker();await configure('local');await sample('pass');await returnPicker();eq(await state(()=>window.LunaTranslationServices.list().find(item=>item.id==='local').verification),'passed','only explicit sample changes design test record');
   await configure('local');await settingsAction('tab','models').click();await field('model.modelId').fill('changed-request-model');await save();await returnPicker();
   const item=await state(()=>window.LunaTranslationServices.list().find(item=>item.id==='local'));eq(item.verification,'unverified','new request revision cannot borrow old sample');ok(item.available,'new valid but untested HTTP is still selectable');await capture('07-http-unverified-new-version');
  });
  await check('SS-05',async()=>{
   await open();await openPicker();await picker().locator('.service-picker-footer [data-service-action=configure]').click();await settingsAction('engine-family','offline').click();
   await page.locator('[data-local-action=add-current]').click();await save();await capture('08-local-configuration-unknown','#settings-window');await returnPicker();
   ok(await pick('offline:apple_translation').isDisabled(),'added Apple with unknown necessary conditions cannot be selected');
   ok((await row('offline:apple_translation').innerText()).includes('未知'),'local unknown is explained');await capture('09-local-gate-blocked');
   await configure('offline:apple_translation');await choose('[data-local-field=selected]','argos_translate');await page.locator('[data-local-action=add-current]').click();await save();await returnPicker();
   ok(await pick('offline:argos_translate').isDisabled(),'unimplemented local route is never treated as untested HTTP');eq((await selection()).mainId,'traditional','local gate rejection never falls back or changes choice');
  });
  await check('SS-06',async()=>{
   await open();await openPicker();eq(await row('study').count(),0,'grammar-only never enters translation list');eq(await row('quick').count(),0,'word-only never enters translation list');
   await configure('local');await field('model.name').fill('Microsoft · 日中翻译');await choose('[data-settings-field="model.templatePurpose"]','grammar');await save();await returnPicker();
   eq(await row('local').count(),0,'translation-looking name cannot override explicit grammar purpose');
   await fixture('unsupported-explicit-capability',()=>{const a=window.LunaDesign,m=a.state.settings.saved.models.local;m.templatePurpose='translation';m.capability='unknown';a.render();});
   await closePicker();await openPicker();ok(await pick('local').isDisabled(),'explicit unsupported translation capability remains blocked');
  });
  await check('SS-07',async()=>{
   await open();const before=await selection();await openPicker();await configure('traditional');await field('model.name').fill('新版名称 · 保存不切换');
   eq((await selection()).mainName,before.mainName,'draft name is not the active immutable snapshot');eq(await state(()=>window.LunaTranslationServices.list().find(item=>item.id==='traditional').name),before.mainName,'picker reads saved name before save');
   await capture('10-edit-draft-separate','#settings-window');await save();eq(await selection(),before,'saving does not activate or change binding version');await capture('11-save-does-not-select','#settings-window');
   await sample('pass');eq(await selection(),before,'sample test cannot select or enable');await returnPicker();await pick('traditional').click();eq((await selection()).mainName,'新版名称 · 保存不切换','only explicit choose references newly saved version');
  });
  await check('SS-08',async()=>{
   await open();const old=await state(()=>window.LunaDesign.state.current);await openPicker();await pick('local').click();
   eq(await state(()=>window.LunaDesign.state.current),old,'selection does not retranslate or rewrite current sentence');eq(await state(()=>window.LunaDesign.state.history[0]),old,'existing history keeps old service');
   await page.locator('#review-next').click();await waitCurrent();eq(await state(()=>window.LunaDesign.state.current.serviceSelection.mainId),'local','subsequent sentence captures chosen service');
   await page.locator('#main-window [data-action=show-hud]').click();await page.locator('#hud-translation-toggle').click();eq(await state(()=>window.LunaDesign.state.session.translationEnabled),false,'explicit switch closes automatic translation');eq((await selection()).enabled,false,'purpose enablement is synchronized with explicit close');eq((await effective()).enabled,false,'effective purpose is closed');
   await openPicker();await pick('traditional').click();eq(await state(()=>window.LunaDesign.state.session.translationEnabled),false,'choosing another service does not restart automation');eq((await selection()).enabled,false,'choosing keeps purpose closed');
   const parent=await state(()=>window.LunaDesign.state.current.recordId);await page.locator('#main-window [data-action=show-hud]').click();await page.locator('#hud-retranslate').click();await page.waitForFunction(old=>window.LunaDesign.state.current.recordId!==old,parent);await waitCurrent();
   eq(await state(()=>window.LunaDesign.state.current.translationStatus),'complete','manual current retranslation works while automation closed');eq(await state(()=>window.LunaDesign.state.session.translationEnabled),false,'manual retranslation does not turn automation on');eq((await selection()).enabled,false,'manual retranslation does not enable purpose');
   await capture('12-manual-retranslation-auto-off','#hud-window');
  });
  await check('SS-09',async()=>{
   await open();await openPicker();await selectScope('global');await pick('local').click();await openPicker();await selectScope('game');await pick('traditional').click();
   eq((await selection()).scope,'game','game choice creates explicit override');
   eq(await state(()=>window.LunaTranslationServices.selection(window.LunaDesign,'mo').mainId),'local','another game inherits global independently');
   await openPicker();await selectScope('global');await pick('traditional').click();eq((await selection()).scope,'game','global edit does not discard explicit game override');
   await openPicker();await selectScope('game');await capture('13-explicit-scope-source');await picker().locator('[data-service-action=inherit]').click();eq((await selection()).scope,'global','restore inheritance is explicit');
   const before=await selection();await page.keyboard.press('Escape');eq(await selection(),before,'cancelling scope picker changes no binding');
  });
  await check('SS-10',async()=>{
   await open();const history=await state(()=>window.LunaDesign.state.history[0]);await choose('#review-fault','late');await page.locator('#review-next').click();
   const delayed=await state(()=>window.LunaDesign.state.current.recordId),before=await selection();await openPicker();await page.keyboard.press('Escape');eq(await selection(),before,'Escape cancels selection without creating a binding');
   await openPicker();await pick('local').click();await page.waitForTimeout(1950);
   const cancelled=await state(()=>window.LunaDesign.state.history.find(item=>item.translationStatus==='cancelled'));
   ok(!!cancelled&&cancelled.recordId===delayed,'superseded in-flight design translation remains cancelled after late window');eq(cancelled.serviceSelection.mainId,'traditional','cancelled record keeps original service identity');
   eq(await state(()=>window.LunaDesign.state.history.find(item=>item.recordId===window.LunaDesign.state.history[0].recordId)),history,'previous completed historical result is unchanged');
   await choose('#review-fault','none');await page.locator('#review-next').click();await waitCurrent();eq(await state(()=>window.LunaDesign.state.current.serviceSelection.mainId),'local','next valid sentence belongs to new binding');await capture('14-cancellation-and-new-sentence','#hud-window');
   const current=await state(()=>window.LunaDesign.state.current.recordId);await page.locator('#main-window .sidebar-item[data-page=history]').click();await page.locator('[data-history="'+history.recordId+'"]').click();await page.locator('[data-action=retranslate-history]').click();
   await page.waitForFunction(count=>window.LunaDesign.state.history.length>count,3);await page.waitForTimeout(900);
   const newer=await page.evaluate(parent=>window.LunaDesign.state.history.find(item=>item.recordId.startsWith(parent+':retry:')),history.recordId);
   ok(!!newer,'non-current history retranslation creates independent record');eq(newer.serviceSelection.mainId,'local','history retranslation freezes newly selected service');eq(await state(()=>window.LunaDesign.state.current.recordId),current,'non-current history retranslation does not replace current sentence');
  });
  await check('SS-11',async()=>{
   await open();await openPicker();await configure('local');await field('model.name').fill('Microsoft · 日中翻译');await save();await returnPicker();eq(await state(()=>window.LunaTranslationServices.list().filter(item=>item.name==='Microsoft · 日中翻译').map(item=>item.id)),['traditional','local'],'same display name retains two stable resource IDs');await row('local').locator('[data-service-extra]').check();eq((await selection()).extraIds,['local'],'comparison order is explicit');
   ok(await row('traditional').locator('[data-service-extra]').isDisabled(),'main cannot simultaneously be a comparison');await pick('local').click();eq((await selection()).extraIds,[],'moving comparison to main removes duplicate');
   await openPicker();await row('traditional').locator('[data-service-extra]').check();eq((await selection()).extraIds,['traditional'],'comparison appended in chosen order');await capture('15-main-and-comparison-dedup');await closePicker();
   await page.locator('#review-next').click();await waitCurrent();eq(await state(()=>window.LunaDesign.state.current.serviceSelection.extraIds),['traditional'],'sentence freezes ordered comparison IDs');
   ok(await page.locator('#hud-translations .service-comparison').count()>0,'independent comparison result is rendered');
   const tools=page.locator('#translation-service-design-tools');if(await tools.getAttribute('open')===null)await tools.locator('summary').click();await choose('#service-comparison-engine','traditional');await choose('#service-comparison-outcome','failed');report.fixtures.push({id:'one-comparison-request-fails',kind:'explicit_page_external_design_control',product_tests_executed:false});await page.locator('[data-service-action=simulate-extra]').click();eq(await state(()=>window.LunaDesign.state.current.translationStatus),'complete','one failed comparison cannot cancel main result');eq(await state(()=>window.LunaDesign.state.current.serviceSelection.extras[0].status),'failed','comparison retains its own failed status');ok((await page.locator('#hud-translations [data-service-result-id=traditional]').innerText()).includes('失败'),'independent failure is visible');await capture('19-independent-comparison-failure','#hud-window');
  });
  await check('SS-12',async()=>{
   await open();await openPicker('hud');await selectScope('game');const before=await selection();await configure('local');
   eq(await state(()=>({origin:window.LunaTranslationServices.ui.returnContext.origin,scope:window.LunaTranslationServices.ui.returnContext.scope,gameId:window.LunaTranslationServices.ui.returnContext.gameId})),{origin:'hud',scope:'game',gameId:'sakura'},'configuration round trip retains original picker route');
   await field('model.name').fill('配置往返新名称');await save();await returnPicker();eq(await selection(),before,'saving and returning never selects or enables');
   eq(await state(()=>window.LunaTranslationServices.ui.origin),'hud','return restores HUD origin');await capture('16-context-return-without-selection');
   await configure('local');await field('model.name').fill('尚未保存草稿');await fixture('game-switch-while-configuring',()=>{const a=window.LunaDesign;a.state.session.gameId='mo';a.state.selectedGame='mo';a.render();});await returnPicker();
   eq(await state(()=>window.LunaTranslationServices.ui.gameId),'sakura','return retains original game even when another window changes active session');eq(await state(()=>window.LunaDesign.state.settings.dirty),true,'unsaved draft remains a draft');
   eq(await state(()=>window.LunaTranslationServices.selection(window.LunaDesign,'sakura')),before,'return does not mutate original game binding');
   await closePicker();await page.setViewportSize({width:1280,height:1000});await openPicker();await capture('17-compact-keyboard-picker');await page.keyboard.press('Escape');eq(await picker().isVisible(),false,'Escape dismisses dialog');
   await choose('#review-theme','light');await openPicker();await capture('18-light-shared-picker');ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'compact design avoids page horizontal overflow');
  });
  eq(report.page_errors,[],'all journeys have no page errors');eq(report.network_requests,[],'all journeys remain offline');eq(report.console_errors,[],'all journeys have no console errors');
  report.source_hashes={};
  const manifest=JSON.parse(fs.readFileSync(path.join(root,source,'manifest.json'),'utf8'));
  for(const name of [source+'/manifest.json','scripts/report/verify_translation_service_selection.cjs',...Object.keys(manifest.owned_sources).map(name=>source+'/'+name)])report.source_hashes[name]=sha(fs.readFileSync(path.join(root,name)));
  if(report.checks.length===12&&report.checks.every(check=>check.status==='passed'))report.status='passed';else process.exitCode=1;
 } catch(error){report.error=error.stack||error.message;console.error(error);process.exitCode=1;}
 finally {if(browser)await browser.close();if(outputOwned)fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({status:report.status,assertions:report.assertions,checks:report.checks.length,screenshots:report.screenshots.length,product_tests_executed:false}));}
})();
