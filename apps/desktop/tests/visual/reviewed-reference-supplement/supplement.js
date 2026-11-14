/* Independently authored candidate. Inputs are DTO-shaped, offline design data.
   No product module, product style, candidate screenshot, SQL, Hook or model is
   imported or executed here. These functions may be reviewed before adoption. */
(() => {
  'use strict';
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const icon = name => window.LunaIcons?.svg(name) || '';
  const option = (value, label, selected) => `<option value="${esc(value)}"${value === selected ? ' selected' : ''}>${esc(label)}</option>`;
  const pairs = values => `<dl>${values.map(([label,value]) => `<div><dt>${esc(label)}</dt><dd>${esc(value ?? '—')}</dd></div>`).join('')}</dl>`;
  const time = ms => new Date(ms).toLocaleTimeString('zh-CN', {hour12:false,timeZone:'Asia/Shanghai',hour:'2-digit',minute:'2-digit',second:'2-digit'});
  const typeLabels = {body:'本次会话正文历史',extraction:'本次会话的全部提取文本',hook_events:'Hook 接入日志'};
  function validatePage(page) {
    if (!page?.snapshot || !page?.completeness || !Array.isArray(page.records)) throw new Error('A typed SessionRecordPage is required');
    for (const record of page.records) if (!/^\d+$/.test(record.sequence)) throw new Error('Sequence must remain a decimal string');
    for (let i=1;i<page.records.length;i++) if(BigInt(page.records[i-1].sequence)>=BigInt(page.records[i].sequence)) throw new Error('Each visible page must remain in received order');
  }
  function completenessText(c) {
    return ({complete_received_range:'本次已接收范围均已保存',incomplete:'本次接收存在已知缺口',unknown:'接收完整性待确认',cleaned:'部分原始载荷已清理'})[c.status] || '接收完整性待确认';
  }
  function rangeHTML(page) {
    const s=page.snapshot,c=page.completeness;
    return `<div class="rw-supplement-range" role="status"><div><span>${esc(completenessText(c))}</span><span>本次查看范围固定，晚到内容单独提示</span>${c.cleanedPayloadCount !== '0' ? `<span>已清理 ${esc(c.cleanedPayloadCount)} 条载荷，记录位置保留</span>` : ''}</div><details><summary>查看保存范围与接收详情</summary><dl>${[['本次查看截至',s.highWatermark],['最早可用记录',s.firstAvailable],['最后收到',s.lastReceived],['最后保存',s.lastPersisted],['保留起点',s.retainedFrom],['已加载范围',`${page.loadedRange.first ?? '—'}～${page.loadedRange.last ?? '—'}`],['完整性适用范围','仅已接收到客户端的记录；不代表上游从未丢失'],['缺失数量',c.unknownRange?'未知':c.knownLoss?'详见关联 Hook 事件':'未检测到已知缺失'],...c.notes.map(n=>['范围说明',n])].map(([k,v])=>`<dt>${esc(k)}</dt><dd>${esc(v??'—')}</dd>`).join('')}</dl></details></div>`;
  }
  function bodyHTML(record) {
    const unit=record.unit;
    return `<article class="rw-text-entry" data-body-sequence="${esc(record.sequence)}"><span class="rw-sequence">#${esc(record.sequence)}</span><div><div class="rw-record-meta"><strong>正文</strong><span class="pill">原文 v${esc(unit.textRevision)}</span><time>${esc(time(unit.createdAtMs))}</time></div><p lang="${esc(unit.sourceLanguage)}">${esc(unit.sourceText)}</p>${unit.translations.length ? unit.translations.map(t=>`<p class="rw-history-translation">${esc(t.correction?.text ?? t.text)}<small> · ${esc(t.providerLabel)}</small></p>`).join('') : '<p class="rw-history-translation">尚无译文，原文保持可读。</p>'}<details><summary>查看正文来源</summary>${pairs([['会话',unit.sessionId],['阅读单元',unit.unitId],['原文版本',unit.textRevision],['原文身份',unit.sourceHash],...record.sourceRefs.map((ref,i)=>[`来源 ${i+1}`,`${ref.eventId} · UTF-8 字节 ${ref.byteStart}～${ref.byteEnd}`])])}</details></div></article>`;
  }
  function extractionHTML(record, sources) {
    const source=sources.find(s=>s.sourceId===record.sourceId);
    const associated=record.bodyRefs.length ? `已形成 ${record.bodyRefs.length} 条正文关联` : '未进入字幕';
    return `<article class="rw-text-entry" data-extraction-sequence="${esc(record.sequence)}"><span class="rw-sequence">#${esc(record.sequence)}</span><div><div class="rw-record-meta"><strong>${esc(source?.label ?? record.sourceId)}</strong><span class="pill">${esc(record.reason)}</span><span class="muted">${esc(associated)}</span><time>${esc(time(record.observedAtMs))}</time></div><p lang="ja">${record.payloadRetained ? esc(record.text ?? '') : '<span class="muted">原始载荷已清理。事件位置与正文关联仍保留。</span>'}</p><details><summary>查看完整来源</summary>${pairs([['事件身份',record.eventId],['文本流',record.sourceId],['上游序号',record.wireSequence],['Hook / 地址',source?`${source.hook} / ${source.address}`:'—'],['上下文',source?`${source.context} / ${source.context2}`:'—'],['正文关联',record.bodyRefs.map(r=>`${r.unitId} · v${r.textRevision}`).join('；') || '未形成正文'],['载荷保留',record.payloadRetained?'已保留原始回调文本':'已清理，不能从正文反造原始载荷']])}</details></div></article>`;
  }
  function hookHTML(record) {
    const labels={info:'信息',warning:'警告',error:'错误'};
    return `<article class="rw-log-entry" data-hook-sequence="${esc(record.sequence)}"><span class="rw-sequence">#${esc(record.sequence)}</span><div><div class="rw-record-meta"><strong>${esc(record.stage)}</strong><span class="pill ${record.level==='info'?'':'warn'}">${esc(labels[record.level]??record.level)}</span><time>${esc(time(record.observedAtMs))}</time></div><p>${esc(record.message)}</p><details><summary>查看事件详情</summary>${pairs([['事件代码',record.code],['事件身份',record.eventId],['会话',record.sessionId ?? '会话建立前'],['接入操作',record.operationId],['关联来源',record.sourceId],['上游序号',record.wireSequence],['缺失数量',record.lostCount ?? (record.integrity==='unknown'?'未知':'未报告')],['接收完整性',record.integrity]])}</details></div></article>`;
  }
  function renderRecords(input) {
    const {dataset,page,sources=[],query={filter:{}},follow=true,loadError=false}=input;
    validatePage(page);
    const f=query.filter??{}, s=page.snapshot;
    const description=dataset==='body'?'按接收顺序保留每次正文；完整原始内容及其他来源在“全部提取”中查看。':dataset==='extraction'?'所有已接收来源均可查看；未选中的流不进入阅读字幕。':'从目标核对、接入到恢复的事件，按发生顺序保留。';
    const filters=dataset==='hook_events'?`<select id="supp-hook-level" aria-label="日志级别">${[['','所有级别'],['info','常规事件'],['warning','警告'],['error','错误']].map(([v,l])=>option(v,l,f.level??'')).join('')}</select><input id="supp-hook-stage" aria-label="事件阶段" type="text" placeholder="全部阶段" value="${esc(f.stage??'')}">`:dataset==='extraction'?`<select id="supp-extraction-source" aria-label="文本来源">${option('','全部文本来源',f.sourceId??'')}${sources.map(source=>option(source.sourceId,source.label+' · '+(source.selected?'已选正文':'未选中'),f.sourceId??'')).join('')}</select>`:'';
    return `<section class="${dataset==='body'?'rw-reading-history':'rw-session-list'} rw-supplement-list" data-supplement-dataset="${esc(dataset)}"><div class="rw-list-intro"><div><h2>${typeLabels[dataset]}</h2><p>${description}</p></div><span class="pill">共 ${esc(s.matchingCount)} 条${f.search||f.level||f.stage||f.sourceId?'匹配记录':''}</span></div><div class="rw-list-toolbar"><label class="rw-search">${icon('search')}<input id="supp-session-search" aria-label="${dataset==='hook_events'?'搜索 Hook 日志':dataset==='body'?'搜索正文历史':'搜索提取文本'}" placeholder="${dataset==='hook_events'?'搜索日志':'搜索原文'}" value="${esc(f.search??'')}"></label>${filters}<button type="button" data-supp-action="follow" aria-pressed="${follow}">${icon(follow?'check':'pause')} ${follow?'跟随新内容':`回到最新${s.newerCount!=='0'?' · '+esc(s.newerCount)+' 条新内容':''}`}</button><button type="button" data-supp-action="export">导出…</button></div>${rangeHTML(page)}${page.completeness.status==='incomplete'||page.completeness.status==='unknown'?`<div class="status-banner warning"><div><strong>${esc(completenessText(page.completeness))}</strong><p>已保存内容继续保留。无法确认的缺失不会补造为完整文本。</p></div></div>`:''}<div class="rw-records" role="log" aria-live="off" aria-label="${typeLabels[dataset]}">${page.records.length ? page.records.map(r=>r.kind==='body'?bodyHTML(r):r.kind==='extraction'?extractionHTML(r,sources):hookHTML(r)).join('') : `<div class="rw-list-empty"><strong>${f.search?'没有匹配结果':'等待收到记录'}</strong><p>${f.search?'试试其他关键词或清除筛选。':'收到后会按顺序出现在这里。'}</p></div>`}</div><div class="rw-list-footer"><span>已显示 ${page.records.length} / ${esc(s.matchingCount)} 条${f.search?'匹配记录':''} · 记录 ${esc(page.loadedRange.first??'—')}～${esc(page.loadedRange.last??'—')}${s.newerCount!=='0'?`<br>新到 ${esc(s.newerCount)} 条，当前查看位置保持不变`:''}</span>${loadError?'<span class="rw-inline-error" role="alert">加载较早记录失败，已显示内容保留。</span><button type="button" data-supp-action="retry-more">重试</button>':page.nextCursor?'<button type="button" data-supp-action="load-more">加载较早记录</button>':'<span class="muted">已到记录起点</span>'}</div></section>`;
  }
  // Reviewed policy: browsing more than 32 logical pixels above latest pauses
  // follow. Geometry is read only from this independent design DOM.
  function bindSessionFollow(root,input) {
    const records=root.querySelector('.rw-records'),button=root.querySelector('[data-supp-action="follow"]');
    if(!records||!button)return;
    let follow=input.follow!==false;
    const render=()=>{button.setAttribute('aria-pressed',String(follow));button.innerHTML=icon(follow?'check':'pause')+' '+(follow?'跟随新内容':'回到最新'+(input.page.snapshot.newerCount!=='0'?' · '+esc(input.page.snapshot.newerCount)+' 条新内容':''));};
    if(follow)records.scrollTop=records.scrollHeight;
    render();
    records.addEventListener('scroll',()=>{if(records.scrollHeight-records.clientHeight-records.scrollTop>32&&follow){follow=false;render();}});
    button.addEventListener('click',()=>{follow=!follow;if(follow)records.scrollTop=records.scrollHeight;render();});
  }
  function applySession(root,input) {
    const footer=root.querySelector('.sidebar-session');if(footer)[...footer.childNodes].filter(n=>n.nodeType===3&&n.textContent.trim()==='原文接收中').forEach(n=>n.textContent='正在接收正文');
    const tab={body:'reading',extraction:'texts',hook_events:'logs'}[input.dataset];
    const tabs=root.querySelector('.rw-session-tabs');
    if(!tabs) throw new Error('Reviewed session tabs missing');
    tabs.querySelectorAll('[role=tab]').forEach(b=>{const selected=b.dataset.rwTab===tab;b.setAttribute('aria-selected',String(selected));b.tabIndex=selected?0:-1;});
    const panel=root.querySelector('.rw-session-panel');
    const preview=panel?.querySelector('.rw-preview');if(preview){const identity=preview.querySelector('.rw-preview-meta>span>small');if(identity)identity.textContent='会话 '+input.query.sessionId;}
    const walking=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);while(walking.nextNode()){const node=walking.currentNode;if(node.textContent.includes('正在接收原文'))node.textContent=node.textContent.replace('正在接收原文','正在接收正文');}
    if(!panel)throw new Error('Reviewed session panel missing');
    panel.id='session-panel-'+tab;panel.setAttribute('aria-labelledby','session-tab-'+tab);
    if(input.dataset==='body') {
      const old=panel.querySelector('.rw-reading-history');
      if(!old) throw new Error('Reviewed reading body must be retained before adding full history');
      const overview=panel.querySelector('.reading-unit');const unit=input.page.records.find(r=>r.kind==='body')?.unit;
      if(overview&&unit){const summary=unit.sourceSummary;const origin=summary?.sourceKind==='hook'?'游戏原文':summary?.sourceKind==='manual'?'手动输入':'来源未记录';const footer=overview.querySelector('.unit-meta span');if(footer)footer.textContent=origin+' · '+footer.textContent;}
      const actions=panel.querySelector('.session-controls');if(actions){const copy=document.createElement('button');copy.type='button';copy.dataset.suppAction='copy-fixed-unit';copy.textContent='复制原文';actions.append(copy);}
      old.outerHTML=renderRecords(input);
    } else panel.innerHTML=renderRecords(input);
    return root;
  }
  const row=(label,value,help='')=>`<div class="ls-row"><div class="ls-row-label">${label}</div><div class="ls-row-value">${value}${help?`<div class="ls-help">${help}</div>`:''}</div></div>`;
  const section=(title,body,note='')=>`<section class="ls-section"><h3>${title}</h3><div class="ls-form">${body}</div>${note?`<p class="ls-section-note">${note}</p>`:''}</section>`;
  const formattedShortcut=value=>String(value??'').split('+').map(part=>({alt:'⌥',super:'⌘',control:'⌃',shift:'⇧'}[part.toLowerCase()]??part.toUpperCase())).join(' ');
  const keys=value=>value?`<span class="rw-supplement-key"><kbd>${esc(formattedShortcut(value))}</kbd></span>`:'<span class="ls-muted">未设置</span>';
  function applyPreferences(root,input) {
    const {preferences:p,capabilities:c,draft,dirty=false,game}=input;
    const field=(name,value)=>{const n=root.querySelector(`[data-settings-field="${name}"]`);if(!n)throw new Error('Reviewed preference control missing: '+name);if(n.type==='checkbox'){n.checked=Boolean(value);n.toggleAttribute('checked',n.checked);}else{n.value=String(value);if(n.tagName==='SELECT'){[...n.options].forEach(o=>o.toggleAttribute('selected',o.value===n.value));const text=n.nextElementSibling?.querySelector('.luna-select-value');if(text)text.textContent=n.selectedOptions[0]?.textContent??'';}else n.setAttribute('value',n.value);}return n;};
    field('display.fontSize',draft.translationFontPt);
    field('display.sourceSize',draft.sourceFontPt);
    const opacity=field('display.opacity',Math.round(draft.hudBackgroundOpacity*100));
    if(opacity.nextElementSibling)opacity.nextElementSibling.textContent=Math.round(draft.hudBackgroundOpacity*100)+'%';
    field('display.pin',draft.overrides['hud.alwaysOnTop']??true);
    field('preferences.preset',draft.hudWidthPt===500?'compact':'comfortable');
    field('preferences.fontScale',1);
    field('preferences.appearance',draft.transparencyMode==='opaque'?'reduced':'standard');
    const sidebarFoot=root.querySelector('.ls-sidebar-foot');
    if(sidebarFoot)sidebarFoot.innerHTML='个人配置 · 默认<br><span>全局与游戏覆盖</span>';
    const scope=p.scopeKey==='global'?'全局默认':`仅此游戏 · ${game.title}`;
    const scopeHTML=row('作用范围',`<select id="supp-pref-scope" aria-label="阅读偏好作用范围" ${dirty?'disabled':''}>${option('global','全局默认',p.scopeKey)}${option('game:'+game.id,'仅此游戏 · '+game.title,p.scopeKey)}</select>`,dirty?'先保存或放弃当前修改，再切换作用范围。':'查看当前来源；修改此范围不会更换游戏或重新接入。')+(p.scopeKey==='global'?'':row('当前来源','<span class="ls-effective">此游戏的已保存偏好</span><button class="ls-button" type="button" data-supp-action="restore-global">恢复继承全局</button>','此游戏有已保存偏好时优先采用；恢复继承后使用全局已保存偏好。'));
    const scopeSection=section('阅读偏好与来源',scopeHTML);
    const scroll=root.querySelector('.ls-scroll');
    if(!scroll)throw new Error('Reviewed settings scroll host missing');
    const heading=scroll.querySelector('.ls-content-heading');
    heading.insertAdjacentHTML('afterend',scopeSection);
    const oldShortcut=[...scroll.querySelectorAll('.ls-section')].find(n=>n.querySelector('h3')?.textContent==='快捷键');
    if(!oldShortcut)throw new Error('Reviewed shortcuts section missing');
    const shortcutLabels={toggle_hud:'显示 / 隐藏浮窗',toggle_capture_pause:'暂停 / 继续取文',cycle_reading_display_mode:'原文与译文模式'};
    const shortcutFields={toggle_hud:'toggleHud',toggle_capture_pause:'togglePause',cycle_reading_display_mode:'cycleDisplayMode'};
    oldShortcut.outerHTML=section('快捷键',c.shortcuts.bindings.map((binding,i)=>row(shortcutLabels[binding.intent],`<input type="text" id="supp-shortcut-${i}" aria-label="${shortcutLabels[binding.intent]}快捷键草稿" value="${esc(draft.shortcuts[shortcutFields[binding.intent]]??binding.desired??'')}" placeholder="未设置"><div class="rw-supplement-source">当前生效：${keys(binding.effective)} · ${esc(binding.scopeKey==='global'?'全局默认':'此游戏覆盖')}</div>${binding.errorMessage?`<p class="ls-help ls-error-text" role="alert">${esc(binding.errorMessage)}</p>`:''}`)).join('')+row('快捷键应用','<div class="rw-shortcut-buttons"><button class="ls-button" type="button" data-supp-action="clear-shortcuts">清除</button><button class="ls-button" type="button" data-supp-action="restore-shortcuts">恢复默认</button><button class="ls-button" type="button" data-supp-action="apply-shortcuts" '+(dirty?'':'disabled')+'>应用快捷键</button></div>','编辑与当前生效绑定分开，失败保留原组合。')+row('位置恢复','<button class="ls-button" type="button" data-supp-action="reset-hud">恢复浮窗位置</button>','拖动顶部手柄或底部空白区域，隐藏再打开保留位置。'),'显示 / 隐藏不暂停取文；原译模式只改变显示，不改变取文与翻译状态。');
    const stateLabels={active:'已注册',draft:'尚未应用',validating:'正在校验',registering:'正在注册',failed:'应用失败，原生效组合保留',cleanup_pending:'正在清理旧绑定'};
    scroll.querySelector('#rw-shortcut-status')?.remove();
    if(input.saveError)scroll.insertAdjacentHTML('beforeend',`<p class="ls-boundary ls-error-text" role="alert">${esc(input.saveError)} 编辑内容已保留，已生效的快捷键以平台回执为准。</p>`);
    scroll.insertAdjacentHTML('beforeend',`<p class="rw-supplement-shortcut-status" role="status"><strong>${esc(stateLabels[c.shortcuts.state]??'状态待确认')}</strong>${c.shortcuts.dispatchSuppressed?' · 正在录制快捷键，产品快捷动作暂不响应':' · 快捷动作按当前生效组合响应'}</p>`);
    const preset=root.querySelector('[data-settings-field="preferences.preset"]');const presetHelp=preset?.closest('.ls-row-value')?.querySelector('.ls-help');if(presetHelp)presetHelp.textContent='预设为有效宽度 / 原文字号 / 译文字号。下方字号为舒适基数，紧凑按比例显示。';
    const materialSection=[...scroll.querySelectorAll('.ls-section')].find(n=>n.querySelector('h3')?.textContent==='阅读布局');
    const actualMaterial=c.material.effective==='native_material'?'原生磨砂材质':c.material.effective==='opaque_fallback'?'实色回退':'当前平台不支持';
    materialSection.insertAdjacentHTML('afterend',section('当前材质状态',row('实际生效',`<span class="ls-effective">${actualMaterial}</span>${c.material.systemReduceTransparency?'<span class="ls-badge">系统减少透明度</span>':''}`,c.material.effectiveReduceTransparency?'背景采用实色，文字保持原有对比与清晰度。':'背景不透明度只改变底板，文字保持原有清晰度。')+(c.material.errorCode?row('应用说明',`<span class="ls-error-text">${esc(c.material.errorCode)}</span>`):'')));
    const footer=root.querySelector('.ls-footer');
    footer.innerHTML=`<div class="ls-footer-status">${dirty?'阅读偏好有未保存修改':'阅读偏好已保存'}<br>${esc(scope)} · 模型用途配置保持不变</div><div class="ls-footer-buttons"><button class="ls-button" type="button" data-supp-action="discard-preferences" ${dirty?'':'disabled'}>放弃修改</button><button class="ls-button ls-primary" type="button" data-supp-action="save-preferences" ${dirty?'':'disabled'}>保存阅读偏好</button></div>`;
    return root;
  }
  window.ReviewedReferenceSupplement={kind:'independent-supplement-candidate-not-approved',renderRecords,applySession,applyPreferences,bindSessionFollow};
})();
