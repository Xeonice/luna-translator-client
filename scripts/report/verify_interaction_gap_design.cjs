/* Six-gap design QA only. This never runs native capture, a game, Hook, a
 * tokenizer, or global shortcut registration. Approved references stay intact. */
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const strictAssert = require('node:assert/strict');
let activeCheck = null;
const serialise = value => { try { return JSON.parse(JSON.stringify(value, (_key,item)=>typeof item==='bigint'?String(item):typeof item==='function'?String(item):item instanceof RegExp?String(item):item)); } catch { return String(value); } };
const assert = new Proxy(strictAssert, { get(target,key) { const fn=target[key]; if(typeof fn!=='function'||key==='AssertionError')return fn; return (...args)=>{const result=Reflect.apply(fn,target,args); if(activeCheck){activeCheck.assertion_count++;activeCheck.observations.push({kind:'assertion',method:String(key),assertion:String(key),actual:serialise(args[0]),expected:serialise(args[1])??null});}return result;}; } });
const observe = (dimension,value) => { if(!activeCheck)throw new Error('Observation outside actual check');activeCheck.observations.push({kind:'observation',dimension,value:serialise(value)}); };
const independentlyHashManifest = () => {
  const manifestPath=process.env.LUNA_DESIGN_SOURCE_MANIFEST_JSON;
  if(!manifestPath)return {};
  const manifest=JSON.parse(fs.readFileSync(manifestPath,'utf8'));
  return Object.fromEntries(Object.keys(manifest.files).sort().map(relative=>{const full=path.resolve(relative);if(!full.startsWith(process.cwd()+path.sep))throw new Error('Source manifest escapes repository');return [relative,hash(full)];}));
};
const { pathToFileURL } = require('node:url');
const { chromium } = require(path.resolve(__dirname, '../../node_modules/playwright-core'));

const source = path.resolve('docs/design/first-product-v1');
const rootOutput = path.resolve(process.argv[2] || '.build/design-six-gaps-20261019/qa');
const runId = new Date().toISOString().replace(/[:.]/g, '-');
const output = path.join(rootOutput, runId);
const checks = [], captures = [], consoleErrors = [], requests = [], excluded = [];
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const sources = () => Object.fromEntries(fs.readdirSync(source).filter(name => fs.statSync(path.join(source,name)).isFile()&&(name==='index.html'||/\.(?:js|css)$/.test(name))).sort().map(name => [name,hash(path.join(source,name))]));
const missing = reason => { const error = new Error(reason); error.notRun = true; throw error; };
let page, context;

/* RW_CAPABILITIES_BEGIN */
const DESIGN_CAPABILITIES = {
  "select_option_spacing_and_single_pointer_keyboard_highlight": {
    "dimensions": [
      "select-pointer-keyboard-single-highlight"
    ]
  },
  "select_scrolling_keeps_option_height_and_spacing": {
    "dimensions": [
      "select-internal-long-scroll"
    ]
  },
  "preview_identity_and_source_label": {
    "dimensions": []
  },
  "preview_explicit_enable": {
    "dimensions": []
  },
  "preview_pause_freezes_frame_then_resume": {
    "dimensions": []
  },
  "preview_permission_unavailable_exit": {
    "dimensions": []
  },
  "hud_pointer_drag_and_control_exclusion": {
    "dimensions": []
  },
  "hud_reset_position_and_desktop_bounds": {
    "dimensions": []
  },
  "hud_shortcuts_match_configuration_and_restore_position": {
    "dimensions": []
  },
  "hud_shortcut_edit_does_not_trigger": {
    "dimensions": []
  },
  "hud_shortcut_conflict_retains_effective_binding": {
    "dimensions": []
  },
  "hud_more_menu_persists_after_pointer_release": {
    "dimensions": []
  },
  "hud_only_reading_region_scrolls": {
    "dimensions": []
  },
  "custom_select_keyboard_cancel_commit_disabled_long": {
    "dimensions": []
  },
  "all_product_selects_are_authored": {
    "dimensions": []
  },
  "select_empty_loading_error_states": {
    "dimensions": [
      "select-empty-loading-error"
    ]
  },
  "session_all_streams_search_full_source_and_follow": {
    "dimensions": []
  },
  "session_texts_more_than_two_pages_preserve_sequence": {
    "dimensions": [
      "session-full-paging"
    ]
  },
  "session_browsing_stops_follow_and_counts_new": {
    "dimensions": [
      "session-paused-visible-sequence"
    ]
  },
  "session_export_scope_preview": {
    "dimensions": []
  },
  "hook_logs_stage_search_filter_and_pagination": {
    "dimensions": []
  },
  "hook_only_events_preserve_browsing_and_count_new": {
    "dimensions": []
  },
  "session_identity_isolation": {
    "dimensions": []
  },
  "ruby_independent_of_segmentation_and_original_reconstruction": {
    "dimensions": []
  },
  "ruby_unknown_and_lexical_failure_keep_original": {
    "dimensions": []
  },
  "hud_translucency_and_reduced_transparency": {
    "dimensions": []
  },
  "component_buttons_icons_and_visible_state_list": {
    "dimensions": []
  },
  "hud_toolbar_reachable_at_560_and_500_without_menu_duplicates": {
    "dimensions": [
      "hud-toolbar-500-560"
    ]
  },
  "hud_direct_actions_operate_on_current_sentence": {
    "dimensions": []
  },
  "lexical_preferences_all_eight_combinations": {
    "dimensions": [
      "lexical-000",
      "lexical-001",
      "lexical-010",
      "lexical-011",
      "lexical-100",
      "lexical-101",
      "lexical-110",
      "lexical-111"
    ]
  },
  "select_all_product_entrypoints_and_state_matrix": {
    "dimensions": [
      "select-all-entrypoints"
    ]
  },
  "preview_switch_cancel_and_stale_frame_identity": {
    "dimensions": [
      "preview-cancel-stale-frame"
    ]
  },
  "no_console_errors_or_network": {
    "dimensions": []
  },
  "source_stable_during_run": {
    "dimensions": []
  }
};
/* RW_CAPABILITIES_END */

