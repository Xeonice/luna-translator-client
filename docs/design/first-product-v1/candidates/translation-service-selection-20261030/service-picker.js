/* Pending design prototype. Saved resources and selection are separate projections.
   No network, filesystem, native engine or production persistence is used here. */
(() => {
  'use strict';
  const app = window.LunaDesign, settings = window.LunaSettings;
  if (!app || !settings) return;
  const clone = value => JSON.parse(JSON.stringify(value));
  const esc = app.escape, icon = name => window.LunaIcons?.svg(name) || '';
  const ui = {origin:'main',scope:'game',gameId:null,returnContext:null,returnFocus:null,extraToolId:'',extraOutcomes:{}};
  const explicitRequests=new Map();let explicitSequence=0;
  const originalEffective = settings.effective;
  const families = {cloud:'云端翻译',offline:'本机离线翻译',llm:'大模型服务'};
  const local = id => window.LunaLocalTranslation.isLocal(id);
  const currentGame = () => app.state.session.gameId || null;
  function gameName(id) {
    return (window.LunaLibrary?.allGames?.() || window.LunaDesignData.games).find(g => g.id === id)?.title || '当前游戏';
  }
  function nameFor(snapshot,id) {
    return local(id) ? snapshot.localEngines?.[id.slice(8)]?.name : snapshot.models?.[id]?.name;
  }
  function fromApplied(entry,gameId) {
    const o=gameId && entry?.snapshot.overrides?.[gameId]?.translation;
    const b=o?.mode==='override'?o.backup:entry?.snapshot.bindings.translation || {};
    return {mainId:b.modelId || '',extraIds:[...new Set(b.extraModels || [])].filter(id=>id!==b.modelId),snapshot:clone(entry?.snapshot || app.state.settings.saved),versions:{},enabled:!!b.enabled,revision:entry?.revision || 1};
  }
  function ensure(target=app) {
    if (!target.state.translationServices) {
      const s=target.state.settings,global=fromApplied(s.applied['global:translation']);
      const games={};
      Object.entries(s.applied).forEach(([key,entry])=>{
        const [gid,purpose]=key.split(':');
        if(gid!=='global'&&purpose==='translation'&&entry.snapshot.overrides?.[gid]?.translation?.mode==='override')games[gid]=fromApplied(entry,gid);
      });
      target.state.translationServices={global,games,sequence:global.revision,enabledByScope:{}};
    }
    return target.state.translationServices;
  }
  function activeEntry(target,gameId) {
    const store=ensure(target);
    return gameId&&store.games[gameId] ? store.games[gameId] : store.global;
  }
  function selectedSnapshot(entry,id=entry.mainId) {return entry.versions?.[id] || entry.snapshot;}
  function purposeEnabled(target,entry,gameId) {
    const flags=ensure(target).enabledByScope||{};
    if(gameId&&Object.hasOwn(flags,gameId))return flags[gameId];
    if(Object.hasOwn(flags,'global'))return flags.global;
    return entry.enabled;
  }
  function selection(target=app,gameId=target.state.session.gameId) {
    const store=ensure(target),entry=activeEntry(target,gameId),own=!!gameId&&!!store.games[gameId];
    return {mainId:entry.mainId,mainName:nameFor(selectedSnapshot(entry),entry.mainId)||'尚未选择',extraIds:[...entry.extraIds],revision:entry.revision,enabled:purposeEnabled(target,entry,gameId),scope:own?'game':'global',gameId:gameId||null,source:own?gameName(gameId)+' · 本游戏专用':gameId?'继承全局默认':'全局默认'};
  }
  function statusFor(target,id,snapshot) {
    const s=target.state.settings;
    if(local(id)) {
      const e=snapshot.localEngines?.[id.slice(8)];
      return e ? window.LunaLocalTranslation.selectionStatus(s,e) : {available:false,status:'已移除',reason:'本机服务配置已移除。',configVersion:0};
    }
    return settings.serviceStatus(s,id,snapshot);
  }
  function list(target=app) {
    const s=target.state.settings,d=s.saved;
    const models=Object.values(d.models || {}).filter(m=>m.templatePurpose==='translation').map(m=>{
      const c=d.connections[m.connectionId];
      return {id:m.id,name:m.name,family:c?.protocol==='traditional'?'cloud':'llm',location:c?.location==='local'?'本机 HTTP':c?.location==='remote'?'远程自托管':'云端',configuration:'saved',revision:s.revision,...statusFor(target,m.id,d)};
    });
    const engines=Object.values(d.localEngines || {}).filter(e=>e.added===true).map(e=>({id:'offline:'+e.id,name:e.name,family:'offline',location:'本机',configuration:'saved',revision:s.revision,...statusFor(target,'offline:'+e.id,d)}));
    return models.concat(engines);
  }
  function selectedStatus(target,entry) {
    const saved=target.state.settings.saved,id=entry.mainId;
    if(!(local(id)?saved.localEngines?.[id.slice(8)]?.added:saved.models?.[id]))return {available:false,status:'未配置',reason:'所选服务尚未添加并保存，或已移除。'};
    return statusFor(target,id,selectedSnapshot(entry));
  }
  settings.effective = function(s,purpose,gameId) {
    if(purpose!=='translation'||s!==app.state.settings)return originalEffective(s,purpose,gameId);
    const entry=activeEntry(app,gameId),status=selectedStatus(app,entry),sel=selection(app,gameId);
    const snapshot=selectedSnapshot(entry),m=snapshot.models?.[entry.mainId],c=m&&snapshot.connections?.[m.connectionId];
    const enabled=purposeEnabled(app,entry,gameId);
    return {enabled:enabled&&status.available,connectionEnabled:status.available,modelId:local(entry.mainId)?entry.mainId:m?.modelId||entry.mainId,serviceId:entry.mainId,modelName:sel.mainName,connectionName:local(entry.mainId)?'本机离线翻译':c?.name||'待配置',outputMode:'text',historyCount:local(entry.mainId)?0:Number(snapshot.bindings.translation.historyCount)||0,revision:entry.revision,status:status.available?(enabled?status.status:'自动翻译已关闭'):status.reason,source:sel.source};
  };
  function viewSelection() { return selection(app,ui.scope==='game'?ui.gameId:null); }
  function mutableEntry() {
    const store=ensure(app);
    if(ui.scope==='game'&&ui.gameId) {
      if(!store.games[ui.gameId])store.games[ui.gameId]=clone(store.global);
      return store.games[ui.gameId];
    }
    return store.global;
  }
  function choose(id,{role='main'}={}) {
    const item=list(app).find(s=>s.id===id);
    if(!item?.available)return false;
    const entry=mutableEntry(),store=ensure(app),before=app.state.session.translationEnabled,adding=role!=='extra'||!entry.extraIds.includes(id);
    if(role==='extra') {
      if(entry.mainId===id)return false;
      entry.extraIds=entry.extraIds.includes(id)?entry.extraIds.filter(x=>x!==id):[...new Set([...entry.extraIds,id])];
    } else {
      entry.mainId=id;entry.extraIds=entry.extraIds.filter(x=>x!==id);
    }
    // Selection freezes one saved resource version. Saving the editor never reaches here.
    if(!entry.versions)entry.versions={};if(adding)entry.versions[id]=clone(app.state.settings.saved);entry.revision=++store.sequence;
    app.render();
    if(role==='main')close();else renderPicker();
    app.notify('已选择'+item.name+'；用于后续台词。自动翻译保持'+(before?'开启':'关闭')+'，当前句与历史保留。');
    return true;
  }
  function setScope(scope) { ui.scope=scope==='game'&&ui.gameId?'game':'global';renderPicker(); }
  function inheritGame() {
    if(!ui.gameId)return false;
    delete ensure(app).games[ui.gameId];
    app.render();renderPicker();return true;
  }
  const dialog=document.createElement('dialog');dialog.id='translation-service-picker';dialog.className='service-picker-dialog';
  dialog.setAttribute('aria-labelledby','translation-service-title');dialog.setAttribute('aria-describedby','translation-service-description');document.body.append(dialog);
  function open({origin='main',scope,gameId}={}) {
    ui.origin=origin;ui.gameId=gameId!==undefined?gameId:currentGame() || (origin==='settings'?app.state.selectedGame:null);
    ui.scope=scope==='global'?'global':ui.gameId?'game':'global';
    ui.returnFocus=document.activeElement;
    window.LunaControls?.close();renderPicker();
    if(!dialog.open)dialog.showModal();
    updateExpanded();
    requestAnimationFrame(()=>dialog.querySelector('[data-service-action="choose"]:not(:disabled),[data-service-action="configure"]')?.focus({preventScroll:true}));
  }
  function close() {
    if(dialog.open)dialog.close();updateExpanded();
    const fallback=document.getElementById(ui.origin==='hud'?'hud-translation-service':'session-translation-service');
    (ui.returnFocus?.isConnected?ui.returnFocus:fallback)?.focus({preventScroll:true});
  }
  function renderPicker() {
    const items=list(app),sel=viewSelection(),game=ui.gameId;
    const scopeOptions='<option value="global" '+(ui.scope==='global'?'selected':'')+'>全局默认</option>'+(game?'<option value="game" '+(ui.scope==='game'?'selected':'')+'>'+esc(gameName(game))+' · 本游戏</option>':'');
    dialog.innerHTML='<header class="service-picker-heading"><div><h2 id="translation-service-title">选择翻译服务</h2><p id="translation-service-description">只显示已添加并保存的翻译配置。</p></div><button type="button" class="service-close" data-service-action="close" aria-label="取消选择翻译服务">'+icon('x')+'</button></header>'+
      '<div class="service-picker-scope"><label for="translation-service-scope">作用范围</label><select id="translation-service-scope" data-service-field="scope" aria-label="翻译服务作用范围">'+scopeOptions+'</select></div><div class="service-source-line"><span>'+esc(sel.source)+'</span>'+(ui.scope==='game'&&sel.scope==='game'?'<button type="button" class="service-weak" data-service-action="inherit">恢复继承全局</button>':'')+'</div>'+
      '<div class="service-picker-list" aria-label="已保存的翻译服务">'+(items.length?items.map(item=>{
        const selected=sel.mainId===item.id,extra=sel.extraIds.includes(item.id),description='service-reason-'+item.id.replace(/[^a-zA-Z0-9_-]/g,'-');
        return '<article class="service-item '+(selected?'is-selected':'')+'" data-service-id="'+esc(item.id)+'" data-service-available="'+item.available+'"><div class="service-item-top"><button type="button" class="service-choice" data-service-action="choose" data-value="'+esc(item.id)+'" aria-pressed="'+selected+'" aria-describedby="'+description+'" '+(!item.available?'disabled':'')+'><span class="service-check" aria-hidden="true">'+(selected?icon('check'):'')+'</span><span class="service-item-title"><strong>'+esc(item.name)+'</strong><small>'+families[item.family]+' · '+esc(item.location)+' · v'+item.configVersion+'</small></span><span class="service-verification '+(!item.available?'is-blocked':'')+'">'+esc(item.status)+'</span></button></div><div class="service-item-bottom"><p id="'+description+'">'+esc(item.reason||(selected?'当前主服务':'可选为主服务；'+(item.verification==='unverified'?'真实可用性尚未验证':'检测为设计模拟')))+'</p><button type="button" class="service-weak" data-service-action="configure" data-value="'+esc(item.id)+'">配置'+'</button></div><label class="service-extra"><input type="checkbox" data-service-extra="'+esc(item.id)+'" '+(extra?'checked ':'')+(!item.available||selected?'disabled ':'')+'>附加对照'+(selected?' · 主服务不重复添加':'')+'</label></article>';
      }).join(''):'<div class="service-empty"><span>'+icon('languages')+'</span><strong>还没有已保存的翻译服务</strong><p>先添加并保存配置，再回到这里选择。</p><button type="button" data-service-action="configure">配置翻译服务</button></div>')+'</div>'+
      '<footer class="service-picker-footer"><p>选择一次即用于后续台词，自动翻译保持'+(app.state.session.translationEnabled?'开启':'关闭')+'。<br>当前句可单独重译，原记录保留。</p><div><button type="button" class="service-weak" data-service-action="configure">'+icon('settings')+'配置翻译服务</button><button type="button" data-service-action="close">完成</button></div></footer>';
    window.LunaControls?.upgrade(dialog);window.LunaIcons?.mount?.(dialog);updateExpanded();
  }
  function configure(id) {
    const s=app.state.settings,d=s.saved;
    ui.returnContext={origin:ui.origin,scope:ui.scope,gameId:ui.gameId,windows:clone(app.state.windows),returnFocus:ui.returnFocus};
    close();s.category='translation';s.purpose='translation';s.scope=ui.scope;
    if(local(id)){s.engineFamily='offline';window.LunaLocalTranslation.ensure(s).selected=id.slice(8);}
    else {
      const m=d.models[id]||d.models.traditional||Object.values(d.models).find(m=>m.templatePurpose==='translation'),c=m&&d.connections[m.connectionId];
      s.selectedModel=m?.id||'';s.selectedConnection=c?.id||Object.keys(d.connections)[0]||'';
      s.engineFamily=c?.protocol==='traditional'?'cloud':'llm';
      s.tab=c&&(!c.endpoint||c.auth==='key'&&!c.credential)?'connections':'models';s.testTarget='model';
    }
    s.notice='保存配置后返回选择；当前翻译服务与启停保持。';
    app.openWindow('settings');
  }
  function returnToPicker() {
    const context=ui.returnContext,s=app.state.settings;
    if(context) {
      Object.assign(app.state.windows,context.windows);ui.returnContext=null;
      ui.returnFocus=context.returnFocus;app.render();
      open({origin:context.origin,scope:context.scope,gameId:context.gameId});
    } else open({origin:'settings',scope:s.scope});
  }
  function triggerHTML(origin,sel) {
    const compact=origin==='hud',id=compact?'hud-translation-service':'session-translation-service';
    return '<button type="button" id="'+id+'" class="service-picker-trigger '+(compact?'service-hud-trigger':'')+'" data-service-action="open" data-origin="'+origin+'" aria-haspopup="dialog" aria-controls="translation-service-picker" aria-expanded="'+dialog.open+'" aria-label="翻译服务：'+esc(sel.mainName)+'">'+icon('languages')+'<span><small>翻译服务</small><strong>'+esc(sel.mainName)+'</strong></span>'+icon('chevron-down')+'</button>';
  }
  function updateExpanded() {
    document.querySelectorAll('.service-picker-trigger').forEach(button=>button.setAttribute('aria-expanded',String(dialog.open)));
  }
  function captureUnitSelection(target) {
    const capture=unit=>{
      const entry=activeEntry(target,unit.gameId);
      const explicit=unit.translationRequestMode==='explicit'||unit.recordId.includes(':retry:');
      const allowed=explicit||purposeEnabled(target,entry,unit.gameId)&&target.state.session.translationEnabled;
      return {recordId:unit.recordId,...selection(target,unit.gameId),extras:entry.extraIds.map(id=>{
        const snapshot=selectedSnapshot(entry,id),health=statusFor(target,id,snapshot);
        const status=!health.available?'blocked':!allowed?'disabled':ui.extraOutcomes[id]||'complete';
        return {id,name:nameFor(snapshot,id)||id,configVersion:health.configVersion,status,result:status==='complete'?unit.expectedTranslation||unit.translation||'此设计样句未提供对照译文。':'',reason:!health.available?health.reason:status==='disabled'?'自动翻译已关闭；本句未请求对照。':status==='failed'?'此对照服务的设计请求失败；其他结果保留。':status==='timeout'?'此对照服务的设计请求超时；其他结果保留。':''};
      })};
    };
    // Retranslating a non-current history record also creates a new identity.
    // Capture every new record once; never rewrite the provenance of its parent.
    target.state.history.forEach(record=>{
      if(!record.serviceSelection||record.serviceSelection.recordId!==record.recordId)record.serviceSelection=capture(record);
    });
    const unit=target.state.current;if(!unit)return;
    const record=target.state.history.find(r=>r.recordId===unit.recordId);
    if(!unit.serviceSelection||unit.serviceSelection.recordId!==unit.recordId)unit.serviceSelection=record?.serviceSelection?clone(record.serviceSelection):capture(unit);
  }
  function comparisonHTML(unit) {
    if(!unit?.serviceSelection?.extras?.length)return '';
    const labels={complete:'完成',failed:'失败',timeout:'超时',pending:'等待返回',cancelled:'已取消',disabled:'未请求',blocked:'不可用'};
    return '<div class="service-comparisons" aria-label="附加翻译对照">'+unit.serviceSelection.extras.map(extra=>'<div class="service-comparison" data-service-result-id="'+esc(extra.id)+'" data-service-result-state="'+esc(extra.status)+'"><small>'+esc(extra.name)+' · 附加对照 · 设计样本 · '+esc(labels[extra.status]||extra.status)+'</small><p>'+esc(extra.status==='complete'?extra.result:extra.status==='pending'?'等待此服务的对照设计返回…':extra.reason||'本句对照已取消；原文与其他结果保留。')+'</p></div>').join('')+'</div>';
  }
  function explicitValid(request) {
    const record=app.state.history.find(r=>r.recordId===request.recordId);
    return !!record&&record.revision===request.unitRevision&&record.gameId===request.gameId&&app.state.session.generation===request.generation&&activeEntry(app,request.gameId).revision===request.selectionRevision&&selectedStatus(app,request.entry).available;
  }
  function finishExplicit(request,status,text='') {
    clearTimeout(request.timer);explicitRequests.delete(request.recordId);
    const record=app.state.history.find(r=>r.recordId===request.recordId);if(!record)return;
    record.translationStatus=status;record.translation=text;
    if(app.state.current?.recordId===record.recordId)app.state.current=clone(record);
  }
  function auditExplicit() {
    let changed=false;
    for(const request of explicitRequests.values())if(!explicitValid(request)){finishExplicit(request,'cancelled');changed=true;}
    return changed;
  }
  function retranslateExplicit(record) {
    if(!record)return false;
    const entry=clone(activeEntry(app,record.gameId)),health=selectedStatus(app,entry);
    if(!health.available){app.notify(health.reason||'所选翻译服务暂不可用，请先配置。');return false;}
    const id=record.recordId+':retry:single:'+Date.now()+':'+(++explicitSequence);
    const copy={...clone(record),id,recordId:id,revision:(record.revision||1)+1,provider:nameFor(selectedSnapshot(entry),entry.mainId)||'所选翻译服务',configRevision:entry.revision,translation:'',translationStatus:'pending',edited:false,editedTranslation:undefined,translationRequestMode:'explicit',time:new Date().toLocaleTimeString('zh-CN',{hour12:false,hour:'2-digit',minute:'2-digit'})};
    delete copy.serviceSelection;
    app.state.history.push(copy);app.state.historySelection=id;
    if(app.state.current?.recordId===record.recordId)app.state.current=clone(copy);
    const request={recordId:id,gameId:copy.gameId,unitRevision:copy.revision,generation:app.state.session.generation,selectionRevision:entry.revision,entry};
    explicitRequests.set(id,request);app.render();
    // A single page-local sample response. Automatic/purpose enablement never changes.
    request.timer=setTimeout(()=>{
      if(!explicitRequests.has(id))return;
      if(!explicitValid(request))finishExplicit(request,'cancelled');
      else finishExplicit(request,app.state.fault==='translation'?'failed':'complete',app.state.fault==='translation'?'':copy.expectedTranslation||record.translation||'此原句没有预置设计译文。');
      app.render();
    },app.state.fault==='late'?1800:800);
    app.notify('仅重译所选这一句；自动翻译保持关闭。新结果独立保存，原记录保留。');return true;
  }
  function installComparisonTools() {
    const footer=document.querySelector('.review-footer');if(!footer)return;
    let tools=document.getElementById('translation-service-design-tools');
    if(!tools){tools=document.createElement('details');tools.id='translation-service-design-tools';tools.className='local-design-tools';footer.append(tools);}
    const items=list(app);if(!items.some(item=>item.id===ui.extraToolId))ui.extraToolId=items.find(item=>selection(app).extraIds.includes(item.id))?.id||items[0]?.id||'';
    tools.innerHTML='<summary>设计工具 · 单项对照返回模拟（待评审）</summary><p>只模拟指定对照的返回。没有真实网络或引擎任务；不更换服务、不改变自动翻译开关。</p><label>对照服务 <select id="service-comparison-engine" data-service-tool="engine" aria-label="模拟对照服务">'+items.map(item=>'<option value="'+esc(item.id)+'" '+(item.id===ui.extraToolId?'selected':'')+'>'+esc(item.name)+'</option>').join('')+'</select></label><label>此服务返回 <select id="service-comparison-outcome" data-service-tool="outcome" aria-label="单项对照模拟结果">'+[['complete','设计样句完成'],['pending','等待返回'],['failed','此项失败'],['timeout','此项超时']].map(([id,label])=>'<option value="'+id+'" '+((ui.extraOutcomes[ui.extraToolId]||'complete')===id?'selected':'')+'>'+label+'</option>').join('')+'</select></label><button type="button" data-service-action="simulate-extra">模拟当前句的此项返回</button>';
    window.LunaControls?.upgrade(tools);
  }
  function simulateComparison() {
    const unit=app.state.current,record=unit&&app.state.history.find(r=>r.recordId===unit.recordId),extra=record?.serviceSelection?.extras.find(e=>e.id===ui.extraToolId);
    if(!extra){app.notify('当前句没有这个服务的附加对照；请在选择器添加后推进新句。');return;}
    const entry=activeEntry(app,unit.gameId),health=statusFor(app,extra.id,selectedSnapshot(entry,extra.id));
    if(record.serviceSelection.revision!==entry.revision||['cancelled','disabled','blocked'].includes(extra.status)||!health.available){app.notify('此句的对照身份已失效或未请求；不会把新返回写入旧任务。');return;}
    extra.status=ui.extraOutcomes[extra.id]||'complete';extra.result=extra.status==='complete'?unit.expectedTranslation||unit.translation||'此设计样句未提供对照译文。':'';
    extra.reason=extra.status==='failed'?'此对照服务的设计请求失败；主译文与其他结果保留。':extra.status==='timeout'?'此对照服务的设计请求超时；主译文与其他结果保留。':'';
    unit.serviceSelection=clone(record.serviceSelection);app.render();
  }
  function decorate(target) {
    if(auditExplicit())queueMicrotask(()=>app.render());
    const sel=selection(target,currentGame()),main=document.getElementById('main-content');
    if(target.state.page==='session') {
      const card=document.createElement('section');card.id='translation-service-current';card.className='service-current';card.setAttribute('aria-label','当前翻译服务');
      card.innerHTML=triggerHTML('main',sel)+'<div class="service-current-copy"><span>'+esc(sel.source)+(sel.extraIds.length?' · '+sel.extraIds.length+' 个附加对照':'')+'</span><small>用于后续台词 · 自动翻译'+(target.state.session.translationEnabled?'开启':'关闭')+'</small></div>';
      const tabs=main.querySelector('.rw-session-tabs'),header=main.querySelector('.page-header');
      if(tabs)tabs.before(card);else if(header)header.after(card);else main.prepend(card);
    }
    const prior=document.getElementById('hud-translation-service');if(prior)prior.remove();
    const tools=document.querySelector('.hud-window-tools');tools?.before(document.createRange().createContextualFragment(triggerHTML('hud',sel)));
    if(target.state.windows.settings&&target.state.settings.category==='translation') {
      const scroll=document.querySelector('#settings-content .ls-scroll');
      const active=selection(target,target.state.settings.scope==='game'?(ui.returnContext?.gameId||currentGame()||target.state.selectedGame):null);
      const banner=document.createElement('div');banner.className='service-settings-return';
      banner.innerHTML='<button type="button" class="service-weak" data-service-action="return">'+icon('arrow-left')+'返回选择翻译服务</button><span>当前：'+esc(active.mainName)+'<br><small>保存配置不会切换服务'+(target.state.settings.dirty?'；未保存编辑不参与选择':'')+'</small></span>';
      scroll?.prepend(banner);
    }
    captureUnitSelection(target);
    target.state.history.forEach(record=>record.serviceSelection?.extras.forEach(extra=>{
      if(extra.status!=='pending')return;
      const health=statusFor(target,extra.id,selectedSnapshot(activeEntry(target,record.gameId),extra.id));
      if(!health.available){extra.status='blocked';extra.reason=health.reason;return;}
      if(record.serviceSelection.revision!==activeEntry(target,record.gameId).revision){extra.status='cancelled';extra.reason='服务选择已改变；迟到对照不会覆盖此句。';}
      else if(!target.state.session.translationEnabled&&record.translationRequestMode!=='explicit'&&!record.recordId.includes(':retry:')){extra.status='cancelled';extra.reason='自动翻译已关闭，此对照设计任务已取消。';}
    }));
    if(target.state.current){const record=target.state.history.find(r=>r.recordId===target.state.current.recordId);if(record?.serviceSelection)target.state.current.serviceSelection=clone(record.serviceSelection);}
    const comparisons=comparisonHTML(target.state.current);
    if(comparisons) {
      document.getElementById('hud-translations')?.insertAdjacentHTML('beforeend',comparisons);
      main.querySelector('.reading-unit')?.insertAdjacentHTML('beforeend',comparisons);
    }
    updateExpanded();
    installComparisonTools();
  }
  const originalAfterRender=window.LunaReviewGaps.afterRender;
  window.LunaReviewGaps.afterRender=function(target,...args){originalAfterRender(target,...args);decorate(target);};
  dialog.addEventListener('cancel',event=>{event.preventDefault();close();});
  dialog.addEventListener('keydown',event=>{event.stopPropagation();if((event.metaKey||event.ctrlKey)&&event.key===',')event.preventDefault();});
  document.addEventListener('click',event=>{
    const el=event.target.closest('[data-service-action]');if(!el)return;
    const action=el.dataset.serviceAction;event.preventDefault();
    if(action==='open')open({origin:el.dataset.origin||'settings',scope:el.dataset.origin==='settings'?app.state.settings.scope:undefined});
    else if(action==='close')close();
    else if(action==='choose')choose(el.dataset.value);
    else if(action==='configure')configure(el.dataset.value);
    else if(action==='return')returnToPicker();
    else if(action==='inherit')inheritGame();
    else if(action==='simulate-extra')simulateComparison();
  });
  document.addEventListener('change',event=>{
    if(event.target.dataset.serviceField==='scope')setScope(event.target.value);
    if(event.target.dataset.serviceExtra)choose(event.target.dataset.serviceExtra,{role:'extra'});
    if(event.target.dataset.serviceTool==='engine'){ui.extraToolId=event.target.value;installComparisonTools();}
    if(event.target.dataset.serviceTool==='outcome')ui.extraOutcomes[ui.extraToolId]=event.target.value;
  });
  // The existing explicit switch is the only operation that enables automatic use.
  document.addEventListener('click',event=>{
    const explicit=event.target.closest('[data-action="retranslate-current"],[data-action="retranslate-history"]');
    if(explicit&&!app.state.session.translationEnabled){
      event.preventDefault();event.stopImmediatePropagation();
      const record=explicit.dataset.action==='retranslate-current'?app.state.current:app.state.history.find(r=>r.recordId===app.state.historySelection)||app.state.history.at(-1);
      retranslateExplicit(record);return;
    }
    const button=event.target.closest('[data-action="toggle-translation"]');if(!button)return;
    const store=ensure(app),key=currentGame()||'global';if(!store.enabledByScope)store.enabledByScope={};
    if(app.state.session.translationEnabled){store.enabledByScope[key]=false;return;}
    const entry=activeEntry(app,currentGame()),status=selectedStatus(app,entry);
    if(!status.available){event.preventDefault();event.stopImmediatePropagation();open({origin:button.id==='hud-translation-toggle'?'hud':'main'});app.notify('先选择已配置且可用的翻译服务。');return;}
    store.enabledByScope[key]=true;
  },true);
  ensure(app);if(!activeEntry(app,currentGame()).enabled)app.state.session.translationEnabled=false;
  window.LunaTranslationServices={list,selection,open,close,choose,setScope,inheritGame,configure,returnToPicker,retranslateExplicit,ui};
  app.render();
})();
