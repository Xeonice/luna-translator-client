/* Candidate-only presentation adapter. Canonical extraction, Hook, service,
   versioning, cancellation and learning logic remain the source of behavior. */
(() => {
  'use strict';
  const app=window.LunaDesign,gaps=window.LunaReviewGaps;
  const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const clone=value=>JSON.parse(JSON.stringify(value));
  const baseSession=gaps.sessionHTML,baseAfter=gaps.afterRender;
  const panels=new Map();let sessionId=null;
  function panel(key) {if(!panels.has(key))panels.set(key,{query:'',stream:'all',level:'all',logScope:'diagnostic',follow:true,limit:12,loadError:false,open:false,frozenEnd:null,scrollTop:0});return panels.get(key);}
  function bucket() {return gaps.sessions.get(app.state.session.sessionId)||{records:[],logs:[]};}
  function activate(key) {const state=panel(key),data=bucket(),end=key==='logs'?data.logs.at(-1)?.id||0:data.records.at(-1)?.sequence||0;Object.assign(gaps.view,state,{tab:key,newCount:state.follow?0:(key==='logs'?data.logs.filter(r=>r.id>state.frozenEnd).length:data.records.filter(r=>r.sequence>state.frozenEnd).length),frozenEnds:{texts:key==='texts'?state.frozenEnd:null,logs:key==='logs'?state.frozenEnd:null}});return {state,end};}
  function pause(key) {const {state,end}=activate(key);if(state.follow){state.follow=false;state.frozenEnd=end;}}
  function updateFollowControl(key) {
    const state=panel(key),data=bucket(),records=key==='logs'?data.logs:key==='reading'?data.records.filter(row=>row.selected):data.records;
    const count=state.follow?0:records.filter(row=>(key==='logs'?row.id:row.sequence)>state.frozenEnd).length;
    const node=document.querySelector('[data-flat-session-section="'+key+'"] [data-flat-list-action="toggle-follow"]');
    if(node){node.setAttribute('aria-pressed',String(state.follow));node.textContent=state.follow?'跟随新内容':'回到最新'+(count?' · '+count+' 条新内容':'');}
  }
  function preserveScroll() {document.querySelectorAll('[data-flat-session-section]').forEach(node=>{const list=node.querySelector('.rw-records');if(list)panel(node.dataset.flatSessionSection).scrollTop=list.scrollTop;});}
  function renderPanel(key,readingHTML) {
    const {state}=activate(key),data=bucket(),all=key==='logs'?data.logs:data.records;
    const visibleTotal=key==='logs'&&state.logScope!=='all'?all.filter(row=>!row.noise).length:all.length;
    const warnings=key==='logs'?all.filter(row=>row.level==='warning'||row.level==='error').length:0;
    const root=document.createElement('div');root.innerHTML=baseSession(app,readingHTML);
    const content=root.querySelector(key==='logs'?'#hook-events':'#session-history');if(!content)return '';
    content.querySelector('.rw-list-intro')?.remove();
    content.querySelectorAll('[id]').forEach(node=>{if(['rw-session-search','rw-session-filter','rw-log-scope'].includes(node.id)){node.dataset.flatField=node.id;node.id='flat-'+key+'-'+node.id;}});
    content.querySelectorAll('[data-rw-action]').forEach(node=>{node.dataset.flatListAction=node.dataset.rwAction;delete node.dataset.rwAction;});
    const title=key==='logs'?'Hook 诊断':'全部提取';
    const description=key==='logs'?'接入阶段与警告，按需查看原生事实':'含选流前、未选来源与合并前原文';
    return '<details class="flat-session-section" data-flat-session-section="'+key+'" '+(state.open?'open':'')+'><summary><span><strong>'+title+'</strong><small>'+description+'</small></span><span class="flat-summary-count">'+(key==='logs'?visibleTotal+' / '+all.length+' 条'+(warnings?' · '+warnings+' 条警告':''):all.length+' 条')+'</span><span class="flat-summary-action">'+(state.open?'收起':'查看')+'</span></summary>'+content.outerHTML+'</details>';
  }
  gaps.sessionHTML=function(target,readingHTML) {
    if(!target.state.session.gameId)return baseSession(target,readingHTML);
    if(sessionId!==target.state.session.sessionId){panels.clear();sessionId=target.state.session.sessionId;}
    const saved=clone(gaps.view);Object.assign(gaps.view,{tab:'reading',readingLimit:bucket().records.length});
    const root=document.createElement('div');root.innerHTML=baseSession(target,readingHTML);
    const heading=root.querySelector('.page-header'),reading=root.querySelector('#session-panel-reading'),preview=root.querySelector('.rw-preview-column');
    reading.removeAttribute('role');reading.removeAttribute('aria-labelledby');reading.id='flat-reading';reading.dataset.flatSection='reading';
    // Present each received sentence once. The latest body item carries the
    // current-result attachment point used by the shared comparison renderer.
    reading.querySelector('.reading-unit')?.remove();
    if(preview){const manage=document.createElement('details');manage.className='flat-preview-management';manage.innerHTML='<summary>窗口关联管理</summary>';reading.querySelectorAll('[data-action="bind-window"],[data-action="unbind-window"]').forEach(button=>manage.append(button));preview.append(manage);}
    reading.querySelectorAll('.form-row').forEach(row=>{if(row.querySelector('.label')?.firstChild?.textContent==='运行窗口')row.remove();else row.querySelectorAll('.label small').forEach(note=>note.remove());});
    const bodyState=panel('reading'),allBody=bucket().records.filter(row=>row.selected),matchedBody=allBody.filter(row=>(!bodyState.query||row.source.toLowerCase().includes(bodyState.query.toLowerCase()))&&(bodyState.stream==='all'||row.streamId===bodyState.stream)),eligibleBody=bodyState.follow||bodyState.frozenEnd===null?matchedBody:matchedBody.filter(row=>row.sequence<=bodyState.frozenEnd),shownBody=eligibleBody.slice(-bodyState.limit).reverse();
    const records=allBody.slice().reverse();
    reading.querySelectorAll('.rw-reading-history .rw-text-entry').forEach((node,index)=>{
      const event=records[index];if(!shownBody.includes(event)){node.remove();return;}const record=app.state.history.find(row=>row.id===event?.unitId);if(!record)return;
      if(index===0)node.classList.add('reading-unit');
      node.dataset.flatHistoryRecord=record.recordId;
      node.insertAdjacentHTML('beforeend','<div class="flat-record-actions"><button data-action="copy-history">复制</button><button data-action="retranslate-history">重译</button><button data-action="learn-history">精读这句</button></div>');
    });
    const bodyHistory=reading.querySelector('.rw-reading-history');if(bodyHistory){bodyHistory.dataset.flatSessionSection='reading';bodyHistory.querySelector('.help-note')?.remove();bodyHistory.querySelector('.rw-list-footer')?.remove();
      bodyHistory.querySelector('.section-heading').insertAdjacentHTML('afterend','<div class="rw-list-toolbar"><label class="rw-search"><input id="flat-reading-rw-session-search" data-flat-field="rw-session-search" aria-label="搜索正文历史" placeholder="搜索本会话正文" value="'+esc(bodyState.query)+'"></label><select id="flat-reading-rw-session-filter" data-flat-field="rw-session-filter" aria-label="正文来源"><option value="all">全部正文来源</option>'+app.game().streams.filter(stream=>allBody.some(row=>row.streamId===stream.id)).map(stream=>'<option value="'+esc(stream.id)+'" '+(bodyState.stream===stream.id?'selected':'')+'>'+esc(stream.label)+'</option>').join('')+'</select><button data-flat-list-action="toggle-follow" aria-pressed="'+bodyState.follow+'">'+(bodyState.follow?'跟随新内容':'回到最新 · '+allBody.filter(row=>row.sequence>bodyState.frozenEnd).length+' 条新内容')+'</button><button data-flat-list-action="export-session">导出…</button></div>');
      bodyHistory.insertAdjacentHTML('beforeend','<div class="rw-list-footer"><span>已显示 '+shownBody.length+' / '+matchedBody.length+' 条正文'+(bodyState.query?'匹配':'')+'</span>'+(shownBody.length<eligibleBody.length?'<button data-flat-list-action="load-more">加载较早正文</button>':'<span>已到记录起点</span>')+'</div>');if(!shownBody.length)bodyHistory.querySelector('.rw-records').innerHTML='<p class="flat-empty">'+(bodyState.query?'没有匹配的正文':'等待收到正文')+'</p>';
    }
    const aux='<div class="flat-session-aux" data-flat-section="session-tools">'+renderPanel('texts',readingHTML)+renderPanel('logs',readingHTML)+'</div>';
    Object.assign(gaps.view,saved,{tab:'reading'});
    return (heading?.outerHTML||'')+aux+'<div class="rw-session-layout flat-session-layout">'+reading.outerHTML+(preview?.outerHTML||'')+'</div>';
  };
  function updateReviewTools() {
    const s=app.state.settings;
    let select=document.getElementById('flat-review-test-outcome');
    if(!select){const label=document.createElement('label');label.textContent='配置测试 ';select=document.createElement('select');select.id='flat-review-test-outcome';select.setAttribute('aria-label','配置样句模拟结果');select.innerHTML='<option value="pass">契约通过</option><option value="auth">认证失败</option><option value="timeout">连接超时</option><option value="structure">输出不合格</option>';select.addEventListener('change',()=>{s.testOutcome=select.value;});label.append(select);document.querySelector('.review-bar>div')?.append(label);}
    select.value=s.testOutcome;
  }
  gaps.afterRender=function(target,...args) {
    // The old tab's page-scroll following must not move the entire flat page.
    gaps.view.tab='reading';baseAfter(target,...args);
    document.querySelectorAll('[data-flat-session-section]').forEach(node=>{
      const state=panel(node.dataset.flatSessionSection),list=node.querySelector('.rw-records');if(list)list.scrollTop=state.follow?(node.dataset.flatSessionSection==='reading'?0:list.scrollHeight):state.scrollTop;
    });
    document.querySelectorAll('[data-flat-disclosure]').forEach(node=>node.addEventListener('toggle',()=>{const s=app.state.settings;(s.flatOpen||(s.flatOpen={}))[node.dataset.flatDisclosure]=node.open;}));
    document.querySelectorAll('details[data-flat-session-section]').forEach(node=>node.addEventListener('toggle',()=>{panel(node.dataset.flatSessionSection).open=node.open;const action=node.querySelector('.flat-summary-action');if(action)action.textContent=node.open?'收起':'查看';}));
    updateReviewTools();
  };
  function fire(attribute,action) {const button=document.createElement('button');button.hidden=true;button.setAttribute(attribute,action);document.body.append(button);button.click();button.remove();}
  document.addEventListener('click',event=>{
    if(event.target.closest('#dialog-confirm')&&document.getElementById('rw-export-kind')){const kind=document.getElementById('rw-export-kind').value;activate(kind==='body'?'reading':kind);}
    const summary=event.target.closest('summary');if(summary&&summary.parentElement.matches('[data-flat-disclosure]')){const d=summary.parentElement,s=app.state.settings;(s.flatOpen||(s.flatOpen={}))[d.dataset.flatDisclosure]=!d.open;}
    if(summary&&summary.parentElement.matches('details[data-flat-session-section]')){const d=summary.parentElement;panel(d.dataset.flatSessionSection).open=!d.open;}
    const history=event.target.closest('[data-flat-history-record]');if(history&&event.target.closest('[data-action]'))app.state.historySelection=history.dataset.flatHistoryRecord;
    const purpose=event.target.closest('[data-flat-purpose]');if(purpose)app.state.settings.purpose=purpose.dataset.flatPurpose;
    const add=event.target.closest('[data-flat-add]');if(add){
      event.preventDefault();event.stopImmediatePropagation();const kind=add.dataset.flatAdd,s=app.state.settings;app.closeDialog();
      if(kind==='offline'){s.engineFamily='offline';s.purpose='translation';fire('data-local-action','new');}
      else {fire('data-settings-action','new-connection');fire('data-settings-action','new-model');const c=s.draft.connections[s.selectedConnection],m=s.draft.models[s.selectedModel];s.engineFamily=kind;s.purpose='translation';c.protocol=kind==='cloud'?'traditional':'openai';c.name=kind==='cloud'?'新云端翻译':'新大模型连接';m.name=kind==='cloud'?'新翻译配置':'新大模型配置';m.capability=kind==='cloud'?'translation':'text';m.templatePurpose='translation';fire('data-settings-action','reset-template');}
      app.render();return;
    }
    const control=event.target.closest('[data-flat-list-action]');if(!control)return;
    event.preventDefault();event.stopImmediatePropagation();preserveScroll();const key=control.closest('[data-flat-session-section]').dataset.flatSessionSection,state=panel(key),action=control.dataset.flatListAction;
    if(action==='toggle-follow'){if(state.follow)pause(key);else{state.follow=true;state.frozenEnd=null;}}
    else if(action==='load-more'||action==='retry-more'){state.limit+=12;state.loadError=false;}
    else if(action==='export-session'){
      activate(key);fire('data-rw-action','export-session');
      const select=document.getElementById('rw-export-kind');if(select)select.value=key==='reading'?'body':key;
      return;
    }
    app.render();
  },true);
  ['input','change'].forEach(type=>document.addEventListener(type,event=>{
    const purpose=event.target.closest('[data-flat-purpose]');if(purpose)app.state.settings.purpose=purpose.dataset.flatPurpose;
    const field=event.target.closest('[data-flat-field]');if(!field)return;
    if(field.dataset.flatField==='rw-session-search'&&type!=='input'||field.dataset.flatField!=='rw-session-search'&&type!=='change')return;
    event.stopImmediatePropagation();preserveScroll();const key=field.closest('[data-flat-session-section]').dataset.flatSessionSection,state=panel(key),name=field.dataset.flatField,position=field.selectionStart;
    pause(key);if(name==='rw-session-search')state.query=field.value;else if(name==='rw-log-scope')state.logScope=field.value;else state[key==='logs'?'level':'stream']=field.value;
    state.limit=12;app.render();const replacement=document.getElementById(field.id);replacement?.focus({preventScroll:true});if(name==='rw-session-search')replacement?.setSelectionRange(position,position);
  },true));
  document.addEventListener('wheel',event=>{const section=event.target.closest('[data-flat-session-section]');if(!section||event.deltaY>=0)return;preserveScroll();pause(section.dataset.flatSessionSection);updateFollowControl(section.dataset.flatSessionSection);}, {passive:true,capture:true});
  window.LunaFlatDesign={panels,savedServices:()=>window.LunaTranslationServices.list(app),version:1};
  app.render();
})();