async function main() {
  fs.mkdirSync(output, { recursive: true });
  const startedAt=new Date().toISOString();
  const before = sources(),fullBefore=independentlyHashManifest();
  const browser = await chromium.launch({ executablePath: process.env.REPORT_BROWSER_EXECUTABLE || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--disable-background-networking'] });
  context = await browser.newContext({ offline: true, viewport: { width:1600,height:1100 }, reducedMotion:'reduce',permissions:['clipboard-read','clipboard-write'] });
  page = await context.newPage(); page.setDefaultTimeout(5000);
  page.on('pageerror', error => consoleErrors.push(error.stack || error.message));
  page.on('console', message => { if (message.type()==='error') consoleErrors.push(message.text()); });
  page.on('request', request => { if (/^https?:/.test(request.url())) requests.push(request.url()); });
  const open = async scenario => {
    await page.goto(pathToFileURL(path.join(source,'index.html')).href + '?scenario=' + encodeURIComponent(scenario));
    await page.evaluate(() => document.fonts.ready);
    await page.waitForFunction(() => window.LunaDesign && window.LunaReviewGaps && window.LunaControls);
    await page.waitForFunction(() => [...document.images].filter(image=>image.offsetParent).every(image=>image.complete && image.naturalWidth>0));
    // Adversarial runs mutate actual rendered geometry, never the assertion result.
    if(process.env.LUNA_DESIGN_MUTATION==='zero-select-gap')await page.addStyleTag({content:'.luna-select-panel {gap:0 !important}'});
    if(process.env.LUNA_DESIGN_MUTATION==='hide-ruby')await page.addStyleTag({content:'#hud-source rt {display:none !important}'});
  };
  const state = fn => page.evaluate(fn);
  const capture = async (name, selector, description) => {
    const file = name + '.png';
    await page.locator(selector).screenshot({ path:path.join(output,file),animations:'disabled' });
    captures.push({file,title:description,selector,description,sha256:hash(path.join(output,file)),check_ids:activeCheck?[activeCheck.id]:[]});
    if(activeCheck)activeCheck.capture_ids.push(name);
  };
  const check = async (id, requirements, action) => {
    if(process.env.LUNA_DESIGN_ONLY_CHECK && process.env.LUNA_DESIGN_ONLY_CHECK!==id)return;
    const item={id,requirements,layer:'design',status:'running',started:new Date().toISOString(),assertion_count:0,observations:[],capture_ids:[],dimensions:DESIGN_CAPABILITIES[id].dimensions};
    activeCheck=item;
    try { await action(); if(item.assertion_count===0)throw new Error('No actual assertions executed');item.status='passed'; }
    catch(error) {
      item.status=error.notRun?'not_run':'failed';item.reason=error.stack || error.message;
      try { await capture('failure-' + id,'#desktop',item.reason); } catch(captureError) { item.capture_error=captureError.message; }
    } finally {item.completed=new Date().toISOString();checks.push(item);activeCheck=null;}
  };
  const box = selector => page.locator(selector).boundingBox();
  const samePosition = (a,b) => { assert.ok(a&&b); assert.ok(Math.abs(a.x-b.x)<1 && Math.abs(a.y-b.y)<1,JSON.stringify({before:a,after:b})); };
  const rawText = () => page.locator('#hud-source').evaluate(node=>{const copy=node.cloneNode(true);copy.querySelectorAll('rt,rp,.hud-language-note').forEach(n=>n.remove());copy.querySelectorAll('br').forEach(n=>n.replaceWith(document.createTextNode('\n')));return copy.textContent;});
  const action = name => page.locator('[data-rw-action="'+name+'"]');
  const choose = async (id,value) => {
    await page.locator('[data-select-for="'+id+'"]').click();
    await page.locator('.luna-select-panel [role=option][data-value="'+value+'"]').click();
  };
  const inspectPreviewDetails = async (preview,{name='session',expected=[]}={}) => {
    const trigger=preview.locator('[data-rw-action="preview-details"]'),identity=await preview.getAttribute('data-preview-id');
    const sessionBefore=await state(()=>JSON.parse(JSON.stringify(LunaDesign.state.session)));
    await trigger.focus();await trigger.press('Enter');
    const dialog=page.locator('#rw-preview-details'),body=page.locator('#rw-preview-details-body');
    assert.equal(await dialog.isVisible(),true);assert.equal(await dialog.getAttribute('data-preview-id'),null);
    assert.equal(await trigger.getAttribute('aria-expanded'),'true');
    const text=await body.innerText();
    for(const field of ['完整游戏名称','原始窗口标题','环境 / Bottle','实际 EXE','完整路径','Windows 本次运行身份','宿主本次运行身份','原生窗口身份','目标身份摘要 / hash','原采集时间','阶段','API','原始原因','原始诊断全文','隐私与保存'])assert.ok(text.includes(field),field);
    assert.ok(text.includes(identity));for(const value of expected)assert.ok(text.includes(value),'Full preview fact missing: '+value);
    const privacy=await state(()=>LunaDesignData.previewDesign.privacy);assert.ok(text.includes(privacy));
    await page.locator('[data-rw-action="copy-preview-details"]').click();
    await page.waitForFunction(()=>document.getElementById('rw-preview-copy-status').textContent==='已复制完整详情');
    const copied=await state(()=>navigator.clipboard.readText());assert.equal(copied,text);
    await body.locator('section').last().scrollIntoViewIfNeeded();
    const scroll=await body.evaluate(node=>({top:node.scrollTop,height:node.clientHeight,scrollHeight:node.scrollHeight,overflowX:node.scrollWidth>node.clientWidth}));
    assert.ok(scroll.top>0);assert.equal(scroll.overflowX,false);
    await body.evaluate(node=>node.scrollTop=0);await dialog.locator('button').first().focus();
    for(const key of ['Shift+Tab','Tab','Tab','Tab','Tab','Tab']){await page.keyboard.press(key);assert.equal(await state(()=>!!document.activeElement.closest('#rw-preview-details')),true);}
    const beforeShortcut=await state(()=>({hud:LunaDesign.state.windows.hud,settings:LunaDesign.state.windows.settings,sheet:LunaDesign.state.sheet}));
    await page.keyboard.press('Alt+Meta+h');await page.keyboard.press('Meta+,');
    assert.deepEqual(await state(()=>({hud:LunaDesign.state.windows.hud,settings:LunaDesign.state.windows.settings,sheet:LunaDesign.state.sheet})),beforeShortcut);
    const preservedText=await body.innerText();await state(()=>LunaDesign.render());
    assert.equal(await dialog.isVisible(),true);assert.equal(await body.innerText(),preservedText);
    await capture('compact-details-'+name,'#rw-preview-details','完整字段、复制、详情滚动及键盘焦点；打开时快照不被整窗render替换');
    await page.keyboard.press('Escape');assert.equal(await dialog.isVisible(),false);
    const current=preview.locator('[data-rw-action="preview-details"]');assert.equal(await current.evaluate(node=>document.activeElement===node),true);assert.equal(await current.getAttribute('aria-expanded'),'false');
    assert.deepEqual(await state(()=>JSON.parse(JSON.stringify(LunaDesign.state.session))),sessionBefore);
    observe('compact-preview-details',{name,identity,allFactsCopied:true,copiedLength:copied.length,scroll,escapeReturnsFocus:true,shortcutsIsolated:true});
  };
  try {
    await check('select_option_spacing_and_single_pointer_keyboard_highlight',['RW-13','RW-14','RW-15','RW-31'],async()=>{
      await open('components');
      const trigger=page.locator('#showcase-normal--trigger');
      await trigger.click();
      const options=page.locator('.luna-select-panel [role=option]');
      const metrics=()=>options.evaluateAll(nodes=>nodes.map(node=>{const rect=node.getBoundingClientRect(),style=getComputedStyle(node);return {value:node.dataset.value,top:rect.top,bottom:rect.bottom,height:rect.height,active:node.classList.contains('is-active'),painted:style.backgroundColor!=='rgba(0, 0, 0, 0)'&&style.backgroundColor!=='transparent'};}));
      await options.nth(1).hover();
      let rows=await metrics();
      assert.ok(rows.every((row,index)=>row.height>=30&&(index===0||row.top-rows[index-1].bottom>=1.99)),'Every option including wrapped labels has a separate highlight rectangle');
      assert.deepEqual(rows.filter(row=>row.active||row.painted).map(row=>row.value),['unselected'],'Pointer moves the single highlight; selection check is independent');
      await capture('11b-select-option-spacing','.luna-select-panel','相邻选项有独立间隔；鼠标移动后只有一个活动高亮');
      await options.nth(3).hover(); rows=await metrics();
      assert.equal(rows[3].painted,false,'Disabled option never receives a hover fill');
      assert.equal(rows.filter(row=>row.active||row.painted).length,1);
      await trigger.press('ArrowDown'); rows=await metrics();
      assert.deepEqual(rows.filter(row=>row.active||row.painted).map(row=>row.value),['long'],'Keyboard owns the single highlight even while pointer remains over a different row');
      await trigger.press('Escape');
      assert.equal(await page.locator('#showcase-normal').inputValue(),'body','Highlight navigation and cancellation do not commit a selection');
      observe('select-pointer-keyboard-single-highlight',{rows,cancelledValue:await page.locator('#showcase-normal').inputValue()});
    });
    await check('select_scrolling_keeps_option_height_and_spacing',['RW-14','RW-15'],async()=>{
      await open('components');
      await state(()=>{const select=document.getElementById('showcase-normal');select.replaceChildren(...Array.from({length:24},(_,index)=>new Option(index%3===0?'较长的文本来源名称 · GameN7_Steam.exe · MAGES2 · 同一选择器中的完整说明 '+index:'文本来源 '+index,'source-'+index)));LunaControls.sync(select);});
      const trigger=page.locator('#showcase-normal--trigger');await trigger.click();
      const geometry=await page.locator('.luna-select-panel').evaluate(panel=>({scrollHeight:panel.scrollHeight,clientHeight:panel.clientHeight,rows:[...panel.querySelectorAll('[role=option]')].map(node=>{const r=node.getBoundingClientRect();return {top:r.top,bottom:r.bottom,height:r.height};})}));
      assert.ok(geometry.scrollHeight>geometry.clientHeight,'Long options scroll inside the panel');
      assert.ok(geometry.rows.every((row,index)=>row.height>=30&&(index===0||row.top-geometry.rows[index-1].bottom>=1.99)),'Scrollable flex layout cannot compress or overlap rows');
      await trigger.press('End');
      assert.ok(await page.locator('.luna-select-panel').evaluate(panel=>panel.scrollTop)>0);
      await capture('12c-select-scrolling-spacing','.luna-select-panel','长列表和多行选项保留行高与间距，只在菜单内部滚动');
      await trigger.press('Enter');assert.equal(await page.locator('#showcase-normal').inputValue(),'source-23');
      assert.equal(await page.locator('.luna-select-panel').count(),0);
      observe('select-internal-long-scroll',{geometry,committedValue:await page.locator('#showcase-normal').inputValue()});
    });
    await check('preview_identity_and_source_label',['RW-01','RW-06'],async()=>{
      await open('window-preview');
      let text=await page.locator('#window-preview').textContent();
      assert.match(text,/运行窗口预览/); assert.equal(await page.locator('#window-preview img').count(),1);
      assert.ok(await page.locator('#window-preview').getAttribute('data-preview-id'));
      assert.match(text,/画面时间|采集时间|更新/);
      assert.equal(await action('enable-preview').count(),0); text=await page.locator('#window-preview').textContent(); assert.match(text,/回放|离线|设计/);
      await capture('01-window-preview','#main-window','窗口身份、帧时间与离线来源标记；不是本机实时截图');
      const runtime=await state(()=>LunaDesign.state.session.runtimeTarget);
      await inspectPreviewDetails(page.locator('#window-preview'),{expected:[runtime.title,runtime.targetPath,String(runtime.windowsPid),String(runtime.hostPid),runtime.processStartedAt]});
      const modes=['available','paused','starting','permission','unavailable','exited','identity_error','long_title','long_error','japanese'],layoutCases=[];
      for(const width of [282,260,720,960])for(const mode of modes){
        await open('window-preview');
        if(mode==='paused')await action('pause-preview').click();else await page.locator('#review-preview').selectOption(mode);
        const preview=page.locator('#window-preview');
        // Only the design host geometry is varied here; the actual full-draft
        // sidebar remains separately captured above at its natural 262px width.
        await preview.evaluate((node,width)=>{node.style.cssText='position:fixed;left:24px;top:200px;width:'+width+'px;z-index:90';},width);
        const geometry=await preview.evaluate(node=>{const r=node.getBoundingClientRect(),image=node.querySelector('.rw-preview-image'),f=image.getBoundingClientRect(),meta=node.querySelector('.rw-preview-meta').getBoundingClientRect(),footer=node.querySelector('.rw-preview-actions').getBoundingClientRect();return{width:r.width,height:r.height,scrollWidth:node.scrollWidth,clientWidth:node.clientWidth,scrollHeight:node.scrollHeight,clientHeight:node.clientHeight,frameRatio:f.width/f.height,objectFit:node.querySelector('img')?getComputedStyle(node.querySelector('img')).objectFit:null,mainOverflow:getComputedStyle(node).overflow,guidanceFont:getComputedStyle(node.querySelector('.rw-preview-guidance')).fontSize,metaBottom:meta.bottom,footerTop:footer.y,headingHeight:node.querySelector('.rw-preview-heading').getBoundingClientRect().height,buttons:[...node.querySelectorAll('button')].map(n=>{const b=n.getBoundingClientRect(),hit=document.elementFromPoint(b.x+b.width/2,b.y+b.height/2);return{name:n.getAttribute('aria-label')||n.textContent,outside:b.x<r.x-.1||b.y<r.y-.1||b.right>r.right+.1||b.bottom>r.bottom+.1,hit:!!hit&&(n===hit||n.contains(hit))};})};});
        assert.equal(geometry.width,width);assert.equal(geometry.height,346);assert.ok(geometry.scrollWidth<=geometry.clientWidth+1);assert.ok(geometry.scrollHeight<=geometry.clientHeight+1);assert.equal(geometry.mainOverflow,'visible');assert.ok(Math.abs(geometry.frameRatio-16/9)<.015);assert.ok(geometry.objectFit===null||geometry.objectFit==='contain');assert.ok(geometry.metaBottom<=geometry.footerTop+.1);assert.ok(geometry.buttons.every(button=>!button.outside&&button.hit),JSON.stringify(geometry));assert.ok(['12px','13px'].includes(geometry.guidanceFont));
        if(mode==='identity_error')assert.equal(await preview.locator('img').count(),0);
        await capture('compact-'+mode+'-'+width,'#window-preview','紧凑共享组件 '+mode+' · 实际组件宽度 '+width+'，无裁按钮或缩放');
        layoutCases.push({mode,width,geometry});
        if(width===282&&['long_title','long_error','japanese'].includes(mode)){
          const fixture=await page.evaluate(mode=>LunaDesignData.previewDesign.stress[mode],mode);
          await inspectPreviewDetails(preview,{name:mode,expected:[fixture.title,fixture.raw].filter(Boolean)});
        }
      }
      assert.equal(layoutCases.length,40);observe('compact-preview-layouts',layoutCases);
      for(const theme of ['light','dark']){
        await open('window-preview');await page.locator('#review-theme').selectOption(theme);assert.equal(await page.locator('#window-preview').evaluate(node=>node.clientHeight>=344),true);await capture('compact-theme-'+theme,'#window-preview','共享预览沿用'+theme+'材质token');
      }
      const cdp=await context.newCDPSession(page);await cdp.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'}]});assert.equal(await page.locator('#window-preview').evaluate(node=>getComputedStyle(node).backdropFilter),'none');await capture('compact-theme-reduced','#window-preview','共享预览减少透明度回退');await cdp.detach();
    });
    await check('preview_explicit_enable',['RW-03'],async()=>{
      await open('window-preview');
      // Legacy check ID retained; 2026-10-21 user override replaces per-use opt-in with permission-aware automatic preview.
      assert.equal(await action('enable-preview').count(),0);
      assert.equal(await page.locator('#window-preview img').count(),1);
      assert.equal(await page.locator('#window-preview').getAttribute('data-preview-owner'),'session');
      for(const tab of ['logs','texts','reading']){await page.locator('[data-rw-tab="'+tab+'"]').click();assert.equal(await page.locator('#window-preview img').count(),1);}
      assert.match(await page.locator('#window-preview').textContent(),/回放|画面跟随/);
      await capture('02-preview-enabled','#main-window','权限具备时自动预览，跨会话页签常驻的离线回放');
    });
    await check('preview_pause_freezes_frame_then_resume',['RW-02','RW-04'],async()=>{
      await open('window-preview'); if(await action('enable-preview').count())assert.equal(await action('enable-preview').count(),0);
      const image=page.locator('#window-preview img'); assert.equal(await image.count(),1);
      const sameFrameTime=await page.locator('#window-preview .rw-preview-meta').textContent();
      const sameFrameIso=await state(()=>LunaReviewGaps.previews.get(LunaDesign.state.session.runtimeTarget.hostWindowId).updatedIso);
      await action('refresh-preview').click();assert.equal(await page.locator('#window-preview .rw-preview-meta').textContent(),sameFrameTime);assert.equal(await state(()=>LunaReviewGaps.previews.get(LunaDesign.state.session.runtimeTarget.hostWindowId).updatedIso),sameFrameIso,'Refreshing the same replay frame cannot forge a new timestamp');
      await action('pause-preview').click();
      const frozen=await image.getAttribute('src');
      const time=await page.locator('#window-preview .rw-preview-meta').textContent();
      await page.locator('#review-next').click();
      assert.equal(await image.getAttribute('src'),frozen); assert.equal(await page.locator('#window-preview .rw-preview-meta').textContent(),time);
      assert.match(await page.locator('#window-preview').textContent(),/已暂停/);
      await capture('03-preview-paused','#main-window','游戏回放推进后暂停帧和时间保持');
      await action('pause-preview').click();
      assert.notEqual(await image.getAttribute('src'),frozen);
      await page.locator('#window-preview [data-rw-action="preview-details"]').click();
      const openFacts=await page.locator('#rw-preview-details-body').innerText(),unitBefore=await state(()=>LunaDesign.state.current.id);
      await state(()=>LunaDesign.advance());
      assert.notEqual(await state(()=>LunaDesign.state.current.id),unitBefore);assert.equal(await page.locator('#rw-preview-details').isVisible(),true);assert.equal(await page.locator('#rw-preview-details-body').innerText(),openFacts,'A new paired frame cannot retarget or replace an opened fact snapshot');
      await page.keyboard.press('Escape');assert.equal(await page.locator('#window-preview [data-rw-action="preview-details"]').evaluate(node=>node===document.activeElement),true);
    });
    await check('preview_permission_unavailable_exit',['RW-03','RW-04'],async()=>{
      await open('window-preview');
      const session=await state(()=>LunaDesign.state.session.sessionId);
      assert.equal(await action('enable-preview').count(),0);
      await page.locator('#review-preview').selectOption('permission');
      assert.match(await page.locator('#window-preview').textContent(),/权限|授权/);
      assert.equal(await state(()=>LunaDesign.state.session.sessionId),session);
      assert.equal(await state(()=>LunaDesign.state.session.status),'connected');
      await capture('04-preview-permission','#main-window','未授权不会阻断Hook会话的设计状态');
      await action('capture-permission').click(); assert.equal(await page.locator('#app-dialog').isVisible(),true);
      assert.match(await page.locator('#app-dialog').textContent(),/不会请求权限|离线设计/); await page.locator('#dialog-cancel').click();
      await page.locator('#review-preview').selectOption('unavailable'); assert.match(await page.locator('#window-preview').textContent(),/无法预览|不可用/);
      await capture('05-preview-unavailable','#main-window','无法采集的独立降级状态');
      await page.locator('#review-preview').selectOption('exited'); assert.match(await page.locator('#window-preview').textContent(),/关闭|退出/);
      assert.equal(await action('refresh-preview').isDisabled(),true);
      await capture('06-preview-exited','#main-window','退出状态停止刷新');
    });
    await check('hud_pointer_drag_and_control_exclusion',['RW-07'],async()=>{
      await open('hud-plain'); const beforeBox=await box('#hud-window'); const handle=await box('.hud-drag-handle');
      await page.mouse.move(handle.x+handle.width/2,handle.y+handle.height/2); await page.mouse.down(); await page.mouse.move(handle.x+handle.width/2+83,handle.y+handle.height/2-47,{steps:8}); await page.mouse.up();
      const afterBox=await box('#hud-window'); assert.ok(Math.abs(afterBox.x-beforeBox.x-83)<2); assert.ok(Math.abs(afterBox.y-beforeBox.y+47)<2);
      const button=await box('#hud-pause'); await page.mouse.move(button.x+button.width/2,button.y+button.height/2); await page.mouse.down(); await page.mouse.move(button.x+button.width/2+50,button.y+button.height/2-30,{steps:4}); await page.mouse.up();
      samePosition(afterBox,await box('#hud-window')); await capture('07-hud-dragged','#desktop','真实浏览器pointer拖动；按钮排除拖动区域');
    });
    await check('hud_reset_position_and_desktop_bounds',['RW-08'],async()=>{
      await open('hud-plain'); const original=await box('#hud-window'); const handle=await box('.hud-drag-handle');
      await page.mouse.move(handle.x+handle.width/2,handle.y+handle.height/2); await page.mouse.down(); await page.mouse.move(1590,100,{steps:10}); await page.mouse.up();
      const moved=await box('#hud-window'),desktop=await box('#desktop'); assert.ok(moved.x>=desktop.x-1); assert.ok(moved.x+moved.width<=desktop.x+desktop.width+1); assert.ok(moved.y>=desktop.y);
      await page.locator('#hud-more').click(); await page.locator('#hud-menu [data-rw-action="reset-hud"]').click(); samePosition(original,await box('#hud-window'));
      await capture('07b-hud-position-reset','#desktop','拖动受页面工作区约束，显式重置恢复字幕位置');
    });
    await check('hud_shortcuts_match_configuration_and_restore_position',['RW-08','RW-09'],async()=>{
      await open('hud-plain'); const beforeBox=await box('#hud-window');
      await page.keyboard.press('Alt+Meta+h'); assert.equal(await page.locator('#hud-window').isVisible(),false);
      assert.equal(await state(()=>LunaDesign.state.session.paused),false);
      await page.keyboard.press('Alt+Meta+h'); assert.equal(await page.locator('#hud-window').isVisible(),true); samePosition(beforeBox,await box('#hud-window'));
      await page.keyboard.press('Alt+Meta+p'); assert.equal(await state(()=>LunaDesign.state.session.paused),true);
      assert.equal(await page.locator('#hud-window').isVisible(),true); await page.keyboard.press('Alt+Meta+p'); assert.equal(await state(()=>LunaDesign.state.session.paused),false);
      await capture('08-hud-shortcuts','#hud-window','页面快捷键与状态一致；不证明系统全局注册');
    });
    await check('hud_shortcut_edit_does_not_trigger',['RW-10','RW-11'],async()=>{
      await open('components'); await action('open-reading-settings').click();
      const field=page.locator('[data-settings-field="display.hideShortcut"]'); assert.equal(await field.isVisible(),true);
      const visible=await state(()=>LunaDesign.state.windows.hud),paused=await state(()=>LunaDesign.state.session.paused);
      await field.focus(); await page.keyboard.press('Alt+Meta+h');
      assert.equal(await state(()=>LunaDesign.state.windows.hud),visible); assert.equal(await state(()=>LunaDesign.state.session.paused),paused);
      await capture('09-shortcut-settings','#settings-window','编辑快捷键时不触发产品动作');
    });
    await check('hud_shortcut_conflict_retains_effective_binding',['RW-10','RW-11'],async()=>{
      await open('components'); await action('open-reading-settings').click();
      const hide=page.locator('[data-settings-field="display.hideShortcut"]');
      const pause=page.locator('[data-settings-field="display.pauseShortcut"]');
      const effectiveBefore=await state(()=>LunaReviewGaps.activeShortcuts());
      await pause.fill(await hide.inputValue()); await pause.blur();
      const apply=action('apply-shortcuts'); if(!await apply.count())missing('尚无快捷键明确应用与冲突失败状态。');
      await apply.click(); assert.match(await page.locator('#rw-shortcut-status').textContent(),/冲突|重复|失败|同一组合/);
      assert.deepEqual(await state(()=>LunaReviewGaps.activeShortcuts()),effectiveBefore);
      assert.match(await page.locator('#rw-shortcut-status').textContent(),/保留|生效/);
      await capture('09b-shortcut-conflict','#settings-window','快捷键草稿冲突，原生效项保留');
      await page.locator('[data-action="close-settings"]').click();
      await page.keyboard.press('Alt+Meta+p'); assert.equal(await state(()=>LunaDesign.state.session.paused),true);
    });
    await check('hud_more_menu_persists_after_pointer_release',['RW-07','RW-32'],async()=>{
      await open('hud-plain'); const beforeBox=await box('#hud-window'); await page.locator('#hud-more').click();
      assert.equal(await page.locator('#hud-menu').isVisible(),true); samePosition(beforeBox,await box('#hud-window'));
      await page.waitForTimeout(120); assert.equal(await page.locator('#hud-menu').isVisible(),true);
      await capture('10-hud-more-menu','#desktop','点击松开后更多菜单保持展开');
      await page.keyboard.press('Escape'); assert.equal(await page.locator('#hud-menu').isVisible(),false);
    });
    await check('hud_only_reading_region_scrolls',['RW-30'],async()=>{
      await open('components'); await action('open-reading-settings').click();
      const fieldId=await page.locator('[data-settings-field="preferences.fontScale"]').getAttribute('id'); await choose(fieldId,'2');
      await page.locator('[data-action="close-settings"]').click(); await page.locator('#main-window [data-action="close-main"]').click();
      const reading=page.locator('#hud-reading'); const dimensions=await reading.evaluate(n=>({height:n.clientHeight,scrollHeight:n.scrollHeight}));
      assert.ok(dimensions.scrollHeight>dimensions.height,'200% ruby and translation should exercise the actual internal scrollbar');
      const controlsBefore=await box('.hud-controls'),hudBefore=await box('#hud-window');
      await reading.hover(); await page.mouse.wheel(0,350); await page.waitForFunction(()=>document.getElementById('hud-reading').scrollTop>0);
      samePosition(controlsBefore,await box('.hud-controls')); samePosition(hudBefore,await box('#hud-window'));
      assert.equal(await page.locator('#hud-window').evaluate(n=>n.scrollTop),0);
      await capture('10b-hud-internal-scroll','#hud-window','200%注音长文只在正文区滚动，操作区和窗口保持位置');
    });
    await check('custom_select_keyboard_cancel_commit_disabled_long',['RW-13','RW-14','RW-15'],async()=>{
      await open('components'); const trigger=page.locator('#showcase-normal--trigger');
      await trigger.focus(); await page.keyboard.press('Space'); assert.equal(await trigger.getAttribute('aria-expanded'),'true');
      assert.equal(await page.locator('.luna-select-panel [role=option]').count(),4);
      await capture('11-custom-select-expanded','body','专用磨砂列表、选中勾记、长项与禁用项');
      await page.keyboard.press('ArrowDown'); await page.keyboard.press('Escape'); assert.equal(await page.locator('#showcase-normal').inputValue(),'body'); assert.equal(await trigger.evaluate(n=>n===document.activeElement),true);
      await trigger.click(); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter'); assert.equal(await page.locator('#showcase-normal').inputValue(),'unselected');
      await choose('showcase-normal','long'); assert.match(await trigger.getAttribute('title'),/GameN7_Steam/);
      await trigger.click(); const disabled=page.locator('.luna-select-panel [data-value="disabled"]'); assert.equal(await disabled.getAttribute('aria-disabled'),'true');
      const disabledBox=await disabled.boundingBox(); await page.mouse.click(disabledBox.x+disabledBox.width/2,disabledBox.y+disabledBox.height/2); assert.equal(await page.locator('#showcase-normal').inputValue(),'long');
      await page.keyboard.press('Escape'); assert.equal(await page.locator('#showcase-disabled--trigger').isDisabled(),true);
      await capture('12-select-long-selected','#main-window','完整长值可访问名称与禁用选择器');
    });
    await check('all_product_selects_are_authored',['RW-16'],async()=>{
      let audited=0;
      for(const scenario of ['components','settings','session-texts','session-logs','existing-library']) {
        await open(scenario);
        const controls=await page.locator('select').evaluateAll(nodes=>nodes.filter(n=>!n.closest('.review-bar')).map(n=>({id:n.id,hidden:n.hidden,ready:n.dataset.customSelect,display:getComputedStyle(n).display,trigger:!!n.nextElementSibling?.matches('[role=combobox]')})));
        controls.forEach(item=>{assert.equal(item.hidden,true,item.id);assert.equal(item.display,'none',item.id);assert.equal(item.ready,'ready',item.id);assert.equal(item.trigger,true,item.id);}); audited+=controls.length;
      }
      assert.ok(audited>=10,'Must inspect multiple real product entry points');
    });
    await check('select_empty_loading_error_states',['RW-14'],async()=>{
      await open('components');
      const empty=page.locator('#showcase-no-options--trigger'); if(!await empty.count())missing('尚无空列表、加载和失败的专用Select候选状态。');
      await empty.click(); assert.equal(await page.locator('.luna-select-panel [role=option]').count(),0);
      for(const key of ['Enter','Space','ArrowDown','ArrowUp','Home','End']) {await page.keyboard.press(key);assert.equal(await page.locator('.luna-select-panel [role=option]').count(),0);assert.equal(await page.locator('#showcase-no-options').inputValue(),'');}
      await capture('12b-select-empty','body','空列表的Enter/Space/方向键不报错、不伪造选项'); await page.keyboard.press('Escape'); assert.equal(await empty.evaluate(n=>n===document.activeElement),true);
      assert.equal(await page.locator('#showcase-loading--trigger').isDisabled(),true); assert.equal(await page.locator('#showcase-error--trigger').isDisabled(),true);
      await action('retry-showcase').click(); assert.match(await page.locator('#showcase-error-message').textContent(),/获取/);
      assert.equal(await page.locator('#showcase-error--trigger').isDisabled(),false); assert.equal(await page.locator('#showcase-error').getAttribute('data-select-state'),null);
      assert.equal(await page.locator('#showcase-error--trigger').getAttribute('data-select-state'),'ready'); assert.equal(await page.locator('#showcase-error--trigger').getAttribute('aria-invalid'),null);
      observe('select-empty-loading-error',{emptyValue:await page.locator('#showcase-no-options').inputValue(),loadingDisabled:await page.locator('#showcase-loading--trigger').isDisabled(),retryState:await page.locator('#showcase-error--trigger').getAttribute('data-select-state')});
    });
    await check('session_all_streams_search_full_source_and_follow',['RW-17','RW-18','RW-21'],async()=>{
      await open('session-texts');
      const all=await state(()=>LunaReviewGaps.sessions.get(LunaDesign.state.session.sessionId).records);
      assert.ok(all.length>0); assert.ok(all.some(r=>!r.selected),'Must expose unselected candidate text');
      const sessionId=await state(()=>LunaDesign.state.session.sessionId); assert.ok(all.every(r=>r.sessionId===sessionId));
      const unselected=all.find(r=>!r.selected); await choose('rw-session-filter',unselected.streamId);
      assert.ok(await page.locator('.rw-text-entry').count()>0); assert.match(await page.locator('.rw-records').textContent(),/未进入字幕|未选/);
      await page.locator('.rw-text-entry summary').first().click(); assert.match(await page.locator('.rw-records').textContent(),/会话|上下文/);
      await capture('13-session-unselected-source','#main-window','未选中来源和完整会话/流身份');
      await choose('rw-session-filter','all'); await page.locator('#rw-session-search').fill(unselected.source.slice(0,5));
      assert.ok(await page.locator('.rw-text-entry').count()>0); await page.locator('#rw-session-search').fill('未出现的搜索文字-qa-no-hit');
      assert.equal(await page.locator('.rw-text-entry').count(),0); assert.match(await page.locator('.rw-list-empty').textContent(),/没有匹配/);
      await page.locator('#rw-session-search').fill(''); assert.equal(await action('toggle-follow').getAttribute('aria-pressed'),'false','Searching should already pause following');
      await action('toggle-follow').click(); assert.equal(await action('toggle-follow').getAttribute('aria-pressed'),'true'); await action('toggle-follow').click(); assert.equal(await action('toggle-follow').getAttribute('aria-pressed'),'false');
    });
    await check('session_texts_more_than_two_pages_preserve_sequence',['RW-19','RW-20','RW-23'],async()=>{
      await open('session-texts');
      for(let i=0;i<30;i++)await page.locator('#review-next').click();
      if(await action('toggle-follow').getAttribute('aria-pressed')==='true')await action('toggle-follow').click();
      const total=await state(()=>LunaReviewGaps.sessions.get(LunaDesign.state.session.sessionId).records.length);
      assert.ok(total>24,'Need more than two 12-row pages');
      const seen=[]; for(let pageNumber=0;pageNumber<=Math.ceil(total/12);pageNumber++) { const seq=await page.locator('[data-extraction-sequence]').evaluateAll(nodes=>nodes.map(n=>Number(n.dataset.extractionSequence))); assert.ok(seq.length>0); seen.splice(0,seen.length,...seq); if(!await action('load-more').count())break; await action('load-more').click(); }
      assert.equal(seen.length,total); assert.equal(new Set(seen).size,total); assert.ok(seen.every((n,i)=>!i||n>seen[i-1]));
      await capture('14-session-paginated','#main-window','超过两页的会话提取可完整加载，序号单调且无重复');
      observe('session-full-paging',{total,sequences:seen});
    });
    await check('session_browsing_stops_follow_and_counts_new',['RW-21'],async()=>{
      await open('session-texts'); for(let i=0;i<16;i++)await page.locator('#review-next').click();
      const scroll=page.locator('#main-content'); await scroll.hover(); await page.mouse.wheel(0,-350);
      await page.waitForFunction(()=>LunaReviewGaps.view.follow===false);
      // Real wheel dispatch is asynchronous; measure data arrival after motion settles.
      await scroll.evaluate(async node=>{let previous=node.scrollTop,stable=0;for(let frame=0;frame<180&&stable<6;frame++){await new Promise(requestAnimationFrame);const current=node.scrollTop;stable=Math.abs(current-previous)<.1?stable+1:0;previous=current;}if(stable<6)throw new Error('Wheel scroll did not settle');});
      const sequences=()=>page.locator('[data-extraction-sequence]').evaluateAll(nodes=>nodes.map(n=>Number(n.dataset.extractionSequence)));
      const visibleBefore=await sequences(),scrollBefore=await scroll.evaluate(n=>n.scrollTop);
      await page.locator('#review-next').click();
      assert.deepEqual(await sequences(),visibleBefore,'Browsing must preserve the same rows, not just the row count');
      const scrollAfter=await scroll.evaluate(n=>n.scrollTop);assert.ok(Math.abs(scrollAfter-scrollBefore)<2,JSON.stringify({message:'Incoming data must not move the user reading position',scrollBefore,scrollAfter}));
      assert.ok(await state(()=>LunaReviewGaps.view.newCount)>0); assert.match(await action('toggle-follow').textContent(),/新内容/);
      await capture('14b-session-new-content','#main-window','上翻停止自动跟随，新提取只增加提示数量');
      await action('toggle-follow').click(); assert.equal(await state(()=>LunaReviewGaps.view.follow),true); assert.equal(await state(()=>LunaReviewGaps.view.newCount),0);
      observe('session-paused-visible-sequence',{visibleBefore,scrollBefore,resumed:await state(()=>LunaReviewGaps.view.follow)});
    });
    await check('session_export_scope_preview',['RW-22'],async()=>{
      await open('session-texts'); const exportButton=action('export-session'); if(!await exportButton.count())missing('尚无会话导出范围和隐私确认候选。');
      await exportButton.click(); assert.equal(await page.locator('#app-dialog').isVisible(),true);
      const exportText=await page.locator('#app-dialog').textContent(); assert.match(exportText,/会话|全部提取/); assert.match(exportText,/游戏原文/); assert.match(exportText,/路径/); assert.match(exportText,/保存位置|分享范围/);
      await capture('14c-session-export','#app-dialog','导出范围和台词隐私提示；不写真实游戏文件'); await page.locator('#dialog-cancel').click();
    });
    await check('hook_logs_stage_search_filter_and_pagination',['RW-17','RW-18','RW-19'],async()=>{
      await open('session-logs');
      assert.match(await page.locator('#hook-events').textContent(),/Hook 接入日志/);
      for(let i=0;i<14;i++)await page.locator('#review-next').click();
      await page.locator('#review-fault').selectOption('integrity');
      await page.locator('#hook-events [data-action="recover"]').click(); await page.locator('#dialog-confirm').click();
      assert.equal(await state(()=>LunaReviewGaps.view.logScope),'diagnostic');
      assert.equal(await page.locator('[data-hook-log]').filter({hasText:'收到文本事件 #'}).count(),0);
      await choose('rw-log-scope','all');
      const total=await state(()=>LunaReviewGaps.sessions.get(LunaDesign.state.session.sessionId).logs.length);
      assert.ok(total>24,'Actual recovery actions must produce more than two pages of events');
      if(await action('toggle-follow').getAttribute('aria-pressed')==='true')await action('toggle-follow').click();
      for(let i=0;i<8 && await action('load-more').count();i++)await action('load-more').click();
      assert.equal(await page.locator('[data-hook-log]').count(),total);
      await choose('rw-session-filter','warning'); assert.ok(await page.locator('[data-hook-log]').count()>0);
      assert.match(await page.locator('.rw-records').textContent(),/警告/);
      await capture('15-hook-warning-events','#main-window','Hook阶段事件与警告筛选');
      await page.locator('#rw-session-search').fill('不匹配的Hook关键字-qa'); assert.equal(await page.locator('[data-hook-log]').count(),0);
    });
    await check('hook_only_events_preserve_browsing_and_count_new',['RW-18','RW-21'],async()=>{
      await open('session-logs'); for(let i=0;i<15;i++)await page.locator('#review-next').click();
      const recordsBefore=await state(()=>LunaReviewGaps.sessions.get(LunaDesign.state.session.sessionId).records.length);
      await page.locator('#main-content').hover(); await page.mouse.wheel(0,-250); await page.waitForFunction(()=>!LunaReviewGaps.view.follow);
      const sequences=()=>page.locator('[data-hook-log]').evaluateAll(nodes=>nodes.map(n=>Number(n.dataset.hookLog)));
      const idsBefore=await sequences(); const scrollBefore=await page.locator('#main-content').evaluate(n=>n.scrollTop);
      await page.locator('#review-fault').selectOption('integrity');
      assert.deepEqual(await sequences(),idsBefore); assert.equal(await state(()=>LunaReviewGaps.sessions.get(LunaDesign.state.session.sessionId).records.length),recordsBefore);
      assert.ok(await state(()=>LunaReviewGaps.view.newCounts.logs)>0); assert.equal(await state(()=>LunaReviewGaps.view.newCounts.texts),0); assert.match(await action('toggle-follow').textContent(),/新内容/);
      assert.ok(Math.abs(await page.locator('#main-content').evaluate(n=>n.scrollTop)-scrollBefore)<2);
      await capture('15b-hook-only-new-count','#main-window','只有新Hook事件时也显示新到数量，旧序列与阅读位置保持');
      await action('toggle-follow').click(); assert.equal(await state(()=>LunaReviewGaps.view.newCount),0);
    });
    await check('session_identity_isolation',['RW-18','RW-23'],async()=>{
      await open('session-texts');
      const first=await state(()=>LunaDesign.state.session.sessionId);
      await page.locator('#main-window [data-action="disconnect"]').click(); await page.locator('#dialog-confirm').click();
      await page.locator('#main-window .sidebar-item[data-page="library"]').click(); await page.locator('[data-game="mo"]').click(); await page.locator('#library-running').click();
      await page.locator('[data-gl-action="select-target"][data-gl-id="sample-window-mo"]').click(); await page.locator('#dialog-confirm').click();
      await page.waitForFunction(()=>LunaDesign.state.sheet?.step===3); await page.locator('[data-stream="mo-body"]').check(); await page.locator('#sheet-next').click();
      await page.locator('#menu-window').click(); await page.locator('[data-menu-action="open-main"]').click(); await page.locator('.sidebar-item[data-page="session"]').click(); await page.locator('[data-rw-tab="texts"]').click();
      const snapshot=await state(()=>({id:LunaDesign.state.session.sessionId,records:LunaReviewGaps.sessions.get(LunaDesign.state.session.sessionId).records,old:LunaReviewGaps.sessions.get('sakura-session-1')?.records.length}));
      assert.notEqual(snapshot.id,first); assert.ok(snapshot.records.length); assert.ok(snapshot.records.every(r=>r.sessionId===snapshot.id)); assert.ok(snapshot.old>0);
      await capture('16-session-isolation','#main-window','切换目标后按新会话显示，旧会话数据保留');
    });
    await check('ruby_independent_of_segmentation_and_original_reconstruction',['RW-26','RW-27','RW-28'],async()=>{
      await open('hud-plain'); const expected=await state(()=>LunaDesign.state.current.source); assert.equal(await rawText(),expected); assert.equal(await page.locator('#hud-source rt').count(),0);
      await page.locator('#hud-furigana').click(); assert.ok(await page.locator('#hud-source rt').count()>0); assert.equal(await page.locator('#hud-source [data-token-index]').count(),0); assert.equal(await rawText(),expected);
      await capture('17-hud-ruby-only','#hud-window','同一句仅开启注音，不隐式开启可点分词或POS');
      await page.locator('#hud-segmentation').click(); assert.ok(await page.locator('#hud-source [data-token-index]').count()>0); assert.equal(await rawText(),expected);
      await page.locator('#hud-pos').click(); assert.ok(await page.locator('#hud-source [class*="lp-pos-"]').count()>0);
      await capture('18-hud-ruby-segmentation-pos','#hud-window','分词、ruby及词性组合的同句对照');
      await page.locator('#hud-furigana').click(); assert.equal(await page.locator('#hud-source rt').count(),0); assert.equal(await rawText(),expected);
    });
    await check('ruby_unknown_and_lexical_failure_keep_original',['RW-27','RW-29'],async()=>{
      await open('hud-unknown'); const expected=await state(()=>LunaDesign.state.current.source); assert.equal(await rawText(),expected);
      assert.ok(await page.locator('.lp-unknown-reading').count()>0); await capture('19-hud-unknown-reading','#hud-window','未知读音不编造注音');
      await open('hud-lexical-error'); const failureSource=await state(()=>LunaDesign.state.current.source); assert.equal(await rawText(),failureSource); assert.equal(await page.locator('#hud-source rt').count(),0);
      assert.match(await page.locator('#hud-window').textContent(),/词法|分词|标注/); await capture('20-hud-lexical-failure','#hud-window','词法失败保留原文及恢复提示');
    });
    await check('hud_translucency_and_reduced_transparency',['RW-24','RW-25'],async()=>{
      await open('hud-ruby'); const normal=await page.locator('#hud-window').evaluate(n=>({background:getComputedStyle(n).backgroundColor,blur:getComputedStyle(n).backdropFilter}));
      assert.match(normal.background,/rgba\(/); assert.notEqual(normal.blur,'none'); await capture('21-hud-translucent','#desktop','半透明磨砂设计；不是原生系统材质实测');
      await open('hud-reduced'); const reduced=await page.locator('#hud-window').evaluate(n=>({background:getComputedStyle(n).backgroundColor,blur:getComputedStyle(n).backdropFilter})); assert.equal(reduced.blur,'none'); assert.notEqual(reduced.background,normal.background);
      await capture('22-hud-reduced','#hud-window','应用减少透明度的实色回退');
      await open('hud-ruby'); const cdp=await context.newCDPSession(page); await cdp.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'}]});
      assert.equal(await page.locator('#hud-window').evaluate(n=>getComputedStyle(n).backdropFilter),'none'); await capture('23-system-reduced','#hud-window','系统媒体偏好仿真的实色回退'); await cdp.detach();
    });
    await check('component_buttons_icons_and_visible_state_list',['RW-31','RW-32'],async()=>{
      await open('components'); assert.equal(await page.locator('#select-showcase').isVisible(),true); assert.equal(await page.locator('#hud-controls').isVisible(),true); assert.equal(await page.locator('#ruby-material').isVisible(),true);
      assert.equal(await page.locator('[data-luna-icon]:not([data-luna-mounted])').count(),0);
      assert.ok(await page.locator('.rw-component-page button:disabled').count()>0); assert.ok(await page.locator('.rw-component-page [aria-pressed=true]').count()>0);
      await capture('24-components','#main-window','统一Select、按钮、图标、语言与移动入口状态');
    });
    await check('hud_toolbar_reachable_at_560_and_500_without_menu_duplicates',['RW-09','RW-32'],async()=>{
      await open('reading');
      const ids=['hud-pin','hud-size','hud-hide','hud-segmentation','hud-furigana','hud-pos','hud-display-mode','hud-pause','hud-translation-toggle','hud-retranslate','hud-copy','hud-learn','hud-more'];
      const inspect=async expectedWidth=>{
        const hud=await box('#hud-window'); assert.equal(Math.round(hud.width),expectedWidth);
        for(const id of ids) {
          const control=page.locator('#'+id); assert.equal(await control.isVisible(),true,id);
          const geometry=await control.evaluate(n=>{const b=n.getBoundingClientRect(),hit=document.elementFromPoint(b.x+b.width/2,b.y+b.height/2);return {x:b.x,right:b.right,y:b.y,bottom:b.bottom,width:b.width,height:b.height,hit:!!hit&&(n===hit||n.contains(hit)),name:n.getAttribute('aria-label')||n.textContent,title:n.getAttribute('title')};});
          assert.ok(geometry.width>0&&geometry.height>0&&geometry.x>=hud.x-1&&geometry.right<=hud.x+hud.width+1,id); assert.ok(geometry.hit,id+' must receive a real pointer hit'); assert.ok(geometry.name.trim(),id+' accessible name');
          await control.focus(); assert.equal(await control.evaluate(n=>n===document.activeElement),true,id+' keyboard focus');
        }
        assert.equal(await page.locator('.hud-controls').evaluate(n=>n.scrollWidth>n.clientWidth),false);
      };
      await inspect(560); await capture('25-toolbar-560','#hud-window','560宽常用操作直接可见，顶部轻量窗口操作与正文分离');
      await page.locator('#hud-size').click(); await inspect(500); await capture('26-toolbar-500','#hud-window','500紧凑宽度同样保留所有常用动作，不截断或横向滚动');
      await page.locator('#hud-more').click();
      const menuActions=await page.locator('#hud-menu [data-action]').evaluateAll(nodes=>nodes.map(n=>n.dataset.action));
      const quick=['pin-hud','toggle-hud-size','toggle-hud','toggle-segmentation','toggle-furigana','toggle-pos','cycle-display-mode','toggle-pause','toggle-translation','retranslate-current','copy-current','learn-current'];
      assert.deepEqual(menuActions.filter(value=>quick.includes(value)),[]);
      assert.ok(menuActions.includes('open-settings')&&menuActions.includes('bind-window')&&menuActions.includes('disconnect'));
      assert.equal(await page.locator('#hud-menu [data-rw-action="reset-hud"]').count(),1);
      await capture('27-toolbar-low-frequency-menu','#desktop','更多仅保留低频设置、关联、位置和断开，无常用动作重复');
      observe('hud-toolbar-500-560',{checks:activeCheck.observations.filter(item=>item.kind==='assertion')});
    });
    await check('hud_direct_actions_operate_on_current_sentence',['RW-09','RW-32'],async()=>{
      await open('hud-plain'); const sourceText=await state(()=>LunaDesign.state.current.source);
      await page.locator('#hud-translation-toggle').click(); assert.equal(await state(()=>LunaDesign.state.session.translationEnabled),false); assert.equal(await state(()=>LunaDesign.state.session.paused),false);
      await page.locator('#hud-translation-toggle').click(); assert.equal(await state(()=>LunaDesign.state.session.translationEnabled),true);
      await page.locator('#hud-pause').click(); assert.equal(await state(()=>LunaDesign.state.session.paused),true); assert.equal(await page.locator('#hud-window').isVisible(),true); await page.locator('#hud-pause').click();
      await page.locator('#hud-copy').click(); assert.equal(await page.locator('#app-dialog textarea').inputValue(),sourceText); await page.locator('#dialog-cancel').click();
      const recordBefore=await state(()=>LunaDesign.state.current.recordId); await page.locator('#hud-retranslate').click(); await page.waitForFunction(()=>LunaDesign.state.current.translationStatus==='complete'); assert.notEqual(await state(()=>LunaDesign.state.current.recordId),recordBefore); assert.equal(await state(()=>LunaDesign.state.current.source),sourceText);
      const idBefore=await state(()=>LunaDesign.state.current.id); await page.locator('#hud-learn').click(); assert.equal(await page.locator('#learning-window').isVisible(),true); assert.equal(await state(()=>LunaDesign.state.learning.unit.id),idBefore); assert.equal(await state(()=>LunaDesign.state.learning.unit.source),sourceText); await page.locator('[data-action="close-learning"]').click();
      const pinBefore=await page.locator('#hud-pin').getAttribute('aria-pressed'); await page.locator('#hud-pin').click(); assert.notEqual(await page.locator('#hud-pin').getAttribute('aria-pressed'),pinBefore);
      for(const mode of ['source','translation','both']) {await page.locator('#hud-display-mode').click(); assert.equal(await state(()=>LunaDesign.state.displayMode),mode);}
      await page.locator('#hud-hide').click(); assert.equal(await page.locator('#hud-window').isVisible(),false); assert.equal(await state(()=>LunaDesign.state.session.paused),false); await page.keyboard.press('Alt+Meta+h'); assert.equal(await page.locator('#hud-window').isVisible(),true);
      await capture('28-toolbar-direct-actions','#hud-window','常用动作直接生效，复制与精读固定原句，显隐不暂停取文');
    });
    await check('lexical_preferences_all_eight_combinations',['RW-26','RW-27','RW-28','RW-29'],async()=>{
      for(const segmentation of [false,true])for(const furigana of [false,true])for(const pos of [false,true]) {
        await open('hud-plain');
        const expected=await state(()=>LunaDesign.state.current.source);
        await page.locator('#hud-segmentation').click();
        if(pos)await page.locator('#hud-pos').click();
        if(furigana)await page.locator('#hud-furigana').click();
        if(!segmentation)await page.locator('#hud-segmentation').click();
        const actual=await state(()=>({segmentation:LunaDesign.state.settings.general.segmentation,furigana:LunaDesign.state.settings.general.furigana,pos:LunaDesign.state.settings.general.pos}));
        assert.deepEqual(actual,{segmentation,furigana,pos});assert.equal(await rawText(),expected);
        const ruby=await page.locator('#hud-source rt:visible').count(),tokens=await page.locator('#hud-source [data-token-index]').count(),colours=await page.locator('#hud-source [class*="lp-pos-"]').count();
        assert.equal(ruby>0,furigana,'Ruby visibility is independent of clickable segmentation');
        assert.equal(tokens>0,segmentation);assert.equal(colours>0,segmentation&&pos);assert.equal(await page.locator('#hud-pos').isDisabled(),!segmentation);
        const dimension='lexical-'+[segmentation,furigana,pos].map(Number).join('');observe(dimension,{actual,ruby,tokens,colours,source:await rawText()});
        await capture('29-'+dimension,'#hud-window','分词/注音/词性独立偏好组合 '+dimension.slice(-3));
      }
    });
    await check('select_all_product_entrypoints_and_state_matrix',['RW-13','RW-14','RW-15','RW-16','RW-31'],async()=>{
      const audited=[];
      const audit=async label=>{
        await page.waitForFunction(()=>[...document.querySelectorAll('select')].filter(n=>!n.closest('.review-bar')).every(n=>n.dataset.customSelect==='ready'));
        const ids=await page.locator('select').evaluateAll(nodes=>nodes.filter(n=>!n.closest('.review-bar')).map(n=>n.id));
        for(const id of ids){
          const select=page.locator('[id="'+id+'"]'),trigger=page.locator('[data-select-for="'+id+'"]');
          assert.equal(await trigger.count(),1,label+':'+id);assert.equal(await select.evaluate(n=>n.hidden&&getComputedStyle(n).display==='none'),true);
          assert.equal(await trigger.getAttribute('role'),'combobox');assert.equal(await trigger.getAttribute('aria-haspopup'),'listbox');
          const before=await select.inputValue();
          if(await trigger.isVisible()&&!await trigger.isDisabled()){
            await trigger.scrollIntoViewIfNeeded();await trigger.focus();await trigger.press('Space');assert.equal(await trigger.getAttribute('aria-expanded'),'true');
            const panel=page.locator('.luna-select-panel');assert.equal(await panel.getAttribute('role'),'listbox');
            const options=panel.locator('[role=option]'),count=await options.count();
            assert.equal(count,await select.locator('option').count());
            if(count){await trigger.press('End');assert.ok(await panel.locator('.is-active[aria-disabled="false"]').count()<=1);}
            await trigger.press('Escape');assert.equal(await select.inputValue(),before);assert.equal(await trigger.getAttribute('aria-expanded'),'false');assert.equal(await trigger.evaluate(n=>n===document.activeElement),true);
          }
          audited.push({context:label,id,disabled:await trigger.isDisabled()});
        }
      };
      for(const scenario of ['components','session-texts','session-logs','existing-library','history','manual']){await open(scenario);await audit(scenario);}
      await open('settings');
      const categories=await page.locator('[data-settings-action="category"]').evaluateAll(nodes=>nodes.map(n=>n.dataset.value));
      for(const category of categories){await page.locator('[data-settings-action="category"][data-value="'+category+'"]').click();await audit('settings/'+category);
        if(category==='translation')for(const tab of ['purposes','connections','models']){await page.locator('[data-settings-action="tab"][data-value="'+tab+'"]').click();await audit('settings/translation/'+tab);}}
      await open('existing-library');await state(()=>LunaLibrary.openAdd(LunaDesign));await audit('library/add');await page.locator('#dialog-cancel').click();
      await state(()=>LunaLibrary.openRunning(LunaDesign));await audit('library/window-binding');await page.locator('#dialog-cancel').click();
      assert.ok(audited.length>20);assert.ok(audited.some(row=>row.context==='library/add'));assert.ok(audited.some(row=>row.context==='settings/translation/models'));
      observe('select-all-entrypoints',audited);await open('components');await capture('30-all-select-entrypoints','#main-window','选择器入口矩阵已逐项验证；具体上下文和状态见观察回执');
    });
    await check('preview_switch_cancel_and_stale_frame_identity',['RW-01','RW-02','RW-05'],async()=>{
      await open('window-preview');assert.equal(await action('enable-preview').count(),0);
      const original=await state(()=>JSON.parse(JSON.stringify(LunaDesign.state.session)));
      const firstId=await page.locator('#window-preview').getAttribute('data-preview-id');
      await state(()=>LunaLibrary.openRunning(LunaDesign));await page.locator('[data-gl-action="select-target"][data-gl-id="sample-window-mo"]').click();
      const preview=page.locator('#app-dialog [data-preview-id]');const selectedId=await preview.getAttribute('data-preview-id');assert.notEqual(selectedId,firstId);
      assert.equal(await page.locator('#app-dialog [data-rw-action="enable-preview"]').count(),0);assert.equal(await preview.locator('img').count(),1);
      const selectedSource=await preview.locator('img').getAttribute('src');
      const candidate=await state(()=>LunaLibrary.windowChoices().find(item=>item.id==='sample-window-mo'));
      const runningDialog=await page.locator('#dialog-content').innerHTML();
      await inspectPreviewDetails(preview,{name:'candidate',expected:[candidate.title,candidate.targetPath,String(candidate.windowsPid),String(candidate.hostPid),candidate.processStartedAt]});
      assert.equal(await page.locator('#dialog-content').innerHTML(),runningDialog,'Candidate details must not overwrite the running-target panel');
      await state(()=>{const old=LunaReviewGaps.previews.get(LunaDesign.state.session.runtimeTarget.hostWindowId);old.scene='late-old-frame.png';old.frameSource='late-old-frame.png';});
      assert.equal(await preview.getAttribute('data-preview-id'),selectedId);assert.equal(await preview.locator('img').getAttribute('src'),selectedSource,'Late state of previous target cannot change selected preview');
      await capture('31-preview-cancel-identity','#app-dialog','切换预览按窗口身份隔离；取消不改变正在取文的会话');
      await page.locator('#dialog-cancel').click();assert.deepEqual(await state(()=>JSON.parse(JSON.stringify(LunaDesign.state.session))),original);
      await state(()=>LunaLibrary.openRunning(LunaDesign));await page.locator('[data-gl-action="select-target"][data-gl-id="sample-window-sakura"]').click();
      assert.equal(await page.locator('#app-dialog [data-preview-id]').getAttribute('data-preview-id'),firstId);await page.locator('#dialog-cancel').click();
      await inspectPreviewDetails(page.locator('#window-preview'),{name:'session-after-same-target-cancel',expected:['会话常驻 · 跨页签继续',original.runtimeTarget.title,original.runtimeTarget.targetPath]});
      await open('window-preview');
      // Two presentation hosts may describe the same native window while the
      // active session retains its own verified runtime snapshot. A candidate
      // review must not replace those facts, even when both share one frame.
      const sessionContext=await state(()=>{
        Object.assign(LunaDesign.state.session.runtimeTarget,{title:'已关联 "会话" <窗口>&快照',targetPath:'C:\\SessionSnapshot\\verified-game.exe',hostPid:49123,windowsPid:7123,processStartedAt:'session-verified-start'});
        LunaDesign.render();return JSON.parse(JSON.stringify(LunaDesign.state.session));
      });
      const sharedBefore=await state(()=>{const p=LunaReviewGaps.previews.get(LunaDesign.state.session.runtimeTarget.hostWindowId);return {frame:p.scene,time:p.updatedIso,count:LunaReviewGaps.previews.size};});
      await state(()=>LunaLibrary.openRunning(LunaDesign));await page.locator('[data-gl-action="select-target"][data-gl-id="sample-window-sakura"]').click();
      const sameTargetCandidate=await state(()=>LunaLibrary.windowChoices().find(item=>item.id==='sample-window-sakura'));
      assert.equal(await page.locator('#app-dialog [data-preview-id]').getAttribute('data-preview-id'),sessionContext.runtimeTarget.hostWindowId);
      await inspectPreviewDetails(page.locator('#app-dialog [data-preview-id]'),{name:'same-window-candidate-owner',expected:['候选核对 · 未确认接入',sameTargetCandidate.title,sameTargetCandidate.targetPath,String(sameTargetCandidate.hostPid),String(sameTargetCandidate.windowsPid),sameTargetCandidate.processStartedAt]});
      await page.locator('#dialog-cancel').click();
      await page.locator('#window-preview [data-rw-action="refresh-preview"]').click();
      await inspectPreviewDetails(page.locator('#window-preview'),{name:'same-window-session-owner',expected:['会话常驻 · 跨页签继续',sessionContext.runtimeTarget.title,sessionContext.runtimeTarget.targetPath,String(sessionContext.runtimeTarget.hostPid),String(sessionContext.runtimeTarget.windowsPid),sessionContext.runtimeTarget.processStartedAt]});
      assert.deepEqual(await state(()=>JSON.parse(JSON.stringify(LunaDesign.state.session))),sessionContext);
      assert.deepEqual(await state(()=>{const p=LunaReviewGaps.previews.get(LunaDesign.state.session.runtimeTarget.hostWindowId);return {frame:p.scene,time:p.updatedIso,count:LunaReviewGaps.previews.size};}),sharedBefore,'Candidate and session detail hosts share the original frame, timestamp and one capture Map');
      observe('preview-presentation-host-isolation',{window:sessionContext.runtimeTarget.hostWindowId,sessionTarget:sessionContext.runtimeTarget,candidateTarget:sameTargetCandidate,sessionUnchanged:true,sharedCaptureUnchanged:true});
      observe('preview-cancel-stale-frame',{originalSession:original.sessionId,originalWindow:firstId,selectedWindow:selectedId,selectedSource,scope:'prototype per-window state isolation; async native frames unexecuted'});
    });
    await check('no_console_errors_or_network',['RW-33'],async()=>{assert.deepEqual(consoleErrors,[]);assert.deepEqual(requests,[]);});
    await check('source_stable_during_run',['RW-34'],async()=>assert.deepEqual(sources(),before));
    excluded.push(...[
      ['real_game','本轮只运行离线交互设计，未连接真实游戏。'],['native','未运行正式包或操作系统窗口材质/移动。'],['global_hotkey','浏览器按键事件不证明系统全局注册、焦点和冲突。'],['tokenizer','注音来自明确设计词法样例，没有运行真实worker/内置资源。'],['live_capture','窗口画面是标记的离线采集回放，没有录屏权限调用。'],['product_regression','未执行新增34项正式产品验收，旧结果不继承。']
    ].map(([id,reason])=>({id,status:'not_run',reason})));
    const failed=checks.filter(item=>item.status==='failed'),notRun=checks.filter(item=>item.status==='not_run');
    const complete=checks.length===Object.keys(DESIGN_CAPABILITIES).length;
    const result={schema_version:2,layer:'design',execution_nonce:process.env.LUNA_DESIGN_EXECUTION_NONCE||null,started_at:startedAt,finished_at:new Date().toISOString(),source_tree_sha256_before:process.env.LUNA_DESIGN_SOURCE_TREE_SHA256||null,source_tree_sha256_after:process.env.LUNA_DESIGN_SOURCE_TREE_SHA256||null,source_files_sha256_before:fullBefore,source_files_sha256_after:independentlyHashManifest(),qa_script_sha256:hash(__filename),coverage_registry_sha256:fs.existsSync('docs/development/reading-window-test-coverage.json')?hash('docs/development/reading-window-test-coverage.json'):null,screenshots:captures.map(c=>({id:c.file.slice(0,-4),path:c.file,sha256:c.sha256,check_ids:c.check_ids})),revision:'six-product-gaps-design-candidate',run_id:runId,scope:complete?'offline design interaction QA only':'filtered adversarial/debug run; not full-suite proof',status:failed.length?'failed':notRun.length||!complete?'incomplete':'passed',artifact_qa_passed:complete&&failed.length===0&&notRun.length===0,design_only:true,user_review:JSON.parse(fs.readFileSync(path.join(source,'manifest.json'),'utf8')).user_review,approval_record:'docs/development/design-approval-20261019.json',approval_record_sha256:fs.existsSync('docs/development/design-approval-20261019.json')?hash('docs/development/design-approval-20261019.json'):null,product_regression_executed:false,real_game:false,native:false,global_hotkey:false,tokenizer:false,live_capture:false,source_scope:'index.html plus all top-level JS/CSS; excludes derived gallery, entry-flow, comparison and review reports',source_sha256:before,source_top_level_sha256:before,source_top_level_sha256_after:sources(),browser:await browser.version(),mode:'offline file:// Chrome headless',viewport:{width:1600,height:1100},command:['node',path.relative(process.cwd(),__filename),rootOutput],checks,captures,excluded_runtime_checks:excluded,console_errors:consoleErrors,http_requests:requests};
    fs.writeFileSync(path.join(output,'verification.json'),JSON.stringify(result,null,2)+'\n');
    process.stdout.write(JSON.stringify({status:result.status,passed:checks.filter(c=>c.status==='passed').length,failed:failed.length,not_run:notRun.length,captures:captures.length,output,verification_sha256:hash(path.join(output,'verification.json'))},null,2)+'\n');
    if(!result.artifact_qa_passed)process.exitCode=1;
  } finally {
    // Browser ownership closes its contexts as well. Closing the context first
    // can stall Chromium teardown after clipboard/native-keyboard exercises.
    process.stderr.write('DESIGN_TEARDOWN browser.close start\n');
    await browser.close();
    process.stderr.write('DESIGN_TEARDOWN browser.close complete\n');
  }
}
main().catch(error=>{fs.mkdirSync(output,{recursive:true});fs.writeFileSync(path.join(output,'fatal-error.json'),JSON.stringify({status:'failed',error:error.stack,checks,captures,console_errors:consoleErrors,http_requests:requests},null,2)+'\n');process.stderr.write(error.stack+'\n');process.exitCode=1;});
