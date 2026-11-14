import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const root=process.cwd(), out=path.resolve('.build/acceptance-closure-20261019/visual/registered-candidate-v3-review7');
const require=createRequire(path.join(root,'package.json'));const {chromium}=require('playwright-core');
const sha=data=>createHash('sha256').update(data).digest('hex');
const original=path.join(root,'docs/design/first-product-v1');
const sourceHashes={};
async function recordOriginals(directory){for(const entry of await fs.readdir(directory,{withFileTypes:true})){const file=path.join(directory,entry.name);if(entry.isDirectory()){if(!['previews','acceptance-v3'].includes(entry.name))await recordOriginals(file);}else if(entry.isFile())sourceHashes[path.relative(root,file)]=sha(await fs.readFile(file));}}
await recordOriginals(original);
await fs.mkdir(path.join(out,'design-origin'),{recursive:true});
for(const file of Object.keys(sourceHashes)){const relative=path.relative(original,path.join(root,file));const dest=path.join(out,'design-origin',relative);await fs.mkdir(path.dirname(dest),{recursive:true});await fs.copyFile(path.join(root,file),dest);}

const browser=await chromium.launch({headless:true});const records=[];const errors=[];
async function open(scenario){const page=await browser.newPage({viewport:{width:1600,height:1100},deviceScaleFactor:1,reducedMotion:'reduce'});page.on('pageerror',e=>errors.push(e.message));await page.goto(pathToFileURL(path.join(original,'index.html')).href+`?scenario=${scenario}&theme=dark`);await page.waitForFunction(()=>!!window.LunaDesign);await page.evaluate(()=>document.fonts.ready);return page;}
const sizes={"empty-library":[1040,720],"add-game":[1040,720],"target-review":[1040,720],"settings-connections":[1000,720],"reading-hud":[560,147],"learning-fixed":[600,670],"library-batch":[1040,720],"display-preferences":[560,344],"model-scheduling":[1000,720],"connection-proxy":[1000,720],"history-earlier":[1040,720]};
const coords={"empty-library":[280,86],"add-game":[280,86],"target-review":[280,86],"settings-connections":[300,80],"reading-hud":[320,451],"learning-fixed":[975,112],"library-batch":[280,86],"display-preferences":[320,451],"model-scheduling":[300,80],"connection-proxy":[300,80],"history-earlier":[280,86]};
async function capture(page,id,selector,operations,{clientSelector=null,changes=[],conflicts=[]}={}){
 const [width,height]=sizes[id], [x,y]=coords[id];
 const originalDesignRuntimeState=await page.evaluate(()=>{const a=window.LunaDesign,s=a.state;return {page:s.page,windows:s.windows,theme:s.theme,session:s.session,current:s.current,learning:s.learning,settings:s.settings};});
 // Snapshot only original, independently authored design DOM. No product imports.
 const source=await page.evaluate(({selector,id})=>{
   const node=document.querySelector(selector);if(!node)throw new Error('Missing original design surface');
   const clone=node.cloneNode(true);clone.id='reference-surface';clone.classList.add('native-design-surface');
   if(clone.tagName==='DIALOG'){clone.removeAttribute('open');clone.setAttribute('open','');clone.setAttribute('role','dialog');}
   const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
   // Prototype-only review controls are absent from the formal client. This is
   // source-level adaptation, with all user fields retained in the comparison.
   clone.querySelectorAll('.gl-design-note,.gl-chooser-row,.gl-fixture-list,.gl-identity-note').forEach(n=>n.remove());
   if(id==='add-game'||id==='library-batch') {
     const form=clone.querySelector('.gl-metadata');
     const field=(label,value,control='input')=>`<label class="gl-field"><span>${label}</span><div>${control==='select'?`<select><option>${esc(value)}</option></select>`:`<input value="${esc(value)}">`}</div></label>`;
     const title=id==='add-game'?'サクラノ詩 10th Anniversary edition':'同名程序 · 1';
     const bottle=id==='add-game'?'/Users/design/Library/Application Support/CrossOver/Bottles/Sakura-no-Uta-10th':'/Users/design/Bottles/Unknown-Bottle-A';
     const exe=id==='add-game'?'C:\\Program Files\\Sakura-no-Uta-10th\\sakuranouta10th.exe':'C:\\Games\\Unknown\\Game.exe';
     form.querySelector('[data-gl-field=title]').setAttribute('value',title);
     form.querySelector('[data-gl-field=bottle]').setAttribute('value',bottle);
     form.querySelector('[data-gl-field=launchPath]').setAttribute('value',exe);
     form.querySelector('[data-gl-field=targetPath]').setAttribute('value',id==='add-game'?exe:'');
     for(const name of ['bottle','launchPath','targetPath'])form.querySelector(`[data-gl-field=${name}]`).closest('label').querySelector('.gl-required')?.remove();
     if(id==='library-batch'){const next=document.createElement('button');next.type='button';next.className='ls-button';next.textContent='核对下一项';form.previousElementSibling?.classList.contains('gl-import-items')&&form.previousElementSibling.append(next);}
     form.querySelector('[data-gl-field=cwd]').setAttribute('value','');
     const original=document.createElement('div');original.innerHTML=field('原名',id==='add-game'?'サクラノ詩':'')+field('原文语言','日语','select');
     const first=form.querySelector('.gl-field');first.after(...original.children);
     const args=form.querySelector('[data-gl-field=args]').closest('.gl-field');
     args.outerHTML='<div class="gl-field"><span>启动参数</span><div><small>按顺序传入游戏，保留每项中的空格与引号。没有参数时可留空。</small><button>添加启动参数</button></div></div>';
   }
   if(id==='target-review'){
     const toolbar=clone.querySelector('.gl-running-toolbar');if(toolbar){const scan=document.createElement('button');scan.type='button';scan.className='ls-button';scan.textContent='扫描与筛选…';toolbar.before(scan);}
     clone.querySelectorAll('.gl-target').forEach(n=>{if(n.dataset.glId!=='sample-window-sakura')n.remove();});
     const row=clone.querySelector('.gl-target small');if(row)row.textContent='Sakura-no-Uta-10th · x64 · Windows PID 384';
     const mapping=clone.querySelector('.gl-mapping');
     if(mapping){
       const vals=mapping.querySelectorAll('dd');vals[1].textContent='PID 384 · sakuranouta10th.exe';vals[3].textContent='C:\\Program Files\\Sakura-no-Uta-10th\\sakuranouta10th.exe';
       mapping.querySelector('.gl-runtime-identity').innerHTML='<summary>高级运行身份</summary><p>Windows 窗口 HWND：未提供</p><p>进程创建身份：133000000000000000</p><p>macOS 宿主 PID：未提供</p><p>macOS 窗口身份：未提供</p>';
     }
   }
   if(id==='history-earlier'){
     const sidebar=clone.querySelector('.sidebar-session');if(sidebar)sidebar.innerHTML='原文 · 翻译 · 理解';clone.querySelectorAll('.sidebar-item i').forEach(node=>node.remove());
     const content=clone.querySelector('#main-content');
     content.innerHTML='<header class="page-header"><div><h1>历史记录</h1><p>原文、译文与人工修正按游戏及会话保留。</p></div><div class="page-actions"><button>刷新记录</button><button>定位当前句</button></div></header><div class="history-layout"><aside class="history-list"><div class="history-filter"><label>游戏筛选<select><option>全部游戏与手动输入</option></select></label><label>会话筛选<select><option>全部会话</option></select></label></div><button class="history-record" aria-pressed="false"><span class="history-source">独立した入力。</span><small>2026/10/19 18:30:00 · 手动输入 · 原文 v1</small></button><button class="history-record" aria-pressed="true"><span class="history-source">以前の原文。</span><small>2026/10/19 18:30:00 · 手动输入 · 原文 v1</small></button></aside><article class="history-detail"><div class="supp-history-meta"><span class="pill">ja · 原文 v1</span><button>复制原文</button></div><div class="source" lang="ja">以前の原文。</div><p class="lp-subtle">尚无译文。翻译用途未配置、未启用或仍在处理时，原文保持可读。</p><div class="history-actions"><button>使用当前配置重译</button><button>精读本句</button></div><div class="metadata">游戏 / 来源：手动输入<br>记录时间：2026/10/19 18:30:00<br>阅读会话：2026/10/19 18:30:00 · 已结束</div></article></div>';
   }
   if(id==='learning-fixed'){
     clone.querySelector('.lp-document-meta').innerHTML='<span class="reference-document-origin"><h1>正在理解的原文</h1><span>サクラノ詩</span></span><span>已固定当前句</span>';
     const latest=clone.querySelector('.lp-latest-note');latest.firstChild.textContent='新台词已到达。此处仍保留原句。';
     const setup=clone.querySelector('.lp-setup');setup.insertAdjacentHTML('afterend','<div class="lp-dictionary-actions"><button>'+window.LunaIcons.svg('book')+'分析本句文法</button></div><p class="lp-subtle">解释范围由已应用的用途配置决定；当前固定原句与允许的同会话上下文分别保留。</p>');
     clone.querySelector('.lp-footer strong').textContent='尚未选择可用模型';clone.querySelector('.lp-footer small').textContent='用途独立配置 · 原句与解释分别保留';
   }
   if(id==='settings-connections'||id==='model-scheduling'||id==='connection-proxy'){
     const tabs=clone.querySelector('.ls-tabs');if(tabs&&!tabs.textContent.includes('术语表')){const t=document.createElement('button');t.textContent='术语表';tabs.append(t);}
     if(id==='settings-connections'){
       const sections=clone.querySelectorAll('.ls-section');
       for(const section of sections)if(section.querySelector('h3')?.textContent==='并列翻译结果')section.innerHTML='<h3>并列翻译引擎</h3><div class="ls-form"><div class="ls-row"><span class="ls-row-label">引擎1</span><span class="ls-row-value"><select><option>Microsoft · 日中翻译</option></select><label class="ls-check"><input type="checkbox" checked><span>启用引擎1</span></label><p><button class="ls-button" disabled>上移</button><button class="ls-button" disabled>下移</button><button class="ls-button">从草稿移除</button></p></span></div><div class="ls-row"><span class="ls-row-label">附加引擎</span><button class="ls-button">添加并列引擎</button></div></div><p class="ls-section-note">按列表顺序展示，各引擎独立显示结果与失败。开关和排序先保存为草稿，应用后才生效。</p>';
       const auto=clone.querySelector('[data-settings-field="binding.auto"]');
       if(auto){auto.setAttribute('disabled','');auto.removeAttribute('checked');const row=auto.closest('.ls-row');row.querySelector('.ls-row-label').textContent='当前会话自动翻译';row.querySelector('.ls-check span').textContent='当前会话自动翻译';const help=row.querySelector('.ls-help')||document.createElement('div');help.className='ls-help';help.textContent='连接游戏后可用。此开关仅控制当前会话，与用途草稿启用及运行许可分别保留。';if(!help.parentElement)row.querySelector('.ls-row-value').append(help);}
     }
     const sections=[...clone.querySelectorAll('.ls-section')];
     const row=(label,value,help='')=>`<div class="ls-row"><div class="ls-row-label">${label}</div><div class="ls-row-value">${value}${help?`<div class="ls-help">${help}</div>`:''}</div></div>`;
     const section=(title,body)=>`<section class="ls-section"><h3>${title}</h3><div class="ls-form">${body}</div></section>`;
     if(id==='settings-connections'){
       const language=clone.querySelector('[data-settings-field="binding.language"]');if(language)language.innerHTML='<option>简体中文</option><option>English</option><option>日本語</option>';
       const effective=sections.flatMap(n=>[...n.querySelectorAll('.ls-row')]).find(n=>n.querySelector('.ls-row-label')?.textContent==='有效配置');
       if(effective)effective.querySelector('.ls-row-value').innerHTML='<span class="ls-effective">Microsoft · 日中翻译 · v0</span><span class="ls-badge">运行许可未建立</span><div class="ls-help">已应用配置与当前运行许可分别保留。</div>';
       const stateRow=clone.querySelector('[data-settings-field="binding.enabled"]')?.closest('.ls-row');if(stateRow){stateRow.querySelector('.ls-check').lastChild.textContent='实时翻译的草稿启用意图';stateRow.querySelector('.ls-help').textContent='应用后才采用此意图；保存或测试不会恢复已停用的运行许可。';}
     }
     if(id==='model-scheduling'){
       const allRows=sections.flatMap(n=>[...n.querySelectorAll('.ls-row')]);
       allRows.find(n=>n.querySelector('.ls-row-label')?.textContent==='模型版本')?.remove();
       const capability=clone.querySelector('[data-settings-field="model.capability"]');if(capability)capability.innerHTML='<option value="text">文本解释</option><option value="structured">结构化解释</option>';
       const capabilityRow=capability?.closest('.ls-row');if(capabilityRow)capabilityRow.querySelector('.ls-help').textContent='能力为手动声明；实际用途仍需测试。';
       const ref=sections.find(n=>n.querySelector('h3')?.textContent==='引用关系');if(ref)ref.outerHTML=section('引用关系',row('草稿用途','全局 · 实时翻译')+row('管理','<button class="ls-button ls-danger">从草稿移除…</button>','仅编辑草稿；历史、系统凭据与已应用版本保留。'));
     }
     if(id==='connection-proxy'){
       const protocol=clone.querySelector('[data-settings-field="connection.protocol"]');if(protocol)protocol.innerHTML='<option>OpenAI 兼容</option><option>Anthropic Messages</option><option>Microsoft 文本翻译 v3</option>';
       const deployment=clone.querySelector('[data-settings-field="connection.location"]');if(deployment)deployment.innerHTML='<option>云端</option><option selected>本机 / 内网服务</option>';
       const authentication=clone.querySelector('[data-settings-field="connection.auth"]');if(authentication)authentication.innerHTML='<option>API Key</option><option selected>无认证（需服务支持）</option>';
       const endpoint=clone.querySelector('[data-settings-field="connection.endpoint"]')?.closest('.ls-row');if(endpoint)endpoint.querySelector('.ls-help').textContent='按实际协议填写；本机无认证服务需要明确选择无认证。';
       const available=sections.find(n=>n.querySelector('h3')?.textContent==='连接可用性');if(available)available.outerHTML=section('连接可用性',row('连接状态','<label class="ls-check"><input type="checkbox" checked><span>此连接的草稿可用意图</span></label>','应用此意图影响新请求；保存或测试不会解除即时停用。'))+section('引用关系',row('草稿用途','全局 · 实时翻译')+row('管理','<button class="ls-button ls-danger">从草稿移除…</button>','仅编辑草稿；历史、系统凭据与已应用版本保留。'));
     }
     clone.querySelector('.ls-footer-status').innerHTML='<span>草稿已保存</span><br>草稿 v0 · 已应用 v0';
     clone.querySelector('[data-settings-action="apply"]').textContent='检查并应用…';
     const foot=clone.querySelector('.ls-sidebar-foot');if(foot)foot.innerHTML='个人配置 · 默认<br><span>全局与游戏覆盖</span>';
   }
   if(id==='history-earlier'){clone.querySelector('#sidebar-session').textContent='原文 · 翻译 · 理解';clone.querySelector('.history-detail .source').style.fontSize='18px';}
   clone.querySelectorAll('.traffic').forEach(n=>{n.replaceChildren();n.setAttribute('aria-hidden','true');});
   clone.querySelectorAll('input').forEach(n=>{n.setAttribute('value',n.value);if(n.type==='checkbox'||n.type==='radio'){if(n.checked)n.setAttribute('checked','');else n.removeAttribute('checked');}});
   clone.querySelectorAll('textarea').forEach(n=>n.textContent=n.value);
   clone.querySelectorAll('select').forEach(n=>[...n.options].forEach(o=>{if(o.selected)o.setAttribute('selected','');else o.removeAttribute('selected');}));
   let parent='';
   if(clone.tagName==='DIALOG'){const host=document.querySelector('#main-window').cloneNode(true);host.id='reference-parent';host.querySelectorAll('.traffic').forEach(n=>{n.replaceChildren();n.setAttribute('aria-hidden','true');});if(id==='target-review'){host.querySelectorAll('.game-cover img').forEach(n=>n.remove());host.querySelectorAll('.details-grid .code').forEach(n=>{if(n.textContent.includes('C:'))n.textContent=['C:','Program Files','Sakura-no-Uta-10th','sakuranouta10th.exe'].join(String.fromCharCode(92));});}parent=host.outerHTML;}
   return {parent,head:[...document.head.querySelectorAll('link[rel=stylesheet],style')].map(n=>{const c=n.cloneNode(true);if(c.href)c.setAttribute('href',new URL(n.href).pathname.split('/').pop());return c.outerHTML;}).join(''),node:clone.outerHTML};
 },{selector,id});
 const sourceFile=`${id}.design-source.html`;
 await fs.writeFile(path.join(out,sourceFile),`<!doctype html><html lang="zh-CN" data-theme="dark"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><base href="design-origin/">${source.head}<link rel="stylesheet" href="../native-content.css"></head><body class="native-design" data-reference-id="${id}" style="--underlay-x:${-x}px;--underlay-y:${-y}px;--underlay-image:url('underlay-reading.png')"><div class="native-design-underlay"></div>${source.parent}${source.node}${source.parent?'<script>const dialog=document.querySelector("#reference-surface");dialog.removeAttribute("open");dialog.showModal();</script>':''}</body></html>`);
 await page.close();
 page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1,reducedMotion:'reduce'});
 page.on('pageerror',error=>errors.push(error.message));
 await page.goto(pathToFileURL(path.join(out,sourceFile)).href);
 await page.evaluate(async()=>{await document.fonts.ready;const image=getComputedStyle(document.querySelector('.native-design-underlay')).backgroundImage;const url=image.match(/^url\(["']?(.*?)["']?\)$/)?.[1];if(url){const img=new Image();img.src=url;await img.decode();}await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));const dialog=document.querySelector('dialog[open]');if(dialog){dialog.tabIndex=-1;dialog.focus({preventScroll:true});}else document.activeElement?.blur?.();});
 await page.mouse.move(0,0);
 const surface=page.locator('#reference-surface');
 const geometry={viewport:{width,height},target:await surface.boundingBox(),deviceScaleFactor:1};
 const finalDesignState=await page.evaluate(()=>({
   headings:[...document.querySelectorAll('h1,h2,h3')].map(n=>n.textContent),
   fields:[...document.querySelectorAll('input,select,textarea')].map(n=>({label:n.getAttribute('aria-label')||n.closest('label')?.querySelector('span')?.textContent||n.dataset.glField||n.dataset.settingsField,value:n.value,checked:n.type==='checkbox'?n.checked:undefined})),
   selectedTabs:[...document.querySelectorAll('.ls-tabs .selected,.ls-purpose-picker .selected')].map(n=>n.textContent),
   targets:[...document.querySelectorAll('.gl-target')].map(n=>({title:n.querySelector('strong')?.textContent,summary:n.querySelector('small')?.textContent,selected:n.classList.contains('selected')})),
   mapping:document.querySelector('.gl-mapping')?.textContent||null,
   sourceText:document.querySelector('#hud-source,.lp-original,.history-detail .source')?.textContent||null,
   clientTitle:document.querySelector('.native-titlebar>strong')?.textContent||null,
   osControlsDrawn:document.querySelectorAll('.traffic>*').length
 }));
 const adaptations=[{kind:'native-client-boundary',detail:'Independent fixed native viewport; design desktop wrapper replaced with explicit same-underlay input. Client border/title/tools remain, only OS traffic-light controls unpainted.'}];
 if(['add-game','target-review','library-batch'].includes(id))adaptations.push({kind:'prototype-only-controls',removed:['.gl-design-note','.gl-chooser-row','.gl-fixture-list','.gl-identity-note'],detail:'Source-level removal of offline replay notes/fixture selector. No screenshot masking and no user field removed.'});
 if(['add-game','library-batch'].includes(id))adaptations.push({kind:'game-contract-data',detail:'Added original title and explicit original language fields; launch arguments are ordered array editor, empty in this state. Explicit full bottle locator, launch path and empty working directory follow declared regression input. Single-add canonical target is entered; batch target remains unknown/empty. Name alone is required for saving; runtime association verifies paths. Batch next-review remains in the same chip-navigation row.'});
 if(['add-game','library-batch','target-review'].includes(id))adaptations.push({kind:'portal-parent-material-input',detail:'Dialog remains over the independent original design main client at1040x720, excluding OS-owned traffic controls; original modal backdrop #10131c55 with blur3 retained via the native dialog.showModal/top-layer path. Add/batch parent library empty; target parent has one Sakura game with cover=null. Only continuous complete dialog is captured; no product image is used as background.'});
 if(id==='target-review')adaptations.push({kind:'target-contract-data',detail:'One backend-provided Sakura candidate; Windows PID384, creation133000000000000000; canonical path Program Files/Sakura-no-Uta-10th; HWND and macOS identities null shown as unavailable. No guessed host mapping.'});
 if(id==='learning-fixed')adaptations.push({kind:'fixed-learning-authoritative-data',detail:'Actual original title from Game.originalTitle; fixed current-sentence user label instead of fictitious #001. Actual unconfigured model status in footer. Explicit PRD-requested analysis action remains available and Rust owns real unconfigured failure. Internal unit/textRevision remain metadata only and are never painted.'});
 if(id==='display-preferences')adaptations.push({kind:'bounded-menu-workarea',detail:'Explicit Reduce Transparency applies the original styles.css accessibility fallback to the menu (solid shell/no blur) as well as HUD, following original DES125 and style-guide36. Preserve all original fourteen actions, source, fixed footer and native344px viewport. Bound the existing menu to viewport minus48px and scroll only its interior; top and bottom references cover every action. Original desktop clipping is retained in original32PNG, not reproduced as an inaccessible native menu.'});
 if(id==='history-earlier')adaptations.push({kind:'history-contract-data',detail:'Empty profile without any Hook session, current manual input and one earlier manual record; no fabricated live-game sidebar. Native flex height replaces desktop hard-coded93px header subtraction, eliminating an empty5px scroll tail while preserving all content. Authored same-state history for independent manual source and earlier original; game/session filters, preserved revision, provenance/time, refresh and explicit learning/retranslation actions. Reuses original history CSS; no product import.'});
 if(['settings-connections','model-scheduling','connection-proxy'].includes(id))adaptations.push({kind:'authoritative-configuration-contract',detail:'No model entity edit-version exists in DTO; remove fictitious static v1 row and show real draft/applied head0 in footer. Connection draft availability intent is separate from runtime panel; reference-management actions only edit drafts. Actual null sourceLanguage preserves automatic source direction, target Chinese. Microsoft applied entity exists but runtime authorization is absent, shown explicitly. Text labels follow actual protocol/auth contract; no designed endpoint or fake sample validation.'});
 if(['settings-connections','model-scheduling','connection-proxy'].includes(id))adaptations.push({kind:'first-product-configuration',detail:'Original five configuration segments plus PRD-required glossary segment; original ten categories retained; local model and connection entity values come from declared fixture, not product screenshot.'});
 if(id==='add-game'||id==='library-batch'){
   const expectedBottle=id==='add-game'?'/Users/design/Library/Application Support/CrossOver/Bottles/Sakura-no-Uta-10th':'/Users/design/Bottles/Unknown-Bottle-A';
   assert.equal(await surface.locator('[data-gl-field=bottle]').inputValue(),expectedBottle,'Serialized design bottle locator must match input state');
   assert.equal(await surface.locator('[data-gl-field=cwd]').inputValue(),'','Serialized working directory must stay empty');
   assert.equal(await surface.locator('[data-gl-field=launchPath]').inputValue(),id==='add-game'?'C:\\Program Files\\Sakura-no-Uta-10th\\sakuranouta10th.exe':'C:\\Games\\Unknown\\Game.exe');
 }
 const filenames=[];
 const save=async variant=>{const file=`${id}.${variant}.png`;await surface.screenshot({path:path.join(out,file),animations:'disabled'});filenames.push({file,variant,sha256:sha(await fs.readFile(path.join(out,file))),bounds:await surface.boundingBox(),scrollTop:await surface.locator('.dialog-body,.ls-scroll,.lp-scroll,.hud-menu,.hud-reading').filter({visible:true}).evaluateAll(ns=>ns.map(n=>({className:n.className,scrollTop:n.scrollTop,clientHeight:n.clientHeight,scrollHeight:n.scrollHeight}))),details:await surface.locator('details').evaluateAll(ns=>ns.map(n=>({summary:n.querySelector('summary')?.textContent,open:n.open})))});};
 await save('top');
 const scroller=surface.locator('.dialog-body,.ls-scroll,.lp-scroll,.hud-menu,.hud-reading').filter({visible:true});
 for(let regionIndex=0;regionIndex<await scroller.count();regionIndex++){
   // Each independent scroll surface is captured continuously, with the other
   // regions restored to their top state. Nothing outside the viewport is masked.
   await scroller.evaluateAll(nodes=>nodes.forEach(node=>node.scrollTop=0));
   const node=scroller.nth(regionIndex);
   const size=await node.evaluate(n=>({height:n.clientHeight,scroll:n.scrollHeight}));
   const max=Math.max(0,size.scroll-size.height),step=Math.max(1,size.height-24);
   const prefix=regionIndex===0?'':`region-${regionIndex}-`;
   for(let top=step,index=1;top<max;top+=step,index++){
     await node.evaluate((n,value)=>n.scrollTop=value,top);await save(`${prefix}scroll-${index}`);
   }
   if(max>0){await node.evaluate(n=>n.scrollTop=n.scrollHeight);await save(`${prefix}bottom`);}
 }

 records.push({id,status:'design-side-supplement-pending-independent-review',source:'Original design DOM/state with explicit native-content layout adapter. No React, product CSS, bundle or candidate pixels imported.',selector:'#reference-surface',operations,changes,conflicts,geometry,files:filenames,originalDesignRuntimeState,finalDesignState,sourceAdaptations:adaptations,clientBoundary:'Continuous complete client surface including border and title/header/tools/footer. Dialog body scroll variants cover complete fields without cropping.',nativeExclusions:['macOS system-owned traffic light controls only; original design control placeholder remains in layout'],underlay:{file:'underlay-reading.png',sha256:sha(await fs.readFile(path.join(out,'underlay-reading.png'))),size:[1600,925],sampleOrigin:[x,y],scale:1},fonts:{family:await surface.evaluate(n=>getComputedStyle(n).fontFamily),deviceScaleFactor:1}});
 await page.close();
}
async function originalSettingsNavigation(page){
 const labels=await page.locator('#settings-content [data-settings-action="category"]').allTextContents();
 assert.equal(labels.length,10,'Original ten single-layer categories must stay intact in design-side candidate');
 return [];
}

try{
 const underlay=await open('reading');
 await underlay.addStyleTag({content:'.review-bar,.menu-bar,.dock,.main-window,.hud-window,.settings-window,.learning-window,.word-popover,.system-menu,.app-toast{visibility:hidden!important}'});
 await underlay.locator('#desktop').screenshot({path:path.join(out,'underlay-reading.png'),animations:'disabled'});await underlay.close();
 let p=await open('first-run');await p.locator('#dock [data-action="toggle-hud"]').click();assert.equal(await p.locator('#hud-window').isVisible(),false);await capture(p,'empty-library','#main-window',['first-run','normal dock toggle-hud hides waiting HUD, preserving all original main content'],{clientSelector:'.main-layout',conflicts:['Original38 had real HUD occlusion. This new unoccluded design-side render is not an approved replacement.']});
 p=await open('first-run');await p.locator('#library-add').click();await p.locator('[data-gl-action="show-fixtures"]').click();await p.locator('[data-gl-action="add-fixture"][data-gl-id="sakura"]').click();await capture(p,'add-game','#app-dialog',['normal original design add-game -> fixture selector -> Sakura program review'],{conflicts:['Original design form has game launch arguments but production form currently does not. Keep complete form/footer; review product field coverage, not crop it away.']});
 p=await open('first-run');await p.locator('#library-add').click();await p.locator('[data-gl-action="show-fixtures"]').click();await p.locator('[data-gl-action="add-fixture"][data-gl-id="sakura"]').click();await p.locator('#dialog-confirm').click();await p.locator('#library-running').click();await p.locator('[data-gl-action="pick-window"]').click();await p.locator('[data-gl-action="select-target"][data-gl-id="sample-window-sakura"]').click();await capture(p,'target-review','#app-dialog',['normal original design select-window -> Sakura candidate -> before single associate-and-attach confirmation'],{conflicts:['Original design keeps candidates and mapping together. Product uses selection then review. Single-confirmation hook-injection semantics stay unchanged; review layout split explicitly.']});
 p=await open('settings');await p.evaluate(()=>{const s=window.LunaDesign.state.settings;s.applied={};s.revision=0;s.draft.connections={microsoft:s.draft.connections.microsoft};s.draft.models={traditional:s.draft.models.traditional};s.draft.bindings.translation.enabled=true;s.applied={'global:translation':{snapshot:structuredClone(s.draft),revision:0,enabled:true,source:'全局用途'}};window.LunaDesign.render();});let changes=await originalSettingsNavigation(p);await capture(p,'settings-connections','#settings-window',['original settings translation purpose','set explicit unconfigured/applied-empty state; draft translation enabled=true'],{clientSelector:'.settings-host',changes,conflicts:['Original ten categories retained. Product currently has six; product must align navigation and reuse actual first-release features, with later-scope categories explicitly explained. Draft/application status presentation remains a real design-to-product gap.']});
 p=await open('reading');await capture(p,'reading-hud','#hud-window',['original reading Sakura first line translated, default 18/24pt comfortable, menu closed'],{conflicts:['Complete original game image and wallpaper underlay retained. Test harness independent-window center/cover wallpaper does not produce same blur sampling.']});
 p=await open('reading');await p.evaluate(()=>{const a=window.LunaDesign;const original=structuredClone(a.state.current);a.openLearning(original,null);a.advance();});await p.waitForFunction(()=>window.LunaDesign.state.current.translationStatus==='complete');assert.notEqual(await p.evaluate(()=>window.LunaDesign.state.current.id),await p.evaluate(()=>window.LunaDesign.state.learning.unit.id));await capture(p,'learning-fixed','#learning-window',['open grammar for Sakura first line','advance original design game to next line; learning object remains original','grammar remains unconfigured'],{clientSelector:'.learning-host',conflicts:['Old11 complete desktop contains this unoccluded window. Fresh independent window candidate is not approved and not current React.']});
 p=await open('first-run');await p.locator('#library-add').click();await p.locator('#gl-game-files').setInputFiles([{name:'Game.exe',mimeType:'application/octet-stream',buffer:Buffer.alloc(0)},{name:'Game.exe',mimeType:'application/octet-stream',buffer:Buffer.alloc(0)}]);for(const [index,bottle]of ['Unknown-Bottle-A','Unknown-Bottle-B'].entries()){await p.locator(`[data-gl-action="select-item"][data-gl-index="${index}"]`).click();for(const [field,value]of Object.entries({title:`同名程序 · ${index+1}`,bottle,launchPath:'C:\\Games\\Unknown\\Game.exe',targetPath:'C:\\Games\\Unknown\\Game.exe',cwd:'C:\\Games\\Unknown'}))await p.locator(`[data-gl-field="${field}"]`).fill(value);await p.locator('[data-gl-field="arch"]').selectOption('x86');}await p.locator('[data-gl-action="select-item"][data-gl-index="0"]').click();await capture(p,'library-batch','#app-dialog',['original design file review with two Game.exe files','distinct bottles and explicit full paths','no save or injection'],{conflicts:['Original design reviews both records before saving; production sequential save must preserve all records/cancel semantics. Do not silently replace established batch behavior.']});
 p=await open('reading');await p.evaluate(()=>{const a=window.LunaDesign;a.state.settings.preferences.fontScale=2;a.state.settings.preferences.appearance='reduced';a.render();});await p.locator('#hud-more').click();assert.equal(await p.locator('#hud-menu').isVisible(),true);await capture(p,'display-preferences','#hud-window',['original reading first line','original settings preferences 200%/reduced','normal HUD menu opened; original32 also contains open menu'],{conflicts:['Direct PNG inspection corrected earlier false closed-menu assumption; full open menu, source, clipping and footer remain part of proposed reference.']});
 p=await open('settings');await p.evaluate(()=>{const a=window.LunaDesign;const d=a.state.settings.draft;d.connections={local:{...d.connections.local,name:'UI configured service',endpoint:'http://127.0.0.1:1234/v1',proxy:'inherit'}};d.models={local:{...d.models.local,name:'UI model A',modelId:'ui-entered-A',maxTokens:'',temperatureMode:'inherit'},b:{...d.models.local,id:'b',name:'UI model B',modelId:'ui-entered-B'}};a.state.settings.tab='models';a.state.settings.selectedModel='local';a.state.settings.applied={};a.state.settings.revision=0;a.render();});changes=await originalSettingsNavigation(p);await capture(p,'model-scheduling','#settings-window',['original model entity editor (not scheduling)','select original local model entity','explicit applied-empty draft state'],{clientSelector:'.settings-host',changes,conflicts:['Historical ID says scheduling, but22 is model entity editor. Dedicated scheduling reference still absent.']});
 p=await open('settings');await p.evaluate(()=>{const a=window.LunaDesign;const d=a.state.settings.draft;d.connections={local:{...d.connections.local,name:'UI configured service',endpoint:'http://127.0.0.1:1234/v1',proxy:'inherit'}};d.models={local:{...d.models.local,name:'UI model A',modelId:'ui-entered-A',maxTokens:'',temperatureMode:'inherit'},b:{...d.models.local,id:'b',name:'UI model B',modelId:'ui-entered-B'}};a.state.settings.tab='connections';a.state.settings.selectedConnection='local';a.state.settings.applied={};a.state.settings.revision=0;a.render();});changes=await originalSettingsNavigation(p);await capture(p,'connection-proxy','#settings-window',['original connection entity editor','select original local unauthenticated service','applied-empty draft state'],{clientSelector:'.settings-host',changes,conflicts:['Original23 uses cloud key auth. This explicit local entity candidate aligns protocol/auth shape; actual controlled endpoint/model IDs and proxy override must be reviewed independently.']});
 p=await open('history');await capture(p,'history-earlier','#main-window',['original history scenario with complete Sakura records','no mutation into a different source/translation pair'],{clientSelector:'.main-layout',conflicts:['Pagination correctness input以前の原文。 is not in original design. Keep original correctness assertion; this candidate only gives same-design-data history layout.']});
}finally{await browser.close();}
const missing=[{id:'wordcard-native-size',reason:'Current original design renders anchored in-page card, not the independently sized native360×460 surface and constrained560×290 work area. Authored native-card reference and boundary decision required.'},{id:'dictionary-distinct-headwords',reason:'Original10 contains風景, not the distinct homophone entries in correctness regression. No fake replacement or unreviewed dictionary senses generated.'},{id:'glossary-editing',reason:'No existing original design glossary editor rendering. Requires explicit authored design for term sources/targets/scope/save/error, not a model-entity screenshot.'},{id:'learning-japanese-linebreak',reason:'Original design has600×670 window. Dedicated1040×760 controlled sentence typography/linebreak design state requires authored review; resizing a screenshot is forbidden.'}];
const sourceUnchanged=[];for(const [file,hash]of Object.entries(sourceHashes))if(sha(await fs.readFile(path.join(root,file)))!==hash)sourceUnchanged.push(file);
const report={schema_version:1,status:'unapproved_design_side_proposals',not_product_acceptance:true,not_registered_reference:true,approved_images_overwritten:false,product_react_source_imported:false,sourceHashes,sourceUnchanged,viewport:{width:1600,height:1100},deviceScaleFactor:1,browser:await (async()=>{const b=await chromium.launch({headless:true});const v=b.version();await b.close();return v})(),records,missing,errors,notes:['Original sources stay read-only. No original design PNG overwritten. Prototype-only replay notes/fixture selector removed at design-source level; user file/target/metadata fields remain.','These screenshots are generated only from design HTML/CSS/JS; they are not actual React candidates or native/runtime proof.','All original ten settings categories retained; previous v1 five-category deletion rejected by independent review and not repeated.','No fixture from product correctness was removed or changed. All15 original capture IDs remain in actual product runner.']};
await fs.writeFile(path.join(out,'metadata.json'),JSON.stringify(report,null,2)+'\n');
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
await fs.writeFile(path.join(out,'index.html'),'<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>补充技术参照候选·未批准</title><style>body{font:15px/1.8 -apple-system,sans-serif;background:#171b24;color:#e9e9ef;max-width:1200px;padding:30px;margin:auto}section{padding:22px;border:1px solid #444957;border-radius:14px;margin:25px 0;background:#232734}img{max-width:100%;height:auto}p,li{color:#bdc1ce}a{color:#c4c5cf}.warn{padding:20px;background:#3b3427;border-radius:12px}@media(max-width:700px){body{padding:16px}section{padding:16px}}</style><h1>补充技术参照候选 · 尚未批准</h1><div class="warn">本页仅由现行设计HTML/CSS/JS渲染，不是产品截图、不是正式包证明，也不是新批准基线。原PNG、tokens、零差异阈值及整稿待审状态保持不变。独立审阅通过前不得替换门禁参考。</div><p><a href="metadata.json">逐状态数据、坐标、原sourceSHA和变更记录</a></p>'+records.map(r=>`<section><h2>${esc(r.id)}</h2><p>${esc(r.operations.join(' → '))}</p><ul>${r.conflicts.map(c=>`<li>${esc(c)}</li>`).join('')}${r.changes.map(c=>`<li>明确提案改动：${esc(JSON.stringify(c))}</li>`).join('')}</ul>${r.files.map(f=>`<figure><img loading="lazy" src="${esc(f.file)}" alt="${esc(r.id)}设计侧未批准候选"><figcaption>${esc(f.file)} · ${esc(f.sha256.slice(0,16))}</figcaption></figure>`).join('')}</section>`).join('')+'<h2>仍缺可比较设计状态</h2>'+missing.map(r=>`<section><h3>${esc(r.id)}</h3><p>${esc(r.reason)}</p></section>`).join('')+'</html>');
console.log(JSON.stringify({status:report.status,rendered:records.length,missing:missing.length,errors,sourceUnchanged}));
