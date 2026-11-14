// Real Chrome verification of the pending flat design. All requests remain offline.
'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {pathToFileURL}=require('node:url');
const root=path.resolve(process.argv[2]||'.'),out=path.resolve(process.argv[3]||'');
const source='docs/design/first-product-v1/candidates/flat-service-session-20261030';
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const report={schema_version:1,kind:'flat_service_session_design_verification',layer:'design',status:'failed',assertions:0,checks:[],screenshots:[],page_errors:[],network_requests:[],console_errors:[],fixtures:[],product_tests_executed:false};
function ok(value,message){report.assertions++;assert.ok(value,message);}
function eq(value,expected,message){report.assertions++;assert.deepEqual(value,expected,message);}
(async()=>{
 let browser,page,outputOwned=false;
 try{
  assert(process.argv[3],'explicit output directory required');
  const relative=path.relative(root,out);assert(relative.startsWith('.build'+path.sep)&&!relative.split(path.sep).includes('..'),'output must be a new directory inside root/.build');
  for(let component=out;;component=path.dirname(component)){
   try{assert(!fs.lstatSync(component).isSymbolicLink(),'output symlinks forbidden');}catch(error){if(error.code!=='ENOENT')throw error;}
   if(component===root)break;
  }
  assert(fs.statSync(root).isDirectory(),'root must be a directory');
  if(fs.existsSync(out)){
   assert(fs.lstatSync(out).isDirectory(),'output must be a directory');
   const entries=fs.readdirSync(out).sort();assert.deepEqual(entries,['stderr.log','stdout.log'],'output must be new or contain only two precreated empty Python logs');
   for(const name of entries){const info=fs.lstatSync(path.join(out,name));assert(info.isFile()&&!info.isSymbolicLink()&&info.size===0,'logs must be regular empty files');}
  }else fs.mkdirSync(out,{recursive:true});
  outputOwned=true;
  const manifest=JSON.parse(fs.readFileSync(path.join(root,source,'manifest.json'),'utf8'));
  const inputs=[source+'/manifest.json','scripts/report/verify_flat_service_session.cjs',...Object.keys(manifest.owned_sources).map(name=>source+'/'+name),...Object.keys(manifest.shared_dependencies).map(name=>'docs/design/first-product-v1/'+name)];
  report.source_hashes=Object.fromEntries(inputs.map(name=>[name,sha(fs.readFileSync(path.join(root,name)))]));
  for(const [name,hash]of Object.entries(manifest.owned_sources))assert.equal(report.source_hashes[source+'/'+name],hash,'owned source identity mismatch: '+name);
  let modules=root;while(!fs.existsSync(path.join(modules,'node_modules/playwright-core'))){const parent=path.dirname(modules);if(parent===modules)throw Error('Existing playwright-core unavailable');modules=parent;}
  const {chromium}=require(path.join(modules,'node_modules/playwright-core'));
  browser=await chromium.launch({executablePath:process.env.REPORT_CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-background-networking']});
  const context=await browser.newContext({offline:true,viewport:{width:1600,height:1100},reducedMotion:'reduce'});
  page=await context.newPage();page.setDefaultTimeout(6500);
  page.on('pageerror',error=>report.page_errors.push(error.message));
  page.on('console',message=>{if(message.type()==='error')report.console_errors.push(message.text());});
  page.on('request',request=>{if(/^https?:/.test(request.url()))report.network_requests.push(request.url());});
  const state=fn=>page.evaluate(fn),editor=()=>page.locator('[data-flat-section=service-editor]');
  const section=key=>page.locator('[data-flat-session-section='+key+']');
  const action=(name,scope='#settings-content')=>page.locator(scope+' [data-settings-action="'+name+'"]');
  const field=(name,scope='#settings-content')=>page.locator(scope+' [data-settings-field="'+name+'"]');
  const listField=(key,name)=>section(key).locator('[data-flat-field="'+name+'"]');
  const follow=key=>section(key).locator('[data-flat-list-action=toggle-follow]');
  const selected=()=>state(()=>({selection:window.LunaTranslationServices.selection(),automatic:window.LunaDesign.state.session.translationEnabled}));
  async function choose(selector,value){
   const select=page.locator(selector),id=await select.getAttribute('id'),trigger=id?page.locator('[data-select-for="'+id+'"]'):null;
   if(trigger&&await trigger.count()){await trigger.click();await page.locator('.luna-select-panel [role=option][data-value="'+value+'"]').click();}
   else await select.selectOption(value);
  }
  async function open(scenario='settings',theme='dark'){
   await page.setViewportSize({width:1600,height:1100});
   await page.goto(pathToFileURL(path.join(root,source,'index.html')).href+'?scenario='+scenario+'&theme='+theme);
   await page.waitForFunction(()=>!!window.LunaFlatDesign&&!!window.LunaTranslationServices);await page.evaluate(()=>document.fonts.ready);
  }
  async function edit(id){await page.locator('[data-flat-service="'+id+'"]').click();}
  async function save(){const controls=action('save');ok(await controls.count()>=1,'save commands are present');for(const control of await controls.all())eq(await control.isDisabled(),false,'all save presentations enable together');await controls.first().click();eq(await state(()=>window.LunaDesign.state.settings.dirty),false,'save finishes as a saved version');for(const control of await controls.all())eq(await control.isDisabled(),true,'all save presentations disable after save');}
  async function capture(name,selector){
   const filename=name+'.png';if(selector)await page.locator(selector).screenshot({path:path.join(out,filename),animations:'disabled'});else await page.screenshot({path:path.join(out,filename),animations:'disabled'});
   const bytes=fs.readFileSync(path.join(out,filename));report.screenshots.push({path:filename,sha256:sha(bytes),bytes:bytes.length,width:bytes.readUInt32BE(16),height:bytes.readUInt32BE(20)});
  }
  async function check(id,fn){const start=report.assertions;try{await fn();report.checks.push({id,status:'passed',assertions:report.assertions-start});}catch(error){report.checks.push({id,status:'failed',assertions:report.assertions-start,error:error.stack||error.message});console.error(id+': '+(error.stack||error.message));try{await capture(id+'-failure');}catch{}}}
  async function fixture(id,fn){report.fixtures.push({id,kind:'explicit_page_local_design_fixture',product_tests_executed:false});await page.evaluate(fn);}
  async function expand(key){const target=section(key);if(!await target.evaluate(node=>node.open))await target.locator(':scope>summary').click();}
  async function advance(count){for(let i=0;i<count;i++)await page.locator('#review-next').click();}
  async function exportData(key,scope='session'){
   await section(key).locator('[data-flat-list-action=export-session]').click();
   eq(await page.locator('#rw-export-kind').inputValue(),key==='reading'?'body':key,'export defaults to originating data set');
   await choose('#rw-export-scope',scope);await page.locator('#dialog-confirm').click();
   const result=JSON.parse(await page.locator('#app-dialog textarea').inputValue());await page.locator('#dialog-cancel').click();return result;
  }

  await check('FL-01',async()=>{
   await open();const data=await state(()=>{const s=window.LunaDesign.state.settings;return Object.keys(s.saved.models).concat(Object.values(s.saved.localEngines).filter(e=>e.added).map(e=>'offline:'+e.id));});
   eq(await page.locator('[data-flat-service]').evaluateAll(nodes=>nodes.map(n=>n.dataset.flatService)),data,'flat directory projects saved resource identities');
   eq(await page.locator('#settings-content [role=tab],#settings-content .local-engine-family').count(),0,'no family or entity tabs');
   ok(await page.locator('[data-flat-section=saved-services]').isVisible(),'saved directory is visible');await capture('01-settings-directory','#settings-window');
   await edit('local');eq(await editor().locator('[data-settings-field="connection.endpoint"]').count(),1,'connection editor coexists');eq(await editor().locator('[data-settings-field="model.modelId"]').count(),1,'applicable model editor coexists');eq(await editor().locator('[data-settings-action=test]').count(),1,'sample action coexists with fields');
   const ids=await page.locator('[data-flat-service]').evaluateAll(nodes=>nodes.map(n=>n.dataset.flatService));eq(new Set(ids).size,ids.length,'same names cannot replace distinct IDs');
   await capture('02-settings-llm-fields','#settings-window');
  });
  await check('FL-02',async()=>{
   await open();await edit('traditional');eq(await editor().locator('[data-settings-field="model.modelId"],[data-settings-field="model.template"]').count(),0,'traditional translation hides LLM-only fields');eq(await editor().locator('[data-settings-field="connection.credential"]').count(),1,'traditional authentication remains');await capture('03-fields-traditional','#settings-window');
   await action('flat-add-service').click();await page.locator('[data-flat-add=offline]').click();
   eq(await editor().getAttribute('data-flat-editor-type'),'offline','native editor replaces HTTP fields in the same page');eq(await editor().locator('[data-settings-field="connection.endpoint"],[data-settings-field="model.modelId"]').count(),0,'native has no fake HTTP or model fields');eq(await editor().locator('[data-local-field=installPath]').count(),0,'Apple uses system resources rather than DLL directory');
   await page.locator('[data-local-field=name]').fill('本机离线配置 · 未验证');await save();await capture('04-fields-apple','#settings-window');
   await choose('[data-local-field=kind]','argos_translate');eq(await editor().locator('[data-local-field=installPath]').count(),1,'Argos resource directory remains');eq(await editor().locator('[data-local-field=route]').count(),1,'Argos direct/pivot route remains');eq(await editor().locator('[data-local-field=arch]').count(),0,'Argos does not expose commercial DLL bitness');await capture('05-fields-argos','#settings-window');
   await choose('[data-local-field=kind]','jbeijing');eq(await editor().locator('[data-local-field=arch]').count(),1,'DLL engine exposes architecture');eq(await editor().locator('[data-local-field=route]').count(),0,'commercial DLL does not expose Argos routing');
   await save();await page.locator('#settings-content [data-service-action=return]').first().click();const id=await state(()=> 'offline:'+window.LunaLocalTranslation.ensure(window.LunaDesign.state.settings).selected);ok(await page.locator('[data-service-id="'+id+'"] [data-service-action=choose]').isDisabled(),'unknown native runtime conditions still block selection');await capture('06-native-gate','#translation-service-picker');
  });
  await check('FL-03',async()=>{
   await open();const before=await selected();await edit('local');await field('model.name').fill('修改后的翻译配置');eq(await selected(),before,'editing cannot activate a draft');await save();eq(await selected(),before,'saving cannot select or enable');
   await action('test').click();await page.waitForFunction(()=>window.LunaDesign.state.settings.testState==='idle');eq(await selected(),before,'testing cannot select or enable');
   await page.locator('#settings-content [data-service-action=return]').first().click();await page.locator('[data-service-id=local] [data-service-action=choose]').click();eq((await selected()).selection.mainId,'local','explicit saved service selection changes binding');eq((await selected()).automatic,before.automatic,'selection preserves automatic translation state');
   await page.locator('#dock [data-action=open-main]').click();await page.locator('#main-window [data-page=session]').click();await page.locator('#session-translation-service').click();await page.locator('[data-service-id=local] [data-service-action=configure]').click();eq(await page.locator('[data-flat-service=local]').getAttribute('aria-pressed'),'true','configure returns to exact resource editor');await page.locator('#settings-content [data-service-action=return]').first().click();ok(await page.locator('#translation-service-picker').isVisible(),'return restores picker context');await capture('07-save-test-select-boundary','#translation-service-picker');
  });
  await check('FL-04',async()=>{
   await open();await edit('local');const disclosure=page.locator('[data-flat-disclosure=model-template]');await disclosure.locator(':scope>summary').click();await choose('[data-settings-field="model.temperatureMode"]','set');ok(await disclosure.evaluate(n=>n.open),'advanced stays open through select rerender');await field('model.temperature').fill('0');await field('model.maxTokens').fill('768');await save();eq(await state(()=>window.LunaDesign.state.settings.saved.models.local.temperature),'0','zero is a real explicit temperature');eq(await state(()=>window.LunaDesign.state.settings.saved.models.local.maxTokens),'768','output length draft saves independently');await capture('08-advanced-model-parameters','#settings-window');
   await page.locator('[data-flat-disclosure=connection-keys]>summary').click();eq(await page.locator('[data-settings-field="keyPool.enabled"]').count(),1,'explicit key pool remains available');eq(await page.locator('[data-settings-action=test-key]').count(),await state(()=>window.LunaDesign.state.settings.keyPool.keys.length),'each key retains independent test action');
   eq(await page.locator('[data-flat-purpose=grammar]').count(),1,'grammar remains a separate purpose');eq(await page.locator('[data-flat-purpose=word]').count(),1,'word remains a separate purpose');
   const wordBefore=await state(()=>window.LunaDesign.state.settings.draft.bindings.word.historyCount);await field('binding.historyCount','[data-flat-purpose=grammar]').fill('2');eq(await state(()=>window.LunaDesign.state.settings.draft.bindings.word.historyCount),wordBefore,'grammar context editing does not affect word context');
   await action('flat-test-purpose','[data-flat-purpose=grammar]').click();await page.waitForFunction(()=>Object.values(window.LunaDesign.state.settings.tests).some(t=>t.target==='grammar'));eq(await state(()=>Object.values(window.LunaDesign.state.settings.tests).filter(t=>t.target==='word').length),0,'grammar sample does not mark word passed');await capture('09-independent-learning','#settings-window');
  });
  await check('FL-05',async()=>{
   await open('window-preview');eq(await page.locator('#main-content [role=tab]').count(),0,'session has no mutually exclusive tabs');eq(await page.locator('[data-flat-session-section]').count(),3,'all three data regions are mounted together');
   ok(await section('reading').isVisible(),'body history is directly visible');ok(await section('texts').locator(':scope>summary').isVisible(),'all extraction summary visible');ok(await section('logs').locator(':scope>summary').isVisible(),'Hook summary visible');await capture('10-session-three-summaries','#main-window');
   await expand('texts');await expand('logs');ok(await section('texts').locator('.rw-records').isVisible(),'expanding Hook leaves extraction mounted');ok(await section('reading').isVisible(),'body remains mounted when both diagnostics expand');
   const data=await state(()=>window.LunaReviewGaps.sessions.get(window.LunaDesign.state.session.sessionId));eq(await section('texts').locator('.rw-text-entry').count(),Math.min(12,data.records.length),'extraction rows follow the independent raw data set');ok(data.records.some(r=>!r.selected),'fixture includes unselected text that is not body history');await capture('11-session-expanded','#main-window');
  });
  await check('FL-06',async()=>{
   await open('window-preview');await advance(18);await expand('texts');await expand('logs');const firstSource=await state(()=>window.LunaDesign.game().streams[0].id);
   await choose('[data-flat-session-section=texts] [data-flat-field=rw-session-filter]',firstSource);await listField('texts','rw-session-search').fill('。');await section('texts').locator('[data-flat-list-action=load-more]').click();
   const before=await state(()=>({...window.LunaFlatDesign.panels.get('texts')})),streamBefore=await state(()=>[...window.LunaDesign.state.session.selectedStreams]);
   await choose('[data-flat-session-section=logs] [data-flat-field=rw-session-filter]','warning');await choose('[data-flat-session-section=logs] [data-flat-field=rw-log-scope]','all');await listField('logs','rw-session-search').fill('模块');
   eq(await listField('texts','rw-session-search').inputValue(),'。','Hook query does not erase extraction query');eq(await listField('texts','rw-session-filter').inputValue(),firstSource,'Hook level does not replace extraction source');eq(await state(()=>window.LunaFlatDesign.panels.get('texts').limit),before.limit,'Hook filtering does not reset extraction page');eq(await state(()=>window.LunaDesign.state.session.selectedStreams),streamBefore,'browsing records does not select another Hook stream');
   await listField('reading','rw-session-search').fill('風景');eq(await listField('logs','rw-session-search').inputValue(),'模块','body query leaves Hook query intact');eq(await listField('texts','rw-session-search').inputValue(),'。','body query leaves extraction query intact');await capture('12-independent-filter-pagination','#main-window');
  });
  await check('FL-07',async()=>{
   await open('window-preview');await advance(28);await expand('texts');await expand('logs');const data=await state(()=>window.LunaReviewGaps.sessions.get(window.LunaDesign.state.session.sessionId));
   ok(data.records.length>24,'real prototype actions create more than two display pages');eq(await section('texts').locator('.rw-text-entry').count(),12,'first extraction page is bounded without truncating data');const full=await exportData('texts');eq(full.records.length,data.records.length,'complete export covers every received raw event');eq(full.records.map(r=>r.sequence),data.records.map(r=>r.sequence),'export preserves raw sequence order');
   const sourceId=data.records[0].streamId;await choose('[data-flat-session-section=texts] [data-flat-field=rw-session-filter]',sourceId);await listField('texts','rw-session-search').fill('。');const filtered=await exportData('texts','filtered');eq(filtered.records.map(r=>r.sequence),data.records.filter(r=>r.streamId===sourceId&&r.source.includes('。')).map(r=>r.sequence),'filtered export searches complete raw set, not loaded rows');
   const body=await exportData('reading');eq(body.records.length,data.records.filter(r=>r.selected).length,'body export excludes unselected sources');await choose('[data-flat-session-section=logs] [data-flat-field=rw-log-scope]','all');const logs=await exportData('logs');eq(logs.records.length,data.logs.length,'all Hook events export retains raw-noise facts');ok(logs.records.some(r=>r.facts?.address),'native installation facts remain traceable');ok(logs.records.some(r=>r.level==='warning'),'warning facts are retained');await capture('13-complete-filtered-export','#main-window');
   await fixture('session-integrity-gap',()=>{const a=window.LunaDesign;a.state.session.integrityLost=true;a.render();});ok((await section('texts').innerText()).includes('完整性待确认'),'gap is visible instead of claiming complete reception');eq(await state(()=>window.LunaReviewGaps.sessions.get(window.LunaDesign.state.session.sessionId).records.length),data.records.length,'gap warning preserves already received events');await capture('14-integrity-gap','#main-window');
  });
  await check('FL-08',async()=>{
   await open('window-preview');await advance(12);await expand('texts');await expand('logs');const preview=await page.locator('.rw-preview').getAttribute('data-preview-id');
   const list=section('texts').locator('.rw-records');await list.hover();await page.mouse.wheel(0,-180);await page.waitForTimeout(80);
   eq(await follow('texts').getAttribute('aria-pressed'),'false','wheel immediately exposes paused following');ok((await follow('texts').innerText()).includes('回到最新'),'paused label offers resume');eq(await follow('logs').getAttribute('aria-pressed'),'true','Hook continues following independently');
   const scroll=await list.evaluate(node=>node.scrollTop);await advance(1);ok((await follow('texts').innerText()).includes('条新内容'),'new raw events have a visible independent count');eq(await follow('texts').getAttribute('aria-pressed'),'false','new data does not resume paused reader');eq(await list.evaluate(node=>node.scrollTop),scroll,'new data preserves paused list scroll');eq(await page.locator('.rw-preview').getAttribute('data-preview-id'),preview,'new events preserve preview target identity');
   const captured=await page.locator('.rw-preview-time time').getAttribute('title');await listField('logs','rw-session-search').fill('引擎');eq(await page.locator('.rw-preview-time time').getAttribute('title'),captured,'filtering does not invent a new capture timestamp');await section('texts').locator(':scope>summary').click();await section('texts').locator(':scope>summary').click();eq(await follow('texts').getAttribute('aria-pressed'),'false','collapse and expand preserves paused state');await follow('texts').click();eq(await follow('texts').getAttribute('aria-pressed'),'true','resume command really resumes its own stream');await capture('15-independent-follow-preview','#main-window');
  });
  await check('FL-09',async()=>{
   await open();eq(await page.locator('#settings-content [data-settings-field=testOutcome]').count(),0,'simulation result controls are outside product settings');eq(await page.locator('.review-bar #flat-review-test-outcome').count(),1,'external review outcome control is available');
   await edit('local');await choose('#flat-review-test-outcome','auth');const before=await selected();await action('test').click();await page.waitForFunction(()=>window.LunaDesign.state.settings.testState==='idle');ok(await state(()=>Object.values(window.LunaDesign.state.settings.tests).some(t=>t.status==='fail'&&t.error==='认证失败')),'external failure control produces a scoped failure record');eq(await selected(),before,'failure simulation cannot choose or enable another service');
   await fixture('empty-config-directory',()=>{const a=window.LunaDesign;a.state.settings.saved.models={};a.state.settings.saved.localEngines={};a.state.settings.draft.models={};a.state.settings.draft.localEngines={};a.state.settings.selectedModel='';a.state.settings.selectedConnection='';a.render();});eq(await page.locator('[data-flat-section=saved-services] [data-flat-service]').count(),0,'empty saved directory does not show template catalog as resources');ok((await page.locator('[data-flat-section=saved-services]').innerText()).includes('尚无已保存服务'),'empty state explains configuration recovery');ok(await action('flat-add-service').isVisible(),'empty state retains adding entry');await capture('16-settings-empty','#settings-window');
  });
  await check('FL-10',async()=>{
   await open('settings','light');eq(await page.locator('html').getAttribute('data-theme'),'light','light appearance is actually rendered');await capture('17-settings-light','#settings-window');
   const summary=page.locator('[data-flat-disclosure=model-template]>summary');await summary.focus();await page.keyboard.press('Enter');ok(await summary.locator('..').evaluate(node=>node.open),'Enter opens focused advanced disclosure');await page.keyboard.press('Tab');ok(await state(()=>document.activeElement!==document.body),'Tab reaches a real control');await page.keyboard.press('Shift+Tab');ok(await state(()=>document.activeElement!==document.body),'reverse traversal retains focus');
   await page.locator('#settings-content [data-service-action=return]').first().click();await page.keyboard.press('Escape');eq(await page.locator('#translation-service-picker').isVisible(),false,'Escape dismisses shared picker');
   await open('window-preview','light');await page.setViewportSize({width:1024,height:900});ok(await state(()=>document.documentElement.scrollWidth<=innerWidth+1),'1024 layout has no document horizontal overflow');ok(await section('texts').locator(':scope>summary').isVisible(),'narrow extraction entry remains reachable');ok(await section('logs').locator(':scope>summary').isVisible(),'narrow Hook entry remains reachable');await capture('18-session-1024');
   await page.evaluate(()=>{document.documentElement.style.zoom='200%';});await page.locator('#main-content').evaluate(node=>node.scrollTop=0);ok(await section('texts').locator(':scope>summary').count()===1,'200 percent retains extraction entry');ok(await section('logs').locator(':scope>summary').count()===1,'200 percent retains Hook entry');await section('texts').locator(':scope>summary').focus();eq(await state(()=>document.activeElement?.parentElement.dataset.flatSessionSection),'texts','200 percent entry is keyboard focusable');await capture('19-session-200-percent');
  });
  eq(report.page_errors,[],'no page errors across all scenarios');eq(report.console_errors,[],'no console errors across all scenarios');eq(report.network_requests,[],'all design scenarios stay offline');
  for(const [name,hash]of Object.entries(report.source_hashes))assert.equal(sha(fs.readFileSync(path.join(root,name))),hash,'verification must not mutate input: '+name);
  if(report.checks.length===10&&report.checks.every(check=>check.status==='passed')&&report.assertions>=40)report.status='passed';else process.exitCode=1;
 }catch(error){report.error=error.stack||error.message;console.error(error);process.exitCode=1;}
 finally{if(browser)await browser.close();if(outputOwned)fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({status:report.status,assertions:report.assertions,checks:report.checks.length,screenshots:report.screenshots.length,product_tests_executed:false}));}
})();
